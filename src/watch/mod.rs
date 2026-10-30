use std::collections::HashSet;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use anyhow::Result;

use crate::coding::axial::{axial_category_for, AxialCategory};
use crate::coding::open::{annotate_failure_raw, OpenCode};
use crate::ingest::{ingest_workspace_traces, TraceOutcome};

/// Options for the Poltergeist-style active watcher.
#[derive(Debug, Clone)]
pub struct WatchOptions {
    pub root: PathBuf,
    pub interval: Duration,
    pub ring_bell: bool,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            interval: Duration::from_millis(800),
            ring_bell: true,
        }
    }
}

/// Runs a Poltergeist-style active file and process watcher that alerts on loop cascades.
pub fn run_watcher(opts: &WatchOptions) -> Result<()> {
    println!(
        "🔭 reval watcher active (root: {}, poll: {:?})",
        opts.root.display(),
        opts.interval
    );
    println!("Watching for agent runs and monitoring loop cascades... (Ctrl+C to stop)\n");

    let mut seen_trace_ids = HashSet::new();

    // Initial warm-up: collect existing traces so we only alert on newly observed ones
    if let Ok(initial_traces) = ingest_workspace_traces(&opts.root, None) {
        for t in initial_traces {
            seen_trace_ids.insert(t.id);
        }
    }

    loop {
        thread::sleep(opts.interval);

        let Ok(current_traces) = ingest_workspace_traces(&opts.root, None) else {
            continue;
        };

        for trace in current_traces {
            if seen_trace_ids.insert(trace.id.clone()) {
                let status_icon = if trace.outcome == TraceOutcome::Pass {
                    "✅"
                } else {
                    "❌"
                };

                println!(
                    "{} [{}] {} (agent: {})",
                    status_icon, trace.id, trace.command_or_summary, trace.agent_or_runner
                );

                if trace.outcome != TraceOutcome::Pass {
                    let (open_code, _) = annotate_failure_raw(&trace)
                        .unwrap_or((OpenCode::UnclassifiedFailure, String::new()));
                    let axial = axial_category_for(open_code);

                    if axial == AxialCategory::LoopCascade {
                        if opts.ring_bell {
                            // Ring terminal bell
                            print!("\x07");
                        }
                        println!(
                            "\n🚨 ==================== [ALARM: RUNAWAY LOOP CASCADE] ===================="
                        );
                        println!("Trace: {}", trace.id);
                        println!("Open code: {}", open_code.as_str());
                        println!("Axial category: {}", axial.as_str());
                        println!("Diagnostic: {}", trace.diagnostic_excerpt.trim());
                        println!("Remediation: {}", axial.remediation_hint());
                        println!(
                            "=========================================================================\n"
                        );
                    }
                }
            }
        }
    }
}
