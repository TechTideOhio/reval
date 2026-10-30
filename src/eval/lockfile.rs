use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::RevalError;
use crate::eval::rubric::CaseSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevalLockfile {
    pub schema_version: u32,
    pub dataset_digest: String,
    pub case_digests: BTreeMap<String, String>,
}

impl RevalLockfile {
    #[must_use]
    pub fn generate(dataset_digest: String, cases: &[CaseSpec]) -> Self {
        let mut digests = BTreeMap::new();
        for c in cases {
            digests.insert(c.id.clone(), c.compute_digest());
        }
        Self {
            schema_version: 1,
            dataset_digest,
            case_digests: digests,
        }
    }

    pub fn verify(
        &self,
        expected_dataset_digest: &str,
        cases: &[CaseSpec],
    ) -> Result<(), RevalError> {
        if self.dataset_digest != expected_dataset_digest {
            return Err(RevalError::LockfileMismatch {
                message: format!(
                    "dataset digest drift: locked={}, current={}",
                    self.dataset_digest, expected_dataset_digest
                ),
            });
        }
        for c in cases {
            if let Some(locked) = self.case_digests.get(&c.id) {
                let current = c.compute_digest();
                if locked != &current {
                    return Err(RevalError::LockfileMismatch {
                        message: format!(
                            "case `{}` content drift: locked={}, current={}",
                            c.id, locked, current
                        ),
                    });
                }
            } else {
                return Err(RevalError::LockfileMismatch {
                    message: format!("unlocked case `{}` present in eval suite", c.id),
                });
            }
        }
        Ok(())
    }

    pub fn save(&self, path: &Path) -> Result<(), RevalError> {
        let text = serde_json::to_string_pretty(self).map_err(|source| RevalError::Json {
            path: path.to_path_buf(),
            source,
        })?;
        fs::write(path, format!("{text}\n")).map_err(|source| RevalError::Io {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn load(path: &Path) -> Result<Self, RevalError> {
        let text = fs::read_to_string(path).map_err(|source| RevalError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_str(&text).map_err(|source| RevalError::Json {
            path: path.to_path_buf(),
            source,
        })
    }
}
