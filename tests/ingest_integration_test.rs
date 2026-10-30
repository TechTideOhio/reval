use reval::{
    evaluate_gate, evaluate_traces, parse_cargo_json, parse_go_test_json, AxialCategory,
    BaselineRatchet, TraceOutcome, TraceRecord, TraceSource,
};

#[test]
fn parses_go_test_streaming_json() {
    let go_stream = r#"
{"Time":"2026-10-30T10:00:00Z","Action":"run","Package":"github.com/techtide/store","Test":"TestSave"}
{"Time":"2026-10-30T10:00:01Z","Action":"output","Package":"github.com/techtide/store","Test":"TestSave","Output":"=== RUN   TestSave\n"}
{"Time":"2026-10-30T10:00:02Z","Action":"output","Package":"github.com/techtide/store","Test":"TestSave","Output":"panic: nil pointer dereference\n"}
{"Time":"2026-10-30T10:00:03Z","Action":"fail","Package":"github.com/techtide/store","Test":"TestSave","Elapsed":0.05}
{"Time":"2026-10-30T10:00:04Z","Action":"run","Package":"github.com/techtide/store","Test":"TestLoad"}
{"Time":"2026-10-30T10:00:05Z","Action":"pass","Package":"github.com/techtide/store","Test":"TestLoad","Elapsed":0.01}
"#;

    let records = parse_go_test_json(go_stream);
    assert_eq!(records.len(), 2);
    let fail_rec = records
        .iter()
        .find(|r| r.outcome == TraceOutcome::Fail)
        .expect("fail test");
    assert!(fail_rec.command_or_summary.contains("TestSave"));
    assert!(fail_rec.diagnostic_excerpt.contains("panic"));
    assert_eq!(fail_rec.source, TraceSource::GoTest);

    let pass_rec = records
        .iter()
        .find(|r| r.outcome == TraceOutcome::Pass)
        .expect("pass test");
    assert!(pass_rec.command_or_summary.contains("TestLoad"));
    assert_eq!(pass_rec.exit_code, Some(0));
}

#[test]
fn parses_cargo_compiler_messages() {
    let cargo_stream = r#"
{"reason":"compiler-message","package_id":"reval 0.1.0 (path+file:///crates/reval)","message":{"message":"mismatched types","level":"error","rendered":"error[E0308]: mismatched types\n  --> src/main.rs:10:5\n"}}
{"reason":"build-finished","success":false}
"#;

    let records = parse_cargo_json(cargo_stream);
    assert_eq!(records.len(), 2);
    assert!(records[0].diagnostic_excerpt.contains("mismatched types"));
    assert_eq!(records[0].outcome, TraceOutcome::Fail);
    assert_eq!(records[1].command_or_summary, "cargo build-finished");
    assert_eq!(records[1].outcome, TraceOutcome::Fail);
}

#[test]
fn ranks_top_3_axial_categories_and_enforces_counterfactual_ratchet() {
    let mut corpus = Vec::new();
    for i in 0..80 {
        corpus.push(TraceRecord {
            id: format!("q-pass-{i:03}"),
            source: TraceSource::QuietLog,
            agent_or_runner: "q.mjs".to_string(),
            outcome: TraceOutcome::Pass,
            dry_run: false,
            exit_code: Some(0),
            duration_ms: Some(1200),
            bounded_status: None,
            loop_streak: None,
            command_or_summary: "pnpm test".to_string(),
            diagnostic_excerpt: "PASS".to_string(),
        });
    }
    for i in 0..20 {
        corpus.push(TraceRecord {
            id: format!("loop-{i:03}"),
            source: TraceSource::LoopEvent,
            agent_or_runner: "q.mjs-loop-guard".to_string(),
            outcome: TraceOutcome::Fail,
            dry_run: false,
            exit_code: Some(3),
            duration_ms: None,
            bounded_status: None,
            loop_streak: Some(if i < 3 { 4 } else { 3 }),
            command_or_summary: "pnpm --filter @vsp/shopfront typecheck".to_string(),
            diagnostic_excerpt: "TS2339 typecheck build order cascade".to_string(),
        });
    }
    for i in 0..10 {
        corpus.push(TraceRecord {
            id: format!("run-drift-{i:03}"),
            source: TraceSource::HarnessRun,
            agent_or_runner: "claude".to_string(),
            outcome: TraceOutcome::Fail,
            dry_run: false,
            exit_code: Some(1),
            duration_ms: Some(40),
            bounded_status: None,
            loop_streak: None,
            command_or_summary: "Claude Code failed".to_string(),
            diagnostic_excerpt:
                "This version of claude.exe is not compatible with the version of Windows"
                    .to_string(),
        });
    }
    for i in 0..5 {
        corpus.push(TraceRecord {
            id: format!("bounded-kill-{i:03}"),
            source: TraceSource::BoundedCodex,
            agent_or_runner: "codex-bounded".to_string(),
            outcome: TraceOutcome::Fail,
            dry_run: false,
            exit_code: Some(1),
            duration_ms: Some(15000),
            bounded_status: Some("agent-error".to_string()),
            loop_streak: None,
            command_or_summary: "bounded".to_string(),
            diagnostic_excerpt:
                "{\"type\":\"error\",\"message\":\"Reconnecting... 2/5 (stream disconnected)\"}"
                    .to_string(),
        });
    }
    for i in 0..25 {
        corpus.push(TraceRecord {
            id: format!("bounded-pass-{i:03}"),
            source: TraceSource::BoundedCodex,
            agent_or_runner: "codex-bounded".to_string(),
            outcome: TraceOutcome::Pass,
            dry_run: false,
            exit_code: Some(0),
            duration_ms: Some(12000),
            bounded_status: Some("complete".to_string()),
            loop_streak: None,
            command_or_summary: "PASS".to_string(),
            diagnostic_excerpt: String::new(),
        });
    }

    let report = evaluate_traces(&corpus, 100).expect("140 traces >= 100");
    assert_eq!(report.reliability.total_traces, 140);
    assert_eq!(report.reliability.failed_traces, 35);
    assert_eq!(report.top_3_axial_categories.len(), 3);
    assert_eq!(
        report.top_3_axial_categories[0].category,
        AxialCategory::LoopCascade
    );
    assert_eq!(
        report.top_3_axial_categories[1].category,
        AxialCategory::RuntimeDrift
    );
    assert_eq!(
        report.top_3_axial_categories[2].category,
        AxialCategory::PrematureKill
    );

    let passing_ratchet = BaselineRatchet {
        schema_version: 1,
        max_loop_cascade_rate_pct: 25.0,
        max_loop_guard_breach_count: 5,
        max_runtime_drift_rate_pct: 30.0,
        max_premature_kill_rate_pct: 15.0,
        min_pass_at_1: 0.70,
        min_pass_at_3: 0.95,
    };
    let verdict_ok = evaluate_gate(report.clone(), &passing_ratchet);
    assert!(verdict_ok.passed, "gate should pass");

    // Counterfactual check: strict 0.0% threshold fails
    let strict_ratchet = BaselineRatchet {
        max_premature_kill_rate_pct: 0.0,
        ..passing_ratchet
    };
    let verdict_fail = evaluate_gate(report, &strict_ratchet);
    assert!(!verdict_fail.passed, "gate must fail closed on regression");
}
