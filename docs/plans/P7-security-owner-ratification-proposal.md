# P7 Security Owner Ratification Proposal

**状態:** 2026-10-12 に原則のみ所有者承認済み。数値・技術詳細は `PROPOSED` のまま。実装未許可。
**権威記録:** [ADR-0042](../decisions/ADR-0042-p7-owner-ratified-directions.md) と [P7 owner-ratification record](P7-owner-ratification-record.md)。
**Process state:** `P7_OWNER_DIRECTIONS_RATIFIED_DETAILS_PENDING`.
**対象:** P7-D01〜D07 のセキュリティ契約。P7B の実装許可ではない。
**前提:** Accepted ADR、プロトコル、migrations 0001–0008 は変更しない。

## 読み方と現状

Accepted 契約は [P7A owner decision package](P7A-owner-decision-package.md) と [P7–P8 cross-phase plan](P7-P8-cross-phase-contract-and-test-plan.md) の記載どおり有効である。とくに ADR-0006 の FTS5 lexical-only、ADR-0010 の分類・資格情報排除、ADR-0022 の未完了な PRIVATE 保護、Task Protocol §8 の Task と独立 Memory の保持、Bounds Protocol の既存上限を再定義しない。

現在のコードに Memory/FTS/Forget 実装はない。Task は認証済み OwnerId を持たず、`origin_device_id` は所有者の証明ではない。文書化された制約は実装の証拠ではない。P7B は明示的に未許可であり、numeric bounds、Forget exhaustion reserve/quarantine/ledger lifecycle、OS-authenticated Owner bootstrap、provenance/Forget詳細、SQLite制約証明と実装許可を含む残件が解消するまで **BLOCKED** とする。

以下の七つの推奨方向は2026-10-12にOwnerが明示承認した。承認はADR-0042に列挙された原則だけに限られる。このproposal内の詳細、候補値、運用機構の比較は引き続き未決であり、承認済みの方向を狭めたり、A/B/C行を暗黙選択したりしない。P7B実装は別途明示許可が必要。未決事項には拒否側の動作を維持する。

## P7-D01 — Owner identity

**Ratified direction; detailed contract text remains pending:** 「各 Task はホストの認証済みローカル主体から発行された不透明・安定・非再利用の `OwnerId` に、受付時に一度だけ結び付く。Task と Memory の owner は TaskEngine/Core が認証コンテキストから設定し、モデル、provider、要求JSON、DeviceId、SessionId から導出しない。同一 OwnerId の複数登録済みデバイスは同じ所有者に属し得るが、デバイス登録は本人認証またはユーザー承認を意味しない。owner 不明・不一致・失効・曖昧な移行元は認可拒否とし、Memory 抽出、検索、忘却、Blob 参照を行わない。OwnerId の移譲は拒否し、別の明示的移行契約を要する。」

**具体的なデフォルト案:** V1 はインストール内ローカル単一所有者。初回のローカル対話で OS が保護するホスト認証コンテキストを確認し、ホスト側でランダムな永続 `OwnerId` を発行する。認証の証拠はホストが検証したローカルセッション主体と再認証結果であり、文字列入力、ログイン中という推定、デバイス鍵、署名済みフレーム単独ではない。ホストが当該OS認証を提供できない環境では bootstrap を完了できず、所有者依存機能を停止する。複数端末は明示的に登録された公開鍵を持つが、登録時に同一Ownerへ結び付ける手順と再認証を必要とする。デバイス紛失はそのデバイスとセッションを失効させ、OwnerId を変えない。回復は新しい認証済みホストセッションと別の登録済み回復手段、またはホストの管理者による監査付きローカル再登録を要する。

既存Taskは migration 中に `origin_device_id` から owner を推定しない。移行時に単一OS主体であることを証明できない既存Taskは「未割当」として隔離し、抽出・検索対象にしない。OwnerId の不一致は一般化された拒否を返し、別Ownerの有無や件数を漏らさない。クロスOwnerの作成・取得・更新・検索・忘却は、同一トランザクション内のOwner条件と複合外部キーで拒否する。

