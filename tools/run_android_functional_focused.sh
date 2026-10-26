#!/usr/bin/env bash
set -euo pipefail

api_level="${1:-}"
case "$api_level" in
  26|35) ;;
  *) echo "Usage: $0 {26|35}" >&2; exit 2 ;;
esac

evidence="app/build/outputs/androidTest-results/functional-api${api_level}"
mkdir -p "$evidence"
filter='app.serea.android.SereaAppShellTest#chatGmailBackShowsOnlyFictionalMailboxData,app.serea.android.SereaAppShellTest#gmailSearchAndMessageDetailUseTheFakeProviderFixtures,app.serea.android.SereaAppShellTest#chatCalendarBackShowsFictionalAgendaAndDetail,app.serea.android.SereaAppShellTest#taskEditorDraftSurvivesActivityRecreation,app.serea.android.SereaAppShellTest#fontScaleTwoKeepsTaskProposalReviewAndFooterReachable,app.serea.android.ChatScreenTest#demoLabelAndConversationTitleAreVisible,app.serea.android.ChatScreenTest#completedDemoConversationSurvivesActivityRecreation,app.serea.android.tasks.TaskScreenTest,app.serea.android.tasks.TaskDateTimePickerTest,app.serea.android.tasks.TasksDatabaseTest'

{
  echo "source_sha=$(git rev-parse HEAD)"
  echo "api_level=$api_level"
  echo "system_image_target=default"
  echo "device_sdk=$(adb shell getprop ro.build.version.sdk | tr -d '\r')"
  echo "device_abi=$(adb shell getprop ro.product.cpu.abi | tr -d '\r')"
  echo "device_fingerprint=$(adb shell getprop ro.build.fingerprint | tr -d '\r')"
  echo "boot_completed=$(adb shell getprop sys.boot_completed | tr -d '\r')"
  adb shell cat /proc/meminfo | tr -d '\r' | sed -n '1,5{s/^/guest_/;p;}'
  echo "default_ime=$(adb shell settings get secure default_input_method | tr -d '\r')"
  echo "test_filter=$filter"
} > "$evidence/environment.txt"

capture() {
  adb logcat -d -b all > "$evidence/logcat.txt" 2>&1 || true
  adb shell dumpsys input_method > "$evidence/input-method.txt" 2>&1 || true
  adb shell settings get secure default_input_method > "$evidence/default-ime-after.txt" 2>&1 || true
}
trap capture EXIT

if ./gradlew --no-daemon \
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
test "$gradle_status" -eq 0
test "$verify_status" -eq 0
