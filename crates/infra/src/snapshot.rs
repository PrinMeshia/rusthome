//! State snapshot (plan §8.4).

use std::fs;
use std::path::Path;

use rusthome_core::State;
use serde::{Deserialize, Serialize};

use crate::error::JournalError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema_version: u32,
    pub last_sequence: u64,
    pub state: State,
    /// Deterministic fingerprint of canonical state (simple V0 hash).
    pub state_hash: u64,
    pub rules_digest: String,
}

impl Snapshot {
    pub fn from_state(
        schema_version: u32,
        last_sequence: u64,
        state: &State,
        rules_digest: impl Into<String>,
    ) -> Self {
        let state_hash = hash_state(state);
        Self {
            schema_version,
            last_sequence,
            state: state.clone(),
            state_hash,
            rules_digest: rules_digest.into(),
        }
    }

    pub fn verify_hash(&self) -> bool {
        self.state_hash == hash_state(&self.state)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), JournalError> {
        let p = path.as_ref();
        let json = serde_json::to_string_pretty(self).map_err(JournalError::Serde)?;
        fs::write(p, json).map_err(|e| JournalError::io(p.to_path_buf(), e))?;
        Ok(())
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, JournalError> {
        let p = path.as_ref();
        let bytes = fs::read(p).map_err(|e| JournalError::io(p.to_path_buf(), e))?;
        let s: Snapshot = serde_json::from_slice(&bytes)?;
        if !s.verify_hash() {
            return Err(JournalError::Corrupt {
                line: 0,
                message: "snapshot state_hash mismatch".into(),
            });
        }
        Ok(s)
    }
}

/// FNV-1a over deterministic JSON of state (V0 stand-in for stable hash).
fn hash_state(state: &State) -> u64 {
    let v = serde_json::to_vec(state).unwrap_or_default();
    fnv1a64(&v)
}

fn fnv1a64(data: &[u8]) -> u64 {
    const OFFSET: u64 = 14695981039346656037;
    const PRIME: u64 = 1099511628211;
    let mut h = OFFSET;
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}
