# H4-E2 Private Calendar Live Gate implementation record

**State:** implementation and mock verification only; the Private product HTTP gate remains `false`. This record does not authorize Google Cloud changes, real OAuth, Google Calendar HTTP, signing, APK distribution, or device access.

## Design outcome

H4-E2 retains the existing `privateBeta` build type rather than adding a Live-capable product variant. Adding a new variant or a true-valued build input during this phase would make it possible to emit a Live-enabled artifact before the separate H4-F Owner decision. `PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED` remains a checked-in literal `false` in the default, Beta, and Private Beta build configurations; the isolated test build also compiles it as false. No property, environment value, UI, Intent, Remote Config, or persisted preference changes this HTTP gate.

Product `PrivateGoogleCalendarComposition` no longer accepts a client, endpoint, OAuth state, or gate override. The compile-time product route policy selects Fake immediately when HTTP is off. If a future separately approved Private build enables HTTP, the policy fails closed unless the exact Private application ID/build, independent OAuth permission, Play Services gateway, enabled Calendar feature, active authorization session, and exact `calendar.events.readonly` grant are all present. A changed session is rejected before provider construction. Live errors do not select Fake.

Synthetic gateway and loopback MockWebServer injection are isolated in `src/isolated` test composition. This route requires an explicit loopback endpoint and is not callable through product composition. The product transport continues to use its fixed Google Calendar HTTPS endpoint. No true-valued product APK was built or made buildable by this change.

## Changed implementation

- `PrivateCalendarProviderRoutePolicy` contains product provider selection and the independent HTTP/OAuth checks.
- `PrivateGoogleCalendarComposition` has no test endpoint/client/gate override and delegates provider selection to compile-time configuration.
- `PrivateGoogleCalendarProviderComposition` is the shared lifecycle contract. Alternate composition factories are accepted only in `isolated` and rejected before invocation in product builds.
- `openGoogleSessionBoundToAuthorization` checks the selected session and Calendar-enabled state before and after construction; it closes a newly created session if the authorization changes.
- `IsolatedGoogleCalendarTestComposition` owns synthetic gateway, client, and loopback URL injection for MockWebServer tests only.
- Route, variant, composition, and ViewModel tests cover Fake while the gate is off, rejection before status/token/HTTP for ineligible configurations, session changes, and Live selection without real I/O.
- `PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED` was not changed from false.

## Local validation evidence

All Gradle runs used JDK 17 at `/Users/yuta/Library/Java/JavaVirtualMachines/temurin-17.jdk/Contents/Home`, Android SDK at `/Users/yuta/Library/Android/sdk`, `--no-daemon --max-workers=2`, and a per-run timeout no greater than 600 seconds.

| Check | Result | Notes |
|---|---|---|
| Policy-only test RED | Confirmed, 3m16s | Initial failure was unresolved expected route-policy symbols, before implementation; compilation failure was the intentional RED, not an assertion/environment failure. |
| Policy-only test GREEN | PASS, 7 tests, 1m29s | All route-policy cases passed. |
| Focused Debug regression | PASS, 96 tests, 2m30s | Earlier focused gate, ViewModel, provider, transport, lifecycle, and mock OAuth cases. |
| Full Debug unit suite | PASS, 496 tests, 1m | 0 failures/errors/skips on the final code revision. |
| Beta HTTP gate | PASS, 1 test, 1m15s | Final Beta variant test verifies HTTP and OAuth flags remain off. |
| Private Beta OAuth-off gate | PASS, 1 test, 6m10s | Final Private Beta unit-only invocation; no APK or bundle was built. |
| Private fake gateway suites with OAuth build input true | PASS, 10 tests, 1m44s | Consent suite (4) and GatewayOperations suite (6); fake operations only, no OAuth or HTTP. |
| Private Beta enabled-input whole task | 467 tests, 1 failed, 1m39s | `PrivateBuildOAuthGateTest` expected the OAuth flag false while fake-only regressions required the input true. Resolved operationally by running the default-off gate and OAuth-enabled fake suites separately; this whole-task invocation is not reported as passing. HTTP stayed false. |
| Isolated unit suite | PASS, 474 tests, 1m | MockWebServer/loopback cases; 0 failures/errors/skips, including session-change close behavior. |
| Isolated Android instrumentation compilation | PASS, 2m10s | `:app:compileIsolatedAndroidTestKotlin`; no emulator/device execution. |
| Focused CI composition contract (local equivalent) | PASS, 12 tests | Restored the established test class name and updated the exact expected method list after splitting product and isolated composition tests. The first PR SHA failed because the old workflow filter no longer matched the renamed class; a new PR SHA must rerun CI. |
| Documentation validation | PASS | `python3 tools/validate_docs.py docs`: 236 Markdown files, identifiers/JSON/placeholders/cross-references valid. |
| Secret scan | PASS | `gitleaks git --staged --redact`: 27.50 KB scanned, no leaks. No private APK/AAB output was found. |
| Independent review | PASS, 0 Blocker / 0 Major / 0 Minor / 0 Questions | Three source/test findings were addressed and re-reviewed; reviewer explicitly did not establish CI or Formal evidence. |
| API 26/35 Formal | NOT RUN locally | Android Emulator binary and AVDs are not installed on this host. Must be established by same-SHA CI. |
| Same-SHA Required / non-Required CI | Pending | Run and report after Draft PR creation; do not treat local checks as CI evidence. |

The initial `./gradlew` invocation from the repository root failed because the wrapper is under `android/`; it did not start Gradle. One rerun also had an invalid `JAVA_HOME` path and did not start Gradle. A separate Isolated rerun initially exposed two stale test-message assertions, which were updated to match the isolated composition's exact-grant rejection message. The first PR SHA also failed the focused non-Required workflow because the test class rename left its exact `--tests` filter and expected case names stale; the original FQCN was restored and the expected case list updated to the exact 12 tests. A local run of the same class filter and exact-case verifier passed. These environment/path/assertion issues were not treated as product RED. The follow-up PR SHA must establish fresh CI results.

## H4-F prerequisites and stop conditions

H4-F remains blocked until separately approved and evidenced: the Personal project/audience and Android OAuth client are configured by the Owner; the exact Private package and approved signing certificate are matched to the client; the code-only Private HTTP gate change receives its own Owner approval and review; same-SHA Required CI and API 26/35 Formal pass; and the Owner approves the exact account, purpose, time window, and stop conditions for real OAuth and API access. No real connection was attempted in H4-E2.

Keep Personal-mode creator-account mismatch fail-closed; test rejection only with synthetic authorization results. Preserve Primary-only, GET-only, events-readonly, Google HTTPS-only requests, no redirects, no CalendarList/write calls, no secret or calendar-content logs, no automatic Cloud AI egress, bounded retry, and no Live-to-Fake fallback. Local disconnect and Google grant revocation remain distinct operations.
