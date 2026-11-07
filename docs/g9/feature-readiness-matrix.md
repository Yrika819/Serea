# Feature Readiness Matrix (main `72dbd245c46fa111a22e9d85e22410278a259326`)

| Area | Status | Source evidence | Gap / beta work |
|---|---|---|---|
| AI Chat | IMPLEMENTED (demo) | Chat UI/ViewModel; `ChatScreenTest`; demo labels | Real inference unconnected; define safe provider/model route, lifecycle, egress/usage ledger; verify on device |
| Model Router | UNKNOWN | Rust crates exist; no verified Android invocation path established in this pass | Map JNI/FFI and invocation path; connect only after model/security decision |
| Local Model | IMPLEMENTED (isolated route candidate) | PR #191/#193 changed `LocalModelChatAdapter.kt`, `LocalModelChatFeatureRouter.kt`; not shown in main during this pass | Determine merge/equivalence; runtime/model format, license, ARM64 memory and Pixel 7a measurements |
| Calendar | IMPLEMENTED (fake/demo) | Google Calendar screens, fake provider/handoff and tests | Real OAuth/API/pagination/revocation not live verified; read-only scope and owner consent required |
| Gmail | IMPLEMENTED (fake/demo) | Gmail fake fixtures, inert HTML test/handoff | Restricted scope approval, MIME/body handling and real API absent/unverified; keep disabled for beta absent approval |
| Google OAuth | IMPLEMENTED (demo) | `googleauth/GoogleAuthorization.kt`, `FakeGoogleAuthorizationGateway.kt`, demo adapters | No real credentials/account flow verified; owner chooses OAuth client/scopes/policy |
| Tasks | VERIFIED (test evidence present) | `tasks/Task*`, database/security tests in project | Re-run formal matrix on current SHA; device process death, notification/reminder acceptance |
| Android Overlay | UNKNOWN | No production overlay path established | Check manifest/service, permission rationale, OS policy, accessibility; defer unless explicit requirement makes P0 |
| Beta Release | IMPLEMENTED (configuration) | package/build variants and release tooling exist | owner signing, provenance, protected environment, Firebase/OIDC and rollback not authorized or live verified |

Status vocabulary: NOT_IMPLEMENTED / IMPLEMENTED / VERIFIED / LIVE_VERIFIED / UNKNOWN. Tests in repository are not treated as live provider/device evidence.

## G9-R3 call-path correction

At main SHA `72dbd245c46fa111a22e9d85e22410278a259326`, `MainActivity.kt:115` constructs `ChatViewModel` with `DemoChatComposition.create(usage)`. `DemoChatComposition.kt:12` defaults to `MockModelChatPort`; it wraps a `LocalDemoModelChatPort`, Egress/Usage controls and Kotlin `AdaptiveModelRouter` constrained to fake provider `serea-demo`. This is the normal Chat path and produces the deterministic demo response.

`LocalModelChatAdapter.kt` exists in main as a guarded adapter/lifecycle with `LocalModelInferenceBackend` interface. It is not used by `MainActivity` composition. The Kotlin `LocalModelChatFeatureRouter` and its tests from #191/#193 are absent from main. No Android JNI `external fun`/native library loader implementation was found in the Chat path. Rust `crates/serea-model-router` is a separate core router implementation; this audit did not find an Android binding connecting it to `ChatViewModel`. A present Rust router or Kotlin `AdaptiveModelRouter` does not mean native inference is active.
