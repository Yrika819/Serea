import argparse
import contextlib
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest import mock

from tools import serea_signing as cli
from tools.serea_identity_registry import IdentityRegistry
from tools.serea_release_ledger import ReleaseLedger
from tools.serea_signing_witness import SigningWitness
from tests import run_disposable_signing_e2e as e2e


class SigningCliSafetyTests(unittest.TestCase):
    def test_signing_blocks_latest_built_attempt_when_managed_receipt_is_missing(self):
        ledger = {"events": [{"lane": "public", "attempt_id": str(uuid.uuid4()),
                              "version_code": 42, "state": "BUILT", "artifact_sha256": "a" * 64}]}
        with self.assertRaisesRegex(cli.SigningError, "no complete managed publication receipt"):
            cli._assert_latest_publications_resolved(ledger, Path("/tmp/not-used"), [])

    def test_published_receipt_with_missing_artifacts_is_unknown(self):
        with tempfile.TemporaryDirectory() as temporary:
            attempt = Path(temporary) / "attempt"
            attempt.mkdir(mode=0o700)
            receipt = {"attempt_id": str(uuid.uuid4()), "lane": "public", "version_code": 42,
                       "state": "PUBLISHED", "signed_apk_sha256": "a" * 64,
                       "sidecar_sha256": "b" * 64, "output_path": str(Path(temporary) / "public.apk")}
            event = {"lane": "public", "version_code": 42, "state": "BUILT",
                     "artifact_sha256": "a" * 64}
            diagnosis = cli._publication_diagnostics(receipt, attempt, event)
            self.assertEqual(diagnosis["integrity"], "UNKNOWN")
            self.assertFalse(diagnosis["managed_apk_matches_receipt"])

    def test_atomic_json_syncs_parent_directory_after_replacement(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            target = parent / "state.json"
            target.write_text('{"old": true}', encoding="utf-8")
            with mock.patch.object(cli, "_fsync_directory", wraps=cli._fsync_directory) as sync_directory:
                cli._atomic_json(target, {"new": True})
            self.assertEqual(json.loads(target.read_text(encoding="utf-8")), {"new": True})
            sync_directory.assert_called_once_with(parent)

    def test_direct_cli_script_imports_registry_and_witness_modules(self):
        result = subprocess.run(["python3", str(Path(__file__).parents[1] / "tools/serea_signing.py"), "--help"],
                                capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("witness-init", result.stdout)

    def test_beta_gradle_variants_never_consume_signing_secrets_or_apply_a_signer(self):
        build_script = Path(__file__).parents[1] / "android/app/build.gradle.kts"
        source = build_script.read_text(encoding="utf-8")
        beta_section = source[source.index('create("beta")'):source.index('create("privateBeta")')]
        private_section = source[source.index('create("privateBeta")'):source.index("    }\n    if (isolatedNativeRuntimeOptIn", source.index('create("privateBeta")'))]
        self.assertNotIn("signingConfigs.getByName", beta_section)
        self.assertNotIn("signingConfigs.getByName", private_section)
        self.assertIn("signingConfig = null", beta_section)
        self.assertIn("signingConfig = null", private_section)
        for credential in (
            "SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PATH",
            "SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD",
            "SEREA_ANDROID_PERSONAL_BETA_KEY_ALIAS",
            "SEREA_ANDROID_PERSONAL_BETA_KEY_PASSWORD",
            "SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256",
            "SEREA_ANDROID_PRIVATE_BETA_KEYSTORE_PATH",
            "SEREA_ANDROID_PRIVATE_BETA_KEYSTORE_PASSWORD",
            "SEREA_ANDROID_PRIVATE_BETA_KEY_ALIAS",
            "SEREA_ANDROID_PRIVATE_BETA_KEY_PASSWORD",
            "SEREA_ANDROID_PRIVATE_BETA_CERT_SHA256",
        ):
            self.assertNotIn(credential, source)
        self.assertNotIn("outputFileName", source)
        self.assertIn('if (privateBetaRequested && ciBuild && !privateBetaUnitTestOnlyInvocation)', source)
        self.assertIn(
            'if (ciBuild && variant.buildType == "privateBeta" && !privateBetaUnitTestOnlyInvocation) {\n'
            '            variant.enable = false\n',
            source,
        )
        self.assertIn('dependsOn(validatePrivateBetaUnsigned)', source)
        self.assertIn('dependsOn(validatePublicBetaUnsigned)', source)

    def test_production_source_provenance_rejects_dirty_worktree(self):
        revision = mock.Mock(stdout="a" * 40 + "\n")
        tree = mock.Mock(stdout="b" * 40 + "\n")
        dirty = mock.Mock(stdout="?? untracked-source.py\n")
        with mock.patch.object(cli.subprocess, "run", side_effect=[revision, tree, dirty]):
            with self.assertRaises(cli.SigningError):
                cli._source_revisions(require_clean=True)

    def test_signing_reservation_consumes_all_stores_once_without_reusing_version(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            root = base / "vault"
            root.mkdir(mode=0o700)
            os.chmod(root, 0o700)
            directories = [base / name for name in ("ledger", "registry", "witness")]
            for directory in directories:
                directory.mkdir(mode=0o700)
                os.chmod(directory, 0o700)
            identities = {
                "public": {"application_id": "app.serea.android.beta", "certificate_sha256": "1" * 64,
                           "initial_version_code": 41},
                "private": {"application_id": "app.serea.android.privatebeta", "certificate_sha256": "2" * 64,
                            "initial_version_code": 71},
            }
            ledger_id = str(uuid.uuid4())
            ledger_path = directories[0] / "release.json"
            registry_path = directories[1] / "identity.json"
            witness_path = directories[2] / "witness.json"
            witness = SigningWitness(witness_path).initialize(
                mode="disposable", ledger_id=ledger_id, identities=identities, confirm_new=True)
            ledger_state = ReleaseLedger.initialize(ledger_path, mode="disposable", identities=identities,
                                                    ledger_id=ledger_id)
            registry = IdentityRegistry(registry_path).initialize(
                mode="disposable", ledger_id=ledger_id, identities=identities, witness=witness)
            manifest = {"identities": {
                lane: {"application_id": data["application_id"], "sha256": data["certificate_sha256"]}
                for lane, data in identities.items()}}
            vault_ledger = {"schema": 1, "release_ledger_id": ledger_id,
                            "release_ledger_path": str(ledger_path), "identity_registry_path": str(registry_path),
                            "signing_witness_path": str(witness_path),
                            "identities": {lane: {"application_id": data["application_id"],
                                                   "next_version_code": data["initial_version_code"],
                                                   "artifacts": []}
                                           for lane, data in identities.items()}}
            reserved = cli._reserve_signing_code(
                lane="public", release_ledger=ReleaseLedger(ledger_path),
                registry=IdentityRegistry(registry_path), witness=SigningWitness(witness_path),
                vault_ledger=vault_ledger, manifest=manifest, mode="disposable")
            self.assertEqual(reserved["version_code"], 41)
            self.assertEqual(vault_ledger["identities"]["public"]["next_version_code"], 42)
            self.assertEqual(ReleaseLedger(ledger_path).read()["lanes"]["public"]["next_version_code"], 42)
            self.assertEqual(IdentityRegistry(registry_path).read()["lanes"]["public"]["high_watermark"], 41)
            self.assertEqual(SigningWitness(witness_path).read()["lanes"]["public"]["high_watermark"], 41)
            with self.assertRaises(cli.SigningError):
                cli._reserve_signing_code(
                    lane="public", release_ledger=ReleaseLedger(ledger_path),
                    registry=IdentityRegistry(registry_path), witness=SigningWitness(witness_path),
                    vault_ledger=vault_ledger, manifest=manifest, mode="disposable")


    def test_sign_apk_command_requires_external_approval_and_separate_state_paths(self):
        args = cli.build_parser().parse_args([
            "sign-apk", "--disposable", "--root", "/tmp/vault",
            "--release-ledger", "/tmp/ledger/release.json", "--identity-registry", "/tmp/registry/identity.json",
            "--witness", "/tmp/witness/witness.json", "--lane", "public", "--apk", "/tmp/unsigned.apk",
            "--provenance", "/tmp/provenance.json", "--approved-apk-sha256", "a" * 64,
            "--approved-provenance-sha256", "b" * 64, "--approved-source-commit", "c" * 40,
            "--approved-source-tree", "d" * 40, "--output", "/tmp/output/signed.apk",
        ])
        self.assertEqual(args.command, "sign-apk")
        self.assertEqual(args.lane, "public")
        self.assertEqual(args.approved_apk_sha256, "a" * 64)

    def test_legacy_gradle_signing_commands_are_fail_closed(self):
        for command in ("build-public", "build-private"):
            with self.subTest(command=command):
                args = cli.build_parser().parse_args([
                    command, "--disposable", "--root", "/tmp/vault",
                    "--release-ledger", "/tmp/ledger/release.json",
                    "--identity-registry", "/tmp/registry/identity.json",
                    "--witness", "/tmp/witness/witness.json",
                    "--authorization-record", "/tmp/authorization.json",
                ])
                with mock.patch.object(cli, "_root_for") as root_for, \
                        mock.patch.object(cli, "_prompts_for_lane") as prompt, \
                        mock.patch.object(cli, "_gradle") as gradle:
                    with self.assertRaisesRegex(cli.SigningError, "Gradle-based signing is retired"):
                        args.handler(args)
                    root_for.assert_not_called()
                    prompt.assert_not_called()
                    gradle.assert_not_called()


    def test_recovery_plan_is_read_only_and_reports_pending_attempts(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            root = base / "vault"
            root.mkdir(mode=0o700)
            os.chmod(root, 0o700)
            dirs = [base / name for name in ("ledger", "registry", "witness")]
            for directory in dirs:
                directory.mkdir(mode=0o700)
                os.chmod(directory, 0o700)
            identities = {
                "public": {"application_id": "app.serea.android.beta", "certificate_sha256": "1" * 64,
                            "initial_version_code": 2},
                "private": {"application_id": "app.serea.android.privatebeta", "certificate_sha256": "2" * 64,
                             "initial_version_code": 2},
            }
            ledger_id = str(uuid.uuid4())
            ledger_path = dirs[0] / "release.json"
            registry_path = dirs[1] / "identity.json"
            witness_path = dirs[2] / "witness.json"
            witness = SigningWitness(witness_path).initialize(mode="disposable", ledger_id=ledger_id,
                                                              identities=identities, confirm_new=True)
            ledger = ReleaseLedger.initialize(ledger_path, mode="disposable", identities=identities,
                                              ledger_id=ledger_id)
            IdentityRegistry(registry_path).initialize(mode="disposable", ledger_id=ledger_id,
                                                       identities=identities, witness=witness)
            reservation = ReleaseLedger(ledger_path).reserve("public")
            artifact_store = ledger_path.parent / f".serea-artifacts-{ledger_id}"
            artifact_store.mkdir(mode=0o700)
            attempt_store = artifact_store / reservation["attempt_id"]
            attempt_store.mkdir(mode=0o700)
            receipt_path = attempt_store / "receipt.json"
            receipt_path.write_text(json.dumps({
                "schema": 1, "ledger_id": ledger_id, "attempt_id": reservation["attempt_id"],
                "lane": "public", "version_code": reservation["version_code"], "state": "RESERVED",
                "input_apk_sha256": "a" * 64, "input_provenance_sha256": "b" * 64,
                "approved_source_commit": "c" * 40, "approved_source_tree": "d" * 40,
                "artifact_path": str(attempt_store / "signed.apk"),
                "sidecar_path": str(attempt_store / "result.json"),
                "signed_apk_sha256": None, "sidecar_sha256": None,
                "output_path": str(base / "signed" / "public.apk")}), encoding="utf-8")
            os.chmod(receipt_path, 0o600)
            before = [path.read_bytes() for path in (ledger_path, registry_path, witness_path)]
            args = argparse.Namespace(disposable=True, root=str(root), release_ledger=str(ledger_path),
                                      identity_registry=str(registry_path), witness=str(witness_path))
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                self.assertEqual(cli.cmd_ledger_recovery_plan(args), 0)
            result = json.loads(output.getvalue())
            self.assertEqual(result["pending"][0]["attempt_id"], reservation["attempt_id"])
            self.assertTrue(result["read_only"])
            self.assertEqual(result["publication_receipts"][0]["attempt_id"], reservation["attempt_id"])
            self.assertEqual(result["publication_receipts"][0]["state"], "RESERVED")
            self.assertEqual(result["publication_receipts"][0]["diagnostics"]["integrity"], "INCOMPLETE")
            self.assertEqual(before, [path.read_bytes() for path in (ledger_path, registry_path, witness_path)])
            receipt_data = json.loads(receipt_path.read_text(encoding="utf-8"))
            receipt_data["state"] = "VERIFIED"
            receipt_data["signed_apk_sha256"] = "e" * 64
            receipt_path.write_text(json.dumps(receipt_data), encoding="utf-8")
            os.chmod(receipt_path, 0o600)
            mismatch_output = io.StringIO()
            with contextlib.redirect_stdout(mismatch_output):
                self.assertEqual(cli.cmd_ledger_recovery_plan(args), 0)
            mismatch_plan = json.loads(mismatch_output.getvalue())
            mismatch_receipt = mismatch_plan["publication_receipts"][0]
            self.assertFalse(mismatch_receipt["diagnostics"]["ledger_state_matches_receipt"])
            self.assertEqual(mismatch_receipt["diagnostics"]["integrity"], "UNKNOWN")
            abandon_args = argparse.Namespace(disposable=True, root=str(root), release_ledger=str(ledger_path),
                                              identity_registry=str(registry_path), witness=str(witness_path),
                                              confirm=f"ABANDON {reservation['attempt_id']} {reservation['version_code']}",
                                              attempt_id=reservation["attempt_id"],
                                              version_code=reservation["version_code"], lane="public",
                                              reason="process-crash-reviewed")
            actual_receipts = cli._read_publication_receipts(ledger_path, ledger_id)
            with mock.patch.object(cli, "ANDROID_ROOT", base / "android"), \
                    mock.patch.object(cli, "_read_publication_receipts", side_effect=[[], actual_receipts]), \
                    mock.patch.object(cli, "_lock", return_value=contextlib.nullcontext()), \
                    mock.patch.object(cli, "_mounted_vault", return_value=contextlib.nullcontext(None)), \
                    mock.patch.object(cli, "_load_manifest_from_mount") as load_manifest:
                with self.assertRaisesRegex(cli.SigningError, "receipt appeared during recovery"):
                    cli.cmd_ledger_abandon(abandon_args)
                load_manifest.assert_not_called()
            receipt_path.unlink()
            incomplete_output = io.StringIO()
            with contextlib.redirect_stdout(incomplete_output):
                self.assertEqual(cli.cmd_ledger_recovery_plan(args), 0)
            incomplete_plan = json.loads(incomplete_output.getvalue())
            incomplete_attempt = next(item for item in incomplete_plan["pending"]
                                      if item["attempt_id"] == reservation["attempt_id"])
            self.assertEqual(incomplete_attempt["integrity"], "UNKNOWN")
            witness_now = SigningWitness(witness_path).read()
            orphan_id = str(uuid.uuid4())
            SigningWitness(witness_path).consume(lane="private", version_code=2, attempt_id=orphan_id,
                                                 expected_generation=witness_now["generation"])
            orphan_output = io.StringIO()
            with contextlib.redirect_stdout(orphan_output):
                self.assertEqual(cli.cmd_ledger_recovery_plan(args), 0)
            orphan_plan = json.loads(orphan_output.getvalue())
            self.assertTrue(any(item["attempt_id"] == orphan_id and
                                item["state"] == "WITNESS_ONLY_OR_STORE_MISMATCH"
                                for item in orphan_plan["pending"]))

    def test_provision_requires_authority_reserved_enrollment_slot(self):
        with self.assertRaises(SystemExit):
            cli.build_parser().parse_args(["provision", "--disposable", "--root", "/tmp/vault"])

    def test_witness_bootstrap_is_an_explicit_cli_operation(self):
        with self.assertRaises(SystemExit):
            cli.build_parser().parse_args(["witness-init", "--disposable", "--root", "/tmp/vault",
                                           "--release-ledger", "/tmp/ledger/release.json",
                                           "--identity-registry", "/tmp/registry/identity.json",
                                           "--witness", "/tmp/witness/witness.json",
                                           "--public-version-code", "2", "--private-version-code", "2"])
        args = cli.build_parser().parse_args(["witness-init", "--disposable", "--root", "/tmp/vault",
                                                  "--release-ledger", "/tmp/ledger/release.json",
                                                  "--identity-registry", "/tmp/registry/identity.json",
                                                  "--witness", "/tmp/witness/witness.json",
                                                  "--enrollment-record", "/tmp/enrollment.json",
                                                  "--authority-public-key", "/tmp/authority-public.pem",
                                                  "--public-version-code", "2", "--private-version-code", "2"])
        self.assertEqual(args.command, "witness-init")

    def test_disposable_root_cannot_overlap_formal_root(self):
        formal = cli.FORMAL_ROOT
        with self.assertRaises(cli.SigningError):
            cli._assert_disposable_root(formal, True)
        with self.assertRaises(cli.SigningError):
            cli._assert_disposable_root(formal / "nested", True)

    def test_symlink_components_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            actual = base / "actual"
            actual.mkdir()
            link = base / "link"
            link.symlink_to(actual, target_is_directory=True)
            with self.assertRaises(cli.SigningError):
                cli._canonical_no_symlinks(link / "vault")

    def test_owner_directory_creation_makes_private_components(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            os.chmod(base, 0o700)
            root = base / "Serea" / "Vault"
            cli._ensure_owner_directory(root, create=True)
            for path in (base / "Serea", root):
                self.assertEqual(path.stat().st_uid, os.getuid())
                self.assertEqual(path.stat().st_mode & 0o777, 0o700)

    def test_status_does_not_create_missing_formal_vault(self):
        with tempfile.TemporaryDirectory() as temporary:
            missing = Path(temporary) / "not-created" / "Vault"
            with mock.patch.object(cli, "FORMAL_ROOT", missing), mock.patch("builtins.print") as output:
                args = argparse.Namespace(disposable=False, root=None)
                self.assertEqual(cli.cmd_status(args), 0)
                self.assertFalse(missing.exists())
                self.assertTrue(any("image_exists=False" in str(call) for call in output.call_args_list))

    def test_copy_refuses_existing_destination_without_modifying_it(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            source = base / "source"
            destination = base / "destination"
            source.write_bytes(b"test image")
            destination.write_bytes(b"keep existing")
            with self.assertRaises(cli.SigningError):
                cli._copy_no_replace(source, destination)
            self.assertEqual(destination.read_bytes(), b"keep existing")

    def test_incomplete_exfat_backup_marker_blocks_restore_validation(self):
        with tempfile.TemporaryDirectory() as temporary:
            image = Path(temporary) / "backup.dmg"
            image.write_bytes(b"partial")
            image.with_name(image.name + ".serea-pending").write_text("incomplete")
            with mock.patch.object(cli, "_mount_context") as mount:
                with self.assertRaises(cli.SigningError):
                    cli._validate_vault(Path(temporary), True, image, {}, readonly=True)
                mount.assert_not_called()

    def test_copy_refuses_marked_incomplete_restore_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            source = base / "backup.dmg"
            source.write_bytes(b"partial")
            source.with_name(source.name + ".serea-pending").write_text("incomplete")
            with self.assertRaises(cli.SigningError):
                cli._copy_no_replace(source, base / "restore.dmg")
            self.assertFalse((base / "restore.dmg").exists())

    def test_attached_image_inventory_fails_closed_on_missing_or_invalid_images(self):
        for inventory in ({}, {"images": "not-a-list"}, {"images": ["not-an-image-entry"]},
                          {"images": [{}]}, {"images": [{"image-path": ""}]},
                          {"images": [{"image-path": "relative.dmg"}]},
                          {"images": [{"image-path": 42}]}):
            with self.subTest(inventory=inventory), \
                 mock.patch.object(cli, "_run", return_value=mock.Mock(
                     stdout=cli.plistlib.dumps(inventory))):
                with self.assertRaises(cli.SigningError):
                    cli._attached_images()

    def test_cleanup_retains_image_if_it_is_still_attached(self):
        with tempfile.TemporaryDirectory() as temporary:
            image = Path(temporary) / "image.dmg"
            image.write_bytes(b"test")
            with mock.patch.object(cli, "_image_is_attached", return_value=True), \
                 mock.patch("builtins.print"):
                cli._remove_created_image_if_detached(image, cli._sha256(image))
            self.assertTrue(image.exists())

    def test_manifest_mode_must_match_requested_lane(self):
        with tempfile.TemporaryDirectory() as temporary:
            mount = Path(temporary)
            (mount / "identity-manifest.json").write_text('{"schema": 1, "mode": "production"}')
            with self.assertRaises(cli.SigningError):
                cli._load_manifest_from_mount(mount, disposable=True)

    def test_lock_rejects_concurrent_operation(self):
        with tempfile.TemporaryDirectory() as temporary:
            private = Path(temporary) / "private"
            private.mkdir(mode=0o700)
            root = private / "Vault"
            root.mkdir(mode=0o700)
            with cli._lock(root):
                with self.assertRaises(cli.SigningError):
                    with cli._lock(root):
                        pass

    def test_signing_lock_rejects_a_second_process(self):
        with tempfile.TemporaryDirectory() as temporary:
            private = Path(temporary)
            os.chmod(private, 0o700)
            root = private / "Vault"
            root.mkdir(mode=0o700)
            script = (
                "import sys\nfrom pathlib import Path\nfrom tools import serea_signing as c\n"
                "try:\n    with c._lock(Path(sys.argv[1])): print('ACQUIRED')\n"
                "except c.SigningError:\n    print('BLOCKED')\n    raise SystemExit(23)\n")
            with cli._lock(root):
                child = subprocess.run([os.sys.executable, "-c", script, str(root)],
                                       cwd=Path(__file__).parents[1], capture_output=True, text=True, timeout=20)
            self.assertEqual(child.returncode, 23, child.stdout + child.stderr)
            self.assertIn("BLOCKED", child.stdout)

    def test_shared_build_lock_rejects_a_concurrent_signing_build(self):
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            (home / "Library" / "Caches").mkdir(parents=True)
            with mock.patch.object(cli.Path, "home", return_value=home):
                with cli._build_lock():
                    with self.assertRaises(cli.SigningError):
                        with cli._build_lock():
                            pass

    def test_secret_input_fails_closed_without_tty(self):
        fake_stdin = mock.Mock()
        fake_stdin.isatty.return_value = False
        fake_stderr = mock.Mock()
        fake_stderr.isatty.return_value = False
        with mock.patch.object(cli.sys, "stdin", fake_stdin), mock.patch.object(cli.sys, "stderr", fake_stderr):
            with self.assertRaises(cli.SigningError):
                cli._secret("test: ")

    @unittest.skipUnless(sys.platform == "darwin", "macOS /tmp aliases /private/tmp")
    def test_mount_context_resolves_macos_tmp_alias_before_strict_mount_checks(self):
        with tempfile.TemporaryDirectory(dir="/private/tmp") as temporary:
            canonical_parent = Path(temporary).resolve(strict=True)
            alias_parent = Path("/tmp") / canonical_parent.name
            self.assertNotEqual(str(alias_parent), str(canonical_parent))
            self.assertEqual(cli._canonical_mountpoint(alias_parent), str(canonical_parent))
            with mock.patch.object(cli.tempfile, "mkdtemp", return_value=str(alias_parent)):
                vault = cli._mount_context(Path(temporary) / "image.dmg", "SereaDisposable")
            self.assertEqual(vault.temp_parent, canonical_parent)
            self.assertEqual(vault.mountpoint, canonical_parent / "volume")
            temp_parent = vault.temp_parent
            assert temp_parent is not None
            temp_parent.rmdir()

    @unittest.skipUnless(sys.platform == "darwin", "macOS /tmp aliases /private/tmp")
    def test_mountpoint_comparison_remains_strict_after_path_normalization(self):
        with tempfile.TemporaryDirectory(dir="/private/tmp") as temporary:
            canonical = Path(temporary).resolve(strict=True)
            self.assertEqual(cli._canonical_mountpoint(Path("/tmp") / canonical.name),
                             cli._canonical_mountpoint(canonical))
            self.assertNotEqual(cli._canonical_mountpoint(canonical),
                                cli._canonical_mountpoint(canonical / "different"))

    def test_attach_failure_does_not_detach_same_image_at_another_mountpoint(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            image = base / "image.dmg"
            image.write_bytes(b"test")
            invocation_root = base / "invocation"
            invocation_root.mkdir()
            mountpoint = invocation_root / "volume"
            vault = cli.MountedVault(image, mountpoint, "SereaSigning")
            attached = [{
                "image-path": str(image.resolve()),
                "system-entities": [{"dev-entry": "/dev/disk42", "mount-point": "/Volumes/OtherMount"}],
            }]
            with mock.patch.object(cli, "_image_is_attached", return_value=False), \
                 mock.patch.object(cli, "_secret", return_value="disposable-only"), \
                 mock.patch.object(cli, "_run", side_effect=cli.SigningError("attach failed")), \
                 mock.patch.object(cli, "_attached_images", return_value=attached), \
                 mock.patch.object(vault, "detach") as detach:
                with self.assertRaises(cli.SigningError):
                    vault.__enter__()
            detach.assert_not_called()
            self.assertFalse(vault.attached)

    def test_gradle_user_home_excludes_user_init_scripts_and_reuses_caches(self):
        with tempfile.TemporaryDirectory() as temporary:
            shared = Path(temporary) / ".gradle"
            (shared / "caches").mkdir(parents=True)
            (shared / "wrapper" / "dists").mkdir(parents=True)
            init_dir = shared / "init.d"
            init_dir.mkdir()
            (init_dir / "owner-init.gradle").write_text("throw new RuntimeException()")
            with mock.patch.object(cli.Path, "home", return_value=Path(temporary)):
                with cli._isolated_gradle_user_home() as isolated:
                    self.assertFalse((isolated / "init.d").exists())
                    self.assertTrue((isolated / "caches").is_symlink())
                    self.assertTrue((isolated / "wrapper" / "dists").is_symlink())
                    self.assertFalse((isolated / "gradle.properties").exists())
                self.assertFalse(isolated.exists())

    def test_private_gradle_environment_omits_public_private_key_inputs(self):
        manifest = {"identities": {
            "public": {"store": "public-beta.p12", "alias": "public", "sha256": "a" * 64},
            "private": {"store": "private-beta.p12", "alias": "private", "sha256": "b" * 64},
        }}
        captured = {}

        def fake_run(args, *, env=None, **kwargs):
            captured.update(env or {})
            return mock.Mock(returncode=0)

        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            output = cli.ANDROID_ROOT / "app/build/outputs/apk/privateBeta"
            with mock.patch.object(cli, "ANDROID_ROOT", base), \
                 mock.patch.object(cli, "_build_lock", return_value=contextlib.nullcontext()), \
                 mock.patch.object(cli, "_run", side_effect=fake_run), \
                 mock.patch.object(cli.Path, "exists", return_value=False), \
                 mock.patch.object(cli.Path, "is_dir", return_value=True), \
                 mock.patch.object(cli.Path, "glob", return_value=[base / "signed.apk"]):
                cli._gradle("private", base, manifest, "disposable-only", allow_downloads=False, version_code=2)
        public_vars = cli.GRADLE_VARIABLES["public"]
        self.assertNotIn(public_vars["path"], captured)
        self.assertNotIn(public_vars["store_password"], captured)
        self.assertNotIn(public_vars["alias"], captured)
        self.assertNotIn(public_vars["key_password"], captured)
        self.assertEqual(captured[public_vars["fingerprint"]], "a" * 64)
        self.assertEqual(captured[cli.GRADLE_VARIABLES["private"]["store_password"]], "disposable-only")
        self.assertEqual(captured["SEREA_OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED"], "false")
        self.assertNotEqual(captured["GRADLE_USER_HOME"], str(Path.home() / ".gradle"))
        self.assertFalse(Path(captured["GRADLE_USER_HOME"]).exists())

    def test_gradle_timeout_removes_only_new_variant_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            manifest = {"identities": {
                "public": {"store": "public-beta.p12", "alias": "public", "sha256": "a" * 64},
                "private": {"store": "private-beta.p12", "alias": "private", "sha256": "b" * 64},
            }}
            output = base / "app/build/outputs/apk/beta"

            def timed_out(*args, **kwargs):
                output.mkdir(parents=True)
                (output / "partial.apk").write_bytes(b"partial")
                raise cli.SigningError("Gradle timed out")

            with mock.patch.object(cli, "ANDROID_ROOT", base), \
                 mock.patch.object(cli, "_build_lock", return_value=contextlib.nullcontext()), \
                 mock.patch.object(cli, "_run", side_effect=timed_out):
                with self.assertRaises(cli.SigningError):
                    cli._gradle("public", base, manifest, "disposable-only", allow_downloads=False, version_code=2)
            self.assertFalse(output.exists())

    def test_apk_verification_requires_exactly_one_expected_signer(self):
        expected = {"version_code": 2, "version_name": "0.1.0-demo", "sha1": "1" * 40,
                    "sha256": "2" * 64}
        with tempfile.TemporaryDirectory() as temporary:
            apk = Path(temporary) / "test.apk"
            apk.write_bytes(b"fake apk")
            sdk = Path(temporary) / "Library/Android/sdk/build-tools/35.0.0"
            sdk.mkdir(parents=True)
            (sdk / "aapt").touch()
            (sdk / "apksigner").touch()
            badging = b"package: name='app.serea.android.beta' versionCode='2' versionName='0.1.0-demo'\n"
            good = ("Verifies\nSigner #1 certificate SHA-1 digest: " + ":".join(["11"] * 20) +
                    "\nSigner #1 certificate SHA-256 digest: " + ":".join(["22"] * 32) + "\n").encode()
            multiple = good + good.replace(b"Signer #1", b"Signer #2")
            with mock.patch.object(cli.Path, "home", return_value=Path(temporary)), \
                 mock.patch.object(cli, "_run", side_effect=[mock.Mock(stdout=badging), mock.Mock(stdout=good),
                                                              mock.Mock(stdout=multiple)]):
                self.assertEqual(cli._verify_apk(apk, "public", expected)["version_code"], "2")
                with self.assertRaises(cli.SigningError):
                    cli._verify_apk(apk, "public", expected)

    def test_apk_verification_rejects_wrong_version_and_version_name(self):
        expected = {"version_code": 2, "version_name": "0.1.0-demo", "sha1": "1" * 40,
                    "sha256": "2" * 64}
        with tempfile.TemporaryDirectory() as temporary:
            apk = Path(temporary) / "test.apk"
            apk.write_bytes(b"fake apk")
            sdk = Path(temporary)
            (sdk / "aapt").touch()
            (sdk / "apksigner").touch()
            badging = b"package: name='app.serea.android.beta' versionCode='2' versionName='0.1.1-demo'\n"
            with mock.patch.object(cli.Path, "home", return_value=Path(temporary)), \
                 mock.patch.object(cli, "_run", return_value=mock.Mock(stdout=badging)):
                with self.assertRaises(cli.SigningError):
                    cli._verify_apk(apk, "public", expected)


class DisposableE2EModeTests(unittest.TestCase):
    def test_e2e_defaults_to_local_recovery_and_external_mode_is_explicit(self):
        self.assertIsNone(e2e.build_parser().parse_args([]).external)
        external = e2e.build_parser().parse_args(["--external", "/Volumes/TEST"])
        self.assertEqual(external.external, Path("/Volumes/TEST"))

    def test_external_mode_requires_a_real_external_mountpoint(self):
        info = {
            "DeviceLocation": "External",
            "Internal": False,
            "MountPoint": "/Volumes/TEST",
            "VolumeName": "TEST",
            "DeviceIdentifier": "disk9s1",
        }
        with mock.patch.object(e2e.cli, "_canonical_no_symlinks", return_value=Path("/Volumes/TEST")), \
             mock.patch.object(Path, "is_dir", return_value=True), \
             mock.patch.object(e2e.os.path, "ismount", return_value=True), \
             mock.patch.object(e2e.subprocess, "run", return_value=mock.Mock(
                 stdout=__import__("plistlib").dumps(info), returncode=0)) as run:
            e2e.validate_external_mount(Path("/Volumes/TEST"))
        self.assertEqual(run.call_args.args[0][:4], ["/usr/sbin/diskutil", "info", "-plist", "/Volumes/TEST"])

    def test_external_mode_rejects_internal_or_unmounted_paths(self):
        internal = {
            "DeviceLocation": "Internal",
            "Internal": True,
            "MountPoint": "/Volumes/TEST",
            "VolumeName": "TEST",
            "DeviceIdentifier": "disk9s1",
        }
        with mock.patch.object(Path, "is_dir", return_value=True), \
             mock.patch.object(e2e.os.path, "ismount", return_value=True), \
             mock.patch.object(e2e.subprocess, "run", return_value=mock.Mock(
                 stdout=__import__("plistlib").dumps(internal), returncode=0)):
            with self.assertRaises(RuntimeError):
                e2e.validate_external_mount(Path("/Volumes/TEST"))
        with mock.patch.object(e2e.cli, "_canonical_no_symlinks", return_value=Path("/Volumes/TEST")), \
             mock.patch.object(e2e.os.path, "ismount", return_value=False), \
             mock.patch.object(e2e.subprocess, "run") as run:
            with self.assertRaises(RuntimeError):
                e2e.validate_external_mount(Path("/Volumes/TEST"))
            run.assert_not_called()

    def test_external_mode_rejects_device_replacement_before_backup(self):
        original = {"mountpoint": "/Volumes/TEST", "volume_name": "TEST", "device_identifier": "disk9s1"}
        replacement = {"mountpoint": "/Volumes/TEST", "volume_name": "TEST", "device_identifier": "disk10s1"}
        with mock.patch.object(e2e, "validate_external_mount", return_value=replacement):
            with self.assertRaises(RuntimeError):
                e2e.validate_external_unchanged(Path("/Volumes/TEST"), original)

    def test_external_mode_rejects_relative_argument_even_if_it_resolves_to_mountpoint(self):
        with mock.patch.object(e2e.cli, "_canonical_no_symlinks", return_value=Path("/Volumes/TEST")), \
             mock.patch.object(Path, "is_dir", return_value=True), \
             mock.patch.object(e2e.os.path, "ismount", return_value=True), \
             mock.patch.object(e2e.subprocess, "run") as run:
            with self.assertRaises(RuntimeError):
                e2e.validate_external_mount(Path("TEST"))
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
