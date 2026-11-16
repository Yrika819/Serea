#!/usr/bin/env python3
"""Owner-controlled Android signing vault CLI. Python standard library only."""
from __future__ import annotations

import argparse
import contextlib
import fcntl
import getpass
import hashlib
import json
import os
import plistlib
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from collections.abc import Iterator
from typing import Any

try:
    from .serea_release_ledger import LedgerError, ReleaseLedger, normalize_sha256
    from .serea_identity_registry import IdentityRegistry, RegistryError
    from .serea_signing_witness import SigningWitness, WitnessError
    from .serea_enrollment_authority import AuthorityError, verify_record
    from . import serea_apk_signer as apk_signer
    from .serea_apk_signer import ArtifactError, import_apk, inspect_unsigned_apk, validate_provenance, sign_apk
except ImportError:
    from serea_release_ledger import LedgerError, ReleaseLedger, normalize_sha256
    from serea_identity_registry import IdentityRegistry, RegistryError
    from serea_signing_witness import SigningWitness, WitnessError
    from serea_enrollment_authority import AuthorityError, verify_record
    import serea_apk_signer as apk_signer
    from serea_apk_signer import ArtifactError, import_apk, inspect_unsigned_apk, validate_provenance, sign_apk

APP_ROOT = Path(__file__).resolve().parents[1]
ANDROID_ROOT = APP_ROOT / "android"
FORMAL_ROOT = Path.home() / "Library" / "Application Support" / "Serea" / "Vault"
FORMAL_IMAGE_NAME = "Serea-Signing.dmg"
FORMAL_VOLUME = "SereaSigning"
DISPOSABLE_IMAGE_NAME = "Serea-Disposable-Signing.dmg"
DISPOSABLE_VOLUME = "SereaDisposable"
PRODUCTION_CONFIRMATION = "SEREA PRODUCTION SIGNING"
# Production stays fail-closed until an Owner-reviewed source change pins its offline Authority key.
TRUSTED_AUTHORITY_PUBLIC_KEY_SHA256: str | None = None
GRADLE_VARIABLES = {
    "public": {
        "path": "SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PATH",
        "store_password": "SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD",
        "alias": "SEREA_ANDROID_PERSONAL_BETA_KEY_ALIAS",
        "key_password": "SEREA_ANDROID_PERSONAL_BETA_KEY_PASSWORD",
        "fingerprint": "SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256",
    },
    "private": {
        "path": "SEREA_ANDROID_PRIVATE_BETA_KEYSTORE_PATH",
        "store_password": "SEREA_ANDROID_PRIVATE_BETA_KEYSTORE_PASSWORD",
        "alias": "SEREA_ANDROID_PRIVATE_BETA_KEY_ALIAS",
        "key_password": "SEREA_ANDROID_PRIVATE_BETA_KEY_PASSWORD",
        "fingerprint": "SEREA_ANDROID_PRIVATE_BETA_CERT_SHA256",
    },
}


class SigningError(RuntimeError):
    """A user-safe operation error; messages must never contain secret values."""


def _tool(name: str) -> str:
    from shutil import which
    value = which(name)
    if not value:
        raise SigningError(f"Required tool is unavailable: {name}")
    return value


def _run(args: list[str | Path], *, env: dict[str, str] | None = None,
         data: bytes | None = None, timeout: int = 120, check: bool = True,
         cwd: Path | None = None) -> subprocess.CompletedProcess[bytes]:
    try:
        result = subprocess.run([str(arg) for arg in args], input=data, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, env=env, timeout=timeout, check=False,
                                cwd=str(cwd) if cwd is not None else None)
    except subprocess.TimeoutExpired as exc:
        raise SigningError(f"{Path(str(args[0])).name} timed out; no retry or forced cleanup was attempted") from exc
    if check and result.returncode:
        # Tool output is deliberately suppressed; it can contain paths or environment diagnostics.
        raise SigningError(f"{Path(str(args[0])).name} failed (exit {result.returncode}); detailed output was suppressed")
    return result


def _canonical_no_symlinks(path: Path, *, allow_missing: bool = True) -> Path:
    expanded = Path(os.path.abspath(os.path.expanduser(str(path))))
    cursor = Path(expanded.anchor)
    for part in expanded.parts[1:]:
        cursor = cursor / part
        try:
            info = cursor.lstat()
        except FileNotFoundError:
            if not allow_missing:
                raise SigningError(f"Path does not exist: {cursor}")
            continue
        if stat.S_ISLNK(info.st_mode):
            raise SigningError(f"Symbolic links are not permitted in signing paths: {cursor}")
    return expanded


def _ensure_owner_directory(path: Path, *, create: bool) -> Path:
    path = _canonical_no_symlinks(path)
    private_targets = (path.parent, path)
    for target in private_targets:
        missing: list[Path] = []
        cursor = target
        while not cursor.exists():
            missing.append(cursor)
            cursor = cursor.parent
        if missing and not create:
            raise SigningError(f"Signing directory does not exist: {missing[-1]}")
        if cursor.is_symlink() or not cursor.is_dir() or cursor.stat().st_uid != os.getuid():
            raise SigningError("Signing directory ancestors must be Owner-owned directories")
        for item in reversed(missing):
            item.mkdir(mode=0o700)
            os.chmod(item, 0o700)
        info = target.stat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid():
            raise SigningError("Signing directories must be Owner-owned directories")
        if stat.S_IMODE(info.st_mode) != 0o700:
            raise SigningError("Signing directory permissions must be exactly 0700")
    return path


def _assert_disposable_root(root: Path, disposable: bool) -> Path:
    root = _canonical_no_symlinks(root)
    formal = _canonical_no_symlinks(FORMAL_ROOT)
    if disposable:
        if root == formal or formal in root.parents or root in formal.parents:
            raise SigningError("Disposable mode cannot target or contain the formal signing vault")
    elif root != formal:
        raise SigningError("Formal mode is restricted to the configured Serea Vault path")
    return root


def _confirm_production(disposable: bool, action: str) -> None:
    if disposable:
        return
    if not sys.stdin.isatty() or not sys.stderr.isatty():
        raise SigningError("Formal operations require an interactive terminal")
    answer = input(f"Formal signing action '{action}'. Type {PRODUCTION_CONFIRMATION!r} to continue: ")
    if answer != PRODUCTION_CONFIRMATION:
        raise SigningError("Owner confirmation did not match; no operation was performed")


def _secret(prompt: str) -> str:
    if not sys.stdin.isatty() or not sys.stderr.isatty():
        raise SigningError("Secret input requires an interactive TTY; refusing to read credentials noninteractively")
    value = getpass.getpass(prompt, stream=sys.stderr)
    if not value:
        raise SigningError("Empty passphrases are not accepted")
    return value


def _secure_env(extra: dict[str, str] | None = None) -> dict[str, str]:
    home = Path.home()
    jdk = home / "Library/Java/JavaVirtualMachines/temurin-17.jdk/Contents/Home"
    sdk = home / "Library/Android/sdk"
    if not (jdk / "bin/java").is_file():
        raise SigningError("Temurin JDK 17 is not installed at the configured path")
    env = {
        "HOME": str(home), "USER": os.environ.get("USER", "owner"),
        "TMPDIR": tempfile.gettempdir(), "JAVA_HOME": str(jdk),
        "ANDROID_HOME": str(sdk), "ANDROID_SDK_ROOT": str(sdk),
        "PATH": f"{jdk / 'bin'}:{sdk / 'build-tools/35.0.0'}:{sdk / 'platform-tools'}:/usr/bin:/bin",
        "LANG": "en_US.UTF-8", "LC_ALL": "en_US.UTF-8",
        "CI": "false", "SEREA_OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED": "false",
        "GRADLE_OPTS": "-Dorg.gradle.daemon=false -Dorg.gradle.caching=false",
    }
    if extra:
        env.update(extra)
    return env


@contextlib.contextmanager
def _isolated_gradle_user_home() -> Iterator[Path]:
    """Exclude user init scripts/properties while reusing offline dependency caches."""
    shared = Path.home() / ".gradle"
    with tempfile.TemporaryDirectory(prefix="serea-gradle-home-") as temporary:
        isolated = Path(temporary)
        for relative in (Path("caches"), Path("wrapper") / "dists"):
            source = shared / relative
            if not source.is_dir() or source.is_symlink():
                continue
            destination = isolated / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.symlink_to(source, target_is_directory=True)
        yield isolated


def _jdk_bin(name: str) -> Path:
    return Path.home() / "Library/Java/JavaVirtualMachines/temurin-17.jdk/Contents/Home/bin" / name


def _lock(root: Path) -> contextlib.AbstractContextManager[None]:
    @contextlib.contextmanager
    def held() -> Iterator[None]:
        checked_root = _ensure_owner_directory(root, create=False)
        lock_path = checked_root / ".serea-signing-cli.lock"
        fd = os.open(lock_path, os.O_CREAT | os.O_RDWR | getattr(os, "O_NOFOLLOW", 0), 0o600)
        try:
            lock_info = os.fstat(fd)
            if not stat.S_ISREG(lock_info.st_mode) or lock_info.st_uid != os.getuid() or lock_info.st_nlink != 1:
                raise SigningError("Unsafe signing lock file; refusing concurrent access")
            os.fchmod(fd, 0o600)
            try:
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError as exc:
                raise SigningError("Another Serea signing operation is already running") from exc
            yield
        finally:
            fcntl.flock(fd, fcntl.LOCK_UN)
            os.close(fd)
    return held()


@contextlib.contextmanager
def _build_lock() -> Iterator[None]:
    lock_path = _canonical_no_symlinks(Path.home() / "Library/Caches/.serea-signing-build.lock")
    parent = lock_path.parent
    if not parent.is_dir() or parent.stat().st_uid != os.getuid():
        raise SigningError("Owner cache directory is unavailable for the shared signing-build lock")
    fd = os.open(lock_path, os.O_CREAT | os.O_RDWR | getattr(os, "O_NOFOLLOW", 0), 0o600)
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_nlink != 1:
            raise SigningError("Unsafe shared signing-build lock file")
        os.fchmod(fd, 0o600)
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as exc:
            raise SigningError("Another Serea signing build is already running") from exc
        yield
    finally:
        fcntl.flock(fd, fcntl.LOCK_UN)
        os.close(fd)


def _fsync_directory(path: Path) -> None:
    fd = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _artifact_store(ledger_path: Path, ledger_id: str, *, create: bool = False) -> Path:
    store = ledger_path.parent / f".serea-artifacts-{ledger_id}"
    if store.exists() or store.is_symlink():
        info = store.lstat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
            raise SigningError("Managed signing artifact store is unsafe")
    elif create:
        store.mkdir(mode=0o700)
        _fsync_directory(store.parent)
    return store


def _write_publication_receipt(attempt_directory: Path, value: dict[str, Any]) -> None:
    info = attempt_directory.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
        raise SigningError("Publication attempt directory is unsafe")
    _atomic_json(attempt_directory / "receipt.json", value)


def _parse_publication_receipt(directory: Path, ledger_id: str) -> dict[str, Any]:
    info = directory.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != 0o700:
        raise SigningError("Managed signing attempt directory is unsafe")
    receipt_path = directory / "receipt.json"
    if receipt_path.is_symlink() or not receipt_path.is_file():
        raise SigningError("Managed signing attempt receipt is missing or unsafe")
    receipt_info = receipt_path.stat()
    if receipt_info.st_uid != os.getuid() or stat.S_IMODE(receipt_info.st_mode) != 0o600 or receipt_info.st_nlink != 1:
        raise SigningError("Managed signing attempt receipt permissions are unsafe")
    try:
        value = json.loads(receipt_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise SigningError("Managed signing attempt receipt is corrupt") from exc
    expected_keys = {"schema", "ledger_id", "attempt_id", "lane", "version_code", "state",
                     "input_apk_sha256", "input_provenance_sha256", "approved_source_commit",
                     "approved_source_tree", "artifact_path", "sidecar_path", "signed_apk_sha256",
                     "sidecar_sha256", "output_path"}
    valid_receipt = (
        isinstance(value, dict)
        and set(value) == expected_keys
        and type(value.get("schema")) is int
        and value["schema"] == 1
        and value.get("ledger_id") == ledger_id
        and value.get("attempt_id") == directory.name
        and isinstance(value.get("lane"), str)
        and value["lane"] in {"public", "private"}
        and type(value.get("version_code")) is int
        and value["version_code"] >= 2
        and isinstance(value.get("state"), str)
        and value["state"] in {"RESERVED", "ATTEMPTED", "VERIFIED", "BUILT", "PUBLISHED"}
        and isinstance(value.get("output_path"), str)
        and "\x00" not in value["output_path"]
        and Path(value["output_path"]).is_absolute()
        and value.get("artifact_path") == str(directory / "signed.apk")
        and value.get("sidecar_path") == str(directory / "result.json")
    )
    if not valid_receipt:
        raise SigningError("Managed signing attempt receipt does not match its path or schema")
    for field in ("input_apk_sha256", "input_provenance_sha256"):
        if not isinstance(value.get(field), str) or not re.fullmatch(r"[0-9a-f]{64}", value[field]):
            raise SigningError("Managed signing receipt input hash is malformed")
    for field in ("approved_source_commit", "approved_source_tree"):
        if not isinstance(value.get(field), str) or not re.fullmatch(r"[0-9a-f]{40}", value[field]):
            raise SigningError("Managed signing receipt source revision is malformed")
    for field in ("signed_apk_sha256", "sidecar_sha256"):
        digest = value.get(field)
        if digest is not None and (not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest)):
            raise SigningError("Managed signing receipt output hash is malformed")
    if value["state"] in {"VERIFIED", "BUILT", "PUBLISHED"} and value["signed_apk_sha256"] is None:
        raise SigningError("Verified publication receipt lacks the signed APK hash")
    if value["state"] in {"BUILT", "PUBLISHED"} and value["sidecar_sha256"] is None:
        raise SigningError("Built publication receipt lacks the result sidecar hash")
    return value


def _read_publication_receipts(ledger_path: Path, ledger_id: str, *,
                               anomalies: list[dict[str, str]] | None = None) -> list[dict[str, Any]]:
    store = _artifact_store(ledger_path, ledger_id)
    if not store.exists():
        return []
    receipts: list[dict[str, Any]] = []
    for directory in sorted(store.iterdir()):
        try:
            receipts.append(_parse_publication_receipt(directory, ledger_id))
        except SigningError as exc:
            if anomalies is None:
                raise
            anomalies.append({"attempt_id": directory.name, "issue": str(exc)})
    return receipts


def _read_fd_sha256(fd: int) -> str:
    """Hash all bytes readable from a descriptor; never re-opens by path."""
    digest = hashlib.sha256()
    while chunk := os.read(fd, 1024 * 1024):
        digest.update(chunk)
    return digest.hexdigest()


def _observed_file_sha256(path: Path) -> str | None:
    """Hash a regular file through one no-follow descriptor.

    The descriptor, not the path, is the object that is measured. A concurrent
    same-UID replacement of the path after the identity check cannot change the
    bytes that are hashed. Unsafe or unreadable paths report ``None`` so callers
    fail closed instead of treating them as verified.
    """
    try:
        canonical = _canonical_no_symlinks(path, allow_missing=False)
        fd = os.open(canonical, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) |
                     getattr(os, "O_CLOEXEC", 0))
    except (OSError, ValueError, SigningError):
        return None
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_nlink > 2:
            return None
        return _read_fd_sha256(fd)
    except OSError:
        return None
    finally:
        os.close(fd)


def _verify_published_artifact(path: Path, expected_sha256: str, *, label: str,
                               file_mode: int,
                               expected_identity: tuple[int, int] | None = None) -> None:
    """Re-open a published artifact by no-follow descriptor and verify it."""
    expected = normalize_sha256(expected_sha256)
    try:
        canonical = _canonical_no_symlinks(path, allow_missing=False)
        fd = os.open(canonical, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) |
                     getattr(os, "O_CLOEXEC", 0))
    except (OSError, ValueError, SigningError) as exc:
        raise SigningError(f"Published {label} is missing, unsafe, or a symbolic link") from exc
    try:
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_nlink > 2 or \
                stat.S_IMODE(info.st_mode) != file_mode:
            raise SigningError(f"Published {label} has unsafe file state")
        if expected_identity is not None and (info.st_dev, info.st_ino) != expected_identity:
            raise SigningError(f"Published {label} is not the verified artifact")
        if _read_fd_sha256(fd) != expected:
            raise SigningError(f"Published {label} hash differs from the verified artifact")
    finally:
        os.close(fd)


