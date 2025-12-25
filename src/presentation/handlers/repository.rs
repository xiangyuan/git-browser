use askama::Template;
use axum::{
    extract::{State, Path, Query},
    response::{Html, IntoResponse, Json},
};
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use crate::presentation::routes::AppContext;
use crate::presentation::dto::RepositoryDto;
use crate::presentation::templates::*;
use crate::shared::result::Result;

/// 列出所有仓库（Web UI）- 使用模板
pub async fn list_repositories(
    State(ctx): State<Arc<AppContext>>,
) -> Result<impl IntoResponse> {
    let repos = ctx.repository_store.list_all().await?;
    
    let repo_items: Vec<RepoItem> = repos
        .iter()
        .map(|r| RepoItem {
            name: r.name.clone(),
            path: r.path.clone(),
            description: r.description.clone(),
            last_modified: r.last_synced_at
                .unwrap_or(r.created_at)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        })
        .collect();
    
    let template = IndexTemplate {
        repositories: repo_items,
    };
    
    Ok(Html(template.render()?))
}

/// UI: 仓库概览页 - 使用模板
pub async fn repo_summary(
    State(ctx): State<Arc<AppContext>>,
    Path(repo_name): Path<String>,
) -> Result<impl IntoResponse> {
    let repo = ctx.repository_store
        .find_by_name(&repo_name)
        .await?
        .ok_or_else(|| crate::shared::error::GitxError::RepositoryNotFound(repo_name.clone()))?;
    
    let repo_path = std::path::PathBuf::from(&repo.path);
    
    // 获取分支列表
    let branches = ctx.git_client.list_branches(&repo_path).await?;
    
    let branch_items: Vec<BranchItem> = branches
        .iter()
        .map(|b| BranchItem {
            name: b.name.clone(),
            commit_sha: b.target_oid.clone(),
            commit_message: "".to_string(),
            author: "".to_string(),
            time: "".to_string(),
        })
        .collect();
    
    let links = get_diff_links(&ctx, &repo_name, None);
    
    let template = SummaryTemplate {
        repo_name: repo_name.clone(),
        repo_path: repo.path.clone(),
        branches: branch_items,
        links,
    };
    
    Ok(Html(template.render()?))
}

/// UI: 提交日志页 - 使用模板
#[derive(Deserialize)]
pub struct LogQuery {
    br: Option<String>,
    ofs: Option<usize>,
}

pub async fn repo_log(
    State(ctx): State<Arc<AppContext>>,
    Path(repo_name): Path<String>,
    Query(query): Query<LogQuery>,
) -> Result<impl IntoResponse> {
    let repo = ctx.repository_store
        .find_by_name(&repo_name)
        .await?
        .ok_or_else(|| crate::shared::error::GitxError::RepositoryNotFound(repo_name.clone()))?;
    
    let branch = query.br.as_deref();
    let offset = query.ofs.unwrap_or(0) as i64;
    let limit = 50i64;
    
    let commits = ctx.commit_store
        .list_by_repository(repo.id, branch, limit, offset)
        .await?;
    
    let commit_items: Vec<CommitItem> = commits
        .iter()
        .map(|c| CommitItem {
            sha: c.oid.clone(),
            sha_short: c.oid[..8.min(c.oid.len())].to_string(),
            message: c.message.as_ref().and_then(|m| m.lines().next()).unwrap_or("").to_string(),
            summary: c.summary.to_string(),
            author_name: c.author_name.clone(),
            author_email: c.author_email.clone(),
            committer_time: c.committer_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        })
        .collect();
    
    let has_more = commit_items.len() >= limit as usize;
    let next_offset = (offset + limit) as usize;
    let links = get_diff_links(&ctx, &repo_name, None);
    
    let template = LogTemplate {
        repo_name: repo_name.clone(),
        commits: commit_items,
        branch: query.br.clone(),
        has_more,
        next_offset,
        links,
    };
    
    Ok(Html(template.render()?))
}

/// UI: 单个提交详情页 - 使用模板
#[derive(Deserialize)]
pub struct CommitQuery {
    id: String,
}

