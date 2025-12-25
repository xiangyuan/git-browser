use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use crate::shared::result::Result;

/// 应用配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub git: GitConfig,
    pub indexer: IndexerConfig,
    pub cache: CacheConfig,
    pub projects: Vec<ProjectConfig>,
}

/// 服务器配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub bind_address: SocketAddr,
    pub cors_origins: Vec<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:8080".parse().unwrap(),
            cors_origins: vec!["http://localhost:3000".to_string()],
        }
    }
}

/// 数据库配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    pub sqlite_path: PathBuf,
    pub max_connections: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            sqlite_path: PathBuf::from("gitx.db"),
            max_connections: 10,
        }
    }
}

/// Git 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GitConfig {
    pub ssh_key_path: Option<PathBuf>,
    pub fetch_timeout_secs: u64,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            ssh_key_path: None,
            fetch_timeout_secs: 300,
        }
    }
}

/// 索引器配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IndexerConfig {
    pub enabled: bool,
    pub interval_secs: u64,
    pub max_commits_per_branch: usize,
    pub worker_threads: usize,
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_secs: 300,
            max_commits_per_branch: 2000,
            worker_threads: 4,
        }
    }
}

/// 缓存配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheConfig {
    pub max_capacity: u64,
    pub ttl_secs: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_capacity: 10000,
            ttl_secs: 3600,
        }
    }
}

/// 项目配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectConfig {
    pub name: String,
    pub base_path: PathBuf,
    pub scan_paths: Vec<String>,
    pub branches: Vec<BranchCompareConfig>,
}

/// 分支对比配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BranchCompareConfig {
    pub name: String,
    pub from_branch: String,
    pub to_branch: String,
}

impl Config {
    /// 从文件加载配置
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| crate::shared::error::GitxError::Config(e.to_string()))?;
        Ok(config)
    }

    /// 从命令行参数和文件加载配置
    pub fn from_args_and_file(
        db_path: PathBuf,
        bind_address: SocketAddr,
        git_base_path: Option<PathBuf>,
    ) -> Result<Self> {
        // 尝试加载配置文件
        let mut config = if let Ok(cfg) = Self::from_file("config.toml") {
            cfg
        } else {
            // 使用默认配置
            Config {
                server: ServerConfig::default(),
                database: DatabaseConfig::default(),
                git: GitConfig::default(),
                indexer: IndexerConfig::default(),
                cache: CacheConfig::default(),
                projects: vec![],
            }
        };

        // 命令行参数覆盖配置文件
        config.server.bind_address = bind_address;
        config.database.sqlite_path = db_path;

        // 如果命令行提供了git路径，优先使用命令行参数
        if let Some(base_path) = git_base_path {
            let project_name = base_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("default")
                .to_string();
            
            config.projects = vec![ProjectConfig {
                name: project_name,
                base_path,
                scan_paths: vec![".".to_string()],  // 扫描整个目录
                branches: vec![],  // 不预设分支对比
            }];
        } else if config.projects.is_empty() {
            // 如果没有命令行参数且配置文件也没有项目，则无法发现仓库
            tracing::warn!("No projects configured and no git base path provided. Use -p to specify a path or add projects to config.toml");
        }

        Ok(config)
    }

    /// 保存配置到文件
    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| crate::shared::error::GitxError::Config(e.to_string()))?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
