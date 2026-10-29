# G9RC Orchestrator Ledger — 2026-10-29

## Authority and protected boundaries

- Protected `main`: `e0acb0254820c381dd97828824bef86a1db5176c` (tree was confirmed unchanged; no merge or push to main).
- PR #155 base: `d4a36fb5d3613bca2d60151de0991a1baac72fc9`; HEAD `c6d9b3971c19d75c6824de616e703b105b6be71f`; tree `b5adb0cd42e33d4e3b065dc6f9e9e716bbee5448`; OPEN/DRAFT. Its current branch CI is partial. Exact-head Fast `40882853803`, RustSec `40882853433`, P7 synthetic Full job `123478701415` passed. Full Linux run `40882853605` remains in progress, Android run `40882853509` has some jobs queued, SQLite `40882853749` in progress, Native matrix `40882853542` partly queued/in progress. This docs-only HEAD has no Android product-test evidence.
- PR #159: Owner decision packet review ACCEPTED for its O-01–O-10 matrix. Exact source `982c8271698c5ea323e7a4d19f4dd662087db2c5`, tree `11e3c0ca61577bb435e0f69b1fcd24313857ff1f`; Android run `40881913899` docs and Required Gate contract passed, while product Build/Unit/Integration/Security/Audit/Emulator/Identity jobs were SKIPPED. This is not product acceptance.
- No real Google OAuth/account/data, Cloud API key or billing, production signing, physical Pixel 7a operation, release distribution, or personal data deletion occurred. Personal Beta remains NO-GO.

## Agent outcomes

### Agent A — Native Runtime

- Original Native worktree reached PR #160. Current source `025aff71effb03e0fe3bd540079530369e9beaf7`, tree `73ff9019802aba4a7ac9437e52ff407842fbfcdc`.
- Agent C independently ACCEPTED the corrective diagnostics patch: historical report/run/artifact evidence restored; raw instrumentation/logcat console tails removed; only allowlisted status/JUnit summaries printed; combined gate+cleanup failure harness checks that raw markers do not escape and evidence is removed.
- Exact-source local checks: `bash tools/g8-local-model-native/test-cleanup.sh` PASS; `bash -n tools/g8-local-model-native/run-e2e.sh` PASS; `git diff --check` PASS.
- Exact #160 focused Native run `40884851425` attempt 1/job `123485695046` remains QUEUED with no runner; Android run `40884851432` is PENDING with no jobs. No Native JUnit or inference artifact exists. Two-turn generation, Usage/Egress, cancellation runtime and recovery remain NOT_VERIFIED.
- Prior source `447e151738cb4e98c5d97cddf47b96e0d1035656` Android run `40882489925`, job `123480094333`, passed APK Build/Lint/Instrumentation compile. Artifact `12901867407`, 33,146,769 bytes, digest `sha256:5d887e5f5ff688675e813af2d7f43fe143cd7bc8f290bee81b094f0e13d12c89`; download was denied (HTTP 403), so APK hash/signer were not inspected. This build-only evidence is not transferred to `025aff7…`.
- `.github/workflows/android-personal-beta.yml` push run `40884839863` ended failure with `jobs=[]`; the workflow is dispatch-only/main-guarded. Classified as a no-job orchestration event, not a product/release failure; no dispatch/retry.
- **Review:** diagnostics patch ACCEPTED. Full Native/runtime gate PENDING_REVIEW / NOT_VERIFIED pending exact-head build and inference.

### Agent B — Android UI / Google / Gmail

- PR #161 initial source `0941043a1e687ec3b4e4778ce387c7f2c4b70ba1`, tree `e64ddd7bafd7c03de3898f2c2503f6ff1aff8604`, was independently reviewed. The HTTPS fixture boundary was ACCEPTED, but local compilation exposed invalid `Dns` SAM lambdas and a missing Compose extension import; a first compiled JVM run also exposed two assertions that expected an unavailable Host header. Those failed attempts are retained in the agent report and were fixed.
- PR #161 follow-up HEAD `ff42d9bcd08010dd7100d708d17fc676bcd5e739`, tree `eadb4800efcf837d63348eb9398d37310f364fb5`, remains OPEN/DRAFT. It changes tests/test helpers/report only; no production code, composition, app/test manifest, Required identity, or Required Gate was changed.
- The test-only client restricts initial requests to HTTPS `www.googleapis.com:443`, exact Calendar routes, GET and allowlisted query keys; it blocks redirects, rejects non-local DNS and rewrites to an ephemeral HTTPS localhost MockWebServer. The post-rewrite `RecordedRequest` asserts localhost URL. No real account/data is used.
- Official Adoptium Temurin JDK `17.0.21+9` was extracted under `/workspace/.tools` (no system Java change); official package SHA-256 matched `8b9b611d917e3dfd8419d3566b5949226f69b052fed7e109c1368add1dfe8711`.
- Exact #161 source local command `./gradlew :app:testDebugUnitTest :app:compileIsolatedAndroidTestKotlin --rerun-tasks` with JDK17 completed SUCCESS: 408 JVM tests, 0 failures, 0 errors, 0 skips; 34 JUnit XML files; aggregate per-file digest `6ec0718bebbe5a188b0eeb70fb41066c09931eb3200229be2d6a6622d82ad11b`. Isolated AndroidTest Kotlin compile passed in the same run (55 tasks executed).
- Android SDK has Build Tools 35.0.0, platform 35, and platform-tools 37.0.1 only; no API26 platform, NDK, or Emulator binary/device. Physical/API26/35 UI runtime was not run locally.
- Agent C review of exact `ff42d9b…` accepted the fixture boundary and assertion changes. Fresh GitHub Android run `40886083127` is PENDING with no jobs/artifacts; API26/35 UI and Required results remain NOT_RUN. Beta workflow `40886075461` also ended with zero jobs; no retry was issued.
- **Review:** test transport/security and assertion changes ACCEPTED; UI/runtime acceptance remains PENDING exact-head Android jobs.

