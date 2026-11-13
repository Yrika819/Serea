# Serea Personal Beta — 2026年11〜12月 統合ロードマップ

> **状態:** Owner向け提案・実装ガイド。2026-11-12の記録は履歴として保持し、現在状態は下記の2026-11-13同期欄を参照。目標日は保証ではない。
> **対象:** [Yrika819/Serea](https://github.com/Yrika819/Serea) / Android Personal Beta / 2026-12-31 GO/NO-GO
> **根拠:** [既存G10–G15計画](../g9/g10-execution-plan.md)、[Beta機能スコープ](../g9/beta-scope.md)、[Owner decision ledger](../plans/2026-10-26-personal-beta-owner-decision-ledger.md)。
> **ガバナンス:** このロードマップ自体は、Merge、実OAuth、Google Cloud変更、実AI API使用、Owner署名、配布、実機操作の許可ではない。各ゲートでOwnerの明示承認を得ること。

## 0.1 現在状態への同期 — 2026-11-13

| 項目 | 現在状態・証拠 |
|---|---|
| canonical main | HEAD `eaa8f7cefb608d846c336a972f33b0812f269f57`; Tree `4cb69b2c6bec8ed60dd90bc3e03d5415d71226b0` |
| H4-A / PR #234 | **統合完了**。Merge SHAはmain HEAD `eaa8f7cefb608d846c336a972f33b0812f269f57`。Primary-only境界の修正とidentity/adb root検証を含む |
| PR #232 | **OPEN・未統合・Draftではない（Ready for review）**。Primary-only実装は#234経由でmainへ統合済み。PR #232の未統合差分や状態を、main上のH4-A状態と混同しない |
| H4-B | Owner指示により独立実装を開始。対象は認可Session失効・再認可・HTTP 401/403/429・切断・アカウント切替・Process Deathの安全性。成果物は独立Draft PRまで |
| 実Google OAuth / Google API | 未有効化。実アカウント・外部API・実Revokeを使用しない。`googleHttpEnabledByOwner=false`を維持 |
| CIスナップショット | main HEADの取得時点でAndroid Formal API26/35、Unit、Integration、Security、Identity Isolation等は成功。Linux stable full validationとLinux fast checksは失敗、combined statusはpending。H4-Bの正式CI結果ではない |

以降の作業ではGitHubから最新値を再取得する。次の節「0. 現在地 — 2026-11-12」は**当時の履歴スナップショット**であり、現在状態を表すものではない。

## 0. 現在地 — 2026-11-12

| 項目 | 状態・証拠 |
|---|---|
| canonical main | `27f56e972a6f8f0b9406b6054c1637fdfb3c587f`; Tree `5ddfd7847540049da35168e653e5eaa93890747f` |
| Private Google認可UI | PR [#228](https://github.com/Yrika819/Serea/pull/228) をmainへ統合済み。Private-only、初期状態Fake |
| Synthetic ConsentのOS Process Death | PR [#231](https://github.com/Yrika819/Serea/pull/231) を統合済み。Android API26/35で旧Resultの実配送と拒否を確認 |
| Primary-only境界 | PR [#232](https://github.com/Yrika819/Serea/pull/232) OPEN/Draft、HEAD `71caa1b9ab6434f0fabfc287b4316add8317af03`、Tree `5234f1c05d1adaf2081d110a54ce5a3ba328d67a`。47 Check Runのうち46 SUCCESS、1 SKIPPED、失敗0。**未Merge** |
| 実Google OAuth/Calendar/Gmail | 未有効化・未受入れ。Private PreviewのGoogle HTTPはFake固定 |
| 実AI | 実プロバイダー／ランタイム選択、利用規約・料金・外部送信承認は未確定。Demo/Fakeを実AIと表示しない |
| Owner署名・実機受入れ・配布 | 未実施。Owner管理ゲートを維持 |

これらのSHAは**この記録時点のスナップショット**。次の作業時は必ずGitHubから最新のmain/PR HEAD/Tree/Required checksを再取得すること。

## 1. 最終目標とスコープ

- **P0（Personal Beta必須）:** Android安定動作、正確にDemoと明示したChat（推論未承認時）、暗号化された会話履歴とTasks、基本UI/アクセシビリティ、削除・OS Process Death復旧、権限処理、安全な署名/更新経路、個人データのない専用環境での実機受入れ。
- **P1（承認条件付きの重点）:** Google Primary Calendar read-only、承認済み実AIルート、通知/リマインダー。Gmailは**opt-in P1**であり、Restricted Scopeの適合・セキュリティ審査が整わなければOFF。
- **P2（年末Betaから除外）:** PC/クラウド操作、自律的な他アプリ操作、LINE、クロスデバイス同期、長期記憶の大規模化、全面UI刷新。

最終表示は3区分: **Personal Beta GO**（承認済み機能を実動作確認）、**Demo Beta GO**（P0は満たすが未承認の実AI/GoogleをOFFのままDemo明示）、**NO-GO**（P0/署名/実機/セキュリティ条件不成立）。いずれもOwnerが判定する。

## 2. 工程・暫定日程

| 期間（JST） | フェーズ | 完了の根拠 / 停止条件 | Owner承認 |
|---|---|---|---|
| 11/13 | **H4-A** PR #234統合 | main HEAD `eaa8f7cefb608d846c336a972f33b0812f269f57`。H4-A統合完了。PR #232はOPENのまま維持（Ready for review、未統合） | #234の統合済み事実。#232の状態変更・統合とは別 |
| 11/13–22 | **H4-B** Google認可失効・切断・再認可 | HTTP 401→Session invalidationと明示的再認可; 403/429分類とbounded retry; local disconnectとprovider-wide revokeの区別; account switch / stale result / cancellation / process death回帰 | 独立Draft PR作成まで明示指示済み。Draft解除・Merge・実Google接続不可 |
| 11/20–26 | **H4-C** PrivateアプリID・署名・配布モデル確定 | `app.serea.android.beta`共有維持 vs Private独立IDを比較。移行/データ/更新、証明書SHA-1/256照合手順、安全な配布・漏えいリスクを文書化 | Application ID変更はOwner決定。鍵の取扱い・署名はOwnerのみ |
| 11/23–12/04 | **H4-D/E/F** OAuth条件確認→限定実接続→Primary Calendar E2E | Android OAuth Clientのpackage/SHA-1一致、Scope `calendar.events.readonly`のみ、`calendarId=primary`以外を拒否、Google HTTP読取・権限取消・アカウント変更・再認可を限定実証。**実操作は事前承認後** | External/Testing・Cloud・アカウント・実機操作を別々に明示承認 |
| 11/16–12/08 | **G10/AIレーン** 実AI選定と安全な統合 | Provider/APIまたは端末ローカル推論のライセンス・利用条件、料金上限、保持/学習・機密情報保護、Egress→Usage→Dispatch順序、失敗時UNKNOWNと失効、ARM64安定性を検証 | モデル、予算、資格情報、外部送信許可はOwner判断 |
| 11/21–30 | **G11** Tasks/通知と機能受入れ | 暗号化保存、CRUD、再起動、予定/通知、権限拒否、時刻/DST、再通知、削除、データ分離。対象環境で観測 | 端末・通知権限は必要時Owner判断 |
| 11/25–12/10 | **Gmail判定レーン** Mock完成、条件成立時のみ実装 | `gmail.readonly`はRestricted。審査・限定利用・データ取扱いを再確認。未承認はFakeのまま | 実Gmailは新規明示承認なしで禁止 |
| 12/01–10 | **G12** 機能凍結・正式受入れ | 凍結後は原則安全性/不具合修正のみ。Android API26/35 Formal、Identity、Process Death、IME、オフライン、回転、BG/FG、削除、Tasks/Chatの実動作と記録 | フリーズ対象/逸脱はOwner判断 |
| 12/11–17 | **G13** 独立Security Review | Keystore、Token/OAuth、Google-originの外部AI egress禁止、ログ/Artifact秘密情報、依存関係/SBOM、供給網、権限、Process Death、データ削除。Critical/High 0を基本条件としMediumは明示判断 | 指摘受容・残留リスクOwner承認 |
| 12/18–24 | **G14** Release Candidate / 実機・更新試験 | 署名証明書、package/versionCode、再現可能な構築、既存データと更新・ロールバック経路、Owner専用配布、専用テスト端末受入れ。個人データ入り端末で削除/初期化しない | 署名・配布・端末操作は別途Owner承認 |
| 12/25–31 | **G15** GO/NO-GO | 決定時main HEADに結び付いた証跡、実機、署名/配布、機能ラベル、未解決リスク、運用停止手順を総合評価 | **最終Owner GO/NO-GO** |

日程に遅れた場合もCI、独立レビュー、個人情報の保護を弱めない。重要な期限は**12/10 Feature Freeze、12/17 Security Review、12/24 RC、12/31 GO/NO-GO**。

## 3. H4-B以降の技術的ゲート

### Google/Calendar

1. PR #232のPrimary-only防御を統合後mainに保持。Transport直呼びを含め、非Primary Calendar IDとCalendarListは認可照会・Token取得・HTTPより前に拒否。Generic Demoは維持。
2. 現行Private Google Calendar PreviewはFakeのまま。実Googleへの経路開通は**別PR・別Owner承認**。
3. 401は現在のSessionを失効させ再認可へ（無限再試行禁止）。403は権限不足/ポリシー/レート制限をレスポンスに応じ区別、429は上限付きbackoff。認可に失敗した応答を新しいSessionへ反映しない。
4. アプリ内切断とPlay Servicesの`revokeAccess`は別操作。Provider側のGrant取消は他のGoogle機能に影響し得るため警告と明示操作が必要。
5. 初回Google CloudはExternal/Testingと本人テスター登録を候補にする。Calendarでは7日制約あり。長期利用のExternal/In Production・personal-use例外は警告/認可可能ユーザー/審査と別管理する。Internalは一般の個人Googleアカウント向けではない。
6. Owner署名証明書からSHA-1/256を照合し、最終Application IDを先に決定。Owner鍵、Access Token、Authorization Code、実Google予定内容をPublic repo/CI/ログへ入れない。

### AIとGmail

- Chat→外部AIは厳格な送信先許可、利用者意図、データ分類、予算上限、Usage記録を経由する。Google起源データが自動的にCloudへ流れないようにする。
- 実AIが年末まで未承認なら**Demo Beta**を明示してP0安定性を優先する。
- Gmail Restricted ScopeはCalendarのSensitive Scopeと審査・扱いが異なる。実Gmailを理由にP0やCalendarの安全性を犠牲にしない。

## 4. 継続して必要なCI・監査証跡

- protected mainのRequired: `Linux fast checks`, `Linux stable full validation`, `Android Emulator Required Gate`, `Linux MSRV 1.89.0`。
- Android API26/35 Formalと必要なInstrumented/UI、Unit、Integration、Security、Personal Beta Identity Isolation、Synthetic Consent Process Death（DELIVERED_REJECTED vs NOT_DELIVERED vs NOT_RUNを区別）。
- RustSec findings=0を目標、Native Platform、SQLite portability、Git identity、Public/Private variant isolation。各テストのPASSは**同じ検証対象SHA**に結び付ける。
- 非Required診断のscope guard/IMEの失敗を黙殺しない。製品欠陥と診断環境の問題を分けて記録する。
- 独立レビューで未解決Critical/Highを残したままMerge/GOにしない。正式GitHub APPROVEDレビューとは別に管理。
- PRは原則Draftで作成。Draft解除・通常Merge・実OAuth・AI活性化・Google Cloud変更・署名・APK配布・Pixel実機操作はそれぞれOwnerの明示承認を要求。Force Push/保護回避禁止。

## 5. Owner Decision Register（2026-11-13同期）

以下は確認できた履歴と今回の明示指示を反映する。記録がない判断を承認済みと推定しない。

| ID | 実際の判断・履歴 | 現在の扱い |
|---|---|---|
| OD-H4-01 | PR #232をReady for reviewへ変更（2026-11-13）。PR #234を統合しH4-Aをmainへ反映 | 完了した履歴。#232はOPEN・未統合のまま維持。#234統合を#232統合と扱わない |
| OD-H4-02 | BetaとPrivate Betaの共通Application ID維持か分離か | 11/22 | 未決定。現行設定を変更しない |
| OD-H4-03 | Google External/Testing・テスト用プロジェクト・Ownerアカウント | 11/24 | 未決定。Cloud/実認証には触れない |
| OD-H4-04 | 実AI Provider/モデル/予算上限/データ利用 | 11/22 | 未決定。Fake/Demoのまま |
| OD-H4-05 | 実Primary Calendar接続の許可 | 12月初旬 | 未承認。Fake固定 |
| OD-H4-06 | Gmail restricted scopeの実接続許可 | 条件が整い次第 | 未承認。OFF |
| OD-H4-07 | Owner署名・配布・端末受入れ | 12/24 | 未承認・未実施 |
| OD-H4-08 | Personal Beta / Demo Beta / NO-GO | 12/31 | 未決定 |
| OD-H4-09 | H4-Bの実装と独立Draft PR作成 | 2026-11-13指示 | 承認済み。Draft解除・Merge・実Google接続は含まない |

## 6. エージェント運用

- セッション開始時にルートの [AGENTS.md](../../AGENTS.md)、本書、G10–G15計画、Beta scope、Owner decision ledgerを読む。
- 実装指示の受領時は現行main/PR SHAと本書のスナップショットを比較し、最新のOwner判断に従う。日付だけで自動的に工程を開始しない。
- 変更は独立Draft PRで管理し、報告にはPR/HEAD/Tree、追加/維持したテスト、API26/35証拠、CI、独立レビュー、禁止操作が未実施であること、残るOwner判断を記載。
- 本書を更新する場合もドキュメント専用PRとし、コード/ビルドの変更とは分離する。Ownerの許可なしにmainへMergeしない。

## 参考資料

- [Google OAuth / Androidクライアント識別](https://developers.google.com/android/guides/client-auth)
- [Google OAuth のSecurity Best Practices](https://developers.google.com/identity/protocols/oauth2/resources/best-practices)
- [Google Calendar API Scopes](https://developers.google.com/workspace/calendar/api/auth)
- [Gmail API Scopes](https://developers.google.com/workspace/gmail/api/auth/scopes)

*外部Google要件は変更される可能性があるため、実設定の前に公式資料を再確認すること。*
