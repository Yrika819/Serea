# API 26/35 Independent Real IME B/C Diagnostic

Status: harness implemented; no emulator run has been dispatched from this branch.

Base: `373b8be15f407d951157d2a80a632e86d979acdf` (PR #132 diagnostic source).

## Scope and execution model

The manual workflow `.github/workflows/android-api35-real-ime-independent-bc.yml` is diagnostic
and Non-Required. It builds the isolated target APK, instrumentation APK, and separately packaged
Test IME APK once, stores their SHA-256 manifest, and then runs four independent cases:

| API | Case | Instrumentation method | AVD profile |
| --- | --- | --- | --- |
| 26 | B continuous | `B_realImeCompositionWithoutSynchronizationBoundaries` | `pixel_2` |
| 26 | C synchronized | `C_realImeCompositionWithSynchronizationBoundaries` | `pixel_2` |
| 35 | B continuous | `B_realImeCompositionWithoutSynchronizationBoundaries` | `pixel_7` |
| 35 | C synchronized | `C_realImeCompositionWithSynchronizationBoundaries` | `pixel_7` |

`max-parallel: 1` serializes the matrix. Each case runs on a separate ephemeral GitHub-hosted
runner and a separately wiped, accelerated Google APIs x86_64 AVD. Each device job installs and
verifies the same three APK files produced by the single build job. No API-specific rebuild is
performed.

The workflow's source-scope guard permits only its exact workflow path, the single-case helper,
this evidence document, and the two diagnostic-only source files involved in the request-contract
repair (`IndependentImeCompositionTest.kt` and `RealImeDiagnosticsProvider.java`). It rejects all
other changes since the diagnostic base.

## Device guards and evidence

The helper reuses `assert_disposable_android_emulator.sh`,
`wait_for_android_default_ime.sh`, and `prepare_isolated_instrumentation_target.sh`. It accepts only
API 26 or API 35 and requires the selected API, `ro.kernel.qemu=1`, emulator hardware/fingerprint
appropriate to that API and x86_64, completed boot, and no preinstalled target, instrumentation,
or Test IME package. The Android system image is explicitly selected as `google_apis;x86_64` in
each matrix job. The isolated target namespace must be empty before the unique one-use campaign
marker is seeded; the existing Android runner must claim that exact marker.

Exactly one fully qualified B or C method is passed to `am instrument`. JUnit validation requires
exactly one testcase, the exact requested method identity, zero failures/errors/skips, and a
successful instrumentation result. Each case captures logcat, IME lifecycle and operation
identity, Activity state, process and memory information, CPU/memory pressure, APK identities,
campaign marker state, and default-IME restoration. The Test IME is disabled and uninstalled, and
the original default IME is verified after each case. Every ADB call performed during
cleanup/restoration has a 15-second deadline and a 3-second forced-kill grace. The two methods and
the two API levels do not share emulator or IME process state.

## Interpretation

These runs are diagnostics and do not substitute for the formal 105-test Required Gate. Every
result must remain attributed to its own API, case, source SHA, APK digests, and fresh AVD. A
diagnostic harness result alone does not establish a product root cause. Real Google accounts,
credentials, and user data are not used.

## Pending evidence

- Build/run IDs: NOT_RUN
- API 26 B/C: NOT_RUN
- API 35 B/C: NOT_RUN
- Artifacts and APK SHA-256: NOT_RUN
- Emulator cleanup and IME restoration: NOT_RUN
