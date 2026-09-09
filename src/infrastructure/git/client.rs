use async_trait::async_trait;
use git2::{Oid, Repository, Sort, DiffOptions, DiffFormat};
use std::path::Path;
use crate::ports::git::{
    GitPort, FetchResult, GitCommit, GitBranch, GitTag, 
    GitCommitDetail, GitDiff, GitDiffPatch, DiffRef
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

            // since_oid 语义是 `git log since..HEAD`：隐藏该提交及其祖先。
            // 绝不能“按时间走到 since 就停”——merge 进来的更早提交会被漏掉。
            if let Some(ref oid_str) = since_oid {
                match Oid::from_str(oid_str) {
                    Ok(oid) => {
                        if let Err(e) = revwalk.hide(oid) {
                            tracing::warn!(
                                "Failed to hide since_oid {} on {}: {}; walking without hide",
                                oid_str,
                                branch,
                                e
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Invalid since_oid {}: {}", oid_str, e);
                    }
                }
            }
            
            let mut commits = Vec::new();
            
            for oid in revwalk {
                let oid = oid?;
                let commit = repo.find_commit(oid)?;
                
                // 跳过合并提交
                if commit.parent_count() > 1 {
                    continue;
                }

                if commits.len() >= limit {
                    break;
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

    async fn is_ancestor(
        &self,
        path: &Path,
        ancestor_oid: &str,
        descendant_oid: &str,
    ) -> Result<bool> {
        let path = path.to_path_buf();
        let ancestor_oid = ancestor_oid.to_string();
        let descendant_oid = descendant_oid.to_string();

        Self::run_blocking(move || {
            let repo = Repository::open(&path)?;
            Ok(is_ancestor_sync(&repo, &ancestor_oid, &descendant_oid))
        })
        .await
    }

    async fn rev_parse(&self, path: &Path, spec: &str) -> Result<String> {
        let path = path.to_path_buf();
        let spec = spec.to_string();

        Self::run_blocking(move || {
            let repo = Repository::open(&path)?;
            let obj = repo.revparse_single(&spec)?;
            Ok(obj.id().to_string())
        })
        .await
    }

    async fn inspect_diff_ref(&self, path: &Path, branch: &str) -> Result<DiffRef> {
        let path = path.to_path_buf();
        let branch = branch.to_string();

        Self::run_blocking(move || {
            let repo = Repository::open(&path)?;
            Ok(inspect_diff_ref_sync(&repo, &branch))
        })
        .await
    }
}

/// `ancestor` 是否为 `descendant` 的祖先（OID 相等视为是）
fn is_ancestor_sync(repo: &Repository, ancestor_oid: &str, descendant_oid: &str) -> bool {
    let Ok(ancestor) = Oid::from_str(ancestor_oid) else {
        return false;
    };
    let Ok(descendant) = Oid::from_str(descendant_oid) else {
        return false;
    };
    if ancestor == descendant {
        return true;
    }
    repo.graph_descendant_of(descendant, ancestor).unwrap_or(false)
}

/// 对比时优先使用“领先远程”的本地分支，这样本地 merge 后立刻能反映最新差异。
fn inspect_diff_ref_sync(repo: &Repository, branch: &str) -> DiffRef {
    let short = branch.strip_prefix("origin/").unwrap_or(branch);
    let remote_spec = if branch.starts_with("origin/") {
        branch.to_string()
    } else {
        format!("origin/{}", short)
    };

    let local_oid = repo
        .revparse_single(&format!("refs/heads/{}", short))
        .ok()
        .map(|obj| obj.id());
    let remote_oid = repo
        .revparse_single(&format!("refs/remotes/origin/{}", short))
        .ok()
        .map(|obj| obj.id());

    match (local_oid, remote_oid) {
        (Some(local), Some(remote)) if local == remote => DiffRef {
            spec: remote_spec,
            local_ahead: false,
        },
        (Some(local), Some(remote)) => {
            let ahead = repo.graph_descendant_of(local, remote).unwrap_or(false);
            if ahead {
                DiffRef {
                    spec: short.to_string(),
                    local_ahead: true,
                }
            } else {
                DiffRef {
                    spec: remote_spec,
                    local_ahead: false,
                }
            }
        }
        (Some(_), None) => DiffRef {
            spec: short.to_string(),
            local_ahead: true,
        },
        (None, _) => DiffRef {
            spec: remote_spec,
            local_ahead: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{RepositoryInitOptions, Signature, Time};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    struct TempGit {
        path: PathBuf,
    }

    impl Drop for TempGit {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn init_temp() -> TempGit {
        let path = std::env::temp_dir().join(format!("gitx-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        let mut opts = RepositoryInitOptions::new();
        opts.initial_head("main");
        Repository::init_opts(&path, &opts).unwrap();
        let repo = Repository::open(&path).unwrap();
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "Test").unwrap();
        cfg.set_str("user.email", "test@example.com").unwrap();
        cfg.set_bool("commit.gpgsign", false).unwrap();
        TempGit { path }
    }

    fn git(path: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(path)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .args(["-c", "commit.gpgsign=false", "-c", "user.name=Test", "-c", "user.email=test@example.com"])
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed: stdout={} stderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn write_commit(path: &Path, message: &str, seconds: i64) -> String {
        let repo = Repository::open(path).unwrap();
        let filename = format!("{}.txt", message.replace(' ', "-"));
        fs::write(path.join(&filename), format!("{}\n{}", message, seconds)).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(&filename)).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = Signature::new("Test", "test@example.com", &Time::new(seconds, 0)).unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let oid = if let Some(ref p) = parent {
            repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[p]).unwrap()
        } else {
            repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[]).unwrap()
        };
        oid.to_string()
    }

    fn setup_merge_repo() -> (TempGit, String, String, String) {
        let tmp = init_temp();
        let path = tmp.path.as_path();

        let base = write_commit(path, "base", 1000);
        git(path, &["checkout", "-b", "feature"]);
        let old_feature = write_commit(path, "old-feature", 1100);
        git(path, &["checkout", "main"]);
        let main_new = write_commit(path, "main-new", 5000);
        git(path, &["checkout", "feature"]);
        let _new_feature = write_commit(path, "new-feature", 6000);
        git(path, &["checkout", "main"]);
        git(path, &["merge", "feature", "--no-edit"]);

        (tmp, base, old_feature, main_new)
    }

    #[tokio::test]
    async fn incremental_index_after_merge_includes_older_side_commits() {
        let (tmp, base, old_feature, main_new) = setup_merge_repo();
        let client = Git2Client::new();

        let commits = client
            .get_commits(&tmp.path, "refs/heads/main", 100, Some(&main_new))
            .await
            .unwrap();
        let oids: Vec<&str> = commits.iter().map(|c| c.oid.as_str()).collect();

        assert!(
            oids.contains(&old_feature.as_str()),
            "merged-in older commit must be indexed; got {:?}",
            oids
        );
        assert!(
            commits.iter().any(|c| c.summary == "new-feature"),
            "newer feature commit must be indexed; got {:?}",
            commits.iter().map(|c| c.summary.clone()).collect::<Vec<_>>()
        );
        assert!(
            !oids.contains(&base.as_str()),
            "common ancestor should be hidden; got {:?}",
            oids
        );
        assert!(
            !oids.contains(&main_new.as_str()),
            "old tip should be hidden; got {:?}",
            oids
        );
        assert!(
            commits.iter().all(|c| c.summary != ""),
            "merge commit itself should be skipped"
        );
    }

    #[tokio::test]
    async fn branch_diff_is_empty_after_merge() {
        let (tmp, _, _, _) = setup_merge_repo();
        let client = Git2Client::new();

        let commits = client
            .get_branch_diff_commits(&tmp.path, "main", "feature", 100)
            .await
            .unwrap();
        assert!(
            commits.is_empty(),
            "after merge, feature should have no unique commits; got {:?}",
            commits.iter().map(|c| c.summary.clone()).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn inspect_diff_ref_prefers_local_when_ahead_of_origin() {
        let tmp = init_temp();
        let path = tmp.path.as_path();
        let remote_tip = write_commit(path, "on-remote", 1000);
        git(path, &["update-ref", "refs/remotes/origin/main", &remote_tip]);

        let _local = write_commit(path, "local-ahead", 2000);
        let repo = Repository::open(path).unwrap();
        let diff_ref = inspect_diff_ref_sync(&repo, "origin/main");

        assert!(diff_ref.local_ahead);
        assert_eq!(diff_ref.spec, "main");
    }

    #[tokio::test]
    async fn inspect_diff_ref_uses_origin_when_in_sync() {
        let tmp = init_temp();
        let path = tmp.path.as_path();
        let tip = write_commit(path, "synced", 1000);
        git(path, &["update-ref", "refs/remotes/origin/main", &tip]);

        let repo = Repository::open(path).unwrap();
        let diff_ref = inspect_diff_ref_sync(&repo, "origin/main");

        assert!(!diff_ref.local_ahead);
        assert_eq!(diff_ref.spec, "origin/main");
    }

    #[test]
    fn ancestor_check_handles_equal_and_descendant() {
        let (tmp, base, _, main_new) = setup_merge_repo();
        let repo = Repository::open(&tmp.path).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap().id().to_string();

        assert!(is_ancestor_sync(&repo, &base, &head));
        assert!(is_ancestor_sync(&repo, &main_new, &head));
        assert!(is_ancestor_sync(&repo, &head, &head));
        assert!(!is_ancestor_sync(&repo, &head, &base));
    }
}
