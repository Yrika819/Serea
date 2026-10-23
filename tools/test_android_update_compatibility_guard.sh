#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
test_root=$(mktemp -d "${TMPDIR:-/tmp}/serea-update-guard-test.XXXXXX")
trap 'rm -rf "$test_root"' EXIT

mock_bin="$test_root/bin"
mkdir -p "$mock_bin"
mock_log="$test_root/adb.log"
mock_logcat="$test_root/logcat.txt"
mock_cert=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
mock_other_cert=BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB

cat > "$mock_bin/adb" <<'MOCK_ADB'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "$MOCK_ADB_LOG"
if [[ "${1:-}" == "-s" ]]; then shift 2; fi

case "${1:-}" in
  devices)
    printf 'List of devices attached\n'
    case "$MOCK_ADB_CASE" in
      physical) printf '0123456789ABCDEF\tdevice product:pixel\n' ;;
      multiple) printf 'emulator-5554\tdevice\nemulator-5556\tdevice\n' ;;
      unauthorized) printf 'emulator-5554\tunauthorized\n' ;;
      fake_emulator|wrong_api|valid) printf 'emulator-5554\tdevice product:sdk\n' ;;
      *) exit 90 ;;
    esac
    ;;
  shell)
    shift
    if [[ "${1:-}" == "getprop" ]]; then
      case "${2:-}" in
        ro.build.version.sdk)
          if [[ "$MOCK_ADB_CASE" == wrong_api ]]; then printf '26\n'; else printf '35\n'; fi
          ;;
        ro.kernel.qemu)
          if [[ "$MOCK_ADB_CASE" == fake_emulator ]]; then printf '0\n'; else printf '1\n'; fi
          ;;
        *) exit 91 ;;
      esac
    elif [[ "${1:-}" == "am" && "${2:-}" == "broadcast" ]]; then
      phase=missing
      token=missing
      while (($#)); do
        if [[ "$1" == "--es" && "${2:-}" == "phase" ]]; then phase="${3:-missing}"; shift 3
        elif [[ "$1" == "--es" && "${2:-}" == "token" ]]; then token="${3:-missing}"; shift 3
        else shift
        fi
      done
      printf 'I/SereaProcessDeath: PASS:%s:%s\n' "$token" "$phase" >> "$MOCK_ADB_LOGCAT"
    elif [[ "${1:-}" == "dumpsys" && "${2:-}" == "package" ]]; then
      installed_version=$(cat "$MOCK_INSTALLED_VERSION_FILE" 2>/dev/null || printf '0')
      printf 'Package [app.serea.android.updatecompat] versionCode=%s minSdk=26\n' "$installed_version"
    fi
    ;;
  uninstall)
    rm -f "$MOCK_INSTALLED_VERSION_FILE"
    printf 'Success\n'
    ;;
  install)
    apk="${@: -1}"
    if grep -q '^signing=wrong$' "$apk"; then
      printf 'Failure [INSTALL_FAILED_UPDATE_INCOMPATIBLE: signatures do not match]\n'
      exit 1
    fi
    sed -n 's/^versionCode=//p' "$apk" > "$MOCK_INSTALLED_VERSION_FILE"
    printf 'Success\n'
    ;;
  logcat)
    shift
    if [[ "${1:-}" == "-c" ]]; then
      : > "$MOCK_ADB_LOGCAT"
    elif [[ "${1:-}" == "-d" ]]; then
      cat "$MOCK_ADB_LOGCAT"
    fi
    ;;
  *) exit 92 ;;
esac
MOCK_ADB
chmod +x "$mock_bin/adb"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

