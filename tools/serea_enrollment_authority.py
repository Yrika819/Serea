"""Offline Ed25519 enrollment and signing authority for Serea release identities."""
from __future__ import annotations

import hashlib
import json
import os
import re
import subprocess
import tempfile
import uuid
from pathlib import Path
from typing import Any

try:
    from .serea_release_ledger import APP_IDS, LANES, MAX_VERSION_CODE, MIN_BETA_VERSION_CODE, normalize_sha256
    from .serea_signing_state import SecureJsonFile, StateFileError
except ImportError:
    from serea_release_ledger import APP_IDS, LANES, MAX_VERSION_CODE, MIN_BETA_VERSION_CODE, normalize_sha256
    from serea_signing_state import SecureJsonFile, StateFileError


class AuthorityError(RuntimeError):
    """Fail-closed error from enrollment or signing authority operations."""


def _uuid(value: Any) -> str:
    try:
        return str(uuid.UUID(value))
    except (ValueError, TypeError, AttributeError) as exc:
        raise AuthorityError("Authority identifier is invalid") from exc


def canonical_json(value: Any) -> bytes:
    try:
        return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True,
                          allow_nan=False).encode("ascii")
    except (TypeError, ValueError) as exc:
        raise AuthorityError("Authority record is not canonical JSON data") from exc


def _openssl(*args: str, data: bytes | None = None, check: bool = True) -> bytes:
    try:
        result = subprocess.run(["openssl", *args], input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                check=False, timeout=15)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise AuthorityError("OpenSSL operation failed or timed out") from exc
    if check and result.returncode != 0:
        raise AuthorityError("OpenSSL operation failed")
    return result.stdout if result.returncode == 0 else b""


def _private_key(path: str | Path) -> Path:
    value = Path(os.path.abspath(os.fspath(path)))
    try:
        info = value.lstat()
    except OSError as exc:
        raise AuthorityError("Authority private key is missing") from exc
    if value.is_symlink() or not value.is_file() or info.st_uid != os.getuid() or info.st_mode & 0o077:
        raise AuthorityError("Authority private key must be an Owner-only regular file")
    return value


def _public_key(path: str | Path) -> Path:
    value = Path(os.path.abspath(os.fspath(path)))
    try:
        info = value.lstat()
    except OSError as exc:
        raise AuthorityError("Pinned Authority public key is missing") from exc
    if value.is_symlink() or not value.is_file() or info.st_uid != os.getuid():
        raise AuthorityError("Pinned Authority public key must be an Owner-owned regular file")
    return value


def generate_keypair(private_path: str | Path, public_path: str | Path) -> None:
    """Create a disposable or offline-Authority Ed25519 keypair without replacing files."""
    private = Path(os.path.abspath(os.fspath(private_path)))
    public = Path(os.path.abspath(os.fspath(public_path)))
    if private.parent != public.parent or not private.parent.is_dir() or private.parent.is_symlink():
        raise AuthorityError("Authority key files must share an existing non-symlink parent")
    if private.exists() or private.is_symlink() or public.exists() or public.is_symlink():
        raise AuthorityError("Authority key path already exists; refusing replacement")
    with tempfile.TemporaryDirectory(prefix=".serea-authority-", dir=private.parent) as temporary:
        temp = Path(temporary)
        temp_private, temp_public = temp / "private.pem", temp / "public.pem"
        _openssl("genpkey", "-algorithm", "ED25519", "-out", str(temp_private))
        os.chmod(temp_private, 0o600)
        _openssl("pkey", "-in", str(temp_private), "-pubout", "-out", str(temp_public))
        os.chmod(temp_public, 0o600)
        try:
            os.link(temp_private, private, follow_symlinks=False)
            os.link(temp_public, public, follow_symlinks=False)
            directory_fd = os.open(private.parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
            try:
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)
        except OSError as exc:
            for path in (private, public):
                try:
                    path.unlink()
                except FileNotFoundError:
                    pass
            raise AuthorityError("Authority keypair publication failed; stop and inspect") from exc


