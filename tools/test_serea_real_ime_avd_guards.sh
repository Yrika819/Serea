#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=tools/serea_real_ime_avd_guards.sh
source "$ROOT/tools/serea_real_ime_avd_guards.sh"

campaign_token="$(serea_make_campaign_token 35 40610433596 1 B_realImeCompositionWithoutSynchronizationBoundaries)"
[[ "$campaign_token" == 'serea-api35-40610433596-1-B-realImeCompositionWithoutSynchronizationBoundaries' ]]
[[ "$campaign_token" =~ ^[A-Za-z0-9-]{1,120}$ ]]
if serea_make_campaign_token 35 40610433596 1 'Invalid method!' > /dev/null 2>&1; then
  echo 'FAIL: invalid campaign-token method was accepted' >&2
  exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

cat > "$tmp/api26.config.ini" <<'EOF'
image.sysdir.1 = system-images/android-26/google_apis/x86_64/
tag.id = google_apis
abi.type = x86_64
EOF
api26_out="$(serea_verify_active_google_apis_avd 26 \
  'Android/sdk_gphone_x86_64/generic_x86_64:8.0.0/test:userdebug/dev-keys' "$tmp/api26.config.ini")"
grep -Fq 'expected_system_image_package=system-images;android-26;google_apis;x86_64' <<< "$api26_out"
grep -Fq 'fingerprint=Android/sdk_gphone_x86_64/generic_x86_64:' <<< "$api26_out"

cat > "$tmp/api35.config.ini" <<'EOF'
image.sysdir.1 = /opt/android-sdk/system-images/android-35/google_apis/x86_64/
tag.id=google_apis
abi.type=x86_64
EOF
api35_out="$(serea_verify_active_google_apis_avd 35 \
  'google/sdk_gphone64_x86_64/emu64xa:15/test:userdebug/dev-keys' "$tmp/api35.config.ini")"
grep -Fq 'configured_system_image_package=system-images;android-35;google_apis;x86_64' <<< "$api35_out"

if serea_verify_active_google_apis_avd 35 \
  'google/sdk_gphone64_x86_64/emu64xa:15/test:userdebug/dev-keys' "$tmp/api26.config.ini" \
  > "$tmp/api-level-mismatch.out" 2>&1; then
  echo 'FAIL: AVD image for another API level was accepted' >&2
  exit 1
fi
grep -Fq 'avd_image_mismatch' "$tmp/api-level-mismatch.out"

cat > "$tmp/mismatched.config.ini" <<'EOF'
image.sysdir.1 = system-images/android-35/google_apis_playstore/x86_64/
tag.id = google_apis_playstore
abi.type = x86_64
EOF
if serea_verify_active_google_apis_avd 35 \
  'google/sdk_gphone64_x86_64/emu64xa:15/test:userdebug/dev-keys' "$tmp/mismatched.config.ini" \
  > "$tmp/mismatch.out" 2>&1; then
  echo 'FAIL: mismatched active AVD image was accepted' >&2
  exit 1
fi
grep -Fq 'avd_image_mismatch' "$tmp/mismatch.out"

if serea_verify_active_google_apis_avd 35 \
  'google/generic_x86_64/emu64xa:15/test:userdebug/dev-keys' "$tmp/api35.config.ini" \
  > "$tmp/fingerprint.out" 2>&1; then
  echo 'FAIL: broad generic API 35 fingerprint was accepted' >&2
  exit 1
fi

sleep 30 &
cooperative_pid=$!
cooperative_started="$(date +%s)"
serea_stop_logcat_bounded "$cooperative_pid" 2 2 > "$tmp/cooperative-stop.txt"
cooperative_elapsed=$(( $(date +%s) - cooperative_started ))
[[ "$cooperative_elapsed" -lt 4 ]]
! serea_logcat_pid_is_running "$cooperative_pid"
grep -Fq 'logcat_stop=term ' "$tmp/cooperative-stop.txt"

bash -c 'trap "" TERM; while :; do sleep 0.1; done' &
stubborn_pid=$!
stubborn_started="$(date +%s)"
serea_stop_logcat_bounded "$stubborn_pid" 1 2 > "$tmp/stubborn-stop.txt"
stubborn_elapsed=$(( $(date +%s) - stubborn_started ))
[[ "$stubborn_elapsed" -lt 5 ]]
! serea_logcat_pid_is_running "$stubborn_pid"
grep -Fq 'logcat_stop=kill ' "$tmp/stubborn-stop.txt"

echo 'SEREA_REAL_IME_AVD_GUARD_TESTS=PASS'
