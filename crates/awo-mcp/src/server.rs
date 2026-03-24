//! MCP server: handles MCP JSON-RPC requests and dispatches awo commands.
//!
//! The server is transport-agnostic — it processes individual messages and
//! returns optional responses.  The stdio loop in `main.rs` handles I/O.

use crate::protocol::{
    INTERNAL_ERROR, InitializeResult, JsonRpcMessage, JsonRpcResponse, METHOD_NOT_FOUND,
    ResourceContent, ResourceDefinition, ResourcesCapability, ServerCapabilities, ServerInfo,
    ToolCallResult, ToolContent, ToolDefinition, ToolsCapability,
};
use awo_core::dispatch::Dispatcher;

/// An MCP tool-serving server backed by an awo [`Dispatcher`].
pub struct McpServer {
    dispatcher: Box<dyn Dispatcher>,
    initialized: bool,
}

impl McpServer {
    pub fn new(dispatcher: Box<dyn Dispatcher>) -> Self {
        Self {
            dispatcher,
            initialized: false,
        }
    }

    /// Process one inbound message.  Returns `None` for notifications
    /// (which must not receive a response per MCP spec).
    pub fn handle_message(&mut self, msg: &JsonRpcMessage) -> Option<JsonRpcResponse> {
        // Notifications have no id — never respond
        if msg.id.is_none() {
            self.handle_notification(&msg.method);
            return None;
        }

        let id = msg.id.clone();
        let result = match msg.method.as_str() {
            "initialize" => self.handle_initialize(),
            "ping" => Ok(serde_json::json!({})),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(&msg.params),
            "resources/list" => self.handle_resources_list(),
            "resources/read" => self.handle_resources_read(&msg.params),
            other => {
                return Some(JsonRpcResponse::error(
                    id,
                    METHOD_NOT_FOUND,
                    format!("unknown method: {other}"),
                ));
            }
        };

        Some(match result {
            Ok(value) => JsonRpcResponse::success(id, value),
            Err(message) => JsonRpcResponse::error(id, INTERNAL_ERROR, message),
        })
    }

    fn handle_notification(&mut self, method: &str) {
        if method == "notifications/initialized" {
            tracing::info!("MCP client initialized");
        }
    }

    // -----------------------------------------------------------------------
    // initialize
    // -----------------------------------------------------------------------

