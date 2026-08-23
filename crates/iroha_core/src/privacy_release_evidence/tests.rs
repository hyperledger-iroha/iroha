// Privacy-release evidence regression tests.
//
// Included by `privacy_release_evidence::tests` to preserve exact libtest names.
use super::*;
use crate::privacy_engines::vega::{
    build_signed_vega_privacy_action_with_rng_v1, sign_prepared_vega_privacy_action_v1,
};
use iroha_primitives::json::Json;
const RAYON_POOL_CHILD_MARKER_V1: &str = "IROHA_PRIVACY_RELEASE_RAYON_POOL_CHILD_V1";
fn compiled_profile_digest_mutations_v1() -> [fn(&mut CompiledPrivacyProfileV1); 5] {
    [
        |profile| profile.parameter_id.0[0] ^= 1,
        |profile| profile.parameter_digest.0[0] ^= 1,
        |profile| profile.verifier_digest.0[0] ^= 1,
        |profile| profile.statement_schema_digest.0[0] ^= 1,
        |profile| profile.engine_manifest_digest.0[0] ^= 1,
    ]
}
#[test]
fn zk_ace_is_unavailable_and_bootle_release_context_binds_every_profile_digest() {
    let zk_ace = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
    assert_eq!(
        compiled_privacy_profile_v1(zk_ace),
        Err(
            crate::privacy_profiles::CompiledPrivacyProfileErrorV1::EngineUnavailable {
                protocol_id: zk_ace,
            }
        )
    );

    let protocol_id = PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1;
    let profile = compiled_privacy_profile_v1(protocol_id).expect("compiled profile");
    let baseline = norito::encode_canonical(&release_statement_context_from_compiled_profile_v1(
        &profile,
        release_network_id_from_genesis_hash([0xa7; 32]),
        3,
        PrivacyTransactionIntentDigestV1::new([0x51; 32]),
    ))
    .expect("release context");
    for mutate in compiled_profile_digest_mutations_v1() {
        let mut changed = profile;
        mutate(&mut changed);
        let changed =
            norito::encode_canonical(&release_statement_context_from_compiled_profile_v1(
                &changed,
                release_network_id_from_genesis_hash([0xa7; 32]),
                3,
                PrivacyTransactionIntentDigestV1::new([0x51; 32]),
            ))
            .expect("changed release context");
        assert_ne!(
            changed,
            baseline,
            "{} release context omitted one compiled-profile digest",
            protocol_id.canonical_label()
        );
    }
}
#[test]
fn verange_release_transcript_binds_every_compiled_profile_digest() {
    let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
        .expect("compiled VeRange profile");
    let parameters =
        VeRangeParametersV1::for_profile(VeRangeBitLengthV1::Bits32).expect("VeRange parameters");
    let encode = |profile: &CompiledPrivacyProfileV1| {
        let binding = verange_binding_from_compiled_profile_v1(
            [0x61; 32],
            parameters.generator_digest(),
            profile,
        );
        let mut material = Vec::new();
        append_p256_binding_material_v1(&mut material, &binding);
        material
    };
    let baseline = encode(&profile);
    for mutate in compiled_profile_digest_mutations_v1() {
        let mut changed = profile;
        mutate(&mut changed);
        assert_ne!(
            encode(&changed),
            baseline,
            "VeRange release transcript omitted one compiled-profile digest"
        );
    }
}
#[test]
fn fcmp_release_statement_length_frames_every_compiled_profile_digest() {
    let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1)
        .expect("compiled FCMP++ profile");
    let encode = |profile: &CompiledPrivacyProfileV1| {
        let mut material = Vec::new();
        append_fcmp_compiled_profile_tuple_v1(&mut material, profile)
            .expect("FCMP++ profile tuple");
        material
    };
    let baseline = encode(&profile);
    let baseline_context =
        fcmp_release_context_hash_v1(&profile).expect("FCMP++ release context hash");
    let domain_length = usize::from(u16::from_be_bytes(
        baseline[..2].try_into().expect("domain length"),
    ));
    assert_eq!(
        &baseline[2..2 + domain_length],
        b"iroha.privacy.release.fcmp-plus-plus.compiled-profile-tuple.v1"
    );
    let count_offset = 2 + domain_length;
    assert_eq!(
        u16::from_be_bytes(
            baseline[count_offset..count_offset + 2]
                .try_into()
                .expect("field count")
        ),
        5
    );
    let mut offset = count_offset + 2;
    for expected in [
        profile.parameter_id.as_bytes().as_slice(),
        profile.parameter_digest.as_bytes().as_slice(),
        profile.verifier_digest.as_bytes().as_slice(),
        profile.statement_schema_digest.as_bytes().as_slice(),
        profile.engine_manifest_digest.as_bytes().as_slice(),
    ] {
        let length = usize::try_from(u64::from_be_bytes(
            baseline[offset..offset + 8]
                .try_into()
                .expect("field length"),
        ))
        .expect("field length fits usize");
        assert_eq!(length, 32);
        offset += 8;
        assert_eq!(&baseline[offset..offset + length], expected);
        offset += length;
    }
    assert_eq!(offset, baseline.len(), "profile tuple has trailing bytes");
    for mutate in compiled_profile_digest_mutations_v1() {
        let mut changed = profile;
        mutate(&mut changed);
        assert_ne!(
            encode(&changed),
            baseline,
            "FCMP++ release statement omitted one compiled-profile digest"
        );
        assert_ne!(
            fcmp_release_context_hash_v1(&changed).expect("changed FCMP++ release context hash"),
            baseline_context,
            "FCMP++ proof context omitted one compiled-profile digest"
        );
    }
}
#[test]
fn zk_ams_release_lineage_uses_distinct_single_action_transactions() {
    let admission =
        zk_ams_admission_transaction_context_v1().expect("admission transaction context");
    let provision =
        zk_ams_provision_transaction_context_v1().expect("provision transaction context");
    assert_eq!(ZK_AMS_RELEASE_ADMISSION_ACTION_INDEX_V1, 0);
    assert_eq!(ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1, 0);
    assert_eq!(admission.network_id, provision.network_id);
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
    let authoritative_network_id =
        release_network_id_from_genesis_hash(ZK_AMS_RELEASE_GENESIS_HASH_V1);
    let native_rejection = verify_zk_ams_release_production_envelope_v1(
        &statement,
        &[0x01],
        &authoritative_network_id,
        ZK_AMS_RELEASE_GENESIS_HASH_V1,
        ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1,
    );
    assert_eq!(
        native_rejection,
        Err(PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected),
        "a canonical one-action envelope must reach the native ZK-AMS verifier"
    );
    assert_eq!(
        require_zk_ams_release_production_native_rejection_v1(
            native_rejection,
            PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted,
        ),
        Ok(()),
    );
    let mut impossible_second_action = statement.clone();
    impossible_second_action.context.action_index = 1;
    let pre_native_rejection = verify_zk_ams_release_production_envelope_v1(
        &impossible_second_action,
        &[0x01],
        &authoritative_network_id,
        ZK_AMS_RELEASE_GENESIS_HASH_V1,
        ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1,
    );
    assert_eq!(
        pre_native_rejection,
        Err(PrivacyReleaseEvidenceErrorClassV1::ProductionEnvelopeRejected),
        "Taira's one-action transaction limit must reject before native verification"
    );
    assert_eq!(
        require_zk_ams_release_production_native_rejection_v1(
            pre_native_rejection,
            PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted,
        ),
        Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant),
        "a pre-native rejection cannot count as ZK-AMS corruption coverage"
    );
    assert_eq!(
        require_zk_ams_release_production_admission_rejection_v1(
            pre_native_rejection,
            PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted,
        ),
        Ok(()),
    );
    let oversized_len = usize::try_from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
        .expect("closed Taira proof-byte ceiling fits usize")
        + 1;
    let mut oversized = vec![0_u8; oversized_len];
    oversized[0] = 1;
    let oversized_rejection = verify_zk_ams_release_production_envelope_v1(
        &statement,
        &oversized,
        &authoritative_network_id,
        ZK_AMS_RELEASE_GENESIS_HASH_V1,
        ZK_AMS_RELEASE_PROVISION_ACTION_INDEX_V1,
    );
    assert_eq!(
        oversized_rejection,
        Err(PrivacyReleaseEvidenceErrorClassV1::ProductionEnvelopeRejected),
        "an oversized ZK-AMS proof must fail admission before native decoding"
    );
    assert_eq!(
        require_zk_ams_release_production_native_rejection_v1(
            oversized_rejection,
            PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted,
        ),
        Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant),
    );
    assert_eq!(
        require_zk_ams_release_production_admission_rejection_v1(
            oversized_rejection,
            PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted,
        ),
        Ok(()),
    );
}
#[test]
fn jindo_release_envelope_requires_the_production_native_dispatch() {
    let profile =
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0)
            .expect("compiled Jindo profile");
    let authoritative_network_id =
        release_network_id_from_genesis_hash(JINDO_RELEASE_GENESIS_HASH_V1);
    let polynomial_commitments = (1_i32..=4)
        .map(|coefficient| {
            let mut encoding =
                vec![0_u8; iroha_data_model::privacy::IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1];
            encoding[..4].copy_from_slice(&coefficient.to_le_bytes());
            iroha_data_model::privacy::PrivacyJindoLatticeCommitmentV1::new(encoding)
        })
        .collect();
    let statement = iroha_data_model::privacy::IrohaJindoPolynomialCommitmentStatementV1 {
        context: release_statement_context_from_compiled_profile_v1(
            &profile,
            authoritative_network_id,
            JINDO_RELEASE_ACTION_INDEX_V1,
            PrivacyTransactionIntentDigestV1::new([0x51; 32]),
        ),
        polynomial_commitments,
        evaluation_point: jindo_field_v1(13),
        claimed_evaluations: (17_u64..=20).map(jindo_field_v1).collect(),
    };
    let native_rejection = verify_jindo_release_production_envelope_v1(
        &statement,
        &[0x01],
        &authoritative_network_id,
        JINDO_RELEASE_GENESIS_HASH_V1,
        JINDO_RELEASE_ACTION_INDEX_V1,
    );
    assert_eq!(
        native_rejection,
        Err(PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected),
        "a canonical Jindo envelope with an invalid wire must reach the native verifier"
    );
    assert_eq!(
        require_jindo_release_production_native_rejection_v1(
            native_rejection,
            PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted,
        ),
        Ok(()),
    );
    let mut impossible_second_action = statement;
    impossible_second_action.context.action_index = 1;
    let pre_native_rejection = verify_jindo_release_production_envelope_v1(
        &impossible_second_action,
        &[0x01],
        &authoritative_network_id,
        JINDO_RELEASE_GENESIS_HASH_V1,
        JINDO_RELEASE_ACTION_INDEX_V1,
    );
    assert_eq!(
        pre_native_rejection,
        Err(PrivacyReleaseEvidenceErrorClassV1::ProductionEnvelopeRejected),
        "Taira's sole-action context must reject before native Jindo verification"
    );
    assert_eq!(
        require_jindo_release_production_native_rejection_v1(
            pre_native_rejection,
            PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted,
        ),
        Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant),
        "release evidence must not count a pre-native rejection as native corruption coverage"
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
        network_id: transaction.network_id,
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
        transaction.network_id,
        release_network_id_from_genesis_hash([0xa7; 32])
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
fn vega_release_envelope_requires_the_production_native_dispatch() {
    let fixture = vega_release_fixture_v1().expect("canonical Vega release fixture");
    let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0)
        .expect("compiled Vega profile");
    let authoritative_network_id = release_network_id_from_genesis_hash(fixture.genesis_hash);
    let input = fixture.public_input;
    let record = fixture.issuer_record;
    let mut statement = VegaExistingCredentialStatementV1 {
        context: release_statement_context_from_compiled_profile_v1(
            &profile,
            authoritative_network_id,
            VEGA_RELEASE_ACTION_INDEX_V1,
            PrivacyTransactionIntentDigestV1::new([0x27; 32]),
        ),
        issuer_id: record.issuer_id,
        issuer_record_epoch: record.record_epoch,
        issuer_record_digest: record.record_digest,
        document_type: record.document_type,
        namespace: record.namespace,
        digest_algorithm: record.digest_algorithm,
        issuer_authentication_algorithm: record.issuer_authentication_algorithm,
        device_authentication_algorithm: record.device_authentication_algorithm,
        issuer_public_key: record.issuer_public_key,
        device_authentication_digest:
            iroha_data_model::privacy::PrivacyVegaDeviceAuthenticationDigestV1::new([0; 32]),
        presentation_date: input.presentation_date,
        minimum_age_years: input.minimum_age_years,
        reader_challenge: input.reader_challenge,
        session_transcript_digest: input.session_transcript_digest,
    };
    refresh_vega_device_authentication_digest_v1(&mut statement, fixture.genesis_hash)
        .expect("canonical Vega device binding");
    let native_rejection = verify_vega_release_production_envelope_v1(
        &statement,
        Some(&record),
        &[0x01],
        &authoritative_network_id,
        fixture.genesis_hash,
        VEGA_RELEASE_ACTION_INDEX_V1,
        VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
    );
    assert_eq!(
        native_rejection,
        Err(PrivacyReleaseEvidenceErrorClassV1::NativeVerifierRejected),
        "a canonical Vega envelope with an invalid wire must reach the native verifier"
    );
    assert_eq!(
        require_vega_release_production_native_rejection_v1(
            native_rejection,
            PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted,
        ),
        Ok(()),
    );
    statement.context.action_index = 1;
    refresh_vega_device_authentication_digest_v1(&mut statement, fixture.genesis_hash)
        .expect("rebound impossible Vega action index");
    let pre_native_rejection = verify_vega_release_production_envelope_v1(
        &statement,
        Some(&record),
        &[0x01],
        &authoritative_network_id,
        fixture.genesis_hash,
        VEGA_RELEASE_ACTION_INDEX_V1,
        VEGA_RELEASE_TRUSTED_TIMESTAMP_MS_V1,
    );
    assert_eq!(
        pre_native_rejection,
        Err(PrivacyReleaseEvidenceErrorClassV1::ProductionEnvelopeRejected),
        "Taira's sole-action context must reject before native Vega verification"
    );
    assert_eq!(
        require_vega_release_production_native_rejection_v1(
            pre_native_rejection,
            PrivacyReleaseEvidenceErrorClassV1::ProofCorruptionAccepted,
        ),
        Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant),
        "release evidence must not count a pre-native rejection as Vega corruption coverage"
    );
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
        crate::privacy_engines::vega::VegaPrivacyActionEffectV1::ActionVerificationAndFinalityOnly
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
    let PrivacyStatementV1::VegaExistingCredentialZkV0(statement) = &submission.envelope.statement
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
    let mut changed_network = payload.clone();
    changed_network.domain = iroha_data_model::transaction::TransactionDomain::Network(
        release_network_id_from_genesis_hash([0xa8; 32]),
    );
    assert!(
        changed_network
            .validate_privacy_transaction_intent_binding_v1()
            .is_err(),
        "network mutation must invalidate the signed intent"
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
    changed_fee.fee_payment = FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(6_000_000));
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
    let binding = VegaMdlConsensusBindingV1::from_context(&statement.context, fixture.genesis_hash);
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
    let changed_binding =
        VegaMdlConsensusBindingV1::from_context(&changed_statement.context, fixture.genesis_hash);
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
    let signed = sign_prepared_vega_privacy_action_v1(prepared, transaction_key_pair.private_key())
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
            address_space_ceiling_bytes: 34_359_738_368,
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
    assert_eq!(
        profiles[0].address_space_ceiling_bytes,
        ZK_X509_PROVER_ADDRESS_SPACE_CEILING_BYTES_V1
    );
}
#[test]
fn privacy_release_rayon_pool_fresh_process_child_v1() {
    if std::env::var_os(RAYON_POOL_CHILD_MARKER_V1).is_none() {
        return;
    }
    assert_eq!(
        std::env::var("RUST_MIN_STACK").as_deref(),
        Ok("1048576"),
        "the child must exercise the release override under a hostile one-MiB default"
    );
    assert_eq!(PRIVACY_RELEASE_RAYON_THREAD_COUNT_V1, 4);
    assert_eq!(PRIVACY_RELEASE_STAGE_STACK_BYTES_V1, 8 * 1024 * 1024);
    initialize_privacy_release_rayon_pool_v1().expect("initialize exact release Rayon pool");
    let worker_stack_probes = rayon::broadcast(|_| {
        let stack_probe = [0xA5_u8; 2 * 1024 * 1024];
        assert_eq!(stack_probe[0], 0xA5);
        assert_eq!(stack_probe[stack_probe.len() - 1], 0xA5);
        std::hint::black_box(&stack_probe);
    });
    assert_eq!(
        worker_stack_probes.len(),
        usize::from(PRIVACY_RELEASE_RAYON_THREAD_COUNT_V1)
    );
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
        .env("RUST_MIN_STACK", "1048576")
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
fn resource_facts_are_frozen_for_every_exact12_stage() {
    for protocol_id in PrivacyProtocolIdV1::ALL {
        for case_kind in PrivacyReleaseCaseKindV1::ALL {
            let facts = privacy_release_resource_facts_v1(protocol_id, case_kind)
                .expect("every exact-12 stage has frozen resource facts");
            assert!(facts.validate());
            if case_kind == PrivacyReleaseCaseKindV1::MaximumShapeResource {
                assert_eq!(facts.primary_units, facts.primary_ceiling);
                assert_eq!(facts.secondary_units, facts.secondary_ceiling);
                assert_eq!(facts.relation_depth, facts.relation_depth_ceiling);
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
fn zk_x509_candidate_profile_and_resource_facts_are_total_before_capture() {
    let first = zk_x509_release_candidate_profile_material_v1()
        .expect("release-candidate profile material");
    let second =
        zk_x509_release_candidate_profile_material_v1().expect("deterministic profile material");
    assert_eq!(first, second);
    assert_eq!(
        first.protocol_id,
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V0
    );
    assert_ne!(first.parameter_id.0, [0; 32]);
    assert_ne!(first.parameter_digest.0, [0; 32]);
    assert_ne!(first.verifier_digest.0, [0; 32]);
    assert_ne!(first.statement_schema_digest.0, [0; 32]);
    assert_ne!(first.engine_manifest_digest.0, [0; 32]);
    for case_kind in PrivacyReleaseCaseKindV1::ALL {
        let resources = privacy_release_resource_facts_v1(first.protocol_id, case_kind)
            .expect("every X.509 release coordinate has frozen resource facts");
        assert!(resources.validate());
    }
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
fn zk_x509_artifact_uses_exact_encoded_x5s1_ceiling_below_outer_action_cap() {
    let protocol_id = PrivacyProtocolIdV1::IrohaZkX509StarkP256V0;
    let case_kind = PrivacyReleaseCaseKindV1::PositiveCanonicalEndToEnd;
    let exact_x5s1_ceiling = u64::from(ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1);
    assert_eq!(
        privacy_release_proof_artifact_ceiling_v1(protocol_id, case_kind, 0),
        Some(exact_x5s1_ceiling)
    );
    assert!(exact_x5s1_ceiling < PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1);
    let descriptor = privacy_release_protocol_descriptor_v1(protocol_id);
    assert!(descriptor.contains("proof-artifact-cap=8212538 exact X5S1 bytes"));
    assert!(descriptor.contains("outer-action-proof-cap=9437184 bytes"));
    let canonical_proof_bytes = vec![0x58, 0x35, 0x53, 0x31];
    let mut artifact = PrivacyReleaseProofArtifactEvidenceV1 {
        artifact_ordinal: 0,
        proof_sha256: sha256_v1(&canonical_proof_bytes),
        canonical_proof_bytes,
        proof_bytes_ceiling: exact_x5s1_ceiling,
    };
    assert!(validate_privacy_release_proof_artifacts_v1(
        protocol_id,
        case_kind,
        core::slice::from_ref(&artifact),
    ));
    for substituted_ceiling in [
        exact_x5s1_ceiling - 1,
        exact_x5s1_ceiling + 1,
        PRIVACY_RELEASE_MAX_PROOF_ARTIFACT_BYTES_V1,
    ] {
        artifact.proof_bytes_ceiling = substituted_ceiling;
        assert!(
            !validate_privacy_release_proof_artifacts_v1(
                protocol_id,
                case_kind,
                core::slice::from_ref(&artifact),
            ),
            "a lower, higher, or outer action ceiling cannot replace the exact X5S1 ceiling"
        );
    }
}
#[test]
fn zk_x509_public_evidence_binds_the_exact_validated_input_shape() {
    let context =
        crate::privacy_engines::zk_x509::relation::release_fixture::reference_statement_context_v1(
        );
    let canonical = build_zk_x509_release_fixture_v1(context.clone(), false)
        .expect("canonical X.509 release fixture");
    let maximum =
        build_zk_x509_release_fixture_v1(context, true).expect("maximum X.509 release fixture");
    canonical
        .resource_shape
        .validate_v1()
        .expect("canonical resource shape");
    maximum
        .resource_shape
        .validate_v1()
        .expect("maximum resource shape");
    assert_eq!(maximum.resource_shape.certificate_chain_depth, 3);
    assert_eq!(maximum.resource_shape.maximum_serial_bytes, 20);
    assert_eq!(
        maximum.resource_shape.disclosed_value_lengths,
        [2, 256, 256, 256]
    );
    assert_eq!(maximum.resource_shape.maximum_disclosed_value_bytes, 256);
    assert_eq!(maximum.resource_shape.ca_membership_index, 1);
    assert!(
        maximum
            .resource_shape
            .ca_membership_path_has_nonzero_sibling
    );
    let statement_material = b"canonical-statement".to_vec();
    let canonical_material = zk_x509_release_public_statement_material_v1(
        statement_material.clone(),
        canonical.resource_shape,
    )
    .expect("canonical shape material");
    let maximum_material = zk_x509_release_public_statement_material_v1(
        statement_material.clone(),
        maximum.resource_shape,
    )
    .expect("maximum shape material");
    assert_ne!(canonical_material, maximum_material);
    assert_ne!(sha256_v1(&canonical_material), sha256_v1(&maximum_material));
    assert!(maximum_material.starts_with(ZK_X509_RELEASE_PUBLIC_MATERIAL_DOMAIN_V1));
    let statement_len_offset = ZK_X509_RELEASE_PUBLIC_MATERIAL_DOMAIN_V1.len();
    assert_eq!(
        &maximum_material[statement_len_offset..statement_len_offset + size_of::<u64>()],
        &u64::try_from(statement_material.len())
            .expect("statement length fits u64")
            .to_be_bytes()
    );
    assert_eq!(
        &maximum_material[statement_len_offset + size_of::<u64>()
            ..statement_len_offset + size_of::<u64>() + statement_material.len()],
        statement_material.as_slice()
    );
    let shape_wire_bytes = size_of::<u8>()
        + ZK_X509_MAX_CHAIN_DEPTH_V1 * size_of::<u32>()
        + size_of::<u32>()
        + size_of::<u8>()
        + 4 * size_of::<u16>()
        + size_of::<u16>()
        + size_of::<u16>()
        + size_of::<u8>();
    let shape_wire = &maximum_material[maximum_material.len() - shape_wire_bytes..];
    assert_eq!(shape_wire.len(), 31);
    assert_eq!(shape_wire[0], 3);
    assert_eq!(shape_wire[17], 20);
    assert_eq!(&shape_wire[18..26], &[0, 2, 1, 0, 1, 0, 1, 0]);
    assert_eq!(&shape_wire[26..28], &256_u16.to_be_bytes());
    assert_eq!(&shape_wire[28..30], &1_u16.to_be_bytes());
    assert_eq!(shape_wire[30], 1);
    let mut invalid_shape = maximum.resource_shape;
    invalid_shape.crl_der_length = 0;
    assert_eq!(
        zk_x509_release_public_statement_material_v1(statement_material, invalid_shape,),
        Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)
    );
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
        proof_bytes_ceiling: privacy_release_proof_artifact_ceiling_v1(protocol_id, case_kind, 0)
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
fn vega_release_descriptor_is_derived_from_the_canonical_mc_constants() {
    let descriptor =
        privacy_release_protocol_descriptor_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0);
    let upstream_commit = super::vega::VEGA_RELEASE_MC_UPSTREAM_COMMIT_V1;
    let wire = super::vega::VEGA_RELEASE_MC_WIRE_DESCRIPTOR_V1;
    assert!(descriptor.contains(&format!(
        "canonical-profile=Microsoft-Vega_MC-Figure9@{upstream_commit}"
    )));
    assert!(descriptor.contains(&format!("wire={wire}")));
    assert!(descriptor.contains(&format!(
        "fixed-primary={} total application constraints",
        VEGA_RELEASE_MC_TOTAL_APP_CONSTRAINTS_V1
    )));
    assert!(descriptor.contains(&format!(
        "fixed-secondary={} maximum circuit variables",
        VEGA_RELEASE_MC_MAX_CIRCUIT_VARIABLES_V1
    )));
    assert!(descriptor.contains(&format!(
        "fixed-public-inputs={}",
        super::vega::VEGA_RELEASE_PUBLIC_INPUT_COUNT_V1
    )));
    assert!(descriptor.contains(&format!(
        "fixed-depth={} relaxed sumcheck rounds",
        VEGA_RELEASE_MC_RELAXED_SUMCHECK_ROUNDS_V1
    )));
    assert_eq!(
        super::vega::VEGA_RELEASE_PUBLIC_INPUT_COUNT_V1,
        VEGA_MDL_PUBLIC_INPUT_COUNT_V1
    );
    let manifest =
        core::str::from_utf8(iroha_zkp_halo2::vega::VEGA_MDL_COMPILED_PROFILE_MANIFEST_V1)
            .expect("Vega compiled profile manifest is ASCII");
    assert!(manifest.contains(&format!("upstream_commit={upstream_commit}")));
    assert!(manifest.contains(
        "vendor_manifest_sha256=539c54251c8853fa99673e71d777966a3e3e238e64028d47b3e683329023236f"
    ));
    assert!(manifest.contains("sha256_steps=birth:2,issuer:6,total:8"));
    assert!(manifest.contains("proof_wire=bincode-1.3.3-fixed-little-endian"));
    assert!(manifest.contains("envelope=IROVEGMC,version:1,context-keccak256:32"));
    assert!(manifest.contains("no-ambient-fallback"));
}
#[test]
fn jindo_release_descriptor_does_not_condition_individual_s35_challenges_on_units() {
    let descriptor = privacy_release_protocol_descriptor_v1(
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
    );
    assert!(descriptor.contains(
            "challenge=complete-uniform-S35-cardinality-4237250353474513005583342210963717757744597392123051014751100017656012472320"
        ));
    assert!(
        descriptor.contains("extraction=difference-invertibility-via-heuristic-well-spreadness")
    );
    assert!(descriptor.contains("split-challenge=uniform-nonzero-Fp-star"));
    assert!(!descriptor.contains("S35-unit"));
}

#[test]
fn zk_ace_release_stages_fail_closed_without_an_activatable_profile() {
    let protocol_id = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
    let descriptor = privacy_release_protocol_descriptor_v1(protocol_id);
    assert!(descriptor.contains("activation=disabled"));
    assert!(descriptor.contains("commitment-binding-ceiling=32-bits"));
    for case_kind in PrivacyReleaseCaseKindV1::ALL {
        assert_eq!(
            run_privacy_release_stage_v1(protocol_id, case_kind),
            Err(PrivacyReleaseEvidenceErrorV1 {
                protocol_id,
                case_kind,
                class: PrivacyReleaseEvidenceErrorClassV1::ProtocolUnavailable,
            })
        );
    }
}

#[test]
#[ignore = "operator-only native proof construction for the complete Bootle/Lantern release stage"]
fn bootle_lantern_release_stage_exercises_one_shot_issuance_and_wire_rejection() {
    let protocol_id = PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1;
    let case_kind = PrivacyReleaseCaseKindV1::ProofCorruptionAndTruncation;
    let evidence = run_privacy_release_stage_v1(protocol_id, case_kind)
        .expect("Bootle/Lantern corruption-and-truncation release stage");
    assert_eq!(evidence.protocol_id, protocol_id);
    assert_eq!(evidence.case_kind, case_kind);
    assert_eq!(
        evidence.failure_class,
        PrivacyReleaseFailureClassV1::CanonicalWireCorruptionAndTruncationRejected
    );
    assert_eq!(evidence.proof_artifacts.len(), 1);
    assert_eq!(
        evidence.proof_artifacts[0].canonical_proof_bytes.len(),
        BOOTLE_PROOF_BYTES_V1
    );
    assert_eq!(
        evidence.proof_artifacts[0].proof_bytes_ceiling,
        u64::try_from(BOOTLE_PROOF_BYTES_V1).expect("fixed ILN1 length fits u64")
    );
    assert!(validate_privacy_release_proof_artifacts_v1(
        protocol_id,
        case_kind,
        &evidence.proof_artifacts,
    ));
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
