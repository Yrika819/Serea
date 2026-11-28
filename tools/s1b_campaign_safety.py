"""Offline S1B APK verification and cooperative install-attempt lease.

This module never invokes adb, installs APKs, issues campaign IDs, or prints campaign tokens.
A future authorized caller may hold ``claim_install_attempt`` across its own device operations;
that caller remains responsible for restricting access to ADB and using only the returned snapshots.
"""
from __future__ import annotations

import argparse
import contextlib
import fcntl
import hashlib
import json
import os
import re
import secrets
import stat
import sys
import tempfile
import zipfile
from dataclasses import dataclass
from datetime import datetime, timezone
from io import BytesIO
from pathlib import Path
from typing import Callable, Iterable, Iterator, Any

if __package__ in {None, ""}:
    import serea_apk_signer
else:
    from tools import serea_apk_signer

CAMPAIGN_RE = re.compile(r"s1b[a-f0-9]{10}\Z")
PACKAGE_RE = re.compile(r"[A-Za-z][A-Za-z0-9_]*(?:\.[A-Za-z][A-Za-z0-9_]*)+\Z")
INVENTORY_PACKAGE_RE = re.compile(r"[A-Za-z][A-Za-z0-9_]*(?:\.[A-Za-z][A-Za-z0-9_]*)*\Z")
SHA256_RE = re.compile(r"[0-9a-f]{64}\Z")
RUNNER = "app.serea.android.SereaIsolatedAndroidJUnitRunner"
PROFILE_AVD = {
    "api26": "serea_s1_api26",
    "api35-4k": "serea_s1_01a3",
    "api35-16k": "serea_s1_16k",
}
TARGET_PREFIX = "app.serea.android.isolated."
MAX_LEDGER_BYTES = 64 * 1024
MAX_PACKAGE_INVENTORY_BYTES = 4 * 1024 * 1024
S1B_BUILD_TOOLS_VERSION = "35.0.0"
COMPONENT_TAGS = {"activity", "activity-alias", "provider", "receiver", "service"}
PERMISSION_TAG_PREFIX = "uses-permission"


class SafetyError(ValueError):
    """Fail-closed host verification or ledger-guard failure."""


@dataclass(frozen=True)
class Artifact:
    role: str
    application_id: str
    snapshot_path: Path
    sha256: str
    certificate_sha256: str
    debuggable: bool
    component_counts: dict[str, int]
    internet_permission: bool
    instrumentation_runner: str | None = None
    instrumentation_target: str | None = None

    def to_public_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "role": self.role,
            "application_id": self.application_id,
            "snapshot_path": str(self.snapshot_path),
            "sha256": self.sha256,
            "certificate_sha256": self.certificate_sha256,
            "debuggable": self.debuggable,
            "components": dict(self.component_counts),
            "internet_permission": self.internet_permission,
        }
        if self.role == "test":
            result["instrumentation_runner"] = self.instrumentation_runner
            result["instrumentation_target"] = self.instrumentation_target
        return result


@dataclass(frozen=True)
class VerifiedCampaign:
    campaign_id: str
    certificate_sha256: str
    trust_anchor_path: Path
    target: Artifact
    test: Artifact
    snapshot_directory: Path
    evidence_path: Path

    def to_public_dict(self) -> dict[str, Any]:
        return {
            "schema": 1,
            "purpose": "S1B_SYNTHETIC_DIAGNOSTIC_ONLY",
            "campaign_id": self.campaign_id,
            "certificate_sha256": self.certificate_sha256,
            "cryptographic_signature_valid": True,
            "supplied_trust_anchor_matched": True,
            "target": self.target.to_public_dict(),
            "test": self.test.to_public_dict(),
            "snapshot_directory": str(self.snapshot_directory),
            "evidence_path": str(self.evidence_path),
        }


@dataclass(frozen=True)
class InstallAttemptPermit:
    campaign_id: str
    target_package: str
    test_package: str
    target_apk: Path
    test_apk: Path
    target_sha256: str
    test_sha256: str
    campaign_token: str
    method: str | None = None

    def to_public_dict(self) -> dict[str, str]:
        return {
            "campaign_id": self.campaign_id,
            "target_package": self.target_package,
            "test_package": self.test_package,
            "target_sha256": self.target_sha256,
            "test_sha256": self.test_sha256,
        }


def _sha256_text(value: object) -> str:
    if not isinstance(value, str):
        raise SafetyError("Owner-approved certificate SHA-256 trust anchor is missing")
    normalized = value.lower()
    if re.fullmatch(r"(?:[0-9a-f]{2}:){31}[0-9a-f]{2}", normalized):
        normalized = normalized.replace(":", "")
    if not SHA256_RE.fullmatch(normalized):
        raise SafetyError("Owner-approved certificate SHA-256 trust anchor is malformed")
    return normalized


def _unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON key")
        result[key] = value
    return result


def _require_campaign(campaign_id: str) -> str:
    if not isinstance(campaign_id, str) or not CAMPAIGN_RE.fullmatch(campaign_id):
        raise SafetyError("Campaign ID is malformed")
    return campaign_id


def _load_trust_anchor(path: str | Path | None) -> tuple[Path, str]:
    if path is None:
        raise SafetyError("No S1B owner-approved certificate trust anchor is configured")
    anchor_path = Path(path)
    try:
        info = _require_owned_regular(anchor_path, mode=0o600, maximum_size=4096)
        fd = os.open(anchor_path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
        with os.fdopen(fd, "rb") as stream:
            opened = os.fstat(stream.fileno())
            if ((opened.st_dev, opened.st_ino, opened.st_size, opened.st_uid, opened.st_nlink,
                 stat.S_IMODE(opened.st_mode)) !=
                    (info.st_dev, info.st_ino, info.st_size, info.st_uid, info.st_nlink, 0o600)):
                raise SafetyError("Certificate trust anchor changed while opening")
            raw = stream.read(4097)
            after = os.fstat(stream.fileno())
            if (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns,
                after.st_uid, after.st_nlink, stat.S_IMODE(after.st_mode)) != (
                    opened.st_dev, opened.st_ino, opened.st_size, opened.st_mtime_ns, opened.st_ctime_ns,
                    opened.st_uid, opened.st_nlink, 0o600):
                raise SafetyError("Certificate trust anchor changed while being read")
        if len(raw) > 4096:
            raise SafetyError("Certificate trust anchor exceeds its size limit")
        data = json.loads(raw.decode("utf-8"), object_pairs_hook=_unique_object)
    except SafetyError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as exc:
        raise SafetyError("Certificate trust anchor is missing, malformed, or unsafe") from exc
    if (not isinstance(data, dict) or set(data) != {"schema", "purpose", "certificate_sha256"}
            or type(data.get("schema")) is not int or data.get("schema") != 1
            or data.get("purpose") != "S1B_SYNTHETIC_DIAGNOSTIC"):
        raise SafetyError("Certificate trust anchor has an unsupported schema or purpose")
    fingerprint = _sha256_text(data.get("certificate_sha256"))
    return anchor_path, fingerprint


def _require_owned_regular(path: Path, *, mode: int | None = None, minimum_size: int = 1,
                           maximum_size: int = serea_apk_signer.MAX_APK_BYTES) -> os.stat_result:
    try:
        info = path.lstat()
    except OSError as exc:
        raise SafetyError("Required input is missing or inaccessible") from exc
    if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_nlink != 1
            or info.st_size < minimum_size or info.st_size > maximum_size):
        raise SafetyError("Input must be an Owner-owned regular file without hard links")
    if mode is not None and stat.S_IMODE(info.st_mode) != mode:
        raise SafetyError("Input file permissions do not match the private staging policy")
    return info


