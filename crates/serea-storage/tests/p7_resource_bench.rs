#![cfg(all(feature = "p7-experimental", feature = "p7-gate-c"))]
//! Manual synthetic history measurements. These are ignored by default and
//! use wall time only because they measure operation latency.
use serea_storage::p7_gate_c::{
    FrameKind, LedgerFileGuard, LedgerFrame, OwnerId, PersistentCheckpointStore, SyntheticKeyring,
    TrustedLedgerCheckpoint, verify_stream,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

static NEXT: AtomicU64 = AtomicU64::new(1);
fn owner() -> OwnerId {
    OwnerId::parse("own_00000000000000000000000000000001").unwrap()
}
fn scratch(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "serea-p7-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    path
}
fn frame(sequence: u64, generation: u64, kind: FrameKind, operation: u64) -> LedgerFrame {
    LedgerFrame {
        sequence,
        generation,
        key_version: 1,
        chunk_index: 0,
        chunk_count: 0,
        target_count: 0,
        closure_digest: [0; 32],
        owner: owner(),
        operation_id: format!("op-{operation}"),
        kind,
        payload: b"synthetic-target".to_vec(),
    }
}

#[test]
#[ignore = "manual resource measurement; run with --ignored --nocapture"]
#[allow(clippy::disallowed_methods)]
fn measure_ledger_append_replay_and_history_growth() {
    let root = scratch("ledger-measure");
    let keys = SyntheticKeyring::synthetic(1, b"ledger-measure-test").unwrap();
    let guard = LedgerFileGuard::acquire(root.join("ledger")).unwrap();
    let mut sequence = 0_u64;
    for operations in [16_u64, 64, 128] {
        let start = Instant::now();
        for operation in ((sequence / 4) + 1)..=operations {
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
                let generation = if index == 3 {
                    operation * 2
                } else {
                    operation * 2 - 1
                };
                guard
                    .append_sync(&frame(sequence, generation, kind, operation), &keys)
                    .unwrap();
            }
        }
        let append_elapsed = start.elapsed();
        let bytes = guard.read_bytes().unwrap();
        let replay_start = Instant::now();
        let frames = verify_stream(&bytes, &keys).unwrap();
        let replay_elapsed = replay_start.elapsed();
        eprintln!(
            "ledger_measure operations={operations} frames={} bytes={} append_ms={} replay_ms={}",
            frames.len(),
            bytes.len(),
            append_elapsed.as_millis(),
            replay_elapsed.as_millis()
        );
    }
    drop(guard);
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[ignore = "manual resource measurement; run with --ignored --nocapture"]
#[allow(clippy::disallowed_methods)]
fn measure_checkpoint_advance_and_reopen_history_growth() {
    let root = scratch("checkpoint-measure");
    let directory = root.join("authority");
    let keys = SyntheticKeyring::synthetic(1, b"checkpoint-measure-key").unwrap();
    let initial = TrustedLedgerCheckpoint {
        revision: 0,
        owner: owner(),
        owner_epoch: 7,
        key_version: 1,
        final_sequence: 0,
        generation: 0,
        stream_digest: Sha256::digest([]).into(),
    };
    let mut authority = PersistentCheckpointStore::bootstrap(&directory, initial, &keys).unwrap();
    for revisions in [16_u64, 64, 128] {
        let start = Instant::now();
        while authority.current().revision < revisions {
            let revision = authority.current().revision + 1;
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
        let advance_elapsed = start.elapsed();
        let reopen_start = Instant::now();
        drop(PersistentCheckpointStore::open_existing(&directory, &keys).unwrap());
        let reopen_elapsed = reopen_start.elapsed();
        let bytes: u64 = fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().metadata().unwrap().len())
            .sum();
        eprintln!(
            "checkpoint_measure revisions={revisions} file_bytes={bytes} advance_ms={} reopen_ms={}",
            advance_elapsed.as_millis(),
            reopen_elapsed.as_millis()
        );
    }
    drop(authority);
    fs::remove_dir_all(root).unwrap();
}
