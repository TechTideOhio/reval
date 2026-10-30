pub mod proof;
pub mod ratchet;
pub mod reliability;

pub use proof::{evaluate_gate, GateCheck, GateVerdict};
pub use ratchet::{evaluate_traces, BaselineRatchet, RevalReport};
pub use reliability::{compute_corpus_sha256, compute_reliability, ReliabilityMetrics};
