use std::{
    collections::BTreeSet,
    future::Future,
    path::{Path, PathBuf},
    time::Duration,
};

use notify::{event::ModifyKind, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{DebounceEventResult, DebouncedEvent, Debouncer, FileIdMap};
use tokio::sync::mpsc::{channel, Receiver};
use tracing::{debug, debug_span, error, instrument, Level};

use crate::{Error, Result};

pub trait FileSystemEventHandler: Send + Sync {
    fn is_change(&self, event: &DebouncedEvent, dir: &Path) -> bool;
}

pub trait FileSystemWatcher {
    fn recv(&mut self) -> impl Future<Output = Option<()>>;
}

pub struct DefaultFileSystemEventHandler {
    pub ignoring: BTreeSet<PathBuf>,
}

impl FileSystemEventHandler for DefaultFileSystemEventHandler {
    #[instrument(level = Level::DEBUG, skip(self, event))]
    fn is_change(&self, event: &DebouncedEvent, dir: &Path) -> bool {
        let changed = match event.kind {
            EventKind::Create(_) | EventKind::Remove(_) => true,
            EventKind::Modify(kind) => matches!(
                kind,
                ModifyKind::Data(_) | ModifyKind::Name(_) | ModifyKind::Other
            ),
            _ => false,
        };
        if changed {
            let count = event.paths.iter()
                .filter_map(|path| {
                    debug!(
                        dir = %dir.display(),
                        path = %path.display(),
                        "computing relative filepath"
                    );
                    match path.strip_prefix(dir) {
                        Ok(path) => {
                            if self.ignoring.contains(path) {
                                debug!(path = %path.display(), "file is ignored");
                                None
                            } else {
                                Some(path)
                            }
                        },
                        Err(err) => {
                            error!(path = %path.display(), "failed to compute relative filepath: {err}");
                            debug!(path = %path.display(), "file is ignored because of previous error");
                            None
                        }
                    }
                })
                .count();
            count > 0
        } else {
            false
        }
    }
}

pub struct DefaultFileSystemWatcher {
    _debouncer: Debouncer<RecommendedWatcher, FileIdMap>,
    rx: Receiver<()>,
}

impl DefaultFileSystemWatcher {
    #[instrument(skip(path, debounce, handler))]
    pub fn start<H: FileSystemEventHandler + 'static>(
        path: &Path,
        debounce: Duration,
        handler: H,
    ) -> Result<Self> {
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
                        let changed = events.iter().any(|event| handler.is_change(event, &path));
                        if changed {
                            debug!("sending notification");
                            tx.blocking_send(())?;
                        }
                        Ok(())
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
