use crabflow::{
    tokio::{TokioExecutor, TokioTask},
    LoopOrchestrator, VecTaskGroup,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let a = TokioTask::new(|| async {
        println!("a");
        Ok::<(), anyhow::Error>(())
    });
    let b = TokioTask::new(|| async {
        println!("a");
        Ok::<(), anyhow::Error>(())
    });
    let c = TokioTask::new(|| async {
        println!("c");
        Ok::<(), anyhow::Error>(())
    });
    let group = VecTaskGroup::new([a, b]).then(VecTaskGroup::new([c]));
    let orch = LoopOrchestrator::new();
    let status = orch.process(&group, &TokioExecutor).await?;
    println!("{status:?}");
    Ok(())
}
