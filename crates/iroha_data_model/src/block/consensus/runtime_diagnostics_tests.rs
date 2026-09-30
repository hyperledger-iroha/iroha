// Runtime diagnostics fixtures and regression tests included in the parent test module.
fn npos_diagnostics() -> SumeragiNposDiagnostics {
    SumeragiNposDiagnostics {
        epoch_length_blocks: NonZeroU64::new(100).unwrap(),
        epoch_seed: [0xA5; 32],
    }
}
fn diagnostics(npos: Option<SumeragiNposDiagnostics>) -> SumeragiDiagnosticsStatus {
    SumeragiDiagnosticsStatus {
        tx_queue_depth: 0,
        tx_queue_capacity: 1,
        tx_queue_retained_bytes: 0,
        tx_queue_max_retained_bytes: 1,
        tx_queue_saturated: false,
        tx_queue_saturated_by_count: false,
        tx_queue_saturated_by_bytes: false,
        tx_queue_saturated_by_age: false,
        tx_queue_oldest_queued_age_ms: 0,
        npos,
        lane_governance_sealed_total: 0,
        lane_governance_sealed_aliases: Vec::new(),
        lane_governance: Vec::new(),
    }
}
#[test]
fn permissioned_diagnostics_omit_npos_shape() {
    let value = norito::json::to_value(&diagnostics(None)).expect("serialize diagnostics");
    assert!(
        value
            .as_object()
            .expect("diagnostics object")
            .get("npos")
            .is_none()
    );
}
#[test]
fn diagnostics_current_shape_roundtrips_without_retired_commitments() {
    for npos in [None, Some(npos_diagnostics())] {
        let expected = diagnostics(npos);
        let encoded = expected.encode();
        assert_eq!(
            SumeragiDiagnosticsStatus::decode_all(&mut encoded.as_slice())
                .expect("decode exact current diagnostics"),
            expected
        );
        let value = norito::json::to_value(&expected).expect("serialize diagnostics");
        assert_eq!(
            norito::json::from_value::<SumeragiDiagnosticsStatus>(value.clone())
                .expect("decode current diagnostics JSON"),
            expected
        );
        for field in [
            "lane_commitments",
            "dataspace_commitments",
            "pipeline_execution",
        ] {
            let mut retired = value.clone();
            let object = retired.as_object_mut().expect("diagnostics object");
            assert!(!object.contains_key(field));
            object.insert(field.to_owned(), norito::json::Value::Array(Vec::new()));
            assert!(
                norito::json::from_value::<SumeragiDiagnosticsStatus>(retired).is_err(),
                "retired {field} must not be accepted even when empty"
            );
        }
    }
}
#[test]
fn diagnostics_binary_rejects_retired_empty_commitment_slots() {
    // Empty vectors have the same encoding for every element type. This hostile
    // layout preserves the exact retired slot positions without restoring DTOs.
    #[derive(Encode)]
    #[expect(clippy::struct_excessive_bools, reason = "The rejection fixture must retain each separate retired wire slot.")]
    struct RetiredCommitmentDiagnostics {
        tx_queue_depth: u64,
        tx_queue_capacity: u64,
        tx_queue_retained_bytes: u64,
        tx_queue_max_retained_bytes: u64,
        tx_queue_saturated: bool,
        tx_queue_saturated_by_count: bool,
        tx_queue_saturated_by_bytes: bool,
        tx_queue_saturated_by_age: bool,
        tx_queue_oldest_queued_age_ms: u64,
        npos: Option<SumeragiNposDiagnostics>,
        lane_commitments: Vec<()>,
        dataspace_commitments: Vec<()>,
        lane_governance_sealed_total: u32,
        lane_governance_sealed_aliases: Vec<String>,
        lane_governance: Vec<SumeragiLaneGovernance>,
    }
    let expected = diagnostics(Some(npos_diagnostics()));
    let retired = RetiredCommitmentDiagnostics {
        tx_queue_depth: expected.tx_queue_depth,
        tx_queue_capacity: expected.tx_queue_capacity,
        tx_queue_retained_bytes: expected.tx_queue_retained_bytes,
        tx_queue_max_retained_bytes: expected.tx_queue_max_retained_bytes,
        tx_queue_saturated: expected.tx_queue_saturated,
        tx_queue_saturated_by_count: expected.tx_queue_saturated_by_count,
        tx_queue_saturated_by_bytes: expected.tx_queue_saturated_by_bytes,
        tx_queue_saturated_by_age: expected.tx_queue_saturated_by_age,
        tx_queue_oldest_queued_age_ms: expected.tx_queue_oldest_queued_age_ms,
        npos: expected.npos,
        lane_commitments: Vec::new(),
        dataspace_commitments: Vec::new(),
        lane_governance_sealed_total: expected.lane_governance_sealed_total,
        lane_governance_sealed_aliases: expected.lane_governance_sealed_aliases,
        lane_governance: expected.lane_governance,
    }
    .encode();
    assert!(SumeragiDiagnosticsStatus::decode_all(&mut retired.as_slice()).is_err());
}
#[test]
fn diagnostics_json_rejects_unknown_outer_and_npos_fields() {
    let mut outer = norito::json::to_value(&diagnostics(Some(npos_diagnostics())))
        .expect("serialize diagnostics");
    outer
        .as_object_mut()
        .expect("diagnostics object")
        .insert("unknown".to_owned(), norito::json::Value::from(1_u64));
    assert!(norito::json::from_value::<SumeragiDiagnosticsStatus>(outer).is_err());
    let mut nested = norito::json::to_value(&diagnostics(Some(npos_diagnostics())))
        .expect("serialize diagnostics");
    nested
        .as_object_mut()
        .and_then(|root| root.get_mut("npos"))
        .and_then(norito::json::Value::as_object_mut)
        .expect("NPoS diagnostics object")
        .insert("unknown".to_owned(), norito::json::Value::from(true));
    assert!(norito::json::from_value::<SumeragiDiagnosticsStatus>(nested).is_err());
}
