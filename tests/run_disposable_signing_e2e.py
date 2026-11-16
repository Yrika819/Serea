"""Run end-to-end Disposable signing tests in a private local workspace.

This creates fresh test-only credentials and removes only paths created by this run.
An external recovery destination is available only through explicit --external.
It never enters formal mode, reads Keychain items, installs APKs, or enables OAuth.
"""
from __future__ import annotations

import argparse
import contextlib
import hashlib
import json
import os
import plistlib
import pty
import secrets
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO))
from tools import serea_signing as cli
from tools.serea_identity_registry import IdentityRegistry, _digest as registry_digest
from tools.serea_release_ledger import ReleaseLedger
from tools.serea_signing_witness import SigningWitness
from tools.serea_enrollment_authority import EnrollmentAuthority, generate_keypair

CLI = REPO / "tools/serea_signing.py"
PRIVATE_TMP = Path("/private/tmp")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--local", action="store_true", help="use local-only recovery in the private temp workspace (default)")
    mode.add_argument("--external", type=Path, help="use this verified mounted external volume for recovery")
    return parser


def validate_external_mount(path: Path) -> dict[str, str]:
    """Require an exact, mounted macOS volume whose diskutil identity is external."""
    if not path.is_absolute():
        raise RuntimeError("External recovery path must be absolute")
    try:
        volume = cli._canonical_no_symlinks(path, allow_missing=False)
        if volume.parent != Path("/Volumes") or not volume.is_dir() or not os.path.ismount(volume):
            raise RuntimeError("External recovery path must be a direct mounted volume under /Volumes")
        result = subprocess.run(["/usr/sbin/diskutil", "info", "-plist", str(volume)],
                                capture_output=True, check=True, timeout=20)
        info = plistlib.loads(result.stdout)
    except RuntimeError:
        raise
    except (OSError, subprocess.SubprocessError, plistlib.InvalidFileException, cli.SigningError) as exc:
        raise RuntimeError("Could not verify external recovery volume identity") from exc
    if not isinstance(info, dict) or info.get("MountPoint") != str(volume) or \
            info.get("DeviceLocation") != "External" or info.get("Internal") is not False or \
            not info.get("VolumeName") or not info.get("DeviceIdentifier"):
        raise RuntimeError("Mounted volume did not positively identify as an external device")
    return {"mountpoint": str(volume), "volume_name": str(info["VolumeName"]),
            "device_identifier": str(info["DeviceIdentifier"])}


def validate_external_unchanged(path: Path, expected: dict[str, str]) -> dict[str, str]:
    current = validate_external_mount(path)
    if current != expected:
        raise RuntimeError("External recovery volume identity changed during the E2E run")
    return current


