#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use reval::{
    annotate_failure, audit_repository, evaluate_gate, evaluate_traces,
    export_otel_and_openinference, ingest_workspace_traces, load_case, load_jsonl_corpus,
    query_lessons, query_lessons_from_store, run_mcp_server, run_watcher, score_suite,
    BaselineRatchet, CausalFailureGraph, EnvelopeStatus, HarnessEnvelope, PersistentGraphStore,
    RevalReport, StratumConfig, TraceOutcome, TraceRecord, TraceTree, WatchBackend, WatchOptions,
};

#[derive(Debug, Parser)]
#[command(
    name = "reval",
    version,
    about = "TideOS Eval Kit (Tool 1): Deterministic evaluation, trace error analysis & repo hygiene"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Multi-agent trace error analysis (Open Coding -> Axial Coding -> Top 3 Metrics)
    Trace {
        #[command(subcommand)]
        sub: TraceCommands,
    },

    /// Stratified golden evaluation suite runner with blocker vetoes and lockfile integrity
    Eval {
        #[command(subcommand)]
        sub: EvalCommands,
    },

    /// Fail-closed quality gate enforcing Top 3 metrics and baseline.json ratchets
    Gate {
        /// Workspace root containing `Tools/agent-harness/{runs,logs}`
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// Optional bounded Codex runs directory
        #[arg(long)]
        bounded_dir: Option<PathBuf>,

        /// Optional pre-exported JSONL trace corpus file
        #[arg(long)]
        corpus: Option<PathBuf>,

        /// Path to `baseline.json` threshold file
        #[arg(long)]
        baseline: PathBuf,

        /// Optional path to write SHA-256 signed proof bundle JSON
        #[arg(long)]
        write_proof: Option<PathBuf>,

        /// Minimum required traces
        #[arg(long, default_value_t = 100)]
        min_traces: usize,

        /// Emit canonical machine-readable JSON envelope
        #[arg(long)]
        json: bool,
    },

    /// Static repository and harness hygiene scanner (.env leaks, lockfile conflicts, dangling skills)
    Hygiene {
        /// Repository or rules path to audit
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Optional skill root directories to verify `See skill:` references against
        #[arg(long)]
        skill_root: Vec<PathBuf>,

        /// Emit canonical machine-readable JSON envelope
        #[arg(long)]
        json: bool,
    },

    /// Active file & process watcher (Poltergeist-style) that alerts on runaway loop cascades
    Watch {
        /// Workspace root containing `Tools/agent-harness/{runs,logs}`
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// Optional bounded Codex runs directory
        #[arg(long)]
        bounded_dir: Option<PathBuf>,

        /// Polling interval in milliseconds
        #[arg(long, default_value_t = 400)]
        interval_ms: u64,

        /// Debounce interval in milliseconds
        #[arg(long, default_value_t = 50)]
        debounce_ms: u64,

        /// Watcher backend engine (native, polling)
        #[arg(long, default_value = "native")]
        backend: String,

        /// Suppress audible terminal bell on alarm
        #[arg(long)]
        no_bell: bool,
    },

    /// Model Context Protocol (MCP) stdio server for Claude Code, Codex, and Cursor
    Mcp {
        /// Workspace root
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum TraceCommands {
    /// Sample production traces, annotate failures in free text (open coding), cluster into axial categories, and emit Top 3 metrics
    Analyze {
        /// Workspace root containing `Tools/agent-harness/{runs,logs}`
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// Optional bounded Codex runs directory
        #[arg(long)]
        bounded_dir: Option<PathBuf>,

        /// Optional pre-exported JSONL trace corpus file
        #[arg(long)]
        corpus: Option<PathBuf>,

        /// Minimum required production traces
        #[arg(long, default_value_t = 100)]
        min_traces: usize,

        /// Emit canonical machine-readable JSON envelope
        #[arg(long)]
        json: bool,
    },

    /// Export anonymized JSONL or OpenTelemetry trace corpus + open-coding free-text annotations
    Export {
        /// Workspace root containing `Tools/agent-harness/{runs,logs}`
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// Optional bounded Codex runs directory
        #[arg(long)]
        bounded_dir: Option<PathBuf>,

        /// Output directory for exported traces
        #[arg(long)]
        out_dir: PathBuf,

        /// Export format (jsonl, otel, all)
        #[arg(long, default_value = "jsonl")]
        format: String,

        /// Emit canonical machine-readable JSON envelope
        #[arg(long)]
        json: bool,
    },

    /// Generate Causal Failure Transition Graph (TJ Sweet / Graph-RAG style)
    Graph {
        /// Workspace root containing `Tools/agent-harness/{runs,logs}`
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// Optional bounded Codex runs directory
        #[arg(long)]
        bounded_dir: Option<PathBuf>,

        /// Optional pre-exported JSONL trace corpus file
        #[arg(long)]
        corpus: Option<PathBuf>,

        /// Output format (mermaid, dot, json)
        #[arg(long, default_value = "mermaid")]
        format: String,

        /// Optional path to persist or update dynamic Bayesian causal graph store
        #[arg(long)]
        persist: Option<PathBuf>,

        /// Optional path to load existing dynamic Bayesian causal graph store
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Proactive lessons learned retrieval for agent memory banks (Mimir / `NornicDB` style)
    Lessons {
        /// Workspace root containing `Tools/agent-harness/{runs,logs}`
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// Optional query keyword (e.g. 'vitest', 'contract', 'claude')
        #[arg(index = 1)]
        query: Option<String>,

        /// Optional path to load dynamic Bayesian causal graph store
        #[arg(long)]
        db: Option<PathBuf>,

        /// Emit canonical machine-readable JSON envelope
        #[arg(long)]
        json: bool,
    },

    /// Export execution trace tree (Harrison Chase / `LangSmith` style)
    Tree {
        /// Workspace root containing `Tools/agent-harness/{runs,logs}`
        #[arg(long, default_value = ".")]
        root: PathBuf,

        /// Optional pre-exported JSONL trace corpus file
        #[arg(long)]
        corpus: Option<PathBuf>,

        /// Optional path to export interactive HTML dashboard
        #[arg(long)]
        html: Option<PathBuf>,

        /// Emit canonical machine-readable JSON envelope
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum EvalCommands {
    /// Evaluate a run directory against a golden evaluation suite
    Suite {
        /// Directory containing case TOML files
        #[arg(long, default_value = "evals/cases")]
        cases_dir: PathBuf,

        /// Run directory containing evidence files to evaluate
        #[arg(long, default_value = ".")]
        run_dir: PathBuf,

        /// Minimum passing overall score threshold (0.0 to 1.0)
        #[arg(long, default_value_t = 0.90)]
        min_score: f64,

        /// Emit canonical machine-readable JSON envelope
        #[arg(long)]
        json: bool,
    },
}

fn default_bounded_dir() -> Option<PathBuf> {
    let home = env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)?;
    let candidate = home.join(".codex").join("private").join("bounded-runs");
    candidate.is_dir().then_some(candidate)
}

fn load_traces(
    root: &Path,
    bounded_dir: Option<&Path>,
    corpus: Option<&Path>,
) -> Result<Vec<TraceRecord>> {
    if let Some(corpus_path) = corpus {
        load_jsonl_corpus(corpus_path)
            .with_context(|| format!("failed to load corpus from {}", corpus_path.display()))
    } else {
        let resolved_bounded = bounded_dir
            .map(Path::to_path_buf)
            .or_else(default_bounded_dir);
        ingest_workspace_traces(root, resolved_bounded.as_deref())
            .with_context(|| format!("failed to ingest traces under {}", root.display()))
    }
}

fn print_human_analysis(report: &RevalReport) {
    println!(
        "=== REVAL ERROR ANALYSIS REPORT (n={} traces, sha256={}...) ===",
        report.reliability.total_traces,
        &report.corpus_sha256[..12]
    );
    println!(
        "Reliability: pass={} fail={} | pass@1={:.1}% | pass@3={:.1}% | pass^3={:.1}%",
        report.reliability.passed_traces,
        report.reliability.failed_traces,
        report.reliability.pass_at_1 * 100.0,
        report.reliability.pass_at_3 * 100.0,
        report.reliability.pass_hat_3 * 100.0,
    );
    println!("\n--- TOP 3 AXIAL CATEGORIES (YOUR FIRST 3 METRICS) ---");
    for cluster in &report.top_3_axial_categories {
        println!(
            "#{rank} [{cat}] {title}: {freq} failures ({share:.1}%) -> metric `{metric}`",
            rank = cluster.rank,
            cat = cluster.category.as_str(),
            title = cluster.title,
            freq = cluster.frequency,
            share = cluster.share_of_failures_pct,
            metric = cluster.metric_name,
        );
        println!("   Open codes: {}", cluster.open_codes.join(", "));
        println!("   Fix hint:   {}", cluster.remediation_hint);
    }
    println!("\n--- COMPUTED FIRST 3 METRICS ---");
    println!(
        "1. loop_cascade_rate_pct   = {:.1}% (loop_guard_breach_count = {})",
        report.first_three_metrics.loop_cascade_rate_pct,
        report.first_three_metrics.loop_guard_breach_count
    );
    println!(
        "2. runtime_drift_rate_pct  = {:.1}%",
        report.first_three_metrics.runtime_drift_rate_pct
    );
    println!(
        "3. premature_kill_rate_pct = {:.1}%",
        report.first_three_metrics.premature_kill_rate_pct
    );
}

fn cmd_trace_analyze(
    root: &Path,
    bounded_dir: Option<&Path>,
    corpus: Option<&Path>,
    min_traces: usize,
    json: bool,
) -> Result<ExitCode> {
    let traces = load_traces(root, bounded_dir, corpus)?;
    let report = evaluate_traces(&traces, min_traces)?;
    let summary = format!(
        "analyzed {} production traces ({} failures across {} axial categories); Top 3 metrics: loop_cascade={:.1}%, runtime_drift={:.1}%, premature_kill={:.1}%",
        report.reliability.total_traces,
        report.reliability.failed_traces,
        report.all_axial_categories.len(),
        report.first_three_metrics.loop_cascade_rate_pct,
        report.first_three_metrics.runtime_drift_rate_pct,
        report.first_three_metrics.premature_kill_rate_pct,
    );
    let next_actions: Vec<String> = report
        .top_3_axial_categories
        .iter()
        .map(|c| format!("{}: {}", c.metric_name, c.remediation_hint))
        .collect();

    if json {
        let env = HarnessEnvelope::new(
            EnvelopeStatus::Success,
            summary,
            next_actions,
            vec![],
            report,
        );
        println!("{}", serde_json::to_string_pretty(&env)?);
    } else {
        print_human_analysis(&report);
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_trace_export(
    root: &Path,
    bounded_dir: Option<&Path>,
    out_dir: &Path,
    format_opt: &str,
    json: bool,
) -> Result<ExitCode> {
    let traces = load_traces(root, bounded_dir, None)?;
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    let mut artifacts = Vec::new();
    let mut open_count = 0usize;

    let format_lower = format_opt.to_lowercase();
    let do_jsonl = format_lower == "jsonl" || format_lower == "all";
    let do_otel = format_lower == "otel" || format_lower == "all";

    if do_jsonl {
        let traces_path = out_dir.join("traces.jsonl");
        let open_coding_path = out_dir.join("open_coding.jsonl");

        let mut traces_buf = String::new();
        let mut open_buf = String::new();

        for rec in &traces {
            traces_buf.push_str(&serde_json::to_string(rec)?);
            traces_buf.push('\n');
            if let Some((coded, _)) = annotate_failure(rec) {
                open_count += 1;
                open_buf.push_str(&serde_json::to_string(&coded)?);
                open_buf.push('\n');
            }
        }

        fs::write(&traces_path, traces_buf)?;
        fs::write(&open_coding_path, open_buf)?;
        artifacts.push(traces_path.display().to_string());
        artifacts.push(open_coding_path.display().to_string());
    }

    if do_otel {
        let (otel_path, oi_path) = export_otel_and_openinference(&traces, out_dir)?;
        artifacts.push(otel_path.display().to_string());
        artifacts.push(oi_path.display().to_string());
    }

    let summary = format!(
        "exported {} scrubbed production traces across format `{}`",
        traces.len(),
        format_opt
    );
    if json {
        let env = HarnessEnvelope::new(
            EnvelopeStatus::Success,
            summary,
            vec!["import otel spans to LangSmith/Phoenix or run `reval gate`".to_string()],
            artifacts,
            serde_json::json!({ "traces": traces.len(), "open_coded_failures": open_count, "format": format_opt }),
        );
        println!("{}", serde_json::to_string_pretty(&env)?);
    } else {
        println!("{summary}");
        for a in &artifacts {
            println!("  artifact: {a}");
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_trace_graph(
    root: &Path,
    bounded_dir: Option<&Path>,
    corpus: Option<&Path>,
    format_opt: &str,
    persist: Option<&Path>,
    db: Option<&Path>,
) -> Result<ExitCode> {
    let traces = load_traces(root, bounded_dir, corpus)?;

    if let Some(persist_path) = persist {
        let mut store = if let Some(db_path) = db {
            PersistentGraphStore::load_or_create(db_path)?
        } else {
            PersistentGraphStore::load_or_create(persist_path)?
        };
        store.ingest_traces(&traces);
        store.save(persist_path)?;
        println!(
            "persisted dynamic Bayesian causal graph to {} ({} edges, {} traces observed)",
            persist_path.display(),
            store.edges.len(),
            store.total_traces_observed
        );
        match format_opt.to_lowercase().as_str() {
            "dot" => println!("{}", store.to_dot()),
            "json" => println!("{}", serde_json::to_string_pretty(&store)?),
            _ => println!("{}", store.to_mermaid()),
        }
    } else if let Some(db_path) = db {
        let store = PersistentGraphStore::load_or_create(db_path)?;
        match format_opt.to_lowercase().as_str() {
            "dot" => println!("{}", store.to_dot()),
            "json" => println!("{}", serde_json::to_string_pretty(&store)?),
            _ => println!("{}", store.to_mermaid()),
        }
    } else {
        let graph = CausalFailureGraph::from_traces(&traces);
        match format_opt.to_lowercase().as_str() {
            "dot" => println!("{}", graph.to_dot()),
            "json" => println!("{}", serde_json::to_string_pretty(&graph)?),
            _ => println!("{}", graph.to_mermaid()),
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_trace_lessons(
    root: &Path,
    query: Option<&str>,
    db: Option<&Path>,
    json: bool,
) -> Result<ExitCode> {
    let lessons = if let Some(db_path) = db {
        let store = PersistentGraphStore::load_or_create(db_path)?;
        query_lessons_from_store(&store, query)
    } else {
        let traces = load_traces(root, None, None)?;
        let graph = CausalFailureGraph::from_traces(&traces);
        query_lessons(&graph, query)
    };

    let summary = format!("retrieved {} actionable lessons learned", lessons.len());
    if json {
        let env = HarnessEnvelope::new(EnvelopeStatus::Success, summary, vec![], vec![], &lessons);
        println!("{}", serde_json::to_string_pretty(&env)?);
    } else {
        println!("=== REVAL PROACTIVE LESSONS (query: {:?}) ===", query);
        for (i, l) in lessons.iter().enumerate() {
            if let Some(bayes_p) = l.bayes_probability_pct {
                let conf = l.confidence_score.unwrap_or(0.0) * 100.0;
                println!(
                    "#{}. [{}] -> [{}] (n={}, P_bayes={:.1}%, conf={:.1}%)",
                    i + 1,
                    l.precursor,
                    l.cascade_risk,
                    l.historical_frequency,
                    bayes_p,
                    conf
                );
            } else {
                println!(
                    "#{}. [{}] -> [{}] (n={}, {:.1}% risk)",
                    i + 1,
                    l.precursor,
                    l.cascade_risk,
                    l.historical_frequency,
                    l.risk_share_pct
                );
            }
            println!("   Rule: {}", l.prescriptive_rule);
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_trace_tree(
    root: &Path,
    corpus: Option<&Path>,
    html: Option<&Path>,
    json: bool,
) -> Result<ExitCode> {
    let traces = load_traces(root, None, corpus)?;
    let tree = TraceTree::from_traces(&traces);

    if let Some(html_path) = html {
        tree.export_html(html_path)
            .with_context(|| format!("failed to export HTML to {}", html_path.display()))?;
        println!("exported trace tree dashboard to {}", html_path.display());
    }

    if json {
        let env = HarnessEnvelope::new(
            EnvelopeStatus::Success,
            format!(
                "trace tree containing {} total nodes (passed={}, failed={})",
                tree.total_nodes, tree.passed_count, tree.failed_count
            ),
            vec![],
            html.map(|p| vec![p.display().to_string()])
                .unwrap_or_default(),
            &tree,
        );
        println!("{}", serde_json::to_string_pretty(&env)?);
    } else if html.is_none() {
        println!(
            "Trace Tree: total={}, pass={}, fail={}",
            tree.total_nodes, tree.passed_count, tree.failed_count
        );
        for node in tree.root_nodes.iter().take(15) {
            let status = if node.outcome == TraceOutcome::Pass {
                "PASS"
            } else {
                "FAIL"
            };
            println!("  [{}] [{}] {}", status, node.agent, node.label);
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_watch(
    root: &Path,
    bounded_dir: Option<&Path>,
    interval_ms: u64,
    debounce_ms: u64,
    backend_str: &str,
    no_bell: bool,
) -> Result<ExitCode> {
    let backend = if backend_str.eq_ignore_ascii_case("polling") {
        WatchBackend::Polling
    } else {
        WatchBackend::Native
    };
    let opts = WatchOptions {
        root: root.to_path_buf(),
        bounded_dir: bounded_dir.map(Path::to_path_buf),
        interval: std::time::Duration::from_millis(interval_ms),
        debounce: std::time::Duration::from_millis(debounce_ms),
        backend,
        ring_bell: !no_bell,
    };
    run_watcher(&opts)?;
    Ok(ExitCode::SUCCESS)
}

fn cmd_mcp(root: &Path) -> Result<ExitCode> {
    run_mcp_server(root)?;
    Ok(ExitCode::SUCCESS)
}

fn cmd_eval_suite(
    cases_dir: &Path,
    run_dir: &Path,
    min_score: f64,
    json: bool,
) -> Result<ExitCode> {
    if !cases_dir.is_dir() {
        println!("no eval cases directory at {}", cases_dir.display());
        return Ok(ExitCode::SUCCESS);
    }
    let entries = fs::read_dir(cases_dir).map_err(|source| reval::RevalError::Io {
        path: cases_dir.to_path_buf(),
        source,
    })?;
    let mut cases = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("toml") {
            cases.push(load_case(&p)?);
        }
    }

    let mut results = Vec::new();
    for c in &cases {
        results.push(c.evaluate(run_dir));
    }

    let default_strata = vec![
        StratumConfig {
            name: "critical".to_string(),
            weight: 0.40,
        },
        StratumConfig {
            name: "core".to_string(),
            weight: 0.40,
        },
        StratumConfig {
            name: "hygiene".to_string(),
            weight: 0.20,
        },
    ];

    let summary = score_suite(results, &default_strata, min_score);
    let status = if summary.gate_passed {
        EnvelopeStatus::Success
    } else {
        EnvelopeStatus::Error
    };

    let summary_msg = format!(
        "eval suite: score={:.2} ({} cases: {} pass, {} fail, {} blocker vetoes)",
        summary.overall_score,
        summary.total_cases,
        summary.passed_cases,
        summary.failed_cases,
        summary.blocker_failures
    );

    if json {
        let env = HarnessEnvelope::new(
            status,
            summary_msg,
            summary.gate_reasons.clone(),
            vec![],
            &summary,
        );
        println!("{}", serde_json::to_string_pretty(&env)?);
    } else {
        let badge = if summary.gate_passed { "PASS" } else { "FAIL" };
        println!("[{badge}] {summary_msg}");
        for r in &summary.gate_reasons {
            println!("  reason: {r}");
        }
    }

    Ok(if summary.gate_passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn cmd_gate(
    root: &Path,
    bounded_dir: Option<&Path>,
    corpus: Option<&Path>,
    baseline: &Path,
    write_proof: Option<&Path>,
    min_traces: usize,
    json: bool,
) -> Result<ExitCode> {
    let traces = load_traces(root, bounded_dir, corpus)?;
    let report = evaluate_traces(&traces, min_traces)?;
    let baseline_raw = fs::read_to_string(baseline)
        .with_context(|| format!("failed to read baseline at {}", baseline.display()))?;
    let ratchet: BaselineRatchet = serde_json::from_str(&baseline_raw)
        .with_context(|| format!("failed to parse baseline at {}", baseline.display()))?;

    let verdict = evaluate_gate(report, &ratchet);
    let mut artifacts = vec![baseline.display().to_string()];
    if let Some(proof_path) = write_proof {
        if let Some(parent) = proof_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            proof_path,
            format!("{}\n", serde_json::to_string_pretty(&verdict)?),
        )?;
        artifacts.push(proof_path.display().to_string());
    }

    let failed_checks: Vec<String> = verdict
        .checks
        .iter()
        .filter(|c| !c.passed)
        .map(|c| {
            format!(
                "{} ({} {} {})",
                c.metric, c.observed, c.comparison, c.threshold
            )
        })
        .collect();

    let status = if verdict.passed {
        EnvelopeStatus::Success
    } else {
        EnvelopeStatus::Error
    };
    let summary = if verdict.passed {
        format!(
            "PASS: all {} axial metric gates passed (proof_sha256={}...)",
            verdict.checks.len(),
            &verdict.proof_sha256[..12]
        )
    } else {
        format!(
            "FAIL: {} of {} axial metric gates regressed: {}",
            failed_checks.len(),
            verdict.checks.len(),
            failed_checks.join(", ")
        )
    };

    if json {
        let env = HarnessEnvelope::new(
            status,
            summary,
            if verdict.passed {
                vec!["commit proof bundle if corpus updated".to_string()]
            } else {
                failed_checks
            },
            artifacts,
            &verdict,
        );
        println!("{}", serde_json::to_string_pretty(&env)?);
    } else {
        println!("{summary}");
        for check in &verdict.checks {
            let badge = if check.passed { "PASS" } else { "FAIL" };
            println!(
                "  [{badge}] {:<26} observed={:<8} {} threshold={}",
                check.metric, check.observed, check.comparison, check.threshold
            );
        }
    }

    Ok(if verdict.passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn cmd_hygiene(path: &Path, skill_root: &[PathBuf], json: bool) -> Result<ExitCode> {
    let audit = audit_repository(path, skill_root)?;
    let status = if audit.passed {
        if audit.medium_severity_count > 0 {
            EnvelopeStatus::Warning
        } else {
            EnvelopeStatus::Success
        }
    } else {
        EnvelopeStatus::Error
    };
    let summary = format!(
        "hygiene scan of {}: {} high, {} medium findings",
        audit.scanned_root, audit.high_severity_count, audit.medium_severity_count
    );
    let next_actions: Vec<String> = audit
        .findings
        .iter()
        .map(|f| format!("{}: {}", f.path, f.remediation))
        .collect();

    if json {
        let env = HarnessEnvelope::new(status, summary, next_actions, vec![], &audit);
        println!("{}", serde_json::to_string_pretty(&env)?);
    } else {
        println!("{summary}");
        for f in &audit.findings {
            println!(
                "  [{:?}] {:?} at {}: {}",
                f.severity, f.kind, f.path, f.message
            );
        }
    }

    Ok(if audit.passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Trace { sub } => match sub {
            TraceCommands::Analyze {
                root,
                bounded_dir,
                corpus,
                min_traces,
                json,
            } => cmd_trace_analyze(
                &root,
                bounded_dir.as_deref(),
                corpus.as_deref(),
                min_traces,
                json,
            ),
            TraceCommands::Export {
                root,
                bounded_dir,
                out_dir,
                format,
                json,
            } => cmd_trace_export(&root, bounded_dir.as_deref(), &out_dir, &format, json),
            TraceCommands::Graph {
                root,
                bounded_dir,
                corpus,
                format,
                persist,
                db,
            } => cmd_trace_graph(
                &root,
                bounded_dir.as_deref(),
                corpus.as_deref(),
                &format,
                persist.as_deref(),
                db.as_deref(),
            ),
            TraceCommands::Lessons {
                root,
                query,
                db,
                json,
            } => cmd_trace_lessons(&root, query.as_deref(), db.as_deref(), json),
            TraceCommands::Tree {
                root,
                corpus,
                html,
                json,
            } => cmd_trace_tree(&root, corpus.as_deref(), html.as_deref(), json),
        },
        Commands::Eval { sub } => match sub {
            EvalCommands::Suite {
                cases_dir,
                run_dir,
                min_score,
                json,
            } => cmd_eval_suite(&cases_dir, &run_dir, min_score, json),
        },
        Commands::Gate {
            root,
            bounded_dir,
            corpus,
            baseline,
            write_proof,
            min_traces,
            json,
        } => cmd_gate(
            &root,
            bounded_dir.as_deref(),
            corpus.as_deref(),
            &baseline,
            write_proof.as_deref(),
            min_traces,
            json,
        ),
        Commands::Hygiene {
            path,
            skill_root,
            json,
        } => cmd_hygiene(&path, &skill_root, json),
        Commands::Watch {
            root,
            bounded_dir,
            interval_ms,
            debounce_ms,
            backend,
            no_bell,
        } => cmd_watch(
            &root,
            bounded_dir.as_deref(),
            interval_ms,
            debounce_ms,
            &backend,
            no_bell,
        ),
        Commands::Mcp { root } => cmd_mcp(&root),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