def _sign(payload: dict[str, Any], private_key: Path) -> dict[str, Any]:
    body = canonical_json(payload)
    with tempfile.TemporaryDirectory(prefix="serea-authority-sign-") as temporary:
        message = Path(temporary) / "message.bin"
        message.write_bytes(body)
        os.chmod(message, 0o600)
        signature = _openssl("pkeyutl", "-sign", "-rawin", "-inkey", str(_private_key(private_key)),
                             "-in", str(message))
    if len(signature) != 64:
        raise AuthorityError("Authority signature has an invalid length")
    return {"payload": payload, "signature_ed25519": signature.hex()}


def verify_record(record: Any, public_key: str | Path, *, expected_purpose: str) -> bool:
    if not isinstance(record, dict) or set(record) != {"payload", "signature_ed25519"}:
        return False
    payload, signature_hex = record["payload"], record["signature_ed25519"]
    if not isinstance(payload, dict) or payload.get("purpose") != expected_purpose or \
            not isinstance(signature_hex, str) or not re.fullmatch(r"[0-9a-f]{128}", signature_hex):
        return False
    try:
        key = _public_key(public_key)
        with tempfile.TemporaryDirectory(prefix="serea-authority-verify-") as temporary:
            sig = Path(temporary) / "signature.bin"
            message = Path(temporary) / "message.bin"
            sig.write_bytes(bytes.fromhex(signature_hex))
            message.write_bytes(canonical_json(payload))
            os.chmod(sig, 0o600)
            os.chmod(message, 0o600)
            result = subprocess.run(["openssl", "pkeyutl", "-verify", "-rawin", "-pubin", "-inkey", str(key),
                                     "-sigfile", str(sig), "-in", str(message)], stdout=subprocess.PIPE,
                                    stderr=subprocess.PIPE, check=False, timeout=15)
        return result.returncode == 0
    except (AuthorityError, OSError, subprocess.TimeoutExpired, ValueError):
        return False


def _validate_identities(identities: Any) -> dict[str, dict[str, Any]]:
    if not isinstance(identities, dict) or set(identities) != LANES:
        raise AuthorityError("Both Public and Private identities are required")
    result = {}
    for lane in sorted(LANES):
        item = identities[lane]
        if not isinstance(item, dict) or set(item) != {"application_id", "certificate_sha256", "initial_version_code"}:
            raise AuthorityError("Enrollment identity is malformed")
        code = item["initial_version_code"]
        if item["application_id"] != APP_IDS[lane] or type(code) is not int or \
                not MIN_BETA_VERSION_CODE <= code <= MAX_VERSION_CODE:
            raise AuthorityError("Enrollment lane identity is invalid")
        result[lane] = {"application_id": APP_IDS[lane],
                        "certificate_sha256": normalize_sha256(item["certificate_sha256"]),
                        "initial_version_code": code}
    if result["public"]["certificate_sha256"] == result["private"]["certificate_sha256"]:
        raise AuthorityError("Public and Private signing certificates must differ")
    return result


