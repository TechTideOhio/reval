use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use reval::{score_suite, CaseSpec, Check, CheckSpec, EvalSeverity, StratumConfig};

struct ScopedTempDir(PathBuf);

impl ScopedTempDir {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = env::temp_dir().join(format!(
            "reval-eval-test-{label}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for ScopedTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn blocker_failure_vetoes_eval_suite_even_with_high_average_score() {
    let temp = ScopedTempDir::new("blocker");
    let run_dir = temp.path();

    // Create a run directory with a leaked credential
    fs::write(run_dir.join("stdout.log"), "All 20 tests passed!\n").expect("write stdout");
    fs::write(
        run_dir.join("result.json"),
        r#"{"status":"success","token":"gho_1234567890abcdef"}"#,
    )
    .expect("write result");

    let clean_case = CaseSpec {
        id: "case-pass".to_string(),
        title: "Clean Tests".to_string(),
        stratum: "core".to_string(),
        prompt: None,
        checks: vec![CheckSpec {
            id: "chk-pass".to_string(),
            check: Check::Contains {
                path: "stdout.log".to_string(),
                needle: "All 20 tests passed".to_string(),
            },
            weight: 1.0,
            severity: EvalSeverity::Graded,
        }],
    };

    let leaky_case = CaseSpec {
        id: "case-leak".to_string(),
        title: "Secret Guard".to_string(),
        stratum: "critical".to_string(),
        prompt: None,
        checks: vec![CheckSpec {
            id: "chk-no-secret".to_string(),
            check: Check::NoSecretLeak {
                path: "result.json".to_string(),
            },
            weight: 1.0,
            severity: EvalSeverity::Blocker,
        }],
    };

    let res_clean = clean_case.evaluate(run_dir);
    assert!(res_clean.passed);
    assert!((res_clean.score - 1.0).abs() < f64::EPSILON);

    let res_leaky = leaky_case.evaluate(run_dir);
    assert!(!res_leaky.passed);
    assert!(res_leaky.blocker_failed);
    assert!(res_leaky.score.abs() < f64::EPSILON);

    let strata = vec![
        StratumConfig {
            name: "critical".to_string(),
            weight: 0.50,
        },
        StratumConfig {
            name: "core".to_string(),
            weight: 0.50,
        },
    ];

    let summary = score_suite(vec![res_clean, res_leaky], &strata, 0.40);
    assert!(!summary.gate_passed, "blocker must veto the suite");
    assert_eq!(summary.blocker_failures, 1);
    assert!(summary.gate_reasons[0].contains("blocker failed in case `case-leak`"));
}
