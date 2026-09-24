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
        None,
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

/// Token accounting reported by the provider for a single completion. Fields
/// stay optional because providers differ in what they report.
#[derive(Clone, Copy, Default)]
pub(super) struct TokenUsage {
    pub(super) prompt_tokens: Option<u64>,
    pub(super) completion_tokens: Option<u64>,
    pub(super) total_tokens: Option<u64>,
    /// Prompt tokens served from the provider's cache (`prompt_tokens_details.cached_tokens`).
    pub(super) cached_tokens: Option<u64>,
    /// Hidden thinking tokens billed as output (`completion_tokens_details.reasoning_tokens`).
    pub(super) reasoning_tokens: Option<u64>,
}

pub(super) struct Completion {
    pub(super) content: String,
    pub(super) usage: Option<TokenUsage>,
}

pub(super) async fn complete(
    base_url: &str,
    model: &str,
    key: &str,
    messages: serde_json::Value,
    max_tokens: u32,
    reasoning_effort: Option<&str>,
    timeout: Duration,
) -> AppResult<Completion> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(timeout)
        .build()
        .map_err(|_| provider_error("Unable to initialize the model connection."))?;
    let mut payload = serde_json::json!({
        // Streaming keeps bytes moving while the provider prefills the prompt, so
        // gateways in front of the provider do not time out on long articles.
        // `include_usage` makes OpenAI-compatible providers emit a final usage chunk.
        "model": model, "messages": messages, "stream": true, "max_tokens": max_tokens,
        "stream_options": {"include_usage": true}
    });
    if let Some(effort) = reasoning_effort {
        // Only reasoning models honor this; it keeps thinking from eating the output budget.
        payload["reasoning_effort"] = serde_json::Value::String(effort.to_owned());
    }
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
async fn read_completion(response: &mut reqwest::Response) -> AppResult<Completion> {
    let mut content = String::new();
    let mut usage = None;
    let mut completed = false;
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
            let event = read_event(&line, &mut content, &mut usage)?;
            saw_sse |= event.is_sse;
            completed |= event.finished;
            if event.done {
                return completion(content, usage);
            }
        }
    }
    if !pending.is_empty() {
        let event = read_event(&pending, &mut content, &mut usage)?;
        saw_sse |= event.is_sse;
        completed |= event.finished;
    }
    // A finish reason or `[DONE]` proves the stream ended on purpose; the usage
    // chunk OpenAI sends after the finish reason must not be mistaken for a stall.
    if saw_sse && !completed {
        return Err(provider_error(
            "The model response was incomplete. Try again or use another model.",
        ));
    }
    completion(content, usage)
}

struct CompletionEvent {
    is_sse: bool,
    finished: bool,
    done: bool,
}

fn read_event(
    line: &[u8],
    content: &mut String,
    usage: &mut Option<TokenUsage>,
) -> AppResult<CompletionEvent> {
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
                done: false,
            });
        }
    };
    if data.is_empty() {
        return Ok(CompletionEvent {
            is_sse,
            finished: false,
            done: false,
        });
    }
    if data == "[DONE]" {
        return Ok(CompletionEvent {
            is_sse,
            finished: true,
            done: true,
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
    // The usage chunk arrives after the finish reason and carries no choices.
    if let Some(reported) = token_usage(&value) {
        *usage = Some(reported);
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
        done: false,
    })
}

fn token_usage(value: &serde_json::Value) -> Option<TokenUsage> {
    let usage = value.get("usage").filter(|usage| usage.is_object())?;
    let tokens = |pointer: &str| usage.pointer(pointer).and_then(serde_json::Value::as_u64);
    Some(TokenUsage {
        prompt_tokens: tokens("/prompt_tokens"),
        completion_tokens: tokens("/completion_tokens"),
        total_tokens: tokens("/total_tokens"),
        cached_tokens: tokens("/prompt_tokens_details/cached_tokens")
            .or_else(|| tokens("/prompt_cache_hit_tokens")),
        reasoning_tokens: tokens("/completion_tokens_details/reasoning_tokens"),
    })
}

fn completion(content: String, usage: Option<TokenUsage>) -> AppResult<Completion> {
    if content.trim().is_empty() {
        return Err(provider_error(
            "The provider returned no text completion. Check the model's Chat Completions compatibility.",
        ));
    }
    Ok(Completion {
        content: content.trim().to_owned(),
        usage,
    })
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

    #[test]
    fn token_usage_reads_cache_and_reasoning_details() {
        let usage = token_usage(&serde_json::json!({
            "usage": {
                "prompt_tokens": 1200,
                "completion_tokens": 340,
                "total_tokens": 1540,
                "prompt_tokens_details": {"cached_tokens": 1024},
                "completion_tokens_details": {"reasoning_tokens": 200}
            }
        }))
        .unwrap();
        assert_eq!(usage.prompt_tokens, Some(1200));
        assert_eq!(usage.cached_tokens, Some(1024));
        assert_eq!(usage.reasoning_tokens, Some(200));
        assert_eq!(usage.total_tokens, Some(1540));

        let hit_only = token_usage(&serde_json::json!({
            "usage": {"prompt_tokens": 10, "prompt_cache_hit_tokens": 8}
        }))
        .unwrap();
        assert_eq!(hit_only.cached_tokens, Some(8));
        assert!(token_usage(&serde_json::json!({"choices": []})).is_none());
    }

    #[tokio::test]
    async fn complete_reads_usage_sent_after_the_finish_reason() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"OK\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":11,\"completion_tokens\":2,\"total_tokens\":13,\"prompt_tokens_details\":{\"cached_tokens\":7},\"completion_tokens_details\":{\"reasoning_tokens\":1}}}\n\n",
            "data: [DONE]\n\n"
        );
        let app = Router::new().route(
            "/v1/chat/completions",
            post(move || async move { ([("content-type", "text/event-stream")], body.to_owned()) }),
        );
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let result = complete(
            &format!("http://{address}/v1"),
            "test-model",
            "",
            serde_json::json!([{"role": "user", "content": "hi"}]),
            32,
            None,
            Duration::from_secs(2),
        )
        .await;
        server.abort();
        let result = result.unwrap();
        assert_eq!(result.content, "OK");
        let usage = result.usage.unwrap();
        assert_eq!(usage.prompt_tokens, Some(11));
        assert_eq!(usage.cached_tokens, Some(7));
        assert_eq!(usage.reasoning_tokens, Some(1));
    }

    #[tokio::test]
    async fn complete_sends_reasoning_effort_only_when_requested() {
        for expected in [Some("low"), None] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let app = Router::new().route(
                "/v1/chat/completions",
                post(move |Json(payload): Json<serde_json::Value>| async move {
                    assert_eq!(
                        payload.get("reasoning_effort").and_then(|value| value.as_str()),
                        expected
                    );
                    (
                        [("content-type", "text/event-stream")],
                        "data: {\"choices\":[{\"delta\":{\"content\":\"OK\"}}]}\n\ndata: [DONE]\n\n",
                    )
                }),
            );
            let server = tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            });
            let result = complete(
                &format!("http://{address}/v1"),
                "test-model",
                "",
                serde_json::json!([{"role": "user", "content": "hi"}]),
                32,
                expected,
                Duration::from_secs(2),
            )
            .await;
            server.abort();
            assert_eq!(result.unwrap().content, "OK");
        }
    }

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
            None,
            Duration::from_secs(2),
        )
        .await;
        server.abort();
        assert_eq!(result.unwrap().content, "Hello world");
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
            None,
            Duration::from_secs(2),
        )
        .await;
        server.abort();
        assert_eq!(result.unwrap().content, "你");
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
