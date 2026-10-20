# Gmail Read-only Domain / Fake / MIME Parser

Date: 2026-10-20

Status: 実装済み。新規 Draft PR 作成後の CI は GitHub Actions で確認し、PR と最終回答に最終 HEAD と Run ID を記録する。

## Git 基準

- Repository: `Yrika819/Serea`
- PR #58: OPEN / DRAFT。確認した HEAD branch は `codex/android-m0-08-abcd-integration-2026-10-20`、HEAD SHA は `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`。
- 新規 Draft PR: [#64](https://github.com/Yrika819/Serea/pull/64)。PR 作成時の head SHA は `4bde27af2328f7d8d87347aa9315241279777398`、base SHA は `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`。
- 実装 branch: `codex/android-q4-gmail-domain-2026-10-20`
- 固定開始 SHA: `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`
- Base branch: `codex/android-m0-08-abcd-integration-2026-10-20`。fetch した実 SHA も `239aaa181cdcfea756cf6c9ebb5eca9194caf7cf`。
- PR #58 の確認時 state / draft / head / base は上記と一致した。PR #58、既存 branch、main は変更していない。
- Repository-local Identity は `Codex <noreply@github.com>`、`serea.identityProfile=codex`、`user.useConfigOnly=true`、`push.default=nothing`。Hooks は `.githooks` に設定し、`tools/git_preflight.py --mode commit` は PASS。
- ローカル `tools/check_commit_identity.py` は GitHub CI event scope がないため実行を拒否した。このため同スクリプトの全履歴判定をローカル PASS としていない。PR の Commit Identity Gate を別途確認する。

## 変更範囲

- `android/app/src/main/java/app/serea/android/gmail/GmailModels.kt`
- `android/app/src/main/java/app/serea/android/gmail/GmailReadPort.kt`
- `android/app/src/main/java/app/serea/android/gmail/FakeGmailProvider.kt`
- `android/app/src/main/java/app/serea/android/gmail/GmailMimeParser.kt`
- `android/app/src/test/java/app/serea/android/gmail/GmailDomainParserTest.kt` (`GmailSecurityPortsTest`)
- `docs/plans/2026-ANDROID-GMAIL-DOMAIN-REPORT.md`

`git status` で上記の新規 package とこの報告書以外に変更がないことを確認した。Gradle/KSP が生成した未追跡の Tasks schema は削除し、schema・migration は残していない。`.github/workflows/**`、Gradle 設定、Manifest、既存 Calendar / Tasks / Security / Chat ファイルには変更を加えていない。

## 実装

- `GmailReadPort` は Inbox page、query search page、message detail、thread detail の read 操作だけを公開する。送信、削除、既読化、ラベル変更の API はない。
- `GmailAuthorizationPort` は状態を返すだけで、OAuth や token を取り扱わない。`MOCK_ONLY` は mock 読み取りにだけ許可され、実認証状態とは区別される。要求 scope は `https://www.googleapis.com/auth/gmail.readonly` のみ。
- `GmailMessage` / `GmailThread` / `GmailMessagePage` は Gmail 出所を必須とし、classification の既定値は `PRIVATE`。`PUBLIC` へ昇格できない。失敗は安定コードだけを含み、provider response、ID、宛先、件名、本文を例外文に含めない。
- `FakeGmailProvider` は `.invalid` ドメインの架空データのみを持ち、Inbox/search/pagination/thread と空ページ、重複ID、拒否、429相当、timeout、cancellation、部分失敗を再現する。
- `GmailMimeParser` は Gmail `messages.get(format=full)` の bounded subset を読む。Base64URL、charset、multipart/alternative、ネストした part、欠落任意項目を扱い、JSON 2 MiB / structural nesting 64、decoded body 合計 1 MiB、MIME depth 16、header 数 200、header value 16 KiB の上限を適用する。不正 JSON/Base64/charset、サイズ不一致、過大・過深入力は内容を含めず `INVALID_RESPONSE` で失敗する。filename を持つ添付 subtree は解釈せず、ダウンロードしない。
- HTML は文字列から tag、コメント、script/style/noscript 内容を除いて表示用テキストへ変換する。属性値は出力せず、外部画像・リンク・script を取得または実行する経路はない。利用側も `htmlTextBody` を HTML/WebView として描画せずテキストとして扱う必要がある。
- Gmail package は `ChatContext`、`ModelChatRequest`、Usage Ledger、Cloud Adapter に接続されていない。HTTP、OAuth、実 Gmail、AI 解析は実装していない。

## 検証

一時 JDK 17 を `/tmp` に置き、システム CA trust store をその JDK に参照させ、環境 proxy を Gradle JVM に渡して実行した。Repository 設定や TLS 検証は緩めていない。

| 検証 | 結果 |
|---|---|
| `./gradlew --no-daemon testDebugUnitTest --tests 'app.serea.android.gmail.*'` | PASS: 12 tests、failures/errors/skips 0 |
| `./gradlew --no-daemon testDebugUnitTest --tests '*SecurityPortsTest'` | PASS: `app.serea.android.gmail.GmailSecurityPortsTest` 12 tests、failures/errors/skips 0。Android workflow の既存 Security selector に含まれることを確認 |
| `./gradlew --no-daemon assembleDebug` | PASS |
| `./gradlew --no-daemon lintDebug` | PASS |
| `git diff --check` | PASS |
| 実 OAuth / 実 Gmail / HTTP E2E / Pixel 7a | NOT_RUN |

最初のローカル Gradle 起動はシステム JDK に `javac` がなく失敗し、JDK 17 と proxy CA trust を一時設定した後に上記検証を完了した。Gmail 用の変更で Android workflow は既存 `android-unit` job の広い全件 selector に含まれず、`android-security` job の `--tests '*SecurityPortsTest'` に含まれる。該当 selector をローカルで実行し、12件が選ばれることを確認した。Workflow ファイルは変更していない。

## Security / 未実装 / 統合

- Gmail由来データは `DataOrigin.GMAIL` と `PRIVATE` classification を保つ。既存 `ChatEgressGate` の Gmail origin deny をテストした。Gmail APIから Cloud Adapter へ転送する依存はない。
- Android Manifest の backup / cleartext policy、Keystore、History DB v4、Tasks DB v1、Usage Ledger、Task CAS には触れていない。ADB・端末データ・署名・実 credential・API key・OAuth token は使っていない。
- 後続 REST Adapter は `GmailReadPort` に接続し、認可 status は OAuth 担当から `GmailAuthorizationPort` 経由で注入する。UI、sync、OAuth token lifecycle、quota/backoff、添付対応は未実装。
- `GmailSecurityPortsTest` は既存 CI selector に入る。新しい Workflow や Gradle dependency は不要。実 API 接続を追加する場合は、別変更で OAuth scope/consent、Egress、retention、エラー本文の漏洩を Security Review する。
- この報告書に PR #64 URL と実装 commit を記録した follow-up commit 後の branch HEAD が最終候補となる。final HEAD SHA / tree SHA とその SHA に紐づく GitHub Actions Run ID / 状態は、follow-up push 後に確認して最終回答へ記録する。旧 HEAD の CI を final HEAD の証拠として扱わない。
