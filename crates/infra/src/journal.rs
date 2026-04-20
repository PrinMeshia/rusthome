//! JSON Lines journal (plan §8.0–8.1).

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use rusthome_core::{Event, JournalEntry, JournalSchemaError, SCHEMA_VERSION};
use uuid::Uuid;

use crate::canon::to_canonical_line;
use crate::error::JournalError;

/// Parameters for an append (plan §15 metadata, §8.0 body).
#[derive(Debug, Clone)]
pub struct JournalAppend {
    pub timestamp: i64,
    pub causal_chain_id: Uuid,
    pub parent_sequence: Option<u64>,
    pub parent_event_id: Option<Uuid>,
    pub rule_id: Option<String>,
    pub event_id: Option<Uuid>,
    pub correlation_id: Option<Uuid>,
    pub trace_id: Option<Uuid>,
    pub event: Event,
}

/// Result of [`Journal::append`] — EPIC 3: command already seen → no new line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalAppendOutcome {
    Committed(JournalEntry),
    DuplicateCommandSkipped { command_id: Uuid },
}

impl JournalAppendOutcome {
    pub fn expect_committed(self) -> JournalEntry {
        match self {
            Self::Committed(e) => e,
            Self::DuplicateCommandSkipped { command_id } => {
                panic!("expected committed journal line, duplicate command_id={command_id}")
            }
        }
    }
}

pub struct Journal {
    path: PathBuf,
    /// `None` if journal is empty (first append accepts any timestamp).
    pub last_timestamp_committed: Option<i64>,
    pub next_sequence: u64,
    /// If true: `sync_all` after each line (§8.1 — durability vs performance).
    pub fsync_after_append: bool,
    /// `command_id`s already present in the journal (append dedup, EPIC 3).
    seen_command_ids: BTreeSet<Uuid>,
}

impl Journal {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, JournalError> {
        let path = path.into();
        if !path.exists() {
            return Ok(Self {
                path,
                last_timestamp_committed: None,
                next_sequence: 0,
                fsync_after_append: false,
                seen_command_ids: BTreeSet::new(),
            });
        }
        let entries = load_and_sort(&path)?;
        let mut last_ts: Option<i64> = None;
        let mut next = 0u64;
        let mut seen_command_ids = BTreeSet::new();
        for (i, e) in entries.iter().enumerate() {
            if e.sequence != next {
                return Err(JournalError::SequenceMismatch {
                    line: i + 1,
                    expected: next,
                    found: e.sequence,
                });
            }
            if let Event::Command(cmd) = &e.event {
                seen_command_ids.insert(cmd.command_id());
            }
            next += 1;
            last_ts = Some(last_ts.map(|t| t.max(e.timestamp)).unwrap_or(e.timestamp));
        }
        Ok(Self {
            path,
            last_timestamp_committed: last_ts,
            next_sequence: next,
            fsync_after_append: false,
            seen_command_ids,
        })
    }

    /// Append with automatic `sequence` and timestamp gate (plan §3.3, §3.4).
    pub fn append(&mut self, a: JournalAppend) -> Result<JournalAppendOutcome, JournalError> {
        if let Event::Command(cmd) = &a.event {
            let id = cmd.command_id();
            if self.seen_command_ids.contains(&id) {
                return Ok(JournalAppendOutcome::DuplicateCommandSkipped { command_id: id });
            }
        }
        if let Some(lt) = self.last_timestamp_committed {
            if a.timestamp < lt {
                return Err(JournalError::TimestampRegressed {
                    ts: a.timestamp,
                    last: lt,
                });
            }
        }
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        self.last_timestamp_committed = Some(
            self.last_timestamp_committed
                .map(|t| t.max(a.timestamp))
                .unwrap_or(a.timestamp),
        );
        let entry = JournalEntry {
            schema_version: SCHEMA_VERSION,
            timestamp: a.timestamp,
            sequence,
            event_id: a.event_id,
            causal_chain_id: a.causal_chain_id,
            parent_sequence: a.parent_sequence,
            parent_event_id: a.parent_event_id,
            rule_id: a.rule_id,
            correlation_id: a.correlation_id,
            trace_id: a.trace_id,
            event: a.event,
        };
        if let Event::Command(cmd) = &entry.event {
            self.seen_command_ids.insert(cmd.command_id());
        }
        self.write_line(&entry)?;
        Ok(JournalAppendOutcome::Committed(entry))
    }

