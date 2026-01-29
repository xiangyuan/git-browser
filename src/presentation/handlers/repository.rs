use askama::Template;
use axum::{
    extract::{State, Path, Query},
    response::{Html, IntoResponse, Json},
    debug_handler,
};
use std::sync::Arc;
use std::fmt;
use std::collections::HashSet;
use serde::{Serialize, Deserialize, de::{self, Deserializer, Visitor, SeqAccess}};
use tokio::process::Command;
use crate::presentation::routes::AppContext;
use crate::presentation::dto::RepositoryDto;
use crate::presentation::templates::*;
use crate::shared::result::Result;
use crate::services::worker::IndexWorker;

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
                .to_rfc3339(),
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
            committer_time: c.committer_time.to_rfc3339(),
            is_empty: false,
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
                committer_time: c.committer_time.to_rfc3339(),
                is_empty: false,
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
        author_time: commit.author_time.to_rfc3339(),
        committer_name: commit.committer_name.clone(),
        committer_email: commit.committer_email.clone(),
        committer_time: commit.committer_time.to_rfc3339(),
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
    
    // 使用 git cherry 检测哪些提交已经被 cherry-pick 过（空提交）
    // git cherry 会返回 "-" 开头的行表示已存在，"+" 开头表示新提交
    let repo_path = std::path::PathBuf::from(&repo.path);
    let cherry_output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("cherry")
        .arg(format!("origin/{}", query.n))  // upstream (目标分支)
        .arg(format!("origin/{}", query.o))  // head (源分支)
        .output()
        .await
        .ok();
    
    // 解析 git cherry 输出，构建已存在提交的 set
    let empty_commits: HashSet<String> = cherry_output
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|line: &str| {
                    // 格式: "- <sha>" 或 "+ <sha>"
                    if line.starts_with("- ") {
                        Some(line[2..].trim().to_string())
                    } else {
                        None
                    }
                })
                .collect::<HashSet<String>>()
        })
        .unwrap_or_default();
    
    let commit_items: Vec<CommitItem> = commits
        .iter()
        .map(|c| {
            // 检查该提交是否在 empty_commits 中（完整sha或前缀匹配）
            let is_empty = empty_commits.iter().any(|ec: &String| 
                c.oid.starts_with(ec) || ec.starts_with(&c.oid)
            );
            CommitItem {
                sha: c.oid.clone(),
                sha_short: c.oid[..8.min(c.oid.len())].to_string(),
                message: c.summary.clone(),
                summary: c.summary.clone(),
                author_name: c.author_name.clone(),
                author_email: c.author_email.clone(),
                committer_time: c.committer_time.to_rfc3339(),
                is_empty,
            }
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

/// API: Sync repository by name (for UI usage)
pub async fn api_sync_repository_by_name(
    State(ctx): State<Arc<AppContext>>,
    Path(repo_name): Path<String>,
) -> Result<Json<SyncResponse>> {
    let repo = ctx.repository_store
        .find_by_name(&repo_name)
        .await?
        .ok_or_else(|| crate::shared::error::GitxError::RepositoryNotFound(repo_name.clone()))?;
    
    let repo_path = std::path::PathBuf::from(&repo.path);
    
    // 1. Fetch from remote
    let result = ctx.git_client.fetch_repository(&repo_path).await?;
    
    // 2. Re-index the repository
    let worker = crate::services::worker::IndexWorker::new(
        ctx.config.clone(),
        ctx.repository_store.clone(),
        ctx.commit_store.clone(),
        ctx.branch_store.clone(),
        ctx.git_client.clone(),
    );
    worker.index_repository(repo.id, &repo_path).await?;
    
    // 3. Update sync time
    ctx.repository_store.update_sync_time(repo.id).await?;
    
    Ok(Json(SyncResponse {
        success: true,
        message: format!("Synced {} branches and re-indexed commits", result.branches_updated.len()),
    }))
}

/// API: Cherry-pick commits
#[derive(Deserialize)]
pub struct CherryPickRequest {
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    commits: Vec<String>,
    #[serde(alias = "n")]
    target_branch: String,
}

fn deserialize_string_or_vec<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrVec;

    impl<'de> Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or list of strings")
        }

        fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(vec![value.to_owned()])
        }

        fn visit_seq<S>(self, mut visitor: S) -> std::result::Result<Self::Value, S::Error>
        where
            S: SeqAccess<'de>,
        {
            let mut vec = Vec::new();
            while let Some(elem) = visitor.next_element()? {
                vec.push(elem);
            }
            Ok(vec)
        }
    }

    deserializer.deserialize_any(StringOrVec)
}

