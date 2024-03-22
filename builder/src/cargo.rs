use std::{future::Future, path::Path, sync::Arc};

use mockable::Command;
use serde::{Deserialize, Serialize};
use tracing::{debug, debug_span, Instrument};

use crate::{cmd::CommandRunner, Result};

const CMD: &str = "cargo";
const READ_MANIFEST_CMD: &str = "read-manifest";

pub trait CargoClient {
    fn load_targets(&self, cwd: &Path) -> impl Future<Output = Result<Vec<String>>>;
}

pub struct DefaultCargoClient<RUNNER: CommandRunner> {
    pub runner: Arc<RUNNER>,
}

impl<RUNNER: CommandRunner> CargoClient for DefaultCargoClient<RUNNER> {
    async fn load_targets(&self, cwd: &Path) -> Result<Vec<String>> {
        let span = debug_span!("load_targets", cwd = %cwd.display());
        async {
            debug!("running cargo read-manifest");
            let cmd = Command::new(CMD).with_cwd(cwd).with_arg(READ_MANIFEST_CMD);
            let output = self.runner.run(&cmd).await?;
            debug!("decoding cargo output as json");
            let manifest: Manifest = serde_json::from_slice(&output.stdout)?;
            let targets = manifest
                .targets
                .into_iter()
                .filter_map(|target| {
                    target
                        .kind
                        .iter()
                        .any(|kind| kind == "bin")
                        .then_some(target.name)
                })
                .collect();
            Ok(targets)
        }
        .instrument(span)
        .await
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Manifest {
    targets: Vec<ManifestTarget>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct ManifestTarget {
    kind: Vec<String>,
    name: String,
}
