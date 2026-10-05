//! Actual inline pulse decode allocations and original prepared scope readers.
use super::*;
use iroha_allocation::{AllocationBudget, ChargedBuffer};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::BlockHeader,
    consensus::{
        FinalizedGlobalThresholdBeaconPulseV1, GlobalThresholdBeaconChainAnchorV1,
        GlobalThresholdBeaconPulseContextV1,
    },
};
use norito::core::{
    DecodeAttemptErrorKind, DecodeLimits, DecodeResourceError, PreparedDecodeWorkspace,
    SequenceSpan, classify_decode_attempt,
};

fn fixture() -> FinalizedGlobalThresholdBeaconPulseV1 {
    // Complete transport fields only. This decoder must not relabel these bytes
    // as an authenticated threshold pulse, native epoch or finalized ancestry.
    FinalizedGlobalThresholdBeaconPulseV1 {
        version: 1,
        network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"physical pulse genesis",
        ))),
        session_id: [2; 32],
        roster_hash: [3; 32],
        transcript_hash: [4; 32],
        context: GlobalThresholdBeaconPulseContextV1 {
            instance: [5; 32],
            epoch: 7,
            epoch_context_id: [6; 32],
            parent_consensus_hash: [7; 32],
            parent_result: [8; 32],
        },
        height: 9,
        round: 0,
        finalized_chain_anchor: GlobalThresholdBeaconChainAnchorV1 {
            height: 8,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"physical pulse parent",
            )),
        },
        signature: [11; 48],
        seed: [12; 32],
        pulse_id: [13; 32],
    }
}
fn source(
    value: &FinalizedGlobalThresholdBeaconPulseV1,
    pool: &AllocationBudget,
    prefix: usize,
    flags: u8,
) -> (ChargedBuffer<u8>, SequenceSpan) {
    let bytes = bare_bytes(value, flags);
    let mut input = ChargedBuffer::new(prefix + bytes.len(), pool).unwrap();
    input.append(&vec![0xa5; prefix]).unwrap();
    input.append(&bytes).unwrap();
    (
        input,
        SequenceSpan {
            start: prefix,
            end: prefix + bytes.len(),
        },
    )
}
fn prepaid_workspace(pool: &AllocationBudget) -> PreparedDecodeWorkspace {
    let mut reservation = pool
        .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
        .unwrap();
    let workspace = PreparedDecodeWorkspace::from_reservation(pool, &mut reservation).unwrap();
    assert_eq!(reservation.remaining_bytes(), 0);
    assert!(workspace.belongs_to(pool));
    workspace
}

fn measured<T>(run: impl FnOnce() -> T) -> (T, usize) {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            TRACKING.with(|tracking| tracking.set(false));
        }
    }
    TRACKING.with(|tracking| assert!(!tracking.replace(true)));
    ALLOCATIONS.with(|count| count.set(0));
    let guard = Restore;
    let value = run();
    let count = ALLOCATIONS.with(Cell::get);
    drop(guard);
    (value, count)
}

#[test]
fn pulse_all_inline_children_decode_with_zero_actual_allocations_at_every_source_alignment() {
    let original = fixture();
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        for prefix in 0..16 {
            let pool = AllocationBudget::new(1 << 20);
            let (input, span) = source(&original, &pool, prefix, flags);
            let source_floor = pool.reserved_bytes();
            let pointer = input.as_slice().as_ptr();
            let hash = Hash::new(input.as_slice());
            let mut workspace = prepaid_workspace(&pool);
            let controls = PreparedDecodeWorkspace::allocation_layouts()
                .iter()
                .map(Layout::size)
                .sum::<usize>();
            assert_eq!(pool.reserved_bytes(), source_floor + controls);
            let limits = DecodeLimits::new(0, 1 << 20, 0, 0, 64);
            let bytes = span.get(input.as_slice()).unwrap();
            let warm = workspace
                .with_limits(limits, limits, || {
                    FinalizedGlobalThresholdBeaconPulseV1::decode_inline_payload(bytes)
                })
                .unwrap()
                .unwrap();
            assert_eq!(warm, original);
            // Every real source/control owner is already present. A zero pool
            // limit and zero logical allocation allowance admit no fresh work.
            pool.set_limit_bytes(0);
            let (decoded, allocations) = measured(|| {
                workspace.with_limits(limits, limits, || {
                    FinalizedGlobalThresholdBeaconPulseV1::decode_inline_payload(bytes)
                })
            });
            assert_eq!(
                allocations,
                0,
                "flags {flags:#x}, original alignment {}",
                bytes.as_ptr().addr() % 8
            );
            assert_eq!(decoded.unwrap().unwrap(), original);
            assert_eq!(pool.reserved_bytes(), source_floor + controls);
            assert_eq!(input.as_slice().as_ptr(), pointer);
            assert_eq!(Hash::new(input.as_slice()), hash);
            assert_eq!(bare_bytes(&original, flags), bytes);
            drop(workspace);
            assert_eq!(pool.reserved_bytes(), source_floor);
            drop(input);
            assert_eq!(pool.reserved_bytes(), 0);
        }
    }
}

