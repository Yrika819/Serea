//! Disabled Gate C development contracts for P7 owner identity and ledger frames.
//!
//! This module is behind the explicit `p7-gate-c` feature, which is rejected in
//! optimized builds. It has no Core composition entry point, OS authenticator,
//! SQLite migration, or production key source. Ledger append is single-writer
//! only: callers must provide external cross-process serialization. `sync_all`
//! is the strongest portable std primitive used here and is not a power-loss or
//! directory-entry durability guarantee.

use fs2::FileExt as _;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const MAX_FRAME_BYTES: usize = 1024;
pub const MAX_PAYLOAD_BYTES: usize = 768;
/// Hard synthetic-input ceilings. These are denial-of-service bounds, not
/// retention or product capacity promises. History is never compacted here.
pub const MAX_LEDGER_FRAMES: usize = 1_024;
pub const MAX_LEDGER_BYTES: usize = MAX_FRAME_BYTES * MAX_LEDGER_FRAMES;
pub const MAX_CHECKPOINT_REVISIONS: usize = 1_024;
const MAX_CHECKPOINT_BYTES: u64 = 256;
/// Provisional closure bounds inherited from the recommendation, not Owner-approved limits.
pub const PROVISIONAL_MAX_CLOSURE_TARGETS: u16 = 1_000;
pub const PROVISIONAL_MAX_CLOSURE_CHUNKS: u16 = 500;
pub const PROVISIONAL_MAX_CLOSURE_BYTES: usize =
    PROVISIONAL_MAX_CLOSURE_CHUNKS as usize * MAX_PAYLOAD_BYTES;
const MAGIC: &[u8; 4] = b"S7LG";
const FORMAT: u16 = 1;
const MAC_BYTES: usize = 32;

/// Owner-approved V1 admission and base-retention ceilings. This data does not
/// enable any Memory API and excludes every recommended value not explicitly
/// ratified by the Owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptedV1Bounds {
    pub max_active_items: usize,
    pub max_item_body_utf8_bytes: usize,
    pub max_active_body_bytes: usize,
    pub max_logical_fts_source_bytes: usize,
    pub extraction_starts_per_utc_day: usize,
    pub concurrent_extractions_per_owner: usize,
    pub working_retention_days: u16,
    pub episodic_retention_days: u16,
    pub semantic_retention_days: u16,
    pub preference_retention_days: u16,
}

pub const ACCEPTED_V1_BOUNDS: AcceptedV1Bounds = AcceptedV1Bounds {
    max_active_items: 1_000,
    max_item_body_utf8_bytes: 4_096,
    max_active_body_bytes: 4 * 1024 * 1024,
    max_logical_fts_source_bytes: 8 * 1024 * 1024,
    extraction_starts_per_utc_day: 20,
    concurrent_extractions_per_owner: 1,
    working_retention_days: 7,
    episodic_retention_days: 90,
    semantic_retention_days: 365,
    preference_retention_days: 365,
};

/// Opaque stable local owner identifier. Construction validates syntax but
/// does not confer authentication or admission authority.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OwnerId(String);

