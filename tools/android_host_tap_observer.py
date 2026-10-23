#!/usr/bin/env python3
"""Inject the exact tap requested by the instrumentation test and return its exit code."""

from __future__ import annotations

import json
import re
import subprocess
import sys
import time
from pathlib import Path


REQUEST = re.compile(r"HOST_INPUT_REQUEST\|id=(\d+)\|x=(-?\d+)\|y=(-?\d+)")
PACKAGE = "app.serea.android.isolated"
DEVICE_DIRECTORY = f"/sdcard/Android/data/{PACKAGE}/files/root-cause"


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: android_host_tap_observer.py OUTPUT_DIR", file=sys.stderr)
        return 2
    output_dir = Path(sys.argv[1])
    output_dir.mkdir(parents=True, exist_ok=True)
    event_log = (output_dir / "tap-injections.jsonl").open("a", encoding="utf-8")
    process = subprocess.Popen(
        ["adb", "logcat", "-v", "raw", "-s", "SereaChatWindowFocus:I"],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        errors="replace",
        bufsize=1,
    )
    assert process.stdout is not None
    seen: set[str] = set()
    try:
        for line in process.stdout:
            match = REQUEST.search(line)
            if match is None:
                continue
            request_id, x_text, y_text = match.groups()
            if request_id in seen:
                continue
            seen.add(request_id)
            x, y = int(x_text), int(y_text)
            started = time.monotonic()
            tap = subprocess.run(
                ["adb", "shell", "input", "tap", str(x), str(y)],
                capture_output=True,
                text=True,
                errors="replace",
                timeout=15,
                check=False,
            )
            result = {
                "request_id": request_id,
                "x": x,
                "y": y,
                "command": ["adb", "shell", "input", "tap", str(x), str(y)],
                "exit_code": tap.returncode,
                "stdout": tap.stdout,
                "stderr": tap.stderr,
                "elapsed_ms": round((time.monotonic() - started) * 1000),
            }
            (output_dir / f"tap-result-{request_id}.txt").write_text(
                "\n".join(
                    [
                        f"exit_code={tap.returncode}",
                        f"stdout={tap.stdout!r}",
                        f"stderr={tap.stderr!r}",
                        f"elapsed_ms={result['elapsed_ms']}",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
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
                        f"exit_code={tap.returncode}",
                        f"stdout={tap.stdout!r}",
                        f"stderr={tap.stderr!r}",
                        f"elapsed_ms={result['elapsed_ms']}",
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
            event_log.write(json.dumps(result, ensure_ascii=False) + "\n")
            event_log.flush()
            print(json.dumps(result, ensure_ascii=False), flush=True)
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
