# G9-R1 Baseline Report

- Repository: `Yrika819/Serea`
- Captured: 2026-11-07 UTC; `origin/main` HEAD: `72dbd245c46fa111a22e9d85e22410278a259326` (2026-11-07)
- Required-check history in the supplied baseline record: 4/4; API 26 and 35 Formal: 105/105 each; native 12/12; SQLite portability 4/4. These are historical evidence entries, not re-run in this recovery pass.
- REST pagination returned 175 OPEN PRs. All 175 IDs are unique and match the existing 175-record snapshot. Per-PR REST file and commit endpoints succeeded for 175/175. Snapshot HEAD/base fields are retained; live revalidation of each branch head/base and check suite has not been completed.
- This pass did not rerun CI, deploy, connect real Google accounts, run a device, sign, or distribute.

## Source findings

Android has a Compose app, Task persistence (`android/app/src/main/java/app/serea/android/tasks/`), demo Google auth and fake provider flows (`googleauth/`, `calendar/`, `gmail/`, `handoff/`), and local encrypted task components. Existing readiness docs state no real provider/model integration ships. These source paths establish implementation presence only; no production service is LIVE_VERIFIED.

A concrete 175-PR table with API evidence and explicit unresolved comparisons is in `open-pr-inventory.csv/json`. None of the remaining PRs is assigned a definitive integration class without ancestry or patch equivalence proof. The 8 priority PRs were examined for changed paths and commits; further base-relative semantic review is needed.
