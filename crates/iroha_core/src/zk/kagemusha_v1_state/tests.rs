//! Focused aggregate-state tests for the three-message KAGEMUSHA V1 protocol.

use super::*;
use std::collections::BTreeMap;

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::BlockHeader,
    domain::DomainId,
    kagemusha::{
        KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1, KAGEMUSHA_PAYMENT_OUTBOX_MIN_BYTES_V1,
        KAGEMUSHA_REDEMPTION_OUTBOX_MIN_BYTES_V1, KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1,
        KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1, KagemushaAcknowledgementV1,
        KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, KagemushaEnabledProfileV1,
        KagemushaEncryptedCreditEnvelopeV1, KagemushaEvidenceFileV1, KagemushaHardwareCredentialV1,
        KagemushaHardwarePlatformClassV1, KagemushaHardwareProfileV1, KagemushaInboxReceiptV1,
        KagemushaLifecycleBindingV1, KagemushaMintAuthorizationContextV1,
        KagemushaMintAuthorizationStatementV1, KagemushaMintAuthorizationV1,
        KagemushaMintCreditStatementV1, KagemushaOperationKindV1, KagemushaPairedProofV1,
        KagemushaTrustedCommitTimeV1, kagemusha_acknowledgement_signing_bytes_v1,
        kagemusha_ciphertext_digest_v1, kagemusha_credit_opening_canonical_len_v1,
        kagemusha_device_key_reference_v1, kagemusha_mint_credit_opening_commitment_v1,
        kagemusha_payment_body_digest_v1, kagemusha_peer_credit_opening_commitment_v1,
        kagemusha_recipient_credential_commitment_v1,
    },
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};

use crate::zk::kagemusha_v1_recursion::{
    KagemushaMintFinalityHelperVerificationRequestV1, KagemushaParityVerificationRequestV1,
    KagemushaStateProofVerificationRequestV1,
};

const TEST_SUITE_COMMITMENT_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:suite-commitment";

#[derive(Clone, Copy, Debug)]
struct AcceptSnapshotRecursiveVerifierV1;

impl KagemushaRecursiveVerifierV1 for AcceptSnapshotRecursiveVerifierV1 {
    fn verify_state_proof_and_decide(
        &self,
        _request: &KagemushaStateProofVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn verify_payment_and_decide(
        &self,
        _request: &KagemushaPaymentRequestV1,
        _payment: &KagemushaPaymentV1,
    ) -> Result<(), String> {
        Ok(())
    }

    fn verify_mint_finality_helper(
        &self,
        _request: &KagemushaMintFinalityHelperVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn verify_terminal_authorization_and_decide(
        &self,
        _request: &KagemushaParityVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct AcceptSnapshotGuardVerifierV1;

impl KagemushaGuardBundleVerifierV1 for AcceptSnapshotGuardVerifierV1 {
    fn verify_mint_reservation(
        &self,
        _statement: &MintReservationStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Ok(())
    }

    fn verify_mint_stage(
        &self,
        _statement: &MintStageStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Ok(())
    }

    fn verify_bootstrap(
        &self,
        _statement: &BootstrapStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Ok(())
    }

    fn verify_transition(
        &self,
        _statement: &HardwareTransitionStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Ok(())
    }

    fn verify_credit_stage(
        &self,
        _statement: &CreditStageStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Ok(())
    }

    fn verify_durability_anchor(
        &self,
        _statement: &DurabilityAnchorStatementV1,
        _guard_bundle: &[u8],
    ) -> Result<(), String> {
        Ok(())
    }
}

fn hardware_test_lane() -> KagemushaLaneIdV1 {
    KagemushaLaneIdV1 {
        network_id: NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::new(b"kagemusha-v1-hardware-persistence-tests"),
        )),
        device_lane_id: [0x11; 32],
        asset: AssetDefinitionId::derive_from_components(
            DomainId::try_new("hardware", "universal").expect("valid test domain"),
            "cash".parse().expect("valid test asset name"),
        ),
        scale: 2,
    }
}

fn snapshot_digest(label: &[u8], tag: u8) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(label);
    hasher.update([0, tag]);
    hasher.finalize().into()
}

fn snapshot_indexed_digest(label: &[u8], index: u64) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(label);
    hasher.update([0]);
    hasher.update(index.to_be_bytes());
    hasher.finalize().into()
}

fn snapshot_device_public_key(signing_key: &SigningKey) -> KagemushaDevicePublicKeyV1 {
    KagemushaDevicePublicKeyV1::from_sec1_bytes(
        signing_key
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes(),
    )
    .expect("canonical snapshot-test P-256 key")
}

fn snapshot_device_signature(
    signing_key: &SigningKey,
    message: &[u8],
) -> KagemushaDeviceSignatureV1 {
    let signature: Signature = signing_key.sign(message);
    let signature = signature.normalize_s().unwrap_or(signature);
    KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
        .expect("canonical low-S snapshot-test signature")
}

fn snapshot_hardware_profile(
    suite_id: DigestV1,
    governance_key: &SigningKey,
) -> KagemushaHardwareProfileV1 {
    let mut hasher = Sha256::new();
    hasher.update(TEST_SUITE_COMMITMENT_DOMAIN_V1);
    hasher.update([0]);
    hasher.update(
        u64::try_from(suite_id.len())
            .expect("fixed suite ID length")
            .to_le_bytes(),
    );
    hasher.update(suite_id);
    let allowed_suite_commitment = hasher.finalize().into();
    KagemushaHardwareProfileV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
        hardware_profile_id: [0; 32],
        provider_id: snapshot_digest(b"snapshot-provider", 1),
        platform_class: KagemushaHardwarePlatformClassV1::DedicatedSecureElement,
        product_class_digest: snapshot_digest(b"snapshot-product", 2),
        firmware_policy_digest: snapshot_digest(b"snapshot-firmware", 3),
        enrollment_attestation_verifier_digest: snapshot_digest(b"snapshot-enrollment-verifier", 4),
        attestation_trust_roots_digest: snapshot_digest(b"snapshot-attestation-roots", 5),
        allowed_suite_commitment,
        policy_epoch: 1,
        governance_credential_public_key: snapshot_device_public_key(governance_key),
        capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
        qualification_report_digest: snapshot_digest(b"snapshot-qualification", 6),
        valid_from_ms: 1,
        expires_at_ms: 1_000_000,
    }
    .seal_hardware_profile_id()
    .expect("canonical snapshot-test hardware profile")
}

