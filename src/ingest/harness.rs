use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::RevalError;
use crate::ingest::redact::redact_sensitive;
use crate::ingest::{TraceOutcome, TraceRecord, TraceSource};

#[derive(Debug, Deserialize)]
struct RawHarnessResult {
    status: Option<String>,
    summary: Option<String>,
    agent: Option<String>,
    #[serde(rename = "dryRun")]
    dry_run: Option<bool>,
    #[serde(rename = "exitCode")]
    exit_code: Option<i64>,
    #[serde(rename = "durationMs")]
    duration_ms: Option<u64>,
    #[serde(rename = "boundedStatus")]
    bounded_status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawBoundedResult {
    status: Option<String>,
    #[serde(rename = "exitCode")]
    exit_code: Option<i64>,
    #[serde(rename = "elapsedSeconds")]
    elapsed_seconds: Option<u64>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawLoopEvent {
    at: Option<String>,
    command: Option<String>,
    cwd: Option<String>,
    count: Option<u32>,
}

pub fn ingest_harness_runs(runs_dir: &Path) -> Result<Vec<TraceRecord>, RevalError> {
    let mut out = Vec::new();
    let entries = fs::read_dir(runs_dir).map_err(|source| RevalError::Io {
        path: runs_dir.to_path_buf(),
        source,
    })?;
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();

    for dir in dirs {
        let result_path = dir.join("result.json");
        if !result_path.is_file() {
            continue;
        }
        let raw_text = fs::read_to_string(&result_path).map_err(|source| RevalError::Io {
            path: result_path.clone(),
            source,
        })?;
        let Ok(parsed) = serde_json::from_str::<RawHarnessResult>(&raw_text) else {
            continue;
        };
        let dir_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown-run")
            .to_string();
        let stderr_path = dir.join("stderr.log");
        let stdout_path = dir.join("stdout.log");
        let stderr_text = fs::read_to_string(&stderr_path).unwrap_or_default();
        let stdout_text = fs::read_to_string(&stdout_path).unwrap_or_default();
        let combined_diag = if stderr_text.trim().is_empty() {
            stdout_text
        } else {
            format!("{stderr_text}\n{stdout_text}")
        };
        let excerpt: String = combined_diag
            .lines()
            .take(12)
            .collect::<Vec<_>>()
            .join(" | ");
        let status = parsed.status.as_deref().unwrap_or("error");
        let outcome = if status == "success" {
            TraceOutcome::Pass
        } else {
            TraceOutcome::Fail
        };
        out.push(TraceRecord {
            id: dir_name,
            source: TraceSource::HarnessRun,
            agent_or_runner: parsed.agent.unwrap_or_else(|| "unknown".to_string()),
            outcome,
            dry_run: parsed.dry_run.unwrap_or(false),
            exit_code: parsed.exit_code,
            duration_ms: parsed.duration_ms,
            bounded_status: parsed.bounded_status,
            loop_streak: None,
            command_or_summary: redact_sensitive(&parsed.summary.unwrap_or_default()),
            diagnostic_excerpt: redact_sensitive(&excerpt),
        });
    }
    Ok(out)
}

pub fn ingest_bounded_runs(bounded_dir: &Path) -> Result<Vec<TraceRecord>, RevalError> {
    let mut out = Vec::new();
    let entries = fs::read_dir(bounded_dir).map_err(|source| RevalError::Io {
        path: bounded_dir.to_path_buf(),
        source,
    })?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".jsonl.result.json"))
        })
        .collect();
    files.sort();

    for res_path in files {
        let raw_text = fs::read_to_string(&res_path).map_err(|source| RevalError::Io {
            path: res_path.clone(),
            source,
        })?;
        let Ok(parsed) = serde_json::from_str::<RawBoundedResult>(&raw_text) else {
            continue;
        };
        let file_name = res_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("bounded")
            .trim_end_matches(".jsonl.result.json")
            .to_string();
        let jsonl_stem = res_path.with_extension("");
        let jsonl_path = jsonl_stem.with_extension("");
        let stderr_path = PathBuf::from(format!("{}.stderr", jsonl_path.display()));

        let stderr_text = fs::read_to_string(&stderr_path).unwrap_or_default();
        let jsonl_tail = fs::read_to_string(&jsonl_path)
            .map(|s| {
                s.lines()
                    .rev()
                    .take(6)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join(" | ")
            })
            .unwrap_or_default();

        let status = parsed.status.unwrap_or_else(|| "failed".to_string());
        let outcome = if status == "complete" {
            TraceOutcome::Pass
        } else {
            TraceOutcome::Fail
        };
        let diag = if stderr_text.trim().is_empty() {
            jsonl_tail
        } else {
            format!(
                "{} | {}",
                stderr_text.lines().take(6).collect::<Vec<_>>().join(" "),
                jsonl_tail
            )
        };

        out.push(TraceRecord {
            id: file_name,
            source: TraceSource::BoundedCodex,
            agent_or_runner: "codex-bounded".to_string(),
            outcome,
            dry_run: false,
            exit_code: parsed.exit_code,
            duration_ms: parsed.elapsed_seconds.map(|s| s.saturating_mul(1000)),
            bounded_status: Some(status),
            loop_streak: None,
            command_or_summary: redact_sensitive(
                parsed.message.as_deref().unwrap_or("bounded codex run"),
            ),
            diagnostic_excerpt: redact_sensitive(&diag),
        });
    }
    Ok(out)
}

