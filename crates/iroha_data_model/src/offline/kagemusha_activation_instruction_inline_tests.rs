fn activation_promotion_binding_wire_fixture(
    activation: &KagemushaRecursiveSpendReleaseActivationV4,
    policy: &OfflineDeviceAttestationPolicy,
    promotion_id: [u8; 32],
) -> KagemushaV4PromotionBindingV1 {
    let manifest = &activation.release_record.manifest;
    KagemushaV4PromotionBindingV1 {
        promotion_controller: KeyPair::from_seed(vec![0xA4; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
        promotion_reservation: KagemushaExactBytesDigestV1::from_bytes(b"wire reservation")
            .expect("wire reservation identity"),
        promotion_id,
        network_id: manifest.network_id,
        reviewed_source_closure_descriptor_sha256: manifest
            .reviewed_source_closure_descriptor_sha256,
        manifest_sha256: manifest.canonical_sha256().expect("wire manifest identity"),
        release_record_sha256: digest(
            &norito::encode_canonical(&activation.release_record)
                .expect("canonical wire release record"),
        ),
        release_policy_source: KagemushaExactBytesDigestV1::from_bytes(b"wire release policy")
            .expect("wire release-policy identity"),
        device_attestation_policy_norito: KagemushaExactBytesDigestV1::from_bytes(
            &norito::encode_canonical(policy).expect("canonical wire device policy"),
        )
        .expect("wire device-policy identity"),
        signed_genesis: KagemushaExactBytesDigestV1::from_bytes(b"wire signed genesis")
            .expect("wire signed-genesis identity"),
        catalog_consensus_policy_digest: digest(b"wire catalog policy"),
        execution_policy_hash: Hash::new(b"wire execution policy"),
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the wire audit keeps current roundtrips and every retired one- and two-field layout rejection together"
)]
fn v4_activation_wire_binds_policy_and_rejects_legacy_layouts() {
    #[derive(Encode)]
    struct RetiredOneFieldActivation {
        activation: KagemushaRecursiveSpendReleaseActivationV4,
    }
    #[derive(Encode)]
    struct RetiredTwoFieldActivation {
        activation: KagemushaRecursiveSpendReleaseActivationV4,
        device_attestation_policy: OfflineDeviceAttestationPolicy,
    }
    fn encode_with_flags(value: &impl norito::NoritoSerialize, flags: u8) -> Vec<u8> {
        let _flags = norito::core::DecodeFlagsGuard::enter(flags);
        let mut bytes = Vec::new();
        let mut encoder = norito::core::Encoder::for_buffer(&mut bytes);
        value
            .serialize(&mut encoder)
            .expect("encode activation payload with explicit layout flags");
        bytes
    }

    let promotion_id = [0xA4; 32];
    let policy = device_attestation_policy_wire_fixture();
    let activation = release_activation_wire_fixture();
    let promotion_binding =
        activation_promotion_binding_wire_fixture(&activation, &policy, promotion_id);
    let instruction = ActivateKagemushaRecursiveReleaseV4::new(
        promotion_binding.clone(),
        activation.clone(),
        policy.clone(),
    );
    let boxed = InstructionBox::from(instruction.clone());
    let bytes = norito::core::to_bytes(&boxed).expect("serialize composite activation");
    let archived = norito::core::from_bytes::<InstructionBox>(&bytes)
        .expect("decode composite activation archive");
    let decoded = InstructionBox::try_deserialize(archived)
        .expect("deserialize composite activation instruction");
    assert_eq!(
        decoded
            .as_any()
            .downcast_ref::<ActivateKagemushaRecursiveReleaseV4>(),
        Some(&instruction),
        "the embedded policy and promotion binding must survive the instruction-box wire path",
    );
    assert_eq!(instruction.device_attestation_policy(), &policy);
    assert_eq!(instruction.promotion_binding(), &promotion_binding);
    assert_eq!(instruction.promotion_id(), &promotion_id);
    assert!(instruction.validate_promotion_id().is_ok());
    let mut zero_binding = promotion_binding;
    zero_binding.promotion_id = [0; 32];
    let zero_promotion =
        ActivateKagemushaRecursiveReleaseV4::new(zero_binding, activation, policy.clone());
    assert!(zero_promotion.validate_promotion_id().is_err());
    let encoded = instruction.encode();
    let flags = norito::core::default_encode_flags();
    assert_eq!(
        flags & norito::core::header_flags::PACKED_STRUCT,
        0,
        "legacy-layout fixture requires the canonical AoS encoding",
    );
    let (roundtrip, used) = ActivateKagemushaRecursiveReleaseV4::decode_from_slice(&encoded)
        .expect("composite activation payload must roundtrip");
    assert_eq!(used, encoded.len());
    assert_eq!(roundtrip, instruction);
    let mut legacy_len = 0usize;
    crate::isi::read_aos_field(&encoded, &mut legacy_len, flags)
        .expect("read the former activation-only field");
    assert!(legacy_len < encoded.len());
    assert!(
        ActivateKagemushaRecursiveReleaseV4::decode_from_slice(&encoded[..legacy_len]).is_err(),
        "legacy one-field activation bytes must fail closed instead of defaulting a policy",
    );
    crate::isi::read_aos_field(&encoded, &mut legacy_len, flags)
        .expect("read the former device-policy field");
    assert!(legacy_len < encoded.len());
    assert!(
        ActivateKagemushaRecursiveReleaseV4::decode_from_slice(&encoded[..legacy_len]).is_err(),
        "legacy two-field activation bytes must fail closed instead of defaulting a promotion id",
    );

    let packed_flags =
        norito::core::default_encode_flags() | norito::core::header_flags::PACKED_STRUCT;
    let packed = encode_with_flags(&instruction, packed_flags);
    {
        let _flags = norito::core::DecodeFlagsGuard::enter(packed_flags);
        let (decoded, used) = ActivateKagemushaRecursiveReleaseV4::decode_from_slice(&packed)
            .expect("packed activation payload must roundtrip");
        assert_eq!(used, packed.len());
        assert_eq!(decoded, instruction);
    }
    for retired in [
        encode_with_flags(
            &RetiredOneFieldActivation {
                activation: instruction.activation().clone(),
            },
            packed_flags,
        ),
        encode_with_flags(
            &RetiredTwoFieldActivation {
                activation: instruction.activation().clone(),
                device_attestation_policy: policy,
            },
            packed_flags,
        ),
    ] {
        let _flags = norito::core::DecodeFlagsGuard::enter(packed_flags);
        assert!(
            ActivateKagemushaRecursiveReleaseV4::decode_from_slice(&retired).is_err(),
            "packed retired activation layouts must fail closed",
        );
    }
}
