# Owner Decision Packet

1. **Chat beta identity:** recommend label as Demo Beta until a licensed inference runtime is approved and verified. Alternative: defer beta. Decision affects product claims and G10 runtime scope.
2. **Inference:** choose on-device model vs external provider only after license/data-flow review. External egress requires explicit provider and data policy approval; no credentials requested or used here.
3. **Calendar:** approve or reject Android OAuth client and minimal read-only scopes, account UX, retention/deletion policy. No write/delete scope proposed.
4. **Gmail:** recommend disabled for year-end absent separate Restricted Scope and policy approval. Decision gates API work.
5. **Overlay:** confirm whether home-screen overlay is a hard acceptance requirement; recommend defer pending user-visible permission flow and accessibility review.
6. **Release:** authorize protected signing environment, owner key handling, certificate fingerprint, Firebase/OIDC and rollback only after RC review.
7. **Device testing:** designate a clean test device/profile with no personal data; personal Pixel 7a remains untouched.
