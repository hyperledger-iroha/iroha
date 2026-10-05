//! Original encoded-source custody across current/undo lane signer admission.
use super::*;
use iroha_allocation::AllocationBudget;
use iroha_data_model::sumeragi_lanes::{
    SumeragiLaneCustody, SumeragiLaneCustodySigners, SumeragiLaneFrontier,
    SumeragiLaneSignerCustody, SumeragiLaneStakeBinding, SumeragiLaneState,
};

fn lanes(identity: u8) -> SumeragiLaneState {
    let mut value = SumeragiLaneState::default();
    value.custody.push(SumeragiLaneCustody {
        lane: LaneId::new(1),
        incarnation: [identity; 32],
        instance: [identity + 1; 32],
        created_at: 10,
        merged: SumeragiLaneFrontier::default(),
        signer_count: 4,
        signers: vec![SumeragiLaneSignerCustody {
            signer: 0,
            binding: SumeragiLaneStakeBinding {
                owner_lane: LaneId::SINGLE,
                validator: iroha_crypto::Hash::prehashed([identity; 32]),
                activation_height: 1,
                tenure: iroha_crypto::Hash::prehashed([identity + 1; 32]),
            },
        }]
        .try_into()
        .unwrap(),
        evidence_horizon: 7,
        slashing_delay: 3,
        retired_at: None,
    });
    value
}
fn demand() -> usize {
    std::mem::size_of::<SumeragiLaneSignerCustody>()
        + SumeragiLaneCustodySigners::control_layout().size()
}
fn raw_field(map: &SnapshotJsonMap<'_>) -> *const u8 {
    match map.fields.get("sumeragi_lanes").unwrap() {
        SnapshotJsonField::Borrowed { raw } => raw.as_ptr(),
        SnapshotJsonField::Owned(_) => panic!("production raw field required"),
    }
}

#[test]
fn native_lane_signer_snapshot_retains_exact_raw_source_until_both_cuts_are_funded() {
    let source = NativeLaneCustodySnapshot {
        revert: Some(mv::json::SnapshotUndoValue { value: lanes(1) }),
        blocks: lanes(2),
    };
    let field = json::to_json(&source).unwrap();
    let raw = format!("{{\"sumeragi_lanes\":{field}}}");
    let mut map = SnapshotJsonMap::parse(&raw, "world").unwrap();
    let pointer = raw_field(&map);
    let pool = AllocationBudget::new(2 * demand() - 1);
    let error = take_native_lane_custody(&mut map, &pool).err().unwrap();
    assert!(matches!(error, StateRestoreError::NativeLaneCustody(_)));
    assert_eq!(raw_field(&map), pointer);
    assert_eq!(
        pool.reserved_bytes(),
        0,
        "refused attempt releases its partial copy"
    );
    pool.set_limit_bytes(2 * demand());
    let restored = take_native_lane_custody(&mut map, &pool).unwrap();
    assert!(map.is_empty());
    assert_eq!(*restored.view().get(), source.blocks);
    assert_eq!(
        *restored.predecessor_view().get(),
        source.revert.as_ref().map(|undo| undo.value.clone())
    );
    assert!(restored.view().custody[0].signers.admitted_to(&pool));
    assert!(
        restored.predecessor_view().get().as_ref().unwrap().custody[0]
            .signers
            .admitted_to(&pool)
    );
    assert_eq!(pool.reserved_bytes(), 2 * demand());
    // Both cuts use the sole explicit-undo Cell schema; admission adds no wire field.
    assert_eq!(json::to_json(&restored).unwrap(), field);
    let original = restored.view().custody[0].signers.as_slice().as_ptr();
    let copy = restored.view().get().clone();
    assert_eq!(copy.custody[0].signers.as_slice().as_ptr(), original);
    assert_eq!(pool.reserved_bytes(), 2 * demand());
}

