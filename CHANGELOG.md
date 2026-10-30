# Changelog

All notable changes to `reval` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.0] - 2026-10-30

### Added
- **Trace Ingestion Engine**:
  - Streaming JSON parser for Go (`go test -json`).
  - Streaming JSON event parser for Rust (`cargo test --format=json` and `cargo clippy --message-format=json`).
  - Autonomous multi-agent harness run ingestion (`agent-harness`, bounded Codex runs, quiet runner logs).
  - High-entropy credential scrubber redacting GitHub PATs, Anthropic API keys, OpenAI keys, private keys, and bearer tokens.
- **Qualitative Coding Pipeline (Husain/Shankar)**:
  - Deterministic free-text open coding for agent failure trajectories.
  - Axial coding clustering failures into 6 structural categories.
  - Automatic ranking and Top 3 metric derivation (`loop_cascade_rate_pct`, `runtime_drift_rate_pct`, `premature_kill_rate_pct`).
  - Pass@1, Pass@3, and Pass^3 reliability calculation.
- **Causal Failure Transition Graph (Sweet / Graph-RAG)**:
  - Directed graph representation linking precursor warnings to downstream cascades.
  - Mermaid flowchart, Graphviz DOT, and canonical JSON output formats.
  - Proactive "lessons learned" retrieval engine for local agent memory banks (Mimir / NornicDB).
- **Execution Trace Tree & Dashboard (Chase / LangSmith)**:
  - Hierarchical trace tree parser aggregating runs, steps, and diagnostics.
  - Self-contained, responsive interactive HTML trace tree dashboard (`tree.html`).
- **Fail-Closed Quality Gating & Proof Generator**:
  - `reval gate` enforcing ratcheted `baseline.json` thresholds.
  - SHA-256 canonical JSON cryptographically signed proof bundle generation (`proof.json`).
- **Stratified Golden Evaluation Suites**:
  - `reval eval suite` runner with blocker veto enforcement (blocker failures immediately score 0.0).
  - Hybrid judge checks (`Check::HybridJudge`) supporting deterministic semantic heuristics and external rubric scripts.
- **Poltergeist-Style Loop Watcher (Steinberger)**:
  - `reval watch` active polling daemon monitoring harness runs with audible terminal bell (`\x07`) and high-visibility alert banner on loop cascades.
- **Model Context Protocol (MCP) Server**:
  - Native stdio JSON-RPC 2.0 MCP server exposing tools (`reval_check_gate`, `reval_analyze_traces`, `reval_get_lessons`, `reval_repo_hygiene`) and resources (`reval://metrics/top3`, `reval://graph/causal`, `reval://lessons/latest`).
- **Static Repository Hygiene Scanner**:
  - Scans for committed `.env` secrets, multi-agent lockfile desyncs, and dangling skill references.
- **Harness Envelope JSON Protocol**:
  - Standard `--json` flag on all subcommands emitting TechTide's canonical `HarnessEnvelope` schema.
