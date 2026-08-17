use super::{
    SoftwareSignerKeyAlgorithmV1, SoftwareSignerProvisioningV1, SoftwareSignerPurposeBindingV1,
    SoftwareSignerRoleV1, SoftwareSignerServiceV1, SoftwareSignerSignatureReceiptV1,
    SoftwareSignerWrappingKeyV1,
    protocol::{
        AdminCommandV1, AdminRequestV1, AdminStatusV1, SORAFS_FOUNDATIONAL_PROMOTION_DOMAIN_V1,
        SignRequestV1, SignStatusV1, admin_request_digest, payload_digest, sign_request_digest,
    },
    typed_payload::{SoftwareSignerPurposeV1, encode_typed_signing_payload},
};
use iroha_crypto::{Hash, HashOf, Signature};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    block::BlockHeader,
    isi::{
        InstructionBox,
        sorafs::{MatchSorafsOrderbook, SubmitSorafsRepairTask},
    },
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use sorafs_manifest::{
    POTR_RECEIPT_VERSION_V1, PotrReceiptV1, PotrStatus, StreamTokenBodyV1,
    proof_stream::ProofStreamTier,
};
use std::{fs, os::unix::fs::PermissionsExt as _, path::Path};
const WRAPPING_KEY: [u8; 32] = [0xA5; 32];
fn test_network_id() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([0x53; Hash::LENGTH]),
    ))
}
fn wrapping_key() -> SoftwareSignerWrappingKeyV1 {
    SoftwareSignerWrappingKeyV1::try_from_bytes(WRAPPING_KEY).expect("valid wrapping key")
}
fn provisioning(
    role: SoftwareSignerRoleV1,
    algorithm: SoftwareSignerKeyAlgorithmV1,
) -> SoftwareSignerProvisioningV1 {
    let service_uid = rustix::process::geteuid().as_raw();
    let handle_role = match role {
        SoftwareSignerRoleV1::ProofOutcome => "proof-outcome",
        SoftwareSignerRoleV1::Repair => "repair",
        SoftwareSignerRoleV1::Reserve => "reserve",
        SoftwareSignerRoleV1::Orderbook => "orderbook",
        SoftwareSignerRoleV1::Promotion => "promotion",
        SoftwareSignerRoleV1::GovernanceDag => "governance-dag",
        SoftwareSignerRoleV1::PotrGateway | SoftwareSignerRoleV1::PotrProvider => "potr",
        SoftwareSignerRoleV1::BillingStatement => "billing",
        SoftwareSignerRoleV1::EvidenceViewer => "evidence-viewer",
        SoftwareSignerRoleV1::StreamToken => "stream-token",
        SoftwareSignerRoleV1::PopCredentials => "pop-credentials",
        SoftwareSignerRoleV1::TairaAuthority => "taira-authority",
    };
    let purpose_binding = match role {
        SoftwareSignerRoleV1::ProofOutcome
        | SoftwareSignerRoleV1::Repair
        | SoftwareSignerRoleV1::Reserve
        | SoftwareSignerRoleV1::Orderbook
        | SoftwareSignerRoleV1::Promotion => SoftwareSignerPurposeBindingV1::NativeOrPromotion,
        SoftwareSignerRoleV1::GovernanceDag => SoftwareSignerPurposeBindingV1::GovernanceDag {
            publisher_peer_id: b"12D3KooWSoftwareSignerFixture".to_vec(),
        },
        SoftwareSignerRoleV1::PotrGateway => SoftwareSignerPurposeBindingV1::PotrGateway {
            signer_id: [0x31; 32],
        },
        SoftwareSignerRoleV1::PotrProvider => SoftwareSignerPurposeBindingV1::PotrProvider {
            signer_id: [0x32; 32],
            provider_id: [0x33; 32],
        },
        SoftwareSignerRoleV1::BillingStatement => {
            SoftwareSignerPurposeBindingV1::BillingStatement {
                signer_id: "billing-signer-primary".to_owned(),
            }
        }
        SoftwareSignerRoleV1::EvidenceViewer => SoftwareSignerPurposeBindingV1::EvidenceViewer,
        SoftwareSignerRoleV1::StreamToken => SoftwareSignerPurposeBindingV1::StreamToken,
        SoftwareSignerRoleV1::PopCredentials => SoftwareSignerPurposeBindingV1::PopCredentials {
            issuer_id: "pop-issuer-primary".to_owned(),
        },
        SoftwareSignerRoleV1::TairaAuthority => SoftwareSignerPurposeBindingV1::TairaAuthority {
            role: "native-evidence".to_owned(),
        },
    };
    let instance = match role {
        SoftwareSignerRoleV1::PotrGateway => "gateway-primary",
        SoftwareSignerRoleV1::PotrProvider => "provider-primary",
        _ => "primary",
    };
    SoftwareSignerProvisioningV1 {
        handle: format!("software://sorafs/{handle_role}/{instance}"),
        service_id: format!("{}-signer-primary", role.as_str()),
        administrator_id: format!("{}-security-primary", role.as_str()),
        service_uid,
        client_uid: service_uid.checked_add(1).expect("fixture client uid"),
        administrator_uid: service_uid
            .checked_add(2)
            .expect("fixture administrator uid"),
        role,
        purpose_binding,
        algorithm,
        key_revision: 1,
        policy_revision: 1,
        policy_digest: [0x51; 32],
        max_request_bytes: 1024 * 1024,
    }
}
fn temporary_parent() -> tempfile::TempDir {
    tempfile::tempdir_in(std::env::current_dir().expect("current directory"))
        .expect("secure temporary parent")
}
fn provision(
    parent: &Path,
    role: SoftwareSignerRoleV1,
    algorithm: SoftwareSignerKeyAlgorithmV1,
) -> SoftwareSignerServiceV1 {
    SoftwareSignerServiceV1::provision(
        parent.join("state"),
        provisioning(role, algorithm),
        wrapping_key(),
    )
    .expect("provision fixture signer")
}
fn sign_request(
    service: &SoftwareSignerServiceV1,
    operation_id: [u8; 32],
    payload: Vec<u8>,
) -> SignRequestV1 {
    let binding = service.public_binding().expect("fixture binding");
    let mut request = SignRequestV1 {
        binding_digest: binding.digest().expect("binding digest"),
        operation_id,
        expected_key_revision: binding.key_revision,
        expected_policy_revision: binding.policy_revision,
        expected_policy_digest: binding.policy_digest,
        payload_digest: payload_digest(&payload),
        payload,
        request_digest: [0; 32],
    };
    request.request_digest = sign_request_digest(&request).expect("request digest");
    request
}
fn typed_request(
    service: &SoftwareSignerServiceV1,
    operation_id: [u8; 32],
    purpose: SoftwareSignerPurposeV1,
    message: &[u8],
) -> SignRequestV1 {
    let role = service.public_binding().expect("fixture binding").role;
    let payload = encode_typed_signing_payload(role, purpose, message)
        .expect("purpose belongs to fixture role");
    sign_request(service, operation_id, payload)
}
fn unsigned_potr_receipt(provider_id: [u8; 32]) -> PotrReceiptV1 {
    PotrReceiptV1 {
        version: POTR_RECEIPT_VERSION_V1,
        manifest_digest: [0x11; 32],
        provider_id,
        tier: ProofStreamTier::Hot,
        deadline_ms: 90_000,
        latency_ms: 42_000,
        status: PotrStatus::Success,
        requested_at_ms: 1_700_000_000_000,
        responded_at_ms: 1_700_000_042_000,
        recorded_at_ms: 1_700_000_042_100,
        range_start: 0,
        range_end: 1_048_575,
        request_id: Some([0x44; 16]),
        trace_id: Some([0x33; 16]),
        note: Some("ok".to_owned()),
        gateway_signature: None,
        provider_signature: None,
    }
}
fn evidence_checkpoint_anchor_message(binding: &super::SoftwareSignerPublicBindingV1) -> Vec<u8> {
    let checkpoint_handle = "runtime://sorafs/evidence-viewer/checkpoint-store/primary";
    let (_, public_key) = binding
        .public_key
        .try_to_bytes()
        .expect("fixture public key bytes");
    let mut message = b"sorafs.evidence-viewer.checkpoint-signature.v1".to_vec();
    message.extend_from_slice(
        &sorafs_node::evidence_viewer::EVIDENCE_VIEWER_CHECKPOINT_VERSION_V1.to_le_bytes(),
    );
    message.extend_from_slice(&1_u64.to_le_bytes());
    message.extend_from_slice(&[0, 0]);
    message.extend_from_slice(&[0x61; 32]);
    message.extend_from_slice(&0_u64.to_le_bytes());
    message.push(0);
    message.push(0);
    message.extend_from_slice(
        &u64::try_from(checkpoint_handle.len())
            .expect("checkpoint handle length")
            .to_le_bytes(),
    );
    message.extend_from_slice(checkpoint_handle.as_bytes());
    message.extend_from_slice(&1_u64.to_le_bytes());
    message.extend_from_slice(&[0x62; 32]);
    message.extend_from_slice(
        &u64::try_from(binding.handle.len())
            .expect("signer handle length")
            .to_le_bytes(),
    );
    message.extend_from_slice(binding.handle.as_bytes());
    message.extend_from_slice(public_key);
    message
}
fn assert_typed_signs(
    service: &SoftwareSignerServiceV1,
    operation_id: [u8; 32],
    purpose: SoftwareSignerPurposeV1,
    message: &[u8],
) {
    let response = service
        .handle_sign_request(&typed_request(service, operation_id, purpose, message))
        .expect("typed signing request");
    assert_eq!(response.status, SignStatusV1::Ok);
    Signature::try_from_bytes(&response.signature)
        .expect("typed signature")
        .verify(
            &service
                .public_binding()
                .expect("fixture binding")
                .public_key,
            message,
        )
        .expect("verify typed signature");
}
fn native_payload(service: &SoftwareSignerServiceV1) -> (Vec<u8>, [u8; 32]) {
    let binding = service.public_binding().expect("fixture binding");
    let builder = TransactionBuilder::new(
        test_network_id(),
        AccountId::new(binding.public_key),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([InstructionBox::from(SubmitSorafsRepairTask::new(
        [0x91; 32],
        vec![0x01],
    ))]);
    (builder.encode_payload(), builder.payload_hash_bytes())
}
fn admin_request(service: &SoftwareSignerServiceV1, command: AdminCommandV1) -> AdminRequestV1 {
    let binding_digest = service
        .public_binding()
        .expect("fixture binding")
        .digest()
        .expect("binding digest");
    AdminRequestV1 {
        binding_digest,
        request_digest: admin_request_digest(binding_digest, &command)
            .expect("admin request digest"),
        command,
    }
}
#[test]
fn native_signing_and_recovery_cover_ed25519_and_ml_dsa() {
    for (index, algorithm) in [
        SoftwareSignerKeyAlgorithmV1::Ed25519,
        SoftwareSignerKeyAlgorithmV1::MlDsa,
    ]
    .into_iter()
    .enumerate()
    {
        let parent = temporary_parent();
        let service = provision(parent.path(), SoftwareSignerRoleV1::Repair, algorithm);
        let binding = service.public_binding().expect("binding");
        let (payload, signing_message) = native_payload(&service);
        let request = sign_request(
            &service,
            [u8::try_from(index + 1).expect("id"); 32],
            payload.clone(),
        );
        let response = service
            .handle_sign_request(&request)
            .expect("sign native transaction");
        assert_eq!(response.status, SignStatusV1::Ok);
        Signature::try_from_bytes(&response.signature)
            .expect("decode signature")
            .verify(&binding.public_key, &signing_message)
            .expect("verify signature");
        let next_request = sign_request(
            &service,
            [u8::try_from(index + 11).expect("next id"); 32],
            payload.clone(),
        );
        service
            .handle_sign_request(&next_request)
            .expect("commit a later native signature");
        let historical_replay = service
            .handle_sign_request(&request)
            .expect("replay the earlier native signature");
        assert_eq!(historical_replay.status, SignStatusV1::Replayed);
        assert_eq!(historical_replay.signature, response.signature);
        assert!(historical_replay.commit_sequence < historical_replay.provenance.audit_sequence);
        assert_ne!(
            historical_replay.commit_audit_head,
            historical_replay.provenance.audit_head
        );
        SoftwareSignerSignatureReceiptV1 {
            operation_id: request.operation_id,
            request_digest: historical_replay.request_digest,
            payload_digest: historical_replay.payload_digest,
            payload_length: u64::try_from(payload.len()).expect("payload length"),
            signature: historical_replay.signature.clone(),
            commit_sequence: historical_replay.commit_sequence,
            commit_audit_head: historical_replay.commit_audit_head,
            replayed: true,
            provenance: historical_replay.provenance.clone(),
            response_digest: historical_replay.response_digest,
            response_attestation: historical_replay.response_attestation.clone(),
        }
        .verify_offline(
            &binding,
            request.operation_id,
            &payload,
            &historical_replay.signature,
        )
        .expect("verify signer-attested historical native replay");
        let audit_head = historical_replay.provenance.audit_head;
        drop(service);
        let reopened = SoftwareSignerServiceV1::open(parent.path().join("state"), wrapping_key())
            .expect("recover signer");
        assert_eq!(
            reopened
                .provenance()
                .expect("recovered provenance")
                .audit_head,
            audit_head
        );
        assert_eq!(
            reopened
                .handle_sign_request(&request)
                .expect("replay recovered signature")
                .status,
            SignStatusV1::Replayed
        );
    }
}
#[test]
fn typed_service_boundary_enforces_roles_purposes_algorithms_and_public_identities() {
    let governance_parent = temporary_parent();
    let governance = provision(
        governance_parent.path(),
        SoftwareSignerRoleV1::GovernanceDag,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    let transition =
        sorafs_node::governance_dag_key_transition_signing_payload_v1(1, 2, [0x41; 32])
            .expect("governance transition payload");
    assert_typed_signs(
        &governance,
        [0x01; 32],
        SoftwareSignerPurposeV1::GovernanceKeyTransition,
        &transition,
    );
    let cross_role = encode_typed_signing_payload(
        SoftwareSignerRoleV1::EvidenceViewer,
        SoftwareSignerPurposeV1::EvidenceReceipt,
        &[0x42; 32],
    )
    .expect("different role payload");
    assert_eq!(
        governance.handle_sign_request(&sign_request(&governance, [0x02; 32], cross_role,)),
        Err(super::SoftwareSignerErrorV1::Rejected)
    );
    let provider_parent = temporary_parent();
    let provider = provision(
        provider_parent.path(),
        SoftwareSignerRoleV1::PotrProvider,
        SoftwareSignerKeyAlgorithmV1::MlDsa,
    );
    let receipt = unsigned_potr_receipt([0x33; 32]);
    let receipt_message = receipt.signing_payload_bytes().expect("PoTR payload");
    assert_typed_signs(
        &provider,
        [0x03; 32],
        SoftwareSignerPurposeV1::PotrProviderReceipt,
        &receipt_message,
    );
    let substituted = unsigned_potr_receipt([0x34; 32])
        .signing_payload_bytes()
        .expect("substituted PoTR payload");
    assert_eq!(
        provider.handle_sign_request(&typed_request(
            &provider,
            [0x04; 32],
            SoftwareSignerPurposeV1::PotrProviderReceipt,
            &substituted,
        )),
        Err(super::SoftwareSignerErrorV1::Rejected),
        "the raw protocol must enforce the provisioned provider identity"
    );
    let gateway_parent = temporary_parent();
    let gateway = provision(
        gateway_parent.path(),
        SoftwareSignerRoleV1::PotrGateway,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    assert_typed_signs(
        &gateway,
        [0x05; 32],
        SoftwareSignerPurposeV1::PotrGatewayReceipt,
        &receipt_message,
    );
    let billing_parent = temporary_parent();
    let billing = provision(
        billing_parent.path(),
        SoftwareSignerRoleV1::BillingStatement,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    assert_typed_signs(
        &billing,
        [0x06; 32],
        SoftwareSignerPurposeV1::BillingStatement,
        &[0x51; 32],
    );
    for invalid in [vec![0; 32], vec![0x51; 31], vec![0x51; 33]] {
        assert_eq!(
            billing.handle_sign_request(&typed_request(
                &billing,
                [u8::try_from(invalid.len()).expect("bounded length"); 32],
                SoftwareSignerPurposeV1::BillingStatement,
                &invalid,
            )),
            Err(super::SoftwareSignerErrorV1::Rejected)
        );
    }
    let evidence_parent = temporary_parent();
    let evidence = provision(
        evidence_parent.path(),
        SoftwareSignerRoleV1::EvidenceViewer,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    let mut receipt_message = b"sorafs.evidence-viewer.receipt-signature.v1".to_vec();
    receipt_message.extend_from_slice(&[0x52; 32]);
    let checkpoint_anchor =
        evidence_checkpoint_anchor_message(&evidence.public_binding().expect("evidence binding"));
    for (operation, purpose, message) in [
        (
            0x07,
            SoftwareSignerPurposeV1::EvidenceReceipt,
            receipt_message.as_slice(),
        ),
        (
            0x08,
            SoftwareSignerPurposeV1::EvidenceCheckpointStoreRecord,
            &[0x53; 32],
        ),
        (
            0x09,
            SoftwareSignerPurposeV1::EvidenceCheckpointAnchor,
            checkpoint_anchor.as_slice(),
        ),
        (
            0x0A,
            SoftwareSignerPurposeV1::EvidenceCompactionArchive,
            &[0x54; 32],
        ),
    ] {
        assert_typed_signs(&evidence, [operation; 32], purpose, message);
    }
    let first = typed_request(
        &evidence,
        [0x0B; 32],
        SoftwareSignerPurposeV1::EvidenceCheckpointStoreRecord,
        &[0x55; 32],
    );
    assert_eq!(
        evidence
            .handle_sign_request(&first)
            .expect("first purpose commit")
            .status,
        SignStatusV1::Ok
    );
    assert_eq!(
        evidence
            .handle_sign_request(&typed_request(
                &evidence,
                [0x0B; 32],
                SoftwareSignerPurposeV1::EvidenceCompactionArchive,
                &[0x55; 32],
            ))
            .expect("cross-purpose replay result")
            .status,
        SignStatusV1::Equivocation
    );
    let stream_parent = temporary_parent();
    let stream = provision(
        stream_parent.path(),
        SoftwareSignerRoleV1::StreamToken,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    let stream_message = StreamTokenBodyV1 {
        token_id: "11".repeat(16),
        manifest_cid: vec![0x61; 32],
        provider_id: [0x62; 32],
        profile_handle: "sorafs.standard".to_owned(),
        max_streams: 1,
        ttl_epoch: 1_060,
        rate_limit_bytes: 1_024,
        issued_at: 1_000,
        requests_per_minute: 1,
        token_pk_version: 1,
    }
    .signing_payload_bytes()
    .expect("stream-token signing payload");
    assert_typed_signs(
        &stream,
        [0x0C; 32],
        SoftwareSignerPurposeV1::StreamToken,
        &stream_message,
    );
    let pop_parent = temporary_parent();
    let pop = provision(
        pop_parent.path(),
        SoftwareSignerRoleV1::PopCredentials,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    for (operation, purpose) in [
        (0x0D, SoftwareSignerPurposeV1::PopCredential),
        (0x0E, SoftwareSignerPurposeV1::PopCommitmentRoot),
        (0x0F, SoftwareSignerPurposeV1::PopRevocationList),
    ] {
        assert_typed_signs(&pop, [operation; 32], purpose, &[operation; 32]);
    }
    for (role, algorithm) in [
        (
            SoftwareSignerRoleV1::GovernanceDag,
            SoftwareSignerKeyAlgorithmV1::MlDsa,
        ),
        (
            SoftwareSignerRoleV1::PotrProvider,
            SoftwareSignerKeyAlgorithmV1::Ed25519,
        ),
    ] {
        let invalid_parent = temporary_parent();
        assert!(
            SoftwareSignerServiceV1::provision(
                invalid_parent.path().join("state"),
                provisioning(role, algorithm),
                wrapping_key(),
            )
            .is_err()
        );
    }
}
#[test]
fn native_roles_reject_cross_role_empty_and_promotion_domain_payloads() {
    let parent = temporary_parent();
    let service = provision(
        parent.path(),
        SoftwareSignerRoleV1::Repair,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    let authority = AccountId::new(service.public_binding().expect("binding").public_key);
    let wrong_role = TransactionBuilder::new(
        test_network_id(),
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([InstructionBox::from(MatchSorafsOrderbook::new(
        [0x92; 32], 1, 1,
    ))])
    .encode_payload();
    assert_eq!(
        service
            .handle_sign_request(&sign_request(&service, [0x31; 32], wrong_role))
            .expect("cross-role request yields a signed rejection")
            .status,
        SignStatusV1::Rejected
    );
    let empty = TransactionBuilder::new(
        test_network_id(),
        authority,
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .encode_payload();
    assert_eq!(
        service
            .handle_sign_request(&sign_request(&service, [0x32; 32], empty))
            .expect("empty instruction request yields a signed rejection")
            .status,
        SignStatusV1::Rejected
    );
    let mut promotion = SORAFS_FOUNDATIONAL_PROMOTION_DOMAIN_V1.to_vec();
    promotion.extend_from_slice(br#"{"schema":"foundational"}"#);
    assert_eq!(
        service.handle_sign_request(&sign_request(&service, [0x33; 32], promotion)),
        Err(super::SoftwareSignerErrorV1::Rejected)
    );
}
#[test]
fn promotion_signs_exact_foundational_bytes_and_requires_ed25519() {
    let parent = temporary_parent();
    let service = provision(
        parent.path(),
        SoftwareSignerRoleV1::Promotion,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    let mut payload = SORAFS_FOUNDATIONAL_PROMOTION_DOMAIN_V1.to_vec();
    payload.extend_from_slice(
        br#"{"schema":"sorafs.production_readiness.foundational_prerequisites.v1"}"#,
    );
    let request = sign_request(&service, [0x11; 32], payload.clone());
    let response = service
        .handle_sign_request(&request)
        .expect("sign exact promotion bytes");
    let binding = service.public_binding().expect("binding");
    let receipt = SoftwareSignerSignatureReceiptV1 {
        operation_id: request.operation_id,
        request_digest: response.request_digest,
        payload_digest: response.payload_digest,
        payload_length: u64::try_from(payload.len()).expect("payload length"),
        signature: response.signature.clone(),
        commit_sequence: response.commit_sequence,
        commit_audit_head: response.commit_audit_head,
        replayed: false,
        provenance: response.provenance.clone(),
        response_digest: response.response_digest,
        response_attestation: response.response_attestation.clone(),
    };
    receipt
        .verify_offline(
            &binding,
            request.operation_id,
            &payload,
            &response.signature,
        )
        .expect("verify complete promotion receipt offline");
    let mut stale_receipt = receipt.clone();
    stale_receipt.provenance.audit_sequence = stale_receipt
        .provenance
        .audit_sequence
        .checked_add(1)
        .expect("fixture sequence");
    assert!(
        stale_receipt
            .verify_offline(
                &binding,
                request.operation_id,
                &payload,
                &response.signature
            )
            .is_err()
    );
    let mut tampered_attestation = receipt.clone();
    tampered_attestation.provenance.attestation[0] ^= 1;
    assert!(
        tampered_attestation
            .verify_offline(
                &binding,
                request.operation_id,
                &payload,
                &response.signature,
            )
            .is_err(),
        "excluding randomized attestation bytes from adjacent-state equality must not accept a tampered attestation"
    );
    let mut substituted_receipt = receipt;
    substituted_receipt.response_digest[0] ^= 1;
    assert!(
        substituted_receipt
            .verify_offline(
                &binding,
                request.operation_id,
                &payload,
                &response.signature
            )
            .is_err()
    );
    Signature::try_from_bytes(&response.signature)
        .expect("decode promotion signature")
        .verify(&binding.public_key, &payload)
        .expect("verify exact promotion bytes");
    let mut mutation = payload;
    mutation.push(b' ');
    assert!(
        Signature::try_from_bytes(&response.signature)
            .expect("decode promotion signature")
            .verify(&binding.public_key, &mutation,)
            .is_err()
    );
    let invalid_parent = temporary_parent();
    assert!(
        SoftwareSignerServiceV1::provision(
            invalid_parent.path().join("state"),
            provisioning(
                SoftwareSignerRoleV1::Promotion,
                SoftwareSignerKeyAlgorithmV1::MlDsa,
            ),
            wrapping_key(),
        )
        .is_err()
    );
}
#[test]
fn sign_idempotency_replays_exact_bytes_and_audits_equivocation() {
    let parent = temporary_parent();
    let service = provision(
        parent.path(),
        SoftwareSignerRoleV1::Promotion,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    let mut payload = SORAFS_FOUNDATIONAL_PROMOTION_DOMAIN_V1.to_vec();
    payload.extend_from_slice(
        br#"{"schema":"sorafs.production_readiness.foundational_prerequisites.v1"}"#,
    );
    let accepted = sign_request(&service, [0x21; 32], payload.clone());
    let first = service
        .handle_sign_request(&accepted)
        .expect("first signature");
    let replay = service
        .handle_sign_request(&accepted)
        .expect("idempotent replay");
    assert_eq!(replay.status, SignStatusV1::Replayed);
    assert_eq!(replay.signature, first.signature);
    payload.insert(SORAFS_FOUNDATIONAL_PROMOTION_DOMAIN_V1.len() + 1, b' ');
    let conflicting = sign_request(&service, [0x21; 32], payload);
    assert_eq!(
        service
            .handle_sign_request(&conflicting)
            .expect("payload-free equivocation result")
            .status,
        SignStatusV1::Equivocation
    );
    drop(service);
    SoftwareSignerServiceV1::open(parent.path().join("state"), wrapping_key())
        .expect("equivocation audit remains recoverable");
}
#[test]
fn rotation_is_predecessor_bound_replay_safe_and_revocation_is_terminal() {
    let parent = temporary_parent();
    let service = provision(
        parent.path(),
        SoftwareSignerRoleV1::Promotion,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    let before = service.provenance().expect("initial provenance");
    let rotate = admin_request(
        &service,
        AdminCommandV1::Rotate {
            operation_id: [0x31; 32],
            expected_audit_head: before.audit_head,
            expected_key_revision: 1,
            new_key_revision: 2,
            new_policy_revision: 2,
            new_policy_digest: [0x52; 32],
            algorithm: SoftwareSignerKeyAlgorithmV1::Ed25519,
        },
    );
    let mut substituted_binding = rotate.clone();
    substituted_binding.binding_digest[0] ^= 1;
    assert_eq!(
        service
            .handle_admin_request(&substituted_binding)
            .expect("substituted binding yields a signed rejection")
            .status,
        AdminStatusV1::Rejected
    );
    let rotated = service
        .handle_admin_request(&rotate)
        .expect("rotate signer");
    assert_eq!(rotated.status, AdminStatusV1::Ok);
    assert_eq!(rotated.provenance.binding.key_revision, 2);
    assert_eq!(
        service
            .handle_admin_request(&rotate)
            .expect("replay rotation")
            .status,
        AdminStatusV1::Replayed
    );
    let revoke = admin_request(
        &service,
        AdminCommandV1::Revoke {
            operation_id: [0x32; 32],
            expected_audit_head: rotated.provenance.audit_head,
            expected_key_revision: 2,
            reason_digest: [0x53; 32],
        },
    );
    let revoked = service
        .handle_admin_request(&revoke)
        .expect("revoke signer");
    assert!(revoked.provenance.revoked);
    let mut payload = SORAFS_FOUNDATIONAL_PROMOTION_DOMAIN_V1.to_vec();
    payload.extend_from_slice(b"{}");
    assert_eq!(
        service
            .handle_sign_request(&sign_request(&service, [0x33; 32], payload))
            .expect("revoked result")
            .status,
        SignStatusV1::StaleOrRevoked
    );
    drop(service);
    assert!(
        SoftwareSignerServiceV1::open(parent.path().join("state"), wrapping_key())
            .expect("recover revoked signer")
            .provenance()
            .expect("revoked provenance")
            .revoked
    );
}
#[test]
fn reviewed_successor_binding_rejects_complete_local_state_rollback() {
    let parent = temporary_parent();
    let service = provision(
        parent.path(),
        SoftwareSignerRoleV1::Promotion,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    let state = parent.path().join("state");
    let old_envelope = fs::read(state.join("key-envelope-v1.norito")).expect("old envelope");
    let before = service.provenance().expect("initial provenance");
    let rotated = service
        .handle_admin_request(&admin_request(
            &service,
            AdminCommandV1::Rotate {
                operation_id: [0x61; 32],
                expected_audit_head: before.audit_head,
                expected_key_revision: 1,
                new_key_revision: 2,
                new_policy_revision: 2,
                new_policy_digest: [0x62; 32],
                algorithm: SoftwareSignerKeyAlgorithmV1::Ed25519,
            },
        ))
        .expect("rotate signer");
    drop(service);
    fs::write(state.join("key-envelope-v1.norito"), old_envelope)
        .expect("restore predecessor envelope");
    fs::remove_file(state.join("audit-v1/00000000000000000002.norito"))
        .expect("remove successor journal record");
    let rolled_back = std::sync::Arc::new(
        SoftwareSignerServiceV1::open(&state, wrapping_key()).expect("open coherent predecessor"),
    );
    let runtime = parent.path().join("runtime-rollback");
    fs::create_dir(&runtime).expect("runtime directory");
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o711)).expect("runtime permissions");
    let policy = super::SoftwareSignerEndpointPolicyV1::try_new(
        runtime.join("request.sock"),
        runtime.join("administrator.sock"),
        rotated.provenance.binding,
    )
    .expect("reviewed successor binding");
    assert!(super::SoftwareSignerServerV1::try_new(rolled_back, policy).is_err());
    assert!(!runtime.join("request.sock").exists());
}
#[test]
fn wrong_key_corrupt_envelope_audit_and_permissions_fail_closed() {
    let parent = temporary_parent();
    let service = provision(
        parent.path(),
        SoftwareSignerRoleV1::Promotion,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    let state = parent.path().join("state");
    drop(service);
    assert!(
        SoftwareSignerServiceV1::open(
            &state,
            SoftwareSignerWrappingKeyV1::try_from_bytes([0xB6; 32]).expect("wrong key"),
        )
        .is_err()
    );
    let envelope = state.join("key-envelope-v1.norito");
    fs::set_permissions(&envelope, fs::Permissions::from_mode(0o644))
        .expect("weaken envelope permissions");
    assert!(SoftwareSignerServiceV1::open(&state, wrapping_key()).is_err());
    fs::set_permissions(&envelope, fs::Permissions::from_mode(0o600))
        .expect("restore envelope permissions");
    let audit = state.join("audit-v1/00000000000000000001.norito");
    let mut bytes = fs::read(&audit).expect("read audit record");
    let last = bytes.last_mut().expect("non-empty audit record");
    *last ^= 1;
    fs::write(&audit, bytes).expect("corrupt audit record");
    assert!(SoftwareSignerServiceV1::open(&state, wrapping_key()).is_err());
}
#[test]
fn state_symlinks_and_hardlinked_secret_envelopes_are_rejected() {
    let parent = temporary_parent();
    let service = provision(
        parent.path(),
        SoftwareSignerRoleV1::Promotion,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    let state = parent.path().join("state");
    drop(service);
    let alias = parent.path().join("state-alias");
    std::os::unix::fs::symlink(&state, &alias).expect("create state symlink");
    assert!(SoftwareSignerServiceV1::open(&alias, wrapping_key()).is_err());
    let envelope = state.join("key-envelope-v1.norito");
    fs::hard_link(&envelope, parent.path().join("envelope-copy"))
        .expect("create envelope hard link");
    assert!(SoftwareSignerServiceV1::open(&state, wrapping_key()).is_err());
}
#[test]
fn debug_output_redacts_runtime_key_and_envelope_ciphertext() {
    let key = wrapping_key();
    assert_eq!(
        format!("{key:?}"),
        "SoftwareSignerWrappingKeyV1([REDACTED])"
    );
    let parent = temporary_parent();
    let service = provision(
        parent.path(),
        SoftwareSignerRoleV1::Promotion,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    let debug = format!("{service:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains(&hex::encode(WRAPPING_KEY)));
}
#[test]
fn startup_rejects_a_substituted_public_binding_before_socket_creation() {
    let parent = temporary_parent();
    let service = std::sync::Arc::new(provision(
        parent.path(),
        SoftwareSignerRoleV1::Promotion,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    ));
    let mut substituted = service.public_binding().expect("binding");
    substituted.policy_digest = [0x77; 32];
    let runtime = parent.path().join("runtime");
    fs::create_dir(&runtime).expect("runtime directory");
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o711)).expect("runtime permissions");
    let policy = super::SoftwareSignerEndpointPolicyV1::try_new(
        runtime.join("request.sock"),
        runtime.join("administrator.sock"),
        substituted,
    )
    .expect("substituted binding remains structurally valid");
    assert!(super::SoftwareSignerServerV1::try_new(service, policy).is_err());
    assert!(!runtime.join("request.sock").exists());
    let wrong_role_parent = temporary_parent();
    let mut wrong_role = provisioning(
        SoftwareSignerRoleV1::Repair,
        SoftwareSignerKeyAlgorithmV1::Ed25519,
    );
    wrong_role.handle = "software://sorafs/orderbook/primary".to_owned();
    let wrong_role_state = wrong_role_parent.path().join("state");
    assert!(
        SoftwareSignerServiceV1::provision(&wrong_role_state, wrong_role, wrapping_key()).is_err()
    );
    assert!(!wrong_role_state.exists());
}
#[test]
fn request_and_administrator_peer_identities_are_not_interchangeable() {
    let service_uid = rustix::process::geteuid().as_raw();
    let client_uid = service_uid.checked_add(1).expect("client uid");
    let administrator_uid = service_uid.checked_add(2).expect("administrator uid");
    assert!(super::unix::peer_uid_is_authorized(client_uid, client_uid));
    assert!(super::unix::peer_uid_is_authorized(
        administrator_uid,
        administrator_uid
    ));
    assert!(!super::unix::peer_uid_is_authorized(
        service_uid,
        client_uid
    ));
    assert!(!super::unix::peer_uid_is_authorized(
        client_uid,
        administrator_uid
    ));
    assert!(!super::unix::peer_uid_is_authorized(
        administrator_uid,
        client_uid
    ));
}
#[test]
fn cli_value_parsers_accept_only_canonical_role_and_algorithm_labels() {
    assert_eq!(
        "promotion".parse::<SoftwareSignerRoleV1>(),
        Ok(SoftwareSignerRoleV1::Promotion)
    );
    assert_eq!(
        "ml-dsa-65".parse::<SoftwareSignerKeyAlgorithmV1>(),
        Ok(SoftwareSignerKeyAlgorithmV1::MlDsa)
    );
    for alias in ["foundational-promotion", "proof-outcome", "reserve-rent"] {
        assert!(alias.parse::<SoftwareSignerRoleV1>().is_err());
    }
    for alias in ["ml-dsa", "mldsa", "ML-DSA-65"] {
        assert!(alias.parse::<SoftwareSignerKeyAlgorithmV1>().is_err());
    }
}
