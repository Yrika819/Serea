# G6-B — On-device AI / Product Adapter

**Checkpoint date:** 2026-10-28 UTC
**Status:** PARTIAL — adapter contract and fake-backend tests are committed; focused JVM execution is blocked by unavailable Gradle distribution download. No Serea app composition or native backend is enabled.

## Agent report

```text
AGENT_ID: B — On-device AI / Product Adapter
ACTIVE_PHASE: G6-B adapter contract and ARM64 readiness audit
GOAL_STATUS: PARTIAL
BASE_SHA: b84feb337cd57918346ab31cb3ea4af4ba880ffb
CURRENT_HEAD: a3f06abe00c3fd3df9f51c5140cdcd01d95f42c8 (pushed PR #136 head; tree `a2e100158188aa3473c6a910d200fd8113bc1353`; accepted code snapshot `91a19febc81def03debf9e3ce29abb35a0c0c993`, tree `ced4366a5f0b3f93fe20cfcac04daadd368b810f`)
TREE_SHA: a2e100158188aa3473c6a910d200fd8113bc1353
PRE_FIX_PR_HEAD: b0e0860ebf47807b90fa06f6e0a7b2f2ac8f688c (Android Integration failed test compilation; B-CI-06 fixed in accepted follow-up)
WORKTREE: /workspace/Serea-g6-agent-b
BRANCH: codex/serea-g6-local-ai-adapter
DRAFT_PR: [#136](https://github.com/Yrika819/Serea/pull/136), OPEN/DRAFT against G5 branch `codex/serea-g5-evidence-closure-20261028`
COMPLETED_WORK: audited isolated G3 source/model metadata; added Product ModelChatPort adapter contract, one-use local Egress consent, model hash/identity checks, usage attribution, backend lifecycle contract, fake-native hostile tests; fixed B-REVIEW-01..05 and B-CI-06, with independent ACCEPTED at code HEAD `91a19febc81def03debf9e3ce29abb35a0c0c993`
CHANGED_FILES: android/app/src/main/java/app/serea/android/SecurityPorts.kt; android/app/src/main/java/app/serea/android/LocalModelChatAdapter.kt; android/app/src/main/java/app/serea/android/LocalModelSecurity.kt; android/app/src/test/java/app/serea/android/LocalModelChatAdapterTest.kt; docs/agent-reports/g6-b-local-ai-adapter.md
TEST_COMMANDS: `cd android && JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 PATH=/usr/lib/jvm/java-21-openjdk-amd64/bin:$PATH ./gradlew --no-daemon --console=plain :app:testDebugUnitTest --tests 'app.serea.android.LocalModelChatAdapterTest'`; `git diff --check`
JUNIT_RESULT: Focused local suite NOT_RUN because Gradle 8.14.6 wrapper download failed with `java.net.ConnectException: Connection refused`; `git diff --check` PASS. On prior PR head `b0e0860`, Android Integration job `122802983264` failed compiling `LocalModelChatAdapterTest.kt:594:43` (`UsageLedgerState.COMPLETED` unresolved; corrected to `SETTLED` in independently accepted source `91a19fe`). The failed job produced no JUnit evidence. New exact head `a3f06ab` checks are pending/queued and no tests have run yet.
CI_RUN_IDS: 40685604635 (Android, pending); 40685604632 (Native, pending); 40685604667 (Full, queued); 40685604672 (Git Hooks, queued); 40685604658 (RustSec, queued); 40685604609 (SQLite, queued); 40685604726 (Fast, queued); 40685596104 (existing main-only Beta workflow, zero jobs / failure on branch push); G3 PoC diagnostic 40657508541 (API 35 x86_64, not G6 adapter CI)
ARTIFACT_IDS: G3 PoC artifact 12802681672, digest sha256:df06078f5c8b4164bd24e255d1df0e2df9069570e08c08eea1295f4f391161f8; no revised-source JUnit or CI artifact yet
SECURITY_IMPACT: adds LOCAL_MODEL as a distinct local-only Egress target; requires exact consent/body/context/model binding, a live conversation ID+generation, and one-time authorization consumption before Usage/provider delegate; Google lineage requires typed validated handoff evidence and current session/account validation; Cloud remains separate and default Chat composition is untouched
DEPENDENCIES: parent composition must create explicit local-model route and usage resolver; Agent A TrustedGoogleHandoff must be mapped from the exact accepted object and revalidated through coordinator.isCurrent with live conversation identity; native backend factory, private model-file staging/hash verification, catalog identity, lifecycle owner, and independent review remain required
KNOWN_ISSUES: Revised G6 adapter tests not executed; no concrete JNI backend is added; local-model route is not wired into product Chat; Google-originated prior-turn context is denied until every context item has an independently bound source proof; a concrete backend must implement the sticky cancel/native-start gate contract; llama.cpp vendor notice/SBOM closure is incomplete so redistribution is not authorized
CURRENT_BLOCKERS: New exact-head CI pending; local Gradle distribution fetch unavailable; no ARM64 runtime/device in this environment; no Pixel 7a acceptance; native runtime and product composition not implemented
INTEGRATION_READY: implementation snapshot accepted for adapter-only integration review; not ready for product enablement
REVIEW_REQUEST: ACCEPTED by independent Agent C at exact code HEAD `91a19febc81def03debf9e3ce29abb35a0c0c993` / tree `ced4366a5f0b3f93fe20cfcac04daadd368b810f`, including B-REVIEW-01..05 and B-CI-06. PR #136 now contains that code and this checkpoint at head `a3f06abe00c3fd3df9f51c5140cdcd01d95f42c8`.
NEXT_ACTION: monitor exact-head CI for PR `a3f06ab`; fix and re-review any CI issue; map A's accepted handoff into the evidence interface; implement and review concrete JNI backend separately; retain product Chat default as Demo
OWNER_APPROVAL_REQUIRED: Pixel 7a physical-device operation; real Google access; cloud AI credentials/fees; production signing/distribution; main merge; Personal Beta GO
```

