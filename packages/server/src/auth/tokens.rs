use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::errors::AppError;

/// Valid agent token scopes.
const VALID_SCOPES: &[&str] = &["docs:read", "docs:write", "comments:read", "comments:write"];

// ── Database row ──

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AgentTokenRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub token_hash: String,
    pub scopes: Vec<String>,
    pub document_ids: Option<Vec<Uuid>>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

// ── API types ──

#[derive(Debug, Serialize)]
pub struct AgentTokenResponse {
    pub id: Uuid,
    pub name: String,
    pub scopes: Vec<String>,
    pub document_ids: Option<Vec<Uuid>>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

impl From<AgentTokenRow> for AgentTokenResponse {
    fn from(row: AgentTokenRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            scopes: row.scopes,
            document_ids: row.document_ids,
            expires_at: row.expires_at,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AgentTokenCreatedResponse {
    pub token_id: Uuid,
    pub secret: String,
    pub name: String,
    pub scopes: Vec<String>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAgentTokenRequest {
    pub name: String,
    pub scopes: Vec<String>,
    pub document_ids: Option<Vec<Uuid>>,
    pub expires_in: String,
}

// ── Token generation ──

/// Generate an agent token secret and its SHA-256 hash.
/// Returns `(raw_secret, hex_hash)`.
pub fn generate_agent_token() -> (String, String) {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 32];
    rng.fill(&mut bytes);
    let raw_secret = format!("cadmus_{}", hex::encode(bytes));
    let hash = hash_token(&raw_secret);
    (raw_secret, hash)
}

/// Hash a raw token string with SHA-256, returning hex.
pub fn hash_token(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}

// ── Validation helpers ──

/// Parse an `expires_in` string like "7d", "30d" into a `chrono::Duration`.
pub fn parse_expires_in(s: &str) -> Result<Duration, AppError> {
    let s = s.trim();
    if !s.ends_with('d') {
        return Err(AppError::BadRequest(
            "expires_in must be in the format '<number>d' (e.g. '30d')".into(),
        ));
    }
    let num_str = &s[..s.len() - 1];
    let days: i64 = num_str.parse().map_err(|_| {
        AppError::BadRequest("expires_in must be in the format '<number>d' (e.g. '30d')".into())
    })?;
    if !(1..=365).contains(&days) {
        return Err(AppError::BadRequest(
            "expires_in must be between 1d and 365d".into(),
        ));
    }
    Ok(Duration::days(days))
}

/// Validate that all scopes are in the allowed set.
pub fn validate_scopes(scopes: &[String]) -> Result<(), AppError> {
    if scopes.is_empty() {
        return Err(AppError::BadRequest("At least one scope is required".into()));
    }
    for scope in scopes {
        if !VALID_SCOPES.contains(&scope.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Invalid scope '{}'. Valid scopes: {}",
                scope,
                VALID_SCOPES.join(", ")
            )));
        }
    }
    Ok(())
}

/// Check that a token has the required scope. JWT users (no token_scopes) pass unconditionally.
pub fn require_scope(token_scopes: &Option<Vec<String>>, scope: &str) -> Result<(), AppError> {
    match token_scopes {
        None => Ok(()), // JWT user — all scopes
        Some(scopes) => {
            if scopes.iter().any(|s| s == scope) {
                Ok(())
            } else {
                Err(AppError::Forbidden(format!(
                    "Token missing required scope '{}'",
                    scope
                )))
            }
        }
    }
}

/// Check that the agent token is allowed to access the given document.
/// If the token has no document_ids restriction (None), access is allowed.
pub fn check_document_restriction(
    document_ids: &Option<Vec<Uuid>>,
    doc_id: Uuid,
) -> Result<(), AppError> {
    match document_ids {
        None => Ok(()),
        Some(ids) => {
            if ids.contains(&doc_id) {
                Ok(())
            } else {
                Err(AppError::Forbidden(
                    "Token is not authorized for this document".into(),
                ))
            }
        }
    }
}