#[test]
fn native_lane_signer_snapshot_rejects_noncanonical_fields_without_consuming_source() {
    let current = json::to_json(&lanes(1)).unwrap();
    for field in [
        format!("{{\"blocks\":{current},\"revert\":null}}"),
        format!("{{\"revert\":null,\"blocks\":{current},\"extra\":0}}"),
        format!("{{\"blocks\":{current}}}"),
    ] {
        let raw = format!("{{\"sumeragi_lanes\":{field}}}");
        let mut map = SnapshotJsonMap::parse(&raw, "world").unwrap();
        let pointer = raw_field(&map);
        let pool = AllocationBudget::new(demand());
        assert!(matches!(
            take_native_lane_custody(&mut map, &pool),
            Err(StateRestoreError::Serialization(_))
        ));
        assert_eq!(raw_field(&map), pointer);
        assert_eq!(pool.reserved_bytes(), 0);
    }
    // The outer parser rejects duplicates before handing any field to this decoder.
    let duplicate = format!(
        "{{\"sumeragi_lanes\":{{\"revert\":null,\"blocks\":{current},\"blocks\":{current}}}}}"
    );
    let error = SnapshotJsonMap::parse(&duplicate, "world").err().unwrap();
    assert!(
        matches!(error, json::Error::InvalidField { ref field, ref message }
        if field == "world" && message == "JSON error: duplicate field `blocks`"),
        "{error:?}"
    );
}

#[test]
fn native_lane_signer_snapshot_decode_refusal_retains_category_and_original_field() {
    let field = json::to_json(&NativeLaneCustodySnapshot {
        revert: None,
        blocks: lanes(1),
    })
    .unwrap();
    let raw = format!("{{\"sumeragi_lanes\":{field}}}");
    let mut map = SnapshotJsonMap::parse(&raw, "world").unwrap();
    let pointer = raw_field(&map);
    let pool = AllocationBudget::new(demand());
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
    let error =
        norito::with_decode_limits_scope(limits, || take_native_lane_custody(&mut map, &pool))
            .err()
            .unwrap();
    assert!(
        matches!(error, StateRestoreError::Serialization(ref error) if error.is_decode_resource_limit()),
        "{error:?}"
    );
    assert_eq!(raw_field(&map), pointer);
    assert_eq!(pool.reserved_bytes(), 0);
    let restored = take_native_lane_custody(&mut map, &pool).unwrap();
    assert!(restored.view().custody[0].signers.admitted_to(&pool));
}

#[test]
fn native_lane_sample_snapshot_retains_raw_source_through_both_cut_refusal_and_retry() {
    use iroha_data_model::sumeragi_lanes::{
        LaneSamplesAdmissionError, LaneStateAdmissionError, SumeragiLaneSample, SumeragiLaneSamples,
    };
    let samples = |height| {
        vec![SumeragiLaneSample {
            height,
            time_ms: height * 1000,
            transactions: 1,
            lanes: 1,
        }]
        .try_into()
        .unwrap()
    };
    let source = NativeLaneCustodySnapshot {
        blocks: SumeragiLaneState {
            samples: samples(3),
            ..SumeragiLaneState::default()
        },
        revert: Some(mv::json::SnapshotUndoValue {
            value: SumeragiLaneState {
                samples: samples(2),
                ..SumeragiLaneState::default()
            },
        }),
    };
    let field = json::to_json(&source).unwrap();
    let raw = format!("{{\"sumeragi_lanes\":{field}}}");
    let mut map = SnapshotJsonMap::parse(&raw, "world").unwrap();
    let pointer = raw_field(&map);
    let demand =
        std::mem::size_of::<SumeragiLaneSample>() + SumeragiLaneSamples::control_layout().size();
    let pool = AllocationBudget::new(2 * demand - 1);
    assert!(matches!(
        take_native_lane_custody(&mut map, &pool),
        Err(StateRestoreError::NativeLaneCustody(
            LaneStateAdmissionError::Samples(LaneSamplesAdmissionError::Admission(_))
        ))
    ));
    assert_eq!(raw_field(&map), pointer);
    assert_eq!(
        pool.reserved_bytes(),
        0,
        "partial current-cut owner refunds on undo refusal"
    );
    pool.set_limit_bytes(2 * demand);
    let restored = take_native_lane_custody(&mut map, &pool).unwrap();
    assert!(map.is_empty());
    assert_eq!(*restored.view().get(), source.blocks);
    assert_eq!(
        *restored.predecessor_view().get(),
        source.revert.as_ref().map(|undo| undo.value.clone())
    );
    assert!(restored.view().samples.admitted_to(&pool));
    assert!(
        restored
            .predecessor_view()
            .get()
            .as_ref()
            .unwrap()
            .samples
            .admitted_to(&pool)
    );
    assert_eq!(pool.reserved_bytes(), 2 * demand);
    assert_eq!(json::to_json(&restored).unwrap(), field);
}