pub async fn repo_commit(
    State(ctx): State<Arc<AppContext>>,
    Path(repo_name): Path<String>,
    Query(query): Query<CommitQuery>,
) -> Result<impl IntoResponse> {
    let repo = ctx.repository_store
        .find_by_name(&repo_name)
        .await?
        .ok_or_else(|| crate::shared::error::GitxError::RepositoryNotFound(repo_name.clone()))?;
    
    let commit = ctx.commit_store
        .find_by_oid(repo.id, &query.id)
        .await?
        .ok_or_else(|| crate::shared::error::GitxError::Internal(format!("Commit {} not found", query.id)))?;
    
    let detail = CommitDetail {
        sha: commit.oid.clone(),
        tree: "".to_string(),
        parents: vec![],
        author_name: commit.author_name.clone(),
        author_email: commit.author_email.clone(),
        author_time: commit.author_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        committer_name: commit.committer_name.clone(),
        committer_email: commit.committer_email.clone(),
        committer_time: commit.committer_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        message: commit.message.clone().unwrap_or_default(),
    };
    
    let links = get_diff_links(&ctx, &repo_name, None);
    
    let template = CommitTemplate {
        repo_name: repo_name.clone(),
        commit: detail,
        links,
    };
    
    Ok(Html(template.render()?))
}

/// UI: 分支对比页 - 使用模板
#[derive(Deserialize)]
pub struct DiffQuery {
    o: String,
    n: String,
}

pub async fn repo_diff(
    State(ctx): State<Arc<AppContext>>,
    Path(repo_name): Path<String>,
    Query(query): Query<DiffQuery>,
) -> Result<impl IntoResponse> {
    let repo = ctx.repository_store
        .find_by_name(&repo_name)
        .await?
        .ok_or_else(|| crate::shared::error::GitxError::RepositoryNotFound(repo_name.clone()))?;
    
    let commits = ctx.commit_store
        .list_by_repository(repo.id, Some(&query.n), 100i64, 0i64)
        .await?;
    
    let commit_items: Vec<CommitItem> = commits
        .iter()
        .map(|c| CommitItem {
            sha: c.oid.clone(),
            sha_short: c.oid[..8.min(c.oid.len())].to_string(),
            message: c.message.as_ref().and_then(|m| m.lines().next()).unwrap_or("").to_string(),
            summary: c.message.as_ref().and_then(|m| m.lines().next()).unwrap_or("").to_string(),
            author_name: c.author_name.clone(),
            author_email: c.author_email.clone(),
            committer_time: c.committer_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        })
        .collect();
    
    let links = get_diff_links(&ctx, &repo_name, Some((&query.o, &query.n)));
    
    let template = DiffTemplate {
        repo_name: repo_name.clone(),
        from_branch: query.o.clone(),
        to_branch: query.n.clone(),
        commits: commit_items,
        links,
    };
    
    Ok(Html(template.render()?))
}

// ===== API Handlers =====

pub async fn api_list_repositories(
    State(ctx): State<Arc<AppContext>>,
) -> Result<Json<Vec<RepositoryDto>>> {
    let repos = ctx.repository_store.list_all().await?;
    let dtos: Vec<RepositoryDto> = repos.into_iter().map(Into::into).collect();
    
    Ok(Json(dtos))
}

pub async fn api_get_repository(
    State(ctx): State<Arc<AppContext>>,
    Path(id): Path<i64>,
) -> Result<Json<RepositoryDto>> {
    let repo = ctx.repository_store
        .find_by_id(id)
        .await?
        .ok_or_else(|| crate::shared::error::GitxError::RepositoryNotFound(id.to_string()))?;
    
    Ok(Json(repo.into()))
}

pub async fn api_sync_repository(
    State(ctx): State<Arc<AppContext>>,
    Path(id): Path<i64>,
) -> Result<Json<SyncResponse>> {
    let repo = ctx.repository_store
        .find_by_id(id)
        .await?
        .ok_or_else(|| crate::shared::error::GitxError::RepositoryNotFound(id.to_string()))?;
    
    let repo_path = std::path::PathBuf::from(&repo.path);
    let result = ctx.git_client.fetch_repository(&repo_path).await?;
    ctx.repository_store.update_sync_time(id).await?;
    
    Ok(Json(SyncResponse {
        success: true,
        message: format!("Synced {} branches", result.branches_updated.len()),
    }))
}

#[derive(Serialize)]
pub struct SyncResponse {
    success: bool,
    message: String,
}

fn get_diff_links(ctx: &AppContext, repo_name: &str, active: Option<(&str, &str)>) -> Vec<DiffLink> {
    ctx.config
        .projects
        .iter()
        .find(|p| p.name == repo_name)
        .map(|p| {
            p.branches
                .iter()
                .map(|b| DiffLink {
                    name: b.name.clone(),
                    from_branch: b.from_branch.clone(),
                    to_branch: b.to_branch.clone(),
                    active: active
                        .map(|(o, n)| o == b.from_branch && n == b.to_branch)
                        .unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default()
}
