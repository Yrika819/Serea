# Serea H4-D2 — Signing Final Closeout

**判定: `OWNER_RISK_REVIEW_REQUIRED`** (独立レビュー判定 `APPROVED_WITH_RESIDUAL_OWNER_RISK` を反映済み)
**Production signing: 未実施・未承認 (`PRODUCTION_SIGNING_AUTHORIZED=false`)**
**Feature freeze: 技術条件は成立。残存する条件付き重大リスクのOwner判断のみが未決。**

## 1. Baseline / authority boundary

- Repository `Yrika819/Serea`; expected HEAD `317c601760ab7148afd979ee9ef092838d76a072`, Tree `2017e9b15c09724459afe3f59c4740bb4a6c04fe`. 最終状態も同一（未コミット・未マージ、dirty/untracked成果物は保護）。
- 作業開始時に存在した dirty/untracked の署名関連源文件・証跡・個人環境は変更／削除していない。新規作成は `tests/test_serea_publish_toctou.py` と本報告書のみ。
- 未実施: GitHub書込（commit/push/PR/merge）、外部通信・ダウンロード、正式鍵/PKCS12/Vault/台帳へのアクセス、Keychain、実OAuth/Google Cloud/実AI、APK配布・インストール・ADB、OSセキュリティ設定変更、他プロジェクト・不明mountpointへの操作、Astra使用。
- 全動的試験は `--disposable` のみ。生成したAES-256暗号化Vault・PKCS12・LEDGER/Registry/Witness・APK・子プロセスはすべて試験専用で、終了時に正常アンマウントし、自作の一時ディレクトリのみ削除した。

## 2. Acceptance criteria

| AC | 判定 | 根拠 / 制約 |
|---|---|---|
| **AC-A** TOCTOU最小対策と動的検証・残留リスク明示 | **PASS** | 単一fd読取 + O_NOFOLLOW + fstat同一性 + 検証済みバイトの専用staging + inode同一性 + 公開後ハッシュ/Receipt検証を実装。新規9件のTOCTOU試験と既存公開/復旧E2Eが全PASS。同一UID非協調プロセスに対する完全防御は脅威モデル外として明示。 |
| **AC-B** 実AES-256暗号化Disposable Vaultでの復旧後新versionCode署名 | **PASS** | 新規暗号化DMGで実Serea APK(42)署名→sidecar公開後に子プロセスをSIGKILL→`ledger-recovery-plan`→`ledger-recover-publish`→再署名なしで公開→APK SHA-256一致→同一コード再利用拒否→code 43実APK署名→API 26/35検証→全状態ストア整合→正常アンマウント。 |
| **AC-C** 指定3ケースの実プロセス競合・中断試験 | **PASS** | C1 同一versionCode並行署名（勝者1・敗者はロックでfail-closed・重複BUILT 0）、C2 署名vs復旧（不正公開0・競合署名成果物0・fail-closed）、C3 復旧公開途中SIGKILL→再実行（PUBLISHED・単一BUILT・high-water整合）。 |
| **AC-D** 公開allowlist・機密監査・最終回帰・独立レビュー・公開準備 | **PARTIAL → Owner判断待ち** | allowlist/除外一覧/Secret監査/回帰/Draft PR/CI契約は整備済み。独立レビューはTerraが401で不可のため代替モデルで実施（下記）。同一SHA CI・Owner成果物選択・正式ホスト決定は別工程。 |
| **AC-S** 中核安全性違反0・権限境界／既存成果物維持 | **PASS** | 独立レビューで Critical 0 / High 0、以前のMajor TOCTOUは構造的に解消。HEAD/Tree不変、禁止操作なし、既存dirty/untracked成果物を維持。条件付き残存はOwner判断へ分離。 |

## 3. Lane A — Minimal TOCTOU hardening

### 3.1 実装修正 (`tools/serea_signing.py`)

