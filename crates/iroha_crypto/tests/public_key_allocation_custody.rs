//! Actual allocator refusal and deallocation ordering for canonical key custody.

use iroha_allocation::{
    AllocationBudget, AllocationCharge, ChargedBuffer, ChargedBufferFromChargeError,
    RetainedPayload,
};
use iroha_crypto::{Algorithm, KeyPair, PublicKey, PublicKeyAllocationError};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::{Cell, RefCell},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering::SeqCst},
    },
};

struct Observer;
static SERIAL: Mutex<()> = Mutex::new(());
static CALLS: AtomicUsize = AtomicUsize::new(0);
static POINTER: AtomicUsize = AtomicUsize::new(0);
static FREES: AtomicUsize = AtomicUsize::new(0);
static RESERVED_AT_ALLOC: AtomicUsize = AtomicUsize::new(0);
static RESERVED_AT_FREE: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static FAIL: Cell<bool> = const { Cell::new(false) };
    static POOL: RefCell<Option<AllocationBudget>> = const { RefCell::new(None) };
}

fn reserved() -> usize {
    POOL.try_with(|pool| {
        pool.borrow()
            .as_ref()
            .map_or(0, AllocationBudget::reserved_bytes)
    })
    .unwrap_or(0)
}

#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Observer {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let enabled = ENABLED.try_with(Cell::get).unwrap_or(false);
        if enabled {
            CALLS.fetch_add(1, SeqCst);
            RESERVED_AT_ALLOC.store(reserved(), SeqCst);
            if FAIL.with(Cell::get) {
                return std::ptr::null_mut();
            }
        }
        // SAFETY: unchanged exact request to the system allocator.
        let pointer = unsafe { System.alloc(layout) };
        if enabled {
            POINTER.store(pointer as usize, SeqCst);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        let tracked = POINTER
            .compare_exchange(pointer as usize, 0, SeqCst, SeqCst)
            .is_ok();
        // SAFETY: return the original allocation with its unchanged layout.
        unsafe { System.dealloc(pointer, layout) };
        if tracked {
            RESERVED_AT_FREE.store(reserved(), SeqCst);
            FREES.fetch_add(1, SeqCst);
        }
    }
}
#[global_allocator]
static ALLOCATOR: Observer = Observer;

fn arm(pool: &AllocationBudget, fail: bool) {
    assert_eq!(POINTER.load(SeqCst), 0);
    CALLS.store(0, SeqCst);
    FREES.store(0, SeqCst);
    RESERVED_AT_ALLOC.store(0, SeqCst);
    RESERVED_AT_FREE.store(0, SeqCst);
    POOL.with(|stored| *stored.borrow_mut() = Some(pool.clone()));
    FAIL.with(|flag| flag.set(fail));
    ENABLED.with(|flag| flag.set(true));
}
fn disarm() {
    ENABLED.with(|flag| flag.set(false));
}
fn key() -> KeyPair {
    #[cfg(feature = "bls")]
    let algorithm = Algorithm::BlsNormal;
    #[cfg(not(feature = "bls"))]
    let algorithm = Algorithm::Ed25519;
    KeyPair::try_from_seed(vec![0x61; 32], algorithm).unwrap()
}
fn charge(pool: &AllocationBudget, layout: Layout) -> AllocationCharge {
    pool.try_reserve(layout).unwrap().try_split(layout).unwrap()
}

