use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use clap::Parser;
use axum::http::{HeaderValue, Method};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::info;

mod shared;
mod domain;
mod ports;
mod infrastructure;
mod services;
mod presentation;

use shared::config::Config;
use shared::result::Result;
use infrastructure::git::Git2Client;
use infrastructure::sqlite::repository_repo::SqliteRepositoryRepository;
use infrastructure::sqlite::commit_repo::SqliteCommitRepository;
use infrastructure::sqlite::branch_repo::SqliteBranchRepository;
use infrastructure::cache::MokaCache;
use presentation::routes::AppContext;


#[derive(Parser, Debug)]
#[clap(name = "Gitx")]
#[clap(author = "xiangyuan@gmail.com")]
#[clap(version = "0.2.0")]
#[clap(about = "Git repository indexer and browser")]
pub struct Args {
    /// The database store directory (SQLite database path)
    #[clap(short, long, value_parser, default_value = "gitx.db")]
    db_path: PathBuf,

    /// Server bind address
    #[clap(short, long, default_value = "127.0.0.1:8080")]
    bind_address: SocketAddr,
    
    /// Base path to scan for git repositories (can be a single repo or directory containing repos)
    #[clap(short = 'p', long = "path", value_parser, value_name = "PATH")]
    git_base_path: Option<PathBuf>,
}


#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    println!("{:?}", args);
    
    // 初始化日志
    let subscriber = tracing_subscriber::fmt();
    subscriber.pretty().init();

    // 加载配置
    let config = Config::from_args_and_file(
        args.db_path.clone(),
        args.bind_address,
        args.git_base_path.clone(),
    )?;
    let config = Arc::new(config);

    info!("Starting GitX server...");
    info!("Configuration loaded: {:?}", config);

    // 初始化 SQLite 数据库
    let sqlite_pool = infrastructure::sqlite::create_pool(
        &config.database.sqlite_path,
        config.database.max_connections,
    )
    .await?;

    // 运行数据库迁移
    info!("Running database migrations...");
    infrastructure::sqlite::run_migrations(&sqlite_pool).await?;
    info!("Database migrations completed");

    // 创建新架构的应用上下文
    let repository_store = Arc::new(SqliteRepositoryRepository::new(sqlite_pool.clone()));
    let commit_store = Arc::new(SqliteCommitRepository::new(sqlite_pool.clone()));
    let branch_store = Arc::new(SqliteBranchRepository::new(sqlite_pool.clone()));
    let git_client = Arc::new(Git2Client::new());
    let cache = Arc::new(MokaCache::new(
        config.cache.max_capacity,
        Duration::from_secs(config.cache.ttl_secs),
    ));

    let app_context = Arc::new(AppContext {
        repository_store: repository_store.clone(),
        commit_store: commit_store.clone(),
        branch_store: branch_store.clone(),
        git_client: git_client.clone(),
        cache,
        config: config.clone(),
    });

    // 启动新架构的索引调度器
    let scheduler = Arc::new(services::scheduler::IndexerScheduler::new(
        config.clone(),
        repository_store.clone(),
        commit_store.clone(),
        branch_store.clone(),
        git_client.clone(),
    ));
    
    info!("Starting indexer scheduler...");
    tokio::spawn(async move {
        scheduler.start().await;
    });

    let serve_dir_service = ServeDir::new("statics");

    // 创建应用路由（新架构）
    let app = presentation::routes::create_app_router(app_context)
        .nest_service("/statics", serve_dir_service)
        .layer(CorsLayer::new()
            .allow_origin(config.server.cors_origins[0].parse::<HeaderValue>()
                .map_err(|e| shared::error::GitxError::Config(e.to_string()))?)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE]));

    let listener = tokio::net::TcpListener::bind(&config.server.bind_address)
        .await
        .map_err(|e| shared::error::GitxError::Io(e))?;

    info!("Server listening on {}", config.server.bind_address);
    info!("Web UI available at: http://{}/", config.server.bind_address);
    info!("API available at: http://{}/api/", config.server.bind_address);
    
    axum::serve(listener, app)
        .await
        .map_err(|e| shared::error::GitxError::Internal(e.to_string()))?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_basic() {
        assert_eq!(2 + 2, 4);
    }
}
