use std::fs;
use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    #[default]
    Graded,
    Blocker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Check {
    FileExists {
        path: String,
    },
    Contains {
        path: String,
        needle: String,
    },
    NotContains {
        path: String,
        needle: String,
    },
    RegexMatch {
        path: String,
        pattern: String,
    },
    NoSecretLeak {
        path: String,
    },
    JsonFieldEquals {
        path: String,
        field: String,
        expected_json: String,
    },
    MaxLineCount {
        path: String,
        max_lines: usize,
    },
    /// Hybrid evaluation: evaluates semantic criteria with code heuristics,
    /// and optionally dispatches to an external judge script if provided.
    HybridJudge {
        path: String,
        criteria: String,
        command: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckSpec {
    pub id: String,
    pub check: Check,
    #[serde(default = "default_weight")]
    pub weight: f64,
    #[serde(default)]
    pub severity: Severity,
}

fn default_weight() -> f64 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckResult {
    pub id: String,
    pub passed: bool,
    pub weight: f64,
    pub severity: Severity,
    pub detail: String,
}

impl CheckSpec {
    #[must_use]
    pub fn evaluate(&self, base_dir: &Path) -> CheckResult {
        let (passed, detail) = match &self.check {
            Check::FileExists { path } => {
                let target = base_dir.join(path);
                if target.exists() {
                    (true, format!("file exists at {path}"))
                } else {
                    (false, format!("missing file at {path}"))
                }
            }
            Check::Contains { path, needle } => {
                let target = base_dir.join(path);
                match fs::read_to_string(&target) {
                    Ok(content) => {
                        if content.contains(needle) {
                            (true, format!("found `{needle}` in {path}"))
                        } else {
                            (false, format!("did not find `{needle}` in {path}"))
                        }
                    }
                    Err(err) => (false, format!("cannot read {path}: {err}")),
                }
            }
            Check::NotContains { path, needle } => {
                let target = base_dir.join(path);
                match fs::read_to_string(&target) {
                    Ok(content) => {
                        if content.contains(needle) {
                            (
                                false,
                                format!("prohibited string `{needle}` found in {path}"),
                            )
                        } else {
                            (true, format!("clean: no `{needle}` in {path}"))
                        }
                    }
                    Err(_) => (true, format!("file absent: {path}")),
                }
            }
            Check::RegexMatch { path, pattern } => {
                let target = base_dir.join(path);
                match (Regex::new(pattern), fs::read_to_string(&target)) {
                    (Ok(re), Ok(content)) => {
                        if re.is_match(&content) {
                            (true, format!("regex matched in {path}"))
                        } else {
                            (false, format!("regex did not match in {path}"))
                        }
                    }
                    (Err(err), _) => (false, format!("invalid regex `{pattern}`: {err}")),
                    (_, Err(err)) => (false, format!("cannot read {path}: {err}")),
                }
            }
            Check::NoSecretLeak { path } => {
                let target = base_dir.join(path);
                match fs::read_to_string(&target) {
                    Ok(content) => {
                        let lower = content.to_lowercase();
                        let found_prefix = ["gho_", "ghp_", "sk-ant-", "sk-proj-", "bearer "]
                            .into_iter()
                            .find(|p| lower.contains(p));
                        if let Some(prefix) = found_prefix {
                            (
                                false,
                                format!("credential pattern `{prefix}` detected in {path}"),
                            )
                        } else {
                            (true, format!("no credential patterns detected in {path}"))
                        }
                    }
                    Err(_) => (true, format!("file absent: {path}")),
                }
            }
            Check::JsonFieldEquals {
                path,
                field,
                expected_json,
            } => {
                let target = base_dir.join(path);
                match fs::read_to_string(&target) {
                    Ok(content) => match (
                        serde_json::from_str::<serde_json::Value>(&content),
                        serde_json::from_str::<serde_json::Value>(expected_json),
                    ) {
                        (Ok(val), Ok(exp)) => {
                            let actual = val.get(field);
                            if actual == Some(&exp) {
                                (true, format!("field `{field}` matches expected JSON"))
                            } else {
                                (
                                    false,
                                    format!(
                                        "field `{field}` mismatch: actual={:?}, expected={:?}",
                                        actual, exp
                                    ),
                                )
                            }
                        }
                        _ => (false, "failed to parse JSON".to_string()),
                    },
                    Err(err) => (false, format!("cannot read {path}: {err}")),
                }
            }
            Check::MaxLineCount { path, max_lines } => {
                let target = base_dir.join(path);
                match fs::read_to_string(&target) {
                    Ok(content) => {
                        let lines = content.lines().count();
                        if lines <= *max_lines {
                            (true, format!("{path} has {lines} lines (<= {max_lines})"))
                        } else {
                            (
                                false,
                                format!("{path} has {lines} lines (exceeds {max_lines})"),
                            )
                        }
                    }
                    Err(err) => (false, format!("cannot read {path}: {err}")),
                }
            }
            Check::HybridJudge {
                path,
                criteria,
                command,
            } => {
                let target = base_dir.join(path);
                match fs::read_to_string(&target) {
                    Ok(content) => {
                        if let Some(cmd) = command {
                            let mut proc = if cfg!(windows) {
                                let mut c = std::process::Command::new("powershell");
                                c.args(["-NoProfile", "-Command", cmd]);
                                c
                            } else {
                                let mut c = std::process::Command::new("sh");
                                c.args(["-c", cmd]);
                                c
                            };
                            proc.env("REVAL_EVAL_FILE", target.to_string_lossy().to_string());
                            proc.env("REVAL_EVAL_CRITERIA", criteria);

                            match proc.output() {
                                Ok(output) => {
                                    if output.status.success() {
                                        let stdout = String::from_utf8_lossy(&output.stdout);
                                        let passed = stdout.contains("\"passed\": true")
                                            || stdout.contains("PASS")
                                            || stdout.trim() == "true";
                                        (
                                            passed,
                                            format!(
                                                "hybrid judge script returned {}: {}",
                                                output.status,
                                                stdout.trim()
                                            ),
                                        )
                                    } else {
                                        (
                                            false,
                                            format!(
                                                "hybrid judge command failed with exit code {:?}",
                                                output.status.code()
                                            ),
                                        )
                                    }
                                }
                                Err(err) => (
                                    false,
                                    format!("failed to spawn hybrid judge command: {err}"),
                                ),
                            }
                        } else {
                            // Fallback heuristic: all semantic search terms in criteria must appear in content
                            let lower = content.to_lowercase();
                            let terms: Vec<&str> = criteria
                                .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
                                .filter(|w| w.len() > 3)
                                .collect();
                            let missing: Vec<&str> = terms
                                .into_iter()
                                .filter(|w| !lower.contains(&w.to_lowercase()))
                                .collect();

                            if missing.is_empty() {
                                (true, format!("semantic criteria satisfied in {path}"))
                            } else {
                                (
                                    false,
                                    format!("semantic criteria missing tokens: {:?}", missing),
                                )
                            }
                        }
                    }
                    Err(err) => (false, format!("cannot read {path}: {err}")),
                }
            }
        };

        CheckResult {
            id: self.id.clone(),
            passed,
            weight: self.weight,
            severity: self.severity,
            detail,
        }
    }
}
