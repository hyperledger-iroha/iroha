// Focused first-release Exact12 release and deployment manifest tests.

use iroha_crypto::{Algorithm, KeyPair, Signature};

fn release_manifest_test_key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("derive deterministic synthetic manifest key")
}

fn release_manifest_test_signature(key: &KeyPair, payload: &[u8]) -> Signature {
    Signature::try_new(key.private_key(), payload).expect("sign synthetic manifest payload")
}

fn release_manifest_placeholder_signature() -> Signature {
    Signature::from_bytes(&[1; 64])
}

#[expect(
    clippy::too_many_lines,
    reason = "the fixture intentionally constructs the complete frozen 12/48/54 matrix"
)]
fn synthetic_valid_release_manifest() -> PrivacyExact12ReleaseManifestV1 {
    let protocols = PrivacyProtocolIdV1::ALL
        .into_iter()
        .enumerate()
        .map(|(index, protocol_id)| {
            let parameter_digest =
                PrivacyParameterDigestV1::new(raw(u8::try_from(index + 1).expect("small index")));
            let verifier_digest =
                PrivacyVerifierDigestV1::new(raw(u8::try_from(index + 21).expect("small index")));
            let security_claim = PrivacySecurityClaimV1 {
                catalog_commitment: PrivacyExact12CatalogCommitmentV1::canonical(),
                protocol_id,
                security_model: protocol_id.security_model(),
                target_security_bits: PRIVACY_MINIMUM_SECURITY_BITS_V1,
                achieved_security_bits: PRIVACY_MINIMUM_SECURITY_BITS_V1,
                parameter_digest,
                verifier_digest,
                reduction_digest: PrivacySecurityReductionDigestV1::new(raw(u8::try_from(
                    index + 41,
                )
                .expect("small index"))),
                audit_bundle_digest: PrivacyAuditBundleDigestV1::new([0; 32]),
            };
            PrivacyReleaseProtocolBindingV1 {
                protocol_id,
                proof_system_id: protocol_id.expected_proof_system(),
                engine_id: protocol_id.expected_engine(),
                parameter_id: PrivacyParameterIdV1::new(raw(
                    u8::try_from(index + 201).expect("small index")
                )),
                parameter_digest,
                verifier_digest,
                statement_schema_digest: PrivacyStatementSchemaDigestV1::new(raw(u8::try_from(
                    index + 61,
                )
                .expect("small index"))),
                engine_manifest_digest: PrivacyEngineManifestDigestV1::new(raw(u8::try_from(
                    index + 81,
                )
                .expect("small index"))),
                security_claim,
                security_claim_digest: PrivacySecurityClaimDigestV1::new([0; 32]),
            }
        })
        .collect::<Vec<_>>();

    let mut stage_receipts = Vec::with_capacity(PRIVACY_EXACT12_RELEASE_STAGE_RECEIPTS_V1);
    let mut proof_artifacts = Vec::with_capacity(PRIVACY_EXACT12_RELEASE_PROOF_ARTIFACTS_V1);
    for (protocol_index, protocol_id) in PrivacyProtocolIdV1::ALL.into_iter().enumerate() {
        let binding = &protocols[protocol_index];
        for (stage_index, stage) in PrivacyReleaseStageV1::ALL.into_iter().enumerate() {
            let stage_ordinal =
                protocol_index * PRIVACY_EXACT12_RELEASE_STAGES_PER_PROTOCOL_V1 + stage_index;
            let receipt_digest = PrivacyReleaseArtifactDigestV1::new(raw(u8::try_from(
                stage_ordinal + 1,
            )
            .expect("48 stages fit u8")));
            stage_receipts.push(PrivacyReleaseStageReceiptV1 {
                stage_ordinal: u16::try_from(stage_ordinal).expect("48 stages fit u16"),
                protocol_id,
                stage,
                security_claim_digest: binding.security_claim_digest,
                parameter_digest: binding.parameter_digest,
                verifier_digest: binding.verifier_digest,
                engine_manifest_digest: binding.engine_manifest_digest,
                receipt_digest,
            });
            for stage_artifact_ordinal in
                0..privacy_exact12_release_proof_artifact_count_v1(protocol_id, stage)
            {
                let artifact_index = proof_artifacts.len();
                proof_artifacts.push(PrivacyReleaseProofArtifactV1 {
                    protocol_id,
                    stage,
                    stage_artifact_ordinal,
                    stage_receipt_digest: receipt_digest,
                    security_claim_digest: binding.security_claim_digest,
                    parameter_digest: binding.parameter_digest,
                    verifier_digest: binding.verifier_digest,
                    engine_manifest_digest: binding.engine_manifest_digest,
                    artifact_digest: PrivacyReleaseArtifactDigestV1::new(raw(u8::try_from(
                        artifact_index + 101,
                    )
                    .expect("54 artifacts fit seed range"))),
                });
            }
        }
    }

    let binary_digest = PrivacyReleaseArtifactDigestV1::new(raw(240));
    let fixture_corpus_digest = PrivacyReleaseArtifactDigestV1::new(raw(159));
    let deterministic_output_digest = PrivacyReleaseArtifactDigestV1::new(raw(176));
    let mut manifest = PrivacyExact12ReleaseManifestV1 {
        version: PRIVACY_EXACT12_RELEASE_MANIFEST_VERSION_V1,
        catalog_id: String::from_utf8(PRIVACY_EXACT12_CATALOG_ID_V1.to_vec())
            .expect("the pinned catalog identity is ASCII"),
        catalog_commitment: PrivacyExact12CatalogCommitmentV1::canonical(),
        source: PrivacyReleaseSourceIdentityV1 {
            source_tree_digest: PrivacyReleaseArtifactDigestV1::new(raw(250)),
            source_tree_clean: true,
            toolchain_id: "synthetic-rust-toolchain".to_owned(),
            toolchain_digest: PrivacyReleaseArtifactDigestV1::new(raw(251)),
            cargo_lock_digest: PrivacyReleaseArtifactDigestV1::new(raw(252)),
        },
        abi_version: PRIVACY_EXACT12_ABI_VERSION_V1,
        abi_hash: PrivacyReleaseArtifactDigestV1::new(raw(253)),
        syscall_list_digest: PrivacyReleaseArtifactDigestV1::new(raw(254)),
        executables: vec![
            PrivacyReleaseExecutableArtifactV1 {
                kind: PrivacyReleaseExecutableKindV1::Binary,
                name: "synthetic-iroha3d".to_owned(),
                artifact_digest: binary_digest,
            },
            PrivacyReleaseExecutableArtifactV1 {
                kind: PrivacyReleaseExecutableKindV1::ContainerImage,
                name: "synthetic-iroha3d-image".to_owned(),
                artifact_digest: PrivacyReleaseArtifactDigestV1::new(raw(241)),
            },
        ],
        protocols,
        stage_receipts,
        proof_artifacts,
        sdk_packages: PrivacyReleaseSdkConsumerV1::ALL
            .into_iter()
            .enumerate()
            .map(|(index, consumer)| PrivacyReleaseSdkPackageV1 {
                consumer,
                package_name: format!("synthetic-sdk-{index}"),
                package_version: "1.0.0-test".to_owned(),
                package_digest: PrivacyReleaseArtifactDigestV1::new(raw(
                    u8::try_from(index + 160).expect("small SDK index")
                )),
                fixture_corpus_digest,
            })
            .collect(),
        hardware_results: PrivacyReleaseHardwareBackendV1::ALL
            .into_iter()
            .enumerate()
            .map(|(index, backend)| PrivacyReleaseHardwareResultV1 {
                backend,
                tested_binary_digest: binary_digest,
                deterministic_output_digest,
                scalar_reference_digest: deterministic_output_digest,
                result_digest: PrivacyReleaseArtifactDigestV1::new(raw(
                    u8::try_from(index + 170).expect("small hardware index")
                )),
                runtime_self_test_passed: true,
            })
            .collect(),
        release_artifact_set_digest: PrivacyReleaseArtifactDigestV1::new([0; 32]),
        audits: Vec::new(),
        audit_bundle_digest: PrivacyAuditBundleDigestV1::new([0; 32]),
        release_signatures: Vec::new(),
        manifest_digest: PrivacyExact12ReleaseManifestDigestV1::new([0; 32]),
    };
    manifest.release_artifact_set_digest = manifest
        .computed_release_artifact_set_digest()
        .expect("digest synthetic artifact set");

    manifest.audits = PrivacyReleaseAuditClassV1::ALL
        .into_iter()
        .enumerate()
        .map(|(index, audit_class)| {
            let auditor =
                release_manifest_test_key(u8::try_from(index + 0x40).expect("small auditor index"));
            let mut disposition = PrivacyAcceptedMediumDispositionV1 {
                finding_digest: PrivacyReleaseArtifactDigestV1::new(raw(
                    u8::try_from(index + 50).expect("small finding index")
                )),
                disposition_digest: PrivacyReleaseArtifactDigestV1::new(raw(u8::try_from(
                    index + 60,
                )
                .expect("small disposition index"))),
                release_artifact_set_digest: manifest.release_artifact_set_digest,
                signature: release_manifest_placeholder_signature(),
            };
            disposition.signature = release_manifest_test_signature(
                &auditor,
                &disposition
                    .signing_bytes()
                    .expect("encode synthetic Medium disposition"),
            );
            let mut audit = PrivacyReleaseAuditV1 {
                audit_class,
                report_digest: PrivacyReleaseArtifactDigestV1::new(raw(
                    u8::try_from(index + 70).expect("small report index")
                )),
                release_artifact_set_digest: manifest.release_artifact_set_digest,
                open_critical_findings: 0,
                open_high_findings: 0,
                accepted_medium_dispositions: vec![disposition],
                auditor: auditor.public_key().clone(),
                signature: release_manifest_placeholder_signature(),
            };
            audit.signature = release_manifest_test_signature(
                &auditor,
                &audit.signing_bytes().expect("encode synthetic audit"),
            );
            audit
        })
        .collect();
    manifest.audit_bundle_digest = manifest
        .computed_audit_bundle_digest()
        .expect("digest synthetic audit bundle");

    for binding in &mut manifest.protocols {
        binding.security_claim.audit_bundle_digest = manifest.audit_bundle_digest;
        binding.security_claim_digest = binding
            .security_claim
            .computed_digest()
            .expect("digest synthetic security claim");
    }
    for receipt in &mut manifest.stage_receipts {
        let protocol_index =
            usize::from(receipt.stage_ordinal) / PRIVACY_EXACT12_RELEASE_STAGES_PER_PROTOCOL_V1;
        receipt.security_claim_digest = manifest.protocols[protocol_index].security_claim_digest;
    }
    for artifact in &mut manifest.proof_artifacts {
        let protocol_index = PrivacyProtocolIdV1::ALL
            .iter()
            .position(|protocol_id| *protocol_id == artifact.protocol_id)
            .expect("closed protocol is present");
        artifact.security_claim_digest = manifest.protocols[protocol_index].security_claim_digest;
    }
    assert_eq!(
        manifest
            .computed_release_artifact_set_digest()
            .expect("redigest synthetic artifact set"),
        manifest.release_artifact_set_digest
    );

    manifest.manifest_digest = manifest
        .computed_manifest_digest()
        .expect("digest synthetic release manifest");
    manifest.release_signatures = PrivacyReleaseSignatureRoleV1::ALL
        .into_iter()
        .enumerate()
        .map(|(index, role)| {
            let signer = release_manifest_test_key(
                u8::try_from(index + 0x50).expect("small release signer index"),
            );
            let payload = PrivacyReleaseSignatureV1::signing_bytes(role, manifest.manifest_digest)
                .expect("encode synthetic release approval");
            PrivacyReleaseSignatureV1 {
                role,
                signer: signer.public_key().clone(),
                signature: release_manifest_test_signature(&signer, &payload),
            }
        })
        .collect();
    manifest
}

