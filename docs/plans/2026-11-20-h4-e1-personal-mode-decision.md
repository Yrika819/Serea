# H4-E1 — Google Personal mode decision

**Decision date:** 2026-11-20
**Status:** Owner decision recorded; implementation and Cloud activation remain unauthorized.

## Owner decision

The Owner formally selects **Google Auth Platform Personal audience** for the first Serea Personal Beta Calendar connection. The application is for the Owner’s own use, and only the single Google Account that creates the Personal application is eligible to authorize it. This is not approval to create or modify a Cloud project, OAuth client, API configuration, or to authenticate or make API requests.

Initial scope is Google Calendar API through an Android OAuth client, with `https://www.googleapis.com/auth/calendar.events.readonly` and the `primary` calendar only. Gmail, Drive, Workspace MCP, Cloud AI, public distribution, and multi-user use are out of scope. Google-origin event data must not be sent automatically to Cloud AI.

For any future multi-user product, use a **separate Google Cloud project and a separately reviewed OAuth design**. Do not expand this Personal project or treat an Owner/Editor role as granting another Google Account access.

## Consequences and constraints

Google documents Personal as a one-time audience choice. Google says Personal cannot be changed to External or Internal, and existing External/Internal projects cannot be changed to Personal. Therefore, prefer a new Personal project; first inspect an existing project’s actual audience in the console only after separate Owner authorization. Never assume an existing External project can be converted. If the existing project is External/Internal, stop and submit a new-project decision; do not mutate it.

Personal mode permits only the Google Account that created the application. A different account is expected to fail with `client_user_mismatch`, even if it is a project Owner or Editor. H4-E1 will cover this only with mock tests, never by attempting real authorization from another account.

Personal is not subject to OAuth app verification, OAuth user-cap, unverified-app-screen, or External/Testing seven-day test-user token-expiry rules. This does **not** promise permanent tokens. Expiration, revocation, loss of grant, account change, and reauthorization remain ordinary fail-closed cases.

If a project’s audience cannot be conclusively identified, do not create a client or enable APIs. Preserve evidence and ask the Owner to decide whether to abandon it and request a new Personal project.

## AI-powered-agent client option

Google’s Android client guidance describes “Use this client for an AI-powered agent” as a declaration for agentic workflows and states that an application with both standard and agent workflows **must create separate clients**; some Google MCP endpoints require such a client. Serea’s present initial human-driven Android Play Services authorization and Calendar REST read are the standard-client path, not an agent/MCP workflow. Use a standard client and leave this option unchecked. This is a design conclusion from the cited Google guidance, not a claim that every future Google product has been exhaustively checked.

## Authority and references

- **Decision provenance:** the user’s explicit H4-E1 instruction in this chat dated 2026-11-20. It records the Owner decision without reproducing any private account, project, client, certificate, or other account-identifying information.
- Google, [Manage app audience](https://support.google.com/cloud/answer/15549945?hl=en) (Personal eligibility, audience immutability, and Personal exceptions).
- Google, [Manage OAuth clients](https://support.google.com/cloud/answer/15549257?hl=en) (Android client inputs and AI-agent client declaration).
- Google, [OAuth 2.0 policies and best practices](https://developers.google.com/identity/protocols/oauth2/resources/best-practices).
- Related readiness evidence and caveats: [H4-E1 readiness audit](../security/h4-e1-google-personal-readiness-audit.md); execution steps: [Owner runbook](h4-e1-personal-cloud-owner-runbook.md).

## Authorization boundary

No Google Cloud Console action, project/client creation, API enablement, account authentication, HTTP request, signing-key operation, APK signing/distribution, or Live Gate activation is authorized by this decision. Those actions require later explicit approvals described in the [H4-E1→H4-F gate matrix](h4-e1-to-h4-f-gate-matrix.md).
