use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SequenceDesc {
    pub ids: BTreeSet<String>,
    pub next: Option<Box<SequenceDesc>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Workflow {
    pub created_at: DateTime<Utc>,
    pub image: String,
    pub loaded: bool,
    pub target: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkflowDesc {
    pub id: String,
    pub seq: SequenceDesc,
}