def _publish_verified_artifact(source: Path, destination: Path, expected_sha256: str, *,
                               label: str, file_mode: int) -> str:
    """Publish verified bytes without a verify-then-link path race.

    The source is opened once with ``O_NOFOLLOW`` and read through that single
    descriptor. Its identity (type, owner, link count, no setuid/setgid) and
    hash are checked on the descriptor before any bytes are staged. The verified
    bytes are written to a freshly created Owner-only staging inode, linked to
    the destination without overwrite, and the destination is re-opened by
    descriptor to confirm it is the same inode with the same bytes. Any
    mismatch stops the operation; callers must retain the attempt and must not
    mark it published.
    """
    expected = normalize_sha256(expected_sha256)
    try:
        canonical_source = _canonical_no_symlinks(source, allow_missing=False)
        source_fd = os.open(canonical_source, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) |
                            getattr(os, "O_CLOEXEC", 0))
    except (OSError, ValueError, SigningError) as exc:
        raise SigningError(f"Cannot open verified {label} for publication") from exc
    try:
        source_info = os.fstat(source_fd)
        if not stat.S_ISREG(source_info.st_mode) or source_info.st_uid != os.getuid() or \
                source_info.st_nlink > 2 or source_info.st_mode & (stat.S_ISUID | stat.S_ISGID):
            raise SigningError(f"Verified {label} has unsafe file state")
        digest = hashlib.sha256()
        verified_bytes = bytearray()
        while chunk := os.read(source_fd, 1024 * 1024):
            digest.update(chunk)
            verified_bytes.extend(chunk)
    except OSError as exc:
        raise SigningError(f"Cannot read verified {label} for publication") from exc
    finally:
        os.close(source_fd)
    if digest.hexdigest() != expected:
        raise SigningError(f"Verified {label} changed before publication; refusing to publish")
    if destination.exists() or destination.is_symlink():
        _verify_published_artifact(destination, expected, label=label, file_mode=file_mode)
        return expected

    staging_fd, staging_name = tempfile.mkstemp(prefix=".serea-publish-", dir=destination.parent)
    staging = Path(staging_name)
    try:
        view = memoryview(verified_bytes)
        written = 0
        while written < len(view):
            written += os.write(staging_fd, view[written:])
        os.fchmod(staging_fd, file_mode)
        os.fsync(staging_fd)
        staged_info = os.fstat(staging_fd)
        os.lseek(staging_fd, 0, os.SEEK_SET)
        if _read_fd_sha256(staging_fd) != expected:
            raise SigningError(f"Staged {label} failed byte verification; refusing publication")
        try:
            os.link(staging, destination, follow_symlinks=False)
            linked = True
        except FileExistsError:
            linked = False
        except OSError as exc:
            raise SigningError(f"Could not publish verified {label}; retain the attempt for recovery review") from exc
        if not linked:
            _verify_published_artifact(destination, expected, label=label, file_mode=file_mode)
            return expected
        _fsync_directory(destination.parent)
        _verify_published_artifact(destination, expected, label=label, file_mode=file_mode,
                                   expected_identity=(staged_info.st_dev, staged_info.st_ino))
    finally:
        os.close(staging_fd)
        with contextlib.suppress(FileNotFoundError):
            staging.unlink()
    return expected


def _publication_diagnostics(receipt: dict[str, Any], attempt_directory: Path,
                             event: dict[str, Any] | None) -> dict[str, Any]:
    expected_ledger_state = {"RESERVED": "RESERVED", "ATTEMPTED": "ATTEMPTED",
                             "VERIFIED": "ATTEMPTED", "BUILT": "BUILT", "PUBLISHED": "BUILT"}[receipt["state"]]
    ledger_matches = bool(event and (event["lane"], event["version_code"], event["state"]) ==
                          (receipt["lane"], receipt["version_code"], expected_ledger_state))
    signed_hash = _observed_file_sha256(attempt_directory / "signed.apk")
    sidecar_hash = _observed_file_sha256(attempt_directory / "result.json")
    managed_apk_matches = receipt["signed_apk_sha256"] is None or signed_hash == receipt["signed_apk_sha256"]
    managed_sidecar_matches = receipt["sidecar_sha256"] is None or sidecar_hash == receipt["sidecar_sha256"]
    if event and event["state"] == "BUILT":
        managed_apk_matches = managed_apk_matches and event["artifact_sha256"] == receipt["signed_apk_sha256"]
    final_apk_hash = final_sidecar_hash = None
    if receipt["state"] == "PUBLISHED":
        final_apk_hash = _observed_file_sha256(Path(receipt["output_path"]))
        final_sidecar_hash = _observed_file_sha256(Path(receipt["output_path"] + ".serea-result.json"))
        managed_apk_matches = managed_apk_matches and final_apk_hash == receipt["signed_apk_sha256"]
        managed_sidecar_matches = managed_sidecar_matches and final_sidecar_hash == receipt["sidecar_sha256"]
    unknown = not ledger_matches or not managed_apk_matches or not managed_sidecar_matches
    if receipt["state"] == "PUBLISHED" and (final_apk_hash is None or final_sidecar_hash is None):
        unknown = True
    return {"ledger_event_state": event["state"] if event else None,
            "ledger_state_matches_receipt": ledger_matches,
            "managed_apk_sha256": signed_hash, "managed_apk_matches_receipt": managed_apk_matches,
            "managed_sidecar_sha256": sidecar_hash, "managed_sidecar_matches_receipt": managed_sidecar_matches,
            "published_apk_sha256": final_apk_hash, "published_sidecar_sha256": final_sidecar_hash,
            "integrity": "UNKNOWN" if unknown else ("PUBLISHED" if receipt["state"] == "PUBLISHED" else "INCOMPLETE")}


def _assert_latest_publications_resolved(release: dict[str, Any], artifact_store: Path,
                                        receipts: list[dict[str, Any]]) -> None:
    for lane in ("public", "private"):
        latest = next((event for event in reversed(release["events"]) if event["lane"] == lane), None)
        if latest is None or latest["state"] != "BUILT":
            continue
        receipt = next((item for item in receipts if item["attempt_id"] == latest["attempt_id"]), None)
        if receipt is None:
            raise SigningError("Latest BUILT attempt has no complete managed publication receipt; recovery is required")
        diagnosis = _publication_diagnostics(receipt, artifact_store / receipt["attempt_id"], latest)
        if diagnosis["integrity"] != "PUBLISHED":
            raise SigningError("Latest BUILT attempt publication evidence is incomplete or mismatched; recovery is required")


def _atomic_json(path: Path, value: dict[str, Any]) -> None:
    if path.exists() or path.is_symlink():
        if path.is_symlink() or not path.is_file():
            raise SigningError(f"Refusing non-regular metadata path: {path.name}")
    fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temp_path = Path(temporary)
    try:
        os.fchmod(fd, 0o600)
        with os.fdopen(fd, "w", encoding="utf-8") as stream:
            json.dump(value, stream, indent=2, sort_keys=True)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        if path.exists():
            os.replace(temp_path, path)
        else:
            os.link(temp_path, path)
            temp_path.unlink()
        _fsync_directory(path.parent)
    finally:
        with contextlib.suppress(FileNotFoundError):
            temp_path.unlink()


def _load_json(path: Path) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise SigningError(f"Required metadata file is missing or unsafe: {path.name}")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise SigningError(f"Cannot read metadata file: {path.name}") from exc
    if not isinstance(value, dict):
        raise SigningError(f"Invalid metadata structure: {path.name}")
    return value


def _image_path(root: Path, disposable: bool) -> Path:
    return root / (DISPOSABLE_IMAGE_NAME if disposable else FORMAL_IMAGE_NAME)


def _attached_images() -> list[dict[str, Any]]:
    result = _run(["/usr/bin/hdiutil", "info", "-plist"], timeout=20)
    try:
        parsed = plistlib.loads(result.stdout)
    except plistlib.InvalidFileException as exc:
        raise SigningError("Cannot verify currently attached disk images") from exc
    if not isinstance(parsed, dict) or not isinstance(parsed.get("images"), list):
        raise SigningError("Cannot verify currently attached disk images")
    images = parsed["images"]
    if any(not isinstance(entry, dict) or not isinstance(entry.get("image-path"), str)
           or not os.path.isabs(entry["image-path"]) for entry in images):
        raise SigningError("Cannot verify currently attached disk images")
    return images


def _image_is_attached(image: Path) -> bool:
    target = str(image.resolve())
    return any(str(entry.get("image-path", "")) == target for entry in _attached_images())


def _canonical_mountpoint(path: str | Path) -> str:
    return os.path.realpath(path)


def _disk_info(mountpoint: Path) -> dict[str, Any]:
    result = _run(["/usr/sbin/diskutil", "info", "-plist", str(mountpoint)], timeout=20)
    try:
        parsed = plistlib.loads(result.stdout)
    except plistlib.InvalidFileException as exc:
        raise SigningError("Cannot verify mounted volume identity") from exc
    return parsed if isinstance(parsed, dict) else {}


@dataclass
class MountedVault:
    image: Path
    mountpoint: Path
    volume_name: str
    readonly: bool = False
    device: str | None = None
    attached: bool = False
    passphrase: str = ""
    temp_parent: Path | None = None

    def __enter__(self) -> Path:
        if _image_is_attached(self.image):
            raise SigningError("Signing image is already mounted; refusing ambiguous mount state")
        try:
            self.mountpoint.mkdir(mode=0o700)
            os.chmod(self.mountpoint, 0o700)
            args: list[str | Path] = ["/usr/bin/hdiutil", "attach", "-plist", "-stdinpass"]
            if self.readonly:
                args.append("-readonly")
            args.extend(["-mountpoint", self.mountpoint, self.image])
            self.passphrase = _secret("Vault passphrase: ")
            result = _run(args, data=(self.passphrase + "\0").encode(), timeout=120)
            attached_info = plistlib.loads(result.stdout)
            system_entities = attached_info.get("system-entities", [])
            expected_mountpoint = _canonical_mountpoint(self.mountpoint)
            matching = [item for item in system_entities
                        if item.get("mount-point") and
                        _canonical_mountpoint(str(item["mount-point"])) == expected_mountpoint]
            if len(matching) != 1 or not matching[0].get("dev-entry"):
                raise SigningError("Mounted volume did not match the requested mount point")
            self.device = str(matching[0]["dev-entry"])
            self.attached = True
            disk = _disk_info(self.mountpoint)
            if not disk.get("MountPoint") or \
                    _canonical_mountpoint(str(disk["MountPoint"])) != expected_mountpoint or \
                    disk.get("VolumeName") != self.volume_name:
                raise SigningError("Mounted volume identity did not match the expected Serea volume")
            image_entry = next((item for item in _attached_images()
                                if str(item.get("image-path", "")) == str(self.image.resolve())), None)
            if image_entry is None:
                raise SigningError("Mounted device is not backed by the requested disk image")
            return self.mountpoint
        except BaseException as exc:
            if not self.attached:
                try:
                    entry = next((item for item in _attached_images()
                                  if str(item.get("image-path", "")) == str(self.image.resolve())), None)
                    entities = entry.get("system-entities", []) if entry else []
                    expected_mountpoint = _canonical_mountpoint(self.mountpoint)
                    matching = next((item for item in entities
                                     if item.get("mount-point") and
                                     _canonical_mountpoint(str(item["mount-point"])) == expected_mountpoint
                                     and item.get("dev-entry")), None)
                    if matching:
                        self.device = str(matching["dev-entry"])
                        self.attached = True
                    elif entry:
                        print("WARNING: image is attached at an unverified mountpoint; leaving it untouched",
                              file=sys.stderr)
                except (SigningError, OSError, plistlib.InvalidFileException):
                    print("WARNING: could not confirm mount state after failure; inspect hdiutil info manually", file=sys.stderr)
            if self.attached:
                try:
                    self.detach()
                except SigningError:
                    print("WARNING: failed mount verification left the image mounted; manual unmount is required", file=sys.stderr)
            self.passphrase = ""
            if not self.attached:
                with contextlib.suppress(OSError):
                    self.mountpoint.rmdir()
                if self.temp_parent is not None:
                    with contextlib.suppress(OSError):
                        self.temp_parent.rmdir()
            if isinstance(exc, plistlib.InvalidFileException):
                raise SigningError("hdiutil returned invalid mount metadata") from exc
            raise

    def detach(self) -> None:
        if not self.attached or not self.device:
            return
        try:
            _run(["/usr/bin/hdiutil", "detach", self.device], timeout=120)
        except SigningError as exc:
            print(f"WARNING: unmount failed; do not delete or move the mounted image: {exc}", file=sys.stderr)
            raise
        self.attached = False
        self.passphrase = ""
        with contextlib.suppress(OSError):
            self.mountpoint.rmdir()

    def __exit__(self, exc_type: Any, exc: Any, tb: Any) -> bool:
        try:
            if self.attached:
                self.detach()
        except SigningError:
            if exc_type is None:
                raise
        finally:
            self.passphrase = ""
            if self.attached:
                print(f"WARNING: retained mountpoint for manual recovery: {self.mountpoint}", file=sys.stderr)
            elif self.temp_parent is not None:
                with contextlib.suppress(OSError):
                    self.temp_parent.rmdir()
        return False


def _mount_context(image: Path, volume_name: str, readonly: bool = False) -> MountedVault:
    parent = Path(tempfile.mkdtemp(prefix="serea-signing-mount-")).resolve(strict=True)
    parent = _canonical_no_symlinks(parent, allow_missing=False)
    os.chmod(parent, 0o700)
    point = parent / "volume"
    return MountedVault(image=image, mountpoint=point, volume_name=volume_name,
                        readonly=readonly, temp_parent=parent)


def _hdiutil_create(image: Path, volume: str, password: str) -> None:
    if image.exists() or image.is_symlink():
        raise SigningError("Refusing to overwrite an existing signing image")
    image.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    _ensure_owner_directory(image.parent, create=True)
    _run(["/usr/bin/hdiutil", "create", "-size", "256m", "-fs", "APFS", "-volname", volume,
          "-type", "UDIF", "-encryption", "AES-256", "-stdinpass", image],
         data=(password + "\0").encode(), timeout=300)
    os.chmod(image, 0o600)
    info = _run(["/usr/bin/hdiutil", "isencrypted", "-plist", image], timeout=30)
    try:
        encrypted = plistlib.loads(info.stdout).get("encrypted") is True
    except (plistlib.InvalidFileException, AttributeError) as exc:
        raise SigningError("Cannot verify disk image encryption state") from exc
    if not encrypted:
        raise SigningError("New disk image did not report encrypted")


def _aliases(disposable: bool) -> dict[str, str]:
    return ({"public": "r13-disposable-public", "private": "r13-disposable-private"} if disposable
            else {"public": "serea-public-beta", "private": "serea-private-beta"})


def _cert_meta(store: Path, alias: str, password: str) -> dict[str, str]:
    env = _secure_env({"SEREA_SIGNING_KEYTOOL_PASSWORD": password})
    command = [_jdk_bin("keytool"), "-J-Duser.language=en", "-J-Duser.country=US", "-list", "-v",
               "-storetype", "PKCS12", "-keystore", store, "-storepass:env", "SEREA_SIGNING_KEYTOOL_PASSWORD",
               "-alias", alias]
    result = _run(command, env=env, timeout=60)
    text = result.stdout.decode("utf-8", "replace")
    meta: dict[str, str] = {}
    for line in text.splitlines():
        item = line.strip()
        for label, key in (("SHA1:", "sha1"), ("SHA256:", "sha256"),
                           ("Signature algorithm name:", "signature_algorithm"),
                           ("Subject Public Key Algorithm:", "public_key"), ("Valid from:", "validity")):
            if item.startswith(label):
                meta[key] = item.split(":", 1)[1].strip().replace(":", "").lower() if key in ("sha1", "sha256") else item.split(":", 1)[1].strip()
    if len(meta.get("sha1", "")) != 40 or len(meta.get("sha256", "")) != 64:
        raise SigningError("Could not parse certificate fingerprints")
    if meta.get("signature_algorithm", "").lower() != "sha256withrsa":
        raise SigningError("Signing certificate is not SHA256withRSA")
    return meta


