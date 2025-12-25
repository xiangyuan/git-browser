-- 仓库表
CREATE TABLE IF NOT EXISTS repositories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    path TEXT UNIQUE NOT NULL,
    description TEXT,
    default_branch TEXT NOT NULL DEFAULT 'main',
    last_synced_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_repositories_path ON repositories(path);
CREATE INDEX IF NOT EXISTS idx_repositories_name ON repositories(name);

-- 提交表
CREATE TABLE IF NOT EXISTS commits (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repository_id INTEGER NOT NULL,
    oid TEXT NOT NULL,
    branch TEXT NOT NULL,
    author_name TEXT NOT NULL,
    author_email TEXT NOT NULL,
    author_time INTEGER NOT NULL,
    committer_name TEXT NOT NULL,
    committer_email TEXT NOT NULL,
    committer_time INTEGER NOT NULL,
    summary TEXT NOT NULL,
    message TEXT,
    parent_oids TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (repository_id) REFERENCES repositories(id) ON DELETE CASCADE,
    UNIQUE(repository_id, oid, branch)
);

CREATE INDEX IF NOT EXISTS idx_commits_repository_branch ON commits(repository_id, branch, author_time DESC);
CREATE INDEX IF NOT EXISTS idx_commits_oid ON commits(oid);
CREATE INDEX IF NOT EXISTS idx_commits_author_time ON commits(author_time DESC);

-- 标签表
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repository_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    target_oid TEXT NOT NULL,
    tagger_name TEXT,
    tagger_email TEXT,
    tagger_time INTEGER,
    message TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (repository_id) REFERENCES repositories(id) ON DELETE CASCADE,
    UNIQUE(repository_id, name)
);

CREATE INDEX IF NOT EXISTS idx_tags_repository ON tags(repository_id);
CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name);

-- 分支表
CREATE TABLE IF NOT EXISTS branches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repository_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    target_oid TEXT NOT NULL,
    is_default BOOLEAN NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (repository_id) REFERENCES repositories(id) ON DELETE CASCADE,
    UNIQUE(repository_id, name)
);

CREATE INDEX IF NOT EXISTS idx_branches_repository ON branches(repository_id);
