//! Aggregate Offline Cash V1 state-machine tests.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use halo2_proofs::halo2curves::{
    group::{Curve as _, Group as _, prime::PrimeCurveAffine as _},
    pasta::{Ep, EpAffine, Eq, EqAffine, Fp, Fq},
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    block::BlockHeader,
    domain::DomainId,
    nexus::AxtAssetIncarnationV1,
    offline::{
        OFFLINE_CASH_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1,
        OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1, OFFLINE_CASH_OUTBOX_RETRY_METADATA_MAX_BYTES_V1,
        OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1, OFFLINE_CASH_WIRE_VERSION_V1,
        OFFLINE_CASH_XCHACHA20POLY1305_NONCE_BYTES_V1, OFFLINE_CASH_XCHACHA20POLY1305_TAG_BYTES_V1,
        OfflineCashAcceptanceIntentV1, OfflineCashAcceptanceTicketV1, OfflineCashAcknowledgementV1,
        OfflineCashArtifactBindingV1, OfflineCashArtifactRoleV1, OfflineCashCommitCertificateV1,
        OfflineCashCommitEvidenceV1, OfflineCashCommitWrapperProofV1, OfflineCashDevicePublicKeyV1,
        OfflineCashDeviceSignatureV1, OfflineCashEncryptedCreditEnvelopeV1,
        OfflineCashHardwareCredentialV1, OfflineCashInboxReceiptV1, OfflineCashLifecycleBindingV1,
        OfflineCashOperationKindV1, OfflineCashOutboxReservationV1, OfflineCashPairedProofV1,
        OfflineCashPaymentRequestV1, OfflineCashPaymentV1, OfflineCashRedemptionStatementV1,
        OfflineCashTransferStatementV1, OfflineCashTrustedCommitTimeV1,
        offline_cash_ciphertext_digest_v1, offline_cash_credit_opening_canonical_len_v1,
        offline_cash_device_key_reference_v1, offline_cash_inbox_receipt_commitment_v1,
        offline_cash_liability_pool_id_v1,
    },
};
use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};
use snark_verifier::{loader::native::NativeLoader, pcs::ipa::IpaAccumulator};

use super::{sparse_merkle::ExactConsumedCreditIndex, *};
use crate::zk::offline_cash_v1_recursion::{
    OFFLINE_CASH_RECURSION_IPA_K_V1, OfflineCashEpAccumulatorV1, OfflineCashEqAccumulatorV1,
    OfflineCashMintFinalityArtifactsV1, OfflineCashMintFinalityHelperVerificationRequestV1,
    OfflineCashParityVerificationRequestV1, OfflineCashRecursionArtifactsV1,
    OfflineCashStateProofVerificationRequestV1, canonical_commit_certificate_digest_v1,
};

fn digest(tag: u8) -> DigestV1 {
    [tag; 32]
}

fn indexed_digest(tag: u8, index: u64) -> DigestV1 {
    let mut digest = [tag; 32];
    digest[24..].copy_from_slice(&index.to_be_bytes());
    digest
}

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"offline-cash-v1-state-tests",
    )))
}

fn asset() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "xor".parse().expect("asset name"),
    )
}

fn asset_incarnation() -> AxtAssetIncarnationV1 {
    let network = network();
    let asset = asset();
    AxtAssetIncarnationV1::derive(
        &network,
        &asset,
        &HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"offline-cash-v1-state-registration",
        )),
        &Hash::new(b"offline-cash-v1-state-execution"),
        1,
    )
}

