# G8N Lane B — Google, Gmail, and Chat Contract

**AGENT_ID:** B
**ACTIVE_PHASE:** G8 evidence closure / G9 Functional Candidate
**GOAL_STATUS:** PARTIAL — Gmail handoff minimization change, focused JVM coverage and scoped independent review are complete; Android UI execution remains pending.
**BASE_SHA:** `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` (PR #146)
**BASE_TREE_SHA:** `25e2bb481a0b25bfed89d6ee25f1812316703b8e`
**TESTED_SOURCE_TREE_SHA:** `b19a6c52a8e8d4ff1c8365d435c047d0aceb300b`
**WORKTREE:** `/workspace/Serea-gmail`
**BRANCH:** `codex/serea-g8n-google-gmail-ui`
**CURRENT_HEAD:** `4e4b8f9d0aad80f330d1c93364b3cbfda5369bcd`
**TREE_SHA:** `b19a6c52a8e8d4ff1c8365d435c047d0aceb300b`
**DRAFT_PR:** pending exact-head CI launch

## Completed change

The Gmail-to-Chat content formatter now passes only the selected body text: plain text takes precedence, followed by the parser-produced inert text from an HTML part. It no longer appends subject, sender, timestamp, or Gmail API snippet. An empty body no longer falls back to the API snippet. The subject remains available as the local preview title. Handoff still binds the source item, body revision, body identity, preview expiry, account/session generation, and confirmation state; title changes are checked at prepare, confirm, and consume.

Added `GoogleTrustedHandoffTest#gmailChatHandoffContainsOnlySelectedBodyAndExcludesMessageMetadata`. Its synthetic message gives distinct sentinel values to message ID, thread ID, sender, recipient, subject, date, snippet and the unused alternate HTML body, then confirms that only the plain body appears in `localChatText()`. The oversized-title test also asserts that subject text is not placed into Gmail Chat content. Calendar handoff behavior is unchanged.

The `GmailMessage` product model has no arbitrary raw-header, label, attachment, provider-response or full-payload field. Those values are not copied into the handoff text. `DataOrigin.GMAIL` is preserved separately as lineage for the existing egress gate.

## Focused verification

Command (Temurin JDK 17.0.21):

```text
JAVA_HOME=/workspace/.tools/temurin17 PATH=/workspace/.tools/temurin17/bin:$PATH ./gradlew --no-daemon --console=plain -Dhttp.proxyHost=proxy -Dhttp.proxyPort=8080 -Dhttps.proxyHost=proxy -Dhttps.proxyPort=8080 -Djavax.net.ssl.trustStore=/etc/ssl/certs/java/cacerts -Djavax.net.ssl.trustStorePassword=changeit :app:testDebugUnitTest --tests app.serea.android.handoff.GoogleTrustedHandoffTest
```

Result: **BUILD SUCCESSFUL**; JUnit `GoogleTrustedHandoffTest`: **22 tests, 0 failures, 0 errors, 0 skipped**. The executed code/test tree matches commit `4e4b8f9d0aad80f330d1c93364b3cbfda5369bcd` / tree `b19a6c52a8e8d4ff1c8365d435c047d0aceb300b`. `git diff --check` passed.

The isolated AndroidTest sources were also checked on that exact source HEAD/tree with:

```text
JAVA_HOME=/workspace/.tools/temurin17 PATH=/workspace/.tools/temurin17/bin:$PATH ./gradlew --no-daemon --console=plain -Dhttp.proxyHost=proxy -Dhttp.proxyPort=8080 -Dhttps.proxyHost=proxy -Dhttps.proxyPort=8080 -Djavax.net.ssl.trustStore=/etc/ssl/certs/java/cacerts -Djavax.net.ssl.trustStorePassword=changeit :app:compileIsolatedAndroidTestKotlin
```

Result: **BUILD SUCCESSFUL**; Gradle marked 26 tasks up-to-date, including `compileIsolatedAndroidTestKotlin`. This is source-compilation evidence, not UI JUnit execution. There was no local Android device (`adb devices -l` listed no devices), so Android UI execution is **NOT_RUN** in this lane.

PR #146 Android run `40804694025`, attempt 1, matched source `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66`; it was `in_progress` at the last query. Full CI `40804693907`, Native `40804693934`, and SQLite `40804693978` were queued; RustSec `40804693889`, Fast `40804693992`, Git Hooks `40804693919`, Release Security `40804722382`, and Mock Beta Contract `40804722157` had succeeded. These are PR #146's evidence, not verification of this new branch. API 26/35 UI execution for Gmail remains **NOT_RUN in this lane** pending the exact formal Android job evidence or a focused emulator run.

## Chat mode contract

There is no implemented product `ChatModelMode` enum or user-facing model switch in this source. The labels below distinguish present code from proposed behavior:

| Mode | Current code/UI | Status |
|---|---|---|
| `DEMO` | Ordinary Chat is labeled `DEMO`; `DemoChatComposition` selects `Serea Local Demo (fictional)` (`serea-demo/local-chat`) with `MockModelChatPort`, local Router → Egress → Usage path and no external service. | Implemented and the only ordinary Chat path. |
| `LOCAL_MODEL` | Isolated `LocalModelChatAdapter` and `EgressTarget.LOCAL_MODEL` exist. The ordinary Chat composition does not select or expose them. | Adapter candidate only; no verified native end-to-end runtime and no product switch. |
| `CLOUD` | Cloud UI and egress types exist, but there is no enabled model selection or live dispatch. Cloud sending is disabled by default; Google-origin lineage remains blocked. | Disabled; no Chat mode switch. |

Before a future mode switch is implemented, keep each conversation tied to its stored model identity, require an explicit choice before carrying prior history to a different model, preserve Usage attribution to the actual selected model, and make mode-specific availability, cancellation and errors visible. Switching must cancel or drain an active send and release local native resources before changing routes. It must never fall back silently to Demo or Cloud. If a local model has not passed runtime verification, ordinary Chat stays on the accurately labeled Demo path. Cloud data consent remains independent from Google connection and handoff consent. These are acceptance requirements, not completed switch behavior.

## Evidence contract

**CHANGED_FILES:** `android/app/src/main/java/app/serea/android/handoff/GoogleTrustedHandoff.kt`; `android/app/src/test/java/app/serea/android/handoff/GoogleTrustedHandoffTest.kt`; this report.
**CI_RUN_IDS:** PR #146 run IDs listed above are comparison evidence only; exact Lane B branch CI pending.
**ARTIFACT_IDS:** None for this lane.
**SECURITY_IMPACT:** Reduces Gmail metadata transferred to local Chat text. Google origin, local-only consent and fail-closed Cloud egress remain intact. No OAuth, live Google data, Cloud request, Composition Root, or `MainActivity` change.
**REVIEW_STATUS:** ACCEPTED by Lane A for this scoped source change at `4e4b8f9d0aad80f330d1c93364b3cbfda5369bcd` / tree `b19a6c52a8e8d4ff1c8365d435c047d0aceb300b`; Lane A explicitly did not accept the unrun API 26/35 UI path or all of G9.
**KNOWN_ISSUES:** Focused Gmail UI and API 26/35 instrumentation have not run on this branch.
**CURRENT_BLOCKERS:** No local emulator; formal exact-HEAD Android result for this successor branch is pending.
**NEXT_ACTION:** Commit/push this successor Draft PR, request independent review, then collect exact-HEAD CI. Keep UI JUnit as NOT_RUN until an exact focused JUnit record exists.
**OWNER_APPROVAL_REQUIRED:** None for synthetic/local work. Real OAuth, real accounts/data, Cloud, production signing, Pixel 7a, main merge and Beta remain Owner controlled.
