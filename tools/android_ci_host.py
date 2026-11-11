#!/usr/bin/env python3
"""Fail-closed Android emulator host preflight and libpulse preparation."""

from __future__ import annotations

import argparse
import glob
import hashlib
import os
import platform
import re
import shutil
import subprocess
import sys
import urllib.error
import urllib.request
from urllib.parse import urlparse
from pathlib import Path
from typing import Callable, Sequence


APT_TIMEOUT_SECONDS = 45
DNS_TIMEOUT_SECONDS = 3
HTTPS_TIMEOUT_SECONDS = 4
APT_OPTIONS = (
    "-o", "Acquire::Retries=1",
    "-o", "Acquire::http::Timeout=12",
    "-o", "Acquire::https::Timeout=12",
)
NETWORK_FAILURE_MARKERS = (
    "temporary failure resolving",
    "could not resolve",
    "name or service not known",
    "connection timed out",
    "connection refused",
    "network is unreachable",
    "failed to connect",
    "could not connect",
    "connection failed",
)
RUN = Callable[..., subprocess.CompletedProcess[str]]


def evidence_dir() -> Path:
    root = os.environ.get("RUNNER_TEMP", "/tmp")
    return Path(os.environ.get("SEREA_ANDROID_HOST_EVIDENCE_DIR", root + "/serea-android-host"))


