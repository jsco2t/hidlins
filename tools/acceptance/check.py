#!/usr/bin/env python3
"""Audit the committed Flutter alpha acceptance and release matrix."""

from __future__ import annotations

import argparse
import json
import re
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_MATRIX = ROOT / "docs" / "flutter-alpha-acceptance.json"
JULY_IDS = {
    *(f"T1.{number}" for number in range(1, 10)),
    *(f"T2.{number}" for number in range(1, 7)),
    *(f"T3.{number}" for number in range(1, 6)),
    *(f"T4.{number}" for number in range(1, 9)),
    *(f"T5.{number}" for number in range(1, 6)),
    *(f"T6.{number}" for number in range(1, 3)),
    *(f"T7.{number}" for number in range(1, 6)),
    *(f"T8.{number}" for number in range(1, 6)),
}
REQUIRED_PLATFORMS = {"rust", "linux", "macos", "ios", "android"}
REQUIRED_WARNING_POLICIES = {"rust", "dart", "linux", "apple", "android"}
ALLOWED_DISPOSITIONS = {
    "already-covered",
    "revalidated",
    "superseded-with-rationale",
    "completed-in-package",
}


def fail(message: str) -> None:
    raise ValueError(message)


def load(path: Path) -> dict[str, object]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot load acceptance matrix {path}: {error}")
    if not isinstance(data, dict):
        fail("acceptance matrix root must be an object")
    return data


def make_targets() -> set[str]:
    text = (ROOT / "Makefile").read_text(encoding="utf-8")
    return set(re.findall(r"^([a-zA-Z][a-zA-Z0-9_-]*):", text, re.MULTILINE))


def check(data: dict[str, object]) -> None:
    if data.get("schema_version") != 1:
        fail("acceptance matrix schema_version must be 1")

    android = data.get("android_build_model")
    expected_android = {
        "flutter": "3.47.2",
        "built_in_kotlin": True,
        "new_dsl": False,
        "kotlin_version": "2.4.0",
        "kotlin_plugin_applied_to_hidlins": False,
        "dependency_validation_bypass": False,
    }
    if not isinstance(android, dict) or any(
        android.get(key) != value for key, value in expected_android.items()
    ):
        fail("Android build-model record does not match the approved Flutter 3.47.2 contract")

    targets = make_targets()
    artifacts = data.get("artifacts")
    if not isinstance(artifacts, list):
        fail("artifacts must be a list")
    platforms = {item.get("platform") for item in artifacts if isinstance(item, dict)}
    if platforms != REQUIRED_PLATFORMS:
        fail(f"artifact platforms are {platforms}, expected {REQUIRED_PLATFORMS}")
    for item in artifacts:
        if not isinstance(item, dict):
            fail("artifact entry must be an object")
        target = item.get("make_target")
        if target not in targets or not item.get("host") or not item.get("locations"):
            fail(f"incomplete artifact entry: {item}")

    warning_policies = data.get("warning_policies")
    if not isinstance(warning_policies, list):
        fail("warning_policies must be a list")
    warning_names = {
        item.get("language") for item in warning_policies if isinstance(item, dict)
    }
    if warning_names != REQUIRED_WARNING_POLICIES:
        fail("warning policy inventory is incomplete")
    for item in warning_policies:
        if not isinstance(item, dict) or item.get("status") != "PASS":
            fail("every warning policy must be recorded as PASS")
        for source in item.get("sources", []):
            if not (ROOT / source).exists():
                fail(f"warning-policy source does not exist: {source}")

    requirements = data.get("requirements")
    if not isinstance(requirements, list) or not requirements:
        fail("requirements must be a non-empty list")
    by_id: dict[str, dict[str, object]] = {}
    for item in requirements:
        if not isinstance(item, dict) or not isinstance(item.get("id"), str):
            fail("requirement entry must have a string id")
        identifier = item["id"]
        if identifier in by_id:
            fail(f"duplicate requirement: {identifier}")
        by_id[identifier] = item
        status = item.get("status")
        if status == "PASS":
            listed_targets = item.get("make_targets")
            evidence = item.get("evidence")
            if not isinstance(listed_targets, list) or not listed_targets:
                fail(f"automated requirement lacks a Make target: {identifier}")
            if any(target not in targets for target in listed_targets):
                fail(f"requirement names an unknown Make target: {identifier}")
            if not isinstance(evidence, list) or not evidence:
                fail(f"automated requirement lacks evidence: {identifier}")
            for source in evidence:
                if not (ROOT / source).exists():
                    fail(f"requirement evidence does not exist: {source}")
        elif status == "SKIPPED":
            if not item.get("reason") or not item.get("precursors"):
                fail(f"skipped residual lacks reason or automated precursor: {identifier}")
        else:
            fail(f"requirement {identifier} has forbidden status {status!r}")
    for identifier, item in by_id.items():
        if item.get("status") == "SKIPPED":
            for precursor in item["precursors"]:
                if precursor not in by_id or by_id[precursor].get("status") != "PASS":
                    fail(f"skipped residual {identifier} has a non-passing precursor {precursor}")

    local_sync = data.get("local_sync_acceptance")
    if not isinstance(local_sync, dict) or local_sync.get("status") != "PASS":
        fail("local-sync acceptance must be automated and PASS")
    local_targets = local_sync.get("make_targets")
    required_local_targets = {
        "test-local-sync-mobile-scenarios-ios",
        "test-local-sync-mobile-scenarios-android",
    }
    if not isinstance(local_targets, list) or not required_local_targets.issubset(local_targets):
        fail("local-sync acceptance lacks both real mobile scenario targets")
    if any(target not in targets for target in local_targets):
        fail("local-sync acceptance names an unknown Make target")
    local_evidence = local_sync.get("evidence")
    if not isinstance(local_evidence, list) or not local_evidence:
        fail("local-sync acceptance lacks evidence")
    for source in local_evidence:
        if not (ROOT / source).exists():
            fail(f"local-sync acceptance evidence does not exist: {source}")

    optional = data.get("local_sync_optional_observations")
    if not isinstance(optional, dict) or optional.get("status") != "OPTIONAL_NON_BLOCKING":
        fail("local-sync human observations must be optional and non-blocking")
    if not optional.get("reason") or not optional.get("procedure"):
        fail("local-sync optional observations lack rationale or procedure")

    july = data.get("july_task_audit")
    if not isinstance(july, list):
        fail("july_task_audit must be a list")
    july_by_id = {
        item.get("id"): item for item in july if isinstance(item, dict)
    }
    if set(july_by_id) != JULY_IDS:
        fail("July task audit must classify exactly T1.1 through T8.5")
    for identifier, item in july_by_id.items():
        if item.get("disposition") not in ALLOWED_DISPOSITIONS or not item.get("evidence"):
            fail(f"July task {identifier} lacks a valid disposition and evidence")

    for document in data.get("required_documents", []):
        if not (ROOT / document).exists():
            fail(f"required acceptance document does not exist: {document}")

    workflow = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
    for target in data.get("required_ci_targets", []):
        if f"make {target}" not in workflow:
            fail(f"CI does not invoke required Make target: {target}")


