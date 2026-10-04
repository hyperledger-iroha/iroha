//! Prepared Strings retain owning errors, original caller storage and full wire.

use iroha_allocation::{AllocationBudget, AllocationRefusal, ChargedBuffer, ChargedBufferError};
use norito::core::{
    CanonicalField, DecodeAttemptErrorKind, DecodeField, DecodeFlagsGuard, DecodeFromSlice,
    DecodeIntoError, DecodeLimits, DecodeResourceError, Encoder, FieldDestination,
    PreparedDecodeError, PreparedDecodeWorkspace, PreparedRecordDestination, SerializePayload,
    StringDestinationError, borrow_canonical_string, classify_decode_attempt,
    decode_field_canonical, decode_field_prefix, decode_string_into, framed_field, header_flags,
    write_len_to_vec_with_flags,
};
use norito::{DeserializePayload, NoritoSchema, NoritoSerialize};
use sha2::{Digest, Sha256};
use std::convert::Infallible;

fn limits(bytes: usize) -> DecodeLimits {
    DecodeLimits::new(1 << 20, 1 << 20, 1 << 20, bytes, 32)
}
fn workspace(pool: &AllocationBudget) -> PreparedDecodeWorkspace {
    let mut reservation = pool
        .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
        .unwrap();
    PreparedDecodeWorkspace::from_reservation(pool, &mut reservation).unwrap()
}
fn initialized(pool: &AllocationBudget, length: usize) -> ChargedBuffer<u8> {
    let mut bytes = ChargedBuffer::new(length, pool).unwrap();
    for _ in 0..length {
        bytes.push_reserved(0xa5);
    }
    bytes
}
fn payload(raw: &[u8], flags: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_len_to_vec_with_flags(&mut bytes, raw.len() as u64, flags);
    bytes.extend_from_slice(raw);
    bytes
}
fn codec(error: StringDestinationError) -> norito::Error {
    match error {
        StringDestinationError::Codec(error) => error,
        other => panic!("unexpected local String geometry: {other}"),
    }
}

