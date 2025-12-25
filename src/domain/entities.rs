use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 仓库实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub default_branch: String,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Repository {
    pub fn new(name: String, path: String) -> Self {
        let now = Utc::now();
        Self {
            id: 0, // 将由数据库生成
            name,
            path,
            description: None,
            default_branch: "main".to_string(),
            last_synced_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn update_sync_time(&mut self) {
        self.last_synced_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }
}

/// 提交实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub id: i64,
    pub repository_id: i64,
    pub oid: String,
    pub branch: String,
    pub author_name: String,
    pub author_email: String,
    pub author_time: DateTime<Utc>,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_time: DateTime<Utc>,
    pub summary: String,
    pub message: Option<String>,
    pub parent_oids: Option<String>, // JSON array
    pub created_at: DateTime<Utc>,
}

impl Commit {
    pub fn new(
        repository_id: i64,
        oid: String,
        branch: String,
        author_name: String,
        author_email: String,
        author_time: DateTime<Utc>,
        committer_name: String,
        committer_email: String,
        committer_time: DateTime<Utc>,
        summary: String,
    ) -> Self {
        Self {
            id: 0,
            repository_id,
            oid,
            branch,
            author_name,
            author_email,
            author_time,
            committer_name,
            committer_email,
            committer_time,
            summary,
            message: None,
            parent_oids: None,
            created_at: Utc::now(),
        }
    }

    pub fn with_message(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }

    pub fn with_parents(mut self, parents: Vec<String>) -> Self {
        // TODO: 需要添加 serde_json 依赖
        self.parent_oids = Some(parents.join(","));
        self
    }
}

/// 标签实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: i64,
    pub repository_id: i64,
    pub name: String,
    pub target_oid: String,
    pub tagger_name: Option<String>,
    pub tagger_email: Option<String>,
    pub tagger_time: Option<DateTime<Utc>>,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// 分支实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub id: i64,
    pub repository_id: i64,
    pub name: String,
    pub target_oid: String,
    pub is_default: bool,
    pub updated_at: DateTime<Utc>,
}