impl OwnerId {
    pub fn parse(value: impl Into<String>) -> Result<Self, LedgerError> {
        let value = value.into();
        let suffix = value.strip_prefix("own_").ok_or(LedgerError::Invalid)?;
        if suffix.len() != 32
            || !suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(LedgerError::Invalid);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Host-authentication evidence purpose. Evidence itself is adapter-produced;
/// this crate intentionally has no public constructor for authenticated owners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthPurpose {
    Bootstrap,
    Forget,
    RenewMemory,
    OwnerRecovery,
    TaskAdmission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPlatform {
    MacOs,
    Windows,
    Linux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticationRequest {
    pub purpose: AuthPurpose,
    pub observed_at_ms: i64,
    pub max_evidence_age_ms: u64,
}

/// Platform-neutral host authentication port. No implementation is available
/// yet; host session state and device possession cannot satisfy this port.
pub trait HostAuthenticator: Send + Sync {
    fn platform(&self) -> HostPlatform;
    fn authenticate(&self, request: AuthenticationRequest) -> Result<AuthEvidence, LedgerError>;
}

/// Explicit scaffold for a platform target without a reviewed adapter.
pub struct UnsupportedHostAuthenticator(HostPlatform);

impl UnsupportedHostAuthenticator {
    pub const fn new(platform: HostPlatform) -> Self {
        Self(platform)
    }
}

impl HostAuthenticator for UnsupportedHostAuthenticator {
    fn platform(&self) -> HostPlatform {
        self.0
    }

    fn authenticate(&self, _request: AuthenticationRequest) -> Result<AuthEvidence, LedgerError> {
        Err(LedgerError::Authentication)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthEvidence {
    purpose: AuthPurpose,
    verified_at_ms: i64,
    fresh_until_ms: i64,
    nonce: [u8; 32],
    adapter_version: String,
}

impl AuthEvidence {
    /// Adapter boundary constructor. A future host adapter must own this call;
    /// the module is currently reachable only by explicit development builds.
    #[allow(dead_code)] // Consumed when a reviewed platform adapter is added.
    pub(crate) fn from_verified_adapter(
        purpose: AuthPurpose,
        verified_at_ms: i64,
        fresh_until_ms: i64,
        nonce: [u8; 32],
        adapter_version: String,
    ) -> Result<Self, LedgerError> {
        if verified_at_ms < 0
            || fresh_until_ms <= verified_at_ms
            || adapter_version.is_empty()
            || adapter_version.len() > 64
            || nonce.iter().all(|byte| *byte == 0)
        {
            return Err(LedgerError::Invalid);
        }
        Ok(Self {
            purpose,
            verified_at_ms,
            fresh_until_ms,
            nonce,
            adapter_version,
        })
    }

    pub fn validate_for(
        &self,
        purpose: AuthPurpose,
        observed_at_ms: i64,
    ) -> Result<(), LedgerError> {
        if self.purpose != purpose
            || observed_at_ms < self.verified_at_ms
            || observed_at_ms >= self.fresh_until_ms
        {
            return Err(LedgerError::Authentication);
        }
        Ok(())
    }

    pub fn nonce(&self) -> &[u8; 32] {
        &self.nonce
    }
}

/// Process-local one-use nonce fence for synthetic host-auth evidence. A real
/// adapter must persist/replay-protect challenges across its actual authority
/// boundary; this set alone is not a restart-safe production replay defense.
#[derive(Default)]
pub struct EvidenceReplayGuard(BTreeSet<[u8; 32]>);

impl EvidenceReplayGuard {
    pub fn consume(
        &mut self,
        evidence: &AuthEvidence,
        purpose: AuthPurpose,
        observed_at_ms: i64,
    ) -> Result<(), LedgerError> {
        evidence.validate_for(purpose, observed_at_ms)?;
        if self.0.len() >= 1024 && !self.0.contains(&evidence.nonce) {
            return Err(LedgerError::TooLarge);
        }
        if !self.0.insert(evidence.nonce) {
            return Err(LedgerError::Authentication);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameKind {
    Intent = 1,
    Chunk = 2,
    PrimaryApplied = 3,
    Finalized = 4,
    Reconciled = 5,
}

impl TryFrom<u8> for FrameKind {
    type Error = LedgerError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Intent),
            2 => Ok(Self::Chunk),
            3 => Ok(Self::PrimaryApplied),
            4 => Ok(Self::Finalized),
            5 => Ok(Self::Reconciled),
            _ => Err(LedgerError::Invalid),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerFrame {
    pub sequence: u64,
    pub generation: u64,
    pub key_version: u16,
    pub chunk_index: u16,
    pub chunk_count: u16,
    pub target_count: u16,
    pub closure_digest: [u8; 32],
    pub owner: OwnerId,
    pub operation_id: String,
    pub kind: FrameKind,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerError {
    Invalid,
    MissingKey,
    Authentication,
    Sequence,
    Generation,
    TooLarge,
    Io,
    /// The bytes authenticate but do not match the independently supplied latest checkpoint.
    Stale,
    /// No persisted authority record exists; ordinary reopen must not bootstrap one.
    MissingAuthority,
}

/// Test/development representation of freshness evidence held outside the ledger
/// rollback domain. Possessing a checkpoint alongside the ledger does not make it
/// independent; production checkpoint custody remains unapproved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedLedgerCheckpoint {
    pub revision: u64,
    pub owner: OwnerId,
    pub owner_epoch: u64,
    pub key_version: u16,
    pub final_sequence: u64,
    pub generation: u64,
    pub stream_digest: [u8; 32],
}

/// Synthetic, append-only checkpoint authority. Each revision is immutable and
/// authenticated. The highest revision is authoritative; a torn highest file
/// denies open instead of falling back. This is a test authority interface,
/// not an independently protected production trust anchor.
pub struct PersistentCheckpointStore {
    directory: PathBuf,
    current: TrustedLedgerCheckpoint,
}

impl PersistentCheckpointStore {
    /// Explicit new-store bootstrap. Callers must establish that no prior
    /// namespace exists before using this constructor.
    pub fn bootstrap(
        directory: impl AsRef<Path>,
        mut checkpoint: TrustedLedgerCheckpoint,
        keys: &SyntheticKeyring,
    ) -> Result<Self, LedgerError> {
        let directory = directory.as_ref().to_path_buf();
        if checkpoint.final_sequence != 0
            || checkpoint.generation != 0
            || checkpoint.stream_digest != Sha256::digest([]).as_slice()
        {
            return Err(LedgerError::Invalid);
        }
        if directory.exists() {
            if fs::symlink_metadata(&directory)
                .map_err(|_| LedgerError::Io)?
                .file_type()
                .is_symlink()
            {
                return Err(LedgerError::Invalid);
            }
            let mut entries = fs::read_dir(&directory).map_err(|_| LedgerError::Io)?;
            if entries.next().is_some() {
                return Err(LedgerError::Stale);
            }
        } else {
            fs::create_dir(&directory).map_err(|_| LedgerError::Io)?;
            sync_directory(&directory)?;
        }
        checkpoint.revision = 1;
        persist_checkpoint(&directory, &checkpoint, keys)?;
        let loaded = Self::open_existing(&directory, keys)?;
        if loaded.current != checkpoint {
            return Err(LedgerError::Stale);
        }
        Ok(loaded)
    }

    /// Reopen only existing authority. Missing/empty directories are a denial.
    pub fn open_existing(
        directory: impl AsRef<Path>,
        keys: &SyntheticKeyring,
    ) -> Result<Self, LedgerError> {
        let directory = directory.as_ref().to_path_buf();
        if fs::symlink_metadata(&directory)
            .map_err(|_| LedgerError::MissingAuthority)?
            .file_type()
            .is_symlink()
        {
            return Err(LedgerError::Invalid);
        }
        let entries = fs::read_dir(&directory).map_err(|_| LedgerError::MissingAuthority)?;
        let mut revisions = Vec::new();
        let mut entry_count = 0usize;
        for entry in entries {
            entry_count = entry_count.checked_add(1).ok_or(LedgerError::TooLarge)?;
            if entry_count > MAX_CHECKPOINT_REVISIONS {
                return Err(LedgerError::TooLarge);
            }
            let entry = entry.map_err(|_| LedgerError::Io)?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(LedgerError::Invalid);
            };
            if let Some(suffix) = name
                .strip_prefix("checkpoint-")
                .and_then(|s| s.strip_suffix(".bin"))
            {
                if suffix.len() != 20 || !suffix.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(LedgerError::Invalid);
                }
                let revision = suffix.parse::<u64>().map_err(|_| LedgerError::Invalid)?;
                let metadata = fs::symlink_metadata(entry.path()).map_err(|_| LedgerError::Io)?;
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    return Err(LedgerError::Invalid);
                }
                if metadata.len() > MAX_CHECKPOINT_BYTES {
                    return Err(LedgerError::TooLarge);
                }
                revisions.push((revision, entry.path()));
            } else {
                return Err(LedgerError::Invalid);
            }
        }
        if revisions.len() > MAX_CHECKPOINT_REVISIONS {
            return Err(LedgerError::TooLarge);
        }
        revisions.sort_by_key(|(revision, _)| *revision);
        let Some((revision, _path)) = revisions.last() else {
            return Err(LedgerError::MissingAuthority);
        };
        let mut prior: Option<TrustedLedgerCheckpoint> = None;
        for (expected, (revision, path)) in (1_u64..).zip(revisions.iter()) {
            if *revision != expected {
                return Err(LedgerError::Stale);
            }
            let metadata = fs::symlink_metadata(path).map_err(|_| LedgerError::Io)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(LedgerError::Invalid);
            }
            let bytes = fs::read(path).map_err(|_| LedgerError::Io)?;
            let current = decode_checkpoint(&bytes, keys)?;
            if current.revision != *revision
                || prior.as_ref().is_some_and(|old| {
                    current.owner != old.owner
                        || current.owner_epoch != old.owner_epoch
                        || current.final_sequence < old.final_sequence
                        || current.generation < old.generation
                })
            {
                return Err(LedgerError::Stale);
            }
            prior = Some(current);
        }
        let checkpoint = prior.ok_or(LedgerError::MissingAuthority)?;
        if checkpoint.revision != *revision {
            return Err(LedgerError::Stale);
        }
        Ok(Self {
            directory,
            current: checkpoint,
        })
    }

    pub fn current(&self) -> &TrustedLedgerCheckpoint {
        &self.current
    }

    /// Refresh only while the caller holds the namespace's cross-process lock.
    pub fn refresh(&mut self, keys: &SyntheticKeyring) -> Result<(), LedgerError> {
        *self = Self::open_existing(&self.directory, keys)?;
        Ok(())
    }

    /// Persist the next immutable revision, then reopen and authenticate it.
    pub fn advance(
        &mut self,
        mut next: TrustedLedgerCheckpoint,
        keys: &SyntheticKeyring,
    ) -> Result<(), LedgerError> {
        if next.owner != self.current.owner
            || next.owner_epoch != self.current.owner_epoch
            || next.final_sequence < self.current.final_sequence
            || next.generation < self.current.generation
            || next.key_version == 0
        {
            return Err(LedgerError::Stale);
        }
        next.revision = self
            .current
            .revision
            .checked_add(1)
            .ok_or(LedgerError::Generation)?;
        if next.revision > MAX_CHECKPOINT_REVISIONS as u64 {
            return Err(LedgerError::Stale);
        }
        persist_checkpoint(&self.directory, &next, keys)?;
        let reopened = Self::open_existing(&self.directory, keys)?;
        if reopened.current != next {
            return Err(LedgerError::Stale);
        }
        self.current = reopened.current;
        Ok(())
    }
}

const CHECKPOINT_MAGIC: &[u8; 4] = b"S7CP";
const CHECKPOINT_FORMAT: u16 = 1;
const CHECKPOINT_MAC_BYTES: usize = 32;

#[cfg(test)]
fn crash_test_process_at(point: &str) {
    if std::env::var("SEREA_P7_CRASH_AT").is_ok_and(|requested| requested == point) {
        std::process::exit(73);
    }
}

#[cfg(not(test))]
fn crash_test_process_at(_point: &str) {}

#[cfg(test)]
fn crash_during_checkpoint_write(file: &mut File, bytes: &[u8]) -> Result<(), LedgerError> {
    if std::env::var("SEREA_P7_CRASH_AT")
        .is_ok_and(|requested| requested == "during-checkpoint-write")
    {
        file.write_all(&bytes[..bytes.len() / 2])
            .map_err(|_| LedgerError::Io)?;
        file.sync_all().map_err(|_| LedgerError::Io)?;
        std::process::exit(73);
    }
    Ok(())
}

#[cfg(not(test))]
fn crash_during_checkpoint_write(_file: &mut File, _bytes: &[u8]) -> Result<(), LedgerError> {
    Ok(())
}

#[cfg(test)]
fn crash_during_ledger_append(file: &mut &File, bytes: &[u8]) -> Result<(), LedgerError> {
    if std::env::var("SEREA_P7_CRASH_AT").is_ok_and(|requested| requested == "during-ledger-append")
    {
        file.write_all(&bytes[..bytes.len() / 2])
            .map_err(|_| LedgerError::Io)?;
        file.sync_all().map_err(|_| LedgerError::Io)?;
        std::process::exit(73);
    }
    Ok(())
}

#[cfg(not(test))]
fn crash_during_ledger_append(_file: &mut &File, _bytes: &[u8]) -> Result<(), LedgerError> {
    Ok(())
}

fn checkpoint_bytes(
    checkpoint: &TrustedLedgerCheckpoint,
    keys: &SyntheticKeyring,
) -> Result<Vec<u8>, LedgerError> {
    if checkpoint.revision == 0
        || checkpoint.key_version == 0
        || checkpoint.owner.as_str().len() > 64
    {
        return Err(LedgerError::Invalid);
    }
    let mut body = Vec::with_capacity(128);
    body.extend_from_slice(CHECKPOINT_MAGIC);
    body.extend_from_slice(&CHECKPOINT_FORMAT.to_be_bytes());
    body.extend_from_slice(&checkpoint.revision.to_be_bytes());
    body.extend_from_slice(&checkpoint.owner_epoch.to_be_bytes());
    body.extend_from_slice(&checkpoint.key_version.to_be_bytes());
    body.extend_from_slice(&checkpoint.final_sequence.to_be_bytes());
    body.extend_from_slice(&checkpoint.generation.to_be_bytes());
    body.extend_from_slice(&checkpoint.stream_digest);
    body.push(checkpoint.owner.as_str().len() as u8);
    body.extend_from_slice(checkpoint.owner.as_str().as_bytes());
    let mac = hmac_sha256(keys.key(checkpoint.key_version)?, &body);
    body.extend_from_slice(&mac);
    if body.len() > 256 {
        return Err(LedgerError::TooLarge);
    }
    Ok(body)
}

fn decode_checkpoint(
    bytes: &[u8],
    keys: &SyntheticKeyring,
) -> Result<TrustedLedgerCheckpoint, LedgerError> {
    if bytes.len() < 4 + 2 + 8 * 4 + 2 + 32 + 1 + 6 + CHECKPOINT_MAC_BYTES || bytes.len() > 256 {
        return Err(LedgerError::Invalid);
    }
    let body_len = bytes.len() - CHECKPOINT_MAC_BYTES;
    let (body, supplied_mac) = bytes.split_at(body_len);
    let mut cursor = 0;
    if take(body, &mut cursor, 4)? != CHECKPOINT_MAGIC {
        return Err(LedgerError::Invalid);
    }
    let format = u16::from_be_bytes(
        take(body, &mut cursor, 2)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?,
    );
    if format != CHECKPOINT_FORMAT {
        return Err(LedgerError::Invalid);
    }
    let revision = u64::from_be_bytes(
        take(body, &mut cursor, 8)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?,
    );
    let owner_epoch = u64::from_be_bytes(
        take(body, &mut cursor, 8)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?,
    );
    let key_version = u16::from_be_bytes(
        take(body, &mut cursor, 2)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?,
    );
    let final_sequence = u64::from_be_bytes(
        take(body, &mut cursor, 8)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?,
    );
    let generation = u64::from_be_bytes(
        take(body, &mut cursor, 8)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?,
    );
    let stream_digest = take(body, &mut cursor, 32)?
        .try_into()
        .map_err(|_| LedgerError::Invalid)?;
    let owner_len = *take(body, &mut cursor, 1)?
        .first()
        .ok_or(LedgerError::Invalid)? as usize;
    if owner_len == 0 || owner_len > 64 {
        return Err(LedgerError::Invalid);
    }
    let owner = std::str::from_utf8(take(body, &mut cursor, owner_len)?)
        .map_err(|_| LedgerError::Invalid)?;
    if cursor != body.len() {
        return Err(LedgerError::Invalid);
    }
    let expected = hmac_sha256(keys.key(key_version)?, body);
    if !constant_time_eq(&expected, supplied_mac) {
        return Err(LedgerError::Authentication);
    }
    Ok(TrustedLedgerCheckpoint {
        owner: OwnerId::parse(owner.to_owned())?,
        owner_epoch,
        key_version,
        final_sequence,
        generation,
        stream_digest,
        revision,
    })
}

fn persist_checkpoint(
    directory: &Path,
    checkpoint: &TrustedLedgerCheckpoint,
    keys: &SyntheticKeyring,
) -> Result<(), LedgerError> {
    let bytes = checkpoint_bytes(checkpoint, keys)?;
    let path = directory.join(format!("checkpoint-{:020}.bin", checkpoint.revision));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| LedgerError::Io)?;
    crash_test_process_at("during-checkpoint-create");
    crash_during_checkpoint_write(&mut file, &bytes)?;
    file.write_all(&bytes).map_err(|_| LedgerError::Io)?;
    file.sync_all().map_err(|_| LedgerError::Io)?;
    crash_test_process_at("after-checkpoint-file-sync");
    drop(file);
    sync_directory(directory)?;
    Ok(())
}

#[cfg(unix)]
fn sync_directory(directory: &Path) -> Result<(), LedgerError> {
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|_| LedgerError::Io)
}

#[cfg(not(unix))]
fn sync_directory(_directory: &Path) -> Result<(), LedgerError> {
    // Windows directory-entry durability is not asserted by this synthetic adapter.
    Ok(())
}

/// Authenticate frames and require exact agreement with a separately supplied
/// freshness checkpoint and primary-database generation. `None` is a denial.
pub fn verify_stream_at_checkpoint(
    bytes: &[u8],
    keys: &SyntheticKeyring,
    checkpoint: Option<&TrustedLedgerCheckpoint>,
    owner: &OwnerId,
    owner_epoch: u64,
    primary_generation: u64,
) -> Result<Vec<LedgerFrame>, LedgerError> {
    let checkpoint = checkpoint.ok_or(LedgerError::Authentication)?;
    let frames = verify_stream(bytes, keys)?;
    let mut operation_terminal = BTreeMap::<&str, FrameKind>::new();
    for frame in &frames {
        operation_terminal.insert(&frame.operation_id, frame.kind);
    }
    if operation_terminal
        .values()
        .any(|kind| *kind != FrameKind::Reconciled)
    {
        return Err(LedgerError::Stale);
    }
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    let (final_sequence, generation, key_version, ledger_owner) = match frames.last() {
        Some(last) if last.kind == FrameKind::Reconciled => (
            last.sequence,
            last.generation,
            last.key_version,
            &last.owner,
        ),
        Some(_) => return Err(LedgerError::Stale),
        None if checkpoint.final_sequence == 0 && checkpoint.generation == 0 => {
            (0, 0, checkpoint.key_version, owner)
        }
        None => return Err(LedgerError::Stale),
    };
    if &checkpoint.owner != owner
        || ledger_owner != owner
        || checkpoint.owner_epoch != owner_epoch
        || checkpoint.final_sequence != final_sequence
        || checkpoint.generation != generation
        || checkpoint.key_version != key_version
        || checkpoint.stream_digest != digest
        || primary_generation != checkpoint.generation
    {
        return Err(LedgerError::Stale);
    }
    Ok(frames)
}

/// Verify the exact prefix represented by a trusted checkpoint and return a
/// separately authenticated contiguous extension. The extension is not normal
/// read authority: callers may only pass it to a constrained recovery path.
pub fn verify_stream_recovery_extension(
    bytes: &[u8],
    keys: &SyntheticKeyring,
    checkpoint: &TrustedLedgerCheckpoint,
    owner: &OwnerId,
    owner_epoch: u64,
) -> Result<(Vec<LedgerFrame>, Vec<LedgerFrame>), LedgerError> {
    let frames = verify_stream(bytes, keys)?;
    if checkpoint.owner != *owner || checkpoint.owner_epoch != owner_epoch {
        return Err(LedgerError::Stale);
    }
    let prefix_count =
        usize::try_from(checkpoint.final_sequence).map_err(|_| LedgerError::TooLarge)?;
    if prefix_count > frames.len() {
        return Err(LedgerError::Stale);
    }
    let mut cursor = 0_usize;
    for _ in 0..prefix_count {
        let len_bytes: [u8; 4] = bytes
            .get(cursor..cursor + 4)
            .ok_or(LedgerError::Invalid)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?;
        let len = u32::from_be_bytes(len_bytes) as usize;
        cursor = cursor
            .checked_add(4 + len)
            .filter(|end| *end <= bytes.len())
            .ok_or(LedgerError::Invalid)?;
    }
    let prefix = &frames[..prefix_count];
    let mut terminal_by_operation = BTreeMap::<&str, FrameKind>::new();
    for frame in prefix {
        terminal_by_operation.insert(&frame.operation_id, frame.kind);
    }
    if Sha256::digest(&bytes[..cursor]).as_slice() != checkpoint.stream_digest
        || prefix.last().map_or(0, |frame| frame.sequence) != checkpoint.final_sequence
        || prefix.last().map_or(0, |frame| frame.generation) != checkpoint.generation
        || prefix.last().is_some_and(|frame| {
            frame.kind != FrameKind::Reconciled
                || frame.key_version != checkpoint.key_version
                || &frame.owner != owner
        })
        || prefix.iter().any(|frame| &frame.owner != owner)
        || terminal_by_operation
            .values()
            .any(|kind| *kind != FrameKind::Reconciled)
        || (prefix_count == 0 && (checkpoint.generation != 0 || checkpoint.final_sequence != 0))
    {
        return Err(LedgerError::Stale);
    }
    Ok((prefix.to_vec(), frames[prefix_count..].to_vec()))
}

#[derive(Clone, Default)]
pub struct SyntheticKeyring(BTreeMap<u16, [u8; 32]>);

impl SyntheticKeyring {
    /// Deterministic generated test/development key. Never use for production.
    pub fn synthetic(version: u16, seed: &[u8]) -> Result<Self, LedgerError> {
        if version == 0 || seed.is_empty() || seed.len() > 256 {
            return Err(LedgerError::Invalid);
        }
        let mut material = b"SEREA-P7-SYNTHETIC-KEY-V1\0".to_vec();
        material.extend_from_slice(seed);
        let key: [u8; 32] = Sha256::digest(material).into();
        Ok(Self(BTreeMap::from([(version, key)])))
    }

    pub fn rotate_synthetic(&mut self, version: u16, seed: &[u8]) -> Result<(), LedgerError> {
        let next = Self::synthetic(version, seed)?;
        if self.0.contains_key(&version) {
            return Err(LedgerError::Invalid);
        }
        self.0.extend(next.0);
        Ok(())
    }

    pub fn forget_version(&mut self, version: u16) {
        self.0.remove(&version);
    }

    fn key(&self, version: u16) -> Result<&[u8; 32], LedgerError> {
        self.0.get(&version).ok_or(LedgerError::MissingKey)
    }
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut block = [0_u8; 64];
    block[..key.len()].copy_from_slice(key);
    let mut inner = Vec::with_capacity(64 + message.len());
    let mut outer = Vec::with_capacity(96);
    for byte in block {
        inner.push(byte ^ 0x36);
        outer.push(byte ^ 0x5c);
    }
    inner.extend_from_slice(message);
    outer.extend_from_slice(&Sha256::digest(inner));
    Sha256::digest(outer).into()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |diff, (a, b)| diff | (a ^ b))
        == 0
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), LedgerError> {
    let len = u16::try_from(bytes.len()).map_err(|_| LedgerError::TooLarge)?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

fn encode(frame: &LedgerFrame, keys: &SyntheticKeyring) -> Result<Vec<u8>, LedgerError> {
    if frame.sequence == 0
        || frame.generation == 0
        || frame.operation_id.is_empty()
        || frame.operation_id.len() > 96
        || frame.payload.len() > MAX_PAYLOAD_BYTES
        || frame.chunk_count > PROVISIONAL_MAX_CLOSURE_CHUNKS
        || frame.target_count > PROVISIONAL_MAX_CLOSURE_TARGETS
        || (frame.target_count == 0) != (frame.chunk_count == 0)
        || (frame.kind == FrameKind::Intent && frame.chunk_index != 0)
        || (frame.kind == FrameKind::Chunk
            && (frame.chunk_count == 0 || frame.chunk_index >= frame.chunk_count))
        || (frame.kind != FrameKind::Intent
            && frame.kind != FrameKind::Chunk
            && frame.chunk_index != 0)
        || (frame.target_count == 0 && frame.closure_digest != [0; 32])
    {
        return Err(LedgerError::Invalid);
    }
    let mut body = Vec::with_capacity(128 + frame.payload.len());
    body.extend_from_slice(MAGIC);
    body.extend_from_slice(&FORMAT.to_be_bytes());
    body.extend_from_slice(&frame.key_version.to_be_bytes());
    body.extend_from_slice(&frame.sequence.to_be_bytes());
    body.extend_from_slice(&frame.generation.to_be_bytes());
    body.extend_from_slice(&frame.chunk_index.to_be_bytes());
    body.extend_from_slice(&frame.chunk_count.to_be_bytes());
    body.extend_from_slice(&frame.target_count.to_be_bytes());
    body.extend_from_slice(&frame.closure_digest);
    body.push(frame.kind as u8);
    put_bytes(&mut body, frame.owner.as_str().as_bytes())?;
    put_bytes(&mut body, frame.operation_id.as_bytes())?;
    put_bytes(&mut body, &frame.payload)?;
    let mac = hmac_sha256(keys.key(frame.key_version)?, &body);
    let len = u32::try_from(body.len() + MAC_BYTES).map_err(|_| LedgerError::TooLarge)?;
    let mut encoded = Vec::with_capacity(4 + len as usize);
    encoded.extend_from_slice(&len.to_be_bytes());
    encoded.extend_from_slice(&body);
    encoded.extend_from_slice(&mac);
    if encoded.len() > MAX_FRAME_BYTES {
        return Err(LedgerError::TooLarge);
    }
    Ok(encoded)
}

fn take<'a>(bytes: &'a [u8], cursor: &mut usize, count: usize) -> Result<&'a [u8], LedgerError> {
    let end = cursor.checked_add(count).ok_or(LedgerError::Invalid)?;
    let value = bytes.get(*cursor..end).ok_or(LedgerError::Invalid)?;
    *cursor = end;
    Ok(value)
}

