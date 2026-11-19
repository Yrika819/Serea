# Android CI HOST_INFRA Recovery — Official Mirror Recheck Report

- Date: 2026-11-19
- Scope: `tools/android_ci_host.py` host helper and its unit tests only
- Blocked: PR #245 (`codex/h4-d1-d2-private-google-calendar`, HEAD `1e8a9c6f6abaf9881cfab9f4631b3a47022429d3`)
- Canonical main at investigation start: `077246aeab909928fb57316f1f3aefc6be69c0b3`
- Failed run: run `44486655006`, attempt 2, job `135460984060`
  (`Personal Beta Identity Isolation (temporary signing keys)`)
- No product code, Calendar implementation, signing infrastructure, OAuth
  configuration or runtime application behavior was changed.

## 1. Observed failure

```
Personal Beta Identity Isolation (temporary signing keys)
  APT Runtime Preparation Identity Isolation
  HOST_INFRA: APT not attempted: official Ubuntu DNS or TLS-verified HTTPS endpoint unavailable
```

Everything else on the same SHA passed: Formal API 26/35, Unit, Integration,
Security, Build/Lint, Host Probe, Dependency Audit, Storage and Emulator.
`Android Emulator Required Gate` failed only as a downstream consequence.

## 2. Evidence

### 2.1 Failed identity job (attempt 2, artifact `14513789860`)

```
runner_image_version=20261111.127.1
apt_sources_sha256_before=2540850e39517800e16060ccc69b5d6da263b6bf90fb9e631542276072703fae
official_dns archive.ubuntu.com: 91.189.91.81 ...
official_dns security.ubuntu.com: 91.189.90.21 ...
official_https .../noble/InRelease: REACHABLE (HTTP 206; TLS verification enabled)
official_https .../noble-updates/InRelease: REACHABLE (HTTP 206; TLS verification enabled)
official_https https://security.ubuntu.com/ubuntu/dists/noble-security/InRelease:
  UNREACHABLE (UNREACHABLE (<urlopen error _ssl.c:983: The handshake operation timed out>))
official_mirror_probe=FAIL
official_mirror_probe_cause=NETWORK_FAILURE
libpulse0_package=NOT_INSTALLED (exit=1)
unresolved_shared_libraries=libpulse.so.0
```

DNS resolved for every official host. Two of three `InRelease` endpoints were
reachable with TLS verification enabled. Exactly one endpoint failed with a
**TLS handshake timeout**.

### 2.2 Attempt 1 of the same job (artifact `14511556232`)

Same runner image, same APT source hash, a *different* single endpoint timed
out (`archive.ubuntu.com/dists/noble/InRelease`), while the other two passed.
The failing endpoint differs between attempts, so the condition is transient
per-connection, not a persistent host or source defect.

### 2.3 Successful jobs in the same run

`Android Formal API 35` (artifact `serea-android-host-api35-attempt-1`) on the
**same runner image** (`20261116.133.1`) and the **same APT source hash**
(`5f425af66ed01fe4ca7ff1ab629ad7ecebc2dd424e9533b5cb58066a8aea8cdf`) reported:

```
official_https .../noble/InRelease: REACHABLE (HTTP 206; TLS verification enabled)
official_https .../noble-updates/InRelease: REACHABLE (HTTP 206; TLS verification enabled)
official_https .../noble-security/InRelease: REACHABLE (HTTP 206; TLS verification enabled)
official_mirror_probe=PASS
```

The only difference between the passing and failing hosts is a single transient
endpoint timeout.

### 2.4 Timing proof of the defective path

| Step | Start | End | Elapsed |
| --- | --- | --- | --- |
| `Host Preflight Identity Isolation` | 09:20:03.43 | 09:20:09.23 | 5.8 s |
| `APT Runtime Preparation Identity Isolation` | 09:20:09.25 | 09:20:09.32 | **0.07 s** |

`install_runtime` returned `HOST_INFRA` in 70 ms. A real DNS + TLS probe of
three official endpoints cannot complete that fast, so the helper performed no
re-diagnosis: it replayed the preflight `FAIL` verbatim.

### 2.5 Two further transient modes in the same helper

Validating the fix on the current runner fleet exposed two independent
latencies in the same host-preparation path. Both are recorded in artifacts
from run `44500277631`.

**(a) Per-endpoint TLS handshake latency above the 4 s cap.**
`serea-android-host-probe-attempt-1` shows `noble-updates` timing out at the
4 s cap in the preflight and in recheck attempts 1 and 2, then verifying in
attempt 3, after which the job reached `HOST_READY`. Across that run 5 of 12
endpoint observations hit the 4 s cap while other observations of the same
endpoints succeeded.

