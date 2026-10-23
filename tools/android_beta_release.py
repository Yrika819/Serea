#!/usr/bin/env python3
"""Fail-closed helpers for the isolated Personal Beta release pipeline."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any


APPLICATION_ID = "app.serea.android.beta"
SHA256_RE = re.compile(r"(?i)^[0-9a-f]{64}$")
SHA1_RE = re.compile(r"(?i)^[0-9a-f]{40}$")


class ReleaseError(RuntimeError):
    pass


def sha256_file(path: Path) -> str:
    if not path.is_file() or path.stat().st_size == 0:
        raise ReleaseError("Required release file is missing or empty.")
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def normalize_fingerprint(value: str) -> str:
    result = re.sub(r"[:\s]", "", value).upper()
    if not SHA256_RE.fullmatch(result):
        raise ReleaseError("A valid certificate SHA-256 fingerprint is required.")
    return result


def run_quietly(command: list[str], *, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(command, check=True, capture_output=True, text=True, env=env)
    except (OSError, subprocess.CalledProcessError) as error:
        # Tool output may contain paths or certificate metadata. It must never include credentials.
        raise ReleaseError("A pinned Android release tool rejected the input or failed.") from error


def apk_identity(apk: Path, aapt: str) -> tuple[str, int]:
    output = run_quietly([aapt, "dump", "badging", str(apk)]).stdout
    match = re.search(r"^package: name='([^']+)' versionCode='([0-9]+)'", output, re.MULTILINE)
    if not match:
        raise ReleaseError("APK package identity could not be read.")
    return match.group(1), int(match.group(2))


def require_unsigned(apk: Path, apksigner: str) -> None:
    try:
        result = subprocess.run(
            [apksigner, "verify", "--verbose", str(apk)],
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError as error:
        raise ReleaseError("Pinned apksigner is unavailable.") from error
    if result.returncode == 0:
        raise ReleaseError("Expected an unsigned APK; refusing a pre-signed build artifact.")
    if "DOES NOT VERIFY" not in (result.stdout + result.stderr):
        raise ReleaseError("Unable to prove that the build artifact is an unsigned APK.")


def verify_signed(
    apk: Path, expected_sha256: str, expected_version_code: int, expected_cert: str,
    aapt: str, apksigner: str,
) -> dict[str, str]:
    expected_cert = normalize_fingerprint(expected_cert)
    digest = sha256_file(apk)
    if not SHA256_RE.fullmatch(expected_sha256) or digest.lower() != expected_sha256.lower():
        raise ReleaseError("Signed APK SHA-256 does not match its authorized value.")
    app_id, version = apk_identity(apk, aapt)
    if app_id != APPLICATION_ID:
        raise ReleaseError("APK Application ID is not the Personal Beta package.")
    if version != expected_version_code or version <= 1:
        raise ReleaseError("APK versionCode does not match the authorized monotonic release version.")
    signature = run_quietly([apksigner, "verify", "--verbose", "--print-certs", str(apk)]).stdout
    match = re.search(r"Signer #1 certificate SHA-256 digest:\s*([0-9a-fA-F:]+)", signature)
    if not match or normalize_fingerprint(match.group(1)) != expected_cert:
        raise ReleaseError("APK signing certificate does not match the Owner-pinned certificate.")
    return {
        "application_id": app_id,
        "version_code": str(version),
        "apk_sha256": digest,
        "certificate_sha256": expected_cert,
    }


def write_outputs(values: dict[str, str]) -> None:
    output = os.environ.get("GITHUB_OUTPUT")
    if output:
        with Path(output).open("a", encoding="utf-8") as stream:
            for key, value in values.items():
                if "\n" in value or "\r" in value:
                    raise ReleaseError("A release output contains an invalid line break.")
                stream.write(f"{key}={value}\n")
    else:
        for key, value in values.items():
            print(f"{key}={value}")


def create_record(args: argparse.Namespace) -> None:
    if not SHA1_RE.fullmatch(args.source_sha) or not SHA1_RE.fullmatch(args.tree_sha):
        raise ReleaseError("Source commit and tree identifiers are required.")
    if not args.run_id.isdigit() or int(args.run_id) <= 0:
        raise ReleaseError("Workflow run identifier is invalid.")
    if args.version_code <= 1 or args.version_code > 2_147_483_647:
        raise ReleaseError("Personal Beta versionCode is outside Android limits.")
    digest = sha256_file(args.apk)
    app_id, version = apk_identity(args.apk, args.aapt)
    if app_id != APPLICATION_ID:
        raise ReleaseError("Unsigned APK Application ID is not the Personal Beta package.")
    if version != args.version_code:
        raise ReleaseError("Unsigned APK versionCode does not match the authorized release version.")
    require_unsigned(args.apk, args.apksigner)
    record: dict[str, Any] = {
        "schema": 1,
        "application_id": app_id,
        "version_code": version,
        "source_sha": args.source_sha.lower(),
        "source_tree_sha": args.tree_sha.lower(),
        "workflow_run_id": args.run_id,
        "apk_sha256": digest,
        "signature_state": "unsigned",
    }
    args.record.write_text(json.dumps(record, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    write_outputs({"apk_sha256": digest, "application_id": app_id, "version_code": str(version)})


def verify_record(args: argparse.Namespace) -> None:
    try:
        record = json.loads(args.record.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ReleaseError("Unsigned APK release metadata is missing or malformed.") from error
    expected = {
        "schema": 1,
        "application_id": APPLICATION_ID,
        "version_code": args.version_code,
        "source_sha": args.source_sha.lower(),
        "source_tree_sha": args.tree_sha.lower(),
        "workflow_run_id": args.run_id,
        "apk_sha256": args.expected_sha256.lower(),
        "signature_state": "unsigned",
    }
    if record != expected:
        raise ReleaseError("Unsigned APK metadata does not match this approved workflow run.")
    if sha256_file(args.apk).lower() != args.expected_sha256.lower():
        raise ReleaseError("Unsigned APK SHA-256 does not match the Build job output.")
    app_id, version = apk_identity(args.apk, args.aapt)
    if app_id != APPLICATION_ID:
        raise ReleaseError("Unsigned APK Application ID is not the Personal Beta package.")
    if version != args.version_code:
        raise ReleaseError("Unsigned APK versionCode does not match the authorized release version.")
    require_unsigned(args.apk, args.apksigner)


def sign_apk(args: argparse.Namespace) -> None:
    verify_record(args)
    store_password = os.environ.get("SEREA_BETA_SIGNING_STORE_PASSWORD", "")
    key_password = os.environ.get("SEREA_BETA_SIGNING_KEY_PASSWORD", "")
    key_alias = os.environ.get("SEREA_BETA_SIGNING_KEY_ALIAS", "")
    if not store_password or not key_password or not key_alias:
        raise ReleaseError("Owner signing credentials are missing from the isolated Signing job.")
    expected_cert = normalize_fingerprint(args.expected_cert)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    try:
        subprocess.run(
            [
                args.apksigner, "sign", "--ks", str(args.keystore), "--ks-key-alias", key_alias,
                "--ks-pass", "env:SEREA_BETA_SIGNING_STORE_PASSWORD",
                "--key-pass", "env:SEREA_BETA_SIGNING_KEY_PASSWORD",
                "--out", str(args.output), str(args.apk),
            ],
            check=True,
            capture_output=True,
            text=True,
            env=env,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        args.output.unlink(missing_ok=True)
        raise ReleaseError("Pinned apksigner could not sign the approved unsigned APK.") from error
    try:
        result = verify_signed(
            args.output, sha256_file(args.output), args.version_code, expected_cert, args.aapt, args.apksigner,
        )
    except ReleaseError:
        args.output.unlink(missing_ok=True)
        args.output.with_suffix(args.output.suffix + ".idsig").unlink(missing_ok=True)
        raise
    write_outputs(result)


def certificate_sha256(certificate: Path, openssl: str) -> str:
    output = run_quietly([openssl, "x509", "-in", str(certificate), "-noout", "-fingerprint", "-sha256"]).stdout
    match = re.search(r"SHA256 Fingerprint=([0-9A-Fa-f:]+)", output, re.IGNORECASE)
    if not match:
        raise ReleaseError("Transfer certificate fingerprint could not be verified.")
    return normalize_fingerprint(match.group(1))


def encrypt_apk(args: argparse.Namespace) -> None:
    if certificate_sha256(args.certificate, args.openssl) != normalize_fingerprint(args.expected_cert):
        raise ReleaseError("Transfer certificate does not match its pinned fingerprint.")
    sha256_file(args.apk)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    run_quietly([
        args.openssl, "cms", "-encrypt", "-binary", "-aes-256-cbc", "-in", str(args.apk),
        "-outform", "DER", "-out", str(args.output), str(args.certificate),
    ])
    write_outputs({"ciphertext_sha256": sha256_file(args.output)})


def decrypt_apk(args: argparse.Namespace) -> None:
    if certificate_sha256(args.certificate, args.openssl) != normalize_fingerprint(args.expected_cert):
        raise ReleaseError("Transfer certificate does not match its pinned fingerprint.")
    password = os.environ.get("SEREA_BETA_TRANSPORT_KEY_PASSWORD", "")
    if not password:
        raise ReleaseError("Transfer private-key password is missing from the isolated Distribution job.")
    env = os.environ.copy()
    try:
        subprocess.run(
            [
                args.openssl, "cms", "-decrypt", "-binary", "-inform", "DER", "-in", str(args.ciphertext),
                "-recip", str(args.certificate), "-inkey", str(args.private_key),
                "-passin", "env:SEREA_BETA_TRANSPORT_KEY_PASSWORD", "-out", str(args.output),
            ],
            check=True,
            capture_output=True,
            text=True,
            env=env,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        args.output.unlink(missing_ok=True)
        raise ReleaseError("Encrypted release artifact could not be authenticated and decrypted.") from error
    digest = sha256_file(args.output)
    if digest.lower() != args.expected_sha256.lower():
        args.output.unlink(missing_ok=True)
        raise ReleaseError("Decrypted signed APK SHA-256 does not match the Signing job output.")
    write_outputs({"apk_sha256": digest})


def copy_android_scan_tree(source: Path, destination: Path) -> None:
    if source.is_symlink() or not source.is_dir() or destination.exists() or destination.is_symlink():
        raise ReleaseError("Android scanner source or destination is invalid.")

    ignored_names = {".gradle", ".idea", "build", "local.properties"}

    def ignore(directory: str, names: list[str]) -> set[str]:
        return set(names).intersection(ignored_names)

    try:
        shutil.copytree(source, destination, ignore=ignore, symlinks=True)
        if any(path.is_symlink() for path in destination.rglob("*")):
            shutil.rmtree(destination, ignore_errors=True)
            raise ReleaseError("Android scanner source tree contains symbolic links.")
    except OSError as error:
        shutil.rmtree(destination, ignore_errors=True)
        raise ReleaseError("Android scanner source tree could not be copied safely.") from error


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    record = commands.add_parser("record-unsigned")
    record.add_argument("--apk", type=Path, required=True)
    record.add_argument("--record", type=Path, required=True)
    record.add_argument("--source-sha", required=True)
    record.add_argument("--tree-sha", required=True)
    record.add_argument("--run-id", required=True)
    record.add_argument("--version-code", type=int, required=True)
    record.add_argument("--aapt", required=True)
    record.add_argument("--apksigner", required=True)
    verify = commands.add_parser("verify-unsigned")
    verify.add_argument("--apk", type=Path, required=True)
    verify.add_argument("--record", type=Path, required=True)
    verify.add_argument("--source-sha", required=True)
    verify.add_argument("--tree-sha", required=True)
    verify.add_argument("--run-id", required=True)
    verify.add_argument("--expected-sha256", required=True)
    verify.add_argument("--version-code", type=int, required=True)
    verify.add_argument("--aapt", required=True)
    verify.add_argument("--apksigner", required=True)
    sign = commands.add_parser("sign")
    for name, typ in (("apk", Path), ("record", Path), ("keystore", Path), ("output", Path)):
        sign.add_argument(f"--{name}", type=typ, required=True)
    for name in ("source-sha", "tree-sha", "run-id", "expected-sha256", "expected-cert", "aapt", "apksigner"):
        sign.add_argument(f"--{name}", required=True)
    sign.add_argument("--version-code", type=int, required=True)
    signed = commands.add_parser("verify-signed")
    signed.add_argument("--apk", type=Path, required=True)
    signed.add_argument("--expected-sha256", required=True)
    signed.add_argument("--expected-cert", required=True)
    signed.add_argument("--version-code", type=int, required=True)
    signed.add_argument("--aapt", required=True)
    signed.add_argument("--apksigner", required=True)
    encrypt = commands.add_parser("encrypt")
    encrypt.add_argument("--apk", type=Path, required=True)
    encrypt.add_argument("--certificate", type=Path, required=True)
    encrypt.add_argument("--expected-cert", required=True)
    encrypt.add_argument("--output", type=Path, required=True)
    encrypt.add_argument("--openssl", default="openssl")
    decrypt = commands.add_parser("decrypt")
    decrypt.add_argument("--ciphertext", type=Path, required=True)
    decrypt.add_argument("--certificate", type=Path, required=True)
    decrypt.add_argument("--private-key", type=Path, required=True)
    decrypt.add_argument("--expected-cert", required=True)
    decrypt.add_argument("--expected-sha256", required=True)
    decrypt.add_argument("--output", type=Path, required=True)
    decrypt.add_argument("--openssl", default="openssl")
    scan_copy = commands.add_parser("copy-android-scan-tree")
    scan_copy.add_argument("--source", type=Path, required=True)
    scan_copy.add_argument("--destination", type=Path, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "record-unsigned":
            create_record(args)
        elif args.command == "verify-unsigned":
            verify_record(args)
        elif args.command == "sign":
            sign_apk(args)
        elif args.command == "verify-signed":
            write_outputs(verify_signed(
                args.apk, args.expected_sha256, args.version_code, args.expected_cert, args.aapt, args.apksigner,
            ))
        elif args.command == "encrypt":
            encrypt_apk(args)
        elif args.command == "decrypt":
            decrypt_apk(args)
        elif args.command == "copy-android-scan-tree":
            copy_android_scan_tree(args.source, args.destination)
        return 0
    except (ReleaseError, OSError) as error:
        print(f"Personal Beta release gate failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
