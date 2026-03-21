#![allow(unused_crate_dependencies)]

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

struct TestEnv {
    root: PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("awo-e2e-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&root).expect("failed to create test root");
        Self { root }
    }

    fn create_repo(&self, name: &str) -> PathBuf {
        let repo_dir = self.root.join("repos").join(name);
        std::fs::create_dir_all(&repo_dir).expect("failed to create repo dir");
        self.git(&repo_dir, &["init", "-b", "main"]);
        std::fs::write(repo_dir.join("README.md"), "hello\n").expect("failed to write README");
        self.git(&repo_dir, &["add", "README.md"]);
        self.git_with_identity(&repo_dir, &["commit", "-m", "init"]);
        repo_dir
    }

    fn git(&self, dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("failed to run git");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_with_identity(&self, dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=AWO Tests",
                "-c",
                "user.email=awo-tests@example.com",
            ])
            .args(args)
            .current_dir(dir)
            .output()
            .expect("failed to run git");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn awo(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_awo"));
        cmd.arg("--json");
        cmd.env("HOME", &self.root);
        cmd.env("XDG_CONFIG_HOME", self.root.join("config"));
        cmd.env("XDG_DATA_HOME", self.root.join("data"));
        #[cfg(windows)]
        {
            cmd.env("LOCALAPPDATA", self.root.join("local"));
            cmd.env("APPDATA", self.root.join("roaming"));
        }
        cmd
    }

    fn run(&self, args: &[&str]) -> Value {
        let mut cmd = self.awo();
        cmd.args(args);
        let output = cmd.output().expect("failed to run awo binary");
        let stdout = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(stdout.trim()).unwrap_or_else(|error| {
            panic!(
                "failed to parse JSON:\n{stdout}\nstderr: {}\nerror: {error}",
                String::from_utf8_lossy(&output.stderr)
            )
        })
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn repo_add_registers_and_appears_in_list() {
    let env = TestEnv::new();
    let repo_dir = env.create_repo("my-project");

    let add_result = env.run(&["repo", "add", repo_dir.to_str().expect("valid repo path")]);
    assert_eq!(add_result["ok"], true, "repo add failed: {add_result}");

    let data = add_result["data"]
        .as_array()
        .expect("data should be an array");
    assert_eq!(data.len(), 1, "should have exactly 1 repo after add");
    let repo = &data[0];
    assert_eq!(repo["name"], "my-project");
    assert_eq!(repo["default_base_branch"], "main");
    assert!(repo["id"].is_string());

    let list_result = env.run(&["repo", "list"]);
    assert_eq!(list_result["ok"], true);
    let repos = list_result["data"]
        .as_array()
        .expect("repo list data should be an array");
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0]["name"], "my-project");
    assert_eq!(repos[0]["id"], repo["id"]);
}

#[test]
fn repo_add_includes_repo_registered_event() {
    let env = TestEnv::new();
    let repo_dir = env.create_repo("evented-repo");

    let result = env.run(&["repo", "add", repo_dir.to_str().expect("valid repo path")]);
    let events = result["events"]
        .as_array()
        .expect("events should be an array");
    let registered = events
        .iter()
        .find(|event| event["type"].as_str() == Some("repo_registered"));
    assert!(
        registered.is_some(),
        "expected repo_registered event in {events:?}"
    );
    let event = registered.expect("repo_registered event should exist");
    assert_eq!(event["name"], "evented-repo");
    assert_eq!(event["default_base_branch"], "main");
}

#[test]
fn repo_add_non_git_directory_returns_error() {
    let env = TestEnv::new();
    let not_git = env.root.join("not-a-repo");
    std::fs::create_dir_all(&not_git).expect("failed to create non-git dir");

    let result = env.run(&["repo", "add", not_git.to_str().expect("valid path")]);
    assert_eq!(result["ok"], false);
    assert!(result["error"].is_string());
}

#[test]
fn repo_add_nonexistent_path_returns_error() {
    let env = TestEnv::new();
    let result = env.run(&["repo", "add", "/nonexistent/path/to/repo"]);
    assert_eq!(result["ok"], false);
    assert!(result["error"].is_string());
}

