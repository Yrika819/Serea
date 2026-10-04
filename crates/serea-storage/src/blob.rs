use rusqlite::{OptionalExtension, params};
use serea_protocol::{DataClass, Digest, canonicalize};
use sha2::{Digest as _, Sha256};

use crate::{AtRestProtection, StoreError, Tx};

/// Content identifier, not access authority. Identity is canonical plaintext
/// SHA-256 plus the exact data class; it contains no payload or protection material.
/// Fields cannot be read or replaced directly:
/// ```compile_fail
/// use serea_storage::BlobRef;
/// fn bypass(blob: &BlobRef) { let _ = &blob.digest; }
/// ```
/// ```compile_fail
/// use serea_storage::BlobRef;
/// fn bypass(blob: &BlobRef) { let _ = blob.class; }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlobRef {
    digest: Digest,
    class: DataClass,
}

impl BlobRef {
    /// Builds a reference from an already-validated protocol Digest and class.
    /// Caller-created refs are harmless: lookup never searches by digest alone.
    pub fn new(digest: Digest, class: DataClass) -> Self {
        Self { digest, class }
    }

    /// SHA-256 of SCJ-1 canonical plaintext, never of protected bytes.
    pub fn digest(&self) -> &Digest {
        &self.digest
    }

    /// Exact class required by composite lookup.
    pub fn class(&self) -> DataClass {
        self.class
    }
}

fn canonical_bytes(bytes: &[u8]) -> Result<Vec<u8>, StoreError> {
    let text = std::str::from_utf8(bytes).map_err(|_| StoreError::CanonicalJson)?;
    canonicalize(text).map_err(|_| StoreError::CanonicalJson)
}

fn plaintext_digest(canonical: &[u8]) -> Digest {
    use std::fmt::Write;
    let mut value = String::from("sha256:");
    for byte in Sha256::digest(canonical) {
        write!(value, "{byte:02x}").expect("writing to String cannot fail");
    }
    Digest::new(value).expect("SHA-256 encoder produces a protocol Digest")
}

impl Tx<'_> {
    fn protection_for(
        &self,
        class: DataClass,
    ) -> Result<Option<&dyn AtRestProtection>, StoreError> {
        match class {
            DataClass::Public | DataClass::Personal => Ok(None),
            DataClass::Private => self
                .protection
                .as_deref()
                .map(Some)
                .ok_or(StoreError::AtRestProtectionUnavailable),
            DataClass::Secret | DataClass::Credential => Err(StoreError::ClassRefused),
        }
    }

    /// Stores original UTF-8 JSON bytes after SCJ-1 canonicalization. Preserve
    /// raw input: serializing an already-lossy Value cannot restore duplicate keys.
    /// No arbitrary binary or caller-supplied digest is accepted.
    ///
    /// Class dispatch precedes dedupe. Existing content must be readable and
    /// digest-valid before reuse. PRIVATE needs the injected backend even on a
    /// dedupe hit; SECRET/CREDENTIAL always refuse. This method does not commit
    /// or attach a parent reference. Propagate errors from the transact closure
    /// to roll back surrounding writes; parent transitions belong to P2F.
    ///
    /// No blob write exists outside the transaction capability:
    /// ```compile_fail
    /// use serea_protocol::DataClass;
    /// use serea_storage::Store;
    /// fn bypass(store: &Store) { store.put_blob(b"{}", DataClass::Public).unwrap(); }
    /// ```
    pub fn put_blob(
        &mut self,
        original_json: &[u8],
        class: DataClass,
    ) -> Result<BlobRef, StoreError> {
        self.ensure_active()?;
        self.protection_for(class)?;
        let canonical = canonical_bytes(original_json)?;
        let blob = BlobRef::new(plaintext_digest(&canonical), class);
        match self.get_blob(&blob) {
            Ok(_) => return Ok(blob),
            Err(StoreError::BlobMissing) => {}
            Err(error) => return Err(error),
        }
        let (content, marker) = match self.protection_for(class)? {
            Some(backend) => (
                backend
                    .protect(&canonical)
                    .map_err(|_| StoreError::AtRestProtectionFailed)?,
                "AT_REST",
            ),
            None => (canonical, "NONE"),
        };
        let size = i64::try_from(content.len()).map_err(|_| StoreError::Sqlite)?;
        self.inner.execute(
            "INSERT INTO blobs(digest,data_class_rank,protection,size_bytes,content) VALUES (?1,?2,?3,?4,?5)",
            params![blob.digest.as_str(), class.rank(), marker, size, content],
        )?;
        Ok(blob)
    }

    /// Resolves only the exact (plaintext digest, class) row. Checks marker and
    /// stored-content length, unprotects PRIVATE, then canonicalizes and verifies
    /// SHA-256 before returning canonical plaintext. Corruption never returns bytes.
    /// This is semantic JSON integrity, not ciphertext authentication or tamper
    /// evidence against a local file writer. No general PRIVATE row support.
    ///
    /// No blob read exists directly on Store:
    /// ```compile_fail
    /// use serea_storage::{BlobRef, Store};
    /// fn bypass(store: &Store, blob: &BlobRef) { store.get_blob(blob).unwrap(); }
    /// ```
    pub fn get_blob(&mut self, blob: &BlobRef) -> Result<Vec<u8>, StoreError> {
        self.ensure_active()?;
        let backend = self.protection_for(blob.class)?;
        // Keep row decoding failures distinct from execution/connection failures.
        let stored = self.inner.query_row(
            "SELECT content,protection,data_class_rank,size_bytes FROM blobs WHERE digest=?1 AND data_class_rank=?2",
            params![blob.digest.as_str(), blob.class.rank()],
            |row| {
                Ok((|| -> Result<(Vec<u8>, String, i64, i64), StoreError> {
                    let corrupt = |_| StoreError::BlobCorrupt;
                    Ok((row.get(0).map_err(corrupt)?, row.get(1).map_err(corrupt)?, row.get(2).map_err(corrupt)?, row.get(3).map_err(corrupt)?))
                })())
            },
        ).optional()?.ok_or(StoreError::BlobMissing)??;
        let (content, marker, rank, size) = stored;
        let expected = if blob.class == DataClass::Private {
            "AT_REST"
        } else {
            "NONE"
        };
        if rank != i64::from(blob.class.rank())
            || marker != expected
            || i64::try_from(content.len()).ok() != Some(size)
        {
            return Err(StoreError::BlobCorrupt);
        }
        let plaintext = match backend {
            Some(backend) => backend
                .unprotect(&content)
                .map_err(|_| StoreError::AtRestProtectionFailed)?,
            None => content,
        };
        let canonical = canonical_bytes(&plaintext).map_err(|_| StoreError::BlobCorrupt)?;
        if plaintext_digest(&canonical) != blob.digest {
            return Err(StoreError::BlobCorrupt);
        }
        Ok(canonical)
    }
}