fn synthetic_valid_deployment(
    release_manifest_digest: PrivacyExact12ReleaseManifestDigestV1,
) -> PrivacyExact12DeploymentQualificationV1 {
    let network_id = network_id(41);
    let endpoint_version = "privacy-v1".to_owned();
    let converged_state_digest = PrivacyReleaseArtifactDigestV1::new(raw(220));
    let validator_keys = (0_u8..4)
        .map(|index| release_manifest_test_key(0x70 + index))
        .collect::<Vec<_>>();
    let mut deployment = PrivacyExact12DeploymentQualificationV1 {
        version: PRIVACY_EXACT12_DEPLOYMENT_QUALIFICATION_VERSION_V1,
        chain_id: crate::ChainId::from("synthetic-chain"),
        network_id,
        genesis_hash: *network_id.as_bytes(),
        release_manifest_digest,
        activation_transaction_digest: PrivacyReleaseArtifactDigestV1::new(raw(221)),
        activations: PrivacyProtocolIdV1::ALL
            .into_iter()
            .enumerate()
            .map(|(index, protocol_id)| PrivacyDeploymentActivationV1 {
                protocol_id,
                // Catalog order is identity order, not an activation chronology.
                activation_height: 21_u64
                    .checked_sub(u64::try_from(index).expect("small activation index"))
                    .expect("the twelve fixture heights remain positive"),
            })
            .collect(),
        validator_roster_digest: PrivacyReleaseArtifactDigestV1::new([0; 32]),
        endpoint_version: endpoint_version.clone(),
        convergence_height: 200,
        converged_state_digest,
        validator_canaries: validator_keys
            .iter()
            .enumerate()
            .map(|(index, key)| {
                let pre_restart_height = 30 + u64::try_from(index).expect("small index") * 20;
                PrivacyDeploymentValidatorCanaryV1 {
                    validator_index: u16::try_from(index).expect("four validators fit u16"),
                    validator: key.public_key().clone(),
                    rollout_wave: u8::try_from(index + 1).expect("four waves fit u8"),
                    restart_count: 1,
                    pre_restart_height,
                    post_restart_height: pre_restart_height + 1,
                    canary_height: pre_restart_height + 10,
                    canary_digest: PrivacyReleaseArtifactDigestV1::new(raw(u8::try_from(
                        index + 230,
                    )
                    .expect("small canary index"))),
                    converged_state_digest,
                    endpoint_version: endpoint_version.clone(),
                }
            })
            .collect(),
        validator_signatures: Vec::new(),
        qualification_digest: PrivacyExact12DeploymentQualificationDigestV1::new([0; 32]),
    };
    deployment.validator_roster_digest = deployment
        .computed_validator_roster_digest()
        .expect("digest synthetic validator roster");
    deployment.qualification_digest = deployment
        .computed_qualification_digest()
        .expect("digest synthetic deployment qualification");
    deployment.validator_signatures = validator_keys
        .iter()
        .take(PRIVACY_EXACT12_DEPLOYMENT_SIGNATURES_V1)
        .enumerate()
        .map(|(index, key)| {
            let validator_index = u16::try_from(index).expect("three signatures fit u16");
            let payload = PrivacyDeploymentValidatorSignatureV1::signing_bytes(
                validator_index,
                deployment.qualification_digest,
            )
            .expect("encode synthetic deployment approval");
            PrivacyDeploymentValidatorSignatureV1 {
                validator_index,
                signature: release_manifest_test_signature(key, &payload),
            }
        })
        .collect();
    deployment
}

