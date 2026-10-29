#!/usr/bin/env bash
set -euo pipefail

target_apk="${1:?target APK required}"
partition="${2:?partition required}"
token="${3:?campaign token required}"
evidence_file="${4:?evidence output required}"
tool_dir="${5:?tool directory required}"
api_level="${6:?API level required}"

if [[ "$partition" != offline && "$partition" != http ]]; then
  echo "Invalid instrumentation partition '$partition'." >&2
  exit 2
fi
if [[ ! "$api_level" =~ ^(26|35)$ || ! "$token" =~ ^[A-Za-z0-9-]{1,120}$ ]]; then
  echo "Invalid API level or campaign token." >&2
  exit 2
fi
if [[ ! -s "$target_apk" ]]; then
  echo "Target APK is missing or empty." >&2
  exit 1
fi

target_package=app.serea.android.isolated
test_package=app.serea.android.isolated.test
for package in "$test_package" "$target_package"; do
  existing="$(adb shell pm path "$package" 2>/dev/null | tr -d '\r')" || exit 1
  if [[ -n "$existing" ]]; then
    adb uninstall "$package" >/dev/null || exit 1
  fi
  remaining="$(adb shell pm path "$package" 2>/dev/null | tr -d '\r')" || exit 1
  if [[ -n "$remaining" ]]; then
    echo "Disposable package $package remains installed after uninstall." >&2
    exit 1
  fi
done

if ! adb install "$target_apk"; then
  echo "Could not install selected $partition target APK." >&2
  exit 1
fi
path_output="$(adb shell pm path "$target_package" 2>/dev/null | tr -d '\r')" || exit 1
mapfile -t path_lines < <(printf '%s\n' "$path_output" | sed '/^[[:space:]]*$/d')
if [[ "${#path_lines[@]}" -ne 1 || ! "${path_lines[0]}" =~ ^package:([^[:space:]]+)$ ]]; then
  echo "Expected exactly one well-formed target package path; got: $path_output" >&2
  exit 1
fi
installed_path="${BASH_REMATCH[1]}"

host_digest_output="$(sha256sum "$target_apk")" || {
  echo "Host sha256sum failed for target APK." >&2
  exit 1
}
read -r expected_sha host_reported_path extra <<< "$host_digest_output"
if [[ ! "$expected_sha" =~ ^[[:xdigit:]]{64}$ || -z "$host_reported_path" || -n "${extra:-}" ]]; then
  echo "Host sha256sum returned malformed target APK evidence." >&2
  exit 1
fi
device_digest_output="$(adb shell sha256sum "$installed_path" 2>/dev/null)" || {
  echo "Device sha256sum failed for installed target APK." >&2
  exit 1
}
read -r actual_sha device_reported_path extra <<< "$device_digest_output"
if [[ ! "$actual_sha" =~ ^[[:xdigit:]]{64}$ || "$device_reported_path" != "$installed_path" || -n "${extra:-}" ]]; then
  echo "Device sha256sum returned malformed installed target evidence." >&2
  exit 1
fi
if [[ "${actual_sha,,}" != "${expected_sha,,}" ]]; then
  echo "Installed target APK digest does not match the selected APK." >&2
  exit 1
fi

EXPECTED_API_LEVEL="$api_level" bash "$tool_dir/prepare_isolated_instrumentation_target.sh" "$token" \
  > "${evidence_file%.txt}-seed.log" 2>&1
{
  printf 'source_sha=%s\n' "$(git rev-parse HEAD)"
  printf 'tree_sha=%s\n' "$(git rev-parse 'HEAD^{tree}')"
  printf 'api_level=%s\npartition=%s\n' "$api_level" "$partition"
  printf 'target_package=%s\n' "$target_package"
  printf 'target_apk=%s\ntarget_apk_sha256=%s\n' "$target_apk" "${expected_sha,,}"
  printf 'installed_target_apk_sha256=%s\ninstalled_target_path=%s\n' "${actual_sha,,}" "$installed_path"
} > "$evidence_file"
echo "partition_target=PASS partition=$partition api=$api_level sha256=${actual_sha,,}"
