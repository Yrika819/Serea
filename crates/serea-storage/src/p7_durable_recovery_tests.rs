//! Test-only authenticated ledger and child-process restart experiments.
//! These files live under disposable temp roots. The keyring is synthetic and
//! does not establish a production authority or independent rollback domain.
use crate::Store;
use rusqlite::params;
use serea_protocol::{Clock, EpochMillis, ProtocolError};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

const OWNER: &str = "own_00000000000000000000000000000071";
const MEMORY: &str = "synthetic-child-forget-target";
static NEXT: AtomicU64 = AtomicU64::new(1);

struct FixedClock;
impl Clock for FixedClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(2000)
    }
}

struct TempRoot(PathBuf);
impl TempRoot {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "serea-p7-auth-ledger-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn db(&self) -> PathBuf {
        self.0.join("primary.sqlite")
    }
    fn ledger(&self) -> PathBuf {
        self.0.join("independent-ledger.test")
    }
}
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Default)]
struct TestKeys(BTreeMap<u16, [u8; 32]>);
impl TestKeys {
    fn generated(seed: &[u8]) -> Self {
        // Deterministic generated synthetic material makes failures reproducible.
        // It is never sourced from a credential store or operating-system key.
        let mut keys = BTreeMap::new();
        keys.insert(
            1,
            Sha256::digest([b"SEREA-P7-TEST-KEY-V1\0".as_slice(), seed].concat()).into(),
        );
        Self(keys)
    }
    fn rotate(&mut self, new_version: u16, seed: &[u8]) {
        let key = Sha256::digest([b"SEREA-P7-TEST-KEY-V1\0".as_slice(), seed].concat()).into();
        self.0.insert(new_version, key);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Record {
    sequence: u64,
    key_version: u16,
    owner: String,
    generation: u64,
    operation: String,
    kind: RecordKind,
    chunk: u16,
    chunk_count: u16,
    memory_ids: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordKind {
    Begin = 1,
    Chunk = 2,
    Applied = 3,
    Finalized = 4,
}
impl TryFrom<u8> for RecordKind {
    type Error = LedgerError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Begin),
            2 => Ok(Self::Chunk),
            3 => Ok(Self::Applied),
            4 => Ok(Self::Finalized),
            _ => Err(LedgerError::Invalid),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LedgerError {
    Invalid,
    MissingKey,
    Io,
    TooLarge,
}

const MAGIC: &[u8; 4] = b"S7LG";
const FORMAT: u16 = 1;
const MAC_LEN: usize = 32;
const MAX_FRAME: usize = 1024;
const MAX_LEDGER_FRAMES: usize = 10_000;

fn put_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), LedgerError> {
    let len = u16::try_from(value.len()).map_err(|_| LedgerError::TooLarge)?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(value);
    Ok(())
}
fn take_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, LedgerError> {
    let end = cursor.checked_add(2).ok_or(LedgerError::Invalid)?;
    let b: [u8; 2] = bytes
        .get(*cursor..end)
        .ok_or(LedgerError::Invalid)?
        .try_into()
        .map_err(|_| LedgerError::Invalid)?;
    *cursor = end;
    Ok(u16::from_be_bytes(b))
}
fn take_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, LedgerError> {
    let end = cursor.checked_add(8).ok_or(LedgerError::Invalid)?;
    let b: [u8; 8] = bytes
        .get(*cursor..end)
        .ok_or(LedgerError::Invalid)?
        .try_into()
        .map_err(|_| LedgerError::Invalid)?;
    *cursor = end;
    Ok(u64::from_be_bytes(b))
}
fn take_bytes<'a>(bytes: &'a [u8], cursor: &mut usize) -> Result<&'a [u8], LedgerError> {
    let len = usize::from(take_u16(bytes, cursor)?);
    let end = cursor.checked_add(len).ok_or(LedgerError::Invalid)?;
    let value = bytes.get(*cursor..end).ok_or(LedgerError::Invalid)?;
    *cursor = end;
    Ok(value)
}
fn encode_payload(record: &Record) -> Result<Vec<u8>, LedgerError> {
    if record.sequence == 0
        || record.generation == 0
        || record.owner.is_empty()
        || record.operation.is_empty()
        || record.memory_ids.len() > 100
        || record.chunk_count == 0
        || record.chunk >= record.chunk_count
    {
        return Err(LedgerError::TooLarge);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&record.sequence.to_be_bytes());
    out.extend_from_slice(&record.generation.to_be_bytes());
    out.push(record.kind as u8);
    out.extend_from_slice(&record.chunk.to_be_bytes());
    out.extend_from_slice(&record.chunk_count.to_be_bytes());
    put_bytes(&mut out, record.owner.as_bytes())?;
    put_bytes(&mut out, record.operation.as_bytes())?;
    out.extend_from_slice(&(record.memory_ids.len() as u16).to_be_bytes());
    for memory_id in &record.memory_ids {
        put_bytes(&mut out, memory_id.as_bytes())?;
    }
    Ok(out)
}
fn decode_payload(payload: &[u8], key_version: u16) -> Result<Record, LedgerError> {
    let mut cursor = 0;
    let sequence = take_u64(payload, &mut cursor)?;
    let generation = take_u64(payload, &mut cursor)?;
    let kind = RecordKind::try_from(*payload.get(cursor).ok_or(LedgerError::Invalid)?)?;
    cursor += 1;
    let chunk = take_u16(payload, &mut cursor)?;
    let chunk_count = take_u16(payload, &mut cursor)?;
    let owner = std::str::from_utf8(take_bytes(payload, &mut cursor)?)
        .map_err(|_| LedgerError::Invalid)?
        .to_owned();
    let operation = std::str::from_utf8(take_bytes(payload, &mut cursor)?)
        .map_err(|_| LedgerError::Invalid)?
        .to_owned();
    let count = usize::from(take_u16(payload, &mut cursor)?);
    if count > 100 || chunk_count == 0 || chunk >= chunk_count {
        return Err(LedgerError::Invalid);
    }
    let mut memory_ids = Vec::with_capacity(count);
    for _ in 0..count {
        memory_ids.push(
            std::str::from_utf8(take_bytes(payload, &mut cursor)?)
                .map_err(|_| LedgerError::Invalid)?
                .to_owned(),
        );
    }
    if cursor != payload.len()
        || sequence == 0
        || generation == 0
        || owner.is_empty()
        || operation.is_empty()
    {
        return Err(LedgerError::Invalid);
    }
    Ok(Record {
        sequence,
        key_version,
        owner,
        generation,
        operation,
        kind,
        chunk,
        chunk_count,
        memory_ids,
    })
}
fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut inner_pad = [0x36_u8; 64];
    let mut outer_pad = [0x5c_u8; 64];
    for (i, b) in key.iter().take(64).enumerate() {
        inner_pad[i] ^= b;
        outer_pad[i] ^= b;
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let inner_hash = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_hash);
    outer.finalize().into()
}
fn encode_record(record: &Record, keys: &TestKeys) -> Result<Vec<u8>, LedgerError> {
    let key = keys
        .0
        .get(&record.key_version)
        .ok_or(LedgerError::MissingKey)?;
    let payload = encode_payload(record)?;
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT.to_be_bytes());
    out.extend_from_slice(&record.key_version.to_be_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&payload);
    let mac = hmac_sha256(key, &out);
    out.extend_from_slice(&mac);
    if out.len() > MAX_FRAME {
        return Err(LedgerError::TooLarge);
    }
    Ok(out)
}
fn parse_records(bytes: &[u8], keys: &TestKeys) -> Result<Vec<Record>, LedgerError> {
    let mut cursor = 0;
    let mut out = Vec::new();
    let mut expected_sequence = 1_u64;
    while cursor < bytes.len() {
        if out.len() >= MAX_LEDGER_FRAMES {
            return Err(LedgerError::TooLarge);
        }
        let start = cursor;
        let header_end = cursor.checked_add(12).ok_or(LedgerError::Invalid)?;
        let header = bytes.get(cursor..header_end).ok_or(LedgerError::Invalid)?;
        if &header[..4] != MAGIC {
            return Err(LedgerError::Invalid);
        }
        let format = u16::from_be_bytes(header[4..6].try_into().map_err(|_| LedgerError::Invalid)?);
        if format != FORMAT {
            return Err(LedgerError::Invalid);
        }
        let key_version =
            u16::from_be_bytes(header[6..8].try_into().map_err(|_| LedgerError::Invalid)?);
        let payload_len =
            u32::from_be_bytes(header[8..12].try_into().map_err(|_| LedgerError::Invalid)?)
                as usize;
        if payload_len + 12 + MAC_LEN > MAX_FRAME {
            return Err(LedgerError::TooLarge);
        }
        cursor = header_end;
        let payload_end = cursor
            .checked_add(payload_len)
            .ok_or(LedgerError::Invalid)?;
        let payload = bytes.get(cursor..payload_end).ok_or(LedgerError::Invalid)?;
        cursor = payload_end;
        let mac_end = cursor.checked_add(MAC_LEN).ok_or(LedgerError::Invalid)?;
        let provided = bytes.get(cursor..mac_end).ok_or(LedgerError::Invalid)?;
        let key = keys.0.get(&key_version).ok_or(LedgerError::MissingKey)?;
        let expected = hmac_sha256(key, bytes.get(start..cursor).ok_or(LedgerError::Invalid)?);
        if !constant_time_eq(provided, &expected) {
            return Err(LedgerError::Invalid);
        }
        cursor = mac_end;
        let record = decode_payload(payload, key_version)?;
        if record.sequence != expected_sequence {
            return Err(LedgerError::Invalid);
        }
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or(LedgerError::Invalid)?;
        out.push(record);
    }
    let mut last_generation = 0_u64;
    let mut last_owner = String::new();
    let mut last_operation = String::new();
    for (index, record) in out.iter().enumerate() {
        if record.generation < last_generation
            || (record.generation == last_generation
                && index > 0
                && (record.owner != last_owner || record.operation != last_operation))
        {
            return Err(LedgerError::Invalid);
        }
        last_generation = record.generation;
        last_owner.clone_from(&record.owner);
        last_operation.clone_from(&record.operation);
    }
    Ok(out)
}
fn operation_status(records: &[Record]) -> Result<(u64, Vec<String>, bool, bool), LedgerError> {
    if records.is_empty() {
        return Err(LedgerError::Invalid);
    }
    let begin = records.first().ok_or(LedgerError::Invalid)?;
    if begin.kind != RecordKind::Begin
        || begin.chunk != 0
        || begin.chunk_count == 0
        || !begin.memory_ids.is_empty()
    {
        return Err(LedgerError::Invalid);
    }
    let mut ids = Vec::new();
    let mut next_chunk = 0_u16;
    let mut applied = false;
    let mut finalized = false;
    for record in records.iter().skip(1) {
        if record.owner != begin.owner
            || record.generation != begin.generation
            || record.operation != begin.operation
            || record.chunk_count != begin.chunk_count
        {
            return Err(LedgerError::Invalid);
        }
        match record.kind {
            RecordKind::Chunk if !applied && !finalized && record.chunk == next_chunk => {
                if record.memory_ids.is_empty() {
                    return Err(LedgerError::Invalid);
                }
                ids.extend(record.memory_ids.iter().cloned());
                next_chunk = next_chunk.checked_add(1).ok_or(LedgerError::Invalid)?;
            }
            RecordKind::Applied
                if !applied
                    && !finalized
                    && next_chunk == begin.chunk_count
                    && record.memory_ids.is_empty() =>
            {
                applied = true
            }
            RecordKind::Finalized if applied && !finalized && record.memory_ids.is_empty() => {
                finalized = true
            }
            _ => return Err(LedgerError::Invalid),
        }
    }
    if next_chunk != begin.chunk_count || ids.is_empty() || ids.len() > 100 {
        return Err(LedgerError::Invalid);
    }
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    if sorted.len() != ids.len() {
        return Err(LedgerError::Invalid);
    }
    Ok((begin.generation, ids, applied, finalized))
}
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0_u8, |diff, (x, y)| diff | (x ^ y)) == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForgetStatus {
    Pending,
    Completed,
}
fn forget_status(records: &[Record]) -> Result<ForgetStatus, LedgerError> {
    let (_, _, applied, finalized) = operation_status(records)?;
    if finalized && applied {
        Ok(ForgetStatus::Completed)
    } else {
        Ok(ForgetStatus::Pending)
    }
}