pub fn ingest_quiet_logs(logs_dir: &Path) -> Result<Vec<TraceRecord>, RevalError> {
    let mut out = Vec::new();
    let entries = fs::read_dir(logs_dir).map_err(|source| RevalError::Io {
        path: logs_dir.to_path_buf(),
        source,
    })?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("log"))
        .collect();
    files.sort();

    for path in files {
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let mut lines = content.lines();
        let cmd_line = lines
            .next()
            .unwrap_or_default()
            .trim_start_matches("$ ")
            .trim();
        let _cwd_line = lines.next().unwrap_or_default();
        let exit_line = lines.next().unwrap_or_default();
        let exit_code = exit_line
            .strip_prefix("# exit: ")
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|tok| tok.parse::<i64>().ok());
        let outcome = if exit_code == Some(0) {
            TraceOutcome::Pass
        } else {
            TraceOutcome::Fail
        };
        let body_excerpt: String = lines
            .filter(|l| !l.trim().is_empty())
            .rev()
            .take(15)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" | ");
        let id = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("quiet-log")
            .to_string();

        out.push(TraceRecord {
            id,
            source: TraceSource::QuietLog,
            agent_or_runner: "q.mjs".to_string(),
            outcome,
            dry_run: false,
            exit_code,
            duration_ms: None,
            bounded_status: None,
            loop_streak: None,
            command_or_summary: redact_sensitive(cmd_line),
            diagnostic_excerpt: redact_sensitive(&body_excerpt),
        });
    }
    Ok(out)
}

pub fn ingest_loop_events(loop_events_path: &Path) -> Result<Vec<TraceRecord>, RevalError> {
    let content = fs::read_to_string(loop_events_path).map_err(|source| RevalError::Io {
        path: loop_events_path.to_path_buf(),
        source,
    })?;
    let mut out = Vec::new();
    for (idx, line) in content.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let Ok(ev) = serde_json::from_str::<RawLoopEvent>(line) else {
            continue;
        };
        let id = format!(
            "loop-{:03}-{}",
            idx + 1,
            ev.at.as_deref().unwrap_or("unknown").replace(':', "-")
        );
        let cmd = ev.command.unwrap_or_default();
        let cwd = ev.cwd.unwrap_or_default();
        out.push(TraceRecord {
            id,
            source: TraceSource::LoopEvent,
            agent_or_runner: "q.mjs-loop-guard".to_string(),
            outcome: TraceOutcome::Fail,
            dry_run: false,
            exit_code: Some(3),
            duration_ms: None,
            bounded_status: None,
            loop_streak: ev.count,
            command_or_summary: redact_sensitive(&cmd),
            diagnostic_excerpt: redact_sensitive(&format!(
                "loop_streak={} cwd={} cmd={}",
                ev.count.unwrap_or(3),
                cwd,
                cmd
            )),
        });
    }
    Ok(out)
}