    fn handle_initialize(&mut self) -> Result<serde_json::Value, String> {
        self.initialized = true;
        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {}),
                resources: Some(ResourcesCapability {
                    subscribe: false,
                    list_changed: false,
                }),
            },
            server_info: ServerInfo {
                name: "awo-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };
        serde_json::to_value(&result).map_err(|e| e.to_string())
    }

    // -----------------------------------------------------------------------
    // tools/list
    // -----------------------------------------------------------------------

    fn handle_tools_list(&self) -> Result<serde_json::Value, String> {
        let tools = tool_definitions();
        serde_json::to_value(serde_json::json!({ "tools": tools })).map_err(|e| e.to_string())
    }

    // -----------------------------------------------------------------------
    // tools/call
    // -----------------------------------------------------------------------

    fn handle_tools_call(
        &mut self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let tool_name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'name' in tools/call params".to_string())?;

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        let result = self.execute_tool(tool_name, &arguments);
        serde_json::to_value(&result).map_err(|e| e.to_string())
    }

    fn execute_tool(&mut self, tool_name: &str, arguments: &serde_json::Value) -> ToolCallResult {
        let command = match map_tool_to_command(tool_name, arguments) {
            Ok(cmd) => cmd,
            Err(message) => return ToolCallResult::error(message),
        };

        match self.dispatcher.dispatch(command) {
            Ok(outcome) => {
                // Include events as structured JSON if any exist
                if outcome.events.is_empty() {
                    ToolCallResult::text(outcome.summary)
                } else {
                    let events_json = serde_json::to_string_pretty(&outcome.events)
                        .unwrap_or_else(|_| "[]".to_string());
                    ToolCallResult {
                        content: vec![
                            ToolContent {
                                content_type: "text".to_string(),
                                text: outcome.summary,
                            },
                            ToolContent {
                                content_type: "text".to_string(),
                                text: format!("Events:\n{events_json}"),
                            },
                        ],
                        is_error: false,
                    }
                }
            }
            Err(error) => ToolCallResult::error(error.to_string()),
        }
    }

    // -----------------------------------------------------------------------
    // resources/list
    // -----------------------------------------------------------------------

    fn handle_resources_list(&self) -> Result<serde_json::Value, String> {
        let resources = resource_definitions();
        serde_json::to_value(serde_json::json!({ "resources": resources }))
            .map_err(|e| e.to_string())
    }

    // -----------------------------------------------------------------------
    // resources/read
    // -----------------------------------------------------------------------

    fn handle_resources_read(
        &mut self,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let uri = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'uri' in resources/read params".to_string())?;

        let content = self.read_resource(uri)?;
        serde_json::to_value(serde_json::json!({ "contents": [content] }))
            .map_err(|e| e.to_string())
    }

    fn read_resource(&mut self, uri: &str) -> Result<ResourceContent, String> {
        match uri {
            "awo://repos" => {
                let command = awo_core::Command::RepoList;
                let outcome = self
                    .dispatcher
                    .dispatch(command)
                    .map_err(|e| e.to_string())?;
                let events_json = serde_json::to_string_pretty(&outcome.events)
                    .unwrap_or_else(|_| "[]".to_string());
                Ok(ResourceContent {
                    uri: uri.to_string(),
                    mime_type: "application/json".to_string(),
                    text: events_json,
                })
            }
            "awo://slots" => {
                let command = awo_core::Command::SlotList { repo_id: None };
                let outcome = self
                    .dispatcher
                    .dispatch(command)
                    .map_err(|e| e.to_string())?;
                let events_json = serde_json::to_string_pretty(&outcome.events)
                    .unwrap_or_else(|_| "[]".to_string());
                Ok(ResourceContent {
                    uri: uri.to_string(),
                    mime_type: "application/json".to_string(),
                    text: events_json,
                })
            }
            "awo://sessions" => {
                let command = awo_core::Command::SessionList { repo_id: None };
                let outcome = self
                    .dispatcher
                    .dispatch(command)
                    .map_err(|e| e.to_string())?;
                let events_json = serde_json::to_string_pretty(&outcome.events)
                    .unwrap_or_else(|_| "[]".to_string());
                Ok(ResourceContent {
                    uri: uri.to_string(),
                    mime_type: "application/json".to_string(),
                    text: events_json,
                })
            }
            "awo://review" => {
                let command = awo_core::Command::ReviewStatus { repo_id: None };
                let outcome = self
                    .dispatcher
                    .dispatch(command)
                    .map_err(|e| e.to_string())?;
                let events_json = serde_json::to_string_pretty(&outcome.events)
                    .unwrap_or_else(|_| "[]".to_string());
                Ok(ResourceContent {
                    uri: uri.to_string(),
                    mime_type: "application/json".to_string(),
                    text: events_json,
                })
            }
            "awo://teams" => {
                let command = awo_core::Command::TeamList { repo_id: None };
                let outcome = self
                    .dispatcher
                    .dispatch(command)
                    .map_err(|e| e.to_string())?;
                let events_json = serde_json::to_string_pretty(&outcome.events)
                    .unwrap_or_else(|_| "[]".to_string());
                Ok(ResourceContent {
                    uri: uri.to_string(),
                    mime_type: "application/json".to_string(),
                    text: events_json,
                })
            }
            _ => Err(format!("unknown resource URI: {uri}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "list_repos".to_string(),
            description: "List all registered Git repositories.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        ToolDefinition {
            name: "acquire_slot".to_string(),
            description: "Acquire a Git worktree slot for isolated work on a repository."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo_id": {
                        "type": "string",
                        "description": "The repository identifier."
                    },
                    "task_name": {
                        "type": "string",
                        "description": "A short name for the task this slot will be used for."
                    },
                    "strategy": {
                        "type": "string",
                        "enum": ["fresh", "warm"],
                        "description": "Slot creation strategy. 'fresh' creates a new worktree, 'warm' reuses an existing one if available.",
                        "default": "fresh"
                    }
                },
                "required": ["repo_id", "task_name"],
            }),
        },
        ToolDefinition {
            name: "release_slot".to_string(),
            description: "Release a previously acquired worktree slot.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "slot_id": {
                        "type": "string",
                        "description": "The slot identifier to release."
                    }
                },
                "required": ["slot_id"],
            }),
        },
        ToolDefinition {
            name: "list_slots".to_string(),
            description: "List all active worktree slots, optionally filtered by repository."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo_id": {
                        "type": "string",
                        "description": "Optional repository ID to filter by."
                    }
                },
            }),
        },
        ToolDefinition {
            name: "start_session".to_string(),
            description:
                "Start an AI agent session in a slot with the specified runtime and prompt."
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "slot_id": {
                        "type": "string",
                        "description": "The slot to run the session in."
                    },
                    "runtime": {
                        "type": "string",
                        "enum": ["codex", "claude", "gemini", "shell"],
                        "description": "The AI runtime to use."
                    },
                    "prompt": {
                        "type": "string",
                        "description": "The task prompt for the AI agent."
                    },
                    "read_only": {
                        "type": "boolean",
                        "description": "Whether the session should be read-only.",
                        "default": false
                    },
                    "dry_run": {
                        "type": "boolean",
                        "description": "If true, prepare but do not execute the session.",
                        "default": false
                    },
                    "launch_mode": {
                        "type": "string",
                        "enum": ["pty", "oneshot"],
                        "description": "Session launch mode.",
                        "default": "pty"
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Optional session timeout in seconds."
                    }
                },
                "required": ["slot_id", "runtime", "prompt"],
            }),
        },
        ToolDefinition {
            name: "cancel_session".to_string(),
            description: "Cancel a running or pending session.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "The session identifier to cancel."
                    }
                },
                "required": ["session_id"],
            }),
        },
        ToolDefinition {
            name: "list_sessions".to_string(),
            description: "List all sessions, optionally filtered by repository.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo_id": {
                        "type": "string",
                        "description": "Optional repository ID to filter by."
                    }
                },
            }),
        },
        ToolDefinition {
            name: "get_review_status".to_string(),
            description: "Get the review status including overlap detection between active slots."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo_id": {
                        "type": "string",
                        "description": "Optional repository ID to filter by."
                    }
                },
            }),
        },
        ToolDefinition {
            name: "get_session_log".to_string(),
            description: "Read the last N lines from a session's output log.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "The session identifier."
                    },
                    "lines": {
                        "type": "integer",
                        "description": "Number of lines to return from the end of the log.",
                        "default": 50
                    },
                    "stream": {
                        "type": "string",
                        "enum": ["stdout", "stderr"],
                        "description": "Which output stream to read.",
                        "default": "stdout"
                    }
                },
                "required": ["session_id"],
            }),
        },
        // ----- Team tools -----
        ToolDefinition {
            name: "list_teams".to_string(),
            description: "List all team manifests, optionally filtered by repository.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "repo_id": {
                        "type": "string",
                        "description": "Optional repository ID to filter by."
                    }
                },
            }),
        },
        ToolDefinition {
            name: "show_team".to_string(),
            description: "Load and display a team manifest by ID.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team_id": {
                        "type": "string",
                        "description": "The team identifier."
                    }
                },
                "required": ["team_id"],
            }),
        },
        ToolDefinition {
            name: "init_team".to_string(),
            description: "Initialize a new team manifest for a repository.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team_id": {
                        "type": "string",
                        "description": "The team identifier."
                    },
                    "repo_id": {
                        "type": "string",
                        "description": "The repository this team works on."
                    },
                    "objective": {
                        "type": "string",
                        "description": "The team's mission objective."
                    },
                    "force": {
                        "type": "boolean",
                        "description": "Overwrite existing manifest if present.",
                        "default": false
                    }
                },
                "required": ["team_id", "repo_id", "objective"],
            }),
        },
        ToolDefinition {
            name: "team_add_member".to_string(),
            description: "Add a member to an existing team.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team_id": {
                        "type": "string",
                        "description": "The team identifier."
                    },
                    "member_id": {
                        "type": "string",
                        "description": "Unique member identifier."
                    },
                    "role": {
                        "type": "string",
                        "enum": ["lead", "worker"],
                        "description": "The member's role.",
                        "default": "worker"
                    },
                    "runtime": {
                        "type": "string",
                        "enum": ["codex", "claude", "gemini", "shell"],
                        "description": "The runtime this member uses."
                    }
                },
                "required": ["team_id", "member_id", "runtime"],
            }),
        },
        ToolDefinition {
            name: "team_add_task".to_string(),
            description: "Add a task to an existing team.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team_id": {
                        "type": "string",
                        "description": "The team identifier."
                    },
                    "task_id": {
                        "type": "string",
                        "description": "Unique task identifier."
                    },
                    "title": {
                        "type": "string",
                        "description": "Human-readable task title."
                    },
                    "owner_id": {
                        "type": "string",
                        "description": "The member_id who owns this task."
                    },
                    "prompt": {
                        "type": "string",
                        "description": "The task prompt for the agent."
                    },
                    "deliverable": {
                        "type": "string",
                        "description": "What this task should produce."
                    },
                    "write_scope": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Files/directories this task may modify."
                    },
                    "verification": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Verification steps for the task."
                    },
                    "verification_command": {
                        "type": "string",
                        "description": "Optional shell command to verify task completion."
                    },
                    "depends_on": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Task IDs this task depends on."
                    }
                },
                "required": ["team_id", "task_id", "title", "owner_id", "prompt", "deliverable"],
            }),
        },
        ToolDefinition {
            name: "team_reset".to_string(),
            description:
                "Reset a team to planning state, clearing all task progress and slot bindings."
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team_id": {
                        "type": "string",
                        "description": "The team identifier."
                    },
                    "force": {
                        "type": "boolean",
                        "description": "Skip confirmation.",
                        "default": false
                    }
                },
                "required": ["team_id"],
            }),
        },
        ToolDefinition {
            name: "team_report".to_string(),
            description: "Generate a comprehensive markdown report of team activity and outcomes."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team_id": {
                        "type": "string",
                        "description": "The team identifier."
                    }
                },
                "required": ["team_id"],
            }),
        },
        ToolDefinition {
            name: "team_archive".to_string(),
            description: "Archive a team whose tasks are all in terminal states.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team_id": {
                        "type": "string",
                        "description": "The team identifier."
                    },
                    "force": {
                        "type": "boolean",
                        "description": "Skip confirmation.",
                        "default": false
                    }
                },
                "required": ["team_id"],
            }),
        },
        ToolDefinition {
            name: "team_delete".to_string(),
            description: "Permanently delete a team manifest.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "team_id": {
                        "type": "string",
                        "description": "The team identifier."
                    }
                },
                "required": ["team_id"],
            }),
        },
        ToolDefinition {
            name: "poll_events".to_string(),
            description: "Poll the event bus for new domain events. Returns events newer than since_seq. Use head_seq from the response as since_seq for the next poll.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "since_seq": {
                        "type": "integer",
                        "description": "Sequence number cursor. Returns events with seq > since_seq. Use 0 to get all buffered events.",
                        "default": 0
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of events to return.",
                        "default": 100
                    }
                },
            }),
        },
    ]
}

