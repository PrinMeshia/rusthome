//! Journal line schema validation (plan §8.2).

use thiserror::Error;

/// Persisted line `schema_version` outside the range this binary accepts (plan §8.2).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum JournalSchemaError {
    #[error("unsupported journal schema_version {found} (this build supports {min}..={max})")]
    UnsupportedVersion { found: u32, min: u32, max: u32 },
}
