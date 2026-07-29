# Cargokit local patches

`app/rust_builder/cargokit/` is vendored third-party build glue, instantiated by
`flutter_rust_bridge_codegen integrate` from the **frb 2.12.0** template. Hidlins
carries local modifications to it.

## Why this directory exists

CONTRIBUTING's recorded upgrade policy is "re-instantiate from a new frb template
version, re-apply patches, re-vendor build-tool deps." That is only executable if
the patch set is discoverable. It previously was not: four of the six edits
carried an in-source `Hidlins patch` marker and one — the `libraryName` change in
`artifacts_provider.dart`, which is what makes artifact lookup resolve
`libhidlins_api` at all — carried none. A maintainer grepping for "Hidlins" after
re-instantiating would have restored four patches and silently dropped that one.

## Invariant

**Every Hidlins edit inside the vendored Cargokit tree carries a `Hidlins patch`
(or `HIDLINS PATCH`) marker comment.** `PATCHED-FILES.txt` in this directory is
the generated inventory.

Regenerate it after touching the tree:

```sh
grep -rn "Hidlins patch\|HIDLINS PATCH" app/rust_builder/cargokit/ \
  --include="*.dart" --include="*.sh" \
  | sed 's/:.*//' | sort -u > tools/dev/cargokit-patches/PATCHED-FILES.txt
```

## Upgrade procedure

1. Note the current frb template version and the files in `PATCHED-FILES.txt`.
2. Re-instantiate Cargokit from the new frb template.
3. `git diff` the vendored tree — every hunk that disappears is a patch to
   re-apply.
4. Re-apply each, keeping the marker comments.
5. Re-vendor the build-tool pub cache (`build_tool/vendor-pub/`).
6. Regenerate `PATCHED-FILES.txt` and update the CONTRIBUTING review entry.

## The patches

| File | Change | Why |
| --- | --- | --- |
| `build_tool/lib/src/cargo.dart` | `CrateInfo` tracks `libraryName` alongside `packageName`; reads `[lib] name`, else normalizes hyphens to underscores | package is `hidlins-api`, library is `hidlins_api` |
| `build_tool/lib/src/artifacts_provider.dart` | artifact lookup uses `libraryName` | otherwise no artifact is found |
| `build_tool/lib/src/build_pod.dart` | lipo output name uses `libraryName` | emitted `libhidlins-api.a` while both podspecs `-force_load libhidlins_api.a`; **broke every macOS/iOS link** |
| `build_tool/lib/src/precompile_binaries.dart` | `libraryName` parameter fed `libraryName` | same class of bug, currently dormant |
| `build_tool/lib/src/verify_binaries.dart` | same | same |
| `build_tool/lib/src/builder.dart` | passes `--offline --locked` to cargo | vendored/offline build posture |
| `run_build_tool.sh` | vendored `PUB_CACHE` + `dart pub get --offline` | vendored/offline build posture |
| `build_pod.sh` | removed a bare `env` dump | it printed the whole environment — signing credentials, CI tokens, keychain vars — into build logs |
