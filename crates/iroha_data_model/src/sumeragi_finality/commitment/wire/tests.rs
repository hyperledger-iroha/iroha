//! Compact result graph roundtrips, contradiction rejection, schema and reconstruction budgets.

use super::*;
use crate::sumeragi_finality::commitment::{
    CommitmentError, MAX_RESULT_PREIMAGE_BYTES, decode_budget_tests::boundary_result,
};
use iroha_schema::IntoSchema;

#[test]
fn compact_result_roundtrips_complete_boundary_without_repeated_epoch_wire() {
    let value = boundary_result(10);
    let bytes = value.preimage().unwrap();
    assert!(bytes.len() <= MAX_RESULT_PREIMAGE_BYTES);
    assert_eq!(ExecutionResultCommitment::decode(&bytes).unwrap(), value);

    // There is one first-release result layout; the former repeated graph is not accepted.
    #[derive(norito::NoritoSerialize, norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::sumeragi_finality::ExecutionResultCommitment")]
    struct RepeatedResult {
        height: u64,
        execution: ExecutionCommitment,
        schedule: ScheduleOutcome,
        beacon: Option<FinalizedGlobalThresholdBeaconPulseV1>,
        native_lanes: NativeLaneStateProof,
    }
    let repeated = norito::encode_canonical(&RepeatedResult {
        height: value.height,
        execution: value.execution,
        schedule: value.schedule.clone(),
        beacon: value.beacon.clone(),
        native_lanes: value.native_lanes.clone(),
    })
    .unwrap();
    assert!(repeated.len() > bytes.len() + 4 * 1024);
    assert!(ExecutionResultCommitment::decode(&repeated).is_err());
}

#[test]
fn compact_result_rejects_either_contradictory_successor_before_encoding() {
    for successor in [0, 1] {
        let mut value = boundary_result(4);
        let slot = if successor == 0 {
            &mut value.schedule.next
        } else {
            &mut value.schedule.after_next
        };
        let ScheduledSlot::Ready(config) = slot else {
            unreachable!()
        };
        config.epoch.leader_seed[0] ^= 1;
        assert!(matches!(
            norito::canonical_frame_len(&value),
            Err(norito::Error::NonCanonicalEncoding)
        ));
        assert!(matches!(
            norito::encode_canonical(&value),
            Err(norito::Error::NonCanonicalEncoding)
        ));
    }
}

#[test]
fn compact_result_preserves_pending_barriers_and_rejects_bad_heights() {
    let mut value = boundary_result(4);
    value.height -= 2;
    value.schedule.height = value.height;
    value.schedule.boundary = None;
    let params = *value.schedule.next.params();
    value.schedule.next = ScheduledSlot::Ready(ScheduledConfig {
        height: value.height + 1,
        epoch: value.schedule.current.clone(),
        params,
    });
    value.schedule.after_next = ScheduledSlot::Ready(ScheduledConfig {
        height: value.height + 2,
        epoch: value.schedule.current.clone(),
        params,
    });
    let (proof, root) =
        NativeLaneStateProof::empty_for_testing(value.schedule.current.network_id, value.height);
    value.native_lanes = proof;
    value.execution.post_state_root = root;
    value.execution.ordinary_writes_root = root;
    value.validate().unwrap();
    assert_eq!(
        ExecutionResultCommitment::decode(&value.preimage().unwrap()).unwrap(),
        value
    );

    let pending = SlotWire::PendingBoundary {
        height: value.height + 3,
        boundary_height: value.schedule.current.authorization.last_height,
        predecessor_context_id: value.schedule.current.context_id().unwrap(),
        params,
    };
    // Pending expansion owns no epoch and consumes no allocation credit.
    let expanded = norito::with_decode_limits(
        norito::DecodeLimits::new(96, MAX_RESULT_PREIMAGE_BYTES, 0, 0, 32),
        || pending.expand(&value.schedule.current),
    )
    .unwrap();
    assert!(
        matches!(expanded, ScheduledSlot::PendingBoundary { height, .. } if height == value.height + 3)
    );
    assert!(matches!(
        SlotWire::project(&expanded, &value.schedule.current).unwrap(),
        SlotWire::PendingBoundary { .. }
    ));

    let ScheduledSlot::Ready(config) = &mut value.schedule.next else {
        unreachable!()
    };
    config.height += 1;
    assert!(matches!(
        ExecutionResultCommitment::decode(&value.preimage().unwrap()),
        Err(CommitmentError::Schedule(_))
    ));
}

#[test]
fn successor_reconstruction_charges_every_clone_to_inherited_decode_budget() {
    let value = boundary_result(4);
    let epoch = &value.schedule.boundary.as_ref().unwrap().next;
    let bytes = epoch_clone_bytes(epoch).unwrap();
    assert!(
        bytes > 4 * 96,
        "includes vectors, both key rosters and all PoPs"
    );
    let slot = SlotWire::project(&value.schedule.next, epoch).unwrap();
    let limits =
        |bytes| norito::DecodeLimits::new(96, MAX_RESULT_PREIMAGE_BYTES, usize::MAX, bytes, 32);
    assert!(
        matches!(norito::with_decode_limits(limits(bytes - 1), || slot.expand(epoch)),
        Err(norito::Error::TotalAllocationExceeded { attempted, limit }) if attempted == bytes as u64 && limit == bytes as u64 - 1)
    );
    assert_eq!(
        norito::with_decode_limits(limits(bytes), || slot.expand(epoch)).unwrap(),
        value.schedule.next
    );
    norito::with_decode_limits_scope(limits(bytes * 2 - 1), || {
        slot.expand(epoch).unwrap();
        let error =
            norito::with_decode_limits(limits(usize::MAX), || slot.expand(epoch)).unwrap_err();
        assert!(
            matches!(error, norito::Error::TotalAllocationExceeded { attempted, limit }
            if attempted == bytes as u64 * 2 && limit == bytes as u64 * 2 - 1)
        );
        // The failed inner scope does not refund the first successful clone's charge.
        assert!(norito::core::reserve_decode_allocation(bytes).is_err());
    });
}

#[test]
fn result_schema_describes_the_compact_wire_projection() {
    let mut map = iroha_schema::MetaMap::new();
    ExecutionResultCommitment::update_schema_map(&mut map);
    assert_eq!(
        map.get::<ExecutionResultCommitment>(),
        map.get::<OwnedResult>()
    );
    let iroha_schema::Metadata::Enum(slots) = map.get::<SlotWire>().unwrap() else {
        panic!("slot enum")
    };
    assert_eq!(slots.variants.len(), 2);
    let ready = slots
        .variants
        .iter()
        .find(|variant| variant.tag == "Ready")
        .unwrap();
    assert_eq!(
        ready.ty,
        Some(core::any::TypeId::of::<
            iroha_schema::EnumVariantPayload<SlotWire, 0>,
        >())
    );
    let payload = map
        .get::<iroha_schema::EnumVariantPayload<SlotWire, 0>>()
        .unwrap();
    let iroha_schema::Metadata::Struct(fields) = payload else {
        panic!("ready payload")
    };
    assert_eq!(
        fields
            .declarations
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["height", "params"]
    );
}
