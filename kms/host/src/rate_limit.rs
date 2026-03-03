//! Per-API-key sliding window rate limiter.
//!
//! Tracks request timestamps per key. Rejects with 429 when window limit exceeded.
//! Default: 60 requests/minute (configurable via KMS_RATE_LIMIT env var).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const WINDOW_SECS: u64 = 60;

#[derive(Clone)]
pub struct RateLimiter {
    windows: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    limit: usize,
}

impl RateLimiter {
    pub fn new(limit: usize) -> Self {
        Self {
            windows: Arc::new(Mutex::new(HashMap::new())),
            limit,
        }
    }

    pub fn from_env() -> Self {
        let limit = std::env::var("KMS_RATE_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        Self::new(limit)
    }

    /// Check if request is allowed. Returns Ok(remaining) or Err(limit).
    pub fn check(&self, key: &str) -> Result<usize, usize> {
        let mut windows = self.windows.lock().unwrap();
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(WINDOW_SECS);

        let timestamps = windows.entry(key.to_string()).or_insert_with(Vec::new);
        timestamps.retain(|t| *t > cutoff);

        if timestamps.len() >= self.limit {
            Err(self.limit)
        } else {
            timestamps.push(now);
            Ok(self.limit - timestamps.len())
        }
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_under_limit() {
        let rl = RateLimiter::new(3);
        assert!(rl.check("key1").is_ok());
        assert!(rl.check("key1").is_ok());
        assert!(rl.check("key1").is_ok());
    }

    #[test]
    fn rejects_over_limit() {
        let rl = RateLimiter::new(2);
        assert!(rl.check("key1").is_ok());
        assert!(rl.check("key1").is_ok());
        assert!(rl.check("key1").is_err());
    }

    #[test]
    fn separate_keys_independent() {
        let rl = RateLimiter::new(1);
        assert!(rl.check("key1").is_ok());
        assert!(rl.check("key2").is_ok());
        assert!(rl.check("key1").is_err());
        assert!(rl.check("key2").is_err());
    }
}