**代替:** A) インストール全体を単一の固定Owner名前空間とし、ローカルユーザー認証はOSのアクセス境界に委ねる (実装が単純だが端末ユーザー共有に弱い)。B) OS主体認証付き複数Owner (より明確な分離だが共有OSプロファイルでのセッション分離設計が必要)。OwnerIdなしは許可しない。

**所有者が明示回答する点:** bootstrap の認証手段と失敗時の扱い、V1単一Ownerか複数Ownerか、端末追加の再認証要件、回復者・回復手順、既存Taskを隔離する方針、移譲拒否、OwnerId形式/安定期間。

**互換性:** 新しい additive schema のみ。Task wire形式と旧migrationは変更しない。既存データのバックフィル規則は証拠がなければ quarantine。

**必須検証:** Task受付時のOwner固定、所有者偽装、同一OSユーザー内の別セッション、DeviceId交換、失効端末、Owner不一致、Owner欠落の fail-closed とクロスOwner負例を、SQLite制約を含む独立接続で検証する。

**実装ゲート:** Owner発行者・認証証拠・legacy quarantine・DDL複合キーが合意され、P7B migration proof が通るまで **BLOCKED**。

**方向:** Owner approved 2026-10-12; detailed implementation parameters remain pending.

## P7-D02 — Task retention と Memory provenance

**Ratified direction; detailed contract text remains pending:** 「通常のTask期限切れは、独立して保持されたMemoryを削除しない。Memoryは元Taskを外部キーで保持する代わりに、最小化された不変provenance snapshotを持つ。明示的な認証済みForgetは対象Memoryとそこから派生したMemoryを消す。元Taskや元コンテンツが保持期限で消えた事実をprovenanceへ追記・偽装せず、内容を含まない削除監査と状態を分離する。」

Snapshot は owner、opaque TaskId/StepId/EventId、source capability/version、host抽出IDと時刻、元source class、Memory class、検証済みdigest参照または source row が消えた状態コードだけを保存する。本文、メール件名、アカウント名、抽出プロンプト、鍵なしdigestは保存しない。Task/Eventの物理削除後も参照IDは履歴として保持し得るが live FK は付けない。元のTaskが存在しないことは「元データが正当だった」証明にならない。

通常期限切れはTask/StepとTask保持対象のみを削除し、保持Memory・provenanceを維持する。Source側からの削除通知は、それ自体ではMemory共有を許可しない。派生物の扱いは明示Forget方針による。

**代替:** A) Task削除時に全派生Memoryも消す (単純だがTask Protocol §8と衝突)。B) provenanceを含めsource完存期間だけMemoryを許可する (履歴の独立保持を失う)。推奨は最小snapshotと明示Forget。

**所有者が回答する点:** historical ID をどの程度保持するか、source/Event削除とMemoryの関係、削除監査期間、Snapshotの許容metadata項目。

**互換性:** Task FKのcascadeにMemoryを含めない。追加テーブルとイベント種別はプロトコル/ADRの承認が別途必要。

**検証:** Task削除前後、source削除後、Task期限切れ後のMemory検索、explicit forget cascade、別Owner/共有Blobを含む分離を決定的fixtureで確認する。

**実装ゲート:** 最小provenance field list、通常期限とForgetの違い、保持上限の決定が必要。

**方向:** Owner approved 2026-10-12; detailed implementation parameters remain pending.

## P7-D03 — Schema と supersession

**Ratified direction; detailed contract text remains pending:** 「所有者分離、Memory同一性、分類付きBlob参照、provenance不変性、supersessionとForgetをSQLiteのPRIMARY KEY/UNIQUE/CHECK/FOREIGN KEY/triggerで強制する。Application assertionだけで保証しない。V1 supersessionは同一Owner・同一分類適合範囲の単一後継とし、各itemは最大1 successor。自己辺・循環を拒否し、insertと探索を単一即時トランザクションで線形化する。」

