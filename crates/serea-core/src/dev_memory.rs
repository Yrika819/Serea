//! Synthetic Core-to-P7 Memory adapter for isolated development fixtures.
//!
//! This adapter deliberately consumes the existing storage-owned
//! `P7DevCompletedConversation` contract. Core has no shared Chat/Conversation
//! DTO yet, so this module does not introduce one. It is not a production
//! adapter and must only receive synthetic fixture values.

use std::path::{Path, PathBuf};

use serea_protocol::{Clock, EpochMillis};
use serea_storage::p7_gate_c::LedgerError;
use serea_storage::{
    MemoryRetrievalPage, P7DevCompletedConversation, P7DevConnection, P7DevExtractionError,
    P7DevForgetStatus, P7DevTurnClass,
};
use sha2::{Digest, Sha256};

/// Independent development adapter around the C-08 synthetic P7 connection.
///
/// It does not alter Core initialization, Store ownership, TaskEngine,
/// authorization policy, or WAL handling. The P7 fixture keeps its existing
/// synthetic Owner and authentication boundary.
pub struct DevMemoryAdapter {
    connection: P7DevConnection,
    database_path: PathBuf,
    ledger_path: PathBuf,
    checkpoint_path: PathBuf,
}

/// The original completed Chat result and best-effort synthetic Memory result.
///
/// Keeping `chat_result` generic ensures extraction errors cannot replace or
/// invalidate the completed Chat result. This type says nothing about failed,
/// cancelled, or in-progress chats; callers must invoke the adapter only from
/// the completed-result path and must pass synthetic fixture conversations.
pub struct CompletedChatMemory<R> {
    /// The completed Chat result, returned unchanged even when extraction fails.
    pub chat_result: R,
    /// Stable, content-derived extraction request identifier for this fixture.
    pub extraction_request_id: String,
    /// The existing P7 admission summary's inserted count, or its extraction error.
    pub extraction: Result<usize, P7DevExtractionError>,
}

impl DevMemoryAdapter {
    /// Opens the dedicated synthetic P7 fixture at these paths.
    ///
    /// Callers must provide new temporary fixture paths. This does not open or
    /// migrate an ordinary Core database.
    pub fn open(
        database_path: impl AsRef<Path>,
        ledger_path: impl AsRef<Path>,
        checkpoint_path: impl AsRef<Path>,
    ) -> Result<Self, serea_storage::StoreError> {
        let database_path = database_path.as_ref().to_path_buf();
        let ledger_path = ledger_path.as_ref().to_path_buf();
        let checkpoint_path = checkpoint_path.as_ref().to_path_buf();
        let connection = P7DevConnection::open(&database_path, &ledger_path, &checkpoint_path)?;
        Ok(Self {
            connection,
            database_path,
            ledger_path,
            checkpoint_path,
        })
    }

    /// Reopens the same synthetic fixture through the existing recovery barrier.
    pub fn reopen(&self) -> Result<Self, serea_storage::StoreError> {
        let connection = P7DevConnection::reopen(
            &self.database_path,
            &self.ledger_path,
            &self.checkpoint_path,
        )?;
        Ok(Self {
            connection,
            database_path: self.database_path.clone(),
            ledger_path: self.ledger_path.clone(),
            checkpoint_path: self.checkpoint_path.clone(),
        })
    }

    /// Composes a successful synthetic Chat result with the C-08 completed
    /// conversation fixture. The C-08 Mock extractor and existing storage
    /// admission/lease, provenance, persistence, and Forget paths remain
    /// authoritative.
    ///
    /// Failed or cancelled Chat results pass through unchanged and do not run
    /// extraction. An unfinished Chat has no successful result to pass. This
    /// method still accepts only synthetic fixture conversations; that type is
    /// not a production conversation contract.
    pub fn on_chat_result<R, E>(
        &self,
        chat_result: Result<R, E>,
        conversation: &P7DevCompletedConversation,
        clock: &dyn Clock,
    ) -> Result<CompletedChatMemory<R>, E> {
        let chat_result = chat_result?;
        Ok(self.extract_completed(chat_result, conversation, clock))
    }

    fn extract_completed<R>(
        &self,
        chat_result: R,
        conversation: &P7DevCompletedConversation,
        clock: &dyn Clock,
    ) -> CompletedChatMemory<R> {
        let extraction_request_id = stable_extraction_request_id(conversation);
        let extraction =
            self.connection
                .extract_and_save_completed(conversation, &extraction_request_id, clock);
        CompletedChatMemory {
            chat_result,
            extraction_request_id,
            extraction,
        }
    }

    /// Retrieves bounded lexical results from the existing synthetic fixture.
    pub fn retrieve(
        &self,
        class: P7DevTurnClass,
        query: &str,
        now: EpochMillis,
        context_budget_bytes: usize,
    ) -> Result<MemoryRetrievalPage, serea_storage::StoreError> {
        self.connection
            .retrieve(class, query, now, context_budget_bytes)
    }

    /// Applies the existing synthetic authenticated Forget operation.
    pub fn forget(
        &self,
        operation_id: &str,
        memory_id: &str,
    ) -> Result<P7DevForgetStatus, LedgerError> {
        self.connection.forget(operation_id, memory_id)
    }
}

fn stable_extraction_request_id(conversation: &P7DevCompletedConversation) -> String {
    let mut digest = Sha256::new();
    digest.update(b"serea-core-p7-dev-extraction-request-v1");
    hash_field(&mut digest, conversation.conversation_id.as_bytes());
    for turn in &conversation.turns {
        hash_field(&mut digest, turn.turn_id.as_bytes());
        hash_field(&mut digest, turn.text.as_bytes());
        hash_field(
            &mut digest,
            match turn.role {
                serea_storage::P7DevTurnRole::User => b"user",
                serea_storage::P7DevTurnRole::Assistant => b"assistant",
            },
        );
        hash_field(
            &mut digest,
            match turn.class {
                P7DevTurnClass::Public => b"public",
                P7DevTurnClass::Personal => b"personal",
            },
        );
    }
    let encoded = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("core-p7-dev-{encoded}")
}

fn hash_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value);
}
