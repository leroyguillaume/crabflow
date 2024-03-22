use std::{error::Error, io::stderr, process::exit};

use ::clap::Parser;
use futures::Future;
use tokio::{
    select,
    signal::unix::{signal, Signal, SignalKind},
};
use tracing::{debug, error, subscriber::set_global_default, Level};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

pub struct SignalListener {
    int: Signal,
    term: Signal,
}

impl SignalListener {
    pub fn init() -> std::io::Result<Self> {
        debug!("creating unix signal listeners");
        Ok(Self {
            int: signal(SignalKind::interrupt())?,
            term: signal(SignalKind::terminate())?,
        })
    }

    pub async fn recv(&mut self) {
        select! {
            _ = self.int.recv() => {
                debug!("SIGINT received");
            }
            _ = self.term.recv() => {
                debug!("SIGTERM received");
            }
        }
    }
}

pub async fn main<E: Error, F: Future<Output = Result<(), E>>, P: Parser>(run: impl Fn(P) -> F) {
    init_tracing();
    let args = P::parse();
    let rc = if let Err(err) = run(args).await {
        error!("{err}");
        1
    } else {
        0
    };
    exit(rc);
}

pub fn init_tracing() {
    let env_filter = EnvFilter::builder()
        .with_env_var("LOG_FILTER")
        .with_default_directive(Level::INFO.into())
        .from_env_lossy();
    let layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_writer(stderr);
    let registry = Registry::default().with(env_filter).with(layer);
    if let Err(err) = set_global_default(registry) {
        eprintln!("failed to initialize logging: {err}");
    }
}

pub mod clap;
#[cfg(feature = "db")]
pub mod db;
