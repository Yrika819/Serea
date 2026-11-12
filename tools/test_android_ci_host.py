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
        self.assertEqual(host.APT_TIMEOUT_SECONDS, 45)

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

    def test_package_installed_requires_dpkg_installed_state(self) -> None:
        self.assertTrue(host.package_installed(
            runner=lambda _args, **_kwargs: subprocess.CompletedProcess([], 0, "installed\n")
        ))
        self.assertFalse(host.package_installed(
            runner=lambda _args, **_kwargs: subprocess.CompletedProcess([], 0, "unpacked\n")
        ))
        self.assertFalse(host.package_installed(
            runner=lambda _args, **_kwargs: subprocess.CompletedProcess([], 1, "")
        ))

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

    def test_official_package_source_is_used_only_after_dns_and_https_probe_pass(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            qemu = sdk / "emulator/qemu/linux-x86_64/qemu-system-x86_64"
            qemu.parent.mkdir(parents=True)
            qemu.write_text("test binary placeholder", encoding="utf-8")
            qemu.chmod(0o755)
            evidence = sdk / "host-preflight.txt"
            results = [
                subprocess.CompletedProcess([], 0, "official update ok"),
                subprocess.CompletedProcess([], 0, "installed"),
            ]
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk), "RUNNER_TEMP": temporary}), patch.object(
                host, "library_report", side_effect=[(["libpulse.so.0 => not found"], ["libpulse.so.0"]), (["libpulse.so.0 => /lib/libpulse.so.0"], [])]
            ), patch.object(host, "package_state", side_effect=["NOT_INSTALLED (exit=1)", "libpulse0 1.0"]), patch.object(
                host, "probe_official_mirrors", return_value=(True, "noble", "amd64", ["official source"], ["HTTPS PASS"])
            ), patch.object(host, "apt_invocation", side_effect=results) as apt, patch.object(
                host, "official_fallback", return_value=(Path(temporary) / "sources.list", ["-o", "source-options"])
            ):
                self.assertEqual(host.install_runtime(evidence, "26"), 0)
            self.assertEqual(apt.call_count, 2)
            self.assertEqual(apt.call_args_list[0].args[0][-1], "update")
            self.assertIn("source-options", apt.call_args_list[0].args[0])
            self.assertIn("source-options", apt.call_args_list[1].args[0])
            content = evidence.read_text(encoding="utf-8")
            self.assertIn("apt_result=INSTALLED_SIGNED_OFFICIAL_UBUNTU", content)
            self.assertIn("apt_signature_validation=enabled; tls_validation=enabled; third_party_repositories=none", content)
            self.assertRegex(content, r"apt_sources_sha256_before_runtime=([0-9a-f]{64})")
            self.assertRegex(content, r"apt_sources_sha256_after=([0-9a-f]{64})")
            before_hash = next(line.split("=", 1)[1] for line in content.splitlines() if line.startswith("apt_sources_sha256_before_runtime="))
            after_hash = next(line.split("=", 1)[1] for line in content.splitlines() if line.startswith("apt_sources_sha256_after="))
            self.assertEqual(before_hash, after_hash)

    def test_system_image_recording_requires_the_requested_api_image(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "host-preflight.txt"
            image = "system-images;android-35;google_apis;x86_64"
            with patch.object(host, "command", return_value=subprocess.CompletedProcess([], 0, f"Installed packages:\n{image}\n")):
                self.assertEqual(host.record_image(evidence, "35", image), 0)
            self.assertIn("android_system_image_status=INSTALLED", evidence.read_text(encoding="utf-8"))
            with patch.object(host, "command", return_value=subprocess.CompletedProcess([], 0, "Installed packages:\n")):
                self.assertEqual(host.record_image(evidence, "35", image), 1)

    def test_unreachable_official_mirror_fails_before_apt(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            qemu = sdk / "emulator/qemu/linux-x86_64/qemu-system-x86_64"
            qemu.parent.mkdir(parents=True)
            qemu.write_text("test binary placeholder", encoding="utf-8")
            qemu.chmod(0o755)
            evidence = sdk / "host-preflight.txt"
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk)}), patch.object(
                host, "library_report", return_value=(["libpulse.so.0 => not found"], ["libpulse.so.0"])
            ), patch.object(host, "package_state", return_value="NOT_INSTALLED (exit=1)"), patch.object(
                host,
                "probe_official_mirrors",
                return_value=(False, "noble", "amd64", ["official source"], ["archive.ubuntu.com: UNRESOLVED", "HTTPS: UNREACHABLE (DNS)"]),
            ), patch.object(
                host, "apt_invocation", side_effect=AssertionError("APT must not run when official mirrors are unreachable")
            ) as apt:
                self.assertEqual(host.install_runtime(evidence, "35"), 1)
            self.assertEqual(apt.call_count, 0)
            self.assertIn("result=HOST_INFRA", evidence.read_text(encoding="utf-8"))
            self.assertIn("failure_cause=NETWORK_FAILURE", evidence.read_text(encoding="utf-8"))

    def test_recorded_mirror_probe_is_reused_only_for_same_apt_sources(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "host-preflight.txt"
            with patch.object(host, "apt_source_snapshot", return_value="sources-a"):
                evidence.write_text(
                    "apt_sources_sha256_before=sources-a\nofficial_mirror_probe=FAIL\nofficial_mirror_probe_reason=DNS failure\n",
                    encoding="utf-8",
                )
                self.assertEqual(host.recorded_mirror_probe(evidence), (False, "DNS failure", "NETWORK_FAILURE"))
            with patch.object(host, "apt_source_snapshot", return_value="sources-b"):
                self.assertIsNone(host.recorded_mirror_probe(evidence))

    def test_preflight_records_runner_and_official_network_provenance(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "host-preflight.txt"
            with patch.dict(os.environ, {"GITHUB_SHA": "head", "GITHUB_RUN_ID": "run", "GITHUB_RUN_ATTEMPT": "2", "RUNNER_OS": "Linux", "RUNNER_ARCH": "X64", "ImageOS": "ubuntu24", "ImageVersion": "2026.11"}), patch.object(
                host, "source_hosts", return_value=["azure.archive.ubuntu.com", "archive.ubuntu.com"]
            ), patch.object(
                host, "resolve_host", side_effect=lambda name, runner=subprocess.run, timeout=host.DNS_TIMEOUT_SECONDS: "192.0.2.1" if name == "archive.ubuntu.com" else "UNRESOLVED (timeout)"
            ), patch.object(
                host, "probe_official_mirrors", return_value=(False, "noble", "amd64", ["source"], ["official_https https://archive.ubuntu.com/ubuntu/dists/noble/InRelease: UNREACHABLE (DNS)"])
            ), patch.object(host, "package_state", return_value="NOT_INSTALLED"), patch.object(
                host, "command", return_value=subprocess.CompletedProcess([], 0, "")
            ), patch.object(host, "library_report", return_value=(["libpulse.so.0 => not found"], ["libpulse.so.0"])):
                self.assertEqual(host.preflight(evidence, "probe"), 0)
            text = evidence.read_text(encoding="utf-8")
            for expected in ("source_head=head", "run_id=run", "run_attempt=2", "runner_image_os=ubuntu24", "runner_image_version=2026.11", "official_mirror_probe=FAIL", "official_mirror_probe_cause=NETWORK_FAILURE", "apt_sources_sha256_before="):
                self.assertIn(expected, text)

    def test_official_apt_update_non_network_failure_is_dependency_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            qemu = sdk / "emulator/qemu/linux-x86_64/qemu-system-x86_64"
            qemu.parent.mkdir(parents=True)
            qemu.write_text("test binary placeholder", encoding="utf-8")
            qemu.chmod(0o755)
            evidence = sdk / "host-preflight.txt"
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk), "RUNNER_TEMP": temporary}), patch.object(
                host, "library_report", return_value=(["libpulse.so.0 => not found"], ["libpulse.so.0"])
            ), patch.object(host, "package_state", return_value="NOT_INSTALLED"), patch.object(
                host, "probe_official_mirrors", return_value=(True, "noble", "amd64", ["official source"], ["HTTPS PASS"])
            ), patch.object(host, "official_fallback", return_value=(Path(temporary) / "sources.list", ["-o", "source-options"])), patch.object(
                host, "apt_invocation", return_value=subprocess.CompletedProcess([], 100, "The repository is not signed")
            ):
                self.assertEqual(host.install_runtime(evidence, "35"), 1)
            content = evidence.read_text(encoding="utf-8")
            self.assertIn("result=DEPENDENCY_FAILURE", content)
            self.assertIn("failure_cause=DEPENDENCY_FAILURE", content)

    def test_apt_timeout_after_successful_probe_is_host_network_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            qemu = sdk / "emulator/qemu/linux-x86_64/qemu-system-x86_64"
            qemu.parent.mkdir(parents=True)
            qemu.write_text("test binary placeholder", encoding="utf-8")
            qemu.chmod(0o755)
            evidence = sdk / "host-preflight.txt"
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk), "RUNNER_TEMP": temporary}), patch.object(
                host, "library_report", return_value=(["libpulse.so.0 => not found"], ["libpulse.so.0"])
            ), patch.object(host, "package_state", return_value="NOT_INSTALLED"), patch.object(
                host, "probe_official_mirrors", return_value=(True, "noble", "amd64", ["official source"], ["HTTPS PASS"])
            ), patch.object(host, "official_fallback", return_value=(Path(temporary) / "sources.list", ["-o", "source-options"])), patch.object(
                host, "apt_invocation", return_value=subprocess.CompletedProcess([], 124, "COMMAND TIMED OUT after 45s")
            ) as apt:
                self.assertEqual(host.install_runtime(evidence, "35"), 1)
            self.assertEqual(apt.call_count, 1)
            content = evidence.read_text(encoding="utf-8")
            self.assertIn("result=HOST_INFRA", content)
            self.assertIn("failure_cause=NETWORK_FAILURE", content)

    def test_apt_install_timeout_is_accepted_only_when_package_and_loader_are_verified(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            qemu = sdk / "emulator/qemu/linux-x86_64/qemu-system-x86_64"
            qemu.parent.mkdir(parents=True)
            qemu.write_text("test binary placeholder", encoding="utf-8")
            qemu.chmod(0o755)
            evidence = sdk / "host-preflight.txt"
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk), "RUNNER_TEMP": temporary}), patch.object(
                host, "library_report", side_effect=[(["libpulse.so.0 => not found"], ["libpulse.so.0"]), (["libpulse.so.0 => /lib/libpulse.so.0"], [])]
            ), patch.object(host, "package_state", side_effect=["NOT_INSTALLED (exit=1)", "libpulse0 1.0"]), patch.object(
                host, "package_installed", return_value=True
            ), patch.object(
                host, "probe_official_mirrors", return_value=(True, "noble", "amd64", ["official source"], ["HTTPS PASS"])
            ), patch.object(
                host, "official_fallback", return_value=(Path(temporary) / "sources.list", ["-o", "source-options"])
            ), patch.object(
                host,
                "apt_invocation",
                side_effect=[subprocess.CompletedProcess([], 0, "official update ok"), subprocess.CompletedProcess([], 124, "COMMAND TIMED OUT after 45s\\nSetting up libpulse0")],
            ) as apt:
                self.assertEqual(host.install_runtime(evidence, "26"), 0)
            self.assertEqual(apt.call_count, 2)
            content = evidence.read_text(encoding="utf-8")
            self.assertIn("apt_install_timeout_recovered=yes", content)
            self.assertIn("libpulse0_status_after=installed", content)
            self.assertIn("result=HOST_READY", content)
            self.assertIn("apt_result=INSTALL_TIMED_OUT_BUT_RUNTIME_VERIFIED", content)

    def test_pre_udev_kvm_inaccessibility_is_recorded_not_failed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "host-preflight.txt"
            with patch.object(host.Path, "exists", return_value=False), patch.object(host.Path, "is_file", return_value=False):
                self.assertEqual(host.record_kvm(evidence, "before"), 0)
                self.assertEqual(host.record_kvm(evidence, "after"), 1)
            content = evidence.read_text(encoding="utf-8")
            self.assertIn("kvm_phase=before", content)
            self.assertIn("kvm_phase=after", content)
            self.assertIn("result=HOST_INFRA", content)

    def test_unresolved_non_pulse_runtime_dependency_is_dependency_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            qemu = sdk / "emulator/qemu/linux-x86_64/qemu-system-x86_64"
            qemu.parent.mkdir(parents=True)
            qemu.write_text("test binary placeholder", encoding="utf-8")
            qemu.chmod(0o755)
            evidence = sdk / "host-preflight.txt"
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk)}), patch.object(
                host, "library_report", return_value=(["libEGL.so => not found"], ["libEGL.so"])
            ), patch.object(host, "package_state", return_value="libpulse0 1.0"):
                self.assertEqual(host.install_runtime(evidence, "35"), 1)
            self.assertIn("result=DEPENDENCY_FAILURE", evidence.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
