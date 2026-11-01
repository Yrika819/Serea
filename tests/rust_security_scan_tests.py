from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("rust_security_scan", ROOT / "tools" / "rust_security_scan.py")
scanner = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(scanner)

CHECKED_OUT_SHA = "1" * 40
TREE_SHA = "2" * 40


def completed(command, returncode=0, stdout="", stderr=""):
    return subprocess.CompletedProcess(command, returncode, stdout, stderr)


class RustSecurityScannerTests(unittest.TestCase):
    def test_version_is_checked_against_pinned_executable(self):
        with mock.patch.dict(os.environ, {"CARGO_AUDIT_VERSION": "0.22.2"}):
            with mock.patch.object(scanner, "run", return_value=completed([], stdout="cargo-audit-audit 0.22.2\n")):
                self.assertEqual(scanner.checked_scanner_version(), ("0.22.2", None))
            with mock.patch.object(scanner, "run", return_value=completed([], stdout="cargo-audit-audit 0.22.1\n")):
                version, error = scanner.checked_scanner_version()
                self.assertEqual(version, "cargo-audit-audit 0.22.1")
        self.assertIn("unexpected cargo-audit version", error)

    def test_failed_version_command_fails_closed(self):
        with mock.patch.dict(os.environ, {"CARGO_AUDIT_VERSION": "0.22.2"}):
            with mock.patch.object(scanner, "run", return_value=completed([], 127, stderr="not found")):
                _, error = scanner.checked_scanner_version()
        self.assertIn("version check failed", error)

    def test_checkout_sha_must_match_github_sha(self):
        def clean_run(command):
            if command[:3] == ["git", "rev-parse", "--verify"]:
                return completed(command, stdout=CHECKED_OUT_SHA + "\n")
            return completed(command)

        with mock.patch.dict(os.environ, {"GITHUB_SHA": CHECKED_OUT_SHA}):
            with mock.patch.object(scanner, "run", side_effect=clean_run):
                self.assertEqual(scanner.checked_checkout_sha(), (CHECKED_OUT_SHA, None))
            with mock.patch.object(scanner, "run", side_effect=lambda command: completed(command, stdout="3" * 40) if command[:3] == ["git", "rev-parse", "--verify"] else completed(command)):
                actual, error = scanner.checked_checkout_sha()
        self.assertEqual(actual, "3" * 40)
        self.assertIn("does not match GITHUB_SHA", error)

    def test_tree_sha_is_required_and_validated(self):
        with mock.patch.object(scanner, "run", return_value=completed([], stdout=TREE_SHA + "\n")):
            self.assertEqual(scanner.checked_tree_sha(), (TREE_SHA, None))
        with mock.patch.object(scanner, "run", return_value=completed([], 1)):
            self.assertIn("could not resolve checked-out tree", scanner.checked_tree_sha()[1])
        with mock.patch.object(scanner, "run", return_value=completed([], stdout="bad-tree")):
            self.assertIn("missing or malformed", scanner.checked_tree_sha()[1])

    def test_missing_or_malformed_github_sha_fails_closed(self):
        for value in (None, "not-a-sha"):
            env = {} if value is None else {"GITHUB_SHA": value}
            with mock.patch.dict(os.environ, env, clear=False):
                if value is None:
                    os.environ.pop("GITHUB_SHA", None)
                with mock.patch.object(scanner, "run", return_value=completed([], stdout=CHECKED_OUT_SHA)):
                    _, error = scanner.checked_checkout_sha()
            self.assertIn("GITHUB_SHA is missing or malformed", error)

    def test_empty_exception_list_is_allowed(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "exceptions.json"
            path.write_text("[]\n", encoding="utf-8")
            self.assertEqual(scanner.load_exceptions(path), ([], []))

    def test_nonempty_exception_list_is_rejected_even_with_approval_fields(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "exceptions.json"
            path.write_text(json.dumps([{
                "advisory_id": "RUSTSEC-2099-0001",
                "package": "example",
                "rationale": "test fixture",
                "expires": "2099-01-01",
                "owner_approval": {
                    "approved_by": "Owner",
                    "approved_at": "2098-01-01T00:00:00Z",
                    "approval_ref": "https://example.invalid/approval",
                },
            }]), encoding="utf-8")
            exceptions, errors = scanner.load_exceptions(path)
        self.assertEqual(exceptions, [])
        self.assertTrue(any("externally verifiable" in error for error in errors))

    def test_malformed_exception_json_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "exceptions.json"
            path.write_text("{bad json", encoding="utf-8")
            _, errors = scanner.load_exceptions(path)
        self.assertTrue(any("invalid" in error for error in errors))

    def test_provenance_json_parse_failure_is_reported(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for rel in scanner.VENDORS:
                vendor = root / rel
                (vendor / "src").mkdir(parents=True)
                (vendor / "Cargo.toml").write_text("[package]\n", encoding="utf-8")
                (vendor / ".cargo_vcs_info.json").write_text("{bad", encoding="utf-8")
            with mock.patch.object(scanner, "ROOT", root):
                inventory, errors = scanner.vendor_inventory()
        self.assertEqual(inventory, [])
        self.assertEqual(len(errors), 2)
        self.assertTrue(all("provenance is unreadable or invalid" in error for error in errors))

    def run_main(self, *, github_sha=CHECKED_OUT_SHA, version_output="cargo-audit-audit 0.22.2\n",
                 audit_output=None, exceptions="[]\n", audit_returncode=0, dirty=False,
                 tree_output=TREE_SHA, tree_returncode=0):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            for rel in scanner.LOCKFILES:
                lock = root / rel
                lock.parent.mkdir(parents=True, exist_ok=True)
                lock.write_text("# fixture\n", encoding="utf-8")
            (root / "security").mkdir()
            (root / "security" / "rust-audit-exceptions.json").write_text(exceptions, encoding="utf-8")
            output = {"vulnerabilities": {"list": []}, "database": {"last-updated": "2099-01-01T00:00:00Z"}}
            audit_output = json.dumps(output) if audit_output is None else audit_output

            def fake_run(command):
                if command == ["git", "rev-parse", "--verify", "HEAD^{tree}"]:
                    return completed(command, tree_returncode, tree_output + "\n")
                if command[:3] == ["git", "rev-parse", "--verify"]:
                    return completed(command, stdout=CHECKED_OUT_SHA + "\n")
                if command == ["git", "diff", "--quiet", "HEAD", "--"]:
                    return completed(command, 1 if dirty else 0)
                if command == ["cargo", "audit", "--version"]:
                    return completed(command, stdout=version_output)
                if command[:3] == ["cargo", "audit", "--json"]:
                    return completed(command, audit_returncode, audit_output)
                return completed(command, 1, stderr="unexpected command")

            env = {
                "GITHUB_SHA": github_sha,
                "GITHUB_EVENT_NAME": "push",
                "FULL_SCAN": "true",
                "CARGO_AUDIT_VERSION": "0.22.2",
            }
            with mock.patch.object(scanner, "ROOT", root), \
                 mock.patch.object(scanner, "OUT", root / "security-results"), \
                 mock.patch.object(scanner, "run", side_effect=fake_run) as run_mock, \
                 mock.patch.object(scanner, "vendor_inventory", return_value=([], [])), \
                 mock.patch.object(scanner, "db_metadata", return_value={"commit": "a" * 40, "last_updated": "2099-01-01T00:00:00Z"}), \
                 mock.patch.dict(os.environ, env, clear=False):
                result = scanner.main()
                evidence = json.loads((root / "security-results" / "rust-security.json").read_text(encoding="utf-8"))
                audit_calls = [call for call in run_mock.call_args_list if call.args[0][:3] == ["cargo", "audit", "--json"]]
        return result, evidence, len(audit_calls)

    def test_clean_full_scan_writes_sha_bound_evidence(self):
        result, evidence, count = self.run_main()
        self.assertEqual(result, 0)
        self.assertEqual(evidence["status"], "passed")
        self.assertEqual(evidence["commit_sha"], CHECKED_OUT_SHA)
        self.assertEqual(evidence["tree_sha"], TREE_SHA)
        self.assertEqual(count, len(scanner.LOCKFILES))

    def test_checkout_sha_mismatch_prevents_scanning(self):
        result, evidence, count = self.run_main(github_sha="4" * 40)
        self.assertEqual(result, 1)
        self.assertEqual(evidence["status"], "scan_failed")
        self.assertEqual(count, 0)

    def test_dirty_worktree_prevents_scanning(self):
        result, evidence, count = self.run_main(dirty=True)
        self.assertEqual(result, 1)
        self.assertEqual(evidence["status"], "scan_failed")
        self.assertEqual(count, 0)

    def test_missing_tree_sha_prevents_scanning(self):
        result, evidence, count = self.run_main(tree_output="", tree_returncode=1)
        self.assertEqual(result, 1)
        self.assertEqual(evidence["status"], "scan_failed")
        self.assertIsNone(evidence["tree_sha"])
        self.assertEqual(count, 0)

    def test_wrong_scanner_version_prevents_scanning(self):
        result, evidence, count = self.run_main(version_output="cargo-audit-audit 0.22.1\n")
        self.assertEqual(result, 1)
        self.assertEqual(evidence["status"], "scan_failed")
        self.assertEqual(count, 0)

    def test_malformed_audit_json_fails_closed(self):
        result, evidence, _ = self.run_main(audit_output="{bad json")
        self.assertEqual(result, 1)
        self.assertEqual(evidence["status"], "scan_failed")
        self.assertTrue(any("could not be parsed" in error for error in evidence["errors"]))

    def test_approval_text_cannot_waive_findings(self):
        exceptions = json.dumps([{
            "advisory_id": "RUSTSEC-2099-0001",
            "package": "example",
            "rationale": "test fixture",
            "expires": "2099-01-01",
            "owner_approval": {"approved_by": "Owner", "approved_at": "2098-01-01", "approval_ref": "approval-1"},
        }])
        audit_output = json.dumps({
            "vulnerabilities": {"list": [{
                "advisory": {"id": "RUSTSEC-2099-0001", "cvss": "9.8"},
                "package": {"name": "example"},
            }]},
            "database": {"last-updated": "2099-01-01T00:00:00Z"},
        })
        result, evidence, _ = self.run_main(
            exceptions=exceptions, audit_output=audit_output, audit_returncode=1
        )
        self.assertEqual(result, 1)
        self.assertEqual(evidence["status"], "scan_failed")
        self.assertTrue(any("exceptions are disabled" in error for error in evidence["errors"]))
        self.assertEqual(evidence["finding_counts"]["total"], len(scanner.LOCKFILES))
        self.assertEqual(len(evidence["unexcepted_findings"]), len(scanner.LOCKFILES))

    def test_audit_process_failure_fails_closed(self):
        result, evidence, _ = self.run_main(audit_returncode=127, audit_output="")
        self.assertEqual(result, 1)
        self.assertEqual(evidence["status"], "scan_failed")
        self.assertTrue(any("execution failed" in error for error in evidence["errors"]))


if __name__ == "__main__":
    unittest.main()
