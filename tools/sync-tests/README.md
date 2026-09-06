# s3-sync integration-test fixtures

This directory holds the throwaway-backend fixtures the s3-sync live-wire
tests run against. The unit tests (`cargo test`) need none of this — they
use `MockS3Client` / `MockHttpClient` / `MemoryTransport`. These fixtures
exist for the **MinIO live-wire tests** (`crates/hidlins-sync/tests/minio_integration.rs`),
which are `#[ignore]`-gated so they only run when you ask for them.

MinIO is the *strict* SigV4 implementation: it rejects canonical-request
encoding bugs that AWS's permissive parser silently accepts. That's why it,
not AWS, is the PR-CI backend (see the implementation plan §8.4.2).

## Prerequisites

- **Docker** (or Podman) — to run the MinIO server container.
- **`mc`** (the MinIO client) — to create test buckets.
  Install: <https://min.io/docs/minio/linux/reference/minio-mc.html>
- The MinIO image tag is pinned — see [`fixtures/MINIO_VERSION.md`](fixtures/MINIO_VERSION.md).

## Local workflow

```sh
make minio-up              # start the pinned MinIO container, wait for health
make test-s3-integration   # run the #[ignore]-gated MINIO-* tests
make minio-down            # stop + remove the container
make test-minio-managed    # clean Rust + real bridge run, then teardown
```

`make minio-up` runs `fixtures/start_minio.sh`, which:

- starts `minio/minio:<pinned>` bound to `127.0.0.1:9000`
  (override the port with `HIDLINS_MINIO_PORT=NNNN make minio-up`),
- waits for the `/minio/health/live` probe,
- writes `fixtures/.minio-env` (git-ignored) with the endpoint + the
  test-only credentials.

`make test-s3-integration` sources `fixtures/.minio-env` so the test process
sees `HIDLINS_MINIO_ENDPOINT` / `HIDLINS_MINIO_ACCESS_KEY` /
`HIDLINS_MINIO_SECRET_KEY` / `HIDLINS_MINIO_REGION`. Each test creates its
own randomly-suffixed bucket (via `fixtures/make_bucket.sh`) so parallel
runs don't collide.

For a controlled physical-device lab, the same fixture can bind to a LAN
interface and bootstrap a named bucket independently of Hidlins:

```sh
HIDLINS_MINIO_BIND=0.0.0.0 \
HIDLINS_MINIO_ADVERTISE_HOST=192.0.2.10 \
HIDLINS_MINIO_PORT=9000 make minio-up
make minio-bucket MINIO_BUCKET=hidlins-device-test
```

Loopback remains the default. A wildcard bind requires an explicit advertised
host reachable by the device; these fixed test credentials must never be
exposed outside a controlled disposable environment.

## Test-only credentials

`MINIO_ROOT_USER=hidlins-test` / `MINIO_ROOT_PASSWORD=hidlins-test-secret`
guard a disposable container with no real data. They are deliberately
well-known. **Never reuse them for anything real.**

## CI

The `integration-s3` job in `.github/workflows/ci.yml` runs these tests on
Linux only (GitHub's macOS runners have no Docker). It calls
`make test-minio-managed`, which runs both the Rust live-wire suite and the
real Flutter bridge suite with deterministic teardown.
