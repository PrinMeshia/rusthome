//! Journal tail loading for the dashboard and `/api/journal`.

use std::path::Path;

use rusthome_core::EventKind;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(crate) struct JournalQuery {
    #[serde(default = "default_limit")]
    pub(crate) limit: usize,
}

pub(crate) fn default_limit() -> usize {
    40
}

#[derive(Serialize)]
pub(crate) struct JournalLineDto {
    pub(crate) sequence: u64,
    pub(crate) timestamp: i64,
    pub(crate) kind: EventKind,
}

pub(crate) fn journal_tail_dtos(path: &Path, limit: usize) -> Result<Vec<JournalLineDto>, String> {
    let entries = rusthome_infra::load_and_sort(path).map_err(|e| e.to_string())?;
    let lim = limit.clamp(1, 500);
    let tail = if entries.len() > lim {
        let start = entries.len() - lim;
        let mut v = entries;
        v.split_off(start)
    } else {
        entries
    };
    Ok(tail
        .into_iter()
        .map(|e| JournalLineDto {
            sequence: e.sequence,
            timestamp: e.timestamp,
            kind: e.event.kind(),
        })
        .collect())
}
