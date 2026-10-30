use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use serde::{Deserialize, Serialize};

use crate::coding::axial::{axial_category_for, AxialCategory};
use crate::coding::open::{annotate_failure_raw, OpenCode};
use crate::ingest::{TraceOutcome, TraceRecord};

/// Node in the causal failure graph.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "id")]
pub enum CausalNode {
    Precursor(OpenCode),
    Cascade(AxialCategory),
}

impl CausalNode {
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Precursor(code) => code.as_str().to_string(),
            Self::Cascade(cat) => cat.as_str().to_string(),
        }
    }
}

/// Directed causal edge representing how an initial trigger/warning transitions to a cascade.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalEdge {
    pub from: CausalNode,
    pub to: CausalNode,
    pub count: usize,
    pub share_pct: f64,
    pub remediation_hint: String,
    pub sample_traces: Vec<String>,
}

/// Causal failure graph linking early tool warnings to downstream cascade failures.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CausalFailureGraph {
    pub total_failure_traces: usize,
    pub nodes: Vec<CausalNode>,
    pub edges: Vec<CausalEdge>,
}

impl CausalFailureGraph {
    /// Constructs a causal graph from a trace corpus.
    #[must_use]
    pub fn from_traces(records: &[TraceRecord]) -> Self {
        let mut transitions: BTreeMap<(OpenCode, AxialCategory), Vec<String>> = BTreeMap::new();
        let mut precursor_counts: BTreeMap<OpenCode, usize> = BTreeMap::new();
        let mut total_failures = 0;

        for r in records {
            if r.outcome == TraceOutcome::Pass {
                continue;
            }
            total_failures += 1;
            let (open_code, _) =
                annotate_failure_raw(r).unwrap_or((OpenCode::UnclassifiedFailure, String::new()));
            let axial = axial_category_for(open_code);

            *precursor_counts.entry(open_code).or_insert(0) += 1;
            transitions
                .entry((open_code, axial))
                .or_default()
                .push(r.id.clone());
        }

        let mut node_set = BTreeSet::new();
        let mut edges = Vec::new();

        for ((open_code, axial), traces) in transitions {
            let from = CausalNode::Precursor(open_code);
            let to = CausalNode::Cascade(axial);
            node_set.insert(from.clone());
            node_set.insert(to.clone());

            let count = traces.len();
            #[allow(clippy::cast_precision_loss)]
            let share_pct = if total_failures > 0 {
                (count as f64 / total_failures as f64) * 100.0
            } else {
                0.0
            };

            let remediation_hint = axial.remediation_hint().to_string();
            let sample_traces = traces.into_iter().take(3).collect();

            edges.push(CausalEdge {
                from,
                to,
                count,
                share_pct: (share_pct * 10.0).round() / 10.0,
                remediation_hint,
                sample_traces,
            });
        }

        edges.sort_by_key(|a| std::cmp::Reverse(a.count));

        Self {
            total_failure_traces: total_failures,
            nodes: node_set.into_iter().collect(),
            edges,
        }
    }

    /// Renders the causal graph in Mermaid flowchart syntax.
    #[must_use]
    pub fn to_mermaid(&self) -> String {
        let mut out = String::from("flowchart LR\n");
        for (idx, edge) in self.edges.iter().enumerate() {
            let from_id = edge.from.label().replace('-', "_");
            let to_id = edge.to.label().replace('-', "_");
            let _ = writeln!(
                out,
                "    {from_id}[\"{}\"] -->|n={}, {:.1}%| {to_id}[\"{}\"]",
                edge.from.label(),
                edge.count,
                edge.share_pct,
                edge.to.label()
            );
            if idx >= 10 {
                break;
            }
        }
        out
    }

    /// Renders the causal graph in Graphviz DOT format.
    #[must_use]
    pub fn to_dot(&self) -> String {
        let mut out =
            String::from("digraph CausalFailureGraph {\n  rankdir=LR;\n  node [shape=box];\n");
        for edge in &self.edges {
            let _ = writeln!(
                out,
                "  \"{}\" -> \"{}\" [label=\"n={} ({:.1}%)\"];",
                edge.from.label(),
                edge.to.label(),
                edge.count,
                edge.share_pct
            );
        }
        out.push_str("}\n");
        out
    }
}
