use serde::{Deserialize, Serialize};

use crate::coding::axial::AxialCategory;
use crate::coding::open::OpenCodedFailure;
use crate::ingest::{TraceRecord, TraceSource};

/// First 3 deterministic metrics promoted from the Top 3 Axial Coding failure categories.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FirstThreeMetrics {
    /// Metric #1: Percentage of command logs + loop events failing due to build-order or lint/contract cascades.
    pub loop_cascade_rate_pct: f64,
    /// Number of loop-guard events where an agent ignored exit code 3 (`loop_streak > 3`).
    pub loop_guard_breach_count: usize,
    /// Metric #2: Percentage of harness + bounded dispatches failing due to binary stub, model catalog, or test pollution drift.
    pub runtime_drift_rate_pct: f64,
    /// Metric #3: Percentage of bounded/harness dispatches killed prematurely on transient stream/TLS reconnect events.
    pub premature_kill_rate_pct: f64,
}

fn round1(val: f64) -> f64 {
    (val * 10.0).round() / 10.0
}

/// Computes the Top 3 metrics with appropriate subsystem denominators.
#[must_use]
pub fn compute_first_three_metrics(
    records: &[TraceRecord],
    failures: &[(OpenCodedFailure, AxialCategory)],
) -> FirstThreeMetrics {
    let command_gate_denominator = records
        .iter()
        .filter(|r| {
            matches!(
                r.source,
                TraceSource::QuietLog
                    | TraceSource::LoopEvent
                    | TraceSource::RustCargo
                    | TraceSource::GoTest
            )
        })
        .count()
        .max(1);

    let dispatch_denominator = records
        .iter()
        .filter(|r| {
            matches!(
                r.source,
                TraceSource::HarnessRun | TraceSource::BoundedCodex
            )
        })
        .count()
        .max(1);

    let loop_cascade_count = failures
        .iter()
        .filter(|(_, cat)| *cat == AxialCategory::LoopCascade)
        .count();
    let runtime_drift_count = failures
        .iter()
        .filter(|(_, cat)| *cat == AxialCategory::RuntimeDrift)
        .count();
    let premature_kill_count = failures
        .iter()
        .filter(|(_, cat)| *cat == AxialCategory::PrematureKill)
        .count();

    let loop_guard_breach_count = records
        .iter()
        .filter(|r| r.source == TraceSource::LoopEvent && r.loop_streak.unwrap_or(0) > 3)
        .count();

    #[allow(clippy::cast_precision_loss)]
    FirstThreeMetrics {
        loop_cascade_rate_pct: round1(
            (loop_cascade_count as f64 / command_gate_denominator as f64) * 100.0,
        ),
        loop_guard_breach_count,
        runtime_drift_rate_pct: round1(
            (runtime_drift_count as f64 / dispatch_denominator as f64) * 100.0,
        ),
        premature_kill_rate_pct: round1(
            (premature_kill_count as f64 / dispatch_denominator as f64) * 100.0,
        ),
    }
}
