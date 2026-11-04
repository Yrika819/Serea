# G10 V12 Native Android and Release Preflight

**Status:** preparation only; Android ARM64 runtime and Pixel 7a acceptance remain `BLOCKED / NOT_RUN`; Personal Beta remains `NO-GO`.
**Evidence source:** PR #200 `1c725fa322ef4ec2a76a71371db5deadc6e59083`, tree `766154f4e3956656a8005b2534bece35809f5e6d`; local environment inspected 2026-10-30 UTC.
**Related candidates:** PR #198 `eb07aee0408e562f17cb472b474a79e1dca8f30f` / tree `681ef92cfbb78f195af01f572eb8ffc6a45be259`; PR #199 `c1d163b4091fbab29a2e4d2c3c42596b355e8226` / tree `0742f510a493219394aae55176b98f1d640a0796`.
**Authority:** PR #190 Owner Decision Ledger is canonical for O-01 through O-10. This document makes no Owner decision and authorizes no device, signing, model download, OAuth, cloud, or distribution action.

## ARM64 Android build and package capability

### Source inventory

- The Android app module uses Kotlin/AGP and has no `ndkVersion`, `externalNativeBuild`, `jniLibs`, `abiFilters`, or `System.loadLibrary` configuration.
- The Rust workspace does not declare a `cdylib` crate or Android JNI entry point. `serea-host` is a native diagnostic executable; its Cargo package has no `crate-type` configuration. Native platform CI builds host executables for Linux, macOS, and Windows targets, including ARM64 operating systems; this is not Android NDK/JNI evidence.
- `LocalModelChatAdapter` defines a backend interface and integrity checks. The inspected #198 APK contains no Serea inference JNI or model weights. No `.so`, GGUF, ONNX, TFLite, or safetensors payload is tracked in the reviewed source.

### Local toolchain inventory

Read-only commands and observed results on the V12 x86_64 Linux host:

| Check | Command | Result |
|---|---|---|
| Android SDK packages | `JAVA_HOME=/usr/lib/jvm/java-21-openjdk-amd64 $ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager --list_installed --sdk_root="$ANDROID_HOME"` | Build Tools 35.0.0, Platform Tools 37.0.1, Android Platform 35 only; no NDK or CMake package. The inherited `JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64` is invalid; Java 21 path works for this inventory command. |
| Rust host | `rustup run stable rustc --version` and `rustup show active-toolchain` | rustc 1.99.1, host `x86_64-unknown-linux-gnu`. |
| Rust Android targets | `rustup target list --installed --toolchain stable` | Only `x86_64-unknown-linux-gnu`; Android targets are not installed. `rustc --print target-list` recognizes `aarch64-linux-android` as a target name but that is not an installed target or successful build. |
| Android Rust linker | `command -v cargo-ndk aarch64-linux-android-clang clang ld.lld llvm-readelf` | No executable found. `/usr/bin/readelf` is available for ELF inspection if an artifact is supplied. |
| Device attachment | `adb devices -l` | No attached devices. |

No NDK, target standard library, Android linker, JNI crate, Android `.so`, or attached ARM64 device is available. Therefore an Android ARM64 static build or package-content verification cannot be performed from this workspace. Do not infer it from Rust ARM64 host CI, x86_64 inference, Java/Kotlin tests, or ABI declarations in fake test metadata. Do not repeat the prior x86_64-host software-emulator boot attempt under the same conditions.

### Required future static and package checks

After a reviewed Android JNI crate, pinned NDK/toolchain, and exact candidate APK exist, a disposable non-production build should record the exact source/tree, target, NDK version, Rust toolchain, Gradle/AGP versions, build run, and all resulting binary hashes. The build must explicitly target Android `arm64-v8a` with minimum API 26 and package the reviewed `.so` into the intended variant. This is a future build plan, not a command that can currently succeed in this repository.

For the exact built artifact, retain and verify:

1. `sha256sum` for APK, each `.so`, and model payload; `apkanalyzer`/`aapt dump badging` for package ID, version code/name, min/target SDK; `apksigner verify --print-certs` for certificate identity. Use only a disposable debug/test certificate until O-01/O-05 permit a Beta signing workflow.
2. APK ZIP inventory for every `lib/<abi>/*.so`; `readelf -hW` or NDK `llvm-readelf -hW` must report AArch64 for `arm64-v8a` libraries. Inspect ELF dependencies/SONAMEs and verify packaged dependencies, JNI symbol/registration expectations, and minimum API compatibility.
3. Package identity: `app.serea.android.demo` and candidate `app.serea.android.beta` are separate IDs in current Gradle configuration, but Beta identity is not Owner-ratified. Prove side-by-side install/data isolation and same-signer forward update only after the relevant decisions and controlled test setup.
4. APK-bound SPDX or CycloneDX coverage reconciled to actual archive members, dependency locks, native libraries, licenses/notices, Gradle plugins, wrapper, Android SDK/NDK, Rust crates, and build tools. `tools/generate_android_lockfile_sbom.py` explicitly covers only locked Gradle components; `tools/bind_android_apk_sbom.py` binds an APK subject/package/version but preserves gaps for dependency edges, artifact hashes, publisher authenticity, plugins, wrapper, and SDK. These tools do not by themselves produce a complete release SBOM.

