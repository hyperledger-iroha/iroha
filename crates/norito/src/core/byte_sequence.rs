//! Advertised element-sequence traversal with caller-owned byte destinations.

use super::*;

/// Decode a byte element sequence into an already owned destination prefix.
///
/// This uses the same advertised element framing as `ConstVec<u8>`, not
/// `Vec<u8>`'s raw-byte specialization. V1 admits fixed or compact per-element
/// length prefixes; reserved layout bits remain rejected. Returns the number of
/// initialized bytes and exact source consumption. Trailing bytes are left for
/// the caller's boundary policy. On error, destination contents are unspecified.
/// Sequence, field, cumulative and nesting accounting remain active; neither
/// element spans nor destination backing are allocated by this function.
///
/// # Errors
/// Returns a fixed framing or resource error for malformed input, exceeded
/// limits, or a declared count larger than the caller's destination.
#[doc(hidden)]
pub fn decode_byte_element_sequence_into(
    bytes: &[u8],
    destination: &mut [u8],
) -> Result<(usize, usize), Error> {
    let (count, _) = read_seq_len_slice(bytes)?;
    let flags = effective_decode_flags().unwrap_or_else(default_encode_flags);
    validate_header_flags(flags)?;
    if count > destination.len() {
        return Err(Error::LengthMismatch);
    }
    let mut index = 0;
    let used = visit_binary_sequence_with_count(
        bytes,
        flags,
        BinarySequenceLayout::from_flags(flags),
        count,
        |span| {
            let element = span.get(bytes)?;
            record_slice_access(element, span.len());
            // A byte has a total, fixed-width scalar decoder. Retain the same
            // field/depth gates without installing the arbitrary-decoder panic
            // hook (whose process-lived Box is outside this caller's custody).
            check_decode_field_length(
                u64::try_from(element.len()).map_err(|_| Error::LengthMismatch)?,
            )?;
            let _depth = DecodeDepthGuard::enter()?;
            let value = read_byte_value(element)?;
            if element.len() != 1 {
                return Err(Error::LengthMismatch);
            }
            destination[index] = value;
            index += 1;
            Ok(())
        },
    )?;
    note_payload_access(bytes, used);
    Ok((count, used))
}

/// The scalar relation shared by u8 slice, archived and element decoding.
pub(super) fn read_byte_value(bytes: &[u8]) -> Result<u8, Error> {
    bytes.first().copied().ok_or(Error::LengthMismatch)
}

