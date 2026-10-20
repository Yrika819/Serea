# Gmail Read-only Domain / Fake / MIME Parser

Date: 2026-10-20

Status: PR #64 追加監査実装済み。監査開始 HEAD `9a91443ff135cdaee5015d6a0e309476e91fc0d7`、base `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`。

## Git 基準

- Repository: `Yrika819/Serea`
- PR #58: OPEN / DRAFT。確認した HEAD branch は `codex/android-m0-08-abcd-integration-2026-10-20`、HEAD SHA は `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`。
- 新規 Draft PR: [#64](https://github.com/Yrika819/Serea/pull/64)。PR 作成時の head SHA は `4bde27af2328f7d8d87347aa9315241279777398`、base SHA は `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`。
- 実装 branch: `codex/android-q4-gmail-domain-2026-10-20`
- 固定開始 SHA: `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`
- 追加監査の実装 commit: `126dda6359cc892c24456e134a8e3e6f8db3cb4a`。Tree SHA: `023e642820ea6ea26de7e4c082aac9c75d31ec0b`。
- Base branch: `codex/android-m0-08-abcd-integration-2026-10-20`。fetch した実 SHA も `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`。
- PR #58 の確認時 state / draft / head / base は上記と一致した。PR #58、既存 branch、main は変更していない。
- Repository-local Identity は `Codex <noreply@github.com>`、`serea.identityProfile=codex`、`user.useConfigOnly=true`、`push.default=nothing`。Hooks は `.githooks` に設定し、`tools/git_preflight.py --mode commit` は PASS。
- ローカル `tools/check_commit_identity.py` は GitHub CI event scope がないため実行を拒否した。このため同スクリプトの全履歴判定をローカル PASS としていない。PR の Commit Identity Gate を別途確認する。

## 変更範囲

- `android/app/src/main/java/app/serea/android/gmail/GmailReadPort.kt`
- `android/app/src/main/java/app/serea/android/gmail/FakeGmailProvider.kt`
- `android/app/src/main/java/app/serea/android/gmail/GmailMimeParser.kt`
- `android/app/src/test/java/app/serea/android/gmail/GmailDomainParserTest.kt` (`GmailSecurityPortsTest`)
- `docs/plans/2026-ANDROID-GMAIL-DOMAIN-REPORT.md`

実際の変更は上記Gmail package内の3 production files、対応test 1 file、report 1 fileに限定した。Gradle/KSP生成物、`.github/workflows/**`、Gradle設定、Manifest、既存 Calendar / Tasks / Security / Chat filesには変更を加えていない。

## 実装

- `GmailReadPort` は Inbox page、query search page、message detail、thread detail の read 操作だけを公開する。送信、削除、既読化、ラベル変更の API はない。
- `GmailAuthorizationPort` は状態を返すだけで、OAuth や token を取り扱わない。`MOCK_ONLY` は mock 読み取りにだけ許可され、実認証状態とは区別される。要求 scope は `https://www.googleapis.com/auth/gmail.readonly` のみ。
- `GmailMessage` / `GmailThread` / `GmailMessagePage` は Gmail 出所を必須とし、classification の既定値は `PRIVATE`。`PUBLIC` へ昇格できない。失敗は安定コードだけを含み、provider response、ID、宛先、件名、本文を例外文に含めない。
- `FakeGmailProvider` は `.invalid` ドメインの架空データのみを持ち、Inbox/search/pagination/thread と空ページ、重複ID、拒否、429相当、timeout、cancellation、部分失敗を再現する。
- `GmailSearchService.collect()` はページ内とページ間のユニークID数で上限を判定し、重複IDの内容矛盾は `INVALID_RESPONSE` にする。空ページは次トークンがあれば続行し、同じトークンの再出現は失敗する。
- Fake読み取りは `MOCK_ONLY` 状態かつscope不足なしの場合だけ許可する。`DENIED` / `REVOKED` / `EXPIRED` / `AUTHORIZED` のFake利用は拒否し、不足scopeは `MISSING_SCOPE` で失敗する。`GmailPermissionPolicy.mayRead()` は将来の実HTTP adapter用に `AUTHORIZED` かつ不足scopeなしを要求し、`MOCK_ONLY` を認めない。Fake用判定は `mayReadFake()` に分離した。
- `GmailMimeParser` は Gmail `messages.get(format=full)` の bounded subset を読む。Base64URL、charset、multipart/alternative、ネストした part、欠落任意項目を扱い、JSON 2 MiB / structural nesting 64、decoded body 合計 1 MiB、MIME depth 16、header 数 200、header value 16 KiB の上限を適用する。alternative はplain textを優先し、なければHTMLを選択する。mixedの独立した本文は同じMIME種別ごとに連結する。未対応multipart（relatedを含む）と未対応inline本文は `INVALID_RESPONSE` で失敗し、本文全体を取得済みとは扱わない。不正 JSON/Base64/charset、サイズ不一致、過大・過深入力も内容を含めず `INVALID_RESPONSE` で失敗する。filename を持つ添付 subtree は解釈せず、ダウンロードしない。
- HTML は文字列から tag、コメント、script/style/noscript 内容を除いて表示用テキストへ変換する。属性値は出力せず、外部画像・リンク・script を取得または実行する経路はない。利用側も `htmlTextBody` を HTML/WebView として描画せずテキストとして扱う必要がある。
- Gmail package は `ChatContext`、`ModelChatRequest`、Usage Ledger、Cloud Adapter に接続されていない。HTTP、OAuth、実 Gmail、AI 解析は実装していない。

## 検証

一時 JDK 17 を `/tmp` に置き、システム CA trust store をその JDK に参照させ、環境 proxy を Gradle JVM に渡して実行した。Repository 設定や TLS 検証は緩めていない。

| 検証 | 結果 |
|---|---|
| `./gradlew --no-daemon testDebugUnitTest --tests 'app.serea.android.gmail.*' --tests '*SecurityPortsTest'` | PASS: 24 tests、failures/errors/skips 0。Gmail `GmailSecurityPortsTest` 15件 + 既存 `app.serea.android.SecurityPortsTest` 9件 |
| `./gradlew --no-daemon assembleDebug lintDebug` | PASS |
| `git diff --check` / `tools/git_preflight.py` commit・push | PASS |
| 実 OAuth / 実 Gmail / HTTP E2E / Pixel 7a | NOT_RUN |

最初のローカル Gradle起動はシステムJDKに `javac` がなく失敗した。JDK 17とproxy CA trustを一時設定し、最終再実行ではテスト24件（Gmail 15件）を完了した。Gmail用の変更で Android workflow は既存 `android-unit` job の広い全件 selector に含まれず、`android-security` job の `--tests '*SecurityPortsTest'` に含まれる。Workflowファイルは変更していない。

## Security / 未実装 / 統合

- Gmail由来データは `DataOrigin.GMAIL` と `PRIVATE` classification を保つ。既存 `ChatEgressGate` の Gmail origin deny をテストした。Gmail APIから Cloud Adapter へ転送する依存はない。
- Android Manifest の backup / cleartext policy、Keystore、History DB v4、Tasks DB v1、Usage Ledger、Task CAS には触れていない。ADB・端末データ・署名・実 credential・API key・OAuth token は使っていない。
- 後続 REST Adapter は `GmailReadPort` に接続し、認可 status は OAuth 担当から `GmailAuthorizationPort` 経由で注入する。UI、sync、OAuth token lifecycle、quota/backoff、添付対応は未実装。
- `GmailSecurityPortsTest` は既存 CI selector に入る。新しい Workflow や Gradle dependency は不要。実 API 接続を追加する場合は、別変更で OAuth scope/consent、Egress、retention、エラー本文の漏洩を Security Review する。
- 追加監査実装commit後に report を更新したため、このreport更新commitが最終HEADになる。GitHub Actionsの結果は必ずその最終HEAD SHAに対して確認し、前のcommitに対するCI結果を流用しない。PR #64はOPEN / Draftを維持し、マージしない。
