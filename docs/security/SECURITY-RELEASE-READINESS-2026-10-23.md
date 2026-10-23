# Serea Security & Release Readiness — 2026-10-23

## 監査対象と証拠の扱い

対象はPR #98の固定HEAD 00e0453f83970a2b7d2bd26ce01462e421f75f46です。PR #98/#99/#100はいずれもopen Draftで、HEADはそれぞれ00e0453 / 8140bd7 / 9dbf5df、この順に祖先関係を確認しました。変更はPR #100の所有ファイルを含まないRelease・Firebase・更新テスト・独立Security領域に限定しています。

SOURCE_VERIFIEDはコードの動作経路を確認した状態、MOCK_PASSは秘密情報なしのローカルfixture試験、OWNER_REQUIREDはGitHub/Google/Firebase設定の現物証拠待ち、NOT_RUNは実端末・実署名・実配布など未実施の項目です。モック結果を実環境のPASSとして扱っていません。

## Threat Matrix

| 脅威・資産 | コード上の確認 | 状態 | 残る受入証拠 |
|---|---|---|---|
| DemoとBetaのpackage/data分離 | debugはapp.serea.android.demo、Betaはapp.serea.android.beta。Android application IDごとにアプリデータ領域が分かれる。Beta workflowはBeta IDのみを検証する。 | SOURCE_VERIFIED | PR #98/#100のAndroid Emulator Required Gateは失敗し、Personal Beta Identity Isolation jobはskip。実Beta APKの並行インストールはNOT_RUN。 |
| Owner署名鍵の保護 | Environment secretからPKCS#12をRunner tempへmode 600で作成。Gradleでalias、鍵、証明書pinを検査し、APK verifierでも証明書を再確認。always() cleanupで鍵・APK build出力を削除。 | SOURCE_VERIFIED | Owner鍵・証明書pin・Environment secretの実設定、実署名はNOT_RUN。 |
| 手動配布の認可とmain制限 | workflow_dispatchのみ、jobはrefs/heads/main限定。canonical repository/workflow/ref/source SHAを確認し、Environment承認・main branch protection・exact-SHA required checksを要求。 | SOURCE_VERIFIED; freshness修復を追加 | GitHub Environmentとmain protectionの実状態はOWNER_REQUIRED。 |
| dispatch SHAの鮮度 | 旧コードはmainの祖先であるだけで許可し、Environment承認待ち中にmainが進んだ場合、古いdispatch SHAを使う余地があった。現在のmain tipとの一致を署名build前・attestation前・Firebase送信直前に再検査する。 | 修復済み、MOCK_PASS | GitHub上のmain更新競合を含むWorkflow実行はNOT_RUN。 |
| Environment reviewer設定のfail-closed | 旧コードはtruthyなmalformed reviewer値を通す可能性があった。required reviewers配列のUser/Team種別と正のIDを検査するよう強化。 | 修復済み、MOCK_PASS | Owner自身が承認者として設定されていることはOWNER_REQUIRED。 |
| Workload Identity Federation | Long-lived service-account JSON fallbackはなく、配布にはWIF provider、service account、Firebase project/app/group、明示的enableが必要。ID token permissionはRelease jobに限定。 | SOURCE_VERIFIED | WIF provider CEL条件、SA binding、有効なclaim制約はOWNER_REQUIRED。 |
| Firebase App Distribution | 配布前にFirebase App IDがAndroid package app.serea.android.betaに結び付くことを一覧APIで確認。配布先は単一group alias。Firebase CLI生出力は結果ログへ出さない。 | SOURCE_VERIFIED, fixture tests MOCK_PASS | 実Project/App/group、group membership、SAの実効IAM権限はOWNER_REQUIRED。実配布NOT_RUN。 |
| APK package / version / checksum / signer | aaptでBeta package IDとversionCode、apksignerで唯一の証明書SHA-256、SHA-256 file digestを検証。Firebase送信直前にもAPK digestを再計算。 | SOURCE_VERIFIED, negative tests MOCK_PASS | Owner鍵で生成したAPKの実署名結果はNOT_RUN。 |
| 同一鍵更新・異鍵拒否・データ保持 | Beta identity testは一時鍵でBeta v100→v101更新、Roomデータ回復、異鍵v102拒否を試す構成。更新互換性fixtureはv100→v101の同鍵更新、異鍵拒否時のインストール状態保持を検査する。 | fixture MOCK_PASS; test design reviewed | 実API 35 emulatorでのBeta identity testは最新Android CIでskip。実端末受入NOT_RUN。 |
| GitHub Actions ArtifactへのOwner署名Beta混入 | Release workflowにartifact uploadはない。通常Android buildはDebug/Release/Instrumentation APKだけをbuild/uploadし、Betaはcompile-only。Beta identity jobは一時署名結果のテキストだけをuploadする。 | SOURCE_VERIFIED, regression test MOCK_PASS | GitHub上のOwner署名run artifactは存在せず、実run NOT_RUN。 |
| OAuth Token再利用・アカウント切替 | AccessTokenLeaseは最大5分、close・feature revokeで再使用拒否。Fake gatewayはaccount switchを試験する。一方、実Play Services gatewayはscope単位でgrantを保持し、account IDを結び付ける実統合テストがない。 | 基本経路SOURCE_VERIFIED; account binding NOT_PROVEN | Real OAuthはdefault disabled、実OAuth/アカウント切替/失効後の実Token試験はNOT_RUN。有効化前のOwner受入判断が必要。 |
| Gmailデータの無断Cloud送信・Prompt Injection | Gmail UIは現在Mock sourceのみを受け入れ、Gmail originはEgress許可集合に含まれない。Chat contextは同一会話の完了履歴だけから作り、未知・個人・機密分類、未承認intent、期限切れintent、別destinationを拒否。TaskProposalは5分期限・単回実行。 | SOURCE_VERIFIED; unit test source reviewed | Real Gmail→AI end-to-endは接続されておらずNOT_RUN。認可境界の将来接続時に再受入が必要。 |
| Keystore / Room / Backup / logs | Room本文・タイトルはAndroid Keystore AES-GCM。Credential repositoryはhardware-backed keyを要求。Manifestでbackup無効、DB/shared preferences/external領域をbackup/transferから除外。Token toString()はredacted。 | SOURCE_VERIFIED | 実端末のKeystore hardware level、backup restore、logcat/crash telemetry検査はNOT_RUN。 |
| Supply-chain改ざん | GitHub Actions参照は50/50が40桁commit SHA。Android wrapper distribution URLは8.14.6とSHA-256固定。Android依存lockfile、Cargo 4 lockfile、Firebase npm lockfileあり。 | source inventory PASS; Publisher authenticity NOT_PROVEN | Gradle verification metadataがなく、Gradle artifact/POM publisher authenticityは未検証。enforcementは有効化しない。 |

