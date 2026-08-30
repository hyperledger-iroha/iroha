mod kagemusha_eligibility_payment_envelope_tests {
    use super::*;
    use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};

    const ISSUED_AT_MS: u64 = 1_800_000_000_000;

    struct EnvelopeFixture {
        request: KagemushaRecipientPaymentRequestV2,
        payment: KagemushaRecursiveSpendPeerPaymentV4,
        payment_v4_norito: Vec<u8>,
        credential: OfflineDeviceEligibilityCredentialV1,
        credential_issuer: PublicKey,
        policy_view: OfflineDeviceAttestationPolicyViewV1,
        wallet_device_key: SigningKey,
        assertion_key: SigningKey,
    }

    fn p256_signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes((&[seed; 32]).into()).expect("non-zero P-256 test scalar")
    }

    fn p256_public_key(key: &SigningKey) -> KagemushaDevicePublicKeyV2 {
        KagemushaDevicePublicKeyV2::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .expect("canonical uncompressed P-256 test key")
    }

    fn p256_sign(key: &SigningKey, message: &[u8]) -> KagemushaDeviceSignatureV2 {
        let signature: P256Signature = key.sign(message);
        let signature = signature.normalize_s().unwrap_or(signature);
        KagemushaDeviceSignatureV2::from_raw_bytes(signature.to_bytes().as_slice())
            .expect("canonical low-S P-256 test signature")
    }

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("deterministic account key")
                .public_key()
                .clone(),
        )
    }

    fn recipient_request(
        statement: &KagemushaRecursiveSpendPublicStatementV4,
        receiver_key: &SigningKey,
    ) -> KagemushaRecipientPaymentRequestV2 {
        let receiver_public_key = p256_public_key(receiver_key);
        let payload = KagemushaRecipientPaymentRequestSigningPayloadV2 {
            network_id: statement.network_id,
            asset: statement.asset.clone(),
            amount: statement.current_note.amount,
            recipient: account(0x31),
            recipient_key_reference: kagemusha_receiver_key_reference_v2(&receiver_public_key)
                .expect("receiver key reference"),
            receiver_device_id: "eligibility-envelope-receiver".to_owned(),
            receiver_public_key,
            request_id: [0x32; 32],
            issued_at_ms: ISSUED_AT_MS,
            expires_at_ms: ISSUED_AT_MS + 60_000,
            recipient_output: statement.current_note.clone(),
            sender_output_prover_material: vec![0x33],
        };
        let signature = p256_sign(
            receiver_key,
            &payload.signing_bytes().expect("request signing bytes"),
        );
        KagemushaRecipientPaymentRequestV2::from_signed_payload(payload, signature)
            .expect("signed recipient request")
    }

    fn peer_bundle(
        mut statement: KagemushaRecursiveSpendPublicStatementV4,
        request: &KagemushaRecipientPaymentRequestV2,
    ) -> KagemushaRecursiveSpendBundleV4 {
        let operation_id = [0x41; 32];
        let binding_digest = [0x42; 32];
        statement.branch_claims = statement
            .topup_anchor_refs
            .iter()
            .map(|anchor| {
                KagemushaRecursiveSpendBranchClaimV2::root(
                    kagemusha_recursive_spend_lineage_root_v2(anchor.anchor_digest)
                        .expect("lineage root"),
                )
                .expect("root branch claim")
                .child(KagemushaRecursiveSpendBranchV2::Recipient, binding_digest)
                .expect("recipient branch claim")
            })
            .collect();
        statement.transition = Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(
            KagemushaRecursiveSpendPeerSplitTransitionV4 {
                binding_digest,
                branch: KagemushaRecursiveSpendBranchV2::Recipient,
                recipient_request_digest: request.digest().expect("recipient request digest"),
                operation_id,
                parent_max_proof_step_count: 1,
                parent_max_peer_hop_count: 0,
            },
        ));
        statement
            .validate_public_binding()
            .expect("recipient statement binding");
        let public_statement_digest = statement.digest().expect("statement digest");
        let verifier_key_id = statement.verifier_key_id.clone();
        let mut state_limbs = vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5];
        state_limbs[0] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5;
        let proof_envelope = KagemushaPastaCycleProofEnvelopeV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
            step_eq_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4.to_owned(),
            step_ep_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4.to_owned(),
            artifact_generation: statement.artifact_binding.generation.clone(),
            manifest_sha256: statement.artifact_binding.manifest_sha256,
            step_eq_parameter_generation: "eligibility-envelope-eq-params".to_owned(),
            step_ep_parameter_generation: "eligibility-envelope-ep-params".to_owned(),
            step_eq_circuit_params_sha256: [0x43; 32],
            step_ep_circuit_params_sha256: [0x44; 32],
            step_eq_verifier_key_sha256: [0x45; 32],
            step_ep_verifier_key_sha256: [0x46; 32],
            state_boundary: KagemushaRecursiveSpendStateBoundaryV5::new(state_limbs)
                .expect("state boundary"),
            proof: ProofBox::new(
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.into(),
                vec![0x47],
            ),
        };
        let mut operation_limbs = [0; KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4];
        operation_limbs[0] = 1;
        let bundle = KagemushaRecursiveSpendBundleV4 {
            statement,
            operation: KagemushaRecursiveSpendOperationVectorV4 {
                limbs: operation_limbs,
            },
            recursive_proof: KagemushaRecursiveSpendProofV4 {
                verifier_key_id,
                public_statement_digest,
                proof_envelope,
            },
        };
        bundle
            .validate_public_binding()
            .expect("recipient bundle binding");
        bundle
    }

    fn membership_witness(
        statement: &KagemushaRecursiveSpendPublicStatementV4,
    ) -> KagemushaNoteMembershipWitnessV2 {
        fn directions(leaf_index: u32) -> Vec<u8> {
            (0..KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2)
                .map(|level| {
                    u8::try_from((leaf_index >> level) & 1)
                        .expect("one direction bit always fits u8")
                })
                .collect()
        }
        let leaf_index = statement.next_zero_leaf_index - 1;
        KagemushaNoteMembershipWitnessV2 {
            leaf_index,
            input_path: KagemushaConfidentialMerklePathV2 {
                siblings: vec![[0x51; 32]; KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2],
                directions: directions(leaf_index),
                root: statement.final_root,
            },
            dummy_input_path: KagemushaConfidentialMerklePathV2 {
                siblings: vec![[0x52; 32]; KAGEMUSHA_CONFIDENTIAL_TREE_DEPTH_V2],
                directions: directions(statement.next_zero_leaf_index),
                root: statement.final_root,
            },
        }
    }

    fn eligibility_credential(
        network_id: NetworkId,
        wallet_device_key: &SigningKey,
        assertion_key: &SigningKey,
    ) -> (
        OfflineDeviceEligibilityCredentialV1,
        PublicKey,
        OfflineDeviceAttestationPolicyViewV1,
    ) {
        let issuer = KeyPair::try_from_seed(vec![0x61; 32], Algorithm::Ed25519)
            .expect("deterministic credential issuer");
        let response_date_ms = ISSUED_AT_MS - 1_000;
        let policy = OfflineDeviceAttestationPolicy {
            version: OFFLINE_DEVICE_ATTESTATION_POLICY_VERSION_V2,
            policy_epoch: 9,
            trusted_roots: Vec::new(),
            revoked_certificate_tbs_sha256: Vec::new(),
            ios_apps: Vec::new(),
            android_apps: Vec::new(),
            android_status_snapshot: Some(OfflineAndroidAttestationStatusSnapshotV1 {
                version: OFFLINE_ANDROID_ATTESTATION_STATUS_SNAPSHOT_VERSION_V1,
                payload_sha256: [0x64; 32],
                response_date_ms,
                last_modified_ms: Some(response_date_ms - 1_000),
                cache_max_age_seconds: 3_600,
                non_valid_serials: Vec::new(),
            }),
            android_vulnerability_rules: Vec::new(),
            require_ios_app_policy: false,
            require_android_app_policy: false,
        };
        let finality_evidence_bytes = b"eligibility envelope finality evidence".to_vec();
        let finality = OfflineDevicePolicyFinalityBindingV1 {
            version: OFFLINE_DEVICE_POLICY_FINALITY_BINDING_VERSION_V1,
            network_id,
            finalized_block_height: 51,
            finalized_block_hash: Hash::new(b"eligibility envelope finalized block"),
            finalized_block_timestamp_ms: response_date_ms,
            finality_evidence_hash: Hash::new(&finality_evidence_bytes),
        };
        let policy_view = OfflineDeviceAttestationPolicyViewV1::new_v1(
            &policy,
            response_date_ms + 3_600_000,
            finality_evidence_bytes,
            finality,
        )
        .expect("finalized eligibility policy view");
        let payload = OfflineDeviceEligibilityCredentialPayloadV1 {
            version: OFFLINE_DEVICE_ELIGIBILITY_CREDENTIAL_VERSION_V1,
            network_id,
            account_id: account(0x62),
            device_id: "eligibility-envelope-sender".to_owned(),
            attestation_key_id: "independent-assertion-key".to_owned(),
            device_public_key: p256_public_key(wallet_device_key),
            assertion_public_key: assertion_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes()
                .to_vec(),
            registration_hash: [0x63; 32],
            eligibility: OfflineDeviceEligibilityOutcomeV1::Eligible,
            policy_epoch: policy_view.policy_epoch,
            policy_hash: policy_view.policy_hash,
            policy_finality: finality,
            policy_freshness_deadline_ms: policy_view.freshness_deadline_ms,
            issued_at_ms: ISSUED_AT_MS,
            expires_at_ms: ISSUED_AT_MS + 60_000,
        };
        let credential = OfflineDeviceEligibilityCredentialV1::sign_v1(
            payload,
            issuer.public_key().clone(),
            issuer.private_key(),
        )
        .expect("issuer-signed eligibility credential");
        (credential, issuer.public_key().clone(), policy_view)
    }

    fn envelope_fixture() -> EnvelopeFixture {
        let provenance_fixture = fixture_with_seeds(&[0x35]);
        let request = recipient_request(&provenance_fixture.statement, &p256_signing_key(0x71));
        let bundle = peer_bundle(provenance_fixture.statement, &request);
        let payment = KagemushaRecursiveSpendPeerPaymentV4 {
            recipient_membership_witness: membership_witness(&bundle.statement),
            recipient_bundle: bundle,
            topup_provenance: provenance_fixture.provenance,
        };
        payment
            .validate_public_binding()
            .expect("complete ABI-21/V4 peer payment");
        let payment_v4_norito =
            norito::encode_canonical(&payment).expect("canonical ABI-21/V4 payment bytes");
        let wallet_device_key = p256_signing_key(0x72);
        let assertion_key = p256_signing_key(0x73);
        let (credential, credential_issuer, policy_view) = eligibility_credential(
            payment.recipient_bundle.statement.network_id,
            &wallet_device_key,
            &assertion_key,
        );
        EnvelopeFixture {
            request,
            payment,
            payment_v4_norito,
            credential,
            credential_issuer,
            policy_view,
            wallet_device_key,
            assertion_key,
        }
    }

    fn signed_envelope(fixture: &EnvelopeFixture) -> KagemushaEligibilityPaymentEnvelopeV1 {
        let payload = KagemushaEligibilityPaymentEnvelopePayloadV1::prepare_v1(
            fixture.payment_v4_norito.clone(),
            fixture.credential.clone(),
            &fixture.request,
        )
        .expect("prepare eligibility envelope");
        let signature = p256_sign(
            &fixture.wallet_device_key,
            &payload.signing_bytes_v1().expect("envelope signing bytes"),
        );
        KagemushaEligibilityPaymentEnvelopeV1::finalize_v1(payload, signature)
            .expect("finalize eligibility envelope with the credential device key")
    }

    #[test]
    fn eligibility_envelope_uses_device_key_and_preserves_v4_payment_and_replay_identity() {
        let fixture = envelope_fixture();
        assert_ne!(
            fixture
                .credential
                .payload
                .device_public_key
                .as_sec1_bytes()
                .as_slice(),
            fixture.credential.payload.assertion_public_key.as_slice(),
            "the wallet device key and platform assertion key are independent",
        );
        let payload = KagemushaEligibilityPaymentEnvelopePayloadV1::prepare_v1(
            fixture.payment_v4_norito.clone(),
            fixture.credential.clone(),
            &fixture.request,
        )
        .expect("prepare eligibility envelope");
        assert_eq!(
            payload.sender_device_public_key,
            fixture.credential.payload.device_public_key,
        );
        let assertion_signature = p256_sign(
            &fixture.assertion_key,
            &payload.signing_bytes_v1().expect("envelope signing bytes"),
        );
        assert!(
            KagemushaEligibilityPaymentEnvelopeV1::finalize_v1(
                payload.clone(),
                assertion_signature,
            )
            .is_err(),
            "the independently credential-bound assertion key cannot sign the one-use envelope",
        );
        let mut assertion_substitution = fixture.credential.clone();
        assertion_substitution.payload.assertion_public_key = fixture
            .wallet_device_key
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes()
            .to_vec();
        assert!(
            KagemushaEligibilityPaymentEnvelopePayloadV1::prepare_v1(
                fixture.payment_v4_norito.clone(),
                assertion_substitution,
                &fixture.request,
            )
            .is_err(),
            "the issuer signature independently binds the assertion key",
        );

        let envelope = signed_envelope(&fixture);
        envelope
            .validate_static_binding_v1()
            .expect("device-key-signed eligibility envelope verifies");
        assert_eq!(
            envelope.payment_v4_norito(),
            fixture.payment_v4_norito.as_slice(),
        );
        assert_eq!(
            norito::decode_canonical::<KagemushaRecursiveSpendPeerPaymentV4>(
                envelope.payment_v4_norito(),
            )
            .expect("decode unchanged inner ABI-21/V4 payment"),
            fixture.payment,
        );
        let envelope_bytes =
            norito::encode_canonical(&envelope).expect("canonical eligibility envelope");
        let retransmission: KagemushaEligibilityPaymentEnvelopeV1 =
            norito::decode_canonical(&envelope_bytes).expect("decode exact retransmission");
        assert_eq!(
            retransmission.one_use_key_sha256_v1().expect("one-use key"),
            envelope
                .one_use_key_sha256_v1()
                .expect("original one-use key"),
        );
        assert_eq!(
            retransmission.identity_sha256_v1().expect("identity"),
            envelope.identity_sha256_v1().expect("original identity"),
        );
        assert_eq!(
            retransmission.payment_v4_norito(),
            fixture.payment_v4_norito.as_slice(),
        );
    }

    #[test]
    fn eligibility_envelope_rejects_operation_request_and_network_mutations() {
        let fixture = envelope_fixture();
        let envelope = signed_envelope(&fixture);
        let mut mutations = Vec::new();

        let mut changed_operation = envelope.clone();
        changed_operation.payload.operation_id = [0x81; 32];
        mutations.push(changed_operation);

        let mut changed_request = envelope.clone();
        changed_request.payload.recipient_request_digest = [0x82; 32];
        mutations.push(changed_request);

        let mut changed_network = envelope;
        changed_network.payload.network_id = network_id(0x83);
        mutations.push(changed_network);

        for mutation in mutations {
            assert!(
                mutation.validate_static_binding_v1().is_err(),
                "operation, request, and network coordinates are jointly bound",
            );
        }
    }

    #[test]
    fn first_delivery_requires_the_current_policy_but_static_retransmission_survives_rotation() {
        let fixture = envelope_fixture();
        let envelope = signed_envelope(&fixture);
        assert_eq!(
            envelope
                .validate_for_first_delivery_v1(
                    &fixture.request,
                    &fixture.credential_issuer,
                    &fixture.policy_view,
                    ISSUED_AT_MS,
                )
                .expect("fresh first delivery"),
            fixture.payment,
        );

        let identity = envelope.identity_sha256_v1().expect("envelope identity");
        let one_use_key = envelope.one_use_key_sha256_v1().expect("one-use key");
        assert!(
            envelope
                .validate_for_first_delivery_v1(
                    &fixture.request,
                    &fixture.credential_issuer,
                    &fixture.policy_view,
                    fixture.request.expires_at_ms(),
                )
                .is_err(),
            "a first delivery fails at the receiver request's exclusive deadline",
        );

        let mut rotated_policy = fixture
            .policy_view
            .validated_policy_v1(ISSUED_AT_MS)
            .expect("decode current policy");
        rotated_policy.policy_epoch += 1;
        let rotated_view = OfflineDeviceAttestationPolicyViewV1::new_v1(
            &rotated_policy,
            fixture.policy_view.freshness_deadline_ms,
            fixture.policy_view.finality_evidence_bytes.clone(),
            fixture.policy_view.finality,
        )
        .expect("construct a later finalized policy view");
        assert!(
            envelope
                .validate_for_first_delivery_v1(
                    &fixture.request,
                    &fixture.credential_issuer,
                    &rotated_view,
                    ISSUED_AT_MS,
                )
                .is_err(),
            "a credential for an older policy cannot admit new value",
        );

        envelope
            .validate_static_binding_v1()
            .expect("the persisted envelope remains statically valid after policy rotation");
        assert_eq!(
            envelope.identity_sha256_v1().expect("stable identity"),
            identity,
        );
        assert_eq!(
            envelope
                .one_use_key_sha256_v1()
                .expect("stable one-use key"),
            one_use_key,
        );
    }

    #[test]
    fn receiver_request_ttl_is_exactly_fifteen_minutes_and_exclusive() {
        let fixture = envelope_fixture();
        let receiver_key = p256_signing_key(0x71);
        let mut payload = fixture.request.signing_payload();
        payload.expires_at_ms = payload.issued_at_ms + KAGEMUSHA_PEER_RECEIVE_REQUEST_MAX_TTL_MS_V1;
        let signature = p256_sign(
            &receiver_key,
            &payload
                .signing_bytes()
                .expect("maximum request signing bytes"),
        );
        let request =
            KagemushaRecipientPaymentRequestV2::from_signed_payload(payload.clone(), signature)
                .expect("the exact fifteen-minute receive-request lifetime is accepted");
        request
            .validate_at(request.expires_at_ms() - 1)
            .expect("the final live millisecond is accepted");
        assert!(request.validate_at(request.expires_at_ms()).is_err());

        payload.expires_at_ms += 1;
        assert!(payload.signing_bytes().is_err());
    }
}
