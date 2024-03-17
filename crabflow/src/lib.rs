use crabflow_common::db::{DatabaseClient, DatabasePool, DatabaseTransaction};

use crabflow_core::{WorkflowDesc, WorkflowState};
pub use crabflow_dsl::*;
use tracing::{info, info_span, Instrument};

pub type Result<T = ()> = anyhow::Result<T>;

pub async fn load_workflow<
    DB: DatabasePool<DBCONN, DBTX>,
    DBCONN: DatabaseClient,
    DBTX: DatabaseTransaction,
>(
    target: &str,
    desc: WorkflowDesc,
    db: &DB,
) -> Result {
    let span = info_span!("load_workflow", workflow.image.target = target);
    async {
        let id = desc.id.clone();
        let mut db = db.acquire().await?;
        let mut workflow = db
            .workflow_by_target(target)
            .await?
            .ok_or_else(|| anyhow::anyhow!("workflow with target `{target}` doesn't exist"))?;
        workflow.state = WorkflowState::Loaded(desc);
        db.update_workflow(&workflow).await?;
        info!(workflow.id = id, workflow.image.target, "workflow loaded");
        Ok(())
    }
    .instrument(span)
    .await
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

pub mod serde_json {
    pub use serde_json::*;
}

pub mod tokio {
    pub use tokio::*;
}

pub mod tracing {
    pub use tracing::*;
}
