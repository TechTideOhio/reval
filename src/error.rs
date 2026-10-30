use std::path::PathBuf;

/// Domain errors for `reval` evaluation, trace analysis, and repository hygiene.
#[derive(Debug, thiserror::Error)]
pub enum RevalError {
    #[error("failed to read file at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse JSON at {}: {source}", path.display())]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to parse TOML at {}: {source}", path.display())]
    Toml {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("trace sample size ({found}) is below required minimum ({minimum})")]
    InsufficientTraces { found: usize, minimum: usize },

    #[error("metric regression detected across {count} gate threshold(s): {details}")]
    GateFailed { count: usize, details: String },

    #[error("repository hygiene audit found {count} blocking violation(s)")]
    AuditFailed { count: usize },

    #[error("eval case `{id}` failed critical blocker: {reason}")]
    BlockerFailed { id: String, reason: String },

    #[error("content-addressed lockfile mismatch: {message}")]
    LockfileMismatch { message: String },
}