従来のMajor条件付き指摘は「検証はパス経由で読み、公開はパス経由でhard-linkするため、同一UIDの非協調 writer が検証後・link前に差し替えられる」というものだった。単なる公開後再検証では解消しないため、公開対象を**検証済みバイト列そのもの**に結びつけた。

- `_read_fd_sha256(fd)`: パスを再オープンせず、オープン済みfdから全バイトを読んでハッシュする。
- `_observed_file_sha256(path)`: `_canonical_no_symlinks` の後、`O_RDONLY|O_NOFOLLOW|O_CLOEXEC` で**一度だけ**オープンし、`fstat` で正規ファイル・Owner・`nlink<=2` を確認してからfd経由でハッシュ。パス再オープンを廃止。
- `_publish_verified_artifact(source, destination, expected_sha256, *, label, file_mode)`:
  1. 入力を `O_NOFOLLOW` で一度だけオープンし、`fstat` で種別／Owner／link数／setuid・setgidを確認。
  2. **そのfdから読んだバイト列**のSHA-256を期待値と比較。不一致ならSTOP（公開しない）。
  3. 公開先が既存なら上書きせず、`_verify_published_artifact` で同じくfd経由の同一性・ハッシュ確認。
  4. 新規なら出力親ディレクトリ（Owner所有0700）に `mkstemp` で専用staging inodeを作成し、検証済みバイトを書き込んでfsync、staging fd自体でもハッシュ再検証。
  5. `os.link(staging, destination, follow_symlinks=False)` で非上書き公開（既存は `FileExistsError` → 既存物を検証してSUCCESS扱いしない／一致なら冪等）。
  6. 公開後に `destination` を `O_NOFOLLOW` で再オープンし、**`(st_dev, st_ino)` が検証したstaging inodeと一致**かつハッシュ一致を確認。不一致ならSTOPし、ReceiptをPUBLISHEDにしない。
- `_recovery_link_no_replace()`: 上記helperへ委譲（APK=0400、sidecar=0600）。 Recovery のsidecar先・APK先の双方で使用。
- `cmd_sign_apk()`: 通常署名のsidecar/APK公開も上記helperへ移行。既存の障害注入メッセージ（"sidecar publication requires recovery review" / "APK commit requires recovery review"）は維持。公開後に `_publication_diagnostics` でReceipt整合性を再確認し、不一致ならSTOP。
- 公開後APK署名検証はAPI 26/35で実施済みのevidenceを再利用し、Recoveryでは公開前に `signer_hashes == [cert, cert]` を厳格確認。新規暗号方式・署名サービス・認証局・OS依存機構は導入していない。

### 3.2 脅威モデル境界（A2）

- 任意の同一UID非信頼プロセス／管理者権限取得者に対する完全防御は本キャンペーン範囲外。技術的対策は「**パス差し替えではなく、検証したfd由来のバイト列とinodeを公開に結びつける**」ことに限定した。
- 正式運用の必須条件（変更なし・Owner判断対象）: 開発環境と署名環境の分離／専用Owner管理アカウント／署名環境での非信頼コード・AIエージェント・Gradle実行禁止／署名資産・状態ストア・成果物保管領域への第三者アクセス禁止／独立検証後のみ外部移送。現行ホストがこれらを満たすとは仮定しない。
- 残存リスク（条件付きMajor相当）: 同一UIDの攻撃者が**公開完了後**に最終成果物パスを別ファイルへ差し替えること、およびattempt/storesを直接改ざんすることは、本技術対策でも抑止できない。`O_NOFOLLOW`・0700ディレクトリ・inode同一性は「検証→公開」の隙間を閉じるが、公開後のパス所有権そのものはOS/運用境界の問題である。重大度を形式的に格下げせず、Ownerの残留リスク判断に委ねる。

### 3.3 Focused tests (`tests/test_serea_publish_toctou.py`, 9/9 PASS)

