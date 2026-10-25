#!/usr/bin/env bash
set -Eeuo pipefail

SEREA_BETA_PHASE='script initialization'
SEREA_BETA_DIAGNOSTICS_PRINTED=0

on_error() {
  local status=$?
  local line="${1:-unknown}"
  local failed_command="${2:-unknown}"
  printf 'Personal Beta harness failed: phase=%q line=%s status=%s command=%q\n' \
    "$SEREA_BETA_PHASE" "$line" "$status" "$failed_command" >&2
  if [[ "$SEREA_BETA_DIAGNOSTICS_PRINTED" != 1 ]] && declare -F print_emulator_diagnostics >/dev/null; then
    print_emulator_diagnostics 'ADB state at the failing harness command:'
  fi
  exit "$status"
}
trap 'on_error "$LINENO" "$BASH_COMMAND"' ERR

set_phase() {
  SEREA_BETA_PHASE="$1"
  SEREA_BETA_DIAGNOSTICS_PRINTED=0
  printf 'Personal Beta harness phase: %s\n' "$SEREA_BETA_PHASE"
}

set_phase 'validate disposable-emulator opt-in'

if [[ "${SEREA_PERSONAL_BETA_DISPOSABLE_EMULATOR:-}" != "1" ]]; then
  echo 'Set SEREA_PERSONAL_BETA_DISPOSABLE_EMULATOR=1 only for a throwaway API 35 emulator.' >&2
  exit 2
fi
command -v adb >/dev/null
command -v keytool >/dev/null
command -v openssl >/dev/null
command -v strings >/dev/null
command -v timeout >/dev/null

serial=''
device_listing=''
set_phase 'discover one emulator serial'
discovery_deadline=$((SECONDS + 60))
for _ in $(seq 1 30); do
  remaining=$((discovery_deadline - SECONDS))
  (( remaining > 0 )) || break
  timeout_seconds=5
  (( timeout_seconds < remaining )) || timeout_seconds=$remaining
  device_listing=$(timeout "${timeout_seconds}s" adb devices -l 2>&1 || true)
  device_rows=$(awk 'NR > 1 && NF >= 2 { print $1 ":" $2 }' <<< "$device_listing")
  row_count=$(awk 'NF { count++ } END { print count+0 }' <<< "$device_rows")
  if [[ "$row_count" == 1 ]]; then
    candidate="${device_rows%%:*}"
    if [[ "$candidate" =~ ^emulator-[0-9]+$ ]]; then
      serial="$candidate"
      break
    fi
  elif [[ "$row_count" -gt 1 ]]; then
    echo 'Expected exactly one disposable emulator and no other Android targets.' >&2
    printf '%s\n' "$device_listing" >&2
    exit 2
  fi
  remaining=$((discovery_deadline - SECONDS))
  (( remaining > 0 )) || break
  sleep_seconds=2
  (( sleep_seconds < remaining )) || sleep_seconds=$remaining
  sleep "$sleep_seconds"
done
if [[ -z "$serial" ]]; then
  echo 'Could not discover exactly one disposable Android emulator within 60 seconds.' >&2
  printf '%s\n' "$device_listing" >&2
  timeout 5s adb devices -l >&2 || true
  exit 2
fi
export ANDROID_SERIAL="$serial"

bounded_adb() {
  local deadline="$1" cap="$2"
  shift 2
  local remaining=$((deadline - SECONDS))
  (( remaining > 0 )) || return 124
  (( cap < remaining )) || cap=$remaining
  timeout "${cap}s" adb "$@"
}

print_emulator_diagnostics() {
  local reason="$1"
  SEREA_BETA_DIAGNOSTICS_PRINTED=1
  echo "$reason" >&2
  timeout 3s adb devices -l >&2 || true
  if [[ -n "$serial" ]]; then
    echo "ADB target: $serial" >&2
    timeout 3s adb -s "$serial" get-state >&2 || true
    timeout 3s adb -s "$serial" shell getprop ro.build.version.sdk >&2 || true
    timeout 3s adb -s "$serial" shell getprop ro.kernel.qemu >&2 || true
    timeout 3s adb -s "$serial" shell getprop sys.boot_completed >&2 || true
  fi
}

