// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! Challenge Management for Passkey Authentication
//!
//! Generates and validates time-limited challenges (3 minutes)
//! Used to prevent replay attacks in Passkey authentication

use anyhow::{anyhow, Result};
use optee_utee::Random;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Challenge validity period in seconds (3 minutes)
const CHALLENGE_EXPIRY_SECONDS: u64 = 180;

/// Represents a single challenge with creation time and used status
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Challenge {
    pub challenge: [u8; 32],
    pub created_at: u64, // Unix timestamp (seconds)
    pub used: bool,
}

impl Challenge {
    /// Create a new random challenge
    pub fn new() -> Result<Self> {
        let mut challenge = [0u8; 32];
        Random::generate(&mut challenge as _);

        // Get current time (note: OP-TEE time API may not be available in all modes)
        // For now, we use a simple counter-based timestamp
        let created_at = Self::get_current_time();

        Ok(Self {
            challenge,
            created_at,
            used: false,
        })
    }

    /// Check if challenge is expired
    pub fn is_expired(&self) -> bool {
        let now = Self::get_current_time();
        now - self.created_at > CHALLENGE_EXPIRY_SECONDS
    }

    /// Mark challenge as used
    pub fn mark_used(&mut self) {
        self.used = true;
    }

    /// Simple time function (in production, use OP-TEE time API)
    /// For now, we use a static counter (needs proper time source)
    fn get_current_time() -> u64 {
        // TODO: Integrate with OP-TEE time API
        // For MVP, we use a simple approach:
        // - In real deployment, use optee_utee::time::SystemTime
        // - For now, use a counter-based approach
        0 // Placeholder - will be updated when time API is integrated
    }
}

/// Global challenge manager
pub struct ChallengeManager {
    challenges: HashMap<[u8; 32], Challenge>,
}

impl ChallengeManager {
    /// Create a new challenge manager
    pub fn new() -> Self {
        Self {
            challenges: HashMap::new(),
        }
    }

    /// Generate a new challenge
    pub fn generate_challenge(&mut self) -> Result<Challenge> {
        let challenge = Challenge::new()?;
        self.challenges
            .insert(challenge.challenge, challenge.clone());

        // Clean expired challenges to prevent memory leak
        self.clean_expired();

        Ok(challenge)
    }

    /// Verify and consume a challenge
    pub fn verify_and_consume(&mut self, challenge_bytes: &[u8; 32]) -> Result<()> {
        let challenge = self
            .challenges
            .get_mut(challenge_bytes)
            .ok_or_else(|| anyhow!("Challenge not found"))?;

        if challenge.used {
            return Err(anyhow!("Challenge already used"));
        }

        if challenge.is_expired() {
            return Err(anyhow!("Challenge expired"));
        }

        challenge.mark_used();
        Ok(())
    }

    /// Remove expired challenges (cleanup)
    fn clean_expired(&mut self) {
        self.challenges.retain(|_, c| !c.is_expired());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_challenge_creation() {
        let challenge = Challenge::new().unwrap();
        assert_eq!(challenge.challenge.len(), 32);
        assert!(!challenge.used);
    }

    #[test]
    fn test_challenge_manager() {
        let mut manager = ChallengeManager::new();

        // Generate challenge
        let challenge = manager.generate_challenge().unwrap();

        // Verify challenge
        assert!(manager.verify_and_consume(&challenge.challenge).is_ok());

        // Try to reuse - should fail
        assert!(manager.verify_and_consume(&challenge.challenge).is_err());
    }
}
