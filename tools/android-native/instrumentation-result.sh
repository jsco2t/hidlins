#!/usr/bin/env bash

hidlins_device_ui_diagnostics() {
  local focused_window="" focus_category="other" power_state=""
  # Never publish raw window titles, hierarchy dumps, or application content.
  if focused_window="$("$1" shell dumpsys -t 5 window 2>/dev/null |
      awk '/mCurrentFocus=/ && !seen {print; seen=1}')"; then
    case "$focused_window" in
      *"Application Error"*|*"Not Responding"*) focus_category="error-dialog" ;;
      *permissioncontroller*) focus_category="permission-dialog" ;;
      *com.android.systemui*) focus_category="system-ui" ;;
      *com.android.provision*|*setupwizard*) focus_category="setup" ;;
      *app.hidlins*) focus_category="hidlins" ;;
      *launcher*) focus_category="launcher" ;;
      *mCurrentFocus=null*) focus_category="none" ;;
    esac
  fi
  if power_state="$("$1" shell dumpsys -t 5 power 2>/dev/null |
      awk '/^[[:space:]]*mWakefulness=/ && !seen {print; seen=1}' |
      sed -nE 's/^[[:space:]]*mWakefulness=(Awake|Asleep|Dreaming|Dozing)([[:space:]].*)?$/\1/p')"; then
    case "$power_state" in
      Awake|Asleep|Dreaming|Dozing) ;;
      *) power_state="unknown" ;;
    esac
  else
    power_state="unknown"
  fi
  printf 'focus-window=%s\nwakefulness=%s\n' "$focus_category" "$power_state"
}

hidlins_device_tests_passed() {
  local result="$1"
  grep -q 'HIDLINS_DEVICE_TESTS 11 passed, 0 failed' <<<"$result" &&
    grep -Fxq 'INSTRUMENTATION_CODE: -1' <<<"$result" &&
    ! grep -Eq 'FAIL |INSTRUMENTATION_FAILED|INSTRUMENTATION_ABORTED' <<<"$result"
}