wait_for_stable_emulator() {
  local phase="$1"
  local deadline=$((SECONDS + 120))
  local stable_samples=0
  local state sdk qemu boot_completed remaining sleep_seconds
  while (( SECONDS < deadline )); do
    if bounded_adb "$deadline" 8 -s "$serial" wait-for-device >/dev/null 2>&1; then
      state=$(bounded_adb "$deadline" 5 -s "$serial" get-state 2>/dev/null || true)
      sdk=$(bounded_adb "$deadline" 5 -s "$serial" shell getprop ro.build.version.sdk 2>/dev/null | tr -d '\r' || true)
      qemu=$(bounded_adb "$deadline" 5 -s "$serial" shell getprop ro.kernel.qemu 2>/dev/null | tr -d '\r' || true)
      boot_completed=$(bounded_adb "$deadline" 5 -s "$serial" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r' || true)
      if [[ "$state" == device && "$sdk" == 35 && "$qemu" == 1 && "$boot_completed" == 1 ]]; then
        stable_samples=$((stable_samples + 1))
        if [[ "$stable_samples" -ge 3 ]]; then return 0; fi
      else
        stable_samples=0
      fi
    else
      stable_samples=0
    fi
    remaining=$((deadline - SECONDS))
    (( remaining > 0 )) || break
    sleep_seconds=2
    (( sleep_seconds < remaining )) || sleep_seconds=$remaining
    sleep "$sleep_seconds"
  done
  print_emulator_diagnostics "Emulator did not remain online with API 35, QEMU, and boot complete during $phase."
  return 1
}

set_phase 'wait for stable API 35 emulator startup'
wait_for_stable_emulator 'initial startup'
set_phase 'restart adb as root and wait for stable transport'
if ! timeout 10s adb -s "$serial" root >/dev/null; then
  print_emulator_diagnostics 'Could not restart adb in root mode on the disposable emulator.'
  exit 2
fi
wait_for_stable_emulator 'adb root restart'
root_uid=$(timeout 8s adb -s "$serial" shell id -u 2>/dev/null | tr -d '\r' || true)
if [[ "$root_uid" != 0 ]]; then
  print_emulator_diagnostics 'The disposable emulator must allow adb root to preserve-check Demo app data.'
  exit 2
fi

wait_for_package_manager() {
  local package_listing='' status=0
  for attempt in $(seq 1 60); do
    if package_listing=$(adb -s "$serial" shell pm list packages -3 2>&1 | tr -d '\r'); then
      status=0
      if [[ "$package_listing" != *"Can't find service: package"* &&
            "$package_listing" != *'adb: device offline'* &&
            "$package_listing" != *'device not found'* ]]; then
        return 0
      fi
    else
      status=$?
      if [[ -n "$package_listing" ]] &&
          ! grep -Eiq 'device offline|device .*not found|no devices/emulators found|connection.*closed|Can.t find service: package' <<< "$package_listing"; then
        printf 'Android Package Manager probe failed (exit=%s): %s\n' "$status" "$package_listing" >&2
        return "$status"
      fi
    fi
    if grep -Eiq 'device offline|device .*not found|no devices/emulators found|connection.*closed' <<< "$package_listing" ||
        { [[ "$status" -ne 0 && -z "$package_listing" ]]; }; then
      printf 'RETRY: waiting for ADB/Package Manager readiness (%s/60).\n' "$attempt" >&2
      adb -s "$serial" reconnect offline >/dev/null 2>&1 || true
    fi
    status=0
    sleep 2
  done
  printf 'Last Package Manager probe: %s\n' "$package_listing" >&2
  echo 'Android Package Manager did not become available on the disposable emulator.' >&2
  return 1
}
set_phase 'wait for Android Package Manager'
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

set_phase 'refuse preinstalled Beta package on the disposable emulator'
beta_path_status=0
if beta_path_response=$(adb -s "$serial" shell pm path "$beta_package" 2>&1); then
  beta_path_status=0
else
  beta_path_status=$?
fi
beta_path_response=$(printf '%s' "$beta_path_response" | tr -d '\r')
beta_existing_path=''
case "$beta_path_response" in
  '')
    if [[ "$beta_path_status" -ne 0 ]]; then
      beta_device_state_status=0
      if beta_device_state=$(timeout 8s adb -s "$serial" get-state 2>&1); then
        beta_device_state_status=0
      else
        beta_device_state_status=$?
      fi
      beta_device_state=$(printf '%s' "$beta_device_state" | tr -d '\r')
      if [[ "$beta_device_state_status" -ne 0 || "$beta_device_state" != device ]]; then
        printf 'ADB device-state probe failed (exit=%s, state=%s) after the empty Beta path response.\n' \
          "$beta_device_state_status" "$beta_device_state" >&2
        print_emulator_diagnostics 'ADB state after the empty, failed Beta package path query:'
        if [[ "$beta_device_state_status" -ne 0 ]]; then exit "$beta_device_state_status"; fi
        exit 1
      fi

      beta_package_list_status=0
      if beta_package_list=$(timeout 10s adb -s "$serial" shell pm list packages "$beta_package" 2>&1); then
        beta_package_list_status=0
      else
        beta_package_list_status=$?
      fi
      beta_package_list=$(printf '%s' "$beta_package_list" | tr -d '\r')
      if [[ "$beta_package_list_status" -ne 0 ]]; then
        printf 'Beta package-list verification failed (exit=%s): %s\n' \
          "$beta_package_list_status" "$beta_package_list" >&2
        print_emulator_diagnostics 'ADB state after the Beta package-list verification failed:'
        exit "$beta_package_list_status"
      fi

      beta_exact_package_found=0
      beta_unexpected_package_output=''
      while IFS= read -r package_line; do
        [[ -z "$package_line" ]] && continue
        if [[ "$package_line" != package:* ]]; then
          beta_unexpected_package_output="$package_line"
          break
        fi
        listed_package="${package_line#package:}"
        if [[ ! "$listed_package" =~ ^[A-Za-z0-9_.]+$ ]]; then
          beta_unexpected_package_output="$package_line"
          break
        fi
        if [[ "$listed_package" == "$beta_package" ]]; then
          beta_exact_package_found=1
        fi
      done <<< "$beta_package_list"

      if [[ -n "$beta_unexpected_package_output" ]]; then
        printf 'Unexpected Beta package-list output: %s\n' "$beta_unexpected_package_output" >&2
        print_emulator_diagnostics 'ADB state after unexpected Beta package-list output:'
        exit 1
      fi
      if [[ "$beta_exact_package_found" -eq 1 ]]; then
        beta_existing_path='present'
      fi
    fi
    ;;
  'Error: package not found'|"Error: package not found: $beta_package")
    # Package Manager reports an absent package with either an empty successful
    # response or its exact not-found diagnostic. Other adb failures remain fatal.
    ;;
  package:*)
    beta_existing_path="$beta_path_response"
    ;;
  *)
    printf 'Beta package query failed (exit=%s): %s\n' "$beta_path_status" "$beta_path_response" >&2
    print_emulator_diagnostics 'ADB state after an unexpected Beta package query response:'
    if [[ "$beta_path_status" -ne 0 ]]; then exit "$beta_path_status"; fi
    exit 1
    ;;
