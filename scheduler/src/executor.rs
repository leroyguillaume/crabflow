use futures::Future;
use k8s_openapi::api::core::v1::Pod;
use kube::{api::PostParams, Api, Client};
use mockable::UuidGenerator;
use tracing::{debug, debug_span, Instrument};

use crate::{Error, Result};

pub trait Executor: Send + Sync {
    fn start(
        &self,
        target: &str,
        image: &str,
        args: &[String],
    ) -> impl Future<Output = Result<String>>;
}

pub struct DefaultExecutor<UUID: UuidGenerator> {
    pub k8s: Client,
    pub pod_template: Pod,
    pub uuid_gen: UUID,
}

impl<UUID: UuidGenerator> Executor for DefaultExecutor<UUID> {
    async fn start(&self, name: &str, image: &str, args: &[String]) -> Result<String> {
        let uuid = self.uuid_gen.generate_v4();
        let (id, _, _, _) = uuid.as_fields();
        let name = format!("{name}-{id}");
        let span = debug_span!("execute", executor.args = ?args, executor.image = image, executor.name = name);
        async {
            let mut pod = self.pod_template.clone();
            let spec = pod.spec.as_mut().ok_or(Error::MissingPodSpec)?;
            let container = spec
                .containers
                .iter_mut()
                .find(|container| container.name == "workflow")
                .ok_or_else(|| Error::MissingPodContainer(name.clone()))?;
            pod.metadata.name = Some(name.clone());
            container.image = Some(image.into());
            container.args = Some(args.to_vec());
            let params = PostParams::default();
            let k8s: Api<Pod> = Api::default_namespaced(self.k8s.clone());
            debug!("starting pod");
            k8s.create(&params, &pod).await?;
            Ok(name)
        }
        .instrument(span)
        .await
    }
}
