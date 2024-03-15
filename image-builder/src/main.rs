use std::{
    path::PathBuf,
    process::{exit, ExitStatus},
    string::FromUtf8Error,
};

use builder::{DefaultImageBuilder, ImageBuilder};
use clap::Parser;
use crabflow_common::{clap::DatabaseOptions, init_tracing};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use tokio::{
    select,
    signal::unix::{signal, SignalKind},
    sync::mpsc::{channel, error::SendError},
};
use tracing::{debug, error, info};

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
    #[command(flatten)]
    opts: Options,
    #[arg(
        env = "WORKFLOWS_DIR",
        help = "Path to directory that contains workflows"
    )]
    path: PathBuf,
    #[arg(short, long, help = "Watch changes on workflows directory")]
    watch: bool,
}

#[derive(clap::Args, Clone, Debug, Eq, PartialEq)]
struct Options {
    #[command(flatten)]
    db: DatabaseOptions,
    #[arg(
        long,
        env = "DOCKER_URL",
        help = "URL to Docker daemon",
        default_value = "unix:///var/run/docker.sock"
    )]
    docker_url: String,
    #[arg(short, long, env = "REGISTRY", help = "Image registry")]
    registry: Option<String>,
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
    let builder = DefaultImageBuilder::init(args.opts).await?;
    if args.watch {
        let (tx, mut rx) = channel(1);
        debug!("creating file system watcher");
        let mut watcher = notify::recommended_watcher({
            move |event: std::result::Result<Event, notify::Error>| {
                let res = event
                    .map_err(Error::from)
                    .and_then(|event| match event.kind {
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                            tx.blocking_send(())?;
                            Ok(())
                        }
                        _ => Ok(()),
                    });
                if let Err(err) = res {
                    error!("{err}");
                }
            }
        })?;
        debug!(path = %args.path.display(), "starting file system watcher");
        watcher.watch(&args.path, RecursiveMode::Recursive)?;
        let mut sigint = signal(SignalKind::interrupt())?;
        let mut sigterm = signal(SignalKind::terminate())?;
        info!("builder started");
        loop {
            select! {
                _ = rx.recv() => {
                    if let Err(err) = builder.build(&args.path).await {
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
        Ok(())
    } else {
        builder.build(&args.path).await
    }
}

mod builder;
