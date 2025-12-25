use async_trait::async_trait;
use sqlx::{SqlitePool, Row};
use chrono::DateTime;
use crate::domain::entities::Branch;
use crate::ports::branch::BranchPort;
use crate::shared::result::Result;

/// SQLite 分支仓储实现
pub struct SqliteBranchRepository {
    pool: SqlitePool,
}

impl SqliteBranchRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BranchPort for SqliteBranchRepository {
    async fn save(&self, branch: &Branch) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO branches (repository_id, name, target_oid, is_default, updated_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(repository_id, name) 
            DO UPDATE SET
                target_oid = excluded.target_oid,
                is_default = excluded.is_default,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(branch.repository_id)
        .bind(&branch.name)
        .bind(&branch.target_oid)
        .bind(branch.is_default)
        .bind(branch.updated_at.timestamp())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn save_many(&self, branches: &[Branch]) -> Result<()> {
        if branches.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;

        for branch in branches {
            sqlx::query(
                r#"
                INSERT INTO branches (repository_id, name, target_oid, is_default, updated_at)
                VALUES (?, ?, ?, ?, ?)
                ON CONFLICT(repository_id, name) 
                DO UPDATE SET
                    target_oid = excluded.target_oid,
                    is_default = excluded.is_default,
                    updated_at = excluded.updated_at
                "#,
            )
            .bind(branch.repository_id)
            .bind(&branch.name)
            .bind(&branch.target_oid)
            .bind(branch.is_default)
            .bind(branch.updated_at.timestamp())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn find_by_repository(&self, repository_id: i64) -> Result<Vec<Branch>> {
        let rows = sqlx::query(
            r#"
            SELECT id, repository_id, name, target_oid, is_default, updated_at
            FROM branches
            WHERE repository_id = ?
            ORDER BY is_default DESC, name ASC
            "#,
        )
        .bind(repository_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| Branch {
                id: r.get("id"),
                repository_id: r.get("repository_id"),
                name: r.get("name"),
                target_oid: r.get("target_oid"),
                is_default: r.get("is_default"),
                updated_at: DateTime::from_timestamp(r.get("updated_at"), 0).unwrap(),
            })
            .collect())
    }

    async fn delete_by_repository(&self, repository_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM branches WHERE repository_id = ?")
            .bind(repository_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
