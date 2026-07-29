# rust_builder

Cargokit glue that builds `crates/hidlins-api` into each platform's native
artifact (CMake on Linux, CocoaPods on macOS/iOS, Gradle on Android).

**This folder is not boilerplate to skim past.** The vendored Cargokit tree
under `cargokit/` carries eight local Hidlins patches — including one that fixes
a macOS/iOS link failure and one that removed an environment dump leaking
signing credentials into build logs. Read
[`tools/dev/cargokit-patches/README.md`](../../tools/dev/cargokit-patches/README.md)
before upgrading or re-instantiating it, and keep the `Hidlins patch` markers.
