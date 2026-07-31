//! X.509 production-dispatch and release-candidate verifier tests.

use super::*;

#[test]
fn zk_x509_dispatch_is_state_first_context_bound_and_fail_closed() {
    let (statement, authoritative_state) = zk_x509_dispatch_fixture_for_test();
    let (_, activation) = active_zk_x509_profile();
    let genesis_hash = [0x91; 32];
    let public =
        ZkX509CredentialPublicBindingV1::from_consensus_context_v1(&statement, genesis_hash)
            .expect("canonical X.509 consensus binding");
    let synthetic_x5s1 = PrivacyProofBytesV1::new(
        encode_zk_x509_credential_envelope_v1(public, b"X5M1main-aggregate", b"X5C1compact-ca")
            .expect("canonical X.509 credential envelope"),
    );
    let malformed_proof = PrivacyProofBytesV1::new(Vec::new());

    assert!(matches!(
        verify_zk_x509_certificate_v1(
            &statement,
            &malformed_proof,
            &zk_x509_context(&statement, &activation, None, false, genesis_hash),
        ),
        Err(PrivacyVerificationErrorV1::ZkX509State(detail))
            if detail.code == PrivacyZkX509StateFailureCodeV1::MissingTrustedState
    ));

    let mut wrong_record = statement.clone();
    wrong_record.trust_anchor_record_digest =
        iroha_data_model::privacy::PrivacyZkX509TrustAnchorRecordDigestV1::new([0xB1; 32]);
    let mut wrong_root = statement.clone();
    wrong_root.ca_membership_root = PrivacyRootV1::new([0xB2; 32]);
    let mut wrong_predicate = statement.clone();
    wrong_predicate.key_usage.content_commitment =
        (!wrong_predicate.key_usage.content_commitment.is_required()).into();
    for (label, candidate) in [
        ("record substitution", wrong_record),
        ("root substitution", wrong_root),
        ("predicate substitution", wrong_predicate),
    ] {
        assert!(
            matches!(
                verify_zk_x509_certificate_v1(
                    &candidate,
                    &malformed_proof,
                    &zk_x509_context(
                        &candidate,
                        &activation,
                        Some(&authoritative_state),
                        false,
                        genesis_hash,
                    ),
                ),
                Err(PrivacyVerificationErrorV1::ZkX509State(detail))
                    if detail.code
                        == PrivacyZkX509StateFailureCodeV1::AuthoritativeStateMismatch
            ),
            "{label} did not fail at the authoritative-state boundary"
        );
    }

    let (_, successor_crl_state) = zk_x509_dispatch_fixture_with_successor_crl_v1();
    assert_eq!(
        successor_crl_state.crl_record().record_epoch,
        authoritative_state
            .crl_record()
            .record_epoch
            .checked_add(1)
            .expect("fixture epoch has a successor")
    );
    assert_ne!(
        successor_crl_state.crl_record().record_digest,
        statement.crl_record_digest
    );
    assert!(matches!(
        verify_zk_x509_certificate_v1(
            &statement,
            &malformed_proof,
            &zk_x509_context(
                &statement,
                &activation,
                Some(&successor_crl_state),
                false,
                genesis_hash,
            ),
        ),
        Err(PrivacyVerificationErrorV1::ZkX509State(detail))
            if detail.code == PrivacyZkX509StateFailureCodeV1::AuthoritativeStateMismatch
    ));

    assert!(matches!(
        verify_zk_x509_certificate_v1(
            &statement,
            &malformed_proof,
            &zk_x509_context(
                &statement,
                &activation,
                Some(&authoritative_state),
                true,
                genesis_hash,
            ),
        ),
        Err(PrivacyVerificationErrorV1::ZkX509State(detail))
            if detail.code == PrivacyZkX509StateFailureCodeV1::DuplicateCertificateNullifier
    ));

    assert!(matches!(
        verify_zk_x509_certificate_v1(
            &statement,
            &malformed_proof,
            &zk_x509_context(
                &statement,
                &activation,
                Some(&authoritative_state),
                false,
                genesis_hash,
            ),
        ),
        Err(PrivacyVerificationErrorV1::NativeZkX509(detail))
            if detail.source
                == ZkX509EngineErrorV1::CredentialProof(
                    ZkX509CredentialProofErrorV1::MalformedEnvelope
                )
    ));

    assert!(matches!(
        verify_zk_x509_certificate_v1(
            &statement,
            &synthetic_x5s1,
            &zk_x509_context(
                &statement,
                &activation,
                Some(&authoritative_state),
                false,
                genesis_hash,
            ),
        ),
        Err(PrivacyVerificationErrorV1::NativeZkX509(detail))
            if detail.source
                == ZkX509EngineErrorV1::CredentialProof(ZkX509CredentialProofErrorV1::MainProof)
    ));

    let mut wrong_intent = statement.clone();
    wrong_intent.context.transaction_intent_digest =
        PrivacyTransactionIntentDigestV1::new([0xC1; 32]);
    let mut wrong_profile = statement.clone();
    wrong_profile.context.parameter_digest = PrivacyParameterDigestV1::new([0xC2; 32]);
    for (label, candidate, candidate_genesis) in [
        ("transaction intent", wrong_intent, genesis_hash),
        ("governed profile", wrong_profile, genesis_hash),
        ("committed genesis", statement.clone(), [0x92; 32]),
    ] {
        assert!(
            matches!(
                verify_zk_x509_certificate_v1(
                    &candidate,
                    &synthetic_x5s1,
                    &zk_x509_context(
                        &candidate,
                        &activation,
                        Some(&authoritative_state),
                        false,
                        candidate_genesis,
                    ),
                ),
                Err(PrivacyVerificationErrorV1::NativeZkX509(detail))
                    if detail.source
                        == ZkX509EngineErrorV1::CredentialProof(
                            ZkX509CredentialProofErrorV1::PublicBindingMismatch
                        )
            ),
            "{label} substitution was not rejected by X5S1 public binding"
        );
    }
}

