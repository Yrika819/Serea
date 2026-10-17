# P8B authority architecture proof — 2026-10-17

**Result:** `AUTHORITY_ARCHITECTURE_SPIKE_PARTIAL`

**Status:** architecture proof only; no production dispatch, migration, Provider invocation, or production authority enabled.

## Executive result

The current code proves narrow mechanics: Rust ownership consumes the private
test Permit on a by-value attempt, Storage `BEGIN IMMEDIATE` rolls back tested
candidate writes together, and independent SQLite handles serialize the
candidate race. An external Cargo consumer cannot access raw `Tx` SQL or
import the test Permit/CommitProof symbols.

It does **not** prove a usable authorization boundary. An external Cargo
consumer can call public `Store::consume_grant_use` from `Store::transact`, can
append an Event through `Tx`, can relabel a public `PolicyEvaluation` outer
decision, and can implement `CapabilityProvider` while inheriting default
`Ready`. None of those APIs creates a Dispatch Intent or invokes Serea's Host
Provider, but each falsifies the idea that Grant use, Event presence, policy
snapshot, Provider ID, or `Ready` alone is an invocation proof. No Candidate
currently creates a cross-crate Permit from a real committed Intent.

Candidate B is refuted as a full authority boundary in the present dependency
graph: Storage cannot import Policy without a dependency cycle and cannot
observe the exact live Host Provider instance. Candidate A as implemented is
only a point-in-time TaskEngine evaluation and is unproven as a transaction
owner. Candidate C, a private Host-binary coordinator backed by typed
TaskEngine/Storage commit revalidation, is the most plausible topology to
prove next. It remains Proposed and needs Owner approval before runtime work.

## Exact intake baseline

- Repository: `Yrika819/Serea`; starting PR #28 HEAD
  `067c5e4e53034c797c0c87a3de55f9f1400d68b0`, parent/base PR #27 HEAD
  `c08615aa08b91abcd67f7ca4a3ac7a0512d7cb95`; PR #28 is OPEN/DRAFT,
  unmerged. It remains untouched.
- `main` is `e0acb0254820c381dd97828824bef86a1db5176c`. The original
  `/workspace/Serea` checkout is clean at that SHA. Work is isolated in
  `/workspace/Serea-p8b-authority`, on branch
  `p8b/authority-architecture-proof-2026-10-17`, based on the PR #28 HEAD.
- PRs #19–#28 remain open Drafts, unmerged, stacked in order; #14 remains an
  open Draft, unmerged, based on main. No review submissions were returned
  for #14 or #19–#28 at intake. No branch protection was changed.
- PR #28 exact-head Actions before this spike: Fast `38995459234`, Full
  `38995459187`, Native `38995459147`, SQLite portability `38995459224`, Git
  Hooks `38995459135`; all completed successfully. These are baseline evidence
  only, not validation of this branch.
- PR #19–#27 exact current heads each had successful Fast, Full, Native,
  SQLite portability, and Git Hooks runs. PR #14's latest fetched exact-head
  runs were Fast `38218999967`, Full `38218999839`, SQLite `38218999980`
  (success); the connector did not return Native/Git Hooks runs for #14.
- Cargo graph was read from `cargo metadata --locked --offline`: no dependency
  cycles. Normal edges relevant to the candidates are
  `TaskEngine -> Policy + Storage`, `Policy -> Storage`, `Storage -> Protocol`,
  `Capability -> Storage + Protocol`, and binary `Host -> Core + TaskEngine +
  Protocol + EventBus`. `serea-host` adds optional Capability/Storage edges
  only in its synthetic feature.
- Rust toolchains: stable `1.99.1`; MSRV `1.85.0`. Rustup required explicit
  `PATH=/workspace/.cargo/bin:$PATH`, `RUSTUP_HOME=/workspace/.rustup`, and
  `CARGO_HOME=/workspace/.cargo`; `cargo +1.85.0 fetch --locked` succeeded.
  The external consumer lock is constrained to Rust 1.85 and committed beside
  its isolated manifest; no workspace `Cargo.lock` was changed.
- `docs/git-safety.md`, ADR-0036/0039/0044/0045, the decision index, the P8B
  Contract Closure Package, and the Owner-ratified ExecPlan were read. ADR-0045
  ratifies directions only; architecture choice, durable counter/schema,
  freshness details, Event payload, Receipt/reconciliation and retention are
  still open.

## Authority graph and trust transitions

