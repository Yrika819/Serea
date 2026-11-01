from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "classify_android_changes", ROOT / "tools/classify_android_changes.py"
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class AndroidChangeScopeTests(unittest.TestCase):
    def test_android_source_and_configuration_require_formal_validation(self) -> None:
        for path in (
            "android/app/src/main/AndroidManifest.xml",
            "android/gradle.properties",
            "settings.gradle.kts",
        ):
            with self.subTest(path=path):
                self.assertEqual((True, False), MODULE.classify([path]))

    def test_workflows_tools_and_tests_are_conservatively_in_scope(self) -> None:
        for path in (
            ".github/workflows/android-calendar-readonly.yml",
            ".github/workflows/new-android-gate.yml",
            "tools/audit_gradle_dependency_candidates.py",
            "tools/instrumentation_to_junit.py",
            "tests/new_android_security_test.py",
            "scripts/build-android-release.sh",
        ):
            with self.subTest(path=path):
                self.assertEqual((True, False), MODULE.classify([path]))

    def test_deleted_and_renamed_paths_are_detected_when_diff_disables_rename_collapse(self) -> None:
        self.assertEqual(
            (True, True),
            MODULE.classify([
                "tools/verify_android_instrumentation.py",  # deleted endpoint
                "docs/renamed-verifier.md",  # renamed endpoint
            ]),
        )

    def test_newline_and_space_paths_remain_unambiguous(self) -> None:
        self.assertEqual((True, False), MODULE.classify(["android/Foo\n.kt", "tools/a verifier.py"]))

    def test_docs_only_changes_do_not_require_emulator_execution(self) -> None:
        self.assertEqual((False, True), MODULE.classify(["docs/android-ci.md", "android/README.md"]))

    def test_unrelated_rust_source_does_not_require_android_emulator(self) -> None:
        self.assertEqual((False, False), MODULE.classify(["crates/serea-host/src/main.rs"]))

    def test_malformed_paths_fail_closed(self) -> None:
        for path in ("/tmp/android.yml", "tools/../outside.sh", "tools\\checker.py"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                MODULE.classify([path])

    def test_unterminated_path_stream_fails_closed(self) -> None:
        from io import BytesIO

        original = MODULE.sys.stdin
        try:
            class Input:
                buffer = BytesIO(b"android/Manifest.xml")

            MODULE.sys.stdin = Input()
            self.assertEqual(2, MODULE.main())
        finally:
            MODULE.sys.stdin = original


if __name__ == "__main__":
    unittest.main()
