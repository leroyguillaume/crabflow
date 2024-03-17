use std::future::Future;

use chrono::{DateTime, Utc};
use crabflow_core::{Image, Workflow, WorkflowState};
use futures::{Stream, StreamExt};
use sqlx::{
    migrate::MigrateError, pool::PoolConnection, postgres::PgConnectOptions, query, query_as,
    Acquire, PgConnection, PgPool, Postgres, Transaction,
};
use tracing::{debug, info, instrument, trace, Level};

macro_rules! impl_client {
    ($ty:ty) => {
        impl DatabaseClient for $ty {
            #[instrument(level = Level::DEBUG, skip(self))]
            async fn all_workflows(&mut self) -> Result<impl Stream<Item = Result<Workflow>>> {
                all_workflows(&mut self.0).await
            }

            #[instrument(level = Level::DEBUG, skip(self))]
            async fn delete_workflow(&mut self, id: &str) -> Result<bool> {
                delete_workflow(id, &mut self.0).await
            }

            #[instrument(level = Level::DEBUG, skip(self))]
            async fn insert_workflow(&mut self, img: &Image) -> Result<Workflow> {
                insert_workflow(&img, &mut self.0).await
            }

            #[instrument(level = Level::DEBUG, skip(self))]
            async fn update_workflow(&mut self, workflow: &Workflow) -> Result<bool> {
                update_workflow(workflow, &mut self.0).await
            }

            #[instrument(level = Level::DEBUG, skip(self))]
            async fn workflow_by_target(&mut self, target: &str) -> Result<Option<Workflow>> {
                workflow_by_target(target, &mut self.0).await
            }
        }
    };
}

pub type Result<T = ()> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Database(
        #[from]
        #[source]
        sqlx::Error,
    ),
    #[error("database migration error: {0}")]
    Migrate(
        #[from]
        #[source]
        MigrateError,
    ),
}

pub trait DatabaseClient: Send + Sync {
    fn all_workflows(
        &mut self,
    ) -> impl Future<Output = Result<impl Stream<Item = Result<Workflow>>>>;

    fn delete_workflow(&mut self, id: &str) -> impl Future<Output = Result<bool>>;

    fn insert_workflow(&mut self, img: &Image) -> impl Future<Output = Result<Workflow>>;

    fn update_workflow(&mut self, workflow: &Workflow) -> impl Future<Output = Result<bool>>;

    fn workflow_by_target(
        &mut self,
        target: &str,
    ) -> impl Future<Output = Result<Option<Workflow>>>;
}

pub trait DatabasePool<CONN: DatabaseClient, TX: DatabaseTransaction>: Send + Sync {
    fn acquire(&self) -> impl Future<Output = Result<CONN>>;

    fn begin(&self) -> impl Future<Output = Result<TX>>;
}

pub trait DatabaseTransaction: DatabaseClient {
    fn commit(self) -> impl Future<Output = Result>;

    fn rollback(self) -> impl Future<Output = Result>;
}

pub struct DefaultDatabasePool(PgPool);

impl DefaultDatabasePool {
    #[instrument]
    pub async fn init(opts: PgConnectOptions) -> Result<Self> {
        info!("initializing database pool");
        let pool = PgPool::connect_with(opts).await?;
        info!("running database migrations");
        sqlx::migrate!("resources/main/db/migrations")
            .run(&pool)
            .await?;
        Ok(DefaultDatabasePool(pool))
    }
}

impl<'a> DatabasePool<DefaultDatabaseConnection, DefaultDatabaseTransaction<'a>>
    for DefaultDatabasePool
{
    async fn acquire(&self) -> Result<DefaultDatabaseConnection> {
        debug!("acquiring database connection");
        let conn = self.0.acquire().await?;
        Ok(DefaultDatabaseConnection(conn))
    }

    async fn begin(&self) -> Result<DefaultDatabaseTransaction<'a>> {
        debug!("beginning transaction");
        let tx = self.0.begin().await?;
        Ok(DefaultDatabaseTransaction(tx))
    }
}

