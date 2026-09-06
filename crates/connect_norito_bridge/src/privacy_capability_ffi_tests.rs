//! Native capability evidence rejection at the foreign-language trust boundary.

use super::*;
use iroha_data_model::{ChainId, privacy::*};

fn unqualified_manifest() -> PrivacyExact12CapabilityManifestV1 {
    PrivacyCapabilitySnapshotV1 {
        version: 1,
        committed_height: 3,
        consensus_policy: PrivacyConsensusPolicyV1::default(),
        qualification: None,
        protocols: PrivacyProtocolIdV1::ALL
            .into_iter()
            .map(|protocol_id| PrivacyCapabilityRowV1 {
                protocol_id,
                compiled_profile: PrivacyCompiledProfileResultV1::Unavailable(
                    PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
                ),
                activation: None,
            })
            .collect(),
    }
    .exact12_capability_manifest_v1()
    .expect("a canonical unavailable snapshot needs no release evidence")
}

fn unsigned_qualification() -> PrivacyExact12QualificationRecordV1 {
    let artifact = PrivacyReleaseArtifactDigestV1::new([0xA1; 32]);
    let audit_bundle_digest = PrivacyAuditBundleDigestV1::new([0xA2; 32]);
    let release_digest = PrivacyExact12ReleaseManifestDigestV1::new([0xA3; 32]);
    let protocols = PrivacyProtocolIdV1::ALL
        .into_iter()
        .map(|protocol_id| {
            let security_claim = PrivacySecurityClaimV1 {
                catalog_commitment: PrivacyExact12CatalogCommitmentV1::canonical(),
                protocol_id,
                security_model: protocol_id.security_model(),
                target_security_bits: 128,
                achieved_security_bits: 128,
                parameter_digest: PrivacyParameterDigestV1::new([0xB1; 32]),
                verifier_digest: PrivacyVerifierDigestV1::new([0xB2; 32]),
                reduction_digest: PrivacySecurityReductionDigestV1::new([0xB3; 32]),
                audit_bundle_digest,
            };
            PrivacyReleaseProtocolBindingV1 {
                protocol_id,
                proof_system_id: protocol_id.expected_proof_system(),
                engine_id: protocol_id.expected_engine(),
                parameter_id: PrivacyParameterIdV1::new([0xB0; 32]),
                parameter_digest: security_claim.parameter_digest,
                verifier_digest: security_claim.verifier_digest,
                statement_schema_digest: PrivacyStatementSchemaDigestV1::new([0xB4; 32]),
                engine_manifest_digest: PrivacyEngineManifestDigestV1::new([0xB5; 32]),
                security_claim_digest: security_claim.computed_digest().expect("claim digest"),
                security_claim,
            }
        })
        .collect();
    PrivacyExact12QualificationRecordV1 {
        release_manifest: PrivacyExact12ReleaseManifestV1 {
            version: 1,
            catalog_id: "iroha-privacy-exact12-v1".to_owned(),
            catalog_commitment: PrivacyExact12CatalogCommitmentV1::canonical(),
            source: PrivacyReleaseSourceIdentityV1 {
                source_tree_digest: artifact,
                source_tree_clean: true,
                toolchain_id: "test-toolchain".to_owned(),
                toolchain_digest: artifact,
                cargo_lock_digest: artifact,
            },
            abi_version: 1,
            abi_hash: artifact,
            syscall_list_digest: PrivacyReleaseArtifactDigestV1::new([0xA4; 32]),
            executables: vec![],
            protocols,
            stage_receipts: vec![],
            proof_artifacts: vec![],
            sdk_packages: vec![],
            hardware_results: vec![],
            release_artifact_set_digest: artifact,
            audits: vec![],
            audit_bundle_digest,
            release_signatures: vec![],
            manifest_digest: release_digest,
        },
        deployment_qualification: PrivacyExact12DeploymentQualificationV1 {
            version: 1,
            chain_id: ChainId::from("test-native-capability-rejection"),
            network_id: NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                Hash::prehashed([0xD0; 32]),
            )),
            genesis_hash: *Hash::prehashed([0xD0; 32]).as_ref(),
            release_manifest_digest: release_digest,
            activation_transaction_digest: artifact,
            activations: PrivacyProtocolIdV1::ALL
                .into_iter()
                .map(|protocol_id| PrivacyDeploymentActivationV1 {
                    protocol_id,
                    activation_height: 2,
                })
                .collect(),
            validator_roster_digest: artifact,
            endpoint_version: "v1".to_owned(),
            convergence_height: 3,
            converged_state_digest: artifact,
            validator_canaries: vec![],
            validator_signatures: vec![],
            qualification_digest: PrivacyExact12DeploymentQualificationDigestV1::new([0xD1; 32]),
        },
    }
}

#[test]
fn native_capability_validator_rejects_unsigned_exact12_evidence() {
    let mut manifest = unqualified_manifest();
    let archive = norito::encode_canonical(&manifest).expect("unqualified archive");
    let validate = |bytes: &[u8]| unsafe {
        iroha_privacy_validate_exact12_capability_manifest_v1(
            bytes.as_ptr(),
            bytes.len() as c_ulong,
        )
    };
    assert_eq!(
        validate(&archive),
        PrivacyCapabilityArchiveValidationStatusV1::Valid.code()
    );

    // Exact twelve bindings, valid per-claim digests, matching release links and
    // recomputed outer framing cannot substitute for any omitted signed evidence.
    manifest.qualification = Some(unsigned_qualification());
    assert_eq!(
        manifest
            .qualification
            .as_ref()
            .expect("unsigned evidence")
            .release_manifest
            .validate(),
        Err(PrivacyExact12ReleaseManifestValidationErrorV1::ExecutableArtifact)
    );
    manifest.manifest_digest = manifest.computed_manifest_digest().expect("outer digest");
    let unsigned = norito::encode_canonical(&manifest).expect("unsigned archive");
    assert_eq!(
        validate(&unsigned),
        PrivacyCapabilityArchiveValidationStatusV1::InvalidManifest.code()
    );
    assert_ne!(validate(&archive[..archive.len() - 1]), 0);
    let mut trailing = archive;
    trailing.push(0);
    assert_ne!(validate(&trailing), 0);
}

#[test]
fn native_capability_validator_enforces_pointer_and_byte_bounds() {
    use PrivacyCapabilityArchiveValidationStatusV1 as Status;
    assert_eq!(
        unsafe { iroha_privacy_validate_exact12_capability_manifest_v1(ptr::null(), 0) },
        Status::NullPointer.code()
    );
    let byte = 0_u8;
    assert_eq!(
        unsafe { iroha_privacy_validate_exact12_capability_manifest_v1(&byte, 0) },
        Status::Empty.code()
    );
    assert_eq!(
        unsafe {
            iroha_privacy_validate_exact12_capability_manifest_v1(
                &byte,
                (PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1 + 1) as c_ulong,
            )
        },
        Status::ArchiveTooLarge.code()
    );
}
