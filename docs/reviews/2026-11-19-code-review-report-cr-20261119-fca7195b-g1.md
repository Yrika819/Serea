# Code Review Report — H4-D1/D2 Private Google Calendar Provider Wiring

## Report Contract

- Report type: `code-review`
- Report ID: `cr-20261119-fca7195b`
- Review chain ID: `rc-20261119-b4a20d8e`
- Review generation: `1`
- Review trigger: `post-implementation`
- Parent review report ID: `cr-20261119-7f3a2c91`
- Parent review report path: `docs/reviews/2026-11-19-code-review-report-cr-20261119-7f3a2c91-g0.md`
- Parent resolution ID: `rr-20261119-a6d94c2f`
- Parent resolution path: `docs/reviews/2026-11-19-receiving-code-review-resolution-rr-20261119-a6d94c2f.md`
- Generated at: `2026-11-19T00:00:00Z`
- Report path: `docs/reviews/2026-11-19-code-review-report-cr-20261119-fca7195b-g1.md`
- Source skill: `code-review`
- Status: `Review incomplete`
- Git mutation during review: `None`
- Scope fingerprint: `sha256:a35508ab2b0fd973fe79d11a114670b2f507d47554da997782328c2255c601c3`

## Scope

- Review date: `2026-11-19`
- Scope kind: `working tree`
- Scope description: `Generation-1 review of the post-G0 implementation delta and affected provider/test execution chains: isolated loopback endpoint and session handoff guards, Private UI E2E fixture correction, Public Beta gate regression, focused workflow JUnit handling, and design-document whitespace correction.`
- Scope mode: `implementation delta plus affected execution chains`
- Baseline: `generation-0 target 0c7237eb9836296ceb55851097cc63855ab92d68; canonical main 077246aeab909928fb57316f1f3aefc6be69c0b3`
- Target: `working tree on codex/h4-d1-d2-private-google-calendar; HEAD 0c7237eb9836296ceb55851097cc63855ab92d68 plus six implementation paths in the working-tree delta`
- Changed paths: `6 implementation paths (review reports are lineage artifacts, not product implementation scope)`
- Diff size: `114 insertions / 9 deletions across the six implementation paths`
- Completion: `Incomplete - API 26/35 runtime, final-SHA CI, and uploaded artifact inspection are not available yet.`
- Requirements consulted: `Owner-approved implementation constraints; H4-D1/D2 requirements; docs/superpowers/specs/2026-11-18-private-google-calendar-live-provider-design.md; docs/security/private-google-calendar-live-activation-checklist.md; generation-0 report and receiving resolution.`
- Prior resolution consulted: `rr-20261119-a6d94c2f at docs/reviews/2026-11-19-receiving-code-review-resolution-rr-20261119-a6d94c2f.md`
- Assumptions: `Review is limited to the current working-tree delta from generation 0 and directly affected dependencies. No real OAuth, Google HTTP, Cloud, account, device, signing, or distribution operation was performed.`
- Excluded as unrelated: `Earlier feature wiring already present at generation 0; unrelated signing, Tasks, Gmail and AI features except where directly needed to trace egress boundaries.`

## Review Orchestration

- Assessment subagent: `Coordinator assessment - the implementation delta is small, but authorization/session security and E2E/workflow/artifact boundaries are independent high-risk review angles.`
- Orchestration decision: `Parallel specialists`
- Decision confidence: `high`
- Decision rationale: `Independent auth/session security tracing and UI/E2E/CI artifact tracing materially improve coverage while keeping ownership disjoint; the coordinator verified the only new candidate against OkHttp behavior.`
- Coordinator override: `None`
- Context or tool limits: `Reviewers performed static inspection only. The coordinator ran local JVM/compile/hygiene checks, but no API emulator/device was available and no final-SHA CI artifact existed.`

### Risk Dimensions

