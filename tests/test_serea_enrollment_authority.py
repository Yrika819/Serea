import json
import os
import tempfile
import unittest
import uuid
from pathlib import Path

from tools.serea_enrollment_authority import (
    AuthorityError,
    EnrollmentAuthority,
    canonical_json,
    generate_keypair,
    verify_record,
)


IDENTITIES = {
    "public": {"application_id": "app.serea.android.beta", "certificate_sha256": "1" * 64,
                "initial_version_code": 2},
    "private": {"application_id": "app.serea.android.privatebeta", "certificate_sha256": "2" * 64,
                 "initial_version_code": 2},
}


class EnrollmentAuthorityTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / "authority"
        self.root.mkdir(mode=0o700)
        os.chmod(self.root, 0o700)
        self.state = self.root / "authority-state.json"
        self.private = self.root / "authority-ed25519-private.pem"
        self.public = self.root / "authority-ed25519-public.pem"
        generate_keypair(self.private, self.public)
        self.authority = EnrollmentAuthority(self.state, self.private, self.public, mode="disposable")
        self.authority.create(mode="disposable")

    def tearDown(self):
        self.temp.cleanup()

    def test_canonical_json_is_order_independent_and_stable(self):
        self.assertEqual(canonical_json({"b": [2, 3], "a": 1}), canonical_json({"a": 1, "b": [2, 3]}))

    def test_enrollment_slot_is_single_use_and_signed(self):
        slot = self.authority.reserve_enrollment(str(uuid.uuid4()))
        signed = self.authority.enroll(slot["payload"]["enrollment_id"], str(uuid.uuid4()), IDENTITIES,
                                       ledger_id=str(uuid.uuid4()))
        self.assertTrue(verify_record(signed, self.public, expected_purpose="enrollment"))
        with self.assertRaises(AuthorityError):
            self.authority.enroll(slot["payload"]["enrollment_id"], str(uuid.uuid4()), IDENTITIES,
                                  ledger_id=str(uuid.uuid4()))

    def test_authorization_consumes_high_water_and_cannot_replay_new_request(self):
        enrollment_id = str(uuid.uuid4())
        self.authority.reserve_enrollment(enrollment_id)
        ledger_id = str(uuid.uuid4())
        self.authority.enroll(enrollment_id, str(uuid.uuid4()), IDENTITIES, ledger_id=ledger_id)
        request = {"request_id": str(uuid.uuid4()), "nonce": "a" * 64, "mode": "disposable",
                   "lane": "public", "application_id": IDENTITIES["public"]["application_id"],
                   "certificate_sha256": IDENTITIES["public"]["certificate_sha256"],
                   "ledger_id": ledger_id, "version_code": 2, "attempt_id": str(uuid.uuid4()),
                   "operation": "sign-apk", "source_commit": "b" * 40, "source_tree": "c" * 40}
        signed = self.authority.authorize(request)
        self.assertTrue(verify_record(signed, self.public, expected_purpose="sign-authorization"))
        with self.assertRaises(AuthorityError):
            self.authority.authorize({**request, "request_id": str(uuid.uuid4()), "attempt_id": str(uuid.uuid4())})
        self.assertEqual(self.authority.read_state()["lanes"]["public"]["high_watermark"], 2)

    def test_authority_refuses_second_identity_enrollment_for_same_lanes(self):
        first_id = str(uuid.uuid4())
        self.authority.reserve_enrollment(first_id)
        self.authority.enroll(first_id, str(uuid.uuid4()), IDENTITIES, ledger_id=str(uuid.uuid4()))
        second_id = str(uuid.uuid4())
        self.authority.reserve_enrollment(second_id)
        changed = {"public": {**IDENTITIES["public"], "certificate_sha256": "3" * 64},
                   "private": {**IDENTITIES["private"], "certificate_sha256": "4" * 64}}
        with self.assertRaises(AuthorityError):
            self.authority.enroll(second_id, str(uuid.uuid4()), changed, ledger_id=str(uuid.uuid4()))

    def test_tampering_wrong_purpose_and_wrong_key_are_rejected(self):
        enrollment_id = str(uuid.uuid4())
        self.authority.reserve_enrollment(enrollment_id)
        signed = self.authority.enroll(enrollment_id, str(uuid.uuid4()), IDENTITIES, ledger_id=str(uuid.uuid4()))
        tampered = json.loads(json.dumps(signed))
        tampered["payload"]["ledger_id"] = str(uuid.uuid4())
        self.assertFalse(verify_record(tampered, self.public, expected_purpose="enrollment"))
        self.assertFalse(verify_record(signed, self.public, expected_purpose="sign-authorization"))
        other_private, other_public = self.root / "other-private.pem", self.root / "other-public.pem"
        generate_keypair(other_private, other_public)
        self.assertFalse(verify_record(signed, other_public, expected_purpose="enrollment"))

    def test_missing_or_existing_authority_state_never_bootstraps_implicitly(self):
        self.state.unlink()
        with self.assertRaises(AuthorityError):
            self.authority.read_state()
        self.authority.create(mode="disposable")
        with self.assertRaises(AuthorityError):
            self.authority.create(mode="disposable")


if __name__ == "__main__":
    unittest.main()
