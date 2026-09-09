# Local-sync fuzz dependency review

This workspace is development/testing infrastructure only. It is excluded from
the root Cargo workspace, has its own lockfile and license policy, and is never
used by a Hidlins application target.

## Pins and purpose

| Item | Exact pin | License | Purpose |
|---|---:|---|---|
| `cargo-fuzz` tool | 0.13.2 | MIT OR Apache-2.0 | Standard Rust driver for bounded libFuzzer campaigns. Installed outside the repository by `make fuzz-toolchain`. |
| Rust nightly | `nightly-2026-09-01` | Rust toolchain licenses | Compiler instrumentation required by `cargo-fuzz`; never used for product builds. |
| `libfuzzer-sys` crate | 0.4.13 | (MIT OR Apache-2.0) AND NCSA | Links LLVM's bundled libFuzzer runtime into the three test executables only. |

`libfuzzer-sys` is a Rust crate and is the only direct fuzz-runtime dependency.
Hidlins does not depend directly on a separately installed LLVM library. The
crate's default `link_libfuzzer` feature compiles the bundled upstream LLVM
libFuzzer source under NCSA; that code exists only in fuzz executables.

## Footprint and maintenance assessment

- Upstream is the Rust Fuzz project (`rust-fuzz/libfuzzer`); 0.4.13 was
  released in June 2026 and is the current audited pin for this work package.
- The vendored crate is 67 files / about 652 KiB. Its runtime graph uses
  existing `arbitrary` and `cc`; `cc`'s parallel build adds `jobserver` to the
  isolated lock. Seeding the isolated lock from `Cargo.lock` prevents duplicate
  versions of the production dependency graph.
- Enabled features are the crate default (`link_libfuzzer`) only. The optional
  dependency is activated solely by the fuzz binaries' `fuzzing` feature;
  stable corpus replay leaves it disabled.
- A small in-repository replacement is not appropriate: coverage-guided fuzz
  instrumentation and sanitizer integration are compiler/runtime facilities,
  not a minor parser helper. Hand-maintaining a fuzz engine would be less
  effective and substantially harder to audit. Existing property/unit tests
  remain the stable first line and replay every committed crash input.
- Other fuzz engines would add another runtime/toolchain and would not remove
  the need to audit native instrumentation code. They provide no narrower fit
  than the Rust ecosystem's standard `cargo-fuzz` path.

## Enforced shipping boundary

Root `deny.toml` does not allow NCSA. `fuzz/deny.toml` permits NCSA only for
`libfuzzer-sys@0.4.13`. `make ncsa-boundary-check` verifies both policies,
workspace and lockfile separation, the resolved production graph, packaging
hooks, and representative contaminated application artifacts. Every product
packaging target performs a graph precheck and scans its output afterward.
