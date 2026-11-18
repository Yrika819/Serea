import os
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest import mock

from tools.serea_identity_registry import IdentityRegistry, RegistryError
from tools.serea_signing_witness import SigningWitness
from tools import serea_signing_state

IDENTITIES = {
    "public": {"application_id": "app.serea.android.beta", "certificate_sha256": "1" * 64,
                "initial_version_code": 4},
    "private": {"application_id": "app.serea.android.privatebeta", "certificate_sha256": "2" * 64,
                 "initial_version_code": 9},
}


class IdentityRegistryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / "registry"
        self.root.mkdir(mode=0o700)
        os.chmod(self.root, 0o700)
        self.path = self.root / "identity.json"
        self.ledger_id = str(uuid.uuid4())

    def tearDown(self):
        self.temp.cleanup()

    def test_missing_registry_is_not_implicitly_initialized(self):
        with self.assertRaises(RegistryError):
            IdentityRegistry(self.path).read()
        self.assertFalse(self.path.exists())

    def test_create_requires_explicit_witness_binding_and_refuses_replacement(self):
        registry = IdentityRegistry(self.path)
        witness = {"witness_id": str(uuid.uuid4()), "mode": "disposable", "ledger_id": self.ledger_id,
                   "generation": 0, "events": [],
                   "lanes": {lane: {**IDENTITIES[lane], "high_watermark": IDENTITIES[lane]["initial_version_code"] - 1,
                                    "last_attempt_id": None} for lane in IDENTITIES}}
        value = registry.initialize(mode="disposable", ledger_id=self.ledger_id,
                                    identities=IDENTITIES, witness=witness)
        self.assertEqual(value["ledger_id"], self.ledger_id)
        self.assertEqual(value["witness_id"], witness["witness_id"])
        self.assertEqual(registry.read()["lanes"]["public"]["high_watermark"], 3)
        original = self.path.read_bytes()
        with self.assertRaises(RegistryError):
            registry.initialize(mode="disposable", ledger_id=self.ledger_id,
                                identities=IDENTITIES, witness=witness)
        self.assertEqual(self.path.read_bytes(), original)

    def test_registry_directory_fsync_crash_cannot_be_retried_as_fresh_registration(self):
        witness_dir = self.root / "witness"
        witness_dir.mkdir(mode=0o700)
        os.chmod(witness_dir, 0o700)
        witness_store = SigningWitness(witness_dir / "witness.json")
        witness = witness_store.initialize(mode="disposable", ledger_id=self.ledger_id,
                                           identities=IDENTITIES, confirm_new=True)
        real_fsync = serea_signing_state.os.fsync
        calls = []
        def fail_directory_fsync(fd):
            calls.append(fd)
            if len(calls) == 2:
                raise OSError("injected directory fsync failure")
            return real_fsync(fd)
        registry = IdentityRegistry(self.path)
        with mock.patch.object(serea_signing_state.os, "fsync", side_effect=fail_directory_fsync):
            with self.assertRaises(RegistryError):
                registry.initialize(mode="disposable", ledger_id=self.ledger_id,
                                    identities=IDENTITIES, witness=witness)
        before = self.path.read_bytes()
        with self.assertRaises(RegistryError):
            registry.initialize(mode="disposable", ledger_id=self.ledger_id,
                                identities=IDENTITIES, witness=witness)
        self.assertEqual(before, self.path.read_bytes())

    def test_registry_and_witness_must_match_attempt_identity_not_only_counter(self):
        witness_dir = self.root / "witness"
        witness_dir.mkdir(mode=0o700)
        os.chmod(witness_dir, 0o700)
        witness = SigningWitness(witness_dir / "witness.json")
        initial = witness.initialize(mode="disposable", ledger_id=self.ledger_id,
                                     identities=IDENTITIES, confirm_new=True)
        registry = IdentityRegistry(self.path)
        registry.initialize(mode="disposable", ledger_id=self.ledger_id,
                            identities=IDENTITIES, witness=initial)
        registry_attempt = str(uuid.uuid4())
        witness_attempt = str(uuid.uuid4())
        registry.consume(lane="public", version_code=4, attempt_id=registry_attempt, expected_generation=0)
        witness.consume(lane="public", version_code=4, attempt_id=witness_attempt, expected_generation=0)
        with self.assertRaises(RegistryError):
            registry.assert_matches_witness(witness.read())

    def test_registry_cannot_be_initialized_from_already_consumed_witness(self):
        witness_dir = self.root / "witness"
        witness_dir.mkdir(mode=0o700)
        os.chmod(witness_dir, 0o700)
        witness = SigningWitness(witness_dir / "witness.json")
        record = witness.initialize(mode="disposable", ledger_id=self.ledger_id,
                                    identities=IDENTITIES, confirm_new=True)
        witness.consume(lane="public", version_code=4, attempt_id=str(uuid.uuid4()), expected_generation=0)
        consumed = witness.read()
        with self.assertRaises(RegistryError):
            IdentityRegistry(self.path).initialize(mode="disposable", ledger_id=self.ledger_id,
                                                   identities=IDENTITIES, witness=consumed)
        self.assertFalse(self.path.exists())

    def test_registry_rejects_wrong_mode_lane_certificate_or_ledger(self):
        registry = IdentityRegistry(self.path)
        witness = {"witness_id": str(uuid.uuid4()), "mode": "disposable", "ledger_id": self.ledger_id,
                   "generation": 0, "events": [],
                   "lanes": {lane: {**IDENTITIES[lane], "high_watermark": IDENTITIES[lane]["initial_version_code"] - 1,
                                    "last_attempt_id": None} for lane in IDENTITIES}}
        registry.initialize(mode="disposable", ledger_id=self.ledger_id,
                            identities=IDENTITIES, witness=witness)
        with self.assertRaises(RegistryError):
            registry.assert_binding(mode="production", ledger_id=self.ledger_id, identities=IDENTITIES)
        changed = {**IDENTITIES, "public": {**IDENTITIES["public"], "certificate_sha256": "3" * 64}}
        with self.assertRaises(RegistryError):
            registry.assert_binding(mode="disposable", ledger_id=self.ledger_id, identities=changed)
        with self.assertRaises(RegistryError):
            registry.assert_binding(mode="disposable", ledger_id=str(uuid.uuid4()), identities=IDENTITIES)

    def test_corrupt_and_symlink_registry_fail_closed(self):
        self.path.write_text("{}", encoding="utf-8")
        os.chmod(self.path, 0o600)
        with self.assertRaises(RegistryError):
            IdentityRegistry(self.path).read()
        self.path.unlink()
        target = self.root / "target"
        target.write_text("{}", encoding="utf-8")
        os.chmod(target, 0o600)
        self.path.symlink_to(target)
        with self.assertRaises(RegistryError):
            IdentityRegistry(self.path).read()


if __name__ == "__main__":
    unittest.main()