- `The isolated-only Live HTTP seam must remain loopback-only and session-bound; an endpoint mistake or generation race could permit unintended request dispatch.`
- `Private UI E2E identity/order, variant gates, and CI artifact contents must not confuse Fake data with Google data or leak sensitive values.`
- `API 26/35 runtime and CI evidence are needed to validate instrumentation execution and artifact contents beyond static review.`

### Reviewer Assignments

| Reviewer | Angle | Owned surfaces | Mandatory cross-checks | Status |
| --- | --- | --- | --- | --- |
| `R1` | Authorization, provider composition, security/privacy | `PrivateGoogleCalendarComposition`, adapter/session/transport dependencies, BuildConfig gates, isolated tests | `loopback destination, post-construction session binding, exact scope, invalidation/retry, token safety, Google-origin egress` | `Complete (static)` |
| `R2` | UI behavior, E2E, variant/workflow isolation | `Private UI loopback test, test identity/runner, focused workflow, Beta gate test, activation docs` | `fixture sequencing, no Fake fallback, cancellation, exact counts, no APK/package task, artifact paths` | `Complete (static)` |

### Synthesis Statement

`R1 found no new authorization/security defect and verified G0 F1/F2 remain closed. R2 verified G0 F3-F5 remain closed and raised one conditional IPv6 loopback normalization candidate. The coordinator reproduced OkHttp 4.12.0's actual HttpUrl.host value (`::1`) and confirmed both allowlists compare against the bracketed form (`[::1]`) or omit IPv6; the candidate is retained as a Minor test-host portability issue, not an external-HTTP risk. API/device/runtime and artifact-content surfaces remain uncovered.`

## Review Snapshot

- Recommendation: `Discuss`
- Completion: `Incomplete - API 26/35 instrumentation and uploaded artifact contents remain unverified.`
- Why now: `The implementation fixes all five G0 findings and has no Critical/High issue identified, but the isolated test endpoint accepts an IPv6 loopback spelling that OkHttp never returns and runtime CI evidence remains outstanding.`
- Must-review now:
  1. `F1` `IPv6 loopback host normalization in the test-only path`
  2. `A9` `API 26/35 instrumentation and final-SHA artifact inspection remain uncovered`
- Findings count: `Blocker 0 | Major 0 | Minor 1 | Question 0`
- Standalone test gaps: `Blocker 0 | Major 0 | Minor 0`
- Coverage confidence: `medium`
- Biggest blind spot: `Final-SHA API 26/35 instrumentation and uploaded JUnit/logcat contents`

## Complete Findings Index

| ID | Severity | Surface | Review risk | Confidence | Origin | Verification | Issue key | Issue fingerprint | Expected basis |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `F1` | `Minor` | `Isolated loopback endpoint validation` | `An IPv6-only MockWebServer URL is a valid loopback endpoint but is rejected by composition and the E2E request-host assertion.` | `high` | `R2-C1; Coordinator` | `OkHttp 4.12.0 runtime check plus static allowlist trace` | `behavior; entry=isolated loopback endpoint validation; contract=accept valid IPv4 and IPv6 loopback MockWebServer endpoints; effect=IPv6-only test host cannot construct or verify the Private Calendar provider` | `ifp-sha256:e070a471393f9728de60c50f53a5dbde55906a550b7653e9703e9e1b0e633322` | `kind:approved-design; strength:authoritative; evidence:Owner condition 4 requires MockWebServer loopback injection and the approved design requires an explicit loopback-only endpoint; IPv6 ::1 is a loopback address and is explicitly listed by the composition.` |

## Blocker

None.

## Major

None.

## Minor

### F1 Minor - IPv6 loopback host normalization in the test-only path

