use std::{future::Future, marker::PhantomData};

use crabflow_common::db::{transactional, DatabaseClient, DatabasePool, DatabaseTransaction};
use crabflow_core::{Workflow, WorkflowDesc, WorkflowState};
use futures::StreamExt;
use tracing::{error, info, info_span, instrument, warn, Instrument};

use crate::{executor::Executor, Result};

pub trait Scheduler: Send + Sync {
    fn schedule(&self) -> impl Future<Output = Result>;
}

pub struct DefaultScheduler<
    DB: DatabasePool<DBCONN, DBTX>,
    DBCONN: DatabaseClient,
    DBTX: DatabaseTransaction,
    EXEC: Executor,
> {
    pub db: DB,
    pub executor: EXEC,
    pub _dbconn: PhantomData<DBCONN>,
    pub _dbtx: PhantomData<DBTX>,
}

impl<
        DB: DatabasePool<DBCONN, DBTX>,
        DBCONN: DatabaseClient,
        DBTX: DatabaseTransaction,
        EXEC: Executor,
    > DefaultScheduler<DB, DBCONN, DBTX, EXEC>
{
    fn log_potential_error<T>(res: Result<T>) {
        if let Err(err) = res {
            error!("{err}");
        }
    }

    async fn execute(_workflow: &Workflow, _desc: &WorkflowDesc) -> Result {
        Ok(())
    }

    async fn load_workflow(&self, workflow: &mut Workflow) -> Result {
        let mut tx = self.db.begin().await?;
        transactional!(tx, async {
            let name = format!("workflow-{}-load", workflow.image.target);
            workflow.state = WorkflowState::Loading;
            let updated = tx.update_workflow_safely(workflow).await?;
            if updated {
                let id = self
                    .executor
                    .start(&name, &workflow.image.tag, &["load".into()])
                    .await?;
                let loading = tx
                    .insert_workflow_loading(&id, &workflow.image.target)
                    .await?;
                info!(
                    workflow.loading.id = loading.id,
                    "loading of workflow started"
                );
            } else {
                warn!("workflow has not been updated because it's locked");
            }
            Ok(())
        })
    }
}

impl<
        DB: DatabasePool<DBCONN, DBTX>,
        DBCONN: DatabaseClient,
        DBTX: DatabaseTransaction,
        EXEC: Executor,
    > Scheduler for DefaultScheduler<DB, DBCONN, DBTX, EXEC>
{
    #[instrument(skip(self))]
    async fn schedule(&self) -> Result {
        let mut db = self.db.acquire().await?;
        let mut workflows = db.all_workflows().await?;
        while let Some(workflow) = workflows.next().await {
            let mut workflow = workflow?;
            match &workflow.state {
                WorkflowState::Created => {
                    let span =
                        info_span!("load_workflow", workflow.image.tag, workflow.image.target);
                    async {
                        let res = self.load_workflow(&mut workflow).await;
                        Self::log_potential_error(res);
                    }
                    .instrument(span)
                    .await;
                }
                WorkflowState::Loaded(desc) => Self::execute(&workflow, desc).await?,
                _ => {}
            }
        }
        Ok(())
    }
}