def _create_key(store: Path, alias: str, password: str, common_name: str) -> None:
    if store.exists() or store.is_symlink():
        raise SigningError(f"Refusing to replace existing key store: {store.name}")
    env = _secure_env({"SEREA_SIGNING_KEYSTORE_PASSWORD": password})
    _run([_jdk_bin("keytool"), "-J-Duser.language=en", "-J-Duser.country=US", "-genkeypair", "-noprompt",
          "-storetype", "PKCS12", "-keystore", store, "-storepass:env", "SEREA_SIGNING_KEYSTORE_PASSWORD",
          "-keypass:env", "SEREA_SIGNING_KEYSTORE_PASSWORD", "-alias", alias, "-keyalg", "RSA", "-keysize", "3072",
          "-sigalg", "SHA256withRSA", "-validity", "18250", "-dname", f"CN={common_name}"],
         env=env, timeout=120)
    os.chmod(store, 0o600)


def _export_cert(store: Path, alias: str, password: str, output: Path) -> None:
    env = _secure_env({"SEREA_SIGNING_KEYSTORE_PASSWORD": password})
    _run([_jdk_bin("keytool"), "-J-Duser.language=en", "-J-Duser.country=US", "-exportcert", "-rfc",
          "-storetype", "PKCS12", "-keystore", store, "-storepass:env", "SEREA_SIGNING_KEYSTORE_PASSWORD",
          "-alias", alias, "-file", output], env=env, timeout=60)
    os.chmod(output, 0o600)


def _public_key_digest(cert: Path) -> str:
    result = _run(["/usr/bin/openssl", "x509", "-in", cert, "-pubkey", "-noout"], timeout=30)
    return hashlib.sha256(result.stdout).hexdigest()


def _fingerprints_from_vault(mount: Path, manifest: dict[str, Any], passwords: dict[str, str]) -> dict[str, dict[str, str]]:
    identities = manifest.get("identities")
    if not isinstance(identities, dict):
        raise SigningError("Identity manifest is malformed")
    fingerprints: dict[str, dict[str, str]] = {}
    for lane in ("public", "private"):
        entry = identities.get(lane)
        if not isinstance(entry, dict):
            raise SigningError("Identity manifest is missing a signing lane")
        store = mount / str(entry.get("store", ""))
        alias = str(entry.get("alias", ""))
        if store.parent != mount or store.is_symlink() or not store.is_file():
            raise SigningError("PKCS12 path is missing or unsafe")
        meta = _cert_meta(store, alias, passwords[lane])
        if meta["sha256"] != str(entry.get("sha256", "")).lower() or meta["sha1"] != str(entry.get("sha1", "")).lower():
            raise SigningError("Signing certificate no longer matches the identity manifest")
        fingerprints[lane] = meta
    if fingerprints["public"]["sha256"] == fingerprints["private"]["sha256"]:
        raise SigningError("Public and Private signing certificates must differ")
    cert_dir = mount / "certificates"
    for lane in ("public", "private"):
        cert = cert_dir / f"{lane}-beta.pem"
        if cert.is_symlink() or not cert.is_file():
            raise SigningError("Public certificate file is missing or unsafe")
        digest = _public_key_digest(cert)
        if digest != identities[lane].get("public_key_sha256"):
            raise SigningError("Public certificate does not match the manifest public key")
    if identities["public"].get("public_key_sha256") == identities["private"].get("public_key_sha256"):
        raise SigningError("Public and Private public keys must differ")
    return fingerprints


def _manifest(mount: Path, disposable: bool, aliases: dict[str, str], passwords: dict[str, str],
              enrollment: dict[str, Any]) -> dict[str, Any]:
    cert_dir = mount / "certificates"
    cert_dir.mkdir(mode=0o700)
    entries: dict[str, Any] = {}
    for lane, app_id, file_name, subject in (
        ("public", "app.serea.android.beta", "public-beta.p12", "Serea Public Beta"),
        ("private", "app.serea.android.privatebeta", "private-beta.p12", "Serea Private Beta"),
    ):
        store = mount / file_name
        alias = aliases[lane]
        _create_key(store, alias, passwords[lane], subject)
        cert = cert_dir / f"{lane}-beta.pem"
        _export_cert(store, alias, passwords[lane], cert)
        meta = _cert_meta(store, alias, passwords[lane])
        entries[lane] = {
            "application_id": app_id, "alias": alias, "store": file_name,
            "sha1": meta["sha1"], "sha256": meta["sha256"],
            "public_key_sha256": _public_key_digest(cert),
            "key_algorithm": "RSA", "key_size": 3072,
            "signature_algorithm": "SHA256withRSA", "validity": meta["validity"],
        }
    if entries["public"]["sha256"] == entries["private"]["sha256"] or entries["public"]["public_key_sha256"] == entries["private"]["public_key_sha256"]:
        raise SigningError("Generated Public and Private identities are not distinct")
    now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    manifest = {"schema": 1, "mode": "disposable" if disposable else "production",
                "created_utc": now, "custodian": "Owner",
                "enrollment": {"authority_id": enrollment["authority_id"],
                               "enrollment_id": enrollment["enrollment_id"],
                               "generation": enrollment["generation"]},
                "identities": entries}
    _atomic_json(mount / "identity-manifest.json", manifest)
    ledger = {"schema": 1, "identities": {
        lane: {"application_id": entries[lane]["application_id"], "next_version_code": 2,
               "version_code_2": "reserved-unreleased", "artifacts": []}
        for lane in ("public", "private")
    }}
    _atomic_json(mount / "version-ledger.json", ledger)
    return manifest


def _load_manifest_from_mount(mount: Path, disposable: bool) -> dict[str, Any]:
    manifest = _load_json(mount / "identity-manifest.json")
    expected_mode = "disposable" if disposable else "production"
    enrollment = manifest.get("enrollment")
    if manifest.get("schema") != 1 or manifest.get("mode") != expected_mode or \
            not isinstance(enrollment, dict) or set(enrollment) != {"authority_id", "enrollment_id", "generation"} or \
            not isinstance(enrollment["authority_id"], str) or not isinstance(enrollment["enrollment_id"], str) or \
            type(enrollment["generation"]) is not int or enrollment["generation"] < 1:
        raise SigningError("Vault identity manifest lacks a valid pre-enrolled Authority slot")
    return manifest


