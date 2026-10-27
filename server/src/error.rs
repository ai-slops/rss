use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Sign in to control this production")]
    Unauthorized,
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Unavailable(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", self.to_string()),
            Self::Invalid(message) => (StatusCode::UNPROCESSABLE_ENTITY, "invalid", message),
            Self::Conflict(message) => (StatusCode::CONFLICT, "conflict", message),
            Self::Unavailable(message) => (StatusCode::SERVICE_UNAVAILABLE, "unavailable", message),
            Self::Internal(error) => {
                tracing::error!(error = ?error, "command failed");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal", "The operation failed. Check service diagnostics; previous production state is retained.".into())
            }
        };
        (status, Json(ErrorBody { code, message })).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