fn account(tag: u8) -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![tag; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

fn lane() -> OfflineCashLaneIdV1 {
    OfflineCashLaneIdV1 {
        network_id: network(),
        device_lane_id: digest(0x32),
        asset: asset(),
        scale: 4,
    }
}

fn state_context() -> OfflineCashStateContextV1 {
    OfflineCashStateContextV1 {
        protocol_version: OFFLINE_CASH_STATE_VERSION_V1,
        suite_id: digest(0x23),
        vk_digest: digest(0x24),
        release_id: artifacts().release_id,
        asset_incarnation: asset_incarnation(),
        hardware_profile_id: digest(0x25),
        policy_epoch: 1,
    }
}

fn epoch() -> HardwareEpochV1 {
    HardwareEpochV1 {
        generation: 1,
        epoch_id: digest(0x33),
    }
}

fn device_binding() -> DevicePolicyBindingV1 {
    DevicePolicyBindingV1 {
        device_key_reference: digest(0x34),
        hardware_policy_id: digest(0x35),
    }
}

fn artifact(role: OfflineCashArtifactRoleV1, tag: u8) -> OfflineCashArtifactBindingV1 {
    OfflineCashArtifactBindingV1 {
        role,
        sha256: digest(tag),
        byte_len: 1_024,
    }
}

fn artifacts() -> OfflineCashRecursionArtifactsV1 {
    OfflineCashRecursionArtifactsV1 {
        release_id: digest(0x43),
        profile_digest: digest(0x44),
        eq_protocol_digest: crate::zk::offline_cash_v1_poseidon::encode(Fp::from(0x15_u64)),
        ep_protocol_digest: crate::zk::offline_cash_v1_poseidon::encode(Fq::from(0x16_u64)),
        commit_wrapper_eq_protocol_digest: crate::zk::offline_cash_v1_poseidon::encode(Fp::from(
            0x17_u64,
        )),
        commit_wrapper_ep_protocol_digest: crate::zk::offline_cash_v1_poseidon::encode(Fq::from(
            0x18_u64,
        )),
        mint_authorization_eq_protocol_digest: crate::zk::offline_cash_v1_poseidon::encode(
            Fp::from(0x19_u64),
        ),
        mint_authorization_ep_protocol_digest: crate::zk::offline_cash_v1_poseidon::encode(
            Fq::from(0x1A_u64),
        ),
        guard_bundle_eq_protocol_digest: crate::zk::offline_cash_v1_poseidon::encode(Fp::from(
            0x1B_u64,
        )),
        guard_bundle_ep_protocol_digest: crate::zk::offline_cash_v1_poseidon::encode(Fq::from(
            0x1C_u64,
        )),
        guard_bundle_verifying_key_eq: artifact(OfflineCashArtifactRoleV1::GuardBundleVkEq, 0x50),
        guard_bundle_verifying_key_ep: artifact(OfflineCashArtifactRoleV1::GuardBundleVkEp, 0x51),
        commit_wrapper_verifying_key_eq: artifact(
            OfflineCashArtifactRoleV1::CommitWrapperVkEq,
            0x52,
        ),
        commit_wrapper_verifying_key_ep: artifact(
            OfflineCashArtifactRoleV1::CommitWrapperVkEp,
            0x53,
        ),
        mint_finality: OfflineCashMintFinalityArtifactsV1 {
            proving_key_eq: artifact(OfflineCashArtifactRoleV1::MintCreditPkEq, 0x54),
            verifying_key_eq: artifact(OfflineCashArtifactRoleV1::MintCreditVkEq, 0x55),
            proving_key_ep: artifact(OfflineCashArtifactRoleV1::MintCreditPkEp, 0x56),
            verifying_key_ep: artifact(OfflineCashArtifactRoleV1::MintCreditVkEp, 0x57),
        },
        artifact_manifest_digest: digest(0x58),
        canonical_empty_effect_digest: digest(0x59),
    }
}

fn eq_history() -> OfflineCashEqAccumulatorV1 {
    let challenges = (0..OFFLINE_CASH_RECURSION_IPA_K_V1)
        .map(|round| Fp::from(u64::from(round) + 1))
        .collect();
    let point = (Eq::generator() * Fp::from(97)).to_affine();
    OfflineCashEqAccumulatorV1::from_native(&IpaAccumulator::<EqAffine, NativeLoader>::new(
        challenges, point,
    ))
    .expect("canonical Eq history")
}

fn ep_history() -> OfflineCashEpAccumulatorV1 {
    let challenges = (0..OFFLINE_CASH_RECURSION_IPA_K_V1)
        .map(|round| Fq::from(u64::from(round) + 1))
        .collect();
    let point = (Ep::generator() * Fq::from(193)).to_affine();
    OfflineCashEpAccumulatorV1::from_native(&IpaAccumulator::<EpAffine, NativeLoader>::new(
        challenges, point,
    ))
    .expect("canonical Ep history")
}

fn eq_history_bytes() -> Vec<u8> {
    static HISTORY: OnceLock<Vec<u8>> = OnceLock::new();
    HISTORY
        .get_or_init(|| eq_history().as_bytes().to_vec())
        .clone()
}

fn ep_history_bytes() -> Vec<u8> {
    static HISTORY: OnceLock<Vec<u8>> = OnceLock::new();
    HISTORY
        .get_or_init(|| ep_history().as_bytes().to_vec())
        .clone()
}

#[derive(Clone, Copy, Debug, Default)]
struct AcceptingRecursiveVerifier;

impl OfflineCashRecursiveVerifierV1 for AcceptingRecursiveVerifier {
    fn verify_state_proof_and_decide(
        &self,
        _request: &OfflineCashStateProofVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn verify_mint_finality_helper(
        &self,
        _request: &OfflineCashMintFinalityHelperVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Ok(())
    }

    fn verify_commit_wrapper_and_decide(
        &self,
        _request: &OfflineCashParityVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Ok(())
    }
}

impl OfflineCashCandidateProofVerifierV1 for AcceptingRecursiveVerifier {
    fn verify_candidate_proof(
        &self,
        _candidate: &PreparedOutgoingCandidateV1,
        _proof: &OfflineCashPairedProofV1,
    ) -> Result<(), String> {
        Ok(())
    }
}

impl OfflineCashCommitWrapperVerifierV1 for AcceptingRecursiveVerifier {
    fn verify_commit_wrapper(
        &self,
        _public_inputs: &OfflineCashCommitWrapperPublicInputsV1,
        _proof: &OfflineCashCommitWrapperProofV1,
    ) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct AcceptingGuardVerifier;

impl OfflineCashGuardBundleVerifierV1 for AcceptingGuardVerifier {
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

#[derive(Clone, Copy, Debug, Default)]
struct RejectingTransitionGuardVerifier;

impl OfflineCashGuardBundleVerifierV1 for RejectingTransitionGuardVerifier {
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
        Err("rejected test transition".to_owned())
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

type TestMachine = OfflineCashStateMachineV1<AcceptingRecursiveVerifier, AcceptingGuardVerifier>;

fn machine(balance: u128) -> TestMachine {
    let lane = lane();
    let context = state_context();
    let consumed_credits = ExactConsumedCreditIndex::empty();
    let state = OfflineCashStateV1::build(
        context,
        offline_cash_liability_pool_id_v1(&lane.network_id, &lane.asset, context.asset_incarnation)
            .expect("liability pool"),
        lane,
        balance,
        0,
        epoch(),
        device_binding(),
        digest(0x60),
        consumed_credits.root(),
    )
    .expect("aggregate state");
    OfflineCashStateMachineV1 {
        state,
        journal_revision: 0,
        pending_credits: BTreeMap::new(),
        accepted_recipient_bindings: BTreeSet::from([device_binding()]),
        accepted_payment_receipts: BTreeMap::new(),
        consumed_credits,
        acceptance_ticket_book: OfflineCashAcceptanceTicketBookV1::new(64 * 1024 * 1024),
        sender_outbox_capacity: OfflineCashSenderOutboxCapacityV1::new(
            u64::from(
                iroha_data_model::offline::OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1
                    .max(iroha_data_model::offline::OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1),
            ) + 1_048_576,
        ),
        outgoing_candidate_journal: OfflineCashOutgoingCandidateJournalV1::default(),
        proof_release: OfflineCashStateProofReleaseV1::from_test_artifacts(artifacts()),
        recursive_verifier: AcceptingRecursiveVerifier,
        guard_verifier: AcceptingGuardVerifier,
    }
}

fn rejecting_transition_machine(
    balance: u128,
) -> OfflineCashStateMachineV1<AcceptingRecursiveVerifier, RejectingTransitionGuardVerifier> {
    let machine = machine(balance);
    OfflineCashStateMachineV1 {
        state: machine.state,
        journal_revision: machine.journal_revision,
        pending_credits: machine.pending_credits,
        accepted_recipient_bindings: machine.accepted_recipient_bindings,
        accepted_payment_receipts: machine.accepted_payment_receipts,
        consumed_credits: machine.consumed_credits,
        acceptance_ticket_book: machine.acceptance_ticket_book,
        sender_outbox_capacity: machine.sender_outbox_capacity,
        outgoing_candidate_journal: machine.outgoing_candidate_journal,
        proof_release: machine.proof_release,
        recursive_verifier: machine.recursive_verifier,
        guard_verifier: RejectingTransitionGuardVerifier,
    }
}

fn signing_key(tag: u8) -> SigningKey {
    SigningKey::from_bytes((&[tag; 32]).into()).expect("P-256 signing key")
}

fn device_public_key(key: &SigningKey) -> OfflineCashDevicePublicKeyV1 {
    OfflineCashDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .expect("device key")
}

fn sign(key: &SigningKey, bytes: &[u8]) -> OfflineCashDeviceSignatureV1 {
    let signature: P256Signature = key.sign(bytes);
    let signature = signature.normalize_s().unwrap_or(signature);
    OfflineCashDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
        .expect("device signature")
}

fn encrypted_credit_stub(recipient_one_time_key: [u8; 32], index: u64) -> Vec<u8> {
    let mut ephemeral_x25519_public_key = [0; 32];
    ephemeral_x25519_public_key[0] = 9;
    let mut nonce = [0x69; OFFLINE_CASH_XCHACHA20POLY1305_NONCE_BYTES_V1];
    nonce[OFFLINE_CASH_XCHACHA20POLY1305_NONCE_BYTES_V1 - core::mem::size_of::<u64>()..]
        .copy_from_slice(&index.to_be_bytes());
    OfflineCashEncryptedCreditEnvelopeV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        ephemeral_x25519_public_key,
        nonce,
        ciphertext_and_tag: vec![
            u8::try_from(index & 0xFF).expect("masked byte");
            offline_cash_credit_opening_canonical_len_v1()
                .expect("opening length")
                + OFFLINE_CASH_XCHACHA20POLY1305_TAG_BYTES_V1
        ],
    }
    .canonical_bytes_against_recipient_key(recipient_one_time_key)
    .expect("canonical encrypted credit")
}

fn payment_stub(
    state: &OfflineCashStateV1,
    index: u64,
) -> (OfflineCashPaymentRequestV1, OfflineCashPaymentV1) {
    payment_stub_with_amount(state, index, 1)
}

fn payment_stub_with_amount(
    state: &OfflineCashStateV1,
    index: u64,
    amount: u128,
) -> (OfflineCashPaymentRequestV1, OfflineCashPaymentV1) {
    assert!(amount > 0, "payment stub amount must be positive");
    let key = signing_key(7);
    let public_key = device_public_key(&key);
    let signature = sign(&key, b"offline-cash-v1-state-test");
    let request_id = indexed_digest(0x70, index);
    let ticket_id = indexed_digest(0x71, index);
    let recipient_one_time_key = {
        let mut key = [0; 32];
        key[0] = 9;
        key
    };
    let hardware_credential = OfflineCashHardwareCredentialV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        credential_id: [0; 32],
        network_id: state.lane.network_id,
        hardware_profile_id: state.hardware_profile_id,
        suite_id: state.suite_id,
        firmware_policy_digest: digest(0x76),
        policy_epoch: state.policy_epoch,
        lane_commitment: state.lane.device_lane_id,
        hardware_epoch_id: state.hardware_epoch.epoch_id,
        hardware_epoch_generation: 1,
        device_public_key: public_key,
        device_key_reference: offline_cash_device_key_reference_v1(&public_key),
        issued_at_ms: 1,
        expires_at_ms: 10_001,
        governance_signature: signature,
    }
    .seal_credential_id()
    .expect("credential id");
    let mut request = OfflineCashPaymentRequestV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: state.release_id,
        network_id: state.lane.network_id,
        asset: state.lane.asset.clone(),
        asset_incarnation: state.asset_incarnation,
        scale: state.lane.scale,
        liability_pool_id: state.liability_pool_id,
        recipient: account(0xA5),
        amount,
        hardware_credential,
        request_id,
        issued_at_ms: 1,
        expires_at_ms: 10_000,
        signature,
    };
    request.signature = sign(
        &key,
        &request
            .canonical_signing_bytes()
            .expect("request signing bytes"),
    );
    let request_digest = request.canonical_digest().expect("request digest");
    let intent = OfflineCashAcceptanceIntentV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        request_digest,
        intent_id: indexed_digest(0x74, index),
        exact_amount: amount,
        sender_one_time_commitment: indexed_digest(0x77, index),
    };
    let intent_digest = intent
        .canonical_digest_against(&request)
        .expect("intent digest");
    let mut ticket = OfflineCashAcceptanceTicketV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        network_id: state.lane.network_id,
        request_id,
        request_digest,
        acceptance_ticket_id: ticket_id,
        asset: state.lane.asset.clone(),
        asset_incarnation: state.asset_incarnation,
        scale: state.lane.scale,
        intent_digest,
        exact_amount: amount,
        reserved_inbox_bytes: OFFLINE_CASH_ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES_V1,
        recipient_one_time_key,
        hardware_profile_id: state.hardware_profile_id,
        policy_epoch: state.policy_epoch,
        issued_at_ms: 1,
        expires_at_ms: 10_000,
        signature,
    };
    ticket.signature = sign(
        &key,
        &ticket
            .canonical_signing_bytes()
            .expect("ticket signing bytes"),
    );
    let ticket_digest = ticket
        .canonical_digest_against(&request, &intent)
        .expect("ticket digest");
    let encrypted_credit = encrypted_credit_stub(recipient_one_time_key, index);
    let commit_evidence =
        OfflineCashCommitEvidenceV1::TrustedTime(OfflineCashTrustedCommitTimeV1 {
            time_evidence_commitment: indexed_digest(0x79, index),
        });
    let statement = OfflineCashTransferStatementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        lifecycle: OfflineCashLifecycleBindingV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            network_id: state.lane.network_id,
            protocol_version: state.protocol_version,
            suite_id: state.suite_id,
            vk_digest: state.vk_digest,
            release_id: state.release_id,
            asset: state.lane.asset.clone(),
            asset_incarnation: state.asset_incarnation,
            scale: state.lane.scale,
            liability_pool_id: state.liability_pool_id,
            hardware_profile_id: state.hardware_profile_id,
            policy_epoch: state.policy_epoch,
            operation_kind: OfflineCashOperationKindV1::SendSplit,
            request_id,
            acceptance_ticket_id: ticket_id,
            credit_id: [0; 32],
            ciphertext_digest: offline_cash_ciphertext_digest_v1(&encrypted_credit),
        },
        amount,
        transition_nullifier: indexed_digest(0x7B, index),
        request_digest,
        acceptance_ticket_digest: ticket_digest,
        recipient_one_time_key,
        ciphertext_commitment: indexed_digest(0x7C, index),
        commit_evidence,
    }
    .seal_credit_id()
    .expect("credit id");
    let semantic_digest = statement.canonical_digest().expect("send statement");
    let candidate_envelope_digest = indexed_digest(0x7D, index);
    let commit_certificate = OfflineCashCommitCertificateV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        certificate_id: [0; 32],
        candidate_envelope_digest,
        lifecycle_binding_digest: statement
            .lifecycle
            .canonical_digest()
            .expect("lifecycle digest"),
        transition_nullifier: statement.transition_nullifier,
        outbox_reservation_commitment: indexed_digest(0x80, index),
        commit_evidence,
        hardware_profile_id: state.hardware_profile_id,
        policy_epoch: state.policy_epoch,
        hardware_terminal_commitment: indexed_digest(0x81, index),
    }
    .seal_certificate_id()
    .expect("terminal certificate");
    let commit_certificate_digest =
        canonical_commit_certificate_digest_v1(&commit_certificate).expect("certificate digest");
    let payment = OfflineCashPaymentV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        statement,
        acceptance_intent: intent,
        acceptance_ticket: ticket,
        commit_certificate,
        proof: OfflineCashCommitWrapperProofV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            eq_protocol_digest: artifacts().commit_wrapper_eq_protocol_digest,
            ep_protocol_digest: artifacts().commit_wrapper_ep_protocol_digest,
            semantic_digest,
            candidate_envelope_digest,
            commit_certificate_digest,
            eq_deferred_audit: crate::zk::offline_cash_v1_poseidon::encode(Fp::from(0x31_u64)),
            ep_deferred_audit: crate::zk::offline_cash_v1_poseidon::encode(Fq::from(0x32_u64)),
            eq_proof: vec![0xA1],
            ep_proof: vec![0xB2],
            eq_history: eq_history_bytes(),
            ep_history: ep_history_bytes(),
        },
        encrypted_credit,
        artifact_manifest_digest: artifacts().artifact_manifest_digest,
    };
    payment
        .validate_shape_against(&request)
        .expect("valid payment stub");
    (request, payment)
}

