mod health;

use std::sync::Arc;

use crate::operator::ctx::OperatorCtx;

#[allow(clippy::needless_pass_by_value)]
pub fn create_router(_ctx: Arc<OperatorCtx>) -> axum::Router {
    axum::Router::new().route("/health", axum::routing::get(health::healthcheck))
}
