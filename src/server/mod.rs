mod routes;

use std::sync::Arc;

use crate::operator::{ctx::OperatorCtx, error::KuoResult};

pub async fn run(ctx: Arc<OperatorCtx>) -> KuoResult<()> {
    let listener =
        tokio::net::TcpListener::bind((ctx.args.server.host.as_str(), ctx.args.server.port))
            .await?;
    tracing::info!("Server listening on {}", listener.local_addr()?);
    let router = axum::Router::new().nest("/api", routes::create_router(ctx.clone()));

    axum::serve(listener, router.into_make_service()).await?;
    Ok(())
}
