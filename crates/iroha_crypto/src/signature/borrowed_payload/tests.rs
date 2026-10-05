//! Original encoded spans, canonical parity and genuine retained-allocation controls.

use std::alloc::Layout;

use iroha_allocation::{
    AllocationBudget, AllocationCharge, AllocationRefusal, ChargedBuffer,
    ChargedBufferFromChargeError,
};
use norito::core::{
    DecodeFlagsGuard, DecodeFromSlice, DecodeLimits, Encoder, Error, SerializePayload, header_flags,
};

use super::*;
use crate::{
    Algorithm, ChargedSignature, Hash, KeyPair, SignaturePayloadError, test_allocations::*,
};

fn encode(value: &Signature) -> Vec<u8> {
    let mut bytes = Vec::new();
    value
        .serialize(&mut Encoder::for_buffer(&mut bytes))
        .unwrap();
    bytes
}
fn source(bytes: &[u8], pool: &AllocationBudget) -> ChargedBuffer<u8> {
    let mut source = ChargedBuffer::new(bytes.len(), pool).unwrap();
    source.append(bytes).unwrap();
    source
}
fn charge(pool: &AllocationBudget, length: usize) -> AllocationCharge {
    let layout = Layout::array::<u8>(length).unwrap();
    pool.try_reserve(layout).unwrap().try_split(layout).unwrap()
}
fn initialized(length: usize, pool: &AllocationBudget) -> ChargedBuffer<u8> {
    let mut bytes = ChargedBuffer::new(length, pool).unwrap();
    for _ in 0..length {
        bytes.push_reserved(0);
    }
    bytes
}
fn codec(error: BorrowedSignaturePayloadError) -> Error {
    match error {
        BorrowedSignaturePayloadError::Decode(PreparedCryptoDecodeError::Codec(original)) => {
            original
        }
        other => panic!("original canonical failure required: {other}"),
    }
}

#[test]
fn borrowed_signature_keeps_original_framed_span_and_complete_wire_with_zero_allocation_fill() {
    let pair = KeyPair::from_seed(vec![0x62; 32], Algorithm::Ed25519);
    let signed =
        Signature::try_new(pair.private_key(), b"canonical original signature source").unwrap();
    let values = [
        Signature::from_bytes(&[0x71; 3]),
        signed,
        Signature::from_bytes(&[0x73; 96]),
    ];
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        for value in &values {
            // The non-64-byte values are codec transport fixtures, not valid
            // signatures for an inferred algorithm. The middle one is genuinely signed.
            let payload = encode(value);
            let full_wire = norito::core::to_bytes(value).unwrap();
            let body_at = full_wire.len() - payload.len();
            assert_eq!(&full_wire[body_at..], payload);
            let pool = AllocationBudget::new(full_wire.len() + value.payload().len());
            let source = source(&full_wire, &pool);
            let source_pointer = source.as_slice().as_ptr();
            let source_hash = Hash::new(source.as_slice());
            let mut output = initialized(value.payload().len(), &pool);
            let output_pointer = output.as_slice().as_ptr();
            pool.set_limit_bytes(0);
            let borrowed = without_allocations(|| {
                Signature::borrow_canonical_payload(&source.as_slice()[body_at..])
            })
            .unwrap();
            assert_eq!(
                borrowed.encoded_payload().as_ptr(),
                source.as_slice()[body_at..].as_ptr()
            );
            assert_eq!(borrowed.encoded_payload(), payload);
            assert_eq!(borrowed.len(), value.payload().len());
            assert!(!borrowed.is_empty());
            assert_eq!(borrowed.used(), payload.len());
            assert!(
                borrowed.used() > borrowed.len(),
                "every byte retains its original framing"
            );
            assert_eq!(
                without_allocations(|| borrowed.fill_into(output.as_mut_slice())).unwrap(),
                payload.len()
            );
            let retained =
                without_allocations(|| ChargedSignature::try_from_preallocated(&pool, output))
                    .unwrap_or_else(|(_, error)| panic!("{error}"));
            assert_eq!(retained.get(), value);
            assert_eq!(retained.get().payload().as_ptr(), output_pointer);
            assert!(retained.belongs_to(&pool));
            assert_eq!(norito::core::to_bytes(retained.get()).unwrap(), full_wire);
            assert_eq!(source.as_slice().as_ptr(), source_pointer);
            assert_eq!(Hash::new(source.as_slice()), source_hash);
            assert_eq!(
                pool.reserved_bytes(),
                full_wire.len() + value.payload().len()
            );
            drop(retained);
            assert_eq!(pool.reserved_bytes(), full_wire.len());
            drop(source);
            assert_eq!(pool.reserved_bytes(), 0);
        }
    }
}

