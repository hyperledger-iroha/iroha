#[cfg(test)]
mod device_authority_p256_tests {
    use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};
    use super::*;
    use crate::domain::DomainId;
    const P256_ORDER: [u8; 32] = [
        0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84, 0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63,
        0x25, 0x51,
    ];
    const P256_HALF_ORDER: [u8; 32] = [
        0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42, 0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31,
        0x92, 0xa8,
    ];
    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes((&[seed; 32]).into()).expect("non-zero P-256 test scalar")
    }
    fn device_public_key(key: &SigningKey) -> KagemushaDevicePublicKeyV2 {
        KagemushaDevicePublicKeyV2::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .expect("canonical uncompressed test key")
    }
    fn sign(key: &SigningKey, message: &[u8]) -> KagemushaDeviceSignatureV2 {
        let signature: P256Signature = key.sign(message);
        let signature = signature.normalize_s().unwrap_or(signature);
        KagemushaDeviceSignatureV2::from_raw_bytes(signature.to_bytes().as_slice())
            .expect("canonical low-S test signature")
    }
    fn scalar_pair(r: [u8; 32], s: [u8; 32]) -> [u8; 64] {
        let mut raw = [0_u8; 64];
        raw[..32].copy_from_slice(&r);
        raw[32..].copy_from_slice(&s);
        raw
    }
    fn one() -> [u8; 32] {
        let mut value = [0_u8; 32];
        value[31] = 1;
        value
    }
    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("deterministic account key")
                .public_key()
                .clone(),
        )
    }
    fn asset(name: &str) -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("offline", "universal").expect("test domain"),
            name.parse().expect("test asset name"),
        )
    }
    fn placeholder_signature() -> KagemushaDeviceSignatureV2 {
        KagemushaDeviceSignatureV2::from_raw_bytes(&scalar_pair(one(), one()))
            .expect("valid low-S placeholder")
    }
    fn authorization(
        assertion_key: &SigningKey,
        ios_authenticator_data: Option<Vec<u8>>,
    ) -> KagemushaRequestAuthorizationV2 {
        let hardware_assertion = ios_authenticator_data.map_or_else(
            || {
                KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(
                    KagemushaAndroidKeyMintHardwareAssertionV1 {
                        signature: placeholder_signature(),
                    },
                )
            },
            |authenticator_data| {
                KagemushaOnlineHardwareAssertionV1::IosAppAttest(
                    KagemushaIosAppAttestHardwareAssertionV1 {
                        authenticator_data,
                        signature: placeholder_signature(),
                    },
                )
            },
        );
        let mut authorization = KagemushaRequestAuthorizationV2 {
            authority: account(21),
            device_id: "hardware-device-21".to_owned(),
            asset_definition_id: asset("cash"),
            operation_id: [0x21; 32],
            issued_at_ms: 1_800_000_000_000,
            expires_at_ms: 1_800_000_030_000,
            nonce: [0x22; 32],
            payload_digest: [0x23; 32],
            registration_hash: [0x24; 32],
            hardware_assertion,
        };
        let signing_bytes = authorization
            .signing_bytes()
            .expect("authorization signing bytes");
        let signed_message = match &authorization.hardware_assertion {
            KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(_) => signing_bytes,
            KagemushaOnlineHardwareAssertionV1::IosAppAttest(assertion) => [
                assertion.authenticator_data.as_slice(),
                signing_bytes.as_slice(),
            ]
            .concat(),
        };
        authorization.set_hardware_signature(sign(assertion_key, &signed_message));
        authorization
    }
    fn recipient_payment_request(
        receiver_key: &SigningKey,
        issued_at_ms: u64,
        expires_at_ms: u64,
    ) -> KagemushaRecipientPaymentRequestV2 {
        let network_id = kagemusha_test_network_id("kagemusha-request-boundary");
        let asset = asset("cash");
        let amount = KagemushaScaledAmountV2::new(500, 2).expect("test amount");
        let receiver_public_key = device_public_key(receiver_key);
        let payload = KagemushaRecipientPaymentRequestSigningPayloadV2 {
            network_id,
            asset: asset.clone(),
            amount,
            recipient: account(51),
            recipient_key_reference: kagemusha_receiver_key_reference_v2(&receiver_public_key)
                .expect("receiver key reference"),
            receiver_device_id: "receiver-device-51".to_owned(),
            receiver_public_key,
            request_id: [0x51; 32],
            issued_at_ms,
            expires_at_ms,
            recipient_output: KagemushaSpendableNoteDescriptorV2 {
                network_id,
                asset,
                note_commitment: [0x52; 32],
                spend_nullifier: [0x53; 32],
                amount,
            },
            sender_output_prover_material: vec![0x54],
        };
        let signature = sign(
            receiver_key,
            &payload.signing_bytes().expect("request signing bytes"),
        );
        KagemushaRecipientPaymentRequestV2::from_signed_payload(payload, signature)
            .expect("signed recipient request")
    }
    fn recipient_payment_bundle(
        request: &KagemushaRecipientPaymentRequestV2,
    ) -> KagemushaRecursiveSpendBundleV4 {
        let anchor = KagemushaRecursiveSpendTopUpAnchorRefV2 {
            topup_operation_id: [0x55; 32],
            anchor_digest: [0x56; 32],
        };
        let lineage_root =
            kagemusha_recursive_spend_lineage_root_v2(anchor.anchor_digest).expect("lineage root");
        let artifact_binding = KagemushaRecursiveSpendArtifactBindingV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            generation: "acknowledgement-expiry-test".to_owned(),
            manifest_sha256: [0x57; 32],
        };
        let verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
            KagemushaPastaCycleParityV1::StepEq,
            artifact_binding.manifest_sha256,
        );
        let recipient_request_digest = request.digest().expect("request digest");
        let operation_id = [0x58; 32];
        let statement = KagemushaRecursiveSpendPublicStatementV4 {
            network_id: *request.network_id(),
            asset: request.asset().clone(),
            asset_scale: request.amount().scale,
            final_root: [0x59; 32],
            next_zero_leaf_index: 1,
            topup_anchor_refs: vec![anchor],
            proof_step_count: 2,
            peer_hop_count: 1,
            current_note: request.recipient_output().clone(),
            branch_claims: vec![
                KagemushaRecursiveSpendBranchClaimV2::root(lineage_root)
                    .expect("root branch claim"),
            ],
            transition: Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(
                KagemushaRecursiveSpendPeerSplitTransitionV4 {
                    binding_digest: [0x5a; 32],
                    branch: KagemushaRecursiveSpendBranchV2::Recipient,
                    recipient_request_digest,
                    operation_id,
                    parent_max_proof_step_count: 1,
                    parent_max_peer_hop_count: 0,
                },
            )),
            artifact_binding: artifact_binding.clone(),
            verifier_key_id: verifier_key_id.clone(),
        };
        let public_statement_digest = statement.digest().expect("statement digest");
        let mut state_limbs = vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V5];
        state_limbs[0] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V5;
        let proof_envelope = KagemushaPastaCycleProofEnvelopeV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
            proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
            transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
            step_eq_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4.to_owned(),
            step_ep_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4.to_owned(),
            artifact_generation: artifact_binding.generation,
            manifest_sha256: artifact_binding.manifest_sha256,
            step_eq_parameter_generation: "ack-expiry-eq-params".to_owned(),
            step_ep_parameter_generation: "ack-expiry-ep-params".to_owned(),
            step_eq_circuit_params_sha256: [0x5b; 32],
            step_ep_circuit_params_sha256: [0x5c; 32],
            step_eq_verifier_key_sha256: [0x5d; 32],
            step_ep_verifier_key_sha256: [0x5e; 32],
            state_boundary: KagemushaRecursiveSpendStateBoundaryV5::new(state_limbs)
                .expect("state boundary"),
            proof: ProofBox::new(
                KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.into(),
                vec![0x5f],
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
    fn receiver_acknowledgement(
        receiver_key: &SigningKey,
        request: &KagemushaRecipientPaymentRequestV2,
        bundle: &KagemushaRecursiveSpendBundleV4,
        accepted_at_ms: u64,
    ) -> KagemushaReceiverAcknowledgementV2 {
        let KagemushaRecursiveSpendTransitionV4::PeerSplit(transition) = bundle
            .statement
            .transition
            .as_ref()
            .expect("peer-split transition")
        else {
            panic!("recipient bundle must carry a peer-split transition")
        };
        let payload = KagemushaReceiverAcknowledgementPayloadV2 {
            operation_id: transition.operation_id,
            recipient_request_digest: request.digest().expect("request digest"),
            payment_bundle_digest: bundle.digest().expect("bundle digest"),
            recipient_commitment: request.recipient_output().note_commitment,
            accepted_at_ms,
            receiver_device_id: request.receiver_device_id().to_owned(),
            receiver_key_reference: kagemusha_receiver_key_reference_v2(
                request.receiver_public_key(),
            )
            .expect("receiver key reference"),
            receiver_public_key: *request.receiver_public_key(),
        };
        let signature = sign(
            receiver_key,
            &payload
                .signing_bytes()
                .expect("acknowledgement signing bytes"),
        );
        KagemushaReceiverAcknowledgementV2 { payload, signature }
    }
    #[test]
    fn device_public_key_accepts_only_canonical_uncompressed_p256() {
        let key = signing_key(7);
        let canonical = key.verifying_key().to_encoded_point(false);
        let parsed =
            KagemushaDevicePublicKeyV2::from_sec1_bytes(canonical.as_bytes()).expect("valid key");
        parsed.validate().expect("decoded key revalidates");
        assert_eq!(parsed.as_sec1_bytes().as_slice(), canonical.as_bytes());
        for malformed in [
            Vec::new(),
            canonical.as_bytes()[..64].to_vec(),
            [canonical.as_bytes(), &[0_u8]].concat(),
            key.verifying_key()
                .to_encoded_point(true)
                .as_bytes()
                .to_vec(),
            vec![0_u8; 65],
        ] {
            assert!(
                KagemushaDevicePublicKeyV2::from_sec1_bytes(&malformed).is_err(),
                "malformed key unexpectedly accepted: {} bytes",
                malformed.len()
            );
        }
        let mut wrong_prefix = canonical.as_bytes().to_vec();
        wrong_prefix[0] = 0x06;
        assert!(KagemushaDevicePublicKeyV2::from_sec1_bytes(&wrong_prefix).is_err());
        let mut off_curve = canonical.as_bytes().to_vec();
        off_curve[64] ^= 0x02;
        assert!(KagemushaDevicePublicKeyV2::from_sec1_bytes(&off_curve).is_err());
        assert_eq!(
            norito::codec::Encode::encode(&parsed),
            canonical.as_bytes(),
            "the key newtype must be wire-transparent"
        );
        // Invalid points are rejected by serialization and deserialization,
        // not merely by higher-level request validation.
        let malformed = KagemushaDevicePublicKeyV2([0_u8; 65]);
        assert!(norito::encode_canonical(&malformed).is_err());
        let mut malformed_bytes = &[0_u8; 65][..];
        assert!(
            <KagemushaDevicePublicKeyV2 as norito::codec::Decode>::decode(&mut malformed_bytes)
                .is_err()
        );
    }
    #[test]
    fn device_signature_rejects_bad_width_scalars_and_high_s() {
        for malformed in [vec![], vec![0_u8; 63], vec![0_u8; 65]] {
            assert!(KagemushaDeviceSignatureV2::from_raw_bytes(&malformed).is_err());
        }
        let der = P256Signature::from_slice(&scalar_pair(one(), one()))
            .expect("valid scalar pair")
            .to_der();
        assert!(KagemushaDeviceSignatureV2::from_raw_bytes(der.as_bytes()).is_err());
        assert!(KagemushaDeviceSignatureV2::from_raw_bytes(&scalar_pair([0; 32], one())).is_err());
        assert!(KagemushaDeviceSignatureV2::from_raw_bytes(&scalar_pair(one(), [0; 32])).is_err());
        assert!(
            KagemushaDeviceSignatureV2::from_raw_bytes(&scalar_pair(P256_ORDER, one())).is_err()
        );
        assert!(
            KagemushaDeviceSignatureV2::from_raw_bytes(&scalar_pair(one(), P256_ORDER)).is_err()
        );
        let mut high_s = P256_HALF_ORDER;
        high_s[31] += 1;
        let high_s = scalar_pair(one(), high_s);
        assert!(KagemushaDeviceSignatureV2::from_raw_bytes(&high_s).is_err());
        let malformed = KagemushaDeviceSignatureV2(high_s);
        assert!(norito::encode_canonical(&malformed).is_err());
        let mut malformed_bytes = malformed.0.as_slice();
        assert!(
            <KagemushaDeviceSignatureV2 as norito::codec::Decode>::decode(&mut malformed_bytes)
                .is_err()
        );
        let valid = KagemushaDeviceSignatureV2::from_raw_bytes(&scalar_pair(one(), one()))
            .expect("valid low-S signature");
        assert_eq!(
            norito::codec::Encode::encode(&valid),
            scalar_pair(one(), one()),
            "the signature newtype must be wire-transparent"
        );
    }
    #[test]
    fn ecdsa_sha256_verification_is_key_and_message_bound() {
        let key = signing_key(9);
        let wrong_key = signing_key(10);
        let public_key = device_public_key(&key);
        let message = b"kagemusha fixed P-256 authority";
        let signature = sign(&key, message);
        signature
            .verify(&public_key, message)
            .expect("valid signature");
        assert!(
            signature
                .verify(&public_key, b"substituted message")
                .is_err()
        );
        assert!(
            signature
                .verify(&device_public_key(&wrong_key), message)
                .is_err()
        );
    }
    #[test]
    fn signed_requests_acknowledgements_and_archives_ignore_ambient_norito_layout() {
        let issued_at_ms = 1_800_000_000_000;
        let expires_at_ms = issued_at_ms + 30_000;
        let receiver_key = signing_key(13);
        let authorization = authorization(&signing_key(14), None);
        let request = recipient_payment_request(&receiver_key, issued_at_ms, expires_at_ms);
        let bundle = recipient_payment_bundle(&request);
        let acknowledgement =
            receiver_acknowledgement(&receiver_key, &request, &bundle, issued_at_ms + 1);
        let expected_authorization_signing_bytes = authorization
            .signing_bytes()
            .expect("canonical authorization signing bytes");
        let expected_request_signing_bytes = request
            .signing_payload()
            .signing_bytes()
            .expect("canonical recipient-request signing bytes");
        let expected_request_digest = request
            .digest()
            .expect("canonical recipient-request digest");
        let expected_bundle_digest = bundle.digest().expect("canonical bundle digest");
        let expected_acknowledgement_signing_bytes = acknowledgement
            .payload
            .signing_bytes()
            .expect("canonical acknowledgement signing bytes");
        let expected_acknowledgement_digest = acknowledgement
            .digest()
            .expect("canonical acknowledgement digest");
        let expected_acknowledgement_archive = acknowledgement
            .canonical_archive_for_payment_v4(&request, &bundle)
            .expect("canonical acknowledgement archive");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        authorization
            .validate_for_payload(authorization.payload_digest)
            .expect("authorization remains valid under alternate ambient layout");
        request
            .validate_public_binding()
            .expect("recipient request remains valid under alternate ambient layout");
        bundle
            .validate_public_binding()
            .expect("bundle remains valid under alternate ambient layout");
        acknowledgement
            .validate_for_payment_v4(&request, &bundle)
            .expect("acknowledgement remains valid under alternate ambient layout");
        assert_eq!(
            authorization
                .signing_bytes()
                .expect("authorization signing bytes under alternate ambient layout"),
            expected_authorization_signing_bytes
        );
        assert_eq!(
            request
                .signing_payload()
                .signing_bytes()
                .expect("recipient-request signing bytes under alternate ambient layout"),
            expected_request_signing_bytes
        );
        assert_eq!(
            request
                .digest()
                .expect("recipient-request digest under alternate ambient layout"),
            expected_request_digest
        );
        assert_eq!(
            bundle
                .digest()
                .expect("bundle digest under alternate ambient layout"),
            expected_bundle_digest
        );
        assert_eq!(
            acknowledgement
                .payload
                .signing_bytes()
                .expect("acknowledgement signing bytes under alternate ambient layout"),
            expected_acknowledgement_signing_bytes
        );
        assert_eq!(
            acknowledgement
                .digest()
                .expect("acknowledgement digest under alternate ambient layout"),
            expected_acknowledgement_digest
        );
        assert_eq!(
            acknowledgement
                .canonical_archive_for_payment_v4(&request, &bundle)
                .expect("acknowledgement archive under alternate ambient layout"),
            expected_acknowledgement_archive
        );
    }
    #[test]
    fn redeem_result_rejects_alternate_layout_and_compressed_expansion_archives() {
        let receiver_request =
            recipient_payment_request(&signing_key(15), 1_800_000_000_000, 1_800_000_030_000);
        let bundle = recipient_payment_bundle(&receiver_request);
        let authorization = authorization(&signing_key(16), None);
        let amount = receiver_request.amount();
        let public_inputs = KagemushaUnshieldPublicInputsBindingV2 {
            input_commitment_0: bundle.statement.current_note.note_commitment,
            input_commitment_1: [0; 32],
            nullifier_0: bundle.statement.current_note.spend_nullifier,
            nullifier_1: [0; 32],
            change_output_commitment: [0; 32],
            root: bundle.statement.final_root,
            public_amount: kagemusha_confidential_amount_encoding_v2(amount.atomic_units),
            asset_tag: [0x61; 32],
            network_tag: [0x62; 32],
        };
        let backend = bundle.recursive_proof.verifier_key_id.backend.clone();
        let request = KagemushaRecursiveSpendRedeemRequestV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            bundle: bundle.clone(),
            recipient: authorization.authority.clone(),
            amount,
            redeem_proof: ProofAttachment::new_ref(
                backend.clone(),
                ProofBox::new(backend, vec![0x63]),
                bundle.recursive_proof.verifier_key_id.clone(),
            ),
            redemption: KagemushaRecursiveSpendRedemptionIntentV4 {
                network_id: bundle.statement.network_id,
                asset: bundle.statement.asset.clone(),
                input_note: bundle.statement.current_note.clone(),
                parent_branch_claims: bundle.statement.branch_claims.clone(),
                parent_topup_anchor_refs: bundle.statement.topup_anchor_refs.clone(),
                parent_proof_step_count: bundle.statement.proof_step_count,
                parent_peer_hop_count: bundle.statement.peer_hop_count,
                parent_bundle_digest: bundle.digest().expect("canonical parent bundle digest"),
                input_root: bundle.statement.final_root,
                recipient: authorization.authority.clone(),
                public_amount: amount,
                change_output: None,
                change_artifact_binding: None,
                unshield_public_inputs: public_inputs,
                unshield_public_inputs_digest: public_inputs
                    .digest()
                    .expect("canonical unshield public-input digest"),
                operation_id: authorization.operation_id,
            },
            offline_change: None,
            block_height: 1,
            operation_id: authorization.operation_id,
            authorization,
        };
        let canonical_request_archive =
            norito::encode_canonical(&request).expect("canonical redeem-request archive");
        let alternate_request_archive = {
            let alternate_flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            to_bytes(&request).expect("alternate-layout redeem-request archive")
        };
        assert_ne!(alternate_request_archive, canonical_request_archive);
        assert_eq!(
            norito::decode_from_bytes::<KagemushaRecursiveSpendRedeemRequestV4>(
                &alternate_request_archive,
            )
            .expect("alternate-layout redeem request remains structurally decodable"),
            request
        );
        let result = KagemushaRecursiveSpendRedeemResultV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            redeem_request_archive: alternate_request_archive,
            offline_change_bundle: None,
            offline_change_membership_witness: None,
            offline_change_topup_provenance: None,
            operation_id: request.operation_id,
        };
        assert!(matches!(
            result.validate_public_binding(),
            Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_result.v4.request_archive",
            })
        ));
        const NORITO_COMPRESSION_OFFSET: usize = 4 + 1 + 1 + 16;
        const NORITO_UNCOMPRESSED_LENGTH_OFFSET: usize = NORITO_COMPRESSION_OFFSET + 1;
        let mut compressed_expansion_archive = canonical_request_archive;
        compressed_expansion_archive[NORITO_COMPRESSION_OFFSET] = norito::Compression::Zstd as u8;
        compressed_expansion_archive[NORITO_UNCOMPRESSED_LENGTH_OFFSET
            ..NORITO_UNCOMPRESSED_LENGTH_OFFSET + std::mem::size_of::<u64>()]
            .copy_from_slice(
                &u64::try_from(KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4)
                    .expect("redeem ceiling fits u64")
                    .to_le_bytes(),
            );
        let compressed_expansion_result = KagemushaRecursiveSpendRedeemResultV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
            redeem_request_archive: compressed_expansion_archive,
            offline_change_bundle: None,
            offline_change_membership_witness: None,
            offline_change_topup_provenance: None,
            operation_id: request.operation_id,
        };
        assert!(matches!(
            compressed_expansion_result.validate_public_binding(),
            Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_result.v4.request_archive",
            })
        ));
        let unshield_backend: iroha_schema::Ident = KAGEMUSHA_CONFIDENTIAL_PROOF_BACKEND.into();
        let mut semantic_unshield = ProofAttachment::new_ref(
            unshield_backend.clone(),
            ProofBox::new(
                unshield_backend.clone(),
                vec![0x65; KAGEMUSHA_UNSHIELD_MAX_PROOF_BYTES_V4],
            ),
            VerifyingKeyId::new(unshield_backend, "unshield-v3-wire-limit"),
        );
        semantic_unshield.vk_commitment = Some([0x68; 32]);
        validate_kagemusha_redeem_proof_attachment_v2(&semantic_unshield)
            .expect("maximum-sized unshield proof remains structurally valid");
        semantic_unshield.proof.bytes.push(0x65);
        assert!(matches!(
            validate_kagemusha_redeem_proof_attachment_v2(&semantic_unshield),
            Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "redeem_proof",
            })
        ));
        let mut maximum_unshield = request.clone();
        maximum_unshield.redeem_proof.proof.bytes =
            vec![0x65; KAGEMUSHA_UNSHIELD_MAX_PROOF_BYTES_V4];
        let maximum_unshield_archive = norito::encode_canonical(&maximum_unshield)
            .expect("encode maximum-sized unshield request");
        preflight_kagemusha_redeem_request_archive_v4(&maximum_unshield_archive)
            .expect("wire preflight accepts the exact unshield limit");
        let maximum_unsigned = maximum_unshield.unsigned_payload();
        let maximum_unsigned_archive = norito::encode_canonical(&maximum_unsigned)
            .expect("encode maximum-sized unsigned unshield fixture");
        preflight_kagemusha_redeem_unsigned_archive_v4(&maximum_unsigned_archive)
            .expect("unsigned wire preflight accepts the exact unshield limit");
        let maximum_build_result = KagemushaRecursiveSpendRedeemBuildResultV4 {
            operation_id: maximum_unsigned.operation_id,
            unsigned: maximum_unsigned,
            authorization_digest: [0x67; 32],
            offline_change_bundle: None,
            offline_change_membership_witness: None,
            offline_change_topup_provenance: None,
        };
        let maximum_build_result_archive = norito::encode_canonical(&maximum_build_result)
            .expect("encode maximum-sized redemption-build result fixture");
        preflight_kagemusha_redeem_build_result_archive_v4(&maximum_build_result_archive)
            .expect("build-result wire preflight accepts the exact unshield limit");
        maximum_unshield.redeem_proof.proof.bytes.push(0x65);
        let oversized_request_archive = norito::encode_canonical(&maximum_unshield)
            .expect("encode oversized unshield request fixture");
        assert!(matches!(
            preflight_kagemusha_redeem_request_archive_v4(&oversized_request_archive),
            Err(norito::Error::FieldLengthExceeded { length, limit })
                if length == (KAGEMUSHA_UNSHIELD_MAX_PROOF_BYTES_V4 + 1) as u64
                    && limit == KAGEMUSHA_UNSHIELD_MAX_PROOF_BYTES_V4 as u64
        ));
        let oversized_unsigned = maximum_unshield.unsigned_payload();
        let oversized_unsigned_archive = norito::encode_canonical(&oversized_unsigned)
            .expect("encode oversized unsigned unshield fixture");
        assert!(matches!(
            preflight_kagemusha_redeem_unsigned_archive_v4(&oversized_unsigned_archive),
            Err(norito::Error::FieldLengthExceeded { .. })
        ));
        let oversized_build_result = KagemushaRecursiveSpendRedeemBuildResultV4 {
            operation_id: oversized_unsigned.operation_id,
            unsigned: oversized_unsigned,
            authorization_digest: [0x67; 32],
            offline_change_bundle: None,
            offline_change_membership_witness: None,
            offline_change_topup_provenance: None,
        };
        let oversized_build_result_archive = norito::encode_canonical(&oversized_build_result)
            .expect("encode oversized redemption-build result fixture");
        assert!(matches!(
            preflight_kagemusha_redeem_build_result_archive_v4(&oversized_build_result_archive),
            Err(norito::Error::FieldLengthExceeded { .. })
        ));
        let mut maximum_request = request;
        maximum_request
            .bundle
            .recursive_proof
            .proof_envelope
            .proof
            .bytes =
            vec![0x64; KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 as usize];
        maximum_request.redeem_proof.proof.bytes =
            vec![0x65; KAGEMUSHA_UNSHIELD_MAX_PROOF_BYTES_V4];
        let mut change_bundle = maximum_request.bundle.clone();
        change_bundle.recursive_proof.proof_envelope.proof.bytes =
            vec![0x66; KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4 as usize];
        maximum_request.offline_change = Some(KagemushaRecursiveSpendRedeemChangeBranchV4 {
            output: change_bundle.statement.current_note.clone(),
            branch_claims: change_bundle.statement.branch_claims.clone(),
            bundle: change_bundle,
        });
        let maximum_archive =
            norito::encode_canonical(&maximum_request).expect("maximum-shaped redeem request");
        assert!(maximum_archive.len() <= KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4);
        let decoded =
            norito::decode_canonical_with_limits::<KagemushaRecursiveSpendRedeemRequestV4>(
                &maximum_archive,
                kagemusha_recursive_spend_redeem_decode_limits_v4(maximum_archive.len()),
            )
            .expect("maximum-shaped redeem request must fit the schema-bounded allocation budget");
        assert_eq!(decoded, maximum_request);
    }
    #[test]
    fn redeem_result_decode_limits_cover_the_exact_archive_ceiling() {
        let limits = kagemusha_recursive_spend_redeem_decode_limits_v4(
            KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4,
        );
        assert_eq!(
            limits.max_sequence_elements(),
            KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4
        );
        assert_eq!(
            limits.max_field_bytes(),
            KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4
        );
        assert_eq!(
            limits.max_total_elements(),
            KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4 * 2
        );
        assert_eq!(
            limits.max_total_allocated_bytes(),
            KAGEMUSHA_RECURSIVE_SPEND_REDEEM_REQUEST_MAX_BYTES_V4
                * (KAGEMUSHA_CANONICAL_DECODE_BASE_ALLOCATION_MULTIPLIER_V4
                    + KAGEMUSHA_REDEEM_CANONICAL_DECODE_EXTRA_ALLOCATION_MULTIPLIER_V4)
                + KAGEMUSHA_REDEEM_CANONICAL_DECODE_FIXED_ALLOCATION_ALLOWANCE_V4
        );
        assert_eq!(
            limits.max_nesting_depth(),
            KAGEMUSHA_RECURSIVE_SPEND_REDEEM_DECODE_MAX_NESTING_DEPTH_V4
        );
    }
    #[test]
    fn recipient_payment_request_expiry_is_exclusive() {
        let issued_at_ms = 1_800_000_000_000;
        let expires_at_ms = issued_at_ms + 30_000;
        let request = recipient_payment_request(&signing_key(11), issued_at_ms, expires_at_ms);
        request
            .validate_at(issued_at_ms)
            .expect("request is valid at issuance");
        request
            .validate_at(expires_at_ms - 1)
            .expect("request is valid immediately before expiry");
        assert!(request.validate_at(issued_at_ms - 1).is_err());
        assert!(request.validate_at(expires_at_ms).is_err());
        assert!(request.validate_at(expires_at_ms + 1).is_err());
    }
    #[test]
    fn receiver_acknowledgement_expiry_is_exclusive() {
        let issued_at_ms = 1_800_000_000_000;
        let expires_at_ms = issued_at_ms + 30_000;
        let receiver_key = signing_key(12);
        let request = recipient_payment_request(&receiver_key, issued_at_ms, expires_at_ms);
        let bundle = recipient_payment_bundle(&request);
        receiver_acknowledgement(&receiver_key, &request, &bundle, expires_at_ms - 1)
            .validate_for_payment_v4(&request, &bundle)
            .expect("acknowledgement is valid immediately before expiry");
        assert!(
            receiver_acknowledgement(&receiver_key, &request, &bundle, expires_at_ms)
                .validate_for_payment_v4(&request, &bundle)
                .is_err(),
            "acknowledgement at the exclusive expiry must fail closed",
        );
    }
    #[test]
    fn online_android_assertion_binds_every_authorization_coordinate_and_key() {
        let key = signing_key(31);
        let wrong_key = signing_key(32);
        let authorization = authorization(&key, None);
        let public_key = key.verifying_key().to_encoded_point(false);
        authorization
            .validate_for_payload(authorization.payload_digest)
            .expect("valid authorization structure");
        authorization
            .verify_hardware_signature(public_key.as_bytes())
            .expect("exact registered key verifies");
        assert!(
            authorization
                .verify_hardware_signature(
                    wrong_key.verifying_key().to_encoded_point(false).as_bytes(),
                )
                .is_err(),
            "a substituted assertion key must fail",
        );
        let mut mutations = Vec::new();
        let mut changed = authorization.clone();
        changed.authority = account(22);
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.device_id = "hardware-device-22".to_owned();
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.asset_definition_id = asset("other_cash");
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.operation_id = [0x31; 32];
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.issued_at_ms += 1;
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.expires_at_ms += 1;
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.nonce = [0x32; 32];
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.payload_digest = [0x33; 32];
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.registration_hash = [0x34; 32];
        mutations.push(changed);
        let mut changed = authorization.clone();
        changed.hardware_assertion = KagemushaOnlineHardwareAssertionV1::IosAppAttest(
            KagemushaIosAppAttestHardwareAssertionV1 {
                authenticator_data: vec![0; 37],
                signature: match &authorization.hardware_assertion {
                    KagemushaOnlineHardwareAssertionV1::AndroidKeyMint(assertion) => {
                        assertion.signature
                    }
                    KagemushaOnlineHardwareAssertionV1::IosAppAttest(_) => unreachable!(),
                },
            },
        );
        mutations.push(changed);
        for mutation in mutations {
            assert!(
                mutation
                    .verify_hardware_signature(public_key.as_bytes())
                    .is_err(),
                "every account/device/asset/platform/hash/time/operation coordinate is signed",
            );
        }
    }
    #[test]
    fn online_ios_assertion_binds_authenticator_data_and_client_data_hash() {
        let key = signing_key(41);
        let mut authenticator_data = vec![0_u8; 37];
        authenticator_data[..32].copy_from_slice(&[0x41; 32]);
        authenticator_data[36] = 1;
        let authorization = authorization(&key, Some(authenticator_data));
        let public_key = key.verifying_key().to_encoded_point(false);
        authorization
            .verify_hardware_signature(public_key.as_bytes())
            .expect("exact App Attest assertion verifies");
        let mut changed_counter = authorization.clone();
        let KagemushaOnlineHardwareAssertionV1::IosAppAttest(assertion) =
            &mut changed_counter.hardware_assertion
        else {
            unreachable!()
        };
        assertion.authenticator_data[36] = 2;
        assert!(
            changed_counter
                .verify_hardware_signature(public_key.as_bytes())
                .is_err(),
            "the signature must bind the exact authenticatorData counter",
        );
        let mut wrong_length = authorization;
        let KagemushaOnlineHardwareAssertionV1::IosAppAttest(assertion) =
            &mut wrong_length.hardware_assertion
        else {
            unreachable!()
        };
        assertion.authenticator_data.truncate(36);
        assert!(
            wrong_length
                .validate_for_payload(wrong_length.payload_digest)
                .is_err(),
            "truncated assertion authData must fail at typed ingress",
        );
    }
}