## G3 source and evidence audit

PR #126 and #127 were inspected through read-only refs; no PoC files or model weights were copied into this G6 branch.

| Source | Exact revision / file | Finding |
|---|---|---|
| PR #126 | head `8ca55a32096bc4449717c2fd3332b0602381d7d5`, tree `6c74ddaad77467c6240aa95feedd7ecf981f7204` | Initial standalone feasibility implementation |
| PR #127 | head `a2bafeea77160e8e2746bdfdd0d309348ac04aa7`, tree `25874a7f3fc53028ae72aa7ddf4cc42a5e823f70` | Reviewed current G3 diagnostic candidate; still outside product Chat and Required runtime path |
| Runtime sources reviewed | `tools/g3-ondevice-poc/README.md`, `model-fixture.json`, `app/build.gradle.kts`, `app/src/main/cpp/CMakeLists.txt`, `app/src/main/cpp/llama_jni.cpp`, `app/src/main/java/app/serea/g3poc/GgufModelChatPort.kt`, `app/src/test/java/app/serea/g3poc/ModelChatPortContractTest.kt`, `docs/agent-reports/g3-ai.md` | Test-local `ModelChatPort` shape is not the Serea production interface. G3 project is absent from Android settings and product APK composition. No model weights are committed. |
| Runtime implementation | llama.cpp commit `276378515ceebee5d14bcc8536603dd5c87491a2` (`b11927`) | Pinned native source. `LLAMA_BUILD_COMMON`, tools, tests, examples, server, OpenSSL, OpenMP, llamafile, dynamic backend loading, KleidiAI and native CPU tuning are disabled. The JNI implementation uses static llama/ggml libraries. |
| Model candidate | `ggml-org/SmolLM2-135M-GGUF`, repo revision `1076b85d135017971845b19b8d07df1479155988`; fixture `SmolLM2-135M-Q4_K_M.gguf`; expected size `101016128`; expected SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5` | Pinned metadata declares Apache-2.0 and base model `HuggingFaceTB/SmolLM2-135M`. The metadata response verifies repository/license/base model; fixture size and hash are expected values checked against fetched bytes by the diagnostic, not claims established by metadata alone. |
| License evidence | Hugging Face metadata verification step in run 40657508541; upstream llama.cpp `LICENSE` at pinned commit | Model metadata declaration is Apache-2.0. llama.cpp license is MIT. The pinned llama.cpp CMake adds vendored hash/miniaudio/nlohmann/sheredom/stb projects; hash sources include SHA-256 public-domain notice, xxHash BSD-2-Clause, and rotate-bits MIT. A complete linked-artifact notice/SBOM inventory is still required; redistribution remains NOT_AUTHORIZED. |

### Runtime status

- `40657508541` completed SUCCESS at exact G3 PR #127 source `a2bafeea77160e8e2746bdfdd0d309348ac04aa7`. Its single diagnostic job selected API 35 `google_apis/x86_64`, built the app and instrumentation APKs with `arm64-v8a` and `x86_64` ABI filters, then ran the model lifecycle test successfully **1/1** on x86_64. Artifact `12802681672` is evidence only; it does not contain model weights.
- **ARM64 Build: PASS (cross-compile only)** at the above diagnostic head; this is not ARM64 runtime evidence.
- **ARM64 Runtime: NOT_RUN.** This agent environment is `x86_64`; `/dev/kvm` is absent and `adb devices -l` listed no device. Parent independently found no qemu-aarch64, Android emulator binary, attached device, or KVM.
- **Physical Pixel 7a: NOT_RUN.** No throughput, latency, peak/RSS memory, thermal, power, battery, or storage acceptance has been recorded.
- The run validates a standalone llama.cpp test app, not the G6 adapter code or production Chat. Its PSS samples are observational and do not prove OS reclamation of native memory.

## Adapter contract and integration boundary

The candidate adds the real-model route identity `serea-local/SmolLM2-135M-Q4_K_M` and a `ModelChatPort` implementation separate from `serea-demo/local-chat`. Before native load it checks model file size and SHA-256 against immutable artifact metadata, exact selected model ID, destination, request/body/context binding, ABI support, prompt/output bounds, context binding, and a bounded native runtime configuration. The backend has no HTTP client, credential reader, endpoint, or cloud fallback.

`LOCAL_MODEL` is a new Egress target with `serea://local-model` destination. A local consent proof is one-use, short-lived, and bound to the exact model, conversation ID+generation, send ID, body/context digest, and source lineage. For Gmail/Calendar input it also binds the exact A handoff ID, account/session/generation, scope/item/revision, selected-content hash, canonical reviewed Chat envelope hash, consent/selection/auth state, destination, and expiry. The composition must map the exact accepted A `TrustedGoogleHandoff`; the registry revalidates that object through A's `GoogleTrustedHandoffCoordinator.isCurrent` using the live conversation binding. Missing mapping, stale session, changed conversation, expiration, cancellation, or replay fail closed before the Usage layer.

