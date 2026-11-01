#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
# This test always expects the dedicated API 35 disposable emulator. Source the
# guard before creating outputs, keys, installing APKs, or mutating app data.
export SEREA_EXPECTED_ANDROID_API=35
source "$repo_root/tools/assert_disposable_android_emulator.sh"
adb_cmd=(adb -s "$ANDROID_SERIAL")

android_dir="$repo_root/android"
test_root=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/serea-update-test.XXXXXX")
results="$android_dir/app/build/outputs/androidTest-results/update-compatibility.txt"
package="app.serea.android.updatecompat"
activity="$package/app.serea.android.MainActivity"
receiver="$package/app.serea.android.ProcessDeathHarnessReceiver"
action="app.serea.android.action.PROCESS_DEATH_HARNESS"
run_token="upgrade-${GITHUB_RUN_ID:-local}-$$-$(date +%s)"
store_file="$test_root/update-test.p12"
alias="serea-update-test"
store_password=$(openssl rand -hex 32)
key_password="$store_password"
export SEREA_UPDATE_TEST_KEYSTORE_PATH="$store_file"
export SEREA_UPDATE_TEST_STORE_PASSWORD="$store_password"
export SEREA_UPDATE_TEST_KEY_PASSWORD="$key_password"
export SEREA_UPDATE_TEST_KEY_ALIAS="$alias"

cleanup() {
  rm -rf "$test_root"
}
trap cleanup EXIT

command -v keytool >/dev/null
command -v openssl >/dev/null
cd "$android_dir"

keytool -genkeypair -noprompt \
  -alias "$alias" \
  -keystore "$store_file" \
  -storetype PKCS12 \
  -keyalg RSA -keysize 2048 -validity 2 \
  -dname "CN=Serea ephemeral Android update test" \
  -storepass:env SEREA_UPDATE_TEST_STORE_PASSWORD \
  -keypass:env SEREA_UPDATE_TEST_KEY_PASSWORD \
  >/dev/null 2>&1

expected_cert=$(LC_ALL=C keytool -list -v -keystore "$store_file" -storetype PKCS12 \
  -storepass:env SEREA_UPDATE_TEST_STORE_PASSWORD -alias "$alias" 2>/dev/null \
  | awk -F ': ' '/SHA256:/{print $2; exit}' | tr -d ':[:space:]' | tr '[:lower:]' '[:upper:]')
test "${#expected_cert}" -eq 64

apk_v1="$test_root/serea-update-v1.apk"
apk_v2="$test_root/serea-update-v2.apk"
build_tools="$ANDROID_HOME/build-tools/35.0.0"

build_version() {
  local version_code="$1"
  ./gradlew --no-daemon --no-build-cache --no-configuration-cache --rerun-tasks :app:assembleDebug \
    "-PsereaVersionCode=$version_code" -PsereaUpdateCompatibilityTest=true
  cp "$android_dir/app/build/outputs/apk/debug/app-debug.apk" "$2"
  "$build_tools/apksigner" verify --verbose --print-certs "$2" > "$test_root/apksigner-$version_code.txt"
  actual_cert=$(awk -F ': ' '/Signer #1 certificate SHA-256 digest:/{print $2; exit}' "$test_root/apksigner-$version_code.txt" \
    | tr -d ':[:space:]' | tr '[:lower:]' '[:upper:]')
  test "$actual_cert" = "$expected_cert"
  "$build_tools/aapt" dump badging "$2" > "$test_root/badging-$version_code.txt"
  grep -F "package: name='$package' versionCode='$version_code'" "$test_root/badging-$version_code.txt" >/dev/null
}

build_version 100 "$apk_v1"
apk_v1_sha=$(sha256sum "$apk_v1" | awk '{print $1}')
"${adb_cmd[@]}" uninstall "$package" >/dev/null 2>&1 || true
"${adb_cmd[@]}" install -r "$apk_v1"

broadcast_phase() {
  local phase="$1"
  "${adb_cmd[@]}" shell am broadcast -n "$receiver" -a "$action" --es phase "$phase" --es token "$run_token" >/dev/null
  for _ in $(seq 1 30); do
    if "${adb_cmd[@]}" logcat -d -s SereaProcessDeath:I | grep -F "PASS:$run_token:$phase" >/dev/null; then return 0; fi
    if "${adb_cmd[@]}" logcat -d -s SereaProcessDeath:E | grep -F "FAIL:$run_token:$phase:" >/dev/null; then
      "${adb_cmd[@]}" logcat -d -s SereaProcessDeath:E >&2
      return 1
    fi
    sleep 1
  done
  echo "Update compatibility harness phase $phase did not report completion." >&2
  "${adb_cmd[@]}" logcat -d -s SereaProcessDeath:I SereaProcessDeath:E >&2
  return 1
}

"${adb_cmd[@]}" logcat -c
broadcast_phase prepare
"${adb_cmd[@]}" shell am force-stop "$package"
"${adb_cmd[@]}" shell am start -W -n "$activity" >/dev/null
sleep 2
"${adb_cmd[@]}" logcat -c
broadcast_phase verify

build_version 101 "$apk_v2"
apk_v2_sha=$(sha256sum "$apk_v2" | awk '{print $1}')
"${adb_cmd[@]}" install -r "$apk_v2"
installed_version=$("${adb_cmd[@]}" shell dumpsys package "$package" | sed -n "s/.*versionCode=\([0-9][0-9]*\).*/\1/p" | head -1 | tr -d '\r')
test "$installed_version" = 101

"${adb_cmd[@]}" shell am force-stop "$package"
"${adb_cmd[@]}" shell am start -W -n "$activity" >/dev/null
sleep 2
"${adb_cmd[@]}" logcat -c
broadcast_phase verify

mkdir -p "$(dirname "$results")"
{
  echo "application_id=$package"
  echo 'version_code_v1=100'
  echo 'version_code_v2=101'
  echo 'apk_signature_verification=PASS'
  echo "test_certificate_sha256=$expected_cert"
  echo "apk_v1_sha256=$apk_v1_sha"
  echo "apk_v2_sha256=$apk_v2_sha"
  echo 'adb_install_replace=PASS'
  echo 'room_encrypted_history_settings_keystore_reference=PASS'
  echo 'process_death_recovery_after_update=PASS'
  echo 'test_key=ephemeral_ci_only_not_for_distribution'
} > "$results"
cat "$results" >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
cat "$results"
