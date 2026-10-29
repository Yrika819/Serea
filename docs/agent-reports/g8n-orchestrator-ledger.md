# G8 Closure → Native AI → G9 → G10 Campaign Ledger

**Checkpoint:** 2026-10-29 UTC 02:40
**Roadmap:** `Serea_Roadmap_v4.md`
**Repository:** `Yrika819/Serea`
**Campaign status:** G8 Evidence Closure **PARTIAL**; Native Runtime **PARTIAL**; Bonsai 2 Evaluation **PARTIAL**; G9 Functional Candidate **PARTIAL**; G10 Preflight **PARTIAL**; Personal Beta **NO-GO**.

This checkpoint records exact-source evidence only. Existing work is preserved; PR #144 and #146 have not been modified. No OAuth, real Google data, Cloud AI credentials/billing, Owner signing key, physical Pixel 7a, protected-main merge, or distribution was used.

## GitHub authority and source identity

| Ref | GitHub state at campaign start |
|---|---|
| Protected `main` | `e0acb0254820c381dd97828824bef86a1db5176c`, tree `4955555be75c5742796740a3283d1a521e0d4806` |
| PR #134 | Known G5 HEAD `b84feb337cd57918346ab31cb3ea4af4ba880ffb` |
| PR #138 | Known G6 HEAD `23b884cf2815a2f3287652d5ffd2c3913d09df76` |
| PR #139 | Branch `codex/serea-g7-calendar-readiness`, current HEAD `0560e1e7bcd13f0194eab8caca89d70ea5a2f142` |
| PR #144 | OPEN/DRAFT, `codex/serea-g8-native-local-runtime`, HEAD `07ea5c6b460cea2eb1ccccc49ccd4f74dc65bb59`, tree `ff721fc7ae7642cbaf226376c23df6541d7c8248` |
| PR #146 | OPEN/DRAFT, `codex/serea-g8-integration-candidate-20261029-r1`, base branch `codex/serea-g7-calendar-readiness`, HEAD `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66`, tree `25e2bb481a0b25bfed89d6ee25f1812316703b8e` |
| Local G8 implementation reference | User-provided tree `eba68ebff6f1be05a1cc16c5043edc1c086f62cc`; not a GitHub commit object (tree ID, not commit SHA) |

Protected `main` matches the supplied authority. Existing campaign branch names were checked on the remote; the new `codex/serea-g8n-*` names were absent. The local base clone began clean on `main` at the protected SHA. The parent candidate is a new branch based on exact PR #146 HEAD; this is a stacked Draft candidate, not a rewrite of #146.

## Agent and worktree assignments

| Lane | Assignment | Worktree / branch | Base SHA | Ownership boundary |
|---|---|---|---|---|
| Parent | Ledger, integration authority, final review/CI | `/workspace/serea-integration` — `codex/serea-g8n-integration-candidate` | `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` | Shared composition, integration, release configuration, and this ledger |
| A — Native AI / Bonsai 2 | PR #144 failure analysis, isolated runtime, model and compatibility evidence | `/workspace/serea-agent-a` — `codex/serea-g8n-native-runtime` | `07ea5c6b460cea2eb1ccccc49ccd4f74dc65bb59` | Native/adapter-only; no default-model switch or shared composition edits |
| B — Android Product / Google / Gmail | Fake-only UI, Gmail safety, lifecycle and data minimization | `/workspace/Serea-gmail` — `codex/serea-g8n-google-gmail-ui` | `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66` | Gmail/Google feature and test files; no shared composition edits |
| C — CI / Security / Release | Formal CI evidence recovery, policy/release packet, independent reviews | Worktree/branch report pending | `0560e1e7bcd13f0194eab8caca89d70ea5a2f142` | Read-only evidence/reports; no product code |

These are three actual child agents. No result is accepted solely from its author's report; source, tests, provenance, licensing and integration safety remain subject to independent review.

## Phase 0 — G8 evidence recovery

All supplied PR #146 runs are attempt 1 and resolve to exact HEAD `d444e7cc04c8f1c4c6d12b1412cfe3a76258af66`.

