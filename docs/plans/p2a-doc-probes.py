#!/usr/bin/env python3
"""Specification probes plus production migration construction; no persistent DB.

Run from the workspace root: python3 docs/plans/p2a-doc-probes.py
Reads the authoritative production SQL, documented patterns and vectors. This
is not a Rust Store/runtime, bundling, durability or later-phase acceptance test.
"""
import hashlib
import itertools
import json
from pathlib import Path
import re
import sqlite3
import struct
import subprocess

DOCS = Path(__file__).resolve().parents[1]
ROOT = DOCS.parent

def document(path):
    return (DOCS / path).read_text()

def blocks(text, language):
    return re.findall(r"```" + language + r"\n(.*?)```", text, re.S)

# Expand the fragments actually published by ADR-0023, rather than a copied pattern.
text = document("decisions/ADR-0023-text-field-validation-categories.md")
fragments = next(b for b in blocks(text, "text") if b.startswith("W = "))
f = dict(line.split(" = ", 1) for line in fragments.strip().splitlines())
b = f["B"].replace("W", f["W"])
patterns = {c: f[c].replace("B", b).replace("E", f["E"]) for c in ("L", "P")}
o = next(block for block in blocks(text, "text") if block.startswith("B\n"))
patterns["O"] = re.sub(r"\s+", "", o).replace("B", b).replace("E", f["E"])
node = r'''
const assert = require('node:assert/strict');
const patterns = JSON.parse(process.argv[1]);
const regex = Object.fromEntries(Object.entries(patterns).map(([k,v]) => [k,new RegExp(v,'u')]));
const ws = new Set([9,10,11,12,13,32,133,160,5760,8192,8193,8194,8195,8196,8197,8198,8199,8200,8201,8202,8232,8233,8239,8287,12288]);
let checks=0;
function check(cat,s,want) { assert.equal(regex[cat].test(s),want,cat+' '+JSON.stringify(s)); checks++; }
function allowed(cat,n) {
  if (cat==='P') return !(n<=8 || (n>=11 && n<=31) || (n>=127 && n<=159));
  return !(n<=31 || (n>=127 && n<=159) || n===8232 || n===8233);
}
// Every Unicode scalar in interior, leading and trailing positions, all categories.
for(let n=0;n<=0x10ffff;n++) {
  if(n>=0xd800 && n<=0xdfff) continue;
  const c=String.fromCodePoint(n);
  for(const cat of ['O','L','P']) {
    check(cat,'x'+c+'x',allowed(cat,n));
    check(cat,c+'x',allowed(cat,n) && !ws.has(n));
    check(cat,'x'+c,allowed(cat,n) && !ws.has(n));
  }
}
for(const cat of ['O','L','P']) { check(cat,'',false); for(const n of ws) check(cat,String.fromCodePoint(n),false); }
const prefixes=['tsk','stp','apr','grt','req','evt','dev','sch','prop','rcp','ses'];
const verbs=['list','read','search','open','control','write','create','send','delete','start','status','run','cancel','result'];
const crockford='0123456789ABCDEFGHJKMNPQRSTVWXYZ';
const hex='0123456789abcdef';
const every=(s,alphabet)=>[...s].every(c=>alphabet.includes(c));
const segment=s=>s.length>=2 && s.length<=32 && 'abcdefghijklmnopqrstuvwxyz'.includes(s[0]) && every(s,'abcdefghijklmnopqrstuvwxyz0123456789_');
function identifier(s) {
  const split=s.indexOf('_');
  if(prefixes.includes(s.slice(0,split))) {
    const body=s.slice(split+1);
    if(body.length===26 && '01234567'.includes(body[0]) && every(body,crockford)) return true;
  }
  if(s.startsWith('idk_') && s.length===68 && every(s.slice(4),hex)) return true;
  if(s.startsWith('sha256:') && s.length===71 && every(s.slice(7),hex)) return true;
  const parts=s.split('.');
  return parts.length===3 && parts[0]!=='goallatch' && segment(parts[0]) && segment(parts[1]) && verbs.includes(parts[2]);
}
const corpus=['worker','worker-1','calendar','x','w','nemotron-3-nano-30b','\ufeffx','x\ufeff','\u200bx','x\u200b'];
for(const prefix of prefixes) for(const first of crockford) for(const last of crockford+'ILOUabcdefghijklmnopqrstuvwxyz') {
  const s=prefix+'_'+first+'0'.repeat(24)+last;
  corpus.push(s,s+'0',s.slice(0,-1),'a:'+s,s+'\n');
}
for(const tag of ['idk_','idk:','idk','sha256:','sha256_','sha256']) for(const size of [63,64,65]) for(const c of hex+'ABCDEFg') corpus.push(tag+c.repeat(size));
for(const provider of ['pp','p','a'.repeat(32),'a'.repeat(33),'goallatch','goallatch_foo','goallatch1','Goallatch']) for(const resource of ['rr','r','a'.repeat(32),'a'.repeat(33)]) for(const verb of [...verbs,'unknown']) {
  const s=provider+'.'+resource+'.'+verb; corpus.push(s,s+'x',s+'\n');
}
for(const s of corpus) {
  const chars=[...s].map(c=>c.codePointAt(0));
  const base=chars.length>0 && !ws.has(chars[0]) && !ws.has(chars[chars.length-1]) && chars.every(n=>allowed('O',n));
  check('O',s,base && !identifier(s));
}
console.log('ECMA-262 '+process.version+': '+checks+' assertions; all Unicode scalars in three positions/categories, pinned whitespace and exact identifier/near-miss corpus PASS');
'''
subprocess.run(["node", "-e", node, json.dumps(patterns)], check=True)

