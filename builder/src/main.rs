use std::{collections::BTreeSet, path::PathBuf, string::FromUtf8Error, sync::Arc, time::Duration};

use builder::{Builder, BuilderMode, DefaultBuilder};
use cargo::DefaultCargoClient;
use clap::Parser;
use cmd::DefaultCommandRunner;
use crabflow_common::{clap::DatabaseOptions, db::DefaultDatabasePool};
use docker::DefaultDockerClient;
use renderer::DefaultRenderer;
use tokio::{
    select,
    signal::unix::{signal, SignalKind},
    sync::mpsc::error::SendError,
};
use tracing::{debug, error, info};

use crate::watcher::{DefaultFileSystemEventHandler, DefaultFileSystemWatcher, FileSystemWatcher};

type Result<T = ()> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("command {0} failed")]
    Command(String),
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
    #[arg(short, long, env = "REGISTRY", help = "Image registry")]
    registry: Option<String>,
    #[arg(short, long, help = "Watch changes on workflows directory")]
    watch: bool,
    #[arg(
        env = "WORKFLOWS_DIR",
        help = "Path to directory that contains workflows"
    )]
    workflows_dir: PathBuf,
}

#[crabflow_macros::main]
async fn run(args: Args) -> Result {
    let runner = Arc::new(DefaultCommandRunner {
        runner: mockable::DefaultCommandRunner,
    });
    let mode = if args.build_only {
        let db = DefaultDatabasePool::init(args.db.into()).await?;
        BuilderMode::normal(db)
    } else {
        BuilderMode::build_only()
    };
    let builder = DefaultBuilder {
        cargo: DefaultCargoClient {
            runner: runner.clone(),
        },
        docker: DefaultDockerClient {
            runner,
            url: args.docker_url,
        },
        mode,
        registry: args.registry,
        renderer: DefaultRenderer::init()?,
        workflows_dir: args.workflows_dir,
    };
    if args.watch {
        let debounce = Duration::from_secs(args.debounce);
        let mut ignoring = BTreeSet::from_iter(args.ignoring);
        ignoring.insert("Dockerfile".into());
        let handler = DefaultFileSystemEventHandler { ignoring };
        let mut watcher =
            DefaultFileSystemWatcher::start(&builder.workflows_dir, debounce, handler)?;
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
mod cargo;
mod cmd;
mod docker;
mod renderer;
mod watcher;
