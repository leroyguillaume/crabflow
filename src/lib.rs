use std::{future::Future, marker::PhantomData, pin::Pin, time::Duration};

use ::tokio::time::sleep;
use chrono::{DateTime, Utc};

/// A task.
#[cfg_attr(test, mockall::automock)]
pub trait Task<ERR: Send + Sync, EXEC: Send + Sync>: Send + Sync {
    /// Returns the status of the task.
    fn status(&self, exec: &EXEC) -> impl Future<Output = Result<TaskStatus, ERR>> + Send;

    /// Deletes the task.
    fn delete(&self, exec: &EXEC) -> impl Future<Output = Result<(), ERR>> + Send;

    /// Starts the task.
    fn start(&self, exec: &EXEC) -> impl Future<Output = Result<TaskStatus, ERR>> + Send;
}

/// A task group.
pub trait TaskGroup<ERR: Send + Sync, EXEC: Send + Sync, TASK: Task<ERR, EXEC> + 'static>:
    Send + Sync
{
    /// Returns the next group.
    fn next(&self) -> Option<&Self>;

    /// Returns an iterator of the tasks of this group.
    fn tasks(&self)
        -> impl Future<Output = Result<impl Iterator<Item = &TASK> + Send, ERR>> + Send;
}

/// Metadata about a finished task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinishedTaskData {
    /// True if the task was deleted, false otherwise.
    pub deleted: bool,
    /// The datetime the task finished.
    pub finished_at: DateTime<Utc>,
    /// The datetime the task started.
    pub started_at: DateTime<Utc>,
}

/// Status about a task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskStatus {
    /// The task failed.
    Failed(FinishedTaskData),
    /// The task is pending to start.
    Pending,
    /// The task is running.
    Running {
        /// The datetime the task started.
        started_at: DateTime<Utc>,
    },
    /// The task succeeded.
    Succeeded(FinishedTaskData),
}

/// Status about a task group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "py", pyo3::pyclass)]
pub enum TaskGroupStatus {
    /// At least one task of the group failed.
    Failed,
    /// At least one task of the group is running.
    Running,
    /// All the tasks of the group succeeded.
    Succeeded,
}

/// A loop-based orchestrator.
///
/// The orchestrator loops until the task group is finished.
///
/// If at least one task of the group failed, [`TaskGroupStatus::Failed`] is returned once all tasks are finished.
///
/// If all tasks of the group succeeded, all tasks of the next one are started. If it's [`None`], [`TaskGroupStatus::Succeeded`] is returned.
///
/// If the task group is running, the orchestrator iterates over all tasks to get their status.
pub struct LoopOrchestrator {
    delay: Duration,
}

impl LoopOrchestrator {
    /// Returns a loop-base orchestrator without delay between iterations.
    pub fn new() -> Self {
        Self {
            delay: Duration::from_secs(0),
        }
    }

    /// Sets a delay between two loop iterations.
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Processes a task group until is finished.
    pub async fn process<
        'a,
        ERR: Send + Sync,
        EXEC: Send + Sync,
        GROUP: TaskGroup<ERR, EXEC, TASK>,
        TASK: Task<ERR, EXEC> + 'static,
    >(
        &self,
        mut group: &'a GROUP,
        exec: &EXEC,
    ) -> Result<TaskGroupStatus, ERR> {
        loop {
            let (status, next) = Self::do_process(group, exec).await?;
            if let Some(next) = next {
                group = next;
                sleep(self.delay).await;
            } else {
                break Ok(status);
            }
        }
    }