    fn write_line(&self, entry: &JournalEntry) -> Result<(), JournalError> {
        let line = to_canonical_line(entry)?;
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| JournalError::io(self.path.clone(), e))?;
        writeln!(f, "{line}").map_err(|e| JournalError::io(self.path.clone(), e))?;
        f.flush()
            .map_err(|e| JournalError::io(self.path.clone(), e))?;
        if self.fsync_after_append {
            f.sync_all()
                .map_err(|e| JournalError::io(self.path.clone(), e))?;
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Enable `sync_all` after each line (plan §8.1 — safer on crash, slower).
    pub fn set_fsync_after_append(&mut self, enabled: bool) {
        self.fsync_after_append = enabled;
    }
}

/// Load journal, validate each line, sort by `(timestamp, sequence)` (plan §5).
pub fn load_and_sort(path: &Path) -> Result<Vec<JournalEntry>, JournalError> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let f = File::open(path).map_err(|e| JournalError::io(path.to_path_buf(), e))?;
    let mut entries = Vec::new();
    for (i, line) in BufReader::new(f).lines().enumerate() {
        let line = line.map_err(|e| JournalError::io(path.to_path_buf(), e))?;
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let entry: JournalEntry = serde_json::from_str(t).map_err(|e| JournalError::Corrupt {
            line: i + 1,
            message: e.to_string(),
        })?;
        let line_no = i + 1;
        entry.validate_supported_schema().map_err(|e| match e {
            JournalSchemaError::UnsupportedVersion {
                found,
                min,
                max,
            } => JournalError::UnsupportedSchemaVersion {
                line: line_no,
                found,
                min,
                max,
            },
        })?;
        entries.push(entry);
    }
    entries.sort_by_key(JournalEntry::sort_key);
    Ok(entries)
}

/// Verify consecutive `sequence` from 0 after sort.
pub fn verify_contiguous_sequence(entries: &[JournalEntry]) -> Result<(), JournalError> {
    for (i, e) in entries.iter().enumerate() {
        let expected = i as u64;
        if e.sequence != expected {
            return Err(JournalError::SequenceMismatch {
                line: i + 1,
                expected,
                found: e.sequence,
            });
        }
    }
    Ok(())
}

/// §8.5 — backup path, then truncate after last valid JSON line.
pub fn repair_journal(path: &Path, backup_suffix: &str) -> Result<(usize, usize), JournalError> {
    if !path.exists() {
        return Ok((0, 0));
    }
    let raw = std::fs::read_to_string(path).map_err(|e| JournalError::io(path.to_path_buf(), e))?;
    let backup = format!("{}{}", path.display(), backup_suffix);
    std::fs::copy(path, &backup).map_err(|e| JournalError::io(path.to_path_buf(), e))?;
    let mut kept = 0usize;
    let mut valid_lines: Vec<String> = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let ok = match serde_json::from_str::<JournalEntry>(t) {
            Ok(e) => e.validate_supported_schema().is_ok(),
            Err(_) => false,
        };
        if ok {
            valid_lines.push(line.to_string());
            kept += 1;
        } else {
            break;
        }
    }
    let dropped = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
        .saturating_sub(kept);
    std::fs::write(
        path,
        valid_lines.join("\n") + if valid_lines.is_empty() { "" } else { "\n" },
    )
    .map_err(|e| JournalError::io(path.to_path_buf(), e))?;
    Ok((kept, dropped))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use tempfile::NamedTempFile;
    use uuid::Uuid;

    #[test]
    fn load_rejects_schema_version_below_min() {
        let line = format!(
            r#"{{"schema_version":1,"timestamp":0,"sequence":0,"causal_chain_id":"{}","family":"observation","variant":"motion_detected","room":"x"}}"#,
            Uuid::nil()
        );
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "{line}").unwrap();
        let err = load_and_sort(f.path()).unwrap_err();
        match err {
            JournalError::UnsupportedSchemaVersion {
                line: 1,
                found: 1,
                min: 2,
                max: 5,
            } => {}
            e => panic!("unexpected error: {e:?}"),
        }
    }

    #[test]
    fn load_accepts_schema_version_2_minimal_line() {
        let line = format!(
            r#"{{"schema_version":2,"timestamp":0,"sequence":0,"causal_chain_id":"{}","family":"observation","variant":"motion_detected","room":"hall"}}"#,
            Uuid::nil()
        );
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "{line}").unwrap();
        let entries = load_and_sort(f.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].schema_version, 2);
    }
}