fn take_bytes<'a>(bytes: &'a [u8], cursor: &mut usize) -> Result<&'a [u8], LedgerError> {
    let len = u16::from_be_bytes(
        take(bytes, cursor, 2)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?,
    ) as usize;
    take(bytes, cursor, len)
}

fn decode(bytes: &[u8], keys: &SyntheticKeyring) -> Result<LedgerFrame, LedgerError> {
    if bytes.len() < MAC_BYTES + 32 || bytes.len() > MAX_FRAME_BYTES {
        return Err(LedgerError::Invalid);
    }
    let declared_len = u32::from_be_bytes(
        bytes
            .get(..4)
            .ok_or(LedgerError::Invalid)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?,
    ) as usize;
    if declared_len.checked_add(4) != Some(bytes.len()) {
        return Err(LedgerError::Invalid);
    }
    let mut cursor = 4;
    if take(bytes, &mut cursor, 4)? != MAGIC {
        return Err(LedgerError::Invalid);
    }
    if u16::from_be_bytes(
        take(bytes, &mut cursor, 2)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?,
    ) != FORMAT
    {
        return Err(LedgerError::Invalid);
    }
    let key_version = u16::from_be_bytes(
        take(bytes, &mut cursor, 2)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?,
    );
    let sequence = u64::from_be_bytes(
        take(bytes, &mut cursor, 8)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?,
    );
    let generation = u64::from_be_bytes(
        take(bytes, &mut cursor, 8)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?,
    );
    let chunk_index = u16::from_be_bytes(
        take(bytes, &mut cursor, 2)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?,
    );
    let chunk_count = u16::from_be_bytes(
        take(bytes, &mut cursor, 2)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?,
    );
    let target_count = u16::from_be_bytes(
        take(bytes, &mut cursor, 2)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?,
    );
    let closure_digest = take(bytes, &mut cursor, 32)?
        .try_into()
        .map_err(|_| LedgerError::Invalid)?;
    let kind = FrameKind::try_from(take(bytes, &mut cursor, 1)?[0])?;
    let owner = OwnerId::parse(
        String::from_utf8(take_bytes(bytes, &mut cursor)?.to_vec())
            .map_err(|_| LedgerError::Invalid)?,
    )?;
    let operation_id = String::from_utf8(take_bytes(bytes, &mut cursor)?.to_vec())
        .map_err(|_| LedgerError::Invalid)?;
    let payload = take_bytes(bytes, &mut cursor)?.to_vec();
    if cursor + MAC_BYTES != bytes.len() {
        return Err(LedgerError::Invalid);
    }
    if sequence == 0
        || generation == 0
        || operation_id.is_empty()
        || payload.len() > MAX_PAYLOAD_BYTES
        || chunk_count > PROVISIONAL_MAX_CLOSURE_CHUNKS
        || target_count > PROVISIONAL_MAX_CLOSURE_TARGETS
        || (target_count == 0) != (chunk_count == 0)
        || (kind == FrameKind::Intent && chunk_index != 0)
        || (kind == FrameKind::Chunk && (chunk_count == 0 || chunk_index >= chunk_count))
        || (kind != FrameKind::Intent && kind != FrameKind::Chunk && chunk_index != 0)
        || (target_count == 0 && closure_digest != [0; 32])
    {
        return Err(LedgerError::Invalid);
    }
    let expected = hmac_sha256(keys.key(key_version)?, &bytes[4..cursor]);
    if !constant_time_eq(&expected, &bytes[cursor..]) {
        return Err(LedgerError::Authentication);
    }
    Ok(LedgerFrame {
        sequence,
        generation,
        key_version,
        chunk_index,
        chunk_count,
        target_count,
        closure_digest,
        owner,
        operation_id,
        kind,
        payload,
    })
}