schema = document("plans/P2-sqlite-schema.md")
migration = ROOT / "crates/serea-storage/migrations/0001_initial.sql"
ddl = migration.read_bytes().decode("utf-8")
assert ddl.endswith("\n")
assert not any("CREATE TABLE schema_migrations" in b for b in blocks(schema, "sql")), "Markdown must not duplicate migration authority"
con = sqlite3.connect(":memory:", isolation_level=None)
con.execute("PRAGMA foreign_keys = ON")
con.executescript(ddl)
inventory = dict(con.execute("SELECT type, count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' GROUP BY type"))
assert inventory == {"table": 10, "trigger": 7, "index": 6}, inventory
assert not any(name in ddl for name in ("leases_generation_matches_step", "event_seq", "TASK_DELETED"))
assert "token" not in [row[1] for row in con.execute("PRAGMA table_info(leases)")]
print("Production migration: 10 tables, 7 triggers, 6 explicit indexes PASS; SHA-256 " + hashlib.sha256(ddl.encode("utf-8")).hexdigest())

# Isolate each exact production column declaration to test its domain without
# unrelated presence/order constraints (acquired_at=MAX cannot have a later expiry).
MIN_MS, MAX_MS = -62167219200000, 253402300799999
expected_instants = {
    "schema_migrations": {"applied_at_ms"},
    "tasks": {"created_at_ms", "updated_at_ms", "deadline_at_ms", "cancelled_at_ms"},
    "task_steps": {"started_at_ms", "completed_at_ms", "lease_expires_at_ms"},
    "side_effect_receipts": {"observed_at_ms"},
    "leases": {"acquired_at_ms", "expires_at_ms", "released_at_ms"},
    "plan_revisions": {"created_at_ms"},
    "task_journal": {"occurred_at_ms"},
}
instant_checks = 0
seen = {}
for table, body in re.findall(r"CREATE TABLE (\w+) \((.*?)\n\) STRICT;", ddl, re.S):
    for match in re.finditer(r"^  (\w+_ms)\s+(INTEGER[^\n]*?)(?:,)?$", body, re.M):
        column, declaration = match.groups()
        assert f"CHECK ({column} BETWEEN {MIN_MS} AND {MAX_MS})" in declaration
        seen.setdefault(table, set()).add(column)
        con.execute("CREATE TABLE instant_probe (" + column + " " + declaration.rstrip(",") + ") STRICT")
        for value in (MIN_MS, MAX_MS, -1, 0, MIN_MS - 1, MAX_MS + 1, -(1 << 63), (1 << 63) - 1, None):
            try:
                con.execute("INSERT INTO instant_probe VALUES (?)", (value,))
                accepted = True
            except sqlite3.IntegrityError:
                accepted = False
            expected = "NOT NULL" not in declaration if value is None else MIN_MS <= value <= MAX_MS
            assert accepted == expected, (table, column, value, accepted)
            con.execute("DELETE FROM instant_probe")
            instant_checks += 1
        con.execute("DROP TABLE instant_probe")
