#!/usr/bin/env python3
"""Exercise optional Emulator PSI and EXIT cleanup with a deterministic fake adb."""

from pathlib import Path
import os
import subprocess
import tempfile
import time


SCRIPT = Path(__file__).with_name("run-diagnostic.sh")
FAKE_ADB = r'''#!/usr/bin/env python3
from pathlib import Path
import os
import sys
import time

args = sys.argv[1:]
state = Path(os.environ["G3_TEST_ADB_STATE_DIR"])
with (state / "adb-calls.txt").open("a", encoding="utf-8") as log:
    log.write(" ".join(args) + "\n")

if os.environ.get("G3_TEST_ADB_HANG_MATCH") == "logcat" and args[:1] == ["logcat"]:
    time.sleep(60)

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
if args[:4] == ["shell", "pm", "list", "packages"] and len(args) == 5:
    package = args[4]
    if (state / f"package-{package}").exists():
        stop(output=f"package:{package}")
    stop()
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
if args and args[0] == "uninstall" and len(args) == 2:
    (state / f"package-{args[1]}").unlink(missing_ok=True)
    if args[1] == "app.serea.g3poc":
        (state / "app-private-model-present").unlink(missing_ok=True)
    stop()
stop()
'''


def run(
    command: str,
    evidence: Path,
    runner_temp: Path,
    adb_bin: Path,
    state: Path,
    hang_match: str | None = None,
) -> subprocess.CompletedProcess[str]:
    environment = os.environ.copy()
    environment.update(
        {
            "G3_EVIDENCE_DIR": str(evidence),
            "RUNNER_TEMP": str(runner_temp),
            "G3_TEST_ADB_STATE_DIR": str(state),
            "PATH": f"{adb_bin}{os.pathsep}{environment['PATH']}",
        }
    )
    if hang_match is not None:
        environment["G3_TEST_ADB_HANG_MATCH"] = hang_match
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
    (state / "package-app.serea.g3poc").write_text("synthetic", encoding="utf-8")
    (state / "package-app.serea.g3poc.test").write_text("synthetic", encoding="utf-8")
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

    (state / "emulator-temp-model-present").write_text("synthetic", encoding="utf-8")
    (state / "app-private-model-present").write_text("synthetic", encoding="utf-8")
    (state / "package-app.serea.g3poc").write_text("synthetic", encoding="utf-8")
    (state / "package-app.serea.g3poc.test").write_text("synthetic", encoding="utf-8")
    (runner_temp / "g3-host-model-fixture.bin").write_text("synthetic", encoding="utf-8")
    hang_evidence = root / "hang-evidence"
    hang_evidence.mkdir()
    hung_cleanup = run("test-cleanup-after-early-failure", hang_evidence, runner_temp, adb_bin, state, "logcat")
    if hung_cleanup.returncode != 23:
        raise SystemExit(f"a hung ADB diagnostic must preserve the primary status 23, got {hung_cleanup.returncode}")
    hang_record = (hang_evidence / "model-cleanup.txt").read_text(encoding="utf-8")
    if "cleanup_adb.logcat=timeout" not in hang_record:
        raise SystemExit("a hung cleanup ADB call must be recorded as a timeout")
    if "cleanup_adb.emulator-meminfo=success" not in hang_record:
        raise SystemExit("cleanup must continue to later ADB calls after a timeout")
    if "primary_exit_status=23" not in hang_record or "final_exit_status=23" not in hang_record:
        raise SystemExit("a cleanup timeout must preserve the primary failure status")
    if "app_private_model_removed=true" not in hang_record or "emulator_temp_model_removed=true" not in hang_record:
        raise SystemExit("cleanup must remove and verify both model copies after a hung ADB diagnostic")
    if (runner_temp / "g3-host-model-fixture.bin").exists():
        raise SystemExit("cleanup must remove the host model after a hung ADB diagnostic")
    if (state / "app-private-model-present").exists() or (state / "emulator-temp-model-present").exists():
        raise SystemExit("hung cleanup left a synthetic model copy behind")

    # Android's instrumentation runner may remove the disposable app package before
    # the EXIT trap. Model that PackageManager cleanup and require it to be recorded.
    (state / "emulator-temp-model-present").write_text("synthetic", encoding="utf-8")
    (state / "app-private-model-present").write_text("synthetic", encoding="utf-8")
    (state / "package-app.serea.g3poc").write_text("synthetic", encoding="utf-8")
    (state / "package-app.serea.g3poc.test").write_text("synthetic", encoding="utf-8")
    # Simulate the test runner's uninstall transition, including PackageManager
    # removal of the target package's private files directory.
    (state / "package-app.serea.g3poc").unlink()
    (state / "app-private-model-present").unlink()
    (runner_temp / "g3-host-model-fixture.bin").write_text("synthetic", encoding="utf-8")
    utp_evidence = root / "utp-uninstall-evidence"
    utp_evidence.mkdir()
    utp_cleanup = run("test-cleanup-after-early-failure", utp_evidence, runner_temp, adb_bin, state)
    if utp_cleanup.returncode != 23:
        raise SystemExit(f"cleanup after test-runner uninstall must preserve the primary status 23, got {utp_cleanup.returncode}")
    utp_record = (utp_evidence / "model-cleanup.txt").read_text(encoding="utf-8")
    if "target_package_absent_after_test_runner=true" not in utp_record:
        raise SystemExit("cleanup must verify and record when UTP already uninstalled the target package")
    if "app_private_model_removal_reason=target_package_uninstalled_by_test_runner" not in utp_record:
        raise SystemExit("cleanup must record package removal as the app-private model cleanup boundary")
    if "emulator_temp_model_removed=true" not in utp_record:
        raise SystemExit("cleanup must still remove the emulator-temp model after UTP uninstalls the app")
    if (state / "app-private-model-present").exists():
        raise SystemExit("the simulated PackageManager uninstall left the app-private model behind")
    if (runner_temp / "g3-host-model-fixture.bin").exists() or (state / "emulator-temp-model-present").exists():
        raise SystemExit("cleanup after UTP uninstall left a synthetic host or emulator model copy behind")

print("runner_failure_paths=PASS optional_psi_denied=recorded adb_hang=bounded later_cleanup=continued utp_uninstall=verified primary_exit=preserved models=removed state=reset")
