#[test]
fn ironwood_appends_protocol_without_expanding_exact12_snapshots() {
    let protocol = PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1;
    assert_eq!(PrivacyRuntimeProtocolIdV1::COUNT, 12);
    assert_eq!(PrivacyRuntimeProtocolIdV1::SUPPORTED_COUNT, 13);
    assert_eq!(PrivacyRuntimeProtocolIdV1::SUPPORTED_PROTOCOLS[12], protocol);
    assert_eq!(
        PrivacyRuntimeProtocolIdV1::ALL,
        PrivacyRuntimeProtocolIdV1::EXACT12_PROTOCOLS
    );
    assert!(!PrivacyRuntimeProtocolIdV1::ALL.contains(&protocol));
    assert_eq!(PrivacyRuntimeProtocolIdV1::EXACT12_COUNT, 12);
    assert_eq!(
        PrivacyRuntimeProtocolIdV1::EXACT12_PROTOCOLS,
        ORCHARD_COMPATIBILITY_PROTOCOLS_V1
    );
    assert_eq!(protocol.encode(), 12_u32.to_le_bytes());
    assert_eq!(protocol.canonical_label(), "ironwood-halo2-actions-v1");
    assert_eq!(
        protocol.canonical_typed_variant_label(),
        "IronwoodHalo2ActionsV1"
    );
    assert!(!privacy_protocol_label_is_exact12_v1(
        protocol.canonical_label()
    ));
    assert_eq!(
        protocol.security_model(),
        PrivacySecurityModelV1::ClassicalRom
    );
    assert_eq!(
        protocol.expected_proof_system(),
        PrivacyProofSystemIdV1::Halo2IpaPasta
    );
    assert_eq!(
        protocol.expected_engine(),
        PrivacyEngineIdV1::NativeHalo2Orchard
    );
    assert_eq!(
        protocol.expected_operation_schema(),
        PrivacyOperationSchemaV1::OrchardNoteActionV1
    );
    assert_eq!(
        protocol.expected_execution_mode(),
        PrivacyExecutionModeV1::NoteAction
    );
    assert_eq!(
        protocol.expected_feature_mask(),
        PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1.expected_feature_mask()
    );
    let json = norito::json::to_json(&protocol).expect("protocol JSON");
    assert_eq!(
        norito::json::from_json::<PrivacyRuntimeProtocolIdV1>(&json).unwrap(),
        protocol
    );
    let mut snapshot = capability_snapshot();
    snapshot.protocols[8].protocol_id = protocol;
    assert!(
        snapshot.validate().is_err(),
        "an extension cannot replace an Exact12 row"
    );
    let mut catalog = compiled_profile_catalog();
    catalog.protocols[8].protocol_id = protocol;
    assert!(
        catalog.validate().is_err(),
        "the original catalog is closed"
    );
}

