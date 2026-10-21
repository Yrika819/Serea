# Android Q4 UI統合レポート

## 成果と固定SHA

- Repository: `Yrika819/Serea`
- Draft PR: [#71](https://github.com/Yrika819/Serea/pull/71)
- Branch: `codex/android-q4-day-ui-integration-2026-10-21`
- Base branch/SHA: `codex/android-q4-night-gate-recovery-2026-10-21` / `fd4321df03a3834044e3432bc288e2512b543fd2`
- 実装・Instrumentation検証HEAD/Tree: `806db1f912d78e197ce8937985dd57424cdfab72` / `96bc39ec27492a92b95818cd943a579eb0580678`
- Base tree: `ef4403783e356ddf0362fc4ad9d5a589467e9248`
- CI run `39595336592`はHEAD `806db1f`を検証し、Android API別instrumentationに3つの共通UI test failureがありました。修正を新しいCodex commitに反映した後、Draft PRの新HEADで全CIを再実行し、このレポートを確定します。

最終JVM XMLは252 tests / 21 classesで、strict source/JUnit verifier PASSです。Debug/Release/Instrumentation BuildとLintはこの実装・Instrumentation HEADと同じsourceでPASSしました。`testDebugUnitTest --rerun-tasks`で252件を再実行して成功し、instrumentation変更を含むAPK再コンパイルもPASSしています。GitHub Android run `39595336592`でAPI 26/35 JUnitとProcess Deathを実行し、次の修正対象を特定しました。

| Lane | 取り込み元の固定SHA | 取り込み方法 |
|---|---|---|
| 修復済み基盤 / PR #70 | `fd4321df03a3834044e3432bc288e2512b543fd2` | 新規ブランチの起点。PR #70は変更せず、親として維持 |
| Calendar / PR #67 | `8d61120e61ab3a1e4387527fd9143ecc7212c675` | Codex identityの`--no-ff` merge、commit `bd4caca2780597ddf68fce275b98dfe8fc026899` |
| TaskProposal / PR #68 | `bb3d936fcc4379f5a36972f35a7f5739942f2c65` | identity不適合の履歴は含めず、固定差分を監査して正規コミットへ再構成 |
| Gmail / PR #69 | `4b20786a9ae065ee94b1b77d64160f48656e0835` | identity不適合の履歴は含めず、共通baseとの差分を監査して正規コミットへ再構成 |
| Navigation | 利用できる固定SHAなし | 既存branch、Tree、Draft PRが存在しないことを確認し、このブランチで新規実装 |

PR #71はOPEN / Draftで、baseは固定したPR #70のHEADです。PR #51、#58〜#70、`main`、既存branchは変更していません。PR #68/69の固定HEADおよび不適合identityの元コミットは実装HEADの祖先ではありません。PR #67のmerge commitは祖先で、実装HEADまでの27 commitはすべて`Codex <noreply@github.com>`です。

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

実装・Instrumentation HEAD `806db1f912d78e197ce8937985dd57424cdfab72`のsourceで、次を実行しました。

```text
./gradlew --no-daemon :app:testDebugUnitTest
```

JUnit XMLとKotlin sourceの厳密verifierで**252 tests / 21 classes、0 failure、0 error、0 skipped**を確認しました。追加した`lateDefiniteProviderFailureAfterCancellationKeepsOutcomeUnknown`も実行済みです。`TaskProposalTest`は30 method、`SereaNavigationStateTest`は5 methodです。件数は重複を除いた最終ソースとJUnit XMLの一致値です。Android instrumentation/JVM JUnit verifierおよびemulator campaign script regressionは合計18件PASSしました。

固定PR #70 sourceと最終sourceのtest method名集合も比較しました。PR #70の178 methods / 16 classesはすべて最終sourceに残り、削除classや欠落methodはありません。TaskProposalの重複解消はPR #68/#70のmethod unionを保つ比較で確認しています。

### Android Instrumentation source

最終source countは**98 methods / 14 classes**です。PR #70基準72件に26件を追加しました。追加26件は`SereaAppShellTest` 18、`MockUiStateIntegrationTest` 4、`SereaThemeTest` 2、`TaskProposalDemoScreenTest` 2です。PR #68/70からの`TaskProposalReviewScreenTest` 2件は維持しています。新しいNavigation/TaskProposal UI変更は既存test methodを編集し、件数を水増ししていません。

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

主要な実画面テストは次のとおりです。

- **Chat / Tasks / Navigation:** `chatIsInitialDestinationAndAllRoutesAreAccessible`、`chatTasksBackReturnsToTheSameChatViewModel`、`backStackReturnsThroughPriorDestinations`、`activityRecreationRestoresRouteAndBackStackButNoProposalSession`、`chatDraftSurvivesGmailAndCalendarDestinationChanges`、`taskEditorDraftSurvivesActivityRecreation`。Tasks UIの作成・編集・完了・再開・Delete確認・古いsave結果保持は`TaskScreenTest` 5件。
- **Gmail:** `chatGmailBackShowsOnlyFictionalMailboxData`、`gmailSearchAndMessageDetailUseTheFakeProviderFixtures`、`gmailAndCalendarDetailsRestoreAcrossActivityRecreation`。`MockUiStateIntegrationTest`はLoading→EmptyとFailure→Retryを確認。
- **Calendar:** `chatCalendarBackShowsFictionalAgendaAndDetail`、`calendarDetailAndSelectedCalendarSurviveAnotherDestination`、`gmailAndCalendarDetailsRestoreAcrossActivityRecreation`。CalendarのLoading/Empty/Failure/Retryも同じFake UI state testで検証。
- **Settings:** `chatSettingsBackKeepsCredentialScreenshotProtectionScoped`でSettings導線、Back、Credential/`FLAG_SECURE`適用範囲を確認。
- **TaskProposal Demo:** `taskProposalDemoIsFakeOnlyUnapprovedAndCanClose`、`taskProposalDemoDoesNotWriteUntilExplicitApproval`、`taskProposalRotationStartsANewUnapprovedFakeReview`と、専用demo screenの2件でclose、承認前write=0、rotation/session更新を確認。
- **Accessibility / visual state:** `navigationDestinationsRemainSemanticallyAvailable`、`screenHeadingsAreExposedToScreenReaders`、`fontScaleTwoKeepsTaskProposalReviewAndFooterReachable`、`SereaThemeTest`のDark/Light scheme、Gmail/CalendarのLoading/Empty/Failure/Retry各2件。

`assembleDebugAndroidTest`はローカルPASSです。ローカルに接続されたAndroid emulator/deviceはなく、API 26/35でのInstrumentationはNOT_RUNです。GitHub Android run `39595336592`で各APIの98 JUnit casesと両API Process Deathを実行しました。個人データのある端末は使用していません。

固定PR #70 instrumentation sourceの72 test method / 10 required classesを最終sourceと比較しました。既存72 methodはすべて名前まで保持され、欠落はありません。残り26 methodを4 class追加し、計98 / 14 classesです。

### Build、Lint、Security

実装HEADで次のローカルGradle taskをPASSしました。

```text
./gradlew --no-daemon :app:testDebugUnitTest :app:assembleDebug :app:assembleRelease :app:assembleDebugAndroidTest :app:lintDebug
```

Gradle build/lintはPASS、Lintは0 error、12 Warning、2 Informationです。Warningは依存library更新提案8件、`SharedPreferences.edit` KTX提案3件、Calendarの明示公開signatureに関する`ModifierParameter` 1件です。Informationはprimitive state allocation提案2件です。JUnitは252/21・0 failure/error/skipでstrict verifier PASSです。静的regression suiteは18件PASS。GitHub run `39595336592`のDependency Audit、Security、Unit、Debug/Release/Instrumentation Build、LintはPASSしました。Storage/Emulator jobはqueuedです。

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

### 最新実装・テストHEAD `806db1f912d78e197ce8937985dd57424cdfab72`

| Workflow | Run ID | 結果 / 状態 |
|---|---:|---|
| Android | `39595336592` | FAIL。Unit/Security/Dependency/Build/Lint/provenance PASS。API 35/26それぞれ98件を実行、同じ3 UI failures。Source/class/XML verifier PASS。Process Death両API PASS、device SDK 35/26確認済み |
| Fast CI | `39595336487` | PASS |
| Full CI | `39595336476` | PASS (Linux stable、MSRV 1.85.0、macOS ARM64/Intel、Windows Gate C) |
| Native platform validation | `39595336569` | IN_PROGRESS (11/12 jobs PASS、Stable macOS x86_64 final job running) |
| Cross-architecture SQLite portability | `39595336506` | IN_PROGRESS (3/4 jobs PASS、Intel-to-ARM open job queued) |
| Git hooks Windows compatibility | `39595336407` | PASS (macOS, Windows, Ubuntu hook regression) |

Android run `39595336592` artifacts are attached to source SHA `806db1f912d78e197ce8937985dd57424cdfab72`. Unit JUnit artifact was downloaded and its ZIP SHA verified against GitHub's artifact digest; it contains all 21 XML suites used by the strict verifier. Emulator evidence artifact `12344789097` records verified SDK 35/26, 98 cases per API, 3 failures per API, and Process Death PASS per API.

| Artifact | ID | SHA-256 |
|---|---:|---|
| `serea-android-unit-junit` (252 / 21 strict JUnit evidence) | `12344505315` | `599e6441e9c8e2f2c80fbbb16da46dfafedcc5ce75f6bff026beb02d093903c7` |
| `serea-android-security-junit` | `12344500524` | `5fcd29ee953a0d3045e5768d1effa5d6b759bd7ea77859767a0d6adae4e042ad` |
| `serea-android-integration-junit` | `12344241857` | `4402d571fe3a066d6d40b3e4a3b58e5670dacb9738dc00a4235317ad080948bc` |
| `serea-android-dependency-audit` | `12343304086` | `452ba713b370fe87531fa6ac5b241f70bf1f1296fb6a4b3ff7c055acd82f4cef` |
| `serea-android-pixel7a-debug-apk-candidate` | `12343724592` | `d1e04983e9f6b99b6a0451f40a880cdbcffa86615e0c33e80deb96c9a938f155` |
| `serea-android-emulator-evidence` (API 26/35 JUnit, logcat, Process Death, provenance) | `12344789097` | `6d341c2f5365853fcd007e439c60d806c3c727cb585c97ad1b6f2fb26e41840a` |

Android workflowはPR #70からAPI別結果分離、実SDK確認、API 26未実行時のNOT_RUN、JUnit source/class/testcase照合、両API Process Death gateを維持しています。Instrumentationは各APIで98件 / 14 classを要求し、空・重複・欠損・skip・failureを許しません。APK provenance gateも保持し、source SHA、checkout SHA、APK hash、certificateを照合します。Workflow、Required Gate名、JUnit verifierを緩和していません。run `39595336592`はAPI 35で10m38s、API 26で3m09s実行し、両API Process DeathはPASS、各98件中3件failureでした。

直前のemulator run `39571225537` (HEAD `b1769d0`)はAPI 35のCalendar list/detail testが約50分実行後workflow timeoutとなり、API 35のJUnit XMLには50 testcase rows、2 failuresが記録され、98件すべての実行を完了できませんでした。API 35 emulator終了後、API 26向けの実行に古いAPI 35 emulatorが残り、SDK 35と検出されたためAPI 26はNOT_RUNでした。`806db1f`ではshell用Calendar fixtureをbulk 0..120に制限し、test timeoutを120秒に設定、API 26前にemulator detach/ADB resetを追加しました。後続runではSDK分離、98件のJUnit、両Process Death PASSを確認できましたが、以下3 UI failuresが残りました。今回の修正ではshell fixtureをbulk 0..71へさらに絞り、pageSize 50で複数ページを保ちます。

| Test | API 26/35 evidence | 修正 |
|---|---|---|
| `fontScaleTwoKeepsTaskProposalReviewAndFooterReachable` | 両API: fontScale 2、320x640で`chat-composer`が表示範囲外 | compact ChatでShell経由Tasks buttonを省略、DEMO/Cloud無効表示を統合、header/empty state/inputを縮小、composerを1行にする。Tasksはbottom navigationから到達可能 |
| `calendarDetailAndSelectedCalendarSurviveAnotherDestination` | API 35: `performScrollToNode(bulk-120)`が120秒timeout。API 26:単発swipe後もscroll offset 0 | shell fixtureをbulk 0..71、provider pageSize 50へ縮小し、第2ページのevent50を選択。日付query後はfixtureのInstantをsystemDefault ZoneIdで日付化し、event40/64へ明示scroll。Global切替前に「今後」へ戻す |
| `gmailSearchAndMessageDetailUseTheFakeProviderFixtures` | 両API:検索heading遷移後もsubject tag 1件を待つCompose timeout | tagがclickable Card内のchild Textにあってmerged semantics treeから見えないため、tagをCardへ移す |

API 26/35のJUnit XMLは各98件 / 14 class、各3 failures、0 errors / 0 skippedです。Source/class/XML verifierはPASS。修正後の新HEADでemulator campaignを再実行し、成功を確認するまではMerge Readyとしません。

過去のemulator run `39524393018`はAPI 26/35で各6 failures、`39530282680`はAPI 35で7 failures、API 26で5 failuresを記録しました。JUnit/source/class verifierと両API Process DeathはこれらのrunでもPASSしました。画面状態同期、Gmail/Calendar Back、Task notice、large-font reachability、誤ったassert oracleを修正しており、これらは修正前の履歴です。今回の決定的証拠は上記最新HEADの新しいAndroid runです。

Android run `39523520371`の初回Unit失敗は、初期restore完了前にsendする旧test fixtureの問題でした。既存会話をfixtureに設定してrestore後に実行するよう直し、後続run `39524393018`では252件のUnit/JUnit、Security、Integration、Dependency Audit、DocumentationがPASSしました。これは現HEADの最新CI結果ではなく、過去修復の証拠です。

## 独立レビューと指摘

実装者とは別のreviewerが元HEADと修正差分をArchitecture/Lifecycle/Concurrency、Android UX/Accessibility、Security/Data Egress、Regression/CIの各観点で再レビューしました。UX reviewerが報告したCalendar日付状態とGlobal切替のP1は修正し、UX/Architecture/Regression reviewersが最終差分で解消を確認しました。Security reviewerも最新差分にP0/P1/P2なしと確認しました。修正後HEADのemulator実証は新CI待ちです。

| 観点 | 結果 |
|---|---|
| Architecture / Lifecycle / Concurrency | P0/P1なし。Chat履歴切替、Gmail検索A→B、Calendar選択A→B、Task save generation、TaskProposal二重承認/session/cancellation、Activity recreationを確認。cancel後のChat遅延失敗は再現test付きで修正し、Aの`OUTCOME_UNKNOWN`とBの状態を維持 |
| Android UX / Accessibility | compact Chatによるcomposer未表示を修正。Calendarの日付queryからGlobal切替時の空リスト予測(P1)を修正し、systemDefaultに応じる日付計算を追加。最終source reviewでP0/P1なし。修正後のAPI実証はCI待ち。TalkBack traversal、IME表示中のsystem Back、dark-mode画面描画はNOT_RUN |
| Security / Data Egress | P0/P1なし。Google scope/token-lease契約、Mock-only/OAuth OFF、Credential/FLAG_SECURE、隔離Fake、body/token/account loggingとartifact境界を確認。最終Instrumentation差分にデータ経路なし |
| Regression / CI | run `39595336592`で98/14 Instrumentation source/XML照合、252/21 JVM、両API Process Death PASSを確認。API 26/35各3 UI failuresに対する修正をレビュー済み。修正後のrerunが必須 |
| Adversarial | P0/P1なし。stale session/result、操作中のfeature切替、承認迂回・二重mutation、assert false positiveを確認。Gmail restoration oracleとCalendar reset oracleは修正済み。font fixture restoreも最終再確認済み |

### Owner Decisions / Beta blockers

残るP2 / Beta前の判断事項:

- Chatの削除確認用ID/titleとTask editorのtitle/description/due fieldsはActivity-scoped ViewModelに置き、Android SavedStateへ書き込みません。Task DatePicker/TimePicker subtreeもSaveableStateRegistryへ登録しません。Activity再作成ではTask editor draftを維持しますが、OS process death後にeditor visibility/draftを復元しません。process death後の未保存draft破棄は設計どおりで、process-death専用draft assertionは未実装です。
- TaskProposal DemoのSessionはSavedState/Parcelableへ保存せず、Activity再作成では新しい未承認Sessionになります。アプリ固有testは`ActivityScenario.recreate`を使い、TaskProposal単独のOS process-death assertionはありません。PR #70の両API Process Death gateは別に維持しています。
- 古いTask saveが新しいeditor中に完了/失敗した場合は画面通知と新draft保持をInstrumentationで検証しました。古いsaveがCancellationExceptionになる分岐の通知・新draft保持は直接assertするInstrumentation coverageがありません。ViewModelのgeneration gateは静的レビューで確認済みで、実動作のP2 coverage gapとして残します。
- Calendarは画面の日付入力/選択日を`rememberSaveable`へ保存します。現在の画面は`FakeCalendarProvider`のみを使いますが、日付文字列はユーザー入力由来のメタデータです。Calendar/event identifiersおよびGmail message/thread route identifiersも画面状態に保存されますが、現接続ではfakeのsynthetic IDsのみです。これらを実providerへ接続する前にSavedStateのデータ分類を再確認してください。
- Gmail destinationが画面ごとにFake providerをrememberする一方、Activity-scoped ViewModelが初期providerを保持します。固定・stateless fake接続では動作影響はありませんが、provider設定可能化前にlifecycleを揃えるかprovider/session再作成契約を決めてください。
- Chatの未送信draftはActivity/ViewModel再作成では保持されますが、OS process death後は暗号化履歴から復元しません。履歴は暗号化保存を維持します。draftのprocess-death保持がBeta要件か決定が必要です。
- API 26/35の自動Instrumentationとは別に、実端末でのTalkBack traversal、IME表示中のsystem Back、画面別dark-mode renderingはNOT_RUNです。個人データのある実機は使用していません。

Owner decisionsはSavedState metadataの将来の実provider接続時の分類、unsent Chat draftのprocess-death期待値、正式なBeta手順でのvisual/TalkBack/IME reacceptanceです。Android emulator regressionsはコードとsource reviewで修正済みですが、次HEADのAPI 26/35でPASSするまで未解決の必須gateとして扱います。PRはDraftのまま保全します。

## NOT_RUNとMerge Readiness

- ローカルAPI 26/35 instrumentation: **NOT_RUN** (`adb devices -l`に接続端末なし)。GitHub run `39595336592`: SDK 35/26で各98件、各3 failures。Process Death API 35/26 **PASS**。修正後HEADでの再実行は進行予定。
- 実Google OAuth、実Gmail/Calendar account、実AI provider: **NOT_RUN**。今回の制約に従い有効化していません。
- 個人データのある実機Instrumentation / Pixel 7a manual reacceptance: **NOT_RUN**。
- Beta署名/release、mainへのmerge: **NOT_RUN**。

**Merge Readiness: Draft / Not ready.** Android Security/Dependency、Build/Lint/Unit、Fast/Full/Git Hooksは`806db1f`でPASS。修正後のDebug/Release/Instrumentation BuildとLintもローカルPASSですが、Calendarの追加日付修正を含む最新ソースでの全CIとAPI 26/35 emulator campaignは未実施です。Required gatesは維持し、修正前のfailureをPASS扱いしません。
