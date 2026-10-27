#!/usr/bin/env python3
"""Exercise optional Emulator PSI and EXIT cleanup with a deterministic fake adb."""

from pathlib import Path
import os
import subprocess
import tempfile


SCRIPT = Path(__file__).with_name("run-diagnostic.sh")
FAKE_ADB = r'''#!/usr/bin/env python3
from pathlib import Path
import os
import sys

args = sys.argv[1:]
state = Path(os.environ["G3_TEST_ADB_STATE_DIR"])
with (state / "adb-calls.txt").open("a", encoding="utf-8") as log:
    log.write(" ".join(args) + "\n")

def stop(code=0, output=None, error=None):
    if output:
        print(output)
    if error:
        print(error, file=sys.stderr)
    raise SystemExit(code)

if args == ["get-state"]:
    stop(output="device")
if args and args[0] == "logcat":
    stop(output="synthetic logcat")
if args[:2] == ["shell", "cat"] and len(args) > 2:
    if args[2] == "/proc/pressure/cpu":
        stop(1, error="cat: /proc/pressure/cpu: Permission denied")
    if args[2] == "/proc/meminfo":
        stop(output="MemTotal: 123 kB")
if args[:2] == ["shell", "dumpsys"]:
    stop(output="synthetic meminfo")
if args[:3] == ["shell", "run-as", "app.serea.g3poc"]:
    tail = args[3:]
    if tail == ["rm", "-f", "files/g3-smollm.gguf"]:
        (state / "app-private-model-present").unlink(missing_ok=True)
        stop()
    if tail == ["test", "!", "-e", "files/g3-smollm.gguf"]:
        stop(0 if not (state / "app-private-model-present").exists() else 1)
if args[:3] == ["shell", "rm", "-f"] and len(args) == 4:
    if args[3] == "/data/local/tmp/g3-smollm.gguf":
        (state / "emulator-temp-model-present").unlink(missing_ok=True)
    stop()
if args[:2] == ["shell", "test"] and args[2:] == ["!", "-e", "/data/local/tmp/g3-smollm.gguf"]:
    stop(0 if not (state / "emulator-temp-model-present").exists() else 1)
if args and args[0] == "uninstall":
    stop()
stop()
'''


def run(command: str, evidence: Path, runner_temp: Path, adb_bin: Path, state: Path) -> subprocess.CompletedProcess[str]:
    environment = os.environ.copy()
    environment.update(
        {
            "G3_EVIDENCE_DIR": str(evidence),
            "RUNNER_TEMP": str(runner_temp),
            "G3_TEST_ADB_STATE_DIR": str(state),
            "PATH": f"{adb_bin}{os.pathsep}{environment['PATH']}",
        }
    )
    return subprocess.run(
        ["bash", str(SCRIPT), command],
        env=environment,
        text=True,
        capture_output=True,
        timeout=30,
        check=False,
    )


with tempfile.TemporaryDirectory(prefix="g3-runner-failures-") as directory:
    root = Path(directory)
    adb_bin = root / "bin"
    adb_bin.mkdir()
    fake_adb = adb_bin / "adb"
    fake_adb.write_text(FAKE_ADB, encoding="utf-8")
    fake_adb.chmod(0o755)

    runner_temp = root / "runner-temp"
    state = root / "fake-device"
    runner_temp.mkdir()
    state.mkdir()
    pressure_evidence = root / "pressure-evidence"
    pressure_evidence.mkdir()
    pressure = run("test-cpu-pressure-optional", pressure_evidence, runner_temp, adb_bin, state)
    if pressure.returncode != 0:
        raise SystemExit(f"optional PSI read must not fail the test: {pressure.stderr}")
    pressure_record = (pressure_evidence / "emulator-cpu-pressure-test.txt").read_text(encoding="utf-8")
    pressure_status = (pressure_evidence / "emulator-cpu-pressure-test.txt.status").read_text(encoding="utf-8")
    if "available=false" not in pressure_record or "Permission denied" not in pressure_record:
        raise SystemExit("denied optional PSI read must be recorded in the evidence")
    if "exit_status=1" not in pressure_status:
        raise SystemExit("denied optional PSI status must be recorded")

    (state / "emulator-temp-model-present").write_text("synthetic", encoding="utf-8")
    (state / "app-private-model-present").write_text("synthetic", encoding="utf-8")
    cleanup_evidence = root / "cleanup-evidence"
    cleanup_evidence.mkdir()
    cleanup = run("test-cleanup-after-early-failure", cleanup_evidence, runner_temp, adb_bin, state)
    if cleanup.returncode != 23:
        raise SystemExit(f"EXIT cleanup must preserve the primary status 23, got {cleanup.returncode}")
    cleanup_record = (cleanup_evidence / "model-cleanup.txt").read_text(encoding="utf-8")
    if "app_private_model_removed=true" not in cleanup_record:
        raise SystemExit("cleanup must remove and verify the synthetic app-private model")
    if "emulator_temp_model_removed=true" not in cleanup_record:
        raise SystemExit("cleanup must remove and verify the synthetic emulator-temp model")
    if "primary_exit_status=23" not in cleanup_record or "final_exit_status=23" not in cleanup_record:
        raise SystemExit("cleanup must record and preserve its primary exit status")
    if "cleanup_state_reset=true" not in cleanup_record:
        raise SystemExit("cleanup must reset its global state")
    if (runner_temp / "g3-host-model-fixture.bin").exists():
        raise SystemExit("cleanup must remove the synthetic host-side model file")
    if (state / "app-private-model-present").exists() or (state / "emulator-temp-model-present").exists():
        raise SystemExit("cleanup left a synthetic model copy behind")

print("runner_failure_paths=PASS optional_psi_denied=recorded primary_exit=preserved models=removed state=reset")
