# H4-F1A — Private Beta Manifest & Signing Readiness

**State:** `F1A_READY_FOR_OWNER_REVIEW` (historical F1A documentation and manifest-label correction; no signing, APK, device, Cloud, OAuth, or Calendar operation). Based on clean worktree branch `docs/h4-f1a-private-beta-manifest-signing-readiness`, from `origin/main` HEAD `6557af533d83e4a1f643fcb308e5356087422410`, Tree `63e76bea7dfa53342a095c0789f8c9a5e614a0ee`. This report was first committed as `c4b9ee59af32c1fa845b3767957d2aff98bb7261` before PR creation. At the original report snapshot, PR #250 was Draft; it was later merged on 2026-11-21: https://github.com/Yrika819/Serea/pull/250. The check statuses below remain tied to their exact historical HEADs and are not live status. The later Owner decision `APPROVAL A — APPROVED (2026-11-21)` and current key/Approval B boundaries are recorded in the [H4-F1D decision record](../plans/2026-11-21-h4-f1d-option-a-owner-decision-and-approval-b-readiness.md); this does not authorize B–E.

## Executive result

- The earlier `26.0.2.1` report was insufficient by itself to identify a cause. Environment inspection found default Oracle JDK `26.0.2.1`, Gradle Wrapper `8.14.6`, AGP `8.12.3`, existing Android SDK and cached dependencies. Gradle's published compatibility table limits Gradle 8.14.6 runtime support to Java 24. This makes an unsupported JDK/runtime the likely environmental explanation; the exact mechanism behind the earlier error text was not captured, so it cannot be proven from that text alone. The same manifest tasks later succeeded using JDK 17, with no dependency-fetch failure observed.
- Re-running with Temurin JDK `17.0.21`, the existing SDK, existing caches and two Gradle workers succeeded twice for Private Beta manifest processing and once for Public Beta. No CI environment variable or OAuth opt-in was set. No APK task was run.
- Inspection exposed a real label mismatch: the Private Beta source had no label override and merged `Serea Demo`. The minimal, authorized correction is in `android/app/src/privateBeta/AndroidManifest.xml`; after the fix, Private Beta and Public Beta both merge `Serea Personal Beta`. The source correction does not change either application ID or build gates.
- Source and merged-manifest evidence supports keeping Private Beta ID `app.serea.android.privatebeta`. Current source defaults (`versionCode=2`, `versionName=0.1.0-demo`) are not an Owner-approved artifact version.
- The fixed Owner-managed Private Beta-only signing identity (Option A) was formally adopted under Approval A on 2026-11-21; its key parameters, generation and custody setup remain subject to Approval B. No key was generated, searched for, read, copied, or used in F1A or by this later decision.
- OAuth and Calendar HTTP remain separate and default-off; HTTP remains hardcoded false in build configuration. Cloud/project/client/API state is unconfirmed. APK signing, provenance, fingerprints, install/update compatibility and device data retention remain `NOT_VERIFIED`.

## Scope and evidence

Reviewed repository instructions and the F0 report/runbook, gate matrix, Private Calendar activation checklist, H4-E2 implementation record, and 2026-11-20 Personal-mode decision. Audited `android/app/build.gradle.kts`, source and generated Private/Public manifests, merger reports, build environment and relevant isolation conditions. Product source was not changed.

Environment evidence from the isolated worktree:

| Item | Observation | Assessment |
|---|---|---|
| Default Java | Oracle JDK `26.0.2.1` | Unsupported for Gradle 8.14.6 runtime |
| Approved validation Java | Temurin JDK `17.0.21` | Used for validation |
| Gradle / AGP | Wrapper `8.14.6` / AGP `8.12.3` | Existing pinned toolchain; no upgrade made |
| Android SDK | Existing installed SDK available by explicit local SDK path | Reused; no bulk installation |
| `ANDROID_HOME` / `ANDROID_SDK_ROOT` | Unset in original shell; local SDK present | Explicit SDK path supplied for validation |
| Gradle cache / network | Required wrapper and plugin/dependency artifacts were available locally; tasks completed | No dependency fetch failure reproduced |
| Working directory | Wrapper is `android/gradlew`, run from `android/` | Root-directory invocation would be invalid |
| `CI`, `GITHUB_ACTIONS` | Both unset for approved local run | CI packaging guard was not bypassed; local verification only |
| OAuth opt-in | `SEREA_OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED` unset | No OAuth opt-in |