#[test]
fn exact_key_refusal_preserves_source_and_original_prepaid_charge_for_retry() {
    let _serial = SERIAL.lock().unwrap();
    let pair = key();
    let source = pair.public_key();
    let source_pointer = source.try_to_bytes().unwrap().1.as_ptr();
    let layout = source.retained_allocation_layout();
    let pool = AllocationBudget::new(layout.size());
    let original = charge(&pool, layout);
    arm(&pool, true);
    let result = source.try_clone_from_charge(&pool, original);
    disarm();
    let (original, error) = match result {
        Err(error) => error,
        Ok(_) => panic!("the physical allocator was instructed to refuse"),
    };
    assert!(matches!(error, PublicKeyAllocationError::Allocation(
        ChargedBufferFromChargeError::Allocator { layout: refused }) if refused == layout));
    assert_eq!(CALLS.load(SeqCst), 1);
    assert_eq!(original.layout(), layout);
    assert!(original.belongs_to(&pool));
    assert_eq!(pool.reserved_bytes(), layout.size());
    assert_eq!(source.try_to_bytes().unwrap().1.as_ptr(), source_pointer);
    // Already prepaid ownership does not re-enter admission after policy shrinks.
    pool.set_limit_bytes(0);
    arm(&pool, false);
    let copied = source.try_clone_from_charge(&pool, original).unwrap();
    disarm();
    assert_eq!(CALLS.load(SeqCst), 1);
    assert_eq!(copied.get(), source);
    assert_ne!(
        copied.get().try_to_bytes().unwrap().1.as_ptr(),
        source_pointer
    );
    assert_eq!(RESERVED_AT_ALLOC.load(SeqCst), layout.size());
    drop(copied);
    assert_eq!(FREES.load(SeqCst), 1);
    assert_eq!(RESERVED_AT_FREE.load(SeqCst), layout.size());
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn wrong_source_and_layout_are_rejected_before_any_allocation_or_refund() {
    let _serial = SERIAL.lock().unwrap();
    let pair = key();
    let source = pair.public_key();
    let layout = source.retained_allocation_layout();
    let pool = AllocationBudget::new(4096);
    let foreign = AllocationBudget::new(4096);
    let original = charge(&foreign, layout);
    arm(&pool, true);
    let result = source.try_clone_from_charge(&pool, original);
    disarm();
    let (original, error) = match result {
        Err(error) => error,
        Ok(_) => panic!("equal limits cannot relabel original source custody"),
    };
    assert_eq!(error, PublicKeyAllocationError::ForeignPool);
    assert_eq!(CALLS.load(SeqCst), 0);
    assert!(original.belongs_to(&foreign));
    assert_eq!(foreign.reserved_bytes(), layout.size());
    drop(original);
    assert_eq!(foreign.reserved_bytes(), 0);
    for wrong in [
        Layout::from_size_align(layout.size() + 1, 1).unwrap(),
        Layout::from_size_align(layout.size(), 2).unwrap(),
    ] {
        let original = charge(&pool, wrong);
        arm(&pool, true);
        let result = source.try_clone_from_charge(&pool, original);
        disarm();
        let (original, error) = match result {
            Err(error) => error,
            Ok(_) => panic!("exact size and alignment must match before allocation"),
        };
        assert!(matches!(error, PublicKeyAllocationError::Allocation(
            ChargedBufferFromChargeError::LayoutMismatch { expected, actual })
            if expected == layout && actual == wrong));
        assert_eq!(CALLS.load(SeqCst), 0);
        assert_eq!(original.layout(), wrong);
        assert_eq!(pool.reserved_bytes(), wrong.size());
        drop(original);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
#[allow(unsafe_code)]
fn original_compact_allocation_survives_canonical_field_moves_until_final_owner_drop() {
    struct Canonical {
        key: PublicKey,
    }
    let _serial = SERIAL.lock().unwrap();
    let pair = key();
    let source = pair.public_key();
    let layout = source.retained_allocation_layout();
    let ledger_layout = Layout::array::<AllocationCharge>(1).unwrap();
    let pool = AllocationBudget::new(layout.size() + ledger_layout.size());
    let mut ledger = ChargedBuffer::new(1, &pool).unwrap();
    let original = charge(&pool, layout);
    arm(&pool, false);
    let copied = source.try_clone_from_charge(&pool, original).unwrap();
    disarm();
    let pointer = copied.get().try_to_bytes().unwrap().1.as_ptr();
    // SAFETY: the exact compact key and its one charge are immediately moved
    // into the private canonical payload and same-pool ledger without failure.
    let (key, original) = unsafe { copied.into_allocation_parts() };
    ledger.push_reserved(original);
    let payload = unsafe { RetainedPayload::try_new(Canonical { key }, ledger, &pool) }
        .unwrap_or_else(|refusal| {
            panic!(
                "every exact allocation belongs to the original execution pool: {}",
                refusal.2
            )
        });
    // SAFETY: the only field is moved unchanged into a new inline record. No
    // allocation, mutation, clone, drop or sharing of its compact bytes occurs.
    let published = unsafe { payload.map_payload(|canonical| (canonical, 7_u64)) };
    assert_eq!(CALLS.load(SeqCst), 1);
    assert_eq!(
        published.get().0.key.try_to_bytes().unwrap().1.as_ptr(),
        pointer
    );
    assert_eq!(&published.get().0.key, source);
    assert_eq!(pool.reserved_bytes(), layout.size() + ledger_layout.size());
    drop(published);
    assert_eq!(FREES.load(SeqCst), 1);
    assert_eq!(
        RESERVED_AT_FREE.load(SeqCst),
        layout.size() + ledger_layout.size()
    );
    assert_eq!(pool.reserved_bytes(), 0);
}

#[cfg(any(feature = "sm", feature = "gost"))]
fn observe_allocations<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    struct EndObservation;
    impl Drop for EndObservation {
        fn drop(&mut self) {
            disarm();
        }
    }
    // Observe actual allocation calls only. Retained compact bytes, owned SM2
    // identifiers, and public error Strings are not funded by this observation pool.
    let observation = AllocationBudget::new(0);
    arm(&observation, false);
    let guard = EndObservation;
    let result = operation();
    drop(guard);
    (result, CALLS.load(SeqCst))
}

#[cfg(feature = "sm")]
#[test]
fn sm2_public_payload_allocates_only_its_retained_identifier() {
    use iroha_crypto::sm::{
        Sm2PrivateKey, decode_sm2_public_key_payload, encode_sm2_public_key_payload,
    };

    let _serial = SERIAL.lock().unwrap();
    let private = Sm2PrivateKey::from_seed("署名者-é", b"SM2 payload custody").unwrap();
    let public = private.public_key();
    let envelope =
        encode_sm2_public_key_payload(public.distid(), &public.to_sec1_bytes(false)).unwrap();
    let (result, calls) = observe_allocations(|| decode_sm2_public_key_payload(&envelope));
    let decoded = result.unwrap();
    assert_eq!(calls, 1, "only the dependency-owned identifier remains");
    assert_eq!(FREES.load(SeqCst), 0);
    assert_eq!(decoded, public);
    assert_eq!(decoded.distid(), public.distid());
    drop(decoded);
    assert_eq!(FREES.load(SeqCst), 1);
    assert_eq!(POINTER.load(SeqCst), 0);
}

#[cfg(feature = "sm")]
#[test]
fn sm2_equality_compares_exact_identity_and_point_without_allocation() {
    use iroha_crypto::sm::{Sm2PrivateKey, Sm2PublicKey};

    let _serial = SERIAL.lock().unwrap();
    let key = Sm2PrivateKey::from_seed("署名者-é", b"SM2 equality custody")
        .unwrap()
        .public_key();
    let same = Sm2PublicKey::from_sec1_bytes(key.distid(), &key.to_sec1_bytes(false)).unwrap();
    let different_identity =
        Sm2PublicKey::from_sec1_bytes("署名者-e\u{301}", &key.to_sec1_bytes(false)).unwrap();
    let different_point = Sm2PrivateKey::from_seed(key.distid(), b"distinct SM2 point")
        .unwrap()
        .public_key();
    let (comparisons, calls) = observe_allocations(|| {
        std::hint::black_box([
            key == same,
            key == different_identity,
            key == different_point,
            same == key,
        ])
    });
    assert_eq!(comparisons, [true, false, false, true]);
    assert_eq!(calls, 0);
}

#[cfg(feature = "sm")]
#[test]
fn compact_sm2_constructor_retains_only_its_original_envelope_allocation() {
    use iroha_crypto::sm::{Sm2PrivateKey, encode_sm2_public_key_payload};
    let _serial = SERIAL.lock().unwrap();
    let id = "x".repeat(8191);
    let owned = Sm2PrivateKey::from_seed(&id, b"compact SM2 custody")
        .unwrap()
        .public_key();
    let envelope = encode_sm2_public_key_payload(&id, &owned.to_sec1_bytes(false)).unwrap();
    let (result, calls) = observe_allocations(|| PublicKey::from_bytes(Algorithm::Sm2, &envelope));
    let key = result.unwrap();
    assert_eq!(calls, 1, "only the final compact envelope is retained");
    assert_eq!(FREES.load(SeqCst), 0);
    assert_eq!(key.to_bytes(), (Algorithm::Sm2, envelope.as_slice()));
    drop(key);
    assert_eq!(FREES.load(SeqCst), 1);
}

#[cfg(feature = "sm")]
#[test]
fn ordinary_typed_and_admission_sm2_verification_allocate_nothing() {
    use iroha_crypto::sm::{Sm2PrivateKey, encode_sm2_public_key_payload};
    use iroha_crypto::{
        Error, Hash, HashOf, Signature, SignatureOf, verify_signature_for_admission,
    };
    let _serial = SERIAL.lock().unwrap();
    let private = Sm2PrivateKey::from_seed("署名者-é", b"SM2 borrowed verification").unwrap();
    let owned = private.public_key();
    let envelope =
        encode_sm2_public_key_payload(owned.distid(), &owned.to_sec1_bytes(false)).unwrap();
    let key = PublicKey::from_bytes(Algorithm::Sm2, &envelope).unwrap();
    let hash = HashOf::<()>::from_untyped_unchecked(Hash::prehashed([0x61; 32]));
    let raw = private.sign(hash.as_ref());
    let signature = Signature::from_bytes(&raw.as_bytes());
    let typed = SignatureOf::<()>::from_signature(signature.clone());
    for _ in 0..3 {
        let (results, allocations) = observe_allocations(|| {
            [
                signature.verify(&key, hash.as_ref()),
                typed.verify_hash(&key, hash),
                verify_signature_for_admission(&signature, &key, hash.as_ref()),
                owned.verify(hash.as_ref(), &raw),
            ]
        });
        assert_eq!(allocations, 0);
        for result in results {
            result.unwrap();
        }
    }
    for index in 0..32 {
        let mut bytes = *hash.as_ref();
        bytes[index] ^= 2;
        let changed = HashOf::<()>::from_untyped_unchecked(Hash::prehashed(bytes));
        let (results, allocations) = observe_allocations(|| {
            [
                signature.verify(&key, changed.as_ref()),
                typed.verify_hash(&key, changed),
                verify_signature_for_admission(&signature, &key, changed.as_ref()),
            ]
        });
        assert_eq!(allocations, 0);
        for result in results {
            assert!(matches!(result, Err(Error::BadSignature)));
        }
    }
    for bytes in [
        &[][..],
        &[0; 63][..],
        &[0; 64][..],
        &[0xff; 64][..],
        &[1; 65][..],
    ] {
        let bad = Signature::from_bytes(bytes);
        let (results, allocations) = observe_allocations(|| {
            [
                bad.verify(&key, hash.as_ref()),
                verify_signature_for_admission(&bad, &key, hash.as_ref()),
            ]
        });
        assert_eq!(allocations, 0);
        for result in results {
            assert!(matches!(result, Err(Error::BadSignature)));
        }
    }
}

#[cfg(feature = "gost")]
struct GostFixture {
    algorithm: Algorithm,
    public: Vec<u8>,
    message: [u8; 32],
    valid: Vec<u8>,
    invalid: Vec<u8>,
}

#[cfg(feature = "gost")]
fn gost_fixtures() -> Vec<GostFixture> {
    // These are the repository's locally generated deterministic vectors. The
    // fixture filename is not evidence of independent upstream vector provenance.
    let root: norito::json::Value =
        norito::json::from_str(include_str!("fixtures/wycheproof_gost.json")).unwrap();
    let groups = root["testGroups"].as_array().unwrap();
    let algorithms = [
        Algorithm::Gost3410_2012_256ParamSetA,
        Algorithm::Gost3410_2012_256ParamSetB,
        Algorithm::Gost3410_2012_256ParamSetC,
        Algorithm::Gost3410_2012_512ParamSetA,
        Algorithm::Gost3410_2012_512ParamSetB,
    ];
    assert_eq!(groups.len(), algorithms.len());
    groups
        .iter()
        .zip(algorithms)
        .enumerate()
        .map(|(index, (group, algorithm))| {
            assert_eq!(
                group["algorithm"].as_str().unwrap(),
                format!("{algorithm:?}")
            );
            let cases = group["tests"].as_array().unwrap();
            assert_eq!(cases.len(), 2);
            assert_eq!(cases[0]["tcId"].as_u64().unwrap(), (index * 2 + 1) as u64);
            assert_eq!(cases[1]["tcId"].as_u64().unwrap(), (index * 2 + 2) as u64);
            assert_eq!(cases[0]["result"].as_str().unwrap(), "valid");
            assert_eq!(cases[1]["result"].as_str().unwrap(), "invalid");
            assert_eq!(cases[0]["msg"], cases[1]["msg"]);
            GostFixture {
                algorithm,
                public: hex::decode(group["public"].as_str().unwrap()).unwrap(),
                message: hex::decode(cases[0]["msg"].as_str().unwrap())
                    .unwrap()
                    .try_into()
                    .unwrap(),
                valid: hex::decode(cases[0]["sig"].as_str().unwrap()).unwrap(),
                invalid: hex::decode(cases[1]["sig"].as_str().unwrap()).unwrap(),
            }
        })
        .collect()
}

#[cfg(feature = "gost")]
#[test]
fn compact_gost_constructor_retains_only_its_original_payload_allocation() {
    let _serial = SERIAL.lock().unwrap();
    for fixture in gost_fixtures() {
        let (result, calls) =
            observe_allocations(|| PublicKey::from_bytes(fixture.algorithm, &fixture.public));
        let key = result.unwrap();
        assert_eq!(calls, 1, "only the final compact payload is retained");
        assert_eq!(FREES.load(SeqCst), 0);
        assert_eq!(
            key.to_bytes(),
            (fixture.algorithm, fixture.public.as_slice())
        );
        drop(key);
        assert_eq!(FREES.load(SeqCst), 1);
        assert_eq!(POINTER.load(SeqCst), 0);
    }
}

#[cfg(feature = "gost")]
#[test]
fn ordinary_typed_and_admission_gost_verification_allocate_nothing() {
    use iroha_crypto::{
        Error, Hash, HashOf, Signature, SignatureOf, verify_signature_for_admission,
    };
    let _serial = SERIAL.lock().unwrap();
    for fixture in gost_fixtures() {
        let key = PublicKey::from_bytes(fixture.algorithm, &fixture.public).unwrap();
        let owned =
            iroha_crypto::gost::parse_public_key(fixture.algorithm, &fixture.public).unwrap();
        let hash = HashOf::<()>::from_untyped_unchecked(Hash::prehashed(fixture.message));
        // Hash's canonical low-bit marker must not alter this fixture's message.
        assert_eq!(hash.as_ref(), &fixture.message);
        let signature = Signature::from_bytes(&fixture.valid);
        let typed = SignatureOf::<()>::from_signature(signature.clone());
        for _ in 0..3 {
            let (results, calls) = observe_allocations(|| {
                [
                    signature.verify(&key, hash.as_ref()),
                    typed.verify_hash(&key, hash),
                    verify_signature_for_admission(&signature, &key, hash.as_ref()),
                    iroha_crypto::gost::verify(
                        fixture.algorithm,
                        hash.as_ref(),
                        &fixture.valid,
                        &owned,
                    ),
                ]
            });
            assert_eq!(calls, 0);
            for result in results {
                result.unwrap();
            }
        }
        let mut changed = fixture.message;
        changed[0] ^= 2;
        let changed = HashOf::<()>::from_untyped_unchecked(Hash::prehashed(changed));
        let (results, calls) = observe_allocations(|| {
            [
                signature.verify(&key, changed.as_ref()),
                typed.verify_hash(&key, changed),
                verify_signature_for_admission(&signature, &key, changed.as_ref()),
                iroha_crypto::gost::verify(
                    fixture.algorithm,
                    changed.as_ref(),
                    &fixture.valid,
                    &owned,
                ),
            ]
        });
        assert_eq!(calls, 0);
        for result in results {
            assert!(matches!(result, Err(Error::BadSignature)));
        }
        let expected = fixture.valid.len();
        for bytes in [
            vec![],
            vec![0; expected - 1],
            vec![0; expected],
            vec![0xff; expected],
            vec![1; expected + 1],
            fixture.invalid,
        ] {
            let bad = Signature::from_bytes(&bytes);
            let typed_bad = SignatureOf::<()>::from_signature(bad.clone());
            let (results, calls) = observe_allocations(|| {
                [
                    bad.verify(&key, hash.as_ref()),
                    typed_bad.verify_hash(&key, hash),
                    verify_signature_for_admission(&bad, &key, hash.as_ref()),
                    iroha_crypto::gost::verify(fixture.algorithm, hash.as_ref(), &bytes, &owned),
                ]
            });
            assert_eq!(calls, 0);
            for result in results {
                assert!(matches!(result, Err(Error::BadSignature)));
            }
        }
    }
}

#[cfg(feature = "gost")]
#[test]
fn malformed_gost_public_key_diagnostics_still_materialize_unfunded_strings() {
    let _serial = SERIAL.lock().unwrap();
    for fixture in gost_fixtures() {
        for bytes in [
            vec![],
            vec![0; fixture.public.len()],
            vec![0xff; fixture.public.len()],
        ] {
            let (result, calls) =
                observe_allocations(|| PublicKey::from_bytes(fixture.algorithm, &bytes));
            assert!(result.is_err());
            assert!(
                calls > 0,
                "the fixed validator does not erase public error funding"
            );
            drop(result);
            assert_eq!(POINTER.load(SeqCst), 0);
        }
    }
}

#[cfg(feature = "gost")]
#[test]
fn owned_gost_public_parser_allocates_only_its_retained_payload() {
    let _serial = SERIAL.lock().unwrap();
    for fixture in gost_fixtures() {
        let (result, calls) = observe_allocations(|| {
            iroha_crypto::gost::parse_public_key(fixture.algorithm, &fixture.public)
        });
        let owned = result.unwrap();
        assert_eq!(
            calls, 1,
            "only the owned public facade's payload is retained"
        );
        assert_eq!(FREES.load(SeqCst), 0);
        assert_eq!(owned.as_bytes(), fixture.public);
        drop(owned);
        assert_eq!(FREES.load(SeqCst), 1);
        assert_eq!(POINTER.load(SeqCst), 0);
    }
}

#[path = "signature_allocation_custody.rs"]
mod signature_allocation_custody;