Impact: `An isolated test host that supplies a valid IPv6 loopback MockWebServer URL cannot construct or fully verify the Private Calendar provider path.`
Review reason: `The composition's intended loopback set includes IPv6, but OkHttp's host normalization makes the bracketed entry ineffective; the UI E2E's independent host assertion also omits IPv6.`
Surface: `PrivateGoogleCalendarComposition initialization and PrivateGoogleCalendarLoopbackUiTest request assertions.`
Issue key: `behavior; entry=isolated loopback endpoint validation; contract=accept valid IPv4 and IPv6 loopback MockWebServer endpoints; effect=IPv6-only test host cannot construct or verify the Private Calendar provider`
Issue fingerprint: `ifp-sha256:e070a471393f9728de60c50f53a5dbde55906a550b7653e9703e9e1b0e633322`
Expected basis: `kind:approved-design; strength:authoritative; evidence:Owner condition 4 requires MockWebServer loopback injection and the approved design requires an explicit loopback-only endpoint; IPv6 ::1 is a loopback address and is explicitly listed by the composition.`
Confidence: `high for host normalization and rejection; medium for occurrence on CI because the default MockWebServer host was not observed on API runners.`
Origin: `R2-C1; Coordinator verification`
Coordinator verification: `Ran OkHttp 4.12.0 HttpUrl parsing for http://[::1]:1234/calendar/v3/: HttpUrl.host returned ::1 and InetAddress reported loopback=true. Composition's LOOPBACK_TEST_HOSTS contains [::1] at line 90, while the UI helper accepts only localhost and 127.0.0.1. No external host is admitted by this mismatch.`