def _require_private_directory(path: Path) -> os.stat_result:
    try:
        info = path.lstat()
    except OSError as exc:
        raise SafetyError("Private directory is missing or inaccessible") from exc
    if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid()
            or stat.S_IMODE(info.st_mode) != 0o700):
        raise SafetyError("Directory must be Owner-owned, non-symlink, and mode 0700")
    return info


def _tool_path(path: str | Path, label: str) -> Path:
    candidate = Path(path)
    try:
        info = candidate.lstat()
    except OSError as exc:
        raise SafetyError(f"Required Android tool is unavailable: {label}") from exc
    if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid()
            or not (info.st_mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH))):
        raise SafetyError(f"Android tool is not a trusted regular executable: {label}")
    return candidate


def resolve_android_tools(sdk_root: str | Path | None = None) -> tuple[Path, Path]:
    root_value = sdk_root or os.environ.get("ANDROID_SDK_ROOT") or os.environ.get("ANDROID_HOME")
    if not root_value:
        raise SafetyError("Android SDK root is unset; cannot resolve offline APK tools")
    selected = Path(root_value) / "build-tools" / S1B_BUILD_TOOLS_VERSION
    try:
        info = selected.lstat()
    except OSError as exc:
        raise SafetyError(f"Required Android Build Tools {S1B_BUILD_TOOLS_VERSION} are unavailable") from exc
    if not stat.S_ISDIR(info.st_mode):
        raise SafetyError("Pinned Android Build Tools directory is not a real directory")
    return (_tool_path(selected / "aapt", "aapt"),
            _tool_path(selected / "apksigner", "apksigner"))


def _run_tool(command: list[str | Path], *, label: str) -> bytes:
    # Existing shared helper passes an argv list with shell=False and a restricted Java env.
    try:
        result = serea_apk_signer._tool_run(command)
    except serea_apk_signer.ArtifactError as exc:
        raise SafetyError(f"Android {label} verification could not run") from exc
    if result.returncode:
        raise SafetyError(f"Android {label} verification failed")
    return result.stdout


def _decode_tool_output(output: bytes, label: str) -> str:
    try:
        return output.decode("utf-8", "strict")
    except UnicodeError as exc:
        raise SafetyError(f"Android {label} returned malformed text") from exc


def _validate_signed_zip(path: Path) -> str:
    try:
        raw, info = serea_apk_signer._read_regular_file(path, maximum=serea_apk_signer.MAX_APK_BYTES)
        if info.st_size != len(raw) or len(raw) < 22:
            raise SafetyError("APK snapshot is incomplete")
        names: set[str] = set()
        expanded = 0
        with zipfile.ZipFile(BytesIO(raw), "r") as archive:
            entries = archive.infolist()
            if not entries or len(entries) > serea_apk_signer.MAX_ENTRIES:
                raise SafetyError("APK ZIP entry count is outside the accepted range")
            manifest_count = 0
            for entry in entries:
                name = entry.filename
                if (not name or "\x00" in name or "\\" in name or name.startswith("/")):
                    raise SafetyError("APK ZIP contains an unsafe entry path")
                pieces = name.rstrip("/").split("/")
                if any(piece in {"", ".", ".."} for piece in pieces):
                    raise SafetyError("APK ZIP contains a traversal entry")
                folded = name.casefold()
                if folded in names:
                    raise SafetyError("APK ZIP contains a duplicate entry")
                names.add(folded)
                mode = (entry.external_attr >> 16) & 0xFFFF
                if stat.S_IFMT(mode) == stat.S_IFLNK:
                    raise SafetyError("APK ZIP contains a symbolic-link entry")
                if entry.flag_bits & 0x1:
                    raise SafetyError("APK ZIP contains an encrypted entry")
                if entry.file_size < 0 or entry.compress_size < 0:
                    raise SafetyError("APK ZIP entry size is malformed")
                expanded += entry.file_size
                if expanded > serea_apk_signer.MAX_UNCOMPRESSED_BYTES:
                    raise SafetyError("APK ZIP expands beyond the accepted size limit")
                if entry.file_size and (not entry.compress_size or entry.file_size >
                                        max(entry.compress_size * 1000, 16 * 1024 * 1024)):
                    raise SafetyError("APK ZIP compression ratio is excessive")
                if name == "AndroidManifest.xml":
                    manifest_count += 1
            if manifest_count != 1:
                raise SafetyError("APK must contain exactly one AndroidManifest.xml")
            corrupt = archive.testzip()
            if corrupt is not None:
                raise SafetyError("APK ZIP entry failed CRC validation")
        return hashlib.sha256(raw).hexdigest()
    except SafetyError:
        raise
    except (OSError, zipfile.BadZipFile, RuntimeError, ValueError) as exc:
        raise SafetyError("APK ZIP structure is malformed") from exc


def _aapt_badging(path: Path, apk: Path) -> tuple[str, bool]:
    output = _decode_tool_output(_run_tool([path, "dump", "badging", apk], label="aapt badging"), "aapt")
    packages = re.findall(r"^package: name='([^']+)'", output, re.MULTILINE)
    if len(packages) != 1 or not PACKAGE_RE.fullmatch(packages[0]):
        raise SafetyError("APK manifest package is missing, malformed, or ambiguous")
    min_sdks = re.findall(r"^sdkVersion:'([^']+)'", output, re.MULTILINE)
    if min_sdks != ["26"]:
        raise SafetyError("S1B APK minSdkVersion must be exactly 26")
    debug_count = sum(1 for line in output.splitlines() if line.strip() == "application-debuggable")
    non_debug_count = sum(1 for line in output.splitlines() if line.strip() == "application-not-debuggable")
    if debug_count + non_debug_count != 1:
        raise SafetyError("APK debuggable state is missing or ambiguous")
    return packages[0], debug_count == 1


_ELEMENT_RE = re.compile(r"^(\s*)E:\s+([^\s(]+)")
_ATTRIBUTE_RE = re.compile(r"^\s*A:\s+(android:[A-Za-z0-9_]+|package)(?:\([^)]*\))?\s*=\s*(.*)$")
_QUOTED_RE = re.compile(r'^"([^"\\]*(?:\\.[^"\\]*)*)"')


def _quoted_attribute(value: str) -> str | None:
    match = _QUOTED_RE.match(value)
    return match.group(1) if match else None


def _component_attribute_value(key: str, raw: str) -> str | None:
    quoted = _quoted_attribute(raw)
    if quoted is not None or key != "android:exported":
        return quoted
    typed_boolean = re.fullmatch(r"\(type 0x12\)0x([0-9a-fA-F]+)", raw.strip())
    if not typed_boolean:
        return None
    return {"0": "false", "00000000": "false", "1": "true", "ffffffff": "true"}.get(
        typed_boolean.group(1).lower()
    )


