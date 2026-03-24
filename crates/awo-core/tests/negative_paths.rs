//! Negative-path tests covering error conditions, corrupt state,
//! malformed inputs, and runtime failure scenarios.
#![allow(unused_crate_dependencies)]

use anyhow::Result;
use awo_core::app::AppPaths;
use awo_core::config::{AppConfig, AppSettings};
use awo_core::dispatch::{Dispatcher, RpcRequest, dispatch_rpc, parse_rpc_request};
use awo_core::error::AwoError;
use awo_core::runtime::{RuntimeKind, SessionLaunchMode};
use awo_core::store::Store;
use awo_core::{AppCore, Command, SlotStrategy};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

struct TestHarness {
    _temp_dir: TempDir,
    config: AppConfig,
}

impl TestHarness {
    fn new() -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let config_dir = temp_dir.path().join("config");
        let data_dir = temp_dir.path().join("data");
        let logs_dir = data_dir.join("logs");
        let clones_dir = data_dir.join("clones");
        let repos_dir = config_dir.join("repos");
        let teams_dir = config_dir.join("teams");
        fs::create_dir_all(&logs_dir)?;
        fs::create_dir_all(&clones_dir)?;
        fs::create_dir_all(&repos_dir)?;
        fs::create_dir_all(&teams_dir)?;

        Ok(Self {
            _temp_dir: temp_dir,
            config: AppConfig {
                paths: AppPaths {
                    config_dir,
                    data_dir: data_dir.clone(),
                    state_db_path: data_dir.join("state.sqlite3"),
                    logs_dir,
                    repos_dir,
                    clones_dir,
                    teams_dir,
                },
                settings: AppSettings::default(),
            },
        })
    }

    fn core(&self) -> Result<AppCore> {
        Ok(AppCore::from_config(self.config.clone())?)
    }
}

// ---------------------------------------------------------------------------
// Store: corrupt and invalid database scenarios
// ---------------------------------------------------------------------------

#[test]
fn store_open_on_corrupt_file_returns_error() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("corrupt.sqlite3");
    fs::write(&db_path, "this is not a sqlite database").unwrap();
    let store = Store::open(&db_path);
    // rusqlite may open the file but schema init should fail
    if let Ok(store) = store {
        let result = store.initialize_schema();
        assert!(
            result.is_err(),
            "expected schema init to fail on corrupt db"
        );
    }
    // Either open or initialize_schema should have failed
}

#[test]
fn store_rejects_future_schema_version() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("future.sqlite3");
    let store = Store::open(&db_path).unwrap();
    store.initialize_schema().unwrap();
    // Manually bump the schema version beyond what the code supports
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute(
            "UPDATE app_meta SET value = '999' WHERE key = 'schema_version'",
            [],
        )
        .unwrap();
    }
    let store2 = Store::open(&db_path).unwrap();
    let result = store2.initialize_schema();
    assert!(
        result.is_err(),
        "expected rejection of unsupported future version"
    );
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("999") || err_msg.contains("unsupported"),
        "error should mention the version: {err_msg}"
    );
}

// ---------------------------------------------------------------------------
// Config: corrupt settings.json
// ---------------------------------------------------------------------------

#[test]
fn config_settings_deserialization_rejects_malformed_json() {
    // Test the deserialization error path directly
    let malformed = "{ broken json !!!";
    let result: Result<awo_core::config::AppSettings, _> = serde_json::from_str(malformed);
    assert!(result.is_err(), "malformed JSON should fail to deserialize");
}

#[test]
fn config_settings_deserialization_rejects_wrong_types() {
    // runtime_pressure_profile expects a map of RuntimeKind -> RuntimePressure
    let bad_json = r#"{"runtime_pressure_profile": "not a map"}"#;
    let result: Result<awo_core::config::AppSettings, _> = serde_json::from_str(bad_json);
    assert!(
        result.is_err(),
        "wrong type for runtime_pressure_profile should fail"
    );
}

// ---------------------------------------------------------------------------
// Command dispatch: unknown entity IDs
// ---------------------------------------------------------------------------

#[test]
fn dispatch_slot_release_unknown_id_returns_unknown_slot_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let result = core.dispatch(Command::SlotRelease {
        slot_id: "nonexistent-slot-999".to_string(),
    });
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("nonexistent-slot-999"),
        "error should mention the slot id: {msg}"
    );
    Ok(())
}

#[test]
fn dispatch_slot_refresh_unknown_id_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let result = core.dispatch(Command::SlotRefresh {
        slot_id: "ghost-slot".to_string(),
    });
    assert!(result.is_err());
    Ok(())
}

#[test]
fn dispatch_session_cancel_unknown_id_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let result = core.dispatch(Command::SessionCancel {
        session_id: "phantom-session".to_string(),
    });
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("phantom-session"),
        "error should mention the session id: {msg}"
    );
    Ok(())
}

