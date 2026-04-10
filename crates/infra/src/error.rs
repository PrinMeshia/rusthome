use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum JournalError {
    #[error("timestamp {ts} regresses last committed {last} (plan §3.4)")]
    TimestampRegressed { ts: i64, last: i64 },
    #[error("journal corrupt at line {line}: {message}")]
    Corrupt { line: usize, message: String },
    #[error("unsupported journal schema_version {found} at line {line} (supported {min}..={max})")]
    UnsupportedSchemaVersion {
        line: usize,
        found: u32,
        min: u32,
        max: u32,
    },
    #[error("duplicate or gap in sequence: expected {expected}, found {found} at line {line}")]
    SequenceMismatch {
        line: usize,
        expected: u64,
        found: u64,
    },
    #[error("IO error on {path:?}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
}

impl JournalError {
    pub fn io(path: PathBuf, source: std::io::Error) -> Self {
        Self::Io { path, source }
    }
}