#[test]
fn prepared_string_borrows_and_fills_original_utf8_for_both_advertised_lengths_without_allocating()
{
    let pool = AllocationBudget::new(1 << 20);
    let mut work = workspace(&pool);
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        for text in [String::new(), "abcd".into(), "é漢🙂".repeat(31)] {
            let bytes = super::bare_bytes(&text, flags);
            let original_hash = Sha256::digest(&bytes);
            let (owning, owning_used) = String::decode_from_slice(&bytes).unwrap();
            let (archived, archived_used) = decode_field_prefix::<String>(&bytes).unwrap();
            let mut destination = initialized(&pool, text.len() + 1);
            let pointer = destination.as_slice().as_ptr();
            let held = pool
                .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
                .unwrap();
            let mut borrowed = None;
            let mut filled = None;
            let allocations = super::allocations_during(|| {
                borrowed = Some(
                    work.with_limits(limits(1 << 20), limits(1 << 20), || {
                        borrow_canonical_string(&bytes)
                    })
                    .unwrap()
                    .unwrap(),
                );
                filled = Some(
                    work.with_limits(limits(1 << 20), limits(1 << 20), || {
                        decode_string_into(&bytes, destination.as_mut_slice())
                    })
                    .unwrap()
                    .unwrap(),
                );
            });
            assert_eq!(allocations, 0);
            let (value, used) = borrowed.unwrap();
            assert_eq!(value, text);
            assert_eq!((owning, archived), (text.clone(), text.clone()));
            assert_eq!(
                (used, owning_used, archived_used),
                (bytes.len(), used, used)
            );
            let prefix = bytes.len() - text.len();
            assert_eq!(value.as_ptr(), bytes[prefix..].as_ptr());
            assert_eq!(filled.unwrap(), (text.len(), used));
            assert_eq!(&destination.as_slice()[..text.len()], text.as_bytes());
            assert_eq!(destination.as_slice()[text.len()], 0xa5);
            assert_eq!(destination.as_slice().as_ptr(), pointer);
            assert!(destination.belongs_to(&pool));
            assert_eq!(pool.reserved_bytes(), pool.limit_bytes());
            assert_eq!(Sha256::digest(&bytes), original_hash);
            drop((held, destination));
        }
    }
    drop(work);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn prepared_string_keeps_owning_utf8_truncation_and_short_destination_error_order() {
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let good = super::bare_bytes(&"é漢🙂".to_owned(), flags);
        let mut destination = [0xa5_u8; 16];
        for end in 0..good.len() {
            let ordinary = String::decode_from_slice(&good[..end]).unwrap_err();
            let borrowed = borrow_canonical_string(&good[..end]).unwrap_err();
            let filled = codec(decode_string_into(&good[..end], &mut destination).unwrap_err());
            assert_eq!(ordinary.to_string(), borrowed.to_string());
            assert_eq!(ordinary.to_string(), filled.to_string());
            assert_eq!(destination, [0xa5; 16]);
        }
        for raw in [&[0xff][..], &[0xc3][..], &[0xed, 0xa0, 0x80][..]] {
            let bad = payload(raw, flags);
            assert!(matches!(
                String::decode_from_slice(&bad),
                Err(norito::Error::InvalidUtf8)
            ));
            assert!(matches!(
                decode_field_canonical::<String>(&bad),
                Err(norito::Error::InvalidUtf8)
            ));
            assert!(matches!(
                borrow_canonical_string(&bad),
                Err(norito::Error::InvalidUtf8)
            ));
            // Malformed original bytes take precedence over even empty output.
            assert!(matches!(
                decode_string_into(&bad, &mut []),
                Err(StringDestinationError::Codec(norito::Error::InvalidUtf8))
            ));
            assert_eq!(destination, [0xa5; 16]);
        }
        let short = decode_string_into(&good, &mut destination[..8]).unwrap_err();
        assert!(matches!(
            short,
            StringDestinationError::Storage {
                available: 8,
                required: 9
            }
        ));
        assert_eq!(destination, [0xa5; 16]);
        decode_string_into(&good, &mut destination).unwrap();
        assert_eq!(&destination[..9], "é漢🙂".as_bytes());
        assert_eq!(&destination[9..], &[0xa5; 7]);
        let mut trailing = good.clone();
        trailing.extend_from_slice(&[0xff, 0xee]);
        assert_eq!(borrow_canonical_string(&trailing).unwrap().1, good.len());
        assert_eq!(String::decode_from_slice(&trailing).unwrap().1, good.len());
        assert!(matches!(
            decode_field_canonical::<String>(&trailing),
            Err(norito::Error::LengthMismatch)
        ));
    }
}

