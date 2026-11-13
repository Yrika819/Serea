#!/usr/bin/env bash

# Retry only transient ADB transport failures while restarting adbd on a disposable Emulator.
# Unsupported-root and other permanent errors fail immediately; the caller still verifies API,
# QEMU, boot completion, and root UID after this command succeeds.
serea_retry_adb_root() {
  local serial="$1"
  local max_attempts="${2:-5}"
  local attempt output

  for ((attempt = 1; attempt <= max_attempts; attempt++)); do
    if output=$(timeout 10s adb -s "$serial" root 2>&1); then
      [[ -z "$output" ]] || printf '%s\n' "$output"
      return 0
    fi

    if grep -Eiq 'unable to connect for root: closed|device offline|device .*not found|no devices/emulators found|connection.*closed' <<< "$output"; then
      printf 'RETRY: transient ADB transport failure during root restart (%s/%s): %s\n' \
        "$attempt" "$max_attempts" "$output" >&2
      timeout 5s adb -s "$serial" reconnect offline >/dev/null 2>&1 || true
      if (( attempt < max_attempts )); then sleep 2; fi
      continue
    fi

    printf 'ADB root restart failed with a non-transient error: %s\n' "$output" >&2
    return 1
  done

  printf 'ADB root restart remained unavailable after %s transient transport attempts.\n' \
    "$max_attempts" >&2
  return 1
}

# Restart adbd and prove root by inspecting the device's effective shell UID. The
# adb root message is diagnostic only: some adb/platform-tool versions return no
# text on success. All waits and transport retries are bounded for CI.
serea_restart_adbd_and_require_root() {
  local serial="$1"
  local evidence_file="$2"
  local max_attempts="${3:-5}"
  local attempt output reconnect_output uid_output
  : > "$evidence_file"
  for ((attempt = 1; attempt <= max_attempts; attempt++)); do
    if output=$(timeout 10s adb -s "$serial" root 2>&1); then
      printf 'adb_root_exit=0\nattempt=%s\n' "$attempt" >> "$evidence_file"
      if [[ -n "$output" ]]; then printf '%s\n' "$output" >> "$evidence_file"; fi
      break
    fi
    printf 'adb_root_exit=nonzero\nattempt=%s\n' "$attempt" >> "$evidence_file"
    if ! grep -Eiq 'unable to connect for root: closed|device offline|device .*not found|no devices/emulators found|connection.*closed|cannot.*root|cannot run as root' <<< "$output"; then
      printf 'FAIL: adb root rejected: %s\n' "$output" >&2
      return 1
    fi
    printf 'transient_adb_root_error=%s\n' "$output" >> "$evidence_file"
    if (( attempt == max_attempts )); then return 1; fi
    timeout 5s adb -s "$serial" reconnect offline > "$evidence_file.reconnect" 2>&1 || true
    for _ in {1..10}; do
      if timeout 5s adb -s "$serial" get-state > "$evidence_file.state" 2>&1 &&
          grep -Fxq device "$evidence_file.state"; then break; fi
      sleep 1
    done
    if ! grep -Fxq device "$evidence_file.state"; then
      printf 'FAIL: ADB did not reconnect after bounded wait.\n' >&2
      return 1
    fi
  done

  for attempt in {1..10}; do
    if uid_output=$(timeout 5s adb -s "$serial" shell id 2>&1); then
      printf 'shell_id_attempt=%s\n%s\n' "$attempt" "$uid_output" >> "$evidence_file"
      if grep -Eq '(^|[[:space:]])uid=0\(root\)([[:space:]]|$)' <<< "${uid_output//$'\r'/}"; then
        printf 'root_uid_verified=true\n' >> "$evidence_file"
        return 0
      fi
      if grep -Eq '(^|[[:space:]])uid=[0-9]+' <<< "$uid_output"; then
        printf 'FAIL: adb shell UID is not root.\n' >&2
        return 1
      fi
    else
      printf 'shell_id_transport_error_attempt=%s\n%s\n' "$attempt" "$uid_output" >> "$evidence_file"
    fi
    if (( attempt < 10 )); then
      timeout 5s adb -s "$serial" reconnect offline > "$evidence_file.reconnect" 2>&1 || true
      sleep 1
    fi
  done
  printf 'FAIL: root UID was not verified after bounded ADB reconnect attempts.\n' >&2
  return 1
}
