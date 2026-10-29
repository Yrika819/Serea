#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root/android"
bash "$repo_root/tools/run_android_instrumentation_campaign.sh" 35
campaign_deadline_epoch="${SEREA_ANDROID_CAMPAIGN_DEADLINE_EPOCH:-$(( $(date +%s) + 150 * 60 ))}"
remaining=$((campaign_deadline_epoch - $(date +%s) - 90))
if (( remaining <= 0 )); then
  echo 'FAIL: shared API 26/API 35 deadline leaves no safe update-compatibility budget.' >&2
  exit 1
fi
if (( remaining > 360 )); then remaining=360; fi
timeout --signal=INT --kill-after=60s "${remaining}s" \
  bash "$repo_root/tools/test_android_update_compatibility.sh"
