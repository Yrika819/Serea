# Personal Beta Lane 4: AI provider activation boundary

## State

Prepared for integration review. No real Cloud provider was enabled or called. The existing Cloud setting remains disabled by default, and this lane adds only a Fake-provider validation route.

## Review basis

Reviewed CloudSettings, ModelRoutingPolicy, the credential repository, usage ledger, egress policy/gate, existing Cloud adapter tests, current integration candidates and open PR stack (#82–#86), applicable ADRs, SECURITY.md, and docs/plans/Serea-high-privilege-security-gates.md. PR #86 is the branch base. The Security Gate and active dependency-verification policy remain unchanged.

## Implementation

Extended UsageLedgerDatabaseTest with an end-to-end fake route through the existing control path:

RoutedModelChatPort → EgressCheckedModelChatPort → UsageControlledModelChatPort → FakeModelProvider

The fixture verifies a cost reservation exists before an approved PUBLIC request reaches the Fake provider; usage and cost settle against durable ledger rows; PRIVATE prior context is denied before reservation/provider dispatch; a local budget limit rejects dispatch before the Fake provider; and cancellation after Fake dispatch leaves a durable OUTCOME_UNKNOWN reservation. The test also asserts default Cloud settings remain disabled.

## Security contract

- No real Cloud credentials, network destination, or provider are activated by this lane.
- Egress policy runs before usage reservation and provider dispatch.
- Only data already approved by the existing egress contract reaches the provider port.
- Usage reservation/settlement and uncertain outcomes remain durable. Existing adapter tests continue to cover revoked credentials, disabled settings, TLS/errors, redirect and retry denial.
- No Gmail-to-Cloud path is introduced; no Security Gate is relaxed.

## Validation

The new end-to-end route uses a Fake model provider and Room-backed usage ledger. Existing SecurityPortsTest, OllamaCloudAdapterTest and usage-ledger tests cover egress denial, private/sensitive context, credential invalidation, disabled Cloud, reservation failure, settlement, cancellation and unknown outcomes. Local checks passed: targeted testDebugUnitTest (54 tests) for SecurityPortsTest, OllamaCloudAdapterTest and UsageLedgerCostTest; assembleIsolatedAndroidTest; lintDebug. The added instrumentation was compiled but not run against a device.

## Owner decisions

Before any real provider activation, the Owner must approve provider terms, minimum age and eligibility, supported regions, prices and budget ceilings, retention/training/data-use terms, exact provider/model allowlist, credential custody and revocation, and the exact data classes and destinations permitted by egress. These questions remain open; no real credential or request is authorized by this PR.

## Integration

Keep the activation-boundary test independent of the Calendar/Gmail and task repository bridges. After owner decisions and independent security review, a later activation PR may bind an approved provider and credential source while preserving egress-before-reservation-before-dispatch ordering. Do not call this Beta Ready until final integration HEAD contains all reviewed work.
