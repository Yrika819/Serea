import multiprocessing
import os
import random
import tempfile
import unittest
from pathlib import Path

from tools.serea_release_ledger import LedgerError, ReleaseLedger

PUBLIC_CERT = "1" * 64
PRIVATE_CERT = "2" * 64


def _reserve(path: str, queue) -> None:
    try:
        event = ReleaseLedger(path).reserve("public")
        queue.put(("ok", event["version_code"]))
    except LedgerError as exc:
        queue.put(("error", str(exc)))


def _reserve_lane(path: str, lane: str, queue) -> None:
    try:
        event = ReleaseLedger(path).reserve(lane)
        queue.put((lane, event["version_code"]))
    except LedgerError as exc:
        queue.put((lane, str(exc)))


class ReleaseLedgerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / "ledger"
        self.root.mkdir(mode=0o700)
        os.chmod(self.root, 0o700)
        self.path = self.root / "release.json"
        self.ledger = ReleaseLedger(self.path)
        ReleaseLedger.initialize(self.path, mode="disposable", identities={
            "public": {"application_id": "app.serea.android.beta", "certificate_sha256": PUBLIC_CERT,
                       "initial_version_code": 2},
            "private": {"application_id": "app.serea.android.privatebeta", "certificate_sha256": PRIVATE_CERT,
                        "initial_version_code": 7},
        })

    def tearDown(self):
        self.temp.cleanup()

    def test_vault_must_include_every_artifact_recorded_as_built_in_release_ledger(self):
        reserved = self.ledger.reserve("public")
        self.ledger.mark_attempted("public", reserved["version_code"], reserved["attempt_id"])
        built = self.ledger.mark_built("public", reserved["version_code"], reserved["attempt_id"],
                                       artifact_sha256="a" * 64, version_name="0.1.0-demo")
        state = self.ledger.read()
        vault_ledger = {
            "schema": 1,
            "release_ledger_id": state["ledger_id"],
            "release_ledger_path": str(self.path),
            "identities": {
                "public": {"application_id": "app.serea.android.beta", "next_version_code": 3,
                           "artifacts": []},
                "private": {"application_id": "app.serea.android.privatebeta", "next_version_code": 7,
                            "artifacts": []},
            },
        }
        manifest = {"identities": {
            "public": {"application_id": "app.serea.android.beta", "sha256": PUBLIC_CERT},
            "private": {"application_id": "app.serea.android.privatebeta", "sha256": PRIVATE_CERT},
        }}

        with self.assertRaisesRegex(LedgerError, "artifact history.*BUILT"):
            self.ledger.assert_matches_vault(vault_ledger, manifest, expected_mode="disposable")

        vault_ledger["identities"]["public"]["artifacts"] = [{
            "version_code": built["version_code"], "version_name": built["version_name"],
            "apk_sha256": built["artifact_sha256"], "built_utc": "2026-11-16T00:00:00Z",
        }]
        self.ledger.assert_matches_vault(vault_ledger, manifest, expected_mode="disposable")

    def test_reservation_consumes_code_and_requires_explicit_terminal_recovery(self):
        reserved = self.ledger.reserve("public")
        self.assertEqual((reserved["state"], reserved["version_code"]), ("RESERVED", 2))
        self.assertEqual(self.ledger.read()["lanes"]["public"]["next_version_code"], 3)
        with self.assertRaises(LedgerError):
            self.ledger.reserve("public")
        attempted = self.ledger.mark_attempted("public", 2, reserved["attempt_id"])
        self.assertEqual(attempted["state"], "ATTEMPTED")
        abandoned = self.ledger.abandon("public", 2, reserved["attempt_id"], reason="build-failed")
        self.assertEqual(abandoned["state"], "ABANDONED")
        next_event = self.ledger.reserve("public")
        self.assertEqual(next_event["version_code"], 3)

    def test_lanes_have_independent_sequences_and_build_distribution_are_distinct(self):
        event = self.ledger.reserve("private")
        self.ledger.mark_attempted("private", 7, event["attempt_id"])
        built = self.ledger.mark_built("private", 7, event["attempt_id"], artifact_sha256="a" * 64,
                                       version_name="0.1.0-demo")
        self.assertEqual(built["state"], "BUILT")
        distributed = self.ledger.mark_distributed("private", 7, event["attempt_id"],
                                                   distribution_ref="owner-receipt-1")
        self.assertEqual(distributed["state"], "DISTRIBUTED")
        self.assertEqual(self.ledger.read()["lanes"]["public"]["next_version_code"], 2)
        self.assertEqual(self.ledger.read()["lanes"]["private"]["next_version_code"], 8)

    def test_corruption_and_symlink_are_rejected(self):
        self.path.write_text("{}", encoding="utf-8")
        os.chmod(self.path, 0o600)
        with self.assertRaises(LedgerError):
            self.ledger.read()
        self.path.unlink()
        target = self.root / "target"
        target.write_text("{}", encoding="utf-8")
        os.chmod(target, 0o600)
        self.path.symlink_to(target)
        with self.assertRaises(LedgerError):
            self.ledger.read()

    def test_concurrent_same_lane_reservations_never_duplicate(self):
        queue = multiprocessing.Queue()
        processes = [multiprocessing.Process(target=_reserve, args=(str(self.path), queue)) for _ in range(2)]
        for process in processes:
            process.start()
        for process in processes:
            process.join(10)
            self.assertEqual(process.exitcode, 0)
        outcomes = [queue.get(timeout=2) for _ in processes]
        self.assertEqual(sum(result[0] == "ok" for result in outcomes), 1, outcomes)
        self.assertEqual(self.ledger.read()["lanes"]["public"]["next_version_code"], 3)

    def test_concurrent_public_and_private_reservations_keep_lane_sequences_separate(self):
        queue = multiprocessing.Queue()
        processes = [multiprocessing.Process(target=_reserve_lane, args=(str(self.path), lane, queue))
                     for lane in ("public", "private")]
        for process in processes:
            process.start()
        for process in processes:
            process.join(10)
            self.assertEqual(process.exitcode, 0)
        outcomes = [queue.get(timeout=2) for _ in processes]
        self.assertEqual(set(outcomes), {("public", 2), ("private", 7)})
        state = self.ledger.read()
        self.assertEqual(state["generation"], 2)
        self.assertEqual(state["lanes"]["public"]["next_version_code"], 3)
        self.assertEqual(state["lanes"]["private"]["next_version_code"], 8)

    def test_generation_compare_and_swap_rejects_stale_reservation(self):
        generation = self.ledger.read()["generation"]
        self.ledger.reserve("public", expected_generation=generation)
        with self.assertRaises(LedgerError):
            self.ledger.reserve("private", expected_generation=generation)

    def test_generated_lane_sequences_preserve_monotonic_consumed_codes(self):
        generator = random.Random(20261115)
        latest = {"public": 1, "private": 6}
        for _ in range(100):
            lane = generator.choice(("public", "private"))
            reserved = self.ledger.reserve(lane)
            self.assertGreater(reserved["version_code"], latest[lane])
            latest[lane] = reserved["version_code"]
            self.ledger.mark_attempted(lane, reserved["version_code"], reserved["attempt_id"])
            self.ledger.abandon(lane, reserved["version_code"], reserved["attempt_id"],
                                reason="process-crash-reviewed")
            state = self.ledger.read()
            self.assertEqual(state["lanes"][lane]["next_version_code"], latest[lane] + 1)
            for other_lane in ("public", "private"):
                self.assertGreaterEqual(state["lanes"][other_lane]["next_version_code"], latest[other_lane] + 1)

    def test_reservation_accepts_a_witness_transaction_id_without_reusing_codes(self):
        import uuid
        attempt_id = str(uuid.uuid4())
        generation = self.ledger.read()["generation"]
        event = self.ledger.reserve("public", expected_generation=generation, attempt_id=attempt_id)
        self.assertEqual(event["attempt_id"], attempt_id)
        self.assertEqual(event["version_code"], 2)
        with self.assertRaises(LedgerError):
            self.ledger.reserve("public", expected_generation=self.ledger.read()["generation"],
                                attempt_id=attempt_id)


if __name__ == "__main__":
    unittest.main()
