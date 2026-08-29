mod auth;
mod library;
mod notes;
mod tasks;

use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, FromRequest, FromRequestParts, Query, Request, State,
        rejection::JsonRejection,
    },
    http::{HeaderValue, Method, StatusCode, Uri, header, request::Parts},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    db,
    error::{AppError, AppResult},
    state::AppState,
};

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/bootstrap/status", get(bootstrap_status))
        .merge(auth::router())
        .merge(library::router())
        .merge(notes::router())
        .merge(tasks::router())
        .method_not_allowed_fallback(method_not_allowed)
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(state, enforce_same_origin))
}

pub(crate) async fn disable_caching(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
    git_commit: &'static str,
    schema_version: i64,
}

async fn health(State(state): State<AppState>) -> AppResult<Json<HealthResponse>> {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(state.pool())
        .await?;
    Ok(Json(HealthResponse {
        status: "ok",
        service: "locus-desk",
        version: env!("CARGO_PKG_VERSION"),
        git_commit: crate::version::GIT_COMMIT,
        schema_version: db::schema_version(state.pool()).await?,
    }))
}

#[derive(Serialize)]
struct BootstrapStatusResponse {
    initialized: bool,
}

async fn bootstrap_status(
    State(state): State<AppState>,
) -> AppResult<Json<BootstrapStatusResponse>> {
    Ok(Json(BootstrapStatusResponse {
        initialized: db::is_initialized(state.pool()).await?,
    }))
}

pub struct ApiJson<T>(pub T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = AppError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(request, state)
            .await
            .map(|Json(value)| Self(value))
            .map_err(|rejection: JsonRejection| {
                AppError::bad_request(format!("Invalid JSON request: {rejection}"))
            })
    }
}

pub struct ApiQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for ApiQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Query::<T>::from_request_parts(parts, state)
            .await
            .map(|Query(value)| Self(value))
            .map_err(|rejection| {
                AppError::bad_request(format!("Invalid query parameters: {rejection}"))
            })
    }
}

async fn enforce_same_origin(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    if is_safe_method(request.method()) {
        return Ok(next.run(request).await);
    }

    let origin = request
        .headers()
        .get(header::ORIGIN)
        .ok_or_else(AppError::forbidden_origin)?
        .to_str()
        .map_err(|_| AppError::forbidden_origin())?;
    let origin_uri: Uri = origin.parse().map_err(|_| AppError::forbidden_origin())?;
    let expected_scheme = if state.config().cookie_secure() {
        "https"
    } else {
        "http"
    };
    let origin_scheme = origin_uri
        .scheme_str()
        .ok_or_else(AppError::forbidden_origin)?;
    let origin_authority = origin_uri
        .authority()
        .map(|authority| authority.as_str())
        .ok_or_else(AppError::forbidden_origin)?;
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(AppError::forbidden_origin)?;
    if origin_scheme != expected_scheme
        || !origin_authority.eq_ignore_ascii_case(host)
        || origin_uri.query().is_some()
        || !matches!(origin_uri.path(), "" | "/")
    {
        return Err(AppError::forbidden_origin());
    }
    Ok(next.run(request).await)
}

const fn is_safe_method(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

pub async fn not_found() -> Response {
    AppError::client(StatusCode::NOT_FOUND, "not_found", "API route not found").into_response()
}

async fn method_not_allowed() -> Response {
    AppError::client(
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        "HTTP method is not allowed for this API route",
    )
    .into_response()
}
