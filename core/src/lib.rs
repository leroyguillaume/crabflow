use std::{cmp::Ordering, collections::BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TaskDesc {
    pub id: String,
    pub next: BTreeSet<TaskDesc>,
}

impl PartialOrd for TaskDesc {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TaskDesc {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id.cmp(&other.id)
    }
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
    pub tasks: BTreeSet<TaskDesc>,
}
