# reval: TideOS Eval Kit (Tool 1)

<div align="center">

**Deterministic evaluation, trace error analysis, causal failure graphs, and fail-closed quality gating for autonomous AI agent harnesses and software factories.**

[![CI](https://github.com/TechTideOhio/reval/actions/workflows/ci.yml/badge.svg)](https://github.com/TechTideOhio/reval/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/TechTideOhio/reval?color=blue&label=release)](https://github.com/TechTideOhio/reval/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-emerald.svg)](LICENSE)
[![Rust: 2024 / 1.82+](https://img.shields.io/badge/Rust-2024%20%2F%201.82%2B-orange.svg)](Cargo.toml)
[![Zero-FFI](https://img.shields.io/badge/Dependencies-Zero--FFI%20Pure%20Rust-purple.svg)](Cargo.toml)

[Quickstart](#60-second-quickstart) • [Core Capabilities](#core-capabilities-architected-for-real-agent-engineers) • [Architecture](#architecture) • [Dashboard](#5-execution-trace-tree-and-visual-dashboard-chase--langsmith-ready) • [Contributing](CONTRIBUTING.md)

</div>

---

## Why reval?

Most AI evaluation frameworks rely on LLM-as-a-judge prompts that suffer from non-determinism, model drift, high token costs, and prompt injection vulnerabilities. When an agent enters an infinite repair loop, breaks a build dependency, or leaks an API credential, an LLM judge cannot be trusted to fail closed in CI.

`reval` implements the empirical evaluation methodology codified by Hamel Husain, Shreya Shankar, and Anthropic/OpenAI research:

> *"Start with error analysis, not metrics. Sample 100+ production traces. Annotate each failure in free text (open coding), then cluster them into failure categories (axial coding). Count frequency per category. Your top 3 categories are your first metrics."*

`reval` is a single, zero-dependency, pure-Rust CLI tool with no `-sys` C libraries, no external Python runtimes, and no LLM API calls in the verification path. It evaluates agent runs deterministically in milliseconds.

---

## Feature Comparison

| Capability | `reval` (TideOS) | LangSmith / Tracing | Promptfoo | PyTest / Unit Tests |
|---|:---:|:---:|:---:|:---:|
| **Empirical Trace Error Analysis** | **Yes** (Husain/Shankar) | UI manual tag | No | No |
| **Deterministic CI Gate (Zero LLM)** | **Yes** (< 0.2s) | Paid cloud eval | Slow prompt eval | Yes (Code only) |
| **Blocker Veto (Score 0.0 on Key Leaks)** | **Yes** (Fail-closed) | No (Averages) | Configurable | Assert fail |
| **Causal Failure Graph (Graph-RAG)** | **Yes** (Mermaid/DOT/JSON) | Trace tree only | No | No |
| **Active Loop Watcher (Poltergeist-Style)** | **Yes** (Terminal bell) | No | No | No |
| **Native Stdio MCP Server** | **Yes** (JSON-RPC) | API / SDK | No | No |
| **Cryptographic Proofs (SHA-256)** | **Yes** (`proof.json`) | Cloud DB | No | No |
| **Binary Portability** | **Pure Rust** (Single binary) | SaaS / Python | Node.js | Python / C-deps |

---

## Core Capabilities (Architected for Real Agent Engineers)

### 1. Poltergeist-Style Loop Watcher (Steinberger-Ready)
Autonomous CLI agents running in parallel grids can spiral into runaway repair loops, burning CPU and tokens. `reval watch` runs an active polling daemon over harness logs and terminal sessions. The instant a loop cascade or circuit breaker is breached, it sounds an audible terminal bell (`\x07`) and prints an immediate high-visibility alert:

```bash
reval watch --root . --interval-ms 800
```

```text
[WATCH] reval watcher active (root: ., poll: 800ms)
Watching for agent runs and monitoring loop cascades... (Ctrl+C to stop)

[PASS] [2026-10-30T14-24-53-cargo-fmt] cargo fmt (agent: q.mjs)
[FAIL] [2026-10-30T11-59-45-pnpm-build] pnpm build (agent: vsp)

==================== [ALARM: RUNAWAY LOOP CASCADE] ====================
Trace: 2026-10-30T11-59-45-pnpm-build
Open code: TYPECHECK-BUILD-ORDER-CASCADE
Axial category: loop_cascade
Diagnostic: error TS2307: Cannot find module '@vsp/contracts'
Remediation: Rebuild upstream @vsp/contracts + regenerate types before downstream typecheck; halt immediately on q.mjs exit 3.
=========================================================================
```

### 2. Native Model Context Protocol (MCP) Server
Claude Code, Codex, and Cursor can run `reval` directly in-session via standard stdio JSON-RPC without subshell gymnastics:

```bash
reval mcp --root .
```

- **Tools**: `reval_check_gate`, `reval_analyze_traces`, `reval_get_lessons`, `reval_repo_hygiene`.
- **Resources**: `reval://metrics/top3`, `reval://graph/causal`, `reval://lessons/latest`.

### 3. Causal Failure Transition Graph (Sweet / `orneryd`-Ready)
Instead of treating failure modes as flat static buckets, `reval trace graph` constructs a directed causal graph (Graph-RAG style) linking open-code precursor warnings (such as `LINT-RATCHET` and `STREAM-DISCONNECT`) to downstream cascade events (`loop_cascade`, `premature_kill`):

```bash
reval trace graph --format mermaid
```

```mermaid
flowchart LR
    VITEST_ASSERTION_ITERATION["VITEST-ASSERTION-ITERATION"] -->|n=37, 33.6%| test_iteration["test_iteration"]
    TYPECHECK_BUILD_ORDER_CASCADE["TYPECHECK-BUILD-ORDER-CASCADE"] -->|n=22, 20.0%| loop_cascade["loop_cascade"]
    LINT_RATCHET_HYGIENE_GATE["LINT-RATCHET-HYGIENE-GATE"] -->|n=15, 13.6%| loop_cascade["loop_cascade"]
    TEST_ARTIFACT_POLLUTION["TEST-ARTIFACT-POLLUTION"] -->|n=11, 10.0%| runtime_drift["runtime_drift"]
    STREAM_DISCONNECT_ABORT["STREAM-DISCONNECT-ABORT"] -->|n=10, 9.1%| premature_kill["premature_kill"]
    ENV_MISSING_DOCKER_OR_DEPS["ENV-MISSING-DOCKER-OR-DEPS"] -->|n=4, 3.6%| env_auth_budget["env_auth_budget"]
```

### 4. Proactive Lessons Learned for Agent Memory (Mimir / NornicDB)
Before an agent attempts a task, it can query historical lessons learned from past failures to avoid known monorepo traps:

```bash
reval trace lessons vitest
```

```text
=== REVAL PROACTIVE LESSONS (query: Some("vitest")) ===
#1. [VITEST-ASSERTION-ITERATION] -> [test_iteration] (n=42, 36.5% risk)
   Rule: Run narrowest single test file while iterating; isolate Postgres advisory-lock fixtures per test.
```

### 5. Execution Trace Tree and Visual Dashboard (Chase / LangSmith-Ready)
`reval trace tree` aggregates multi-agent trajectories into an interactive, self-contained HTML dashboard with search filtering, collapsible nodes, and error badges:

```bash
reval trace tree --html fixtures/tree.html
```

---

## 60-Second Quickstart

### Installation

#### Option 1: Cargo (from source)
```bash
cargo install --git https://github.com/TechTideOhio/reval
```

#### Option 2: Pre-compiled GitHub Release Binary
Download the single standalone binary directly from [Releases](https://github.com/TechTideOhio/reval/releases/latest) for your platform (Linux x86_64, macOS Apple Silicon / Intel, Windows x86_64).

### 1. Ingest and Analyze Multi-Agent Traces
Scans harness runs and execution logs, performs open coding on failures, clusters them into axial categories, and calculates Pass@1, Pass@3, and Pass^3:

```bash
reval trace analyze --root .
```

Output:
```text
=== REVAL ERROR ANALYSIS REPORT (n=459 traces, sha256=54256d4e169b...) ===
Reliability: pass=349 fail=110 | pass@1=76.0% | pass@3=98.6% | pass^3=44.0%

--- TOP 3 AXIAL CATEGORIES (YOUR FIRST 3 METRICS) ---
#1 [loop_cascade] Monorepo Build-Order & Gate Loop Cascades: 37 failures (33.6%) -> metric `loop_cascade_rate`
   Open codes: LINT-RATCHET-HYGIENE-GATE, TYPECHECK-BUILD-ORDER-CASCADE
   Fix hint:   Rebuild upstream @vsp/contracts + regenerate types before downstream typecheck; halt immediately on q.mjs exit 3.
#2 [test_iteration] Unit/Integration Assertion & Concurrency Test Iteration: 37 failures (33.6%) -> metric `test_iteration_rate`
   Open codes: VITEST-ASSERTION-ITERATION
   Fix hint:   Run narrowest single test file while iterating; isolate Postgres advisory-lock fixtures per test.
#3 [runtime_drift] Agent Binary, Self-Update Stub & Model Catalog Drift: 15 failures (13.6%) -> metric `runtime_drift_rate`
   Open codes: BINARY-SELF-UPDATE-STUB, ENV-PATH-SANDBOX, MODEL-CATALOG-MISMATCH, TEST-ARTIFACT-POLLUTION
   Fix hint:   Resolve native claude.exe (>4KB) and desktop codex.exe directly; isolate test runs via HARNESS_RUNS_DIR.

--- COMPUTED FIRST 3 METRICS ---
1. loop_cascade_rate_pct   = 15.9% (loop_guard_breach_count = 10)
2. runtime_drift_rate_pct  = 6.6%
3. premature_kill_rate_pct = 4.9%
```

### 2. Export Scrubbed Trace Corpus
Exports an anonymized JSONL corpus with high-entropy tokens and secrets redacted:

```bash
reval trace export --root . --out-dir fixtures/
```

### 3. Run Quality Gate Against Baseline
Evaluates trace reliability against ratcheted thresholds, ensuring pass rates never backslide:

```bash
reval gate \
  --corpus fixtures/traces.jsonl \
  --baseline fixtures/baseline.json \
  --write-proof fixtures/proof.json
```

Output:
```text
PASS: all 6 axial metric gates passed (proof_sha256=66dee7210f9f...)
  [PASS] loop_cascade_rate_pct      observed=15.9     <= threshold=20
  [PASS] loop_guard_breach_count    observed=10       <= threshold=10
  [PASS] runtime_drift_rate_pct     observed=6.6      <= threshold=8
  [PASS] premature_kill_rate_pct    observed=4.9      <= threshold=5
  [PASS] pass_at_1                  observed=0.7603   >= threshold=0.75
  [PASS] pass_at_3                  observed=0.9862   >= threshold=0.98
```

### 4. Scan Repository and Agent Hygiene
Verifies `.env` files are not committed, pnpm/cargo lockfiles are intact, and skill markdown references point to real files:

```bash
reval hygiene .
```

---

## Machine-Readable JSON Protocol

Every subcommand supports `--json` to output TechTide's canonical harness envelope:

```json
{
  "status": "success",
  "summary": "PASS: all 6 axial metric gates passed (proof_sha256=66dee7210f9f...)",
  "next_actions": ["commit proof bundle if corpus updated"],
  "artifacts": ["fixtures/baseline.json"],
  "data": { ... }
}
```

---

## Architecture

```text
reval/
├── Cargo.toml               # Rust 2024 edition, pure Rust dependencies
├── .cargo/config.toml       # Lints, target optimizations
├── .github/workflows/ci.yml # Multi-platform CI (Ubuntu, macOS, Windows)
├── .github/workflows/release.yml # Multi-target automated binary releases
├── reval.ps1                # PowerShell wrapper
├── fixtures/                # Calibrated traces, baselines, proofs and dashboard
└── src/
    ├── main.rs              # CLI parser (clap derive)
    ├── lib.rs               # Library root export
    ├── error.rs             # RevalError domain error enum
    ├── envelope.rs          # HarnessEnvelope JSON protocol schema
    ├── watch/               # Active Poltergeist-style loop watcher
    ├── mcp/                 # Native Model Context Protocol (MCP) server
    ├── tree/                # Hierarchical trace tree and HTML exporter
    ├── ingest/              # Corpus ingestion and regex secret redaction
    │   ├── golang.rs        # `go test -json` streaming parser
    │   ├── rust.rs          # `cargo test/clippy` JSON event parser
    │   └── harness.rs       # Multi-agent prompt/stdout/result loader
    ├── coding/              # Husain/Shankar qualitative coding
    │   ├── open.rs          # Free-text failure annotation
    │   ├── axial.rs         # Clustering into 6 axial categories
    │   ├── top3.rs          # Ranking and Top 3 metric derivation
    │   ├── causal.rs        # Directed Causal Failure Transition Graph
    │   └── lessons.rs       # Proactive agent memory query engine
    ├── eval/                # Stratified golden test evaluation
    │   ├── checks.rs        # Deterministic regex, json, and hybrid judge checks
    │   ├── rubric.rs        # Blocker veto weighting and scoring
    │   └── stratum.rs       # Stratified score combination
    ├── gate/                # Continuous quality gating
    │   ├── reliability.rs   # Pass@1, Pass@3, Pass^3 calculation
    │   ├── ratchet.rs       # baseline.json threshold validator
    │   └── proof.rs         # Canonical JSON SHA-256 signed proof generator
    └── hygiene/             # Static repo health checks
        └── checks.rs        # .env leaks, lockfile conflicts, dangling skills
```

---

## Verification & Test Gates

Run the test suite and release build locally:

```bash
# Formatting
cargo fmt --all -- --check

# Clippy lints (warnings treated as errors)
cargo clippy --all-targets -- -D warnings

# Integration and property test suite (950+ randomized property iterations)
cargo test --all-targets

# Release build
cargo build --release
```

---

## License

MIT (c) [TechTide AI](https://techtideai.io) & Alexi5000. See [LICENSE](LICENSE) for details.