fn snapshot_hardware_credential(
    network_id: NetworkId,
    lane_commitment: DigestV1,
    epoch: HardwareEpochV1,
    profile: &KagemushaHardwareProfileV1,
    suite_id: DigestV1,
    device_key: &SigningKey,
    governance_key: &SigningKey,
) -> KagemushaHardwareCredentialV1 {
    let device_public_key = snapshot_device_public_key(device_key);
    let mut credential = KagemushaHardwareCredentialV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        credential_id: [0; 32],
        network_id,
        hardware_profile_id: profile.hardware_profile_id,
        suite_id,
        firmware_policy_digest: profile.firmware_policy_digest,
        policy_epoch: profile.policy_epoch,
        lane_commitment,
        hardware_epoch_id: epoch.epoch_id,
        hardware_epoch_generation: u64::try_from(epoch.generation)
            .expect("snapshot-test epoch fits u64"),
        device_public_key,
        device_key_reference: kagemusha_device_key_reference_v1(&device_public_key),
        issued_at_ms: 10,
        expires_at_ms: 900_000,
        governance_signature: snapshot_device_signature(governance_key, b"unsealed credential"),
    }
    .seal_credential_id()
    .expect("canonical snapshot-test credential ID");
    credential.governance_signature = snapshot_device_signature(
        governance_key,
        &credential
            .canonical_signing_bytes()
            .expect("snapshot-test credential signing bytes"),
    );
    credential
        .validate_against_profile(profile)
        .expect("snapshot-test credential matches governed profile");
    credential
}

fn snapshot_paired_proof(
    semantic_digest: DigestV1,
    eq_protocol_digest: DigestV1,
    ep_protocol_digest: DigestV1,
    tag: u8,
) -> KagemushaPairedProofV1 {
    KagemushaPairedProofV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        eq_protocol_digest,
        ep_protocol_digest,
        semantic_digest,
        guard_eq_credential_audit: snapshot_digest(b"snapshot-guard-eq", tag),
        guard_ep_credential_audit: snapshot_digest(b"snapshot-guard-ep", tag.wrapping_add(1)),
        eq_deferred_audit: eq_protocol_digest,
        ep_deferred_audit: ep_protocol_digest,
        eq_proof: vec![tag; 32],
        ep_proof: vec![tag.wrapping_add(1); 32],
        eq_history: crate::zk::kagemusha_v1_recursion::tests::eq_history(u64::from(tag))
            .as_bytes()
            .to_vec(),
        ep_history: crate::zk::kagemusha_v1_recursion::tests::ep_history(u64::from(tag) + 1)
            .as_bytes()
            .to_vec(),
    }
}

fn snapshot_mint_credit(
    state: &KagemushaStateV1,
    artifacts: KagemushaRecursionArtifactsV1,
    recipient_credential: &KagemushaHardwareCredentialV1,
    recipient: AccountId,
    amount: u128,
) -> (
    KagemushaMintAuthorizationV1,
    KagemushaMintCreditV1,
    KagemushaCreditOpeningV1,
) {
    let operation_id = snapshot_digest(b"snapshot-mint-operation", 1);
    let recipient_one_time_key = snapshot_digest(b"snapshot-mint-recipient-key", 2);
    let credit_commitment_opening = snapshot_digest(b"snapshot-mint-opening", 3);
    let recipient_binding_opening = snapshot_digest(b"snapshot-mint-recipient-opening", 4);
    let encrypted_credit = KagemushaEncryptedCreditEnvelopeV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        ephemeral_x25519_public_key: snapshot_digest(b"snapshot-mint-ephemeral-key", 5),
        nonce: [0x33; KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1],
        ciphertext_and_tag: vec![
            0x44;
            kagemusha_credit_opening_canonical_len_v1()
                .expect("fixed credit opening length")
                + KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1
        ],
    }
    .canonical_bytes_against_recipient_key(recipient_one_time_key)
    .expect("canonical snapshot-test mint envelope");
    let recipient_credential_commitment = kagemusha_recipient_credential_commitment_v1(
        operation_id,
        recipient_credential.credential_id,
        recipient_binding_opening,
    )
    .expect("snapshot-test recipient credential commitment");
    let credit_commitment = kagemusha_mint_credit_opening_commitment_v1(
        &state.lane.network_id,
        &state.lane.asset,
        state.asset_incarnation,
        state.lane.scale,
        state.liability_pool_id,
        amount,
        &recipient,
        recipient_one_time_key,
        credit_commitment_opening,
    )
    .expect("snapshot-test mint opening commitment");
    let context = KagemushaMintAuthorizationContextV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        operation_id,
        release_id: state.release_id,
        suite_id: state.suite_id,
        vk_digest: state.vk_digest,
        artifact_manifest_digest: artifacts.artifact_manifest_digest,
        network_id: state.lane.network_id,
        asset: state.lane.asset.clone(),
        asset_incarnation: state.asset_incarnation,
        scale: state.lane.scale,
        liability_pool_id: state.liability_pool_id,
        amount,
        payer: recipient.clone(),
        recipient: recipient.clone(),
        hardware_credential_id: recipient_credential.credential_id,
        hardware_profile_id: state.hardware_profile_id,
        policy_epoch: state.policy_epoch,
        recipient_credential_commitment,
        credit_commitment,
        recipient_one_time_key,
    };
    let issuance_commitment = snapshot_digest(b"snapshot-mint-issuance", 6);
    let provisional_statement = KagemushaMintCreditStatementV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        lifecycle: KagemushaLifecycleBindingV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            network_id: state.lane.network_id,
            protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
            suite_id: state.suite_id,
            vk_digest: state.vk_digest,
            release_id: state.release_id,
            asset: state.lane.asset.clone(),
            asset_incarnation: state.asset_incarnation,
            scale: state.lane.scale,
            liability_pool_id: state.liability_pool_id,
            hardware_profile_id: state.hardware_profile_id,
            policy_epoch: state.policy_epoch,
            operation_kind: KagemushaOperationKindV1::MintFold,
            request_id: [0; 32],
            receiver_lane_commitment: [0; 32],
            credit_id: [0; 32],
            ciphertext_digest: kagemusha_ciphertext_digest_v1(&encrypted_credit),
        },
        recipient_credential_commitment,
        authorization_context_digest: context
            .canonical_digest()
            .expect("snapshot-test mint context digest"),
        mint_authorization_digest: snapshot_digest(b"snapshot-provisional-authorization", 7),
        amount,
        issuance_commitment,
        recipient: recipient.clone(),
        credit_commitment,
        minted_at_ms: 100,
    }
    .seal_credit_id()
    .expect("snapshot-test mint credit ID");
    let credit_opening = KagemushaCreditOpeningV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        credit_id: provisional_statement.lifecycle.credit_id,
        amount,
        credit_commitment_opening,
        recipient_binding_opening,
        recovery_nonce: snapshot_digest(b"snapshot-mint-recovery", 8),
    };
    let authorization_statement = KagemushaMintAuthorizationStatementV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        context,
        issuance_commitment,
        credit_id: provisional_statement.lifecycle.credit_id,
        ciphertext_digest: provisional_statement.lifecycle.ciphertext_digest,
    };
    let authorization = KagemushaMintAuthorizationV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        proof: snapshot_paired_proof(
            authorization_statement
                .canonical_digest()
                .expect("snapshot-test authorization statement digest"),
            artifacts.mint_authorization_eq_protocol_digest,
            artifacts.mint_authorization_ep_protocol_digest,
            0x61,
        ),
        statement: authorization_statement,
    };
    let mut statement = provisional_statement;
    statement.mint_authorization_digest = authorization
        .canonical_digest()
        .expect("snapshot-test authorization digest");
    statement = statement
        .seal_credit_id()
        .expect("authorization-independent mint credit ID");
    let finality_certificate_binding = snapshot_digest(b"snapshot-finality-certificate", 9);
    let finality_authority_head = snapshot_digest(b"snapshot-finality-authority", 10);
    let mut proof = snapshot_paired_proof(
        statement
            .canonical_digest()
            .expect("snapshot-test mint statement digest"),
        artifacts.mint_finality_eq_protocol_digest,
        artifacts.mint_finality_ep_protocol_digest,
        0x71,
    );
    proof.guard_eq_credential_audit = finality_certificate_binding;
    proof.guard_ep_credential_audit = finality_authority_head;
    let credit = KagemushaMintCreditV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        proof,
        finality_certificate_binding,
        finality_authority_head,
        finality_genesis_roster_id: snapshot_digest(b"snapshot-finality-genesis", 11),
        finality_proof_binding_digest: snapshot_digest(b"snapshot-finality-proof", 12),
        statement,
        encrypted_credit,
        artifact_manifest_digest: artifacts.artifact_manifest_digest,
    };
    credit
        .validate_shape_against_authorization(&authorization)
        .expect("snapshot-test mint credit matches authorization");
    (authorization, credit, credit_opening)
}

