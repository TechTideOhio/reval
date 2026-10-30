use serde::{Deserialize, Serialize};

/// Canonical execution status aligned with `AGENTS.md` and `agent-harness-construction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnvelopeStatus {
    Success,
    Warning,
    Error,
}

impl EnvelopeStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// Deterministic machine output envelope emitted by all `reval` subcommands in `--json` mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessEnvelope<T: Serialize> {
    pub status: EnvelopeStatus,
    pub summary: String,
    pub next_actions: Vec<String>,
    pub artifacts: Vec<String>,
    pub data: T,
}

impl<T: Serialize> HarnessEnvelope<T> {
    pub fn new(
        status: EnvelopeStatus,
        summary: impl Into<String>,
        next_actions: Vec<String>,
        artifacts: Vec<String>,
        data: T,
    ) -> Self {
        Self {
            status,
            summary: summary.into(),
            next_actions,
            artifacts,
            data,
        }
    }
}
