use std::{future::Future, path::Path};

use crabflow_core::{Image, Workflow, WorkflowState};
use futures::{Stream, StreamExt};
use sqlx::{
    migrate::{MigrateError, Migrator},
    pool::PoolConnection,
    postgres::PgConnectOptions,
    query, query_as, Acquire, PgConnection, PgPool, Postgres, Transaction,
};
use tracing::{debug, debug_span, info, instrument, trace, Instrument, Level};

macro_rules! impl_client {
    ($ty:ty) => {
        impl DatabaseClient for $ty {
            #[instrument(level = Level::DEBUG, skip(self))]
            async fn all_workflows(&mut self) -> Result<impl Stream<Item = Result<Workflow>>> {
                all_workflows(&mut self.0).await
            }

            async fn delete_workflow(&mut self, id: &str) -> Result<bool> {
                let span = debug_span!("delete_workflow", workflow.id = id);
                delete_workflow(id, &mut self.0).instrument(span).await
            }

            async fn insert_workflow(&mut self, image: &Image) -> Result<Workflow> {
                let span = debug_span!(
                    "insert_workflow",
                    workflow.image.tag = image.tag,
                    workflow.image.target = image.target
                );
                insert_workflow(&image, &mut self.0).instrument(span).await
            }

            async fn update_workflow(&mut self, workflow: &Workflow) -> Result<bool> {
                let span =
                    debug_span!("update_workflow", workflow.image.tag, workflow.image.target,);
                update_workflow(workflow, false, &mut self.0)
                    .instrument(span)
                    .await
            }

            async fn update_workflow_safely(&mut self, workflow: &Workflow) -> Result<bool> {
                let span = debug_span!(
                    "update_workflow_safely",
                    workflow.image.tag,
                    workflow.image.target,
                );
                update_workflow(workflow, true, &mut self.0)
                    .instrument(span)
                    .await
            }

            async fn workflow_by_target(&mut self, target: &str) -> Result<Option<Workflow>> {
                let span = debug_span!("workflow_by_target", workflow.image.target = target);
                workflow_by_target(target, &mut self.0)
                    .instrument(span)
                    .await
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
    #[error("json error: {0}")]
    Json(
        #[from]
        #[source]
        serde_json::Error,
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

    fn insert_workflow(&mut self, image: &Image) -> impl Future<Output = Result<Workflow>>;

    fn update_workflow(&mut self, workflow: &Workflow) -> impl Future<Output = Result<bool>>;

    fn update_workflow_safely(&mut self, workflow: &Workflow)
        -> impl Future<Output = Result<bool>>;

    fn workflow_by_target(
        &mut self,
        target: &str,
    ) -> impl Future<Output = Result<Option<Workflow>>>;
}

pub trait DatabasePool<CONN: DatabaseClient, TX: DatabaseTransaction>: Send + Sync {
    fn acquire(&self) -> impl Future<Output = Result<CONN>>;

    fn begin(&self) -> impl Future<Output = Result<TX>>;

    fn run_migrations(&self) -> impl Future<Output = Result>;
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
        let db = PgPool::connect_with(opts).await?;
        Ok(DefaultDatabasePool(db))
    }
}

impl<'a> DatabasePool<DefaultDatabaseConnection, DefaultDatabaseTransaction<'a>>
    for DefaultDatabasePool
{
    async fn acquire(&self) -> Result<DefaultDatabaseConnection> {
        debug!("acquiring database connection");
        let db = self.0.acquire().await?;
        Ok(DefaultDatabaseConnection(db))
    }

    async fn begin(&self) -> Result<DefaultDatabaseTransaction<'a>> {
        debug!("beginning transaction");
        let tx = self.0.begin().await?;
        Ok(DefaultDatabaseTransaction(tx))
    }

    async fn run_migrations(&self) -> Result {
        debug!("loading database migrations");
        let migrator = Migrator::new(Path::new("resources/main/db/migrations")).await?;
        info!("running database migrations");
        migrator.run(&self.0).await?;
        Ok(())
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

async fn all_workflows<
    'a,
    A: Acquire<'a, Database = Postgres, Connection = &'a mut PgConnection>,
>(
    db: A,
) -> Result<impl Stream<Item = Result<Workflow>> + 'a> {
    trace!("acquiring database connection");
    let db = db.acquire().await?;
    let sql = include_str!("../resources/main/db/queries/all-workflows.sql");
    debug!("fetching all workflows");
    let workflows = query_as(sql).fetch(db).map(|row| row.map_err(Error::from));
    Ok(workflows)
}

async fn delete_workflow<
    'a,
    A: Acquire<'a, Database = Postgres, Connection = &'a mut PgConnection>,
>(
    id: &str,
    db: A,
) -> Result<bool> {
    trace!("acquiring database connection");
    let db = db.acquire().await?;
    let sql = include_str!("../resources/main/db/queries/delete-workflow.sql");
    debug!("deleting workflow workflow");
    let res = query(sql).bind(id).execute(db).await?;
    Ok(res.rows_affected() > 0)
}

async fn insert_workflow<
    'a,
    A: Acquire<'a, Database = Postgres, Connection = &'a mut PgConnection>,
>(
    image: &Image,
    db: A,
) -> Result<Workflow> {
    trace!("acquiring database connection");
    let db = db.acquire().await?;
    let sql = include_str!("../resources/main/db/queries/insert-workflow.sql");
    debug!("inserting workflow");
    let workflow = query_as(sql)
        .bind(&image.target)
        .bind(&image.tag)
        .fetch_one(db)
        .await?;
    Ok(workflow)
}

async fn update_workflow<
    'a,
    A: Acquire<'a, Database = Postgres, Connection = &'a mut PgConnection>,
>(
    workflow: &Workflow,
    safely: bool,
    db: A,
) -> Result<bool> {
    trace!("acquiring database connection");
    let db = db.acquire().await?;
    let sql = if safely {
        include_str!("../resources/main/db/queries/update-workflow.sql")
    } else {
        include_str!("../resources/main/db/queries/update-workflow-safely.sql")
    };
    let desc = match &workflow.state {
        WorkflowState::Loaded(desc) => {
            let value = serde_json::to_value(desc)?;
            Some(value)
        }
        _ => None,
    };
    debug!("updating workflow");
    let res = query(sql)
        .bind(&workflow.image.target)
        .bind(&workflow.image.tag)
        .bind(&workflow.state)
        .bind(desc)
        .execute(db)
        .await?;
    Ok(res.rows_affected() > 0)
}

async fn workflow_by_target<
    'a,
    A: Acquire<'a, Database = Postgres, Connection = &'a mut PgConnection>,
>(
    target: &str,
    db: A,
) -> Result<Option<Workflow>> {
    trace!("acquiring database connection");
    let db = db.acquire().await?;
    let sql = include_str!("../resources/main/db/queries/workflow-by-target.sql");
    debug!("fetching workflow");
    let workflow = query_as(sql).bind(target).fetch_optional(db).await?;
    Ok(workflow)
}
