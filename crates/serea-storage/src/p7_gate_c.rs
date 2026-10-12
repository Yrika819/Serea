//! Disabled Gate C development contracts for P7 owner identity and ledger frames.
//!
//! This module is behind the explicit `p7-gate-c` feature, which is rejected in
//! optimized builds. It has no Core composition entry point, OS authenticator,
//! SQLite migration, or production key source. Ledger append is single-writer
//! only: callers must provide external cross-process serialization. `sync_all`
//! is the strongest portable std primitive used here and is not a power-loss or
//! directory-entry durability guarantee.

use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

pub const MAX_FRAME_BYTES: usize = 1024;
pub const MAX_PAYLOAD_BYTES: usize = 768;
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

/// Append and flush one record. This primitive assumes the caller already
/// holds a verified exclusive cross-process lock; it does not provide one.
pub fn append_sync(
    path: &Path,
    frame: &LedgerFrame,
    keys: &SyntheticKeyring,
) -> Result<(), LedgerError> {
    let encoded = encode(frame, keys)?;
    let mut existing = Vec::new();
    if path.exists() {
        File::open(path)
            .map_err(|_| LedgerError::Io)?
            .take((MAX_FRAME_BYTES as u64 * 10_001) + 1)
            .read_to_end(&mut existing)
            .map_err(|_| LedgerError::Io)?;
        if existing.len() > MAX_FRAME_BYTES * 10_001 {
            return Err(LedgerError::TooLarge);
        }
    }
    let records = verify_stream(&existing, keys)?;
    if frame.sequence != records.len() as u64 + 1 {
        return Err(LedgerError::Sequence);
    }
    existing.extend_from_slice(&encoded);
    verify_stream(&existing, keys)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|_| LedgerError::Io)?;
    file.write_all(&encoded).map_err(|_| LedgerError::Io)?;
    file.sync_all().map_err(|_| LedgerError::Io)
}

pub fn read_verified(
    path: &Path,
    keys: &SyntheticKeyring,
) -> Result<Vec<LedgerFrame>, LedgerError> {
    let file = File::open(path).map_err(|_| LedgerError::Io)?;
    let mut bytes = Vec::new();
    file.take((MAX_FRAME_BYTES as u64 * 10_001) + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| LedgerError::Io)?;
    if bytes.len() > MAX_FRAME_BYTES * 10_001 {
        return Err(LedgerError::TooLarge);
    }
    verify_stream(&bytes, keys)
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
    fn gate_c_feature_does_not_select_or_apply_experimental_migration_0009() {
        assert_eq!(crate::Migrations::LATEST, 8);
        assert!(
            crate::Migrations::embedded()
                .iter()
                .all(|migration| migration.version <= 8)
        );
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
        assert_eq!(verify_stream(&encoded.concat(), &keys).unwrap().len(), 5);

        let missing_chunk = [
            encode(&intent, &keys).unwrap(),
            encode(&applied, &keys).unwrap(),
        ];
        assert!(verify_stream(&missing_chunk.concat(), &keys).is_err());
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
        append_sync(&path, &frame(1, 1, 1, FrameKind::Intent), &keys).unwrap();
        append_sync(&path, &frame(2, 1, 1, FrameKind::PrimaryApplied), &keys).unwrap();
        append_sync(&path, &frame(3, 1, 1, FrameKind::Finalized), &keys).unwrap();
        append_sync(&path, &frame(4, 2, 1, FrameKind::Reconciled), &keys).unwrap();
        assert_eq!(read_verified(&path, &keys).unwrap().len(), 4);
        assert_eq!(
            append_sync(&path, &frame(4, 2, 1, FrameKind::Reconciled), &keys),
            Err(LedgerError::Sequence)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
