use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
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
pub struct TaskExecution {
    pub id: String,
    pub started_at: DateTime<Utc>,
    pub status: TaskExecutionStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "db", derive(sqlx::Type))]
#[cfg_attr(
    feature = "db",
    sqlx(type_name = "task_execution_status", rename_all = "snake_case")
)]
pub enum TaskExecutionStatus {
    Failed,
    Pending,
    Running,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Workflow {
    pub created_at: DateTime<Utc>,
    pub image: Image,
    pub state: WorkflowState,
}

#[cfg(feature = "db")]
impl sqlx::FromRow<'_, sqlx::postgres::PgRow> for Workflow {
    fn from_row(row: &'_ sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;

        let state = match row.try_get("state")? {
            WorkflowStateSql::Created => WorkflowState::Created,
            WorkflowStateSql::Loaded => {
                let desc = row.try_get("descriptor")?;
                let desc = serde_json::from_value(desc)
                    .map_err(|err| sqlx::Error::Decode(Box::new(err)))?;
                WorkflowState::Loaded(desc)
            }
            WorkflowStateSql::Loading => WorkflowState::Loading,
        };
        Ok(Self {
            created_at: row.try_get("created_at")?,
            image: Image {
                tag: row.try_get("tag")?,
                target: row.try_get("target")?,
            },
            state,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkflowDesc {
    pub id: String,
    pub seq: SequenceDesc,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowLoading {
    pub id: String,
    pub target: String,
    pub started_at: DateTime<Utc>,
}

#[cfg(feature = "db")]
impl sqlx::FromRow<'_, sqlx::postgres::PgRow> for WorkflowLoading {
    fn from_row(row: &'_ sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;

        Ok(Self {
            id: row.try_get("id")?,
            target: row.try_get("target")?,
            started_at: row.try_get("started_at")?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowExecution {
    pub desc: WorkflowDesc,
    pub id: Uuid,
    pub kind: WorkflowExecutionKind,
    pub started_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "db", derive(sqlx::Type))]
#[cfg_attr(
    feature = "db",
    sqlx(type_name = "workflow_execution_kind", rename_all = "snake_case")
)]
pub enum WorkflowExecutionKind {
    Manual,
    Scheduled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkflowState {
    Created,
    Loaded(WorkflowDesc),
    Loading,
}

#[cfg(feature = "db")]
impl sqlx::Encode<'_, sqlx::Postgres> for WorkflowState {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Postgres as sqlx::database::HasArguments<'_>>::ArgumentBuffer,
    ) -> sqlx::encode::IsNull {
        let state = match self {
            Self::Created => WorkflowStateSql::Created,
            Self::Loaded(_) => WorkflowStateSql::Loaded,
            Self::Loading => WorkflowStateSql::Loading,
        };
        <WorkflowStateSql as sqlx::Encode<'_, sqlx::Postgres>>::encode_by_ref(&state, buf)
    }
}

#[cfg(feature = "db")]
impl sqlx::Type<sqlx::Postgres> for WorkflowState {
    fn compatible(ty: &<sqlx::Postgres as sqlx::Database>::TypeInfo) -> bool {
        WorkflowStateSql::compatible(ty)
    }

    fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
        WorkflowStateSql::type_info()
    }
}

#[cfg(feature = "db")]
#[derive(Clone, Copy, Debug, Eq, PartialEq, sqlx::Type)]
#[sqlx(type_name = "workflow_state", rename_all = "snake_case")]
enum WorkflowStateSql {
    Created,
    Loaded,
    Loading,
}