設計要求: `UNIQUE(owner_id,task_id)`、`UNIQUE(owner_id,memory_id)` 等を全複合FKの親に明示する。Memory ID は既存protocolの `EventId` grammarを用いる。Blob FKとlookupは厳密に `(digest,data_class_rank)`。Provenanceは一度挿入後不変。Supersession辺はimmutable。Task/Stepに親UNIQUEが無い複合FKを仮定せず、Task保持を妨げない。

V1 は1件の旧Memoryから最大1件へ置換する単純ルール。訂正が分岐する必要が実証された場合、グラフ拡張は別決定とする。SQLiteの複数接続競合を含むcycle検出を証明できない場合、supersessionの書き込み自体を拒否する。FTSは派生索引であり、行の所有権・分類・存在・忘却の権威源にならない。

**代替:** A) 単一successor (推奨、単純でcycle検証可能)。B) DAGのbranch/fan-in (クエリ・Forget・ロック・収束意味が複雑)。C) 辺なし、旧行を失効化するだけ (履歴関係を失う)。

**所有者が回答する点:** single-successorの承認、削除した旧itemの参照維持期間、旧schemaでTask/Step親UNIQUEを追加するか、FTS contentless/外部content構成。

**互換性:** migrations 0001–0008 は凍結。将来の追加migrationでSQL制約・triggerを付加し、親一意性と制約動作を実SQLiteで証明する。

**検証:** migration途中失敗rollback、複合owner/class違反、Provenance更新拒否、A→B→A cycle、同時接続競合、FTS orphan/integrity検査。

**実装ゲート:** D01/D02確定と実DDLの各親キー証明後。P7B schema design authorizationとは別にmigration適用前のレビューが必要。

**方向:** Owner approved 2026-10-12; detailed implementation parameters remain pending.

## P7-D04 — Classification と保存保護

**Ratified direction; detailed contract text remains pending:** 「初期Memoryは `PUBLIC` と `PERSONAL` のみ。`PRIVATE` を入力・派生・provenance・検索索引・Blob・監査に含む保存要求は、完全な本番保護が監査されるまでトランザクション前に拒否する。`SECRET` と `CREDENTIAL` は通常Memoryの入力、出力、prompt、Blob、FTS、ログ、crash reportに決して入れない。分類は元要求・source・応答・provenance・derived textの最高クラスを継承し、redactionはstored classを下げない。」

PERSONALについてもホストDB directory/file/WAL/SHM/FTS、owner-only OS権限、symlink置換・権限継承、同一端末の他アプリ、OSバックアップ/同期、診断ログ、crash dump、provenance metadata、検索履歴、temporary fileを調べる。OS file ACLとバックアップ除外は最低限の保護で、暗号化の代替や侵害OSからの保証ではない。暗号化を追加する場合はキー分離、unlock/recovery、rotation、WAL/backup/FTS、クラッシュを含む別の設計証明を要する。PERSONALであってもディスク盗難等の残余リスクは明記する。

Cloud egressではSECRET/CREDENTIALは全modelに禁止。PRIVATE cloud egressは既存契約のdefault-disabledを維持する。分類system自体は変更しない。

**代替:** A) PUBLIC/PERSONALのみ、PRIVATE fail-closed (推奨)。B) PRIVATEを受け入れながらFTS/metadataを無保護にする (拒否)。C) audited production encryption + backup/WAL/index/key recoveryを完了してからPRIVATEを別owner decisionで許可。

**所有者が回答する点:** PERSONAL保存の意味ある認識・consent、OS backup除外、file permission最低要件、at-rest暗号化をP7の前提にするか、PRIVATE activationの別ゲート。

**互換性:** 現行class rank/egressを変えない。ADR-0022 Proposedのまま。新規暗号key migrationも行わない。

**検証:** class downgrade/property fuzz、DB/WAL/FTS/temp artifactsへのPRIVATE文字列不在、credential sentinelのserialize/log/crash path不在、file permission・backup policyのtarget OS integration。

