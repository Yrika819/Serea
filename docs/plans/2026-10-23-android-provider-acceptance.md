# Android Personal Beta provider acceptance

## Source and ownership

- Repository: `Yrika819/Serea`.
- Provider worktree base: PR #98 fixed HEAD `00e0453f83970a2b7d2bd26ce01462e421f75f46` (base `e0acb0254820c381dd97828824bef86a1db5176c`).
- PR #99 is a gate-recovery draft with head `8140bd759e7a01041364ebc443ba452f546d325a`, based on PR #98's fixed head above.
- PR #100 is a separate gate-recovery draft based on #99. Keep this Provider branch on the pinned PR #98 base until #100's latest exact-head campaign is green.
- This work changes Provider Domain, Adapter, Authorization, Session, Egress tests, and this report. It does not modify PR #98/#99/#100, Android UI, Calendar instrumentation, CI harnesses, or native lifecycle code.
- No real Google account, Calendar, Gmail, Cloud AI, OAuth credential, or owner-managed Google/Firebase setting was used.

## Capability matrix

| Capability | Domain | HTTP adapter | Auth/provider wiring | Android UI reachability | Fake/Mock acceptance | Real Provider E2E |
|---|---|---|---|---|---|---|
| Gmail read | Implemented: inbox/search/page, message and thread domain, bounded MIME parser | Implemented: `GmailHttpReadPort`, GET-only, `gmail.readonly` | Structure exists. OAuth is off by default; Owner gate and a pinned Android account session are required. No OAuth project/client or consent configuration is present. | Fake mailbox only; app shell constructs `FakeGmailProvider` directly. | MockWebServer tests exercise the production HTTP adapter; session tests cover the Owner gate, account binding, concurrent cancellation, stale-response denial, and account replacement through lease acquisition. | **NOT RUN / BLOCKED** on Owner OAuth setup and separate authorized-account acceptance. |
| Calendar read | Implemented: Today/Tomorrow/date/range, event recurrence identity, all-day and timed events | Implemented: `GoogleCalendarHttpReadPort`, GET-only, bounded pagination and payloads | Structure exists. OAuth is off by default; Owner gate and a pinned Android account session are required. No OAuth project/client or consent configuration is present. | Fake calendar only; app shell constructs `FakeCalendarProvider` directly. | MockWebServer tests exercise the production HTTP adapter; session tests cover default Fake, read scope, account binding, isolated cancellation, and account replacement through lease acquisition. | **NOT RUN / BLOCKED** on Owner OAuth setup and separate authorized-account acceptance. |
| AI chat | Implemented: local demo chat, model routing, typed egress contract, durable usage ledger, encrypted conversation history | Ollama Cloud HTTP adapter exists as a gated candidate; it is not the app's active chat transport | Candidate composition and credential path exist, but the active app uses `MockModelChatPort`; `CloudSettings.enabled` defaults false and the selected model defaults empty. Provider qualification and Owner approval are absent. | Demo chat only; no real AI provider is reachable from the app UI. | Mock HTTP tests cover request/response, multi-turn payloads, error handling, limits, and usage. Egress tests assert Gmail/Calendar-origin context produces zero credential reads and zero network calls. | **NOT RUN / BLOCKED** on Owner provider, age eligibility, terms, model, spending, credential, and data-egress decisions. |

`TaskProposal`, its encrypted repository, and its isolated demo Fake are present in PR #98. They do not connect to Gmail, Calendar, or an external AI provider.

## Implemented provider safeguards