Look here first:
- [PrivateGoogleCalendarComposition.kt](../../android/app/src/main/java/app/serea/android/googleauth/PrivateGoogleCalendarComposition.kt#L24-L30)
- [PrivateGoogleCalendarLoopbackUiTest.kt](../../android/app/src/androidTest/java/app/serea/android/PrivateGoogleCalendarLoopbackUiTest.kt#L205-L210)

Failure mode:
- Expected: `A valid IPv6 loopback MockWebServer URL should satisfy the isolated test-only loopback restriction and host assertion, while all external hosts remain rejected.`
- Current: `HttpUrl.host for [::1] is ::1, but the composition allowlist compares against [::1]; the E2E request assertion accepts only localhost and 127.0.0.1.`

Evidence:
- `R2 static trace: PrivateGoogleCalendarComposition.kt:24-29 validates HttpUrl.host against LOOPBACK_TEST_HOSTS; lines 89-90 store [::1].`
- `Coordinator runtime check with the project's cached OkHttp 4.12.0 artifact returned host=::1 and isLoopback=true for http://[::1]:1234/calendar/v3/.`
- `PrivateGoogleCalendarLoopbackUiTest.kt:209-210 independently rejects a request whose HttpUrl.host is ::1.`
- `The current CI test server uses the default MockWebServer URL; no runtime evidence establishes that this issue occurs on the configured API 26/35 runners.`

Assumptions and limits:
- `The design's loopback-only MockWebServer requirement covers valid IPv6 loopback as well as IPv4/localhost. The failure is restricted to test endpoint portability and does not permit external traffic.`

Reviewer action:
`Track a focused test-host portability follow-up; no Critical/High issue blocks approval.`

## Questions

None.

## Test Gaps

None as a standalone gap. The IPv6 behavior is tracked as F1; runtime/API and artifact limits are listed as A9.

## Review Coverage Ledger

| Area ID | Area / path | Touched files or entry points | Owner | Depth | Status | Result | Evidence / next step |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `A1` | `Private Live endpoint selection and session handoff` | [composition](../../android/app/src/main/java/app/serea/android/googleauth/PrivateGoogleCalendarComposition.kt) | `R1; Coordinator` | `contract trace; dependency trace` | `Finding F1` | `IPv6 normalization candidate` | `Endpoint and session race fixes checked; the IPv6 spelling issue is test-only.` |
| `A2` | `Authorization adapter, provider sessions, and HTTP transport` | [adapter](../../android/app/src/main/java/app/serea/android/googleauth/GoogleCalendarAuthorizationAdapter.kt); [sessions](../../android/app/src/main/java/app/serea/android/calendar/CalendarProviderSessions.kt); [transport](../../android/app/src/main/java/app/serea/android/calendar/transport/GoogleCalendarHttpReadPort.kt) | `R1` | `dependency trace; contract trace` | `Reviewed - no issue found` | `Dependencies verified` | `Exact scopes, session invalidation, bounded retry, cancellation, redirect/token protection, and primary-only preflight traced.` |
| `A3` | `Private UI E2E detail, refresh, cancellation, close, and labels` | [E2E](../../android/app/src/androidTest/java/app/serea/android/PrivateGoogleCalendarLoopbackUiTest.kt) | `R2; Coordinator` | `contract trace; compile verified` | `Reviewed - no issue found` | `Fixture and error contracts verified` | `Detail fixture matches selected event/time; empty response follows detail; selected-Live errors do not fall back to Fake.` |
| `A4` | `Beta HTTP gate regression` | [Beta test](../../android/app/src/testBeta/java/app/serea/android/googleauth/BetaCalendarHttpGateTest.kt); [BuildConfig](../../android/app/build.gradle.kts) | `R2; Coordinator` | `test and build-config trace` | `Reviewed - no issue found` | `Variant test verified` | `Beta unit assertion passed 1/1 and failed under a temporary enabled-gate mutation; workflow adds only the unit-test task.` |
| `A5` | `Focused workflow, identities, JUnit and artifact paths` | [workflow](../../.github/workflows/android-functional-focused.yml); [runner](../../tools/run_android_functional_focused.sh); [identity manifest](../../android/app/src/androidTest/assets/serea-non-required-test-identities.txt) | `R2` | `static workflow trace` | `Reviewed - no issue found` | `Workflow paths inspected` | `Exact 47-case focused runner identity set is retained; unit workflow checks 11 isolated cases and one Beta gate case. Actual uploaded contents remain uninspected.` |
| `A6` | `Activation prerequisites and design contract` | [checklist](../../docs/security/private-google-calendar-live-activation-checklist.md); [design](../superpowers/specs/2026-11-18-private-google-calendar-live-provider-design.md) | `R2; Coordinator` | `contract trace` | `Reviewed - no issue found` | `No activation boundary issue` | `Separate Owner approvals, default-off HTTP, least scope, signing and egress boundaries remain explicit.` |
| `A7` | `Product variant and external-egress boundaries` | [BuildConfig](../../android/app/build.gradle.kts); [Calendar egress policy](../../android/app/src/main/java/app/serea/android/calendar/CalendarPermissionPolicy.kt) | `R1` | `dependency trace` | `Reviewed - no issue found` | `Product boundaries verified` | `Product HTTP gates remain literal false; Google-origin data is denied to external model dispatch.` |
| `A8` | `Credential fixture and test-only gateway` | [E2E gateway](../../android/app/src/androidTest/java/app/serea/android/PrivateGoogleCalendarLoopbackUiTest.kt#L75-L108) | `R2; Coordinator` | `static trust-boundary trace` | `Reviewed - no issue found` | `Synthetic inputs only` | `Synthetic short-lived token and exact read-only scope only; no real OAuth/account credential source.` |
| `A9` | `API 26/35 instrumentation and actual final-SHA CI artifact contents` | [focused workflow](../../.github/workflows/android-functional-focused.yml); [test runner](../../tools/run_android_functional_focused.sh) | `Coordinator` | `static only` | `Not covered` | `No runtime/artifact access` | `Local adb reported no attached devices and emulator was unavailable; no PR/final-SHA artifact exists yet. Run both API jobs and inspect the JUnit/logcat artifacts before declaring complete.` |

## Subagent Candidate Adjudication

| Candidate ID | Proposed by | Decision | Final ID | Coordinator evidence | Reason |
| --- | --- | --- | --- | --- | --- |
| `R1` | `R1` | `No new candidates` | `None` | `Coordinator reviewed R1's endpoint/session/scope/invalidation traces against the source and the local 11-case suite.` | `No new issue found; G0 F1/F2 remain closed under the recorded fixes.` |
| `R2-C1` | `R2` | `accepted` | `F1` | `OkHttp 4.12.0 runtime parsing confirmed host=::1 for a bracketed IPv6 literal; both production test allowlists were inspected.` | `Distinct from G0 F1: this rejects valid loopback; it does not allow an external production endpoint.` |
| `R2-dismissed-fixtures` | `R2` | `dismissed` | `None` | `The queued detail/refresh sequence and selected event ID/time were read directly.` | `G0 F3 is fixed; no remaining fixture mismatch found.` |
| `Coordinator-G0-reconciliation` | `Coordinator` | `kept closed` | `None` | `All five parent findings are listed in Prior Resolution Reconciliation with the concrete code/doc/test changes.` | `The current generation does not reopen the same semantic issue.` |

## Evidence Appendix

### Diff Inventory

| File or area | Classification | Semantic review area considered |
| --- | --- | --- |
| `.github/workflows/android-functional-focused.yml` | `config; test-only` | `Exact unit identities/count, no product APK task, JUnit paths` |
| `android/app/src/androidTest/java/app/serea/android/PrivateGoogleCalendarLoopbackUiTest.kt` | `test-only` | `Synthetic auth, loopback requests, detail/refresh fixture, cancellation, labels, artifact-safe assertions` |
| `android/app/src/main/java/app/serea/android/googleauth/PrivateGoogleCalendarComposition.kt` | `surface` | `Isolated test endpoint allowlist, authorization session generation and provider construction` |
| `android/app/src/testIsolated/java/app/serea/android/googleauth/PrivateGoogleCalendarCompositionTest.kt` | `test-only` | `Null/external endpoint and session-change regression cases` |
| `android/app/src/testBeta/java/app/serea/android/googleauth/BetaCalendarHttpGateTest.kt` | `test-only` | `Public Beta generated BuildConfig gate remains false` |
| `docs/superpowers/specs/2026-11-18-private-google-calendar-live-provider-design.md` | `docs-only` | `Accurate scope and clean validation` |

### Verification Commands

- `:app:testDebugUnitTest` -> `487 tests; failures=0, errors=0, skipped=0.`
- `:app:testPrivateBetaUnitTest --tests app.serea.android.googleauth.PrivateBuildOAuthGateTest` -> `1 test passed; no APK task invoked.`
- `:app:testBetaUnitTest --tests app.serea.android.googleauth.BetaCalendarHttpGateTest` -> `1 test passed; temporary true-gate mutation produced the expected assertion failure.`
- `:app:testIsolatedUnitTest --tests app.serea.android.googleauth.PrivateGoogleCalendarCompositionTest` -> `11 tests; failures=0, errors=0, skipped=0.`
- `:app:compileIsolatedAndroidTestKotlin`, `:app:compileBetaKotlin`, `:app:compilePrivateBetaKotlin` -> `passed.`
- `python3 tools/validate_docs.py docs` -> `224 Markdown files passed.`
- `python3 /Users/yuta/.agents/skills/code-review/scripts/validate_review_report.py docs/reviews/2026-11-19-code-review-report-cr-20261119-7f3a2c91-g0.md` -> `5 findings; 0 test gaps; 11 coverage areas; validator passed.`
- `bash -n tools/run_android_functional_focused.sh`, Ruby YAML parse, `git diff --check`, and scoped credential/scope scans -> `passed.`
- OkHttp 4.12.0 `HttpUrl.get("http://[::1]:1234/calendar/v3/")` -> `host=::1; isLoopback=true; url=http://[::1]:1234/calendar/v3/.`
- `adb devices -l` -> `List of devices attached` (none); `emulator` -> `not installed`.
- No generation-1 reviewer ran an independent Gradle/device test; R1/R2 supplied static coverage. The coordinator's listed local tests were executed before the generation-1 review.

### Supporting Code Links

| ID | Role | Link | Why it matters |
| --- | --- | --- | --- |
| `F1` | `endpoint guard` | [composition endpoint check](../../android/app/src/main/java/app/serea/android/googleauth/PrivateGoogleCalendarComposition.kt#L24-L30) | `The URL host is compared against the explicit test loopback allowlist.` |
| `F1` | `allowlist` | [loopback host set](../../android/app/src/main/java/app/serea/android/googleauth/PrivateGoogleCalendarComposition.kt#L89-L91) | `The IPv6 string is bracketed although HttpUrl.host is unbracketed.` |
| `F1` | `E2E assertion` | [request host assertion](../../android/app/src/androidTest/java/app/serea/android/PrivateGoogleCalendarLoopbackUiTest.kt#L205-L210) | `The UI E2E host helper supports only localhost/IPv4.` |

### Dismissed Coordinator Candidates

| Candidate | Decision | Evidence |
| --- | --- | --- |
| `The production Google HTTP endpoint remains reachable through the Private test composition.` | `dismissed` | `The Private composition now requires an explicit loopback URL before status/token access; the HTTP product gate is false in every product variant.` |
| `The session recheck can invalidate a replacement session.` | `dismissed` | `The expected session ID is compared after adapter construction; a mismatch closes the new session and fails before token/HTTP.` |
| `The E2E detail fixture still mismatches the selected event.` | `dismissed` | `The selected event and detail response both use synthetic-timed at 14:00 -05:00; the empty refresh response follows it.` |
| `The Beta regression workflow can build or sign an APK.` | `dismissed` | `It invokes only :app:testBetaUnitTest and asserts one JUnit case; no package task was added.` |
| `Private Beta OAuth/UI gates activate Google Calendar HTTP.` | `dismissed` | `The independent PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED BuildConfig is literal false in Private Beta; beta test and Private gate test assert the isolation.` |

### Blind Spots

| Area ID | Blind spot | Decision risk | What would resolve it |
| --- | --- | --- | --- |
| `A9` | `No API 26/35 runtime or uploaded-artifact inspection for the final SHA.` | `Static checks cannot establish device behavior or prove that no sensitive data entered collected JUnit/logcat artifacts.` | `Run required same-SHA CI and focused API 26/35 E2E jobs, verify all four Private identities, and inspect uploaded JUnit/logcat artifacts.` |

## Prior Resolution Reconciliation

| Issue key | Issue fingerprint | Parent item/verdict | Relevant change or new evidence | Decision |
| --- | --- | --- | --- | --- |
| `behavior; entry=isolated primary provider composition; contract=all test HTTP targets loopback MockWebServer; effect=missing or external endpoint permits Google request` | `ifp-sha256:ad5b065a852e4b61696c075175225677fa73fc4223a7e1ed5707d1254748b582` | `F1 Fixed` | `kind:code; ref:android/app/src/main/java/app/serea/android/googleauth/PrivateGoogleCalendarComposition.kt and PrivateGoogleCalendarCompositionTest.kt; change:requires a concrete loopback endpoint and tests reject missing/production URLs before authorization.` | `kept closed` |
| `behavior; entry=authorized Primary session construction; contract=provider token adapter stays bound to the session that passed authorization; effect=replacement session can authorize a read after account switch` | `ifp-sha256:a57e6a400a177f6dbe29f914c1283f961ae18d6e9f1d4a4a675d9be4c220bee2` | `F2 Fixed` | `kind:code; ref:PrivateGoogleCalendarComposition.kt and PrivateGoogleCalendarCompositionTest.kt; change:rechecks the session after adapter construction and rejects a mismatch before token/HTTP.` | `kept closed` |
| `behavior; entry=private-calendar-loopback-happy-path; contract=detail response matches selected event and refresh receives an empty-list response; effect=focused E2E fails before remaining success assertions` | `ifp-sha256:b8bb0882fd36698eba8f59578f900037e0f836d0c245b8c29df7e91d46589b50` | `F3 Fixed` | `kind:code; ref:android/app/src/androidTest/java/app/serea/android/PrivateGoogleCalendarLoopbackUiTest.kt; change:detail response now matches selected ID/time and the following queued refresh response is empty.` | `kept closed` |
| `behavior; entry=product HTTP gate verification; contract=every product variant generated gate is false; effect=Beta variant override escapes regression tests` | `ifp-sha256:8c1d32136a4f4563a3d48a593ef71a32f54a24570036b129840b260ed8385bec` | `F4 Fixed` | `kind:code; ref:android/app/src/testBeta/java/app/serea/android/googleauth/BetaCalendarHttpGateTest.kt and .github/workflows/android-functional-focused.yml; change:adds a Beta-only BuildConfig test and unit-only workflow invocation.` | `kept closed` |
| `behavior; entry=branch diff validation; contract=git diff --check passes for all introduced documentation; effect=trailing whitespace makes the required diff check fail` | `ifp-sha256:bacd4970133d06c33e39d0709afcf2aa76e999d611b0ab4675eef480e17dfd7d` | `F5 Fixed` | `kind:code; ref:docs/superpowers/specs/2026-11-18-private-google-calendar-live-provider-design.md; change:removed trailing spaces from the Status and Baseline lines.` | `kept closed` |

## Receiving Handoff

- Handoff status: `Terminal post-review - return to user/owner`
- Automatic receiving permitted: `No`
- Source report ID: `cr-20261119-fca7195b`
- Scope fingerprint to recheck: `sha256:a35508ab2b0fd973fe79d11a114670b2f507d47554da997782328c2255c601c3`
- Actionable finding IDs: `None`
- Deferred finding IDs: `F1`
- Actionable test-gap IDs: `None`
- Deferred test-gap IDs: `None`
- Open question IDs: `None`
- Open coverage area IDs: `A9`
- Highest-risk verification to repeat: `Run the exact final-SHA focused API 26/35 suite and inspect all uploaded JUnit/logcat artifacts for accidental sensitive payloads.`
- Suggested implementation boundaries: `If the IPv6 test-host issue is addressed in a later authorized task, normalize the host comparison and E2E helper together; keep product gates false.`
- Re-review note: `This generation-1 report is terminal. Remaining Minor and Not-covered items return to the Owner; do not automatically invoke receiving-code-review.`
- Chain rule: `Generation 1 is terminal. Any later implementation requires an explicit user request and a new generation-0 review chain.`

## Report Self-Check

- [yes] Actual assessment mode and rationale are recorded: coordinator assessment with two delegated read-only specialists.
- [yes] Every changed review-relevant or unknown-impact implementation area appears once in `Review Coverage Ledger`.
- [yes] Every final finding appears once in the index and once as a matching card.
- [yes] Every `Finding F#` area references an existing finding.
- [yes] Every standalone test gap has a stable ID and severity; none are separate from F1/A9.
- [yes] Every finding has a unique semantic issue fingerprint and an authoritative expected-behavior basis.
- [yes] Generation, trigger, parent report, parent resolution, scope mode, and terminal handoff satisfy the bounded chain contract.
- [yes] Generation 1 reconciles all five relevant parent findings and keeps them closed.
- [yes] Every non-Question finding and standalone test gap appears exactly once in actionable or deferred handoff IDs; the uncovered area appears in its open list.
- [yes] Every meaningful specialist candidate has an adjudication.
- [yes] Every `Not covered` area has a reason and next step.
- [yes] Recommendation follows the skill mapping: a Not-covered area requires Discuss.
- [no] The validator passes; generation 1 includes `--parent-report` and `--parent-resolution`.
- [yes] Reviewers did not mutate Git state; the coordinator did not stage, commit, or push during review.
