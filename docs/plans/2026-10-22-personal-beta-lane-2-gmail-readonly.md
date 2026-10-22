# Personal Beta Lane 2: Gmail read-only foundation

## State

Prepared for integration review. No live Google account or Google API was used. Gmail remains unwired from production composition/UI and has no default authorization activation. This foundation is not Beta readiness.

## Review basis

Reviewed the Gmail domain, Fake provider, MIME parser and UI, shared Google authorization boundary, current integration candidates and open PR stack (#82–#86), applicable ADRs, SECURITY.md, and docs/plans/Serea-high-privilege-security-gates.md. PR #86 is the branch base. Existing policy leaves real provider access behind separate owner and security gates.

## Implementation

Added a GmailReadPort HTTP implementation with only these GET operations:

- Inbox and query search list requests, including page tokens.
- Individual full-message retrieval.
- Full-thread retrieval.

The adapter checks the gmail.readonly permission before requesting a token, validates message/thread IDs, enforces page/query/token/message/thread/aggregate response bounds, parses responses through the existing MIME parser, and maps HTTP, timeout, cancellation and malformed-response failures to typed errors. Redirect following and automatic retries are disabled. Only the exact Gmail read scope can be requested. GoogleGmailAuthorizationAdapter defaults real HTTP authorization off, rejects Mock-only gateways, and rejects token leases that do not contain gmail.readonly.

## Security contract

- No write, send, draft, modify, delete, watch, history, attachment-download or mutation endpoint exists in this adapter.
- gmail.readonly is the only scope requested. Real authorization is off unless an owner activation value is explicitly injected.
- No Gmail message is stored by the transport or forwarded to Cloud AI. The adapter is not wired to a cloud egress path.
- Credentials remain in short-lived leases. Error messages do not include provider bodies.
- No real account, credential or live API call was used.

## Validation

GmailHttpReadPortTest uses Fake authorization/token fixtures and MockWebServer for inbox/search pagination, message/thread reads, scope enforcement, Mock-only rejection, disabled-by-default behavior, token invalidation, 401/403/404/429/5xx mapping, malformed and oversized responses, network failure, timeout and cancellation. GmailSecurityPortsTest covers Fake-provider contracts, read-only API shape and MIME parsing; GmailViewModelTest covers the existing UI model.

Local checks passed: targeted testDebugUnitTest (55 tests) for GmailHttpReadPortTest, GmailSecurityPortsTest and GmailViewModelTest; lintDebug.

## Owner decisions

Before any real OAuth access, the Owner must decide whether to request Google's restricted gmail.readonly scope; complete OAuth app verification and any required security assessment; approve publication/testing mode, consent disclosures, privacy policy and limited-use commitments; approve age/region eligibility and data retention; and explicitly authorize real-account testing. These items are not approved by this PR.

## Integration

After the Google authorization foundation is integrated, land this transport boundary before a separate account-selection/consent and Gmail UI integration. The UI must preserve local Fake behavior, display scope/account state, and keep Gmail content out of Cloud AI unless a separate, exact, user-approved egress design is reviewed. Do not call this Beta Ready until final integration HEAD contains all reviewed work.
