use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{errors::AppError, AppState};

#[derive(Serialize)]
pub struct DocumentSummary {
    pub id: Uuid,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
pub struct CreateDocumentRequest {
    pub title: String,
    pub content: Option<String>,
}

#[derive(Deserialize)]
pub struct ContentQuery {
    pub format: Option<String>,    // "markdown" or "json"
    pub version: Option<String>,
}

#[derive(Deserialize)]
pub struct PushContentRequest {
    pub base_version: String,
    pub format: String,
    pub content: String,
}

// --- Handler stubs ---

pub async fn list_documents(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<Vec<DocumentSummary>>, AppError> {
    // TODO: Query database for documents accessible to the authenticated user
    Ok(Json(vec![]))
}

pub async fn create_document(
    State(_state): State<Arc<AppState>>,
    Json(_body): Json<CreateDocumentRequest>,
) -> Result<Json<DocumentSummary>, AppError> {
    // TODO: Create document in database, optionally parse initial markdown content
    Err(AppError::Internal("Not yet implemented".to_string()))
}

pub async fn get_document(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<Uuid>,
) -> Result<Json<DocumentSummary>, AppError> {
    // TODO: Fetch document metadata
    Err(AppError::NotFound("Document not found".to_string()))
}

pub async fn get_content(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<Uuid>,
    Query(_query): Query<ContentQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    // TODO: Load document from session manager, serialize via sidecar if markdown requested
    Err(AppError::Internal("Not yet implemented".to_string()))
}

pub async fn push_content(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<Uuid>,
    Json(_body): Json<PushContentRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // TODO: Parse pushed markdown via sidecar, diff against base version,
    // translate Steps to Yrs operations, apply to live document
    Err(AppError::Internal("Not yet implemented".to_string()))
}

pub async fn list_comments(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<Uuid>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    // TODO: Query comments for this document
    Ok(Json(vec![]))
}

pub async fn create_comment(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<Uuid>,
    Json(_body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    // TODO: Create comment, convert offsets to RelativePositions, broadcast event
    Err(AppError::Internal("Not yet implemented".to_string()))
}
