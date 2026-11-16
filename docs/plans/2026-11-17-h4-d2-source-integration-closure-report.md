# Serea H4-D2-Final — Source Integration & CI Closure

**総合判定: `PARTIAL`（同一SHA CI の環境障害を調査中。最终结果で確定する）**
**Production signing: 未承認・無効 (`PRODUCTION_SIGNING_AUTHORIZED=false`)**
**Merge / Draft解除 / Force Push / 既存PR変更: 未実施**

## 1. 最新main / 隔離環境

| 項目 | 値 |
|---|---|
| canonical main HEAD / Tree (2026-11-17 取得) | `317c601760ab7148afd979ee9ef092838d76a072` / `2017e9b15c09724459afe3f59c4740bb4a6c04fe` |
| Branch Protection Required checks | `Linux fast checks`, `Linux stable full validation`, `Android Emulator Required Gate`, `Linux MSRV 1.89.0`（enforce_admins=true） |
| 専用worktree | `/Users/yuta/.local/share/opencode/worktree/eb2ced/h4-d2-signing-source-closure` |
| 専用branch | `codex/h4-d2-signing-source-closure` |
| Git identity | `yrika <143304522+Yrika819@users.noreply.github.com>`（既存承認済み設定と一致） |
| 既存PR確認 | #244 は本PR。#243 はロードマップ文書Draft（非競合・未変更）。#242/#240/#237 等はマージ済み。#232/#239/#238 等は未統合で未変更 |

## 2. Draft PR

- URL: https://github.com/Yrika819/Serea/pull/244
- Title: `H4-D2: finalize isolated signing and receipt-backed recovery`
- State: **OPEN / Draft**（Draft解除・Mergeせず）
- PR HEAD (commit): `8c53bab33e5abb167f94a5d096fe9555b14a2232`
- PR Tree: `2de9c134c7e976227c0f0821e9ec7528dc62d34d`
- 既存dirty/untracked作業ツリー（`/Users/yuta/Desktop/Serea`）は reset/clean/stash せず保護。

## 3. Owner判断の記録（AC-01）

- **O-07 を Owner Decision Ledger に追加**: 「条件付き同一UID残留リスクと運用条件の承認＋署名基盤機能凍結」。承認日 2026-11-16 の会話。
- 承認内容: 開発／署名環境分離、専用Ownerアカウント、署名環境でのAIエージェント・非信頼コード・Gradle禁止、署名資産／状態ストアへの第三者アクセス禁止、移送前独立検証、同一UID攻撃者に対する完全防御を保証しないこと。
- **明示的に未承認（O-07 scope noteに記載）**: 正式ホスト条件の成立確認、正式鍵生成、Production有効化、正式APK署名、Merge、配布、実OAuth／実AI。残留リスク承認と運用条件成立確認を混同しない。

## 4. 公開ファイル一覧（allowlistと一致、28件）

- signing: `tools/serea_signing.py`, `serea_apk_signer.py`, `serea_release_ledger.py`, `serea_identity_registry.py`, `serea_signing_state.py`, `serea_signing_witness.py`, `serea_enrollment_authority.py`
- tests: `tests/test_serea_publish_toctou.py`, `test_serea_sign_apk_cli_e2e.py`, `test_serea_apk_signer.py`, `test_serea_apk_signer_e2e.py`, `test_serea_enrollment_authority.py`, `test_serea_identity_registry.py`, `test_serea_release_ledger.py`, `test_serea_signing_witness.py`, `test_serea_signing_cli.py`, `run_disposable_signing_e2e.py`（テストimport依存のみ。レガシー `build-public`/`build-private` 経路は現行成功証跡ではない）
- build/CI: `.github/workflows/android.yml`, `android/app/build.gradle.kts`, `android/gradle.properties`, `tools/validate_docs.py`, `tools/verify_android_build_provenance.py`
- docs: `docs/plans/2026-10-26-personal-beta-owner-decision-ledger.md`, `2026-11-15-owner-production-signing-launch-runbook.md`, `2026-11-16-h4-d2-final-dynamic-acceptance-report.md`, `2026-11-16-h4-d2-final-recovery-closure-report.md`, `2026-11-16-h4-d2-signing-closure-report.md`, `2026-11-17-h4-d2-signing-final-closeout-report.md`

除外: `.superpowers/`, `.local/`, ビルド出力, 個人Gradle設定, 鍵/PKCS12/Vault/状態ストア, 個人パス含有文書, 署名無関係のCalendar/Gmail/AI文書。

## 5. 最終ソースハッシュ（公開候補・主要）

