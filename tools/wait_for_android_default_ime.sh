#!/usr/bin/env bash

# Read-only readiness check for a disposable Android emulator's configured
# keyboard. Boot-completed can precede initialization of this secure setting.
wait_for_android_default_ime() {
  local max_attempts="${1:-30}"
  local attempt ime
  if [[ ! "$max_attempts" =~ ^[1-9][0-9]*$ ]]; then
    return 2
  fi

  SEREA_ANDROID_DEFAULT_IME=''
  SEREA_ANDROID_DEFAULT_IME_ATTEMPTS=0
  for ((attempt = 1; attempt <= max_attempts; attempt++)); do
    ime="$(adb shell settings get secure default_input_method 2>/dev/null | tr -d '\r' || true)"
    SEREA_ANDROID_DEFAULT_IME_ATTEMPTS="$attempt"
    if [[ -n "$ime" && "$ime" != 'null' ]]; then
      SEREA_ANDROID_DEFAULT_IME="$ime"
      return 0
    fi
    if ((attempt < max_attempts)); then
      sleep 1
    fi
  done
  SEREA_ANDROID_DEFAULT_IME="$ime"
  return 1
}
