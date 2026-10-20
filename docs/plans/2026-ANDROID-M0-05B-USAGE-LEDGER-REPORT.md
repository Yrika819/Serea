# AND-M0-05B Model Usage & Cost Ledger

Date: 2026-10-20

Base: PR #50 head `94492d281fe0ee583744c932259c3525d5c34f86`

Branch: `codex/android-m0-05b-model-usage-ledger-2026-10-20`

## Scope and observed baseline

The existing `UsageAccounting.kt` already reserves estimated input and output tokens before dispatch, applies UTC daily/monthly request and token limits, settles provider-reported token counts, retains reservations when usage is missing, and marks interrupted requests `OUTCOME_UNKNOWN`. The existing `usage_reservations` primary key is the send ID. The v3 database has additive migrations 1→2 and 2→3.

The missing pieces were durable model/provider attribution, a request-time price record, persistent cost status, and model/provider read APIs. The existing `ModelChatRequest` carries `sendId` and `selectedModelId`, but no provider ID. Provider identity is therefore never parsed from a model ID.

## Ledger schema and state machine

Room v4 adds `model_usage_ledger`, with `requestId` as its SQLite primary key and foreign key to `usage_reservations`, plus day/model/provider and month/model/provider indexes. One row is created in the same `UsageDao` transaction as the pre-egress reservation and UTC window increments. A failed ledger insert rolls back the reservation, limit window, and last-observed clock update; the SQLite constraint also prevents orphan ledger rows.

The ledger stores send ID, model/provider attribution, request timestamp, UTC day/month keys, estimated input and reserved output tokens, nullable provider-reported input/output tokens, usage and cost statuses, estimated amount and currency, source and verification time, the immutable input/output price snapshot and revision, reservation state, settlement time, uncertainty reason, and model attribution status. It stores no conversation text, provider payload, response body, credentials, or headers.

State transitions are one-way:

| State | Meaning | Reservation handling |
| --- | --- | --- |
| `IN_FLIGHT` | Reserved; no response outcome recorded | Full reservation remains |
| `SETTLED` | Both provider token counts were received and settled once | Reservation window is adjusted to reported counts |
| `USAGE_UNKNOWN` | Response returned without complete usage | Full reservation remains; cost is unknown |
| `OUTCOME_UNKNOWN` | Timeout, cancellation, failure, or process death left the remote result uncertain | Full reservation remains; no later settlement is allowed without a new reconciliation contract |

Repeated settle and recovery calls are no-ops once the row leaves `IN_FLIGHT`. `OUTCOME_UNKNOWN` is not silently converted to `SETTLED`. Negative provider token counts are rejected and cannot reduce accounting.

## Model/provider attribution contract

`UsageAttributionResolver` is the compatibility extension point. The default `RequestModelAttribution` copies only an explicit `selectedModelId`; provider remains `UNKNOWN`, and price remains absent. It does not guess from ID syntax. IDs are length/character validated, and unknown values remain the literal `UNKNOWN`.

For AND-M0-05A integration, put `UsageControlledModelChatPort` downstream of `RoutedModelChatPort` and upstream of the provider. A resolver can then receive the selected ID and look up the exact entry in a trusted `ModelCatalog` to supply its provider ID and verified price evidence. The Mock HTTP instrumentation contract uses this order and verifies two model selections and their explicit fixture provider attribution. This does not claim that the production composition root has completed that integration.

## Cost estimation and price snapshots

`VerifiedPriceSnapshot` accepts decimal strings for input/output price per million tokens, ISO 4217 currency, source, verification timestamp, and optional effective time/revision. Negative prices, invalid currencies, absent source, and malformed timestamps are rejected. The snapshot is copied into the request ledger row and is never looked up again during settlement, so later catalog edits do not alter old calculations.

Calculations use `BigDecimal`; amounts are stored as decimal strings paired with a currency. Summary totals remain separated by currency. No production price is embedded in the app; tests label prices as fictional fixtures. During `IN_FLIGHT`, the ledger may store an `ESTIMATED` amount from the estimated input and reserved output tokens. After provider usage arrives, the row is recalculated as `USAGE_REPORTED_COST_ESTIMATED`, never as a final provider invoice amount. Missing/invalid price evidence or usage, and uncertain outcomes, produce `UNKNOWN`, not zero.

## Aggregation API

`UsageRepository.summaries()` returns summaries grouped by provider+model for UTC daily, UTC monthly, or all-time windows. `UsageAggregationDimension` explicitly selects overall, provider, model, or provider+model grouping. `overallUsage()`, `providerUsage()`, `modelUsage()`, `providerModelUsage()`, `dailySummary()`, `monthlySummary()`, and `unknownUsageSummary()` provide stable read helpers. Each result contains request count, estimated input tokens, reserved output tokens, provider-reported input/output, uncertain request count, estimated costs grouped by currency, and unknown-cost count. `UNKNOWN` attribution is retained as its own group.

