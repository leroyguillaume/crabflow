use std::{future::Future, marker::PhantomData};

use crabflow_common::db::{
    DatabaseClient, DatabasePool, DatabaseTransaction, DefaultDatabaseConnection,
    DefaultDatabasePool, DefaultDatabaseTransaction,
};
use crabflow_core::{Workflow, WorkflowDesc, WorkflowState};
use futures::StreamExt;
use tracing::instrument;

use crate::{Args, Result};

pub trait Scheduler: Send + Sync {
    fn schedule(&self) -> impl Future<Output = Result>;
}

pub struct DefaultScheduler<
    DB: DatabasePool<DBCONN, DBTX>,
    DBCONN: DatabaseClient,
    DBTX: DatabaseTransaction,
> {
    db: DB,
    _dbconn: PhantomData<DBCONN>,
    _dbtx: PhantomData<DBTX>,
}

impl
    DefaultScheduler<DefaultDatabasePool, DefaultDatabaseConnection, DefaultDatabaseTransaction<'_>>
{
    #[instrument(skip(args))]
    pub async fn init(args: Args) -> Result<Self> {
        let db = DefaultDatabasePool::init(args.db.into()).await?;
        Ok(Self {
            db,
            _dbconn: PhantomData,
            _dbtx: PhantomData,
        })
    }
}

impl<DB: DatabasePool<DBCONN, DBTX>, DBCONN: DatabaseClient, DBTX: DatabaseTransaction>
    DefaultScheduler<DB, DBCONN, DBTX>
{
    async fn execute(_workflow: &Workflow, _desc: &WorkflowDesc) -> Result {
        Ok(())
    }

    async fn load_workflow(_workflow: &Workflow) -> Result {
        Ok(())
    }
}

impl<DB: DatabasePool<DBCONN, DBTX>, DBCONN: DatabaseClient, DBTX: DatabaseTransaction> Scheduler
    for DefaultScheduler<DB, DBCONN, DBTX>
{
    #[instrument(skip(self))]
    async fn schedule(&self) -> Result {
        let mut db = self.db.acquire().await?;
        let mut workflows = db.all_workflows().await?;
        while let Some(workflow) = workflows.next().await {
            let workflow = workflow?;
            match &workflow.state {
                WorkflowState::Created => Self::load_workflow(&workflow).await?,
                WorkflowState::Loaded(desc) => Self::execute(&workflow, desc).await?,
                _ => {}
            }
        }
        Ok(())
    }
}
