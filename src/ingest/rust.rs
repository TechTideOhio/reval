use serde::Deserialize;

use crate::ingest::redact::redact_sensitive;
use crate::ingest::{TraceOutcome, TraceRecord, TraceSource};

#[derive(Debug, Deserialize)]
struct CargoMessage {
    reason: String,
    message: Option<CompilerMessagePayload>,
    package_id: Option<String>,
    success: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CompilerMessagePayload {
    rendered: Option<String>,
    message: String,
    level: String,
}

/// Parses a `cargo test --message-format=json` or `cargo clippy --message-format=json` stream.
#[must_use]
pub fn parse_cargo_json(stream: &str) -> Vec<TraceRecord> {
    let mut records = Vec::new();

    for (idx, line) in stream.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let Ok(msg) = serde_json::from_str::<CargoMessage>(line) else {
            continue;
        };

        if msg.reason == "compiler-message" {
            if let Some(payload) = msg.message {
                if payload.level == "error" {
                    let pkg = msg
                        .package_id
                        .as_deref()
                        .unwrap_or("crate")
                        .split_whitespace()
                        .next()
                        .unwrap_or("crate");
                    let diag = payload
                        .rendered
                        .as_deref()
                        .unwrap_or(&payload.message)
                        .lines()
                        .take(6)
                        .collect::<Vec<_>>()
                        .join(" | ");

                    records.push(TraceRecord {
                        id: format!("cargo:{pkg}:{idx}"),
                        source: TraceSource::RustCargo,
                        agent_or_runner: "cargo".to_string(),
                        outcome: TraceOutcome::Fail,
                        dry_run: false,
                        exit_code: Some(101),
                        duration_ms: None,
                        bounded_status: None,
                        loop_streak: None,
                        command_or_summary: redact_sensitive(&format!(
                            "cargo compile error in {pkg}"
                        )),
                        diagnostic_excerpt: redact_sensitive(&diag),
                    });
                }
            }
        } else if msg.reason == "build-finished" {
            let is_ok = msg.success.unwrap_or(false);
            records.push(TraceRecord {
                id: format!("cargo:build-finished:{idx}"),
                source: TraceSource::RustCargo,
                agent_or_runner: "cargo".to_string(),
                outcome: if is_ok {
                    TraceOutcome::Pass
                } else {
                    TraceOutcome::Fail
                },
                dry_run: false,
                exit_code: Some(if is_ok { 0 } else { 101 }),
                duration_ms: None,
                bounded_status: None,
                loop_streak: None,
                command_or_summary: "cargo build-finished".to_string(),
                diagnostic_excerpt: format!("success={is_ok}"),
            });
        }
    }

    records
}
