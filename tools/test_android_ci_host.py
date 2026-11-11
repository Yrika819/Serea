from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from tools import android_ci_host as host


class AndroidCiHostTests(unittest.TestCase):
    def test_network_failure_classification_is_specific(self) -> None:
        self.assertTrue(host.is_network_failure("Temporary failure resolving 'azure.archive.ubuntu.com'"))
        self.assertTrue(host.is_network_failure("Failed to connect: Connection timed out"))
        self.assertFalse(host.is_network_failure("COMMAND TIMED OUT after 90s"))
        self.assertFalse(host.is_network_failure("The repository is not signed."))
        self.assertFalse(host.is_network_failure("Package libpulse0 has no installation candidate"))

    def test_ldd_result_distinguishes_package_presence_from_loadability(self) -> None:
        output = """\
        libpulse.so.0 => not found
        libasound.so.2 => /lib/x86_64-linux-gnu/libasound.so.2 (0x00007f00)
        """
        self.assertEqual(host.unresolved_libraries(output), ["libpulse.so.0"])
        self.assertEqual(host.unresolved_libraries("libpulse.so.0 => /lib/libpulse.so.0 (0x0000)"), [])

    def test_official_fallback_is_release_and_architecture_scoped(self) -> None:
        amd64 = host.fallback_source_lines("noble", "amd64", "/usr/share/keyrings/ubuntu-archive-keyring.gpg")
        arm64 = host.fallback_source_lines("noble", "arm64", "/usr/share/keyrings/ubuntu-archive-keyring.gpg")
        self.assertIn("https://archive.ubuntu.com/ubuntu noble main universe", amd64[0])
        self.assertIn("arch=amd64 signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg", amd64[0])
        self.assertIn("https://ports.ubuntu.com/ubuntu-ports noble main universe", arm64[0])
        self.assertIn("https://security.ubuntu.com/ubuntu noble-security", amd64[2])
        self.assertIn("https://ports.ubuntu.com/ubuntu-ports noble-security", arm64[2])
        self.assertTrue(host.fallback_check_urls("noble", "amd64")[2].startswith("https://security.ubuntu.com/ubuntu/dists/noble-security/"))
        self.assertTrue(host.fallback_check_urls("noble", "arm64")[2].startswith("https://ports.ubuntu.com/ubuntu-ports/dists/noble-security/"))
        with self.assertRaises(RuntimeError):
            host.fallback_source_lines("jammy", "amd64", "/keyring")
        with self.assertRaises(RuntimeError):
            host.fallback_source_lines("noble", "x86", "/keyring")

    def test_loadable_emulator_runtime_skips_apt_even_if_package_database_lacks_libpulse(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            qemu = sdk / "emulator/qemu/linux-x86_64/qemu-system-x86_64"
            qemu.parent.mkdir(parents=True)
            qemu.write_text("test binary placeholder", encoding="utf-8")
            qemu.chmod(0o755)
            evidence = sdk / "host-preflight.txt"
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk)}), patch.object(
                host, "library_report", return_value=(["libpulse.so.0 => /lib/libpulse.so.0"], [])
            ), patch.object(host, "package_state", return_value="NOT_INSTALLED (exit=1)"), patch.object(
                host, "apt_invocation", side_effect=AssertionError("APT must be skipped when the runtime is loadable")
            ):
                self.assertEqual(host.install_runtime(evidence, "35"), 0)
            content = evidence.read_text(encoding="utf-8")
            self.assertIn("libpulse0_package_before=NOT_INSTALLED", content)
            self.assertIn("apt_result=SKIPPED", content)
            self.assertIn("all emulator QEMU dynamic libraries resolve", content)

    def test_apt_policy_is_bounded(self) -> None:
        self.assertIn("Acquire::Retries=1", host.APT_OPTIONS)
        self.assertIn("Acquire::http::Timeout=12", host.APT_OPTIONS)
        self.assertIn("Acquire::https::Timeout=12", host.APT_OPTIONS)
        self.assertEqual(host.APT_TIMEOUT_SECONDS, 90)

    def test_apt_invocation_applies_retries_and_timeout_to_subprocess(self) -> None:
        seen = {}

        def fake_runner(args, **kwargs):
            seen["args"] = args
            seen["kwargs"] = kwargs
            return subprocess.CompletedProcess(args, 0, "ok")

        host.apt_invocation(("update",), runner=fake_runner)
        self.assertIn("Acquire::Retries=1", seen["args"])
        self.assertIn("Acquire::http::Timeout=12", seen["args"])
        self.assertIn("Acquire::https::Timeout=12", seen["args"])
        self.assertEqual(seen["kwargs"]["timeout"], host.APT_TIMEOUT_SECONDS)

    def test_runtime_package_reinstall_repairs_package_database_loader_mismatch(self) -> None:
        self.assertNotIn("--reinstall", host.runtime_install_args("NOT_INSTALLED (exit=1)"))
        self.assertIn("--reinstall", host.runtime_install_args("libpulse0 16.1+dfsg1-2ubuntu10.1"))

    def test_source_host_extraction_redacts_credentials(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sources = root / "sources.list.d"
            sources.mkdir()
            (sources / "ubuntu.sources").write_text(
                "Types: deb\nURIs: https://user:secret@mirror.example.test/ubuntu\nSuites: noble\n",
                encoding="utf-8",
            )
            hosts = host.source_hosts(root)
            self.assertIn("mirror.example.test", hosts)
            self.assertNotIn("secret", host.safe_text(sources / "ubuntu.sources"))

    def test_official_fallback_is_used_only_after_network_failure_and_rechecks_libraries(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            qemu = sdk / "emulator/qemu/linux-x86_64/qemu-system-x86_64"
            qemu.parent.mkdir(parents=True)
            qemu.write_text("test binary placeholder", encoding="utf-8")
            qemu.chmod(0o755)
            evidence = sdk / "host-preflight.txt"
            results = [
                subprocess.CompletedProcess([], 0, "primary update ok"),
                subprocess.CompletedProcess([], 100, "Temporary failure resolving azure.archive.ubuntu.com"),
                subprocess.CompletedProcess([], 0, "official update ok"),
                subprocess.CompletedProcess([], 0, "installed"),
            ]
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk), "RUNNER_TEMP": temporary}), patch.object(
                host, "library_report", side_effect=[(["libpulse.so.0 => not found"], ["libpulse.so.0"]), (["libpulse.so.0 => /lib/libpulse.so.0"], [])]
            ), patch.object(host, "package_state", side_effect=["NOT_INSTALLED (exit=1)", "libpulse0 1.0"]), patch.object(
                host, "apt_invocation", side_effect=results
            ) as apt, patch.object(host, "official_fallback", return_value=(Path(temporary) / "sources.list", ["-o", "source-options"])), patch.object(
                host, "append"
            ):
                self.assertEqual(host.install_runtime(evidence, "26"), 0)
            self.assertEqual(apt.call_count, 4)
            self.assertIn("source-options", apt.call_args_list[2].args[0])
            self.assertIn("source-options", apt.call_args_list[3].args[0])

    def test_system_image_recording_requires_the_requested_api_image(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "host-preflight.txt"
            image = "system-images;android-35;google_apis;x86_64"
            with patch.object(host, "command", return_value=subprocess.CompletedProcess([], 0, f"Installed packages:\n{image}\n")):
                self.assertEqual(host.record_image(evidence, "35", image), 0)
            self.assertIn("android_system_image_status=INSTALLED", evidence.read_text(encoding="utf-8"))
            with patch.object(host, "command", return_value=subprocess.CompletedProcess([], 0, "Installed packages:\n")):
                self.assertEqual(host.record_image(evidence, "35", image), 1)

    def test_non_network_apt_failure_fails_closed_without_mirror_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            qemu = sdk / "emulator/qemu/linux-x86_64/qemu-system-x86_64"
            qemu.parent.mkdir(parents=True)
            qemu.write_text("test binary placeholder", encoding="utf-8")
            qemu.chmod(0o755)
            evidence = sdk / "host-preflight.txt"
            results = [
                subprocess.CompletedProcess([], 0, "primary update ok"),
                subprocess.CompletedProcess([], 100, "The repository is not signed"),
            ]
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk)}), patch.object(
                host, "library_report", return_value=(["libpulse.so.0 => not found"], ["libpulse.so.0"])
            ), patch.object(host, "package_state", return_value="NOT_INSTALLED (exit=1)"), patch.object(
                host, "apt_invocation", side_effect=results
            ) as apt, patch.object(host, "official_fallback", side_effect=AssertionError("fallback must be network-failure-only")), patch.object(
                host, "append"
            ):
                self.assertEqual(host.install_runtime(evidence, "35"), 1)
            self.assertEqual(apt.call_count, 2)


if __name__ == "__main__":
    unittest.main()
