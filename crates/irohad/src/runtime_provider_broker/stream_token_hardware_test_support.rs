// Structurally valid untrusted transport claims only. The custody bytes and non-role signature
// messages below do not establish attestation, completion, finality or hardware qualification.
mod stream_token_hardware_test_support {
    use super::*;
    use sorafs_manifest::signer::{
        custody::{SignerCustodyAnchorV1, SignerCustodyBindingV1},
        protocol::{
            SignerKeyAlgorithmV1, SignerKeyOperationPurposeV1, SignerOperationActionV1,
            SignerOperationAuditHeadV1, SignerOperationCommitmentV1, SignerOperationCustodyV1,
            SignerOperationIntentV1, SignerOperationReservationV1, SignerOperationSignatureV1,
            SignerPurposeBindingV1, SignerRoleV1,
        },
        receipt::SignerOperationProvenanceV1,
        stream_token::{SIGNER_STREAM_TOKEN_RECEIPT_MAGIC_V1, SignerStreamTokenRequestV1},
    };

    pub(super) fn hardware_binding() -> StreamTokenHardwareRuntimeBindingV1 {
        StreamTokenHardwareRuntimeBindingV1::new(
            SignerCustodyBindingV1 {
                chain_id: "stream-token-mutation-test-chain".to_owned(),
                network_id: *network_id().as_bytes(),
                runtime_handle: "hsm://sorafs/stream-token/primary-a".to_owned(),
                key_handle: "pkcs11://sorafs/stream-token/key-primary".to_owned(),
                service_id: "stream-primary".to_owned(),
                administrator_id: "stream-security-primary".to_owned(),
                role: SignerRoleV1::StreamToken,
                purpose: SignerPurposeBindingV1::StreamToken {
                    provider_id: [0x22; 32],
                },
                algorithm: SignerKeyAlgorithmV1::Ed25519,
                public_key: test_auth_keypair().public_key().clone(),
                key_revision: 7,
                policy_revision: 9,
                policy_digest: TEST_POLICY_DIGEST,
            },
            "state://sorafs/stream-token/observer-primary".to_owned(),
            [0x74; 32],
        )
        .expect("bounded exact simulated public pins")
    }

    pub(super) fn body() -> sorafs_manifest::StreamTokenBodyV1 {
        sorafs_manifest::StreamTokenBodyV1 {
            token_id: "0123456789abcdef0123456789abcdef".to_owned(),
            manifest_cid: vec![0x21; 32],
            provider_id: [0x22; 32],
            profile_handle: "sorafs.sf1@1.0.0".to_owned(),
            max_streams: 4,
            ttl_epoch: 1_700_000_600,
            rate_limit_bytes: 8 * 1024 * 1024,
            issued_at: 1_700_000_000,
            requests_per_minute: 120,
            token_pk_version: 7,
        }
    }

    pub(super) fn expected(
        body: &sorafs_manifest::StreamTokenBodyV1,
    ) -> SignerStreamTokenExpectedV1 {
        SignerStreamTokenExpectedV1::new(body, hardware_binding().custody())
            .expect("independently prepared test operation")
    }

