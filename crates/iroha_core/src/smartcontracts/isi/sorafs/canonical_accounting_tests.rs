// Canonical framed identity and persistence regression tests.

#[test]
fn pin_accounting_key_ignores_ambient_norito_layout() {
    let authority = alice();
    let canonical = norito::encode_canonical(&authority).expect("canonical account identity");
    let expected = StatePath::from_str(&format!(
        "{PIN_AUTHORITY_USAGE_STATE_KEY_PREFIX_V1}{}",
        hex::encode(blake3_hash(&canonical).as_bytes()),
    ))
    .expect("expected authority accounting key");
    let mut distinct_layout = false;
    for flags in 0..=u8::MAX {
        if norito::core::validate_header_flags(flags).is_err() {
            continue;
        }
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        let before = norito::to_bytes(&authority).expect("ambient account identity");
        distinct_layout |= before != canonical;
        assert_eq!(
            pin_authority_usage_key(&authority).expect("accounting key"),
            expected
        );
        assert_ne!(
            pin_authority_usage_key(&bob()).expect("different authority key"),
            expected
        );
        assert_eq!(
            norito::to_bytes(&authority).expect("restored layout"),
            before
        );
    }
    assert!(distinct_layout);
}

#[test]
fn v1_norito_decoders_reject_advertised_alternate_layouts() {
    let provider = ProviderId::new([0x49; 32]);
    let report = repair_report(
        "REP-ALTERNATE-LAYOUT",
        provider,
        [0x4A; 32],
        &alice(),
        4_000,
    );
    let canonical = norito::encode_canonical(&report).expect("encode canonical repair report");
    let alternate_flags =
        norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let alternate = {
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(&report).expect("encode alternate-layout repair report")
    };
    assert_ne!(alternate, canonical);
    assert_eq!(
        norito::decode_from_bytes::<RepairReportV1>(&alternate)
            .expect("ordinary Norito accepts the advertised alternate layout"),
        report
    );
    let payload_error = decode_repair_payload::<RepairReportV1>(&alternate, "repair report")
        .expect_err("admitted repair payload must reject alternate layout");
    assert!(
        smart_contract_error_message(&payload_error)
            .contains("repair report is not exact canonical Norito")
    );
    for error in [
        decode_repair_state::<RepairReportV1>(&alternate, "repair report")
            .expect_err("persisted repair state must reject alternate layout"),
        decode_stored_repair_payload::<RepairReportV1>(&alternate, "repair report")
            .expect_err("stored repair payload must reject alternate layout"),
    ] {
        assert!(matches!(
            error,
            InstructionExecutionError::InvariantViolation(message)
                if message.contains("repair report is not exact canonical Norito")
        ));
    }
    let mut alias = default_alias_binding();
    let bundle = decode_alias_proof_untrusted_signers(&alias.proof)
        .expect("decode canonical alias fixture integrity");
    alias.proof = {
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        norito::to_bytes(&bundle).expect("encode alternate-layout alias proof")
    };
    let alias_error = validate_manifest_alias_binding(
        &alias,
        &default_digest(),
        &default_root_cid(),
        Some((5, default_policy().retention_epoch)),
    )
    .expect_err("alias proof must reject alternate layout");
    assert!(
        smart_contract_error_message(&alias_error).contains("not canonical Norito"),
        "unexpected alias rejection: {alias_error:?}"
    );
}
