# Personal Beta Lane 3: approved TaskProposal repository bridge

## State

Prepared for integration review. The real task repository bridge is available only through the existing explicit review/approval path. It is not wired into production UI and is not enabled by default.

## Review basis

Reviewed TaskProposalReviewSession and its approval/expiry states, TaskProposalConfirmer, the Fake/demo repositories and UX, the encrypted Room-backed EncryptedTaskRepository, current Android integration candidates and open PR stack (#82–#86), applicable ADRs, SECURITY.md, and docs/plans/Serea-high-privilege-security-gates.md. PR #86 is the branch base. No provider or real-world effect is introduced.

## Implementation

Added an internal durable proposal-apply port implemented by EncryptedTaskRepository. TaskProposalConfirmer uses it only after explicit approval and existing proposal validation. The durable path:

- Creates a task with its proposal receipt in one Room transaction.
- Applies update/complete with revision compare-and-set and receipt insertion in one transaction.
- Applies delete with the exact confirmed target, expected revision and receipt in one transaction.
- Stores no proposal body in the receipt; task content remains encrypted.
- Rejects proposal replay after receipt persistence, including after repository/database recreation.
- Propagates cancellation so the review session can retain OUTCOME_UNKNOWN; a committed receipt blocks a second application.

Fake and demo repositories keep their existing contract. The production UI remains unconnected.

## Security contract

- No mutation occurs without explicit approval.
- Non-create operations require the exact target ID to be confirmed.
- Review-session expiry and status handling remain authoritative; expired approval does not reach the repository.
- Revision CAS rejects stale targets. Proposal receipts prevent double apply.
- DELETE remains a distinct explicit operation and its receipt persists even though the task row is removed.
- The encrypted repository and existing Security Gate are unchanged; no schema migration or Security Gate relaxation is introduced.

## Validation

TaskRepositoryTest covers durable create/update/complete/delete replay prevention across repository recreation, explicit approval and matching delete target, expiry, and a lost-result fault where the review becomes OUTCOME_UNKNOWN while the durable receipt prevents replay. TasksDatabaseTest adds a Room close/reopen test for all four operations and receipt persistence. Instrumentation compilation is separate from execution; device/emulator execution is delegated to scheduled PR CI.

Local checks passed: targeted testDebugUnitTest (39 tests) for TaskRepositoryTest and TaskProposalTest; assembleIsolatedAndroidTest; lintDebug. The instrumentation test was compiled but not run against a device.

## Owner decisions

- Approve the eventual production review screen and the exact confirmation language/target display for destructive DELETE.
- Approve recovery guidance for process death and OUTCOME_UNKNOWN states; the durable receipt prevents replay but does not itself tell a user whether to retry a new proposal.
- Approve any future production UI enablement separately.

## Integration

This repository bridge can follow the Android UI repair stack and be reviewed independently of provider transports. A later UI PR should bind the current encrypted repository only after the Owner approves the confirmation and recovery presentation. Keep the Fake UX available during that stage. Do not call this Beta Ready until final integration HEAD contains all reviewed work.
