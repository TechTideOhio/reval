use std::fs;

use reval::{export_otel_and_openinference, TraceOutcome, TraceRecord, TraceSource};

#[test]
fn test_export_otel_and_openinference_formats() {
    let temp_dir = std::env::temp_dir().join(format!("reval-otel-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("failed to create temp dir");

    let sample_traces = vec![
        TraceRecord {
            id: "trace-run-001".to_string(),
            source: TraceSource::HarnessRun,
            agent_or_runner: "claude-code".to_string(),
            outcome: TraceOutcome::Pass,
            dry_run: false,
            exit_code: Some(0),
            duration_ms: Some(150),
            bounded_status: None,
            loop_streak: None,
            command_or_summary: "pnpm test".to_string(),
            diagnostic_excerpt: "All 15 tests passed".to_string(),
        },
        TraceRecord {
            id: "trace-run-002".to_string(),
            source: TraceSource::QuietLog,
            agent_or_runner: "codex".to_string(),
            outcome: TraceOutcome::Fail,
            dry_run: false,
            exit_code: Some(1),
            duration_ms: Some(420),
            bounded_status: None,
            loop_streak: None,
            command_or_summary: "pnpm build".to_string(),
            diagnostic_excerpt: "error TS2307: Cannot find module '@vsp/contracts'".to_string(),
        },
    ];

    let (otel_path, oi_path) = export_otel_and_openinference(&sample_traces, &temp_dir)
        .expect("exporting OTel and OpenInference should succeed");

    assert!(otel_path.exists(), "traces.otel.json must exist");
    assert!(oi_path.exists(), "traces.openinference.jsonl must exist");

    // Verify OTel JSON structure
    let otel_content = fs::read_to_string(&otel_path).expect("read otel");
    let otel_json: serde_json::Value =
        serde_json::from_str(&otel_content).expect("parse otel json");

    let resource_spans = otel_json.get("resourceSpans").expect("resourceSpans");
    assert_eq!(resource_spans.as_array().expect("array").len(), 1);

    let spans = &resource_spans[0]["scopeSpans"][0]["spans"];
    let span_arr = spans.as_array().expect("spans array");
    assert_eq!(span_arr.len(), 2);

    // Verify first span (pass)
    assert_eq!(span_arr[0]["status"]["code"], 1); // OK
    assert_eq!(span_arr[0]["name"], "pnpm test");

    // Verify second span (fail)
    assert_eq!(span_arr[1]["status"]["code"], 2); // ERROR
    let attributes = span_arr[1]["attributes"].as_array().expect("attributes");
    let has_open_code = attributes.iter().any(|attr| {
        attr["key"] == "reval.open_code"
            && attr["value"]["stringValue"] == "TYPECHECK-BUILD-ORDER-CASCADE"
    });
    assert!(
        has_open_code,
        "failed span must include classified open code attribute"
    );

    // Verify OpenInference JSONL
    let oi_content = fs::read_to_string(&oi_path).expect("read oi");
    let lines: Vec<&str> = oi_content.lines().collect();
    assert_eq!(lines.len(), 2);

    let oi_rec: serde_json::Value = serde_json::from_str(lines[1]).expect("parse oi line 2");
    assert_eq!(oi_rec["span_kind"], "AGENT");
    assert_eq!(oi_rec["status_code"], "ERROR");
    assert_eq!(oi_rec["open_code"], "TYPECHECK-BUILD-ORDER-CASCADE");
    assert_eq!(oi_rec["axial_category"], "loop_cascade");

    let _ = fs::remove_dir_all(&temp_dir);
}
