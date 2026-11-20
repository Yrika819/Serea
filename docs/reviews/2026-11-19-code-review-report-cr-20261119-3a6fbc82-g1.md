# Code Review Report — H4-D1/D2 Post-Implementation Delta

## Report Contract

- Report type: `code-review`
- Report ID: `cr-20261119-3a6fbc82`
- Review chain ID: `rc-20261119-2a9d620b`
- Review generation: `1`
- Review trigger: `post-implementation`
- Parent review report ID: `cr-20261119-92d7c4e1`
- Parent review report path: `docs/reviews/2026-11-19-code-review-report-cr-20261119-92d7c4e1-g0.md`
- Parent resolution ID: `rr-20261119-e1a4b9d2`
- Parent resolution path: `docs/reviews/2026-11-19-receiving-code-review-resolution-rr-20261119-e1a4b9d2.md`
- Generated at: `2026-11-18T23:22:37Z`
- Report path: `docs/reviews/2026-11-19-code-review-report-cr-20261119-3a6fbc82-g1.md`
- Source skill: `code-review`
- Status: `Review complete`
- Git mutation during review: `None`
- Scope fingerprint: `sha256:8f6fe291dd47d6c38196476c6d4ec05b466fc25bd0e34328beec79add9411712`

## Scope

- Review date: `2026-11-19`
- Scope kind: `commit range`
- Scope description: `Terminal generation-1 review of the delta from the frozen G0 target through 425adc2 and the directly affected provider, isolated loopback, Private UI E2E, and CI test-accounting chains. The broader H4-D1/D2 implementation is not reopened. The new G0 report document is lineage input, not application behavior.`
- Scope mode: `implementation delta plus affected execution chains`
- Baseline: `9dd770012ce7f79e82de1d30acc60fd8641aa1d7; Tree 6f67e77e1fd1cf80284a5c7ab32402b6bf58b757`
- Target: `425adc266556765bbbf6c9d4e33dbc8c86dd7419; Tree 99f3901dca3c2029e01ac37313ff361868c8ebb1`
- Changed paths: `11`
- Diff size: `440 insertions / 9 deletions; 52,192 diff bytes; diff SHA-256 497fc17746d43862dbd103372b4dc3a7976076a72321cbc9a8fa871e80eea094`
- Completion: `Complete within the reviewed scope. One isolated IPv6 endpoint integration gap and one minor visibility-assertion gap are reported; no Critical or High finding was identified.`
- Requirements consulted: `Owner-approved H4-D1/D2 instructions; frozen source report cr-20261119-92d7c4e1; its resolution rr-20261119-e1a4b9d2; docs/security/private-google-calendar-live-activation-checklist.md; the approved Private Calendar provider design; PR #245 metadata and target-SHA CI evidence.`
- Prior resolution consulted: `rr-20261119-e1a4b9d2 at docs/reviews/2026-11-19-receiving-code-review-resolution-rr-20261119-e1a4b9d2.md`
- Assumptions: `Source-SHA GitHub test artifacts reflect commit 425adc2 as recorded by their workflow/environment metadata. No real OAuth, Google API, account, Google Cloud, signing, distribution, or device operation is in scope or was performed.`
- Excluded as unrelated: `Unchanged provider/session/HTTP implementation except where directly reached by changed IPv6 composition; Tasks, Gmail, AI, signing, and unrelated Real IME diagnostic workflow failure.`

## Review Orchestration

- Assessment subagent: `Coordinator assessment - the 11-path delta spans independent authorization/variant and Android E2E/CI evidence risks; the subagent split materially improves adversarial coverage.`
- Orchestration decision: `Parallel specialists`
- Decision confidence: `high`
- Decision rationale: `R1 traced the Live provider and trust boundaries; R2 independently checked loopback policy, UI request assertions, test identity thresholds, and uploaded artifacts. Their scopes were separable, while the coordinator checked the cross-layer IPv6 path and reconciled both candidates.`
- Coordinator override: `None`
- Context or tool limits: `No local Android device/runtime was used in this read-only review. Same-SHA GitHub CI and downloaded artifacts were inspected. Artifact scanning is marker-based, not semantic DLP.`