`test_observed_hash_rejects_symlink` / `test_observed_hash_matches_fd_bytes` / `test_publish_copies_verified_bytes_to_fresh_output` / `test_publish_refuses_existing_conflicting_output` / `test_publish_is_idempotent_for_matching_output` / `test_publish_rejects_source_symlink` / `test_publish_detects_wrong_expected_hash` / `test_publish_detects_source_byte_replacement` / `test_publish_detects_link_redirection_to_unverified_bytes`。

「検証後の入力差し替え」「シンボリックリンク変更」「APK/sidecar改ざん」「既存出力との競合」「正常公開・冪等復旧」をfdレベルで実証。通常署名・Recovery双方の既存E2E（9件）もPASS。

## 4. Lane B — Real encrypted Vault recovery (最重要動的受入)

`tests/run_disposable_signing_e2e.py` は `build-public`/`build-private`（現在fail-closed）に依存するため再利用せず、現行CLI (`init → provision → fingerprint → witness-init → ledger-init → sign-apk → ledger-recovery-plan → ledger-recover-publish → lock`) をptyドライバで実行した。APK入力は既存の実Serea unsigned APK (`app-beta-unsigned.apk`, code 42, SHA-256 `71bd31de…840ef`) を使用。ディレクトリfixtureでの代替ではない。

**最終ソース上での再実行証跡（LANE_B_PASS, 最終 run）:**

```text
git HEAD / Tree                 : 317c601760ab7148afd979ee9ef092838d76a072 / 2017e9b15c09724459afe3f59c4740bb4a6c04fe
tools/serea_signing.py          : a17b33860f2cda3a2cd8ea8b4ffcbed60b5999eec5d92b250a4adf820f715bce
vault                           : fresh AES-256/APFS encrypted Disposable DMG (hdiutil isencrypted=true)
ledger_id                       : 93b158a7-4db7-4adb-a179-accb031991c0
attempt_id (interrupted)        : 51a08057-a9fd-4037-bfcb-6feeb3ec01f6
child exit (SIGKILL at sidecar) : -9
real Serea unsigned input       : 71bd31de17b34cd46f343d415ea99e62d5aedd4f1a2f0c3af0a897d78df840ef  (code 42)
managed APK SHA-256 before rec  : a3c96b89fa147c4416e9314add8d5e68b2183bab394c26fb33a41f3c78ece2c3
published APK SHA-256 after rec : a3c96b89fa147c4416e9314add8d5e68b2183bab394c26fb33a41f3c78ece2c3  (同一 → 再署名なし)
receipt state before / after    : BUILT / PUBLISHED
API 26/35 recovered & next code : PASS / PASS
next code 43 signed APK SHA-256 : d2afb947d105ededa29963cfdf643c0bfc4c1e03bb833f328de796d55243455c
public certificate SHA-256      : 6f4366cc46497f05ee269bc51483a121ece7aa89dcc2561a80e0d777d30cad1b  (Disposable)
state high-water (public)       : 43
```

非公開の一時領域に保存した証跡・Disposable証明書フィンガープリントは本報告書のハッシュのみで、鍵・パスフレーズは一切保持していない。

実施シーケンス（14ステップ、すべてPASS）:

1. Disposable ED25519 Authority + enrollment slot を独立生成。
2. 新規AES-256/APFS暗号化DMG Vaultを `init`（`hdiutil isencrypted` で暗号化を実確認）。
3. 独立Public/Private PKCS12を `provision`。
4. 外部 Witness / Identity Registry / Release Ledger を `witness-init` → `ledger-init` でバインド。
5. 実Serea APK(42)とprovenanceを準備。
6. 実Vault上で署名する子プロセスを、sidecar公開後・APK公開前に **`SIGKILL`** で中断（`exit=-9`）。
7. `lock` で安全にアンマウント、`ledger-recovery-plan` で中断attemptを特定（自動再公開なし）。
8. 正しい確認文字列 `PUBLISH public <attempt-id> 42` で `ledger-recover-publish` 実行。
9. 公開APK SHA-256 == 復旧前managed APK SHA-256（**再署名なし**を実証）。Receipt `BUILT → PUBLISHED`。
10. API 26/35 signer検証PASS、冪等再実行PASS、BUILTイベント1件のみ。
11. 消費済みcode 42の再署名要求を拒否（成果物なし）。
12. code 43の実APK fixtureを署名し、API 26/35検証PASS、next_version_code=44。
13. Ledger/Registry/Witness/Vaultのhigh-water(43)・ledger id整合を確認、`ledger-status` PASS。
14. 正常 `lock` アンマウント、証跡を非公開領域へ保存。

