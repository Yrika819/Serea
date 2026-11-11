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


APT_TIMEOUT_SECONDS = 90
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


def record_failure(path: Path, category: str, reason: str) -> int:
    append(path, f"result={category}\nreason={reason}")
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


def resolve_host(host: str, runner: RUN = subprocess.run) -> str:
    result = command(("getent", "ahosts", host), timeout=8, runner=runner)
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


def https_reachable(url: str, timeout: int = 8) -> tuple[bool, str]:
    request = urllib.request.Request(url, method="GET", headers={"Range": "bytes=0-0", "User-Agent": "Serea-Android-CI-Host-Preflight"})
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            response.read(1)
            return 200 <= response.status < 400, f"HTTP {response.status}; TLS verification enabled"
    except (urllib.error.URLError, TimeoutError, OSError) as error:
        return False, f"UNREACHABLE ({error})"


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
        f"uname_m={command(('uname', '-m'), timeout=5, runner=runner).stdout.strip() or platform.machine()}",
        "os_release:",
        safe_text(Path("/etc/os-release")),
        "apt_sources_before:",
        f"apt_sources_sha256_before={apt_source_snapshot()}",
    ]
    for source in apt_source_files():
        lines.append(f"--- {source} ---\n{safe_text(source)}")
    lines.append("apt_mirror_dns:")
    for host in source_hosts():
        lines.append(f"{host}: {resolve_host(host, runner)}")
    lines.append("official_ubuntu_mirror_https:")
    try:
        release, architecture, plan = official_mirror_plan()
        lines.extend((f"fallback_release={release}", f"fallback_architecture={architecture}"))
        for url in plan[3:]:
            ok, detail = https_reachable(url)
            lines.append(f"{url}: {'REACHABLE' if ok else 'UNREACHABLE'} ({detail})")
    except RuntimeError as error:
        lines.append(f"fallback_plan=UNAVAILABLE ({error})")
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
    urls = list(plan[3:])
    append(path, f"fallback_release={release}\nfallback_architecture={architecture}\napt_sources_fallback_before_update:\n" + "\n".join(lines))
    for host in sorted({urlparse(url).hostname for url in urls if urlparse(url).hostname}):
        append(path, f"fallback_dns {host}: {resolve_host(host, runner)}")
    for url in urls:
        ok, detail = https_reachable(url)
        append(path, f"fallback_official_https {url}: {'PASS' if ok else 'FAIL'} ({detail})")
        if not ok:
            raise RuntimeError(f"official Ubuntu mirror HTTPS verification failed for {url}: {detail}")
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
        append(path, "apt_result=SKIPPED\napt_skip_reason=all emulator QEMU dynamic libraries resolve; libpulse package presence recorded independently")
        print("Emulator runtime dependencies are loadable; APT installation skipped.")
        return 0
    if other_missing:
        return record_failure(path, "HOST_INFRA", f"unresolved non-libpulse emulator libraries: {','.join(other_missing)}")
    if not pulse_missing:
        return record_failure(path, "HOST_INFRA", "runtime dependency state is inconsistent")

    default_outputs: list[str] = []
    default_update = apt_invocation(("update",), runner=runner)
    default_outputs.append(f"default apt-get update exit={default_update.returncode}\n{default_update.stdout}")
    default_install: subprocess.CompletedProcess[str] | None = None
    if default_update.returncode == 0:
        default_install = apt_invocation(runtime_install_args(package), runner=runner)
        default_outputs.append(f"default apt-get install exit={default_install.returncode}\n{default_install.stdout}")
        if default_install.returncode == 0:
            after, still_missing = library_report(sdk, runner)
            append(path, "\n".join(default_outputs) + f"\napt_sources_sha256_after={apt_source_snapshot()}\nlibpulse0_package_after={package_state(runner)}\nldd_after:\n" + "\n".join(after))
            if still_missing:
                return record_failure(path, "HOST_INFRA", f"shared libraries remain unresolved: {','.join(still_missing)}")
            append(path, "apt_result=INSTALLED_DEFAULT_SOURCE\nsource_change=none")
            return 0

    failure_output = "\n".join(default_outputs)
    append(path, "\n".join(default_outputs))
    if not is_network_failure(failure_output):
        category = "HOST_INFRA" if "command timed out after" in failure_output.lower() else "DEPENDENCY_FAILURE"
        return record_failure(path, category, "default Ubuntu APT failed without a confirmed network/mirror failure; official fallback prohibited")
    append(path, "default_source_failure=CONFIRMED_NETWORK_OR_MIRROR_FAILURE\nsource_change=existing system APT sources remain untouched; trying scoped official Ubuntu sources only")

    try:
        source_file, source_options = official_fallback(path, api, runner)
    except RuntimeError as error:
        return record_failure(path, "HOST_INFRA", f"official Ubuntu fallback unavailable: {error}")
    update = apt_invocation((*source_options, "update"), runner=runner)
    append(path, f"official fallback apt-get update exit={update.returncode}\n{update.stdout}")
    if update.returncode != 0:
        append(path, f"apt_sources_sha256_after={apt_source_snapshot()}\napt_sources_after=system sources unchanged; scoped fallback was temporary")
        return record_failure(path, "HOST_INFRA", "official Ubuntu fallback update failed")
    install = apt_invocation((*source_options, *runtime_install_args(package)), runner=runner)
    append(path, f"official fallback apt-get install exit={install.returncode}\n{install.stdout}")
    after, still_missing = library_report(sdk, runner)
    append(path, f"apt_sources_sha256_after={apt_source_snapshot()}\napt_sources_after=system sources unchanged; scoped fallback source was {source_file}\nlibpulse0_package_after={package_state(runner)}\nldd_after:\n" + "\n".join(after))
    if install.returncode != 0 or still_missing:
        return record_failure(path, "HOST_INFRA", f"official fallback failed or runtime libraries remain unresolved: {','.join(still_missing)}")
    append(path, "apt_result=INSTALLED_OFFICIAL_UBUNTU_FALLBACK\napt_signature_validation=enabled; tls_validation=enabled; third_party_repositories=none")
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
    parser.add_argument("phase", choices=("initialize", "preflight", "install-runtime", "record-image"))
    parser.add_argument("--api", required=True)
    parser.add_argument("--image")
    parser.add_argument("--evidence", type=Path, default=evidence_dir() / "host-preflight.txt")
    args = parser.parse_args(argv)
    if args.phase == "initialize":
        args.evidence.parent.mkdir(parents=True, exist_ok=True)
        args.evidence.write_text(
            f"source_head={os.environ.get('GITHUB_SHA', 'unknown')}\n"
            f"run_id={os.environ.get('GITHUB_RUN_ID', 'unknown')}\n"
            f"run_attempt={os.environ.get('GITHUB_RUN_ATTEMPT', 'unknown')}\n"
            f"api={args.api}\nphase=HOST_PREFLIGHT_INITIALIZED\n",
            encoding="utf-8",
        )
        return 0
    if args.phase == "preflight":
        return preflight(args.evidence, args.api)
    if args.phase == "install-runtime":
        return install_runtime(args.evidence, args.api)
    if not args.image:
        parser.error("record-image requires --image")
    return record_image(args.evidence, args.api, args.image)


if __name__ == "__main__":
    sys.exit(main())
