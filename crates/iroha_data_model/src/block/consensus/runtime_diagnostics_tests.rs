// Runtime diagnostics fixtures and regression tests included in the parent test module.
fn npos_diagnostics() -> SumeragiNposDiagnostics {
    SumeragiNposDiagnostics {
        epoch_length_blocks: NonZeroU64::new(100).unwrap(),
        epoch_seed: [0xA5; 32],
    }
}
fn diagnostics(npos: Option<SumeragiNposDiagnostics>) -> SumeragiDiagnosticsStatus {
    SumeragiDiagnosticsStatus {
        pipeline_execution: SumeragiPipelineExecutionStatus::default(),
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
        lane_commitments: Vec::new(),
        dataspace_commitments: Vec::new(),
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
