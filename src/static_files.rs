use std::borrow::Cow;

use axum::{
    body::Body,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/dist/"]
struct WebAssets;

pub async fn serve(uri: Uri) -> Response {
    let requested_path = uri.path().trim_start_matches('/');

    if requested_path.split('/').any(|segment| segment == "..") {
        return StatusCode::NOT_FOUND.into_response();
    }

    if requested_path.is_empty() {
        return serve_asset("index.html");
    }

    if WebAssets::get(requested_path).is_some() {
        return serve_asset(requested_path);
    }

    let last_segment = requested_path.rsplit('/').next().unwrap_or_default();
    if last_segment.contains('.') {
        StatusCode::NOT_FOUND.into_response()
    } else {
        serve_asset("index.html")
    }
}

fn serve_asset(path: &str) -> Response {
    let Some(data) = asset_data(path) else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::CACHE_CONTROL, "no-store")],
            "Frontend assets are unavailable. Run `pnpm --dir web build` first.",
        )
            .into_response();
    };

    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let cache_control = if matches!(path, "index.html" | "service-worker.js") {
        "no-cache"
    } else if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=3600"
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CACHE_CONTROL, cache_control)
        .body(Body::from(data.into_owned()))
        .expect("static response headers should be valid")
}

fn asset_data(path: &str) -> Option<Cow<'static, [u8]>> {
    if let Some(asset) = WebAssets::get(path) {
        return Some(asset.data);
    }
    #[cfg(test)]
    if path == "index.html" {
        return Some(Cow::Borrowed(include_bytes!("../web/index.html")));
    }
    None
}

#[cfg(test)]
mod tests {
    use axum::http::{StatusCode, Uri, header};

    use super::serve;

    #[tokio::test]
    async fn serves_spa_fallback_without_caching_the_document() {
        let response = serve(Uri::from_static("/archive")).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-cache"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/html"
        );
    }

    #[tokio::test]
    async fn missing_file_like_paths_do_not_fall_back_to_the_spa() {
        let response = serve(Uri::from_static("/missing.js")).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn serves_the_service_worker_without_http_caching() {
        let response = serve(Uri::from_static("/service-worker.js")).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-cache"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/javascript"
        );
    }

    #[tokio::test]
    async fn serves_installable_app_metadata_and_icons() {
        let manifest = serve(Uri::from_static("/manifest.webmanifest")).await;
        assert_eq!(manifest.status(), StatusCode::OK);
        assert_eq!(
            manifest.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/manifest+json"
        );

        let icon = serve(Uri::from_static("/icons/apple-touch-icon.png")).await;
        assert_eq!(icon.status(), StatusCode::OK);
        assert_eq!(
            icon.headers().get(header::CONTENT_TYPE).unwrap(),
            "image/png"
        );
    }
}
