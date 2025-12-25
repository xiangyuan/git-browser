use async_trait::async_trait;
use git2::{Oid, Repository, Sort, DiffOptions, DiffFormat};
use std::path::Path;
use crate::ports::git::{
    GitPort, FetchResult, GitCommit, GitBranch, GitTag, 
    GitCommitDetail, GitDiff, GitDiffPatch
};
use crate::shared::result::Result;
use crate::shared::error::GitxError;

/// Git 客户端实现（基于 git2-rs）
pub struct Git2Client {
    // 可以添加配置，如 SSH 密钥路径等
}

impl Git2Client {
    pub fn new() -> Self {
        Self {}
    }

    /// 在线程池中执行阻塞的 Git 操作
    async fn run_blocking<F, T>(f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        tokio::task::spawn_blocking(f)
            .await
            .map_err(|e| GitxError::Internal(format!("Task join error: {}", e)))?
    }

    /// Git 凭证回调（SSH 密钥认证）
    fn git_credentials(
        _url: &str,
        username: Option<&str>,
        _allowed: git2::CredentialType,
    ) -> std::result::Result<git2::Cred, git2::Error> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        git2::Cred::ssh_key(
            username.unwrap_or("git"),
            Some(Path::new(&format!("{}/.ssh/id_rsa.pub", home))),
            Path::new(&format!("{}/.ssh/id_rsa", home)),
            None,
        )
    }
}

impl Default for Git2Client {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GitPort for Git2Client {
    async fn fetch_repository(&self, path: &Path) -> Result<FetchResult> {
        let path = path.to_path_buf();
        
        Self::run_blocking(move || {
            let repo = Repository::open(&path)?;
            let mut remote = repo.find_remote("origin")?;
            
            let mut callbacks = git2::RemoteCallbacks::new();
            callbacks.credentials(Self::git_credentials);
            
            // 进度回调
            callbacks.transfer_progress(|stats| {
                if stats.received_objects() == stats.total_objects() {
                    tracing::debug!(
                        "Resolving deltas {}/{}",
                        stats.indexed_deltas(),
                        stats.total_deltas()
                    );
                } else if stats.total_objects() > 0 {
                    tracing::debug!(
                        "Received {}/{} objects",
                        stats.received_objects(),
                        stats.total_objects()
                    );
                }
                true
            });

            let mut fetch_options = git2::FetchOptions::new();
            fetch_options.remote_callbacks(callbacks);
            
            // Fetch all refs
            remote.fetch(&[] as &[&str], Some(&mut fetch_options), None)?;
            
            // 获取更新的分支列表
            let branches_updated: Vec<String> = repo
                .branches(Some(git2::BranchType::Remote))?
                .filter_map(|b| b.ok())
                .filter_map(|(branch, _)| branch.name().ok().flatten().map(String::from))
                .collect();

            Ok(FetchResult {
                commits_fetched: 0, // TODO: 计算新增提交数
                branches_updated,
            })
        })
        .await
    }

    async fn get_commits(
        &self,
        path: &Path,
        branch: &str,
        limit: usize,
        since_oid: Option<&str>,
    ) -> Result<Vec<GitCommit>> {
        let path = path.to_path_buf();
        let branch = branch.to_string();
        let since_oid = since_oid.map(String::from);
        
        Self::run_blocking(move || {
            let repo = Repository::open(&path)?;
            // 检查分支是否存在
            let _reference = repo.find_reference(&branch)?;
            
            let mut revwalk = repo.revwalk()?;
            revwalk.set_sorting(Sort::TIME)?;
            revwalk.push_ref(&branch)?;
            
            let mut commits = Vec::new();
            let since_oid_parsed = if let Some(ref oid_str) = since_oid {
                Some(Oid::from_str(oid_str)?)
            } else {
                None
            };
            
            for (idx, oid) in revwalk.enumerate() {
                if idx >= limit {
                    break;
                }
                
                let oid = oid?;
                
                // 如果找到起始点，停止
                if let Some(since) = since_oid_parsed {
                    if oid == since {
                        break;
                    }
                }
                
                let commit = repo.find_commit(oid)?;
                
                // 跳过合并提交
                if commit.parent_count() > 1 {
                    continue;
                }
                
                let author = commit.author();
                let committer = commit.committer();
                
                commits.push(GitCommit {
                    oid: commit.id().to_string(),
                    author_name: String::from_utf8_lossy(author.name_bytes()).to_string(),
                    author_email: String::from_utf8_lossy(author.email_bytes()).to_string(),
                    author_time: author.when().seconds(),
                    committer_name: String::from_utf8_lossy(committer.name_bytes()).to_string(),
                    committer_email: String::from_utf8_lossy(committer.email_bytes()).to_string(),
                    committer_time: committer.when().seconds(),
                    summary: commit.summary().unwrap_or("").to_string(),
                    message: commit.body().map(String::from),
                    parent_oids: commit.parent_ids().map(|id| id.to_string()).collect(),
                });
            }
            
            Ok(commits)
        })
        .await
    }