pub(super) fn visit_binary_sequence_with_count(
    bytes: &[u8],
    flags: u8,
    layout: BinarySequenceLayout,
    count: usize,
    mut visit: impl FnMut(SequenceSpan) -> Result<(), Error>,
) -> Result<usize, Error> {
    let (declared_count, mut offset) = inspect_seq_len_slice(bytes)?;
    if declared_count != count {
        return Err(Error::LengthMismatch);
    }
    validate_binary_sequence_reservation(bytes, flags, layout, count)?;
    match layout {
        BinarySequenceLayout::LengthPrefixed => {
            for _ in 0..count {
                let tail = bytes.get(offset..).ok_or(Error::LengthMismatch)?;
                let (elem_len, header_len) = read_len_from_slice_with_flags(tail, flags)?;
                let start = offset
                    .checked_add(header_len)
                    .ok_or(Error::LengthMismatch)?;
                let end = start.checked_add(elem_len).ok_or(Error::LengthMismatch)?;
                if end > bytes.len() {
                    return Err(Error::LengthMismatch);
                }
                visit(SequenceSpan { start, end })?;
                offset = end;
            }
            Ok(offset)
        }
        BinarySequenceLayout::FixedOffsets => {
            let entries = count.checked_add(1).ok_or(Error::LengthMismatch)?;
            let offset_table_len = entries.checked_mul(8).ok_or(Error::LengthMismatch)?;
            let offsets_start = offset;
            let offsets_end = offsets_start
                .checked_add(offset_table_len)
                .ok_or(Error::LengthMismatch)?;
            let offsets = bytes
                .get(offsets_start..offsets_end)
                .ok_or(Error::LengthMismatch)?;
            let data_len = read_u64_le_at(offsets, count)?
                .try_into()
                .map_err(|_| Error::LengthMismatch)?;
            let data_start = offsets_end;
            let data_end = data_start
                .checked_add(data_len)
                .ok_or(Error::LengthMismatch)?;
            if data_end > bytes.len() {
                return Err(Error::LengthMismatch);
            }
            let mut prev = 0usize;
            for idx in 0..count {
                let next = read_u64_le_at(offsets, idx + 1)?
                    .try_into()
                    .map_err(|_| Error::LengthMismatch)?;
                if next < prev || next > data_len {
                    return Err(Error::LengthMismatch);
                }
                let start = data_start.checked_add(prev).ok_or(Error::LengthMismatch)?;
                let end = data_start.checked_add(next).ok_or(Error::LengthMismatch)?;
                visit(SequenceSpan { start, end })?;
                prev = next;
            }
            if prev != data_len {
                return Err(Error::LengthMismatch);
            }
            Ok(data_end)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(values: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        write_element_sequence::<u8, _>(
            &mut Encoder::for_buffer(&mut bytes),
            values.iter(),
            u64::MAX,
        )
        .unwrap();
        bytes
    }

    #[test]
    fn caller_storage_matches_generic_sequence_under_every_advertised_layout() {
        for flags in [0, header_flags::COMPACT_LEN] {
            let _flags = DecodeFlagsGuard::enter(flags);
            for values in [&[][..], &[0x11, 0x22, 0x33][..]] {
                let bytes = encode(values);
                let mut output = [0; 3];
                let (len, used) = decode_byte_element_sequence_into(&bytes, &mut output).unwrap();
                let (generic, generic_used) =
                    decode_element_sequence_from_slice_serial::<u8>(&bytes).unwrap();
                assert_eq!(len, values.len());
                assert_eq!(&output[..len], values);
                assert_eq!(generic, values);
                assert_eq!(used, generic_used);
                assert_eq!(used, bytes.len());
                let mut trailing = bytes.clone();
                trailing.extend_from_slice(&[0xff, 0x00]);
                assert_eq!(
                    decode_byte_element_sequence_into(&trailing, &mut output).unwrap(),
                    (len, used)
                );
                if !values.is_empty() {
                    assert!(matches!(
                        decode_byte_element_sequence_into(&bytes, &mut output[..len - 1]),
                        Err(Error::LengthMismatch)
                    ));
                }
            }
        }
    }

    #[test]
    fn first_offset_is_rejected_before_any_plan_allocation() {
        let _flags = DecodeFlagsGuard::enter(0);
        // Exercise the retained internal planner directly. PACKED_SEQ remains
        // a reserved wire-header bit and is never enabled by these fixtures.
        let mut bytes = 1_u64.to_le_bytes().to_vec();
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.push(0x51);
        // Count admission keeps its existing one-byte charge. No credit remains
        // for a SequenceSpan allocation; the framing reason must win first.
        let limits = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 1, usize::MAX);
        let (result, usage) = with_decode_limits_measured(limits, || {
            plan_binary_sequence(&bytes, 0, BinarySequenceLayout::FixedOffsets)
        });
        assert!(matches!(result, Err(Error::LengthMismatch)));
        assert_eq!(usage.total_allocated_bytes(), 1);
        let mut empty = 0_u64.to_le_bytes().to_vec();
        empty.extend_from_slice(&1_u64.to_le_bytes());
        let zero = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
        let (result, usage) = with_decode_limits_measured(zero, || {
            plan_binary_sequence(&empty, 0, BinarySequenceLayout::FixedOffsets)
        });
        assert!(matches!(result, Err(Error::LengthMismatch)));
        assert_eq!(usage.total_allocated_bytes(), 0);
    }

    #[test]
    fn malformed_lengths_offsets_and_limits_share_the_canonical_relation() {
        for flags in [0, header_flags::COMPACT_LEN] {
            let _flags = DecodeFlagsGuard::enter(flags);
            let valid = encode(&[0x41, 0x42]);
            let mut output = [0; 2];
            let mut bad = valid.clone();
            bad[8] = 3;
            assert!(decode_byte_element_sequence_into(&bad, &mut output).is_err());
            assert!(decode_element_sequence_from_slice_serial::<u8>(&bad).is_err());
            for len in 0..valid.len() {
                assert!(decode_byte_element_sequence_into(&valid[..len], &mut output).is_err());
                assert!(decode_element_sequence_from_slice_serial::<u8>(&valid[..len]).is_err());
            }
            for limits in [
                DecodeLimits::new(1, usize::MAX, usize::MAX, usize::MAX, usize::MAX),
                DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, usize::MAX),
                DecodeLimits::new(usize::MAX, usize::MAX, 1, usize::MAX, usize::MAX),
                DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 1, usize::MAX),
                DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 0),
            ] {
                assert!(
                    with_decode_limits(limits, || decode_byte_element_sequence_into(
                        &valid,
                        &mut output
                    ))
                    .unwrap_err()
                    .is_decode_resource_limit()
                );
            }
        }
    }

    #[test]
    fn fixed_value_error_preserves_display_without_becoming_a_resource_failure() {
        let error = Error::InvalidValue {
            context: "public key",
        };
        assert_eq!(error.to_string(), "invalid public key");
        assert!(!error.is_decode_resource_limit());
    }
}
