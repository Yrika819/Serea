import hashlib
import json
import os
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest import mock

from tools.serea_signing_witness import SigningWitness, WitnessError
from tools import serea_signing_state

IDENTITIES = {
    "public": {"application_id": "app.serea.android.beta", "certificate_sha256": "1" * 64,
                "initial_version_code": 4},
    "private": {"application_id": "app.serea.android.privatebeta", "certificate_sha256": "2" * 64,
                 "initial_version_code": 9},
}


class SigningWitnessTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / "witness"
        self.root.mkdir(mode=0o700)
        os.chmod(self.root, 0o700)
        self.path = self.root / "witness.json"
        self.ledger_id = str(uuid.uuid4())
        self.witness = SigningWitness(self.path)

    def tearDown(self):
        self.temp.cleanup()

    def test_missing_witness_is_unknown_not_first_enrollment(self):
        with self.assertRaises(WitnessError):
            self.witness.read()
        self.assertFalse(self.path.exists())

    def test_explicit_enrollment_and_monotonic_compare_and_advance(self):
        initial = self.witness.initialize(mode="disposable", ledger_id=self.ledger_id,
                                         identities=IDENTITIES, confirm_new=True)
        self.assertEqual(initial["lanes"]["public"]["high_watermark"], 3)
        transaction = str(uuid.uuid4())
        advanced = self.witness.consume(lane="public", version_code=4, attempt_id=transaction,
                                        expected_generation=0)
        self.assertEqual(advanced["lanes"]["public"]["high_watermark"], 4)
        replay = self.witness.consume(lane="public", version_code=4, attempt_id=transaction,
                                      expected_generation=0)
        self.assertEqual(replay["generation"], advanced["generation"])
        with self.assertRaises(WitnessError):
            self.witness.consume(lane="public", version_code=4, attempt_id=str(uuid.uuid4()),
                                 expected_generation=advanced["generation"])
        with self.assertRaises(WitnessError):
            self.witness.consume(lane="public", version_code=3, attempt_id=str(uuid.uuid4()),
                                 expected_generation=advanced["generation"])

    def test_witness_binding_mismatch_and_existing_file_are_rejected(self):
        self.witness.initialize(mode="disposable", ledger_id=self.ledger_id,
                                identities=IDENTITIES, confirm_new=True)
        with self.assertRaises(WitnessError):
            self.witness.assert_binding(mode="production", ledger_id=self.ledger_id, identities=IDENTITIES)
        changed = {**IDENTITIES, "private": {**IDENTITIES["private"], "certificate_sha256": "4" * 64}}
        with self.assertRaises(WitnessError):
            self.witness.assert_binding(mode="disposable", ledger_id=self.ledger_id, identities=changed)
        before = self.path.read_bytes()
        with self.assertRaises(WitnessError):
            self.witness.initialize(mode="disposable", ledger_id=self.ledger_id,
                                    identities=IDENTITIES, confirm_new=True)
        self.assertEqual(self.path.read_bytes(), before)

    def test_witness_history_must_match_each_lane_high_water_mark(self):
        self.witness.initialize(mode="disposable", ledger_id=self.ledger_id,
                                identities=IDENTITIES, confirm_new=True)
        self.witness.consume(lane="public", version_code=4, attempt_id=str(uuid.uuid4()), expected_generation=0)
        value = json.loads(self.path.read_text(encoding="utf-8"))
        value["lanes"]["public"]["high_watermark"] = 5
        value["head_sha256"] = hashlib.sha256(json.dumps(
            {key: item for key, item in value.items() if key != "head_sha256"},
            sort_keys=True, separators=(",", ":")).encode("ascii")).hexdigest()
        self.path.write_text(json.dumps(value), encoding="utf-8")
        os.chmod(self.path, 0o600)
        with self.assertRaises(WitnessError):
            self.witness.read()

    def test_file_fsync_failure_does_not_publish_a_consumed_code(self):
        self.witness.initialize(mode="disposable", ledger_id=self.ledger_id,
                                identities=IDENTITIES, confirm_new=True)
        with mock.patch.object(serea_signing_state.os, "fsync", side_effect=OSError("injected file fsync failure")):
            with self.assertRaises(WitnessError):
                self.witness.consume(lane="public", version_code=4, attempt_id=str(uuid.uuid4()),
                                     expected_generation=0)
        self.assertEqual(self.witness.read()["lanes"]["public"]["high_watermark"], 3)

    def test_directory_fsync_failure_after_publish_is_uncertain_but_consumed(self):
        self.witness.initialize(mode="disposable", ledger_id=self.ledger_id,
                                identities=IDENTITIES, confirm_new=True)
        real_fsync = serea_signing_state.os.fsync
        calls = []
        def fail_directory_fsync(fd):
            calls.append(fd)
            if len(calls) == 2:
                raise OSError("injected directory fsync failure")
            return real_fsync(fd)
        with mock.patch.object(serea_signing_state.os, "fsync", side_effect=fail_directory_fsync):
            with self.assertRaises(WitnessError):
                self.witness.consume(lane="public", version_code=4, attempt_id=str(uuid.uuid4()),
                                     expected_generation=0)
        self.assertEqual(self.witness.read()["lanes"]["public"]["high_watermark"], 4)

    def test_corrupt_and_symlink_witness_fail_closed(self):
        self.path.write_text("{}", encoding="utf-8")
        os.chmod(self.path, 0o600)
        with self.assertRaises(WitnessError):
            self.witness.read()
        self.path.unlink()
        target = self.root / "target"
        target.write_text("{}", encoding="utf-8")
        os.chmod(target, 0o600)
        self.path.symlink_to(target)
        with self.assertRaises(WitnessError):
            self.witness.read()


if __name__ == "__main__":
    unittest.main()