fn stage_stub(
    state: &OfflineCashStateV1,
    request: OfflineCashPaymentRequestV1,
    payment: OfflineCashPaymentV1,
    envelope_digest: DigestV1,
) -> StagedCreditV1 {
    let credit_id = CreditIdV1(payment.statement.lifecycle.credit_id);
    StagedCreditV1 {
        request,
        payment,
        envelope_digest,
        stage_certificate: CreditStageCertificateV1 {
            statement: CreditStageStatementV1 {
                version: OFFLINE_CASH_STATE_VERSION_V1,
                recipient_lane: state.lane.clone(),
                receiver_state_commitment: state.state_commitment,
                receiver_hardware_epoch: state.hardware_epoch,
                receiver_device_policy_binding: state.device_policy_binding,
                receiver_state_nonce_commitment: state.state_nonce_commitment,
                credit_id,
                envelope_digest,
                staged_at_ms: 10_001,
                journal_revision_before: 0,
                journal_revision_after: 1,
            },
            guard_bundle: vec![0xA1],
        },
    }
}

fn recoverable_stage_stub(
    state: &OfflineCashStateV1,
    request: OfflineCashPaymentRequestV1,
    payment: OfflineCashPaymentV1,
    journal_revision_before: u128,
) -> (StagedCreditV1, AcceptedPaymentReceiptV1) {
    let credit_id = CreditIdV1(payment.statement.lifecycle.credit_id);
    let envelope_digest = canonical_sha256_digest(CREDIT_ENVELOPE_DOMAIN, &payment)
        .expect("canonical payment envelope digest");
    let journal_revision_after = journal_revision_before
        .checked_add(1)
        .expect("bounded test revision");
    let stage_certificate = CreditStageCertificateV1 {
        statement: CreditStageStatementV1 {
            version: OFFLINE_CASH_STATE_VERSION_V1,
            recipient_lane: state.lane.clone(),
            receiver_state_commitment: state.state_commitment,
            receiver_hardware_epoch: state.hardware_epoch,
            receiver_device_policy_binding: state.device_policy_binding,
            receiver_state_nonce_commitment: state.state_nonce_commitment,
            credit_id,
            envelope_digest,
            staged_at_ms: 10_001_u64
                .checked_add(u64::try_from(journal_revision_before).expect("bounded revision"))
                .expect("bounded staging time"),
            journal_revision_before,
            journal_revision_after,
        },
        guard_bundle: vec![0xA1],
    };
    let request_digest = request.canonical_digest().expect("request digest");
    let payment_digest = payment
        .canonical_digest_against(&request)
        .expect("payment digest");
    let inbox_receipt = OfflineCashInboxReceiptV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        credit_id: credit_id.0,
        receipt_commitment: offline_cash_inbox_receipt_commitment_v1(
            state.lane.device_lane_id,
            state.hardware_epoch.epoch_id,
            journal_revision_after,
            credit_id.0,
            payment_digest,
        )
        .expect("inbox receipt commitment"),
    };
    let mut acknowledgement = OfflineCashAcknowledgementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        request_digest,
        payment_digest,
        inbox_receipt,
        signature: sign(&signing_key(7), b"placeholder"),
    };
    acknowledgement.signature = sign(
        &signing_key(7),
        &acknowledgement
            .canonical_signing_bytes()
            .expect("acknowledgement signing bytes"),
    );
    let durable_acknowledgement =
        DurableAcknowledgementV1::from_acknowledgement(acknowledgement, &request, &payment)
            .expect("durable acknowledgement");
    let staged = StagedCreditV1 {
        request: request.clone(),
        payment: payment.clone(),
        envelope_digest,
        stage_certificate: stage_certificate.clone(),
    };
    let receipt = AcceptedPaymentReceiptV1 {
        credit_id,
        envelope_digest,
        request,
        payment,
        stage_certificate,
        durable_acknowledgement,
    };
    (staged, receipt)
}

fn accepted_receipt_stub(staged: &StagedCreditV1) -> AcceptedPaymentReceiptV1 {
    let credit_id = CreditIdV1(staged.payment.statement.lifecycle.credit_id);
    let key = signing_key(7);
    let mut acknowledgement = OfflineCashAcknowledgementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        request_digest: staged.request.canonical_digest().expect("request digest"),
        payment_digest: staged
            .payment
            .canonical_digest_against(&staged.request)
            .expect("payment digest"),
        inbox_receipt: OfflineCashInboxReceiptV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            credit_id: credit_id.0,
            receipt_commitment: staged.envelope_digest,
        },
        signature: sign(&key, b"placeholder acknowledgement"),
    };
    acknowledgement.signature = sign(
        &key,
        &acknowledgement
            .canonical_signing_bytes()
            .expect("acknowledgement signing bytes"),
    );
    acknowledgement
        .validate_shape_against(&staged.request, &staged.payment)
        .expect("valid acknowledgement stub");
    AcceptedPaymentReceiptV1 {
        credit_id,
        envelope_digest: staged.envelope_digest,
        request: staged.request.clone(),
        payment: staged.payment.clone(),
        stage_certificate: staged.stage_certificate.clone(),
        durable_acknowledgement: DurableAcknowledgementV1 {
            canonical_bytes: norito::encode_canonical(&acknowledgement)
                .expect("acknowledgement encoding"),
            acknowledgement,
        },
    }
}

fn transition_authorization(preview: &TransitionPreviewV1) -> TransitionAuthorizationV1 {
    TransitionAuthorizationV1 {
        hardware_certificate: HardwareTransitionCertificateV1 {
            statement: preview.hardware_statement.clone(),
            guard_bundle: vec![0xA1],
        },
        proof: OfflineCashPairedProofV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            eq_protocol_digest: artifacts().eq_protocol_digest,
            ep_protocol_digest: artifacts().ep_protocol_digest,
            semantic_digest: preview.transport_semantic_digest,
            guard_eq_credential_audit: crate::zk::offline_cash_v1_poseidon::encode(Fp::from(
                0x41_u64,
            )),
            guard_ep_credential_audit: crate::zk::offline_cash_v1_poseidon::encode(Fq::from(
                0x42_u64,
            )),
            eq_deferred_audit: crate::zk::offline_cash_v1_poseidon::encode(Fp::from(0x43_u64)),
            ep_deferred_audit: crate::zk::offline_cash_v1_poseidon::encode(Fq::from(0x44_u64)),
            eq_proof: vec![0xC1],
            ep_proof: vec![0xD2],
            eq_history: eq_history_bytes(),
            ep_history: ep_history_bytes(),
        },
    }
}

fn prepared_send(
    machine: &TestMachine,
    request: OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
    reservation_id: DigestV1,
) -> (PreparedOutgoingCandidateV1, TransitionPreviewV1) {
    prepared_send_with_commit_evidence(
        machine,
        request,
        payment,
        reservation_id,
        payment.statement.commit_evidence,
    )
    .expect("prepared send")
}

