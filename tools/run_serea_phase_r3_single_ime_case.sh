#!/usr/bin/env bash
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/tools/serea_real_ime_avd_guards.sh"
OUT="${SEREA_R3_OUT:?Set SEREA_R3_OUT}"
APKS="${SEREA_R3_APKS:?Set SEREA_R3_APKS}"
API="${SEREA_R3_API:?Set SEREA_R3_API}"
METHOD="${1:-${SEREA_R3_METHOD:-}}"
TARGET=app.serea.android.isolated
TEST=app.serea.android.isolated.test
IME=app.serea.android.r25testime
IME_COMPONENT="$IME/app.serea.android.diagnostics.RealImeService"
RUNNER="$TEST/app.serea.android.SereaIsolatedAndroidJUnitRunner"
CLASS=app.serea.android.diagnostics.IndependentImeCompositionTest
EXPECTED_TEST="$CLASS#$METHOD"
ORIGINAL_IME=''
IME_INSTALLED=false
LOGCAT_PID=''
CURRENT_STAGE=preflight

mkdir -p "$OUT/anr" "$OUT/junit"
exec > >(tee -a "$OUT/driver.txt") 2>&1

fail() {
  local stage="$1"
  shift
  printf 'failed_stage=%s\nfailure=%s\n' "$stage" "$*" > "$OUT/failure.txt"
  echo "FAIL [$stage]: $*" >&2
  exit 1
}

# Every ADB call performed by EXIT cleanup/restoration is bounded, including a
# stuck adb client. This helper is not used to weaken or retry test operations.
cleanup_adb() {
  timeout --signal=TERM --kill-after=3s 15s adb "$@"
}

verify_installed_apk() {
  local package="$1" artifact="$2" output="$3" paths actual expected
  paths="$(adb shell pm path "$package" 2>/dev/null | tr -d '\r' | sed -n 's/^package://p')"
  if [[ -z "$paths" || "$paths" == *$'\n'* ]]; then
    printf 'package=%s\ninstalled_paths=%s\nresult=FAIL\n' "$package" "$paths" > "$output"
    return 1
  fi
  actual="$(adb shell sha256sum "$paths" 2>/dev/null | awk 'NR == 1 { print $1 }')"
  expected="$(sha256sum "$APKS/$artifact" | awk '{ print $1 }')"
  printf 'package=%s\ninstalled_path=%s\ninstalled_sha256=%s\nexpected_sha256=%s\n' \
    "$package" "$paths" "$actual" "$expected" > "$output"
  [[ -n "$actual" && "$actual" == "$expected" ]]
}