#[test]
fn borrowed_signature_uses_original_complete_framing_and_fixed_payload_error_precedence() {
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let valid = encode(&Signature::from_bytes(&[0x79; 3]));
        for length in 0..valid.len() {
            let original = Signature::decode_from_slice(&valid[..length]).unwrap_err();
            let borrowed =
                without_allocations(|| Signature::borrow_canonical_payload(&valid[..length]))
                    .err()
                    .expect("every actual truncation fails");
            assert_eq!(codec(borrowed).to_string(), original.to_string());
        }
        for (value, expected) in [
            (Signature::from_bytes(&[]), SignaturePayloadError::Empty),
            (
                Signature::from_bytes(&[0; 3]),
                SignaturePayloadError::AllZero,
            ),
        ] {
            let bytes = encode(&value);
            let original = Signature::decode_from_slice(&bytes).unwrap_err();
            let borrowed = without_allocations(|| Signature::borrow_canonical_payload(&bytes))
                .err()
                .expect("same fixed validity rejection");
            assert!(matches!(borrowed,
                BorrowedSignaturePayloadError::Decode(PreparedCryptoDecodeError::Signature(actual))
                    if actual == expected));
            assert_eq!(original.to_string(), expected.to_string());
            let mut trailing = bytes;
            trailing.push(0x43);
            // Complete framing errors win even when the payload is intrinsically empty/all zero.
            let original = Signature::decode_from_slice(&trailing).unwrap_err();
            let borrowed = without_allocations(|| Signature::borrow_canonical_payload(&trailing))
                .err()
                .expect("original complete extent required");
            assert!(matches!(codec(borrowed), Error::LengthMismatch));
            assert!(matches!(original, Error::LengthMismatch));
        }
        for length in [0, 2] {
            let mut malformed = 1_u64.to_le_bytes().to_vec();
            norito::core::write_len_to_vec_with_flags(&mut malformed, length, flags);
            malformed.extend_from_slice(&[0x71, 0x73]);
            let original = Signature::decode_from_slice(&malformed).unwrap_err();
            let borrowed = without_allocations(|| Signature::borrow_canonical_payload(&malformed))
                .err()
                .expect("each original byte needs exact length1");
            assert_eq!(codec(borrowed).to_string(), original.to_string());
        }
    }
}

