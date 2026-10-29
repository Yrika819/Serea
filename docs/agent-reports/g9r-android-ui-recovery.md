# G9R Android UI Recovery

Status: PARTIAL — test changes saved for CI; Android UI acceptance and independent review pending.

- Base candidate source: `683854ba94cb3a1e1972b73d0f1f60392f2271d6` (PR #155 before the parent documentation checkpoint).
- Scope: Android and JVM test sources only. No production composition, manifest, URL policy, required identity, or Required Gate changes.
- Changes: isolated test-only HTTPS Calendar fixture with exact Google Calendar GET origin/path/query checks and redirects disabled; agenda failure diagnostics and semantics-based scrolling; API35 focus recovery diagnostics; removed only a nonfunctional scroll-to action while retaining stale-account and no-send assertions.
- Local verification: `git diff --check` PASS. Gradle JVM tests and AndroidTest compile are NOT_RUN because the configured Java 17 path is missing and the available Java 21 runtime lacks `javac`.
- API26/35 UI acceptance: NOT_RUN for this source. Earlier artifacts are bound to their original #155 source and are not reused.
- Google OAuth, real Google accounts/data, Cloud Egress, and production transport behavior were not activated.
- Required next steps: exact-head Android CI, review of fixture transport and test assertions, API26/35 UI JUnit evidence, then update the parent candidate with a reviewed change.

No UI pass is claimed by this report.
