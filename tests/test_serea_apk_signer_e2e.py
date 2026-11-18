import os
import subprocess
import shutil
import tempfile
import zipfile
import unittest
from pathlib import Path

from tools import serea_apk_signer as signer


SDK = Path.home() / "Library/Android/sdk"
BUILD_TOOLS = SDK / "build-tools/35.0.0"
ANDROID_JAR = SDK / "platforms/android-35/android.jar"
JDK_BIN = Path.home() / "Library/Java/JavaVirtualMachines/temurin-17.jdk/Contents/Home/bin"


@unittest.skipUnless(all(path.is_file() for path in (
        BUILD_TOOLS / "aapt", BUILD_TOOLS / "apksigner", BUILD_TOOLS / "zipalign",
        ANDROID_JAR, JDK_BIN / "keytool")), "Android Build Tools 35/JDK 17 unavailable")
class DisposableApkSigningTests(unittest.TestCase):
    def tamper_entry_payload(self, apk: Path, entry_name: str, output: Path) -> None:
        with zipfile.ZipFile(apk, "r") as archive:
            info = archive.getinfo(entry_name)
        data = bytearray(apk.read_bytes())
        offset = info.header_offset
        name_length = int.from_bytes(data[offset + 26:offset + 28], "little")
        extra_length = int.from_bytes(data[offset + 28:offset + 30], "little")
        data[offset + 30 + name_length + extra_length] ^= 0x01
        output.write_bytes(data)

    def build_unsigned_apk(self, directory: Path, lane: str, version_code: int) -> Path:
        package_name = signer.APP_IDS[lane]
        manifest = directory / "AndroidManifest.xml"
        manifest.write_text(
            f'<manifest xmlns:android="http://schemas.android.com/apk/res/android" '
            f'package="{package_name}" android:versionCode="{version_code}" '
            f'android:versionName="0.1.0-test"><uses-sdk android:minSdkVersion="26" '
            'android:targetSdkVersion="35"/><application android:label="Serea disposable"/></manifest>',
            encoding="utf-8")
        apk = directory / f"{lane}-unsigned.apk"
        subprocess.run([str(BUILD_TOOLS / "aapt"), "package", "-f", "-M", str(manifest),
                        "-I", str(ANDROID_JAR), "-F", str(apk)], check=True,
                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
        return apk

    def make_keystore(self, directory: Path, alias: str) -> tuple[Path, str]:
        password = f"disposable-{alias}-passphrase"
        keystore = directory / f"{alias}.p12"
        subprocess.run([str(JDK_BIN / "keytool"), "-genkeypair", "-storetype", "PKCS12",
                        "-keystore", str(keystore), "-storepass", password, "-keypass", password,
                        "-alias", alias, "-dname", f"CN={alias}", "-validity", "2", "-keyalg", "RSA"],
                       check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
        return keystore, password

    def test_signs_real_android_apk_and_verifies_single_expected_signer(self):
        with tempfile.TemporaryDirectory(prefix="serea-apk-sign-e2e-") as temporary:
            root = Path(temporary)
            os.chmod(root, 0o700)
            unsigned = self.build_unsigned_apk(root, "public", 42)
            keystore, password = self.make_keystore(root, "public-disposable")
            output = root / "signed.apk"

            evidence = signer.sign_apk(
                unsigned, output, lane="public", version_code=42, version_name="0.1.0-test",
                keystore=keystore, alias="public-disposable", password=password,
                aapt=BUILD_TOOLS / "aapt", zipalign=BUILD_TOOLS / "zipalign",
                apksigner=BUILD_TOOLS / "apksigner", expected_certificate_sha256=None, mode="disposable")

            self.assertEqual(evidence["application_id"], signer.APP_IDS["public"])
            self.assertEqual(evidence["version_code"], 42)
            self.assertEqual(evidence["signer_count"], 1)
            self.assertTrue(output.is_file())
            self.assertFalse(unsigned == output)
            with self.assertRaises(signer.ArtifactError):
                signer.sign_apk(
                    output, root / "resigned.apk", lane="public", version_code=42,
                    version_name="0.1.0-test", keystore=keystore, alias="public-disposable",
                    password=password, aapt=BUILD_TOOLS / "aapt", zipalign=BUILD_TOOLS / "zipalign",
                    apksigner=BUILD_TOOLS / "apksigner", expected_certificate_sha256=None,
                    mode="disposable")
            self.assertFalse((root / "resigned.apk").exists())
            with self.assertRaisesRegex(signer.ArtifactError, "lane"):
                signer.sign_apk(
                    unsigned, root / "wrong-lane.apk", lane="private", version_code=42,
                    version_name="0.1.0-test", keystore=keystore, alias="public-disposable",
                    password=password, aapt=BUILD_TOOLS / "aapt", zipalign=BUILD_TOOLS / "zipalign",
                    apksigner=BUILD_TOOLS / "apksigner", expected_certificate_sha256=None,
                    mode="disposable")
            with self.assertRaisesRegex(signer.ArtifactError, "certificate"):
                signer.sign_apk(
                    unsigned, root / "wrong-signer.apk", lane="public", version_code=42,
                    version_name="0.1.0-test", keystore=keystore, alias="public-disposable",
                    password=password, aapt=BUILD_TOOLS / "aapt", zipalign=BUILD_TOOLS / "zipalign",
                    apksigner=BUILD_TOOLS / "apksigner", expected_certificate_sha256="0" * 64,
                    mode="disposable")
            self.assertFalse((root / "wrong-lane.apk").exists())
            self.assertFalse((root / "wrong-signer.apk").exists())

    def test_rejects_tampered_android_apk_without_publishing(self):
        with tempfile.TemporaryDirectory(prefix="serea-apk-sign-tamper-") as temporary:
            root = Path(temporary)
            os.chmod(root, 0o700)
            valid = self.build_unsigned_apk(root, "public", 42)
            tampered = root / "tampered.apk"
            self.tamper_entry_payload(valid, "AndroidManifest.xml", tampered)
            keystore, password = self.make_keystore(root, "public-disposable")
            output = root / "tampered-signed.apk"
            with self.assertRaises(signer.ArtifactError):
                signer.sign_apk(
                    tampered, output, lane="public", version_code=42, version_name="0.1.0-test",
                    keystore=keystore, alias="public-disposable", password=password,
                    aapt=BUILD_TOOLS / "aapt", zipalign=BUILD_TOOLS / "zipalign",
                    apksigner=BUILD_TOOLS / "apksigner", expected_certificate_sha256=None,
                    mode="disposable")
            self.assertFalse(output.exists())

    def test_apksigner_rejects_tampered_signed_manifest_payload(self):
        with tempfile.TemporaryDirectory(prefix="serea-signed-tamper-") as temporary:
            root = Path(temporary)
            os.chmod(root, 0o700)
            unsigned = self.build_unsigned_apk(root, "private", 71)
            keystore, password = self.make_keystore(root, "private-disposable")
            signed = root / "signed.apk"
            signer.sign_apk(
                unsigned, signed, lane="private", version_code=71, version_name="0.1.0-test",
                keystore=keystore, alias="private-disposable", password=password,
                aapt=BUILD_TOOLS / "aapt", zipalign=BUILD_TOOLS / "zipalign",
                apksigner=BUILD_TOOLS / "apksigner", expected_certificate_sha256=None,
                mode="disposable")
            tampered = root / "tampered-signed.apk"
            self.tamper_entry_payload(signed, "AndroidManifest.xml", tampered)
            verification = signer._tool_run([BUILD_TOOLS / "apksigner", "verify", "--min-sdk-version", "26",
                                             "--max-sdk-version", "35", tampered])
            self.assertNotEqual(verification.returncode, 0)

    def test_source_basename_cannot_collide_with_internal_signer_files(self):
        with tempfile.TemporaryDirectory(prefix="serea-apk-sign-names-") as temporary:
            root = Path(temporary)
            os.chmod(root, 0o700)
            unsigned = self.build_unsigned_apk(root, "public", 42)
            keystore, password = self.make_keystore(root, "public-disposable")
            for name in ("aligned.apk", "signed.apk"):
                source = root / name
                shutil.copyfile(unsigned, source)
                output = root / f"result-{name}"
                signer.sign_apk(
                    source, output, lane="public", version_code=42, version_name="0.1.0-test",
                    keystore=keystore, alias="public-disposable", password=password,
                    aapt=BUILD_TOOLS / "aapt", zipalign=BUILD_TOOLS / "zipalign",
                    apksigner=BUILD_TOOLS / "apksigner", expected_certificate_sha256=None,
                    mode="disposable")
                self.assertTrue(output.is_file())
                source.unlink()

    def test_signing_helper_rejects_production_mode(self):
        with tempfile.TemporaryDirectory(prefix="serea-apk-sign-mode-") as temporary:
            root = Path(temporary)
            os.chmod(root, 0o700)
            unsigned = self.build_unsigned_apk(root, "public", 42)
            keystore, password = self.make_keystore(root, "public-disposable")
            with self.assertRaisesRegex(signer.ArtifactError, "Disposable"):
                signer.sign_apk(
                    unsigned, root / "signed.apk", lane="public", version_code=42,
                    version_name="0.1.0-test", keystore=keystore, alias="public-disposable",
                    password=password, aapt=BUILD_TOOLS / "aapt", zipalign=BUILD_TOOLS / "zipalign",
                    apksigner=BUILD_TOOLS / "apksigner", expected_certificate_sha256=None,
                    mode="production")

    def test_rejects_existing_output_without_overwriting(self):
        with tempfile.TemporaryDirectory(prefix="serea-apk-sign-output-") as temporary:
            root = Path(temporary)
            os.chmod(root, 0o700)
            unsigned = self.build_unsigned_apk(root, "private", 71)
            keystore, password = self.make_keystore(root, "private-disposable")
            output = root / "signed.apk"
            output.write_bytes(b"preserve existing output")

            with self.assertRaises(signer.ArtifactError):
                signer.sign_apk(
                    unsigned, output, lane="private", version_code=71, version_name="0.1.0-test",
                    keystore=keystore, alias="private-disposable", password=password,
                    aapt=BUILD_TOOLS / "aapt", zipalign=BUILD_TOOLS / "zipalign",
                    apksigner=BUILD_TOOLS / "apksigner", expected_certificate_sha256=None, mode="disposable")

            self.assertEqual(output.read_bytes(), b"preserve existing output")


if __name__ == "__main__":
    unittest.main()