**(b) `apt-get update` above the 45 s command cap.**
`serea-android-host-api26-attempt-1` recorded a passing preflight probe, then:

```
official apt-get update exit=124
Get:1 ... Get:21 https://security.ubuntu.com/ubuntu noble-security/universe amd64 c-n-f Metadata [24.2 kB]
COMMAND TIMED OUT after 45s
```

The update was progressing normally (21 indexes, ~28 MB of official signed
indexes, no error) when the 45 s wall-clock cap aborted it. The same steps took
39 s and 52 s in run `44486655006`, so the budget was already marginal and the
degraded host throughput pushed it over.

Neither mode is a persistent outage and neither is a dependency failure.

## 3. Root cause

In `tools/android_ci_host.py`:

- `preflight()` records `official_mirror_probe=FAIL` as an observational
  diagnostic (preflight exit code is 0 by design).
- `recorded_mirror_probe(path)` returns the recorded status — both `PASS` **and**
  `FAIL` — whenever the current APT source SHA-256 equals the recorded one.
- `install_runtime()` therefore treated a transient preflight failure as a
  terminal verdict:

```python
mirror_probe = recorded_mirror_probe(path)
if mirror_probe is None:
    ...  # only re-probe when no record exists
if not mirror_probe[0]:
    return record_failure(path, "HOST_INFRA", ...)   # 70 ms after preflight
```

Re-running the failed job reproduces the same outcome because the sticky
verdict, not the network, drives the result.

Classification: `Host Infrastructure / transient network` — **not** a dependency
failure and **not** an application or test failure.

## 4. Reproduction before the fix

Replaying the real attempt-2 evidence file through the helper at
`origin/main`:

```
install_runtime exit=1  probe_attempts=0
HOST_INFRA: APT not attempted: official Ubuntu DNS or TLS-verified HTTPS endpoint unavailable
```

Zero probe attempts on a host that had just been observed with two of three
endpoints reachable.

## 5. Fix

Minimal, fail-closed changes in `tools/android_ci_host.py` only.

1. `reusable_mirror_probe()` reuses a recorded result **only when it is `PASS`**.
   A recorded `FAIL`, or a changed APT source hash, forces re-diagnosis.
   `recorded_mirror_probe()` keeps its existing parsing contract.
2. `recheck_official_mirrors()` re-diagnoses the official mirrors with a bounded
   budget before any APT command:
   - `MIRROR_PROBE_ATTEMPTS = 3`, `MIRROR_PROBE_RETRY_DELAY_SECONDS = 5`
   - worst added wall clock is bounded (`<= 120 s`, asserted by a unit test)
   - success requires **every** official `InRelease` endpoint to be reachable
     with TLS verification enabled
   - exhaustion records `official_mirror_recheck_exhausted=true` and returns
     `HOST_INFRA` without running APT
   - a deterministic `RuntimeError` (unsupported OS, architecture or missing
     Ubuntu keyring) is not retried and stays a dependency failure
3. Machine-readable diagnostics (`official_https_failure_classes=`) distinguish
   `DNS_FAILURE`, `TLS_VERIFICATION_FAILURE`, `HTTP_ERROR`, `TIMEOUT` and
   `CONNECTION_FAILURE`. Each recheck attempt appends
   `official_mirror_recheck attempt=<n>/<max>`, its result and its failure
   classes to the same evidence file, so the deciding observation is auditable.
4. Two bounded latency caps are re-based on the evidence in section 2.5, each
   still hard-capped and asserted by unit tests:
   - `HTTPS_TIMEOUT_SECONDS`: 4 s -> 10 s per official endpoint.
   - `APT_TIMEOUT_SECONDS`: 45 s -> 180 s for the official `apt-get` invocation.
   `Acquire::http::Timeout=12`, `Acquire::https::Timeout=12` and
   `Acquire::Retries=1` are unchanged, and the recheck total stays bounded
   (`<= 300 s` worst case, asserted). No time limit becomes unlimited.

### 5.1 Unchanged safety properties

- Official Ubuntu 24.04 noble sources only (`archive.ubuntu.com`,
  `security.ubuntu.com`, `ports.ubuntu.com`).
- `signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg` retained; APT
  signature verification retained; TLS certificate verification retained.
- The temporary source is written outside `/etc/apt` and the system source hash
  is compared before and after installation; any change is `HOST_INFRA`.
- No third-party mirror, no OS-wide APT source edit, no unauthenticated install.
- `libpulse0` and the QEMU `ldd` state are re-verified after APT; unresolved
  libraries are never reported as success.
- No `continue-on-error`, no Required Gate change, no artifact fabrication.

## 6. Reproduction after the fix

Same real attempt-2 evidence, one transient `TIMEOUT` then recovery:

