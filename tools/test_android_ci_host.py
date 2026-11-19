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
        # Index download on a degraded GitHub-hosted runner exceeded the previous 45 s
        # budget while still progressing; the cap stays bounded and is asserted below.
        self.assertGreaterEqual(host.APT_TIMEOUT_SECONDS, 45)
        self.assertLessEqual(host.APT_TIMEOUT_SECONDS, 180)

    def test_apt_execution_time_limits_remain_bounded(self) -> None:
        self.assertLessEqual(host.APT_TIMEOUT_SECONDS, 180, "APT execution must keep a hard wall-clock cap")
        self.assertLessEqual(host.HTTPS_TIMEOUT_SECONDS, 20, "per-endpoint HTTPS probe must keep a hard cap")
        self.assertGreaterEqual(host.HTTPS_TIMEOUT_SECONDS, 10, "4 s per-endpoint TLS handshake budget was below observed runner latency")
        self.assertEqual(host.DNS_TIMEOUT_SECONDS, 3)

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
            ) as apt, patch.object(host, "SLEEP", side_effect=lambda _seconds: None):
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


class RecordedMirrorProbeReuseTests(unittest.TestCase):
    """A preflight FAIL must never be replayed as a final verdict."""

    RECOVERED_FAIL = (
        False,
        "noble",
        "amd64",
        ["official source"],
        [
            "official_https https://security.ubuntu.com/ubuntu/dists/noble-security/InRelease: UNREACHABLE (TIMEOUT; TLS handshake timed out)",
            "official_https_failure_classes=TIMEOUT",
        ],
    )
    PASS = (
        True,
        "noble",
        "amd64",
        ["official source"],
        [
            "official_https https://archive.ubuntu.com/ubuntu/dists/noble/InRelease: REACHABLE (HTTP 206; TLS verification enabled)",
            "official_https_failure_classes=none",
        ],
    )

    def _install(self, evidence: Path, sdk: Path):
        qemu = sdk / "emulator/qemu/linux-x86_64/qemu-system-x86_64"
        qemu.parent.mkdir(parents=True, exist_ok=True)
        qemu.write_text("test binary placeholder", encoding="utf-8")
        qemu.chmod(0o755)
        return evidence

    def test_recorded_preflight_failure_is_re_diagnosed_and_recovers(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            evidence = self._install(sdk / "host-preflight.txt", sdk)
            evidence.write_text(
                "apt_sources_sha256_before=sources-same\n"
                "official_mirror_probe=FAIL\n"
                "official_mirror_probe_reason=official Ubuntu DNS or TLS-verified HTTPS endpoint unavailable\n"
                "official_mirror_probe_cause=NETWORK_FAILURE\n",
                encoding="utf-8",
            )
            sleeps: list[float] = []
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk), "RUNNER_TEMP": temporary}), patch.object(
                host, "apt_source_snapshot", return_value="sources-same"
            ), patch.object(
                host, "library_report", side_effect=[(["libpulse.so.0 => not found"], ["libpulse.so.0"]), (["libpulse.so.0 => /lib/libpulse.so.0"], [])]
            ), patch.object(host, "package_state", side_effect=["NOT_INSTALLED (exit=1)", "libpulse0 1.0"]), patch.object(
                host, "package_installed", return_value=True
            ), patch.object(
                host, "probe_official_mirrors", side_effect=[self.RECOVERED_FAIL, self.PASS]
            ) as probe, patch.object(
                host, "SLEEP", side_effect=sleeps.append
            ), patch.object(
                host, "official_fallback", return_value=(sdk / "sources.list", ["-o", "source-options"])
            ), patch.object(
                host, "apt_invocation", side_effect=[subprocess.CompletedProcess([], 0, "official update ok"), subprocess.CompletedProcess([], 0, "Setting up libpulse0")]
            ) as apt:
                self.assertEqual(host.install_runtime(evidence, "35"), 0)
            self.assertEqual(probe.call_count, 2, "a recorded preflight FAIL must be re-diagnosed before APT")
            self.assertEqual(apt.call_count, 2)
            self.assertEqual(len(sleeps), 1)
            self.assertLessEqual(sleeps[0], host.MIRROR_PROBE_RETRY_DELAY_SECONDS)
            content = evidence.read_text(encoding="utf-8")
            self.assertIn("official_mirror_recheck attempt=1/", content)
            self.assertIn("official_mirror_recheck_result=FAIL", content)
            self.assertIn("official_mirror_recheck_failure_classes=TIMEOUT", content)
            self.assertIn("official_mirror_recheck_result=PASS", content)
            self.assertIn("official_mirror_probe=PASS", content)
            self.assertIn("result=HOST_READY", content)
            self.assertIn("apt_result=INSTALLED_SIGNED_OFFICIAL_UBUNTU", content)

    def test_exhausted_recheck_stops_as_host_infra_without_apt(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            evidence = self._install(sdk / "host-preflight.txt", sdk)
            evidence.write_text(
                "apt_sources_sha256_before=sources-same\nofficial_mirror_probe=FAIL\nofficial_mirror_probe_reason=transient\n",
                encoding="utf-8",
            )
            sleeps: list[float] = []
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk), "RUNNER_TEMP": temporary}), patch.object(
                host, "apt_source_snapshot", return_value="sources-same"
            ), patch.object(
                host, "library_report", return_value=(["libpulse.so.0 => not found"], ["libpulse.so.0"])
            ), patch.object(host, "package_state", return_value="NOT_INSTALLED (exit=1)"), patch.object(
                host, "probe_official_mirrors", return_value=self.RECOVERED_FAIL
            ) as probe, patch.object(
                host, "SLEEP", side_effect=sleeps.append
            ), patch.object(
                host, "apt_invocation", side_effect=AssertionError("APT must not run after exhausted network recheck")
            ) as apt:
                self.assertEqual(host.install_runtime(evidence, "35"), 1)
            self.assertEqual(probe.call_count, host.MIRROR_PROBE_ATTEMPTS)
            self.assertEqual(len(sleeps), host.MIRROR_PROBE_ATTEMPTS - 1)
            self.assertEqual(apt.call_count, 0)
            content = evidence.read_text(encoding="utf-8")
            self.assertIn("result=HOST_INFRA", content)
            self.assertIn("failure_cause=NETWORK_FAILURE", content)
            self.assertIn(f"official_mirror_recheck attempt={host.MIRROR_PROBE_ATTEMPTS}/{host.MIRROR_PROBE_ATTEMPTS}", content)
            self.assertIn("official_mirror_recheck_exhausted=true", content)

    def test_successful_recorded_probe_is_still_reused_without_network_access(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            evidence = self._install(sdk / "host-preflight.txt", sdk)
            evidence.write_text(
                "apt_sources_sha256_before=sources-same\n"
                "official_mirror_probe=PASS\n"
                "official_mirror_probe_reason=all official InRelease endpoints verified\n"
                "official_mirror_probe_cause=HOST_READY\n",
                encoding="utf-8",
            )
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk), "RUNNER_TEMP": temporary}), patch.object(
                host, "apt_source_snapshot", return_value="sources-same"
            ), patch.object(
                host, "library_report", side_effect=[(["libpulse.so.0 => not found"], ["libpulse.so.0"]), (["libpulse.so.0 => /lib/libpulse.so.0"], [])]
            ), patch.object(host, "package_state", side_effect=["NOT_INSTALLED", "libpulse0 1.0"]), patch.object(
                host, "package_installed", return_value=True
            ), patch.object(
                host, "probe_official_mirrors", side_effect=AssertionError("a recorded PASS must be reused, not re-probed")
            ), patch.object(
                host, "official_fallback", return_value=(sdk / "sources.list", ["-o", "source-options"])
            ), patch.object(
                host, "apt_invocation", side_effect=[subprocess.CompletedProcess([], 0, "ok"), subprocess.CompletedProcess([], 0, "ok")]
            ):
                self.assertEqual(host.install_runtime(evidence, "35"), 0)
            content = evidence.read_text(encoding="utf-8")
            self.assertIn("official_mirror_probe_reuse=RECORDED_PASS", content)
            self.assertIn("result=HOST_READY", content)

    def test_reused_probe_requires_unchanged_apt_sources(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "host-preflight.txt"
            evidence.write_text(
                "apt_sources_sha256_before=sources-a\n"
                "official_mirror_probe=PASS\n"
                "official_mirror_probe_reason=all official InRelease endpoints verified\n"
                "official_mirror_probe_cause=HOST_READY\n",
                encoding="utf-8",
            )
            with patch.object(host, "apt_source_snapshot", return_value="sources-a"):
                self.assertEqual(host.reusable_mirror_probe(evidence, "sources-a"), (True, "all official InRelease endpoints verified", "HOST_READY"))
                self.assertIsNone(host.reusable_mirror_probe(evidence, "sources-b"))
            with patch.object(host, "apt_source_snapshot", return_value="sources-b"):
                self.assertIsNone(host.reusable_mirror_probe(evidence, "sources-b"))
            evidence.write_text(
                "apt_sources_sha256_before=sources-a\nofficial_mirror_probe=FAIL\nofficial_mirror_probe_reason=transient\n",
                encoding="utf-8",
            )
            with patch.object(host, "apt_source_snapshot", return_value="sources-a"):
                self.assertIsNone(host.reusable_mirror_probe(evidence, "sources-a"))

    def test_recheck_policy_is_bounded(self) -> None:
        self.assertEqual(host.MIRROR_PROBE_ATTEMPTS, 3)
        self.assertLessEqual(host.MIRROR_PROBE_RETRY_DELAY_SECONDS, 10)
        worst_case = (
            host.MIRROR_PROBE_ATTEMPTS * (3 * host.HTTPS_TIMEOUT_SECONDS + 3 * host.DNS_TIMEOUT_SECONDS)
            + (host.MIRROR_PROBE_ATTEMPTS - 1) * host.MIRROR_PROBE_RETRY_DELAY_SECONDS
        )
        self.assertLessEqual(worst_case, 300, "bounded recheck must keep a hard total wall-clock cap")

    def test_official_source_plan_unavailable_fails_closed_without_retry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            evidence = self._install(sdk / "host-preflight.txt", sdk)
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk), "RUNNER_TEMP": temporary}), patch.object(
                host, "library_report", return_value=(["libpulse.so.0 => not found"], ["libpulse.so.0"])
            ), patch.object(host, "package_state", return_value="NOT_INSTALLED"), patch.object(
                host, "reusable_mirror_probe", return_value=None
            ), patch.object(
                host, "probe_official_mirrors", side_effect=RuntimeError("official fallback supports Ubuntu 24.04 noble only; observed ID=debian codename=bookworm")
            ) as probe, patch.object(
                host, "SLEEP", side_effect=AssertionError("unsupported OS conditions must not be retried")
            ), patch.object(
                host, "apt_invocation", side_effect=AssertionError("APT must not run for an unsupported OS")
            ) as apt:
                self.assertEqual(host.install_runtime(evidence, "35"), 1)
            self.assertEqual(probe.call_count, 1)
            self.assertEqual(apt.call_count, 0)
            content = evidence.read_text(encoding="utf-8")
            self.assertIn("result=HOST_INFRA", content)
            self.assertIn("official_mirror_probe_cause=DEPENDENCY_FAILURE", content)
            self.assertIn("failure_cause=DEPENDENCY_FAILURE", content)
            self.assertNotIn("result=HOST_READY", content)
            self.assertNotIn("official_mirror_recheck_exhausted=true", content)

    def test_system_apt_source_mutation_during_install_is_host_infra(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            evidence = self._install(sdk / "host-preflight.txt", sdk)
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk), "RUNNER_TEMP": temporary}), patch.object(
                host, "reusable_mirror_probe", return_value=(True, "all official InRelease endpoints verified", "HOST_READY")
            ), patch.object(
                host, "apt_source_snapshot", side_effect=["hash-before", "hash-after"]
            ), patch.object(
                host, "library_report", return_value=(["libpulse.so.0 => not found"], ["libpulse.so.0"])
            ), patch.object(host, "package_state", return_value="NOT_INSTALLED"), patch.object(
                host, "package_installed", return_value=True
            ), patch.object(
                host, "official_fallback", return_value=(sdk / "sources.list", ["-o", "source-options"])
            ), patch.object(
                host, "apt_invocation", side_effect=[subprocess.CompletedProcess([], 0, "ok"), subprocess.CompletedProcess([], 0, "ok")]
            ):
                self.assertEqual(host.install_runtime(evidence, "35"), 1)
            content = evidence.read_text(encoding="utf-8")
            self.assertIn("result=HOST_INFRA", content)
            self.assertIn("system APT source configuration changed", content)
            self.assertNotIn("result=HOST_READY", content)


