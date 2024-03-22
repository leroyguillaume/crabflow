use std::{future::Future, marker::PhantomData, path::PathBuf};

use crabflow_common::db::{DatabaseClient, DatabasePool, DatabaseTransaction};
use crabflow_core::{Image, WorkflowState};
use serde::Serialize;
use tracing::{debug, info, instrument, warn};

use crate::{cargo::CargoClient, docker::DockerClient, renderer::Renderer, Result};

pub enum BuilderMode<
    DB: DatabasePool<DBCONN, DBTX>,
    DBCONN: DatabaseClient,
    DBTX: DatabaseTransaction,
> {
    BuildOnly,
    Normal {
        db: DB,
        _dbconn: PhantomData<DBCONN>,
        _dbtx: PhantomData<DBTX>,
    },
}

impl<DB: DatabasePool<DBCONN, DBTX>, DBCONN: DatabaseClient, DBTX: DatabaseTransaction>
    BuilderMode<DB, DBCONN, DBTX>
{
    pub fn build_only() -> Self {
        Self::BuildOnly
    }

    pub fn normal(db: DB) -> Self {
        Self::Normal {
            db,
            _dbconn: PhantomData,
            _dbtx: PhantomData,
        }
    }
}

pub trait Builder {
    fn build(&self) -> impl Future<Output = Result>;
}

pub struct DefaultBuilder<
    CARGO: CargoClient,
    DB: DatabasePool<DBCONN, DBTX>,
    DBCONN: DatabaseClient,
    DBTX: DatabaseTransaction,
    DOCKER: DockerClient,
    RENDERER: Renderer,
> {
    pub cargo: CARGO,
    pub docker: DOCKER,
    pub mode: BuilderMode<DB, DBCONN, DBTX>,
    pub registry: Option<String>,
    pub renderer: RENDERER,
    pub workflows_dir: PathBuf,
}

impl<
        CARGO: CargoClient,
        DB: DatabasePool<DBCONN, DBTX>,
        DBCONN: DatabaseClient,
        DBTX: DatabaseTransaction,
        DOCKER: DockerClient,
        RENDERER: Renderer,
    > Builder for DefaultBuilder<CARGO, DB, DBCONN, DBTX, DOCKER, RENDERER>
{
    #[instrument(skip(self))]
    async fn build(&self) -> Result {
        let targets = self.cargo.load_targets(&self.workflows_dir).await?;
        let dockerfile_template = include_str!("../resources/main/Dockerfile.liquid");
        let dockerfile_path = self.workflows_dir.join("Dockerfile");
        let dockerfile_template_values = DockerfileTemplateValues { targets: &targets };
        debug!("converting dockerfile template values into json");
        let dockerfile_template_values = serde_json::to_value(dockerfile_template_values)?;
        self.renderer.render(
            dockerfile_template,
            &dockerfile_path,
            &dockerfile_template_values,
        )?;
        for target in targets {
            let tag = if let Some(registry) = &self.registry {
                format!("{registry}/{target}")
            } else {
                target.clone()
            };
            info!(
                workflow.image.tag = tag,
                workflow.image.target = target,
                "building image"
            );
            self.docker
                .build(&tag, &target, &self.workflows_dir)
                .await?;
            info!(
                workflow.image.tag = tag,
                workflow.image.target = target,
                "image successfully built"
            );
            if let BuilderMode::Normal { db, .. } = &self.mode {
                info!(
                    workflow.image.tag = tag,
                    workflow.image.target = target,
                    "pushing image"
                );
                self.docker.push(&tag).await?;
                info!(
                    workflow.image.tag = tag,
                    workflow.image.target = target,
                    "image successfully pushed"
                );
                let mut db = db.acquire().await?;
                if let Some(mut workflow) = db.workflow_by_target(&target).await? {
                    workflow.image = Image { tag, target };
                    workflow.state = WorkflowState::Created;
                    if db.update_workflow_safely(&workflow).await? {
                        info!(
                            workflow.image.tag,
                            workflow.image.target, "workflow updated"
                        );
                    } else {
                        warn!(
                            workflow.image.tag,
                            workflow.image.target,
                            "workflow has not been updated because it's locked"
                        );
                    }
                } else {
                    let image = Image { tag, target };
                    let workflow = db.insert_workflow(&image).await?;
                    info!(
                        workflow.image.tag,
                        workflow.image.target, "workflow created"
                    );
                };
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct DockerfileTemplateValues<'a> {
    targets: &'a [String],
}