run_rejected() {
  local scenario="$1"
  local output="$test_root/$scenario.out"
  : > "$mock_log"
  : > "$mock_logcat"
  local -a env_args=(
    "PATH=$mock_bin:$PATH"
    "MOCK_ADB_CASE=$scenario"
    "MOCK_ADB_LOG=$mock_log"
    "MOCK_ADB_LOGCAT=$mock_logcat"
    "CI=true"
    "GITHUB_ACTIONS=true"
    "GITHUB_RUN_ID=123456"
    "SEREA_DISPOSABLE_CI_EMULATOR=1"
  )
  if [[ "$scenario" == missing_flag ]]; then
    env_args=(
      "PATH=$mock_bin:$PATH"
      "MOCK_ADB_CASE=valid"
      "MOCK_ADB_LOG=$mock_log"
      "MOCK_ADB_LOGCAT=$mock_logcat"
      "CI=true"
      "GITHUB_ACTIONS=true"
      "GITHUB_RUN_ID=123456"
    )
  elif [[ "$scenario" == serial_mismatch ]]; then
    env_args+=("ANDROID_SERIAL=emulator-5556")
  fi
  if env "${env_args[@]}" bash "$repo_root/tools/test_android_update_compatibility.sh" >"$output" 2>&1; then
    fail "$scenario was accepted"
  fi
  if grep -E '(^| )(install|uninstall)( |$)|logcat -c' "$mock_log" >/dev/null; then
    cat "$mock_log" >&2
    fail "$scenario reached an ADB mutation"
  fi
  if grep -F ' shell ' "$mock_log" | grep -Ev ' shell getprop ro\.(build\.version\.sdk|kernel\.qemu)$' >/dev/null; then
    cat "$mock_log" >&2
    fail "$scenario reached an ADB shell command beyond read-only guard checks"
  fi
  echo "PASS: rejected $scenario with no ADB data mutation"
}

run_rejected physical
run_rejected multiple
run_rejected unauthorized
run_rejected fake_emulator
run_rejected wrong_api
run_rejected serial_mismatch
run_rejected missing_flag

guard_line=$(grep -n 'source .*assert_disposable_android_emulator\.sh' "$repo_root/tools/test_android_update_compatibility.sh" | cut -d: -f1)
temp_setup_line=$(grep -n '^test_root=' "$repo_root/tools/test_android_update_compatibility.sh" | cut -d: -f1)
[[ -n "$guard_line" && -n "$temp_setup_line" && "$guard_line" -lt "$temp_setup_line" ]] || \
  fail 'compatibility test must source the guard before creating test state'

# Build an isolated fixture repository so the allowed-path test creates no
# Gradle outputs or Android state inside the user's checkout.
fixture_repo="$test_root/fixture"
mkdir -p "$fixture_repo/tools" "$fixture_repo/android" "$test_root/android-sdk/build-tools/35.0.0"
cp "$repo_root/tools/assert_disposable_android_emulator.sh" "$fixture_repo/tools/"
cp "$repo_root/tools/test_android_update_compatibility.sh" "$fixture_repo/tools/"
cat > "$fixture_repo/android/gradlew" <<'MOCK_GRADLE'
#!/usr/bin/env bash
set -euo pipefail
version=
isolated=false
for arg in "$@"; do
  case "$arg" in
    -PsereaVersionCode=*) version="${arg#-PsereaVersionCode=}" ;;
    -PsereaUpdateCompatibilityTest=true) isolated=true ;;
  esac
done
[[ "$isolated" == true && ( "$version" == 100 || "$version" == 101 ) ]]
mkdir -p app/build/outputs/apk/debug
signing=primary
if [[ "${SEREA_UPDATE_TEST_KEYSTORE_PATH:-}" == *wrong-signer* ]]; then signing=wrong; fi
printf 'applicationId=app.serea.android.updatecompat\nversionCode=%s\nsigning=%s\n' "$version" "$signing" > app/build/outputs/apk/debug/app-debug.apk
MOCK_GRADLE
chmod +x "$fixture_repo/android/gradlew"