struct AuthLedger {
    path: PathBuf,
    keys: TestKeys,
}
impl AuthLedger {
    fn read(&self) -> Result<Vec<Record>, LedgerError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let mut bytes = Vec::new();
        File::open(&self.path)
            .and_then(|mut f| f.read_to_end(&mut bytes))
            .map_err(|_| LedgerError::Io)?;
        parse_records(&bytes, &self.keys)
    }
    fn append_with<W: Write>(&self, writer: &mut W, record: &Record) -> Result<(), LedgerError> {
        let current = self.read()?;
        let expected = current
            .last()
            .map_or(1, |r| r.sequence.checked_add(1).unwrap_or(0));
        if expected == 0 || record.sequence != expected {
            return Err(LedgerError::Invalid);
        }
        let encoded = encode_record(record, &self.keys)?;
        writer.write_all(&encoded).map_err(|_| LedgerError::Io)?;
        writer.flush().map_err(|_| LedgerError::Io)
    }
    fn append(&self, record: &Record) -> Result<(), LedgerError> {
        let lock_path = self.path.with_extension("writer.lock");
        let lock_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .map_err(|_| LedgerError::Io)?;
        let lock = LedgerWriteLock {
            path: lock_path,
            _file: lock_file,
        };
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|_| LedgerError::Io)?;
        self.append_with(&mut file, record)?;
        file.sync_all().map_err(|_| LedgerError::Io)?;
        drop(lock);
        Ok(())
    }
}
struct LedgerWriteLock {
    path: PathBuf,
    _file: File,
}
impl Drop for LedgerWriteLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Test composition-root gate: authenticate and reconcile the complete local
/// ledger before returning a Store handle usable by the synthetic Memory API.
/// Production Core does not call this prototype and remains Memory-disabled.
fn open_after_recovery(db: &Path, ledger: &AuthLedger) -> Result<Store, LedgerError> {
    let records = ledger.read()?;
    let (generation, memory_ids, applied, finalized) = operation_status(&records)?;
    if records[0].owner != OWNER {
        return Err(LedgerError::Invalid);
    }
    let store = Store::open(db, &FixedClock).map_err(|_| LedgerError::Io)?;
    store.transact(|tx| {
        let current: i64 = tx.inner.query_row("SELECT generation FROM p7_ledger_state WHERE owner_id=?1", [OWNER], |r| r.get(0))?;
        if current < 0 || current as u64 > generation || (current as u64 != generation && (current as u64).checked_add(1) != Some(generation)) { return Err(crate::StoreError::Sqlite); }
        tx.inner.execute("UPDATE p7_ledger_state SET read_state='RECOVERY_REQUIRED' WHERE owner_id=?1", [OWNER])?;
        for memory_id in &memory_ids {
            tx.inner.execute("INSERT INTO p7_memory_tombstones VALUES(?1,?2,?3,?4,2000) ON CONFLICT(owner_id,memory_id) DO NOTHING", params![OWNER,memory_id,records[0].operation,generation as i64])?;
            tx.inner.execute("DELETE FROM p7_memory_items WHERE owner_id=?1 AND memory_id=?2", params![OWNER,memory_id])?;
        }
        if (current as u64) < generation {
            tx.inner.execute("UPDATE p7_ledger_state SET generation=?2 WHERE owner_id=?1", params![OWNER,generation as i64])?;
        }
        Ok(())
    }).map_err(|_| LedgerError::Io)?;
    let next = records
        .last()
        .map_or(1, |r| r.sequence.checked_add(1).unwrap_or(0));
    if !applied {
        ledger.append(&record(
            next,
            RecordKind::Applied,
            records.last().unwrap().key_version,
            vec![],
        ))?;
    }
    if !finalized {
        let next = ledger
            .read()?
            .last()
            .map_or(1, |r| r.sequence.checked_add(1).unwrap_or(0));
        ledger.append(&record(
            next,
            RecordKind::Finalized,
            records.last().unwrap().key_version,
            vec![],
        ))?;
    }
    store
        .transact(|tx| {
            let current: i64 = tx.inner.query_row(
                "SELECT generation FROM p7_ledger_state WHERE owner_id=?1",
                [OWNER],
                |r| r.get(0),
            )?;
            if current != generation as i64 {
                return Err(crate::StoreError::Sqlite);
            }
            tx.inner.execute(
                "UPDATE p7_ledger_state SET read_state='READY' WHERE owner_id=?1",
                [OWNER],
            )?;
            Ok(())
        })
        .map_err(|_| LedgerError::Io)?;
    Ok(store)
}