**実装ゲート:** P7BはPUBLIC/PERSONALだけの制約案を準備可能。ただしD01 HIGH blocker解消後。PRIVATEは別途production backend・backup/wal/index/metadata検証と明示owner approvalまで不可。

**方向:** Owner approved 2026-10-12; detailed implementation parameters remain pending.

## P7-D05 — Retention、quota、renewal

以下は比較案であり、**どの数値も承認済みではない**。初期の想定端末はローカル1ユーザー、SSD、単一プロセス、SQLite WAL、各Memoryは小さなUTF-8テキスト、検索はページ単位。実測なしの最大値は性能保証ではない。期限・容量超過時は抽出/renewalを拒否し、既存非期限Memoryを黙って削除しない。検索/モデル取得は期限を延長しない。

| 上限 | A: 保守案 | B: 中間案 | C: 大きめ案 | 主な根拠・失敗動作 |
| --- | ---: | ---: | ---: | --- |
| Ownerあたりactive item | 1,000 | 5,000 | 20,000 | 全件走査・復元対象。満杯は新規抽出拒否 |
| 1 item UTF-8 body | 4 KiB | 16 KiB | 64 KiB | hostile extractionとindex cost。超過は切り詰めず拒否 |
| Ownerあたりactive body合計 | 4 MiB | 32 MiB | 256 MiB | metadata除外の明示budget。transaction前に拒否 |
| OwnerあたりFTS index | 8 MiB | 64 MiB | 512 MiB | rebuild用一時余裕を別途確保。quota不明時は検索/書込停止、索引再構築はbounded |
| extraction開始 | 20/日、1同時 | 100/日、2同時 | 500/日、4同時 | model/provider費用とpoisoning面積。超過拒否、queue無制限化しない |
| WORKING retention | 1日 | 7日 | 14日 | 短期contextのみ。通常GCで期限超過分を段階削除 |
| EPISODIC retention | 30日 | 90日 | 180日 | 事実経過の露出期間 |
| SEMANTIC retention | 90日 | 365日 | 730日 | 長期utility/privacy tradeoff |
| PREFERENCE retention | 90日 | 365日 | 730日 | preference誤推定の継続リスク |
| 更新上限 | 0 renew、最大年齢=retention | 明示ユーザーrenew最大2回、合計期間≤2×retention | 明示renew最大4回、合計期間≤5×retention | retrieval/re-extractionはrenewに数えない。上限後は新しいitemとして再抽出+新規同意を要求 |
| GC batch | 25 rows / tx、≤100 ms目標 | 100 rows / tx、≤250 ms目標 | 500 rows / tx、≤500 ms目標 | 時間は目標であってwall-clock保証ではない。timeout/error時commitせずcursorから再開 |
| Forget tombstone budget | 10,000件/Owner、metadata≤256 B | 100,000件/Owner、metadata≤512 B | 1,000,000件/Owner、metadata≤1 KiB | 期限内のsuppressionを維持。budget一杯は新しいforget/reconciliationを拒否してfail-closed、古いものを黙って落とさない |
| Recovery/backup retention | 7日以内、1世代 | 30日、3世代 | 90日、7世代 | backupは暗号化、owner ACL、外部保管可否を明記。forget tombstoneはbackupより長く外部reconcile可能な場所に保持 |

FTS ratioは本文とtokenizer依存で変動するのでA/B/Cのindex byteは仮説値。Rust bundled SQLiteと実コーパスで計測し、必要ならownerが数値を変更する。Tombstoneの期間は復元可能な最大backup期間以上。復元可能期間を無期限にするならsuppression記録も無期限であり、privacy costを明示承認する。Keyを使うsuppressionではキー喪失時に復元snapshotを公開せず隔離する。

**推奨案:** Aを初期提案とし、現場測定後ownerが修正する。ただし数値は個々に未決。`7/90/365/365日` は中間案Bの値であり、現行P7A候補に過ぎない。

**所有者が回答する点:** 表の各数値/行ごとの選択、日/byte/item意味、index実測前の暫定上限、最大backup期間、tombstone retention、quota超過UI/監査。

