//! Existing ML-DSA relation and actual Rust allocator controls for borrowed verification.

use iroha_crypto::{
    Algorithm, Error, KeyPair, PublicKey, Signature, pqc_verify_batch_deterministic,
    verify_signature_for_admission,
};
#[cfg(feature = "pqc")]
use iroha_crypto::{Hash, HashOf, SignatureOf};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct Observer;
thread_local! {
    static OBSERVE: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}
fn record_allocation() {
    if OBSERVE.try_with(Cell::get).unwrap_or(false) {
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
    }
}
// SAFETY: forwards every original pointer, alignment and size unchanged to System.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Observer {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: unchanged request from the caller.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: unchanged request from the caller.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation();
        // SAFETY: original allocation and requested size are forwarded unchanged.
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: original allocation and its layout are forwarded unchanged.
        unsafe { System.dealloc(pointer, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Observer = Observer;

fn without_allocations<T>(operation: impl FnOnce() -> T) -> T {
    struct EndObservation;
    impl Drop for EndObservation {
        fn drop(&mut self) {
            OBSERVE.with(|flag| flag.set(false));
        }
    }
    assert!(!OBSERVE.with(Cell::get));
    ALLOCATIONS.with(|count| count.set(0));
    OBSERVE.with(|flag| flag.set(true));
    let guard = EndObservation;
    let result = operation();
    drop(guard);
    assert_eq!(
        ALLOCATIONS.with(Cell::get),
        0,
        "borrowed ML-DSA verification allocated"
    );
    result
}

#[cfg(feature = "pqc")]
fn verify_facades(key: &PublicKey, signature: &Signature, message: &[u8], valid: bool) {
    let (_, bytes) = key.to_bytes();
    for outcome in [
        without_allocations(|| signature.verify(key, message)),
        without_allocations(|| verify_signature_for_admission(signature, key, message)),
        without_allocations(|| {
            pqc_verify_batch_deterministic(&[message], &[signature.payload()], &[bytes], [0; 32])
        }),
    ] {
        if valid {
            assert!(outcome.is_ok());
        } else {
            assert!(matches!(outcome, Err(Error::BadSignature)));
        }
    }
}

#[test]
fn allocator_observer_detects_allocation_and_unwind() {
    assert!(
        std::panic::catch_unwind(|| without_allocations(|| std::hint::black_box(vec![0x51; 73])))
            .is_err()
    );
    assert!(!OBSERVE.with(Cell::get));
    assert!(
        std::panic::catch_unwind(|| without_allocations(|| panic!("observer unwind control")))
            .is_err()
    );
    assert!(!OBSERVE.with(Cell::get));
    without_allocations(|| ());
}

#[cfg(feature = "pqc")]
#[test]
fn native_known_answer_verifies_cold_and_repeated_without_rust_allocation() {
    let vectors: norito::json::Value = norito::json::from_slice(include_bytes!(
        "../../soranet_pq/tests/fixtures/pq_vectors.json"
    ))
    .unwrap();
    let rows = vectors.get("mldsa").unwrap().as_array().unwrap();
    let mut count = 0;
    for row in rows {
        if row.get("suite").unwrap().as_str() != Some("mldsa65") {
            continue;
        }
        let bytes = |field| hex::decode(row.get(field).unwrap().as_str().unwrap()).unwrap();
        let key = PublicKey::from_bytes(Algorithm::MlDsa, &bytes("public_key")).unwrap();
        let signature = Signature::from_bytes(&bytes("signature"));
        let message = bytes("message");
        for _ in 0..3 {
            verify_facades(&key, &signature, &message, true);
        }
        let mut changed_message = message.clone();
        changed_message.push(0x51);
        verify_facades(&key, &signature, &changed_message, false);
        count += 1;
    }
    assert!(count > 0, "the existing native KAT must contain ML-DSA-65");
}

#[cfg(feature = "pqc")]
#[test]
fn signature_length_zero_and_byte_mutations_allocate_nothing() {
    let pair = KeyPair::from_seed(vec![0x53; 32], Algorithm::MlDsa);
    let message = [0x59; 32];
    let signature = Signature::new(pair.private_key(), &message);
    verify_facades(pair.public_key(), &signature, &message, true);
    for length in [0, 1, 3308, 3310] {
        let invalid = Signature::from_bytes(&vec![0x51; length]);
        verify_facades(pair.public_key(), &invalid, &message, false);
    }
    let zero = Signature::from_bytes(&[0; 3309]);
    verify_facades(pair.public_key(), &zero, &message, false);
    for index in [0, 47, 48, 1000, 3308] {
        let mut changed = signature.payload().to_vec();
        changed[index] ^= 1;
        verify_facades(
            pair.public_key(),
            &Signature::from_bytes(&changed),
            &message,
            false,
        );
    }
    let mut changed_key = pair.public_key().to_bytes().1.to_vec();
    changed_key[0] ^= 1;
    let changed_key = PublicKey::from_bytes(Algorithm::MlDsa, &changed_key).unwrap();
    verify_facades(&changed_key, &signature, &message, false);
}

#[cfg(feature = "pqc")]
#[test]
fn typed_hash_facade_shares_borrowed_relation_and_rejects_each_changed_byte() {
    let pair = KeyPair::from_seed(vec![0x54; 32], Algorithm::MlDsa);
    let hash = HashOf::<()>::from_untyped_unchecked(Hash::prehashed([0x61; 32]));
    let signature = Signature::new(pair.private_key(), hash.as_ref());
    let typed = SignatureOf::<()>::from_signature(signature.clone());
    without_allocations(|| typed.verify_hash(pair.public_key(), hash)).unwrap();
    verify_facades(pair.public_key(), &signature, hash.as_ref(), true);
    for index in 0..32 {
        let mut bytes = *hash.as_ref();
        bytes[index] ^= 2; // Preserve the canonical marker in the final byte.
        let changed = HashOf::<()>::from_untyped_unchecked(Hash::prehashed(bytes));
        assert!(matches!(
            without_allocations(|| typed.verify_hash(pair.public_key(), changed)),
            Err(Error::BadSignature)
        ));
        verify_facades(pair.public_key(), &signature, changed.as_ref(), false);
    }
}

#[cfg(feature = "pqc")]
#[test]
fn context_signatures_remain_outside_the_empty_context_facade() {
    let pair = KeyPair::from_seed(vec![0x56; 32], Algorithm::MlDsa);
    let message = b"fixed context-bound message";
    for length in [1, 255] {
        let context = vec![0x64; length];
        let signature =
            Signature::try_new_with_context(pair.private_key(), &context, message).unwrap();
        verify_facades(pair.public_key(), &signature, message, false);
    }
    assert!(Signature::try_new_with_context(pair.private_key(), &[0x65; 256], message).is_err());
}

#[test]
fn constructor_and_norito_codec_preserve_canonical_mldsa_envelope() {
    let bytes = [0x51; 1952];
    let key = PublicKey::from_bytes(Algorithm::MlDsa, &bytes).unwrap();
    assert_eq!(key.to_bytes(), (Algorithm::MlDsa, bytes.as_slice()));
    let encoded = norito::encode_canonical(&key).unwrap();
    let decoded: PublicKey = norito::decode_canonical(&encoded).unwrap();
    assert_eq!(decoded, key);
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), encoded);
    assert!(norito::decode_canonical::<PublicKey>(&encoded[..encoded.len() - 1]).is_err());
    for length in [0, 1, 1951, 1953] {
        let error = PublicKey::from_bytes(Algorithm::MlDsa, &vec![0x51; length]).unwrap_err();
        assert_eq!(error.to_string(), "invalid ML-DSA public key length");
    }
    assert_eq!(
        PublicKey::from_bytes(Algorithm::MlDsa, &[0; 1952])
            .unwrap_err()
            .to_string(),
        "invalid ML-DSA public key: all-zero material"
    );
    let ed = KeyPair::from_seed(vec![0x61; 32], Algorithm::Ed25519);
    assert!(matches!(KeyPair::new(key, ed.private_key().clone()),
        Err(Error::KeyGen(ref message)) if message == "Mismatch of key algorithms"));
}

#[cfg(not(feature = "pqc"))]
#[test]
fn absent_native_feature_rejects_verification_without_allocating() {
    let bytes = [0x51; 1952];
    let key = PublicKey::from_bytes(Algorithm::MlDsa, &bytes).unwrap();
    let signature = Signature::from_bytes(&[0x61; 3309]);
    let message = b"feature-independent envelope verification";
    assert!(matches!(
        without_allocations(|| signature.verify(&key, message)),
        Err(Error::BadSignature)
    ));
    assert!(matches!(
        without_allocations(|| verify_signature_for_admission(&signature, &key, message)),
        Err(Error::BadSignature)
    ));
    assert!(matches!(
        without_allocations(|| pqc_verify_batch_deterministic(
            &[message],
            &[signature.payload()],
            &[&bytes],
            [0; 32]
        )),
        Err(Error::BadSignature)
    ));
}
