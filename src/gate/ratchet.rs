use serde::{Deserialize, Serialize};

use crate::coding::{
    annotate_failure, cluster_axial_categories, compute_first_three_metrics, AxialClusterSummary,
    FirstThreeMetrics,
};
use crate::error::RevalError;
use crate::gate::reliability::{compute_corpus_sha256, compute_reliability, ReliabilityMetrics};
use crate::ingest::{TraceOutcome, TraceRecord};

/// Complete empirical evaluation report produced by trace analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevalReport {
    pub corpus_sha256: String,
    pub reliability: ReliabilityMetrics,
    pub first_three_metrics: FirstThreeMetrics,
    pub top_3_axial_categories: Vec<AxialClusterSummary>,
    pub all_axial_categories: Vec<AxialClusterSummary>,
}

/// Baseline ratchet configuration stored in `baseline.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaselineRatchet {
    pub schema_version: u32,
    pub max_loop_cascade_rate_pct: f64,
    pub max_loop_guard_breach_count: usize,
    pub max_runtime_drift_rate_pct: f64,
    pub max_premature_kill_rate_pct: f64,
    pub min_pass_at_1: f64,
    pub min_pass_at_3: f64,
}

/// Evaluates a slice of production traces: runs open coding, axial coding, and computes Top 3 metrics.
pub fn evaluate_traces(
    records: &[TraceRecord],
    min_traces: usize,
) -> Result<RevalReport, RevalError> {
    if records.len() < min_traces {
        return Err(RevalError::InsufficientTraces {
            found: records.len(),
            minimum: min_traces,
        });
    }

    let total_traces = records.len();
    let passed_traces = records
        .iter()
        .filter(|r| r.outcome == TraceOutcome::Pass)
        .count();
    let failed_traces = total_traces.saturating_sub(passed_traces);
    let (pass_at_1, pass_at_3, pass_hat_3) = compute_reliability(total_traces, passed_traces, 3);

    let failures: Vec<_> = records.iter().filter_map(annotate_failure).collect();
    let all_axial_categories = cluster_axial_categories(&failures);
    let top_3_axial_categories: Vec<_> = all_axial_categories.iter().take(3).cloned().collect();

    let first_three_metrics = compute_first_three_metrics(records, &failures);

    Ok(RevalReport {
        corpus_sha256: compute_corpus_sha256(records),
        reliability: ReliabilityMetrics {
            total_traces,
            passed_traces,
            failed_traces,
            pass_at_1,
            pass_at_3,
            pass_hat_3,
        },
        first_three_metrics,
        top_3_axial_categories,
        all_axial_categories,
    })
}
