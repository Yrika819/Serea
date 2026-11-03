from __future__ import annotations
import hashlib
import tempfile
import unittest
import zipfile
from pathlib import Path

TARGET = "build/outputs/apk/isolated/app-isolated.apk"
TEST = "build/outputs/apk/androidTest/isolated/app-isolated-androidTest.apk"
PROVENANCE = "build/outputs/pixel7a-reacceptance/provenance.txt"

class FormalArtifactLayoutTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.archive = self.root / "build-artifact.zip"
        self.target_bytes = b"fixture isolated target APK bytes"
        self.test_bytes = b"fixture isolated instrumentation APK bytes"
        content = {
            "source_commit": "source-sha",
            "build_checkout_commit": "merge-sha",
            "github_run_id": "13228275068",
            f"isolated_apk_path_{TARGET}": TARGET,
            f"isolated_apk_path_{TEST}": TEST,
            "target_package": "app.serea.android.isolated",
            "test_package": "app.serea.android.isolated.test",
            "target_sha256": hashlib.sha256(self.target_bytes).hexdigest(),
            "test_sha256": hashlib.sha256(self.test_bytes).hexdigest(),
        }
        self.metadata = content
        lines = [f"source_commit={content['source_commit']}", f"build_checkout_commit={content['build_checkout_commit']}",
                 f"github_run_id={content['github_run_id']}", f"isolated_apk_path={TARGET}",
                 f"isolated_apk_package={content['target_package']}", f"isolated_apk_sha256={content['target_sha256']}",
                 f"isolated_apk_path={TEST}", f"isolated_apk_package={content['test_package']}",
                 f"isolated_apk_sha256={content['test_sha256']}"]
        with zipfile.ZipFile(self.archive, "w") as zf:
            zf.writestr(TARGET, self.target_bytes)
            zf.writestr(TEST, self.test_bytes)
            zf.writestr(PROVENANCE, "\n".join(lines) + "\n")

    def tearDown(self):
        self.temp.cleanup()

    def extract_and_verify(self, *, source="source-sha", build="merge-sha", run="13228275068", mutate=None):
        dest = self.root / "android/app"
        dest.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(self.archive) as zf:
            zf.extractall(dest)
        target, test = dest / TARGET, dest / TEST
        provenance = dest / PROVENANCE
        self.assertTrue(target.is_file())
        self.assertTrue(test.is_file())
        self.assertTrue(provenance.is_file())
        fields = {}
        for line in provenance.read_text().splitlines():
            if "=" in line:
                key, value = line.split("=", 1)
                fields.setdefault(key, []).append(value)
        self.assertEqual(fields["source_commit"], [source])
        self.assertEqual(fields["build_checkout_commit"], [build])
        self.assertEqual(fields["github_run_id"], [run])
        packages = fields["isolated_apk_package"]
        self.assertEqual(packages, ["app.serea.android.isolated", "app.serea.android.isolated.test"])
        self.assertEqual(fields["isolated_apk_sha256"], [hashlib.sha256(target.read_bytes()).hexdigest(), hashlib.sha256(test.read_bytes()).hexdigest()])
        self.assertEqual(fields["isolated_apk_path"], [TARGET, TEST])
        if mutate:
            mutate(fields)
        return fields

    def test_confirmed_artifact_paths_extract_and_verify(self):
        import yaml
        workflow = yaml.safe_load((Path(__file__).resolve().parents[1] / ".github/workflows/android.yml").read_text())
        jobs = workflow["jobs"]
        for api in ("26", "35"):
            step = next(s for s in jobs[f"android-emulator-api{api}"]["steps"] if s.get("name") == "Download build APKs from this workflow run")
            self.assertEqual(step["with"]["path"], "android/app")
        self.extract_and_verify()

    def test_source_build_and_run_mismatch_fails(self):
        for kwargs in ({"source":"wrong"}, {"build":"wrong"}, {"run":"wrong"}):
            with self.subTest(kwargs=kwargs), self.assertRaises(AssertionError):
                self.extract_and_verify(**kwargs)

    def test_package_and_sha_mismatch_fail_closed(self):
        original = self.archive.read_bytes()
        for old, new in (("app.serea.android.isolated.test", "wrong.package"),
                         (hashlib.sha256(self.target_bytes).hexdigest(), "0" * 64)):
            self.archive.write_bytes(original)
            with zipfile.ZipFile(self.archive, "r") as source:
                entries = [(item.filename, source.read(item.filename)) for item in source.infolist()]
            with zipfile.ZipFile(self.archive, "w") as target:
                for name, data in entries:
                    if name == PROVENANCE:
                        data = data.decode().replace(old, new).encode()
                    target.writestr(name, data)
            with self.subTest(new=new), self.assertRaises(AssertionError):
                self.extract_and_verify()

    def test_missing_or_corrupt_artifact_fails(self):
        with zipfile.ZipFile(self.archive, "w") as zf:
            zf.writestr(TEST, self.test_bytes)
        with self.assertRaises(AssertionError):
            self.extract_and_verify()

if __name__ == "__main__":
    unittest.main()
