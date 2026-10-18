# Android AND-04 owner decisions

These decisions block production cloud-AI calls. Phase 1 contains no API key,
OAuth token, account sign-in, HTTP client, or external service operation.

1. **Provider architecture:** choose direct mobile-to-provider or an
   authenticated Serea backend; name the provider/model and acceptable
   availability region.
2. **Data sent:** choose whether user-authored chat may be classified PERSONAL,
   what trusted minimization/redaction rules are required, and which fields or
   sources are always excluded. Confirm PRIVATE, SECRET, CREDENTIAL and
   unknown remain denied.
3. **Retention and deletion:** set provider retention/training terms, local
   history retention, encrypted storage requirements, account deletion and
   conversation deletion semantics, and crash/logging/analytics exclusions.
4. **Credential custody:** approve auth/account model, Android Keystore key
   requirements, hardware-backed fallback policy, revocation, device-loss
   recovery, backup exclusion, and reauthentication behavior.
5. **Cost and reliability:** choose per-user/day/month spending limits, model
   timeout, cancellation semantics, whether any retry is offered, and how
   indeterminate timeout results are presented.
6. **Distribution and disclosures:** choose personal APK, closed testing, or
   Play distribution; approve OAuth scopes and verification, privacy notice,
   data safety declarations, and security review gates.
7. **History owner:** confirm Android-only local ownership for 2026 and whether
   any export is needed. PC sync remains deferred and requires a separate ADR.

Calendar read-only scopes, reminders, local tasks, and conditional Gmail
read-only are separate feature decisions; no source permission grants model
egress. Calendar writes, email sending, PC capability grants, and P8 effects
remain out of scope.