fn prepared_send_with_commit_evidence(
    machine: &TestMachine,
    request: OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
    reservation_id: DigestV1,
    commit_evidence: OfflineCashCommitEvidenceV1,
) -> Result<(PreparedOutgoingCandidateV1, TransitionPreviewV1), OfflineCashStateErrorV1> {
    let statement = &payment.statement;
    let amount = statement.amount;
    let outbox_reservation = OfflineCashOutboxReservationV1 {
        reservation_id,
        operation_kind: OfflineCashOperationKindV1::SendSplit,
        reserved_outbox_bytes: iroha_data_model::offline::OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1,
        issued_at_ms: 100,
        expires_at_ms: 10_000,
    };
    let successor = machine
        .next_state(
            machine
                .state
                .balance
                .checked_sub(amount)
                .expect("sufficient send balance"),
            machine.state.hardware_epoch,
            machine.state.device_policy_binding,
            digest(0xDA),
            machine.state.consumed_credit_root,
        )
        .expect("send successor");
    let lifecycle_binding_digest = statement
        .lifecycle
        .canonical_digest()
        .expect("send lifecycle digest");
    let effect_digest = statement.canonical_digest().expect("send effect digest");
    let successor_commitment = successor.state_commitment;
    let preview = machine
        .transition_preview(
            OfflineCashTransitionKindV1::SendSplit,
            successor,
            effect_digest,
            [0; 32],
            [0; 32],
            statement.lifecycle.credit_id,
            request.hardware_credential.lane_commitment,
            TransitionAuxiliaryBindingsV1 {
                lifecycle_binding_digest,
                precommit_binding_digest: outbox_reservation
                    .canonical_commitment()
                    .expect("send reservation commitment"),
                ..TransitionAuxiliaryBindingsV1::default()
            },
            9_000,
            |normalized_guard_statement_digest| {
                local_transition_transport_digest(
                    OfflineCashTransitionKindV1::SendSplit,
                    machine.state.release_id,
                    machine.state.liability_pool_id,
                    effect_digest,
                    machine.state.state_commitment,
                    successor_commitment,
                    normalized_guard_statement_digest,
                )
            },
        )
        .expect("send preview");
    let normalized_guard_statement_digest = preview
        .normalized_guard_statement
        .canonical_digest()
        .expect("send guard statement digest");
    let state_transition_digest = preview
        .proof_statement
        .digest()
        .expect("send transition digest");
    let material = PreparedSendMaterialV1 {
        proof_statement: preview.proof_statement.clone(),
        transport_semantic_digest: preview.transport_semantic_digest,
        request,
        acceptance_intent: payment.acceptance_intent,
        acceptance_ticket: payment.acceptance_ticket.clone(),
        transition_nullifier: statement.transition_nullifier,
        ciphertext_commitment: statement.ciphertext_commitment,
        encrypted_credit: payment.encrypted_credit.clone(),
        commit_evidence,
        outbox_reservation,
        sealed_transition_inputs: vec![0xDB],
        sealed_recovery_seeds: vec![0xDC],
        normalized_guard_statement_digest,
    };
    let prepared = PreparedOutgoingCandidateV1::send(
        machine.state.clone(),
        preview.successor.clone(),
        state_transition_digest,
        material,
    )?;
    Ok((prepared, preview))
}

fn prepared_redemption(
    machine: &TestMachine,
    reservation_id: DigestV1,
    reserved_outbox_bytes: u32,
) -> (PreparedOutgoingCandidateV1, TransitionPreviewV1) {
    let amount = 3;
    let terminal_nullifier = digest(0xD1);
    let redemption_commitment = digest(0xD2);
    let commit_evidence =
        OfflineCashCommitEvidenceV1::TrustedTime(OfflineCashTrustedCommitTimeV1 {
            time_evidence_commitment: digest(0xD3),
        });
    let outbox_reservation = OfflineCashOutboxReservationV1 {
        reservation_id,
        operation_kind: OfflineCashOperationKindV1::RedeemSplit,
        reserved_outbox_bytes,
        issued_at_ms: 100,
        expires_at_ms: 10_000,
    };
    let successor = machine
        .next_state(
            machine.state.balance - amount,
            machine.state.hardware_epoch,
            machine.state.device_policy_binding,
            digest(0xD4),
            machine.state.consumed_credit_root,
        )
        .expect("redemption successor");
    let lifecycle = OfflineCashLifecycleBindingV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        network_id: machine.state.lane.network_id,
        protocol_version: machine.state.protocol_version,
        suite_id: machine.state.suite_id,
        vk_digest: machine.state.vk_digest,
        release_id: machine.state.release_id,
        asset: machine.state.lane.asset.clone(),
        asset_incarnation: machine.state.asset_incarnation,
        scale: machine.state.lane.scale,
        liability_pool_id: machine.state.liability_pool_id,
        hardware_profile_id: machine.state.hardware_profile_id,
        policy_epoch: machine.state.policy_epoch,
        operation_kind: OfflineCashOperationKindV1::RedeemSplit,
        request_id: [0; 32],
        acceptance_ticket_id: [0; 32],
        credit_id: [0; 32],
        ciphertext_digest: [0; 32],
    };
    let lifecycle_binding_digest = lifecycle.canonical_digest().expect("lifecycle digest");
    let beneficiary = account(0xD5);
    let effect_digest = OfflineCashRedemptionStatementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        lifecycle: lifecycle.clone(),
        amount,
        beneficiary: beneficiary.clone(),
        terminal_nullifier,
        redemption_commitment,
        redemption_id: [1; 32],
        commit_evidence,
    }
    .seal_redemption_id()
    .expect("redemption id")
    .canonical_digest()
    .expect("redemption effect digest");
    let successor_commitment = successor.state_commitment;
    let preview = machine
        .transition_preview(
            OfflineCashTransitionKindV1::RedeemSplit,
            successor,
            effect_digest,
            [0; 32],
            [0; 32],
            [0; 32],
            [0; 32],
            TransitionAuxiliaryBindingsV1 {
                lifecycle_binding_digest,
                precommit_binding_digest: outbox_reservation
                    .canonical_commitment()
                    .expect("reservation commitment"),
                ..TransitionAuxiliaryBindingsV1::default()
            },
            1_000,
            |normalized_guard_statement_digest| {
                local_transition_transport_digest(
                    OfflineCashTransitionKindV1::RedeemSplit,
                    machine.state.release_id,
                    machine.state.liability_pool_id,
                    effect_digest,
                    machine.state.state_commitment,
                    successor_commitment,
                    normalized_guard_statement_digest,
                )
            },
        )
        .expect("redemption preview");
    let normalized_guard_statement_digest = preview
        .normalized_guard_statement
        .canonical_digest()
        .expect("guard statement digest");
    let state_transition_digest = preview.proof_statement.digest().expect("transition digest");
    let material = PreparedRedemptionMaterialV1 {
        proof_statement: preview.proof_statement.clone(),
        transport_semantic_digest: preview.transport_semantic_digest,
        amount,
        beneficiary,
        terminal_nullifier,
        redemption_commitment,
        commit_evidence,
        outbox_reservation,
        sealed_transition_inputs: vec![0xD6],
        sealed_recovery_seeds: vec![0xD7],
        normalized_guard_statement_digest,
    };
    let prepared = PreparedOutgoingCandidateV1::redemption(
        machine.state.clone(),
        preview.successor.clone(),
        state_transition_digest,
        material,
    )
    .expect("prepared redemption");
    (prepared, preview)
}

fn commit_certificate(
    candidate: &PersistedOutgoingCandidateV1,
    commit_evidence: OfflineCashCommitEvidenceV1,
) -> OfflineCashCommitCertificateV1 {
    let prepared = &candidate.prepared;
    OfflineCashCommitCertificateV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        certificate_id: [0; 32],
        candidate_envelope_digest: candidate.candidate_envelope_digest,
        lifecycle_binding_digest: prepared
            .lifecycle()
            .canonical_digest()
            .expect("lifecycle digest"),
        transition_nullifier: prepared.transition_nullifier(),
        outbox_reservation_commitment: prepared
            .outbox_reservation
            .canonical_commitment()
            .expect("reservation commitment"),
        commit_evidence,
        hardware_profile_id: prepared.lifecycle().hardware_profile_id,
        policy_epoch: prepared.lifecycle().policy_epoch,
        hardware_terminal_commitment: digest(0xD8),
    }
    .seal_certificate_id()
    .expect("terminal certificate")
}

fn commit_wrapper_proof(
    committed: &CommittedOutgoingCandidateV1,
) -> OfflineCashCommitWrapperProofV1 {
    let public_inputs = committed
        .public_wrapper_inputs()
        .expect("terminal public inputs");
    OfflineCashCommitWrapperProofV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        eq_protocol_digest: artifacts().commit_wrapper_eq_protocol_digest,
        ep_protocol_digest: artifacts().commit_wrapper_ep_protocol_digest,
        semantic_digest: public_inputs.semantic_digest,
        candidate_envelope_digest: public_inputs.candidate_envelope_digest,
        commit_certificate_digest: public_inputs.commit_certificate_digest,
        eq_deferred_audit: crate::zk::offline_cash_v1_poseidon::encode(Fp::from(0x71_u64)),
        ep_deferred_audit: crate::zk::offline_cash_v1_poseidon::encode(Fq::from(0x72_u64)),
        eq_proof: vec![0xE8],
        ep_proof: vec![0xE9],
        eq_history: eq_history_bytes(),
        ep_history: ep_history_bytes(),
    }
}

fn rebind_anchor_to_snapshot(
    anchor: &mut DurabilityAnchorV1,
    snapshot: &OfflineCashStateSnapshotV1,
) {
    anchor.statement = DurabilityAnchorStatementV1 {
        version: snapshot.version,
        lane: snapshot.state.lane.clone(),
        state_commitment: snapshot.state.state_commitment,
        hardware_epoch: snapshot.state.hardware_epoch,
        device_policy_binding: snapshot.state.device_policy_binding,
        state_nonce_commitment: snapshot.state.state_nonce_commitment,
        logical_sequence: snapshot.state.logical_sequence,
        journal_revision: snapshot.journal_revision,
        snapshot_commitment: snapshot.snapshot_commitment,
    };
}