#[test]
fn team_init_creates_manifest_and_shows_it() {
    let env = TestEnv::new();
    let repo_dir = env.create_repo("team-project");
    let add_result = env.run(&["repo", "add", repo_dir.to_str().expect("valid repo path")]);
    let repo_id = add_result["data"][0]["id"]
        .as_str()
        .expect("repo id should be a string")
        .to_string();

    let init_result = env.run(&[
        "team",
        "init",
        &repo_id,
        "alpha-team",
        "Ship the feature",
        "--lead-runtime",
        "claude",
        "--lead-model",
        "sonnet",
    ]);
    assert_eq!(init_result["ok"], true, "team init failed: {init_result}");
    let manifest = &init_result["data"]["manifest"];
    assert_eq!(manifest["team_id"], "alpha-team");
    assert_eq!(manifest["repo_id"], repo_id.as_str());
    assert_eq!(manifest["objective"], "Ship the feature");
    assert_eq!(manifest["status"], "planning");
    assert_eq!(manifest["lead"]["runtime"], "claude");
    assert_eq!(manifest["lead"]["model"], "sonnet");
    assert!(
        init_result["data"]["manifest_path"].is_string(),
        "should include manifest path"
    );

    let show_result = env.run(&["team", "show", "alpha-team"]);
    assert_eq!(show_result["ok"], true);
    assert_eq!(show_result["data"]["team_id"], "alpha-team");
    assert_eq!(show_result["data"]["objective"], "Ship the feature");

    let list_result = env.run(&["team", "list"]);
    assert_eq!(list_result["ok"], true);
    let teams = list_result["data"]
        .as_array()
        .expect("team list data should be an array");
    assert_eq!(teams.len(), 1);
    assert_eq!(teams[0]["team_id"], "alpha-team");
}

#[test]
fn team_init_rejects_unknown_repo_id() {
    let env = TestEnv::new();
    let result = env.run(&[
        "team",
        "init",
        "nonexistent-repo",
        "my-team",
        "An objective",
    ]);
    assert_eq!(result["ok"], false);
    assert!(
        result["error"]
            .as_str()
            .expect("error should be a string")
            .contains("nonexistent-repo")
    );
}

#[test]
fn team_init_duplicate_without_force_fails() {
    let env = TestEnv::new();
    let repo_dir = env.create_repo("dup-team-repo");
    let add_result = env.run(&["repo", "add", repo_dir.to_str().expect("valid repo path")]);
    let repo_id = add_result["data"][0]["id"]
        .as_str()
        .expect("repo id should be a string")
        .to_string();

    let first = env.run(&["team", "init", &repo_id, "dup-team", "Objective"]);
    assert_eq!(first["ok"], true);

    let second = env.run(&["team", "init", &repo_id, "dup-team", "New objective"]);
    assert_eq!(second["ok"], false);
    assert!(
        second["error"]
            .as_str()
            .expect("error should be a string")
            .contains("already exists")
    );
}

#[test]
fn team_init_duplicate_with_force_succeeds() {
    let env = TestEnv::new();
    let repo_dir = env.create_repo("force-team-repo");
    let add_result = env.run(&["repo", "add", repo_dir.to_str().expect("valid repo path")]);
    let repo_id = add_result["data"][0]["id"]
        .as_str()
        .expect("repo id should be a string")
        .to_string();

    env.run(&["team", "init", &repo_id, "force-team", "Objective v1"]);
    let result = env.run(&[
        "team",
        "init",
        &repo_id,
        "force-team",
        "Objective v2",
        "--force",
    ]);
    assert_eq!(result["ok"], true);
    assert_eq!(result["data"]["manifest"]["objective"], "Objective v2");
}

#[test]
fn team_member_add_and_task_add_appear_in_show() {
    let env = TestEnv::new();
    let repo_dir = env.create_repo("member-task-repo");
    let add_result = env.run(&["repo", "add", repo_dir.to_str().expect("valid repo path")]);
    let repo_id = add_result["data"][0]["id"]
        .as_str()
        .expect("repo id should be a string")
        .to_string();

    env.run(&[
        "team",
        "init",
        &repo_id,
        "mt-team",
        "Test members and tasks",
        "--lead-runtime",
        "claude",
    ]);

    let member_result = env.run(&[
        "team",
        "member",
        "add",
        "mt-team",
        "worker-a",
        "implementer",
        "--runtime",
        "shell",
        "--write-scope",
        "src/lib.rs",
    ]);
    assert_eq!(
        member_result["ok"], true,
        "member add failed: {member_result}"
    );

    let task_result = env.run(&[
        "team",
        "task",
        "add",
        "mt-team",
        "task-1",
        "worker-a",
        "Implement feature",
        "Add the thing",
        "--deliverable",
        "A patch",
        "--verification",
        "cargo test",
        "--write-scope",
        "src/lib.rs",
    ]);
    assert_eq!(task_result["ok"], true, "task add failed: {task_result}");

    let show = env.run(&["team", "show", "mt-team"]);
    let manifest = &show["data"];
    let members = manifest["members"]
        .as_array()
        .expect("members should be an array");
    assert_eq!(members.len(), 1);
    assert_eq!(members[0]["member_id"], "worker-a");
    assert_eq!(members[0]["role"], "implementer");

    let tasks = manifest["tasks"]
        .as_array()
        .expect("tasks should be an array");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["task_id"], "task-1");
    assert_eq!(tasks[0]["owner_id"], "worker-a");
    assert_eq!(tasks[0]["state"], "todo");
}

