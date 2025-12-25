use serde::{Deserialize, Serialize};
use std::fmt;

/// 仓库 ID 值对象
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepositoryId(pub i64);

impl RepositoryId {
    pub fn new(id: i64) -> Self {
        Self(id)
    }

    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

impl fmt::Display for RepositoryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 提交 SHA 值对象
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommitSha(String);

impl CommitSha {
    pub fn new(sha: String) -> Result<Self, String> {
        if sha.len() != 40 {
            return Err(format!("Invalid commit SHA length: {}", sha.len()));
        }
        
        if !sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("Invalid commit SHA: contains non-hex characters".to_string());
        }
        
        Ok(Self(sha))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommitSha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 分支名称值对象
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchName(String);

impl BranchName {
    pub fn new(name: String) -> Self {
        Self(name)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 移除 refs/remotes/origin/ 前缀
    pub fn short_name(&self) -> &str {
        self.0
            .strip_prefix("refs/remotes/origin/")
            .or_else(|| self.0.strip_prefix("refs/heads/"))
            .unwrap_or(&self.0)
    }
}

impl From<String> for BranchName {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for BranchName {
    fn from(s: &str) -> Self {
        Self::new(s.to_string())
    }
}

impl fmt::Display for BranchName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
