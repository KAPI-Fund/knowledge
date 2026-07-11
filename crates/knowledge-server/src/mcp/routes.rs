//! Stateless MCP Streamable HTTP endpoint: a single POST /api/mcp handling
//! JSON-RPC `initialize`, `notifications/*`, `tools/list`, and `tools/call`
//! with direct `application/json` responses (allowed by the MCP spec). No
//! sessions, no SSE, no batching. Auth is Bearer at the HTTP layer (401);
//! tool failures come back as `result.isError` per the spec, not RPC errors.

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::app::state::AppState;
use crate::auth::principal::resolve_principal;

use super::tools::{self, ToolError};

const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];
const DEFAULT_PROTOCOL_VERSION: &str = "2025-03-26";

pub async fn handle_mcp(
  State(state): State<AppState>,
  headers: HeaderMap,
  body: String,
) -> Response {
  let request = match parse_single_request(&body) {
    Ok(request) => request,
    Err(response) => return response,
  };
  let id = request.get("id").cloned().unwrap_or(Value::Null);
  let Some(method) = request.get("method").and_then(Value::as_str) else {
    return rpc_error(id, -32600, "Invalid Request: missing method").into_response();
  };
  let params = request.get("params").cloned().unwrap_or_else(|| json!({}));

  match method {
    "initialize" => initialize(id, &params).into_response(),
    method if method.starts_with("notifications/") => StatusCode::ACCEPTED.into_response(),
    "tools/list" | "tools/call" => {
      // Per the MCP spec, authorization happens at the HTTP layer.
      let principal = match resolve_principal(&state, &headers).await {
        Ok(principal) => principal,
        Err(_) => {
          return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "unauthorized" })),
          )
            .into_response();
        }
      };
      if method == "tools/list" {
        return rpc_result(id, json!({ "tools": tools::tool_list() })).into_response();
      }
      let Some(name) = params.get("name").and_then(Value::as_str) else {
        return rpc_error(id, -32602, "tools/call requires params.name").into_response();
      };
      let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
      match tools::call_tool(&state, &principal, name, &arguments).await {
        Ok(text) => rpc_result(
          id,
          json!({ "content": [{ "type": "text", "text": text }] }),
        )
        .into_response(),
        Err(ToolError::InvalidParams(message)) | Err(ToolError::UnknownTool(message)) => {
          rpc_error(id, -32602, &message).into_response()
        }
        Err(ToolError::Execution(message)) => rpc_result(
          id,
          json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true,
          }),
        )
        .into_response(),
      }
    }
    other => rpc_error(id, -32601, &format!("Method not found: {other}")).into_response(),
  }
}

pub async fn method_not_allowed() -> Response {
  StatusCode::METHOD_NOT_ALLOWED.into_response()
}

fn parse_single_request(body: &str) -> Result<Value, Response> {
  let value: Value = match serde_json::from_str(body) {
    Ok(value) => value,
    Err(_) => return Err(rpc_error(Value::Null, -32700, "Parse error").into_response()),
  };
  if value.is_array() {
    return Err(
      rpc_error(Value::Null, -32600, "Batch requests are not supported").into_response(),
    );
  }
  if !value.is_object() {
    return Err(rpc_error(Value::Null, -32600, "Invalid Request").into_response());
  }
  Ok(value)
}

fn initialize(id: Value, params: &Value) -> Json<Value> {
  let requested = params.get("protocolVersion").and_then(Value::as_str);
  rpc_result(
    id,
    json!({
      "protocolVersion": negotiate_protocol_version(requested),
      "capabilities": { "tools": {} },
      "serverInfo": {
        "name": "knowledge-server",
        "version": env!("CARGO_PKG_VERSION"),
      },
    }),
  )
}

fn negotiate_protocol_version(requested: Option<&str>) -> &'static str {
  requested
    .and_then(|version| {
      SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .find(|supported| **supported == version)
        .copied()
    })
    .unwrap_or(DEFAULT_PROTOCOL_VERSION)
}

fn rpc_result(id: Value, result: Value) -> Json<Value> {
  Json(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

fn rpc_error(id: Value, code: i64, message: &str) -> Json<Value> {
  Json(json!({
    "jsonrpc": "2.0",
    "id": id,
    "error": { "code": code, "message": message },
  }))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn negotiates_supported_versions_and_falls_back() {
    assert_eq!(negotiate_protocol_version(Some("2024-11-05")), "2024-11-05");
    assert_eq!(negotiate_protocol_version(Some("2025-03-26")), "2025-03-26");
    assert_eq!(negotiate_protocol_version(Some("2025-06-18")), "2025-06-18");
    assert_eq!(negotiate_protocol_version(Some("1999-01-01")), "2025-03-26");
    assert_eq!(negotiate_protocol_version(None), "2025-03-26");
  }

  #[test]
  fn rejects_batch_requests() {
    let response = parse_single_request("[]").unwrap_err();
    assert_eq!(response.status(), StatusCode::OK);
    let request = parse_single_request(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#).unwrap();
    assert_eq!(request["method"], "ping");
  }

  #[test]
  fn rejects_malformed_json_and_non_objects() {
    assert!(parse_single_request("{not json").is_err());
    assert!(parse_single_request("42").is_err());
    assert!(parse_single_request("\"text\"").is_err());
  }

  #[test]
  fn initialize_echoes_negotiated_version() {
    let Json(body) = initialize(json!(1), &json!({ "protocolVersion": "2025-06-18" }));
    assert_eq!(body["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(body["result"]["serverInfo"]["name"], "knowledge-server");
    assert!(body["result"]["capabilities"]["tools"].is_object());

    let Json(fallback) = initialize(json!(2), &json!({}));
    assert_eq!(fallback["result"]["protocolVersion"], "2025-03-26");
  }
}
