#[test]
fn npos_diagnostics_reject_zero_seed_and_invalid_windows() {
    let mut value = npos_diagnostics();
    value.epoch_seed = [0; 32];
    assert_eq!(
        value.validate(),
        Err("NPoS diagnostics epoch seed must be non-zero")
    );

    let mut value = npos_diagnostics();
    value.vrf_reveal_deadline_offset = value.vrf_commit_deadline_offset;
    assert_eq!(
        value.validate(),
        Err("NPoS diagnostics reveal deadline must follow commit deadline")
    );

    let mut value = npos_diagnostics();
    value.vrf_reveal_deadline_offset = NonZeroU64::new(101).unwrap();
    assert_eq!(
        value.validate(),
        Err("NPoS diagnostics reveal deadline must not exceed epoch length")
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
