# Serea P8B — L1-A transaction facts proof report

作成日: 2026-10-17

判定: **`L1A_PARTIAL`**（合成データのdurable fact観測のみ）

## 要約

隔離worktreeに、TaskEngineが1回の実SQLite `BEGIN IMMEDIATE` 内で既存Storageの型付きreaderを呼び、Task/Step/Plan、active Policy snapshot、Registry/Descriptor/Overlay、Leaseと任意Grantのdurable factsをまとめて読み、PreparedActionとの不一致を列挙する読み取り専用観測境界を追加した。LeaseとGrantに不足していたreaderもStorageに限定型で追加した。

このAPIはPolicy判定、認可、Intent、CommitProof、Permitを返さない。Owner issuer、Worker origin、trusted clock、Host Provider instance/readinessは取得できず、P8B全体の認可成立ではない。L2 Intent writer、L3 Permit factory、Provider invokeは追加していない。

## ベース、作業領域、PR

- GitHub main: `e0acb0254820c381dd97828824bef86a1db5176c`
- PR #30: open Draft、head `ba16eaa7c45e51e30aa3185bd6f6a96484ba3c42`、base branch `p8b/authority-architecture-proof-2026-10-17`。PR #29 head `bb4c4d2b8a3bf015d735ab0ad519523f6b961299` は祖先。
- 元checkout `/workspace/Serea`: mainでcleanのまま。
- 隔離worktree: `/workspace/Serea-p8b-l1a`、branch `codex/p8b-l1a-facts-proof-2026-10-17`。作業開始HEADはPR #30 exact HEAD。
- 作業ブランチの最終commit、push SHA、Draft PR URL、exact-head Actionsはpush後に記録する。
- #31以降のopen Draftはintake時に存在しなかった。#14と#19–#30はopen Draft。branch protection APIは403で状態不明。#30の既存Fast/Full/Native 12/SQLite 4/Git Hooksは全成功だが、今回の検証ではない。

## 実装

- `crates/serea-storage/src/lease.rs`: `Tx::observe_lease_facts`。既存private `matching()`でLeaseGuardのorigin、Task/Step、owner、generation、unreleased fenceを確認し、acquired/expiry row factsを返す。時間判定はしない。ownerのDebug表示はredact。
- `crates/serea-storage/src/approval.rs`: `Tx::observe_grant_facts`。Grant/approval/task/plan/capability/version/registry/descriptor/action-set pins、status、uses remaining、stored expiry、sealとmember count、指定Stepのmember digests/use stateを返す。消費しない。Grant全member listは返さず、expiryもtrusted clockなしでは評価しない。
- `crates/serea-task-engine/src/engine.rs`: `TaskEngine::observe_l1_facts`。一つの`Store::transact` callback内でTask/Step/Plan、Registry generation、Step binding、Descriptor、Overlay、active Policy、Lease、optional Grantを読む。PreparedActionはselector/比較対象であり、入力factsとしてPolicy evaluatorへ渡さない。不一致はtyped mismatchで返す。結果型はprivate fieldsで構築不能、認可型ではなくDebugも公開しない。
- 関連追加: Storage/TaskEngine実SQLite tests、Storage public-method inventory、外部Cargo probe、ExecPlanと本報告。
- migration 0001–0008、migration catalog、schema、Cargo依存、feature isolation、ADR-0046状態は変更なし。`unsafe`、Provider呼出し、Intent writer、Permit、CommitProofなし。

## Fact provenance matrix

