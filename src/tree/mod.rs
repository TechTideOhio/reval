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
    pub remediation_hint: Option<String>,
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
            let (open_code, axial, hint) = if r.outcome == TraceOutcome::Pass {
                passed += 1;
                (None, None, None)
            } else {
                failed += 1;
                let (code, _) = annotate_failure_raw(r)
                    .unwrap_or((OpenCode::UnclassifiedFailure, String::new()));
                let ax = axial_category_for(code);
                let remediation = ax.remediation_hint().to_string();
                (Some(code), Some(ax), Some(remediation))
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
                remediation_hint: hint,
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
        #[allow(clippy::cast_precision_loss)]
        let pass_pct = if self.total_nodes > 0 {
            (self.passed_count as f64 / self.total_nodes as f64) * 100.0
        } else {
            0.0
        };

        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>TideOS reval — Multi-Agent Execution Trace Dashboard</title>
  <style>
    :root {{
      --bg: #090d16;
      --card-bg: #111827;
      --card-border: #1f2937;
      --text: #f3f4f6;
      --text-muted: #9ca3af;
      --accent: #38bdf8;
      --accent-glow: rgba(56, 189, 248, 0.15);
      --pass: #10b981;
      --pass-bg: rgba(16, 185, 129, 0.1);
      --fail: #f43f5e;
      --fail-bg: rgba(244, 63, 94, 0.1);
      --warning: #f59e0b;
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", sans-serif;
      background: var(--bg);
      color: var(--text);
      line-height: 1.5;
      padding: 32px 24px;
      max-width: 1380px;
      margin: 0 auto;
    }}
    header {{
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 28px;
      padding-bottom: 20px;
      border-bottom: 1px solid var(--card-border);
    }}
    .brand {{
      display: flex;
      align-items: center;
      gap: 12px;
    }}
    .logo {{
      width: 36px;
      height: 36px;
      background: linear-gradient(135deg, #38bdf8, #818cf8);
      border-radius: 8px;
      display: flex;
      align-items: center;
      justify-content: center;
      font-weight: 800;
      color: #090d16;
      font-size: 1.1rem;
    }}
    h1 {{ font-size: 1.5rem; font-weight: 700; color: #fff; letter-spacing: -0.02em; }}
    .subtitle {{ font-size: 0.85rem; color: var(--text-muted); font-family: ui-monospace, monospace; }}
    .header-actions {{ display: flex; gap: 10px; align-items: center; }}
    .btn {{
      background: #1e293b;
      border: 1px solid #334155;
      color: var(--text);
      padding: 8px 14px;
      border-radius: 6px;
      font-size: 0.85rem;
      cursor: pointer;
      transition: all 0.15s ease;
      font-family: inherit;
    }}
    .btn:hover {{ background: #334155; border-color: #475569; }}
    .stats-grid {{
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: 16px;
      margin-bottom: 24px;
    }}
    .stat-card {{
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 10px;
      padding: 16px 20px;
    }}
    .stat-label {{ font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-muted); margin-bottom: 6px; font-weight: 600; }}
    .stat-val {{ font-size: 1.8rem; font-weight: 700; color: #fff; }}
    .stat-val.pass {{ color: var(--pass); }}
    .stat-val.fail {{ color: var(--fail); }}
    .controls {{
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 10px;
      padding: 14px 18px;
      margin-bottom: 24px;
      display: flex;
      flex-wrap: wrap;
      gap: 12px;
      align-items: center;
      justify-content: space-between;
    }}
    .filter-pills {{ display: flex; gap: 8px; flex-wrap: wrap; }}
    .pill {{
      background: #1e293b;
      border: 1px solid transparent;
      color: var(--text-muted);
      padding: 6px 12px;
      border-radius: 20px;
      font-size: 0.8rem;
      cursor: pointer;
      transition: all 0.15s ease;
    }}
    .pill:hover {{ color: #fff; background: #334155; }}
    .pill.active {{ background: rgba(56, 189, 248, 0.15); border-color: var(--accent); color: var(--accent); font-weight: 600; }}
    .search-input {{
      background: #0f172a;
      border: 1px solid #334155;
      color: #fff;
      padding: 8px 14px;
      border-radius: 6px;
      font-size: 0.85rem;
      width: 320px;
      outline: none;
      transition: border-color 0.15s;
    }}
    .search-input:focus {{ border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }}
    .trace-list {{ display: flex; flex-direction: column; gap: 10px; }}
    .trace-item {{
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 8px;
      overflow: hidden;
      transition: border-color 0.15s;
    }}
    .trace-item:hover {{ border-color: #374151; }}
    .trace-header {{
      padding: 12px 18px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      cursor: pointer;
      user-select: none;
    }}
    .trace-main {{ display: flex; align-items: center; gap: 12px; flex: 1; min-width: 0; }}
    .status-badge {{
      display: inline-flex;
      align-items: center;
      justify-content: center;
      padding: 3px 8px;
      border-radius: 4px;
      font-size: 0.72rem;
      font-weight: 700;
      font-family: ui-monospace, monospace;
    }}
    .status-badge.pass {{ background: var(--pass-bg); color: var(--pass); border: 1px solid rgba(16, 185, 129, 0.3); }}
    .status-badge.fail {{ background: var(--fail-bg); color: var(--fail); border: 1px solid rgba(244, 63, 94, 0.3); }}
    .agent-tag {{
      background: #1e293b;
      color: #cbd5e1;
      padding: 2px 7px;
      border-radius: 4px;
      font-size: 0.75rem;
      font-family: ui-monospace, monospace;
      border: 1px solid #334155;
    }}
    .trace-label {{
      font-size: 0.9rem;
      font-weight: 500;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      color: #e2e8f0;
    }}
    .trace-tags {{ display: flex; gap: 6px; align-items: center; }}
    .tag-open {{
      background: rgba(124, 58, 237, 0.15);
      border: 1px solid rgba(124, 58, 237, 0.3);
      color: #c084fc;
      padding: 2px 8px;
      border-radius: 4px;
      font-size: 0.72rem;
      font-family: ui-monospace, monospace;
    }}
    .tag-axial {{
      background: rgba(2, 132, 199, 0.15);
      border: 1px solid rgba(2, 132, 199, 0.3);
      color: #38bdf8;
      padding: 2px 8px;
      border-radius: 4px;
      font-size: 0.72rem;
      font-family: ui-monospace, monospace;
    }}
    .trace-drawer {{
      border-top: 1px solid var(--card-border);
      background: #0b1120;
      padding: 16px 20px;
      display: none;
    }}
    .trace-drawer.open {{ display: block; }}
    .remediation-box {{
      background: rgba(245, 158, 11, 0.08);
      border: 1px solid rgba(245, 158, 11, 0.25);
      border-radius: 6px;
      padding: 10px 14px;
      margin-bottom: 12px;
      font-size: 0.85rem;
      color: #fde68a;
    }}
    .diag-box {{
      background: #030712;
      border: 1px solid #1f2937;
      border-radius: 6px;
      padding: 12px;
      font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
      font-size: 0.8rem;
      color: #94a3b8;
      overflow-x: auto;
      white-space: pre-wrap;
      max-height: 280px;
    }}
  </style>
</head>
<body>
  <header>
    <div class="brand">
      <div class="logo">R</div>
      <div>
        <h1>reval — Multi-Agent Trace Tree</h1>
        <div class="subtitle">TideOS Eval Kit (Tool 1) • Empirical Production Error Analysis</div>
      </div>
    </div>
    <div class="header-actions">
      <button class="btn" onclick="toggleAll()">Expand All</button>
      <button class="btn" onclick="copyJson()">Copy Raw JSON</button>
    </div>
  </header>

  <section class="stats-grid">
    <div class="stat-card">
      <div class="stat-label">Total Traces Sampled</div>
      <div class="stat-val">{total}</div>
    </div>
    <div class="stat-card">
      <div class="stat-label">Pass@1 Empirical Reliability</div>
      <div class="stat-val pass">{pass_pct:.1}%</div>
    </div>
    <div class="stat-card">
      <div class="stat-label">Passed Traces</div>
      <div class="stat-val pass">{pass_c}</div>
    </div>
    <div class="stat-card">
      <div class="stat-label">Open-Coded Failures</div>
      <div class="stat-val fail">{fail_c}</div>
    </div>
  </section>

  <section class="controls">
    <div class="filter-pills">
      <button class="pill active" onclick="setFilter('all', this)">All ({total})</button>
      <button class="pill" onclick="setFilter('fail', this)">Failures Only ({fail_c})</button>
      <button class="pill" onclick="setFilter('loop_cascade', this)">Loop Cascades</button>
      <button class="pill" onclick="setFilter('test_iteration', this)">Test Iteration</button>
      <button class="pill" onclick="setFilter('runtime_drift', this)">Runtime Drift</button>
      <button class="pill" onclick="setFilter('pass', this)">Passed ({pass_c})</button>
    </div>
    <input type="text" id="searchInput" class="search-input" placeholder="Search commands, agents, error codes..." oninput="filterTraces()" />
  </section>

  <section id="traceContainer" class="trace-list"></section>

  <script>
    const dataset = {json_data};
    let activeFilter = 'all';
    let allExpanded = false;

    function setFilter(filter, el) {{
      activeFilter = filter;
      document.querySelectorAll('.filter-pills .pill').forEach(p => p.classList.remove('active'));
      el.classList.add('active');
      filterTraces();
    }}

    function filterTraces() {{
      const query = document.getElementById('searchInput').value.toLowerCase();
      const filtered = dataset.root_nodes.filter(n => {{
        const matchesQuery = !query ||
          n.label.toLowerCase().includes(query) ||
          n.agent.toLowerCase().includes(query) ||
          (n.open_code && n.open_code.toLowerCase().includes(query)) ||
          (n.axial_category && n.axial_category.toLowerCase().includes(query)) ||
          (n.diagnostic && n.diagnostic.toLowerCase().includes(query));

        if (!matchesQuery) return false;

        if (activeFilter === 'all') return true;
        if (activeFilter === 'pass') return n.outcome === 'pass';
        if (activeFilter === 'fail') return n.outcome !== 'pass';
        if (activeFilter === 'loop_cascade') return n.axial_category === 'loop_cascade';
        if (activeFilter === 'test_iteration') return n.axial_category === 'test_iteration';
        if (activeFilter === 'runtime_drift') return n.axial_category === 'runtime_drift';

        return true;
      }});

      render(filtered);
    }}

    function render(nodes) {{
      const container = document.getElementById('traceContainer');
      container.innerHTML = '';

      if (nodes.length === 0) {{
        container.innerHTML = '<div style="text-align: center; color: var(--text-muted); padding: 40px;">No matching execution traces found.</div>';
        return;
      }}

      nodes.forEach((n, idx) => {{
        const item = document.createElement('div');
        item.className = 'trace-item';

        const isPass = n.outcome === 'pass';
        const badgeClass = isPass ? 'pass' : 'fail';
        const badgeText = isPass ? 'PASS' : 'FAIL';

        let tagsHtml = '';
        if (n.open_code) tagsHtml += `<span class="tag-open">${{escapeHtml(n.open_code)}}</span>`;
        if (n.axial_category) tagsHtml += `<span class="tag-axial">${{escapeHtml(n.axial_category)}}</span>`;

        let drawerContent = '';
        if (n.remediation_hint) {{
          drawerContent += `<div class="remediation-box"><strong>Remediation Rule:</strong> ${{escapeHtml(n.remediation_hint)}}</div>`;
        }}
        if (n.diagnostic && n.diagnostic.trim().length > 0) {{
          drawerContent += `<div class="diag-box">${{escapeHtml(n.diagnostic.trim())}}</div>`;
        }} else if (isPass) {{
          drawerContent += `<div style="color: var(--pass); font-size: 0.85rem;">✓ Run completed with zero errors.</div>`;
        }}

        item.innerHTML = `
          <div class="trace-header" onclick="toggleDrawer(${{idx}})">
            <div class="trace-main">
              <span class="status-badge ${{badgeClass}}">${{badgeText}}</span>
              <span class="agent-tag">${{escapeHtml(n.agent)}}</span>
              <span class="trace-label" title="${{escapeHtml(n.label)}}">${{escapeHtml(n.label)}}</span>
            </div>
            <div class="trace-tags">
              ${{tagsHtml}}
              <span style="color: var(--text-muted); font-size: 0.8rem; margin-left: 8px;">▾</span>
            </div>
          </div>
          <div id="drawer-${{idx}}" class="trace-drawer ${{allExpanded ? 'open' : ''}}">
            <div style="font-size: 0.78rem; color: var(--text-muted); margin-bottom: 8px; font-family: ui-monospace, monospace;">Trace ID: ${{escapeHtml(n.id)}}</div>
            ${{drawerContent}}
          </div>
        `;
        container.appendChild(item);
      }});
    }}

    function toggleDrawer(idx) {{
      const d = document.getElementById(`drawer-${{idx}}`);
      if (d) d.classList.toggle('open');
    }}

    function toggleAll() {{
      allExpanded = !allExpanded;
      document.querySelectorAll('.trace-drawer').forEach(d => {{
        if (allExpanded) d.classList.add('open');
        else d.classList.remove('open');
      }});
    }}

    function copyJson() {{
      navigator.clipboard.writeText(JSON.stringify(dataset, null, 2)).then(() => {{
        alert('Trace tree JSON copied to clipboard!');
      }});
    }}

    function escapeHtml(str) {{
      if (!str) return '';
      return String(str).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
    }}

    render(dataset.root_nodes);
  </script>
</body>
</html>"#,
            total = self.total_nodes,
            pass_c = self.passed_count,
            fail_c = self.failed_count,
            pass_pct = pass_pct,
            json_data = json_data
        );

        fs::write(output_path, html)
    }
}
