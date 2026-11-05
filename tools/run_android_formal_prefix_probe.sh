#!/usr/bin/env bash
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo/android"
mkdir -p app/build/outputs/androidTest-results/api35-isolation
probe=$(mktemp "$repo/tools/.formal-prefix-probe.XXXXXX")
trap 'rm -f "$probe"' EXIT
python3 - "$repo/tools/run_android_instrumentation_campaign.sh" "$probe" <<'PY'
import pathlib, sys
s=pathlib.Path(sys.argv[1]).read_text()
start='      if [[ "$api_level" == 35 ]]; then\n        # Focused companion run preserves the full suite and captures complete window/IME diagnostics.\n'
a=s.index(start)
b=s.index('      if [[ "$diagnostics_result" -eq 0 ]]; then\n        adb install "$isolated_apk"', a)
s=s[:a]+'''      # This experiment skips the prior focused/class diagnostic stages. They are
      # recorded as NOT_RUN; their historical effect is not reproduced.
'''+s[b:]
needle='''          -Pandroid.testInstrumentationRunnerArguments.listener=app.serea.android.SereaInstrumentationLifecycleListener \\
          connectedIsolatedAndroidTest > "$evidence/instrumentation-gradle.log" 2>&1
'''
replacement='''          -Pandroid.testInstrumentationRunnerArguments.listener=app.serea.android.SereaInstrumentationLifecycleListener \\
          -Pandroid.testInstrumentationRunnerArguments.class="$(paste -sd, "$tool_dir/api35-formal-prefix-reference.txt")" \\
          connectedIsolatedAndroidTest > "$evidence/instrumentation-gradle.log" 2>&1
'''
assert s.count(needle)==1
s=s.replace(needle,replacement)
needle2='''        if [[ "$gradle_result" -eq 124 || "$gradle_result" -eq 137 ]]; then
'''
injected='''        # Let logcat drain before stopping capture. Full 105 and post-run gates are not run.
        sleep 2
        stop_live_capture
        order_status=1
        python3 "$tool_dir/verify_api35_prefix_order.py" "$tool_dir/api35-formal-prefix-reference.txt" "$evidence/logcat-live.txt" > "$evidence/prefix-order.log" 2>&1
        order_status=$?
        junit_status=1
        if [[ -d "$root/connected" ]]; then
          mv "$root/connected" "$evidence/junit-prefix"
          verify_args=("$evidence/junit-prefix" --minimum 62 --exact-count 62)
          while IFS= read -r test_identity; do verify_args+=(--require-test "$test_identity"); done < "$tool_dir/api35-formal-prefix-reference.txt"
          python3 "$tool_dir/verify_android_instrumentation.py" "${verify_args[@]}" > "$evidence/prefix-junit-verification.log" 2>&1
          junit_status=$?
        else
          echo 'NOT_RUN: connected JUnit missing' > "$evidence/prefix-junit-verification.log"
        fi
        cleanup_test_ime
        trap - EXIT
        printf 'formal_105=NOT_RUN\\nprior_diagnostic_stages=NOT_RUN\\nprocess_death=NOT_RUN\\ndata_isolation_final_verify=NOT_RUN\\ngradle_exit=%s\\norder_exit=%s\\njunit_exit=%s\\nime_cleanup=%s\\n' "$gradle_result" "$order_status" "$junit_status" "$test_ime_cleanup_result" > "$evidence/prefix-result.txt"
        if [[ "$gradle_result" -eq 0 && "$order_status" -eq 0 && "$junit_status" -eq 0 && "$test_ime_cleanup_result" == PASS ]]; then exit 0; else exit 1; fi
'''
assert s.count(needle2)==1
s=s.replace(needle2,injected+needle2)
pathlib.Path(sys.argv[2]).write_text(s)
PY
sha256sum "$repo/tools/run_android_instrumentation_campaign.sh" "$probe" "$repo/tools/api35-formal-prefix-reference.txt" > app/build/outputs/androidTest-results/api35-isolation/formal-script-hashes.txt
bash "$probe" 35
