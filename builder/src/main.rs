use std::{
    collections::BTreeSet,
    path::PathBuf,
    process::{exit, ExitStatus},
    string::FromUtf8Error,
    time::Duration,
};

use builder::{Builder, DefaultBuilder};
use clap::Parser;
use crabflow_common::{clap::DatabaseOptions, init_tracing};
use tokio::{
    select,
    signal::unix::{signal, SignalKind},
    sync::mpsc::error::SendError,
};
use tracing::{debug, error, info};

use crate::{
    builder::BuilderConfig,
    watcher::{DefaultFileSystemWatcher, FileSystemWatcher},
};

type Result<T = ()> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("command {name} exited with {status}")]
    Command { name: String, status: ExitStatus },
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
    #[error("liquid error: {0}")]
    Liquid(
        #[from]
        #[source]
        liquid::Error,
    ),
    #[error("notify error: {0}")]
    Notify(
        #[from]
        #[source]
        notify::Error,
    ),
    #[error("json error: {0}")]
    Json(
        #[from]
        #[source]
        serde_json::Error,
    ),
    #[error("failed to send message to channel: {0}")]
    Send(
        #[from]
        #[source]
        SendError<()>,
    ),
    #[error("failed to decode utf-8: {0}")]
    Utf8(
        #[from]
        #[source]
        FromUtf8Error,
    ),
}

#[derive(Clone, Debug, Eq, Parser, PartialEq)]
#[command(version)]
struct Args {
    #[arg(short, long, env = "BUILD_ONLY", help = "Only build images")]
    build_only: bool,
    #[arg(
        long,
        env = "DEBOUNCE",
        help = "Minimal number of seconds between two notify events before to re-build",
        default_value_t = 1
    )]
    debounce: u64,
    #[command(flatten)]
    db: DatabaseOptions,
    #[arg(
        long,
        env = "DOCKER_URL",
        help = "URL to Docker daemon",
        default_value = "unix:///var/run/docker.sock"
    )]
    docker_url: String,
    #[arg(short, long, env = "IGNORING", help = "Paths to ignore")]
    ignoring: Vec<PathBuf>,
    #[arg(
        env = "WORKFLOWS_DIR",
        help = "Path to directory that contains workflows"
    )]
    path: PathBuf,
    #[arg(short, long, env = "REGISTRY", help = "Image registry")]
    registry: Option<String>,
    #[arg(short, long, help = "Watch changes on workflows directory")]
    watch: bool,
}

#[tokio::main]
async fn main() {
    init_tracing();
    let args = Args::parse();
    let rc = if let Err(err) = run(args).await {
        error!("{err}");
        1
    } else {
        0
    };
    exit(rc);
}

async fn run(args: Args) -> Result {
    let config = BuilderConfig {
        build_only: args.build_only,
        db: args.db,
        docker_url: args.docker_url,
        path: args.path,
        registry: args.registry,
    };
    let builder = DefaultBuilder::init(config).await?;
    if args.watch {
        let debounce = Duration::from_secs(args.debounce);
        let mut ignoring = BTreeSet::from_iter(args.ignoring);
        ignoring.insert("Dockerfile".into());
        let mut watcher = DefaultFileSystemWatcher::start(builder.path(), debounce, ignoring)?;
        let mut sigint = signal(SignalKind::interrupt())?;
        let mut sigterm = signal(SignalKind::terminate())?;
        info!("builder started");
        loop {
            select! {
                _ = watcher.recv() => {
                    if let Err(err) = builder.build().await {
                        error!("{err}");
                    }
                }
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
        info!("builder stopped");
    } else {
        builder.build().await?;
    }
    Ok(())
}

mod builder;
mod watcher;
