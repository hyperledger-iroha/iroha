//! Optional prepared children retain canonical framing, custody and refusal.

use iroha_allocation::{AllocationBudget, AllocationRefusal, ChargedBuffer, ChargedBufferError};
use norito::core::{
    CanonicalField, DecodeAttemptErrorKind, DecodeField, DecodeFlagsGuard, DecodeFromSlice,
    DecodeIntoError, DecodeLimits, Encoder, FieldDestination, PayloadRef, PreparedDecodeError,
    PreparedDecodeWorkspace, PreparedRecordDestination, SequenceDestinationError, SerializePayload,
    classify_decode_attempt, decode_field_canonical, decode_raw_byte_sequence_into, framed_field,
    header_flags, write_len_to_vec_with_flags,
};
use norito::{DeserializePayload, NoritoSchema, NoritoSerialize};
use sha2::{Digest, Sha256};
use std::convert::Infallible;

fn limits() -> DecodeLimits {
    DecodeLimits::new(4096, 1 << 20, 1 << 20, 1 << 20, 32)
}
fn workspace(pool: &AllocationBudget) -> PreparedDecodeWorkspace {
    let mut reservation = pool
        .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
        .unwrap();
    PreparedDecodeWorkspace::from_reservation(pool, &mut reservation).unwrap()
}
fn field_bytes(payload: &[u8], flags: u8) -> Vec<u8> {
    let mut field = Vec::new();
    write_len_to_vec_with_flags(&mut field, payload.len() as u64, flags);
    field.extend_from_slice(payload);
    field
}
fn field<T>(bytes: &[u8]) -> CanonicalField<'_, Option<T>>
where
    T: for<'de> DeserializePayload<'de> + SerializePayload,
{
    let mut offset = 0;
    let field = framed_field::<Option<T>>(bytes, &mut offset).unwrap();
    assert_eq!(offset, bytes.len());
    field
}
fn bool_child<E>(child: CanonicalField<'_, bool>) -> Result<bool, DecodeIntoError<E>> {
    child.with_payload(|bytes| {
        let (value, used) = bool::decode_from_slice(bytes)?;
        if used != bytes.len() {
            return Err(norito::Error::LengthMismatch.into());
        }
        Ok(value)
    })
}

#[test]
fn prepared_optional_child_matches_owning_none_some_nested_and_advertised_lengths() {
    let pool = AllocationBudget::new(4096);
    let mut work = workspace(&pool);
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        for value in [None, Some(false), Some(true)] {
            let payload = super::bare_bytes(&value, flags);
            let bytes = field_bytes(&payload, flags);
            let ordinary = decode_field_canonical::<Option<bool>>(&payload).unwrap();
            let mut result = None;
            let mut visits = 0;
            let allocations = super::allocations_during(|| {
                result = Some(
                    work.with_limits(limits(), limits(), || {
                        field::<bool>(&bytes).decode_optional::<_, Infallible>(|child| {
                            visits += 1;
                            bool_child(child)
                        })
                    })
                    .unwrap(),
                );
            });
            assert_eq!(allocations, 0);
            assert_eq!(result.unwrap().unwrap(), ordinary.0);
            assert_eq!(
                visits,
                usize::from(value.is_some()),
                "None never visits a child"
            );
            assert_eq!(ordinary.1, payload.len());
        }
        for value in [None, Some(None), Some(Some(false)), Some(Some(true))] {
            let payload = super::bare_bytes(&value, flags);
            let bytes = field_bytes(&payload, flags);
            let ordinary = decode_field_canonical::<Option<Option<bool>>>(&payload).unwrap();
            let mut result = None;
            let allocations = super::allocations_during(|| {
                result = Some(
                    work.with_limits(limits(), limits(), || {
                        field::<Option<bool>>(&bytes).decode_optional::<_, Infallible>(|child| {
                            child.decode_optional(bool_child)
                        })
                    })
                    .unwrap(),
                );
            });
            assert_eq!(allocations, 0);
            assert_eq!(result.unwrap().unwrap(), ordinary.0);
            assert_eq!(ordinary.1, payload.len());
        }
    }
    drop(work);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn prepared_optional_child_preserves_truncation_tag_flags_and_child_before_trailer_errors() {
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let valid = super::bare_bytes(&Some(true), flags);
        let mut payloads: Vec<Vec<u8>> =
            (0..valid.len()).map(|end| valid[..end].to_vec()).collect();
        payloads.push(vec![2]);
        payloads.push(vec![0, 9]);
        let mut trailing = valid.clone();
        trailing.push(9);
        payloads.push(trailing.clone());
        // An intrinsically invalid child precedes the same outer trailing byte.
        trailing[valid.len() - 1] = 2;
        payloads.push(trailing);
        for payload in payloads {
            let ordinary = decode_field_canonical::<Option<bool>>(&payload).unwrap_err();
            let bytes = field_bytes(&payload, flags);
            let prepared = field::<bool>(&bytes)
                .decode_optional::<_, Infallible>(bool_child)
                .unwrap_err()
                .into_codec();
            assert_eq!(ordinary.to_string(), prepared.to_string());
        }
        let invalid = vec![2];
        let owning_slice = Option::<bool>::decode_from_slice(&invalid).unwrap_err();
        assert!(
            matches!(owning_slice, norito::Error::Message(ref message) if message == "invalid option tag")
        );
        let owning_archive = decode_field_canonical::<Option<bool>>(&invalid).unwrap_err();
        assert!(matches!(
            owning_archive,
            norito::Error::InvalidTag {
                context: "Option::try_deserialize",
                tag: 2
            }
        ));
    }
    // Original fixed-width bytes cannot be reinterpreted as compact child lengths.
    let payload = super::bare_bytes(&Some(true), 0);
    let _flags = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
    let bytes = field_bytes(&payload, header_flags::COMPACT_LEN);
    let ordinary = decode_field_canonical::<Option<bool>>(&payload).unwrap_err();
    let prepared = field::<bool>(&bytes)
        .decode_optional::<_, Infallible>(bool_child)
        .unwrap_err()
        .into_codec();
    assert_eq!(ordinary.to_string(), prepared.to_string());
}

