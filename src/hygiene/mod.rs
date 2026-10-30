pub mod checks;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::RevalError;
pub use checks::{HygieneFinding, HygieneFindingKind, Severity};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoAuditReport {
    pub scanned_root: String,
    pub passed: bool,
    pub high_severity_count: usize,
    pub medium_severity_count: usize,
    pub findings: Vec<HygieneFinding>,
}

/// Deterministically scans a repository or workspace root for hygiene violations.
pub fn audit_repository(
    repo_root: &Path,
    skill_roots: &[PathBuf],
) -> Result<RepoAuditReport, RevalError> {
    let mut findings = Vec::new();

    checks::check_env_file(repo_root, &mut findings);
    checks::check_runtime_dirs(repo_root, &mut findings);
    checks::check_conflicting_lockfiles(repo_root, &mut findings);
    checks::check_readme(repo_root, &mut findings)?;
    checks::check_ci_workflows(repo_root, &mut findings)?;

    let rules_dir = repo_root.join("rules");
    if rules_dir.is_dir() && !skill_roots.is_empty() {
        checks::check_dangling_skills(&rules_dir, skill_roots, &mut findings)?;
    }

    let high_severity_count = findings
        .iter()
        .filter(|f| f.severity == Severity::High)
        .count();
    let medium_severity_count = findings
        .iter()
        .filter(|f| f.severity == Severity::Medium)
        .count();

    Ok(RepoAuditReport {
        scanned_root: repo_root.display().to_string(),
        passed: high_severity_count == 0,
        high_severity_count,
        medium_severity_count,
        findings,
    })
}
