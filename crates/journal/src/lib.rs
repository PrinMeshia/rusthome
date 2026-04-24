//! Persisted **journal line** envelope: metadata + flattened [`rusthome_core::Event`].
//!
//! Separated from `rusthome-core` so the domain crate focuses on projection and rules; this crate
//! is the on-disk contract (plan §8).

mod error;
mod line;

pub use error::JournalSchemaError;
pub use line::{
    journal_schema_supported, JournalEntry, MIN_SUPPORTED_JOURNAL_SCHEMA, SCHEMA_VERSION,
};
