#!/usr/bin/env bash
set -euo pipefail

apk_path="${1:?usage: verify_personal_beta_apk.sh APK VERSION_CODE CERT_SHA256 [LABEL]}"
expected_version="${2:?versionCode is required}"
expected_fingerprint="${3:?expected certificate SHA-256 is required}"
expected_label="${4:-Serea Personal Beta}"
build_tools_version="${BUILD_TOOLS_VERSION:-35.0.0}"
android_home="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
if [[ -z "$android_home" ]]; then
  echo 'ANDROID_HOME or ANDROID_SDK_ROOT is required.' >&2
  exit 2
fi
aapt="$android_home/build-tools/$build_tools_version/aapt"
apksigner="$android_home/build-tools/$build_tools_version/apksigner"
for tool in "$aapt" "$apksigner"; do
  if [[ ! -x "$tool" ]]; then
    echo "Android build tool is unavailable: $tool" >&2
    exit 2
  fi
done
if [[ ! -s "$apk_path" ]]; then
  echo "Personal Beta APK is missing or empty: $apk_path" >&2
  exit 1
fi
if [[ ! "$expected_version" =~ ^[1-9][0-9]*$ || "$expected_version" -le 1 || "$expected_version" -gt 2147483647 ]]; then
  echo 'Personal Beta versionCode must be an integer greater than 1.' >&2
  exit 1
fi

normalize_fingerprint() {
  tr -d ':[:space:]' | tr '[:lower:]' '[:upper:]'
}
expected_fingerprint=$(printf '%s' "$expected_fingerprint" | normalize_fingerprint)
if [[ ! "$expected_fingerprint" =~ ^[0-9A-F]{64}$ ]]; then
  echo 'Expected certificate fingerprint must be 64 hexadecimal characters.' >&2
  exit 1
fi

temp_dir=$(mktemp -d "${TMPDIR:-/tmp}/serea-personal-beta-verify.XXXXXX")
trap 'rm -rf "$temp_dir"' EXIT
badging="$temp_dir/badging.txt"
signature="$temp_dir/signature.txt"
"$aapt" dump badging "$apk_path" > "$badging"
grep -F "package: name='app.serea.android.beta' versionCode='$expected_version'" "$badging" >/dev/null
grep -F "application-label:'$expected_label'" "$badging" >/dev/null
"$apksigner" verify --verbose --print-certs "$apk_path" > "$signature"
actual_fingerprint=$(awk -F ': ' '/Signer #1 certificate SHA-256 digest:/{print $2; exit}' "$signature" | normalize_fingerprint)
if [[ "$actual_fingerprint" != "$expected_fingerprint" ]]; then
  echo 'APK signer fingerprint did not match the required Personal Beta certificate.' >&2
  exit 1
fi
echo "Personal Beta APK verified: applicationId=app.serea.android.beta versionCode=$expected_version label=$expected_label certificateSHA256=$actual_fingerprint"
