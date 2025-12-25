use async_trait::async_trait;
use crate::domain::entities::Commit;
use crate::shared::result::Result;

/// 提交仓储接口
#[async_trait]
pub trait CommitPort: Send + Sync {
    /// 根据 OID 查找提交
    async fn find_by_oid(&self, repository_id: i64, oid: &str) -> Result<Option<Commit>>;

    /// 获取仓库的提交列表（分页）
    async fn list_by_repository(
        &self,
        repository_id: i64,
        branch: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Commit>>;

    /// 获取仓库某分支的最新提交
    async fn get_latest_commit(
        &self,
        repository_id: i64,
        branch: &str,
    ) -> Result<Option<Commit>>;

    /// 批量插入提交
    async fn bulk_insert(&self, commits: &[Commit]) -> Result<usize>;

    /// 保存单个提交
    async fn save(&self, commit: &Commit) -> Result<i64>;

    /// 删除仓库的所有提交
    async fn delete_by_repository(&self, repository_id: i64) -> Result<()>;

    /// 统计提交数量
    async fn count_by_repository(&self, repository_id: i64, branch: Option<&str>) -> Result<i64>;
}
