#[cfg(test)]
mod tests {
    use iroha_data_model::privacy::{
        AnonymousPgcActivationLimitsV1, PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
        PrivacyProposedLifecycleV1,
    };
    use iroha_schema::{Declaration, MetaMap, NamedFieldsMeta, TypeId};

    use super::*;

    struct SchemaOrderAb;

    impl TypeId for SchemaOrderAb {
        fn id() -> String {
            "privacy-test::CanonicalSchema".to_owned()
        }
    }

    impl IntoSchema for SchemaOrderAb {
        fn type_name() -> String {
            "CanonicalSchema".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            u32::update_schema_map(map);
            u64::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "alpha".to_owned(),
                        ty: core::any::TypeId::of::<u32>(),
                    },
                    Declaration {
                        name: "beta".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                ],
            }));
        }
    }

    struct SchemaOrderBa;

    impl TypeId for SchemaOrderBa {
        fn id() -> String {
            "privacy-test::CanonicalSchema".to_owned()
        }
    }

    impl IntoSchema for SchemaOrderBa {
        fn type_name() -> String {
            "CanonicalSchema".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            u32::update_schema_map(map);
            u64::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "beta".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                    Declaration {
                        name: "alpha".to_owned(),
                        ty: core::any::TypeId::of::<u32>(),
                    },
                ],
            }));
        }
    }

    struct SchemaRetyped;

    impl TypeId for SchemaRetyped {
        fn id() -> String {
            "privacy-test::CanonicalSchema".to_owned()
        }
    }

    impl IntoSchema for SchemaRetyped {
        fn type_name() -> String {
            "CanonicalSchema".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            u64::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "alpha".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                    Declaration {
                        name: "beta".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                ],
            }));
        }
    }

    struct SchemaEquivalentAliases;

    impl TypeId for SchemaEquivalentAliases {
        fn id() -> String {
            "privacy-test::EquivalentAliases".to_owned()
        }
    }

    impl IntoSchema for SchemaEquivalentAliases {
        fn type_name() -> String {
            "EquivalentAliases".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            String::update_schema_map(map);
            Box::<str>::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "owned".to_owned(),
                        ty: core::any::TypeId::of::<String>(),
                    },
                    Declaration {
                        name: "boxed".to_owned(),
                        ty: core::any::TypeId::of::<Box<str>>(),
                    },
                ],
            }));
        }
    }

    struct SchemaConflictLeft;

    impl TypeId for SchemaConflictLeft {
        fn id() -> String {
            "privacy-test::ConflictingAlias".to_owned()
        }
    }

    impl IntoSchema for SchemaConflictLeft {
        fn type_name() -> String {
            "ConflictingAlias".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            map.insert::<Self>(Metadata::Int(IntMode::FixedWidth));
        }
    }

    struct SchemaConflictRight;

    impl TypeId for SchemaConflictRight {
        fn id() -> String {
            "privacy-test::ConflictingAlias".to_owned()
        }
    }

    impl IntoSchema for SchemaConflictRight {
        fn type_name() -> String {
            "ConflictingAlias".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            map.insert::<Self>(Metadata::Bool);
        }
    }

    struct SchemaConflictingAliases;

    impl TypeId for SchemaConflictingAliases {
        fn id() -> String {
            "privacy-test::ConflictingAliases".to_owned()
        }
    }

    impl IntoSchema for SchemaConflictingAliases {
        fn type_name() -> String {
            "ConflictingAliases".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            SchemaConflictLeft::update_schema_map(map);
            SchemaConflictRight::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "left".to_owned(),
                        ty: core::any::TypeId::of::<SchemaConflictLeft>(),
                    },
                    Declaration {
                        name: "right".to_owned(),
                        ty: core::any::TypeId::of::<SchemaConflictRight>(),
                    },
                ],
            }));
        }
    }

    fn verange_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
            .expect("fixed VeRange parameters derive")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    #[cfg(feature = "zk-stark")]
    fn zk_ace_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0)
            .expect("fixed ZK-ACE profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn pgc_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
            .expect("fixed Anonymous-PGC parameters derive")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn jindo_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0)
            .expect("fixed Jindo parameters derive")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn vega_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0)
            .expect("fixed Vega profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn bootle_lantern_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_bootle_lantern_profile_material_v1()
            .expect("fixed Bootle/Lantern profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn orchard_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::OrchardHalo2ActionsV1)
            .expect("fixed Orchard profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn fcmp_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1)
            .expect("fixed FCMP++ profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn ivm_private_note_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1)
            .expect("fixed IVM private-note profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn pq_masp_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::PqMaspStarkV0)
            .expect("fixed PQ-MASP profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn zk_x509_activation() -> PrivacyProtocolActivationRecordV1 {
        zk_x509_release_candidate_profile_material_v1()
            .expect("release-pinned zk-X.509 candidate profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    #[test]
    fn semantic_parameter_labels_and_framed_note_profiles_cannot_drift() {
        assert_eq!(
            IVM_PRIVATE_NOTE_PARAMETER_SET_LABEL_V1,
            b"goldilocks-sha256-proof-managed-note-stark+private-note-vm16x8-tree32-v1"
        );
        assert_eq!(
            PQ_MASP_PARAMETER_SET_LABEL_V1,
            b"goldilocks-sha256-proof-managed-note-stark+pq-masp+mldsa65+mlkem768-v1"
        );
        #[cfg(feature = "zk-stark")]
        assert_eq!(
            ZK_ACE_PARAMETER_SET_LABEL_V1,
            b"goldilocks-poseidon2-transparent-stark-v1"
        );
        for stale_geometry in [
            b"mask255".as_slice(),
            b"mask111".as_slice(),
            b"three-lane".as_slice(),
            b"blowup32".as_slice(),
        ] {
            assert!(
                !IVM_PRIVATE_NOTE_PARAMETER_SET_LABEL_V1
                    .windows(stale_geometry.len())
                    .any(|window| window == stale_geometry)
            );
            assert!(
                !PQ_MASP_PARAMETER_SET_LABEL_V1
                    .windows(stale_geometry.len())
                    .any(|window| window == stale_geometry)
            );
            #[cfg(feature = "zk-stark")]
            assert!(
                !ZK_ACE_PARAMETER_SET_LABEL_V1
                    .windows(stale_geometry.len())
                    .any(|window| window == stale_geometry)
            );
        }
        let shared_digest: [u8; 32] =
            Sha256::digest(PROOF_MANAGED_NOTE_STARK_GEOMETRY_DESCRIPTOR_V1).into();
        assert_eq!(shared_digest, PROOF_MANAGED_NOTE_STARK_GEOMETRY_DIGEST_V1);
        assert_eq!(
            proof_managed_note_stark_profile_digest_v1(
                IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1
            ),
            IVM_PRIVATE_NOTE_STARK_PROFILE_DIGEST_V1
        );
        assert_eq!(
            proof_managed_note_stark_profile_digest_v1(PQ_MASP_STARK_PROFILE_DESCRIPTOR_V1),
            PQ_MASP_STARK_PROFILE_DIGEST_V1
        );
        assert!(
            IVM_PRIVATE_NOTE_MAX_PROOF_BYTES_V1
                < usize::try_from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
                    .expect("global proof cap fits usize"),
            "the independent private-note proof cap must remain below the governed global cap"
        );
        assert_eq!(
            PQ_MASP_MAX_AUTHORIZATION_PROOF_BYTES_V1,
            usize::try_from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
                .expect("global proof cap fits usize"),
            "the complete PQ-MASP authorization wire consumes the governed global cap"
        );
    }

    #[test]
    fn pq_masp_profile_binds_the_exact_wallet_and_verified_effect_schemas() {
        assert_eq!(
            PQ_MASP_WALLET_CIPHERTEXT_SCHEMA_V1,
            b"typed-output:recipient-id32+encapsulation-digest32+output-commitment32+ciphertext[PQE1+mlkem768-ciphertext1088+nonce24+xchacha20poly1305[PQN1+value-u128be+authorization-key-digest32+recipient-id32+nullifier-key-digest32+rho32+blinding32+memo-digest32]+tag16]|mlkem768-domain-kdf|aad:domain+asset-definition-id-u64be-length+norito+pool-id32+output-commitment32+recipient-id32+encapsulation-digest32"
        );
        assert_eq!(
            PQ_MASP_VERIFIED_EFFECT_SCHEMA_V1,
            b"namespace:norito|bootstrap-digest:32|asset-definition-id:norito|current-root:32|current-epoch:u64|next-root:32|next-epoch:u64|transition:pq-masp{ordered-nullifiers[32]+ordered-output-commitments[32]+validator-derived-successor-frontier}|value-balance:none"
        );
        for stale_field in [
            b"value-u128le".as_slice(),
            b"rseed32".as_slice(),
            b"anchor-epoch".as_slice(),
            b"ordered-encrypted-outputs".as_slice(),
            b"expiry-height".as_slice(),
        ] {
            assert!(
                !PQ_MASP_WALLET_CIPHERTEXT_SCHEMA_V1
                    .windows(stale_field.len())
                    .any(|window| window == stale_field)
                    && !PQ_MASP_VERIFIED_EFFECT_SCHEMA_V1
                        .windows(stale_field.len())
                        .any(|window| window == stale_field),
                "stale PQ-MASP profile field survived: {}",
                String::from_utf8_lossy(stale_field)
            );
        }

        let exact = compiled_pq_masp_profile_v1().expect("compiled PQ-MASP profile");
        for changed in [
            compiled_pq_masp_profile_v1_with_schemas(
                b"substituted-wallet-schema",
                PQ_MASP_VERIFIED_EFFECT_SCHEMA_V1,
            )
            .expect("structurally valid wallet-schema substitution"),
            compiled_pq_masp_profile_v1_with_schemas(
                PQ_MASP_WALLET_CIPHERTEXT_SCHEMA_V1,
                b"substituted-verified-effect-schema",
            )
            .expect("structurally valid effect-schema substitution"),
        ] {
            assert_eq!(changed.parameter_id, exact.parameter_id);
            assert_eq!(changed.parameter_digest, exact.parameter_digest);
            assert_ne!(changed.verifier_digest, exact.verifier_digest);
            assert_eq!(
                changed.statement_schema_digest,
                exact.statement_schema_digest
            );
            assert_ne!(changed.engine_manifest_digest, exact.engine_manifest_digest);
        }
    }

    #[test]
    fn local_compiled_profile_catalog_is_exact12_and_contains_no_governance_state() {
        let catalog = compiled_privacy_profile_catalog_v1().expect("compiled profile catalog");
        assert_eq!(catalog.version, PRIVACY_COMPILED_PROFILE_CATALOG_VERSION_V1);
        assert_eq!(catalog.protocols.len(), PrivacyProtocolIdV1::COUNT);
        assert!(
            catalog
                .protocols
                .iter()
                .map(|row| row.protocol_id)
                .eq(PrivacyProtocolIdV1::ALL)
        );

        let json = norito::json::to_json(&catalog).expect("catalog JSON");
        for forbidden in [
            "committed_height",
            "consensus_policy",
            "activation",
            "lifecycle",
        ] {
            assert!(
                !json.contains(forbidden),
                "local catalog must not expose governance field {forbidden}"
            );
        }
    }

    #[test]
    fn compiled_profile_catalog_cache_returns_owned_isolated_clones() {
        let canonical = compiled_privacy_profile_catalog_v1().expect("compiled profile catalog");
        canonical.validate().expect("canonical compiled catalog");
        assert_eq!(canonical.protocols.len(), PrivacyProtocolIdV1::COUNT);
        let canonical_archive =
            norito::encode_canonical(&canonical).expect("canonical compiled catalog archive");

        let mut caller_owned = canonical;
        caller_owned.protocols.rotate_left(1);
        assert!(
            caller_owned.validate().is_err(),
            "mutating one returned clone must make only that caller's copy noncanonical"
        );

        let subsequent =
            compiled_privacy_profile_catalog_v1().expect("subsequent compiled profile catalog");
        subsequent
            .validate()
            .expect("the cached canonical catalog must remain valid");
        assert_eq!(subsequent.protocols.len(), PrivacyProtocolIdV1::COUNT);
        assert!(
            subsequent
                .protocols
                .iter()
                .map(|row| row.protocol_id)
                .eq(PrivacyProtocolIdV1::ALL)
        );
        assert_eq!(
            norito::encode_canonical(&subsequent)
                .expect("subsequent canonical compiled catalog archive"),
            canonical_archive,
            "a caller mutation must not alias or modify the immutable cache"
        );
    }

    #[test]
    fn local_compiled_profile_catalog_archive_rejects_canonical_substitution() {
        use PrivacyCompiledProfileCatalogArchiveValidationStatusV1 as Status;

        let catalog = compiled_privacy_profile_catalog_v1().expect("compiled profile catalog");
        let archive = norito::encode_canonical(&catalog).expect("canonical catalog");
        assert_eq!(
            validate_local_privacy_compiled_profile_catalog_archive_v1(&archive),
            Status::Valid
        );

        let mut substituted = catalog;
        let profile = substituted
            .protocols
            .iter_mut()
            .find_map(|row| match &mut row.compiled_profile {
                PrivacyCompiledProfileResultV1::Available(profile) => Some(profile),
                PrivacyCompiledProfileResultV1::Unavailable(_) => None,
            })
            .expect("at least one compiled profile");
        let mut digest = *profile.parameter_digest.as_bytes();
        digest[0] ^= 0x80;
        profile.parameter_digest = PrivacyParameterDigestV1::new(digest);
        profile
            .validate()
            .expect("substituted profile remains structural");
        let substituted =
            norito::encode_canonical(&substituted).expect("canonical substituted catalog");
        assert_eq!(
            validate_privacy_compiled_profile_catalog_archive_v1(&substituted),
            Status::Valid,
            "the data-model validator establishes only canonical catalog structure"
        );
        assert_eq!(
            validate_local_privacy_compiled_profile_catalog_archive_v1(&substituted),
            Status::InvalidCatalog,
            "the local validator must reject a canonical profile substitution"
        );
    }

    #[test]
    fn only_governance_released_engines_have_compiled_profiles() {
        let available = PrivacyProtocolIdV1::ALL
            .into_iter()
            .filter(|protocol_id| compiled_privacy_profile_v1(*protocol_id).is_ok())
            .collect::<Vec<_>>();
        let mut expected = vec![
            #[cfg(feature = "zk-stark")]
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
            PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
            PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
            PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            PrivacyProtocolIdV1::PqMaspStarkV0,
        ];
        if require_activation_readiness_v1(zk_x509_activation_readiness_v1()).is_ok() {
            expected.push(PrivacyProtocolIdV1::IrohaZkX509StarkP256V0);
        }
        assert!(
            zk_x509_release_candidate_profile_material_v1().is_ok(),
            "X.509 candidate material must derive independently of governance release"
        );
        assert_eq!(available, expected);
    }

    #[test]
    fn ivm_private_note_profile_binds_distinct_proof_and_wallet_randomness_policies() {
        let exact = compiled_ivm_private_note_profile_v1().expect("compiled IVM profile");
        assert_ne!(
            TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
            CURVE_PROVER_RANDOMNESS_POLICY_V1
        );
        assert!(
            IVM_PRIVATE_NOTE_ENGINE_DESCRIPTOR_V1
                .windows(CURVE_PROVER_RANDOMNESS_POLICY_V1.len())
                .any(|window| window == CURVE_PROVER_RANDOMNESS_POLICY_V1)
        );

        let mut changed_proof_policy = TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1.to_vec();
        changed_proof_policy[0] ^= 1;
        let mut changed_wallet_policy = CURVE_PROVER_RANDOMNESS_POLICY_V1.to_vec();
        changed_wallet_policy[0] ^= 1;
        for changed in [
            compiled_ivm_private_note_profile_v1_with_randomness_policies(
                &changed_proof_policy,
                CURVE_PROVER_RANDOMNESS_POLICY_V1,
            )
            .expect("structurally valid proof-policy mutation"),
            compiled_ivm_private_note_profile_v1_with_randomness_policies(
                TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
                &changed_wallet_policy,
            )
            .expect("structurally valid wallet-policy mutation"),
        ] {
            assert_eq!(changed.parameter_id, exact.parameter_id);
            assert_ne!(changed.parameter_digest, exact.parameter_digest);
            assert_ne!(changed.verifier_digest, exact.verifier_digest);
            assert_eq!(
                changed.statement_schema_digest,
                exact.statement_schema_digest
            );
            assert_ne!(changed.engine_manifest_digest, exact.engine_manifest_digest);
        }
    }

    #[test]
    fn ivm_private_note_and_pq_masp_profiles_are_exact_bounded_and_mutation_closed() {
        let cases = [
            (
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                ivm_private_note_activation(),
                PrivacyProtocolActivationLimitsV1::IrohaIvmPrivateNoteStarkV1(
                    IvmPrivateNoteActivationLimitsV1 {
                        max_input_count: IVM_PRIVATE_NOTE_MAX_INPUTS_V1,
                        max_output_count: IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1,
                    },
                ),
            ),
            (
                PrivacyProtocolIdV1::PqMaspStarkV0,
                pq_masp_activation(),
                PrivacyProtocolActivationLimitsV1::PqMaspStarkV0(PqMaspActivationLimitsV1 {
                    max_input_count: PQ_MASP_MAX_INPUTS_V1,
                    max_output_count: PQ_MASP_MAX_OUTPUTS_V1,
                }),
            ),
        ];

        for (protocol_id, valid, expected_limits) in cases {
            let first = compiled_privacy_profile_v1(protocol_id).expect("compiled native profile");
            let second = compiled_privacy_profile_v1(protocol_id).expect("deterministic profile");
            assert_eq!(first, second);
            assert_eq!(
                first.proof_system_id,
                PrivacyProofSystemIdV1::StarkFriSha256Goldilocks
            );
            assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeGoldilocksStarkFri);
            assert_eq!(first.protocol_limits, expected_limits);
            for digest in [
                *first.parameter_id.as_bytes(),
                *first.parameter_digest.as_bytes(),
                *first.verifier_digest.as_bytes(),
                *first.statement_schema_digest.as_bytes(),
                *first.engine_manifest_digest.as_bytes(),
            ] {
                assert_ne!(digest, [0; 32]);
            }
            let expected_bindings = match protocol_id {
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => (
                    "b5db09ae42957802c502855459a102ba8e829bfb86a0356691455de0a08fbec0".to_owned(),
                    "a665cfcbea5576a1cf533997e575ebd49957ce320c483c019e784f8fc93457e1".to_owned(),
                    "5f2214526473a3b617e09c43dd9f48795f11d7f169bb645e76ce0693b0483abb".to_owned(),
                    "b30e388a3f3dbb6d2e93aa8c53a5df355238b763d6c3fcd766f7d0c3f0afca5f".to_owned(),
                    "99158955397f0aa94c2bae5285cb2e6f7602506366e6f583a6797ffaa77874d1".to_owned(),
                ),
                PrivacyProtocolIdV1::PqMaspStarkV0 => (
                    "10a8697291331061099a6c67eaeac3bc29f77aea951f2f2ad55ca29d0f816951".to_owned(),
                    "120ad9e6f616fdd05168a2dde5608654094a18b97bfc89ebedf86b7fbaf335b8".to_owned(),
                    "dc7c983c9b683ec2b4efc998408a59afd213272ac37bcee5720cf68a0f4516c9".to_owned(),
                    "4932c64b8f113632ba145e18ca5cc85496fbc96d103b19d712643348f3153727".to_owned(),
                    "e6cd364435e6ef1d85ef0a825b05cbf48a65ecf10e9f152d68935f84246c9601".to_owned(),
                ),
                _ => unreachable!("the test covers only IVM private note and PQ-MASP"),
            };
            assert_eq!(
                (
                    hex::encode(first.parameter_id.as_bytes()),
                    hex::encode(first.parameter_digest.as_bytes()),
                    hex::encode(first.verifier_digest.as_bytes()),
                    hex::encode(first.statement_schema_digest.as_bytes()),
                    hex::encode(first.engine_manifest_digest.as_bytes()),
                ),
                expected_bindings,
                "every consensus-critical {} binding is a pinned KAT",
                protocol_id.canonical_label(),
            );

            validate_compiled_privacy_activation_v1(&valid)
                .expect("exact compiled activation is accepted");
            let mutations: [(
                CompiledPrivacyProfileValidationErrorV1,
                fn(&mut PrivacyProtocolActivationRecordV1),
            ); 8] = [
                (
                    CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                    |record| record.proof_system_id = PrivacyProofSystemIdV1::Halo2IpaPasta,
                ),
                (
                    CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                    |record| record.engine_id = PrivacyEngineIdV1::NativeHalo2Orchard,
                ),
                (
                    CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                    |record| record.parameter_id.0[0] ^= 1,
                ),
                (
                    CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                    |record| record.parameter_digest.0[0] ^= 1,
                ),
                (
                    CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                    |record| record.verifier_digest.0[0] ^= 1,
                ),
                (
                    CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                    |record| record.statement_schema_digest.0[0] ^= 1,
                ),
                (
                    CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                    |record| record.engine_manifest_digest.0[0] ^= 1,
                ),
                (
                    CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                    |record| match &mut record.protocol_limits {
                        PrivacyProtocolActivationLimitsV1::IrohaIvmPrivateNoteStarkV1(limits) => {
                            limits.max_input_count += 1;
                        }
                        PrivacyProtocolActivationLimitsV1::PqMaspStarkV0(limits) => {
                            limits.max_output_count += 1;
                        }
                        _ => unreachable!("test covers only IVM private note and PQ-MASP"),
                    },
                ),
            ];
            for (expected, mutate) in mutations {
                let mut changed = valid;
                mutate(&mut changed);
                assert_eq!(
                    validate_compiled_privacy_activation_v1(&changed),
                    Err(expected)
                );
            }
        }
    }

    #[test]
    fn compiling_ivm_private_note_and_pq_masp_does_not_activate_their_lifecycles() {
        let snapshot = committed_privacy_capability_snapshot_v1(
            42,
            PrivacyConsensusPolicyV1::taira_default(),
            |_| None,
        )
        .expect("empty committed lifecycle state is valid");
        for protocol_id in [
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            PrivacyProtocolIdV1::PqMaspStarkV0,
        ] {
            let row = snapshot
                .protocols
                .iter()
                .find(|row| row.protocol_id == protocol_id)
                .expect("exact12 row");
            assert!(matches!(
                row.compiled_profile,
                PrivacyCompiledProfileResultV1::Available(_)
            ));
            assert_eq!(row.activation, None);
        }
    }

    #[test]
    fn fcmp_profile_is_deterministic_exact_bounded_and_mutation_closed() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1)
            .expect("compiled FCMP++");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1)
            .expect("compiled FCMP++");
        assert_eq!(first, second);
        assert_eq!(
            first.proof_system_id,
            PrivacyProofSystemIdV1::FcmpPlusPlusCurveTreeBulletproofs
        );
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeFcmpPlusPlus);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::MoneroFcmpPlusPlusV1(FcmpActivationLimitsV1 {
                max_input_count: FCMP_MAX_INPUTS_V1,
                max_output_count: FCMP_MAX_OUTPUTS_V1,
            })
        );
        assert_eq!(
            fcmp_plus_plus_wire_size_v1(
                FCMP_MAX_INPUTS_NATIVE_V1,
                FCMP_MAX_TREE_LAYERS_V1,
                FCMP_MAX_OUTPUTS_NATIVE_V1,
            )
            .expect("maximum FCMP++ wire"),
            FCMP_MAX_PROOF_WIRE_BYTES_V1
        );
        assert!(
            FCMP_MAX_PROOF_WIRE_BYTES_V1
                <= usize::try_from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
                    .expect("global proof cap fits usize")
        );
        for digest in [
            fcmp_compiled_profile_digest_v1(),
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "8a24198f13ce0dbe0f4747874def956dc15ca98f9308c29ed678afddbe989a04".to_owned(),
                "92ee53970444330e37716b98a9eb1c04d8e52eb1ffe08103fb2745cc1abc9a89".to_owned(),
                "5e83f32ed7edf764e50fc8cebf5b4d8b75cb9e42a296965514b033d49dae4ac4".to_owned(),
                "c1577ce5a4a22e089a2fd7547f7fea32b7b35808967149d0e7f96a2ecb8c4ba7".to_owned(),
                "fb5e94756f9f234641b27899b7fd63bb48f3b5f92c24266d76e6d4de16231b27".to_owned(),
            ),
            "every consensus-critical FCMP++ binding is a pinned KAT",
        );
        let mut mutated_randomness_policy = CURVE_PROVER_RANDOMNESS_POLICY_V1.to_vec();
        mutated_randomness_policy[0] ^= 1;
        let policy_mutation =
            compiled_fcmp_profile_v1_with_randomness_policy(&mutated_randomness_policy)
                .expect("structurally valid FCMP++ policy mutation");
        assert_eq!(policy_mutation.parameter_id, first.parameter_id);
        assert_ne!(policy_mutation.parameter_digest, first.parameter_digest);
        assert_ne!(policy_mutation.verifier_digest, first.verifier_digest);
        assert_eq!(
            policy_mutation.statement_schema_digest,
            first.statement_schema_digest
        );
        assert_ne!(
            policy_mutation.engine_manifest_digest,
            first.engine_manifest_digest
        );

        let valid = fcmp_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact FCMP++ activation");
        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 8] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| record.proof_system_id = PrivacyProofSystemIdV1::Halo2IpaPasta,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeHalo2Orchard,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                |record| {
                    record.protocol_limits =
                        PrivacyProtocolActivationLimitsV1::MoneroFcmpPlusPlusV1(
                            FcmpActivationLimitsV1 {
                                max_input_count: FCMP_MAX_INPUTS_V1 + 1,
                                max_output_count: FCMP_MAX_OUTPUTS_V1,
                            },
                        );
                },
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn bootle_lantern_profile_is_deterministic_complete_bounded_and_mutation_closed() {
        let first = compiled_bootle_lantern_profile_material_v1().expect("profile material");
        let second = compiled_bootle_lantern_profile_material_v1().expect("profile material");
        assert_eq!(first, second);
        assert_eq!(
            first.proof_system_id,
            PrivacyProofSystemIdV1::LanternLnp22ModuleLinearNorm
        );
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeLanternLnp22);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::IrohaBootleLanternAnoncredV1
        );
        assert_eq!(APPLICATION_RING_DEGREE_V1, 64);
        assert_eq!(
            APPLICATION_RING_DEGREE_V1,
            BOOTLE_LANTERN_MODEL_RING_DEGREE_V1
        );
        assert_eq!(
            BOOTLE_LANTERN_APPLICATION_MODULUS_V1,
            BOOTLE_LANTERN_MODEL_APPLICATION_MODULUS_V1
        );
        assert_eq!(APPLICATION_ROWS_V1, 8);
        assert_eq!(APPLICATION_ROWS_V1, BOOTLE_LANTERN_MODEL_ATTRIBUTE_COUNT_V1);
        assert_eq!(APPLICATION_WITNESS_POLYNOMIALS_V1, 48);
        assert_eq!(
            BOOTLE_LANTERN_PARAMETER_SET_LABEL_V1,
            b"falcon512-ntru-r512-as-r64-rank8-interleaved"
        );
        for required in [
            &b"BLNS-specialization-no-main-construction-reduction"[..],
            &b"rust-fn-dsa-workspace-0.3-daf14859b5aa3f8d75c42966ba7de83e6eb59997"[..],
        ] {
            assert!(
                BOOTLE_LANTERN_IMPLEMENTATION_PROVENANCE_V1
                    .windows(required.len())
                    .any(|window| window == required),
                "implementation provenance omitted {}",
                String::from_utf8_lossy(required)
            );
        }
        for (descriptor, required) in [
            (
                BOOTLE_LANTERN_ISSUER_PARAMETER_SCHEMA_V1,
                &b"H_i[j]=h[8*j+i]"[..],
            ),
            (
                BOOTLE_LANTERN_RELATION_SCHEMA_V1,
                &b"A_r*r+A_tau*tau+A_m*m+scope-s1-H*s2=0"[..],
            ),
            (
                BOOTLE_LANTERN_CREDENTIAL_SCOPE_SCHEMA_V1,
                &b"excluded:action-index+transaction-intent-digest"[..],
            ),
            (
                BOOTLE_LANTERN_BLIND_ISSUANCE_SCHEMA_V1,
                &b"atomic-height-aware-Fresh-to-Processing-before-one-master64"[..],
            ),
            (
                BOOTLE_LANTERN_NATIVE_PRODUCER_SCHEMA_V1,
                &b"cached-completed-replay-does-not-touch-rng"[..],
            ),
            (
                BOOTLE_LANTERN_TRANSCRIPT_SCHEMA_V1,
                &b"issuer-generated-one-shot-issuance-authorization-digest"[..],
            ),
            (
                BOOTLE_LANTERN_ISSUANCE_WIRE_DESCRIPTOR_V1,
                &b"ILA1:fixed320"[..],
            ),
            (
                BOOTLE_LANTERN_ISSUANCE_WIRE_DESCRIPTOR_V1,
                &b"ILR1:fixed3176"[..],
            ),
            (
                BOOTLE_LANTERN_ISSUANCE_WIRE_DESCRIPTOR_V1,
                &b"ILQ1:fixed71576"[..],
            ),
            (
                BOOTLE_LANTERN_ISSUANCE_WIRE_SCHEMA_V1,
                &b"caller-cap-before-exact-length-before-allocation"[..],
            ),
            (
                BOOTLE_LANTERN_ISSUER_PROFILE_DESCRIPTOR_V1,
                &b"authorization-state:Fresh-Processing-Completed-or-Failed"[..],
            ),
            (
                BOOTLE_LANTERN_ISSUANCE_RANDOMNESS_DESCRIPTOR_V1,
                &b"closed-purpose-enum:no-caller-selected-labels"[..],
            ),
            (
                BOOTLE_LANTERN_ISSUANCE_STORE_PROFILE_DESCRIPTOR_V1,
                &b"canonical-process-lease+unix-nonblocking-exclusive-flock-held-for-lifetime"[..],
            ),
            (
                BOOTLE_LANTERN_FALCON512_MAPPING_DESCRIPTOR_V1,
                &b"H_i[j]=h[8*j+i]"[..],
            ),
            (
                BOOTLE_LANTERN_FALCON512_IMPLEMENTATION_PROVENANCE_V1,
                &b"arbitrary-R512-target"[..],
            ),
            (
                BOOTLE_CREDENTIAL_RANDOMNESS_PROFILE_DESCRIPTOR_V1,
                &b"sign-cache:issuance-local-persistent"[..],
            ),
        ] {
            assert!(
                descriptor
                    .windows(required.len())
                    .any(|window| window == required),
                "compiled descriptor omitted {}",
                String::from_utf8_lossy(required)
            );
        }
        assert_eq!(
            BOOTLE_LANTERN_SCOPE_APPLICATION_ACCEPTANCE_LIMIT_V1,
            BOOTLE_LANTERN_APPLICATION_MODULUS_V1 * 5
        );
        assert_eq!(BOOTLE_LANTERN_SCOPE_MAX_COEFFICIENT_ATTEMPTS_V1, 4_096);
        assert_eq!(CREDENTIAL_RANDOMNESS_POLYNOMIALS_V1, 16);
        assert_eq!(
            CREDENTIAL_RANDOMNESS_NORM_SQUARED_BOUND_V1,
            RANDOMNESS_NORM_SQUARED_BOUND_V1
        );
        assert_eq!(MAX_CREDENTIAL_RANDOMNESS_VECTOR_ATTEMPTS_V1, 64);
        assert_eq!(MAX_CREDENTIAL_RANDOMNESS_COEFFICIENT_PROPOSALS_V1, 256);
        assert_eq!(
            MAX_BOOTLE_LANTERN_ISSUER_KEYGEN_CANDIDATES_V1,
            BOOTLE_LANTERN_FALCON512_DEFAULT_KEYGEN_CANDIDATES_V1
        );
        assert_eq!(MAX_BOOTLE_LANTERN_AUTHORIZATION_ID_ATTEMPTS_V1, 4);
        assert_eq!(MAX_BOOTLE_LANTERN_AUTHORIZATION_LIFETIME_BLOCKS_V1, 4_096);
        assert_eq!(MAX_BOOTLE_LANTERN_PREIMAGE_ATTEMPTS_V1, 64);
        assert_eq!(BOOTLE_LANTERN_ISSUANCE_STORE_MAX_RECORD_BYTES_V1, 3_310);
        assert_eq!(BOOTLE_LANTERN_ISSUANCE_STORE_HARD_MAX_RECORDS_V1, 1_000_000);
        assert_eq!(
            BOOTLE_LANTERN_ISSUANCE_STORE_HARD_MAX_TOTAL_BYTES_V1,
            3_310_000_000
        );
        assert_eq!(BOOTLE_LANTERN_ISSUANCE_STORE_DEFAULT_MAX_RECORDS_V1, 4_096);
        assert_eq!(
            BOOTLE_LANTERN_ISSUANCE_STORE_DEFAULT_MAX_TOTAL_BYTES_V1,
            13_557_760
        );
        assert_eq!(
            BOOTLE_LANTERN_ISSUANCE_STORE_DEFAULT_RETENTION_BLOCKS_V1,
            4_096
        );
        assert_eq!(BLIND_ISSUANCE_AUTHORIZATION_BYTES_V1, 320);
        assert_eq!(BLIND_ISSUANCE_REQUEST_BYTES_V1, 71_576);
        assert_eq!(BLIND_ISSUANCE_REQUEST_HEADER_BYTES_V1, 16);
        assert_eq!(BLIND_ISSUANCE_REQUEST_MAGIC_V1, *b"ILQ1");
        assert_eq!(BLIND_ISSUANCE_REQUEST_VERSION_V1, 1);
        assert_eq!(BLIND_ISSUANCE_REQUEST_PURPOSE_TAG_V1, 1);
        assert_eq!(BLIND_ISSUANCE_REQUEST_TARGET_POLYNOMIALS_V1, 8);
        assert_eq!(BLIND_ISSUANCE_REQUEST_RING_DEGREE_V1, 64);
        assert_eq!(BLIND_ISSUANCE_RESPONSE_BYTES_V1, 3_176);
        assert_eq!(BLIND_ISSUANCE_REQUEST_PROOF_MAGIC_V1, *b"ILB1");
        assert_eq!(BLIND_ISSUANCE_REQUEST_PROOF_PURPOSE_TAG_V1, 1);
        assert_eq!(
            BOOTLE_LANTERN_CREDENTIAL_SCOPE_DIGEST_DOMAIN_V1,
            b"iroha.privacy.bootle-lantern.credential-scope-digest.v1"
        );
        assert_ne!(bootle_lantern_issuer_profile_digest_v1(), [0; 32]);
        assert_eq!(BOOTLE_LANTERN_PROOF_BYTES_V1, 70_344);
        assert!(
            u64::try_from(BOOTLE_LANTERN_PROOF_BYTES_V1).expect("proof size fits u64")
                <= u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
        );
        assert_ne!(public_parameter_seed_v1(), [0; 32]);
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "55bea016d0919cde8d24b54bb35eb01f7578a9a91189aececa34c7fc1b90e75c".to_owned(),
                "6a0b33463d71f6aec27ad330ae4424e3ed317a841dc1a0d79c5389905072ffc9".to_owned(),
                "7733ada1295556a13c3f626f270d1287324e28e987613d97e1e1605ff4d22ce8".to_owned(),
                "9c7c4f65128a4d924955b8b0fb6bfcc56ec34d14224ddfefebe32771c19a9e54".to_owned(),
                "e613fbbaf3e0470524a2924e72e5f8adc93c3950a26c5a4e9af8b7a74b88078b".to_owned(),
            ),
            "every consensus-critical Bootle/Lantern binding is a pinned KAT"
        );

        if !BOOTLE_LANTERN_FULL_ENGINE_AVAILABLE_V1 {
            return;
        }
        let valid = bootle_lantern_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 8] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| {
                    record.proof_system_id =
                        PrivacyProofSystemIdV1::FcmpPlusPlusCurveTreeBulletproofs;
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeFcmpPlusPlus,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                |record| {
                    record.protocol_limits =
                        PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                            JindoActivationLimitsV1 {
                                max_polynomial_count: 1,
                            },
                        );
                },
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn bootle_lantern_complete_sampling_profile_is_parameter_bound_and_kat_pinned() {
        assert!(
            BOOTLE_LANTERN_TRANSCRIPT_SCHEMA_V1
                .windows(b"max-rejected-uniform-draws-per-coefficient=4096".len())
                .any(|window| { window == b"max-rejected-uniform-draws-per-coefficient=4096" })
        );

        let public_parameter_seed = public_parameter_seed_v1();
        let sampling_profile_digest = bootle_sampling_profile_digest_v1();
        assert_eq!(
            hex::encode(sampling_profile_digest),
            "6e037c7342b327b75df5621f999506799174254ca7a7846d7549a6526f6ef897"
        );
        let governed =
            bootle_lantern_parameter_digest_v1(&public_parameter_seed, &sampling_profile_digest);
        assert_eq!(
            hex::encode(governed),
            "6a0b33463d71f6aec27ad330ae4424e3ed317a841dc1a0d79c5389905072ffc9"
        );
        for index in 0..sampling_profile_digest.len() {
            let mut mutated_sampling_profile_digest = sampling_profile_digest;
            mutated_sampling_profile_digest[index] ^= 1;
            assert_ne!(
                governed,
                bootle_lantern_parameter_digest_v1(
                    &public_parameter_seed,
                    &mutated_sampling_profile_digest
                ),
                "sampling-profile digest byte {index} was not parameter-bound"
            );
        }
    }

    #[test]
    fn orchard_profile_is_deterministic_complete_bounded_and_mutation_closed() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::OrchardHalo2ActionsV1)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::OrchardHalo2ActionsV1)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(first.proof_system_id, PrivacyProofSystemIdV1::Halo2IpaPasta);
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeHalo2Orchard);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(OrchardActivationLimitsV1 {
                max_action_count: ORCHARD_MODEL_MAX_ACTIONS_V1,
            })
        );
        assert_eq!(ORCHARD_ENGINE_MAX_ACTIONS_V1, 2);
        assert_eq!(ORCHARD_MODEL_MAX_ACTIONS_V1, 2);
        assert!(
            orchard_authorization_wire_size_v1(2).expect("wire size")
                <= usize::try_from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
                    .expect("global proof cap fits usize")
        );
        assert_ne!(orchard_empty_root_v1(), [0; 32]);
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        let mut mutated_source_policy = TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1.to_vec();
        mutated_source_policy[0] ^= 1;
        let mut mutated_bridge_policy = ORCHARD_PROVER_RANDOMNESS_POLICY_V1.to_vec();
        mutated_bridge_policy[0] ^= 1;
        for (label, source_policy, bridge_policy) in [
            (
                "source",
                mutated_source_policy.as_slice(),
                ORCHARD_PROVER_RANDOMNESS_POLICY_V1,
            ),
            (
                "bridge",
                TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
                mutated_bridge_policy.as_slice(),
            ),
        ] {
            let policy_mutation =
                compiled_orchard_profile_v1_with_randomness_policies(source_policy, bridge_policy)
                    .expect("structurally valid Orchard policy mutation");
            assert_eq!(
                policy_mutation.parameter_id, first.parameter_id,
                "{label} policy changed the parameter family"
            );
            assert_ne!(
                policy_mutation.parameter_digest, first.parameter_digest,
                "{label} policy was not parameter-bound"
            );
            assert_ne!(
                policy_mutation.verifier_digest, first.verifier_digest,
                "{label} policy was not verifier-bound"
            );
            assert_eq!(
                policy_mutation.statement_schema_digest, first.statement_schema_digest,
                "{label} policy changed the statement schema"
            );
            assert_ne!(
                policy_mutation.engine_manifest_digest, first.engine_manifest_digest,
                "{label} policy was not engine-manifest-bound"
            );
        }
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "8d5a2946c58314ac12d2968ffe9e8e0c672e3bbceefaaefad6a87420ea7dd212".to_owned(),
                "b27b73d59151415e21b158c75ed9371cccd795655b604e4a6b53db621660b66e".to_owned(),
                "c788016923d55e5455f3114735999f3c01f06aac8e7af2ce2bed4968b29800ea".to_owned(),
                "0412d379f8cbf01109d994bc74f148a13e38fc64350308597c047a0e6ec95fd9".to_owned(),
                "25f22d98c4f37d513361402fa5730caf214d097b624b2abd848dd932da39751e".to_owned(),
            ),
            "every consensus-critical Orchard profile binding is a pinned KAT"
        );

        let valid = orchard_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [fn(&mut PrivacyProtocolActivationRecordV1); 7] = [
            |record| record.parameter_id.0[0] ^= 1,
            |record| record.parameter_digest.0[0] ^= 1,
            |record| record.verifier_digest.0[0] ^= 1,
            |record| record.statement_schema_digest.0[0] ^= 1,
            |record| record.engine_manifest_digest.0[0] ^= 1,
            |record| {
                record.proof_system_id = PrivacyProofSystemIdV1::FcmpPlusPlusCurveTreeBulletproofs
            },
            |record| record.engine_id = PrivacyEngineIdV1::NativeFcmpPlusPlus,
        ];
        for mutate in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert!(validate_compiled_privacy_activation_v1(&changed).is_err());
        }
    }

    #[cfg(not(feature = "zk-stark"))]
    #[test]
    fn zk_ace_remains_fail_closed_without_a_sound_compiled_profile() {
        let protocol_id = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
        assert_eq!(
            compiled_privacy_profile_v1(protocol_id),
            Err(CompiledPrivacyProfileErrorV1::EngineUnavailable { protocol_id })
        );
    }

    #[cfg(feature = "zk-stark")]
    #[test]
    fn zk_ace_profile_is_deterministic_complete_and_bounded() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            first.proof_system_id,
            PrivacyProofSystemIdV1::StarkFriSha256Goldilocks
        );
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeGoldilocksStarkFri);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::ZkAcePqAuthorizationV0
        );
        assert!(ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1 <= TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1);
        assert_ne!(zk_ace_compiled_profile_digest_v1(), [0; 32]);
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "7f6efa99b249c5a95d2828338ffd533bd3e2e3cb8748f9bef984d34783cd727c".to_owned(),
                "eccf8e390650afa055dd617a18094f064eea06b1a9116fe9d6443d2f8ffb184f".to_owned(),
                "c6862c2f31dd4121b92af8fb272580101cc79344aea739a1b90f6cf8501b7509".to_owned(),
                "fc01374c09dc173e7c184f790fb959c495457ee8490eb3b18b48a802e5aa1d4e".to_owned(),
                "a94a0f8cfa1762a38921c47777c1c8ce22a82f0e9bb8ebf0857f51347ed73531".to_owned(),
            )
        );
    }

    #[cfg(feature = "zk-stark")]
    #[test]
    fn zk_ace_compiled_profile_rejects_every_binding_mismatch() {
        let valid = zk_ace_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 8] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| {
                    record.proof_system_id = PrivacyProofSystemIdV1::JindoPolynomialCommitment;
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeJindo,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                |record| {
                    record.protocol_limits =
                        PrivacyProtocolActivationLimitsV1::VegaExistingCredentialZkV0;
                },
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn zk_ams_profile_is_unavailable_until_every_mkhe_gate_closes() {
        let expected = CompiledPrivacyProfileErrorV1::EngineUnavailable {
            protocol_id: PrivacyProtocolIdV1::IrohaZkAmsV1,
        };
        assert_eq!(
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkAmsV1),
            Err(expected)
        );
        assert_eq!(
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkAmsV1),
            Err(expected),
            "the unavailable result must be deterministic"
        );

        let candidate = zk_ams_release_candidate_profile_material_v1()
            .expect("release-candidate profile material derives independently of activation");
        assert_eq!(candidate.protocol_id, PrivacyProtocolIdV1::IrohaZkAmsV1);
        for digest in [
            *candidate.parameter_id.as_bytes(),
            *candidate.parameter_digest.as_bytes(),
            *candidate.verifier_digest.as_bytes(),
            *candidate.statement_schema_digest.as_bytes(),
            *candidate.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }

        let candidate_activation = candidate.activation_record(
            PrivacyProtocolLifecycleV1::Proposed(PrivacyProposedLifecycleV1 {
                proposed_at_height: 100,
                activate_at_height: 400,
            }),
        );
        assert_eq!(
            validate_compiled_privacy_activation_v1(&candidate_activation),
            Err(CompiledPrivacyProfileValidationErrorV1::Profile(expected)),
            "release-candidate material must never bypass the production readiness gate",
        );

        let readiness =
            iroha_zkp_halo2::vega::zk_ams_mkhe_readiness_v1().expect("candidate readiness derives");
        assert!(readiness.parameter_gate);
        assert!(readiness.noise_gate);
        assert!(readiness.security_gate);
        assert!(!readiness.resource_gate);
        assert!(!readiness.wire_gate);
        assert!(!readiness.malicious_party_gate);
        assert!(!readiness.decryption_share_gate);
        assert!(readiness.packing_gate);
        assert!(!readiness.phase23_gate);
        assert!(!readiness.release_kat_gate);
        assert!(!readiness.is_ready());
    }

    #[test]
    fn structural_schema_digest_detects_reordering_and_retyping() {
        let original = canonical_schema_digest_v1::<SchemaOrderAb>().expect("schema");
        let reordered = canonical_schema_digest_v1::<SchemaOrderBa>().expect("schema");
        let retyped = canonical_schema_digest_v1::<SchemaRetyped>().expect("schema");
        assert_ne!(original, reordered);
        assert_ne!(original, retyped);
        assert_ne!(reordered, retyped);
        assert_eq!(
            original,
            canonical_schema_digest_v1::<SchemaOrderAb>().expect("schema")
        );
    }

    #[test]
    fn structural_schema_digest_deduplicates_only_equivalent_aliases() {
        let equivalent =
            canonical_schema_digest_v1::<SchemaEquivalentAliases>().expect("equivalent aliases");
        assert_ne!(equivalent, [0; 32]);
        assert_eq!(
            canonical_schema_digest_v1::<SchemaEquivalentAliases>().expect("equivalent aliases"),
            equivalent
        );
        assert_eq!(
            canonical_schema_digest_v1::<SchemaConflictingAliases>(),
            Err(CanonicalSchemaDigestErrorV1::ConflictingStableTypeId)
        );
    }

    #[test]
    fn verange_profile_is_deterministic_and_uses_effective_global_cap() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
                VeRangeActivationLimitsV1 {
                    max_aggregation_count: 8,
                }
            )
        );
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "97e8be40e495bb6723db0ca73c04d2441ff166cf2163ddd2662c7e6a083f2c32".to_owned(),
                "3d79fe744741f956cb589f45774f922b849cf93833e6a9ebdedf1f815f1b7b44".to_owned(),
                "9b1a285d43ddc306b4d9ca6eac525b49b073f7d281ecf94299730613f683aa13".to_owned(),
                "32c038ab076bf2cab61bb15ffd07675e64b6849fce6e935252160b640d11b5c4".to_owned(),
                "5464e209f243f68189a84fad74e435aa78653d2fdd3458601787daf5479a45b0".to_owned(),
            )
        );
    }

    #[test]
    fn anonymous_pgc_profile_is_deterministic_complete_and_bounded() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
            PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1
        );
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
                AnonymousPgcActivationLimitsV1 {
                    max_anonymity_set_size: 64,
                    max_recipient_count: 8,
                }
            )
        );
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "58c1a93d39f23727ae8b5bbb661414f3dcadf2479575282cd7e3b9ebbb5589fc".to_owned(),
                "ca09d19ed5f3bb56ba7432a67b7ad14697c4874ab7870ea53441e4df0624bd7b".to_owned(),
                "aa352369f2a1fd0c9377414a2721728c35a95a4bc72497118e75c765edacd99e".to_owned(),
                "080aaf7d1f9d44c5dad6a5adc393034715fbf428d1dd1e5b59e33808c110aa96".to_owned(),
                "a74d8f690da89d50b9950e6d3496179f98bc6e60b71ec11e408c908aad73a81b".to_owned(),
            )
        );
    }

    #[test]
    fn jindo_profile_is_deterministic_complete_and_bounded() {
        let first =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0)
                .expect("profile");
        let second =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0)
                .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            first.proof_system_id,
            PrivacyProofSystemIdV1::JindoPolynomialCommitment
        );
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeJindo);
        assert_eq!(JINDO_NATIVE_PROOF_BYTES_V1, 331_912);
        assert_ne!(jindo_crs_digest_v1(), [0; 32]);
        let provenance = core::str::from_utf8(JINDO_SOURCE_PROVENANCE_V1)
            .expect("Jindo source provenance is ASCII");
        assert!(provenance.contains("revision-2026-06-02"));
        assert!(provenance.contains("ringo-snark@805eab27"));
        let wire = core::str::from_utf8(JINDO_PROOF_WIRE_LABEL_V1)
            .expect("Jindo proof wire label is ASCII");
        for required in ["IJP2", "7-outer", "12-inner", "644-field", "no-IJP1"] {
            assert!(
                wire.contains(required),
                "Jindo wire descriptor lost {required}"
            );
        }
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                JindoActivationLimitsV1 {
                    max_polynomial_count: u32::try_from(JINDO_MAX_BATCH_SIZE_V1)
                        .expect("fixed Jindo batch size fits u32"),
                }
            )
        );
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
                hex::encode(jindo_crs_digest_v1()),
            ),
            (
                "48bdc194dcd85c416db5b1c00e58dba42357098dfb807d060497d7495911692c".to_owned(),
                "56c9d07c283889a824768299b65dd69e2b6befbd123434be8571d21b32b0794b".to_owned(),
                "89fe6e1c19c8b4851bf33b66479fba2d747943442009679c8618158165fad76e".to_owned(),
                "7b87a8f64c9345e3ce13c2f4ce02a183e3806a8d2cea0faf7b6b0a00491aed28".to_owned(),
                "ae3bf287b0c3c0f8c3163db10a06f037f79e3a5967ed6a84eadb054cc809d95a".to_owned(),
                "424603d0ab5f57eed76aa365ec100cb4ac583e10dc801727363b6e188f5edd27".to_owned(),
            )
        );
    }

    #[test]
    fn vega_profile_is_deterministic_complete_and_bounded() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            first.proof_system_id,
            PrivacyProofSystemIdV1::VegaNeutronNovaSpartanHyraxT256
        );
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeVega);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::VegaExistingCredentialZkV0
        );
        assert!(MAX_VEGA_PROOF_BYTES_V1 <= TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1 as usize);
        assert_ne!(vega_mdl_canonical_relation_digest_v1(), [0; 32]);
        assert_ne!(vega_mdl_compiled_profile_digest_v1(), [0; 32]);
        assert_eq!(
            vega_mdl_verifier_digest_v1().expect("canonical Vega-MC verifier digest"),
            VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1,
        );
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "9fa2a07d17989e07bb7ff804bb408e95e127b80ab5e01258b77af9b00c82607d".to_owned(),
                "cf6bb53805e982444751db072c04d8b52dd9e14712cb90bbf23f68bbf2650c82".to_owned(),
                "6056ad21ff647212dcc81ff5508e5348400ca734a230073ac6367fa9c7b5ba3f".to_owned(),
                "f45032acceaf4b65e5afe114ca1f87fde477a73040e07c60a2c99e831f4cdc63".to_owned(),
                "c701b59a7083969770841a85a784608543c61e5849fed0670bfd97c2aa845009".to_owned(),
            )
        );
    }

    #[test]
    #[ignore = "operator-only KAT regeneration after an intentional compiled-profile change"]
    fn print_all_compiled_profile_tuples() {
        for protocol_id in PrivacyProtocolIdV1::ALL {
            let profile = if protocol_id == PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 {
                zk_x509_release_candidate_profile_material_v1()
            } else {
                compiled_privacy_profile_v1(protocol_id)
            }
            .unwrap_or_else(|error| {
                panic!(
                    "compiled profile for {}: {error}",
                    protocol_id.canonical_label()
                )
            });
            eprintln!(
                "{}={}|{}|{}|{}|{}",
                protocol_id.canonical_label(),
                hex::encode(profile.parameter_id.as_bytes()),
                hex::encode(profile.parameter_digest.as_bytes()),
                hex::encode(profile.verifier_digest.as_bytes()),
                hex::encode(profile.statement_schema_digest.as_bytes()),
                hex::encode(profile.engine_manifest_digest.as_bytes()),
            );
        }
    }

    #[test]
    fn vega_compiled_profile_rejects_every_binding_mismatch() {
        let valid = vega_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 7] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| {
                    record.proof_system_id = PrivacyProofSystemIdV1::IrohaVeRangeP256;
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeVeRangeP256,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn jindo_compiled_profile_rejects_every_binding_and_policy_mismatch() {
        let valid = jindo_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 8] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| {
                    record.proof_system_id = PrivacyProofSystemIdV1::IrohaVeRangeP256;
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeVeRangeP256,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                |record| {
                    record.protocol_limits =
                        PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                            JindoActivationLimitsV1 {
                                max_polynomial_count: 5,
                            },
                        );
                },
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn every_compiled_cryptographic_binding_is_immutable() {
        let valid = verange_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");

        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 7] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| record.proof_system_id = PrivacyProofSystemIdV1::StarkFriSha256Goldilocks,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeGoldilocksStarkFri,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn compiled_validation_accepts_lower_protocol_policy_without_changing_digests() {
        let verange_compiled =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
                .expect("VeRange profile");
        let mut verange = verange_activation();
        verange.protocol_limits = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 1,
            },
        );
        validate_compiled_privacy_activation_v1(&verange).expect("lower VeRange policy");
        assert_eq!(verange.parameter_id, verange_compiled.parameter_id);
        assert_eq!(verange.parameter_digest, verange_compiled.parameter_digest);
        assert_eq!(verange.verifier_digest, verange_compiled.verifier_digest);
        assert_eq!(
            verange.statement_schema_digest,
            verange_compiled.statement_schema_digest
        );
        assert_eq!(
            verange.engine_manifest_digest,
            verange_compiled.engine_manifest_digest
        );

        let pgc_compiled = compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
            .expect("PGC profile");
        let mut pgc = pgc_activation();
        pgc.protocol_limits = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 16,
                max_recipient_count: 1,
            },
        );
        validate_compiled_privacy_activation_v1(&pgc).expect("lower PGC policy");
        assert_eq!(pgc.parameter_id, pgc_compiled.parameter_id);
        assert_eq!(pgc.parameter_digest, pgc_compiled.parameter_digest);
        assert_eq!(pgc.verifier_digest, pgc_compiled.verifier_digest);
        assert_eq!(
            pgc.statement_schema_digest,
            pgc_compiled.statement_schema_digest
        );
        assert_eq!(
            pgc.engine_manifest_digest,
            pgc_compiled.engine_manifest_digest
        );

        let jindo_compiled =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0)
                .expect("Jindo profile");
        let mut jindo = jindo_activation();
        jindo.protocol_limits = PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
            JindoActivationLimitsV1 {
                max_polynomial_count: 1,
            },
        );
        validate_compiled_privacy_activation_v1(&jindo).expect("lower Jindo policy");
        assert_eq!(jindo.parameter_id, jindo_compiled.parameter_id);
        assert_eq!(jindo.parameter_digest, jindo_compiled.parameter_digest);
        assert_eq!(jindo.verifier_digest, jindo_compiled.verifier_digest);
        assert_eq!(
            jindo.statement_schema_digest,
            jindo_compiled.statement_schema_digest
        );
        assert_eq!(
            jindo.engine_manifest_digest,
            jindo_compiled.engine_manifest_digest
        );

        let orchard_compiled =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::OrchardHalo2ActionsV1)
                .expect("Orchard profile");
        let mut orchard = orchard_activation();
        orchard.protocol_limits =
            PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(OrchardActivationLimitsV1 {
                max_action_count: 1,
            });
        validate_compiled_privacy_activation_v1(&orchard).expect("lower Orchard policy");
        assert_eq!(orchard.parameter_id, orchard_compiled.parameter_id);
        assert_eq!(orchard.parameter_digest, orchard_compiled.parameter_digest);
        assert_eq!(orchard.verifier_digest, orchard_compiled.verifier_digest);
        assert_eq!(
            orchard.statement_schema_digest,
            orchard_compiled.statement_schema_digest
        );
        assert_eq!(
            orchard.engine_manifest_digest,
            orchard_compiled.engine_manifest_digest
        );
    }

    #[test]
    fn compiled_validation_rejects_protocol_limit_overflow_mismatch_and_invalid_lowering() {
        let mut invalid = Vec::new();

        let mut verange_over = verange_activation();
        verange_over.protocol_limits = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 9,
            },
        );
        invalid.push(verange_over);

        let mut pgc_n_over = pgc_activation();
        pgc_n_over.protocol_limits = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 65,
                max_recipient_count: 8,
            },
        );
        invalid.push(pgc_n_over);

        let mut pgc_k_over = pgc_activation();
        pgc_k_over.protocol_limits = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 64,
                max_recipient_count: 9,
            },
        );
        invalid.push(pgc_k_over);

        let mut pgc_bad_closed_set = pgc_activation();
        pgc_bad_closed_set.protocol_limits =
            PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
                AnonymousPgcActivationLimitsV1 {
                    max_anonymity_set_size: 17,
                    max_recipient_count: 1,
                },
            );
        invalid.push(pgc_bad_closed_set);

        let mut zero_verange = verange_activation();
        zero_verange.protocol_limits = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 0,
            },
        );
        invalid.push(zero_verange);

        let mut jindo_over = jindo_activation();
        jindo_over.protocol_limits =
            PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                JindoActivationLimitsV1 {
                    max_polynomial_count: 5,
                },
            );
        invalid.push(jindo_over);

        let mut zero_jindo = jindo_activation();
        zero_jindo.protocol_limits =
            PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                JindoActivationLimitsV1 {
                    max_polynomial_count: 0,
                },
            );
        invalid.push(zero_jindo);

        let mut orchard_over = orchard_activation();
        orchard_over.protocol_limits =
            PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(OrchardActivationLimitsV1 {
                max_action_count: ORCHARD_MODEL_MAX_ACTIONS_V1 + 1,
            });
        invalid.push(orchard_over);

        let mut zero_orchard = orchard_activation();
        zero_orchard.protocol_limits =
            PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(OrchardActivationLimitsV1 {
                max_action_count: 0,
            });
        invalid.push(zero_orchard);

        let mut wrong_variant = verange_activation();
        wrong_variant.protocol_limits = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 16,
                max_recipient_count: 1,
            },
        );
        invalid.push(wrong_variant);

        for activation in invalid {
            assert_eq!(
                validate_compiled_privacy_activation_v1(&activation),
                Err(CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch)
            );
        }
    }

    #[test]
    fn zk_x509_compiled_activation_is_complete_and_immutable() {
        let candidate = zk_x509_release_candidate_profile_material_v1()
            .expect("release candidate profile material");
        assert_eq!(
            (
                hex::encode(candidate.parameter_id.as_bytes()),
                hex::encode(candidate.parameter_digest.as_bytes()),
                hex::encode(candidate.verifier_digest.as_bytes()),
                hex::encode(candidate.statement_schema_digest.as_bytes()),
                hex::encode(candidate.engine_manifest_digest.as_bytes()),
            ),
            (
                "1ef8a47c6314a4a91e4446086b8c0c7110879e7770b441c663c1c398d5ea518b".to_owned(),
                "19c064109579bf83809043cec4e1ea9744af3486251e5253911f4d87634999ff".to_owned(),
                "4a7f1f34a569d9b5cedc137e12df012eee740dd32dbf2dff375b7f1b08766c0c".to_owned(),
                "f228f0d842277d2df246a1e6aa66880726a617d669e176efa37ad5a106bc7f60".to_owned(),
                "709883293be4fb2c89740490724394990c8f4d600c2b8e0a41a9539bd2211fdb".to_owned(),
            ),
            "every consensus-critical zk-X.509 binding is a pinned KAT",
        );
        let valid = zk_x509_activation();
        validate_compiled_privacy_activation_against_profile_v1(&valid, &candidate)
            .expect("exact release-pinned zk-X.509 activation");
        assert_eq!(
            valid.proof_system_id,
            PrivacyProofSystemIdV1::StarkFriSha256Goldilocks
        );
        assert_eq!(valid.engine_id, PrivacyEngineIdV1::NativeGoldilocksStarkFri);
        assert_eq!(
            valid.protocol_limits,
            PrivacyProtocolActivationLimitsV1::IrohaZkX509StarkP256V0
        );

        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 8] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| {
                    record.proof_system_id =
                        PrivacyProtocolIdV1::VeRangeTransparentRangeV1.expected_proof_system();
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| {
                    record.engine_id =
                        PrivacyProtocolIdV1::VeRangeTransparentRangeV1.expected_engine();
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                |record| {
                    record.protocol_limits =
                        PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
                            VeRangeActivationLimitsV1 {
                                max_aggregation_count: 1,
                            },
                        );
                },
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_against_profile_v1(&changed, &candidate),
                Err(expected)
            );
        }

        let mut wrong_protocol = valid;
        wrong_protocol.protocol_id = PrivacyProtocolIdV1::VeRangeTransparentRangeV1;
        assert_eq!(
            validate_compiled_privacy_activation_against_profile_v1(&wrong_protocol, &candidate),
            Err(CompiledPrivacyProfileValidationErrorV1::ProtocolMismatch)
        );
    }

    #[test]
    fn anonymous_pgc_compiled_bindings_are_immutable() {
        let valid = pgc_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [fn(&mut PrivacyProtocolActivationRecordV1); 5] = [
            |record| {
                record.parameter_digest.0[0] ^= 1;
            },
            |record| record.verifier_digest.0[0] ^= 1,
            |record| record.statement_schema_digest.0[0] ^= 1,
            |record| record.engine_manifest_digest.0[0] ^= 1,
            |record| {
                let PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(ref mut limits) =
                    record.protocol_limits
                else {
                    unreachable!("fixture is Anonymous PGC");
                };
                limits.max_recipient_count += 1;
            },
        ];
        for mutate in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert!(validate_compiled_privacy_activation_v1(&changed).is_err());
        }
    }
}