#[test]
fn prepared_string_preserves_declared_logical_charge_before_body_utf8_and_each_real_pass() {
    let pool = AllocationBudget::new(4096);
    let mut work = workspace(&pool);
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let good = payload(b"abcdef", flags);
        let malformed = payload(&[0xff; 6], flags);
        for bytes in [&good[..], &malformed[..], &good[..good.len() - 1]] {
            let ordinary = work
                .with_limits(limits(5), limits(5), || String::decode_from_slice(bytes))
                .unwrap()
                .unwrap_err();
            let prepared = work
                .with_limits(limits(5), limits(5), || borrow_canonical_string(bytes))
                .unwrap()
                .unwrap_err();
            assert_eq!(
                ordinary.decode_resource_error(),
                prepared.decode_resource_error()
            );
            assert_eq!(
                prepared.decode_resource_error(),
                Some(DecodeResourceError::TotalAllocationExceeded {
                    attempted: 6,
                    limit: 5
                })
            );
        }
        let mut destination = [0xa5; 6];
        let mut result = None;
        assert_eq!(
            super::allocations_during(|| {
                result = Some(
                    work.with_limits(limits(11), limits(11), || {
                        borrow_canonical_string(&good)?;
                        decode_string_into(&good, &mut destination).map_err(codec)
                    })
                    .unwrap(),
                );
            }),
            0
        );
        assert_eq!(
            result.unwrap().unwrap_err().decode_resource_error(),
            Some(DecodeResourceError::TotalAllocationExceeded {
                attempted: 12,
                limit: 11
            })
        );
        assert_eq!(destination, [0xa5; 6]);
        work.with_limits(limits(12), limits(12), || {
            borrow_canonical_string(&good).unwrap();
            decode_string_into(&good, &mut destination).unwrap();
        })
        .unwrap();
        assert_eq!(&destination, b"abcdef");
    }
    drop(work);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn prepared_string_keeps_exact_enclosing_cause_field_depth_and_same_workspace_retry() {
    let pool = AllocationBudget::new(4096);
    let mut caller = workspace(&pool);
    let mut work = workspace(&pool);
    let flags = header_flags::COMPACT_LEN;
    let _flags = DecodeFlagsGuard::enter(flags);
    let bytes = payload(b"abcdef", flags);
    let original_hash = Sha256::digest(&bytes);
    let original_pointer = bytes.as_ptr();
    let mut output = [0xa5; 6];
    let mut nested_original = None;
    let cause = caller
        .with_limits(limits(1 << 20), limits(5), || {
            classify_decode_attempt(|| {
                work.with_limits(limits(1 << 20), limits(1 << 20), || {
                    let nested = classify_decode_attempt(|| {
                        decode_string_into(&bytes, &mut output).map_err(codec)
                    })
                    .unwrap_err();
                    assert_eq!(nested.kind(), DecodeAttemptErrorKind::EnclosingLimit);
                    let cause = nested.into_error();
                    match &cause {
                        norito::Error::ScopedDecodeResource(original) => {
                            nested_original = Some(original.clone());
                        }
                        other => panic!("nested reader lost the original refusal: {other}"),
                    }
                    Err::<(), _>(cause)
                })
                .unwrap()
            })
            .unwrap_err()
        })
        .unwrap();
    assert_eq!(cause.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    assert_eq!(output, [0xa5; 6]);
    let original = match cause.into_error() {
        norito::Error::ScopedDecodeResource(original) => original,
        other => panic!("the original caller refusal must be retained: {other}"),
    };
    assert_eq!(nested_original.take().unwrap(), original);
    work.with_limits(limits(1 << 20), limits(1 << 20), || {
        decode_string_into(&bytes, &mut output).unwrap()
    })
    .unwrap();
    assert_eq!(&output, b"abcdef");
    let outer = classify_decode_attempt(|| {
        Err::<(), _>(norito::Error::ScopedDecodeResource(original.clone()))
    })
    .unwrap_err();
    // Without the original owning observer, an old cause cannot acquire a new
    // enclosing classification merely because the same workspace was retried.
    assert_eq!(outer.kind(), DecodeAttemptErrorKind::Invalid);
    match outer.into_error() {
        norito::Error::ScopedDecodeResource(retained) => assert_eq!(retained, original),
        other => panic!("unchanged original scope must survive retry: {other}"),
    }
    let field_bytes = payload(&bytes, flags);
    for limit in [
        DecodeLimits::new(4096, bytes.len() - 1, 4096, 4096, 32),
        DecodeLimits::new(4096, 4096, 4096, 4096, 0),
    ] {
        caller
            .with_limits(limits(1 << 20), limit, || {
                let ordinary = classify_decode_attempt(|| {
                    work.with_limits(limits(1 << 20), limits(1 << 20), || {
                        decode_field_canonical::<String>(&bytes).map(|_| ())
                    })
                    .unwrap()
                })
                .unwrap_err();
                let prepared = classify_decode_attempt(|| {
                    work.with_limits(limits(1 << 20), limits(1 << 20), || {
                        let mut offset = 0;
                        framed_field::<String>(&field_bytes, &mut offset)?
                            .with_payload::<_, Infallible>(|bytes| {
                                borrow_canonical_string(bytes)?;
                                Ok(())
                            })
                            .map_err(DecodeIntoError::into_codec)
                    })
                    .unwrap()
                })
                .unwrap_err();
                assert_eq!(ordinary.kind(), DecodeAttemptErrorKind::EnclosingLimit);
                assert_eq!(prepared.kind(), DecodeAttemptErrorKind::EnclosingLimit);
                assert_eq!(
                    ordinary.into_error().decode_resource_error(),
                    prepared.into_error().decode_resource_error()
                );
            })
            .unwrap();
    }
    assert_eq!(bytes.as_ptr(), original_pointer);
    assert_eq!(Sha256::digest(&bytes), original_hash);
    drop((original, work, caller));
    assert_eq!(pool.reserved_bytes(), 0);
}

#[derive(Debug, PartialEq, NoritoSerialize, DeserializePayload, NoritoSchema)]
#[norito_schema(name = "prepared.StringRecord")]
#[norito(decode_fields)]
struct Record {
    text: String,
}
#[derive(NoritoSerialize)]
struct Filled<'a> {
    text: &'a str,
}
struct Destination {
    bytes: ChargedBuffer<u8>,
    length: Option<usize>,
}
impl FieldDestination for Destination {
    type Error = StringDestinationError;
}
impl DecodeField<0, String> for Destination {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, String>,
    ) -> Result<(), DecodeIntoError<Self::Error>> {
        field.with_payload(|bytes| {
            let (written, used) = decode_string_into(bytes, self.bytes.as_mut_slice()).map_err(
                |error| match error {
                    StringDestinationError::Codec(error) => DecodeIntoError::Codec(error),
                    error @ StringDestinationError::Storage { .. } => {
                        DecodeIntoError::Destination(error)
                    }
                },
            )?;
            if used != bytes.len() {
                return Err(norito::Error::LengthMismatch.into());
            }
            self.length = Some(written);
            Ok(())
        })
    }
}
impl SerializePayload for Destination {
    fn serialize(&self, encoder: &mut Encoder<'_>) -> Result<(), norito::Error> {
        let length = self.length.ok_or(norito::Error::LengthMismatch)?;
        let text = std::str::from_utf8(&self.bytes.as_slice()[..length])
            .map_err(|_| norito::Error::InvalidUtf8)?;
        Filled { text }.serialize(encoder)
    }
}
impl PreparedRecordDestination<Record> for Destination {
    fn reset(&mut self) {
        self.length = None;
    }
}

