use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

use super::jwt;
use super::tokens;
use crate::errors::AppError;
use crate::AppState;

/// Authenticated user extracted from a valid JWT access token or agent token.
/// Add this to any handler's parameters to require authentication.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub is_agent: bool,
    pub agent_name: Option<String>,
    pub token_scopes: Option<Vec<String>>,
    pub token_document_ids: Option<Vec<Uuid>>,
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

        // 3. Route based on token prefix
        if token.starts_with("cadmus_") {
            // Agent token path
            let hash = tokens::hash_token(token);
            let row = state
                .db
                .get_agent_token_by_hash(&hash)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?
                .ok_or_else(|| AppError::Unauthorized("Invalid agent token".into()))?;

            // Check revoked
            if row.revoked_at.is_some() {
                return Err(AppError::Unauthorized("Agent token has been revoked".into()));
            }

            // Check expired
            if row.expires_at < Utc::now() {
                return Err(AppError::Unauthorized("Agent token has expired".into()));
            }

            // Look up user for email/display_name
            let user = state
                .db
                .get_user_by_id(row.user_id)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?
                .ok_or_else(|| AppError::Unauthorized("Token owner no longer exists".into()))?;

            Ok(AuthUser {
                user_id: row.user_id,
                email: user.email,
                display_name: user.display_name,
                is_agent: true,
                agent_name: Some(row.name),
                token_scopes: Some(row.scopes),
                token_document_ids: row.document_ids,
            })
        } else {
            // JWT path (existing logic)
            let claims = jwt::validate_token(token, "access", &state.config.jwt_secret)?;

            let user_id = Uuid::parse_str(&claims.sub)
                .map_err(|_| AppError::Unauthorized("Invalid token subject".into()))?;

            Ok(AuthUser {
                user_id,
                email: claims.email.unwrap_or_default(),
                display_name: claims.name.unwrap_or_default(),
                is_agent: false,
                agent_name: None,
                token_scopes: None,
                token_document_ids: None,
            })
        }
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