fn record(sequence: u64, kind: RecordKind, key_version: u16, ids: Vec<String>) -> Record {
    Record {
        sequence,
        key_version,
        owner: OWNER.into(),
        generation: 1,
        operation: "synthetic-forget-1".into(),
        kind,
        chunk: 0,
        chunk_count: 1,
        memory_ids: ids,
    }
}
fn seed(path: &Path) -> Store {
    let store = Store::open(path, &FixedClock).unwrap();
    store.transact(|tx| {
        tx.inner.execute("INSERT INTO p7_owners VALUES(?1,71,'ACTIVE',0)", [OWNER])?;
        tx.inner.execute("INSERT INTO p7_resource_accounting(owner_id) VALUES(?1)", [OWNER])?;
        tx.inner.execute("INSERT INTO p7_ledger_state(owner_id,read_state) VALUES(?1,'READY')", [OWNER])?;
        tx.inner.execute("INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES(?1,?2,0,'WORKING','synthetic process recovery',1,2000000000)", params![MEMORY,OWNER])?;
        tx.inner.execute("INSERT INTO p7_memory_provenance(memory_id,owner_id,source_id,source_class_rank,metadata_bytes,recorded_at_ms) VALUES(?1,?2,'synthetic-source',0,0,1)", params![MEMORY,OWNER])?;
        Ok(())
    }).unwrap();
    store
}
fn visible(store: &Store) -> i64 {
    store
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT count(*) FROM p7_memory_items WHERE memory_id=?1",
                [MEMORY],
                |r| r.get(0),
            )?)
        })
        .unwrap()
}
fn keyring() -> TestKeys {
    TestKeys::generated(b"isolated synthetic test identity")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TestBackupManifest {
    owner: String,
    primary_generation: u64,
    ledger_sequence: u64,
    primary_digest: [u8; 32],
}
fn create_test_manifest(
    db: &Path,
    ledger: &AuthLedger,
    generation: u64,
) -> Result<TestBackupManifest, LedgerError> {
    let records = ledger.read()?;
    Ok(TestBackupManifest {
        owner: OWNER.into(),
        primary_generation: generation,
        ledger_sequence: records.last().map_or(0, |r| r.sequence),
        primary_digest: Sha256::digest(fs::read(db).map_err(|_| LedgerError::Io)?).into(),
    })
}
fn verify_test_manifest(
    manifest: &TestBackupManifest,
    db: &Path,
    ledger: &AuthLedger,
) -> Result<(), LedgerError> {
    let records = ledger.read()?;
    let actual_digest: [u8; 32] = Sha256::digest(fs::read(db).map_err(|_| LedgerError::Io)?).into();
    let actual_generation: i64 = Store::open(db, &FixedClock)
        .map_err(|_| LedgerError::Io)?
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT generation FROM p7_ledger_state WHERE owner_id=?1",
                [OWNER],
                |r| r.get(0),
            )?)
        })
        .map_err(|_| LedgerError::Io)?;
    if manifest.owner != OWNER
        || manifest.primary_digest != actual_digest
        || manifest.primary_generation != actual_generation as u64
        || manifest.ledger_sequence != records.last().map_or(0, |r| r.sequence)
        || records
            .first()
            .is_some_and(|r| r.generation != manifest.primary_generation)
    {
        return Err(LedgerError::Invalid);
    }
    Ok(())
}

