//! Transport-agnostic command dispatch.
//!
//! This module provides the [`Dispatcher`] trait that abstracts command
//! execution from the transport layer.  Both the in-process CLI path
//! (`DirectDispatcher` via `AppCore`) and the future daemon JSON-RPC
//! path will implement this trait.
//!
//! It also defines the JSON-RPC 2.0 envelope types used for daemon
//! communication over Unix Domain Sockets (or Named Pipes on Windows).

use crate::commands::{Command, CommandOutcome};
use crate::error::{AwoError, AwoResult};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Dispatcher trait
// ---------------------------------------------------------------------------

/// A transport-agnostic command executor.
///
/// Implementors accept a [`Command`] and return a [`CommandOutcome`].
/// This is the single entry point for all state-mutating orchestration
/// operations, whether invoked in-process or over RPC.
pub trait Dispatcher {
    fn dispatch(&mut self, command: Command) -> AwoResult<CommandOutcome>;
}

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 envelope types
// ---------------------------------------------------------------------------

/// A JSON-RPC 2.0 request envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default = "default_params")]
    pub params: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
}

fn default_params() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

/// A JSON-RPC 2.0 success result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResult {
    pub ok: bool,
    pub summary: String,
    pub events: Vec<crate::events::DomainEvent>,
}

/// A JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<RpcResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
}

// Standard JSON-RPC error codes
const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INTERNAL_ERROR: i64 = -32603;
const APPLICATION_ERROR: i64 = -32000;

impl RpcRequest {
    /// Build a well-formed request from a [`Command`].
    pub fn from_command(
        command: &Command,
        id: serde_json::Value,
    ) -> Result<Self, serde_json::Error> {
        let full = serde_json::to_value(command)?;
        let params = full
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        Ok(Self {
            jsonrpc: "2.0".to_string(),
            method: command.method_name().to_string(),
            params,
            id: Some(id),
        })
    }
}

impl RpcResponse {
    fn success(outcome: CommandOutcome, id: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(RpcResult {
                ok: true,
                summary: outcome.summary,
                events: outcome.events,
            }),
            error: None,
            id,
        }
    }

    fn error(code: i64, message: String, id: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(RpcError {
                code,
                message,
                data: None,
            }),
            id,
        }
    }
}

// ---------------------------------------------------------------------------
// RPC dispatch
// ---------------------------------------------------------------------------

/// Dispatch a raw JSON-RPC request through the given [`Dispatcher`].
///
/// This function handles the full JSON-RPC lifecycle:
/// 1. Parse the request envelope
/// 2. Decode method + params into a [`Command`]
/// 3. Execute via the dispatcher
/// 4. Wrap the result (or error) into an [`RpcResponse`]
pub fn dispatch_rpc(dispatcher: &mut dyn Dispatcher, request: &RpcRequest) -> RpcResponse {
    if request.jsonrpc != "2.0" {
        return RpcResponse::error(
            INVALID_REQUEST,
            "expected jsonrpc version \"2.0\"".to_string(),
            request.id.clone(),
        );
    }

    let command = match Command::from_rpc(&request.method, request.params.clone()) {
        Ok(command) => command,
        Err(error) => {
            return RpcResponse::error(
                METHOD_NOT_FOUND,
                format!("unknown or malformed method `{}`: {error}", request.method),
                request.id.clone(),
            );
        }
    };

    match dispatcher.dispatch(command) {
        Ok(outcome) => RpcResponse::success(outcome, request.id.clone()),
        Err(error) => RpcResponse::error(APPLICATION_ERROR, error.to_string(), request.id.clone()),
    }
}

/// Parse a raw JSON byte slice into an [`RpcRequest`], or return
/// a parse-error [`RpcResponse`].
pub fn parse_rpc_request(bytes: &[u8]) -> Result<RpcRequest, Box<RpcResponse>> {
    serde_json::from_slice::<RpcRequest>(bytes).map_err(|error| {
        Box::new(RpcResponse::error(
            PARSE_ERROR,
            format!("parse error: {error}"),
            None,
        ))
    })
}

