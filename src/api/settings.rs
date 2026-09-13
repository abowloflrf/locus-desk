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
        .route("/settings/general", get(read_general).put(save_general))
        .route("/settings/ai", get(read).put(save))
        .route("/settings/ai/test", post(test_connection))
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeneralSettings {
    language: String,
}

pub(super) async fn language(pool: &sqlx::SqlitePool, user_id: i64) -> AppResult<String> {
    Ok(
        sqlx::query_scalar("SELECT language FROM user_preferences WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or_else(|| "en".to_owned()),
    )
}

async fn read_general(
    State(state): State<AppState>,
    context: RequestContext,
) -> AppResult<Json<GeneralSettings>> {
    Ok(Json(GeneralSettings {
        language: language(state.pool(), context.user_id).await?,
    }))
}

async fn save_general(
    State(state): State<AppState>,
    context: RequestContext,
    ApiJson(input): ApiJson<GeneralSettings>,
) -> AppResult<Json<GeneralSettings>> {
    if !matches!(input.language.as_str(), "en" | "zh-CN") {
        return Err(AppError::validation("Choose Chinese or English."));
    }
    sqlx::query("INSERT INTO user_preferences (user_id, language) VALUES (?, ?) ON CONFLICT (user_id) DO UPDATE SET language = excluded.language")
        .bind(context.user_id).bind(&input.language).execute(state.pool()).await?;
    Ok(Json(input))
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
    complete(
        base_url,
        model,
        key,
        serde_json::json!([
            {"role": "user", "content": "Reply with OK only."}
        ]),
        32,
        timeout,
    )
    .await?;
    Ok(ConnectionResult {
        elapsed_ms: started.elapsed().as_millis(),
    })
}

const MAX_COMPLETION_BYTES: usize = 64 * 1024;
/// Providers reached through an overloaded gateway can accept connections slowly.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(60);

pub(super) async fn complete(
    base_url: &str,
    model: &str,
    key: &str,
    messages: serde_json::Value,
    max_tokens: u32,
    timeout: Duration,
) -> AppResult<String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(timeout)
        .build()
        .map_err(|_| provider_error("Unable to initialize the model connection."))?;
    let payload = serde_json::json!({
        // Streaming keeps bytes moving while the provider prefills the prompt, so
        // gateways in front of the provider do not time out on long articles.
        "model": model, "messages": messages, "stream": true, "max_tokens": max_tokens
    });
    let mut request = client
        .post(format!("{base_url}/chat/completions"))
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
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
                "The provider rejected the request. Check the model ID and Chat Completions compatibility."
            }
            300..=399 => "The provider redirected the request. Enter the final API base URL.",
            _ => "The provider could not complete the request. Try again later.",
        };
        return Err(provider_error(format!(
            "{message} (HTTP {})",
            status.as_u16()
        )));
    }
    read_completion(&mut response).await
}

/// Server-sent events arrive in arbitrary chunks, so partial lines are buffered
/// as bytes until a newline completes them. An SSE response is only accepted
/// after the provider sends an explicit terminal event.
async fn read_completion(response: &mut reqwest::Response) -> AppResult<String> {
    let mut content = String::new();
    let mut pending = Vec::new();
    let mut saw_sse = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("text/event-stream"))
        });
    loop {
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(error) => return Err(connection_error(error)),
        };
        pending.extend_from_slice(&chunk);
        if pending.len() > MAX_COMPLETION_BYTES || content.len() > MAX_COMPLETION_BYTES {
            return Err(provider_error(
                "The provider response exceeded the size limit.",
            ));
        }
        while let Some(end) = pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = pending.drain(..=end).collect();
            let event = read_event(&line, &mut content)?;
            saw_sse |= event.is_sse;
            if event.finished {
                return completion_text(content);
            }
        }
    }
    if !pending.is_empty() {
        let event = read_event(&pending, &mut content)?;
        saw_sse |= event.is_sse;
        if event.finished {
            return completion_text(content);
        }
    }
    if saw_sse {
        return Err(provider_error(
            "The model response was incomplete. Try again or use another model.",
        ));
    }
    completion_text(content)
}

struct CompletionEvent {
    is_sse: bool,
    finished: bool,
}

fn read_event(line: &[u8], content: &mut String) -> AppResult<CompletionEvent> {
    let line = std::str::from_utf8(line).map_err(|_| {
        provider_error("The provider returned invalid JSON. Check the API base URL.")
    })?;
    let line = line.trim();
    let (data, is_sse) = match line.strip_prefix("data:") {
        Some(data) => (data.trim(), true),
        // Tolerate providers that ignore `stream: true` and answer with one JSON body.
        None if line.starts_with('{') => (line, false),
        None => {
            return Ok(CompletionEvent {
                is_sse: false,
                finished: false,
            });
        }
    };
    if data.is_empty() {
        return Ok(CompletionEvent {
            is_sse,
            finished: false,
        });
    }
    if data == "[DONE]" {
        return Ok(CompletionEvent {
            is_sse,
            finished: true,
        });
    }
    let value: serde_json::Value = serde_json::from_str(data).map_err(|_| {
        provider_error("The provider returned invalid JSON. Check the API base URL.")
    })?;
    if value.get("error").is_some() {
        return Err(provider_error(
            "The provider reported an error while generating the response. Try again later.",
        ));
    }
    let finish_reason = value
        .pointer("/choices/0/finish_reason")
        .and_then(serde_json::Value::as_str);
    if finish_reason == Some("length") {
        return Err(provider_error(
            "The model response was incomplete. Try again or use another model.",
        ));
    }
    if let Some(text) = value
        .pointer("/choices/0/delta/content")
        .or_else(|| value.pointer("/choices/0/message/content"))
        .and_then(serde_json::Value::as_str)
    {
        content.push_str(text);
    }
    Ok(CompletionEvent {
        is_sse,
        finished: finish_reason.is_some(),
    })
}

