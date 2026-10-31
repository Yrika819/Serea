# Codex Cloud — Serea V21 First Development Task

**Execution gate:** Start only after Owner authorizes the selected commit/push plan in `v21-codex-cloud-migration-handoff.md` and the approved changes are present in GitHub. This instruction does not authorize commit, push, merge, PR-state changes, model downloads, Pixel operations, signing, or billing.

## Source of truth and preflight

1. Repository: `Yrika819/Serea`; expected integration PR: #204.
2. Expected branch at current snapshot: `codex/serea-v13-integration-candidate`; expected baseline head `8017e7e9e8987dc66361eb9297cf87535d8ef7b1`; parent/base branch is the current #199 source `codex/serea-v11-agent-a-vm-candidate` at `018d19f025e5f3232abcf2a112b8dbf954bf04e8`.
3. Before changing anything, report actual branch, HEAD, base/head ancestry, tracked/untracked state, and exact latest PR status. If actual refs differ from this snapshot, stop and reconcile with Owner rather than force-resetting or assuming equality.
4. Read repository `AGENTS.md`, then the approved V21 migration handoff and the relevant V20 CI evidence. If Native work is requested, also read Native clone `AGENTS.md` and treat it as a separate source lane.
5. Never import `.kotlin/`, `.cxx/`, build outputs, APK/AAB/`.so`/device pulls, model weights, raw logs, secrets, personal data, or Mac-specific absolute paths.

## First source task — recover the V20 Android CI fixes safely

1. Inspect the exact commit diff containing the approved V20 CI repair. Confirm the two logged causes in GitHub Run `41054235106`: invalid Compose `assertExists` explicit import and Kotlin mixed-array bind inference errors. Confirm fixes are limited to removing the import while retaining the member call, plus `arrayOf<Any?>` bind arrays.
2. Preserve existing tests, manifests, Required/Additional identity counts and formal API26/API35 gates. Do not fold unrelated Compose UI, PhysicalQA, NativeQA or doc changes into the CI-only commit.
3. Use JDK 17 and pinned wrapper 8.14.6. Run the narrowest CI-equivalent compile and focused tests first, then all required Cloud CI checks only after the exact approved candidate is ready. Keep Android/NDK work sequential; cap workers as configured by project guidance.
4. Record source SHA, command, task outcomes, JUnit count/failures/skips, artifact hashes, and independent review result. A CI run is evidence only for its exact source SHA. Do not claim Formal Android Required PASS until the post-fix exact-head API26/API35 gate actually completes.
5. Ask for review after a focused diff is ready. Do not create a commit or push without the separate explicit Owner approval.

## Follow-on — physical test design (Cloud source work only)

After the CI repair review, make a design-only proposal (implementation only if separately authorized) for:

- Unicode text injection into the Compose editor as one case, explicitly distinct from actual Japanese IME conversion/commit.
- `ActivityScenario.recreate()` as Activity recreation, distinct from a target-process kill/relaunch case.
- Exact method/token allowlist, isolated target package `app.serea.android.isolated`, companion test package `app.serea.android.isolated.test`, signer/hash preflight and host serial validation for Pixel 7a / `lynx` / API37 / arm64-v8a.
- A separate physical-only runner/Gradle entry point. Preserve `SereaIsolatedAndroidJUnitRunner` emulator-only guard and all formal required selectors/counts. Reject broad selectors and fail closed on package/device/API/ABI/token mismatch.
- Do not install IMEs or APKs, change Pixel settings, access notifications/personal data, clear app/Room/Keystore data, or run the Pixel without explicit Owner approval. No model acquisition/inference is part of this task.

## Native/model lane remains separate and blocked

PR #160 Run `40884851425` is an API35 x86_64 diagnostic inference PASS on its exact source. `arm64-v8a` packaging/build PASS is not ARM64 runtime. Do not transplant #160/#163 into #204. No model weights may be downloaded, staged, or run until Owner supplies exact approved artifact/revision/SHA, license/distribution decision and quantitative resource limits. Model product integration remains default-off and isolated.

## Done criteria

- CI repair is causally reviewed and passes focused Cloud compile/tests on an exact source SHA.
- No Required count/gate or Runner safety restriction was loosened.
- Post-fix Formal Android Required is separately reported PASS/FAIL/NOT_RUN from its actual exact-head run.
- Physical Japanese IME/Activity recreation/process-death proposal is concrete, but all device work remains NOT_RUN until Owner approval.
- No commit/push/merge has occurred without Owner approval; no local-only artifacts or sensitive files entered the Cloud source branch.