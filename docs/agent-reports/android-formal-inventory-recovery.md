# Android Formal inventory recovery

## Base and scope

Branch `cloud/serea-v13-android-formal-inventory` starts at C-04R HEAD `8fef15004e8089ab2a7d99bbfbfdc837fb8b5da3`. It repairs exact instrumentation inventory accounting only.

## Correction

`UsageLedgerDatabaseTest` contains 14 methods: 12 in Original Required Set and two additional regression cases. `providerInputAboveReservationEstimateSettlesAndPricesReportedUsage`, added by C-03 commit `7f5e165`, was missing from the additional manifest. The class inventory check consequently expected 12 + 1 = 13 while finding 14 methods.

The additional manifest now contains three identities; the total is 103 Original Required + 3 Additional = 106. The original required set remains 103. The UsageLedger class baseline remains 12 and the verifier derives its three additional methods from the manifest. Test identities, signed artifact provenance, APK certificate checks, and required gates are retained unchanged.

## Verification and CI

The C-06 Formal run `45976772516` showed API26/API35 inventory failure at the 13-versus-14 source check; it did not report a provider product-test failure. Run `45976772516` is historical and is not reused as evidence for this change.

Targeted local verifier/manifest tests are recorded with this branch's CI result. A new Android workflow is expected from the Draft PR and must provide same-run Build provenance and API26/API35 Formal results. Until that run completes, Android Formal remains NOT_VERIFIED. Any dependency-resolution error must be reported separately from product-test outcomes.
