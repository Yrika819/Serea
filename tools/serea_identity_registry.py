"""Independent lane identity binding and local high-water registry."""
from __future__ import annotations

import hashlib
import json
import uuid
from pathlib import Path
from typing import Any

try:
    from .serea_release_ledger import APP_IDS, LANES, MAX_VERSION_CODE, MIN_BETA_VERSION_CODE, normalize_sha256
    from .serea_signing_state import SecureJsonFile, StateFileError
except ImportError:
    from serea_release_ledger import APP_IDS, LANES, MAX_VERSION_CODE, MIN_BETA_VERSION_CODE, normalize_sha256
    from serea_signing_state import SecureJsonFile, StateFileError


class RegistryError(RuntimeError):
    pass


def _digest(value: dict[str, Any]) -> str:
    body = {key: item for key, item in value.items() if key != "head_sha256"}
    return hashlib.sha256(json.dumps(body, sort_keys=True, separators=(",", ":")).encode("ascii")).hexdigest()


def _identities(value: Any) -> dict[str, dict[str, Any]]:
    if not isinstance(value, dict) or set(value) != LANES:
        raise RegistryError("Both signing lane identities are required")
    result = {}
    for lane in sorted(LANES):
        entry = value[lane]
        if not isinstance(entry, dict) or set(entry) != {"application_id", "certificate_sha256", "initial_version_code"}:
            raise RegistryError("Signing lane identity is malformed")
        code = entry["initial_version_code"]
        if entry["application_id"] != APP_IDS[lane] or type(code) is not int or \
                not MIN_BETA_VERSION_CODE <= code <= MAX_VERSION_CODE:
            raise RegistryError("Signing lane identity or initial versionCode is invalid")
        cert = normalize_sha256(entry["certificate_sha256"])
        result[lane] = {"application_id": APP_IDS[lane], "certificate_sha256": cert,
                        "initial_version_code": code, "high_watermark": code - 1,
                        "last_attempt_id": None}
    if result["public"]["certificate_sha256"] == result["private"]["certificate_sha256"]:
        raise RegistryError("Public and Private signing certificates must differ")
    return result