fn synthetic_qualified_capability_rows(
    release: &PrivacyExact12ReleaseManifestV1,
    deployment: &PrivacyExact12DeploymentQualificationV1,
) -> Vec<PrivacyCapabilityRowV1> {
    release
        .protocols
        .iter()
        .zip(&deployment.activations)
        .map(|(binding, deployed)| {
            let protocol_limits = protocol_limits(binding.protocol_id);
            let profile = PrivacyCompiledProfileSnapshotV1 {
                protocol_id: binding.protocol_id,
                proof_system_id: binding.proof_system_id,
                engine_id: binding.engine_id,
                parameter_id: binding.parameter_id,
                parameter_digest: binding.parameter_digest,
                verifier_digest: binding.verifier_digest,
                statement_schema_digest: binding.statement_schema_digest,
                engine_manifest_digest: binding.engine_manifest_digest,
                protocol_limits,
            };
            PrivacyCapabilityRowV1 {
                protocol_id: binding.protocol_id,
                compiled_profile: PrivacyCompiledProfileResultV1::Available(profile),
                activation: Some(PrivacyProtocolActivationRecordV1 {
                    protocol_id: binding.protocol_id,
                    proof_system_id: binding.proof_system_id,
                    engine_id: binding.engine_id,
                    parameter_id: binding.parameter_id,
                    parameter_digest: binding.parameter_digest,
                    verifier_digest: binding.verifier_digest,
                    statement_schema_digest: binding.statement_schema_digest,
                    engine_manifest_digest: binding.engine_manifest_digest,
                    lifecycle: PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
                        proposed_at_height: 1,
                        activated_at_height: deployed.activation_height,
                        state_since_height: deployed.activation_height,
                    }),
                    protocol_limits,
                    pending_protocol_limits_tightening: None,
                }),
            }
        })
        .collect()
}

