use crate::shared::error::GitxError;

/// 统一的 Result 类型别名
pub type Result<T> = std::result::Result<T, GitxError>;