### Risk Dimensions

- `Synthetic authorization and the isolated-only Live HTTP seam cross product and test variants; accidental broadening could enable a product network path.`
- `Loopback host validation is layered across composition, provider session creation, HTTP transport, and Android cleartext policy; each layer must agree on canonical IPv6 host form.`
- `UI E2E and formal-count changes affect release evidence; thresholds and request assertions must remain exact and artifacts must not expose synthetic account/event/token data.`

### Reviewer Assignments

| Reviewer | Angle | Owned surfaces | Mandatory cross-checks | Status |
| --- | --- | --- | --- | --- |
| `R1` | `Security, provider, and variant isolation` | `PrivateGoogleAuthorizationViewModel test Gateway guard; PrivateGoogleCalendarComposition; isolated manifest/network policy; direct provider/session/HTTP contracts` | `Product HTTP false; isolated-only injection; canonical host through HTTP transport; Primary scope; token/session and no-egress boundaries` | `Complete` |
| `R2` | `Android UI E2E, test accounting, and CI artifacts` | `PrivateGoogleCalendarLoopbackUiTest; focused workflow; exact focused/identity counts; instrumentation-result verifier; relevant artifact scans` | `Exact Primary GET paths and scopes; no threshold reduction; timezone detail assertions; sanitized artifacts; API 26/35 evidence` | `Complete` |

### Synthesis Statement

`The coordinator independently traced R1's IPv6 candidate from the composition into CalendarProviderSessions and the HTTP port, confirming that the downstream allowlist still rejects the canonical host. The earlier report's F3 disposition is therefore reopened for this same semantic issue. R2's timezone candidate was verified as a test-only assertion gap: no UI defect was demonstrated, but the count assertion alone does not express that both matching nodes are displayed. Other material candidates were dismissed against the changed source, explicit Owner invariants, and same-SHA evidence. No Critical or High issue remains in the reviewed delta.`

## Review Snapshot

- Recommendation: `Pass with caveat`
- Completion: `Complete within the reviewed scope`
- Why now: `The terminal review confirms the Private/product gates and required test thresholds remain safe; the remaining issues are limited to isolated IPv6 test transport support and a minor visibility assertion.`
- Must-review now:
  1. `F1` `Canonical IPv6 loopback endpoint rejected by HTTP transport`
  2. `T1` `Timezone matches counted but not individually asserted displayed`
- Findings count: `Blocker 0 | Major 0 | Minor 1 | Question 0`
- Standalone test gaps: `Blocker 0 | Major 0 | Minor 1`
- Coverage confidence: `medium`
- Biggest blind spot: `The canonical IPv6 endpoint is accepted by composition but not exercised through provider construction and HTTP dispatch; the downstream mismatch is reported as F1.`

## Complete Findings Index

| ID | Severity | Surface | Review risk | Confidence | Origin | Verification | Issue key | Issue fingerprint | Expected basis |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `F1` | `Minor` | `Isolated Primary provider construction with IPv6 loopback base URL` | `Composition admits canonical ::1, but the downstream HTTP-port host allowlist retains [::1], so provider session construction rejects the valid loopback endpoint.` | `high` | `R1-C1; Coordinator` | `Static call-chain trace through composition, CalendarProviderSessions, and GoogleCalendarHttpReadPort; compared with the limited IPv6 test.` | `behavior; entry=isolated loopback endpoint validation; contract=valid IPv6 loopback hosts are accepted consistently by composition and request verification; effect=IPv6 MockWebServer paths are rejected before provider traffic` | `ifp-sha256:93adac8397dcb76e16d749459060f8e226196978affc02b26049bd71d87027ce` | `kind:owner-decision; strength:authoritative; evidence:Owner requires a loopback-only MockWebServer path and the accepted G0 review explicitly identifies canonical ::1 as a supported loopback host.` |

