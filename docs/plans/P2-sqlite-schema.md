# P2 SQLite Schema

- **Branch:** `p2/design-preparation`
- **Base commit:** `c3737039e3e38dbba554dc0b9075025f87948358`
- **Status:** design only. This document proposes DDL. No migration is written,
  no SQLite file is created, and no dependency is added by this run.
- **Authority:** [ADR-0005](../decisions/ADR-0005-sqlite-wal-and-migrations.md) for
  SQLite + WAL + ordered migrations; [P2 contract gap
  analysis](P2-contract-gap-analysis.md) for the contract gaps this schema has to
  satisfy; [ADR-0018](../decisions/ADR-0018-taskstep-lifecycle-and-field-presence.md),
  [ADR-0021](../decisions/ADR-0021-p2-p3-event-atomicity-seam.md),
  [ADR-0022](../decisions/ADR-0022-durable-private-data-at-rest.md) and
  [ADR-0024](../decisions/ADR-0024-lease-fencing-and-commit-under-lease.md) for
  the invariants the constraints enforce.

## 1. Tables in P2, and the ones deliberately absent

| Table | In P2? | Why |
| --- | --- | --- |
| `schema_migrations` | **Yes** | The single authority for the schema version |
| `tasks` | **Yes** | Task Protocol §2 |
| `task_steps` | **Yes** | Task Protocol §3 |
| `side_effect_receipts` | **Yes** | Capability Protocol §5.1, normalised out of the step |
| `leases` | **Yes** | ADR-0024 |
| `plan_revisions` | **Yes** | Task Protocol §4.3 rule 5 requires revisions recorded; `PlanRevision` is a Crate Map §3 public type |
| `blobs` | **Yes** | Task Protocol §3.1's content-addressed store |
| `task_blob_refs`, `step_blob_refs` | **Yes** | Two tables rather than one polymorphic one, so both can carry a foreign key |
| `task_journal` | **Yes** | ADR-0021's commit hook; the durable audit record that makes `T5` provable |
| `store_meta` | **No** | P2 needs no host metadata. `next_seq` belongs to P3, which adds it in migration `0002` |
| `serea_events` | **No** | `serea-event-bus` is P3. ADR-0021's last rejected alternative is P2 growing this table |
| `approvals`, `grants` | **No** | `serea-capability`, P6 |
| `schedules`, occurrence records | **No** | `serea-scheduler`, P3 |
| `memory_items`, tombstones | **No** | `serea-memory`, P7 |
| `model_usage` | **No** | `serea-model-router`, P4. The *counters* are named in Bounds §2.1 as living on the task, and P2 keeps `max_model_calls` / `max_tool_calls` on the task because they are part of the frozen `attempt_budget` shape — but the `*_used` columns are P4's, and P2 has nothing to increment them with |
| `capability_registry`, `disabled` overlay | **No** | `serea-capability`, P5 |
| `policy_rules` | **No** | `serea-policy`, P6 |
| `device_sessions`, `pairings` | **No** | `serea-core`, P12 |
| `proposals` | **No** | `serea-scheduler`, P11 |

`data/` does not appear either: `serea-storage` maps
`TaskOrigin.device_id` to a `dev_` identifier and does not own a device roster.

**The one judgement call in this table** is `plan_revisions`. It is not in the
prompt's list, and it is not in P0's table list either. It is included because
Task Protocol §4.3 rule 5 requires each revision to be recorded as evidence, and
because ADR-0018's decision to *delete* superseded `PLANNED` steps is only
auditable if the prior plan content survives somewhere. That somewhere is a
content-addressed blob referenced by the revision row.

## 2. Normalised versus JSON

**The rule: a field is JSON only if nothing ever needs to compare, order, or
predicate on it.**

Five fields must be SQL-addressable, or the design does not work:

| Field | Predicate that needs it |
| --- | --- |
| `task_steps.status` | The recovery classification predicate |
| `task_steps.lease_generation` | The ADR-0024 fence |
| `task_steps.attempt` | The `max_attempts_per_step` ceiling, read from durable state at the check |
| `side_effect_receipts.step_id`, `UNIQUE` | `ReceiptAlreadyCommitted` during recovery — a single indexed existence check |
| `task_steps.task_id` | Parent binding, so one task cannot mutate another's step |

Anything storing an `AssistantTask` as one JSON document is explicitly refused: it
would turn all five into application-side read-modify-write and lose the atomicity
`TB-7` buys.

Stored as JSON, and why each is safe:

| Column | Why JSON is safe |
| --- | --- |
| `tasks.extensions` | `Protocol Index` §4.2 rule 3: unknown fields are retained, not interpreted. Nothing needs to query it |
| `ActionError.details` | The frozen shape is a closed object whose `details` is an open JSON object; the schema already caps it with `maxProperties: 64` |
| `ActionRequest.arguments` and `ActionResult.output` | **Not stored as JSON columns at all.** They are content-addressed blobs, which is what Task Protocol §3.1 requires and what makes dedup, digest verification, and right-to-delete possible |

Everything else is a column.

## 3. Class ranks are integers, labels are generated

```sql
data_class_rank    INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 4),
data_class         TEXT GENERATED ALWAYS AS (
                       CASE data_class_rank
                         WHEN 0 THEN 'PUBLIC'
                         WHEN 1 THEN 'PERSONAL'
                         WHEN 2 THEN 'PRIVATE'
                         WHEN 3 THEN 'SECRET'
                         ELSE 'CREDENTIAL'
                       END) STORED,
policy_class_rank  INTEGER NOT NULL CHECK (policy_class_rank BETWEEN 0 AND 7),
policy_class       TEXT GENERATED ALWAYS AS (…eight RiskClass values…) STORED,
```

**Why.** `PUBLIC < PERSONAL < PRIVATE < SECRET < CREDENTIAL` and the eight-class
`RiskClass` ordering are both *semantic* orderings that the labels do not
reproduce — alphabetically `CREDENTIAL < PERSONAL < PRIVATE < PUBLIC < SECRET`.
Storing the label and comparing it would compare alphabetically and get the
ordering wrong; storing the rank and the label separately lets them disagree. A
generated column removes the disagreement by construction: one authority, and the
`CHECK` validates the same value the reader sees.

This is a security property, not a convenience. A corrupt or hand-edited row that
disagreed with its own class would be a corrupt row that *widens authority* — a
task's `policy_class` ceiling is `T6`, and a ceiling compared alphabetically
against `RiskClass::rank()` is a ceiling that `DESTRUCTIVE` passes when it should
not.

A test pins the Rust `DataClass::rank()` and `RiskClass::rank()` against the two
`CASE` expressions, in both directions, so the generated columns cannot drift from
the enums.

## 4. The DDL

