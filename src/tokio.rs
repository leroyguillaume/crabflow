use std::{future::Future, pin::Pin, sync::Arc};

use chrono::{DateTime, Utc};
use tokio::{
    spawn,
    sync::Mutex,
    task::{JoinError, JoinHandle},
};

use crate::{FinishedTaskData, Task, TaskStatus};

/// A Tokio executor.
pub struct TokioExecutor;

/// An error that occurs when Tokio task failed.
#[derive(Debug, thiserror::Error)]
pub enum TokioTaskError {
    /// Occurs when a task is started twice.
    #[error("task is already started")]
    AlreadyStarted,
    /// Occurs when waiting for job failed.
    #[error("failed to wait for task: {0}")]
    Join(
        #[from]
        #[source]
        JoinError,
    ),
}

/// An implementation of task with Tokio.
#[allow(clippy::type_complexity)]
pub struct TokioTask {
    closure: Box<
        dyn Fn(DateTime<Utc>) -> Pin<Box<dyn Future<Output = TaskStatus> + Send>> + Send + Sync,
    >,
    join: Mutex<Option<(JoinHandle<TaskStatus>, DateTime<Utc>)>>,
}

impl TokioTask {
    /// Returns a Tokio task.
    ///
    /// *Arguments*
    /// - `closure`: a closure that will be called when the task is started.
    pub fn new<
        ERR,
        FN: Fn() -> FUT + Send + Sync + 'static,
        FUT: Future<Output = Result<(), ERR>> + Send + Sync,
    >(
        closure: FN,
    ) -> Self {
        let closure = Arc::new(closure);
        Self {
            closure: Box::new(move |started_at| {
                let closure = closure.clone();
                Box::pin(async move {
                    let res = closure().await;
                    let data = FinishedTaskData {
                        deleted: true,
                        finished_at: Utc::now(),
                        started_at,
                    };
                    match res {
                        Ok(_) => TaskStatus::Succeeded(data),
                        Err(_) => TaskStatus::Failed(data),
                    }
                })
            }),
            join: Mutex::new(None),
        }
    }
}

impl Task<TokioTaskError, TokioExecutor> for TokioTask {
    async fn delete(&self, _exec: &TokioExecutor) -> Result<(), TokioTaskError> {
        Ok(())
    }

    async fn start(&self, _exec: &TokioExecutor) -> Result<TaskStatus, TokioTaskError> {
        let mut join = self.join.lock().await;
        if join.is_some() {
            return Err(TokioTaskError::AlreadyStarted);
        }
        let started_at = Utc::now();
        *join = Some((spawn((self.closure)(started_at)), started_at));
        Ok(TaskStatus::Running { started_at })
    }

    async fn status(&self, _exec: &TokioExecutor) -> Result<TaskStatus, TokioTaskError> {
        let mut guard = self.join.lock().await;
        if let Some((join, started_at)) = guard.take() {
            if join.is_finished() {
                let status = join.await?;
                Ok(status)
            } else {
                *guard = Some((join, started_at));
                Ok(TaskStatus::Running { started_at })
            }
        } else {
            Ok(TaskStatus::Pending)
        }
    }
}
