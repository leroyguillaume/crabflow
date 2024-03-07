use std::{
    path::PathBuf,
    process::{exit, ExitStatus},
    string::FromUtf8Error,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use builder::{DefaultImageBuilder, ImageBuilder};
use clap::Parser;
use crabflow_common::init_tracing;
use notify::{EventKind, RecursiveMode, Watcher};
use tracing::{debug, error, info};

type Result<T = ()> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("command {name} exited with {status}")]
    Command { name: String, status: ExitStatus },
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

fn main() {
    init_tracing();
    let args = Args::parse();
    let rc = if let Err(err) = run(args) {
        error!("{err}");
        1
    } else {
        0
    };
    exit(rc);
}

fn run(args: Args) -> Result {
    let builder = DefaultImageBuilder::init(args.opts)?;
    if args.watch {
        debug!("creating file system watcher");
        let mut watcher = notify::recommended_watcher({
            let path = args.path.clone();
            move |event: std::prelude::v1::Result<notify::Event, notify::Error>| {
                let res = event
                    .map_err(Error::from)
                    .and_then(|event| match event.kind {
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                            builder.build(&path)
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
        let over = Arc::new(AtomicBool::default());
        signal_hook::flag::register(signal_hook::consts::SIGINT, over.clone())?;
        signal_hook::flag::register(signal_hook::consts::SIGTERM, over.clone())?;
        info!("builder started");
        while !over.load(Ordering::Relaxed) {}
        info!("builder stopped");
        Ok(())
    } else {
        builder.build(&args.path)
    }
}

mod builder;
