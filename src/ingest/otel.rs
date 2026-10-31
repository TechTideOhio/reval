use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::coding::axial::axial_category_for;
use crate::coding::open::{annotate_failure_raw, OpenCode};
use crate::ingest::{TraceOutcome, TraceRecord};

/// OpenTelemetry Span Attribute value representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OTelAttributeValue {
    String {
        #[serde(rename = "stringValue")]
        string_value: String,
    },
    Int {
        #[serde(rename = "intValue")]
        int_value: i64,
    },
    Bool {
        #[serde(rename = "boolValue")]
        bool_value: bool,
    },
}

/// OpenTelemetry Key-Value attribute pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OTelAttribute {
    pub key: String,
    pub value: OTelAttributeValue,
}

impl OTelAttribute {
    pub fn string(key: impl Into<String>, val: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: OTelAttributeValue::String {
                string_value: val.into(),
            },
        }
    }

    pub fn int(key: impl Into<String>, val: i64) -> Self {
        Self {
            key: key.into(),
            value: OTelAttributeValue::Int { int_value: val },
        }
    }

    pub fn bool(key: impl Into<String>, val: bool) -> Self {
        Self {
            key: key.into(),
            value: OTelAttributeValue::Bool { bool_value: val },
        }
    }
}

/// OpenTelemetry Span Status (1 = OK, 2 = ERROR).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OTelSpanStatus {
    pub code: u32,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub message: String,
}

/// OpenTelemetry / `OpenInference` Span representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OTelSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: String,
    pub name: String,
    pub kind: u32,
    pub start_time_unix_nano: u64,
    pub end_time_unix_nano: u64,
    pub attributes: Vec<OTelAttribute>,
    pub status: OTelSpanStatus,
}

/// OpenTelemetry Scope Spans bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OTelScopeSpans {
    pub scope: OTelInstrumentationScope,
    pub spans: Vec<OTelSpan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OTelInstrumentationScope {
    pub name: String,
    pub version: String,
}

/// OpenTelemetry Resource Spans root object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OTelResourceSpans {
    pub resource: OTelResource,
    pub scope_spans: Vec<OTelScopeSpans>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OTelResource {
    pub attributes: Vec<OTelAttribute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OTelExportBundle {
    pub resource_spans: Vec<OTelResourceSpans>,
}

/// `OpenInference` single-span JSONL record for direct `LangSmith` / Phoenix ingestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenInferenceRecord {
    pub id: String,
    pub trace_id: String,
    pub span_id: String,
    pub name: String,
    pub span_kind: String,
    pub start_time: String,
    pub end_time: String,
    pub status_code: String,
    pub status_message: Option<String>,
    pub input: String,
    pub output: String,
    pub agent_name: String,
    pub open_code: String,
    pub axial_category: String,
}

/// Derives a 32-hex-character trace ID deterministically from a string.
fn derive_trace_id(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"trace:");
    hasher.update(input.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    hash[..32].to_string()
}

/// Derives a 16-hex-character span ID deterministically from a string.
fn derive_span_id(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"span:");
    hasher.update(input.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    hash[..16].to_string()
}