    async fn list_branches(&self, path: &Path) -> Result<Vec<GitBranch>> {
        let path = path.to_path_buf();
        
        Self::run_blocking(move || {
            let repo = Repository::open(&path)?;
            let head = repo.head().ok();
            let head_name = head.as_ref().and_then(|h| h.name()).map(String::from);
            
            let mut branches = Vec::new();
            
            for branch in repo.branches(Some(git2::BranchType::Remote))? {
                let (branch, _) = match branch {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!("Skipping invalid branch: {}", e);
                        continue;
                    }
                };
                
                // 跳过没有名称的分支
                let name = match branch.name() {
                    Ok(Some(n)) => n.to_string(),
                    _ => {
                        tracing::warn!("Skipping branch with invalid name");
                        continue;
                    }
                };
                
                // 跳过没有目标的分支
                let target = match branch.get().target() {
                    Some(t) => t,
                    None => {
                        tracing::warn!("Skipping branch {} without target", name);
                        continue;
                    }
                };
                
                branches.push(GitBranch {
                    name: name.clone(),
                    target_oid: target.to_string(),
                    is_head: head_name.as_ref().map_or(false, |h| h == &name),
                });
            }
            
            Ok(branches)
        })
        .await
    }

    async fn list_tags(&self, path: &Path) -> Result<Vec<GitTag>> {
        let path = path.to_path_buf();
        
        Self::run_blocking(move || {
            let repo = Repository::open(&path)?;
            let mut tags = Vec::new();
            
            for tag_name in repo.tag_names(None)?.iter().flatten() {
                let reference = repo.find_reference(&format!("refs/tags/{}", tag_name))?;
                let target_oid = reference.target().ok_or(GitxError::InvalidRef)?;
                
                // 尝试获取标注标签信息
                let (tagger_name, tagger_email, tagger_time, message) = if let Ok(tag) = reference.peel_to_tag() {
                    let tagger = tag.tagger();
                    (
                        tagger.as_ref().map(|t| String::from_utf8_lossy(t.name_bytes()).to_string()),
                        tagger.as_ref().map(|t| String::from_utf8_lossy(t.email_bytes()).to_string()),
                        tagger.as_ref().map(|t| t.when().seconds()),
                        tag.message().map(String::from),
                    )
                } else {
                    (None, None, None, None)
                };
                
                tags.push(GitTag {
                    name: tag_name.to_string(),
                    target_oid: target_oid.to_string(),
                    tagger_name,
                    tagger_email,
                    tagger_time,
                    message,
                });
            }
            
            Ok(tags)
        })
        .await
    }

