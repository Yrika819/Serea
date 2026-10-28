# G7 Readiness Ledger — Calendar OAuth Mock E2E, Local AI and Release Decisions

**Checkpoint:** 2026-10-28 UTC  
**Roadmap:** Owner-provided `Serea_Roadmap_v4.md`, read from the supplied attachment  
**G6 authority:** candidate PR #138, source `23b884cf2815a2f3287652d5ffd2c3913d09df76`, tree `f77fea2f7800dbf8daa1f5f25175c52f951f31a4`  
**G5 frozen baseline:** PR #134, source `b84feb337cd57918346ab31cb3ea4af4ba880ffb`, tree `9133d4a3ba2ab870d126e56cdd22336b856c94e1`  
**Protected `main`:** `e0acb0254820c381dd97828824bef86a1db5176c` — unchanged  
**G7 branch:** `codex/serea-g7-calendar-readiness`, based on the exact G6 candidate above  
**Status:** PARTIAL — readiness preparation is underway; targeted G7 JVM result and independent review are pending. No live service or production action was performed.

## G6 handoff

G6 is recorded as **SUCCESS — Functional Readiness Candidate** on exact candidate SHA `23b884c…`. Its same-SHA Formal Android run `40710445577` passed 11/11 jobs, including API 26 and API 35 Formal Required 105/105 each, Identity, Process Death, Data Isolation, and Sentinel. Focused UI, Android Contract, Full, Fast, Native, SQLite, RustSec, Git Hooks, and Release Security evidence is recorded in PR #138. Personal Beta remains **NO-GO**; this candidate is not merged.

The supplied v4 schedule was read. The next decision gate is 2026-11-10. Its G7 work includes Calendar-first Mock E2E, Owner OAuth setup material, AI adapter acceptance, provider conditions, UI/Usage contracts, and a limited CI efficiency trial. The earlier 2026-11-03 Package ID decision remains Owner-owned and unresolved in this preparation branch.

## Calendar OAuth and read-only Mock E2E

### Current implementation boundary

- `PlayServicesGoogleAuthorizationGateway` wraps Play Services AuthorizationClient. Its `realOAuthEnabled` constructor default is false, authorization is foreground/user-initiated, and an explicitly selected Android account is required.
- `CalendarProviderSessions.openDefault()` selects Fake. The Google HTTP session additionally requires the Owner activation gate and a gateway whose mode is `PLAY_SERVICES`.
- `GoogleCalendarAuthorizationAdapter` keeps `calendar.events.readonly` as the event-read scope and treats `calendar.calendarlist.readonly` as optional for calendar discovery. The Fake gateway cannot authorize the Google transport.
- `SereaAppShell` and `AppProviderComposition.fakeDefault()` still select Fake providers. The Play Services authorization gateway is not wired into the ordinary application UI or product composition. A production OAuth flow is not implemented or enabled by this G7 candidate.
- Calendar HTTP has read methods only, uses a fixed Google API origin in production, disallows redirects and automatic retry, and admits only loopback HTTP for tests.

### G7 test and existing coverage

Added `CalendarOAuthMockE2ETest.syntheticConsentAllowsOnlyCalendarReadAndAccountSwitchStopsFurtherDispatch`. It composes the Calendar provider session, shared Google authorization adapter, synthetic authorization fixture, and a loopback MockWebServer. The fixture uses a synthetic account/session and a deliberately invalid test token. The test requires authorization before any token or HTTP use, grants only the Calendar events read scope through an in-test consent action, performs one GET against the loopback Calendar endpoint, validates the parsed event and timezone, and rejects a read after a synthetic account switch without a second dispatch.

This is a contract test for the provider-to-Calendar boundary. It does not call Play Services, show Google's consent UI, obtain an OAuth token, reach `googleapis.com`, use an account, or exercise the production Activity Result flow.

The existing focused suites separately cover:

- Fake and disabled-owner fallback to the Fake provider.
- Denial and expiry before HTTP dispatch; account switch during token acquisition.
- Session replacement, authorization revocation, delayed HTTP callbacks, and cancellation.
- 401, missing-scope 403, quota 403/429, 404, 410, 5xx, no automatic retry, response-body redaction, and redirect token stripping.
- Scope mismatch and malformed/oversized response rejection.

### Calendar-first activation plan (not executed)

