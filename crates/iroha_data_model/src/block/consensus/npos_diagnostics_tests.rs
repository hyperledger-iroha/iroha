#[test]
fn npos_diagnostics_rejects_zero_seed() {
    let mut value = npos_diagnostics();
    value.epoch_seed = [0; 32];
    assert_eq!(
        value.validate(),
        Err("NPoS diagnostics epoch seed must be non-zero")
    );
}
#[test]
fn npos_diagnostics_json_rejects_zero_nonzero_fields() {
    let mut value =
        norito::json::to_value(&npos_diagnostics()).expect("serialize NPoS diagnostics");
    value
        .as_object_mut()
        .expect("NPoS diagnostics object")
        .insert(
            "epoch_length_blocks".to_owned(),
            norito::json::Value::from(0_u64),
        );
    assert!(norito::json::from_value::<SumeragiNposDiagnostics>(value).is_err());
}

#[test]
fn npos_diagnostics_json_rejects_retired_vrf_counter_fields() {
    let mut value =
        norito::json::to_value(&npos_diagnostics()).expect("serialize NPoS diagnostics");
    value
        .as_object_mut()
        .expect("NPoS diagnostics object")
        .insert(
            "vrf_penalty_epoch".to_owned(),
            norito::json::Value::from(7_u64),
        );
    assert!(norito::json::from_value::<SumeragiNposDiagnostics>(value).is_err());
}
