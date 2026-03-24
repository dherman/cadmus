use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use super::middleware::AuthUser;
use super::tokens;
use crate::auth::{jwt, password};
use crate::errors::AppError;
use crate::AppState;

// ── Request types ──

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub display_name: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

// ── Response types ──

#[derive(Serialize)]
pub struct AuthResponse {
    pub user: UserProfile,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

#[derive(Serialize)]
pub struct UserProfile {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_in: u64,
}

#[derive(Serialize)]
pub struct WsTokenResponse {
    pub ws_token: String,
    pub expires_in: u64,
}

// ── Handlers ──

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), AppError> {
    // Validate
    let email = body.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::BadRequest("Invalid email address".to_string()));
    }

    let display_name = body.display_name.trim().to_string();
    if display_name.is_empty() || display_name.len() > 100 {
        return Err(AppError::BadRequest(
            "Display name must be between 1 and 100 characters".to_string(),
        ));
    }

    if body.password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    // Check for existing user
    if state
        .db
        .get_user_by_email(&email)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .is_some()
    {
        return Err(AppError::Conflict("Email already registered".to_string()));
    }

    // Hash password
    let password_hash = password::hash_password(&body.password)?;

    // Create user
    let id = Uuid::new_v4();
    let user = state
        .db
        .create_user(id, &email, &display_name, &password_hash)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Issue tokens
    let access_token = jwt::create_access_token(
        user.id,
        &user.email,
        &user.display_name,
        &state.config.jwt_secret,
    )?;
    let refresh_token = jwt::create_refresh_token(user.id, &state.config.jwt_secret)?;

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            user: UserProfile {
                id: user.id,
                email: user.email,
                display_name: user.display_name,
            },
            access_token,
            refresh_token,
            expires_in: 900,
        }),
    ))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let email = body.email.trim().to_lowercase();

    let user = state
        .db
        .get_user_by_email(&email)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Unauthorized("Invalid email or password".to_string()))?;

    if !password::verify_password(&body.password, &user.password_hash)? {
        return Err(AppError::Unauthorized(
            "Invalid email or password".to_string(),
        ));
    }

    let access_token = jwt::create_access_token(
        user.id,
        &user.email,
        &user.display_name,
        &state.config.jwt_secret,
    )?;
    let refresh_token = jwt::create_refresh_token(user.id, &state.config.jwt_secret)?;

    Ok(Json(AuthResponse {
        user: UserProfile {
            id: user.id,
            email: user.email,
            display_name: user.display_name,
        },
        access_token,
        refresh_token,
        expires_in: 900,
    }))
}

pub async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<TokenResponse>, AppError> {
    let claims = jwt::validate_token(&body.refresh_token, "refresh", &state.config.jwt_secret)?;

    let user_id: Uuid = claims
        .sub
        .parse()
        .map_err(|_| AppError::Unauthorized("Invalid or expired token".to_string()))?;

    // Ensure user still exists
    let user = state
        .db
        .get_user_by_id(user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Unauthorized("Invalid or expired token".to_string()))?;

    let access_token = jwt::create_access_token(
        user.id,
        &user.email,
        &user.display_name,
        &state.config.jwt_secret,
    )?;

    Ok(Json(TokenResponse {
        access_token,
        expires_in: 900,
    }))
}

pub async fn ws_token(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<WsTokenResponse>, AppError> {
    let ws_token = jwt::create_ws_token(auth.user_id, &state.config.jwt_secret)?;

    Ok(Json(WsTokenResponse {
        ws_token,
        expires_in: 30,
    }))
}

pub async fn me(auth: AuthUser) -> Json<UserProfile> {
    Json(UserProfile {
        id: auth.user_id,
        email: auth.email,
        display_name: auth.display_name,
    })
}

// ── Agent token handlers ──

pub async fn create_token(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<tokens::CreateAgentTokenRequest>,
) -> Result<(StatusCode, Json<tokens::AgentTokenCreatedResponse>), AppError> {
    // Validate name
    let name = body.name.trim().to_string();
    if name.is_empty() || name.len() > 255 {
        return Err(AppError::BadRequest(
            "Token name must be between 1 and 255 characters".into(),
        ));
    }

    // Validate scopes
    tokens::validate_scopes(&body.scopes)?;

    // Validate expires_in
    let duration = tokens::parse_expires_in(&body.expires_in)?;
    let expires_at = Utc::now() + duration;

    // Validate document_ids if provided
    if let Some(ref doc_ids) = body.document_ids {
        for doc_id in doc_ids {
            // Check user has access to this document
            state
                .db
                .get_user_permission(*doc_id, auth.user_id)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?
                .ok_or_else(|| {
                    AppError::Forbidden(format!(
                        "You don't have access to document {}",
                        doc_id
                    ))
                })?;
        }
    }

    // Generate token
    let (raw_secret, token_hash) = tokens::generate_agent_token();

    // Store in DB
    let row = state
        .db
        .create_agent_token(
            auth.user_id,
            &name,
            &token_hash,
            &body.scopes,
            body.document_ids.as_deref(),
            expires_at,
        )
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(tokens::AgentTokenCreatedResponse {
            token_id: row.id,
            secret: raw_secret,
            name: row.name,
            scopes: row.scopes,
            expires_at: row.expires_at,
        }),
    ))
}

pub async fn list_tokens(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<tokens::AgentTokenResponse>>, AppError> {
    let rows = state
        .db
        .list_agent_tokens(auth.user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let tokens: Vec<tokens::AgentTokenResponse> = rows.into_iter().map(Into::into).collect();
    Ok(Json(tokens))
}

pub async fn revoke_token(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(token_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let found = state
        .db
        .revoke_agent_token(token_id, auth.user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if !found {
        return Err(AppError::NotFound("Token not found".into()));
    }

    Ok(StatusCode::NO_CONTENT)
}
