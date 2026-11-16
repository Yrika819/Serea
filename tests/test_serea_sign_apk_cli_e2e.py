import argparse
import contextlib
import hashlib
import io
import json
import os
import subprocess
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest import mock

from tools import serea_signing as cli
from tools.serea_identity_registry import IdentityRegistry
from tools.serea_release_ledger import ReleaseLedger
from tools.serea_signing_witness import SigningWitness

SDK = Path.home() / "Library/Android/sdk"
TOOLS = SDK / "build-tools/35.0.0"
ANDROID_JAR = SDK / "platforms/android-35/android.jar"
JDK_BIN = Path.home() / "Library/Java/JavaVirtualMachines/temurin-17.jdk/Contents/Home/bin"


@unittest.skipUnless(all(path.is_file() for path in (TOOLS / "aapt", TOOLS / "apksigner", TOOLS / "zipalign",
                                                       ANDROID_JAR, JDK_BIN / "keytool")),
                     "Android Build Tools 35/JDK 17 unavailable")
class DisposableSignApkCliTests(unittest.TestCase):
    def create_apk(self, directory: Path, lane: str, code: int) -> Path:
        lane_directory = directory / f"fixture-{lane}-{code}"
        lane_directory.mkdir(mode=0o700)
        manifest = lane_directory / "AndroidManifest.xml"
        manifest.write_text(
            f'<manifest xmlns:android="http://schemas.android.com/apk/res/android" '
            f'package="{cli.apk_signer.APP_IDS[lane]}" android:versionCode="{code}" '
            'android:versionName="0.1.0-test"><uses-sdk android:minSdkVersion="26" '
            'android:targetSdkVersion="35"/><application android:label="Disposable"/></manifest>',
            encoding="utf-8")
        apk = lane_directory / f"{lane}-unsigned.apk"
        subprocess.run([str(TOOLS / "aapt"), "package", "-f", "-M", str(manifest), "-I",
                        str(ANDROID_JAR), "-F", str(apk)], check=True, stdout=subprocess.PIPE,
                       stderr=subprocess.PIPE, timeout=30)
        return apk

    def create_identity(self, directory: Path, lane: str) -> tuple[Path, str, dict[str, str]]:
        alias = f"{lane}-disposable"
        password = f"test-only-{lane}-passphrase"
        store = directory / f"{lane}.p12"
        subprocess.run([str(JDK_BIN / "keytool"), "-genkeypair", "-storetype", "PKCS12", "-keystore",
                        str(store), "-storepass", password, "-keypass", password, "-alias", alias,
                        "-dname", f"CN={alias}", "-validity", "2", "-keyalg", "RSA"], check=True,
                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
        certificate = directory / f"{lane}-certificate.pem"
        subprocess.run([str(JDK_BIN / "keytool"), "-exportcert", "-rfc", "-keystore", str(store),
                        "-storepass", password, "-alias", alias, "-file", str(certificate)], check=True,
                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
        return store, password, cli._cert_meta(store, alias, password), certificate

    def make_case(self, base: Path):
        vault_root = base / "vault"
        vault_root.mkdir(mode=0o700)
        os.chmod(vault_root, 0o700)
        mount = base / "mount"
        mount.mkdir(mode=0o700)
        os.chmod(mount, 0o700)
        state_dirs = [base / name for name in ("ledger", "registry", "witness")]
        for directory in state_dirs:
            directory.mkdir(mode=0o700)
            os.chmod(directory, 0o700)
        ledger_path = state_dirs[0] / "release.json"
        registry_path = state_dirs[1] / "registry.json"
        witness_path = state_dirs[2] / "witness.json"
        stores = {}
        identities = {}
        manifest_identities = {}
        certificate_directory = mount / "certificates"
        certificate_directory.mkdir(mode=0o700)
        for lane, code in (("public", 42), ("private", 71)):
            store, password, meta, certificate = self.create_identity(base, lane)
            mounted_store = mount / f"{lane}.p12"
            mounted_store.write_bytes(store.read_bytes())
            os.chmod(mounted_store, 0o600)
            stores[lane] = (mounted_store, password, f"{lane}-disposable")
            identities[lane] = {"application_id": cli.apk_signer.APP_IDS[lane],
                                 "certificate_sha256": meta["sha256"], "initial_version_code": code}
            certificate_name = "public-beta.pem" if lane == "public" else "private-beta.pem"
            mounted_certificate = certificate_directory / certificate_name
            mounted_certificate.write_bytes(certificate.read_bytes())
            os.chmod(mounted_certificate, 0o600)
            manifest_identities[lane] = {"application_id": identities[lane]["application_id"],
                                         "sha256": meta["sha256"], "sha1": meta["sha1"],
                                         "store": f"{lane}.p12", "alias": f"{lane}-disposable",
                                         "public_key_sha256": cli._public_key_digest(mounted_certificate)}
        ledger_id = str(uuid.uuid4())
        witness = SigningWitness(witness_path).initialize(mode="disposable", ledger_id=ledger_id,
                                                          identities=identities, confirm_new=True)
        ReleaseLedger.initialize(ledger_path, mode="disposable", identities=identities, ledger_id=ledger_id)
        IdentityRegistry(registry_path).initialize(mode="disposable", ledger_id=ledger_id,
                                                   identities=identities, witness=witness)
        vault_ledger = {"schema": 1, "release_ledger_id": ledger_id,
                        "release_ledger_path": str(ledger_path), "identity_registry_path": str(registry_path),
                        "signing_witness_path": str(witness_path),
                        "identities": {lane: {"application_id": identity["application_id"],
                                               "next_version_code": identity["initial_version_code"],
                                               "artifacts": []} for lane, identity in identities.items()}}
        (mount / "version-ledger.json").write_text(json.dumps(vault_ledger), encoding="utf-8")
        manifest = {"schema": 1, "mode": "disposable",
                    "enrollment": {"authority_id": "disposable-test-authority",
                                   "enrollment_id": str(uuid.uuid4()), "generation": 1},
                    "identities": manifest_identities}
        (mount / "identity-manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
        os.chmod(mount / "identity-manifest.json", 0o600)
        os.chmod(mount / "version-ledger.json", 0o600)
        return vault_root, mount, ledger_path, registry_path, witness_path, vault_ledger, manifest, stores

    def invocation(self, lane: str, code: int, apk: Path, provenance: Path, output: Path,
                   state_paths: tuple[Path, Path, Path], root: Path):
        apk_hash = hashlib.sha256(apk.read_bytes()).hexdigest()
        provenance_value = {"schema": 1, "lane": lane,
                            "variant": "beta" if lane == "public" else "privateBeta",
                            "application_id": cli.apk_signer.APP_IDS[lane], "version_code": code,
                            "version_name": "0.1.0-test", "source_commit": "a" * 40,
                            "source_tree": "b" * 40, "gradle_wrapper_sha256": "c" * 64,
                            "build_config_sha256": "d" * 64, "build_conditions": "offline",
                            "test_result": "PASS", "apk_sha256": apk_hash}
        provenance.write_text(json.dumps(provenance_value, sort_keys=True, separators=(",", ":")),
                              encoding="utf-8")
        os.chmod(provenance, 0o600)
        provenance_hash = hashlib.sha256(provenance.read_bytes()).hexdigest()
        args = cli.build_parser().parse_args([
            "sign-apk", "--disposable", "--root", str(root), "--release-ledger", str(state_paths[0]),
            "--identity-registry", str(state_paths[1]), "--witness", str(state_paths[2]),
            "--lane", lane, "--apk", str(apk), "--provenance", str(provenance),
            "--approved-apk-sha256", apk_hash, "--approved-provenance-sha256", provenance_hash,
            "--approved-source-commit", "a" * 40, "--approved-source-tree", "b" * 40,
            "--output", str(output)])
        return args

    def test_cli_signs_both_lanes_consumes_code_and_rejects_replay(self):
        with tempfile.TemporaryDirectory(prefix="serea-sign-cli-test-") as temporary:
            base = Path(temporary)
            os.chmod(base, 0o700)
            root, mount, ledger, registry, witness_path, internal, manifest, stores = self.make_case(base)
            state_paths = (ledger, registry, witness_path)
            output_dir = base / "signed"
            output_dir.mkdir(mode=0o700)
            results = {}
            evidence_records = []
            apks = {}
            provenance_paths = {}
            for lane, code in (("public", 42), ("private", 71)):
                apk = self.create_apk(base, lane, code)
                provenance = base / f"{lane}-provenance.json"
                apks[lane] = apk
                provenance_paths[lane] = provenance
                output = output_dir / f"{lane}-signed.apk"
                args = self.invocation(lane, code, apk, provenance, output, state_paths, root)
                with mock.patch.object(cli, "_root_for", return_value=root), \
                        mock.patch.object(cli, "_lock", return_value=contextlib.nullcontext()), \
                        mock.patch.object(cli, "_mounted_vault", return_value=contextlib.nullcontext(mount)), \
                        mock.patch.object(cli, "_prompts_for_lane", return_value=stores[lane][1]), \
                        mock.patch.object(cli.apk_signer, "read_provenance", wraps=cli.apk_signer.read_provenance) as read_provenance:
                    with contextlib.redirect_stdout(io.StringIO()):
                        self.assertEqual(cli.cmd_sign_apk(args), 0)
                    self.assertEqual(read_provenance.call_count, 1)
                result_record = json.loads(output.with_name(output.name + ".serea-result.json").read_text())
                self.assertEqual(result_record["result"], "PASS")
                self.assertEqual(result_record["evidence"]["apk_sha256"],
                                 hashlib.sha256(output.read_bytes()).hexdigest())
                evidence = result_record["evidence"]
                evidence_records.append({"lane": lane, **evidence})
                self.assertEqual(evidence["verified_api_levels"], [26, 35])
                results[lane] = cli.apk_signer._verified_signer(
                    output, TOOLS / "apksigner", sdk=26)
                self.assertEqual(cli.apk_signer._verified_signer(output, TOOLS / "apksigner", sdk=35),
                                 results[lane])
                self.assertEqual(evidence["certificate_sha256"], results[lane])
            self.assertNotEqual(results["public"], results["private"])
            artifact_store = ledger.parent / f".serea-artifacts-{ReleaseLedger(ledger).read()['ledger_id']}"
            receipt_files = sorted(artifact_store.glob("*/receipt.json"))
            self.assertEqual(len(receipt_files), 2)
            receipts = [json.loads(path.read_text(encoding="utf-8")) for path in receipt_files]
            self.assertEqual({receipt["state"] for receipt in receipts}, {"PUBLISHED"})
            self.assertTrue(all(Path(receipt["artifact_path"]).is_file() and
                                Path(receipt["sidecar_path"]).is_file() for receipt in receipts))
            plan_args = argparse.Namespace(disposable=True, root=str(root), release_ledger=str(ledger),
                                           identity_registry=str(registry), witness=str(witness_path))
            with mock.patch.object(cli, "_root_for", return_value=root):
                planned = io.StringIO()
                with contextlib.redirect_stdout(planned):
                    self.assertEqual(cli.cmd_ledger_recovery_plan(plan_args), 0)
            recovery_plan = json.loads(planned.getvalue())
            self.assertEqual(recovery_plan["pending"], [])
            self.assertEqual({item["state"] for item in recovery_plan["publication_receipts"]}, {"PUBLISHED"})
            print("DISPOSABLE_APK_SIGNING_EVIDENCE " + json.dumps(evidence_records, sort_keys=True))
            release_state = ReleaseLedger(ledger).read()
            self.assertEqual(release_state["lanes"]["public"]["next_version_code"], 43)
            self.assertEqual(release_state["lanes"]["private"]["next_version_code"], 72)
            self.assertEqual([event["state"] for event in release_state["events"]],
                             ["RESERVED", "ATTEMPTED", "BUILT", "RESERVED", "ATTEMPTED", "BUILT"])
            replay = self.invocation("public", 42, apks["public"], provenance_paths["public"],
                                     output_dir / "replay.apk", state_paths, root)
            with mock.patch.object(cli, "_root_for", return_value=root), \
                    mock.patch.object(cli, "_lock", return_value=contextlib.nullcontext()), \
                    mock.patch.object(cli, "_mounted_vault", return_value=contextlib.nullcontext(mount)):
                with self.assertRaises(cli.SigningError):
                    cli.cmd_sign_apk(replay)
            self.assertFalse((output_dir / "replay.apk").exists())

    def test_recovery_plan_reports_unhashable_receipt_fields_as_unknown(self):
        with tempfile.TemporaryDirectory(prefix="serea-sign-malformed-receipt-") as temporary:
            base = Path(temporary)
            os.chmod(base, 0o700)
            root, mount, ledger, registry, witness_path, internal, manifest, stores = self.make_case(base)
            ledger_id = ReleaseLedger(ledger).read()["ledger_id"]
            artifact_store = ledger.parent / f".serea-artifacts-{ledger_id}"
            artifact_store.mkdir(mode=0o700)
            attempt_id = str(uuid.uuid4())
            attempt_directory = artifact_store / attempt_id
            attempt_directory.mkdir(mode=0o700)
            receipt = {"schema": 1, "ledger_id": ledger_id, "attempt_id": attempt_id,
                       "lane": [], "version_code": 42, "state": "RESERVED",
                       "input_apk_sha256": "a" * 64, "input_provenance_sha256": "b" * 64,
                       "approved_source_commit": "c" * 40, "approved_source_tree": "d" * 40,
                       "artifact_path": str(attempt_directory / "signed.apk"),
                       "sidecar_path": str(attempt_directory / "result.json"),
                       "signed_apk_sha256": None, "sidecar_sha256": None,
                       "output_path": str(base / "signed.apk")}
            receipt_path = attempt_directory / "receipt.json"
            receipt_path.write_text(json.dumps(receipt), encoding="utf-8")
            os.chmod(receipt_path, 0o600)
            plan_args = argparse.Namespace(disposable=True, root=str(root), release_ledger=str(ledger),
                                           identity_registry=str(registry), witness=str(witness_path))
            with mock.patch.object(cli, "_root_for", return_value=root):
                planned = io.StringIO()
                with contextlib.redirect_stdout(planned):
                    self.assertEqual(cli.cmd_ledger_recovery_plan(plan_args), 0)
            plan = json.loads(planned.getvalue())
            self.assertEqual(plan["publication_receipts"][0]["attempt_id"], attempt_id)
            self.assertEqual(plan["pending"][0]["integrity"], "UNKNOWN")

    def test_recovery_plan_reports_invalid_output_path_as_unknown(self):
        with tempfile.TemporaryDirectory(prefix="serea-sign-invalid-receipt-path-") as temporary:
            base = Path(temporary)
            os.chmod(base, 0o700)
            root, mount, ledger, registry, witness_path, internal, manifest, stores = self.make_case(base)
            ledger_id = ReleaseLedger(ledger).read()["ledger_id"]
            artifact_store = ledger.parent / f".serea-artifacts-{ledger_id}"
            artifact_store.mkdir(mode=0o700)
            attempt_id = str(uuid.uuid4())
            attempt_directory = artifact_store / attempt_id
            attempt_directory.mkdir(mode=0o700)
            receipt = {"schema": 1, "ledger_id": ledger_id, "attempt_id": attempt_id,
                       "lane": "public", "version_code": 42, "state": "PUBLISHED",
                       "input_apk_sha256": "a" * 64, "input_provenance_sha256": "b" * 64,
                       "approved_source_commit": "c" * 40, "approved_source_tree": "d" * 40,
                       "artifact_path": str(attempt_directory / "signed.apk"),
                       "sidecar_path": str(attempt_directory / "result.json"),
                       "signed_apk_sha256": "e" * 64, "sidecar_sha256": "f" * 64,
                       "output_path": str(base / "bad\u0000signed.apk")}
            receipt_path = attempt_directory / "receipt.json"
            receipt_path.write_text(json.dumps(receipt), encoding="utf-8")
            os.chmod(receipt_path, 0o600)
            long_path_attempt_id = str(uuid.uuid4())
            long_path_directory = artifact_store / long_path_attempt_id
            long_path_directory.mkdir(mode=0o700)
            long_path_receipt = {**receipt, "attempt_id": long_path_attempt_id,
                                 "artifact_path": str(long_path_directory / "signed.apk"),
                                 "sidecar_path": str(long_path_directory / "result.json"),
                                 "output_path": "/" + "x" * 5000}
            long_path_receipt_path = long_path_directory / "receipt.json"
            long_path_receipt_path.write_text(json.dumps(long_path_receipt), encoding="utf-8")
            os.chmod(long_path_receipt_path, 0o600)
            plan_args = argparse.Namespace(disposable=True, root=str(root), release_ledger=str(ledger),
                                           identity_registry=str(registry), witness=str(witness_path))
            with mock.patch.object(cli, "_root_for", return_value=root):
                planned = io.StringIO()
                with contextlib.redirect_stdout(planned):
                    self.assertEqual(cli.cmd_ledger_recovery_plan(plan_args), 0)
            plan = json.loads(planned.getvalue())
            self.assertEqual(len(plan["publication_receipts"]), 2)
            self.assertTrue(all(item["diagnostics"]["integrity"] == "UNKNOWN"
                                for item in plan["publication_receipts"]))

    def test_store_mismatch_stops_before_passphrase_or_keystore_access(self):
        with tempfile.TemporaryDirectory(prefix="serea-sign-state-mismatch-") as temporary:
            base = Path(temporary)
            os.chmod(base, 0o700)
            root, mount, ledger, registry, witness_path, internal, manifest, stores = self.make_case(base)
            witness = SigningWitness(witness_path)
            current = witness.read()
            witness.consume(lane="public", version_code=42, attempt_id=str(uuid.uuid4()),
                            expected_generation=current["generation"])
            apk = self.create_apk(base, "public", 42)
            provenance = base / "public-provenance.json"
            output_dir = base / "signed"
            output_dir.mkdir(mode=0o700)
            args = self.invocation("public", 42, apk, provenance, output_dir / "result.apk",
                                   (ledger, registry, witness_path), root)
            with mock.patch.object(cli, "_root_for", return_value=root), \
                    mock.patch.object(cli, "_lock", return_value=contextlib.nullcontext()), \
                    mock.patch.object(cli, "_mounted_vault", return_value=contextlib.nullcontext(mount)), \
                    mock.patch.object(cli, "_prompts_for_lane") as prompt, \
                    mock.patch.object(cli, "_verify_one_lane") as keystore_verify:
                with self.assertRaises((cli.SigningError, cli.RegistryError)):
                    cli.cmd_sign_apk(args)
                prompt.assert_not_called()
                keystore_verify.assert_not_called()
            self.assertFalse((output_dir / "result.apk").exists())

    def test_verified_attempt_recovery_builds_and_publishes_without_resigning_then_advances_code(self):
        with tempfile.TemporaryDirectory(prefix="serea-sign-recovery-verified-") as temporary:
            base = Path(temporary)
            os.chmod(base, 0o700)
            root, mount, ledger, registry, witness_path, internal, manifest, stores = self.make_case(base)
            state_paths = (ledger, registry, witness_path)
            output_dir = base / "signed"
            output_dir.mkdir(mode=0o700)
            output = output_dir / "public-v42.apk"
            first_apk = self.create_apk(base, "public", 42)
            first_provenance = base / "public-v42-provenance.json"
            sign_args = self.invocation("public", 42, first_apk, first_provenance, output, state_paths, root)

            with mock.patch.object(cli, "_root_for", return_value=root), \
                    mock.patch.object(cli, "_lock", return_value=contextlib.nullcontext()), \
                    mock.patch.object(cli, "_mounted_vault", return_value=contextlib.nullcontext(mount)), \
                    mock.patch.object(cli, "_prompts_for_lane", return_value=stores["public"][1]), \
                    mock.patch.object(cli, "_ledger_add", side_effect=cli.SigningError("injected before Vault artifact history")):
                with self.assertRaisesRegex(cli.SigningError, "before Vault artifact history"):
                    cli.cmd_sign_apk(sign_args)

            release = ReleaseLedger(ledger).read()
            self.assertEqual([event["state"] for event in release["events"]], ["RESERVED", "ATTEMPTED"])
            attempt_id = release["events"][-1]["attempt_id"]
            receipt_path = ledger.parent / f".serea-artifacts-{release['ledger_id']}" / attempt_id / "receipt.json"
            self.assertEqual(json.loads(receipt_path.read_text())["state"], "VERIFIED")
            self.assertFalse(output.exists())
            with mock.patch.object(cli, "_root_for", return_value=root), \
                    mock.patch.object(cli, "_lock", return_value=contextlib.nullcontext()), \
                    mock.patch.object(cli, "_mounted_vault", return_value=contextlib.nullcontext(mount)):
                with self.assertRaises(cli.SigningError):
                    cli.cmd_sign_apk(sign_args)
            self.assertFalse(output.exists(), "same consumed version must remain blocked until recovery")

            recover_args = cli.build_parser().parse_args([
                "ledger-recover-publish", "--disposable", "--root", str(root),
                "--release-ledger", str(ledger), "--identity-registry", str(registry),
                "--witness", str(witness_path), "--lane", "public", "--attempt-id", attempt_id,
                "--version-code", "42", "--confirm-publish", f"PUBLISH public {attempt_id} 42",
            ])
            with mock.patch.object(cli, "_root_for", return_value=root), \
                    mock.patch.object(cli, "_mounted_vault", return_value=contextlib.nullcontext(mount)), \
                    mock.patch.object(cli, "_prompts_for_lane", return_value=stores["public"][1]), \
                    mock.patch.object(cli, "sign_apk", side_effect=AssertionError("Recovery must not re-sign")):
                with contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(cli.cmd_ledger_recover_publish(recover_args), 0)
            self.assertTrue(output.is_file())
            sidecar = output.with_name(output.name + ".serea-result.json")
            self.assertTrue(sidecar.is_file())
            self.assertEqual(json.loads(receipt_path.read_text())["state"], "PUBLISHED")
            self.assertEqual([event["state"] for event in ReleaseLedger(ledger).read()["events"]],
                             ["RESERVED", "ATTEMPTED", "BUILT"])
            self.assertEqual(cli.apk_signer._verified_signer(output, TOOLS / "apksigner", sdk=26),
                             manifest["identities"]["public"]["sha256"].replace(":", "").lower())
            self.assertEqual(cli.apk_signer._verified_signer(output, TOOLS / "apksigner", sdk=35),
                             manifest["identities"]["public"]["sha256"].replace(":", "").lower())

            next_apk = self.create_apk(base, "public", 43)
            next_provenance = base / "public-v43-provenance.json"
            next_output = output_dir / "public-v43.apk"
            next_args = self.invocation("public", 43, next_apk, next_provenance, next_output, state_paths, root)
            with mock.patch.object(cli, "_root_for", return_value=root), \
                    mock.patch.object(cli, "_lock", return_value=contextlib.nullcontext()), \
                    mock.patch.object(cli, "_mounted_vault", return_value=contextlib.nullcontext(mount)), \
                    mock.patch.object(cli, "_prompts_for_lane", return_value=stores["public"][1]), \
                    contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(cli.cmd_sign_apk(next_args), 0)
            final = ReleaseLedger(ledger).read()
            self.assertEqual(final["lanes"]["public"]["next_version_code"], 44)
            self.assertEqual([event["version_code"] for event in final["events"] if event["lane"] == "public"],
                             [42, 42, 42, 43, 43, 43])
            self.assertEqual(cli.apk_signer._verified_signer(next_output, TOOLS / "apksigner", sdk=35),
                             manifest["identities"]["public"]["sha256"].replace(":", "").lower())

    def test_built_attempt_recovery_resumes_after_sidecar_and_is_idempotent(self):
        with tempfile.TemporaryDirectory(prefix="serea-sign-recovery-publish-") as temporary:
            base = Path(temporary)
            os.chmod(base, 0o700)
            root, mount, ledger, registry, witness_path, internal, manifest, stores = self.make_case(base)
            state_paths = (ledger, registry, witness_path)
            output_dir = base / "signed"
            output_dir.mkdir(mode=0o700)
            output = output_dir / "public-v42.apk"
            apk = self.create_apk(base, "public", 42)
            provenance = base / "public-provenance.json"
            sign_args = self.invocation("public", 42, apk, provenance, output, state_paths, root)
            actual_link = os.link

            def fail_apk_publication(source, destination, *args, **kwargs):
                if Path(destination) == output:
                    raise OSError("injected crash before final APK publication")
                return actual_link(source, destination, *args, **kwargs)

            with mock.patch.object(cli, "_root_for", return_value=root), \
                    mock.patch.object(cli, "_lock", return_value=contextlib.nullcontext()), \
                    mock.patch.object(cli, "_mounted_vault", return_value=contextlib.nullcontext(mount)), \
                    mock.patch.object(cli, "_prompts_for_lane", return_value=stores["public"][1]), \
                    mock.patch.object(cli.os, "link", side_effect=fail_apk_publication):
                with self.assertRaisesRegex(cli.SigningError, "APK commit requires recovery review"):
                    cli.cmd_sign_apk(sign_args)
            release = ReleaseLedger(ledger).read()
            attempt_id = release["events"][-1]["attempt_id"]
            receipt_path = ledger.parent / f".serea-artifacts-{release['ledger_id']}" / attempt_id / "receipt.json"
            self.assertEqual(json.loads(receipt_path.read_text())["state"], "BUILT")
            published_sidecar = output.with_name(output.name + ".serea-result.json")
            self.assertTrue(published_sidecar.is_file())
            self.assertFalse(output.exists())
            recover_args = cli.build_parser().parse_args([
                "ledger-recover-publish", "--disposable", "--root", str(root),
                "--release-ledger", str(ledger), "--identity-registry", str(registry),
                "--witness", str(witness_path), "--lane", "public", "--attempt-id", attempt_id,
                "--version-code", "42", "--confirm-publish", f"PUBLISH public {attempt_id} 42",
            ])
            managed_sidecar = receipt_path.parent / "result.json"
            original_sidecar = managed_sidecar.read_bytes()
            managed_sidecar.write_text("{}", encoding="utf-8")
            with mock.patch.object(cli, "_root_for", return_value=root), \
                    mock.patch.object(cli, "_mounted_vault", return_value=contextlib.nullcontext(mount)), \
                    mock.patch.object(cli, "_prompts_for_lane", return_value=stores["public"][1]):
                with self.assertRaisesRegex(cli.SigningError, "does not match"):
                    cli.cmd_ledger_recover_publish(recover_args)
            self.assertFalse(output.exists(), "tampered managed sidecar must not publish the APK")
            managed_sidecar.write_bytes(original_sidecar)
            with mock.patch.object(cli, "_root_for", return_value=root), \
                    mock.patch.object(cli, "_mounted_vault", return_value=mount), \
                    mock.patch.object(cli, "_prompts_for_lane", return_value=stores["public"][1]), \
                    contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(cli.cmd_ledger_recover_publish(recover_args), 0)
                self.assertEqual(cli.cmd_ledger_recover_publish(recover_args), 0)
            self.assertTrue(output.is_file())
            self.assertEqual(json.loads(receipt_path.read_text())["state"], "PUBLISHED")
            self.assertEqual(cli.apk_signer._verified_signer(output, TOOLS / "apksigner", sdk=26),
                             manifest["identities"]["public"]["sha256"].replace(":", "").lower())
            self.assertEqual(len([event for event in ReleaseLedger(ledger).read()["events"]
                                  if event["state"] == "BUILT"]), 1)

    def test_built_record_failure_does_not_publish_final_apk(self):
        with tempfile.TemporaryDirectory(prefix="serea-sign-publish-failure-") as temporary:
            base = Path(temporary)
            os.chmod(base, 0o700)
            root, mount, ledger, registry, witness_path, internal, manifest, stores = self.make_case(base)
            apk = self.create_apk(base, "public", 42)
            provenance = base / "public-provenance.json"
            output_dir = base / "signed"
            output_dir.mkdir(mode=0o700)
            output = output_dir / "public-signed.apk"
            args = self.invocation("public", 42, apk, provenance, output,
                                   (ledger, registry, witness_path), root)
            with mock.patch.object(cli, "_root_for", return_value=root), \
                    mock.patch.object(cli, "_lock", return_value=contextlib.nullcontext()), \
                    mock.patch.object(cli, "_mounted_vault", return_value=contextlib.nullcontext(mount)), \
                    mock.patch.object(cli, "_prompts_for_lane", return_value=stores["public"][1]), \
                    mock.patch.object(ReleaseLedger, "mark_built", side_effect=cli.LedgerError("injected built failure")):
                with self.assertRaises(cli.LedgerError):
                    cli.cmd_sign_apk(args)
            self.assertFalse(output.exists(), "final artifact must remain unpublished when BUILT is not durable")
            self.assertEqual([event["state"] for event in ReleaseLedger(ledger).read()["events"]],
                             ["RESERVED", "ATTEMPTED"])

    def test_sigkill_after_built_attempt_preserves_consumed_code_and_discoverable_artifact(self):
        with tempfile.TemporaryDirectory(prefix="serea-sign-sigkill-") as temporary:
            base = Path(temporary)
            os.chmod(base, 0o700)
            root, mount, ledger, registry, witness_path, internal, manifest, stores = self.make_case(base)
            apk = self.create_apk(base, "public", 42)
            provenance = base / "public-provenance.json"
            output_dir = base / "signed"
            output_dir.mkdir(mode=0o700)
            output = output_dir / "public-signed.apk"
            args = self.invocation("public", 42, apk, provenance, output,
                                   (ledger, registry, witness_path), root)
            child_args = {key: value for key, value in vars(args).items() if key != "handler"}
            script = r'''import argparse, contextlib, json, os, signal, sys
from pathlib import Path
from unittest import mock
from tools import serea_signing as cli
from tools.serea_release_ledger import ReleaseLedger
data = json.loads(sys.argv[1])
data["handler"] = cli.cmd_sign_apk
args = argparse.Namespace(**data)
@contextlib.contextmanager
def mounted(*unused, **kwargs):
    yield Path(sys.argv[2])
def kill(*unused, **kwargs):
    os.kill(os.getpid(), signal.SIGKILL)
with mock.patch.object(cli, "_root_for", return_value=Path(sys.argv[3])), \
     mock.patch.object(cli, "_lock", return_value=contextlib.nullcontext()), \
     mock.patch.object(cli, "_mounted_vault", side_effect=mounted), \
     mock.patch.object(cli, "_prompts_for_lane", side_effect=lambda lane: sys.stdin.readline().strip()), \
     mock.patch.object(ReleaseLedger, "mark_built", side_effect=kill):
    cli.cmd_sign_apk(args)
'''
            child = subprocess.run([os.sys.executable, "-c", script, json.dumps(child_args), str(mount), str(root)],
                                   input=(stores["public"][1] + "\n").encode("utf-8"),
                                   cwd=Path(__file__).parents[1], stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, timeout=120, check=False)
            self.assertEqual(child.returncode, -9, child.stdout.decode("utf-8", "replace") +
                             child.stderr.decode("utf-8", "replace"))
            state = ReleaseLedger(ledger).read()
            self.assertEqual([event["state"] for event in state["events"]], ["RESERVED", "ATTEMPTED"])
            self.assertEqual(state["lanes"]["public"]["next_version_code"], 43)
            self.assertFalse(output.exists())
            store = ledger.parent / f".serea-artifacts-{state['ledger_id']}"
            receipts = list(store.glob("*/receipt.json"))
            self.assertEqual(len(receipts), 1)
            self.assertTrue((receipts[0].parent / "signed.apk").is_file())
            plan_args = argparse.Namespace(disposable=True, root=str(root), release_ledger=str(ledger),
                                           identity_registry=str(registry), witness=str(witness_path))
            with mock.patch.object(cli, "_root_for", return_value=root):
                planned = io.StringIO()
                with contextlib.redirect_stdout(planned):
                    self.assertEqual(cli.cmd_ledger_recovery_plan(plan_args), 0)
            plan = json.loads(planned.getvalue())
            self.assertEqual(plan["publication_receipts"][0]["state"], "VERIFIED")
            self.assertEqual(plan["pending"][0]["attempt_id"], state["events"][-1]["attempt_id"])

    def test_signer_failure_keeps_attempted_code_consumed(self):
        with tempfile.TemporaryDirectory(prefix="serea-sign-failed-attempt-") as temporary:
            base = Path(temporary)
            os.chmod(base, 0o700)
            root, mount, ledger, registry, witness_path, internal, manifest, stores = self.make_case(base)
            apk = self.create_apk(base, "public", 42)
            provenance = base / "public-provenance.json"
            output_dir = base / "signed"
            output_dir.mkdir(mode=0o700)
            output = output_dir / "public-signed.apk"
            args = self.invocation("public", 42, apk, provenance, output,
                                   (ledger, registry, witness_path), root)
            with mock.patch.object(cli, "_root_for", return_value=root), \
                    mock.patch.object(cli, "_lock", return_value=contextlib.nullcontext()), \
                    mock.patch.object(cli, "_mounted_vault", return_value=contextlib.nullcontext(mount)), \
                    mock.patch.object(cli, "_prompts_for_lane", return_value=stores["public"][1]), \
                    mock.patch.object(cli, "sign_apk", side_effect=cli.ArtifactError("injected signer failure")):
                with self.assertRaisesRegex(cli.ArtifactError, "injected"):
                    cli.cmd_sign_apk(args)
            state = ReleaseLedger(ledger).read()
            self.assertEqual(state["lanes"]["public"]["next_version_code"], 43)
            self.assertEqual([event["state"] for event in state["events"]], ["RESERVED", "ATTEMPTED"])
            persisted = json.loads((mount / "version-ledger.json").read_text(encoding="utf-8"))
            self.assertEqual(persisted["identities"]["public"]["next_version_code"], 43)
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