#[test]
fn prepared_optional_child_keeps_exact_enclosing_field_and_depth_refusal_provenance() {
    let pool = AllocationBudget::new(4096);
    let mut caller = workspace(&pool);
    let mut work = workspace(&pool);
    let flags = header_flags::COMPACT_LEN;
    let _flags = DecodeFlagsGuard::enter(flags);
    let payload = super::bare_bytes(&Some(true), flags);
    let bytes = field_bytes(&payload, flags);
    for limit in [
        DecodeLimits::new(4096, payload.len() - 1, 4096, 4096, 32),
        DecodeLimits::new(4096, 4096, 4096, 4096, 1),
    ] {
        caller
            .with_limits(limits(), limit, || {
                let ordinary = classify_decode_attempt(|| {
                    work.with_limits(limits(), limits(), || {
                        decode_field_canonical::<Option<bool>>(&payload).map(|_| ())
                    })
                    .unwrap()
                })
                .unwrap_err();
                let prepared = classify_decode_attempt(|| {
                    work.with_limits(limits(), limits(), || {
                        let mut offset = 0;
                        framed_field::<Option<bool>>(&bytes, &mut offset)?
                            .decode_optional::<_, Infallible>(bool_child)
                            .map(|_| ())
                            .map_err(DecodeIntoError::into_codec)
                    })
                    .unwrap()
                })
                .unwrap_err();
                assert_eq!(ordinary.kind(), DecodeAttemptErrorKind::EnclosingLimit);
                assert_eq!(prepared.kind(), DecodeAttemptErrorKind::EnclosingLimit);
                assert_eq!(ordinary.to_string(), prepared.to_string());
                assert_eq!(
                    ordinary.into_error().decode_resource_error(),
                    prepared.into_error().decode_resource_error()
                );
                let mut original_scope = None;
                let outer = classify_decode_attempt(|| {
                    work.with_limits(limits(), limits(), || {
                        let nested = classify_decode_attempt(|| {
                            let mut offset = 0;
                            framed_field::<Option<bool>>(&bytes, &mut offset)?
                                .decode_optional::<_, Infallible>(bool_child)
                                .map(|_| ())
                                .map_err(DecodeIntoError::into_codec)
                        })
                        .unwrap_err();
                        assert_eq!(nested.kind(), DecodeAttemptErrorKind::EnclosingLimit);
                        let cause = nested.into_error();
                        match &cause {
                            norito::Error::ScopedDecodeResource(original) => {
                                original_scope = Some(original.clone());
                            }
                            other => panic!("must retain original opaque caller cause: {other}"),
                        }
                        Err::<(), _>(cause)
                    })
                    .unwrap()
                })
                .unwrap_err();
                assert_eq!(outer.kind(), DecodeAttemptErrorKind::EnclosingLimit);
                match outer.into_error() {
                    norito::Error::ScopedDecodeResource(original) => {
                        assert_eq!(original_scope.unwrap(), original);
                    }
                    other => panic!("outer observer lost the original cause: {other}"),
                }
            })
            .unwrap();
    }
    drop((work, caller));
    assert_eq!(pool.reserved_bytes(), 0);
}