fn snapshot_peer_payment(
    state: &KagemushaStateV1,
    artifacts: KagemushaRecursionArtifactsV1,
    recipient_credential: KagemushaHardwareCredentialV1,
    recipient_key: &SigningKey,
    amount: u128,
    identity: u64,
) -> (
    KagemushaPaymentRequestV1,
    KagemushaPaymentV1,
    KagemushaCreditOpeningV1,
    KagemushaAcknowledgementV1,
) {
    let fixture = crate::zk::kagemusha_v1_recursion::tests::incoming_payment_fixture(
        0x81, 0x82, 3, 5, 32, 32,
    );
    let mut request = fixture.request;
    request.release_id = state.release_id;
    request.network_id = state.lane.network_id;
    request.asset = state.lane.asset.clone();
    request.asset_incarnation = state.asset_incarnation;
    request.scale = state.lane.scale;
    request.liability_pool_id = state.liability_pool_id;
    request.amount = amount;
    request.request_id = snapshot_indexed_digest(b"snapshot-peer-request", identity);
    request.hardware_credential = recipient_credential;
    request.issued_at_ms = 100;
    request.expires_at_ms = 10_000;
    request.signature = snapshot_device_signature(recipient_key, b"unsealed request");
    request.signature = snapshot_device_signature(
        recipient_key,
        &request
            .canonical_signing_bytes()
            .expect("snapshot-test request signing bytes"),
    );
    request
        .validate_shape()
        .expect("valid snapshot-test receiver request");

    let request_digest = request
        .canonical_digest()
        .expect("snapshot-test request digest");
    let credit_commitment_opening = snapshot_digest(b"snapshot-peer-opening", 1);
    let recipient_binding_opening = snapshot_digest(b"snapshot-peer-recipient-opening", 2);
    let recovery_nonce = snapshot_digest(b"snapshot-peer-recovery", 3);
    let mut payment = fixture.payment;
    payment.output.request_digest = request_digest;
    payment.output.amount = amount;
    payment.output.ciphertext_commitment = kagemusha_peer_credit_opening_commitment_v1(
        request_digest,
        request.recipient_encryption_key,
        amount,
        credit_commitment_opening,
        recipient_binding_opening,
        recovery_nonce,
    )
    .expect("snapshot-test peer opening commitment");
    payment.output.credit_id = [0; 32];
    payment.output = payment
        .output
        .seal_credit_id_against(&request)
        .expect("snapshot-test peer credit ID");
    payment.proof.eq_protocol_digest = artifacts.commit_wrapper_eq_protocol_digest;
    payment.proof.ep_protocol_digest = artifacts.commit_wrapper_ep_protocol_digest;
    payment.proof.semantic_digest =
        kagemusha_payment_body_digest_v1(&payment.output, &payment.encrypted_credit)
            .expect("snapshot-test payment body digest");
    payment.proof.commit_certificate_digest = payment
        .commit_certificate
        .canonical_digest()
        .expect("snapshot-test commit certificate digest");
    payment
        .validate_shape_against(&request)
        .expect("valid snapshot-test payment envelope");
    let credit_opening = KagemushaCreditOpeningV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        credit_id: payment.output.credit_id,
        amount,
        credit_commitment_opening,
        recipient_binding_opening,
        recovery_nonce,
    };
    let inbox_receipt = KagemushaInboxReceiptV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        credit_id: payment.output.credit_id,
        receipt_commitment: snapshot_digest(b"snapshot-peer-receipt", 4),
    };
    let payment_digest = payment
        .canonical_digest_against(&request)
        .expect("snapshot-test payment digest");
    let acknowledgement_signing_bytes = kagemusha_acknowledgement_signing_bytes_v1(
        KAGEMUSHA_WIRE_VERSION_V1,
        request_digest,
        payment_digest,
        inbox_receipt,
    )
    .expect("snapshot-test acknowledgement signing bytes");
    let acknowledgement = KagemushaAcknowledgementV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        request_digest,
        payment_digest,
        inbox_receipt,
        signature: snapshot_device_signature(recipient_key, &acknowledgement_signing_bytes),
    };
    acknowledgement
        .validate_shape_against(&request, &payment)
        .expect("valid snapshot-test durable acknowledgement");
    (request, payment, credit_opening, acknowledgement)
}

fn snapshot_receive_fold_authorization(
    machine: &KagemushaStateMachineV1<
        AcceptSnapshotRecursiveVerifierV1,
        AcceptSnapshotGuardVerifierV1,
        KagemushaMemoryAuthenticatedHistoryStoreV1,
    >,
    preview: &PeerCreditFoldPreviewV1,
    artifacts: KagemushaRecursionArtifactsV1,
    device_key: &SigningKey,
    proof_tag: u8,
) -> TransitionAuthorizationV1 {
    let authorization = TransitionAuthorizationV1::new(
        HardwareTransitionCertificateV1 {
            statement: preview.transition.hardware_statement.clone(),
            guard_bundle: vec![0x95],
        },
        snapshot_paired_proof(
            preview.transition.transport_semantic_digest,
            artifacts.eq_protocol_digest,
            artifacts.ep_protocol_digest,
            proof_tag,
        ),
    );
    let signing_bytes = machine
        .receive_fold_history_root_selection_signing_bytes(preview)
        .expect("snapshot-test history root selection signing bytes");
    machine
        .authorize_receive_fold_history(
            preview,
            authorization,
            &snapshot_device_public_key(device_key),
            snapshot_device_signature(device_key, &signing_bytes),
        )
        .expect("snapshot-test authenticated receive-fold history")
}

