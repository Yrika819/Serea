# Android Personal Beta provider acceptance

## Source and ownership

- Repository: `Yrika819/Serea`.
- Provider worktree base: PR #98 fixed HEAD `00e0453f83970a2b7d2bd26ce01462e421f75f46` (base `e0acb0254820c381dd97828824bef86a1db5176c`).
- PR #99 is a gate-recovery draft with head `8140bd759e7a01041364ebc443ba452f546d325a`, based on PR #98's fixed head above.
- PR #100 is a gate-recovery draft based on #99. Its head moved from `9dbf5df768a2aeef8f68e2da8e60b2556f310952` to `497f06fbc604a527c870960092d59176d81e39ae` during this review; the latest inspected head is `497f06f`.
- This work changes Provider Domain, Adapter, Authorization, Session, Egress tests, and this report. It does not modify PR #98/#99/#100, Android UI, Calendar instrumentation, CI harnesses, or native lifecycle code.
- No real Google account, Calendar, Gmail, Cloud AI, OAuth credential, or owner-managed Google/Firebase setting was used.

## Capability matrix

| Capability | Domain | HTTP adapter | Auth/provider wiring | Android UI reachability | Fake/Mock acceptance | Real Provider E2E |
|---|---|---|---|---|---|---|
| Gmail read | Implemented: inbox/search/page, message and thread domain, bounded MIME parser | Implemented: `GmailHttpReadPort`, GET-only, `gmail.readonly` | Structure exists. OAuth is off by default; Owner gate and a pinned Android account session are required. No OAuth project/client or consent configuration is present. | Fake mailbox only; app shell constructs `FakeGmailProvider` directly. | MockWebServer tests exercise the production HTTP adapter; session tests cover the Owner gate, account binding, replacement, and account switch during a multi-request read. | **NOT RUN / BLOCKED** on Owner OAuth setup and separate authorized-account acceptance. |
| Calendar read | Implemented: Today/Tomorrow/date/range, event recurrence identity, all-day and timed events | Implemented: `GoogleCalendarHttpReadPort`, GET-only, bounded pagination and payloads | Structure exists. OAuth is off by default; Owner gate and a pinned Android account session are required. No OAuth project/client or consent configuration is present. | Fake calendar only; app shell constructs `FakeCalendarProvider` directly. | MockWebServer tests exercise the production HTTP adapter; session tests cover default Fake, read scope, account binding, replacement, and account switch during pagination. | **NOT RUN / BLOCKED** on Owner OAuth setup and separate authorized-account acceptance. |
| AI chat | Implemented: local demo chat, model routing, typed egress contract, durable usage ledger, encrypted conversation history | Ollama Cloud HTTP adapter exists as a gated candidate; it is not the app's active chat transport | Candidate composition and credential path exist, but the active app uses `MockModelChatPort`; `CloudSettings.enabled` defaults false and the selected model defaults empty. Provider qualification and Owner approval are absent. | Demo chat only; no real AI provider is reachable from the app UI. | Mock HTTP tests cover request/response, multi-turn payloads, error handling, limits, and usage. Egress tests assert Gmail/Calendar-origin context produces zero credential reads and zero network calls. | **NOT RUN / BLOCKED** on Owner provider, age eligibility, terms, model, spending, credential, and data-egress decisions. |

`TaskProposal`, its encrypted repository, and its isolated demo Fake are present in PR #98. They do not connect to Gmail, Calendar, or an external AI provider.

## Implemented provider safeguards