/// Map an [`AwoError`] to a JSON-RPC error code.
pub fn error_code_for(error: &AwoError) -> i64 {
    match error {
        AwoError::UnknownRepoId { .. }
        | AwoError::UnknownSlotId { .. }
        | AwoError::UnknownSessionId { .. }
        | AwoError::UnknownTaskId { .. }
        | AwoError::UnknownOwnerId { .. } => METHOD_NOT_FOUND,
        AwoError::Validation { .. }
        | AwoError::InvalidState { .. }
        | AwoError::UnsupportedValue { .. } => INVALID_REQUEST,
        _ => INTERNAL_ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_roundtrip_serialization() {
        let commands = vec![
            Command::NoOp {
                label: "test".to_string(),
            },
            Command::RepoList,
            Command::SlotAcquire {
                repo_id: "repo-1".to_string(),
                task_name: "fix-bug".to_string(),
                strategy: crate::slot::SlotStrategy::Fresh,
            },
            Command::SessionStart {
                slot_id: "slot-1".to_string(),
                runtime: crate::runtime::RuntimeKind::Claude,
                prompt: "Fix the tests".to_string(),
                read_only: false,
                dry_run: true,
                launch_mode: crate::runtime::SessionLaunchMode::Pty,
                attach_context: true,
                timeout_secs: None,
            },
            Command::SlotList { repo_id: None },
            Command::SessionCancel {
                session_id: "sess-1".to_string(),
            },
            Command::ReviewStatus {
                repo_id: Some("repo-1".to_string()),
            },
        ];

        for original in &commands {
            let json = serde_json::to_string(original)
                .unwrap_or_else(|e| panic!("serialize {:?}: {e}", original.method_name()));
            let restored: Command = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("deserialize {:?}: {e}", original.method_name()));
            assert_eq!(
                original.method_name(),
                restored.method_name(),
                "method name mismatch after roundtrip"
            );
        }
    }

    #[test]
    fn command_method_names_are_dot_separated() {
        let command = Command::SlotAcquire {
            repo_id: "r".to_string(),
            task_name: "t".to_string(),
            strategy: crate::slot::SlotStrategy::Warm,
        };
        assert_eq!(command.method_name(), "slot.acquire");

        let command = Command::SessionStart {
            slot_id: "s".to_string(),
            runtime: crate::runtime::RuntimeKind::Shell,
            prompt: "p".to_string(),
            read_only: false,
            dry_run: false,
            launch_mode: crate::runtime::SessionLaunchMode::Oneshot,
            attach_context: false,
            timeout_secs: None,
        };
        assert_eq!(command.method_name(), "session.start");
    }

    #[test]
    fn command_from_rpc_parses_method_and_params() {
        let params = serde_json::json!({
            "repo_id": "my-repo",
            "task_name": "refactor",
            "strategy": "fresh"
        });
        let command = Command::from_rpc("slot.acquire", params).unwrap();
        assert_eq!(command.method_name(), "slot.acquire");
        if let Command::SlotAcquire {
            repo_id,
            task_name,
            strategy,
        } = command
        {
            assert_eq!(repo_id, "my-repo");
            assert_eq!(task_name, "refactor");
            assert_eq!(strategy, crate::slot::SlotStrategy::Fresh);
        } else {
            panic!("expected SlotAcquire");
        }
    }

    #[test]
    fn command_from_rpc_rejects_unknown_method() {
        let result = Command::from_rpc("bogus.method", serde_json::json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn rpc_request_from_command_roundtrip() {
        let command = Command::SlotRelease {
            slot_id: "slot-42".to_string(),
        };
        let request = RpcRequest::from_command(&command, serde_json::json!(7)).unwrap();
        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.method, "slot.release");
        assert_eq!(request.id, Some(serde_json::json!(7)));
        assert_eq!(request.params["slot_id"], "slot-42");

        // Reconstruct the command from the request
        let restored = Command::from_rpc(&request.method, request.params).unwrap();
        assert_eq!(restored.method_name(), "slot.release");
    }

    #[test]
    fn rpc_response_success_shape() {
        let outcome = CommandOutcome {
            summary: "done".to_string(),
            events: vec![],
        };
        let response = RpcResponse::success(outcome, Some(serde_json::json!(1)));
        assert!(response.result.is_some());
        assert!(response.error.is_none());
        assert!(response.result.unwrap().ok);
    }

    #[test]
    fn rpc_response_error_shape() {
        let response = RpcResponse::error(
            APPLICATION_ERROR,
            "something failed".to_string(),
            Some(serde_json::json!(2)),
        );
        assert!(response.result.is_none());
        assert!(response.error.is_some());
        let error = response.error.unwrap();
        assert_eq!(error.code, APPLICATION_ERROR);
        assert_eq!(error.message, "something failed");
    }

    #[test]
    fn parse_rpc_request_valid() {
        let json = br#"{"jsonrpc":"2.0","method":"repo.list","params":{},"id":1}"#;
        let request = parse_rpc_request(json).unwrap();
        assert_eq!(request.method, "repo.list");
    }

    #[test]
    fn parse_rpc_request_invalid_json() {
        let json = b"not json";
        let response = parse_rpc_request(json).unwrap_err();
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, PARSE_ERROR);
    }

    #[test]
    fn dispatch_rpc_rejects_wrong_version() {
        struct NoopDispatcher;
        impl Dispatcher for NoopDispatcher {
            fn dispatch(&mut self, _: Command) -> AwoResult<CommandOutcome> {
                unreachable!()
            }
        }

        let request = RpcRequest {
            jsonrpc: "1.0".to_string(),
            method: "noop".to_string(),
            params: serde_json::json!({"label": "test"}),
            id: Some(serde_json::json!(1)),
        };
        let response = dispatch_rpc(&mut NoopDispatcher, &request);
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, INVALID_REQUEST);
    }

    #[test]
    fn dispatch_rpc_rejects_unknown_method() {
        struct NoopDispatcher;
        impl Dispatcher for NoopDispatcher {
            fn dispatch(&mut self, _: Command) -> AwoResult<CommandOutcome> {
                unreachable!()
            }
        }

        let request = RpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "bogus".to_string(),
            params: serde_json::json!({}),
            id: Some(serde_json::json!(1)),
        };
        let response = dispatch_rpc(&mut NoopDispatcher, &request);
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, METHOD_NOT_FOUND);
    }

    #[test]
    fn error_code_mapping() {
        assert_eq!(
            error_code_for(&AwoError::unknown_repo("x")),
            METHOD_NOT_FOUND
        );
        assert_eq!(
            error_code_for(&AwoError::validation("bad")),
            INVALID_REQUEST
        );
        assert_eq!(
            error_code_for(&AwoError::supervisor("boom")),
            INTERNAL_ERROR
        );
    }

    #[test]
    fn unit_variant_serialization() {
        // RepoList has no params — ensure it round-trips correctly
        let command = Command::RepoList;
        let json = serde_json::to_value(&command).unwrap();
        assert_eq!(json["method"], "repo.list");
        // Unit variants with adjacently-tagged may not have "params" key
        let restored: Command = serde_json::from_value(json).unwrap();
        assert_eq!(restored.method_name(), "repo.list");
    }
}
