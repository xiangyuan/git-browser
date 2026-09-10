-- find_diff_commits 实际按 (author_time, summary) 匹配，旧索引列对不上
CREATE INDEX IF NOT EXISTS idx_commits_diff_logical
ON commits(repository_id, branch, author_time, summary);
