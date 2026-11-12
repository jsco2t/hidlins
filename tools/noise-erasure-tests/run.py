#!/usr/bin/env python3
"""Observe real owned SHA bytes in isolated, instrumented copies of selected code.

Instrumentation reports booleans; it never supplies a missing wipe. Local path
patches make Cargo track fixture edits, including each independent negative case.
No fixture, observer dependency, or instrumentation enters an application graph.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
FEATURES = ["std", "use-curve25519", "use-chacha20poly1305", "use-sha2", "use-getrandom"]
OBSERVER = "hidlins-erasure-observer"


def replace(source: str, before: str, after: str, count: int = 1) -> str:
    if source.count(before) != count:
        raise RuntimeError(f"fixture source context count {source.count(before)} != {count}: {before!r}")
    return source.replace(before, after)


def edit(path: Path, before: str, after: str, count: int = 1) -> None:
    path.write_text(replace(path.read_text(), before, after, count))


def write_toml(path: Path, value: dict) -> None:
    """Render the deliberately small scalar/list/table fixture manifests."""
    lines: list[str] = []

    def table(data: dict, prefix: tuple[str, ...]) -> None:
        if prefix:
            lines.append("[" + ".".join(json.dumps(part) for part in prefix) + "]")
        for key, item in data.items():
            if not isinstance(item, dict):
                lines.append(json.dumps(key) + " = " + json.dumps(item))
        for key, item in data.items():
            if isinstance(item, dict):
                table(item, (*prefix, key))

    table(value, ())
    path.write_text("\n".join(lines) + "\n")


def fixture_manifest(crate: Path, observer: Path | None) -> None:
    """Retain the selected production capabilities, never fetch optional tools.

    Registry lockfiles omit unselected optional packages. Cargo resolves all
    optional paths of a local crate, so remove only unselected dependencies and
    developer targets from these disposable copies. Keep cfg feature names.
    """
    original = tomllib.loads((crate / "Cargo.toml").read_text())
    name = original["package"]["name"]
    allowed = {
        "snow": {"blake2", "chacha20poly1305", "curve25519-dalek", "getrandom", "sha2", "subtle", "zeroize"},
        "sha2": {"cfg-if", "digest"},
        "digest": {"common", "block-buffer", "zeroize"},
        "block-buffer": {"generic-array", "hybrid-array", "zeroize"},
    }[name]
    package = {key: val for key, val in original["package"].items() if key in {
        "name", "version", "edition", "rust-version", "license", "repository", "build",
    }}
    value = {"package": package, "lib": {"path": "src/lib.rs"}}
    dependencies = {key: val for key, val in original.get("dependencies", {}).items() if key in allowed}
    feature_names = set(original.get("features", {}))
    features = {}
    for feature, values in original.get("features", {}).items():
        features[feature] = [entry for entry in values if
            entry.split("/")[0].rstrip("?").removeprefix("dep:") in allowed | feature_names]
    for removed in set(original.get("dependencies", {})) - allowed:
        features.setdefault(removed, [])
    if features:
        value["features"] = features
    if observer is not None:
        dependencies[OBSERVER] = {"path": str(observer)}
    value["dependencies"] = dependencies
    if original.get("build-dependencies"):
        value["build-dependencies"] = original["build-dependencies"]
    if original.get("target"):
        value["target"] = original["target"]
    if original.get("lints"):
        value["lints"] = original["lints"]
    write_toml(crate / "Cargo.toml", value)


def instrument_legacy(crates: dict[str, Path]) -> None:
    path = crates["block-buffer"] / "src/lib.rs"
    edit(path, "        self.set_pos_unchecked(0);\n    }\n\n    /// Pad remaining data",
         "        self.set_pos_unchecked(0);\n"
         '        hidlins_erasure_observer::record_bytes("stored-buffer", &self.buffer);\n'
         "    }\n\n    /// Pad remaining data")


def instrument_modern(crates: dict[str, Path]) -> None:
    # Core fields are initialized integers, observed after the real destructor.
    core = crates["sha2"] / "src/block_api.rs"
    source, count = re.subn(
        r'(impl Drop for Sha(?:256|512)VarCore \{\n    fn drop\(&mut self\) \{\n.*?)(\n    \}\n\})',
        lambda match: match[1] + '\n        hidlins_erasure_observer::record_erased("sha-core", self.state.iter().all(|word| *word == 0) && self.block_len == 0);' + match[2],
        core.read_text(), flags=re.DOTALL,
    )
    if count != 2:
        raise RuntimeError("expected both actual SHA core destructors")
    core.write_text(source)

    buffer = crates["block-buffer"] / "src/lib.rs"
    # Test-only initialization makes every backing byte readable even when a
    # negative control removes zeroization. Unused bytes do not affect hashing.
    edit(buffer, "let mut buffer = MaybeUninit::uninit();",
         "let mut buffer = MaybeUninit::new(Default::default());")
    source, count = re.subn(
        r'(impl<BS: BlockSizes, K: BufferKind> Drop for BlockBuffer<BS, K> \{\n    #\[inline\]\n    fn drop\(&mut self\) \{\n.*?)(\n    \}\n\})',
        lambda match: match[1] + '\n        // SAFETY: fixture Default initializes the complete byte Array; copies preserve initialization.\n        let initialized = unsafe { self.buffer.assume_init_ref() };\n        hidlins_erasure_observer::record_bytes("stored-buffer", initialized.as_slice());' + match[2],
        buffer.read_text(), flags=re.DOTALL,
    )
    if count != 1:
        raise RuntimeError("expected actual stored-buffer destructor")
    buffer.write_text(source)
    source = buffer.read_text()
    declaration = "let buf = self.pad_with_zeros();" if "let buf = self.pad_with_zeros();" in source else "let mut buf = self.pad_with_zeros();"
    source = replace(source, declaration, declaration.replace("self.pad_with_zeros()", 'hidlins_erasure_observer::Bytes::new("padding", self.pad_with_zeros())'))
    buffer.write_text(source)

    digest = crates["digest"] / "src/block_api/ct_variable.rs"
    source = digest.read_text()
    declaration = "let full_res = Array::<u8, T::OutputSize>::default();"
    if declaration in source:
        source = replace(source, declaration, 'let full_res = hidlins_erasure_observer::Bytes::new("digest-output", Array::<u8, T::OutputSize>::default());')
    else:
        source = replace(source, "let mut full_res = Default::default();", 'let mut full_res = hidlins_erasure_observer::Bytes::new("digest-output", Array::<u8, T::OutputSize>::default());')
    digest.write_text(source)

    snow = crates["snow"] / "src/resolvers/default.rs"
    source = snow.read_text()
    for size in (32, 64):
        guarded = f"let mut hash = Zeroizing::new([0_u8; {size}]);"
        plain = f"let mut hash = [0_u8; {size}];"
        observed = f'hidlins_erasure_observer::Bytes::new("hash-output", [0_u8; {size}])'
        if guarded in source:
            source = replace(source, guarded, f"let mut hash = Zeroizing::new({observed});")
        elif plain in source:
            source = replace(source, plain, f"let mut hash = {observed};")
        else:
            raise RuntimeError("missing native-output scratch in selected Snow SHA implementation")
    # One extra dereference reaches the same native array underneath the
    # transparent observer, without changing the actual outer guard's behavior.
    if "Zeroizing::new(hidlins_erasure_observer::Bytes" in source:
        source = replace(source, "((&mut *hash).into())", "((&mut **hash).into())", 2)
    snow.write_text(source)


def vectors(destination: Path) -> None:
    lengths = [0, 1, 55, 56, 63, 64, 111, 112, 127, 128, 129]
    rows = ["const HASH_VECTORS: &[(usize, &str, &str)] = &["]
    for length in lengths:
        data = bytes([0xa5]) * length
        rows.append(f'({length}, "{hashlib.sha256(data).hexdigest()}", "{hashlib.sha512(data).hexdigest()}"),')
    (destination / "hash_vectors.rs").write_text("\n".join(rows) + "];\n")
    entries = json.loads((ROOT / "vendor/snow/tests/vectors/cacophony.txt").read_text())["vectors"]
    rows = ["const WIRE_VECTORS: &[WireVector] = &["]
    for pattern in ("Noise_XX_25519_ChaChaPoly_SHA256", "Noise_IK_25519_ChaChaPoly_SHA256"):
        vector = next(entry for entry in entries if entry["protocol_name"] == pattern)
        rows.append("WireVector {")
        for field, key in {
            "pattern": "protocol_name", "prologue": "init_prologue", "initiator": "init_static",
            "initiator_ephemeral": "init_ephemeral", "remote": "init_remote_static",
            "responder": "resp_static", "responder_ephemeral": "resp_ephemeral", "handshake_hash": "handshake_hash",
        }.items():
            rows.append(field + ": " + json.dumps(vector.get(key, "")) + ",")
        rows.append("messages: &[")
        for message in vector["messages"]:
            rows.append("(" + json.dumps(message["payload"]) + ", " + json.dumps(message["ciphertext"]) + "),")
        rows.append("], },")
    (destination / "wire_vectors.rs").write_text("\n".join(rows) + "];\n")


def remove_guard(path: Path, variable: str) -> None:
    edit(path, f'        #[cfg(feature = "zeroize")]\n        let mut {variable} = zeroize::Zeroizing::new({variable});\n',
         f'        #[cfg(feature = "zeroize")]\n        let mut {variable} = {variable};\n')


def mutate(crates: dict[str, Path], case: str) -> None:
    snow = crates["snow"] / "src/resolvers/default.rs"
    if case == "core-wipe":
        edit(crates["sha2"] / "src/block_api.rs", "            self.state.zeroize();\n            self.block_len.zeroize();",
             "            // negative control: omit both actual core erasures", 2)
    elif case == "buffer-wipe":
        edit(crates["block-buffer"] / "src/lib.rs", '        #[cfg(feature = "zeroize")]\n        self.zeroize();',
             "        // negative control: omit actual stored-buffer erasure")
    elif case == "hash-output":
        for size in (32, 64):
            edit(snow, f"Zeroizing::new([0_u8; {size}])", f"[0_u8; {size}]")
    elif case == "digest-output":
        remove_guard(crates["digest"] / "src/block_api/ct_variable.rs", "full_res")
    elif case == "padding":
        remove_guard(crates["block-buffer"] / "src/lib.rs", "buf")
    elif case == "consume-state":
        edit(snow, "core::mem::take(&mut self.hasher).finalize_into(", "self.hasher.finalize_into_reset(", 2)
    elif case == "feature-off":
        path = crates["snow"] / "Cargo.toml"
        # Normalize away developer target arrays before editing the small
        # fixture manifest; production source remains entirely uninstrumented.
        fixture_manifest(crates["snow"], None)
        manifest = tomllib.loads(path.read_text())
        manifest["dependencies"]["sha2"]["features"] = []
        write_toml(path, manifest)
    elif case != "positive":
        raise RuntimeError("unknown negative control")


def prepare(directory: Path, case: str) -> tuple[Path, bool]:
    directory.mkdir()
    snow_manifest = tomllib.loads((ROOT / "vendor/snow/Cargo.toml").read_text())
    legacy = snow_manifest["dependencies"]["sha2"]["version"].lstrip("=").startswith("0.10")
    selected = {"snow": "snow", "block-buffer": "block-buffer-0.10.4"} if legacy else {
        "snow": "snow", "sha2": "sha2", "digest": "digest", "block-buffer": "block-buffer",
    }
    crates = {}
    for name, vendor_name in selected.items():
        destination = directory / "instrumented" / name
        shutil.copytree(ROOT / "vendor" / vendor_name, destination)
        crates[name] = destination
    observer = directory / "observer"
    (observer / "src").mkdir(parents=True)
    shutil.copy2(HERE / "observer.rs", observer / "src/lib.rs")
    write_toml(observer / "Cargo.toml", {
        "package": {"name": OBSERVER, "version": "0.0.0", "edition": "2021", "license": "MIT"},
        "dependencies": {"zeroize": {"version": "=1.8.2", "default-features": False}},
    })
    mutate(crates, case)
    if case != "feature-off":
        if legacy:
            instrument_legacy(crates)
        else:
            instrument_modern(crates)
        # A removed wipe may leave its import unused. This fixture-only
        # allowance ensures the negative control tests actual residual bytes.
        for path in [crate / "src/lib.rs" for crate in crates.values()]:
            source = path.read_text()
            # Inner attributes precede inner documentation in these copies.
            path.write_text("#![allow(unused_imports)]\n" + source)
    for name, crate in crates.items():
        use_observer = case != "feature-off" and (not legacy or name == "block-buffer")
        fixture_manifest(crate, observer if use_observer else None)
    (directory / "src").mkdir()
    if case == "feature-off":
        (directory / "src/lib.rs").write_text("//! Compile-time production erasure feature guard.\n")
    else:
        shutil.copy2(HERE / "probe.rs", directory / "src/lib.rs")
        vectors(directory)
    write_toml(directory / "Cargo.toml", {
        "package": {"name": "hidlins-noise-erasure-fixture", "version": "0.0.0", "edition": "2021", "license": "MIT"},
        "workspace": {}, "features": {"legacy": []},
        "dependencies": {
            "snow": {"path": str(crates["snow"]), "default-features": False, "features": FEATURES},
            OBSERVER: {"path": str(observer)},
            "zeroize": {"version": "=1.8.2", "default-features": False},
        },
        "patch": {"crates-io": {name: {"path": str(crate)} for name, crate in crates.items()}},
    })
    return directory / "Cargo.toml", legacy


def run_case(directory: Path, case: str, target: Path, log: Path) -> tuple[int, str, bool]:
    manifest, legacy = prepare(directory, case)
    cargo = os.environ.get("CARGO", "cargo")
    # Metadata cannot execute newly selected dependency code. Verify that the
    # fixture selected only versions from the committed, reviewed root lock.
    metadata = subprocess.run([cargo, "metadata", "--manifest-path", str(manifest), "--offline", "--format-version", "1"],
                              cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if metadata.returncode:
        raise RuntimeError("fixture metadata failed: " + metadata.stderr)
    locked = {(entry["name"], entry["version"]) for entry in tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]}
    allowed = locked | {("hidlins-noise-erasure-fixture", "0.0.0"), (OBSERVER, "0.0.0")}
    actual = {(entry["name"], entry["version"]) for entry in json.loads(metadata.stdout)["packages"]}
    if actual - allowed:
        raise RuntimeError(f"unreviewed fixture dependency versions: {actual - allowed}")
    argv = [cargo, "test", "--manifest-path", str(manifest), "--offline", "--locked", "--lib", "--target-dir", str(target)]
    if legacy:
        argv += ["--features", "legacy"]
    result = subprocess.run(argv, cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False)
    log.write_text("argv: " + json.dumps(argv) + "\n" + result.stdout)
    return result.returncode, result.stdout, legacy


def main() -> int:
    scratch = ROOT / ".codex-tmp/noise-erasure"
    scratch.mkdir(parents=True, exist_ok=True)
    evidence = Path(tempfile.mkdtemp(prefix="evidence-", dir=scratch))
    target = scratch / "target"
    with tempfile.TemporaryDirectory(prefix="sources-", dir=scratch) as temporary:
        sources = Path(temporary)
        code, output, legacy = run_case(sources / "positive", "positive", target, evidence / "positive.log")
        if code:
            print(output[-14000:])
            print(f"Actual owned-byte erasure check failed; evidence: {evidence}")
            return 1
        if legacy:
            raise RuntimeError("legacy reset-only SHA unexpectedly passed the byte-erasure regression")
        print("PASS: actual SHA core, backing buffer and scratch bytes; known answers and lifecycle coverage")
        for case in ("core-wipe", "buffer-wipe", "hash-output", "digest-output", "padding", "consume-state", "feature-off"):
            code, output, _ = run_case(sources / case, case, target, evidence / (case + ".log"))
            if not code:
                raise RuntimeError(f"negative control unexpectedly passed: {case}")
            if case == "feature-off":
                if "Zeroize" not in output or "could not compile" not in output:
                    raise RuntimeError("feature-off control did not reject the production erasure trait contract")
            elif "test result: FAILED" not in output:
                raise RuntimeError(f"negative control failed before actual byte assertions: {case}\n{output[-6000:]}")
            print(f"PASS: missing {case} rejected")
    print(f"Erasure evidence: {evidence}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
