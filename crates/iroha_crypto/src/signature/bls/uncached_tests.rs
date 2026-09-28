//! Current/upstream relation and Rust allocation controls for the staged core.

use super::*;
use crate::{Error, KeyGenOption};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
use w3f_bls::{EngineBLS, Message, SerializableToBytes as _};

use super::super::{
    implementation::{BlsConfiguration, BlsImpl, VerifyOkCacheAccess},
    normal::NormalConfiguration,
    small::SmallConfiguration,
};

struct ObservedAllocator;

thread_local! {
    static OBSERVE: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn record_allocation() {
    if OBSERVE.try_with(Cell::get).unwrap_or(false) {
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
    }
}

// SAFETY: this observer forwards each request and original layout to System;
// it changes no pointer, allocation size, alignment, or deallocation behavior.
#[allow(unsafe_code)] // Required only to forward the allocator's original pointer/layout contract.
unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: forwarded unchanged from GlobalAlloc's caller.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: forwarded unchanged from GlobalAlloc's caller.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation();
        // SAFETY: forwarded unchanged from GlobalAlloc's caller.
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwarded unchanged from GlobalAlloc's caller.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;

pub(in crate::signature::bls) fn without_allocations<T>(operation: impl FnOnce() -> T) -> T {
    struct EndObservation;
    impl Drop for EndObservation {
        fn drop(&mut self) {
            OBSERVE.with(|active| active.set(false));
        }
    }
    assert!(
        !OBSERVE.with(Cell::get),
        "allocation observations cannot nest"
    );
    ALLOCATIONS.with(|count| count.set(0));
    OBSERVE.with(|active| active.set(true));
    let guard = EndObservation;
    let result = operation();
    drop(guard);
    assert_eq!(ALLOCATIONS.with(Cell::get), 0, "uncached core allocated");
    result
}

#[test]
fn allocation_observer_detects_backing_and_retires_during_unwind() {
    let observed = std::panic::catch_unwind(|| {
        without_allocations(|| {
            std::hint::black_box(vec![0x5a_u8; 73]);
        });
    });
    assert!(observed.is_err(), "a real Vec allocation must be detected");
    assert!(!OBSERVE.with(Cell::get));
    let unwind = std::panic::catch_unwind(|| {
        without_allocations(|| panic!("unwind during the observed operation"));
    });
    assert!(unwind.is_err());
    assert!(!OBSERVE.with(Cell::get));
    without_allocations(|| ());
}

fn fixture<C: BlsConfiguration>(seed: u8, message: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let (public_key, private_key) =
        BlsImpl::<C>::try_keypair(KeyGenOption::UseSeed(vec![seed; 32])).unwrap();
    (
        public_key.to_bytes(),
        BlsImpl::<C>::try_sign(message, &private_key).unwrap(),
    )
}

fn compare<C: BlsConfiguration + VerifyOkCacheAccess>(
    orientation: Orientation,
    public_key: &[u8],
    signature: &[u8],
    message: &[u8],
) {
    // Both facades use the same relation; this checks their result/error adapters.
    // W3f/blst signatures and message-point controls below establish independent evidence.
    let typed = BlsImpl::<C>::parse_public_key(public_key)
        .map_err(Error::from)
        .and_then(|key| BlsImpl::<C>::verify(message, signature, &key));
    let borrowed = without_allocations(|| verify(orientation, public_key, signature, message));
    assert_eq!(
        borrowed.is_ok(),
        typed.is_ok(),
        "facade verification verdict"
    );
    match (typed, borrowed) {
        (Err(Error::BadSignature), Err(Rejection::Verification))
        | (Err(Error::Parse(_)), Err(Rejection::Parse(_)))
        | (Ok(()), Ok(())) => {}
        pair => panic!("deterministic rejection category changed: {pair:?}"),
    }
}

fn current_parity<C: BlsConfiguration + VerifyOkCacheAccess>(orientation: Orientation) {
    for message in [Vec::new(), vec![0x42; 32], vec![0x63; 16_384]] {
        for seed in [0x31, 0x32] {
            let (public_key, signature) = fixture::<C>(seed, &message);
            compare::<C>(orientation, &public_key, &signature, &message);
            without_allocations(|| verify(orientation, &public_key, &signature, &message))
                .expect("independently generated w3f signature verifies");
            compare::<C>(orientation, &public_key, &signature, b"changed message");
            assert!(matches!(
                without_allocations(|| verify(
                    orientation,
                    &public_key,
                    &signature,
                    b"changed message"
                )),
                Err(Rejection::Verification)
            ));
            let (wrong_key, _) = fixture::<C>(seed + 2, &message);
            compare::<C>(orientation, &wrong_key, &signature, &message);
            assert!(matches!(
                without_allocations(|| verify(orientation, &wrong_key, &signature, &message)),
                Err(Rejection::Verification)
            ));
        }
    }
}

#[test]
fn existing_normal_and_small_signatures_keep_the_exact_contextual_relation() {
    current_parity::<NormalConfiguration>(Orientation::Normal);
    current_parity::<SmallConfiguration>(Orientation::Small);
}

fn malformed_parity<C: BlsConfiguration + VerifyOkCacheAccess>(orientation: Orientation) {
    let message = [0x74; 32];
    let (public_key, signature) = fixture::<C>(0x73, &message);
    for len in 0..signature.len() {
        compare::<C>(orientation, &public_key, &signature[..len], &message);
    }
    for len in 0..public_key.len() {
        compare::<C>(orientation, &public_key[..len], &signature, &message);
    }
    for fill in [0, 0xff] {
        compare::<C>(
            orientation,
            &public_key,
            &vec![fill; signature.len()],
            &message,
        );
        compare::<C>(
            orientation,
            &vec![fill; public_key.len()],
            &signature,
            &message,
        );
    }
    let mut changed = signature.clone();
    changed.push(0);
    compare::<C>(orientation, &public_key, &changed, &message);
    let mut changed = public_key.clone();
    changed.push(0);
    compare::<C>(orientation, &changed, &signature, &message);
    // Every compressed flag combination, canonical infinity and one coordinate
    // bit per byte. No malformed fixture is accepted merely by test construction.
    for flags in 0..8 {
        let mut changed = signature.clone();
        changed[0] = (changed[0] & 0x1f) | (flags << 5);
        compare::<C>(orientation, &public_key, &changed, &message);
        let mut changed = public_key.clone();
        changed[0] = (changed[0] & 0x1f) | (flags << 5);
        compare::<C>(orientation, &changed, &signature, &message);
    }
    let mut identity = vec![0; signature.len()];
    identity[0] = 0xc0;
    compare::<C>(orientation, &public_key, &identity, &message);
    let mut identity = vec![0; public_key.len()];
    identity[0] = 0xc0;
    compare::<C>(orientation, &identity, &signature, &message);
    for offset in 0..signature.len() {
        let mut changed = signature.clone();
        changed[offset] ^= 1;
        compare::<C>(orientation, &public_key, &changed, &message);
    }
}

#[test]
fn malformed_encodings_preserve_current_rejection_categories_without_allocation() {
    malformed_parity::<NormalConfiguration>(Orientation::Normal);
    malformed_parity::<SmallConfiguration>(Orientation::Small);
}

#[test]
fn subgroup_controls_are_valid_points_and_rejected_in_both_positions() {
    // Exact existing subgroup controls from bls/tests.rs, not generated data.
    let g1_bytes = hex_literal::hex!(
        "8000000000000000000000000000000000000000000000000000000000000000\
         00000000000000000000000000000004"
    );
    let g2_bytes = hex_literal::hex!(
        "8158b0083c00046272a9b63583963fff07e147f3f9e6e24174328ad8bc2aa150\
         298f3189a9cf6ed626f461e944bbd3d117762a3b9108c4a74a151b732a6075bf\
         2199bc19c48c393d4ceb92d0a76057be02f08540770fabd60262cea73ea1906c"
    );
    let g1_point = G1Affine::from_compressed_unchecked(&g1_bytes).unwrap();
    let g2_point = G2Affine::from_compressed_unchecked(&g2_bytes).unwrap();
    assert!(bool::from(g1_point.is_on_curve()));
    assert!(bool::from(g2_point.is_on_curve()));
    assert!(!bool::from(g1_point.is_torsion_free()));
    assert!(!bool::from(g2_point.is_torsion_free()));
    let message = [0x81; 32];
    let (normal_key, normal_sig) = fixture::<NormalConfiguration>(0x81, &message);
    let (small_key, small_sig) = fixture::<SmallConfiguration>(0x82, &message);
    compare::<NormalConfiguration>(Orientation::Normal, &g1_bytes, &normal_sig, &message);
    compare::<NormalConfiguration>(Orientation::Normal, &normal_key, &g2_bytes, &message);
    compare::<SmallConfiguration>(Orientation::Small, &g2_bytes, &small_sig, &message);
    compare::<SmallConfiguration>(Orientation::Small, &small_key, &g1_bytes, &message);
    for (orientation, key, proof) in [
        (
            Orientation::Normal,
            g1_bytes.as_slice(),
            normal_sig.as_slice(),
        ),
        (
            Orientation::Normal,
            normal_key.as_slice(),
            g2_bytes.as_slice(),
        ),
        (
            Orientation::Small,
            g2_bytes.as_slice(),
            small_sig.as_slice(),
        ),
        (
            Orientation::Small,
            small_key.as_slice(),
            g1_bytes.as_slice(),
        ),
    ] {
        assert!(matches!(
            without_allocations(|| verify(orientation, key, proof, &message)),
            Err(Rejection::Parse(_))
        ));
    }
}

#[test]
fn fixed_prefixes_and_dst_match_the_current_upstream_message_points() {
    let message = [0x91; 32];
    let current = Message::new(b"for signing messages", &message);
    let normal = current.hash_to_signature_curve::<w3f_bls::ZBLS>();
    let normal = w3f_bls::ZBLS::signature_point_to_byte(&normal);
    let candidate = blstrs::G2Projective::hash_to_curve(&message, HASH_TO_FIELD_DST, NORMAL_PREFIX);
    assert_eq!(normal.as_slice(), candidate.to_compressed().as_slice());
    let small = current.hash_to_signature_curve::<w3f_bls::TinyBLS381>();
    let small = w3f_bls::TinyBLS381::signature_point_to_byte(&small);
    let candidate = blstrs::G1Projective::hash_to_curve(&message, HASH_TO_FIELD_DST, SMALL_PREFIX);
    assert_eq!(small.as_slice(), candidate.to_compressed().as_slice());
}

#[test]
fn independent_blst_signatures_verify_with_typed_and_borrowed_facades() {
    let message = [0xa1; 32];
    let normal = blst::min_pk::SecretKey::key_gen(&[0xa2; 32], &[]).unwrap();
    let signature = normal
        .sign(&message, HASH_TO_FIELD_DST, NORMAL_PREFIX)
        .to_bytes();
    compare::<NormalConfiguration>(
        Orientation::Normal,
        &normal.sk_to_pk().to_bytes(),
        &signature,
        &message,
    );
    without_allocations(|| {
        verify(
            Orientation::Normal,
            &normal.sk_to_pk().to_bytes(),
            &signature,
            &message,
        )
    })
    .expect("independent blst normal signature verifies");
    let small = blst::min_sig::SecretKey::key_gen(&[0xa3; 32], &[]).unwrap();
    let signature = small
        .sign(&message, HASH_TO_FIELD_DST, SMALL_PREFIX)
        .to_bytes();
    compare::<SmallConfiguration>(
        Orientation::Small,
        &small.sk_to_pk().to_bytes(),
        &signature,
        &message,
    );
    without_allocations(|| {
        verify(
            Orientation::Small,
            &small.sk_to_pk().to_bytes(),
            &signature,
            &message,
        )
    })
    .expect("independent blst small signature verifies");
}