### Manifest task evidence

Command, from `android/` with JDK 17 and the existing SDK:

```sh
./gradlew :app:processPrivateBetaManifest --no-daemon --max-workers=2
```

It completed twice, below the 600-second per-run limit (55 seconds before the label correction; 1 minute 29 seconds after). The Public Beta comparison task `:app:processBetaManifest --no-daemon --max-workers=2` completed once (1 minute 25 seconds). No retry beyond the authorized limit was needed. A pre-fix assertion failed specifically because the Private merged label was `Serea Demo`; the post-fix assertion passed. These are manifest-processing results only, not APK/build/signing results.

Generated local evidence (not committed as build output):

- Private merged manifest: `android/app/build/intermediates/merged_manifest/privateBeta/processPrivateBetaMainManifest/AndroidManifest.xml`
- Private merger report: `android/app/build/outputs/logs/manifest-merger-privateBeta-report.txt`
- Public merged manifest: `android/app/build/intermediates/merged_manifest/beta/processBetaMainManifest/AndroidManifest.xml`
- Public merger report: `android/app/build/outputs/logs/manifest-merger-beta-report.txt`

The Private report attributes its application-label override to `src/privateBeta/AndroidManifest.xml` after the correction. F0's historical statement that this override was already in the Private source was wrong; F0 was corrected to distinguish that source-level error from F0's failed merge attempt and to reference this F1A evidence.

### Merged manifest comparison

| Field | Private Beta | Public Beta | Result |
|---|---|---|---|
| Package / application ID in merged XML | `app.serea.android.privatebeta` | `app.serea.android.beta` | Distinct IDs preserved |
| Merged source versionCode | `2` | `1` | Manifest task evidence only; not artifact metadata or Owner approval |
| Merged source versionName | `0.1.0-demo` | `0.1.0-demo` | Manifest task evidence only |
| App label | `Serea Personal Beta` | `Serea Personal Beta` | Private mismatch corrected; Public unchanged |
| INTERNET | Present | Present | Expected |
| Backup / cleartext | `allowBackup=false`; `usesCleartextTraffic=false` | Same | Secure source controls present |
| Product activities | Main launcher exported; Tasks non-exported; Private account picker and consent relay non-exported | Main launcher exported; Tasks non-exported; no Private auth activities | Expected variant separation |
| Additional permissions | INTERNET plus app-ID-scoped AndroidX dynamic receiver signature permission | Same, with Public app ID | No Gmail, Calendar, contacts, storage, or other unexpected permission found |
| Dependency components | Google Sign-In hub/revocation, Google API activity, AndroidX Room service, Startup provider, Profile Installer receiver protected by `android.permission.DUMP` | Same dependency components | Expected dependency manifest contributions; not unexplained product components |
| Debug/Isolated harness | No debug harness receiver; no isolated manifest route | No debug harness receiver | No Debug/Isolated-only harness component observed |

**Limit:** merged manifests cannot prove a built APK's final application ID/version, signer, signature validity, provenance, installability or update compatibility. They also do not constitute a device-level component/data-isolation test.

## Private Beta identity and build isolation

Source review of `android/app/build.gradle.kts` confirmed:

1. `namespace` and base application ID remain `app.serea.android`; Debug suffix is `.demo`, Isolated suffix `.isolated`, Public Beta suffix `.beta`, and Private Beta suffix `.privatebeta`. Private version code defaults to `2` and permits a Gradle override; version name inherits `0.1.0-demo`.
2. Public Beta identity and signing behavior are unchanged. Public Beta explicitly clears signing configuration. Private Beta inherits Beta then explicitly clears signing configuration; neither has a formal signing key configured.
3. Base OAuth BuildConfig values are false. Public Beta OAuth/UI flags are false. Private OAuth enablement requires the explicit Private-only `SEREA_OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED=true` opt-in; it was not set. Private auth UI may be compiled while real OAuth remains off.
4. `PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED` is false in base, Beta and Private Beta configuration. No HTTP activation was performed.
5. In CI (`CI=true` or `GITHUB_ACTIONS=true`), Private Beta variants are disabled except for the narrow `testPrivateBetaUnitTest` test-only invocation; other requested Private Beta tasks are rejected. No packaging guard or CI setting was modified.
6. Debug/Isolated paths retain separate application ID suffixes and test-only source sets. The F1A change adds no HTTP client injection, test URL, OAuth behavior, signing configuration, or product code.
7. Different application IDs provide Android package-level data separation for installed apps; this source/manifest audit did not install either variant or inspect device data.