def _parse_manifest_tree(text: str) -> dict[str, Any]:
    stack: list[tuple[int, str, dict[str, Any]]] = []
    roots = 0
    applications: list[dict[str, Any]] = []
    instrumentations: list[dict[str, str | None]] = []
    permissions: list[str] = []
    components = {"activity": 0, "provider": 0, "receiver": 0, "service": 0}
    component_nodes: list[dict[str, Any]] = []
    root_packages: list[str] = []
    shared_user_attributes: set[str] = set()
    for line in text.splitlines():
        element = _ELEMENT_RE.match(line)
        if element:
            indent = len(element.group(1))
            tag = element.group(2)
            while stack and stack[-1][0] >= indent:
                stack.pop()
            parent = stack[-1][1] if stack else None
            node: dict[str, Any] = {}
            if tag == "manifest" and parent is None:
                roots += 1
            if tag == "application" and parent == "manifest":
                applications.append(node)
            if tag in COMPONENT_TAGS and parent == "application":
                category = "activity" if tag in {"activity", "activity-alias"} else tag
                components[category] += 1
                node["_component_tag"] = tag
                component_nodes.append(node)
            if tag == "instrumentation" and parent == "manifest":
                node.update({"android:name": None, "android:targetPackage": None})
                instrumentations.append(node)
            if tag.startswith(PERMISSION_TAG_PREFIX) and parent == "manifest":
                node["_permission"] = True
            stack.append((indent, tag, node))
            continue
        attribute = _ATTRIBUTE_RE.match(line)
        if not attribute or not stack:
            continue
        key = attribute.group(1)
        value = _quoted_attribute(attribute.group(2))
        current = stack[-1][2]
        tag = stack[-1][1]
        if key == "package" and tag == "manifest" and value is not None:
            root_packages.append(value)
        if tag == "manifest" and key in {"android:sharedUserId", "android:sharedUserMaxSdkVersion"}:
            shared_user_attributes.add(key)
        if tag == "application" and key == "android:name":
            current["_application_name"] = value if value is not None else "<unresolved>"
        if current.get("_component_tag") and key in {"android:name", "android:exported"}:
            component_value = _component_attribute_value(key, attribute.group(2))
            current[key] = component_value if component_value is not None else "<unresolved>"
        if tag == "instrumentation" and key in {"android:name", "android:targetPackage"}:
            current[key] = value
        if current.get("_permission") and tag.startswith(PERMISSION_TAG_PREFIX) and key == "android:name":
            if value is None:
                permissions.append("<unresolved>")
            else:
                permissions.append(value)
    if roots != 1 or len(applications) != 1 or len(root_packages) != 1:
        raise SafetyError("APK manifest root/application identity is missing or ambiguous")
    app_attributes = applications[0]
    return {
        "package": root_packages[0],
        "shared_user_attributes": shared_user_attributes,
        "application_name_present": "_application_name" in app_attributes,
        "components": components,
        "component_details": [
            {"tag": node["_component_tag"], "name": node.get("android:name"),
             "exported": node.get("android:exported")}
            for node in component_nodes
        ],
        "instrumentations": instrumentations,
        "permissions": permissions,
    }


def _manifest_for_role(aapt: Path, apk: Path, role: str, expected_package: str,
                       expected_target: str) -> tuple[str, dict[str, int], bool, bool, str | None, str | None]:
    package, debuggable = _aapt_badging(aapt, apk)
    tree_text = _decode_tool_output(
        _run_tool([aapt, "dump", "xmltree", apk, "AndroidManifest.xml"], label="aapt manifest"),
        "aapt manifest",
    )
    manifest = _parse_manifest_tree(tree_text)
    if package != expected_package or manifest["package"] != package:
        raise SafetyError(f"{role} APK package does not match the campaign identity")
    if manifest["shared_user_attributes"]:
        raise SafetyError(f"{role} APK uses sharedUserId and cannot guarantee package UID separation")
    if manifest["application_name_present"]:
        raise SafetyError(f"{role} APK declares an Application class outside the minimal diagnostic contract")
    components = manifest["components"]
    if role == "target":
        expected_components = [{
            "tag": "activity", "name": "app.serea.android.MainActivity", "exported": "false",
        }]
        if manifest["component_details"] != expected_components:
            raise SafetyError("Campaign target component policy requires only non-exported MainActivity")
    elif role == "test":
        expected_test_components = [{
            "tag": "activity",
            "name": "androidx.test.core.app.InstrumentationActivityInvoker$EmptyActivity",
            "exported": "true",
        }]
        if manifest["component_details"] != expected_test_components:
            raise SafetyError(
                "test APK component policy permits only the AndroidX Test EmptyActivity"
            )
    if not debuggable:
        raise SafetyError(f"{role} APK is not debuggable for the synthetic diagnostic lane")
    internet = "android.permission.INTERNET" in manifest["permissions"]
    if role == "target":
        if internet:
            raise SafetyError("Campaign target APK declares INTERNET permission")
        if manifest["instrumentations"]:
            raise SafetyError("Campaign target APK unexpectedly declares instrumentation")
        return package, components, debuggable, internet, None, None
    instrumentations = manifest["instrumentations"]
    if len(instrumentations) != 1:
        raise SafetyError("Campaign test APK must declare exactly one instrumentation")
    instrumentation = instrumentations[0]
    runner = instrumentation.get("android:name")
    target = instrumentation.get("android:targetPackage")
    if not runner or not target:
        raise SafetyError("Campaign instrumentation runner/targetPackage is missing or unresolved")
    if runner != RUNNER:
        raise SafetyError("Campaign instrumentation runner does not match the allowlisted class")
    if target != expected_target:
        raise SafetyError("Campaign instrumentation targetPackage does not match the target APK")
    return package, components, debuggable, internet, runner, target


def _verified_signer_range(apk: Path, apksigner: Path) -> str:
    output = _decode_tool_output(_run_tool([
        apksigner, "verify", "--verbose", "--print-certs", "--min-sdk-version", "26",
        "--max-sdk-version", "35", apk,
    ], label="apksigner signature"), "apksigner")
    lowered = output.lower()
    signer_blocks = re.findall(r"signer #\d+ certificate sha-256 digest:", lowered)
    digests = re.findall(r"certificate sha-256 digest:\s*([0-9a-f:]+)", lowered)
    if len(signer_blocks) != 1 or len(digests) != 1:
        raise SafetyError("APK must have exactly one verifiable signer")
    fingerprint = digests[0].replace(":", "")
    if not SHA256_RE.fullmatch(fingerprint):
        raise SafetyError("apksigner returned a malformed SHA-256 certificate fingerprint")
    return fingerprint


def _verify_artifact(role: str, apk: Path, *, campaign_id: str, target_package: str,
                     expected_certificate_sha256: str, aapt: Path, apksigner: Path) -> Artifact:
    before = _require_owned_regular(apk, mode=0o400)
    digest = _validate_signed_zip(apk)
    signer_fingerprint = _verified_signer_range(apk, apksigner)
    if signer_fingerprint != expected_certificate_sha256:
        raise SafetyError(f"{role} APK signature is cryptographically valid but certificate does not match the supplied trust anchor")
    package = target_package if role == "target" else target_package + ".test"
    actual_package, components, debuggable, internet, runner, instrumentation_target = _manifest_for_role(
        aapt, apk, role, package, target_package,
    )
    if actual_package != package:
        raise SafetyError(f"{role} APK application ID is not the exact campaign package")
    after = _require_owned_regular(apk, mode=0o400)
    final_digest = _validate_signed_zip(apk)
    if (final_digest != digest or
            (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) !=
            (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns)):
        raise SafetyError("APK snapshot changed while signature/manifest verification was in progress")
    return Artifact(
        role=role, application_id=actual_package, snapshot_path=apk, sha256=digest,
        certificate_sha256=signer_fingerprint, debuggable=debuggable, component_counts=components,
        internet_permission=internet, instrumentation_runner=runner,
        instrumentation_target=instrumentation_target,
    )


def _safe_tool_pair(aapt: str | Path, apksigner: str | Path) -> tuple[Path, Path]:
    return _tool_path(aapt, "aapt"), _tool_path(apksigner, "apksigner")