## 5. Lane C — Essential process competition

実子プロセス＋既存 `flock` ロック＋ready/gate同期（各ケースで子プロセスを明示識別）。1回の新規暗号化Disposable Vault上で連続実施。**最終ソース上で再実行し `LANE_C_PASS` / exit=0 を確認済み**（新規暗号化Vault `isencrypted=true`）。

```text
C1 同一versionCode並行署名 : 勝者 exit=0 / 敗者 exit=1 (lock fail-closed), BUILT(42)=1, next=43
   published APK SHA-256 : 999c35e0cb349514040d32384262b549181d8b20e13f413241f9bc6c6046dd26
C2 署名 vs 復旧           : process exits [1,1] (両方fail-closed), 競合署名成果物0, BUILT(43)=1, next=44
   復旧公開バイト==検証済みmanagedバイト
C3 復旧途中SIGKILL→再実行  : interrupted exit=-9, Receipt=PUBLISHED, BUILT(44)=1, high-water=44
   公開ハッシュ==managedハッシュ
```

- **C1 同一versionCode並行署名**: 2プロセスを同時開始（片方は8秒ロック保持）。勝者1・敗者は `"Another Serea signing operation is already running"` でfail-closed（`exit=1`）。敗者成果物0、code 42のBUILT 1件、next=43。
- **C2 署名 vs 復旧**: code 43のBUILT/sidecar済みattemptを作り、`ledger-recover-publish` と `sign-apk`(code 44) を競合開始。両プロセスともfail-closed（公開前状態を維持）で、競合署名の成果物0。その後、明示的再試行で復旧を完了し、公開バイト==検証済みmanagedバイト、code 43のBUILT 1件。
- **C3 復旧途中強制終了と再実行**: `ledger-recover-publish` を「final APK公開直前」でSIGKILL（`exit=-9`）。APK未公開・Receipt BUILTを確認後、同一コマンドを再実行 → Receipt `PUBLISHED`、公開ハッシュ==managedハッシュ、code 44のBUILT 1件、Registry/Witness high-water=44、`ledger-status` PASS。

能動的に終了させたのは今回生成した試験子プロセスのみ。他プロセスは終了させていない。

## 6. Lane D — Source audit / regression / freeze preparation

### 6.1 公開候補allowlist（厳格・個別ファイル単位）

**コード（変更/新規）**
- `tools/serea_signing.py` **(変更)**
- `tests/test_serea_publish_toctou.py` **(新規)**
- `tools/serea_release_ledger.py`, `tools/serea_apk_signer.py`, `tools/serea_identity_registry.py`, `tools/serea_signing_witness.py`, `tools/serea_signing_state.py`, `tools/serea_enrollment_authority.py`
- `tests/test_serea_sign_apk_cli_e2e.py`, `tests/test_serea_apk_signer.py`, `tests/test_serea_apk_signer_e2e.py`, `tests/test_serea_release_ledger.py`, `tests/test_serea_identity_registry.py`, `tests/test_serea_signing_witness.py`, `tests/test_serea_signing_cli.py`, `tests/test_serea_enrollment_authority.py`, `tests/run_disposable_signing_e2e.py`
- 追跡済み変更: `.github/workflows/android.yml`, `android/app/build.gradle.kts`, `android/gradle.properties`, `tools/validate_docs.py`, `tools/verify_android_build_provenance.py`

