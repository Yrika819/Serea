#!/usr/bin/env bash

# android-emulator-runner invokes each line of its `script` input in a fresh
# shell. Keep this stateful campaign in one Bash process.
set +e

api_level="${1:-}"
if [[ "$api_level" != 26 && "$api_level" != 35 ]]; then
  echo "Usage: $0 {26|35}" >&2
  exit 2
fi
tool_dir="$(cd "$(dirname "$0")" && pwd)"

# API 26 and API 35 run sequentially in one Actions job. Share a single wall-clock
# deadline so the first API cannot consume time needed to save the second API's
# evidence. The workflow sets this once near job start; local runs receive the
# same bounded 170-minute default.
if [[ "${SEREA_ANDROID_CAMPAIGN_DEADLINE_EPOCH:-}" =~ ^[0-9]+$ ]]; then
  campaign_deadline_epoch="$SEREA_ANDROID_CAMPAIGN_DEADLINE_EPOCH"
else
  campaign_deadline_epoch=$(( $(date +%s) + 170 * 60 ))
fi
bounded_command_seconds() {
  local cap="$1" reserve_seconds="${2:-150}" now remaining available
  now="$(date +%s)" || return 1
  remaining=$((campaign_deadline_epoch - now))
  available=$((remaining - reserve_seconds))
  if (( available <= 0 )); then return 1; fi
  if (( available < cap )); then cap="$available"; fi
  printf '%s\n' "$cap"
}

run_bounded_gradle() {
  local cap_seconds="$1" kill_after_seconds="$2" log_file="$3"
  shift 3
  local timeout_seconds
  timeout_seconds="$(bounded_command_seconds "$cap_seconds" "$((kill_after_seconds + 30))")" || {
    echo 'NOT_RUN: shared Android campaign deadline leaves no safe Gradle command budget.' > "$log_file"
    return 124
  }
  timeout --signal=INT --kill-after="${kill_after_seconds}s" "${timeout_seconds}s" ./gradlew --no-daemon "$@" > "$log_file" 2>&1
}

root='app/build/outputs/androidTest-results'
evidence="$root/api$api_level"
mkdir -p "$evidence" || exit 1
variant_filters="$evidence/variant-filters.json"
offline_filter=''
http_filter=''
{
  echo "source_checkout_sha=$(git rev-parse HEAD 2>/dev/null || echo unknown)"
  if [[ -n "${GITHUB_EVENT_PATH:-}" && -f "$GITHUB_EVENT_PATH" ]]; then
    python3 - "$GITHUB_EVENT_PATH" <<'PY'
import json, sys
try:
    event = json.load(open(sys.argv[1], encoding="utf-8"))
    print("candidate_source_sha=" + str(event.get("pull_request", {}).get("head", {}).get("sha", "unknown")))
except Exception as error:
    print("candidate_source_sha=unavailable:" + str(error))
PY
  else
    echo "candidate_source_sha=${GITHUB_SHA:-unknown}"
  fi
  echo "campaign_run_id=${GITHUB_RUN_ID:-local}"
  echo "campaign_attempt=${GITHUB_RUN_ATTEMPT:-1}"
  echo "api_level=$api_level"
  for apk in app/build/outputs/apk/isolated/app-isolated.apk app/build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk; do
    if [[ -f "$apk" ]]; then sha256sum "$apk"; else echo "apk_missing=$apk"; fi
  done
} > "$evidence/build-provenance.txt"
printf 'NOT_RUN\n' > "$evidence/instrumentation-status.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/offline-status.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/http-status.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/process-death-status.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/device-api-level.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/api-result.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/data-isolation-status.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/data-isolation.log" || exit 1
printf 'NOT_RUN\n' > "$evidence/instrumentation-timeout.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/test-lifecycle-host.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/test-device-state.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/test-device-state-status.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/instrumentation-gradle-exit-code.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/instrumentation-verification-exit-code.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/diagnostic-status.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/instrumentation-gradle.log" || exit 1
printf 'NOT_RUN\n' > "$evidence/process-death-host-lifecycle.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/logcat-live.txt" || exit 1
printf 'NOT_READY\n' > "$evidence/process-death-readiness-status.txt" || exit 1
printf 'NOT_READY\n' > "$evidence/process-death-readiness.log" || exit 1

logcat_pid=''
initial_default_ime=''
test_ime_installed=false
test_ime_cleanup_result=NOT_RUN
stop_live_capture() {
  if [[ -n "$logcat_pid" ]]; then
    kill "$logcat_pid" 2>/dev/null || true
    wait "$logcat_pid" 2>/dev/null || true
    logcat_pid=''
  fi
  if [[ -s "$evidence/logcat-live.txt" ]]; then
    python3 "$tool_dir/summarize_android_test_lifecycle.py" \
      "$evidence/logcat-live.txt" "$evidence/test-lifecycle-summary.txt" "$evidence" || true
  fi
}
cleanup_test_ime() {
  if [[ "$test_ime_installed" == true ]]; then
    local cleanup_log="$evidence/test-ime-cleanup.txt"
    {
      printf 'started_at_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)"
      printf 'initial_default_ime=%s\n' "$initial_default_ime"
      adb shell ime set "$initial_default_ime"
      adb shell ime disable app.serea.android.r25testime/app.serea.android.diagnostics.RealImeService
      adb uninstall app.serea.android.r25testime
      printf 'restored_default_ime=%s\n' "$(adb shell settings get secure default_input_method | tr -d '\r')"
      printf 'test_ime_package_path=%s\n' "$(adb shell pm path app.serea.android.r25testime 2>/dev/null | tr -d '\r')"
      printf 'finished_at_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)"
    } > "$cleanup_log" 2>&1
    if [[ "$(sed -n 's/^restored_default_ime=//p' "$cleanup_log")" != "$initial_default_ime" || \
          -n "$(sed -n 's/^test_ime_package_path=//p' "$cleanup_log")" ]]; then
      echo 'Test IME settings or package cleanup verification failed.' >> "$cleanup_log"
      printf 'FAIL\n' > "$evidence/test-ime-cleanup-status.txt"
      test_ime_cleanup_result=FAIL
    else
      printf 'PASS\n' > "$evidence/test-ime-cleanup-status.txt"
      test_ime_cleanup_result=PASS
      test_ime_installed=false
    fi
  fi
}
trap 'stop_live_capture; cleanup_test_ime' EXIT
trap 'stop_live_capture; exit 130' INT
trap 'stop_live_capture; exit 143' TERM

