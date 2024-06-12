use axum::response::IntoResponse;

pub async fn healthcheck() -> impl IntoResponse {
    "OK"
}