assert seen == expected_instants, seen
assert ddl.count(f"BETWEEN {MIN_MS} AND {MAX_MS}") == 14
print(f"EpochMillis: {instant_checks} exact production-column boundary/NULL probes across all 14 instants PASS (other table invariants tested separately)")

checksum_checks = 0
for checksum in ("sha256:" + "a" * 64, "sha256:" + "0" * 64, "sha256:" + "A" * 64, "sha256:" + "g" * 64, "sha256:" + "a" * 63, "sha256:" + "a" * 65, "sha257:" + "a" * 64, "sha256:" + "a" * 63 + "\x00", "sha256:" + "a" * 64 + "\x00hidden"):
    try:
        con.execute("INSERT INTO schema_migrations VALUES (1, '0001_initial', ?, 0)", (checksum,))
        accepted = True
    except sqlite3.IntegrityError:
        accepted = False
    assert accepted == bool(re.fullmatch(r"sha256:[0-9a-f]{64}", checksum))
    con.execute("DELETE FROM schema_migrations")
    checksum_checks += 1
print(f"Migration checksum grammar: {checksum_checks} positive/negative probes PASS")

# STRICT applies affinity first; integer-to-TEXT is a legal lossless conversion.
con.execute("CREATE TABLE affinity_probe (text_value TEXT, integer_value INTEGER) STRICT")
con.execute("INSERT INTO affinity_probe VALUES (123, 0)")
assert con.execute("SELECT text_value, typeof(text_value) FROM affinity_probe").fetchone() == ("123", "text")
for sql in ("INSERT INTO affinity_probe VALUES (X'01', 0)", "INSERT INTO affinity_probe VALUES ('ok', 'not-an-integer')"):
    try:
        con.execute(sql)
        raise AssertionError("STRICT accepted wrong type")
    except sqlite3.IntegrityError:
        pass
con.execute("DROP TABLE affinity_probe")
assert con.execute("PRAGMA wal_checkpoint(TRUNCATE)").fetchone() == (0, -1, -1)
print("SQLite semantics: integer→TEXT accepted; BLOB→TEXT/nonnumeric TEXT→INTEGER refused; memory checkpoint (0, -1, -1) PASS")
TASK = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA"
STEP = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF"
DIGEST = "sha256:" + "a" * 64
con.execute("INSERT INTO tasks (task_id,kind,title,state,origin_kind,data_class_rank,policy_class_rank,created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,max_attempts_per_step) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)", (TASK,"USER_REQUEST","probe","READY","USER",0,0,0,0,3,3,3))
errors = dict(error_kind="PROVIDER_ERROR", error_code="UPSTREAM_5XX", error_message="probe", error_retryable=0, error_host_action="RETRY", error_details="{}")
statuses = ["PLANNED","LEASED","EXECUTING","WAITING","SUCCEEDED","FAILED","RECONCILED_ABSENT"]
kinds = ["CAPABILITY","MODEL_TURN","WAIT_APPROVAL","WAIT_USER","WAIT_SCHEDULE","VERIFY","NOTIFY","DELEGATE"]

def step(status, kind="MODEL_TURN"):
    row = dict(step_id=STEP, task_id=TASK, sequence=0, kind=kind, status=status, input_digest=DIGEST)
    if kind in ("CAPABILITY","DELEGATE","VERIFY"):
        row.update(provider_id="calendar", capability_id="calendar.events.list", capability_version="1.2.0", idempotency_key="idk_"+"a"*64)
    if status != "PLANNED": row.update(attempt=1, lease_generation=1)
    if status in ("LEASED","EXECUTING"): row.update(lease_owner="worker-a", lease_expires_at_ms=10)
    if status not in ("PLANNED","LEASED"): row.update(started_at_ms=1)
    if status in ("SUCCEEDED","FAILED","RECONCILED_ABSENT"): row.update(completed_at_ms=2)
    if status == "SUCCEEDED": row.update(result_digest=DIGEST)
    if status == "FAILED": row.update({k:v for k,v in errors.items() if k != "error_details"})
    return row

checks = 0

def insert(row):
    cols = ",".join(row)
    con.execute("INSERT INTO task_steps ("+cols+") VALUES ("+",".join(":"+k for k in row)+")", row)