#[test]
fn team_archive_requires_terminal_tasks() {
    let env = TestEnv::new();
    let repo_dir = env.create_repo("archive-repo");
    let add_result = env.run(&["repo", "add", repo_dir.to_str().expect("valid repo path")]);
    let repo_id = add_result["data"][0]["id"]
        .as_str()
        .expect("repo id should be a string")
        .to_string();

    env.run(&[
        "team",
        "init",
        &repo_id,
        "arc-team",
        "Test archive",
        "--lead-runtime",
        "claude",
    ]);
    env.run(&[
        "team",
        "member",
        "add",
        "arc-team",
        "worker-a",
        "implementer",
        "--runtime",
        "shell",
    ]);
    env.run(&[
        "team",
        "task",
        "add",
        "arc-team",
        "task-1",
        "worker-a",
        "Do thing",
        "Details",
        "--deliverable",
        "output",
    ]);

    let archive = env.run(&["team", "archive", "arc-team"]);
    assert_eq!(archive["ok"], false, "archive should fail with todo task");

    env.run(&["team", "task", "state", "arc-team", "task-1", "done"]);

    let archive = env.run(&["team", "archive", "arc-team"]);
    assert_eq!(archive["ok"], true, "archive should succeed: {archive}");
    assert_eq!(archive["data"]["status"], "archived");
}

#[test]
fn team_reset_clears_state_with_force() {
    let env = TestEnv::new();
    let repo_dir = env.create_repo("reset-repo");
    let add_result = env.run(&["repo", "add", repo_dir.to_str().expect("valid repo path")]);
    let repo_id = add_result["data"][0]["id"]
        .as_str()
        .expect("repo id should be a string")
        .to_string();

    env.run(&[
        "team",
        "init",
        &repo_id,
        "rst-team",
        "Test reset",
        "--lead-runtime",
        "claude",
    ]);
    env.run(&[
        "team",
        "member",
        "add",
        "rst-team",
        "worker-a",
        "implementer",
        "--runtime",
        "shell",
    ]);
    env.run(&[
        "team",
        "task",
        "add",
        "rst-team",
        "task-1",
        "worker-a",
        "Do thing",
        "Details",
        "--deliverable",
        "output",
    ]);

    env.run(&["team", "task", "state", "rst-team", "task-1", "in_progress"]);

    let preview = env.run(&["team", "reset", "rst-team"]);
    assert_eq!(preview["ok"], true);

    let reset = env.run(&["team", "reset", "rst-team", "--force"]);
    assert_eq!(reset["ok"], true, "reset --force should succeed: {reset}");
    assert_eq!(reset["data"]["status"], "planning");

    let show = env.run(&["team", "show", "rst-team"]);
    assert_eq!(show["data"]["tasks"][0]["state"], "todo");
}

#[test]
fn review_status_reports_zero_slots_after_repo_add() {
    let env = TestEnv::new();
    let repo_dir = env.create_repo("review-repo");
    env.run(&["repo", "add", repo_dir.to_str().expect("valid repo path")]);

    let review = env.run(&["review", "status"]);
    assert_eq!(review["ok"], true);
    let data = &review["data"];
    assert_eq!(data["active_slots"], 0);
    assert_eq!(data["dirty_slots"], 0);
    assert_eq!(data["pending_sessions"], 0);
    assert_eq!(data["completed_sessions"], 0);
    assert_eq!(data["failed_sessions"], 0);
}

#[test]
fn slot_list_with_repo_filter_returns_empty() {
    let env = TestEnv::new();
    let repo_dir = env.create_repo("slot-filter-repo");
    let add_result = env.run(&["repo", "add", repo_dir.to_str().expect("valid repo path")]);
    let repo_id = add_result["data"][0]["id"]
        .as_str()
        .expect("repo id should be a string")
        .to_string();

    let slots = env.run(&["slot", "list", "--repo-id", &repo_id]);
    assert_eq!(slots["ok"], true);
    let data = slots["data"].as_array().expect("data should be an array");
    assert!(data.is_empty(), "fresh repo should have no slots");
}