    async fn get_commit_detail(&self, path: &Path, oid: &str) -> Result<GitCommitDetail> {
        let path = path.to_path_buf();
        let oid_str = oid.to_string();
        
        Self::run_blocking(move || {
            let repo = Repository::open(&path)?;
            let oid = Oid::from_str(&oid_str)?;
            let commit = repo.find_commit(oid)?;
            
            // 获取提交基本信息
            let author = commit.author();
            let committer = commit.committer();
            
            let git_commit = GitCommit {
                oid: commit.id().to_string(),
                author_name: String::from_utf8_lossy(author.name_bytes()).to_string(),
                author_email: String::from_utf8_lossy(author.email_bytes()).to_string(),
                author_time: author.when().seconds(),
                committer_name: String::from_utf8_lossy(committer.name_bytes()).to_string(),
                committer_email: String::from_utf8_lossy(committer.email_bytes()).to_string(),
                committer_time: committer.when().seconds(),
                summary: commit.summary().unwrap_or("").to_string(),
                message: commit.body().map(String::from),
                parent_oids: commit.parent_ids().map(|id| id.to_string()).collect(),
            };
            
            // 计算 diff
            let tree = commit.tree()?;
            let parent_tree = if commit.parent_count() > 0 {
                Some(commit.parent(0)?.tree()?)
            } else {
                None
            };
            
            let diff = repo.diff_tree_to_tree(
                parent_tree.as_ref(),
                Some(&tree),
                Some(&mut DiffOptions::new()),
            )?;
            
            // 获取 diff 统计信息
            let stats = diff.stats()?;
            let diff_stats = format!(
                "{} files changed, {} insertions(+), {} deletions(-)",
                stats.files_changed(),
                stats.insertions(),
                stats.deletions()
            );
            
            // 生成 diff HTML（保持git格式）
            let mut diff_html = String::new();
            let mut diff_plain = Vec::new();
            
            diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
                let content = String::from_utf8_lossy(line.content());
                diff_plain.extend_from_slice(line.content());
                
                // HTML转义
                let escaped = content
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");
                
                match line.origin() {
                    '+' => diff_html.push_str(&format!("<span class=\"diff-add-line\">{}</span>", escaped)),
                    '-' => diff_html.push_str(&format!("<span class=\"diff-remove-line\">{}</span>", escaped)),
                    ' ' => diff_html.push_str(&format!("<span class=\"diff-context\"> {}</span>", escaped)),
                    _ => diff_html.push_str(&escaped),
                }
                true
            })?;
            
            Ok(GitCommitDetail {
                commit: git_commit,
                diff_stats,
                diff_html,
                diff_plain,
            })
        })
        .await
    }

    async fn compare_commits(
        &self,
        path: &Path,
        from_oid: &str,
        to_oid: &str,
    ) -> Result<GitDiff> {
        let path = path.to_path_buf();
        let from_oid_str = from_oid.to_string();
        let to_oid_str = to_oid.to_string();
        
        Self::run_blocking(move || {
            let repo = Repository::open(&path)?;
            let from_oid = Oid::from_str(&from_oid_str)?;
            let to_oid = Oid::from_str(&to_oid_str)?;
            
            let from_commit = repo.find_commit(from_oid)?;
            let to_commit = repo.find_commit(to_oid)?;
            
            let from_tree = from_commit.tree()?;
            let to_tree = to_commit.tree()?;
            
            let diff = repo.diff_tree_to_tree(
                Some(&from_tree),
                Some(&to_tree),
                Some(&mut DiffOptions::new()),
            )?;
            
            let stats = diff.stats()?;
            let stats_str = format!(
                "{} files changed, {} insertions(+), {} deletions(-)",
                stats.files_changed(),
                stats.insertions(),
                stats.deletions()
            );
            
            let mut patches = Vec::new();
            
            diff.print(DiffFormat::Patch, |delta, _hunk, _line| {
                let old_path = delta.old_file().path().map(|p| p.display().to_string());
                let new_path = delta.new_file().path().map(|p| p.display().to_string());
                let status = format!("{:?}", delta.status());
                
                patches.push(GitDiffPatch {
                    old_path,
                    new_path,
                    status,
                    hunks: vec![], // TODO: 收集 hunks
                });
                
                true
            })?;
            
            Ok(GitDiff {
                stats: stats_str,
                patches,
            })
        })
        .await
    }
    
    async fn get_branch_diff_commits(
        &self,
        path: &Path,
        old_branch: &str,
        new_branch: &str,
        limit: usize,
    ) -> Result<Vec<GitCommit>> {
        let path = path.to_path_buf();
        let old_branch = old_branch.to_string();
        let new_branch = new_branch.to_string();
        
        Self::run_blocking(move || {
            use std::process::Command;
            
            // 直接使用git命令行，确保行为一致
            // git log old_branch..new_branch --oneline --no-merges --format=%H
            let output = Command::new("git")
                .current_dir(&path)
                .args(&[
                    "log",
                    &format!("{}..{}", old_branch, new_branch),
                    "--no-merges",
                    &format!("-{}", limit),
                    "--format=%H",
                ])
                .output()
                .map_err(|e| GitxError::Internal(format!("Failed to run git command: {}", e)))?;
            
            if !output.status.success() {
                return Err(GitxError::Internal(format!(
                    "Git command failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                )));
            }
            
            let oids_str = String::from_utf8_lossy(&output.stdout);
            let repo = Repository::open(&path)?;
            let mut commits = Vec::new();
            
            for line in oids_str.lines() {
                let oid_str = line.trim();
                if oid_str.is_empty() {
                    continue;
                }
                
                let oid = Oid::from_str(oid_str)?;
                let commit = repo.find_commit(oid)?;
                
                let author = commit.author();
                let committer = commit.committer();
                
                commits.push(GitCommit {
                    oid: commit.id().to_string(),
                    author_name: String::from_utf8_lossy(author.name_bytes()).to_string(),
                    author_email: String::from_utf8_lossy(author.email_bytes()).to_string(),
                    author_time: author.when().seconds(),
                    committer_name: String::from_utf8_lossy(committer.name_bytes()).to_string(),
                    committer_email: String::from_utf8_lossy(committer.email_bytes()).to_string(),
                    committer_time: committer.when().seconds(),
                    summary: commit.summary().unwrap_or("").to_string(),
                    message: commit.body().map(String::from),
                    parent_oids: commit.parent_ids().map(|id| id.to_string()).collect(),
                });
            }
            
            Ok(commits)
        })
        .await
    }
}