#[test]
fn ironwood_statement_proof_and_envelope_roundtrip_with_distinct_binding() {
    let protocol = PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1;
    let statement = statement_for(protocol);
    let old_statement = statement_for(PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1);
    let proof = proof_for(protocol);
    let limits = PrivacyConsensusLimitsV1::taira_default();
    assert_eq!(&statement.encode()[..4], 12_u32.to_le_bytes());
    assert_eq!(&proof.encode()[..4], 12_u32.to_le_bytes());
    assert_eq!(
        statement.action_role_v1(),
        PrivacyActionRoleV1::IronwoodActions
    );
    assert_eq!(proof.action_role_v1(), PrivacyActionRoleV1::IronwoodActions);
    assert_eq!(PrivacyActionRoleV1::IronwoodActions.protocol_id(), protocol);
    assert_eq!(
        PrivacyActionRoleV1::IronwoodActions.canonical_label(),
        "ironwood-actions"
    );
    assert_eq!(
        protocol.action_roles_v1(),
        &[PrivacyActionRoleV1::IronwoodActions]
    );
    assert_ne!(statement.digest().unwrap(), old_statement.digest().unwrap());
    statement
        .validate(&limits)
        .expect("valid Ironwood statement");
    let namespace = PrivacyRuntimeNamespaceV1::from_statement(&statement);
    namespace.validate().expect("Ironwood pool namespace");
    assert_eq!(namespace.protocol_id(), protocol);
    assert_eq!(
        namespace.scope(),
        PrivacyRuntimeNamespaceV1::from_statement(&old_statement).scope()
    );
    assert!(PrivacyRootRoleV1::NoteCommitmentAnchor.is_compatible_with(protocol));
    assert!(!PrivacyRootRoleV1::PgcAccountState.is_compatible_with(protocol));
    let json = norito::json::to_json(&statement).unwrap();
    assert_eq!(
        norito::json::from_json::<PrivacyRuntimeStatementV1>(&json).unwrap(),
        statement
    );
    let json = norito::json::to_json(&proof).unwrap();
    assert_eq!(
        norito::json::from_json::<PrivacyRuntimeProofV1>(&json).unwrap(),
        proof
    );
    let envelope = PrivacyRuntimeProofEnvelopeV1::bind_v1(statement.clone(), proof.clone()).unwrap();
    assert_eq!(
        envelope.action_role_v1().unwrap(),
        PrivacyActionRoleV1::IronwoodActions
    );
    let compiled = profile(&envelope);
    envelope
        .validate_against_profile_v1(&compiled, &compiled.protocol_limits, &limits)
        .unwrap();
    let bytes = norito::encode_canonical(&envelope).unwrap();
    assert_eq!(
        norito::decode_canonical::<PrivacyRuntimeProofEnvelopeV1>(&bytes).unwrap(),
        envelope
    );
    let old_envelope = super::exact12_fixture::envelope(old_statement.clone());
    let old_compiled = profile(&old_envelope);
    assert!(matches!(
        envelope.validate_against_profile_v1(&old_compiled, &old_compiled.protocol_limits, &limits),
        Err(PrivacyProofEnvelopeValidationError::ProfileProtocolMismatch { .. })
    ));
    assert!(matches!(
        old_envelope.validate_against_profile_v1(&compiled, &compiled.protocol_limits, &limits),
        Err(PrivacyProofEnvelopeValidationError::ProfileProtocolMismatch { .. })
    ));
    for (statement, proof) in [
        (
            statement,
            proof_for(PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1),
        ),
        (old_statement, proof),
    ] {
        assert!(matches!(
            PrivacyRuntimeProofEnvelopeV1::bind_v1(statement, proof),
            Err(PrivacyProofEnvelopeValidationError::ProofProtocolMismatch { .. })
        ));
    }
    let mut mutated = envelope.clone();
    mutated.statement.context_mut().transaction_intent_digest =
        PrivacyTransactionIntentDigestV1::new(raw(250));
    assert_ne!(mutated.statement.context(), envelope.statement.context());
    *mutated.proof.bytes_mut() = PrivacyProofBytesV1::new(vec![7; 4]);
    assert_eq!(mutated.proof.bytes().as_bytes(), &[7; 4]);
}

