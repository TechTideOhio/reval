use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::RevalError;
use crate::eval::checks::{CheckResult, CheckSpec, Severity};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseSpec {
    pub id: String,
    pub title: String,
    pub stratum: String,
    pub prompt: Option<String>,
    #[serde(default)]
    pub checks: Vec<CheckSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseResult {
    pub id: String,
    pub title: String,
    pub stratum: String,
    pub passed: bool,
    pub score: f64,
    pub blocker_failed: bool,
    pub checks: Vec<CheckResult>,
}

impl CaseSpec {
    #[must_use]
    pub fn evaluate(&self, run_dir: &Path) -> CaseResult {
        let mut total_weight = 0.0;
        let mut earned_weight = 0.0;
        let mut blocker_failed = false;
        let mut results = Vec::new();

        for check in &self.checks {
            let res = check.evaluate(run_dir);
            total_weight += res.weight;
            if res.passed {
                earned_weight += res.weight;
            } else if res.severity == Severity::Blocker {
                blocker_failed = true;
            }
            results.push(res);
        }

        let raw_score = if total_weight > 0.0 {
            earned_weight / total_weight
        } else {
            1.0
        };

        let score = if blocker_failed { 0.0 } else { raw_score };
        let passed = !blocker_failed && score >= 0.999;

        CaseResult {
            id: self.id.clone(),
            title: self.title.clone(),
            stratum: self.stratum.clone(),
            passed,
            score: (score * 1000.0).round() / 1000.0,
            blocker_failed,
            checks: results,
        }
    }

    #[must_use]
    pub fn compute_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.id.as_bytes());
        hasher.update(b"|");
        hasher.update(self.stratum.as_bytes());
        hasher.update(b"|");
        if let Some(p) = &self.prompt {
            hasher.update(p.as_bytes());
        }
        for c in &self.checks {
            hasher.update(c.id.as_bytes());
            hasher.update(format!(":{:?}:{}", c.severity, c.weight).as_bytes());
        }
        format!("{:x}", hasher.finalize())
    }
}

/// Computes a dataset digest across all cases in a suite.
pub fn compute_suite_digest(cases: &[CaseSpec]) -> String {
    let mut sorted = cases.to_vec();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let mut hasher = Sha256::new();
    for c in &sorted {
        hasher.update(c.compute_digest().as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

/// Loads an eval case TOML file from disk.
pub fn load_case(path: &Path) -> Result<CaseSpec, RevalError> {
    let content = fs::read_to_string(path).map_err(|source| RevalError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&content).map_err(|source| RevalError::Toml {
        path: path.to_path_buf(),
        source,
    })
}
