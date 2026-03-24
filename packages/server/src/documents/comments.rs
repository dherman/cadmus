use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CommentRow {
    pub id: Uuid,
    pub document_id: Uuid,
    pub author_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub anchor_start: Option<Vec<u8>>,
    pub anchor_end: Option<Vec<u8>>,
    pub body: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CommentResponse {
    pub id: Uuid,
    pub document_id: Uuid,
    pub author: CommentAuthor,
    pub parent_id: Option<Uuid>,
    pub body: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CommentAuthor {
    pub id: Uuid,
    pub display_name: String,
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateCommentRequest {
    pub body: String,
    pub anchor_from: Option<u64>,
    pub anchor_to: Option<u64>,
    pub base_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReplyRequest {
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct EditCommentRequest {
    pub body: String,
}