/// Verify a complete byte stream. Torn frames, reordering, and generation
/// rollback are errors; callers must quarantine rather than truncate.
pub fn verify_stream(
    bytes: &[u8],
    keys: &SyntheticKeyring,
) -> Result<Vec<LedgerFrame>, LedgerError> {
    if bytes.len() > MAX_LEDGER_BYTES {
        return Err(LedgerError::TooLarge);
    }
    let mut cursor = 0;
    let mut frames = Vec::new();
    let mut generation = 0_u64;
    let mut owner: Option<OwnerId> = None;
    let mut operations = BTreeMap::<String, (u64, u8, u16, u16, u16, [u8; 32])>::new();
    let mut closure_payloads = BTreeMap::<String, Vec<u8>>::new();
    while cursor < bytes.len() {
        let len_end = cursor.checked_add(4).ok_or(LedgerError::Invalid)?;
        let len_bytes: [u8; 4] = bytes
            .get(cursor..len_end)
            .ok_or(LedgerError::Invalid)?
            .try_into()
            .map_err(|_| LedgerError::Invalid)?;
        let frame_len = u32::from_be_bytes(len_bytes) as usize;
        if !(32 + MAC_BYTES..=MAX_FRAME_BYTES - 4).contains(&frame_len) {
            return Err(LedgerError::TooLarge);
        }
        let end = len_end.checked_add(frame_len).ok_or(LedgerError::Invalid)?;
        let frame = decode(bytes.get(cursor..end).ok_or(LedgerError::Invalid)?, keys)?;
        if frames.len() >= MAX_LEDGER_FRAMES {
            return Err(LedgerError::TooLarge);
        }
        if frame.sequence != frames.len() as u64 + 1 {
            return Err(LedgerError::Sequence);
        }
        if frame.generation < generation || frame.generation > generation.saturating_add(1) {
            return Err(LedgerError::Generation);
        }
        if owner.as_ref().is_some_and(|known| known != &frame.owner) {
            return Err(LedgerError::Invalid);
        }
        owner.get_or_insert_with(|| frame.owner.clone());
        let transition = operations.get(&frame.operation_id).copied();
        if frame.kind == FrameKind::Chunk {
            let payload = closure_payloads
                .entry(frame.operation_id.clone())
                .or_default();
            if payload
                .len()
                .checked_add(frame.payload.len())
                .is_none_or(|len| len > PROVISIONAL_MAX_CLOSURE_BYTES)
            {
                return Err(LedgerError::TooLarge);
            }
            payload.extend_from_slice(&frame.payload);
        }
        let closure_complete = if frame.kind != FrameKind::PrimaryApplied {
            true
        } else if frame.target_count == 0 {
            frame.closure_digest == [0; 32]
        } else {
            let committed: [u8; 32] = Sha256::digest(
                closure_payloads
                    .get(&frame.operation_id)
                    .ok_or(LedgerError::Invalid)?,
            )
            .into();
            committed == frame.closure_digest
        };
        let next = match (frame.kind, transition) {
            (FrameKind::Intent, None) => (
                frame.generation,
                1,
                frame.chunk_count,
                0,
                frame.target_count,
                frame.closure_digest,
            ),
            (FrameKind::Chunk, Some((op_generation, 1, count, next_chunk, targets, digest)))
                if op_generation == frame.generation
                    && frame.chunk_index == next_chunk
                    && frame.chunk_count == count
                    && frame.target_count == targets
                    && frame.closure_digest == digest
                    && !frame.payload.is_empty() =>
            {
                (op_generation, 1, count, next_chunk + 1, targets, digest)
            }
            (
                FrameKind::PrimaryApplied,
                Some((op_generation, 1, count, next_chunk, targets, digest)),
            ) if op_generation == frame.generation
                && next_chunk == count
                && frame.chunk_count == count
                && frame.target_count == targets
                && frame.closure_digest == digest
                && closure_complete =>
            {
                (op_generation, 2, count, next_chunk, targets, digest)
            }
            (
                FrameKind::Finalized,
                Some((op_generation, 2, count, next_chunk, targets, digest)),
            ) if op_generation == frame.generation
                && frame.chunk_count == count
                && frame.target_count == targets
                && frame.closure_digest == digest =>
            {
                (op_generation, 3, count, next_chunk, targets, digest)
            }
            (
                FrameKind::Reconciled,
                Some((op_generation, 3, count, next_chunk, targets, digest)),
            ) if op_generation.checked_add(1) == Some(frame.generation)
                && frame.chunk_count == count
                && frame.target_count == targets
                && frame.closure_digest == digest =>
            {
                (frame.generation, 4, count, next_chunk, targets, digest)
            }
            _ => return Err(LedgerError::Invalid),
        };
        operations.insert(frame.operation_id.clone(), next);
        generation = frame.generation;
        frames.push(frame);
        cursor = end;
    }
    Ok(frames)
}

