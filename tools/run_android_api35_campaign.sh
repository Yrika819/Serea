#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root/android"
bash "$repo_root/tools/run_android_instrumentation_campaign.sh" 35
bash "$repo_root/tools/test_android_update_compatibility.sh"
