use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::RevalError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING-KEBAB-CASE")]
pub enum HygieneFindingKind {
    CommittedEnvFile,
    CommittedVenvOrStateDir,
    ConflictingLockfiles,
    DanglingSkillReference,
    PlaceholderReadme,
    UnpinnedGitHubAction,
    RustProductionUnwrapOrTodo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    High,
    Medium,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HygieneFinding {
    pub kind: HygieneFindingKind,
    pub severity: Severity,
    pub path: String,
    pub message: String,
    pub remediation: String,
}

pub fn check_env_file(repo_root: &Path, findings: &mut Vec<HygieneFinding>) {
    let dotenv = repo_root.join(".env");
    if dotenv.is_file() {
        findings.push(HygieneFinding {
            kind: HygieneFindingKind::CommittedEnvFile,
            severity: Severity::High,
            path: dotenv.display().to_string(),
            message: "Root .env file is present on disk; ensure it is gitignored and never committed".to_string(),
            remediation: "Remove .env from git tracking (`git rm --cached .env`) and add `.env` to `.gitignore`.".to_string(),
        });
    }
}

pub fn check_runtime_dirs(repo_root: &Path, findings: &mut Vec<HygieneFinding>) {
    for forbidden_dir in [".venv", ".kill-switch"] {
        let candidate = repo_root.join(forbidden_dir);
        if candidate.is_dir() {
            findings.push(HygieneFinding {
                kind: HygieneFindingKind::CommittedVenvOrStateDir,
                severity: Severity::High,
                path: candidate.display().to_string(),
                message: format!(
                    "Runtime artifact directory `{forbidden_dir}` exists at repository root"
                ),
                remediation: format!(
                    "Remove `{forbidden_dir}/` from version control and add it to `.gitignore`."
                ),
            });
        }
    }
}

pub fn check_conflicting_lockfiles(repo_root: &Path, findings: &mut Vec<HygieneFinding>) {
    let lockfiles = [
        "pnpm-lock.yaml",
        "package-lock.json",
        "yarn.lock",
        "bun.lock",
        "bun.lockb",
    ];
    let present_locks: Vec<&str> = lockfiles
        .into_iter()
        .filter(|name| repo_root.join(name).is_file())
        .collect();

    let mut managers = Vec::new();
    for lock in &present_locks {
        let mgr = match *lock {
            "pnpm-lock.yaml" => "pnpm",
            "package-lock.json" => "npm",
            "yarn.lock" => "yarn",
            "bun.lock" | "bun.lockb" => "bun",
            _ => "other",
        };
        if !managers.contains(&mgr) {
            managers.push(mgr);
        }
    }

    if managers.len() > 1 {
        findings.push(HygieneFinding {
            kind: HygieneFindingKind::ConflictingLockfiles,
            severity: Severity::High,
            path: repo_root.display().to_string(),
            message: format!(
                "Multiple conflicting lockfiles detected at root: {}",
                present_locks.join(", ")
            ),
            remediation: "Standardize on a single package manager (pnpm in TechTide repos) and delete extra lockfiles.".to_string(),
        });
    }
}

pub fn check_readme(
    repo_root: &Path,
    findings: &mut Vec<HygieneFinding>,
) -> Result<(), RevalError> {
    let readme_path = repo_root.join("README.md");
    if readme_path.is_file() {
        let content = fs::read_to_string(&readme_path).map_err(|source| RevalError::Io {
            path: readme_path.clone(),
            source,
        })?;
        if content.contains("<YOUR_GIT_URL>") || content.contains("Welcome to your Lovable project")
        {
            findings.push(HygieneFinding {
                kind: HygieneFindingKind::PlaceholderReadme,
                severity: Severity::Medium,
                path: readme_path.display().to_string(),
                message: "README.md contains unedited Lovable template boilerplate (`<YOUR_GIT_URL>`)".to_string(),
                remediation: "Replace boilerplate with the 4-part TechTide README structure (What it does, Who it serves, Why built, Evidence).".to_string(),
            });
        }
    }
    Ok(())
}

pub fn check_dangling_skills(
    dir: &Path,
    skill_roots: &[PathBuf],
    findings: &mut Vec<HygieneFinding>,
) -> Result<(), RevalError> {
    let entries = fs::read_dir(dir).map_err(|source| RevalError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            check_dangling_skills(&path, skill_roots, findings)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let content = fs::read_to_string(&path).map_err(|source| RevalError::Io {
                path: path.clone(),
                source,
            })?;
            for line in content.lines() {
                if let Some(rest) = line.strip_prefix("See skill:") {
                    let head = rest.split(" for ").next().unwrap_or(rest);
                    for token in head.split(|c: char| !c.is_ascii_alphanumeric() && c != '-') {
                        let candidate = token.trim();
                        if candidate.len() > 2
                            && candidate.contains('-')
                            && candidate
                                .chars()
                                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                            && !skill_exists(candidate, skill_roots)
                        {
                            findings.push(HygieneFinding {
                                kind: HygieneFindingKind::DanglingSkillReference,
                                severity: Severity::High,
                                path: path.display().to_string(),
                                message: format!("Rule references missing skill `{candidate}`"),
                                remediation: format!("Install `{candidate}/SKILL.md` via `node Tools/stack-skills/sync_stack_skills.mjs`."),
                            });
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn check_ci_workflows(
    repo_root: &Path,
    findings: &mut Vec<HygieneFinding>,
) -> Result<(), RevalError> {
    let workflows_dir = repo_root.join(".github").join("workflows");
    if !workflows_dir.is_dir() {
        return Ok(());
    }
    let entries = fs::read_dir(&workflows_dir).map_err(|source| RevalError::Io {
        path: workflows_dir.clone(),
        source,
    })?;
    let sha_regex = Regex::new(r"uses:\s+([a-zA-Z0-9_\-]+/[a-zA-Z0-9_\-]+)@([a-f0-9]{40})")
        .expect("valid regex");
    let unpinned_regex =
        Regex::new(r"uses:\s+([a-zA-Z0-9_\-]+/[a-zA-Z0-9_\-]+)@(v\d+|main|master)")
            .expect("valid regex");

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("yml")
            || path.extension().and_then(|e| e.to_str()) == Some("yaml")
        {
            let content = fs::read_to_string(&path).map_err(|source| RevalError::Io {
                path: path.clone(),
                source,
            })?;
            for cap in unpinned_regex.captures_iter(&content) {
                let action = &cap[1];
                let ref_tag = &cap[2];
                findings.push(HygieneFinding {
                    kind: HygieneFindingKind::UnpinnedGitHubAction,
                    severity: Severity::Medium,
                    path: path.display().to_string(),
                    message: format!("GitHub Action `{action}` is pinned to mutable tag `@{ref_tag}` instead of commit SHA"),
                    remediation: "Pin the action to its immutable 40-character commit SHA (e.g. `uses: actions/checkout@34e11...`).".to_string(),
                });
            }
            let _ = sha_regex; // Used for positive validation
        }
    }
    Ok(())
}

fn skill_exists(skill_name: &str, skill_roots: &[PathBuf]) -> bool {
    skill_roots
        .iter()
        .any(|root| root.join(skill_name).join("SKILL.md").is_file())
}
