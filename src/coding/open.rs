use serde::{Deserialize, Serialize};

use crate::ingest::{TraceOutcome, TraceRecord, TraceSource};

/// Fine-grained open-coding tag assigned during free-text failure annotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING-KEBAB-CASE")]
pub enum OpenCode {
    TypecheckBuildOrderCascade,
    LintRatchetHygieneGate,
    BinarySelfUpdateStub,
    ModelCatalogMismatch,
    EnvPathSandbox,
    TestArtifactPollution,
    StreamDisconnectAbort,
    SandboxTlsTimeout,
    VitestAssertionIteration,
    ConcurrencyTestFlake,
    ShellQuotingSplit,
    ShellCmdSyntax,
    GitTrustBoundary,
    ConfigTomlSchema,
    AuthOauthExpired,
    EnvMissingDockerOrDeps,
    ToolBudgetExhaustion,
    GoPanicOrRace,
    GoSubtestAssertion,
    RustCompileFailure,
    UnclassifiedFailure,
}

impl OpenCode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TypecheckBuildOrderCascade => "TYPECHECK-BUILD-ORDER-CASCADE",
            Self::LintRatchetHygieneGate => "LINT-RATCHET-HYGIENE-GATE",
            Self::BinarySelfUpdateStub => "BINARY-SELF-UPDATE-STUB",
            Self::ModelCatalogMismatch => "MODEL-CATALOG-MISMATCH",
            Self::EnvPathSandbox => "ENV-PATH-SANDBOX",
            Self::TestArtifactPollution => "TEST-ARTIFACT-POLLUTION",
            Self::StreamDisconnectAbort => "STREAM-DISCONNECT-ABORT",
            Self::SandboxTlsTimeout => "SANDBOX-TLS-TIMEOUT",
            Self::VitestAssertionIteration => "VITEST-ASSERTION-ITERATION",
            Self::ConcurrencyTestFlake => "CONCURRENCY-TEST-FLAKE",
            Self::ShellQuotingSplit => "SHELL-QUOTING-SPLIT",
            Self::ShellCmdSyntax => "SHELL-CMD-SYNTAX",
            Self::GitTrustBoundary => "GIT-TRUST-BOUNDARY",
            Self::ConfigTomlSchema => "CONFIG-TOML-SCHEMA",
            Self::AuthOauthExpired => "AUTH-OAUTH-EXPIRED",
            Self::EnvMissingDockerOrDeps => "ENV-MISSING-DOCKER-OR-DEPS",
            Self::ToolBudgetExhaustion => "TOOL-BUDGET-EXHAUSTION",
            Self::GoPanicOrRace => "GO-PANIC-OR-RACE",
            Self::GoSubtestAssertion => "GO-SUBTEST-ASSERTION",
            Self::RustCompileFailure => "RUST-COMPILE-FAILURE",
            Self::UnclassifiedFailure => "UNCLASSIFIED-FAILURE",
        }
    }
}

/// Open-coded annotation for a single failed trace record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenCodedFailure {
    pub trace_id: String,
    pub source: TraceSource,
    pub open_code: OpenCode,
    pub free_text_annotation: String,
    pub evidence: String,
}

/// Annotates a failed `TraceRecord` in free text (open coding).
#[must_use]
pub fn annotate_failure_raw(record: &TraceRecord) -> Option<(OpenCode, String)> {
    if record.outcome == TraceOutcome::Pass {
        return None;
    }

    let combined = format!(
        "{} {}",
        record.command_or_summary.to_lowercase(),
        record.diagnostic_excerpt.to_lowercase()
    );

    let result = match record.source {
        TraceSource::HarnessRun => classify_harness_failure(record, &combined),
        TraceSource::BoundedCodex => classify_bounded_failure(record, &combined),
        TraceSource::QuietLog => classify_quiet_log_failure(&combined),
        TraceSource::LoopEvent => classify_loop_event_failure(&combined),
        TraceSource::GoTest => classify_go_test_failure(&combined),
        TraceSource::RustCargo => classify_rust_cargo_failure(&combined),
    };
    Some(result)
}

fn classify_harness_failure(record: &TraceRecord, combined: &str) -> (OpenCode, String) {
    if record.dry_run && combined.contains("opencode not installed") {
        (
            OpenCode::TestArtifactPollution,
            "Dry-run test invocation wrote real missing-binary error directory for uninstalled OpenCode into runs/".to_string(),
        )
    } else if record.dry_run && combined.contains("not installed") {
        (
            OpenCode::EnvPathSandbox,
            "Sandboxed where.exe failed to locate npm shim on PATH during pre-dispatch check"
                .to_string(),
        )
    } else if combined.contains("not compatible with the version of windows")
        || combined.contains("native binary not installed")
        || combined.contains("truncated self-update stub")
    {
        (
            OpenCode::BinarySelfUpdateStub,
            "Claude npm self-update left a 500-byte shell stub at bin/claude.exe instead of copying native binary".to_string(),
        )
    } else if combined.contains("oauth session expired")
        || combined.contains("failed to authenticate")
    {
        (
            OpenCode::AuthOauthExpired,
            "Headless agent dispatch rejected due to expired OAuth token requiring interactive refresh".to_string(),
        )
    } else if combined.contains("unrecognized subcommand") {
        (
            OpenCode::ShellQuotingSplit,
            "Windows cmd.exe split multi-word bootstrap prompt on spaces when spawned with args array".to_string(),
        )
    } else if combined.contains("filename, directory name, or volume label syntax is incorrect") {
        (
            OpenCode::ShellCmdSyntax,
            "Invalid Windows cmd.exe path/redirection quoting syntax during shell spawn"
                .to_string(),
        )
    } else if combined.contains("skip-git-repo-check")
        || combined.contains("not inside a trusted directory")
    {
        (
            OpenCode::GitTrustBoundary,
            "Codex CLI refused non-git TechTide workspace root without --skip-git-repo-check flag"
                .to_string(),
        )
    } else if combined.contains("unknownissuer") || combined.contains("timed out") {
        (
            OpenCode::SandboxTlsTimeout,
            "Subprocess hung reconnecting after sandbox TLS certificate inspection error (UnknownIssuer)".to_string(),
        )
    } else {
        (
            OpenCode::UnclassifiedFailure,
            format!(
                "Unclassified harness run failure: {}",
                record.command_or_summary
            ),
        )
    }
}

