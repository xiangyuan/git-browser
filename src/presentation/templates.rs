use askama::Template;

/// 主页 - 仓库列表
#[derive(Template)]
#[template(path = "index_simple.html")]
pub struct IndexTemplate {
    pub repositories: Vec<RepoItem>,
}

#[derive(Clone)]
pub struct RepoItem {
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub last_modified: String,
}

/// 仓库概览页
#[derive(Template)]
#[template(path = "summary_simple.html")]
pub struct SummaryTemplate {
    pub repo_name: String,
    pub repo_path: String,
    pub branches: Vec<BranchItem>,
    pub all_branches: Vec<String>,
}

#[derive(Clone)]
pub struct BranchItem {
    pub name: String,
    pub commit_sha: String,
    pub commit_message: String,
    pub author: String,
    pub time: String,
}

/// 提交日志页
#[derive(Template)]
#[template(path = "log_simple.html")]
pub struct LogTemplate {
    pub repo_name: String,
    pub commits: Vec<CommitItem>,
    pub branch: Option<String>,
    pub has_more: bool,
    pub next_offset: usize,
    pub all_branches: Vec<String>,
}

#[derive(Clone)]
pub struct CommitItem {
    pub sha: String,
    pub sha_short: String,
    pub message: String,
    pub summary: String,  // 为模板兼容性添加，与message相同
    pub author_name: String,
    pub author_email: String,
    pub committer_time: String,
}

/// 单个提交详情
#[derive(Template)]
#[template(path = "commit_simple.html")]
pub struct CommitTemplate {
    pub repo_name: String,
    pub commit: CommitDetail,
    pub all_branches: Vec<String>,
}

#[derive(Clone)]
pub struct CommitDetail {
    pub sha: String,
    pub tree: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub author_time: String,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_time: String,
    pub message: String,
    pub diff_stats: String,
    pub diff: String,
}

/// 分支对比页
#[derive(Template)]
#[template(path = "diff_simple.html")]
pub struct DiffTemplate {
    pub repo_name: String,
    pub from_branch: String,
    pub to_branch: String,
    pub branches: Vec<String>,
    pub commits: Vec<CommitItem>,
}