def run_cli(args: list[str], inputs: list[str], timeout: int = 2400) -> tuple[int, str]:
    pid, master = pty.fork()
    if pid == 0:
        os.chdir(REPO)
        os.execv(sys.executable, [sys.executable, str(CLI), *args])
    pending = list(inputs)
    transcript = bytearray()
    sent_prompts = 0
    deadline = time.monotonic() + timeout
    status: int | None = None
    try:
        while status is None:
            if time.monotonic() > deadline:
                with contextlib.suppress(ProcessLookupError):
                    os.killpg(pid, signal.SIGTERM)
                grace_deadline = time.monotonic() + 5
                child_reaped = False
                while time.monotonic() < grace_deadline:
                    finished, _ = os.waitpid(pid, os.WNOHANG)
                    child_reaped = child_reaped or bool(finished)
                    try:
                        os.killpg(pid, 0)
                    except ProcessLookupError:
                        break
                    time.sleep(0.1)
                else:
                    with contextlib.suppress(ProcessLookupError):
                        os.killpg(pid, signal.SIGKILL)
                if not child_reaped:
                    os.waitpid(pid, 0)
                raise RuntimeError("Disposable CLI test timed out; its process group was terminated")
            ready, _, _ = select.select([master], [], [], 0.2)
            if ready:
                try:
                    chunk = os.read(master, 4096)
                except OSError:
                    chunk = b""
                if chunk:
                    transcript.extend(chunk)
                    prompt_count = transcript.count(b"passphrase:")
                    while pending and sent_prompts < prompt_count:
                        os.write(master, pending.pop(0).encode() + b"\n")
                        sent_prompts += 1
                        print(f"Disposable prompt {sent_prompts}/{len(inputs)} answered (value hidden).", flush=True)
            finished, child_status = os.waitpid(pid, os.WNOHANG)
            if finished:
                status = os.waitstatus_to_exitcode(child_status)
        # Collect any final nonsecret output before releasing the terminal.
        while select.select([master], [], [], 0)[0]:
            try:
                final_chunk = os.read(master, 4096)
            except OSError:
                break
            if not final_chunk:
                break
            transcript.extend(final_chunk)
        text = bytes(transcript).decode("utf-8", "replace")
        if any(secret_value and secret_value in text for secret_value in inputs):
            raise RuntimeError("A disposable password was echoed into captured output")
        if pending:
            raise RuntimeError(f"CLI exited before all expected secret prompts ({len(pending)} remaining): {text[-1200:]}")
        return status, text
    except BaseException:
        with contextlib.suppress(ProcessLookupError):
            os.killpg(pid, signal.SIGTERM)
        grace_deadline = time.monotonic() + 5
        child_reaped = False
        while time.monotonic() < grace_deadline:
            try:
                finished, _ = os.waitpid(pid, os.WNOHANG)
            except ChildProcessError:
                child_reaped = True
                break
            if finished:
                child_reaped = True
                break
            time.sleep(0.1)
        else:
            with contextlib.suppress(ProcessLookupError):
                os.killpg(pid, signal.SIGKILL)
        if not child_reaped:
            with contextlib.suppress(ChildProcessError):
                os.waitpid(pid, 0)
        raise
    finally:
        os.close(master)