The existing timestamp monotonicity rule remains: observed time is `max(now, lastObservedAt)`, so a wall-clock rollback does not reopen an earlier limit or summary window. Checked `Long` additions reject overflow before ledger/window writes; aggregate sums also fail rather than wrap.

## Migration and compatibility

The design adds a dedicated ledger table beside `usage_reservations`, rather than widening the existing reservation row. This keeps the legacy limit-accounting shape stable while a single Room transaction and one request-ID key guarantee the two rows move together. It also makes price and attribution data independently indexable without changing encrypted history rows.

Migration 3→4 is additive and has no destructive fallback. It creates the table/indexes and backfills one `UNKNOWN` model/provider row for each old reservation. Existing settled rows preserve provider-reported token counts; prior estimated/reserved counts and prices unavailable in v3 are not invented. Existing uncertain reservations keep their state and reservation amounts. Conversations, encrypted message ciphertext, settings, usage windows, and reservations are not deleted. Room schema export and full 1→latest, 2→latest, 3→latest migration tests are part of this change.

History AES-GCM/Keystore code and task-domain databases are untouched. The usage ledger follows the existing local Room storage protection boundary; it contains accounting metadata only and does not encrypt these non-content fields separately. Rooted/live-compromised devices remain outside the security guarantee.

## Soft Limits, Egress, and parallel work

Daily/monthly request and token limits remain global. `UNKNOWN` model/provider attribution still reserves against those same limits. Reservation and ledger insert happen before calling the delegate. The existing Egress wrapper must remain outside usage accounting for denied cloud requests so DENY does not create a reservation; the new work does not change Egress or enable a provider.

No `AdaptiveModelRouter.kt`, `CloudProvider.kt`, `SecurityPorts.kt`, `MainActivity.kt`, or C Tasks-domain source was changed. C's future `SereaTasksDatabase` remains independent.

## Test and CI evidence

The Android workflow's existing Unit selection used explicit class patterns. It now includes `UsageLedgerCostTest`; the API 26/35 instrumentation verifier still requires at least 34 total tests and additionally requires eleven `UsageLedgerDatabaseTest` cases. Existing required class thresholds are unchanged.

The new JVM cost tests cover separate input/output prices, zero/large token counts, missing usage/pricing, exact decimal arithmetic, invalid prices/currency/source, and immutable revisions. Android instrumentation tests cover model/provider summaries, duplicate/conflicting send IDs and settle, unknown attribution/usage, timeout/cancellation/network failure, reopen recovery, concurrent reservation, atomic rollback on forced ledger insert failure, Egress DENY with zero reservation, near-`Long.MAX_VALUE` overflow rejection, UTC day/month/leap/year boundaries, price revision history, successful 1/2/3→4 migration, and failure rollback without destructive fallback.

Execution status in this workspace:

| Check | Result |
| --- | --- |
| `git diff --check` | PASS |
| Android build / Unit / Integration / Security / Lint / Dependency Audit | NOT_RUN: Gradle distribution download was refused by the environment network connection |
| API 26 / API 35 instrumentation and process death | NOT_RUN locally; CI evidence pending final pushed HEAD |
| Fast CI / Full CI / Native / SQLite / Windows hooks / Commit Identity | NOT_RUN locally; CI evidence pending final pushed HEAD |

The execution environment has JDK 21, but its inherited `JAVA_HOME` points to absent JDK 17. After selecting JDK 21 for the Gradle process, the wrapper could not download Gradle 8.11.1 (`Connection refused`). No test is reported as passed on that basis. Final CI run IDs and outcomes must be recorded after the branch is pushed.

## Owner decisions and remaining integration work

1. AND-M0-05A must ensure `selectedModelId` is set before the Usage wrapper reserves. Place routing outside accounting, and inject Provider ID by exact trusted catalog lookup. Supply a verified price snapshot only from an approved catalog/source.
2. Production catalog evidence and production pricing are not included here; costs stay unknown until trusted evidence is injected.
3. Provider billing remains authoritative. These local estimates do not enforce a provider account spending cap.
4. Usage-ledger metadata is stored in the app-private Room database without separate field encryption, matching the existing accounting database boundary; it contains no conversation content or credentials.

## Safety statement

No live provider communication, API key, real account, real charge, user conversation export, or GitHub secret change was used for implementation. HTTP coverage uses MockWebServer loopback fixtures only. This report makes no claim that provider usage or charges were verified against a live service.
