# Serea Android M0-08 A/B/C/D 統合レポート

Date: 2026-10-20

Status: 統合候補。最終HEADのGitHub Actionsが完了するまでCI判定は未確定。

Base: `94492d281fe0ee583744c932259c3525d5c34f86`

Integration branch: `codex/android-m0-08-abcd-integration-2026-10-20`

Implementation HEAD before this report commit: `04d8da0f0f38f9839c2b81019f8d36463fd955f9`

## 1. Git Topology

PR #50のHEADを共通Baseとして、新しい統合ブランチを作成した。A/B/C/Dの各HEADはすべてBaseをmerge-baseとし、ソースコミットを履歴に残している。PR #50はPR #50をbaseとする候補のまま維持する。mainは変更していない。

## 2. Source PR / SHA一覧

| Source | PR | Source HEAD | Base SHA | Baseからのコミット数 |
|---|---:|---|---|---:|
| A Provider Integration | #55 | `347dcba414babd1cc75b2e86c539278bbf95ddec` | `94492d281fe0ee583744c932259c3525d5c34f86` | 3 |
| B Usage Ledger | #53 | `34f90471e5adf37de064137587d533304fe60325` | 同上 | 13 |
| C Encrypted Tasks | #56 | `274fb97c9b7609157cffba13e6ca92f7365de34d` | 同上 | 13 |
| D Calendar Mock | #57 | `ecbdd73ffed3a9cbb6de8fbf54f511793cf58192` | 同上 | 8 |

## 3. Integration Branch

`codex/android-m0-08-abcd-integration-2026-10-20` はBase SHAから開始した。対象PRはこのブランチだけであり、Draft PRのbaseは `codex/android-m0-04-integration-candidate-2026-10-20`。

## 4. Merge Strategy

順序どおり通常の `--no-ff` mergeを行い、rebase、force-push、履歴書換えは使用していない。

| Merge | Commit |
|---|---|
| A / PR #55 | `c21a2bbcc25bf95db63e75362fa2736b9ceab195` |
| B / PR #53 | `06fc79829b6ec4313c9e5b04261c5640ee17d847` |
| C / PR #56 | `28521ee2a321f9fead0606dc0f0c06fafd29be34` |
| D / PR #57 | `30467af6c77c6931d3139fe418fd6c3426e0ada7` |

Router/Usage接続を `df070e71b9172b341d4d85a89c19c19149119540`、Chat/Tasks両DB統合テストを `04d8da0f0f38f9839c2b81019f8d36463fd955f9` で追加した。

## 5. Commit Identity

Repository-local Codex設定を使用した。`serea.identityProfile=codex`、`user.name=Codex`、`user.email=noreply@github.com`、`user.useConfigOnly=true`、`push.default=nothing`。Author/Committer環境変数の上書きはなかった。pre-commit / pre-push hooksを有効化し、各merge commitと追加commitの前にGit preflightを実行した。

各Source SHAのBaseからの全コミットをIdentity Gateで検証し、統合HEADのBase以降43コミットもPASS。旧PR #52の不適切なSHA `0f917945b2c4964ca0bbffe75be2ff6cf692dd27` と旧PR #54 SHA `3a54ace78f4d4a2992e5b011d99ea9348d3ae30f` はBaseおよび統合HEADの祖先にない。

## 6. Conflict Resolution

- `.github/workflows/android.yml`: A/B/CのUnit selectorとInstrumentation verifierを統合。どのPRのselectorも欠落させない。
- `ChatHttpE2ETest.kt`: Git上は自動mergeされたが、内容を意味的に監査。AのRouter/HTTP/二モデル/拒否テストとBのUsage attributionテストを併存させ、統合用テストを追加した。
- 他の直接競合はなかった。Dは専用Calendar packageとWorkflowを使用した。
- Source PRの固有ファイル差分は統合HEADに存在する。意図的な競合解決を除き、機能変更の欠落は確認していない。

