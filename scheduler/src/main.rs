use std::{fs::File, marker::PhantomData, path::PathBuf, time::Duration};

use clap::Parser;
use crabflow_common::{clap::DatabaseOptions, db::DefaultDatabasePool};
use k8s_openapi::api::core::v1::Pod;
use mockable::DefaultUuidGenerator;
use tokio::{
    select,
    signal::unix::{signal, SignalKind},
    time::sleep,
};
use tracing::{debug, error, info};

use crate::{
    executor::DefaultExecutor,
    scheduler::{DefaultScheduler, Scheduler},
};

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
    #[error("kubernetes error: {0}")]
    Kube(
        #[from]
        #[source]
        kube::Error,
    ),
    #[error("pod-template: no container `{0}` found")]
    MissingPodContainer(String),
    #[error("pod-template: spec must be defined")]
    MissingPodSpec,
    #[error("yaml error: {0}")]
    Yaml(
        #[from]
        #[source]
        serde_yaml::Error,
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
    #[arg(
        long,
        env = "POD_TEMPLATE",
        help = "Path to Pod template file",
        default_value = "etc/pod-template.yaml"
    )]
    pod_template: PathBuf,
}

#[crabflow_macros::main]
async fn run(args: Args) -> Result {
    let delay = Duration::from_secs(args.delay);
    info!(path = %args.pod_template.display(), "loading pod template");
    debug!(path = %args.pod_template.display(), "opening pod template file");
    let mut file = File::open(&args.pod_template)?;
    debug!(path = %args.pod_template.display(), "decoding pod template from yaml");
    let pod_template: Pod = serde_yaml::from_reader(&mut file)?;
    drop(file);
    debug!("initializing kubernetes client");
    let k8s = kube::Client::try_default().await?;
    let db = DefaultDatabasePool::init(args.db.into()).await?;
    let scheduler = DefaultScheduler {
        db,
        executor: DefaultExecutor {
            k8s,
            pod_template,
            uuid_gen: DefaultUuidGenerator,
        },
        _dbconn: PhantomData,
        _dbtx: PhantomData,
    };
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;
    info!("scheduler started");
    loop {
        if let Err(err) = scheduler.schedule().await {
            error!("{err}");
        }
        select! {
            _ = sleep(delay) => {}
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

mod executor;
mod scheduler;
