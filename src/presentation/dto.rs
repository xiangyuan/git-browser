use serde::{Deserialize, Serialize};
use crate::domain::entities::{Repository, Commit};

/// 仓库 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryDto {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub default_branch: String,
    pub last_synced_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Repository> for RepositoryDto {
    fn from(repo: Repository) -> Self {
        Self {
            id: repo.id,
            name: repo.name,
            path: repo.path,
            description: repo.description,
            default_branch: repo.default_branch,
            last_synced_at: repo.last_synced_at.map(|dt| dt.to_rfc3339()),
            created_at: repo.created_at.to_rfc3339(),
            updated_at: repo.updated_at.to_rfc3339(),
        }
    }
}

/// 提交 DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitDto {
    pub id: i64,
    pub repository_id: i64,
    pub oid: String,
    pub branch: String,
    pub author_name: String,
    pub author_email: String,
    pub author_time: String,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_time: String,
    pub summary: String,
    pub message: Option<String>,
    pub created_at: String,
}

impl From<Commit> for CommitDto {
    fn from(commit: Commit) -> Self {
        Self {
            id: commit.id,
            repository_id: commit.repository_id,
            oid: commit.oid,
            branch: commit.branch,
            author_name: commit.author_name,
            author_email: commit.author_email,
            author_time: commit.author_time.to_rfc3339(),
            committer_name: commit.committer_name,
            committer_email: commit.committer_email,
            committer_time: commit.committer_time.to_rfc3339(),
            summary: commit.summary,
            message: commit.message,
            created_at: commit.created_at.to_rfc3339(),
        }
    }
}
