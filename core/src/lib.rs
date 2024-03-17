use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "db", derive(sqlx::FromRow))]
pub struct Image {
    pub tag: String,
    pub target: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SequenceDesc {
    pub ids: BTreeSet<String>,
    pub next: Option<Box<SequenceDesc>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "db", derive(sqlx::FromRow))]
pub struct Workflow {
    pub created_at: DateTime<Utc>,
    #[cfg_attr(feature = "db", sqlx(flatten))]
    pub image: Image,
    pub state: WorkflowState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkflowDesc {
    pub id: String,
    pub seq: SequenceDesc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "db", derive(sqlx::Type))]
#[cfg_attr(
    feature = "db",
    sqlx(type_name = "workflow_state", rename_all = "snake_case")
)]
pub enum WorkflowState {
    Created,
    Loaded,
    Loading,
}
