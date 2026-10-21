# Android Q4 UI統合レポート

## 成果と固定SHA

- Repository: `Yrika819/Serea`
- Draft PR: [#71](https://github.com/Yrika819/Serea/pull/71)
- Branch: `codex/android-q4-day-ui-integration-2026-10-21`
- Base branch/SHA: `codex/android-q4-night-gate-recovery-2026-10-21` / `fd4321df03a3834044e3432bc288e2512b543fd2`
- 最新のコード検証はHEAD `95f2110c6f408cdb12757628add9bf9269e7725c`を起点にした最終作業ツリーで実施。commit後のHEAD/Treeは、CI完了後の本レポート更新で固定します。
- Base tree: `ef4403783e356ddf0362fc4ad9d5a589467e9248`

このレポートと最終ソースをコミットしてからGitHub CIを実行します。CIを通過したソースを変更せず、docs-only report updateが必要な場合は、検証済みコードHEAD/TreeとPR最終HEAD/Treeを区別して記録します。

| Lane | 取り込み元の固定SHA | 取り込み方法 |
|---|---|---|
| 修復済み基盤 / PR #70 | `fd4321df03a3834044e3432bc288e2512b543fd2` | 新規ブランチの起点。PR #70は変更せず、親として維持 |
| Calendar / PR #67 | `8d61120e61ab3a1e4387527fd9143ecc7212c675` | Codex identityの`--no-ff` merge、commit `bd4caca2780597ddf68fce275b98dfe8fc026899` |
| TaskProposal / PR #68 | `bb3d936fcc4379f5a36972f35a7f5739942f2c65` | identity不適合の履歴は含めず、固定差分を監査して正規コミットへ再構成 |
| Gmail / PR #69 | `4b20786a9ae065ee94b1b77d64160f48656e0835` | identity不適合の履歴は含めず、共通baseとの差分を監査して正規コミットへ再構成 |
| Navigation | 利用できる固定SHAなし | 既存branch、Tree、Draft PRが存在しないことを確認し、このブランチで新規実装 |

PR #71はOPEN / Draftで、baseは固定したPR #70のHEADです。PR #51、#58〜#70、`main`、既存branchは変更していません。PR #68/69の固定HEADおよび不適合identityの元コミットは最終HEADの祖先ではありません。PR #67のmerge commitは祖先で、統合ブランチから固定baseまでの11 commitはすべて`Codex <noreply@github.com>`です。

## Git Identity、Hooks、差分の監査

開始時、commit時、push時にrepository-local Git設定、Author、Committerを検証しました。`user.name=Codex`、`user.email=noreply@github.com`、`user.useConfigOnly=true`、`core.hooksPath=.githooks`、`push.default=nothing`、`serea.identityProfile=codex`を確認しています。CommitとpushのPreflight、通常のpre-commit/pre-push hookはPASSです。`--no-verify`、force push、hook・preflight・identity guardの無効化は使用していません。

PR #68固定commit `bb3d936fcc4379f5a36972f35a7f5739942f2c65`とPR #69の3 commit (`960c59875de640ce16a3c39d4ac6848af6a08f49`, `6908d085b264df2f1fa368ecab96f7d54b2901a9`, `4b20786a9ae065ee94b1b77d64160f48656e0835`)のGit identityは許可済みCodex identityに適合しません。PR #69共通baseは`072aa2b823523f7a92196299dd86cfb3509a78e3`です。固定差分を監査したうえで必要なコードを新しいCodex commitとして取り込み、元の不適合commitは履歴へ混入していないことを`git merge-base --is-ancestor`で確認しました。

## PR #67/#68/#69の統合証拠と競合解決

### Gmail

PR #69の固定HEADと最初の統合commit `010d92d5d3d229f3f889f4b3db9e0e922e23cc77`で、次のblob SHAが一致しました。

| File | 固定#69 blob SHA |
|---|---|
| `android/app/src/main/java/app/serea/android/gmail/ui/GmailViewModel.kt` | `83541f6abd2dcc2c39f20be104171c18f04247a1` |
| `android/app/src/test/java/app/serea/android/gmail/ui/GmailUiTestFake.kt` | `b4f6733540e28c05dcfafb7c30d66d993381598c` |
| `android/app/src/test/java/app/serea/android/gmail/ui/GmailViewModelTest.kt` | `e1b1562d0d31d6cc9c56e39f4b9bef4063ab4e04` |
| `docs/plans/2026-ANDROID-Q4-NIGHT-GMAIL-UI-REPORT.md` | `9625d663dff0cb6a58707b4266e7c8be5aa9e55f` |

共通base `072aa2b823523f7a92196299dd86cfb3509a78e3`から固定#69 HEADへの全5変更pathは最初の統合commitにも存在します。上記4コード/test/documentation blobは一致し、残る`GmailScreen.kt`は固定版`baa69722731e0f0e6f934ce9ade33a4ba89195d8`から初回統合時に`d0495a2c030f4c053007ef1a3f6212442b610ae4`へ適応しています。これはdestination接続とscroll/semantics対応です。Gmailのfake-backed search/detail/read-only flowはCompose UI testsで検証しました。最終GmailViewModel/testの後続差分はIME、query draft、非同期検索結果の回帰修正です。

`GmailScreen.kt`は固定差分から取り込み、画面サイズ・スクロールとテストsemanticsに必要なUI接続を加えました。ViewModel/port契約と読み取り専用挙動を維持しています。最終版では検索IME処理、検索入力保持、古い検索結果の破棄もテストしています。以降の画面統合・競合修正によるGmailViewModel/testの変更は、当初blobの一致ではなく固定差分との意味比較とJUnitで検証しました。Navigationからは`FakeGmailProvider`を`GmailViewModel.factory(provider, provider)`へ注入します。

### TaskProposal

`TaskProposalTest.kt`はPR #68の承認敵対テスト20件とPR #70の回帰テスト15件を削除せず統合しました。重複は5 method、unionは30 methodで、最終sourceのmethod名集合も両方のunionと一致します。PR #70世代のCancellation regressionを維持し、古いreview/session表示、二重承認、Task ID照合、CAS、Delete追加確認、承認前write 0をカバーします。

Demoは専用のin-memory Fake Repositoryのみを使い、通常Tasks DB、暗号化TaskRepository、Chat/providerから隔離しています。UIには日本語の4操作プレビュー、Deleteの追加確認、二重tap防止、Session再利用拒否を実装しています。

### Calendarと共通差分

PR #67は`--no-ff` mergeで履歴を保持しました。`CalendarAgendaScreen`、`CalendarAgendaUiController`、UI fake、Formatting/Controller/Security tests、実装レポートを維持しています。Navigationは`FakeCalendarProvider`とその`Clock`、`ZoneId.systemDefault()`を渡し、東京timezoneを補っていません。実OAuth、Calendar HTTP、書き込みAPIは有効化していません。

PR #70とUIレーンのCI差分は、workflowを丸ごと上書きせず意味的に統合しました。API 26/35別の証拠、JUnit source/XML照合、空・重複・欠損検出、API 26未実行時の`NOT_RUN`、両API Process Death gate、Required check名を保持し、新規Instrumentation 26件を各クラスの厳密件数へ加えました。

## Navigation新規実装と実画面接続

Navigationは**旧コードの復旧ではなく新規実装**です。想定branch `codex/android-q4-night-navigation-2026-10-21`、Draft PR、候補Tree `dfd73e2c15cb4903cb6a224fc02306aad9963f17`を作業開始時とPR #67/#68/#69統合後に確認しましたが、利用できませんでした。存在しない以前の実装やInstrumentation 10件を統合済み成果には数えていません。

新規Navigationコード:

- `android/app/src/main/java/app/serea/android/DestinationContract.kt`
- `android/app/src/main/java/app/serea/android/SereaAppShell.kt`
- `android/app/src/test/java/app/serea/android/SereaNavigationStateTest.kt`
- `android/app/src/androidTest/java/app/serea/android/SereaAppShellTest.kt`
- `android/app/src/androidTest/java/app/serea/android/MockUiStateIntegrationTest.kt`
- `android/app/src/androidTest/java/app/serea/android/SereaThemeTest.kt`
- `android/app/src/androidTest/java/app/serea/android/assistant/taskproposal/TaskProposalDemoScreenTest.kt`
- `android/app/src/main/java/app/serea/android/MainActivity.kt` — 既存ActivityからApp Shellへ接続

DestinationはChat、Tasks、Gmail、Calendar、Settings、TaskProposal Demoです。destination contractとback stackをActivity再作成時に復元し、ChatViewModelはActivity scopeで共有してroute変更時に作り直しません。`rememberSaveableStateHolder`で画面状態を保持し、TaskProposalの承認SessionはSavedStateへ保存しません。Tasksは既存TaskViewModel/TaskScreenへ接続し、既存`TasksActivity`のfallback導線も残しています。SettingsではCredential UIと`FLAG_SECURE`の適用範囲を維持しました。

Navigationが使う公開Composableと実provider:

```kotlin
GmailScreen(viewModel = gmailViewModel, onBack = ..., modifier = ...)
GmailViewModel.factory(readPort = fakeProvider, authorizationPort = fakeProvider)
CalendarAgendaScreen(onBack = ..., provider = fakeProvider, clock = fakeProvider.clock,
    displayZone = ZoneId.systemDefault(), modifier = ...)
TaskProposalDemoScreen(modifier = ..., onClose = ...)
```

Chat、Tasks、Settingsを含め、6画面すべてApp Shellから到達できます。Material 3 / `SereaTheme`、日本語、Back、スクロール、semantics、dark/light modeを使い、Loading/Empty/Failure/RetryをGmail/Calendar fake providerでテストしています。新規依存関係は追加していません。

## テストとBuild

### Android JVM

最終作業ツリー（HEAD `95f2110c6f408cdb12757628add9bf9269e7725c`からのソース差分を含む）で、次を実行しました。

```text
./gradlew --no-daemon :app:testDebugUnitTest
```

JUnit XMLとKotlin sourceの厳密verifierで**251 tests / 21 classes、0 failure、0 error、0 skipped**を確認しました。`TaskProposalTest`は30 method、`SereaNavigationStateTest`は5 methodです。件数は重複を除いた最終ソースとJUnit XMLの一致値です。Android unit/instrumentation JUnit verifier regression 14件とemulator campaign script regression 4件もPASSしました。

固定PR #70 sourceと最終sourceのtest method名集合も比較しました。PR #70の178 methods / 16 classesはすべて最終sourceに残り、削除classや欠落methodはありません。TaskProposalの重複解消はPR #68/#70のmethod unionを保つ比較で確認しています。

### Android Instrumentation source

最終source countは**98 methods / 14 classes**です。PR #70基準72件に26件を追加しました。追加26件は`SereaAppShellTest` 18、`MockUiStateIntegrationTest` 4、`SereaThemeTest` 2、`TaskProposalDemoScreenTest` 2です。PR #68/70からの`TaskProposalReviewScreenTest` 2件は維持しています。

画面別テストは次のとおりです。

| Class | 件数 | 主な検証 |
|---|---:|---|
| `app.serea.android.EncryptedHistoryTest` | 16 | encrypted history / process restoration |
| `app.serea.android.ChatScreenTest` | 16 | Chat Compose regressions |
| `app.serea.android.ChatHttpE2ETest` | 4 | mock HTTP contract |
| `app.serea.android.UsageLedgerDatabaseTest` | 12 | Usage Ledger DB |
| `app.serea.android.tasks.TasksDatabaseTest` | 6 | Tasks DB |
| `app.serea.android.tasks.TaskScreenTest` | 5 | Tasks UI |
| `app.serea.android.ChatTasksDatabaseIntegrationTest` | 1 | Chat/Tasks storage integration |
| `app.serea.android.ChatHistoryUxTest` | 4 | Chat history UX |
| `app.serea.android.tasks.TaskDateTimePickerTest` | 6 | DatePicker/TimePicker regressions |
| `app.serea.android.assistant.taskproposal.TaskProposalReviewScreenTest` | 2 | review UI adversarial checks |
| `app.serea.android.SereaAppShellTest` | 18 | Chat初期表示、6 destinations、Back Stack/recreation、Chat/Task draft、Gmail検索/詳細、Calendar詳細、Fake-only、未承認write 0、TalkBack semantics、fontScale |
| `app.serea.android.assistant.taskproposal.TaskProposalDemoScreenTest` | 2 | pending operation control、古いSession破棄 |
| `app.serea.android.SereaThemeTest` | 2 | Material 3 Dark/Light scheme |
| `app.serea.android.MockUiStateIntegrationTest` | 4 | Gmail/Calendar Loading→Empty、Failure→Retry |

この14 classの件数合計は98です。API別JUnit XMLはCI verifierで全classのsource method集合・件数との一致を要求します。

API 26とAPI 35のエミュレーター実行数、Process Deathは最終GitHub Android runの実JUnit/XMLとevidenceを確認してこの欄を確定します。Instrumentation APKのローカルassembleはemulator実行PASSとは数えていません。個人データのある端末は使用していません。

固定PR #70 instrumentation sourceの72 test method / 10 required classesを最終sourceと比較しました。既存72 methodはすべて名前まで保持され、欠落はありません。残り26 methodを4 class追加し、計98 / 14 classesです。

### Build、Lint、Security

最終作業ツリーで次のローカルGradle taskをPASSしました。

```text
./gradlew --no-daemon :app:assembleDebug :app:assembleRelease :app:assembleDebugAndroidTest :app:lintDebug
```

Lintは0 error、12 Warning、2 Informationです。GitHub runではDebug/Release/Instrumentation Build、Lint、dependency vulnerability audit、Egress security testsも実行します。

PR #70のHistory DB v4、Tasks DB v1、Keystore/encryption、Usage Ledger、Egress Gate、`PENDING / OUTCOME_UNKNOWN`、TaskViewModel世代管理を維持しています。schema、TaskRepository、SecurityPorts、UsageAccountingの差分はありません。`EncryptedStorage.kt`の差分はChat testability用の`ConversationHistoryPort`境界で、production repositoryの暗号化、DAO、CAS transactionを変更していません。

Gmail/CalendarはFake-only、Chatは`MockModelChatPort`のみです。OAuth real mode、Gmail REST、Calendar HTTP、実AI provider、実アカウントは有効化していません。メール・予定内容をAIへ送る経路、WebView HTML実行、Gmail mutation、Calendar writeを追加していません。DataOrigin/classificationを保ち、token/body/accountをログへ出していません。

## GitHub Actions

PR #70固定基盤の既存成功run:

| Workflow | Run ID | 結果 |
|---|---:|---|
| Android | `39473032291` | PASS、報告済みAPI 26/35各72件、両Process Death |
| Full CI | `39473032285` | PASS |
| Fast CI | `39473032287` | PASS |
| Native | `39473032282` | PASS |
| SQLite | `39473032284` | PASS |
| Git Hooks | `39473032292` | PASS |

PR #71の前回push HEAD `95f2110c6f408cdb12757628add9bf9269e7725c`に対するCI結果です。Android emulatorの6失敗を受けてCompose待機/IME/フォント拡大テストを修正し、最新作業ツリーをローカル検証済みです。修正版をpush後、以下runを更新します。

| Workflow | Run ID | 状態 |
|---|---:|---|
| Android | `39524393018` | FAIL — API 26/35は各98件/14クラスを実行、各6 failures。Process Deathは両API PASS |
| Fast CI | `39524392991` | PASS |
| Full CI | `39524393035` | PASS |
| Native platform validation | `39524392973` | PASS |
| Cross-architecture SQLite portability | `39524392984` | PASS |
| Git hooks Windows compatibility | `39524393060` | PASS |

Android run `39523520371`では初回Unit実行がfinalization test 2件で失敗しました。原因はproduction regressionではなく、history repository付きViewModelが初期restore中に`sendMessage()`を拒否するのに、test fixtureがrestoreを待たず`completeStarted`をawaitしていたことです。既存会話を明示してfinalization経路へ入るfixtureに直し、251件すべてPASS後のCI run `39524393018`ではAndroid Unit、Security、Integration、Dependency Audit、DocumentationがPASSしました。しかしAPI 26/35のCompose instrumentationは各6件失敗しました。最新作業ツリーでは失敗したCompose待機を表示/意味状態待ちに置換し、IMEのBack挙動とfontScale 2時のスクロール到達性を修正しました。再実行後、API別件数・Process Death・全workflowの最終結果を追記します。

## 独立レビューと指摘

実装者とは別のreviewerがArchitecture/Lifecycle/Concurrency、Android UX/Accessibility、Security/Data Egress、Regression/CI、Adversarialの各観点をレビューし、修正後も関連差分を再確認しました。最終テストfixture修正は別のTest/CI reviewerも確認しました。

| 観点 | 結果 |
|---|---|
| Architecture / Lifecycle / Concurrency | P0/P1なし。Chat履歴切替、Gmail検索A→B、Calendar選択A→B、Task save generation、TaskProposal二重承認/session/cancellation、Activity recreationを確認。Chat finalization中の履歴切替・cancel state不整合には再現テストを追加して修正 |
| Android UX / Accessibility | P0/P1なし。6 route、Back、TalkBack semantics、fontScale 2、dark/light、詳細/スクロールをInstrumentationで確認。実機での視覚・TalkBack/IME確認は未実行 |
| Security / Data Egress | P0/P1なし。Google scope/token-lease契約、Mock-only/OAuth OFF、Credential/FLAG_SECURE、隔離Fake、body/token/account loggingとartifact境界を確認 |
| Regression / CI | P0/P1なし。source/JUnit exact count、API別証拠、Process Death gate、identity/lockfileを確認 |
| Adversarial | P0/P1なし。stale session/result、操作中のfeature切替、承認迂回・二重mutation、assert false positiveを確認 |

残るP2 / Beta前のOwner判断:

- Chatの削除確認用ID/titleとTask editorのtitle/description/due fieldsはActivity-scoped ViewModelに置き、Android SavedStateへ書き込みません。Task DatePicker/TimePicker subtreeもSaveableStateRegistryへ登録しません。Activity再作成ではTask editor draftを維持しますが、OS process death後にeditor visibility/draftを復元しません。process death後の未保存draft破棄は設計どおりで、process-death専用draft assertionは未実装です。
- Calendarは画面の日付入力/選択日を`rememberSaveable`へ保存します。現在の画面は`FakeCalendarProvider`のみを使いますが、日付文字列はユーザー入力由来のメタデータです。Calendar/event identifiersおよびGmail message/thread route identifiersも画面状態に保存されますが、現接続ではfakeのsynthetic IDsのみです。これらを実providerへ接続する前にSavedStateのデータ分類を再確認してください。
- API 26/35の自動Instrumentationとは別に、実端末でのTalkBack announcement、ナビゲーション領域の視認性/タッチ、IME表示中のBack操作はNOT_RUNです。個人データのある実機は使用していません。
- Lintの最新taskは再実行中です。完了後、warning数とCI結果を本レポートへ記録します。

未解決P0/P1はありません。Owner DecisionsはCalendar date metadataのSavedState分類確認と、実データのない端末/正式なBeta手順でのvisual/TalkBack/IME reacceptanceです。これらの判断が済むまでPRはDraftのまま保全します。

## NOT_RUNとMerge Readiness

- ローカルAPI 26/35 instrumentationおよびProcess Death: **NOT_RUN**。最終GitHub Android campaignの実evidenceを確認します。
- 実Google OAuth、実Gmail/Calendar account、実AI provider: **NOT_RUN**。今回の制約に従い有効化していません。
- 個人データのある実機Instrumentation / Pixel 7a manual reacceptance: **NOT_RUN**。
- Beta署名/release、mainへのmerge: **NOT_RUN**。

**Merge Readiness: Draft / 最終API 26・35のInstrumentation、Process Death、必須CI結果とOwnerのP2判断待ち。** Required gateを緩和せず、実行していないCompose/emulator動作をPASSとは扱いません。