#[test]
fn release_artifact_digest_is_an_exact_fixed_width_type() {
    let digest = PrivacyReleaseArtifactDigestV1::new(raw(30));
    assert_eq!(digest.as_bytes(), &raw(30));
    assert!(!digest.is_zero());
    assert!(PrivacyReleaseArtifactDigestV1::new([0; 32]).is_zero());
    assert_fixed_width_norito(&digest, &raw(30));
}

#[test]
fn proof_artifact_distribution_is_the_frozen_fifty_four() {
    let mut count = 0_usize;
    for protocol_id in PrivacyProtocolIdV1::ALL {
        for stage in PrivacyReleaseStageV1::ALL {
            count += usize::from(privacy_exact12_release_proof_artifact_count_v1(
                protocol_id,
                stage,
            ));
        }
    }
    assert_eq!(count, PRIVACY_EXACT12_RELEASE_PROOF_ARTIFACTS_V1);
    assert_eq!(
        privacy_exact12_release_proof_artifact_count_v1(
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            PrivacyReleaseStageV1::PositiveCanonicalEndToEnd,
        ),
        2
    );
    assert_eq!(
        privacy_exact12_release_proof_artifact_count_v1(
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            PrivacyReleaseStageV1::PublicStatementBindingMutation,
        ),
        1
    );
    for stage in PrivacyReleaseStageV1::ALL {
        assert_eq!(
            privacy_exact12_release_proof_artifact_count_v1(
                PrivacyProtocolIdV1::IrohaZkAmsV1,
                stage,
            ),
            2
        );
    }
}

