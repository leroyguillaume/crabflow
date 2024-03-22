use std::io::stderr;

use tokio::{
    select,
    signal::unix::{signal, Signal, SignalKind},
};
use tracing::{debug, subscriber::set_global_default, Level};
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

#[cfg(feature = "clap")]
pub mod clap;
#[cfg(feature = "db")]
pub mod db;
