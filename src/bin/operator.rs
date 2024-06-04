use std::sync::Arc;

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();
    let ctx = Arc::new(kuo::operator::ctx::OperatorCtx::new().await?);

    let controller = kuo::operator::controller::run(ctx.clone());
    let server = kuo::operator::server::run(ctx.clone());

    tokio::select!(
        cnt_result = controller => {
            tracing::warn!("Controller stopped. Exiting.");
            cnt_result?;
        },
        srv_result = server => {
            tracing::warn!("Server stopped. Exiting.");
            srv_result?;
        },
    );
    Ok(())
}