#[derive(Serialize)]
pub struct CherryPickResponse {
    success: bool,
    count: usize,
    skipped: usize,
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
    Json(req): Json<CherryPickRequest>,
) -> Result<Json<CherryPickResponse>> {
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
        return Ok(Json(CherryPickResponse {
            success: false,
            count: 0,
            skipped: 0,
            error: Some(format!("Failed to fetch: {}", error_msg)),
        }));
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
        return Ok(Json(CherryPickResponse {
            success: false,
            count: 0,
            skipped: 0,
            error: Some(format!("Failed to checkout {}: {}", local_branch, error_msg)),
        }));
    }
    
    // 4. 执行git cherry-pick
    let mut success_count = 0;
    let mut skipped_count = 0;
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
            let stdout_msg = String::from_utf8_lossy(&output.stdout).to_string();
            
            // 检查是否是空提交的情况（提交已存在或无变化）
            let is_empty_commit = error_msg.contains("nothing to commit") 
                || error_msg.contains("empty")
                || error_msg.contains("The previous cherry-pick is now empty")
                || stdout_msg.contains("nothing to commit");
            
            if is_empty_commit {
                // 跳过空提交，使用 --skip 继续
                let _ = Command::new("git")
                    .arg("-C")
                    .arg(&repo_path)
                    .arg("cherry-pick")
                    .arg("--skip")
                    .output()
                    .await;
                skipped_count += 1;
                continue;
            }
            
            // 其他错误，尝试abort并返回失败
            let _ = Command::new("git")
                .arg("-C")
                .arg(&repo_path)
                .arg("cherry-pick")
                .arg("--abort")
                .output()
                .await;
            
            return Ok(Json(CherryPickResponse {
                success: false,
                count: success_count,
                skipped: skipped_count,
                error: Some(format!("Failed at commit {}: {}", commit_oid, error_msg)),
            }));
        }
    }
    
    Ok(Json(CherryPickResponse {
        success: true,
        count: success_count,
        skipped: skipped_count,
        error: None,
    }))
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

    // 处理分支名称：如果包含 origin/ 前缀，去掉它
    let branch_name = if req.branch.starts_with("origin/") {
        req.branch.strip_prefix("origin/").unwrap()
    } else {
        &req.branch
    };
    
    // 执行git push
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("push")
        .arg("origin")
        .arg(branch_name)
        .output()
        .await?;
    
    if output.status.success() {
        // 触发索引更新，确保前端 Diff 视图能及时刷新
        let worker = IndexWorker::new(
            ctx.config.clone(),
            ctx.repository_store.clone(),
            ctx.commit_store.clone(),
            ctx.branch_store.clone(),
            ctx.git_client.clone(),
        );
        // 忽略索引错误，不影响 Push 结果
        if let Err(e) = worker.index_repository(repo.id, &repo_path).await {
            tracing::error!("Failed to index repository after push: {}", e);
        }

        Ok(Json(PushResponse {
            success: true,
            error: None,
        }))
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr).to_string();
        
        // 如果是因为远程有更新导致失败（non-fast-forward），尝试 pull --rebase
        if error_msg.contains("rejected") || error_msg.contains("fetch first") {
            // 尝试 pull --rebase
            let pull_output = Command::new("git")
                .arg("-C")
                .arg(&repo_path)
                .arg("pull")
                .arg("--rebase")
                .arg("origin")
                .arg(branch_name)
                .output()
                .await?;
// 触发索引更新
                    let worker = IndexWorker::new(
                        ctx.config.clone(),
                        ctx.repository_store.clone(),
                        ctx.commit_store.clone(),
                        ctx.branch_store.clone(),
                        ctx.git_client.clone(),
                    );
                    if let Err(e) = worker.index_repository(repo.id, &repo_path).await {
                        tracing::error!("Failed to index repository after auto-rebase push: {}", e);
                    }

                    
            if pull_output.status.success() {
                // Rebase 成功，再次尝试 Push
                let push_retry = Command::new("git")
                    .arg("-C")
                    .arg(&repo_path)
                    .arg("push")
                    .arg("origin")
                    .arg(branch_name)
                    .output()
                    .await?;
                
                if push_retry.status.success() {
                    return Ok(Json(PushResponse {
                        success: true,
                        error: None,
                    }));
                } else {
                    let retry_err = String::from_utf8_lossy(&push_retry.stderr).to_string();
                    return Ok(Json(PushResponse {
                        success: false,
                        error: Some(format!("Auto-rebase succeeded but push failed again: {}", retry_err)),
                    }));
                }
            } else {
                // Rebase 失败（可能有冲突），尝试 abort
                let _ = Command::new("git")
                    .arg("-C")
                    .arg(&repo_path)
                    .arg("rebase")
                    .arg("--abort")
                    .output()
                    .await;
                
                let pull_err = String::from_utf8_lossy(&pull_output.stderr).to_string();
                return Ok(Json(PushResponse {
                    success: false,
                    error: Some(format!("Remote has changes. Auto-rebase failed (likely conflicts): {}", pull_err)),
                }));
            }
        }

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