| Transition / owner | Current evidence | What it cannot prove for invocation |
|---|---|---|
| User/model proposal → P5 `PreparedActionV1` | Capability validates the trusted schema and pins descriptor, registry generation and input digest. | A prepared action is not P6 approval or P8 dispatch authority. |
| Registry/provider → `CapabilityAvailabilitySnapshotV1` | `build_for_generation` checks durable generation, descriptors and overlay, then calls `health()` once and stores it keyed by `ProviderId`. | Snapshot is frozen and has no Provider instance identity; it can become stale. `CapabilityProvider::health()` defaults to `Ready`. |
| Prepared action → P6 Policy | TaskEngine loads Task, Step, binding, descriptor and overlay, then evaluates a `PolicyInputV1`. | It returns `PolicyEvaluation`, documented as a point-in-time explanation. Re-reading occurs later for other lifecycle operations; no dispatch transaction consumes this evaluation. |
| Policy / Approval → Grant | Approval lifecycle seals exact Step members, verifies current status/expiry, and consumption enforces exact membership and one use per Step. | `Store::consume_grant_use` and `Tx::consume_grant_use` are public. A dependent crate can consume a Grant member using caller-supplied `now`; consumption is historical use, not invoke authority. |
| Grant use → Dispatch Intent / Budget / IDK / RequestId / Event | No production method exists in PR #28. Candidate schema and `cfg(test)` helpers exist only as test fixtures. | There is no runtime atomic commit result from which to issue a real cross-crate Permit. |
| Intent → Permit → Host Provider invoke | A private Storage `#[cfg(test)]` simulator models by-value consumption and fake effects. | It is disconnected from TaskEngine, Host, `CapabilityProvider`, and candidate Intent creation. No runtime `invoke` path exists in Host. |

`NOT_REQUIRED` from `complete_approval_gate_not_required` records that a human
approval is not needed for the enumerated actions. The source explicitly says
it grants no authority to execute a Capability Step. `CAPABILITY_REQUESTED`
means committed Intent by contract, but a downstream caller can append an Event
through `Tx`; Event presence by itself is not a permit or proof of an Intent.

### Threat boundaries

- **Untrusted data:** model/user/provider values are treated as data and pass
  protocol/schema/P6 checks. A model claim or Provider response does not mint
  authority.
- **Trusted compiled Serea crates:** Rust visibility and typed APIs limit raw
  SQL; the transaction closure exposes `Tx`, not `rusqlite::Transaction`.
- **External Cargo consumers:** a real path-dependent Cargo project can call
  public typed methods and implement public traits. It cannot access `Tx.inner`
  or import the test-only Permit/CommitProof names. It can relabel the public
  outer `PolicyEvaluation.decision`, consume a Grant use, append an Event,
  and provide its own `CapabilityProvider`. No current API interprets those
  caller-controlled facts as permission to invoke Serea's Provider.
- **Compromised same-user process or local database file:** outside crate
  privacy guarantees. Local SQLite modification/restore and compromise of the
  executable are not authenticated by Rust types or this test suite.

## Candidate comparison

| Candidate | Result | Evidence and exact obstruction |
|---|---|---|
| **A — TaskEngine-owned authority seam** | `UNPROVEN` (current TaskEngine-only point-evaluation shape is insufficient) | `evaluate_prepared_action` loads separate snapshots and emits explicitly non-authorizing evidence. `TaskEngine` depends on Policy and Storage, so it can coordinate them, but the current `Tx` API has no P8 commit operation and TaskEngine does not own Host's live Provider. Public external Store handles remain available. No A factory/Permit API exists to compile-falsify. |
| **B — Storage-owned self-validating public API** | `REFUTED` under the current crate graph | `serea-policy -> serea-storage`; reversing that edge makes a cycle. Storage cannot establish live in-process Provider instance or readiness. `Tx` raw SQL is private (external E0616/E0599), but public `Store::transact` composes existing typed grant-use and Event methods. No current method creates Intent or Permit. |
| **C — Host private coordinator + typed lower-layer transaction** | `UNPROVEN`, plausible | `serea-host` is a binary-only package and owns the live provider composition point. Existing edges permit an acyclic Host→TaskEngine/Policy/Storage dependency direction. A private Permit could be held only in Host; exact typed transaction facts, cross-crate surface, readiness synchronization and arbitrary-consumer behavior still need an executable proof. |

## External Cargo consumer probes

The reproducible fixture is
[`tools/p8b_authority_cargo_probe`](../../tools/p8b_authority_cargo_probe/).
It depends on the real local `serea-storage`, `serea-task-engine`,
`serea-policy`, `serea-capability`, `serea-core`, and `serea-protocol` crates;
it is a separate Cargo workspace and does not substitute toy implementations.

Command (from repository root):

```bash
python3 tools/p8b_authority_cargo_probe/run.py
```

Observed on both Rust 1.99.1 and 1.85.0, with the fixture's locked dependency
set:

- Positive compile: public `Store::transact` + `Tx::consume_grant_use`,
  `Tx::append_event`, outer `PolicyEvaluation` relabeling, and a downstream
  `CapabilityProvider` implementation that inherits default `Ready` all
  compile. This proves reachability only; no DB authority or Provider I/O was
  executed.
