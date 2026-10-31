# V21 Cloud-Safe Summary — Android CI Recovery

Date: 2026-10-31. This is a sanitized derivative of the local V20 CI recovery record; it intentionally omits local absolute paths, usernames, SDK locations, ADB serials, binary APKs, raw logs and device pulls. Preserve the original evidence locally.

## Exact GitHub failure and repair

- GitHub Android run `41054235106` failed at exact source HEAD `8017e7e9e8987dc66361eb9297cf87535d8ef7b1`, task `:app:compileIsolatedAndroidTestKotlin`.
- Logged causes: unresolved explicit import of `androidx.compose.ui.test.assertExists` in `LocalModelChatViewModelIntegrationAndroidTest.kt` (the member call remains valid), and mixed Kotlin array/reified intersection inference diagnostics in `LegacyHistoryV4MigrationDiagnosticTest.kt` bind arguments.
- Preserved source fix: remove the invalid import; type affected SQLite bind arrays as `arrayOf<Any?>`. No required test identity/count/gate was reduced.

## Local verification recorded

- **PASS:** `:app:compilePhysicalqaAndroidTestKotlin -PsereaPhysicalQaRunner=true --rerun-tasks` using JDK 17, Gradle Wrapper 8.14.6, existing Android SDK and sequential Gradle execution capped at two workers.
- **PASS:** `:app:assemblePhysicalqaAndroidTest -PsereaPhysicalQaRunner=true`.
- **PASS:** `:app:testDebugUnitTest --tests 'app.serea.android.LocalModelChatAdapterTest'` — 29 tests, 0 failures, 0 errors, 0 skips.
- **PASS (prior authorized physical test, not this UI campaign):** one exact allowlisted Fake-only local consent/usage replay test returned `OK (1 test)`. No real model/provider/cloud inference was involved.
- **PASS:** independent read-only code review accepted the two compiler fixes as causal and semantics-preserving.
- **NOT_RUN:** post-fix GitHub Formal Android Required, including disposable API26/API35 instrumentation and required-count/process-death/data-isolation/sentinel gates. Run `41054235106` remains a failure; local checks do not replace or convert that result.

## Boundaries

No required gate was loosened. No model weights, credentials, Google data or personal device data were used in the CI repair. The prior physical Fake-only result is not a Japanese IME, Activity recreation, ARM64 model or V20 UI JUnit result. New Formal CI must be bound to the exact authorized commit SHA.