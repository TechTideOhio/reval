use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::gate::ratchet::{BaselineRatchet, RevalReport};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateCheck {
    pub metric: String,
    pub observed: f64,
    pub threshold: f64,
    pub comparison: String,
    pub passed: bool,
}

/// Verdict returned by comparing a `RevalReport` against a `BaselineRatchet`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateVerdict {
    pub passed: bool,
    pub proof_sha256: String,
    pub checks: Vec<GateCheck>,
    pub report: RevalReport,
}

/// Evaluates a `RevalReport` against a committed `BaselineRatchet` and produces a SHA-256 signed proof bundle.
#[must_use]
pub fn evaluate_gate(report: RevalReport, baseline: &BaselineRatchet) -> GateVerdict {
    let m = &report.first_three_metrics;
    let r = &report.reliability;

    #[allow(clippy::cast_precision_loss)]
    let checks = vec![
        GateCheck {
            metric: "loop_cascade_rate_pct".to_string(),
            observed: m.loop_cascade_rate_pct,
            threshold: baseline.max_loop_cascade_rate_pct,
            comparison: "<=".to_string(),
            passed: m.loop_cascade_rate_pct <= baseline.max_loop_cascade_rate_pct,
        },
        GateCheck {
            metric: "loop_guard_breach_count".to_string(),
            observed: m.loop_guard_breach_count as f64,
            threshold: baseline.max_loop_guard_breach_count as f64,
            comparison: "<=".to_string(),
            passed: m.loop_guard_breach_count <= baseline.max_loop_guard_breach_count,
        },
        GateCheck {
            metric: "runtime_drift_rate_pct".to_string(),
            observed: m.runtime_drift_rate_pct,
            threshold: baseline.max_runtime_drift_rate_pct,
            comparison: "<=".to_string(),
            passed: m.runtime_drift_rate_pct <= baseline.max_runtime_drift_rate_pct,
        },
        GateCheck {
            metric: "premature_kill_rate_pct".to_string(),
            observed: m.premature_kill_rate_pct,
            threshold: baseline.max_premature_kill_rate_pct,
            comparison: "<=".to_string(),
            passed: m.premature_kill_rate_pct <= baseline.max_premature_kill_rate_pct,
        },
        GateCheck {
            metric: "pass_at_1".to_string(),
            observed: r.pass_at_1,
            threshold: baseline.min_pass_at_1,
            comparison: ">=".to_string(),
            passed: r.pass_at_1 >= baseline.min_pass_at_1,
        },
        GateCheck {
            metric: "pass_at_3".to_string(),
            observed: r.pass_at_3,
            threshold: baseline.min_pass_at_3,
            comparison: ">=".to_string(),
            passed: r.pass_at_3 >= baseline.min_pass_at_3,
        },
    ];

    let passed = checks.iter().all(|c| c.passed);
    let mut hasher = Sha256::new();
    hasher.update(report.corpus_sha256.as_bytes());
    for c in &checks {
        hasher.update(c.metric.as_bytes());
        hasher.update(format!(":{}:{}", c.observed, c.passed).as_bytes());
    }
    let proof_sha256 = format!("{:x}", hasher.finalize());

    GateVerdict {
        passed,
        proof_sha256,
        checks,
        report,
    }
}
