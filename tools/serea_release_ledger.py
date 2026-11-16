#!/usr/bin/env python3
"""Owner-controlled, fail-closed high-water ledger for Serea signing versions.

The hash chain detects accidental/corrupt edits; it is not an authenticated
witness and cannot detect coordinated rollback of every copy.
"""
from __future__ import annotations

import contextlib
import fcntl
import hashlib
import json
import os
import re
import secrets
import stat
import time
import uuid
from pathlib import Path
from typing import Any

MAX_VERSION_CODE = 2_100_000_000
MIN_BETA_VERSION_CODE = 2
ZERO_HASH = "0" * 64
APP_IDS = {
    "public": "app.serea.android.beta",
    "private": "app.serea.android.privatebeta",
}
LANES = frozenset(APP_IDS)
ABANDON_REASONS = frozenset({
    "build-failed",
    "owner-abandoned-before-attempt",
    "process-crash-reviewed",
    "artifact-review-rejected",
})
TOP_KEYS = {"schema", "mode", "ledger_id", "generation", "lanes", "events", "head_sha256"}
LANE_KEYS = {"application_id", "certificate_sha256", "initial_version_code", "next_version_code"}
EVENT_KEYS = {
    "sequence", "lane", "version_code", "state", "occurred_utc", "attempt_id",
    "application_id", "certificate_sha256", "artifact_sha256", "version_name",
    "distribution_ref", "reason", "previous_hash", "event_hash",
}


class LedgerError(RuntimeError):
    """Safe-to-display ledger refusal; messages must not contain secrets."""


def normalize_sha256(value: str) -> str:
    if not isinstance(value, str):
        raise LedgerError("Certificate/artifact SHA-256 must be text")
    normalized = value.replace(":", "").strip().lower()
    if not re.fullmatch(r"[0-9a-f]{64}", normalized):
        raise LedgerError("Certificate/artifact SHA-256 must contain exactly 64 hexadecimal digits")
    return normalized