**互換性・検証:** Bounds Protocolに合う新上限は追加ADR/contract変更を要する。境界値±1、並行抽出、renewal競合、quota rollback、bounded GC継続、backup/tombstone比を検証する。

**実装ゲート:** 数字をownerが選ぶまではmigrationに黙示値を埋め込まない。未決の項目は抽出をdisableするか、厳格なtemporary fail-closed値を明示して別途承認を得る。

**方向:** Owner approved 2026-10-12; detailed implementation parameters remain pending.

## P7-D06 — Retrieval

**Ratified direction; detailed contract text remains pending:** 「初期検索はSQLite FTS5 lexical-only。Ownerとallowlisted classをbase rowで必ずfilterし、FTS matchは候補選択にだけ使う。query grammar、bytes、tokens, page size, ranking workにhost boundを置き、stable tie-breakを持つ。Model outputはquery proposalにすぎず、検索権限・classification・ownerを変更しない。FTS rebuild/integrity mismatchは検索を停止し、索引を修復してから再開する。」

Japanese `unicode61` は形態素解析せず、連続日本語の部分一致/再現率に未測定の制限がある。文字列substring品質、rank方向、token grammar、pagination snapshotは未確定。クエリをFTS grammarとして無制限解釈せず、安全なliteral query builderを優先する。検索後にowner/class判定する設計は禁止。

**代替:** A) FTS5 + `unicode61` (ADR-0006の単純基線、検索品質は未保証)。B) trigram (indexが大きく短いquery edge caseあり)。C) host側tokenizerとversioned token stream (追加実装とcorpus/version移行)。Semantic/vectorは別ADR。

**所有者が回答する点:** tokenizer選択、query grammar、rank/tie-break、snapshot/paginationの安定性、Japanese corpus合格条件。

**互換性:** ADR-0006を保ち、FTS以外の意味検索を導入しない。tokenizer変更にはrebuild/version plan。

**必須検証:** 実際のRust `rusqlite` bundled SQLiteでFTS5 module/compile options、transaction rollback、content visibility、index rebuild/integrity、query boundsを確認。Japanese品質評価には代表的で合成か適切に匿名化されたコーパスとowner承認の関連度判定が必要。Python SQLite実験から品質数値を主張しない。

**実装ゲート:** lexical baselineはowner合意後P7Cでfake sourceを使って実装可能。Rust SQLite targetと日本語corpusを検証するまで検索品質production claimは不可。

**方向:** Owner approved 2026-10-12; detailed implementation parameters remain pending.

## P7-D07 — Forget と resurrection suppression

**Ratified direction; detailed contract text remains pending:** 「Forgetは認証済みOwnerの明示要求で開始し、対象itemと全てのtransitive derived item、Provenance、FTS rowを同一Owner namespace内で削除対象にする。共有Blobは `(digest,class)` ごとの実参照数が0のときだけGC可能。削除と同一transactionで内容を含まないtombstone/削除世代を記録する。backup restoreは削除記録を外部照合し、最新状態とのreconciliation完了までMemoryを検索・抽出・同期に公開しない。」

Tombstoneは本文、メール件名、平文digestを含まない。必要なsource-equivalence suppressionを選ぶ場合のみ、domain-separated HMAC (keyed, owner-scoped input) を用い、HMAC keyを別のOS保護領域に保存し、key version、rotation overlap、restore時の照合、失効を定義する。HMACはsource equality leak自体がある。key喪失/不整合はsnapshotを隔離し、Forget記録なし扱いにしない。長期tombstoneはprivacy metadataであり、保持期限・backupより長い期間・上限を明示決定する。再抽出は同一source extraction contractのtombstone照合前に拒否する。

通常GCはexpiryを消す処理。Forgetは派生範囲とresurrection suppressionを記録する処理。Physical secure eraseは別で、SQLite page reuse、WAL/SHM、SSD wear-leveling、snapshot/backupから物理消去を保証しない。