**文書（公開候補のうち安全なもの）**
- `docs/plans/2026-11-15-owner-production-signing-launch-runbook.md`
- `docs/plans/2026-11-16-h4-d2-final-dynamic-acceptance-report.md`
- `docs/plans/2026-11-16-h4-d2-final-recovery-closure-report.md`
- `docs/plans/2026-11-16-h4-d2-signing-closure-report.md`
- `docs/plans/2026-11-17-h4-d2-signing-final-closeout-report.md` (本報告)

### 6.2 除外一覧（必須）

- 正式・Disposable鍵、PKCS12、`.p12`/`.pfx`/`.jks`/`.keystore`/`.pem`/`.key`、Vaultイメージ `.dmg`
- 実運用台帳・状態記録・Receipt・`identity-manifest.json` / `version-ledger.json` / `authority-*` state
- パスフレーズ／個人設定／`.local/`／`.superpowers/`／`android/app/build/`／`android/local.properties`／`android/.gradle/`／キャッシュ
- 個人パス・ユーザー名を含む以下の文書は公開除外（値は出力しない）: `2026-11-14-owner-signing-identity-audit.md`, `2026-11-15-user-visible-regression-report-8a13e3ea.md`, `2026-11-15-owner-signing-cli-r14-addendum.md`, `2026-11-15-owner-absent-campaign-report.md`, `2026-11-15-owner-absent-r14-report.md`, `2026-11-15-h4-d2-r5-postfix-regression-review-03c868c1.md`, `2026-11-14-owner-signing-establishment-preflight.md`, `2026-11-14-disposable-android-signing-evidence-closure.md`, `2026-11-15-owner-signing-cli-runbook.md`（superseded）
- Calendar/Gmail/AI/Personal-Beta等の署名無関係文書は本allowlistに含めない。

### 6.3 機密情報監査結果

- 公開候補コード（`tools/` 7ファイル + `tests/` 11ファイル + Android/CI変更）: **クリーン**。秘密鍵／PKCS12／トークン／API key／メール／個人ユーザー名／個人homeパスのリテラルなし。パスフレーズはstdin/getpass由来でargv・env・ログに出さない。個人パスは `Path.home()` で実行時構築。
- 文書: 上記除外文書にのみ個人パス／ユーザー名が存在。公開候補文書のハッシュ値はすべて**file-content／Disposable fixture**のもので、正式Public/Private証明書フィンガープリントは存在しない（preflightで `NOT AVAILABLE` と記録済み）。
- リポジトリ内に秘密ファイル（鍵/DMG/PKCS12）は存在しない。`~/Library/Application Support/Serea` も存在しない。
- `.superpowers/`（428ファイル）は untracked かつ gitignore されていないため、`git add -A` 等の一括操作では絶対に公開しない。今回はGit書込を行っていない。
- 監査で検出した「秘密らしき値」はすべて benign（fixtureパスフレーズ、Disposableattempt ID、commit/tree SHA、Actionsピン）であり、値をログ/レポート/AIコンテキストへ出力していない。

### 6.4 最終回帰（最終ソース上）

| コマンド | 結果 |
|---|---|
| `python3 -m unittest tests.test_serea_publish_toctou tests.test_serea_sign_apk_cli_e2e` | **18/18 PASS** |
| `python3 -m unittest tests.test_serea_release_ledger tests.test_serea_identity_registry tests.test_serea_signing_witness tests.test_serea_signing_cli tests.test_serea_apk_signer tests.test_serea_enrollment_authority tests.test_android_build_provenance` | **82/82 PASS**（内 Build Provenance 5/5） |
| `python3 -m py_compile tools/serea_signing.py tests/test_serea_publish_toctou.py` | PASS |
| `python3 tools/validate_docs.py docs` | PASS（243 Markdown files scanned） |
| `git diff --check` | PASS |

