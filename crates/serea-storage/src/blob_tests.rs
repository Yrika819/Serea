//! P2D tests use the production Store and migration. SQL access is crate-private.

use crate::{AtRestProtection, AtRestProtectionError, BlobRef, Store, StoreError};
use rusqlite::params;
use serea_protocol::{Clock, DataClass, EpochMillis, ProtocolError, digest_of};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(0)
    }
}

fn count(store: &Store) -> i64 {
    store
        .conn
        .lock()
        .unwrap()
        .query_row("SELECT count(*) FROM blobs", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn private_without_a_backend_is_refused_and_writes_nothing() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    assert_eq!(
        store.transact(|tx| tx.put_blob(br#"{"private":"synthetic"}"#, DataClass::Private)),
        Err(StoreError::AtRestProtectionUnavailable)
    );
    assert_eq!(count(&store), 0);
}

#[test]
fn public_canonical_blob_round_trip() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let blob = store
        .transact(|tx| tx.put_blob(br#"{ "z":2, "a":1 }"#, DataClass::Public))
        .unwrap();
    assert_eq!(blob.digest(), &digest_of(r#"{"a":1,"z":2}"#).unwrap());
    assert_eq!(blob.class(), DataClass::Public);
    assert_eq!(
        store.transact(|tx| tx.get_blob(&blob)).unwrap(),
        br#"{"a":1,"z":2}"#
    );
    assert_eq!(count(&store), 1);
}

#[test]
fn secret_is_refused_and_writes_nothing() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    assert_eq!(
        store.transact(|tx| tx.put_blob(b"{}", DataClass::Secret)),
        Err(StoreError::ClassRefused)
    );
    assert_eq!(count(&store), 0);
}

const SENTINEL: &str = "PRIVATE-payload-backend-diagnostic-do-not-log";

// NOT ENCRYPTION. NOT SECURITY. NEVER PRODUCTION. Synthetic cfg(test) wiring
// only. The prefix expands stored length and distinguishes incompatible doubles.
struct TestProtection {
    identity: u8,
    fail_protect: bool,
    fail_unprotect: bool,
    protect_calls: AtomicUsize,
    unprotect_calls: AtomicUsize,
}
impl TestProtection {
    fn new(identity: u8) -> Self {
        Self {
            identity,
            fail_protect: false,
            fail_unprotect: false,
            protect_calls: AtomicUsize::new(0),
            unprotect_calls: AtomicUsize::new(0),
        }
    }
    fn diagnostic_failure() -> AtRestProtectionError {
        let underlying = std::io::Error::other(SENTINEL);
        assert!(underlying.to_string().contains(SENTINEL));
        // The backend must discard internal diagnostics at the typed boundary.
        AtRestProtectionError
    }
}
impl AtRestProtection for TestProtection {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, AtRestProtectionError> {
        self.protect_calls.fetch_add(1, Ordering::Relaxed);
        if self.fail_protect {
            return Err(Self::diagnostic_failure());
        }
        let mut output = vec![b'T', b'E', b'S', b'T', self.identity];
        output.extend(plaintext.iter().map(|byte| byte ^ 0xa5));
        Ok(output)
    }
    fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, AtRestProtectionError> {
        self.unprotect_calls.fetch_add(1, Ordering::Relaxed);
        if self.fail_unprotect || !protected.starts_with(&[b'T', b'E', b'S', b'T', self.identity]) {
            return Err(Self::diagnostic_failure());
        }
        Ok(protected[5..].iter().map(|byte| byte ^ 0xa5).collect())
    }
}

fn protected_store(backend: Arc<TestProtection>) -> Store {
    Store::open_in_memory_with_protection(&Fixed, backend).unwrap()
}

fn put(store: &Store, input: &[u8], class: DataClass) -> BlobRef {
    store.transact(|tx| tx.put_blob(input, class)).unwrap()
}

fn stored(store: &Store, blob: &BlobRef) -> (Vec<u8>, String, i64) {
    store.conn.lock().unwrap().query_row(
        "SELECT content,protection,size_bytes FROM blobs WHERE digest=?1 AND data_class_rank=?2",
        params![blob.digest().as_str(), blob.class().rank()],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).unwrap()
}

fn replace_content(store: &Store, blob: &BlobRef, content: &[u8]) {
    store.conn.lock().unwrap().execute(
        "UPDATE blobs SET content=?1,size_bytes=length(?1) WHERE digest=?2 AND data_class_rank=?3",
        params![content, blob.digest().as_str(), blob.class().rank()],
    ).unwrap();
}

fn assert_safe(error: StoreError) {
    use std::error::Error;
    assert!(!format!("{error}").contains(SENTINEL));
    assert!(!format!("{error:?}").contains(SENTINEL));
    assert!(error.source().is_none());
}

#[test]
fn personal_stores_canonical_plaintext_without_invoking_backend() {
    let backend = Arc::new(TestProtection::new(1));
    let store = protected_store(backend.clone());
    for class in [DataClass::Public, DataClass::Personal] {
        let blob = put(&store, b" { \"n\": 7 } ", class);
        assert_eq!(
            stored(&store, &blob),
            (br#"{"n":7}"#.to_vec(), "NONE".into(), 7)
        );
        assert_eq!(
            store.transact(|tx| tx.get_blob(&blob)).unwrap(),
            br#"{"n":7}"#
        );
    }
    assert_eq!(backend.protect_calls.load(Ordering::Relaxed), 0);
    assert_eq!(backend.unprotect_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn canonical_member_order_whitespace_and_legal_escaping_dedupe() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let first = put(&store, br#"{"z":"/","a":"a"}"#, DataClass::Public);
    for input in [
        br#" { "a" : "a", "z" : "/" } "#.as_slice(),
        br#"{"\u0061":"\u0061","z":"\/"}"#.as_slice(),
    ] {
        assert_eq!(put(&store, input, DataClass::Public), first);
    }
    assert_eq!(stored(&store, &first).0, br#"{"a":"a","z":"/"}"#);
    assert_eq!(count(&store), 1);
}

#[test]
fn same_canonical_bytes_same_class_dedupe_to_one_row() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let first = put(&store, b"{}", DataClass::Personal);
    assert_eq!(put(&store, b"{}", DataClass::Personal), first);
    assert_eq!(count(&store), 1);
}

#[test]
fn same_plaintext_digest_public_and_personal_store_two_rows() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let public = put(&store, b"{}", DataClass::Public);
    let personal = put(&store, b"{}", DataClass::Personal);
    assert_eq!(public.digest(), personal.digest());
    assert_ne!(public, personal);
    assert_eq!(count(&store), 2);
    for blob in [&public, &personal] {
        assert_eq!(store.transact(|tx| tx.get_blob(blob)).unwrap(), b"{}");
    }
}

fn refuses_canonical(input: &[u8]) {
    let store = Store::open_in_memory(&Fixed).unwrap();
    for class in [DataClass::Public, DataClass::Personal] {
        let error = store.transact(|tx| tx.put_blob(input, class)).unwrap_err();
        assert_eq!(error, StoreError::CanonicalJson);
        assert_safe(error);
        assert_eq!(count(&store), 0);
    }
}

#[test]
fn malformed_json_refuses_without_echoing_payload() {
    refuses_canonical(format!("{{\"{SENTINEL}\":").as_bytes());
}
#[test]
fn duplicate_decoded_keys_refuse() {
    refuses_canonical(br#"{"a":1,"\u0061":2}"#);
    refuses_canonical(br#"{"outer":{"a":1,"a":2}}"#);
}
#[test]
fn invalid_utf8_refuses() {
    refuses_canonical(&[b'"', 0xff, b'"']);
}
#[test]
fn fractional_numeric_forms_refuse() {
    for input in [b"1.0".as_slice(), b"0.2", b"-1.5"] {
        refuses_canonical(input);
    }
}
#[test]
fn exponent_numeric_forms_refuse() {
    for input in [b"1e0".as_slice(), b"1E+2", b"-1e-1"] {
        refuses_canonical(input);
    }
}
#[test]
fn out_of_domain_and_non_scj_integer_forms_refuse() {
    for input in [
        b"18446744073709551616".as_slice(),
        b"-9223372036854775809",
        b"-0",
        b"+1",
        b"01",
    ] {
        refuses_canonical(input);
    }
}
#[test]
fn arbitrary_binary_is_not_implicitly_wrapped_as_json() {
    refuses_canonical(b"not-json-binary\0");
}
#[test]
fn scj_root_types_integer_endpoints_and_depth_are_preserved() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    for input in [
        b"null".as_slice(),
        b"true",
        b"\"text\"",
        b"[]",
        b"-9223372036854775808",
        b"18446744073709551615",
    ] {
        let blob = put(&store, input, DataClass::Public);
        assert_eq!(store.transact(|tx| tx.get_blob(&blob)).unwrap(), input);
    }
    refuses_canonical(format!("{}0{}", "[".repeat(64), "]".repeat(64)).as_bytes());
}

#[test]
fn credential_is_refused_and_writes_nothing() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    assert_eq!(
        store.transact(|tx| tx.put_blob(b"{}", DataClass::Credential)),
        Err(StoreError::ClassRefused)
    );
    assert_eq!(count(&store), 0);
}
#[test]
fn classification_refuses_before_parsing_or_existing_row_success() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    put(&store, b"{}", DataClass::Public);
    for input in [b"{}".as_slice(), &[0xff]] {
        for class in [DataClass::Secret, DataClass::Credential] {
            assert_eq!(
                store.transact(|tx| tx.put_blob(input, class)),
                Err(StoreError::ClassRefused)
            );
        }
        assert_eq!(
            store.transact(|tx| tx.put_blob(input, DataClass::Private)),
            Err(StoreError::AtRestProtectionUnavailable)
        );
    }
    assert_eq!(count(&store), 1);
}
#[test]
fn refused_secret_and_credential_refs_never_resolve() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    for class in [DataClass::Secret, DataClass::Credential] {
        let blob = BlobRef::new(digest_of("{}").unwrap(), class);
        assert_eq!(
            store.transact(|tx| tx.get_blob(&blob)),
            Err(StoreError::ClassRefused)
        );
    }
}
#[test]
fn missing_and_wrong_digest_refs_return_blob_missing() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    put(&store, b"{}", DataClass::Public);
    let forged = BlobRef::new(digest_of("[]").unwrap(), DataClass::Public);
    assert_eq!(
        store.transact(|tx| tx.get_blob(&forged)),
        Err(StoreError::BlobMissing)
    );
}
#[test]
fn corrupt_modified_content_is_detected_on_read_and_dedupe() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let blob = put(&store, b"{}", DataClass::Public);
    for corrupt in [
        b"[]".as_slice(),
        b"broken",
        br#"{"a":1,"a":2}"#,
        b"1e0",
        &[0xff],
    ] {
        replace_content(&store, &blob, corrupt);
        assert_eq!(
            store.transact(|tx| tx.get_blob(&blob)),
            Err(StoreError::BlobCorrupt)
        );
        assert_eq!(
            store.transact(|tx| tx.put_blob(b"{}", DataClass::Public)),
            Err(StoreError::BlobCorrupt)
        );
        assert_eq!(stored(&store, &blob).0, corrupt);
        assert_eq!(count(&store), 1);
    }
}
#[test]
fn ordinary_read_returns_canonical_plaintext_after_recanonicalization() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let blob = put(&store, br#"{"a":1,"z":2}"#, DataClass::Public);
    replace_content(&store, &blob, br#" { "z" : 2, "a" : 1 } "#);
    assert_eq!(
        store.transact(|tx| tx.get_blob(&blob)).unwrap(),
        br#"{"a":1,"z":2}"#
    );
}
#[test]
fn wrong_class_ref_never_searches_digest_alone() {
    let store = protected_store(Arc::new(TestProtection::new(1)));
    let private = put(&store, b"{}", DataClass::Private);
    for class in [DataClass::Public, DataClass::Personal] {
        let wrong = BlobRef::new(private.digest().clone(), class);
        assert_eq!(
            store.transact(|tx| tx.get_blob(&wrong)),
            Err(StoreError::BlobMissing)
        );
    }
    let public = put(&store, b"{}", DataClass::Public);
    assert_eq!(count(&store), 2);
    assert_ne!(stored(&store, &private).0, stored(&store, &public).0);
    replace_content(&store, &public, b"[]");
    assert_eq!(
        store.transact(|tx| tx.get_blob(&public)),
        Err(StoreError::BlobCorrupt)
    );
    assert_eq!(store.transact(|tx| tx.get_blob(&private)).unwrap(), b"{}");
}
#[test]
fn private_protection_round_trip_plaintext_digest_and_stored_size() {
    let backend = Arc::new(TestProtection::new(1));
    let store = protected_store(backend.clone());
    let blob = put(&store, br#" { "z":2, "a":1 } "#, DataClass::Private);
    let canonical = br#"{"a":1,"z":2}"#;
    let (content, protection, size) = stored(&store, &blob);
    assert_ne!(content, canonical);
    assert_eq!(protection, "AT_REST");
    assert_eq!(size, i64::try_from(content.len()).unwrap());
    assert_eq!(content.len(), canonical.len() + 5);
    assert_eq!(
        blob.digest(),
        &digest_of(std::str::from_utf8(canonical).unwrap()).unwrap()
    );
    assert_eq!(store.transact(|tx| tx.get_blob(&blob)).unwrap(), canonical);
    assert_eq!(put(&store, canonical, DataClass::Private), blob);
    assert_eq!(count(&store), 1);
    assert_eq!(backend.protect_calls.load(Ordering::Relaxed), 1);
}
#[test]
fn private_protect_failure_writes_nothing_and_redacts_diagnostics() {
    let mut backend = TestProtection::new(1);
    backend.fail_protect = true;
    let store = protected_store(Arc::new(backend));
    let error = store
        .transact(|tx| tx.put_blob(format!("\"{SENTINEL}\"").as_bytes(), DataClass::Private))
        .unwrap_err();
    assert_eq!(error, StoreError::AtRestProtectionFailed);
    assert_safe(error);
    assert_eq!(count(&store), 0);
}
#[test]
fn private_unprotect_failure_refuses_read_and_dedupe_without_plaintext_fallback() {
    let mut backend = TestProtection::new(1);
    backend.fail_unprotect = true;
    let store = protected_store(Arc::new(backend));
    let blob = put(&store, b"{}", DataClass::Private);
    for error in [
        store.transact(|tx| tx.get_blob(&blob)).unwrap_err(),
        store
            .transact(|tx| tx.put_blob(b"{}", DataClass::Private))
            .unwrap_err(),
    ] {
        assert_eq!(error, StoreError::AtRestProtectionFailed);
        assert_safe(error);
    }
    assert_ne!(stored(&store, &blob).0, b"{}");
    assert_eq!(count(&store), 1);
}
#[test]
fn private_unprotected_invalid_or_wrong_plaintext_is_blob_corrupt() {
    let backend = Arc::new(TestProtection::new(1));
    let store = protected_store(backend.clone());
    let blob = put(&store, b"{}", DataClass::Private);
    for wrong in [b"[]".as_slice(), b"not-json", b"1.0", &[0xff]] {
        replace_content(&store, &blob, &backend.protect(wrong).unwrap());
        assert_eq!(
            store.transact(|tx| tx.get_blob(&blob)),
            Err(StoreError::BlobCorrupt)
        );
        assert_eq!(
            store.transact(|tx| tx.put_blob(b"{}", DataClass::Private)),
            Err(StoreError::BlobCorrupt)
        );
    }
}
#[test]
fn corrupt_protection_markers_and_sizes_refuse_without_repair() {
    for class in [DataClass::Public, DataClass::Personal, DataClass::Private] {
        for corrupt_size in [false, true] {
            let store = protected_store(Arc::new(TestProtection::new(1)));
            let blob = put(&store, b"{}", class);
            let conn = store.conn.lock().unwrap();
            // Deliberately model a damaged/local-writer row, not a supported API.
            conn.pragma_update(None, "ignore_check_constraints", "ON")
                .unwrap();
            if corrupt_size {
                conn.execute("UPDATE blobs SET size_bytes=size_bytes+1", [])
                    .unwrap();
            } else {
                conn.execute(
                    "UPDATE blobs SET protection=?1",
                    [if class == DataClass::Private {
                        "NONE"
                    } else {
                        "AT_REST"
                    }],
                )
                .unwrap();
            }
            conn.pragma_update(None, "ignore_check_constraints", "OFF")
                .unwrap();
            drop(conn);
            assert_eq!(
                store.transact(|tx| tx.get_blob(&blob)),
                Err(StoreError::BlobCorrupt)
            );
            assert_eq!(
                store.transact(|tx| tx.put_blob(b"{}", class)),
                Err(StoreError::BlobCorrupt)
            );
            assert_eq!(count(&store), 1);
        }
    }
}
#[test]
fn blob_errors_and_backend_error_formatters_are_payload_free() {
    use std::error::Error;
    for error in [
        StoreError::CanonicalJson,
        StoreError::BlobMissing,
        StoreError::BlobCorrupt,
        StoreError::ClassRefused,
        StoreError::AtRestProtectionUnavailable,
        StoreError::AtRestProtectionFailed,
    ] {
        assert_safe(error);
    }
    let error = TestProtection::diagnostic_failure();
    assert!(!format!("{error}").contains(SENTINEL));
    assert!(!format!("{error:?}").contains(SENTINEL));
    assert!(error.source().is_none());
    let blob = BlobRef::new(
        digest_of(&format!("\"{SENTINEL}\"")).unwrap(),
        DataClass::Private,
    );
    assert!(!format!("{blob:?}").contains(SENTINEL));
}
#[test]
fn put_blob_then_closure_error_rolls_back_all_new_blob_rows() {
    let store = protected_store(Arc::new(TestProtection::new(1)));
    let result: Result<(), StoreError> = store.transact(|tx| {
        for class in [DataClass::Public, DataClass::Personal, DataClass::Private] {
            tx.put_blob(b"{}", class)?;
        }
        Err(StoreError::ClassRefused)
    });
    assert_eq!(result, Err(StoreError::ClassRefused));
    assert_eq!(count(&store), 0);
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp {
    dir: PathBuf,
    path: PathBuf,
}
impl Temp {
    fn new() -> Self {
        loop {
            let dir = std::env::temp_dir().join(format!(
                "serea-p2d-blobs-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&dir) {
                Ok(()) => {
                    return Self {
                        path: dir.join("store.sqlite"),
                        dir,
                    };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("test directory creation failed: {error}"),
            }
        }
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}
#[test]
fn committed_public_and_personal_blobs_survive_file_reopen() {
    let temp = Temp::new();
    let store = Store::open(&temp.path, &Fixed).unwrap();
    let public = put(&store, b"{}", DataClass::Public);
    let personal = put(&store, b"[]", DataClass::Personal);
    store.checkpoint_for_close().unwrap();
    drop(store);
    let store = Store::open(&temp.path, &Fixed).unwrap();
    for (blob, expected) in [(&public, b"{}"), (&personal, b"[]")] {
        assert_eq!(store.transact(|tx| tx.get_blob(blob)).unwrap(), expected);
    }
    assert_eq!(count(&store), 2);
}
#[test]
fn private_existing_row_requires_compatible_backend_before_read_or_dedupe() {
    let temp = Temp::new();
    let backend = Arc::new(TestProtection::new(1));
    let store = Store::open_with_protection(&temp.path, &Fixed, backend.clone()).unwrap();
    let blob = put(&store, b"{}", DataClass::Private);
    store.checkpoint_for_close().unwrap();
    drop(store);
    let store = Store::open(&temp.path, &Fixed).unwrap();
    assert_eq!(
        store.transact(|tx| tx.get_blob(&blob)),
        Err(StoreError::AtRestProtectionUnavailable)
    );
    assert_eq!(
        store.transact(|tx| tx.put_blob(b"{}", DataClass::Private)),
        Err(StoreError::AtRestProtectionUnavailable)
    );
    assert_eq!(count(&store), 1);
    drop(store);
    let store =
        Store::open_with_protection(&temp.path, &Fixed, Arc::new(TestProtection::new(2))).unwrap();
    assert_eq!(
        store.transact(|tx| tx.get_blob(&blob)),
        Err(StoreError::AtRestProtectionFailed)
    );
    assert_eq!(
        store.transact(|tx| tx.put_blob(b"{}", DataClass::Private)),
        Err(StoreError::AtRestProtectionFailed)
    );
    assert_eq!(count(&store), 1);
    drop(store);
    let store = Store::open_with_protection(&temp.path, &Fixed, backend).unwrap();
    assert_eq!(store.transact(|tx| tx.get_blob(&blob)).unwrap(), b"{}");
    assert_eq!(put(&store, b"{}", DataClass::Private), blob);
}

// Storage identity must not assume deterministic backend envelopes.
struct VaryingProtection(AtomicU64);
impl AtRestProtection for VaryingProtection {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, AtRestProtectionError> {
        let counter = self.0.fetch_add(1, Ordering::Relaxed);
        let mut output = counter.to_be_bytes().to_vec();
        output.extend(plaintext.iter().map(|byte| byte ^ 0x5a));
        Ok(output)
    }
    fn unprotect(&self, protected: &[u8]) -> Result<Vec<u8>, AtRestProtectionError> {
        let bytes = protected.get(8..).ok_or(AtRestProtectionError)?;
        Ok(bytes.iter().map(|byte| byte ^ 0x5a).collect())
    }
}
#[test]
fn changing_backend_output_does_not_change_plaintext_identity_or_dedupe() {
    let backend = Arc::new(VaryingProtection(AtomicU64::new(0)));
    assert_ne!(
        backend.protect(b"{}").unwrap(),
        backend.protect(b"{}").unwrap()
    );
    let store = Store::open_in_memory_with_protection(&Fixed, backend.clone()).unwrap();
    let blob = put(&store, b"{}", DataClass::Private);
    let before = stored(&store, &blob);
    assert_eq!(put(&store, b" {} ", DataClass::Private), blob);
    assert_eq!(stored(&store, &blob), before);
    assert_eq!(backend.0.load(Ordering::Relaxed), 3);
    assert_eq!(store.transact(|tx| tx.get_blob(&blob)).unwrap(), b"{}");
}

#[test]
fn private_missing_ref_dispatches_backend_absence_before_lookup() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    let private = BlobRef::new(digest_of("{}").unwrap(), DataClass::Private);
    assert_eq!(
        store.transact(|tx| tx.get_blob(&private)),
        Err(StoreError::AtRestProtectionUnavailable)
    );
}

#[test]
fn private_corrupt_metadata_dispatches_backend_absence_before_verification() {
    for (marker, size) in [("NONE", 2), ("AT_REST", 3)] {
        let store = Store::open_in_memory(&Fixed).unwrap();
        let private = BlobRef::new(digest_of("{}").unwrap(), DataClass::Private);
        {
            let conn = store.conn.lock().unwrap();
            // Damaged raw-writer fixture, not supported protection or encryption.
            conn.pragma_update(None, "ignore_check_constraints", "ON")
                .unwrap();
            conn.execute("INSERT INTO blobs(digest,data_class_rank,protection,size_bytes,content) VALUES (?1,2,?2,?3,?4)", params![private.digest().as_str(), marker, size, b"{}".as_slice()]).unwrap();
            conn.pragma_update(None, "ignore_check_constraints", "OFF")
                .unwrap();
        }
        assert_eq!(
            store.transact(|tx| tx.get_blob(&private)),
            Err(StoreError::AtRestProtectionUnavailable)
        );
    }
}

#[test]
fn configured_backend_is_never_called_for_non_private_put_get_or_dedupe() {
    let backend = Arc::new(TestProtection::new(1));
    let store = protected_store(backend.clone());
    for class in [DataClass::Public, DataClass::Personal] {
        let blob = put(&store, b"{}", class);
        assert_eq!(put(&store, b" {} ", class), blob);
        assert_eq!(store.transact(|tx| tx.get_blob(&blob)).unwrap(), b"{}");
    }
    for class in [DataClass::Secret, DataClass::Credential] {
        for input in [b"{}".as_slice(), &[0xff]] {
            assert_eq!(
                store.transact(|tx| tx.put_blob(input, class)),
                Err(StoreError::ClassRefused)
            );
        }
        let refused = BlobRef::new(digest_of("{}").unwrap(), class);
        assert_eq!(
            store.transact(|tx| tx.get_blob(&refused)),
            Err(StoreError::ClassRefused)
        );
    }
    assert_eq!(count(&store), 2);
    assert_eq!(backend.protect_calls.load(Ordering::Relaxed), 0);
    assert_eq!(backend.unprotect_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn caught_refused_puts_make_zero_changes_even_when_transaction_commits() {
    for configured in [false, true] {
        let mut backend = TestProtection::new(1);
        backend.fail_protect = true;
        let store = if configured {
            protected_store(Arc::new(backend))
        } else {
            Store::open_in_memory(&Fixed).unwrap()
        };
        let allowed = store
            .transact(|tx| {
                let allowed = tx.put_blob(b"[]", DataClass::Public)?;
                let changes: i64 = tx
                    .inner
                    .query_row("SELECT total_changes()", [], |r| r.get(0))?;
                for (input, class, expected) in [
                    (
                        b"{}".as_slice(),
                        DataClass::Private,
                        if configured {
                            StoreError::AtRestProtectionFailed
                        } else {
                            StoreError::AtRestProtectionUnavailable
                        },
                    ),
                    (
                        b"{}".as_slice(),
                        DataClass::Secret,
                        StoreError::ClassRefused,
                    ),
                    (
                        b"{}".as_slice(),
                        DataClass::Credential,
                        StoreError::ClassRefused,
                    ),
                    (
                        b"not-json".as_slice(),
                        DataClass::Public,
                        StoreError::CanonicalJson,
                    ),
                ] {
                    assert_eq!(tx.put_blob(input, class), Err(expected));
                    assert_eq!(
                        tx.inner
                            .query_row("SELECT count(*) FROM blobs", [], |r| r.get::<_, i64>(0))?,
                        1
                    );
                    assert_eq!(
                        tx.inner
                            .query_row("SELECT total_changes()", [], |r| r.get::<_, i64>(0))?,
                        changes
                    );
                }
                Ok(allowed)
            })
            .unwrap();
        assert_eq!(count(&store), 1);
        assert_eq!(store.transact(|tx| tx.get_blob(&allowed)).unwrap(), b"[]");
    }
}

#[test]
fn unicode_raw_and_escaped_inputs_dedupe_to_utf8_canonical_bytes() {
    let store = protected_store(Arc::new(TestProtection::new(1)));
    let canonical = "{\"é\":\"雪\",\"😀\":1}".as_bytes();
    for class in [DataClass::Public, DataClass::Personal, DataClass::Private] {
        let blob = put(&store, "{\"😀\":1,\"é\":\"雪\"}".as_bytes(), class);
        assert_eq!(
            put(&store, br#"{"\u00e9":"\u96ea","\ud83d\ude00":1}"#, class),
            blob
        );
        assert_eq!(store.transact(|tx| tx.get_blob(&blob)).unwrap(), canonical);
    }
    assert_eq!(count(&store), 3);
}

#[test]
fn exact_scj_depth_limit_round_trips_through_blob_storage() {
    let store = protected_store(Arc::new(TestProtection::new(1)));
    let input = format!("{}0{}", "[".repeat(63), "]".repeat(63));
    for class in [DataClass::Public, DataClass::Personal, DataClass::Private] {
        let blob = put(&store, input.as_bytes(), class);
        assert_eq!(
            store.transact(|tx| tx.get_blob(&blob)).unwrap(),
            input.as_bytes()
        );
    }
}

#[test]
fn configured_private_invalid_original_json_refuses_before_backend_or_insert() {
    let backend = Arc::new(TestProtection::new(1));
    let store = protected_store(backend.clone());
    for input in [
        b"broken".as_slice(),
        &[0xff],
        br#"{"a":1,"\u0061":2}"#,
        b"1.0",
        b"1e0",
        b"18446744073709551616",
    ] {
        store
            .transact(|tx| {
                assert_eq!(
                    tx.put_blob(input, DataClass::Private),
                    Err(StoreError::CanonicalJson)
                );
                assert_eq!(
                    tx.inner
                        .query_row("SELECT count(*) FROM blobs", [], |r| r.get::<_, i64>(0))?,
                    0
                );
                Ok(())
            })
            .unwrap();
    }
    assert_eq!(count(&store), 0);
    assert_eq!(backend.protect_calls.load(Ordering::Relaxed), 0);
    assert_eq!(backend.unprotect_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn private_noncanonical_recovered_plaintext_is_normalized_and_verified_for_dedupe() {
    let backend = Arc::new(TestProtection::new(1));
    let store = protected_store(backend.clone());
    let canonical = br#"{"a":1,"z":2}"#;
    let blob = put(&store, canonical, DataClass::Private);
    replace_content(
        &store,
        &blob,
        &backend.protect(br#" { "z" : 2, "a" : 1 } "#).unwrap(),
    );
    let before = stored(&store, &blob);
    assert_eq!(store.transact(|tx| tx.get_blob(&blob)).unwrap(), canonical);
    assert_eq!(put(&store, canonical, DataClass::Private), blob);
    assert_eq!(stored(&store, &blob), before);
    assert_eq!(backend.protect_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn p2d_public_method_inventory_excludes_parent_text_reference_and_later_phase_apis() {
    fn public_methods(source: &str) -> Vec<&str> {
        let mut methods: Vec<_> = source
            .lines()
            .filter_map(|line| line.trim().strip_prefix("pub fn "))
            .map(|line| line.split(['(', '<']).next().unwrap())
            .collect();
        methods.sort_unstable();
        methods
    }
    assert_eq!(
        public_methods(include_str!("store.rs")),
        [
            "checkpoint_for_close",
            "open",
            "open_in_memory",
            "open_in_memory_with_protection",
            "open_with_protection",
            "schema_version",
            "transact",
            "verify_integrity"
        ]
    );
    assert_eq!(
        public_methods(include_str!("blob.rs")),
        ["class", "digest", "get_blob", "new", "put_blob"]
    );
    assert!(public_methods(include_str!("tx.rs")).is_empty());
    // P2E authority is unchanged. P2F admits only whole begin/outcome methods;
    // independent parent/text/reference mutations and future engine stay excluded.
    assert_eq!(
        public_methods(include_str!("lease.rs")),
        [
            "acquire_lease",
            "generation",
            "release_lease",
            "renew_lease"
        ]
    );
    assert_eq!(
        public_methods(include_str!("outcome.rs")),
        ["begin_attempt", "commit_step_outcome"]
    );
    let root = include_str!("lib.rs");
    assert!(root.contains("mod lease;"));
    for forbidden in [
        "mod engine;",
        "mod recovery;",
        "mod journal;",
        "mod participant;",
        "pub mod testing",
        "TaskBlobRole",
        "StepBlobRole",
        "ClassifiedTextSlot",
    ] {
        assert!(!root.contains(forbidden), "forbidden surface: {forbidden}");
    }
}

#[test]
fn blob_ref_keeps_private_fields_and_has_no_wire_serialization_surface() {
    let source = include_str!("blob.rs");
    let fields = source
        .split("pub struct BlobRef {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    assert_eq!(fields.trim(), "digest: Digest,\n    class: DataClass,");
    assert!(!source.contains("Serialize"));
    assert!(!source.contains("serde"));
}
