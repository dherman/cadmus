use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Database connection wrapper.
#[derive(Clone)]
pub struct Database {
    pub pool: PgPool,
}

/// A row from the `documents` table.
#[derive(Debug, sqlx::FromRow)]
pub struct DocumentRow {
    pub id: Uuid,
    pub title: String,
    pub schema_version: i32,
    pub snapshot_key: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A document row with the requesting user's role and ownership info.
#[derive(Debug, sqlx::FromRow)]
pub struct DocumentWithRole {
    pub id: Uuid,
    pub title: String,
    pub schema_version: i32,
    pub snapshot_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub role: String,
    pub is_owner: bool,
}

/// A permission row joined with user info.
#[derive(Debug, sqlx::FromRow)]
pub struct PermissionWithUser {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
}

/// A comment row joined with author info.
#[derive(Debug, sqlx::FromRow)]
pub struct CommentWithAuthor {
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
    pub author_display_name: String,
    pub author_email: String,
}

/// A row from the `users` table.
#[derive(Debug, sqlx::FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Database {
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPool::connect(url).await?;
        Ok(Self { pool })
    }

    /// Create a Database that defers connection until the first query.
    /// Useful for tests that don't actually need the database.
    pub fn connect_lazy(url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPool::connect_lazy(url)?;
        Ok(Self { pool })
    }

    // ── User queries ──

    pub async fn create_user(
        &self,
        id: Uuid,
        email: &str,
        display_name: &str,
        password_hash: &str,
    ) -> Result<UserRow, sqlx::Error> {
        sqlx::query_as::<_, UserRow>(
            r#"INSERT INTO users (id, email, display_name, password_hash)
               VALUES ($1, $2, $3, $4)
               RETURNING id, email, display_name, password_hash, created_at, updated_at"#,
        )
        .bind(id)
        .bind(email)
        .bind(display_name)
        .bind(password_hash)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_user_by_id(&self, id: Uuid) -> Result<Option<UserRow>, sqlx::Error> {
        sqlx::query_as::<_, UserRow>(
            "SELECT id, email, display_name, password_hash, created_at, updated_at FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<UserRow>, sqlx::Error> {
        sqlx::query_as::<_, UserRow>(
            "SELECT id, email, display_name, password_hash, created_at, updated_at FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
    }

    // ── Document queries ──

    pub async fn create_document(
        &self,
        id: Uuid,
        title: &str,
        created_by: Option<Uuid>,
    ) -> Result<DocumentRow, sqlx::Error> {
        sqlx::query_as::<_, DocumentRow>(
            r#"INSERT INTO documents (id, title, created_by) VALUES ($1, $2, $3)
               RETURNING id, title, schema_version, snapshot_key, created_by, created_at, updated_at"#,
        )
        .bind(id)
        .bind(title)
        .bind(created_by)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_document(&self, id: Uuid) -> Result<Option<DocumentRow>, sqlx::Error> {
        sqlx::query_as::<_, DocumentRow>(
            "SELECT id, title, schema_version, snapshot_key, created_by, created_at, updated_at FROM documents WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_documents(&self) -> Result<Vec<DocumentRow>, sqlx::Error> {
        sqlx::query_as::<_, DocumentRow>(
            "SELECT id, title, schema_version, snapshot_key, created_by, created_at, updated_at FROM documents ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn delete_document(&self, id: Uuid) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM documents WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn update_document_title(
        &self,
        id: Uuid,
        title: &str,
    ) -> Result<Option<DocumentRow>, sqlx::Error> {
        sqlx::query_as::<_, DocumentRow>(
            r#"UPDATE documents SET title = $2, updated_at = NOW() WHERE id = $1
               RETURNING id, title, schema_version, snapshot_key, created_by, created_at, updated_at"#,
        )
        .bind(id)
        .bind(title)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn update_document_snapshot(
        &self,
        id: Uuid,
        snapshot_key: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE documents SET snapshot_key = $2, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .bind(snapshot_key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn append_update_log(
        &self,
        document_id: Uuid,
        data: &[u8],
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO update_log (document_id, data) VALUES ($1, $2)")
            .bind(document_id)
            .bind(data)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_update_log(&self, document_id: Uuid) -> Result<Vec<Vec<u8>>, sqlx::Error> {
        let rows: Vec<(Vec<u8>,)> =
            sqlx::query_as("SELECT data FROM update_log WHERE document_id = $1 ORDER BY id ASC")
                .bind(document_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    pub async fn clear_update_log(&self, document_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM update_log WHERE document_id = $1")
            .bind(document_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Permission queries ──

    pub async fn create_permission(
        &self,
        document_id: Uuid,
        user_id: Uuid,
        role: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO document_permissions (id, document_id, user_id, role) VALUES ($1, $2, $3, $4)",
        )
        .bind(Uuid::new_v4())
        .bind(document_id)
        .bind(user_id)
        .bind(role)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_user_permission(
        &self,
        document_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT role FROM document_permissions WHERE document_id = $1 AND user_id = $2",
        )
        .bind(document_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn list_permissions_with_users(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<PermissionWithUser>, sqlx::Error> {
        sqlx::query_as::<_, PermissionWithUser>(
            r#"SELECT dp.user_id, u.email, u.display_name, dp.role
               FROM document_permissions dp
               INNER JOIN users u ON u.id = dp.user_id
               WHERE dp.document_id = $1
               ORDER BY dp.created_at ASC"#,
        )
        .bind(document_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn update_permission(
        &self,
        document_id: Uuid,
        user_id: Uuid,
        role: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE document_permissions SET role = $3 WHERE document_id = $1 AND user_id = $2",
        )
        .bind(document_id)
        .bind(user_id)
        .bind(role)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_permission(
        &self,
        document_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result =
            sqlx::query("DELETE FROM document_permissions WHERE document_id = $1 AND user_id = $2")
                .bind(document_id)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list_accessible_documents(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<DocumentRow>, sqlx::Error> {
        sqlx::query_as::<_, DocumentRow>(
            r#"SELECT d.id, d.title, d.schema_version, d.snapshot_key, d.created_by, d.created_at, d.updated_at
               FROM documents d
               INNER JOIN document_permissions dp ON dp.document_id = d.id
               WHERE dp.user_id = $1
               ORDER BY d.updated_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_accessible_documents_with_role(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<DocumentWithRole>, sqlx::Error> {
        sqlx::query_as::<_, DocumentWithRole>(
            r#"SELECT d.id, d.title, d.schema_version, d.snapshot_key,
                      d.created_at, d.updated_at,
                      dp.role,
                      (d.created_by = $1) AS is_owner
               FROM documents d
               INNER JOIN document_permissions dp ON dp.document_id = d.id
               WHERE dp.user_id = $1
               ORDER BY d.updated_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_document_with_role(
        &self,
        document_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<DocumentWithRole>, sqlx::Error> {
        sqlx::query_as::<_, DocumentWithRole>(
            r#"SELECT d.id, d.title, d.schema_version, d.snapshot_key,
                      d.created_at, d.updated_at,
                      dp.role,
                      (d.created_by = $1) AS is_owner
               FROM documents d
               INNER JOIN document_permissions dp ON dp.document_id = d.id
               WHERE d.id = $2 AND dp.user_id = $1"#,
        )
        .bind(user_id)
        .bind(document_id)
        .fetch_optional(&self.pool)
        .await
    }

    // ── Agent token queries ──

    pub async fn create_agent_token(
        &self,
        user_id: Uuid,
        name: &str,
        token_hash: &str,
        scopes: &[String],
        document_ids: Option<&[Uuid]>,
        expires_at: DateTime<Utc>,
    ) -> Result<crate::auth::tokens::AgentTokenRow, sqlx::Error> {
        sqlx::query_as::<_, crate::auth::tokens::AgentTokenRow>(
            r#"INSERT INTO agent_tokens (user_id, name, token_hash, scopes, document_ids, expires_at)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id, user_id, name, token_hash, scopes, document_ids, expires_at, revoked_at, created_at"#,
        )
        .bind(user_id)
        .bind(name)
        .bind(token_hash)
        .bind(scopes)
        .bind(document_ids)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_agent_tokens(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<crate::auth::tokens::AgentTokenRow>, sqlx::Error> {
        sqlx::query_as::<_, crate::auth::tokens::AgentTokenRow>(
            r#"SELECT id, user_id, name, token_hash, scopes, document_ids, expires_at, revoked_at, created_at
               FROM agent_tokens
               WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > NOW()
               ORDER BY created_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_agent_token_by_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<crate::auth::tokens::AgentTokenRow>, sqlx::Error> {
        sqlx::query_as::<_, crate::auth::tokens::AgentTokenRow>(
            r#"SELECT id, user_id, name, token_hash, scopes, document_ids, expires_at, revoked_at, created_at
               FROM agent_tokens
               WHERE token_hash = $1"#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn revoke_agent_token(
        &self,
        token_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE agent_tokens SET revoked_at = NOW() WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL",
        )
        .bind(token_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // ── Comment queries ──

    pub async fn list_comments(
        &self,
        document_id: Uuid,
        status_filter: Option<&str>,
    ) -> Result<Vec<CommentWithAuthor>, sqlx::Error> {
        match status_filter {
            Some(status) => {
                sqlx::query_as::<_, CommentWithAuthor>(
                    r#"SELECT c.id, c.document_id, c.author_id, c.parent_id,
                              c.anchor_start, c.anchor_end, c.body, c.status,
                              c.created_at, c.updated_at,
                              u.display_name AS author_display_name,
                              u.email AS author_email
                       FROM comments c
                       JOIN users u ON c.author_id = u.id
                       WHERE c.document_id = $1 AND c.status = $2
                       ORDER BY c.created_at ASC"#,
                )
                .bind(document_id)
                .bind(status)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query_as::<_, CommentWithAuthor>(
                    r#"SELECT c.id, c.document_id, c.author_id, c.parent_id,
                              c.anchor_start, c.anchor_end, c.body, c.status,
                              c.created_at, c.updated_at,
                              u.display_name AS author_display_name,
                              u.email AS author_email
                       FROM comments c
                       JOIN users u ON c.author_id = u.id
                       WHERE c.document_id = $1
                       ORDER BY c.created_at ASC"#,
                )
                .bind(document_id)
                .fetch_all(&self.pool)
                .await
            }
        }
    }

    pub async fn create_comment(
        &self,
        document_id: Uuid,
        author_id: Uuid,
        body: &str,
        anchor_start: Option<&[u8]>,
        anchor_end: Option<&[u8]>,
    ) -> Result<crate::documents::comments::CommentRow, sqlx::Error> {
        sqlx::query_as::<_, crate::documents::comments::CommentRow>(
            r#"INSERT INTO comments (document_id, author_id, body, anchor_start, anchor_end)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id, document_id, author_id, parent_id, anchor_start, anchor_end,
                         body, status, created_at, updated_at"#,
        )
        .bind(document_id)
        .bind(author_id)
        .bind(body)
        .bind(anchor_start)
        .bind(anchor_end)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn create_reply(
        &self,
        document_id: Uuid,
        author_id: Uuid,
        parent_id: Uuid,
        body: &str,
    ) -> Result<crate::documents::comments::CommentRow, sqlx::Error> {
        sqlx::query_as::<_, crate::documents::comments::CommentRow>(
            r#"INSERT INTO comments (document_id, author_id, parent_id, body)
               VALUES ($1, $2, $3, $4)
               RETURNING id, document_id, author_id, parent_id, anchor_start, anchor_end,
                         body, status, created_at, updated_at"#,
        )
        .bind(document_id)
        .bind(author_id)
        .bind(parent_id)
        .bind(body)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_comment(
        &self,
        comment_id: Uuid,
    ) -> Result<Option<crate::documents::comments::CommentRow>, sqlx::Error> {
        sqlx::query_as::<_, crate::documents::comments::CommentRow>(
            r#"SELECT id, document_id, author_id, parent_id, anchor_start, anchor_end,
                      body, status, created_at, updated_at
               FROM comments WHERE id = $1"#,
        )
        .bind(comment_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn update_comment_body(
        &self,
        comment_id: Uuid,
        body: &str,
    ) -> Result<crate::documents::comments::CommentRow, sqlx::Error> {
        sqlx::query_as::<_, crate::documents::comments::CommentRow>(
            r#"UPDATE comments SET body = $2, updated_at = NOW() WHERE id = $1
               RETURNING id, document_id, author_id, parent_id, anchor_start, anchor_end,
                         body, status, created_at, updated_at"#,
        )
        .bind(comment_id)
        .bind(body)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn update_comment_status(
        &self,
        comment_id: Uuid,
        status: &str,
    ) -> Result<crate::documents::comments::CommentRow, sqlx::Error> {
        sqlx::query_as::<_, crate::documents::comments::CommentRow>(
            r#"UPDATE comments SET status = $2, updated_at = NOW() WHERE id = $1
               RETURNING id, document_id, author_id, parent_id, anchor_start, anchor_end,
                         body, status, created_at, updated_at"#,
        )
        .bind(comment_id)
        .bind(status)
        .fetch_one(&self.pool)
        .await
    }
}
