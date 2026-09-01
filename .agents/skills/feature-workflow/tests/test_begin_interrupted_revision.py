import json
from pathlib import Path
import sys
import tempfile
import unittest


SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
sys.path.insert(0, str(SCRIPTS))

from begin_interrupted_revision import begin_interrupted_revision  # noqa: E402
from plan_hash import calculate_plan_hash  # noqa: E402


class InterruptedRevisionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.workflow = Path(self.temporary.name) / "workflow"
        (self.workflow / "tasks").mkdir(parents=True)
        (self.workflow / "evidence").mkdir()
        (self.workflow / "request.md").write_text("# Request\n")
        (self.workflow / "plan.md").write_text("# Plan\n")
        (self.workflow / "gate.json").write_text('{"standard_commands": []}\n')
        (self.workflow / "tasks" / "001-complete.md").write_text(
            "# Task 001: Complete work\n"
        )
        (self.workflow / "tasks" / "002-interrupted.md").write_text(
            "# Task 002: Interrupted work\n"
        )
        (self.workflow / "evidence" / "001.md").write_text(
            "# Evidence — Task 001\n"
        )
        self._write_state("BLOCKED")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _write_state(self, phase: str) -> None:
        state = {
            "schema_version": 2,
            "work_id": "2026-08-31-example",
            "title": "Example",
            "phase": phase,
            "created_at": "2026-08-31T00:00:00Z",
            "updated_at": "2026-08-31T01:00:00Z",
            "approved_at": "2026-08-31T00:10:00Z",
            "completed_at": None,
            "archived_at": None,
            "baseline_sha": "a" * 40,
            "final_sha": None,
            "plan_revision": 3,
            "acceptance_round": 1,
            "completed_rounds": [],
            "approved_plan_sha256": calculate_plan_hash(self.workflow),
            "current_task": "002",
            "tasks": [
                {
                    "id": "001",
                    "path": "tasks/001-complete.md",
                    "round": 1,
                    "status": "COMPLETE",
                    "attempts": 2,
                    "implementation": "main",
                    "evidence": "evidence/001.md",
                },
                {
                    "id": "002",
                    "path": "tasks/002-interrupted.md",
                    "round": 1,
                    "status": "IN_PROGRESS",
                    "attempts": 1,
                    "implementation": None,
                    "evidence": None,
                },
            ],
            "task_retry_limit": 3,
            "final_retry_limit": 2,
            "final_attempts": 0,
            "stop_continuations": 4,
            "max_stop_continuations": 50,
            "block_reason": "old blocker",
        }
        (self.workflow / "state.json").write_text(json.dumps(state, indent=2) + "\n")
        (self.workflow / "status.md").write_text("old status\n")

    def _state(self) -> dict:
        return json.loads((self.workflow / "state.json").read_text())

    def test_blocked_package_reopens_only_incomplete_work(self) -> None:
        result = begin_interrupted_revision(
            self.workflow, "2026-08-31T02:00:00Z"
        )
        state = self._state()

        self.assertEqual(
            result,
            {"plan_revision": 4, "reopened_task_ids": ["002"]},
        )
        self.assertEqual(state["phase"], "PLANNING")
        self.assertEqual(state["plan_revision"], 4)
        self.assertIsNone(state["approved_at"])
        self.assertIsNone(state["approved_plan_sha256"])
        self.assertIsNone(state["current_task"])
        self.assertIsNone(state["block_reason"])
        self.assertEqual(state["stop_continuations"], 0)

        completed, reopened = state["tasks"]
        self.assertEqual(completed["status"], "COMPLETE")
        self.assertEqual(completed["attempts"], 2)
        self.assertEqual(completed["implementation"], "main")
        self.assertEqual(completed["evidence"], "evidence/001.md")
        self.assertEqual(reopened["status"], "PENDING")
        self.assertEqual(reopened["attempts"], 0)
        self.assertIsNone(reopened["implementation"])
        self.assertIsNone(reopened["evidence"])

        status = (self.workflow / "status.md").read_text()
        self.assertIn("Phase: `PLANNING`", status)
        self.assertIn("Plan revision: `4`", status)
        self.assertIn("Approved: no", status)
        self.assertIn("002 — Interrupted work", status)

    def test_plan_change_required_package_can_reopen(self) -> None:
        self._write_state("PLAN_CHANGE_REQUIRED")

        begin_interrupted_revision(self.workflow, "2026-08-31T02:00:00Z")

        self.assertEqual(self._state()["phase"], "PLANNING")

    def test_plan_drift_refuses_without_mutating_package(self) -> None:
        (self.workflow / "plan.md").write_text("# Changed plan\n")
        original_state = (self.workflow / "state.json").read_text()
        original_status = (self.workflow / "status.md").read_text()

        with self.assertRaisesRegex(ValueError, "approved planning artifacts changed"):
            begin_interrupted_revision(self.workflow, "2026-08-31T02:00:00Z")

        self.assertEqual((self.workflow / "state.json").read_text(), original_state)
        self.assertEqual((self.workflow / "status.md").read_text(), original_status)

    def test_other_phases_are_rejected(self) -> None:
        for phase in ("AWAITING_APPROVAL", "DONE", "EXECUTING"):
            with self.subTest(phase=phase):
                self._write_state(phase)
                with self.assertRaisesRegex(
                    ValueError, "requires phase BLOCKED or PLAN_CHANGE_REQUIRED"
                ):
                    begin_interrupted_revision(
                        self.workflow, "2026-08-31T02:00:00Z"
                    )


if __name__ == "__main__":
    unittest.main()
