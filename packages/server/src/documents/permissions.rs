use std::cmp::Ordering;
use std::sync::Arc;

use axum::extract::{Json, Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::db::Database;
use crate::errors::AppError;
use crate::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Read,
    Comment,
    Edit,
}

impl Permission {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "read" => Some(Permission::Read),
            "comment" => Some(Permission::Comment),
            "edit" => Some(Permission::Edit),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Permission::Read => "read",
            Permission::Comment => "comment",
            Permission::Edit => "edit",
        }
    }
}

impl PartialOrd for Permission {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Permission {
    fn cmp(&self, other: &Self) -> Ordering {
        let rank = |p: &Permission| match p {
            Permission::Read => 0,
            Permission::Comment => 1,
            Permission::Edit => 2,
        };
        rank(self).cmp(&rank(other))
    }
}

// --- Permission check helpers ---

pub async fn require_permission(
    db: &Database,
    user_id: Uuid,
    document_id: Uuid,
    required: Permission,
) -> Result<Permission, AppError> {
    let role_str = db
        .get_user_permission(document_id, user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Forbidden("You don't have access to this document".into()))?;

    let permission = Permission::from_str(&role_str)
        .ok_or_else(|| AppError::Internal(format!("Invalid role in database: {}", role_str)))?;

    if permission < required {
        return Err(AppError::Forbidden("Insufficient permissions".into()));
    }

    Ok(permission)
}

pub async fn require_owner(
    db: &Database,
    user_id: Uuid,
    document_id: Uuid,
) -> Result<(), AppError> {
    let doc = db
        .get_document(document_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Document not found".into()))?;

    if doc.created_by != Some(user_id) {
        return Err(AppError::Forbidden(
            "Only the document owner can perform this action".into(),
        ));
    }

    Ok(())
}

// --- Sharing endpoint types ---

#[derive(Serialize)]
pub struct PermissionEntry {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub is_owner: bool,
}

#[derive(Deserialize)]
pub struct AddPermissionRequest {
    pub email: String,
    pub role: String,
}

#[derive(Deserialize)]
pub struct UpdatePermissionRequest {
    pub role: String,
}

// --- Sharing endpoint handlers ---

pub async fn list_permissions(
    auth: AuthUser,
    Path(doc_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<PermissionEntry>>, AppError> {
    require_permission(&state.db, auth.user_id, doc_id, Permission::Edit).await?;

    let doc = state
        .db
        .get_document(doc_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Document not found".into()))?;

    let perms = state
        .db
        .list_permissions_with_users(doc_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let entries: Vec<PermissionEntry> = perms
        .into_iter()
        .map(|p| PermissionEntry {
            is_owner: doc.created_by == Some(p.user_id),
            user_id: p.user_id,
            email: p.email,
            display_name: p.display_name,
            role: p.role,
        })
        .collect();

    Ok(Json(entries))
}

pub async fn add_permission(
    auth: AuthUser,
    Path(doc_id): Path<Uuid>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddPermissionRequest>,
) -> Result<StatusCode, AppError> {
    require_owner(&state.db, auth.user_id, doc_id).await?;

    let email = body.email.to_lowercase().trim().to_string();
    let user = state
        .db
        .get_user_by_email(&email)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    // Validate role
    Permission::from_str(&body.role).ok_or_else(|| AppError::BadRequest("Invalid role".into()))?;

    // Check if permission already exists
    if state
        .db
        .get_user_permission(doc_id, user.id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .is_some()
    {
        return Err(AppError::Conflict("User already has access".into()));
    }

    state
        .db
        .create_permission(doc_id, user.id, &body.role)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(StatusCode::CREATED)
}

pub async fn update_permission_handler(
    auth: AuthUser,
    Path((doc_id, target_user_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdatePermissionRequest>,
) -> Result<StatusCode, AppError> {
    require_owner(&state.db, auth.user_id, doc_id).await?;

    if target_user_id == auth.user_id {
        return Err(AppError::BadRequest(
            "Cannot change your own permissions".into(),
        ));
    }

    // Validate role
    Permission::from_str(&body.role).ok_or_else(|| AppError::BadRequest("Invalid role".into()))?;

    let updated = state
        .db
        .update_permission(doc_id, target_user_id, &body.role)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if !updated {
        return Err(AppError::NotFound("Permission not found".into()));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_permission_handler(
    auth: AuthUser,
    Path((doc_id, target_user_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<AppState>>,
) -> Result<StatusCode, AppError> {
    require_owner(&state.db, auth.user_id, doc_id).await?;

    if target_user_id == auth.user_id {
        return Err(AppError::BadRequest("Cannot remove your own access".into()));
    }

    let deleted = state
        .db
        .delete_permission(doc_id, target_user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if !deleted {
        return Err(AppError::NotFound("Permission not found".into()));
    }

    Ok(StatusCode::NO_CONTENT)
}