#[test]
fn authenticated_frames_enforce_sequence_key_rotation_and_strict_parsing() {
    let known_key = [0x0b_u8; 20];
    let expected = [
        0xb0, 0x34, 0x4c, 0x61, 0xd8, 0xdb, 0x38, 0x53, 0x5c, 0xa8, 0xaf, 0xce, 0xaf, 0x0b, 0xf1,
        0x2b, 0x88, 0x1d, 0xc2, 0x00, 0xc9, 0x83, 0x3d, 0xa7, 0x26, 0xe9, 0x37, 0x6c, 0x2e, 0x32,
        0xcf, 0xf7,
    ];
    assert_eq!(
        hmac_sha256(&known_key, b"Hi There"),
        expected,
        "HMAC-SHA256 RFC 4231 vector"
    );
    let root = TempRoot::new();
    let mut keys = keyring();
    let ledger = AuthLedger {
        path: root.ledger(),
        keys: keys.clone(),
    };
    let mut begin = record(1, RecordKind::Begin, 1, vec![]);
    begin.chunk_count = 2;
    ledger.append(&begin).unwrap();
    keys.rotate(2, b"rotated synthetic test key");
    let rotated = AuthLedger {
        path: root.ledger(),
        keys: keys.clone(),
    };
    let mut second = record(2, RecordKind::Chunk, 1, vec!["synthetic-m1".into()]);
    second.chunk = 0;
    second.chunk_count = 2;
    rotated.append(&second).unwrap();
    let mut third = record(3, RecordKind::Chunk, 2, vec![MEMORY.into()]);
    third.chunk = 1;
    third.chunk_count = 2;
    rotated.append(&third).unwrap();
    assert_eq!(
        operation_status(&rotated.read().unwrap()).unwrap().1.len(),
        2
    );
    assert_eq!(
        forget_status(&rotated.read().unwrap()),
        Ok(ForgetStatus::Pending)
    );
    assert_eq!(
        rotated
            .read()
            .unwrap()
            .iter()
            .map(|r| r.key_version)
            .collect::<Vec<_>>(),
        [1, 1, 2]
    );

    let mut missing = TestKeys::default();
    missing.0.insert(2, keys.0[&2]);
    assert_eq!(
        parse_records(&fs::read(root.ledger()).unwrap(), &missing),
        Err(LedgerError::MissingKey)
    );

    let valid = fs::read(root.ledger()).unwrap();
    let frame_len = |bytes: &[u8], start: usize| {
        let payload_len =
            u32::from_be_bytes(bytes[start + 8..start + 12].try_into().unwrap()) as usize;
        12 + payload_len + MAC_LEN
    };
    let first_len = frame_len(&valid, 0);
    let second_len = frame_len(&valid, first_len);
    let third_start = first_len + second_len;
    assert!(
        parse_records(&valid[..valid.len() - 1], &keys).is_err(),
        "truncated final frame denied"
    );
    let first = &valid[..first_len];
    let second_frame = &valid[first_len..third_start];
    let third_frame = &valid[third_start..];
    let reordered = [first, third_frame, second_frame].concat();
    assert!(
        parse_records(&reordered, &keys).is_err(),
        "reordered sequence denied"
    );
    let duplicate = [first, first].concat();
    assert!(
        parse_records(&duplicate, &keys).is_err(),
        "duplicate sequence denied"
    );
    let mut tampered = valid;
    let last = tampered.len() - MAC_LEN - 1;
    tampered[last] ^= 0x20;
    assert!(
        parse_records(&tampered, &keys).is_err(),
        "MAC tampering denied"
    );

    let mut bad_order = rotated.read().unwrap();
    bad_order.swap(1, 2);
    assert!(
        operation_status(&bad_order).is_err(),
        "out-of-order chunks are rejected"
    );

    let mut too_many = Vec::new();
    for sequence in 1..=MAX_LEDGER_FRAMES as u64 + 1 {
        let mut frame = record(sequence, RecordKind::Begin, 1, vec![]);
        frame.operation = format!("synthetic-op-{sequence}");
        too_many.extend_from_slice(&encode_record(&frame, &keys).unwrap());
    }
    assert_eq!(
        parse_records(&too_many, &keys),
        Err(LedgerError::TooLarge),
        "ledger frame count bound is structural"
    );
}