restore_test_ime() {
  local cleanup_file="$OUT/ime-cleanup.txt" restored enabled package_path status=0
  if [[ "$IME_INSTALLED" != true ]]; then
    restored="$(cleanup_adb shell settings get secure default_input_method 2>/dev/null | tr -d '\r')" || status=1
    enabled="$(cleanup_adb shell settings get secure enabled_input_methods 2>/dev/null | tr -d '\r')" || status=1
    package_path="$(cleanup_adb shell pm path "$IME" 2>/dev/null | tr -d '\r')" || status=1
    {
      printf 'cleanup_action=NOT_REQUIRED\noriginal_default_ime=%s\nrestored_default_ime=%s\n' \
        "$ORIGINAL_IME" "$restored"
      printf 'enabled_ime_state=%s\ntest_ime_package_path=%s\n' "$enabled" "$package_path"
      if [[ -z "$ORIGINAL_IME" || "$ORIGINAL_IME" == null || "$restored" != "$ORIGINAL_IME" || -n "$package_path" ]]; then
        status=1
      fi
      [[ "$enabled" != *"$IME_COMPONENT"* ]] || status=1
      printf 'cleanup_finished_at_utc=%s\ncleanup_status=%s\n' \
        "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" "$([[ "$status" -eq 0 ]] && echo PASS || echo FAIL)"
    } > "$cleanup_file" 2>&1
    if [[ "$status" -eq 0 ]]; then
      echo 'NOT_REQUIRED; original IME unchanged' > "$OUT/test-ime-cleanup-status.txt"
      return 0
    fi
    echo 'FAIL' > "$OUT/test-ime-cleanup-status.txt"
    return 1
  fi
  {
    local set_default_exit disable_test_ime_exit uninstall_test_ime_exit
    printf 'cleanup_started_at_utc=%s\noriginal_default_ime=%s\n' \
      "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" "$ORIGINAL_IME"
    if [[ -z "$ORIGINAL_IME" || "$ORIGINAL_IME" == null ]]; then
      echo 'Cannot restore because the original default IME was not captured.'
      status=1
      set_default_exit=125
    elif cleanup_adb shell ime set "$ORIGINAL_IME"; then
      set_default_exit=0
    else
      set_default_exit=$?
      echo 'Failed to restore original default IME.'
      status=1
    fi
    if cleanup_adb shell ime disable "$IME_COMPONENT"; then
      disable_test_ime_exit=0
    else
      disable_test_ime_exit=$?
      status=1
    fi
    if cleanup_adb uninstall "$IME"; then
      uninstall_test_ime_exit=0
    else
      uninstall_test_ime_exit=$?
      status=1
    fi
    restored="$(cleanup_adb shell settings get secure default_input_method 2>/dev/null | tr -d '\r')" || status=1
    enabled="$(cleanup_adb shell settings get secure enabled_input_methods 2>/dev/null | tr -d '\r')" || status=1
    package_path="$(cleanup_adb shell pm path "$IME" 2>/dev/null | tr -d '\r')" || status=1
    printf 'restored_default_ime=%s\nenabled_ime_state=%s\ntest_ime_package_path=%s\n' \
      "$restored" "$enabled" "$package_path"
    [[ "$restored" == "$ORIGINAL_IME" ]] || status=1
    [[ "$enabled" != *"$IME_COMPONENT"* ]] || status=1
    [[ -z "$package_path" ]] || status=1
    printf 'restore_default_exit=%s\ndisable_test_ime_exit=%s\nuninstall_test_ime_exit=%s\n' \
      "$set_default_exit" "$disable_test_ime_exit" "$uninstall_test_ime_exit"
    printf 'cleanup_finished_at_utc=%s\ncleanup_status=%s\n' \
      "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" "$([[ "$status" -eq 0 ]] && echo PASS || echo FAIL)"
  } > "$cleanup_file" 2>&1
  if [[ "$status" -eq 0 ]]; then
    echo 'PASS' > "$OUT/test-ime-cleanup-status.txt"
    IME_INSTALLED=false
    return 0
  fi
  echo 'FAIL' > "$OUT/test-ime-cleanup-status.txt"
  return 1
}

