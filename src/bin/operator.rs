use std::sync::Arc;

use kuo::operator::ctx::OperatorCtx;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();
    let ctx = Arc::new(OperatorCtx::new().await?);
    kuo::operator::controller::run(ctx.clone()).await?;
    Ok(())
}