No production key, signing operation, or Beta artifact is available or authorized. Existing Gradle beta signing checks fail closed if required key material/fingerprint/version inputs are missing; this preparation must not supply any.

## Model fixture provenance and license boundary

PR #199's test-only ViewModel fixture creates a temporary `.gguf` file from the seven bytes `06 02 06 04 03 03 08`; its size is 7 bytes and SHA-256 is `d883c7320d0ed3b809075f9bcf140228c99159318fa816538746796a443295ee`. It passes that file to a fake inference backend, which checks that a file exists and returns fixed test output. The declared repository `https://huggingface.co/test/viewmodel-integration-fake`, revision, `Apache-2.0` license ID, and runtime fields are test metadata only. The bytes are not a real GGUF model and those metadata fields are not evidence of an upstream license or redistribution permission.

Adapter unit tests also write small arbitrary temporary byte arrays and inject a fake backend. They prove integrity checks and route policy against test doubles; they do not exercise a supported model format, a model license, a native runtime, or model performance. No model weights are tracked or downloaded by this review.

Before selecting or redistributing any real model under O-10, record an exact source revision, upstream publisher/source, license text and model-specific terms, required notices/attribution, redistribution and commercial-use permissions, payload format, file size, SHA-256, and compatibility/security review. At runtime, recheck expected size and SHA-256 before load, keep the file in app-private storage, and bind model/runtime/library hashes to the same build packet. An `Apache-2.0` string in test data is not a substitute for this review.

## Database migration, recovery, and backup evidence

- `SereaHistoryDatabase` is Room schema version 5, with explicit migrations 1→2, 2→3, 3→4, and 4→5. `TasksDatabase` is schema version 1. Exported schema files exist for history versions 1, 4, and 5, and task schema version 1.
- The Required Android test inventory includes migration coverage from history versions 1, 2, and 3 that preserves synthetic history and unknown attribution, plus a failed migration case that checks the prior database remains at its old version and retains data. There is no equivalent successful fixture migration from version 4 in that test, nor demonstrated process-kill/reopen recovery during an upgrade, backup restore, or a supported cross-version Beta update on the physical Pixel.
- The app manifest disables backup and both legacy and modern backup rules exclude root/files/databases/shared preferences/external files. This is source policy; OEM/device restore behavior and recovery requirements still need synthetic, exact-package validation. A documented backup policy is required before any destructive recovery exercise.

Before release, define supported source/target schema pairs and expected encrypted-record preservation. Verify each supported migration from a realistic synthetic pre-upgrade DB, interrupt/restart behavior, keystore/key continuity, usage-ledger consistency, backup exclusion or an approved encrypted backup path, and update compatibility with a same-signer forward APK. Prove downgrade and signer mismatch fail without mutating existing state. Never use uninstall/data-clear as a rollback mechanism.

## Non-destructive Pixel 7a acceptance sequence

This is preparation only. Do not operate the Pixel until O-06 is separately authorized and package/signing/version policy is settled under O-01/O-05. Use a disposable profile with synthetic data; stop if the profile may contain personal data or an existing Serea install.

1. Before install, capture device model/API, available storage, battery/thermal state, package inventory, app version, and signer read-only. Preserve existing Demo state.
2. Install only an authorized exact candidate into the disposable profile. Verify package IDs and app-private storage roots. Do not uninstall or replace Demo.
3. Populate synthetic history/settings/model fixture only. Check encrypted DB open, schema, backup exclusions, ABI/library hashes, model hash/size, and offline behavior.
4. Exercise same-signer forward update, supported migrations, interruption/restart recovery, and synthetic backup/restore procedure. Confirm downgrade/signer mismatch rejection preserves data.
5. Exercise cold launch, background/foreground, Activity recreation, OS process death, trim/memory pressure, model eviction/reload, cancellation/load failures, and crash/ANR recovery. Verify consent/process state is not resurrected and usage settles or becomes unknown consistently. Verify there is no cloud or Demo fallback and no unauthorized egress.
6. Collect only sanitized outcomes and artifact hashes. Remove disposable synthetic data only within its specifically authorized test profile; do not touch personal data, reset the device, or retain prompts, accounts, tokens, or private logs.

## Current G10 disposition

G10 preflight is `PARTIAL`. The existing G10 V11 plan remains the readiness matrix. Package identity/coexistence and signing policy remain pending O-01/O-05; complete APK-bound SBOM/NOTICE is a release blocker; model selection/license/redistribution remains O-10 undecided; DB update/recovery is not demonstrated; Android ARM64 JNI/model runtime is blocked/not run; Pixel 7a is blocked/not run pending O-06. Personal Beta remains `NO-GO` until every mandatory exact-source gate closes and the Owner explicitly sets O-08 to GO. O-07 merge approval remains separate.
