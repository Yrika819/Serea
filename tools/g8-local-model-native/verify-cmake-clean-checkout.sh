#!/usr/bin/env bash
set -euo pipefail

CMAKE_BIN="${1:?pass the pinned Android CMake executable path}"
ROOT_DIR="$(git rev-parse --show-toplevel)"
LLAMA_SOURCE="$ROOT_DIR/llama.cpp"
EXPECTED_LLAMA_COMMIT='276378515ceebee5d14bcc8536603dd5c87491a2'
TEMP_ROOT="$(mktemp -d "${RUNNER_TEMP:-/tmp}/serea-g8-cmake-provenance.XXXXXX")"
WORKTREE="$TEMP_ROOT/llama.cpp"

cleanup() {
    git -C "$LLAMA_SOURCE" worktree remove --force "$WORKTREE" >/dev/null 2>&1 || true
    rm -rf "$TEMP_ROOT"
}
trap cleanup EXIT

test "$(git -C "$LLAMA_SOURCE" rev-parse HEAD)" = "$EXPECTED_LLAMA_COMMIT"
git -C "$LLAMA_SOURCE" worktree add --quiet --detach "$WORKTREE" "$EXPECTED_LLAMA_COMMIT"

assert_rejected_as_dirty() {
    local case_name="$1"
    local build_dir="$TEMP_ROOT/build-$case_name"
    local log_file="$TEMP_ROOT/cmake-$case_name.log"
    if "$CMAKE_BIN" -S "$ROOT_DIR/android/app/src/main/cpp" -B "$build_dir" \
        -DLLAMA_SRC="$WORKTREE" >"$log_file" 2>&1; then
        echo "FAIL: CMake accepted $case_name llama.cpp changes" >&2
        exit 1
    fi
    if ! grep -Fq 'LLAMA_SRC must be a clean Git worktree' "$log_file"; then
        echo "FAIL: CMake rejected $case_name before the source cleanliness guard" >&2
        tail -n 80 "$log_file" >&2
        exit 1
    fi
}

printf '%s\n' 'untracked provenance fixture' > "$WORKTREE/g8-untracked-provenance-fixture"
assert_rejected_as_dirty untracked
rm "$WORKTREE/g8-untracked-provenance-fixture"

printf '\n%s\n' '# tracked provenance fixture' >> "$WORKTREE/README.md"
assert_rejected_as_dirty tracked

echo 'cmake_clean_checkout_guard=PASS tracked=REJECTED untracked=REJECTED'
