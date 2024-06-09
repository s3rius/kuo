use std::sync::Arc;

use kuo::operator::{ctx::OperatorCtx, error::KuoResult};

#[tokio::main]
pub async fn main() -> KuoResult<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();
    let ctx = Arc::new(OperatorCtx::new().await?);
    kuo::operator::controller::run(ctx.clone()).await?;
    Ok(())
}