cleanup() {
  local main_status="$1" final_status="$1" junit_status=NOT_RUN restore_status=0
  trap - EXIT INT TERM
  set +e
  if [[ -n "$LOGCAT_PID" ]]; then
    if ! serea_stop_logcat_bounded "$LOGCAT_PID" 2 2 > "$OUT/logcat-stop.txt" 2>&1; then
      final_status=1
    fi
    LOGCAT_PID=''
  fi
  if command -v adb >/dev/null 2>&1 && cleanup_adb get-state >/dev/null 2>&1; then
    cleanup_adb logcat -d -b all -v threadtime > "$OUT/logcat.txt" 2>&1
    cleanup_adb shell dumpsys activity activities > "$OUT/activity-after.txt" 2>&1
    cleanup_adb shell dumpsys input_method > "$OUT/input-method-before-cleanup.txt" 2>&1
    cleanup_adb shell dumpsys meminfo > "$OUT/meminfo-after.txt" 2>&1
    cleanup_adb shell dumpsys cpuinfo > "$OUT/cpuinfo-after.txt" 2>&1
    cleanup_adb shell cat /proc/pressure/cpu > "$OUT/guest-cpu-pressure-after.txt" 2>&1
    cleanup_adb shell cat /proc/pressure/memory > "$OUT/guest-memory-pressure-after.txt" 2>&1
    cleanup_adb shell dumpsys activity lastanr > "$OUT/lastanr.txt" 2>&1
    cleanup_adb shell settings get secure default_input_method > "$OUT/default-ime-before-cleanup.txt" 2>&1
    cleanup_adb shell ps -A > "$OUT/processes-after.txt" 2>&1
    cleanup_adb root > "$OUT/adb-root.txt" 2>&1
    cleanup_adb wait-for-device >/dev/null 2>&1
    cleanup_adb pull /data/anr "$OUT/anr" > "$OUT/anr-pull.txt" 2>&1
  fi
  if [[ -s "$OUT/instrumentation.txt" ]]; then
    if python3 "$ROOT/tools/instrumentation_to_junit.py" "$OUT/instrumentation.txt" \
      "$OUT/junit/real-ime-${METHOD}.xml" "$CLASS"; then
      if python3 "$ROOT/tools/verify_android_instrumentation.py" "$OUT/junit" \
        --minimum 1 --exact-count 1 --require-test "$EXPECTED_TEST" \
        > "$OUT/junit-verification.txt" 2>&1; then
        junit_status=PASS
      else
        junit_status=FAIL
      fi
    else
      junit_status=FAIL
    fi
  fi
  printf 'junit_status=%s\n' "$junit_status" > "$OUT/junit-status.txt"
  [[ "$junit_status" == PASS ]] || final_status=1
  if restore_test_ime; then
    restore_status=0
  else
    restore_status=1
  fi
  [[ "$restore_status" -eq 0 ]] || final_status=1
  if [[ -n "$ORIGINAL_IME" ]]; then
    observed_default_ime='unavailable'
    if command -v adb >/dev/null 2>&1 && cleanup_adb get-state >/dev/null 2>&1; then
      if observed_default_ime="$(cleanup_adb shell settings get secure default_input_method 2>/dev/null | tr -d '\r')"; then
        :
      else
        observed_default_ime='unavailable'
        final_status=1
      fi
      cleanup_adb shell ime list -s > "$OUT/available-ime-services-after-cleanup.txt" 2>&1 || final_status=1
    fi
    printf 'expected_default_ime=%s\nobserved_default_ime=%s\n' \
      "$ORIGINAL_IME" "$observed_default_ime" > "$OUT/default-ime-restoration-verification.txt"
    if [[ -s "$OUT/ime-cleanup.txt" ]] && grep -Fq "restored_default_ime=$ORIGINAL_IME" "$OUT/ime-cleanup.txt" && \
      grep -Fq 'cleanup_status=PASS' "$OUT/ime-cleanup.txt" && \
      [[ "$observed_default_ime" == "$ORIGINAL_IME" ]]; then
      echo 'verified=true' >> "$OUT/default-ime-restoration-verification.txt"
    else
      echo 'verified=false' >> "$OUT/default-ime-restoration-verification.txt"
      final_status=1
    fi
  else
    echo 'verified=false; original default IME was not captured' > "$OUT/default-ime-restoration-verification.txt"
    final_status=1
  fi
  {
    printf 'api=%s\nmethod=%s\nsource_commit=%s\nsource_tree=%s\n' \
      "$API" "$METHOD" "$(git -C "$ROOT" rev-parse HEAD 2>/dev/null || echo unavailable)" \
      "$(git -C "$ROOT" rev-parse 'HEAD^{tree}' 2>/dev/null || echo unavailable)"
    printf 'stage_at_exit=%s\nmain_exit_status=%s\nfinal_status=%s\nfinished_at_utc=%s\n' \
      "$CURRENT_STAGE" "$main_status" "$final_status" "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)"
  } > "$OUT/result.txt"
  if [[ "$final_status" -ne 0 && ! -s "$OUT/failure.txt" ]]; then
    printf 'failed_stage=%s\nfailure=Test, JUnit validation, or cleanup did not pass.\n' \
      "$CURRENT_STAGE" > "$OUT/failure.txt"
  fi
  exit "$final_status"
}

on_exit() {
  local status=$?
  cleanup "$status"
}
trap on_exit EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

if [[ ( "$API" != 26 && "$API" != 35 ) || ( "$METHOD" != B_realImeCompositionWithoutSynchronizationBoundaries && \
      "$METHOD" != C_realImeCompositionWithSynchronizationBoundaries ) ]]; then
  fail preflight 'Only API 26 or 35 Test B or Test C is permitted.'
fi
if [[ ! "${GITHUB_RUN_ID:-}" =~ ^[0-9]+$ || "${GITHUB_ACTIONS:-}" != true || "${CI:-}" != true ]]; then
  fail preflight 'This helper is restricted to a GitHub Actions disposable Emulator run.'
fi
if [[ "$(git -C "$ROOT" rev-parse HEAD)" != "${GITHUB_SHA:-}" ]]; then
  fail source 'Checked-out source SHA does not match GITHUB_SHA.'
