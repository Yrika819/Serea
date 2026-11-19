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

All three endpoints were reachable on a host running image `20261116.133.1`
with APT source hash `5f425af6…` (the same image and hash as failed identity
attempt 1), in the same run as the failing identity job. Failed attempt 2 ran
on image `20261111.127.1` with hash `2540850e…`, so the runner image and source
state are not identical between all of these hosts. The decisive difference is
that the failing hosts lost one endpoint per attempt, and a different endpoint
on each attempt, while the passing host verified all three.

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
   - `MIRROR_PROBE_DEADLINE_SECONDS = 90` is a hard wall-clock ceiling; the loop
     stops at the deadline and records
     `official_mirror_recheck_deadline_exceeded=true`
   - success requires **every** official `InRelease` endpoint to be reachable
     with TLS verification enabled; a different endpoint failing in each round
     never aggregates into a success
   - exhaustion records `official_mirror_recheck_exhausted=true` and returns
     `HOST_INFRA` without running APT
   - a deterministic `RuntimeError` (unsupported OS, architecture or missing
     Ubuntu keyring) is not retried and stays a dependency failure
   - any unexpected probe exception is caught, recorded as
     `official_mirror_recheck_unexpected_probe_error=` and fails closed
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
   `Acquire::Retries=1` are unchanged. The recheck adds no retries to APT and
   both the attempt budget and the wall-clock deadline are asserted by unit
   tests, so no time limit becomes unlimited.
5. Independent-review hardening (each with a regression test):
   - redirects are refused **before** they are followed (`NoRedirectHandler`,
     asserted on the installed handler), so no probe can contact another host,
     and a 3xx or a final URL that is not the requested official endpoint is
     `REDIRECT_REJECTED`;
   - both APT commands disable redirects for the generic keys **and** for each
     official host (`Acquire::http|
     https::<host>::AllowRedirect=false`), because APT resolves host-specific
     `Acquire` settings before the generic ones;
   - `apt-get update` runs with `APT::Update::Error-Mode=any` so a partial index
     fetch cannot continue to install;
   - every verdict is written as one immutable record
     (`official_mirror_probe_record_start=` .. `record_end=`) whose bound
     `official_mirror_probe_apt_sources_sha256=` is taken **before** its probe.
     Fields are first-wins, only a terminated record counts, and once any bound
     record exists the legacy header form is ignored, so an orphan or
     header-less verdict can never inherit another source state;
   - a source change detected across a recheck probe
     (`official_mirror_recheck_source_changed_during_probe=true`) fails closed;
   - a probe that only completes after the deadline cannot be recorded as
     `PASS`; each endpoint gets `remaining/3` as its own timeout so three
     sequential endpoints fit the remaining budget
     (`official_mirror_recheck_https_timeout_seconds=`), and the retry wait is
     clamped so it cannot sleep past the deadline;
   - malformed HTTP responses are classified instead of escaping the recheck.

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

