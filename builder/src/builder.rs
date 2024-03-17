use std::{
    fs::File,
    future::Future,
    marker::PhantomData,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use crabflow_common::db::{
    DatabaseClient, DatabasePool, DatabaseTransaction, DefaultDatabaseConnection,
    DefaultDatabasePool, DefaultDatabaseTransaction,
};
use crabflow_core::{Image, WorkflowState};
use liquid::{object, Parser, ParserBuilder};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, instrument, warn, Level};

use crate::{Args, Error, Result};

const CMD_CARGO: &str = "cargo";
const CMD_DOCKER: &str = "docker";

pub trait CargoClient {
    fn load_targets(&self, dir_path: &Path) -> Result<Vec<String>>;
}

pub trait DockerClient {
    fn build(&self, target: &str, tag: &str, path: &Path) -> Result;

    fn push(&self, tag: &str) -> Result;
}

pub trait Builder {
    fn build(&self) -> impl Future<Output = Result>;
}

pub trait Renderer {
    fn render(&self, template: &str, path: &Path, targets: &[String]) -> Result;
}

pub struct DefaultCargoClient;

impl CargoClient for DefaultCargoClient {
    #[instrument(level = Level::DEBUG, skip(self))]
    fn load_targets(&self, dir_path: &Path) -> Result<Vec<String>> {
        debug!("running cargo");
        let output = Command::new(CMD_CARGO)
            .current_dir(dir_path)
            .arg("read-manifest")
            .output()?;
        let stdout = command_stdout(CMD_CARGO, output)?;
        debug!("decoding cargo output as json");
        let manifest: Manifest = serde_json::from_str(&stdout)?;
        let targets = manifest
            .targets
            .into_iter()
            .filter_map(|target| {
                target
                    .kind
                    .iter()
                    .any(|kind| kind == "bin")
                    .then_some(target.name)
            })
            .collect();
        Ok(targets)
    }
}

pub struct DefaultBuilder<
    CARGO: CargoClient,
    DB: DatabasePool<DBCONN, DBTX>,
    DBCONN: DatabaseClient,
    DBTX: DatabaseTransaction,
    DOCKER: DockerClient,
    RENDERER: Renderer,
> {
    cargo: CARGO,
    docker: DOCKER,
    path: PathBuf,
    registry: Option<String>,
    renderer: RENDERER,
    run_kind: RunKind<DB, DBCONN, DBTX>,
}

impl
    DefaultBuilder<
        DefaultCargoClient,
        DefaultDatabasePool,
        DefaultDatabaseConnection,
        DefaultDatabaseTransaction<'_>,
        DefaultDockerClient,
        DefaultRenderer,
    >
{
    #[instrument]
    pub async fn init(args: Args) -> Result<Self> {
        let run_kind = if args.build_only {
            RunKind::BuildOnly
        } else {
            let db = DefaultDatabasePool::init(args.db.into()).await?;
            RunKind::full(db)
        };
        debug!("creating Liquid parser");
        let parser = ParserBuilder::with_stdlib().build()?;
        Ok(Self {
            cargo: DefaultCargoClient,
            docker: DefaultDockerClient {
                url: args.docker_url,
            },
            path: args.path,
            registry: args.registry,
            renderer: DefaultRenderer { parser },
            run_kind,
        })
    }
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
        let targets = self.cargo.load_targets(&self.path)?;
        let dockerfile_template = include_str!("../resources/main/Dockerfile.liquid");
        let dockerfile_path = self.path.join("Dockerfile");
        self.renderer
            .render(dockerfile_template, &dockerfile_path, &targets)?;
        for target in targets {
            let tag = if let Some(registry) = &self.registry {
                format!("{registry}/{target}")
            } else {
                target.clone()
            };
            info!(tag, target, "building image");
            self.docker.build(&target, &tag, &self.path)?;
            info!(tag, target, "image successfully built");
            if let RunKind::Full { db, .. } = &self.run_kind {
                info!(tag, target, "pushing image");
                self.docker.push(&tag)?;
                info!(tag, target, "image successfully pushed");
                let image = Image { tag, target };
                let mut db = db.acquire().await?;
                if let Some(mut workflow) = db.workflow_by_target(&image.target).await? {
                    workflow.image = image;
                    workflow.state = WorkflowState::Created;
                    if db.update_workflow(&workflow).await? {
                        info!(
                            tag = workflow.image.tag,
                            target = workflow.image.target,
                            "workflow updated"
                        );
                    } else {
                        warn!(
                            tag = workflow.image.tag,
                            target = workflow.image.target,
                            "workflow has not been updated because it's locked"
                        );
                    }
                } else {
                    let workflow = db.insert_workflow(&image).await?;
                    info!(
                        tag = workflow.image.tag,
                        target = workflow.image.target,
                        "workflow created"
                    );
                };
            }
        }
        Ok(())
    }
}

pub struct DefaultDockerClient {
    url: String,
}

impl DockerClient for DefaultDockerClient {
    #[instrument(level = Level::DEBUG, skip(self))]
    fn build(&self, target: &str, tag: &str, dir_path: &Path) -> Result {
        debug!("running docker build");
        let output = Command::new(CMD_DOCKER)
            .current_dir(dir_path)
            .arg("-H")
            .arg(&self.url)
            .arg("build")
            .arg("-t")
            .arg(tag)
            .arg("--target")
            .arg(target)
            .arg(".")
            .output()?;
        command_stdout(CMD_DOCKER, output)?;
        Ok(())
    }

    #[instrument(level = Level::DEBUG, skip(self))]
    fn push(&self, tag: &str) -> Result {
        debug!("running docker push");
        let output = Command::new(CMD_DOCKER)
            .arg("-H")
            .arg(&self.url)
            .arg("push")
            .arg(tag)
            .output()?;
        command_stdout(CMD_DOCKER, output)?;
        Ok(())
    }
}

pub struct DefaultRenderer {
    parser: Parser,
}

impl Renderer for DefaultRenderer {
    #[instrument(level = Level::DEBUG, skip(self))]
    fn render(&self, template: &str, path: &Path, targets: &[String]) -> Result {
        debug!("parsing template");
        let template = self.parser.parse(template)?;
        let obj = object!({
            "targets": targets,
        });
        debug!("rendering template");
        let mut file = File::create(path)?;
        template.render_to(&mut file, &obj)?;
        Ok(())
    }
}

enum RunKind<DB: DatabasePool<DBCONN, DBTX>, DBCONN: DatabaseClient, DBTX: DatabaseTransaction> {
    BuildOnly,
    Full {
        db: DB,
        _dbconn: PhantomData<DBCONN>,
        _dbtx: PhantomData<DBTX>,
    },
}

impl<DB: DatabasePool<DBCONN, DBTX>, DBCONN: DatabaseClient, DBTX: DatabaseTransaction>
    RunKind<DB, DBCONN, DBTX>
{
    fn full(db: DB) -> Self {
        Self::Full {
            db,
            _dbconn: PhantomData,
            _dbtx: PhantomData,
        }
    }
}

fn command_stdout(command: &str, output: Output) -> Result<String> {
    debug!("decoding command output as utf-8");
    let stdout = String::from_utf8(output.stdout)?;
    if tracing::enabled!(Level::DEBUG) {
        for line in stdout.lines() {
            debug!("{command}: {line}");
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            debug!("{command}: {line}");
        }
    }
    if output.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            error!("{command}: {line}");
        }
        Err(Error::Command {
            name: command.into(),
            status: output.status,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Manifest {
    targets: Vec<ManifestTarget>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ManifestTarget {
    kind: Vec<String>,
    name: String,
}