/// API: Merge source branch into target branch
#[derive(Deserialize)]
pub struct MergeRequest {
    source_branch: String,
    target_branch: String,
}

#[derive(Serialize)]
pub struct MergeResponse {
    success: bool,
    message: Option<String>,
    error: Option<String>,
}

pub async fn api_merge(
    State(ctx): State<Arc<AppContext>>,
    Path(repo_name): Path<String>,
    Json(req): Json<MergeRequest>,
) -> Result<Json<MergeResponse>> {
    let repo = ctx.repository_store
        .find_by_name(&repo_name)
        .await?
        .ok_or_else(|| crate::shared::error::GitxError::RepositoryNotFound(repo_name.clone()))?;
    
    let repo_path = std::path::PathBuf::from(&repo.path);
    
    // 1. Fetch latest from remote
    let fetch_output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("fetch")
        .arg("origin")
        .output()
        .await?;
    
    if !fetch_output.status.success() {
        let error_msg = String::from_utf8_lossy(&fetch_output.stderr).to_string();
        return Ok(Json(MergeResponse {
            success: false,
            message: None,
            error: Some(format!("Failed to fetch: {}", error_msg)),
        }));
    }
    
    // 2. Process branch names (remove origin/ prefix if present)
    let source_branch = if req.source_branch.starts_with("origin/") {
        req.source_branch.clone()
    } else {
        format!("origin/{}", req.source_branch)
    };
    
    let local_target = if req.target_branch.starts_with("origin/") {
        req.target_branch.strip_prefix("origin/").unwrap().to_string()
    } else {
        req.target_branch.clone()
    };
    
    // 3. Checkout target branch
    let checkout_output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("checkout")
        .arg("-B")
        .arg(&local_target)
        .arg(format!("origin/{}", local_target))
        .output()
        .await?;
    
    if !checkout_output.status.success() {
        let error_msg = String::from_utf8_lossy(&checkout_output.stderr).to_string();
        return Ok(Json(MergeResponse {
            success: false,
            message: None,
            error: Some(format!("Failed to checkout {}: {}", local_target, error_msg)),
        }));
    }
    
    // 4. Perform merge
    let merge_output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("merge")
        .arg(&source_branch)
        .arg("--no-edit")
        .output()
        .await?;
    
    if merge_output.status.success() {
        let stdout_msg = String::from_utf8_lossy(&merge_output.stdout).to_string();
        
        // Check if already up-to-date
        if stdout_msg.contains("Already up to date") || stdout_msg.contains("Already up-to-date") {
            return Ok(Json(MergeResponse {
                success: true,
                message: Some("Already up to date. No merge needed.".to_string()),
                error: None,
            }));
        }
        
        Ok(Json(MergeResponse {
            success: true,
            message: Some(format!("Successfully merged {} into {}", req.source_branch, local_target)),
            error: None,
        }))
    } else {
        let error_msg = String::from_utf8_lossy(&merge_output.stderr).to_string();
        let stdout_msg = String::from_utf8_lossy(&merge_output.stdout).to_string();
        
        // Check for merge conflicts
        if error_msg.contains("CONFLICT") || stdout_msg.contains("CONFLICT") {
            // Abort the merge to leave repo in clean state
            let _ = Command::new("git")
                .arg("-C")
                .arg(&repo_path)
                .arg("merge")
                .arg("--abort")
                .output()
                .await;
            
            return Ok(Json(MergeResponse {
                success: false,
                message: None,
                error: Some("Merge conflict detected. Please resolve conflicts manually.".to_string()),
            }));
        }
        
        Ok(Json(MergeResponse {
            success: false,
            message: None,
            error: Some(format!("Merge failed: {}", error_msg)),
        }))
    }
}
