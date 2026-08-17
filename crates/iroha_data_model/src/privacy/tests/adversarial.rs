#[test]
fn public_balance_scope_never_accepts_universal_as_a_partition() {
    let limits = PrivacyConsensusLimitsV1::taira_default();
    let universal = AssetBalanceScope::Dataspace(crate::nexus::DataSpaceId::UNIVERSAL);
    for protocol in [
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
        PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
        PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
    ] {
        let mut statement = statement_for(protocol);
        match &mut statement {
            PrivacyStatementV1::ZkAcePqAuthorizationV0(statement) => {
                statement.public_balance_scope = universal;
            }
            PrivacyStatementV1::OrchardHalo2ActionsV1(statement) => {
                statement.public_balance_scope = universal;
            }
            PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) => {
                statement.public_balance_scope = universal;
            }
            _ => unreachable!("test protocol list is closed"),
        }
        assert_eq!(
            statement.validate(&limits),
            Err(PrivacyStatementValidationError::UniversalPublicBalanceScope),
            "{protocol:?} accepted the coordinator as a balance partition",
        );
    }
}
#[test]
fn zk_x509_policy_caps_ordering_and_transition_adversaries_fail_closed() {
    assert_zk_x509_policy_caps_and_ordering();
    assert_zk_x509_trust_anchor_transitions();
    assert_zk_x509_certificate_policy_transitions();
}
#[test]
fn bootle_lantern_disclosures_are_fixed_direct_and_canonically_ordered() {
    let limits = PrivacyConsensusLimitsV1::taira_default();
    let base = statement_for(PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1);
    let mutate = |f: fn(&mut IrohaBootleLanternAnoncredStatementV1)| {
        let mut value = base.clone();
        let PrivacyStatementV1::IrohaBootleLanternAnoncredV1(statement) = &mut value else {
            unreachable!()
        };
        f(statement);
        value.validate(&limits)
    };
    assert!(matches!(
        mutate(|statement| statement.issuer_policy_epoch = 0),
        Err(PrivacyStatementValidationError::ZeroEpoch {
            field: PrivacyEpochFieldV1::IssuerPolicy
        })
    ));
    assert!(matches!(
        mutate(|statement| {
            statement.issuer_policy_record_digest =
                PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32])
        }),
        Err(PrivacyStatementValidationError::ZeroTypedField {
            field: PrivacyTypedFieldV1::IssuerPolicyRecordDigest
        })
    ));
    assert!(matches!(
        mutate(|statement| statement.disclosures.swap(0, 1)),
        Err(PrivacyStatementValidationError::BootleLanternDisclosuresNotStrictlyIncreasing)
    ));
    assert!(matches!(
        mutate(|statement| statement.disclosures[1].index = 8),
        Err(PrivacyStatementValidationError::BootleLanternDisclosureIndexOutOfBounds { index: 8 })
    ));
    assert!(matches!(
        mutate(|statement| statement.disclosures[1].index = statement.disclosures[0].index),
        Err(PrivacyStatementValidationError::BootleLanternDisclosuresNotStrictlyIncreasing)
    ));
    assert!(matches!(
        mutate(|statement| {
            statement.disclosures = (0_u8..=8)
                .map(|index| BootleLanternDisclosedAttributeV1 {
                    index,
                    value: BootleLanternAttributeValueV1::new([index; 8]),
                })
                .collect()
        }),
        Err(PrivacyStatementValidationError::TooManyBootleLanternDisclosures { count: 9, max: 8 })
    ));
    let mut all_boundaries = base;
    let PrivacyStatementV1::IrohaBootleLanternAnoncredV1(statement) = &mut all_boundaries else {
        unreachable!()
    };
    statement.disclosures = (0_u8..8)
        .map(|index| BootleLanternDisclosedAttributeV1 {
            index,
            value: BootleLanternAttributeValueV1::new(if index.is_multiple_of(2) {
                [0; 8]
            } else {
                [u8::MAX; 8]
            }),
        })
        .collect();
    all_boundaries
        .validate(&limits)
        .expect("all eight direct zero/maximum values are canonical");
}
#[test]
fn zk_ace_policy_record_is_canonical_self_digested_and_roundtrips() {
    let record = zk_ace_policy(
        PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1,
        11,
        PrivacyZkAcePolicyLifecycleV1::Active,
    );
    record.validate_initial().expect("canonical initial policy");
    assert_eq!(
        record
            .compute_record_digest()
            .expect("recompute canonical policy digest"),
        record.record_digest
    );
    let encoded = norito::to_bytes(&record).expect("encode ZK-ACE policy");
    let decoded: PrivacyZkAcePolicyRecordV1 =
        norito::decode_from_bytes(&encoded).expect("decode ZK-ACE policy");
    assert_eq!(decoded, record);
    decoded
        .validate_initial()
        .expect("decoded policy validates");
    let json = norito::json::to_json(&record).expect("encode ZK-ACE policy JSON");
    let decoded_json: PrivacyZkAcePolicyRecordV1 =
        norito::json::from_json(&json).expect("decode ZK-ACE policy JSON");
    assert_eq!(decoded_json, record);
    decoded_json
        .validate_initial()
        .expect("JSON-decoded policy validates");
    let object_prefix = json
        .strip_suffix('}')
        .expect("policy JSON is a top-level object");
    let unknown_field = format!("{object_prefix},\"unexpected_policy_alias\":true}}");
    assert!(
        norito::json::from_json::<PrivacyZkAcePolicyRecordV1>(&unknown_field).is_err(),
        "unknown JSON fields must not create an alternate first-release policy encoding"
    );
    let mut zero_digest = record.clone();
    zero_digest.record_digest = PrivacyZkAcePolicyRecordDigestV1::new([0; 32]);
    assert_eq!(
        zero_digest.validate(),
        Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroRecordDigest)
    );
    let mut tamperings = Vec::new();
    let mut tampered = record.clone();
    tampered.policy_id = PrivacyPolicyIdV1::new(raw(90));
    tamperings.push(tampered);
    let mut tampered = record.clone();
    tampered.identity_commitment = commitment(91);
    tamperings.push(tampered);
    let mut tampered = record.clone();
    tampered.policy_digest = PrivacyPolicyDigestV1::new(raw(92));
    tamperings.push(tampered);
    let mut tampered = record.clone();
    tampered.authorization_epoch = 2;
    tamperings.push(tampered);
    let mut tampered = record.clone();
    tampered.asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("privacy", "universal").expect("domain"),
        Name::from_str("other_asset").expect("asset name"),
    );
    tamperings.push(tampered);
    let mut tampered = record.clone();
    tampered.source_allowlist.push(account(99));
    tampered.source_allowlist.sort_unstable();
    tamperings.push(tampered);
    let mut tampered = record;
    tampered.lifecycle = PrivacyZkAcePolicyLifecycleV1::Revoked;
    tamperings.push(tampered);
    for tampered in tamperings {
        assert_eq!(
            tampered.validate(),
            Err(PrivacyZkAcePolicyRecordValidationErrorV1::RecordDigestMismatch)
        );
    }
}
fn construct_zk_ace_policy_for_test(
    policy_id: PrivacyPolicyIdV1,
    identity_commitment: PrivacyCommitmentV1,
    policy_digest: PrivacyPolicyDigestV1,
    authorization_epoch: u64,
    source_allowlist: Vec<AccountId>,
    lifecycle: PrivacyZkAcePolicyLifecycleV1,
) -> Result<PrivacyZkAcePolicyRecordV1, PrivacyZkAcePolicyRecordValidationErrorV1> {
    PrivacyZkAcePolicyRecordV1::new(
        policy_id,
        identity_commitment,
        policy_digest,
        authorization_epoch,
        asset_definition_id(),
        source_allowlist,
        lifecycle,
    )
}
fn assert_zk_ace_policy_scalar_boundaries(
    policy_id: PrivacyPolicyIdV1,
    identity: PrivacyCommitmentV1,
    digest: PrivacyPolicyDigestV1,
    allowlist: &[AccountId],
) {
    assert_eq!(
        construct_zk_ace_policy_for_test(
            PrivacyPolicyIdV1::new([0; 32]),
            identity,
            digest,
            1,
            allowlist.to_vec(),
            PrivacyZkAcePolicyLifecycleV1::Active,
        ),
        Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroPolicyId)
    );
    assert_eq!(
        construct_zk_ace_policy_for_test(
            policy_id,
            PrivacyCommitmentV1::new([0; 32]),
            digest,
            1,
            allowlist.to_vec(),
            PrivacyZkAcePolicyLifecycleV1::Active,
        ),
        Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroIdentityCommitment)
    );
    assert_eq!(
        construct_zk_ace_policy_for_test(
            policy_id,
            identity,
            PrivacyPolicyDigestV1::new([0; 32]),
            1,
            allowlist.to_vec(),
            PrivacyZkAcePolicyLifecycleV1::Active,
        ),
        Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroPolicyDigest)
    );
    assert_eq!(
        construct_zk_ace_policy_for_test(
            policy_id,
            identity,
            digest,
            0,
            allowlist.to_vec(),
            PrivacyZkAcePolicyLifecycleV1::Active,
        ),
        Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroAuthorizationEpoch)
    );
    assert_eq!(
        construct_zk_ace_policy_for_test(
            policy_id,
            identity,
            digest,
            1,
            Vec::new(),
            PrivacyZkAcePolicyLifecycleV1::Active,
        ),
        Err(PrivacyZkAcePolicyRecordValidationErrorV1::EmptySourceAllowlist)
    );
}
fn assert_zk_ace_policy_allowlist_and_origin_boundaries(
    policy_id: PrivacyPolicyIdV1,
    identity: PrivacyCommitmentV1,
    digest: PrivacyPolicyDigestV1,
    allowlist: &[AccountId],
) {
    let over_limit = vec![account(20); PRIVACY_ZK_ACE_MAX_SOURCE_ACCOUNTS_V1 + 1];
    assert_eq!(
        construct_zk_ace_policy_for_test(
            policy_id,
            identity,
            digest,
            1,
            over_limit,
            PrivacyZkAcePolicyLifecycleV1::Active,
        ),
        Err(
            PrivacyZkAcePolicyRecordValidationErrorV1::SourceAllowlistTooLarge {
                actual: PRIVACY_ZK_ACE_MAX_SOURCE_ACCOUNTS_V1 + 1,
                max: PRIVACY_ZK_ACE_MAX_SOURCE_ACCOUNTS_V1,
            }
        )
    );
    let mut reversed = allowlist.to_vec();
    reversed.reverse();
    assert_eq!(
        construct_zk_ace_policy_for_test(
            policy_id,
            identity,
            digest,
            1,
            reversed,
            PrivacyZkAcePolicyLifecycleV1::Active,
        ),
        Err(PrivacyZkAcePolicyRecordValidationErrorV1::NonCanonicalSourceAllowlist)
    );
    let duplicate = vec![allowlist[0].clone(), allowlist[0].clone()];
    assert_eq!(
        construct_zk_ace_policy_for_test(
            policy_id,
            identity,
            digest,
            1,
            duplicate,
            PrivacyZkAcePolicyLifecycleV1::Active,
        ),
        Err(PrivacyZkAcePolicyRecordValidationErrorV1::NonCanonicalSourceAllowlist)
    );
    let noncanonical_epoch = zk_ace_policy(2, 11, PrivacyZkAcePolicyLifecycleV1::Active);
    assert_eq!(
        noncanonical_epoch.validate_initial(),
        Err(PrivacyZkAcePolicyRecordValidationErrorV1::NonCanonicalInitialEpoch { actual: 2 })
    );
    let initially_revoked = zk_ace_policy(
        PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1,
        11,
        PrivacyZkAcePolicyLifecycleV1::Revoked,
    );
    assert_eq!(
        initially_revoked.validate_initial(),
        Err(PrivacyZkAcePolicyRecordValidationErrorV1::InitialPolicyNotActive)
    );
}
#[test]
fn zk_ace_policy_registration_rejects_every_noncanonical_boundary() {
    let policy_id = PrivacyPolicyIdV1::new(raw(10));
    let identity = commitment(11);
    let digest = PrivacyPolicyDigestV1::new(raw(12));
    let allowlist = zk_ace_allowlist();
    assert_zk_ace_policy_scalar_boundaries(policy_id, identity, digest, &allowlist);
    assert_zk_ace_policy_allowlist_and_origin_boundaries(policy_id, identity, digest, &allowlist);
}
#[test]
fn zk_ace_rotation_rejects_replays_skips_noops_and_terminal_policies() {
    let current = zk_ace_policy(1, 11, PrivacyZkAcePolicyLifecycleV1::Active);
    let successor = zk_ace_policy(2, 21, PrivacyZkAcePolicyLifecycleV1::Active);
    validate_zk_ace_policy_rotation_v1(&current, &successor)
        .expect("canonical one-epoch identity rotation");
    let mut invalid_current = current.clone();
    invalid_current.record_digest = PrivacyZkAcePolicyRecordDigestV1::new(raw(90));
    assert_eq!(
        validate_zk_ace_policy_rotation_v1(&invalid_current, &successor),
        Err(
            PrivacyZkAcePolicyTransitionValidationErrorV1::InvalidCurrent(
                PrivacyZkAcePolicyRecordValidationErrorV1::RecordDigestMismatch
            )
        )
    );
    let mut invalid_successor = successor.clone();
    invalid_successor.record_digest = PrivacyZkAcePolicyRecordDigestV1::new(raw(91));
    assert_eq!(
        validate_zk_ace_policy_rotation_v1(&current, &invalid_successor),
        Err(
            PrivacyZkAcePolicyTransitionValidationErrorV1::InvalidSuccessor(
                PrivacyZkAcePolicyRecordValidationErrorV1::RecordDigestMismatch
            )
        )
    );
    let mut different_policy = successor.clone();
    different_policy.policy_id = PrivacyPolicyIdV1::new(raw(92));
    redigest_zk_ace_policy(&mut different_policy);
    assert_eq!(
        validate_zk_ace_policy_rotation_v1(&current, &different_policy),
        Err(PrivacyZkAcePolicyTransitionValidationErrorV1::PolicyIdMismatch)
    );
    for epoch in [1, 3] {
        let candidate = zk_ace_policy(epoch, 21, PrivacyZkAcePolicyLifecycleV1::Active);
        assert!(matches!(
            validate_zk_ace_policy_rotation_v1(&current, &candidate),
            Err(
                PrivacyZkAcePolicyTransitionValidationErrorV1::NonCanonicalSuccessorEpoch {
                    expected: 2,
                    actual
                }
            ) if actual == epoch
        ));
    }
    let revoked_successor = zk_ace_policy(2, 21, PrivacyZkAcePolicyLifecycleV1::Revoked);
    assert_eq!(
        validate_zk_ace_policy_rotation_v1(&current, &revoked_successor),
        Err(PrivacyZkAcePolicyTransitionValidationErrorV1::RotationSuccessorNotActive)
    );
    let no_op = zk_ace_policy(2, 11, PrivacyZkAcePolicyLifecycleV1::Active);
    assert_eq!(
        validate_zk_ace_policy_rotation_v1(&current, &no_op),
        Err(PrivacyZkAcePolicyTransitionValidationErrorV1::IdentityCommitmentUnchanged)
    );
    let revoked_current = zk_ace_policy(1, 11, PrivacyZkAcePolicyLifecycleV1::Revoked);
    assert_eq!(
        validate_zk_ace_policy_rotation_v1(&revoked_current, &successor),
        Err(PrivacyZkAcePolicyTransitionValidationErrorV1::CurrentNotActive)
    );
    let max_epoch = zk_ace_policy(u64::MAX, 11, PrivacyZkAcePolicyLifecycleV1::Active);
    let max_successor = zk_ace_policy(u64::MAX, 21, PrivacyZkAcePolicyLifecycleV1::Active);
    assert_eq!(
        validate_zk_ace_policy_rotation_v1(&max_epoch, &max_successor),
        Err(PrivacyZkAcePolicyTransitionValidationErrorV1::EpochOverflow)
    );
}
#[test]
fn zk_ace_revocation_is_one_step_terminal_and_content_preserving() {
    let current = zk_ace_policy(1, 11, PrivacyZkAcePolicyLifecycleV1::Active);
    let successor = zk_ace_policy(2, 11, PrivacyZkAcePolicyLifecycleV1::Revoked);
    validate_zk_ace_policy_revocation_v1(&current, &successor)
        .expect("canonical one-epoch revocation");
    let active_successor = zk_ace_policy(2, 11, PrivacyZkAcePolicyLifecycleV1::Active);
    assert_eq!(
        validate_zk_ace_policy_revocation_v1(&current, &active_successor),
        Err(PrivacyZkAcePolicyTransitionValidationErrorV1::RevocationSuccessorNotRevoked)
    );
    let mut mutations = Vec::new();
    let mut changed_identity = successor.clone();
    changed_identity.identity_commitment = commitment(21);
    redigest_zk_ace_policy(&mut changed_identity);
    mutations.push(changed_identity);
    let mut changed_policy_digest = successor.clone();
    changed_policy_digest.policy_digest = PrivacyPolicyDigestV1::new(raw(22));
    redigest_zk_ace_policy(&mut changed_policy_digest);
    mutations.push(changed_policy_digest);
    let mut changed_asset = successor.clone();
    changed_asset.asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("privacy", "universal").expect("domain"),
        Name::from_str("other_asset").expect("asset name"),
    );
    redigest_zk_ace_policy(&mut changed_asset);
    mutations.push(changed_asset);
    let mut changed_allowlist = successor.clone();
    changed_allowlist.source_allowlist.push(account(99));
    changed_allowlist.source_allowlist.sort_unstable();
    redigest_zk_ace_policy(&mut changed_allowlist);
    mutations.push(changed_allowlist);
    for mutation in mutations {
        assert_eq!(
            validate_zk_ace_policy_revocation_v1(&current, &mutation),
            Err(PrivacyZkAcePolicyTransitionValidationErrorV1::RevocationContentsChanged)
        );
    }
    let mut different_policy = successor.clone();
    different_policy.policy_id = PrivacyPolicyIdV1::new(raw(92));
    redigest_zk_ace_policy(&mut different_policy);
    assert_eq!(
        validate_zk_ace_policy_revocation_v1(&current, &different_policy),
        Err(PrivacyZkAcePolicyTransitionValidationErrorV1::PolicyIdMismatch)
    );
    for epoch in [1, 3] {
        let candidate = zk_ace_policy(epoch, 11, PrivacyZkAcePolicyLifecycleV1::Revoked);
        assert!(matches!(
            validate_zk_ace_policy_revocation_v1(&current, &candidate),
            Err(
                PrivacyZkAcePolicyTransitionValidationErrorV1::NonCanonicalSuccessorEpoch {
                    expected: 2,
                    actual
                }
            ) if actual == epoch
        ));
    }
    let revoked_current = zk_ace_policy(1, 11, PrivacyZkAcePolicyLifecycleV1::Revoked);
    assert_eq!(
        validate_zk_ace_policy_revocation_v1(&revoked_current, &successor),
        Err(PrivacyZkAcePolicyTransitionValidationErrorV1::CurrentNotActive)
    );
    let max_epoch = zk_ace_policy(u64::MAX, 11, PrivacyZkAcePolicyLifecycleV1::Active);
    let max_successor = zk_ace_policy(u64::MAX, 11, PrivacyZkAcePolicyLifecycleV1::Revoked);
    assert_eq!(
        validate_zk_ace_policy_revocation_v1(&max_epoch, &max_successor),
        Err(PrivacyZkAcePolicyTransitionValidationErrorV1::EpochOverflow)
    );
}
fn assert_bootle_lantern_policy_roundtrip(record: &BootleLanternIssuerPolicyV1) {
    record.validate_initial().expect("canonical initial record");
    assert_eq!(
        record
            .computed_record_digest()
            .expect("canonical record digest"),
        record.record_digest
    );
    let encoded = norito::to_bytes(record).expect("encode policy");
    let decoded: BootleLanternIssuerPolicyV1 =
        norito::decode_from_bytes(&encoded).expect("decode policy");
    assert_eq!(&decoded, record);
    let json = norito::json::to_json(record).expect("encode policy JSON");
    let decoded_json: BootleLanternIssuerPolicyV1 =
        norito::json::from_json(&json).expect("decode policy JSON");
    assert_eq!(&decoded_json, record);
    let object_prefix = json
        .strip_suffix('}')
        .expect("policy JSON is a top-level object");
    let unknown_field = format!("{object_prefix},\"legacy_policy_alias\":true}}");
    assert!(
        norito::json::from_json::<BootleLanternIssuerPolicyV1>(&unknown_field).is_err(),
        "unknown JSON fields must not create an alternate first-release policy encoding"
    );
}
#[expect(
    clippy::too_many_lines,
    reason = "all matrix-shape and coefficient boundaries are checked in one helper"
)]
fn assert_bootle_lantern_matrix_boundaries(record: &BootleLanternIssuerPolicyV1) {
    let mut invalid = record.clone();
    invalid.issuer_public_matrix.entries.pop();
    assert!(matches!(
        invalid.validate(),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::InvalidIssuerMatrixEntryCount {
                count: 63,
                expected: 64
            }
        )
    ));
    invalid = record.clone();
    invalid.issuer_public_matrix.entries[0].coefficients.pop();
    assert!(matches!(
        invalid.validate(),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::InvalidPolynomialCoefficientCount {
                polynomial: 0,
                count: 63,
                expected: 64
            }
        )
    ));
    invalid = record.clone();
    invalid.issuer_public_matrix.entries[0].coefficients[0] = BOOTLE_LANTERN_APPLICATION_MODULUS_V1;
    assert!(matches!(
        invalid.validate(),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::NonCanonicalMatrixCoefficient {
                row: 0,
                column: 0,
                coefficient: 0,
                value: BOOTLE_LANTERN_APPLICATION_MODULUS_V1
            }
        )
    ));
    invalid = record.clone();
    for polynomial in &mut invalid.issuer_public_matrix.entries {
        polynomial.coefficients.fill(0);
    }
    assert_eq!(
        invalid.validate(),
        Err(BootleLanternIssuerPolicyValidationErrorV1::AllZeroIssuerMatrix)
    );
    invalid = record.clone();
    invalid.issuer_public_matrix.entries[1].coefficients[7] ^= 1;
    redigest_bootle_lantern_policy(&mut invalid);
    assert!(matches!(
        invalid.validate(),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::InvalidR512MultiplicationMatrix {
                row: 0,
                column: 1,
                coefficient: 7,
                ..
            }
        )
    ));
    let mut monomial_first_column: [BootleLanternPolynomialV1;
        BOOTLE_LANTERN_ISSUER_MATRIX_DIMENSION_V1] =
        core::array::from_fn(|_| BootleLanternPolynomialV1 {
            coefficients: vec![0; BOOTLE_LANTERN_RING_DEGREE_V1],
        });
    monomial_first_column[0].coefficients[0] = 1;
    invalid = record.clone();
    invalid.issuer_public_matrix =
        BootleLanternIssuerPublicMatrixV1::from_r512_first_column_blocks_v1(&monomial_first_column)
            .expect("canonical monomial multiplication matrix shape");
    redigest_bootle_lantern_policy(&mut invalid);
    assert_eq!(
        invalid.validate(),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::SparseIssuerPublicKey {
                nonzero_coefficients: 1,
                minimum: 256,
            }
        )
    );
    let mut sparse_first_column: [BootleLanternPolynomialV1;
        BOOTLE_LANTERN_ISSUER_MATRIX_DIMENSION_V1] =
        core::array::from_fn(|_| BootleLanternPolynomialV1 {
            coefficients: vec![0; BOOTLE_LANTERN_RING_DEGREE_V1],
        });
    for coefficient in sparse_first_column
        .iter_mut()
        .flat_map(|polynomial| &mut polynomial.coefficients)
        .take(BOOTLE_LANTERN_ISSUER_PUBLIC_KEY_MIN_NONZERO_COEFFICIENTS_V1 - 1)
    {
        *coefficient = 1;
    }
    invalid = record.clone();
    invalid.issuer_public_matrix =
        BootleLanternIssuerPublicMatrixV1::from_r512_first_column_blocks_v1(&sparse_first_column)
            .expect("canonical sparse multiplication matrix shape");
    redigest_bootle_lantern_policy(&mut invalid);
    assert_eq!(
        invalid.validate(),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::SparseIssuerPublicKey {
                nonzero_coefficients: 255,
                minimum: 256,
            }
        )
    );
    invalid = record.clone();
    invalid.issuer_parameter_digest.0[0] ^= 1;
    assert_eq!(
        invalid.validate(),
        Err(BootleLanternIssuerPolicyValidationErrorV1::IssuerParameterDigestMismatch)
    );
}
fn dense_bootle_first_column()
-> [BootleLanternPolynomialV1; BOOTLE_LANTERN_ISSUER_MATRIX_DIMENSION_V1] {
    core::array::from_fn(|block| BootleLanternPolynomialV1 {
        coefficients: (0..BOOTLE_LANTERN_RING_DEGREE_V1)
            .map(|coefficient| {
                u16::try_from((block * BOOTLE_LANTERN_RING_DEGREE_V1 + coefficient) % 12_288 + 1)
                    .expect("fixture residue fits u16")
            })
            .collect(),
    })
}
fn negacyclic_basis_shift(coefficients: &[u16], shift: usize) -> Vec<u16> {
    let mut shifted = vec![0_u16; coefficients.len()];
    for (index, coefficient) in coefficients.iter().copied().enumerate() {
        let destination = index + shift;
        if destination < coefficients.len() {
            shifted[destination] = coefficient;
        } else {
            shifted[destination - coefficients.len()] = if coefficient == 0 {
                0
            } else {
                BOOTLE_LANTERN_APPLICATION_MODULUS_V1 - coefficient
            };
        }
    }
    shifted
}
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the R512 constructor mutation corpus is one cohesive algebraic matrix"
)]
fn bootle_lantern_r512_matrix_constructor_is_exact_and_mutation_closed() {
    let first_column = dense_bootle_first_column();
    let mut short_first_column = first_column.clone();
    short_first_column[7].coefficients.pop();
    assert!(matches!(
        BootleLanternIssuerPublicMatrixV1::from_r512_first_column_blocks_v1(&short_first_column),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::InvalidPolynomialCoefficientCount {
                polynomial: 56,
                count: 63,
                expected: 64,
            }
        )
    ));
    let mut noncanonical_first_column = first_column.clone();
    noncanonical_first_column[7].coefficients[63] = BOOTLE_LANTERN_APPLICATION_MODULUS_V1;
    assert!(matches!(
        BootleLanternIssuerPublicMatrixV1::from_r512_first_column_blocks_v1(
            &noncanonical_first_column
        ),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::NonCanonicalMatrixCoefficient {
                row: 7,
                column: 0,
                coefficient: 63,
                value: BOOTLE_LANTERN_APPLICATION_MODULUS_V1,
            }
        )
    ));
    let matrix = BootleLanternIssuerPublicMatrixV1::from_r512_first_column_blocks_v1(&first_column)
        .expect("canonical dense degree-512 public-key matrix");
    matrix
        .validate_r512_multiplication_structure_v1()
        .expect("constructed matrix passes its structural validator");
    let dimension = BOOTLE_LANTERN_ISSUER_MATRIX_DIMENSION_V1;
    let degree = BOOTLE_LANTERN_RING_DEGREE_V1;
    let mut wrong_entry_count = matrix.clone();
    wrong_entry_count.entries.pop();
    assert!(matches!(
        wrong_entry_count.validate_r512_multiplication_structure_v1(),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::InvalidIssuerMatrixEntryCount {
                count: 63,
                expected: 64,
            }
        )
    ));
    let mut wrong_coefficient_count = matrix.clone();
    wrong_coefficient_count.entries[63].coefficients.pop();
    assert!(matches!(
        wrong_coefficient_count.validate_r512_multiplication_structure_v1(),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::InvalidPolynomialCoefficientCount {
                polynomial: 63,
                count: 63,
                expected: 64,
            }
        )
    ));
    let mut noncanonical = matrix.clone();
    noncanonical.entries[63].coefficients[63] = BOOTLE_LANTERN_APPLICATION_MODULUS_V1;
    assert!(matches!(
        noncanonical.validate_r512_multiplication_structure_v1(),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::NonCanonicalMatrixCoefficient {
                row: 7,
                column: 7,
                coefficient: 63,
                value: BOOTLE_LANTERN_APPLICATION_MODULUS_V1,
            }
        )
    ));
    let all_zero = BootleLanternIssuerPublicMatrixV1 {
        entries: vec![
            BootleLanternPolynomialV1 {
                coefficients: vec![0; degree],
            };
            dimension * dimension
        ],
    };
    assert_eq!(
        all_zero.validate_r512_multiplication_structure_v1(),
        Err(BootleLanternIssuerPolicyValidationErrorV1::AllZeroIssuerMatrix)
    );
    let mut dependent_blocks = 0_usize;
    for row in 0..dimension {
        for column in 0..dimension {
            let actual = &matrix.entries[row * dimension + column].coefficients;
            if column == 0 {
                assert_eq!(actual, &first_column[row].coefficients);
                continue;
            }
            dependent_blocks += 1;
            if row >= column {
                assert_eq!(
                    actual,
                    &first_column[row - column].coefficients,
                    "lower-triangular block B[{row}][{column}] did not reuse H_{}",
                    row - column
                );
            } else {
                let source = &first_column[row + dimension - column].coefficients;
                let expected = negacyclic_basis_shift(source, 1);
                assert_eq!(
                    actual,
                    &expected,
                    "upper-triangular block B[{row}][{column}] is not Y times H_{}",
                    row + dimension - column
                );
                assert_eq!(
                    actual[0],
                    BOOTLE_LANTERN_APPLICATION_MODULUS_V1 - source[degree - 1],
                    "upper block wrap coefficient did not negate modulo 12289"
                );
                assert_eq!(
                    actual[17], source[16],
                    "upper block non-wrap coefficient did not shift by one"
                );
            }
        }
    }
    assert_eq!(
        dependent_blocks, 56,
        "every dependent block must be checked"
    );
    let mut stale_dependents = matrix.clone();
    stale_dependents.entries[3 * dimension].coefficients[5] ^= 1;
    assert!(matches!(
        stale_dependents.validate_r512_multiplication_structure_v1(),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::InvalidR512MultiplicationMatrix {
                row: 0,
                column: 5,
                coefficient: 6,
                ..
            }
        )
    ));
    let mut h = vec![0_u16; dimension * degree];
    for block in 0..dimension {
        for coefficient in 0..degree {
            h[block + dimension * coefficient] = first_column[block].coefficients[coefficient];
        }
    }
    for basis in 0..h.len() {
        let direct = negacyclic_basis_shift(&h, basis);
        let input_block = basis % dimension;
        let input_coefficient = basis / dimension;
        let mut through_blocks = vec![0_u16; h.len()];
        for output_block in 0..dimension {
            let block = &matrix.entries[output_block * dimension + input_block].coefficients;
            let product = negacyclic_basis_shift(block, input_coefficient);
            for coefficient in 0..degree {
                through_blocks[output_block + dimension * coefficient] = product[coefficient];
            }
        }
        assert_eq!(
            through_blocks, direct,
            "R512 multiplication mismatch for basis vector X^{basis}"
        );
    }
}
fn assert_bootle_lantern_allowed_value_boundaries(record: &BootleLanternIssuerPolicyV1) {
    let mut invalid = record.clone();
    invalid.allowed_values.pop();
    assert!(matches!(
        invalid.validate(),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::InvalidAllowedValueRuleCount {
                count: 7,
                expected: 8
            }
        )
    ));
    invalid = record.clone();
    invalid.allowed_values[0]
        .values
        .push(BootleLanternAttributeValueV1::new([1; 8]));
    assert_eq!(
        invalid.validate(),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::AllowedValuesForOptionalAttribute {
                index: 0
            }
        )
    );
    invalid = record.clone();
    invalid.allowed_values[1].values = vec![
        BootleLanternAttributeValueV1::new([2; 8]),
        BootleLanternAttributeValueV1::new([2; 8]),
    ];
    assert_eq!(
        invalid.validate(),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::AllowedValuesNotStrictlyIncreasing {
                index: 1
            }
        )
    );
    invalid = record.clone();
    invalid.allowed_values[1].values =
        vec![
            BootleLanternAttributeValueV1::new([3; 8]);
            usize::try_from(BOOTLE_LANTERN_MAX_ALLOWED_VALUES_PER_ATTRIBUTE_V1 + 1)
                .expect("test bound fits usize")
        ];
    assert!(matches!(
        invalid.validate(),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::TooManyAllowedValues {
                index: 1,
                count: 33,
                max: 32
            }
        )
    ));
    invalid = record.clone();
    invalid.record_digest = PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]);
    assert_eq!(
        invalid.validate(),
        Err(BootleLanternIssuerPolicyValidationErrorV1::ZeroRecordDigest)
    );
    invalid = record.clone();
    invalid.record_digest = PrivacyBootleLanternIssuerPolicyDigestV1::new(raw(199));
    assert_eq!(
        invalid.validate(),
        Err(BootleLanternIssuerPolicyValidationErrorV1::RecordDigestMismatch)
    );
}
fn assert_bootle_lantern_policy_rotation_boundaries(record: &BootleLanternIssuerPolicyV1) {
    let mut successor = record.clone();
    successor.epoch = 2;
    successor.required_disclosure_bitmap |= 1;
    redigest_bootle_lantern_policy(&mut successor);
    successor
        .validate_successor(record)
        .expect("strict policy rotation");
    let mut non_consecutive = successor.clone();
    non_consecutive.epoch = 3;
    redigest_bootle_lantern_policy(&mut non_consecutive);
    assert!(matches!(
        non_consecutive.validate_successor(record),
        Err(
            BootleLanternIssuerPolicyValidationErrorV1::NonConsecutiveEpoch {
                previous: 1,
                next: 3,
                expected: 2
            }
        )
    ));
    let mut unchanged = record.clone();
    unchanged.epoch = 2;
    redigest_bootle_lantern_policy(&mut unchanged);
    assert_eq!(
        unchanged.validate_successor(record),
        Err(BootleLanternIssuerPolicyValidationErrorV1::UnchangedRotation)
    );
    let mut revoked = record.clone();
    revoked.epoch = 2;
    revoked.lifecycle = BootleLanternIssuerPolicyLifecycleV1::Revoked;
    redigest_bootle_lantern_policy(&mut revoked);
    revoked
        .validate_revocation_successor(record)
        .expect("exact terminal revocation");
    let mut revocation_with_rotation = revoked.clone();
    revocation_with_rotation.required_disclosure_bitmap ^= 1;
    redigest_bootle_lantern_policy(&mut revocation_with_rotation);
    assert_eq!(
        revocation_with_rotation.validate_revocation_successor(record),
        Err(BootleLanternIssuerPolicyValidationErrorV1::RevocationMustPreservePolicy)
    );
    assert_eq!(
        revoked.validate_successor(&revoked),
        Err(BootleLanternIssuerPolicyValidationErrorV1::PolicyAlreadyRevoked)
    );
    let mut wrong_initial_epoch = record.clone();
    wrong_initial_epoch.epoch = 2;
    redigest_bootle_lantern_policy(&mut wrong_initial_epoch);
    assert_eq!(
        wrong_initial_epoch.validate_initial(),
        Err(BootleLanternIssuerPolicyValidationErrorV1::InvalidInitialEpoch { epoch: 2 })
    );
    let mut revoked_initial = record.clone();
    revoked_initial.lifecycle = BootleLanternIssuerPolicyLifecycleV1::Revoked;
    redigest_bootle_lantern_policy(&mut revoked_initial);
    assert_eq!(
        revoked_initial.validate_initial(),
        Err(BootleLanternIssuerPolicyValidationErrorV1::InitialPolicyMustBeActive)
    );
}
#[test]
fn bootle_lantern_issuer_policy_is_canonical_bounded_and_rotates_monotonically() {
    let record = bootle_lantern_policy();
    assert_bootle_lantern_policy_roundtrip(&record);
    assert_bootle_lantern_matrix_boundaries(&record);
    assert_bootle_lantern_allowed_value_boundaries(&record);
    assert_bootle_lantern_policy_rotation_boundaries(&record);
}
fn assert_orchard_count_and_ciphertext_boundaries(limits: &PrivacyConsensusLimitsV1) {
    let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
    let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
        unreachable!()
    };
    statement.actions.clear();
    assert!(matches!(
        orchard.validate(limits),
        Err(PrivacyStatementValidationError::InvalidOrchardActionCount { count: 0, max: 2 })
    ));
    let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
    let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
        unreachable!()
    };
    statement.actions = vec![
        orchard_action(110),
        orchard_action(120),
        orchard_action(130),
    ];
    assert!(matches!(
        orchard.validate(limits),
        Err(PrivacyStatementValidationError::InvalidOrchardActionCount { count: 3, max: 2 })
    ));
    for malformed_len in [
        ORCHARD_ENCRYPTED_NOTE_BYTES_V1 - 1,
        ORCHARD_ENCRYPTED_NOTE_BYTES_V1 + 1,
    ] {
        let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
        let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
            unreachable!()
        };
        statement.actions[0]
            .encrypted_note
            .resize(malformed_len, 0xA5);
        assert!(matches!(
            orchard.validate(limits),
            Err(PrivacyStatementValidationError::InvalidOrchardEncryptedNoteSize { index: 0, .. })
        ));
    }
    for malformed_len in [
        ORCHARD_OUTGOING_CIPHERTEXT_BYTES_V1 - 1,
        ORCHARD_OUTGOING_CIPHERTEXT_BYTES_V1 + 1,
    ] {
        let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
        let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
            unreachable!()
        };
        statement.actions[0]
            .outgoing_ciphertext
            .resize(malformed_len, 0xA5);
        assert!(matches!(
            orchard.validate(limits),
            Err(
                PrivacyStatementValidationError::InvalidOrchardOutgoingCiphertextSize {
                    index: 0,
                    ..
                }
            )
        ));
    }
}
fn assert_orchard_uniqueness_and_balance_boundaries(limits: &PrivacyConsensusLimitsV1) {
    let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
    let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
        unreachable!()
    };
    statement.actions.push(orchard_action(120));
    statement.actions[1].nullifier = statement.actions[0].nullifier;
    assert_eq!(
        orchard.validate(limits),
        Err(PrivacyStatementValidationError::DuplicateOrchardNullifier { index: 1 })
    );
    let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
    let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
        unreachable!()
    };
    statement.actions.push(orchard_action(120));
    statement.actions[1].note_commitment = statement.actions[0].note_commitment;
    assert_eq!(
        orchard.validate(limits),
        Err(PrivacyStatementValidationError::DuplicateOrchardNoteCommitment { index: 1 })
    );
    let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
    let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
        unreachable!()
    };
    statement.value_balance = PrivacyValueBalanceV1 {
        direction: PrivacyValueBalanceDirectionV1::OutOfPool,
        amount: ORCHARD_MAX_VALUE_BALANCE_V1 + 1,
    };
    assert_eq!(
        orchard.validate(limits),
        Err(
            PrivacyStatementValidationError::OrchardValueBalanceOutOfRange {
                amount: ORCHARD_MAX_VALUE_BALANCE_V1 + 1,
                max: ORCHARD_MAX_VALUE_BALANCE_V1,
            }
        )
    );
    let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
    let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
        unreachable!()
    };
    statement.actions.push(orchard_action(120));
    statement.actions[0].nullifier = [0; 32];
    statement.actions[0].note_commitment = [0; 32];
    orchard
        .validate(limits)
        .expect("zero is a canonical Pallas field encoding, not a schema sentinel");
}
#[expect(
    clippy::too_many_lines,
    reason = "the non-Orchard private-transfer boundary cases form one protocol matrix"
)]
fn assert_other_private_transfer_shape_boundaries(limits: &PrivacyConsensusLimitsV1) {
    let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
    let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
        unreachable!()
    };
    statement.inputs = vec![fcmp_input(121), fcmp_input(122), fcmp_input(123)];
    assert!(matches!(
        fcmp.validate(limits),
        Err(PrivacyStatementValidationError::InvalidFcmpInputCount {
            count: 3,
            max: FCMP_MAX_INPUTS_V1
        })
    ));
    let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
    let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
        unreachable!()
    };
    statement.output_set_root.layers = 0;
    assert!(matches!(
        fcmp.validate(limits),
        Err(PrivacyStatementValidationError::InvalidFcmpTreeRoot(
            PrivacyFcmpTreeRootValidationErrorV1::InvalidLayerCount { layers: 0, .. }
        ))
    ));
    let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
    let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
        unreachable!()
    };
    statement.inputs[0].rerandomization_commitment = [0; 32];
    assert_eq!(
        fcmp.validate(limits),
        Err(PrivacyStatementValidationError::InvalidFcmpInput {
            index: 0,
            source: PrivacyFcmpInputValidationErrorV1::ZeroComponent {
                component: PrivacyFcmpInputComponentV1::RerandomizationCommitment,
            },
        })
    );
    for duplicate_key_image in [true, false] {
        let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
        let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
            unreachable!()
        };
        statement.inputs.push(fcmp_input(141));
        if duplicate_key_image {
            statement.inputs[1].key_image = statement.inputs[0].key_image;
            assert_eq!(
                fcmp.validate(limits),
                Err(PrivacyStatementValidationError::DuplicateFcmpKeyImage { index: 1 })
            );
        } else {
            statement.inputs[1].pseudo_out = statement.inputs[0].pseudo_out;
            assert_eq!(
                fcmp.validate(limits),
                Err(PrivacyStatementValidationError::DuplicateFcmpPseudoOut { index: 1 })
            );
        }
    }
    let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
    let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
        unreachable!()
    };
    statement.outputs[0].amount_commitment = [0; 32];
    assert_eq!(
        fcmp.validate(limits),
        Err(PrivacyStatementValidationError::InvalidFcmpOutput {
            index: 0,
            source: PrivacyFcmpOutputTupleValidationErrorV1::ZeroComponent {
                component: PrivacyFcmpOutputComponentV1::AmountCommitment,
            },
        })
    );
    let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
    let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
        unreachable!()
    };
    statement.outputs.push(statement.outputs[0]);
    statement
        .encrypted_outputs
        .push(statement.encrypted_outputs[0].clone());
    assert_eq!(
        fcmp.validate(limits),
        Err(PrivacyStatementValidationError::DuplicateFcmpOutputId { index: 1 })
    );
    let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
    let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
        unreachable!()
    };
    statement.encrypted_outputs[0].output_id = fcmp_output(222).output_id();
    assert_eq!(
        fcmp.validate(limits),
        Err(PrivacyStatementValidationError::FcmpEncryptedOutputIdMismatch { index: 0 })
    );
    let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
    let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
        unreachable!()
    };
    statement.encrypted_outputs.clear();
    assert_eq!(
        fcmp.validate(limits),
        Err(PrivacyStatementValidationError::MissingEncryptedOutput)
    );
    let mut ivm = statement_for(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
    let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) = &mut ivm else {
        unreachable!()
    };
    statement.action_digest = PrivacyActionDigestV1::new([0; 32]);
    assert_eq!(
        ivm.validate(limits),
        Err(PrivacyStatementValidationError::ZeroTypedField {
            field: PrivacyTypedFieldV1::ActionDigest,
        })
    );
    let mut ivm = statement_for(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
    let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) = &mut ivm else {
        unreachable!()
    };
    statement.action_digest.0[0] ^= 1;
    assert_eq!(
        ivm.validate(limits),
        Err(PrivacyStatementValidationError::ActionDigestMismatch)
    );
    let mut ivm = statement_for(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
    let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) = &mut ivm else {
        unreachable!()
    };
    statement.execution_epoch += 1;
    statement.action_digest = statement
        .computed_action_digest()
        .expect("recompute mismatched-epoch action digest");
    assert_eq!(
        ivm.validate(limits),
        Err(PrivacyStatementValidationError::EpochBindingMismatch {
            field: PrivacyEpochFieldV1::Execution,
            root_epoch: 15,
            bound_epoch: 16,
        })
    );
    let mut ivm = statement_for(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
    let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) = &mut ivm else {
        unreachable!()
    };
    statement.nullifiers = vec![nullifier(127), nullifier(128), nullifier(129)];
    statement.action_digest = statement
        .computed_action_digest()
        .expect("recompute oversized-input action digest");
    assert!(matches!(
        ivm.validate(limits),
        Err(PrivacyStatementValidationError::TooManyNullifiers {
            count: 3,
            max: IVM_PRIVATE_NOTE_MAX_INPUTS_V1
        })
    ));
    let mut pq = statement_for(PrivacyProtocolIdV1::PqMaspStarkV0);
    let PrivacyStatementV1::PqMaspStarkV0(statement) = &mut pq else {
        unreachable!()
    };
    statement.output_commitments = vec![commitment(130), commitment(131), commitment(132)];
    assert!(matches!(
        pq.validate(limits),
        Err(PrivacyStatementValidationError::TooManyCommitments {
            count: 3,
            max: PQ_MASP_MAX_OUTPUTS_V1
        })
    ));
    let mut pq = statement_for(PrivacyProtocolIdV1::PqMaspStarkV0);
    let PrivacyStatementV1::PqMaspStarkV0(statement) = &mut pq else {
        unreachable!()
    };
    statement.authorization_epoch += 1;
    assert_eq!(
        pq.validate(limits),
        Err(PrivacyStatementValidationError::EpochBindingMismatch {
            field: PrivacyEpochFieldV1::Authorization,
            root_epoch: 17,
            bound_epoch: 18,
        })
    );
    let mut malformed = statement_for(PrivacyProtocolIdV1::PqMaspStarkV0);
    let PrivacyStatementV1::PqMaspStarkV0(statement) = &mut malformed else {
        unreachable!()
    };
    statement.encrypted_outputs[0].recipient = PrivacyRecipientIdV1::new([0; 32]);
    assert!(matches!(
        malformed.validate(limits),
        Err(PrivacyStatementValidationError::ZeroEncryptedOutputRecipient { index: 0 })
    ));
}
#[test]
fn private_transfer_shapes_enforce_hard_caps_and_ordered_ciphertexts() {
    let limits = PrivacyConsensusLimitsV1::taira_default();
    assert_orchard_count_and_ciphertext_boundaries(&limits);
    assert_orchard_uniqueness_and_balance_boundaries(&limits);
    assert_other_private_transfer_shape_boundaries(&limits);
}
#[test]
fn private_ivm_encrypted_output_codec_is_exact_and_fail_closed() {
    assert_eq!(PRIVACY_IVM_PRIVATE_NOTE_PLAINTEXT_BYTES_V1, 180);
    assert_eq!(PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1, 224);
    let canonical = statement_for(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
    let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) = &canonical else {
        unreachable!()
    };
    let ciphertext = statement.encrypted_outputs[0].ciphertext.clone();
    assert_eq!(
        ciphertext.len(),
        PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1
    );
    assert_eq!(
        ciphertext.get(..4),
        Some(PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_MAGIC_V1.as_slice())
    );
    let mut malformed = Vec::new();
    let mut truncated = ciphertext.clone();
    truncated.pop();
    malformed.push(truncated);
    let mut suffixed = ciphertext.clone();
    suffixed.push(0);
    malformed.push(suffixed);
    let mut wrong_magic = ciphertext.clone();
    wrong_magic[0] ^= 1;
    malformed.push(wrong_magic);
    let mut zero_nonce = ciphertext.clone();
    zero_nonce[4..4 + PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_NONCE_BYTES_V1].fill(0);
    malformed.push(zero_nonce);
    let mut zero_payload = ciphertext;
    zero_payload[4 + PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_NONCE_BYTES_V1..].fill(0);
    malformed.push(zero_payload);
    for ciphertext in malformed {
        let mut ivm = statement_for(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
        let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) = &mut ivm else {
            unreachable!()
        };
        statement.encrypted_outputs[0].ciphertext = ciphertext;
        statement.action_digest = statement
            .computed_action_digest()
            .expect("recompute malformed-codec action digest");
        assert_eq!(
            ivm.validate(&PrivacyConsensusLimitsV1::taira_default()),
            Err(
                PrivacyStatementValidationError::InvalidIvmPrivateEncryptedOutputCodec { index: 0 }
            )
        );
    }
}
#[test]
fn lifecycle_edges_preserve_history_and_retirement_is_terminal() {
    let proposed = PrivacyProtocolLifecycleV1::Proposed(PrivacyProposedLifecycleV1 {
        proposed_at_height: 1,
        activate_at_height: 3,
    });
    let active = PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
        proposed_at_height: 1,
        activated_at_height: 3,
        state_since_height: 3,
    });
    let suspended = PrivacyProtocolLifecycleV1::Suspended(PrivacySuspendedLifecycleV1 {
        proposed_at_height: 1,
        activated_at_height: 3,
        state_since_height: 4,
    });
    let resumed = PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
        proposed_at_height: 1,
        activated_at_height: 3,
        state_since_height: 5,
    });
    let retired = PrivacyProtocolLifecycleV1::Retired(PrivacyRetiredLifecycleV1 {
        proposed_at_height: 1,
        activated_at_height: Some(3),
        state_since_height: 6,
    });
    proposed
        .validate_transition_to(&active)
        .expect("proposal activates");
    for mismatched_active in [
        PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
            proposed_at_height: 2,
            activated_at_height: 3,
            state_since_height: 3,
        }),
        PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
            proposed_at_height: 1,
            activated_at_height: 2,
            state_since_height: 2,
        }),
        PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
            proposed_at_height: 1,
            activated_at_height: 3,
            state_since_height: 4,
        }),
    ] {
        assert_eq!(
            proposed.validate_transition_to(&mismatched_active),
            Err(PrivacyLifecycleTransitionError::InvalidTransition)
        );
    }
    active
        .validate_transition_to(&suspended)
        .expect("active suspends");
    suspended
        .validate_transition_to(&resumed)
        .expect("suspension resumes");
    resumed
        .validate_transition_to(&retired)
        .expect("active retires");
    assert!(retired.validate_transition_to(&active).is_err());
    let invalid = PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
        proposed_at_height: 3,
        activated_at_height: 3,
        state_since_height: 3,
    });
    assert!(invalid.validate().is_err());
    let rewritten_history = PrivacyProtocolLifecycleV1::Suspended(PrivacySuspendedLifecycleV1 {
        proposed_at_height: 2,
        activated_at_height: 3,
        state_since_height: 4,
    });
    assert!(active.validate_transition_to(&rewritten_history).is_err());
}
#[test]
fn activation_effective_height_uses_active_lifecycle_payload() {
    let limits = PrivacyConsensusLimitsV1::taira_default();
    let envelope = envelope(statement_for(
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
    ));
    let mut activation = activation(&envelope);
    activation.lifecycle = PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
        proposed_at_height: 1,
        activated_at_height: 2,
        state_since_height: 5,
    });
    assert_eq!(
        envelope.validate_against_activation(&activation, &limits, 4),
        Err(
            PrivacyProofEnvelopeValidationError::ActivationNotEffective {
                current_height: 4,
                effective_height: 5,
            }
        )
    );
    assert_eq!(
        envelope.validate_against_activation(&activation, &limits, 5),
        Ok(())
    );
}
#[test]
fn envelopes_fail_closed_on_every_binding_and_resource_mutation() {
    let limits = PrivacyConsensusLimitsV1::taira_default();
    let statement = statement_for(PrivacyProtocolIdV1::VeRangeTransparentRangeV1);
    let base = envelope(statement);
    base.validate_with_limits(&limits).expect("valid envelope");
    let mut invalid = base.clone();
    invalid.protocol_id = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
    assert!(invalid.validate_with_limits(&limits).is_err());
    invalid = base.clone();
    invalid.proof_system_id = PrivacyProofSystemIdV1::StarkFriSha256Goldilocks;
    assert!(invalid.validate_with_limits(&limits).is_err());
    invalid = base.clone();
    invalid.engine_id = PrivacyEngineIdV1::NativeJindo;
    assert!(invalid.validate_with_limits(&limits).is_err());
    invalid = base.clone();
    invalid.parameter_id = PrivacyParameterIdV1::new(raw(220));
    assert!(invalid.validate_with_limits(&limits).is_err());
    invalid = base.clone();
    invalid.parameter_digest = PrivacyParameterDigestV1::new([0; 32]);
    assert!(invalid.validate_with_limits(&limits).is_err());
    invalid = base.clone();
    invalid.verifier_digest = PrivacyVerifierDigestV1::new([0; 32]);
    assert!(invalid.validate_with_limits(&limits).is_err());
    invalid = base.clone();
    invalid.statement_schema_digest = PrivacyStatementSchemaDigestV1::new([0; 32]);
    assert!(invalid.validate_with_limits(&limits).is_err());
    invalid = base.clone();
    invalid.engine_manifest_digest = PrivacyEngineManifestDigestV1::new([0; 32]);
    assert!(invalid.validate_with_limits(&limits).is_err());
    invalid = base.clone();
    invalid.statement_digest = PrivacyStatementDigestV1::new(raw(221));
    assert!(invalid.validate_with_limits(&limits).is_err());
    invalid = base.clone();
    invalid.proof = PrivacyProofV1::ZkAcePqAuthorizationV0(PrivacyProofBytesV1::new(vec![1]));
    assert!(invalid.validate_with_limits(&limits).is_err());
    invalid = base.clone();
    invalid.proof = PrivacyProofV1::VeRangeTransparentRangeV1(PrivacyProofBytesV1::new(Vec::new()));
    assert!(invalid.validate_with_limits(&limits).is_err());
    invalid = base.clone();
    invalid.proof = PrivacyProofV1::VeRangeTransparentRangeV1(PrivacyProofBytesV1::new(vec![0; 3]));
    assert!(invalid.validate_with_limits(&limits).is_err());
    let mut proof_limited = limits;
    proof_limited.max_proof_bytes_per_action = 2;
    proof_limited.validate().expect("lower proof limit");
    assert!(base.validate_with_limits(&proof_limited).is_err());
    let mut governed = activation(&base);
    base.validate_against_activation(&governed, &limits, 2)
        .expect("active matching activation");
    assert!(
        base.validate_against_activation(&governed, &limits, 1)
            .is_err()
    );
    governed.parameter_digest = PrivacyParameterDigestV1::new(raw(222));
    assert!(
        base.validate_against_activation(&governed, &limits, 2)
            .is_err()
    );
    governed = activation(&base);
    governed.lifecycle = PrivacyProtocolLifecycleV1::Suspended(PrivacySuspendedLifecycleV1 {
        proposed_at_height: 1,
        activated_at_height: 2,
        state_since_height: 3,
    });
    assert!(
        base.validate_against_activation(&governed, &limits, 3)
            .is_err()
    );
    governed = activation(&base);
    governed.protocol_limits =
        PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(VeRangeActivationLimitsV1 {
            max_aggregation_count: 1,
        });
    assert!(
        base.validate_against_activation(&governed, &limits, 2)
            .is_err()
    );
    let framed = norito::to_bytes(&base).expect("frame envelope");
    let mut truncated = framed.clone();
    truncated.pop();
    assert!(norito::decode_from_bytes::<PrivacyProofEnvelopeV1>(&truncated).is_err());
    let mut trailing = framed;
    trailing.push(0);
    assert!(norito::decode_from_bytes::<PrivacyProofEnvelopeV1>(&trailing).is_err());
    for unknown in [99_u32, u32::MAX] {
        assert!(PrivacyProofSystemIdV1::decode(&mut unknown.to_le_bytes().as_slice()).is_err());
        assert!(PrivacyEngineIdV1::decode(&mut unknown.to_le_bytes().as_slice()).is_err());
        assert!(PrivacyStatementV1::decode(&mut unknown.to_le_bytes().as_slice()).is_err());
    }
}