/// Verify that one ledger operation reached its terminal reconciliation frame.
/// This proves ledger state only; callers must still compare the primary DB's
/// Owner/generation/tombstones/FTS state before they can acknowledge Forget.
pub fn verify_ledger_operation_reconciled(
    bytes: &[u8],
    keys: &SyntheticKeyring,
    owner: &OwnerId,
    operation_id: &str,
    generation: u64,
) -> Result<(), LedgerError> {
    let frames = verify_stream(bytes, keys)?;
    let last = frames
        .iter()
        .rfind(|frame| frame.operation_id == operation_id)
        .ok_or(LedgerError::Invalid)?;
    if &last.owner == owner && last.kind == FrameKind::Reconciled && last.generation == generation {
        Ok(())
    } else {
        Err(LedgerError::Invalid)
    }
}

/// Cooperative cross-process ledger lock and bounded file adapter. The stable
/// sidecar must live beside the ledger in a trusted directory and must never
/// be unlinked while processes may hold it. This does not defend against a
/// malicious same-user process replacing files or bypassing the lock.
pub struct LedgerFileGuard {
    _lock_file: File,
    /// The sidecar lock serializes processes; this mutex serializes cursor
    /// operations when one guard is shared by threads in a process.
    ledger_file: std::sync::Mutex<File>,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
enum SimulatedWriteFault {
    Short,
    WriteZero,
    NoSpace,
}

#[cfg(test)]
fn apply_simulated_write_fault(
    file: &mut &File,
    encoded: &[u8],
    fault: Option<SimulatedWriteFault>,
) -> Result<(), LedgerError> {
    match fault {
        Some(SimulatedWriteFault::WriteZero | SimulatedWriteFault::NoSpace) => Err(LedgerError::Io),
        Some(SimulatedWriteFault::Short) => {
            file.write_all(&encoded[..encoded.len() / 2])
                .map_err(|_| LedgerError::Io)?;
            file.sync_all().map_err(|_| LedgerError::Io)?;
            Err(LedgerError::Io)
        }
        None => Ok(()),
    }
}

#[cfg(not(test))]
fn apply_simulated_write_fault(
    _file: &mut &File,
    _encoded: &[u8],
    _fault: Option<SimulatedWriteFault>,
) -> Result<(), LedgerError> {
    Ok(())
}

impl LedgerFileGuard {
    /// Acquire/create a synthetic ledger for a new namespace or test fixture.
    pub fn acquire(ledger_path: impl AsRef<Path>) -> Result<Self, LedgerError> {
        Self::acquire_inner(ledger_path.as_ref(), true)
    }

    /// Acquire an existing ledger without recreating a missing authority file.
    pub fn acquire_existing(ledger_path: impl AsRef<Path>) -> Result<Self, LedgerError> {
        Self::acquire_inner(ledger_path.as_ref(), false)
    }