    #[allow(clippy::type_complexity)]
    fn do_process<
        'a,
        ERR: Send + Sync,
        EXEC: Send + Sync,
        GROUP: TaskGroup<ERR, EXEC, TASK>,
        TASK: Task<ERR, EXEC> + 'static,
    >(
        group: &'a GROUP,
        exec: &'a EXEC,
    ) -> Pin<Box<dyn Future<Output = Result<(TaskGroupStatus, Option<&'a GROUP>), ERR>> + Send + 'a>>
    {
        Box::pin(async move {
            let mut count = 0;
            let mut finished = 0;
            let mut succeeded = 0;
            let tasks = group.tasks().await?;
            for task in tasks {
                count += 1;
                let status = task.status(exec).await?;
                Self::handle_task_status(&status, task, &mut finished, &mut succeeded, exec)
                    .await?;
            }
            if count == finished {
                if succeeded == count {
                    if let Some(next) = group.next() {
                        Self::do_process(next, exec).await
                    } else {
                        Ok((TaskGroupStatus::Succeeded, None))
                    }
                } else {
                    Ok((TaskGroupStatus::Failed, None))
                }
            } else {
                Ok((TaskGroupStatus::Running, Some(group)))
            }
        })
    }

    fn handle_task_status<'a, ERR: Send + Sync, EXEC: Send + Sync, TASK: Task<ERR, EXEC>>(
        status: &'a TaskStatus,
        task: &'a TASK,
        finished: &'a mut usize,
        succeeded: &'a mut usize,
        exec: &'a EXEC,
    ) -> Pin<Box<dyn Future<Output = Result<(), ERR>> + Send + 'a>> {
        Box::pin(async move {
            match status {
                TaskStatus::Failed(data) => {
                    if !data.deleted {
                        task.delete(exec).await?;
                    }
                    *finished += 1;
                }
                TaskStatus::Pending => {
                    let status = task.start(exec).await?;
                    Self::handle_task_status(&status, task, finished, succeeded, exec).await?;
                }
                TaskStatus::Succeeded(data) => {
                    if !data.deleted {
                        task.delete(exec).await?;
                    }
                    *finished += 1;
                    *succeeded += 1;
                }
                _ => {}
            }
            Ok(())
        })
    }
}

impl Default for LoopOrchestrator {
    /// See [`LoopOrchestrator::new`].
    fn default() -> Self {
        Self::new()
    }
}

/// Vec-based task group.
pub struct VecTaskGroup<ERR: Send + Sync, EXEC: Send + Sync, TASK: Task<ERR, EXEC>> {
    next: Option<Box<Self>>,
    tasks: Vec<TASK>,
    _err: PhantomData<ERR>,
    _exec: PhantomData<EXEC>,
}

impl<ERR: Send + Sync, EXEC: Send + Sync, TASK: Task<ERR, EXEC>> VecTaskGroup<ERR, EXEC, TASK> {
    /// Returns a vec-based task group.
    pub fn new<TASKS: IntoIterator<Item = TASK>>(tasks: TASKS) -> Self {
        Self {
            next: None,
            tasks: tasks.into_iter().collect(),
            _err: PhantomData,
            _exec: PhantomData,
        }
    }

    /// Sets a group to start after this one is succeeded.
    pub fn then(mut self, group: Self) -> Self {
        self.next = Some(Box::new(group));
        self
    }
}

impl<ERR: Send + Sync, EXEC: Send + Sync, TASK: Task<ERR, EXEC> + 'static>
    TaskGroup<ERR, EXEC, TASK> for VecTaskGroup<ERR, EXEC, TASK>
{
    fn next(&self) -> Option<&Self> {
        self.next.as_ref().map(|group| group.as_ref())
    }

    async fn tasks(&self) -> Result<impl Iterator<Item = &TASK>, ERR> {
        Ok(self.tasks.iter())
    }
}

#[cfg(feature = "py")]
pub mod py;
#[cfg(feature = "tokio")]
pub mod tokio;

#[cfg(test)]
mod test {
    use chrono::Utc;

    use super::{
        FinishedTaskData, LoopOrchestrator, MockTask, TaskGroupStatus, TaskStatus, VecTaskGroup,
    };

    #[derive(Debug)]
    struct MockErr;

    struct MockExecutor;

    mod loop_orchestrator {
        use super::*;

        mod do_process {
            use super::*;

            #[tokio::test]
            async fn start_a() {
                let a_status_init = TaskStatus::Pending;
                let a_status_final = TaskStatus::Running {
                    started_at: Utc::now(),
                };
                let b_status_init = TaskStatus::Running {
                    started_at: Utc::now(),
                };
                let c_status_init = TaskStatus::Pending;
                let (mut a, b, c) = mock_tasks(a_status_init, b_status_init, c_status_init);
                a.expect_start()
                    .returning(move |_| Box::pin(async move { Ok(a_status_final) }));
                let group = VecTaskGroup::new([a, b]).then(VecTaskGroup::new([c]));
                let (status, group) = LoopOrchestrator::do_process(&group, &MockExecutor)
                    .await
                    .unwrap();
                assert_eq!(status, TaskGroupStatus::Running);
                let group = group.unwrap();
                assert_eq!(group.tasks.len(), 2);
            }