#[test]
fn borrowed_signature_layout_and_geometry_refusal_leave_original_source_and_destination_untouched()
{
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let bytes = encode(&Signature::from_bytes(&[0x61; 3]));
        let pool = AllocationBudget::new(bytes.len() + 4);
        let source = source(&bytes, &pool);
        let mut output = initialized(4, &pool);
        output.as_mut_slice().fill(0x55);
        let pointer = output.as_slice().as_ptr();
        let source_pointer = source.as_slice().as_ptr();
        let source_hash = Hash::new(source.as_slice());
        let borrowed =
            without_allocations(|| Signature::borrow_canonical_payload(source.as_slice())).unwrap();
        let error = without_allocations(|| borrowed.fill_into(output.as_mut_slice())).unwrap_err();
        assert!(matches!(
            error,
            BorrowedSignaturePayloadError::Decode(PreparedCryptoDecodeError::Geometry {
                expected: 4,
                offered: 3
            })
        ));
        assert_eq!(output.as_slice(), &[0x55; 4]);
        let other = flags ^ header_flags::COMPACT_LEN;
        {
            let _changed = DecodeFlagsGuard::enter(other);
            let error = without_allocations(|| borrowed.fill_into(&mut output.as_mut_slice()[..3]))
                .unwrap_err();
            assert!(
                matches!(error, BorrowedSignaturePayloadError::LayoutChanged {
                expected, actual } if expected == flags && actual == other)
            );
            assert_eq!(output.as_slice(), &[0x55; 4]);
        }
        pool.set_limit_bytes(0);
        assert_eq!(
            without_allocations(|| borrowed.fill_into(&mut output.as_mut_slice()[..3])).unwrap(),
            bytes.len()
        );
        assert_eq!(output.as_slice(), &[0x61, 0x61, 0x61, 0x55]);
        assert_eq!(output.as_slice().as_ptr(), pointer);
        assert_eq!(source.as_slice().as_ptr(), source_pointer);
        assert_eq!(Hash::new(source.as_slice()), source_hash);
        assert_eq!(pool.reserved_bytes(), bytes.len() + 4);
        drop((source, output));
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

#[test]
fn borrowed_signature_real_capacity_and_allocator_refusals_keep_same_admitted_leaf_for_retry() {
    let value = Signature::from_bytes(&[0x53; 96]);
    let bytes = encode(&value);
    let pool = AllocationBudget::new(bytes.len() + 96);
    let source = source(&bytes, &pool);
    let source_pointer = source.as_slice().as_ptr();
    let source_hash = Hash::new(source.as_slice());
    let borrowed =
        without_allocations(|| Signature::borrow_canonical_payload(source.as_slice())).unwrap();
    let blocker = pool.try_reserve_bytes(1).unwrap();
    let refused = pool.try_reserve_bytes(borrowed.len()).unwrap_err();
    assert!(
        matches!(refused, AllocationRefusal::Capacity { requested_bytes:96,
        reserved_bytes, limit_bytes, .. } if reserved_bytes == bytes.len()+1 && limit_bytes == bytes.len()+96)
    );
    assert_eq!(source.as_slice().as_ptr(), source_pointer);
    assert_eq!(Hash::new(source.as_slice()), source_hash);
    drop(blocker);
    let original = charge(&pool, borrowed.len());
    let (result, attempts) = with_allocation_failure(borrowed.len(), || {
        allocations_during(|| ChargedBuffer::<u8>::try_from_charge(borrowed.len(), original))
    });
    let (original, error) = result.err().expect("actual physical allocator refusal");
    assert_eq!(attempts, 1);
    assert_eq!(
        error,
        ChargedBufferFromChargeError::Allocator {
            layout: Layout::array::<u8>(96).unwrap()
        }
    );
    assert_eq!(original.layout(), Layout::array::<u8>(96).unwrap());
    assert!(original.belongs_to(&pool));
    assert_eq!(pool.reserved_bytes(), bytes.len() + 96);
    pool.set_limit_bytes(0);
    let (output, attempts) =
        allocations_during(|| ChargedBuffer::<u8>::try_from_charge(96, original));
    let mut output = output.unwrap_or_else(|(_, error)| panic!("{error}"));
    assert_eq!(attempts, 1);
    for _ in 0..output.capacity() {
        output.push_reserved(0);
    }
    let pointer = output.as_slice().as_ptr();
    without_allocations(|| borrowed.fill_into(output.as_mut_slice())).unwrap();
    let retained = without_allocations(|| ChargedSignature::try_from_preallocated(&pool, output))
        .unwrap_or_else(|(_, error)| panic!("{error}"));
    assert_eq!(retained.get(), &value);
    assert_eq!(retained.get().payload().as_ptr(), pointer);
    assert_eq!(source.as_slice().as_ptr(), source_pointer);
    assert_eq!(Hash::new(source.as_slice()), source_hash);
    assert_eq!(pool.reserved_bytes(), bytes.len() + 96);
    drop(retained);
    assert_eq!(pool.reserved_bytes(), bytes.len());
    drop(source);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn borrowed_signature_original_enclosing_count_cause_survives_source_fill_retry() {
    let bytes = encode(&Signature::from_bytes(&[0x53; 64]));
    let pool = AllocationBudget::new(bytes.len() + 64);
    let source = source(&bytes, &pool);
    let mut output = initialized(64, &pool);
    let pointer = output.as_slice().as_ptr();
    let source_pointer = source.as_slice().as_ptr();
    let source_hash = Hash::new(source.as_slice());
    let borrowed = Signature::borrow_canonical_payload(source.as_slice()).unwrap();
    let caller = DecodeLimits::new(1024, 4096, 1024, 63, 32);
    let protocol = DecodeLimits::new(1024, 4096, 1024, 64, 32);
    // These original logical scopes are component fixture owners; their controls
    // are not claimed as a funded enclosing production graph.
    let original = ncore::with_decode_limits_scope(caller, || {
        ncore::classify_decode_attempt(|| {
            ncore::with_decode_limits_scope(protocol, || {
                Signature::decode_from_slice(source.as_slice())
            })
        })
    })
    .unwrap_err();
    let refusal = ncore::with_decode_limits_scope(caller, || {
        ncore::classify_decode_attempt(|| {
            ncore::with_decode_limits_scope(protocol, || {
                match without_allocations(|| borrowed.fill_into(output.as_mut_slice())) {
                    Err(error) => Err::<usize, _>(codec(error)),
                    Ok(_) => panic!("same original count must exceed enclosing budget"),
                }
            })
        })
    })
    .unwrap_err();
    assert_eq!(
        refusal.kind(),
        ncore::DecodeAttemptErrorKind::EnclosingLimit
    );
    assert_eq!(refusal.kind(), original.kind());
    assert_eq!(refusal.to_string(), original.to_string());
    assert_eq!(output.as_slice(), &[0; 64]);
    assert_eq!(output.as_slice().as_ptr(), pointer);
    assert_eq!(pool.reserved_bytes(), bytes.len() + 64);
    pool.set_limit_bytes(0);
    without_allocations(|| borrowed.fill_into(output.as_mut_slice())).unwrap();
    assert_eq!(output.as_slice(), &[0x53; 64]);
    assert_eq!(
        refusal.into_error().decode_resource_error(),
        original.into_error().decode_resource_error()
    );
    assert_eq!(source.as_slice().as_ptr(), source_pointer);
    assert_eq!(Hash::new(source.as_slice()), source_hash);
    drop((source, output));
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn ordinary_signature_keeps_actual_allocation_refusal_before_late_framing_rejection() {
    let mut bytes = encode(&Signature::from_bytes(&[0x53; 23]));
    bytes.push(0x99);
    let (original, allocations) = with_allocation_failure(23, || {
        allocations_during(|| Signature::decode_from_slice(&bytes))
    });
    assert_eq!(allocations, 1);
    assert!(matches!(
        original,
        Err(Error::AllocationFailed { bytes: 23 })
    ));
    let borrowed = without_allocations(|| Signature::borrow_canonical_payload(&bytes))
        .err()
        .expect("borrowed source checks complete framing without owning allocation");
    assert!(matches!(codec(borrowed), Error::LengthMismatch));
    assert!(matches!(
        Signature::decode_from_slice(&bytes),
        Err(Error::LengthMismatch)
    ));
}
