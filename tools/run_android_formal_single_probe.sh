#!/usr/bin/env bash
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo/android"
mkdir -p app/build/outputs/androidTest-results/api35-isolation
probe=$(mktemp "$repo/tools/.formal-single-probe.XXXXXX")
trap 'rm -f "$probe"' EXIT
python3 - "$repo/tools/run_android_instrumentation_campaign.sh" "$probe" <<'PY'
import pathlib, sys
original = pathlib.Path(sys.argv[1]).read_text()
needle = '''        run_diagnostic_stage cursor-position-focused \\
          app.serea.android.SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges \\
          900 1 app.serea.android.SereaAppShellTest#chatDraftSurvivesGmailAndCalendarDestinationChanges || diagnostics_result=1
'''
if original.count(needle) != 2:
    raise SystemExit('Expected exactly two original focused-stage call sites')
injected = '''        # Diagnostic-only stopping point: never label unexecuted formal stages as passed.
        stop_live_capture
        cleanup_test_ime
        trap - EXIT
        printf 'formal_105=NOT_RUN\\nprocess_death=NOT_RUN\\ndata_isolation_final_verify=NOT_RUN\\nfocused_stage_status=%s\\nime_cleanup=%s\\n' \\
          "$diagnostics_result" "$test_ime_cleanup_result" > "$evidence/formal-single-result.txt"
        if [[ "$diagnostics_result" -eq 0 && "$test_ime_cleanup_result" == PASS ]]; then exit 0; else exit 1; fi
'''
pathlib.Path(sys.argv[2]).write_text(original.replace(needle, needle + injected, 1))
PY
sha256sum "$repo/tools/run_android_instrumentation_campaign.sh" "$probe" > app/build/outputs/androidTest-results/api35-isolation/formal-script-hashes.txt
bash "$probe" 35