#[test]
fn snapshot_restore_keeps_mixed_old_and_current_epoch_credits_spendable() {
    let artifacts = crate::zk::kagemusha_v1_recursion::tests::artifacts();
    let suite_id = snapshot_digest(b"snapshot-suite", 1);
    let vk_digest = snapshot_digest(b"snapshot-verifier-set", 2);
    let governance_key = SigningKey::from_bytes((&[8; 32]).into()).expect("governance key");
    let profile = snapshot_hardware_profile(suite_id, &governance_key);
    let enabled_profile = KagemushaEnabledProfileV1 {
        hardware_profile: profile,
        hardware_profile_id: profile.hardware_profile_id,
        suite_id,
        vk_digest,
        qualification_digest: snapshot_digest(b"snapshot-qualification-matrix", 3),
        policy_epoch: profile.policy_epoch,
        qualification_report: KagemushaEvidenceFileV1 {
            sha256: profile.qualification_report_digest,
            byte_len: 1,
        },
    };
    let proof_release =
        KagemushaStateProofReleaseV1::from_test_artifacts(artifacts, vec![enabled_profile])
            .expect("snapshot-test proof release");
    let payment_context =
        crate::zk::kagemusha_v1_recursion::tests::incoming_payment_fixture(1, 2, 3, 5, 32, 32)
            .request;
    let lane = KagemushaLaneIdV1 {
        network_id: payment_context.network_id,
        device_lane_id: snapshot_digest(b"snapshot-lane", 4),
        asset: payment_context.asset.clone(),
        scale: payment_context.scale,
    };
    let old_epoch = HardwareEpochV1 {
        generation: 7,
        epoch_id: snapshot_digest(b"snapshot-old-epoch", 5),
    };
    let current_epoch = HardwareEpochV1 {
        generation: 8,
        epoch_id: snapshot_digest(b"snapshot-current-epoch", 6),
    };
    let old_device_key = SigningKey::from_bytes((&[17; 32]).into()).expect("old device key");
    let current_device_key =
        SigningKey::from_bytes((&[18; 32]).into()).expect("current device key");
    let old_credential = snapshot_hardware_credential(
        lane.network_id,
        lane.device_lane_id,
        old_epoch,
        &profile,
        suite_id,
        &old_device_key,
        &governance_key,
    );
    let old_policy = DevicePolicyBindingV1 {
        device_key_reference: old_credential.device_key_reference,
        hardware_policy_id: snapshot_digest(b"snapshot-old-policy", 7),
    };
    let context = KagemushaStateContextV1 {
        protocol_version: KAGEMUSHA_STATE_VERSION_V1,
        suite_id,
        vk_digest,
        release_id: artifacts.release_id,
        asset_incarnation: payment_context.asset_incarnation,
        hardware_profile_id: profile.hardware_profile_id,
        policy_epoch: profile.policy_epoch,
    };
    let liability_pool_id = derive_liability_pool_id(&lane, payment_context.asset_incarnation)
        .expect("snapshot-test liability pool");
    let state = KagemushaStateV1::build(
        context,
        liability_pool_id,
        lane.clone(),
        0,
        0,
        old_epoch,
        old_policy,
        snapshot_digest(b"snapshot-old-state-nonce", 8),
        ExactConsumedCreditIndex::empty().root(),
    )
    .expect("snapshot-test old-epoch state");
    let authenticated_history = KagemushaStateAuthenticatedHistoryV1::open(
        KagemushaMemoryAuthenticatedHistoryStoreV1::new(8 * 1024 * 1024),
    )
    .expect("empty authenticated history");
    let mut machine = KagemushaStateMachineV1 {
        state,
        journal_revision: 0,
        inbox_revision: 0,
        pending_credits: BTreeMap::new(),
        accepted_recipient_bindings: BTreeSet::from([old_policy]),
        accepted_payment_receipts: BTreeMap::new(),
        mint_inbox: KagemushaMintInboxV1::default(),
        consumed_credits: ExactConsumedCreditIndex::empty(),
        authenticated_history,
        receiver_inbox_capacity: KagemushaReceiverInboxCapacityV1::new(32 * 1024 * 1024),
        sender_outbox_capacity: KagemushaSenderOutboxCapacityV1::new(8 * 1024 * 1024),
        outgoing_candidate_journal: KagemushaOutgoingCandidateJournalV1::default(),
        proof_release: proof_release.clone(),
        recursive_verifier: AcceptSnapshotRecursiveVerifierV1,
        guard_verifier: AcceptSnapshotGuardVerifierV1,
    };

    let mint_amount = 4;
    let (mint_authorization, mint_credit, mint_opening) = snapshot_mint_credit(
        machine.state(),
        artifacts,
        &old_credential,
        payment_context.recipient.clone(),
        mint_amount,
    );
    let recipient_key_handle_binding = snapshot_digest(b"snapshot-mint-key-handle", 9);
    let reserved_inbox_bytes = MintInboxReservationV1::required_reservation_bytes(
        &mint_authorization,
        &old_credential,
        &mint_opening,
        recipient_key_handle_binding,
    )
    .expect("snapshot-test mint reservation size");
    let mint_reservation = MintInboxReservationV1::new(
        mint_authorization.clone(),
        old_credential,
        mint_opening,
        recipient_key_handle_binding,
        reserved_inbox_bytes,
    )
    .expect("snapshot-test mint reservation");
    let reservation_statement = machine
        .preview_mint_reservation(&mint_reservation)
        .expect("preview old-epoch mint reservation");
    machine
        .reserve_mint_credit(
            &mint_reservation,
            &MintReservationCertificateV1 {
                statement: reservation_statement,
                guard_bundle: vec![0x91],
            },
        )
        .expect("reserve old-epoch mint inbox capacity");
    let verified_mint = VerifiedMintStageV1::for_tests(mint_reservation, mint_credit.clone())
        .expect("test backend verified exact mint inputs");
    let mint_stage_statement = machine
        .preview_stage_mint_credit(&verified_mint, 200)
        .expect("preview old-epoch mint stage");
    machine
        .stage_mint_credit(
            &mint_authorization,
            &mint_credit,
            Some(&verified_mint),
            Some(&MintStageCertificateV1 {
                statement: mint_stage_statement,
                guard_bundle: vec![0x92],
            }),
        )
        .expect("stage finalized old-epoch mint");
    let mint_credit_id = CreditIdV1(mint_credit.statement.lifecycle.credit_id);
    assert_eq!(machine.inbox_revision(), 2);

    let current_credential = snapshot_hardware_credential(
        lane.network_id,
        lane.device_lane_id,
        current_epoch,
        &profile,
        suite_id,
        &current_device_key,
        &governance_key,
    );
    let current_policy = DevicePolicyBindingV1 {
        device_key_reference: current_credential.device_key_reference,
        hardware_policy_id: snapshot_digest(b"snapshot-current-policy", 10),
    };
    machine.state = KagemushaStateV1::build(
        context,
        liability_pool_id,
        lane,
        0,
        0,
        current_epoch,
        current_policy,
        snapshot_digest(b"snapshot-current-state-nonce", 11),
        machine.consumed_credits.root(),
    )
    .expect("snapshot-test current-epoch state");
    machine.journal_revision = 0;
    machine.inbox_revision = 0;
    machine.accepted_recipient_bindings.insert(current_policy);

    let peer_amount = 3;
    let (request, payment, peer_opening, acknowledgement) = snapshot_peer_payment(
        machine.state(),
        artifacts,
        current_credential,
        &current_device_key,
        peer_amount,
        1,
    );
    let peer_stage_statement = machine
        .preview_stage_payment(&request, &payment, &peer_opening, 300)
        .expect("preview current-epoch peer stage");
    machine
        .stage_payment(
            request,
            payment.clone(),
            peer_opening,
            300,
            Some(PaymentStageAuthorizationV1 {
                stage_certificate: CreditStageCertificateV1 {
                    statement: peer_stage_statement,
                    guard_bundle: vec![0x93],
                },
                acknowledgement,
            }),
        )
        .expect("stage current-epoch peer credit");
    let peer_credit_id = CreditIdV1(payment.output.credit_id);
    assert_eq!(machine.inbox_revision(), 1);

    let snapshot = machine.snapshot().expect("canonical recovery snapshot");
    let anchor = machine
        .seal_durability_anchor(vec![0x94])
        .expect("hardware-sealed recovery anchor");
    let canonical = norito::encode_canonical(&snapshot).expect("encode canonical state snapshot");
    let decoded: KagemushaStateSnapshotV1 =
        norito::decode_from_bytes(&canonical).expect("decode canonical state snapshot");
    assert_eq!(decoded, snapshot);
    assert_eq!(
        norito::encode_canonical(&decoded).expect("re-encode canonical state snapshot"),
        canonical
    );
    assert_eq!(decoded.mint_inbox.pending_count(), 1);
    assert_eq!(decoded.pending_credits.len(), 1);
    assert_eq!(
        decoded
            .mint_inbox
            .pending_values()
            .next()
            .expect("restored mint record")
            .stage_certificate()
            .statement
            .hardware_epoch,
        old_epoch
    );
    assert_eq!(
        decoded.pending_credits[0]
            .stage_certificate
            .statement
            .receiver_hardware_epoch,
        current_epoch
    );

    #[cfg(unix)]
    {
        // The existing snapshot fixture isolates restoration from recursive proving. The disk
        // boundary reuses the same verifiers; this test does not qualify production proof keys.
        let parent = tempfile::tempdir().expect("disk history parent");
        let path = parent
            .path()
            .canonicalize()
            .expect("physical parent")
            .join("history");
        let credentials = KagemushaHistoryDeviceCredentialsV1::new(
            profile.hardware_profile_id,
            [
                (
                    old_epoch.generation,
                    snapshot_device_public_key(&old_device_key),
                ),
                (
                    current_epoch.generation,
                    snapshot_device_public_key(&current_device_key),
                ),
            ],
        )
        .expect("pinned test hardware epochs");
        let binding = disk_history_lane_binding(decoded.state.context(), &decoded.state.lane)
            .expect("exact lane binding");
        drop(
            KagemushaDiskAuthenticatedHistoryStoreV1::create_new(
                &path,
                binding,
                credentials.clone(),
                8 * 1024 * 1024,
            )
            .expect("durable empty authenticated history"),
        );
        let mut stale_anchor = anchor.clone();
        stale_anchor.statement.journal_revision += 1;
        assert!(matches!(
            KagemushaStateMachineV1::restore_from_disk_history(
                decoded.clone(),
                &stale_anchor,
                proof_release.clone(),
                &path,
                credentials.clone(),
                8 * 1024 * 1024,
                AcceptSnapshotRecursiveVerifierV1,
                AcceptSnapshotGuardVerifierV1,
            ),
            Err(KagemushaStateErrorV1::SnapshotRollback)
        ));
        assert!(matches!(
            KagemushaStateMachineV1::restore_from_disk_history(
                decoded.clone(),
                &anchor,
                proof_release.clone(),
                &path,
                credentials.clone(),
                8 * 1024 * 1024,
                AcceptSnapshotRecursiveVerifierV1,
                RejectAllKagemushaGuardBundleVerifierV1,
            ),
            Err(KagemushaStateErrorV1::GuardRejected(_))
        ));
        let disk_restored = KagemushaStateMachineV1::restore_from_disk_history(
            decoded.clone(),
            &anchor,
            proof_release.clone(),
            &path,
            credentials,
            8 * 1024 * 1024,
            AcceptSnapshotRecursiveVerifierV1,
            AcceptSnapshotGuardVerifierV1,
        )
        .expect("restore through concrete disk history and existing snapshot checks");
        assert_eq!(disk_restored.snapshot().expect("disk snapshot"), decoded);
    }

    let restored = KagemushaStateMachineV1::restore(
        decoded,
        &anchor,
        proof_release,
        KagemushaMemoryAuthenticatedHistoryStoreV1::new(8 * 1024 * 1024),
        AcceptSnapshotRecursiveVerifierV1,
        AcceptSnapshotGuardVerifierV1,
    )
    .expect("restore mixed pending credits from canonical bytes");
    let mut expected = vec![
        (
            mint_credit_id,
            mint_amount,
            PendingCreditFoldV1::Mint(mint_credit_id),
        ),
        (
            peer_credit_id,
            peer_amount,
            PendingCreditFoldV1::Receive(peer_credit_id),
        ),
    ];
    expected.sort_by_key(|(credit_id, _, _)| *credit_id);
    let expected_folds = expected
        .iter()
        .map(|(_, _, fold)| *fold)
        .collect::<Vec<_>>();
    let watermark = restored.pending_credit_watermark();
    assert_eq!(watermark.hardware_epoch(), current_epoch);
    assert_eq!(watermark.inbox_revision(), 1);
    assert_eq!(
        restored
            .next_pending_fold_through(watermark)
            .expect("select deterministic mixed fold after restart"),
        expected_folds.first().copied()
    );
    assert_eq!(
        restored
            .next_pending_fold_required_for_amount_through(watermark, 0)
            .expect("zero target is already covered"),
        None
    );
    assert_eq!(
        restored
            .next_pending_fold_required_for_amount_through(watermark, 1)
            .expect("positive target requires the first deterministic fold"),
        expected_folds.first().copied()
    );
    assert_eq!(
        restored
            .pending_fold_plan_required_for_amount(expected[0].1)
            .expect("the first sorted credit covers its exact target"),
        expected_folds[..1]
    );
    assert_eq!(
        restored
            .pending_fold_plan_required_for_amount(mint_amount + peer_amount)
            .expect("both restored credits cover the combined target"),
        expected_folds
    );
    assert_eq!(
        restored.pending_fold_plan_required_for_amount(mint_amount + peer_amount + 1),
        Err(KagemushaStateErrorV1::InsufficientBalance)
    );
}