fn classify_bounded_failure(record: &TraceRecord, combined: &str) -> (OpenCode, String) {
    if record.bounded_status.as_deref() == Some("tool-limit") {
        (
            OpenCode::ToolBudgetExhaustion,
            "Worker exhausted hard cap of 60 tool calls before emitting turn.completed".to_string(),
        )
    } else if combined.contains("invalid transport") || combined.contains("lean.config.toml") {
        (
            OpenCode::ConfigTomlSchema,
            "Codex strict config parser rejected partial MCP table override missing transport field".to_string(),
        )
    } else if combined.contains("model is not supported")
        || combined.contains("invalid_request_error")
    {
        (
            OpenCode::ModelCatalogMismatch,
            "Legacy npm Codex CLI rejected gpt-6.1-sol model catalog under ChatGPT OAuth account"
                .to_string(),
        )
    } else if combined.contains("reconnecting...")
        || combined.contains("stream disconnected")
        || combined.contains("websocket closed")
        || combined.contains("remote compaction")
    {
        (
            OpenCode::StreamDisconnectAbort,
            "codex-run.mjs killed subprocess tree prematurely on transient WebSocket Reconnecting... event".to_string(),
        )
    } else {
        (
            OpenCode::UnclassifiedFailure,
            format!("Bounded runner failure ({:?})", record.bounded_status),
        )
    }
}

fn classify_quiet_log_failure(combined: &str) -> (OpenCode, String) {
    if combined.contains("spawnsync docker enoent")
        || combined.contains("cannot find package")
        || combined.contains("err_module_not_found")
    {
        (
            OpenCode::EnvMissingDockerOrDeps,
            "Command failed due to missing Docker binary in PATH or un-installed worktree dependencies".to_string(),
        )
    } else if combined.contains("typecheck")
        || combined.contains("tsc")
        || combined.contains("ts2")
        || combined.contains("ts1")
        || combined.contains("types:studio-api")
    {
        (
            OpenCode::TypecheckBuildOrderCascade,
            "Downstream TypeScript typecheck/codegen failed due to unbuilt upstream @vsp/contracts schema".to_string(),
        )
    } else if combined.contains("eslint")
        || combined.contains("verify-ui-drift")
        || combined.contains("verify:contracts")
        || combined.contains("verify:interactions")
        || combined.contains("pnpm lint")
    {
        (
            OpenCode::LintRatchetHygieneGate,
            "Repo quality gate failed on ESLint warning ratchet, UI token drift, or contract parity check".to_string(),
        )
    } else if combined.contains("advisory-lock") || combined.contains("waitforadvisorywaiters") {
        (
            OpenCode::ConcurrencyTestFlake,
            "Postgres integration test timed out waiting on advisory-lock waiter due to unisolated DB state".to_string(),
        )
    } else {
        (
            OpenCode::VitestAssertionIteration,
            "Unit/component test assertion failure during iterative feature development"
                .to_string(),
        )
    }
}

fn classify_loop_event_failure(combined: &str) -> (OpenCode, String) {
    if combined.contains("typecheck")
        || combined.contains("types:studio-api")
        || combined.contains("build")
    {
        (
            OpenCode::TypecheckBuildOrderCascade,
            "Agent tripped 3x loop guard repeating failing typecheck/build without rebuilding upstream contracts".to_string(),
        )
    } else if combined.contains("lint")
        || combined.contains("verify-ui-drift")
        || combined.contains("verify:contracts")
        || combined.contains("verify:interactions")
    {
        (
            OpenCode::LintRatchetHygieneGate,
            "Agent tripped 3x loop guard repeating failing lint/ratchet/drift gate without fixing violation".to_string(),
        )
    } else {
        (
            OpenCode::VitestAssertionIteration,
            "Agent tripped 3x loop guard repeating identical failing test command without changing failure signature".to_string(),
        )
    }
}

fn classify_go_test_failure(combined: &str) -> (OpenCode, String) {
    if combined.contains("panic:")
        || combined.contains("data race")
        || combined.contains("fatal error:")
    {
        (
            OpenCode::GoPanicOrRace,
            "Go test failed with runtime panic, data race detection, or fatal runtime error"
                .to_string(),
        )
    } else {
        (
            OpenCode::GoSubtestAssertion,
            "Go table-driven subtest assertion failure".to_string(),
        )
    }
}

fn classify_rust_cargo_failure(combined: &str) -> (OpenCode, String) {
    (
        OpenCode::RustCompileFailure,
        format!(
            "Rust compiler/clippy error: {}",
            combined.chars().take(120).collect::<String>()
        ),
    )
}
