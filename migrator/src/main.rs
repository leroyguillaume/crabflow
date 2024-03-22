use std::path::PathBuf;

use clap::Parser;
use crabflow_common::{
    clap::DatabaseOptions,
    db::{DatabasePool, DefaultDatabasePool},
};

pub type Result<T = ()> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(
    #[from]
    #[source]
    crabflow_common::db::Error,
);

#[derive(Clone, Debug, Eq, Parser, PartialEq)]
#[command(version)]
pub struct Args {
    #[command(flatten)]
    db: DatabaseOptions,
    #[arg(
        env = "MIGRATIONS_DIR",
        help = "Directory that contains migration files",
        default_value = "resources/main/db/migrations"
    )]
    migrations_dir: PathBuf,
}

#[crabflow_macros::main]
async fn run(args: Args) -> Result {
    let db = DefaultDatabasePool::init(args.db.into()).await?;
    db.run_migrations(&args.migrations_dir).await?;
    Ok(())
}