## Blocker

None.

## Major

None.

## Minor

### F1 Minor - Canonical IPv6 loopback endpoint rejected by HTTP transport

Impact: `The isolated test composition can accept an IPv6 loopback URL but cannot construct its Primary Google HTTP provider session. This does not create product Google traffic or widen network access.`

Review reason: `The composition accepts the canonical unbracketed host ::1, while GoogleCalendarHttpReadPort validates the same HttpUrl.host against a TEST_HOSTS set containing the bracketed spelling [::1]. The failure is fail-closed, but the advertised isolated IPv6 test endpoint is not usable end to end.`

Surface: `PrivateGoogleCalendarComposition.openAuthorizedPrimary -> CalendarProviderSessions.openPrimaryGoogleReadOnly -> GoogleCalendarHttpReadPort constructor.`

Issue key: `behavior; entry=isolated loopback endpoint validation; contract=valid IPv6 loopback hosts are accepted consistently by composition and request verification; effect=IPv6 MockWebServer paths are rejected before provider traffic`

Issue fingerprint: `ifp-sha256:93adac8397dcb76e16d749459060f8e226196978affc02b26049bd71d87027ce`

Expected basis: `kind:owner-decision; strength:authoritative; evidence:Owner requires a loopback-only MockWebServer path and the accepted G0 review explicitly identifies canonical ::1 as a supported loopback host.`

Confidence: `High for the static mismatch. The current focused E2E server host resolves to a working loopback form, so the pass on API 26/35 does not disprove the IPv6-specific failure.`

Origin: `R1-C1; Coordinator`

Coordinator verification: `Re-read the current HttpUrl host handling assumption reflected by the existing canonical IPv6 composition test; verified that the same baseUrl is passed through CalendarProviderSessions to GoogleCalendarHttpReadPort. The lower constructor requires baseUrl.host in TEST_HOSTS, whose IPv6 entry remains [::1]. The added test only constructs PrivateGoogleCalendarComposition and reads opensGoogleProvider; it does not create a provider session.`