/// Converts a slice of `TraceRecord`s into OpenTelemetry and `OpenInference` formats and writes to `out_dir`.
pub fn export_otel_and_openinference(
    traces: &[TraceRecord],
    out_dir: &Path,
) -> Result<(PathBuf, PathBuf)> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create output dir {}", out_dir.display()))?;

    let otel_path = out_dir.join("traces.otel.json");
    let openinference_path = out_dir.join("traces.openinference.jsonl");

    let mut spans = Vec::new();
    let mut openinference_lines = String::new();

    // Base epoch timestamp: 2026-10-30T12:00:00Z in nanos
    let base_nano = 1_793_361_600_000_000_000u64;

    for (idx, r) in traces.iter().enumerate() {
        let trace_id = derive_trace_id(&r.id);
        let span_id = derive_span_id(&format!("{}:{}", r.id, r.command_or_summary));

        let (open_code, _) =
            annotate_failure_raw(r).unwrap_or((OpenCode::UnclassifiedFailure, String::new()));
        let axial = axial_category_for(open_code);

        let duration_ms = r.duration_ms.unwrap_or(100);
        let start_time_nano = base_nano + (idx as u64 * 1_000_000_000);
        let end_time_nano = start_time_nano + (duration_ms * 1_000_000);

        let (status_code, status_msg) = if r.outcome == TraceOutcome::Pass {
            (1u32, String::new())
        } else {
            (2u32, r.diagnostic_excerpt.clone())
        };

        let mut attributes = vec![
            OTelAttribute::string("openinference.span.kind", "AGENT"),
            OTelAttribute::string("agent.name", &r.agent_or_runner),
            OTelAttribute::string("input.value", &r.command_or_summary),
            OTelAttribute::string("output.value", &r.diagnostic_excerpt),
            OTelAttribute::string("trace.id", &r.id),
            OTelAttribute::string(
                "trace.outcome",
                if r.outcome == TraceOutcome::Pass {
                    "pass"
                } else {
                    "fail"
                },
            ),
            OTelAttribute::int(
                "trace.duration_ms",
                i64::try_from(duration_ms).unwrap_or(i64::MAX),
            ),
        ];

        if r.outcome != TraceOutcome::Pass {
            attributes.push(OTelAttribute::string("reval.open_code", open_code.as_str()));
            attributes.push(OTelAttribute::string(
                "reval.axial_category",
                axial.as_str(),
            ));
            attributes.push(OTelAttribute::string(
                "reval.remediation_hint",
                axial.remediation_hint(),
            ));
        }

        spans.push(OTelSpan {
            trace_id: trace_id.clone(),
            span_id: span_id.clone(),
            parent_span_id: String::new(),
            name: r.command_or_summary.clone(),
            kind: 1, // INTERNAL
            start_time_unix_nano: start_time_nano,
            end_time_unix_nano: end_time_nano,
            attributes,
            status: OTelSpanStatus {
                code: status_code,
                message: status_msg.clone(),
            },
        });

        // OpenInference JSONL record
        let oi_rec = OpenInferenceRecord {
            id: r.id.clone(),
            trace_id,
            span_id,
            name: r.command_or_summary.clone(),
            span_kind: "AGENT".to_string(),
            start_time: format!("{}ns", start_time_nano),
            end_time: format!("{}ns", end_time_nano),
            status_code: if r.outcome == TraceOutcome::Pass {
                "OK".to_string()
            } else {
                "ERROR".to_string()
            },
            status_message: if status_msg.is_empty() {
                None
            } else {
                Some(status_msg)
            },
            input: r.command_or_summary.clone(),
            output: r.diagnostic_excerpt.clone(),
            agent_name: r.agent_or_runner.clone(),
            open_code: open_code.as_str().to_string(),
            axial_category: axial.as_str().to_string(),
        };
        openinference_lines.push_str(&serde_json::to_string(&oi_rec)?);
        openinference_lines.push('\n');
    }

    let bundle = OTelExportBundle {
        resource_spans: vec![OTelResourceSpans {
            resource: OTelResource {
                attributes: vec![
                    OTelAttribute::string("service.name", "reval-agent-harness"),
                    OTelAttribute::string("telemetry.sdk.language", "rust"),
                    OTelAttribute::string("telemetry.sdk.name", "reval"),
                    OTelAttribute::string("telemetry.sdk.version", "0.2.0"),
                ],
            },
            scope_spans: vec![OTelScopeSpans {
                scope: OTelInstrumentationScope {
                    name: "reval.tracer".to_string(),
                    version: "0.2.0".to_string(),
                },
                spans,
            }],
        }],
    };

    fs::write(&otel_path, serde_json::to_string_pretty(&bundle)?)?;
    fs::write(&openinference_path, openinference_lines)?;

    Ok((otel_path, openinference_path))
}
