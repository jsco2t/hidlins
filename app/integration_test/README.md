# Real-bridge integration tests

The executable desktop integration suites live in
[`../test_bridge/integration/`](../test_bridge/integration/) and run through
`make app-test-integration` / `make app-test-integration-minio`.

They are kept beside the existing headless bridge suite because Flutter treats
every Dart file under this conventional directory as a device-driver test and
requires `integration_test → flutter_driver → webdriver`. Hidlins' D1 cases
open the host cdylib directly and need no device protocol; retaining the
`flutter_test` harness avoids adding a network-capable WebDriver dependency to
the vendored pub closure while exercising the same production bridge and Rust
session code.
