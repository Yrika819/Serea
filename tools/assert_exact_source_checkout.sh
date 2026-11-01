#!/usr/bin/env bash
set -euo pipefail

expected_source_sha="${1:-${EXPECTED_SOURCE_SHA:-}}"
actual_source_sha="$(git rev-parse HEAD)"

if [[ ! "$expected_source_sha" =~ ^[0-9a-f]{40}$ ]]; then
  echo 'FAIL: expected source SHA must be a full lowercase 40-character commit SHA.' >&2
  exit 1
fi

if [[ "$actual_source_sha" != "$expected_source_sha" ]]; then
  echo "FAIL: expected source SHA $expected_source_sha but checked out $actual_source_sha." >&2
  exit 1
fi

echo "PASS: checked out exact source SHA $actual_source_sha"