struct ShortWriter {
    remaining: usize,
}
impl Write for ShortWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "synthetic short write",
            ));
        }
        let count = buf.len().min(self.remaining);
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn short_write_and_wrong_generation_fail_closed_without_completing_operation() {
    let root = TempRoot::new();
    let ledger = AuthLedger {
        path: root.ledger(),
        keys: keyring(),
    };
    let mut short = ShortWriter { remaining: 9 };
    assert_eq!(
        ledger.append_with(&mut short, &record(1, RecordKind::Begin, 1, vec![])),
        Err(LedgerError::Io)
    );
    assert!(
        ledger.read().unwrap().is_empty(),
        "failed writer did not claim a durable record"
    );
    let mut wrong = record(2, RecordKind::Finalized, 1, vec![]);
    wrong.generation = 2;
    assert!(encode_record(&wrong, &ledger.keys).is_ok());
    assert!(
        ledger.append(&wrong).is_err(),
        "sequence cannot skip the missing intent"
    );
}

#[test]
fn append_is_serialized_across_processes_by_create_new_lockfile() {
    if let Some(path) = std::env::var_os("SEREA_P7_LOCK_CHILD") {
        let ledger = AuthLedger {
            path: PathBuf::from(path),
            keys: keyring(),
        };
        std::process::exit(
            if ledger
                .append(&record(1, RecordKind::Begin, 1, vec![]))
                .is_err()
            {
                81
            } else {
                82
            },
        );
    }
    // This validates create_new's cross-process exclusion semantics only. A
    // production lock design still needs stale-lock recovery and OS review.
    let root = TempRoot::new();
    let ledger_path = root.ledger();
    let lock = ledger_path.with_extension("writer.lock");
    let first = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("p7_durable_recovery_tests::append_is_serialized_across_processes_by_create_new_lockfile")
        .env("SEREA_P7_LOCK_CHILD", &ledger_path)
        .status().unwrap();
    assert_eq!(status.code(), Some(81));
    drop(first);
}