fn reseal_snapshot(snapshot: &mut OfflineCashStateSnapshotV1) {
    snapshot.snapshot_commitment = canonical_poseidon_digest(
        SNAPSHOT_COMMITMENT_DOMAIN,
        &SnapshotCommitmentPreimageV1 {
            version: snapshot.version,
            state: snapshot.state.clone(),
            journal_revision: snapshot.journal_revision,
            pending_credits: snapshot.pending_credits.clone(),
            accepted_recipient_bindings: snapshot.accepted_recipient_bindings.clone(),
            accepted_payment_receipts: snapshot.accepted_payment_receipts.clone(),
            consumed_credits: snapshot.consumed_credits.clone(),
            acceptance_ticket_book: snapshot.acceptance_ticket_book.clone(),
            sender_outbox_capacity: snapshot.sender_outbox_capacity.clone(),
            outgoing_candidate_journal: snapshot.outgoing_candidate_journal.clone(),
        },
    )
    .expect("snapshot commitment");
}

#[test]
fn aggregate_state_commitment_binds_typed_incarnation_and_hardware_profile() {
    let machine = machine(7);
    machine.state().validate().expect("valid aggregate state");

    let mut changed_profile = machine.state().clone();
    changed_profile.hardware_profile_id = digest(0x99);
    assert_eq!(
        changed_profile.validate(),
        Err(OfflineCashStateErrorV1::StateCommitmentMismatch)
    );

    let mut changed_incarnation = machine.state().clone();
    changed_incarnation.asset_incarnation = AxtAssetIncarnationV1::derive(
        &network(),
        &asset(),
        &HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"another-registration")),
        &Hash::new(b"another-execution"),
        2,
    );
    assert_eq!(
        changed_incarnation.validate(),
        Err(OfflineCashStateErrorV1::InvalidReleaseOrLiabilityPool)
    );
}

#[test]
fn receive_fold_late_authorization_failure_is_fully_atomic() {
    let mut machine = rejecting_transition_machine(0);
    for index in 0..3_u64 {
        let (request, payment) = payment_stub(machine.state(), index);
        let credit_id = CreditIdV1(payment.statement.lifecycle.credit_id);
        machine
            .acceptance_ticket_book
            .preseed_consumed_payment_for_test(
                request.clone(),
                &payment,
                indexed_digest(0x92, index),
            )
            .expect("capacity-backed consumed ticket");
        let (staged, receipt) =
            recoverable_stage_stub(machine.state(), request, payment, u128::from(index));
        assert!(machine.pending_credits.insert(credit_id, staged).is_none());
        assert!(
            machine
                .accepted_payment_receipts
                .insert(credit_id, receipt)
                .is_none()
        );
    }
    machine
        .acceptance_ticket_book
        .finish_consumed_payment_preseed_for_test()
        .expect("exact ticket meters");
    machine.journal_revision = 3;
    let snapshot_usage = receiver_snapshot_capacity_usage_v1(
        &machine.pending_credits,
        &machine.accepted_payment_receipts,
        &machine.consumed_credits,
    )
    .expect("staged snapshot usage");
    let maximum_committed_bytes = machine.acceptance_ticket_book.committed_inbox_bytes();
    machine
        .acceptance_ticket_book
        .reconcile_receiver_snapshot_usage(
            snapshot_usage.live_bytes,
            snapshot_usage.retained_bytes,
            maximum_committed_bytes,
        )
        .expect("materialize staged snapshot bytes");

    let credit_id = *machine
        .pending_credits
        .keys()
        .next()
        .expect("staged credit");
    let preview = machine
        .preview_receive_fold(credit_id, digest(0x9A), 20_000)
        .expect("receive preview");
    let authorization = transition_authorization(&preview.transition);
    let state_before = machine.state.clone();
    let revision_before = machine.journal_revision;
    let pending_before = machine.pending_credits.clone();
    let replay_before = machine.consumed_credits.records();
    let replay_root_before = machine.consumed_credits.root();
    let ticket_book_before = machine.acceptance_ticket_book.clone();
    let receipts_before = machine.accepted_payment_receipts.clone();

    assert_eq!(
        machine.receive_fold_prepared(preview, authorization),
        Err(OfflineCashStateErrorV1::GuardRejected(
            "rejected test transition".to_owned()
        ))
    );
    assert_eq!(machine.state, state_before);
    assert_eq!(machine.journal_revision, revision_before);
    assert_eq!(machine.pending_credits, pending_before);
    assert_eq!(machine.consumed_credits.records(), replay_before);
    assert_eq!(machine.consumed_credits.root(), replay_root_before);
    assert_eq!(machine.acceptance_ticket_book, ticket_book_before);
    assert_eq!(machine.accepted_payment_receipts, receipts_before);
}

#[test]
fn mocked_state_machine_folds_one_thousand_credits_and_builds_one_spend() {
    let mut machine = machine(0);
    machine.acceptance_ticket_book = OfflineCashAcceptanceTicketBookV1::new(512 * 1024 * 1024);
    let mut expected_replay_records = BTreeMap::new();
    for index in 0..1_000_u64 {
        let (request, payment) = payment_stub(machine.state(), index);
        let credit_id = CreditIdV1(payment.statement.lifecycle.credit_id);
        machine
            .acceptance_ticket_book
            .preseed_consumed_payment_for_test(
                request.clone(),
                &payment,
                indexed_digest(0x92, index),
            )
            .expect("capacity-backed consumed ticket");
        let (staged, receipt) =
            recoverable_stage_stub(machine.state(), request, payment, u128::from(index));
        let envelope_digest = staged.envelope_digest;
        assert!(
            expected_replay_records
                .insert(credit_id, envelope_digest)
                .is_none()
        );
        assert!(
            machine
                .accepted_payment_receipts
                .insert(credit_id, receipt)
                .is_none()
        );
        assert!(machine.pending_credits.insert(credit_id, staged).is_none());
    }
    machine
        .acceptance_ticket_book
        .finish_consumed_payment_preseed_for_test()
        .expect("exact aggregate ticket capacity");
    machine.journal_revision = 1_000;

    assert_eq!(machine.pending_credit_count(), 1_000);
    assert_eq!(
        machine
            .pending_credits_required_for_amount(1_000)
            .expect("all credits required")
            .len(),
        1_000
    );

    let mut fold_index = 0_u64;
    while !machine.pending_credits.is_empty() {
        let credit_id = *machine
            .pending_credits
            .keys()
            .next()
            .expect("staged credit");
        let preview = machine
            .preview_receive_fold(
                credit_id,
                indexed_digest(0x91, fold_index),
                20_000 + fold_index,
            )
            .expect("single-credit receive preview");
        let authorization = transition_authorization(&preview.transition);
        machine
            .receive_fold_prepared(preview, authorization)
            .expect("authorized receive fold");
        fold_index += 1;
    }

    assert_eq!(fold_index, 1_000);
    assert_eq!(machine.pending_credit_count(), 0);
    assert_eq!(machine.state().balance, 1_000);
    assert_eq!(machine.state().logical_sequence, 1_000);
    assert_eq!(machine.journal_revision(), 2_000);
    assert_eq!(machine.consumed_credits.len(), 1_000);
    assert_eq!(machine.accepted_payment_receipts.len(), 1_000);
    for (credit_id, envelope_digest) in &expected_replay_records {
        assert_eq!(
            machine.consumed_credits.get(*credit_id),
            Some(*envelope_digest)
        );
        assert_eq!(
            machine
                .accepted_payment_receipts
                .get(credit_id)
                .map(|receipt| receipt.envelope_digest),
            Some(*envelope_digest)
        );
    }

    let snapshot = machine.snapshot().expect("recoverable folded snapshot");
    assert_eq!(snapshot.journal_revision, 2_000);
    assert_eq!(snapshot.consumed_credits.len(), 1_000);
    assert_eq!(snapshot.accepted_payment_receipts.len(), 1_000);
    for record in &snapshot.consumed_credits {
        assert_eq!(
            expected_replay_records.get(&record.credit_id),
            Some(&record.envelope_digest)
        );
    }
    let anchor = machine
        .seal_durability_anchor(vec![0xA1])
        .expect("hardware-sealed recovery anchor");
    let mut restored = TestMachine::restore(
        snapshot,
        &anchor,
        OfflineCashStateProofReleaseV1::from_test_artifacts(artifacts()),
        AcceptingRecursiveVerifier,
        AcceptingGuardVerifier,
    )
    .expect("recover complete aggregate state");
    assert_eq!(restored.journal_revision(), 2_000);
    assert_eq!(restored.consumed_credits.len(), 1_000);
    for (credit_id, envelope_digest) in expected_replay_records {
        assert_eq!(
            restored.consumed_credits.get(credit_id),
            Some(envelope_digest)
        );
    }

    const AGGREGATE_AMOUNT: u128 = 1_000;
    let reservation_id = indexed_digest(0xE3, 1_000);
    let (request, payment_template) =
        payment_stub_with_amount(restored.state(), 1_000, AGGREGATE_AMOUNT);
    let commit_evidence = payment_template.statement.commit_evidence;
    let (prepared, preview) = prepared_send(
        &restored,
        request.clone(),
        &payment_template,
        reservation_id,
    );
    let (_, capability) = restored
        .prepare_outgoing_candidate(prepared)
        .expect("prepare aggregate send");
    let candidate = restored
        .persist_outgoing_candidate(&capability, transition_authorization(&preview).proof)
        .expect("persist aggregate send proof");
    let committed = restored
        .commit_outgoing_candidate(capability, commit_certificate(&candidate, commit_evidence))
        .expect("commit aggregate send");
    assert_eq!(restored.state(), &preview.successor);
    assert_eq!(restored.state().balance, 0);
    assert_eq!(restored.state().logical_sequence, 1_001);
    assert_eq!(restored.journal_revision(), 2_001);

    let public_inputs = committed
        .public_wrapper_inputs()
        .expect("aggregate send public inputs");
    let finalized = restored
        .finalize_outgoing_candidate(
            OfflineCashCommitWrapperProofV1 {
                version: OFFLINE_CASH_WIRE_VERSION_V1,
                eq_protocol_digest: artifacts().commit_wrapper_eq_protocol_digest,
                ep_protocol_digest: artifacts().commit_wrapper_ep_protocol_digest,
                semantic_digest: public_inputs.semantic_digest,
                candidate_envelope_digest: public_inputs.candidate_envelope_digest,
                commit_certificate_digest: public_inputs.commit_certificate_digest,
                eq_deferred_audit: crate::zk::offline_cash_v1_poseidon::encode(Fp::from(0x71_u64)),
                ep_deferred_audit: crate::zk::offline_cash_v1_poseidon::encode(Fq::from(0x72_u64)),
                eq_proof: vec![0xE8],
                ep_proof: vec![0xE9],
                eq_history: eq_history_bytes(),
                ep_history: ep_history_bytes(),
            },
            vec![0xEA],
        )
        .expect("finalize aggregate payment");
    let payment = match &finalized.envelope {
        OfflineCashOutgoingEnvelopeV1::Payment(payment) => payment,
        OfflineCashOutgoingEnvelopeV1::Redemption(_) => {
            panic!("aggregate send must expose one payment")
        }
    };
    payment
        .validate_shape_against(&request)
        .expect("valid aggregate payment");
    assert_eq!(payment.statement.amount, AGGREGATE_AMOUNT);
    assert_eq!(payment.acceptance_intent.exact_amount, AGGREGATE_AMOUNT);
    assert_eq!(payment.acceptance_ticket.exact_amount, AGGREGATE_AMOUNT);
    assert_eq!(
        payment.statement.lifecycle.operation_kind,
        OfflineCashOperationKindV1::SendSplit
    );
    assert_eq!(
        restored
            .outgoing_candidate_journal()
            .finalized_outbox_count(),
        1
    );
    assert_eq!(restored.state().balance, 0);
    assert_eq!(
        restored
            .expose_outgoing_candidate(reservation_id)
            .expect("expose aggregate payment"),
        finalized.retry_bytes()
    );
    let payment_bytes = norito::encode_canonical(payment).expect("canonical aggregate payment");
    assert_eq!(finalized.retry_bytes(), payment_bytes.as_slice());
    assert!(
        finalized.retry_bytes().len()
            <= iroha_data_model::offline::OFFLINE_CASH_PAYMENT_MAX_BYTES_V1
    );
}

