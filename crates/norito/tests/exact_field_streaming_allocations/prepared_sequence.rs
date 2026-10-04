//! Prepared sequence scratch, exact canonical order and original refusal parity.

use iroha_allocation::{AllocationBudget, ChargedBuffer};
use norito::core::{
    DecodeFlagsGuard, DecodeFromSlice, DecodeIntoError, DecodeLimits, PreparedDecodeWorkspace,
    SequenceDestinationError, SequenceSpan, decode_element_sequence_from_slice_serial,
    decode_raw_byte_sequence_into, header_flags, prepare_element_sequence, with_decode_limits,
};
use std::convert::Infallible;

fn initialized<T: Copy>(pool: &AllocationBudget, count: usize, value: T) -> ChargedBuffer<T> {
    let layout = std::alloc::Layout::array::<T>(count).unwrap();
    let mut reservation = pool.try_reserve_layouts([layout]).unwrap();
    let mut buffer = ChargedBuffer::from_reservation(count, &mut reservation).unwrap();
    for _ in 0..count {
        buffer.push_reserved(value);
    }
    buffer
}
fn scratch(pool: &AllocationBudget, count: usize) -> ChargedBuffer<SequenceSpan> {
    initialized(
        pool,
        count,
        SequenceSpan {
            start: usize::MAX,
            end: usize::MAX,
        },
    )
}
fn codec(error: SequenceDestinationError) -> norito::Error {
    match error {
        SequenceDestinationError::Codec(error) => error,
        other => panic!("unexpected local geometry: {other}"),
    }
}
fn limits(bytes: usize) -> DecodeLimits {
    DecodeLimits::new(4096, 1 << 20, 1 << 20, bytes, 32)
}
fn workspace(pool: &AllocationBudget) -> PreparedDecodeWorkspace {
    let mut reservation = pool
        .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
        .unwrap();
    PreparedDecodeWorkspace::from_reservation(pool, &mut reservation).unwrap()
}

#[test]
fn prepared_spans_keep_every_advertised_layout_and_unused_slots_out_of_the_plan() {
    let pool = AllocationBudget::new(1 << 20);
    let mut spans = scratch(&pool, 962);
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        for count in [0, 4, 31, 961] {
            let values: Vec<u64> = (0..count as u64).map(|i| i * 7 + 3).collect();
            let bytes = super::bare_bytes(&values, flags);
            let (ordinary, used) =
                decode_element_sequence_from_slice_serial::<u64>(&bytes).unwrap();
            let plan = prepare_element_sequence(&bytes, spans.as_mut_slice()).unwrap();
            assert_eq!(plan.len(), count);
            assert_eq!(plan.is_empty(), count == 0);
            assert_eq!(plan.used(), used);
            let mut visited = 0;
            plan.decode_elements::<u64, Infallible>(|index, field| {
                let value = field.with_payload(|bytes| {
                    let (value, used) = u64::decode_from_slice(bytes)?;
                    if used != bytes.len() {
                        return Err(norito::Error::LengthMismatch.into());
                    }
                    Ok(value)
                })?;
                assert_eq!(value, ordinary[index]);
                visited += 1;
                Ok(())
            })
            .unwrap();
            assert_eq!(visited, count);
            assert_eq!(
                spans.as_slice()[961],
                SequenceSpan {
                    start: usize::MAX,
                    end: usize::MAX
                },
                "unused initialized slot must not be counted or overwritten"
            );
        }
    }
}

#[test]
fn prepared_plan_keeps_late_framing_failure_before_any_element_or_local_output() {
    let pool = AllocationBudget::new(4096);
    let mut spans = scratch(&pool, 4);
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let bytes = super::bare_bytes(&vec![false, true, false, true], flags);
        for end in 0..bytes.len() {
            let ordinary =
                decode_element_sequence_from_slice_serial::<bool>(&bytes[..end]).unwrap_err();
            let prepared = match prepare_element_sequence(&bytes[..end], spans.as_mut_slice()) {
                Err(error) => codec(error),
                Ok(_) => panic!("truncated plan must not produce any element visitor"),
            };
            assert_eq!(
                ordinary.to_string(),
                prepared.to_string(),
                "prefix {end}, flags {flags}"
            );
        }
        // Wrong first bool and malformed last framing must still report framing first.
        let first_prefix = if flags == 0 { 8 } else { 1 };
        let mut late = bytes.clone();
        late[8 + first_prefix] = 2;
        late.pop();
        assert!(matches!(
            decode_element_sequence_from_slice_serial::<bool>(&late),
            Err(norito::Error::LengthMismatch)
        ));
        assert!(matches!(
            prepare_element_sequence(&late, spans.as_mut_slice()),
            Err(SequenceDestinationError::Codec(
                norito::Error::LengthMismatch
            ))
        ));
        let too_small = prepare_element_sequence(&bytes, &mut spans.as_mut_slice()[..3])
            .err()
            .unwrap();
        assert!(matches!(
            too_small,
            SequenceDestinationError::Storage {
                available: 3,
                required: 4
            }
        ));
        let mut trailing = bytes.clone();
        trailing.extend_from_slice(&[0xff, 0xee]);
        let plan = prepare_element_sequence(&trailing, spans.as_mut_slice()).unwrap();
        assert_eq!(plan.used(), bytes.len());
    }
}

