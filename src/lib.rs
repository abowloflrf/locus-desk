pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod static_files;
pub mod workspace;

use axum::Router;

/// Builds the complete HTTP router.
pub fn app() -> Router {
    let api = Router::new()
        .nest("/v1", api::router())
        .fallback(api::not_found);

    Router::new()
        .nest("/api", api)
        .fallback(static_files::serve)
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
    };
    use serde_json::Value;
    use tower::ServiceExt;

    use super::app;

    #[tokio::test]
    async fn health_endpoint_reports_service_metadata() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/health")
                    .body(Body::empty())
                    .expect("request should be valid"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE),
            Some(&header::HeaderValue::from_static("application/json"))
        );

        let body = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("response body should be readable");
        let payload: Value = serde_json::from_slice(&body).expect("response should be JSON");

        assert_eq!(payload["status"], "ok");
        assert_eq!(payload["service"], "locus-desk");
        assert_eq!(payload["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(payload["schemaVersion"], 0);
    }

    #[tokio::test]
    async fn unknown_api_routes_never_fall_back_to_the_spa() {
        for uri in ["/api/unknown", "/api/v1/unknown"] {
            let response = app()
                .oneshot(
                    Request::builder()
                        .uri(uri)
                        .body(Body::empty())
                        .expect("request should be valid"),
                )
                .await
                .expect("router should respond");

            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(
                response.headers().get(header::CONTENT_TYPE),
                Some(&header::HeaderValue::from_static("application/json"))
            );
        }
    }
}
