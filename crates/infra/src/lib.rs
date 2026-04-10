//! Persistence: JSONL journal, snapshot, load/sort/replay helpers.

mod canon;
mod error;
mod journal;
mod snapshot;

pub use canon::to_canonical_line;
pub use error::JournalError;
pub use journal::{
    load_and_sort, repair_journal, verify_contiguous_sequence, Journal, JournalAppend,
    JournalAppendOutcome,
};
pub use snapshot::Snapshot;