class EnrollmentAuthority:
    """Offline owner-held state and signing operations; never part of APK signing credentials."""

    def __init__(self, state_path: str | Path, private_key: str | Path, public_key: str | Path, *, mode: str):
        if mode not in {"disposable", "production"}:
            raise AuthorityError("Authority mode is invalid")
        self.file = SecureJsonFile(state_path)
        self.private_key = Path(private_key)
        self.public_key = Path(public_key)
        self.mode = mode

    @staticmethod
    def _validate_state(value: Any) -> dict[str, Any]:
        if not isinstance(value, dict) or set(value) != {
                "schema", "mode", "authority_id", "generation", "enrollment_slots", "enrollments", "lanes",
                "requests"} or value["schema"] != 1 or value["mode"] not in {"disposable", "production"}:
            raise AuthorityError("Authority state is missing, corrupt, or unsupported")
        _uuid(value["authority_id"])
        if type(value["generation"]) is not int or value["generation"] < 0 or \
                not isinstance(value["enrollment_slots"], dict) or not isinstance(value["enrollments"], dict) or \
                not isinstance(value["lanes"], dict) or not isinstance(value["requests"], dict):
            raise AuthorityError("Authority state fields are malformed")
        return value

    def create(self, *, mode: str) -> dict[str, Any]:
        if mode != self.mode:
            raise AuthorityError("Authority mode mismatch")
        value = {"schema": 1, "mode": mode, "authority_id": str(uuid.uuid4()), "generation": 0,
                 "enrollment_slots": {}, "enrollments": {}, "lanes": {}, "requests": {}}
        try:
            self.file.create(value)
        except StateFileError as exc:
            raise AuthorityError(str(exc)) from exc
        return value

    def read_state(self) -> dict[str, Any]:
        try:
            value = self._validate_state(self.file.read())
        except StateFileError as exc:
            raise AuthorityError(str(exc)) from exc
        if value["mode"] != self.mode:
            raise AuthorityError("Authority state mode mismatch")
        return value

    def _update(self, transform):
        try:
            value = self.file.update(lambda current: transform(self._validate_state(current)))
        except StateFileError as exc:
            raise AuthorityError(str(exc)) from exc
        return self._validate_state(value)

    def reserve_enrollment(self, enrollment_id: str) -> dict[str, Any]:
        enrollment_id = _uuid(enrollment_id)
        def update(state):
            if enrollment_id in state["enrollment_slots"] or enrollment_id in state["enrollments"]:
                raise AuthorityError("Enrollment ID is already reserved or consumed")
            generation = state["generation"] + 1
            slot_payload = {"purpose": "enrollment-slot", "authority_id": state["authority_id"],
                            "enrollment_id": enrollment_id, "mode": self.mode, "generation": generation}
            signed = _sign(slot_payload, self.private_key)
            slots = {**state["enrollment_slots"], enrollment_id: {"generation": generation, "record": signed}}
            return {**state, "generation": generation, "enrollment_slots": slots}
        state = self._update(update)
        return state["enrollment_slots"][enrollment_id]["record"]

    def enroll(self, enrollment_id: str, request_id: str, identities: dict[str, dict[str, Any]], *,
               ledger_id: str) -> dict[str, Any]:
        enrollment_id, request_id, ledger_id = _uuid(enrollment_id), _uuid(request_id), _uuid(ledger_id)
        identities = _validate_identities(identities)
        request_hash = hashlib.sha256(canonical_json({"enrollment_id": enrollment_id, "request_id": request_id,
                                                       "identities": identities, "ledger_id": ledger_id,
                                                       "mode": self.mode})).hexdigest()
        result = {}
        def update(state):
            previous = state["enrollments"].get(enrollment_id)
            if previous:
                if previous["request_hash"] != request_hash:
                    raise AuthorityError("Enrollment ID is already bound to another identity")
                result["record"] = previous["record"]
                return state
            slot = state["enrollment_slots"].get(enrollment_id)
            if not slot:
                raise AuthorityError("Enrollment ID was not pre-reserved by the Authority")
            if state["enrollments"]:
                raise AuthorityError("Authority lane identities are already enrolled; migration is not bootstrap")
            if any(item["ledger_id"] == ledger_id for item in state["enrollments"].values()):
                raise AuthorityError("Release Ledger ID is already enrolled")
            for lane in LANES:
                if any(item["identities"][lane]["certificate_sha256"] == identities[lane]["certificate_sha256"]
                       for item in state["enrollments"].values()):
                    raise AuthorityError("Signing certificate is already enrolled")
            generation = state["generation"] + 1
            payload = {"purpose": "enrollment", "authority_id": state["authority_id"],
                       "enrollment_id": enrollment_id, "request_id": request_id, "mode": self.mode,
                       "ledger_id": ledger_id, "generation": generation, "identities": identities}
            signed = _sign(payload, self.private_key)
            enrollments = {**state["enrollments"], enrollment_id: {"request_hash": request_hash,
                                                                       "ledger_id": ledger_id,
                                                                       "identities": identities,
                                                                       "record": signed}}
            lanes = {lane: {"enrollment_id": enrollment_id, "ledger_id": ledger_id,
                            "application_id": identities[lane]["application_id"],
                            "certificate_sha256": identities[lane]["certificate_sha256"],
                            "high_watermark": identities[lane]["initial_version_code"] - 1,
                            "initial_version_code": identities[lane]["initial_version_code"]} for lane in LANES}
            state = {**state, "generation": generation, "enrollments": enrollments, "lanes": lanes}
            result["record"] = signed
            return state
        self._update(update)
        return result["record"]

    def authorize(self, request: dict[str, Any]) -> dict[str, Any]:
        required = {"request_id", "nonce", "mode", "lane", "application_id", "certificate_sha256", "ledger_id",
                    "version_code", "attempt_id", "operation", "source_commit", "source_tree"}
        if not isinstance(request, dict) or set(request) != required:
            raise AuthorityError("Signing authorization request schema is invalid")
        request = dict(request)
        request["request_id"] = _uuid(request["request_id"])
        request["attempt_id"] = _uuid(request["attempt_id"])
        if request["lane"] not in LANES or request["mode"] != self.mode or request["operation"] != "sign-apk" or \
                not isinstance(request["nonce"], str) or not re.fullmatch(r"[0-9a-f]{64}", request["nonce"]):
            raise AuthorityError("Signing authorization request is invalid")
        lane = request["lane"]
        request["certificate_sha256"] = normalize_sha256(request["certificate_sha256"])
        if type(request["version_code"]) is not int or not MIN_BETA_VERSION_CODE <= request["version_code"] <= MAX_VERSION_CODE or \
                not re.fullmatch(r"[0-9a-f]{40}", str(request["source_commit"])) or \
                not re.fullmatch(r"[0-9a-f]{40}", str(request["source_tree"])):
            raise AuthorityError("Signing authorization request bounds are invalid")
        request_hash = hashlib.sha256(canonical_json(request)).hexdigest()
        result = {}
        def update(state):
            previous = state["requests"].get(request["request_id"])
            if previous:
                if previous["request_hash"] != request_hash:
                    raise AuthorityError("Request ID was already used for a different signing request")
                result["record"] = previous["record"]
                return state
            enrolled = next((entry for entry in state["enrollments"].values()
                             if entry["ledger_id"] == request["ledger_id"]), None)
            if enrolled is None or state["mode"] != request["mode"]:
                raise AuthorityError("Signing identity is not enrolled in this Authority")
            identity = enrolled["identities"][lane]
            if request["application_id"] != identity["application_id"] or \
                    request["certificate_sha256"] != identity["certificate_sha256"]:
                raise AuthorityError("Signing request identity does not match enrollment")
            lane_state = state["lanes"][lane]
            if lane_state["ledger_id"] != request["ledger_id"] or \
                    lane_state["high_watermark"] + 1 != request["version_code"]:
                raise AuthorityError("Signing request versionCode is stale, skipped, or already consumed")
            generation = state["generation"] + 1
            payload = {"purpose": "sign-authorization", "authority_id": state["authority_id"],
                       **request, "generation": generation}
            signed = _sign(payload, self.private_key)
            lane_state = {**lane_state, "high_watermark": request["version_code"]}
            lanes = {**state["lanes"], lane: lane_state}
            requests = {**state["requests"], request["request_id"]: {"request_hash": request_hash,
                                                                        "record": signed}}
            state = {**state, "generation": generation, "lanes": lanes, "requests": requests}
            result["record"] = signed
            return state
        self._update(update)
        return result["record"]
