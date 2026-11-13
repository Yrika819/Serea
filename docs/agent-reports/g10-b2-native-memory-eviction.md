# G10-B2 Native Memory Eviction

## Scope

This change adds an opt-in Android memory callback bridge and temporary Native eviction to the existing local-model adapter. It does not connect the Native backend to product Chat composition. The bridge is owned by an app-scoped lifecycle owner and must be registered and closed there.

## Lifecycle behavior

- `TRIM_MEMORY_UI_HIDDEN` records visibility only. It does not cancel an active generation or request eviction by itself.
- `TRIM_MEMORY_BACKGROUND` and an explicit owner request enqueue nonblocking eviction work. Duplicate callbacks coalesce by ticket; a stale completion cannot clear a newer ticket.
- An owner may allow an explicitly continued background operation. Eviction waits behind the adapter mutex, so a running generation completes and settles before release.
- `RUNNING_LOW` and other legacy trim values are not used as a lifecycle policy. Process death is recovered through process-local state reconstruction; old consent proofs are not persisted.

## Load, release, and reload

Before a load, the adapter verifies model size and SHA-256 and checks `/proc/meminfo` `MemAvailable` against the configured resident-memory budget plus a 128 MiB OS reserve. This is a conservative preflight observation, not a hard runtime cap.

Eviction marks the adapter unavailable, advances the local consent lifecycle generation, then waits for in-flight adapter work. The Native backend's close is asynchronous, so eviction waits up to 30 seconds for the existing JNI release result. Reload is possible only after JNI confirms no Native ownership and `NativeReleaseCoordinator` confirms `RELEASED`. Any timeout or failure leaves the adapter fail-closed. A later load creates a new backend and re-verifies model size and hash.

## Consent and dispatch boundary

Consent proofs carry their process-local lifecycle generation. Eviction invalidates pending proofs and increments the generation. `consumeOnce` checks that generation before and after suspending conversation/handoff validation. The adapter checks it before model loading and atomically claims Native dispatch immediately before generation. The same gate serializes dispatch with eviction: if eviction wins, generation is refused pre-dispatch; if dispatch wins, it may finish and eviction waits for it.

Only `LocalModelAuthorizationRegistry` is invalidated. Cloud and Demo egress authorization paths are not modified.

## Usage

Usage reservation and settlement continue through the existing wrappers. A memory check or eviction rejection before Native generation remains a pre-dispatch failure and releases its reservation. Once Native generation starts, an unconfirmed result retains the existing `OUTCOME_UNKNOWN` handling. Eviction does not convert an unknown result into success.

## Native model test

The isolated GGUF workflow runs the existing fixed-revision, fixed-SHA fixture through adapter generation, eviction, stale-proof rejection, fresh-backend reload and generation, and confirmed second release. The fixture remains ephemeral to the isolated x86_64 API 35 runner and is removed by the workflow cleanup trap. No weights are committed or uploaded.
