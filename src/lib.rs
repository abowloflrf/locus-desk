pub mod api;
pub mod auth;
pub mod clock;
pub mod commands;
pub mod config;
pub mod data_management;
pub mod db;
pub mod error;
pub mod notes;
mod patch;
pub mod state;
pub mod static_files;
pub mod tasks;
pub mod version;
pub mod workspace;

use axum::{Router, extract::Request, middleware};
use state::AppState;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    trace::{DefaultOnResponse, TraceLayer},
};
use tracing::Level;

/// Builds the complete HTTP router.
pub fn app(state: AppState) -> Router {
    let api = Router::new()
        .nest("/v1", api::router(state.clone()))
        .fallback(api::not_found)
        .layer(middleware::from_fn(api::disable_caching));

    let trace = TraceLayer::new_for_http()
        .make_span_with(|request: &Request| {
            let request_id = request
                .extensions()
                .get::<RequestId>()
                .and_then(|value| value.header_value().to_str().ok())
                .unwrap_or("unknown");
            tracing::info_span!(
                "http_request",
                request_id,
                method = %request.method(),
                path = %request.uri().path()
            )
        })
        .on_response(DefaultOnResponse::new().level(Level::INFO));

    Router::new()
        .nest("/api", api)
        .fallback(static_files::serve)
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(trace)
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .with_state(state)
}

#[cfg(test)]
mod integration_tests;
