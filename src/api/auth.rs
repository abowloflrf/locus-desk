use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    routing::{get, post},
};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;

use crate::{
    api::ApiJson,
    auth,
    error::AppResult,
    state::AppState,
    workspace::{RequestContext, SessionInfo},
};

const LOGIN_BODY_LIMIT_BYTES: usize = 8 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/auth/login",
            post(login).layer(DefaultBodyLimit::max(LOGIN_BODY_LIMIT_BYTES)),
        )
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    username: String,
    password: String,
}

async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    ApiJson(request): ApiJson<LoginRequest>,
) -> AppResult<(CookieJar, Json<SessionInfo>)> {
    let username = auth::canonical_username(&request.username)?;
    let now = state.clock().now().timestamp_millis();
    let reservation = state.login_limiter().reserve(&username, now)?;
    let result = auth::login(state.pool(), &username, &request.password, now).await;
    let (token, identity) = match result {
        Ok(value) => {
            reservation.succeed(now)?;
            value
        }
        Err(error) => {
            reservation.fail(now)?;
            return Err(error);
        }
    };
    let context = RequestContext::from_identity(identity)?;
    let info = context.session_info(&state);
    let cookie = auth::session_cookie(token, state.config().cookie_secure());
    Ok((jar.add(cookie), Json(info)))
}

async fn logout(
    State(state): State<AppState>,
    context: RequestContext,
    jar: CookieJar,
) -> AppResult<(CookieJar, StatusCode)> {
    auth::delete_session(state.pool(), &context.token_hash).await?;
    Ok((jar.remove(auth::removal_cookie()), StatusCode::NO_CONTENT))
}

async fn me(State(state): State<AppState>, context: RequestContext) -> Json<SessionInfo> {
    Json(context.session_info(&state))
}
