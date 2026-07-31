// Privacy-release evidence regression tests.
//
// Included by `privacy_release_evidence::tests` to preserve exact libtest names.

    use iroha_primitives::json::Json;

    use super::*;
    use crate::privacy_engines::vega::{
        build_signed_vega_privacy_action_with_rng_v1, sign_prepared_vega_privacy_action_v1,
    };

    const RAYON_POOL_CHILD_MARKER_V1: &str = "IROHA_PRIVACY_RELEASE_RAYON_POOL_CHILD_V1";

    #[test]
    fn zk_ams_release_lineage_uses_distinct_single_action_transactions() {
        let admission =
            zk_ams_admission_transaction_context_v1().expect("admission transaction context");
        let provision =
            zk_ams_provision_transaction_context_v1().expect("provision transaction context");

        assert_eq!(ZK_AMS_RELEASE_ADMISSION_ACTION_INDEX_V1, 0);
        assert_eq!(ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1, 0);
        assert_eq!(admission.chain_id, provision.chain_id);
        assert_eq!(admission.authority, provision.authority);
        assert_eq!(admission.time_to_live, provision.time_to_live);
        assert_eq!(admission.fee_payment, provision.fee_payment);
        assert_eq!(admission.metadata, provision.metadata);
        assert!(
            admission.creation_time < provision.creation_time,
            "admission must precede provisioning"
        );
        assert!(
            admission.nonce.expect("admission nonce") < provision.nonce.expect("provision nonce"),
            "sequential transactions require ordered nonces"
        );
    }

    #[test]
    fn zk_ams_release_envelope_distinguishes_admission_from_native_rejection() {
        let ring = zk_ams_sorted_ring_v1(ZK_AMS_MIN_RING_SIZE_V1).expect("canonical minimum ring");
        let key_image = zk_ams_key_image_v1(&ring[5].1).expect("canonical key image");
        let statement = zk_ams_provision_statement_v1(
            &ring,
            key_image,
            PrivacyRootV1::new([0x41; 32]),
            2,
            PrivacyZkAmsRegistryRecordDigestV1::new([0x42; 32]),
        )
        .expect("canonical provisioning statement");
        let authoritative_chain_id = ChainId::from(ZK_AMS_RELEASE_CHAIN_ID_V1);

        assert_eq!(
            verify_zk_ams_release_production_envelope_v1(
                &statement,
                &[0x01],
                &authoritative_chain_id,
                ZK_AMS_RELEASE_GENESIS_HASH_V1,
                ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1,
            ),
            Err(PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected),
            "a canonical one-action envelope must reach the native ZK-AMS verifier"
        );

        let mut impossible_second_action = statement;
        impossible_second_action.context.action_index = 1;
        assert_eq!(
            verify_zk_ams_release_production_envelope_v1(
                &impossible_second_action,
                &[0x01],
                &authoritative_chain_id,
                ZK_AMS_RELEASE_GENESIS_HASH_V1,
                ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1,
            ),
            Err(PrivacyReleaseEvidenceErrorClassV1::ProductionEnvelopeRejected),
            "Taira's one-action transaction limit must reject before native verification"
        );
    }

    #[test]
    fn vega_release_fixture_uses_the_canonical_single_taira_action() {
        let fixture = vega_release_fixture_v1().expect("canonical Vega release fixture");
        let transaction =
            vega_release_transaction_context_v1().expect("canonical Vega transaction context");
        let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0)
            .expect("compiled Vega profile");
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let context = PrivacyStatementContextV1 {
            chain_id: transaction.chain_id.clone(),
            action_index: VEGA_RELEASE_ACTION_INDEX_V1,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0x27; 32]),
            parameter_id: profile.parameter_id,
            parameter_digest: profile.parameter_digest,
            verifier_digest: profile.verifier_digest,
            statement_schema_digest: profile.statement_schema_digest,
            engine_manifest_digest: profile.engine_manifest_digest,
        };

        fixture
            .public_input
            .issuer_record
            .validate()
            .expect("canonical active Vega issuer record");
        context
            .validate(&limits)
            .expect("Vega is the sole privacy action in its transaction");
        assert_eq!(VEGA_RELEASE_ACTION_INDEX_V1, 0);
        assert_eq!(
            transaction.chain_id,
            ChainId::from(VEGA_RELEASE_CHAIN_ID_V1)
        );
        assert_eq!(
            transaction.creation_time,
            Duration::from_millis(VEGA_RELEASE_CREATION_TIME_MS_V1)
        );
        assert_eq!(transaction.nonce, NonZeroU32::new(VEGA_RELEASE_NONCE_V1));

        let mut impossible_second_action = context;
        impossible_second_action.action_index = 1;
        assert!(matches!(
            impossible_second_action.validate(&limits),
            Err(
                iroha_data_model::privacy::PrivacyStatementValidationError::ActionIndexOutOfBounds {
                    index: 1,
                    max_actions: 1,
                }
            )
        ));
    }

    #[test]
    #[ignore = "release gate: proves the full native Vega Figure 9 action once"]
    fn vega_action_api_binds_signs_and_rejects_transaction_proof_and_statement_drift() {
        let fixture = vega_release_fixture_v1().expect("canonical Vega release fixture");
        let witness_material = VegaPrivacyActionWitnessMaterialV1::new(
            fixture.issuer_authentication_sig_structure.clone(),
            fixture.mobile_security_object_payload.clone(),
            fixture.birth_date_issuer_signed_item.clone(),
            &fixture.issuer_signature.to_bytes(),
        )
        .expect("canonical Vega action witness material");
        let mut rng = EvidenceRng06::new([0x91; 32]);
        let prepared = prepare_vega_privacy_action_with_rng_v1(
            vega_release_transaction_context_v1().expect("canonical transaction context"),
            fixture.public_input,
            witness_material,
            &fixture.device_signing_key,
            fixture.genesis_hash,
            VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
            &mut rng,
        )
        .expect("canonical two-pass Vega action");
        assert_ne!(prepared.transaction_intent_digest(), [0; 32]);
        assert_ne!(prepared.statement_digest(), [0; 32]);
        assert_ne!(prepared.proof_envelope_hash(), [0; 32]);
        assert_eq!(
            prepared.effect(),
            crate::privacy_engines::vega::VegaPrivacyActionEffectV1::
                ActionVerificationAndFinalityOnly
        );
        let prepared_debug = format!("{prepared:?}");
        assert!(!prepared_debug.contains("TransactionPayload"));
        assert!(!prepared_debug.contains("PrivacyProofBytes"));
        assert!(!prepared_debug.contains("issuer_authentication_sig_structure"));

        let payload = prepared.release_evidence_payload_v1().clone();
        match payload.instructions() {
            iroha_data_model::transaction::Executable::Instructions(instructions) => {
                assert_eq!(instructions.len(), 1, "exactly one direct Vega action");
                assert!(
                    instructions[0]
                        .as_any()
                        .downcast_ref::<SubmitPrivacyProofV1>()
                        .is_some(),
                    "the sole action must be the typed Vega submission"
                );
            }
            other => panic!("unexpected Vega executable form: {other:?}"),
        }
        assert!(
            payload.attachments.is_none(),
            "canonical Vega actions cannot carry proof attachments"
        );
        let (intent, submission) = payload
            .privacy_transaction_intent_binding_if_present_v1()
            .expect("canonical direct privacy scan")
            .expect("exactly one Vega submission");
        assert_eq!(intent.as_bytes(), &prepared.transaction_intent_digest());
        let PrivacyStatementV1::VegaExistingCredentialZkV0(statement) =
            &submission.envelope.statement
        else {
            panic!("prepared Vega statement changed variant")
        };
        let PrivacyProofV1::VegaExistingCredentialZkV0(proof) = &submission.envelope.proof else {
            panic!("prepared Vega proof changed variant")
        };
        assert_eq!(statement.context.action_index, VEGA_PRIVACY_ACTION_INDEX_V1);
        assert!(!proof.as_bytes().is_empty());
        assert_eq!(
            prepared.statement_bytes(),
            u32::try_from(
                norito::to_bytes(&submission.envelope.statement)
                    .expect("typed Vega statement encodes")
                    .len()
            )
            .expect("bounded Vega statement")
        );
        assert_eq!(
            prepared.proof_bytes(),
            u32::try_from(proof.as_bytes().len()).expect("bounded Vega proof")
        );
        let encoded_envelope =
            norito::to_bytes(&submission.envelope).expect("typed Vega envelope encodes");
        assert_eq!(
            prepared.encoded_proof_envelope_bytes(),
            u32::try_from(encoded_envelope.len()).expect("bounded Vega envelope")
        );
        assert_eq!(
            prepared.proof_envelope_hash(),
            *iroha_crypto::Hash::new(&encoded_envelope).as_ref()
        );
        submission
            .envelope
            .validate_with_limits(&PrivacyConsensusLimitsV1::taira_default())
            .expect("prepared envelope is intrinsically valid");
        let mut proof_empty_escape = submission.envelope.clone();
        proof_empty_escape.proof =
            PrivacyProofV1::VegaExistingCredentialZkV0(PrivacyProofBytesV1::new(Vec::new()));
        assert!(
            proof_empty_escape
                .validate_with_limits(&PrivacyConsensusLimitsV1::taira_default())
                .is_err(),
            "the internal proof-empty projection must never be submittable"
        );

        let mut changed_chain = payload.clone();
        changed_chain.chain = ChainId::from("vega-signed-action-wrong-chain-v1");
        assert!(
            changed_chain
                .validate_privacy_transaction_intent_binding_v1()
                .is_err(),
            "chain mutation must invalidate the signed intent"
        );
        let mut changed_authority = payload.clone();
        changed_authority.authority =
            privacy_release_account_v1(0x57).expect("fixed alternate authority");
        assert!(
            changed_authority
                .validate_privacy_transaction_intent_binding_v1()
                .is_err(),
            "authority mutation must invalidate the signed intent"
        );
        let mut changed_creation_time = payload.clone();
        changed_creation_time.creation_time_ms += 1;
        assert!(
            changed_creation_time
                .validate_privacy_transaction_intent_binding_v1()
                .is_err(),
            "creation-time mutation must invalidate the signed intent"
        );
        let mut changed_fee = payload.clone();
        changed_fee.fee_payment =
            FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(6_000_000));
        assert!(
            changed_fee
                .validate_privacy_transaction_intent_binding_v1()
                .is_err(),
            "fee mutation must invalidate the signed intent"
        );
        let mut changed_ttl = payload.clone();
        changed_ttl.time_to_live_ms = NonZeroU64::new(61_000);
        assert!(
            changed_ttl
                .validate_privacy_transaction_intent_binding_v1()
                .is_err(),
            "TTL mutation must invalidate the signed intent"
        );
        let mut changed_nonce = payload.clone();
        changed_nonce.nonce = NonZeroU32::new(VEGA_RELEASE_NONCE_V1 + 1);
        assert!(
            changed_nonce
                .validate_privacy_transaction_intent_binding_v1()
                .is_err(),
            "nonce mutation must invalidate the signed intent"
        );
        let mut changed_metadata = payload.clone();
        changed_metadata.metadata.insert(
            "vega_intent_mutation"
                .parse()
                .expect("canonical metadata key"),
            Json::new(1_u32),
        );
        assert!(
            changed_metadata
                .validate_privacy_transaction_intent_binding_v1()
                .is_err(),
            "metadata mutation must invalidate the signed intent"
        );

        let binding =
            VegaMdlConsensusBindingV1::from_context(&statement.context, fixture.genesis_hash);
        let mut changed_proof = proof.as_bytes().to_vec();
        let changed_proof_index = changed_proof.len() / 2;
        changed_proof[changed_proof_index] ^= 1;
        assert!(
            verify_mdl_figure9_v1(
                statement,
                &binding,
                VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
                &changed_proof,
            )
            .is_err(),
            "proof drift must fail native verification"
        );
        let mut changed_statement = statement.clone();
        changed_statement.minimum_age_years += 1;
        refresh_vega_device_authentication_digest_v1(&mut changed_statement, fixture.genesis_hash)
            .expect("mutated statement has canonical H_dev");
        let changed_binding = VegaMdlConsensusBindingV1::from_context(
            &changed_statement.context,
            fixture.genesis_hash,
        );
        assert!(
            verify_mdl_figure9_v1(
                &changed_statement,
                &changed_binding,
                VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
                proof.as_bytes(),
            )
            .is_err(),
            "statement drift must fail native verification"
        );
        let mut impossible_second_action = statement.clone();
        impossible_second_action.context.action_index = 1;
        assert!(matches!(
            PrivacyStatementV1::VegaExistingCredentialZkV0(impossible_second_action)
                .validate(&PrivacyConsensusLimitsV1::taira_default()),
            Err(
                iroha_data_model::privacy::PrivacyStatementValidationError::ActionIndexOutOfBounds {
                    index: 1,
                    max_actions: 1,
                }
            )
        ));

        let transaction_key_pair = KeyPair::try_from_seed(vec![0x56; 32], Algorithm::Ed25519)
            .expect("fixed Vega transaction key");
        let expected_intent = prepared.transaction_intent_digest();
        let signed =
            sign_prepared_vega_privacy_action_v1(prepared, transaction_key_pair.private_key())
                .expect("sign sealed Vega action");
        signed
            .signed_transaction()
            .verify_signature()
            .expect("signed Vega transaction verifies");
        assert_eq!(signed.transaction_intent_digest(), expected_intent);
        assert_eq!(
            signed.transaction_hash(),
            *signed.signed_transaction().hash().as_ref()
        );
        assert!(
            signed.signed_transaction().attachments().is_none(),
            "signed canonical Vega actions cannot carry attachments"
        );
        let signed_debug = format!("{signed:?}");
        assert!(!signed_debug.contains("SignedTransaction {"));
        assert!(!signed_debug.contains("PrivacyProofBytes"));
        let mut signed_intent_drift = signed.signed_transaction().payload().clone();
        signed_intent_drift.nonce = NonZeroU32::new(VEGA_RELEASE_NONCE_V1 + 2);
        let independently_resigned_drift = TransactionBuilder::from_payload(signed_intent_drift)
            .expect("otherwise canonical drifted payload")
            .try_sign(transaction_key_pair.private_key())
            .expect("transaction signature covers the drifted payload");
        independently_resigned_drift
            .verify_signature()
            .expect("drifted payload has an independently valid transaction signature");
        assert!(
            independently_resigned_drift
                .privacy_transaction_intent_binding_if_present_v1()
                .is_err(),
            "a valid transaction signature cannot redeem a stale Vega intent"
        );

        let wrong_key_fixture =
            vega_release_fixture_v1().expect("second canonical Vega release fixture");
        let wrong_key_material = VegaPrivacyActionWitnessMaterialV1::new(
            wrong_key_fixture
                .issuer_authentication_sig_structure
                .clone(),
            wrong_key_fixture.mobile_security_object_payload.clone(),
            wrong_key_fixture.birth_date_issuer_signed_item.clone(),
            &wrong_key_fixture.issuer_signature.to_bytes(),
        )
        .expect("canonical wrong-key witness material");
        let foreign_key_pair = KeyPair::try_from_seed(vec![0x57; 32], Algorithm::Ed25519)
            .expect("fixed foreign transaction key");
        let wrong_key = build_signed_vega_privacy_action_with_rng_v1(
            vega_release_transaction_context_v1().expect("canonical transaction context"),
            wrong_key_fixture.public_input,
            wrong_key_material,
            &wrong_key_fixture.device_signing_key,
            wrong_key_fixture.genesis_hash,
            VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
            foreign_key_pair.private_key(),
            &mut EvidenceRng06::new([0x92; 32]),
        );
        assert!(matches!(
            wrong_key,
            Err(crate::privacy_engines::vega::VegaPrivacyActionBuildErrorV1::AuthorityKeyMismatch)
        ));
    }

    #[test]
    fn canonical_process_profile_is_exact_and_has_one_authoritative_source() {
        let profiles = PrivacyProtocolIdV1::ALL
            .into_iter()
            .filter_map(privacy_release_process_profile_v1)
            .collect::<Vec<_>>();
        assert_eq!(
            profiles,
            vec![PrivacyReleaseProcessProfileV1 {
                protocol_id: PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
                elapsed_ceiling_millis: 300_000,
                peak_rss_ceiling_bytes: 12_884_901_888,
            }]
        );
        assert_eq!(
            profiles[0].elapsed_ceiling_millis,
            ZK_X509_PROVER_TARGET_SECONDS_V1 * 1_000
        );
        assert_eq!(
            profiles[0].peak_rss_ceiling_bytes,
            ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1
        );
    }

    #[test]
    fn privacy_release_rayon_pool_fresh_process_child_v1() {
        if std::env::var_os(RAYON_POOL_CHILD_MARKER_V1).is_none() {
            return;
        }
        assert_eq!(PRIVACY_RELEASE_RAYON_THREAD_COUNT_V1, 4);
        initialize_privacy_release_rayon_pool_v1().expect("initialize exact release Rayon pool");
        assert_eq!(
            rayon::current_num_threads(),
            usize::from(PRIVACY_RELEASE_RAYON_THREAD_COUNT_V1)
        );
        assert_eq!(
            initialize_privacy_release_rayon_pool_v1(),
            Err(PrivacyReleaseRayonPoolErrorV1::InitializationRejected),
            "a second global-pool initialization must fail closed"
        );
    }

    #[test]
    fn privacy_release_rayon_pool_is_one_time_and_exact_at_api_boundary_v1() {
        let executable = std::env::current_exe().expect("resolve core unit-test executable");
        let output = std::process::Command::new(executable)
            .arg("privacy_release_rayon_pool_fresh_process_child_v1")
            .arg("--nocapture")
            .env(RAYON_POOL_CHILD_MARKER_V1, "1")
            .output()
            .expect("execute release Rayon API child");
        assert!(
            output.status.success(),
            "release Rayon API child failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn frozen_stage_order_is_explicit_and_matches_the_enum_product() {
        assert!(validate_privacy_release_stage_coordinates_v1(
            &PRIVACY_RELEASE_STAGE_COORDINATES_V1
        ));
        let mut observed = Vec::new();
        for protocol_id in PrivacyProtocolIdV1::ALL {
            for case_kind in PrivacyReleaseCaseKindV1::ALL {
                observed.push(privacy_release_stage_ordinal_v1(protocol_id, case_kind));
            }
        }
        assert_eq!(observed.len(), PRIVACY_RELEASE_STAGE_COUNT_V1);
        assert_eq!(
            observed,
            (0..u16::try_from(PRIVACY_RELEASE_STAGE_COUNT_V1).unwrap()).collect::<Vec<_>>()
        );
        assert_eq!(
            PRIVACY_RELEASE_STAGE_COORDINATES_V1
                .map(|coordinate| coordinate.stage_ordinal)
                .to_vec(),
            observed
        );
    }

    #[test]
    fn resource_facts_are_frozen_for_available_stages_and_x509_remains_pending() {
        for protocol_id in PrivacyProtocolIdV1::ALL {
            for case_kind in PrivacyReleaseCaseKindV1::ALL {
                let facts = privacy_release_resource_facts_v1(protocol_id, case_kind);
                if protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 {
                    assert_eq!(facts, None);
                    assert_eq!(
                        run_privacy_release_stage_v1(protocol_id, case_kind),
                        Err(PrivacyReleaseEvidenceErrorV1 {
                            protocol_id,
                            case_kind,
                            class: PrivacyReleaseEvidenceErrorClassV1::ProtocolUnavailable,
                        })
                    );
                } else {
                    let facts = facts.expect("implemented stage has frozen resource facts");
                    assert!(facts.validate());
                    if case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource {
                        assert_eq!(facts.primary_units, facts.primary_ceiling);
                        assert_eq!(facts.secondary_units, facts.secondary_ceiling);
                        assert_eq!(facts.relation_depth, facts.relation_depth_ceiling);
                    }
                }
            }
        }
    }

    #[test]
    fn exact_parsers_reject_aliases_and_case_folding() {
        for case_kind in PrivacyReleaseCaseKindV1::ALL {
            assert_eq!(
                PrivacyReleaseCaseKindV1::from_canonical_label(case_kind.canonical_label()),
                Some(case_kind)
            );
        }
        assert_eq!(
            PrivacyReleaseCaseKindV1::from_canonical_label("Positive-Canonical-End-To-End"),
            None
        );
        assert_eq!(
            PrivacyReleaseCaseKindV1::from_canonical_label("positive-canonical-end-to-end "),
            None
        );
        assert_eq!(
            PrivacyReleaseCaseKindV1::from_canonical_label("positive"),
            None
        );
    }

    #[test]
    fn evidence_seeds_are_deterministic_and_purpose_separated() {
        let case_kind = PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation;
        let purposes: [&[u8]; 6] = [
            b"canonical-fixture-keygen",
            b"canonical-fixture-encryption",
            b"canonical-proof",
            b"invalid-path-fixture-keygen",
            b"invalid-path-fixture-encryption",
            b"invalid-path-proof",
        ];
        for protocol_id in [
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            PrivacyProtocolIdV1::PqMaspStarkV0,
        ] {
            let seeds = purposes
                .iter()
                .map(|purpose| {
                    stage_purpose_seed_v1(protocol_id, case_kind, purpose)
                        .expect("fixed evidence purpose derives a seed")
                })
                .collect::<Vec<_>>();
            for (index, seed) in seeds.iter().enumerate() {
                assert_eq!(
                    *seed,
                    stage_purpose_seed_v1(protocol_id, case_kind, purposes[index])
                        .expect("same purpose derives the same seed")
                );
                for other in &seeds[index + 1..] {
                    assert_ne!(seed, other);
                }
            }
        }
        assert_ne!(
            stage_purpose_seed_v1(
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                case_kind,
                b"canonical-proof",
            )
            .expect("IVM proof seed"),
            stage_purpose_seed_v1(
                PrivacyProtocolIdV1::PqMaspStarkV0,
                case_kind,
                b"canonical-proof",
            )
            .expect("PQ-MASP proof seed"),
        );
    }

    #[test]
    fn unavailable_protocols_fail_closed_without_placeholder_evidence() {
        let error = run_privacy_release_stage_v1(
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd,
        )
        .expect_err("incomplete X.509 release fixture must fail closed");
        assert_eq!(
            error.class,
            PrivacyReleaseEvidenceErrorClassV1::ProtocolUnavailable
        );
    }

    #[test]
    fn maximum_fixture_dimensions_equal_governed_first_release_caps() {
        assert_eq!(BOOTLE_LANTERN_MAX_ALLOWED_VALUES_PER_ATTRIBUTE_V1, 32);
        assert_eq!(ZK_AMS_MAX_RING_SIZE_V1, 64);
        assert_eq!(ZK_AMS_MAX_ADMISSION_BATCH_SIZE_V1, 8);
        assert_eq!(ORCHARD_MAX_ACTIONS_V1, 2);
        assert_eq!(ORCHARD_TREE_DEPTH_V1, 32);

        let orchard = orchard_maximum_spend_fixture_v1()
            .expect("maximum Orchard fixture has two shared-anchor real spends");
        assert_eq!(orchard.spends.len(), ORCHARD_MAX_ACTIONS_V1);
        assert_eq!(orchard.total_value, 36);
        assert_ne!(orchard.anchor, orchard_empty_root_v1());
    }

    #[test]
    fn ordered_proof_artifact_cardinality_is_closed_and_fail_closed() {
        let artifact = |protocol_id: PrivacyProtocolIdV1,
                        case_kind: PrivacyReleaseCaseKindV1,
                        artifact_ordinal: u8| {
            let canonical_proof_bytes =
                vec![artifact_ordinal.saturating_add(1); usize::from(artifact_ordinal) + 1];
            PrivacyReleaseProofArtifactEvidenceV1 {
                artifact_ordinal,
                proof_sha256: sha256_v1(&canonical_proof_bytes),
                canonical_proof_bytes,
                proof_bytes_ceiling: privacy_release_proof_artifact_ceiling_v1(
                    protocol_id,
                    case_kind,
                    artifact_ordinal,
                )
                .expect("valid fixture artifact has a canonical ceiling"),
            }
        };
        let ordinary_protocol = PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1;
        let ordinary_case = PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd;
        let pgc_protocol = PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1;
        let zk_ams_protocol = PrivacyProtocolIdV1::IrohaZkAmsV1;
        let maximum_case = PrivacyReleaseCaseKindV1::MaximumShapeResource;
        let adversarial_case = PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation;
        assert_eq!(
            privacy_release_proof_artifact_count_v1(ordinary_protocol, ordinary_case),
            1
        );
        assert_eq!(
            privacy_release_proof_artifact_count_v1(pgc_protocol, maximum_case),
            2
        );
        assert_eq!(
            privacy_release_proof_artifact_count_v1(pgc_protocol, ordinary_case),
            2
        );
        assert_eq!(
            privacy_release_proof_artifact_count_v1(zk_ams_protocol, maximum_case),
            2
        );
        assert_eq!(
            privacy_release_proof_artifact_count_v1(zk_ams_protocol, ordinary_case),
            2
        );
        assert_eq!(
            privacy_release_proof_artifact_count_v1(zk_ams_protocol, adversarial_case),
            2
        );
        assert!(validate_privacy_release_proof_artifacts_v1(
            ordinary_protocol,
            ordinary_case,
            &[artifact(ordinary_protocol, ordinary_case, 0)],
        ));
        assert!(validate_privacy_release_proof_artifacts_v1(
            pgc_protocol,
            maximum_case,
            &[
                artifact(pgc_protocol, maximum_case, 0),
                artifact(pgc_protocol, maximum_case, 1),
            ],
        ));
        assert!(validate_privacy_release_proof_artifacts_v1(
            pgc_protocol,
            ordinary_case,
            &[
                artifact(pgc_protocol, ordinary_case, 0),
                artifact(pgc_protocol, ordinary_case, 1),
            ],
        ));
        assert!(validate_privacy_release_proof_artifacts_v1(
            zk_ams_protocol,
            ordinary_case,
            &[
                artifact(zk_ams_protocol, ordinary_case, 0),
                artifact(zk_ams_protocol, ordinary_case, 1),
            ],
        ));
        assert!(validate_privacy_release_proof_artifacts_v1(
            zk_ams_protocol,
            adversarial_case,
            &[
                artifact(zk_ams_protocol, adversarial_case, 0),
                artifact(zk_ams_protocol, adversarial_case, 1),
            ],
        ));
        assert_eq!(
            privacy_release_proof_artifact_ceiling_v1(pgc_protocol, ordinary_case, 0),
            u64::try_from(MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1).ok()
        );
        assert_eq!(
            privacy_release_proof_artifact_ceiling_v1(pgc_protocol, ordinary_case, 1),
            u64::try_from(MAX_PGC_PAYMENT_PROOF_BYTES_V1).ok()
        );
        assert_eq!(
            privacy_release_proof_artifact_ceiling_v1(zk_ams_protocol, ordinary_case, 0),
            u64::try_from(MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1).ok()
        );
        assert_eq!(
            privacy_release_proof_artifact_ceiling_v1(zk_ams_protocol, ordinary_case, 1),
            u64::try_from(MAX_ZK_AMS_LSAG_PROOF_BYTES_V1).ok()
        );
        assert_eq!(
            privacy_release_proof_artifact_ceiling_v1(zk_ams_protocol, adversarial_case, 0),
            u64::try_from(MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1).ok()
        );
        assert_eq!(
            privacy_release_proof_artifact_ceiling_v1(zk_ams_protocol, adversarial_case, 1),
            u64::try_from(MAX_ZK_AMS_LSAG_PROOF_BYTES_V1).ok()
        );

        let valid = artifact(ordinary_protocol, ordinary_case, 0);
        let mut hash_mismatch = valid.clone();
        hash_mismatch.proof_sha256[0] ^= 1;
        let mut empty = valid.clone();
        empty.canonical_proof_bytes.clear();
        empty.proof_sha256 = sha256_v1(&empty.canonical_proof_bytes);
        let mut over_ceiling = valid.clone();
        over_ceiling.canonical_proof_bytes = vec![
            7;
            usize::try_from(over_ceiling.proof_bytes_ceiling)
                .expect("FCMP++ ceiling fits usize")
                + 1
        ];
        over_ceiling.proof_sha256 = sha256_v1(&over_ceiling.canonical_proof_bytes);
        let mut zero_ceiling = valid.clone();
        zero_ceiling.proof_bytes_ceiling = 0;
        let mut substituted_ceiling = valid.clone();
        substituted_ceiling.proof_bytes_ceiling = substituted_ceiling
            .proof_bytes_ceiling
            .checked_sub(1)
            .expect("FCMP++ ceiling is nonzero");
        let mut unbounded_ceiling = valid.clone();
        unbounded_ceiling.proof_bytes_ceiling = PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1 + 1;
        let mut byte_mutation = valid.clone();
        byte_mutation.canonical_proof_bytes[0] ^= 1;
        let malformed = [
            Vec::new(),
            vec![valid.clone(), valid.clone()],
            vec![hash_mismatch],
            vec![empty],
            vec![over_ceiling],
            vec![zero_ceiling],
            vec![substituted_ceiling],
            vec![unbounded_ceiling],
            vec![byte_mutation],
        ];
        for artifacts in malformed {
            assert!(!validate_privacy_release_proof_artifacts_v1(
                ordinary_protocol,
                ordinary_case,
                &artifacts,
            ));
        }
        let pgc_artifact_zero = artifact(pgc_protocol, maximum_case, 0);
        let pgc_artifact_one = artifact(pgc_protocol, maximum_case, 1);
        for artifacts in [
            vec![pgc_artifact_zero.clone()],
            vec![pgc_artifact_one.clone(), pgc_artifact_zero.clone()],
            vec![pgc_artifact_zero.clone(), pgc_artifact_zero.clone()],
            vec![
                pgc_artifact_zero.clone(),
                PrivacyReleaseProofArtifactEvidenceV1 {
                    artifact_ordinal: 2,
                    ..pgc_artifact_one.clone()
                },
            ],
            vec![
                pgc_artifact_zero.clone(),
                pgc_artifact_one.clone(),
                pgc_artifact_one,
            ],
        ] {
            assert!(!validate_privacy_release_proof_artifacts_v1(
                pgc_protocol,
                maximum_case,
                &artifacts,
            ));
        }
    }

    #[test]
    fn proof_artifact_consensus_cap_is_exact_and_cap_plus_one_rejects() {
        assert_eq!(
            PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1,
            u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
        );
        assert_eq!(PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1, 9 * 1024 * 1024);
        let protocol_id = PrivacyProtocolIdV1::PqMaspStarkV0;
        let case_kind = PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd;
        let ceiling = privacy_release_proof_artifact_ceiling_v1(protocol_id, case_kind, 0)
            .expect("PQ-MASP stage has one canonical ceiling");
        assert_eq!(ceiling, PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1);

        let canonical_proof_bytes =
            vec![0x5a; usize::try_from(ceiling).expect("Taira proof cap fits usize")];
        let mut artifact = PrivacyReleaseProofArtifactEvidenceV1 {
            artifact_ordinal: 0,
            proof_sha256: sha256_v1(&canonical_proof_bytes),
            canonical_proof_bytes,
            proof_bytes_ceiling: ceiling,
        };
        assert!(validate_privacy_release_proof_artifacts_v1(
            protocol_id,
            case_kind,
            core::slice::from_ref(&artifact),
        ));

        artifact.canonical_proof_bytes.push(0);
        artifact.proof_sha256 = sha256_v1(&artifact.canonical_proof_bytes);
        assert!(!validate_privacy_release_proof_artifacts_v1(
            protocol_id,
            case_kind,
            core::slice::from_ref(&artifact),
        ));
    }

    #[test]
    fn every_typed_artifact_has_one_protocol_ceiling_below_the_consensus_cap() {
        let mut artifact_count = 0_usize;
        for protocol_id in PrivacyProtocolIdV1::ALL {
            for case_kind in PrivacyReleaseCaseKindV1::ALL {
                let stage_count = usize::from(privacy_release_proof_artifact_count_v1(
                    protocol_id,
                    case_kind,
                ));
                artifact_count = artifact_count
                    .checked_add(stage_count)
                    .expect("closed artifact count fits usize");
                for ordinal in 0..stage_count {
                    let ceiling = privacy_release_proof_artifact_ceiling_v1(
                        protocol_id,
                        case_kind,
                        u8::try_from(ordinal).expect("at most two artifacts"),
                    )
                    .expect("every required artifact has one canonical ceiling");
                    assert!(ceiling > 0);
                    assert!(ceiling <= PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1);
                }
                assert!(
                    privacy_release_proof_artifact_ceiling_v1(
                        protocol_id,
                        case_kind,
                        u8::try_from(stage_count).expect("at most two artifacts"),
                    )
                    .is_none()
                );
            }
        }
        assert_eq!(artifact_count, PRIVACY_RELEASE_PROOF_ARTIFACT_COUNT_V1);
    }

    #[test]
    fn canonical_proof_bytes_use_json_base64_and_round_trip_exactly() {
        let protocol_id = PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1;
        let case_kind = PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd;
        let canonical_proof_bytes = vec![0x00, 0x01, 0xfe, 0xff];
        let artifact = PrivacyReleaseProofArtifactEvidenceV1 {
            artifact_ordinal: 0,
            proof_sha256: sha256_v1(&canonical_proof_bytes),
            canonical_proof_bytes,
            proof_bytes_ceiling: privacy_release_proof_artifact_ceiling_v1(
                protocol_id,
                case_kind,
                0,
            )
            .expect("FCMP++ stage has one canonical ceiling"),
        };
        let json = norito::json::to_json(&artifact).expect("artifact JSON encodes");
        assert!(json.contains("\"canonical_proof_bytes\":\"AAH+/w==\""));
        let decoded: PrivacyReleaseProofArtifactEvidenceV1 =
            norito::json::from_str(&json).expect("artifact JSON decodes");
        assert_eq!(decoded, artifact);
        let unpadded = json.replace("AAH+/w==", "AAH+/w");
        assert!(
            norito::json::from_str::<PrivacyReleaseProofArtifactEvidenceV1>(&unpadded).is_err(),
            "non-canonical base64 spelling must reject"
        );

        let mut legacy_json = json;
        let closing_brace = legacy_json
            .pop()
            .expect("canonical artifact JSON has a closing brace");
        assert_eq!(closing_brace, '}');
        legacy_json.push_str(",\"proof_bytes\":4}");
        assert!(
            norito::json::from_str::<PrivacyReleaseProofArtifactEvidenceV1>(&legacy_json).is_err(),
            "removed reported-length field must not be accepted as a compatibility alias"
        );
    }

    #[test]
    fn every_protocol_has_one_distinct_nonempty_canonical_descriptor() {
        let descriptors = PrivacyProtocolIdV1::ALL.map(privacy_release_protocol_descriptor_v1);
        assert!(descriptors.iter().all(|descriptor| !descriptor.is_empty()));
        for (index, descriptor) in descriptors.iter().enumerate() {
            assert!(!descriptors[index + 1..].contains(descriptor));
        }
    }

    #[test]
    #[ignore = "operator-only native proof construction for the complete ZK-AMS corruption stage"]
    fn zk_ams_corruption_stage_rejects_maximum_and_submaximum_wire_mutations() {
        let protocol_id = PrivacyProtocolIdV1::IrohaZkAmsV1;
        let case_kind = PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation;
        let evidence =
            run_privacy_release_stage_v1(protocol_id, case_kind).expect("ZK-AMS corruption stage");
        assert_eq!(evidence.protocol_id, protocol_id);
        assert_eq!(evidence.case_kind, case_kind);
        assert_eq!(
            evidence.failure_class,
            PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected
        );
        assert_eq!(
            evidence.proof_artifacts.len(),
            usize::from(privacy_release_proof_artifact_count_v1(
                protocol_id,
                case_kind
            ))
        );
        assert!(validate_privacy_release_proof_artifacts_v1(
            protocol_id,
            case_kind,
            &evidence.proof_artifacts,
        ));
    }