```text
a17b33860f2cda3a2cd8ea8b4ffcbed60b5999eec5d92b250a4adf820f715bce  tools/serea_signing.py
4b7945a381ccc6f98a3fec1bf9df400be49ce79b0915d0c8200d111a2cab0b85  tests/test_serea_publish_toctou.py
4a5ba8fdb86a836a66c30f3117835e8a8de184076ee47e9456b5647f8e12b0bc  tests/test_serea_sign_apk_cli_e2e.py
33ab0d32427f6cce1628594dcfc6ae645669c8cad16f5aec8ef177f26045c0cf  tools/serea_release_ledger.py
d2806925421c45181e536a8d70cd74995521cca3b97ba01fe20dcf76b5d78a40  android/app/build.gradle.kts
06a55a1fcba0484a760fe08178a21b8de21f441476a7f535119dad0850ae6d79  android/gradle.properties
e587520bd6698bad26d626719f60325a7ad70485c0bb30c16ed4122945bd95fe  .github/workflows/android.yml
```

全28件の SHA-256 manifest を Phase D で生成済み。HEAD `8c53bab…` の Tree に完全一致。

## 6. 機密情報監査（AC-04）

- 公開対象28件をオフライン走査。API key / Access Token / 認証情報 / PEM秘密鍵 / PKCS12 / 個人homeパス / ユーザー名 / メール / トークン様文字列: **0件**。
- 検出パターンはすべて benign と分類: コンテンツSHA-256、GitHub Action SHAピン、base64様APKファイル名、getpass プロンプト文言（`passphrase:` ラベル）のみ。
- 値そのものはログ・レポート・AIコンテキストへ出力していない。実際の秘密設定・正式資産は読取っていない。
- Workflowの.new/変更差分（`.github/workflows/android.yml`）は既存CIへ署名unittestステップを追加するのみで、外部送信・秘密利用・権限拡大なし。

## 7. push前検証（AC-05）

| 検証 | 結果 |
|---|---|
| Python Signing/Recovery/Ledger/TOCTOU/Build Provenance 一括 | **100/100 PASS** |
| py_compile（署名tools + 変更tools + 主要tests） | PASS |
| docs validator | PASS（219 Markdown files、壊れたリンクなし） |
| `git diff --check` | PASS |
| Production fail-closed / 旧Gradle署名経路拒否 / Public-Private分離 | 既存テストでPASS（201行の `build-public`/`build-private` 拒否、証明書相異チェック等） |
| 公開ファイル SHA-256 manifest | 生成済み（全28件） |

Android 88/88 と実暗号化Vault証跡は過去工程の成功証跡を利用（変更なしのため不要な反復を避けた）。macOS固有の暗号化Vault証跡をLinux CI結果と混同しない。

## 8. Required CI / 同一SHA（AC-07）

**初回 PR HEAD `8c53bab33e5abb167f94a5d096fe9555b14a2232` の Same-SHA CI:**

| Required check | 結果 |
|---|---|
| Linux fast checks | **success** |
| Linux stable full validation | **success** |
| Linux MSRV 1.89.0 | **success** |
| Android Emulator Required Gate | **failure**（下記） |

- `Linux fast/stable/MSRV` は同一SHAで success。
- `Android Emulator Required Gate` は、上流ジョブ `Android Unit` の失敗と `HOST_INFRA: APT not attempted: official Ubuntu DNS or TLS-verified HTTPS endpoint unavailable`（`Android Host Probe`）に連鎖して failure。Unit 失敗の直接原因は**私が追加した CI ステップ**が macOS 専用の `tests.test_serea_signing_cli`（Temurin/JDK を `~/Library/...` に要求）を `ubuntu-24.04` で実行したこと（H-1 に関連）。
- 非Requiredの `Build target... Test IME` は診断 workflow の scope guard が本署名PRの変更範囲を「診断対象外」として正しく拒否したもの。ガード緩和は禁止のため修正しない。

**H-1 最小修正（2nd commit で反映）:**
- `.github/workflows/android.yml`: Linux Unit ジョブから macOS 専用 `tests.test_serea_signing_cli` を除外し、portable な4モジュールに限定（同 suite は macOS/ローカルで100/100 PASS済み）。
- `tools/test_personal_beta_identity.sh`: Gradle が unsigned APK (`app-beta-unsigned.apk`) を出力する契約に合わせ、テスト所有の一時PKCS12でローカル `apksigner` 署名してから emulator 検証。Required gate の証明書変更セマンティクスは維持（本番鍵・Gradle署名設定は不使用）。
- `tests/run_disposable_signing_e2e.py`: 廃止された `:app:validatePublicBetaSigning`/`validatePrivateBetaSigning` 参照を現行CLIの fail-closed 検証（identity 分離・独立PKCS12・retired Gradle経路拒否）へ置換。
- `tools/serea_signing.py`: SDK/build-tools 参照を `_sdk_build_tools()` へ集約（動作不変）。
- O-07 scope note の参照先レポートを公開候補へ追加（dangling pointer 解消）。

