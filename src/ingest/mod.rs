pub mod golang;
pub mod harness;
pub mod redact;
pub mod rust;

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::RevalError;

pub use golang::parse_go_test_json;
pub use harness::{
    ingest_bounded_runs, ingest_harness_runs, ingest_loop_events, ingest_quiet_logs,
};
pub use redact::redact_sensitive;
pub use rust::parse_cargo_json;

/// Source store from which a production trace record was ingested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceSource {
    HarnessRun,
    BoundedCodex,
    QuietLog,
    LoopEvent,
    GoTest,
    RustCargo,
}

impl TraceSource {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HarnessRun => "harness_run",
            Self::BoundedCodex => "bounded_codex",
            Self::QuietLog => "quiet_log",
            Self::LoopEvent => "loop_event",
            Self::GoTest => "go_test",
            Self::RustCargo => "rust_cargo",
        }
    }
}

/// High-level outcome of a single trace execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TraceOutcome {
    Pass,
    Fail,
}

/// Canonical normalized production trace record across all Software Factory stores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceRecord {
    pub id: String,
    pub source: TraceSource,
    pub agent_or_runner: String,
    pub outcome: TraceOutcome,
    pub dry_run: bool,
    pub exit_code: Option<i64>,
    pub duration_ms: Option<u64>,
    pub bounded_status: Option<String>,
    pub loop_streak: Option<u32>,
    pub command_or_summary: String,
    pub diagnostic_excerpt: String,
}

/// Ingests all production traces from a `TechTide` workspace root and optional bounded-runs directory.
pub fn ingest_workspace_traces(
    workspace_root: &Path,
    bounded_runs_dir: Option<&Path>,
) -> Result<Vec<TraceRecord>, RevalError> {
    let mut records = Vec::new();

    let runs_dir = workspace_root
        .join("Tools")
        .join("agent-harness")
        .join("runs");
    if runs_dir.is_dir() {
        records.extend(ingest_harness_runs(&runs_dir)?);
    }

    let logs_dir = workspace_root
        .join("Tools")
        .join("agent-harness")
        .join("logs");
    if logs_dir.is_dir() {
        records.extend(ingest_quiet_logs(&logs_dir)?);
        let loop_events_path = logs_dir.join("loop-events.jsonl");
        if loop_events_path.is_file() {
            records.extend(ingest_loop_events(&loop_events_path)?);
        }
    }

    if let Some(bounded_dir) = bounded_runs_dir {
        if bounded_dir.is_dir() {
            records.extend(ingest_bounded_runs(bounded_dir)?);
        }
    }

    records.sort_by(|a, b| a.source.cmp(&b.source).then_with(|| a.id.cmp(&b.id)));
    Ok(records)
}

/// Loads pre-exported JSONL trace records from a file.
pub fn load_jsonl_corpus(path: &Path) -> Result<Vec<TraceRecord>, RevalError> {
    let content = fs::read_to_string(path).map_err(|source| RevalError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut out = Vec::new();
    for line in content.lines().filter(|l| !l.trim().is_empty()) {
        let record: TraceRecord =
            serde_json::from_str(line).map_err(|source| RevalError::Json {
                path: path.to_path_buf(),
                source,
            })?;
        out.push(record);
    }
    out.sort_by(|a, b| a.source.cmp(&b.source).then_with(|| a.id.cmp(&b.id)));
    Ok(out)
}
