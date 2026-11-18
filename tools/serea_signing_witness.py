"""Owner-controlled append-only witness for Disposable/production signing high-water marks."""
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


class WitnessError(RuntimeError):
    pass


def _hash(value: dict[str, Any]) -> str:
    body = {key: item for key, item in value.items() if key != "head_sha256"}
    return hashlib.sha256(json.dumps(body, sort_keys=True, separators=(",", ":")).encode("ascii")).hexdigest()


def _identities(value: Any) -> dict[str, dict[str, Any]]:
    if not isinstance(value, dict) or set(value) != LANES:
        raise WitnessError("Both signing lane identities are required")
    lanes = {}
    for lane in sorted(LANES):
        entry = value[lane]
        if not isinstance(entry, dict) or set(entry) != {"application_id", "certificate_sha256", "initial_version_code"}:
            raise WitnessError("Signing witness identity is malformed")
        code = entry["initial_version_code"]
        if entry["application_id"] != APP_IDS[lane] or type(code) is not int or \
                not MIN_BETA_VERSION_CODE <= code <= MAX_VERSION_CODE:
            raise WitnessError("Signing witness lane or initial versionCode is invalid")
        lanes[lane] = {"application_id": APP_IDS[lane], "certificate_sha256": normalize_sha256(entry["certificate_sha256"]),
                       "initial_version_code": code, "high_watermark": code - 1, "last_attempt_id": None}
    if lanes["public"]["certificate_sha256"] == lanes["private"]["certificate_sha256"]:
        raise WitnessError("Public and Private signing certificates must differ")
    return lanes


