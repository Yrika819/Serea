# Serea Night Lane 3 — Gmail Mock Inbox UI Report

## Revision and scope

- Repository: `Yrika819/Serea`
- Branch: `codex/android-q4-night-gmail-ui-2026-10-21`
- Requested Draft PR base: `codex/android-q4-abcdefg-integration-2026-10-20`
- Pinned base: `072aa2b823523f7a92196299dd86cfb3509a78e3` (PR #66 was OPEN and Draft at this SHA)
- UI implementation HEAD: `960c59875de640ce16a3c39d4ac6848af6a08f49`
- UI implementation tree SHA: `f76e9b0daf22ab0097b947da528ce7b5e4b237f1`
- The implementation HEAD/tree above records the UI and JVM test commit. This report is a documentation-only follow-up commit.

Only these UI-specific source/test files were added:

- `android/app/src/main/java/app/serea/android/gmail/ui/GmailScreen.kt`
- `android/app/src/main/java/app/serea/android/gmail/ui/GmailViewModel.kt`
- `android/app/src/test/java/app/serea/android/gmail/ui/GmailUiTestFake.kt`
- `android/app/src/test/java/app/serea/android/gmail/ui/GmailViewModelTest.kt`

No shared domain, Activity, manifest, Gradle, workflow, OAuth, Task, History, egress, credential, or keystore files were changed. PR #58–#66 and `main` were not changed.

## UI behavior

The Compose Material 3 entry point provides a Japanese inbox/search/detail/thread flow. It includes fictional sender, subject, snippet and date rows; empty/loading/error states; retry; opaque-token paging; search progress and cancellation; detail navigation; ordered thread browsing; long-body chunking; and explicit missing-body text. The persistent banner reads `DEMO / 架空のメール — 実アカウントには接続していません`.

Paging keeps the `pageToken` contract opaque, rejects blank/oversized/repeated cursors, collapses exact duplicate message IDs and rejects conflicting duplicates. It prevents overlapping page loads, ignores stale results using an operation generation, and caps work at 20 pages / 500 messages. Empty pages with a continuation can advance. Search/detail route and query identifiers are stored through `SavedStateHandle`; bodies, paging tokens, and authorization state are not saved. Back navigation preserves the cursor history for the same list context and clears it when the list context changes.

Message details prefer nonblank `plainTextBody`, fall back to the Domain's safe `htmlTextBody` text representation, and explicitly report a missing body. HTML is displayed with Compose `Text`; there is no WebView. Thread results must match the requested thread ID and each message's thread ID. Message detail responses must match the requested message ID.

## Public Compose API and Navigation handoff

- `GmailScreen(viewModel: GmailViewModel, onBack: () -> Unit, modifier: Modifier = Modifier)`
- `GmailViewModel.factory(readPort: GmailReadPort, authorizationPort: GmailAuthorizationPort)`

A Navigation host can create and retain the state holder, then call the composable from its destination. Keep the existing fake provider injected in this phase:

```kotlin
val provider = remember { FakeGmailProvider() }
val gmailViewModel: GmailViewModel = viewModel(
    factory = GmailViewModel.factory(provider, provider),
)
GmailScreen(viewModel = gmailViewModel, onBack = onNavigateBack)
```

No Activity dependency is required by the new composables. The parent applies its existing Material theme, including dark mode and font scaling.

## Security boundaries

- Every read requires `GmailAuthorizationState.MOCK_ONLY` with no missing scopes. `MOCK_ONLY` is never treated as real Gmail authorization.
- Returned messages must have `source == MOCK`, retain `DataOrigin.GMAIL`, and must not be `DataClassification.PUBLIC`.
- UI operations use only the injected read and authorization ports. There are no network clients, OAuth calls, write methods, AI calls, attachment reads, or content logging in the new UI code.
- Errors are mapped to fixed Japanese messages; provider details and mail data are not interpolated into errors.
- The JVM boundary check verifies Fake-only port calls and checks the compiled `GmailViewModel` / `GmailScreenKt` constant pools for transport, OAuth, AI, and WebView references. This evidence is limited to the injected Fake execution and those two compiled classes; it is not a general runtime network monitor. The production UI implementation contains no transport path.

## JVM tests and local verification

- New Gmail UI JVM tests: **28**, all passed.
- Full `:app:testDebugUnitTest` suite: **194 tests**, 0 failures, 0 errors, 0 skipped.
- `:app:assembleDebug`: PASS.
- `:app:assembleRelease`: PASS.
- `:app:lintDebug`: PASS.
- `:app:assembleDebugAndroidTest`: PASS (APK assembly only).
- `git diff --cached --check`: PASS before the implementation commit; Git author identity was configured; no active custom hooks path was configured.
- Hosted GitHub CI: the first `gh pr checks` query after creating Draft PR #69, on HEAD `6908d085b264df2f1fa368ecab96f7d54b2901a9`, showed all listed jobs pending (0 completed). No repeated polling was done; use the PR Checks page for current status. The local build and test results above are complete.

The 28 Gmail tests cover inbox success/empty; one/multiple pages and opaque tokens; duplicate IDs, empty pages, invalid/repeating tokens and limits; search success/empty/error/blank, A→B races, cancellation and navigation; detail ID/body fallback/missing body/long text; thread order, selection and ID consistency; permission denied, missing scope, expired/revoked/real authorization rejection; timeout, rate limit and retry; SavedState restoration without bodies/tokens; GMAIL origin/non-PUBLIC classification; and Fake-only request wiring plus the scoped compiled-reference check.

## Independent reviews and fixes

- **Security review:** no P0/P1. Fixed the test's inert zero-network counter by replacing it with Fake-only invocation assertions and a scoped compiled-reference check.
- **Concurrency review:** no P0/P1. Preserved consumed paging-token history when backing out of detail, and restored an empty search without reissuing it. Added regression tests.
- **UI/accessibility review:** no P0/P1. Added Inbox/Search headings and result announcements, ellipsis for long sender/subject, removed a duplicate thread-list action, and made body chunking preserve surrogate pairs. Parent Material theme controls dark mode.
- **QA review:** no P0/P1. Restored-detail failures now display a retryable error, long-body integrity is covered, and the ineffective network counter was removed.
- **Separate final-diff review after fixes:** no P0/P1 and no further edits requested. It confirmed the compile fixes, blank-body fallback, Inbox root-stack reset, and test updates. Its note on the limited network-boundary test evidence is reflected above.

Review-fix tests were rerun: focused Gmail UI tests (28/28) and the full JVM suite (194/194) passed. All listed build/lint tasks passed afterward.

## NOT_RUN and remaining work

- Compose visual, TalkBack, large-font, and light/dark appearance checks on an emulator: **NOT_RUN**; this environment has no emulator executable or configured AVD.
- Hosted GitHub CI: **NOT_RUN locally**, pending Draft PR creation.
- No unresolved P0/P1 or product/security blocker was identified. Navigation integration remains for the integration phase because `MainActivity.kt` was explicitly out of scope; use the injected `FakeGmailProvider` handoff above. Persistent Compose instrumentation tests were intentionally not added to avoid changing the shared Android test gate.

Disposable-emulator integration specification: verify fictional banner and TalkBack heading order; search progress/cancel and A→B freshness; empty/error inbox with retry; detail Back; thread ordering and selection; long subject/address/body scrolling at fontScale 2.0; and light/dark themes. Keep provider injection Fake-only.