```
install_runtime exit=0  probe_attempts=2  slept=[5]
official_mirror_probe_reuse=NONE (no reusable PASS; preflight FAIL or changed APT sources must be re-diagnosed)
official_mirror_recheck attempt=1/3
official_mirror_recheck_result=FAIL
official_mirror_recheck_failure_classes=TIMEOUT
official_mirror_recheck_wait_seconds=5
official_mirror_recheck attempt=2/3
official_mirror_recheck_result=PASS
official_mirror_probe=PASS
apt_sources_sha256_before_runtime=2540850e...703fae
apt_sources_sha256_after=2540850e...703fae
libpulse0_status_after=installed
ldd_after: libpulse.so.0 => /lib/x86_64-linux-gnu/libpulse.so.0
result=HOST_READY
apt_result=INSTALLED_SIGNED_OFFICIAL_UBUNTU
```

## 7. Regression tests

`tools/test_android_ci_host.py` (19 existing cases retained unmodified in
substance; 12 cases added):

| Requirement | Test |
| --- | --- |
| Preflight FAIL, network recovers, recheck PASS | `test_recorded_preflight_failure_is_re_diagnosed_and_recovers` |
| Preflight FAIL, recheck also FAIL, `HOST_INFRA` | `test_exhausted_recheck_stops_as_host_infra_without_apt` |
| Recorded PASS still reused without network | `test_successful_recorded_probe_is_still_reused_without_network_access` |
| Only same APT sources may reuse a PASS | `test_reused_probe_requires_unchanged_apt_sources` |
| Partial official endpoint reachability fails | `test_partial_official_endpoint_reachability_fails_closed` |
| All endpoints reachable is the only success | `test_all_official_endpoints_reachable_is_the_only_success` |
| Per-endpoint and APT caps stay bounded | `test_apt_execution_time_limits_remain_bounded`, `test_apt_policy_is_bounded`, `test_recheck_policy_is_bounded` |
| DNS failure distinguished | `test_dns_failure_is_classified_without_attempting_https` |
| TLS verification failure fails closed | `test_tls_verification_failure_is_classified_as_failure` |
| Invalid OS/source plan fails without retry | `test_official_source_plan_unavailable_fails_closed_without_retry` |
| Official source mutation detected | `test_system_apt_source_mutation_during_install_is_host_infra` |
| Retry budget bounded | `test_recheck_policy_is_bounded` |
| Legacy detail lists tolerated | `test_probe_failure_classes_tolerates_legacy_detail_lists` |

Pre-existing coverage retained: resolved QEMU libraries skip APT
(`test_loadable_emulator_runtime_skips_apt_even_if_package_database_lacks_libpulse`),
unresolved `libpulse0` re-verified after APT
(`test_official_package_source_is_used_only_after_dns_and_https_probe_pass`),
APT failure never treated as success
(`test_official_apt_update_non_network_failure_is_dependency_failure`,
`test_apt_timeout_after_successful_probe_is_host_network_failure`,
`test_apt_install_timeout_is_accepted_only_when_package_and_loader_are_verified`).

The only edit to a pre-existing test adds `SLEEP` injection to
`test_unreachable_official_mirror_fails_before_apt`; all of its assertions
(exit 1, APT not invoked, `result=HOST_INFRA`, `failure_cause=NETWORK_FAILURE`)
are unchanged.

## 8. Residual risk

- The bounded budget (3 attempts, 5 s apart) cannot defeat a genuinely
  persistent outage; such a host still fails closed as `HOST_INFRA` with a
  classified cause. Round semantics stay all-or-nothing: a partial success in
  one round never aggregates into overall success
  (`test_exhausted_recheck_stops_as_host_infra_without_apt`).
- A slow but working endpoint can still consume an attempt, and `apt-get` can
  still exceed 180 s on a severely throttled host. Both fail closed with a
  classified cause rather than a false success.
- Preflight evidence can record `FAIL` while the job proceeds after a recheck
  `PASS`. Both lines are retained in the same artifact with explicit
  `official_mirror_probe_reuse=` / `official_mirror_recheck` markers, so the
  decision sequence is unambiguous under review.
- This change does not address why individual GitHub-hosted runners exhibit
  per-connection TLS handshake latency or throttled egress; that remains
  external. The fix only prevents such transients from being recorded as a
  terminal verdict or being cut off by a marginal cap.

## 9. Next steps

1. Integrate this fix through its own Draft PR with Owner approval.
2. Re-run the Required gates on PR #245 against the new `main`.
3. Confirm `Personal Beta Identity Isolation (temporary signing keys)` actually
   executes the temporary-signing-key checks and emits
   `personal-beta-identity.txt` before treating the gate as satisfied.