// ---------------------------------------------------------------------------
// Resource definitions
// ---------------------------------------------------------------------------

fn resource_definitions() -> Vec<ResourceDefinition> {
    vec![
        ResourceDefinition {
            uri: "awo://repos".to_string(),
            name: "Repository List".to_string(),
            description: "All registered Git repositories.".to_string(),
            mime_type: "application/json".to_string(),
        },
        ResourceDefinition {
            uri: "awo://slots".to_string(),
            name: "Slot List".to_string(),
            description: "All active worktree slots.".to_string(),
            mime_type: "application/json".to_string(),
        },
        ResourceDefinition {
            uri: "awo://sessions".to_string(),
            name: "Session List".to_string(),
            description: "All agent sessions.".to_string(),
            mime_type: "application/json".to_string(),
        },
        ResourceDefinition {
            uri: "awo://review".to_string(),
            name: "Review Status".to_string(),
            description: "Review and overlap detection status across all repos.".to_string(),
            mime_type: "application/json".to_string(),
        },
        ResourceDefinition {
            uri: "awo://teams".to_string(),
            name: "Team List".to_string(),
            description: "All team manifests.".to_string(),
            mime_type: "application/json".to_string(),
        },
    ]
}

// ---------------------------------------------------------------------------
// Tool → Command mapping
// ---------------------------------------------------------------------------

