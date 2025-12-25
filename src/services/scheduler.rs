use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{info, error};
use crate::ports::repository::RepositoryPort;
use crate::ports::git::GitPort;
use crate::shared::config::Config;
use crate::shared::result::Result;
use crate::services::discovery::RepositoryDiscovery;
use crate::services::worker::IndexWorker;

/// 索引调度器 - 定期扫描和调度索引任务
pub struct IndexerScheduler {
    config: Arc<Config>,
    repository_store: Arc<dyn RepositoryPort>,
    git_client: Arc<dyn GitPort>,
}

impl IndexerScheduler {
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

        // 2. 为每个仓库执行索引
        for repo_info in discovered_repos {
            match self.index_repository(&repo_info).await {
                Ok(indexed) => {
                    if indexed {
                        stats.repos_synced += 1;
                    }
                }
                Err(e) => {
                    error!("Failed to index repository {}: {}", repo_info.path.display(), e);
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

        // 2. 同步仓库
        info!("Syncing repository: {}", repo_info.name);
        match self.git_client.fetch_repository(&repo_info.path).await {
            Ok(fetch_result) => {
                info!(
                    "Repository synced: {} branches updated",
                    fetch_result.branches_updated.len()
                );
            }
            Err(e) => {
                error!("Failed to sync repository {}: {}", repo_info.name, e);
                return Ok(false);
            }
        }

        // 3. 创建索引工作者并执行索引
        let worker = IndexWorker::new(
            Arc::clone(&self.config),
            Arc::clone(&self.repository_store),
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
