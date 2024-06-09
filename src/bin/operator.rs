use std::sync::Arc;

use kuo::operator::{ctx::OperatorCtx, error::KuoResult};

#[tokio::main]
pub async fn main() -> KuoResult<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();
    let ctx = Arc::new(OperatorCtx::new().await?);
    tokio::select! {
        _ = kuo::operator::controller::run(ctx.clone()) => {
            tracing::warn!("Controller stopped. Exiting.");
        }
        _ = kuo::operator::server::run(ctx.clone()) => {
            tracing::warn!("Server stopped. Exiting.");
        }
    }
    Ok(())
}