- 既存の実Serea APK署名 (Public/Private)・Android 88/88・Private OAuth Fake JVM 10/10 は前工程の成功証跡を利用し、Android/Gradle変更がないため再実行していない（不要な再ビルド禁止）。
- タイムアウトはPASSとして扱っていない。回帰中のargparse負例出力・duplicate-ZIP警告は既知の期待出力。状態ストアのsymlinkガードで既定TMPDIR(`/var/...`)が拒否されたため、以前の成功手順どおり非symlinkの `/private/var/folders/.../opencode` をTMPDIRに指定して実行した（環境障害の原因調査済み、同じ失敗条件の無意味な再試行はしていない）。

### 6.5 独立レビュー

- 優先指定の **Terra は HTTP 401 で利用不可**。外部代替2モデルも401だったため、ローカルプロバイダの独立レビューエージェントで同一スコープのread-onlyレビューを実施した。これはGitHub承認でも外部監査でもない。
- **判定: `APPROVED_WITH_RESIDUAL_OWNER_RISK`。Critical 0件 / High 0件。** 以前のMajor（verify-then-link TOCTOU）は構造的に解消されたと判定（単一fd読取→fstat→同fdハッシュ→staging inode再ハッシュ→非上書きlink→`(st_dev,st_ino)`＋ハッシュ再検証）。Production fail-closed、消費済みコード再利用防止、単一BUILT、Receipt-backed Recovery（再署名なし・同一バイト・sidecar先・API 26/35・4ストア整合）、新規テストの実効性、レビュー対象ファイルのシークレット hygiene も確認。
- 残存指摘（いずれも同一UID／条件付きで、コード未修正のままOwner条件として分離）:
  - **Major（条件付き・運用残存、コード欠陥ではない）**: `PUBLISHED` 記録後の同一UID改ざんはツールでは検出不可。补偿統制は「移送前の独立検証」であり、専用Ownerアカウント・署名環境分離・非信頼コード/AI/Gradle禁止・第三者アクセス禁止のattestが必要。重大度は格下げしない。
  - **Moderate**: 公開後検証失敗時に`destination`をunlinkせず未検証バイトが正規パスに残る（ReceiptはBUILTのまま成功扱いせず、復旧も不一致を拒否）。移送前独立検証が必須の补偿統制。
  - **Moderate**: RecoveryのAPK証跡（`_apk_identity` / `_verify_apk` / `_verified_signer`）は管理APKをパスで再オープンするため、同一UID差し替えで証跡と公開バイトが食い違う可能性（公開バイト自体はハッシュ束縛で安全）。
  - **Moderate**: 作業ツリーの`android/`, Gradle, CI差分は本変更ではないが、freezeスナップショットには含めず別レビューが必要（本報告のallowlistは既に除外／ Separate review 対象として明記）。
  - **Minor**: `FileExistsError`分岐・`nlink>2`・クロスFS拒否のテスト欠如、全artifactのメモリ2倍保持、生の`OSError`トレース、Receipt binding（従来どおり）、Vaultパスフレーズ無き復旧不可（運用条件）。
- レビューは初回1回のみ。追加コード変更を行っていないため再レビューは不要（差分なし）。

### 6.6 公開準備

- **Draft PR title (未作成):** `H4-D2: descriptor-bound verified publication and Disposable encrypted-Vault recovery`
- **Draft body (未作成):** “Bind verified bytes to a no-follow descriptor and a fresh Owner-only staging inode before publication; never overwrite an existing artifact and stop on any hash/inode mismatch. Recovery republishes exact pre-verified bytes without re-signing and is Disposable-only. Add focused TOCTOU and encrypted-Vault interruption/recovery evidence. Production remains disabled and unauthorized. Require independent review and all Required checks on the exact Owner-selected source SHA.”
- **CI contract:** 正式承認SHA上で4つのRequired checks（`Linux fast checks`, `Linux stable full validation`, `Android Emulator Required Gate`, `Linux MSRV 1.89.0`）＋Python署名/Ledger/Recovery・docs・Build Provenance・Android API 26/35 variant isolation。Private OAuth EnabledはFake JVMのみ。secrets/正式署名は有効化しない。**同一SHA CIは NOT_RUN（別工程）。**

