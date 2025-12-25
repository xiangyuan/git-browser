pub mod repository_repo;
pub mod commit_repo;
pub mod branch_repo;

use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::path::Path;
use crate::shared::result::Result;
use crate::shared::error::GitxError;

/// 初始化 SQLite 数据库连接池
pub async fn create_pool(database_path: &Path, max_connections: u32) -> Result<SqlitePool> {
    // 确保数据库文件的父目录存在
    if let Some(parent) = database_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    
    // SQLite连接字符串，添加create_if_missing选项
    let url = format!("sqlite://{}?mode=rwc", database_path.display());
    
    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect(&url)
        .await?;

    Ok(pool)
}

/// 运行数据库迁移
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| GitxError::Internal(format!("Migration failed: {}", e)))?;
    Ok(())
}