def _ledger_add(mount: Path, lane: str, apk: Path, identity: dict[str, Any]) -> None:
    ledger_path = mount / "version-ledger.json"
    ledger = _load_json(ledger_path)
    current = ledger.get("identities", {}).get(lane)
    if not isinstance(current, dict):
        raise SigningError("Version ledger is malformed")
    version_code = int(identity["version_code"])
    expected = int(current.get("next_version_code", 2))
    if version_code + 1 != expected:
        raise SigningError("APK versionCode does not match the consumed Vault ledger value")
    current.setdefault("artifacts", []).append({
        "version_code": version_code, "version_name": identity["version_name"],
        "apk_sha256": _sha256(apk), "built_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    })
    _atomic_json(ledger_path, ledger)


def _verify_apk(apk: Path, lane: str, expected: dict[str, Any]) -> dict[str, str]:
    sdk = Path.home() / "Library/Android/sdk/build-tools/35.0.0"
    aapt = sdk / "aapt"
    apksigner = sdk / "apksigner"
    if not aapt.is_file() or not apksigner.is_file():
        raise SigningError("Android Build Tools 35.0.0 are required")
    badging = _run([aapt, "dump", "badging", apk], timeout=60).stdout.decode("utf-8", "replace")
    match = re.search(r"package: name='([^']+)' versionCode='([^']+)' versionName='([^']*)'", badging)
    if not match:
        raise SigningError("Cannot read APK identity")
    expected_id = "app.serea.android.beta" if lane == "public" else "app.serea.android.privatebeta"
    if match.group(1) != expected_id or match.group(2) != str(expected["version_code"]):
        raise SigningError("APK application ID or versionCode does not match the signing lane")
    if match.group(3) != str(expected.get("version_name", "0.1.0-demo")):
        raise SigningError("APK versionName does not match the expected Serea Beta identity")
    signature = _run([apksigner, "verify", "--print-certs", apk], timeout=60).stdout.decode("utf-8", "replace").lower()
    signer_blocks = re.findall(r"signer #\d+ certificate sha-256 digest:", signature)
    sha256_values = re.findall(r"certificate sha-256 digest:\s*([0-9a-f:]+)", signature)
    sha1_values = re.findall(r"certificate sha-1 digest:\s*([0-9a-f:]+)", signature)
    normalize = lambda value: value.replace(":", "")
    if len(signer_blocks) != 1 or len(sha256_values) != 1 or len(sha1_values) != 1 or \
            normalize(expected["sha1"]) != normalize(sha1_values[0]) or \
            normalize(expected["sha256"]) != normalize(sha256_values[0]):
        raise SigningError("APK must have exactly one signer matching the Vault certificate")
    return {"application_id": match.group(1), "version_code": match.group(2),
            "version_name": match.group(3), "apk_sha256": _sha256(apk)}


def _gradle(lane: str, mount: Path, manifest: dict[str, Any], password: str, *,
            allow_downloads: bool, version_code: int) -> Path:
    with _build_lock():
        return _gradle_locked(lane, mount, manifest, password, allow_downloads=allow_downloads,
                              version_code=version_code)


def _gradle_locked(lane: str, mount: Path, manifest: dict[str, Any], password: str, *,
                   allow_downloads: bool, version_code: int) -> Path:
    identity = manifest["identities"][lane]
    store = mount / identity["store"]
    env = _secure_env()
    env["SEREA_OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED"] = "false"
    if lane == "public":
        values = GRADLE_VARIABLES["public"]
        env.update({values["path"]: str(store), values["store_password"]: password,
                    values["alias"]: identity["alias"], values["key_password"]: password,
                    values["fingerprint"]: identity["sha256"]})
        task = ":app:assembleBeta"
        props = [f"-PsereaBetaVersionCode={version_code}"]
        variant = "beta"
    else:
        public = manifest["identities"]["public"]
        values = GRADLE_VARIABLES["private"]
        env.update({GRADLE_VARIABLES["public"]["fingerprint"]: public["sha256"],
                    values["path"]: str(store), values["store_password"]: password,
                    values["alias"]: identity["alias"], values["key_password"]: password,
                    values["fingerprint"]: identity["sha256"]})
        task = ":app:assemblePrivateBeta"
        props = [f"-PsereaPrivateBetaVersionCode={version_code}"]
        variant = "privateBeta"
    output = ANDROID_ROOT / "app/build/outputs/apk" / variant
    if output.exists():
        raise SigningError(f"Refusing to overwrite existing APK output directory: {output}")
    flags = ["--no-daemon", "--no-configuration-cache", "--no-build-cache", "--max-workers=2", "--console=plain"]
    if not allow_downloads:
        flags.append("--offline")
    try:
        with _isolated_gradle_user_home() as gradle_user_home:
            env["GRADLE_USER_HOME"] = str(gradle_user_home)
            result = _run([ANDROID_ROOT / "gradlew", *flags, *props, task], env=env, cwd=ANDROID_ROOT,
                          timeout=2400, check=False)
    except BaseException:
        if output.is_dir() and not output.is_symlink():
            shutil.rmtree(output)
        raise
    finally:
        for name in list(env):
            if "PASSWORD" in name or name.endswith("_KEY_PASSWORD"):
                env.pop(name, None)
    if result.returncode:
        if output.is_dir() and not output.is_symlink():
            shutil.rmtree(output)
        raise SigningError(f"Gradle {task} failed (exit {result.returncode}); full build output was suppressed")
    candidates = list(output.glob("*.apk")) if output.is_dir() else []
    if len(candidates) != 1:
        if output.is_dir() and not output.is_symlink():
            shutil.rmtree(output)
        raise SigningError("Gradle did not produce exactly one APK for the selected variant")
    return candidates[0]


def _mounted_vault(root: Path, disposable: bool, *, readonly: bool = False) -> MountedVault:
    return _mount_context(_image_path(root, disposable), DISPOSABLE_VOLUME if disposable else FORMAL_VOLUME, readonly)


def _root_for(args: argparse.Namespace, *, create: bool = False) -> Path:
    disposable = bool(getattr(args, "disposable", False))
    requested = getattr(args, "root", None)
    if disposable and not requested:
        raise SigningError("Disposable mode requires an explicit --root outside the formal vault")
    root = Path(requested).expanduser() if requested else FORMAL_ROOT
    root = _assert_disposable_root(root, disposable)
    return _ensure_owner_directory(root, create=create)


def _confirm_twice(prompt: str) -> str:
    first = _secret(prompt)
    second = _secret("Confirm passphrase: ")
    if first != second:
        first = second = ""
        raise SigningError("Passphrases did not match")
    second = ""
    return first


def _verify_one_lane(mount: Path, manifest: dict[str, Any], lane: str, password: str) -> dict[str, Any]:
    identities = manifest.get("identities", {})
    entry = identities.get(lane)
    other = identities.get("private" if lane == "public" else "public")
    if not isinstance(entry, dict) or not isinstance(other, dict):
        raise SigningError("Identity manifest is incomplete")
    store = mount / str(entry.get("store", ""))
    if store.parent != mount or store.is_symlink() or not store.is_file():
        raise SigningError("PKCS12 path is missing or unsafe")
    meta = _cert_meta(store, str(entry.get("alias", "")), password)
    if meta["sha1"] != entry.get("sha1") or meta["sha256"] != entry.get("sha256"):
        raise SigningError("Signing certificate does not match the manifest")
    cert_dir = mount / "certificates"
    cert = cert_dir / f"{lane}-beta.pem"
    other_cert = cert_dir / ("private-beta.pem" if lane == "public" else "public-beta.pem")
    for candidate in (cert, other_cert):
        if candidate.is_symlink() or not candidate.is_file():
            raise SigningError("Public certificate file is missing or unsafe")
    if _public_key_digest(cert) != entry.get("public_key_sha256"):
        raise SigningError("Certificate public key does not match manifest")
    if _public_key_digest(other_cert) != other.get("public_key_sha256"):
        raise SigningError("Other lane public certificate does not match manifest")
    if entry.get("sha256") == other.get("sha256") or entry.get("public_key_sha256") == other.get("public_key_sha256"):
        raise SigningError("Public and Private identities must differ")
    return meta


def _prompts_for_lane(lane: str) -> str:
    return _secret("Public PKCS12 passphrase: " if lane == "public" else "Private PKCS12 passphrase: ")


def cmd_doctor(args: argparse.Namespace) -> int:
    jdk = Path.home() / "Library/Java/JavaVirtualMachines/temurin-17.jdk/Contents/Home"
    checks = {
        "Python 3.9+": sys.version_info >= (3, 9),
        "hdiutil": Path("/usr/bin/hdiutil").is_file(),
        "diskutil": Path("/usr/sbin/diskutil").is_file(),
        "JDK 17": (jdk / "bin/java").is_file() and (jdk / "bin/keytool").is_file(),
        "Gradle wrapper": (ANDROID_ROOT / "gradlew").is_file(),
        "Android Build Tools 35.0.0": (Path.home() / "Library/Android/sdk/build-tools/35.0.0/apksigner").is_file(),
        "OpenSSL": Path("/usr/bin/openssl").is_file(),
    }
    for name, ok in checks.items():
        print(f"{name}: {'PASS' if ok else 'MISSING'}")
    try:
        status = _run(["/usr/bin/fdesetup", "status"], timeout=10, check=False).stdout.decode("utf-8", "replace").strip()
        print(f"FileVault: {status or 'UNKNOWN'} (residual host risk; this CLI does not change it)")
    except SigningError:
        print("FileVault: UNKNOWN (residual host risk)")
    print("Secret source: non-echoing interactive TTY; Keychain integration is not used")
    return 0 if all(checks.values()) else 1


def cmd_status(args: argparse.Namespace) -> int:
    root = _assert_disposable_root(Path(args.root).expanduser() if args.disposable and args.root else
                                   (Path(args.root).expanduser() if args.root else FORMAL_ROOT), args.disposable)
    if args.disposable and not args.root:
        raise SigningError("Disposable mode requires an explicit --root outside the formal vault")
    if root.exists():
        root = _ensure_owner_directory(root, create=False)
    image = _image_path(root, args.disposable)
    print(f"mode={'disposable' if args.disposable else 'formal'}")
    print(f"vault_directory={root}")
    print(f"image_exists={image.is_file() and not image.is_symlink()}")
    if image.exists() and not image.is_symlink():
        print(f"image_mode={stat.S_IMODE(image.stat().st_mode):04o}")
        print(f"mounted={_image_is_attached(image)}")
    if not args.disposable:
        print("filevault_status=RECHECK_WITH_doctor; known-off is residual risk")
    else:
        print("disposable_mode_only=true")
    return 0


def cmd_init(args: argparse.Namespace) -> int:
    root = _root_for(args, create=True)
    _confirm_production(args.disposable, "init encrypted vault")
    with _lock(root):
        image = _image_path(root, args.disposable)
        if image.exists() or image.is_symlink():
            raise SigningError("Vault image already exists; refusing replacement")
        password = _confirm_twice("Vault passphrase: ")
        try:
            _hdiutil_create(image, DISPOSABLE_VOLUME if args.disposable else FORMAL_VOLUME, password)
        finally:
            password = ""
    print("Encrypted vault created and left unmounted.")
    return 0


def cmd_provision(args: argparse.Namespace) -> int:
    root = _root_for(args)
    enrollment = _signed_record(args, "enrollment_slot_record", "enrollment-slot")
    if not isinstance(enrollment.get("enrollment_id"), str) or type(enrollment.get("generation")) is not int:
        raise SigningError("Authority enrollment slot is malformed")
    _confirm_production(args.disposable, "create identities within an Authority-reserved enrollment slot")
    aliases = _aliases(args.disposable)
    with _lock(root):
        vault = _mounted_vault(root, args.disposable)
        with vault as mount:
            if any((mount / name).exists() or (mount / name).is_symlink()
                   for name in ("public-beta.p12", "private-beta.p12", "identity-manifest.json", "version-ledger.json", "certificates")):
                raise SigningError("Vault already contains identity material; automatic regeneration is forbidden")
            public_password = _confirm_twice("Public PKCS12 passphrase: ")
            private_password = _confirm_twice("Private PKCS12 passphrase: ")
            if public_password == private_password:
                public_password = private_password = ""
                raise SigningError("Public and Private passphrases must be independent")
            if vault.passphrase in (public_password, private_password):
                public_password = private_password = ""
                raise SigningError("Vault and PKCS12 passphrases must be independent")
            try:
                _manifest(mount, args.disposable, aliases, {"public": public_password, "private": private_password},
                          enrollment)
            finally:
                public_password = private_password = ""
    print("Independent PKCS12 identities and nonsecret ledgers created.")
    return 0


def cmd_fingerprint(args: argparse.Namespace) -> int:
    root = _root_for(args)
    _confirm_production(args.disposable, "read certificate fingerprints")
    manifest: dict[str, Any] = {}
    fingerprints: dict[str, dict[str, str]] = {}
    with _lock(root), _mounted_vault(root, args.disposable) as mount:
        manifest = _load_manifest_from_mount(mount, args.disposable)
        passwords = {"public": _prompts_for_lane("public"), "private": _prompts_for_lane("private")}
        try:
            fingerprints = _fingerprints_from_vault(mount, manifest, passwords)
        finally:
            passwords = {"public": "", "private": ""}
    print(json.dumps({lane: {"application_id": manifest["identities"][lane]["application_id"],
                             "alias": manifest["identities"][lane]["alias"],
                             "sha1": fingerprints[lane]["sha1"], "sha256": fingerprints[lane]["sha256"]}
                      for lane in ("public", "private")}, indent=2, sort_keys=True))
    return 0


def _external_state_paths(args: argparse.Namespace, root: Path) -> tuple[Path, Path, Path]:
    ledger = ReleaseLedger(Path(args.release_ledger).expanduser()).path
    registry = IdentityRegistry(Path(args.identity_registry).expanduser()).path
    witness = SigningWitness(Path(args.witness).expanduser()).path
    paths = (ledger, registry, witness)
    parents = [path.parent for path in paths]
    if len(set(parents)) != 3 or len(set(paths)) != 3:
        raise SigningError("Release Ledger, Identity Registry and witness require distinct private parent directories")
    for path in paths:
        if not path.parent.is_dir() or path.parent.is_symlink() or \
                stat.S_IMODE(path.parent.stat().st_mode) != 0o700 or path.parent.stat().st_uid != os.getuid():
            raise SigningError("Each external signing-state parent must be Owner-owned mode 0700")
        if path == root or root in path.parents or path.parent in root.parents:
            raise SigningError("External signing state must be outside the Vault root and its parent")
    return paths


def _trusted_authority_key(args: argparse.Namespace) -> Path:
    if args.disposable:
        path_value = getattr(args, "authority_public_key", None)
        if not path_value:
            raise SigningError("Disposable operations require an explicit test Authority public key")
        path = _canonical_no_symlinks(Path(path_value), allow_missing=False)
    else:
        if getattr(args, "authority_public_key", None):
            raise SigningError("Production Authority keys cannot be selected with a CLI argument")
        if TRUSTED_AUTHORITY_PUBLIC_KEY_SHA256 is None:
            raise SigningError("No reviewed Production Authority trust anchor is configured")
        path = Path.home() / "Library" / "Application Support" / "Serea" / "Authority" / "authority-public.pem"
        path = _canonical_no_symlinks(path, allow_missing=False)
        if hashlib.sha256(path.read_bytes()).hexdigest() != TRUSTED_AUTHORITY_PUBLIC_KEY_SHA256:
            raise SigningError("Production Authority public key differs from the reviewed trust anchor")
    return path


def _signed_record(args: argparse.Namespace, attribute: str, purpose: str) -> dict[str, Any]:
    path_value = getattr(args, attribute, None)
    if not path_value:
        raise SigningError(f"A signed Authority {purpose} record is required")
    path = _canonical_no_symlinks(Path(path_value), allow_missing=False)
    value = _load_json(path)
    try:
        valid = verify_record(value, _trusted_authority_key(args), expected_purpose=purpose)
    except AuthorityError as exc:
        raise SigningError("Authority record verification failed") from exc
    if not valid:
        raise SigningError(f"Authority {purpose} signature is invalid")
    payload = value.get("payload")
    expected_mode = "disposable" if args.disposable else "production"
    if payload.get("mode") != expected_mode:
        raise SigningError("Authority record mode does not match this signing operation")
    return payload


def _remove_if_present(path: Path, label: str) -> None:
    try:
        path.lstat()
    except FileNotFoundError:
        return
    raise SigningError(f"{label} already exists; refusing reinitialization")


def _identity_inputs(manifest: dict[str, Any], fingerprints: dict[str, dict[str, str]], args: argparse.Namespace) -> dict[str, dict[str, Any]]:
    return {lane: {"application_id": manifest["identities"][lane]["application_id"],
                   "certificate_sha256": fingerprints[lane]["sha256"],
                   "initial_version_code": getattr(args, f"{lane}_version_code")}
            for lane in ("public", "private")}


def cmd_ledger_status(args: argparse.Namespace) -> int:
    root = _root_for(args)
    ledger_path, registry_path, witness_path = _external_state_paths(args, root)
    release = ReleaseLedger(ledger_path).read()
    registry = IdentityRegistry(registry_path).read()
    witness = SigningWitness(witness_path).read()
    summary = {"mode": release["mode"], "ledger_id": release["ledger_id"],
               "ledger_generation": release["generation"], "registry_id": registry["registry_id"],
               "registry_generation": registry["generation"], "witness_id": witness["witness_id"],
               "witness_generation": witness["generation"], "lanes": {}}
    for lane in ("public", "private"):
        lane_events = [event for event in release["events"] if event["lane"] == lane]
        summary["lanes"][lane] = {
            "application_id": release["lanes"][lane]["application_id"],
            "certificate_sha256": release["lanes"][lane]["certificate_sha256"],
            "next_version_code": release["lanes"][lane]["next_version_code"],
            "registry_high_watermark": registry["lanes"][lane]["high_watermark"],
            "witness_high_watermark": witness["lanes"][lane]["high_watermark"],
            "registry_last_attempt_id": registry["lanes"][lane]["last_attempt_id"],
            "witness_last_attempt_id": witness["lanes"][lane]["last_attempt_id"],
            "latest_state": lane_events[-1]["state"] if lane_events else None,
            "latest_attempt_id": lane_events[-1]["attempt_id"] if lane_events else None,
        }
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


def cmd_ledger_recovery_plan(args: argparse.Namespace) -> int:
    root = _root_for(args)
    ledger_path, registry_path, witness_path = _external_state_paths(args, root)
    release = ReleaseLedger(ledger_path).read()
    registry = IdentityRegistry(registry_path).read()
    witness = SigningWitness(witness_path).read()
    receipt_anomalies: list[dict[str, str]] = []
    raw_receipts = _read_publication_receipts(ledger_path, release["ledger_id"], anomalies=receipt_anomalies)
    events_by_attempt = {event["attempt_id"]: event for event in release["events"]}
    artifact_store = _artifact_store(ledger_path, release["ledger_id"])
    publication_receipts = []
    pending = []
    reported_attempts: set[str] = set()
    for receipt in raw_receipts:
        event = events_by_attempt.get(receipt["attempt_id"])
        attempt_directory = artifact_store / receipt["attempt_id"]
        diagnosis = _publication_diagnostics(receipt, attempt_directory, event)
        publication_receipts.append({**receipt, "diagnostics": diagnosis})
        if diagnosis["integrity"] != "PUBLISHED":
            lane_events = [item for item in release["events"] if item["lane"] == receipt["lane"]]
            lane_witness = [item for item in witness["events"] if item["lane"] == receipt["lane"]]
            latest_witness = lane_witness[-1] if lane_witness else None
            output = ANDROID_ROOT / "app/build/outputs/apk" / (
                "beta" if receipt["lane"] == "public" else "privateBeta")
            candidates = sorted(path.name for path in output.glob("*.apk")) if output.is_dir() and not output.is_symlink() else []
            pending.append({"lane": receipt["lane"], "attempt_id": receipt["attempt_id"],
                            "version_code": receipt["version_code"], "state": event["state"] if event else "UNKNOWN",
                            "registry_high_watermark": registry["lanes"][receipt["lane"]]["high_watermark"],
                            "witness_high_watermark": witness["lanes"][receipt["lane"]]["high_watermark"],
                            "witness_attempt_present": bool(latest_witness and latest_witness["attempt_id"] == receipt["attempt_id"]),
                            "apk_candidates": candidates, "publication_receipt": publication_receipts[-1],
                            "integrity": diagnosis["integrity"],
                            "automatic_action": "none; preserve managed attempt and require exact Owner review"})
        reported_attempts.add(receipt["attempt_id"])
    for anomaly in receipt_anomalies:
        event = events_by_attempt.get(anomaly["attempt_id"])
        lane = event["lane"] if event else None
        lane_events = [item for item in witness["events"] if item["lane"] == lane] if lane else []
        witness_latest = lane_events[-1] if lane_events else None
        diagnosis = {"integrity": "UNKNOWN", "issue": anomaly["issue"],
                     "ledger_state_matches_receipt": False}
        summary = {"attempt_id": anomaly["attempt_id"], "diagnostics": diagnosis}
        publication_receipts.append(summary)
        pending.append({"lane": lane, "attempt_id": anomaly["attempt_id"],
                        "version_code": event["version_code"] if event else None,
                        "state": event["state"] if event else "UNKNOWN",
                        "registry_high_watermark": registry["lanes"][lane]["high_watermark"] if lane else None,
                        "witness_high_watermark": witness["lanes"][lane]["high_watermark"] if lane else None,
                        "witness_attempt_present": bool(witness_latest and witness_latest["attempt_id"] == anomaly["attempt_id"]),
                        "apk_candidates": [], "publication_receipt": summary,
                        "integrity": "UNKNOWN",
                        "automatic_action": "none; attempt directory or receipt is incomplete; Owner review required"})
        reported_attempts.add(anomaly["attempt_id"])
    for lane in ("public", "private"):
        lane_events = [event for event in release["events"] if event["lane"] == lane]
        latest = lane_events[-1] if lane_events else None
        witness_events = [event for event in witness["events"] if event["lane"] == lane]
        witness_latest = witness_events[-1] if witness_events else None
        ledger_pending = latest is not None and latest["state"] in {"RESERVED", "ATTEMPTED"}
        witness_only = witness_latest is not None and (latest is None or latest["attempt_id"] != witness_latest["attempt_id"])
        if ledger_pending or witness_only:
            attempt = latest if ledger_pending else witness_latest
            if attempt is not None and attempt["attempt_id"] not in reported_attempts:
                output = ANDROID_ROOT / "app/build/outputs/apk" / ("beta" if lane == "public" else "privateBeta")
                candidates = sorted(path.name for path in output.glob("*.apk")) if output.is_dir() and not output.is_symlink() else []
                pending.append({"lane": lane, "attempt_id": attempt["attempt_id"], "version_code": attempt["version_code"],
                                "state": latest["state"] if ledger_pending else "WITNESS_ONLY_OR_STORE_MISMATCH",
                                "registry_high_watermark": registry["lanes"][lane]["high_watermark"],
                                "witness_high_watermark": witness["lanes"][lane]["high_watermark"],
                                "witness_attempt_present": bool(witness_latest and witness_latest["attempt_id"] == attempt["attempt_id"]),
                                "apk_candidates": candidates, "publication_receipt": None,
                                "integrity": "UNKNOWN",
                                "automatic_action": "none; Owner confirmation and exact store reconciliation required"})
                reported_attempts.add(attempt["attempt_id"])
        if latest is not None and latest["state"] == "BUILT" and latest["attempt_id"] not in reported_attempts:
            missing_receipt = {"attempt_id": latest["attempt_id"], "diagnostics": {
                "integrity": "UNKNOWN", "issue": "latest BUILT event has no managed publication receipt"}}
            publication_receipts.append(missing_receipt)
            pending.append({"lane": lane, "attempt_id": latest["attempt_id"], "version_code": latest["version_code"],
                            "state": "BUILT_WITHOUT_RECEIPT", "registry_high_watermark": registry["lanes"][lane]["high_watermark"],
                            "witness_high_watermark": witness["lanes"][lane]["high_watermark"],
                            "witness_attempt_present": bool(witness_latest and witness_latest["attempt_id"] == latest["attempt_id"]),
                            "apk_candidates": [], "publication_receipt": missing_receipt,
                            "integrity": "UNKNOWN",
                            "automatic_action": "none; legacy or missing managed evidence requires Owner reconciliation"})
            reported_attempts.add(latest["attempt_id"])
    print(json.dumps({"ledger_id": release["ledger_id"], "generation": release["generation"],
                      "pending": pending, "publication_receipts": publication_receipts,
                      "artifact_store": str(artifact_store), "read_only": True}, indent=2, sort_keys=True))
    return 0


def _assert_recovery_artifact_history(vault_ledger: dict[str, Any], release: dict[str, Any], *,
                                      lane: str, event_state: str, version_code: int,
                                      apk_sha256: str, version_name: str) -> bool:
    current_record_present = False
    for current_lane in ("public", "private"):
        internal = vault_ledger["identities"][current_lane]["artifacts"]
        built = [event for event in release["events"]
                 if event["lane"] == current_lane and event["state"] == "BUILT"]
        expected_count = len(built)
        if current_lane == lane and event_state == "ATTEMPTED":
            if len(internal) == expected_count + 1:
                candidate = internal[-1]
                if candidate.get("version_code") != version_code or \
                        candidate.get("version_name") != version_name or \
                        normalize_sha256(str(candidate.get("apk_sha256", ""))) != apk_sha256:
                    raise SigningError("Vault contains a conflicting artifact for the recoverable attempt")
                current_record_present = True
            elif len(internal) != expected_count:
                raise SigningError("Vault artifact history cannot be reconciled with the recoverable attempt")
        elif len(internal) != expected_count:
            raise SigningError("Vault artifact history does not match Release Ledger BUILT records")
        for artifact, event in zip(internal[:expected_count], built):
            if artifact.get("version_code") != event["version_code"] or \
                    artifact.get("version_name") != event["version_name"] or \
                    normalize_sha256(str(artifact.get("apk_sha256", ""))) != event["artifact_sha256"]:
                raise SigningError("Vault artifact history does not match Release Ledger BUILT records")
    return current_record_present


def _recovery_link_no_replace(source: Path, destination: Path, expected_sha256: str, *, label: str) -> None:
    """Recovery publication entry point backed by descriptor-bound verification.

    The helper re-reads the source through one no-follow descriptor, stages the
    exact verified bytes, links without overwrite, and re-verifies the published
    inode and hash. A conflict stops recovery and preserves the attempt for
    Owner review; it is never treated as a successful publication.
    """
    file_mode = 0o400 if "APK" in label else 0o600
    _publish_verified_artifact(source, destination, expected_sha256, label=label, file_mode=file_mode)


def cmd_ledger_recover_publish(args: argparse.Namespace) -> int:
    if not args.disposable:
        raise SigningError("Publication recovery is enabled only for explicitly Disposable state")
    root = _root_for(args)
    ledger_path, registry_path, witness_path = _external_state_paths(args, root)
    release_ledger = ReleaseLedger(ledger_path)
    registry = IdentityRegistry(registry_path)
    witness = SigningWitness(witness_path)
    password = ""
    try:
        with _lock(root), _mounted_vault(root, True) as mount:
            release = release_ledger.read(expected_mode="disposable")
            receipts = _read_publication_receipts(ledger_path, release["ledger_id"])
            receipt = next((item for item in receipts if item["attempt_id"] == args.attempt_id), None)
            if receipt is None:
                raise SigningError("Managed publication receipt is missing; recovery state is UNKNOWN")
            if (receipt["lane"], receipt["version_code"]) != (args.lane, args.version_code) or \
                    args.confirm_publish != f"PUBLISH {args.lane} {args.attempt_id} {args.version_code}":
                raise SigningError("Recovery confirmation must exactly name lane, attempt ID and versionCode")
            lane_events = [event for event in release["events"] if event["lane"] == args.lane]
            event = next((item for item in reversed(lane_events) if item["attempt_id"] == args.attempt_id), None)
            if event is None or not lane_events or lane_events[-1]["attempt_id"] != args.attempt_id or \
                    event["version_code"] != args.version_code or event["state"] not in {"ATTEMPTED", "BUILT"}:
                raise SigningError("Release Ledger does not contain the exact latest recoverable attempt")
            signed_apk_sha256 = receipt["signed_apk_sha256"]
            if receipt["state"] not in {"VERIFIED", "BUILT", "PUBLISHED"} or \
                    not isinstance(signed_apk_sha256, str):
                raise SigningError("Receipt lacks verified signing evidence; state is UNKNOWN")
            if event["state"] == "BUILT" and (event["artifact_sha256"] != signed_apk_sha256 or
                                                event["version_name"] is None):
                raise SigningError("BUILT Ledger event does not match the publication receipt")
            if any((latest := next((item for item in reversed(release["events"])
                                    if item["lane"] == other_lane), None)) is not None and
                   latest["state"] in {"RESERVED", "ATTEMPTED"}
                   for other_lane in ("public", "private") if other_lane != args.lane):
                raise SigningError("Other signing lane has an unfinished attempt; Recovery is blocked")

            manifest = _load_manifest_from_mount(mount, True)
            vault_ledger = _validate_internal_release_ledger(mount)
            identities = {lane: {"application_id": release["lanes"][lane]["application_id"],
                                 "certificate_sha256": release["lanes"][lane]["certificate_sha256"],
                                 "initial_version_code": release["lanes"][lane]["initial_version_code"]}
                          for lane in ("public", "private")}
            if vault_ledger.get("release_ledger_id") != release["ledger_id"] or \
                    vault_ledger.get("release_ledger_path") != str(ledger_path) or \
                    vault_ledger.get("identity_registry_path") != str(registry_path) or \
                    vault_ledger.get("signing_witness_path") != str(witness_path):
                raise SigningError("Vault and independent signing-state bindings disagree; recovery is blocked")
            registry_state = registry.assert_binding(mode="disposable", ledger_id=release["ledger_id"],
                                                     identities=identities)
            witness_state = witness.assert_binding(mode="disposable", ledger_id=release["ledger_id"],
                                                   identities=identities)
            registry_state = registry.assert_matches_witness(witness_state)
            if registry_state["witness_id"] != witness_state["witness_id"]:
                raise SigningError("Registry and Witness identity bindings disagree")
            for lane in ("public", "private"):
                latest = next((item for item in reversed(release["events"]) if item["lane"] == lane), None)
                expected_high = release["lanes"][lane]["next_version_code"] - 1
                expected_attempt = latest["attempt_id"] if latest else None
                if vault_ledger["identities"][lane]["next_version_code"] != release["lanes"][lane]["next_version_code"] or \
                        registry_state["lanes"][lane]["high_watermark"] != expected_high or \
                        witness_state["lanes"][lane]["high_watermark"] != expected_high or \
                        registry_state["lanes"][lane]["last_attempt_id"] != expected_attempt or \
                        witness_state["lanes"][lane]["last_attempt_id"] != expected_attempt:
                    raise SigningError("Vault, Ledger, Registry and Witness high-water state disagrees")
            if manifest["identities"][args.lane]["application_id"] != identities[args.lane]["application_id"] or \
                    normalize_sha256(manifest["identities"][args.lane]["sha256"]) != identities[args.lane]["certificate_sha256"]:
                raise SigningError("Vault signer identity differs from the recoverable lane")
            if receipt["output_path"] != str(_canonical_no_symlinks(Path(receipt["output_path"]).expanduser())):
                raise SigningError("Receipt output path is not canonical")
            output = Path(receipt["output_path"])
            if not output.parent.is_dir() or output.parent.is_symlink() or \
                    output.parent.stat().st_uid != os.getuid() or stat.S_IMODE(output.parent.stat().st_mode) != 0o700:
                raise SigningError("Recovery output parent must be an existing Owner-owned mode 0700 directory")
            attempt_directory = _artifact_store(ledger_path, release["ledger_id"]) / args.attempt_id
            managed_apk = attempt_directory / "signed.apk"
            result_stage = attempt_directory / "result.json"
            if attempt_directory.stat().st_dev != output.parent.stat().st_dev:
                raise SigningError("Managed attempt and requested output are on different filesystems")
            password = _prompts_for_lane(args.lane)
            identity = _verify_one_lane(mount, manifest, args.lane, password)
            apk_info = managed_apk.lstat()
            if not stat.S_ISREG(apk_info.st_mode) or apk_info.st_uid != os.getuid() or \
                    stat.S_IMODE(apk_info.st_mode) != 0o400 or apk_info.st_nlink > 2:
                raise SigningError("Managed signed APK permissions or link count are unsafe")
            apk_hash = _observed_file_sha256(managed_apk)
            if not isinstance(apk_hash, str) or apk_hash != signed_apk_sha256:
                raise SigningError("Managed signed APK does not match its publication receipt")
            apk_app_id, apk_version_code, apk_version_name = apk_signer._apk_identity(
                managed_apk, Path.home() / "Library/Android/sdk/build-tools/35.0.0/aapt")
            if apk_app_id != identities[args.lane]["application_id"] or apk_version_code != args.version_code:
                raise SigningError("Managed APK identity does not match the recoverable lane and versionCode")
            if event["state"] == "BUILT" and event["version_name"] != apk_version_name:
                raise SigningError("Managed APK versionName does not match the durable BUILT Ledger event")
            apk_evidence: dict[str, Any] = _verify_apk(managed_apk, args.lane,
                                       {"version_code": args.version_code,
                                        "version_name": apk_version_name,
                                        "sha1": identity["sha1"], "sha256": identity["sha256"]})
            apk_evidence["version_code"] = apk_version_code
            cert_sha256 = identity["sha256"].lower().replace(":", "")
            signer_hashes = [apk_signer._verified_signer(managed_apk, Path.home() /
                              "Library/Android/sdk/build-tools/35.0.0/apksigner", sdk=sdk)
                             for sdk in (26, 35)]
            if signer_hashes != [cert_sha256, cert_sha256]:
                raise SigningError("Managed signed APK does not verify with the lane certificate on API 26 and 35")
            apk_evidence.update({"signer_count": 1, "certificate_sha256": cert_sha256,
                                 "verified_api_levels": [26, 35], "size_bytes": apk_info.st_size,
                                 "source_sha256": receipt["input_apk_sha256"]})
            if event["state"] == "BUILT" and (event["artifact_sha256"] != apk_hash or
                                                event["version_name"] != apk_evidence["version_name"]):
                raise SigningError("Managed APK does not match the durable BUILT Ledger event")
            artifact_present = _assert_recovery_artifact_history(
                vault_ledger, release, lane=args.lane, event_state=event["state"],
                version_code=args.version_code, apk_sha256=apk_hash,
                version_name=apk_evidence["version_name"])

            result = {"schema": 1, "result": "PASS", "mode": "disposable", "lane": args.lane,
                      "attempt_id": args.attempt_id, "ledger_id": release["ledger_id"],
                      "evidence": apk_evidence}
            result_path = output.with_name(output.name + ".serea-result.json")
            staged_hash = _observed_file_sha256(result_stage)
            if result_stage.exists() or result_stage.is_symlink():
                info = result_stage.lstat()
                if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or \
                        stat.S_IMODE(info.st_mode) != 0o600 or info.st_nlink > 2:
                    raise SigningError("Managed result sidecar permissions or link count are unsafe")
                if not isinstance(staged_hash, str) or \
                        json.loads(result_stage.read_text(encoding="utf-8")) != result:
                    raise SigningError("Managed result sidecar does not match the verified APK")
                if receipt["sidecar_sha256"] is not None and staged_hash != receipt["sidecar_sha256"]:
                    raise SigningError("Managed result sidecar does not match its publication receipt")
            else:
                if receipt["sidecar_sha256"] is not None or receipt["state"] != "VERIFIED":
                    raise SigningError("Receipt claims a result sidecar that is missing; state is UNKNOWN")
                _atomic_json(result_stage, result)
                staged_hash = _sha256(result_stage)
            if not isinstance(staged_hash, str) or (receipt["sidecar_sha256"] is not None and
                                                       staged_hash != receipt["sidecar_sha256"]):
                raise SigningError("Managed result sidecar does not match its publication receipt")
            for destination, expected_hash, label in ((output, apk_hash, "final APK"),
                                                       (result_path, str(staged_hash), "published sidecar")):
                if destination.exists() or destination.is_symlink():
                    if _observed_file_sha256(destination) != expected_hash:
                        raise SigningError(f"Existing {label} conflicts with the verified attempt; refusing overwrite")

            if event["state"] == "ATTEMPTED":
                if not artifact_present:
                    _ledger_add(mount, args.lane, managed_apk,
                                {"version_code": args.version_code, "version_name": apk_evidence["version_name"]})
                    vault_ledger = _validate_internal_release_ledger(mount)
                receipt["sidecar_sha256"] = str(staged_hash)
                receipt["state"] = "BUILT"
                _write_publication_receipt(attempt_directory, receipt)
                release_ledger.mark_built(args.lane, args.version_code, args.attempt_id,
                                          artifact_sha256=apk_hash, version_name=apk_evidence["version_name"])
            elif receipt["state"] != "PUBLISHED":
                receipt["sidecar_sha256"] = str(staged_hash)
                receipt["state"] = "BUILT"
                _write_publication_receipt(attempt_directory, receipt)
            updated_release = release_ledger.read(expected_mode="disposable")
            try:
                release_ledger.assert_matches_vault(vault_ledger, manifest, expected_mode="disposable")
            except LedgerError as exc:
                raise SigningError("Recovery could not reconcile Vault artifacts with the completed Ledger") from exc
            completed_event = next((item for item in updated_release["events"]
                                    if item["attempt_id"] == args.attempt_id and item["state"] == "BUILT"), None)
            if completed_event is None or completed_event["artifact_sha256"] != apk_hash:
                raise SigningError("Recovery did not establish the exact BUILT Ledger event")
            _recovery_link_no_replace(result_stage, result_path, str(staged_hash), label="result sidecar")
            _recovery_link_no_replace(managed_apk, output, apk_hash, label="final APK")
            receipt["state"] = "PUBLISHED"
            _write_publication_receipt(attempt_directory, receipt)
            diagnosis = _publication_diagnostics(receipt, attempt_directory, completed_event)
            if diagnosis["integrity"] != "PUBLISHED":
                raise SigningError("Recovery publication did not pass final receipt verification")
            print(json.dumps({"result": "PUBLISHED", "lane": args.lane, "version_code": args.version_code,
                              "attempt_id": args.attempt_id, "apk_sha256": apk_hash,
                              "sidecar_sha256": staged_hash, "verified_api_levels": [26, 35]},
                             indent=2, sort_keys=True))
            return 0
        raise SigningError("Recovery could not complete publication")
    finally:
        password = ""


def cmd_ledger_abandon(args: argparse.Namespace) -> int:
    root = _root_for(args)
    _confirm_production(args.disposable, "reconcile and abandon a consumed release reservation")
    ledger_path, registry_path, witness_path = _external_state_paths(args, root)
    if args.confirm != f"ABANDON {args.attempt_id} {args.version_code}":
        raise SigningError("Recovery confirmation must exactly name the attempt ID and versionCode")
    release_preview = ReleaseLedger(ledger_path).read(expected_mode="disposable" if args.disposable else "production")
    receipts = _read_publication_receipts(ledger_path, release_preview["ledger_id"])
    if any(receipt["attempt_id"] == args.attempt_id for receipt in receipts):
        raise SigningError("Managed publication receipt exists; inspect its artifact and hashes before recovery")
    output = ANDROID_ROOT / "app/build/outputs/apk" / ("beta" if args.lane == "public" else "privateBeta")
    if output.is_dir() and not output.is_symlink() and any(output.glob("*.apk")):
        raise SigningError("APK candidate exists; inspect signer, hash, app ID, versionCode and provenance before recovery")
    release_ledger = ReleaseLedger(ledger_path)
    registry = IdentityRegistry(registry_path)
    witness_store = SigningWitness(witness_path)
    mode = "disposable" if args.disposable else "production"
    with _lock(root), _mounted_vault(root, args.disposable) as mount:
        locked_release = release_ledger.read(expected_mode=mode)
        locked_receipts = _read_publication_receipts(ledger_path, locked_release["ledger_id"])
        if any(receipt["attempt_id"] == args.attempt_id for receipt in locked_receipts):
            raise SigningError("Managed publication receipt appeared during recovery; inspect its artifact and hashes")
        manifest = _load_manifest_from_mount(mount, args.disposable)
        internal = _validate_internal_release_ledger(mount)
        release = release_ledger.read(expected_mode=mode)
        identities = {lane: {"application_id": release["lanes"][lane]["application_id"],
                             "certificate_sha256": release["lanes"][lane]["certificate_sha256"],
                             "initial_version_code": release["lanes"][lane]["initial_version_code"]}
                      for lane in ("public", "private")}
        registry_state = registry.assert_binding(mode=mode, ledger_id=release["ledger_id"], identities=identities)
        witness_state = witness_store.assert_binding(mode=mode, ledger_id=release["ledger_id"], identities=identities)
        if registry_state["witness_id"] != witness_state["witness_id"] or \
                internal.get("release_ledger_id") != release["ledger_id"] or \
                internal.get("release_ledger_path") != str(ledger_path) or \
                internal.get("identity_registry_path") != str(registry_path) or \
                internal.get("signing_witness_path") != str(witness_path):
            raise SigningError("Vault, Ledger, Registry and witness binding mismatch; automatic recovery is forbidden")
        for other_lane in ("public", "private"):
            if other_lane == args.lane:
                continue
            other_events = [event for event in release["events"] if event["lane"] == other_lane]
            other_attempt = other_events[-1]["attempt_id"] if other_events else None
            other_high = release["lanes"][other_lane]["next_version_code"] - 1
            if registry_state["lanes"][other_lane]["high_watermark"] != other_high or \
                    witness_state["lanes"][other_lane]["high_watermark"] != other_high or \
                    registry_state["lanes"][other_lane]["last_attempt_id"] != other_attempt or \
                    witness_state["lanes"][other_lane]["last_attempt_id"] != other_attempt or \
                    internal["identities"][other_lane]["next_version_code"] != release["lanes"][other_lane]["next_version_code"]:
                raise SigningError("Other lane has inconsistent signing state; recovery is blocked")
        if manifest["identities"][args.lane]["application_id"] != identities[args.lane]["application_id"] or \
                normalize_sha256(manifest["identities"][args.lane]["sha256"]) != identities[args.lane]["certificate_sha256"]:
            raise SigningError("Recovery signer identity does not match the bound lane")
        witnessed = [event for event in witness_state["events"] if event["attempt_id"] == args.attempt_id]
        if not witnessed or witnessed[-1]["lane"] != args.lane or witnessed[-1]["version_code"] != args.version_code:
            raise SigningError("Independent witness has no exact durable reservation for this attempt")
        lane_registry = registry_state["lanes"][args.lane]
        if lane_registry["high_watermark"] == args.version_code - 1:
            registry_state = registry.consume(lane=args.lane, version_code=args.version_code,
                                              attempt_id=args.attempt_id, expected_generation=registry_state["generation"])
            lane_registry = registry_state["lanes"][args.lane]
        if lane_registry["high_watermark"] != args.version_code or lane_registry["last_attempt_id"] != args.attempt_id or \
                witness_state["lanes"][args.lane]["high_watermark"] != args.version_code or \
                witness_state["lanes"][args.lane]["last_attempt_id"] != args.attempt_id:
            raise SigningError("Registry and witness do not agree on the consumed recovery attempt")
        registry_state = registry.assert_matches_witness(witness_state)
        lane_events = [event for event in release["events"] if event["lane"] == args.lane]
        latest = lane_events[-1] if lane_events else None
        if latest and latest["attempt_id"] == args.attempt_id and latest["version_code"] == args.version_code and \
                latest["state"] == "ABANDONED":
            print("Reservation is already ABANDONED; no state changed.")
            return 0
        lane_release = release["lanes"][args.lane]
        if lane_release["next_version_code"] == args.version_code:
            if latest and latest["state"] in {"RESERVED", "ATTEMPTED"}:
                raise SigningError("Ledger counter contradicts its unfinished event")
            release_ledger.reserve(args.lane, expected_generation=release["generation"], attempt_id=args.attempt_id)
            release = release_ledger.read(expected_mode=mode)
            lane_events = [event for event in release["events"] if event["lane"] == args.lane]
            latest = lane_events[-1]
        lane_release = release["lanes"][args.lane]
        if not latest or latest["attempt_id"] != args.attempt_id or latest["version_code"] != args.version_code or \
                latest["state"] not in {"RESERVED", "ATTEMPTED"} or lane_release["next_version_code"] != args.version_code + 1:
            raise SigningError("Ledger does not contain the exact recoverable reservation")
        current = internal["identities"][args.lane]["next_version_code"]
        if current not in {args.version_code, args.version_code + 1}:
            raise SigningError("Vault counter is outside the safe recovery window")
        if current == args.version_code:
            internal["identities"][args.lane]["next_version_code"] = args.version_code + 1
            _atomic_json(mount / "version-ledger.json", internal)
        release_ledger.abandon(args.lane, args.version_code, args.attempt_id, reason=args.reason)
    print(f"Reservation {args.attempt_id} versionCode={args.version_code} recorded ABANDONED; code remains consumed.")
    return 0


def cmd_witness_init(args: argparse.Namespace) -> int:
    root = _root_for(args)
    enrollment = _signed_record(args, "enrollment_record", "enrollment")
    _confirm_production(args.disposable, "bind a pre-authorized enrollment to local signing state")
    ledger_path, registry_path, witness_path = _external_state_paths(args, root)
    _remove_if_present(ledger_path, "Release Ledger")
    _remove_if_present(registry_path, "Identity Registry")
    _remove_if_present(witness_path, "Signing witness")
    record: dict[str, Any] | None = None
    with _lock(root), _mounted_vault(root, args.disposable) as mount:
        manifest = _load_manifest_from_mount(mount, args.disposable)
        internal = _validate_internal_release_ledger(mount)
        if internal.get("release_ledger_id") or internal.get("release_ledger_path") or \
                internal.get("identity_registry_path") or internal.get("signing_witness_path"):
            raise SigningError("Vault already has an external signing-state binding; witness bootstrap is forbidden")
        if any(internal["identities"][lane]["artifacts"] for lane in ("public", "private")):
            raise SigningError("Witness bootstrap is forbidden after any Vault artifact history")
        if any(getattr(args, f"{lane}_version_code") < internal["identities"][lane]["next_version_code"]
               for lane in ("public", "private")):
            raise SigningError("Witness baseline may not lower a Vault counter")
        passwords = {lane: _prompts_for_lane(lane) for lane in ("public", "private")}
        try:
            fingerprints = _fingerprints_from_vault(mount, manifest, passwords)
            inputs = _identity_inputs(manifest, fingerprints, args)
            mode = "disposable" if args.disposable else "production"
            slot = manifest["enrollment"]
            if enrollment.get("mode") != mode or enrollment.get("identities") != inputs or \
                    not isinstance(enrollment.get("ledger_id"), str) or \
                    enrollment.get("enrollment_id") != slot["enrollment_id"] or \
                    enrollment.get("authority_id") != slot["authority_id"]:
                raise SigningError("Signed enrollment does not match the Vault identity and mode")
            record = SigningWitness(witness_path).initialize(
                mode=mode, ledger_id=enrollment["ledger_id"], identities=inputs, confirm_new=True)
        finally:
            passwords = {"public": "", "private": ""}
    if record is None:
        raise SigningError("Signing witness initialization did not complete")
    print("Explicit signing witness initialized. Preserve its independent backup; absence is not first enrollment.")
    print(f"witness_id={record['witness_id']} ledger_id={record['ledger_id']}")
    return 0


def cmd_release_ledger_init(args: argparse.Namespace) -> int:
    root = _root_for(args)
    enrollment = _signed_record(args, "enrollment_record", "enrollment")
    _confirm_production(args.disposable, "bind Vault, Identity Registry and Release Ledger to signed enrollment")
    ledger_path, registry_path, witness_path = _external_state_paths(args, root)
    _remove_if_present(ledger_path, "Release Ledger")
    _remove_if_present(registry_path, "Identity Registry")
    witness = SigningWitness(witness_path).read()
    release: dict[str, Any] | None = None
    registry: dict[str, Any] | None = None
    with _lock(root), _mounted_vault(root, args.disposable) as mount:
        manifest = _load_manifest_from_mount(mount, args.disposable)
        vault_ledger = _validate_internal_release_ledger(mount)
        if vault_ledger.get("release_ledger_id") or vault_ledger.get("release_ledger_path") or \
                vault_ledger.get("identity_registry_path") or vault_ledger.get("signing_witness_path"):
            raise SigningError("Vault is already bound to external signing state; reinitialization is forbidden")
        if any(vault_ledger["identities"][lane]["artifacts"] for lane in ("public", "private")):
            raise SigningError("Cannot initialize external signing state over existing artifact history")
        if any(getattr(args, f"{lane}_version_code") < vault_ledger["identities"][lane]["next_version_code"]
               for lane in ("public", "private")):
            raise SigningError("External signing state may not lower a consumed Vault counter")
        passwords = {lane: _prompts_for_lane(lane) for lane in ("public", "private")}
        try:
            fingerprints = _fingerprints_from_vault(mount, manifest, passwords)
            inputs = _identity_inputs(manifest, fingerprints, args)
            mode = "disposable" if args.disposable else "production"
            slot = manifest["enrollment"]
            if enrollment.get("mode") != mode or enrollment.get("identities") != inputs or \
                    enrollment.get("ledger_id") != witness["ledger_id"] or \
                    enrollment.get("enrollment_id") != slot["enrollment_id"] or \
                    enrollment.get("authority_id") != slot["authority_id"]:
                raise SigningError("Signed enrollment does not match the Vault identity, mode or witness Ledger ID")
            SigningWitness(witness_path).assert_binding(mode=mode, ledger_id=witness["ledger_id"], identities=inputs)
            for lane in ("public", "private"):
                if witness["lanes"][lane]["high_watermark"] != inputs[lane]["initial_version_code"] - 1:
                    raise SigningError("Witness has already consumed a version; bootstrap cannot reset its high-water mark")
            release = ReleaseLedger.initialize(ledger_path, mode=mode, identities=inputs,
                                               ledger_id=witness["ledger_id"])
            registry = IdentityRegistry(registry_path).initialize(mode=mode, ledger_id=release["ledger_id"],
                                                                  identities=inputs, witness=witness)
            for lane in ("public", "private"):
                vault_ledger["identities"][lane]["next_version_code"] = release["lanes"][lane]["next_version_code"]
            vault_ledger["release_ledger_id"] = release["ledger_id"]
            vault_ledger["release_ledger_path"] = str(ledger_path)
            vault_ledger["identity_registry_path"] = str(registry_path)
            vault_ledger["signing_witness_path"] = str(witness_path)
            _atomic_json(mount / "version-ledger.json", vault_ledger)
        finally:
            passwords = {"public": "", "private": ""}
    if release is None or registry is None:
        raise SigningError("External signing-state initialization did not complete")
    print(f"Identity Registry and independent Release Ledger bound in {release['mode']} mode.")
    print(f"ledger_id={release['ledger_id']} registry_id={registry['registry_id']} generation={release['generation']}")
    return 0


def _source_revisions(*, require_clean: bool = False) -> tuple[str, str]:
    try:
        commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=APP_ROOT, check=True,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=10).stdout.strip()
        tree = subprocess.run(["git", "rev-parse", "HEAD^{tree}"], cwd=APP_ROOT, check=True,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=10).stdout.strip()
        if require_clean:
            status = subprocess.run(["git", "status", "--porcelain", "--untracked-files=all"], cwd=APP_ROOT,
                                    check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                                    timeout=10).stdout
            if status:
                raise SigningError("Production signing requires a clean source worktree")
    except (OSError, subprocess.SubprocessError) as exc:
        raise SigningError("Cannot establish signing source provenance") from exc
    if not re.fullmatch(r"[0-9a-f]{40}", commit) or not re.fullmatch(r"[0-9a-f]{40}", tree):
        raise SigningError("Signing source revision is malformed")
    return commit, tree


def cmd_authorization_request(args: argparse.Namespace) -> int:
    root = _root_for(args)
    _confirm_production(args.disposable, "prepare a one-use Authority signing request")
    ledger_path, registry_path, witness_path = _external_state_paths(args, root)
    mode = "disposable" if args.disposable else "production"
    release = ReleaseLedger(ledger_path).read(expected_mode=mode)
    registry = IdentityRegistry(registry_path).read()
    witness = SigningWitness(witness_path).read()
    lane = args.lane
    if registry["ledger_id"] != release["ledger_id"] or witness["ledger_id"] != release["ledger_id"] or \
            registry["witness_id"] != witness["witness_id"]:
        raise SigningError("Local enrollment stores disagree; cannot prepare Authority request")
    identity = release["lanes"][lane]
    commit, tree = _source_revisions(require_clean=not args.disposable)
    request = {"request_id": str(uuid.uuid4()), "nonce": os.urandom(32).hex(), "mode": mode, "lane": lane,
               "application_id": identity["application_id"], "certificate_sha256": identity["certificate_sha256"],
               "ledger_id": release["ledger_id"], "version_code": identity["next_version_code"],
               "attempt_id": str(uuid.uuid4()), "operation": "sign-apk", "source_commit": commit,
               "source_tree": tree}
    print(json.dumps(request, sort_keys=True, indent=2))
    return 0


def _reconcile_signing_stores(*, release_ledger: ReleaseLedger, registry: IdentityRegistry,
                              witness: SigningWitness, vault_ledger: dict[str, Any],
                              manifest: dict[str, Any], mode: str) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    try:
        release_state = release_ledger.assert_matches_vault(vault_ledger, manifest, expected_mode=mode)
    except LedgerError as exc:
        raise SigningError("Signing state is not eligible for a new version allocation") from exc
    identities = {name: {"application_id": release_state["lanes"][name]["application_id"],
                         "certificate_sha256": release_state["lanes"][name]["certificate_sha256"],
                         "initial_version_code": release_state["lanes"][name]["initial_version_code"]}
                  for name in ("public", "private")}
    registry_state = registry.assert_binding(mode=mode, ledger_id=release_state["ledger_id"], identities=identities)
    witness_state = witness.assert_binding(mode=mode, ledger_id=release_state["ledger_id"], identities=identities)
    registry_state = registry.assert_matches_witness(witness_state)
    for name in ("public", "private"):
        latest = next((event for event in reversed(release_state["events"]) if event["lane"] == name), None)
        expected_attempt = latest["attempt_id"] if latest is not None else None
        expected_high = release_state["lanes"][name]["next_version_code"] - 1
        if registry_state["lanes"][name]["high_watermark"] != expected_high or \
                witness_state["lanes"][name]["high_watermark"] != expected_high or \
                registry_state["lanes"][name]["last_attempt_id"] != expected_attempt:
            raise SigningError("Vault, Release Ledger, Registry and Witness high-water state disagrees")
    return release_state, registry_state, witness_state


def _reserve_signing_code(*, lane: str, release_ledger: ReleaseLedger, registry: IdentityRegistry,
                          witness: SigningWitness, vault_ledger: dict[str, Any], manifest: dict[str, Any],
                          mode: str) -> dict[str, Any]:
    if lane not in {"public", "private"}:
        raise SigningError("Unknown signing lane")
    release_state, registry_state, witness_state = _reconcile_signing_stores(
        release_ledger=release_ledger, registry=registry, witness=witness,
        vault_ledger=vault_ledger, manifest=manifest, mode=mode)
    version_code = release_state["lanes"][lane]["next_version_code"]
    attempt_id = str(uuid.uuid4())
    try:
        witness_state = witness.consume(lane=lane, version_code=version_code, attempt_id=attempt_id,
                                        expected_generation=witness_state["generation"])
        registry.consume(lane=lane, version_code=version_code, attempt_id=attempt_id,
                         expected_generation=registry_state["generation"])
        reservation = release_ledger.reserve(lane, expected_generation=release_state["generation"],
                                             attempt_id=attempt_id)
    except (LedgerError, RegistryError, WitnessError) as exc:
        raise SigningError("Version allocation became uncertain; stop and reconcile all stores") from exc
    if reservation["version_code"] != version_code:
        raise SigningError("Release Ledger allocated an unexpected versionCode")
    lane_ledger = vault_ledger.get("identities", {}).get(lane)
    if not isinstance(lane_ledger, dict) or lane_ledger.get("next_version_code") != version_code:
        raise SigningError("Vault version counter changed during allocation")
    lane_ledger["next_version_code"] = version_code + 1
    return reservation


def cmd_sign_apk(args: argparse.Namespace) -> int:
    if not args.disposable:
        raise SigningError("Formal APK signing is disabled; only disposable sign-apk operations are enabled")
    root = _root_for(args)
    output = _canonical_no_symlinks(Path(args.output).expanduser())
    if not output.parent.is_dir() or output.parent.stat().st_uid != os.getuid() or \
            stat.S_IMODE(output.parent.stat().st_mode) != 0o700:
        raise SigningError("Signed APK output parent must be an existing Owner-owned mode 0700 directory")
    if output.exists() or output.is_symlink():
        raise SigningError("Signed APK output already exists; refusing overwrite")
    result_path = output.with_name(output.name + ".serea-result.json")
    if result_path.exists() or result_path.is_symlink():
        raise SigningError("Signing result record already exists; refusing overwrite")
    lane = args.lane
    ledger_path, registry_path, witness_path = _external_state_paths(args, root)
    release_ledger = ReleaseLedger(ledger_path)
    registry = IdentityRegistry(registry_path)
    witness = SigningWitness(witness_path)
    password = ""
    try:
        with _lock(root), _mounted_vault(root, True) as mount:
            manifest = _load_manifest_from_mount(mount, True)
            vault_ledger = _validate_internal_release_ledger(mount)
            if vault_ledger.get("release_ledger_path") != str(ledger_path) or \
                    vault_ledger.get("identity_registry_path") != str(registry_path) or \
                    vault_ledger.get("signing_witness_path") != str(witness_path):
                raise SigningError("Vault is not bound to the supplied independent signing stores")
            release_state, _, _ = _reconcile_signing_stores(
                release_ledger=release_ledger, registry=registry, witness=witness,
                vault_ledger=vault_ledger, manifest=manifest, mode="disposable")
            artifact_store = _artifact_store(ledger_path, release_state["ledger_id"])
            existing_receipts = _read_publication_receipts(ledger_path, release_state["ledger_id"])
            _assert_latest_publications_resolved(release_state, artifact_store, existing_receipts)
            version_code = release_state["lanes"][lane]["next_version_code"]
            artifact_store = _artifact_store(ledger_path, release_state["ledger_id"], create=True)
            if artifact_store.stat().st_dev != output.parent.stat().st_dev:
                raise SigningError("Managed artifact store and requested output must be on the same filesystem")
            with tempfile.TemporaryDirectory(prefix="serea-apk-import-", dir=output.parent) as staging_name:
                staging = Path(staging_name)
                os.chmod(staging, 0o700)
                staged = import_apk(args.apk, staging)
                inspection = inspect_unsigned_apk(staged)
                apk_signer._assert_no_apk_signing_block(staged)
                sdk = Path.home() / "Library/Android/sdk/build-tools/35.0.0"
                existing_signature = apk_signer._tool_run([
                    sdk / "apksigner", "verify", "--min-sdk-version", "26", "--max-sdk-version", "35", staged])
                if existing_signature.returncode == 0:
                    raise SigningError("Input APK is already signed")
                app_id, actual_code, version_name = apk_signer._apk_identity(
                    staged, Path.home() / "Library/Android/sdk/build-tools/35.0.0/aapt")
                if app_id != apk_signer.APP_IDS[lane] or actual_code != version_code:
                    raise SigningError("Unsigned APK identity/version does not match the next Release Ledger code")
                _, provenance_hash = validate_provenance(
                    args.provenance, lane=lane, application_id=app_id, version_code=actual_code,
                    version_name=version_name, apk_sha256=inspection.sha256,
                    approved_apk_sha256=args.approved_apk_sha256,
                    approved_provenance_sha256=args.approved_provenance_sha256,
                    approved_source_commit=args.approved_source_commit,
                    approved_source_tree=args.approved_source_tree, return_sha256=True)
                password = _prompts_for_lane(lane)
                identity = _verify_one_lane(mount, manifest, lane, password)
                reservation = _reserve_signing_code(
                    lane=lane, release_ledger=release_ledger, registry=registry, witness=witness,
                    vault_ledger=vault_ledger, manifest=manifest, mode="disposable")
                if reservation["version_code"] != actual_code:
                    raise SigningError("APK versionCode differs from the durable reservation")
                attempt_directory = artifact_store / reservation["attempt_id"]
                attempt_directory.mkdir(mode=0o700)
                _fsync_directory(artifact_store)

                receipt = {"schema": 1, "ledger_id": release_state["ledger_id"],
                           "attempt_id": reservation["attempt_id"], "lane": lane,
                           "version_code": actual_code, "state": "RESERVED",
                           "input_apk_sha256": inspection.sha256,
                           "input_provenance_sha256": provenance_hash,
                           "approved_source_commit": args.approved_source_commit,
                           "approved_source_tree": args.approved_source_tree,
                           "artifact_path": str(attempt_directory / "signed.apk"),
                           "sidecar_path": str(attempt_directory / "result.json"),
                           "signed_apk_sha256": None, "sidecar_sha256": None,
                           "output_path": str(output)}
                _write_publication_receipt(attempt_directory, receipt)
                _atomic_json(mount / "version-ledger.json", vault_ledger)
                release_ledger.mark_attempted(lane, actual_code, reservation["attempt_id"])
                receipt["state"] = "ATTEMPTED"
                _write_publication_receipt(attempt_directory, receipt)
                staged_output = attempt_directory / "signed.apk"
                evidence = sign_apk(
                    staged, staged_output, lane=lane, version_code=actual_code, version_name=version_name,
                    keystore=mount / manifest["identities"][lane]["store"],
                    alias=manifest["identities"][lane]["alias"], password=password,
                    aapt=sdk / "aapt", zipalign=sdk / "zipalign", apksigner=sdk / "apksigner",
                    expected_certificate_sha256=identity["sha256"].lower().replace(":", ""), mode="disposable")
                receipt["state"] = "VERIFIED"
                receipt["signed_apk_sha256"] = evidence["apk_sha256"]
                _write_publication_receipt(attempt_directory, receipt)
                _ledger_add(mount, lane, staged_output, {"version_code": actual_code, "version_name": version_name})
                release_ledger.mark_built(lane, actual_code, reservation["attempt_id"],
                                          artifact_sha256=evidence["apk_sha256"], version_name=version_name)
                result = {"schema": 1, "result": "PASS", "mode": "disposable",
                          "lane": lane, "attempt_id": reservation["attempt_id"],
                          "ledger_id": release_state["ledger_id"], "evidence": evidence}
                sidecar_stage = attempt_directory / "result.json"
                _atomic_json(sidecar_stage, result)
                receipt["state"] = "BUILT"
                receipt["sidecar_sha256"] = _sha256(sidecar_stage)
                _write_publication_receipt(attempt_directory, receipt)
                try:
                    _publish_verified_artifact(sidecar_stage, result_path, receipt["sidecar_sha256"],
                                               label="result sidecar", file_mode=0o600)
                except SigningError as exc:
                    raise SigningError("BUILT artifact and receipt are retained; sidecar publication requires recovery review") from exc
                try:
                    _publish_verified_artifact(staged_output, output, evidence["apk_sha256"],
                                               label="final APK", file_mode=0o400)
                except SigningError as exc:
                    raise SigningError("Sidecar is published; APK commit requires recovery review") from exc
                receipt["state"] = "PUBLISHED"
                _write_publication_receipt(attempt_directory, receipt)
                final_release = release_ledger.read(expected_mode="disposable")
                final_event = next((item for item in final_release["events"]
                                    if item["attempt_id"] == reservation["attempt_id"] and
                                    item["state"] == "BUILT"), None)
                if _publication_diagnostics(receipt, attempt_directory, final_event)["integrity"] != "PUBLISHED":
                    raise SigningError("Publication did not pass final receipt integrity verification")
                print(json.dumps(evidence, sort_keys=True, indent=2))
                print("Disposable APK signing and verification passed; no distribution was performed.")
                return 0
    finally:
        password = ""


def cmd_build(args: argparse.Namespace, lane: str) -> int:
    del args, lane
    raise SigningError("Gradle-based signing is retired; build unsigned APKs in the development domain and sign only in the isolated signer")
    authorization = _signed_record(args, "authorization_record", "sign-authorization")
    _confirm_production(args.disposable, f"build {lane} Beta APK")
    ledger_path, registry_path, witness_path = _external_state_paths(args, root)
    release_ledger = ReleaseLedger(ledger_path)
    registry = IdentityRegistry(registry_path)
    witness = SigningWitness(witness_path)
    mode = "disposable" if args.disposable else "production"
    if not args.disposable and (TRUSTED_AUTHORITY_PUBLIC_KEY_SHA256 is None or
                                _source_revisions(require_clean=True) != (authorization.get("source_commit"),
                                                                           authorization.get("source_tree"))):
        raise SigningError("Production Authority trust anchor or signed source provenance is unavailable")
    apk: Path | None = None
    observed: dict[str, str] = {}
    with _lock(root), _mounted_vault(root, args.disposable) as mount:
        manifest = _load_manifest_from_mount(mount, args.disposable)
        vault_ledger = _validate_internal_release_ledger(mount)
        release_state = release_ledger.assert_matches_vault(vault_ledger, manifest, expected_mode=mode)
        identity_inputs = {name: {"application_id": release_state["lanes"][name]["application_id"],
                                  "certificate_sha256": release_state["lanes"][name]["certificate_sha256"],
                                  "initial_version_code": release_state["lanes"][name]["initial_version_code"]}
                           for name in ("public", "private")}
        registry_state = registry.assert_binding(mode=mode, ledger_id=release_state["ledger_id"],
                                                 identities=identity_inputs)
        witness_state = witness.assert_binding(mode=mode, ledger_id=release_state["ledger_id"],
                                               identities=identity_inputs)
        registry_state = registry.assert_matches_witness(witness_state)
        if registry_state["witness_id"] != witness_state["witness_id"] or \
                vault_ledger.get("identity_registry_path") != str(registry_path) or \
                vault_ledger.get("signing_witness_path") != str(witness_path):
            raise SigningError("Vault, Identity Registry and independent witness bindings disagree")
        for name in ("public", "private"):
            next_code = release_state["lanes"][name]["next_version_code"]
            expected_high = next_code - 1
            lane_events = [event for event in release_state["events"] if event["lane"] == name]
            expected_attempt = lane_events[-1]["attempt_id"] if lane_events else None
            if registry_state["lanes"][name]["high_watermark"] != expected_high or \
                    witness_state["lanes"][name]["high_watermark"] != expected_high or \
                    registry_state["lanes"][name]["last_attempt_id"] != expected_attempt:
                raise SigningError("Vault, Ledger, Registry and independent witness high-water state disagrees")
        password = _prompts_for_lane(lane)
        try:
            identity = _verify_one_lane(mount, manifest, lane, password)
            version_code = release_state["lanes"][lane]["next_version_code"]
            attempt_id = authorization.get("attempt_id")
            source_commit, source_tree = _source_revisions()
            if authorization.get("mode") != mode or authorization.get("lane") != lane or \
                    authorization.get("operation") != "sign-apk" or \
                    authorization.get("application_id") != release_state["lanes"][lane]["application_id"] or \
                    authorization.get("certificate_sha256") != release_state["lanes"][lane]["certificate_sha256"] or \
                    authorization.get("ledger_id") != release_state["ledger_id"] or \
                    authorization.get("version_code") != version_code or \
                    authorization.get("source_commit") != source_commit or authorization.get("source_tree") != source_tree or \
                    not isinstance(attempt_id, str):
                raise SigningError("Signed Authority authorization does not match this exact signing attempt")
            witness.consume(lane=lane, version_code=version_code, attempt_id=attempt_id,
                            expected_generation=witness_state['generation'])
            registry.consume(lane=lane, version_code=version_code, attempt_id=attempt_id,
                             expected_generation=registry_state['generation'])
            reserved = release_ledger.reserve(lane, expected_generation=release_state["generation"],
                                              attempt_id=attempt_id)
            if reserved["version_code"] != version_code:
                raise SigningError("Release Ledger reserved an unexpected versionCode")
            identity["version_code"] = version_code
            identity["version_name"] = "0.1.0-demo"
            lane_ledger = vault_ledger.get("identities", {}).get(lane)
            if not isinstance(lane_ledger, dict) or lane_ledger.get("next_version_code") != version_code:
                raise SigningError("Vault and independent Release Ledger counters disagree")
            lane_ledger["next_version_code"] = version_code + 1
            _atomic_json(mount / "version-ledger.json", vault_ledger)
            release_ledger.mark_attempted(lane, version_code, attempt_id)
            apk = _gradle(lane, mount, manifest, password, allow_downloads=args.allow_dependency_downloads,
                          version_code=version_code)
            try:
                observed = _verify_apk(apk, lane, identity)
                _ledger_add(mount, lane, apk, observed)
                release_ledger.mark_built(lane, version_code, attempt_id,
                                          artifact_sha256=observed["apk_sha256"],
                                          version_name=observed["version_name"])
            except Exception:
                output_dir = apk.parent
                if output_dir.is_dir() and not output_dir.is_symlink():
                    shutil.rmtree(output_dir)
                raise
        finally:
            password = ""
    if apk is None:
        raise SigningError("APK build did not produce a verified artifact")
    print(f"APK build and identity verification passed: {apk}")
    print(json.dumps(observed, indent=2, sort_keys=True))
    print("No install or distribution was performed.")
    return 0


def _validate_internal_release_ledger(mount: Path) -> dict[str, Any]:
    value = _load_json(mount / "version-ledger.json")
    identities = value.get("identities")
    if value.get("schema") != 1 or not isinstance(identities, dict) or set(identities) != {"public", "private"}:
        raise SigningError("Vault internal version ledger is missing or malformed")
    expected_ids = {"public": "app.serea.android.beta", "private": "app.serea.android.privatebeta"}
    for lane, app_id in expected_ids.items():
        entry = identities[lane]
        if not isinstance(entry, dict) or entry.get("application_id") != app_id:
            raise SigningError("Vault internal version ledger lane identity is malformed")
        code = entry.get("next_version_code")
        if type(code) is not int or not 2 <= code <= 2_100_000_001:
            raise SigningError("Vault internal version ledger counter is outside the supported range")
        artifacts = entry.get("artifacts")
        if not isinstance(artifacts, list):
            raise SigningError("Vault internal version ledger artifact history is malformed")
        seen_codes: set[int] = set()
        for artifact in artifacts:
            if not isinstance(artifact, dict) or set(artifact) != {
                    "version_code", "version_name", "apk_sha256", "built_utc"}:
                raise SigningError("Vault internal version ledger artifact record is malformed")
            artifact_code = artifact.get("version_code")
            if type(artifact_code) is not int or artifact_code < 2 or artifact_code >= code or artifact_code in seen_codes:
                raise SigningError("Vault internal version ledger artifact history conflicts with its counter")
            if not isinstance(artifact.get("version_name"), str) or not artifact["version_name"]:
                raise SigningError("Vault internal version ledger artifact versionName is malformed")
            if not re.fullmatch(r"[0-9a-f]{64}", str(artifact.get("apk_sha256", ""))):
                raise SigningError("Vault internal version ledger artifact hash is malformed")
            if not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", str(artifact.get("built_utc", ""))):
                raise SigningError("Vault internal version ledger timestamp is malformed")
            seen_codes.add(artifact_code)
    ledger_id = value.get("release_ledger_id")
    ledger_path = value.get("release_ledger_path")
    registry_path = value.get("identity_registry_path")
    witness_path = value.get("signing_witness_path")
    if (ledger_id is None) != (ledger_path is None) or (ledger_id is None) != (registry_path is None) or \
            (ledger_id is None) != (witness_path is None):
        raise SigningError("Vault external signing-state binding is incomplete")
    if ledger_id is not None:
        try:
            str(uuid.UUID(ledger_id))
        except (ValueError, TypeError, AttributeError) as exc:
            raise SigningError("Vault independent Release Ledger identifier is malformed") from exc
        if not isinstance(ledger_path, str) or not Path(ledger_path).is_absolute() or \
                not isinstance(registry_path, str) or not Path(registry_path).is_absolute() or \
                not isinstance(witness_path, str) or not Path(witness_path).is_absolute():
            raise SigningError("Vault external signing-state path is malformed")
    return value


def _validate_vault(root: Path, disposable: bool, image: Path, passwords: dict[str, str], *, readonly: bool) -> dict[str, Any]:
    pending = image.with_name(image.name + ".serea-pending")
    if pending.exists() or pending.is_symlink():
        raise SigningError("Backup has an incomplete-copy marker; do not use it as a recovery image")
    volume = DISPOSABLE_VOLUME if disposable else FORMAL_VOLUME
    manifest: dict[str, Any] | None = None
    with _mount_context(image, volume, readonly=readonly) as mount:
        manifest = _load_manifest_from_mount(mount, disposable)
        _fingerprints_from_vault(mount, manifest, passwords)
        vault_ledger = _validate_internal_release_ledger(mount)
        bound_path = vault_ledger.get("release_ledger_path")
        if not bound_path:
            raise SigningError("Vault has no independent Release Ledger binding; recovery is not signing-ready")
        release_ledger = ReleaseLedger(bound_path)
        registry = IdentityRegistry(vault_ledger["identity_registry_path"])
        witness = SigningWitness(vault_ledger["signing_witness_path"])
        try:
            release = release_ledger.assert_matches_vault(vault_ledger, manifest,
                                                          expected_mode="disposable" if disposable else "production")
            inputs = {lane: {"application_id": release["lanes"][lane]["application_id"],
                             "certificate_sha256": release["lanes"][lane]["certificate_sha256"],
                             "initial_version_code": release["lanes"][lane]["initial_version_code"]}
                      for lane in ("public", "private")}
            registry_state = registry.assert_binding(mode=release["mode"], ledger_id=release["ledger_id"],
                                                     identities=inputs)
            witness_state = witness.assert_binding(mode=release["mode"], ledger_id=release["ledger_id"],
                                                   identities=inputs)
            registry_state = registry.assert_matches_witness(witness_state)
            if registry_state["witness_id"] != witness_state["witness_id"]:
                raise SigningError("Vault restore has a different Identity Registry witness binding")
            for lane in ("public", "private"):
                expected = release["lanes"][lane]["next_version_code"] - 1
                lane_events = [event for event in release["events"] if event["lane"] == lane]
                expected_attempt = lane_events[-1]["attempt_id"] if lane_events else None
                if registry_state["lanes"][lane]["high_watermark"] != expected or \
                        witness_state["lanes"][lane]["high_watermark"] != expected or \
                        registry_state["lanes"][lane]["last_attempt_id"] != expected_attempt:
                    raise SigningError("Vault restore does not match Registry and independent witness high-water state")
        except (LedgerError, RegistryError, WitnessError) as exc:
            raise SigningError("Vault recovery does not reconcile with all independent signing records") from exc
    if manifest is None:
        raise SigningError("Vault validation did not produce an identity manifest")
    return manifest


def _copy_no_replace(source: Path, destination: Path) -> str:
    source = _canonical_no_symlinks(source, allow_missing=False)
    destination = _canonical_no_symlinks(destination)
    if source.is_symlink() or not source.is_file():
        raise SigningError("Backup source is not a regular file")
    source_pending = source.with_name(source.name + ".serea-pending")
    if source_pending.exists() or source_pending.is_symlink():
        raise SigningError("Backup source has an incomplete-copy marker; refusing restore")
    destination_pending = destination.with_name(destination.name + ".serea-pending")
    if destination_pending.exists() or destination_pending.is_symlink():
        raise SigningError("Destination has an incomplete-copy marker; refusing overwrite")
    if destination.exists() or destination.is_symlink():
        raise SigningError("Destination already exists; refusing overwrite")
    parent = destination.parent
    if not parent.exists() or not parent.is_dir():
        raise SigningError("Destination parent directory must already exist")
    free = shutil.disk_usage(parent).free
    if free < source.stat().st_size:
        raise SigningError("Destination does not have enough free space")
    fd, temp_name = tempfile.mkstemp(prefix=".serea-backup-", dir=parent)
    temp = Path(temp_name)
    try:
        os.fchmod(fd, 0o600)
        with os.fdopen(fd, "wb") as out, source.open("rb") as src:
            shutil.copyfileobj(src, out, length=1024 * 1024)
            out.flush()
            os.fsync(out.fileno())
        source_hash = _sha256(source)
        if _sha256(temp) != source_hash:
            raise SigningError("Backup copy SHA-256 verification failed")
        if sys.platform == "darwin":
            import ctypes
            libc = ctypes.CDLL("/usr/lib/libSystem.B.dylib", use_errno=True)
            renamex = libc.renamex_np
            renamex.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint]
            renamex.restype = ctypes.c_int
            if renamex(os.fsencode(temp), os.fsencode(destination), 0x00000004) != 0:  # RENAME_EXCL
                error = ctypes.get_errno()
                if error not in (45, 95):  # Some external filesystems, including ExFAT, lack RENAME_EXCL.
                    raise OSError(error, os.strerror(error), str(destination))
                # Preserve no-overwrite semantics on such filesystems with an exclusive final create.
                # A durable marker makes an interrupted direct copy explicitly unusable as a backup.
                pending = destination.with_name(destination.name + ".serea-pending")
                marker_fd = os.open(pending, os.O_WRONLY | os.O_CREAT | os.O_EXCL |
                                    getattr(os, "O_NOFOLLOW", 0), 0o600)
                with os.fdopen(marker_fd, "w", encoding="ascii") as marker:
                    marker.write("incomplete backup copy; verify before recovery\\n")
                    marker.flush()
                    os.fsync(marker.fileno())
                _fsync_directory(parent)
                final_created = False
                try:
                    final_fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL |
                                       getattr(os, "O_NOFOLLOW", 0), 0o600)
                    final_created = True
                    with os.fdopen(final_fd, "wb") as out, temp.open("rb") as staged:
                        shutil.copyfileobj(staged, out, length=1024 * 1024)
                        out.flush()
                        os.fsync(out.fileno())
                    if _sha256(destination) != source_hash:
                        raise SigningError("External backup copy SHA-256 verification failed")
                    pending.unlink()
                    _fsync_directory(parent)
                except Exception:
                    if final_created:
                        with contextlib.suppress(FileNotFoundError):
                            destination.unlink()
                    with contextlib.suppress(FileNotFoundError):
                        pending.unlink()
                    raise
        else:
            os.link(temp, destination)
            temp.unlink()
        return source_hash
    finally:
        with contextlib.suppress(FileNotFoundError):
            temp.unlink()