**Versioning hold:** Do not use source default `2` as the real APK version without Owner approval. A release version must exceed any prior installed Private Beta version while retaining the exact package and compatible signer. No APK values are inferred from source.

## Signing design and Owner decisions

- Option A, a fixed Owner-managed Private Beta-only certificate, was formally adopted by Approval A on 2026-11-21; specific parameters and key generation remain pending Approval B. Full Owner-only preparation is in [the H4-F1A signing and APK verification runbook](../plans/h4-f1a-owner-private-signing-and-apk-verification-runbook.md) and the dated [H4-F1D decision/readiness record](../plans/2026-11-21-h4-f1d-option-a-owner-decision-and-approval-b-readiness.md).

Before any key/artifact action, the Owner must complete Approval B's final key-parameter, host, custody, backup and recovery decisions, and separately decide the approved versionCode and exact later artifact/device actions. Approval A (adopting Option A) is already recorded; it does not authorize key generation, artifact access/build/signing, device operations or Cloud activity. Do not find or inspect a prior key or APK without separate, tightly scoped authorization.

## Google Cloud / OAuth handoff prerequisites

No Google Cloud action is requested or performed in F1A. Before an Owner-approved next stage can create/configure a Google Android OAuth client, all these must be established: Owner-selected signing method; exact approved and provenance-verified signed Private APK; package `app.serea.android.privatebeta`; successful `apksigner` verification; single expected signer (or separately justified signer set); signer SHA-1 from that exact APK; independent SHA-256 comparison; Owner-confirmed Google Auth Platform Personal audience; Android client type; Calendar API only; exact `calendar.events.readonly` scope; and `primary`-only access design. OAuth client registration is `BLOCKED_BY_SIGNING_IDENTITY` until the APK signer SHA-1 is verified.

Personal is limited to the creator's one Google account and is not a multi-user design. Gmail, Drive, Calendar write/list, billing/quota changes, and AI-powered-agent client configuration are excluded. Any combined onboarding operation needs explicit approval for both project/audience setup and Android client creation. Cloud state stays **UNCONFIRMED** until the Owner reports observed results. No credentials, identities, project/client IDs, fingerprints, APK hashes, tokens, or calendar data belong in public PR content.

## Findings and residual risks

- **Critical:** none identified in this bounded manifest/build-configuration review.
- **High:** none identified in this bounded review. PR #250 Required CI and Android check evidence is reported separately in the HEAD-stamped snapshot below; these checks do not constitute a full product security certification.
- **Medium:** no approved signed APK/signer identity, version decision, provenance, signer multiplicity, installation state, or update compatibility. Owner decisions and explicit artifact authorization are prerequisites for a future signing operation; verified signer evidence is a hard block for OAuth registration and any real connection.
- **Medium:** actual Personal audience, OAuth client and Calendar API/quota/billing configuration remain unconfirmed and Owner-controlled.
- **Low:** F0 contained an inaccurate source attribution for the Private label; corrected in this branch. A source label fix is verified in generated manifest, but no installed UI was inspected.
- **Separate existing issue (not changed in F1A):** Public Beta also currently displays `Serea Personal Beta`. Owner requested this be tracked separately; F1A did not modify the Public Beta label or manifest.

### H4-E2 to F1B remaining work

- Obtain Owner decisions for signing identity, custody/recovery, approved APK/version and narrowly scoped artifact/device actions.
- Under explicit Owner authorization, produce and independently verify the exact signed APK; retain private evidence and compare to any existing approved Private artifact before asserting update compatibility.
- Keep Cloud preparation blocked until verified signer SHA-1 exists; then provide an explicit `OWNER_ACTION_REQUIRED — GOOGLE_CLOUD` handoff with current official UI guidance and wait for Owner results. This report does not request Cloud operation now.
- Continue to require distinct approval for Cloud project/audience and client creation, API enablement, billing/quota, code-gate changes, authentication/token acquisition, Calendar HTTP and device use. F1A does not approve or perform any of them.
- Do not enable `PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED`; it remains false. H4-E2's design/CI evidence is historical evidence only, not fresh F1A CI. Real OAuth, Calendar access, account lifecycle, data-egress and install/update behavior remain untested.

