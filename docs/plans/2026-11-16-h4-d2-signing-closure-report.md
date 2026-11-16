# Serea H4-D2 Signing Closure — Final Report

**総合判定: `PARTIAL`**  
**Production署名: 未実施・未承認 (`PRODUCTION_SIGNING_AUTHORIZED=false`)**

## Baseline and scope

- HEAD: `317c601760ab7148afd979ee9ef092838d76a072`
- Tree: `2017e9b15c09724459afe3f59c4740bb4a6c04fe`
- 作業中にHEAD/Treeを変更していない。既存のdirty/untracked成果物は保持。Commit/Push/PR/Merge、外部通信、Gradle、正式資産・Keychain・外部媒体操作、APK配布・実機操作、OS設定変更は行っていない。
- Astraは使用していない。
- 対象コードはDisposable専用署名とmanaged publication receipt/recovery診断。正式処理は引き続きfail-closed。

## Acceptance criteria

| AC | 判定 | 根拠 |
|---|---|---|
| AC-01 公開・台帳整合性 | **PARTIAL** | Ledger-bound managed attempt store、receipt、非上書きhard-link公開（sidecar先、APK最終）を使用。`BUILT`失敗テストとSIGKILL後の状態確認はPASS。ただしsidecar/APK公開境界そのものの強制終了試験や全永続化境界の証明は未実施。 |
| AC-02 Recovery CLI | **PARTIAL** | `ledger-recovery-plan`はread-onlyで有効receipt、receipt欠損/破損、receipt-less attempt、witness-only reservationを診断し、異常時にUNKNOWNを報告する。`ledger-abandon`はmanaged receiptを検出すると拒否。再公開・隔離の自動操作はなく、Ownerの手動照合が必要。 |
| AC-03 実プロセス障害注入 | **PARTIAL** | Disposable子プロセスをSIGKILLし、APK署名後・Ledger BUILT前の境界でversionCode消費、未公開状態、発見可能なreceipt/artifactを確認。その他の要求境界は未試験。 |
| AC-04 並行処理 | **PARTIAL** | 別プロセスの署名ロック競合を関連Python回帰で確認。並行する署名対署名、署名対Recovery、同一出力先公開の全競合マトリクスは未実施。 |
| AC-05 実暗号化Vault上の両lane署名 | **NOT_RUN** | 本実行はdirectory mount fixture。Disposable encrypted disk imageの生成/mountは実施していない。 |
| AC-06 暗号化backup/隔離restore/旧Vault拒否 | **NOT_RUN** | 今回の証跡なし。 |
| AC-07 Python/Android回帰 | **PARTIAL** | 対象Python回帰94/94 PASS（内訳は下記）。Android Debug/Private OAuth Fake Gradle回帰は未実行。 |
| AC-08 最終独立レビュー | **PARTIAL** | 実装担当と別の内部レビュアーがreceipt/recovery/publication範囲を静的レビューし、最終スコープ内Critical/High 0件。これは全署名システムの完全監査でも動的検証でもない。 |
| AC-09 成果物保護・権限境界 | **PASS（観測範囲）** | HEAD/Tree維持。Disposable以外の鍵/Vault/台帳を操作せず、禁止操作なし。全ホスト状態の独立証明ではない。 |
| AC-10 Owner手順・公開/CI準備 | **PARTIAL** | Owner runbookを現行のreceipt/recovery制限に同期し、docs validator PASS。正式ソース選定、公開allowlist/秘密情報の包括監査、同一SHA CI、Draft PR準備は未完了。 |

## Implementation and Recovery

- `tools/serea_signing.py`:
  - receipt schemaで`schema`/`lane`/`state`の型を検証し、無効なNUL出力パスを拒否。
  - 成功済みPUBLISHED receiptもRecovery plan上で既知attemptとして追跡し、誤って`BUILT_WITHOUT_RECEIPT`として重複報告しない。
  - ファイル観測中の`OSError`/`ValueError`/`SigningError`は証拠欠損として扱い、Recovery診断をUNKNOWNにする。最新BUILTの整合性が確認できなければ次の署名を拒否。
  - valid receiptのないattempt directoryや状態不一致は自動復旧せず、診断・Owner照合に留める。
- `tests/test_serea_sign_apk_cli_e2e.py`:
  - 正常PUBLISHEDのpending誤報、unhashable lane、NUL/ENAMETOOLONG出力パスの回帰テストを追加。
  - 子プロセスSIGKILL、二laneの実Disposable fixture署名・再生拒否の既存検証を再実行。