def _remove_created_image_if_detached(image: Path, expected_hash: str) -> None:
    try:
        if _image_is_attached(image):
            print(f"WARNING: retaining image still attached for manual recovery: {image}", file=sys.stderr)
            return
        if image.is_file() and not image.is_symlink() and _sha256(image) == expected_hash:
            image.unlink()
    except (OSError, SigningError):
        print(f"WARNING: could not confirm safe cleanup; retaining image: {image}", file=sys.stderr)


def cmd_backup(args: argparse.Namespace) -> int:
    root = _root_for(args)
    _confirm_production(args.disposable, "copy unmounted encrypted vault backup")
    destination = _canonical_no_symlinks(Path(args.destination).expanduser())
    with _lock(root):
        image = _image_path(root, args.disposable)
        if _image_is_attached(image):
            raise SigningError("Vault must be unmounted before backup")
        passwords = {"public": _prompts_for_lane("public"), "private": _prompts_for_lane("private")}
        copy_created = False
        digest = ""
        try:
            # Validate source before copying; it is mounted read-only and detached before bytes are copied.
            _validate_vault(root, args.disposable, image, passwords, readonly=True)
            digest = _copy_no_replace(image, destination)
            copy_created = True
            _validate_vault(root, args.disposable, destination, passwords, readonly=True)
        except Exception:
            if copy_created:
                _remove_created_image_if_detached(destination, digest)
            raise
        finally:
            passwords = {"public": "", "private": ""}
    print(f"Encrypted image backup verified: {destination}")
    print(f"sha256={digest}")
    return 0


