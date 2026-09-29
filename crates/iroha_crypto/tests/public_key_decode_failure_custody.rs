//! Actual allocation observations for fixed public-key decode rejection custody.

use iroha_crypto::{Algorithm, KeyPair, PublicKey};
use iroha_primitives::const_vec::ConstVec;
use norito::core::{
    DecodeFlagsGuard, DecodeFromSlice, DecodeLimits, DeserializePayload, Encoder, Error,
    PayloadCtxGuard, SerializePayload, header_flags, with_decode_limits_measured,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct Observer;
thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static CALLS: Cell<usize> = const { Cell::new(0) };
    static REFUSE_NEXT: Cell<bool> = const { Cell::new(false) };
}
fn record() {
    if ACTIVE.try_with(Cell::get).unwrap_or(false) {
        let _ = CALLS.try_with(|count| count.set(count.get() + 1));
    }
}
fn refuse() -> bool {
    ACTIVE.try_with(Cell::get).unwrap_or(false)
        && REFUSE_NEXT
            .try_with(|next| next.replace(false))
            .unwrap_or(false)
}
// SAFETY: forwards original requests to System or returns null for one armed
// test allocation; failed realloc leaves the original allocation untouched.
#[allow(unsafe_code)]
unsafe impl GlobalAlloc for Observer {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        if refuse() {
            return std::ptr::null_mut();
        }
        // SAFETY: forwards the caller's exact allocation layout.
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        if refuse() {
            return std::ptr::null_mut();
        }
        // SAFETY: forwards the caller's exact allocation layout.
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record();
        if refuse() {
            return std::ptr::null_mut();
        }
        // SAFETY: forwards the original pointer, layout and requested size.
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: forwards the original allocation and exact layout.
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Observer = Observer;

fn observe<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            ACTIVE.with(|active| active.set(false));
        }
    }
    assert!(!ACTIVE.with(Cell::get));
    CALLS.with(|calls| calls.set(0));
    ACTIVE.with(|active| active.set(true));
    let guard = Guard;
    let result = operation();
    drop(guard);
    (result, CALLS.with(Cell::get))
}

