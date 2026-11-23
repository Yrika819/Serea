# Embedded Core synthetic diagnostic (incomplete)

This is a local feasibility experiment, not a product integration. Only `-PsereaEmbeddedCore=true` enables the Android adapter, bindings and native libraries; it disables other Android build types and produces an unsigned isolated APK by default. An additional explicit `-PsereaEmbeddedCoreSignedDiagnostic=true` opts into a disposable key at `target/s1-android-home/diagnostic-only.p12`; create and use such a key only under specific Owner authorization, and never use a personal device. No existing Room database is used.

Generator and runtime must both be UniFFI 0.32.2. One-time generator installation (use a clean environment; cargo-ndk has printed environment variables on panic):

```sh
env -i HOME="$HOME" PATH="/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:$HOME/.cargo/bin" TMPDIR="${TMPDIR:-/tmp}" CARGO_BUILD_JOBS=2 cargo install uniffi --version 0.32.2 --features cli --locked --root "$PWD/target/s1-tools" --bin uniffi-bindgen
```

From the `android` directory, set `JAVA_HOME` to JDK 17 and `ANDROID_HOME` to the installed SDK and run, using the same clean-environment pattern:

```sh
./gradlew :app:compileIsolatedAndroidTestKotlin :app:assembleIsolated -PsereaEmbeddedCore=true --no-daemon --max-workers=2 -Dorg.gradle.jvmargs=-Xmx1536m
```

The task invokes cargo-ndk 4.1.2, NDK 28.2.13676358, API 26 and the installed x86_64/arm64 Rust targets. It builds both ABIs and generates Kotlin in `android/app/build/generated/embeddedCore`. The diagnostic's dependency lock is separate. The synthetic lifecycle was exercised on an isolated x86_64 API 35 emulator; panic behavior, concurrent processes, other ABIs/devices and production safety remain unverified.
