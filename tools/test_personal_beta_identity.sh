#!/usr/bin/env bash
set -euo pipefail

if [[ "${SEREA_PERSONAL_BETA_DISPOSABLE_EMULATOR:-}" != "1" ]]; then
  echo 'Set SEREA_PERSONAL_BETA_DISPOSABLE_EMULATOR=1 only for a throwaway API 35 emulator.' >&2
  exit 2
fi
command -v adb >/dev/null
command -v keytool >/dev/null
command -v openssl >/dev/null
command -v strings >/dev/null

device_rows=$(adb devices | awk 'NR > 1 && NF >= 2 { print $1 ":" $2 }')
if [[ "$(printf '%s\n' "$device_rows" | sed '/^$/d' | wc -l | tr -d ' ')" != 1 ]]; then
  echo 'Expected exactly one connected disposable emulator.' >&2
  adb devices -l >&2
  exit 2
fi
serial="${device_rows%%:*}"
if [[ "${device_rows#*:}" != device || ! "$serial" =~ ^emulator-[0-9]+$ ]]; then
  echo "Refusing non-emulator Android target '$serial'." >&2
  exit 2
fi
export ANDROID_SERIAL="$serial"
if [[ "$(adb shell getprop ro.build.version.sdk | tr -d '\r')" != 35 ||
      "$(adb shell getprop ro.kernel.qemu | tr -d '\r')" != 1 ]]; then
  echo 'Refusing emulator: the Personal Beta identity test requires an API 35 QEMU emulator.' >&2
  exit 2
fi
adb root >/dev/null
adb wait-for-device
if [[ "$(adb shell id -u | tr -d '\r')" != 0 ]]; then
  echo 'The disposable emulator must allow adb root to preserve-check Demo app data.' >&2
  exit 2
fi

wait_for_package_manager() {
  for _ in $(seq 1 60); do
    local package_listing
    package_listing=$(adb shell pm list packages -3 2>&1 | tr -d '\r' || true)
    if [[ "$package_listing" != *"Can't find service: package"* ]]; then return 0; fi
    sleep 2
  done
  echo 'Android Package Manager did not become available on the disposable emulator.' >&2
  return 1
}
wait_for_package_manager

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
android_dir="$repo_root/android"
test_root=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/serea-personal-beta.XXXXXX")
results="$android_dir/app/build/outputs/androidTest-results/personal-beta-identity.txt"
demo_package='app.serea.android.demo'
beta_package='app.serea.android.beta'
beta_activity="$beta_package/app.serea.android.MainActivity"
beta_receiver="$beta_package/app.serea.android.ProcessDeathHarnessReceiver"
action='app.serea.android.action.PROCESS_DEATH_HARNESS'
token="personal-beta-$$-$(date +%s)"
build_tools_version="${BUILD_TOOLS_VERSION:-35.0.0}"
build_tools="${ANDROID_HOME:-${ANDROID_SDK_ROOT:?ANDROID_HOME or ANDROID_SDK_ROOT is required}}/build-tools/$build_tools_version"
aapt="$build_tools/aapt"
apksigner="$build_tools/apksigner"

cleanup() { rm -rf "$test_root"; }
trap cleanup EXIT

mkdir -p "$(dirname "$results")"
printf 'NOT_RUN\n' > "$results"

beta_existing_path=$(adb shell pm path "$beta_package" | tr -d '\r')
if [[ -n "$beta_existing_path" ]]; then
  echo 'Refusing an emulator that already contains app.serea.android.beta; use a fresh disposable AVD.' >&2
  exit 2
fi

normalize_fingerprint() { tr -d ':[:space:]' | tr '[:lower:]' '[:upper:]'; }
apk_fingerprint() {
  "$apksigner" verify --verbose --print-certs "$1" \
    | awk -F ': ' '/Signer #1 certificate SHA-256 digest:/{print $2; exit}' \
    | normalize_fingerprint
}
installed_apk_path() {
  adb shell pm path "$demo_package" | tr -d '\r' | sed -n 's/^package://p' | head -1
}
demo_data_snapshot() {
  adb shell 'for d in databases shared_prefs files no_backup; do p=/data/user/0/app.serea.android.demo/$d; if [ -d "$p" ]; then find "$p" -type f -exec sha256sum {} \;; fi; done' \
    | tr -d '\r' | LC_ALL=C sort
}
demo_package_version() {
  adb shell dumpsys package "$demo_package" \
    | awk '/versionCode=/{print; exit}' | tr -d '\r'
}
verify_demo_unchanged() {
  [[ -n "$(adb shell pm path "$demo_package" | tr -d '\r')" ]]
  [[ "$(installed_apk_path)" == "$demo_apk_path_before" ]]
  [[ "$(demo_package_version)" == "$demo_version_before" ]]
  adb pull "$demo_apk_path_before" "$test_root/demo-after.apk" >/dev/null
  [[ "$(apk_fingerprint "$test_root/demo-after.apk")" == "$demo_fingerprint_before" ]]
  [[ "$(demo_data_snapshot)" == "$demo_data_before" ]]
}