#[derive(Debug, PartialEq, NoritoSerialize, DeserializePayload, NoritoSchema)]
#[norito_schema(name = "prepared.OptionalRecord")]
#[norito(decode_fields)]
struct Record {
    bytes: Option<Vec<u8>>,
}
#[derive(NoritoSerialize)]
struct Filled<'a> {
    bytes: Option<PayloadRef<'a, &'a [u8]>>,
}
#[derive(Debug)]
enum DestinationError {
    Storage { available: usize, required: usize },
}
struct Destination {
    bytes: ChargedBuffer<u8>,
    value: Option<Option<usize>>,
}
impl FieldDestination for Destination {
    type Error = DestinationError;
}
impl DecodeField<0, Option<Vec<u8>>> for Destination {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, Option<Vec<u8>>>,
    ) -> Result<(), DecodeIntoError<Self::Error>> {
        self.value = Some(field.decode_optional(|child| {
            child.with_payload(|payload| {
                let (written, used) =
                    decode_raw_byte_sequence_into(payload, self.bytes.as_mut_slice()).map_err(
                        |error| match error {
                            SequenceDestinationError::Codec(error) => DecodeIntoError::Codec(error),
                            SequenceDestinationError::Storage {
                                available,
                                required,
                            } => DecodeIntoError::Destination(DestinationError::Storage {
                                available,
                                required,
                            }),
                        },
                    )?;
                if used != payload.len() {
                    return Err(norito::Error::LengthMismatch.into());
                }
                Ok(written)
            })
        })?);
        Ok(())
    }
}
impl SerializePayload for Destination {
    fn serialize(&self, encoder: &mut Encoder<'_>) -> Result<(), norito::Error> {
        let value = self.value.ok_or(norito::Error::LengthMismatch)?;
        let bytes = value.map(|length| &self.bytes.as_slice()[..length]);
        Filled {
            bytes: bytes.as_ref().map(PayloadRef),
        }
        .serialize(encoder)
    }
}
impl PreparedRecordDestination<Record> for Destination {
    fn reset(&mut self) {
        self.value = None;
    }
}

#[test]
fn prepared_optional_record_retains_original_saturated_backing_and_full_canonical_frame() {
    let pool = AllocationBudget::new(1 << 20);
    let mut work = workspace(&pool);
    let mut bytes = ChargedBuffer::new(31, &pool).unwrap();
    for _ in 0..31 {
        bytes.push_reserved(0_u8);
    }
    let mut destination = Destination { bytes, value: None };
    let pointer = destination.bytes.as_slice().as_ptr();
    for count in [0, 4, 31] {
        for value in [None, Some((0..count as u8).collect::<Vec<_>>())] {
            let expected = Record { bytes: value };
            let frame = norito::encode_canonical(&expected).unwrap();
            let original_hash = Sha256::digest(&frame);
            let held = pool
                .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
                .unwrap();
            let original_reserved = pool.reserved_bytes();
            let mut result = None;
            let allocations = super::allocations_during(|| {
                result = Some(work.decode_canonical_into::<Record, _>(
                    &frame,
                    limits(),
                    &mut destination,
                ));
            });
            assert_eq!(allocations, 0);
            result.unwrap().unwrap();
            assert_eq!(
                destination.value.unwrap(),
                expected.bytes.as_ref().map(Vec::len)
            );
            if let Some(value) = &expected.bytes {
                assert_eq!(&destination.bytes.as_slice()[..value.len()], value);
            }
            assert_eq!(destination.bytes.as_slice().as_ptr(), pointer);
            assert!(destination.bytes.belongs_to(&pool));
            assert_eq!(pool.reserved_bytes(), original_reserved);
            assert_eq!(Sha256::digest(&frame), original_hash);
            drop(held);
        }
    }
    let oversized = norito::encode_canonical(&Record {
        bytes: Some(vec![9; 32]),
    })
    .unwrap();
    let error = work
        .decode_canonical_into::<Record, _>(&oversized, limits(), &mut destination)
        .unwrap_err();
    assert!(matches!(
        error,
        PreparedDecodeError::Destination(DestinationError::Storage {
            available: 31,
            required: 32
        })
    ));
    assert!(destination.value.is_none());
    assert_eq!(destination.bytes.as_slice().as_ptr(), pointer);
    drop((work, destination));
    assert_eq!(pool.reserved_bytes(), 0);
}

