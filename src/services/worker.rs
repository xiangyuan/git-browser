use std::path::Path;
use std::sync::Arc;
use chrono::DateTime;
use tracing::{info, debug, error};
use crate::domain::entities::Commit;
use crate::ports::repository::RepositoryPort;
use crate::ports::git::GitPort;
use crate::shared::config::Config;
use crate::shared::result::Result;

/// 索引工作者 - 执行实际的索引工作
pub struct IndexWorker {
    config: Arc<Config>,
    #[allow(dead_code)]  // 后续功能会使用
    repository_store: Arc<dyn RepositoryPort>,
    git_client: Arc<dyn GitPort>,
}

impl IndexWorker {
    pub fn new(
        config: Arc<Config>,
        repository_store: Arc<dyn RepositoryPort>,
        git_client: Arc<dyn GitPort>,
    ) -> Self {
        Self {
            config,
            repository_store,
            git_client,
        }
    }

    /// 索引单个仓库的所有分支
    pub async fn index_repository(&self, repository_id: i64, path: &Path) -> Result<IndexResult> {
        let mut result = IndexResult::default();

        // 获取所有分支
        let branches = self.git_client.list_branches(path).await?;
        
        info!("Found {} branches to index", branches.len());

        for branch in branches {
            // 只索引 remote 分支
            if !branch.name.starts_with("refs/remotes/origin/") {
                continue;
            }

            debug!("Indexing branch: {}", branch.name);

            match self.index_branch(repository_id, path, &branch.name).await {
                Ok(count) => {
                    result.commits_indexed += count;
                    result.branches_indexed += 1;
                }
                Err(e) => {
                    error!("Failed to index branch {}: {}", branch.name, e);
                    result.branches_failed += 1;
                }
            }
        }

        info!(
            "Repository indexing completed: {} commits, {} branches",
            result.commits_indexed,
            result.branches_indexed
        );

        Ok(result)
    }

    /// 索引单个分支（增量更新）
    async fn index_branch(
        &self,
        repository_id: i64,
        path: &Path,
        branch: &str,
    ) -> Result<usize> {
        // 这里需要 CommitPort，但我们还没有注入
        // 暂时返回 0，后续需要添加 commit_store
        
        // TODO: 获取最后索引的提交
        // let last_indexed = self.commit_store.get_latest_commit(repository_id, branch).await?;
        let last_indexed_oid: Option<String> = None; // 暂时全量索引

        // 获取新提交
        let commits = self.git_client.get_commits(
            path,
            branch,
            self.config.indexer.max_commits_per_branch,
            last_indexed_oid.as_deref(),
        ).await?;

        if commits.is_empty() {
            debug!("No new commits for branch {}", branch);
            return Ok(0);
        }

        // 转换为领域实体
        let domain_commits: Vec<Commit> = commits
            .into_iter()
            .map(|c| {
                Commit::new(
                    repository_id,
                    c.oid,
                    branch.to_string(),
                    c.author_name,
                    c.author_email,
                    DateTime::from_timestamp(c.author_time, 0).unwrap(),
                    c.committer_name,
                    c.committer_email,
                    DateTime::from_timestamp(c.committer_time, 0).unwrap(),
                    c.summary,
                )
                .with_message(c.message.unwrap_or_default())
                .with_parents(c.parent_oids)
            })
            .collect();

        let count = domain_commits.len();

        // TODO: 批量插入
        // self.commit_store.bulk_insert(&domain_commits).await?;

        info!("Indexed {} commits for branch {}", count, branch);

        Ok(count)
    }
}

#[derive(Debug, Default)]
pub struct IndexResult {
    pub commits_indexed: usize,
    pub branches_indexed: usize,
    pub branches_failed: usize,
}
