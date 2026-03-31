#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


@dataclass
class StepResult:
    name: str
    command: list[str]
    cwd: str
    exit_code: int
    stdout: str
    stderr: str
    status: str
    evidence: str


def repo_root() -> Path:
    return Path(__file__).resolve().parent.parent


def binary_name(name: str) -> str:
    return f"{name}.exe" if os.name == "nt" else name


def sanitize_output(text: str) -> str:
    return text.rstrip()


def run_command(
    name: str,
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str],
    input_text: str | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    print(f"[smoke] {name}: {' '.join(command)}")
    result = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        input=input_text,
        text=True,
        capture_output=True,
        check=False,
    )
    if check and result.returncode != 0:
        raise RuntimeError(
            f"{name} failed with exit code {result.returncode}\n"
            f"stdout:\n{sanitize_output(result.stdout)}\n"
            f"stderr:\n{sanitize_output(result.stderr)}"
        )
    return result


def json_payload(result: subprocess.CompletedProcess[str], step_name: str) -> Any:
    if not result.stdout.strip():
        raise RuntimeError(
            f"{step_name} returned empty stdout\nstderr:\n{sanitize_output(result.stderr)}"
        )
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(
            f"{step_name} returned invalid JSON: {error}\nstdout:\n{sanitize_output(result.stdout)}"
        ) from error


def data_id(payload: Any, step_name: str) -> str:
    data = payload.get("data")
    if data is None:
        raise RuntimeError(f"{step_name} JSON payload contained null data: {payload}")
    if isinstance(data, list):
        if not data:
            raise RuntimeError(f"{step_name} JSON payload contained an empty data array")
        first = data[0]
        if "id" not in first:
            raise RuntimeError(f"{step_name} first JSON data item had no id: {payload}")
        return str(first["id"])
    if isinstance(data, dict) and "id" in data:
        return str(data["id"])
    raise RuntimeError(f"{step_name} JSON payload did not contain a usable id: {payload}")


def ensure_contains(result: subprocess.CompletedProcess[str], expected: str, step_name: str) -> None:
    haystack = f"{result.stdout}\n{result.stderr}"
    if expected not in haystack:
        raise RuntimeError(
            f"{step_name} did not contain expected text `{expected}`\n"
            f"stdout:\n{sanitize_output(result.stdout)}\n"
            f"stderr:\n{sanitize_output(result.stderr)}"
        )


def append_result(
    results: list[StepResult],
    *,
    name: str,
    command: list[str],
    cwd: Path,
    completed: subprocess.CompletedProcess[str],
    evidence: str,
    status: str | None = None,
) -> None:
    results.append(
        StepResult(
            name=name,
            command=command,
            cwd=str(cwd),
            exit_code=completed.returncode,
            stdout=sanitize_output(completed.stdout),
            stderr=sanitize_output(completed.stderr),
            status=status or ("PASS" if completed.returncode == 0 else "FAIL"),
            evidence=evidence,
        )
    )


def platform_label() -> str:
    bits = platform.architecture()[0]
    return f"{platform.system()} {platform.release()} ({bits})"


def build_env(state_root: Path) -> dict[str, str]:
    env = os.environ.copy()
    for key in ("AWO_CONFIG_DIR", "AWO_DATA_DIR", "AWO_CLONES_DIR", "AWO_WORKTREES_DIR"):
        env.pop(key, None)

    config_dir = state_root / "config"
    data_dir = state_root / "data"
    clones_dir = state_root / "clones"
    worktrees_dir = state_root / "worktrees"
    for path in (config_dir, data_dir, clones_dir, worktrees_dir):
        path.mkdir(parents=True, exist_ok=True)

    env["AWO_CONFIG_DIR"] = str(config_dir)
    env["AWO_DATA_DIR"] = str(data_dir)
    env["AWO_CLONES_DIR"] = str(clones_dir)
    env["AWO_WORKTREES_DIR"] = str(worktrees_dir)
    env["HOME"] = str(state_root / "home")
    (state_root / "home").mkdir(parents=True, exist_ok=True)

    if os.name == "nt":
        local = state_root / "localappdata"
        roaming = state_root / "appdata"
        local.mkdir(parents=True, exist_ok=True)
        roaming.mkdir(parents=True, exist_ok=True)
        env["LOCALAPPDATA"] = str(local)
        env["APPDATA"] = str(roaming)
    else:
        xdg_config = state_root / "xdg-config"
        xdg_data = state_root / "xdg-data"
        xdg_config.mkdir(parents=True, exist_ok=True)
        xdg_data.mkdir(parents=True, exist_ok=True)
        env["XDG_CONFIG_HOME"] = str(xdg_config)
        env["XDG_DATA_HOME"] = str(xdg_data)

    return env


