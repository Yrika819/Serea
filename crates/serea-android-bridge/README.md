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

The tested setup used cargo-ndk 4.1.2, NDK 28.2.13676358, API 26 and installed x86_64/arm64 Rust targets. Gradle currently hardcodes the macOS NDK path and does **not** enforce the cargo-ndk version. It builds both ABIs and generates Kotlin in `android/app/build/generated/embeddedCore`. The diagnostic's Android dependency lock is separate. The bridge is an unconditional Rust workspace member: normal `cargo ... --workspace` commands build it and resolve its dependencies even when the Android opt-in is disabled. The synthetic lifecycle was exercised on isolated API 26 and 35 x86_64 emulators; panic behavior, concurrent processes, other ABIs/devices and production safety remain unverified.

The bridge holds a non-blocking OS lock on a persistent sidecar associated with the canonical DB path until shutdown. This coordinates cooperating bridge processes without taking SQLite's own locks; Unix DB hard links are rejected, and SQLite sidecar names and the ownership suffix are reserved. The application must keep the DB and lock file in its private `noBackupFilesDir` and must not unlink, rename, replace, or hard-link either while in use. Never remove the persistent lock file as cleanup: an existing holder can otherwise retain a lock on an unlinked inode while another process locks its replacement. Android app-private storage is a boundary against other ordinary application UIDs, not malicious code sharing the UID, privileged code, or non-cooperating SQLite clients. The native diagnostic API accepts arbitrary paths and does not enforce this directory boundary. Therefore this is a conditional diagnostic guarantee, not a security boundary or a general lock service. Filesystem locking and process-kill recovery must be tested on Android; host results do not establish Android behavior.

The Rust mutex intentionally fails closed if poisoned: a panic while holding it leaves this diagnostic unavailable until the process restarts; native abort cannot be caught by Kotlin. A live Core can also be stranded until process restart if its adapter is lost without `close()`. Cancellation, process-wide ownership and robust recovery are not production-ready.
