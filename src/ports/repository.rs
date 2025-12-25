use async_trait::async_trait;
use crate::domain::entities::Repository;
use crate::shared::result::Result;

/// 仓库仓储接口（Repository Pattern）
#[async_trait]
pub trait RepositoryPort: Send + Sync {
    /// 根据 ID 查找仓库
    async fn find_by_id(&self, id: i64) -> Result<Option<Repository>>;

    /// 根据路径查找仓库
    async fn find_by_path(&self, path: &str) -> Result<Option<Repository>>;

    /// 根据名称查找仓库
    async fn find_by_name(&self, name: &str) -> Result<Option<Repository>>;

    /// 列出所有仓库
    async fn list_all(&self) -> Result<Vec<Repository>>;

    /// 保存仓库（插入或更新）
    async fn save(&self, repo: &Repository) -> Result<i64>;

    /// 删除仓库
    async fn delete(&self, id: i64) -> Result<()>;

    /// 更新同步时间
    async fn update_sync_time(&self, id: i64) -> Result<()>;

    /// 检查路径是否存在
    async fn exists_by_path(&self, path: &str) -> Result<bool>;
}