phase() {
  local phase_name="$1"
  local request_id="beta-${phase_name}-$(date +%s%N)-${RANDOM}"
  adb logcat -c
  adb shell am broadcast -f 0x20 -n "$beta_receiver" -a "$action" \
    --es phase "$phase_name" --es token "$token" --es requestId "$request_id" >/dev/null
  for _ in $(seq 1 45); do
    local result_log
    result_log=$(adb logcat -d -s SereaProcessDeath:V)
    if grep -F "FAIL:$token:$phase_name:$request_id:" <<< "$result_log" >/dev/null; then
      printf '%s\n' "$result_log" >&2
      return 1
    fi
    if grep -F "PASS:$token:$phase_name:$request_id" <<< "$result_log" >/dev/null; then return 0; fi
    sleep 1
  done
  echo "Personal Beta encrypted-data phase '$phase_name' timed out." >&2
  adb logcat -d -s SereaProcessDeath:V >&2
  return 1
}

reboot_emulator() {
  adb reboot
  adb wait-for-device
  for _ in $(seq 1 120); do
    [[ "$(adb shell getprop sys.boot_completed | tr -d '\r')" == 1 ]] && break
    sleep 1
  done
  [[ "$(adb shell getprop sys.boot_completed | tr -d '\r')" == 1 ]]
  adb root >/dev/null
  adb wait-for-device
  wait_for_package_manager
}

make_key() {
  local path="$1" alias="$2" password_name="$3"
  keytool -genkeypair -noprompt -alias "$alias" -keystore "$path" -storetype PKCS12 \
    -keyalg RSA -keysize 2048 -validity 30 \
    -dname 'CN=Serea disposable Personal Beta identity test' \
    -storepass:env "$password_name" -keypass:env "$password_name" >/dev/null 2>&1
  keytool -list -v -keystore "$path" -storetype PKCS12 -alias "$alias" \
    -storepass:env "$password_name" 2>/dev/null \
    | awk -F ': ' '/SHA256:/{print $2; exit}' | normalize_fingerprint
}

build_beta() {
  local version="$1" harness="$2" output="$3"
  local args=(--no-daemon --no-build-cache --no-configuration-cache --rerun-tasks :app:assembleBeta "-PsereaBetaVersionCode=$version")
  if [[ "$harness" == true ]]; then args+=("-PsereaPersonalBetaValidationHarness=true"); fi
  (cd "$android_dir" && ./gradlew "${args[@]}")
  cp "$android_dir/app/build/outputs/apk/beta/app-beta.apk" "$output"
  bash "$repo_root/tools/verify_personal_beta_apk.sh" "$output" "$version" \
    "$SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256"
}

cd "$android_dir"
./gradlew --no-daemon --no-build-cache --no-configuration-cache :app:assembleDebug
demo_apk="$android_dir/app/build/outputs/apk/debug/app-debug.apk"
demo_apk_id=$("$aapt" dump badging "$demo_apk" | sed -n "s/^package: name='\\([^']*\\)'.*/\\1/p")
[[ "$demo_apk_id" == "$demo_package" ]]
if [[ -z "$(adb shell pm path "$demo_package" | tr -d '\r')" ]]; then
  adb install "$demo_apk"
fi
demo_apk_path_before=$(installed_apk_path)
[[ -n "$demo_apk_path_before" ]]
adb pull "$demo_apk_path_before" "$test_root/demo-before.apk" >/dev/null
demo_fingerprint_before=$(apk_fingerprint "$test_root/demo-before.apk")
demo_version_before=$(demo_package_version)
demo_data_before=$(demo_data_snapshot)

export SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PATH="$test_root/personal-beta-test.p12"
export SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD="$(openssl rand -hex 32)"
export SEREA_ANDROID_PERSONAL_BETA_KEY_ALIAS='serea-personal-beta-test'
export SEREA_ANDROID_PERSONAL_BETA_KEY_PASSWORD="$SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD"
SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256=$(make_key \
  "$SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PATH" "$SEREA_ANDROID_PERSONAL_BETA_KEY_ALIAS" \
  SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD)
export SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256
beta_test_fingerprint="$SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256"
demo_fingerprint="$demo_fingerprint_before"
if [[ "$SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256" == "$demo_fingerprint" ]]; then
  echo 'Disposable Beta signing key unexpectedly matches the Demo signer.' >&2
  exit 1
