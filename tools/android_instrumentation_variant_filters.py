#!/usr/bin/env python3
"""Partition the unchanged instrumentation gate across offline and loopback-only APKs."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

from verify_android_instrumentation import collect_source_test_identities, read_test_manifest

HTTP_CLASS = "app.serea.android.ChatHttpE2ETest"
EXPECTED_REQUIRED = 103
EXPECTED_ADDITIONAL = 2
EXPECTED_HTTP = 4
EXPECTED_TOTAL = 105


def identity_text(identity: tuple[str, str]) -> str:
    return f"{identity[0]}#{identity[1]}"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--required", type=Path, required=True)
    parser.add_argument("--additional", type=Path, required=True)
    parser.add_argument("--non-required", type=Path, required=True)
    parser.add_argument("--http-tests", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    try:
        required = read_test_manifest(args.required)
        additional = read_test_manifest(args.additional)
        non_required = read_test_manifest(args.non_required)
        declared_http = read_test_manifest(args.http_tests)
        if len(required) != EXPECTED_REQUIRED or len(additional) != EXPECTED_ADDITIONAL:
            raise ValueError(
                f"Required gate changed: required={len(required)} additional={len(additional)} "
                f"expected={EXPECTED_REQUIRED}+{EXPECTED_ADDITIONAL}"
            )
        if required & additional or non_required & (required | additional):
            raise ValueError("Required, Additional, and non-required identity manifests overlap")
        formal = required | additional
        if len(formal) != EXPECTED_TOTAL:
            raise ValueError(f"Expected exactly {EXPECTED_TOTAL} formal identities, got {len(formal)}")
        http = {identity for identity in formal if identity[0] == HTTP_CLASS}
        if declared_http != http:
            raise ValueError(
                f"HTTP loopback manifest differs from the four formal Chat HTTP identities: "
                f"missing={sorted(http - declared_http)} unexpected={sorted(declared_http - http)}"
            )
        offline = formal - http
        if len(http) != EXPECTED_HTTP or len(offline) != EXPECTED_TOTAL - EXPECTED_HTTP:
            raise ValueError(f"Unexpected split sizes: offline={len(offline)} http={len(http)}")
        source_identities = collect_source_test_identities(args.source_root)
        all_manifest_identities = formal | non_required
        if source_identities != all_manifest_identities:
            missing = sorted(all_manifest_identities - source_identities)
            unexpected = sorted(source_identities - all_manifest_identities)
            raise ValueError(f"Source/manifest identity mismatch: missing={missing} unexpected={unexpected}")

        source_sha = subprocess.run(
            ["git", "rev-parse", "HEAD"], check=True, capture_output=True, text=True
        ).stdout.strip()
        tree_sha = subprocess.run(
            ["git", "rev-parse", "HEAD^{tree}"], check=True, capture_output=True, text=True
        ).stdout.strip()
        payload = {
            "source_sha": source_sha,
            "tree_sha": tree_sha,
            "required_count": len(required),
            "additional_count": len(additional),
            "formal_total": len(formal),
            "offline_count": len(offline),
            "http_count": len(http),
            "offline_identities": [identity_text(identity) for identity in sorted(offline)],
            "http_identities": [identity_text(identity) for identity in sorted(http)],
            "offline_filter": ",".join(identity_text(identity) for identity in sorted(offline)),
            "http_filter": ",".join(identity_text(identity) for identity in sorted(http)),
            "all_unique": len(offline | http) == EXPECTED_TOTAL,
        }
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
        print(
            f"android_instrumentation_variant_filter=PASS source={source_sha} tree={tree_sha} "
            f"offline={len(offline)} http={len(http)} formal={len(formal)}"
        )
        return 0
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"android_instrumentation_variant_filter=FAIL reason={error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
