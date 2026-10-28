#!/usr/bin/env python3
"""Host regressions for pinned model metadata and APK permission boundaries."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def load_script(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec and spec.loader
    spec.loader.exec_module(module)
    return module


license_module = load_script("capture_model_license", ROOT / "scripts/capture-model-license.py")
permissions = load_script("verify_apk_permissions", ROOT / "scripts/verify-apk-permissions.py")
fixture = json.loads((ROOT / "model-fixture.json").read_text(encoding="utf-8"))
readme_lines = (ROOT / "README.md").read_text(encoding="utf-8").splitlines()
metadata_claims = [line for line in readme_lines if "metadata-only query" in line]
if len(metadata_claims) != 1:
    raise SystemExit("README must state the evidence boundary for the metadata-only query exactly once")
metadata_claim = metadata_claims[0]
for marker in (
    "repository's `apache-2.0` license declaration",
    "base-model metadata only",
    "expected fixture values",
    "not confirmed by the metadata response",
    "weight-fetch step checks the actual downloaded bytes",
):
    if marker not in metadata_claim:
        raise SystemExit(f"README metadata evidence claim must preserve this boundary: {marker}")
metadata = {
    "id": fixture["repository"],
    "sha": fixture["repositoryRevision"],
    "cardData": {"license": fixture["license"], "base_model": [fixture["baseModel"]]},
}
assert license_module.validate_metadata(metadata, fixture)["declared_license"] == "apache-2.0"
evidence = license_module.build_evidence(
    metadata,
    fixture,
    "https://huggingface.co/api/models/fixture/revision/immutable",
    b"synthetic metadata only",
    "2026-10-28T00:00:00Z",
)
assert evidence["expected_artifact_from_model_fixture"] == {
    "file": fixture["file"],
    "size_bytes": fixture["sizeBytes"],
    "sha256": fixture["sha256"],
}
assert evidence["artifact_values_status"] == "expected fixture values; not verified by the license metadata response"
assert "model_file_sha256" not in evidence
for altered in (
    {**metadata, "sha": "0" * 40},
    {**metadata, "cardData": {"license": "unknown", "base_model": [fixture["baseModel"]]}},
):
    try:
        license_module.validate_metadata(altered, fixture)
    except ValueError:
        pass
    else:
        raise SystemExit("pinned model metadata mismatch must fail closed")

app_dump = "package: app.serea.g3poc\n"
test_dump = (
    "package: app.serea.g3poc.test\n"
    "uses-permission: name='android.permission.REORDER_TASKS'\n"
)
permissions.require_allowlist(
    permissions.parse_dump(app_dump, permissions.APP_PACKAGE),
    permissions.APP_ALLOWED,
    "target APK",
)
permissions.require_allowlist(
    permissions.parse_dump(test_dump, permissions.TEST_PACKAGE),
    permissions.TEST_ALLOWED,
    "AndroidTest APK",
)
for bad_app in (
    app_dump + "uses-permission: name='android.permission.INTERNET'\n",
    app_dump + "uses-permission-sdk-23: name='android.permission.ACCESS_NETWORK_STATE'\n",
):
    try:
        permissions.require_allowlist(
            permissions.parse_dump(bad_app, permissions.APP_PACKAGE),
            permissions.APP_ALLOWED,
            "target APK",
        )
    except ValueError:
        pass
    else:
        raise SystemExit("unapproved target APK permissions must fail closed")
for bad_test in (
    test_dump + "uses-permission: name='android.permission.INTERNET'\n",
    "package: app.serea.g3poc.test\n",
):
    try:
        permissions.require_allowlist(
            permissions.parse_dump(bad_test, permissions.TEST_PACKAGE),
            permissions.TEST_ALLOWED,
            "AndroidTest APK",
        )
    except ValueError:
        pass
    else:
        raise SystemExit("AndroidTest APK permissions must match the exact allowlist")
try:
    permissions.parse_dump("package: unexpected.package\n", permissions.APP_PACKAGE)
except ValueError:
    pass
else:
    raise SystemExit("unexpected package identity must fail closed")

print("poc_metadata_contracts=PASS license_revision=exact permissions=explicit_allowlists")