修正後、100/100 回帰・py_compile・bash -n・docs validator(220)・diff --check はすべて再PASS。新SHAで Same-SHA CI を再実行する。

## 9. 独立レビュー（AC-08）

**初回レビュー（SHA `8c53bab…`）: Critical 0 / High 1 / Major 2 / Moderate 2 / Minor 2 → `CHANGES_REQUIRED`。**
- High H-1: unsigned Beta 契約と既存 verifier（`test_personal_beta_identity.sh` / `verify_personal_beta_apk.sh`）の不整合 → 上記最小修正で解消。
- Major: 廃止 Gradle Guard タスク参照（→ 現行CLI検証へ置換）、macOS SDK ハードコード（→ helper集約）。
- Moderate: `android.enableResourceOptimizations=false` の全局適用（署名パス専用として既存証跡どおり維持。サイズトレードオフはOwner判断事項として記録）、O-07 の未存在レポート参照（→ レポートを公開候補へ追加）。
- 署名安全コア（TOCTOU、non-overwrite、消費コード再利用防止、Receipt-backed無再署名復旧、lane分離、Production fail-closed）は sound と確認。テスト削除・Required無効化なし。シークレット/PII clean。O-07 は適切にスコープされた承認記録と確認。

**再レビュー:** H-1 修正差分（上記4ファイル＋レポート）のみを対象に同一SHAで実施→（結果反映）。Terra は 401 で利用不能のためローカル代替レビュアーを使用。

## 10. AC 別判定

| AC | 判定 | 根拠 |
|---|---|---|
| AC-01 Owner判断・機能凍結記録 | PASS | O-07 を Ledger へ記録。未承認項目を明記 |
| AC-02 最新main/既存PR確認 | PASS | main `317c6017`、Required 4件、PR一覧取得済み |
| AC-03 隔離worktree安全選定 | PASS | 専用worktree/branch、明示allowlist、`git add -A`禁止 |
| AC-04 機密監査PASS | PASS | 公開28件スキャン、値出力なし |
| AC-05 回帰・安全契約PASS | PASS | 100/100、py_compile、docs、diff --check |
| AC-06 Draft PR作成 | PASS | #244（Draft維持） |
| AC-07 同一SHA Required CI | **PARTIAL** | 初回SHAで3/4 success・GateはH-1/HOST_INFRA連鎖失敗。修正後SHAで再検証中 |
| AC-08 独立レビューCritical/High 0 | **PARTIAL** | 初回はHigh 1（H-1）。修正後再レビュー待ち |
| AC-09 ロードマップ案・3レーン準備 | PASS | 更新ドラフトとLane A/B/Cを準備 |
| AC-10 成果物保護・権限境界 | PASS | 既存worktree無変更、禁止操作なし |

## 11. 残存Ownerゲート / ロードマップ更新案

- 残存: 正式ホスト硬化・FileVault/SIP/OCLP、暗号化外部バックアップと独立保管、正式鍵生成、同一SHA Required CI完了、独立レビュー、Merge判断、Production有効化判断、実OAuth/実AI承認。
- ロードマップ更新案: `docs/plans/2026-11-17-h4-d2-signing-source-closure-roadmap-update-draft.md` に準備済み。既存ロードマップの履歴を保持し「0.2 現在状態への同期 — 2026-11-17」を追加する方式。文書専用の後続PR向けで、署名PR #244 には混入させていない。既存PR #243 の変更なし。
- 後続3レーン: Lane A 署名・リリース管理（正式ホスト／Production設計準備／鍵・バックアップ）、Lane B Google統合（Private-only/Primary-only契約確認／Calendar準備／Gmail Mockと権限整理）、Lane C AI統合（推論候補／Provider abstraction／データ分類・Egress・予算／Fake provider検証）。今回はB/Cの実装・実接続を行わない。

## 12. 権限境界

- 実施: GitHub読み取り、必要なfetch、専用worktree/branch、明示allowlistのcommit、新規branchへの通常push、Draft PR作成、CI確認、オフライン検証、内部独立レビュー。
- 未実施: main直接commit/push、Draft解除、Merge、Force Push、既存PR変更、Branch Protection変更、Production有効化、正式鍵/APK署名、Keychain読取、実OAuth/AI、APK配布・実機、OS設定変更、Astra、他プロジェクトへの干渉。