#[test]
fn prepared_send_rejects_commit_evidence_substitution_after_effect_binding() {
    let machine = machine(10);
    let (request, payment) = payment_stub_with_amount(machine.state(), 10_001, 3);
    let reservation_id = indexed_digest(0xE3, 10_001);
    prepared_send(&machine, request.clone(), &payment, reservation_id);

    let substituted_commit_evidence =
        OfflineCashCommitEvidenceV1::TrustedTime(OfflineCashTrustedCommitTimeV1 {
            time_evidence_commitment: digest(0xEE),
        });
    assert_ne!(
        substituted_commit_evidence,
        payment.statement.commit_evidence
    );
    let mut substituted_statement = payment.statement.clone();
    substituted_statement.commit_evidence = substituted_commit_evidence;
    let substituted_statement = substituted_statement
        .seal_credit_id()
        .expect("substituted statement credit id");
    assert_eq!(substituted_statement.amount, payment.statement.amount);
    assert_eq!(substituted_statement.lifecycle, payment.statement.lifecycle);
    assert_ne!(
        substituted_statement
            .canonical_digest()
            .expect("substituted statement digest"),
        payment
            .statement
            .canonical_digest()
            .expect("original statement digest")
    );

    assert_eq!(
        prepared_send_with_commit_evidence(
            &machine,
            request,
            &payment,
            reservation_id,
            substituted_commit_evidence,
        ),
        Err(OfflineCashStateErrorV1::InvalidCandidateStage)
    );
}

#[test]
fn receiver_snapshot_meter_covers_max_guard_and_collection_framing() {
    let machine = machine(0);
    let mut pending = BTreeMap::new();
    let mut receipts = BTreeMap::new();
    let consumed = ExactConsumedCreditIndex::empty();
    let mut usage_at_127 = None;

    for index in 0..128_u64 {
        let (request, payment) = payment_stub(machine.state(), index);
        let credit_id = CreditIdV1(payment.statement.lifecycle.credit_id);
        let envelope_digest = indexed_digest(0xE4, index);
        let mut staged = stage_stub(machine.state(), request, payment, envelope_digest);
        staged.stage_certificate.guard_bundle = if index == 0 {
            vec![0xA5; OFFLINE_CASH_GUARD_BUNDLE_MAX_BYTES_V1]
        } else {
            vec![0xA5]
        };
        let receipt = accepted_receipt_stub(&staged);
        assert!(pending.insert(credit_id, staged).is_none());
        assert!(receipts.insert(credit_id, receipt).is_none());
        let usage = receiver_snapshot_capacity_usage_v1(&pending, &receipts, &consumed)
            .expect("exact receiver snapshot usage");
        assert_eq!(usage.retained_bytes, 0);
        assert!(
            usage.live_bytes
                <= u64::try_from(pending.len()).expect("bounded test count")
                    * candidate_lifecycle::RECEIVER_SNAPSHOT_ENTRY_MAX_BYTES_V1
        );
        if index == 0 {
            assert!(usage.live_bytes > OFFLINE_CASH_GUARD_BUNDLE_MAX_BYTES_V1 as u64);
        }
        if pending.len() == 127 {
            usage_at_127 = Some(usage.live_bytes);
        }
    }

    let usage_at_128 = receiver_snapshot_capacity_usage_v1(&pending, &receipts, &consumed)
        .expect("128-entry receiver snapshot usage")
        .live_bytes;
    assert!(usage_at_128 > usage_at_127.expect("127-entry usage"));

    let full_projection = ReceiverSnapshotCapacityProjectionV1 {
        pending_credits: pending.values().cloned().collect(),
        accepted_payment_receipts: receipts.values().cloned().collect(),
        consumed_credits: Vec::new(),
    };
    let empty_projection = ReceiverSnapshotCapacityProjectionV1 {
        pending_credits: Vec::new(),
        accepted_payment_receipts: Vec::new(),
        consumed_credits: Vec::new(),
    };
    let exact_delta = norito::encode_canonical(&full_projection)
        .expect("full projection")
        .len()
        .checked_sub(
            norito::encode_canonical(&empty_projection)
                .expect("empty projection")
                .len(),
        )
        .expect("projection grows");
    assert_eq!(
        usage_at_128,
        u64::try_from(exact_delta).expect("delta fits")
    );

    let mut folded = ExactConsumedCreditIndex::empty();
    for receipt in receipts.values() {
        folded
            .insert(receipt.credit_id, receipt.envelope_digest)
            .expect("unique folded credit");
    }
    let folded_usage = receiver_snapshot_capacity_usage_v1(&BTreeMap::new(), &receipts, &folded)
        .expect("folded receiver snapshot usage");
    assert_eq!(folded_usage.live_bytes, 0);
    assert!(folded_usage.retained_bytes > OFFLINE_CASH_GUARD_BUNDLE_MAX_BYTES_V1 as u64);
    assert!(
        folded_usage.retained_bytes
            <= 128 * candidate_lifecycle::RECEIVER_SNAPSHOT_ENTRY_MAX_BYTES_V1
    );
}

#[test]
fn receive_backlog_has_no_count_based_admission_limit() {
    let mut machine = machine(5);
    for index in 0..17_u64 {
        let (request, payment) = payment_stub(machine.state(), index);
        let credit_id = CreditIdV1(payment.statement.lifecycle.credit_id);
        let envelope_digest = indexed_digest(0xA0, index);
        let staged = stage_stub(machine.state(), request, payment, envelope_digest);
        machine.pending_credits.insert(credit_id, staged);
    }
    for (index, credit_id) in machine.pending_credits.keys().copied().enumerate() {
        machine
            .preview_receive_fold(
                credit_id,
                indexed_digest(0xA1, index as u64),
                30_000 + index as u64,
            )
            .expect("every staged credit remains independently foldable");
    }
    assert_eq!(machine.pending_credit_count(), 17);
}

#[test]
fn proof_histories_remain_constant_size_as_balance_history_grows() {
    let preview = machine(0)
        .preview_rotate(
            HardwareEpochV1 {
                generation: 2,
                epoch_id: digest(0xB0),
            },
            DevicePolicyBindingV1 {
                device_key_reference: digest(0xB1),
                hardware_policy_id: digest(0xB2),
            },
            digest(0xB3),
            40_000,
        )
        .expect("rotation preview");
    let proof = transition_authorization(&preview).proof;
    assert_eq!(
        proof.eq_history.len(),
        OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1
    );
    assert_eq!(
        proof.ep_history.len(),
        OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1
    );
}