def write_reports(
    *,
    results: list[StepResult],
    report_json: Path | None,
    report_md: Path | None,
    smoke_repo: Path,
    profile: str,
) -> None:
    summary = {
        "pass": sum(1 for result in results if result.status == "PASS"),
        "fail": sum(1 for result in results if result.status != "PASS"),
    }
    payload = {
        "generated_at": datetime.now(timezone.utc).astimezone().isoformat(),
        "platform": platform_label(),
        "repo_root": str(repo_root()),
        "smoke_repo": str(smoke_repo),
        "profile": profile,
        "summary": summary,
        "results": [
            {
                "name": result.name,
                "status": result.status,
                "evidence": result.evidence,
                "command": result.command,
                "cwd": result.cwd,
            }
            for result in results
        ],
    }

    if report_json is not None:
        report_json.parent.mkdir(parents=True, exist_ok=True)
        report_json.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    if report_md is not None:
        report_md.parent.mkdir(parents=True, exist_ok=True)
        lines = [
            "# Awo Smoke Report",
            "",
            f"- Generated: {payload['generated_at']}",
            f"- Platform: {payload['platform']}",
            f"- Profile: `{profile}`",
            f"- Repo root: `{payload['repo_root']}`",
            f"- Smoke repo: `{payload['smoke_repo']}`",
            "",
            "## Summary",
            "",
            f"- Passed: {summary['pass']}",
            f"- Failed: {summary['fail']}",
            "",
            "## Results",
            "",
        ]
        for result in results:
            lines.extend(
                [
                    f"### {result.name}",
                    f"- Status: `{result.status}`",
                    f"- Evidence: {result.evidence}",
                    f"- Command: `{ ' '.join(result.command) }`",
                    "",
                ]
            )
        report_md.write_text("\n".join(lines), encoding="utf-8")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run the core Awo Console smoke workflow against built binaries.",
    )
    parser.add_argument(
        "--profile",
        default="debug",
        choices=("debug", "release"),
        help="Cargo profile whose binaries should be exercised.",
    )
    parser.add_argument(
        "--binary-dir",
        type=Path,
        help="Directory containing the built binaries. Defaults to target/<profile>.",
    )
    parser.add_argument(
        "--report-json",
        type=Path,
        help="Optional path for a machine-readable smoke report.",
    )
    parser.add_argument(
        "--report-md",
        type=Path,
        help="Optional path for a markdown smoke report.",
    )
    parser.add_argument(
        "--keep-temp",
        action="store_true",
        help="Keep the temporary smoke state directory instead of deleting it.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = repo_root()
    binary_dir = (args.binary_dir or (root / "target" / args.profile)).resolve()
    awo = binary_dir / binary_name("awo")
    awod = binary_dir / binary_name("awod")
    awo_mcp = binary_dir / binary_name("awo-mcp")
    for binary in (awo, awod, awo_mcp):
        if not binary.exists():
            raise RuntimeError(
                f"required binary `{binary}` does not exist; build it before running smoke"
            )

    temp_root = Path(tempfile.mkdtemp(prefix="awo-smoke-"))
    state_root = temp_root / "state"
    smoke_repo = temp_root / "repo"
    results: list[StepResult] = []
    env = build_env(state_root)
    daemon_proc: subprocess.Popen[str] | None = None

    try:
        smoke_repo.mkdir(parents=True, exist_ok=True)

        init_repo_cmd = [
            "git",
            "init",
            "-b",
            "main",
            str(smoke_repo),
        ]
        init_repo = run_command("git init", init_repo_cmd, cwd=root, env=env)
        append_result(
            results,
            name="git_init",
            command=init_repo_cmd,
            cwd=root,
            completed=init_repo,
            evidence="Initialized the isolated smoke repository.",
        )
        (smoke_repo / "README.md").write_text("# Awo Smoke Repo\n", encoding="utf-8")
        git_add = run_command("git add", ["git", "add", "README.md"], cwd=smoke_repo, env=env)
        append_result(
            results,
            name="git_add",
            command=["git", "add", "README.md"],
            cwd=smoke_repo,
            completed=git_add,
            evidence="Staged the smoke repository seed file.",
        )
        git_commit_cmd = [
            "git",
            "-c",
            "user.name=Awo Smoke",
            "-c",
            "user.email=smoke@example.com",
            "commit",
            "-m",
            "init",
        ]
        git_commit = run_command("git commit", git_commit_cmd, cwd=smoke_repo, env=env)
        append_result(
            results,
            name="git_commit",
            command=git_commit_cmd,
            cwd=smoke_repo,
            completed=git_commit,
            evidence="Created the initial commit for the isolated smoke repo.",
        )

        repo_add_cmd = [str(awo), "--json", "repo", "add", str(smoke_repo)]
        repo_add = run_command("repo add", repo_add_cmd, cwd=root, env=env)
        repo_id = data_id(json_payload(repo_add, "repo add"), "repo add")
        append_result(
            results,
            name="repo_add",
            command=repo_add_cmd,
            cwd=root,
            completed=repo_add,
            evidence=f"Registered the smoke repo as `{repo_id}`.",
        )

        for name, command, evidence in [
            (
                "repo_list",
                [str(awo), "repo", "list"],
                f"`repo list` returned the registered repo `{repo_id}`.",
            ),
            (
                "context_doctor",
                [str(awo), "context", "doctor", repo_id],
                "Context doctor completed successfully for the isolated repo.",
            ),
            (
                "skills_doctor",
                [str(awo), "skills", "doctor", repo_id],
                "Skills doctor completed successfully for the isolated repo.",
            ),
            (
                "runtime_list",
                [str(awo), "runtime", "list"],
                "Runtime list completed successfully.",
            ),
        ]:
            completed = run_command(name, command, cwd=root, env=env)
            append_result(
                results,
                name=name,
                command=command,
                cwd=root,
                completed=completed,
                evidence=evidence,
            )

        slot_acquire_cmd = [
            str(awo),
            "--json",
            "slot",
            "acquire",
            repo_id,
            "warm-smoke",
            "--strategy",
            "warm",
        ]
        slot_acquire = run_command("slot acquire", slot_acquire_cmd, cwd=root, env=env)
        slot_id = data_id(json_payload(slot_acquire, "slot acquire"), "slot acquire")
        append_result(
            results,
            name="slot_acquire",
            command=slot_acquire_cmd,
            cwd=root,
            completed=slot_acquire,
            evidence=f"Acquired warm slot `{slot_id}`.",
        )

        for name, command, evidence in [
            (
                "slot_release",
                [str(awo), "slot", "release", slot_id],
                f"Released warm slot `{slot_id}`.",
            ),
            (
                "slot_delete",
                [str(awo), "slot", "delete", slot_id],
                f"Deleted released warm slot `{slot_id}`.",
            ),
        ]:
            completed = run_command(name, command, cwd=root, env=env)
            append_result(
                results,
                name=name,
                command=command,
                cwd=root,
                completed=completed,
                evidence=evidence,
            )

        session_slot_cmd = [
            str(awo),
            "--json",
            "slot",
            "acquire",
            repo_id,
            "session-smoke",
            "--strategy",
            "warm",
        ]
        session_slot = run_command("session slot acquire", session_slot_cmd, cwd=root, env=env)
        session_slot_id = data_id(
            json_payload(session_slot, "session slot acquire"),
            "session slot acquire",
        )
        append_result(
            results,
            name="session_slot_acquire",
            command=session_slot_cmd,
            cwd=root,
            completed=session_slot,
            evidence=f"Acquired session slot `{session_slot_id}`.",
        )

        session_start_cmd = [
            str(awo),
            "--json",
            "session",
            "start",
            session_slot_id,
            "shell",
            "echo 'hello from awo smoke'; pwd",
            "--read-only",
        ]
        session_start = run_command("session start", session_start_cmd, cwd=root, env=env)
        session_id = data_id(json_payload(session_start, "session start"), "session start")
        append_result(
            results,
            name="session_start",
            command=session_start_cmd,
            cwd=root,
            completed=session_start,
            evidence=f"Started shell smoke session `{session_id}`.",
        )

        session_log_cmd = [str(awo), "session", "log", session_id]
        session_log = run_command("session log", session_log_cmd, cwd=root, env=env)
        ensure_contains(session_log, "hello from awo smoke", "session log")
        append_result(
            results,
            name="session_log",
            command=session_log_cmd,
            cwd=root,
            completed=session_log,
            evidence=f"`session log` for `{session_id}` contained the smoke marker text.",
        )

        for name, command, evidence in [
            (
                "session_slot_release",
                [str(awo), "slot", "release", session_slot_id],
                f"Released session slot `{session_slot_id}`.",
            ),
            (
                "session_slot_delete",
                [str(awo), "slot", "delete", session_slot_id],
                f"Deleted session slot `{session_slot_id}` after log inspection.",
            ),
        ]:
            completed = run_command(name, command, cwd=root, env=env)
            append_result(
                results,
                name=name,
                command=command,
                cwd=root,
                completed=completed,
                evidence=evidence,
            )

        daemon_status_before_cmd = [str(awo), "daemon", "status"]
        daemon_status_before = run_command(
            "daemon status before",
            daemon_status_before_cmd,
            cwd=root,
            env=env,
            check=False,
        )
        daemon_status_text = f"{daemon_status_before.stdout}\n{daemon_status_before.stderr}".lower()
        daemon_preexisting = "not running" not in daemon_status_text
        append_result(
            results,
            name="daemon_status_before",
            command=daemon_status_before_cmd,
            cwd=root,
            completed=daemon_status_before,
            evidence=(
                "Daemon status reported `not running` before explicit startup."
                if not daemon_preexisting
                else "Daemon was already active in isolated state before the explicit lifecycle check."
            ),
            status="PASS",
        )

        if daemon_preexisting:
            daemon_stop_existing_cmd = [str(awo), "daemon", "stop"]
            daemon_stop_existing = run_command(
                "daemon stop existing",
                daemon_stop_existing_cmd,
                cwd=root,
                env=env,
            )
            append_result(
                results,
                name="daemon_stop_existing",
                command=daemon_stop_existing_cmd,
                cwd=root,
                completed=daemon_stop_existing,
                evidence="Stopped the preexisting daemon so the explicit lifecycle check starts cleanly.",
            )
            daemon_status_reset = run_command(
                "daemon status reset",
                daemon_status_before_cmd,
                cwd=root,
                env=env,
                check=False,
            )
            ensure_contains(daemon_status_reset, "not running", "daemon status reset")
            append_result(
                results,
                name="daemon_status_reset",
                command=daemon_status_before_cmd,
                cwd=root,
                completed=daemon_status_reset,
                evidence="Daemon status returned to `not running` before the explicit startup check.",
                status="PASS",
            )

        daemon_stdout = temp_root / "awod.stdout.log"
        daemon_stderr = temp_root / "awod.stderr.log"
        daemon_proc = subprocess.Popen(
            [str(awod)],
            cwd=root,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=daemon_stdout.open("w", encoding="utf-8"),
            stderr=daemon_stderr.open("w", encoding="utf-8"),
            text=True,
        )
        try:
            daemon_status_running_cmd = [str(awo), "daemon", "status"]
            daemon_running = None
            for _ in range(20):
                candidate = run_command(
                    "daemon status running",
                    daemon_status_running_cmd,
                    cwd=root,
                    env=env,
                    check=False,
                )
                if candidate.returncode == 0 and "healthy" in f"{candidate.stdout}\n{candidate.stderr}":
                    daemon_running = candidate
                    break
                time.sleep(0.25)

            if daemon_running is None:
                raise RuntimeError(
                    "daemon did not reach a healthy state during smoke validation"
                )
            append_result(
                results,
                name="daemon_status_running",
                command=daemon_status_running_cmd,
                cwd=root,
                completed=daemon_running,
                evidence="Daemon reached a healthy state after explicit startup.",
            )

            daemon_repo_list_cmd = [str(awo), "repo", "list"]
            daemon_repo_list = run_command(
                "daemon repo list",
                daemon_repo_list_cmd,
                cwd=root,
                env=env,
            )
            append_result(
                results,
                name="daemon_repo_list",
                command=daemon_repo_list_cmd,
                cwd=root,
                completed=daemon_repo_list,
                evidence="Repo list worked while the daemon was running explicitly.",
            )

            daemon_stop_cmd = [str(awo), "daemon", "stop"]
            daemon_stop = run_command("daemon stop", daemon_stop_cmd, cwd=root, env=env)
            append_result(
                results,
                name="daemon_stop",
                command=daemon_stop_cmd,
                cwd=root,
                completed=daemon_stop,
                evidence="Explicit daemon stop completed successfully.",
            )
        finally:
            if daemon_proc is not None:
                daemon_proc.wait(timeout=10)
                daemon_proc = None

        daemon_status_after_cmd = [str(awo), "daemon", "status"]
        daemon_status_after = run_command(
            "daemon status after",
            daemon_status_after_cmd,
            cwd=root,
            env=env,
            check=False,
        )
        ensure_contains(daemon_status_after, "not running", "daemon status after")
        append_result(
            results,
            name="daemon_status_after",
            command=daemon_status_after_cmd,
            cwd=root,
            completed=daemon_status_after,
            evidence="Daemon status returned to `not running` after explicit stop.",
            status="PASS",
        )

        team_commands: list[tuple[str, list[str], str]] = [
            (
                "team_init",
                [
                    str(awo),
                    "team",
                    "init",
                    repo_id,
                    "smoke-team",
                    "Smoke the local orchestration loop",
                ],
                "Initialized the smoke team.",
            ),
            (
                "team_member_add",
                [
                    str(awo),
                    "team",
                    "member",
                    "add",
                    "smoke-team",
                    "worker-a",
                    "worker",
                    "--runtime",
                    "shell",
                    "--model",
                    "local-shell",
                    "--notes",
                    "smoke worker",
                ],
                "Added the smoke worker.",
            ),
            (
                "team_plan_add",
                [
                    str(awo),
                    "team",
                    "plan",
                    "add",
                    "smoke-team",
                    "plan-smoke",
                    "Plan a shell task",
                    "Create an executable task card from planning",
                    "--owner-id",
                    "worker-a",
                    "--deliverable",
                    "A generated task card",
                ],
                "Added a draft plan item.",
            ),
            (
                "team_plan_approve",
                [str(awo), "team", "plan", "approve", "smoke-team", "plan-smoke"],
                "Approved the draft plan item.",
            ),
            (
                "team_plan_generate",
                [
                    str(awo),
                    "team",
                    "plan",
                    "generate",
                    "smoke-team",
                    "plan-smoke",
                    "task-planned",
                    "--owner-id",
                    "worker-a",
                    "--deliverable",
                    "A generated task card",
                ],
                "Generated a task card from the approved plan item.",
            ),
            (
                "team_task_add",
                [
                    str(awo),
                    "team",
                    "task",
                    "add",
                    "smoke-team",
                    "task-shell",
                    "worker-a",
                    "Inspect repo",
                    "echo 'task smoke'; pwd; ls",
                    "--deliverable",
                    "A repo listing",
                    "--read-only",
                ],
                "Added the shell-backed smoke task card.",
            ),
            (
                "team_task_start",
                [str(awo), "team", "task", "start", "smoke-team", "task-shell"],
                "Started the shell-backed smoke task card.",
            ),
        ]

        for name, command, evidence in team_commands:
            completed = run_command(name, command, cwd=root, env=env)
            append_result(
                results,
                name=name,
                command=command,
                cwd=root,
                completed=completed,
                evidence=evidence,
            )

        team_show_cmd = [str(awo), "team", "show", "smoke-team"]
        team_show = run_command("team show", team_show_cmd, cwd=root, env=env)
        ensure_contains(team_show, "review", "team show")
        append_result(
            results,
            name="team_show",
            command=team_show_cmd,
            cwd=root,
            completed=team_show,
            evidence="Team show reflected the completed task in review-ready state.",
        )

        for name, command, evidence in [
            (
                "team_task_add_replacement",
                [
                    str(awo),
                    "team",
                    "task",
                    "add",
                    "smoke-team",
                    "task-replacement",
                    "worker-a",
                    "Replacement",
                    "pwd",
                    "--deliverable",
                    "A replacement task",
                    "--read-only",
                ],
                "Added the replacement task card.",
            ),
            (
                "team_task_supersede",
                [
                    str(awo),
                    "team",
                    "task",
                    "supersede",
                    "smoke-team",
                    "task-planned",
                    "task-replacement",
                ],
                "Superseded the generated task card with the replacement task.",
            ),
            (
                "team_report",
                [str(awo), "team", "report", "smoke-team"],
                "Generated the team report successfully.",
            ),
            (
                "team_teardown",
                [str(awo), "team", "teardown", "smoke-team", "--force"],
                "Force teardown completed successfully.",
            ),
            (
                "team_delete",
                [str(awo), "team", "delete", "smoke-team"],
                "Deleted the smoke team successfully.",
            ),
        ]:
            completed = run_command(name, command, cwd=root, env=env)
            append_result(
                results,
                name=name,
                command=command,
                cwd=root,
                completed=completed,
                evidence=evidence,
            )

        tui_cmd = [str(awo)]
        tui_quit = run_command(
            "tui quit smoke",
            tui_cmd,
            cwd=root,
            env=env,
            input_text="q",
        )
        append_result(
            results,
            name="tui_quit_smoke",
            command=tui_cmd,
            cwd=root,
            completed=tui_quit,
            evidence="Non-interactive `q` input exited the TUI cleanly.",
        )

        write_reports(
            results=results,
            report_json=args.report_json,
            report_md=args.report_md,
            smoke_repo=smoke_repo,
            profile=args.profile,
        )
        print(f"[smoke] PASS: {len(results)} steps completed successfully")
        if args.report_json is not None:
            print(f"[smoke] JSON report: {args.report_json}")
        if args.report_md is not None:
            print(f"[smoke] Markdown report: {args.report_md}")
        return 0
    finally:
        if daemon_proc is not None and daemon_proc.poll() is None:
            daemon_proc.kill()
            daemon_proc.wait(timeout=10)
        if args.keep_temp:
            print(f"[smoke] kept temporary state at {temp_root}")
        else:
            shutil.rmtree(temp_root, ignore_errors=True)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:
        print(f"[smoke] FAIL: {error}", file=sys.stderr)
        sys.exit(1)
