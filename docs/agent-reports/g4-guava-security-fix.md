# G4 Guava security fix checkpoint

## Scope and provenance

- Branch: `codex/serea-g4-guava-security-fix`
- Base: `c9ce1c2674e57dbab787d97be5f24c9822f4fe22` (PR #125 head)
- Dependency patch commit: `f4fe40400f5dded8fe5006420a2e91ca6d2a803e`, tree `2bcc849f8b03edf61ebdb57144c07e07e2860240`
- Addresses OSV `GHSA-xxph-c9ww-hj94` / `CVE-2026-102554`, affecting Guava through `33.7.1`.
- `android/app/build.gradle.kts` forces Guava to `33.7.2-jre` for app configurations. The locked consumers are JVM-side AGP, KSP, and Android Test Platform tooling; no app runtime configuration uses Guava.
- `android/app/gradle.lockfile` now selects Guava `33.7.2-jre` across all 21 configurations that previously locked Guava, with its updated transitive dependency versions.

## Verification

- Gradle lock resolution selected `33.7.2-jre` for all 21 locked configurations. The latent `_internal-unified-test-platform-android-test-plugin-host-retention` configuration is not exposed in the normal Gradle model, so a temporary init script materialized it from its existing locked modules for lock refresh. No emulator or AndroidTest runtime was launched.
- `:app:dependencies :app:generateLicenseReport` — **BUILD SUCCESSFUL**.
- OSV-Scanner `2.3.3`, recursive scan of `android/` — **317 packages, 0 vulnerabilities**; the cited advisory is absent.
- `:app:compileDebugKotlin`, `:app:compileIsolatedKotlin`, `:app:compileIsolatedAndroidTestKotlin` — **PASS**.
- `:app:testDebugUnitTest` — **325/325**, 0 failures, 0 errors, 0 skipped.
- Local Gradle verification used Temurin JDK `21.0.13+9`, downloaded to `/tmp` from Adoptium and checked against SHA-256 `d7f5d5cf865a2902b70ff549b97586a7d3f234e016f041ab447ea740c7d66d1c`.

## Boundary

The patch and evidence are local to this branch. No push, PR, emulator run, or Required Gate run was performed while G4 Android run `40533346554` was active.
