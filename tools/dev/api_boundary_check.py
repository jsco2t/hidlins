#!/usr/bin/env python3
"""Keep native artifacts and network dependencies at their Rust boundaries."""

from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[2]
NETWORK = {"ureq", "reqwest", "hyper", "curl", "isahc", "attohttpc", "surf", "tonic"}


def check(root: Path) -> list[str]:
    errors = []
    manifests = sorted((root / "crates").glob("*/Cargo.toml"))
    if not manifests:
        return ["no Rust crate manifests found"]
    for path in manifests:
        data = tomllib.loads(path.read_text())
        name = data["package"]["name"]
        artifacts = set(data.get("lib", {}).get("crate-type", []))
        if name != "hidlins-api" and artifacts & {"cdylib", "staticlib"}:
            errors.append(f"{name}: native library artifacts belong to hidlins-api")
        if name != "hidlins-sync":
            tables = [data, *data.get("target", {}).values()]
            for table in tables:
                for section in ("dependencies", "build-dependencies"):
                    for alias, dependency in table.get(section, {}).items():
                        package = dependency.get("package", alias) if isinstance(dependency, dict) else alias
                        if package in NETWORK:
                            errors.append(f"{name}: network dependency {package} belongs to hidlins-sync")
    return errors


if __name__ == "__main__":
    failures = check(ROOT)
    if failures:
        raise SystemExit("\n".join(failures))
    print("Rust native artifact and network dependency boundaries are intact.")