class IdentityRegistry:
    def __init__(self, path: str | Path):
        try:
            self._file = SecureJsonFile(path)
        except StateFileError as exc:
            raise RegistryError(str(exc)) from exc
        self.path = self._file.path

    @staticmethod
    def _validate(value: Any) -> dict[str, Any]:
        if not isinstance(value, dict) or set(value) != {"schema", "registry_id", "witness_id", "mode", "ledger_id", "generation", "lanes", "head_sha256"}:
            raise RegistryError("Identity Registry schema is missing or unsupported")
        if value["schema"] != 1 or value["mode"] not in {"disposable", "production"} or \
                type(value["generation"]) is not int or value["generation"] < 0:
            raise RegistryError("Identity Registry metadata is malformed")
        try:
            uuid.UUID(value["registry_id"])
            uuid.UUID(value["witness_id"])
            uuid.UUID(value["ledger_id"]) 
        except (ValueError, TypeError, AttributeError) as exc:
            raise RegistryError("Identity Registry identifiers are malformed") from exc
        lanes = value["lanes"]
        if not isinstance(lanes, dict) or set(lanes) != LANES:
            raise RegistryError("Identity Registry lanes are malformed")
        for lane in LANES:
            entry = lanes[lane]
            if not isinstance(entry, dict) or set(entry) != {
                    "application_id", "certificate_sha256", "initial_version_code", "high_watermark", "last_attempt_id"}:
                raise RegistryError("Identity Registry lane record is malformed")
            if entry["application_id"] != APP_IDS[lane] or \
                    normalize_sha256(entry["certificate_sha256"]) != entry["certificate_sha256"]:
                raise RegistryError("Identity Registry lane identity is invalid")
            initial, high = entry["initial_version_code"], entry["high_watermark"]
            if type(initial) is not int or type(high) is not int or not MIN_BETA_VERSION_CODE <= initial <= MAX_VERSION_CODE or not initial - 1 <= high <= MAX_VERSION_CODE:
                raise RegistryError("Identity Registry high-water mark is invalid")
            attempt = entry["last_attempt_id"]
            if attempt is not None:
                try:
                    uuid.UUID(attempt)
                except (ValueError, TypeError, AttributeError) as exc:
                    raise RegistryError("Identity Registry attempt identifier is invalid") from exc
        if lanes["public"]["certificate_sha256"] == lanes["private"]["certificate_sha256"] or \
                value["head_sha256"] != _digest(value):
            raise RegistryError("Identity Registry integrity check failed")
        return value

    def read(self) -> dict[str, Any]:
        try:
            return self._validate(self._file.read())
        except StateFileError as exc:
            raise RegistryError(str(exc)) from exc

    def initialize(self, *, mode: str, ledger_id: str, identities: dict[str, dict[str, Any]],
                   witness: dict[str, Any]) -> dict[str, Any]:
        if mode not in {"disposable", "production"}:
            raise RegistryError("Identity Registry mode is invalid")
        lanes = _identities(identities)
        try:
            uuid.UUID(ledger_id)
        except (ValueError, TypeError, AttributeError) as exc:
            raise RegistryError("Release Ledger identifier is invalid") from exc
        witness_lanes = witness.get("lanes") if isinstance(witness, dict) else None
        if not isinstance(witness, dict) or witness.get("mode") != mode or witness.get("ledger_id") != ledger_id or \
                not isinstance(witness_lanes, dict) or set(witness_lanes) != LANES or any(
                    witness_lanes[lane].get("application_id") != lanes[lane]["application_id"] or
                    witness_lanes[lane].get("certificate_sha256") != lanes[lane]["certificate_sha256"] or
                    witness_lanes[lane].get("initial_version_code") != lanes[lane]["initial_version_code"] or
                    witness_lanes[lane].get("high_watermark") != lanes[lane]["initial_version_code"] - 1 or
                    witness_lanes[lane].get("last_attempt_id") is not None
                    for lane in LANES) or witness.get("generation") != 0 or witness.get("events") != []:
            raise RegistryError("Identity Registry requires an exact durable witness binding")
        try:
            uuid.UUID(witness.get("witness_id", ""))
        except (ValueError, TypeError, AttributeError) as exc:
            raise RegistryError("Identity Registry witness identity is invalid") from exc
        value = {"schema": 1, "registry_id": str(uuid.uuid4()), "witness_id": witness["witness_id"],
                 "mode": mode, "ledger_id": ledger_id,
                 "generation": 0, "lanes": lanes, "head_sha256": ""}
        value["head_sha256"] = _digest(value)
        self._validate(value)
        try:
            self._file.create(value)
        except StateFileError as exc:
            raise RegistryError(str(exc)) from exc
        return value

    def assert_binding(self, *, mode: str, ledger_id: str, identities: dict[str, dict[str, Any]]) -> dict[str, Any]:
        value = self.read()
        expected = _identities(identities)
        if value["mode"] != mode or value["ledger_id"] != ledger_id:
            raise RegistryError("Identity Registry mode or Release Ledger binding differs")
        for lane in LANES:
            actual = value["lanes"][lane]
            if actual["application_id"] != expected[lane]["application_id"] or \
                    actual["certificate_sha256"] != expected[lane]["certificate_sha256"] or \
                    actual["initial_version_code"] != expected[lane]["initial_version_code"]:
                raise RegistryError("Identity Registry signer identity differs")
        return value

    def assert_matches_witness(self, witness: dict[str, Any]) -> dict[str, Any]:
        value = self.read()
        if not isinstance(witness, dict) or value["witness_id"] != witness.get("witness_id") or \
                value["mode"] != witness.get("mode") or value["ledger_id"] != witness.get("ledger_id"):
            raise RegistryError("Identity Registry does not match the independent witness binding")
        witness_lanes = witness.get("lanes")
        if not isinstance(witness_lanes, dict) or set(witness_lanes) != LANES:
            raise RegistryError("Independent witness lanes are missing")
        for lane in LANES:
            local, independent = value["lanes"][lane], witness_lanes[lane]
            for field in ("application_id", "certificate_sha256", "initial_version_code", "high_watermark", "last_attempt_id"):
                if local[field] != independent.get(field):
                    raise RegistryError("Identity Registry lane state differs from the independent witness")
        return value

    def consume(self, *, lane: str, version_code: int, attempt_id: str, expected_generation: int) -> dict[str, Any]:
        if lane not in LANES or type(version_code) is not int or not MIN_BETA_VERSION_CODE <= version_code <= MAX_VERSION_CODE:
            raise RegistryError("Identity Registry reservation is invalid")
        try:
            uuid.UUID(attempt_id)
        except (ValueError, TypeError, AttributeError) as exc:
            raise RegistryError("Identity Registry attempt identifier is invalid") from exc
        def update(value: Any) -> dict[str, Any]:
            candidate = self._validate(value)
            if candidate["generation"] != expected_generation:
                raise RegistryError("Identity Registry generation changed")
            entry = candidate["lanes"][lane]
            if entry["last_attempt_id"] == attempt_id and entry["high_watermark"] == version_code:
                return candidate
            if version_code != entry["high_watermark"] + 1:
                raise RegistryError("Identity Registry versionCode is not the next unconsumed value")
            lanes = {name: dict(item) for name, item in candidate["lanes"].items()}
            lanes[lane]["high_watermark"] = version_code
            lanes[lane]["last_attempt_id"] = attempt_id
            candidate = {**candidate, "generation": candidate["generation"] + 1, "lanes": lanes}
            candidate["head_sha256"] = _digest(candidate)
            return self._validate(candidate)
        try:
            return self._validate(self._file.update(update))
        except StateFileError as exc:
            raise RegistryError(str(exc)) from exc
