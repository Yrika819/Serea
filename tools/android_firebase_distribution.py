#!/usr/bin/env python3
"""Fail-closed Personal Beta signing checks and Firebase App Distribution adapter."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any


EXPECTED_APPLICATION_ID = "app.serea.android.beta"
EXPECTED_REPOSITORY = "Yrika819/Serea"
EXPECTED_WORKFLOW_PATH = ".github/workflows/android-personal-beta.yml"
FINGERPRINT_RE = re.compile(r"(?i)^[0-9a-f]{64}$")
COMMIT_SHA_RE = re.compile(r"(?i)^[0-9a-f]{40}$")
FIREBASE_APP_ID_RE = re.compile(r"^[0-9]+:[0-9]+:android:[0-9a-fA-F]+$")
GROUP_ALIAS_RE = re.compile(r"^[a-z][a-z0-9-]{2,62}$")
FIREBASE_PROJECT_ID_RE = re.compile(r"^[a-z][a-z0-9-]{4,28}[a-z0-9]$")


class PipelineError(RuntimeError):
    pass


def _normalize_sha256(value: str) -> str:
    normalized = re.sub(r"[:\s]", "", value).upper()
    if not FINGERPRINT_RE.fullmatch(normalized):
        raise PipelineError("Expected a 64-character SHA-256 value.")
    return normalized


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def validate_source(
    repository: str,
    ref: str,
    workflow_ref: str,
    source_sha: str,
    checkout_sha: str,
) -> None:
    expected_workflow_ref = f"{EXPECTED_REPOSITORY}/{EXPECTED_WORKFLOW_PATH}@refs/heads/main"
    if repository != EXPECTED_REPOSITORY:
        raise PipelineError("Personal Beta workflow must run in the canonical Serea repository.")
    if ref != "refs/heads/main" or workflow_ref != expected_workflow_ref:
        raise PipelineError("Personal Beta signing is restricted to the protected main workflow source.")
    if not COMMIT_SHA_RE.fullmatch(source_sha) or source_sha.lower() != checkout_sha.lower():
        raise PipelineError("Checked-out source SHA does not match the exact workflow dispatch SHA.")


def validate_main_tip(source_sha: str, main_sha: str) -> None:
    if not COMMIT_SHA_RE.fullmatch(source_sha) or not COMMIT_SHA_RE.fullmatch(main_sha):
        raise PipelineError("Protected main tip or workflow source SHA is missing or malformed.")
    if source_sha.lower() != main_sha.lower():
        raise PipelineError("Workflow source is no longer the current protected-main tip.")


def validate_environment_protection(environment: Any, expected_name: str) -> None:
    if not isinstance(environment, dict) or environment.get("name") != expected_name:
        raise PipelineError("Personal Beta GitHub Environment is missing or has the wrong name.")
    protection_rules = environment.get("protection_rules", [])
    if not isinstance(protection_rules, list):
        raise PipelineError("Personal Beta Environment protection rules are invalid.")
    reviewers = [
        reviewer
        for rule in protection_rules
        if isinstance(rule, dict) and rule.get("type") == "required_reviewers"
        and isinstance(rule.get("reviewers"), list)
        for reviewer in rule["reviewers"]
        if isinstance(reviewer, dict)
        and reviewer.get("type") in {"User", "Team"}
        and isinstance(reviewer.get("reviewer"), dict)
        and isinstance(reviewer["reviewer"].get("id"), int)
        and not isinstance(reviewer["reviewer"].get("id"), bool)
        and reviewer["reviewer"]["id"] > 0
    ]
    if not reviewers:
        raise PipelineError("Personal Beta Environment has no required reviewers; Owner approval is mandatory.")
    branch_policy = environment.get("deployment_branch_policy")
    if not isinstance(branch_policy, dict) or branch_policy.get("protected_branches") is not True:
        raise PipelineError("Personal Beta Environment must restrict deployments to protected branches.")
    if branch_policy.get("custom_branch_policies") is not False:
        raise PipelineError("Personal Beta Environment must use the protected-branches restriction.")


def validate_main_branch_protection(protection: Any, required_names: list[str]) -> None:
    if not isinstance(protection, dict) or not protection:
        raise PipelineError("GitHub main branch protection is missing or invalid.")
    if (
        not isinstance(required_names, list)
        or not required_names
        or any(not isinstance(name, str) or not name.strip() for name in required_names)
        or len(set(required_names)) != len(required_names)
    ):
        raise PipelineError("Owner-configured required checks are missing or invalid.")

    status_checks = protection.get("required_status_checks")
    if not isinstance(status_checks, dict):
        raise PipelineError("Protected main must require status checks.")
    contexts = status_checks.get("contexts", []) or []
    checks = status_checks.get("checks", []) or []
    if not isinstance(contexts, list) or not isinstance(checks, list):
        raise PipelineError("Protected main required-check configuration is invalid.")
    configured_names = {
        name
        for name in contexts
        if isinstance(name, str) and name.strip()
    }
    configured_names.update(
        check.get("context")
        for check in checks
        if isinstance(check, dict) and isinstance(check.get("context"), str)
    )
    missing = sorted(set(required_names) - configured_names)
    if missing:
        raise PipelineError("Protected main does not require every Owner-configured check.")

    reviews = protection.get("required_pull_request_reviews")
    approval_count = reviews.get("required_approving_review_count") if isinstance(reviews, dict) else None
    if not isinstance(approval_count, int) or isinstance(approval_count, bool) or approval_count < 1:
        raise PipelineError("Protected main must require at least one pull request approval.")
    enforce_admins = protection.get("enforce_admins")
    if not isinstance(enforce_admins, dict) or enforce_admins.get("enabled") is not True:
        raise PipelineError("Protected main must enforce branch rules for administrators.")


def version_code_for_run(run_number: str, run_attempt: str, baseline: str) -> int:
    if not run_number.isdigit() or not run_attempt.isdigit() or not baseline.isdigit():
        raise PipelineError("GitHub workflow numbers and Owner versionCode baseline must be numeric.")
    if int(run_attempt) != 1:
        raise PipelineError(
            "Workflow reruns reuse a versionCode and are blocked; dispatch a new run to advance it."
        )
    if int(baseline) < 1:
        raise PipelineError("Owner Personal Beta versionCode baseline must be at least 1.")
    version_code = int(baseline) + int(run_number)
    if version_code <= int(baseline) or version_code > 2_147_483_647:
        raise PipelineError("Generated Personal Beta versionCode is outside Android limits.")
    return version_code


def verify_firebase_app(apps_payload: Any, app_id: str, package_name: str) -> None:
    if app_id != app_id.strip() or not FIREBASE_APP_ID_RE.fullmatch(app_id):
        raise PipelineError("Firebase Android App ID is missing or invalid.")
    if package_name != EXPECTED_APPLICATION_ID:
        raise PipelineError("Firebase application package must be the Personal Beta ID.")
    if isinstance(apps_payload, dict):
        apps_payload = apps_payload.get("result", apps_payload.get("apps"))
    if not isinstance(apps_payload, list):
        raise PipelineError("Firebase Android app inventory is invalid.")
    matching = [entry for entry in apps_payload if isinstance(entry, dict) and entry.get("appId") == app_id]
    if len(matching) != 1 or matching[0].get("packageName") != EXPECTED_APPLICATION_ID:
        raise PipelineError("Configured Firebase app ID is not bound to app.serea.android.beta.")


def required_check_failures(
    required_names: list[str], check_runs: list[dict[str, Any]], source_sha: str
) -> list[str]:
    if not COMMIT_SHA_RE.fullmatch(source_sha):
        raise PipelineError("Exact source SHA is missing or malformed.")
    if not required_names or any(not isinstance(name, str) or not name.strip() for name in required_names):
        raise PipelineError("Owner-configured required check names are missing or invalid.")
    if len(set(required_names)) != len(required_names):
        raise PipelineError("Owner-configured required check names contain duplicates.")

    failures: list[str] = []
    for name in required_names:
        matching = [
            check
            for check in check_runs
            if check.get("name") == name
            and check.get("head_sha", "").lower() == source_sha.lower()
            and isinstance(check.get("app"), dict)
            and check["app"].get("slug") == "github-actions"
        ]
        if not matching:
            failures.append(f"{name}: missing on source SHA")
            continue
        latest = max(
            matching,
            key=lambda check: (str(check.get("started_at", "")), int(check.get("id", 0) or 0)),
        )
        if latest.get("status") != "completed" or latest.get("conclusion") != "success":
            failures.append(f"{name}: latest result is not success")
    return failures


def verify_apk(
    apk: Path,
    application_id: str,
    version_code: int,
    expected_certificate_sha256: str,
    aapt: str,
    apksigner: str,
) -> dict[str, str]:
    if application_id != EXPECTED_APPLICATION_ID:
        raise PipelineError("The configured Application ID does not match the Personal Beta contract.")
    if version_code <= 1 or version_code > 2_147_483_647:
        raise PipelineError("Personal Beta versionCode is outside Android limits.")
    if not apk.is_file() or apk.stat().st_size == 0:
        raise PipelineError("The expected Personal Beta APK is missing or empty.")

    try:
        badging = subprocess.run(
            [aapt, "dump", "badging", str(apk)],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError) as error:
        raise PipelineError("APK package metadata could not be read.") from error

    package_match = re.search(
        r"^package: name='([^']+)' versionCode='([0-9]+)'(?:\s|$)", badging, re.MULTILINE
    )
    if not package_match:
        raise PipelineError("APK package metadata is missing or malformed.")
    if package_match.group(1) != application_id:
        raise PipelineError("APK Application ID does not match the Personal Beta ID.")
    if int(package_match.group(2)) != version_code:
        raise PipelineError("APK versionCode does not match the requested Beta version.")

    try:
        signature = subprocess.run(
            [apksigner, "verify", "--verbose", "--print-certs", str(apk)],
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError) as error:
        raise PipelineError("APK signature verification failed.") from error

    certificate_digests = re.findall(
        r"Signer #\d+ certificate SHA-256 digest:\s*([0-9a-fA-F:]+)", signature
    )
    if len(certificate_digests) != 1:
        raise PipelineError("APK must have exactly one verifiable signing certificate.")
    actual_certificate = _normalize_sha256(certificate_digests[0])
    try:
        expected_certificate = _normalize_sha256(expected_certificate_sha256)
    except PipelineError as error:
        raise PipelineError("Owner signing certificate pin is missing or invalid.") from error
    if actual_certificate != expected_certificate:
        raise PipelineError("APK signing certificate does not match the Owner pin.")

    return {
        "application_id": application_id,
        "version_code": str(version_code),
        "apk_sha256": _sha256_file(apk),
        "certificate_sha256": actual_certificate,
    }


def _write_result(path: Path | None, result: str) -> None:
    if path is not None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(result + "\n", encoding="utf-8")


def distribute_apk(
    apk: Path,
    expected_sha256: str,
    firebase_cli: str,
    app_id: str,
    group_alias: str,
    distribution_enabled: str,
    release_notes_file: Path,
    result_file: Path | None = None,
) -> str:
    try:
        if distribution_enabled != "true":
            raise PipelineError("Owner has not enabled Firebase distribution for this Environment.")
        if not FIREBASE_APP_ID_RE.fullmatch(app_id):
            raise PipelineError("Firebase Android App ID is missing or invalid.")
        if not GROUP_ALIAS_RE.fullmatch(group_alias):
            raise PipelineError("Approved Firebase tester group alias is missing or invalid.")
        expected = _normalize_sha256(expected_sha256).lower()
        if not apk.is_file() or _sha256_file(apk) != expected:
            _write_result(result_file, "FAILED_CHECKSUM_MISMATCH")
            raise PipelineError("APK checksum changed after verification; distribution was blocked.")
        if not release_notes_file.is_file():
            raise PipelineError("Firebase release notes are unavailable.")

        command = [
            firebase_cli,
            "appdistribution:distribute",
            str(apk),
            "--app",
            app_id,
            "--groups",
            group_alias,
            "--release-notes-file",
            str(release_notes_file),
        ]
        try:
            result = subprocess.run(command, capture_output=True, text=True, check=False)
        except OSError as error:
            _write_result(result_file, "FAILED")
            raise PipelineError("Firebase CLI could not be started.") from error
        if result.returncode != 0:
            private_output = f"{result.stdout}\n{result.stderr}"
            if re.search(
                r"\b(?:401|403)\b|PERMISSION_DENIED|UNAUTHENTICATED|unauthenticated|permission denied|not authorized",
                private_output,
                re.IGNORECASE,
            ):
                _write_result(result_file, "FAILED_AUTHORIZATION")
                raise PipelineError("Firebase authorization was denied; raw CLI output was suppressed.")
            _write_result(result_file, "FAILED")
            raise PipelineError("Firebase distribution failed; raw CLI output was suppressed.")

        _write_result(result_file, "SUCCESS")
        return "SUCCESS"
    except PipelineError:
        if result_file is not None and not result_file.exists():
            _write_result(result_file, "FAILED_PREFLIGHT")
        raise


def _load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise PipelineError("A required JSON input is missing or invalid.") from error


def _write_github_outputs(values: dict[str, str]) -> None:
    output = os.environ.get("GITHUB_OUTPUT")
    if not output:
        return
    with Path(output).open("a", encoding="utf-8") as stream:
        for name, value in values.items():
            if "\n" in value or "\r" in value:
                raise PipelineError("A workflow output contains an invalid line break.")
            stream.write(f"{name}={value}\n")


def run_preflight(distribute_requested: bool) -> None:
    required = (
        "SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_BASE64",
        "SEREA_ANDROID_PERSONAL_BETA_KEYSTORE_PASSWORD",
        "SEREA_ANDROID_PERSONAL_BETA_KEY_ALIAS",
        "SEREA_ANDROID_PERSONAL_BETA_KEY_PASSWORD",
        "SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256",
        "SEREA_PERSONAL_BETA_VERSION_CODE_BASELINE",
        "SEREA_REQUIRED_CHECKS_JSON",
    )
    missing = [name for name in required if not os.environ.get(name, "").strip()]
    if missing:
        raise PipelineError("Required protected Environment configuration is missing.")
    try:
        _normalize_sha256(os.environ["SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256"])
        baseline = os.environ["SEREA_PERSONAL_BETA_VERSION_CODE_BASELINE"]
        if not baseline.isdigit() or int(baseline) < 1 or int(baseline) > 2_147_483_646:
            raise PipelineError("Owner Personal Beta versionCode baseline is invalid.")
        required_checks = json.loads(os.environ["SEREA_REQUIRED_CHECKS_JSON"])
    except (PipelineError, json.JSONDecodeError) as error:
        raise PipelineError("Required protected Environment configuration is invalid.") from error
    if not isinstance(required_checks, list):
        raise PipelineError("Required check configuration must be a JSON array.")
    if not required_checks or any(not isinstance(name, str) or not name.strip() for name in required_checks):
        raise PipelineError("Required check configuration must name at least one check.")
    if len(set(required_checks)) != len(required_checks):
        raise PipelineError("Required check configuration contains duplicate names.")

    if distribute_requested:
        distribution_values = (
            "SEREA_FIREBASE_DISTRIBUTION_ENABLED",
            "SEREA_GCP_WIF_PROVIDER",
            "SEREA_GCP_SERVICE_ACCOUNT",
            "SEREA_FIREBASE_PROJECT_ID",
            "SEREA_FIREBASE_APP_ID",
            "SEREA_FIREBASE_APPROVED_TESTER_GROUP",
        )
        if any(not os.environ.get(name, "").strip() for name in distribution_values):
            raise PipelineError("Firebase distribution is not configured in the protected Environment.")
        provider = os.environ["SEREA_GCP_WIF_PROVIDER"]
        if not re.fullmatch(
            r"projects/[0-9]+/locations/global/workloadIdentityPools/[A-Za-z0-9_-]+/providers/[A-Za-z0-9_-]+",
            provider,
        ):
            raise PipelineError("Google Workload Identity Federation provider is invalid.")
        if not re.fullmatch(
            r"[a-z][a-z0-9-]*@[a-z0-9.-]+\.iam\.gserviceaccount\.com",
            os.environ["SEREA_GCP_SERVICE_ACCOUNT"],
        ):
            raise PipelineError("Google Workload Identity Federation service account is invalid.")
        if not FIREBASE_PROJECT_ID_RE.fullmatch(os.environ["SEREA_FIREBASE_PROJECT_ID"]):
            raise PipelineError("Firebase project ID is missing or invalid.")
        if os.environ["SEREA_FIREBASE_DISTRIBUTION_ENABLED"] != "true":
            raise PipelineError("Owner has not enabled Firebase distribution for this Environment.")
        if not FIREBASE_APP_ID_RE.fullmatch(os.environ["SEREA_FIREBASE_APP_ID"]):
            raise PipelineError("Firebase Android App ID is missing or invalid.")
        if not GROUP_ALIAS_RE.fullmatch(os.environ["SEREA_FIREBASE_APPROVED_TESTER_GROUP"]):
            raise PipelineError("Approved Firebase tester group alias is missing or invalid.")


def write_report(args: argparse.Namespace) -> None:
    requested = args.distribute_requested == "true"
    result_file = Path(args.result_file) if args.result_file else None
    result = "NOT_RUN"
    if requested:
        if result_file and result_file.is_file():
            result = result_file.read_text(encoding="utf-8").strip() or "FAILED"
        elif args.auth_outcome == "failure":
            result = "FAILED_AUTHORIZATION"
        elif args.firebase_app_outcome == "failure":
            result = "FAILED_FIREBASE_APP_BINDING"
        elif args.apk_outcome != "success":
            result = "FAILED_PREPARATION"
        else:
            result = "FAILED_ATTESTATION_OR_PRECHECK"

    def value(env_name: str, fallback: str = "NOT_AVAILABLE") -> str:
        return os.environ.get(env_name, "").strip() or fallback

    configured_firebase_app = os.environ.get("SEREA_FIREBASE_APP_ID", "").strip()
    if not configured_firebase_app:
        configured_firebase_app = "NOT_CONFIGURED"
    elif not FIREBASE_APP_ID_RE.fullmatch(configured_firebase_app):
        configured_firebase_app = "INVALID_CONFIGURED_VALUE"
    apk_outcome = args.apk_outcome or "not_run"
    apk_result = "PASS" if apk_outcome == "success" else "NOT_RUN" if apk_outcome == "skipped" else "FAILURE"

    fields = [
        ("Source SHA", value("SOURCE_SHA")),
        ("Tree SHA", value("SOURCE_TREE_SHA")),
        ("Workflow ID", value("WORKFLOW_ID")),
        ("Workflow path", value("WORKFLOW_PATH")),
        ("Run ID", value("GITHUB_RUN_ID")),
        ("Run attempt", value("GITHUB_RUN_ATTEMPT")),
        ("Application ID", value("VERIFIED_APPLICATION_ID")),
        ("Version Code", value("VERIFIED_VERSION_CODE")),
        ("APK SHA-256", value("VERIFIED_APK_SHA256")),
        ("Signing Certificate SHA-256", value("VERIFIED_CERTIFICATE_SHA256")),
        ("APK verification result", apk_result),
        ("Firebase package binding result", args.firebase_app_outcome or "not_run"),
        ("Firebase App ID", configured_firebase_app),
        ("Distribution result", result),
    ]
    lines = ["## Personal Beta Firebase App Distribution provenance", ""]
    lines.extend(f"- **{label}:** `{field_value}`" for label, field_value in fields)
    lines.append("")
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with Path(summary).open("a", encoding="utf-8") as stream:
            stream.write("\n".join(lines))
    else:
        sys.stdout.write("\n".join(lines))


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    preflight = subparsers.add_parser("preflight")
    preflight.add_argument("--distribute-requested", choices=("true", "false"), required=True)

    source = subparsers.add_parser("validate-source")
    source.add_argument("--repository", required=True)
    source.add_argument("--ref", required=True)
    source.add_argument("--workflow-ref", required=True)
    source.add_argument("--source-sha", required=True)
    source.add_argument("--checkout-sha", required=True)

    main_tip = subparsers.add_parser("validate-main-tip")
    main_tip.add_argument("--source-sha", required=True)
    main_tip.add_argument("--main-sha", required=True)

    version = subparsers.add_parser("version-code")
    version.add_argument("--run-number", required=True)
    version.add_argument("--run-attempt", required=True)

    environment = subparsers.add_parser("verify-environment")
    environment.add_argument("--environment-file", type=Path, required=True)
    environment.add_argument("--name", required=True)

    main_protection = subparsers.add_parser("verify-main-protection")
    main_protection.add_argument("--protection-file", type=Path, required=True)

    checks = subparsers.add_parser("check-required")
    checks.add_argument("--check-runs", type=Path, required=True)

    verify = subparsers.add_parser("verify")
    verify.add_argument("--apk", type=Path, required=True)
    verify.add_argument("--application-id", required=True)
    verify.add_argument("--version-code", type=int, required=True)
    verify.add_argument("--aapt", required=True)
    verify.add_argument("--apksigner", required=True)

    firebase_app = subparsers.add_parser("verify-firebase-app")
    firebase_app.add_argument("--apps-file", type=Path, required=True)
    firebase_app.add_argument("--app-id", required=True)
    firebase_app.add_argument("--package-name", required=True)

    deploy = subparsers.add_parser("distribute")
    deploy.add_argument("--apk", type=Path, required=True)
    deploy.add_argument("--expected-sha256", required=True)
    deploy.add_argument("--release-notes-file", type=Path, required=True)
    deploy.add_argument("--result-file", type=Path, required=True)

    report = subparsers.add_parser("report")
    report.add_argument("--distribute-requested", choices=("true", "false"), required=True)
    report.add_argument("--result-file")
    report.add_argument("--apk-outcome", default="")
    report.add_argument("--auth-outcome", default="")
    report.add_argument("--firebase-app-outcome", default="")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    try:
        if args.command == "preflight":
            run_preflight(args.distribute_requested == "true")
        elif args.command == "validate-source":
            validate_source(args.repository, args.ref, args.workflow_ref, args.source_sha, args.checkout_sha)
        elif args.command == "validate-main-tip":
            validate_main_tip(args.source_sha, args.main_sha)
        elif args.command == "version-code":
            version_code = version_code_for_run(
                args.run_number,
                args.run_attempt,
                os.environ["SEREA_PERSONAL_BETA_VERSION_CODE_BASELINE"],
            )
            _write_github_outputs({"version_code": str(version_code)})
            if not os.environ.get("GITHUB_OUTPUT"):
                print(f"version_code={version_code}")
        elif args.command == "verify-environment":
            validate_environment_protection(_load_json(args.environment_file), args.name)
        elif args.command == "verify-main-protection":
            required_names = json.loads(os.environ.get("SEREA_REQUIRED_CHECKS_JSON", "null"))
            validate_main_branch_protection(_load_json(args.protection_file), required_names)
        elif args.command == "check-required":
            required_names = json.loads(os.environ.get("SEREA_REQUIRED_CHECKS_JSON", "null"))
            check_runs = _load_json(args.check_runs)
            if isinstance(check_runs, dict):
                check_runs = check_runs.get("check_runs")
            if not isinstance(required_names, list) or not isinstance(check_runs, list):
                raise PipelineError("Required check configuration or GitHub check response is invalid.")
            if any(not isinstance(check, dict) for check in check_runs):
                raise PipelineError("GitHub check response contains an invalid check record.")
            failures = required_check_failures(required_names, check_runs, os.environ["SOURCE_SHA"])
            if failures:
                raise PipelineError("One or more required checks are missing or not successful on the exact source SHA.")
        elif args.command == "verify":
            result = verify_apk(
                args.apk,
                args.application_id,
                args.version_code,
                os.environ.get("SEREA_ANDROID_PERSONAL_BETA_CERT_SHA256", ""),
                args.aapt,
                args.apksigner,
            )
            _write_github_outputs(result)
        elif args.command == "verify-firebase-app":
            verify_firebase_app(_load_json(args.apps_file), args.app_id, args.package_name)
            _write_github_outputs({"firebase_app_id": args.app_id, "package_name": args.package_name})
        elif args.command == "distribute":
            distribute_apk(
                args.apk,
                args.expected_sha256,
                os.environ.get("FIREBASE_CLI", "firebase"),
                os.environ.get("SEREA_FIREBASE_APP_ID", ""),
                os.environ.get("SEREA_FIREBASE_APPROVED_TESTER_GROUP", ""),
                os.environ.get("SEREA_FIREBASE_DISTRIBUTION_ENABLED", ""),
                args.release_notes_file,
                args.result_file,
            )
        elif args.command == "report":
            write_report(args)
    except (PipelineError, KeyError, TypeError, json.JSONDecodeError) as error:
        print(f"Personal Beta blocked: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