    pub(super) fn receipt(payload: &[u8]) -> SignerStreamTokenReceiptV1 {
        let (_, expected) =
            prepare_stream_token_signing_payload_v1(payload, hardware_binding().custody())
                .expect("canonical exact test body and provider");
        let original_custody = SignerOperationCustodyV1 {
            record_digest: [0x81; 32],
            control_state_digest: [0x82; 32],
        };
        let request = SignerStreamTokenRequestV1 {
            operation_id: expected.operation_id(),
            binding_digest: expected.binding_digest(),
            original_custody,
            signing_payload_digest: expected.signing_payload_digest(),
            signing_payload_size: expected.signing_payload_size(),
            issued_at_unix_ms: expected.issued_at_unix_ms(),
            expires_at_unix_ms: expected.expires_at_unix_ms(),
        };
        let intent = SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: expected.operation_id(),
            request_digest: request.digest().unwrap(),
            previous_audit: SignerOperationAuditHeadV1 {
                sequence: 0,
                digest: [0; 32],
            },
        };
        let reservation = SignerOperationReservationV1 {
            reservation_id: [0x83; 32],
            fence: 1,
            expires_at_unix_ms: 1_700_000_600_000,
        };
        let audit = SignerOperationAuditHeadV1 {
            sequence: 1,
            digest: [0x84; 32],
        };
        let signature = Signature::try_new(test_auth_keypair().private_key(), payload)
            .expect("actual exact role signature");
        SignerStreamTokenReceiptV1 {
            magic: SIGNER_STREAM_TOKEN_RECEIPT_MAGIC_V1,
            version: 1,
            custody_record: vec![0x85; 32],
            request,
            intent,
            reservation,
            provenance: SignerOperationProvenanceV1 {
                original_custody,
                signing_anchor: SignerCustodyAnchorV1 {
                    height: 10,
                    block_hash: [0x86; 32],
                    state_digest: [0x87; 32],
                },
                intent_digest: intent.digest().unwrap(),
                reservation,
                audit,
            },
            commitment: SignerOperationCommitmentV1 {
                audit,
                response_digest: [0x88; 32],
            },
            signatures: [
                SignerKeyOperationPurposeV1::RolePayload,
                SignerKeyOperationPurposeV1::AuditRecord,
                SignerKeyOperationPurposeV1::Provenance,
                SignerKeyOperationPurposeV1::Response,
            ]
            .into_iter()
            .map(|purpose| SignerOperationSignatureV1 {
                purpose,
                message_digest: [0x89; 32],
                signature: signature.payload().to_vec(),
            })
            .collect(),
        }
    }
    pub(super) fn query() -> SignerStreamTokenObservationRequestV1 {
        use sorafs_manifest::signer::stream_token_evidence::SignerStreamTokenObservationPhaseV1;
        SignerStreamTokenObservationRequestV1 {
            magic: SignerStreamTokenObservationRequestV1::magic(),
            phase: SignerStreamTokenObservationPhaseV1::Startup,
            subject: SignerStreamTokenObservationRequestSubjectV1::CurrentCustody {
                binding_digest: stream_token_binding_digest_v1(hardware_binding().custody())
                    .unwrap(),
            },
            challenge: [0x91; 32],
            minimum_anchor: SignerCustodyAnchorV1 {
                height: 10,
                block_hash: [0x92; 32],
                state_digest: [0x93; 32],
            },
            not_before_unix_ms: 1_700_000_000_000,
        }
    }

    pub(super) fn state_claim(
        query: &SignerStreamTokenObservationRequestV1,
    ) -> SignerStreamTokenStateObservationV1 {
        use sorafs_manifest::signer::{
            custody::{SignerCustodyActiveHeadV1, SignerCustodyAuthorityV1},
            stream_token_evidence::{
                SignerStreamTokenStateObservationBodyV1, SignerStreamTokenStateSubjectV1,
            },
        };
        let hardware = hardware_binding();
        SignerStreamTokenStateObservationV1 {
            body: SignerStreamTokenStateObservationBodyV1 {
                magic: SignerStreamTokenStateObservationBodyV1::magic(),
                request_digest: query.digest().unwrap(),
                phase: query.phase,
                subject: SignerStreamTokenStateSubjectV1::CurrentCustody {
                    binding_digest: stream_token_binding_digest_v1(hardware.custody()).unwrap(),
                },
                authority: SignerCustodyAuthorityV1 {
                    service_id: "independent-observer".to_owned(),
                    administrator_id: "observer-administrator".to_owned(),
                    key_revision: 1,
                    policy_revision: 1,
                    policy_digest: [0x94; 32],
                },
                chain_id: hardware.custody().chain_id.clone(),
                network_id: hardware.custody().network_id,
                observed_at_unix_ms: query.not_before_unix_ms,
                expires_at_unix_ms: query.not_before_unix_ms + 1000,
                current_anchor: query.minimum_anchor,
                active_head: SignerCustodyActiveHeadV1 {
                    record_digest: [0x95; 32],
                    sequence: 1,
                    approved_anchor: query.minimum_anchor,
                    key_revision: hardware.custody().key_revision,
                    policy_revision: hardware.custody().policy_revision,
                    policy_digest: hardware.custody().policy_digest,
                },
                signer_revoked: false,
                attester_revoked: false,
            },
            // Deliberately unauthenticated: transport must preserve claims without minting authority.
            signature: [0x96; 64],
        }
    }
}
