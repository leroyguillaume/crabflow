use std::io::stderr;

use tracing::{subscriber::set_global_default, Level};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

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
