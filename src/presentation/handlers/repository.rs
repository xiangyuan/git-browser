use askama::Template;
use axum::{
    extract::{State, Path, Query},
    response::{Html, IntoResponse, Json},
    debug_handler,
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
    
    let all_branches: Vec<String> = branches
        .iter()
        .map(|b| b.name.clone())
        .collect();
    

    let template = SummaryTemplate {
        repo_name: repo_name.clone(),
        repo_path: repo.path.clone(),
        branches: branch_items,
        all_branches,
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
    let all_branches = get_all_branches(&ctx, repo.id).await?;

    let template = LogTemplate {
        repo_name: repo_name.clone(),
        commits: commit_items,
        branch: query.br.clone(),
        has_more,
        next_offset,
        all_branches,
    };
    
    Ok(Html(template.render()?))
}

/// UI: 单个提交详情页 - 使用模板
#[derive(Deserialize)]
pub struct CommitQuery {
    id: Option<String>,
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
    
    // 如果没有指定commit id，显示默认分支的commit列表
    if query.id.is_none() {
        // 从branches表获取默认分支
        let branches = ctx.branch_store
            .find_by_repository(repo.id)
            .await?;
        
        // 找到默认分支，或使用第一个分支
        let default_branch_name = branches
            .iter()
            .find(|b| b.is_default)
            .or_else(|| branches.first())
            .map(|b| b.name.as_str())
            .unwrap_or("origin/main");
        
        let limit = 50i64;
        let commits = ctx.commit_store
            .list_by_repository(repo.id, Some(default_branch_name), limit, 0)
            .await?;
        
        let commit_items: Vec<CommitItem> = commits
            .iter()
            .map(|c| CommitItem {
                sha: c.oid.clone(),
                sha_short: c.oid[..8.min(c.oid.len())].to_string(),
                message: c.message.as_ref().and_then(|m| m.lines().next()).unwrap_or("").to_string(),
                summary: c.summary.clone(),
                author_name: c.author_name.clone(),
                author_email: c.author_email.clone(),
                committer_time: c.committer_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            })
            .collect();
        
        let all_branches = get_all_branches(&ctx, repo.id).await?;

        let len = commit_items.len();
        let template = LogTemplate {
            repo_name: repo_name.clone(),
            commits: commit_items,
            branch: Some(default_branch_name.to_string()),
            has_more: len >= limit as usize,
            next_offset: limit as usize,
            all_branches,
        };
        
        return Ok(Html(template.render()?));
    }
    
    let commit_id = query.id.unwrap();
    
    let commit = ctx.commit_store
        .find_by_oid(repo.id, &commit_id)
        .await?
        .ok_or_else(|| crate::shared::error::GitxError::Internal(format!("Commit {} not found", commit_id)))?;
    
    // 从 git 获取完整的 commit detail（包含 diff）
    let repo_path = std::path::PathBuf::from(&repo.path);
    let git_detail = ctx.git_client.get_commit_detail(&repo_path, &commit_id).await?;
    
    let detail = CommitDetail {
        sha: commit.oid.clone(),
        tree: "".to_string(), // GitCommit没有tree_oid字段，暂时留空
        parents: git_detail.commit.parent_oids.clone(),
        author_name: commit.author_name.clone(),
        author_email: commit.author_email.clone(),
        author_time: commit.author_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        committer_name: commit.committer_name.clone(),
        committer_email: commit.committer_email.clone(),
        committer_time: commit.committer_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        message: commit.message.clone().unwrap_or_default(),
        diff_stats: git_detail.diff_stats.clone(),
        diff: git_detail.diff_html.clone(),
    };
    
    let all_branches = get_all_branches(&ctx, repo.id).await?;

    let template = CommitTemplate {
        repo_name: repo_name.clone(),
        commit: detail,
        all_branches,
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
    
    // 获取所有分支列表用于下拉选择
    let all_branches = ctx.branch_store
        .find_by_repository(repo.id)
        .await?;
    
    let branch_names: Vec<String> = all_branches
        .iter()
        .map(|b| b.name.clone())
        .collect();
    
    // 使用数据库中已索引的commits进行对比
    // 通过 author_name + summary + committer_time 识别相同的逻辑commit
    // 这样可以正确处理cherry-pick的情况
    let commits = ctx.commit_store
        .find_diff_commits(repo.id, &query.o, &query.n, 1000)
        .await?;
    
    let commit_items: Vec<CommitItem> = commits
        .iter()
        .map(|c| CommitItem {
            sha: c.oid.clone(),
            sha_short: c.oid[..8.min(c.oid.len())].to_string(),
            message: c.summary.clone(),
            summary: c.summary.clone(),
            author_name: c.author_name.clone(),
            author_email: c.author_email.clone(),
            committer_time: c.committer_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        })
        .collect();
    

    let template = DiffTemplate {
        repo_name: repo_name.clone(),
        from_branch: query.o.clone(),
        to_branch: query.n.clone(),
        branches: branch_names,
        commits: commit_items,
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

/// API: Cherry-pick commits
#[derive(Deserialize)]
pub struct CherryPickRequest {
    commits: Vec<String>,
    target_branch: String,
}

#[derive(Serialize)]
pub struct CherryPickResponse {
    success: bool,
    count: usize,
    error: Option<String>,
}

// HTMX 响应模板
#[derive(Template)]
#[template(path = "macros/alert.html")]
struct AlertTemplate {
    level: String, // success, error, warning, info
    message: String,
}

#[debug_handler]
pub async fn api_cherry_pick(
    State(ctx): State<Arc<AppContext>>,
    Path(repo_name): Path<String>,
    // Axum 0.7+ 不支持 Option<Json<T>> 这种自动推断
    // 我们需要手动处理 body
    req: Option<Json<CherryPickRequest>>,
) -> Result<impl IntoResponse> {
    // 如果是 JSON 请求，直接使用
    let req = if let Some(Json(r)) = req {
        r
    } else {
        // 如果不是 JSON，尝试解析 Form
        // 注意：这里简化处理，实际上应该根据 Content-Type 判断
        // 但由于 axum 的 extractor 机制，我们不能同时拥有两个 body extractor
        // 所以对于 HTMX，我们让它发送 JSON 或者我们手动解析 Bytes
        return Ok(Html("<div class='msg-error'>Invalid request format. Please use JSON.</div>".to_string()).into_response());
    };

    let repo = ctx.repository_store
        .find_by_name(&repo_name)
        .await?
        .ok_or_else(|| crate::shared::error::GitxError::RepositoryNotFound(repo_name.clone()))?;
    
    let repo_path = std::path::PathBuf::from(&repo.path);
    
    use tokio::process::Command;
    
    // 1. 首先fetch远程分支获取最新代码
    let fetch_output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("fetch")
        .arg("origin")
        .output()
        .await?;
    
    if !fetch_output.status.success() {
        let error_msg = String::from_utf8_lossy(&fetch_output.stderr).to_string();
        let msg = format!("Failed to fetch: {}", error_msg);
        
        // 如果是 HTMX 请求 (通过 header 判断，这里简单起见直接返回 HTML)
        // 更好的做法是检查 HX-Request header
        return Ok(Html(format!(
            r#"<div class="msg-error">❌ {}</div>"#, 
            msg
        )).into_response());
    }
    
    // 2. 处理目标分支名称（如果是origin/xxx，去掉origin/前缀）
    let local_branch = if req.target_branch.starts_with("origin/") {
        req.target_branch.strip_prefix("origin/").unwrap().to_string()
    } else {
        req.target_branch.clone()
    };
    
    // 3. Checkout到目标分支（如果本地分支不存在，基于远程分支创建）
    let checkout_output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("checkout")
        .arg("-B")  // 创建或重置本地分支
        .arg(&local_branch)
        .arg(format!("origin/{}", local_branch))
        .output()
        .await?;
    
    if !checkout_output.status.success() {
        let error_msg = String::from_utf8_lossy(&checkout_output.stderr).to_string();
        let msg = format!("Failed to checkout {}: {}", local_branch, error_msg);
        return Ok(Html(format!(
            r#"<div class="msg-error">❌ {}</div>"#, 
            msg
        )).into_response());
    }
    
    // 4. 执行git cherry-pick
    let mut success_count = 0;
    for commit_oid in &req.commits {
        let output = Command::new("git")
            .arg("-C")
            .arg(&repo_path)
            .arg("cherry-pick")
            .arg(commit_oid)
            .output()
            .await?;
        
        if output.status.success() {
            success_count += 1;
        } else {
            let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
            // 如果失败，尝试abort
            let _ = Command::new("git")
                .arg("-C")
                .arg(&repo_path)
                .arg("cherry-pick")
                .arg("--abort")
                .output()
                .await;
            
            let msg = format!("Failed at commit {}: {}", commit_oid, error_msg);
            return Ok(Html(format!(
                r#"<div class="msg-error">❌ {}</div>"#, 
                msg
            )).into_response());
        }
    }
    
    let msg = format!("Successfully cherry-picked {} commits to {}", success_count, local_branch);
    Ok(Html(format!(
        r#"<div class="msg-success">✅ {}</div>
           <script>
               document.getElementById('cherry-picked-count').textContent = '(Picked {})';
               document.getElementById('push-btn').style.display = 'inline-block';
           </script>
        "#, 
        msg, success_count
    )).into_response())
}

/// API: Push branch to remote
#[derive(Deserialize)]
pub struct PushRequest {
    branch: String,
}

#[derive(Serialize)]
pub struct PushResponse {
    success: bool,
    error: Option<String>,
}

pub async fn api_push(
    State(ctx): State<Arc<AppContext>>,
    Path(repo_name): Path<String>,
    Json(req): Json<PushRequest>,
) -> Result<Json<PushResponse>> {
    let repo = ctx.repository_store
        .find_by_name(&repo_name)
        .await?
        .ok_or_else(|| crate::shared::error::GitxError::RepositoryNotFound(repo_name.clone()))?;
    
    let repo_path = std::path::PathBuf::from(&repo.path);
    
    use tokio::process::Command;
    
    // 执行git push
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("push")
        .arg("origin")
        .arg(&req.branch)
        .output()
        .await?;
    
    if output.status.success() {
        Ok(Json(PushResponse {
            success: true,
            error: None,
        }))
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
        Ok(Json(PushResponse {
            success: false,
            error: Some(error_msg),
        }))
    }
}

async fn get_all_branches(ctx: &AppContext, repo_id: i64) -> Result<Vec<String>> {
    let branches = ctx.branch_store
        .find_by_repository(repo_id)
        .await?;
    Ok(branches.iter().map(|b| b.name.clone()).collect())
}
