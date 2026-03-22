#!/usr/bin/env python3
"""Normalize Grok CLI event-stream output into one JSON document.

This wrapper is intended for integrations that expect a single JSON object
instead of Grok's line-delimited event stream.
"""

from __future__ import annotations

import argparse
import html
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run Grok CLI and emit one normalized JSON document."
    )
    parser.add_argument(
        "-d",
        "--directory",
        default=".",
        help="working directory to pass through to grok",
    )
    parser.add_argument(
        "-m",
        "--model",
        default=None,
        help="optional Grok model override",
    )
    parser.add_argument(
        "--raw-events",
        action="store_true",
        help="include raw parsed event objects in the output",
    )
    parser.add_argument(
        "prompt",
        nargs="+",
        help="prompt to send to grok",
    )
    return parser.parse_args()


def normalize_event(line: str) -> dict[str, Any]:
    event = json.loads(line)
    if isinstance(event.get("tool_calls"), list):
        for tool_call in event["tool_calls"]:
            function = tool_call.get("function")
            if not isinstance(function, dict):
                continue
            arguments = function.get("arguments")
            if isinstance(arguments, str):
                # Grok sometimes HTML-escapes shell operators in tool arguments.
                function["arguments"] = html.unescape(arguments)
    return event


def build_result(
    events: list[dict[str, Any]],
    malformed_lines: list[str],
    stderr: str,
    returncode: int,
    include_raw_events: bool,
) -> dict[str, Any]:
    assistant_messages = [
        event.get("content")
        for event in events
        if event.get("role") == "assistant" and isinstance(event.get("content"), str)
    ]
    tool_events = [event for event in events if event.get("role") == "tool"]
    tool_calls = []
    for event in events:
        calls = event.get("tool_calls")
        if isinstance(calls, list):
            tool_calls.extend(calls)

    result: dict[str, Any] = {
        "ok": returncode == 0 and not malformed_lines,
        "returncode": returncode,
        "final_text": assistant_messages[-1] if assistant_messages else None,
        "assistant_messages": assistant_messages,
        "tool_calls": tool_calls,
        "tool_results": [
            {
                "tool_call_id": event.get("tool_call_id"),
                "content": event.get("content"),
            }
            for event in tool_events
        ],
        "stderr": stderr,
        "malformed_lines": malformed_lines,
    }
    if include_raw_events:
        result["events"] = events
    return result


def main() -> int:
    args = parse_args()
    prompt = " ".join(args.prompt)
    cmd = ["grok", "-d", str(Path(args.directory).resolve()), "-p", prompt]
    if args.model:
        cmd.extend(["-m", args.model])

    completed = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )

    events: list[dict[str, Any]] = []
    malformed_lines: list[str] = []
    for raw_line in completed.stdout.splitlines():
        line = raw_line.strip()
        if not line:
            continue
        try:
            events.append(normalize_event(line))
        except json.JSONDecodeError:
            malformed_lines.append(raw_line)

    result = build_result(
        events=events,
        malformed_lines=malformed_lines,
        stderr=completed.stderr,
        returncode=completed.returncode,
        include_raw_events=args.raw_events,
    )
    json.dump(result, sys.stdout, ensure_ascii=True)
    sys.stdout.write("\n")
    return 0 if result["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
