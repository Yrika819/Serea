#!/usr/bin/env bash

# Shared fail-closed checks for the disposable real-IME diagnostic workflow.

serea_make_campaign_token() {
  local api="$1" run_id="$2" attempt="$3" method="$4"
  local safe_method token

  if [[ "$api" != 26 && "$api" != 35 ]] || \
     [[ ! "$run_id" =~ ^[0-9]+$ ]] || \
     [[ ! "$attempt" =~ ^[0-9]+$ ]] || \
     [[ ! "$method" =~ ^[A-Za-z][A-Za-z0-9_]{0,119}$ ]]; then
    echo 'invalid_campaign_token_input' >&2
    return 1
  fi

  # The instrumentation method identifier may contain underscores, while the
  # seed helper deliberately accepts only alphanumerics and hyphens.
  safe_method="${method//_/-}"
  token="serea-api${api}-${run_id}-${attempt}-${safe_method}"
  if [[ ${#token} -gt 120 || ! "$token" =~ ^[A-Za-z0-9-]+$ ]]; then
    echo 'campaign_token_outside_seed_contract' >&2
    return 1
  fi
  printf '%s' "$token"
}

serea_verify_isolated_seed_log() {
  local log_file="$1"
  [[ -r "$log_file" ]] || return 1
  grep -Eq '^SEED_OK uid=[0-9]+ dataDir=/data/(user/0|data)/app\.serea\.android\.isolated marker=no_backup/serea-instrumentation-token(\\nprofileVerifierMarkerBeforeUtp=ABSENT)?$' \
    "$log_file" &&
    grep -Fq 'PASS: fresh isolated namespace received a one-use host seed without starting the app.' "$log_file"
}

serea_verify_active_google_apis_avd() {
  local api="$1" fingerprint="$2" config="$3"
  local expected_package image_sysdir actual_package tag_id abi

  if [[ "$api" != 26 && "$api" != 35 ]]; then
    echo "unsupported_api=$api" >&2
    return 1
  fi
  if [[ ! -r "$config" ]]; then
    echo "avd_config_missing=$config" >&2
    return 1
  fi

  expected_package="system-images;android-${api};google_apis;x86_64"
  image_sysdir="$(awk -F= '$1 ~ /^[[:space:]]*image\.sysdir\.1[[:space:]]*$/ { sub(/^[[:space:]]+/, "", $2); sub(/[[:space:]]+$/, "", $2); print $2; exit }' "$config")"
  tag_id="$(awk -F= '$1 ~ /^[[:space:]]*tag\.id[[:space:]]*$/ { sub(/^[[:space:]]+/, "", $2); sub(/[[:space:]]+$/, "", $2); print $2; exit }' "$config")"
  abi="$(awk -F= '$1 ~ /^[[:space:]]*abi\.type[[:space:]]*$/ { sub(/^[[:space:]]+/, "", $2); sub(/[[:space:]]+$/, "", $2); print $2; exit }' "$config")"
  image_sysdir="${image_sysdir%/}"
  local image_relative="${image_sysdir#*system-images/}"
  if [[ "$image_relative" == "$image_sysdir" || "$image_relative" == *".."* ]]; then
    echo "invalid_avd_image_sysdir=$image_sysdir" >&2
    return 1
  fi
  actual_package="system-images;${image_relative//\//;}"
  if [[ "$actual_package" != "$expected_package" || "$tag_id" != google_apis || "$abi" != x86_64 ]]; then
    printf 'avd_image_mismatch expected_package=%s actual_package=%s tag_id=%s abi=%s config=%s\n' \
      "$expected_package" "$actual_package" "$tag_id" "$abi" "$config" >&2
    return 1
  fi

  # Check a concrete emulator device family per API. The active AVD config above
  # is the authority for the image tag; do not infer it from a generic fingerprint.
  case "$api" in
    26)
      case "$fingerprint" in
        Android/sdk_gphone_x86_64/generic_x86_64:*|google/sdk_gphone_x86_64/generic_x86_64:*) ;;
        *) echo "unexpected_api26_fingerprint=$fingerprint" >&2; return 1 ;;
      esac
      ;;
    35)
      case "$fingerprint" in
        */sdk_gphone64_x86_64/emu64xa:*) ;;
        *) echo "unexpected_api35_fingerprint=$fingerprint" >&2; return 1 ;;
      esac
      ;;
  esac

  printf 'expected_system_image_package=%s\nconfigured_system_image_path=%s\nconfigured_system_image_package=%s\ntag_id=%s\nabi_type=%s\nfingerprint=%s\n' \
    "$expected_package" "$image_sysdir" "$actual_package" "$tag_id" "$abi" "$fingerprint"
}

serea_logcat_pid_is_running() {
  local pid="$1" stat_line tail state
  [[ -r "/proc/$pid/stat" ]] || return 1
  stat_line="$(cat "/proc/$pid/stat" 2>/dev/null)" || return 1
  # Strip pid and parenthesized command name; the next field is process state.
  tail="${stat_line##*) }"
  state="${tail%% *}"
  [[ -n "$state" && "$state" != Z && "$state" != X ]]
}

serea_wait_logcat_exit_bounded() {
  local pid="$1" seconds="$2" ticks i
  ticks=$((seconds * 10))
  for ((i = 0; i < ticks; i++)); do
    serea_logcat_pid_is_running "$pid" || return 0
    sleep 0.1
  done
  ! serea_logcat_pid_is_running "$pid"
}

serea_stop_logcat_bounded() {
  local pid="$1" term_seconds="${2:-2}" kill_seconds="${3:-2}"
  if ! [[ "$pid" =~ ^[0-9]+$ ]] || ((pid <= 1)); then
    echo "logcat_stop_invalid_pid=$pid"
    return 1
  fi
  if ! serea_logcat_pid_is_running "$pid"; then
    wait "$pid" 2>/dev/null || true
    echo "logcat_stop=already-exited pid=$pid"
    return 0
  fi

  kill -TERM "$pid" 2>/dev/null || true
  if serea_wait_logcat_exit_bounded "$pid" "$term_seconds"; then
    wait "$pid" 2>/dev/null || true
    echo "logcat_stop=term pid=$pid term_deadline_seconds=$term_seconds"
    return 0
  fi

  echo "logcat_stop=term-timeout action=kill pid=$pid term_deadline_seconds=$term_seconds"
  kill -KILL "$pid" 2>/dev/null || true
  if serea_wait_logcat_exit_bounded "$pid" "$kill_seconds"; then
    wait "$pid" 2>/dev/null || true
    echo "logcat_stop=kill pid=$pid kill_deadline_seconds=$kill_seconds"
    return 0
  fi

  # Never block on an unbounded shell wait if the host reports an uninterruptible
  # process. The caller marks cleanup failed and preserves the evidence.
  echo "logcat_stop=failed-to-exit pid=$pid kill_deadline_seconds=$kill_seconds" >&2
  return 1
}
