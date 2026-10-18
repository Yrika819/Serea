# Serea Android Phase 2 implementation report

Date: 2026-10-18 · State: Draft implementation evidence

## Scope and status

This report covers AND-04/05/06 on the stacked Android branch. `IMPLEMENTED`,
`TESTED_LOCAL`, `CI_PASS`, `DEVICE_TESTED`, `OWNER_PENDING`, `BLOCKED`, and
`NOT_RUN` are reported separately in the final PR description and run record.
The branch does not change Rust Core, the P7/P8 research series, formal
migrations, or frozen protocols.

## Candidate behavior

- Demo mode stores and answers ordinary chat locally. It makes no HTTP calls.
- Cloud Candidate is a separate settings action that sends a fixed public
  contract probe to Ollama's pinned HTTPS API host. Ordinary chat text and
  history remain ineligible for cloud submission pending the Owner's
  classification decision.
- The provider supports bounded NDJSON streaming, usage counts, cancellation,
  timeout/error mapping, disabled redirects, model allowlisting, and local
  response limits. It never retries automatically.
- Android Keystore AES-GCM protects the credential and encrypted history title
  and body. Cloud credential save/read requires actual StrongBox/TEE hardware
  backing; software-backed Android Keystore is refused for that path pending
  Owner approval. Room stores Android-only metadata and ciphertext. Backup is
  disabled and no destructive migration fallback is used.
- UI supports Demo conversations, history navigation/deletion, masked key
  configuration, provider probe, and pending/failure/cancellation states.
  Android's `FLAG_SECURE` is applied while settings expose the credential field
  and is removed when settings close. This blocks ordinary screenshots and
  recent-app thumbnails in that window; it cannot prevent accessibility
  capture, a compromised OS, camera capture, or transient keyboard/memory
  exposure.

## Important limits

No live provider request or real API key was used. Provider model availability,
current terms, account conditions, exact retention, regional processing, and
revocation must be rechecked by the Owner before external data use. Local
soft-limits do not guarantee a provider bill ceiling. The current candidate
does not send ordinary user messages to a model, so production multi-turn
cloud chat, context truncation, and provider usage budgets are not enabled.
Room schema currently exports version 1 only; migration tests for a future
version do not yet exist. Software-backed Android Keystore is reported
accurately but acceptance policy is unresolved. Physical secure erasure is not
claimed.

## Validation evidence

Local verification on 2026-10-18 used JDK 17, Gradle Wrapper 8.11.1, SDK 35.
The combined Gradle command passed `assembleDebug`, `assembleRelease`,
`testDebugUnitTest`, `assembleDebugAndroidTest`, `lintDebug`, and the license
report task before the final CI submission. The JVM suites passed twice
locally, including a forced rerun of all tasks. Each run had 22 tests:
ViewModel state/cancellation 5/5, Mock HTTP/provider 12/12, and egress
security 5/5. Latest local JUnit XML had zero failures, errors, or skips.

The repository defines 6 Room/Keystore/manifest instrumentation tests and 3
Compose tests (9 total). They compiled into the Android Test APK, but were
`NOT_RUN` locally because there is no emulator binary/system image or attached
device. GitHub Actions also found that the runner does not expose readable
`/dev/kvm`; software CPU emulation exited before boot, so the Android Storage
and Emulator job explicitly reports the instrumentation step `NOT_RUN`. Its
Job Summary includes the emulator log tail. A first attempt had an unbounded
`adb wait-for-device` and timed out after 15 minutes; the workflow now checks
the process and boot state with bounded waits, tries software mode when KVM is
absent, and reports unavailable emulation without marking instrumentation as
passed. Pixel 7a remains `NOT_RUN` until a physical device is attached and
exercised. Activity recreation and DB close/reopen are covered by test code,
but actual process-death recovery is not verified.

