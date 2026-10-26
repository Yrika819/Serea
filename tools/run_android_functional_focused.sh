#!/usr/bin/env bash
set -euo pipefail

api_level="${1:-}"
case "$api_level" in
  26|35) ;;
  *) echo "Usage: $0 {26|35}" >&2; exit 2 ;;
esac

system_image_target="${SEREA_SYSTEM_IMAGE_TARGET:-}"
emulator_profile="${SEREA_EMULATOR_PROFILE:-unknown}"
emulator_arch="${SEREA_EMULATOR_ARCH:-unknown}"
emulator_gpu_backend="${SEREA_EMULATOR_GPU_BACKEND:-unknown}"
if [[ "$system_image_target" != "google_apis" ]]; then
  echo 'Focused Android UI verification requires the declared Google APIs system image.' >&2
  exit 2
fi

evidence="app/build/outputs/androidTest-results/functional-api${api_level}"
filter='app.serea.android.SereaAppShellTest#chatGmailBackShowsOnlyFictionalMailboxData,app.serea.android.SereaAppShellTest#gmailSearchAndMessageDetailUseTheFakeProviderFixtures,app.serea.android.SereaAppShellTest#chatCalendarBackShowsFictionalAgendaAndDetail,app.serea.android.SereaAppShellTest#taskEditorDraftSurvivesActivityRecreation,app.serea.android.SereaAppShellTest#fontScaleTwoKeepsTaskProposalReviewAndFooterReachable,app.serea.android.ChatScreenTest#demoLabelAndConversationTitleAreVisible,app.serea.android.ChatScreenTest#completedDemoConversationSurvivesActivityRecreation,app.serea.android.tasks.TaskScreenTest,app.serea.android.tasks.TaskDateTimePickerTest,app.serea.android.tasks.TasksDatabaseTest'

if [[ -e "$evidence" || -e app/build/outputs/androidTest-results/connected ]]; then
  echo 'Refusing to reuse stale Focused evidence or connected instrumentation results.' >&2
  exit 2
fi
mkdir -p "$evidence"
initial_default_ime=''
gradle_status=1
verify_status=1

{
  echo "source_sha=$(git rev-parse HEAD)"
  echo "api_level=$api_level"
  echo "system_image_target=$system_image_target"
  echo "emulator_profile=$emulator_profile"
  echo "emulator_arch=$emulator_arch"
  echo "emulator_gpu_backend=$emulator_gpu_backend"
  if [[ -n "${ANDROID_HOME:-}" && -x "$ANDROID_HOME/emulator/emulator" ]]; then
    echo "emulator_version=$("$ANDROID_HOME/emulator/emulator" -version 2>&1 | head -n 1)"
    echo "emulator_accel_check=$("$ANDROID_HOME/emulator/emulator" -accel-check 2>&1 | tr '\n' ';')"
  else
    echo 'emulator_version=MISSING'
    echo 'emulator_accel_check=MISSING'
  fi
  echo "host_arch=$(uname -m)"
  echo "host_cpu_count=$(nproc)"
  echo "guest_uptime=$(adb shell cat /proc/uptime | tr -d '\r')"
  echo "device_sdk=$(adb shell getprop ro.build.version.sdk | tr -d '\r')"
  echo "device_abi=$(adb shell getprop ro.product.cpu.abi | tr -d '\r')"
  echo "device_fingerprint=$(adb shell getprop ro.build.fingerprint | tr -d '\r')"
  echo "device_hardware=$(adb shell getprop ro.hardware | tr -d '\r')"
  echo "device_qemu=$(adb shell getprop ro.kernel.qemu | tr -d '\r')"
  echo "boot_completed=$(adb shell getprop sys.boot_completed | tr -d '\r')"
  adb shell cat /proc/meminfo | tr -d '\r' | sed -n '1,5{s/^/guest_/;p;}'
  echo "default_ime=$(adb shell settings get secure default_input_method | tr -d '\r')"
  echo "display_size=$(adb shell wm size | tr -d '\r')"
  echo "display_density=$(adb shell wm density | tr -d '\r')"
  echo "run_id=${GITHUB_RUN_ID:-local}"
  echo "run_attempt=${GITHUB_RUN_ATTEMPT:-1}"
  echo "test_filter=$filter"
} > "$evidence/environment.txt"

host_diagnostics="${RUNNER_TEMP:-}/serea-functional-api${api_level}-host.txt"
if [[ -n "${RUNNER_TEMP:-}" && -s "$host_diagnostics" ]]; then
  cp "$host_diagnostics" "$evidence/host-diagnostics.txt"
else
  printf 'MISSING: runner host/KVM preflight diagnostics were not available.\n' > "$evidence/host-diagnostics.txt"
fi
{
  echo "instrumentation_environment_observed_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo 'runner_cpu_pressure_at_emulator_ready:'
  cat /proc/pressure/cpu
  echo 'runner_memory_at_emulator_ready:'
  free -h
} >> "$evidence/host-diagnostics.txt"