**代替:** A) Backup外のowner-controlled deletion ledger/tombstone (推奨、restore時networkなしでもcheck可能な複製が必要)。B) versioned HMAC tombstone (低metadata露出ではなく、same-source linkabilityを受け入れる)。C) Tombstoneなしはbackup restore後のresurrectionを防げず不許可。

**所有者が回答する点:** Forgetのtarget粒度、derived closure、tombstone lifetime/byte quota、ledgerの独立保管先、backup rollback時のminimum data, HMAC採用/rotation/key-loss、shared blob GC。

**互換性:** 新しいdeleted status/ledgerはadditive contract/migration。現行EventId再利用/ID grammarを変更しない。

**検証:** Task削除後Forget、transitive chain、shared Blob、replay extraction、restore-before-reconciliation fail-closed、ledger rollback/key loss、HMAC rotation、bounded GCをfake backupで確認する。

**実装ゲート:** tombstone lifetimeとrestore先行時のfail-closed reconciliationソースを確定するまでForget production実装・restore公開は不可。

**方向:** Owner approved 2026-10-12; detailed implementation parameters remain pending.

## Owner decision ledger (原則承認済み・詳細未決)

| ID | 承認済み原則 | 主要な未決定事項 | P7B blocker | 状態 |
| --- | --- | --- | --- | --- |
| P7-D01 | Single local Owner、host-verified OS auth、immutable admission binding、unprovable legacy quarantine | OS bootstrap/recovery details | **Yes** | Principle accepted; details pending |
| P7-D02 | Independent retention and minimized immutable provenance without blocking Task FK | field list, Forget scope | Yes | Principle accepted; details pending |
| P7-D03 | Additive owner-scoped constraints, verified unique parents, class-bound Blob, single successor | executable SQL/cycle proof | Yes | Principle accepted; proof pending |
| P7-D04 | PUBLIC/PERSONAL only; PRIVATE fails closed; SECRET/CREDENTIAL excluded | separate production protection for PRIVATE | **Yes for PRIVATE** | Principle accepted; gate remains |
| P7-D05 | Strict bounds and no unsafe eviction/Forget bypass | all numeric values, reserve/exhaustion/ledger bounds | **Yes** | Principle accepted; values pending |
| P7-D06 | SQLite FTS5 lexical; provisional unicode61; authoritative filters/bounded literal/stable results | target SQLite proof and Japanese quality | Runtime gate | Principle accepted; evidence pending |
| P7-D07 | Authenticated Forget/derived closure/content-free tombstone/shared-Blob-safe GC/independent reconciliation | reserve, durable quarantine, ledger/restore lifecycle | **Yes** | Principle accepted; mechanism pending |

**Ratification scope:** the owner approved these seven principles on 2026-10-12 as a package. This does not settle listed details or authorize implementation; a separate decision is required for remaining details and P7B.


## 2026-10-12 ratification overlay and mandatory exhaustion rule

The owner-approved scope and unresolved parameters are authoritative in [ADR-0042](../decisions/ADR-0042-p7-owner-ratified-directions.md) and the [ratification record](P7-owner-ratification-record.md). Candidate quotas and lifetimes remain PROPOSED; the prior A/B/C matrix remains unselected.

Forget must not silently leave targeted Memory retrievable when tombstone storage is full. Deletion capacity must be reserved independently of ordinary Memory growth, and new extraction/storage must stop before reserve exhaustion. If durable deletion proof cannot be committed, affected Memory must be quarantined from retrieval and model egress, the incomplete outcome truthfully reported, and recoverable evidence preserved. An in-memory flag alone is not crash-safe. Durable quarantine across restart and backup restoration, reserve sizing, exhaustion protocol and bounds remain separate technical decisions and proof obligations.

SQLite constraints/audit rows do not resist arbitrary local file modification. P7B research must state the OS filesystem protection assumption and same-user process residual risk. Production high-impact provider effects require separately reviewed integrity and rollback protection for Policy, Approval, Grant and Dispatch authority (or comparably defensible trusted authority); failures fail closed. No protection claim is made against a fully compromised OS.
