use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use std::sync::Arc;
use uuid::Uuid;

use super::jwt;
use crate::errors::AppError;
use crate::AppState;

/// Authenticated user extracted from a valid JWT access token.
/// Add this to any handler's parameters to require authentication.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
}

impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        // 1. Get Authorization header
        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("Missing authorization header".into()))?;

        // 2. Extract Bearer token
        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::Unauthorized("Invalid authorization header".into()))?;

        // 3. Validate JWT (must be "access" type)
        let claims = jwt::validate_token(token, "access", &state.config.jwt_secret)?;

        // 4. Build AuthUser from claims
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| AppError::Unauthorized("Invalid token subject".into()))?;

        Ok(AuthUser {
            user_id,
            email: claims.email.unwrap_or_default(),
            display_name: claims.name.unwrap_or_default(),
        })
    }
}

/// Optional auth extractor — never rejects, returns None if no valid token.
pub struct OptionalAuthUser(pub Option<AuthUser>);

impl FromRequestParts<Arc<AppState>> for OptionalAuthUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        Ok(OptionalAuthUser(
            AuthUser::from_request_parts(parts, state).await.ok(),
        ))
    }
}