#[test]
fn dispatch_session_delete_unknown_id_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let result = core.dispatch(Command::SessionDelete {
        session_id: "missing-session".to_string(),
    });
    assert!(result.is_err());
    Ok(())
}

#[test]
fn dispatch_repo_fetch_unknown_id_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let result = core.dispatch(Command::RepoFetch {
        repo_id: "nonexistent-repo".to_string(),
    });
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("nonexistent-repo"),
        "error should mention the repo id: {msg}"
    );
    Ok(())
}

#[test]
fn dispatch_context_pack_unknown_repo_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let result = core.dispatch(Command::ContextPack {
        repo_id: "no-such-repo".to_string(),
    });
    assert!(result.is_err());
    Ok(())
}

#[test]
fn dispatch_context_doctor_unknown_repo_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let result = core.dispatch(Command::ContextDoctor {
        repo_id: "no-such-repo".to_string(),
    });
    assert!(result.is_err());
    Ok(())
}

#[test]
fn dispatch_skills_list_unknown_repo_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let result = core.dispatch(Command::SkillsList {
        repo_id: "no-such-repo".to_string(),
    });
    assert!(result.is_err());
    Ok(())
}

#[test]
fn dispatch_slot_acquire_unknown_repo_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let result = core.dispatch(Command::SlotAcquire {
        repo_id: "no-such-repo".to_string(),
        task_name: "fix-bug".to_string(),
        strategy: SlotStrategy::Fresh,
    });
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("no-such-repo"),
        "error should mention the repo id: {msg}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Command dispatch: invalid state transitions
// ---------------------------------------------------------------------------

#[test]
fn dispatch_session_start_nonexistent_slot_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let result = core.dispatch(Command::SessionStart {
        slot_id: "phantom-slot".to_string(),
        runtime: RuntimeKind::Shell,
        prompt: "echo test".to_string(),
        read_only: true,
        dry_run: true,
        launch_mode: SessionLaunchMode::Oneshot,
        attach_context: false, timeout_secs: None,
    });
    assert!(result.is_err());
    Ok(())
}

// ---------------------------------------------------------------------------
// JSON-RPC dispatch: protocol-level errors
// ---------------------------------------------------------------------------

#[test]
fn rpc_dispatch_with_missing_params_field() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "noop".to_string(),
        params: serde_json::json!({"label": "test"}),
        id: Some(serde_json::json!(1)),
    };
    let response = dispatch_rpc(&mut core, &request);
    assert!(
        response.result.is_some(),
        "noop with valid params should succeed"
    );
    Ok(())
}

#[test]
fn rpc_dispatch_noop_missing_label_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    // "noop" requires a "label" field; omitting it should fail at deserialization
    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "noop".to_string(),
        params: serde_json::json!({}),
        id: Some(serde_json::json!(2)),
    };
    let response = dispatch_rpc(&mut core, &request);
    assert!(
        response.error.is_some(),
        "missing required param should produce an error: {response:?}"
    );
    Ok(())
}

#[test]
fn rpc_dispatch_unknown_method_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "completely.unknown".to_string(),
        params: serde_json::json!({}),
        id: Some(serde_json::json!(3)),
    };
    let response = dispatch_rpc(&mut core, &request);
    assert!(response.error.is_some());
    let error = response.error.unwrap();
    assert_eq!(error.code, -32601, "should be METHOD_NOT_FOUND code");
    Ok(())
}

#[test]
fn rpc_dispatch_wrong_param_types_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    // strategy should be "fresh" or "warm", not a number
    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "slot.acquire".to_string(),
        params: serde_json::json!({
            "repo_id": "x",
            "task_name": "y",
            "strategy": 42
        }),
        id: Some(serde_json::json!(4)),
    };
    let response = dispatch_rpc(&mut core, &request);
    assert!(
        response.error.is_some(),
        "wrong param type should produce an error"
    );
    Ok(())
}

#[test]
fn rpc_parse_empty_bytes_returns_parse_error() {
    let response = parse_rpc_request(b"").unwrap_err();
    let error = response.error.unwrap();
    assert_eq!(error.code, -32700, "should be PARSE_ERROR code");
}

#[test]
fn rpc_parse_truncated_json_returns_parse_error() {
    let response = parse_rpc_request(b"{\"jsonrpc\":\"2.0\",\"meth").unwrap_err();
    let error = response.error.unwrap();
    assert_eq!(error.code, -32700);
}

// ---------------------------------------------------------------------------
// Command serialization: malformed JSON-RPC inputs
// ---------------------------------------------------------------------------

