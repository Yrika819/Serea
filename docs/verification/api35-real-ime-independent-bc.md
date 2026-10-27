# API 35 Independent Real IME B/C Diagnostic

Status: harness implemented; no emulator run has been dispatched from this branch.

Base: `373b8be15f407d951157d2a80a632e86d979acdf` (PR #132 diagnostic source).

## Scope

The workflow `.github/workflows/android-api35-real-ime-independent-bc.yml` is a manual,
non-Required diagnostic. It builds the isolated target APK, instrumentation APK, and separately
packaged Test IME APK once, stores their SHA-256 manifest, and then runs the existing Test B and
Test C methods as separate matrix jobs. `max-parallel: 1` serializes those jobs. Each job gets a
separate GitHub-hosted runner and starts a wiped API 35 Google APIs x86_64 AVD with hardware
acceleration required; software-emulated fallback is rejected.

The build and both device jobs record the exact source commit/tree. Each device job verifies the
build artifact manifest and all three APK digests, then verifies the installed target, test, and
Test IME APKs against those same files. The workflow refuses source diffs outside `.github/workflows`,
the one diagnostic helper, and this evidence document since the stated base SHA.

## Device guards and evidence

The helper reuses `assert_disposable_android_emulator.sh`,
`wait_for_android_default_ime.sh`, and `prepare_isolated_instrumentation_target.sh`. It requires
API 35, `ro.kernel.qemu=1`, an emulator hardware/fingerprint, x86_64 ABI, completed boot, no
preinstalled target/test/IME packages, and an empty isolated target namespace before the unique
one-use campaign marker is seeded. The existing Android test runner must claim that exact marker.

Only one fully qualified B or C test method is passed to `am instrument`. The generated JUnit is
checked for exactly one testcase, the exact requested identity, zero failures/errors/skips, and a
successful instrumentation result. The helper captures logcat, IME service state, Activity state,
process and memory information, CPU/memory pressure, APK identities, campaign marker state, and
default-IME restoration. The Test IME is disabled and uninstalled, and the original default IME is
verified after each case. The GitHub-hosted AVD itself is disposable and the two methods do not
share emulator or IME process state.

## Interpretation

These runs are diagnostics and are not a substitute for the 105-test Required Gate. B/C results
must remain attributed to their own run, method, source SHA, APK digests, and separate AVD. No
product root cause is implied by a test harness result alone. Real Google accounts, credentials,
and user data are not used.

## Pending evidence

- Build/run IDs: NOT_RUN
- API 35 B: NOT_RUN
- API 35 C: NOT_RUN
- Artifacts and APK SHA-256: NOT_RUN
- Emulator cleanup: NOT_RUN