- Negative `Tx.inner`: expected `E0616`, private field.
- Negative `Tx.execute_batch`: expected `E0599`, no raw SQL method.
- Negative `serea_storage::SyntheticDispatchPermit`: expected `E0432`, no
  public symbol (the existing type is `#[cfg(test)]` private). The same
  external source attempts construction, `Clone`, `Copy`, and serde
  serialization/deserialization; Rust rejects the import before any of these
  operations can type-check. This demonstrates there is no currently
  reachable Permit type; it does not establish traits of a future type.
- Negative `serea_storage::CommitProof`: expected `E0432`, no current public
  proof type.

The negatives establish current API absence/visibility. They do not prove a
future public factory or cross-crate commit operation has no bypass.

## Real Storage evidence

Tests were run against actual `serea_storage::Store` and SQLite, with candidate
0009/0010 SQL applied only by disposable test fixtures and never added to the
ordinary migration catalog. The composition code lives in the Storage crate's
test-only `migration_0005_tests.rs`; it uses crate-private SQL inside
`Store::transact` because production Storage has no P8 typed operation. The
Provider readiness input is an enum explicitly named
`SyntheticReadyForCompositionOnly`; there is no Provider trait or Host runtime
in that transaction.

Rust 1.85.0 command:

```bash
cargo test -p serea-storage --lib real_storage_ --all-features --locked --offline -- --test-threads=1
```

Result: 8 passed, 0 failed, 633 filtered. The passing cases cover:

- Grant-use + candidate Intent + `CAPABILITY_REQUESTED` Event in one actual
  SQLite transaction; injected refusal after Grant use or Intent rolls back.
- Independent reopen sees committed Grant-use/Intent/Event together; duplicate
  same-Step Intent is refused.
- Budget 0 refuses without writes; budget 1 serializes two distinct Steps so
  only one commits; budget 2 commits two distinct Steps.
- Event insert failure, Grant revocation, Policy revision change, cancellation,
  deadline equality, and stale Lease generation refuse before partial writes.
- Two independent Store handles racing the same candidate Intent admit exactly
  one commit.
- Unresolved candidate Intent survives Task/Step/Grant-use deletion refusal and
  independent Event content expiry/reopen.

Rust 1.85.0 command:

```bash
cargo test -p serea-storage --lib p8 --all-features --locked --offline -- --test-threads=1
```

Result: 18 passed, 0 failed, 623 filtered. This covers test-only transaction
failure injection, synthetic Permit by-value/mismatch burn, Fake effect ledger,
SQLite reopen and no-replay cases. These results do not establish physical
power-loss durability, real external exactly-once behavior, genuine Host
readiness, or a production API.

The actual Stored Intent and the Permit simulation are separate tests. They do
not prove an Intent transaction causes Permit issuance. Candidate 0010 also
requires a Grant even for its synthetic OBSERVE path; no-Grant OBSERVE
authorization remains unspecified and unproven.

The revocation, cancellation, deadline, policy and lease cases mutate durable
state before attempting the candidate commit. They prove changed facts are
re-read by that test helper, not a barrier-scheduled race between those writers
and a real P8 authority operation. The same-Step and distinct-Step worker races
are concurrent; no P8 runtime operation exists at which to test the other
linearization races.

## RED/GREEN and failure injection

The existing real-Storage failure tests are RED refusal cases against stale or
changed facts, and GREEN cases against unchanged synthetic fixtures. They run
against the real Storage outer transaction and independent SQLite reopen.
Injected points after Grant use, after Intent insertion, and before Event
completion leave zero Grant-use/Intent/Event rows and restore the Grant count.
The test-only P8 authority transaction mechanics also inject failures after
Grant+Budget, after Intent, at Event insertion, and immediately before commit;
those cases use toy synthetic tables and are not counted as real-Storage proof.

Only the `cfg(test)` helper's synthetic readiness refusal was temporarily
weakened; no production guard was changed. The exact readiness test failed RED
in an isolated detached scratch worktree with `left: Ok(())`,
`right: Err(ConstraintViolation)` (exit 101). After removing the scratch
worktree, the untouched test passed GREEN (1 passed). This proves that the
fixture detects removal of its local test-only refusal; it does not establish
a production readiness guard.

## Local validation evidence

Environment was repaired by setting the existing workspace toolchain paths;
`rustup toolchain install 1.85.0 --profile minimal --component rustfmt
--component clippy` reported the existing toolchain unchanged. `cargo +1.85.0
fetch --locked` succeeded. No dependency mirror was used. Local workspace
build/test commands use `--locked` and `--offline` after fetch.

