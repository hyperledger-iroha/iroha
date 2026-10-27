//! Exact network binding and archive rejection at the privacy JNI boundary.

use super::*;

fn canonical_manifest(network: [u8; 32]) -> PrivacyExact12CapabilityManifestV1 {
    use iroha_data_model::privacy::PrivacyConsensusPolicyV1;
    iroha_core_privacy::privacy_profiles::committed_privacy_capability_snapshot_v1(
        network_id_from_raw_bytes(&network).expect("canonical network"),
        3,
        PrivacyConsensusPolicyV1::taira_default(),
        |_| None,
    )
    .expect("same-network snapshot without a protocol policy")
    .exact12_capability_manifest_v1()
    .expect("manifest")
}
#[test]
fn manifest_genesis_identity_must_match_the_exact_expected_network() {
    let expected = [0xA5; 32];
    let other = [0xA7; 32];
    let manifest = canonical_manifest(expected);
    assert!(java_privacy_manifest_network_matches(&manifest, &expected));
    assert!(!java_privacy_manifest_network_matches(&manifest, &other));
    assert!(!java_privacy_manifest_network_matches(
        &canonical_manifest(other),
        &expected
    ));
    assert!(!java_privacy_manifest_network_matches(
        &manifest,
        &[0xA4; 32]
    ));
    let archive = manifest
        .canonical_bytes()
        .expect("canonical runtime manifest");
    let index = PrivacyProtocolIdV1::VeRangeTransparentRangeV1 as i32;
    assert!(java_privacy_exact12_capability_tuple_admitted(
        &archive, index, &expected
    ));
    assert!(!java_privacy_exact12_capability_tuple_admitted(
        &archive, index, &other
    ));
    assert!(!java_privacy_exact12_capability_tuple_admitted(
        &archive,
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V1 as i32,
        &expected
    ));
    let mut substituted = manifest;
    substituted.network_id = network_id_from_raw_bytes(&other).unwrap();
    let stale_digest = norito::encode_canonical(&substituted).unwrap();
    assert!(!java_privacy_exact12_capability_tuple_admitted(
        &stale_digest,
        index,
        &other
    ));
    substituted.manifest_digest = substituted.computed_manifest_digest().unwrap();
    let foreign = substituted.canonical_bytes().unwrap();
    assert!(!java_privacy_exact12_capability_tuple_admitted(
        &foreign, index, &expected
    ));
}

#[test]
fn tuple_and_construction_reject_malformed_archives_before_admission() {
    assert!(!java_privacy_exact12_capability_tuple_admitted(
        b"archived-shell",
        0,
        &[0xA5; 32]
    ));
    assert!(!java_privacy_exact12_submit_proof_admitted(
        b"archived-shell",
        0,
        b"instruction",
        &[0xA5; 32],
    ));
}

#[test]
fn absent_policy_uses_canonical_limits_and_policy_cannot_enable_unavailable_implementation() {
    use iroha_data_model::privacy::{
        PrivacyCompiledProfileResultV1, PrivacyProtocolLimitsV1, PrivacyProtocolPolicyV1,
    };
    let network = [0xA5; 32];
    let mut manifest = canonical_manifest(network);
    let available = PrivacyProtocolIdV1::VeRangeTransparentRangeV1 as usize;
    assert!(manifest.protocols[available].policy.is_none());
    let archive = manifest.canonical_bytes().unwrap();
    assert!(java_privacy_exact12_capability_tuple_admitted(
        &archive,
        available as i32,
        &network
    ));
    let PrivacyCompiledProfileResultV1::Available(profile) =
        manifest.protocols[available].compiled_profile
    else {
        panic!("actual VeRange profile must be available");
    };
    manifest.protocols[available].policy = Some(PrivacyProtocolPolicyV1 {
        protocol_id: profile.protocol_id,
        current_limits: profile.protocol_limits,
        pending_update: None,
    });
    let unavailable = PrivacyProtocolIdV1::IrohaZkX509StarkP256V1 as usize;
    manifest.protocols[unavailable].policy = Some(PrivacyProtocolPolicyV1 {
        protocol_id: PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
        current_limits: PrivacyProtocolLimitsV1::IrohaZkX509StarkP256V1,
        pending_update: None,
    });
    manifest.manifest_digest = manifest.computed_manifest_digest().unwrap();
    let archive = manifest.canonical_bytes().unwrap();
    assert!(java_privacy_exact12_capability_tuple_admitted(
        &archive,
        available as i32,
        &network
    ));
    assert!(!java_privacy_exact12_capability_tuple_admitted(
        &archive,
        unavailable as i32,
        &network
    ));
}

