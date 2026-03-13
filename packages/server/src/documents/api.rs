use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::middleware::AuthUser;
use crate::{db::DocumentRow, errors::AppError, AppState};

#[derive(Serialize)]
pub struct DocumentSummary {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<DocumentRow> for DocumentSummary {
    fn from(row: DocumentRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Deserialize)]
pub struct CreateDocumentRequest {
    pub title: String,
    pub content: Option<String>,
}

#[derive(Deserialize)]
pub struct ContentQuery {
    pub format: Option<String>, // "markdown" or "json"
    pub version: Option<String>,
}

#[derive(Deserialize)]
pub struct PushContentRequest {
    pub base_version: String,
    pub format: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct UpdateDocumentRequest {
    pub title: Option<String>,
}

// --- Handlers ---

pub async fn list_documents(
    _auth: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DocumentSummary>>, AppError> {
    let rows = state
        .db
        .list_documents()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let docs: Vec<DocumentSummary> = rows.into_iter().map(Into::into).collect();
    Ok(Json(docs))
}

pub async fn create_document(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateDocumentRequest>,
) -> Result<(StatusCode, Json<DocumentSummary>), AppError> {
    if body.title.trim().is_empty() {
        return Err(AppError::BadRequest("Title is required".to_string()));
    }

    let id = Uuid::new_v4();
    let row = state
        .db
        .create_document(id, body.title.trim(), Some(auth.user_id))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Auto-grant Edit permission to the creator
    state
        .db
        .create_permission(id, auth.user_id, "edit")
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(row.into())))
}

pub async fn get_document(
    _auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<DocumentSummary>, AppError> {
    let row = state
        .db
        .get_document(id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Document not found".to_string()))?;

    Ok(Json(row.into()))
}

pub async fn delete_document(
    _auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let doc = state
        .db
        .get_document(id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Document not found".to_string()))?;

    // Unload from memory if active
    state.document_sessions.unload(id);

    // Delete S3 snapshot if exists
    if doc.snapshot_key.is_some() {
        state
            .storage
            .delete_snapshot(id)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
    }

    // Delete from database (cascades to update_log and permissions)
    state
        .db
        .delete_document(id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn update_document(
    _auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateDocumentRequest>,
) -> Result<Json<DocumentSummary>, AppError> {
    let title = body
        .title
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("Title is required".to_string()))?;

    let row = state
        .db
        .update_document_title(id, title.trim())
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Document not found".to_string()))?;

    Ok(Json(row.into()))
}

pub async fn get_content(
    _auth: AuthUser,
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<Uuid>,
    Query(_query): Query<ContentQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    // TODO: Load document from session manager, serialize via sidecar if markdown requested
    Err(AppError::Internal("Not yet implemented".to_string()))
}

pub async fn push_content(
    _auth: AuthUser,
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<Uuid>,
    Json(_body): Json<PushContentRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // TODO: Parse pushed markdown via sidecar, diff against base version,
    // translate Steps to Yrs operations, apply to live document
    Err(AppError::Internal("Not yet implemented".to_string()))
}

pub async fn list_comments(
    _auth: AuthUser,
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<Uuid>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    // TODO: Query comments for this document
    Ok(Json(vec![]))
}

pub async fn create_comment(
    _auth: AuthUser,
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<Uuid>,
    Json(_body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    // TODO: Create comment, convert offsets to RelativePositions, broadcast event
    Err(AppError::Internal("Not yet implemented".to_string()))
}