#[test]
fn synthetic_release_and_deployment_validate_and_roundtrip() {
    let release = synthetic_valid_release_manifest();
    assert_eq!(release.protocols.len(), 12);
    assert_eq!(release.stage_receipts.len(), 48);
    assert_eq!(release.proof_artifacts.len(), 54);
    assert_eq!(release.validate(), Ok(()));
    let encoded = norito::encode_canonical(&release).expect("encode synthetic release manifest");
    let decoded: PrivacyExact12ReleaseManifestV1 =
        norito::decode_canonical(&encoded).expect("decode synthetic release manifest");
    assert_eq!(decoded, release);

    let deployment = synthetic_valid_deployment(release.manifest_digest);
    assert_eq!(deployment.validator_canaries.len(), 4);
    assert_eq!(deployment.validator_signatures.len(), 3);
    assert_eq!(deployment.validate(), Ok(()));
    let encoded =
        norito::encode_canonical(&deployment).expect("encode synthetic deployment qualification");
    let decoded: PrivacyExact12DeploymentQualificationV1 =
        norito::decode_canonical(&encoded).expect("decode synthetic deployment qualification");
    assert_eq!(decoded, deployment);
}

#[test]
fn exact12_qualification_links_full_manifests_and_all_twelve_activations() {
    let release = synthetic_valid_release_manifest();
    let deployment = synthetic_valid_deployment(release.manifest_digest);
    let qualification = PrivacyExact12QualificationRecordV1 {
        release_manifest: release.clone(),
        deployment_qualification: deployment.clone(),
    };
    let rows = synthetic_qualified_capability_rows(&release, &deployment);
    assert_eq!(qualification.validate(), Ok(()));
    assert_eq!(qualification.validate_against_snapshot(200, &rows), Ok(()));
    assert_eq!(
        qualification.validate_protocol_at_snapshot(200, &rows[0]),
        Ok(())
    );

    let encoded = norito::encode_canonical(&qualification)
        .expect("encode complete Exact12 qualification record");
    let decoded: PrivacyExact12QualificationRecordV1 =
        norito::decode_canonical(&encoded).expect("decode complete Exact12 qualification record");
    assert_eq!(decoded, qualification);
    let mut trailing = encoded;
    trailing.push(0);
    assert!(norito::decode_canonical::<PrivacyExact12QualificationRecordV1>(&trailing).is_err());

    let mismatched = PrivacyExact12QualificationRecordV1 {
        release_manifest: release.clone(),
        deployment_qualification: synthetic_valid_deployment(
            PrivacyExact12ReleaseManifestDigestV1::new(raw(199)),
        ),
    };
    assert_eq!(
        mismatched.validate(),
        Err(PrivacyExact12QualificationRecordValidationErrorV1::ReleaseManifestDigest)
    );

    let mut wrong_height_rows = rows;
    let Some(activation) = &mut wrong_height_rows[0].activation else {
        unreachable!("qualified fixture has every activation")
    };
    let PrivacyProtocolLifecycleV1::Active(lifecycle) = &mut activation.lifecycle else {
        unreachable!("qualified fixture uses active lifecycles")
    };
    lifecycle.activated_at_height += 1;
    lifecycle.state_since_height += 1;
    assert_eq!(
        qualification.validate_against_snapshot(200, &wrong_height_rows),
        Err(PrivacyExact12QualificationRecordValidationErrorV1::ProtocolBinding)
    );
}