## 7. 最終ソースハッシュ（全修正完了後）

```text
317c601760ab7148afd979ee9ef092838d76a072  git HEAD (unchanged)
2017e9b15c09724459afe3f59c4740bb4a6c04fe  git Tree (baseline; local worktree remains dirty/untracked)
a17b33860f2cda3a2cd8ea8b4ffcbed60b5999eec5d92b250a4adf820f715bce  tools/serea_signing.py (hardened publication)
4b7945a381ccc6f98a3fec1bf9df400be49ce79b0915d0c8200d111a2cab0b85  tests/test_serea_publish_toctou.py (new)
4a5ba8fdb86a836a66c30f3117835e8a8de184076ee47e9456b5647f8e12b0bc  tests/test_serea_sign_apk_cli_e2e.py (unchanged)
33ab0d32427f6cce1628594dcfc6ae645669c8cad16f5aec8ef177f26045c0cf  tools/serea_release_ledger.py (unchanged)
```

These hashes identify local dirty/untracked files only. They are not an approved source snapshot, signed manifest, or same-SHA CI result.

`tools/serea_signing.py` の `a17b3386…` は、Lane A 最終回帰（18/18, 82/82）および Lane B / Lane C の最終動的再実行で使用した**同一SHA**である。独立レビュー後はコード変更を行っていないため再レビュー不要（差分なし）。

## 8. 残存リスク / Owner残存作業

1. **同一UID条件付きTOCTOU残存**: 公開後パス差し替え・状態ストア直接改ざんは技術範囲外。専用Ownerアカウント・署名環境分離・非信頼コード/AI/Gradle実行禁止・第三者アクセス禁止を正式運用の必須条件としてOwnerが承認するか判断する（重大度は格下げしない）。
2. **Receipt/provenance binding（Minor条件付き）**: 復旧がsource-input hashと出力パスをOwner-private receiptから取る点は従来どおり。独立認証済みattempt recordによる強いbindingは未実装で、延期を明示する。
3. **ホスト信頼**: FileVault Off・SIP/OCLP懸念は未解決。正式署名ホスト選択・FileVault/SIP/OCLP・パッチ適用・専用アカウントはOwner決定。
4. **バックアップ／independent custody**: 暗号化Vaultと別置きのLedger/Registry/Witnessの暗号化バックアップ・外部媒体・独立保管・復元リハーサルはOwner決定（今回は外部媒体に触れていない）。
5. **同一SHA CI / ソース選択**: Ownerが-clean snapshotとallowlistを選択し、レビュー済みSHAでRequired checksを取得する。
6. **正式鍵生成・正式署名・配布・同一SHA CI** は別工程。本報告は一切のProduction承認を与えない。

## 9. 正式ソース確定工程への移行可否

- 技術的必須条件（Lane A/B/C）は成立し、中核安全性の未修正Critical/Highはない。
- したがって **署名基盤の追加機能開発はここで終了し凍結** してよい。次の開発資源は Calendar / Gmail / AI 統合へ優先配分する。
- ただし `SIGNING_FEATURE_FREEZE_READY` は「未解決の重大な安全性問題が0」の場合のみ。本件では条件付きMajor（同一UID）と運用条件が残るため、判定は **`OWNER_RISK_REVIEW_REQUIRED`** とする（最終レビューでCritical/Highが出た場合は `PARTIAL` または `BLOCKED` に変更）。
- Owner残留リスク承認・正式ホスト・外部バックアップ・正式鍵生成・同一SHA CIが完了した時点で、別工程として正式ソース確定／署名有効化の判断を行う。

---

**Final decision: `OWNER_RISK_REVIEW_REQUIRED`**