class MirrorProbeDiagnosticsTests(unittest.TestCase):
    def test_partial_official_endpoint_reachability_fails_closed(self) -> None:
        plan = ("noble", "amd64", ("deb-line-1", "deb-line-2", "deb-line-3", *host.fallback_check_urls("noble", "amd64")))

        def fake_reachable(url: str, timeout: int = host.HTTPS_TIMEOUT_SECONDS):
            if "noble-security" in url:
                return False, "UNREACHABLE (TIMEOUT; TLS handshake timed out)"
            return True, "HTTP 206; TLS verification enabled"

        with patch.object(host, "official_mirror_plan", return_value=plan), patch.object(
            host, "resolve_host", return_value="192.0.2.10"
        ), patch.object(host, "https_reachable", side_effect=fake_reachable):
            ok, _release, _architecture, _sources, details = host.probe_official_mirrors()
        self.assertFalse(ok, "every official InRelease endpoint must be reachable")
        self.assertEqual(host.probe_failure_classes(details), ["TIMEOUT"])
        self.assertIn("official_https_failure_classes=TIMEOUT", details)

    def test_all_official_endpoints_reachable_is_the_only_success(self) -> None:
        plan = ("noble", "amd64", ("deb-line-1", "deb-line-2", "deb-line-3", *host.fallback_check_urls("noble", "amd64")))
        with patch.object(host, "official_mirror_plan", return_value=plan), patch.object(
            host, "resolve_host", return_value="192.0.2.10"
        ), patch.object(host, "https_reachable", return_value=(True, "HTTP 206; TLS verification enabled")):
            ok, _release, _architecture, _sources, details = host.probe_official_mirrors()
        self.assertTrue(ok)
        self.assertEqual(host.probe_failure_classes(details), [])
        self.assertIn("official_https_failure_classes=none", details)

    def test_dns_failure_is_classified_without_attempting_https(self) -> None:
        plan = ("noble", "amd64", ("deb-line-1", "deb-line-2", "deb-line-3", *host.fallback_check_urls("noble", "amd64")))
        with patch.object(host, "official_mirror_plan", return_value=plan), patch.object(
            host, "resolve_host", return_value="UNRESOLVED (exit=2; Temporary failure in name resolution)"
        ), patch.object(host, "https_reachable", side_effect=AssertionError("HTTPS must not be attempted for unresolved DNS")):
            ok, _release, _architecture, _sources, details = host.probe_official_mirrors()
        self.assertFalse(ok)
        self.assertEqual(host.probe_failure_classes(details), ["DNS_FAILURE"])

    def test_tls_verification_failure_is_classified_as_failure(self) -> None:
        import ssl
        import urllib.error

        error = urllib.error.URLError(ssl.SSLCertVerificationError("certificate verify failed: unable to get local issuer certificate"))
        with patch.object(host.urllib.request, "urlopen", side_effect=error):
            self.assertEqual(host.https_reachable("https://archive.ubuntu.com/ubuntu/dists/noble/InRelease"), (False, "UNREACHABLE (TLS_VERIFICATION_FAILURE; certificate verify failed)"))
        self.assertEqual(host.classify_https_failure("UNREACHABLE (HTTP Error 403: Forbidden)"), "HTTP_ERROR")
        self.assertEqual(host.classify_https_failure("UNREACHABLE (The handshake operation timed out)"), "TIMEOUT")
        self.assertEqual(host.classify_https_failure("UNREACHABLE (DNS resolution failed for archive.ubuntu.com)"), "DNS_FAILURE")

    def test_probe_failure_classes_tolerates_legacy_detail_lists(self) -> None:
        self.assertEqual(host.probe_failure_classes(["official_https x: UNREACHABLE (DNS)"]), [])
        self.assertEqual(host.probe_failure_classes(["official_https_failure_classes=none"]), [])
        self.assertEqual(host.probe_failure_classes(["official_https_failure_classes=TIMEOUT,DNS_FAILURE"]), ["TIMEOUT", "DNS_FAILURE"])

    def test_redirect_away_from_the_official_endpoint_is_rejected(self) -> None:
        class Redirected:
            status = 200

            def __enter__(self):
                return self

            def __exit__(self, *_exc):
                return False

            def read(self, _size=-1):
                return b"x"

            def geturl(self):
                return "http://example.invalid/ubuntu/dists/noble/InRelease"

        with patch.object(host.urllib.request, "urlopen", return_value=Redirected()):
            ok, detail = host.https_reachable("https://archive.ubuntu.com/ubuntu/dists/noble/InRelease")
        self.assertFalse(ok, "a redirected or downgraded response must never count as an official endpoint PASS")
        self.assertIn("REDIRECT_REJECTED", detail)

    def test_malformed_http_response_is_classified_not_escaped(self) -> None:
        import http.client

        with patch.object(host.urllib.request, "urlopen", side_effect=http.client.BadStatusLine("garbage")):
            ok, detail = host.https_reachable("https://archive.ubuntu.com/ubuntu/dists/noble/InRelease")
        self.assertFalse(ok)
        self.assertIn("UNREACHABLE (", detail)

    def test_all_endpoints_must_be_probed_for_a_pass(self) -> None:
        plan = ("noble", "amd64", ("deb-line-1", "deb-line-2", "deb-line-3", *host.fallback_check_urls("noble", "amd64")))
        with patch.object(host, "official_mirror_plan", return_value=plan), patch.object(
            host, "resolve_host", return_value="192.0.2.10"
        ), patch.object(host, "https_reachable", return_value=(True, "HTTP 206; TLS verification enabled")) as reachable:
            ok, _release, _architecture, _sources, details = host.probe_official_mirrors()
        self.assertTrue(ok)
        self.assertEqual(reachable.call_count, 3, "every official InRelease endpoint must be probed")

    def test_rotating_endpoint_failure_never_aggregates_into_success(self) -> None:
        """Three rounds where a different endpoint fails each time must still fail closed."""
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            qemu = sdk / "emulator/qemu/linux-x86_64/qemu-system-x86_64"
            qemu.parent.mkdir(parents=True)
            qemu.write_text("test binary placeholder", encoding="utf-8")
            qemu.chmod(0o755)
            evidence = sdk / "host-preflight.txt"
            plan = ("noble", "amd64", ("deb-line-1", "deb-line-2", "deb-line-3", *host.fallback_check_urls("noble", "amd64")))
            rounds = iter(range(1_000))
            state = {"round": -1}

            def rotating(url: str, timeout: int = host.HTTPS_TIMEOUT_SECONDS):
                if "/dists/noble/" in url and state["round"] == 0:
                    return False, "UNREACHABLE (TIMEOUT; timed out)"
                if "/dists/noble-updates/" in url and state["round"] == 1:
                    return False, "UNREACHABLE (TIMEOUT; timed out)"
                if "/dists/noble-security/" in url and state["round"] == 2:
                    return False, "UNREACHABLE (TIMEOUT; timed out)"
                return True, "HTTP 206; TLS verification enabled"

            def advancing(**_kwargs):
                state["round"] = next(rounds)
                return plan

            sleeps: list[float] = []
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk), "RUNNER_TEMP": temporary}), patch.object(
                host, "official_mirror_plan", side_effect=advancing
            ), patch.object(host, "resolve_host", return_value="192.0.2.10"), patch.object(
                host, "https_reachable", side_effect=rotating
            ), patch.object(host, "MONOTONIC", return_value=0.0), patch.object(
                host, "SLEEP", side_effect=sleeps.append
            ), patch.object(
                host, "library_report", return_value=(["libpulse.so.0 => not found"], ["libpulse.so.0"])
            ), patch.object(host, "package_state", return_value="NOT_INSTALLED"), patch.object(
                host, "apt_invocation", side_effect=AssertionError("APT must not run when a rotating endpoint keeps failing")
            ) as apt:
                self.assertEqual(host.install_runtime(evidence, "35"), 1)
            self.assertEqual(apt.call_count, 0)
            content = evidence.read_text(encoding="utf-8")
            self.assertEqual(content.count("official_mirror_recheck_result=FAIL"), host.MIRROR_PROBE_ATTEMPTS)
            self.assertIn("official_mirror_recheck_exhausted=true", content)
            self.assertNotIn("official_mirror_probe=PASS", content)
            self.assertNotIn("result=HOST_READY", content)