fn algorithms() -> &'static [Algorithm] {
    &[
        Algorithm::Ed25519,
        Algorithm::Secp256k1,
        Algorithm::MlDsa,
        #[cfg(feature = "bls")]
        Algorithm::BlsNormal,
        #[cfg(feature = "bls")]
        Algorithm::BlsSmall,
        #[cfg(feature = "gost")]
        Algorithm::Gost3410_2012_256ParamSetA,
        #[cfg(feature = "gost")]
        Algorithm::Gost3410_2012_256ParamSetB,
        #[cfg(feature = "gost")]
        Algorithm::Gost3410_2012_256ParamSetC,
        #[cfg(feature = "gost")]
        Algorithm::Gost3410_2012_512ParamSetA,
        #[cfg(feature = "gost")]
        Algorithm::Gost3410_2012_512ParamSetB,
        #[cfg(feature = "sm")]
        Algorithm::Sm2,
    ]
}
fn key(algorithm: Algorithm) -> PublicKey {
    #[cfg(not(feature = "pqc"))]
    if algorithm == Algorithm::MlDsa {
        return PublicKey::from_bytes(algorithm, &[0x5b; 1952]).unwrap();
    }
    KeyPair::try_from_seed(vec![0x5b; 32], algorithm)
        .unwrap()
        .public_key()
        .clone()
}
fn invalid_payloads(algorithm: Algorithm, payload: &[u8]) -> Vec<Vec<u8>> {
    let mut trailing = payload.to_vec();
    trailing.push(0);
    let mut cases = vec![
        vec![],
        vec![0; payload.len()],
        payload[..payload.len() - 1].to_vec(),
        trailing,
    ];
    match algorithm {
        Algorithm::Ed25519 => {
            let mut identity = vec![0; 32];
            identity[0] = 1;
            cases.push(identity);
            let mut noncanonical = vec![0xff; 32];
            noncanonical[0] = 0xee;
            noncanonical[31] = 0x7f;
            cases.push(noncanonical);
            cases.push(vec![2; 32]);
            cases.push(
                (curve25519_dalek::constants::ED25519_BASEPOINT_POINT
                    + curve25519_dalek::constants::EIGHT_TORSION[1])
                    .compress()
                    .to_bytes()
                    .to_vec(),
            );
        }
        Algorithm::Secp256k1 => {
            use k256::elliptic_curve::sec1::ToEncodedPoint as _;
            cases.push(
                k256::PublicKey::from_sec1_bytes(payload)
                    .unwrap()
                    .to_encoded_point(false)
                    .as_bytes()
                    .to_vec(),
            );
            cases.push(vec![0xff; 33]);
        }
        Algorithm::MlDsa => {}
        #[cfg(feature = "bls")]
        Algorithm::BlsNormal | Algorithm::BlsSmall => {
            let mut identity = vec![0; payload.len()];
            identity[0] = 0xc0;
            cases.push(identity);
            let mut invalid_flags = payload.to_vec();
            invalid_flags[0] |= 0xe0;
            cases.push(invalid_flags);
            let mut point = vec![0; payload.len()];
            point[0] = 0x80;
            cases.push(point);
            cases.push(vec![0xff; payload.len()]);
        }
        #[cfg(feature = "gost")]
        Algorithm::Gost3410_2012_256ParamSetA
        | Algorithm::Gost3410_2012_256ParamSetB
        | Algorithm::Gost3410_2012_256ParamSetC
        | Algorithm::Gost3410_2012_512ParamSetA
        | Algorithm::Gost3410_2012_512ParamSetB => {
            cases.push(vec![0xff; payload.len()]);
            cases.push(vec![1; payload.len()]);
        }
        #[cfg(feature = "sm")]
        Algorithm::Sm2 => {
            let id_len = usize::from(u16::from_be_bytes(payload[..2].try_into().unwrap()));
            let mut bad_tag = payload.to_vec();
            bad_tag[2 + id_len] = 0x02;
            cases.push(bad_tag);
            if id_len > 0 {
                let mut bad_utf8 = payload.to_vec();
                bad_utf8[2] = 0xff;
                cases.push(bad_utf8);
            }
            let mut bad_point = payload.to_vec();
            bad_point[3 + id_len..].fill(0xff);
            cases.push(bad_point);
        }
    }
    cases
}
fn limits(bytes: usize) -> DecodeLimits {
    DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, bytes, usize::MAX)
}
fn layouts() -> [u8; 2] {
    [0, header_flags::COMPACT_LEN]
}
fn binary(bytes: &[u8], flags: u8) -> Vec<u8> {
    let _flags = DecodeFlagsGuard::enter(flags);
    let mut output = Vec::new();
    ConstVec::from(bytes.to_vec())
        .serialize(&mut Encoder::for_buffer(&mut output))
        .unwrap();
    output
}
fn archived(bytes: &[u8], flags: u8) -> Result<PublicKey, Error> {
    let _payload = PayloadCtxGuard::enter_with_flags(bytes, flags);
    let view = norito::core::archived_from_slice::<u8>(bytes)?;
    PublicKey::try_deserialize(view.archived().cast::<PublicKey>())
}
fn fixed_key_error(result: &Result<PublicKey, Error>) {
    assert!(matches!(
        result,
        Err(Error::InvalidValue {
            context: "public key"
        })
    ));
}

#[test]
fn observer_detects_actual_allocation_and_resets_after_unwind() {
    let (_, calls) = observe(|| std::hint::black_box(vec![0x51; 37]));
    assert!(calls > 0);
    assert!(std::panic::catch_unwind(|| observe(|| panic!("observer unwind control"))).is_err());
    assert!(!ACTIVE.with(Cell::get));
    assert_eq!(observe(|| ()).1, 0);
}

#[test]
fn all_algorithms_reject_invalid_borrowed_material_without_allocations() {
    for &algorithm in algorithms() {
        let valid = key(algorithm);
        let (_, bytes) = valid.to_bytes();
        for invalid in invalid_payloads(algorithm, bytes) {
            for _ in 0..2 {
                let (result, calls) =
                    observe(|| PublicKey::from_bytes_for_decode(algorithm, &invalid));
                assert_eq!(calls, 0, "{algorithm:?}, length {}", invalid.len());
                fixed_key_error(&result);
            }
        }
    }
}

