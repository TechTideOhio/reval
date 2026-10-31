use std::fs;

use reval::{
    query_lessons_from_store, PersistentGraphStore, TraceOutcome, TraceRecord, TraceSource,
};

#[test]
fn test_persistent_bayesian_causal_graph() {
    let temp_file =
        std::env::temp_dir().join(format!("reval-graph-test-{}.json", std::process::id()));
    let _ = fs::remove_file(&temp_file);

    let mut store = PersistentGraphStore::load_or_create(&temp_file)
        .expect("should create default graph store");
    assert_eq!(store.edges.len(), 0);

    let batch1 = vec![
        TraceRecord {
            id: "run-001".to_string(),
            source: TraceSource::QuietLog,
            agent_or_runner: "claude".to_string(),
            outcome: TraceOutcome::Fail,
            dry_run: false,
            exit_code: Some(1),
            duration_ms: Some(200),
            bounded_status: None,
            loop_streak: None,
            command_or_summary: "vitest run".to_string(),
            diagnostic_excerpt: "AssertionError: expected true to be false".to_string(),
        },
        TraceRecord {
            id: "run-002".to_string(),
            source: TraceSource::QuietLog,
            agent_or_runner: "claude".to_string(),
            outcome: TraceOutcome::Fail,
            dry_run: false,
            exit_code: Some(1),
            duration_ms: Some(250),
            bounded_status: None,
            loop_streak: None,
            command_or_summary: "vitest run auth.test.ts".to_string(),
            diagnostic_excerpt: "AssertionError: lock timeout on postgres test db".to_string(),
        },
    ];

    store.ingest_traces(&batch1);
    assert!(!store.edges.is_empty());
    assert_eq!(store.total_failures_observed, 2);

    let first_edge = &store.edges[0];
    assert!(first_edge.bayes_probability_pct > 0.0);
    assert!(first_edge.confidence_score >= 0.0 && first_edge.confidence_score <= 1.0);
    assert!(first_edge.credible_interval_low_pct <= first_edge.bayes_probability_pct);
    assert!(first_edge.credible_interval_high_pct >= first_edge.bayes_probability_pct);

    // Save to disk
    store.save(&temp_file).expect("saving graph store");
    assert!(temp_file.exists());

    // Reload from disk and perform incremental ingestion
    let mut reloaded =
        PersistentGraphStore::load_or_create(&temp_file).expect("reload graph store");
    assert_eq!(reloaded.total_failures_observed, 2);

    let batch2 = vec![TraceRecord {
        id: "run-003".to_string(),
        source: TraceSource::HarnessRun,
        agent_or_runner: "claude".to_string(),
        outcome: TraceOutcome::Fail,
        dry_run: false,
        exit_code: Some(1),
        duration_ms: Some(300),
        bounded_status: None,
        loop_streak: None,
        command_or_summary: "pnpm build".to_string(),
        diagnostic_excerpt: "error TS2307: Cannot find module '@vsp/contracts'".to_string(),
    }];

    reloaded.ingest_traces(&batch2);
    assert_eq!(reloaded.total_failures_observed, 3);

    // Verify Mermaid output contains Bayesian statistics
    let mermaid = reloaded.to_mermaid();
    assert!(mermaid.contains("flowchart LR"));
    assert!(mermaid.contains("P="));

    // Verify lessons query with Bayesian confidence
    let lessons = query_lessons_from_store(&reloaded, Some("vitest"));
    assert!(!lessons.is_empty());
    assert!(lessons[0].bayes_probability_pct.is_some());
    assert!(lessons[0].confidence_score.is_some());

    let _ = fs::remove_file(&temp_file);
}
