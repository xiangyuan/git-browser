use async_trait::async_trait;
use crate::domain::entities::Branch;
use crate::shared::result::Result;

#[async_trait]
pub trait BranchPort: Send + Sync {
    /// 保存分支信息
    async fn save(&self, branch: &Branch) -> Result<()>;
    
    /// 保存多个分支
    async fn save_many(&self, branches: &[Branch]) -> Result<()>;
    
    /// 根据仓库ID查询所有分支
    async fn find_by_repository(&self, repository_id: i64) -> Result<Vec<Branch>>;
    
    /// 删除仓库的所有分支
    async fn delete_by_repository(&self, repository_id: i64) -> Result<()>;
}