#[test]
fn every_valid_algorithm_allocates_only_its_exact_compact_destination() {
    for &algorithm in algorithms() {
        let valid = key(algorithm);
        let (_, bytes) = valid.to_bytes();
        let exact = bytes.len() + 1;
        // The first BLS iteration has no decoder-cache entry; the second is warm.
        for _ in 0..2 {
            let ((result, calls), usage) = with_decode_limits_measured(limits(exact), || {
                observe(|| PublicKey::from_bytes_for_decode(algorithm, bytes))
            });
            assert_eq!(result.unwrap(), valid);
            assert_eq!(calls, 1, "{algorithm:?}");
            assert_eq!(usage.total_allocated_bytes(), exact);
            let ((result, calls), usage) = with_decode_limits_measured(limits(exact - 1), || {
                observe(|| PublicKey::from_bytes_for_decode(algorithm, bytes))
            });
            assert!(matches!(result, Err(Error::TotalAllocationExceeded { .. })));
            assert_eq!(calls, 0);
            assert!(usage.total_allocated_bytes() < exact);
        }
    }
}

#[test]
fn binary_entrypoints_validate_before_allocation_for_all_algorithms_and_layouts() {
    for &algorithm in algorithms() {
        let valid = key(algorithm);
        let (tag, payload) = valid.to_bytes();
        let tag = tag as u8;
        let mut compact = vec![tag];
        compact.extend_from_slice(payload);
        for flags in layouts() {
            let encoded = binary(&compact, flags);
            let _flags = DecodeFlagsGuard::enter(flags);
            let ((value, used), calls) = {
                let (result, calls) = observe(|| PublicKey::decode_from_slice(&encoded));
                (result.unwrap(), calls)
            };
            assert_eq!(value, valid);
            assert_eq!(used, encoded.len());
            assert_eq!(calls, 1);
            let (result, calls) = observe(|| archived(&encoded, flags));
            assert_eq!(result.unwrap(), valid);
            assert_eq!(calls, 1);
            let mut reencoded = Vec::new();
            value
                .serialize(&mut Encoder::for_buffer(&mut reencoded))
                .unwrap();
            assert_eq!(reencoded, encoded);

            for bad in [vec![], vec![0xff], vec![tag], {
                let mut bad = vec![0; compact.len()];
                bad[0] = tag;
                bad
            }] {
                let malformed = binary(&bad, flags);
                let (slice_result, slice_calls) =
                    observe(|| PublicKey::decode_from_slice(&malformed));
                let (archive_result, archive_calls) = observe(|| archived(&malformed, flags));
                assert_eq!(
                    (slice_calls, archive_calls),
                    (0, 0),
                    "{algorithm:?} flags {flags}"
                );
                for result in [slice_result.map(|(key, _)| key), archive_result] {
                    match bad.first() {
                        None => assert!(matches!(result, Err(Error::LengthMismatch))),
                        Some(0xff) => {
                            assert!(matches!(result, Err(Error::InvalidTag { tag: 0xff, .. })))
                        }
                        _ => fixed_key_error(&result),
                    }
                }
            }
            let mut trailing = encoded.clone();
            trailing.extend_from_slice(&[0xf0, 0x12]);
            let (prefix, used) = PublicKey::decode_from_slice(&trailing).unwrap();
            assert_eq!(prefix, valid);
            assert_eq!(used, encoded.len());
            let (result, calls) = observe(|| archived(&trailing, flags));
            assert!(matches!(result, Err(Error::LengthMismatch)));
            assert_eq!(calls, 0);
        }
    }
}

#[test]
fn byte_sequence_destination_shares_framing_and_limits_without_allocations() {
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        let encoded = binary(&[0, 0x41, 0xff], flags);
        let mut destination = [0; 3];
        let (result, calls) =
            observe(|| norito::core::decode_byte_element_sequence_into(&encoded, &mut destination));
        assert_eq!(result.unwrap(), (3, encoded.len()));
        assert_eq!(destination, [0, 0x41, 0xff]);
        assert_eq!(calls, 0);
        let (generic, used) = ConstVec::<u8>::decode_from_slice(&encoded).unwrap();
        assert_eq!(&*generic, &destination);
        assert_eq!(used, encoded.len());
        let mut short = [0; 2];
        assert!(matches!(
            norito::core::decode_byte_element_sequence_into(&encoded, &mut short),
            Err(Error::LengthMismatch)
        ));
        for length in 0..encoded.len() {
            let (result, calls) = observe(|| {
                norito::core::decode_byte_element_sequence_into(
                    &encoded[..length],
                    &mut destination,
                )
            });
            assert!(result.is_err());
            assert_eq!(calls, 0);
        }
        let mut malformed = encoded.clone();
        // A length-prefixed scalar byte must have length 1.
        malformed[8] = 2;
        let (result, calls) = observe(|| {
            norito::core::decode_byte_element_sequence_into(&malformed, &mut destination)
        });
        assert!(result.is_err());
        assert_eq!(calls, 0);
        assert!(ConstVec::<u8>::decode_from_slice(&malformed).is_err());
        let too_few = DecodeLimits::new(2, usize::MAX, usize::MAX, usize::MAX, usize::MAX);
        let (result, _) = with_decode_limits_measured(too_few, || {
            norito::core::decode_byte_element_sequence_into(&encoded, &mut destination)
        });
        assert!(matches!(result, Err(Error::SequenceLengthExceeded { .. })));
    }
}

