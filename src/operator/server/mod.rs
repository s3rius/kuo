use std::sync::Arc;

use axum::routing::get;
use tower_http::trace::TraceLayer;

use super::ctx::OperatorCtx;

async fn health() -> &'static str {
    "OK"
}

pub async fn run(ctx: Arc<OperatorCtx>) -> anyhow::Result<()> {
    tracing::info!("Starting server");
    // build our application with a route
    let app = axum::Router::new().route("/health", get(health)).layer(
        TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<_>| {
            // Log the matched route's path (with placeholders not filled in).
            // Use request.uri() or OriginalUri if you want the real path.
            let matched_path = request.uri().path_and_query().map(ToString::to_string);

            tracing::info_span!(
                "http_request",
                method = ?request.method(),
                matched_path,
                some_other_field = tracing::field::Empty,
            )
        }),
    );

    let tcp_listener =
        tokio::net::TcpListener::bind((ctx.args.server_host.clone(), ctx.args.server_port)).await?;

    axum::serve(tcp_listener, app).await?;
    Ok(())
}
