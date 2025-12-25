use async_trait::async_trait;
use std::path::Path;
use crate::shared::result::Result;

/// Git 操作接口
#[async_trait]
pub trait GitPort: Send + Sync {
    /// 拉取仓库更新
    async fn fetch_repository(&self, path: &Path) -> Result<FetchResult>;

    /// 获取提交列表
    async fn get_commits(
        &self,
        path: &Path,
        branch: &str,
        limit: usize,
        since_oid: Option<&str>,
    ) -> Result<Vec<GitCommit>>;

    /// 获取所有分支
    async fn list_branches(&self, path: &Path) -> Result<Vec<GitBranch>>;

    /// 获取所有标签
    async fn list_tags(&self, path: &Path) -> Result<Vec<GitTag>>;

    /// 获取提交详情（包含 diff）
    async fn get_commit_detail(&self, path: &Path, oid: &str) -> Result<GitCommitDetail>;

    /// 比较两个提交
    async fn compare_commits(
        &self,
        path: &Path,
        from_oid: &str,
        to_oid: &str,
    ) -> Result<GitDiff>;
    
    /// 获取两个分支之间的差异commits（类似 git log old_branch..new_branch）
    /// 返回在new_branch但不在old_branch的commits
    async fn get_branch_diff_commits(
        &self,
        path: &Path,
        old_branch: &str,
        new_branch: &str,
        limit: usize,
    ) -> Result<Vec<GitCommit>>;
}

/// Fetch 操作结果
#[derive(Debug)]
pub struct FetchResult {
    pub commits_fetched: usize,
    pub branches_updated: Vec<String>,
}

/// Git 提交信息（从 git2 提取）
#[derive(Debug, Clone)]
pub struct GitCommit {
    pub oid: String,
    pub author_name: String,
    pub author_email: String,
    pub author_time: i64,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_time: i64,
    pub summary: String,
    pub message: Option<String>,
    pub parent_oids: Vec<String>,
}

/// Git 分支信息
#[derive(Debug, Clone)]
pub struct GitBranch {
    pub name: String,
    pub target_oid: String,
    pub is_head: bool,
}

/// Git 标签信息
#[derive(Debug, Clone)]
pub struct GitTag {
    pub name: String,
    pub target_oid: String,
    pub tagger_name: Option<String>,
    pub tagger_email: Option<String>,
    pub tagger_time: Option<i64>,
    pub message: Option<String>,
}

/// 提交详情（包含 diff）
#[derive(Debug)]
pub struct GitCommitDetail {
    pub commit: GitCommit,
    pub diff_stats: String,
    pub diff_html: String,
    pub diff_plain: Vec<u8>,
}

/// Diff 信息
#[derive(Debug)]
pub struct GitDiff {
    pub stats: String,
    pub patches: Vec<GitDiffPatch>,
}

/// Diff Patch
#[derive(Debug)]
pub struct GitDiffPatch {
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub status: String,
    pub hunks: Vec<String>,
}
