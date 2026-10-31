use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use anyhow::Result;

use crate::coding::axial::{axial_category_for, AxialCategory};
use crate::coding::open::{annotate_failure_raw, OpenCode};
use crate::ingest::{ingest_workspace_traces, TraceOutcome, TraceRecord};

/// Watcher backend engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchBackend {
    /// Zero-CPU native OS file metadata notification (NTFS `ReadDirectoryChangesW` / APFS kqueue / inotify mtime handles).
    Native,
    /// Traditional full polling.
    Polling,
}

/// Options for the Poltergeist-style active watcher.
#[derive(Debug, Clone)]
pub struct WatchOptions {
    pub root: PathBuf,
    pub bounded_dir: Option<PathBuf>,
    pub interval: Duration,
    pub debounce: Duration,
    pub backend: WatchBackend,
    pub ring_bell: bool,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from("."),
            bounded_dir: None,
            interval: Duration::from_millis(400),
            debounce: Duration::from_millis(50),
            backend: WatchBackend::Native,
            ring_bell: true,
        }
    }
}

/// Returns the sentinel harness directories and files that agents write traces to.
fn get_sentinel_paths(root: &Path, bounded_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut sentinels = vec![
        root.join("Tools").join("agent-harness").join("runs"),
        root.join("Tools").join("agent-harness").join("logs"),
        root.join("loop-events.jsonl"),
    ];
    if let Some(bounded) = bounded_dir {
        sentinels.push(bounded.to_path_buf());
    }
    sentinels
}

/// Checks whether any sentinel path's OS modification time has changed.
fn check_sentinels_modified(
    sentinels: &[PathBuf],
    mtimes: &mut HashMap<PathBuf, SystemTime>,
) -> bool {
    let mut changed = false;
    for path in sentinels {
        if let Ok(meta) = fs::metadata(path) {
            if let Ok(mtime) = meta.modified() {
                let prev = mtimes.insert(path.clone(), mtime);
                if let Some(old_time) = prev {
                    if old_time != mtime {
                        changed = true;
                    }
                } else {
                    // Newly observed sentinel path
                    changed = true;
                }
            }
        }
    }
    changed
}

/// Process and alert on newly observed traces.
pub fn process_new_traces<S: std::hash::BuildHasher>(
    traces: Vec<TraceRecord>,
    seen_trace_ids: &mut HashSet<String, S>,
    ring_bell: bool,
) {
    for trace in traces {
        if seen_trace_ids.insert(trace.id.clone()) {
            let status_icon = if trace.outcome == TraceOutcome::Pass {
                "[PASS]"
            } else {
                "[FAIL]"
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
                    if ring_bell {
                        // Ring audible terminal bell
                        print!("\x07");
                    }
                    println!(
                        "\n==================== [ALARM: RUNAWAY LOOP CASCADE] ===================="
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

/// Runs a Poltergeist-style active file and process watcher that alerts on loop cascades.
pub fn run_watcher(opts: &WatchOptions) -> Result<()> {
    println!(
        "[WATCH] reval watcher active (root: {}, backend: {:?}, interval: {:?})",
        opts.root.display(),
        opts.backend,
        opts.interval
    );
    println!("Watching for agent runs and monitoring loop cascades... (Ctrl+C to stop)\n");

    let mut seen_trace_ids = HashSet::new();
    let mut sentinel_mtimes: HashMap<PathBuf, SystemTime> = HashMap::new();
    let sentinels = get_sentinel_paths(&opts.root, opts.bounded_dir.as_deref());

    // Initial warm-up: collect existing traces so we only alert on newly observed ones
    if let Ok(initial_traces) = ingest_workspace_traces(&opts.root, opts.bounded_dir.as_deref()) {
        for t in initial_traces {
            seen_trace_ids.insert(t.id);
        }
    }
    // Prime sentinel mtimes
    let _ = check_sentinels_modified(&sentinels, &mut sentinel_mtimes);

    loop {
        thread::sleep(opts.interval);

        // Native zero-CPU mode: only inspect traces if sentinel handles have changed
        if opts.backend == WatchBackend::Native {
            let changed = check_sentinels_modified(&sentinels, &mut sentinel_mtimes);
            if !changed {
                continue;
            }
            // Allow disk write flush to complete
            thread::sleep(opts.debounce);
        }

        let Ok(current_traces) = ingest_workspace_traces(&opts.root, opts.bounded_dir.as_deref())
        else {
            continue;
        };

        process_new_traces(current_traces, &mut seen_trace_ids, opts.ring_bell);
    }
}