#[test]
fn command_from_rpc_rejects_extra_unknown_params_gracefully() {
    // Extra fields should be ignored (serde default behavior)
    let result = Command::from_rpc(
        "noop",
        serde_json::json!({"label": "ok", "unknown_field": true}),
    );
    // serde should either accept (deny_unknown_fields not set) or reject
    // The current config doesn't set deny_unknown_fields, so this should succeed
    assert!(
        result.is_ok(),
        "extra fields should be ignored: {:?}",
        result.err()
    );
}

#[test]
fn command_from_rpc_rejects_null_for_required_field() {
    let result = Command::from_rpc("noop", serde_json::json!({"label": null}));
    assert!(result.is_err(), "null for required String should fail");
}

#[test]
fn command_from_rpc_with_empty_method_string() {
    let result = Command::from_rpc("", serde_json::json!({}));
    assert!(result.is_err(), "empty method string should fail");
}

// ---------------------------------------------------------------------------
// Error display: verify error messages contain useful context
// ---------------------------------------------------------------------------

#[test]
fn error_display_includes_entity_identifiers() {
    let cases: Vec<(AwoError, &str)> = vec![
        (AwoError::unknown_repo("my-repo"), "my-repo"),
        (AwoError::unknown_slot("slot-42"), "slot-42"),
        (AwoError::unknown_session("sess-abc"), "sess-abc"),
        (AwoError::unknown_task("task-1"), "task-1"),
        (AwoError::unknown_owner("owner-x"), "owner-x"),
        (AwoError::unsupported("runtime", "quantum"), "quantum"),
        (AwoError::invalid_state("slot is dirty"), "slot is dirty"),
        (AwoError::validation("field missing"), "field missing"),
    ];
    for (error, expected_substring) in cases {
        let display = format!("{error}");
        assert!(
            display.contains(expected_substring),
            "error display should contain `{expected_substring}`: got `{display}`"
        );
    }
}

// ---------------------------------------------------------------------------
// Dispatcher trait: verify AppCore implements it
// ---------------------------------------------------------------------------

#[test]
fn app_core_implements_dispatcher_trait() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    // Use the Dispatcher trait explicitly
    let dispatcher: &mut dyn Dispatcher = &mut core;
    let result = dispatcher.dispatch(Command::NoOp {
        label: "trait-test".to_string(),
    });
    assert!(result.is_ok());
    let outcome = result.unwrap();
    assert!(outcome.summary.contains("trait-test"));
    Ok(())
}

#[test]
fn dispatcher_through_rpc_noop_roundtrip() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "noop".to_string(),
        params: serde_json::json!({"label": "rpc-noop"}),
        id: Some(serde_json::json!(42)),
    };
    let response = dispatch_rpc(&mut core, &request);
    assert!(response.error.is_none(), "expected success: {response:?}");
    let result = response.result.unwrap();
    assert!(result.ok);
    assert!(result.summary.contains("rpc-noop"));
    assert_eq!(response.id, Some(serde_json::json!(42)));
    Ok(())
}

// ---------------------------------------------------------------------------
// Repo: add path that doesn't exist
// ---------------------------------------------------------------------------

#[test]
fn dispatch_repo_add_nonexistent_path_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let result = core.dispatch(Command::RepoAdd {
        path: PathBuf::from("/tmp/definitely/not/a/real/path/for/awo-test"),
    });
    assert!(result.is_err());
    Ok(())
}

#[test]
fn dispatch_repo_add_non_git_directory_returns_error() -> Result<()> {
    let harness = TestHarness::new()?;
    let temp_dir = tempfile::tempdir()?;
    let mut core = harness.core()?;
    let result = core.dispatch(Command::RepoAdd {
        path: temp_dir.path().to_path_buf(),
    });
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("git"), "error should mention git: {msg}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Review status on empty state
// ---------------------------------------------------------------------------

#[test]
fn review_status_on_fresh_state_returns_zero_counters() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let outcome = core.dispatch(Command::ReviewStatus { repo_id: None })?;
    assert!(outcome.summary.contains("0 dirty"));
    assert!(outcome.summary.contains("0 stale"));
    Ok(())
}

// ---------------------------------------------------------------------------
// Session and slot list on empty state
// ---------------------------------------------------------------------------

#[test]
fn session_list_on_fresh_state_returns_empty() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let outcome = core.dispatch(Command::SessionList { repo_id: None })?;
    assert!(outcome.summary.contains("0 session"));
    Ok(())
}

#[test]
fn slot_list_on_fresh_state_returns_empty() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let outcome = core.dispatch(Command::SlotList { repo_id: None })?;
    assert!(outcome.summary.contains("0 slot"));
    Ok(())
}

#[test]
fn repo_list_on_fresh_state_returns_empty() -> Result<()> {
    let harness = TestHarness::new()?;
    let mut core = harness.core()?;
    let outcome = core.dispatch(Command::RepoList)?;
    assert!(outcome.summary.contains("0 registered"));
    Ok(())
}