Artifacts: `android/app/build/outputs/apk/debug/app-debug.apk` (installable
debug build), `android/app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk`,
and `android/app/build/outputs/apk/release/app-release-unsigned.apk` (release
variant compile output; unsigned). Room schema version 1 is exported. Docs
validation passed. APK sizes were 11 MiB debug, 992 KiB instrumentation, and
6.9 MiB unsigned release. Room has only schema version 1, so there is no prior
schema migration path to exercise; migration tests and destructive-upgrade
testing remain future work. Branch coverage was not measured; there is no
numeric coverage claim.

### Test result ledger

Results below distinguish defined cases from executed cases. Storage tests
are included in the Android Instrumentation row, and the Emulator row refers
to the same nine instrumentation cases.

| Area | Defined | PASS | FAIL | NOT_RUN |
| --- | ---: | ---: | ---: | ---: |
| Unit / ViewModel | 5 | 5 | 0 | 0 |
| Mock HTTP integration | 12 | 12 | 0 | 0 |
| Egress security | 5 | 5 | 0 | 0 |
| Room / Keystore storage | 6 | 0 | 0 | 6 |
| Android Instrumentation | 9 | 0 | 0 | 9 |
| Android Emulator | 9 | 0 | 0 | 9 |

The first three rows are disjoint suites, but all execute under the JVM unit
test task. Storage, instrumentation, and emulator rows overlap and must not
be summed with one another. Numeric branch coverage is unavailable. There is
no dedicated Android migration, process-death, or backup-restore test result.

### GitHub Actions evidence on implementation commit `7a394e0`

All workflows below targeted `7a394e0e3c45a4a8da46610cf8032ba263a6f55c` and
completed successfully. Android's emulator instrumentation step is explicitly
`NOT_RUN` for the runner limitation above; the Instrumentation APK compile
step passed.

| Workflow | Run ID | Result |
| --- | ---: | --- |
| Android | 39134709112 | PASS; instrumentation execution NOT_RUN |
| Fast CI | 39134709139 | PASS |
| Git hooks Windows compatibility | 39134709117 | PASS |
| Full CI | 39134709167 | PASS |
| Cross-architecture SQLite portability | 39134709119 | PASS |
| Native platform validation | 39134709164 | PASS (12/12 matrix jobs) |

Android's seven jobs passed: Build/Lint/Instrumentation Compile, Unit,
Integration, Security Tests, Storage and Emulator, Documentation, and
Dependency Audit. The Storage and Emulator job passed its environment check
and recorded tests as NOT_RUN; it is not evidence of a Room, Keystore, or UI
instrumentation pass. No workflow used a provider secret or live API request.

The first GitHub dependency scan found vulnerable Android Test Platform
transitives in the lockfile: protobuf 3.24.4 (CVE-2024-7254, CVSS 8.7) and
Netty 4.1.93.Final. OSV returned 43 vulnerable package/advisory pairs, including
protobuf's two artifact entries and affected Netty 4.1 modules.
Gradle now resolves protobuf to 3.25.9 and Netty to 4.1.139.Final for these
tooling configurations. A locally run, checksum-verified OSV Scanner v2.3.3
scan of the updated lockfile returned zero findings. The dependency license
report covers release, unit-test, and Android-test runtime configurations,
with 134 license records and no unknown/unlicensed entries in the generated
report. Android Dependency Audit passed on the implementation commit above.

Coverage targets were not verified. The dependency audit reports no remaining
Critical/High finding. Android Keystore behavior, backup behavior, Room
operations, and UI interactions still need an emulator/device run before they
can be called platform-tested; no independent Android security review has
been performed.

Manual install, Demo/history/delete, rotation, backup, offline, and
Owner-approved fixed-probe checks are in the
[Pixel 7a checklist](2026-ANDROID-PHASE2-PIXEL7A-TEST-CHECKLIST.md).

## Delivery

This is a stacked Draft PR based on the current PR #41 head. Do not merge,
retarget, accept ADR-0045, or enable ordinary cloud egress until the Owner
decisions in the companion packet are resolved.
