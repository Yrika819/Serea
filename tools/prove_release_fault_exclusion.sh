#!/usr/bin/env bash
# P2H release-exclusion proof. Isolated target tree is shared between builds so
# common dependencies are compiled once; A/B artifacts are classified by seam
# presence and C proves the engine integration-test feature path is active.
set -euo pipefail

SCRATCH="${1:?usage: prove_release_fault_exclusion.sh <scratch-dir>}"
mkdir -p "$SCRATCH"
TARGET_DIR="$SCRATCH/target"

one_storage_rmeta() {
    local dir="$1" label="$2" kind="$3"
    local -a found=()
    for f in "$dir"/libserea_storage-*.rmeta; do
        [ -f "$f" ] || continue
        local has_seam=0
        if strings "$f" | grep 'storage/src/fault.rs' >/dev/null; then has_seam=1; fi
        if [ "$kind" = "seam" ] && [ "$has_seam" -eq 1 ]; then found+=("$f"); fi
        if [ "$kind" = "clean" ] && [ "$has_seam" -eq 0 ]; then found+=("$f"); fi
    done
    if [ "${#found[@]}" -ne 1 ]; then
        echo "FAIL: $label expected exactly 1 $kind storage rmeta, got ${#found[@]}"
        exit 1
    fi
    printf '%s' "${found[0]}"
}

assert_no_seam() {
    local artifact="$1" label="$2"
    if strings "$artifact" | grep 'storage/src/fault\.rs' >/dev/null; then
        echo "FAIL: $label references fault.rs"; exit 1
    fi
    for symbol in Window Action arm arm_after is_armed reach announce; do
        if strings "$artifact" | grep "^${symbol}$" >/dev/null; then
            echo "FAIL: $label exposes '$symbol'"; exit 1
        fi
    done
    echo "ok: $label has no fault-seam source reference and no seam symbol"
}

DEPS="$TARGET_DIR/release/deps"
echo "== A: default storage release =="
CARGO_TARGET_DIR="$TARGET_DIR" cargo build --release -p serea-storage --offline
A=$(one_storage_rmeta "$DEPS" "A" "clean")
assert_no_seam "$A" "A (default storage release)"

# Preserve A because subsequent Cargo invocations may add another feature hash.
cp "$A" "$SCRATCH/default-storage.rmeta"
A="$SCRATCH/default-storage.rmeta"

echo "== B: positive control, explicit seam feature =="
CARGO_TARGET_DIR="$TARGET_DIR" cargo build --release -p serea-storage \
    --features p2h-fault-injection --offline
B=$(one_storage_rmeta "$DEPS" "B" "seam")
if cmp -s "$A" "$B"; then
    echo "FAIL: default and feature artifacts are byte-identical"; exit 1
fi
echo "ok: feature control references fault.rs and differs from default build"

echo "== C: engine all-targets release test build =="
CARGO_TARGET_DIR="$TARGET_DIR" cargo test --no-run --release -p serea-task-engine --all-targets --offline --message-format=json > "$SCRATCH/c-build.json"
python3 - "$SCRATCH/c-build.json" <<'PY'
import json
import subprocess
import sys

executables = []
with open(sys.argv[1], encoding="utf-8") as stream:
    for line in stream:
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        target = message.get("target", {})
        if message.get("reason") == "compiler-artifact" and target.get("name") == "crash":
            executable = message.get("executable")
            if executable:
                executables.append(executable)
if len(executables) != 1:
    raise SystemExit(f"FAIL: expected one crash test executable from C, got {executables}")
strings = subprocess.run(["strings", executables[0]], check=True, capture_output=True, text=True).stdout
if "crates/serea-storage/src/fault.rs" not in strings:
    raise SystemExit(f"FAIL: C crash test executable lacks fault seam: {executables[0]}")
print(f"ok: C's exact crash integration-test binary contains the seam: {executables[0]}")
PY

echo "== D: default workspace release =="
CARGO_TARGET_DIR="$TARGET_DIR" cargo build --release --workspace --offline \
    --message-format=json > "$SCRATCH/d-build.json"
python3 - "$SCRATCH/d-build.json" <<'PY'
import json
import subprocess
import sys

found = {"serea_storage": 0, "serea_task_engine": 0}
with open(sys.argv[1], encoding="utf-8") as stream:
    for line in stream:
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("reason") != "compiler-artifact":
            continue
        target = message.get("target", {})
        name = target.get("name")
        if name not in found or "lib" not in target.get("kind", []):
            continue
        files = [path for path in message.get("filenames", []) if path.endswith((".rmeta", ".rlib"))]
        for path in files:
            strings = subprocess.run(["strings", path], check=True, capture_output=True, text=True).stdout
            if "storage/src/fault.rs" in strings or set(strings.splitlines()).intersection(
                ("Window", "Action", "arm", "arm_after", "reach", "announce")
            ):
                raise SystemExit(f"FAIL: default workspace artifact carries fault seam: {path}")
            found[name] += 1
if any(count == 0 for count in found.values()):
    raise SystemExit(f"FAIL: default workspace artifacts not identified: {found}")
print(f"ok: D-specific workspace production artifacts are clean: {found}")
PY
echo "RELEASE EXCLUSION PROVEN for default storage/workspace release artifacts."
echo "The crash test executable contains the seam only in its test target path."