def cli_hash(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_json_result(output: str) -> dict[str, object]:
    start = output.find("{")
    if start < 0:
        raise RuntimeError("Expected nonsecret JSON evidence was missing")
    value, _ = json.JSONDecoder().raw_decode(output[start:])
    return value


def require(label: str, args: list[str], inputs: list[str], *, ok: bool = True,
            timeout: int = 2400) -> str:
    code, output = run_cli(args, inputs, timeout)
    if (code == 0) != ok:
        raise RuntimeError(f"{label} unexpected exit={code}: {output[-1200:]}")
    return output


def run_gradle_guard_cases(root: Path, work: Path, passwords: dict[str, str]) -> None:
    with cli._lock(root):
        _run_gradle_guard_cases_locked(root, work, passwords)


def _run_gradle_guard_cases_locked(root: Path, work: Path, passwords: dict[str, str]) -> None:
    image = root / cli.DISPOSABLE_IMAGE_NAME
    if cli._image_is_attached(image):
        raise RuntimeError("Disposable Vault is already mounted before Guard probes")
    mountpoint = work / "guard-mount"
    mountpoint.mkdir(mode=0o700)
    os.chmod(mountpoint, 0o700)
    device: str | None = None
    attach_started = False
    try:
        attach_started = True
        attached = cli._run(["/usr/bin/hdiutil", "attach", "-plist", "-stdinpass", "-mountpoint",
                             mountpoint, image], data=(passwords["vault"] + "\0").encode(), timeout=120)
        metadata = plistlib.loads(attached.stdout)
        entities = metadata.get("system-entities", [])
        matching = [item for item in entities
                    if item.get("mount-point") and
                    cli._canonical_mountpoint(str(item["mount-point"])) == cli._canonical_mountpoint(mountpoint)
                    and item.get("dev-entry")]
        if len(matching) != 1:
            raise RuntimeError("Disposable guard-test mount did not match its requested mountpoint")
        device = str(matching[0]["dev-entry"])
        manifest = cli._load_manifest_from_mount(mountpoint, disposable=True)
        identities = manifest["identities"]
        public = identities["public"]
        private = identities["private"]
        flags = ["--no-daemon", "--no-configuration-cache", "--no-build-cache", "--max-workers=2",
                 "--console=plain", "--offline"]

        def lane_env(lane: str) -> dict[str, str]:
            env = cli._secure_env()
            if lane == "public":
                values = cli.GRADLE_VARIABLES["public"]
                entry = public
                secret = passwords["public"]
                env.update({values["path"]: str(mountpoint / entry["store"]),
                            values["store_password"]: secret, values["alias"]: entry["alias"],
                            values["key_password"]: secret, values["fingerprint"]: entry["sha256"]})
            else:
                values = cli.GRADLE_VARIABLES["private"]
                entry = private
                secret = passwords["private"]
                env.update({cli.GRADLE_VARIABLES["public"]["fingerprint"]: public["sha256"],
                            values["path"]: str(mountpoint / entry["store"]),
                            values["store_password"]: secret, values["alias"]: entry["alias"],
                            values["key_password"]: secret, values["fingerprint"]: entry["sha256"]})
            return env

        cases = []
        env = lane_env("public")
        env[cli.GRADLE_VARIABLES["public"]["fingerprint"]] = "0" * 64
        cases.append(("Public wrong fingerprint", env, "Beta keystore certificate does not match configured SHA-256.",
                      "-PsereaBetaVersionCode=2", ":app:validatePublicBetaSigning"))
        env = lane_env("private")
        env[cli.GRADLE_VARIABLES["private"]["fingerprint"]] = "0" * 64
        cases.append(("Private wrong fingerprint", env, "Beta keystore certificate does not match configured SHA-256.",
                      "-PsereaPrivateBetaVersionCode=2", ":app:validatePrivateBetaSigning"))
        env = lane_env("private")
        env[cli.GRADLE_VARIABLES["public"]["fingerprint"]] = private["sha256"]
        env[cli.GRADLE_VARIABLES["private"]["fingerprint"]] = private["sha256"]
        cases.append(("same Public/Private signer", env, "Public and Private Beta signing certificates must differ.",
                      "-PsereaPrivateBetaVersionCode=2", ":app:validatePrivateBetaSigning"))
        env = cli._secure_env()
        cases.append(("missing Public signing inputs", env, "Beta packaging blocked; missing",
                      "-PsereaBetaVersionCode=2", ":app:validatePublicBetaSigning"))
        env = cli._secure_env()
        env[cli.GRADLE_VARIABLES["public"]["fingerprint"]] = public["sha256"]
        cases.append(("missing Private signing inputs", env, "Beta packaging blocked; missing",
                      "-PsereaPrivateBetaVersionCode=2", ":app:validatePrivateBetaSigning"))
        env = lane_env("private")
        env[cli.GRADLE_VARIABLES["private"]["store_password"]] = passwords["public"]
        env[cli.GRADLE_VARIABLES["private"]["key_password"]] = passwords["public"]
        cases.append(("Private wrong PKCS12 password", env,
                      "Beta PKCS12 keystore could not be opened with the configured password.",
                      "-PsereaPrivateBetaVersionCode=2", ":app:validatePrivateBetaSigning"))

        with cli._isolated_gradle_user_home() as gradle_user_home:
            with cli._build_lock():
                for label, env, expected, version_arg, task in cases:
                    env["GRADLE_USER_HOME"] = str(gradle_user_home)
                    result = cli._run([cli.ANDROID_ROOT / "gradlew", *flags, version_arg, task], env=env,
                                      cwd=cli.ANDROID_ROOT, timeout=600, check=False)
                    captured = (result.stdout + result.stderr).decode("utf-8", "replace")
                    if result.returncode == 0 or expected.lower() not in captured.lower():
                        redacted = captured
                        for value in passwords.values():
                            if value:
                                redacted = redacted.replace(value, "[redacted disposable value]")
                        raise RuntimeError(f"Gradle Guard case failed: {label}: {redacted[-800:]}")
                    if any(value and value in captured for value in passwords.values()):
                        raise RuntimeError("A disposable password appeared in captured Guard output")
                    for name in list(env):
                        if "PASSWORD" in name:
                            env.pop(name, None)
                    print(f"Gradle Guard {label}: REJECTED as expected.", flush=True)
    finally:
        if attach_started:
            if device is None:
                try:
                    target = str(image.resolve())
                    entry = next((item for item in cli._attached_images()
                                  if str(item.get("image-path", "")) == target), None)
                    entities = entry.get("system-entities", []) if entry else []
                    matching = [item for item in entities
                                if item.get("mount-point") and
                                cli._canonical_mountpoint(str(item["mount-point"])) == cli._canonical_mountpoint(mountpoint)
                                and item.get("dev-entry")]
                    device = str(matching[0]["dev-entry"]) if len(matching) == 1 else None
                except (cli.SigningError, OSError, plistlib.InvalidFileException):
                    print("WARNING: Guard probe could not verify mount state; inspect hdiutil info manually.", file=sys.stderr)
            if device:
                cli._run(["/usr/bin/hdiutil", "detach", device], timeout=120)
                if mountpoint.is_dir() and not mountpoint.is_symlink():
                    mountpoint.rmdir()
            elif cli._image_is_attached(image):
                print(f"WARNING: Guard probe image remains attached; retain {work} for manual unmount.", file=sys.stderr)
            elif mountpoint.is_dir() and not mountpoint.is_symlink():
                mountpoint.rmdir()


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    external_identity = validate_external_mount(args.external) if args.external is not None else None
    if any((REPO / "android/app/build/outputs/apk" / name).exists() for name in ("beta", "privateBeta")):
        raise RuntimeError("Refusing to use pre-existing Beta APK output directories")

    a = secrets.token_urlsafe(32)
    b = secrets.token_urlsafe(32)
    c = secrets.token_urlsafe(32)
    wrong = secrets.token_urlsafe(32)
    if len({a, b, c, wrong}) != 4:
        raise RuntimeError("Random disposable credential collision")

    work = Path(tempfile.mkdtemp(prefix="serea-r14-disposable-", dir=PRIVATE_TMP))
    os.chmod(work, 0o700)
    root = work / "vault"
    ledger_dir = work / "release-ledger"
    registry_dir = work / "identity-registry"
    witness_dir = work / "signing-witness"
    for private_dir in (ledger_dir, registry_dir, witness_dir):
        private_dir.mkdir(mode=0o700)
        os.chmod(private_dir, 0o700)
    release_ledger = ledger_dir / "release.json"
    identity_registry = registry_dir / "identity.json"
    signing_witness = witness_dir / "witness.json"
    external_root = Path(external_identity["mountpoint"]) if external_identity else None
    recovery_dir = (external_root / f"Serea-Disposable-Recovery-{uuid.uuid4()}") \
        if external_root else work / "recovery"
    backup = recovery_dir / "Disposable-Recovery.dmg"
    restore = work / "restored-copy.dmg"
    created_outputs: list[tuple[Path, int, int, str]] = []
    created_recovery_dir = False
    recovery_dir_identity: tuple[int, int] | None = None
    results: list[str] = []
    authority_dir = work / "disposable-authority"
    authority_dir.mkdir(mode=0o700)
    os.chmod(authority_dir, 0o700)
    authority_private = authority_dir / "authority-private.pem"
    authority_public = authority_dir / "authority-public.pem"
    authority_state_path = authority_dir / "authority-state.json"
    generate_keypair(authority_private, authority_public)
    authority = EnrollmentAuthority(authority_state_path, authority_private, authority_public, mode="disposable")
    authority.create(mode="disposable")
    enrollment_id = str(uuid.uuid4())
    enrollment_slot = authority.reserve_enrollment(enrollment_id)
    enrollment_slot_path = authority_dir / "enrollment-slot.json"
    enrollment_slot_path.write_text(json.dumps(enrollment_slot, sort_keys=True), encoding="utf-8")
    os.chmod(enrollment_slot_path, 0o600)
    try:
        print("Running disposable Vault initialization...", flush=True)
        require("vault init", ["init", "--disposable", "--root", str(root)], [a, a])
        results.append("disposable AES-256/APFS Vault init: PASS")
        print("Running independent disposable PKCS12 provisioning...", flush=True)
        require("key provisioning", ["provision", "--disposable", "--root", str(root),
                "--enrollment-slot-record", str(enrollment_slot_path), "--authority-public-key", str(authority_public)],
                [a, b, b, c, c])
        results.append("independent Public/Private PKCS12 provisioning: PASS")
        prebind_snapshot = work / "prebinding-vault.dmg"
        shutil.copy2(root / cli.DISPOSABLE_IMAGE_NAME, prebind_snapshot)
        os.chmod(prebind_snapshot, 0o600)
        fingerprint_args = ["fingerprint", "--disposable", "--root", str(root)]
        fingerprint_evidence = parse_json_result(require("fingerprints", fingerprint_args, [a, b, c]))
        def authority_response(lane: str, attempt_id: str | None = None) -> Path:
            state = ReleaseLedger(release_ledger).read()
            identity = state["lanes"][lane]
            commit, tree = cli._source_revisions()
            request = {"request_id": str(uuid.uuid4()), "nonce": secrets.token_hex(32), "mode": "disposable",
                       "lane": lane, "application_id": identity["application_id"],
                       "certificate_sha256": identity["certificate_sha256"], "ledger_id": state["ledger_id"],
                       "version_code": identity["next_version_code"], "attempt_id": attempt_id or str(uuid.uuid4()),
                       "operation": "sign-apk", "source_commit": commit, "source_tree": tree}
            signed = authority.authorize(request)
            output = authority_dir / f"authorization-{lane}-{request['attempt_id']}.json"
            output.write_text(json.dumps(signed, sort_keys=True), encoding="utf-8")
            os.chmod(output, 0o600)
            return output

        ledger_id = str(uuid.uuid4())
        identities = {lane: {"application_id": fingerprint_evidence[lane]["application_id"],
                             "certificate_sha256": fingerprint_evidence[lane]["sha256"],
                             "initial_version_code": 2} for lane in ("public", "private")}
        enrollment_record = authority.enroll(enrollment_id, str(uuid.uuid4()), identities, ledger_id=ledger_id)
        enrollment_path = authority_dir / "enrollment.json"
        enrollment_path.write_text(json.dumps(enrollment_record, sort_keys=True), encoding="utf-8")
        os.chmod(enrollment_path, 0o600)
        authority_args = ["--authority-public-key", str(authority_public), "--enrollment-record", str(enrollment_path)]
        require("signed Disposable enrollment binds witness", ["witness-init", "--disposable", "--root", str(root),
                "--release-ledger", str(release_ledger), "--identity-registry", str(identity_registry),
                "--witness", str(signing_witness), *authority_args,
                "--public-version-code", "2", "--private-version-code", "2"], [a, b, c])
        require("independent Release Ledger and Registry initialization", ["ledger-init", "--disposable", "--root", str(root),
                "--release-ledger", str(release_ledger), "--identity-registry", str(identity_registry),
                "--witness", str(signing_witness), *authority_args,
                "--public-version-code", "2", "--private-version-code", "2"], [a, b, c])
        results.append("independent witness, Identity Registry and Release Ledger bound outside encrypted Vault: PASS")
        print("Running disposable Public/Private Gradle Guard negative cases...", flush=True)
        run_gradle_guard_cases(root, work, {"vault": a, "public": b, "private": c})
        results.append("Public/Private wrong fingerprint, same signer, missing inputs, and Private wrong password Guards: PASS")
        results.append("Public/Private certificate identities and public keys distinct: PASS")
        require("wrong Vault passphrase", fingerprint_args, [wrong], ok=False)
        results.append("wrong Vault passphrase: REJECTED")
        require("wrong PKCS12 passphrase", fingerprint_args, [a, wrong, c], ok=False)
        results.append("wrong PKCS12 passphrase: REJECTED")

        release_state = ReleaseLedger(release_ledger).read()
        witness_state = SigningWitness(signing_witness).read()
        failed_attempt = str(uuid.uuid4())
        failed_code = release_state["lanes"]["public"]["next_version_code"]
        failed_authorization = authority_response("public", failed_attempt)
        SigningWitness(signing_witness).consume(lane="public", version_code=failed_code,
                                                attempt_id=failed_attempt,
                                                expected_generation=witness_state["generation"])
        require("explicit recovery after durable witness reservation", ["ledger-abandon", "--disposable",
                "--root", str(root), "--release-ledger", str(release_ledger), "--identity-registry",
                str(identity_registry), "--witness", str(signing_witness), "--lane", "public",
                "--attempt-id", failed_attempt, "--version-code", str(failed_code),
                "--reason", "process-crash-reviewed", "--confirm", f"ABANDON {failed_attempt} {failed_code}"], [a])
        recovered = ReleaseLedger(release_ledger).read()
        if recovered["lanes"]["public"]["next_version_code"] != failed_code + 1:
            raise RuntimeError("Abandoned versionCode was not permanently consumed")
        results.append("witness-only interrupted reservation reconciled and permanently abandoned: PASS")

        print("Running disposable Public APK build...", flush=True)
        public_authorization = authority_response("public")
        public_output = require("Public APK build", ["build-public", "--disposable", "--root", str(root),
                                "--release-ledger", str(release_ledger), "--identity-registry", str(identity_registry),
                                "--witness", str(signing_witness), "--authorization-record", str(public_authorization),
                                "--authority-public-key", str(authority_public)], [a, b], timeout=2400)
        public_apk_evidence = parse_json_result(public_output)
        public_dir = REPO / "android/app/build/outputs/apk/beta"
        public_stat = public_dir.stat()
        created_outputs.append((public_dir, public_stat.st_dev, public_stat.st_ino,
                                str(public_apk_evidence["apk_sha256"])))
        results.append("disposable Public APK build and identity verification: PASS")
        print("Running disposable Private APK build...", flush=True)
        private_authorization = authority_response("private")
        private_output = require("Private APK build", ["build-private", "--disposable", "--root", str(root),
                                 "--release-ledger", str(release_ledger), "--identity-registry", str(identity_registry),
                                 "--witness", str(signing_witness), "--authorization-record", str(private_authorization),
                                 "--authority-public-key", str(authority_public)], [a, c], timeout=2400)
        private_apk_evidence = parse_json_result(private_output)
        private_dir = REPO / "android/app/build/outputs/apk/privateBeta"
        private_stat = private_dir.stat()
        created_outputs.append((private_dir, private_stat.st_dev, private_stat.st_ino,
                                str(private_apk_evidence["apk_sha256"])))
        results.append("disposable Private APK build and identity verification: PASS")

        missing_registry_dir = work / "missing-registry"
        missing_witness_dir = work / "missing-witness"
        bad_registry_dir = work / "mismatched-registry"
        bad_ledger_dir = work / "mismatched-ledger"
        bad_witness_dir = work / "mismatched-witness"
        stale_root = work / "stale-prebinding-vault"
        stale_ledger_dir = work / "stale-ledger"
        stale_registry_dir = work / "stale-registry"
        for directory in (missing_registry_dir, missing_witness_dir, bad_registry_dir, bad_ledger_dir,
                          bad_witness_dir, stale_root, stale_ledger_dir, stale_registry_dir):
            directory.mkdir(mode=0o700)
            os.chmod(directory, 0o700)
        require("missing Registry is not treated as first registration", ["build-public", "--disposable",
                "--root", str(root), "--release-ledger", str(release_ledger), "--identity-registry",
                str(missing_registry_dir / "identity.json"), "--witness", str(signing_witness)], [a], ok=False)
        require("missing witness is not treated as first registration", ["build-public", "--disposable",
                "--root", str(root), "--release-ledger", str(release_ledger), "--identity-registry",
                str(identity_registry), "--witness", str(missing_witness_dir / "witness.json")], [a], ok=False)
        bad_registry_path = bad_registry_dir / "identity.json"
        bad_registry_data = json.loads(identity_registry.read_text(encoding="utf-8"))
        bad_registry_data["lanes"]["public"]["certificate_sha256"] = "3" * 64
        bad_registry_data["head_sha256"] = registry_digest(bad_registry_data)
        bad_registry_path.write_text(json.dumps(bad_registry_data), encoding="utf-8")
        os.chmod(bad_registry_path, 0o600)
        require("mismatched Registry signer identity rejection", ["build-public", "--disposable", "--root",
                str(root), "--release-ledger", str(release_ledger), "--identity-registry", str(bad_registry_path),
                "--witness", str(signing_witness)], [a], ok=False)
        bad_ledger_path = bad_ledger_dir / "release.json"
        shutil.copy2(release_ledger, bad_ledger_path)
        require("replacement Ledger path rejection", ["build-public", "--disposable", "--root", str(root),
                "--release-ledger", str(bad_ledger_path), "--identity-registry", str(identity_registry),
                "--witness", str(signing_witness)], [a], ok=False)
        bad_witness_path = bad_witness_dir / "witness.json"
        bad_witness_data = json.loads(signing_witness.read_text(encoding="utf-8"))
        bad_witness_data["witness_id"] = str(uuid.uuid4())
        witness_body = {key: value for key, value in bad_witness_data.items() if key != "head_sha256"}
        bad_witness_data["head_sha256"] = hashlib.sha256(json.dumps(
            witness_body, sort_keys=True, separators=(",", ":")).encode("ascii")).hexdigest()
        bad_witness_path.write_text(json.dumps(bad_witness_data), encoding="utf-8")
        os.chmod(bad_witness_path, 0o600)
        require("mismatched witness identity rejection", ["build-public", "--disposable", "--root", str(root),
                "--release-ledger", str(release_ledger), "--identity-registry", str(identity_registry),
                "--witness", str(bad_witness_path)], [a], ok=False)
        results.append("missing/mismatched Registry, Ledger and witness signing refusal: PASS")

        stale_image = stale_root / cli.DISPOSABLE_IMAGE_NAME
        shutil.copy2(prebind_snapshot, stale_image)
        os.chmod(stale_image, 0o600)
        require("pre-bind Vault cannot be rebound after witnessed consumption", ["ledger-init", "--disposable",
                "--root", str(stale_root), "--release-ledger", str(stale_ledger_dir / "release.json"),
                "--identity-registry", str(stale_registry_dir / "identity.json"), "--witness", str(signing_witness),
                "--public-version-code", "2", "--private-version-code", "2"], [a, b, c], ok=False)
        if (stale_ledger_dir / "release.json").exists() or (stale_registry_dir / "identity.json").exists():
            raise RuntimeError("Stale pre-binding Vault created replacement identity state")
        stale_restore = work / "stale-restore.dmg"
        require("pre-binding Vault restore is rejected", ["restore", "--disposable", "--root", str(root),
                "--backup", str(prebind_snapshot), "--destination", str(stale_restore)], [b, c, a], ok=False)
        if stale_restore.exists():
            raise RuntimeError("Rejected stale Vault restore left a candidate image")
        results.append("pre-bind Vault + new Ledger/Registry and stale restore rejected: PASS")

        if external_identity:
            if external_root is None:
                raise RuntimeError("External recovery identity has no verified mountpoint")
            external_identity = validate_external_unchanged(external_root, external_identity)
        recovery_dir.mkdir(mode=0o700)
        os.chmod(recovery_dir, 0o700)
        recovery_dir_identity = (recovery_dir.stat().st_dev, recovery_dir.stat().st_ino)
        created_recovery_dir = True
        require("disposable recovery backup", ["backup", "--disposable", "--root", str(root),
                                 "--destination", str(backup)], [b, c, a, a])
        results.append("encrypted DMG copy/hash/read-only restore from " +
                       (f"external volume {external_identity['volume_name']}: PASS" if external_identity
                        else "local disposable workspace: PASS"))
        require("local isolated restore", ["restore", "--disposable", "--root", str(root),
                                           "--backup", str(backup), "--destination", str(restore)],
                [b, c, a])
        results.append("isolated restore from recovery copy: PASS")
        restore.write_bytes(b"deliberately-corrupt-disposable-image")
        require("corrupt backup rejection", ["verify-backup", "--disposable", "--root", str(root),
                                              "--backup", str(restore)], [b, c, a], ok=False)
        results.append("tampered encrypted-image copy rejection: PASS")
        require("lock", ["lock", "--disposable", "--root", str(root)], [])
        results.append("normal unmount/lock: PASS")
        print("\n".join(results))
        print("Disposable cert fingerprints (not production identities):")
        print(json.dumps(fingerprint_evidence, indent=2, sort_keys=True))
        print("Disposable APK identity/hash evidence:")
        print(json.dumps({"public": public_apk_evidence, "private": private_apk_evidence}, indent=2, sort_keys=True))
        if external_identity:
            print("External recovery was validated with a disposable encrypted image only.")
        else:
            print("Local recovery was validated with a disposable encrypted image only; no external recovery was tested.")
        return 0
    finally:
        # CLI commands normally unmount in context managers; do not force-detach on failure.
        retained_mount = False
        for image in (root / "Serea-Disposable-Signing.dmg", restore, backup):
            if image.exists() and not image.is_symlink():
                try:
                    attached_paths = {str(item.get("image-path", "")) for item in cli._attached_images()}
                except (cli.SigningError, OSError, plistlib.InvalidFileException):
                    attached_paths = {str(image)}
                if str(image.resolve()) in attached_paths:
                    print(f"WARNING: disposable image remains attached; retain for manual detach: {image}", file=sys.stderr)
                    retained_mount = True
                    continue
                image.unlink()
            pending = image.with_name(image.name + ".serea-pending")
            if pending.is_file() and not pending.is_symlink():
                pending.unlink()
        for output, device, inode, expected_apk_hash in created_outputs:
            if output.is_dir() and not output.is_symlink():
                current = output.stat()
                apks = list(output.glob("*.apk"))
                if (current.st_dev, current.st_ino) == (device, inode) and len(apks) == 1 and \
                        cli_hash(apks[0]) == expected_apk_hash:
                    shutil.rmtree(output)
                else:
                    print(f"WARNING: retained APK output changed since this run: {output}", file=sys.stderr)
        if created_recovery_dir and recovery_dir.is_dir() and not recovery_dir.is_symlink() and \
                recovery_dir_identity == (recovery_dir.stat().st_dev, recovery_dir.stat().st_ino):
            try:
                recovery_dir.rmdir()
            except OSError:
                print(f"WARNING: retained non-empty test directory: {recovery_dir}", file=sys.stderr)
        if work.exists() and not work.is_symlink() and not retained_mount:
            shutil.rmtree(work)
        a = b = c = wrong = ""


if __name__ == "__main__":
    raise SystemExit(main())