#[test]
fn release_rejects_abi_stage_artifact_and_signature_mutations() {
    let release = synthetic_valid_release_manifest();

    let mut bad_abi = release.clone();
    bad_abi.abi_version = 0;
    assert_eq!(
        bad_abi.validate(),
        Err(PrivacyExact12ReleaseManifestValidationErrorV1::AbiBinding)
    );
    let mut zero_abi_hash = release.clone();
    zero_abi_hash.abi_hash = PrivacyReleaseArtifactDigestV1::new([0; 32]);
    assert_eq!(
        zero_abi_hash.validate(),
        Err(PrivacyExact12ReleaseManifestValidationErrorV1::AbiBinding)
    );

    let mut bad_stage = release.clone();
    bad_stage.stage_receipts[0].stage_ordinal = 1;
    assert_eq!(
        bad_stage.validate(),
        Err(PrivacyExact12ReleaseManifestValidationErrorV1::StageReceipt)
    );
    let mut bad_artifact_ordinal = release.clone();
    bad_artifact_ordinal.proof_artifacts[0].stage_artifact_ordinal = 1;
    assert_eq!(
        bad_artifact_ordinal.validate(),
        Err(PrivacyExact12ReleaseManifestValidationErrorV1::ProofArtifact)
    );
    let mut bad_distribution = release.clone();
    bad_distribution.proof_artifacts.swap(4, 5);
    assert_eq!(
        bad_distribution.validate(),
        Err(PrivacyExact12ReleaseManifestValidationErrorV1::ProofArtifact)
    );
    let mut bad_cross_binding = release.clone();
    bad_cross_binding.proof_artifacts[0].stage_receipt_digest =
        release.stage_receipts[1].receipt_digest;
    assert_eq!(
        bad_cross_binding.validate(),
        Err(PrivacyExact12ReleaseManifestValidationErrorV1::ProofArtifact)
    );
    let mut missing_artifact = release.clone();
    missing_artifact.proof_artifacts.pop();
    assert_eq!(
        missing_artifact.validate(),
        Err(PrivacyExact12ReleaseManifestValidationErrorV1::ProofArtifactCount)
    );

    let mut missing_approval = release.clone();
    missing_approval.release_signatures.pop();
    assert_eq!(
        missing_approval.validate(),
        Err(PrivacyExact12ReleaseManifestValidationErrorV1::ReleaseSignatureCount)
    );
    let mut forged_approval = release;
    forged_approval.release_signatures[0].signature = Signature::from_bytes(&[0xA5; 64]);
    assert_eq!(
        forged_approval.validate(),
        Err(PrivacyExact12ReleaseManifestValidationErrorV1::ReleaseSignature)
    );
}

