# G4 Cloud Egress Body Lineage

Date: 2026-10-27 UTC
Remediation commit: `4f86c5283edcfb7e0a0868d711a05bf7680247a3`
Base integration candidate: `373b8be15f407d951157d2a80a632e86d979acdf`
Worktree / branch: `/workspace/Serea-g4-egress-body-lineage` / `codex/serea-g4-egress-body-lineage`

## Finding and scope

An independent source review found a medium egress gap: the cloud gate checked prior-turn context provenance but did not check the current editor body. At the inspected candidate, the product Chat UI used the local Demo route and there was no current Google-to-Cloud dispatch path. This was therefore a latent activation blocker, not an observed live-service disclosure.

The first remediation review found two bypasses in the proposed fix: deleting imported text to blank and retyping could clear the lineage, and a direct call to the inner cloud adapter did not compare request and contract lineage. Both were repaired before this commit. The independent reviewer re-read the updated diff and reported no remaining blocker for the app-internal provider-prefill path.

External clipboard text cannot be attributed by this app-internal lineage field. Google-origin content must enter the composer through the provider-prefill API (or an equivalent API that carries inherited lineage) before any Google UI is wired to Chat.

## Change

- `ChatUiState` tracks current-editor source lineage separately from prior-turn context.
- Gmail/Calendar provider prefill assigns its provider origin and rejects blank prefill values.
- Ordinary edits, including delete-to-empty and retype, preserve inherited Google ancestry. User authorship is added as an ancestry marker; it does not erase Gmail/Calendar.
- `newConversation()` is the explicit fresh-draft boundary. It clears editor lineage and bumps the editor generation when a blank editor still carries lineage, so stale IME/editor callbacks cannot repopulate the reset draft.
- `ModelChatRequest` and `EgressContract` carry current-body lineage. Cloud egress denies missing, unknown, Gmail, Calendar, and other unapproved ancestry before Usage reservation or provider dispatch.
- The outer egress wrapper and inner `OllamaModelChatPort` both require request/contract lineage equality. The approved payload is also checked against the request lineage before dispatch.
- Local Demo routing remains available and carries lineage without enabling Cloud egress.

## Verification

Focused JVM command, run from `android/` with Temurin JDK 21:

```sh
./gradlew --no-daemon :app:testDebugUnitTest \
  --tests 'app.serea.android.SecurityPortsTest' \
  --tests 'app.serea.android.LocalFakeRoutingAdversarialTest' \
  --tests 'app.serea.android.GoogleOriginEgressAdversarialTest' \
  --tests 'app.serea.android.ChatViewModelTest' \
  --tests 'app.serea.android.OllamaCloudAdapterTest'
```

Result: **71/71 passed; 0 failures, 0 errors, 0 skips**.

| Test class | Tests | Failures | Errors | Skips |
|---|---:|---:|---:|---:|
| `SecurityPortsTest` | 10 | 0 | 0 | 0 |
| `LocalFakeRoutingAdversarialTest` | 3 | 0 | 0 | 0 |
| `GoogleOriginEgressAdversarialTest` | 1 | 0 | 0 | 0 |
| `ChatViewModelTest` | 36 | 0 | 0 | 0 |
| `OllamaCloudAdapterTest` | 21 | 0 | 0 | 0 |

Additional checks:

- `:app:compileIsolatedAndroidTestKotlin`: BUILD SUCCESSFUL (instrumentation source compiles; no Emulator test was run in this local step).
- `git diff --check`: passed.
- Focused fixtures are synthetic. Cloud adapter tests use MockWebServer; no Google account, real provider, real key, or real user data was used.

JUnit XML is preserved under [`evidence/g4-egress-body-lineage/`](evidence/g4-egress-body-lineage/).

## Independent review

The read-only G4 reviewer confirmed the two earlier implementation findings were closed for app-internal provider-prefill and reiterated that external clipboard attribution remains unknown. A second read-only review requested one more direct Cloud-port regression where both the request and contract carry matching Gmail/Calendar lineage, plus removal of trailing whitespace in this report. Both changes are now included and the focused suite was rerun.

Follow-up evidence: [`evidence/g4-egress-body-lineage-review-2/`](evidence/g4-egress-body-lineage-review-2/) records 72/72 passing tests with zero failures, errors, or skips. The added direct Cloud-port test exercises both Gmail and Calendar taint, and asserts zero credential reads and zero MockWebServer requests. `git diff --check` passes after removing the trailing whitespace. This run used Temurin 21.0.13+9 and no Emulator.

## Integration state

- The PR #132 candidate at `373b8be...` completed its exact-head campaign: API 26 Required 105/105 passed; API 35 failed before formal Required, at one focused `chatDraftSurvivesGmailAndCalendarDestinationChanges` check when Compose remained empty at `R4-B-committed` after accepted IME operations across three Android-created input sessions. API 35 Formal Required is NOT_RUN. Full, Fast, Native, SQLite, RustSec, Git Hooks, and Android Contract passed on `373b8be...`.
- No result from `373b8be...` is attributed to remediation commit `4f86c52...`.
- A non-Required R3 real-IME diagnostic was dispatched at exact HEAD `373b8be...`, Run `40603105338`. It is still in progress; its workflow runs API 35 startup/IME-state cases, then B/C/D and API 26 follow-up, so it is broader than an isolated fresh-AVD-per-case experiment.
- This remediation is not yet on PR #132. Independent review of the new follow-up commit is pending. Preserve the API 35 root cause as UNKNOWN; do not use a passing R3 diagnostic as a substitute for the formal gate.
- Real Google/AI services remain disabled/unconnected; Personal Beta remains NO-GO.