All local-model and Google-handoff expiration values use Android monotonic elapsed milliseconds (`SystemClock.elapsedRealtime()`), never epoch time. The local `EgressCheckedModelChatPort` has a dedicated injectable monotonic clock for `LOCAL_MODEL`; its existing wall-clock source remains for Cloud contracts. Pending local proofs are bounded to 16, pruned on issue, and can be explicitly revoked when the user cancels.

The intended order stays `RoutedModelChatPort → EgressCheckedModelChatPort(LOCAL_MODEL) → UsageControlledModelChatPort → LocalModelChatAdapter`. `LocalModelUsageAttributionResolver` records the explicit local model ID and provider `local-device`; no verified price is invented, so monetary cost is unknown (device energy/thermal cost is not accounted). Default app Chat remains the Demo route until parent review, a production native implementation, device/resource acceptance, accurate model display, and extra CI are complete.

### Tests authored (execution pending)

`LocalModelChatAdapterTest` uses synthetic files and a fake native backend. It covers product interface compatibility, multiple turns, token usage, identity/price attribution, hash mismatch before native load, cloud/demonstration target rejection, exact provider handoff source/body envelope, stale handoff, exact consent expiry, missing live conversation binding, payload tampering, one-use replay before another usage reservation, explicit consent cancellation/revocation, known pre-inference failure reservation release, outcome-unknown after generation starts, close during model load, close before backend publication, close at the backend load/generate native-start gates, fresh backend recovery after load/generate failure, and a test-only `RoutedModelChatPort → EgressCheckedModelChatPort → UsageControlledModelChatPort → LocalModelChatAdapter` route.

These tests were not run because the local Gradle wrapper distribution download failed with connection refused. No test or build result for G6 adapter source is claimed.