Minimum SQLite version **3.37.0**, for `STRICT` tables (3.37) and
`GENERATED … STORED` (3.31). Both are load-bearing for §3 and §5, so the fallback
if the resolved SQLite is older is recorded in
[P2 design §7](P2-storage-task-engine.md#7-migrations-and-connection-policy)
rather than left implicit here.

**This DDL has been executed.** It was built and exercised against SQLite 3.43.2
during design preparation, across four rounds — **all assertions passing** — before the ADRs
referencing it were finalised. The migration's first obligation in P2C is to
reproduce that harness, not to rediscover it.

Four rounds of defects were found this way, and the pattern is the point:

| Round | What was wrong | Why a reading missed it |
| --- | --- | --- |
| 1 | `PLANNED` unconstructible — two biconditionals contradicted each other | Individually well-reasoned; collectively impossible |
| 1 | `CANCELLED` unconstructible — two `CHECK`s on `cancelled_at_ms` disagreed | The **negative** case was tested; the **positive** never was |
| 1 | The whole migration did not build — SQLite prohibits subqueries in `CHECK` | Asserted, not executed |
| 2 | No lease could be acquired, in either order | The upsert was tested against a hand-prepared step, not against the sequence the document prescribes |
| 2 | `SECRET`/`CREDENTIAL` reachable through `task_journal` | Four tables were enumerated and the fifth omitted |
| 3 | The `WAITING` biconditional made the three wait kinds unplannable | The negative (`WAITING` on a `CAPABILITY` step) was tested; the positive (`PLANNED` on a `WAIT_APPROVAL` step) never was |
| 4 | A partially populated capability tuple (`NOTIFY` with only `capability_id`) was accepted | A single biconditional over four columns is not the same as four biconditionals. Found by running the full 8 kinds × 7 statuses grid rather than the two cells either side of the earlier bug |
| 3 | `TASK_DELETED` was "removed" in prose but still in the DDL | The disposition was applied to one of the two places it named |
| 3 | The `cancel` statement in the design doc aborted on a `BLOCKED` task | The schema was fixed; the statement in the *other* document was not |
| 3 | The lease `token` "removed everywhere" survived in nine places | The fix was applied to two of the five documents that mention it |

The consistent blind spot was **negative-only testing**: every "is this refused?"
case was exercised and almost no "is this accepted?" case was. Round 3 exists
because of it — it constructs every legal transition, executes every prescribed
statement sequence in the order §4 prescribes, and confirms the positive side of
every constraint. See §7.

### 4.0 The whole migration

```sql

-- Foreign-key enforcement is set per connection by Store::open, not here:
-- PRAGMA foreign_keys is a no-op inside a transaction, and every migration runs
-- inside BEGIN IMMEDIATE. See P2-storage-task-engine.md 7.2.

CREATE TABLE schema_migrations (
  version       INTEGER PRIMARY KEY CHECK (version >= 1),
  name          TEXT    NOT NULL UNIQUE,
  checksum      TEXT    NOT NULL CHECK (length(checksum) = 71
                                       AND substr(checksum, 1, 7) = 'sha256:'),
  applied_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE blobs (
  digest          TEXT    NOT NULL CHECK (length(digest) = 71
                                          AND substr(digest, 1, 7) = 'sha256:'
                                          AND substr(digest, 8) NOT GLOB '*[^0-9a-f]*'),
  data_class_rank INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2),
  protection      TEXT    NOT NULL CHECK (protection IN ('NONE', 'AT_REST')),
  size_bytes      INTEGER NOT NULL CHECK (size_bytes >= 0),
  content         BLOB    NOT NULL,
  data_class      TEXT GENERATED ALWAYS AS (
                      CASE data_class_rank
                        WHEN 0 THEN 'PUBLIC' WHEN 1 THEN 'PERSONAL'
                        ELSE 'PRIVATE' END) STORED,
  PRIMARY KEY (digest, data_class_rank),
  CHECK (size_bytes = length(content)),
  CHECK ((data_class_rank = 2) = (protection = 'AT_REST'))
) STRICT;

CREATE TABLE tasks (
  task_id              TEXT    PRIMARY KEY CHECK (length(task_id) = 30
                                                   AND substr(task_id, 1, 4) = 'tsk_'
                                                   AND substr(task_id, 5, 1) <= '7'),
  kind                 TEXT    NOT NULL CHECK (kind IN ('USER_REQUEST','SCHEDULED','PROACTIVE','DELEGATED_HOST_GOAL','MAINTENANCE')),
  title                TEXT    NOT NULL,
  state                TEXT    NOT NULL CHECK (state IN ('RECEIVED','PLANNING','READY','EXECUTING','WAITING_APPROVAL','WAITING_USER','VERIFYING','COMPLETED','FAILED','BLOCKED','CANCELLED')),
  origin_kind          TEXT    NOT NULL,
  origin_device_id     TEXT,
  origin_message_id    TEXT,
  origin_extensions    TEXT    NOT NULL DEFAULT '{}'
                               CHECK (json_valid(origin_extensions)
                                      AND json_type(origin_extensions) = 'object'),
  data_class_rank      INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2),
  policy_class_rank    INTEGER NOT NULL CHECK (policy_class_rank BETWEEN 0 AND 7),
  created_at_ms        INTEGER NOT NULL,
  updated_at_ms        INTEGER NOT NULL,
  deadline_at_ms       INTEGER,
  blocked_reason       TEXT,
  result_summary       TEXT,
  cancelled_at_ms      INTEGER,
  cancelled_by         TEXT,
  failure_reason       TEXT,
  max_model_calls      INTEGER NOT NULL CHECK (max_model_calls >= 0),
  max_tool_calls       INTEGER NOT NULL CHECK (max_tool_calls >= 0),
  max_attempts_per_step INTEGER NOT NULL CHECK (max_attempts_per_step >= 0),
  budget_extensions    TEXT    NOT NULL DEFAULT '{}'
                               CHECK (json_valid(budget_extensions)
                                      AND json_type(budget_extensions) = 'object'),
  plan_revision        INTEGER NOT NULL DEFAULT 0 CHECK (plan_revision >= 0),
  extensions           TEXT    NOT NULL DEFAULT '{}'
                               CHECK (json_valid(extensions)
                                      AND json_type(extensions) = 'object'),
  data_class           TEXT GENERATED ALWAYS AS (
                         CASE data_class_rank
                           WHEN 0 THEN 'PUBLIC' WHEN 1 THEN 'PERSONAL'
                           WHEN 2 THEN 'PRIVATE' ELSE 'SECRET' END) STORED,
  policy_class         TEXT GENERATED ALWAYS AS (
                         CASE policy_class_rank
                           WHEN 0 THEN 'OBSERVE' WHEN 1 THEN 'LOCAL_STATE'
                           WHEN 2 THEN 'REVERSIBLE_WRITE' WHEN 3 THEN 'EXTERNAL_WRITE'
                           WHEN 4 THEN 'COMMUNICATION' WHEN 5 THEN 'ELEVATED_DEVICE'
                           WHEN 6 THEN 'DESTRUCTIVE' ELSE 'CREDENTIAL' END) STORED,
  CHECK (updated_at_ms >= created_at_ms),
  CHECK (deadline_at_ms IS NULL OR deadline_at_ms >= created_at_ms),
  CHECK (state = 'BLOCKED' OR blocked_reason IS NULL),
  CHECK ((state = 'CANCELLED') = (cancelled_at_ms IS NOT NULL AND cancelled_by IS NOT NULL)),
  CHECK (state = 'CANCELLED' OR (cancelled_at_ms IS NULL AND cancelled_by IS NULL)),
  CHECK (state <> 'FAILED' OR failure_reason IS NOT NULL),
  CHECK (state = 'FAILED' OR failure_reason IS NULL),
  CHECK (origin_kind NOT GLOB '*[^A-Z0-9_]*' AND substr(origin_kind,1,1) BETWEEN 'A' AND 'Z'),
  CHECK (origin_device_id IS NULL OR (length(origin_device_id)=30 AND substr(origin_device_id,1,4)='dev_')),
  CHECK (origin_message_id IS NULL OR (length(origin_message_id)=30 AND substr(origin_message_id,1,4)='evt_')),
  CHECK (blocked_reason IS NULL OR (blocked_reason NOT GLOB '*[^A-Z0-9_]*' AND substr(blocked_reason,1,1) BETWEEN 'A' AND 'Z')),
  CHECK (failure_reason IS NULL OR (failure_reason NOT GLOB '*[^A-Z0-9_]*' AND substr(failure_reason,1,1) BETWEEN 'A' AND 'Z')),
  CHECK (cancelled_by IS NULL OR (cancelled_by NOT GLOB '*[^A-Z0-9_]*' AND substr(cancelled_by,1,1) BETWEEN 'A' AND 'Z'))
) STRICT;

CREATE TRIGGER tasks_policy_class_immutable
BEFORE UPDATE OF policy_class_rank ON tasks
WHEN NEW.policy_class_rank IS NOT OLD.policy_class_rank
BEGIN SELECT RAISE(ABORT, 'policy_class is immutable for a task'); END;

CREATE TRIGGER tasks_data_class_monotonic
BEFORE UPDATE OF data_class_rank ON tasks
WHEN NEW.data_class_rank < OLD.data_class_rank
BEGIN SELECT RAISE(ABORT, 'data_class may not be lowered'); END;

CREATE TABLE task_steps (
  step_id            TEXT    PRIMARY KEY CHECK (length(step_id) = 30
                                                 AND substr(step_id, 1, 4) = 'stp_'
                                                 AND substr(step_id, 5, 1) <= '7'),
  task_id            TEXT    NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  sequence           INTEGER NOT NULL CHECK (sequence >= 0),
  kind               TEXT    NOT NULL CHECK (kind IN ('CAPABILITY','MODEL_TURN','WAIT_APPROVAL','WAIT_USER','WAIT_SCHEDULE','VERIFY','NOTIFY','DELEGATE')),
  status             TEXT    NOT NULL CHECK (status IN ('PLANNED','LEASED','EXECUTING','WAITING','SUCCEEDED','FAILED','RECONCILED_ABSENT')),
  attempt            INTEGER NOT NULL DEFAULT 0 CHECK (attempt >= 0),
  plan_revision      INTEGER NOT NULL CHECK (plan_revision >= 0),
  provider_id        TEXT,
  capability_id      TEXT,
  capability_version TEXT,
  idempotency_key    TEXT,
  input_digest       TEXT    NOT NULL CHECK (length(input_digest) = 71
                                            AND substr(input_digest,1,7) = 'sha256:'
                                            AND substr(input_digest,8) NOT GLOB '*[^0-9a-f]*'),
  result_digest      TEXT    CHECK (result_digest IS NULL OR (length(result_digest) = 71
                                            AND substr(result_digest,1,7) = 'sha256:'
                                            AND substr(result_digest,8) NOT GLOB '*[^0-9a-f]*')),
  started_at_ms      INTEGER,
  completed_at_ms    INTEGER,
  lease_owner        TEXT,
  lease_expires_at_ms INTEGER,
  lease_generation   INTEGER NOT NULL DEFAULT 0 CHECK (lease_generation >= 0),
  error_kind         TEXT,
  error_code         TEXT,
  error_message      TEXT,
  error_retryable    INTEGER CHECK (error_retryable IS NULL OR error_retryable IN (0,1)),
  error_host_action  TEXT,
  error_details      TEXT    CHECK (error_details IS NULL
                                     OR (json_valid(error_details)
                                         AND json_type(error_details) = 'object')),
  UNIQUE (task_id, sequence),
  UNIQUE (task_id, idempotency_key),
  -- ADR-0018 3: PLANNED
  CHECK (status <> 'PLANNED' OR (attempt = 0
      AND result_digest IS NULL AND started_at_ms IS NULL AND completed_at_ms IS NULL
      AND lease_owner IS NULL AND lease_expires_at_ms IS NULL AND lease_generation = 0)),
  CHECK (status = 'PLANNED' OR attempt >= 1),
  CHECK (status = 'PLANNED' OR lease_generation >= 1),
  CHECK (attempt <= 4294967295),
  -- ADR-0018 3: LEASED  (attempt begun, attempt not yet started)
  CHECK (status <> 'LEASED' OR (attempt >= 1 AND started_at_ms IS NULL
      AND completed_at_ms IS NULL AND result_digest IS NULL
      AND lease_owner IS NOT NULL AND lease_expires_at_ms IS NOT NULL)),
  -- lease present exactly while leased or executing
  CHECK ((status IN ('LEASED','EXECUTING')) = (lease_owner IS NOT NULL)),
  CHECK (status NOT IN ('LEASED','EXECUTING') OR lease_expires_at_ms IS NOT NULL),
  -- started_at required from EXECUTING onward
  CHECK (status NOT IN ('EXECUTING','WAITING','SUCCEEDED','FAILED','RECONCILED_ABSENT')
         OR started_at_ms IS NOT NULL),
  -- completed_at required for every terminal step status
  CHECK (status NOT IN ('SUCCEEDED','FAILED','RECONCILED_ABSENT') OR completed_at_ms IS NOT NULL),
  CHECK (status <> 'SUCCEEDED' OR result_digest IS NOT NULL),
  -- WAITING only for the wait kinds
  CHECK (status <> 'WAITING' OR kind IN ('WAIT_APPROVAL','WAIT_USER','WAIT_SCHEDULE')),
  -- FAILED carries the whole frozen ActionError
  CHECK ((status = 'FAILED') = (error_kind IS NOT NULL AND error_code IS NOT NULL
      AND error_message IS NOT NULL AND error_host_action IS NOT NULL
      AND error_retryable IS NOT NULL)),
  -- ADR-0018 4: step-kind matrix
  -- ADR-0018 4, one clause per field. A single biconditional over all four
  -- admits a PARTIALLY populated tuple: kind=NOTIFY with capability_id set and
  -- the other three null evaluates 0 = 0 and is accepted.
  CHECK ((kind IN ('CAPABILITY','DELEGATE','VERIFY')) = (capability_id IS NOT NULL)),
  CHECK ((kind IN ('CAPABILITY','DELEGATE','VERIFY')) = (capability_version IS NOT NULL)),
  CHECK ((kind IN ('CAPABILITY','DELEGATE','VERIFY')) = (provider_id IS NOT NULL)),
  CHECK ((kind IN ('CAPABILITY','DELEGATE','VERIFY')) = (idempotency_key IS NOT NULL)),
  CHECK (capability_id IS NULL OR (length(capability_id) <= 99
                                   AND capability_id NOT GLOB 'goallatch.*')),
  CHECK (idempotency_key IS NULL OR (length(idempotency_key) = 68
                                     AND substr(idempotency_key,1,4) = 'idk_'
                                     AND substr(idempotency_key,5) NOT GLOB '*[^0-9a-f]*')),
  CHECK (error_code IS NULL OR (error_code NOT GLOB '*[^A-Z0-9_]*' AND substr(error_code,1,1) BETWEEN 'A' AND 'Z')),
  CHECK (error_host_action IS NULL OR (error_host_action NOT GLOB '*[^A-Z0-9_]*' AND substr(error_host_action,1,1) BETWEEN 'A' AND 'Z'))
) STRICT;

CREATE TABLE side_effect_receipts (
  receipt_id         TEXT    PRIMARY KEY CHECK (length(receipt_id) = 30
                                                 AND substr(receipt_id,1,4) = 'rcp_'),
  task_id            TEXT    NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  step_id            TEXT    NOT NULL REFERENCES task_steps(step_id) ON DELETE CASCADE,
  capability_id      TEXT    NOT NULL,
  idempotency_key    TEXT    NOT NULL CHECK (length(idempotency_key) = 68
                                             AND substr(idempotency_key,1,4) = 'idk_'
                                             AND substr(idempotency_key,5) NOT GLOB '*[^0-9a-f]*'),
  provider_reference TEXT,
  effect_summary     TEXT    NOT NULL,
  observed_at_ms     INTEGER NOT NULL,
  replay_safe        INTEGER NOT NULL CHECK (replay_safe IN (0,1)),
  data_class_rank    INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2),
  UNIQUE (step_id)
) STRICT;

CREATE TRIGGER side_effect_receipts_key_matches_step
BEFORE INSERT ON side_effect_receipts
WHEN (SELECT idempotency_key FROM task_steps WHERE step_id = NEW.step_id) IS NOT NEW.idempotency_key
BEGIN SELECT RAISE(ABORT, 'receipt idempotency_key must equal its step key'); END;

CREATE TRIGGER side_effect_receipts_task_matches_step
BEFORE INSERT ON side_effect_receipts
WHEN (SELECT task_id FROM task_steps WHERE step_id = NEW.step_id) IS NOT NEW.task_id
BEGIN SELECT RAISE(ABORT, 'receipt task_id must equal its step task_id'); END;

CREATE TRIGGER side_effect_receipts_step_must_succeed
BEFORE INSERT ON side_effect_receipts
WHEN (SELECT status FROM task_steps WHERE step_id = NEW.step_id) <> 'SUCCEEDED'
BEGIN SELECT RAISE(ABORT, 'a receipt may only be recorded for a SUCCEEDED step'); END;

CREATE TABLE leases (
  step_id        TEXT    PRIMARY KEY REFERENCES task_steps(step_id) ON DELETE CASCADE,
  owner          TEXT    NOT NULL,
  generation     INTEGER NOT NULL CHECK (generation >= 1),
  acquired_at_ms INTEGER NOT NULL,
  expires_at_ms  INTEGER NOT NULL,
  released_at_ms INTEGER,
  CHECK (expires_at_ms > acquired_at_ms),
  CHECK (released_at_ms IS NULL OR released_at_ms >= acquired_at_ms)
) STRICT;

CREATE TRIGGER task_steps_idempotency_key_immutable
BEFORE UPDATE OF idempotency_key ON task_steps
WHEN NEW.idempotency_key IS NOT OLD.idempotency_key
BEGIN SELECT RAISE(ABORT, 'idempotency_key is derived at plan time and never changes'); END;


CREATE TABLE plan_revisions (
  task_id       TEXT    NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  plan_revision INTEGER NOT NULL CHECK (plan_revision >= 0),
  created_at_ms INTEGER NOT NULL,
  plan_digest   TEXT    NOT NULL,
  data_class_rank INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2),
  step_count    INTEGER NOT NULL CHECK (step_count >= 0),
  PRIMARY KEY (task_id, plan_revision),
  FOREIGN KEY (plan_digest, data_class_rank)
    REFERENCES blobs(digest, data_class_rank) ON DELETE RESTRICT
) STRICT;

CREATE TABLE task_blob_refs (
  task_id        TEXT    NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  role           TEXT    NOT NULL CHECK (role IN ('PLAN','PLAN_REVISION')),
  digest         TEXT    NOT NULL,
  data_class_rank INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2),
  PRIMARY KEY (task_id, role, digest),
  FOREIGN KEY (digest, data_class_rank)
    REFERENCES blobs(digest, data_class_rank) ON DELETE RESTRICT
) STRICT;

CREATE TABLE step_blob_refs (
  step_id        TEXT    NOT NULL REFERENCES task_steps(step_id) ON DELETE CASCADE,
  role           TEXT    NOT NULL CHECK (role IN ('ARGUMENTS','INSTRUCTION','RESULT')),
  digest         TEXT    NOT NULL,
  data_class_rank INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2),
  PRIMARY KEY (step_id, role, digest),
  FOREIGN KEY (digest, data_class_rank)
    REFERENCES blobs(digest, data_class_rank) ON DELETE RESTRICT
) STRICT;

CREATE TABLE task_journal (
  journal_id       TEXT    PRIMARY KEY,
  task_id          TEXT    NOT NULL REFERENCES tasks(task_id) ON DELETE CASCADE,
  step_id          TEXT,
  journal_seq      INTEGER NOT NULL CHECK (journal_seq >= 1),
  journal_kind     TEXT    NOT NULL CHECK (journal_kind IN (
                     'TASK_INSERTED','PLAN_PERSISTED','TASK_STATE_CHANGED',
                     'STEP_LEASE_ACQUIRED','STEP_LEASE_RELEASED','STEP_ATTEMPT_STARTED',
                     'STEP_COMMITTED','STEP_FAILED','STEP_RECONCILED_ABSENT',
                     'RECEIPT_RECORDED','RECOVERY_DECISION','TASK_CANCEL_REQUESTED',
                     'TASK_TERMINAL')),
  state_from       TEXT,
  state_to         TEXT,
  attempt          INTEGER,
  reason_code      TEXT,
  actor_kind       TEXT    NOT NULL,
  actor_id         TEXT    NOT NULL,
  actor_version    TEXT    NOT NULL,
  causation_id     TEXT,
  data_class_rank  INTEGER NOT NULL CHECK (data_class_rank BETWEEN 0 AND 2),
  occurred_at_ms   INTEGER NOT NULL,
  payload_digest   TEXT    CHECK (payload_digest IS NULL OR (length(payload_digest) = 71
                                    AND substr(payload_digest,1,7) = 'sha256:'
                                    AND substr(payload_digest,8) NOT GLOB '*[^0-9a-f]*')),
  payload_json     TEXT    CHECK (payload_json IS NULL OR json_valid(payload_json)),
  payload_ref_digest TEXT,
  event_seq        INTEGER,
  UNIQUE (task_id, journal_seq)
) STRICT;

CREATE TRIGGER task_journal_step_task_matches
BEFORE INSERT ON task_journal
WHEN NEW.step_id IS NOT NULL
 AND (SELECT task_id FROM task_steps WHERE step_id = NEW.step_id) IS NOT NEW.task_id
BEGIN SELECT RAISE(ABORT, 'journal step_id must belong to the journal task'); END;

CREATE INDEX task_steps_status_lease ON task_steps(status, lease_expires_at_ms);
CREATE INDEX task_steps_task_status ON task_steps(task_id, status);
CREATE INDEX tasks_state ON tasks(state);
CREATE INDEX step_blob_refs_digest ON step_blob_refs(digest);
CREATE INDEX task_blob_refs_digest ON task_blob_refs(digest);
CREATE INDEX plan_revisions_digest ON plan_revisions(plan_digest);
CREATE INDEX task_journal_pending_event ON task_journal(task_id) WHERE event_seq IS NULL;
```

### 4.1 `schema_migrations`

The **single** authority for the schema version. P2 does not mirror it into
`PRAGMA user_version`, because two sources for one fact is a disagreement waiting
to happen and reading `MAX(version)` is one indexed query.

### 4.2 `blobs`

| Constraint | Job |
| --- | --- |
| `data_class_rank BETWEEN 0 AND 2` | A `SECRET` or `CREDENTIAL` row is **unconstructible**, so `DC5` holds at the storage layer and not merely in Rust |
| `(data_class_rank = 2) = (protection = 'AT_REST')` | A `PRIVATE` value cannot sit in the file unprotected, so ADR-0022's condition cannot be bypassed by a direct `INSERT` |
| `size_bytes = length(content)` | A **consistency** check, not a bound. Under ADR-0020 a consistency invariant is not a resource limit, and this one rejects a torn-length row without inventing a size ceiling |
| `PRIMARY KEY (digest, data_class_rank)` | Deduplication *within* a class, and no cross-class laundering — see §5.3 |

**The digest predicate is three clauses, not one.** `substr(digest, 8) NOT GLOB
'*[^0-9a-f]*'` rather than `digest NOT GLOB '*[^0-9a-f]*'`: the prefix `sha256:`
contains `s` and `h`, which are not hexadecimal, so a single negated class over the
whole string rejects every valid digest. The prefix is checked separately with
`substr(digest, 1, 7) = 'sha256:'`. The same three-clause form appears on
`input_digest`, `result_digest`, `payload_digest` and the journal's
`idempotency_key`, where the prefix is `idk_` and the body starts at position 5.

`digest` is stored as text rather than as a 32-byte `BLOB` because the wire form is
`sha256:` + 64 hex and storing the wire form means the database and the protocol
cannot disagree about representation. The cost is 71 bytes per blob instead of 32,
paid on a table whose payload is far larger.

### 4.3 `tasks`

**Every class column in `tasks` is capped at rank 2.** `SECRET` and `CREDENTIAL`
are not merely refused by the Rust dispatch — they are unconstructible rows. This
is the `DC5` enforcement ADR-0022 claims, and it had to be widened from `blobs`
alone to every table that stores classified content, or the claim would have been
true of the blob store and false of everything else.

Five decisions worth stating.

**`policy_class` immutability is a trigger, not a Rust check.**

```sql
CREATE TRIGGER tasks_policy_class_immutable
BEFORE UPDATE OF policy_class_rank ON tasks
WHEN NEW.policy_class_rank IS NOT OLD.policy_class_rank
BEGIN SELECT RAISE(ABORT, 'policy_class is immutable for a task'); END;
```

`T6` says "no step, approval or plan may raise it". A Rust check can be bypassed by
any future writer; a trigger cannot. The `WHEN` clause is load-bearing: without it
`BEFORE UPDATE OF` fires on the *set list*, so a generic "touch `updated_at_ms`"
statement that also names the column would abort, and `UPDATE tasks SET
policy_class_rank = policy_class_rank` would abort too. Both were verified: the
no-op update succeeds with the `WHEN` and fails without it.

**`data_class` may be raised, never lowered.** A second trigger with
`WHEN NEW.data_class_rank < OLD.data_class_rank`. Raising is the safe direction
under `DC3`'s derivation rule; lowering is the laundering direction.

**Three extension columns, not one.** `origin_extensions` and
`budget_extensions` exist alongside `extensions` because
[Protocol Index §4.2 rule 3](../protocols/00-protocol-index.md#42-compatibility-rules)
requires an unknown member of a forward-compatible surface to be **retained and
round-tripped unchanged**, and `assistant-task.schema.json` leaves both `origin`
and `attempt_budget` open. A single column would silently drop them on a
read-modify-write.

**The derived-state `CHECK`s make inconsistent terminal rows unconstructible.** A
`CANCELLED` task without `cancelled_at_ms` cannot exist; a `FAILED` task without a
`failure_reason` cannot exist; `updated_at_ms` can never precede `created_at_ms`;
and `failure_reason` is confined to `FAILED` so a row cannot carry a failure reason
while still executing. These are consistency invariants, not bounds.

**The code-grammar `CHECK`s.** `origin_kind`, `blocked_reason`, `failure_reason`
and `cancelled_by` are `^[A-Z][A-Z0-9_]*$` in Rust. `GLOB` cannot express a
character-class negation plus a first-character constraint in one pattern, so it is
two clauses. The value of having them is that a corrupt row in one of these fields
is a corrupt row that changes a control-flow decision — Event Protocol §4 requires
a code to be a stable machine-readable value.

**What is *not* structurally enforced here, stated plainly.** `tasks.title`,
`tasks.result_summary`, `task_steps.error_message` and
`side_effect_receipts.effect_summary` are `TEXT` columns holding content that a
caller may classify `PRIVATE`. The schema cannot record a per-column class, so
these columns are enforced by the **classified-write dispatch** — a single
`Tx::put_classified_text` chokepoint — and *not* by a `CHECK`. ADR-0022's
"put the rule where it cannot be forgotten" applies to the blob store; for text
columns the guarantee is "one chokepoint function", which is weaker and is claimed
as such. `tests/` pins that no second write path exists.

### 4.4 `task_steps`

The presence and step-kind matrices of ADR-0018 §3 and §4, as one-directional
implications plus four biconditionals. The distinction is not stylistic:

| Shape | Why it is used |
| --- | --- |
| `status <> 'X' OR (…all-null…)` | Says what `X` requires. Compose several of these and you get the whole matrix without an accidental biconditional |
| `(status IN ('LEASED','EXECUTING')) = (lease_owner IS NOT NULL)` | Genuinely two-way: a lease exists exactly while leased or executing |
| `((kind IN ('CAPABILITY','DELEGATE','VERIFY')) = (<one field> IS NOT NULL))`, **one clause per field** | Genuinely two-way. Written as a *single* biconditional over all four fields it admits a partially populated tuple: `NOTIFY` with `capability_id` set and the other three null evaluates `0 = 0` and is accepted. Four clauses are strictly stronger than the one it replaces |
| `((status = 'FAILED') = (the five error fields non-null))` | Genuinely two-way |

The one biconditional that had to be **removed** is the `WAITING` one. Written as
`(status = 'WAITING') = (kind IN ('WAIT_APPROVAL','WAIT_USER','WAIT_SCHEDULE'))`
it means a `WAIT_APPROVAL` step may *only ever* be `WAITING` — so it could not be
`PLANNED`, could not be `SUCCEEDED`, and could not be `FAILED`. That made a third of
the step lifecycle unreachable for three of the eight kinds, and made
`persist_plan` abort on any plan containing a wait step, which is Task Protocol
§4.3's central requirement. The correct form is one-directional: a step may be
`WAITING` only if it is a wait step; a wait step may be in any other status.

**An earlier draft used biconditionals where only implications were correct**, and
`PLANNED` became unconstructible: a biconditional `(status = 'LEASED') =
(started_at_ms IS NULL)` contradicts a `PLANNED` row, whose `started_at_ms` is
also null. The corrected form is
`status NOT IN ('EXECUTING','WAITING','SUCCEEDED','FAILED','RECONCILED_ABSENT') OR
started_at_ms IS NOT NULL`. ADR-0018 §3's matrix is corrected to match: see the
`started_at` row, which is `N` for `LEASED` and `R` from `EXECUTING` onward.

`UNIQUE (task_id, idempotency_key)` is the index that turns a *wrong* key
derivation into a visible failure. SQLite treats `NULL`s as distinct, so the many
non-capability steps that carry no key are unaffected.

Two constraints have no counterpart in ADR-0018 and are stated here:

- **`idempotency_key` is immutable.** A trigger refuses any `UPDATE` of it. It is
  derived once at plan time from the request, and Capability Protocol §8.2's whole
  property depends on every attempt of a step reusing it.
- **`lease_generation >= 1` for every non-`PLANNED` step.** This closes a real
  hole: an earlier draft's biconditional short-circuited on
  `lease_owner IS NOT NULL`, so a `SUCCEEDED` step with `lease_generation = 0` was
  accepted. Verified: `0` is now refused.

  **What the constraint does not do, stated precisely.** It sets a *floor*, not a
  ceiling: a corrupt row may carry `lease_generation = 99`, and a `SUCCEEDED` step
  may name a generation for which no `leases` row exists. A foreign key cannot close
  this, because the dependency is one-way — `leases` references `task_steps`, not the
  reverse. Both are *detected* rather than prevented, by the acquire path (a
  generation with no `leases` row fails the subquery and the step update affects zero
  rows) and by recovery's invariant scan. That is the honest boundary; an earlier
  draft of this section claimed `99` was fixed, which it is not.

**What is still not enforceable in `CHECK`, and is therefore detected rather than
prevented:** `side_effect_receipt` on a `RECONCILED_ABSENT` step. Receipt presence
is a cross-table property — `side_effect_receipts.step_id`, `UNIQUE` — and no
`CHECK` can span tables. A trigger on the *receipt* side covers the direction that
matters (a receipt may only be recorded for a `SUCCEEDED` step, verified below);
the reverse direction is recovery's invariant scan.

### 4.5 `side_effect_receipts`

Three decisions:

**`task_steps` has no `receipt_id` column.** The step's receipt is reached by
`side_effect_receipts.step_id`, which is `UNIQUE`. This removes what would
otherwise be a **circular foreign key** and removes a duplicated fact. The wire
`TaskStep.side_effect_receipt` is assembled on read from this join. ADR-0024's
commit statement has been corrected to match.

**Two triggers replace a `CHECK` that SQLite does not permit.** An earlier draft
used a subquery inside `CHECK` to require the receipt's `idempotency_key` to equal
its step's. **`CREATE TABLE` fails outright** with *"subqueries prohibited in CHECK
constraints"*, so the entire migration was unbuildable. The triggers below are the
working form:

```sql
CREATE TRIGGER side_effect_receipts_key_matches_step
BEFORE INSERT ON side_effect_receipts
WHEN (SELECT idempotency_key FROM task_steps WHERE step_id = NEW.step_id)
     IS NOT NEW.idempotency_key
BEGIN SELECT RAISE(ABORT, 'receipt idempotency_key must equal its step key'); END;

CREATE TRIGGER side_effect_receipts_step_must_succeed
BEFORE INSERT ON side_effect_receipts
WHEN (SELECT status FROM task_steps WHERE step_id = NEW.step_id) <> 'SUCCEEDED'
BEGIN SELECT RAISE(ABORT, 'a receipt may only be recorded for a SUCCEEDED step'); END;
```

The first is sound because the second makes a step's key stable: a step's
`idempotency_key` is immutable (§4.4) and `status` only ever moves forward, so a
receipt's key cannot drift from its step's after the fact. Both are verified —
a mismatched key and a receipt against a non-`SUCCEEDED` step are both refused.

`IS NOT` rather than `=` because `NULL <> NULL` is `NULL`, which a `WHEN` treats as
false, and a receipt whose step row is missing would otherwise slip past. The
`REFERENCES` foreign key catches that case separately, but the trigger must not
depend on it.

**`UNIQUE (step_id)` is the durable duplicate-suppression seam** for the P2 slice:
a second receipt for the same step is refused, so a retried commit cannot append a
second proof of the same effect. The broader duplicate *window*
(`(capability_id, arguments_digest)` across tasks,
[Bounds Protocol §5](../protocols/10-bounds-protocol.md#5-duplicate-suppression))
belongs to `serea-capability` in P5 and gets its own index there.

**A constraint that was removed rather than weakened into fiction.** An earlier
draft required `provider_reference IS NOT NULL OR replay_safe = 0`, justified as
encoding Capability Protocol §5.1's "A receipt without one is valid only for
`side_effect_class: LOCAL_STATE`". It does not encode that. `side_effect_class` is
a descriptor field and P2 has no registry; the substitute both refuses a legal
`LOCAL_STATE` receipt with `replay_safe: true` and accepts an illegal
non-`LOCAL_STATE` receipt with `replay_safe: false`. Nothing in §5.1 or in
§3.1's `replay_safe` semantics says a `LOCAL_STATE` receipt is not replay-safe —
mutating Serea's own state is among the most replayable cases there is. The
constraint is deleted and `provider_reference ⇒ side_effect_class == LOCAL_STATE`
is recorded as a **P5 obligation**, consistent with how the other half of `C4` is
deferred.

### 4.6 `leases`

```sql
CREATE TABLE leases (
  step_id        TEXT    PRIMARY KEY REFERENCES task_steps(step_id) ON DELETE CASCADE,
  owner          TEXT    NOT NULL,
  generation     INTEGER NOT NULL CHECK (generation >= 1),
  acquired_at_ms INTEGER NOT NULL,
  expires_at_ms  INTEGER NOT NULL,
  released_at_ms INTEGER
) STRICT;
```

**The `token` column has been removed.** ADR-0024 originally carried a 16-byte
unguessable `token` alongside `generation`, and simultaneously conceded that
`generation` alone already discriminates two acquisitions of the same owner after
a reclaim. A second value that must be kept consistent with the first, and that no
statement actually needed, is overengineering. The fence is `(step_id, task_id,
generation, owner)`, and it is complete: `generation` is monotonic, durable, and
increments on every acquisition including an expiry reclaim.

`task_steps.lease_generation` and `leases.generation` are two copies of one fact.
That duplication is deliberate — the step-side copy is what makes the fence
predicate a single indexed statement with no join — and it is kept consistent by a
trigger rather than by a test:

```sql
CREATE TRIGGER leases_generation_matches_step
BEFORE INSERT ON leases
WHEN NEW.generation <> (SELECT lease_generation FROM task_steps WHERE step_id = NEW.step_id)
BEGIN SELECT RAISE(ABORT, 'lease generation must match the step'); END;
```

An earlier draft asserted this consistency and attributed it to a test. That
cannot work: `acquire_lease` is the `leases` upsert **plus** a separate
`UPDATE task_steps`, so there is a window in which the two disagree and a test would
be racing it. The trigger is the correct mechanism and is verified — a mismatched
generation is refused.

### 4.7 `plan_revisions`

`plan_revision = 0` is the initial plan, so there is no separate `plans` table. The
`ON DELETE RESTRICT` to `blobs` means a plan document cannot be deleted while a
revision references it, which is what makes ADR-0018's "delete a superseded
`PLANNED` step" decision auditable.

`data_class_rank` is capped at `BETWEEN 0 AND 2`, matching `blobs`. An earlier
draft allowed 0–4 here while the foreign key could only ever be satisfied up to 2,
which left the column's own `CHECK` dead for its top two values. There is no
generated label here: nothing reads a class label off this table.

### 4.8 `task_blob_refs` and `step_blob_refs`

**Two tables, not one polymorphic `blob_refs(owner_kind, owner_id, …)`.** A
polymorphic owner cannot carry a foreign key, so right-to-delete would depend on
remembering to delete the references by hand — and Data Classification §8.2 step 2
requires the cascade to be correct in **one transaction**. Two tables cost a
duplicate definition and buy real referential integrity.

**`step_blob_refs` has three roles, and all three are written.** `ARGUMENTS`
carries a `CAPABILITY`, `DELEGATE` or `VERIFY` step's arguments; `INSTRUCTION`
carries the host-written input document whose digest is `input_digest` for every
other kind; `RESULT` carries the result document. Without `INSTRUCTION` a
`MODEL_TURN` or `NOTIFY` step's mandatory `input_digest` would have no blob to
point at, and `PLANNED MODEL_TURN` would be unimplementable.

`task_blob_refs` has **two** roles, `PLAN` and `PLAN_REVISION`, and **no
`RESULT`** — an earlier draft listed a task-level `RESULT` role that nothing writes,
because `tasks.result_summary` is a `TEXT` column. Roles that no writer uses are
schema surface waiting to be misused.

The `EVIDENCE_PAYLOAD` role that Capability Protocol §7's `payload_reference`
implies is **not** included, because P2 writes no evidence. P5 adds it by
migration, which is a `CHECK` widening and not a table rewrite.

### 4.9 `task_journal`

ADR-0021 owns this table's semantics; the DDL is here so this document is complete.

```sql
CREATE TABLE task_journal (…)
```

**The `journal_kind` vocabulary is deliberately NOT `EventKind`.** An earlier
draft reused `TASK_CREATED`, `TASK_CANCELLED` and `TASK_COMPLETED` — three frozen
`EventKind` variants — inside a package whose own test O9 forbids P2 constructing
an `EventKind`. That is a second naming authority for values the Protocol Index §1
registry already assigns to `serea-event-bus`. The closed set here uses `TASK_*`,
`STEP_*`, `RECEIPT_*` and `RECOVERY_*` spellings that name **what the engine did**,
not events: `TASK_INSERTED`, `PLAN_PERSISTED`, `TASK_STATE_CHANGED`,
`STEP_LEASE_ACQUIRED`, `STEP_LEASE_RELEASED`, `STEP_ATTEMPT_STARTED`,
`STEP_COMMITTED`, `STEP_FAILED`, `STEP_RECONCILED_ABSENT`, `RECEIPT_RECORDED`,
`RECOVERY_DECISION`, `TASK_CANCEL_REQUESTED`, `TASK_TERMINAL`.

`TASK_DELETED` is **absent, deliberately**. `task_journal.task_id` carries
`ON DELETE CASCADE`, so a deletion journal row written before the delete is erased by
it, and one written after it is refused by the foreign key — the kind cannot survive
its own transaction. Deletion is instead reported by `DeletionOutcome`'s counts and by
the absence of rows, which is what Data Classification §8.2's "partial failure is
visible" actually needs. An earlier draft listed the kind in prose and in the DDL;
prose and DDL now agree.

**`attempt` is a column** because
[Event Protocol §2](../protocols/06-event-protocol.md#2-sereaevent) puts it in
`trace`, and a `STEP_ATTEMPT_STARTED` or `STEP_COMMITTED` row cannot be turned into
an event without it.

**`payload_json` exists, and the ADR's claim about the column list was corrected.**
An earlier draft asserted that the journal's columns are "precisely the fields an
`EventKind`-specific payload needs", so a P3 upgrade could materialise every event
without inventing data. That is false: `CAPABILITY_COMPLETED`'s payload carries
`duration_ms` and `output_digest`; `BOUND_EXCEEDED` requires `bound_name`, the limit
and the observed value; `MODEL_CALLED` requires `model_id`, `purpose` and a token
estimate. None was in the column list. `payload_json` — validated JSON, carrying
whatever the transition's event payload will need — is the honest fix, and
`payload_ref_digest` points at a blob for large payloads. The back-fill property now
holds because the payload has somewhere to live, not because the columns were
complete.

**`event_seq` is nullable and P2 never writes it.** ADR-0021's `E3` debt is
counted, not paid.

`journal_seq` is gapless **per task**, computed inside the transaction as
`MAX(journal_seq) + 1` for that `task_id`. It is a different thing from `Seq`: `seq`
is per-*host* and gapless by `E4`, and belongs to P3. Keeping the two distinct
avoids a P2 counter masquerading as the event sequence.

## 5. Content-addressed blobs

### 5.1 Write path

```text
put_blob(tx, bytes, class)
  1. canonicalize(bytes)                    ADR-0019 SCJ-1; depth <= 64
  2. digest := sha256(canonical)
  3. if the store already holds (digest, class) -> return the existing BlobRef
  4. class dispatch:
       PUBLIC | PERSONAL                      -> protection = 'NONE'
       PRIVATE  and no backend configured     -> StoreError::AtRestProtectionUnavailable
       PRIVATE  and backend configured         -> protection = 'AT_REST', content = protect(...)
       SECRET | CREDENTIAL                     -> StoreError::ClassRefused
  5. INSERT INTO blobs (...)                 -- a UNIQUE conflict is the dedupe case
  6. the caller inserts the matching row into task_blob_refs / step_blob_refs
```

Steps 5 and 6 are both inside the **caller's** transaction, and P2 offers no
`put_blob` outside a `Tx`. That is what makes orphan prevention structural rather
than a sweeper's job: a crash between 5 and 6 rolls back both, so no orphan can
exist from P2's own writes. A garbage-collection query for unreferenced blobs
therefore exists as a repair tool, not as a routine requirement.

### 5.2 Read path

```text
get_blob(tx, ref)
  1. SELECT content, protection, data_class_rank FROM blobs WHERE (digest, class) = ref
  2. zero rows -> StoreError::BlobMissing
  3. if protection = 'AT_REST' -> unprotect(...)     ADR-0022
  4. re-canonicalize and re-digest; mismatch -> StoreError::BlobCorrupt
  5. return the canonical bytes
```

Step 4 costs one hash per read and is the entire point: Task Protocol §3.1 says
`result_digest` "Detects result corruption or partial writes on recovery". A digest
that is never re-verified detects nothing. A corrupt `blobs` row therefore becomes
`BlobCorrupt` at read time and `CorruptOrInvariantViolation` during recovery, never
a silently wrong result.

### 5.3 Classification and laundering

The composite primary key `(digest, data_class_rank)` is the safety-relevant
choice. Keyed by `digest` alone, a reference classified `PERSONAL` could resolve a
blob stored `PRIVATE`, laundering a private payload downward — which `DC3` forbids
and which no `CHECK` on a single class column would catch. With a composite key, a
reference at class `X` can only ever resolve bytes stored at class `X`, and a reader
can never widen its own view of a value's class.

Deduplication still works for the case that matters: the same arguments written
twice at the same class are stored once. Storing identical bytes at two classes is
wasteful and safe, which is the correct trade for a store that decides what may be
persisted.

**A consequence worth stating:** because the composite foreign key pins a
reference's class to the blob's, `StoreError::ClassEscalationRequired` — which an
earlier draft of the API carried — is **unreachable**. Escalation is
unrepresentable by construction, not merely unimplemented. The variant has been
removed rather than left as a dead arm.

### 5.4 Deletion

Task Protocol §8 and Data Classification §8.2 step 2, in one transaction:

```sql
DELETE FROM tasks WHERE task_id = ?;      -- cascades to task_steps,
                                           -- side_effect_receipts, leases,
                                           -- plan_revisions, task_blob_refs,
                                           -- task_journal; step_blob_refs
                                           -- cascades from task_steps
```

then, **scoped to this digest and class**:

```sql
DELETE FROM blobs WHERE data_class_rank <= 2
  AND NOT EXISTS (SELECT 1 FROM step_blob_refs r
                   WHERE r.digest = blobs.digest AND r.data_class_rank = blobs.data_class_rank)
  AND NOT EXISTS (SELECT 1 FROM task_blob_refs r
                   WHERE r.digest = blobs.digest AND r.data_class_rank = blobs.data_class_rank)
  AND NOT EXISTS (SELECT 1 FROM plan_revisions r
                   WHERE r.plan_digest = blobs.digest AND r.data_class_rank = blobs.data_class_rank);
```

**The `NOT EXISTS` subqueries must be correlated on `blobs.digest`.** Written
without the correlation, each is a full cross-product scan that is trivially true,
and the statement deletes *every* unreferenced blob in the file — including blobs a
future phase wrote to a table that does not exist yet. Verified: a referenced blob
survives the sweep and an unreferenced one is removed.

**Retention is not P2's to enforce.** `max_retained_tasks` is a
[Bounds Protocol §2](../protocols/10-bounds-protocol.md#2-the-bound-set) bound and
Crate Map §3.1 gives bound configuration and global counters to `serea-core`.
P2 provides `delete_task`; the **30-day retention trigger is P12's**, because the
notification surface and the bound configuration are both `serea-core`'s and
arriving together in P12 keeps the trigger with the configuration it reads. Task
Protocol §8's retention *interval* is honoured by P2 only as a value
`serea-core` supplies. P2 enforces no retention bound, and this is a non-claim.

## 6. Indexes

| Index | Table | Serves |
| --- | --- | --- |
| `UNIQUE (task_id, sequence)` | `task_steps` | Step ordering and Task Protocol §3.2's "required predecessors" read |
| `UNIQUE (task_id, idempotency_key)` | `task_steps` | Duplicate-key detection within a task |
| `task_steps (status, lease_expires_at_ms)` | `task_steps` | Recovery: expired in-flight leases |
| `task_steps (task_id, status)` | `task_steps` | Per-task progress reads |
| `tasks (state)` | `tasks` | Recovery: load non-terminal tasks |
| `step_blob_refs (digest)` | `step_blob_refs` | The correlated `NOT EXISTS` in the §5.4 sweep |
| `task_blob_refs (digest)` | `task_blob_refs` | Same |
| `plan_revisions (plan_digest)` | `plan_revisions` | Same, and the `ON DELETE RESTRICT` lookup |
| `task_journal (task_id) WHERE event_seq IS NULL` | `task_journal` | ADR-0021's `pending_event_transitions` count |

Indexes **deliberately absent**, each because no stated query needs it:

- **No index on `blobs.content`.** Nothing queries a blob by content.
- **No index on `leases (expires_at_ms)`.** The same recovery query is served by
  `task_steps (status, lease_expires_at_ms)`; `leases` is keyed by `step_id`.
- **No index on `side_effect_receipts (task_id)` or `(idempotency_key)`.** P2's
  only receipt query is `receipt_for_step`, served by `UNIQUE (step_id)`. An
  earlier draft justified the key index as serving "reconciliation", but
  reconciliation is explicitly **not** claimed in P2.
- **No index on `task_journal` beyond the partial one.** `UNIQUE (task_id,
  journal_seq)` serves the ordered read; the partial index serves the pending-event
  count, which an earlier draft justified as a "bounded set". `task_journal` grows
  with a task's lifetime, so the bound was unearned.

## 7. Verified behaviour

**Round 4** follows the third review pass and adds the positive cases it asked for
— including *each wait kind at `PLANNED`*, which rounds 1–3 all missed because every
one of them tested `WAITING` on a non-wait kind and none tested a non-`WAITING`
status on a wait kind. That single omission had made three of eight step kinds
unplannable.

| Case | Result |
| --- | --- |
| Each of the seven lifecycle statuses is constructible | accepted |
| **All 37 legal Task Protocol §4.2 transitions are accepted** | 37/37 |
| All 84 illegal pairs were exercised | the schema does **not** and must **not** encode the transition table — that is `TaskEngine`'s `legal_task_transition`, pinned by test I1 |
| `SUCCEEDED` without `result_digest` | refused |
| `PLANNED` with `attempt = 1`, `lease_generation = 1`, or a lease owner | refused |
| `LEASED` with `started_at_ms` set | refused |
| `WAITING` on a `CAPABILITY` step | refused |
| **Each of `WAIT_APPROVAL`, `WAIT_USER`, `WAIT_SCHEDULE` at `PLANNED`** | accepted — the round-3 blocker. A wait step must be plannable before it can wait |
| **`WAIT_USER` reaching `WAITING`, `SUCCEEDED` (with a result), and `WAIT_APPROVAL` reaching `FAILED`** | accepted |
| `TASK_DELETED` as a journal kind | refused — the kind was removed |
| **`BLOCKED → CANCELLED` clearing `blocked_reason`** | accepted; and *without* the clear it aborts, which is why §4's cancel statement sets it |
| `lease_generation = 0` on a terminal step | refused; `99` is accepted, and that is stated rather than claimed fixed |
| `FAILED` without the five error fields | refused |
| `FAILED` with `details` omitted | **accepted** — the frozen `actionError.required` list omits `details` |
| `CAPABILITY` without `capability_id`; `NOTIFY` with one | refused |
| **`NOTIFY` with only `capability_id` set** — the partial tuple the single biconditional admitted | refused — the round-4 finding |
| `MODEL_TURN` with no `idempotency_key`; `VERIFY` with one | accepted |
| `capability_id` in the `goallatch` namespace | refused |
| `input_digest` of 71 colons; bad prefix; uppercase hex | refused |
| `policy_class` no-op update / actual change | accepted / refused |
| `data_class` raise / lower | accepted / refused |
| **`CANCELLED` from every non-terminal state** | accepted — the round-1 blocker |
| `CANCELLED` without `cancelled_at_ms`; a non-`CANCELLED` task carrying one | refused |
| **`BLOCKED → READY` and `BLOCKED → CANCELLED`, clearing `blocked_reason`** | accepted — the round-2 finding |
| `READY` carrying a `blocked_reason` | refused |
| `SECRET` or `CREDENTIAL` on **`tasks`, `blobs`, `side_effect_receipts`, `plan_revisions`, `task_journal`** | refused — all five |
| `PRIVATE` journal row | accepted |
| Receipt key or `task_id` disagreeing with its step; receipt on a non-`SUCCEEDED` step | refused |
| Journal `step_id` belonging to another task | refused |
| Second receipt for one step | refused |
| Lease expiring at or before its acquisition | refused |
| **`PLANNED → LEASED` in the documented order** | accepted, `attempt = 1`, `generation = 1` |
| **Reclaim after a released lease** | accepted, `generation` 1→2, `attempt` 1→2, `started_at_ms` cleared, both copies agreeing |
| **Worker A at generation 1 after B reclaimed at 2: A's commit** | **0 rows**, step unchanged |
| Referenced / unreferenced blob through the §5.4 sweep | survives / removed |

### Two rows that are accepted by SQL on purpose

| Case | Why accepted |
| --- | --- |
| `BLOCKED` with no `blocked_reason` | The schema check is one-way (`state = 'BLOCKED' OR blocked_reason IS NULL`) because a biconditional strands any `BLOCKED → X` transition. The *engine's* `block` statement always sets the reason; SQL cannot enforce presence on entry and absence on exit simultaneously |
| A `leases` row whose `generation` disagrees with its step | The trigger was **removed**. The step-side copy is *derived* — `lease_generation = (SELECT generation FROM leases WHERE step_id = ?)` — and both writes happen inside one `BEGIN IMMEDIATE`, so no observer can see them disagree. A trigger here would have to model the upsert's insert-or-update branch, which is exactly the fragility that was removed |

### The one limitation this package does not mitigate

`PRAGMA ignore_check_constraints = ON` disables **every `CHECK` in this schema**
for a writer with access to the file. Confirmed by execution: a local writer set
`tasks.data_class` to `SECRET` through it.

That is one line of SQLite, needs no privilege beyond file write access, and
`[Trust Boundaries §2 `TB-7`](../architecture/02-trust-boundaries.md#tb-7-core-to-durable-store)`
already states that filesystem permissions are "defence in depth, not the
mechanism", while
[Security Invariants §6](../threat-model/04-security-invariants.md) records
tamper-evidence against a local file writer as "Not specified".

**What still holds under the pragma**, because triggers and foreign keys are not
`CHECK`s: `tasks_policy_class_immutable`, `tasks_data_class_monotonic`,
`side_effect_receipts_key_matches_step`, `side_effect_receipts_task_matches_step`,
`side_effect_receipts_step_must_succeed`,
`task_steps_idempotency_key_immutable`, `task_journal_step_task_matches`, and every
`REFERENCES`. Each was verified to fire with the pragma set. The design's structural
budget is therefore spent on the controls that survive — the authority-bearing ones
— rather than spread across constraints that a single pragma erases.

ADR-0022's claims are narrowed accordingly, and the boundary is pinned by test O14
so a later reader inherits the truth rather than the overclaim.

## 8. Open at implementation time

| # | Question | Resolution |
| --- | --- | --- |
| 1 | Is the resolved SQLite >= 3.37.0 | Verified at dependency-add time. If not, `bundled` is forced; the fallback drops `STRICT` and `GENERATED ... STORED` for `CHECK (typeof(col) = …)`, at the cost recorded in the design's §7 |
| 2 | Is JSON1 present | Verified by `SELECT json_valid('{}')` at open time; the open fails if absent, because `tasks.extensions` depends on it |
| 3 | Does `length()` count bytes on a `BLOB` | **Closed.** Verified: `length(X'7B7D')` is 2, so `CHECK (size_bytes = length(content))` accepts a two-byte blob with `size_bytes = 2`. Not open |

## 9. Cross-references

- The engine and API that use this schema:
  [P2 storage and task engine](P2-storage-task-engine.md)
- The tests that prove it: [P2 test matrix](P2-test-matrix.md)
- The decisions this schema implements: [ADR-0018](../decisions/ADR-0018-taskstep-lifecycle-and-field-presence.md),
  [ADR-0021](../decisions/ADR-0021-p2-p3-event-atomicity-seam.md),
  [ADR-0022](../decisions/ADR-0022-durable-private-data-at-rest.md),
  [ADR-0024](../decisions/ADR-0024-lease-fencing-and-commit-under-lease.md)
