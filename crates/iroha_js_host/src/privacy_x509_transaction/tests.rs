//! Original-model byte and signature controls; structural proof bytes never establish proof validity.
use super::*;
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{block::BlockHeader, privacy::*, transaction::FeePaymentIntent};
use std::time::Duration;

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"js-x509-original",
    )))
}

// Deliberately structural public inputs, with no credential, witness, valid proof or State.
fn request(authority: AccountId) -> SubmitPrivacyZkX509CertificateV1 {
    SubmitPrivacyZkX509CertificateV1::new(
        IrohaZkX509StarkP256StatementV1 {
            context: PrivacyStatementContextV1 {
                network_id: network(),
                action_index: 0,
                transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0; 32]),
                parameter_id: PrivacyParameterIdV1::new([1; 32]),
                parameter_digest: PrivacyParameterDigestV1::new([2; 32]),
                verifier_digest: PrivacyVerifierDigestV1::new([3; 32]),
                statement_schema_digest: PrivacyStatementSchemaDigestV1::new([4; 32]),
                engine_manifest_digest: PrivacyEngineManifestDigestV1::new([5; 32]),
            },
            trust_anchor_id: PrivacyIssuerIdV1::new([6; 32]),
            certificate_policy_id: PrivacyPolicyIdV1::new([7; 32]),
            trust_anchor_record_digest: PrivacyZkX509TrustAnchorRecordDigestV1::new([8; 32]),
            trust_anchor_record_epoch: 1,
            certificate_policy_record_digest: PrivacyZkX509CertificatePolicyRecordDigestV1::new(
                [9; 32],
            ),
            certificate_policy_record_epoch: 1,
            crl_record_digest: PrivacyZkX509CrlRecordDigestV1::new([10; 32]),
            crl_record_epoch: 1,
            subject_public_key_digest: PrivacyCertificateKeyDigestV1::new([11; 32]),
            ca_membership_root: PrivacyRootV1::new([12; 32]),
            ca_membership_root_epoch: 1,
            key_usage: PrivacyX509KeyUsageV1 {
                digital_signature: PrivacyX509KeyUsageRequirementV1::new(true),
                content_commitment: PrivacyX509KeyUsageRequirementV1::new(false),
                key_encipherment: PrivacyX509KeyUsageRequirementV1::new(false),
                key_agreement: PrivacyX509KeyUsageRequirementV1::new(false),
            },
            extended_key_usages: vec![PrivacyX509ExtendedKeyUsageV1::WalletIdentity],
            disclosed_attributes: Vec::new(),
            presentation_not_before_unix_seconds: 1_400,
            presentation_not_after_unix_seconds: 1_600,
            wallet_account: authority,
            wallet_challenge: PrivacyChallengeV1::new([13; 32]),
            certificate_nullifier: PrivacyNullifierV1::new([14; 32]),
        },
        ActionAttachmentRefV1 {
            action_index: 0,
            slot_index: 0,
        },
    )
}

