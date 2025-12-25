use axum::{Router, routing::{get, post}};
use std::sync::Arc;
use crate::presentation::handlers;
use crate::infrastructure::cache::MokaCache;

/// 应用状态（新架构）
pub struct AppContext {
    pub repository_store: Arc<dyn crate::ports::repository::RepositoryPort>,
    pub commit_store: Arc<dyn crate::ports::commit::CommitPort>,
    pub branch_store: Arc<dyn crate::ports::branch::BranchPort>,
    pub git_client: Arc<dyn crate::ports::git::GitPort>,
    #[allow(dead_code)]  // 后续功能会使用
    pub cache: Arc<MokaCache>,  // 使用具体类型
    #[allow(dead_code)]  // 后续功能会使用
    pub config: Arc<crate::shared::config::Config>,
}

/// 创建应用路由
pub fn create_app_router(ctx: Arc<AppContext>) -> Router {
    Router::new()
        // 主页 - 仓库列表
        .route("/", get(handlers::repository::list_repositories))
        
        // UI 路由 - 仓库页面
        .route("/{repo}/summary", get(handlers::repository::repo_summary))
        .route("/{repo}/log", get(handlers::repository::repo_log))
        .route("/{repo}/commit", get(handlers::repository::repo_commit))
        .route("/{repo}/diff-beta", get(handlers::repository::repo_diff))
        .route("/{repo}/api/cherry-pick", post(handlers::repository::api_cherry_pick))
        .route("/{repo}/api/push", post(handlers::repository::api_push))
        
        // API 路由
        .nest("/api", api_routes())
        
        .with_state(ctx)
}

/// API 路由
fn api_routes() -> Router<Arc<AppContext>> {
    Router::new()
        // 仓库 API
        .route("/repositories", get(handlers::repository::api_list_repositories))
        .route("/repositories/{id}", get(handlers::repository::api_get_repository))
        .route("/repositories/{id}/sync", get(handlers::repository::api_sync_repository))
        
        // 提交 API
        .route("/repositories/{id}/commits", get(handlers::commit::api_list_commits))
        .route("/repositories/{id}/commits/{oid}", get(handlers::commit::api_get_commit))
        
        // 分支 API
        .route("/repositories/{id}/branches", get(handlers::branch::api_list_branches))
}