#[test]
fn mock_recursive_verifier_one_thousand_credits_form_one_sendable_redeemable_aggregate() {
    // This test exercises the production state machine, durable staging, exact replay inserts,
    // canonical recovery, and terminal candidate derivation. The explicitly named test verifier
    // accepts structural paired proofs; real Halo2 recursion is qualified separately.
    const CREDIT_COUNT: u64 = 1_000;
    let artifacts = crate::zk::kagemusha_v1_recursion::tests::artifacts();
    let suite_id = snapshot_digest(b"aggregate-suite", 1);
    let vk_digest = snapshot_digest(b"aggregate-verifier-set", 2);
    let governance_key = SigningKey::from_bytes((&[28; 32]).into()).expect("governance key");
    let device_key = SigningKey::from_bytes((&[29; 32]).into()).expect("device key");
    let profile = snapshot_hardware_profile(suite_id, &governance_key);
    let enabled_profile = KagemushaEnabledProfileV1 {
        hardware_profile: profile,
        hardware_profile_id: profile.hardware_profile_id,
        suite_id,
        vk_digest,
        qualification_digest: snapshot_digest(b"aggregate-qualification-matrix", 3),
        policy_epoch: profile.policy_epoch,
        qualification_report: KagemushaEvidenceFileV1 {
            sha256: profile.qualification_report_digest,
            byte_len: 1,
        },
    };
    let proof_release =
        KagemushaStateProofReleaseV1::from_test_artifacts(artifacts, vec![enabled_profile])
            .expect("aggregate-test proof release");
    let payment_context =
        crate::zk::kagemusha_v1_recursion::tests::incoming_payment_fixture(1, 2, 3, 5, 32, 32)
            .request;
    let lane = KagemushaLaneIdV1 {
        network_id: payment_context.network_id,
        device_lane_id: snapshot_digest(b"aggregate-lane", 4),
        asset: payment_context.asset.clone(),
        scale: payment_context.scale,
    };
    let hardware_epoch = HardwareEpochV1 {
        generation: 1,
        epoch_id: snapshot_digest(b"aggregate-epoch", 5),
    };
    let credential = snapshot_hardware_credential(
        lane.network_id,
        lane.device_lane_id,
        hardware_epoch,
        &profile,
        suite_id,
        &device_key,
        &governance_key,
    );
    let device_policy_binding = DevicePolicyBindingV1 {
        device_key_reference: credential.device_key_reference,
        hardware_policy_id: snapshot_digest(b"aggregate-policy", 6),
    };
    let context = KagemushaStateContextV1 {
        protocol_version: KAGEMUSHA_STATE_VERSION_V1,
        suite_id,
        vk_digest,
        release_id: artifacts.release_id,
        asset_incarnation: payment_context.asset_incarnation,
        hardware_profile_id: profile.hardware_profile_id,
        policy_epoch: profile.policy_epoch,
    };
    let liability_pool_id = derive_liability_pool_id(&lane, payment_context.asset_incarnation)
        .expect("aggregate-test liability pool");
    let state = KagemushaStateV1::build(
        context,
        liability_pool_id,
        lane,
        0,
        0,
        hardware_epoch,
        device_policy_binding,
        snapshot_digest(b"aggregate-state-nonce", 7),
        ExactConsumedCreditIndex::empty().root(),
    )
    .expect("aggregate-test zero state");
    let authenticated_history = KagemushaStateAuthenticatedHistoryV1::open(
        KagemushaMemoryAuthenticatedHistoryStoreV1::new(8 * 1024 * 1024),
    )
    .expect("empty aggregate authenticated history");
    let mut machine = KagemushaStateMachineV1 {
        state,
        journal_revision: 0,
        inbox_revision: 0,
        pending_credits: BTreeMap::new(),
        accepted_recipient_bindings: BTreeSet::from([device_policy_binding]),
        accepted_payment_receipts: BTreeMap::new(),
        mint_inbox: KagemushaMintInboxV1::default(),
        consumed_credits: ExactConsumedCreditIndex::empty(),
        authenticated_history,
        receiver_inbox_capacity: KagemushaReceiverInboxCapacityV1::new(64 * 1024 * 1024),
        sender_outbox_capacity: KagemushaSenderOutboxCapacityV1::new(8 * 1024 * 1024),
        outgoing_candidate_journal: KagemushaOutgoingCandidateJournalV1::default(),
        proof_release: proof_release.clone(),
        recursive_verifier: AcceptSnapshotRecursiveVerifierV1,
        guard_verifier: AcceptSnapshotGuardVerifierV1,
    };

    for index in 0..CREDIT_COUNT {
        let (request, payment, opening, acknowledgement) = snapshot_peer_payment(
            machine.state(),
            artifacts,
            credential.clone(),
            &device_key,
            1,
            index,
        );
        let stage_statement = machine
            .preview_stage_payment(&request, &payment, &opening, 20_000 + index)
            .expect("preview one distinct delayed peer credit");
        let credit_id = CreditIdV1(payment.output.credit_id);
        machine
            .stage_payment(
                request,
                payment,
                opening,
                20_000 + index,
                Some(PaymentStageAuthorizationV1 {
                    stage_certificate: CreditStageCertificateV1 {
                        statement: stage_statement,
                        guard_bundle: vec![0x96],
                    },
                    acknowledgement,
                }),
            )
            .expect("durably stage one distinct peer credit");
        let preview = machine
            .preview_receive_fold(
                credit_id,
                snapshot_indexed_digest(b"aggregate-successor-nonce", index),
                30_000 + index,
            )
            .expect("preview one exact replay-protected receive fold");
        let authorization = snapshot_receive_fold_authorization(
            &machine,
            &preview,
            artifacts,
            &device_key,
            (index as u8).wrapping_add(1),
        );
        let successor = machine
            .receive_fold_prepared(preview, authorization)
            .expect("install one authorized receive fold");
        assert_eq!(successor.balance, u128::from(index + 1));
    }

    assert_eq!(machine.state().balance, u128::from(CREDIT_COUNT));
    assert!(machine.pending_credits.is_empty());
    assert_eq!(machine.consumed_credits.len(), CREDIT_COUNT as usize);
    let history_store = machine.authenticated_history.clone().into_store();
    let snapshot = machine.snapshot().expect("aggregate recovery snapshot");
    let anchor = machine
        .seal_durability_anchor(vec![0x97])
        .expect("aggregate hardware-sealed recovery anchor");
    let canonical = norito::encode_canonical(&snapshot).expect("encode aggregate snapshot");
    let decoded: KagemushaStateSnapshotV1 =
        norito::decode_from_bytes(&canonical).expect("decode aggregate snapshot");
    assert_eq!(decoded, snapshot);
    assert_eq!(decoded.state.balance, u128::from(CREDIT_COUNT));
    assert!(decoded.pending_credits.is_empty());
    assert_eq!(decoded.consumed_credits.len(), CREDIT_COUNT as usize);

    let destination_key =
        SigningKey::from_bytes((&[30; 32]).into()).expect("destination device key");
    let destination_credential = snapshot_hardware_credential(
        machine.state().lane.network_id,
        snapshot_digest(b"aggregate-destination-lane", 8),
        HardwareEpochV1 {
            generation: 1,
            epoch_id: snapshot_digest(b"aggregate-destination-epoch", 9),
        },
        &profile,
        suite_id,
        &destination_key,
        &governance_key,
    );
    let (send_request, send_fixture, _, _) = snapshot_peer_payment(
        machine.state(),
        artifacts,
        destination_credential,
        &destination_key,
        u128::from(CREDIT_COUNT),
        CREDIT_COUNT,
    );
    let full_send = machine
        .prepare_send_split(SendSplitPreparationV1 {
            request: send_request,
            encrypted_credit: send_fixture.encrypted_credit,
            transition_nullifier: snapshot_digest(b"aggregate-send-nullifier", 10),
            ciphertext_commitment: send_fixture.output.ciphertext_commitment,
            successor_state_nonce_commitment: snapshot_digest(
                b"aggregate-send-successor-nonce",
                11,
            ),
            commit_evidence: KagemushaCommitEvidenceV1::TrustedTime(KagemushaTrustedCommitTimeV1 {
                time_evidence_commitment: snapshot_digest(b"aggregate-send-time", 12),
            }),
            commit_authorization_reference_ms: 500,
            outbox_reservation: KagemushaOutboxReservationV1 {
                reservation_id: snapshot_digest(b"aggregate-send-reservation", 13),
                operation_kind: KagemushaOperationKindV1::SendSplit,
                reserved_outbox_bytes: aggregate_outbox_reservation_bytes(
                    KagemushaOperationKindV1::SendSplit,
                ),
                issued_at_ms: 100,
                expires_at_ms: 10_000,
            },
            prepared_one_use_authorization_digest: snapshot_digest(b"aggregate-send-one-use", 14),
            sealed_transition_inputs: vec![0x98],
            sealed_recovery_seeds: vec![0x99],
        })
        .expect("derive one ordinary full-balance send");
    assert_eq!(full_send.private_state_link().0.balance, 1_000);
    assert_eq!(full_send.private_state_link().1.balance, 0);

    let restored = KagemushaStateMachineV1::restore(
        decoded,
        &anchor,
        proof_release,
        history_store,
        AcceptSnapshotRecursiveVerifierV1,
        AcceptSnapshotGuardVerifierV1,
    )
    .expect("restore the canonical aggregate with committed authenticated history");
    let redeem_preparation = |amount, tag| RedeemSplitPreparationV1 {
        amount,
        beneficiary: payment_context.recipient.clone(),
        terminal_nullifier: snapshot_digest(b"aggregate-redeem-nullifier", tag),
        redemption_commitment: snapshot_digest(b"aggregate-redeem-commitment", tag),
        successor_state_nonce_commitment: snapshot_digest(b"aggregate-redeem-successor-nonce", tag),
        commit_evidence: KagemushaCommitEvidenceV1::TrustedTime(KagemushaTrustedCommitTimeV1 {
            time_evidence_commitment: snapshot_digest(b"aggregate-redeem-time", tag),
        }),
        commit_authorization_reference_ms: 500,
        outbox_reservation: KagemushaOutboxReservationV1 {
            reservation_id: snapshot_digest(b"aggregate-redeem-reservation", tag),
            operation_kind: KagemushaOperationKindV1::RedeemSplit,
            reserved_outbox_bytes: aggregate_outbox_reservation_bytes(
                KagemushaOperationKindV1::RedeemSplit,
            ),
            issued_at_ms: 100,
            expires_at_ms: 10_000,
        },
        prepared_one_use_authorization_digest: snapshot_digest(b"aggregate-redeem-one-use", tag),
        sealed_transition_inputs: vec![tag],
        sealed_recovery_seeds: vec![tag.wrapping_add(1)],
    };
    let partial_redemption = restored
        .prepare_redeem_split(redeem_preparation(400, 15))
        .expect("derive a partial aggregate redemption");
    assert_eq!(partial_redemption.private_state_link().0.balance, 1_000);
    assert_eq!(partial_redemption.private_state_link().1.balance, 600);
    let full_redemption = restored
        .prepare_redeem_split(redeem_preparation(1_000, 16))
        .expect("derive a full aggregate redemption");
    assert_eq!(full_redemption.private_state_link().0.balance, 1_000);
    assert_eq!(full_redemption.private_state_link().1.balance, 0);
}

