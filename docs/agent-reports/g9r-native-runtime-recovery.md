# G9R Native Runtime Recovery

Status: PARTIAL — review and exact-head Android runtime evidence pending.

- Base PR #158 source: `80e9990625b000e9d56a78743a7460ac120d4979`.
- Worktree branch before successor checkpoint: `codex/serea-g9r-native-recovery`.
- Scope: JNI cancellation observation, native terminal outcome, bounded release diagnostics, and cleanup failure reporting. No product Chat integration.
- Focused result: `bash tools/g8-local-model-native/test-cleanup.sh` PASS; `bash -n tools/g8-local-model-native/run-e2e.sh` PASS; `git diff --check` PASS.
- Android/JNI compile and Emulator inference: NOT_RUN in this environment. Configured Java 17 path is missing; available Java 21 installation is a JRE without `javac`.
- Prior exact PR #158 run `40866167732` had failed ADB package query and device-temp cleanup. It produced no JUnit/runtime artifact. Model inference remains NOT_VERIFIED.
- Required next steps: independent review; focused AndroidTest/JNI build; exact-head x86_64 Emulator run proving cancellation, usage outcome, release and cleanup; then ARM64 build/runtime status separately.
- Pixel 7a physical runtime: NOT_RUN and requires Owner approval.

Security invariants remain: isolated offline target, model hash validation, bounded resource use, fail-closed cleanup, no Cloud fallback, and no product integration before acceptance.