class ProbeSafetyBoundTests(unittest.TestCase):
    def test_recheck_stops_at_the_wall_clock_deadline(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            qemu = sdk / "emulator/qemu/linux-x86_64/qemu-system-x86_64"
            qemu.parent.mkdir(parents=True)
            qemu.write_text("test binary placeholder", encoding="utf-8")
            qemu.chmod(0o755)
            evidence = sdk / "host-preflight.txt"
            failing = (
                False,
                "noble",
                "amd64",
                ["official source"],
                ["official_https x: UNREACHABLE (TIMEOUT; timed out)", "official_https_failure_classes=TIMEOUT"],
            )
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk), "RUNNER_TEMP": temporary}), patch.object(
                host, "library_report", return_value=(["libpulse.so.0 => not found"], ["libpulse.so.0"])
            ), patch.object(host, "package_state", return_value="NOT_INSTALLED"), patch.object(
                host, "probe_official_mirrors", return_value=failing
            ) as probe, patch.object(
                host, "MONOTONIC", side_effect=[0.0, host.MIRROR_PROBE_DEADLINE_SECONDS + 1.0]
            ), patch.object(host, "SLEEP", side_effect=lambda _s: None), patch.object(
                host, "apt_invocation", side_effect=AssertionError("APT must not run after the probe deadline")
            ):
                self.assertEqual(host.install_runtime(evidence, "35"), 1)
            self.assertLess(probe.call_count, host.MIRROR_PROBE_ATTEMPTS, "the deadline must end the recheck instead of exhausting attempts")
            content = evidence.read_text(encoding="utf-8")
            self.assertIn("official_mirror_recheck_deadline_exceeded=true", content)
            self.assertIn("result=HOST_INFRA", content)

    def test_recheck_deadline_is_bounded(self) -> None:
        self.assertGreaterEqual(host.MIRROR_PROBE_DEADLINE_SECONDS, 30)
        self.assertLessEqual(host.MIRROR_PROBE_DEADLINE_SECONDS, 180)
        self.assertLessEqual(host.MIRROR_PROBE_DEADLINE_SECONDS, host.APT_TIMEOUT_SECONDS)

    def test_unexpected_probe_error_fails_closed_with_evidence(self) -> None:
        import http.client

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
                host, "probe_official_mirrors", side_effect=http.client.BadStatusLine("garbage")
            ) as probe, patch.object(host, "MONOTONIC", return_value=0.0), patch.object(
                host, "SLEEP", side_effect=lambda _s: None
            ), patch.object(
                host, "apt_invocation", side_effect=AssertionError("APT must not run after an unexpected probe error")
            ) as apt:
                self.assertEqual(host.install_runtime(evidence, "35"), 1)
            self.assertEqual(probe.call_count, host.MIRROR_PROBE_ATTEMPTS)
            self.assertEqual(apt.call_count, 0)
            content = evidence.read_text(encoding="utf-8")
            self.assertIn("result=HOST_INFRA", content)
            self.assertIn("official_mirror_recheck_unexpected_probe_error=BadStatusLine", content)
            self.assertIn("official_mirror_recheck_exhausted=true", content)

    def test_apt_update_uses_strict_error_mode(self) -> None:
        seen: list[list[str]] = []

        def fake_runner(args, **_kwargs):
            seen.append(list(args))
            return subprocess.CompletedProcess(args, 0, "ok")

        host.apt_invocation(("update",), runner=fake_runner)
        self.assertEqual(host.APT_UPDATE_STRICT_OPTIONS, ("-o", "APT::Update::Error-Mode=any"))
        self.assertLessEqual(len(host.APT_UPDATE_STRICT_OPTIONS), 2, "strict update mode must not add retries or unbounded options")

    def test_official_apt_update_uses_strict_error_mode(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            qemu = sdk / "emulator/qemu/linux-x86_64/qemu-system-x86_64"
            qemu.parent.mkdir(parents=True)
            qemu.write_text("test binary placeholder", encoding="utf-8")
            qemu.chmod(0o755)
            evidence = sdk / "host-preflight.txt"
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk), "RUNNER_TEMP": temporary}), patch.object(
                host, "reusable_mirror_probe", return_value=(True, "all official InRelease endpoints verified", "HOST_READY")
            ), patch.object(
                host, "library_report", side_effect=[(["libpulse.so.0 => not found"], ["libpulse.so.0"]), (["libpulse.so.0 => /lib/libpulse.so.0"], [])]
            ), patch.object(host, "package_state", side_effect=["NOT_INSTALLED", "libpulse0 1.0"]), patch.object(
                host, "package_installed", return_value=True
            ), patch.object(
                host, "official_fallback", return_value=(sdk / "sources.list", ["-o", "source-options"])
            ), patch.object(
                host, "apt_invocation", side_effect=[subprocess.CompletedProcess([], 0, "ok"), subprocess.CompletedProcess([], 0, "ok")]
            ) as apt:
                self.assertEqual(host.install_runtime(evidence, "35"), 0)
            self.assertIn("APT::Update::Error-Mode=any", apt.call_args_list[0].args[0])
            self.assertEqual(apt.call_args_list[0].args[0][-1], "update")

    def test_source_change_after_cache_validation_is_detected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sdk = Path(temporary)
            qemu = sdk / "emulator/qemu/linux-x86_64/qemu-system-x86_64"
            qemu.parent.mkdir(parents=True)
            qemu.write_text("test binary placeholder", encoding="utf-8")
            qemu.chmod(0o755)
            evidence = sdk / "host-preflight.txt"
            evidence.write_text(
                "apt_sources_sha256_before=hash-one\n"
                "official_mirror_probe=PASS\n"
                "official_mirror_probe_reason=all official InRelease endpoints verified\n"
                "official_mirror_probe_cause=HOST_READY\n",
                encoding="utf-8",
            )
            with patch.dict(os.environ, {"ANDROID_HOME": str(sdk), "RUNNER_TEMP": temporary}), patch.object(
                host, "apt_source_snapshot", side_effect=["hash-one", "hash-two"]
            ), patch.object(
                host, "library_report", side_effect=[(["libpulse.so.0 => not found"], ["libpulse.so.0"]), (["libpulse.so.0 => /lib/libpulse.so.0"], [])]
            ), patch.object(host, "package_state", side_effect=["NOT_INSTALLED", "libpulse0 1.0"]), patch.object(
                host, "package_installed", return_value=True
            ), patch.object(
                host, "official_fallback", return_value=(sdk / "sources.list", ["-o", "source-options"])
            ), patch.object(
                host, "apt_invocation", side_effect=[subprocess.CompletedProcess([], 0, "ok"), subprocess.CompletedProcess([], 0, "ok")]
            ):
                self.assertEqual(host.install_runtime(evidence, "35"), 1)
            content = evidence.read_text(encoding="utf-8")
            self.assertIn("result=HOST_INFRA", content)
            self.assertIn("system APT source configuration changed", content)
            self.assertNotIn("result=HOST_READY", content)

    def test_recheck_verdict_carries_its_own_source_hash(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            evidence = Path(temporary) / "host-preflight.txt"
            with patch.object(host, "apt_source_snapshot", return_value="hash-bound"), patch.object(
                host, "probe_official_mirrors", return_value=(True, "noble", "amd64", ["s"], ["official_https x: REACHABLE (HTTP 206; TLS verification enabled)", "official_https_failure_classes=none"])
            ):
                host.recheck_official_mirrors(evidence)
                content = evidence.read_text(encoding="utf-8")
                self.assertIn("official_mirror_probe_apt_sources_sha256=hash-bound", content)
                self.assertEqual(host.recorded_mirror_probe(evidence)[0], True)
            with patch.object(host, "apt_source_snapshot", return_value="hash-changed"):
                self.assertIsNone(host.recorded_mirror_probe(evidence))


if __name__ == "__main__":
    unittest.main()
