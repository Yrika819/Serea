#!/usr/bin/env bash
# Source this guard before any Android test that mutates app-private data.
# It deliberately leaves the caller's shell options unchanged.

guard_fail() {
  echo "$*" >&2
  return 1
}

if [[ "${CI:-}" != "true" || "${GITHUB_ACTIONS:-}" != "true" || \
      "${SEREA_DISPOSABLE_CI_EMULATOR:-}" != "1" || \
      ! "${GITHUB_RUN_ID:-}" =~ ^[0-9]+$ ]]; then
  guard_fail "Refusing Android data mutation outside the explicitly designated disposable GitHub Actions emulator."
  return 1
fi

expected_api="${SEREA_EXPECTED_ANDROID_API:-}"
if [[ ! "$expected_api" =~ ^(26|35)$ ]]; then
  guard_fail "SEREA_EXPECTED_ANDROID_API must explicitly select API 26 or 35; refusing Android data mutation."
  return 1
fi

if ! command -v adb >/dev/null; then
  guard_fail "adb is unavailable; refusing Android data mutation."
  return 1
fi
if ! devices_output="$(adb devices)"; then
  guard_fail "Could not enumerate Android devices; refusing Android data mutation."
  return 1
fi
mapfile -t device_rows < <(printf '%s\n' "$devices_output" | awk 'NR > 1 && NF >= 2 { print $1 "\t" $2 }')
if [[ "${#device_rows[@]}" -ne 1 ]]; then
  guard_fail "Expected exactly one connected Android device; refusing to choose an unknown target."
  return 1
fi

IFS=$'\t' read -r serial state <<< "${device_rows[0]}"
if [[ "$state" != "device" || ! "$serial" =~ ^emulator-[0-9]+$ ]]; then
  guard_fail "Refusing non-emulator or unauthorized Android target '$serial' ('$state')."
  return 1
fi
if [[ -n "${ANDROID_SERIAL:-}" && "$ANDROID_SERIAL" != "$serial" ]]; then
  guard_fail "ANDROID_SERIAL does not match the only connected emulator."
  return 1
fi

if ! sdk_output="$(adb -s "$serial" shell getprop ro.build.version.sdk)" || \
   ! qemu_output="$(adb -s "$serial" shell getprop ro.kernel.qemu)"; then
  guard_fail "Could not verify emulator API and qemu properties; refusing Android data mutation."
  return 1
fi
sdk="${sdk_output//$'\r'/}"
qemu="${qemu_output//$'\r'/}"
if [[ "$sdk" != "$expected_api" || "$qemu" != "1" ]]; then
  guard_fail "Refusing emulator: expected API=$expected_api and ro.kernel.qemu=1; observed API=$sdk qemu=$qemu."
  return 1
fi

export ANDROID_SERIAL="$serial"
echo "Disposable emulator accepted: serial=$serial api=$sdk qemu=$qemu run_id=$GITHUB_RUN_ID"
