//! Workspace authorization boundary.

use std::str::FromStr;

use axum::{extract::FromRequestParts, http::request::Parts};
use axum_extra::extract::cookie::CookieJar;
use chrono_tz::Tz;
use serde::Serialize;

use crate::{
    auth::{self, SESSION_COOKIE_NAME, SessionIdentity},
    clock,
    error::{AppError, AppResult},
    state::AppState,
};

#[derive(Clone, Debug)]
pub struct RequestContext {
    pub token_hash: String,
    pub user_id: i64,
    pub user_uid: String,
    pub username: String,
    pub workspace_id: i64,
    pub workspace_uid: String,
    pub workspace_name: String,
    pub timezone: Tz,
    pub role: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionInfo {
    pub user: SessionUser,
    pub workspace: SessionWorkspace,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionUser {
    pub uid: String,
    pub username: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionWorkspace {
    pub uid: String,
    pub name: String,
    pub timezone: String,
    pub today: String,
    pub role: String,
}

impl RequestContext {
    pub fn from_identity(identity: SessionIdentity) -> AppResult<Self> {
        let timezone = Tz::from_str(&identity.timezone).map_err(|_| {
            AppError::Internal(format!(
                "workspace contains invalid timezone: {:?}",
                identity.timezone
            ))
        })?;
        Ok(Self {
            token_hash: identity.token_hash,
            user_id: identity.user_id,
            user_uid: identity.user_uid,
            username: identity.username,
            workspace_id: identity.workspace_id,
            workspace_uid: identity.workspace_uid,
            workspace_name: identity.workspace_name,
            timezone,
            role: identity.role,
        })
    }

    pub fn session_info(&self, state: &AppState) -> SessionInfo {
        SessionInfo {
            user: SessionUser {
                uid: self.user_uid.clone(),
                username: self.username.clone(),
            },
            workspace: SessionWorkspace {
                uid: self.workspace_uid.clone(),
                name: self.workspace_name.clone(),
                timezone: self.timezone.name().to_owned(),
                today: clock::today(state.clock().now(), self.timezone).to_string(),
                role: self.role.clone(),
            },
        }
    }
}

impl FromRequestParts<AppState> for RequestContext {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(SESSION_COOKIE_NAME)
            .map(|cookie| cookie.value().to_owned())
            .ok_or_else(AppError::unauthorized)?;
        let identity =
            auth::authenticate(state.pool(), &token, state.clock().now().timestamp_millis())
                .await?;
        Self::from_identity(identity)
    }
}
