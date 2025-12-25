-- 为分支差异查询添加优化索引
-- 这个索引支持通过 (author_name, summary, committer_time) 快速匹配相同的逻辑commit
CREATE INDEX IF NOT EXISTS idx_commits_diff_match 
ON commits(repository_id, branch, author_name, summary, committer_time);

-- 优化 committer_time 排序
CREATE INDEX IF NOT EXISTS idx_commits_committer_time 
ON commits(repository_id, branch, committer_time DESC);