def _write_private_json(path: Path, value: dict[str, Any]) -> None:
    directory = path.parent
    directory_info = _require_private_directory(directory)
    directory_fd = os.open(directory, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) |
                            getattr(os, "O_NOFOLLOW", 0))
    temp_name = f".{path.name}.{secrets.token_hex(8)}.tmp"
    fd = -1
    published = False
    try:
        current = os.fstat(directory_fd)
        if (current.st_dev, current.st_ino) != (directory_info.st_dev, directory_info.st_ino):
            raise SafetyError("Evidence directory changed during validation")
        if path.exists() or path.is_symlink():
            raise SafetyError("Verification evidence path already exists")
        fd = os.open(temp_name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
                     0o600, dir_fd=directory_fd)
        with os.fdopen(fd, "w", encoding="utf-8", closefd=True) as stream:
            fd = -1
            os.fchmod(stream.fileno(), 0o400)
            json.dump(value, stream, sort_keys=True, separators=(",", ":"))
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temp_name, path.name, src_dir_fd=directory_fd, dst_dir_fd=directory_fd)
        published = True
        os.fsync(directory_fd)
    except (OSError, TypeError, ValueError) as exc:
        raise SafetyError("Private verification evidence could not be durably written") from exc
    finally:
        if fd >= 0:
            os.close(fd)
        if not published:
            with contextlib.suppress(OSError):
                os.unlink(temp_name, dir_fd=directory_fd)
        os.close(directory_fd)


def _new_snapshot_directory(root: Path, campaign_id: str) -> Path:
    _require_private_directory(root)
    root_fd = os.open(root, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) |
                       getattr(os, "O_NOFOLLOW", 0))
    selected: str | None = None
    try:
        for _ in range(5):
            name = f"s1b-verified-{campaign_id}-{secrets.token_hex(6)}"
            try:
                os.mkdir(name, 0o700, dir_fd=root_fd)
            except FileExistsError:
                continue
            selected = name
            os.fsync(root_fd)
            break
    except OSError as exc:
        raise SafetyError("Cannot durably create a private APK snapshot directory") from exc
    finally:
        os.close(root_fd)
    if selected is None:
        raise SafetyError("Cannot allocate a unique APK snapshot directory")
    result = root / selected
    _require_private_directory(result)
    return result


def _snapshot_apk(source: str | Path, role_directory: Path) -> Path:
    source_path = Path(source)
    if source_path.suffix.lower() != ".apk":
        raise SafetyError("Input artifact must have an .apk extension")
    try:
        return serea_apk_signer.import_apk(source_path, role_directory)
    except serea_apk_signer.ArtifactError as exc:
        raise SafetyError(f"{source_path.name} cannot be safely snapshotted") from exc


def verify_campaign_apks(*, target_apk: str | Path, test_apk: str | Path, campaign_id: str,
                         staging_root: str | Path, trust_anchor: str | Path | None,
                         aapt: str | Path, apksigner: str | Path) -> VerifiedCampaign:
    campaign_id = _require_campaign(campaign_id)
    target_package = TARGET_PREFIX + campaign_id
    test_package = target_package + ".test"
    if not PACKAGE_RE.fullmatch(target_package) or not PACKAGE_RE.fullmatch(test_package):
        raise SafetyError("Derived campaign package identity is malformed")
    trust_anchor_path, pin = _load_trust_anchor(trust_anchor)
    aapt_path, apksigner_path = _safe_tool_pair(aapt, apksigner)
    root = Path(staging_root)
    _require_private_directory(root)
    snapshot_directory = _new_snapshot_directory(root, campaign_id)
    target_dir = snapshot_directory / "target"
    test_dir = snapshot_directory / "test"
    target_dir.mkdir(mode=0o700)
    test_dir.mkdir(mode=0o700)
    target_snapshot = _snapshot_apk(target_apk, target_dir)
    test_snapshot = _snapshot_apk(test_apk, test_dir)
    target = _verify_artifact("target", target_snapshot, campaign_id=campaign_id,
                              target_package=target_package, expected_certificate_sha256=pin,
                              aapt=aapt_path, apksigner=apksigner_path)
    test = _verify_artifact("test", test_snapshot, campaign_id=campaign_id,
                            target_package=target_package, expected_certificate_sha256=pin,
                            aapt=aapt_path, apksigner=apksigner_path)
    evidence_path = snapshot_directory / "verification.json"
    result = VerifiedCampaign(campaign_id, pin, trust_anchor_path, target, test,
                               snapshot_directory, evidence_path)
    _write_private_json(evidence_path, result.to_public_dict())
    os.chmod(evidence_path, 0o400, follow_symlinks=False)
    return result


def _verify_snapshot_pair(verified: VerifiedCampaign, aapt: Path, apksigner: Path) -> tuple[Artifact, Artifact]:
    if not CAMPAIGN_RE.fullmatch(verified.campaign_id):
        raise SafetyError("Verified campaign identity is malformed")
    _require_private_directory(verified.snapshot_directory)
    anchor_path, pin = _load_trust_anchor(verified.trust_anchor_path)
    if anchor_path != verified.trust_anchor_path or pin != verified.certificate_sha256:
        raise SafetyError("Owner-approved certificate trust anchor changed after APK verification")
    target_package = TARGET_PREFIX + verified.campaign_id
    checked: list[Artifact] = []
    for expected in (verified.target, verified.test):
        _require_private_directory(expected.snapshot_path.parent)
        info = _require_owned_regular(expected.snapshot_path, mode=0o400)
        if expected.snapshot_path.parent.parent != verified.snapshot_directory:
            raise SafetyError("Verified APK snapshot escaped its private session directory")
        actual = _verify_artifact(expected.role, expected.snapshot_path,
                                  campaign_id=verified.campaign_id, target_package=target_package,
                                  expected_certificate_sha256=pin, aapt=aapt, apksigner=apksigner)
        after = _require_owned_regular(expected.snapshot_path, mode=0o400)
        if (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns) != (
                after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns):
            raise SafetyError("Verified APK snapshot changed during revalidation")
        if (actual.sha256 != expected.sha256 or actual.application_id != expected.application_id
                or actual.certificate_sha256 != expected.certificate_sha256
                or actual.component_counts != expected.component_counts
                or actual.debuggable != expected.debuggable
                or actual.instrumentation_runner != expected.instrumentation_runner
                or actual.instrumentation_target != expected.instrumentation_target):
            raise SafetyError("Verified APK snapshot no longer matches its evidence")
        checked.append(actual)
    if checked[0].certificate_sha256 != checked[1].certificate_sha256:
        raise SafetyError("Target and test APK do not share the approved diagnostic signer")
    return checked[0], checked[1]


def _validate_ledger_directory(directory: Path) -> os.stat_result:
    return _require_private_directory(directory)


