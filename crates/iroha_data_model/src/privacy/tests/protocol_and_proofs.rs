    use std::str::FromStr as _;

    use hex_literal::hex;

    use crate::{domain::DomainId, name::Name};

    use super::{
        exact12_fixture::{
            account, assert_fixed_width_norito, asset_definition_id, bootle_lantern_policy,
            commitment, context, encrypted_output, envelope, fcmp_input, fcmp_output,
            jindo_commitment, jindo_field, nullifier, orchard_action, p256_ciphertext, p256_point,
            proof_for, proof_variant_name, raw, redigest_bootle_lantern_policy,
            redigest_zk_ace_policy, sample_statements, sorted_fcmp_outputs, statement_for,
            statement_variant_name, zk_ace_allowlist, zk_ace_policy, zk_ams_anchor,
            zk_ams_provision_statement, zk_ams_seed_key, zk_x509_certificate_policy, zk_x509_crl,
            zk_x509_trust_anchor,
        },
        *,
    };

    fn pgc_accounts(count: u8) -> Vec<PrivacyPgcAccountV1> {
        (1..=count)
            .map(|seed| PrivacyPgcAccountV1 {
                public_key: p256_point(seed),
                encrypted_balance: p256_ciphertext(seed),
            })
            .collect()
    }

    fn pgc_bootstrap() -> PrivacyPgcAccountBootstrapV1 {
        let statement = statement_for(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1);
        PrivacyPgcAccountBootstrapV1 {
            namespace: PrivacyNamespaceV1::from_statement(&statement),
            initial_root: PrivacyRootV1::new(raw(201)),
            initial_epoch: 1,
            total_supply: 160,
            accounts: pgc_accounts(16),
        }
    }

    #[derive(Clone, Copy)]
    enum RootCorruption {
        ZeroSuccessor,
        Unchanged,
        SkippedEpoch,
        EpochOverflow,
    }

    fn corrupt_root_transition(statement: &mut PrivacyStatementV1, corruption: RootCorruption) {
        macro_rules! corrupt {
            ($current:expr, $epoch:expr, $next:expr, $next_epoch:expr) => {
                match corruption {
                    RootCorruption::ZeroSuccessor => $next = PrivacyRootV1::new([0; 32]),
                    RootCorruption::Unchanged => $next = $current,
                    RootCorruption::SkippedEpoch => {
                        $next_epoch = $epoch.checked_add(2).expect("fixture epoch has room")
                    }
                    RootCorruption::EpochOverflow => {
                        $epoch = u64::MAX;
                        $next_epoch = 0;
                    }
                }
            };
        }
        match statement {
            PrivacyStatementV1::AnonymousPgcKOutOfNV1(statement) => corrupt!(
                statement.account_state_root,
                statement.account_state_root_epoch,
                statement.next_account_state_root,
                statement.next_account_state_root_epoch
            ),
            PrivacyStatementV1::IrohaZkAmsV1(statement) => {
                let PrivacyZkAmsActionV1::BatchAdmission(batch) = &mut statement.action else {
                    panic!("ZK-AMS provisioning does not manage a root transition")
                };
                corrupt!(
                    batch.account_registry_root,
                    batch.account_registry_root_epoch,
                    batch.next_account_registry_root,
                    batch.next_account_registry_root_epoch
                )
            }
            PrivacyStatementV1::OrchardHalo2ActionsV1(_) => {
                panic!("Orchard successor roots are derived from the authoritative node frontier")
            }
            PrivacyStatementV1::MoneroFcmpPlusPlusV1(_)
            | PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(_)
            | PrivacyStatementV1::PqMaspStarkV0(_) => {
                panic!("FCMP++ and private-note successor roots are validator-derived")
            }
            _ => panic!("protocol does not manage a root transition"),
        }
    }

    fn protocol_limits(protocol: PrivacyProtocolIdV1) -> PrivacyProtocolActivationLimitsV1 {
        match protocol {
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV0 => {
                PrivacyProtocolActivationLimitsV1::ZkAcePqAuthorizationV0
            }
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 => {
                PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
                    AnonymousPgcActivationLimitsV1 {
                        max_anonymity_set_size: ANONYMOUS_PGC_MAX_ANONYMITY_SET_SIZE_V1,
                        max_recipient_count: ANONYMOUS_PGC_MAX_RECIPIENTS_V1,
                    },
                )
            }
            PrivacyProtocolIdV1::VeRangeTransparentRangeV1 => {
                PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
                    VeRangeActivationLimitsV1 {
                        max_aggregation_count: VERANGE_TAIRA_MAX_AGGREGATION_COUNT_V1,
                    },
                )
            }
            PrivacyProtocolIdV1::IrohaZkAmsV1 => {
                PrivacyProtocolActivationLimitsV1::IrohaZkAmsV1(ZkAmsActivationLimitsV1 {
                    max_batch_size: ZK_AMS_MAX_BATCH_SIZE_V1,
                    max_ring_size: ZK_AMS_MAX_RING_SIZE_V1,
                })
            }
            PrivacyProtocolIdV1::VegaExistingCredentialZkV0 => {
                PrivacyProtocolActivationLimitsV1::VegaExistingCredentialZkV0
            }
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 => {
                PrivacyProtocolActivationLimitsV1::IrohaZkX509StarkP256V0
            }
            PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0 => {
                PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                    JindoActivationLimitsV1 {
                        max_polynomial_count: IROHA_JINDO_MAX_POLYNOMIALS_V1,
                    },
                )
            }
            PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1 => {
                PrivacyProtocolActivationLimitsV1::IrohaBootleLanternAnoncredV1
            }
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1 => {
                PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(
                    OrchardActivationLimitsV1 {
                        max_action_count: ORCHARD_MAX_ACTIONS_V1,
                    },
                )
            }
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => {
                PrivacyProtocolActivationLimitsV1::MoneroFcmpPlusPlusV1(FcmpActivationLimitsV1 {
                    max_input_count: FCMP_MAX_INPUTS_V1,
                    max_output_count: FCMP_MAX_OUTPUTS_V1,
                })
            }
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => {
                PrivacyProtocolActivationLimitsV1::IrohaIvmPrivateNoteStarkV1(
                    IvmPrivateNoteActivationLimitsV1 {
                        max_input_count: IVM_PRIVATE_NOTE_MAX_INPUTS_V1,
                        max_output_count: IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1,
                    },
                )
            }
            PrivacyProtocolIdV1::PqMaspStarkV0 => {
                PrivacyProtocolActivationLimitsV1::PqMaspStarkV0(PqMaspActivationLimitsV1 {
                    max_input_count: PQ_MASP_MAX_INPUTS_V1,
                    max_output_count: PQ_MASP_MAX_OUTPUTS_V1,
                })
            }
        }
    }

    fn assert_stable_schema_wire<T>(value: &T, schema_name: &str, expected_schema_hash: [u8; 16])
    where
        T: norito::NoritoSerialize
            + for<'de> norito::NoritoDeserialize<'de>
            + PartialEq
            + core::fmt::Debug
            + 'static,
    {
        let derived_schema_hash = norito::core::schema_hash_for_name(schema_name);
        assert_eq!(
            derived_schema_hash, expected_schema_hash,
            "permanent schema-name KAT changed for {schema_name}"
        );
        assert_eq!(
            <T as norito::NoritoSerialize>::schema_hash(),
            expected_schema_hash
        );
        assert_eq!(
            <T as norito::NoritoDeserialize<'static>>::schema_hash(),
            expected_schema_hash
        );

        let legacy_type_name_hash = norito::core::type_name_schema_hash::<T>();
        assert_ne!(
            legacy_type_name_hash, expected_schema_hash,
            "permanent public schema must not fall back to the Rust type name"
        );

        let canonical = norito::encode_canonical(value).expect("encode permanent-schema frame");
        assert_eq!(
            &canonical[6..22],
            expected_schema_hash.as_slice(),
            "canonical frame header must carry the permanent schema identity"
        );
        assert_eq!(
            norito::decode_canonical::<T>(&canonical).expect("decode permanent-schema frame"),
            *value
        );

        let mut legacy_header = canonical.clone();
        legacy_header[6..22].copy_from_slice(&legacy_type_name_hash);
        assert!(
            matches!(
                norito::decode_canonical::<T>(&legacy_header),
                Err(norito::Error::SchemaMismatch)
            ),
            "the pre-release Rust type-name wire must fail closed"
        );

        let mut forged_header = canonical;
        forged_header[6] ^= 0x80;
        assert!(
            matches!(
                norito::decode_canonical::<T>(&forged_header),
                Err(norito::Error::SchemaMismatch)
            ),
            "an unknown schema identity must fail before payload decoding"
        );
    }

    #[test]
    fn first_release_privacy_schema_names_and_old_headers_are_frozen() {
        let statement = statement_for(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0);
        let PrivacyStatementV1::ZkAcePqAuthorizationV0(authorization_statement) = &statement else {
            unreachable!("ZK-ACE fixture must use the typed authorization statement")
        };
        let authorization_statement = authorization_statement.clone();
        let public_inputs =
            crate::zk::ZkAcePrivacyPublicInputsV1::new(authorization_statement.clone(), raw(0xD1));
        let proof = proof_for(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0);
        let proof_envelope = envelope(statement.clone());
        let policy = zk_ace_policy(
            PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1,
            11,
            PrivacyZkAcePolicyLifecycleV1::Active,
        );
        let policy_material = PrivacyZkAcePolicyDigestMaterialV1 {
            policy_id: policy.policy_id,
            identity_commitment: policy.identity_commitment,
            policy_digest: policy.policy_digest,
            authorization_epoch: policy.authorization_epoch,
            asset_definition_id: policy.asset_definition_id,
            source_allowlist: policy.source_allowlist,
            lifecycle: policy.lifecycle,
        };

        assert_stable_schema_wire(
            &authorization_statement,
            ZK_ACE_AUTHORIZATION_STATEMENT_SCHEMA_NAME_V1,
            hex!("4acf679326b17350dcb57f4ea7ac20a1"),
        );
        assert_stable_schema_wire(
            &statement,
            PRIVACY_STATEMENT_SCHEMA_NAME_V1,
            hex!("7966b2f6ebc8c1ff1a1eb8ac458657af"),
        );
        assert_stable_schema_wire(
            &public_inputs,
            crate::zk::ZK_ACE_PRIVACY_PUBLIC_INPUTS_SCHEMA_NAME_V1,
            hex!("0f16958a9641702815d6cddee3aeb8aa"),
        );
        assert_stable_schema_wire(
            &policy_material,
            ZK_ACE_POLICY_DIGEST_MATERIAL_SCHEMA_NAME_V1,
            hex!("5f08d7306e9c76b183a60175fa514966"),
        );
        assert_stable_schema_wire(
            &proof,
            PRIVACY_PROOF_SCHEMA_NAME_V1,
            hex!("8335f36ecd62f6cc59715441a3496a27"),
        );
        assert_stable_schema_wire(
            &proof_envelope,
            PRIVACY_PROOF_ENVELOPE_SCHEMA_NAME_V1,
            hex!("3956178024ddd2abae83d3a5b59827fb"),
        );
    }

    fn activation(envelope: &PrivacyProofEnvelopeV1) -> PrivacyProtocolActivationRecordV1 {
        PrivacyProtocolActivationRecordV1 {
            protocol_id: envelope.protocol_id,
            proof_system_id: envelope.proof_system_id,
            engine_id: envelope.engine_id,
            parameter_id: envelope.parameter_id,
            parameter_digest: envelope.parameter_digest,
            verifier_digest: envelope.verifier_digest,
            statement_schema_digest: envelope.statement_schema_digest,
            engine_manifest_digest: envelope.engine_manifest_digest,
            lifecycle: PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
                proposed_at_height: 1,
                activated_at_height: 2,
                state_since_height: 2,
            }),
            protocol_limits: protocol_limits(envelope.protocol_id),
            pending_protocol_limits_tightening: None,
            assurance: PrivacyAssuranceV1::Experimental,
        }
    }

    fn compiled_profile_snapshot(
        activation: &PrivacyProtocolActivationRecordV1,
    ) -> PrivacyCompiledProfileSnapshotV1 {
        PrivacyCompiledProfileSnapshotV1 {
            protocol_id: activation.protocol_id,
            proof_system_id: activation.proof_system_id,
            engine_id: activation.engine_id,
            parameter_id: activation.parameter_id,
            parameter_digest: activation.parameter_digest,
            verifier_digest: activation.verifier_digest,
            statement_schema_digest: activation.statement_schema_digest,
            engine_manifest_digest: activation.engine_manifest_digest,
            protocol_limits: activation.protocol_limits,
        }
    }

    fn capability_snapshot() -> PrivacyCapabilitySnapshotV1 {
        let pgc_activation = activation(&envelope(statement_for(
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
        )));
        let pgc_profile = compiled_profile_snapshot(&pgc_activation);
        PrivacyCapabilitySnapshotV1 {
            version: PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1,
            committed_height: 2,
            consensus_policy: PrivacyConsensusPolicyV1::taira_default(),
            protocols: PrivacyProtocolIdV1::ALL
                .into_iter()
                .map(|protocol_id| {
                    if protocol_id == PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 {
                        PrivacyCapabilityRowV1 {
                            protocol_id,
                            compiled_profile: PrivacyCompiledProfileResultV1::Available(
                                pgc_profile,
                            ),
                            activation: Some(pgc_activation),
                        }
                    } else {
                        PrivacyCapabilityRowV1 {
                            protocol_id,
                            compiled_profile: PrivacyCompiledProfileResultV1::Unavailable(
                                PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
                            ),
                            activation: None,
                        }
                    }
                })
                .collect(),
        }
    }

    #[test]
    fn protocol_ids_keep_closed_norito_discriminants() {
        assert_eq!(PrivacyProtocolIdV1::ALL.len(), PrivacyProtocolIdV1::COUNT);
        for (expected, protocol) in PrivacyProtocolIdV1::ALL.into_iter().enumerate() {
            let encoded = protocol.encode();
            assert_eq!(encoded, u32::try_from(expected).unwrap().to_le_bytes());
            assert_eq!(
                PrivacyProtocolIdV1::decode(&mut encoded.as_slice()).expect("decode protocol"),
                protocol
            );
        }
        let protocol_count =
            u32::try_from(PrivacyProtocolIdV1::COUNT).expect("protocol count fits u32");
        for unknown in [protocol_count, 99, u32::MAX] {
            assert!(
                PrivacyProtocolIdV1::decode(&mut unknown.to_le_bytes().as_slice()).is_err(),
                "unknown protocol discriminant {unknown} must fail"
            );
        }
    }

    #[test]
    fn protocol_ids_have_unique_exact_external_labels() {
        let expected = [
            (
                PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
                "zk-ace-pq-authorization-v0",
            ),
            (
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                "anonymous-pgc-k-out-of-n-v1",
            ),
            (
                PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
                "verange-transparent-range-v1",
            ),
            (PrivacyProtocolIdV1::IrohaZkAmsV1, "iroha-zk-ams-v1"),
            (
                PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
                "vega-existing-credential-zk-v0",
            ),
            (
                PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
                "iroha-zk-x509-stark-p256-v0",
            ),
            (
                PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
                "iroha-jindo-polynomial-commitment-v0",
            ),
            (
                PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
                "iroha-bootle-lantern-anoncred-v1",
            ),
            (
                PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
                "orchard-halo2-actions-v1",
            ),
            (
                PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
                "monero-fcmp-plus-plus-v1",
            ),
            (
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                "iroha-ivm-private-note-stark-v1",
            ),
            (PrivacyProtocolIdV1::PqMaspStarkV0, "pq-masp-stark-v0"),
        ];
        assert_eq!(expected.len(), PrivacyProtocolIdV1::COUNT);

        for (index, (protocol, label)) in expected.into_iter().enumerate() {
            assert_eq!(PrivacyProtocolIdV1::ALL[index], protocol);
            assert_eq!(protocol.canonical_label(), label);
            assert_eq!(
                PrivacyProtocolIdV1::from_canonical_label(label),
                Some(protocol)
            );
            assert!(
                PrivacyProtocolIdV1::ALL[..index]
                    .iter()
                    .all(|prior| prior.canonical_label() != label),
                "duplicate privacy protocol label {label}"
            );
        }
    }

    #[test]
    fn active_and_retired_protocol_labels_share_one_exact_reservation_namespace() {
        let mut reserved = std::collections::BTreeSet::new();
        for protocol in PrivacyProtocolIdV1::ALL {
            let label = protocol.canonical_label();
            assert!(
                privacy_protocol_label_is_reserved_v1(label),
                "active label {label} must be reserved"
            );
            assert!(reserved.insert(label), "duplicate active label {label}");
        }
        for label in PRIVACY_RETIRED_PROTOCOL_LABELS_V1 {
            assert!(
                PrivacyProtocolIdV1::from_canonical_label(label).is_none(),
                "retired label {label} must not become active"
            );
            assert!(
                privacy_protocol_label_is_reserved_v1(label),
                "retired label {label} must remain reserved"
            );
            assert!(
                reserved.insert(label),
                "retired label {label} overlaps another reservation"
            );
        }
        assert_eq!(
            reserved.len(),
            PrivacyProtocolIdV1::COUNT + PRIVACY_RETIRED_PROTOCOL_LABELS_V1.len()
        );

        for label in reserved {
            for near_miss in [
                format!("generic-{label}"),
                format!("{label}-generic"),
                format!(" {label}"),
                format!("{label} "),
                label.to_ascii_uppercase(),
            ] {
                assert!(
                    !privacy_protocol_label_is_reserved_v1(&near_miss),
                    "near-miss label {near_miss:?} must not alias {label:?}"
                );
            }
        }
    }

    #[test]
    fn protocol_id_parser_rejects_aliases_retired_ids_and_noncanonical_text() {
        for label in [
            "",
            " ",
            " iroha-zk-ams-v1",
            "iroha-zk-ams-v1 ",
            "IROHA-ZK-AMS-V1",
            "zk-ams-recursive-admission-v0",
            "zk-x509-onchain-identity-v0",
            "jindo-lattice-pcs-zk-v0",
            "sis-hints-anoncred-pq-v0",
            "sis-with-hints",
            "iroha-bootle-genisis-ac-stark-v0",
            "miden-stark-note-v1",
            "pq-masp-stark-fri-v1",
            "zkat-policy-private-auth-v1",
            "silent-threshold-anoncred-v0",
            "penumbra-masp-v1",
            "aztec-private-rollup-v1",
            "iroha-zk-ams-v1\0",
            "iroha-zk-\u{200b}ams-v1",
            "iroha\u{ff0f}zk-ams-v1",
            "iroh\u{0430}-zk-ams-v1",
        ] {
            assert!(
                PrivacyProtocolIdV1::from_canonical_label(label).is_none(),
                "non-canonical protocol label {label:?} must fail"
            );
        }
    }

    fn assert_protocol_json_labels_roundtrip() {
        for protocol in PrivacyProtocolIdV1::ALL {
            let expected = format!(
                "{{\"protocol\":\"{}\",\"value\":null}}",
                protocol.canonical_label()
            );
            assert_eq!(
                norito::json::to_json(&protocol).expect("serialize protocol id"),
                expected
            );
            assert_eq!(
                norito::json::from_json::<PrivacyProtocolIdV1>(&expected)
                    .expect("deserialize protocol id"),
                protocol
            );

            let limits = protocol_limits(protocol);
            let limits_json = norito::json::to_json(&limits).expect("serialize protocol limits");
            assert!(
                limits_json.starts_with(&format!(
                    "{{\"protocol\":\"{}\",\"limits\":",
                    protocol.canonical_label()
                )),
                "unexpected activation-limit label: {limits_json}"
            );
            assert_eq!(
                norito::json::from_json::<PrivacyProtocolActivationLimitsV1>(&limits_json)
                    .expect("deserialize protocol limits"),
                limits
            );
        }
    }

    fn assert_proof_system_json_labels_roundtrip() {
        let proof_systems = [
            (
                PrivacyProofSystemIdV1::StarkFriSha256Goldilocks,
                "stark-fri-sha256-goldilocks",
            ),
            (
                PrivacyProofSystemIdV1::ZkAmsMaskedRelaxedSpartanT256Ristretto255Sha3_512,
                "zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512",
            ),
            (
                PrivacyProofSystemIdV1::AnonymousPgcP256,
                "anonymous-pgc-p256",
            ),
            (
                PrivacyProofSystemIdV1::IrohaVeRangeP256,
                "iroha-verange-p256",
            ),
            (
                PrivacyProofSystemIdV1::VegaNeutronNovaSpartanHyraxT256,
                "vega-neutron-nova-spartan-hyrax-t256",
            ),
            (
                PrivacyProofSystemIdV1::JindoPolynomialCommitment,
                "jindo-polynomial-commitment",
            ),
            (PrivacyProofSystemIdV1::Halo2IpaPasta, "halo2-ipa-pasta"),
            (
                PrivacyProofSystemIdV1::FcmpPlusPlusCurveTreeBulletproofs,
                "fcmp-plus-plus-curve-tree-bulletproofs",
            ),
            (
                PrivacyProofSystemIdV1::LanternLnp22ModuleLinearNorm,
                "lantern-lnp22-module-linear-norm",
            ),
        ];
        for (value, label) in proof_systems {
            let expected = format!("{{\"proof_system\":\"{label}\",\"value\":null}}");
            assert_eq!(
                norito::json::to_json(&value).expect("serialize proof-system id"),
                expected
            );
            assert_eq!(
                norito::json::from_json::<PrivacyProofSystemIdV1>(&expected)
                    .expect("deserialize proof-system id"),
                value
            );
        }
    }

    fn assert_engine_json_labels_roundtrip() {
        let engines = [
            (
                PrivacyEngineIdV1::NativeGoldilocksStarkFri,
                "native-goldilocks-stark-fri",
            ),
            (
                PrivacyEngineIdV1::NativeZkAmsMaskedRelaxedSpartanT256Ristretto255,
                "native-zk-ams-masked-relaxed-spartan-t256-ristretto255",
            ),
            (
                PrivacyEngineIdV1::NativeAnonymousPgcP256,
                "native-anonymous-pgc-p256",
            ),
            (PrivacyEngineIdV1::NativeVeRangeP256, "native-verange-p256"),
            (PrivacyEngineIdV1::NativeVega, "native-vega"),
            (PrivacyEngineIdV1::NativeJindo, "native-jindo"),
            (
                PrivacyEngineIdV1::NativeHalo2Orchard,
                "native-halo2-orchard",
            ),
            (
                PrivacyEngineIdV1::NativeFcmpPlusPlus,
                "native-fcmp-plus-plus",
            ),
            (
                PrivacyEngineIdV1::NativeLanternLnp22,
                "native-lantern-lnp22",
            ),
        ];
        for (value, label) in engines {
            let expected = format!("{{\"engine\":\"{label}\",\"value\":null}}");
            assert_eq!(
                norito::json::to_json(&value).expect("serialize engine id"),
                expected
            );
            assert_eq!(
                norito::json::from_json::<PrivacyEngineIdV1>(&expected)
                    .expect("deserialize engine id"),
                value
            );
        }
    }

    fn assert_unavailable_reason_json_labels_roundtrip() {
        let unavailable = [
            (
                PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
                "{\"reason\":\"engine-unavailable\",\"detail\":null}",
            ),
            (
                PrivacyCompiledProfileUnavailableReasonV1::ProfileInitializationFailed,
                "{\"reason\":\"profile-initialization-failed\",\"detail\":null}",
            ),
            (
                PrivacyCompiledProfileUnavailableReasonV1::StatementSchemaInvalid(
                    PrivacyCompiledStatementSchemaErrorV1::ConflictingStableTypeId,
                ),
                "{\"reason\":\"statement-schema-invalid\",\"detail\":{\"schema_error\":\"conflicting-stable-type-id\",\"detail\":null}}",
            ),
            (
                PrivacyCompiledProfileUnavailableReasonV1::StatementSchemaInvalid(
                    PrivacyCompiledStatementSchemaErrorV1::MissingTypeReference,
                ),
                "{\"reason\":\"statement-schema-invalid\",\"detail\":{\"schema_error\":\"missing-type-reference\",\"detail\":null}}",
            ),
        ];
        for (value, expected) in unavailable {
            assert_eq!(
                norito::json::to_json(&value).expect("serialize unavailable reason"),
                expected
            );
            assert_eq!(
                norito::json::from_json::<PrivacyCompiledProfileUnavailableReasonV1>(expected)
                    .expect("deserialize unavailable reason"),
                value
            );
        }
    }

    #[test]
    fn privacy_public_json_labels_are_exact_and_roundtrip() {
        assert_protocol_json_labels_roundtrip();
        assert_proof_system_json_labels_roundtrip();
        assert_engine_json_labels_roundtrip();
        assert_unavailable_reason_json_labels_roundtrip();
        assert_eq!(
            norito::json::to_json(&PrivacyAssuranceV1::Experimental).expect("serialize assurance"),
            "{\"assurance\":\"experimental\",\"value\":null}"
        );
        let lifecycle = PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
            proposed_at_height: 1,
            activated_at_height: 2,
            state_since_height: 2,
        });
        assert_eq!(
            norito::json::to_json(&lifecycle).expect("serialize lifecycle"),
            "{\"state\":\"active\",\"record\":{\"proposed_at_height\":1,\"activated_at_height\":2,\"state_since_height\":2}}"
        );
    }

    #[test]
    fn privacy_public_json_rejects_aliases_case_whitespace_confusables_and_unknown_fields() {
        for hostile in [
            "AnonymousPgcKOutOfNV1",
            "anonymous-pgc-k-out-of-n",
            "anonymous-pgc-k-out-of-n-v0",
            "ANONYMOUS-PGC-K-OUT-OF-N-V1",
            " anonymous-pgc-k-out-of-n-v1",
            "anonymous-pgc-k-out-of-n-v1 ",
            "anonymous\u{2010}pgc-k-out-of-n-v1",
            "anonym\u{043e}us-pgc-k-out-of-n-v1",
            "iroha-bootle-genisis-ac-stark-v0",
            "unknown",
        ] {
            let json = format!("{{\"protocol\":\"{hostile}\",\"value\":null}}");
            assert!(
                norito::json::from_json::<PrivacyProtocolIdV1>(&json).is_err(),
                "hostile protocol JSON {json} must fail"
            );
        }
        for hostile in [
            "{\"protocol\":\"anonymous-pgc-k-out-of-n-v1\",\"value\":null,\"extra\":1}",
            "{\"protocol\":\"anonymous-pgc-k-out-of-n-v1\",\"protocol\":\"anonymous-pgc-k-out-of-n-v1\",\"value\":null}",
            "{\"proof_system\":\"AnonymousPgcP256\",\"value\":null}",
            "{\"proof_system\":\"anonymous-pgc-p256 \",\"value\":null}",
            "{\"proof_system\":\"stark-fri-poseidon2-goldilocks\",\"value\":null}",
            "{\"engine\":\"NativeAnonymousPgcP256\",\"value\":null}",
            "{\"engine\":\"native-anonymous-pgc-p25\u{ff16}\",\"value\":null}",
            "{\"reason\":\"EngineUnavailable\",\"detail\":null}",
            "{\"reason\":\"engine-unavailable\",\"detail\":null,\"extra\":false}",
            "{\"reason\":\"statement-schema-invalid\",\"detail\":{\"schema_error\":\"MissingTypeReference\",\"detail\":null}}",
            "{\"assurance\":\"production\",\"value\":null}",
            "{\"assurance\":\"Experimental\",\"value\":null}",
        ] {
            let rejected = norito::json::from_json::<PrivacyProtocolIdV1>(hostile).is_err()
                && norito::json::from_json::<PrivacyProofSystemIdV1>(hostile).is_err()
                && norito::json::from_json::<PrivacyEngineIdV1>(hostile).is_err()
                && norito::json::from_json::<PrivacyCompiledProfileUnavailableReasonV1>(hostile)
                    .is_err()
                && norito::json::from_json::<PrivacyAssuranceV1>(hostile).is_err();
            assert!(rejected, "hostile closed-enum JSON {hostile} must fail");
        }
    }

    fn available_pgc_profile(
        snapshot: &PrivacyCapabilitySnapshotV1,
    ) -> PrivacyCompiledProfileSnapshotV1 {
        match snapshot.protocols[1].compiled_profile {
            PrivacyCompiledProfileResultV1::Available(profile) => profile,
            PrivacyCompiledProfileResultV1::Unavailable(_) => unreachable!("PGC fixture available"),
        }
    }

    fn assert_capability_snapshot_codecs(snapshot: &PrivacyCapabilitySnapshotV1) -> String {
        snapshot.validate().expect("valid capability snapshot");

        let archive = norito::to_bytes(snapshot).expect("encode snapshot");
        let decoded: PrivacyCapabilitySnapshotV1 =
            norito::decode_from_bytes(&archive).expect("decode snapshot");
        assert_eq!(decoded, *snapshot);
        decoded.validate().expect("validate decoded snapshot");

        let canonical = norito::json::to_json(snapshot).expect("serialize snapshot JSON");
        let decoded_json: PrivacyCapabilitySnapshotV1 =
            norito::json::from_json(&canonical).expect("decode snapshot JSON");
        assert_eq!(decoded_json, *snapshot);
        decoded_json.validate().expect("validate JSON snapshot");
        canonical
    }

    fn assert_capability_snapshot_json_adversaries(
        snapshot: &PrivacyCapabilitySnapshotV1,
        canonical: &str,
    ) {
        let unknown = canonical.replacen('{', "{\"unknown\":true,", 1);
        assert!(
            norito::json::from_json::<PrivacyCapabilitySnapshotV1>(&unknown).is_err(),
            "unknown top-level field must fail"
        );
        let duplicate = canonical.replacen('{', "{\"version\":1,", 1);
        assert!(
            norito::json::from_json::<PrivacyCapabilitySnapshotV1>(&duplicate).is_err(),
            "duplicate top-level field must fail"
        );

        let assurance_alias = canonical.replacen(
            "\"assurance\":\"experimental\"",
            "\"assurance\":\"production\"",
            1,
        );
        assert!(
            norito::json::from_json::<PrivacyCapabilitySnapshotV1>(&assurance_alias).is_err(),
            "non-Experimental assurance must fail"
        );

        let pgc_profile = available_pgc_profile(snapshot);
        let parameter_json =
            norito::json::to_json(&pgc_profile.parameter_id).expect("serialize fixed bytes");
        let malformed_fixed_bytes = canonical.replacen(
            &format!("\"parameter_id\":{parameter_json}"),
            "\"parameter_id\":[1]",
            1,
        );
        assert!(
            norito::json::from_json::<PrivacyCapabilitySnapshotV1>(&malformed_fixed_bytes).is_err(),
            "wrong-length fixed bytes must fail"
        );
        let out_of_range_fixed_bytes = canonical.replacen(
            &format!("\"parameter_id\":{parameter_json}"),
            "\"parameter_id\":[256,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]",
            1,
        );
        assert!(
            norito::json::from_json::<PrivacyCapabilitySnapshotV1>(&out_of_range_fixed_bytes)
                .is_err(),
            "out-of-range fixed byte must fail"
        );
    }

    fn assert_capability_snapshot_structural_adversaries(snapshot: PrivacyCapabilitySnapshotV1) {
        let mut missing = snapshot.clone();
        missing.protocols.pop();
        assert!(matches!(
            missing.validate(),
            Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolCount { .. })
        ));

        let mut duplicate_row = snapshot.clone();
        duplicate_row.protocols[2] = duplicate_row.protocols[1];
        assert!(matches!(
            duplicate_row.validate(),
            Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolOrder { .. })
        ));

        let mut reordered = snapshot.clone();
        reordered.protocols.swap(0, 1);
        assert!(matches!(
            reordered.validate(),
            Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolOrder { .. })
        ));

        let mut embedded_id_mismatch = snapshot.clone();
        embedded_id_mismatch.protocols[2].compiled_profile =
            PrivacyCompiledProfileResultV1::Available(available_pgc_profile(&snapshot));
        assert!(matches!(
            embedded_id_mismatch.validate(),
            Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolRow {
                source: PrivacyCapabilityRowValidationErrorV1::CompiledProfileProtocolMismatch { .. },
                ..
            })
        ));

        let mut activation_profile_mismatch = snapshot.clone();
        activation_profile_mismatch.protocols[1]
            .activation
            .as_mut()
            .expect("PGC activation")
            .parameter_digest = PrivacyParameterDigestV1::new(raw(250));
        assert!(matches!(
            activation_profile_mismatch.validate(),
            Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolRow {
                source: PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                    field: PrivacyCapabilityBindingFieldV1::ParameterDigest,
                },
                ..
            })
        ));

        let mut unavailable_activation = snapshot;
        unavailable_activation.protocols[2].activation = Some(activation(&envelope(
            statement_for(PrivacyProtocolIdV1::VeRangeTransparentRangeV1),
        )));
        assert!(matches!(
            unavailable_activation.validate(),
            Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolRow {
                source: PrivacyCapabilityRowValidationErrorV1::UnavailableActivation { .. },
                ..
            })
        ));
    }

    #[test]
    fn capability_snapshot_roundtrips_and_rejects_structural_adversaries() {
        let snapshot = capability_snapshot();
        let canonical = assert_capability_snapshot_codecs(&snapshot);
        assert_capability_snapshot_json_adversaries(&snapshot, &canonical);
        assert_capability_snapshot_structural_adversaries(snapshot);
    }

    #[test]
    fn canonical_capability_archive_validator_is_bounded_typed_and_fail_closed() {
        use PrivacyCapabilityArchiveValidationStatusV1 as Status;

        assert_eq!(PRIVACY_BRIDGE_ABI_VERSION_V1, 21);
        assert_eq!(PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1, 256 * 1024);
        assert_eq!(Status::Valid.code(), 0);
        assert_eq!(Status::NullPointer.code(), 1);
        assert_eq!(Status::Empty.code(), 2);
        assert_eq!(Status::ArchiveTooLarge.code(), 3);
        assert_eq!(Status::DecodeResourceLimit.code(), 4);
        assert_eq!(Status::SchemaMismatch.code(), 5);
        assert_eq!(Status::NonCanonical.code(), 6);
        assert_eq!(Status::MalformedArchive.code(), 7);
        assert_eq!(Status::InvalidSnapshot.code(), 8);

        let snapshot = capability_snapshot();
        let archive = norito::encode_canonical(&snapshot).expect("canonical capability archive");
        assert_eq!(
            validate_privacy_capability_archive_v1(&archive),
            Status::Valid
        );

        assert_eq!(validate_privacy_capability_archive_v1(&[]), Status::Empty);
        assert_eq!(
            validate_privacy_capability_archive_v1(&vec![
                0;
                PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1
                    + 1
            ]),
            Status::ArchiveTooLarge
        );
        assert_eq!(
            validate_privacy_capability_archive_v1(&archive[..archive.len() - 1]),
            Status::MalformedArchive
        );

        let mut wrong_schema = archive.clone();
        wrong_schema[6] ^= 0x80;
        assert_eq!(
            validate_privacy_capability_archive_v1(&wrong_schema),
            Status::SchemaMismatch
        );

        // Preserve a valid CRC over a one-byte payload while substituting the
        // expected snapshot schema. Header-only validation used to accept this
        // exact adversary; the typed decoder must reject it.
        let mut one_byte_fake = norito::encode_canonical(&0_u8).expect("canonical one-byte value");
        one_byte_fake[6..22].copy_from_slice(&archive[6..22]);
        assert_eq!(
            validate_privacy_capability_archive_v1(&one_byte_fake),
            Status::MalformedArchive
        );

        let mut reordered = snapshot.clone();
        reordered.protocols.swap(0, 1);
        let reordered =
            norito::encode_canonical(&reordered).expect("canonical reordered snapshot bytes");
        assert_eq!(
            validate_privacy_capability_archive_v1(&reordered),
            Status::InvalidSnapshot
        );

        let mut profile_mutation = snapshot.clone();
        let PrivacyCompiledProfileResultV1::Available(profile) =
            &mut profile_mutation.protocols[1].compiled_profile
        else {
            panic!("PGC fixture must have a compiled profile");
        };
        profile.parameter_digest = PrivacyParameterDigestV1::new([0; 32]);
        let profile_mutation =
            norito::encode_canonical(&profile_mutation).expect("canonical invalid-profile bytes");
        assert_eq!(
            validate_privacy_capability_archive_v1(&profile_mutation),
            Status::InvalidSnapshot
        );

        let mut activation_mutation = snapshot.clone();
        activation_mutation.protocols[1]
            .activation
            .as_mut()
            .expect("PGC fixture activation")
            .parameter_digest = PrivacyParameterDigestV1::new(raw(250));
        let activation_mutation = norito::encode_canonical(&activation_mutation)
            .expect("canonical activation-mismatch bytes");
        assert_eq!(
            validate_privacy_capability_archive_v1(&activation_mutation),
            Status::InvalidSnapshot
        );

        let mut excessive_rows = snapshot;
        excessive_rows.protocols.push(excessive_rows.protocols[0]);
        let excessive_rows =
            norito::encode_canonical(&excessive_rows).expect("canonical excessive-row bytes");
        assert_eq!(
            validate_privacy_capability_archive_v1(&excessive_rows),
            Status::DecodeResourceLimit
        );
    }

    #[test]
    fn all_protocol_mappings_and_typed_variants_are_exact() {
        let statements = sample_statements();
        assert_eq!(statements.len(), PrivacyProtocolIdV1::COUNT);
        for (protocol, statement) in PrivacyProtocolIdV1::ALL.into_iter().zip(statements) {
            assert_eq!(statement.protocol_id(), protocol);
            let proof = proof_for(protocol);
            assert_eq!(proof.protocol_id(), protocol);
            assert_eq!(
                statement_variant_name(&statement),
                protocol.canonical_typed_variant_label()
            );
            assert_eq!(
                proof_variant_name(&proof),
                protocol.canonical_typed_variant_label()
            );
            assert_eq!(
                protocol_limits(protocol).protocol_id(),
                protocol,
                "activation limits must carry the same closed protocol tag"
            );
        }
        assert_eq!(
            PrivacyProtocolIdV1::VeRangeTransparentRangeV1.expected_proof_system(),
            PrivacyProofSystemIdV1::IrohaVeRangeP256
        );
        assert_eq!(
            PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0.expected_proof_system(),
            PrivacyProofSystemIdV1::JindoPolynomialCommitment
        );
        assert_eq!(
            PrivacyProtocolIdV1::IrohaZkAmsV1.expected_proof_system(),
            PrivacyProofSystemIdV1::ZkAmsMaskedRelaxedSpartanT256Ristretto255Sha3_512
        );
        assert_eq!(
            PrivacyProtocolIdV1::IrohaZkAmsV1.expected_engine(),
            PrivacyEngineIdV1::NativeZkAmsMaskedRelaxedSpartanT256Ristretto255
        );
        for protocol in [
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            PrivacyProtocolIdV1::PqMaspStarkV0,
        ] {
            assert_eq!(
                protocol.expected_proof_system(),
                PrivacyProofSystemIdV1::StarkFriSha256Goldilocks,
                "{protocol:?} must identify the SHA-256 transcript/Merkle STARK"
            );
        }
    }

    #[test]
    fn exact12_typed_fixture_bundle_is_byte_complete_bounded_and_mutation_closed() {
        use PrivacyExact12FixtureBundleValidationStatusV1 as Status;

        let bundle = privacy_exact12_fixture_bundle_v1().expect("typed exact12 fixture bundle");
        let archive =
            privacy_exact12_fixture_bundle_bytes_v1().expect("canonical exact12 fixture archive");
        assert_eq!(bundle.version, 1);
        assert_eq!(bundle.rows.len(), PrivacyProtocolIdV1::COUNT);
        assert!(archive.len() <= PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1);
        assert_eq!(
            validate_privacy_exact12_fixture_bundle_v1(&archive),
            Status::Valid
        );

        for (row, expected_protocol) in bundle.rows.iter().zip(PrivacyProtocolIdV1::ALL) {
            assert_eq!(row.protocol_id, expected_protocol);
            let statement: PrivacyStatementV1 =
                norito::decode_from_bytes(&row.statement_norito).expect("typed statement");
            assert_eq!(statement.protocol_id(), expected_protocol);
            assert_eq!(
                norito::encode_canonical(&statement).expect("canonical statement"),
                row.statement_norito
            );

            let envelope: PrivacyProofEnvelopeV1 =
                norito::decode_from_bytes(&row.envelope_norito).expect("typed envelope");
            assert_eq!(envelope.protocol_id, expected_protocol);
            assert_eq!(envelope.statement, statement);
            envelope
                .validate_with_limits(&PrivacyConsensusLimitsV1::default())
                .expect("intrinsically valid sample envelope");
            assert_eq!(
                norito::encode_canonical(&envelope).expect("canonical envelope"),
                row.envelope_norito
            );
        }

        assert_eq!(
            validate_privacy_exact12_fixture_bundle_v1(&[]),
            Status::Empty
        );
        assert_eq!(
            validate_privacy_exact12_fixture_bundle_v1(&vec![
                0;
                PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1
                    + 1
            ]),
            Status::ArchiveTooLarge
        );

        let truncated = &archive[..archive.len() - 1];
        assert!(
            !validate_privacy_exact12_fixture_bundle_v1(truncated).is_valid(),
            "truncated outer archive must reject"
        );
        let mut trailing = archive.clone();
        trailing.push(0);
        assert!(
            !validate_privacy_exact12_fixture_bundle_v1(&trailing).is_valid(),
            "trailing outer bytes must reject"
        );

        let mut wrong_version = bundle.clone();
        wrong_version.version = 2;
        let wrong_version =
            norito::encode_canonical(&wrong_version).expect("canonical wrong-version mutation");
        assert_eq!(
            validate_privacy_exact12_fixture_bundle_v1(&wrong_version),
            Status::InvalidBundle
        );

        let mut reordered = bundle.clone();
        reordered.rows.swap(0, 1);
        let reordered = norito::encode_canonical(&reordered).expect("canonical reordered mutation");
        assert_eq!(
            validate_privacy_exact12_fixture_bundle_v1(&reordered),
            Status::InvalidBundle
        );

        let mut cross_protocol = bundle.clone();
        cross_protocol.rows[0].protocol_id = PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1;
        let cross_protocol =
            norito::encode_canonical(&cross_protocol).expect("canonical cross-protocol mutation");
        assert_eq!(
            validate_privacy_exact12_fixture_bundle_v1(&cross_protocol),
            Status::InvalidBundle
        );

        let mut stale_statement = bundle.clone();
        stale_statement.rows[0].statement_norito[0] ^= 1;
        let stale_statement =
            norito::encode_canonical(&stale_statement).expect("canonical stale-statement mutation");
        assert_eq!(
            validate_privacy_exact12_fixture_bundle_v1(&stale_statement),
            Status::InvalidBundle
        );

        let mut stale_envelope = bundle;
        let final_byte = stale_envelope.rows[11].envelope_norito.len() - 1;
        stale_envelope.rows[11].envelope_norito[final_byte] ^= 1;
        let stale_envelope =
            norito::encode_canonical(&stale_envelope).expect("canonical stale-envelope mutation");
        assert_eq!(
            validate_privacy_exact12_fixture_bundle_v1(&stale_envelope),
            Status::InvalidBundle
        );
    }

    #[test]
    #[ignore = "explicit regeneration helper for fixtures/privacy/exact12_v1.tsv"]
    fn emit_exact12_typed_envelope_fixture_rows() {
        for row in privacy_exact12_typed_envelope_rows_v1().expect("compiled exact12 semantics") {
            println!(
                "typed-envelope\t{}\t{}\t{}\t{}\t{}",
                row.protocol_id.canonical_label(),
                row.statement_variant,
                row.proof_variant,
                hex::encode(row.statement_digest),
                hex::encode(row.envelope_sha256)
            );
        }
    }

    #[test]
    #[ignore = "explicit complete regeneration helper for fixtures/privacy/exact12_v1.tsv"]
    fn emit_exact12_matrix_fixture() {
        let bytes = privacy_exact12_matrix_bytes_v1().expect("compiled exact12 matrix");
        print!(
            "{}",
            std::str::from_utf8(&bytes).expect("exact12 generator emits UTF-8")
        );
    }

    #[test]
    fn exact12_cross_sdk_matrix_binds_registry_routes_and_typed_envelopes() {
        let matrix = include_str!("../../../../../fixtures/privacy/exact12_v1.tsv");
        let generated =
            privacy_exact12_matrix_bytes_v1().expect("generate compiled exact12 matrix");
        assert_eq!(
            matrix.as_bytes(),
            generated,
            "checked-in exact12 matrix must be regenerated from compiled typed semantics"
        );
        assert!(matrix.ends_with('\n'), "matrix must end with one LF");
        assert!(!matrix.contains('\r'), "matrix must use canonical LF lines");
        assert!(
            matrix
                .strip_suffix('\n')
                .expect("terminal LF")
                .lines()
                .all(|line| !line.is_empty()),
            "matrix must not contain empty rows"
        );

        let mut matrix_version = None;
        let mut registry_sha256 = None;
        let mut protocols = Vec::new();
        let mut typed_envelopes = Vec::new();
        let mut retired = Vec::new();
        for (line_index, line) in matrix.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields = line.split('\t').collect::<Vec<_>>();
            match fields.as_slice() {
                ["matrix-version", version] => {
                    assert!(
                        matrix_version.replace(*version).is_none(),
                        "duplicate version"
                    );
                }
                ["registry-sha256", digest] => {
                    assert!(
                        registry_sha256.replace(*digest).is_none(),
                        "duplicate registry digest"
                    );
                }
                ["protocol", index, label, statement_variant, proof_variant] => {
                    protocols.push((
                        index.parse::<usize>().expect("decimal protocol index"),
                        *label,
                        *statement_variant,
                        *proof_variant,
                    ));
                }
                [
                    "typed-envelope",
                    label,
                    statement_variant,
                    proof_variant,
                    statement_digest,
                    envelope_sha256,
                ] => typed_envelopes.push((
                    *label,
                    *statement_variant,
                    *proof_variant,
                    *statement_digest,
                    *envelope_sha256,
                )),
                ["retired", label] => retired.push(*label),
                _ => panic!("malformed exact12 matrix row {}", line_index + 1),
            }
        }

        assert_eq!(matrix_version, Some("1"));
        assert_eq!(protocols.len(), PrivacyProtocolIdV1::COUNT);
        assert_eq!(typed_envelopes.len(), PrivacyProtocolIdV1::COUNT);
        assert!(!retired.is_empty());
        let semantic_rows =
            privacy_exact12_typed_envelope_rows_v1().expect("compiled exact12 semantics");
        assert_eq!(semantic_rows.len(), PrivacyProtocolIdV1::COUNT);
        let mut registry_preimage = String::new();
        let mut unique_labels = std::collections::BTreeSet::new();
        for (expected_index, (protocol, semantic)) in PrivacyProtocolIdV1::ALL
            .into_iter()
            .zip(&semantic_rows)
            .enumerate()
        {
            let (index, label, expected_statement_variant, expected_proof_variant) =
                protocols[expected_index];
            assert_eq!(index, expected_index);
            assert_eq!(label, protocol.canonical_label());
            assert_eq!(semantic.protocol_id, protocol);
            assert_eq!(
                PrivacyProtocolIdV1::from_canonical_label(label),
                Some(protocol)
            );
            assert!(unique_labels.insert(label), "duplicate protocol label");
            assert_eq!(semantic.statement_variant, expected_statement_variant);
            assert_eq!(semantic.proof_variant, expected_proof_variant);
            registry_preimage.push_str(label);
            registry_preimage.push('\n');
        }
        assert_eq!(
            hex::encode(Sha256::digest(registry_preimage.as_bytes())),
            registry_sha256.expect("registry digest")
        );

        for (semantic, row) in semantic_rows.iter().zip(typed_envelopes) {
            let (
                label,
                expected_statement_variant,
                expected_proof_variant,
                expected_statement_digest,
                expected_envelope_sha256,
            ) = row;
            assert_eq!(
                PrivacyProtocolIdV1::from_canonical_label(label),
                Some(semantic.protocol_id)
            );
            assert_eq!(semantic.statement_variant, expected_statement_variant);
            assert_eq!(semantic.proof_variant, expected_proof_variant);
            for expected_digest in [expected_statement_digest, expected_envelope_sha256] {
                assert_eq!(expected_digest.len(), 64);
                assert!(
                    expected_digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
                    "typed envelope digest must be canonical lowercase hex"
                );
                assert!(
                    expected_digest.bytes().any(|byte| byte != b'0'),
                    "typed envelope digest must not be the zero placeholder"
                );
            }
            assert_eq!(
                hex::encode(semantic.statement_digest),
                expected_statement_digest
            );
            assert_eq!(
                hex::encode(semantic.envelope_sha256),
                expected_envelope_sha256
            );
        }

        let mut unique_retired = std::collections::BTreeSet::new();
        for label in retired {
            assert!(unique_retired.insert(label), "duplicate retired label");
            assert!(
                PrivacyProtocolIdV1::from_canonical_label(label).is_none(),
                "retired label {label:?} must remain unrepresentable"
            );
        }
    }

    #[test]
    fn exact12_compiled_semantics_are_closed_unique_and_context_bound() {
        let rows = privacy_exact12_typed_envelope_rows_v1().expect("compiled exact12 semantics");
        assert_eq!(rows.len(), PrivacyProtocolIdV1::COUNT);
        assert_eq!(
            rows.iter().map(|row| row.protocol_id).collect::<Vec<_>>(),
            PrivacyProtocolIdV1::ALL.to_vec()
        );
        assert!(rows.iter().all(|row| {
            row.statement_variant == row.protocol_id.canonical_typed_variant_label()
                && row.proof_variant == row.protocol_id.canonical_typed_variant_label()
                && row.statement_digest != [0; 32]
                && row.envelope_sha256 != [0; 32]
        }));
        assert_eq!(
            rows.iter()
                .map(|row| row.statement_digest)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            PrivacyProtocolIdV1::COUNT
        );
        assert_eq!(
            rows.iter()
                .map(|row| row.envelope_sha256)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            PrivacyProtocolIdV1::COUNT
        );

        let mut mutated = statement_for(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0);
        let PrivacyStatementV1::ZkAcePqAuthorizationV0(statement) = &mut mutated else {
            unreachable!("closed first row is ZK-ACE")
        };
        statement.context.action_index = 1;
        let mutated_statement_digest = mutated.digest().expect("mutated statement digest");
        let mutated_envelope_sha256: [u8; 32] = Sha256::digest(
            norito::encode_canonical(&envelope(mutated)).expect("mutated canonical envelope"),
        )
        .into();
        assert_ne!(
            *mutated_statement_digest.as_bytes(),
            rows[0].statement_digest
        );
        assert_ne!(mutated_envelope_sha256, rows[0].envelope_sha256);
    }

    #[test]
    fn fixed_digest_types_are_exact_and_nonzero_checked() {
        macro_rules! check_type {
            ($type:ident, $seed:expr) => {{
                let value = $type::new(raw($seed));
                assert_eq!(value.as_bytes(), &raw($seed));
                assert_eq!(value.into_bytes(), raw($seed));
                assert!(!value.is_zero());
                assert!($type::new([0; 32]).is_zero());
                assert_fixed_width_norito(&value, &raw($seed));
                let encoded = value.encode();
                // Bare Norito encodes a fixed byte-array field as one
                // canonical compact-width prefix followed by exactly 32
                // bytes. No variable-length payload is admitted.
                assert_eq!(encoded.len(), 33);
                assert_eq!(
                    $type::decode(&mut encoded.as_slice()).expect("decode fixed value"),
                    value
                );
            }};
        }
        check_type!(PrivacyParameterIdV1, 1);
        check_type!(PrivacyParameterDigestV1, 2);
        check_type!(PrivacyVerifierDigestV1, 3);
        check_type!(PrivacyStatementSchemaDigestV1, 4);
        check_type!(PrivacyEngineManifestDigestV1, 5);
        check_type!(PrivacyStatementDigestV1, 6);
        check_type!(PrivacyTransactionIntentDigestV1, 17);
        check_type!(PrivacyOrchardPoolBootstrapDigestV1, 20);
        check_type!(PrivacyProofManagedPoolBootstrapDigestV1, 21);
        check_type!(PrivacyFcmpOutputIdV1, 22);
        check_type!(PrivacyFcmpKeyImageV1, 23);
        check_type!(PrivacyBootleLanternIssuerPolicyDigestV1, 18);
        check_type!(PrivacyNullifierV1, 7);
        check_type!(PrivacyCommitmentV1, 8);
        check_type!(PrivacyPoolIdV1, 9);
        check_type!(PrivacyZkAmsRegistryIdV1, 10);
        check_type!(PrivacyPolicyIdV1, 10);
        check_type!(PrivacyRootV1, 11);
        check_type!(PrivacyChallengeV1, 12);
        check_type!(PrivacyZkAmsPhcHashV1, 13);
        check_type!(PrivacyZkAmsSubjectCommitmentV1, 14);
        check_type!(PrivacyZkAmsCredentialNonceV1, 15);
        check_type!(PrivacyZkAmsIssuerPolicyRecordDigestV1, 16);
        check_type!(PrivacyZkAmsRegistryRecordDigestV1, 19);
    }

    #[test]
    fn all_statements_and_envelopes_roundtrip_and_validate() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        for statement in sample_statements() {
            statement.validate(&limits).expect("valid typed statement");
            let statement_bytes = norito::to_bytes(&statement).expect("frame statement");
            let decoded_statement: PrivacyStatementV1 =
                norito::decode_from_bytes(&statement_bytes).expect("decode statement");
            assert_eq!(decoded_statement, statement);
            assert_eq!(
                decoded_statement.digest().expect("decoded digest"),
                statement.digest().expect("original digest")
            );

            let envelope = envelope(statement);
            envelope
                .validate_with_limits(&limits)
                .expect("valid intrinsic envelope");
            let activation = activation(&envelope);
            activation.validate().expect("valid activation");
            envelope
                .validate_against_activation(&activation, &limits, 2)
                .expect("valid active envelope");

            let bytes = norito::to_bytes(&envelope).expect("frame envelope");
            let decoded: PrivacyProofEnvelopeV1 =
                norito::decode_from_bytes(&bytes).expect("decode envelope");
            assert_eq!(decoded, envelope);
        }
    }

    #[test]
    fn normalization_accessors_cover_every_statement_and_nested_proof_variant() {
        for mut statement in sample_statements() {
            let replacement = PrivacyTransactionIntentDigestV1::new(raw(231));
            statement.context_mut().transaction_intent_digest = replacement;
            assert_eq!(statement.context().transaction_intent_digest, replacement);
        }

        let mut batch =
            PrivacyProofV1::IrohaZkAmsV1(IrohaZkAmsProofV1::MaskedRelaxedSpartanBatchAdmission(
                PrivacyProofBytesV1::new(vec![1, 2, 3]),
            ));
        batch.bytes_mut().bytes.clear();
        assert!(batch.bytes().as_bytes().is_empty());

        let mut provisioning =
            PrivacyProofV1::IrohaZkAmsV1(IrohaZkAmsProofV1::Ristretto255LsagProvisionAccount(
                PrivacyProofBytesV1::new(vec![4, 5, 6]),
            ));
        provisioning.bytes_mut().bytes.clear();
        assert!(provisioning.bytes().as_bytes().is_empty());

        for mut proof in sample_statements()
            .into_iter()
            .map(envelope)
            .map(|envelope| envelope.proof)
        {
            proof.bytes_mut().bytes.clear();
            assert!(proof.bytes().as_bytes().is_empty());
        }
    }

    #[test]
    fn zk_ams_envelope_requires_the_proof_variant_for_its_exact_action() {
        let limits = PrivacyConsensusLimitsV1::taira_default();

        let batch_statement = statement_for(PrivacyProtocolIdV1::IrohaZkAmsV1);
        let mut batch_envelope = envelope(batch_statement);
        batch_envelope
            .validate_with_limits(&limits)
            .expect("batch masked Relaxed Spartan proof variant");
        batch_envelope.proof = PrivacyProofV1::IrohaZkAmsV1(
            IrohaZkAmsProofV1::Ristretto255LsagProvisionAccount(PrivacyProofBytesV1::new(vec![1])),
        );
        assert!(matches!(
            batch_envelope.validate_with_limits(&limits),
            Err(PrivacyProofEnvelopeValidationError::ZkAmsActionProofMismatch)
        ));

        let provision_statement = zk_ams_provision_statement(16);
        let mut provision_envelope = envelope(provision_statement);
        provision_envelope
            .validate_with_limits(&limits)
            .expect("provisioning LSAG proof variant");
        provision_envelope.proof =
            PrivacyProofV1::IrohaZkAmsV1(IrohaZkAmsProofV1::MaskedRelaxedSpartanBatchAdmission(
                PrivacyProofBytesV1::new(vec![1]),
            ));
        assert!(matches!(
            provision_envelope.validate_with_limits(&limits),
            Err(PrivacyProofEnvelopeValidationError::ZkAmsActionProofMismatch)
        ));
    }

    #[test]
    fn p256_wire_types_are_exact_width_and_closed() {
        let point = p256_point(9);
        assert_eq!(point.as_bytes().len(), 33);
        assert_fixed_width_norito(&point, point.as_bytes());
        let encoded = point.encode();
        assert_eq!(encoded.len(), 34);
        assert_eq!(
            PrivacyP256PointV1::decode(&mut encoded.as_slice()).expect("decode exact point"),
            point
        );
        assert!(PrivacyP256PointV1::decode(&mut [0x02; 32].as_slice()).is_err());
        assert!(PrivacyP256PointV1::decode(&mut [0x02; 34].as_slice()).is_err());

        let ciphertext = p256_ciphertext(10);
        let bytes = norito::to_bytes(&ciphertext).expect("frame ciphertext");
        let decoded: PrivacyP256CiphertextV1 =
            norito::decode_from_bytes(&bytes).expect("decode ciphertext");
        assert_eq!(decoded, ciphertext);

        for unknown in [2_u32, 3, u32::MAX] {
            assert!(
                PrivacyVeRangeBitLengthV1::decode(&mut unknown.to_le_bytes().as_slice()).is_err()
            );
        }
    }

    #[test]
    fn zk_ams_ristretto_wire_types_and_action_tags_are_closed() {
        let seed_key = zk_ams_seed_key(9);
        assert_eq!(seed_key.as_bytes().len(), 32);
        assert_fixed_width_norito(&seed_key, seed_key.as_bytes());

        let key_image = PrivacyZkAmsKeyImageV1::new(raw(10));
        assert_eq!(key_image.as_bytes().len(), 32);
        assert_fixed_width_norito(&key_image, key_image.as_bytes());
        let encoded = seed_key.encode();
        assert_eq!(encoded.len(), 33);
        assert_eq!(
            PrivacyZkAmsSeedPublicKeyV1::decode(&mut encoded.as_slice())
                .expect("decode exact seed key"),
            seed_key
        );
        assert!(PrivacyZkAmsSeedPublicKeyV1::decode(&mut [9; 31].as_slice()).is_err());
        assert!(PrivacyZkAmsSeedPublicKeyV1::decode(&mut [9; 33].as_slice()).is_err());

        let key_image = PrivacyZkAmsKeyImageV1::new(raw(10));
        assert_eq!(key_image.encode().len(), 33);
        assert!(PrivacyZkAmsKeyImageV1::decode(&mut key_image.encode().as_slice()).is_ok());

        for unknown in [2_u32, 3, u32::MAX] {
            assert!(PrivacyZkAmsActionV1::decode(&mut unknown.to_le_bytes().as_slice()).is_err());
            assert!(IrohaZkAmsProofV1::decode(&mut unknown.to_le_bytes().as_slice()).is_err());
        }
    }

    #[test]
    fn context_rejects_unusable_chain_ids_and_action_indexes() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let mut value = context();
        value.chain_id = ChainId::from("");
        assert!(matches!(
            value.validate(&limits),
            Err(PrivacyStatementValidationError::InvalidChainIdLength { bytes: 0, .. })
        ));

        value = context();
        value.chain_id = ChainId::from("x".repeat(255));
        value.validate(&limits).expect("255-byte chain id");
        value.chain_id = ChainId::from("x".repeat(256));
        assert!(matches!(
            value.validate(&limits),
            Err(PrivacyStatementValidationError::InvalidChainIdLength { bytes: 256, .. })
        ));

        value = context();
        value.action_index = 1;
        assert!(matches!(
            value.validate(&limits),
            Err(PrivacyStatementValidationError::ActionIndexOutOfBounds {
                index: 1,
                max_actions: 1
            })
        ));

        value = context();
        value.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new([0; 32]);
        assert_eq!(
            value.validate(&limits),
            Err(PrivacyStatementValidationError::ZeroTransactionIntentDigest)
        );
    }

    #[test]
    fn native_consensus_binding_roundtrips_only_in_the_canonical_wire_shape() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let binding = PrivacyNativeConsensusBindingV1::new(&context(), raw(200), &limits)
            .expect("construct canonical native consensus binding");
        binding.validate(&limits).expect("validate binding");
        binding
            .validate_against_context(&context(), &limits)
            .expect("binding matches its statement context");

        let canonical = norito::encode_canonical(&binding).expect("encode canonical binding");
        assert_eq!(
            norito::decode_canonical::<PrivacyNativeConsensusBindingV1>(&canonical)
                .expect("decode canonical binding"),
            binding
        );

        let mut truncated = canonical.clone();
        truncated.pop();
        assert!(
            norito::decode_canonical::<PrivacyNativeConsensusBindingV1>(&truncated).is_err(),
            "truncated binding must fail closed"
        );

        let mut trailing = canonical;
        trailing.push(0);
        assert!(
            norito::decode_canonical::<PrivacyNativeConsensusBindingV1>(&trailing).is_err(),
            "trailing binding bytes must fail closed"
        );

        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::core::to_bytes(&binding).expect("encode alternate-layout binding")
        };
        assert_ne!(
            alternate,
            norito::encode_canonical(&binding).expect("canonical")
        );
        assert!(matches!(
            norito::decode_canonical::<PrivacyNativeConsensusBindingV1>(&alternate),
            Err(norito::Error::NonCanonicalEncoding)
        ));

        let json = norito::json::to_json(&binding).expect("encode binding JSON");
        assert_eq!(
            norito::json::from_json::<PrivacyNativeConsensusBindingV1>(&json)
                .expect("decode canonical binding JSON"),
            binding
        );
        let prefix = json.strip_suffix('}').expect("binding JSON is an object");
        assert!(
            norito::json::from_json::<PrivacyNativeConsensusBindingV1>(&format!(
                "{prefix},\"legacy_genesis\":true}}"
            ))
            .is_err(),
            "unknown legacy JSON fields must fail closed"
        );
    }

    #[test]
    fn native_consensus_binding_digest_changes_on_every_consensus_axis() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let binding = PrivacyNativeConsensusBindingV1::new(&context(), raw(200), &limits)
            .expect("construct canonical native consensus binding");
        let expected = binding.digest().expect("digest canonical binding");

        let mut mutations = Vec::new();

        let mut mutated = binding.clone();
        mutated.chain_id = ChainId::from("another-privacy-chain");
        mutations.push(("chain_id", mutated));

        let mut mutated = binding.clone();
        mutated.genesis_hash = raw(201);
        mutations.push(("genesis_hash", mutated));

        let mut mutated = binding.clone();
        mutated.action_index = 1;
        mutations.push(("action_index", mutated));

        let mut mutated = binding.clone();
        mutated.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new(raw(202));
        mutations.push(("transaction_intent_digest", mutated));

        let mut mutated = binding.clone();
        mutated.parameter_id = PrivacyParameterIdV1::new(raw(203));
        mutations.push(("parameter_id", mutated));

        let mut mutated = binding.clone();
        mutated.parameter_digest = PrivacyParameterDigestV1::new(raw(204));
        mutations.push(("parameter_digest", mutated));

        let mut mutated = binding.clone();
        mutated.verifier_digest = PrivacyVerifierDigestV1::new(raw(205));
        mutations.push(("verifier_digest", mutated));

        let mut mutated = binding.clone();
        mutated.statement_schema_digest = PrivacyStatementSchemaDigestV1::new(raw(206));
        mutations.push(("statement_schema_digest", mutated));

        let mut mutated = binding;
        mutated.engine_manifest_digest = PrivacyEngineManifestDigestV1::new(raw(207));
        mutations.push(("engine_manifest_digest", mutated));

        for (axis, mutation) in mutations {
            assert_ne!(
                mutation.digest().expect("digest binding mutation"),
                expected,
                "changing {axis} must change the native consensus-binding digest"
            );
        }
    }

    #[test]
    fn native_consensus_binding_rejects_zero_genesis_and_unusable_chain_ids() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        assert_eq!(
            PrivacyNativeConsensusBindingV1::new(&context(), [0; 32], &limits),
            Err(PrivacyNativeConsensusBindingValidationErrorV1::ZeroGenesisHash)
        );

        let mut invalid_context = context();
        invalid_context.chain_id = ChainId::from("");
        assert!(matches!(
            PrivacyNativeConsensusBindingV1::new(&invalid_context, raw(200), &limits),
            Err(
                PrivacyNativeConsensusBindingValidationErrorV1::InvalidContext(
                    PrivacyStatementValidationError::InvalidChainIdLength { bytes: 0, .. }
                )
            )
        ));

        invalid_context = context();
        invalid_context.chain_id = ChainId::from("x".repeat(256));
        assert!(matches!(
            PrivacyNativeConsensusBindingV1::new(&invalid_context, raw(200), &limits),
            Err(
                PrivacyNativeConsensusBindingValidationErrorV1::InvalidContext(
                    PrivacyStatementValidationError::InvalidChainIdLength { bytes: 256, .. }
                )
            )
        ));

        invalid_context.chain_id = ChainId::from("x".repeat(255));
        PrivacyNativeConsensusBindingV1::new(&invalid_context, raw(200), &limits)
            .expect("maximum-length chain id remains canonical");
    }

    #[test]
    fn native_consensus_binding_rejects_every_statement_context_substitution() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let base_context = context();
        let binding = PrivacyNativeConsensusBindingV1::new(&base_context, raw(200), &limits)
            .expect("construct canonical native consensus binding");

        let mut substitutions = Vec::new();

        let mut substituted = base_context.clone();
        substituted.chain_id = ChainId::from("substituted-chain");
        substitutions.push((
            "chain_id",
            substituted,
            PrivacyNativeConsensusBindingValidationErrorV1::ChainIdMismatch,
        ));

        let mut substituted = base_context.clone();
        substituted.action_index = 1;
        substitutions.push((
            "action_index",
            substituted,
            PrivacyNativeConsensusBindingValidationErrorV1::ActionIndexMismatch,
        ));

        let mut substituted = base_context.clone();
        substituted.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new(raw(210));
        substitutions.push((
            "transaction_intent_digest",
            substituted,
            PrivacyNativeConsensusBindingValidationErrorV1::TransactionIntentDigestMismatch,
        ));

        let mut substituted = base_context.clone();
        substituted.parameter_id = PrivacyParameterIdV1::new(raw(211));
        substitutions.push((
            "parameter_id",
            substituted,
            PrivacyNativeConsensusBindingValidationErrorV1::ParameterIdMismatch,
        ));

        let mut substituted = base_context.clone();
        substituted.parameter_digest = PrivacyParameterDigestV1::new(raw(212));
        substitutions.push((
            "parameter_digest",
            substituted,
            PrivacyNativeConsensusBindingValidationErrorV1::ParameterDigestMismatch,
        ));

        let mut substituted = base_context.clone();
        substituted.verifier_digest = PrivacyVerifierDigestV1::new(raw(213));
        substitutions.push((
            "verifier_digest",
            substituted,
            PrivacyNativeConsensusBindingValidationErrorV1::VerifierDigestMismatch,
        ));

        let mut substituted = base_context.clone();
        substituted.statement_schema_digest = PrivacyStatementSchemaDigestV1::new(raw(214));
        substitutions.push((
            "statement_schema_digest",
            substituted,
            PrivacyNativeConsensusBindingValidationErrorV1::StatementSchemaDigestMismatch,
        ));

        let mut substituted = base_context;
        substituted.engine_manifest_digest = PrivacyEngineManifestDigestV1::new(raw(215));
        substitutions.push((
            "engine_manifest_digest",
            substituted,
            PrivacyNativeConsensusBindingValidationErrorV1::EngineManifestDigestMismatch,
        ));

        for (axis, substituted, expected) in substitutions {
            assert_eq!(
                binding.validate_against_context(&substituted, &limits),
                Err(expected),
                "substituted {axis} must fail closed"
            );
        }

        let mut zero_genesis = binding;
        zero_genesis.genesis_hash = [0; 32];
        assert_eq!(
            zero_genesis.validate_against_context(&context(), &limits),
            Err(PrivacyNativeConsensusBindingValidationErrorV1::ZeroGenesisHash)
        );
    }

    #[test]
    fn privacy_context_statement_proof_and_envelope_json_are_closed() {
        let context = context();
        let context_json = norito::json::to_json(&context).expect("encode privacy context JSON");
        let context_prefix = context_json
            .strip_suffix('}')
            .expect("privacy context JSON is an object");
        assert!(
            norito::json::from_json::<PrivacyStatementContextV1>(&format!(
                "{context_prefix},\"legacy_context\":true}}"
            ))
            .is_err()
        );

        let envelope = envelope(statement_for(
            PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
        ));
        let envelope_json = norito::json::to_json(&envelope).expect("encode privacy envelope JSON");
        let decoded: PrivacyProofEnvelopeV1 =
            norito::json::from_json(&envelope_json).expect("decode canonical envelope JSON");
        assert_eq!(decoded, envelope);
        let envelope_prefix = envelope_json
            .strip_suffix('}')
            .expect("privacy envelope JSON is an object");
        assert!(
            norito::json::from_json::<PrivacyProofEnvelopeV1>(&format!(
                "{envelope_prefix},\"legacy_envelope\":true}}"
            ))
            .is_err()
        );

        let statement_json =
            norito::json::to_json(&envelope.statement).expect("encode typed statement JSON");
        let statement_prefix = statement_json
            .strip_suffix('}')
            .expect("typed statement JSON is an object");
        assert!(
            norito::json::from_json::<PrivacyStatementV1>(&format!(
                "{statement_prefix},\"legacy_statement\":true}}"
            ))
            .is_err()
        );

        let proof_json = norito::json::to_json(&envelope.proof).expect("encode typed proof JSON");
        let proof_prefix = proof_json
            .strip_suffix('}')
            .expect("typed proof JSON is an object");
        assert!(
            norito::json::from_json::<PrivacyProofV1>(&format!(
                "{proof_prefix},\"legacy_proof\":true}}"
            ))
            .is_err()
        );
    }

    #[test]
    fn first_release_statements_reject_nested_unknown_json_fields() {
        for protocol_id in PrivacyProtocolIdV1::ALL {
            let statement = statement_for(protocol_id);
            let canonical =
                norito::json::to_json(&statement).expect("encode private-transfer statement JSON");
            let nested_prefix = canonical
                .strip_suffix("}}")
                .expect("tagged statement ends with nested and outer objects");
            let hostile = format!("{nested_prefix},\"legacy_transfer\":true}}}}");
            assert!(
                norito::json::from_json::<PrivacyStatementV1>(&hostile).is_err(),
                "nested unknown field must fail for {protocol_id:?}"
            );
        }
        for (protocol_id, removed_field) in [
            (PrivacyProtocolIdV1::ZkAcePqAuthorizationV0, "fee"),
            (PrivacyProtocolIdV1::OrchardHalo2ActionsV1, "fee"),
            (PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1, "fee"),
            (PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1, "fee"),
            (PrivacyProtocolIdV1::PqMaspStarkV0, "fee"),
            (
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                "next_state_root",
            ),
            (
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                "next_state_root_epoch",
            ),
            (PrivacyProtocolIdV1::PqMaspStarkV0, "next_anchor"),
            (PrivacyProtocolIdV1::PqMaspStarkV0, "next_anchor_epoch"),
        ] {
            let canonical = norito::json::to_json(&statement_for(protocol_id))
                .expect("encode validator-derived-successor statement");
            let nested_prefix = canonical
                .strip_suffix("}}")
                .expect("tagged statement ends with nested and outer objects");
            let hostile = format!("{nested_prefix},\"{removed_field}\":null}}}}");
            assert!(
                norito::json::from_json::<PrivacyStatementV1>(&hostile).is_err(),
                "removed caller-selected field `{removed_field}` must not decode"
            );
        }

        let authorization = norito::json::to_json(&PrivacyPqAuthorizationProfileV1::MlDsa65)
            .expect("encode PQ authorization profile");
        let authorization_prefix = authorization
            .strip_suffix('}')
            .expect("PQ authorization profile is an object");
        assert!(
            norito::json::from_json::<PrivacyPqAuthorizationProfileV1>(&format!(
                "{authorization_prefix},\"legacy\":null}}"
            ))
            .is_err()
        );

        let encryption =
            norito::json::to_json(&PrivacyPqNoteEncryptionProfileV1::MlKem768XChaCha20Poly1305)
                .expect("encode PQ note-encryption profile");
        let encryption_prefix = encryption
            .strip_suffix('}')
            .expect("PQ note-encryption profile is an object");
        assert!(
            norito::json::from_json::<PrivacyPqNoteEncryptionProfileV1>(&format!(
                "{encryption_prefix},\"legacy\":null}}"
            ))
            .is_err()
        );

        let encrypted_note =
            norito::json::to_json(&encrypted_output(31, 32)).expect("encode encrypted note");
        let encrypted_note_prefix = encrypted_note
            .strip_suffix('}')
            .expect("encrypted note is an object");
        assert!(
            norito::json::from_json::<PrivacyEncryptedOutputV1>(&format!(
                "{encrypted_note_prefix},\"legacy_ciphertext\":null}}"
            ))
            .is_err(),
            "nested private-note encrypted output must reject unknown fields"
        );

        let output = fcmp_output(33);
        let fcmp_encrypted_output = PrivacyFcmpEncryptedOutputV1 {
            recipient: PrivacyRecipientIdV1::new(raw(34)),
            ephemeral_public_key: PrivacyEncryptionKeyV1::new(raw(35)),
            output_id: output.output_id(),
            ciphertext: vec![0xA5],
        };
        let fcmp_encrypted_output_json =
            norito::json::to_json(&fcmp_encrypted_output).expect("encode FCMP++ encrypted output");
        let fcmp_encrypted_output_prefix = fcmp_encrypted_output_json
            .strip_suffix('}')
            .expect("FCMP++ encrypted output is an object");
        assert!(
            norito::json::from_json::<PrivacyFcmpEncryptedOutputV1>(&format!(
                "{fcmp_encrypted_output_prefix},\"legacy_output_id\":null}}"
            ))
            .is_err(),
            "nested FCMP++ encrypted output must reject unknown fields"
        );
    }

    #[test]
    fn taira_consensus_limits_reject_zero_overflow_and_inconsistent_profiles() {
        let defaults = PrivacyConsensusLimitsV1::taira_default();
        defaults.validate().expect("Taira defaults");
        assert_eq!(
            defaults.max_actions_per_transaction,
            TAIRA_PRIVACY_MAX_ACTIONS_PER_TRANSACTION_V1
        );
        assert_eq!(defaults.max_proof_bytes_per_action, 9 * 1024 * 1024);
        assert_eq!(defaults.max_action_bytes, 9 * 1024 * 1024);
        assert_eq!(defaults.max_privacy_bytes_per_transaction, 9 * 1024 * 1024);
        assert_eq!(defaults.max_privacy_bytes_per_block, 18 * 1024 * 1024);
        assert_eq!(
            defaults.max_commitments_per_action,
            TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1
        );

        let invalid = [
            {
                let mut value = defaults;
                value.max_actions_per_transaction = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_actions_per_block = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_proof_bytes_per_action = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_action_bytes = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_privacy_bytes_per_transaction = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_privacy_bytes_per_block = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_statement_and_encrypted_output_bytes_per_transaction = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_nullifiers_per_action = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_commitments_per_action = 0;
                value
            },
            {
                let mut value = defaults;
                value.retained_root_count = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_commitments_per_action = TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1 + 1;
                value
            },
            {
                let mut value = defaults;
                value.max_action_bytes = defaults.max_proof_bytes_per_action - 1;
                value
            },
        ];
        for value in invalid {
            assert!(
                value.validate().is_err(),
                "mutated limits must fail: {value:?}"
            );
        }

        let hard_maximum_mutations: [(PrivacyLimitFieldV1, u32, fn(&mut PrivacyConsensusLimitsV1));
            4] = [
            (
                PrivacyLimitFieldV1::ProofBytesPerAction,
                TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1,
                |value| value.max_proof_bytes_per_action += 1,
            ),
            (
                PrivacyLimitFieldV1::ActionBytes,
                TAIRA_PRIVACY_MAX_ACTION_BYTES_V1,
                |value| value.max_action_bytes += 1,
            ),
            (
                PrivacyLimitFieldV1::PrivacyBytesPerTransaction,
                TAIRA_PRIVACY_MAX_BYTES_PER_TRANSACTION_V1,
                |value| value.max_privacy_bytes_per_transaction += 1,
            ),
            (
                PrivacyLimitFieldV1::PrivacyBytesPerBlock,
                TAIRA_PRIVACY_MAX_BYTES_PER_BLOCK_V1,
                |value| value.max_privacy_bytes_per_block += 1,
            ),
        ];
        for (field, hard_max, mutate) in hard_maximum_mutations {
            let mut value = defaults;
            mutate(&mut value);
            assert_eq!(
                value.validate(),
                Err(PrivacyConsensusLimitsValidationError::ExceedsHardMaximum {
                    field,
                    value: hard_max + 1,
                    hard_max,
                })
            );
        }
    }

    #[test]
    fn privacy_proof_payload_admits_exact_nine_mib_and_rejects_cap_plus_one() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let maximum =
            usize::try_from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1).expect("cap fits usize");
        let mut proof = PrivacyProofBytesV1::new(vec![0xA5; maximum]);
        proof.validate(&limits).expect("exact 9 MiB proof payload");

        proof.bytes.push(0x5A);
        assert_eq!(
            proof.validate(&limits),
            Err(PrivacyProofValidationError::TooLarge {
                bytes: u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1) + 1,
                max: TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1,
            })
        );
    }

    type ConsensusLimitMutationV1 = (PrivacyLimitFieldV1, fn(&mut PrivacyConsensusLimitsV1));

    #[test]
    fn consensus_limit_tightening_is_strict_and_rejects_every_component_increase() {
        let current = PrivacyConsensusLimitsV1 {
            max_actions_per_transaction: 1,
            max_actions_per_block: 1,
            max_proof_bytes_per_action: 1_024,
            max_action_bytes: 2_048,
            max_privacy_bytes_per_transaction: 4_096,
            max_privacy_bytes_per_block: 8_192,
            max_statement_and_encrypted_output_bytes_per_transaction: 1_024,
            max_nullifiers_per_action: 4,
            max_commitments_per_action: 4,
            retained_root_count: 100,
        };
        current.validate().expect("lower valid current profile");

        assert!(matches!(
            current.validate_tightening_to(&current),
            Err(PrivacyConsensusLimitsTighteningErrorV1::NoChange)
        ));
        let mut strict = current;
        strict.retained_root_count -= 1;
        current
            .validate_tightening_to(&strict)
            .expect("one component may be lowered");

        let mutations: [ConsensusLimitMutationV1; 10] = [
            (PrivacyLimitFieldV1::ActionsPerTransaction, |value| {
                value.max_actions_per_transaction += 1
            }),
            (PrivacyLimitFieldV1::ActionsPerBlock, |value| {
                value.max_actions_per_block += 1;
            }),
            (PrivacyLimitFieldV1::ProofBytesPerAction, |value| {
                value.max_proof_bytes_per_action += 1;
            }),
            (PrivacyLimitFieldV1::ActionBytes, |value| {
                value.max_action_bytes += 1;
            }),
            (PrivacyLimitFieldV1::PrivacyBytesPerTransaction, |value| {
                value.max_privacy_bytes_per_transaction += 1
            }),
            (PrivacyLimitFieldV1::PrivacyBytesPerBlock, |value| {
                value.max_privacy_bytes_per_block += 1;
            }),
            (
                PrivacyLimitFieldV1::StatementAndEncryptedOutputBytesPerTransaction,
                |value| value.max_statement_and_encrypted_output_bytes_per_transaction += 1,
            ),
            (PrivacyLimitFieldV1::NullifiersPerAction, |value| {
                value.max_nullifiers_per_action += 1;
            }),
            (PrivacyLimitFieldV1::CommitmentsPerAction, |value| {
                value.max_commitments_per_action += 1;
            }),
            (PrivacyLimitFieldV1::RetainedRootCount, |value| {
                value.retained_root_count += 1;
            }),
        ];
        for (field, mutate) in mutations {
            let mut candidate = current;
            mutate(&mut candidate);
            let error = current
                .validate_tightening_to(&candidate)
                .expect_err("an increased component must fail closed");
            if field == PrivacyLimitFieldV1::ActionsPerTransaction {
                assert!(matches!(
                    error,
                    PrivacyConsensusLimitsTighteningErrorV1::InvalidNext(
                        PrivacyConsensusLimitsValidationError::ExceedsHardMaximum {
                            field: PrivacyLimitFieldV1::ActionsPerTransaction,
                            ..
                        }
                    )
                ));
            } else {
                assert!(matches!(
                    error,
                    PrivacyConsensusLimitsTighteningErrorV1::Increase {
                        field: actual,
                        ..
                    } if actual == field
                ));
            }
        }

        let mut mixed = strict;
        mixed.max_actions_per_block += 1;
        assert!(matches!(
            current.validate_tightening_to(&mixed),
            Err(PrivacyConsensusLimitsTighteningErrorV1::Increase {
                field: PrivacyLimitFieldV1::ActionsPerBlock,
                ..
            })
        ));
    }

    #[test]
    fn consensus_policy_schedule_enforces_exact_notice_and_snapshot_boundaries() {
        let current_limits = PrivacyConsensusLimitsV1::taira_default();
        let mut next_limits = current_limits;
        next_limits.max_actions_per_block -= 1;
        next_limits.retained_root_count -= 1;
        let valid = PrivacyConsensusPolicyTighteningV1 {
            scheduled_at_height: 100,
            effective_at_height: 100 + MIN_PRIVACY_POLICY_DELAY_BLOCKS_V1,
            next_limits,
        };
        valid
            .validate_against(&current_limits)
            .expect("exact +300 schedule");

        for invalid in [
            PrivacyConsensusPolicyTighteningV1 {
                scheduled_at_height: 0,
                ..valid
            },
            PrivacyConsensusPolicyTighteningV1 {
                effective_at_height: 99,
                ..valid
            },
            PrivacyConsensusPolicyTighteningV1 {
                effective_at_height: 100,
                ..valid
            },
            PrivacyConsensusPolicyTighteningV1 {
                effective_at_height: valid.effective_at_height - 1,
                ..valid
            },
            PrivacyConsensusPolicyTighteningV1 {
                scheduled_at_height: u64::MAX - 100,
                effective_at_height: u64::MAX,
                ..valid
            },
        ] {
            assert!(
                invalid.validate_against(&current_limits).is_err(),
                "invalid schedule must reject: {invalid:?}"
            );
        }

        let policy = PrivacyConsensusPolicyV1 {
            current_limits,
            pending_tightening: Some(valid),
        };
        assert!(matches!(
            policy.validate_at_committed_height(99),
            Err(
                PrivacyPolicyValidationErrorV1::PendingScheduledAfterCommitted {
                    scheduled_at_height: 100,
                    committed_height: 99
                }
            )
        ));
        policy
            .validate_at_committed_height(100)
            .expect("schedule exists in its admitting committed block");
        policy
            .validate_at_committed_height(valid.effective_at_height - 1)
            .expect("effective E remains pending in committed E-1");
        assert!(matches!(
            policy.validate_at_committed_height(valid.effective_at_height),
            Err(PrivacyPolicyValidationErrorV1::PendingNotFuture {
                effective_at_height,
                committed_height
            }) if effective_at_height == valid.effective_at_height
                && committed_height == valid.effective_at_height
        ));
        assert_eq!(
            policy.admission_retained_root_count(),
            next_limits.retained_root_count
        );
    }

    #[test]
    fn protocol_limit_schedule_rejects_bad_timing_mismatch_increase_and_noop() {
        let current = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 8,
            },
        );
        let next = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 7,
            },
        );
        let valid = PrivacyProtocolLimitsTighteningV1 {
            scheduled_at_height: 25,
            effective_at_height: 25 + MIN_PRIVACY_POLICY_DELAY_BLOCKS_V1,
            next_limits: next,
        };
        valid
            .validate_against(&current)
            .expect("exact delayed protocol tightening");

        assert!(matches!(
            PrivacyProtocolLimitsTighteningV1 {
                next_limits: current,
                ..valid
            }
            .validate_against(&current),
            Err(PrivacyProtocolLimitsTighteningValidationErrorV1::NoChange)
        ));
        assert!(matches!(
            PrivacyProtocolLimitsTighteningV1 {
                effective_at_height: valid.effective_at_height - 1,
                ..valid
            }
            .validate_against(&current),
            Err(PrivacyProtocolLimitsTighteningValidationErrorV1::Schedule(
                PrivacyPolicyValidationErrorV1::LeadTimeTooShort { .. }
            ))
        ));
        assert!(matches!(
            PrivacyProtocolLimitsTighteningV1 {
                next_limits: PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(
                    OrchardActivationLimitsV1 {
                        max_action_count: 1,
                    }
                ),
                ..valid
            }
            .validate_against(&current),
            Err(PrivacyProtocolLimitsTighteningValidationErrorV1::Limits(
                PrivacyProtocolActivationLimitsValidationError::ProtocolMismatch { .. }
            ))
        ));

        let lower_current = next;
        assert!(matches!(
            PrivacyProtocolLimitsTighteningV1 {
                next_limits: current,
                ..valid
            }
            .validate_against(&lower_current),
            Err(PrivacyProtocolLimitsTighteningValidationErrorV1::Limits(
                PrivacyProtocolActivationLimitsValidationError::ExceedsConfiguredCeiling {
                    field: PrivacyActivationLimitFieldV1::VeRangeAggregationCount,
                    value: 8,
                    ceiling: 7
                }
            ))
        ));
    }
