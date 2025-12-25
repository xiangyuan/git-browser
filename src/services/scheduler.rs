use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{info, error};
use crate::ports::repository::RepositoryPort;
use crate::ports::commit::CommitPort;
use crate::ports::branch::BranchPort;
use crate::ports::git::GitPort;
use crate::shared::config::Config;
use crate::shared::result::Result;
use crate::services::discovery::RepositoryDiscovery;
use crate::services::worker::IndexWorker;

/// 索引调度器 - 定期扫描和调度索引任务
pub struct IndexerScheduler {
    config: Arc<Config>,
    repository_store: Arc<dyn RepositoryPort>,
    commit_store: Arc<dyn CommitPort>,
    branch_store: Arc<dyn BranchPort>,
    git_client: Arc<dyn GitPort>,
}

impl IndexerScheduler {
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

    /// 启动调度器（长期运行）
    pub async fn start(self: &Arc<Self>) {
        if !self.config.indexer.enabled {
            info!("Indexer is disabled in configuration");
            return;
        }

        let interval_duration = Duration::from_secs(self.config.indexer.interval_secs);
        let mut interval = time::interval(interval_duration);

        info!(
            "Indexer scheduler started, interval: {}s",
            self.config.indexer.interval_secs
        );

        loop {
            interval.tick().await;
            
            info!("Starting scheduled indexing cycle");
            
            match self.run_index_cycle().await {
                Ok(stats) => {
                    info!(
                        "Index cycle completed: {} repos discovered, {} synced",
                        stats.repos_discovered,
                        stats.repos_synced
                    );
                }
                Err(e) => {
                    error!("Index cycle failed: {}", e);
                }
            }
        }
    }

    /// 执行一次索引周期
    async fn run_index_cycle(&self) -> Result<IndexStats> {
        let mut stats = IndexStats::default();

        // 1. 发现仓库
        let discovery = RepositoryDiscovery::new(self.config.clone());
        let discovered_repos = discovery.discover_all().await?;
        stats.repos_discovered = discovered_repos.len();

        info!("Discovered {} repositories", stats.repos_discovered);

        // 2. 并行为每个仓库执行索引
        let repo_count = discovered_repos.len();
        let tasks: Vec<_> = discovered_repos
            .into_iter()
            .enumerate()
            .map(|(idx, repo_info)| {
                let config = self.config.clone();
                let repository_store = self.repository_store.clone();
                let commit_store = self.commit_store.clone();
                let branch_store = self.branch_store.clone();
                let git_client = self.git_client.clone();
                
                tokio::spawn(async move {
                    info!("[{}/{}] Starting to index: {}", idx + 1, repo_count, repo_info.name);
                    
                    // 创建临时scheduler实例来调用index_repository
                    let temp_scheduler = IndexerScheduler {
                        config,
                        repository_store,
                        commit_store,
                        branch_store,
                        git_client,
                    };
                    
                    let result = temp_scheduler.index_repository(&repo_info).await;
                    if let Ok(true) = result {
                        info!("[{}/{}] ✓ Finished indexing: {}", idx + 1, repo_count, repo_info.name);
                    }
                    result
                })
            })
            .collect();

        // 等待所有任务完成
        for task in tasks {
            match task.await {
                Ok(Ok(indexed)) => {
                    if indexed {
                        stats.repos_synced += 1;
                    }
                }
                Ok(Err(e)) => {
                    error!("Failed to index repository: {}", e);
                    stats.repos_failed += 1;
                }
                Err(e) => {
                    error!("Task panicked: {}", e);
                    stats.repos_failed += 1;
                }
            }
        }

        Ok(stats)
    }

    /// 索引单个仓库
    async fn index_repository(&self, repo_info: &super::discovery::DiscoveredRepo) -> Result<bool> {
        // 1. 检查仓库是否已存在
        let existing_repo = self.repository_store
            .find_by_path(&repo_info.path.display().to_string())
            .await?;

        let repository_id = if let Some(mut repo) = existing_repo {
            // 更新已存在的仓库
            info!("Updating existing repository: {}", repo.name);
            repo.update_sync_time();
            self.repository_store.save(&repo).await?
        } else {
            // 创建新仓库
            info!("Adding new repository: {}", repo_info.name);
            let new_repo = crate::domain::entities::Repository::new(
                repo_info.name.clone(),
                repo_info.path.display().to_string(),
            );
            self.repository_store.save(&new_repo).await?
        };

        // 2. 同步仓库（添加超时和错误处理）
        info!("Syncing repository: {}", repo_info.name);
        
        let fetch_timeout = Duration::from_secs(self.config.git.fetch_timeout_secs);
        let fetch_result = tokio::time::timeout(
            fetch_timeout,
            self.git_client.fetch_repository(&repo_info.path)
        ).await;
        
        match fetch_result {
            Ok(Ok(result)) => {
                info!(
                    "Repository synced: {} branches updated",
                    result.branches_updated.len()
                );
            }
            Ok(Err(e)) => {
                error!("Failed to fetch repository {}: {}", repo_info.name, e);
                info!("Continuing with local data...");
            }
            Err(_) => {
                error!("Fetch timeout for repository {}, continuing with local data", repo_info.name);
            }
        }

        // 3. 创建索引工作者并执行索引
        let worker = IndexWorker::new(
            Arc::clone(&self.config),
            Arc::clone(&self.repository_store),
            Arc::clone(&self.commit_store),
            Arc::clone(&self.branch_store),
            Arc::clone(&self.git_client),
        );

        worker.index_repository(repository_id, &repo_info.path).await?;

        Ok(true)
    }

    /// 手动触发索引（用于 API）
    pub async fn trigger_index(&self, repository_id: i64) -> Result<()> {
        let repo = self.repository_store
            .find_by_id(repository_id)
            .await?
            .ok_or_else(|| crate::shared::error::GitxError::RepositoryNotFound(
                repository_id.to_string()
            ))?;

        let repo_path = std::path::PathBuf::from(&repo.path);
        
        // 同步仓库
        self.git_client.fetch_repository(&repo_path).await?;

        // 索引仓库
        let worker = IndexWorker::new(
            Arc::clone(&self.config),
            Arc::clone(&self.repository_store),
            Arc::clone(&self.commit_store),
            Arc::clone(&self.branch_store),
            Arc::clone(&self.git_client),
        );

        worker.index_repository(repository_id, &repo_path).await?;

        // 更新同步时间
        self.repository_store.update_sync_time(repository_id).await?;

        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct IndexStats {
    pub repos_discovered: usize,
    pub repos_synced: usize,
    pub repos_failed: usize,
}
