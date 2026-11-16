"""Non-trusting APK and provenance validation primitives for the isolated signer."""
from __future__ import annotations

import hashlib
import json
import os
import re
import stat
import subprocess
import tempfile
import zipfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

APP_IDS = {"public": "app.serea.android.beta", "private": "app.serea.android.privatebeta"}
VARIANTS = {"public": "beta", "private": "privateBeta"}
MAX_APK_BYTES = 1024 * 1024 * 1024
MAX_UNCOMPRESSED_BYTES = 2 * 1024 * 1024 * 1024
MAX_ENTRIES = 20_000
MAX_PROVENANCE_BYTES = 64 * 1024
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
REVISION_RE = re.compile(r"[0-9a-f]{40}\Z")


class ArtifactError(ValueError):
    """An untrusted artifact or approval record failed validation."""


@dataclass(frozen=True)
class ApkInspection:
    size_bytes: int
    sha256: str
    entries: int


def _read_regular_file(path: str | Path, *, maximum: int) -> tuple[bytes, os.stat_result]:
    target = Path(path).expanduser()
    try:
        before = target.lstat()
    except OSError as exc:
        raise ArtifactError("Input file is missing or inaccessible") from exc
    if not stat.S_ISREG(before.st_mode) or before.st_uid != os.getuid() or before.st_nlink != 1:
        raise ArtifactError("Input must be an Owner-owned regular file without hard links")
    if before.st_size <= 0 or before.st_size > maximum:
        raise ArtifactError("Input file size is outside the accepted range")
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(target, flags)
    except OSError as exc:
        raise ArtifactError("Input changed or is a symbolic link") from exc
    try:
        opened = os.fstat(fd)
        if (opened.st_dev, opened.st_ino, opened.st_uid, opened.st_size) != (
                before.st_dev, before.st_ino, os.getuid(), before.st_size):
            raise ArtifactError("Input identity changed while opening")
        if opened.st_size > maximum:
            raise ArtifactError("Input file exceeds the accepted size limit")
        chunks: list[bytes] = []
        remaining = maximum + 1
        while remaining:
            block = os.read(fd, min(1024 * 1024, remaining))
            if not block:
                break
            chunks.append(block)
            remaining -= len(block)
        data = b"".join(chunks)
        after = os.fstat(fd)
        if (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns) != (
                opened.st_dev, opened.st_ino, opened.st_size, opened.st_mtime_ns) or len(data) != opened.st_size:
            raise ArtifactError("Input changed while it was being read")
        return data, opened
    finally:
        os.close(fd)