capture_device_state() {
  local output_file="$1" status_file="$2" capture_result=0 dump_file="${1}.instrumentation.tmp"
  local adb_state adb_status boot_state boot_status target_pid test_pid
  {
    printf 'captured_at_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)"
    adb_state="$(timeout 10s adb -s "$ANDROID_SERIAL" get-state 2>&1)"
    adb_status=$?
    printf 'adb_state=%s\n' "$adb_state"
    if [[ "$adb_status" -ne 0 ]]; then capture_result=1; fi
    boot_state="$(timeout 10s adb -s "$ANDROID_SERIAL" shell getprop sys.boot_completed 2>&1)"
    boot_status=$?
    boot_state="${boot_state//$'\r'/}"
    printf 'boot_completed=%s\n' "$boot_state"
    if [[ "$boot_status" -ne 0 ]]; then capture_result=1; fi
    target_pid="$(timeout 10s adb -s "$ANDROID_SERIAL" shell pidof app.serea.android.isolated 2>&1)"
    test_pid="$(timeout 10s adb -s "$ANDROID_SERIAL" shell pidof app.serea.android.isolated.test 2>&1)"
    printf 'isolated_target_pid=%s\n' "${target_pid//$'\r'/}"
    printf 'isolated_test_pid=%s\n' "${test_pid//$'\r'/}"
    printf 'instrumentation_state:\n'
    if timeout 10s adb -s "$ANDROID_SERIAL" shell dumpsys activity instrumentation > "$dump_file" 2>&1; then
      if grep -E -q 'Instrumentation|mFinished|mWatcher|target=|runner=' "$dump_file"; then
        grep -E 'Instrumentation|mFinished|mWatcher|target=|runner=' "$dump_file" | head -40
      else
        printf 'No matching active instrumentation record was returned.\n'
      fi
    else
      printf 'Instrumentation state could not be retrieved.\n'
      capture_result=1
    fi
    rm -f "$dump_file"
  } > "$output_file"
  if [[ "$capture_result" -eq 0 ]]; then
    printf 'PASS\n' > "$status_file"
  else
    printf 'FAIL\n' > "$status_file"
  fi
  # Device diagnostics are observational; their status never replaces Gradle's exit code.
  return 0
}

run_diagnostic_stage() {
  local name="$1" filter="$2" timeout_seconds="$3" expected_count="$4" exact_test="$5"
  local stage_dir="$evidence/stages/$name" stage_token gradle_status verify_status verify_exit_code
  if ! rm -rf "$stage_dir" || [[ -e "$stage_dir" ]]; then
    echo "FAIL: could not clear stale evidence for diagnostic stage $name." >&2
    return 1
  fi
  mkdir -p "$stage_dir" || return 1
  printf 'NOT_RUN\n' > "$stage_dir/status.txt"
  printf 'NOT_RUN\n' > "$stage_dir/gradle-status.txt"
  printf 'NOT_RUN\n' > "$stage_dir/gradle-exit-code.txt"
  printf 'NOT_RUN\n' > "$stage_dir/verification-status.txt"
  printf 'NOT_RUN\n' > "$stage_dir/verification-exit-code.txt"
  printf 'NOT_RUN\n' > "$stage_dir/device-state-status.txt"
  if [[ -n "$(adb shell pm path app.serea.android.isolated 2>/dev/null | tr -d '\r')" || \
        -n "$(adb shell pm path app.serea.android.isolated.test 2>/dev/null | tr -d '\r')" ]]; then
    echo 'FAIL: UTP left a stage package installed; refusing to replace it or clear its data.' > "$stage_dir/result.txt"
    printf 'FAIL\n' > "$stage_dir/status.txt"
    return 1
  fi
  if ! adb install app/build/outputs/apk/isolated/app-isolated.apk; then
    echo 'FAIL: could not install the fresh isolated target for this stage.' > "$stage_dir/result.txt"
    printf 'FAIL\n' > "$stage_dir/status.txt"
    return 1
  fi
  local target_paths target_actual target_expected test_expected
  target_paths="$(adb shell pm path app.serea.android.isolated 2>/dev/null | tr -d '\r' | sed -n 's/^package://p')"
  target_actual="$(adb shell sha256sum "$target_paths" 2>/dev/null | awk 'NR == 1 { print $1 }')"
  target_expected="$(sha256sum app/build/outputs/apk/isolated/app-isolated.apk | awk '{print $1}')"
  if [[ -z "$target_paths" || "$target_paths" == *$'\n'* || "$target_actual" != "$target_expected" ]]; then
    echo 'FAIL: could not verify the installed isolated target APK SHA-256.' > "$stage_dir/result.txt"
    printf 'FAIL\n' > "$stage_dir/status.txt"
    return 1
  fi
  stage_token="serea-isolated-api${api_level}-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}-${name}"
  EXPECTED_API_LEVEL="$api_level" bash "$tool_dir/prepare_isolated_instrumentation_target.sh" "$stage_token" \
    > "$stage_dir/seed.log" 2>&1 || {
      echo 'FAIL: host seed could not prove a fresh isolated namespace.' > "$stage_dir/result.txt"
      printf 'FAIL\n' > "$stage_dir/status.txt"
      return 1
    }
  printf 'stage=%s\ntest_filter=%s\nstage_started_at_utc=%s\n' \
    "$name" "$filter" "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" > "$stage_dir/host-lifecycle.txt"
  local bounded_seconds
  bounded_seconds="$(bounded_command_seconds "$timeout_seconds" 60)" || {
    echo 'NOT_RUN: shared Android campaign deadline leaves no safe diagnostic-stage budget.' > "$stage_dir/gradle.log"
    printf 'NOT_RUN\n' > "$stage_dir/gradle-status.txt"
    printf 'NOT_RUN\n' > "$stage_dir/status.txt"
    return 1
  }
  timeout --signal=INT --kill-after=30s "${bounded_seconds}s" ./gradlew --no-daemon \
    -Pandroid.testInstrumentationRunnerArguments.sereaCampaignToken="$stage_token" \
    -Pandroid.testInstrumentationRunnerArguments.listener=app.serea.android.SereaInstrumentationLifecycleListener \
    -Pandroid.testInstrumentationRunnerArguments.class="$filter" connectedIsolatedAndroidTest \
    > "$stage_dir/gradle.log" 2>&1 &
  local gradle_pid=$! install_sha_status=1 test_paths test_actual
  while kill -0 "$gradle_pid" 2>/dev/null; do
    test_paths="$(adb shell pm path app.serea.android.isolated.test 2>/dev/null | tr -d '\r' | sed -n 's/^package://p')"
    if [[ -n "$test_paths" && "$test_paths" != *$'\n'* ]]; then
      test_actual="$(adb shell sha256sum "$test_paths" 2>/dev/null | awk 'NR == 1 { print $1 }')"
      test_expected="$(sha256sum app/build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk 2>/dev/null | awk '{print $1}')"
      if [[ -n "$test_actual" ]]; then
        {
          printf 'source_sha=%s\n' "$(git rev-parse HEAD)"
          printf 'target_package=app.serea.android.isolated\ninstalled_target_apk_sha256=%s\n' "$target_actual"
          printf 'target_expected_apk_sha256=%s\n' "$target_expected"
          printf 'test_package=app.serea.android.isolated.test\ninstalled_test_apk_sha256=%s\n' "$test_actual"
          printf 'test_expected_apk_sha256=%s\n' "$test_expected"
          printf 'target_installed_path=%s\ntest_installed_path=%s\n' "$target_paths" "$test_paths"
        } > "$stage_dir/installed-apk-sha256.txt"
        if [[ "$test_actual" == "$test_expected" ]]; then install_sha_status=0; fi
        break
      fi
    fi
    sleep 0.25
  done
  wait "$gradle_pid"
  gradle_status=$?
  printf '%s\n' "$gradle_status" > "$stage_dir/gradle-exit-code.txt"
  if [[ "$gradle_status" -eq 0 ]]; then
    printf 'PASS\n' > "$stage_dir/gradle-status.txt"
  elif [[ "$gradle_status" -eq 124 || "$gradle_status" -eq 137 ]]; then
    printf 'TIMEOUT\n' > "$stage_dir/gradle-status.txt"
  else
    printf 'FAIL\n' > "$stage_dir/gradle-status.txt"
  fi
  printf 'stage_finished_at_utc=%s\nstage_exit_code=%s\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" "$gradle_status" >> "$stage_dir/host-lifecycle.txt"
  if [[ -d "$root/connected" ]]; then
    mv "$root/connected" "$stage_dir/junit"
  fi
  if [[ -d "$stage_dir/junit" ]]; then
    local verify_args=("$stage_dir/junit" --minimum "$expected_count" --exact-count "$expected_count")
    if [[ -n "$exact_test" ]]; then
      verify_args+=(--require-test "$exact_test")
    else
      verify_args+=(--require-class "$filter=$expected_count")
    fi
    python3 "$tool_dir/verify_android_instrumentation.py" "${verify_args[@]}" \
      > "$stage_dir/verification.log" 2>&1
    verify_exit_code=$?
    printf '%s\n' "$verify_exit_code" > "$stage_dir/verification-exit-code.txt"
    if [[ "$verify_exit_code" -eq 0 ]]; then
      verify_status=0
      printf 'PASS\n' > "$stage_dir/verification-status.txt"
    else
      verify_status=1
      printf 'FAIL\n' > "$stage_dir/verification-status.txt"
    fi
  else
    verify_status=1
    verify_exit_code=125
    printf 'NOT_RUN\n' > "$stage_dir/verification-exit-code.txt"
    printf 'NOT_RUN\n' > "$stage_dir/verification-status.txt"
    echo 'NOT_RUN: diagnostic Gradle stage produced no JUnit directory to verify.' > "$stage_dir/verification.log"
  fi
  if [[ "$gradle_status" -ne 0 || "$verify_status" -ne 0 || "$install_sha_status" -ne 0 ]]; then
    capture_device_state "$stage_dir/device-state.txt" "$stage_dir/device-state-status.txt"
  fi
  cat "$stage_dir/verification.log"
  if [[ "$gradle_status" -eq 0 && "$verify_exit_code" -eq 0 && "$install_sha_status" -eq 0 ]]; then
    printf 'PASS\n' > "$stage_dir/status.txt"
    echo 'PASS: Gradle exit=0 and exact JUnit verification passed.' > "$stage_dir/result.txt"
    return 0
  fi
  printf 'FAIL: gradle_status=%s gradle_exit_code=%s verification_status=%s verification_exit_code=%s install_sha_status=%s\n' \
    "$(cat "$stage_dir/gradle-status.txt")" "$gradle_status" \
    "$(cat "$stage_dir/verification-status.txt")" "$(cat "$stage_dir/verification-exit-code.txt")" "$install_sha_status" \
    > "$stage_dir/result.txt"
  if [[ "$gradle_status" -eq 124 || "$gradle_status" -eq 137 ]]; then
    printf 'NOT_RUN\n' > "$stage_dir/status.txt"
  else
    printf 'FAIL\n' > "$stage_dir/status.txt"
  fi
  return 1
}

