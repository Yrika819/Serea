#!/usr/bin/env bash
set -euo pipefail

expected_apk="${1:?expected APK path required}"
package="${2:?package ID required}"
evidence_file="${3:?evidence output path required}"
if [[ ! -s "$expected_apk" || ! "$package" =~ ^[A-Za-z0-9_.]+$ ]]; then
  echo "Expected APK or package ID is invalid." >&2
  exit 2
fi

host_output="$(sha256sum "$expected_apk")" || {
  echo "Host sha256sum failed for expected APK." >&2
  exit 1
}
mapfile -t host_lines <<< "$host_output"
if [[ "${#host_lines[@]}" -ne 1 ]]; then
  echo "Host sha256sum returned multiple output lines." >&2
  exit 1
fi
read -r expected_sha host_path host_extra <<< "${host_lines[0]}"
if [[ ! "$expected_sha" =~ ^[[:xdigit:]]{64}$ || "$host_path" != "$expected_apk" || -n "${host_extra:-}" ]]; then
  echo "Host sha256sum output is malformed." >&2
  exit 1
fi

path_output="$(adb shell pm path "$package" 2>/dev/null | tr -d '\r')" || {
  echo "Package Manager path query failed for $package." >&2
  exit 1
}
mapfile -t path_lines < <(printf '%s\n' "$path_output" | sed '/^[[:space:]]*$/d')
if [[ "${#path_lines[@]}" -ne 1 || ! "${path_lines[0]}" =~ ^package:([^[:space:]]+)$ ]]; then
  echo "Expected exactly one well-formed installed package path; got: $path_output" >&2
  exit 1
fi
installed_path="${BASH_REMATCH[1]}"

device_output="$(adb shell sha256sum "$installed_path" 2>/dev/null)" || {
  echo "Device sha256sum failed for installed APK." >&2
  exit 1
}
mapfile -t device_lines <<< "$device_output"
if [[ "${#device_lines[@]}" -ne 1 ]]; then
  echo "Device sha256sum returned multiple output lines." >&2
  exit 1
fi
read -r actual_sha device_path device_extra <<< "${device_lines[0]}"
if [[ ! "$actual_sha" =~ ^[[:xdigit:]]{64}$ || "$device_path" != "$installed_path" || -n "${device_extra:-}" ]]; then
  echo "Device sha256sum output is malformed." >&2
  exit 1
fi
if [[ "${actual_sha,,}" != "${expected_sha,,}" ]]; then
  echo "Installed APK SHA-256 does not match the expected artifact." >&2
  exit 1
fi

{
  printf 'package=%s\n' "$package"
  printf 'expected_apk=%s\nexpected_sha256=%s\n' "$expected_apk" "${expected_sha,,}"
  printf 'installed_path=%s\ninstalled_sha256=%s\n' "$installed_path" "${actual_sha,,}"
} > "$evidence_file"
echo "installed_apk_sha256=PASS package=$package sha256=${actual_sha,,}"
