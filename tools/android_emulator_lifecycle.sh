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
