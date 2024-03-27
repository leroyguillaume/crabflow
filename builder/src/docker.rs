use std::{future::Future, path::Path, sync::Arc};

use mockable::Command;
use tracing::{debug, debug_span, instrument, Instrument, Level};

use crate::{cmd::CommandRunner, Result};

const CMD: &str = "docker";

pub trait DockerClient {
    fn build(&self, tag: &str, target: &str, cwd: &Path) -> impl Future<Output = Result>;

    fn info(&self) -> impl Future<Output = Result>;

    fn push(&self, tag: &str) -> impl Future<Output = Result>;
}

pub struct DefaultDockerClient<RUNNER: CommandRunner> {
    pub runner: Arc<RUNNER>,
    pub url: String,
}

impl<RUNNER: CommandRunner> DockerClient for DefaultDockerClient<RUNNER> {
    async fn build(&self, tag: &str, target: &str, cwd: &Path) -> Result {
        let span =
            debug_span!("build", cwd = %cwd.display(), docker.target = target, docker.tag = tag);
        async {
            debug!("running docker build");
            let cmd = Command::new(CMD)
                .with_cwd(cwd)
                .with_arg("-H")
                .with_arg(&self.url)
                .with_arg("build")
                .with_arg("-t")
                .with_arg(tag)
                .with_arg("--target")
                .with_arg(target)
                .with_arg(".");
            self.runner.run(&cmd).await?;
            Ok(())
        }
        .instrument(span)
        .await
    }

    #[instrument(level = Level::DEBUG, skip(self))]
    async fn info(&self) -> Result {
        let cmd = Command::new(CMD)
            .with_arg("-H")
            .with_arg(&self.url)
            .with_arg("info");
        self.runner.run(&cmd).await?;
        Ok(())
    }

    async fn push(&self, tag: &str) -> Result {
        let span = debug_span!("push", docker.tag = tag);
        async {
            debug!("running docker push");
            let cmd = Command::new(CMD)
                .with_arg("-H")
                .with_arg(&self.url)
                .with_arg("push")
                .with_arg(tag);
            self.runner.run(&cmd).await?;
            Ok(())
        }
        .instrument(span)
        .await
    }
}