#[test]
fn ironwood_limits_are_separate_and_keep_the_two_action_ceiling() {
    let protocol = PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1;
    let limits = PrivacyRuntimeProtocolLimitsV1::canonical_default_v1(protocol);
    let old_limits =
        PrivacyRuntimeProtocolLimitsV1::canonical_default_v1(PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1);
    assert_eq!(
        limits,
        PrivacyRuntimeProtocolLimitsV1::IronwoodHalo2ActionsV1(OrchardLimitsV1 {
            max_action_count: 2
        })
    );
    assert_eq!(&limits.encode()[..4], 12_u32.to_le_bytes());
    limits.validate_with_ceiling(&limits).unwrap();
    assert!(limits.validate_with_ceiling(&old_limits).is_err());
    assert!(old_limits.validate_with_ceiling(&limits).is_err());
    let json = norito::json::to_json(&limits).unwrap();
    assert_eq!(
        norito::json::from_json::<PrivacyRuntimeProtocolLimitsV1>(&json).unwrap(),
        limits
    );
    let policy = PrivacyProtocolPolicyV1::canonical_default_v1(protocol);
    policy.validate().unwrap();
    let bytes = norito::encode_canonical(&policy).unwrap();
    assert_eq!(
        norito::decode_canonical::<PrivacyProtocolPolicyV1>(&bytes).unwrap(),
        policy
    );
    let statement = statement_for(protocol);
    limits.validate_statement(&statement).unwrap();
    assert!(old_limits.validate_statement(&statement).is_err());
    assert!(
        limits
            .validate_statement(&statement_for(PrivacyRuntimeProtocolIdV1::OrchardHalo2ActionsV1))
            .is_err()
    );
    for count in [0, 3] {
        assert!(
            PrivacyRuntimeProtocolLimitsV1::IronwoodHalo2ActionsV1(OrchardLimitsV1 {
                max_action_count: count
            })
            .validate()
            .is_err()
        );
        let mut invalid = statement.clone();
        let PrivacyRuntimeStatementV1::IronwoodHalo2ActionsV1(inner) = &mut invalid else {
            unreachable!()
        };
        inner.actions.resize(count as usize, orchard_action(80));
        assert!(
            invalid
                .validate(&PrivacyConsensusLimitsV1::taira_default())
                .is_err()
        );
    }
    let mut invalid = statement;
    let PrivacyRuntimeStatementV1::IronwoodHalo2ActionsV1(inner) = &mut invalid else {
        unreachable!()
    };
    inner.anchor = PrivacyRootV1::new([0; 32]);
    assert!(
        invalid
            .validate(&PrivacyConsensusLimitsV1::taira_default())
            .is_err()
    );
}

#[test]
fn ironwood_bootstrap_has_a_separate_namespace_frame_and_digest() {
    let bootstrap = PrivacyIronwoodPoolBootstrapV1::new(
        PrivacyPoolIdV1::new(raw(210)),
        asset_definition_id(),
        AssetBalanceScope::Global,
        account(211),
    )
    .unwrap();
    let old = PrivacyOrchardPoolBootstrapV1::new(
        bootstrap.pool_id,
        bootstrap.asset_definition_id.clone(),
        bootstrap.public_balance_scope,
        bootstrap.reserve_account.clone(),
    )
    .unwrap();
    assert_ne!(bootstrap.namespace(), old.namespace());
    assert_eq!(
        bootstrap.namespace().protocol_id(),
        PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1
    );
    assert_ne!(
        bootstrap.digest().unwrap().as_bytes(),
        old.digest().unwrap().as_bytes()
    );
    let bytes = norito::encode_canonical(&bootstrap).unwrap();
    assert_eq!(
        norito::decode_canonical::<PrivacyIronwoodPoolBootstrapV1>(&bytes).unwrap(),
        bootstrap
    );
    assert!(norito::decode_canonical::<PrivacyOrchardPoolBootstrapV1>(&bytes).is_err());
    let old_bytes = norito::encode_canonical(&old).unwrap();
    assert!(norito::decode_canonical::<PrivacyIronwoodPoolBootstrapV1>(&old_bytes).is_err());
    let json = norito::json::to_json(&bootstrap).unwrap();
    assert_eq!(
        norito::json::from_json::<PrivacyIronwoodPoolBootstrapV1>(&json).unwrap(),
        bootstrap
    );
    let mut invalid = bootstrap.clone();
    invalid.pool_id = PrivacyPoolIdV1::new([0; 32]);
    assert_eq!(
        invalid.validate(),
        Err(PrivacyIronwoodPoolBootstrapValidationErrorV1::ZeroPoolId)
    );
    invalid = bootstrap;
    invalid.public_balance_scope =
        AssetBalanceScope::Dataspace(iroha_model_base::topology::DataSpaceId::UNIVERSAL);
    assert_eq!(
        invalid.validate(),
        Err(PrivacyIronwoodPoolBootstrapValidationErrorV1::UniversalPublicBalanceScope)
    );
}