## Validation performed and not performed

| Validation | Result |
|---|---|
| JDK / Gradle / SDK / environment diagnosis | Completed; default JDK 26 is outside Gradle 8.14.6's supported range and is the likely environment explanation; exact mechanism of the initial error text was not captured; no dependency-fetch failure seen |
| Private Beta manifest task, JDK 17, ≤2 workers | Completed twice, pre/post label correction |
| Public Beta manifest comparison, JDK 17, ≤2 workers | Completed once |
| Merged XML and merger reports | Inspected; source package/version/label/components compared |
| Build variant and gate source audit | Completed read-only |
| Gradle unit tests / Android Formal / APK packaging or signing / CI | F1A PR CI ran on initial PR HEAD `c4b9ee59af32c1fa845b3767957d2aff98bb7261`; details and current rerun status are recorded in the time-stamped check snapshot below. No APK packaging/signing was performed |
| Signed APK metadata/certificate/provenance/update compatibility | `NOT_VERIFIED` |
| Device/data isolation | `NOT_RUN` |
| Google Cloud / OAuth / token / Calendar HTTP | `NOT_AUTHORIZED`, `NOT_RUN` |
| Key creation/access, signing, install/distribution | `NOT_AUTHORIZED`, `NOT_RUN` |
| Independent review / PR / Required CI | Independent read-only review and corrective follow-up completed; PR #250 created and remains Draft. See the time-stamped check snapshot below for Required CI, API 26/35 Formal, Identity Isolation, and the HTTP 429 retry state |

F0/H4-E2 historical CI results are not F1A checks. The following is a **time-bounded GitHub snapshot checked 2026-11-21**, not a claim about any later HEAD: PR #250 initial HEAD `c4b9ee59af32c1fa845b3767957d2aff98bb7261` / Tree `830b56ad4d89d5fbe6eec2bf873f148dc875ee3b`; Android workflow run `44863062553`; focused-test run `44863062417` attempt 2. Fetched `origin/main` remained `6557af533d83e4a1f643fcb308e5356087422410` / Tree `63e76bea7dfa53342a095c0789f8c9a5e614a0ee`.

| Same-HEAD check (PR #250 initial HEAD) | State at this snapshot |
|---|---|
| Required CI: Full CI Linux MSRV, Fast CI Linux fast checks, Full CI Linux stable full validation | All 3 `SUCCESS` |
| Android Formal API 26 | `IN_PROGRESS` |
| Android Formal API 35 | `IN_PROGRESS` |
| Personal Beta Identity Isolation | `SUCCESS` on this HEAD; this is the workflow check result only, not agent key handling |
| Non-Required Focused provider/session tests | Initial attempt `DELIVERED_REJECTED` due Maven Central HTTP 429 while resolving dependencies before test execution; test JUnit evidence was not produced. The failed job rerun was started as run attempt 2 on the same source SHA and was `IN_PROGRESS` when this snapshot was recorded |
| Other Android checks | Android Build/Lint/Instrumentation Compile, Android Unit, Android Integration, Android Security, Android Documentation, Dependency Audit, and Host Probe reported `SUCCESS` |

The HTTP 429 failure is an external dependency-delivery/`HOST_INFRA` failure, not evidence of a product regression or a test assertion failure. Because the failed command did not reach the tests, do not count it as a test pass or product test failure. The failed evidence upload was consequential to no test-result files existing after dependency resolution aborted. Recheck live PR checks before relying on this snapshot. Do not claim build, Formal, CI, or signing PASS from manifest processing alone.

## F1A decision

**`F1A_READY_FOR_OWNER_REVIEW`** for the bounded manifest/source and Owner-preparation scope. Independent review is complete; PR #250 remains Draft. The initial PR HEAD has 3/3 Required CI checks successful, while API 26/35 Formal, Identity Isolation, and the retry of the Non-Required HTTP 429 job were still pending at the snapshot above. Recheck these on the latest PR HEAD before treating the verification gate as closed. **Not** approval for artifact signing, Google Cloud, real OAuth or live Calendar access. H4-F1B is **NO-GO for signing until the Owner selects the signing method and separately authorizes the exact artifact operation**; OAuth/client registration remains blocked until the resulting approved APK and signer evidence are verified. Keep PR #250 Draft and stop for Owner review.
