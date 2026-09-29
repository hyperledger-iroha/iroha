//! Signed native consensus metadata and RS16 layout contracts.
use super::*;
use norito::codec::DecodeAll as _;
#[test]
fn consensus_modes_project_canonical_protocol_identities() {
    assert_eq!(ConsensusMode::Permissioned.tag(), PERMISSIONED_TAG);
    assert_eq!(ConsensusMode::Npos.tag(), NPOS_TAG);
    assert_eq!(
        ConsensusMode::Permissioned.bls_domain(),
        PERMISSIONED_BLS_DOMAIN
    );
    assert_eq!(ConsensusMode::Npos.bls_domain(), NPOS_BLS_DOMAIN);
    assert!(ConsensusMode::Permissioned.is_permissioned());
    assert!(!ConsensusMode::Npos.is_permissioned());
    for mode in [ConsensusMode::Permissioned, ConsensusMode::Npos] {
        let parameter_mode = crate::parameter::system::SumeragiConsensusMode::from(mode);
        assert_eq!(ConsensusMode::from(parameter_mode), mode);
    }
}
#[test]
fn payload_encoding_uses_natural_zero_tag_and_rejects_retired_tag_one() {
    let canonical = PayloadEncoding::ReedSolomon16.encode();
    assert_eq!(canonical, 0_u32.to_le_bytes());
    assert_eq!(
        PayloadEncoding::decode_all(&mut canonical.as_slice())
            .expect("decode canonical RS16 payload encoding"),
        PayloadEncoding::ReedSolomon16
    );
    let retired_tag = 1_u32.to_le_bytes();
    assert!(
        PayloadEncoding::decode_all(&mut retired_tag.as_slice()).is_err(),
        "retired payload-encoding tag 1 must fail closed"
    );
}
#[test]
fn payload_encoding_json_rejects_retired_plain_variant() {
    let canonical = norito::json::to_value(&PayloadEncoding::ReedSolomon16)
        .expect("serialize canonical RS16 payload encoding");
    assert_eq!(
        norito::json::from_value::<PayloadEncoding>(canonical.clone())
            .expect("decode canonical RS16 payload encoding"),
        PayloadEncoding::ReedSolomon16
    );
    let mut retired = canonical;
    let encoding = retired
        .as_object_mut()
        .expect("adjacently tagged payload encoding")
        .get_mut("encoding")
        .expect("payload encoding tag");
    assert_eq!(encoding.as_str(), Some("reed_solomon16"));
    *encoding = norito::json::Value::String("plain".to_owned());
    assert!(
        norito::json::from_value::<PayloadEncoding>(retired).is_err(),
        "retired Plain payload encoding must fail closed"
    );
}
#[test]
fn data_availability_layout_enforces_protocol_resource_caps() {
    let maximum = DataAvailabilityLayout {
        encoding: PayloadEncoding::ReedSolomon16,
        chunk_size_bytes: MAX_DA_CHUNK_SIZE_BYTES,
        data_shards: MAX_DA_DATA_SHARDS,
        parity_shards: MAX_DA_PARITY_SHARDS,
        max_payload_size_bytes: MAX_DA_PAYLOAD_SIZE_BYTES,
        max_chunk_count: MAX_DA_CHUNK_COUNT,
    };
    assert_eq!(validate_data_availability_layout(maximum), Ok(()));
    let mut invalid_layouts = Vec::new();
    invalid_layouts.push(DataAvailabilityLayout {
        chunk_size_bytes: MAX_DA_CHUNK_SIZE_BYTES + 2,
        ..maximum
    });
    invalid_layouts.push(DataAvailabilityLayout {
        data_shards: MAX_DA_DATA_SHARDS + 1,
        ..maximum
    });
    invalid_layouts.push(DataAvailabilityLayout {
        parity_shards: MAX_DA_PARITY_SHARDS + 1,
        ..maximum
    });
    invalid_layouts.push(DataAvailabilityLayout {
        max_payload_size_bytes: MAX_DA_PAYLOAD_SIZE_BYTES + 1,
        ..maximum
    });
    invalid_layouts.push(DataAvailabilityLayout {
        max_chunk_count: MAX_DA_CHUNK_COUNT + 1,
        ..maximum
    });
    invalid_layouts.push(DataAvailabilityLayout {
        data_shards: 1,
        parity_shards: 15,
        ..maximum
    });
    invalid_layouts.push(DataAvailabilityLayout {
        data_shards: 1_024,
        parity_shards: 1_024,
        max_chunk_count: u32::MAX,
        ..maximum
    });
    for invalid in invalid_layouts {
        assert_eq!(
            validate_data_availability_layout(invalid),
            Err(ValidationError::InvalidDataAvailabilityLayout)
        );
    }
}
#[test]
fn genesis_context_roundtrips_and_rejects_unbound_policy() {
    let context = SumeragiV2GenesisContextParameters::recommended();
    assert_eq!(context.validate(), Ok(()));
    assert_eq!(
        SumeragiV2GenesisContextParameters::decode_all(&mut context.encode().as_slice()).unwrap(),
        context
    );
    let mut invalid = context;
    invalid.nexus_amx_context_hash = [0; 32];
    assert_eq!(
        invalid.validate(),
        Err(ValidationError::InvalidNexusAmxContextHash)
    );
    invalid = context;
    invalid.execution_policy_hash = [0; 32];
    assert_eq!(
        invalid.validate(),
        Err(ValidationError::InvalidExecutionPolicyHash)
    );
}
#[test]
fn committees_require_exact_three_f_plus_one_geometry() {
    for n in 0..=40 {
        assert_eq!(
            is_valid_committee_size(n),
            (4..=31).contains(&n) && n % 3 == 1
        );
    }
}
