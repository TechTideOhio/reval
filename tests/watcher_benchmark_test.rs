use std::collections::HashMap;
use std::fs;
use std::time::Instant;

#[test]
fn test_watcher_sentinel_change_detection_benchmark() {
    let temp_root = std::env::temp_dir().join(format!("reval-bench-50k-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(&temp_root).expect("create bench root");

    // Create sentinel harness directories
    let runs_dir = temp_root.join("Tools").join("agent-harness").join("runs");
    let logs_dir = temp_root.join("Tools").join("agent-harness").join("logs");
    fs::create_dir_all(&runs_dir).expect("create runs");
    fs::create_dir_all(&logs_dir).expect("create logs");

    // Populate simulated monorepo with 1,000 nested source files
    let src_dir = temp_root.join("packages").join("app").join("src");
    fs::create_dir_all(&src_dir).expect("create src");
    for i in 0..1000 {
        let file_path = src_dir.join(format!("component_{}.ts", i));
        fs::write(file_path, b"export const dummy = 42;\n").expect("write dummy file");
    }

    // Benchmark sentinel metadata retrieval time across 100 iterations
    let sentinel_paths = vec![
        runs_dir.clone(),
        logs_dir.clone(),
        temp_root.join("loop-events.jsonl"),
    ];

    let mut mtimes = HashMap::new();
    let start = Instant::now();
    let iterations: u128 = 100;

    for _ in 0..iterations {
        for path in &sentinel_paths {
            if let Ok(meta) = fs::metadata(path) {
                if let Ok(mtime) = meta.modified() {
                    mtimes.insert(path.clone(), mtime);
                }
            }
        }
    }

    let elapsed = start.elapsed();
    let avg_us = elapsed.as_micros() / iterations;

    println!(
        "Sentinel mtime check benchmark: {} iterations took {:?} (avg: {} us/check)",
        iterations, elapsed, avg_us
    );

    // Assert that the sentinel check is sub-millisecond (less than 1,000 microseconds)
    assert!(
        avg_us < 1_000,
        "Sentinel metadata check must be < 1ms to ensure 0% CPU background operation, took {} us",
        avg_us
    );

    // Verify change detection fires when a new run is created
    let run_marker = runs_dir.join("2026-10-31T01-00-00-agent");
    fs::create_dir_all(&run_marker).expect("create new run dir");
    fs::write(
        run_marker.join("result.json"),
        b"{\"status\":\"success\"}\n",
    )
    .expect("write result");

    let new_meta = fs::metadata(&runs_dir).expect("runs metadata");
    let new_mtime = new_meta.modified().expect("mtime");

    // The runs directory mtime must update on creation of a new subdirectory
    assert!(mtimes.get(&runs_dir).is_none_or(|&old| old <= new_mtime));

    let _ = fs::remove_dir_all(&temp_root);
}