capture() {
  adb logcat -d -b all > "$evidence/logcat.txt" 2>&1 || true
  adb shell dumpsys input_method > "$evidence/input-method.txt" 2>&1 || true
  adb shell ime list -s > "$evidence/available-ime-services.txt" 2>&1 || true
  adb shell settings get secure default_input_method > "$evidence/default-ime-after.txt" 2>&1 || true
  {
    echo "instrumentation_finished_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo 'runner_cpu_pressure_after_instrumentation:'
    cat /proc/pressure/cpu
    echo 'runner_memory_after_instrumentation:'
    free -h
  } >> "$evidence/host-diagnostics.txt"
}
trap capture EXIT

export EXPECTED_API_LEVEL="$api_level"
export SEREA_EXPECTED_ANDROID_API="$api_level"
source ../tools/assert_disposable_android_emulator.sh
source ../tools/wait_for_android_default_ime.sh
if wait_for_android_default_ime; then
  initial_default_ime="$SEREA_ANDROID_DEFAULT_IME"
  printf 'default_ime=%s\nreadiness_attempts=%s\n' \
    "$initial_default_ime" "$SEREA_ANDROID_DEFAULT_IME_ATTEMPTS" >> "$evidence/environment.txt"
else
  initial_default_ime="$SEREA_ANDROID_DEFAULT_IME"
  printf 'default_ime_status=NOT_READY\nobserved_default_ime=%s\nreadiness_attempts=%s\n' \
    "$initial_default_ime" "$SEREA_ANDROID_DEFAULT_IME_ATTEMPTS" > "$evidence/default-ime-readiness.txt"
  adb shell ime list -s >> "$evidence/default-ime-readiness.txt" 2>&1 || true
  adb shell dumpsys input_method >> "$evidence/default-ime-readiness.txt" 2>&1 || true
  echo 'Could not capture the disposable emulator default IME after 30 readiness checks.' >&2
  exit 1
fi

if [[ -n "$(adb shell pm path app.serea.android.isolated 2>/dev/null | tr -d '\r')" || \
      -n "$(adb shell pm path app.serea.android.isolated.test 2>/dev/null | tr -d '\r')" ]]; then
  echo 'Refusing an emulator with a preinstalled isolated target or test package.' >&2
  exit 1
fi

if ! ./gradlew --no-daemon :app:assembleIsolated > "$evidence/prepare-gradle.log" 2>&1; then
  cat "$evidence/prepare-gradle.log"
  exit 1
fi
target_apk='app/build/outputs/apk/isolated/app-isolated.apk'
if [[ ! -s "$target_apk" ]]; then
  echo 'Isolated target APK was not produced.' >&2
  exit 1
fi
sha256sum "$target_apk" > "$evidence/target-apk-sha256.txt"
if ! adb install "$target_apk" > "$evidence/target-install.log" 2>&1; then
  cat "$evidence/target-install.log"
  exit 1
fi
target_path="$(adb shell pm path app.serea.android.isolated | tr -d '\r' | sed -n 's/^package://p')"
target_installed_sha="$(adb shell sha256sum "$target_path" | awk 'NR == 1 { print $1 }')"
target_expected_sha="$(awk '{ print $1 }' "$evidence/target-apk-sha256.txt")"
if [[ -z "$target_path" || "$target_path" == *$'\n'* || "$target_installed_sha" != "$target_expected_sha" ]]; then
  echo 'Installed isolated target APK did not match the freshly built artifact.' >&2
  exit 1
fi
printf 'installed_path=%s\ninstalled_sha256=%s\nexpected_sha256=%s\n' \
  "$target_path" "$target_installed_sha" "$target_expected_sha" > "$evidence/installed-target-apk.txt"

campaign_token="serea-functional-api${api_level}-${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-1}"
if ! EXPECTED_API_LEVEL="$api_level" bash ../tools/prepare_isolated_instrumentation_target.sh "$campaign_token" \
  > "$evidence/campaign-seed.log" 2>&1; then
  cat "$evidence/campaign-seed.log"
  exit 1
fi
echo 'campaign_seed=PASS' >> "$evidence/environment.txt"

if ./gradlew --no-daemon \
  -Pandroid.testInstrumentationRunnerArguments.sereaCampaignToken="$campaign_token" \
  -Pandroid.testInstrumentationRunnerArguments.listener=app.serea.android.SereaInstrumentationLifecycleListener \
  -Pandroid.testInstrumentationRunnerArguments.class="$filter" \
  connectedIsolatedAndroidTest > "$evidence/gradle.log" 2>&1; then
  gradle_status=0
else
  gradle_status=$?
fi
printf '%s\n' "$gradle_status" > "$evidence/gradle-exit-code.txt"

