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
        let rows: Vec<(Vec<u8>,)> = sqlx::query_as(
            "SELECT data FROM update_log WHERE document_id = $1 ORDER BY id ASC",
        )
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
        let result = sqlx::query(
            "DELETE FROM document_permissions WHERE document_id = $1 AND user_id = $2",
        )
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
}