def probe(row, accepted):
    global checks
    con.execute("SAVEPOINT probe")
    try:
        try: insert(row); actual = True
        except sqlite3.IntegrityError: actual = False
        assert actual == accepted, (row, accepted)
        checks += 1
    finally:
        con.execute("ROLLBACK TO probe"); con.execute("RELEASE probe")

for kind, status in itertools.product(kinds, statuses):
    probe(step(status,kind), status!="WAITING" or kind in ("WAIT_APPROVAL","WAIT_USER","WAIT_SCHEDULE"))
# All nonempty error subsets outside FAILED, not just each column independently.
for status in statuses:
    for mask in range(64):
        row = step(status, "WAIT_USER" if status=="WAITING" else "MODEL_TURN")
        for k in errors: row.pop(k,None)
        row.update({k:v for i,(k,v) in enumerate(errors.items()) if mask & (1<<i)})
        probe(row, mask & 31 == 31 if status=="FAILED" else mask==0)
for generation in [-1,0,1,4294967295,4294967296]:
    probe(dict(step("EXECUTING"),lease_generation=generation),1<=generation<=4294967295)
# Non-authoritative explanatory lease snippets get bounded u32-generation parity
# only. They are not migrations; the full schema above comes only from production.
adr = document("decisions/ADR-0024-lease-fencing-and-commit-under-lease.md")
lease_blocks = [b[b.index("CREATE TABLE leases"):b.index(") STRICT;",b.index("CREATE TABLE leases"))+10] for b in [ddl]+blocks(schema,"sql")+blocks(adr,"sql") if "CREATE TABLE leases" in b]
for block in lease_blocks:
    db = sqlite3.connect(":memory:")
    db.executescript("CREATE TABLE task_steps(step_id TEXT PRIMARY KEY);"+block)
    for gen in [-1,0,1,4294967295,4294967296]:
        try:
            db.execute("INSERT INTO leases VALUES ('step','worker',?,0,10,NULL)",(gen,)); accepted=True
        except sqlite3.IntegrityError: accepted=False
        assert accepted == (1<=gen<=4294967295)
        db.rollback(); checks+=1
    db.close()

outcome = next(b for b in blocks(adr,"sql") if "SET status = 'SUCCEEDED'" in b)
release = next(b for b in blocks(adr,"sql") if b.startswith("UPDATE leases SET released_at_ms"))
renew = next(b for b in blocks(adr,"sql") if b.startswith("UPDATE leases SET expires_at_ms"))
acquire = next(b for b in blocks(adr,"sql") if "INSERT INTO leases" in b)
for scenario, expected in [("live",1),("expired",1),("released",0),("reclaimed",0),("wrong-owner",0),("missing",0)]:
    con.execute("SAVEPOINT fence")
    insert(step("EXECUTING"))
    if scenario!="missing":
        con.execute("INSERT INTO leases VALUES (?,?,?,?,?,NULL)",(STEP,"worker-b" if scenario=="wrong-owner" else "worker-a",2 if scenario=="reclaimed" else 1,0,10))
    if scenario=="released":
        assert con.execute(release,(20,STEP,"worker-a",1)).rowcount==1
        assert con.execute(release,(20,STEP,"worker-a",1)).rowcount==0
    if scenario in ("expired","released"):

        assert con.execute(renew,(30,STEP,"worker-a",1,20)).rowcount==0
    p = dict(digest=DIGEST,now_ms=20 if scenario=="expired" else 5,step_id=STEP,task_id=TASK,generation=1,owner="worker-a")
    assert con.execute(outcome,p).rowcount==expected,scenario
    assert con.execute("SELECT status,lease_generation FROM task_steps").fetchone()==("SUCCEEDED" if expected else "EXECUTING",1)
    con.execute("ROLLBACK TO fence"); con.execute("RELEASE fence"); checks+=1
con.execute("SAVEPOINT overflow")
insert(dict(step("EXECUTING"),lease_generation=4294967295))
con.execute("INSERT INTO leases VALUES (?,?,?,?,?,NULL)",(STEP,"worker-a",4294967295,0,10))
before = (con.execute("SELECT * FROM leases").fetchall(),con.execute("SELECT * FROM task_steps").fetchall())
try:
    con.execute(acquire,dict(step_id=STEP,owner="worker-b",now_ms=20,expires_at_ms=30))
    raise AssertionError("generation overflow accepted")
