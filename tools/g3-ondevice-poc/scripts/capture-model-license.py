#!/usr/bin/env python3
"""Capture license metadata for the exact immutable model repository revision."""

from __future__ import annotations

import argparse
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import Request, urlopen


MAX_METADATA_BYTES = 1_000_000


def validate_metadata(metadata: dict, fixture: dict) -> dict:
    repository = fixture["repository"]
    revision = fixture["repositoryRevision"]
    card = metadata.get("cardData") or {}
    observed_id = metadata.get("id") or metadata.get("_id")
    if observed_id != repository:
        raise ValueError(f"metadata repository mismatch: expected {repository}, got {observed_id}")
    if metadata.get("sha") != revision:
        raise ValueError(f"metadata revision mismatch: expected {revision}, got {metadata.get('sha')}")
    if card.get("license") != fixture["license"]:
        raise ValueError(
            f"model license mismatch: expected {fixture['license']}, got {card.get('license')}"
        )
    expected_base = fixture["baseModel"]
    observed_bases = card.get("base_model") or []
    if expected_base not in observed_bases:
        raise ValueError(f"base model mismatch: expected {expected_base}, got {observed_bases}")
    return {
        "metadata_repository": observed_id,
        "metadata_revision": metadata["sha"],
        "declared_license": card["license"],
        "base_model": expected_base,
    }


def build_evidence(metadata: dict, fixture: dict, url: str, payload: bytes, retrieved_utc: str) -> dict:
    return {
        "source_url": url,
        "retrieved_utc": retrieved_utc,
        "metadata_response_sha256": hashlib.sha256(payload).hexdigest(),
        **validate_metadata(metadata, fixture),
        "expected_artifact_from_model_fixture": {
            "file": fixture["file"],
            "size_bytes": fixture["sizeBytes"],
            "sha256": fixture["sha256"],
        },
        "artifact_values_status": "expected fixture values; not verified by the license metadata response",
        "weights_downloaded_by_this_step": False,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("fixture", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    fixture = json.loads(args.fixture.read_text(encoding="utf-8"))
    url = (
        f"https://huggingface.co/api/models/{fixture['repository']}"
        f"/revision/{fixture['repositoryRevision']}"
    )
    request = Request(url, headers={"User-Agent": "Serea-G3-diagnostic-metadata/1"})
    with urlopen(request, timeout=20) as response:
        payload = response.read(MAX_METADATA_BYTES + 1)
    if len(payload) > MAX_METADATA_BYTES:
        raise ValueError("pinned model metadata response exceeded the size bound")
    metadata = json.loads(payload)
    evidence = build_evidence(
        metadata,
        fixture,
        url,
        payload,
        datetime.now(timezone.utc).isoformat(),
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(
        f"model_license_metadata=PASS repository={evidence['metadata_repository']} "
        f"revision={evidence['metadata_revision']} license={evidence['declared_license']} "
        "weights_downloaded=false"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