#[test]
fn deployment_rejects_chain_rollout_quorum_and_signature_mutations() {
    let release = synthetic_valid_release_manifest();
    let deployment = synthetic_valid_deployment(release.manifest_digest);

    let mut other_chain = deployment.clone();
    other_chain.chain_id = crate::ChainId::from("other-chain");
    assert_eq!(
        other_chain.validate(),
        Err(PrivacyExact12DeploymentQualificationValidationErrorV1::QualificationDigest)
    );
    let mut wrong_genesis = deployment.clone();
    wrong_genesis.genesis_hash[0] ^= 1;
    assert_eq!(
        wrong_genesis.validate(),
        Err(PrivacyExact12DeploymentQualificationValidationErrorV1::NetworkGenesis)
    );

    let mut out_of_order_wave = deployment.clone();
    out_of_order_wave.validator_canaries[1].pre_restart_height =
        out_of_order_wave.validator_canaries[0].canary_height - 1;
    assert_eq!(
        out_of_order_wave.validate(),
        Err(PrivacyExact12DeploymentQualificationValidationErrorV1::Canary)
    );

    let mut short_quorum = deployment.clone();
    short_quorum.validator_signatures.pop();
    assert_eq!(
        short_quorum.validate(),
        Err(PrivacyExact12DeploymentQualificationValidationErrorV1::SignatureCount)
    );
    let mut duplicated_signer = deployment.clone();
    duplicated_signer.validator_signatures[1].validator_index = 0;
    assert_eq!(
        duplicated_signer.validate(),
        Err(PrivacyExact12DeploymentQualificationValidationErrorV1::Signature)
    );
    let mut forged_signature = deployment;
    forged_signature.validator_signatures[0].signature = Signature::from_bytes(&[0x5A; 64]);
    assert_eq!(
        forged_signature.validate(),
        Err(PrivacyExact12DeploymentQualificationValidationErrorV1::Signature)
    );
}

#[cfg(feature = "json")]
#[test]
fn release_and_deployment_json_roundtrip_and_reject_unknown_fields() {
    let release = synthetic_valid_release_manifest();
    let release_json = norito::json::to_json(&release).expect("serialize release manifest JSON");
    let release_roundtrip: PrivacyExact12ReleaseManifestV1 =
        norito::json::from_json(&release_json).expect("decode canonical release JSON");
    assert_eq!(release_roundtrip, release);
    let hostile_release = release_json.replacen('{', "{\"unexpected\":0,", 1);
    assert!(norito::json::from_json::<PrivacyExact12ReleaseManifestV1>(&hostile_release).is_err());

    let deployment = synthetic_valid_deployment(release.manifest_digest);
    let deployment_json =
        norito::json::to_json(&deployment).expect("serialize deployment qualification JSON");
    let deployment_roundtrip: PrivacyExact12DeploymentQualificationV1 =
        norito::json::from_json(&deployment_json).expect("decode canonical deployment JSON");
    assert_eq!(deployment_roundtrip, deployment);
    let hostile_deployment = deployment_json.replacen('{', "{\"unexpected\":0,", 1);
    assert!(
        norito::json::from_json::<PrivacyExact12DeploymentQualificationV1>(&hostile_deployment)
            .is_err()
    );

    let qualification = PrivacyExact12QualificationRecordV1 {
        release_manifest: release,
        deployment_qualification: deployment,
    };
    let qualification_json =
        norito::json::to_json(&qualification).expect("serialize qualification record JSON");
    let qualification_roundtrip: PrivacyExact12QualificationRecordV1 =
        norito::json::from_json(&qualification_json).expect("decode qualification record JSON");
    assert_eq!(qualification_roundtrip, qualification);
    let hostile_qualification = qualification_json.replacen('{', "{\"unexpected\":0,", 1);
    assert!(
        norito::json::from_json::<PrivacyExact12QualificationRecordV1>(&hostile_qualification)
            .is_err()
    );
}
