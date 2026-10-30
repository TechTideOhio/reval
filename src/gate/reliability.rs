use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ingest::{TraceOutcome, TraceRecord};

/// Empirical reliability metrics (`pass@k` and `pass^k`) across trace executions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReliabilityMetrics {
    pub total_traces: usize,
    pub passed_traces: usize,
    pub failed_traces: usize,
    pub pass_at_1: f64,
    pub pass_at_3: f64,
    pub pass_hat_3: f64,
}

fn round4(val: f64) -> f64 {
    (val * 10000.0).round() / 10000.0
}

/// Computes exact `pass@k` ($1 - (1 - p)^k$) and `pass^k` ($p^k$) reliability metrics.
#[must_use]
pub fn compute_reliability(total: usize, passed: usize, k: u32) -> (f64, f64, f64) {
    if total == 0 {
        return (0.0, 0.0, 0.0);
    }
    let clamped_passed = passed.min(total);
    #[allow(clippy::cast_precision_loss)]
    let p = clamped_passed as f64 / total as f64;
    let k_i32 = i32::try_from(k).unwrap_or(3);
    let pass_at_k = 1.0 - (1.0 - p).powi(k_i32);
    let pass_hat_k = p.powi(k_i32);
    (
        round4(p),
        round4(pass_at_k.clamp(0.0, 1.0)),
        round4(pass_hat_k.clamp(0.0, 1.0)),
    )
}

/// Computes a deterministic order-independent SHA-256 hex digest over a slice of `TraceRecord`s.
#[must_use]
pub fn compute_corpus_sha256(records: &[TraceRecord]) -> String {
    let mut sorted = records.to_vec();
    sorted.sort_by(|a, b| a.source.cmp(&b.source).then_with(|| a.id.cmp(&b.id)));
    let mut hasher = Sha256::new();
    for rec in &sorted {
        hasher.update(rec.source.as_str().as_bytes());
        hasher.update(b"|");
        hasher.update(rec.id.as_bytes());
        hasher.update(b"|");
        hasher.update(match rec.outcome {
            TraceOutcome::Pass => b"pass",
            TraceOutcome::Fail => b"fail",
        });
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}