run_formal_partition() {
  local partition="$1" filter="$2" task="$3" token="$4" expected_test_apk="$5"
  local partition_dir="$evidence/junit/$partition" log_file="$evidence/$partition-gradle.log"
  local gradle_pid gradle_status install_sha_status=1
  local -a variant_gradle_args=()
  if [[ "$partition" == http ]]; then variant_gradle_args=(-PsereaHttpLoopbackTestVariant=true); fi
  mkdir -p "$partition_dir" || return 1
  rm -rf "$partition_dir"/*
  rm -rf "$root/connected"
  local bounded_seconds
  local partition_cap=840
  if [[ "$partition" == http ]]; then partition_cap=240; fi
  bounded_seconds="$(bounded_command_seconds "$partition_cap" 150)" || {
    echo 'NOT_RUN: shared Android campaign deadline leaves no safe formal-partition budget.' > "$log_file"
    printf '124\n' > "$evidence/$partition-gradle-exit-code.txt"
    printf 'NOT_RUN\n' > "$evidence/$partition-status.txt"
    printf 'TIMEOUT\n' > "$evidence/instrumentation-timeout.txt"
    echo 'Formal partition did not start because the shared API 26/API 35 deadline had no safe command budget.' \
      > "$evidence/instrumentation-timeout-detail.txt"
    return 1
  }
  timeout --signal=INT --kill-after=120s "${bounded_seconds}s" ./gradlew --no-daemon "${variant_gradle_args[@]}" \
    -Pandroid.testInstrumentationRunnerArguments.sereaCampaignToken="$token" \
    -Pandroid.testInstrumentationRunnerArguments.listener=app.serea.android.SereaInstrumentationLifecycleListener \
    -Pandroid.testInstrumentationRunnerArguments.class="$filter" "$task" > "$log_file" 2>&1 &
  gradle_pid=$!
  while kill -0 "$gradle_pid" 2>/dev/null; do
    if bash "$tool_dir/verify_android_installed_apk_sha256.sh" "$expected_test_apk" \
        app.serea.android.isolated.test "$evidence/$partition-installed-apk-sha256.txt" \
        > "$evidence/$partition-apk-attestation-probe.log" 2>&1; then
      {
        printf 'source_sha=%s\n' "$(git rev-parse HEAD)"
        printf 'partition=%s\nfilter=%s\ntask=%s\n' "$partition" "$filter" "$task"
      } >> "$evidence/$partition-installed-apk-sha256.txt"
      install_sha_status=0
      break
    fi
    sleep 0.25
  done
  wait "$gradle_pid"
  gradle_status=$?
  printf '%s\n' "$gradle_status" > "$evidence/$partition-gradle-exit-code.txt"
  if [[ -d "$root/connected" ]]; then mv "$root/connected" "$partition_dir/xml"; fi
  if [[ -d "$partition_dir/xml" ]]; then
    local expected_count
    if [[ "$partition" == offline ]]; then expected_count=101; else expected_count=4; fi
    python3 "$tool_dir/verify_android_instrumentation.py" "$partition_dir/xml" \
      --minimum "$expected_count" --exact-count "$expected_count" \
      > "$evidence/$partition-verification.log" 2>&1
    local verify_status=$?
    printf '%s\n' "$verify_status" > "$evidence/$partition-verification-exit-code.txt"
    if [[ "$gradle_status" -eq 0 && "$verify_status" -eq 0 && "$install_sha_status" -eq 0 ]]; then
      printf 'PASS\n' > "$evidence/$partition-status.txt"
      return 0
    fi
  else
    printf 'NOT_RUN\n' > "$evidence/$partition-verification-exit-code.txt"
    printf 'NOT_RUN: Gradle did not produce JUnit XML for this partition.\n' > "$evidence/$partition-verification.log"
  fi
  if [[ "$gradle_status" -eq 124 || "$gradle_status" -eq 137 ]]; then
    printf 'NOT_RUN\n' > "$evidence/$partition-status.txt"
  else
    printf 'FAIL\n' > "$evidence/$partition-status.txt"
  fi
  return 1
}

cleanup_failed() {
  message="Failed to clear stale API $api_level instrumentation evidence; campaign was not run."
  printf '%s\n' "$message" > "$evidence/device-api-error.txt" || true
  printf 'FAIL\n' > "$evidence/api-result.txt" || true
  echo "### Android API $api_level: instrumentation=NOT_RUN process-death=NOT_RUN device-api=NOT_RUN cleanup=FAIL" \
    >> "$GITHUB_STEP_SUMMARY"
  echo "$message" >&2
  exit 1
}

if ! rm -rf "$root/connected" "$evidence/junit" "$evidence/stages" || \
   [[ -e "$root/connected" || -e "$evidence/junit" || -e "$evidence/stages" ]]; then
  cleanup_failed
fi
if ! rm -f \
  "$evidence/device-api-error.txt" \
  "$evidence/junit-verification.log" \
  "$evidence/process-death.log" \
  "$evidence/data-isolation-error.txt" \
  "$evidence/test-lifecycle-summary.txt" \
  "$evidence/instrumentation-timeout-detail.txt" || \
  [[ -e "$evidence/device-api-error.txt" || -e "$evidence/junit-verification.log" || -e "$evidence/process-death.log" || -e "$evidence/data-isolation-error.txt" || -e "$evidence/test-lifecycle-summary.txt" || -e "$evidence/instrumentation-timeout-detail.txt" ]]; then
  cleanup_failed
fi

EXPECTED_API_LEVEL="$api_level"
SEREA_EXPECTED_ANDROID_API="$api_level"
export EXPECTED_API_LEVEL SEREA_EXPECTED_ANDROID_API
if ! source "$tool_dir/assert_disposable_android_emulator.sh"; then
  observed_api="${SEREA_OBSERVED_ANDROID_API:-}"
  observed_qemu="${SEREA_OBSERVED_ANDROID_QEMU:-}"
  if [[ -n "$observed_api" ]]; then
    printf '%s\n' "$observed_api" > "$evidence/device-api-level.txt"
    {
      echo "Expected API $api_level, observed '$observed_api'."
      echo "Observed ro.kernel.qemu=$observed_qemu."
    } > "$evidence/device-api-error.txt"
    printf 'FAIL\n' > "$evidence/api-result.txt"
    echo "### Android API $api_level: instrumentation=NOT_RUN process-death=NOT_RUN data-isolation=NOT_RUN device-api=$observed_api" \
      >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
  fi
  exit 1
fi

sdk="$(adb shell getprop ro.build.version.sdk | tr -d '\r')"
printf '%s\n' "$sdk" > "$evidence/device-api-level.txt"
sdk_ok=false
if [[ "$sdk" == "$api_level" ]]; then
  sdk_ok=true
else
  echo "Expected API $api_level, observed '$sdk'." > "$evidence/device-api-error.txt"
fi
if [[ "$sdk_ok" == true ]]; then
  source "$tool_dir/wait_for_android_default_ime.sh"
  if wait_for_android_default_ime; then
    initial_default_ime="$SEREA_ANDROID_DEFAULT_IME"
    {
      printf 'status=READY\ninitial_default_ime=%s\nattempts=%s\n' \
        "$initial_default_ime" "$SEREA_ANDROID_DEFAULT_IME_ATTEMPTS"
      printf 'available_ime_services:\n'
      adb shell ime list -s 2>&1
    } > "$evidence/ime-readiness.txt"
  else
    initial_default_ime="$SEREA_ANDROID_DEFAULT_IME"
    {
      printf 'status=NOT_READY\nobserved_default_ime=%s\nattempts=%s\n' \
        "$initial_default_ime" "$SEREA_ANDROID_DEFAULT_IME_ATTEMPTS"
      printf 'available_ime_services:\n'
      adb shell ime list -s 2>&1
      printf 'input_method_manager:\n'
      adb shell dumpsys input_method 2>&1
    } > "$evidence/ime-readiness.txt"
    adb logcat -d -b all > "$evidence/ime-readiness-logcat.txt" 2>&1 || true
    echo 'Could not capture the disposable emulator default IME after 30 readiness checks.' > "$evidence/device-api-error.txt"
    sdk_ok=false
  fi
fi

gradle_result=1
junit_result=NOT_RUN
data_isolation_result=NOT_RUN
data_sentinel_token=''
isolated_target_result=NOT_RUN
process_death_target_ready=false
if [[ "$sdk_ok" == true ]]; then
  diagnostics_result=NOT_RUN
  demo_package='app.serea.android.demo'
  isolated_package='app.serea.android.isolated'
  test_package='app.serea.android.isolated.test'
  ime_package='app.serea.android.r25testime'
  debug_apk='app/build/outputs/apk/debug/app-debug.apk'
  isolated_apk='app/build/outputs/apk/isolated/app-isolated.apk'
  if [[ "$api_level" == 26 ]]; then
    # Android 8 accepts `--user all` but returns an empty successful result.
    # Its default query enumerates the disposable emulator's system user.
    retained_packages="$(adb shell pm list packages -u 2>/dev/null)"
  else
    retained_packages="$(adb shell pm list packages -u --user all 2>/dev/null)"
  fi
  retained_packages_result=$?
  retained_packages="$(printf '%s' "$retained_packages" | tr -d '\r')"
  if [[ "$retained_packages_result" -ne 0 ]]; then
    echo "Unable to prove that no installed or retained Serea package data exists; refusing campaign." > "$evidence/data-isolation-error.txt"
    data_isolation_result=FAIL
    printf 'FAIL\n' > "$evidence/data-isolation-status.txt"
  elif ! printf '%s\n' "$retained_packages" | awk '
       NF { if ($0 !~ /^package:[A-Za-z0-9_.]+$/) malformed=1; count++ }
       END { exit (count > 0 && !malformed) ? 0 : 1 }
     '; then
    echo "Installed/retained package inventory was empty or malformed; refusing campaign." > "$evidence/data-isolation-error.txt"
    data_isolation_result=FAIL
    printf 'FAIL\n' > "$evidence/data-isolation-status.txt"
  elif { printf '%s\n' "$retained_packages" | grep -Fxq "package:$demo_package" || \
       printf '%s\n' "$retained_packages" | grep -Fxq "package:$isolated_package" || \
       printf '%s\n' "$retained_packages" | grep -Fxq "package:$test_package" || \
       printf '%s\n' "$retained_packages" | grep -Fxq "package:$ime_package" || \
       [[ -n "$(adb shell pm path "$demo_package" 2>/dev/null | tr -d '\r')" || \
          -n "$(adb shell pm path "$isolated_package" 2>/dev/null | tr -d '\r')" || \
          -n "$(adb shell pm path "$test_package" 2>/dev/null | tr -d '\r')" || \
          -n "$(adb shell pm path "$ime_package" 2>/dev/null | tr -d '\r')" ]]; }; then
    echo "Refusing emulator with an installed package or retained Serea app data; no app data will be cleared." > "$evidence/data-isolation-error.txt"
    data_isolation_result=FAIL
    printf 'FAIL\n' > "$evidence/data-isolation-status.txt"
  else
    if ! python3 "$tool_dir/android_instrumentation_variant_filters.py" \
      --required "$tool_dir/android-instrumentation-required-tests.txt" \
      --additional "$tool_dir/android-instrumentation-additional-tests.txt" \
      --non-required app/src/androidTest/assets/serea-non-required-test-identities.txt \
      --http-tests "$tool_dir/android-instrumentation-http-loopback-tests.txt" \
      --source-root app/src/androidTest/java --output "$variant_filters"; then
      echo 'FAIL: could not prove exact 105-test offline/HTTP instrumentation partition.' > "$evidence/partition-results.txt"
      printf 'FAIL\n' > "$evidence/api-result.txt"
      echo "### Android API $api_level: instrumentation=NOT_RUN process-death=NOT_RUN data-isolation=FAIL device-api=$sdk" \
        >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
      exit 1
    fi
    offline_filter="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8"))["offline_filter"])' "$variant_filters")"
    http_filter="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1], encoding="utf-8"))["http_filter"])' "$variant_filters")"
    token="serea-api${api_level}-${GITHUB_RUN_ID:?GITHUB_RUN_ID required}-${GITHUB_RUN_ATTEMPT:?GITHUB_RUN_ATTEMPT required}"
    data_sentinel_token="$token"
    run_bounded_gradle 240 60 "$evidence/instrumentation-gradle.log" \
      :app:assembleDebug :app:assembleIsolated :r25-ime-test:assembleDebug
    build_debug_result=$?
    ime_apk='r25-ime-test/build/outputs/apk/debug/r25-test-ime-debug.apk'
    if [[ ! -s "$ime_apk" ]]; then ime_apk='r25-ime-test/build/outputs/apk/debug/r25-ime-test-debug.apk'; fi
    if [[ "$build_debug_result" -eq 0 && -s "$ime_apk" ]] && adb install "$ime_apk"; then
      test_ime_installed=true
      sha256sum "$ime_apk" > "$evidence/test-ime-apk-sha256.txt"
      printf 'NOT_RUN\n' > "$evidence/test-ime-cleanup-status.txt"
      test_ime_install_result=0
    else
      test_ime_install_result=1
    fi
    if [[ "$build_debug_result" -eq 0 && -s "$debug_apk" && -s "$isolated_apk" ]]; then
      adb install "$debug_apk"
      install_demo_result=$?
    else
      install_demo_result=1
    fi
    if [[ "$install_demo_result" -eq 0 && "$test_ime_install_result" -eq 0 ]]; then
      EXPECTED_API_LEVEL="$api_level" bash "$tool_dir/run_android_data_sentinel.sh" "$token" seed \
        > "$evidence/data-isolation.log" 2>&1
      sentinel_seed_result=$?
    else
      sentinel_seed_result=1
      echo "Could not install fresh demo and isolated packages for the instrumentation campaign." > "$evidence/data-isolation.log"
    fi
    if [[ "$sentinel_seed_result" -eq 0 && -z "$(adb shell pm path "$isolated_package" 2>/dev/null | tr -d '\r')" && \
          -z "$(adb shell pm path "$test_package" 2>/dev/null | tr -d '\r')" ]]; then
      campaign_token="serea-isolated-api${api_level}-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"
      printf 'api_level=%s\ncampaign_token=%s\ncampaign_started_at_utc=%s\n' \
        "$api_level" "$campaign_token" "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" > "$evidence/test-lifecycle-host.txt"
      adb -s "$ANDROID_SERIAL" logcat -v threadtime -s \
        SereaTestLifecycle:I SereaCalendarStage:I SereaIsolatedRunner:I SereaProfileVerifier:I \
        SereaComposerIme:I SereaRealIme:I \
        SereaProcessDeath:I SereaChatWindowFocus:E SereaChatInputRegression:I \
        chat-demo-label:I chat-current-delete-confirmation:I \
        chat-current-delete-status:I chat-delete-all-confirmation:I chat-delete-all-status:I \
        chat-restored-conversation:I chat-draft-gmail-search-timeout:I \
        font-scale-gmail-heading:I gmail-search-result-timeout:I \
        gmail-repeat-search-result-timeout:I task-proposal-font-scale:I AndroidRuntime:E \
        > "$evidence/logcat-live.txt" 2>&1 &
      logcat_pid=$!
      diagnostics_result=0
      if [[ "$api_level" == 35 ]]; then
        # Focused companion run preserves the full suite and captures complete window/IME diagnostics.
        run_diagnostic_stage cursor-position-focused \
          app.serea.android.SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges \
          120 1 app.serea.android.SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges || diagnostics_result=1
        if [[ "$diagnostics_result" -eq 0 ]]; then
          run_diagnostic_stage calendar-class app.serea.android.SereaAppShellTest 420 19 '' || diagnostics_result=1
        fi
        if [[ "$diagnostics_result" -eq 0 ]]; then
          run_diagnostic_stage chat-screen-class app.serea.android.ChatScreenTest 360 16 '' || diagnostics_result=1
        fi
      else
        run_diagnostic_stage cursor-position-focused \
          app.serea.android.SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges \
          120 1 app.serea.android.SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges || diagnostics_result=1
        if [[ "$diagnostics_result" -eq 0 ]]; then
          run_diagnostic_stage calendar-class app.serea.android.SereaAppShellTest 420 19 '' || diagnostics_result=1
        fi
        if [[ "$diagnostics_result" -eq 0 ]]; then
          run_diagnostic_stage chat-screen-class app.serea.android.ChatScreenTest 360 16 '' || diagnostics_result=1
        fi
      fi
      http_target_apk='app/build/outputs/apk/isolatedHttp/app-isolatedHttp.apk'
      http_test_apk='app/build/outputs/apk/androidTest/isolatedHttp/app-isolatedHttp-androidTest.apk'
      http_variant_build_result=1
      if [[ "$diagnostics_result" -eq 0 ]]; then
        run_bounded_gradle 240 60 "$evidence/http-variant-build.log" \
          -PsereaHttpLoopbackTestVariant=true :app:assembleIsolatedHttp :app:assembleIsolatedHttpAndroidTest
        http_variant_build_result=$?
        if [[ "$http_variant_build_result" -eq 0 && -s "$http_target_apk" && -s "$http_test_apk" ]]; then
          if [[ -z "${ANDROID_HOME:-}" || ! -x "$ANDROID_HOME/build-tools/35.0.0/aapt" ]]; then
            echo 'FAIL: Android build-tools 35.0.0/aapt is unavailable for APK permission verification.' \
              >> "$evidence/http-variant-build.log"
            http_variant_build_result=1
          else
            printf 'status=NOT_RUN\n' > "$evidence/apk-permission-boundary.txt"
            if python3 "$tool_dir/verify_android_http_test_apk_permissions.py" \
              --aapt "$ANDROID_HOME/build-tools/35.0.0/aapt" \
              --offline-target app/build/outputs/apk/isolated/app-isolated.apk \
              --offline-test app/build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk \
              --http-target "$http_target_apk" --http-test "$http_test_apk" \
              --evidence "$evidence/apk-permission-boundary.txt" \
              >> "$evidence/http-variant-build.log" 2>&1; then
              printf 'status=PASS\n' >> "$evidence/apk-permission-boundary.txt"
            else
              http_variant_build_result=1
            fi
          fi
        else
          echo 'FAIL: isolatedHttp target or instrumentation APK is missing.' >> "$evidence/http-variant-build.log"
          http_variant_build_result=1
        fi
      fi
      printf 'source_sha=%s\ntree_sha=%s\napi_level=%s\n' \
        "$(git rev-parse HEAD)" "$(git rev-parse HEAD^{tree})" "$api_level" \
        >> "$evidence/apk-permission-boundary.txt" 2>/dev/null || true
      cat "$evidence/http-variant-build.log" 2>/dev/null || true
      if [[ "$diagnostics_result" -eq 0 && "$http_variant_build_result" -eq 0 ]] && \
          bash "$tool_dir/prepare_android_instrumentation_variant_target.sh" "$isolated_apk" offline \
            "$campaign_token" "$evidence/offline-installed-target.txt" "$tool_dir" "$api_level"; then
        isolated_target_result=0
      else
        isolated_target_result=1
      fi
      if [[ "$diagnostics_result" -eq 0 ]]; then
        printf 'PASS\n' > "$evidence/diagnostic-status.txt"
      else
        printf 'FAIL\n' > "$evidence/diagnostic-status.txt"
      fi
      if [[ "$isolated_target_result" -eq 0 ]]; then
        printf 'instrumentation_started_at_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" >> "$evidence/test-lifecycle-host.txt"
        printf 'offline_filter_count=101\nhttp_filter_count=4\n' > "$evidence/partition-contract.txt"
        printf 'offline_filter=%s\nhttp_filter=%s\n' "$offline_filter" "$http_filter" \
          >> "$evidence/partition-contract.txt"
        if run_formal_partition offline "$offline_filter" connectedIsolatedAndroidTest "$campaign_token" \
          app/build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk; then
          offline_partition_result=0
        else
          offline_partition_result=1
        fi
        cat "$evidence/offline-gradle.log" 2>/dev/null || true
        http_campaign_token="serea-isolated-api${api_level}-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}-http"
        if bash "$tool_dir/prepare_android_instrumentation_variant_target.sh" "$http_target_apk" http \
            "$http_campaign_token" "$evidence/http-installed-target.txt" "$tool_dir" "$api_level"; then
          if run_formal_partition http "$http_filter" connectedIsolatedHttpAndroidTest "$http_campaign_token" "$http_test_apk"; then
            http_partition_result=0
          else
            http_partition_result=1
          fi
        else
          http_partition_result=1
          printf 'FAIL\n' > "$evidence/http-status.txt"
          echo 'FAIL: fresh isolatedHttp package could not be installed, hash-verified, and seeded.' \
            > "$evidence/http-verification.log"
        fi
        cat "$evidence/http-gradle.log" 2>/dev/null || true
        partition_identity_status=NOT_RUN
        if [[ -d "$evidence/junit/offline/xml" && -d "$evidence/junit/http/xml" ]]; then
          python3 "$tool_dir/verify_android_instrumentation_partition_results.py" \
            --filters "$variant_filters" --offline-junit "$evidence/junit/offline/xml" \
            --http-junit "$evidence/junit/http/xml" --evidence-dir "$evidence" --api-level "$api_level" \
            > "$evidence/partition-identity-verification.log" 2>&1
          partition_identity_exit=$?
          if [[ "$partition_identity_exit" -eq 0 ]]; then
            partition_identity_status=PASS
          else
            partition_identity_status=FAIL
            gradle_result=1
          fi
        else
          echo 'NOT_RUN: one or both partition JUnit directories are absent.' \
            > "$evidence/partition-identity-verification.log"
        fi
        printf '%s\n' "$partition_identity_status" > "$evidence/partition-identity-status.txt"
        cat "$evidence/partition-identity-verification.log"
        if [[ "$offline_partition_result" -eq 0 && "$http_partition_result" -eq 0 ]]; then
          if [[ "$partition_identity_status" == PASS ]]; then gradle_result=0; else gradle_result=1; fi
        else
          gradle_result=1
        fi
        printf 'offline=%s\nhttp=%s\n' "$(cat "$evidence/offline-status.txt" 2>/dev/null || echo NOT_RUN)" \
          "$(cat "$evidence/http-status.txt" 2>/dev/null || echo NOT_RUN)" > "$evidence/partition-results.txt"
        if grep -Eq '^(124|137)$' "$evidence/offline-gradle-exit-code.txt" "$evidence/http-gradle-exit-code.txt" 2>/dev/null; then
          printf 'TIMEOUT\n' > "$evidence/instrumentation-timeout.txt"
          echo 'At least one bounded instrumentation partition reached its per-partition or shared campaign deadline.' \
            > "$evidence/instrumentation-timeout-detail.txt"
        elif [[ -f "$evidence/offline-gradle-exit-code.txt" && -f "$evidence/http-gradle-exit-code.txt" ]]; then
          printf 'NOT_TIMED_OUT\n' > "$evidence/instrumentation-timeout.txt"
        fi
        # The HTTP APK has only been used for the four synthetic loopback identities. Remove both
        # disposable package installs before the process-death harness reinstalls the offline APK.
        for package in app.serea.android.isolated.test app.serea.android.isolated; do
          package_path_output="$(adb shell pm path "$package" 2>/dev/null)"
          package_path_query_result=$?
          package_path_output="$(printf '%s' "$package_path_output" | tr -d '\r')"
          if [[ "$package_path_query_result" -ne 0 ]]; then
            echo "FAIL: Package Manager path query failed before cleanup for $package." \
              >> "$evidence/partition-results.txt"
            gradle_result=1
            continue
          fi
          if [[ -n "$package_path_output" ]]; then
            adb uninstall "$package" >> "$evidence/partition-results.txt" 2>&1 || gradle_result=1
          fi
          remaining_path_output="$(adb shell pm path "$package" 2>/dev/null)"
          remaining_path_query_result=$?
          remaining_path_output="$(printf '%s' "$remaining_path_output" | tr -d '\r')"
          if [[ "$remaining_path_query_result" -ne 0 || -n "$remaining_path_output" ]]; then
            echo "FAIL: disposable package $package remains after formal partitions." >> "$evidence/partition-results.txt"
            gradle_result=1
          fi
        done
        {
          printf 'instrumentation_finished_at_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)"
          printf 'instrumentation_exit_code=%s\n' "$gradle_result"
        } >> "$evidence/test-lifecycle-host.txt"
        printf '%s\n' "$gradle_result" > "$evidence/instrumentation-gradle-exit-code.txt"
        if [[ "$gradle_result" -ne 0 ]]; then
          capture_device_state "$evidence/test-device-state.txt" "$evidence/test-device-state-status.txt"
        fi
      else
        gradle_result=1
        if [[ "$diagnostics_result" -ne 0 ]]; then
          echo "Formal 101+4 instrumentation partitions were NOT_RUN because a bounded diagnostic or APK permission stage failed; see evidence." \
            > "$evidence/junit-verification.log"
          printf 'NOT_RUN\n' > "$evidence/instrumentation-status.txt"
        else
          echo "Fresh isolated instrumentation target could not be proven." >> "$evidence/data-isolation.log"
        fi
      fi
    else
      gradle_result=1
      echo "Isolated test package was present before instrumentation or sentinel setup failed." >> "$evidence/data-isolation.log"
    fi
    if [[ -n "$(adb shell pm path "$demo_package" 2>/dev/null | tr -d '\r')" ]]; then
      EXPECTED_API_LEVEL="$api_level" bash "$tool_dir/run_android_data_sentinel.sh" "$token" verify \
        >> "$evidence/data-isolation.log" 2>&1
      sentinel_verify_result=$?
    else
      sentinel_verify_result=1
      echo "Demo package disappeared during instrumentation." >> "$evidence/data-isolation.log"
    fi
    if [[ "$sentinel_seed_result" -eq 0 && "$sentinel_verify_result" -eq 0 ]]; then
      data_isolation_result=PASS
      printf 'PASS\n' > "$evidence/data-isolation-status.txt"
      printf 'debug_package=%s\nisolated_package=%s\nisolated_test_package=%s\nsentinel_token=%s\ncampaign_token=%s\n' \
        "$demo_package" "$isolated_package" "$test_package" "$token" "$campaign_token" >> "$evidence/data-isolation.log"
    else
      data_isolation_result=FAIL
      printf 'FAIL\n' > "$evidence/data-isolation-status.txt"
      echo "Sentinel history/preferences were not proven to survive the instrumentation campaign." > "$evidence/data-isolation-error.txt"
    fi
  fi
  if [[ -d "$root/connected" ]]; then
    mv "$root/connected" "$evidence/junit"
  fi

  # Always create a new isolated namespace for Process Death. Never trust a marker
  # left by diagnostics, UTP, or a partially completed instrumentation run.
  if [[ "$data_isolation_result" == PASS && -n "$campaign_token" ]]; then
    isolated_path="$(adb shell pm path "$isolated_package" 2>/dev/null | tr -d '\r')"
    uninstall_result=0
    if [[ -n "$isolated_path" ]]; then
      echo "Removing the campaign-owned isolated target before a fresh Process Death seed." \
        >> "$evidence/process-death-readiness.log"
      adb uninstall "$isolated_package" >> "$evidence/data-isolation.log" 2>&1
      uninstall_result=$?
    fi
    isolated_path_after_uninstall="$(adb shell pm path "$isolated_package" 2>/dev/null | tr -d '\r')"
    if [[ "$api_level" == 26 ]]; then
      retained_after_uninstall="$(adb shell pm list packages -u 2>/dev/null)"
    else
      retained_after_uninstall="$(adb shell pm list packages -u --user all 2>/dev/null)"
    fi
    package_inventory_result=$?
    retained_target_count="$(printf '%s\n' "$retained_after_uninstall" | grep -F -x -c "package:$isolated_package")"
    if [[ "$uninstall_result" -ne 0 || -n "$isolated_path_after_uninstall" || \
          "$package_inventory_result" -ne 0 || "$retained_target_count" -ne 0 ]]; then
      echo "FAIL: isolated Process Death target could not be proven absent before reseeding." \
        >> "$evidence/process-death-readiness.log"
      printf 'FAIL\n' > "$evidence/process-death-readiness-status.txt"
    elif adb install "$isolated_apk" && [[ -s "$isolated_apk" ]]; then
      EXPECTED_API_LEVEL="$api_level" bash "$tool_dir/prepare_isolated_instrumentation_target.sh" \
        "serea-isolated-api${api_level}-${GITHUB_RUN_ID:?GITHUB_RUN_ID required}-${GITHUB_RUN_ATTEMPT:?GITHUB_RUN_ATTEMPT required}" process-death \
        >> "$evidence/data-isolation.log" 2>&1
      reseed_result=$?
      if [[ "$reseed_result" -eq 0 && -s "$isolated_apk" && \
            -n "$(adb shell pm path "$isolated_package" 2>/dev/null | tr -d '\r')" ]]; then
        process_death_target_ready=true
        printf 'PASS\n' > "$evidence/process-death-readiness-status.txt"
        echo "PASS: freshly installed isolated target accepted the Process Death seed." \
          >> "$evidence/process-death-readiness.log"
      else
        printf 'FAIL\n' > "$evidence/process-death-readiness-status.txt"
        echo "FAIL: fresh Process Death target installation or seed was not verified." \
          >> "$evidence/process-death-readiness.log"
      fi
    else
      printf 'FAIL\n' > "$evidence/process-death-readiness-status.txt"
      echo "FAIL: could not install the isolated APK for a fresh Process Death seed." \
        >> "$evidence/process-death-readiness.log"
    fi
  fi

  if [[ -d "$evidence/junit" ]]; then
    python3 "$tool_dir/verify_android_instrumentation.py" "$evidence/junit" \
      --source-root app/src/androidTest/java --minimum 105 --exact-count 105 \
      --required-test-manifest "$tool_dir/android-instrumentation-required-tests.txt" \
      --additional-test-manifest "$tool_dir/android-instrumentation-additional-tests.txt" \
      --non-required-test-manifest app/src/androidTest/assets/serea-non-required-test-identities.txt \
      --required-count 103 --additional-count 2 \
      --api-level "$api_level" --profile-verifier-log "$evidence/logcat-live.txt" \
      --require-class app.serea.android.EncryptedHistoryTest=17 \
      --require-class app.serea.android.ChatScreenTest=16 \
      --require-class app.serea.android.ChatHttpE2ETest=4 \
      --require-class app.serea.android.UsageLedgerDatabaseTest=12 \
      --require-class app.serea.android.tasks.TasksDatabaseTest=6 \
      --require-class app.serea.android.tasks.TaskScreenTest=6 \
      --require-class app.serea.android.InstrumentationStorageIsolationTest=1 \
      --require-class app.serea.android.ChatTasksDatabaseIntegrationTest=1 \
      --require-class app.serea.android.ChatHistoryUxTest=4 \
      --require-class app.serea.android.tasks.TaskDateTimePickerTest=6 \
      --require-class app.serea.android.assistant.taskproposal.TaskProposalReviewScreenTest=2 \
      --require-class app.serea.android.SereaAppShellTest=19 \
      --require-class app.serea.android.assistant.taskproposal.TaskProposalDemoScreenTest=3 \
      --require-class app.serea.android.SereaThemeTest=2 \
      --require-class app.serea.android.MockUiStateIntegrationTest=4 \
      > "$evidence/junit-verification.log" 2>&1
    verify_result=$?
    printf '%s\n' "$verify_result" > "$evidence/instrumentation-verification-exit-code.txt"
    cat "$evidence/junit-verification.log"
    if [[ "$verify_result" -eq 0 ]]; then
      junit_result=PASS
    else
      junit_result=FAIL
    fi
  elif [[ "$gradle_result" -ne 0 ]]; then
    if [[ "$diagnostics_result" != NOT_RUN && "$diagnostics_result" -ne 0 ]] || \
        { [[ ! -d "$evidence/junit/offline/xml" ]] && [[ ! -d "$evidence/junit/http/xml" ]]; }; then
      junit_result=NOT_RUN
      printf 'NOT_RUN\n' > "$evidence/instrumentation-verification-exit-code.txt"
      echo "Formal partition JUnit was NOT_RUN because diagnostics/setup did not start the instrumentation tasks; partial stage output is retained separately." \
        > "$evidence/junit-verification.log"
    else
      junit_result=FAIL
      printf 'NOT_RUN\n' > "$evidence/instrumentation-verification-exit-code.txt"
      echo "Instrumentation Gradle command failed with exit code $gradle_result and produced no connected JUnit directory." \
        > "$evidence/junit-verification.log"
    fi
  else
    junit_result=FAIL
    printf 'NOT_RUN\n' > "$evidence/instrumentation-verification-exit-code.txt"
    echo "Instrumentation Gradle command succeeded but produced no connected JUnit directory." \
      > "$evidence/junit-verification.log"
  fi
  if [[ "$gradle_result" -eq 0 && "$junit_result" == FAIL ]]; then
    capture_device_state "$evidence/test-device-state.txt" "$evidence/test-device-state-status.txt"
  fi
fi
printf '%s\n' "$junit_result" > "$evidence/instrumentation-status.txt"

death_result=NOT_RUN
if [[ "$sdk_ok" == true && "$data_isolation_result" == PASS && "$process_death_target_ready" == true ]]; then
  process_death_timeout="$(bounded_command_seconds 120 60)"
  if [[ -z "$process_death_timeout" ]]; then
    printf 'NOT_RUN\n' > "$evidence/process-death-status.txt"
    echo 'NOT_RUN: shared campaign deadline leaves no safe Process Death budget.' > "$evidence/process-death.log"
    process_death_target_ready=false
  else
  printf 'process_death_started_at_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" \
    > "$evidence/process-death-host-lifecycle.txt"
  EXPECTED_API_LEVEL="$api_level" SEREA_SENTINEL_TOKEN="$data_sentinel_token" \
    SEREA_CAMPAIGN_TOKEN="serea-isolated-api${api_level}-${GITHUB_RUN_ID:?GITHUB_RUN_ID required}-${GITHUB_RUN_ATTEMPT:?GITHUB_RUN_ATTEMPT required}" \
    timeout --signal=INT --kill-after=30s "${process_death_timeout}s" bash "$tool_dir/run_android_process_death_harness.sh" \
    > "$evidence/process-death.log" 2>&1
  death_exit=$?
  printf 'process_death_finished_at_utc=%s\nprocess_death_exit_code=%s\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" "$death_exit" \
    >> "$evidence/process-death-host-lifecycle.txt"
  cat "$evidence/process-death.log"
  if [[ "$death_exit" -eq 0 ]]; then
    death_result=PASS
  else
    death_result=FAIL
  fi
  fi
elif [[ "$sdk_ok" == true ]]; then
  printf 'NOT_RUN\n' > "$evidence/process-death-host-lifecycle.txt"
  echo "Process Death not run because the disposable-device, demo-sentinel, or isolated-target readiness gate did not pass." \
    > "$evidence/process-death.log"
fi
printf '%s\n' "$death_result" > "$evidence/process-death-status.txt"

if [[ "$test_ime_installed" == true ]]; then cleanup_test_ime; fi

if [[ "$sdk_ok" == true && "$gradle_result" -eq 0 && "$junit_result" == PASS && "$death_result" == PASS && "$data_isolation_result" == PASS && "$test_ime_cleanup_result" == PASS ]]; then
  result=PASS
else
  result=FAIL
fi
printf '%s\n' "$result" > "$evidence/api-result.txt"
echo "### Android API $api_level: instrumentation=$junit_result process-death=$death_result data-isolation=$data_isolation_result device-api=$sdk" \
  >> "$GITHUB_STEP_SUMMARY"
if [[ "$result" != PASS ]]; then
  exit 1
fi
