from __future__ import annotations

import hashlib
import subprocess
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VERIFY = ROOT / "tools/verify_android_build_provenance.py"
TARGET = "build/outputs/apk/isolated/app-isolated.apk"
TEST = "build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk"
DEBUG = "build/outputs/apk/debug/app-debug.apk"
PROVENANCE = "build/outputs/pixel7a-reacceptance/provenance.txt"


class BuildProvenanceFailureInjectionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.archive = self.root / "confirmed-build-artifact.zip"
        self.destination = self.root / "artifact"
        self.destination.mkdir()
        self.target_bytes = b"target APK fixture bytes"
        self.test_bytes = b"test APK fixture bytes"
        self.debug_bytes = b"debug APK fixture bytes"
        self.aapt = self.root / "aapt"
        self.aapt.write_text(
            "#!/usr/bin/env python3\n"
            "import pathlib, sys\n"
            "name = pathlib.Path(sys.argv[-1]).name\n"
            "packages = {'app-isolated.apk':'app.serea.android.isolated', "
            "'app-isolated-androidTest.apk':'app.serea.android.isolated.test', "
            "'app-debug.apk':'app.serea.android.demo'}\n"
            "print(\"package: name='%s'\" % packages[name])\n",
            encoding="utf-8",
        )
        self.aapt.chmod(0o755)
        self.lines = [
            "source_commit=source-sha",
            "build_checkout_commit=merge-sha",
            "github_run_id=77",
            "github_run_attempt=1",
            "workflow=Android",
            "package=app.serea.android.demo",
            f"apk_sha256={hashlib.sha256(self.debug_bytes).hexdigest()}",
            "build_lint_instrumentation_compile=PASS",
            "isolated_apk_path=android/app/build/outputs/apk/isolated/app-isolated.apk",
            "isolated_apk_package=app.serea.android.isolated",
            f"isolated_apk_sha256={hashlib.sha256(self.target_bytes).hexdigest()}",
            "isolated_apk_path=android/app/build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk",
            "isolated_apk_package=app.serea.android.isolated.test",
            f"isolated_apk_sha256={hashlib.sha256(self.test_bytes).hexdigest()}",
        ]
        self.write_archive(self.lines)
        with zipfile.ZipFile(self.archive) as artifact:
            artifact.extractall(self.destination)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def write_archive(self, lines: list[str]) -> None:
        with zipfile.ZipFile(self.archive, "w") as artifact:
            artifact.writestr(TARGET, self.target_bytes)
            artifact.writestr(TEST, self.test_bytes)
            artifact.writestr(DEBUG, self.debug_bytes)
            artifact.writestr(PROVENANCE, "\n".join(lines) + "\n")

    @property
    def provenance_path(self) -> Path:
        return self.destination / PROVENANCE

    def invoke(self) -> subprocess.CompletedProcess[str]:
        root = self.destination / "build/outputs"
        return subprocess.run(
            ["python3", str(VERIFY), "--provenance", str(self.provenance_path),
             "--source-sha", "source-sha", "--build-sha", "merge-sha", "--run-id", "77",
             "--debug-apk", str(root / "apk/debug/app-debug.apk"),
             "--target-apk", str(root / "apk/isolated/app-isolated.apk"),
             "--test-apk", str(root / "apk/androidTest/isolated/app-isolated-androidTest.apk"),
             "--aapt", str(self.aapt)], capture_output=True, text=True, cwd=ROOT,
        )

    def test_normal_confirmed_artifact_passes(self) -> None:
        result = self.invoke()
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)
        self.assertIn("PASS: Build APK provenance", result.stdout)

    def assert_provenance_fails(self, lines: list[str], key: str) -> None:
        self.write_archive(lines)
        with zipfile.ZipFile(self.archive) as artifact:
            artifact.extractall(self.destination)
        result = self.invoke()
        self.assertNotEqual(0, result.returncode, result.stdout + result.stderr)
        self.assertIn(key, result.stderr)

    def test_duplicate_scalar_fields_same_and_conflicting_fail(self) -> None:
        for key in ("source_commit", "build_checkout_commit", "github_run_id", "github_run_attempt",
                    "package", "apk_sha256", "build_lint_instrumentation_compile"):
            line = next(value for value in self.lines if value.startswith(key + "="))
            value = line.split("=", 1)[1]
            for duplicate in (line, f"{key}=conflicting-value"):
                with self.subTest(key=key, duplicate=duplicate):
                    self.assert_provenance_fails(self.lines + [duplicate], key)

    def test_required_scalar_fields_missing_or_empty_fail(self) -> None:
        for key in ("source_commit", "build_checkout_commit", "github_run_id", "github_run_attempt",
                    "package", "apk_sha256", "build_lint_instrumentation_compile"):
            with self.subTest(key=key, missing=True):
                self.assert_provenance_fails([line for line in self.lines if not line.startswith(key + "=")], key)
            with self.subTest(key=key, empty=True):
                self.assert_provenance_fails(
                    [f"{key}=" if line.startswith(key + "=") else line for line in self.lines], key)

    def test_extra_reordered_or_rebound_target_test_records_fail(self) -> None:
        extra = self.lines + ["isolated_apk_path=unexpected.apk"]
        self.assert_provenance_fails(extra, "isolated_apk_path/package/sha256")
        records = self.lines.copy()
        first = records.index("isolated_apk_path=android/app/build/outputs/apk/isolated/app-isolated.apk")
        records[first], records[first + 3] = records[first + 3], records[first]
        self.assert_provenance_fails(records, "isolated_apk_path")
        for old, replacement, key in (
            ("isolated_apk_package=app.serea.android.isolated", "isolated_apk_package=app.serea.android.isolated.test", "isolated_apk_package"),
            (f"isolated_apk_sha256={hashlib.sha256(self.target_bytes).hexdigest()}",
             f"isolated_apk_sha256={hashlib.sha256(self.test_bytes).hexdigest()}", "isolated_apk_sha256[target]"),
        ):
            changed = self.lines.copy()
            index = changed.index(old)
            changed[index] = replacement
            self.assert_provenance_fails(changed, key)

        package_lines = [i for i, line in enumerate(self.lines) if line.startswith("isolated_apk_package=")]
        swapped_packages = self.lines.copy()
        swapped_packages[package_lines[0]], swapped_packages[package_lines[1]] = (
            swapped_packages[package_lines[1]], swapped_packages[package_lines[0]])
        self.assert_provenance_fails(swapped_packages, "isolated_apk_package")

        sha_lines = [i for i, line in enumerate(self.lines) if line.startswith("isolated_apk_sha256=")]
        swapped_hashes = self.lines.copy()
        swapped_hashes[sha_lines[0]], swapped_hashes[sha_lines[1]] = (
            swapped_hashes[sha_lines[1]], swapped_hashes[sha_lines[0]])
        self.assert_provenance_fails(swapped_hashes, "isolated_apk_sha256[target]")

    def test_missing_or_empty_isolated_apk_records_fail(self) -> None:
        field_lines = [line for line in self.lines if line.startswith((
            "isolated_apk_path=", "isolated_apk_package=", "isolated_apk_sha256="))]
        for index, line in enumerate(field_lines):
            with self.subTest(line=line, missing=True):
                changed = self.lines.copy()
                changed.remove(line)
                self.assert_provenance_fails(changed, "isolated_apk_path/package/sha256")
            with self.subTest(line=line, empty=True):
                changed = self.lines.copy()
                position = changed.index(line)
                key = line.split("=", 1)[0]
                changed[position] = f"{key}="
                self.assert_provenance_fails(changed, key)


if __name__ == "__main__":
    unittest.main()