esac
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
set_phase 'build and capture existing Demo identity/data'
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
set_phase 'generate temporary Beta signing identity'
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
set_phase 'build and install Beta v99 with temporary signer'
build_beta 99 false "$apk_v99"
adb install "$apk_v99"
[[ -n "$(adb shell pm path "$demo_package" | tr -d '\r')" ]]
[[ -n "$(adb shell pm path "$beta_package" | tr -d '\r')" ]]

set_phase 'build and install Beta v100 and seed encrypted state'
build_beta 100 true "$apk_v100"
adb install -r "$apk_v100"
installed_version=$(adb shell dumpsys package "$beta_package" \
  | sed -n 's/.*versionCode=\([0-9][0-9]*\).*/\1/p' | head -1 | tr -d '\r')
[[ "$installed_version" == 100 ]]
phase campaign-seed-and-claim
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

set_phase 'reboot and verify encrypted Beta state'
reboot_emulator
adb shell am start -W -n "$beta_activity" >/dev/null
sleep 2
phase verify
set_phase 'build and install same-signer Beta v101 update'
build_beta 101 true "$apk_v101"
adb install -r "$apk_v101"
installed_version=$(adb shell dumpsys package "$beta_package" \
  | sed -n 's/.*versionCode=\([0-9][0-9]*\).*/\1/p' | head -1 | tr -d '\r')
[[ "$installed_version" == 101 ]]
set_phase 'reboot and reverify encrypted Beta state'
reboot_emulator
adb shell am start -W -n "$beta_activity" >/dev/null
sleep 2
phase verify

wrong_key="$test_root/wrong-signer.p12"
set_phase 'build and reject different-signer Beta v102 update'
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

set_phase 'verify Demo identity/data isolation and write evidence'
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
