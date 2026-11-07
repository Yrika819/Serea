# G9-R4 Status

- Main: `72dbd245c46fa111a22e9d85e22410278a259326`.
- At start PR #219 was OPEN/Draft, HEAD `771dcd65c10149b0f89aa3ae87c6f7dda0d3b708`, merge state BLOCKED while native stable/MSRV checks were still in progress. Required Gate, Linux fast and identity had passed. The existing COMMENTED review targeted `c2249623d881421e05dd05f607be65c333367dce`; it preceded two documentation-only commits and did not match the starting HEAD.
- The 9 additional PRs were fetched; base-relative and main-relative patches, commit patch IDs/parents, check summaries and reviews are recorded in `local-model-pr-evidence.json` and `pr-diffs/`. All statuses refer to each exact PR HEAD. No reviews were recorded for these 9 PRs.
- #219's R4 updates specify the Fake Backend-only composition spike and evidence-based dispositions. Latest CI/review will be recorded after this update is pushed.
- R4 audit does not implement G10-A, add a model, enable external service, or change main.

Decision: no code from the 9 PRs must be cherry-picked before the fake-only spike; #185's fresh-consent rule is mandatory as an acceptance condition and is already supported by process-local one-use registry primitives in main. The full chain must be tested. Native runtime and device-memory/ARM64 readiness remain gates before any real inference.

## Exact starting-HEAD checks (771dcd65c10149b0f89aa3ae87c6f7dda0d3b708)

GitHub commit check-runs are SHA-pinned. At review time: Identity PASS; Android Emulator Required Gate PASS; RustSec PASS; Linux stable x86_64/ARM64 PASS; SQLite portability open/open-success jobs PASS; Linux MSRV and Linux stable full validation IN_PROGRESS; several Windows/macOS native jobs and Windows synthetic validation IN_PROGRESS. Android Formal API 26/35 and integration suites were SKIPPED because this docs-only change did not trigger them. Existing COMMENTED review SHA was `c2249623d881421e05dd05f607be65c333367dce`, so a new review is required for the R4 documentation and classification changes.

Independent G9-R4 technical review was completed against HEAD `0bd8d7ad43333e911a88e28d9054b0979f2a8413`. Reviewer verified all nine saved diffs and merge-bases against Git and the stated ancestry. Corrections incorporated in a later commit: #182 is NOVEL as the newest audited provenance stack; Adapter fixture hashing and asynchronous close semantics are now reflected in G10-A; MainActivity stays Demo and the fake route is test-injected. A final COMMENTED review must target the current post-correction HEAD.
