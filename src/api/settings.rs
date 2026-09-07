use std::time::{Duration, Instant};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::{
    api::ApiJson,
    error::{AppError, AppResult},
    state::AppState,
    workspace::RequestContext,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/settings/ai", get(read).put(save))
        .route("/settings/ai/test", post(test_connection))
}

#[derive(Default, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
struct AiSettings {
    base_url: String,
    model: String,
    has_api_key: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SaveSettings {
    base_url: String,
    model: String,
    // Omitted keys preserve the saved credential; empty keys clear it.
    api_key: Option<String>,
}

async fn read(
    State(state): State<AppState>,
    context: RequestContext,
) -> AppResult<Json<AiSettings>> {
    let settings = sqlx::query_as::<_, AiSettings>(
        "SELECT base_url, model, api_key != '' AS has_api_key FROM ai_settings WHERE workspace_id = ? AND user_id = ?",
    ).bind(context.workspace_id).bind(context.user_id).fetch_optional(state.pool()).await?;
    Ok(Json(settings.unwrap_or_default()))
}

async fn save(
    State(state): State<AppState>,
    context: RequestContext,
    ApiJson(input): ApiJson<SaveSettings>,
) -> AppResult<Json<AiSettings>> {
    validate_settings(&input)?;
    let base_url = input.base_url.trim().trim_end_matches('/');
    let model = input.model.trim();
    let api_key = input.api_key.as_deref().map(str::trim);
    let settings = sqlx::query_as::<_, AiSettings>(
        "INSERT INTO ai_settings (workspace_id, user_id, base_url, model, api_key) VALUES (?, ?, ?, ?, COALESCE(?, ''))
         ON CONFLICT (workspace_id, user_id) DO UPDATE SET base_url = excluded.base_url, model = excluded.model, api_key = COALESCE(?, ai_settings.api_key)
         RETURNING base_url, model, api_key != '' AS has_api_key",
    ).bind(context.workspace_id).bind(context.user_id).bind(base_url).bind(model).bind(api_key).bind(api_key)
    .fetch_one(state.pool()).await?;
    Ok(Json(settings))
}

fn validate_settings(input: &SaveSettings) -> AppResult<()> {
    let base_url = input.base_url.trim().trim_end_matches('/');
    let model = input.model.trim();
    let url = url::Url::parse(base_url)
        .map_err(|_| AppError::validation("Enter a valid HTTP or HTTPS base URL."))?;
    if base_url.len() > 2048
        || !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::validation(
            "Use an HTTP or HTTPS base URL without credentials, query parameters, or fragments.",
        ));
    }
    if model.is_empty() || model.len() > 200 || model.chars().any(char::is_control) {
        return Err(AppError::validation(
            "Enter a model name of up to 200 bytes without control characters.",
        ));
    }
    let api_key = input.api_key.as_deref().map(str::trim);
    if api_key.is_some_and(|key| key.len() > 8192 || key.chars().any(char::is_control)) {
        return Err(AppError::validation(
            "The API key is too long or contains control characters.",
        ));
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionResult {
    elapsed_ms: u128,
}

async fn test_connection(
    State(state): State<AppState>,
    context: RequestContext,
    ApiJson(input): ApiJson<SaveSettings>,
) -> AppResult<Json<ConnectionResult>> {
    validate_settings(&input)?;
    let base_url = input.base_url.trim().trim_end_matches('/');
    let key =
        match input.api_key.as_deref() {
            Some(key) => key.trim().to_owned(),
            None => {
                let saved = sqlx::query_as::<_, (String, String)>(
                "SELECT base_url, api_key FROM ai_settings WHERE workspace_id = ? AND user_id = ?",
            ).bind(context.workspace_id).bind(context.user_id).fetch_optional(state.pool()).await?;
                match saved {
                    Some((saved_url, key)) if !key.is_empty() => {
                        // Never forward a stored credential to an untested replacement endpoint.
                        if saved_url != base_url {
                            return Err(AppError::validation(
                                "The base URL changed. Enter an API key to test this endpoint.",
                            ));
                        }
                        key
                    }
                    _ => String::new(),
                }
            }
        };
    probe_model(base_url, input.model.trim(), &key, Duration::from_secs(30))
        .await
        .map(Json)
}

async fn probe_model(
    base_url: &str,
    model: &str,
    key: &str,
    timeout: Duration,
) -> AppResult<ConnectionResult> {
    let started = Instant::now();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(timeout)
        .build()
        .map_err(|_| provider_error("Unable to initialize the model connection."))?;
    let payload = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "Reply with OK only."}],
        "stream": false,
        "max_tokens": 32
    });
    let mut request = client
        .post(format!("{base_url}/chat/completions"))
        .header("Content-Type", "application/json")
        .body(payload.to_string());
    if !key.is_empty() {
        request = request.bearer_auth(key);
    }
    let mut response = request.send().await.map_err(connection_error)?;
    let status = response.status();
    if !status.is_success() {
        let message = match status.as_u16() {
            401 | 403 => {
                "The provider rejected authentication. Check the API key and model permissions."
            }
            404 => "The endpoint or model was not found. Check the base URL and model ID.",
            429 => "The provider rate limit or quota was exceeded. Check your quota and try again.",
            400 | 422 => {
                "The provider rejected the test request. Check the model ID and Chat Completions compatibility."
            }
            300..=399 => "The provider redirected the request. Enter the final API base URL.",
            _ => "The provider could not complete the test request. Try again later.",
        };
        return Err(provider_error(format!(
            "{message} (HTTP {})",
            status.as_u16()
        )));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(connection_error)? {
        if bytes.len() + chunk.len() > 64 * 1024 {
            return Err(provider_error(
                "The provider response exceeded the test size limit.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| {
        provider_error("The provider returned invalid JSON. Check the API base URL.")
    })?;
    let content = value
        .pointer("/choices/0/message/content")
        .and_then(serde_json::Value::as_str);
    if !content.is_some_and(|content| !content.trim().is_empty()) {
        return Err(provider_error(
            "The provider returned no text completion. Check the model's Chat Completions compatibility.",
        ));
    }
    Ok(ConnectionResult {
        elapsed_ms: started.elapsed().as_millis(),
    })
}

fn provider_error(message: impl Into<String>) -> AppError {
    AppError::client(StatusCode::BAD_GATEWAY, "model_test_failed", message.into())
}

fn connection_error(error: reqwest::Error) -> AppError {
    if error.is_timeout() {
        provider_error("The model request timed out. Try again or check the provider.")
    } else {
        // Provider errors and response bodies may contain credentials.
        provider_error(
            "Unable to reach the provider. Check the URL, network, and TLS configuration.",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{response::IntoResponse, routing::post};

    #[tokio::test]
    async fn model_probe_checks_response_and_bounds_failures() {
        for (status, body, expected) in [
            (
                200,
                r#"{"choices":[{"message":{"content":"OK"}}]}"#.to_owned(),
                None,
            ),
            (200, "<html>Login</html>".to_owned(), Some("invalid JSON")),
            (
                200,
                r#"{"choices":[]}"#.to_owned(),
                Some("no text completion"),
            ),
            (401, "secret-key".to_owned(), Some("authentication")),
            (404, String::new(), Some("not found")),
            (429, String::new(), Some("quota")),
            (302, String::new(), Some("redirected")),
            (200, "x".repeat(65537), Some("size limit")),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let app = Router::new().route(
                "/v1/chat/completions",
                post(
                    move |headers: axum::http::HeaderMap,
                          Json(payload): Json<serde_json::Value>| async move {
                        assert_eq!(headers["authorization"], "Bearer secret-key");
                        assert_eq!(payload["model"], "test-model");
                        assert_eq!(payload["stream"], false);
                        (
                            StatusCode::from_u16(status).unwrap(),
                            [("location", "/redirect-target")],
                            body,
                        )
                            .into_response()
                    },
                ),
            );
            let server = tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            });
            let result = probe_model(
                &format!("http://{address}/v1"),
                "test-model",
                "secret-key",
                Duration::from_secs(2),
            )
            .await;
            server.abort();
            if let Some(expected) = expected {
                let error = result
                    .err()
                    .expect("provider response should fail")
                    .to_string();
                assert!(error.contains(expected), "{error}");
                assert!(!error.contains("secret-key"));
            } else {
                assert!(result.is_ok());
            }
        }
    }

    #[tokio::test]
    async fn model_probe_times_out() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                tokio::time::sleep(Duration::from_secs(1)).await;
                "{}"
            }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let result = probe_model(
            &format!("http://{address}/v1"),
            "test-model",
            "",
            Duration::from_millis(50),
        )
        .await;
        server.abort();
        assert!(result.err().unwrap().to_string().contains("timed out"));
    }
}