#[test]
fn reserved_layout_bits_are_rejected_by_actual_header_admission() {
    let key = key(Algorithm::Ed25519);
    let frame = norito::to_bytes(&key).unwrap();
    for bit in [
        header_flags::PACKED_SEQ,
        header_flags::PACKED_STRUCT,
        header_flags::VARINT_OFFSETS,
        header_flags::COMPACT_SEQ_LEN,
        header_flags::FIELD_BITSET,
        0x40,
        0x80,
    ] {
        let mut malformed = frame.clone();
        malformed[norito::core::Header::SIZE - 1] |= bit;
        let (result, calls) = observe(|| norito::core::Header::read(malformed.as_slice()));
        assert!(matches!(
            result,
            Err(Error::UnsupportedFeature("layout flag"))
        ));
        assert_eq!(calls, 0);
        assert!(matches!(
            norito::decode_from_bytes::<PublicKey>(&malformed),
            Err(Error::UnsupportedFeature("layout flag"))
        ));
    }
}

#[test]
fn fresh_thread_byte_sequence_relations_allocate_zero() {
    for flags in layouts() {
        let valid = binary(&[0, 0x41, 0xff], flags);
        let empty = binary(&[], flags);
        let mut wrong_length = valid.clone();
        wrong_length[8] = 2;
        for (bytes, expected) in [
            (valid.clone(), Some(3)),
            (empty, Some(0)),
            (wrong_length, None),
            (valid[..valid.len() - 1].to_vec(), None),
        ] {
            std::thread::spawn(move || {
                let _flags = DecodeFlagsGuard::enter(flags);
                let mut destination = [0; 3];
                let (result, calls) = observe(|| {
                    norito::core::decode_byte_element_sequence_into(&bytes, &mut destination)
                });
                assert_eq!(calls, 0);
                assert_eq!(result.ok().map(|(count, _)| count), expected);
            })
            .join()
            .unwrap();
        }
    }
}

#[test]
fn arbitrary_decoder_hook_has_separate_cold_process_custody() {
    const CHILD: &str = "IROHA_TEST_PUBLIC_KEY_HOOK_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let (cold, cold_calls) = observe(|| norito::core::decode_field_canonical::<u8>(&[0x41]));
        assert_eq!(cold.unwrap(), (0x41, 1));
        assert_eq!(cold_calls, 1);
        let (warm, warm_calls) = observe(|| norito::core::decode_field_canonical::<u8>(&[0x41]));
        assert_eq!(warm.unwrap(), (0x41, 1));
        assert_eq!(warm_calls, 0);
        return;
    }
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "arbitrary_decoder_hook_has_separate_cold_process_custody",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn every_algorithm_returns_fixed_allocation_failure_on_real_destination_refusal() {
    for &algorithm in algorithms() {
        let valid = key(algorithm);
        let (_, bytes) = valid.to_bytes();
        REFUSE_NEXT.with(|next| next.set(true));
        let (result, calls) = observe(|| PublicKey::from_bytes_for_decode(algorithm, bytes));
        assert_eq!(calls, 1);
        assert!(
            matches!(result,Err(Error::AllocationFailed { bytes: requested }) if requested==bytes.len() as u64+1)
        );
        assert!(!REFUSE_NEXT.with(Cell::get));
        assert_eq!(
            PublicKey::from_bytes_for_decode(algorithm, bytes).unwrap(),
            valid
        );
    }
}
