use std::time::Duration;

use clap::Parser;
use crabflow_common::clap::DatabaseOptions;
use tokio::{
    select,
    signal::unix::{signal, SignalKind},
    time::{sleep_until, Instant},
};
use tracing::{debug, error, info};

use crate::scheduler::{DefaultScheduler, Scheduler};

type Result<T = ()> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("{0}")]
    Database(
        #[from]
        #[source]
        crabflow_common::db::Error,
    ),
    #[error("i/o error: {0}")]
    Io(
        #[from]
        #[source]
        std::io::Error,
    ),
}

#[derive(Clone, Debug, Eq, Parser, PartialEq)]
#[command(version)]
struct Args {
    #[command(flatten)]
    db: DatabaseOptions,
    #[arg(
        short,
        long,
        env = "DELAY",
        help = "Number of seconds between two scheduler loops",
        default_value_t = 30
    )]
    delay: u64,
    #[arg(long, env = "POD_TEMPLATE", help = "JSON-encoded Pod template", default_value = include_str!("../resources/main/k8s/pod.json"))]
    pod_template: String,
}

#[crabflow_macros::main]
async fn run(args: Args) -> Result {
    let delay = Duration::from_secs(args.delay);
    let scheduler = DefaultScheduler::init(args).await?;
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;
    info!("scheduler started");
    loop {
        if let Err(err) = scheduler.schedule().await {
            error!("{err}");
        }
        select! {
            _ = sleep_until(Instant::now() + delay) => {}
            _ = sigint.recv() => {
                debug!("SIGINT received");
                break;
            }
            _ = sigterm.recv() => {
                debug!("SIGTERM received");
                break;
            }
        }
    }
    info!("scheduler stopped");
    Ok(())
}

mod scheduler;