fn ordinary_hardware_transition() -> HardwareTransitionStatementV1 {
    let epoch = HardwareEpochV1 {
        generation: 7,
        epoch_id: [0x21; 32],
    };
    let policy = DevicePolicyBindingV1 {
        device_key_reference: [0x22; 32],
        hardware_policy_id: [0x23; 32],
    };
    HardwareTransitionStatementV1 {
        version: KAGEMUSHA_STATE_VERSION_V1,
        kind: KagemushaTransitionKindV1::SendSplit,
        amount: 5,
        lane: hardware_test_lane(),
        predecessor_commitment: [0x31; 32],
        successor_commitment: [0x32; 32],
        predecessor_sequence: 40,
        successor_sequence: 41,
        predecessor_epoch: epoch,
        successor_epoch: epoch,
        predecessor_device_policy_binding: policy,
        successor_device_policy_binding: policy,
        predecessor_state_nonce_commitment: [0x33; 32],
        successor_state_nonce_commitment: [0x34; 32],
        journal_revision_before: 70,
        journal_revision_after: 71,
        state_transition_digest: [0x35; 32],
        normalized_guard_statement_digest: [0x36; 32],
    }
}

fn rotation_hardware_transition() -> HardwareTransitionStatementV1 {
    let mut statement = ordinary_hardware_transition();
    statement.kind = KagemushaTransitionKindV1::Rotate;
    statement.amount = 0;
    statement.predecessor_sequence = u128::MAX;
    statement.successor_sequence = 0;
    statement.predecessor_epoch = HardwareEpochV1 {
        generation: 7,
        epoch_id: [0x21; 32],
    };
    statement.successor_epoch = HardwareEpochV1 {
        generation: 8,
        epoch_id: [0x41; 32],
    };
    statement.successor_device_policy_binding = DevicePolicyBindingV1 {
        device_key_reference: [0x42; 32],
        hardware_policy_id: [0x43; 32],
    };
    statement.journal_revision_before = u128::MAX;
    statement.journal_revision_after = 0;
    statement
}