def cmd_verify_backup(args: argparse.Namespace) -> int:
    _confirm_production(args.disposable, "verify encrypted backup")
    backup = _canonical_no_symlinks(Path(args.backup).expanduser(), allow_missing=False)
    passwords = {"public": _prompts_for_lane("public"), "private": _prompts_for_lane("private")}
    try:
        manifest = _validate_vault(_root_for(args), args.disposable, backup, passwords, readonly=True)
    finally:
        passwords = {"public": "", "private": ""}
    print("Backup read-only mount, PKCS12 access, and certificate identities verified.")
    print(f"mode={manifest.get('mode')}")
    print(f"sha256={_sha256(backup)}")
    return 0


def cmd_restore(args: argparse.Namespace) -> int:
    root = _root_for(args)
    _confirm_production(args.disposable, "restore backup to new isolated path")
    source = _canonical_no_symlinks(Path(args.backup).expanduser(), allow_missing=False)
    destination = _canonical_no_symlinks(Path(args.destination).expanduser())
    formal_image = _image_path(root, False)
    if destination == formal_image or destination == _image_path(root, True):
        raise SigningError("Restore target may not be the active formal or disposable vault path")
    if destination.exists() or destination.is_symlink():
        raise SigningError("Restore target exists; refusing overwrite")
    passwords = {"public": _prompts_for_lane("public"), "private": _prompts_for_lane("private")}
    created_hash: str | None = None
    try:
        created_hash = _copy_no_replace(source, destination)
        _validate_vault(root, args.disposable, destination, passwords, readonly=True)
    except Exception:
        if created_hash:
            _remove_created_image_if_detached(destination, created_hash)
        raise
    finally:
        passwords = {"public": "", "private": ""}
    print(f"Backup restored to a new isolated image: {destination}")
    print(f"sha256={created_hash}")
    return 0