`tools/test_android_ci_host.py`: 51 cases (the 19 pre-existing cases retained;
the only body changes are the pinned `APT_TIMEOUT_SECONDS == 45` becoming an
asserted 45..180 bound and a `SLEEP` injection in the pre-existing unreachable-
mirror test; 32 cases added):

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
| Retry budget bounded | `test_recheck_policy_is_bounded`, `test_recheck_stops_at_the_wall_clock_deadline`, `test_recheck_deadline_is_bounded` |
| Redirect / malformed response rejected | `test_redirect_away_from_the_official_endpoint_is_rejected`, `test_malformed_http_response_is_classified_not_escaped`, `test_unexpected_probe_error_fails_closed_with_evidence` |
| Every endpoint is probed for a PASS | `test_all_endpoints_must_be_probed_for_a_pass` |
| Rotating endpoint failure never aggregates | `test_rotating_endpoint_failure_never_aggregates_into_success` |
| Partial APT index fetch cannot continue | `test_apt_update_uses_strict_error_mode`, `test_official_apt_update_uses_strict_error_mode` |
| Verdict is bound to its own source hash | `test_recheck_verdict_carries_its_own_source_hash`, `test_source_change_after_cache_validation_is_detected`, `test_source_change_after_cache_validation_is_detected`, `test_recheck_source_change_during_probe_fails_closed`, `test_verdict_block_is_bound_to_one_complete_record`, `test_incomplete_verdict_block_is_not_reused` |
| Redirect refused before following | `test_redirect_is_refused_before_it_is_followed`, `test_redirect_handler_refuses_instead_of_following`, `test_redirect_away_from_the_official_endpoint_is_rejected`, `test_apt_refuses_redirects_for_every_official_host` |
| Only a terminated bound record is reusable | `test_only_a_complete_bound_record_is_reusable`, `test_orphan_hash_cannot_rebind_an_accepted_record`, `test_bound_records_disable_the_legacy_fallback`, `test_preflight_writes_a_self_contained_bound_record` |
| Deadline applies to in-flight PASS | `test_pass_completed_after_the_deadline_is_rejected`, `test_in_flight_probe_budget_is_capped_by_the_deadline` |
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

## 8. Verification on this PR's SHA

Draft PR: #246 (`codex/android-host-mirror-recheck`), base `main`
`077246aeab909928fb57316f1f3aefc6be69c0b3`.

The results below are recorded for head `e01a49263f990b9c3eff267218f8c87c8ea4ec77`
(tree `5803533256e1b01ddce6d7bb86db9269819821b6`), the first head that carried
the mirror-recheck fix. Later heads change only the probe hardening described in
section 5 and 9, and must be re-verified on their own SHA before merge.

Required checks (branch protection):

| Check | Result |
| --- | --- |
| Linux fast checks | pass |
| Linux stable full validation | pass |
| Android Emulator Required Gate | pass |
| Linux MSRV 1.89.0 | pass |

The Required Gate re-verified every dependency on the same SHA
(`BUILD_RESULT`, `HOST_PROBE_RESULT`, `UNIT_RESULT`, `INTEGRATION_RESULT`,
`SECURITY_RESULT`, `EMULATOR_RESULT`, `BETA_IDENTITY_RESULT`, `AUDIT_RESULT`,
`BUNDLE_RESULT` all `success`, `INSTRUMENTATION_RESULT: PASS` with 103/103 plus
2/2 on both API levels).

`Personal Beta Identity Isolation (temporary signing keys)` now passes
(13m35s). It executed the disposable identity checks and produced
temporary-key evidence:

```
application_id=app.serea.android.beta
demo_and_beta_parallel_install=PASS
demo_package_data_and_signer_unchanged=PASS
beta_encrypted_room_data_after_emulator_reboot=PASS
same_test_key_version_100_to_101_update=PASS
different_test_key_version_102_update=INSTALL_FAILED_UPDATE_INCOMPATIBLE
owner_signing_key_used=no
demo_uninstall_attempted=no
beta_native_library_exclusion_versionCode=99..102 result=PASS
```

Its host evidence recorded `official_mirror_probe=PASS`,
`official apt-get update exit=0`, `official apt-get install exit=0`,
`libpulse0_status_after=installed`, `result=HOST_READY` and
`apt_signature_validation=enabled; tls_validation=enabled;
third_party_repositories=none`.

`Android Formal API 26` (20m12s) and `Android Formal API 35` (25m31s) pass on the
same SHA.

### 8.1 Non-Required failures on this SHA

