use std::collections::HashMap;
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

        // 先读出旧 tip，再覆盖 branches 表。增量索引必须用「上一次的 tip」，
        // 不能用 committer_time 最大的那条（merge 进来的更早提交会被漏掉）。
        let existing_branches = self.branch_store.find_by_repository(repository_id).await?;
        let old_tips: HashMap<String, String> = existing_branches
            .into_iter()
            .map(|b| (b.name, b.target_oid))
            .collect();

        let branches = self.git_client.list_branches(path).await?;
        
        info!("Found {} branches to index", branches.len());

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

        for branch in &branches {
            // 只索引 remote 分支（格式如 origin/main）
            if !branch.name.starts_with("origin/") {
                continue;
            }

            debug!("Indexing branch: {}", branch.name);

            let ref_name = format!("refs/remotes/{}", branch.name);
            let previous_tip = old_tips.get(&branch.name).map(String::as_str);
            
            match self.index_branch(
                repository_id,
                path,
                &ref_name,
                &branch.name,
                previous_tip,
                &branch.target_oid,
            ).await {
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

        if !branch_entities.is_empty() {
            self.branch_store.save_many(&branch_entities).await?;
            info!("Saved {} branches to database", branch_entities.len());
        }

        info!(
            "Repository indexing completed: {} commits, {} branches",
            result.commits_indexed,
            result.branches_indexed
        );

        Ok(result)
    }

    /// 把本地分支上尚未推送的提交索引到对应的 origin/* 记录里。
    /// merge / cherry-pick 之后 origin 还没动，必须走本地 ref 才能让 log/diff 立刻更新。
    pub async fn index_local_as_remote(
        &self,
        repository_id: i64,
        path: &Path,
        local_branch: &str,
        remote_branch_name: &str,
    ) -> Result<usize> {
        let local_tip = self.git_client.rev_parse(path, "HEAD").await?;
        let existing = self.branch_store.find_by_repository(repository_id).await?;
        let previous_tip = existing
            .iter()
            .find(|b| b.name == remote_branch_name)
            .map(|b| b.target_oid.as_str());
        let ref_name = format!("refs/heads/{}", local_branch);

        self.index_branch(
            repository_id,
            path,
            &ref_name,
            remote_branch_name,
            previous_tip,
            &local_tip,
        ).await
    }

    /// 索引单个分支（增量更新）
    pub async fn index_branch(
        &self,
        repository_id: i64,
        path: &Path,
        ref_name: &str,        // 完整ref路径，如 refs/remotes/origin/main
        branch_name: &str,     // 简短名称，如 origin/main
        previous_tip: Option<&str>,
        current_tip: &str,
    ) -> Result<usize> {
        if previous_tip == Some(current_tip) {
            debug!("Branch {} tip unchanged ({}), skip", branch_name, current_tip);
            return Ok(0);
        }

        let mut since_oid: Option<String> = None;

        if let Some(old_tip) = previous_tip {
            let ancestor = self
                .git_client
                .is_ancestor(path, old_tip, current_tip)
                .await
                .unwrap_or(false);
            if ancestor {
                since_oid = Some(old_tip.to_string());
                debug!(
                    "Incremental index for {} ({}..{})",
                    branch_name, old_tip, current_tip
                );
            } else {
                info!(
                    "History rewritten for {} ({} -> {}), rebuilding commit index",
                    branch_name, old_tip, current_tip
                );
                self.commit_store
                    .delete_by_branch(repository_id, branch_name)
                    .await?;
            }
        } else {
            // 第一次索引该分支：清掉可能残留的旧行
            self.commit_store
                .delete_by_branch(repository_id, branch_name)
                .await?;
        }

        let commits = self.git_client.get_commits(
            path,
            ref_name,
            self.config.indexer.max_commits_per_branch,
            since_oid.as_deref(),
        ).await?;

        if commits.is_empty() {
            debug!("No new commits for branch {}", branch_name);
            return Ok(0);
        }

        let domain_commits: Vec<Commit> = commits
            .into_iter()
            .map(|c| {
                Commit::new(
                    repository_id,
                    c.oid,
                    branch_name.to_string(),
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
