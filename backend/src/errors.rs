use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde_json::json;

#[derive(Debug)]
pub enum ApiError {
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    BadRequest(String),
    Internal(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, msg) = match self {
            ApiError::Unauthorized(m) => ("unauthorized", m),
            ApiError::Forbidden(m) => ("forbidden", m),
            ApiError::NotFound(m) => ("not_found", m),
            ApiError::BadRequest(m) => ("bad_request", m),
            ApiError::Internal(m) => ("internal", m),
        };
        write!(f, "{kind}: {msg}")
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let message = match self {
            // Never leak internal error details to clients.
            ApiError::Internal(m) => {
                tracing::error!("internal error: {m}");
                "internal server error".to_string()
            }
            other => other.to_string(),
        };
        HttpResponse::build(self.status_code()).json(json!({ "error": message }))
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Internal(format!("database error: {e}"))
    }
}

impl From<redis::RedisError> for ApiError {
    fn from(e: redis::RedisError) -> Self {
        ApiError::Internal(format!("redis error: {e}"))
    }
}
