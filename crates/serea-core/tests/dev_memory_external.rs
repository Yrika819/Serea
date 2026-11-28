#![cfg(feature = "p7-memory-integration-dev")]

//! Verifies the opt-in adapter through the public crate dependency surface.

use serea_core::dev_memory::DevMemoryAdapter;
use serea_storage::{
    MemoryClass, P7DevCompletedConversation, P7DevExtractionError, P7DevForgetStatus, P7DevTurn,
    P7DevTurnClass, P7DevTurnRole,
};
use serea_testkit::TestClock;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "serea-core-p7-dev-external-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn database(&self) -> PathBuf {
        self.0.join("memory.sqlite")
    }
    fn ledger(&self) -> PathBuf {
        self.0.join("forget.ledger")
    }
    fn checkpoint(&self) -> PathBuf {
        self.0.join("checkpoint")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn conversation() -> P7DevCompletedConversation {
    P7DevCompletedConversation {
        conversation_id: "synthetic-external-core-chat-1".into(),
        turns: vec![P7DevTurn {
            turn_id: "synthetic-external-core-turn-1".into(),
            role: P7DevTurnRole::User,
            class: P7DevTurnClass::Personal,
            text: "Synthetic external crate memory marker in Kyoto".into(),
        }],
    }
}

#[test]
fn external_crate_can_use_dev_adapter_without_promoting_fixture_to_production_contract() {
    let scratch = Scratch::new();
    let clock = TestClock::at_epoch();
    let adapter =
        DevMemoryAdapter::open(scratch.database(), scratch.ledger(), scratch.checkpoint()).unwrap();
    let conversation = conversation();

    // Err is the only representation available here for failed, cancelled, and unfinished
    // calls. The fixture input alone cannot turn those outcomes into completed chats.
    for outcome in ["failed", "cancelled", "unfinished"] {
        assert!(
            adapter
                .on_chat_result::<(), _>(Err(outcome), &conversation, &clock)
                .is_err()
        );
    }
    assert!(
        adapter
            .retrieve(
                P7DevTurnClass::Personal,
                "Kyoto",
                clock.now_ms().unwrap(),
                4096
            )
            .unwrap()
            .entries
            .is_empty()
    );

    let first = adapter
        .on_chat_result(
            Ok::<_, ()>(String::from("synthetic completed response")),
            &conversation,
            &clock,
        )
        .unwrap();
    assert_eq!(first.chat_result, "synthetic completed response");
    assert_eq!(first.extraction.as_ref().unwrap(), &1);
    let request_id = first.extraction_request_id.clone();
    assert!(request_id.starts_with("core-p7-dev-"));

    let replay = adapter
        .on_chat_result(
            Ok::<_, ()>(String::from("synthetic replay response")),
            &conversation,
            &clock,
        )
        .unwrap();
    assert_eq!(replay.extraction_request_id, request_id);
    assert_eq!(replay.extraction.as_ref().unwrap(), &1);

    let failed_extraction = adapter
        .on_chat_result(
            Ok::<_, ()>(String::from("chat survives mock extraction failure")),
            &P7DevCompletedConversation {
                conversation_id: "synthetic-empty-chat".into(),
                turns: vec![],
            },
            &clock,
        )
        .unwrap();
    assert_eq!(
        failed_extraction.chat_result,
        "chat survives mock extraction failure"
    );
    assert_eq!(
        failed_extraction.extraction,
        Err(P7DevExtractionError::NoEligibleTurns)
    );

    let reopened = adapter.reopen().unwrap();
    let found = reopened
        .retrieve(
            P7DevTurnClass::Personal,
            "Kyoto",
            clock.now_ms().unwrap(),
            4096,
        )
        .unwrap();
    assert_eq!(found.entries.len(), 1);
    assert_eq!(found.entries[0].record.class, MemoryClass::Personal);
    assert_eq!(
        found.entries[0].provenance.source_class,
        MemoryClass::Personal
    );
    assert!(
        found.entries[0]
            .provenance
            .source_id
            .contains("synthetic-external-core-turn-1")
    );

    let memory_id = found.entries[0].record.memory_id.clone();
    assert_eq!(
        reopened
            .forget("synthetic-external-forget-1", &memory_id)
            .unwrap(),
        P7DevForgetStatus::Completed
    );
    let after_forget = reopened.reopen().unwrap();
    assert!(
        after_forget
            .retrieve(
                P7DevTurnClass::Personal,
                "Kyoto",
                clock.now_ms().unwrap(),
                4096
            )
            .unwrap()
            .entries
            .is_empty()
    );

    let refused = after_forget
        .on_chat_result(
            Ok::<_, ()>(String::from("chat preserved after forget")),
            &conversation,
            &clock,
        )
        .unwrap();
    assert_eq!(refused.chat_result, "chat preserved after forget");
    assert!(refused.extraction.is_err());
    assert!(
        after_forget
            .retrieve(
                P7DevTurnClass::Personal,
                "Kyoto",
                clock.now_ms().unwrap(),
                4096
            )
            .unwrap()
            .entries
            .is_empty()
    );
}