| Workflow | Run | State at 02:40 UTC | Job / artifact evidence |
|---|---:|---|---|
| Android | `40804694025` | IN_PROGRESS | Change Scope, Security, Integration, Docs, Dependency Audit passed. Build/Lint/Instrumentation Compile and Android Unit were running on assigned `ubuntu-24.04` runners. Integration JUnit artifact `12869310588`, SHA-256 `62d700d4c25ac1ef0c9ddc3645586028a2347465c42370e16a3153ab108a0355`; Security JUnit `12868263908`, SHA-256 `6c757e8b54d382fc0e9734cd4fe6be6e892cd8d8408a96fac0dd9f949fe9ef12`; dependency audit `12868532793`, SHA-256 `32e9947f6dc9497a516ae626b14f2d33b8fee341c9e06d8a00e05ca04162ec01`. API 26/35 Required, Identity, Process Death, Data Isolation, Sentinel and their JUnit evidence were not yet confirmed. |
| Full CI | `40804693907` | IN_PROGRESS | Windows P7 Gate C, MSRV Linux and stable Linux were assigned/running. macOS ARM64 and Intel jobs were QUEUED with no runner assignment. No conclusion yet. |
| Native platform validation | `40804693934` | IN_PROGRESS | Linux ARM64/x86_64, stable macOS ARM64 and stable Windows x86_64 passed. Windows ARM64, stable macOS x86_64, MSRV Linux x86_64 and Windows x86_64 were running. MSRV macOS ARM64 was QUEUED without a runner. This workflow validates the Rust native matrix; it is not evidence for Android JNI inference. |
| SQLite portability | `40804693978` | IN_PROGRESS | macOS ARM64 DB production passed; artifact `12869265751`, SHA-256 `a24f408ba84461818e3f4bb915977ad699900c54ac5dc18416009e3d305be039`. macOS Intel DB production and ARM64→Intel consumption were QUEUED without runner assignment. |
| RustSec | `40804693889` | SUCCESS | Artifact `12867589033`, SHA-256 `0f360ef81a5ab03efc14fecf99197c758df0be54cd48cd058935caa11078bee7`. |
| Fast CI | `40804693992` | SUCCESS | Linux fast checks passed; no artifact. |
| Git Hooks | `40804693919` | SUCCESS | Windows, Ubuntu and macOS jobs all passed; no artifact. |
| Release Security | `40804722382` | SUCCESS | Mock release contract + APK-bound lockfile SBOM artifact `12868174398`, SHA-256 `17ad843799fdde28e42a9ea099d1c9d571e8f6d093b958a1ee17a464037c40a7`. CI-only debug APK and mock signing only. |
| Mock Beta Contract | `40804722157` | SUCCESS | One mock signing/Firebase contract job passed; no artifact. |

Pending jobs above had no assigned runner where stated; they are infrastructure queue delays, not product failures. No workflow was rerun at this checkpoint. There is no same-HEAD Android API 26/35 Required result, Identity result, Process Death/Data Isolation/Sentinel result or instrumented Gmail/Calendar UI result yet.

## PR #144 native failure analysis

Latest exact-head PR checks include Native focused run `40803623345`, attempt 1, on `07ea5c6b460cea2eb1ccccc49ccd4f74dc65bb59`. The job failed at `Build opt-in product JNI APKs for both supported ABIs` while invoking `:app:assembleIsolated :app:assembleIsolatedAndroidTest`; the build step returned exit 1. It expected pinned llama.cpp `276378515ceebee5d14bcc8536603dd5c87491a2`, model revision `1076b85d135017971845b19b8d07df1479155988`, SHA-256 `e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5`, size 101,016,128 bytes.

The workflow redirected Gradle output to `evidence/g8-local-native/gradle-native-build.log`, but no artifact was uploaded for the failed run. Therefore the observable category is a **native APK assembly/build failure**; JNI compiler, CMake, NDK/ABI, llama.cpp API, link, packaging and test-harness subcause is **UNCONFIRMED**. The output recovery path is a harness/evidence gap. Do not call this a proven JNI or CMake defect. Lane A is adding recoverable failure diagnostics and will use a new source SHA for any subsequent CI evidence.

Run `40803623345` is the G8 Native Runtime Focused non-required workflow, separate from Rust `40804693934`. No x86_64 inference evidence can be inferred from either run's name or build failure.

## Current implementation progress and gates

| Milestone | Status | Evidence / remaining gates |
|---|---|---|
| G8 Evidence Closure | PARTIAL | PR #146 exact source verified; 5 auxiliary runs already successful, Android/Full/Native/SQLite still in progress at checkpoint; required Android emulator identities and UI E2E not yet evidenced. |
| Native Runtime | PARTIAL | PR #144 adapter/build candidate exists but latest native APK build failed; log artifact is absent; new JNI inference, two-turn generation, lifecycle, cancellation, memory release and x86_64/ARM64 execution remain unproven. |
| Bonsai 2 Evaluation | PARTIAL | Prior source reports cover SmolLM2 metadata/license baseline only. Bonsai 2 2B/4B/9B formats, quantization, compatible backend, Android runtime, performance and Japanese/tool quality still require verified public-source and measured evidence. |
| G9 Functional Candidate | PARTIAL | Existing PR #146 Fake Calendar and consent foundation is under CI. Lane B has identified and is repairing the Gmail handoff metadata-minimization path; Android UI tests and lifecycle/process-death evidence remain incomplete. |
| G10 Preflight | PARTIAL | CI-only mock signing/SBOM and mock Firebase/signing contract passed on #146; native inventory, complete APK/SBOM provenance, migration and rollback plan, owner runbooks and Pixel acceptance are pending. |
| Personal Beta | NO-GO | Requires all P0 same-HEAD Required gates, Owner decisions, production credential/key/device approval and Pixel 7a acceptance. |

