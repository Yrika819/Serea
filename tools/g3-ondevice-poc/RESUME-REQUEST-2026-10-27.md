# G3 diagnostic resume request

This checkpoint requests exactly one fresh diagnostic run for the current recovery source on PR #127. It exists to trigger the marker-gated, non-required G3 workflow; it does not alter the inference implementation, product code, or any CI gate.

- Stale run: `40533524841`, attempt 1, source `f96e77a798575c971313599ded8b267a353d5b46`; queued with zero jobs and zero artifacts.
- Before this checkpoint, no same-branch G3 run had started a job. The earlier API 35 attempt `40515091197` belongs to the parent source and is not reusable for the recovery-head AndroidTest.
- PR #127 remains the owner branch. The new run is expected to build the recovery-head APKs, download and verify the pinned public model once, then run its exact diagnostic test. No APK or model is persisted by this checkpoint.
- This is emulator feasibility only. It does not enable a real provider, integrate AI into normal Chat, or claim Pixel 7a performance.

Only one marker commit is intended. Do not create another trigger if the stale run is replaced or a new run appears.
