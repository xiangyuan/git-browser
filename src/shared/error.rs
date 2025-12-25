use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// 统一的错误类型
#[derive(Debug, thiserror::Error)]
pub enum GitxError {
    /// Git 操作错误
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    /// SQLx 数据库错误
    #[error("SQLx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    /// IO 错误
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// 序列化错误
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    /// 仓库未找到
    #[error("Repository not found: {0}")]
    RepositoryNotFound(String),

    /// 提交未找到
    #[error("Commit not found: {0}")]
    CommitNotFound(String),

    /// 引用未找到
    #[error("Reference not found: {0}")]
    ReferenceNotFound(String),

    /// 无效的路径
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// 无效的 OID
    #[error("Invalid OID: {0}")]
    InvalidOid(String),

    /// 无效的引用
    #[error("Invalid reference")]
    InvalidRef,

    /// 配置错误
    #[error("Configuration error: {0}")]
    Config(String),

    /// 解析错误
    #[error("Parse error: {0}")]
    Parse(String),

    /// 内部错误
    #[error("Internal error: {0}")]
    Internal(String),

    /// Anyhow 错误兼容
    #[error(transparent)]
    Other(#[from] anyhow::Error),

    /// Template 渲染错误
    #[error("Template error: {0}")]
    Template(#[from] askama::Error),
}

/// 用于 Axum 的错误响应实现
impl IntoResponse for GitxError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            GitxError::RepositoryNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            GitxError::CommitNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            GitxError::ReferenceNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            GitxError::InvalidPath(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            GitxError::InvalidOid(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            GitxError::Config(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            GitxError::Sqlx(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()),
            GitxError::Git(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Git operation failed".to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
        };

        tracing::error!("Request error: {}", self);

        (status, message).into_response()
    }
}

/// 从字符串创建配置错误
impl From<String> for GitxError {
    fn from(s: String) -> Self {
        GitxError::Config(s)
    }
}

/// 从 &str 创建配置错误
impl From<&str> for GitxError {
    fn from(s: &str) -> Self {
        GitxError::Config(s.to_string())
    }
}