#[test]
fn prepared_string_record_checks_full_generated_canonical_frame_under_saturated_original_pool() {
    let pool = AllocationBudget::new(1 << 20);
    let mut work = workspace(&pool);
    let mut destination = Destination {
        bytes: initialized(&pool, 31),
        length: None,
    };
    let pointer = destination.bytes.as_slice().as_ptr();
    for text in [String::new(), "abcd".into(), "x".repeat(31)] {
        let record = Record { text };
        let frame = norito::encode_canonical(&record).unwrap();
        assert_eq!(norito::decode_canonical::<Record>(&frame).unwrap(), record);
        let original_hash = Sha256::digest(&frame);
        let held = pool
            .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
            .unwrap();
        let mut result = None;
        assert_eq!(
            super::allocations_during(|| {
                result = Some(work.decode_canonical_into::<Record, _>(
                    &frame,
                    limits(1 << 20),
                    &mut destination,
                ));
            }),
            0
        );
        result.unwrap().unwrap();
        assert_eq!(destination.length, Some(record.text.len()));
        assert_eq!(
            &destination.bytes.as_slice()[..record.text.len()],
            record.text.as_bytes()
        );
        assert_eq!(destination.bytes.as_slice().as_ptr(), pointer);
        assert!(destination.bytes.belongs_to(&pool));
        assert_eq!(pool.reserved_bytes(), pool.limit_bytes());
        assert_eq!(Sha256::digest(&frame), original_hash);
        drop(held);
        let mut trailing = frame.clone();
        trailing.push(0);
        assert!(
            work.decode_canonical_into::<Record, _>(&trailing, limits(1 << 20), &mut destination)
                .is_err()
        );
        assert!(destination.length.is_none());
    }
    let frame = norito::encode_canonical(&Record {
        text: "x".repeat(32),
    })
    .unwrap();
    assert!(matches!(
        work.decode_canonical_into::<Record, _>(&frame, limits(1 << 20), &mut destination),
        Err(PreparedDecodeError::Destination(
            StringDestinationError::Storage {
                available: 31,
                required: 32
            }
        ))
    ));
    assert!(destination.length.is_none());
    assert_eq!(destination.bytes.as_slice().as_ptr(), pointer);
    drop((destination, work));
    assert_eq!(pool.reserved_bytes(), 0);
}

