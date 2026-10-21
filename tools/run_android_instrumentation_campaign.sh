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

root='app/build/outputs/androidTest-results'
evidence="$root/api$api_level"
mkdir -p "$evidence" || exit 1
printf 'NOT_RUN\n' > "$evidence/instrumentation-status.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/process-death-status.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/device-api-level.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/api-result.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/data-isolation-status.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/data-isolation.log" || exit 1

cleanup_failed() {
  message="Failed to clear stale API $api_level instrumentation evidence; campaign was not run."
  printf '%s\n' "$message" > "$evidence/device-api-error.txt" || true
  printf 'FAIL\n' > "$evidence/api-result.txt" || true
  echo "### Android API $api_level: instrumentation=NOT_RUN process-death=NOT_RUN device-api=NOT_RUN cleanup=FAIL" \
    >> "$GITHUB_STEP_SUMMARY"
  echo "$message" >&2
  exit 1
}

if ! rm -rf "$root/connected" "$evidence/junit" || [[ -e "$root/connected" || -e "$evidence/junit" ]]; then
  cleanup_failed
fi
if ! rm -f \
  "$evidence/device-api-error.txt" \
  "$evidence/junit-verification.log" \
  "$evidence/process-death.log" \
  "$evidence/data-isolation-error.txt" || \
  [[ -e "$evidence/device-api-error.txt" || -e "$evidence/junit-verification.log" || -e "$evidence/process-death.log" || -e "$evidence/data-isolation-error.txt" ]]; then
  cleanup_failed
fi

EXPECTED_API_LEVEL="$api_level"
export EXPECTED_API_LEVEL
source "$tool_dir/assert_disposable_android_emulator.sh" || exit 1

sdk="$(adb shell getprop ro.build.version.sdk | tr -d '\r')"
printf '%s\n' "$sdk" > "$evidence/device-api-level.txt"
sdk_ok=false
if [[ "$sdk" == "$api_level" ]]; then
  sdk_ok=true
else
  echo "Expected API $api_level, observed '$sdk'." > "$evidence/device-api-error.txt"
fi

gradle_result=1
junit_result=NOT_RUN
data_isolation_result=NOT_RUN
data_sentinel_token=''
isolated_target_result=NOT_RUN
if [[ "$sdk_ok" == true ]]; then
  demo_package='app.serea.android.demo'
  isolated_package='app.serea.android.isolated'
  test_package='app.serea.android.isolated.test'
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
       [[ -n "$(adb shell pm path "$demo_package" 2>/dev/null | tr -d '\r')" || \
          -n "$(adb shell pm path "$isolated_package" 2>/dev/null | tr -d '\r')" || \
          -n "$(adb shell pm path "$test_package" 2>/dev/null | tr -d '\r')" ]]; }; then
    echo "Refusing emulator with an installed package or retained Serea app data; no app data will be cleared." > "$evidence/data-isolation-error.txt"
    data_isolation_result=FAIL
    printf 'FAIL\n' > "$evidence/data-isolation-status.txt"
  else
    token="serea-api${api_level}-${GITHUB_RUN_ID:?GITHUB_RUN_ID required}-${GITHUB_RUN_ATTEMPT:?GITHUB_RUN_ATTEMPT required}"
    data_sentinel_token="$token"
    ./gradlew --no-daemon :app:assembleDebug :app:assembleIsolated
    build_debug_result=$?
    if [[ "$build_debug_result" -eq 0 && -s "$debug_apk" && -s "$isolated_apk" ]]; then
      adb install "$debug_apk"
      install_demo_result=$?
    else
      install_demo_result=1
    fi
    if [[ "$install_demo_result" -eq 0 ]]; then
      EXPECTED_API_LEVEL="$api_level" bash "$tool_dir/run_android_data_sentinel.sh" "$token" seed \
        > "$evidence/data-isolation.log" 2>&1
      sentinel_seed_result=$?
    else
      sentinel_seed_result=1
      echo "Could not install fresh demo and isolated packages for the instrumentation campaign." > "$evidence/data-isolation.log"
    fi
    if [[ "$sentinel_seed_result" -eq 0 && -z "$(adb shell pm path "$isolated_package" 2>/dev/null | tr -d '\r')" && \
          -z "$(adb shell pm path "$test_package" 2>/dev/null | tr -d '\r')" ]]; then
      adb install "$isolated_apk"
      isolated_install_result=$?
      campaign_token="serea-isolated-api${api_level}-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}"
      if [[ "$isolated_install_result" -eq 0 ]]; then
        EXPECTED_API_LEVEL="$api_level" bash "$tool_dir/prepare_isolated_instrumentation_target.sh" "$campaign_token" \
          >> "$evidence/data-isolation.log" 2>&1
        isolated_target_result=$?
      else
        isolated_target_result=1
      fi
      if [[ "$isolated_target_result" -eq 0 ]]; then
        ./gradlew --no-daemon \
          -Pandroid.testInstrumentationRunnerArguments.sereaCampaignToken="$campaign_token" \
          connectedIsolatedAndroidTest
        gradle_result=$?
      else
        gradle_result=1
        echo "Fresh isolated instrumentation target could not be proven." >> "$evidence/data-isolation.log"
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
    if [[ "$sentinel_seed_result" -eq 0 && "$sentinel_verify_result" -eq 0 && "$isolated_target_result" -eq 0 ]]; then
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

  if [[ -d "$evidence/junit" ]]; then
    python3 "$tool_dir/verify_android_instrumentation.py" "$evidence/junit" \
      --source-root app/src/androidTest/java --minimum 103 \
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
    cat "$evidence/junit-verification.log"
    if [[ "$verify_result" -eq 0 ]]; then
      junit_result=PASS
    else
      junit_result=FAIL
    fi
  elif [[ "$gradle_result" -ne 0 ]]; then
    junit_result=FAIL
    echo "Instrumentation Gradle command failed with exit code $gradle_result and produced no connected JUnit directory." \
      > "$evidence/junit-verification.log"
  else
    junit_result=FAIL
    echo "Instrumentation Gradle command succeeded but produced no connected JUnit directory." \
      > "$evidence/junit-verification.log"
  fi
fi
printf '%s\n' "$junit_result" > "$evidence/instrumentation-status.txt"

death_result=NOT_RUN
if [[ "$sdk_ok" == true && "$data_isolation_result" == PASS ]]; then
  EXPECTED_API_LEVEL="$api_level" SEREA_SENTINEL_TOKEN="$data_sentinel_token" \
    SEREA_CAMPAIGN_TOKEN="serea-isolated-api${api_level}-${GITHUB_RUN_ID:?GITHUB_RUN_ID required}-${GITHUB_RUN_ATTEMPT:?GITHUB_RUN_ATTEMPT required}" \
    bash "$tool_dir/run_android_process_death_harness.sh" \
    > "$evidence/process-death.log" 2>&1
  death_exit=$?
  cat "$evidence/process-death.log"
  if [[ "$death_exit" -eq 0 ]]; then
    death_result=PASS
  else
    death_result=FAIL
  fi
elif [[ "$sdk_ok" == true ]]; then
  echo "Process Death not run because the disposable-device and demo-sentinel isolation gate did not pass." \
    > "$evidence/process-death.log"
fi
printf '%s\n' "$death_result" > "$evidence/process-death-status.txt"

if [[ "$sdk_ok" == true && "$gradle_result" -eq 0 && "$junit_result" == PASS && "$death_result" == PASS && "$data_isolation_result" == PASS ]]; then
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