def _canonical_json(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("ascii")


def _event_hash(event: dict[str, Any]) -> str:
    material = {key: value for key, value in event.items() if key != "event_hash"}
    return hashlib.sha256(_canonical_json(material)).hexdigest()


def _canonical_path(path: str | Path) -> Path:
    expanded = Path(os.path.abspath(os.path.expanduser(os.fspath(path))))
    cursor = Path(expanded.anchor)
    for part in expanded.parts[1:]:
        cursor = cursor / part
        try:
            info = cursor.lstat()
        except FileNotFoundError:
            continue
        if stat.S_ISLNK(info.st_mode):
            raise LedgerError("Release Ledger paths must not contain symbolic links")
    return expanded


def _utc_now() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def _validate_version_code(value: Any, *, allow_exhausted: bool = False) -> int:
    if type(value) is not int:
        raise LedgerError("Release Ledger versionCode must be an integer")
    upper = MAX_VERSION_CODE + 1 if allow_exhausted else MAX_VERSION_CODE
    if value < MIN_BETA_VERSION_CODE or value > upper:
        raise LedgerError("Release Ledger versionCode is outside the supported Android range")
    return value


def _validate_mode(mode: str) -> str:
    if mode not in ("disposable", "production"):
        raise LedgerError("Release Ledger mode must be disposable or production")
    return mode


def _empty_event(sequence: int, lane: str, version_code: int, state: str,
                 attempt_id: str, identity: dict[str, Any], previous_hash: str, *,
                 artifact_sha256: str | None = None, version_name: str | None = None,
                 distribution_ref: str | None = None, reason: str | None = None) -> dict[str, Any]:
    event = {
        "sequence": sequence,
        "lane": lane,
        "version_code": version_code,
        "state": state,
        "occurred_utc": _utc_now(),
        "attempt_id": attempt_id,
        "application_id": identity["application_id"],
        "certificate_sha256": identity["certificate_sha256"],
        "artifact_sha256": artifact_sha256,
        "version_name": version_name,
        "distribution_ref": distribution_ref,
        "reason": reason,
        "previous_hash": previous_hash,
        "event_hash": "",
    }
    event["event_hash"] = _event_hash(event)
    return event


def _validate_ledger(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict) or set(value) != TOP_KEYS or value.get("schema") != 1:
        raise LedgerError("Release Ledger schema is missing or unsupported")
    mode = value.get("mode")
    if not isinstance(mode, str):
        raise LedgerError("Release Ledger mode is missing or malformed")
    _validate_mode(mode)
    try:
        uuid.UUID(value.get("ledger_id", ""))
    except (ValueError, AttributeError, TypeError) as exc:
        raise LedgerError("Release Ledger identifier is invalid") from exc
    if type(value.get("generation")) is not int or value["generation"] < 0:
        raise LedgerError("Release Ledger generation is invalid")
    lanes = value.get("lanes")
    events = value.get("events")
    if not isinstance(lanes, dict) or set(lanes) != LANES or not isinstance(events, list):
        raise LedgerError("Release Ledger lanes or event list are malformed")
    if len(events) > 100_000:
        raise LedgerError("Release Ledger event list exceeds its supported limit")

    for lane, application_id in APP_IDS.items():
        identity = lanes[lane]
        if not isinstance(identity, dict) or set(identity) != LANE_KEYS:
            raise LedgerError("Release Ledger lane identity is malformed")
        if identity.get("application_id") != application_id:
            raise LedgerError("Release Ledger application ID does not match the signing lane")
        fingerprint = normalize_sha256(identity.get("certificate_sha256", ""))
        if identity["certificate_sha256"] != fingerprint:
            raise LedgerError("Release Ledger certificate fingerprint is not canonical")
        _validate_version_code(identity.get("initial_version_code"))
        _validate_version_code(identity.get("next_version_code"), allow_exhausted=True)

    previous_hash = ZERO_HASH
    expected_sequence = 0
    current: dict[tuple[str, int], dict[str, Any]] = {}
    expected_next = {lane: lanes[lane]["initial_version_code"] for lane in LANES}
    reservation_count = {lane: 0 for lane in LANES}
    for event in events:
        if not isinstance(event, dict) or set(event) != EVENT_KEYS:
            raise LedgerError("Release Ledger event schema is malformed")
        expected_sequence += 1
        if type(event.get("sequence")) is not int or event["sequence"] != expected_sequence:
            raise LedgerError("Release Ledger event sequence is not contiguous")
        lane = event.get("lane")
        if lane not in LANES or event.get("application_id") != lanes[lane]["application_id"]:
            raise LedgerError("Release Ledger event is bound to the wrong application")
        if event.get("certificate_sha256") != lanes[lane]["certificate_sha256"]:
            raise LedgerError("Release Ledger event is bound to the wrong signing certificate")
        version_code = _validate_version_code(event.get("version_code"))
        state = event.get("state")
        if state not in {"RESERVED", "ATTEMPTED", "BUILT", "DISTRIBUTED", "ABANDONED"}:
            raise LedgerError("Release Ledger contains an unknown event state")
        if event.get("previous_hash") != previous_hash or event.get("event_hash") != _event_hash(event):
            raise LedgerError("Release Ledger hash chain validation failed")
        if not isinstance(event.get("occurred_utc"), str) or not re.fullmatch(
                r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", event["occurred_utc"]):
            raise LedgerError("Release Ledger event timestamp is malformed")
        try:
            attempt_id = str(uuid.UUID(event.get("attempt_id", "")))
        except (ValueError, AttributeError, TypeError) as exc:
            raise LedgerError("Release Ledger attempt identifier is invalid") from exc
        key = (lane, version_code)
        prior = current.get(key)
        if state == "RESERVED":
            if version_code != expected_next[lane] or (prior and prior["state"] not in {"BUILT", "DISTRIBUTED", "ABANDONED"}):
                raise LedgerError("Release Ledger reservation sequence or transition is invalid")
            expected_next[lane] += 1
            reservation_count[lane] += 1
            if attempt_id in {record["attempt_id"] for record in current.values()}:
                raise LedgerError("Release Ledger attempt identifier was reused")
            if any(event.get(field) is not None for field in ("artifact_sha256", "version_name", "distribution_ref", "reason")):
                raise LedgerError("Reserved Release Ledger event contains unexpected data")
        else:
            if prior is None or prior["attempt_id"] != attempt_id:
                raise LedgerError("Release Ledger state transition has no matching reservation")
            allowed = {
                "ATTEMPTED": {"RESERVED"},
                "BUILT": {"ATTEMPTED"},
                "DISTRIBUTED": {"BUILT"},
                "ABANDONED": {"RESERVED", "ATTEMPTED"},
            }
            if prior["state"] not in allowed[state]:
                raise LedgerError("Release Ledger state transition is invalid")
            if state == "ATTEMPTED" and any(event.get(field) is not None for field in (
                    "artifact_sha256", "version_name", "distribution_ref", "reason")):
                raise LedgerError("Attempted Release Ledger event contains unexpected data")
            if state == "BUILT":
                normalize_sha256(event.get("artifact_sha256") or "")
                if not isinstance(event.get("version_name"), str) or not event["version_name"]:
                    raise LedgerError("Built Release Ledger event is missing versionName")
                if event.get("distribution_ref") is not None or event.get("reason") is not None:
                    raise LedgerError("Built Release Ledger event contains unexpected data")
            if state == "DISTRIBUTED":
                if event.get("artifact_sha256") != prior.get("artifact_sha256") or \
                        event.get("version_name") != prior.get("version_name"):
                    raise LedgerError("Distribution record does not match the built artifact")
                if not isinstance(event.get("distribution_ref"), str) or not re.fullmatch(
                        r"[A-Za-z0-9._:/-]{1,120}", event["distribution_ref"]):
                    raise LedgerError("Distribution reference is malformed")
                if event.get("reason") is not None:
                    raise LedgerError("Distribution event contains an unexpected reason")
            if state == "ABANDONED":
                if event.get("reason") not in ABANDON_REASONS:
                    raise LedgerError("Abandonment reason is not an approved nonsecret category")
                if any(event.get(field) is not None for field in ("artifact_sha256", "version_name", "distribution_ref")):
                    raise LedgerError("Abandoned Release Ledger event contains artifact data")
        current[key] = {**event, "attempt_id": attempt_id}
        previous_hash = event["event_hash"]

    if value["generation"] != len(events):
        raise LedgerError("Release Ledger generation does not match its event sequence")
    if value["head_sha256"] != previous_hash:
        raise LedgerError("Release Ledger head hash does not match its event chain")
    for lane in LANES:
        if value["lanes"][lane]["next_version_code"] != expected_next[lane]:
            raise LedgerError("Release Ledger high-water counter does not match its reservation history")
    return value


class ReleaseLedger:
    """Strict JSON ledger stored in a dedicated owner-private directory."""

    def __init__(self, path: str | Path):
        self.path = _canonical_path(path)
        self.parent = self.path.parent
        self.lock_name = self.path.name + ".lock"
        self.temp_prefix = f".{self.path.name}.serea-tmp-"

    def _open_parent(self) -> int:
        if not self.parent.is_dir() or self.parent.is_symlink():
            raise LedgerError("Release Ledger parent directory must already exist and be non-symlink")
        parent_info = self.parent.stat()
        if parent_info.st_uid != os.getuid() or stat.S_IMODE(parent_info.st_mode) != 0o700:
            raise LedgerError("Release Ledger parent directory must be Owner-owned with mode 0700")
        flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
        try:
            fd = os.open(self.parent, flags)
        except OSError as exc:
            raise LedgerError("Cannot safely open Release Ledger directory") from exc
        opened = os.fstat(fd)
        if (opened.st_dev, opened.st_ino, opened.st_uid) != (
                parent_info.st_dev, parent_info.st_ino, os.getuid()):
            os.close(fd)
            raise LedgerError("Release Ledger directory identity changed")
        return fd

    @contextlib.contextmanager
    def _locked(self):
        directory_fd = self._open_parent()
        lock_fd: int | None = None
        try:
            flags = os.O_CREAT | os.O_RDWR | getattr(os, "O_NOFOLLOW", 0)
            try:
                lock_fd = os.open(self.lock_name, flags, 0o600, dir_fd=directory_fd)
            except OSError as exc:
                raise LedgerError("Cannot safely open Release Ledger lock") from exc
            info = os.fstat(lock_fd)
            if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_nlink != 1 or \
                    stat.S_IMODE(info.st_mode) != 0o600:
                raise LedgerError("Release Ledger lock file is unsafe")
            fcntl.flock(lock_fd, fcntl.LOCK_EX)
            names = os.listdir(directory_fd)
            if any(name.startswith(self.temp_prefix) for name in names):
                raise LedgerError("A stale Release Ledger staging file exists; Owner recovery is required")
            yield directory_fd
        finally:
            if lock_fd is not None:
                with contextlib.suppress(OSError):
                    fcntl.flock(lock_fd, fcntl.LOCK_UN)
                os.close(lock_fd)
            os.close(directory_fd)

    def _load_at(self, directory_fd: int) -> dict[str, Any]:
        flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
        try:
            fd = os.open(self.path.name, flags, dir_fd=directory_fd)
        except OSError as exc:
            raise LedgerError("Release Ledger is missing or cannot be opened safely") from exc
        try:
            info = os.fstat(fd)
            if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or \
                    stat.S_IMODE(info.st_mode) != 0o600 or info.st_nlink != 1:
                raise LedgerError("Release Ledger file must be Owner-owned, regular and mode 0600")
            with os.fdopen(fd, "r", encoding="utf-8") as stream:
                fd = -1
                try:
                    value = json.load(stream)
                except (OSError, UnicodeError, json.JSONDecodeError) as exc:
                    raise LedgerError("Release Ledger JSON is corrupt") from exc
        finally:
            if fd >= 0:
                os.close(fd)
        return _validate_ledger(value)

    def _atomic_write(self, directory_fd: int, value: dict[str, Any], *, create: bool = False) -> None:
        name = self.temp_prefix + secrets.token_hex(16)
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
        try:
            fd = os.open(name, flags, 0o600, dir_fd=directory_fd)
        except OSError as exc:
            raise LedgerError("Cannot create Release Ledger staging file") from exc
        published = False
        try:
            data = json.dumps(value, indent=2, sort_keys=True).encode("utf-8") + b"\n"
            view = memoryview(data)
            while view:
                written = os.write(fd, view)
                if written <= 0:
                    raise LedgerError("Release Ledger write made no progress")
                view = view[written:]
            os.fsync(fd)
            os.close(fd)
            fd = -1
            if create:
                try:
                    os.link(name, self.path.name, src_dir_fd=directory_fd, dst_dir_fd=directory_fd,
                            follow_symlinks=False)
                except FileExistsError as exc:
                    raise LedgerError("Release Ledger already exists; refusing replacement") from exc
                published = True
                os.unlink(name, dir_fd=directory_fd)
            else:
                existing = os.stat(self.path.name, dir_fd=directory_fd, follow_symlinks=False)
                if not stat.S_ISREG(existing.st_mode) or existing.st_uid != os.getuid() or \
                        stat.S_IMODE(existing.st_mode) != 0o600 or existing.st_nlink != 1:
                    raise LedgerError("Existing Release Ledger was replaced or has unsafe permissions")
                os.replace(name, self.path.name, src_dir_fd=directory_fd, dst_dir_fd=directory_fd)
            published = True
            os.fsync(directory_fd)
        except OSError as exc:
            if published:
                raise LedgerError("Release Ledger publish durability is uncertain; stop and reconcile") from exc
            raise LedgerError("Release Ledger update failed before atomic publish") from exc
        finally:
            if fd >= 0:
                os.close(fd)
            if not published:
                with contextlib.suppress(FileNotFoundError):
                    os.unlink(name, dir_fd=directory_fd)

    @staticmethod
    def _identity(application_id: str, certificate_sha256: str) -> dict[str, str]:
        lane = next((name for name, app_id in APP_IDS.items() if app_id == application_id), None)
        if lane is None:
            raise LedgerError("Application ID does not identify a supported signing lane")
        return {"application_id": application_id,
                "certificate_sha256": normalize_sha256(certificate_sha256)}

    @classmethod
    def initialize(cls, path: str | Path, *, mode: str,
                   identities: dict[str, dict[str, Any]], ledger_id: str | None = None) -> dict[str, Any]:
        mode = _validate_mode(mode)
        if not isinstance(identities, dict) or set(identities) != LANES:
            raise LedgerError("Both independent signing lanes are required to initialize the Release Ledger")
        lanes: dict[str, dict[str, Any]] = {}
        for lane in sorted(LANES):
            item = identities[lane]
            if not isinstance(item, dict) or set(item) != {"application_id", "certificate_sha256", "initial_version_code"}:
                raise LedgerError("Release Ledger initialization identity is malformed")
            expected = cls._identity(item["application_id"], item["certificate_sha256"])
            if expected["application_id"] != APP_IDS[lane]:
                raise LedgerError("Release Ledger lane does not match its Application ID")
            version_code = _validate_version_code(item["initial_version_code"])
            lanes[lane] = {**expected, "initial_version_code": version_code,
                           "next_version_code": version_code}
        if lanes["public"]["certificate_sha256"] == lanes["private"]["certificate_sha256"]:
            raise LedgerError("Public and Private Release Ledger identities must differ")
        try:
            ledger_identifier = str(uuid.UUID(ledger_id)) if ledger_id is not None else str(uuid.uuid4())
        except (ValueError, TypeError, AttributeError) as exc:
            raise LedgerError("Release Ledger identifier is invalid") from exc
        ledger = cls(path)
        with ledger._locked() as directory_fd:
            if ledger.path.name in os.listdir(directory_fd):
                raise LedgerError("Release Ledger already exists; refusing replacement")
            value = {"schema": 1, "mode": mode, "ledger_id": ledger_identifier,
                     "generation": 0, "lanes": lanes, "events": [], "head_sha256": ZERO_HASH}
            _validate_ledger(value)
            ledger._atomic_write(directory_fd, value, create=True)
        return value

    def read(self, *, expected_mode: str | None = None) -> dict[str, Any]:
        with self._locked() as directory_fd:
            value = self._load_at(directory_fd)
        if expected_mode is not None and value["mode"] != expected_mode:
            raise LedgerError("Release Ledger mode does not match the requested operation")
        return value

    @staticmethod
    def _latest_by_lane(ledger: dict[str, Any]) -> dict[str, dict[str, Any] | None]:
        latest: dict[str, dict[str, Any] | None] = {lane: None for lane in LANES}
        for event in ledger["events"]:
            latest[event["lane"]] = event
        return latest

    def assert_matches_vault(self, vault_ledger: dict[str, Any], manifest: dict[str, Any], *,
                             expected_mode: str) -> dict[str, Any]:
        ledger = self.read(expected_mode=expected_mode)
        if not isinstance(vault_ledger, dict) or vault_ledger.get("schema") != 1:
            raise LedgerError("Vault version ledger is missing or malformed")
        vault_lanes = vault_ledger.get("identities")
        identities = manifest.get("identities")
        if not isinstance(vault_lanes, dict) or not isinstance(identities, dict):
            raise LedgerError("Vault identity or version ledger is incomplete")
        if vault_ledger.get("release_ledger_id") != ledger["ledger_id"] or \
                vault_ledger.get("release_ledger_path") != str(self.path):
            raise LedgerError("Vault is not bound to this independent Release Ledger")
        latest = self._latest_by_lane(ledger)
        for lane in LANES:
            external = ledger["lanes"][lane]
            internal = vault_lanes.get(lane)
            manifest_identity = identities.get(lane)
            if not isinstance(internal, dict) or not isinstance(manifest_identity, dict):
                raise LedgerError("Vault lane metadata is incomplete")
            if manifest_identity.get("application_id") != external["application_id"] or \
                    normalize_sha256(str(manifest_identity.get("sha256", ""))) != external["certificate_sha256"]:
                raise LedgerError("Vault signer identity differs from the independent Release Ledger")
            next_code = _validate_version_code(internal.get("next_version_code"), allow_exhausted=True)
            if next_code != external["next_version_code"]:
                raise LedgerError("Vault counter and independent Release Ledger disagree; signing is blocked")
            state = latest[lane]
            if state is not None and state["state"] in {"RESERVED", "ATTEMPTED"}:
                raise LedgerError("Release Ledger has an unfinished attempt; Owner review is required")
            artifacts = internal.get("artifacts")
            built_events = [event for event in ledger["events"]
                            if event["lane"] == lane and event["state"] == "BUILT"]
            if not isinstance(artifacts, list) or len(artifacts) != len(built_events):
                raise LedgerError("Vault artifact history does not match Release Ledger BUILT records")
            for artifact, event in zip(artifacts, built_events):
                if not isinstance(artifact, dict) or \
                        artifact.get("version_code") != event["version_code"] or \
                        artifact.get("version_name") != event["version_name"] or \
                        normalize_sha256(str(artifact.get("apk_sha256", ""))) != event["artifact_sha256"]:
                    raise LedgerError("Vault artifact history does not match Release Ledger BUILT records")
        return ledger

    def _append(self, lane: str, version_code: int, expected_state: set[str] | None,
                target_state: str, *, attempt_id: str | None = None,
                artifact_sha256: str | None = None, version_name: str | None = None,
                distribution_ref: str | None = None, reason: str | None = None,
                expected_generation: int | None = None) -> dict[str, Any]:
        if lane not in LANES:
            raise LedgerError("Unknown Release Ledger lane")
        version_code = _validate_version_code(version_code)
        ledger = self
        with ledger._locked() as directory_fd:
            value = ledger._load_at(directory_fd)
            if expected_generation is not None and value["generation"] != expected_generation:
                raise LedgerError("Release Ledger generation changed during the operation")
            identity = value["lanes"][lane]
            events = value["events"]
            latest = next((event for event in reversed(events)
                           if event["lane"] == lane and event["version_code"] == version_code), None)
            if target_state == "RESERVED":
                if expected_state is not None or version_code != identity["next_version_code"]:
                    raise LedgerError("Release Ledger version is not the next unconsumed version")
                lane_latest = next((event for event in reversed(events) if event["lane"] == lane), None)
                if lane_latest and lane_latest["state"] in {"RESERVED", "ATTEMPTED"}:
                    raise LedgerError("Lane has an unfinished reservation; explicit Owner recovery is required")
                attempt_id = attempt_id or str(uuid.uuid4())
                try:
                    uuid.UUID(attempt_id)
                except (ValueError, TypeError, AttributeError) as exc:
                    raise LedgerError("Release Ledger attempt identifier is invalid") from exc
            else:
                if latest is None or latest["state"] not in (expected_state or set()) or \
                        not attempt_id or latest["attempt_id"] != attempt_id:
                    raise LedgerError("Release Ledger transition does not match the current reservation")
            if target_state == "ABANDONED" and reason not in ABANDON_REASONS:
                raise LedgerError("Abandonment requires an approved nonsecret reason category")
            if target_state == "BUILT":
                artifact_sha256 = normalize_sha256(artifact_sha256 or "")
                if not isinstance(version_name, str) or not version_name or len(version_name) > 80:
                    raise LedgerError("Built artifact versionName is invalid")
            if target_state == "DISTRIBUTED":
                if latest is None:
                    raise LedgerError("No built artifact is available for distribution recording")
                artifact_sha256 = latest["artifact_sha256"]
                version_name = latest["version_name"]
                if not isinstance(distribution_ref, str) or not re.fullmatch(
                        r"[A-Za-z0-9._:/-]{1,120}", distribution_ref):
                    raise LedgerError("Distribution reference must be a nonsecret receipt identifier")
            previous_hash = value["head_sha256"]
            event = _empty_event(value["generation"] + 1, lane, version_code, target_state,
                                 attempt_id or "", identity, previous_hash,
                                 artifact_sha256=artifact_sha256, version_name=version_name,
                                 distribution_ref=distribution_ref, reason=reason)
            candidate = {**value, "generation": value["generation"] + 1,
                         "events": [*events, event], "head_sha256": event["event_hash"]}
            if target_state == "RESERVED":
                candidate["lanes"] = {**value["lanes"], lane: {
                    **identity, "next_version_code": version_code + 1}}
            _validate_ledger(candidate)
            ledger._atomic_write(directory_fd, candidate)
            return event

    def reserve(self, lane: str, *, expected_generation: int | None = None,
                attempt_id: str | None = None) -> dict[str, Any]:
        value = self.read()
        if lane not in LANES:
            raise LedgerError("Unknown Release Ledger lane")
        version_code = value["lanes"][lane]["next_version_code"]
        _validate_version_code(version_code)
        return self._append(lane, version_code, None, "RESERVED", expected_generation=expected_generation,
                            attempt_id=attempt_id)

    def mark_attempted(self, lane: str, version_code: int, attempt_id: str) -> dict[str, Any]:
        return self._append(lane, version_code, {"RESERVED"}, "ATTEMPTED", attempt_id=attempt_id)

    def mark_built(self, lane: str, version_code: int, attempt_id: str, *,
                   artifact_sha256: str, version_name: str) -> dict[str, Any]:
        return self._append(lane, version_code, {"ATTEMPTED"}, "BUILT", attempt_id=attempt_id,
                            artifact_sha256=artifact_sha256, version_name=version_name)

    def mark_distributed(self, lane: str, version_code: int, attempt_id: str, *,
                         distribution_ref: str) -> dict[str, Any]:
        return self._append(lane, version_code, {"BUILT"}, "DISTRIBUTED", attempt_id=attempt_id,
                            distribution_ref=distribution_ref)

    def abandon(self, lane: str, version_code: int, attempt_id: str, *, reason: str) -> dict[str, Any]:
        return self._append(lane, version_code, {"RESERVED", "ATTEMPTED"}, "ABANDONED",
                            attempt_id=attempt_id, reason=reason)
