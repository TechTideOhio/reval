#![forbid(unsafe_code)]

pub mod coding;
pub mod envelope;
pub mod error;
pub mod eval;
pub mod gate;
pub mod hygiene;
pub mod ingest;
pub mod mcp;
pub mod tree;
pub mod watch;

pub use coding::{
    annotate_failure, axial_category_for, cluster_axial_categories, compute_first_three_metrics,
    query_lessons, AxialCategory, AxialClusterSummary, CausalEdge, CausalFailureGraph, CausalNode,
    FirstThreeMetrics, LessonLearned, OpenCode, OpenCodedFailure,
};
pub use envelope::{EnvelopeStatus, HarnessEnvelope};
pub use error::RevalError;
pub use eval::{
    compute_suite_digest, load_case, score_suite, CaseResult, CaseSpec, Check, CheckResult,
    CheckSpec, RevalLockfile, Severity as EvalSeverity, StratumConfig, SuiteSummary,
};
pub use gate::{
    compute_corpus_sha256, compute_reliability, evaluate_gate, evaluate_traces, BaselineRatchet,
    GateCheck, GateVerdict, ReliabilityMetrics, RevalReport,
};
pub use hygiene::{
    audit_repository, HygieneFinding, HygieneFindingKind, RepoAuditReport,
    Severity as HygieneSeverity,
};
pub use ingest::{
    ingest_bounded_runs, ingest_harness_runs, ingest_loop_events, ingest_quiet_logs,
    ingest_workspace_traces, load_jsonl_corpus, parse_cargo_json, parse_go_test_json,
    redact_sensitive, TraceOutcome, TraceRecord, TraceSource,
};
pub use mcp::run_mcp_server;
pub use tree::{TraceTree, TraceTreeNode};
pub use watch::{run_watcher, WatchOptions};

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn sample_trace(
        id: &str,
        source: TraceSource,
        outcome: TraceOutcome,
        summary: &str,
        diag: &str,
    ) -> TraceRecord {
        TraceRecord {
            id: id.to_string(),
            source,
            agent_or_runner: "test".to_string(),
            outcome,
            dry_run: false,
            exit_code: Some(i64::from(outcome != TraceOutcome::Pass)),
            duration_ms: Some(100),
            bounded_status: None,
            loop_streak: None,
            command_or_summary: summary.to_string(),
            diagnostic_excerpt: diag.to_string(),
        }
    }

    #[rstest]
    #[case(
        TraceSource::QuietLog,
        "pnpm --filter @vsp/shopfront typecheck",
        "TS2339: Property 'maxValue' does not exist on type 'ZodType<number>'",
        OpenCode::TypecheckBuildOrderCascade,
        AxialCategory::LoopCascade
    )]
    #[case(
        TraceSource::QuietLog,
        "pnpm lint",
        "scripts/eslint-ratchet.mjs regression warning count",
        OpenCode::LintRatchetHygieneGate,
        AxialCategory::LoopCascade
    )]
    #[case(
        TraceSource::HarnessRun,
        "Claude Code failed (exit 1)",
        "This version of claude.exe is not compatible with the version of Windows you're running.",
        OpenCode::BinarySelfUpdateStub,
        AxialCategory::RuntimeDrift
    )]
    #[case(
        TraceSource::BoundedCodex,
        "bounded codex run",
        "{\"type\":\"error\",\"message\":\"Reconnecting... 2/5 (stream disconnected before completion)\"}",
        OpenCode::StreamDisconnectAbort,
        AxialCategory::PrematureKill
    )]
    #[case(
        TraceSource::GoTest,
        "go test ./pkg/store",
        "panic: runtime error: invalid memory address or nil pointer dereference",
        OpenCode::GoPanicOrRace,
        AxialCategory::TestIteration
    )]
    #[case(
        TraceSource::RustCargo,
        "cargo compile error in crate",
        "error[E0308]: mismatched types | expected struct `String`, found `&str`",
        OpenCode::RustCompileFailure,
        AxialCategory::LoopCascade
    )]
    fn open_and_axial_coding_classifies_real_software_factory_failures(
        #[case] source: TraceSource,
        #[case] summary: &str,
        #[case] diag: &str,
        #[case] expected_open: OpenCode,
        #[case] expected_axial: AxialCategory,
    ) {
        let rec = sample_trace("t-1", source, TraceOutcome::Fail, summary, diag);
        let coded = annotate_failure(&rec);
        assert!(coded.is_some());
        if let Some((open, axial)) = coded {
            assert_eq!(open.open_code, expected_open);
            assert_eq!(axial, expected_axial);
            assert_ne!(open.free_text_annotation, "");
        }
    }

    #[test]
    fn passing_traces_produce_no_open_coded_failure() {
        let rec = sample_trace(
            "ok-1",
            TraceSource::QuietLog,
            TraceOutcome::Pass,
            "pnpm test",
            "OK",
        );
        assert!(annotate_failure(&rec).is_none());
    }

    #[test]
    fn redacts_windows_paths_and_tokens() {
        let raw = "C:\\Users\\Admin\\TechTide\\Tools and token gho_1234567890abcdef in log";
        let scrubbed = redact_sensitive(raw);
        assert!(!scrubbed.contains("Admin"));
        assert!(!scrubbed.contains("gho_1234567890abcdef"));
        assert!(scrubbed.contains("<WORKSPACE>"));
        assert!(scrubbed.contains("<REDACTED_TOKEN>"));
    }
}
