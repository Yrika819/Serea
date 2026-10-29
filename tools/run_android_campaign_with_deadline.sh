#!/usr/bin/env bash
set -euo pipefail

api="${1:-}"
case "$api" in
  26)
    campaign=(bash ../tools/run_android_instrumentation_campaign.sh 26)
    deadline_failure='FAIL: shared API 26/API 35 campaign deadline leaves no safe API 26 execution budget.'
    ;;
  35)
    campaign=(bash ../tools/run_android_api35_campaign.sh)
    deadline_failure='FAIL: shared API 26/API 35 campaign deadline leaves no safe API 35 execution budget.'
    ;;
  *)
    echo 'Usage: run_android_campaign_with_deadline.sh 26|35' >&2
    exit 2
    ;;
esac

deadline="${SEREA_ANDROID_CAMPAIGN_DEADLINE_EPOCH:-}"
if [[ ! "$deadline" =~ ^[0-9]+$ ]]; then
  echo 'FAIL: shared Android campaign deadline is missing or invalid.' >&2
  exit 2
fi

now="$(date +%s)"
remaining=$((deadline - now - 180))
if (( remaining <= 0 )); then
  echo "$deadline_failure" >&2
  exit 1
fi

# The emulator action invokes script through /bin/sh. Keep Bash syntax here,
# outside its inline script, and preserve timeout's exact exit status.
exec timeout --signal=INT --kill-after=60s "${remaining}s" "${campaign[@]}"