#[test]
fn prepare_atomically_reserves_capacity_and_snapshot_rejects_torn_outbox() {
    let mut machine = machine(10);
    let total_outbox_bytes = machine.sender_outbox_capacity.total_outbox_bytes();
    let oversized = u32::try_from(total_outbox_bytes + 1).expect("test capacity fits u32");
    let (oversized_prepared, _) = prepared_redemption(&machine, digest(0xE1), oversized);
    assert_eq!(
        machine.prepare_outgoing_candidate(oversized_prepared),
        Err(OfflineCashStateErrorV1::SenderOutboxCapacityExhausted)
    );
    assert!(matches!(
        machine.outgoing_candidate_journal.stage(),
        OfflineCashOutgoingJournalStageV1::Empty
    ));
    assert_eq!(machine.sender_outbox_capacity.committed_outbox_bytes(), 0);

    let reservation_bytes = iroha_data_model::offline::OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1;
    let (prepared, _) = prepared_redemption(&machine, digest(0xE2), reservation_bytes);
    let (outcome, _capability) = machine
        .prepare_outgoing_candidate(prepared.clone())
        .expect("initial prepare");
    assert_eq!(outcome, SenderOutboxReservationOutcomeV1::Reserved);
    let (outcome, _replayed_capability) = machine
        .prepare_outgoing_candidate(prepared.clone())
        .expect("idempotent prepare");
    assert_eq!(outcome, SenderOutboxReservationOutcomeV1::AlreadyReserved);
    assert!(matches!(
        machine.outgoing_candidate_journal.stage(),
        OfflineCashOutgoingJournalStageV1::Prepared(existing) if existing == &prepared
    ));
    assert!(machine.sender_outbox_capacity.committed_outbox_bytes() > u64::from(reservation_bytes));
    assert!(machine.sender_outbox_capacity.retained_metadata_bytes() > 0);
    assert!(
        machine
            .sender_outbox_capacity
            .reserved_terminal_metadata_bytes()
            > 0
    );

    let snapshot = machine.snapshot().expect("prepared snapshot");
    let anchor = machine
        .seal_durability_anchor(vec![0xA1])
        .expect("prepared anchor");
    let restored = TestMachine::restore(
        snapshot.clone(),
        &anchor,
        machine.proof_release,
        AcceptingRecursiveVerifier,
        AcceptingGuardVerifier,
    )
    .expect("prepared restore");
    assert_eq!(restored.snapshot().expect("restored snapshot"), snapshot);

    let mut torn = snapshot;
    torn.sender_outbox_capacity = OfflineCashSenderOutboxCapacityV1::new(total_outbox_bytes);
    reseal_snapshot(&mut torn);
    let mut forged_anchor = anchor;
    forged_anchor.statement.snapshot_commitment = torn.snapshot_commitment;
    assert!(matches!(
        TestMachine::restore(
            torn,
            &forged_anchor,
            machine.proof_release,
            AcceptingRecursiveVerifier,
            AcceptingGuardVerifier,
        ),
        Err(OfflineCashStateErrorV1::SnapshotIntegrity)
    ));
}

#[test]
fn mock_candidate_recovery_commits_one_successor_and_preserves_retry_bytes() {
    let mut origin = machine(10);
    let reservation_id = digest(0xEA);
    let commit_evidence =
        OfflineCashCommitEvidenceV1::TrustedTime(OfflineCashTrustedCommitTimeV1 {
            time_evidence_commitment: digest(0xD3),
        });
    let (prepared, preview) = prepared_redemption(
        &origin,
        reservation_id,
        iroha_data_model::offline::OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1,
    );
    let predecessor = origin.state().clone();
    let predecessor_revision = origin.journal_revision();
    let (_, initial_capability) = origin
        .prepare_outgoing_candidate(prepared.clone())
        .expect("prepare before candidate persistence");
    let prepared_snapshot = origin.snapshot().expect("prepared recovery snapshot");
    let prepared_anchor = origin
        .seal_durability_anchor(vec![0xA1])
        .expect("prepared recovery anchor");
    origin = TestMachine::restore(
        prepared_snapshot,
        &prepared_anchor,
        origin.proof_release,
        AcceptingRecursiveVerifier,
        AcceptingGuardVerifier,
    )
    .expect("recover prepared operation before proving");
    let recovered_prepared_capability = origin
        .recover_outgoing_commit_capability()
        .expect("reissue prepared commit authority");
    assert_eq!(initial_capability, recovered_prepared_capability);
    let candidate = origin
        .persist_outgoing_candidate(
            &recovered_prepared_capability,
            transition_authorization(&preview).proof,
        )
        .expect("persist candidate before simulated crash");
    assert_eq!(origin.state(), &predecessor);
    assert_eq!(origin.journal_revision(), predecessor_revision);
    assert!(matches!(
        origin.outgoing_candidate_journal.stage(),
        OfflineCashOutgoingJournalStageV1::Candidate(existing) if existing == &candidate
    ));

    let candidate_snapshot = origin.snapshot().expect("candidate snapshot");
    let candidate_anchor = origin
        .seal_durability_anchor(vec![0xA1])
        .expect("candidate anchor");
    let mut recovered = TestMachine::restore(
        candidate_snapshot.clone(),
        &candidate_anchor,
        origin.proof_release,
        AcceptingRecursiveVerifier,
        AcceptingGuardVerifier,
    )
    .expect("first candidate recovery");
    let recovered_again = TestMachine::restore(
        candidate_snapshot.clone(),
        &candidate_anchor,
        origin.proof_release,
        AcceptingRecursiveVerifier,
        AcceptingGuardVerifier,
    )
    .expect("repeated candidate recovery");
    assert_eq!(
        recovered.snapshot().expect("first recovered snapshot"),
        candidate_snapshot
    );
    assert_eq!(
        recovered_again
            .snapshot()
            .expect("second recovered snapshot"),
        candidate_snapshot
    );

    let recovered_capability = recovered
        .recover_outgoing_commit_capability()
        .expect("recover candidate commit authority");
    let replay_capability = recovered
        .recover_outgoing_commit_capability()
        .expect("idempotently recover candidate commit authority");
    assert_eq!(recovered_capability, replay_capability);
    let certificate = commit_certificate(&candidate, commit_evidence);
    let committed = recovered
        .commit_outgoing_candidate(recovered_capability, certificate.clone())
        .expect("commit the recovered candidate once");
    assert_eq!(recovered.state(), &preview.successor);
    assert_eq!(
        recovered.journal_revision(),
        preview.proof_statement.journal_revision_after
    );
    assert_eq!(
        recovered.state().logical_sequence,
        predecessor.logical_sequence + 1
    );
    let after_commit = recovered.snapshot().expect("committed snapshot");
    assert_eq!(
        recovered.commit_outgoing_candidate(replay_capability, certificate),
        Err(OfflineCashStateErrorV1::InvalidCandidateStage)
    );
    assert_eq!(
        recovered
            .snapshot()
            .expect("snapshot after rejected replay"),
        after_commit,
        "recovered commit authority cannot install the successor twice"
    );

    let committed_anchor = recovered
        .seal_durability_anchor(vec![0xA1])
        .expect("committed anchor");
    let mut finalize_after_recovery = TestMachine::restore(
        after_commit.clone(),
        &committed_anchor,
        origin.proof_release,
        AcceptingRecursiveVerifier,
        AcceptingGuardVerifier,
    )
    .expect("recover committed candidate for wrapper generation");
    let mut finalize_after_repeated_recovery = TestMachine::restore(
        after_commit,
        &committed_anchor,
        origin.proof_release,
        AcceptingRecursiveVerifier,
        AcceptingGuardVerifier,
    )
    .expect("repeat committed-candidate recovery");
    let wrapper_proof = commit_wrapper_proof(&committed);
    let retry_metadata = vec![0xEB; 32];
    let finalized = finalize_after_recovery
        .finalize_outgoing_candidate(wrapper_proof.clone(), retry_metadata.clone())
        .expect("finalize recovered candidate");
    let repeated_finalized = finalize_after_repeated_recovery
        .finalize_outgoing_candidate(wrapper_proof, retry_metadata)
        .expect("repeat deterministic finalization after recovery");
    assert_eq!(finalized, repeated_finalized);
    assert_eq!(finalized.retry_bytes(), repeated_finalized.retry_bytes());
    assert_eq!(
        finalize_after_recovery
            .outgoing_candidate_journal
            .finalized_outbox_count(),
        1
    );
    assert_eq!(
        finalize_after_recovery
            .expose_outgoing_candidate(reservation_id)
            .expect("first exposure"),
        finalized.retry_bytes()
    );
    assert_eq!(
        finalize_after_recovery
            .expose_outgoing_candidate(reservation_id)
            .expect("idempotent exposure"),
        finalized.retry_bytes()
    );

    let finalized_snapshot = finalize_after_recovery
        .snapshot()
        .expect("finalized snapshot");
    let finalized_anchor = finalize_after_recovery
        .seal_durability_anchor(vec![0xA1])
        .expect("finalized anchor");
    let mut final_recovery = TestMachine::restore(
        finalized_snapshot.clone(),
        &finalized_anchor,
        origin.proof_release,
        AcceptingRecursiveVerifier,
        AcceptingGuardVerifier,
    )
    .expect("recover finalized envelope");
    assert_eq!(
        final_recovery
            .expose_outgoing_candidate(reservation_id)
            .expect("exposure after final recovery"),
        finalized.retry_bytes()
    );
    assert_eq!(final_recovery.state(), &preview.successor);
    assert_eq!(final_recovery.journal_revision(), predecessor_revision + 1);
    assert_eq!(
        final_recovery.prepare_outgoing_candidate(prepared),
        Err(OfflineCashStateErrorV1::InvalidCandidateStage)
    );
    assert_eq!(
        final_recovery
            .snapshot()
            .expect("single-successor snapshot"),
        finalized_snapshot
    );
}

