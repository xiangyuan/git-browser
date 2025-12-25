use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tracing::{debug, warn};
use crate::shared::config::Config;
use crate::shared::result::Result;

/// 仓库发现服务
pub struct RepositoryDiscovery {
    config: Arc<Config>,
}

impl RepositoryDiscovery {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    /// 发现所有配置的仓库
    pub async fn discover_all(&self) -> Result<Vec<DiscoveredRepo>> {
        let mut all_repos = Vec::new();

        for project in &self.config.projects {
            for scan_path in &project.scan_paths {
                let full_path = project.base_path.join(scan_path);
                
                if !full_path.exists() {
                    warn!("Scan path does not exist: {}", full_path.display());
                    continue;
                }

                // 只检查 scan_path 指定的路径是否为 git 仓库，不递归子目录
                if self.is_git_repo(&full_path) {
                    let name = full_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    debug!("Found repository: {}", full_path.display());
                    
                    let canonical_path = match full_path.canonicalize() {
                        Ok(p) => p,
                        Err(e) => {
                            warn!("Failed to canonicalize path {}: {}", full_path.display(), e);
                            full_path.clone()
                        }
                    };
                    
                    all_repos.push(DiscoveredRepo {
                        name,
                        path: canonical_path,
                    });
                } else {
                    debug!("Path is not a git repository: {}", full_path.display());
                }
            }
        }

        debug!("Discovered {} repositories in total", all_repos.len());
        Ok(all_repos)
    }

    /// 检查路径是否为 Git 仓库
    fn is_git_repo(&self, path: &Path) -> bool {
        path.join(".git").exists() || path.join("packed-refs").exists()
    }
}

/// 发现的仓库信息
#[derive(Debug, Clone)]
pub struct DiscoveredRepo {
    pub name: String,
    pub path: PathBuf,
}