## 7. Android Workflow統合

Required job名、Android Emulator Required Gate、API 26/35、Lint、Dependency Audit、Documentation、Artifact出力、Process Death実行を維持した。必須Unit selectorは `ChatViewModelTest`、`AdaptiveModelRouterTest`、`UsageLedgerCostTest`、`TaskDomainTest`、`TaskRepositoryTest`、`TaskProposalTest`。既存のIntegration jobは `OllamaCloudAdapterTest`、Security jobは `SecurityPortsTest` を実行する。

VerifierはAPI 26/35のそれぞれで、次のSource Countと同数のJUnit test caseを要求する。合計59は7クラスのSource Countの和であり、任意に固定した数字ではない。

| Instrumentation class | 必須件数 |
|---|---:|
| `EncryptedHistoryTest` | 16 |
| `ChatScreenTest` | 16 |
| `ChatHttpE2ETest` | 4 |
| `UsageLedgerDatabaseTest` | 12 |
| `TasksDatabaseTest` | 6 |
| `TaskScreenTest` | 4 |
| `ChatTasksDatabaseIntegrationTest` | 1 |
| **合計** | **59** |

VerifierはJUnit XMLの欠落、failure/error/skip、各クラスの件数不足または過剰を失敗にする。必須テストの `continue-on-error` は設定していない。

## 8. ChatHttpE2ETest統合

最終Sourceには4メソッドがある。既存の二ターン履歴、Router選択とManual unavailable、AのDirect Provider二モデル経路、統合後の二段目Egress拒否を保持する。Direct Provider経路の構成は次の順序。

`ModelChatRequest → RoutedModelChatPort → EgressCheckedModelChatPort → UsageControlledModelChatPort → OllamaModelChatPort → OllamaCloudAdapter → MockWebServer`

HTTP本文のmodel ID、Ledgerのrequest ID/model ID/provider ID、架空価格によるcost、Provider usageのsettlement、重複Send IDの拒否、UNKNOWN料金のnull保存、Fallbackなしを検証する。Denied / unavailable / duplicateではMock HTTPの追加要求と新規Ledger rowがないことを確認する。

## 9. Router / Usage Composition Root

`DirectProviderChatComposition.create()` は後方互換の任意 `UsageAttributionResolver` を受け取る。未指定時は `CatalogUsageAttributionResolver` を使う。Routerで選択されたIDを `ModelChatRequest.selectedModelId` に設定し、その後外側Egress Gate、Usage Reservation、Ollama portの独立Egress Gate、Adapterの順に処理する。

## 10. Provider Attribution

Resolverは再routingせず、選択済みIDと完全一致する1件をCatalogから検索する。Catalogのfreshness、`VERIFIED`またはテスト専用`TEST_FIXTURE` provenance、entitlement、availability、source、検証日時、有効期限を確認する。Provider IDは該当Descriptorの明示値から読み、Model ID文字列から推測しない。条件を満たさないManual選択はReservation前に失敗する。

## 11. Price Snapshot

`VerifiedModelPriceSnapshot`はmodel ID、Bの `VerifiedPriceSnapshot`、有効期限をひとまとまりで保持する。対象ID、Decimal入力/出力価格、ISO currency、source、verified/effective日時、expiry、revisionを検証する。Aの `ModelPricing` のDouble値は請求価格へ変換しない。該当する検証済み価格証跡がなければCostは `UNKNOWN`、amount/currencyはnullとし、0として保存しない。架空価格はMock E2E fixture内だけで、本番Catalogは空のまま。

## 12. Security Boundary

外側Egress拒否はUsageより前に行われ、HTTP、Reservation、Ledger rowが0件。二つ目のOllama Egressが通信前に拒否した場合は、専用のpre-dispatch型を通して当該Atomic Reservation/Ledger/Window更新をrollbackする。Timeout、Cancellation、HTTP応答後の失敗は結果不明として既存どおり予約を保持する。

