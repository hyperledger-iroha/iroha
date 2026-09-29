//! Complete ten-seat result codecs and inherited caller allocation-limit controls.
//!
//! These are valid native context graphs, not certified execution or finality owners.

use super::*;
use crate::{
    sumeragi::epoch::{
        ValidatorEpochBoundaryV1, ValidatorEpochContextV1,
        tests::{fixture, retained},
    },
    sumeragi_finality::{ScheduledConfig, ScheduledSlot},
};
use iroha_crypto::HashOf;
use iroha_sumeragi::types::ChainParams;

fn ten_seat_boundary() -> ExecutionResultCommitment {
    let current = fixture(10);
    let next = retained(&current);
    let height = current.authorization.last_height;
    let params = ChainParamsRecord::from_core(&ChainParams::default());
    let slot = |height| {
        ScheduledSlot::Ready(ScheduledConfig {
            height,
            epoch: next.clone(),
            params,
        })
    };
    let (native_lanes, root) = NativeLaneStateProof::empty_for_testing(current.network_id, height);
    ExecutionResultCommitment::new(
        height,
        ExecutionCommitment {
            parent_state_root: Hash::new(b"ten-seat result prestate"),
            post_state_root: root,
            ordinary_writes_root: root,
            kagemusha_top_up_root: None,
            kagemusha_top_up_count: 0,
            executed_block_wire_len: 1,
            executed_block_wire_hash: Hash::new(b"synthetic codec result, not executed World"),
            transaction_input_commitment: None,
            transaction_output_commitment: None,
        },
        ScheduleOutcome {
            height,
            boundary: Some(ValidatorEpochBoundaryV1 {
                version: 1,
                height,
                predecessor_context_id: current.context_id().unwrap(),
                selection_anchor: HashOf::from_untyped_unchecked(Hash::new(
                    b"synthetic codec boundary parent",
                )),
                next: next.clone(),
                preparation: None,
            }),
            current,
            next: slot(height + 1),
            after_next: slot(height + 2),
        },
        None,
        native_lanes,
    )
    .unwrap()
}

fn protocol_limits(allocation: usize) -> norito::DecodeLimits {
    norito::DecodeLimits::new(
        96,
        MAX_RESULT_PREIMAGE_BYTES,
        norito::canonical_decode_limits(MAX_RESULT_PREIMAGE_BYTES).max_total_elements(),
        allocation,
        32,
    )
}

fn assert_ten_seats(epoch: &ValidatorEpochContextV1) {
    assert_eq!(epoch.committee.len(), 10);
    assert_eq!(epoch.authority.validators.len(), 10);
    epoch.validate().unwrap();
}

#[test]
fn ten_seat_boundary_result_roundtrips_complete_repeated_epoch_graph() {
    let value = ten_seat_boundary();
    let bytes = value.preimage().unwrap();
    assert!(bytes.len() <= MAX_RESULT_PREIMAGE_BYTES);
    assert_ten_seats(&value.schedule.current);
    assert_ten_seats(&value.schedule.boundary.as_ref().unwrap().next);
    for slot in [&value.schedule.next, &value.schedule.after_next] {
        let ScheduledSlot::Ready(config) = slot else {
            panic!("boundary must install each successor");
        };
        assert_ten_seats(&config.epoch);
    }
    // This geometry reproduces the fixed four-wire-length cap's actual codec refusal.
    // It does not infer allocation demand from the wire length alone.
    let old_cap = 4 * MAX_RESULT_PREIMAGE_BYTES;
    let error = norito::decode_canonical_with_limits::<ExecutionResultCommitment>(
        &bytes,
        protocol_limits(old_cap),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        norito::Error::TotalAllocationExceeded { attempted, limit }
            if attempted > limit && limit == old_cap as u64
    ));
    let decoded = ExecutionResultCommitment::decode(&bytes).unwrap();
    assert_eq!(decoded, value);
    assert_eq!(decoded.preimage().unwrap(), bytes);
    assert_eq!(decoded.result().unwrap(), value.result().unwrap());
    let mut invalid = value;
    invalid.schedule.boundary = None;
    assert!(matches!(
        ExecutionResultCommitment::decode(&norito::encode_canonical(&invalid).unwrap()),
        Err(CommitmentError::Schedule(_))
    ));
}

#[test]
fn strict_outer_allocation_limit_refuses_same_result_and_scope_exit_allows_retry() {
    let value = ten_seat_boundary();
    let bytes = value.preimage().unwrap();
    let original = bytes.clone();
    let inner =
        protocol_limits(norito::canonical_decode_limits(bytes.len()).max_total_allocated_bytes());
    let error = norito::with_decode_limits(protocol_limits(0), || {
        norito::decode_canonical_with_limits::<ExecutionResultCommitment>(&bytes, inner)
    })
    .unwrap_err();
    assert!(error.is_decode_resource_limit());
    assert!(matches!(
        error,
        norito::Error::TotalAllocationExceeded { attempted, limit: 0 } if attempted > 0
    ));
    let wrapped = norito::with_decode_limits_scope(protocol_limits(0), || {
        ExecutionResultCommitment::decode(&bytes)
    });
    // TODO: The production result/finality wrappers currently erase codec resource types.
    // Record their refusal without claiming typed operational propagation through them.
    assert_eq!(wrapped, Err(CommitmentError::Encoding(error.to_string())));
    assert_eq!(bytes, original);
    assert_eq!(ExecutionResultCommitment::decode(&bytes).unwrap(), value);
}

#[test]
fn original_caller_decode_counter_is_not_replaced_after_nested_refusal() {
    let value = ten_seat_boundary();
    let bytes = value.preimage().unwrap();
    let cap = norito::canonical_decode_limits(bytes.len()).max_total_allocated_bytes();
    norito::with_decode_limits_scope(protocol_limits(cap), || {
        // Occupy the existing caller's counter before entering the result decoder. This is
        // counter-policy evidence only; it is not an original memory-pool reservation.
        norito::core::reserve_decode_allocation(cap).unwrap();
        let inner = protocol_limits(cap);
        let error =
            norito::decode_canonical_with_limits::<ExecutionResultCommitment>(&bytes, inner)
                .unwrap_err();
        assert!(matches!(
            error,
            norito::Error::TotalAllocationExceeded { attempted, limit }
                if attempted > limit && limit == cap as u64
        ));
        assert_eq!(
            ExecutionResultCommitment::decode(&bytes),
            Err(CommitmentError::Encoding(error.to_string()))
        );
        // Failed nested scopes must neither reset nor refund the caller's consumed counter.
        assert!(matches!(
            norito::core::reserve_decode_allocation(1),
            Err(norito::Error::TotalAllocationExceeded { attempted, limit })
                if attempted == cap as u64 + 1 && limit == cap as u64
        ));
    });
    assert_eq!(ExecutionResultCommitment::decode(&bytes).unwrap(), value);
}

#[test]
fn strict_outer_element_limit_refuses_without_replacing_original_counter() {
    let value = ten_seat_boundary();
    let bytes = value.preimage().unwrap();
    let canonical = norito::canonical_decode_limits(bytes.len());
    let outer = norito::DecodeLimits::new(
        96,
        MAX_RESULT_PREIMAGE_BYTES,
        0,
        canonical.max_total_allocated_bytes(),
        32,
    );
    let refusal =
        norito::with_decode_limits_scope(outer, || ExecutionResultCommitment::decode(&bytes));
    assert!(matches!(refusal, Err(CommitmentError::Encoding(_))));
    assert_eq!(ExecutionResultCommitment::decode(&bytes).unwrap(), value);
}
