use async_trait::async_trait;
use sqlx::{SqlitePool, Row};
use chrono::{DateTime, Utc};
use crate::domain::entities::Repository;
use crate::ports::repository::RepositoryPort;
use crate::shared::result::Result;

/// SQLite 仓库仓储实现
pub struct SqliteRepositoryRepository {
    pool: SqlitePool,
}

impl SqliteRepositoryRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RepositoryPort for SqliteRepositoryRepository {
    async fn find_by_id(&self, id: i64) -> Result<Option<Repository>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, path, description, default_branch,
                   last_synced_at, created_at, updated_at
            FROM repositories
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Repository {
            id: r.get("id"),
            name: r.get("name"),
            path: r.get("path"),
            description: r.get("description"),
            default_branch: r.get("default_branch"),
            last_synced_at: r.get::<Option<i64>, _>("last_synced_at")
                .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
            created_at: DateTime::from_timestamp(r.get("created_at"), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.get("updated_at"), 0).unwrap(),
        }))
    }

    async fn find_by_path(&self, path: &str) -> Result<Option<Repository>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, path, description, default_branch,
                   last_synced_at, created_at, updated_at
            FROM repositories
            WHERE path = ?
            "#,
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Repository {
            id: r.get("id"),
            name: r.get("name"),
            path: r.get("path"),
            description: r.get("description"),
            default_branch: r.get("default_branch"),
            last_synced_at: r.get::<Option<i64>, _>("last_synced_at")
                .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
            created_at: DateTime::from_timestamp(r.get("created_at"), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.get("updated_at"), 0).unwrap(),
        }))
    }

    async fn find_by_name(&self, name: &str) -> Result<Option<Repository>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, path, description, default_branch,
                   last_synced_at, created_at, updated_at
            FROM repositories
            WHERE name = ?
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Repository {
            id: r.get("id"),
            name: r.get("name"),
            path: r.get("path"),
            description: r.get("description"),
            default_branch: r.get("default_branch"),
            last_synced_at: r.get::<Option<i64>, _>("last_synced_at")
                .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
            created_at: DateTime::from_timestamp(r.get("created_at"), 0).unwrap(),
            updated_at: DateTime::from_timestamp(r.get("updated_at"), 0).unwrap(),
        }))
    }

    async fn list_all(&self) -> Result<Vec<Repository>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, path, description, default_branch,
                   last_synced_at, created_at, updated_at
            FROM repositories
            ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| Repository {
                id: r.get("id"),
                name: r.get("name"),
                path: r.get("path"),
                description: r.get("description"),
                default_branch: r.get("default_branch"),
                last_synced_at: r.get::<Option<i64>, _>("last_synced_at")
                    .map(|ts| DateTime::from_timestamp(ts, 0).unwrap()),
                created_at: DateTime::from_timestamp(r.get("created_at"), 0).unwrap(),
                updated_at: DateTime::from_timestamp(r.get("updated_at"), 0).unwrap(),
            })
            .collect())
    }

    async fn save(&self, repo: &Repository) -> Result<i64> {
        let created_ts = repo.created_at.timestamp();
        let updated_ts = repo.updated_at.timestamp();
        let last_synced_ts = repo.last_synced_at.map(|dt| dt.timestamp());

        let result = sqlx::query(
            r#"
            INSERT INTO repositories (name, path, description, default_branch, last_synced_at, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(path) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                default_branch = excluded.default_branch,
                last_synced_at = excluded.last_synced_at,
                updated_at = excluded.updated_at
            RETURNING id
            "#,
        )
        .bind(&repo.name)
        .bind(&repo.path)
        .bind(&repo.description)
        .bind(&repo.default_branch)
        .bind(last_synced_ts)
        .bind(created_ts)
        .bind(updated_ts)
        .fetch_one(&self.pool)
        .await?;

        Ok(result.get("id"))
    }

    async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM repositories WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn update_sync_time(&self, id: i64) -> Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query("UPDATE repositories SET last_synced_at = ?, updated_at = ? WHERE id = ?")
            .bind(now)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn exists_by_path(&self, path: &str) -> Result<bool> {
        let row = sqlx::query("SELECT 1 FROM repositories WHERE path = ?")
            .bind(path)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }
}