- `docs/plans/2026-11-15-owner-production-signing-launch-runbook.md`を最終観測状態に更新。
- `ledger-recovery-plan`は診断専用。現実装に自動republish/隔離コマンドはない。Receipt-backed attemptは保持し、同じversionCodeで再署名せず、ハッシュ・Ledger・Vault等をOwnerが手動照合する。異常が不明な場合は停止する。

## Verification evidence

- `python3 -m unittest tests.test_serea_sign_apk_cli_e2e -q`: **7 PASS**。両laneのAPK fixture実署名、単一署名者、Public/Private別証明書、Application ID/versionCode保持、API 26/35検証を含む。
- `python3 -m unittest tests.test_serea_release_ledger tests.test_serea_identity_registry tests.test_serea_signing_witness tests.test_serea_enrollment_authority tests.test_serea_signing_cli tests.test_serea_apk_signer tests.test_serea_apk_signer_e2e tests.test_android_build_provenance -q`: **87 PASS**。
- 合計: **94 Python tests PASS**。期待されたCLI negative-case usage出力、duplicate ZIP fixture warning、未検証mountpointを残す既存warningあり。mountpointには触れていない。
- `py_compile`: PASS。`git diff --check`: PASS。
- `python3 tools/validate_docs.py docs`: report作成前に239 Markdown files PASS。報告書作成後の再実行は最終確認対象。
- Gradle/Android回帰: **NOT_RUN**。本キャンペーン境界でGradleは実行せず、対象Serea未署名APK E2Eも実施していない。

### Latest Disposable fixture signing evidence

| Lane | Application ID | versionCode | APK SHA-256 | Certificate SHA-256 | API verification |
|---|---|---:|---|---|---|
| Public | `app.serea.android.beta` | 42 | `bd8ef1db471d1cb38cb4a4a3e8f93560c8ea47524bc3a0ddfd0b7543fff4ee06` | `c2de8e50ae5d32903480e7028a42772e9454bdb36b78a2a888a60592293224bb` | 26, 35 PASS |
| Private | `app.serea.android.privatebeta` | 71 | `7e5ac48736f1bcff249d8fa3f400908b25d571624dbaae61694bd60908a1f5cb` | `83553478df71a66d5b101e03f26f1c6f5d37621f6a7fbcde413322ea179fe56d` | 26, 35 PASS |

These are disposable generated APK fixtures, **not Serea application APKs**, and are not distributable artifacts.

## Independent review

A separate internal reviewer (not Astra) performed static, read-only review of receipt parsing, publication diagnostics, recovery reporting, latest-BUILT signing gate, and the associated tests. The review found and the implementation/tests addressed: (1) healthy publication misreported as missing, (2) unhashable JSON receipt fields aborting recovery, and (3) malformed/NUL/overlong output paths aborting recovery. Final scoped review reported no unresolved Critical/High and no demonstrated signing bypass. Dynamic crash/concurrency coverage remains limited as listed above; review is not an external audit.

## Source SHA-256

```text
bcc9475f85c5148e0042163ebd416e65e139a6024106a912c246351f8e37102a  tools/serea_signing.py
b0a5dd7576bf5cbcbd281982c058e391f4fa10bee60c9fe625d48b972649ddc4  tests/test_serea_sign_apk_cli_e2e.py
```

## Owner gates and deferred work

- Select and secure a supported independent signing host; resolve FileVault/SIP/OCLP concerns. Prior read-only host observations recorded FileVault Off and a nonstandard/unsupported SIP configuration; no host setting was changed.
- Establish formal identities/Vault only after formal source selection and separate Owner authorization; validate encrypted backup and isolated restore on Owner-controlled media.
- Build and attest actual Serea unsigned Public/Private APKs from the selected reviewed source; complete same-SHA CI and Android API 26/35 required checks before any formal signing decision.
- Complete real encrypted Disposable Vault integration and backup/restore E2E before claiming that acceptance gate.
- Expand only the essential fault matrix if needed: sidecar/APK publication boundary, after-BUILT/pre-publication, recovery interruption, signer/recovery/output races. Do not treat untested boundaries as proven.
- Automatic republishing and fully automated recovery are deferred; uncertain states require manual Owner reconciliation. Consumed version codes are never rolled back or reused.

This report does not establish host trust, backup readiness, formal Authority readiness, or permission to sign/distribute a Production APK.