except sqlite3.IntegrityError: pass
assert before==(con.execute("SELECT * FROM leases").fetchall(),con.execute("SELECT * FROM task_steps").fetchall())
con.execute("ROLLBACK TO overflow"); con.execute("RELEASE overflow"); checks+=1
print("SQLite "+sqlite3.sqlite_version+": "+str(checks)+" probes against production migration; kind/status (51 legal cells), all error subsets, bounded explanatory u32 parity, outcome/release/expiry and overflow PASS")

# Construct all documented legal task pairs, without claiming SQL enforces the
# engine transition graph or that the historical 69-check harness was rerun.
design = document("plans/P2-storage-task-engine.md")
transition_block = next(b for b in blocks(design, "rust") if "fn legal_task_transition" in b)
pairs = re.findall(r"\(([A-Z][a-zA-Z]+), ([A-Z][a-zA-Z]+)\)", transition_block)
assert len(pairs) == 37
wire = lambda state: re.sub(r"([a-z])([A-Z])", r"\1_\2", state).upper()
def task_state(state):
    cancelled = state == "CANCELLED"
    con.execute("UPDATE tasks SET state=?, blocked_reason=?, failure_reason=?, cancelled_at_ms=?, cancelled_by=? WHERE task_id=?",
                (state, "PROBE" if state == "BLOCKED" else None, "PROBE" if state == "FAILED" else None, 0 if cancelled else None, "USER" if cancelled else None, TASK))
for source, destination in pairs:
    con.execute("SAVEPOINT task_pair")
    try:
        task_state(wire(source))
        task_state(wire(destination))
        assert con.execute("SELECT state FROM tasks WHERE task_id=?", (TASK,)).fetchone() == (wire(destination),)
    finally:
        con.execute("ROLLBACK TO task_pair")
        con.execute("RELEASE task_pair")
print("Positive task constructibility: 37/37 documented legal transitions PASS against production migration (not engine runtime validation)")
con.close()

# Reconstruct literal published vectors independently; framing names AND values.
canonical = document("decisions/ADR-0019-canonical-json-and-idempotency-preimage.md")
def scj(value):
    return json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(",",":")).replace("\x7f",r"\u007f").encode()
def key(cap,version,args,task=TASK):
    fields = [("task_id",task.encode()),("step_id",STEP.encode()),("capability_id",cap.encode()),("capability_version",version.encode()),("arguments_canonical",args)]
    lp = lambda x: struct.pack(">Q",len(x))+x
    pre = b"serea.idempotency.v1\0"+bytes([len(fields)])+b"".join(lp(n.encode())+lp(v) for n,v in fields)
    return "idk_"+hashlib.sha256(pre).hexdigest(),len(pre)
vector_checks=0
for line in canonical.splitlines():
    cells=[c.strip() for c in line.split("|")[1:-1]]
    if len(cells)==4 and cells[0].isdigit() and cells[-1].startswith("`sha256:"):
        actual=scj(json.loads(cells[1].strip("`")))
        assert actual.decode()==cells[2].strip("`")
        assert "sha256:"+hashlib.sha256(actual).hexdigest()==cells[3].strip("`")
        vector_checks+=1
    if len(cells)==5 and cells[-1].startswith("`idk_"):
        args=cells[3].split("`,",1)[0].strip("`")
        task=TASK[:-1]+"B" if "task_id" in cells[3] else TASK
        actual,length=key(cells[1].strip("`"),cells[2].strip("`"),scj(json.loads(args)),task)
        assert actual==cells[4].strip("`"),cells
        if cells[0]=="1": assert length==282
        vector_checks+=1
    if len(cells)==4 and cells[-1].startswith("`idk_"):
        actual,length=key("pp.rr.list",cells[1].strip("`"),cells[2].strip("`").encode())
        assert actual==cells[3].strip("`"),cells
        assert length==(244 if cells[0]=="Generic scalar framing" else 250)
        vector_checks+=1
assert vector_checks==21,vector_checks
print("SCJ-1/IDK-1: 21 published vectors reconstructed PASS; vector-1 282 bytes; raw A/B private framing only, valid-ID scalar/object lengths pinned")