## 実装した修復

1. tools/android_firebase_distribution.pyにmain tipの完全一致検査を追加し、欠落・malformed SHA・dispatch後のmain進行を拒否します。
2. .github/workflows/android-personal-beta.ymlはEnvironment承認後にmainをfetchし、Owner署名build前、attestation前、Firebase upload直前にも同じmain SHAを要求します。古いrunや古いworkflow sourceの配布を防ぎます。
3. Environment reviewer検証を厳格化し、GitHub API形式のUser/Team reviewer objectと正の数値IDを確認します。空でない文字列や不正型は受け入れません。
4. Release contract testにmain tip mismatch、malformed config、malformed reviewerのNegative Testを追加しました。
5. Actions artifact回帰試験はupload path globを解析し、android/app/build/**、**/*.apk、Beta APKに一致する広すぎるpatternを検出します。
6. 更新互換性testは別の一時署名鍵での更新拒否を行い、拒否後のversionCodeと保存データが残ることを確認します。実エミュレータ試験はDisposable Emulator guardで囲みます。

PR #100が編集する.github/workflows/android.yml、android/app/build.gradle.kts、Calendar UI、Instrumentation inventory、Host lifecycle等は変更していません。

## Supply-chain判定

- .github/workflows: 10 workflowすべてにrootまたはjob levelの明示的permissionsがあります。pull_request_targetとbranch/tag名だけのAction参照は見つかりませんでした。全50 Action参照がcommit SHA固定です。SHA固定はPublisherの真正性検証を意味しません。
- android/gradle/wrapper/gradle-wrapper.properties: Gradle 8.14.6、distributionSha256Sumあり。Wrapper JAR自体の独立Publisher照合は未実施です。
- android/app/gradle.lockfile: lockAllConfigurations()あり。Firebase CLIは15.34.1で、634個のregistry packageすべてにnpm integrity hashがあります。
- Rustはrootとvendor内の4 lockfileをscanner対象として定義。GitHubの最新#100 RustSec jobはPASS。ただしPull Request時は既定でrootと変更されたvendor lockfileだけをscanし、変更のないvendorは軽量経路です。#98の記録には4 lockfile full scan PASSがあります。
- android/gradle/verification-metadata.xmlは存在しません。既存docs/security/gradle-verification-enforcement-readiness.mdはpublisher証拠・task coverage不足と判定し、enforcementを有効化しない判断を明記しています。
- tools/test_gradle_dependency_verification.shはローカル実行を試みましたが、Gradle 8.14.6 distribution取得時にConnection refusedで停止しました。これはnegative fixtureの結果ではなく、今回のlocal verificationはBLOCKEDです。既存の一時fixture test記録を保持し、検証不足を理由にenforcementは変更していません。