#[test]
fn child_process_exit_after_durable_intent_reopens_quarantined_then_recovers() {
    if let Ok(mode) = std::env::var("SEREA_P7_RECOVERY_CHILD") {
        let db = PathBuf::from(std::env::var_os("SEREA_P7_RECOVERY_DB").unwrap());
        let ledger_path = PathBuf::from(std::env::var_os("SEREA_P7_RECOVERY_LEDGER").unwrap());
        let ledger = AuthLedger {
            path: ledger_path,
            keys: keyring(),
        };
        let intent = record(1, RecordKind::Begin, 1, vec![]);
        ledger.append(&intent).unwrap();
        ledger
            .append(&record(2, RecordKind::Chunk, 1, vec![MEMORY.into()]))
            .unwrap();
        if mode == "after-intent" || mode == "hard-kill-after-intent" {
            if mode.starts_with("hard-kill-") {
                println!("CHECKPOINT:AFTER_LEDGER_INTENT");
                std::io::stdout().flush().unwrap();
                let mut release = [0_u8; 1];
                let _ = std::io::stdin().read_exact(&mut release);
            }
            if mode == "after-intent" {
                std::process::exit(71);
            }
        }
        let store = Store::open(&db, &FixedClock).unwrap();
        if mode == "during-primary-transaction" {
            let _: Result<(), crate::StoreError> = store.transact(|tx| {
                tx.inner.execute("UPDATE p7_ledger_state SET read_state='RECOVERY_REQUIRED' WHERE owner_id=?1", [OWNER])?;
                tx.inner.execute("INSERT INTO p7_memory_tombstones VALUES(?1,?2,'synthetic-forget-1',1,2000) ON CONFLICT(owner_id,memory_id) DO NOTHING", params![OWNER,MEMORY])?;
                tx.inner.execute("DELETE FROM p7_memory_items WHERE owner_id=?1 AND memory_id=?2", params![OWNER,MEMORY])?;
                std::process::exit(74);
            });
            std::process::exit(75);
        }
        if mode == "hard-kill-during-primary-transaction" {
            let _: Result<(), crate::StoreError> = store.transact(|tx| {
                tx.inner.execute("UPDATE p7_ledger_state SET read_state='RECOVERY_REQUIRED' WHERE owner_id=?1", [OWNER])?;
                tx.inner.execute("INSERT INTO p7_memory_tombstones VALUES(?1,?2,'synthetic-forget-1',1,2000) ON CONFLICT(owner_id,memory_id) DO NOTHING", params![OWNER,MEMORY])?;
                tx.inner.execute("DELETE FROM p7_memory_items WHERE owner_id=?1 AND memory_id=?2", params![OWNER,MEMORY])?;
                println!("CHECKPOINT:INSIDE_PRIMARY_TRANSACTION");
                std::io::stdout().flush().unwrap();
                let mut release = [0_u8; 1];
                let _ = std::io::stdin().read_exact(&mut release);
                Ok(())
            });
            std::process::exit(76);
        }
        store
            .transact(|tx| {
                tx.inner.execute(
                    "UPDATE p7_ledger_state SET read_state='RECOVERY_REQUIRED' WHERE owner_id=?1",
                    [OWNER],
                )?;
                tx.inner.execute("INSERT INTO p7_memory_tombstones VALUES(?1,?2,'synthetic-forget-1',1,2000) ON CONFLICT(owner_id,memory_id) DO NOTHING", params![OWNER,MEMORY])?;
                tx.inner.execute(
                    "DELETE FROM p7_memory_items WHERE owner_id=?1 AND memory_id=?2",
                    params![OWNER, MEMORY],
                )?;
                tx.inner.execute("UPDATE p7_ledger_state SET generation=1,read_state='RECOVERY_REQUIRED' WHERE owner_id=?1", [OWNER])?;
                Ok(())
            })
            .unwrap();
        if mode == "after-primary-commit" {
            std::process::exit(72);
        }
        if mode == "hard-kill-after-primary-commit" {
            println!("CHECKPOINT:AFTER_PRIMARY_COMMIT");
            std::io::stdout().flush().unwrap();
            let mut release = [0_u8; 1];
            let _ = std::io::stdin().read_exact(&mut release);
            std::process::exit(77);
        }
        std::process::exit(73);
    }

    for (mode, expected_code) in [
        ("after-intent", 71),
        ("during-primary-transaction", 74),
        ("after-primary-commit", 72),
    ] {
        let root = TempRoot::new();
        let seeded = seed(&root.db());
        drop(seeded);
        let status: ExitStatus = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("p7_durable_recovery_tests::child_process_exit_after_durable_intent_reopens_quarantined_then_recovers")
            .arg("--nocapture")
            .env("SEREA_P7_RECOVERY_CHILD", mode)
            .env("SEREA_P7_RECOVERY_DB", root.db())
            .env("SEREA_P7_RECOVERY_LEDGER", root.ledger())
            .status().unwrap();
        assert_eq!(status.code(), Some(expected_code));
        let keys = keyring();
        let ledger = AuthLedger {
            path: root.ledger(),
            keys,
        };
        assert_eq!(
            ledger.read().unwrap().len(),
            2,
            "intent and closure survive process exit"
        );
        let reopened = Store::open(&root.db(), &FixedClock).unwrap();
        let state: String = reopened
            .transact(|tx| {
                Ok(tx.inner.query_row(
                    "SELECT read_state FROM p7_ledger_state WHERE owner_id=?1",
                    [OWNER],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(
            visible(&reopened),
            if mode == "after-primary-commit" { 0 } else { 1 }
        );
        if mode == "after-primary-commit" {
            assert_eq!(state, "RECOVERY_REQUIRED");
        }
        drop(reopened);
        let authorized = open_after_recovery(&root.db(), &ledger)
            .expect("recovery gate runs before Memory handle");
        assert_eq!(visible(&authorized), 0);
        assert_eq!(
            ledger
                .read()
                .unwrap()
                .iter()
                .map(|r| r.kind)
                .collect::<Vec<_>>(),
            [
                RecordKind::Begin,
                RecordKind::Chunk,
                RecordKind::Applied,
                RecordKind::Finalized
            ]
        );
        assert_eq!(
            forget_status(&ledger.read().unwrap()),
            Ok(ForgetStatus::Completed)
        );
        let frame_count = ledger.read().unwrap().len();
        drop(authorized);
        let second_reopen = open_after_recovery(&root.db(), &ledger).unwrap();
        assert_eq!(visible(&second_reopen), 0);
        assert_eq!(
            ledger.read().unwrap().len(),
            frame_count,
            "completed replay is idempotent"
        );
    }
}

#[test]
fn parent_kills_child_at_three_forget_commit_windows_and_recovers_from_durable_files() {
    for (mode, checkpoint, expected_visible) in [
        (
            "hard-kill-after-intent",
            "CHECKPOINT:AFTER_LEDGER_INTENT",
            1,
        ),
        (
            "hard-kill-during-primary-transaction",
            "CHECKPOINT:INSIDE_PRIMARY_TRANSACTION",
            1,
        ),
        (
            "hard-kill-after-primary-commit",
            "CHECKPOINT:AFTER_PRIMARY_COMMIT",
            0,
        ),
    ] {
        let root = TempRoot::new();
        drop(seed(&root.db()));
        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("--exact")
            .arg("p7_durable_recovery_tests::child_process_exit_after_durable_intent_reopens_quarantined_then_recovers")
            .arg("--nocapture")
            .env("SEREA_P7_RECOVERY_CHILD", mode)
            .env("SEREA_P7_RECOVERY_DB", root.db())
            .env("SEREA_P7_RECOVERY_LEDGER", root.ledger())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut output = std::io::BufReader::new(stdout);
        loop {
            let mut line = String::new();
            assert!(
                output.read_line(&mut line).unwrap() > 0,
                "child exited before checkpoint"
            );
            if line.trim() == checkpoint {
                break;
            }
        }
        child.kill().unwrap();
        assert!(
            !child.wait().unwrap().success(),
            "parent must force child termination"
        );

        let ledger = AuthLedger {
            path: root.ledger(),
            keys: keyring(),
        };
        assert_eq!(ledger.read().unwrap().len(), 2);
        let reopened = Store::open(&root.db(), &FixedClock).unwrap();
        assert_eq!(visible(&reopened), expected_visible);
        drop(reopened);
        let recovered = open_after_recovery(&root.db(), &ledger)
            .expect("no Memory handle before reconciliation");
        assert_eq!(visible(&recovered), 0);
        assert_eq!(
            forget_status(&ledger.read().unwrap()),
            Ok(ForgetStatus::Completed)
        );
    }
}

#[test]
fn missing_authority_wrong_generation_and_old_snapshot_deny_before_reads() {
    let root = TempRoot::new();
    let old = root.0.join("old.sqlite");
    let seeded = seed(&root.db());
    seeded.checkpoint_for_close().unwrap();
    drop(seeded);
    fs::copy(root.db(), &old).unwrap();
    let empty = AuthLedger {
        path: root.ledger(),
        keys: keyring(),
    };
    assert!(empty.read().unwrap().is_empty());
    assert!(
        open_after_recovery(&root.db(), &empty).is_err(),
        "missing authority refuses before returning a Memory-capable handle"
    );
    let mut wrong = record(1, RecordKind::Begin, 1, vec![MEMORY.into()]);
    wrong.generation = 99;
    empty.append(&wrong).unwrap();
    fs::copy(old, root.db()).unwrap();
    let restored = Store::open(&root.db(), &FixedClock).unwrap();
    let ledger = empty.read().unwrap();
    let primary_generation: i64 = restored
        .transact(|tx| {
            Ok(tx.inner.query_row(
                "SELECT generation FROM p7_ledger_state WHERE owner_id=?1",
                [OWNER],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_ne!(primary_generation as u64 + 1, ledger[0].generation);
    assert!(
        open_after_recovery(&root.db(), &empty).is_err(),
        "wrong generation remains quarantined"
    );
    let absent_keys = AuthLedger {
        path: root.ledger(),
        keys: TestKeys::default(),
    };
    assert_eq!(absent_keys.read(), Err(LedgerError::MissingKey));
    assert_eq!(
        visible(&restored),
        1,
        "callers must retain startup barrier and not expose this row"
    );
}

#[test]
fn backup_manifest_binds_owner_primary_bytes_generation_and_ledger_position() {
    let root = TempRoot::new();
    let backup_path = root.0.join("backup-primary.sqlite");
    let seeded = seed(&root.db());
    seeded.checkpoint_for_close().unwrap();
    drop(seeded);
    fs::copy(root.db(), &backup_path).unwrap();
    let ledger = AuthLedger {
        path: root.ledger(),
        keys: keyring(),
    };
    let manifest = create_test_manifest(&backup_path, &ledger, 0).unwrap();
    assert!(verify_test_manifest(&manifest, &backup_path, &ledger).is_ok());
    File::create(root.ledger()).unwrap();
    let backup_ledger = root.0.join("backup-ledger.frames");
    fs::copy(root.ledger(), &backup_ledger).unwrap();

    let mut intent = record(1, RecordKind::Begin, 1, vec![]);
    intent.generation = 1;
    ledger.append(&intent).unwrap();
    let mut chunk = record(2, RecordKind::Chunk, 1, vec![MEMORY.into()]);
    chunk.generation = 1;
    ledger.append(&chunk).unwrap();
    assert!(
        verify_test_manifest(&manifest, &backup_path, &ledger).is_err(),
        "old backup manifest cannot reconcile with the current ledger position"
    );

    // Restoring both artifacts restores the same local trust evidence. This
    // setup cannot detect that complete rollback; the test labels the limit.
    let restored_together = AuthLedger {
        path: backup_ledger,
        keys: keyring(),
    };
    assert!(restored_together.read().unwrap().is_empty());
    assert!(
        verify_test_manifest(&manifest, &backup_path, &restored_together).is_ok(),
        "complete rollback of both local artifacts is unobservable to this design"
    );
}

#[test]
fn validly_authenticated_cross_owner_forget_is_refused_before_handle_return() {
    let root = TempRoot::new();
    let seeded = seed(&root.db());
    drop(seeded);
    let ledger = AuthLedger {
        path: root.ledger(),
        keys: keyring(),
    };
    let mut begin = record(1, RecordKind::Begin, 1, vec![]);
    begin.owner = "own_00000000000000000000000000000099".into();
    ledger.append(&begin).unwrap();
    let mut chunk = record(2, RecordKind::Chunk, 1, vec![MEMORY.into()]);
    chunk.owner = begin.owner.clone();
    ledger.append(&chunk).unwrap();
    assert!(open_after_recovery(&root.db(), &ledger).is_err());
}
