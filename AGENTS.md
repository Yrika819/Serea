# Serea Android Worktree — Agent Instructions

この独立Git worktreeではAndroid / Kotlin / Compose / Room / PhysicalQAを扱います。Ownerの最新タスクで許可された範囲のみ変更し、このファイルを実機操作やリモート操作の承認として解釈しません。

## 仕事の進め方

- 標準モデルはGPT-6 Luna / High（起動側で指定）。指定値と実際に確認できたモデル・effortを分けて報告。
- ChatGPTは指示設計・軽量確認・レビュー、Zed/Codex Agentは大規模実装・ビルド・検証を担当。承認されたタスクは原因分析からFocusedテストと証跡作成まで完遂してください。
- 常時すべての資料を読まないこと。実機ならAndroid CLI Skill（~/.agents/skills/android-cli/SKILL.md）、変更対象の最新 docs/agent-reports/、該当コードを参照。Room・Google・Nativeの資料は対象作業時に参照します。

## Gitとデータ保護

- git rev-parse --show-toplevelでルートを確定し、HEAD、branch、git status --short、tracked差分、未追跡ファイルを作業前後に確認。既存V15以降の差分やEvidenceを保全します。
- git reset --hard / git clean / 無断rebase / force pushは禁止。別基点のNative PRを丸ごとMergeしません。Commit/Push/Merge/PR状態変更にはOwner承認が必要です。
- タスク範囲内の小規模変更や非破壊Focusedテストは自律実行可。Pixelへのインストール・UI操作・実機テスト、実OAuth、モデル取得、署名、本番アカウント、費用発生はそれぞれ明示承認が必要です。

## Android固有の不変条件

- LOCAL_MODEL_CHAT_ENABLEDと通常製品Native経路はデフォルトOFF。Default Deny / Cloud Disabled / sourceLineage / Egress / Consent / Usage / revocationを迂回しません。Fakeを実Google/OAuth/実推論として報告しません。
- app.serea.android.demoを更新・消去・削除しません。isolated、physicalqa、nativeqaのPackage ID・データ・署名・Runnerを混同しません。
- Pixel実機は対象serialが一意で、lynx / API37 / arm64-v8a等の期待条件をその都度確認。Runnerには正確なTest IDとtokenを指定し、Emulator専用Runnerや広いconnectedAndroidTestで回避しません。
- UI / layout / IME / Backの再開にはOwnerの画面準備確認を要します。**2026-10-31にOwnerがPixel 7aはロック解除済み・通知パネルは閉じており・ホーム画面表示中と明示確認済み**。従って現状この確認条件は満たされ、同じ確認を再要求しません。`NotificationShade`というWindow名の存在だけで通知パネルが開いていると断定しないでください。実際の画面遮蔽やロックを別途確認した場合だけ当該UI操作を停止し、個人通知・他アプリの内容は取得しないこと。Room/Keystoreの実データを消去しません。
- Formal Required（API26/35）、Identity、Process Death、Data Isolation、Sentinelの必須条件を緩めません。G5の過去PASSを現在のHEADのPASSに流用しません。

## 検証と報告

- 対象Focusedテストから開始し、Gradle/NDKビルドは逐次、原則 --max-workers=2 --no-daemon。
- Kotlin compile、APK assemble、署名/権限監査、実機JUnit、正式CIを個別に評価。PASS / FAIL / BLOCKED / NOT_RUNを区別します。
- 重要変更は実装者と別の独立レビューを受け、CHANGES_REQUESTEDなら修正し再レビュー。無断で次のフェーズに進みません。
- 日本語で STATUS / BASE_SHA / HEAD / CHANGED_FILES / TESTS / JUNIT_COUNT / FAILURES / SKIPS / REVIEW / ARTIFACT_SHA / SECURITY_IMPACT / BLOCKERS / NEXT_ACTION を報告し、該当Ledgerを更新します。

参考: https://developers.openai.com/blog/rethinking-skills-and-prompts-for-gpt-6-astra