#[test]
fn mock_candidate_restore_rejects_torn_and_corrupt_boundaries() {
    let mut machine = machine(10);
    let (prepared, preview) = prepared_redemption(
        &machine,
        digest(0xEC),
        iroha_data_model::offline::OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1,
    );
    let (_, capability) = machine
        .prepare_outgoing_candidate(prepared)
        .expect("prepare candidate boundary");
    let candidate = machine
        .persist_outgoing_candidate(&capability, transition_authorization(&preview).proof)
        .expect("persist candidate boundary");
    let snapshot = machine.snapshot().expect("candidate snapshot");
    let anchor = machine
        .seal_durability_anchor(vec![0xA1])
        .expect("candidate anchor");
    TestMachine::restore(
        snapshot.clone(),
        &anchor,
        machine.proof_release,
        AcceptingRecursiveVerifier,
        AcceptingGuardVerifier,
    )
    .expect("uncorrupted candidate restores");

    let mut torn_successor = snapshot.clone();
    torn_successor.state = preview.successor.clone();
    torn_successor.journal_revision = preview.proof_statement.journal_revision_after;
    reseal_snapshot(&mut torn_successor);
    let mut torn_successor_anchor = anchor.clone();
    rebind_anchor_to_snapshot(&mut torn_successor_anchor, &torn_successor);
    assert!(matches!(
        TestMachine::restore(
            torn_successor,
            &torn_successor_anchor,
            machine.proof_release,
            AcceptingRecursiveVerifier,
            AcceptingGuardVerifier,
        ),
        Err(OfflineCashStateErrorV1::SnapshotIntegrity)
    ));

    let committed_record = CommittedOutgoingCandidateV1::from_hardware_commit(
        candidate.clone(),
        commit_certificate(
            &candidate,
            OfflineCashCommitEvidenceV1::TrustedTime(OfflineCashTrustedCommitTimeV1 {
                time_evidence_commitment: digest(0xD3),
            }),
        ),
    )
    .expect("construct committed terminal record");
    let mut committed_journal = OfflineCashOutgoingCandidateJournalV1::default();
    committed_journal
        .prepare(candidate.prepared.clone())
        .expect("install prepared record before torn commit");
    committed_journal
        .persist_candidate(candidate.clone())
        .expect("install candidate record before torn commit");
    committed_journal
        .commit(committed_record)
        .expect("install committed record without advancing aggregate head");
    let mut torn_commit = snapshot.clone();
    torn_commit.outgoing_candidate_journal = committed_journal;
    reseal_snapshot(&mut torn_commit);
    let mut torn_commit_anchor = anchor.clone();
    rebind_anchor_to_snapshot(&mut torn_commit_anchor, &torn_commit);
    assert!(matches!(
        TestMachine::restore(
            torn_commit,
            &torn_commit_anchor,
            machine.proof_release,
            AcceptingRecursiveVerifier,
            AcceptingGuardVerifier,
        ),
        Err(OfflineCashStateErrorV1::SnapshotIntegrity)
    ));

    let mut corrupt_candidate = candidate.clone();
    corrupt_candidate.candidate_proof.eq_proof[0] ^= 0x01;
    let mut corrupt_journal = OfflineCashOutgoingCandidateJournalV1::default();
    corrupt_journal
        .prepare(corrupt_candidate.prepared.clone())
        .expect("install forged prepare record");
    corrupt_journal
        .persist_candidate(corrupt_candidate)
        .expect("install forged candidate record");
    let mut corrupt_proof = snapshot.clone();
    corrupt_proof.outgoing_candidate_journal = corrupt_journal;
    reseal_snapshot(&mut corrupt_proof);
    let mut corrupt_proof_anchor = anchor.clone();
    rebind_anchor_to_snapshot(&mut corrupt_proof_anchor, &corrupt_proof);
    assert!(matches!(
        TestMachine::restore(
            corrupt_proof,
            &corrupt_proof_anchor,
            machine.proof_release,
            AcceptingRecursiveVerifier,
            AcceptingGuardVerifier,
        ),
        Err(OfflineCashStateErrorV1::SnapshotIntegrity)
    ));

    let mut missing_reservation = snapshot;
    missing_reservation.sender_outbox_capacity =
        OfflineCashSenderOutboxCapacityV1::new(machine.sender_outbox_capacity.total_outbox_bytes());
    reseal_snapshot(&mut missing_reservation);
    let mut missing_reservation_anchor = anchor;
    rebind_anchor_to_snapshot(&mut missing_reservation_anchor, &missing_reservation);
    assert!(matches!(
        TestMachine::restore(
            missing_reservation,
            &missing_reservation_anchor,
            machine.proof_release,
            AcceptingRecursiveVerifier,
            AcceptingGuardVerifier,
        ),
        Err(OfflineCashStateErrorV1::SnapshotIntegrity)
    ));
}

#[test]
fn hardware_commit_atomically_installs_successor_and_final_retry_state() {
    let mut machine = machine(10);
    let reservation_bytes = iroha_data_model::offline::OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1;
    let (prepared, preview) = prepared_redemption(&machine, digest(0xE3), reservation_bytes);
    let (_, capability) = machine
        .prepare_outgoing_candidate(prepared)
        .expect("atomic prepare");
    let precommitted_outbox_bytes = machine.sender_outbox_capacity.committed_outbox_bytes();
    let mut candidate_proof = transition_authorization(&preview).proof;
    candidate_proof.eq_proof = vec![0xE8; OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1];
    candidate_proof.ep_proof = vec![0xE9; OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1];
    let candidate = machine
        .persist_outgoing_candidate(&capability, candidate_proof)
        .expect("persist candidate");
    let certificate = commit_certificate(
        &candidate,
        OfflineCashCommitEvidenceV1::TrustedTime(OfflineCashTrustedCommitTimeV1 {
            time_evidence_commitment: digest(0xD3),
        }),
    );
    let committed = machine
        .commit_outgoing_candidate(capability, certificate)
        .expect("atomic hardware commit");
    assert_eq!(machine.state(), &preview.successor);
    assert_eq!(
        machine.journal_revision(),
        preview.proof_statement.journal_revision_after
    );
    assert!(matches!(
        machine.outgoing_candidate_journal.stage(),
        OfflineCashOutgoingJournalStageV1::Committed(existing) if existing == &committed
    ));
    assert_eq!(
        machine.preview_rotate(
            HardwareEpochV1 {
                generation: 2,
                epoch_id: digest(0xE4),
            },
            DevicePolicyBindingV1 {
                device_key_reference: digest(0xE5),
                hardware_policy_id: digest(0xE6),
            },
            digest(0xE7),
            2_000,
        ),
        Err(OfflineCashStateErrorV1::InvalidCandidateStage)
    );
    let committed_snapshot = machine.snapshot().expect("committed snapshot");
    let committed_anchor = machine
        .seal_durability_anchor(vec![0xA1])
        .expect("committed anchor");
    TestMachine::restore(
        committed_snapshot,
        &committed_anchor,
        machine.proof_release,
        AcceptingRecursiveVerifier,
        AcceptingGuardVerifier,
    )
    .expect("committed restore");

    let public_inputs = committed
        .public_wrapper_inputs()
        .expect("terminal public inputs");
    let wrapper_proof = OfflineCashCommitWrapperProofV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        eq_protocol_digest: artifacts().commit_wrapper_eq_protocol_digest,
        ep_protocol_digest: artifacts().commit_wrapper_ep_protocol_digest,
        semantic_digest: public_inputs.semantic_digest,
        candidate_envelope_digest: public_inputs.candidate_envelope_digest,
        commit_certificate_digest: public_inputs.commit_certificate_digest,
        eq_deferred_audit: crate::zk::offline_cash_v1_poseidon::encode(Fp::from(0x71_u64)),
        ep_deferred_audit: crate::zk::offline_cash_v1_poseidon::encode(Fq::from(0x72_u64)),
        eq_proof: vec![0xE8; OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1],
        ep_proof: vec![0xE9; OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1],
        eq_history: eq_history_bytes(),
        ep_history: ep_history_bytes(),
    };
    let finalized = machine
        .finalize_outgoing_candidate(
            wrapper_proof,
            vec![0xEA; OFFLINE_CASH_OUTBOX_RETRY_METADATA_MAX_BYTES_V1 as usize],
        )
        .expect("finalize retry envelope");
    assert_eq!(
        machine.sender_outbox_capacity.committed_outbox_bytes(),
        precommitted_outbox_bytes,
        "a valid final envelope must fit the slot reserved before hardware commit"
    );
    assert!(
        finalized
            .canonical_storage_bytes()
            .expect("canonical durable record bytes")
            <= candidate_lifecycle::implementation_live_outbox_slot_bytes_v1(
                OfflineCashOperationKindV1::RedeemSplit,
            )
            .expect("redemption durable slot bound")
    );
    assert!(matches!(
        machine.outgoing_candidate_journal.stage(),
        OfflineCashOutgoingJournalStageV1::Empty
    ));
    assert_eq!(machine.state(), &preview.successor);
    assert_eq!(
        machine
            .expose_outgoing_candidate(digest(0xE3))
            .expect("retry bytes"),
        finalized.retry_bytes()
    );
    let finalized_snapshot = machine.snapshot().expect("finalized snapshot");
    let finalized_anchor = machine
        .seal_durability_anchor(vec![0xA1])
        .expect("finalized anchor");
    TestMachine::restore(
        finalized_snapshot,
        &finalized_anchor,
        machine.proof_release,
        AcceptingRecursiveVerifier,
        AcceptingGuardVerifier,
    )
    .expect("finalized restore");

    machine
        .release_outgoing_candidate(digest(0xE3), finalized.envelope_digest)
        .expect("release retry envelope");
    assert_eq!(
        machine.sender_outbox_capacity.committed_outbox_bytes(),
        machine.sender_outbox_capacity.retained_metadata_bytes()
    );
    assert!(machine.sender_outbox_capacity.retained_metadata_bytes() > 0);
    assert_eq!(
        machine
            .sender_outbox_capacity
            .reserved_terminal_metadata_bytes(),
        0
    );
    assert_eq!(
        machine.expose_outgoing_candidate(digest(0xE3)),
        Err(OfflineCashStateErrorV1::InvalidCandidateStage)
    );
    machine.snapshot().expect("released snapshot");
}
