# Security Findings

## Firebase distribution dependency

`tools/firebase-distribution/package.json` pins `firebase-tools` 15.34.1. Its lockfile currently resolves nested `glob/node_modules/minimatch` 9.0.9, root minimatch 3.1.5, readdir-glob minimatch 5.1.9, and `superstatic/node_modules/minimatch` 6.2.3. The `glob` 10 dependency accepts `^9.0.4`, explaining why the selected 9.x remains capped below the reported fixed 11.0.1. The exact advisory and current registry graph were not verified in this pass; do not claim the lockfile is secure or force an override. Next: fetch advisory metadata, inspect `npm explain minimatch`, check firebase-tools/superstatic compatible releases, then update package and lock together and run `npm ci`, `npm audit --omit=dev`, and distribution tests. No dependency mutation made.

## Other boundaries

- OAuth credentials, signing keys and Firebase distribution remain owner-controlled and NOT_RUN.
- Demo auth/fake providers do not establish secure real-token storage or revocation behavior.
- Review Android Keystore, classification, redacted logs, egress/usage ledger, SBOM and pinned GitHub Actions before RC.
- Do not weaken audit or add unverified overrides.