            #[tokio::test]
            async fn start_c() {
                let a_status_init = TaskStatus::Pending;
                let a_status_final = TaskStatus::Succeeded(FinishedTaskData {
                    deleted: false,
                    finished_at: Utc::now(),
                    started_at: Utc::now(),
                });
                let b_status_init = TaskStatus::Succeeded(FinishedTaskData {
                    deleted: true,
                    finished_at: Utc::now(),
                    started_at: Utc::now(),
                });
                let c_status_init = TaskStatus::Pending;
                let c_status_final = TaskStatus::Running {
                    started_at: Utc::now(),
                };
                let (mut a, b, mut c) = mock_tasks(a_status_init, b_status_init, c_status_init);
                a.expect_start()
                    .returning(move |_| Box::pin(async move { Ok(a_status_final) }));
                a.expect_delete().returning(|_| Box::pin(async { Ok(()) }));
                c.expect_start()
                    .returning(move |_| Box::pin(async move { Ok(c_status_final) }));
                let group = VecTaskGroup::new([a, b]).then(VecTaskGroup::new([c]));
                let (status, group) = LoopOrchestrator::do_process(&group, &MockExecutor)
                    .await
                    .unwrap();
                assert_eq!(status, TaskGroupStatus::Running);
                let group = group.unwrap();
                assert_eq!(group.tasks.len(), 1);
            }

            #[tokio::test]
            async fn failed() {
                let a_status_init = TaskStatus::Succeeded(FinishedTaskData {
                    deleted: true,
                    finished_at: Utc::now(),
                    started_at: Utc::now(),
                });
                let b_status_init = TaskStatus::Failed(FinishedTaskData {
                    deleted: false,
                    finished_at: Utc::now(),
                    started_at: Utc::now(),
                });
                let c_status_init = TaskStatus::Pending;
                let (a, mut b, c) = mock_tasks(a_status_init, b_status_init, c_status_init);
                b.expect_delete().returning(|_| Box::pin(async { Ok(()) }));
                let group = VecTaskGroup::new([a, b]).then(VecTaskGroup::new([c]));
                let (status, group) = LoopOrchestrator::do_process(&group, &MockExecutor)
                    .await
                    .unwrap();
                assert_eq!(status, TaskGroupStatus::Failed);
                assert!(group.is_none());
            }

            #[tokio::test]
            async fn succeeded() {
                let a_status_init = TaskStatus::Succeeded(FinishedTaskData {
                    deleted: true,
                    finished_at: Utc::now(),
                    started_at: Utc::now(),
                });
                let b_status_init = TaskStatus::Succeeded(FinishedTaskData {
                    deleted: true,
                    finished_at: Utc::now(),
                    started_at: Utc::now(),
                });
                let c_status_init = TaskStatus::Succeeded(FinishedTaskData {
                    deleted: true,
                    finished_at: Utc::now(),
                    started_at: Utc::now(),
                });
                let (a, b, c) = mock_tasks(a_status_init, b_status_init, c_status_init);
                let group = VecTaskGroup::new([a, b]).then(VecTaskGroup::new([c]));
                let (status, group) = LoopOrchestrator::do_process(&group, &MockExecutor)
                    .await
                    .unwrap();
                assert_eq!(status, TaskGroupStatus::Succeeded);
                assert!(group.is_none());
            }

            fn mock_tasks(
                a_status: TaskStatus,
                b_status: TaskStatus,
                c_status: TaskStatus,
            ) -> (
                MockTask<MockErr, MockExecutor>,
                MockTask<MockErr, MockExecutor>,
                MockTask<MockErr, MockExecutor>,
            ) {
                let mut a = MockTask::new();
                a.expect_status()
                    .returning(move |_| Box::pin(async move { Ok(a_status) }));
                let mut b = MockTask::new();
                b.expect_status()
                    .returning(move |_| Box::pin(async move { Ok(b_status) }));
                let mut c = MockTask::new();
                c.expect_status()
                    .returning(move |_| Box::pin(async move { Ok(c_status) }));
                (a, b, c)
            }
        }
    }
}