fi

apk_v99="$test_root/personal-beta-v99.apk"
apk_v100="$test_root/personal-beta-v100.apk"
apk_v101="$test_root/personal-beta-v101.apk"
apk_wrong="$test_root/personal-beta-wrong-signer-v102.apk"
build_beta 99 false "$apk_v99"
adb install "$apk_v99"
[[ -n "$(adb shell pm path "$demo_package" | tr -d '\r')" ]]
[[ -n "$(adb shell pm path "$beta_package" | tr -d '\r')" ]]

build_beta 100 true "$apk_v100"
adb install -r "$apk_v100"
installed_version=$(adb shell dumpsys package "$beta_package" \
  | sed -n 's/.*versionCode=\([0-9][0-9]*\).*/\1/p' | head -1 | tr -d '\r')
[[ "$installed_version" == 100 ]]
phase campaign-seed
phase campaign-consume
phase prepare
database_listing=$(adb shell "ls -l /data/user/0/$beta_package/databases" | tr -d '\r')
printf '%s\n' "$database_listing"
printf '%s\n' "$database_listing" | awk '$5 ~ /^[0-9]+$/ && $5 > 0 && ($NF == "serea-history.db" || $NF == "serea-tasks-process-death-harness.db") { found[$NF]=1 } END { exit !(found["serea-history.db"] && found["serea-tasks-process-death-harness.db"]) }'
adb pull "/data/user/0/$beta_package/databases/serea-history.db" "$test_root/beta-history.db" >/dev/null
if strings "$test_root/beta-history.db" | grep -F -e 'completed answer' -e 'interrupted request' >/dev/null; then
  echo 'Sensitive Beta history fixture was present as plaintext in the Room database.' >&2
  exit 1
fi
adb pull "/data/user/0/$beta_package/databases/serea-tasks-process-death-harness.db" "$test_root/beta-tasks.db" >/dev/null
if strings "$test_root/beta-tasks.db" | grep -F -e 'Process recovery completed' -e 'Persisted encrypted task' >/dev/null; then
  echo 'Sensitive Beta task fixture was present as plaintext in the Room database.' >&2
  exit 1
fi

reboot_emulator
adb shell am start -W -n "$beta_activity" >/dev/null
sleep 2
phase verify
build_beta 101 true "$apk_v101"
adb install -r "$apk_v101"
installed_version=$(adb shell dumpsys package "$beta_package" \
  | sed -n 's/.*versionCode=\([0-9][0-9]*\).*/\1/p' | head -1 | tr -d '\r')
[[ "$installed_version" == 101 ]]
reboot_emulator
adb shell am start -W -n "$beta_activity" >/dev/null
sleep 2
phase verify

wrong_key="$test_root/wrong-signer.p12"
export SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PATH="$wrong_key"
export SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD="$(openssl rand -hex 32)"
export SEREA_ANDROID_PERSONAL_BETA_KEY_ALIAS='serea-personal-beta-wrong-test'
export SEREA_ANDROID_PERSONAL_BETA_KEY_PASSWORD="$SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD"
SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256=$(make_key "$wrong_key" \
  "$SEREA_ANDROID_PERSONAL_BETA_KEY_ALIAS" SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD)
export SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256
build_beta 102 true "$apk_wrong"
if adb install -r "$apk_wrong" > "$test_root/wrong-signer-install.txt" 2>&1; then
  echo 'Android unexpectedly accepted an update signed by a different certificate.' >&2
  exit 1
fi
grep -F 'INSTALL_FAILED_UPDATE_INCOMPATIBLE' "$test_root/wrong-signer-install.txt" >/dev/null
installed_version=$(adb shell dumpsys package "$beta_package" \
  | sed -n 's/.*versionCode=\([0-9][0-9]*\).*/\1/p' | head -1 | tr -d '\r')
[[ "$installed_version" == 101 ]]
phase verify

verify_demo_unchanged
mkdir -p "$(dirname "$results")"
{
  echo 'application_id=app.serea.android.beta'
  echo 'demo_and_beta_parallel_install=PASS'
  echo 'demo_package_data_and_signer_unchanged=PASS'
  echo 'beta_encrypted_room_data_after_emulator_reboot=PASS'
  echo 'same_test_key_version_100_to_101_update=PASS'
  echo 'different_test_key_version_102_update=INSTALL_FAILED_UPDATE_INCOMPATIBLE'
  echo "beta_test_certificate_sha256=$beta_test_fingerprint"
  echo 'owner_signing_key_used=no'
  echo 'demo_uninstall_attempted=no'
} > "$results"
cat "$results"
