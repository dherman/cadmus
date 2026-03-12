use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Database connection wrapper.
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

    pub async fn create_document(&self, id: Uuid, title: &str) -> Result<DocumentRow, sqlx::Error> {
        sqlx::query_as::<_, DocumentRow>(
            r#"INSERT INTO documents (id, title) VALUES ($1, $2)
               RETURNING id, title, schema_version, snapshot_key, created_at, updated_at"#,
        )
        .bind(id)
        .bind(title)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get_document(&self, id: Uuid) -> Result<Option<DocumentRow>, sqlx::Error> {
        sqlx::query_as::<_, DocumentRow>(
            "SELECT id, title, schema_version, snapshot_key, created_at, updated_at FROM documents WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_documents(&self) -> Result<Vec<DocumentRow>, sqlx::Error> {
        sqlx::query_as::<_, DocumentRow>(
            "SELECT id, title, schema_version, snapshot_key, created_at, updated_at FROM documents ORDER BY updated_at DESC",
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
}
