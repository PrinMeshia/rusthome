//! Shared helpers for integration tests in this crate.

use std::path::PathBuf;

use tempfile::TempDir;

/// Temporary directory containing `events.jsonl` (path returned; file created on first `Journal::open`).
pub fn temp_events_jsonl() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.jsonl");
    (dir, path)
}