Calendar originはEgress allowlistに含まれず、TasksはChatContext/Provider requestへ接続されない。本文、credential、raw HTTP responseはUsage Ledgerに保存しない。

## 13. History DB v4

`SereaHistoryDatabase` v4、`model_usage_ledger`、Atomic Reservation/Settlement、soft limit、UTC daily/monthly window、UNKNOWN usage/outcome、重複Send ID拒否を保持する。Migration 1→2→3→4を含む1→4、2→4、3→4移行テストはBのまま。pre-dispatch rollbackは既存Schemaを変更しない。

## 14. Tasks DB v1

`SereaTasksDatabase` v1はHistory DBと独立する。Task payloadは `serea-tasks-v1` Keystore aliasでAES-GCM暗号化される。Historyの `serea-history-v1` aliasと異なり、backup exclusionも維持する。MainActivityに最小限のTasks導線を維持した。

## 15. Process Death

Cのdebug-only harness拡張を保持した。HistoryとTasksのprepare/verify/delete後確認を行い、両SQLiteファイルの存在も検査する。API 26/35それぞれでInstrumentation後にOS Process Death harnessを実行する。Android Runner結果が返るまでは未検証。

## 16. Calendar Mock Foundation

Calendar Domain、read-only port、Fake Provider、Google JSON parser、Agenda、permission policy、timezone/DST、all-day、recurrence instance、cancellation、pagination、security testsを維持する。Source testは22件。専用 `.github/workflows/android-calendar-readonly.yml` を維持し、通常Android workflowと別に実行する。Calendar UI、OAuth、Google通信、書き込みは追加していない。CalendarイベントをChatContext/ModelChatRequestへ加える処理もない。

## 17. Unit / Integration Results

このCodex Cloud workspaceにはGradle実行ファイル/配布物がなく、Gradle Wrapperの取得が `services.gradle.org` へのConnection refusedで停止した。このためローカルAndroid Build/Unit/InstrumentationはPASS扱いにしていない。Instrumentation verifierのPython regression 5件、Android/Calendar workflow YAML parse、Process Death shell syntax、Source Count整合チェックはPASS。

## 18. API 26 / API 35 Results

両APIで59件以上、上表7クラスの正確な件数、failure/error/skip 0、Process Death成功をRequired Gateにした。実際のJUnit XML、API別総数、Process Death結果は最終HEADのGitHub Actions完了後にPR check runから確認する。現時点では未実施。

## 19. All Required CI

Android Build/Unit/Integration/Security/Lint/Dependency Audit/Documentation、API 26/35 InstrumentationとProcess Death、Calendar専用Workflow、Fast CI、Full CI、Native Platform Validation、SQLite Portability、Windows Hooks Compatibility、Commit Identity Gateは、統合HEADをpushしてから実行する。実行前なので各結果は **PENDING**。未実施CIをPASSとして扱わない。

## 20. Existing PR Preservation

PR #50〜#57の既存branch、HEAD、state、targetは変更していない。PRのclose/retarget、main push/merge、force push、branch protection、required check、tag、hook policyの変更なし。新規Draft PRだけをPR #50 baseで作成する。

## 21. Remaining Beta Blockers

- 実Provider利用、production catalog/料金投入前のOwner承認とSecurity review。
- Google OAuth、token lifecycle、実Google通信。
- AI Tool Calling / Task連携。Task proposalはユーザー承認なしにMutationしない。
- 画面完成、実機Pixel 7a再受入、Beta配布準備。
- ADR-0044/0045はPROPOSED / NOT RATIFIEDのまま。

## 22. Owner Decisions

実Providerを有効にする条件、Catalogの信頼sourceと料金Snapshot運用、Google Calendar scope/consent/distribution、Calendar-to-AIまたはCalendar-to-Task連携の許可範囲、Pixel 7a signing/data preservation確認、Beta配布と独立Security reviewを別途判断する。今回のMock結果はこれらの承認を意味しない。