cat > "$mock_bin/openssl" <<'MOCK_OPENSSL'
#!/usr/bin/env bash
printf '%064d\n' 0
MOCK_OPENSSL
cat > "$mock_bin/keytool" <<'MOCK_KEYTOOL'
#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "-genkeypair" ]]; then
  while (($#)); do
    if [[ "$1" == "-keystore" ]]; then mkdir -p "$(dirname "$2")"; : > "$2"; break; fi
    shift
  done
else
  store=
  while (($#)); do
    if [[ "$1" == "-keystore" ]]; then store="$2"; break; fi
    shift
  done
  if [[ "$store" == *wrong-signer* ]]; then
    printf 'SHA256: %s\n' "$MOCK_OTHER_CERT_SHA256"
  else
    printf 'SHA256: %s\n' "$MOCK_CERT_SHA256"
  fi
fi
MOCK_KEYTOOL
chmod +x "$mock_bin/openssl" "$mock_bin/keytool"

cat > "$test_root/android-sdk/build-tools/35.0.0/apksigner" <<'MOCK_APKSIGNER'
#!/usr/bin/env bash
apk="${@: -1}"
if grep -q '^signing=wrong$' "$apk"; then
  printf 'Signer #1 certificate SHA-256 digest: %s\n' "$MOCK_OTHER_CERT_SHA256"
else
  printf 'Signer #1 certificate SHA-256 digest: %s\n' "$MOCK_CERT_SHA256"
fi
MOCK_APKSIGNER
cat > "$test_root/android-sdk/build-tools/35.0.0/aapt" <<'MOCK_AAPT'
#!/usr/bin/env bash
apk="${@: -1}"
application_id=$(sed -n 's/^applicationId=//p' "$apk")
version=$(sed -n 's/^versionCode=//p' "$apk")
printf "package: name='%s' versionCode='%s' versionName='0.1.0-demo'\n" "$application_id" "$version"
MOCK_AAPT
chmod +x "$test_root/android-sdk/build-tools/35.0.0/apksigner" "$test_root/android-sdk/build-tools/35.0.0/aapt"

: > "$mock_log"
: > "$mock_logcat"
mkdir -p "$test_root/runner"
summary="$test_root/summary.md"
if ! env \
  "PATH=$mock_bin:$PATH" \
  "MOCK_ADB_CASE=valid" \
  "MOCK_ADB_LOG=$mock_log" \
  "MOCK_ADB_LOGCAT=$mock_logcat" \
  "MOCK_INSTALLED_VERSION_FILE=$test_root/installed-version.txt" \
  "MOCK_CERT_SHA256=$mock_cert" \
  "MOCK_OTHER_CERT_SHA256=$mock_other_cert" \
  ANDROID_HOME="$test_root/android-sdk" \
  RUNNER_TEMP="$test_root/runner" \
  GITHUB_STEP_SUMMARY="$summary" \
  CI=true GITHUB_ACTIONS=true GITHUB_RUN_ID=123456 SEREA_DISPOSABLE_CI_EMULATOR=1 \
  bash "$fixture_repo/tools/test_android_update_compatibility.sh" > "$test_root/allowed.out" 2>&1; then
  cat "$test_root/allowed.out" >&2
  cat "$mock_log" >&2
  fail "valid disposable emulator did not complete the mocked compatibility test"
fi

grep -F 'Disposable emulator accepted: serial=emulator-5554 api=35 qemu=1' "$test_root/allowed.out" >/dev/null || fail 'valid emulator was not accepted'
grep -F -- '-s emulator-5554 uninstall app.serea.android.updatecompat' "$mock_log" >/dev/null || fail 'isolated package was not the uninstall target'
install_count=$(grep -F -c -- '-s emulator-5554 install -r' "$mock_log")
[[ "$install_count" -eq 3 ]] || fail 'the test must attempt v1, different-key rejection, and same-key v2 installs'
grep -F 'app.serea.android.demo' "$mock_log" >/dev/null && fail 'production demo package appeared in ADB commands'
result_file="$fixture_repo/android/app/build/outputs/androidTest-results/update-compatibility.txt"
grep -F 'application_id=app.serea.android.updatecompat' "$result_file" >/dev/null || fail 'isolated package ID missing from results'
grep -F 'version_code_v1=100' "$result_file" >/dev/null || fail 'v100 evidence missing'
grep -F 'version_code_v2=101' "$result_file" >/dev/null || fail 'v101 evidence missing'
grep -F 'different_key_update_rejected=PASS' "$result_file" >/dev/null || fail 'different-key rejection evidence missing'
grep -F 'room_encrypted_history_settings_keystore_reference=PASS' "$result_file" >/dev/null || fail 'Room/Keystore evidence missing'
grep -F 'process_death_recovery_after_update=PASS' "$result_file" >/dev/null || fail 'Process Death evidence missing'
echo 'PASS: disposable API 35 update flow accepts same-key v101, rejects a different-key APK, and preserves app data'