@contextlib.contextmanager
def reservation_lock(ledger_directory: str | Path, *, blocking: bool = False) -> Iterator[int]:
    directory = Path(ledger_directory)
    directory_info = _validate_ledger_directory(directory)
    directory_fd = -1
    lock_fd = -1
    try:
        directory_fd = os.open(directory, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) |
                               getattr(os, "O_NOFOLLOW", 0))
        opened_dir = os.fstat(directory_fd)
        if (opened_dir.st_dev, opened_dir.st_ino, opened_dir.st_uid) != (
                directory_info.st_dev, directory_info.st_ino, os.getuid()):
            raise SafetyError("Campaign ledger directory changed while opening")
        lock_fd = os.open(".reservation.lock", os.O_RDWR | getattr(os, "O_NOFOLLOW", 0),
                          dir_fd=directory_fd)
        lock_info = os.fstat(lock_fd)
        lock_path_info = os.stat(".reservation.lock", dir_fd=directory_fd, follow_symlinks=False)
        if ((not stat.S_ISREG(lock_info.st_mode) or lock_info.st_uid != os.getuid()
             or lock_info.st_nlink != 1 or stat.S_IMODE(lock_info.st_mode) != 0o600)
                or (lock_info.st_dev, lock_info.st_ino) != (lock_path_info.st_dev, lock_path_info.st_ino)):
            raise SafetyError("Campaign reservation lock must be Owner-owned, regular, stable, and mode 0600")
        directory_after = os.stat(directory, follow_symlinks=False)
        if (directory_info.st_dev, directory_info.st_ino) != (directory_after.st_dev, directory_after.st_ino):
            raise SafetyError("Campaign ledger directory changed while acquiring its lock")
        operation = fcntl.LOCK_EX | (0 if blocking else fcntl.LOCK_NB)
        try:
            fcntl.flock(lock_fd, operation)
        except BlockingIOError as exc:
            raise SafetyError("Campaign reservation/install lock is busy") from exc
        lock_path_after = os.stat(".reservation.lock", dir_fd=directory_fd, follow_symlinks=False)
        if (lock_info.st_dev, lock_info.st_ino) != (lock_path_after.st_dev, lock_path_after.st_ino):
            raise SafetyError("Campaign reservation lock entry changed while locked")
    except SafetyError:
        if lock_fd >= 0:
            with contextlib.suppress(OSError):
                fcntl.flock(lock_fd, fcntl.LOCK_UN)
            os.close(lock_fd)
        if directory_fd >= 0:
            os.close(directory_fd)
        raise
    except OSError as exc:
        if lock_fd >= 0:
            with contextlib.suppress(OSError):
                fcntl.flock(lock_fd, fcntl.LOCK_UN)
            os.close(lock_fd)
        if directory_fd >= 0:
            os.close(directory_fd)
        raise SafetyError("Campaign reservation lock is missing or unsafe") from exc
    try:
        yield directory_fd
    finally:
        with contextlib.suppress(OSError):
            fcntl.flock(lock_fd, fcntl.LOCK_UN)
        os.close(lock_fd)
        os.close(directory_fd)


def _read_campaign_record(directory_fd: int, name: str) -> dict[str, Any]:
    try:
        info = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
        if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_nlink != 1
                or stat.S_IMODE(info.st_mode) != 0o600 or info.st_size < 1
                or info.st_size > MAX_LEDGER_BYTES):
            raise SafetyError("Campaign ledger record must be Owner-owned, regular, single-link, and mode 0600")
        flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
        fd = os.open(name, flags, dir_fd=directory_fd)
        with os.fdopen(fd, "rb") as stream:
            opened = os.fstat(stream.fileno())
            if ((opened.st_dev, opened.st_ino, opened.st_size, opened.st_uid, opened.st_nlink,
                 stat.S_IMODE(opened.st_mode)) !=
                    (info.st_dev, info.st_ino, info.st_size, info.st_uid, info.st_nlink, 0o600)):
                raise SafetyError("Campaign ledger changed while opening")
            raw = stream.read(MAX_LEDGER_BYTES + 1)
            after = os.fstat(stream.fileno())
            if (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns,
                after.st_uid, after.st_nlink, stat.S_IMODE(after.st_mode)) != (
                    opened.st_dev, opened.st_ino, opened.st_size, opened.st_mtime_ns, opened.st_ctime_ns,
                    opened.st_uid, opened.st_nlink, 0o600):
                raise SafetyError("Campaign ledger changed while being read")
        if len(raw) > MAX_LEDGER_BYTES:
            raise SafetyError("Campaign ledger record exceeds its size limit")
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=_unique_object)
    except SafetyError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError, ValueError) as exc:
        raise SafetyError("Campaign ledger record is missing, malformed, or unsafe") from exc
    if not isinstance(value, dict):
        raise SafetyError("Campaign ledger record must be a JSON object")
    return value


S1B_OWNER_SCOPE = "S1B-S1-02-API35-4K"
S1B_OWNER_METHODS = {
    "ownership": "app.serea.android.core.EmbeddedCoreAdapterTest#processOwnerSurvivesAdapterReplacement",
    "ownership-recovery": "app.serea.android.core.EmbeddedCoreAdapterTest#processOwnerSurvivesAdapterReplacement",
    "ownership-recovery-r2": "app.serea.android.core.EmbeddedCoreAdapterTest#processOwnerSurvivesAdapterReplacement",
    "chat-core": "app.serea.android.EmbeddedCoreDiagnosticChatUiTest#diagnosticChatUsesCoreMockAcrossRecreationAndCleansUp",
    "chat-core-recovery": "app.serea.android.EmbeddedCoreDiagnosticChatUiTest#diagnosticChatUsesCoreMockAcrossRecreationAndCleansUp",
}


def _validate_campaign_record(record: dict[str, Any], verified: VerifiedCampaign, name: str,
                              expected_method: str | None = None) -> None:
    cid = verified.campaign_id
    if name != f"{cid}.json" or record.get("campaign_id") != cid:
        raise SafetyError("Campaign ledger ID does not match the verified APK pair")
    if record.get("state") != "PREPARED_ONLY_NOT_BUILT_NOT_INSTALLED":
        raise SafetyError("Campaign ledger ID has already been attempted or consumed")
    if record.get("state") != "PREPARED_ONLY_NOT_BUILT_NOT_INSTALLED":
        raise SafetyError("Campaign ID is not fresh; attempted/consumed records cannot be reused")
    required = {"campaign_id", "profile", "avd", "target_package", "test_package",
                "campaign_token", "created_utc", "state", "minimum_free_gib_at_prepare", "constraints"}
    optional = {"owner_approved_scope", "method", "install_attempted_utc", "verified_apk_sha256",
                "verified_certificate_sha256", "runtime_attempted_utc", "runtime_method",
                "runtime_result_sha256", "runtime_cleanup_status", "runtime_not_started_utc",
                "runtime_not_started_reason"}
    if not required.issubset(record) or set(record) - required - optional:
        raise SafetyError("Campaign ledger record has missing or unknown fields")
    profile = record.get("profile")
    if profile not in PROFILE_AVD or record.get("avd") != PROFILE_AVD[profile]:
        raise SafetyError("Campaign ledger profile/AVD binding is invalid")
    target = TARGET_PREFIX + cid
    if (record.get("target_package") != target or record.get("test_package") != target + ".test"
            or verified.target.application_id != target or verified.test.application_id != target + ".test"):
        raise SafetyError("Campaign ledger package binding does not match the APK evidence")
    token = record.get("campaign_token")
    if not isinstance(token, str) or not re.fullmatch(r"[0-9a-f]{32}", token):
        raise SafetyError("Campaign ledger token is missing or malformed")
    created = record.get("created_utc")
    try:
        created_at = datetime.fromisoformat(created) if isinstance(created, str) else None
    except ValueError as exc:
        raise SafetyError("Campaign ledger creation timestamp is malformed") from exc
    if created_at is None or created_at.tzinfo is None:
        raise SafetyError("Campaign ledger creation timestamp must include a timezone")
    minimum_free = record.get("minimum_free_gib_at_prepare")
    if type(minimum_free) is not int or minimum_free < 8:
        raise SafetyError("Campaign ledger free-space attestation is missing or below policy")
    constraints = record.get("constraints")
    if not isinstance(constraints, str) or "No existing AVD/app/db/logs" not in constraints:
        raise SafetyError("Campaign ledger safety constraints are missing")
    scope = record.get("owner_approved_scope")
    method_name = record.get("method")
    if scope is not None or method_name is not None:
        if (scope != S1B_OWNER_SCOPE or method_name not in S1B_OWNER_METHODS
                or profile != "api35-4k"):
            raise SafetyError("Campaign ledger is outside the Owner-approved API35-4KB S1B/S1-02 scope")
    if expected_method is not None and S1B_OWNER_METHODS.get(method_name) != expected_method:
        raise SafetyError("Campaign ledger method does not match this one-time runtime selection")
    if record.get("state") == "RUNTIME_PASS":
        if (S1B_OWNER_METHODS.get(method_name) != record.get("runtime_method")
                or not re.fullmatch(r"[0-9a-f]{64}", str(record.get("runtime_result_sha256", "")))
                or record.get("runtime_cleanup_status")!="CLEANUP_PASS"):
            raise SafetyError("Runtime PASS ledger evidence is incomplete or inconsistent")


