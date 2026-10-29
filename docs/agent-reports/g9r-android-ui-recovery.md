# G9R Android UI Recovery

Status: PARTIAL — reviewed test changes are integrated in PR #162; Android API26/API35 UI and Required acceptance remain NOT_RUN.

- Base candidate source: `683854ba94cb3a1e1972b73d0f1f60392f2271d6` (PR #155 before the parent documentation checkpoint).
- Scope: Android and JVM test sources only. No production composition, manifest, URL policy, required identity, or Required Gate changes.
- Changes: isolated test-only HTTPS Calendar fixture with exact Google Calendar GET origin/path/query checks and redirects disabled; agenda failure diagnostics and semantics-based scrolling; API35 focus recovery diagnostics; removed only a nonfunctional scroll-to action while retaining stale-account and no-send assertions.
- Local verification at integration source `333e11a25e15ccf3efc923f3ac66b0f245ad306b` / tree `94d3122f4908a2f892dc37b9b3a42adf3453701e`: with isolated Temurin JDK 17.0.21+9, `:app:testDebugUnitTest :app:compileIsolatedAndroidTestKotlin --rerun-tasks` PASS; 408 JVM tests, 0 failures/errors/skips across 34 XML files, and isolated AndroidTest Kotlin compilation PASS (55 tasks). This updates the earlier missing-JDK snapshot below. The source itself was clean and `git diff --check` passed.
- API26/35 UI acceptance: NOT_RUN for this source. Earlier artifacts are bound to their original #155 source and are not reused.
- Google OAuth, real Google accounts/data, Cloud Egress, and production transport behavior were not activated.
- Independent review accepted the fixture boundary and assertions; PR #162 includes the same UI test sources as PR #161 follow-up `ff42d9bcd08010dd7100d708d17fc676bcd5e739`.
- Exact-head PR #162 Android run `40888081858` is PENDING with only Change Scope job `123497539347` queued. API26/35 Required and focused UI JUnit are NOT_RUN at the current source.

No UI pass is claimed by this report.

## Local JDK 17 follow-up

The official Adoptium JDK 17.0.21+9 archive was downloaded into the isolated workspace and matched SHA-256 `8b9b611d917e3dfd8419d3566b5949226f69b052fed7e109c1368add1dfe8711`; no system Java configuration was changed.

The first JDK-backed build found two test-fixture defects: `okhttp3.Dns` is not a Kotlin SAM and the instrumentation test lacked the `onAllNodesWithTag` extension import. These were fixed using explicit `Dns.lookup()` implementations in each test source set and the missing import. The first compiled JVM run also exposed two assertions expecting a `Host` header that MockWebServer did not expose; assertions now verify the recorded URL is `localhost`, while the test-only interceptor continues to enforce the exact Google Calendar origin before rewriting.

After fixes, `:app:testDebugUnitTest` completed SUCCESS: 408 tests, 0 failures, 0 errors, 0 skipped across 34 XML files. Aggregate `sha256sum` over the per-file `sha256sum` lines: `629bc454be4bf9cf2720d1f0e649c66bfde188866f64219dd196e6176e8b812c`.

`:app:compileIsolatedAndroidTestKotlin` completed SUCCESS with Java 17: 26 tasks, 2 executed and 24 up-to-date. The initial guessed task `:app:compileDebugAndroidTestKotlin` did not exist; no source test was run by that command. API26/35 emulator tests remain NOT_RUN locally because this SDK has no Emulator, NDK, or API26 platform. The exact-head GitHub UI run is still needed for UI acceptance.