#[test]
fn prepared_sequence_preserves_original_count_span_and_output_limit_order() {
    let pool = AllocationBudget::new(4096);
    let mut spans = scratch(&pool, 4);
    let _flags = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
    let bytes = super::bare_bytes(&vec![3_u64, 5, 7, 11], header_flags::COMPACT_LEN);
    let span_bytes = 4 * std::mem::size_of::<SequenceSpan>();
    let value_bytes = 4 * std::mem::size_of::<u64>();
    for maximum in [3, 4 + span_bytes - 1, 4 + span_bytes + value_bytes - 1] {
        let ordinary = with_decode_limits(limits(maximum), || {
            decode_element_sequence_from_slice_serial::<u64>(&bytes)
        })
        .unwrap_err();
        let prepared = with_decode_limits(limits(maximum), || {
            let plan = prepare_element_sequence(&bytes, spans.as_mut_slice()).map_err(codec)?;
            plan.decode_elements::<u64, Infallible>(|_, _| {
                panic!("original logical refusal precedes every value")
            })
            .map_err(DecodeIntoError::into_codec)
        })
        .unwrap_err();
        assert_eq!(ordinary.to_string(), prepared.to_string());
        assert!(ordinary.is_decode_resource_limit());
    }
}

#[test]
fn nested_four_and_thirty_one_squared_rows_reuse_actual_original_scratch_without_allocations() {
    let pool = AllocationBudget::new(1 << 20);
    let mut work = workspace(&pool);
    let mut outer = scratch(&pool, 31);
    let mut inner = scratch(&pool, 31);
    let mut values = initialized(&pool, 961, 0_u64);
    let outer_pointer = outer.as_slice().as_ptr();
    let inner_pointer = inner.as_slice().as_ptr();
    let values_pointer = values.as_slice().as_ptr();
    for n in [4, 31] {
        let expected: Vec<Vec<u64>> = (0..n)
            .map(|row| (0..n).map(|column| (row * 31 + column) as u64).collect())
            .collect();
        let flags = header_flags::COMPACT_LEN;
        let _flags = DecodeFlagsGuard::enter(flags);
        let bytes = super::bare_bytes(&expected, flags);
        let held = pool
            .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
            .unwrap();
        let original = pool.try_reserve_bytes(1).unwrap_err();
        let mut visits = 0;
        let allocation_count = super::allocations_during(|| {
            work.with_limits(limits(1 << 20), limits(1 << 20), || {
                let plan = prepare_element_sequence(&bytes, outer.as_mut_slice()).unwrap();
                assert_eq!(plan.len(), n);
                assert_eq!(plan.used(), bytes.len());
                plan.decode_elements::<Vec<u64>, Infallible>(|row, field| {
                    field.with_payload(|bytes| {
                        let plan = prepare_element_sequence(bytes, inner.as_mut_slice())
                            .map_err(|error| DecodeIntoError::Codec(codec(error)))?;
                        assert_eq!(plan.len(), n);
                        assert_eq!(plan.used(), bytes.len());
                        plan.decode_elements::<u64, Infallible>(|column, field| {
                            field.with_payload(|bytes| {
                                let (value, used) = u64::decode_from_slice(bytes)?;
                                if used != bytes.len() {
                                    return Err(norito::Error::LengthMismatch.into());
                                }
                                values.as_mut_slice()[row * n + column] = value;
                                visits += 1;
                                Ok(())
                            })
                        })
                    })
                })
                .unwrap();
            })
            .unwrap();
        });
        assert_eq!(allocation_count, 0);
        assert_eq!(visits, n * n);
        for (actual, expected) in values.as_slice()[..n * n]
            .iter()
            .zip(expected.iter().flatten())
        {
            assert_eq!(actual, expected);
        }
        assert_eq!(outer.as_slice().as_ptr(), outer_pointer);
        assert_eq!(inner.as_slice().as_ptr(), inner_pointer);
        assert_eq!(values.as_slice().as_ptr(), values_pointer);
        assert!(matches!(
            original,
            iroha_allocation::AllocationRefusal::Capacity { .. }
        ));
        drop(held);
    }
}

#[test]
fn raw_bytes_keep_owning_layout_logical_limits_and_original_backing() {
    let pool = AllocationBudget::new(4096);
    let mut output = initialized(&pool, 31, 0_u8);
    let pointer = output.as_slice().as_ptr();
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let values: Vec<u8> = (1..32).collect();
        let bytes = super::bare_bytes(&values, flags);
        let count = super::allocations_during(|| {
            let (length, used) =
                decode_raw_byte_sequence_into(&bytes, output.as_mut_slice()).unwrap();
            assert_eq!((length, used), (31, bytes.len()));
        });
        assert_eq!(count, 0);
        assert_eq!(output.as_slice(), values);
        assert_eq!(output.as_slice().as_ptr(), pointer);
        let ordinary =
            with_decode_limits(limits(61), || Vec::<u8>::decode_from_slice(&bytes)).unwrap_err();
        let prepared = with_decode_limits(limits(61), || {
            decode_raw_byte_sequence_into(&bytes, output.as_mut_slice()).map_err(codec)
        })
        .unwrap_err();
        assert_eq!(ordinary.to_string(), prepared.to_string());
        assert!(matches!(
            decode_raw_byte_sequence_into(&bytes, &mut output.as_mut_slice()[..30]),
            Err(SequenceDestinationError::Storage {
                required: 31,
                available: 30
            })
        ));
    }
}
