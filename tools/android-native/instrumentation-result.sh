#!/usr/bin/env bash

hidlins_device_tests_passed() {
  local result="$1"
  grep -q 'HIDLINS_DEVICE_TESTS 11 passed, 0 failed' <<<"$result" &&
    grep -Fxq 'INSTRUMENTATION_CODE: -1' <<<"$result" &&
    ! grep -Eq 'FAIL |INSTRUMENTATION_FAILED|INSTRUMENTATION_ABORTED' <<<"$result"
}