def append(path: Path, message: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as stream:
        stream.write(message.rstrip() + "\n")


def record_failure(path: Path, category: str, reason: str, cause: str | None = None) -> int:
    append(path, f"result={category}\nfailure_cause={cause or category}\nreason={reason}")
    print(f"{category}: {reason}", file=sys.stderr)
    return 1


def command(
    args: Sequence[str],
    *,
    timeout: int = 15,
    env: dict[str, str] | None = None,
    runner: RUN = subprocess.run,
) -> subprocess.CompletedProcess[str]:
    try:
        return runner(
            list(args), text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            timeout=timeout, check=False, env=env,
        )
    except subprocess.TimeoutExpired as error:
        output = error.stdout or ""
        if isinstance(output, bytes):
            output = output.decode("utf-8", errors="replace")
        return subprocess.CompletedProcess(list(args), 124, f"{output}\nCOMMAND TIMED OUT after {timeout}s")
    except OSError as error:
        return subprocess.CompletedProcess(list(args), 124, str(error))


def safe_text(path: Path, limit: int = 16_384) -> str:
    try:
        value = path.read_text(encoding="utf-8", errors="replace")[:limit]
    except OSError as error:
        return f"UNAVAILABLE ({error})"
    # APT source files should not contain credentials. Redact URL user-info defensively.
    return re.sub(r"(https?://)[^/@\s]+:[^/@\s]+@", r"\1redacted@", value)


def apt_source_files(root: Path = Path("/etc/apt")) -> list[Path]:
    paths: list[Path] = []
    for candidate in (root / "sources.list", root / "apt-mirrors.txt"):
        if candidate.is_file():
            paths.append(candidate)
    for pattern in ("*.list", "*.sources"):
        paths.extend(sorted((root / "sources.list.d").glob(pattern)))
    return paths


def apt_source_snapshot(root: Path = Path("/etc/apt")) -> str:
    digest = hashlib.sha256()
    for source_file in apt_source_files(root):
        digest.update(str(source_file).encode("utf-8"))
        digest.update(b"\0")
        digest.update(source_file.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def source_hosts(root: Path = Path("/etc/apt")) -> list[str]:
    hosts: set[str] = set()
    for source_file in apt_source_files(root):
        for uri in re.findall(r"https?://[^\s,]+", safe_text(source_file)):
            uri = uri.rstrip("/]")
            try:
                host = urlparse(uri).hostname
            except (ValueError, AttributeError):
                host = None
            if host:
                hosts.add(host)
    # GitHub-hosted Ubuntu runners may route these aliases through apt-mirrors.txt.
    for candidate in ("azure.archive.ubuntu.com", "archive.ubuntu.com", "security.ubuntu.com", "ports.ubuntu.com"):
        hosts.add(candidate)
    return sorted(hosts)


def resolve_host(host: str, runner: RUN = subprocess.run, timeout: int = DNS_TIMEOUT_SECONDS) -> str:
    result = command(("getent", "ahosts", host), timeout=timeout, runner=runner)
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip().replace("\n", "; ")
    return f"UNRESOLVED (exit={result.returncode}; {result.stdout.strip()})"


def official_mirror_plan() -> tuple[str, str, tuple[str, ...]]:
    release = ""
    os_release: dict[str, str] = {}
    try:
        for line in Path("/etc/os-release").read_text(encoding="utf-8").splitlines():
            if "=" in line:
                key, value = line.split("=", 1)
                os_release[key] = value.strip().strip('"')
    except OSError:
        pass
    release = os_release.get("VERSION_CODENAME", "")
    architecture = command(("dpkg", "--print-architecture"), timeout=5).stdout.strip()
    if os_release.get("ID") != "ubuntu" or release != "noble":
        raise RuntimeError(f"official fallback supports Ubuntu 24.04 noble only; observed ID={os_release.get('ID')} codename={release}")
    if architecture not in ("amd64", "arm64"):
        raise RuntimeError(f"unsupported Ubuntu 24.04 architecture for fallback: {architecture}")
    keyring = Path("/usr/share/keyrings/ubuntu-archive-keyring.gpg")
    if not keyring.is_file():
        raise RuntimeError(f"Ubuntu archive signing key is unavailable: {keyring}")
    lines = fallback_source_lines(release, architecture, str(keyring))
    checks = fallback_check_urls(release, architecture)
    return release, architecture, tuple(lines + list(checks))


def fallback_source_lines(release: str, architecture: str, keyring: str) -> list[str]:
    if release != "noble":
        raise RuntimeError(f"refusing official fallback for unexpected Ubuntu release: {release}")
    if architecture == "amd64":
        base = "https://archive.ubuntu.com/ubuntu"
        security_base = "https://security.ubuntu.com/ubuntu"
    elif architecture == "arm64":
        base = "https://ports.ubuntu.com/ubuntu-ports"
        security_base = base
    else:
        raise RuntimeError(f"unsupported Ubuntu 24.04 architecture for fallback: {architecture}")
    return [
        f"deb [arch={architecture} signed-by={keyring}] {base} {release} main universe",
        f"deb [arch={architecture} signed-by={keyring}] {base} {release}-updates main universe",
        f"deb [arch={architecture} signed-by={keyring}] {security_base} {release}-security main universe",
    ]


def fallback_check_urls(release: str, architecture: str) -> tuple[str, ...]:
    if release != "noble":
        raise RuntimeError(f"refusing official fallback checks for unexpected Ubuntu release: {release}")
    if architecture == "amd64":
        base = "https://archive.ubuntu.com/ubuntu"
        security_base = "https://security.ubuntu.com/ubuntu"
    elif architecture == "arm64":
        base = "https://ports.ubuntu.com/ubuntu-ports"
        security_base = base
    else:
        raise RuntimeError(f"unsupported Ubuntu 24.04 architecture for fallback: {architecture}")
    return (
        f"{base}/dists/{release}/InRelease",
        f"{base}/dists/{release}-updates/InRelease",
        f"{security_base}/dists/{release}-security/InRelease",
    )


def https_reachable(url: str, timeout: int = HTTPS_TIMEOUT_SECONDS) -> tuple[bool, str]:
    request = urllib.request.Request(url, method="GET", headers={"Range": "bytes=0-0", "User-Agent": "Serea-Android-CI-Host-Preflight"})
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            response.read(1)
            return 200 <= response.status < 400, f"HTTP {response.status}; TLS verification enabled"
    except (urllib.error.URLError, TimeoutError, OSError) as error:
        return False, f"UNREACHABLE ({error})"


def probe_official_mirrors(
    runner: RUN = subprocess.run,
    dns_cache: dict[str, str] | None = None,
) -> tuple[bool, str, str, list[str], list[str]]:
    """Check DNS and TLS-verified HTTPS for every official InRelease endpoint."""
    release, architecture, plan = official_mirror_plan()
    sources = list(plan[:3])
    urls = list(plan[3:])
    resolved = dns_cache if dns_cache is not None else {}
    details: list[str] = []
    hosts = sorted({urlparse(url).hostname or "" for url in urls} - {""})
    for host in hosts:
        if host not in resolved:
            resolved[host] = resolve_host(host, runner, timeout=DNS_TIMEOUT_SECONDS)
        details.append(f"official_dns {host}: {resolved[host]}")
    ok = True
    for url in urls:
        host = urlparse(url).hostname or ""
        if "UNRESOLVED" in resolved.get(host, "UNRESOLVED"):
            reachable = False
            detail = f"UNREACHABLE (DNS resolution failed for {host})"
        else:
            reachable, detail = https_reachable(url, timeout=HTTPS_TIMEOUT_SECONDS)
        details.append(f"official_https {url}: {'REACHABLE' if reachable else 'UNREACHABLE'} ({detail})")
        ok = ok and reachable
    return ok, release, architecture, sources, details


def recorded_mirror_probe(path: Path) -> tuple[bool, str, str] | None:
    """Reuse a preflight result only when the current APT source hash matches."""
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return None
    recorded_hash = next((line.split("=", 1)[1] for line in reversed(lines) if line.startswith("apt_sources_sha256_before=")), None)
    status = next((line.split("=", 1)[1] for line in reversed(lines) if line.startswith("official_mirror_probe=")), None)
    reason = next((line.split("=", 1)[1] for line in reversed(lines) if line.startswith("official_mirror_probe_reason=")), "no probe reason recorded")
    cause = next((line.split("=", 1)[1] for line in reversed(lines) if line.startswith("official_mirror_probe_cause=")), "NETWORK_FAILURE")
    if recorded_hash != apt_source_snapshot() or status not in ("PASS", "FAIL"):
        return None
    return status == "PASS", reason, cause


def qemu_binaries(sdk: Path) -> list[Path]:
    matches = sorted(Path(path) for path in glob.glob(str(sdk / "emulator/qemu/linux-*/qemu-system-x86_64*")))
    return [path for path in matches if path.is_file() and os.access(path, os.X_OK)]


def library_report(sdk: Path, runner: RUN = subprocess.run) -> tuple[list[str], list[str]]:
    binaries = qemu_binaries(sdk)
    if not binaries:
        return ["qemu_x86_64=NOT_FOUND"], ["Android Emulator QEMU x86_64 binary not found"]
    search_path = os.pathsep.join(str(path) for path in (sdk / "emulator/lib64", sdk / "emulator/lib64/qt/lib") if path.exists())
    env = os.environ.copy()
    env["LD_LIBRARY_PATH"] = os.pathsep.join(filter(None, (search_path, env.get("LD_LIBRARY_PATH", ""))))
    report: list[str] = []
    missing: list[str] = []
    for binary in binaries:
        result = command(("ldd", str(binary)), timeout=15, env=env, runner=runner)
        report.append(f"ldd {binary} (exit={result.returncode}):\n{result.stdout.strip()}")
        if result.returncode != 0:
            missing.append(f"ldd failed for {binary}")
        missing.extend(unresolved_libraries(result.stdout))
    return report, sorted(set(missing))


def package_state(runner: RUN = subprocess.run) -> str:
    result = command(("dpkg-query", "-W", "-f=${Package} ${Version}\\n", "libpulse0"), timeout=5, runner=runner)
    return result.stdout.strip() if result.returncode == 0 else f"NOT_INSTALLED (exit={result.returncode})"


def preflight(path: Path, api: str, runner: RUN = subprocess.run) -> int:
    lines = [
        f"phase=HOST_PREFLIGHT api={api}",
        f"source_head={os.environ.get('GITHUB_SHA', 'unknown')}",
        f"run_id={os.environ.get('GITHUB_RUN_ID', 'unknown')}",
        f"run_attempt={os.environ.get('GITHUB_RUN_ATTEMPT', 'unknown')}",
        f"runner_os={os.environ.get('RUNNER_OS', 'unknown')}",
        f"runner_arch={os.environ.get('RUNNER_ARCH', 'unknown')}",
        f"runner_image_os={os.environ.get('ImageOS', 'unknown')}",
        f"runner_image_version={os.environ.get('ImageVersion', 'unknown')}",
        f"uname_m={command(('uname', '-m'), timeout=5, runner=runner).stdout.strip() or platform.machine()}",
        "os_release:",
        safe_text(Path("/etc/os-release")),
        "apt_sources_before:",
        f"apt_sources_sha256_before={apt_source_snapshot()}",
    ]
    for source in apt_source_files():
        lines.append(f"--- {source} ---\n{safe_text(source)}")
    lines.append("apt_mirror_dns:")
    dns_results: dict[str, str] = {}
    for host in source_hosts():
        dns_results[host] = resolve_host(host, runner, timeout=DNS_TIMEOUT_SECONDS)
        lines.append(f"{host}: {dns_results[host]}")
    lines.append("official_ubuntu_mirror_https:")
    try:
        mirrors_ok, release, architecture, _sources, details = probe_official_mirrors(runner, dns_results)
        lines.extend((f"fallback_release={release}", f"fallback_architecture={architecture}"))
        lines.extend(details)
        lines.append(f"official_mirror_probe={'PASS' if mirrors_ok else 'FAIL'}")
        probe_reason = "all official InRelease endpoints verified" if mirrors_ok else "official Ubuntu DNS or TLS-verified HTTPS endpoint unavailable"
        lines.append(f"official_mirror_probe_reason={probe_reason}")
        lines.append(f"official_mirror_probe_cause={'HOST_READY' if mirrors_ok else 'NETWORK_FAILURE'}")
    except RuntimeError as error:
        lines.append(f"fallback_plan=UNAVAILABLE ({error})")
        lines.append("official_mirror_probe=FAIL")
        lines.append(f"official_mirror_probe_reason=official source plan unavailable: {error}")
        lines.append("official_mirror_probe_cause=DEPENDENCY_FAILURE")
    lines.append(f"libpulse0_package={package_state(runner)}")
    ldconfig = command(("ldconfig", "-p"), timeout=10, runner=runner)
    lines.append("ldconfig_pulse_entries:")
    pulse_lines = [line for line in ldconfig.stdout.splitlines() if "libpulse" in line]
    lines.extend(pulse_lines or [f"NONE (ldconfig exit={ldconfig.returncode})"])
    sdk = Path(os.environ.get("ANDROID_HOME", "/nonexistent"))
    emulator = sdk / "emulator/emulator"
    if emulator.is_file() and os.access(emulator, os.X_OK):
        version = command((str(emulator), "-version"), timeout=15, runner=runner)
        acceleration = command((str(emulator), "-accel-check"), timeout=15, runner=runner)
        lines.append(f"emulator_version_exit={version.returncode}\n{version.stdout.strip()}")
        lines.append(f"emulator_acceleration_check_exit={acceleration.returncode}\n{acceleration.stdout.strip()}")
    else:
        lines.append("emulator_version=NOT_INSTALLED")
        lines.append("emulator_acceleration_check=NOT_RUN")
    if Path("/dev/kvm").exists():
        stat = command(("stat", "-Lc", "device=%n owner=%U group=%G mode=%a", "/dev/kvm"), timeout=5, runner=runner)
        lines.append(f"dev_kvm=present; readable={os.access('/dev/kvm', os.R_OK)}; writable={os.access('/dev/kvm', os.W_OK)}; {stat.stdout.strip()}")
    else:
        lines.append("dev_kvm=absent")
    ldd_report, missing = library_report(sdk, runner)
    lines.append("android_emulator_qemu_dynamic_libraries:")
    lines.extend(ldd_report)
    lines.append(f"unresolved_shared_libraries={','.join(missing) if missing else 'none'}")
    append(path, "\n".join(lines))
    print(f"Host preflight saved: {path}")
    # Preflight is observational; the following runtime/KVM steps make hard decisions.
    return 0


def is_network_failure(output: str) -> bool:
    lowered = output.lower()
    return any(marker in lowered for marker in NETWORK_FAILURE_MARKERS)


def unresolved_libraries(ldd_output: str) -> list[str]:
    names = []
    for line in ldd_output.splitlines():
        match = re.match(r"\s*(\S+)\s+=>\s+not found", line)
        if match:
            names.append(match.group(1))
    return sorted(set(names))


def apt_invocation(args: Sequence[str], *, timeout: int = APT_TIMEOUT_SECONDS, runner: RUN = subprocess.run) -> subprocess.CompletedProcess[str]:
    return command(("sudo", "apt-get", *APT_OPTIONS, *args), timeout=timeout, runner=runner)


def runtime_install_args(package: str) -> tuple[str, ...]:
    args = ["install", "--no-install-recommends", "-y"]
    if package.startswith("libpulse0 "):
        args.append("--reinstall")
    args.append("libpulse0")
    return tuple(args)


def official_fallback(path: Path, api: str, runner: RUN = subprocess.run) -> tuple[Path, list[str]]:
    release, architecture, plan = official_mirror_plan()
    lines = list(plan[:3])
    append(path, f"fallback_release={release}\nfallback_architecture={architecture}\napt_sources_fallback_before_update:\n" + "\n".join(lines))
    runner_temp = Path(os.environ.get("RUNNER_TEMP", "/tmp"))
    run_id = re.sub(r"[^A-Za-z0-9_.-]", "_", os.environ.get("GITHUB_RUN_ID", "local"))
    root = runner_temp / f"serea-apt-fallback-api{api}-{run_id}"
    root.mkdir(parents=True, exist_ok=True)
    list_file = root / "sources.list"
    list_file.write_text("\n".join(lines) + "\n", encoding="utf-8")
    (root / "lists/partial").mkdir(parents=True, exist_ok=True)
    return list_file, [
        "-o", f"Dir::Etc::sourcelist={list_file}",
        "-o", "Dir::Etc::sourceparts=-",
        "-o", f"Dir::State::lists={root / 'lists'}",
        "-o", "Dir::State::status=/var/lib/dpkg/status",
    ]


def install_runtime(path: Path, api: str, runner: RUN = subprocess.run) -> int:
    sdk = Path(os.environ.get("ANDROID_HOME", "/nonexistent"))
    binaries = qemu_binaries(sdk)
    if not binaries:
        append(path, f"phase=APT_RUNTIME_PREPARATION api={api}")
        return record_failure(path, "HOST_INFRA", "Android Emulator QEMU x86_64 binary not found")
    before, missing = library_report(sdk, runner)
    package = package_state(runner)
    append(path, f"phase=APT_RUNTIME_PREPARATION api={api}\nlibpulse0_package_before={package}\nldd_before:\n" + "\n".join(before))
    pulse_missing = any(name.startswith("libpulse") for name in missing)
    other_missing = [name for name in missing if not name.startswith("libpulse")]
    if not missing:
        append(path, "result=HOST_READY\napt_result=SKIPPED\napt_skip_reason=all emulator QEMU dynamic libraries resolve; libpulse package presence recorded independently")
        print("Emulator runtime dependencies are loadable; APT installation skipped.")
        return 0
    if other_missing:
        return record_failure(path, "DEPENDENCY_FAILURE", f"unresolved non-libpulse emulator libraries: {','.join(other_missing)}")
    if not pulse_missing:
        return record_failure(path, "HOST_INFRA", "runtime dependency state is inconsistent")
    mirror_probe = recorded_mirror_probe(path)
    if mirror_probe is None:
        try:
            probe_ok, _release, _architecture, _sources, details = probe_official_mirrors(runner)
            probe_reason = "all official InRelease endpoints verified" if probe_ok else "official Ubuntu DNS or TLS-verified HTTPS endpoint unavailable"
            probe_cause = "HOST_READY" if probe_ok else "NETWORK_FAILURE"
            append(path, "\n".join(details) + f"\nofficial_mirror_probe={'PASS' if probe_ok else 'FAIL'}\nofficial_mirror_probe_reason={probe_reason}\nofficial_mirror_probe_cause={probe_cause}")
            mirror_probe = (probe_ok, probe_reason, probe_cause)
        except RuntimeError as error:
            mirror_probe = (False, f"official source plan unavailable: {error}", "DEPENDENCY_FAILURE")
            append(path, f"official_mirror_probe=FAIL\nofficial_mirror_probe_reason={mirror_probe[1]}\nofficial_mirror_probe_cause={mirror_probe[2]}")
    if not mirror_probe[0]:
        return record_failure(
            path,
            "HOST_INFRA",
            f"APT not attempted: {mirror_probe[1]}",
            cause=mirror_probe[2],
        )
    original_sources_hash = apt_source_snapshot()
    append(path, f"apt_source_policy=temporary official Ubuntu 24.04 source only; existing system sources unchanged\napt_sources_sha256_before_runtime={original_sources_hash}")
    try:
        source_file, source_options = official_fallback(path, api, runner)
    except RuntimeError as error:
        return record_failure(path, "DEPENDENCY_FAILURE", f"official Ubuntu source configuration unavailable: {error}")
    update = apt_invocation((*source_options, "update"), timeout=APT_TIMEOUT_SECONDS, runner=runner)
    append(path, f"official apt-get update exit={update.returncode}\n{update.stdout}")
    if update.returncode != 0:
        append(path, f"apt_sources_sha256_after={apt_source_snapshot()}\napt_sources_after=system sources unchanged; temporary source was {source_file}")
        cause = "NETWORK_FAILURE" if is_network_failure(update.stdout) or "command timed out after" in update.stdout.lower() else "DEPENDENCY_FAILURE"
        category = "HOST_INFRA" if cause == "NETWORK_FAILURE" else "DEPENDENCY_FAILURE"
        return record_failure(path, category, "official Ubuntu apt-get update failed", cause=cause)
    install = apt_invocation((*source_options, *runtime_install_args(package)), timeout=APT_TIMEOUT_SECONDS, runner=runner)
    append(path, f"official apt-get install exit={install.returncode}\n{install.stdout}")
    after, still_missing = library_report(sdk, runner)
    final_sources_hash = apt_source_snapshot()
    append(path, f"apt_sources_sha256_after={final_sources_hash}\napt_sources_after=system sources unchanged; temporary source was {source_file}\nlibpulse0_package_after={package_state(runner)}\nldd_after:\n" + "\n".join(after))
    if final_sources_hash != original_sources_hash:
        return record_failure(path, "HOST_INFRA", "system APT source configuration changed during temporary package installation")
    if install.returncode != 0 or still_missing:
        cause = "NETWORK_FAILURE" if is_network_failure(install.stdout) or "command timed out after" in install.stdout.lower() else "DEPENDENCY_FAILURE"
        category = "HOST_INFRA" if cause == "NETWORK_FAILURE" else "DEPENDENCY_FAILURE"
        return record_failure(path, category, f"official package install failed or runtime libraries remain unresolved: {','.join(still_missing)}", cause=cause)
    append(path, "result=HOST_READY\napt_result=INSTALLED_SIGNED_OFFICIAL_UBUNTU\napt_signature_validation=enabled; tls_validation=enabled; third_party_repositories=none")
    return 0


def record_kvm(path: Path, phase: str, runner: RUN = subprocess.run) -> int:
    present = Path("/dev/kvm").exists()
    readable = present and os.access("/dev/kvm", os.R_OK)
    writable = present and os.access("/dev/kvm", os.W_OK)
    stat = command(("stat", "-Lc", "device=%n owner=%U group=%G mode=%a", "/dev/kvm"), timeout=5, runner=runner) if present else None
    emulator = Path(os.environ.get("ANDROID_HOME", "/nonexistent")) / "emulator/emulator"
    acceleration = command((str(emulator), "-accel-check"), timeout=15, runner=runner) if emulator.is_file() else None
    append(
        path,
        "\n".join((
            f"kvm_phase={phase}",
            f"kvm_device_present={present}",
            f"kvm_readable={readable}",
            f"kvm_writable={writable}",
            f"kvm_stat={stat.stdout.strip() if stat else 'NOT_PRESENT'}",
            f"emulator_accel_exit={acceleration.returncode if acceleration else 'NOT_RUN'}",
            f"emulator_accel_output={acceleration.stdout.strip() if acceleration else 'NOT_RUN'}",
        )),
    )
    print(f"KVM {phase} diagnostic saved: present={present} readable={readable} writable={writable}")
    if phase == "after" and not (present and readable and writable and acceleration and acceleration.returncode == 0):
        return record_failure(path, "HOST_INFRA", "KVM access or emulator acceleration unavailable after udev preparation")
    if phase == "after":
        append(path, "host_kvm_readiness=PASS")
    return 0


def record_image(path: Path, api: str, image: str, runner: RUN = subprocess.run) -> int:
    result = command(("sdkmanager", "--list_installed"), timeout=45, runner=runner)
    append(path, f"phase=ANDROID_IMAGE_VERIFICATION api={api}\nexpected_system_image={image}\nsdkmanager_list_installed_exit={result.returncode}\n{result.stdout.strip()}")
    if result.returncode != 0 or image not in result.stdout:
        return record_failure(path, "DEPENDENCY_FAILURE", f"requested Android system image is not installed: {image}")
    append(path, f"android_api_level={api}\nandroid_system_image_status=INSTALLED")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("phase", choices=("initialize", "preflight", "install-runtime", "record-image", "record-kvm"))
    parser.add_argument("--api", required=True)
    parser.add_argument("--image")
    parser.add_argument("--phase-detail", choices=("before", "after"))
    parser.add_argument("--evidence", type=Path, default=evidence_dir() / "host-preflight.txt")
    args = parser.parse_args(argv)
    if args.phase == "initialize":
        args.evidence.parent.mkdir(parents=True, exist_ok=True)
        args.evidence.write_text(
            f"source_head={os.environ.get('GITHUB_SHA', 'unknown')}\n"
            f"run_id={os.environ.get('GITHUB_RUN_ID', 'unknown')}\n"
            f"run_attempt={os.environ.get('GITHUB_RUN_ATTEMPT', 'unknown')}\n"
            f"runner_os={os.environ.get('RUNNER_OS', 'unknown')}\n"
            f"runner_arch={os.environ.get('RUNNER_ARCH', 'unknown')}\n"
            f"runner_image_os={os.environ.get('ImageOS', 'unknown')}\n"
            f"runner_image_version={os.environ.get('ImageVersion', 'unknown')}\n"
            f"api={args.api}\nphase=HOST_PREFLIGHT_INITIALIZED\n",
            encoding="utf-8",
        )
        return 0
    if args.phase == "preflight":
        return preflight(args.evidence, args.api)
    if args.phase == "install-runtime":
        return install_runtime(args.evidence, args.api)
    if args.phase == "record-kvm":
        if not args.phase_detail:
            parser.error("record-kvm requires --phase-detail before|after")
        return record_kvm(args.evidence, args.phase_detail)
    if not args.image:
        parser.error("record-image requires --image")
    return record_image(args.evidence, args.api, args.image)


if __name__ == "__main__":
    sys.exit(main())