fi
if [[ ! -s "$APKS/provenance.txt" || ! -s "$APKS/apk-sha256.txt" ]]; then
  fail apk-provenance 'Built APK provenance or digest manifest is missing.'
fi
if [[ "$(sed -n 's/^source_commit=//p' "$APKS/provenance.txt")" != "$GITHUB_SHA" || \
      "$(sed -n 's/^source_tree=//p' "$APKS/provenance.txt")" != "$(git -C "$ROOT" rev-parse 'HEAD^{tree}')" ]]; then
  fail apk-provenance 'APK build provenance does not match the checked-out source.'
fi
if [[ "$(sed -n 's/^apk_manifest_sha256=//p' "$APKS/provenance.txt")" != \
      "$(sha256sum "$APKS/apk-sha256.txt" | awk '{print $1}')" ]] || \
   ! (cd "$APKS" && sha256sum --check apk-sha256.txt); then
  fail apk-provenance 'APK manifest digest or one of the three APK SHA-256 values did not match.'
fi

CURRENT_STAGE=emulator-guard
export EXPECTED_API_LEVEL="$API" SEREA_EXPECTED_ANDROID_API="$API"
source "$ROOT/tools/assert_disposable_android_emulator.sh" || fail emulator-guard 'Disposable Emulator guard rejected the device.'
if [[ "$(adb shell getprop sys.boot_completed | tr -d '\r')" != 1 ]]; then
  fail emulator-guard 'Android boot_completed is not 1.'
fi
hardware="$(adb shell getprop ro.hardware | tr -d '\r')"
fingerprint="$(adb shell getprop ro.build.fingerprint | tr -d '\r')"
abi="$(adb shell getprop ro.product.cpu.abi | tr -d '\r')"
active_avd_reply="$(adb emu avd name | tr -d '\r')"
active_avd="$(printf '%s\n' "$active_avd_reply" | awk 'NF && $0 != "OK" && $0 !~ /^KO:/ { print; exit }')"
if [[ ! "$active_avd" =~ ^[A-Za-z0-9_.-]+$ ]]; then
  fail emulator-guard "Could not identify the active AVD from adb emu avd name: $active_avd_reply"
fi
avd_home="${ANDROID_AVD_HOME:-${ANDROID_USER_HOME:-$HOME/.android}/avd}"
avd_config="$avd_home/$active_avd.avd/config.ini"
if ! avd_image_metadata="$(serea_verify_active_google_apis_avd "$API" "$fingerprint" "$avd_config")"; then
  fail emulator-guard "Active AVD image or device fingerprint did not match the requested API $API Google APIs x86_64 image."
fi
if [[ ( "$hardware" != ranchu && "$hardware" != goldfish ) || "$abi" != x86_64 ]]; then
  fail emulator-guard "Unexpected API $API emulator identity: hardware=$hardware abi=$abi fingerprint=$fingerprint"
fi
{
  printf 'api=%s\nhardware=%s\nabi=%s\nfingerprint=%s\nqemu=%s\nboot_completed=1\n' \
    "$API" "$hardware" "$abi" "$fingerprint" "$SEREA_OBSERVED_ANDROID_QEMU"
  printf 'active_avd=%s\nactive_avd_config=%s\n' "$active_avd" "$avd_config"
  printf '%s\n' "$avd_image_metadata"
  printf 'serial=%s\nfont_scale=%s\n' "$(adb get-serialno | tr -d '\r')" \
    "$(adb shell settings get system font_scale | tr -d '\r')"
  adb shell wm size
  adb shell wm density
} > "$OUT/emulator-metadata.txt" 2>&1
emulator -list-avds > "$OUT/avds.txt" 2>&1 || true
find "$HOME/.android/avd" -maxdepth 2 -type f -name config.ini -print -exec cat {} \; > "$OUT/avd-config.txt" 2>&1 || true
adb shell cat /proc/pressure/cpu > "$OUT/guest-cpu-pressure-before.txt" 2>&1 || true
adb shell cat /proc/pressure/memory > "$OUT/guest-memory-pressure-before.txt" 2>&1 || true
adb shell dumpsys meminfo > "$OUT/meminfo-before.txt" 2>&1 || true
adb shell dumpsys cpuinfo > "$OUT/cpuinfo-before.txt" 2>&1 || true
adb shell dumpsys activity activities > "$OUT/activity-before.txt" 2>&1 || true
adb shell dumpsys input_method > "$OUT/input-method-before.txt" 2>&1 || true
adb shell ps -A > "$OUT/processes-before.txt" 2>&1 || true