fn original_payload(request: &SubmitPrivacyZkX509CertificateV1, time: u64) -> TransactionPayload {
    let mut builder = TransactionBuilder::new(
        network(),
        request.statement.wallet_account.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([request.clone()]);
    builder.set_creation_time(Duration::from_millis(time));
    builder.set_ttl(Duration::from_millis(60_000));
    builder.into_payload().unwrap()
}

fn manifest() -> PrivacyExact12CapabilityManifestV1 {
    let catalog = super::super::privacy_compiled_profile_catalog().unwrap();
    PrivacyCapabilitySnapshotV1 {
        version: PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1,
        network_id: network(),
        committed_height: 3,
        consensus_policy: PrivacyConsensusPolicyV1::taira_default(),
        protocols: catalog
            .protocols
            .into_iter()
            .map(|row| PrivacyCapabilityRowV1 {
                protocol_id: row.protocol_id,
                compiled_profile: row.compiled_profile,
                policy: None,
            })
            .collect(),
    }
    .exact12_capability_manifest_v1()
    .unwrap()
}

#[test]
fn x509_request_decoder_rejects_supplied_intent_slot_ordinal_and_noncanonical_archive() {
    let key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let request = request(AccountId::new(key.public_key().clone()));
    let archive = norito::encode_canonical(&request).unwrap();
    assert_eq!(decode_request(&archive).unwrap(), request);
    for changed in 0..4 {
        let mut request = request.clone();
        match changed {
            0 => {
                request.statement.context.transaction_intent_digest =
                    PrivacyTransactionIntentDigestV1::new([1; 32])
            }
            1 => request.proof_ref.slot_index = 1,
            2 => request.proof_ref.action_index = 1,
            _ => request.statement.context.action_index = 1,
        }
        assert!(decode_request(&norito::encode_canonical(&request).unwrap()).is_err());
    }
    let mut trailing = archive;
    trailing.push(0);
    assert!(decode_request(&trailing).is_err());
    assert!(decode_request(&[]).is_err());
    assert!(
        decode_request(&vec![
            0;
            SubmitPrivacyZkX509CertificateV1::canonical_frame_byte_limit_v1()
                + 1
        ])
        .is_err()
    );
}

#[test]
fn x509_model_plan_preserves_original_context_and_binds_only_one_proof_occurrence() {
    let key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let request = request(AccountId::new(key.public_key().clone()));
    let original = original_payload(&request, 17);
    let limits = PrivacyConsensusLimitsV1::taira_default();
    let projection = project(&original, &request, &limits).unwrap();
    let changed = project(&original_payload(&request, 18), &request, &limits).unwrap();
    assert_ne!(projection.ordered_intent, changed.ordered_intent);
    assert_ne!(projection.action_digest, changed.action_digest);
    let mut payload = declared_payload(&original, &request, vec![0xab; 128]).unwrap();
    payload.validate_signed_attachments_v1().unwrap();
    let attachments = payload.attachments.as_ref().unwrap();
    assert_eq!(attachments.actions().len(), 1);
    assert_eq!(attachments.occurrences().len(), 1);
    assert_eq!(attachments.occurrences()[0].bytes, vec![0xab; 128]);
    assert_eq!(
        payload.ordered_intent_digest_v1().unwrap().as_bytes(),
        &projection.ordered_intent
    );
    assert_eq!(
        attachments.actions()[0].statement_digest.as_bytes(),
        &projection.action_digest
    );
    let statement: PrivacyStatementV1 =
        norito::decode_canonical(&projection.statement_norito).unwrap();
    assert_eq!(
        statement,
        PrivacyStatementV1::IrohaZkX509StarkP256V1(projection.statement)
    );
    payload.attachments = None;
    assert_eq!(
        payload, original,
        "proof insertion preserves every independent field"
    );
}

#[test]
fn x509_structural_completed_payload_signature_is_exact_and_rejects_other_key() {
    let key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let other = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let request = request(AccountId::new(key.public_key().clone()));
    // These bytes are intentionally not a native proof. Only the transaction signature is tested.
    let payload =
        declared_payload(&original_payload(&request, 17), &request, vec![0xab; 128]).unwrap();
    payload.validate_signed_attachments_v1().unwrap();
    let builder = TransactionBuilder::from_payload(payload.clone()).unwrap();
    assert!(
        super::super::sign_js_transaction(
            builder.clone(),
            other.private_key(),
            "X509 negative key"
        )
        .is_err()
    );
    let signed =
        super::super::sign_js_transaction(builder, key.private_key(), "X509 structural signature")
            .unwrap();
    signed.verify_signature().unwrap();
    let wire = super::super::encode_canonical_signed_transaction_v1(&signed).unwrap();
    let decoded = super::super::decode_canonical_signed_transaction_v1(&wire).unwrap();
    assert_eq!(decoded.payload(), &payload);
    let missing = declared_payload(&original_payload(&request, 17), &request, Vec::new()).unwrap();
    assert!(
        missing.validate_signed_attachments_v1().is_err(),
        "planning placeholders are never signable originals"
    );
}

#[test]
fn x509_public_native_prepare_and_sign_preserve_actual_incomplete_security_refusal() {
    let key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let request = request(AccountId::new(key.public_key().clone()));
    let original = original_payload(&request, 17);
    let manifest = manifest();
    let owner = JsPrivacyX509PreparedDirectV1 {
        projection: project(
            &original,
            &request,
            &PrivacyConsensusLimitsV1::taira_default(),
        )
        .unwrap(),
        original,
        request: request.clone(),
        manifest: manifest.clone(),
    };
    let wanted = compiled_privacy_profile_v1(PROTOCOL).unwrap_err();
    assert!(matches!(wanted, iroha_core_privacy::privacy_profiles::CompiledPrivacyProfileErrorV1::JointSecurityReductionIncomplete { .. }));
    let input = JsPrivacyX509DirectInputV1 {
        manifest: manifest.canonical_bytes().unwrap().into(),
        network_id: network().as_bytes().to_vec().into(),
        request_norito: norito::encode_canonical(&request).unwrap().into(),
        authority: request
            .statement
            .wallet_account
            .to_i105_for_discriminant(369)
            .unwrap(),
        fee_payment_json: norito::json::to_json(&FeePaymentIntent::authority(Vec::new(), None))
            .unwrap(),
        metadata_json: None,
        creation_time_ms: 17,
        ttl_ms: 60_000,
        nonce: None,
    };
    let error = privacy_prepare_x509_direct_transaction_v1(input)
        .err()
        .expect("current real construction refuses");
    assert!(error.reason.contains(&wanted.to_string()));
    let error = owner
        .sign(vec![0xab; 128].into(), vec![1; 32].into())
        .err()
        .expect("no test-created owner can bypass real admission");
    assert!(error.reason.contains(&wanted.to_string()));
    let mut returned = owner.materialized_statement_norito();
    returned.as_mut().fill(0);
    assert_ne!(
        returned.as_ref(),
        owner.materialized_statement_norito().as_ref()
    );
    assert_eq!(
        owner.action_statement_digest().as_ref(),
        &owner.projection.action_digest
    );
    assert_eq!(
        owner.ordered_intent_digest().as_ref(),
        &owner.projection.ordered_intent
    );
    assert_eq!(
        owner.action_statement_norito().as_ref(),
        &owner.projection.action_statement
    );
    let original_bytes = owner.original_payload_bytes().unwrap();
    assert_eq!(
        original_bytes.as_ref(),
        TransactionBuilder::from_payload(owner.original.clone())
            .unwrap()
            .encode_payload()
    );
}

#[test]
fn x509_native_admission_checks_original_network_and_authority_before_selection() {
    let key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let request = request(AccountId::new(key.public_key().clone()));
    let manifest = manifest();
    let other = AccountId::new(
        KeyPair::random_with_algorithm(Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    assert!(
        require_admission(&manifest, &request, &network(), &other)
            .err()
            .unwrap()
            .reason
            .contains("authority")
    );
    let foreign = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::new(b"foreign"),
    ));
    assert!(
        require_admission(
            &manifest,
            &request,
            &foreign,
            &request.statement.wallet_account
        )
        .err()
        .unwrap()
        .reason
        .contains("network")
    );
}

// Independently framed structural container with no valid MAIN or CA proof. This test-only
// encoder owns no production codec and its success means framing/header binding only.
fn structural_container(statement: &IrohaZkX509StarkP256StatementV1) -> Vec<u8> {
    let digest = PrivacyStatementV1::IrohaZkX509StarkP256V1(statement.clone())
        .digest()
        .unwrap();
    let genesis = statement.context.network_id.as_bytes();
    let domain = b"iroha.zk-x509.credential-consensus-context.v1";
    let mut context = b"iroha.zk-x509.sha256.frame.v1".to_vec();
    context.extend_from_slice(&u16::try_from(domain.len()).unwrap().to_be_bytes());
    context.extend_from_slice(domain);
    context.extend_from_slice(&2_u16.to_be_bytes());
    for field in [digest.as_bytes().as_slice(), genesis.as_slice()] {
        context.extend_from_slice(&u64::try_from(field.len()).unwrap().to_be_bytes());
        context.extend_from_slice(field);
    }
    let mut encoded = b"X5S1".to_vec();
    encoded.extend_from_slice(&1_u16.to_be_bytes());
    encoded.extend_from_slice(&2_u16.to_be_bytes());
    encoded.extend_from_slice(&[0x5a; 32]);
    encoded.extend_from_slice(&iroha_crypto::sha256(context));
    encoded.extend_from_slice(statement.ca_membership_root.as_bytes());
    let root_spki_channel = 30 + 2 * u32::try_from(statement.disclosed_attributes.len()).unwrap();
    encoded.extend_from_slice(&root_spki_channel.to_be_bytes());
    encoded.extend_from_slice(&[0; 132 * 32]);
    for (kind, magic) in [(1_u16, b"X5M1"), (2_u16, b"X5C1")] {
        encoded.extend_from_slice(&kind.to_be_bytes());
        encoded.extend_from_slice(&0_u16.to_be_bytes());
        encoded.extend_from_slice(&4_u32.to_be_bytes());
        encoded.extend_from_slice(magic);
    }
    encoded
}

#[test]
fn x509_native_container_boundary_rejects_malformed_and_substituted_originals() {
    use iroha_core_privacy::privacy_engines::zk_x509::credential_stark::{
        decode_zk_x509_credential_envelope_v1, encode_zk_x509_credential_envelope_v1,
    };
    let key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let request = request(AccountId::new(key.public_key().clone()));
    let projection = project(
        &original_payload(&request, 17),
        &request,
        &PrivacyConsensusLimitsV1::taira_default(),
    )
    .unwrap();
    let statement = &projection.statement;
    let proof = structural_container(statement);
    // Positive control tests only the same framing/header boundary called by sign().
    // Inner records are four-byte magic strings, so this is never a valid credential proof.
    require_proof_container(statement, &network(), &proof).unwrap();
    let decoded = decode_zk_x509_credential_envelope_v1(&proof).unwrap();
    assert_eq!(
        encode_zk_x509_credential_envelope_v1(
            decoded.proof_instance,
            decoded.public,
            decoded.main_aggregate,
            decoded.ca_subproof,
            &[0; 132 * 32],
        )
        .unwrap(),
        proof
    );
    let malformed = |bytes: &[u8]| {
        assert!(
            require_proof_container(statement, &network(), bytes)
                .unwrap_err()
                .reason
                .contains("container is malformed")
        );
    };
    malformed(&[]);
    malformed(&[0xab; 128]);
    for length in [4, 40, 108, proof.len() - 1] {
        malformed(&proof[..length]);
    }
    let mut trailing = proof.clone();
    trailing.push(0);
    malformed(&trailing);
    for offset in [0, 5, 7, 108 + 132 * 32 + 1, proof.len() - 1] {
        let mut changed = proof.clone();
        changed[offset] ^= 1;
        malformed(&changed);
    }
    let mut noncanonical_opening = proof.clone();
    noncanonical_opening[108..116].fill(0xff);
    malformed(&noncanonical_opening);
    for offset in [40, 72, 107] {
        let mut changed = proof.clone();
        changed[offset] ^= 1;
        assert!(
            require_proof_container(statement, &network(), &changed)
                .unwrap_err()
                .reason
                .contains("public binding does not match")
        );
    }
    let mut changed_statement = statement.clone();
    changed_statement.wallet_challenge = PrivacyChallengeV1::new([0x77; 32]);
    assert!(
        require_proof_container(&changed_statement, &network(), &proof)
            .unwrap_err()
            .reason
            .contains("public binding does not match")
    );
    let foreign = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::new(b"foreign-container"),
    ));
    assert!(
        require_proof_container(statement, &foreign, &proof)
            .unwrap_err()
            .reason
            .contains("statement or genesis binding is invalid")
    );
}
