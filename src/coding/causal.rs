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

/// Directed causal edge enriched with Bayesian posterior probability and credible intervals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BayesianCausalEdge {
    pub from: CausalNode,
    pub to: CausalNode,
    pub transition_count: usize,
    pub total_precursor_count: usize,
    pub bayes_probability_pct: f64,
    pub credible_interval_low_pct: f64,
    pub credible_interval_high_pct: f64,
    pub confidence_score: f64,
    pub remediation_hint: String,
    pub sample_traces: Vec<String>,
}

/// Dynamic, persistent causal failure graph store for autonomous agent memory banks (Mimir / `NornicDB` style).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentGraphStore {
    pub version: String,
    pub total_traces_observed: usize,
    pub total_failures_observed: usize,
    pub nodes: Vec<CausalNode>,
    pub edges: Vec<BayesianCausalEdge>,
}

impl Default for PersistentGraphStore {
    fn default() -> Self {
        Self {
            version: "0.2.0".to_string(),
            total_traces_observed: 0,
            total_failures_observed: 0,
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}

impl PersistentGraphStore {
    /// Loads an existing persistent graph store from disk or initializes a new default store.
    pub fn load_or_create(path: &std::path::Path) -> anyhow::Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let store: Self = serde_json::from_str(&content)?;
            Ok(store)
        } else {
            Ok(Self::default())
        }
    }

    /// Saves the persistent graph store to a JSON file.
    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Incrementally updates the graph with newly observed traces using Bayesian updating.
    #[allow(clippy::cast_precision_loss)]
    pub fn ingest_traces(&mut self, records: &[TraceRecord]) {
        self.total_traces_observed += records.len();

        let mut precursor_totals: BTreeMap<OpenCode, usize> = BTreeMap::new();
        let mut transitions: BTreeMap<(OpenCode, AxialCategory), Vec<String>> = BTreeMap::new();

        // Seed with existing edge counts
        for edge in &self.edges {
            if let (CausalNode::Precursor(open), CausalNode::Cascade(axial)) =
                (&edge.from, &edge.to)
            {
                *precursor_totals.entry(*open).or_insert(0) += edge.total_precursor_count;
                transitions
                    .entry((*open, *axial))
                    .or_insert_with(|| edge.sample_traces.clone());
            }
        }

        for r in records {
            if r.outcome == TraceOutcome::Pass {
                continue;
            }
            self.total_failures_observed += 1;
            let (open_code, _) =
                annotate_failure_raw(r).unwrap_or((OpenCode::UnclassifiedFailure, String::new()));
            let axial = axial_category_for(open_code);

            *precursor_totals.entry(open_code).or_insert(0) += 1;
            transitions
                .entry((open_code, axial))
                .or_default()
                .push(r.id.clone());
        }

        let mut node_set = BTreeSet::new();
        let mut new_edges = Vec::new();

        // Laplace prior: alpha = 1.0, beta = 1.0
        let alpha = 1.0f64;
        let beta = 1.0f64;

        for ((open_code, axial), traces) in transitions {
            let from = CausalNode::Precursor(open_code);
            let to = CausalNode::Cascade(axial);
            node_set.insert(from.clone());
            node_set.insert(to.clone());

            let k_usize = traces.len();
            let n_usize =
                (*precursor_totals.get(&open_code).unwrap_or(&traces.len())).max(traces.len());

            let k = k_usize as f64;
            let n = n_usize as f64;

            let p_bayes = (k + alpha) / (n + alpha + beta);
            let variance = ((k + alpha) * (n - k + beta))
                / ((n + alpha + beta).powi(2) * (n + alpha + beta + 1.0));
            let std_dev = variance.sqrt();

            let low = (p_bayes - 1.96 * std_dev).max(0.0);
            let high = (p_bayes + 1.96 * std_dev).min(1.0);
            let confidence = (1.0 - 2.0 * std_dev).clamp(0.0, 1.0);

            let remediation_hint = axial.remediation_hint().to_string();
            let sample_traces = traces.into_iter().take(3).collect();

            new_edges.push(BayesianCausalEdge {
                from,
                to,
                transition_count: k_usize,
                total_precursor_count: n_usize,
                bayes_probability_pct: (p_bayes * 1000.0).round() / 10.0,
                credible_interval_low_pct: (low * 1000.0).round() / 10.0,
                credible_interval_high_pct: (high * 1000.0).round() / 10.0,
                confidence_score: (confidence * 1000.0).round() / 1000.0,
                remediation_hint,
                sample_traces,
            });
        }

        new_edges.sort_by_key(|e| std::cmp::Reverse(e.transition_count));
        self.nodes = node_set.into_iter().collect();
        self.edges = new_edges;
    }

    /// Renders the persistent Bayesian causal graph in Mermaid flowchart syntax.
    #[must_use]
    pub fn to_mermaid(&self) -> String {
        let mut out = String::from("flowchart LR\n");
        for (idx, edge) in self.edges.iter().enumerate() {
            let from_id = edge.from.label().replace('-', "_");
            let to_id = edge.to.label().replace('-', "_");
            let _ = writeln!(
                out,
                "    {from_id}[\"{}\"] -->|n={}, P={:.1}% [{:.1}%, {:.1}%]| {to_id}[\"{}\"]",
                edge.from.label(),
                edge.transition_count,
                edge.bayes_probability_pct,
                edge.credible_interval_low_pct,
                edge.credible_interval_high_pct,
                edge.to.label()
            );
            if idx >= 10 {
                break;
            }
        }
        out
    }

    /// Renders the persistent Bayesian causal graph in Graphviz DOT format.
    #[must_use]
    pub fn to_dot(&self) -> String {
        let mut out =
            String::from("digraph BayesianCausalGraph {\n  rankdir=LR;\n  node [shape=box];\n");
        for edge in &self.edges {
            let _ = writeln!(
                out,
                "  \"{}\" -> \"{}\" [label=\"n={} (P={:.1}% CI=[{:.1}%, {:.1}%])\"];",
                edge.from.label(),
                edge.to.label(),
                edge.transition_count,
                edge.bayes_probability_pct,
                edge.credible_interval_low_pct,
                edge.credible_interval_high_pct
            );
        }
        out.push_str("}\n");
        out
    }
}