#[test]
fn pulse_original_enclosing_field_cause_keeps_prepaid_counter_reader_through_retry_and_last_refund()
{
    let flags = header_flags::COMPACT_LEN;
    let _flags = DecodeFlagsGuard::enter(flags);
    let original = fixture();
    let pool = AllocationBudget::new(1 << 20);
    let (input, span) = source(&original, &pool, 9, flags);
    let source_floor = pool.reserved_bytes();
    let pointer = input.as_slice().as_ptr();
    let hash = Hash::new(input.as_slice());
    let mut caller = prepaid_workspace(&pool);
    let mut decoder = prepaid_workspace(&pool);
    let controls = PreparedDecodeWorkspace::allocation_layouts();
    assert_eq!(
        pool.reserved_bytes(),
        source_floor + 2 * controls.iter().map(Layout::size).sum::<usize>()
    );
    let protocol = DecodeLimits::new(0, 1 << 20, 0, 0, 64);
    let narrow = DecodeLimits::new(0, 1, 0, 0, 64);
    let bytes = span.get(input.as_slice()).unwrap();
    let (failed, allocations) = measured(|| {
        caller.with_limits(narrow, narrow, || {
            classify_decode_attempt(|| {
                decoder
                    .with_limits(protocol, protocol, || {
                        FinalizedGlobalThresholdBeaconPulseV1::decode_inline_payload(bytes)
                    })
                    .unwrap()
            })
        })
    });
    assert_eq!(allocations, 0);
    let cause = failed.unwrap().unwrap_err();
    assert_eq!(cause.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    assert_eq!(
        std::error::Error::source(&cause)
            .unwrap()
            .downcast_ref::<norito::Error>()
            .unwrap()
            .decode_resource_error(),
        Some(DecodeResourceError::FieldLengthExceeded {
            length: std::mem::size_of::<u16>() as u64,
            limit: 1
        })
    );
    pool.set_limit_bytes(0);
    let (retry, allocations) = measured(|| {
        decoder.with_limits(protocol, protocol, || {
            FinalizedGlobalThresholdBeaconPulseV1::decode_inline_payload(bytes)
        })
    });
    assert_eq!(allocations, 0);
    assert_eq!(retry.unwrap().unwrap(), original);
    assert_eq!(cause.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    assert_eq!(input.as_slice().as_ptr(), pointer);
    assert_eq!(Hash::new(input.as_slice()), hash);
    drop(decoder);
    drop(caller);
    // The observer's original family keeps exactly the first prepared decoder
    // counter control alive; no source or completed field is copied or refunded.
    assert_eq!(pool.reserved_bytes(), source_floor + controls[0].size());
    assert_eq!(
        cause.into_error().decode_resource_error(),
        Some(DecodeResourceError::FieldLengthExceeded {
            length: std::mem::size_of::<u16>() as u64,
            limit: 1
        })
    );
    assert_eq!(pool.reserved_bytes(), source_floor);
    drop(input);
    assert_eq!(pool.reserved_bytes(), 0);
}

struct LocatePulseLeaves {
    base: usize,
    version: Option<std::ops::Range<usize>>,
    network: Option<std::ops::Range<usize>>,
}
impl norito::core::FieldDestination for LocatePulseLeaves {
    type Error = std::convert::Infallible;
}
impl<const INDEX: usize, T> norito::core::DecodeField<INDEX, T> for LocatePulseLeaves {
    type Value = ();
    fn decode_field(
        &mut self,
        field: norito::core::CanonicalField<'_, T>,
    ) -> Result<(), norito::core::DecodeIntoError<Self::Error>> {
        let start = field
            .bytes()
            .as_ptr()
            .addr()
            .checked_sub(self.base)
            .unwrap();
        let span = start..start.checked_add(field.bytes().len()).unwrap();
        match INDEX {
            0 => self.version = Some(span),
            1 => self.network = Some(span),
            _ => {}
        }
        Ok(())
    }
}

#[test]
fn original_pulse_scalar_alignment_refusal_and_inline_hash_invalidity_keep_their_actual_causes_and_original_counter()
 {
    use norito::core::DecodeRecordFields;
    let flags = header_flags::COMPACT_LEN;
    let _flags = DecodeFlagsGuard::enter(flags);
    let original = fixture();
    let wire = bare_bytes(&original, flags);
    let alignment = norito::core::archived_payload_align::<FinalizedGlobalThresholdBeaconPulseV1>();
    assert!(alignment > std::mem::align_of::<u16>());
    let pool = AllocationBudget::new(1 << 20);
    let mut input =
        ChargedBuffer::<u8>::new(wire.len().checked_add(alignment - 1).unwrap(), &pool).unwrap();
    let prefix = (alignment - input.as_slice().as_ptr().addr() % alignment) % alignment;
    input.append(&vec![0xa5; prefix]).unwrap();
    input.append(&wire).unwrap();
    let span = SequenceSpan {
        start: prefix,
        end: prefix + wire.len(),
    };
    let (version, network) = {
        let bytes = span.get(input.as_slice()).unwrap();
        assert_eq!(bytes.as_ptr().addr() % alignment, 0);
        let mut fields = LocatePulseLeaves {
            base: bytes.as_ptr().addr(),
            version: None,
            network: None,
        };
        let (_, used) =
            FinalizedGlobalThresholdBeaconPulseV1::decode_fields(bytes, &mut fields).unwrap();
        assert_eq!(used, bytes.len());
        (fields.version.unwrap(), fields.network.unwrap())
    };
    let scalar_bytes = version.len();
    assert_eq!(scalar_bytes, std::mem::size_of::<u16>());
    assert_ne!(
        (input.as_slice().as_ptr().addr() + span.start + version.start)
            % std::mem::align_of::<u16>(),
        0
    );
    let floor = pool.reserved_bytes();
    let mut caller = prepaid_workspace(&pool);
    let mut decoder = prepaid_workspace(&pool);
    assert_eq!(
        pool.reserved_bytes(),
        floor
            + 2 * PreparedDecodeWorkspace::allocation_layouts()
                .iter()
                .map(Layout::size)
                .sum::<usize>()
    );
    let zero = DecodeLimits::new(0, wire.len(), 0, 0, 64);
    assert_eq!(
        decoder
            .with_limits(zero, zero, || {
                FinalizedGlobalThresholdBeaconPulseV1::decode_inline_payload(
                    span.get(input.as_slice()).unwrap(),
                )
            })
            .unwrap()
            .unwrap(),
        original
    );
    assert_eq!(
        &input.as_slice()[span.start + network.start..span.start + network.end],
        original.network_id.as_bytes()
    );
    input.as_mut_slice()[span.start + network.end - 1] &= !1;

    let pointer = input.as_slice().as_ptr();
    let hash = Hash::new(input.as_slice());
    let bytes = span.get(input.as_slice()).unwrap();
    let protocol = norito::canonical_decode_limits(bytes.len());
    pool.set_limit_bytes(0);
    let (owning, allocations) = measured(|| {
        caller.with_limits(zero, zero, || {
            classify_decode_attempt(|| {
                decoder.with_limits(protocol, protocol, || {
            norito::core::decode_field_canonical::<FinalizedGlobalThresholdBeaconPulseV1>(bytes)
        }).unwrap()
            })
        })
    });
    assert_eq!(
        allocations, 0,
        "logical copy refusal must precede the actual heap allocation"
    );
    let owning = owning.unwrap().unwrap_err();
    assert_eq!(owning.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    assert_eq!(
        std::error::Error::source(&owning)
            .unwrap()
            .downcast_ref::<norito::Error>()
            .unwrap()
            .decode_resource_error(),
        Some(DecodeResourceError::TotalAllocationExceeded {
            attempted: scalar_bytes as u64,
            limit: 0
        })
    );
    let (inline, allocations) = measured(|| {
        classify_decode_attempt(|| {
            decoder
                .with_limits(zero, zero, || {
                    FinalizedGlobalThresholdBeaconPulseV1::decode_inline_payload(bytes)
                })
                .unwrap()
        })
    });
    assert_eq!(allocations, 0);
    let inline = inline.unwrap_err();
    assert_eq!(inline.kind(), DecodeAttemptErrorKind::Invalid);
    assert!(matches!(
        inline.into_error(),
        norito::Error::InvalidValue {
            context: "hash lsb"
        }
    ));
    assert_eq!(owning.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    assert_eq!(input.as_slice().as_ptr(), pointer);
    assert_eq!(Hash::new(input.as_slice()), hash);
    drop(decoder);
    drop(caller);
    assert_eq!(
        pool.reserved_bytes(),
        floor + PreparedDecodeWorkspace::allocation_layouts()[0].size()
    );
    assert_eq!(
        owning.into_error().decode_resource_error(),
        Some(DecodeResourceError::TotalAllocationExceeded {
            attempted: scalar_bytes as u64,
            limit: 0
        })
    );
    assert_eq!(pool.reserved_bytes(), floor);
    drop(input);
    assert_eq!(pool.reserved_bytes(), 0);
}