## Focused test結果

| Test | Result |
|---|---|
| python3 -m unittest discover -s tests -p android_firebase_distribution_tests.py | PASS — 21 tests |
| bash tools/test_android_update_compatibility_guard.sh | PASS — physical/multiple/unauthorized/fake/wrong API/serial mismatch/missing designationをADB mutation前に拒否。mocked API 35経路は同鍵v101更新、異鍵拒否、data retention確認 |
| bash -n tools/test_android_update_compatibility.sh tools/test_android_update_compatibility_guard.sh | PASS |
| bash tools/test_gradle_dependency_verification.sh | BLOCKED — Gradle distribution downloadがConnection refused |
| PR #100 Fast CI #598 / Full CI #587 | PASS |
| PR #100 Android Personal Beta Contract #16 / Git hooks #336 / Cross-architecture SQLite #565 | PASS |
| PR #100 Android Dependency Audit job | PASS — latest run #268 |
| PR #100 Rust Dependency Security job | PASS — latest run #21 |
| PR #100 Native platform validation | PASS — latest run #387; all 12 matrix jobs completed |
| PR #100 Android Emulator Required Gate | FAIL — API 26/35 instrumentation failure。Beta Identity Isolation jobはSKIP |
| Owner signing / Firebase / real Google OAuth / physical device / real Beta emulator acceptance | NOT_RUN |

MOCK_PASSはGoogle/Firebase IAM、Owner鍵、実emulatorや実配布の証拠ではありません。

## Owner Setup ChecklistとEvidence Contract

