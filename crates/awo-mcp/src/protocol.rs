//! MCP (Model Context Protocol) JSON-RPC types.
//!
//! Implements the subset of the MCP specification needed for a tool-serving
//! server over stdio transport (newline-delimited JSON-RPC 2.0).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Inbound: messages from the MCP client
// ---------------------------------------------------------------------------

/// A generic JSON-RPC 2.0 message read from stdin.
///
/// If `id` is `None`, this is a notification (no response expected).
#[derive(Debug, Deserialize)]
pub struct JsonRpcMessage {
    #[allow(dead_code)]
    pub jsonrpc: String,
    pub method: String,
    #[serde(default = "empty_object")]
    pub params: serde_json::Value,
    pub id: Option<serde_json::Value>,
}

fn empty_object() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

// ---------------------------------------------------------------------------
// Outbound: responses to the MCP client
// ---------------------------------------------------------------------------

/// A JSON-RPC 2.0 response written to stdout.
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 notification written to stdout.
#[derive(Debug, Serialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: &'static str,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

impl JsonRpcResponse {
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0",
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(id: Option<serde_json::Value>, code: i64, message: String) -> Self {
        Self {
            jsonrpc: "2.0",
            result: None,
            error: Some(JsonRpcError { code, message }),
            id,
        }
    }
}

impl JsonRpcNotification {
    pub fn new(method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0",
            method: method.into(),
            params: Some(params),
        }
    }
}

// Standard JSON-RPC error codes
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INTERNAL_ERROR: i64 = -32603;

// ---------------------------------------------------------------------------
// MCP protocol types
// ---------------------------------------------------------------------------

/// Server information returned in the `initialize` response.
#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Server capabilities declared during initialization.
#[derive(Debug, Serialize)]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
}

#[derive(Debug, Serialize)]
pub struct ToolsCapability {}

#[derive(Debug, Serialize)]
pub struct ResourcesCapability {
    pub subscribe: bool,
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

/// The result of an `initialize` request.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ServerInfo,
}

/// A tool definition returned by `tools/list`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// The result of a `tools/call` request.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
}

/// A content block within a tool call result.
#[derive(Debug, Serialize)]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

impl ToolCallResult {
    pub fn text(text: String) -> Self {
        Self {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text,
            }],
            is_error: false,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: message,
            }],
            is_error: true,
        }
    }
}

/// A resource definition returned by `resources/list`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDefinition {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime_type: String,
}

/// A resource content block returned by `resources/read`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceContent {
    pub uri: String,
    pub mime_type: String,
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_rpc_response_success_shape() {
        let resp =
            JsonRpcResponse::success(Some(serde_json::json!(1)), serde_json::json!({"ok": true}));
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["result"]["ok"], true);
        assert!(json.get("error").is_none());
        assert_eq!(json["id"], 1);
    }

    #[test]
    fn json_rpc_response_error_shape() {
        let resp = JsonRpcResponse::error(
            Some(serde_json::json!(2)),
            METHOD_NOT_FOUND,
            "not found".into(),
        );
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(json["error"]["message"], "not found");
        assert!(json.get("result").is_none());
    }

    #[test]
    fn tool_call_result_text_is_not_error() {
        let result = ToolCallResult::text("done".into());
        assert!(!result.is_error);
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].content_type, "text");
    }

    #[test]
    fn tool_call_result_error_is_flagged() {
        let result = ToolCallResult::error("bad".into());
        assert!(result.is_error);
    }

    #[test]
    fn initialize_result_serializes_camel_case() {
        let result = InitializeResult {
            protocol_version: "2024-11-05".into(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {}),
                resources: None,
            },
            server_info: ServerInfo {
                name: "test".into(),
                version: "0.0.0".into(),
            },
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("protocolVersion").is_some());
        assert!(json.get("serverInfo").is_some());
        assert!(json.get("protocol_version").is_none());
    }

    #[test]
    fn message_deserializes_without_params() {
        let raw = r#"{"jsonrpc":"2.0","method":"ping","id":1}"#;
        let msg: JsonRpcMessage = serde_json::from_str(raw).unwrap();
        assert_eq!(msg.method, "ping");
        assert!(msg.params.is_object());
    }

    #[test]
    fn notification_has_no_id() {
        let raw = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let msg: JsonRpcMessage = serde_json::from_str(raw).unwrap();
        assert!(msg.id.is_none());
    }
}
