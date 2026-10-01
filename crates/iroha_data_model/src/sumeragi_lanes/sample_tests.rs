//! Original immutable sample-window ownership and exact canonical projection.
use super::*;

#[test]
fn lane_sample_clones_retain_original_backing() {
    let source = SumeragiLaneState {
        samples: vec![SumeragiLaneSample {
            height: 1,
            time_ms: 10,
            transactions: 3,
            lanes: 1,
        }]
        .try_into()
        .unwrap(),
        ..SumeragiLaneState::default()
    };
    let copy = source.clone();
    assert_eq!(source.samples, copy.samples);
    assert_eq!(
        source.samples.as_ptr(),
        copy.samples.as_ptr(),
        "World clones must retain the original immutable sample allocation"
    );
}

fn sample(height: u64) -> SumeragiLaneSample {
    SumeragiLaneSample {
        height,
        time_ms: height * 1000,
        transactions: height,
        lanes: 1,
    }
}
fn demand(count: usize) -> usize {
    count * std::mem::size_of::<SumeragiLaneSample>() + SumeragiLaneSamples::control_layout().size()
}
#[test]
fn lane_samples_admit_exact_original_pool_and_refund_after_last_clone() {
    use iroha_allocation::AllocationBudget;
    let decoded: SumeragiLaneSamples = vec![sample(1), sample(2)].try_into().unwrap();
    let source_pointer = decoded.as_ptr();
    let pool = AllocationBudget::new(demand(2) - 1);
    assert!(matches!(
        decoded.admit(&pool),
        Err(LaneSamplesAdmissionError::Admission(_))
    ));
    assert_eq!(pool.reserved_bytes(), 0);
    assert_eq!(decoded.as_ptr(), source_pointer);
    pool.set_limit_bytes(demand(2));
    let admitted = decoded.admit(&pool).unwrap();
    let pointer = admitted.as_ptr();
    assert_ne!(source_pointer, pointer);
    assert_eq!(pool.reserved_bytes(), demand(2));
    pool.set_limit_bytes(0);
    let retained = admitted.admit(&pool).unwrap();
    assert_eq!(retained.as_ptr(), pointer);
    let foreign = AllocationBudget::new(demand(3));
    assert!(matches!(
        admitted.admit(&foreign),
        Err(LaneSamplesAdmissionError::ForeignBudget)
    ));
    assert!(matches!(
        admitted.retain_and_append(sample(3), 3, &foreign),
        Err(LaneSamplesAdmissionError::ForeignBudget)
    ));
    assert_eq!(foreign.reserved_bytes(), 0);
    drop(admitted);
    assert_eq!(pool.reserved_bytes(), demand(2));
    drop(retained);
    assert_eq!(pool.reserved_bytes(), 0);
}
#[test]
fn lane_samples_suffix_append_precharges_exact_overlap_and_retries_same_source() {
    use iroha_allocation::AllocationBudget;
    let decoded: SumeragiLaneSamples = vec![sample(1), sample(2), sample(3)].try_into().unwrap();
    let pool = AllocationBudget::new(demand(3));
    let original = decoded.admit(&pool).unwrap();
    let pointer = original.as_ptr();
    pool.set_limit_bytes(demand(3) + demand(2) - 1);
    assert!(matches!(
        original.retain_and_append(sample(4), 2, &pool),
        Err(LaneSamplesAdmissionError::Admission(_))
    ));
    assert_eq!(original.as_ptr(), pointer);
    assert_eq!(original.as_slice(), &[sample(1), sample(2), sample(3)]);
    assert_eq!(pool.reserved_bytes(), demand(3));
    pool.set_limit_bytes(demand(3) + demand(2));
    let appended = original.retain_and_append(sample(4), 2, &pool).unwrap();
    assert_eq!(appended.as_slice(), &[sample(3), sample(4)]);
    assert_eq!(pool.reserved_bytes(), demand(3) + demand(2));
    drop(appended);
    assert_eq!(pool.reserved_bytes(), demand(3));
    pool.set_limit_bytes(demand(3) + demand(4));
    let unbounded = original
        .retain_and_append(sample(4), usize::MAX, &pool)
        .unwrap();
    assert_eq!(unbounded.len(), 4);
    assert_eq!(pool.reserved_bytes(), demand(3) + demand(4));
    let empty = original.retain_and_append(sample(4), 0, &pool).unwrap();
    assert!(empty.is_empty());
    drop((empty, unbounded, original));
    assert_eq!(pool.reserved_bytes(), 0);
}
#[test]
fn lane_samples_preserve_exact_vec_binary_json_and_state_schema() {
    #[derive(norito::Encode, norito::NoritoSchema, norito::json::JsonSerialize)]
    #[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLaneState")]
    struct OriginalState {
        lanes: Vec<SumeragiLaneRecord>,
        custody: Vec<SumeragiLaneCustody>,
        samples: Vec<SumeragiLaneSample>,
        last_transition: u64,
        incarnations: u64,
    }
    for rows in [vec![], vec![sample(1), sample(2)]] {
        let decoded: SumeragiLaneSamples = rows.clone().try_into().unwrap();
        let pool = iroha_allocation::AllocationBudget::new(demand(rows.len()));
        let admitted = decoded.admit(&pool).unwrap();
        let state = SumeragiLaneState {
            samples: admitted,
            last_transition: 7,
            incarnations: 9,
            ..SumeragiLaneState::default()
        };
        let original = OriginalState {
            lanes: vec![],
            custody: vec![],
            samples: rows.clone(),
            last_transition: 7,
            incarnations: 9,
        };
        assert_eq!(
            norito::encode_canonical(&decoded).unwrap(),
            norito::encode_canonical(&rows).unwrap()
        );
        let bytes = norito::encode_canonical(&state).unwrap();
        assert_eq!(bytes, norito::encode_canonical(&original).unwrap());
        assert_eq!(
            norito::decode_canonical::<SumeragiLaneState>(&bytes).unwrap(),
            state
        );
        let json = norito::json::to_json(&state).unwrap();
        assert_eq!(json, norito::json::to_json(&original).unwrap());
        assert_eq!(
            norito::json::from_str::<SumeragiLaneState>(&json).unwrap(),
            state
        );
    }
    let mut map = iroha_schema::MetaMap::new();
    SumeragiLaneState::update_schema_map(&mut map);
    let iroha_schema::Metadata::Struct(meta) = map.get::<SumeragiLaneState>().unwrap() else {
        panic!("canonical State struct")
    };
    assert_eq!(
        meta.declarations
            .iter()
            .map(|d| d.name.as_str())
            .collect::<Vec<_>>(),
        [
            "lanes",
            "custody",
            "samples",
            "last_transition",
            "incarnations"
        ]
    );
    assert_eq!(
        meta.declarations[2].ty,
        std::any::TypeId::of::<Vec<SumeragiLaneSample>>()
    );
}
#[test]
fn lane_sample_decoded_control_refusal_is_typed_and_retry_does_not_claim_progress() {
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
    let error =
        norito::with_decode_limits_scope(limits, || SumeragiLaneSamples::try_from(vec![sample(1)]))
            .unwrap_err();
    assert!(error.is_decode_resource_limit(), "{error:?}");
    let retried = SumeragiLaneSamples::try_from(vec![sample(1)]).unwrap();
    assert_eq!(retried.as_slice(), &[sample(1)]);
    let pool = iroha_allocation::AllocationBudget::new(0);
    assert!(!retried.admitted_to(&pool));
    assert!(
        SumeragiLaneSamples::default()
            .admit(&pool)
            .unwrap()
            .is_empty()
    );
    assert_eq!(pool.reserved_bytes(), 0);
}
