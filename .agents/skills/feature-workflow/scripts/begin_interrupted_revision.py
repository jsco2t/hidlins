#!/usr/bin/env python3

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import tempfile

from plan_hash import calculate_plan_hash, repository_workflow_root


ALLOWED_PHASES = {"BLOCKED", "PLAN_CHANGE_REQUIRED"}


def fail(message: str) -> None:
    raise ValueError(message)


def task_title(workflow: Path, task: dict) -> str:
    path = workflow / task["path"]
    try:
        heading = path.read_text().splitlines()[0]
    except (FileNotFoundError, IndexError):
        return task["path"]
    prefix = f"# Task {task['id']}: "
    return heading.removeprefix(prefix)


def render_status(workflow: Path, state: dict) -> str:
    lines = [
        "# Work-Package Status",
        "",
        f"Work ID: `{state['work_id']}`",
        "",
        f"Title: `{state['title']}`",
        "",
        f"Phase: `{state['phase']}`",
        "",
        f"Plan revision: `{state['plan_revision']}`",
        "",
        f"Acceptance round: `{state.get('acceptance_round', 1)}`",
        "",
        f"Prior completed rounds: `{len(state.get('completed_rounds', []))}`",
        "",
        "Approved: no",
        "",
        "Current task: none",
        "",
        "## Tasks",
        "",
    ]
    for task in state["tasks"]:
        checked = "x" if task["status"] == "COMPLETE" else " "
        details = task["status"]
        if task["status"] == "COMPLETE":
            details = task.get("implementation") or "complete"
        attempts = task.get("attempts", 0)
        attempt_label = "attempt" if attempts == 1 else "attempts"
        lines.append(
            f"- [{checked}] {task['id']} — {task_title(workflow, task)} — "
            f"round {task.get('round', 1)} — {details} — "
            f"{attempts} {attempt_label}"
        )
    lines.extend(
        [
            "",
            "## Package quality gate",
            "",
            "Pending interrupted-plan revision and re-approval",
            "",
            "## Current blocker",
            "",
            "None",
            "",
        ]
    )
    return "\n".join(lines)


def write_atomic(path: Path, contents: str) -> None:
    with tempfile.NamedTemporaryFile(
        mode="w", encoding="utf-8", dir=path.parent, delete=False
    ) as handle:
        handle.write(contents)
        handle.flush()
        os.fsync(handle.fileno())
        temporary = Path(handle.name)
    try:
        os.replace(temporary, path)
    except Exception:
        temporary.unlink(missing_ok=True)
        raise


def begin_interrupted_revision(workflow: Path, timestamp: str) -> dict:
    state_path = workflow / "state.json"
    try:
        state = json.loads(state_path.read_text())
    except FileNotFoundError:
        fail(f"no active workflow state at {state_path}")
    except json.JSONDecodeError as error:
        fail(f"invalid workflow state: {error}")

    phase = state.get("phase")
    if phase not in ALLOWED_PHASES:
        fail("interrupted revision requires phase BLOCKED or PLAN_CHANGE_REQUIRED")

    expected_hash = state.get("approved_plan_sha256")
    if not expected_hash:
        fail("interrupted package has no approved plan hash")
    try:
        actual_hash = calculate_plan_hash(workflow)
    except FileNotFoundError as error:
        fail(str(error))
    if actual_hash != expected_hash:
        fail(
            "approved planning artifacts changed: "
            f"expected {expected_hash}, got {actual_hash}"
        )

    if not state.get("approved_at"):
        fail("interrupted package has no approval timestamp")
    if state.get("completed_at") or state.get("final_sha"):
        fail("interrupted package unexpectedly has completion state")

    tasks = state.get("tasks")
    if not isinstance(tasks, list) or not tasks:
        fail("interrupted package has no task state")

    reopened_task_ids = []
    for task in tasks:
        task_id = task.get("id", "<unknown>")
        task_path = task.get("path")
        if not task_path or not (workflow / task_path).is_file():
            fail(f"task {task_id} has no task document")
        if task.get("status") == "COMPLETE":
            evidence = task.get("evidence")
            if not evidence or not (workflow / evidence).is_file():
                fail(f"completed task {task_id} has no evidence file")
            continue
        reopened_task_ids.append(task_id)

    if not reopened_task_ids:
        fail("interrupted package has no incomplete tasks to revise")

    for task in tasks:
        if task.get("status") == "COMPLETE":
            continue
        task.update(
            {
                "status": "PENDING",
                "attempts": 0,
                "implementation": None,
                "evidence": None,
            }
        )

    state.update(
        {
            "schema_version": 2,
            "phase": "PLANNING",
            "updated_at": timestamp,
            "approved_at": None,
            "plan_revision": int(state["plan_revision"]) + 1,
            "approved_plan_sha256": None,
            "current_task": None,
            "final_attempts": 0,
            "stop_continuations": 0,
            "block_reason": None,
        }
    )

    status_path = workflow / "status.md"
    old_status = status_path.read_text() if status_path.exists() else None
    try:
        write_atomic(status_path, render_status(workflow, state))
        write_atomic(state_path, json.dumps(state, indent=2) + "\n")
    except Exception:
        if old_status is None:
            status_path.unlink(missing_ok=True)
        else:
            write_atomic(status_path, old_status)
        raise

    return {
        "plan_revision": state["plan_revision"],
        "reopened_task_ids": reopened_task_ids,
    }


def main() -> None:
    parser = argparse.ArgumentParser(
        description=(
            "Verify and reopen an interrupted approved workflow for human-requested "
            "plan revision."
        )
    )
    parser.add_argument(
        "--workflow-root",
        type=Path,
        default=None,
        help="workflow directory (defaults to <git-root>/.ai/workflow)",
    )
    args = parser.parse_args()
    workflow = args.workflow_root or repository_workflow_root()
    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    try:
        result = begin_interrupted_revision(workflow.resolve(), timestamp)
    except (OSError, TypeError, ValueError) as error:
        raise SystemExit(f"cannot begin interrupted revision: {error}") from error
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
