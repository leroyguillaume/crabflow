use std::{sync::Arc, time::Duration};

use pyo3::{
    exceptions::PyTypeError,
    pyclass, pymethods, pymodule,
    types::{PyFunction, PyList, PyModule},
    Bound, Py, PyErr, PyResult, Python,
};
use tokio::runtime::Runtime;

use crate::{
    tokio::{TokioExecutor, TokioTask, TokioTaskError},
    LoopOrchestrator, Task, TaskGroup, TaskGroupStatus, TaskStatus,
};

#[pymodule]
pub fn crabflow(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyLoopOrchestrator>()?;
    module.add_class::<PyTask>()?;
    module.add_class::<PyTaskGroup>()?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
enum PyTaskError {
    #[error("python error: {0}")]
    Python(
        #[from]
        #[source]
        PyErr,
    ),
    #[error("tokio error: {0}")]
    Tokio(
        #[from]
        #[source]
        TokioTaskError,
    ),
}

/// A Python wrapper for [`LoopOrchestator`].
#[pyclass(frozen)]
struct PyLoopOrchestrator(Arc<LoopOrchestrator>);

#[pymethods]
impl PyLoopOrchestrator {
    /// Returns a Python wrapper of loop-based orchestrator.
    ///
    /// *Arguments*
    /// - `delay`: The number of seconds between loop iterations.
    #[new]
    #[pyo3(signature = (delay = 0))]
    pub fn new(delay: u64) -> Self {
        Self(Arc::new(
            LoopOrchestrator::new().with_delay(Duration::from_secs(delay)),
        ))
    }

    /// Processes a task group until is finished.
    pub async fn process(&self, group: Py<PyTaskGroup>) -> PyResult<TaskGroupStatus> {
        let rt = Runtime::new()?;
        let join = rt.spawn({
            let orch = self.0.clone();
            async move {
                let status = orch.process(group.get(), &TokioExecutor).await?;
                Ok::<TaskGroupStatus, PyTaskError>(status)
            }
        });
        let status = join
            .await
            .map_err(|err| PyTypeError::new_err(err.to_string()))??;
        Ok(status)
    }
}

/// A Python wrapper for a [`TokioTask`].
#[pyclass(frozen)]
struct PyTask(TokioTask);

#[pymethods]
impl PyTask {
    /// Returns a Python wrapper for a Tokio task.
    ///
    /// *Arguments*
    /// - `closure`: the closure that will be called when the task started.
    #[new]
    pub fn new(closure: Py<PyFunction>) -> Self {
        Self(TokioTask::new(move || {
            let closure = closure.clone();
            async move {
                Python::with_gil(|py| {
                    closure.call0(py)?;
                    Ok::<(), PyTaskError>(())
                })
            }
        }))
    }
}

impl Task<PyTaskError, TokioExecutor> for PyTask {
    async fn delete(&self, exec: &TokioExecutor) -> Result<(), PyTaskError> {
        self.0.delete(exec).await?;
        Ok(())
    }

    async fn start(&self, exec: &TokioExecutor) -> Result<TaskStatus, PyTaskError> {
        let status = self.0.start(exec).await?;
        Ok(status)
    }

    async fn status(&self, exec: &TokioExecutor) -> Result<TaskStatus, PyTaskError> {
        let status = self.0.status(exec).await?;
        Ok(status)
    }
}

/// A task group.
#[pyclass(frozen)]
struct PyTaskGroup {
    next: Option<Py<Self>>,
    tasks: Vec<Py<PyTask>>,
}

#[pymethods]
impl PyTaskGroup {
    /// Returns a task group.
    ///
    /// *Arguments*
    /// - `tasks`: a list of [`PyTask`].
    /// - `next`: the next group to call after this one is succeeded.
    #[new]
    #[pyo3(signature = (tasks, next = None))]
    pub fn new(tasks: Py<PyList>, next: Option<Py<Self>>) -> PyResult<Self> {
        Python::with_gil(|py| {
            let tasks: Vec<Py<PyTask>> = tasks.extract(py)?;
            Ok(Self { next, tasks })
        })
    }
}

impl TaskGroup<PyTaskError, TokioExecutor, PyTask> for PyTaskGroup {
    fn next(&self) -> Option<&Self> {
        self.next.as_ref().map(|group| group.get())
    }

    async fn tasks(&self) -> Result<impl Iterator<Item = &PyTask>, PyTaskError> {
        Ok(self.tasks.iter().map(|task| task.get()))
    }
}

impl From<PyTaskError> for PyErr {
    fn from(err: PyTaskError) -> Self {
        match err {
            PyTaskError::Python(err) => err,
            PyTaskError::Tokio(err) => PyTypeError::new_err(err.to_string()),
        }
    }
}