fn charged_child(
    child: CanonicalField<'_, Vec<u8>>,
    pool: &AllocationBudget,
    capacity: usize,
) -> Result<ChargedBuffer<u8>, DecodeIntoError<ChargedBufferError>> {
    child.with_payload(|payload| {
        let mut output =
            ChargedBuffer::new(capacity, pool).map_err(DecodeIntoError::Destination)?;
        for _ in 0..capacity {
            output.push_reserved(0_u8);
        }
        let (written, used) = decode_raw_byte_sequence_into(payload, output.as_mut_slice())
            .map_err(|error| match error {
                SequenceDestinationError::Codec(error) => DecodeIntoError::Codec(error),
                _ => panic!("original fixture fits its exact planned leaf"),
            })?;
        if written != capacity || used != payload.len() {
            return Err(norito::Error::LengthMismatch.into());
        }
        Ok(output)
    })
}

#[test]
fn prepared_optional_child_returns_real_pool_and_allocator_refusals_with_unchanged_source_retry() {
    let pool = AllocationBudget::new(4096);
    let mut work = workspace(&pool);
    let flags = header_flags::COMPACT_LEN;
    let _flags = DecodeFlagsGuard::enter(flags);
    let expected = (0..31).collect::<Vec<u8>>();
    let payload = super::bare_bytes(&Some(expected.clone()), flags);
    let wire = field_bytes(&payload, flags);
    let mut source = ChargedBuffer::new(wire.len(), &pool).unwrap();
    source.append(&wire).unwrap();
    let pointer = source.as_slice().as_ptr();
    let original_hash = Sha256::digest(source.as_slice());
    let floor = pool.reserved_bytes();
    let held = pool.try_reserve_bytes(pool.limit_bytes() - floor).unwrap();
    let mut result = None;
    let allocations = super::allocations_during(|| {
        result = Some(
            work.with_limits(limits(), limits(), || {
                field::<Vec<u8>>(source.as_slice())
                    .decode_optional(|child| charged_child(child, &pool, 31))
            })
            .unwrap(),
        );
    });
    assert_eq!(allocations, 0);
    let error = match result.unwrap() {
        Err(error) => error,
        Ok(_) => panic!("real original pool is saturated"),
    };
    assert!(matches!(
        error,
        DecodeIntoError::Destination(ChargedBufferError::Admission(AllocationRefusal::Capacity {
            requested_bytes: 31,
            reserved_bytes,
            limit_bytes,
            ..
        })) if reserved_bytes == pool.limit_bytes() && limit_bytes == pool.limit_bytes()
    ));
    assert_eq!(pool.reserved_bytes(), pool.limit_bytes());
    drop(held);
    struct RestoreRefusal(usize, usize);
    impl Drop for RestoreRefusal {
        fn drop(&mut self) {
            super::REFUSE_SIZE.set(self.0);
            super::MATCHES_BEFORE_REFUSAL.set(self.1);
        }
    }
    let refusal = RestoreRefusal(
        super::REFUSE_SIZE.replace(31),
        super::MATCHES_BEFORE_REFUSAL.replace(0),
    );
    let result = work
        .with_limits(limits(), limits(), || {
            field::<Vec<u8>>(source.as_slice())
                .decode_optional(|child| charged_child(child, &pool, 31))
        })
        .unwrap();
    drop(refusal);
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("real child backing allocator must refuse"),
    };
    assert!(matches!(
        error,
        DecodeIntoError::Destination(ChargedBufferError::Allocator {
            requested_bytes: 31
        })
    ));
    assert_eq!(pool.reserved_bytes(), floor);
    assert_eq!(source.as_slice().as_ptr(), pointer);
    assert_eq!(Sha256::digest(source.as_slice()), original_hash);
    let output = work
        .with_limits(limits(), limits(), || {
            field::<Vec<u8>>(source.as_slice())
                .decode_optional(|child| charged_child(child, &pool, 31))
        })
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(output.as_slice(), expected);
    assert!(output.belongs_to(&pool));
    assert_eq!(pool.reserved_bytes(), floor + 31);
    drop(output);
    assert_eq!(pool.reserved_bytes(), floor);
    drop((source, work));
    assert_eq!(pool.reserved_bytes(), 0);
}