    fn acquire_inner(ledger_path: &Path, create: bool) -> Result<Self, LedgerError> {
        let leaf = ledger_path.file_name().ok_or(LedgerError::Invalid)?;
        let parent = ledger_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = fs::canonicalize(parent).map_err(|_| LedgerError::Io)?;
        let ledger_path = parent.join(leaf);
        if let Ok(metadata) = fs::symlink_metadata(&ledger_path) {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(LedgerError::Invalid);
            }
        } else if !create {
            return Err(LedgerError::MissingAuthority);
        }
        let mut lock_name = ledger_path.as_os_str().to_os_string();
        lock_name.push(".lock");
        let lock_path = PathBuf::from(lock_name);
        if let Ok(metadata) = fs::symlink_metadata(&lock_path) {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(LedgerError::Invalid);
            }
        }
        let lock_file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .map_err(|_| LedgerError::Io)?;
        lock_file.lock_exclusive().map_err(|_| LedgerError::Io)?;
        let lock_meta = lock_file.metadata().map_err(|_| LedgerError::Io)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let current = fs::symlink_metadata(&lock_path).map_err(|_| LedgerError::Io)?;
            if current.file_type().is_symlink()
                || current.dev() != lock_meta.dev()
                || current.ino() != lock_meta.ino()
            {
                return Err(LedgerError::Stale);
            }
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        if create {
            options.create(true);
        }
        let ledger_file = options
            .open(&ledger_path)
            .map_err(|_| LedgerError::MissingAuthority)?;
        let opened_meta = ledger_file.metadata().map_err(|_| LedgerError::Io)?;
        if !opened_meta.is_file() {
            return Err(LedgerError::Invalid);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let current = fs::symlink_metadata(&ledger_path).map_err(|_| LedgerError::Io)?;
            if current.file_type().is_symlink()
                || current.dev() != opened_meta.dev()
                || current.ino() != opened_meta.ino()
            {
                return Err(LedgerError::Stale);
            }
        }
        Ok(Self {
            _lock_file: lock_file,
            ledger_file: std::sync::Mutex::new(ledger_file),
        })
    }

    /// Append and flush one record while holding this guard's OS lock.
    pub fn append_sync(
        &self,
        frame: &LedgerFrame,
        keys: &SyntheticKeyring,
    ) -> Result<(), LedgerError> {
        self.append_sync_inner(frame, keys, None)
    }

    #[cfg(test)]
    fn append_sync_simulated(
        &self,
        frame: &LedgerFrame,
        keys: &SyntheticKeyring,
        fault: SimulatedWriteFault,
    ) -> Result<(), LedgerError> {
        self.append_sync_inner(frame, keys, Some(fault))
    }

    fn append_sync_inner(
        &self,
        frame: &LedgerFrame,
        keys: &SyntheticKeyring,
        fault: Option<SimulatedWriteFault>,
    ) -> Result<(), LedgerError> {
        let encoded = encode(frame, keys)?;
        let ledger_file = self.ledger_file.lock().map_err(|_| LedgerError::Io)?;
        let mut existing = Vec::new();
        let current_len = ledger_file.metadata().map_err(|_| LedgerError::Io)?.len();
        if current_len > MAX_LEDGER_BYTES as u64
            || current_len.saturating_add(encoded.len() as u64) > MAX_LEDGER_BYTES as u64
        {
            return Err(LedgerError::TooLarge);
        }
        let mut file = &*ledger_file;
        file.seek(SeekFrom::Start(0)).map_err(|_| LedgerError::Io)?;
        file.take(MAX_LEDGER_BYTES as u64 + 1)
            .read_to_end(&mut existing)
            .map_err(|_| LedgerError::Io)?;
        if existing.len() > MAX_LEDGER_BYTES {
            return Err(LedgerError::TooLarge);
        }
        existing.extend_from_slice(&encoded);
        let records = verify_stream(&existing, keys)?;
        if records.last().map(|record| record.sequence) != Some(frame.sequence) {
            return Err(LedgerError::Sequence);
        }
        let mut file = &*ledger_file;
        file.seek(SeekFrom::End(0)).map_err(|_| LedgerError::Io)?;
        apply_simulated_write_fault(&mut file, &encoded, fault)?;
        crash_during_ledger_append(&mut file, &encoded)?;
        file.write_all(&encoded).map_err(|_| LedgerError::Io)?;
        file.sync_all().map_err(|_| LedgerError::Io)
    }

    /// Read and authenticate the complete bounded ledger while holding the lock.
    pub fn read_verified(&self, keys: &SyntheticKeyring) -> Result<Vec<LedgerFrame>, LedgerError> {
        verify_stream(&self.read_bytes()?, keys)
    }

    /// Read the bounded bytes from the already opened ledger inode.
    pub fn read_bytes(&self) -> Result<Vec<u8>, LedgerError> {
        let ledger_file = self.ledger_file.lock().map_err(|_| LedgerError::Io)?;
        let mut file = &*ledger_file;
        file.seek(SeekFrom::Start(0)).map_err(|_| LedgerError::Io)?;
        let mut bytes = Vec::new();
        file.take(MAX_LEDGER_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| LedgerError::Io)?;
        if bytes.len() > MAX_LEDGER_BYTES {
            return Err(LedgerError::TooLarge);
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(1);
    fn owner() -> OwnerId {
        OwnerId::parse("own_00000000000000000000000000000001").unwrap()
    }
    fn frame(sequence: u64, generation: u64, key_version: u16, kind: FrameKind) -> LedgerFrame {
        LedgerFrame {
            sequence,
            generation,
            key_version,
            chunk_index: 0,
            chunk_count: 0,
            target_count: 0,
            closure_digest: [0; 32],
            owner: owner(),
            operation_id: "forget-test-1".into(),
            kind,
            payload: b"synthetic-target-1".to_vec(),
        }
    }

    #[test]
    fn owner_id_auth_purpose_freshness_and_nonce_are_typed() {
        assert!(OwnerId::parse("own_bad").is_err());
        let evidence = AuthEvidence::from_verified_adapter(
            AuthPurpose::Forget,
            100,
            200,
            [7; 32],
            "fake-v1".into(),
        )
        .unwrap();
        assert_eq!(evidence.validate_for(AuthPurpose::Forget, 199), Ok(()));
        assert_eq!(
            evidence.validate_for(AuthPurpose::RenewMemory, 199),
            Err(LedgerError::Authentication)
        );
        assert_eq!(
            evidence.validate_for(AuthPurpose::Forget, 200),
            Err(LedgerError::Authentication)
        );
        let mut guard = EvidenceReplayGuard::default();
        assert_eq!(guard.consume(&evidence, AuthPurpose::Forget, 199), Ok(()));
        assert_eq!(
            guard.consume(&evidence, AuthPurpose::Forget, 199),
            Err(LedgerError::Authentication)
        );
        assert!(
            AuthEvidence::from_verified_adapter(
                AuthPurpose::Forget,
                1,
                2,
                [0; 32],
                "fake-v1".into()
            )
            .is_err()
        );
        for platform in [
            HostPlatform::MacOs,
            HostPlatform::Windows,
            HostPlatform::Linux,
        ] {
            let adapter = UnsupportedHostAuthenticator::new(platform);
            assert_eq!(adapter.platform(), platform);
            assert_eq!(
                adapter.authenticate(AuthenticationRequest {
                    purpose: AuthPurpose::Forget,
                    observed_at_ms: 199,
                    max_evidence_age_ms: 1,
                }),
                Err(LedgerError::Authentication)
            );
        }
    }

    #[test]
    fn gate_c_feature_does_not_select_experimental_migration_without_its_feature() {
        if cfg!(feature = "p7-experimental") {
            assert_eq!(crate::Migrations::LATEST, 9);
            assert!(crate::Migrations::embedded().iter().any(|migration| {
                migration.version == 9 && migration.name == "test-fixture-p7-experimental-memory-v9"
            }));
        } else {
            assert_eq!(crate::Migrations::LATEST, 8);
            assert!(
                crate::Migrations::embedded()
                    .iter()
                    .all(|migration| migration.version <= 8)
            );
        }
        assert_eq!(ACCEPTED_V1_BOUNDS.max_active_items, 1_000);
        assert_eq!(ACCEPTED_V1_BOUNDS.max_item_body_utf8_bytes, 4_096);
        assert_eq!(ACCEPTED_V1_BOUNDS.working_retention_days, 7);
        assert_eq!(ACCEPTED_V1_BOUNDS.preference_retention_days, 365);
    }

    #[test]
    fn hmac_sha256_matches_rfc_4231_test_case_one() {
        let expected = [
            0xb0, 0x34, 0x4c, 0x61, 0xd8, 0xdb, 0x38, 0x53, 0x5c, 0xa8, 0xaf, 0xce, 0xaf, 0x0b,
            0xf1, 0x2b, 0x88, 0x1d, 0xc2, 0x00, 0xc9, 0x83, 0x3d, 0xa7, 0x26, 0xe9, 0x37, 0x6c,
            0x2e, 0x32, 0xcf, 0xf7,
        ];
        assert_eq!(hmac_sha256(&[0x0b; 20], b"Hi There"), expected);
    }

    #[test]
    fn forget_intent_requires_complete_ordered_chunks_before_primary_apply() {
        let keys = SyntheticKeyring::synthetic(1, b"chunk-fixture").unwrap();
        let closure = b"synthetic-target-1";
        let digest: [u8; 32] = Sha256::digest(closure).into();
        let mut intent = frame(1, 1, 1, FrameKind::Intent);
        intent.payload.clear();
        intent.chunk_count = 1;
        intent.target_count = 1;
        intent.closure_digest = digest;
        let mut chunk = frame(2, 1, 1, FrameKind::Chunk);
        chunk.chunk_index = 0;
        chunk.chunk_count = 1;
        chunk.target_count = 1;
        chunk.closure_digest = digest;
        chunk.payload = closure.to_vec();
        let mut applied = frame(3, 1, 1, FrameKind::PrimaryApplied);
        applied.chunk_count = 1;
        applied.target_count = 1;
        applied.closure_digest = digest;
        let mut finalized = frame(4, 1, 1, FrameKind::Finalized);
        finalized.chunk_count = 1;
        finalized.target_count = 1;
        finalized.closure_digest = digest;
        let mut reconciled = frame(5, 2, 1, FrameKind::Reconciled);
        reconciled.chunk_count = 1;
        reconciled.target_count = 1;
        reconciled.closure_digest = digest;
        let encoded = [
            intent.clone(),
            chunk.clone(),
            applied.clone(),
            finalized,
            reconciled,
        ]
        .iter()
        .map(|entry| encode(entry, &keys).unwrap())
        .collect::<Vec<_>>();
        let complete_stream = encoded.concat();
        assert_eq!(verify_stream(&complete_stream, &keys).unwrap().len(), 5);
        assert_eq!(
            verify_ledger_operation_reconciled(
                &complete_stream,
                &keys,
                &owner(),
                "forget-test-1",
                2
            ),
            Ok(())
        );
        assert_eq!(
            verify_ledger_operation_reconciled(
                &complete_stream,
                &keys,
                &OwnerId::parse("own_00000000000000000000000000000002").unwrap(),
                "forget-test-1",
                2
            ),
            Err(LedgerError::Invalid)
        );
        assert_eq!(
            verify_ledger_operation_reconciled(
                &complete_stream,
                &keys,
                &owner(),
                "forget-test-1",
                1
            ),
            Err(LedgerError::Invalid)
        );

        let missing_chunk = [
            encode(&intent, &keys).unwrap(),
            encode(&applied, &keys).unwrap(),
        ];
        assert!(verify_stream(&missing_chunk.concat(), &keys).is_err());
        assert!(
            verify_ledger_operation_reconciled(
                &missing_chunk.concat(),
                &keys,
                &owner(),
                "forget-test-1",
                2
            )
            .is_err()
        );
        intent.chunk_count = 2;
        intent.target_count = 2;
        chunk.chunk_index = 1;
        chunk.chunk_count = 2;
        chunk.target_count = 2;
        let reordered_chunk = [
            encode(&intent, &keys).unwrap(),
            encode(&chunk, &keys).unwrap(),
        ];
        assert_eq!(
            verify_stream(&reordered_chunk.concat(), &keys),
            Err(LedgerError::Invalid)
        );
    }

    #[test]
    fn independently_checkpointed_stream_rejects_valid_prefixes_and_generation_mismatch() {
        let keys = SyntheticKeyring::synthetic(1, b"freshness-fixture").unwrap();
        let first = [
            frame(1, 1, 1, FrameKind::Intent),
            frame(2, 1, 1, FrameKind::PrimaryApplied),
            frame(3, 1, 1, FrameKind::Finalized),
            frame(4, 2, 1, FrameKind::Reconciled),
        ];
        let mut second = [
            frame(5, 2, 1, FrameKind::Intent),
            frame(6, 2, 1, FrameKind::PrimaryApplied),
            frame(7, 2, 1, FrameKind::Finalized),
            frame(8, 3, 1, FrameKind::Reconciled),
        ];
        for entry in &mut second {
            entry.operation_id = "forget-test-2".into();
        }
        let frames = [first.to_vec(), second.to_vec()].concat();
        let encoded = frames
            .iter()
            .map(|entry| encode(entry, &keys).unwrap())
            .collect::<Vec<_>>();
        let complete = encoded.concat();
        let checkpoint = TrustedLedgerCheckpoint {
            revision: 1,
            owner: owner(),
            owner_epoch: 7,
            key_version: 1,
            final_sequence: 8,
            generation: 3,
            stream_digest: Sha256::digest(&complete).into(),
        };

        assert_eq!(verify_stream(&complete, &keys).unwrap().len(), 8);
        assert!(verify_stream_at_checkpoint(&complete, &keys, None, &owner(), 7, 3).is_err());
        assert!(
            verify_stream_at_checkpoint(&complete, &keys, Some(&checkpoint), &owner(), 7, 3)
                .is_ok()
        );

        // Frame integrity alone accepts an older complete prefix, including one
        // missing only the final reconciliation frame. Freshness must reject it.
        for keep in [1, 2, 3, 4, 5, 6, 7] {
            let prefix = encoded[..keep].concat();
            assert_eq!(verify_stream(&prefix, &keys).unwrap().len(), keep);
            assert_eq!(
                verify_stream_at_checkpoint(&prefix, &keys, Some(&checkpoint), &owner(), 7, 3),
                Err(LedgerError::Stale)
            );
        }
        let torn = &complete[..complete.len() - 1];
        assert!(verify_stream(torn, &keys).is_err());
        assert_eq!(
            verify_stream_at_checkpoint(&complete, &keys, Some(&checkpoint), &owner(), 8, 3),
            Err(LedgerError::Stale),
            "unknown Owner epoch is a freshness failure"
        );
        assert_eq!(
            verify_stream_at_checkpoint(&complete, &keys, Some(&checkpoint), &owner(), 7, 2),
            Err(LedgerError::Stale),
            "primary older than checkpoint is denied"
        );
        assert_eq!(
            verify_stream_at_checkpoint(&complete, &keys, Some(&checkpoint), &owner(), 7, 4),
            Err(LedgerError::Stale),
            "primary newer than checkpoint requires reconciliation"
        );
        let other_owner = OwnerId::parse("own_00000000000000000000000000000002").unwrap();
        assert_eq!(
            verify_stream_at_checkpoint(&complete, &keys, Some(&checkpoint), &other_owner, 7, 3),
            Err(LedgerError::Stale)
        );
        let mut revoked = keys.clone();
        revoked.forget_version(1);
        assert_eq!(
            verify_stream_at_checkpoint(&complete, &revoked, Some(&checkpoint), &owner(), 7, 3),
            Err(LedgerError::MissingKey)
        );
        let mut unknown_version = checkpoint.clone();
        unknown_version.key_version = 2;
        assert_eq!(
            verify_stream_at_checkpoint(&complete, &keys, Some(&unknown_version), &owner(), 7, 3),
            Err(LedgerError::Stale)
        );

        // A later operation's reconciliation cannot hide an earlier unresolved
        // intent in the same authenticated stream.
        let mut pending = encode(&frame(1, 1, 1, FrameKind::Intent), &keys).unwrap();
        let mut later = [
            frame(2, 1, 1, FrameKind::Intent),
            frame(3, 1, 1, FrameKind::PrimaryApplied),
            frame(4, 1, 1, FrameKind::Finalized),
            frame(5, 2, 1, FrameKind::Reconciled),
        ];
        for entry in &mut later {
            entry.operation_id = "later-op".into();
            pending.extend(encode(entry, &keys).unwrap());
        }
        let pending_checkpoint = TrustedLedgerCheckpoint {
            revision: 1,
            owner: owner(),
            owner_epoch: 7,
            key_version: 1,
            final_sequence: 5,
            generation: 2,
            stream_digest: Sha256::digest(&pending).into(),
        };
        assert!(verify_stream(&pending, &keys).is_ok());
        assert_eq!(
            verify_stream_at_checkpoint(&pending, &keys, Some(&pending_checkpoint), &owner(), 7, 2),
            Err(LedgerError::Stale)
        );

        // If all local evidence is rolled back together, this verifier cannot
        // distinguish it. The supplied checkpoint itself must survive elsewhere.
        let old = Vec::new();
        let rolled_back_checkpoint = TrustedLedgerCheckpoint {
            revision: 1,
            owner: owner(),
            owner_epoch: 7,
            key_version: 1,
            final_sequence: 0,
            generation: 0,
            stream_digest: Sha256::digest(&old).into(),
        };
        assert!(
            verify_stream_at_checkpoint(&old, &keys, Some(&rolled_back_checkpoint), &owner(), 7, 0)
                .is_ok()
        );
    }

    #[test]
    fn ledger_detects_truncation_tampering_order_and_unknown_key() {
        let keys = SyntheticKeyring::synthetic(1, b"fixture-one").unwrap();
        let one = encode(&frame(1, 1, 1, FrameKind::Intent), &keys).unwrap();
        let applied = encode(&frame(2, 1, 1, FrameKind::PrimaryApplied), &keys).unwrap();
        let finalized = encode(&frame(3, 1, 1, FrameKind::Finalized), &keys).unwrap();
        let reconciled = encode(&frame(4, 2, 1, FrameKind::Reconciled), &keys).unwrap();
        let mut stream = [
            one.clone(),
            applied.clone(),
            finalized.clone(),
            reconciled.clone(),
        ]
        .concat();
        assert_eq!(verify_stream(&stream, &keys).unwrap().len(), 4);
        assert!(verify_stream(&stream[..stream.len() - 1], &keys).is_err());
        stream[20] ^= 1;
        assert_eq!(
            verify_stream(&stream, &keys),
            Err(LedgerError::Authentication)
        );
        assert_eq!(
            verify_stream(&[applied.clone(), one.clone()].concat(), &keys),
            Err(LedgerError::Sequence)
        );
        let no_key = SyntheticKeyring::default();
        assert_eq!(verify_stream(&one, &no_key), Err(LedgerError::MissingKey));
        let premature_final = encode(&frame(2, 1, 1, FrameKind::Finalized), &keys).unwrap();
        assert_eq!(
            verify_stream(&[one.clone(), premature_final].concat(), &keys),
            Err(LedgerError::Invalid)
        );
    }

    #[test]
    fn ledger_rotation_requires_old_and_new_verification_keys() {
        let mut keys = SyntheticKeyring::synthetic(1, b"old-synthetic").unwrap();
        let first = encode(&frame(1, 1, 1, FrameKind::Intent), &keys).unwrap();
        let applied = encode(&frame(2, 1, 1, FrameKind::PrimaryApplied), &keys).unwrap();
        keys.rotate_synthetic(2, b"new-synthetic").unwrap();
        assert_eq!(
            keys.rotate_synthetic(2, b"replacement-must-refuse"),
            Err(LedgerError::Invalid)
        );
        let second = encode(&frame(3, 1, 2, FrameKind::Finalized), &keys).unwrap();
        let reconciled = encode(&frame(4, 2, 2, FrameKind::Reconciled), &keys).unwrap();
        assert_eq!(
            verify_stream(
                &[first.clone(), applied, second, reconciled].concat(),
                &keys
            )
            .unwrap()
            .len(),
            4
        );
        keys.forget_version(1);
        assert_eq!(verify_stream(&first, &keys), Err(LedgerError::MissingKey));
    }

    #[test]
    fn ledger_append_reopen_uses_only_synthetic_frames() {
        let root = std::env::temp_dir().join(format!(
            "serea-gate-c-ledger-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("ledger.synthetic");
        let keys = SyntheticKeyring::synthetic(1, b"generated-test-fixture").unwrap();
        let guard = LedgerFileGuard::acquire(&path).unwrap();
        guard
            .append_sync(&frame(1, 1, 1, FrameKind::Intent), &keys)
            .unwrap();
        guard
            .append_sync(&frame(2, 1, 1, FrameKind::PrimaryApplied), &keys)
            .unwrap();
        guard
            .append_sync(&frame(3, 1, 1, FrameKind::Finalized), &keys)
            .unwrap();
        guard
            .append_sync(&frame(4, 2, 1, FrameKind::Reconciled), &keys)
            .unwrap();
        assert_eq!(guard.read_verified(&keys).unwrap().len(), 4);
        assert_eq!(
            guard.append_sync(&frame(4, 2, 1, FrameKind::Reconciled), &keys),
            Err(LedgerError::Sequence)
        );
        drop(guard);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_append_calls_on_one_guard_never_duplicate_a_sequence() {
        use std::sync::{Arc, Barrier};
        use std::thread;
        let root = std::env::temp_dir().join(format!(
            "serea-gate-c-same-guard-race-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let keys = SyntheticKeyring::synthetic(1, b"same-guard-race-key").unwrap();
        for iteration in 0..128 {
            let guard = Arc::new(
                LedgerFileGuard::acquire(root.join(format!("ledger-{iteration}"))).unwrap(),
            );
            let barrier = Arc::new(Barrier::new(2));
            let first = {
                let guard = Arc::clone(&guard);
                let barrier = Arc::clone(&barrier);
                let keys = keys.clone();
                thread::spawn(move || {
                    barrier.wait();
                    guard.append_sync(&frame(1, 1, 1, FrameKind::Intent), &keys)
                })
            };
            let second = {
                let guard = Arc::clone(&guard);
                let barrier = Arc::clone(&barrier);
                let keys = keys.clone();
                thread::spawn(move || {
                    barrier.wait();
                    guard.append_sync(&frame(1, 1, 1, FrameKind::Intent), &keys)
                })
            };
            let results = [first.join().unwrap(), second.join().unwrap()];
            assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
            assert_eq!(guard.read_verified(&keys).unwrap().len(), 1);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_checkpoint_bootstrap_reopen_tamper_and_missing_authority() {
        let root = std::env::temp_dir().join(format!(
            "serea-gate-c-checkpoint-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let directory = root.join("authority");
        let keys = SyntheticKeyring::synthetic(1, b"checkpoint-test-generated-key").unwrap();
        let initial = TrustedLedgerCheckpoint {
            revision: 0,
            owner: owner(),
            owner_epoch: 7,
            key_version: 1,
            final_sequence: 0,
            generation: 0,
            stream_digest: Sha256::digest([]).into(),
        };
        assert_eq!(
            PersistentCheckpointStore::open_existing(&directory, &keys).err(),
            Some(LedgerError::MissingAuthority)
        );
        let mut authority =
            PersistentCheckpointStore::bootstrap(&directory, initial, &keys).unwrap();
        let first_revision = authority.current().clone();
        let next = TrustedLedgerCheckpoint {
            revision: 0,
            owner: owner(),
            owner_epoch: 7,
            key_version: 1,
            final_sequence: 5,
            generation: 2,
            stream_digest: [7; 32],
        };
        authority.advance(next.clone(), &keys).unwrap();
        assert_eq!(authority.current().revision, 2);
        drop(authority);
        assert_eq!(
            PersistentCheckpointStore::open_existing(&directory, &keys)
                .unwrap()
                .current(),
            &TrustedLedgerCheckpoint {
                revision: 2,
                ..next.clone()
            }
        );

        let latest = directory.join("checkpoint-00000000000000000002.bin");
        let mut bytes = fs::read(&latest).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        fs::write(&latest, bytes).unwrap();
        assert_eq!(
            PersistentCheckpointStore::open_existing(&directory, &keys).err(),
            Some(LedgerError::Authentication)
        );

        // A rolled-back authority directory selects the old record; it cannot
        // detect its own complete rollback without an outside surviving anchor.
        fs::remove_file(&latest).unwrap();
        let rolled_back = PersistentCheckpointStore::open_existing(&directory, &keys).unwrap();
        assert_eq!(rolled_back.current(), &first_revision);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn checkpoint_missing_middle_truncated_highest_duplicate_and_wrong_key_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "serea-gate-c-checkpoint-corruption-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let keys = SyntheticKeyring::synthetic(1, b"checkpoint-corruption-key").unwrap();
        let initial = TrustedLedgerCheckpoint {
            revision: 0,
            owner: owner(),
            owner_epoch: 7,
            key_version: 1,
            final_sequence: 0,
            generation: 0,
            stream_digest: Sha256::digest([]).into(),
        };

        let middle = root.join("middle");
        let mut authority =
            PersistentCheckpointStore::bootstrap(&middle, initial.clone(), &keys).unwrap();
        for revision in 2..=3 {
            authority
                .advance(
                    TrustedLedgerCheckpoint {
                        revision: 0,
                        owner: owner(),
                        owner_epoch: 7,
                        key_version: 1,
                        final_sequence: revision,
                        generation: revision,
                        stream_digest: [revision as u8; 32],
                    },
                    &keys,
                )
                .unwrap();
        }
        drop(authority);
        fs::remove_file(middle.join("checkpoint-00000000000000000002.bin")).unwrap();
        assert_eq!(
            PersistentCheckpointStore::open_existing(&middle, &keys).err(),
            Some(LedgerError::Stale)
        );

        let truncated = root.join("truncated");
        let mut authority =
            PersistentCheckpointStore::bootstrap(&truncated, initial.clone(), &keys).unwrap();
        authority
            .advance(
                TrustedLedgerCheckpoint {
                    revision: 0,
                    owner: owner(),
                    owner_epoch: 7,
                    key_version: 1,
                    final_sequence: 1,
                    generation: 1,
                    stream_digest: [1; 32],
                },
                &keys,
            )
            .unwrap();
        drop(authority);
        let highest = truncated.join("checkpoint-00000000000000000002.bin");
        let bytes = fs::read(&highest).unwrap();
        fs::write(&highest, &bytes[..bytes.len() - 1]).unwrap();
        assert_eq!(
            PersistentCheckpointStore::open_existing(&truncated, &keys).err(),
            Some(LedgerError::Invalid)
        );

        let duplicate = root.join("duplicate");
        let mut authority =
            PersistentCheckpointStore::bootstrap(&duplicate, initial.clone(), &keys).unwrap();
        authority
            .advance(
                TrustedLedgerCheckpoint {
                    revision: 0,
                    owner: owner(),
                    owner_epoch: 7,
                    key_version: 1,
                    final_sequence: 1,
                    generation: 1,
                    stream_digest: [1; 32],
                },
                &keys,
            )
            .unwrap();
        drop(authority);
        let first = fs::read(duplicate.join("checkpoint-00000000000000000001.bin")).unwrap();
        fs::write(duplicate.join("checkpoint-00000000000000000002.bin"), first).unwrap();
        assert_eq!(
            PersistentCheckpointStore::open_existing(&duplicate, &keys).err(),
            Some(LedgerError::Stale)
        );

        let other_keys = SyntheticKeyring::synthetic(2, b"different-checkpoint-key").unwrap();
        let wrong_key = root.join("wrong-key");
        PersistentCheckpointStore::bootstrap(&wrong_key, initial, &keys).unwrap();
        assert_eq!(
            PersistentCheckpointStore::open_existing(&wrong_key, &other_keys).err(),
            Some(LedgerError::MissingKey)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn checkpoint_directory_symlink_replacement_is_refused_on_refresh() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!(
            "serea-gate-c-checkpoint-path-replacement-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let directory = root.join("authority");
        let keys = SyntheticKeyring::synthetic(1, b"checkpoint-path-key").unwrap();
        let mut authority = PersistentCheckpointStore::bootstrap(
            &directory,
            TrustedLedgerCheckpoint {
                revision: 0,
                owner: owner(),
                owner_epoch: 7,
                key_version: 1,
                final_sequence: 0,
                generation: 0,
                stream_digest: Sha256::digest([]).into(),
            },
            &keys,
        )
        .unwrap();
        fs::rename(&directory, root.join("original")).unwrap();
        let replacement = root.join("replacement");
        fs::create_dir(&replacement).unwrap();
        symlink(&replacement, &directory).unwrap();
        assert_eq!(authority.refresh(&keys), Err(LedgerError::Invalid));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn checkpoint_reopen_rejects_oversized_revision_before_allocation() {
        let root = std::env::temp_dir().join(format!(
            "serea-gate-c-checkpoint-bound-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let directory = root.join("authority");
        fs::create_dir(&directory).unwrap();
        fs::write(
            directory.join("checkpoint-00000000000000000001.bin"),
            vec![0; MAX_CHECKPOINT_BYTES as usize + 1],
        )
        .unwrap();
        let keys = SyntheticKeyring::synthetic(1, b"checkpoint-bound-test").unwrap();
        assert_eq!(
            PersistentCheckpointStore::open_existing(&directory, &keys).err(),
            Some(LedgerError::TooLarge)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn checkpoint_revision_count_and_advance_ceiling_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "serea-gate-c-checkpoint-revision-cap-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let directory = root.join("authority");
        fs::create_dir(&directory).unwrap();
        for revision in 1..=MAX_CHECKPOINT_REVISIONS + 1 {
            fs::write(directory.join(format!("checkpoint-{revision:020}.bin")), []).unwrap();
        }
        let keys = SyntheticKeyring::synthetic(1, b"checkpoint-cap-key").unwrap();
        assert_eq!(
            PersistentCheckpointStore::open_existing(&directory, &keys).err(),
            Some(LedgerError::TooLarge)
        );
        fs::remove_dir_all(&root).unwrap();

        let mut authority = PersistentCheckpointStore::bootstrap(
            std::env::temp_dir().join(format!(
                "serea-p7-cap-{}",
                NEXT.fetch_add(1, Ordering::Relaxed)
            )),
            TrustedLedgerCheckpoint {
                revision: 0,
                owner: owner(),
                owner_epoch: 7,
                key_version: 1,
                final_sequence: 0,
                generation: 0,
                stream_digest: Sha256::digest([]).into(),
            },
            &keys,
        )
        .unwrap();
        authority.current.revision = MAX_CHECKPOINT_REVISIONS as u64;
        let next = TrustedLedgerCheckpoint {
            revision: 0,
            owner: owner(),
            owner_epoch: 7,
            key_version: 1,
            final_sequence: 1,
            generation: 1,
            stream_digest: [1; 32],
        };
        assert_eq!(authority.advance(next, &keys), Err(LedgerError::Stale));
        authority.current.revision = u64::MAX;
        let overflow = TrustedLedgerCheckpoint {
            revision: 0,
            owner: owner(),
            owner_epoch: 7,
            key_version: 1,
            final_sequence: 1,
            generation: 1,
            stream_digest: [2; 32],
        };
        assert_eq!(
            authority.advance(overflow, &keys),
            Err(LedgerError::Generation)
        );
        fs::remove_dir_all(authority.directory).unwrap();
    }

    #[test]
    fn ledger_verifier_rejects_stream_over_hard_byte_limit() {
        let keys = SyntheticKeyring::synthetic(1, b"ledger-bound-test").unwrap();
        assert_eq!(
            verify_stream(&vec![0; MAX_LEDGER_BYTES + 1], &keys),
            Err(LedgerError::TooLarge)
        );
    }

    #[test]
    fn ledger_verifier_rejects_frame_count_over_hard_limit() {
        let keys = SyntheticKeyring::synthetic(1, b"ledger-frame-cap-key").unwrap();
        let mut bytes = Vec::new();
        let mut sequence = 0u64;
        for operation in 1..=((MAX_LEDGER_FRAMES / 4) + 1) as u64 {
            for (index, kind) in [
                FrameKind::Intent,
                FrameKind::PrimaryApplied,
                FrameKind::Finalized,
                FrameKind::Reconciled,
            ]
            .into_iter()
            .enumerate()
            {
                sequence += 1;
                let mut frame = frame(
                    sequence,
                    if index == 3 {
                        operation * 2
                    } else {
                        operation * 2 - 1
                    },
                    1,
                    kind,
                );
                frame.operation_id = format!("frame-cap-{operation}");
                bytes.extend_from_slice(&encode(&frame, &keys).unwrap());
            }
        }
        assert!(bytes.len() <= MAX_LEDGER_BYTES);
        assert_eq!(verify_stream(&bytes, &keys), Err(LedgerError::TooLarge));
    }

    #[cfg(unix)]
    #[test]
    fn ledger_guard_rejects_symlinks_and_keeps_the_opened_inode() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!(
            "serea-gate-c-paths-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let target = root.join("target.ledger");
        fs::write(&target, []).unwrap();
        let link = root.join("linked.ledger");
        symlink(&target, &link).unwrap();
        assert_eq!(
            LedgerFileGuard::acquire_existing(&link).err(),
            Some(LedgerError::Invalid)
        );

        let data = root.join("stable.ledger");
        fs::write(&data, []).unwrap();
        let guard = LedgerFileGuard::acquire_existing(&data).unwrap();
        let original = root.join("stable-original.ledger");
        fs::rename(&data, &original).unwrap();
        fs::write(&data, []).unwrap();
        let keys = SyntheticKeyring::synthetic(1, b"path-test-generated-key").unwrap();
        guard
            .append_sync(&frame(1, 1, 1, FrameKind::Intent), &keys)
            .unwrap();
        assert!(guard.read_verified(&keys).is_ok());
        assert!(
            fs::read(&data).unwrap().is_empty(),
            "the active guard remains bound to the opened inode"
        );
        drop(guard);

        let sidecar_ledger = root.join("sidecar.ledger");
        fs::write(&sidecar_ledger, []).unwrap();
        let lock_target = root.join("lock-target");
        fs::write(&lock_target, []).unwrap();
        let mut lock_name = sidecar_ledger.as_os_str().to_os_string();
        lock_name.push(".lock");
        symlink(lock_target, PathBuf::from(lock_name)).unwrap();
        assert_eq!(
            LedgerFileGuard::acquire_existing(sidecar_ledger).err(),
            Some(LedgerError::Invalid)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn simulated_short_write_write_zero_and_enospc_never_complete_a_frame() {
        let root = std::env::temp_dir().join(format!(
            "serea-gate-c-write-faults-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let keys = SyntheticKeyring::synthetic(1, b"write-fault-test-key").unwrap();
        for (index, fault) in [
            SimulatedWriteFault::WriteZero,
            SimulatedWriteFault::NoSpace,
            SimulatedWriteFault::Short,
        ]
        .into_iter()
        .enumerate()
        {
            let path = root.join(format!("fault-{index}.ledger"));
            let guard = LedgerFileGuard::acquire(&path).unwrap();
            assert_eq!(
                guard.append_sync_simulated(&frame(1, 1, 1, FrameKind::Intent), &keys, fault),
                Err(LedgerError::Io)
            );
            match fault {
                SimulatedWriteFault::Short => assert!(guard.read_verified(&keys).is_err()),
                SimulatedWriteFault::WriteZero | SimulatedWriteFault::NoSpace => {
                    assert!(guard.read_verified(&keys).unwrap().is_empty())
                }
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}