### Agent C — CI / Security / Release

- Reviewed exact sources and run/artifact provenance for PRs #155/#159/#160/#161; did not modify files.
- PR #159 decision matrix ACCEPTED. PR #161 fixture boundary ACCEPTED. PR #160 diagnostics correction ACCEPTED; Native inference BLOCKED/NOT_VERIFIED. PR #155 formal CI remains partial.
- Owner gates remain closed for OAuth, Cloud, Restricted Gmail activation, production signing/distribution, Pixel, main merge and Beta GO.

## Parent integration candidate

- Started a new branch `codex/serea-g9rc-ui-accepted` from PR #155 exact head `c6d9b3971c19d75c6824de616e703b105b6be71f` (tree `b5adb0cd42e33d4e3b065dc6f9e9e716bbee5448`).
- Selectively cherry-picked only Agent C-accepted PR #161 UI-test changes: `0941043a1e687ec3b4e4778ce387c7f2c4b70ba1` and `ff42d9bcd08010dd7100d708d17fc676bcd5e739`. Result before this Ledger commit: HEAD `dd45ef48ae4a494235292f7d9f9b50d203a75537`, tree `3cf35a4379fb4628a49c3ec30961dc87a9ef4fea`.
- Native Runtime source was not integrated because no exact-head inference/runtime evidence exists. No composition-root changes were integrated.
- This branch is intended for a new Draft integration PR based on PR #155's branch. Exact integrated-source CI must still run. API26/35 UI acceptance stays PARTIAL until JUnit identities/counts/artifacts prove execution on both API levels.

## Current gates / next action

- G8 Evidence Closure: PARTIAL.
- Android Recovery: PARTIAL (local JVM and instrumentation compile pass; API26/35 UI runtime pending).
- Native Runtime: PARTIAL; Real Model Inference NOT_RUN.
- G9 Functional Candidate: PARTIAL.
- Model Selection Preflight: PARTIAL; static official-source review only, standard model UNDECIDED, no weights downloaded.
- G10 Preflight: PARTIAL; decision packet accepted, product release rehearsals and evidence remain.
- Personal Beta: NO-GO.
- Next: finish the exact integrated-source JDK checks, push this branch as a Draft PR, monitor its API26/35 Required/UI jobs without retries while runner-unassigned, and continue #160 exact-head Native build/inference when the focused runner starts. Re-review any changed source SHA before accepting. No main merge or owner-only action.

## Integrated candidate focused verification

Before this Ledger update, the new candidate source was `8ea137642ef6a0c7ef0e2a78905569ea7d598b58`, tree `779c2e05a9d2bd3c62b71566c5c2e049a3aed0f9`. On that exact source, Java 17 command `./gradlew :app:testDebugUnitTest :app:compileIsolatedAndroidTestKotlin --rerun-tasks` completed SUCCESS: JVM 408 tests, 0 failures, 0 errors, 0 skips, 34 JUnit XML files; AndroidTest Kotlin compile passed. Aggregate per-file JUnit digest: `3c4283818089c7c52f3ca531eae80e98bee3c93fe16d4fd75ecaa3490d46190e`. No emulator was run. This result is local focused verification and does not replace API26/35 Required/UI CI.

The new integration branch is being kept separate from PR #155. It contains the reviewed test-only UI transport fixes and this ledger. A new Draft PR and exact-head hosted CI are the next steps; Native diagnostic code remains only in PR #160 because Native inference has not been established.

## #162 current-source checkpoint — 2026-10-29

PR #162 is OPEN/DRAFT on `codex/serea-g9rc-ui-accepted`, based on PR #155's branch `codex/serea-g9r-integration-candidate`. Before this checkpoint the exact candidate HEAD was `6a37e090a228633faf319fd6c8fa1596e7ae9c9f`, tree `b697ce9f197cf826d343dfa6e60ecef8f2e1542d`. This source contains only the reviewed PR #161 test-only changes plus the new ledger atop PR #155; no Native Runtime source was integrated.

On that exact candidate source, JDK17 `./gradlew :app:testDebugUnitTest :app:compileIsolatedAndroidTestKotlin --rerun-tasks` completed SUCCESS: 408 tests, 0 failures/errors/skips, 34 JUnit XML files, isolated AndroidTest Kotlin compile PASS. Aggregate JUnit digest: `95c50242b30e61e08d11dec9a1c67b2e48cc473b3cd8a9791658aa9b553c7b59`. No Emulator was run locally.

Exact-head PR #162 checks started at run IDs: Android `40887554119` (Change Scope job `123495223453` QUEUED), Fast `40887553700`, Full `40887553860`, Native platform `40887553695`, SQLite `40887553607`, RustSec `40887553982`, Git Hooks `40887554005`; these were QUEUED at the latest query. They are bound to `6a37e09…` and are not yet Required/UI evidence.

PR #160 focused Native run `40884851425`, attempt 1/job `123485695046`, has advanced from QUEUED and started at `2026-10-29T14:01:15Z`. Exact source remains `025aff71effb03e0fe3bd540079530369e9beaf7`; at last query it was installing pinned Android build/Emulator packages. The inference step has not run yet; no result or artifact is claimed.