class NegativeControls(unittest.TestCase):
    def fixture(self) -> dict[str, object]:
        return load(DEFAULT_MATRIX)

    def test_missing_automated_evidence_is_rejected(self) -> None:
        data = self.fixture()
        next(item for item in data["requirements"] if item["status"] == "PASS")["evidence"] = []
        with self.assertRaisesRegex(ValueError, "lacks evidence"):
            check(data)

    def test_unbacked_skip_is_rejected(self) -> None:
        data = self.fixture()
        next(item for item in data["requirements"] if item["status"] == "SKIPPED")["precursors"] = ["missing"]
        with self.assertRaisesRegex(ValueError, "non-passing precursor"):
            check(data)

    def test_manual_status_is_rejected(self) -> None:
        data = self.fixture()
        data["requirements"][0]["status"] = "MANUAL"
        with self.assertRaisesRegex(ValueError, "forbidden status"):
            check(data)

    def test_blocking_local_sync_manual_release_is_rejected(self) -> None:
        data = self.fixture()
        data["local_sync_acceptance"]["status"] = "BLOCKED"
        with self.assertRaisesRegex(ValueError, "local-sync acceptance"):
            check(data)

    def test_missing_july_task_is_rejected(self) -> None:
        data = self.fixture()
        data["july_task_audit"].pop()
        with self.assertRaisesRegex(ValueError, "classify exactly"):
            check(data)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("check", "self-test"))
    parser.add_argument("--matrix", type=Path, default=DEFAULT_MATRIX)
    args = parser.parse_args()
    if args.command == "self-test":
        result = unittest.TextTestRunner(verbosity=1).run(
            unittest.defaultTestLoader.loadTestsFromTestCase(NegativeControls)
        )
        raise SystemExit(0 if result.wasSuccessful() else 1)
    check(load(args.matrix))
    print("  OK: Flutter alpha evidence, skips, artifacts, warnings, CI, and 45 July tasks are complete")


if __name__ == "__main__":
    main()