fn map_tool_to_command(
    tool_name: &str,
    args: &serde_json::Value,
) -> Result<awo_core::Command, String> {
    match tool_name {
        "list_repos" => Ok(awo_core::Command::RepoList),
        "acquire_slot" => {
            let repo_id = require_string(args, "repo_id")?;
            let task_name = require_string(args, "task_name")?;
            let strategy = match args.get("strategy").and_then(|v| v.as_str()) {
                Some("warm") => awo_core::SlotStrategy::Warm,
                _ => awo_core::SlotStrategy::Fresh,
            };
            Ok(awo_core::Command::SlotAcquire {
                repo_id,
                task_name,
                strategy,
            })
        }
        "release_slot" => {
            let slot_id = require_string(args, "slot_id")?;
            Ok(awo_core::Command::SlotRelease { slot_id })
        }
        "list_slots" => {
            let repo_id = optional_string(args, "repo_id");
            Ok(awo_core::Command::SlotList { repo_id })
        }
        "start_session" => {
            let slot_id = require_string(args, "slot_id")?;
            let runtime_str = require_string(args, "runtime")?;
            let runtime: awo_core::RuntimeKind = runtime_str
                .parse()
                .map_err(|_| format!("unknown runtime: {runtime_str}"))?;
            let prompt = require_string(args, "prompt")?;
            let read_only = args
                .get("read_only")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let dry_run = args
                .get("dry_run")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let launch_mode = match args.get("launch_mode").and_then(|v| v.as_str()) {
                Some("oneshot") => awo_core::SessionLaunchMode::Oneshot,
                _ => awo_core::SessionLaunchMode::Pty,
            };
            let timeout_secs = args
                .get("timeout_secs")
                .and_then(|v| v.as_u64())
                .map(|v| v as i64);
            Ok(awo_core::Command::SessionStart {
                slot_id,
                runtime,
                prompt,
                read_only,
                dry_run,
                launch_mode,
                attach_context: true,
                timeout_secs,
            })
        }
        "cancel_session" => {
            let session_id = require_string(args, "session_id")?;
            Ok(awo_core::Command::SessionCancel { session_id })
        }
        "list_sessions" => {
            let repo_id = optional_string(args, "repo_id");
            Ok(awo_core::Command::SessionList { repo_id })
        }
        "get_review_status" => {
            let repo_id = optional_string(args, "repo_id");
            Ok(awo_core::Command::ReviewStatus { repo_id })
        }
        "get_session_log" => {
            let session_id = require_string(args, "session_id")?;
            let lines = args
                .get("lines")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize);
            let stream = optional_string(args, "stream");
            Ok(awo_core::Command::SessionLog {
                session_id,
                lines,
                stream,
            })
        }
        "list_teams" => {
            let repo_id = optional_string(args, "repo_id");
            Ok(awo_core::Command::TeamList { repo_id })
        }
        "show_team" => {
            let team_id = require_string(args, "team_id")?;
            Ok(awo_core::Command::TeamShow { team_id })
        }
        "init_team" => {
            let team_id = require_string(args, "team_id")?;
            let repo_id = require_string(args, "repo_id")?;
            let objective = require_string(args, "objective")?;
            let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
            Ok(awo_core::Command::TeamInit {
                team_id,
                repo_id,
                objective,
                force,
            })
        }
        "team_add_member" => {
            let team_id = require_string(args, "team_id")?;
            let member_id = require_string(args, "member_id")?;
            let role = optional_string(args, "role").unwrap_or_else(|| "worker".to_string());
            let runtime = optional_string(args, "runtime");
            let member = awo_core::TeamMember {
                member_id,
                role,
                runtime,
                model: None,
                execution_mode: awo_core::TeamExecutionMode::ExternalSlots,
                slot_id: None,
                branch_name: None,
                read_only: false,
                write_scope: Vec::new(),
                context_packs: Vec::new(),
                skills: Vec::new(),
                notes: None,
                fallback_runtime: None,
                fallback_model: None,
                routing_preferences: None,
            };
            Ok(awo_core::Command::TeamMemberAdd { team_id, member })
        }
        "team_add_task" => {
            let team_id = require_string(args, "team_id")?;
            let task_id = require_string(args, "task_id")?;
            let title = require_string(args, "title")?;
            let owner_id = require_string(args, "owner_id")?;
            let prompt = require_string(args, "prompt")?;
            let deliverable = require_string(args, "deliverable")?;
            let write_scope = optional_string_array(args, "write_scope");
            let verification = optional_string_array(args, "verification");
            let verification_command = optional_string(args, "verification_command");
            let depends_on = optional_string_array(args, "depends_on");
            let task = awo_core::TaskCard {
                task_id,
                title,
                summary: prompt,
                owner_id,
                runtime: None,
                slot_id: None,
                branch_name: None,
                read_only: false,
                write_scope,
                deliverable,
                verification,
                verification_command,
                depends_on,
                state: awo_core::TaskCardState::Todo,
                result_summary: None,
                output_log_path: None,
            };
            Ok(awo_core::Command::TeamTaskAdd { team_id, task })
        }
        "team_reset" => {
            let team_id = require_string(args, "team_id")?;
            let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
            Ok(awo_core::Command::TeamReset { team_id, force })
        }
        "team_report" => {
            let team_id = require_string(args, "team_id")?;
            Ok(awo_core::Command::TeamReport { team_id })
        }
        "team_archive" => {
            let team_id = require_string(args, "team_id")?;
            let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
            Ok(awo_core::Command::TeamArchive { team_id, force })
        }
        "team_delete" => {
            let team_id = require_string(args, "team_id")?;
            Ok(awo_core::Command::TeamDelete { team_id })
        }
        "poll_events" => {
            let since_seq = args.get("since_seq").and_then(|v| v.as_u64());
            let limit = args
                .get("limit")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize);
            Ok(awo_core::Command::EventsPoll { since_seq, limit })
        }
        _ => Err(format!("unknown tool: {tool_name}")),
    }
}

