#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=tools/serea_real_ime_avd_guards.sh
source "$ROOT/tools/serea_real_ime_avd_guards.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

campaign_token="$(serea_make_campaign_token 35 40610433596 1 B_realImeCompositionWithoutSynchronizationBoundaries)"
[[ "$campaign_token" == 'serea-api35-40610433596-1-B-realImeCompositionWithoutSynchronizationBoundaries' ]]
[[ "$campaign_token" =~ ^[A-Za-z0-9-]{1,120}$ ]]
if serea_make_campaign_token 35 40610433596 1 'Invalid method!' > /dev/null 2>&1; then
  echo 'FAIL: invalid campaign-token method was accepted' >&2
  exit 1
fi

if ! serea_verify_package_absent_from_listing 'app.serea.android.r25testime' 'package:app.serea.android.other'; then
  echo 'FAIL: an absent Test IME package was not recognized in a valid package listing' >&2
  exit 1
fi
if ! serea_verify_package_absent_from_listing 'app.serea.android.r25testime' $'package:android\npackage:com.google.android.inputmethod.latin'; then
  echo 'FAIL: a valid multi-package inventory was rejected' >&2
  exit 1
fi
if ! serea_verify_package_absent_from_listing 'app.serea.android.r25testime' 'package:app.serea.android.r25testime.child'; then
  echo 'FAIL: a package-name prefix collision was treated as the Test IME package' >&2
  exit 1
fi
if serea_verify_package_absent_from_listing 'app.serea.android.r25testime' ''; then
  echo 'FAIL: an empty package listing was accepted as proof of package absence' >&2
  exit 1
fi
if serea_verify_package_absent_from_listing 'app.serea.android.r25testime' 'package:'; then
  echo 'FAIL: a malformed empty package entry was accepted' >&2
  exit 1
fi
if serea_verify_package_absent_from_listing 'app.serea.android.r25testime' 'error package:garbage'; then
  echo 'FAIL: error text containing package syntax was accepted' >&2
  exit 1
fi
if serea_verify_package_absent_from_listing 'app.serea.android.r25testime' $'package:com.example.valid\nmalformed'; then
  echo 'FAIL: a malformed line in a package inventory was accepted' >&2
  exit 1
fi
if serea_verify_package_absent_from_listing 'app.serea.android.r25testime' 'package:app.serea.android.r25testime'; then
  echo 'FAIL: an installed Test IME package was accepted as absent' >&2
  exit 1
fi
if serea_verify_package_absent_from_listing '' ''; then
  echo 'FAIL: an empty package identity was accepted' >&2
  exit 1
fi

cat > "$tmp/seed.log" <<'EOF'
SEED_OK uid=10078 dataDir=/data/data/app.serea.android.isolated marker=no_backup/serea-instrumentation-token\nprofileVerifierMarkerBeforeUtp=ABSENT
PASS: fresh isolated namespace received a one-use host seed without starting the app.
EOF
if ! serea_verify_isolated_seed_log "$tmp/seed.log"; then
  echo 'FAIL: a valid API 26 run-as seed log was rejected' >&2
  exit 1
fi
cat > "$tmp/seed-api35.log" <<'EOF'
SEED_OK uid=10078 dataDir=/data/user/0/app.serea.android.isolated marker=no_backup/serea-instrumentation-token\nprofileVerifierMarkerBeforeUtp=ABSENT
PASS: fresh isolated namespace received a one-use host seed without starting the app.
EOF
if ! serea_verify_isolated_seed_log "$tmp/seed-api35.log"; then
  echo 'FAIL: a valid API 35 run-as seed log was rejected' >&2
  exit 1
fi
sed '/^PASS:/d' "$tmp/seed.log" > "$tmp/seed-without-pass.log"
if serea_verify_isolated_seed_log "$tmp/seed-without-pass.log"; then
  echo 'FAIL: a seed log without the host PASS marker was accepted' >&2
  exit 1
fi
sed '/^SEED_OK /d' "$tmp/seed.log" > "$tmp/seed-without-device-marker.log"
if serea_verify_isolated_seed_log "$tmp/seed-without-device-marker.log"; then
  echo 'FAIL: a seed log without the run-as SEED_OK marker was accepted' >&2
  exit 1
fi

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