1. Owner ratifies the stable Beta package ID and the matching release certificate. Derive certificate SHA-1 and SHA-256 from the same approved certificate; do not substitute a CI or temporary key.
2. Owner creates or selects a dedicated Google Cloud project, enables Calendar API only, configures an Android OAuth client for the approved package/certificate pair, and completes the consent-screen, privacy disclosure, tester and verification review.
3. A separately reviewed Android UI selects the account, explains the exact Calendar scope and purpose, launches the one-shot AuthorizationClient resolution, and handles cancel, denial, recreation, process death, disconnect and revoke. No startup authorization.
4. The authorized provider session binds the selected opaque account identity and session generation. Account replacement synchronously invalidates requests and old results before delivery. Cache and pending callbacks are cleared on disconnect.
5. With an Owner-controlled test account, test only read-only Calendar list/event/today/tomorrow/range paths, timezone and all-day cases, 401/403/429, expiry, account switch, cancellation, revoke and reconnect. No write endpoint, Gmail scope or Cloud model is enabled.
6. Keep live acceptance local to an Owner-controlled environment. Public CI remains synthetic and must not receive tokens, account identifiers, event bodies or screenshots.

**Open production gap:** account selection and consent-result UI/composition are not connected. A green Mock E2E is not evidence of Google OAuth acceptance. The real Calendar connection stays `NOT_RUN` until separate Owner approval and a reviewed UI integration.

## UI and Usage contract for a future local model

- Provider connection and Chat handoff are separate choices. A Google connection grant does not authorize Chat use. Each Chat handoff must show the selected source item, origin, purpose, selected content, destination model identity, local/offline destination, expiry and cancel/confirm actions.
- The G6 handoff consent is explicitly local-only. It never grants Cloud Egress. Cloud transfer needs a separate review and consent contract binding exact destination, content, purpose, provider terms and expiry.
- The G6 adapter reports the distinct model identity `serea-local/SmolLM2-135M-Q4_K_M` and provider `local-device`; the Fake native backend is a test double, not a runnable model. Normal product Chat remains on its current Demo route until a concrete native backend and acceptance gates pass.
- The UI must identify Demo, local model, and Cloud provider as distinct modes. It must never label Fake output as real model output or silently fall back from local inference to Cloud.
- Usage records model/provider identity and token counts when known. Unknown prices remain `UNKNOWN`, not `$0`; local energy/thermal cost is not currently accounted for. Reservations must be released for a known pre-inference cancellation and remain outcome-unknown after native generation may have started.
- Before enablement: verify encrypted conversation history, deletion, memory/token limits, user-visible cancellation, native resource release, crash recovery, offline enforcement and model hash/license. No model binary is added to Git.

**Adapter integration gap:** the G6 `LocalModelSecurity` proof currently accepts `authorizationState == MOCK_ONLY`, matching the Fake handoff. A real authorized Calendar handoff will require a separately reviewed proof/schema update and tests; the current adapter candidate must not be interpreted as accepting real Google-origin data.

## Provider conditions review (2026-10-28 snapshot)

The Android Cloud adapter is fixed to `https://ollama.com/api/chat`, but Cloud is disabled by default and Google-origin lineage is rejected before credential or socket use. No provider account, key, API call or billing was used for this review.

