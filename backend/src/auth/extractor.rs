use actix_web::{web, FromRequest, HttpRequest};
use futures_util::future::{ready, Ready};
use uuid::Uuid;

use crate::auth::jwt;
use crate::errors::ApiError;
use crate::models::ROLE_ADMIN;
use crate::state::AppState;

/// Extracted from a valid `Authorization: Bearer <jwt>` header.
/// Any handler taking this parameter requires authentication.
#[derive(Debug, Clone)]
pub struct AuthedUser {
    pub id: Uuid,
    pub email: String,
    pub role: String,
}

impl AuthedUser {
    pub fn require_admin(&self) -> Result<(), ApiError> {
        if self.role == ROLE_ADMIN {
            Ok(())
        } else {
            Err(ApiError::Forbidden(
                "only admin can perform this action".into(),
            ))
        }
    }
}

impl FromRequest for AuthedUser {
    type Error = ApiError;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut actix_web::dev::Payload) -> Self::Future {
        ready(extract(req))
    }
}

fn extract(req: &HttpRequest) -> Result<AuthedUser, ApiError> {
    let state = req
        .app_data::<web::Data<AppState>>()
        .ok_or_else(|| ApiError::Internal("app state missing".into()))?;

    let header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("missing Authorization header".into()))?;

    let token = header
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::Unauthorized("expected Bearer token".into()))?;

    let claims = jwt::verify_token(token, &state.config.jwt_secret)
        .map_err(|_| ApiError::Unauthorized("invalid or expired token".into()))?;

    Ok(AuthedUser {
        id: claims.sub,
        email: claims.email,
        role: claims.role,
    })
}