- Google scopes remain least-privilege: `gmail.readonly`; `calendar.events.readonly` for event reads; and `calendar.calendarlist.readonly` only for calendar discovery. Google currently classifies `gmail.readonly` as restricted. Public use requires the applicable OAuth verification; transmitting or server-storing restricted data can require a security assessment. See [Gmail scopes](https://developers.google.com/workspace/gmail/api/auth/scopes), [Google Workspace user data policy](https://developers.google.com/workspace/workspace-api-user-data-developer-policy), and [Calendar scopes](https://developers.google.com/workspace/calendar/api/auth).
- The Play Services gateway now takes an explicitly selected Android `Account`, applies it to each authorization request, and issues tokens with an opaque, process-local authorization-session binding. It does not persist or log an account name. No account means no active real grant. A new account requires a new gateway and replacement provider session, which invalidates in-flight reads from the prior session.
- Gmail now has a `GmailProviderSessions` boundary matching Calendar's default-Fake/opt-in-real behavior. Real HTTP requires the Owner gate and a Play Services authorization gateway. A Fake gateway cannot select the HTTP adapter.
- Gmail and Calendar read data retain their source classification. Neither provider is connected to the AI request builder. The egress gate denies `GMAIL` and `CALENDAR` origins before credentials or a socket are used, even when the context is labeled `PUBLIC`.
- Gmail MIME content is parsed as bounded text. HTML markup and active/external content are not rendered or fetched by the transport. Provider errors contain stable codes and do not copy response bodies or message content into ordinary logs.
- AI remains off. The existing HTTP candidate is Ollama Cloud; its current [Terms of Service](https://ollama.com/terms) require the user to be at least 18. Beta user eligibility is not established, so no age condition or provider restriction is bypassed and no provider is activated.

## Test and audit evidence

Focused unit tests passed on the provider branch for:

- `GoogleAuthorizationTest`
- `CalendarProviderSessionsTest`, `GoogleCalendarHttpReadPortTest`, and `CalendarDomainParserTest` (including recurrence exceptions and timezone handling)
- `GmailProviderSessionsTest`, `GmailHttpReadPortTest`, and `GmailSecurityPortsTest` (Gmail domain, MIME, read-only boundary, and safe handling)
- `OllamaCloudAdapterTest` and `SecurityPortsTest`

Result: **115 tests, 0 failures, 0 errors, 0 skips** across the nine suites above, run in two filtered Gradle invocations. `:app:lintDebug` and `git diff --check` also passed. Existing Android instrumentation `UsageLedgerDatabaseTest` exercises Fake-provider reservation/settlement, cancellation to `OUTCOME_UNKNOWN`, duplicate-send rejection, and database reopen recovery; it was not run in this provider-only local validation. That reopen test is not a real OS process-death test. Real OAuth, real Google API, real Cloud AI, device authorization, and Android instrumentation remain **NOT RUN**. PR #100 owns the Calendar instrumentation and Android lifecycle gates; this report does not change or replace those gates.

At the latest CI status check, PR #100's exact head `497f06f` still had Full CI and Native platform validation in progress, with Cross-architecture SQLite and Android Required Gate queued. Provider PR creation is deferred until this campaign completes so its workflows do not compete with those required checks.

## Owner decisions and remaining real-account work

1. Configure the official Android OAuth client for the final application ID and signing-certificate fingerprint; enable the required APIs; decide consent-screen audience, publication/testing state, test users, privacy disclosures, verification, retention, and Google data-use obligations.
2. Approve Gmail `gmail.readonly` use after reviewing its restricted-scope requirements and the app's processing, storage, and egress behavior.
3. Select an account-selection UX and ensure switching the selected account replaces both Gmail and Calendar sessions. The UI does not yet expose this flow.
4. For AI, decide whether the existing Ollama candidate is eligible for this beta. If so, explicitly approve 18+ eligibility, provider terms/privacy, model allowlist, supported regions, retention/data use, credential custody, and cost/request/token budgets. Otherwise select no cloud provider; do not route around provider terms.
5. After Owner configuration, run separate test-account acceptance for Authorized → Active, revoke → Denied, expiry → Reauthorization Required, account switch → no data mixing, provider errors, unknown authorization → fail closed, and cancellation. These checks must use the production adapters with a test account and must not be inferred from MockWebServer results.

## Integration after PR #100 stabilizes

Keep this provider draft based on the pinned PR #98 candidate while #99/#100 are still being validated. Wait for PR #100's final exact-head API 26/35, Data Isolation, Sentinel, Process Death, and ProfileVerifier evidence and its required CI to finish. Then integrate the provider commit on a new descendant of the accepted stack (without force-pushing or editing #98/#99/#100), resolve any moved-base conflicts, and rerun the required checks on that exact descendant. Merge only after both provider review and the unchanged Android/security gates pass.
