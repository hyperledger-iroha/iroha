// Custom reference budgets and signed output frames share the canonical V1 boundary.
fn assert_bounded_reference_frame<T>(
    value: &T,
    schema: &str,
    maximum_bytes: usize,
    decode: impl Fn(&[u8]) -> Result<T, String>,
    validate: impl Fn(&[u8]) -> ValidationOutcomeV1,
) where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
    T: PartialEq + std::fmt::Debug,
{
    let canonical = norito::encode_canonical(value).unwrap();
    let expected = validate(&canonical);
    assert!(expected.is_ok(), "{expected:?}");
    let tagged = crate::canonical_test_support::with_compression_tag(value);
    let header = norito::core::Header::read(tagged.as_slice()).unwrap();
    let offset = header.magic.len() + 2 + header.schema.len() + 1;
    let mut advertised = tagged[..norito::core::Header::SIZE].to_vec();
    advertised[offset..offset + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        norito::core::Header::read(advertised.as_slice())
            .unwrap()
            .length,
        u64::MAX
    );
    let rejection = format!("{schema} payload is not the exact canonical Norito encoding");
    let no_allocation = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 128);
    let mut distinct_layouts = 0;
    for flags in reference_frame_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(decode(&canonical).unwrap(), *value);
        assert_eq!(validate(&canonical), expected);
        for bytes in [&tagged, &advertised] {
            assert_eq!(
                norito::with_decode_limits_scope(no_allocation, || decode(bytes)),
                Err(rejection.clone())
            );
            let outcome = validate(bytes);
            assert_failure(&outcome, "SFS-NORITO-001", CATEGORY_NORITO);
            assert!(outcome.message.contains(&rejection));
            assert_eq!(outcome.inputs, expected.inputs);
            assert_eq!(outcome.generated_at, expected.generated_at);
        }
        let limit_error =
            norito::with_decode_limits_scope(no_allocation, || decode(&canonical)).unwrap_err();
        assert!(
            limit_error.starts_with("cumulative allocation "),
            "{limit_error}"
        );
        assert!(
            limit_error.ends_with(" bytes exceeds decode limit 0"),
            "{limit_error}"
        );
        let alternate = norito::core::to_bytes(value).unwrap();
        if alternate != canonical {
            distinct_layouts += 1;
            assert_eq!(norito::decode_from_bytes::<T>(&alternate).unwrap(), *value);
            assert_eq!(decode(&alternate), Err(rejection.clone()));
            assert_failure(&validate(&alternate), "SFS-NORITO-001", CATEGORY_NORITO);
        }
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert!(distinct_layouts > 0);
    let oversized = vec![0; maximum_bytes + 1];
    assert_eq!(
        decode(&oversized).unwrap_err(),
        format!(
            "{schema} payload is {} bytes; maximum canonical size is {maximum_bytes}",
            oversized.len()
        )
    );
}

#[test]
fn reference_cancel_lock_decoder_preserves_canonical_budget_and_outcome() {
    let value: CancelAssetLockWireV1 =
        fixture("fixtures/sorafs_manifest/appeal_finance/cancel_asset_lock_v1.to");
    assert_bounded_reference_frame(
        &value,
        "CancelAssetLock",
        CANCEL_ASSET_LOCK_REFERENCE_MAX_BYTES_V1,
        decode_cancel_asset_lock_reference,
        |bytes| validate_appeal_finance_cancel_asset_lock_bytes(bytes, "cancel.to", 101),
    );
    let zero = fixture_bytes(
        "fixtures/sorafs_manifest/appeal_finance/negative/cancel_asset_lock_zero_expected_v1.to",
    );
    for flags in reference_frame_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        assert_failure(
            &validate_appeal_finance_cancel_asset_lock_bytes(&zero, "zero.to", 102),
            "SFS-VAL-001",
            CATEGORY_VALIDATION,
        );
    }
}

#[test]
fn reference_pdp_decoders_preserve_canonical_budgets_and_signed_outcomes() {
    let commitment: PdpCommitmentV1 = fixture("fixtures/sorafs_manifest/pdp/commitment_v1.to");
    let challenge: PdpChallengeV1 = fixture("fixtures/sorafs_manifest/pdp/challenge_v1.to");
    let proof: PdpProofV1 = fixture("fixtures/sorafs_manifest/pdp/proof_v1.to");
    assert_bounded_reference_frame(
        &commitment,
        "PdpCommitmentV1",
        PDP_COMMITMENT_MAX_CANONICAL_BYTES_V1,
        decode_pdp_commitment_reference,
        |bytes| validate_pdp_commitment_bytes(bytes, "commitment.to", 111),
    );
    assert_bounded_reference_frame(
        &challenge,
        "PdpChallengeV1",
        PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1,
        decode_pdp_challenge_reference,
        |bytes| validate_pdp_challenge_bytes(bytes, "challenge.to", 112),
    );
    assert_bounded_reference_frame(
        &proof,
        "PdpProofV1",
        PDP_PROOF_MAX_CANONICAL_BYTES_V1,
        decode_pdp_proof_reference,
        |bytes| validate_pdp_proof_bytes(bytes, "proof.to", 113),
    );
    let mut tampered = proof;
    tampered.signature.signature[0] ^= 1;
    let bytes = norito::encode_canonical(&tampered).unwrap();
    let expected = validate_pdp_proof_bytes(&bytes, "tampered.to", 114);
    assert!(!expected.is_ok());
    assert_eq!(expected.category, CATEGORY_SIGNATURE);
    for flags in reference_frame_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            validate_pdp_proof_bytes(&bytes, "tampered.to", 114),
            expected
        );
    }
}

#[test]
fn reference_orderbook_signed_outputs_ignore_caller_layout() {
    let seed = [0xB7; 32];
    let key = SigningKey::from_bytes(&seed);
    let order = orderbook_order_request();
    let cancel = orderbook_order_cancel();
    let receipt = orderbook_settlement_receipt();
    let cases = [
        (
            OrderbookKind::OrderRequest,
            norito::encode_canonical(&order).unwrap(),
            norito::encode_canonical(&sign_order_request_ed25519_v1(order, &key).unwrap()).unwrap(),
        ),
        (
            OrderbookKind::OrderCancel,
            norito::encode_canonical(&cancel).unwrap(),
            norito::encode_canonical(&sign_order_cancel_ed25519_v1(cancel, &key).unwrap()).unwrap(),
        ),
        (
            OrderbookKind::SettlementReceipt,
            norito::encode_canonical(&receipt).unwrap(),
            norito::encode_canonical(&sign_settlement_receipt_ed25519_v1(receipt, &key).unwrap())
                .unwrap(),
        ),
    ];
    for flags in reference_frame_layouts() {
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        for (kind, input, expected) in &cases {
            let signed = sign_orderbook_payload_bytes_ed25519_v1(*kind, input, &seed).unwrap();
            assert_eq!(&signed, expected);
            assert_success(&validate_orderbook_payload_bytes(
                *kind,
                &signed,
                "signed.to",
                121,
            ));
        }
        assert_signed_order_request(&cases[0].2, &verifying_key(&seed));
        assert_signed_order_cancel(&cases[1].2, &verifying_key(&seed));
        assert_signed_settlement_receipt(&cases[2].2, &verifying_key(&seed));
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
}
