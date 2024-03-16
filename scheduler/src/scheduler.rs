use std::{future::Future, marker::PhantomData};

use crabflow_common::{
    clap::DatabaseOptions,
    db::{
        DatabaseClient, DatabasePool, DatabaseTransaction, DefaultDatabaseConnection,
        DefaultDatabasePool, DefaultDatabaseTransaction,
    },
};
use tracing::instrument;

use crate::Result;

pub trait Scheduler: Send + Sync {
    fn schedule(&self) -> impl Future<Output = Result>;
}

pub struct DefaultScheduler<
    DB: DatabasePool<DBCONN, DBTX>,
    DBCONN: DatabaseClient,
    DBTX: DatabaseTransaction,
> {
    _db: DB,
    _dbconn: PhantomData<DBCONN>,
    _dbtx: PhantomData<DBTX>,
}

impl
    DefaultScheduler<DefaultDatabasePool, DefaultDatabaseConnection, DefaultDatabaseTransaction<'_>>
{
    #[instrument]
    pub async fn init(opts: DatabaseOptions) -> Result<Self> {
        let db = DefaultDatabasePool::init(opts.into()).await?;
        Ok(Self {
            _db: db,
            _dbconn: PhantomData,
            _dbtx: PhantomData,
        })
    }
}

impl<DB: DatabasePool<DBCONN, DBTX>, DBCONN: DatabaseClient, DBTX: DatabaseTransaction> Scheduler
    for DefaultScheduler<DB, DBCONN, DBTX>
{
    #[instrument(skip(self))]
    async fn schedule(&self) -> Result {
        Ok(())
    }
}
