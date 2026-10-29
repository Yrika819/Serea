# G8N Lane B — Google, Gmail, and Chat Contract

**AGENT_ID:** B
**ACTIVE_PHASE:** G8 evidence closure / G9 Functional Candidate
**GOAL_STATUS:** PARTIAL — Gmail handoff minimization change, focused JVM coverage and scoped independent review are complete; Android UI execution remains pending.
**BASE_SHA:** `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` (PR #146)
**BASE_TREE_SHA:** `25e2bb481a0b25bfed89d6ee25f1812316703b8e`
**TESTED_SOURCE_TREE_SHA:** `b19a6c52a8e8d4ff1c8365d435c047d0aceb300b`
**WORKTREE:** `/workspace/Serea-gmail`
**BRANCH:** `codex/serea-g8n-google-gmail-ui`
**VERIFIED_CODE_HEAD:** `4e4b8f9d0aad80f330d1c93364b3cbfda5369bcd`
**VERIFIED_CODE_TREE_SHA:** `b19a6c52a8e8d4ff1c8365d435c047d0aceb300b`
**DRAFT_PR:** [#150 OPEN / DRAFT](https://github.com/Yrika819/Serea/pull/150), based on PR #146's frozen candidate branch at `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66`

## Completed change

The Gmail-to-Chat content formatter now passes only the selected body text: plain text takes precedence, followed by the parser-produced inert text from an HTML part. It no longer appends subject, sender, timestamp, or Gmail API snippet. An empty body no longer falls back to the API snippet. The subject remains available as the local preview title. Handoff still binds the source item, body revision, body identity, preview expiry, account/session generation, and confirmation state; title changes are checked at prepare, confirm, and consume.

Added `GoogleTrustedHandoffTest#gmailChatHandoffContainsOnlySelectedBodyAndExcludesMessageMetadata`. Its synthetic message gives distinct sentinel values to message ID, thread ID, sender, recipient, subject, date, snippet, source, classification, origin, and the unused alternate HTML body; it also checks that account/session IDs, generation, scope, source ID/revision, content identity, service/origin, consent, destination, expiry, conversation and selection metadata stay out of `localChatText()`. The oversized-title test also asserts that subject text is not placed into Gmail Chat content. A separate HTML-only test passes encoded hostile HTML through `GmailMimeParser`, a Fake provider, and handoff, then checks that only inert text reaches Chat and the script, event handler and remote URL do not. Calendar handoff behavior is unchanged.

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

This is a read-only B4 review of PR #146's exact base (`d444e7cc04c8f1c4c6d12b1412cfe3a76258af66`) and PR #150's code candidate (`4e4b8f9d0aad80f330d1c93364b3cbfda5369bcd`). PR #150 changes only the Gmail handoff formatter and its tests; it does not change chat composition or model selection.

| Mode | Current code/UI | Status |
|---|---|---|
| `DEMO` | `MainActivity` composes `DemoChatComposition.create(...)`, which pins the `Serea Local Demo (fictional)` descriptor (`serea-demo/local-chat`) to `MockModelChatPort`. The path is local and deterministic, with Router → Egress → Usage wrappers and no network. The UI labels the conversation as Demo. | Implemented and the only ordinary Chat path. |
| `LOCAL_MODEL` | `LocalModelChatAdapter`, `EgressTarget.LOCAL_MODEL`, a content-bound local-model consent proof, and local-model Usage attribution exist as isolated infrastructure. No production Chat composition invokes them, no user-facing `DEMO`/`LOCAL_MODEL`/`CLOUD` mode picker exists, and the Android native runtime is unverified. | Adapter/security candidate only; no product switch or native model acceptance. |
| `CLOUD` | Cloud adapter and Egress contract code exist, but ordinary Chat does not compose a Cloud adapter. The UI labels Cloud sending disabled, disables its connection-test button, and says provider eligibility and Owner activation are unconfirmed. | Disabled; no Chat mode switch or active Cloud dispatch path. |

The visible model settings have only `AUTO`/`MANUAL` routing preferences and balanced/cost/quality routing policy. They persist locally and affect an on-screen preview built from an empty `LocalModelCatalog`; they do not change the fixed Demo composition or provide a model choice. Thus neither those preferences nor the saved setting are a product model-mode switch.

History is encrypted. Persisted `StoredConversation` has a `modelId` field that defaults to `demo-local` when Chat creates a conversation, but the in-memory `Conversation` used by Chat has no model identity and there is no implemented cross-model history carry, reset, migration, or confirmation flow. Usage is accounted per request with the selected model/provider attribution in a separate ledger; the active Demo wrapper resolves the fixed Demo descriptor, while local-model attribution code is not reached from the product path. Current cancellation/error handling covers the active Demo request (including timeout, definite failure, usage-limit failure, and conservative unknown-outcome status after cancellation). There is no mode-specific switch behavior for draining a request, releasing native resources, changing model-bound context, or displaying mode-specific availability/errors.

The Google consent boundary remains distinct from Cloud. The only Google connection path here is a Fake, read-only grant. Google handoff is an in-memory, single-use confirmation for `LOCAL_CHAT` with `LOCAL_PROCESSING_ONLY`; it is not a Cloud permission. Cloud Egress separately requires its own explicit send-intent contract and rejects Google/Gmail/Calendar lineage; `GoogleTrustedHandoffTest#googleLineageCannotUseCloudEgressEvenAfterLocalConsent` proves this denial after local handoff confirmation. No enabled Cloud consent/send UI exists in this candidate.

Before implementing a real mode switch, keep the active conversation's model identity explicit, require an explicit choice before carrying history to another model, retain Usage attribution to the model actually selected, and show mode-specific availability, cancellation, and errors. Switching must stop or drain the active send and release native resources before changing routes. It must not silently fall back to Demo or Cloud. Until the local runtime passes its gates, ordinary Chat must stay accurately labeled Demo. These are acceptance requirements, not completed switch behavior.

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