struct RestoreRefusal(usize, usize);
impl Drop for RestoreRefusal {
    fn drop(&mut self) {
        super::REFUSE_SIZE.set(self.0);
        super::MATCHES_BEFORE_REFUSAL.set(self.1);
    }
}
fn refuse(size: usize) -> RestoreRefusal {
    RestoreRefusal(
        super::REFUSE_SIZE.replace(size),
        super::MATCHES_BEFORE_REFUSAL.replace(0),
    )
}

#[test]
fn prepared_string_preserves_real_pool_and_allocator_refusal_then_original_source_retry() {
    let pool = AllocationBudget::new(4096);
    let mut work = workspace(&pool);
    let flags = header_flags::COMPACT_LEN;
    let _flags = DecodeFlagsGuard::enter(flags);
    let wire = payload(&[b'x'; 31], flags);
    let mut source = ChargedBuffer::new(wire.len(), &pool).unwrap();
    source.append(&wire).unwrap();
    let source_pointer = source.as_slice().as_ptr();
    let source_hash = Sha256::digest(source.as_slice());
    let floor = pool.reserved_bytes();
    let held = pool.try_reserve_bytes(pool.limit_bytes() - floor).unwrap();
    let mut result = None;
    assert_eq!(
        super::allocations_during(|| {
            result = Some(
                work.with_limits(limits(1 << 20), limits(1 << 20), || {
                    let (text, _) = borrow_canonical_string(source.as_slice()).unwrap();
                    ChargedBuffer::<u8>::new(text.len(), &pool)
                })
                .unwrap(),
            );
        }),
        0
    );
    assert!(matches!(
        result.unwrap(),
        Err(ChargedBufferError::Admission(AllocationRefusal::Capacity {
            requested_bytes: 31,
            reserved_bytes,
            limit_bytes,
            ..
        })) if reserved_bytes == pool.limit_bytes() && limit_bytes == pool.limit_bytes()
    ));
    assert_eq!(pool.reserved_bytes(), pool.limit_bytes());
    drop(held);
    let refusal = refuse(31);
    let error = ChargedBuffer::<u8>::new(31, &pool).err().unwrap();
    assert!(matches!(
        error,
        ChargedBufferError::Allocator {
            requested_bytes: 31
        }
    ));
    assert!(matches!(
        String::decode_from_slice(source.as_slice()),
        Err(norito::Error::AllocationFailed { bytes: 31 })
    ));
    assert_eq!(pool.reserved_bytes(), floor);
    drop(refusal);
    let mut output = initialized(&pool, 31);
    let output_pointer = output.as_slice().as_ptr();
    let refusal = refuse(31);
    let mut result = None;
    assert_eq!(
        super::allocations_during(|| {
            result = Some(
                work.with_limits(limits(1 << 20), limits(1 << 20), || {
                    decode_string_into(source.as_slice(), output.as_mut_slice())
                })
                .unwrap(),
            );
        }),
        0
    );
    result.unwrap().unwrap();
    drop(refusal);
    assert_eq!(output.as_slice(), &[b'x'; 31]);
    assert_eq!(output.as_slice().as_ptr(), output_pointer);
    assert!(output.belongs_to(&pool));
    assert_eq!(source.as_slice().as_ptr(), source_pointer);
    assert_eq!(Sha256::digest(source.as_slice()), source_hash);
    assert_eq!(pool.reserved_bytes(), floor + output.capacity());
    drop(output);
    assert_eq!(pool.reserved_bytes(), floor);
    drop((source, work));
    assert_eq!(pool.reserved_bytes(), 0);
}