| Fact | 読み取り元 | 同一BEGIN IMMEDIATEか | 信頼区分と制約 |
|---|---|---:|---|
| Task/Step ID、kind/status、Task state/cancel/deadline、risk/data ceilings、budget、current plan revision、Step membership/input digest、Plan履歴・blob内容 | SQLiteの`Tx::load_task` | Yes | `VERIFIED_DURABLE`; selectorと比較 |
| Policy active revision、canonical rules/digest | SQLiteの`Tx::current_policy_revision` | Yes | `VERIFIED_DURABLE`; このreaderは評価しない。既存P6評価もcaller factsが必要 |
| Registry generation/activation/manifest、Task pin、Step binding | typed `Tx` registry readers | Yes | `VERIFIED_DURABLE`; pinごと比較 |
| Descriptor digest、capability/version/provider/implementation/properties | typed `Tx::get_descriptor_revision` | Yes | `VERIFIED_DURABLE`; PreparedActionは比較対象 |
| Overlay enabled/removed/experimental opt-in/revision | typed `Tx::get_capability_overlay` | Yes | `VERIFIED_DURABLE`; live snapshotとして観測 |
| Lease owner/generation/acquired/expiry/releaseとguard origin | `Tx::observe_lease_facts` +既存private `matching()` | Yes | DB rowは`VERIFIED_DURABLE`; owner値とclockは認証されない |
| Grant ID/status/task/plan/member/action-set digest/uses/seal/expiry/selected-step use | `Tx::observe_grant_facts` | Yes | 選択StepのDB factsは`VERIFIED_DURABLE`; expiryは未評価。全member vectorは未取得 |
| Prepared arguments digest、requested_by、data class | callerのPreparedAction。digestだけdurable Stepと照合 | selectorは同一Txで照合 | `HOST_PROVIDED_UNVERIFIED`;他の属性はDBから信頼して再構成できない |
| device posture、destructive opt-in、automation context | 認定されたdurable sourceがない | No | `UNAVAILABLE_OR_UNRATIFIED` |
| Owner issuer/auth、Worker principal、trusted time | 独立した検証済み発行元なし | No | `UNAVAILABLE_OR_UNRATIFIED`。caller値を信用しない |
| exact live Provider instance、readiness/lifecycle | Host private lifecycle契約なし。provider IDやdefault Readyのみ | No | `UNAVAILABLE_OR_UNRATIFIED` |
| Intent、budget charge、IDK、RequestId、commit result、Permit | 今回のwriter/factoryなし | No | 対象外。APIは生成しない |

`VERIFIED_WITH_TRUSTED_EXTERNAL_SOURCE`に分類できるOwner/Worker/Clock/Provider事実は今回存在しない。SQLiteファイル改変/復元、Host/process compromise、power loss、外部効果はこの証明の対象外。

## 実SQLite tests / RED-GREEN

| Test | RED/GREEN・DB・結果 | 限界 |
|---|---|---|
| `task_engine_policy_input_is_derived_from_current_pinned_durable_facts` | MSRV/Stable PASS。file-backed StoreでTask/Plan/Policy/Registry/Descriptor/Overlay/Leaseを同一transaction観測。alternate PreparedAction input digestはmismatch。後続Overlay変更を別Storeから読み、close/reopen後に`verify_integrity` PASS。callerがmutable outer PolicyEvaluation.decisionをAllowに書換えても入力に使われない。 | policy evaluation/authorizationではない。observer固有のbarrier-held writer raceは未実施 |
| `lease_fact_observation_is_read_only_fenced_and_does_not_launder_rollback` | PASS。実SQLite。committed guardでrow読取、roll back acquisition由来guardはLeaseFenced、観測前後のlease row同一。 | unit fixtureはin-memory。file-backed一体化testは上記 |
| `read_only_grant_fact_projection_observes_member_and_use_without_consuming` | PASS。file-backed typed Store fixture。member/nonmember、unused/used/exhaustedを観測。projection前後でGrant-useとevent-content counts不変。close/reopenとintegrity PASS |
| `authenticated_revocation_is_grant_only_atomic_and_preserves_prior_consumption` | PASS。独立Store operation後、projectionがREVOKED/sealed/member digestを読む |
| policy change | `storage_tests::active_policy_change_waits_for_transaction_scoped_p6_evaluation` PASS、独立Store writer serialization |
| cancellation | `independent_connections_serialize_task_cancellation_against_authenticated_response` PASS |
| Grant revoke race | `independent_connections_serialize_r2_member_consumption_against_revocation` PASS |
| Lease reclaim race | `committed_reclaim_wins_before_cross_store_old_outcome_attempt` PASS |
| RED/GREEN | disposable detached worktree + isolated target。exact Step input-digest guardを無効化→対象assertion失敗、exit 101 (RED)。guard復帰→同じtest PASS、exit 0 (GREEN)。worktree/target cleanup済み |

