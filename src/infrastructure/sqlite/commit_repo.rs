use async_trait::async_trait;
use sqlx::{SqlitePool, Row};
use chrono::DateTime;
use crate::domain::entities::Commit;
use crate::ports::commit::CommitPort;
use crate::shared::result::Result;

/// SQLite 提交仓储实现
pub struct SqliteCommitRepository {
    pool: SqlitePool,
}

impl SqliteCommitRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CommitPort for SqliteCommitRepository {
    async fn find_by_oid(&self, repository_id: i64, oid: &str) -> Result<Option<Commit>> {
        let row = sqlx::query(
            r#"
            SELECT id, repository_id, oid, branch,
                   author_name, author_email, author_time,
                   committer_name, committer_email, committer_time,
                   summary, message, parent_oids, created_at
            FROM commits
            WHERE repository_id = ? AND oid = ?
            LIMIT 1
            "#,
        )
        .bind(repository_id)
        .bind(oid)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Commit {
            id: r.get("id"),
            repository_id: r.get("repository_id"),
            oid: r.get("oid"),
            branch: r.get("branch"),
            author_name: r.get("author_name"),
            author_email: r.get("author_email"),
            author_time: DateTime::from_timestamp(r.get("author_time"), 0).unwrap(),
            committer_name: r.get("committer_name"),
            committer_email: r.get("committer_email"),
            committer_time: DateTime::from_timestamp(r.get("committer_time"), 0).unwrap(),
            summary: r.get("summary"),
            message: r.get("message"),
            parent_oids: r.get("parent_oids"),
            created_at: DateTime::from_timestamp(r.get("created_at"), 0).unwrap(),
        }))
    }

    async fn list_by_repository(
        &self,
        repository_id: i64,
        branch: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Commit>> {
        let rows = if let Some(branch_name) = branch {
            sqlx::query(
                r#"
                SELECT id, repository_id, oid, branch,
                       author_name, author_email, author_time,
                       committer_name, committer_email, committer_time,
                       summary, message, parent_oids, created_at
                FROM commits
                WHERE repository_id = ? AND branch = ?
                ORDER BY author_time DESC
                LIMIT ? OFFSET ?
                "#,
            )
            .bind(repository_id)
            .bind(branch_name)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, repository_id, oid, branch,
                       author_name, author_email, author_time,
                       committer_name, committer_email, committer_time,
                       summary, message, parent_oids, created_at
                FROM commits
                WHERE repository_id = ?
                ORDER BY author_time DESC
                LIMIT ? OFFSET ?
                "#,
            )
            .bind(repository_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| Commit {
                id: r.get("id"),
                repository_id: r.get("repository_id"),
                oid: r.get("oid"),
                branch: r.get("branch"),
                author_name: r.get("author_name"),
                author_email: r.get("author_email"),
                author_time: DateTime::from_timestamp(r.get("author_time"), 0).unwrap(),
                committer_name: r.get("committer_name"),
                committer_email: r.get("committer_email"),
                committer_time: DateTime::from_timestamp(r.get("committer_time"), 0).unwrap(),
                summary: r.get("summary"),
                message: r.get("message"),
                parent_oids: r.get("parent_oids"),
                created_at: DateTime::from_timestamp(r.get("created_at"), 0).unwrap(),
            })
            .collect())
    }

    async fn get_latest_commit(
        &self,
        repository_id: i64,
        branch: &str,
    ) -> Result<Option<Commit>> {
        let row = sqlx::query(
            r#"
            SELECT id, repository_id, oid, branch,
                   author_name, author_email, author_time,
                   committer_name, committer_email, committer_time,
                   summary, message, parent_oids, created_at
            FROM commits
            WHERE repository_id = ? AND branch = ?
            ORDER BY author_time DESC
            LIMIT 1
            "#,
        )
        .bind(repository_id)
        .bind(branch)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Commit {
            id: r.get("id"),
            repository_id: r.get("repository_id"),
            oid: r.get("oid"),
            branch: r.get("branch"),
            author_name: r.get("author_name"),
            author_email: r.get("author_email"),
            author_time: DateTime::from_timestamp(r.get("author_time"), 0).unwrap(),
            committer_name: r.get("committer_name"),
            committer_email: r.get("committer_email"),
            committer_time: DateTime::from_timestamp(r.get("committer_time"), 0).unwrap(),
            summary: r.get("summary"),
            message: r.get("message"),
            parent_oids: r.get("parent_oids"),
            created_at: DateTime::from_timestamp(r.get("created_at"), 0).unwrap(),
        }))
    }

    async fn bulk_insert(&self, commits: &[Commit]) -> Result<usize> {
        let mut tx = self.pool.begin().await?;
        let mut count = 0;

        for commit in commits {
            let author_time_ts = commit.author_time.timestamp();
            let committer_time_ts = commit.committer_time.timestamp();
            let created_ts = commit.created_at.timestamp();

            sqlx::query(
                r#"
                INSERT INTO commits (
                    repository_id, oid, branch,
                    author_name, author_email, author_time,
                    committer_name, committer_email, committer_time,
                    summary, message, parent_oids, created_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(repository_id, oid, branch) DO NOTHING
                "#,
            )
            .bind(commit.repository_id)
            .bind(&commit.oid)
            .bind(&commit.branch)
            .bind(&commit.author_name)
            .bind(&commit.author_email)
            .bind(author_time_ts)
            .bind(&commit.committer_name)
            .bind(&commit.committer_email)
            .bind(committer_time_ts)
            .bind(&commit.summary)
            .bind(&commit.message)
            .bind(&commit.parent_oids)
            .bind(created_ts)
            .execute(&mut *tx)
            .await?;

            count += 1;
        }

        tx.commit().await?;
        Ok(count)
    }

    async fn save(&self, commit: &Commit) -> Result<i64> {
        let author_time_ts = commit.author_time.timestamp();
        let committer_time_ts = commit.committer_time.timestamp();
        let created_ts = commit.created_at.timestamp();

        let result = sqlx::query(
            r#"
            INSERT INTO commits (
                repository_id, oid, branch,
                author_name, author_email, author_time,
                committer_name, committer_email, committer_time,
                summary, message, parent_oids, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(repository_id, oid, branch) DO UPDATE SET
                summary = excluded.summary,
                message = excluded.message
            RETURNING id
            "#,
        )
        .bind(commit.repository_id)
        .bind(&commit.oid)
        .bind(&commit.branch)
        .bind(&commit.author_name)
        .bind(&commit.author_email)
        .bind(author_time_ts)
        .bind(&commit.committer_name)
        .bind(&commit.committer_email)
        .bind(committer_time_ts)
        .bind(&commit.summary)
        .bind(&commit.message)
        .bind(&commit.parent_oids)
        .bind(created_ts)
        .fetch_one(&self.pool)
        .await?;

        Ok(result.get("id"))
    }

    async fn delete_by_repository(&self, repository_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM commits WHERE repository_id = ?")
            .bind(repository_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn count_by_repository(&self, repository_id: i64, branch: Option<&str>) -> Result<i64> {
        let count: i64 = if let Some(branch_name) = branch {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM commits WHERE repository_id = ? AND branch = ?",
            )
            .bind(repository_id)
            .bind(branch_name)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_scalar("SELECT COUNT(*) FROM commits WHERE repository_id = ?")
                .bind(repository_id)
                .fetch_one(&self.pool)
                .await?
        };

        Ok(count)
    }
    
    async fn find_diff_commits(
        &self,
        repository_id: i64,
        old_branch: &str,
        new_branch: &str,
        limit: i64,
    ) -> Result<Vec<Commit>> {
        // 查找在old_branch但不在new_branch的commits（老分支特有的commits）
        // 通过 (author_name, summary, committer_time) 组合来识别相同的逻辑commit
        // 使用 LEFT JOIN + IS NULL 代替 NOT EXISTS，性能更好
        let rows = sqlx::query(
            r#"
            SELECT c.id, c.repository_id, c.oid, c.branch,
                   c.author_name, c.author_email, c.author_time,
                   c.committer_name, c.committer_email, c.committer_time,
                   c.summary, c.message, c.parent_oids, c.created_at
            FROM commits c
            LEFT JOIN commits new ON 
                new.repository_id = c.repository_id
                AND new.branch = ?
                AND new.author_name = c.author_name
                AND new.summary = c.summary
            WHERE c.repository_id = ? 
              AND c.branch = ?
              AND new.id IS NULL
            ORDER BY c.committer_time DESC
            LIMIT ?
            "#,
        )
        .bind(new_branch)
        .bind(repository_id)
        .bind(old_branch)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| Commit {
                id: r.get("id"),
                repository_id: r.get("repository_id"),
                oid: r.get("oid"),
                branch: r.get("branch"),
                author_name: r.get("author_name"),
                author_email: r.get("author_email"),
                author_time: DateTime::from_timestamp(r.get("author_time"), 0).unwrap(),
                committer_name: r.get("committer_name"),
                committer_email: r.get("committer_email"),
                committer_time: DateTime::from_timestamp(r.get("committer_time"), 0).unwrap(),
                summary: r.get("summary"),
                message: r.get("message"),
                parent_oids: r.get("parent_oids"),
                created_at: DateTime::from_timestamp(r.get("created_at"), 0).unwrap(),
            })
            .collect())
    }
}
