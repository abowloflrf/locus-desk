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
    let Some(asset) = WebAssets::get(path) else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::CACHE_CONTROL, "no-store")],
            "Frontend assets are unavailable. Run `pnpm --dir web build` first.",
        )
            .into_response();
    };

    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let cache_control = if path == "index.html" {
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
        .body(Body::from(asset.data.into_owned()))
        .expect("static response headers should be valid")
}
