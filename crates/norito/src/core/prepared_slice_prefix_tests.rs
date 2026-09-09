//! Prepared payload prefix consumption, state restoration, and budget checks.

use super::*;

#[derive(Debug, PartialEq)]
struct BytePrefix {
    value: u8,
    flags: Option<u8>,
}

// Deliberately decoder-only: this boundary must not need encoding or a frame.
impl<'de> DeserializePayload<'de> for BytePrefix {
    fn deserialize(archived: &'de Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("test byte prefix")
    }

    fn try_deserialize(archived: &'de Archived<Self>) -> Result<Self, Error> {
        let ptr = archived as *const _ as *const u8;
        let mut offset = 0;
        let value = decode_context_field_fixed_canonical::<u8>(ptr, &mut offset, 1)?;
        if value == 0 {
            return Err(Error::NonCanonicalEncoding);
        }
        finish_context_fields(ptr, offset)?;
        Ok(Self {
            value,
            flags: effective_decode_flags(),
        })
    }
}

#[derive(Debug, PartialEq)]
struct EmptyPrefix;

impl<'de> DeserializePayload<'de> for EmptyPrefix {
    fn deserialize(_archived: &'de Archived<Self>) -> Self {
        Self
    }
}

fn prepare<T>(bytes: &[u8]) -> PreparedDecodeSlice<'_> {
    prepare_decode_from_slice(
        bytes,
        archived_payload_size::<T>(),
        archived_payload_align::<T>(),
    )
    .expect("prepare test prefix")
}

#[test]
fn prepared_prefix_preserves_used_flags_errors_and_caller_boundary() {
    reset_decode_state();
    let caller = [0xaa, 0xbb];
    let valid = prepare::<BytePrefix>(&[7, 0xff]);
    let invalid = prepare::<BytePrefix>(&[0, 0xff]);
    for flags in [0, header_flags::COMPACT_LEN, header_flags::PACKED_STRUCT] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let _caller = PayloadCtxGuard::enter(&caller);
        for boundary in [FieldDecodeBoundary::Canonical, FieldDecodeBoundary::Prefix] {
            let _boundary = FieldDecodeBoundaryGuard::enter(boundary);
            assert_eq!(
                decode_prepared_slice_prefix::<BytePrefix>(&valid).unwrap(),
                (
                    BytePrefix {
                        value: 7,
                        flags: Some(flags)
                    },
                    1
                )
            );
            assert!(matches!(
                decode_prepared_slice_prefix::<BytePrefix>(&invalid),
                Err(Error::NonCanonicalEncoding)
            ));
            assert_eq!(effective_decode_flags(), Some(flags));
            assert_eq!(
                payload_ctx(),
                Some((caller.as_ptr() as usize, caller.len()))
            );
            let finish = finish_context_fields(caller.as_ptr(), 1);
            match boundary {
                FieldDecodeBoundary::Canonical => {
                    assert!(matches!(finish, Err(Error::LengthMismatch)))
                }
                FieldDecodeBoundary::Prefix => assert!(finish.is_ok()),
            }
        }
    }
    assert!(payload_ctx().is_none());
    assert_eq!(effective_decode_flags(), None);
}

#[test]
fn prepared_prefix_zero_consumption_is_valid_without_a_caller_context() {
    reset_decode_state();
    for bytes in [&[][..], &[0xff][..]] {
        let prepared = prepare::<EmptyPrefix>(bytes);
        assert_eq!(
            decode_prepared_slice_prefix::<EmptyPrefix>(&prepared).unwrap(),
            (EmptyPrefix, 0)
        );
        assert!(payload_ctx().is_none());
        assert_eq!(effective_decode_flags(), None);
    }
}

#[test]
fn prepared_prefix_keeps_active_field_and_nesting_budgets() {
    reset_decode_state();
    let prepared = prepare::<BytePrefix>(&[7, 0xff]);
    let _flags = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
    let field_limit = DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, usize::MAX);
    assert!(matches!(
        with_decode_limits(field_limit, || decode_prepared_slice_prefix::<BytePrefix>(
            &prepared
        )),
        Err(Error::FieldLengthExceeded {
            length: 1,
            limit: 0
        })
    ));
    let depth_limit = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 0);
    assert!(matches!(
        with_decode_limits(depth_limit, || decode_prepared_slice_prefix::<BytePrefix>(
            &prepared
        )),
        Err(Error::NestingDepthExceeded {
            depth: 1,
            limit: 0,
            ..
        })
    ));
    assert_eq!(effective_decode_flags(), Some(header_flags::COMPACT_LEN));
    assert!(payload_ctx().is_none());
    assert_eq!(
        decode_prepared_slice_prefix::<BytePrefix>(&prepared)
            .unwrap()
            .1,
        1
    );
}
