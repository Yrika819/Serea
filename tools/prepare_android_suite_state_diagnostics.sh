#!/usr/bin/env bash
set -euo pipefail

api_level="${1:-}"
if [[ "$api_level" != 26 && "$api_level" != 35 ]]; then
  echo "Usage: $0 {26|35}" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_sha='b1e7a7359045d716e9f903d85c1291dff8453520'
source_repo="$GITHUB_WORKSPACE/serea-pr100"
evidence_root="$GITHUB_WORKSPACE/root-cause/suite-state/api$api_level"
mkdir -p "$evidence_root"

if ! git -C "$repo_root" cat-file -e "$source_sha^{commit}"; then
  git -C "$repo_root" fetch --no-tags origin "$source_sha"
fi
if [[ -e "$source_repo" ]]; then
  echo "Refusing to replace existing source worktree: $source_repo" >&2
  exit 1
fi
git -C "$repo_root" worktree add --detach "$source_repo" "$source_sha"

for path in \
  android/app/src/androidTest/java/app/serea/android/ChatScreenTest.kt \
  android/app/src/androidTest/java/app/serea/android/SereaAppShellTest.kt \
  android/app/src/main/java/app/serea/android/SereaAppShell.kt; do
  install -D "$repo_root/$path" "$source_repo/$path"
done

expected=$'android/app/src/androidTest/java/app/serea/android/ChatScreenTest.kt\nandroid/app/src/androidTest/java/app/serea/android/SereaAppShellTest.kt\nandroid/app/src/main/java/app/serea/android/SereaAppShell.kt'
actual="$(git -C "$source_repo" diff --name-only)"
if [[ "$actual" != "$expected" ]]; then
  echo "Unexpected files differ from the fixed source tree." >&2
  printf 'expected:\n%s\nactual:\n%s\n' "$expected" "$actual" >&2
  exit 1
fi
production_actual="$(git -C "$source_repo" diff --name-only -- android/app/src/main)"
production_expected='android/app/src/main/java/app/serea/android/SereaAppShell.kt'
if [[ "$production_actual" != "$production_expected" ]]; then
  echo 'The only allowed product candidate overlay is SereaAppShell.kt.' >&2
  exit 1
fi
if [[ "$(git -C "$source_repo" rev-parse HEAD)" != "$source_sha" ]]; then
  echo 'Test subject SHA mismatch.' >&2
  exit 1
fi

required_count="$(awk 'NF { count++ } END { print count + 0 }' "$source_repo/tools/android-instrumentation-required-tests.txt")"
additional_count="$(awk 'NF { count++ } END { print count + 0 }' "$source_repo/tools/android-instrumentation-additional-tests.txt")"
total_count=$((required_count + additional_count))
if [[ "$required_count" != 103 || "$additional_count" != 2 || "$total_count" != 105 ]]; then
  echo "Expected 103 Required + 2 Additional = 105 tests; found $required_count + $additional_count = $total_count." >&2
  exit 1
fi

{
  printf 'api=%s\n' "$api_level"
  printf 'test_subject_sha=%s\n' "$(git -C "$source_repo" rev-parse HEAD)"
  printf 'diagnostic_checkout_sha=%s\n' "$(git -C "$repo_root" rev-parse HEAD)"
  printf 'base_app_main_tree_sha=%s\n' "$(git -C "$source_repo" rev-parse "$source_sha:android/app/src/main")"
  printf 'diagnostic_product_overlay_sha256='; sha256sum \
    "$source_repo/android/app/src/main/java/app/serea/android/SereaAppShell.kt"
  printf 'diagnostic_product_overlay_paths:\n%s\n' "$production_actual"
  printf 'required_test_count=%s\nadditional_test_count=%s\nfull_suite_test_count=%s\n' \
    "$required_count" "$additional_count" "$total_count"
  printf 'diagnostic_test_overlay_sha256='; sha256sum \
    "$source_repo/android/app/src/androidTest/java/app/serea/android/ChatScreenTest.kt" \
    "$source_repo/android/app/src/androidTest/java/app/serea/android/SereaAppShellTest.kt"
  printf 'changed_paths:\n%s\n' "$actual"
  printf 'build_started_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)"
} > "$evidence_root/source-manifest.txt"

set +e
(
  cd "$source_repo/android" || exit 1
  ./gradlew --no-daemon :app:assembleIsolated :app:assembleIsolatedAndroidTest
) > "$evidence_root/build.log" 2>&1
build_status=$?
set -e
printf '%s\n' "$build_status" > "$evidence_root/build-exit-code.txt"
printf 'build_finished_utc=%s\nbuild_exit_code=%s\n' \
  "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" "$build_status" >> "$evidence_root/source-manifest.txt"
if [[ "$build_status" -ne 0 ]]; then
  echo "Diagnostic instrumentation build failed; see $evidence_root/build.log" >&2
  exit "$build_status"
fi

apk_root="$source_repo/android/app/build/outputs/apk"
target_apk="$apk_root/isolated/app-isolated.apk"
if [[ ! -s "$target_apk" ]]; then
  echo "Isolated target APK not found: $target_apk" >&2
  exit 1
fi
find "$apk_root" -type f -name '*.apk' -print0 | sort -z | xargs -0 sha256sum > "$evidence_root/apk-sha256.txt"
if [[ "$(wc -l < "$evidence_root/apk-sha256.txt" | tr -d ' ')" -lt 2 ]]; then
  echo 'Expected both the isolated target APK and instrumentation APK.' >&2
  exit 1
fi
printf 'target_apk=%s\n' "$target_apk" >> "$evidence_root/source-manifest.txt"
cat "$evidence_root/apk-sha256.txt" >> "$evidence_root/source-manifest.txt"
echo "Prepared one isolated APK set from subject $source_sha for API $api_level."
