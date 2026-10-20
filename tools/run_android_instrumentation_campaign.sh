#!/usr/bin/env bash

# android-emulator-runner invokes each line of its `script` input in a fresh
# shell. Keep this stateful campaign in one Bash process.
set +e

api_level="${1:-}"
if [[ "$api_level" != 26 && "$api_level" != 35 ]]; then
  echo "Usage: $0 {26|35}" >&2
  exit 2
fi

root='app/build/outputs/androidTest-results'
evidence="$root/api$api_level"
mkdir -p "$evidence" || exit 1
printf 'NOT_RUN\n' > "$evidence/instrumentation-status.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/process-death-status.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/device-api-level.txt" || exit 1
printf 'NOT_RUN\n' > "$evidence/api-result.txt" || exit 1

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
  "$evidence/process-death.log" || \
  [[ -e "$evidence/device-api-error.txt" || -e "$evidence/junit-verification.log" || -e "$evidence/process-death.log" ]]; then
  cleanup_failed
fi

adb devices -l
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
if [[ "$sdk_ok" == true ]]; then
  ./gradlew --no-daemon connectedDebugAndroidTest
  gradle_result=$?
  if [[ -d "$root/connected" ]]; then
    mv "$root/connected" "$evidence/junit"
  fi

  if [[ -d "$evidence/junit" ]]; then
    python3 ../tools/verify_android_instrumentation.py "$evidence/junit" \
      --source-root app/src/androidTest/java --minimum 71 \
      --require-class app.serea.android.EncryptedHistoryTest=16 \
      --require-class app.serea.android.ChatScreenTest=16 \
      --require-class app.serea.android.ChatHttpE2ETest=4 \
      --require-class app.serea.android.UsageLedgerDatabaseTest=12 \
      --require-class app.serea.android.tasks.TasksDatabaseTest=6 \
      --require-class app.serea.android.tasks.TaskScreenTest=5 \
      --require-class app.serea.android.ChatTasksDatabaseIntegrationTest=1 \
      --require-class app.serea.android.ChatHistoryUxTest=4 \
      --require-class app.serea.android.tasks.TaskDateTimePickerTest=6 \
      --require-class app.serea.android.assistant.taskproposal.TaskProposalReviewScreenTest=2 \
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
if [[ "$sdk_ok" == true ]]; then
  EXPECTED_API_LEVEL="$api_level" bash ../tools/run_android_process_death_harness.sh \
    > "$evidence/process-death.log" 2>&1
  death_exit=$?
  cat "$evidence/process-death.log"
  if [[ "$death_exit" -eq 0 ]]; then
    death_result=PASS
  else
    death_result=FAIL
  fi
fi
printf '%s\n' "$death_result" > "$evidence/process-death-status.txt"

if [[ "$sdk_ok" == true && "$gradle_result" -eq 0 && "$junit_result" == PASS && "$death_result" == PASS ]]; then
  result=PASS
else
  result=FAIL
fi
printf '%s\n' "$result" > "$evidence/api-result.txt"
echo "### Android API $api_level: instrumentation=$junit_result process-death=$death_result device-api=$sdk" \
  >> "$GITHUB_STEP_SUMMARY"
if [[ "$result" != PASS ]]; then
  exit 1
fi