CURRENT_STAGE=default-ime-readiness
source "$ROOT/tools/wait_for_android_default_ime.sh"
if ! wait_for_android_default_ime 30; then
  ORIGINAL_IME="$SEREA_ANDROID_DEFAULT_IME"
  printf 'default_ime_status=NOT_READY\nobserved=%s\nattempts=%s\n' \
    "$ORIGINAL_IME" "$SEREA_ANDROID_DEFAULT_IME_ATTEMPTS" > "$OUT/default-ime-readiness.txt"
  adb shell ime list -s >> "$OUT/default-ime-readiness.txt" 2>&1 || true
  adb shell dumpsys input_method >> "$OUT/default-ime-readiness.txt" 2>&1 || true
  fail default-ime-readiness 'Android did not expose a configured default IME after 30 checks.'
fi
ORIGINAL_IME="$SEREA_ANDROID_DEFAULT_IME"
printf 'default_ime=%s\nreadiness_attempts=%s\n' \
  "$ORIGINAL_IME" "$SEREA_ANDROID_DEFAULT_IME_ATTEMPTS" > "$OUT/default-ime-readiness.txt"
printf 'original_default_ime=%s\n' "$ORIGINAL_IME" > "$OUT/ime-restoration-verification.txt"

CURRENT_STAGE=fresh-package-guard
for package in "$TARGET" "$TEST" "$IME"; do
  package_path="$(adb shell pm path "$package" 2>/dev/null | tr -d '\r')"
  if [[ -n "$package_path" ]]; then
    printf 'package=%s\npackage_path=%s\n' "$package" "$package_path" >> "$OUT/fresh-package-guard-failed.txt"
    fail fresh-package-guard "Refusing preinstalled target, instrumentation, or Test IME package: $package"
  fi
done
echo 'fresh_avd_packages_absent=PASS' > "$OUT/fresh-package-guard.txt"

CURRENT_STAGE=apk-install
if ! adb install "$APKS/r25-ime-test-debug.apk" > "$OUT/install-ime.txt" 2>&1; then
  fail apk-install 'Could not install the separately packaged Test IME APK.'
fi
IME_INSTALLED=true
if ! adb install "$APKS/app-isolated.apk" > "$OUT/install-target.txt" 2>&1; then
  fail apk-install 'Could not install the isolated target APK.'
fi
if ! adb install "$APKS/app-isolated-androidTest.apk" > "$OUT/install-test.txt" 2>&1; then
  fail apk-install 'Could not install the isolated instrumentation APK.'
fi
if ! verify_installed_apk "$TARGET" app-isolated.apk "$OUT/installed-target-apk.txt"; then
  fail installed-apk-sha 'Installed target APK SHA-256 does not match the build artifact.'
fi
if ! verify_installed_apk "$TEST" app-isolated-androidTest.apk "$OUT/installed-test-apk.txt"; then
  fail installed-apk-sha 'Installed instrumentation APK SHA-256 does not match the build artifact.'
fi
if ! verify_installed_apk "$IME" r25-ime-test-debug.apk "$OUT/installed-ime-apk.txt"; then
  fail installed-apk-sha 'Installed Test IME APK SHA-256 does not match the build artifact.'
fi
adb shell pm grant "$TEST" app.serea.android.r25testime.permission.DIAGNOSTICS > "$OUT/permission-grant.txt" 2>&1 || true

CURRENT_STAGE=ime-selection
if ! adb shell ime enable "$IME_COMPONENT" > "$OUT/ime-enable.txt" 2>&1; then
  fail ime-selection 'Android could not enable the Test IME.'
fi
if ! adb shell ime set "$IME_COMPONENT" > "$OUT/ime-select.txt" 2>&1; then
  fail ime-selection 'Android could not select the Test IME.'