if [ -d app/build/outputs/androidTest-results/connected ]; then
  cp -R app/build/outputs/androidTest-results/connected "$evidence/junit"
fi
for apk in \
  app/build/outputs/apk/isolated/app-isolated.apk \
  app/build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk; do
  if [[ -s "$apk" ]]; then sha256sum "$apk" >> "$evidence/apk-sha256.txt"; fi
done

if [ -d "$evidence/junit" ]; then
  if python3 ../tools/verify_android_instrumentation.py "$evidence/junit" \
    --minimum 26 --exact-count 26 \
    --require-test app.serea.android.SereaAppShellTest#chatGmailBackShowsOnlyFictionalMailboxData \
    --require-test app.serea.android.SereaAppShellTest#gmailSearchAndMessageDetailUseTheFakeProviderFixtures \
    --require-test app.serea.android.SereaAppShellTest#chatCalendarBackShowsFictionalAgendaAndDetail \
    --require-test app.serea.android.SereaAppShellTest#taskEditorDraftSurvivesActivityRecreation \
    --require-test app.serea.android.SereaAppShellTest#fontScaleTwoKeepsTaskProposalReviewAndFooterReachable \
    --require-test app.serea.android.ChatScreenTest#demoLabelAndConversationTitleAreVisible \
    --require-test app.serea.android.ChatScreenTest#completedDemoConversationSurvivesActivityRecreation \
    --require-test app.serea.android.tasks.TaskScreenTest#emptyCreateEditCompleteAndReopenAreReachable \
    --require-test app.serea.android.tasks.TaskScreenTest#invalidTitleAndDateAreShownWithoutRepositoryWrite \
    --require-test app.serea.android.tasks.TaskScreenTest#deleteCancelPreservesTaskAndConfirmRemovesIt \
    --require-test app.serea.android.tasks.TaskScreenTest#deletingFrozenTaskIdCannotDeleteAnotherTask \
    --require-test app.serea.android.tasks.TaskScreenTest#rowActionsAnnounceTheirTargetTask \
    --require-test app.serea.android.tasks.TaskScreenTest#lateSaveFromClosedEditorPreservesTheNewEditorsUnsavedDraft \
    --require-test app.serea.android.tasks.TaskDateTimePickerTest#datePickerConfirmSavesDateOnlyTaskAndEditorCanScroll \
    --require-test app.serea.android.tasks.TaskDateTimePickerTest#timePickerConfirmSavesTwentyFourHourTimeWithIanaZone \
    --require-test app.serea.android.tasks.TaskDateTimePickerTest#editingExistingTaskUpdatesExactDraftAndExpectedRevision \
    --require-test app.serea.android.tasks.TaskDateTimePickerTest#invalidIanaZoneIsReportedAndDoesNotCallRepository \
    --require-test app.serea.android.tasks.TaskDateTimePickerTest#daylightSavingGapAndOverlapAreReportedWithoutRepositoryCalls \
    --require-test app.serea.android.tasks.TaskDateTimePickerTest#timeWithoutDateShowsJapaneseDateErrorAndDoesNotSave \
    --require-test app.serea.android.tasks.TasksDatabaseTest#roomCreatesDatabaseAndEncryptedTaskSurvivesCloseReopen \
    --require-test app.serea.android.tasks.TasksDatabaseTest#ciphertextTamperingAndCiphertextSwapAreRejected \
    --require-test app.serea.android.tasks.TasksDatabaseTest#databaseAndKeyAliasesAreSeparateAndKeyLossPreservesCiphertext \
    --require-test app.serea.android.tasks.TasksDatabaseTest#concurrentUpdatesUseRevisionCasAndDeleteDoesNotResurrect \
    --require-test app.serea.android.tasks.TasksDatabaseTest#proposalReceiptIsUniqueAndDuplicateConcurrentCreateRollsBack \
    --require-test app.serea.android.tasks.TasksDatabaseTest#approvedProposalMutationsAndSingleUseReceiptsSurviveDatabaseReopen \
    --require-test app.serea.android.tasks.TasksDatabaseTest#completedStateAndDeletePersistAcrossDatabaseReopen \
    > "$evidence/verification.log" 2>&1; then
    verify_status=0
  else
    verify_status=$?
  fi
else
  verify_status=1
  echo 'Instrumentation JUnit output directory was not created.' > "$evidence/verification.log"
fi

printf '%s\n' "$verify_status" > "$evidence/verification-exit-code.txt"
after_default_ime="$(adb shell settings get secure default_input_method | tr -d '\r')"
printf 'initial=%s\nafter=%s\n' "$initial_default_ime" "$after_default_ime" > "$evidence/default-ime-verified.txt"
if [[ "$after_default_ime" != "$initial_default_ime" ]]; then
  echo 'Disposable emulator IME selection changed during the Focused run.' >&2
  exit 1
fi
test "$gradle_status" -eq 0
test "$verify_status" -eq 0
