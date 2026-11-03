#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 2 ]]; then
  echo "Usage: $0 <isolated-apk> <evidence-file>" >&2
  exit 2
fi

tool_dir="$(cd "$(dirname "$0")" && pwd)"
source "$tool_dir/assert_disposable_android_emulator.sh"
source "$tool_dir/android_package_path.sh"

package='app.serea.android.isolated'
apk="$1"
evidence="$2"
if [[ ! -s "$apk" ]]; then
  echo 'Refusing to install a missing or empty isolated target APK.' >&2
  exit 1
fi

expected_sha="$(sha256sum "$apk" | awk '{print $1}')"
[[ "$expected_sha" =~ ^[0-9a-f]{64}$ ]]

package_paths=''
if ! package_paths="$(android_package_path "$package")"; then
  echo 'Could not safely query the isolated target package; refusing installation.' >&2
  exit 1
fi

installation='already-installed'
if [[ -z "$package_paths" ]]; then
  installation='installed-exact-candidate-apk'
  if ! adb -s "$ANDROID_SERIAL" install "$apk"; then
    echo 'Could not install the candidate isolated target APK on the disposable emulator.' >&2
    exit 1
  fi
  if ! package_paths="$(android_package_path "$package")"; then
    echo 'Could not verify the isolated target package after installation.' >&2
    exit 1
  fi
fi

if [[ -z "$package_paths" || "$package_paths" == *$'\n'* || "$package_paths" != package:* ]]; then
  echo 'The isolated target must resolve to exactly one installed APK path.' >&2
  exit 1
fi
apk_path="${package_paths#package:}"
if [[ ! "$apk_path" =~ ^/[^[:space:]]+$ ]]; then
  echo 'The isolated target APK path is malformed.' >&2
  exit 1
fi

if ! installed_sha="$(adb -s "$ANDROID_SERIAL" shell sha256sum "$apk_path" | awk 'NR == 1 { print $1 }')"; then
  echo 'Could not hash the installed isolated target APK.' >&2
  exit 1
fi
if [[ "$installed_sha" != "$expected_sha" ]]; then
  echo 'Installed isolated target APK does not match the candidate APK; refusing process-death test.' >&2
  exit 1
fi

mkdir -p "$(dirname "$evidence")"
{
  printf 'api=%s\nserial=%s\npackage=%s\ninstallation=%s\n' \
    "$SEREA_OBSERVED_ANDROID_API" "$ANDROID_SERIAL" "$package" "$installation"
  printf 'candidate_apk_sha256=%s\ninstalled_apk_sha256=%s\ninstalled_apk_path=%s\n' \
    "$expected_sha" "$installed_sha" "$apk_path"
} > "$evidence"
printf 'PASS: isolated target package is backed by the exact candidate APK (%s).\n' "$installation"