#[test]
fn session_list_with_repo_filter_returns_empty() {
    let env = TestEnv::new();
    let repo_dir = env.create_repo("sess-filter-repo");
    let add_result = env.run(&["repo", "add", repo_dir.to_str().expect("valid repo path")]);
    let repo_id = add_result["data"][0]["id"]
        .as_str()
        .expect("repo id should be a string")
        .to_string();

    let sessions = env.run(&["session", "list", "--repo-id", &repo_id]);
    assert_eq!(sessions["ok"], true);
    let data = sessions["data"]
        .as_array()
        .expect("data should be an array");
    assert!(data.is_empty(), "fresh repo should have no sessions");
}

#[test]
fn multiple_repos_each_appear_in_list() {
    let env = TestEnv::new();
    let repo_a = env.create_repo("project-alpha");
    let repo_b = env.create_repo("project-beta");

    env.run(&["repo", "add", repo_a.to_str().expect("valid repo path")]);
    env.run(&["repo", "add", repo_b.to_str().expect("valid repo path")]);

    let list = env.run(&["repo", "list"]);
    let repos = list["data"].as_array().expect("data should be an array");
    assert_eq!(repos.len(), 2);

    let names: Vec<&str> = repos
        .iter()
        .filter_map(|repo| repo["name"].as_str())
        .collect();
    assert!(
        names.contains(&"project-alpha"),
        "missing alpha in {names:?}"
    );
    assert!(names.contains(&"project-beta"), "missing beta in {names:?}");
}

#[test]
fn teams_from_different_repos_both_appear_in_list() {
    let env = TestEnv::new();
    let repo_a = env.create_repo("multi-team-a");
    let repo_b = env.create_repo("multi-team-b");

    let result_a = env.run(&["repo", "add", repo_a.to_str().expect("valid repo path")]);
    let id_a = result_a["data"][0]["id"]
        .as_str()
        .expect("repo id should be a string")
        .to_string();
    let result_b = env.run(&["repo", "add", repo_b.to_str().expect("valid repo path")]);
    let id_b = result_b["data"]
        .as_array()
        .expect("data should be an array")
        .iter()
        .find(|repo| repo["name"].as_str() == Some("multi-team-b"))
        .expect("repo should exist in repo add result")["id"]
        .as_str()
        .expect("repo id should be a string")
        .to_string();

    env.run(&["team", "init", &id_a, "team-a", "Objective A"]);
    env.run(&["team", "init", &id_b, "team-b", "Objective B"]);

    let list = env.run(&["team", "list"]);
    let teams = list["data"].as_array().expect("data should be an array");
    assert_eq!(teams.len(), 2);

    let team_ids: Vec<&str> = teams
        .iter()
        .filter_map(|team| team["team_id"].as_str())
        .collect();
    assert!(team_ids.contains(&"team-a"));
    assert!(team_ids.contains(&"team-b"));
}

#[test]
fn task_state_transitions_through_lifecycle() {
    let env = TestEnv::new();
    let repo_dir = env.create_repo("state-repo");
    let add_result = env.run(&["repo", "add", repo_dir.to_str().expect("valid repo path")]);
    let repo_id = add_result["data"][0]["id"]
        .as_str()
        .expect("repo id should be a string")
        .to_string();

    env.run(&[
        "team",
        "init",
        &repo_id,
        "lc-team",
        "Lifecycle test",
        "--lead-runtime",
        "claude",
    ]);
    env.run(&[
        "team",
        "member",
        "add",
        "lc-team",
        "worker-a",
        "implementer",
        "--runtime",
        "shell",
    ]);
    env.run(&[
        "team",
        "task",
        "add",
        "lc-team",
        "task-1",
        "worker-a",
        "Do work",
        "Details",
        "--deliverable",
        "output",
    ]);

    let in_progress = env.run(&["team", "task", "state", "lc-team", "task-1", "in_progress"]);
    assert_eq!(in_progress["ok"], true);
    assert_eq!(in_progress["data"]["tasks"][0]["state"], "in_progress");
    assert_eq!(in_progress["data"]["status"], "running");

    let review = env.run(&["team", "task", "state", "lc-team", "task-1", "review"]);
    assert_eq!(review["data"]["tasks"][0]["state"], "review");

    let done = env.run(&["team", "task", "state", "lc-team", "task-1", "done"]);
    assert_eq!(done["data"]["tasks"][0]["state"], "done");
    assert_eq!(done["data"]["status"], "complete");
}