#[test]
fn pending_prefix_has_no_protocol_count_ceiling() {
    let pending = (0_u32..4_096)
        .map(|index| {
            let mut id = [0_u8; 32];
            id[..4].copy_from_slice(&index.to_be_bytes());
            (CreditIdV1(id), 1_u128)
        })
        .collect::<BTreeMap<_, _>>();
    let selected =
        required_pending_credit_prefix(0, 4_096, pending).expect("all credits remain spendable");
    assert_eq!(selected.len(), 4_096);
}

#[test]
fn pending_prefix_checks_asset_arithmetic_overflow() {
    let pending = [(CreditIdV1([1; 32]), 1_u128)];
    assert_eq!(
        required_pending_credit_prefix(u128::MAX, u128::MAX, pending),
        Ok(Vec::new())
    );
}

#[test]
fn hardware_transition_requires_exact_next_and_fresh_successor_authority() {
    let statement = ordinary_hardware_transition();
    assert_eq!(statement.validate_exact_next(), Ok(()));

    let mut skipped_sequence = statement.clone();
    skipped_sequence.successor_sequence += 1;
    assert_eq!(
        skipped_sequence.validate_exact_next(),
        Err(KagemushaStateErrorV1::HardwareCertificateMismatch)
    );

    let mut skipped_journal = statement.clone();
    skipped_journal.journal_revision_after += 1;
    assert_eq!(
        skipped_journal.validate_exact_next(),
        Err(KagemushaStateErrorV1::HardwareCertificateMismatch)
    );

    let mut changed_epoch = statement.clone();
    changed_epoch.successor_epoch.epoch_id[0] ^= 1;
    assert_eq!(
        changed_epoch.validate_exact_next(),
        Err(KagemushaStateErrorV1::HardwareCertificateMismatch)
    );

    let mut changed_policy = statement.clone();
    changed_policy
        .successor_device_policy_binding
        .device_key_reference[0] ^= 1;
    assert_eq!(
        changed_policy.validate_exact_next(),
        Err(KagemushaStateErrorV1::HardwareCertificateMismatch)
    );

    let mut reused_nonce = statement.clone();
    reused_nonce.successor_state_nonce_commitment = reused_nonce.predecessor_state_nonce_commitment;
    assert_eq!(
        reused_nonce.validate_exact_next(),
        Err(KagemushaStateErrorV1::HardwareCertificateMismatch)
    );

    let mut zero_value_transition = statement;
    zero_value_transition.amount = 0;
    assert_eq!(
        zero_value_transition.validate_exact_next(),
        Err(KagemushaStateErrorV1::HardwareCertificateMismatch)
    );
}