## Lane updates

### Lane A — Native AI / Bonsai 2

- Current branch starts at the exact existing PR #144 HEAD. Existing PR is unchanged.
- Latest source-bound native build run failed during isolated APK assembly; no recoverable Gradle log artifact exists. Root cause is unconfirmed.
- Local execution environment currently has Java 21 without `javac`, no `adb` device, and no `/dev/kvm`; native emulator inference is not available locally unless the runner/toolchain is improved.
- Native library/model download, compile, emulator inference, ARM64 runtime and Pixel testing remain NOT_RUN until direct evidence is produced.
- Next: repair failure artifact capture; inspect current JNI/CMake/build config and latest native logs; perform bounded source/metadata investigation of Bonsai 2 without committing weights or claiming unmeasured compatibility.

### Lane B — Android Product / Google / Gmail

- Branch starts at PR #146 exact source. Fake-only and no-OAuth boundary preserved.
- Found that selected Gmail handoff content included subject/from/date and snippet; patching toward body-only Chat handoff plus an exhaustive synthetic metadata-exclusion regression. Source revision binding is being updated to bind the full selected record without disclosing excluded metadata.
- Focused JVM execution has not started: available Java runtime has no compiler. A workspace-local JDK availability check is in progress.
- Next: complete focused source test, produce exact command and JUnit, and request independent security review. If no JDK/emulator becomes available, compilation/UI execution remains NOT_RUN.

### Lane C — CI / Security / Release

- Current work: recover PR #146 complete run/job/JUnit/artifact evidence; PR #144 failure classification; Owner-ready release/provider/model decisions; independent reviews of A/B.
- Early check confirms queued macOS jobs have empty `runner_name`; assigned runners for other workflows are actively executing. No retry is authorized by this evidence alone.
- Next: record every exact job/artifact result after the running workflows settle, review reports and security boundaries, then return any concrete issues to their lane owners.

## Owner decision ledger

| Decision / approval | State | Gate |
|---|---|---|
| Android Beta Package ID and Demo/Beta coexistence | OWNER DECISION REQUIRED | Before Android OAuth client identity or Beta installation/update contract |
| Signing certificate / production key custody | OWNER DECISION REQUIRED | Before real OAuth SHA binding or production signing |
| Google Cloud project, Calendar scope and OAuth disclosures | OWNER DECISION REQUIRED | Before any real OAuth activation |
| Gmail `gmail.readonly`, Restricted Scope and verification eligibility | OWNER DECISION REQUIRED | Before any live Gmail activation |
| Real Google account/data usage | OWNER APPROVAL REQUIRED | Not used; no access performed |
| Cloud provider age/region/terms/eligibility, credentials and billing | OWNER DECISION REQUIRED | Cloud remains disabled; no credentials used |
| Model distribution/license and product default model | OWNER DECISION REQUIRED | No model weights committed; no default switch |
| Pixel 7a physical acceptance | OWNER APPROVAL REQUIRED | No device operation performed |
| Protected `main` merge, production key/signing, Beta distribution | OWNER APPROVAL REQUIRED | Not performed |
| Personal Beta GO | OWNER DECISION REQUIRED | Remains NO-GO until every P0 gate and Owner decision is complete |

## Source-bound evidence rules

For every new result retain source SHA + Tree SHA, exact command/config, run + attempt + job, JUnit test IDs and totals, failures/errors/skips, runner allocation, artifact IDs/digests, APK hash, native dependency identity, model revision/hash, reviewer decision and source/report branch. Required manifests and thresholds cannot be relaxed. Compile success does not equal execution; queue does not equal pass; a failed build without its Gradle log does not identify its root cause. Evidence from another SHA remains historical.

## Resume point

1. Poll existing #146 runs without restarting them; update this ledger from their exact job, JUnit, identity, artifact and hash evidence.
2. Lane A repairs diagnostics and evaluates the exact PR #144 build failure on a new campaign SHA; independently review any change before using its CI.
3. Lane B finishes body-only Gmail handoff and explicit excluded-metadata tests; review session/account generation binding and stale selection behavior.
4. Lane C completes release/security decision packet and independent review of each lane.
5. Integrate only ACCEPTED parts into this successor branch, freeze one HEAD/tree, run focused tests and formal CI against that frozen source.
6. Proceed to G10 preflight only where dependencies are satisfied; keep runtime, real OAuth, physical-device and release-only gates NOT_RUN/OWNER-BLOCKED as applicable.

**This checkpoint does not promise unattended/background continuation.** New work must resume from this ledger, actual branch/PR HEADs and exact same-HEAD evidence.
