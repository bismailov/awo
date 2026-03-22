#![allow(unused_crate_dependencies)]

use serde_json::Value;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestContext {
    root: std::path::PathBuf,
}

impl TestContext {
    fn new() -> Self {
        Self {
            root: unique_state_root(),
        }
    }

    fn run_awo(&self, args: &[&str], json: bool) -> Output {
        let mut cmd = awo();
        if json {
            cmd.arg("--json");
        }
        cmd.args(args);
        let config_dir = self.root.join("config");
        let data_dir = self.root.join("data");
        std::fs::create_dir_all(&config_dir).expect("test config dir should be creatable");
        std::fs::create_dir_all(&data_dir).expect("test data dir should be creatable");
        cmd.env("AWO_CONFIG_DIR", &config_dir);
        cmd.env("AWO_DATA_DIR", &data_dir);
        cmd.env("HOME", &self.root);
        cmd.env("XDG_CONFIG_HOME", &config_dir);
        cmd.env("XDG_DATA_HOME", &data_dir);
        #[cfg(windows)]
        {
            cmd.env("LOCALAPPDATA", self.root.join("local"));
            cmd.env("APPDATA", self.root.join("roaming"));
        }
        cmd.output().expect("awo binary should run")
    }

    fn run_awo_json(&self, args: &[&str]) -> (Output, Value) {
        let output = self.run_awo(args, true);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed = serde_json::from_str(stdout.trim()).unwrap_or_else(|error| {
            panic!(
                "failed to parse JSON from awo stdout:\n{stdout}\nstderr: {}\nerror: {error}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
        (output, parsed)
    }
}

fn awo() -> Command {
    Command::new(env!("CARGO_BIN_EXE_awo"))
}

fn unique_state_root() -> std::path::PathBuf {
    let suffix = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be monotonic")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "awo-app-json-{}-{nanos}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("test state root should be creatable");
    root
}

fn isolate_state_dir(cmd: &mut Command) {
    let root = unique_state_root();
    cmd.env("HOME", &root);
    cmd.env("XDG_CONFIG_HOME", root.join("config"));
    cmd.env("XDG_DATA_HOME", root.join("data"));
    #[cfg(windows)]
    {
        cmd.env("LOCALAPPDATA", root.join("local"));
        cmd.env("APPDATA", root.join("roaming"));
    }
}

fn run_awo(args: &[&str], json: bool) -> Output {
    let mut cmd = awo();
    if json {
        cmd.arg("--json");
    }
    cmd.args(args);
    isolate_state_dir(&mut cmd);
    cmd.output().expect("awo binary should run")
}

fn run_awo_json(args: &[&str]) -> (Output, Value) {
    let output = run_awo(args, true);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed = serde_json::from_str(stdout.trim()).unwrap_or_else(|error| {
        panic!(
            "failed to parse JSON from awo stdout:\n{stdout}\nstderr: {}\nerror: {error}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (output, parsed)
}

fn assert_unified_keys(json: &Value) {
    let object = json.as_object().expect("JSON envelope should be an object");
    let keys = object.keys().cloned().collect::<Vec<_>>();
    assert!(object.contains_key("ok"), "missing ok in {keys:?}");
    assert!(
        object.contains_key("summary"),
        "missing summary in {keys:?}"
    );
    assert!(object.contains_key("error"), "missing error in {keys:?}");
    assert!(object.contains_key("events"), "missing events in {keys:?}");
    assert!(object.contains_key("data"), "missing data in {keys:?}");
    assert_eq!(object.len(), 5, "unexpected top-level keys: {keys:?}");
}

#[test]
fn runtime_list_json_has_unified_success_envelope() {
    let (output, json) = run_awo_json(&["runtime", "list"]);

    assert!(output.status.success());
    assert_unified_keys(&json);
    assert_eq!(json["ok"], true);
    assert_eq!(json["summary"], Value::Null);
    assert_eq!(json["error"], Value::Null);
    assert!(
        json["events"]
            .as_array()
            .expect("events should be an array")
            .is_empty()
    );
    assert!(json["data"].is_array(), "data should be an array");
}

#[test]
fn runtime_list_json_contains_all_known_runtimes() {
    let (_output, json) = run_awo_json(&["runtime", "list"]);
    let runtimes = json["data"]
        .as_array()
        .expect("data should be an array")
        .iter()
        .filter_map(|entry| entry["runtime"].as_str())
        .collect::<Vec<_>>();

    for expected in ["codex", "claude", "gemini", "shell"] {
        assert!(
            runtimes.contains(&expected),
            "missing runtime `{expected}` in {runtimes:?}"
        );
    }
}

#[test]
fn runtime_show_json_returns_single_runtime() {
    let (_output, json) = run_awo_json(&["runtime", "show", "claude"]);

    assert_unified_keys(&json);
    assert_eq!(json["ok"], true);
    let data = json["data"].as_array().expect("data should be an array");
    assert_eq!(data.len(), 1);
    assert_eq!(data[0]["runtime"], "claude");
    assert!(data[0]["display_name"].is_string());
}

#[test]
fn runtime_show_invalid_returns_unified_error_envelope() {
    let (output, json) = run_awo_json(&["runtime", "show", "bogus"]);

    assert!(
        output.status.success(),
        "--json errors should still exit zero"
    );
    assert_unified_keys(&json);
    assert_eq!(json["ok"], false);
    assert_eq!(json["summary"], Value::Null);
    assert!(json["error"].is_string());
    assert!(
        json["events"]
            .as_array()
            .expect("events should be an array")
            .is_empty()
    );
    assert_eq!(json["data"], Value::Null);
}

#[test]
fn non_json_error_exits_nonzero() {
    let output = run_awo(&["runtime", "show", "bogus"], false);
    assert!(!output.status.success());
}

#[test]
fn debug_noop_json_contains_summary_and_events() {
    let (_output, json) = run_awo_json(&["debug", "noop", "--label", "json-test"]);

    assert_unified_keys(&json);
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"], Value::Null);
    assert!(json["summary"].is_string());
    let events = json["events"]
        .as_array()
        .expect("events should be an array");
    assert!(
        events
            .iter()
            .any(|event| event["type"] == "command_received"),
        "expected command_received in {events:?}"
    );
    let completed = events
        .iter()
        .find(|event| event["type"] == "no_op_completed")
        .expect("expected no_op_completed event");
    assert_eq!(completed["label"], "json-test");
}

#[test]
fn repo_list_json_is_empty_for_fresh_state() {
    let (_output, json) = run_awo_json(&["repo", "list"]);
    assert_eq!(json["ok"], true);
    assert!(
        json["data"]
            .as_array()
            .expect("data should be an array")
            .is_empty()
    );
}

#[test]
fn slot_list_json_is_empty_for_fresh_state() {
    let (_output, json) = run_awo_json(&["slot", "list"]);
    assert_eq!(json["ok"], true);
    assert!(
        json["data"]
            .as_array()
            .expect("data should be an array")
            .is_empty()
    );
}

#[test]
fn session_list_json_is_empty_for_fresh_state() {
    let (_output, json) = run_awo_json(&["session", "list"]);
    assert_eq!(json["ok"], true);
    assert!(
        json["data"]
            .as_array()
            .expect("data should be an array")
            .is_empty()
    );
}

#[test]
fn team_list_json_is_empty_for_fresh_state() {
    let (_output, json) = run_awo_json(&["team", "list"]);
    assert_eq!(json["ok"], true);
    assert!(
        json["data"]
            .as_array()
            .expect("data should be an array")
            .is_empty()
    );
}

#[test]
fn review_status_json_returns_summary_object() {
    let (_output, json) = run_awo_json(&["review", "status"]);
    assert_eq!(json["ok"], true);
    let data = &json["data"];
    assert!(data["active_slots"].is_number());
    assert!(data["dirty_slots"].is_number());
    assert!(data["pending_sessions"].is_number());
}

#[test]
fn session_cancel_unknown_id_returns_json_error() {
    let (_output, json) = run_awo_json(&["session", "cancel", "missing-session"]);
    assert_eq!(json["ok"], false);
    assert!(json["error"].is_string());
}

#[test]
fn slot_release_unknown_id_returns_json_error() {
    let (_output, json) = run_awo_json(&["slot", "release", "missing-slot"]);
    assert_eq!(json["ok"], false);
    assert!(json["error"].is_string());
}

#[test]
fn team_show_unknown_id_returns_json_error() {
    let (_output, json) = run_awo_json(&["team", "show", "missing-team"]);
    assert_eq!(json["ok"], false);
    assert!(json["error"].is_string());
}

#[test]
fn runtime_list_json_entries_include_budget_fields() {
    let (_output, json) = run_awo_json(&["runtime", "list"]);
    let entries = json["data"].as_array().expect("data should be an array");
    assert!(
        !entries.is_empty(),
        "expected at least one runtime capability"
    );
    for entry in entries {
        assert!(entry["cost_tier"].is_string());
        assert!(entry["limit_profile"].is_string());
        assert!(entry["operator_note"].is_string());
        assert!(entry["notes"].is_array());
    }
}

#[test]
fn runtime_show_json_includes_budget_metadata_and_notes() {
    let (_output, json) = run_awo_json(&["runtime", "show", "claude"]);
    let entry = &json["data"][0];
    assert_eq!(entry["runtime"], "claude");
    assert_eq!(entry["cost_tier"], "premium");
    assert_eq!(entry["limit_profile"], "api_metered");
    assert!(entry["operator_note"].is_string());
    assert!(
        !entry["notes"]
            .as_array()
            .expect("notes should be an array")
            .is_empty()
    );
}

#[test]
fn runtime_show_claude_has_native_team_capabilities() {
    let (_output, json) = run_awo_json(&["runtime", "show", "claude"]);
    let entry = &json["data"][0];
    assert_eq!(entry["inline_subagents"], "native");
    assert_eq!(entry["multi_session_teams"], "native");
    assert_eq!(entry["skill_preload"], "native");
}

#[test]
fn runtime_route_preview_primary_selected_by_default() {
    let (output, json) = run_awo_json(&[
        "runtime",
        "route-preview",
        "--primary",
        "claude",
        "--fallback-runtime",
        "gemini",
    ]);

    assert!(output.status.success());
    assert_unified_keys(&json);
    assert_eq!(json["ok"], true);
    let data = &json["data"];
    assert_eq!(data["selected_runtime"], "claude");
    assert_eq!(data["source"], "primary");
    assert!(
        data["reason"]
            .as_str()
            .unwrap()
            .contains("meets all routing preferences")
    );
}

#[test]
fn runtime_route_preview_fallback_selected_when_primary_exceeds_cost() {
    let (output, json) = run_awo_json(&[
        "runtime",
        "route-preview",
        "--primary",
        "claude",
        "--fallback-runtime",
        "gemini",
        "--max-cost-tier",
        "standard",
    ]);

    assert!(output.status.success());
    assert_unified_keys(&json);
    assert_eq!(json["ok"], true);
    let data = &json["data"];
    assert_eq!(data["selected_runtime"], "gemini");
    assert_eq!(data["source"], "fallback");
    assert!(
        data["reason"]
            .as_str()
            .unwrap()
            .contains("primary rejected")
    );
}

#[test]
fn runtime_route_preview_invalid_runtime_returns_json_error() {
    let (output, json) = run_awo_json(&["runtime", "route-preview", "--primary", "bogus"]);

    assert!(output.status.success());
    assert_unified_keys(&json);
    assert_eq!(json["ok"], false);
    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("Matching variant not found")
    );
}

#[test]
fn runtime_route_preview_no_fallback_honors_flag() {
    let (output, json) = run_awo_json(&[
        "runtime",
        "route-preview",
        "--primary",
        "claude",
        "--fallback-runtime",
        "gemini",
        "--max-cost-tier",
        "standard",
        "--no-fallback",
    ]);

    assert!(output.status.success());
    assert_unified_keys(&json);
    assert_eq!(json["ok"], true);
    let data = &json["data"];
    assert_eq!(data["selected_runtime"], "claude");
    assert_eq!(data["source"], "primary");
    assert!(
        data["reason"]
            .as_str()
            .unwrap()
            .contains("fallback was not allowed")
    );
}

#[test]
fn runtime_route_preview_soft_limit_pressure_keeps_primary() {
    let (output, json) = run_awo_json(&[
        "runtime",
        "route-preview",
        "--primary",
        "claude",
        "--fallback-runtime",
        "gemini",
        "--pressure",
        "claude=soft_limit",
    ]);

    assert!(output.status.success());
    assert_unified_keys(&json);
    assert_eq!(json["ok"], true);
    let data = &json["data"];
    assert_eq!(data["selected_runtime"], "claude");
    assert_eq!(data["source"], "primary");
    assert!(data["reason"].as_str().unwrap().contains("soft_limit"));
}

#[test]
fn runtime_route_preview_hard_limit_pressure_prefers_fallback() {
    let (output, json) = run_awo_json(&[
        "runtime",
        "route-preview",
        "--primary",
        "claude",
        "--fallback-runtime",
        "gemini",
        "--pressure",
        "claude=hard_limit",
    ]);

    assert!(output.status.success());
    assert_unified_keys(&json);
    assert_eq!(json["ok"], true);
    let data = &json["data"];
    assert_eq!(data["selected_runtime"], "gemini");
    assert_eq!(data["source"], "fallback");
    assert!(data["reason"].as_str().unwrap().contains("hard limit"));
}

#[test]
fn runtime_route_preview_rejects_malformed_pressure_value() {
    let (output, json) = run_awo_json(&[
        "runtime",
        "route-preview",
        "--primary",
        "claude",
        "--pressure",
        "claude",
    ]);

    assert!(output.status.success());
    assert_unified_keys(&json);
    assert_eq!(json["ok"], false);
    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("expected runtime=level")
    );
}

#[test]
fn runtime_pressure_set_and_list_persists_locally() {
    let test_context = TestContext::new();

    let (output, json) =
        test_context.run_awo_json(&["runtime", "pressure", "set", "claude", "soft_limit"]);
    assert!(output.status.success());
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["claude"], "soft_limit");

    let (_output, json) = test_context.run_awo_json(&["runtime", "pressure", "list"]);
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["claude"], "soft_limit");

    let (_output, json) =
        test_context.run_awo_json(&["runtime", "pressure", "set", "gemini", "hard_limit"]);
    assert_eq!(json["ok"], true);
    assert_eq!(json["data"]["claude"], "soft_limit");
    assert_eq!(json["data"]["gemini"], "hard_limit");
}

#[test]
fn runtime_pressure_clear_removes_entry() {
    let test_context = TestContext::new();

    let (_output, json) =
        test_context.run_awo_json(&["runtime", "pressure", "set", "claude", "soft_limit"]);
    assert_eq!(json["ok"], true);

    let (_output, json) = test_context.run_awo_json(&["runtime", "pressure", "clear", "claude"]);
    assert_eq!(json["ok"], true);
    assert!(json["data"].as_object().expect("object data").is_empty());

    let (_output, json) = test_context.run_awo_json(&["runtime", "pressure", "list"]);
    assert_eq!(json["ok"], true);
    assert!(json["data"].as_object().expect("object data").is_empty());
}

#[test]
fn runtime_route_preview_uses_stored_pressure_by_default() {
    let test_context = TestContext::new();

    let (_output, json) =
        test_context.run_awo_json(&["runtime", "pressure", "set", "claude", "hard_limit"]);
    assert_eq!(json["ok"], true);

    let (output, json) = test_context.run_awo_json(&[
        "runtime",
        "route-preview",
        "--primary",
        "claude",
        "--fallback-runtime",
        "gemini",
    ]);

    assert!(output.status.success());
    assert_unified_keys(&json);
    assert_eq!(json["ok"], true);
    let data = &json["data"];
    assert_eq!(data["selected_runtime"], "gemini");
    assert_eq!(data["source"], "fallback");
    assert!(data["reason"].as_str().unwrap().contains("hard limit"));
}

#[test]
fn runtime_route_preview_cli_pressure_overrides_stored() {
    let test_context = TestContext::new();

    let (_output, json) =
        test_context.run_awo_json(&["runtime", "pressure", "set", "claude", "soft_limit"]);
    assert_eq!(json["ok"], true);

    let (output, json) = test_context.run_awo_json(&[
        "runtime",
        "route-preview",
        "--primary",
        "claude",
        "--fallback-runtime",
        "gemini",
        "--pressure",
        "claude=hard_limit",
    ]);

    assert!(output.status.success());
    assert_unified_keys(&json);
    assert_eq!(json["ok"], true);
    let data = &json["data"];
    assert_eq!(data["selected_runtime"], "gemini");
    assert_eq!(data["source"], "fallback");
    assert!(data["reason"].as_str().unwrap().contains("hard limit"));
}
