#!/usr/bin/env python3
"""Fail-closed Android emulator host preflight and libpulse preparation."""

from __future__ import annotations

import argparse
import glob
import hashlib
import http.client
import os
import platform
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from urllib.parse import urlparse
from pathlib import Path
from typing import Callable, Sequence


APT_TIMEOUT_SECONDS = 180
DNS_TIMEOUT_SECONDS = 3
# A 4 s per-endpoint cap produced TLS handshake timeouts on degraded GitHub-hosted
# runners while the same endpoint verified moments later; keep a hard cap but above
# the observed handshake latency.
HTTPS_TIMEOUT_SECONDS = 10
# Bounded re-diagnosis budget for a transient official-mirror connectivity failure.
# Worst case adds MIRROR_PROBE_ATTEMPTS * (3 probes) + (attempts - 1) * delay seconds.
MIRROR_PROBE_ATTEMPTS = 3
MIRROR_PROBE_RETRY_DELAY_SECONDS = 5
# Hard wall-clock ceiling for one re-diagnosis, so a slow endpoint cannot extend the
# bounded attempt budget indefinitely.
MIRROR_PROBE_DEADLINE_SECONDS = 90
SLEEP: Callable[[float], None] = time.sleep
MONOTONIC: Callable[[], float] = time.monotonic
HTTPS_FAILURE_MARKERS = (
    ("DNS_FAILURE", ("dns resolution failed", "name or service not known", "temporary failure in name resolution", "nodename nor servname", "getaddrinfo failed", "name resolution", "unresolved")),
    ("TLS_VERIFICATION_FAILURE", ("certificate verify failed", "certificate_verify_failed", "self-signed certificate", "self signed certificate", "hostname mismatch", "unable to get local issuer", "unknown ca")),
    ("HTTP_ERROR", ("http error", "http status")),
    ("TIMEOUT", ("timed out", "timeout")),
)
HTTPS_FAILURE_REASONS = {
    "DNS_FAILURE": "DNS resolution failed",
    "TLS_VERIFICATION_FAILURE": "certificate verify failed",
    "HTTP_ERROR": "HTTP status failure",
    "REDIRECT_REJECTED": "response was redirected away from the official endpoint",
    "TIMEOUT": "timed out",
    "CONNECTION_FAILURE": "connection failed",
}