- `Build target, instrumentation, and Test IME APKs once`
  (`android-api35-real-ime-independent-bc.yml`, explicitly "Diagnostic,
  Non-Required") fails its own scope guard with "Verify source scope is limited to
  this diagnostic harness". Its `paths:` trigger lists
  `tools/android_ci_host.py` and `tools/test_android_ci_host.py`, but its
  allowlist omits them, so any change to the helper trips the guard. The same job
  also failed on the commit that last changed the helper (`b7fc506`, PR #231).
  This is a pre-existing inconsistency in that diagnostic workflow and is not
  modified here.
- `API 35 focused Fake providers, local Chat and Tasks E2E` failed on an
  external Maven Central artefact fetch
  (`Could not download annotations-12.0.jar ... Repository MavenRepo is disabled
  due to earlier error`) after its host preparation reached `HOST_READY`. The
  same `:app:javaPreCompileDebug` KSP classpath fetch failure also hit
  `Android Unit` and `Android Security Tests` on first attempt; both passed on a
  same-SHA re-run with no code change, confirming an external infrastructure
  flake rather than a regression.

## 9. Independent review

Two independent review rounds were run. Both reported **Critical 0 / High 0**
against a separate reviewer, not GitHub approval.

Round 1 (head `e01a4926`, tree `5803533256b0`): 5 Medium, 3 Low. Medium findings
and their first fixes:

| Finding | Resolution |
| --- | --- |
| Followed redirect could yield a plaintext/third-party `PASS` | redirect or downgrade is rejected (`REDIRECT_REJECTED`) |
| `apt-get update` could return 0 on partial index failure | `APT::Update::Error-Mode=any` |
| Source change between cache validation and runtime baseline escaped detection | single snapshot is both the reuse check and the runtime baseline; verdicts carry their own hash |
| Attempt cap without a wall-clock ceiling | `MIRROR_PROBE_DEADLINE_SECONDS = 90` |
| Malformed HTTP response escaped the retry and the evidence | classified and recorded, still fails closed |

Round 2 (head `eacf4a7b`, tree `eb835557ae`): 3 Medium, 4 Low. The round-1 fixes
were judged incomplete in three respects, each fixed with a regression test:

| Round-2 finding | Resolution |
| --- | --- |
| A probe that completed after the deadline could still be recorded `PASS` | `PASS` is rejected when the clock has passed the deadline, the in-flight probe timeout is clamped to the remaining budget, and `official_mirror_recheck_deadline_exceeded=true` is recorded |
| A cached `PASS` could inherit a different block's source hash, and the recheck read the hash after probing | only a completed verdict block is parsed, the bound hash is taken before the probe, and a change across the probe fails closed |
| `urlopen` still followed the redirect before the final-URL check | `NoRedirectHandler` refuses the redirect first, and both APT commands disable cross-host redirects |

Round 3 (head `2d7c381f`, tree `f252d89ea3`): 3 Medium, 0 Low. Round-2 fixes were
judged bypassable in three respects, each fixed with a regression test:

| Round-3 finding | Resolution |
| --- | --- |
| A mutable block could be rebound by a later orphan hash, and a header-less verdict could inherit another preflight hash | verdicts are immutable `record_start`/`record_end` records with first-wins fields; the legacy header form is ignored once any bound record exists; preflight now writes its own bound record |
| The clamp was computed once and the retry wait could exceed the deadline | per-endpoint timeout is `remaining/3`, the wait is clamped to the deadline, and a late `PASS` is rejected |
| APT host-specific `Acquire::*::<host>::AllowRedirect` can outrank the generic key | redirect refusal is passed for each official host as well as generically |

Low findings after round 3: the ordinary install-success branch reports on the
loader result while dpkg state is recorded separately (accepted: loader
resolution is the gating property); the non-Required Real IME diagnostic rejects
this change through its pre-existing scope guard (documented in 8.1); one
added test was vacuous as an invocation check (strengthened to assert the
forwarded arguments); and report wording about run comparability and test
counts (corrected in 2.3, 7 and 8).

## 10. Residual risk

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

## 11. Next steps

1. Integrate this fix through its own Draft PR with Owner approval.
2. Re-run the Required gates on PR #245 against the new `main`.
3. Confirm `Personal Beta Identity Isolation (temporary signing keys)` actually
   executes the temporary-signing-key checks and emits
   `personal-beta-identity.txt` before treating the gate as satisfied.