- Google scopes remain least-privilege: `gmail.readonly`; `calendar.events.readonly` for event reads; and `calendar.calendarlist.readonly` only for calendar discovery. Google currently classifies `gmail.readonly` as restricted. Public use requires the applicable OAuth verification; transmitting or server-storing restricted data can require a security assessment. See [Gmail scopes](https://developers.google.com/workspace/gmail/api/auth/scopes), [Google Workspace user data policy](https://developers.google.com/workspace/workspace-api-user-data-developer-policy), and [Calendar scopes](https://developers.google.com/workspace/calendar/api/auth).
- The Play Services gateway now takes an explicitly selected Android `Account`, applies it to each authorization request, and issues tokens with an opaque, process-local authorization-session binding. It does not persist or log an account name. No account means no active real grant. A new account requires a new gateway and replacement provider session, which invalidates in-flight reads from the prior session. Gmail and Calendar token adapters recheck account binding and feature enablement after token acquisition and before dispatch.
- Gmail now has a `GmailProviderSessions` boundary matching Calendar's default-Fake/opt-in-real behavior. Real HTTP requires the Owner gate and a Play Services authorization gateway. A Fake gateway cannot select the HTTP adapter.
- Gmail and Calendar session teardown cancels each read's private child `Job`, not the caller's coroutine job. The final result-delivery gate rejects a response when replacement closed the old session first. Provider cancellation is mapped to a typed authorization denial while ordinary caller cancellation still propagates.
- Gmail and Calendar read data retain their source classification. Neither provider is connected to the AI request builder. The egress gate denies `GMAIL` and `CALENDAR` origins before credentials or a socket are used, even when the context is labeled `PUBLIC`.
- Gmail MIME content is parsed as bounded text. HTML markup and active/external content are not rendered or fetched by the transport. Provider errors contain stable codes and do not copy response bodies or message content into ordinary logs.
- AI remains off. The existing HTTP candidate is Ollama Cloud; its current [Terms of Service](https://ollama.com/terms) require the user to be at least 18. Beta user eligibility is not established, so no age condition or provider restriction is bypassed and no provider is activated.

## Test and audit evidence

Earlier provider validation passed 115 focused tests across nine suites. After the session-cancellation and post-lease binding review, the following six focused suites passed again:

- `GmailProviderSessionsTest`: two concurrent reads, parent/sibling preservation, caller cancellation, stale response rejection after session replacement, and account replacement immediately after lease acquisition with zero HTTP dispatch.
- `GmailHttpReadPortTest`: post-acquisition account binding, read-only Gmail scope, HTTP failure handling, and bounded response behavior.
- `CalendarProviderSessionsTest`: caller/sibling preservation and account/session replacement during and immediately after token acquisition with zero HTTP dispatch.
- `GoogleCalendarHttpReadPortTest`: post-acquisition account binding, pagination, timezones, and bounded HTTP handling.
- `OllamaCloudAdapterTest` and `SecurityPortsTest`: Gmail/Calendar-origin egress denial before credential read or network dispatch.

Result: **76 tests, 0 failures, 0 errors, 0 skips** in the latest focused run. `:app:lintDebug` and `git diff --check` also passed on the final implementation. Real OAuth, real Google API, and real Cloud AI remain **NOT RUN**. Android instrumentation is tracked independently below; its passing process-death check uses only synthetic data and does not constitute real-provider acceptance.

## PR #102 fixed-HEAD CI evidence

Initial campaign was collected on PR #102 HEAD `1e50524b45d12c54e999a4f5b6a7f4a24444b98f`, before the concurrency follow-up commit:

| Workflow | Run | Result |
|---|---:|---|
| Fast CI | 601 | PASS |
| Git hooks Windows compatibility | 339 | PASS |
| Rust Dependency Security | 24 | PASS |
| Full CI | 590 | PASS |
| Cross-architecture SQLite portability | 568 | PASS |
| Native platform validation | 390 | PASS |
| Android | 271 | FAIL, base failures below |

Android Change Scope, Integration, Dependency Audit, Build/Lint/Instrumentation Compile, Documentation, Unit, Security Tests, and the Pixel 7a Reacceptance Bundle passed. Personal Beta Identity Isolation was skipped because the emulator gate failed. Android Storage and Emulator and Android Emulator Required Gate failed.

The Android API 35 instrumentation failures were `SereaAppShellTest.calendarDetailAndSelectedCalendarSurviveAnotherDestination` (component not displayed) and `UsageLedgerDatabaseTest.fakeProviderBridgeRoutesApprovedDataAndKeepsCostAndUnknownOutcomesDurable` (`UsageLimitExceededException`). The instrumentation verifier also found 13 `UsageLedgerDatabaseTest` methods where its unchanged contract expects 12. API 26 instrumentation evidence is **NOT_RUN** after the verifier failure; API 26 and API 35 process-death evidence passed with synthetic data.

These Android failures were present on the exact PR #102 base SHA `00e0453f83970a2b7d2bd26ce01462e421f75f46`: base Full CI run 571 and SQLite run 549 passed; base Android run 252 had the same two API 35 test failures and the same test-count mismatch. Base Native run 371 failed only on the Windows ARM64 Rust 1.85 test `model_accounting_is_bound_to_a_task_created_by_the_scheduler_task_engine` (host exited before readiness); the full Native campaign on PR #102 passed all targets. This Provider diff has no path overlap with PR #100's Android UI/instrumentation/native files or PR #101's release/signing/supply-chain files.

## Owner decisions and remaining real-account work

1. Configure the official Android OAuth client for the final application ID and signing-certificate fingerprint; enable the required APIs; decide consent-screen audience, publication/testing state, test users, privacy disclosures, verification, retention, and Google data-use obligations.
2. Approve Gmail `gmail.readonly` use after reviewing its restricted-scope requirements and the app's processing, storage, and egress behavior.
3. Select an account-selection UX and ensure switching the selected account replaces both Gmail and Calendar sessions. The UI does not yet expose this flow.
4. For AI, decide whether the existing Ollama candidate is eligible for this beta. If so, explicitly approve 18+ eligibility, provider terms/privacy, model allowlist, supported regions, retention/data use, credential custody, and cost/request/token budgets. Otherwise select no cloud provider; do not route around provider terms.
5. After Owner configuration, run separate test-account acceptance for Authorized → Active, revoke → Denied, expiry → Reauthorization Required, account switch → no data mixing, provider errors, unknown authorization → fail closed, and cancellation. These checks must use the production adapters with a test account and must not be inferred from MockWebServer results.

## Integration after PR #100 stabilizes

Keep this provider draft based on the pinned PR #98 candidate while #99/#100 are still being validated. Wait for PR #100's latest exact-head API 26/35, Data Isolation, Sentinel, Process Death, and ProfileVerifier evidence and its required CI to finish. Then integrate the provider commit on a new descendant of the accepted stack (without force-pushing or editing #98/#99/#100), resolve any moved-base conflicts, and rerun the required checks on that exact descendant. Merge only after both provider review and the unchanged Android/security gates pass.
