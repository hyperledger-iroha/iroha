// Structural coverage for the sole guarded provider-credit instruction layout.

#[test]
fn provider_credit_cas_json_requires_explicit_null_or_exact_hash() {
    let initial = UpsertProviderCredit::new(None, provider_credit());
    let value = norito::json::to_value(&initial).expect("initial JSON");
    assert_eq!(
        value.get("expected_current"),
        Some(&norito::json::Value::Null)
    );
    assert_eq!(
        norito::json::from_value::<UpsertProviderCredit>(value.clone()).unwrap(),
        initial
    );
    let mut omitted = value;
    omitted.as_object_mut().unwrap().remove("expected_current");
    assert!(norito::json::from_value::<UpsertProviderCredit>(omitted).is_err());

    let hash = HashOf::new(&initial.record);
    let update = UpsertProviderCredit::new(Some(hash), provider_credit());
    let value = norito::json::to_value(&update).unwrap();
    assert_eq!(
        value.get("expected_current"),
        Some(&norito::json::to_value(&hash).unwrap())
    );
    assert_eq!(
        norito::json::from_value::<UpsertProviderCredit>(value).unwrap(),
        update
    );
}

#[test]
fn provider_credit_cas_has_one_canonical_guarded_frame() {
    for expected_current in [None, Some(HashOf::new(&provider_credit()))] {
        let request = UpsertProviderCredit::new(expected_current, provider_credit());
        assert_slice_roundtrip(request.clone());
        let wire = norito::to_bytes(&request).unwrap();
        let limits = norito::DecodeLimits::new(4096, 64 * 1024, 4096, 256 * 1024, 64);
        assert_eq!(
            norito::decode_canonical_with_limits::<UpsertProviderCredit>(&wire, limits).unwrap(),
            request
        );
        let mut trailing = wire;
        trailing.push(0);
        assert!(
            norito::decode_canonical_with_limits::<UpsertProviderCredit>(&trailing, limits)
                .is_err()
        );
    }
}

#[test]
fn provider_credit_cas_rejects_the_unguarded_record_layout() {
    #[derive(Encode)]
    struct UnguardedProviderCredit {
        record: ProviderCreditRecord,
    }
    let retired = UnguardedProviderCredit {
        record: provider_credit(),
    }
    .encode();
    assert!(
        <UpsertProviderCredit as norito::core::DecodeFromSlice>::decode_from_slice(&retired)
            .is_err()
    );
    assert!(norito::decode_from_bytes::<UpsertProviderCredit>(&retired).is_err());
}

#[test]
#[ignore = "explicit native maintenance capture of both guarded provider-credit cases"]
fn print_provider_credit_cas_record_fixture_rows() {
    // Preserve both populated record inputs from the registry and slice-decoder
    // fixtures. The current native codec alone supplies all schema/frame hashes.
    let registry_record = ProviderCreditRecord::new(
        provider(0xC1),
        xor_quantity_nanos(1),
        Quantity::zero(),
        Quantity::zero(),
        Quantity::zero(),
        0,
        0,
        Metadata::default(),
    );
    let selected = provider_credit();
    let rows = vec![
        crate::isi::generated_record_identity_tests::capture(UpsertProviderCredit::new(
            None,
            registry_record,
        )),
        crate::isi::generated_record_identity_tests::capture(UpsertProviderCredit::new(
            Some(HashOf::new(&selected)),
            selected,
        )),
    ];
    println!(
        "PROVIDER_CREDIT_CAS_FIXTURE_ROWS={}",
        norito::json::to_json(&rows).unwrap()
    );
}
