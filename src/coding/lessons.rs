use serde::{Deserialize, Serialize};

use crate::coding::causal::CausalFailureGraph;

/// Actionable lesson learned for agent memory banks before starting a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonLearned {
    pub precursor: String,
    pub cascade_risk: String,
    pub historical_frequency: usize,
    pub risk_share_pct: f64,
    pub prescriptive_rule: String,
    pub sample_traces: Vec<String>,
}

/// Retrieves proactive lessons learned based on an intended task or tool command.
#[must_use]
pub fn query_lessons(graph: &CausalFailureGraph, query: Option<&str>) -> Vec<LessonLearned> {
    let q = query.map(str::to_lowercase);

    let mut lessons = Vec::new();
    for edge in &graph.edges {
        let precursor = edge.from.label();
        let cascade = edge.to.label();
        let hint = &edge.remediation_hint;

        let matches = match &q {
            Some(needle) => {
                precursor.to_lowercase().contains(needle)
                    || cascade.to_lowercase().contains(needle)
                    || hint.to_lowercase().contains(needle)
            }
            None => true,
        };

        if matches {
            lessons.push(LessonLearned {
                precursor,
                cascade_risk: cascade,
                historical_frequency: edge.count,
                risk_share_pct: edge.share_pct,
                prescriptive_rule: hint.clone(),
                sample_traces: edge.sample_traces.clone(),
            });
        }
    }

    lessons
}
