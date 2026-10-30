use serde::{Deserialize, Serialize};

use crate::coding::open::{annotate_failure_raw, OpenCode, OpenCodedFailure};
use crate::ingest::{TraceOutcome, TraceRecord};

/// Axial category formed by clustering open codes that share a structural failure mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AxialCategory {
    LoopCascade,
    RuntimeDrift,
    PrematureKill,
    TestIteration,
    ShellTrustConfig,
    EnvAuthBudget,
}

impl AxialCategory {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LoopCascade => "loop_cascade",
            Self::RuntimeDrift => "runtime_drift",
            Self::PrematureKill => "premature_kill",
            Self::TestIteration => "test_iteration",
            Self::ShellTrustConfig => "shell_trust_config",
            Self::EnvAuthBudget => "env_auth_budget",
        }
    }

    #[must_use]
    pub fn metric_name(self) -> &'static str {
        match self {
            Self::LoopCascade => "loop_cascade_rate",
            Self::RuntimeDrift => "runtime_drift_rate",
            Self::PrematureKill => "premature_kill_rate",
            Self::TestIteration => "test_iteration_rate",
            Self::ShellTrustConfig => "shell_trust_config_rate",
            Self::EnvAuthBudget => "env_auth_budget_rate",
        }
    }

    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::LoopCascade => "Monorepo Build-Order & Gate Loop Cascades",
            Self::RuntimeDrift => "Agent Binary, Self-Update Stub & Model Catalog Drift",
            Self::PrematureKill => "Subprocess Stream-Disconnect & TLS Kills",
            Self::TestIteration => "Unit/Integration Assertion & Concurrency Test Iteration",
            Self::ShellTrustConfig => "Windows Shell Quoting, Git Trust & TOML Config",
            Self::EnvAuthBudget => "OAuth Expiry, Missing Docker/Deps & Tool Budget Cap",
        }
    }

    #[must_use]
    pub fn remediation_hint(self) -> &'static str {
        match self {
            Self::LoopCascade => {
                "Rebuild upstream @vsp/contracts + regenerate types before downstream typecheck; halt immediately on q.mjs exit 3."
            }
            Self::RuntimeDrift => {
                "Resolve native claude.exe (>4KB) and desktop codex.exe directly; isolate test runs via HARNESS_RUNS_DIR."
            }
            Self::PrematureKill => {
                "Ignore transient `Reconnecting... i/n` events in codex-run.mjs; only terminate on `turn.failed` or non-reconnect errors."
            }
            Self::TestIteration => {
                "Run narrowest single test file while iterating; isolate Postgres advisory-lock fixtures per test."
            }
            Self::ShellTrustConfig => {
                "Pass single pre-quoted command line on Windows cmd.exe and include --skip-git-repo-check for non-git roots."
            }
            Self::EnvAuthBudget => {
                "Verify OAuth session and Docker daemon availability pre-dispatch; split large multi-file patches under 60-tool cap."
            }
        }
    }
}

/// Maps any `OpenCode` exhaustively (no wildcard `_` match arms) to its parent `AxialCategory`.
#[must_use]
pub fn axial_category_for(code: OpenCode) -> AxialCategory {
    match code {
        OpenCode::TypecheckBuildOrderCascade
        | OpenCode::LintRatchetHygieneGate
        | OpenCode::RustCompileFailure => AxialCategory::LoopCascade,

        OpenCode::BinarySelfUpdateStub
        | OpenCode::ModelCatalogMismatch
        | OpenCode::EnvPathSandbox
        | OpenCode::TestArtifactPollution => AxialCategory::RuntimeDrift,

        OpenCode::StreamDisconnectAbort | OpenCode::SandboxTlsTimeout => {
            AxialCategory::PrematureKill
        }

        OpenCode::VitestAssertionIteration
        | OpenCode::ConcurrencyTestFlake
        | OpenCode::GoPanicOrRace
        | OpenCode::GoSubtestAssertion
        | OpenCode::UnclassifiedFailure => AxialCategory::TestIteration,

        OpenCode::ShellQuotingSplit
        | OpenCode::ShellCmdSyntax
        | OpenCode::GitTrustBoundary
        | OpenCode::ConfigTomlSchema => AxialCategory::ShellTrustConfig,

        OpenCode::AuthOauthExpired
        | OpenCode::EnvMissingDockerOrDeps
        | OpenCode::ToolBudgetExhaustion => AxialCategory::EnvAuthBudget,
    }
}

/// Clustered axial category summary ranked by empirical frequency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AxialClusterSummary {
    pub rank: usize,
    pub category: AxialCategory,
    pub metric_name: String,
    pub title: String,
    pub frequency: usize,
    pub share_of_failures_pct: f64,
    pub open_codes: Vec<String>,
    pub sample_trace_ids: Vec<String>,
    pub remediation_hint: String,
}

/// Annotates a failed trace and determines its axial category.
#[must_use]
pub fn annotate_failure(record: &TraceRecord) -> Option<(OpenCodedFailure, AxialCategory)> {
    if record.outcome == TraceOutcome::Pass {
        return None;
    }
    let (code, note) = annotate_failure_raw(record)?;
    let axial = axial_category_for(code);
    let open = OpenCodedFailure {
        trace_id: record.id.clone(),
        source: record.source,
        open_code: code,
        free_text_annotation: note,
        evidence: if record.diagnostic_excerpt.is_empty() {
            record.command_or_summary.clone()
        } else {
            record.diagnostic_excerpt.clone()
        },
    };
    Some((open, axial))
}

/// Clusters open-coded failure annotations into ranked Axial Categories.
#[must_use]
pub fn cluster_axial_categories(
    failures: &[(OpenCodedFailure, AxialCategory)],
) -> Vec<AxialClusterSummary> {
    let all_categories = [
        AxialCategory::LoopCascade,
        AxialCategory::RuntimeDrift,
        AxialCategory::PrematureKill,
        AxialCategory::TestIteration,
        AxialCategory::ShellTrustConfig,
        AxialCategory::EnvAuthBudget,
    ];

    let total_failures = failures.len();
    let mut clusters: Vec<AxialClusterSummary> = all_categories
        .into_iter()
        .filter_map(|cat| {
            let matching: Vec<&(OpenCodedFailure, AxialCategory)> =
                failures.iter().filter(|(_, c)| *c == cat).collect();
            if matching.is_empty() {
                return None;
            }
            let freq = matching.len();
            #[allow(clippy::cast_precision_loss)]
            let share = if total_failures == 0 {
                0.0
            } else {
                ((freq as f64 / total_failures as f64) * 1000.0).round() / 10.0
            };

            let mut open_codes: Vec<String> = matching
                .iter()
                .map(|(f, _)| f.open_code.as_str().to_string())
                .collect();
            open_codes.sort();
            open_codes.dedup();

            let sample_trace_ids: Vec<String> = matching
                .iter()
                .take(5)
                .map(|(f, _)| f.trace_id.clone())
                .collect();

            Some(AxialClusterSummary {
                rank: 0,
                category: cat,
                metric_name: cat.metric_name().to_string(),
                title: cat.title().to_string(),
                frequency: freq,
                share_of_failures_pct: share,
                open_codes,
                sample_trace_ids,
                remediation_hint: cat.remediation_hint().to_string(),
            })
        })
        .collect();

    clusters.sort_by(|a, b| {
        b.frequency
            .cmp(&a.frequency)
            .then_with(|| a.category.cmp(&b.category))
    });

    for (idx, cluster) in clusters.iter_mut().enumerate() {
        cluster.rank = idx + 1;
    }
    clusters
}