既存のpolicy/cancellation/grant/lease raceは別々のStorage fenceを証明し、`observe_l1_facts`自体と一体化したwriter raceではない。fake Owner/Worker/time/Provider-ready攻撃はruntime authorityを生まないが、Host invoke endpoint自体がなく、今回のconsumer probeはcompile surface evidenceに留まる。

## External Cargo consumer

`python3 tools/p8b_authority_cargo_probe/run.py` Stable 1.99.0 / Rust 1.85.0: PASS。両版でtyped read-only projection呼出しはcompileでき、`L1FactObservation`のstruct literalはprivate fieldsで拒否。raw `Tx.inner`はE0616、SQL executorはE0599、現存しないPermit/CommitProof importはE0432。E0432は現行symbolの不在であって将来factoryの安全証明ではない。consumer sourceではcaller-owned default-Ready Provider、PolicyEvaluation relabel、Grant-use+Event compositionがcompileする。これらはinvoke権限ではなく、probeは外部効果を呼ばない。

## Local QA / CI

- Rust 1.85.0: locked fetch、fmt、workspace check/test all-target/all-features、Clippy `-D warnings`、release Host `--no-default-features` all PASS。Storage libは643 tests、workspace test command exit 0。
- Stable 1.99.0: workspace all-target/all-features tests、fmt、check、Clippy `-D warnings`、release Host `--no-default-features` all PASS。vendored serde_jsonの既存`usize::max_value` deprecation warningあり、変更コード由来でない。
- 両toolchain `cargo metadata --locked --offline --format-version 1`: PASS。
- 最終ソースでworkspace all-targets/all-features testはStableとRust 1.85.0両方exit 0。Stable/MSRV fmt、workspace check、Clippy `-D warnings`、release Host `--no-default-features` checkもPASS。全featuresはdevelopment test surfacesを含むQA設定であり、production Provider/Memoryを有効化していない。
- docs validator: 152 Markdown PASS。workspace smoke PASS。unittest 82 PASS。static Synthetic Host boundary PASS。Core API boundaryはPATH未設定の初回を明示的Rust PATHで再実行してPASS。`git diff --check`: PASS。
- 今回branchのexact-head GitHub Actions: push後に追記。main/PR #30の既存workflow結果は今回のHEADの検証に転用しない。

## 未証明事項とOwner判断

1. Owner issuerの認証型・発行主体・session freshness/revocationを定めること。
2. Worker principalの由来をlease acquisitionに結び、偽owner文字列を拒むこと。
3. Host trusted clockとrollback/freshness契約を定めること。
4. Host所有Provider instance identity/lifecycle lockとfresh readiness precommit/preinvoke契約を定めること。
5. Grant不要OBSERVEの正確な認可形を決めること。
6. 選択StepのGrant memberだけでよいか、sealed全member setをTxで再読・照合するか決めること。
7. observer自身へのbarrier-held independent-Store raceを追加するか決めること。

次に推奨する工学作業は、OwnerがWorker/clock/no-Grant OBSERVEを決めた後、`p5e_capability_binding.rs`にobserver-specific second-Store barrier raceを足し、引き続き外部trust root不足時はblockedのままにすること。これはL1-B以降のOwner判断であり、本結果はL1-Aの部分的成功である。

## Git / 再開点

- ADR-0046はProposedのまま。schema/migration/catalog/feature/architecture versionに変更なし。
- Final branch HEAD / commit / Draft PR / exact-head workflow run IDsはpush後に記録する。
- 再開worktree: `/workspace/Serea-p8b-l1a`。基点HEAD: PR #30 `ba16eaa7c45e51e30aa3185bd6f6a96484ba3c42`。次の対象: `TaskEngine::observe_l1_facts`、`Tx::observe_lease_facts`、`Tx::observe_grant_facts`、および前項のOwner判断。
