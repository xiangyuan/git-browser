use async_trait::async_trait;
use std::time::Duration;
use serde::{de::DeserializeOwned, Serialize};
use crate::shared::result::Result;

/// 缓存接口
#[async_trait]
pub trait CachePort: Send + Sync {
    /// 获取缓存值
    async fn get<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>>;

    /// 设置缓存值
    async fn set<T: Serialize + Send + Sync>(&self, key: &str, value: &T, ttl: Duration) -> Result<()>;

    /// 删除缓存
    async fn delete(&self, key: &str) -> Result<()>;

    /// 检查键是否存在
    async fn exists(&self, key: &str) -> Result<bool>;

    /// 清空所有缓存
    async fn clear(&self) -> Result<()>;
}