Look here first:
- [`PrivateGoogleCalendarComposition.kt`](../../android/app/src/main/java/app/serea/android/googleauth/PrivateGoogleCalendarComposition.kt#L89)
- [`GoogleCalendarHttpReadPort.kt`](../../android/app/src/main/java/app/serea/android/calendar/transport/GoogleCalendarHttpReadPort.kt#L407)

Failure mode:
- Expected: `A canonical IPv6 loopback URL admitted by the isolated composition can pass the transport's loopback validation and create a Primary-only session; product variants remain unable to use this test endpoint.`
- Current: `The composition allows ::1, but the transport only lists [::1]. Its constructor requirement fails while opening the session, before any HTTP request.`

Evidence:
- `PrivateGoogleCalendarComposition` accepts host ::1 and passes the same baseUrl to `openPrimaryGoogleReadOnly`.
- `CalendarProviderSessions` passes the URL into `GoogleCalendarHttpReadPort`, whose `loopbackTest` check compares `baseUrl.host` with `TEST_HOSTS = {127.0.0.1, localhost, [::1]}`.
- `canonicalIpv6LoopbackEndpointIsAccepted` checks only the composition property; it does not call `openAuthorizedPrimary` or instantiate the HTTP port.
- `:app:testIsolatedUnitTest` reports 12/12 and focused UI E2E passes on API 26/35, but those checks do not cover this IPv6 provider-construction path.

Assumptions and limits:
- `The contract intends support for the canonical ::1 host, as recorded by the source review and the explicit composition/unit test. This is an isolated test endpoint compatibility issue only.`

Reviewer action:
`Align the HTTP port's loopback host comparison with canonical HttpUrl.host form and add a provider-opening test that proves the IPv6 URL reaches the MockWebServer without broadening the allowlist.`

## Questions

None.

## Test Gaps

| ID | Severity | Surface | Missing coverage | Risk | Origin | Evidence | Issue key | Issue fingerprint | Expected basis |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `T1` | `Minor` | `Private Calendar event-detail timezone rendering` | `No assertion individually proves both event-timezone and display-timezone text nodes are displayed in the opened detail.` | `A hidden or offscreen timezone value could coexist with the exact node count and still pass the test.` | `R2-C1; Coordinator` | `The test scrolls the detail item, asserts two America/New_York matches by count, then asserts the display-timezone label and formatted date displayed; it does not assert each matching text node with a displayed assertion.` | `test-gap; entry=Private Calendar event-detail timezone rendering; contract=both event and display timezone values are visibly asserted in the opened event detail; gap=timezone text uses cardinality assertion rather than individual displayed-node assertion` | `ifp-sha256:ba28ba0d2d47bbdf5330720800225a16234e3a11bc54060e714c775d9e590c37` | `kind:owner-decision; strength:authoritative; evidence:Owner's H4-D1/D2 acceptance criteria explicitly require event-detail display and timezone coverage in the UI E2E.` |

## Review Coverage Ledger

| Area ID | Area / path | Touched files or entry points | Owner | Depth | Status | Result | Evidence / next step |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `A1` | `Private synthetic Gateway boundary and product Live gate` | `PrivateGoogleAuthorizationViewModel.kt; build.gradle.kts product BuildConfig; product ViewModel factory` | `R1` | `contract trace` | `Reviewed - no issue found` | `Non-null synthetic Gateway is rejected outside isolated; product factories use default composition; product HTTP settings remain false.` | `Debug regression test and same-SHA variant checks support the boundary; no activation change recommended.` |
| `A2` | `Isolated cleartext exception` | `src/isolated/AndroidManifest.xml; loopback_network_security_config.xml; main AndroidManifest.xml` | `R1; R2` | `runtime verified` | `Reviewed - no issue found` | `Only the isolated overlay selects a loopback-only exception; base and product cleartext remain denied.` | `UI policy assertions permit localhost, 127.0.0.1, ::1 and reject example.com; focused API 26/35 suites pass.` |
| `A3` | `Canonical IPv6 host through provider and HTTP transport` | `PrivateGoogleCalendarComposition.kt; CalendarProviderSessions.kt; GoogleCalendarHttpReadPort.kt; canonicalIpv6LoopbackEndpointIsAccepted` | `R1; Coordinator` | `dependency trace` | `Finding F1` | `The composition-level test does not instantiate the transport; the downstream allowlist rejects canonical ::1.` | `Add full provider-opening and request coverage after aligning the lower allowlist.` |
| `A4` | `Private authorization UI to MockWebServer Primary event flow` | `PrivateGoogleCalendarLoopbackUiTest.kt; ViewModel; provider session; HTTP read port` | `R2` | `runtime verified` | `Reviewed - no issue found` | `The successful path checks four GET paths, two-page loading, details, refresh, session close, exact read-only scope, and no CalendarList/Gmail route.` | `Same-SHA API 26/35 focused suites each report 47/47; product defaults remain Fake.` |
| `A5` | `Event detail timezone display assertion` | `PrivateGoogleCalendarLoopbackUiTest.kt; CalendarAgendaScreen.kt DetailField rendering` | `R2; Coordinator` | `contract trace + runtime evidence` | `Reviewed - no issue found` | `UI behavior was not shown incorrect; standalone T1 records that the test does not assert both matching values individually displayed.` | `Strengthen the assertion if the Owner requires explicit visibility proof; existing successful UI output is supporting evidence only.` |
| `A6` | `Request boundary and synthetic scope assertions` | `PrivateGoogleCalendarLoopbackUiTest.kt; GoogleCalendarAuthorizationAdapter.kt; GoogleCalendarHttpReadPort.kt` | `R2; R1` | `dependency trace + runtime verified` | `Reviewed - no issue found` | `The fixture asserts loopback host, exact Primary event/detail paths, GET methods, pagination token order, no CalendarList/Gmail paths, and calendar.events.readonly only.` | `Same-SHA UI run and changed source trace; unchanged contract tests remain in the required suite.` |
| `A7` | `Focused and formal exact test accounting` | `android-functional-focused.yml; run_android_functional_focused.sh; tests/android_instrumentation_results_tests.py` | `R2; Coordinator` | `contract trace + runtime verified` | `Reviewed - no issue found` | `Focused count remains exact 47; Formal remains exact 105 = 103 required + 2 additional; non-required count is updated to 33 without overlap.` | `The Python instrumentation regression suite passed 13/13 locally; Required Android Unit and emulator contexts pass on target SHA.` |
| `A8` | `CI artifact handling for synthetic fixtures` | `Focused workflow artifact upload; target-SHA .ci-artifacts bundle` | `R2` | `runtime verified` | `Reviewed - no issue found` | `No searched synthetic account, event, session, or token marker was found in 195 available artifact files.` | `Marker scan is not a general DLP proof; artifact files are local evidence and will be removed from the worktree.` |
| `A9` | `Future Live activation checklist and owner gates` | `docs/security/private-google-calendar-live-activation-checklist.md` | `Coordinator` | `contract trace` | `Reviewed - no issue found` | `The new cleartext statement matches the isolated-only manifest; all future Cloud, OAuth, signing, and real HTTP prerequisites remain separate Owner decisions.` | `No future activation or real access was performed.` |
| `A10` | `Frozen generation-0 review artifact` | `docs/reviews/2026-11-19-code-review-report-cr-20261119-92d7c4e1-g0.md` | `Coordinator` | `contract trace` | `Not review-relevant` | `This file is the immutable parent review artifact, read in full and validated; no application behavior is introduced by adding it to the PR.` | `Do not rewrite the frozen report.` |

## Subagent Candidate Adjudication

| Candidate ID | Proposed by | Decision | Final ID | Coordinator evidence | Reason |
| --- | --- | --- | --- | --- | --- |
| `R1-C1` | `R1` | `accepted` | `F1` | `Confirmed the same base URL flows through openPrimaryGoogleReadOnly into a transport constructor that checks the mismatched host set.` | `The downstream check makes the canonical IPv6 composition acceptance incomplete; failure is isolated and fail-closed.` |
| `R2-C1` | `R2` | `accepted` | `T1` | `Verified assertCountEquals(2) checks cardinality only, while the displayed-node assertions target the label and formatted date.` | `This is a narrow assertion-strength gap, not evidence of incorrect rendered UI.` |
| `R1-D1` | `R1` | `dismissed` | `None` | `Isolated manifest alone selects the exception; false base policy, exact loopback entries, and example.com rejection are explicit.` | `No product cleartext or arbitrary-host broadening.` |
| `R1-D2` | `R1` | `dismissed` | `None` | `Product BuildConfig remains false; product ViewModel construction supplies no Gateway; composition rejects non-isolated Live selection.` | `No current product HTTP activation route.` |
| `R1-D3` | `R1` | `dismissed` | `None` | `Primary events-only scope and preflight/nonprimary rejection are unchanged and covered by direct source trace.` | `No scope or resource expansion in the delta.` |
| `R2-D1` | `R2` | `dismissed` | `None` | `The UI test asserts exact Primary GET paths, methods, pagination, loopback host, no CalendarList/Gmail, and the events-read-only scope.` | `The E2E request-boundary assertion is sufficiently explicit for its IPv4/hostname fixture path.` |
| `R2-D2` | `R2` | `dismissed` | `None` | `Focused stays 47 exact; Formal stays 105 exact with 103+2; only the isolated composition identity and non-required count increase to match added tests.` | `No test threshold was weakened.` |
| `R2-D3` | `R2` | `dismissed` | `None` | `Scanned 195 target-SHA artifact files for listed synthetic fixture and token markers; no matches.` | `No evidence of fixture exposure; retain the marker-scan limit as a blind spot.` |

## Evidence Appendix

### Diff Inventory

| File or area | Classification | Semantic review area considered |
| --- | --- | --- |
| `.github/workflows/android-functional-focused.yml` | `config` | Exact isolated composition JUnit identity and count |
| `android/app/src/androidTest/java/app/serea/android/PrivateGoogleCalendarLoopbackUiTest.kt` | `test-only` | Private authorization, loopback request assertions, timezone detail, session lifecycle |
| `android/app/src/isolated/AndroidManifest.xml` | `config` | Isolated-only network security config wiring |
| `android/app/src/isolated/res/xml/loopback_network_security_config.xml` | `config` | Loopback-only cleartext exception; false base policy |
| `android/app/src/main/java/app/serea/android/googleauth/PrivateGoogleAuthorizationViewModel.kt` | `surface` | Test Gateway injection boundary |
| `android/app/src/main/java/app/serea/android/googleauth/PrivateGoogleCalendarComposition.kt` | `surface` | Canonical loopback host selection and test composition |
| `android/app/src/testDebug/java/app/serea/android/googleauth/PrivateGoogleAuthorizationViewModelTest.kt` | `test-only` | Debug rejection of synthetic Gateway injection |
| `android/app/src/testIsolated/java/app/serea/android/googleauth/PrivateGoogleCalendarCompositionTest.kt` | `test-only` | Isolated endpoint composition contract |
| `docs/reviews/2026-11-19-code-review-report-cr-20261119-92d7c4e1-g0.md` | `docs-only` | Immutable generation-0 review input |
| `docs/security/private-google-calendar-live-activation-checklist.md` | `docs-only` | Owner-only prerequisites and isolated test networking statement |
| `tests/android_instrumentation_results_tests.py` | `test-only` | Exact non-required identity count and disjointness |

### Verification Commands

- `git --no-pager diff --stat 9dd7700...425adc2` -> 11 paths, 440 insertions / 9 deletions; normalized diff SHA-256 `497fc17746d43862dbd103372b4dc3a7976076a72321cbc9a8fa871e80eea094`.
- `python3 -m unittest discover -s tests -p android_instrumentation_results_tests.py -v` -> 13/13 passed.
- `python3 tools/validate_docs.py docs` -> passed after this report was added; 229 Markdown files scanned, including the resolution and generation-1 report.
- Source-SHA focused Android UI E2E -> API 26 47/47 and API 35 47/47, from run `44410627534`.
- Source-SHA Android Formal -> API 26 105/105 and API 35 105/105, from run `44410627969`.
- Provider unit evidence -> 310 tests, 0 failures/errors/skips; isolated composition 12/12; Beta HTTP gate 1/1, recorded for target SHA `425adc2`.
- `gh pr checks 245 --repo Yrika819/Serea --json name,state,workflow,link` -> all four Required contexts successful: Linux fast checks, Linux stable full validation, Android Emulator Required Gate, Linux MSRV 1.89.0.
- Bounded artifact scan -> 195 files under `.ci-artifacts/44410627534`; no matches for listed synthetic fixture/account/session/token markers, `Bearer`, `access_token`, or `refresh_token`.
- Static IPv6 trace -> composition host allowlist `::1`; downstream HTTP `TEST_HOSTS` contains `[::1]`; canonical IPv6 unit test does not open a provider session.
- `git diff --check` -> passed for the tracked baseline-to-target code delta; the new reports are to be staged and checked before commit.
- Non-required `Serea API 26/35 Real IME B/C (Diagnostic, Non-Required)` -> failed at unrelated source-scope allowlist check before build/APK execution; classified `NOT_DELIVERED`, not a target implementation failure or host infrastructure failure.

### Supporting Code Links

| ID | Role | Link | Why it matters |
| --- | --- | --- | --- |
| `F1` | `composition` | [`PrivateGoogleCalendarComposition.kt`](../../android/app/src/main/java/app/serea/android/googleauth/PrivateGoogleCalendarComposition.kt#L89) | Canonical `::1` is admitted at the composition boundary |
| `F1` | `transport` | [`GoogleCalendarHttpReadPort.kt`](../../android/app/src/main/java/app/serea/android/calendar/transport/GoogleCalendarHttpReadPort.kt#L83) | Downstream URL validation is applied when the provider session is built |
| `F1` | `allowlist` | [`GoogleCalendarHttpReadPort.kt`](../../android/app/src/main/java/app/serea/android/calendar/transport/GoogleCalendarHttpReadPort.kt#L407) | The HTTP transport retains the bracketed IPv6 spelling |
| `F1` | `test` | [`PrivateGoogleCalendarCompositionTest.kt`](../../android/app/src/testIsolated/java/app/serea/android/googleauth/PrivateGoogleCalendarCompositionTest.kt#L230) | Current IPv6 test stops at composition construction |
| `T1` | `assertion` | [`PrivateGoogleCalendarLoopbackUiTest.kt`](../../android/app/src/androidTest/java/app/serea/android/PrivateGoogleCalendarLoopbackUiTest.kt#L177) | Count assertion does not assert both matching nodes displayed |
| `T1` | `renderer` | [`CalendarAgendaScreen.kt`](../../android/app/src/main/java/app/serea/android/calendar/ui/CalendarAgendaScreen.kt#L745) | Detail view renders both event and display timezone fields |

### Dismissed Coordinator Candidates

| Candidate | Decision | Evidence |
| --- | --- | --- |
| `Product builds can turn on Live HTTP through the new test seam` | `Dismissed` | `Test Gateway injection is isolated-only; product HTTP flags remain false; Live composition rejects non-isolated builds and requires a loopback URL.` |
| `The isolated manifest now permits arbitrary cleartext or external hosts` | `Dismissed` | `The config base is false, only exact loopback domains are allowed, and runtime NetworkSecurityPolicy assertion rejects example.com.` |
| `Primary-only or read-only scope was weakened` | `Dismissed` | `The delta touches no provider/transport permission logic; UI requests remain four Primary GETs and status/token scope assertions are exactly calendar.events.readonly.` |

### Blind Spots

| Area ID | Blind spot | Decision risk | What would resolve it |
| --- | --- | --- | --- |
| `A3` | `No full IPv6 MockWebServer provider-open/request test exists; the lower host mismatch is static and fail-closed.` | `IPv6-only test environments cannot use the accepted composition URL; this is captured as F1, not an unreviewed surface.` | `Align the lower allowlist and test openAuthorizedPrimary through a request received by an IPv6 MockWebServer.` |
| `A8` | `Artifact review checks explicit fixture and token markers only, not arbitrary encodings or all semantic data.` | `A zero marker count is bounded evidence, not general DLP proof.` | `Run the repository-wide secret/artifact audit on final-SHA artifacts and retain sanitized output.` |
| `A5` | `Source-SHA CI runs test UI behavior, but the assertion does not establish individual visibility of both equal timezone values.` | `A regression hiding one timezone value might not be independently detected.` | `Add displayed assertions for both matching nodes, ideally scoped to the detail container.` |

## Prior Resolution Reconciliation

| Issue key | Issue fingerprint | Parent item/verdict | Relevant change or new evidence | Decision |
| --- | --- | --- | --- | --- |
| `behavior; entry=isolated Private Calendar loopback provider request; contract=MockWebServer HTTP is allowed only for loopback in isolated instrumentation while product cleartext remains denied; effect=the UI E2E cannot reach its deterministic Calendar responses` | `ifp-sha256:59e666842d76a0d07d0b5a5635501cee590a82cf1d02c1dc5578caa1a2f5c1aa` | `F1 Fixed` | `None` | `kept closed` |
| `behavior; entry=Android instrumentation identity regression test; contract=the explicit non-required test identity manifest count stays synchronized with the added cases; effect=Android Unit CI fails and the Required Android gate remains rejected` | `ifp-sha256:feb32c8b283409532ad5ee7d1b0ae24d49e6dfa0e3de48ff4466e6f3b0d59268` | `F2 Fixed` | `None` | `kept closed` |
| `behavior; entry=isolated loopback endpoint validation; contract=valid IPv6 loopback hosts are accepted consistently by composition and request verification; effect=IPv6 MockWebServer paths are rejected before provider traffic` | `ifp-sha256:93adac8397dcb76e16d749459060f8e226196978affc02b26049bd71d87027ce` | `F3 Fixed` | `kind:code; ref:PrivateGoogleCalendarComposition.kt and PrivateGoogleCalendarCompositionTest.kt; change:composition now admits canonical ::1 but the directly invoked HTTP port still compares the canonical host against TEST_HOSTS containing only the bracketed IPv6 spelling` | `reopened as F1 because the downstream provider-construction check remains incompatible` |
| `behavior; entry=Private Google Calendar ViewModel gateway injection; contract=synthetic authorization gateways are injectable only in the isolated test build; effect=product code can inject fake authorization state despite test-only boundary` | `ifp-sha256:4e9080c588878bc42b0336f4d699729dec9de007082f6fa0cc6642f80c0069ff` | `F4 Fixed` | `None` | `kept closed` |

## Receiving Handoff

- Handoff status: `Terminal post-review - return to user/owner`
- Automatic receiving permitted: `No`
- Source report ID: `cr-20261119-3a6fbc82`
- Scope fingerprint to recheck: `sha256:8f6fe291dd47d6c38196476c6d4ec05b466fc25bd0e34328beec79add9411712`
- Actionable finding IDs: `F1`
- Deferred finding IDs: `None`
- Actionable test-gap IDs: `T1`
- Deferred test-gap IDs: `None`
- Open question IDs: `None`
- Open coverage area IDs: `None`
- Highest-risk verification to repeat: `Run the isolated IPv6 provider-construction/request test after aligning the transport host allowlist; recheck product HTTP-off and Public/Private isolation on that exact SHA.`
- Suggested implementation boundaries: `If approved for follow-up, change only the HTTP transport's canonical loopback comparison and its isolated provider-opening test; separately strengthen the timezone semantics assertions. Do not change product HTTP gates, OAuth, scope, Cloud, signing, or external access.`
- Re-review note: `This generation-1 report is terminal. No automatic receiving-code-review cycle is permitted; return remaining items to the Owner.`
- Chain rule: `Generation 1 is terminal. Any later implementation/review requires a new explicit Owner request and a new generation-0 review chain.`

## Report Self-Check

- `yes` Actual assessment mode and rationale are recorded: coordinator assessment and two bounded parallel specialists.
- `yes` Every changed review-relevant or unknown-impact area appears once in the Review Coverage Ledger.
- `yes` Every final finding appears once in the index and once as a matching card.
- `yes` Every Finding F# area references an existing finding.
- `yes` The standalone test gap has a stable ID and severity.
- `yes` Every F# and T# has a unique semantic fingerprint and an authoritative expected basis.
- `yes` Generation, trigger, parent review, parent resolution, and terminal handoff satisfy the lineage contract.
- `yes` Every relevant parent disposition is reconciled; reopened F3 uses concrete changed-code evidence and preserves its semantic fingerprint.
- `yes` Every non-Question finding and test gap is partitioned exactly once; there are no Questions or Not-covered coverage areas.
- `yes` Every meaningful specialist candidate is accepted or adjudicated with coordinator evidence.
- `yes` Remaining blind spots state concrete limits and next verification.
- `yes` Recommendation follows the mapping: Minor finding and Minor test gap -> Pass with caveat.
- `yes` Generation-1 report validator and final documentation validation passed.
- `yes` Git state was not mutated during this review.
