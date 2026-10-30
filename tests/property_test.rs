use reval::{
    annotate_failure, cluster_axial_categories, compute_corpus_sha256, compute_reliability,
    TraceOutcome, TraceRecord, TraceSource,
};

/// Deterministic `SplitMix64` PRNG for hermetic, zero-FFI property-based testing across 500 cases.
struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn range_usize(&mut self, min: usize, max_exclusive: usize) -> usize {
        if max_exclusive <= min {
            return min;
        }
        let span = (max_exclusive - min) as u64;
        min + usize::try_from(self.next_u64() % span).unwrap_or(0)
    }
}

#[test]
fn property_reliability_metrics_obey_monotonic_ordering() {
    let mut rng = SplitMix64::new(0xA81A_1E7A_2026_1030);
    for _ in 0..500 {
        let total = rng.range_usize(1, 500);
        let passed = rng.range_usize(0, total + 1);
        let (p1, p_at_3, p_hat_3) = compute_reliability(total, passed, 3);
        assert!((0.0..=1.0).contains(&p1));
        assert!((0.0..=1.0).contains(&p_at_3));
        assert!((0.0..=1.0).contains(&p_hat_3));
        // pass^3 <= pass@1 <= pass@3 for all p in [0, 1] (within 1e-4 rounding tolerance)
        assert!(p_hat_3 <= p1 + 0.0002, "p^3={p_hat_3} > p@1={p1}");
        assert!(p1 <= p_at_3 + 0.0002, "p@1={p1} > p@3={p_at_3}");
    }
}

#[test]
fn property_axial_cluster_frequencies_conserve_total_open_coded_failures() {
    let mut rng = SplitMix64::new(0x50F7_FAC7_0459_2026);
    for _ in 0..250 {
        let n_loop = rng.range_usize(0, 30);
        let n_drift = rng.range_usize(0, 30);
        let n_kill = rng.range_usize(0, 30);
        let n_pass = rng.range_usize(0, 50);

        let mut records = Vec::with_capacity(n_loop + n_drift + n_kill + n_pass);
        for i in 0..n_loop {
            records.push(TraceRecord {
                id: format!("loop-{i}"),
                source: TraceSource::LoopEvent,
                agent_or_runner: "q.mjs".to_string(),
                outcome: TraceOutcome::Fail,
                dry_run: false,
                exit_code: Some(3),
                duration_ms: None,
                bounded_status: None,
                loop_streak: Some(3),
                command_or_summary: "pnpm typecheck".to_string(),
                diagnostic_excerpt: "typecheck failed".to_string(),
            });
        }
        for i in 0..n_drift {
            records.push(TraceRecord {
                id: format!("drift-{i}"),
                source: TraceSource::HarnessRun,
                agent_or_runner: "claude".to_string(),
                outcome: TraceOutcome::Fail,
                dry_run: false,
                exit_code: Some(1),
                duration_ms: Some(35),
                bounded_status: None,
                loop_streak: None,
                command_or_summary: "Claude failed".to_string(),
                diagnostic_excerpt: "not compatible with the version of Windows".to_string(),
            });
        }
        for i in 0..n_kill {
            records.push(TraceRecord {
                id: format!("kill-{i}"),
                source: TraceSource::BoundedCodex,
                agent_or_runner: "codex".to_string(),
                outcome: TraceOutcome::Fail,
                dry_run: false,
                exit_code: Some(1),
                duration_ms: Some(15000),
                bounded_status: Some("agent-error".to_string()),
                loop_streak: None,
                command_or_summary: "bounded".to_string(),
                diagnostic_excerpt: "Reconnecting... 2/5 (stream disconnected)".to_string(),
            });
        }
        for i in 0..n_pass {
            records.push(TraceRecord {
                id: format!("pass-{i}"),
                source: TraceSource::QuietLog,
                agent_or_runner: "q.mjs".to_string(),
                outcome: TraceOutcome::Pass,
                dry_run: false,
                exit_code: Some(0),
                duration_ms: Some(100),
                bounded_status: None,
                loop_streak: None,
                command_or_summary: "pnpm test".to_string(),
                diagnostic_excerpt: "OK".to_string(),
            });
        }

        let open_coded: Vec<_> = records.iter().filter_map(annotate_failure).collect();
        let clusters = cluster_axial_categories(&open_coded);
        let sum_freq: usize = clusters.iter().map(|c| c.frequency).sum();
        assert_eq!(open_coded.len(), n_loop + n_drift + n_kill);
        assert_eq!(sum_freq, open_coded.len());
    }
}

#[test]
fn property_corpus_sha256_is_order_independent() {
    let mut rng = SplitMix64::new(0xCAFE_BABE_1234_5678);
    for case_idx in 0..200 {
        let len = rng.range_usize(2, 40);
        let forward: Vec<TraceRecord> = (0..len)
            .map(|idx| TraceRecord {
                id: format!("trace-{case_idx}-{idx}-{}", rng.next_u64() & 0xFFFF),
                source: TraceSource::QuietLog,
                agent_or_runner: "q.mjs".to_string(),
                outcome: if idx % 2 == 0 {
                    TraceOutcome::Pass
                } else {
                    TraceOutcome::Fail
                },
                dry_run: false,
                exit_code: Some(0),
                duration_ms: None,
                bounded_status: None,
                loop_streak: None,
                command_or_summary: "cmd".to_string(),
                diagnostic_excerpt: "diag".to_string(),
            })
            .collect();
        let mut reversed = forward.clone();
        reversed.reverse();
        assert_eq!(
            compute_corpus_sha256(&forward),
            compute_corpus_sha256(&reversed)
        );
    }
}
