use std::{path::PathBuf, process::exit, time::Duration};

use clap::Parser;
use crabflow_common::{init_tracing, SignalListener};
use tokio::{select, time::sleep};
use tracing::{error, info};

use crate::git::{DefaultSynchronizer, Synchronizer};

type Result<T = ()> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("empty repository")]
    EmptyRepository,
    #[error("git error: {0}")]
    Git(
        #[from]
        #[source]
        git2::Error,
    ),
    #[error("i/o error: {0}")]
    Io(
        #[from]
        #[source]
        std::io::Error,
    ),
    #[error("failed to decode utf-8")]
    Utf8,
}

#[derive(Clone, Debug, Eq, Parser, PartialEq)]
struct Args {
    #[arg(
        short,
        long,
        env = "DELAY",
        help = "Number of seconds between each git pull",
        default_value_t = 10
    )]
    delay: u64,
    #[arg(
        short,
        long,
        env = "WORKFLOWS_DIR",
        help = "Path to directory into clone repository",
        default_value = "."
    )]
    path: PathBuf,
    #[arg(env = "REPOSITORY", help = "Repository URL")]
    repository: String,
    #[command(flatten)]
    rev: RevisionArg,
}

#[derive(clap::Args, Clone, Debug, Eq, PartialEq)]
#[group(multiple = false)]
struct RevisionArg {
    #[arg(short, long, env = "BRANCH", help = "Branch name")]
    branch: Option<String>,
    #[arg(short, long, env = "TAG", help = "Tag name")]
    tag: Option<String>,
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
    let delay = Duration::from_secs(args.delay);
    let rev = args.rev.into();
    let git = DefaultSynchronizer::init(&args.path, &args.repository, &rev)?;
    info!("synchronizer started");
    let mut sig = SignalListener::init()?;
    loop {
        if let Err(err) = git.synchronize() {
            error!("{err}");
        }
        select! {
            _ = sleep(delay) => {}
            _ = sig.recv() => {
                break;
            }
        }
    }
    info!("synchronizer stopped");
    Ok(())
}

mod git;