def import_apk(source: str | Path, staging_directory: str | Path) -> Path:
    source_path = Path(source)
    staging = Path(staging_directory)
    try:
        directory_info = staging.lstat()
    except OSError as exc:
        raise ArtifactError("Signing staging directory is missing or inaccessible") from exc
    if not stat.S_ISDIR(directory_info.st_mode) or directory_info.st_uid != os.getuid() or \
            stat.S_IMODE(directory_info.st_mode) != 0o700:
        raise ArtifactError("Signing staging directory must be Owner-owned with mode 0700")
    data, _ = _read_regular_file(source_path, maximum=MAX_APK_BYTES)
    target = staging / source_path.name
    if target.parent != staging or target.exists() or target.is_symlink():
        raise ArtifactError("Signing staging target already exists or is unsafe")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(target, flags, 0o600)
    except OSError as exc:
        raise ArtifactError("Cannot safely create the staged APK") from exc
    try:
        with os.fdopen(fd, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
            os.fchmod(stream.fileno(), 0o400)
        directory_fd = os.open(staging, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    except OSError as exc:
        try:
            target.unlink()
        except OSError:
            pass
        raise ArtifactError("Staging the APK was not durably completed") from exc
    return target


def _tool_run(command: list[str | Path], *, input_data: bytes | None = None) -> subprocess.CompletedProcess[bytes]:
    java_home = Path.home() / "Library/Java/JavaVirtualMachines/temurin-17.jdk/Contents/Home"
    env = {"HOME": str(Path.home()), "JAVA_HOME": str(java_home),
           "PATH": f"{java_home / 'bin'}:/usr/bin:/bin", "TMPDIR": tempfile.gettempdir(),
           "LANG": "C", "LC_ALL": "C"}
    try:
        result = subprocess.run([str(item) for item in command], input=input_data, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, env=env, timeout=120, check=False)
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise ArtifactError(f"Android tool failed: {Path(str(command[0])).name}") from exc
    return result


def _assert_no_apk_signing_block(path: Path) -> None:
    data, _ = _read_regular_file(path, maximum=MAX_APK_BYTES)
    eocd = data.rfind(b"PK\x05\x06", max(0, len(data) - 65557))
    if eocd < 0 or eocd + 22 > len(data):
        raise ArtifactError("APK end-of-central-directory record is missing")
    central_offset = int.from_bytes(data[eocd + 16:eocd + 20], "little")
    if central_offset == 0xFFFFFFFF or central_offset < 24 or central_offset > eocd:
        raise ArtifactError("APK uses an unsupported or malformed ZIP64 central directory")
    if data[central_offset - 16:central_offset] == b"APK Sig Block 42":
        raise ArtifactError("APK already contains an APK Signing Block")


def _apk_identity(path: Path, aapt: str | Path) -> tuple[str, int, str]:
    result = _tool_run([aapt, "dump", "badging", path])
    if result.returncode:
        raise ArtifactError("aapt could not parse the APK manifest")
    output = result.stdout.decode("utf-8", "replace")
    match = re.search(r"^package: name='([^']+)' versionCode='([0-9]+)' versionName='([^']*)'", output, re.M)
    if not match:
        raise ArtifactError("APK manifest lacks a valid application ID/version identity")
    return match.group(1), int(match.group(2)), match.group(3)


def _verified_signer(path: Path, apksigner: str | Path, *, sdk: int) -> str:
    result = _tool_run([apksigner, "verify", "--verbose", "--print-certs", "--min-sdk-version", str(sdk),
                        "--max-sdk-version", str(sdk), path])
    if result.returncode:
        raise ArtifactError(f"apksigner verification failed for API {sdk}")
    output = result.stdout.decode("utf-8", "replace").lower()
    signer_blocks = re.findall(r"signer #\d+ certificate sha-256 digest:", output)
    digests = re.findall(r"certificate sha-256 digest:\s*([0-9a-f:]+)", output)
    if len(signer_blocks) != 1 or len(digests) != 1:
        raise ArtifactError("APK must contain exactly one verifiable signer")
    digest = digests[0].replace(":", "")
    if not SHA256_RE.fullmatch(digest):
        raise ArtifactError("apksigner returned a malformed certificate digest")
    return digest


def sign_apk(source: str | Path, output: str | Path, *, lane: str, version_code: int,
             version_name: str, keystore: str | Path, alias: str, password: str,
             aapt: str | Path, zipalign: str | Path, apksigner: str | Path,
             expected_certificate_sha256: str | None, mode: str) -> dict[str, Any]:
    if mode != "disposable":
        raise ArtifactError("Only Disposable APK signing is enabled")
    if lane not in APP_IDS:
        raise ArtifactError("Unknown signing lane")
    if not isinstance(password, str) or not password or "\n" in password or "\r" in password:
        raise ArtifactError("Signing passphrase is invalid")
    source_path = Path(source)
    output_path = Path(output)
    key_path = Path(keystore)
    if key_path.is_symlink() or not key_path.is_file() or key_path.stat().st_uid != os.getuid():
        raise ArtifactError("Signing keystore is missing or unsafe")
    parent = output_path.parent
    try:
        parent_info = parent.lstat()
    except OSError as exc:
        raise ArtifactError("Output parent directory is missing") from exc
    if not stat.S_ISDIR(parent_info.st_mode) or parent_info.st_uid != os.getuid() or \
            stat.S_IMODE(parent_info.st_mode) != 0o700:
        raise ArtifactError("Output parent directory must be Owner-owned with mode 0700")
    if output_path.exists() or output_path.is_symlink():
        raise ArtifactError("Output already exists; refusing overwrite")
    if expected_certificate_sha256 is not None and not SHA256_RE.fullmatch(expected_certificate_sha256):
        raise ArtifactError("Expected signer certificate SHA-256 is malformed")
    with tempfile.TemporaryDirectory(prefix="serea-sign-", dir=parent) as temp_name:
        work = Path(temp_name)
        os.chmod(work, 0o700)
        input_stage = work / "input"
        input_stage.mkdir(mode=0o700)
        staged = import_apk(source_path, input_stage)
        inspection = inspect_unsigned_apk(staged)
        _assert_no_apk_signing_block(staged)
        app_id, actual_code, actual_name = _apk_identity(staged, aapt)
        if app_id != APP_IDS[lane] or actual_code != version_code or actual_name != version_name:
            raise ArtifactError("APK application ID or version identity differs from the reserved lane")
        existing = _tool_run([apksigner, "verify", "--min-sdk-version", "26", "--max-sdk-version", "35", staged])
        if existing.returncode == 0:
            raise ArtifactError("APK is already signed")
        aligned = work / "aligned.apk"
        signed = work / "signed.apk"
        aligned_result = _tool_run([zipalign, "-p", "4", staged, aligned])
        if aligned_result.returncode:
            raise ArtifactError("zipalign failed; no output was published")
        checked = _tool_run([zipalign, "-c", "-p", "4", aligned])
        if checked.returncode:
            raise ArtifactError("zipalign verification failed")
        credentials = (password + "\n" + password + "\n").encode("utf-8")
        signed_result = _tool_run([
            apksigner, "sign", "--ks", key_path, "--ks-type", "PKCS12", "--ks-key-alias", alias,
            "--ks-pass", "stdin", "--key-pass", "stdin", "--min-sdk-version", "26",
            "--max-sdk-version", "35", "--in", aligned, "--out", signed,
        ], input_data=credentials)
        credentials = b""
        if signed_result.returncode or not signed.is_file():
            raise ArtifactError("apksigner signing failed; no output was published")
        final_app, final_code, final_name = _apk_identity(signed, aapt)
        if (final_app, final_code, final_name) != (app_id, actual_code, actual_name):
            raise ArtifactError("Signed APK identity changed")
        cert_digests = [_verified_signer(signed, apksigner, sdk=sdk) for sdk in (26, 35)]
        if cert_digests[0] != cert_digests[1] or (expected_certificate_sha256 is not None and
                                                   cert_digests[0] != expected_certificate_sha256):
            raise ArtifactError("Signed APK certificate does not match the expected single signer")
        signed_hash = hashlib.sha256(signed.read_bytes()).hexdigest()
        os.chmod(signed, 0o400)
        signed_fd = os.open(signed, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        try:
            os.fsync(signed_fd)
        finally:
            os.close(signed_fd)
        try:
            os.link(signed, output_path)
        except OSError as exc:
            raise ArtifactError("Cannot publish signed APK without replacing an existing file") from exc
        directory_fd = os.open(parent, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    return {"application_id": app_id, "version_code": actual_code, "version_name": actual_name,
            "signer_count": 1, "certificate_sha256": cert_digests[0], "apk_sha256": signed_hash,
            "verified_api_levels": [26, 35], "size_bytes": output_path.stat().st_size,
            "source_sha256": inspection.sha256}


def inspect_unsigned_apk(path: str | Path) -> ApkInspection:
    data, info = _read_regular_file(path, maximum=MAX_APK_BYTES)
    if len(data) < 22:
        raise ArtifactError("APK is not a complete ZIP archive")
    names: set[str] = set()
    uncompressed = 0
    try:
        from io import BytesIO
        with zipfile.ZipFile(BytesIO(data), "r") as archive:
            entries = archive.infolist()
            if not entries or len(entries) > MAX_ENTRIES:
                raise ArtifactError("APK ZIP entry count is outside the accepted range")
            has_manifest = False
            for entry in entries:
                name = entry.filename
                if not name or "\x00" in name or "\\" in name or name.startswith("/"):
                    raise ArtifactError("APK ZIP contains an unsafe entry path")
                parts = name.rstrip("/").split("/")
                if any(part in {"", ".", ".."} for part in parts):
                    raise ArtifactError("APK ZIP contains a traversal entry path")
                folded = name.casefold()
                if folded in names:
                    raise ArtifactError("APK ZIP contains a duplicate entry")
                names.add(folded)
                mode = (entry.external_attr >> 16) & 0xFFFF
                if stat.S_IFMT(mode) == stat.S_IFLNK:
                    raise ArtifactError("APK ZIP contains a symbolic link entry")
                if entry.flag_bits & 0x1:
                    raise ArtifactError("APK ZIP contains an encrypted entry")
                if entry.file_size < 0 or entry.compress_size < 0:
                    raise ArtifactError("APK ZIP entry size is malformed")
                uncompressed += entry.file_size
                if uncompressed > MAX_UNCOMPRESSED_BYTES:
                    raise ArtifactError("APK ZIP expands beyond the accepted size limit")
                if entry.file_size > 0 and entry.compress_size == 0:
                    raise ArtifactError("APK ZIP entry has an invalid compression size")
                if entry.compress_size and entry.file_size > max(entry.compress_size * 1000, 16 * 1024 * 1024):
                    raise ArtifactError("APK ZIP compression ratio is excessive")
                if name == "AndroidManifest.xml":
                    has_manifest = True
                upper = name.upper()
                if upper.startswith("META-INF/") and upper.endswith((".RSA", ".DSA", ".EC", ".SF")):
                    raise ArtifactError("APK already signed: v1 signature entry is present")
                if upper.startswith("META-INF/SIG-"):
                    raise ArtifactError("APK already signed: signature entry is present")
            if not has_manifest:
                raise ArtifactError("APK has no AndroidManifest.xml")
            corrupt = archive.testzip()
            if corrupt is not None:
                raise ArtifactError("APK ZIP entry failed CRC validation")
    except ArtifactError:
        raise
    except (OSError, zipfile.BadZipFile, RuntimeError, ValueError) as exc:
        raise ArtifactError("APK ZIP structure is malformed") from exc
    return ApkInspection(info.st_size, hashlib.sha256(data).hexdigest(), len(names))


def read_provenance(path: str | Path) -> tuple[dict[str, Any], str]:
    raw, _ = _read_regular_file(path, maximum=MAX_PROVENANCE_BYTES)
    try:
        value = json.loads(raw.decode("utf-8"))
    except (UnicodeError, json.JSONDecodeError) as exc:
        raise ArtifactError("Build provenance JSON is malformed") from exc
    if not isinstance(value, dict):
        raise ArtifactError("Build provenance must be a JSON object")
    expected_fields = {
        "schema", "lane", "variant", "application_id", "version_code", "version_name",
        "source_commit", "source_tree", "gradle_wrapper_sha256", "build_config_sha256",
        "build_conditions", "test_result", "apk_sha256",
    }
    if set(value) != expected_fields:
        raise ArtifactError("Build provenance fields are missing or unexpected")
    return value, hashlib.sha256(raw).hexdigest()


def validate_provenance(path: str | Path, *, lane: str, application_id: str, version_code: int,
                        version_name: str, apk_sha256: str, approved_apk_sha256: str,
                        approved_provenance_sha256: str, approved_source_commit: str,
                        approved_source_tree: str, return_sha256: bool = False) -> dict[str, Any] | tuple[dict[str, Any], str]:
    if lane not in APP_IDS or application_id != APP_IDS[lane]:
        raise ArtifactError("Requested lane and Application ID disagree")
    for label, digest in (("APK", apk_sha256), ("approved APK", approved_apk_sha256),
                          ("approved provenance", approved_provenance_sha256)):
        if not isinstance(digest, str) or not SHA256_RE.fullmatch(digest):
            raise ArtifactError(f"{label} SHA-256 is malformed")
    if apk_sha256 != approved_apk_sha256:
        raise ArtifactError("APK hash differs from the separately approved hash")
    if not isinstance(approved_source_commit, str) or not REVISION_RE.fullmatch(approved_source_commit) or \
            not isinstance(approved_source_tree, str) or not REVISION_RE.fullmatch(approved_source_tree):
        raise ArtifactError("Owner-approved source revision is malformed")
    if type(version_code) is not int or not 2 <= version_code <= 2_100_000_000:
        raise ArtifactError("APK versionCode is outside the supported range")
    if not isinstance(version_name, str) or not version_name or len(version_name) > 80:
        raise ArtifactError("APK versionName is invalid")
    value, actual_provenance_sha256 = read_provenance(path)
    if actual_provenance_sha256 != approved_provenance_sha256:
        raise ArtifactError("Provenance hash differs from the separately approved hash")
    expected = {
        "schema": 1,
        "lane": lane,
        "variant": VARIANTS[lane],
        "application_id": application_id,
        "version_code": version_code,
        "version_name": version_name,
        "source_commit": approved_source_commit,
        "source_tree": approved_source_tree,
        "build_conditions": "offline",
        "test_result": "PASS",
        "apk_sha256": apk_sha256,
    }
    for key, expected_value in expected.items():
        if type(value.get(key)) is not type(expected_value) or value.get(key) != expected_value:
            raise ArtifactError(f"Build provenance {key} differs from the approved signing request")
    for key in ("gradle_wrapper_sha256", "build_config_sha256"):
        if not isinstance(value.get(key), str) or not SHA256_RE.fullmatch(value[key]):
            raise ArtifactError(f"Build provenance {key} is malformed")
    return (value, actual_provenance_sha256) if return_sha256 else value