def _mounted_devices_for(image: Path, expected_volume: str) -> list[str]:
    target = str(image.resolve())
    devices = []
    for item in _attached_images():
        if str(item.get("image-path", "")) != target:
            continue
        for entity in item.get("system-entities", []):
            dev = entity.get("dev-entry")
            mountpoint = entity.get("mount-point")
            if dev and mountpoint:
                info = _disk_info(Path(str(mountpoint)))
                if info.get("VolumeName") != expected_volume:
                    raise SigningError("Mounted volume name differs; refusing to detach")
                devices.append(str(dev))
    return devices


def cmd_lock(args: argparse.Namespace) -> int:
    root = _root_for(args)
    _confirm_production(args.disposable, "unmount Serea signing vault")
    image = _image_path(root, args.disposable)
    expected = DISPOSABLE_VOLUME if args.disposable else FORMAL_VOLUME
    with _lock(root):
        devices = _mounted_devices_for(image, expected)
        for device in devices:
            _run(["/usr/bin/hdiutil", "detach", device], timeout=120)
    print("Vault is unmounted." if devices else "Vault was not mounted; no action needed.")
    return 0


def _add_common(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--disposable", action="store_true", help="Operate only on an explicitly supplied disposable root")
    parser.add_argument("--root", help="Vault directory; disposable mode requires this option")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Owner-controlled Serea signing vault CLI")
    commands = parser.add_subparsers(dest="command", required=True)
    for name, handler in (("doctor", cmd_doctor), ("init", cmd_init), ("provision", cmd_provision),
                          ("status", cmd_status), ("fingerprint", cmd_fingerprint), ("backup", cmd_backup),
                          ("verify-backup", cmd_verify_backup), ("restore", cmd_restore), ("lock", cmd_lock)):
        sub = commands.add_parser(name)
        _add_common(sub)
        sub.set_defaults(handler=handler)
    provision = commands.choices["provision"]
    provision.add_argument("--enrollment-slot-record", required=True)
    provision.add_argument("--authority-public-key")
    for name, handler in (("ledger-status", cmd_ledger_status), ("ledger-recovery-plan", cmd_ledger_recovery_plan),
                          ("authorization-request", cmd_authorization_request)):
        sub = commands.add_parser(name)
        _add_common(sub)
        sub.add_argument("--release-ledger", required=True)
        sub.add_argument("--identity-registry", required=True)
        sub.add_argument("--witness", required=True)
        if name == "authorization-request":
            sub.add_argument("--lane", choices=("public", "private"), required=True)
        sub.set_defaults(handler=handler)
    recover = commands.add_parser("ledger-recover-publish")
    _add_common(recover)
    recover.add_argument("--release-ledger", required=True)
    recover.add_argument("--identity-registry", required=True)
    recover.add_argument("--witness", required=True)
    recover.add_argument("--lane", choices=("public", "private"), required=True)
    recover.add_argument("--attempt-id", required=True)
    recover.add_argument("--version-code", type=int, required=True)
    recover.add_argument("--confirm-publish", required=True)
    recover.set_defaults(handler=cmd_ledger_recover_publish)
    abandon = commands.add_parser("ledger-abandon")
    _add_common(abandon)
    abandon.add_argument("--release-ledger", required=True)
    abandon.add_argument("--identity-registry", required=True)
    abandon.add_argument("--witness", required=True)
    abandon.add_argument("--lane", choices=("public", "private"), required=True)
    abandon.add_argument("--attempt-id", required=True)
    abandon.add_argument("--version-code", type=int, required=True)
    abandon.add_argument("--reason", choices=("build-failed", "owner-abandoned-before-attempt",
                                                "process-crash-reviewed", "artifact-review-rejected"), required=True)
    abandon.add_argument("--confirm", required=True)
    abandon.set_defaults(handler=cmd_ledger_abandon)
    witness_init = commands.add_parser("witness-init")
    _add_common(witness_init)
    witness_init.add_argument("--release-ledger", required=True)
    witness_init.add_argument("--identity-registry", required=True)
    witness_init.add_argument("--witness", required=True)
    witness_init.add_argument("--enrollment-record", required=True)
    witness_init.add_argument("--authority-public-key")
    witness_init.add_argument("--public-version-code", type=int, required=True)
    witness_init.add_argument("--private-version-code", type=int, required=True)
    witness_init.set_defaults(handler=cmd_witness_init)
    ledger_init = commands.add_parser("ledger-init")
    _add_common(ledger_init)
    ledger_init.add_argument("--release-ledger", required=True)
    ledger_init.add_argument("--identity-registry", required=True)
    ledger_init.add_argument("--witness", required=True)
    ledger_init.add_argument("--enrollment-record", required=True)
    ledger_init.add_argument("--authority-public-key")
    ledger_init.add_argument("--public-version-code", type=int, required=True)
    ledger_init.add_argument("--private-version-code", type=int, required=True)
    ledger_init.set_defaults(handler=cmd_release_ledger_init)
    signing = commands.add_parser("sign-apk")
    _add_common(signing)
    signing.add_argument("--release-ledger", required=True)
    signing.add_argument("--identity-registry", required=True)
    signing.add_argument("--witness", required=True)
    signing.add_argument("--lane", choices=("public", "private"), required=True)
    signing.add_argument("--apk", required=True)
    signing.add_argument("--provenance", required=True)
    signing.add_argument("--approved-apk-sha256", required=True)
    signing.add_argument("--approved-provenance-sha256", required=True)
    signing.add_argument("--approved-source-commit", required=True)
    signing.add_argument("--approved-source-tree", required=True)
    signing.add_argument("--output", required=True)
    signing.set_defaults(handler=cmd_sign_apk)
    public = commands.add_parser("build-public")
    _add_common(public)
    public.add_argument("--allow-dependency-downloads", action="store_true")
    public.add_argument("--release-ledger", required=True)
    public.add_argument("--identity-registry", required=True)
    public.add_argument("--witness", required=True)
    public.add_argument("--authorization-record", required=True)
    public.add_argument("--authority-public-key")
    public.set_defaults(handler=lambda a: cmd_build(a, "public"))
    private = commands.add_parser("build-private")
    _add_common(private)
    private.add_argument("--allow-dependency-downloads", action="store_true")
    private.add_argument("--release-ledger", required=True)
    private.add_argument("--identity-registry", required=True)
    private.add_argument("--witness", required=True)
    private.add_argument("--authorization-record", required=True)
    private.add_argument("--authority-public-key")
    private.set_defaults(handler=lambda a: cmd_build(a, "private"))
    backup = commands.choices["backup"]
    backup.add_argument("--destination", required=True)
    verify = commands.choices["verify-backup"]
    verify.add_argument("--backup", required=True)
    restore = commands.choices["restore"]
    restore.add_argument("--backup", required=True)
    restore.add_argument("--destination", required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return int(args.handler(args))
    except (SigningError, LedgerError, RegistryError, WitnessError, OSError, ValueError, KeyError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