fn require_string(args: &serde_json::Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("missing required argument: {key}"))
}

fn optional_string(args: &serde_json::Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn optional_string_array(args: &serde_json::Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use awo_core::commands::{Command, CommandOutcome};
    use awo_core::dispatch::Dispatcher;
    use awo_core::error::AwoResult;

    struct EchoDispatcher;
    impl Dispatcher for EchoDispatcher {
        fn dispatch(&mut self, command: Command) -> AwoResult<CommandOutcome> {
            Ok(CommandOutcome {
                summary: format!("executed: {}", command.method_name()),
                events: vec![],
            })
        }
    }

    fn make_server() -> McpServer {
        McpServer::new(Box::new(EchoDispatcher))
    }

    fn request(method: &str, params: serde_json::Value) -> JsonRpcMessage {
        JsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: Some(serde_json::json!(1)),
        }
    }

    fn notification(method: &str) -> JsonRpcMessage {
        JsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: serde_json::json!({}),
            id: None,
        }
    }

    #[test]
    fn initialize_returns_server_info() {
        let mut server = make_server();
        let msg = request("initialize", serde_json::json!({}));
        let resp = server.handle_message(&msg).unwrap();
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "awo-mcp");
        assert!(result["capabilities"]["tools"].is_object());
        assert!(result["capabilities"]["resources"].is_object());
    }

    #[test]
    fn ping_returns_empty_object() {
        let mut server = make_server();
        let msg = request("ping", serde_json::json!({}));
        let resp = server.handle_message(&msg).unwrap();
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap(), serde_json::json!({}));
    }

    #[test]
    fn notification_returns_no_response() {
        let mut server = make_server();
        let msg = notification("notifications/initialized");
        assert!(server.handle_message(&msg).is_none());
    }

    #[test]
    fn unknown_method_returns_error() {
        let mut server = make_server();
        let msg = request("bogus/method", serde_json::json!({}));
        let resp = server.handle_message(&msg).unwrap();
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, METHOD_NOT_FOUND);
    }

    #[test]
    fn tools_list_returns_all_tools() {
        let mut server = make_server();
        let msg = request("tools/list", serde_json::json!({}));
        let resp = server.handle_message(&msg).unwrap();
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert!(
            tools.len() >= 18,
            "expected at least 18 tools, got {}",
            tools.len()
        );

        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        // Slot/session tools
        assert!(names.contains(&"acquire_slot"));
        assert!(names.contains(&"release_slot"));
        assert!(names.contains(&"start_session"));
        assert!(names.contains(&"get_review_status"));
        assert!(names.contains(&"get_session_log"));
        assert!(names.contains(&"list_repos"));
        assert!(names.contains(&"list_slots"));
        assert!(names.contains(&"list_sessions"));
        assert!(names.contains(&"cancel_session"));
        // Team tools
        assert!(names.contains(&"list_teams"));
        assert!(names.contains(&"show_team"));
        assert!(names.contains(&"init_team"));
        assert!(names.contains(&"team_add_member"));
        assert!(names.contains(&"team_add_task"));
        assert!(names.contains(&"team_reset"));
        assert!(names.contains(&"team_report"));
        assert!(names.contains(&"team_archive"));
        assert!(names.contains(&"team_delete"));
    }

    #[test]
    fn tools_call_dispatches_command() {
        let mut server = make_server();
        let msg = request(
            "tools/call",
            serde_json::json!({
                "name": "list_repos",
                "arguments": {}
            }),
        );
        let resp = server.handle_message(&msg).unwrap();
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("executed: repo.list"));
        // isError should not be present (it's false and skip_serializing)
        assert!(result.get("isError").is_none());
    }

    #[test]
    fn tools_call_unknown_tool_returns_error_in_content() {
        let mut server = make_server();
        let msg = request(
            "tools/call",
            serde_json::json!({
                "name": "nonexistent_tool",
                "arguments": {}
            }),
        );
        let resp = server.handle_message(&msg).unwrap();
        let result = resp.result.unwrap();
        assert!(result["isError"].as_bool().unwrap());
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("unknown tool"));
    }

    #[test]
    fn tools_call_missing_required_arg_returns_error() {
        let mut server = make_server();
        let msg = request(
            "tools/call",
            serde_json::json!({
                "name": "acquire_slot",
                "arguments": {}
            }),
        );
        let resp = server.handle_message(&msg).unwrap();
        let result = resp.result.unwrap();
        assert!(result["isError"].as_bool().unwrap());
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("missing required argument: repo_id"));
    }

    #[test]
    fn tools_call_acquire_slot_maps_correctly() {
        let mut server = make_server();
        let msg = request(
            "tools/call",
            serde_json::json!({
                "name": "acquire_slot",
                "arguments": {
                    "repo_id": "my-repo",
                    "task_name": "fix-bug",
                    "strategy": "warm"
                }
            }),
        );
        let resp = server.handle_message(&msg).unwrap();
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("executed: slot.acquire"));
    }

    #[test]
    fn resources_list_returns_definitions() {
        let mut server = make_server();
        let msg = request("resources/list", serde_json::json!({}));
        let resp = server.handle_message(&msg).unwrap();
        let result = resp.result.unwrap();
        let resources = result["resources"].as_array().unwrap();
        assert!(resources.len() >= 5);

        let uris: Vec<&str> = resources
            .iter()
            .map(|r| r["uri"].as_str().unwrap())
            .collect();
        assert!(uris.contains(&"awo://repos"));
        assert!(uris.contains(&"awo://slots"));
        assert!(uris.contains(&"awo://sessions"));
        assert!(uris.contains(&"awo://review"));
        assert!(uris.contains(&"awo://teams"));
    }

    #[test]
    fn resources_read_dispatches_command() {
        let mut server = make_server();
        let msg = request("resources/read", serde_json::json!({"uri": "awo://repos"}));
        let resp = server.handle_message(&msg).unwrap();
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        let contents = result["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["uri"], "awo://repos");
        assert_eq!(contents[0]["mimeType"], "application/json");
    }

    #[test]
    fn resources_read_unknown_uri_returns_error() {
        let mut server = make_server();
        let msg = request(
            "resources/read",
            serde_json::json!({"uri": "awo://nonexistent"}),
        );
        let resp = server.handle_message(&msg).unwrap();
        assert!(resp.error.is_some());
        assert!(resp.error.unwrap().message.contains("unknown resource URI"));
    }

    #[test]
    fn map_tool_start_session_defaults() {
        let args = serde_json::json!({
            "slot_id": "slot-1",
            "runtime": "claude",
            "prompt": "Fix the bug"
        });
        let cmd = map_tool_to_command("start_session", &args).unwrap();
        if let awo_core::Command::SessionStart {
            read_only,
            dry_run,
            launch_mode,
            attach_context,
            ..
        } = cmd
        {
            assert!(!read_only);
            assert!(!dry_run);
            assert_eq!(launch_mode, awo_core::SessionLaunchMode::Pty);
            assert!(attach_context);
        } else {
            panic!("expected SessionStart");
        }
    }

    #[test]
    fn map_tool_invalid_runtime_returns_error() {
        let args = serde_json::json!({
            "slot_id": "slot-1",
            "runtime": "invalid_runtime",
            "prompt": "Fix it"
        });
        let result = map_tool_to_command("start_session", &args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown runtime"));
    }

    #[test]
    fn map_tool_list_teams_dispatches() {
        let mut server = make_server();
        let msg = request(
            "tools/call",
            serde_json::json!({
                "name": "list_teams",
                "arguments": {}
            }),
        );
        let resp = server.handle_message(&msg).unwrap();
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("executed: team.list"));
    }

    #[test]
    fn map_tool_init_team_dispatches() {
        let mut server = make_server();
        let msg = request(
            "tools/call",
            serde_json::json!({
                "name": "init_team",
                "arguments": {
                    "team_id": "alpha",
                    "repo_id": "my-repo",
                    "objective": "Fix all bugs"
                }
            }),
        );
        let resp = server.handle_message(&msg).unwrap();
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("executed: team.init"));
    }

    #[test]
    fn map_tool_team_add_member_dispatches() {
        let mut server = make_server();
        let msg = request(
            "tools/call",
            serde_json::json!({
                "name": "team_add_member",
                "arguments": {
                    "team_id": "alpha",
                    "member_id": "agent-1",
                    "runtime": "claude"
                }
            }),
        );
        let resp = server.handle_message(&msg).unwrap();
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("executed: team.member.add"));
    }

    #[test]
    fn map_tool_team_add_task_dispatches() {
        let mut server = make_server();
        let msg = request(
            "tools/call",
            serde_json::json!({
                "name": "team_add_task",
                "arguments": {
                    "team_id": "alpha",
                    "task_id": "task-1",
                    "title": "Fix the bug",
                    "owner_id": "agent-1",
                    "prompt": "Fix the login bug",
                    "deliverable": "A tested patch"
                }
            }),
        );
        let resp = server.handle_message(&msg).unwrap();
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("executed: team.task.add"));
    }

    #[test]
    fn map_tool_team_report_dispatches() {
        let mut server = make_server();
        let msg = request(
            "tools/call",
            serde_json::json!({
                "name": "team_report",
                "arguments": { "team_id": "alpha" }
            }),
        );
        let resp = server.handle_message(&msg).unwrap();
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("executed: team.report"));
    }

    #[test]
    fn map_tool_team_delete_dispatches() {
        let mut server = make_server();
        let msg = request(
            "tools/call",
            serde_json::json!({
                "name": "team_delete",
                "arguments": { "team_id": "alpha" }
            }),
        );
        let resp = server.handle_message(&msg).unwrap();
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("executed: team.delete"));
    }

    #[test]
    fn map_tool_team_add_task_missing_required_arg() {
        let args = serde_json::json!({
            "team_id": "alpha",
            "task_id": "task-1"
            // missing title, owner_id, prompt, deliverable
        });
        let result = map_tool_to_command("team_add_task", &args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing required argument"));
    }

    #[test]
    fn map_tool_poll_events_dispatches() {
        let mut server = make_server();
        let msg = request(
            "tools/call",
            serde_json::json!({
                "name": "poll_events",
                "arguments": { "since_seq": 0, "limit": 50 }
            }),
        );
        let resp = server.handle_message(&msg).unwrap();
        let result = resp.result.unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("executed: events.poll"));
    }

    #[test]
    fn map_tool_poll_events_defaults() {
        let cmd = map_tool_to_command("poll_events", &serde_json::json!({})).unwrap();
        assert_eq!(cmd.method_name(), "events.poll");
    }
}
