use std::{
    collections::BTreeSet,
    future::Future,
    path::{Path, PathBuf},
    time::Duration,
};

use notify::{event::ModifyKind, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{DebounceEventResult, Debouncer, FileIdMap};
use tokio::sync::mpsc::{channel, Receiver};
use tracing::{debug, debug_span, error, instrument};

use crate::{Error, Result};

pub trait FileSystemWatcher {
    fn recv(&mut self) -> impl Future<Output = Option<()>>;
}

pub struct DefaultFileSystemWatcher {
    _debouncer: Debouncer<RecommendedWatcher, FileIdMap>,
    rx: Receiver<()>,
}

impl DefaultFileSystemWatcher {
    #[instrument(skip(path, debounce, ignoring))]
    pub fn start(path: &Path, debounce: Duration, ignoring: BTreeSet<PathBuf>) -> Result<Self> {
        debug!(path = %path.display(), "canonicalizing path");
        let path = path.canonicalize()?;
        let (tx, rx) = channel(1);
        debug!("creating file system watcher");
        let mut debouncer = notify_debouncer_full::new_debouncer(debounce, None, {
            let path = path.clone();
            move |event: DebounceEventResult| {
                let span = debug_span!("watch");
                let _enter = span.enter();
                let res = event
                    .map_err(|mut errs| Error::Notify(errs.remove(0)))
                    .and_then(|events| {
                        let changed = events.into_iter().any(|event| {
                            debug!("file system event received");
                            let count = event
                                .paths
                                .iter()
                                .filter_map(|event_path| {
                                    debug!(
                                        event_path = %event_path.display(),
                                        path = %path.display(),
                                        "computing relative filepath"
                                    );
                                    match event_path.strip_prefix(&path) {
                                        Ok(path) => Some(path),
                                        Err(err) => {
                                            error!("failed to compute relative filepath: {err}");
                                            None
                                        }
                                    }
                                })
                                .filter(|path| !ignoring.contains(*path))
                                .count();
                            let changed = match event.kind {
                                EventKind::Create(_) | EventKind::Remove(_) => true,
                                EventKind::Modify(kind) => matches!(
                                    kind,
                                    ModifyKind::Data(_) | ModifyKind::Name(_) | ModifyKind::Other
                                ),
                                _ => false,
                            };
                            count > 0 && changed
                        });
                        if changed {
                            tx.blocking_send(())?;
                            Ok(())
                        } else {
                            Ok(())
                        }
                    });
                if let Err(err) = res {
                    error!("{err}");
                }
            }
        })?;
        debug!(path = %path.display(), "starting file system watcher");
        debouncer.watcher().watch(&path, RecursiveMode::Recursive)?;
        Ok(Self {
            _debouncer: debouncer,
            rx,
        })
    }
}

impl FileSystemWatcher for DefaultFileSystemWatcher {
    async fn recv(&mut self) -> Option<()> {
        self.rx.recv().await
    }
}