#[cfg(feature = "privacy-release-evidence")]
#[test]
fn zk_x509_candidate_verifier_does_not_relax_consensus_availability() {
    use crate::privacy_profiles::CompiledPrivacyProfileErrorV1;

    let (profile, activation) = active_zk_x509_profile();
    let (statement, authoritative_state) = zk_x509_dispatch_fixture_for_test();
    let genesis_hash = [0x91; 32];
    let public =
        ZkX509CredentialPublicBindingV1::from_consensus_context_v1(&statement, genesis_hash)
            .expect("canonical X.509 consensus binding");
    let synthetic_x5s1 =
        encode_zk_x509_credential_envelope_v1(public, b"X5M1main-aggregate", b"X5C1compact-ca")
            .expect("canonical synthetic X5S1");
    let typed_statement = PrivacyStatementV1::IrohaZkX509StarkP256V0(statement.clone());
    let statement_digest = typed_statement.digest().expect("statement digest");
    let envelope = PrivacyProofEnvelopeV1 {
        protocol_id: profile.protocol_id,
        proof_system_id: profile.proof_system_id,
        engine_id: profile.engine_id,
        parameter_id: profile.parameter_id,
        parameter_digest: profile.parameter_digest,
        verifier_digest: profile.verifier_digest,
        statement_schema_digest: profile.statement_schema_digest,
        engine_manifest_digest: profile.engine_manifest_digest,
        statement_digest,
        statement: typed_statement,
        proof: PrivacyProofV1::IrohaZkX509StarkP256V0(PrivacyProofBytesV1::new(synthetic_x5s1)),
    };

    let normal = verify_privacy_envelope_v1(
        &envelope,
        zk_x509_context(
            &statement,
            &activation,
            Some(&authoritative_state),
            false,
            genesis_hash,
        ),
    );
    if compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkX509StarkP256V0).is_ok() {
        assert!(matches!(
            normal,
            Err(PrivacyVerificationErrorV1::NativeZkX509(detail))
                if detail.source
                    == ZkX509EngineErrorV1::CredentialProof(
                        ZkX509CredentialProofErrorV1::MainProof
                    )
        ));
    } else {
        assert!(matches!(
            normal,
            Err(PrivacyVerificationErrorV1::CompiledActivation(detail))
                if detail.source
                    == CompiledPrivacyProfileValidationErrorV1::Profile(
                        CompiledPrivacyProfileErrorV1::EngineUnavailable {
                            protocol_id: PrivacyProtocolIdV1::IrohaZkX509StarkP256V0
                        }
                    )
        ));
    }

    assert!(matches!(
        verify_zk_x509_release_candidate_envelope_v1(
            &envelope,
            zk_x509_context(
                &statement,
                &activation,
                Some(&authoritative_state),
                false,
                genesis_hash,
            ),
        ),
        Err(PrivacyVerificationErrorV1::NativeZkX509(detail))
            if detail.source
                == ZkX509EngineErrorV1::CredentialProof(ZkX509CredentialProofErrorV1::MainProof)
    ));
}
