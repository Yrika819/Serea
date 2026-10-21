#!/usr/bin/env bash
# Source this guard before any Android test that mutates app-private data. It
# deliberately leaves the caller's shell options unchanged.

if [[ "${CI:-}" != "true" || "${GITHUB_ACTIONS:-}" != "true" || \
      "${SEREA_DISPOSABLE_CI_EMULATOR:-}" != "1" ]]; then
  echo "Refusing data-mutating Android tests outside the explicitly designated disposable GitHub Actions emulator." >&2
  exit 1
fi

if ! command -v adb >/dev/null; then
  echo "adb is unavailable; refusing Android data mutation." >&2
  exit 1
fi
device_rows="$(adb devices | awk 'NR > 1 && NF >= 2 { print $1 ":" $2 }')"
if [[ "$(printf '%s\n' "$device_rows" | sed '/^$/d' | wc -l | tr -d ' ')" != 1 ]]; then
  echo "Expected exactly one connected Android device; refusing to choose an unknown target." >&2
  adb devices -l >&2
  exit 1
fi

serial="${device_rows%%:*}"
state="${device_rows#*:}"
if [[ "$state" != "device" || ! "$serial" =~ ^emulator-[0-9]+$ ]]; then
  echo "Refusing non-emulator or unauthorized Android target '$serial' ('$state')." >&2
  adb devices -l >&2
  exit 1
fi
if [[ -n "${ANDROID_SERIAL:-}" && "$ANDROID_SERIAL" != "$serial" ]]; then
  echo "ANDROID_SERIAL does not match the only connected emulator." >&2
  exit 1
fi
export ANDROID_SERIAL="$serial"

sdk="$(adb shell getprop ro.build.version.sdk | tr -d '\r')"
qemu="$(adb shell getprop ro.kernel.qemu | tr -d '\r')"
if [[ ( "$sdk" != "26" && "$sdk" != "35" ) || "$qemu" != "1" ]]; then
  echo "Refusing emulator: only API 26/35 qemu emulators are accepted; observed API=$sdk qemu=$qemu." >&2
  exit 1
fi
echo "Disposable emulator accepted: serial=$serial api=$sdk qemu=$qemu run_id=${GITHUB_RUN_ID:-missing}"