APT_OPTIONS = (
    "-o", "Acquire::Retries=1",
    "-o", "Acquire::http::Timeout=12",
    "-o", "Acquire::https::Timeout=12",
)
# Index fetch warnings must fail the update instead of continuing with partial indexes.
APT_UPDATE_STRICT_OPTIONS = ("-o", "APT::Update::Error-Mode=any")
# Never follow a mirror redirect to another host; official sources only. The
# generic keys are repeated per official host because APT resolves host-specific
# Acquire settings before the generic ones.
OFFICIAL_SOURCE_HOSTS = ("archive.ubuntu.com", "security.ubuntu.com", "ports.ubuntu.com")
APT_REDIRECT_OPTIONS = tuple(
    ["-o", "Acquire::http::AllowRedirect=false", "-o", "Acquire::https::AllowRedirect=false"]
    + [
        option
        for host in OFFICIAL_SOURCE_HOSTS
        for option in (
            "-o", f"Acquire::http::{host}::AllowRedirect=false",
            "-o", f"Acquire::https::{host}::AllowRedirect=false",
        )
    ]
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


def classify_https_failure(detail: str) -> str:
    """Map an HTTPS probe detail to a machine-readable network failure class."""
    lowered = detail.lower()
    for category, markers in HTTPS_FAILURE_MARKERS:
        if any(marker in lowered for marker in markers):
            return category
    return "CONNECTION_FAILURE"


def https_failure_detail(failure_class: str, status: int | None = None) -> str:
    reason = f"HTTP status {status}" if status is not None else HTTPS_FAILURE_REASONS.get(failure_class, "connection failed")
    return f"UNREACHABLE ({failure_class}; {reason})"


class NoRedirectHandler(urllib.request.HTTPRedirectHandler):
    """Refuse redirects instead of following them to another host or scheme."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):  # noqa: D401 - urllib hook
        return None


OPENER = urllib.request.build_opener(NoRedirectHandler)


def https_reachable(url: str, timeout: int = HTTPS_TIMEOUT_SECONDS) -> tuple[bool, str]:
    request = urllib.request.Request(url, method="GET", headers={"Range": "bytes=0-0", "User-Agent": "Serea-Android-CI-Host-Preflight"})
    try:
        with OPENER.open(request, timeout=timeout) as response:
            # Defence in depth: only the exact requested official URL may be a PASS.
            if response.geturl().split("#", 1)[0] != url:
                return False, https_failure_detail("REDIRECT_REJECTED")
            response.read(1)
            if 200 <= response.status < 400:
                return True, f"HTTP {response.status}; TLS verification enabled"
            return False, https_failure_detail("HTTP_ERROR", status=response.status)
    except urllib.error.HTTPError as error:
        if 300 <= error.code < 400:
            return False, https_failure_detail("REDIRECT_REJECTED")
        return False, https_failure_detail("HTTP_ERROR", status=error.code)
    except (urllib.error.URLError, http.client.HTTPException, TimeoutError, OSError) as error:
        # TLS verification is never disabled: certificate failures are terminal FAIL results.
        return False, https_failure_detail(classify_https_failure(f"{type(error).__name__}: {error}"))


def probe_official_mirrors(
    runner: RUN = subprocess.run,
    dns_cache: dict[str, str] | None = None,
    https_timeout: int = HTTPS_TIMEOUT_SECONDS,
) -> tuple[bool, str, str, list[str], list[str]]:
    """Check DNS and TLS-verified HTTPS for every official InRelease endpoint."""
    release, architecture, plan = official_mirror_plan()
    sources = list(plan[:3])
    urls = list(plan[3:])
    resolved = dns_cache if dns_cache is not None else {}
    details: list[str] = []
    failure_classes: list[str] = []
    hosts = sorted({urlparse(url).hostname or "" for url in urls} - {""})
    for host in hosts:
        if host not in resolved:
            resolved[host] = resolve_host(host, runner, timeout=DNS_TIMEOUT_SECONDS)
        details.append(f"official_dns {host}: {resolved[host]}")
    for url in urls:
        host = urlparse(url).hostname or ""
        if "UNRESOLVED" in resolved.get(host, "UNRESOLVED"):
            reachable = False
            detail = f"UNREACHABLE (DNS_FAILURE; DNS resolution failed for {host})"
        else:
            reachable, detail = https_reachable(url, timeout=https_timeout)
        details.append(f"official_https {url}: {'REACHABLE' if reachable else 'UNREACHABLE'} ({detail})")
        if not reachable:
            failure_classes.append(classify_https_failure(detail))
    details.append("official_https_failure_classes=" + (",".join(sorted(set(failure_classes))) if failure_classes else "none"))
    return not failure_classes, release, architecture, sources, details


def plan_fingerprint(release: str, architecture: str, sources: Sequence[str]) -> str:
    """Stable identity of the official endpoint plan a verdict was produced for."""
    digest = hashlib.sha256()
    digest.update(f"{release}/{architecture}".encode("utf-8"))
    for line in sources:
        digest.update(b"\0")
        digest.update(line.encode("utf-8"))
    return f"{release}/{architecture}/{digest.hexdigest()[:16]}"


def current_plan_fingerprint() -> str:
    release, architecture, plan = official_mirror_plan()
    return plan_fingerprint(release, architecture, plan[:3])


def probe_failure_classes(details: Sequence[str]) -> list[str]:
    for line in reversed(list(details)):
        if line.startswith("official_https_failure_classes="):
            value = line.split("=", 1)[1].strip()
            if value in ("", "none"):
                return []
            return [part for part in value.split(",") if part]
    return []


def _parse_bound_records(lines: Sequence[str]) -> list[dict[str, str]]:
    """Parse only complete ``record_start``/``record_end`` verdict records.

    Fields are first-wins, so a later orphan hash line can never rebind an
    accepted verdict, and a record without an end marker is not a verdict at all.
    """
    records: list[dict[str, str]] = []
    current: dict[str, str] | None = None
    for line in lines:
        if line.startswith("official_mirror_probe_record_start="):
            current = {"kind": line.split("=", 1)[1]}
            continue
        if current is None:
            continue
        if line.startswith("official_mirror_probe_record_end="):
            kind = line.split("=", 1)[1].strip()
            if kind and kind == current.get("kind") and {"probe", "hash", "plan", "reason"} <= current.keys():
                records.append(current)
            current = None
            continue
        for field, key in (
            ("official_mirror_probe=", "probe"),
            ("official_mirror_probe_apt_sources_sha256=", "hash"),
            ("official_mirror_probe_plan=", "plan"),
            ("official_mirror_probe_reason=", "reason"),
            ("official_mirror_probe_cause=", "cause"),
        ):
            if line.startswith(field):
                current.setdefault(key, line.split("=", 1)[1])
                break
    return records


def _parse_legacy_records(lines: Sequence[str]) -> list[dict[str, str]]:
    """Parse the pre-record-marker form: header, then probe/reason[/cause] adjacent."""
    records: list[dict[str, str]] = []
    preflight_hash: str | None = None
    for index, line in enumerate(lines):
        if line.startswith("apt_sources_sha256_before="):
            preflight_hash = line.split("=", 1)[1]
            continue
        if not line.startswith("official_mirror_probe=") or preflight_hash is None:
            continue
        reason_line = lines[index + 1] if index + 1 < len(lines) else ""
        if not reason_line.startswith("official_mirror_probe_reason="):
            continue
        cause_line = lines[index + 2] if index + 2 < len(lines) else ""
        records.append({
            "kind": "legacy",
            "probe": line.split("=", 1)[1],
            "hash": preflight_hash,
            "reason": reason_line.split("=", 1)[1],
            "cause": cause_line.split("=", 1)[1] if cause_line.startswith("official_mirror_probe_cause=") else "NETWORK_FAILURE",
        })
    return records


def recorded_probe_record(path: Path) -> tuple[bool, str, str, str | None] | None:
    """Return the last usable verdict and the source hash it is bound to.

    Once any bound record exists the legacy header form is ignored, so a
    header-less verdict can never inherit an unrelated source state.
    """
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return None
    records = _parse_bound_records(lines) or _parse_legacy_records(lines)
    if not records:
        return None
    record = records[-1]
    if record["probe"] not in ("PASS", "FAIL"):
        return None
    return record["probe"] == "PASS", record.get("reason", "no probe reason recorded"), record.get("cause", "NETWORK_FAILURE"), record["hash"]


def verdict_record_lines(kind: str, probe: str, source_hash: str, reason: str, cause: str, plan: str = "") -> list[str]:
    """Emit one immutable, self-contained verdict record."""
    return [
        f"official_mirror_probe_record_start={kind}",
        f"official_mirror_probe={'PASS' if probe == 'PASS' else 'FAIL'}",
        f"official_mirror_probe_apt_sources_sha256={source_hash}",
        f"official_mirror_probe_plan={plan}",
        f"official_mirror_probe_reason={reason}",
        f"official_mirror_probe_cause={cause}",
        f"official_mirror_probe_record_end={kind}",
    ]


def recorded_mirror_probe(path: Path) -> tuple[bool, str, str] | None:
    """Reuse a preflight result only when the current APT source hash matches."""
    record = recorded_probe_record(path)
    if record is None or record[3] != apt_source_snapshot():
        return None
    return record[0], record[1], record[2]


def reusable_mirror_probe(path: Path, current_hash: str, current_plan: str = "") -> tuple[bool, str, str] | None:
    """Reuse only a bound PASS record; anything else must be re-diagnosed before APT.

    A preflight FAIL can be a transient DNS/TLS failure that cleared minutes later, so
    replaying it as a terminal verdict would fail closed for the wrong reason. Legacy
    header-only evidence is never reusable either: it predates the redirect refusal
    and cannot prove which endpoint produced the verdict.
    """
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return None
    records = _parse_bound_records(lines)
    if not records:
        return None
    record = records[-1]
    if record["probe"] != "PASS" or record["hash"] != current_hash or record.get("plan", "") != current_plan:
        return None
    return True, record.get("reason", "no probe reason recorded"), record.get("cause", "NETWORK_FAILURE")


def recheck_official_mirrors(path: Path, runner: RUN = subprocess.run) -> tuple[bool, str, str]:
    """Re-diagnose the official mirrors with a bounded retry budget, fail-closed."""
    append(
        path,
        "\n".join((
            f"official_mirror_recheck_policy=attempts={MIRROR_PROBE_ATTEMPTS}; delay_seconds={MIRROR_PROBE_RETRY_DELAY_SECONDS}",
            "official_mirror_recheck_success_requires=every official InRelease endpoint reachable over TLS-verified HTTPS",
            "official_mirror_recheck_tls_validation=enabled",
        )),
    )
    reason = "official Ubuntu DNS or TLS-verified HTTPS endpoint unavailable"
    cause = "NETWORK_FAILURE"
    deadline = MONOTONIC() + MIRROR_PROBE_DEADLINE_SECONDS
    for attempt in range(1, MIRROR_PROBE_ATTEMPTS + 1):
        remaining = deadline - MONOTONIC()
        if remaining <= 0:
            append(
                path,
                "\n".join((
                    "official_mirror_recheck_deadline_exceeded=true",
                    f"official_mirror_recheck_deadline_seconds={MIRROR_PROBE_DEADLINE_SECONDS}",
                    "official_mirror_recheck_deadline_scope=no probe work is started after the deadline; an in-flight attempt may overrun by one endpoint timeout",
                    "official_mirror_recheck_exhausted=true",
                )),
            )
            return False, reason, cause
        source_hash_before_probe = apt_source_snapshot()
        if MONOTONIC() >= deadline:
            append(path, "official_mirror_recheck_deadline_exceeded=true\nofficial_mirror_recheck_exhausted=true")
            return False, reason, cause
        # Three sequential endpoints must together fit the remaining budget.
        https_timeout = max(1, min(HTTPS_TIMEOUT_SECONDS, int(remaining) // 3)) if remaining > 0 else 1
        try:
            ok, probe_release, probe_architecture, probe_sources, details = probe_official_mirrors(runner, https_timeout=https_timeout)
            probe_plan = plan_fingerprint(probe_release, probe_architecture, probe_sources)
            if ok:
                reason, cause = "all official InRelease endpoints verified", "HOST_READY"
            else:
                reason, cause = "official Ubuntu DNS or TLS-verified HTTPS endpoint unavailable", "NETWORK_FAILURE"
        except RuntimeError as error:
            ok = False
            probe_plan = ""
            details = [f"official_source_plan_unavailable={error}"]
            reason, cause = f"official source plan unavailable: {error}", "DEPENDENCY_FAILURE"
        except Exception as error:  # noqa: BLE001 - a probe crash must fail closed with evidence
            ok = False
            probe_plan = ""
            details = [f"official_mirror_recheck_unexpected_probe_error={type(error).__name__}: {error}"]
            reason = f"official mirror probe raised {type(error).__name__}"
            cause = "NETWORK_FAILURE"
        source_hash_after_probe = apt_source_snapshot()
        source_changed = source_hash_after_probe != source_hash_before_probe
        deadline_exceeded = ok and MONOTONIC() >= deadline
        if source_changed:
            ok = False
            reason = "system APT source configuration changed during the official mirror probe"
            cause = "DEPENDENCY_FAILURE"
        elif deadline_exceeded:
            ok = False
            reason = "official mirror recheck exceeded its wall-clock deadline"
            cause = "HOST_DEADLINE_EXCEEDED"
        classes = probe_failure_classes(details)
        append(
            path,
            "\n".join((
                *details,
                f"official_mirror_recheck attempt={attempt}/{MIRROR_PROBE_ATTEMPTS}",
                f"official_mirror_recheck_result={'PASS' if ok else 'FAIL'}",
                f"official_mirror_recheck_failure_classes={','.join(classes) if classes else 'none'}",
                f"official_mirror_recheck_source_changed_during_probe={'true' if source_changed else 'false'}",
                "official_mirror_recheck_deadline_enforcement=no new attempt starts after the deadline; one in-flight attempt may overrun by one endpoint timeout",
                f"official_mirror_recheck_https_timeout_seconds={https_timeout}",
                *verdict_record_lines("recheck", "PASS" if ok else "FAIL", source_hash_before_probe, reason, cause, probe_plan),
            )),
        )
        if source_changed:
            append(path, "official_mirror_recheck_exhausted=true")
            return False, reason, cause
        if deadline_exceeded:
            append(path, "official_mirror_recheck_deadline_exceeded=true\nofficial_mirror_recheck_exhausted=true")
            return False, reason, cause
        if ok:
            return True, reason, cause
        if cause == "DEPENDENCY_FAILURE":
            # Unsupported OS/architecture/keyring is deterministic; retrying cannot help.
            return False, reason, cause
        if attempt < MIRROR_PROBE_ATTEMPTS:
            wait_seconds = min(MIRROR_PROBE_RETRY_DELAY_SECONDS, max(0.0, deadline - MONOTONIC()))
            if wait_seconds <= 0:
                append(path, "official_mirror_recheck_deadline_exceeded=true\nofficial_mirror_recheck_exhausted=true")
                return False, reason, cause
            append(path, f"official_mirror_recheck_wait_seconds={wait_seconds}")
            SLEEP(wait_seconds)
    append(path, "official_mirror_recheck_exhausted=true")
    return False, reason, cause


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


def package_installed(runner: RUN = subprocess.run) -> bool:
    result = command(("dpkg-query", "-W", "-f=${db:Status-Status}\\n", "libpulse0"), timeout=5, runner=runner)
    return result.returncode == 0 and result.stdout.strip() == "installed"


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
    probe_source_hash = apt_source_snapshot()
    try:
        probe_source_plan = current_plan_fingerprint()
    except RuntimeError:
        probe_source_plan = ""
    try:
        mirrors_ok, release, architecture, sources, details = probe_official_mirrors(runner, dns_results)
        probe_source_plan = plan_fingerprint(release, architecture, sources)
        lines.extend((f"fallback_release={release}", f"fallback_architecture={architecture}"))
        lines.extend(details)
        lines.extend(verdict_record_lines(
            "preflight",
            "PASS" if mirrors_ok else "FAIL",
            probe_source_hash,
            "all official InRelease endpoints verified" if mirrors_ok else "official Ubuntu DNS or TLS-verified HTTPS endpoint unavailable",
            "HOST_READY" if mirrors_ok else "NETWORK_FAILURE",
            probe_source_plan,
        ))
    except RuntimeError as error:
        lines.append(f"fallback_plan=UNAVAILABLE ({error})")
        lines.extend(verdict_record_lines(
            "preflight",
            "FAIL",
            probe_source_hash,
            f"official source plan unavailable: {error}",
            "DEPENDENCY_FAILURE",
            probe_source_plan,
        ))
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


def recorded_probe_plan(path: Path) -> str | None:
    """Return the official endpoint plan a reusable verdict was produced for."""
    records = _parse_bound_records(path.read_text(encoding="utf-8", errors="replace").splitlines())
    if not records:
        return None
    return records[-1].get("plan") or None


def official_fallback(
    path: Path, api: str, runner: RUN = subprocess.run, verified_plan: str | None = None
) -> tuple[Path, list[str]]:
    release, architecture, plan = official_mirror_plan()
    resolved_plan = plan_fingerprint(release, architecture, plan[:3])
    if verified_plan is not None and resolved_plan != verified_plan:
        raise RuntimeError(
            f"official mirror plan changed before APT (verified {verified_plan}, resolved {resolved_plan})"
        )
    append(path, f"official_mirror_plan_at_apt={resolved_plan}")
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
    source_hash_before = apt_source_snapshot()
    try:
        reusable_plan = current_plan_fingerprint()
    except RuntimeError:
        # Without a plan the cached verdict cannot be bound to this host; force a recheck.
        reusable_plan = None
    mirror_probe = reusable_mirror_probe(path, source_hash_before, reusable_plan) if reusable_plan is not None else None
    if mirror_probe is not None:
        append(path, "official_mirror_probe_reuse=RECORDED_PASS (recorded preflight PASS; APT sources unchanged)")
    else:
        append(path, "official_mirror_probe_reuse=NONE (no reusable PASS; preflight FAIL or changed APT sources must be re-diagnosed)")
        mirror_probe = recheck_official_mirrors(path, runner)
    if not mirror_probe[0]:
        return record_failure(
            path,
            "HOST_INFRA",
            f"APT not attempted: {mirror_probe[1]}",
            cause=mirror_probe[2],
        )
    verdict = recorded_probe_record(path)
    verdict_plan = recorded_probe_plan(path)
    current_sources_hash = apt_source_snapshot()
    if verdict is None or not verdict[0] or verdict[3] != current_sources_hash:
        return record_failure(
            path,
            "HOST_INFRA",
            "official mirror verdict is not bound to the current APT source state",
            cause="NETWORK_FAILURE",
        )
    if verdict_plan is None:
        return record_failure(
            path,
            "HOST_INFRA",
            "official mirror verdict is not bound to an official endpoint plan",
            cause="NETWORK_FAILURE",
        )
    original_sources_hash = verdict[3]
    append(path, f"apt_source_policy=temporary official Ubuntu 24.04 source only; existing system sources unchanged\napt_sources_sha256_before_runtime={original_sources_hash}\napt_sources_sha256_at_handoff={current_sources_hash}")
    try:
        source_file, source_options = official_fallback(path, api, runner, verdict_plan)
    except RuntimeError as error:
        return record_failure(path, "DEPENDENCY_FAILURE", f"official Ubuntu source configuration unavailable: {error}")
    apt_policy_options = (*source_options, *APT_REDIRECT_OPTIONS)
    update = apt_invocation((*apt_policy_options, *APT_UPDATE_STRICT_OPTIONS, "update"), timeout=APT_TIMEOUT_SECONDS, runner=runner)
    append(path, f"official apt-get update exit={update.returncode}\n{update.stdout}")
    if update.returncode != 0:
        append(path, f"apt_sources_sha256_after={apt_source_snapshot()}\napt_sources_after=system sources unchanged; temporary source was {source_file}")
        cause = "NETWORK_FAILURE" if is_network_failure(update.stdout) or "command timed out after" in update.stdout.lower() else "DEPENDENCY_FAILURE"
        category = "HOST_INFRA" if cause == "NETWORK_FAILURE" else "DEPENDENCY_FAILURE"
        return record_failure(path, category, "official Ubuntu apt-get update failed", cause=cause)
    install = apt_invocation((*apt_policy_options, *runtime_install_args(package)), timeout=APT_TIMEOUT_SECONDS, runner=runner)
    append(path, f"official apt-get install exit={install.returncode}\n{install.stdout}")
    after, still_missing = library_report(sdk, runner)
    package_after = package_state(runner)
    package_after_installed = package_installed(runner)
    final_sources_hash = apt_source_snapshot()
    append(path, f"apt_sources_sha256_after={final_sources_hash}\napt_sources_after=system sources unchanged; temporary source was {source_file}\nlibpulse0_package_after={package_after}\nlibpulse0_status_after={'installed' if package_after_installed else 'not-installed'}\nldd_after:\n" + "\n".join(after))
    if final_sources_hash != original_sources_hash:
        return record_failure(path, "HOST_INFRA", "system APT source configuration changed during temporary package installation")
    if install.returncode == 124 and "command timed out after" in install.stdout.lower() and not still_missing and package_after_installed:
        append(path, "apt_install_timeout_recovered=yes; dpkg confirms libpulse0 installed and all QEMU dynamic libraries resolve\nresult=HOST_READY\napt_result=INSTALL_TIMED_OUT_BUT_RUNTIME_VERIFIED\napt_signature_validation=enabled; tls_validation=enabled; third_party_repositories=none")
        print("APT install timed out after the signed package transaction; dpkg and QEMU loader checks verify the runtime is ready.")
        return 0
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