class SigningWitness:
    def __init__(self, path: str | Path):
        try:
            self._file = SecureJsonFile(path)
        except StateFileError as exc:
            raise WitnessError(str(exc)) from exc
        self.path = self._file.path

    @staticmethod
    def _validate(value: Any) -> dict[str, Any]:
        if not isinstance(value, dict) or set(value) != {
                "schema", "witness_id", "mode", "ledger_id", "generation", "lanes", "events", "head_sha256"}:
            raise WitnessError("Signing witness schema is missing or unsupported")
        if value["schema"] != 1 or value["mode"] not in {"disposable", "production"} or \
                type(value["generation"]) is not int or value["generation"] < 0 or not isinstance(value["events"], list):
            raise WitnessError("Signing witness metadata is malformed")
        try:
            uuid.UUID(value["witness_id"])
            uuid.UUID(value["ledger_id"])
        except (ValueError, TypeError, AttributeError) as exc:
            raise WitnessError("Signing witness identifier is invalid") from exc
        lanes = value["lanes"]
        if not isinstance(lanes, dict) or set(lanes) != LANES:
            raise WitnessError("Signing witness lanes are malformed")
        for lane in LANES:
            entry = lanes[lane]
            if not isinstance(entry, dict) or set(entry) != {
                    "application_id", "certificate_sha256", "initial_version_code", "high_watermark", "last_attempt_id"}:
                raise WitnessError("Signing witness lane record is malformed")
            if entry["application_id"] != APP_IDS[lane] or normalize_sha256(entry["certificate_sha256"]) != entry["certificate_sha256"]:
                raise WitnessError("Signing witness identity is invalid")
            initial, high = entry["initial_version_code"], entry["high_watermark"]
            if type(initial) is not int or type(high) is not int or not MIN_BETA_VERSION_CODE <= initial <= MAX_VERSION_CODE or \
                    not initial - 1 <= high <= MAX_VERSION_CODE:
                raise WitnessError("Signing witness high-water mark is invalid")
        previous = "0" * 64
        history = {lane: lanes[lane]["initial_version_code"] - 1 for lane in LANES}
        last_attempt = {lane: None for lane in LANES}
        for index, event in enumerate(value["events"], start=1):
            if not isinstance(event, dict) or set(event) != {"sequence", "lane", "version_code", "attempt_id", "previous_hash", "event_hash"} or \
                    type(event["sequence"]) is not int or event["sequence"] != index or event["lane"] not in LANES or \
                    event["previous_hash"] != previous:
                raise WitnessError("Signing witness history is malformed")
            lane = event["lane"]
            if type(event["version_code"]) is not int or event["version_code"] != history[lane] + 1 or \
                    event["version_code"] > MAX_VERSION_CODE:
                raise WitnessError("Signing witness event reuses or skips a versionCode")
            try:
                uuid.UUID(event["attempt_id"])
            except (ValueError, TypeError, AttributeError) as exc:
                raise WitnessError("Signing witness attempt identifier is malformed") from exc
            expected = hashlib.sha256(json.dumps({key: item for key, item in event.items() if key != "event_hash"},
                                                 sort_keys=True, separators=(",", ":")).encode("ascii")).hexdigest()
            if event["event_hash"] != expected:
                raise WitnessError("Signing witness history integrity check failed")
            previous = event["event_hash"]
            history[lane] = event["version_code"]
            last_attempt[lane] = event["attempt_id"]
        if any(lanes[lane]["high_watermark"] != history[lane] or
               lanes[lane]["last_attempt_id"] != last_attempt[lane] for lane in LANES):
            raise WitnessError("Signing witness lane high-water marks disagree with its event history")
        if type(value["generation"]) is not int or value["generation"] != len(value["events"]) or value["head_sha256"] != _hash(value):
            raise WitnessError("Signing witness generation or integrity check failed")
        return value

    def read(self) -> dict[str, Any]:
        try:
            return self._validate(self._file.read())
        except StateFileError as exc:
            raise WitnessError(str(exc)) from exc

    def initialize(self, *, mode: str, ledger_id: str, identities: dict[str, dict[str, Any]],
                   confirm_new: bool) -> dict[str, Any]:
        if not confirm_new:
            raise WitnessError("Explicit Owner confirmation is required to establish a new witness")
        if mode not in {"disposable", "production"}:
            raise WitnessError("Signing witness mode is invalid")
        try:
            uuid.UUID(ledger_id)
        except (ValueError, TypeError, AttributeError) as exc:
            raise WitnessError("Release Ledger identifier is invalid") from exc
        value = {"schema": 1, "witness_id": str(uuid.uuid4()), "mode": mode, "ledger_id": ledger_id,
                 "generation": 0, "lanes": _identities(identities), "events": [], "head_sha256": ""}
        value["head_sha256"] = _hash(value)
        self._validate(value)
        try:
            self._file.create(value)
        except StateFileError as exc:
            raise WitnessError(str(exc)) from exc
        return value

    def assert_binding(self, *, mode: str, ledger_id: str, identities: dict[str, dict[str, Any]]) -> dict[str, Any]:
        value = self.read()
        expected = _identities(identities)
        if value["mode"] != mode or value["ledger_id"] != ledger_id:
            raise WitnessError("Signing witness mode or Release Ledger binding differs")
        for lane in LANES:
            actual = value["lanes"][lane]
            if actual["application_id"] != expected[lane]["application_id"] or \
                    actual["certificate_sha256"] != expected[lane]["certificate_sha256"] or \
                    actual["initial_version_code"] != expected[lane]["initial_version_code"]:
                raise WitnessError("Signing witness identity differs")
        return value

    def consume(self, *, lane: str, version_code: int, attempt_id: str, expected_generation: int) -> dict[str, Any]:
        if lane not in LANES or type(version_code) is not int or not MIN_BETA_VERSION_CODE <= version_code <= MAX_VERSION_CODE:
            raise WitnessError("Signing witness reservation is invalid")
        try:
            uuid.UUID(attempt_id)
        except (ValueError, TypeError, AttributeError) as exc:
            raise WitnessError("Signing witness attempt identifier is invalid") from exc
        def update(value: Any) -> dict[str, Any]:
            candidate = self._validate(value)
            for event in candidate["events"]:
                if event["attempt_id"] == attempt_id:
                    if event["lane"] == lane and event["version_code"] == version_code:
                        return candidate
                    raise WitnessError("Signing witness attempt identifier was reused")
            if candidate["generation"] != expected_generation:
                raise WitnessError("Signing witness generation changed")
            entry = candidate["lanes"][lane]
            if version_code != entry["high_watermark"] + 1:
                raise WitnessError("Signing witness refuses a reused or skipped versionCode")
            event = {"sequence": candidate["generation"] + 1, "lane": lane, "version_code": version_code,
                     "attempt_id": attempt_id, "previous_hash": candidate["events"][-1]["event_hash"] if candidate["events"] else "0" * 64,
                     "event_hash": ""}
            event["event_hash"] = hashlib.sha256(json.dumps({key: item for key, item in event.items() if key != "event_hash"},
                                                            sort_keys=True, separators=(",", ":")).encode("ascii")).hexdigest()
            lanes = {name: dict(item) for name, item in candidate["lanes"].items()}
            lanes[lane]["high_watermark"] = version_code
            lanes[lane]["last_attempt_id"] = attempt_id
            candidate = {**candidate, "generation": candidate["generation"] + 1,
                         "lanes": lanes, "events": [*candidate["events"], event]}
            candidate["head_sha256"] = _hash(candidate)
            return self._validate(candidate)
        try:
            return self._validate(self._file.update(update))
        except StateFileError as exc:
            raise WitnessError(str(exc)) from exc
