use std::sync::Arc;

use kuo::operator::{ctx::OperatorCtx, error::KuoResult};

#[tokio::main]
pub async fn main() -> KuoResult<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();
    let ctx = Arc::new(OperatorCtx::new().await?);
    let operator = kuo::operator::controller::run(ctx.clone());
    let server = kuo::server::run(ctx.clone());

    tokio::select! {
        res = operator => {
            match res {
                Ok(_) => tracing::info!("Operator has stopped."),
                Err(e) => tracing::error!("Operator has crashed: {:?}", e),
            }
        }
        res = server => {
            match res {
                Ok(_) => tracing::info!("Server has stopped."),
                Err(e) => tracing::error!("Server has crashed: {:?}", e),
            }
        }
    }
    Ok(())
}
