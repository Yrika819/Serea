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

The same read-only G4 reviewer first returned CHANGES_REQUESTED with the two findings above. After the fixes and new regressions, the reviewer returned: “prior findings addressed for the app-internal provider-prefill path. No remaining blocker in this remediation.” The review also reiterated the external clipboard attribution limitation. Review did not run tests; the test results above are from the orchestrator’s local run.

## Integration state

- The current PR #132 candidate at `373b8be...` has in-progress exact-head Android and Native runs. Full, Fast, SQLite, RustSec, Git Hooks, and Android Contract results on that SHA are tracked separately in the orchestrator ledger/PR.
- No result from `373b8be...` is attributed to remediation commit `4f86c52...`.
- This remediation is not yet on PR #132. After the parent SHA's current Required campaign finishes, advance only the owned integration branch by fast-forward and rerun the complete Required campaign on the resulting exact HEAD.
- Real Google/AI services remain disabled/unconnected; Personal Beta remains NO-GO.
