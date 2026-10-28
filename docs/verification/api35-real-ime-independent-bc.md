# API 26/35 Independent Real IME B/C Diagnostic

Status: harness implemented and integrated with the isolated Test IME session-contract repair;
no emulator run has been dispatched from the campaign branch.

Functional source origin: PR #132 diagnostic source `373b8be15f407d951157d2a80a632e86d979acdf`.
Campaign source-scope base: `e0643750321a682fdf53b1024b8491ebcb128fa7`, the clean G4 candidate
with the reviewed Google-origin egress-lineage repair and its local focused evidence. The workflow
compares only changes after this campaign base against its narrow diagnostic allowlist.

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
this evidence document, the B/C and provider request-contract sources
(`IndependentImeCompositionTest.kt`, `RealImeDiagnosticsProvider.java`, and `RealImeService.java`),
the formal Chat acceptance test's Test IME adapter (`SereaAppShellTest.kt`), and the exact AVD guard
module and its regression script. It rejects all other changes since the campaign base.

The formal test adapter was updated to read a fresh Android-owned service/session/connection/input
type snapshot immediately before each operation, submit all four identities as preconditions, and
assert that the provider invoked that exact connection and accepted the call. This removes the old
`fieldName` expectation (which the provider does not expose) and prevents reuse of a stale connection.
No product source or acceptance assertion was changed.

## Device guards and evidence

The helper reuses `assert_disposable_android_emulator.sh`,
`wait_for_android_default_ime.sh`, and `prepare_isolated_instrumentation_target.sh`. It accepts only
API 26 or API 35 and requires the selected API, `ro.kernel.qemu=1`, emulator hardware/fingerprint
appropriate to that API and x86_64, completed boot, and no preinstalled target, instrumentation,
or Test IME package. The Android system image is explicitly selected as `google_apis;x86_64` in
each matrix job. The isolated target namespace must be empty before the unique one-use campaign
marker is seeded; the existing Android runner must claim that exact marker.

The single-case helper identifies the active AVD using `adb emu avd name`, reads that AVD's
`config.ini`, and requires the configured image tag to exactly match
`system-images;android-${API};google_apis;x86_64`. Fingerprint acceptance is API-specific; generic
fingerprint substrings do not satisfy the image check. The known API 26 fingerprint form
`Android/sdk_gphone_x86_64/generic_x86_64` is accepted without requiring a `google/` prefix. A fake
config regression test rejects an API/image mismatch and a Play Store image.

Exactly one fully qualified B or C method is passed to `am instrument`. JUnit validation requires
exactly one testcase, the exact requested method identity, zero failures/errors/skips, and a
successful instrumentation result. Each case captures logcat, IME lifecycle and operation
identity, Activity state, process and memory information, CPU/memory pressure, APK identities,
campaign marker state, and default-IME restoration. The Test IME is disabled and uninstalled, and
the original default IME is verified after each case. Every ADB call performed during
cleanup/restoration has a 15-second deadline and a 3-second forced-kill grace. Logcat shutdown uses
bounded TERM/KILL polling and never performs an unbounded wait. Fake-process regressions cover both
normal TERM shutdown and a TERM-ignoring process that requires KILL. The two methods and the two API
levels do not share emulator or IME process state.

## Interpretation

These runs are diagnostics and do not substitute for the formal 105-test Required Gate. Every
result must remain attributed to its own API, case, source SHA, APK digests, and fresh AVD. A
diagnostic harness result alone does not establish a product root cause. Real Google accounts,
credentials, and user data are not used.

## Pending evidence

- Local target/test/IME APK build: PASS on the campaign worktree; exact final commit evidence pending
- Workflow Run ID / artifact IDs: NOT_RUN
- API 26 B/C: NOT_RUN
- API 35 B/C: NOT_RUN
- Campaign APK SHA-256: pending exact-head build capture
- Emulator cleanup and IME restoration: NOT_RUN

## Request-contract build checkpoint

The separate diagnostic-only request-contract repair was reviewed for B/C independence and locally
built at the exact HEAD below. Its build does not imply instrumentation success.

- Source branch: `codex/serea-r3-ime-operation-contract-20261027`
- Source HEAD: `bc31a9a6d1aabb2ff1c2915b508fa7808074ae6a`
- Source tree: `b486a12a84d2ac51d10396a705cec6812ba778b0`
- B/C separation review: ACCEPTED by independent adversarial reviewer; runtime remains NOT_RUN
- Build: `:app:assembleIsolated :app:assembleIsolatedAndroidTest :r25-ime-test:assembleDebug` — PASS
- Instrumentation / Emulator: NOT_RUN
- Target APK SHA-256: `d50d1b7486e5ebd1f4d16750ae73224b9d547979f717ee44028fd5fde0a455dc`
- Instrumentation APK SHA-256: `3ca33abc59924cf66dc5e822cf641eaaf05ff3d7078bb508112f6f94e940d4b4`
- Test IME APK SHA-256: `9efc2070ed61aba6e154617c457dd33b07ab6e059b03f7d71dbb7e7d3f580a43`
- Full build log and static-check summary: `/workspace/evidence-r3-contract-0463854.md` (updated for this HEAD)
