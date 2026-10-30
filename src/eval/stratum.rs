use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::eval::rubric::CaseResult;

/// Stratum configuration specifying the relative weight of a category of cases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StratumConfig {
    pub name: String,
    pub weight: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuiteSummary {
    pub total_cases: usize,
    pub passed_cases: usize,
    pub failed_cases: usize,
    pub blocker_failures: usize,
    pub overall_score: f64,
    pub gate_passed: bool,
    pub gate_reasons: Vec<String>,
    pub strata_scores: BTreeMap<String, f64>,
    pub results: Vec<CaseResult>,
}

/// Aggregates case results with stratified macro-scoring and blocker vetoes.
#[must_use]
pub fn score_suite(
    results: Vec<CaseResult>,
    strata: &[StratumConfig],
    min_score_threshold: f64,
) -> SuiteSummary {
    let total_cases = results.len();
    let passed_cases = results.iter().filter(|r| r.passed).count();
    let failed_cases = total_cases.saturating_sub(passed_cases);

    let mut blocker_failures = 0usize;
    let mut gate_reasons = Vec::new();

    let mut cases_by_stratum: BTreeMap<String, Vec<&CaseResult>> = BTreeMap::new();
    for res in &results {
        if res.blocker_failed {
            blocker_failures += 1;
            gate_reasons.push(format!("blocker failed in case `{}`", res.id));
        }
        cases_by_stratum
            .entry(res.stratum.clone())
            .or_default()
            .push(res);
    }

    let stratum_weight_map: BTreeMap<&str, f64> =
        strata.iter().map(|s| (s.name.as_str(), s.weight)).collect();

    let mut weighted_sum = 0.0;
    let mut total_strata_weight = 0.0;
    let mut strata_scores = BTreeMap::new();

    for (stratum_name, cases) in &cases_by_stratum {
        let avg_score = if cases.is_empty() {
            0.0
        } else {
            let sum: f64 = cases.iter().map(|c| c.score).sum();
            #[allow(clippy::cast_precision_loss)]
            let len = cases.len() as f64;
            sum / len
        };
        let rounded_avg = (avg_score * 1000.0).round() / 1000.0;
        strata_scores.insert(stratum_name.clone(), rounded_avg);

        let weight = stratum_weight_map
            .get(stratum_name.as_str())
            .copied()
            .unwrap_or(1.0);
        weighted_sum += avg_score * weight;
        total_strata_weight += weight;
    }

    let raw_overall = if total_strata_weight > 0.0 {
        weighted_sum / total_strata_weight
    } else {
        0.0
    };
    let overall_score = (raw_overall * 1000.0).round() / 1000.0;

    if overall_score < min_score_threshold {
        gate_reasons.push(format!(
            "overall score {overall_score:.2} below threshold {min_score_threshold:.2}"
        ));
    }

    let gate_passed = blocker_failures == 0 && overall_score >= min_score_threshold;

    SuiteSummary {
        total_cases,
        passed_cases,
        failed_cases,
        blocker_failures,
        overall_score,
        gate_passed,
        gate_reasons,
        strata_scores,
        results,
    }
}
