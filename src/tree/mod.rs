use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::coding::axial::{axial_category_for, AxialCategory};
use crate::coding::open::{annotate_failure_raw, OpenCode};
use crate::ingest::{TraceOutcome, TraceRecord};

/// Node in a hierarchical execution trace tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceTreeNode {
    pub id: String,
    pub label: String,
    pub outcome: TraceOutcome,
    pub agent: String,
    pub exit_code: Option<i64>,
    pub open_code: Option<OpenCode>,
    pub axial_category: Option<AxialCategory>,
    pub diagnostic: String,
    pub children: Vec<TraceTreeNode>,
}

/// Execution trace tree aggregating multi-agent trajectories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceTree {
    pub total_nodes: usize,
    pub passed_count: usize,
    pub failed_count: usize,
    pub root_nodes: Vec<TraceTreeNode>,
}

impl TraceTree {
    /// Builds a trace tree from a slice of trace records.
    #[must_use]
    pub fn from_traces(records: &[TraceRecord]) -> Self {
        let mut root_nodes = Vec::new();
        let mut passed = 0;
        let mut failed = 0;

        for r in records {
            let (open_code, axial) = if r.outcome == TraceOutcome::Pass {
                passed += 1;
                (None, None)
            } else {
                failed += 1;
                let (code, _) = annotate_failure_raw(r)
                    .unwrap_or((OpenCode::UnclassifiedFailure, String::new()));
                (Some(code), Some(axial_category_for(code)))
            };

            root_nodes.push(TraceTreeNode {
                id: r.id.clone(),
                label: r.command_or_summary.clone(),
                outcome: r.outcome,
                agent: r.agent_or_runner.clone(),
                exit_code: r.exit_code,
                open_code,
                axial_category: axial,
                diagnostic: r.diagnostic_excerpt.clone(),
                children: Vec::new(),
            });
        }

        Self {
            total_nodes: records.len(),
            passed_count: passed,
            failed_count: failed,
            root_nodes,
        }
    }

    /// Exports an interactive, self-contained HTML dashboard for visual trace inspection.
    pub fn export_html(&self, output_path: &Path) -> std::io::Result<()> {
        let json_data = serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string());
        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>TideOS reval — Multi-Agent Trace Tree Dashboard</title>
  <style>
    body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, monospace; margin: 20px; background: #0f172a; color: #f8fafc; }}
    h1 {{ font-size: 1.4rem; color: #38bdf8; }}
    .stats {{ display: flex; gap: 15px; margin-bottom: 20px; }}
    .card {{ background: #1e293b; padding: 12px 18px; border-radius: 6px; border: 1px solid #334155; }}
    .card .val {{ font-size: 1.5rem; font-weight: bold; }}
    .pass {{ color: #4ade80; }}
    .fail {{ color: #f87171; }}
    input {{ background: #1e293b; border: 1px solid #475569; color: #fff; padding: 8px 12px; border-radius: 4px; width: 300px; margin-bottom: 15px; }}
    .node {{ background: #1e293b; margin: 6px 0; padding: 10px; border-radius: 4px; border-left: 4px solid #64748b; font-size: 0.9rem; }}
    .node.pass {{ border-left-color: #4ade80; }}
    .node.fail {{ border-left-color: #f87171; }}
    .badge {{ display: inline-block; padding: 2px 6px; border-radius: 3px; font-size: 0.75rem; font-weight: bold; margin-left: 8px; }}
    .badge-code {{ background: #7c3aed; color: #fff; }}
    .badge-cat {{ background: #0284c7; color: #fff; }}
    pre {{ background: #090d16; padding: 8px; border-radius: 4px; overflow-x: auto; color: #cbd5e1; font-size: 0.8rem; }}
  </style>
</head>
<body>
  <h1>TideOS reval — Execution Trace Tree Dashboard</h1>
  <div class="stats">
    <div class="card"><div>Total Traces</div><div class="val">{total}</div></div>
    <div class="card"><div>Passed</div><div class="val pass">{pass_c}</div></div>
    <div class="card"><div>Failed</div><div class="val fail">{fail_c}</div></div>
  </div>
  <input type="text" id="filter" placeholder="Filter by command, agent, or error code..." oninput="filterNodes()" />
  <div id="tree"></div>

  <script>
    const data = {json_data};
    const treeDiv = document.getElementById('tree');

    function render(nodes) {{
      treeDiv.innerHTML = '';
      nodes.forEach(n => {{
        const el = document.createElement('div');
        el.className = 'node ' + (n.outcome === 'pass' ? 'pass' : 'fail');
        let badges = '';
        if (n.open_code) badges += `<span class="badge badge-code">${{n.open_code}}</span>`;
        if (n.axial_category) badges += `<span class="badge badge-cat">${{n.axial_category}}</span>`;
        el.innerHTML = `
          <div><strong>[${{n.agent}}]</strong> ${{escapeHtml(n.label)}} ${{badges}}</div>
          ${{n.diagnostic ? `<pre>${{escapeHtml(n.diagnostic.slice(0, 300))}}</pre>` : ''}}
        `;
        treeDiv.appendChild(el);
      }});
    }}

    function filterNodes() {{
      const query = document.getElementById('filter').value.toLowerCase();
      const filtered = data.root_nodes.filter(n =>
        n.label.toLowerCase().includes(query) ||
        n.agent.toLowerCase().includes(query) ||
        (n.open_code && n.open_code.toLowerCase().includes(query)) ||
        (n.axial_category && n.axial_category.toLowerCase().includes(query))
      );
      render(filtered);
    }}

    function escapeHtml(str) {{
      return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }}

    render(data.root_nodes);
  </script>
</body>
</html>"#,
            total = self.total_nodes,
            pass_c = self.passed_count,
            fail_c = self.failed_count,
            json_data = json_data
        );

        fs::write(output_path, html)
    }
}
