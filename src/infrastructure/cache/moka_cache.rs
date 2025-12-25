use async_trait::async_trait;
use moka::future::Cache;
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;
use crate::ports::cache::CachePort;
use crate::shared::result::Result;

/// Moka 内存缓存实现
pub struct MokaCache {
    #[allow(dead_code)]  // 通过方法使用
    cache: Cache<String, Vec<u8>>,
}

impl MokaCache {
    pub fn new(max_capacity: u64, default_ttl: Duration) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_live(default_ttl)
            .build();

        Self { cache }
    }
}

#[async_trait]
impl CachePort for MokaCache {
    async fn get<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>> {
        match self.cache.get(&key.to_string()).await {
            Some(bytes) => {
                let value = bincode::deserialize(&bytes)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    async fn set<T: Serialize + Send + Sync>(&self, key: &str, value: &T, _ttl: Duration) -> Result<()> {
        let bytes = bincode::serialize(value)?;
        
        // Moka 不支持单独设置 TTL，使用全局 TTL
        // 如果需要更灵活的 TTL，考虑使用其他缓存方案
        self.cache.insert(key.to_string(), bytes).await;
        
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.cache.invalidate(&key.to_string()).await;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        Ok(self.cache.contains_key(&key.to_string()))
    }

    async fn clear(&self) -> Result<()> {
        self.cache.invalidate_all();
        Ok(())
    }
}
