use std::future::Future;

use mockable::{Command, CommandOutput};
use tracing::{debug, debug_span, error, Instrument, Level};

use crate::{Error, Result};

macro_rules! log_output {
    ($cmd:expr, $output:expr, $lvl:ident) => {
        let output = String::from_utf8_lossy($output);
        for line in output.lines() {
            $lvl!("{}: {line}", $cmd);
        }
    };
}

pub trait CommandRunner {
    fn run(&self, cmd: &Command) -> impl Future<Output = Result<CommandOutput>>;
}

pub struct DefaultCommandRunner<RUNNER: mockable::CommandRunner> {
    pub runner: RUNNER,
}

impl<RUNNER: mockable::CommandRunner> CommandRunner for DefaultCommandRunner<RUNNER> {
    async fn run(&self, cmd: &Command) -> Result<CommandOutput> {
        let span = debug_span!("run", args = ?cmd.args, cmd = cmd.program);
        async {
            debug!("running command");
            let output = self.runner.run(cmd).await?;
            if tracing::enabled!(Level::DEBUG) {
                log_output!(&cmd.program, &output.stdout, debug);
            }
            if let Some(0) = output.code {
                if tracing::enabled!(Level::DEBUG) {
                    log_output!(&cmd.program, &output.stderr, debug);
                }
                Ok(output)
            } else {
                log_output!(&cmd.program, &output.stderr, error);
                Err(Error::Command(cmd.program.clone()))
            }
        }
        .instrument(span)
        .await
    }
}