def _fsync_directory(directory_fd: int) -> None:
    os.fsync(directory_fd)


def _atomic_update_record(directory_fd: int, record_name: str, record: dict[str, Any]) -> None:
    temp_name = f".{record_name}.{secrets.token_hex(8)}.tmp"
    temp_fd = -1
    published = False
    try:
        temp_fd = os.open(temp_name, os.O_WRONLY | os.O_CREAT | os.O_EXCL |
                          getattr(os, "O_NOFOLLOW", 0), 0o600, dir_fd=directory_fd)
        with os.fdopen(temp_fd, "w", encoding="utf-8", closefd=True) as stream:
            temp_fd = -1
            os.fchmod(stream.fileno(), 0o600)
            json.dump(record, stream, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        current = os.stat(record_name, dir_fd=directory_fd, follow_symlinks=False)
        if (not stat.S_ISREG(current.st_mode) or current.st_uid != os.getuid()
                or current.st_nlink != 1 or stat.S_IMODE(current.st_mode) != 0o600
                or current.st_size > MAX_LEDGER_BYTES):
            raise SafetyError("Campaign ledger record became unsafe before replacement")
        os.replace(temp_name, record_name, src_dir_fd=directory_fd, dst_dir_fd=directory_fd)
        published = True
        _fsync_directory(directory_fd)
    except SafetyError:
        raise
    except (OSError, TypeError, ValueError) as exc:
        raise SafetyError("Campaign ledger update was not durably confirmed; install is not authorized") from exc
    finally:
        if temp_fd >= 0:
            os.close(temp_fd)
        if not published:
            with contextlib.suppress(OSError):
                os.unlink(temp_name, dir_fd=directory_fd)


def validate_device_observation(profile: str, observation: object) -> None:
    """Validate supplied measurements only; this is not device identity attestation.

    A caller must acquire these values itself under the shared reservation lock and
    revalidate the serial against the emulator. This helper runs no commands and
    does not authorize an install.
    """
    expected = {"api26": (26, 4096), "api35-4k": (35, 4096),
                "api35-16k": (35, 16384)}
    if not isinstance(profile, str) or profile not in expected or not isinstance(observation, dict) or set(observation) != {
            "serial", "avd", "api", "abi", "page_size", "host_free_bytes"}:
        raise SafetyError("Invalid device observation or profile")
    serial = observation["serial"]
    match = re.fullmatch(r"emulator-([0-9]{4,5})", serial) if isinstance(serial, str) else None
    if not match or not 5554 <= int(match.group(1)) <= 5682 or int(match.group(1)) % 2:
        raise SafetyError("Invalid emulator serial")
    api, page_size = expected[profile]
    if (observation["avd"] != PROFILE_AVD[profile] or observation["abi"] != "x86_64"
            or type(observation["api"]) is not int or observation["api"] != api
            or type(observation["page_size"]) is not int or observation["page_size"] != page_size
            or type(observation["host_free_bytes"]) is not int
            or observation["host_free_bytes"] < 8 * 1024**3):
        raise SafetyError("Device observation does not match profile or capacity policy")


def _normalize_installed_packages(value: Iterable[str]) -> set[str]:
    if isinstance(value, (str, bytes)):
        raise SafetyError("Package inventory result is malformed")
    try:
        packages = list(value)
    except (TypeError, RuntimeError) as exc:
        raise SafetyError("Package inventory could not be read") from exc
    if not packages or any(not isinstance(item, str) or not INVENTORY_PACKAGE_RE.fullmatch(item) for item in packages):
        raise SafetyError("Package inventory is empty or ambiguous; fail closed")
    if len(packages) != len(set(packages)):
        raise SafetyError("Package inventory contains duplicate entries")
    return set(packages)


def parse_package_inventory(output: bytes | str) -> set[str]:
    """Parse a complete `pm list packages`-style response; this function runs no command."""
    if isinstance(output, (bytes, str)) and len(output) > MAX_PACKAGE_INVENTORY_BYTES:
        raise SafetyError("Package inventory exceeds its size limit")
    if isinstance(output, bytes):
        try:
            text = output.decode("utf-8", "strict")
        except UnicodeError as exc:
            raise SafetyError("Package inventory output is not UTF-8") from exc
    elif isinstance(output, str):
        text = output
    else:
        raise SafetyError("Package inventory output has an invalid type")
    packages: list[str] = []
    lines = text.splitlines()
    if not lines:
        raise SafetyError("Package inventory output is empty")
    for line in lines:
        match = re.fullmatch(r"package:([A-Za-z][A-Za-z0-9_]*(?:\.[A-Za-z][A-Za-z0-9_]*)*)", line.strip())
        if not match:
            raise SafetyError("Package inventory output is malformed or incomplete")
        packages.append(match.group(1))
    return _normalize_installed_packages(packages)


@contextlib.contextmanager
def claim_install_attempt(*, ledger_directory: str | Path, verified: VerifiedCampaign,
                          installed_packages: Callable[[], bytes | str], aapt: str | Path,
                          apksigner: str | Path,
                          device_observation: Callable[[], object] | None = None,
                          expected_method: str | None = None) -> Iterator[InstallAttemptPermit]:
    """Durably consume one fresh ledger ID and hold the shared reservation lock.

    The caller must provide an inventory supplier. It is invoked while the lock is held.
    This function never invokes ADB or installs an APK. Exceptions after yield do not roll
    the ledger back; a campaign in INSTALL_ATTEMPTED is never reusable.
    """
    if not callable(installed_packages):
        raise SafetyError("A package-absence inventory callback is required")
    directory = Path(ledger_directory)
    with reservation_lock(directory, blocking=False) as directory_fd:
        cid = _require_campaign(verified.campaign_id)
        record_name = f"{cid}.json"
        record = _read_campaign_record(directory_fd, record_name)
        _validate_campaign_record(record, verified, record_name, expected_method)
        if device_observation is not None:
            if not callable(device_observation):
                raise SafetyError("Device observation callback is invalid")
            try:
                validate_device_observation(record["profile"], device_observation())
            except SafetyError:
                raise
            except Exception as exc:
                raise SafetyError("Device observation failed; install is not authorized") from exc
        aapt_path, apksigner_path = _safe_tool_pair(aapt, apksigner)
        target, test = _verify_snapshot_pair(verified, aapt_path, apksigner_path)
        if target.application_id != verified.target.application_id or test.application_id != verified.test.application_id:
            raise SafetyError("APK pair changed during locked pre-install revalidation")
        try:
            packages = parse_package_inventory(installed_packages())
        except SafetyError:
            raise
        except Exception as exc:
            raise SafetyError("Package inventory failed; install is not authorized") from exc
        if target.application_id in packages or test.application_id in packages:
            raise SafetyError("Campaign target/test package already installed; refusing update")
        record["state"] = "INSTALL_ATTEMPTED"
        record["install_attempted_utc"] = datetime.now(timezone.utc).isoformat()
        record["verified_apk_sha256"] = {"target": target.sha256, "test": test.sha256}
        record["verified_certificate_sha256"] = verified.certificate_sha256
        _atomic_update_record(directory_fd, record_name, record)
        permit = InstallAttemptPermit(
            cid, target.application_id, test.application_id, target.snapshot_path, test.snapshot_path,
            target.sha256, test.sha256, record["campaign_token"], record.get("method"),
        )
        yield permit


def campaign_token_for_execution(*, ledger_directory: str | Path, verified: VerifiedCampaign,
                                 expected_method: str) -> str:
    """Read a campaign token into process memory after strict ID/method/APK validation; never log it."""
    directory=Path(ledger_directory)
    with reservation_lock(directory,blocking=False) as directory_fd:
        cid=_require_campaign(verified.campaign_id)
        record=_read_campaign_record(directory_fd,f"{cid}.json")
        state=record.get("state")
        common=(record.get("campaign_id")==cid and record.get("profile")=="api35-4k"
            and record.get("avd")=="serea_s1_01a3" and record.get("owner_approved_scope")==S1B_OWNER_SCOPE
            and S1B_OWNER_METHODS.get(record.get("method"))==expected_method
            and record.get("target_package")==verified.target.application_id
            and record.get("test_package")==verified.test.application_id)
        if state=="INSTALL_ATTEMPTED":
            common=common and record.get("verified_apk_sha256")=={"target":verified.target.sha256,"test":verified.test.sha256}
            common=common and record.get("verified_certificate_sha256")==verified.certificate_sha256
        elif state!="PREPARED_ONLY_NOT_BUILT_NOT_INSTALLED":common=False
        token=record.get("campaign_token")
        if not common or not isinstance(token,str) or not re.fullmatch(r"[0-9a-f]{32}",token):
            raise SafetyError("Campaign token does not belong to this unconsumed runtime authorization")
        return token


def read_install_attempt(*, ledger_directory: str | Path, verified: VerifiedCampaign,
                         expected_method: str) -> dict[str, Any]:
    """Read a previously consumed install record without changing/reinstalling packages."""
    directory=Path(ledger_directory)
    with reservation_lock(directory,blocking=False) as directory_fd:
        cid=_require_campaign(verified.campaign_id)
        record=_read_campaign_record(directory_fd,f"{cid}.json")
        if (record.get("campaign_id")!=cid or record.get("state")!="INSTALL_ATTEMPTED"
                or record.get("profile")!="api35-4k" or record.get("avd")!="serea_s1_01a3"
                or record.get("owner_approved_scope")!=S1B_OWNER_SCOPE
                or S1B_OWNER_METHODS.get(record.get("method"))!=expected_method
                or record.get("verified_apk_sha256")!={"target":verified.target.sha256,"test":verified.test.sha256}
                or record.get("verified_certificate_sha256")!=verified.certificate_sha256):
            raise SafetyError("Existing install attempt is not bound to this campaign/APK pair/method")
        return {key:record[key] for key in ("campaign_id","profile","avd","target_package","test_package",
            "state","owner_approved_scope","method","verified_apk_sha256","verified_certificate_sha256")}


def mark_runtime_attempt(*, ledger_directory: str | Path, verified: VerifiedCampaign,
                         expected_method: str) -> str:
    """Record the one-shot runtime attempt immediately before dispatching instrumentation."""
    if expected_method not in S1B_OWNER_METHODS.values():
        raise SafetyError("Runtime method is outside the two Owner-approved S1B/S1-02 methods")
    directory=Path(ledger_directory)
    with reservation_lock(directory,blocking=False) as directory_fd:
        cid=_require_campaign(verified.campaign_id)
        name=f"{cid}.json"
        record=_read_campaign_record(directory_fd,name)
        if (record.get("state")!="INSTALL_ATTEMPTED"
                or record.get("owner_approved_scope")!=S1B_OWNER_SCOPE
                or S1B_OWNER_METHODS.get(record.get("method"))!=expected_method
                or record.get("verified_apk_sha256")!={"target":verified.target.sha256,"test":verified.test.sha256}
                or record.get("verified_certificate_sha256")!=verified.certificate_sha256):
            raise SafetyError("Runtime attempt is not bound to this installed campaign/APK pair/method")
        record["state"]="RUNTIME_ATTEMPTED"
        record["runtime_attempted_utc"]=datetime.now(timezone.utc).isoformat()
        record["runtime_method"]=expected_method
        _atomic_update_record(directory_fd,name,record)
    return record["campaign_token"]


def mark_runtime_not_started(*, ledger_directory: str | Path, verified: VerifiedCampaign,
                             expected_method: str, reason: str) -> None:
    """Record a proven pre-instrumentation abort without rewriting or reusing its ID."""
    allowed_reasons={"pre_instrumentation_setup_failed","adb_process_spawn_failed"}
    if expected_method not in S1B_OWNER_METHODS.values() or reason not in allowed_reasons:
        raise SafetyError("Not-started runtime classification is outside the approved synthetic states")
    directory=Path(ledger_directory)
    with reservation_lock(directory,blocking=False) as directory_fd:
        cid=_require_campaign(verified.campaign_id)
        name=f"{cid}.json"
        record=_read_campaign_record(directory_fd,name)
        state=record.get("state")
        if (state not in {"INSTALL_ATTEMPTED","RUNTIME_ATTEMPTED"}
                or record.get("owner_approved_scope")!=S1B_OWNER_SCOPE
                or S1B_OWNER_METHODS.get(record.get("method"))!=expected_method
                or record.get("verified_apk_sha256")!={"target":verified.target.sha256,"test":verified.test.sha256}
                or record.get("verified_certificate_sha256")!=verified.certificate_sha256
                or (state=="RUNTIME_ATTEMPTED" and record.get("runtime_method")!=expected_method)
                or "runtime_result_sha256" in record):
            raise SafetyError("Not-started classification is not bound to a fresh, proven pre-instrumentation failure")
        record["state"]="PREPARATION_ABORTED_NOT_EXECUTED"
        record.pop("runtime_attempted_utc",None)
        record.pop("runtime_method",None)
        record["runtime_not_started_utc"]=datetime.now(timezone.utc).isoformat()
        record["runtime_not_started_reason"]=reason
        record["runtime_cleanup_status"]="PENDING"
        _atomic_update_record(directory_fd,name,record)


def save_runtime_evidence(*, ledger_directory: str | Path, campaign_id: str,
                          output: str, campaign_token: str) -> tuple[Path,str]:
    cid=_require_campaign(campaign_id)
    if not isinstance(output,str) or not isinstance(campaign_token,str) or not re.fullmatch(r"[0-9a-f]{32}",campaign_token):
        raise SafetyError("Instrumentation evidence or campaign token is malformed")
    safe_output=output.replace(campaign_token,"[REDACTED]").encode("utf-8")
    directory=Path(ledger_directory)
    with reservation_lock(directory,blocking=False) as directory_fd:
        evidence_dir="runtime-evidence"
        try:os.mkdir(evidence_dir,0o700,dir_fd=directory_fd)
        except FileExistsError:pass
        try:
            evidence_fd=os.open(evidence_dir,os.O_RDONLY|os.O_DIRECTORY|getattr(os,"O_NOFOLLOW",0),dir_fd=directory_fd)
        except OSError as exc:raise SafetyError("Runtime evidence directory is unsafe") from exc
        try:
            info=os.fstat(evidence_fd)
            if not stat.S_ISDIR(info.st_mode) or info.st_uid!=os.getuid() or stat.S_IMODE(info.st_mode)!=0o700:
                raise SafetyError("Runtime evidence directory must be private and Owner-owned")
            name=cid+".instrumentation.txt"
            flags=os.O_WRONLY|os.O_CREAT|os.O_EXCL|getattr(os,"O_NOFOLLOW",0)|getattr(os,"O_CLOEXEC",0)
            try:fd=os.open(name,flags,0o600,dir_fd=evidence_fd)
            except FileExistsError as exc:raise SafetyError("Runtime evidence already exists; campaign may not be retried") from exc
            with os.fdopen(fd,"wb") as stream:
                stream.write(safe_output);stream.flush();os.fsync(stream.fileno())
            os.fsync(evidence_fd)
        finally:os.close(evidence_fd)
    return directory/"runtime-evidence"/name,hashlib.sha256(safe_output).hexdigest()


def mark_runtime_pass(*, ledger_directory: str | Path, verified: VerifiedCampaign,
                      expected_method: str, result_sha256: str,
                      cleanup_status: str = "CLEANUP_PASS") -> None:
    if (expected_method not in S1B_OWNER_METHODS.values()
            or not re.fullmatch(r"[0-9a-f]{64}",result_sha256)
            or cleanup_status!="CLEANUP_PASS"):
        raise SafetyError("Runtime PASS evidence is malformed")
    directory=Path(ledger_directory)
    with reservation_lock(directory,blocking=False) as directory_fd:
        cid=_require_campaign(verified.campaign_id)
        name=f"{cid}.json"
        record=_read_campaign_record(directory_fd,name)
        if (record.get("state")!="RUNTIME_ATTEMPTED"
                or record.get("owner_approved_scope")!=S1B_OWNER_SCOPE
                or S1B_OWNER_METHODS.get(record.get("method"))!=expected_method
                or record.get("runtime_method")!=expected_method
                or record.get("verified_apk_sha256")!={"target":verified.target.sha256,"test":verified.test.sha256}
                or record.get("verified_certificate_sha256")!=verified.certificate_sha256):
            raise SafetyError("Runtime PASS evidence does not match the consumed campaign attempt")
        record["state"]="RUNTIME_PASS"
        record["runtime_result_sha256"]=result_sha256
        record["runtime_cleanup_status"]=cleanup_status
        _atomic_update_record(directory_fd,name,record)


@contextlib.contextmanager
def claim_observed_install_attempt(*, ledger_directory: str | Path, verified: VerifiedCampaign,
                                   device_observation: Callable[[], object],
                                   installed_packages: Callable[[], bytes | str], aapt: str | Path,
                                   apksigner: str | Path,
                                   expected_method: str | None = None) -> Iterator[InstallAttemptPermit]:
    """Cooperative guard only: observation source and install runner are not supplied here.

    Callers must use a separately reviewed fixed runner which establishes genuine device
    identity and collects observations under the shared SDK lock. Supplying a callback is
    not proof of either condition and this API alone does not authorize device operations.
    """
    if not callable(device_observation):
        raise SafetyError("A device observation callback is required")
    with claim_install_attempt(ledger_directory=ledger_directory, verified=verified,
                               installed_packages=installed_packages, aapt=aapt, apksigner=apksigner,
                               device_observation=device_observation, expected_method=expected_method) as permit:
        yield permit


def _require_temporary_private_directory(path: str | Path, label: str) -> Path:
    directory = Path(path)
    try:
        temp_root = Path(tempfile.gettempdir()).resolve(strict=True)
        info = directory.lstat()
        resolved = directory.resolve(strict=True)
    except OSError as exc:
        raise SafetyError(f"Synthetic {label} directory is missing or unsafe") from exc
    if (resolved == temp_root or not resolved.is_relative_to(temp_root)
            or not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid()
            or stat.S_IMODE(info.st_mode) != 0o700):
        raise SafetyError(f"Offline preflight requires an Owner-owned mode-0700 {label} below the system temp root")
    return directory


def offline_synthetic_preflight(*, target_apk: str | Path, test_apk: str | Path,
                                campaign_id: str, staging_root: str | Path,
                                trust_anchor: str | Path, ledger_directory: str | Path,
                                installed_packages: bytes | str, aapt: str | Path,
                                apksigner: str | Path) -> dict[str, Any]:
    """Exercise verification plus the install gate against a synthetic temp ledger only.

    This function consumes that temporary ledger ID as INSTALL_ATTEMPTED, but deliberately
    returns no InstallAttemptPermit and has no device-operation path. It is an offline
    readiness check, not authorization to install or run a campaign.
    """
    campaign = _require_campaign(campaign_id)
    directory = _require_temporary_private_directory(ledger_directory, "offline ledger")
    snapshot_root = _require_temporary_private_directory(staging_root, "snapshot")
    if isinstance(installed_packages, bytes):
        inventory = installed_packages
    elif isinstance(installed_packages, str):
        inventory = installed_packages.encode("utf-8")
    else:
        raise SafetyError("Offline preflight requires a supplied synthetic package-inventory snapshot")
    if not inventory or len(inventory) > MAX_PACKAGE_INVENTORY_BYTES:
        raise SafetyError("Synthetic package inventory is empty or exceeds the accepted size limit")

    verified = verify_campaign_apks(
        target_apk=target_apk, test_apk=test_apk, campaign_id=campaign,
        staging_root=snapshot_root, trust_anchor=trust_anchor, aapt=aapt, apksigner=apksigner,
    )
    with claim_install_attempt(
        ledger_directory=directory, verified=verified,
        installed_packages=lambda: inventory, aapt=aapt, apksigner=apksigner,
    ) as permit:
        return {
            "schema": 1,
            "status": "OFFLINE_SYNTHETIC_PREFLIGHT_PASS",
            "purpose": "S1B_SYNTHETIC_DIAGNOSTIC_ONLY",
            "campaign_id": permit.campaign_id,
            "certificate_sha256": verified.certificate_sha256,
            "ledger_state": "INSTALL_ATTEMPTED",
            "device_operation": "NOT_RUN",
            "install_authorized": False,
            "runtime_authorized": False,
            "eligible_for_runtime_owner_review": True,
            "target": {
                "application_id": permit.target_package,
                "snapshot_path": str(permit.target_apk),
                "sha256": permit.target_sha256,
            },
            "test": {
                "application_id": permit.test_package,
                "snapshot_path": str(permit.test_apk),
                "sha256": permit.test_sha256,
            },
        }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Offline S1B signed-APK verification; never invokes ADB or installs packages.")
    subparsers = parser.add_subparsers(dest="command", required=True)
    verify = subparsers.add_parser("verify", help="snapshot and validate a synthetic diagnostic APK pair")
    verify.add_argument("--campaign-id", required=True)
    verify.add_argument("--target-apk", required=True, type=Path)
    verify.add_argument("--test-apk", required=True, type=Path)
    verify.add_argument("--staging-root", required=True, type=Path,
                        help="existing Owner-owned directory with mode 0700")
    verify.add_argument("--trust-anchor", required=True, type=Path,
                        help="external Owner-approved S1B diagnostic certificate policy (mode 0600)")
    verify.add_argument("--sdk-root", default=None)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command != "verify":
            raise SafetyError("Unsupported command")
        aapt, apksigner = resolve_android_tools(args.sdk_root)
        result = verify_campaign_apks(
            target_apk=args.target_apk, test_apk=args.test_apk, campaign_id=args.campaign_id,
            staging_root=args.staging_root,
            trust_anchor=args.trust_anchor,
            aapt=aapt, apksigner=apksigner,
        )
        print(json.dumps(result.to_public_dict(), sort_keys=True))
        return 0
    except SafetyError as exc:
        print(json.dumps({"status": "REJECTED", "error": str(exc)}, sort_keys=True), file=sys.stderr)
        return 2
    except OSError:
        print(json.dumps({"status": "REJECTED", "error": "Host file operation failed; no install authorization"},
                         sort_keys=True), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
