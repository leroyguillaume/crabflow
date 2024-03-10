use std::{
    fs::File,
    path::Path,
    process::{Command, Output},
};

use liquid::{object, Parser, ParserBuilder};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, instrument, Level};

use crate::{Error, Options, Result};

const CMD_CARGO: &str = "cargo";
const CMD_DOCKER: &str = "docker";

pub trait CargoClient {
    fn load_targets(&self, dir_path: &Path) -> Result<Vec<String>>;
}

pub trait DockerClient {
    fn build(&self, target: &str, tag: &str, path: &Path) -> Result;

    fn push(&self, tag: &str) -> Result;
}

pub trait ImageBuilder {
    fn build(&self, dir_path: &Path) -> Result;
}

pub trait LiquidRenderer {
    fn render(&self, template: &str, path: &Path, targets: &[String]) -> Result;
}

pub struct DefaultCargoClient;

impl CargoClient for DefaultCargoClient {
    #[instrument(level = Level::DEBUG, skip(self))]
    fn load_targets(&self, dir_path: &Path) -> Result<Vec<String>> {
        debug!("running cargo");
        let output = Command::new(CMD_CARGO)
            .current_dir(dir_path)
            .arg("read-manifest")
            .output()?;
        let stdout = command_stdout(CMD_CARGO, output)?;
        debug!("decoding cargo output as json");
        let manifest: Manifest = serde_json::from_str(&stdout)?;
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
}

pub struct DefaultImageBuilder<CARGO: CargoClient, DOCKER: DockerClient, LIQUID: LiquidRenderer> {
    cargo: CARGO,
    docker: DOCKER,
    liquid: LIQUID,
    registry: Option<String>,
}

impl DefaultImageBuilder<DefaultCargoClient, DefaultDockerClient, DefaultLiquidRenderer> {
    pub fn init(opts: Options) -> Result<Self> {
        debug!("creating Liquid parser");
        let parser = ParserBuilder::with_stdlib().build()?;
        Ok(Self {
            cargo: DefaultCargoClient,
            docker: DefaultDockerClient {
                url: opts.docker_url,
            },
            liquid: DefaultLiquidRenderer { parser },
            registry: opts.registry,
        })
    }
}

impl<CARGO: CargoClient, DOCKER: DockerClient, LIQUID: LiquidRenderer> ImageBuilder
    for DefaultImageBuilder<CARGO, DOCKER, LIQUID>
{
    #[instrument(skip(self))]
    fn build(&self, dir_path: &Path) -> Result {
        let targets = self.cargo.load_targets(dir_path)?;
        let dockerfile_template = include_str!("../resources/main/Dockerfile.liquid");
        let dockerfile_path = dir_path.join("Dockerfile");
        self.liquid
            .render(dockerfile_template, &dockerfile_path, &targets)?;
        for target in targets {
            let tag = if let Some(registry) = &self.registry {
                format!("{registry}/{target}")
            } else {
                target.clone()
            };
            info!(tag, "building image");
            self.docker.build(&target, &tag, dir_path)?;
            self.docker.push(&tag)?;
            info!(tag, "image successfully built");
        }
        Ok(())
    }
}

pub struct DefaultDockerClient {
    url: String,
}

impl DockerClient for DefaultDockerClient {
    #[instrument(level = Level::DEBUG, skip(self))]
    fn build(&self, target: &str, tag: &str, dir_path: &Path) -> Result {
        debug!("running docker build");
        let output = Command::new(CMD_DOCKER)
            .current_dir(dir_path)
            .arg("-H")
            .arg(&self.url)
            .arg("build")
            .arg("-t")
            .arg(tag)
            .arg("--target")
            .arg(target)
            .arg(".")
            .output()?;
        command_stdout(CMD_DOCKER, output)?;
        Ok(())
    }

    #[instrument(level = Level::DEBUG, skip(self))]
    fn push(&self, tag: &str) -> Result {
        debug!("running docker push");
        let output = Command::new(CMD_DOCKER)
            .arg("-H")
            .arg(&self.url)
            .arg("push")
            .arg(tag)
            .output()?;
        command_stdout(CMD_DOCKER, output)?;
        Ok(())
    }
}

pub struct DefaultLiquidRenderer {
    parser: Parser,
}

impl LiquidRenderer for DefaultLiquidRenderer {
    #[instrument(level = Level::DEBUG, skip(self, template, targets))]
    fn render(&self, template: &str, path: &Path, targets: &[String]) -> Result {
        debug!("parsing template");
        let template = self.parser.parse(template)?;
        let obj = object!({
            "targets": targets,
        });
        debug!("rendering template");
        let mut file = File::create(path)?;
        template.render_to(&mut file, &obj)?;
        Ok(())
    }
}

fn command_stdout(command: &str, output: Output) -> Result<String> {
    debug!("decoding command output as utf-8");
    let stdout = String::from_utf8(output.stdout)?;
    if tracing::enabled!(Level::DEBUG) {
        for line in stdout.lines() {
            debug!("{command}: {line}");
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            debug!("{command}: {line}");
        }
    }
    if output.status.success() {
        Ok(stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines() {
            error!("{command}: {line}");
        }
        Err(Error::Command {
            name: command.into(),
            status: output.status,
        })
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