| 設定 | Ownerの実施事項 | 保存する受入証拠 |
|---|---|---|
| GitHub Environment | serea-personal-betaを作成。Owner承認者をrequired reviewerに設定し、deployment branch policyをprotected branchesに限定。SecretsとVariablesをこのEnvironmentに限定。 | Environment API設定のredacted JSON、承認者/branch policyのOwner確認記録。 |
| main Branch Protection | Required status checksをSEREA_REQUIRED_CHECKS_JSONと完全一致させる。最低1件のPR approval、administrator enforcement、直push禁止を設定。 | /branches/main/protectionのredacted JSON、required check names一覧、Protection変更履歴。 |
| Firebase Project/App/Group | Firebase Android app packageがapp.serea.android.betaと一致することを確認。Owner tester groupを作り、最小人数にする。 | firebase apps:list androidのredacted結果、app ID/package結合、group membershipのOwner確認。 |
| WIF | Providerのattribute mapping/conditionをこのrepo、refs/heads/main、serea-personal-beta Environment、.github/workflows/android-personal-beta.ymlに限定。Service account側のWIF principal bindingも同じ条件にする。 | Redacted provider CEL condition、mapping、service-account IAM binding、OIDC claim sample（token自体は保存しない）。 |
| Firebase IAM | Service accountへFirebase App Distribution uploadとapp inventory readに必要な最小権限だけ付与。Project Owner/Editorや長期JSON keyを配布jobへ与えない。 | Role名、scope、effective permissionのredacted IAM出力。現時点でコードから実権限は確認できない。 |
| Owner署名鍵 | 既存Beta配布がある場合は同じ署名鍵を使い、rotationはAndroid signing lineage手順として別途承認。KeyStoreファイル、password、aliasはrepo/artifact/issueに置かない。Baselineは過去に配布した最大versionCode。 | Offline keytool fingerprintとAPK verifier fingerprintの一致、鍵保管/backup/アクセス権、Environment secret名だけ。秘密値は保存しない。 |
| Google OAuth | Android OAuth clientをpackage/certificateに結び付け、Gmail readonly、Calendar events readonly、Calendar list readonlyだけを申請。Google consent screen/verificationとrestricted scope要件を完了。 | Client ID（公開識別子）、package、certificate SHA-1、scope一覧、Google verification状態、失効試験の結果。 |
| Emergency Disable / Revocation | 配布停止はSEREA_FIREBASE_DISTRIBUTION_ENABLED=falseに加えWIF service-account bindingを外し、Environment signing secretsを削除/無効化。Tester group membershipを止める。OAuth停止はGoogle grantをrevokeし、アプリ側featureをdisable。 | disable設定、WIF/IAM撤去結果、group無効化、再実行がfail-closedになったrun、OAuth revoke後のtoken再利用拒否記録。 |
| Release Candidate署名・更新 | main tip、required checks、Environment承認、package ID、versionCode、APK SHA-256、certificate SHA-256、provenanceを同じSHAで確認。実配布を許可するOwner決定がある場合のみFirebase uploadを実行。既存Betaから同じOwner鍵でupdateし、local encrypted dataを確認。 | workflow run ID/source/tree SHA、check run結果、APK digest/cert/package/version、attestation verifier結果、Firebase release ID/group、update後data確認。APK bytesやkeyはpublic GitHub Artifactに保存しない。 |
| CI artifact/log review | GitHub Actions artifactの一覧・各path globとrelease summary/logを確認し、Owner署名APK、OAuth token、keystore、service-account credential、メール/予定本文がないことを検査。 | Artifact名/path一覧とredacted log scan記録。 |

## 未証明項目とOwner判断

1. 現Environmentのreviewerとbranch protectionはこの監査から取得できず、Owner設定済みとは判定していません。
2. Google WIF CEL条件、service-account IAM、Firebase tester groupが未確認です。
3. 実OAuthのaccount switchはproduction gatewayで検証されていません。Fake gateway testだけでは実Google account identity bindingを証明できません。Real OAuthはdefault disabledでGmail UIもmock source限定ですが、有効化前にaccount-switch後のold token rejectionとaccount-bound view/data clearingを受入するOwner判断が必要です。
4. Android Dependency AuditとRustSecはCIでPASSですが、Gradle publisher authenticity未証明のためDependency Verification enforcementを有効にしません。完了条件は既存gradle-verification-enforcement-readiness.mdに記載されたpublisher evidence、configuration/task coverage、fresh cache full matrix、negative tests、独立reviewです。
5. Owner鍵による署名、Firebase配布、実Google OAuth、物理device、Beta update emulator testはすべてNOT_RUNです。

## PR #100との安全な統合手順

1. この作業はPR #98固定HEADから派生した独立branchのまま保持し、PR #100がopenの間はそのHEADへrebase/cherry-pickしません。
2. PR #98→#99→#100が順に受入/mergeされた後、mainがPR #100 HEADの祖先であることをfetchして検証します。
3. Draft PR baseをmainへ変更する前後でmerge-base、base/head diff、変更path一覧を再確認し、このPRの差分がRelease/Firebase/update/securityファイルだけであること、PR #100所有ファイルとの交差が0であることを確認します。
4. Mainが進んだ場合はCIをそのexact head SHAで再実行します。Release workflowは配布直前のmain tip不一致を拒否します。
