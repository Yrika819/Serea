#!/usr/bin/env bash
set -uo pipefail

toolchain="${1:?Rust toolchain required}"
focused_test="${2:?focused test required}"
platform="${3:?platform label required}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
evidence="$root/root-cause/native"
mkdir -p "$evidence"
failures=0

{
  echo "platform=$platform"
  echo "toolchain=$toolchain"
  echo "head_sha=$(git -C "$root" rev-parse HEAD)"
  echo "base_sha=$(git -C "$root" rev-parse HEAD^)"
  echo "started_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  rustc "+$toolchain" -vV
  cargo "+$toolchain" --version
} > "$evidence/environment.txt" 2>&1

run_case() {
  local label="$1"
  shift
  local output="$evidence/$label.log"
  echo "stage=$label started_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ) command=cargo +$toolchain $*" | tee "$evidence/$label.meta.txt"
  cargo "+$toolchain" "$@" > "$output" 2>&1
  local status=$?
  printf 'exit_code=%s\nfinished_utc=%s\n' "$status" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$evidence/$label.meta.txt"
  rg -n 'SEREA_ROOT_CAUSE|test result:|FAILED|panicked at|synthetic_host_error_detail|Host .* scenario failed|Process completed' "$output" \
    | tail -160 || true
  echo "stage=$label exit_code=$status"
  if [[ "$status" -ne 0 ]]; then failures=1; fi
}

cargo "+$toolchain" fetch --locked > "$evidence/fetch.log" 2>&1
fetch_status=$?
printf 'fetch_exit_code=%s\n' "$fetch_status" >> "$evidence/environment.txt"
if [[ "$fetch_status" -ne 0 ]]; then
  failures=1
else
  for repetition in 1 2 3; do
    run_case "focused-$repetition" test --locked -p serea-host --features synthetic-test-hooks \
      --test host_lifecycle "$focused_test" -- --exact --nocapture --test-threads=1
  done
  run_case "host-lifecycle-suite" test --locked -p serea-host --features synthetic-test-hooks \
    --test host_lifecycle -- --nocapture
fi

printf 'completed_utc=%s\nfailures_observed=%s\n' \
  "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$failures" > "$evidence/campaign-summary.txt"
exit "$failures"
