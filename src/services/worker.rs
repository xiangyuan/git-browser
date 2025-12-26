use std::path::Path;
use std::sync::Arc;
use chrono::DateTime;
use tracing::{info, debug, error};
use crate::domain::entities::{Commit, Branch};
use crate::ports::repository::RepositoryPort;
use crate::ports::commit::CommitPort;
use crate::ports::branch::BranchPort;
use crate::ports::git::GitPort;
use crate::shared::config::Config;
use crate::shared::result::Result;

/// 索引工作者 - 执行实际的索引工作
pub struct IndexWorker {
    config: Arc<Config>,
    #[allow(dead_code)]  // 后续功能会使用
    repository_store: Arc<dyn RepositoryPort>,
    commit_store: Arc<dyn CommitPort>,
    branch_store: Arc<dyn BranchPort>,
    git_client: Arc<dyn GitPort>,
}

impl IndexWorker {
    pub fn new(
        config: Arc<Config>,
        repository_store: Arc<dyn RepositoryPort>,
        commit_store: Arc<dyn CommitPort>,
        branch_store: Arc<dyn BranchPort>,
        git_client: Arc<dyn GitPort>,
    ) -> Self {
        Self {
            config,
            repository_store,
            commit_store,
            branch_store,
            git_client,
        }
    }

    /// 索引单个仓库的所有分支
    pub async fn index_repository(&self, repository_id: i64, path: &Path) -> Result<IndexResult> {
        let mut result = IndexResult::default();

        // 获取所有分支
        let branches = self.git_client.list_branches(path).await?;
        
        info!("Found {} branches to index", branches.len());

        // 将分支信息转换为实体并保存到数据库
        let branch_entities: Vec<Branch> = branches
            .iter()
            .map(|b| Branch {
                id: 0, // 由数据库生成
                repository_id,
                name: b.name.clone(),
                target_oid: b.target_oid.clone(),
                is_default: b.is_head,
                updated_at: chrono::Utc::now(),
            })
            .collect();

        if !branch_entities.is_empty() {
            self.branch_store.save_many(&branch_entities).await?;
            info!("Saved {} branches to database", branch_entities.len());
        }

        for branch in branches {
            // 只索引 remote 分支（格式如 origin/main）
            if !branch.name.starts_with("origin/") {
                continue;
            }

            debug!("Indexing branch: {}", branch.name);

            // 构建完整的 ref 路径用于 get_commits
            let ref_name = format!("refs/remotes/{}", branch.name);
            
            // 但存储时使用简短名称（origin/main）
            match self.index_branch(repository_id, path, &ref_name, &branch.name).await {
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
        ref_name: &str,        // 完整ref路径，如 refs/remotes/origin/main
        branch_name: &str,     // 简短名称，如 origin/main
    ) -> Result<usize> {
        // 获取最后索引的提交
        let last_indexed = self.commit_store.get_latest_commit(repository_id, branch_name).await?;
        let last_indexed_oid = last_indexed.map(|c| c.oid);

        if let Some(ref oid) = last_indexed_oid {
            debug!("Found last indexed commit for {}: {}", branch_name, oid);
        }

        // 获取新提交
        let commits = self.git_client.get_commits(
            path,
            ref_name,  // 使用完整ref路径
            self.config.indexer.max_commits_per_branch,
            last_indexed_oid.as_deref(),
        ).await?;

        if commits.is_empty() {
            debug!("No new commits for branch {}", branch_name);
            return Ok(0);
        }

        // 转换为领域实体
        let domain_commits: Vec<Commit> = commits
            .into_iter()
            .map(|c| {
                Commit::new(
                    repository_id,
                    c.oid,
                    branch_name.to_string(),  // 存储简短名称
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

        // 使用bulk_insert批量插入，比逐个save快很多
        match self.commit_store.bulk_insert(&domain_commits).await {
            Ok(inserted) => {
                info!("Indexed {} commits for branch {}", inserted, branch_name);
            }
            Err(e) => {
                error!("Failed to bulk insert commits: {}", e);
                return Err(e);
            }
        }

        Ok(count)
    }
}

#[derive(Debug, Default)]
pub struct IndexResult {
    pub commits_indexed: usize,
    pub branches_indexed: usize,
    pub branches_failed: usize,
}
