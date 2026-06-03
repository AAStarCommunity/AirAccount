//! Per-API-key sliding window rate limiter (process-in-memory).
//!
//! Tracks request timestamps per key in a `HashMap<String, Vec<Instant>>`.
//! Rejects with 429 when the sliding-window limit is exceeded.
//! Default: 100 requests/minute (configurable via KMS_RATE_LIMIT env var).
//!
//! **Limitations** (by design for this deployment):
//! - State is process-local: counters reset on restart and are not shared across instances.
//!   A deliberate restart or multi-instance deployment can bypass the per-credential limit.
//! - For stronger guarantees, move state to TEE secure storage or a shared DB table.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const WINDOW_SECS: u64 = 60;
/// Hard cap on distinct tracked keys. Prevents memory DoS from flooding with unique key strings.
/// Requests from unseen keys are rejected with the same 429 code when the map is full.
const MAX_TRACKED_KEYS: usize = 10_000;

struct Inner {
    windows: HashMap<String, Vec<Instant>>,
    /// Last time a full-map sweep was performed to evict empty buckets.
    last_full_gc: Instant,
}

#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<Inner>>,
    limit: usize,
}

impl RateLimiter {
    pub fn new(limit: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                windows: HashMap::new(),
                last_full_gc: Instant::now(),
            })),
            limit,
        }
    }

    pub fn from_env() -> Self {
        let limit = std::env::var("KMS_RATE_LIMIT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(100);
        Self::new(limit)
    }

    /// Check if request is allowed. Returns Ok(remaining) or Err(limit).
    pub fn check(&self, key: &str) -> Result<usize, usize> {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(WINDOW_SECS);

        // Full sweep: evict stale timestamps and empty buckets from all keys.
        // Amortized once per WINDOW_SECS to keep per-call cost O(1) for the common
        // case while still preventing unbounded map growth for high-cardinality keys.
        if now.duration_since(inner.last_full_gc) >= Duration::from_secs(WINDOW_SECS) {
            inner.windows.retain(|_, v| {
                v.retain(|t| *t > cutoff);
                !v.is_empty()
            });
            inner.last_full_gc = now;
        }

        // Hard cap: reject unseen keys when map is at capacity to prevent memory DoS.
        if !inner.windows.contains_key(key) && inner.windows.len() >= MAX_TRACKED_KEYS {
            return Err(self.limit);
        }

        // Per-call: only evict stale timestamps for the current key.
        let timestamps = inner
            .windows
            .entry(key.to_string())
            .or_insert_with(Vec::new);
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
