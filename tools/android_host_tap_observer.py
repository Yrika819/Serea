#!/usr/bin/env python3
"""Capture Android window/IME ownership and optionally inject requested system taps."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
import time
from pathlib import Path


REQUEST = re.compile(r"HOST_INPUT_REQUEST\|id=(\d+)\|x=(-?\d+)\|y=(-?\d+)")
HOST_DUMPSYS_REQUEST = re.compile(r"HOST_DUMPSYS_REQUEST\|id=(\d+)\|stage=([^|]+)")
PACKAGE = "app.serea.android.isolated"
DEVICE_DIRECTORY = f"/sdcard/Android/data/{PACKAGE}/files/root-cause"
FULL_DUMPSYS_STAGES = {"focused-before-1", "chat-screen-class", "app-shell-class", "full-suite", "focused-after-1"}
FOCUS_LINES = re.compile(
    r"mCurrentFocus|mFocusedApp|mFocusedWindow|mIme(Input|Layering|Control)Target|"
    r"mInputMethodTarget|mTopFocusedDisplayId|mFocusedDisplayId|mServedView|"
    r"mServedConnecting|mInputShown|mShowRequested|mImeWindowVis|mCurFocusedWindow|"
    r"mCurrentTextBoxAttribute|mCurId|mInputMethodClient|mImeInsets|mImeRequestedVisible",
    re.IGNORECASE,
)


def shell(command: list[str], timeout: int = 30) -> dict[str, object]:
    try:
        result = subprocess.run(
            command,
            capture_output=True,
            text=True,
            errors="replace",
            timeout=timeout,
            check=False,
        )
        output = result.stdout
        return {
            "command": command,
            "exit_code": result.returncode,
            "stdout": output,
            "stderr": result.stderr,
            "stdout_bytes": len(output.encode()),
            "stdout_sha256": hashlib.sha256(output.encode()).hexdigest(),
        }
    except subprocess.TimeoutExpired as error:
        stdout = error.stdout.decode(errors="replace") if isinstance(error.stdout, bytes) else error.stdout or ""
        stderr = error.stderr.decode(errors="replace") if isinstance(error.stderr, bytes) else error.stderr or ""
        return {
            "command": command,
            "exit_code": None,
            "stdout": stdout,
            "stderr": stderr,
            "stdout_bytes": len(stdout.encode()),
            "stdout_sha256": hashlib.sha256(stdout.encode()).hexdigest(),
            "timeout_seconds": timeout,
        }


def capture_snapshot(output_dir: Path, event_id: str, phase: str, stage: str) -> None:
    preserve_all = Path(stage).name in FULL_DUMPSYS_STAGES
    extracted: dict[str, list[str]] = {}
    results: dict[str, dict[str, object]] = {}
    for service in ("window", "activity activities", "input_method"):
        result = shell(["adb", "shell", "dumpsys", service])
        raw = str(result["stdout"])
        lines = [line.strip() for line in raw.splitlines() if FOCUS_LINES.search(line)]
        extracted[service] = lines[:240]
        if service == "window" or (service == "input_method" and preserve_all):
            safe_service = service.replace(" ", "-")
            (output_dir / f"dumpsys-{safe_service}-{event_id}-{phase}.txt").write_text(raw, encoding="utf-8")
        if service == "activity activities" and preserve_all:
            (output_dir / f"dumpsys-activity-activities-{event_id}-{phase}.txt").write_text(raw, encoding="utf-8")
        results[service] = {key: value for key, value in result.items() if key != "stdout"}
    summary = {
        "event_id": event_id,
        "phase": phase,
        "stage": stage,
        "captured_at_epoch_ms": round(time.time() * 1000),
        "preserved_raw_input_method": preserve_all,
        "commands": results,
        "focus_lines": extracted,
    }
    with (output_dir / "window-focus-snapshots.jsonl").open("a", encoding="utf-8") as target:
        target.write(json.dumps(summary, ensure_ascii=False) + "\n")


def inject_tap(output_dir: Path, event_log, request_id: str, x: int, y: int, stage: str) -> None:
    capture_snapshot(output_dir, request_id, "before-tap", stage)
    command = ["adb", "shell", "input", "tap", str(x), str(y)]
    started = time.monotonic()
    tap = shell(command, timeout=15)
    elapsed_ms = round((time.monotonic() - started) * 1000)
    result: dict[str, object] = {
        "request_id": request_id,
        "stage": stage,
        "x": x,
        "y": y,
        "command": command,
        "exit_code": tap["exit_code"],
        "stdout": tap["stdout"],
        "stderr": tap["stderr"],
        "elapsed_ms": elapsed_ms,
    }
    subprocess.run(
        ["adb", "shell", "mkdir", "-p", DEVICE_DIRECTORY],
        capture_output=True,
        text=True,
        timeout=15,
        check=False,
    )
    response = output_dir / f"device-tap-result-{request_id}.txt"
    response.write_text(
        "\n".join(
            [
                f"exit_code={tap['exit_code']}",
                f"stdout={tap['stdout']!r}",
                f"stderr={tap['stderr']!r}",
                f"elapsed_ms={elapsed_ms}",
            ]
        )
        + "\n",
        encoding="utf-8",
    )
    push = subprocess.run(
        ["adb", "push", str(response), f"{DEVICE_DIRECTORY}/tap-result-{request_id}.txt"],
        capture_output=True,
        text=True,
        timeout=15,
        check=False,
    )
    result["response_push_exit_code"] = push.returncode
    result["response_push_stdout"] = push.stdout
    result["response_push_stderr"] = push.stderr
    capture_snapshot(output_dir, request_id, "after-tap", stage)
    event_log.write(json.dumps(result, ensure_ascii=False) + "\n")
    event_log.flush()
    print(json.dumps(result, ensure_ascii=False), flush=True)


def capture_synchronized_request(output_dir: Path, event_log, request_id: str, stage_name: str, stage: str) -> None:
    capture_snapshot(output_dir, request_id, "synchronized-request", stage)
    result = {
        "request_id": request_id,
        "stage_name": stage_name,
        "stage": stage,
        "captured_at_epoch_ms": round(time.time() * 1000),
        "window_snapshot": f"dumpsys-window-{request_id}-synchronized-request.txt",
    }
    response = output_dir / f"device-dumpsys-result-{request_id}.txt"
    response.write_text(json.dumps(result, ensure_ascii=False) + "\n", encoding="utf-8")
    subprocess.run(
        ["adb", "shell", "mkdir", "-p", DEVICE_DIRECTORY],
        capture_output=True,
        text=True,
        timeout=15,
        check=False,
    )
    push = subprocess.run(
        ["adb", "push", str(response), f"{DEVICE_DIRECTORY}/dumpsys-result-{request_id}.txt"],
        capture_output=True,
        text=True,
        timeout=15,
        check=False,
    )
    result["response_push_exit_code"] = push.returncode
    result["response_push_stdout"] = push.stdout
    result["response_push_stderr"] = push.stderr
    event_log.write(json.dumps(result, ensure_ascii=False) + "\n")
    event_log.flush()
    print(json.dumps(result, ensure_ascii=False), flush=True)


def main() -> int:
    if len(sys.argv) not in (2, 3):
        print("usage: android_host_tap_observer.py OUTPUT_DIR [--inject-system-taps]", file=sys.stderr)
        return 2
    output_dir = Path(sys.argv[1])
    inject_system_taps = len(sys.argv) == 3 and sys.argv[2] == "--inject-system-taps"
    output_dir.mkdir(parents=True, exist_ok=True)
    event_log = (output_dir / "tap-injections.jsonl").open("a", encoding="utf-8")
    process = subprocess.Popen(
        ["adb", "logcat", "-v", "raw", "-s", "SereaChatWindowFocus:I", "SereaRootCauseFocusOrder:I"],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        errors="replace",
        bufsize=1,
    )
    assert process.stdout is not None
    seen: set[str] = set()
    stage = Path(output_dir).name
    try:
        for line in process.stdout:
            host_request = HOST_DUMPSYS_REQUEST.search(line)
            if host_request is not None:
                request_id, stage_name = host_request.groups()
                capture_synchronized_request(output_dir, event_log, request_id, stage_name, stage)
                continue

            match = REQUEST.search(line)
            if match is None or not inject_system_taps:
                continue
            request_id, x_text, y_text = match.groups()
            if request_id in seen:
                continue
            seen.add(request_id)
            inject_tap(output_dir, event_log, request_id, int(x_text), int(y_text), stage)
    except KeyboardInterrupt:
        pass
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        event_log.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
