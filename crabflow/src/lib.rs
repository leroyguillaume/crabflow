use crabflow_common::clap::DatabaseOptions;

pub use crabflow_dsl::*;

pub type Result<T = ()> = anyhow::Result<T>;

pub async fn load_workflow(_json: &str, _opts: DatabaseOptions) -> Result {
    Ok(())
}

pub mod anyhow {
    pub use anyhow::*;
}

pub mod clap {
    pub use clap::*;
}

pub mod common {
    pub use crabflow_common::*;
}

pub mod core {
    pub use crabflow_core::*;
}

pub mod tokio {
    pub use tokio::*;
}