| Command | Result |
|---|---|
| `cargo test --workspace --all-targets --all-features --locked --offline` (stable 1.99.1) | PASS; complete workspace and all enabled features; 0 failed. |
| `cargo test --workspace --all-targets --all-features --locked --offline` (MSRV 1.85.0) | PASS; complete workspace and all enabled features; 0 failed. |
| `cargo check --workspace --all-targets --all-features --locked --offline` (MSRV 1.85.0) | PASS, 27.20s. |
| `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` (MSRV 1.85.0) | PASS, 19.33s. |
| `cargo fmt --all -- --check` plus fixture `rustfmt --edition 2024` (MSRV 1.85.0) | PASS. |
| Storage `real_storage_` selector (MSRV 1.85.0) | PASS; 8 passed. |
| Storage `p8` selector (MSRV 1.85.0) | PASS; 18 passed. |
| External Cargo positive/negative probes (stable 1.99.1) | PASS; expected compile diagnostics matched. |
| External Cargo positive/negative probes (MSRV 1.85.0) | PASS; expected compile diagnostics matched. |
| `cargo +1.85.0 fetch --locked` | PASS; no Cargo.lock changes. |
| `python3 tools/validate_docs.py docs` | PASS, 149 Markdown files. |
| `python3 tests/workspace_smoke.py`; workspace smoke unittest | PASS; 82 unittest cases. |
| Synthetic Host static boundary; Core API external boundary (stable 1.99.1) | PASS. |

The external Cargo fixture initially resolved `sha2 0.11.1` in a manifest
without an MSRV declaration; that release requires Rust 1.88. The standalone
fixture now declares Rust 1.85 and has its own compatible lock. No dependency
version in the Serea workspace lock was changed. The Core API boundary script
also failed once when run under the MSRV override because its generated
external lock selected the same incompatible `sha2 0.11.1`; rerunning it under
stable succeeded. The new external Cargo probe is added to Fast CI so its
positive/negative compiler behavior runs on the PR head. Its first Fast CI
attempt then exposed that an external Cargo workspace does not inherit the
root workspace's local `[patch.crates-io]` entries when Cargo runs offline.
The fixture now pins its own vendor paths and serde_json version; stable and
MSRV external compile probes both pass after that correction.

## Exact-head GitHub refresh

At intake all current PR stack branches #19–#28 were OPEN/DRAFT/UNMERGED; PR #14
was OPEN/DRAFT/UNMERGED separately. All current #19–#28 exact heads reported
successful Fast, Full, Native, SQLite portability, and Git Hooks workflows.
PR #28 reference head runs are listed above. Main remains the SHA above. No
workflow from those older heads is evidence for this new branch; a new Draft PR
must be checked at its own exact head before this proof is closed.

PR #29 is the single permitted new Draft PR, based on #28's exact HEAD. Its
first Fast run failed only at external probe dependency resolution as described
above. That fixture was corrected and re-run locally on both supported compiler
versions. Final exact-head CI run IDs and statuses must be added here before
classifying this spike as proved or partial.

## Proposed architecture package and restart point

[ADR-0046](../decisions/ADR-0046-p8b-authority-ownership.md) proposes Candidate
C solely for Owner selection. It does not accept the architecture or authorize
runtime changes.

Next restart point after Owner response:

1. Re-fetch main and PR #14/#19–#28 and confirm this branch still descends from
   the current PR #28 HEAD.
2. Resolve the approved topology and exact typed Host readiness/TaskEngine
   transaction seam.
3. In an isolated scratch crate/worktree, compile the approved external
   consumer attacks again and add real Transaction/Host API tests. Do not add a
   canonical migration, Provider invoke, Receipt, Retry or Production Memory.
4. Re-run cross-connection races for cancellation, Grant revocation, Policy,
   deadline and Lease generation at the actual commit point; reopen from an
   independent Store after each injected failure.

Resume command:

```bash
cd /workspace/Serea-p8b-authority
git status --short --branch
git rev-parse HEAD
```

## Remaining technical questions / Owner gates

1. Select A, B after a dependency refactor, C, or a different exact topology;
   identify which component privately creates the Permit.
2. Define a public-call-safe typed commit boundary that cannot use an
   unchecked `PolicyEvaluation`, fake Owner/readiness, or caller-controlled
   clock as authority. Explain how policy rules are re-evaluated against the
   current immutable revision while the Storage write is serialized.
3. Define Host Provider instance identity and fresh readiness synchronization
   before commit and immediately before invoke, including post-commit refusal.
4. Decide durable D1 counter/source, no-Grant OBSERVE semantics, restore and
   Task deletion interaction without refund or replay.
5. Specify the closed bounded D5 correlation payload and D4 retention/Forget/
   restore disposition without inferring it from Event or missing Intent.

No PR merge, close, retarget, history rewrite, main push or protection bypass
occurred. No production dispatch was enabled.
