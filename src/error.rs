use std::{borrow::Cow, io};

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use sqlx::migrate::MigrateError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{message}")]
    Client {
        status: StatusCode,
        code: &'static str,
        message: Cow<'static, str>,
    },
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migration(#[from] MigrateError),
    #[error("filesystem operation failed: {0}")]
    Io(#[from] io::Error),
    #[error("password processing failed")]
    Password,
    #[error("random number generation failed")]
    Random,
    #[error("application setup failed: {0}")]
    Setup(String),
    #[error("internal application error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn bad_request(message: impl Into<Cow<'static, str>>) -> Self {
        Self::client(StatusCode::BAD_REQUEST, "invalid_request", message)
    }

    pub fn unauthorized() -> Self {
        Self::client(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "Authentication is required",
        )
    }

    pub fn invalid_credentials() -> Self {
        Self::client(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "Username or password is incorrect",
        )
    }

    pub fn not_found(resource: &'static str) -> Self {
        Self::client(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("{resource} was not found"),
        )
    }

    pub fn validation(message: impl Into<Cow<'static, str>>) -> Self {
        Self::client(
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation_failed",
            message,
        )
    }

    pub fn rate_limited() -> Self {
        Self::client(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many login attempts; try again later",
        )
    }

    pub fn forbidden_origin() -> Self {
        Self::client(
            StatusCode::FORBIDDEN,
            "origin_not_allowed",
            "Cross-origin modification is not allowed",
        )
    }

    pub fn client(
        status: StatusCode,
        code: &'static str,
        message: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::Client {
            status,
            code,
            message: message.into(),
        }
    }
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            Self::Client {
                status,
                code,
                message,
            } => (*status, *code, message.as_ref()),
            _ => {
                tracing::error!(error = %self, "request failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "An unexpected server error occurred",
                )
            }
        };

        (
            status,
            Json(ErrorEnvelope {
                error: ErrorBody { code, message },
            }),
        )
            .into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