pub struct DefaultDatabaseConnection(PoolConnection<Postgres>);

impl_client!(DefaultDatabaseConnection);

pub struct DefaultDatabaseTransaction<'a>(Transaction<'a, Postgres>);

impl_client!(DefaultDatabaseTransaction<'_>);

impl DatabaseTransaction for DefaultDatabaseTransaction<'_> {
    async fn commit(self) -> Result {
        self.0.commit().await?;
        Ok(())
    }

    async fn rollback(self) -> Result {
        self.0.rollback().await?;
        Ok(())
    }
}

struct WorkflowRecord {
    pub created_at: DateTime<Utc>,
    pub tag: String,
    pub target: String,
    pub state: WorkflowState,
}

impl From<WorkflowRecord> for Workflow {
    fn from(rec: WorkflowRecord) -> Self {
        Self {
            created_at: rec.created_at,
            img: Image {
                tag: rec.tag,
                target: rec.target,
            },
            state: rec.state,
        }
    }
}

async fn all_workflows<
    'a,
    A: Acquire<'a, Database = Postgres, Connection = &'a mut PgConnection>,
>(
    conn: A,
) -> Result<impl Stream<Item = Result<Workflow>> + 'a> {
    trace!("acquiring database connection");
    let conn = conn.acquire().await?;
    let sql = include_str!("../resources/main/db/queries/all-workflows.sql");
    debug!("fetching all workflows");
    let workflows = query_as(sql)
        .fetch(conn)
        .map(|row| row.map_err(Error::from));
    Ok(workflows)
}

async fn delete_workflow<
    'a,
    A: Acquire<'a, Database = Postgres, Connection = &'a mut PgConnection>,
>(
    id: &str,
    conn: A,
) -> Result<bool> {
    trace!("acquiring database connection");
    let conn = conn.acquire().await?;
    let sql = include_str!("../resources/main/db/queries/delete-workflow.sql");
    debug!("deleting workflow workflow");
    let res = query(sql).bind(id).execute(conn).await?;
    Ok(res.rows_affected() > 0)
}

async fn insert_workflow<
    'a,
    A: Acquire<'a, Database = Postgres, Connection = &'a mut PgConnection>,
>(
    img: &Image,
    conn: A,
) -> Result<Workflow> {
    trace!("acquiring database connection");
    let conn = conn.acquire().await?;
    let sql = include_str!("../resources/main/db/queries/insert-workflow.sql");
    debug!("inserting workflow");
    let workflow = query_as(sql)
        .bind(&img.target)
        .bind(&img.tag)
        .fetch_one(conn)
        .await?;
    Ok(workflow)
}

async fn update_workflow<
    'a,
    A: Acquire<'a, Database = Postgres, Connection = &'a mut PgConnection>,
>(
    workflow: &Workflow,
    conn: A,
) -> Result<bool> {
    trace!("acquiring database connection");
    let conn = conn.acquire().await?;
    let sql = include_str!("../resources/main/db/queries/update-workflow.sql");
    debug!("updating workflow");
    let res = query(sql)
        .bind(&workflow.img.target)
        .bind(&workflow.img.tag)
        .bind(workflow.state)
        .execute(conn)
        .await?;
    Ok(res.rows_affected() > 0)
}

async fn workflow_by_target<
    'a,
    A: Acquire<'a, Database = Postgres, Connection = &'a mut PgConnection>,
>(
    target: &str,
    conn: A,
) -> Result<Option<Workflow>> {
    trace!("acquiring database connection");
    let conn = conn.acquire().await?;
    let sql = include_str!("../resources/main/db/queries/workflow-by-target.sql");
    debug!("fetching workflow");
    let workflow = query_as(sql).bind(target).fetch_optional(conn).await?;
    Ok(workflow)
}