| Candidate | Evidence reviewed | G7 disposition |
|---|---|---|
| On-device SmolLM2-135M Q4_K_M | G6 B pinned model metadata declares Apache-2.0; llama.cpp pinned source declares MIT. The complete linked-native dependency notices/SBOM are not closed. ARM64 runtime and Pixel 7a performance are NOT_RUN. | Preferred privacy direction, but redistribution and product acceptance remain blocked. Keep model outside Git and do not display it as runnable product Chat. |
| Ollama Cloud | [Terms of Service](https://ollama.com/terms) page reports “Last updated: May 2026”; section 2 says users must be at least 18. The page says prompts/outputs are not used to train models and grants a limited license to process content to provide the service. [Privacy Policy](https://ollama.com/privacy) says cloud prompts/responses are processed transiently, with account/basic usage metadata collected; it describes transfer/retention and deletion terms that need Owner/jurisdiction review. | Not approved. Age eligibility, region, account, service tier, current retention, cost, and provider-specific data handling need an Owner decision. No Cloud key/API use. Google-origin content remains blocked regardless of this preliminary provider review. |
| Other hosted model providers | No service or current terms were selected for this G7 packet. | Not evaluated and not a candidate. Do not infer eligibility from consumer chat-product terms. |

The Ollama Terms page's 18+ requirement is the conservative gate for its service; a different age statement in privacy material does not override service Terms. The page is a dated snapshot, not a legal determination. Recheck the current provider terms, age, jurisdiction, data-processing, retention, cost, abuse and deletion conditions immediately before any separately authorized activation.

Google's [API Services User Data Policy](https://developers.google.com/terms/api-services-user-data-policy) states that use/transfer of Google user data is limited to providing or improving prominent user-facing features with user consent, with additional requirements for sensitive/restricted scopes and applicable verification/security assessments. That policy does not make this app's local-only handoff a Cloud consent, and this packet does not determine legal or Google verification eligibility. Keep Google→Cloud disabled.

## Limited CI efficiency trial

Added `.github/workflows/android-g7-calendar-oauth-mock-focused.yml` as a **Non-Required** workflow. For a PR targeting the G6 integration branch, it checks out the exact head SHA and runs only the one synthetic Calendar OAuth-to-Mock HTTP JVM test, then requires the exact JUnit case with zero failures/errors/skips and uploads the JUnit output. It uses read-only repository permission, no secrets, no emulator, and no external endpoint.

The trial does not alter Android Required, branch protection, Formal manifest/assertion/failure/skip conditions, main-only Beta workflow, or any existing workflow. It is intended to provide a quick, narrow signal while the normal Android Required workflow still runs for Android changes. Its CI execution and timings are pending; no efficiency gain is claimed yet. Compare focused job queue/runtime with the Android JVM/Required jobs and remove or revise the experiment if it adds cost without shortening useful feedback.

## Change and verification status

```text
AGENT_ID: Parent Orchestrator
ACTIVE_PHASE: G7 Readiness Preparation
GOAL_STATUS: PARTIAL — Mock E2E and limited workflow drafted; review and exact-head CI pending
BASE_SHA: 23b884cf2815a2f3287652d5ffd2c3913d09df76
CURRENT_HEAD: pending local commit
TREE_SHA: pending local commit
WORKTREE: /workspace/Serea-g7-calendar-readiness
BRANCH: codex/serea-g7-calendar-readiness
DRAFT_PR: not created yet; independent review pending
COMPLETED_WORK: read supplied roadmap v4; inspected G6 OAuth, Calendar, handoff, local adapter and release readiness evidence; added synthetic Calendar mock E2E and non-required focused CI trial; reviewed current Ollama Terms/Privacy and Google User Data Policy
CHANGED_FILES: android/app/src/test/java/app/serea/android/calendar/CalendarOAuthMockE2ETest.kt; .github/workflows/android-g7-calendar-oauth-mock-focused.yml; docs/agent-reports/g7-orchestrator-ledger.md
TEST_COMMANDS: `python3 tools/validate_docs.py docs`; `git diff --check`; targeted Gradle JVM run attempted but wrapper download failed with `ConnectException: Connection refused`
JUNIT_RESULT: NOT_RUN locally; GitHub CI pending
CI_RUN_IDS: none for G7 yet
ARTIFACT_IDS: none
SECURITY_IMPACT: no production code, credential, OAuth configuration or live endpoint changed; new test uses a synthetic token and loopback-only MockWebServer
DEPENDENCIES: independent review; exact-head G7 CI; Owner O-01 Package ID, O-02 OAuth configuration, O-03 AI/provider eligibility decisions
KNOWN_ISSUES: Google Activity Result/UI composition remains unwired; LocalModelSecurity only accepts MOCK_ONLY handoffs; ARM64 runtime, Pixel 7a speed/thermal acceptance and complete native dependency notices remain open
CURRENT_BLOCKERS: local Gradle distribution could not be downloaded; no GitHub CI has run for this G7 head
INTEGRATION_READY: no — review and CI pending
REVIEW_REQUEST: independent Source/Test/Security/Integration review of the synthetic OAuth test, non-required workflow and provider/adapter contract
NEXT_ACTION: run documentation checks, obtain independent review, push reviewed Draft PR, inspect focused and normal exact-head CI
OWNER_APPROVAL_REQUIRED: real Google OAuth/account/data; Cloud AI API key/billing/provider activation; production signing/distribution; Pixel 7a operation; main merge; Personal Beta GO
```

## Owner decisions and release status

| Decision | Target | Status at this checkpoint |
|---|---:|---|
| O-01 Package ID and Demo/Beta coexistence | 2026-11-03 | Owner decision pending; current code expects Beta `app.serea.android.beta` |
| O-02 Google Cloud project/client/package/certificate/scope | 2026-11-10 | Checklist prepared; no project/client/account created |
| O-03 AI model/provider/age/region/terms/cost/retention | 2026-11-10 | Preliminary Ollama terms review recorded; no provider approved |
| O-04 Google-data transfer scope/destination/consent | before 2026-12-01 | Google-origin Cloud transfer disabled; separate approval remains necessary |
| O-05 Owner production key and distribution | before signing | no key created or used |
| O-06 Pixel 7a acceptance date/scope | November 2026 | device operation not performed |
| O-07 main integration | RC review | main unchanged; G6/G7 candidates remain Draft |
| O-08 Personal Beta decision | 2026-12-31 | NO-GO until all mandatory P0 evidence and Owner decision exist |

## G7 gate outlook

G7 preparation is not the 2026-11-10 release decision. Before that gate, close the mock CI/review evidence, complete the Owner OAuth decision packet with any required package/certificate inputs, and update the AI acceptance matrix. If Owner decisions or device/service approvals are unavailable, keep corresponding real-service/device items `NOT_RUN`, maintain Fake/Local operation, and report `PARTIAL`; do not relax the G6 Security Gates.