fi
selected_ime="$(adb shell settings get secure default_input_method 2>/dev/null | tr -d '\r')"
enabled_imes="$(adb shell settings get secure enabled_input_methods 2>/dev/null | tr -d '\r')"
{
  printf 'expected_selected_ime=%s\nselected_ime=%s\nenabled_input_methods=%s\n' \
    "$IME_COMPONENT" "$selected_ime" "$enabled_imes"
  adb shell ime list -s
} > "$OUT/ime-state-selected.txt" 2>&1
if [[ "$selected_ime" != "$IME_COMPONENT" || "$enabled_imes" != *"$IME_COMPONENT"* ]]; then
  fail ime-selection 'Android settings do not confirm the Test IME is enabled and selected.'
fi

CURRENT_STAGE=fresh-data-seed
if ! adb shell pm clear "$TARGET" > "$OUT/target-pm-clear.txt" 2>&1 || \
   ! grep -Fq Success "$OUT/target-pm-clear.txt"; then
  fail fresh-data-seed 'Could not clear the newly installed isolated target before seeding.'
fi
if ! TOKEN="$(serea_make_campaign_token "$API" "${GITHUB_RUN_ID:-}" \
    "${GITHUB_RUN_ATTEMPT:-1}" "$METHOD")"; then
  fail fresh-data-seed 'Generated campaign token does not satisfy the isolated runner token contract.'
fi
printf 'campaign_token=%s\nmethod=%s\nseed_started_at_utc=%s\n' \
  "$TOKEN" "$METHOD" "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" > "$OUT/campaign-marker.txt"
if ! SEREA_DISPOSABLE_CI_EMULATOR=1 SEREA_EXPECTED_ANDROID_API="$API" EXPECTED_API_LEVEL="$API" \
    bash "$ROOT/tools/prepare_isolated_instrumentation_target.sh" "$TOKEN" \
    > "$OUT/campaign-seed.log" 2>&1; then
  fail fresh-data-seed 'The existing run-as based isolated namespace seed guard failed.'
fi
if ! serea_verify_isolated_seed_log "$OUT/campaign-seed.log"; then
  fail fresh-data-seed 'Campaign seed helper did not report its explicit PASS marker.'
fi

CURRENT_STAGE=instrumentation-startup
printf 'method=%s\nclass=%s\nrunner=%s\nstart_utc=%s\n' \
  "$METHOD" "$CLASS" "$RUNNER" "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" > "$OUT/method-metadata.txt"
adb logcat -d -v threadtime > "$OUT/logcat-startup.txt" 2>&1 || true
adb logcat -c >/dev/null 2>&1 || true
adb logcat -v threadtime > "$OUT/logcat-live.txt" 2>&1 &
LOGCAT_PID=$!
CURRENT_STAGE=instrumentation-test
set +e
timeout --signal=TERM 240s adb shell am instrument -w -r \
  -e class "$EXPECTED_TEST" -e sereaCampaignToken "$TOKEN" "$RUNNER" \
  > "$OUT/instrumentation.txt" 2>&1
instrumentation_exit=$?
set -e
printf 'am_instrument_exit=%s\nend_utc=%s\n' \
  "$instrumentation_exit" "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" >> "$OUT/method-metadata.txt"
if [[ "$instrumentation_exit" -ne 0 ]] || \
   ! grep -Fq 'INSTRUMENTATION_STATUS: numtests=1' "$OUT/instrumentation.txt" || \
   ! grep -Fq "INSTRUMENTATION_STATUS: test=$METHOD" "$OUT/instrumentation.txt" || \
   ! grep -Fq 'INSTRUMENTATION_CODE: -1' "$OUT/instrumentation.txt" || \
   grep -Fq 'FAILURES!!!' "$OUT/instrumentation.txt"; then
  fail instrumentation-test 'Instrumentation did not report exactly one successful requested method.'
fi

CURRENT_STAGE=campaign-marker-claim
claimed_token="$(adb shell run-as "$TARGET" cat no_backup/serea-instrumentation-claimed 2>/dev/null | tr -d '\r')"
printf 'expected_claimed_token=%s\nobserved_claimed_token=%s\n' \
  "$TOKEN" "$claimed_token" > "$OUT/campaign-marker-claim.txt"
if [[ "$claimed_token" != "$TOKEN" ]]; then
  fail campaign-marker-claim 'The isolated instrumentation runner did not claim this exact one-use campaign token.'
fi
CURRENT_STAGE=complete
echo 'method_result=PASS' > "$OUT/method-result.txt"
exit 0
