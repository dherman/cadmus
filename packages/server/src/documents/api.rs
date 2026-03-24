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
use crate::db::{CommentWithAuthor, DocumentRow, DocumentWithRole};
use crate::documents::permissions::{require_owner, require_permission, Permission};
use crate::errors::AppError;
use crate::AppState;

#[derive(Serialize)]
pub struct DocumentSummary {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub role: String,
    pub is_owner: bool,
}

impl From<DocumentWithRole> for DocumentSummary {
    fn from(row: DocumentWithRole) -> Self {
        Self {
            id: row.id,
            title: row.title,
            created_at: row.created_at,
            updated_at: row.updated_at,
            role: row.role,
            is_owner: row.is_owner,
        }
    }
}

impl From<DocumentRow> for DocumentSummary {
    fn from(row: DocumentRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            created_at: row.created_at,
            updated_at: row.updated_at,
            role: "edit".to_string(),
            is_owner: true,
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
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<DocumentSummary>>, AppError> {
    let rows = state
        .db
        .list_accessible_documents_with_role(auth.user_id)
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
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<DocumentSummary>, AppError> {
    let row = state
        .db
        .get_document_with_role(id, auth.user_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Forbidden("You don't have access to this document".to_string()))?;

    Ok(Json(row.into()))
}

pub async fn delete_document(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    require_owner(&state.db, auth.user_id, id).await?;

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
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateDocumentRequest>,
) -> Result<Json<DocumentSummary>, AppError> {
    require_permission(&state.db, auth.user_id, id, Permission::Edit).await?;
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
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Query(query): Query<ContentQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_permission(&state.db, auth.user_id, id, Permission::Read).await?;

    let format = query.format.as_deref().unwrap_or("json");

    if format != "json" && format != "markdown" {
        return Err(AppError::BadRequest(
            "Invalid format: must be 'json' or 'markdown'".to_string(),
        ));
    }

    // Load or get the document session
    let session = state
        .document_sessions
        .get_or_load(id, &state.db, &state.storage)
        .await?;

    // Extract ProseMirror JSON from the Yrs document
    let doc_json = {
        let awareness = session.awareness.read().await;
        let yrs_doc = awareness.doc();
        super::yrs_json::extract_prosemirror_json(yrs_doc)
            .unwrap_or_else(|_| super::yrs_json::empty_doc_json())
    };

    match format {
        "markdown" => {
            let markdown = state.sidecar.serialize(doc_json, 1).await.map_err(|e| {
                AppError::BadGateway(format!("Markdown conversion service error: {}", e))
            })?;

            Ok(Json(serde_json::json!({
                "format": "markdown",
                "content": markdown
            })))
        }
        _ => Ok(Json(serde_json::json!({
            "format": "json",
            "content": doc_json
        }))),
    }
}

pub async fn push_content(
    auth: AuthUser,
    State(_state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(_body): Json<PushContentRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    require_permission(&_state.db, auth.user_id, id, Permission::Edit).await?;
    // TODO: Parse pushed markdown via sidecar, diff against base version,
    // translate Steps to Yrs operations, apply to live document
    Err(AppError::Internal("Not yet implemented".to_string()))
}

// --- Comment types ---

use super::comments::{
    CommentAuthor, CommentResponse, CreateCommentRequest, CreateReplyRequest, EditCommentRequest,
};

#[derive(Deserialize)]
pub struct CommentListQuery {
    pub status: Option<String>,
}

fn comment_with_author_to_response(row: CommentWithAuthor) -> CommentResponse {
    CommentResponse {
        id: row.id,
        document_id: row.document_id,
        author: CommentAuthor {
            id: row.author_id,
            display_name: row.author_display_name,
            email: row.author_email,
        },
        parent_id: row.parent_id,
        body: row.body,
        status: row.status,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

async fn get_comment_response(
    db: &crate::db::Database,
    comment: &super::comments::CommentRow,
) -> Result<CommentResponse, AppError> {
    let user = db
        .get_user_by_id(comment.author_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Internal("Comment author not found".to_string()))?;
    Ok(CommentResponse {
        id: comment.id,
        document_id: comment.document_id,
        author: CommentAuthor {
            id: user.id,
            display_name: user.display_name,
            email: user.email,
        },
        parent_id: comment.parent_id,
        body: comment.body.clone(),
        status: comment.status.clone(),
        created_at: comment.created_at,
        updated_at: comment.updated_at,
    })
}

// --- Comment handlers ---

pub async fn list_comments(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Query(query): Query<CommentListQuery>,
) -> Result<Json<Vec<CommentResponse>>, AppError> {
    require_permission(&state.db, auth.user_id, id, Permission::Read).await?;

    let status_filter = match query.status.as_deref() {
        Some("all") | None => None,
        Some("open") => Some("open"),
        Some("resolved") => Some("resolved"),
        Some(_) => return Err(AppError::BadRequest("Invalid status filter".to_string())),
    };

    let rows = state
        .db
        .list_comments(id, status_filter)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let comments: Vec<CommentResponse> = rows.into_iter().map(comment_with_author_to_response).collect();
    Ok(Json(comments))
}

pub async fn create_comment(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateCommentRequest>,
) -> Result<(StatusCode, Json<CommentResponse>), AppError> {
    require_permission(&state.db, auth.user_id, id, Permission::Comment).await?;

    if body.body.trim().is_empty() {
        return Err(AppError::BadRequest("Comment body cannot be empty".to_string()));
    }

    // Anchor conversion deferred to PR 2 — store NULL anchors for now
    let comment = state
        .db
        .create_comment(id, auth.user_id, body.body.trim(), None, None)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let response = get_comment_response(&state.db, &comment).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn reply_to_comment(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((doc_id, comment_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<CreateReplyRequest>,
) -> Result<(StatusCode, Json<CommentResponse>), AppError> {
    require_permission(&state.db, auth.user_id, doc_id, Permission::Comment).await?;

    if body.body.trim().is_empty() {
        return Err(AppError::BadRequest("Reply body cannot be empty".to_string()));
    }

    let parent = state
        .db
        .get_comment(comment_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Comment not found".to_string()))?;

    if parent.document_id != doc_id {
        return Err(AppError::NotFound("Comment not found".to_string()));
    }

    if parent.parent_id.is_some() {
        return Err(AppError::BadRequest(
            "Cannot reply to a reply — only top-level comments can have replies".to_string(),
        ));
    }

    let reply = state
        .db
        .create_reply(doc_id, auth.user_id, comment_id, body.body.trim())
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let response = get_comment_response(&state.db, &reply).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn edit_comment(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((doc_id, comment_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<EditCommentRequest>,
) -> Result<Json<CommentResponse>, AppError> {
    require_permission(&state.db, auth.user_id, doc_id, Permission::Comment).await?;

    if body.body.trim().is_empty() {
        return Err(AppError::BadRequest("Comment body cannot be empty".to_string()));
    }

    let comment = state
        .db
        .get_comment(comment_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Comment not found".to_string()))?;

    if comment.document_id != doc_id {
        return Err(AppError::NotFound("Comment not found".to_string()));
    }

    if comment.author_id != auth.user_id {
        return Err(AppError::Forbidden(
            "Only the comment author can edit this comment".to_string(),
        ));
    }

    let updated = state
        .db
        .update_comment_body(comment_id, body.body.trim())
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let response = get_comment_response(&state.db, &updated).await?;
    Ok(Json(response))
}

pub async fn resolve_comment(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((doc_id, comment_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<CommentResponse>, AppError> {
    require_permission(&state.db, auth.user_id, doc_id, Permission::Comment).await?;

    let comment = state
        .db
        .get_comment(comment_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Comment not found".to_string()))?;

    if comment.document_id != doc_id {
        return Err(AppError::NotFound("Comment not found".to_string()));
    }

    if comment.parent_id.is_some() {
        return Err(AppError::BadRequest(
            "Only top-level comments can be resolved".to_string(),
        ));
    }

    let updated = state
        .db
        .update_comment_status(comment_id, "resolved")
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let response = get_comment_response(&state.db, &updated).await?;
    Ok(Json(response))
}

pub async fn unresolve_comment(
    auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((doc_id, comment_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<CommentResponse>, AppError> {
    require_permission(&state.db, auth.user_id, doc_id, Permission::Comment).await?;

    let comment = state
        .db
        .get_comment(comment_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("Comment not found".to_string()))?;

    if comment.document_id != doc_id {
        return Err(AppError::NotFound("Comment not found".to_string()));
    }

    if comment.parent_id.is_some() {
        return Err(AppError::BadRequest(
            "Only top-level comments can be unresolved".to_string(),
        ));
    }

    let updated = state
        .db
        .update_comment_status(comment_id, "open")
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let response = get_comment_response(&state.db, &updated).await?;
    Ok(Json(response))
}