fn completion_text(content: String) -> AppResult<String> {
    if content.trim().is_empty() {
        return Err(provider_error(
            "The provider returned no text completion. Check the model's Chat Completions compatibility.",
        ));
    }
    Ok(content.trim().to_owned())
}

fn provider_error(message: impl Into<String>) -> AppError {
    AppError::client(
        StatusCode::BAD_GATEWAY,
        "model_request_failed",
        message.into(),
    )
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
    async fn model_probe_handles_streamed_responses_and_provider_failures() {
        let oversized = format!(
            "data: {}\n\n",
            serde_json::json!({"choices":[{"delta":{"content":"x".repeat(65_537)}}]})
        );
        for (status, body, expected) in [
            (
                200,
                "data: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}]}\n\ndata: [DONE]\n\n"
                    .to_owned(),
                None,
            ),
            // Providers that ignore `stream: true` still answer with one JSON body.
            (
                200,
                r#"{"choices":[{"message":{"content":"OK"}}]}"#.to_owned(),
                None,
            ),
            (
                200,
                "<html>Login</html>".to_owned(),
                Some("no text completion"),
            ),
            (200, r#"{"choices":[]}"#.to_owned(), Some("no text completion")),
            (
                200,
                "data: {\"choices\":[]}\n\n".to_owned(),
                Some("incomplete"),
            ),
            (
                200,
                "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n"
                    .to_owned(),
                Some("incomplete"),
            ),
            (
                200,
                "data: {\"choices\":[{\"delta\":{\"content\":\"OK\"},\"finish_reason\":\"stop\"}]}\n\n"
                    .to_owned(),
                None,
            ),
            (
                200,
                "data: {\"error\":{\"message\":\"secret-key\"}}\n\n".to_owned(),
                Some("reported an error"),
            ),
            (
                200,
                "data: {\"choices\":[{\"delta\":{\"content\":\"partial\"},\"finish_reason\":\"length\"}]}\n\n"
                    .to_owned(),
                Some("incomplete"),
            ),
            (
                200,
                "data: {\"choices\":\n\n".to_owned(),
                Some("invalid JSON"),
            ),
            (401, "secret-key".to_owned(), Some("authentication")),
            (404, String::new(), Some("not found")),
            (429, String::new(), Some("quota")),
            (302, String::new(), Some("redirected")),
            (200, oversized, Some("size limit")),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let app = Router::new().route(
                "/v1/chat/completions",
                post(
                    move |headers: axum::http::HeaderMap,
                          Json(payload): Json<serde_json::Value>| async move {
                        assert_eq!(headers["authorization"], "Bearer secret-key");
                        assert_eq!(headers["accept"], "text/event-stream");
                        assert_eq!(payload["model"], "test-model");
                        assert_eq!(payload["stream"], true);
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
    async fn complete_reassembles_stream_chunks_split_mid_line() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 4096];
            let _ = socket.read(&mut request).await.unwrap();
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
            for part in [
                "data: {\"choices\":[{\"delta\":{\"content\":\"Hel",
                "lo \"}}]}\n\n: keep-alive\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"world\"}}]}\n",
                "\ndata: [DONE]\n\n",
            ] {
                socket.write_all(part.as_bytes()).await.unwrap();
                socket.flush().await.unwrap();
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        });
        let result = complete(
            &format!("http://{address}/v1"),
            "test-model",
            "",
            serde_json::json!([{"role": "user", "content": "hi"}]),
            32,
            Duration::from_secs(2),
        )
        .await;
        server.abort();
        assert_eq!(result.unwrap(), "Hello world");
    }

    #[tokio::test]
    async fn complete_preserves_utf8_split_across_response_chunks() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 4096];
            let _ = socket.read(&mut request).await.unwrap();
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
            socket
                .write_all(b"data: {\"choices\":[{\"delta\":{\"content\":\"")
                .await
                .unwrap();
            socket.write_all(&[0xe4]).await.unwrap();
            socket.flush().await.unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
            socket.write_all(&[0xbd, 0xa0]).await.unwrap();
            socket
                .write_all(b"\"}}]}\n\ndata: [DONE]\n\n")
                .await
                .unwrap();
        });
        let result = complete(
            &format!("http://{address}/v1"),
            "test-model",
            "",
            serde_json::json!([{"role": "user", "content": "hi"}]),
            32,
            Duration::from_secs(2),
        )
        .await;
        server.abort();
        assert_eq!(result.unwrap(), "你");
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
