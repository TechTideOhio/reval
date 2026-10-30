use serde::Deserialize;

use crate::ingest::redact::redact_sensitive;
use crate::ingest::{TraceOutcome, TraceRecord, TraceSource};

#[derive(Debug, Deserialize)]
struct GoTestEvent {
    #[serde(rename = "Action")]
    action: String,
    #[serde(rename = "Package")]
    package: Option<String>,
    #[serde(rename = "Test")]
    test: Option<String>,
    #[serde(rename = "Output")]
    output: Option<String>,
    #[serde(rename = "Elapsed")]
    elapsed: Option<f64>,
}

/// Parses a `go test -json` streaming text output into normalized `TraceRecord`s.
#[must_use]
pub fn parse_go_test_json(stream: &str) -> Vec<TraceRecord> {
    let mut records = Vec::new();
    let mut outputs_by_test: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();

    for line in stream.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(ev) = serde_json::from_str::<GoTestEvent>(line) else {
            continue;
        };
        let key = match (&ev.package, &ev.test) {
            (Some(pkg), Some(t)) => format!("{pkg}::{t}"),
            (Some(pkg), None) => pkg.clone(),
            (None, Some(t)) => t.clone(),
            (None, None) => "global".to_string(),
        };

        if let Some(out) = ev.output {
            outputs_by_test
                .entry(key.clone())
                .or_default()
                .push(out.trim_end().to_string());
        }

        if ev.action == "fail" || ev.action == "pass" {
            let is_pass = ev.action == "pass";
            let diag = outputs_by_test.remove(&key).unwrap_or_default().join(" | ");
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let duration_ms = ev.elapsed.map(|s| (s * 1000.0).round() as u64);

            records.push(TraceRecord {
                id: format!("go:{}", key.replace('/', "-")),
                source: TraceSource::GoTest,
                agent_or_runner: "go-test".to_string(),
                outcome: if is_pass {
                    TraceOutcome::Pass
                } else {
                    TraceOutcome::Fail
                },
                dry_run: false,
                exit_code: Some(i64::from(!is_pass)),
                duration_ms,
                bounded_status: None,
                loop_streak: None,
                command_or_summary: redact_sensitive(&format!("go test {}", key)),
                diagnostic_excerpt: redact_sensitive(&diag),
            });
        }
    }
    records
}
