use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

use anyhow::Result;
use serde_json::{json, Value};

use crate::coding::causal::CausalFailureGraph;
use crate::coding::lessons::query_lessons;
use crate::gate::{evaluate_gate, evaluate_traces, BaselineRatchet};
use crate::hygiene::audit_repository;
use crate::ingest::ingest_workspace_traces;

/// Starts the Model Context Protocol (MCP) server over standard input/output.
pub fn run_mcp_server(workspace_root: &Path) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    let root = workspace_root.to_path_buf();

    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let req: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("");
        let id = req.get("id").cloned();

        let resp = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {},
                        "resources": {}
                    },
                    "serverInfo": {
                        "name": "reval",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            }),
            "notifications/initialized" => continue,
            "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [
                        {
                            "name": "reval_check_gate",
                            "description": "Evaluates whether current agent execution traces pass baseline reliability gates.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "baseline_path": {
                                        "type": "string",
                                        "description": "Optional path to baseline.json file (defaults to Tools/reval/fixtures/baseline.json)"
                                    }
                                }
                            }
                        },
                        {
                            "name": "reval_analyze_traces",
                            "description": "Performs Husain/Shankar trace error analysis (open/axial coding) and computes Top 3 metrics.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {}
                            }
                        },
                        {
                            "name": "reval_get_lessons",
                            "description": "Queries historical failure traps and prescriptive lessons learned for an intended task (Mimir/agent memory).",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "query": {
                                        "type": "string",
                                        "description": "Task keywords or tool name (e.g. 'vitest', 'claude', 'contract rebuild')"
                                    }
                                }
                            }
                        },
                        {
                            "name": "reval_repo_hygiene",
                            "description": "Scans workspace for committed .env secrets, lockfile conflicts, and dangling skill references.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "target_path": {
                                        "type": "string",
                                        "description": "Subdirectory to scan (defaults to workspace root)"
                                    }
                                }
                            }
                        }
                    ]
                }
            }),
            "tools/call" => {
                let params = req.get("params").cloned().unwrap_or(Value::Null);
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));

                let content_text = handle_tool_call(&root, tool_name, &args);

                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": content_text
                            }
                        ]
                    }
                })
            }
            "resources/list" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "resources": [
                        {
                            "uri": "reval://metrics/top3",
                            "name": "Top 3 Production Failure Metrics",
                            "mimeType": "application/json"
                        },
                        {
                            "uri": "reval://graph/causal",
                            "name": "Causal Failure Transition Graph",
                            "mimeType": "application/json"
                        },
                        {
                            "uri": "reval://lessons/latest",
                            "name": "Proactive Lessons Learned for Agent Memory",
                            "mimeType": "application/json"
                        }
                    ]
                }
            }),
            "resources/read" => {
                let params = req.get("params").cloned().unwrap_or(Value::Null);
                let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");

                let content_text = handle_resource_read(&root, uri);

                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "contents": [
                            {
                                "uri": uri,
                                "mimeType": "application/json",
                                "text": content_text
                            }
                        ]
                    }
                })
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("Method `{method}` not found")
                }
            }),
        };

        let out = serde_json::to_string(&resp)? + "\n";
        stdout.write_all(out.as_bytes())?;
        stdout.flush()?;
    }

    Ok(())
}

fn handle_tool_call(root: &Path, name: &str, args: &Value) -> String {
    match name {
        "reval_check_gate" => {
            let baseline_str = args
                .get("baseline_path")
                .and_then(|v| v.as_str())
                .unwrap_or("Tools/reval/fixtures/baseline.json");
            let baseline_path = root.join(baseline_str);

            let baseline_raw = match fs::read_to_string(&baseline_path) {
                Ok(s) => s,
                Err(err) => {
                    return format!(
                        "Failed to read baseline file at {}: {err}",
                        baseline_path.display()
                    )
                }
            };
            let ratchet: BaselineRatchet = match serde_json::from_str(&baseline_raw) {
                Ok(r) => r,
                Err(err) => {
                    return format!(
                        "Failed to parse baseline file at {}: {err}",
                        baseline_path.display()
                    )
                }
            };

            let traces = match ingest_workspace_traces(root, None) {
                Ok(t) => t,
                Err(err) => return format!("Failed to ingest traces: {err}"),
            };

            let report = match evaluate_traces(&traces, 1) {
                Ok(r) => r,
                Err(err) => return format!("Failed to evaluate traces: {err}"),
            };

            let verdict = evaluate_gate(report, &ratchet);
            serde_json::to_string_pretty(&verdict).unwrap_or_else(|_| "{}".to_string())
        }
        "reval_analyze_traces" => {
            let traces = match ingest_workspace_traces(root, None) {
                Ok(t) => t,
                Err(err) => return format!("Failed to ingest traces: {err}"),
            };

            let report = match evaluate_traces(&traces, 1) {
                Ok(r) => r,
                Err(err) => return format!("Failed to evaluate traces: {err}"),
            };
            serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
        }
        "reval_get_lessons" => {
            let query = args.get("query").and_then(|v| v.as_str());
            let traces = match ingest_workspace_traces(root, None) {
                Ok(t) => t,
                Err(err) => return format!("Failed to ingest traces: {err}"),
            };

            let graph = CausalFailureGraph::from_traces(&traces);
            let lessons = query_lessons(&graph, query);
            serde_json::to_string_pretty(&lessons).unwrap_or_else(|_| "[]".to_string())
        }
        "reval_repo_hygiene" => {
            let sub = args
                .get("target_path")
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let target = root.join(sub);
            let report = match audit_repository(&target, &[]) {
                Ok(r) => r,
                Err(err) => return format!("Failed to audit repository: {err}"),
            };
            serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
        }
        _ => format!("Unknown tool `{name}`"),
    }
}

fn handle_resource_read(root: &Path, uri: &str) -> String {
    let traces = match ingest_workspace_traces(root, None) {
        Ok(t) => t,
        Err(err) => return format!("{{\"error\": \"{err}\"}}"),
    };

    match uri {
        "reval://metrics/top3" => {
            let report = match evaluate_traces(&traces, 1) {
                Ok(r) => r,
                Err(err) => return format!("{{\"error\": \"{err}\"}}"),
            };
            serde_json::to_string_pretty(&report.first_three_metrics)
                .unwrap_or_else(|_| "{}".to_string())
        }
        "reval://graph/causal" => {
            let graph = CausalFailureGraph::from_traces(&traces);
            serde_json::to_string_pretty(&graph).unwrap_or_else(|_| "{}".to_string())
        }
        "reval://lessons/latest" => {
            let graph = CausalFailureGraph::from_traces(&traces);
            let lessons = query_lessons(&graph, None);
            serde_json::to_string_pretty(&lessons).unwrap_or_else(|_| "[]".to_string())
        }
        _ => format!("{{\"error\": \"Unknown resource URI `{uri}`\"}}"),
    }
}