#[test]
fn hardware_epoch_rotation_is_the_only_rollback_safe_counter_rollover() {
    let statement = rotation_hardware_transition();
    assert_eq!(statement.validate_exact_next(), Ok(()));

    let mut skipped_epoch = statement.clone();
    skipped_epoch.successor_epoch.generation += 1;
    assert_eq!(
        skipped_epoch.validate_exact_next(),
        Err(KagemushaStateErrorV1::InvalidHardwareRotation)
    );

    let mut reused_epoch = statement.clone();
    reused_epoch.successor_epoch.epoch_id = reused_epoch.predecessor_epoch.epoch_id;
    assert_eq!(
        reused_epoch.validate_exact_next(),
        Err(KagemushaStateErrorV1::InvalidHardwareRotation)
    );

    let mut reused_policy = statement.clone();
    reused_policy.successor_device_policy_binding = reused_policy.predecessor_device_policy_binding;
    assert_eq!(
        reused_policy.validate_exact_next(),
        Err(KagemushaStateErrorV1::InvalidHardwareRotation)
    );

    let mut epoch_overflow = statement;
    epoch_overflow.predecessor_epoch.generation = u128::MAX;
    epoch_overflow.successor_epoch.generation = u128::MAX;
    assert_eq!(
        epoch_overflow.validate_exact_next(),
        Err(KagemushaStateErrorV1::HardwareEpochOverflow)
    );
}

#[test]
fn ordinary_hardware_counters_fail_closed_at_u128_rollover() {
    let mut sequence_overflow = ordinary_hardware_transition();
    sequence_overflow.predecessor_sequence = u128::MAX;
    sequence_overflow.successor_sequence = u128::MAX;
    assert_eq!(
        sequence_overflow.validate_exact_next(),
        Err(KagemushaStateErrorV1::SequenceOverflow)
    );

    let mut journal_overflow = ordinary_hardware_transition();
    journal_overflow.journal_revision_before = u128::MAX;
    journal_overflow.journal_revision_after = u128::MAX;
    assert_eq!(
        journal_overflow.validate_exact_next(),
        Err(KagemushaStateErrorV1::JournalRevisionOverflow)
    );
}

#[path = "history_mint_attempt_tests.rs"]
mod history_mint_attempt_tests;

fn aggregate_outbox_reservation_bytes(operation_kind: KagemushaOperationKindV1) -> u32 {
    u32::try_from(
        candidate_lifecycle::implementation_live_outbox_slot_bytes_v1(operation_kind)
            .expect("terminal operation has a complete Core recovery slot"),
    )
    .expect("Core slot fits hardware reservation field")
}

#[test]
fn sender_reservation_uses_complete_core_slot_not_only_wire_envelope_floor() {
    for (operation_kind, wire_floor) in [
        (
            KagemushaOperationKindV1::SendSplit,
            KAGEMUSHA_PAYMENT_OUTBOX_MIN_BYTES_V1,
        ),
        (
            KagemushaOperationKindV1::RedeemSplit,
            KAGEMUSHA_REDEMPTION_OUTBOX_MIN_BYTES_V1,
        ),
    ] {
        let mut capacity = KagemushaSenderOutboxCapacityV1::new(8 * 1024 * 1024);
        let journal = KagemushaOutgoingCandidateJournalV1::default();
        let mut reservation = KagemushaOutboxReservationV1 {
            reservation_id: snapshot_digest(b"complete-core-slot", 1),
            operation_kind,
            reserved_outbox_bytes: wire_floor,
            issued_at_ms: 100,
            expires_at_ms: 10_000,
        };
        assert!(
            reservation.validate().is_ok(),
            "wire framing floor is independently valid"
        );
        assert_eq!(
            capacity.reserve(reservation, &journal),
            Err(KagemushaStateErrorV1::SenderOutboxCapacityExhausted)
        );
        assert_eq!(capacity.committed_outbox_bytes(), 0);
        reservation.reserved_outbox_bytes = aggregate_outbox_reservation_bytes(operation_kind);
        assert_eq!(
            capacity.reserve(reservation, &journal),
            Ok(SenderOutboxReservationOutcomeV1::Reserved)
        );
        assert!(capacity.committed_outbox_bytes() < capacity.total_outbox_bytes());
    }
}

#[cfg(unix)]
#[path = "coordinator_operation_store_tests.rs"]
mod coordinator_operation_store_tests;
