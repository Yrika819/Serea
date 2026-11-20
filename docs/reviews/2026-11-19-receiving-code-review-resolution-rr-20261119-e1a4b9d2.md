# Receiving Code Review Resolution — H4-D1/D2 G0 Runtime and CI Follow-up

## Resolution Contract

- Report type: `receiving-code-review`
- Resolution ID: `rr-20261119-e1a4b9d2`
- Review chain ID: `rc-20261119-2a9d620b`
- Source report ID: `cr-20261119-92d7c4e1`
- Source report path: `docs/reviews/2026-11-19-code-review-report-cr-20261119-92d7c4e1-g0.md`
- Source scope fingerprint: `sha256:5a055b561af98ec8429a801bb7c57672ec612d8d327bd90eeb079165f012dc70`
- Resolution date: `2026-11-19`
- Worktree branch: `codex/h4-d1-d2-private-google-calendar`
- Source-review target: `9dd770012ce7f79e82de1d30acc60fd8641aa1d7` (Tree `6f67e77e1fd1cf80284a5c7ab32402b6bf58b757`)
- Resolved implementation target: `425adc266556765bbbf6c9d4e33dbc8c86dd7419` (Tree `99f3901dca3c2029e01ac37313ff361868c8ebb1`)
- Resolution status: `All four source findings fixed; generation-1 independent review remains required`
- Google activity: `None; no real OAuth, Google API request, account, Cloud, signing, APK distribution, or device operation`

## Source Review

The complete frozen generation-0 report `cr-20261119-92d7c4e1` was read and validated without modification. Its four actionable findings were F1 isolated loopback cleartext policy, F2 stale non-required instrumentation identity count, F3 IPv6 loopback canonicalization, and F4 test Gateway injection accepted outside the isolated build. Its open blind spot was that the pre-fix Compose timeout logs did not identify the caught network exception or actual MockWebServer host.

The source report's findings were verified against current code, tests, and same-SHA CI evidence. The implementation fixes were made in commit `b5c45033d6f9ced74169fb211778b374db0f56db`; the final test assertion correction was subsequently made in `d209639` and `425adc2`.

## Disposition Ledger

| Source item | Issue fingerprint | Source severity | Evidence and scope | Verdict |
| --- | --- | --- | --- | --- |
| `F1` | `ifp-sha256:59e666842d76a0d07d0b5a5635501cee590a82cf1d02c1dc5578caa1a2f5c1aa` | `Major` | `android/app/src/isolated/AndroidManifest.xml` opts into an isolated-only network security config. The config denies cleartext by default and allows only localhost, 127.0.0.1, and ::1. The UI test asserts those loopback policies, rejects example.com, and confirms its MockWebServer URL is loopback. Product manifest and product cleartext policy were not relaxed. Focused UI E2E passed 47/47 on API 26 and 47/47 on API 35 at source SHA 425adc2. The original timeout's exclusive cause is not inferred from the successful rerun. | `Fixed` |
| `F2` | `ifp-sha256:feb32c8b283409532ad5ee7d1b0ae24d49e6dfa0e3de48ff4466e6f3b0d59268` | `Major` | The exact non-required identity expectation now matches the 33 explicitly listed identities; required/additional thresholds were not reduced. `python3 -m unittest discover -s tests -p android_instrumentation_results_tests.py -v` passed 13/13, and same-SHA Android Unit CI passed. | `Fixed` |
| `F3` | `ifp-sha256:93adac8397dcb76e16d749459060f8e226196978affc02b26049bd71d87027ce` | `Minor` | The composition allowlist and UI request-host assertion now use OkHttp's canonical `::1` representation. The isolated unit suite includes a deterministic IPv6 endpoint acceptance test. Focused API 26/35 UI E2E passed 47/47 on each API. | `Fixed` |
| `F4` | `ifp-sha256:4e9080c588878bc42b0336f4d699729dec9de007082f6fa0cc6642f80c0069ff` | `Minor` | `PrivateGoogleAuthorizationViewModel` rejects non-null synthetic Gateway injection unless `BuildConfig.BUILD_TYPE == "isolated"`; a Debug regression test verifies rejection. The production factory continues to use the default null seam and the product HTTP gate remains false. | `Fixed` |

## Challenges to Source Review

None. No source finding was disputed or reclassified. Fixing F1 does not prove that cleartext denial was the sole cause of every original timeout; the bounded claim is that the isolated loopback transport is now explicitly permitted and the full focused E2E passes on both required API levels.

## Additional Test Correction After the Source Review

The G0 report reviewed the UI/E2E path and documented a blind spot in its prior timeout evidence. After the fixes, the first focused E2E attempt on `d209639` exposed a separate Compose assertion issue: `America/New_York` appeared in two descendants of a tagged parent, while the assertion searched the parent node. The assertion was corrected in `425adc2` to assert exactly two matching descendants. This is a test-query correction; it changes no provider, authorization, calendar, or product behavior. The focused isolated UI E2E then passed on API 26 and API 35, 47/47 each.

## Verification Record

All evidence below is tied to source SHA `425adc266556765bbbf6c9d4e33dbc8c86dd7419` unless otherwise stated.

- G0 report validator -> valid: 4 findings, 0 test gaps, 9 coverage areas, recommendation `Changes requested`.
- `python3 tools/validate_docs.py docs` -> passed; 228 Markdown files scanned, including this resolution report.
- `python3 -m unittest discover -s tests -p android_instrumentation_results_tests.py -v` -> 13/13 passed.
- `:app:compileIsolatedAndroidTestKotlin` -> passed using Temurin JDK 17.
- Focused isolated UI E2E -> API 26 47/47; API 35 47/47.
- Android Formal -> API 26 105/105 and API 35 105/105; no failures, errors, or skips.
- Provider unit artifact -> 310 tests, 0 failures/errors/skips; exact isolated composition suite 12/12; Beta gate 1/1.
- Same-SHA Required contexts: `Linux fast checks`, `Linux stable full validation`, `Android Emulator Required Gate`, and `Linux MSRV 1.89.0` all succeeded.
- `git diff --check` -> passed.
- Bounded downloaded-artifact marker scan -> no matches for the recorded synthetic account/event fixture strings or common token markers; this is not a general DLP proof.

The non-required `Serea API 26/35 Real IME B/C (Diagnostic, Non-Required)` check failed at its unrelated source-scope allowlist step before building or running APKs. It is classified `NOT_DELIVERED`; it is not represented as a code/test failure or host-infrastructure failure, and its unrelated workflow was not changed.

## Invariants Preserved

- Public, Debug, Demo, and ordinary Private Beta remain Fake-only; product Google Calendar HTTP is still disabled.
- OAuth enablement and Google HTTP enablement remain independent. The test Gateway and loopback HTTP seam are isolated-build-only.
- `calendarId=primary`, the read-only Calendar Events scope, existing Session lifecycle/invalidation, transport, retry controls, token protections, and no-auto-egress boundary were not weakened.
- No Gmail scope, Calendar write API/scope, test activation property for product builds, or test credential was added.
- The frozen source report and the older unrelated report/resolution chain were not edited.

## Next Gate

Perform a terminal generation-1 independent review with baseline `9dd770012ce7f79e82de1d30acc60fd8641aa1d7` and target `425adc266556765bbbf6c9d4e33dbc8c86dd7419`, limited to the implementation delta and affected execution chains. Link both this resolution and the frozen source report. Generation 1 must be terminal; do not start another receiving cycle automatically.