#[test]
fn generic_construction_without_policy_preserves_committed_protocol_limits() {
    use iroha_data_model::privacy::{
        PrivacyCompiledProfileResultV1, PrivacyProtocolLimitsV1, PrivacyProtocolPolicyV1,
        PrivacyStatementV1, VeRangeLimitsV1, privacy_action_role_samples_v1,
    };
    let network = [0xA5; 32];
    let protocol = PrivacyProtocolIdV1::VeRangeTransparentRangeV1;
    let mut manifest = canonical_manifest(network);
    let index = protocol as usize;
    let PrivacyCompiledProfileResultV1::Available(profile) =
        manifest.protocols[index].compiled_profile
    else {
        panic!("actual VeRange profile must be available");
    };
    // This exercises construction-time structural binding only. The maintained sample
    // proof bytes are not evidence of native cryptographic verification or execution.
    let mut envelope = privacy_action_role_samples_v1()
        .unwrap()
        .into_iter()
        .find(|sample| sample.envelope.protocol_id == protocol)
        .unwrap()
        .envelope;
    let PrivacyStatementV1::VeRangeTransparentRangeV1(statement) = &mut envelope.statement else {
        unreachable!();
    };
    statement.context.network_id = network_id_from_raw_bytes(&network).unwrap();
    statement.context.parameter_id = profile.parameter_id;
    statement.context.parameter_digest = profile.parameter_digest;
    statement.context.verifier_digest = profile.verifier_digest;
    statement.context.statement_schema_digest = profile.statement_schema_digest;
    statement.context.engine_manifest_digest = profile.engine_manifest_digest;
    assert_eq!(statement.aggregation_count, 2);
    envelope.parameter_id = profile.parameter_id;
    envelope.parameter_digest = profile.parameter_digest;
    envelope.verifier_digest = profile.verifier_digest;
    envelope.statement_schema_digest = profile.statement_schema_digest;
    envelope.engine_manifest_digest = profile.engine_manifest_digest;
    envelope.statement_digest = envelope.statement.digest().unwrap();
    let instruction = norito::encode_canonical(&SubmitPrivacyProofV1 { envelope }).unwrap();
    let archive = manifest.canonical_bytes().unwrap();
    assert!(java_privacy_exact12_submit_proof_admitted(
        &archive,
        index as i32,
        &instruction,
        &network
    ));
    manifest.protocols[index].policy = Some(PrivacyProtocolPolicyV1 {
        protocol_id: protocol,
        current_limits: PrivacyProtocolLimitsV1::VeRangeTransparentRangeV1(VeRangeLimitsV1 {
            max_aggregation_count: 1,
        }),
        pending_update: None,
    });
    manifest.manifest_digest = manifest.computed_manifest_digest().unwrap();
    let ffi_check = |manifest: &[u8]| unsafe {
        crate::privacy_construction::iroha_privacy_validate_exact12_submit_proof_construction_v1(
            manifest.as_ptr(),
            manifest.len().try_into().unwrap(),
            index as u32,
            instruction.as_ptr(),
            instruction.len().try_into().unwrap(),
            network.as_ptr(),
            32,
        )
    };
    assert_eq!(
        ffi_check(&archive),
        0,
        "C and JNI accept the same no-policy construction"
    );
    let bounded_archive = manifest.canonical_bytes().unwrap();
    assert_ne!(
        ffi_check(&bounded_archive),
        0,
        "C also enforces the lowered protocol limit"
    );
    assert!(java_privacy_exact12_capability_tuple_admitted(
        &bounded_archive,
        index as i32,
        &network
    ));
    assert!(!java_privacy_exact12_submit_proof_admitted(
        &bounded_archive,
        index as i32,
        &instruction,
        &network
    ));
}
