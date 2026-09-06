//! Fail-closed verification of recorded device-to-device KAGEMUSHA handoff evidence.
//!
//! This module verifies already-generated recursive proofs and their exact cross-envelope
//! bindings. It never constructs a proof and therefore cannot by itself satisfy the real
//! recursive-handoff qualification corridor.

use std::collections::BTreeSet;

use iroha_data_model::kagemusha::{
    KagemushaPairedProofV1, KagemushaPaymentRequestV1, KagemushaPaymentV1,
    kagemusha_prepared_transfer_digest_v1,
};

use crate::zk::kagemusha_v1_recursion::{
    KagemushaOperationV1, KagemushaRecursionArtifactsV1, KagemushaRecursionErrorV1,
    KagemushaRecursiveVerifierV1, KagemushaStateRelationPublicInputsV1,
    kagemusha_incoming_proof_binding_digest_v1, verify_kagemusha_state_proof_v1,
};

use super::{
    CREDIT_ENVELOPE_DOMAIN, KagemushaStateV1, ReceiveFoldCreditV1, ReceiveFoldV1,
    canonical_sha256_digest,
};

/// One recorded positive-value device-to-device payment handoff.
///
/// The sender proof must be a `SendSplit`, `payment` must be its exact terminal envelope, and the
/// receiver proof must be the `ReceiveFold` which consumes that envelope. This is verifier input,
/// not a proof-generation witness or monetary capability.
#[derive(Clone, Copy, Debug)]
pub struct KagemushaHandoffEvidenceV1<'a> {
    /// Sender's recursively authenticated `SendSplit` public statement.
    pub sender_public_inputs: &'a KagemushaStateRelationPublicInputsV1,
    /// Sender's paired recursive state proof.
    pub sender_state_proof: &'a KagemushaPairedProofV1,
    /// Signed receiver request authenticated by the terminal payment.
    pub payment_request: &'a KagemushaPaymentRequestV1,
    /// Sender's committed terminal payment and paired proof.
    pub payment: &'a KagemushaPaymentV1,
    /// Public receive-credit transcript reconstructed from the durably staged payment.
    pub receive_credit: ReceiveFoldCreditV1,
    /// Receiver's recursively authenticated `ReceiveFold` public statement.
    pub receiver_public_inputs: &'a KagemushaStateRelationPublicInputsV1,
    /// Receiver's paired recursive state proof.
    pub receiver_state_proof: &'a KagemushaPairedProofV1,
}

/// Canonical byte sizes whose equality is required across a verified handoff sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaHandoffEvidenceSizesV1 {
    /// Canonical signed request bytes.
    pub payment_request_bytes: usize,
    /// Canonical complete committed-payment bytes.
    pub payment_bytes: usize,
    /// Canonical sender paired-state-proof bytes.
    pub sender_state_proof_bytes: usize,
    /// Canonical receiver paired-state-proof bytes.
    pub receiver_state_proof_bytes: usize,
}

/// Result of verifying a history-length-independent handoff evidence sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaHandoffSequenceVerificationV1 {
    /// Number of terminally verified positive-value handoffs.
    pub verified_handoffs: usize,
    /// Exact canonical component sizes shared by every verified handoff.
    pub constant_sizes: KagemushaHandoffEvidenceSizesV1,
}

/// Verify one supplied positive-value `SendSplit -> PaymentV1 -> ReceiveFold` handoff.
///
/// This function is deliberately non-generative: success means the configured governed backend
/// terminally accepted both supplied state proofs and the supplied payment proof, and that every
/// overlapping public value was identical. It does not create or extend recursive history.
///
/// # Errors
///
/// Rejects malformed states, a non-payment operation, arithmetic or exact-next violations,
/// request/payment/credit substitution, replay-root non-advancement, artifact substitution, or
/// any governed backend verification failure.
pub fn verify_kagemusha_handoff_evidence_v1<V: KagemushaRecursiveVerifierV1>(
    verifier: &V,
    artifacts: KagemushaRecursionArtifactsV1,
    evidence: KagemushaHandoffEvidenceV1<'_>,
) -> Result<KagemushaHandoffEvidenceSizesV1, KagemushaRecursionErrorV1> {
    let sender = evidence.sender_public_inputs;
    let receiver = evidence.receiver_public_inputs;
    let sender_predecessor = monetary_predecessor(sender, KagemushaOperationV1::SendSplit)?;
    let receiver_predecessor = monetary_predecessor(receiver, KagemushaOperationV1::ReceiveFold)?;

    validate_send_transition(sender_predecessor, &sender.successor, sender.amount)?;
    validate_receive_transition(receiver_predecessor, &receiver.successor, receiver.amount)?;
    validate_common_asset_context(&sender.successor, &receiver.successor)?;
    validate_receiver_request_context(&receiver.successor, evidence.payment_request)?;
    validate_artifact_bindings(artifacts, sender, receiver, evidence.payment)?;

    evidence
        .payment
        .validate_shape_against(evidence.payment_request)
        .map_err(|error| KagemushaRecursionErrorV1::TransportBinding(error.to_string()))?;
    let payment = evidence.payment;
    let request = evidence.payment_request;
    let request_digest = request
        .canonical_digest()
        .map_err(|error| KagemushaRecursionErrorV1::TransportBinding(error.to_string()))?;
    let prepared_transfer_digest = kagemusha_prepared_transfer_digest_v1(
        request,
        payment.output.sender_before_commitment,
        payment.output.sender_after_commitment,
        payment.output.transition_nullifier,
        payment.output.ciphertext_commitment,
    )
    .map_err(|error| KagemushaRecursionErrorV1::TransportBinding(error.to_string()))?;

    if sender.amount != request.amount
        || receiver.amount != request.amount
        || payment.output.amount != request.amount
        || payment.output.request_digest != request_digest
        || payment.output.sender_before_commitment != sender_predecessor.state_commitment
        || payment.output.sender_after_commitment != sender.successor.state_commitment
        || sender.peer_credit_id != payment.output.credit_id
        || sender.recipient_encryption_key_binding != request.recipient_encryption_key
        || sender.receive_credit_binding_digest != [0; 32]
        || sender.prepared_transition_binding_digest != prepared_transfer_digest
        || sender.lifecycle_binding_digest != payment.commit_certificate.lifecycle_binding_digest
        || payment.commit_certificate.hardware_profile_id != sender.successor.hardware_profile_id
        || payment.commit_certificate.policy_epoch != sender.successor.policy_epoch
    {
        return Err(binding_error(
            "sender transition and terminal payment disagree",
        ));
    }

    let expected_envelope_digest = canonical_sha256_digest(CREDIT_ENVELOPE_DOMAIN, payment)
        .map_err(|error| KagemushaRecursionErrorV1::Codec(error.to_string()))?;
    let expected_output_digest = payment
        .output
        .canonical_digest_against(request)
        .map_err(|error| KagemushaRecursionErrorV1::TransportBinding(error.to_string()))?;
    let expected_incoming_binding = kagemusha_incoming_proof_binding_digest_v1(request, payment)?;
    let receive_credit = evidence.receive_credit;
    let receive_fold = ReceiveFoldV1::try_new(receive_credit)
        .map_err(|error| KagemushaRecursionErrorV1::TransportBinding(error.to_string()))?;
    if receive_credit.amount != request.amount
        || receive_credit.credit_id.0 != payment.output.credit_id
        || receive_credit.recipient_lane_id != receiver.successor.lane.device_lane_id
        || receive_credit.incoming_proof_binding_digest != expected_incoming_binding
        || receive_credit.receiver_binding_digest != request.hardware_credential.credential_id
        || receive_credit.payment_output_digest != expected_output_digest
        || receive_credit.envelope_digest != expected_envelope_digest
        || receiver.peer_credit_id != [0; 32]
        || receiver.recipient_encryption_key_binding != [0; 32]
        || receiver.prepared_transition_binding_digest != [0; 32]
        || receiver.receive_credit_binding_digest != receive_fold.canonical_transcript_digest()
    {
        return Err(binding_error("receiver fold and staged payment disagree"));
    }

    verify_kagemusha_state_proof_v1(verifier, artifacts, sender, evidence.sender_state_proof)?;
    verifier
        .verify_payment_and_decide(request, payment)
        .map_err(KagemushaRecursionErrorV1::PaymentProofRejected)?;
    verify_kagemusha_state_proof_v1(verifier, artifacts, receiver, evidence.receiver_state_proof)?;

    canonical_sizes(evidence)
}

/// Verify an arbitrary-length chain of supplied handoff evidence with exact size invariance.
///
/// Every handoff is independently terminally verified. For each later handoff, its sender
/// predecessor must be byte-for-byte the prior receiver successor, proving that the same aggregate
/// value—not a rotation-only or model-only state—continues through the chain. No history-count cap
/// is applied. The checker retains only ordinary local bookkeeping, so physical memory remains the
/// only sequence-length bound.
///
/// This verifier does not generate proofs. A qualification run must first generate real recursive
/// SendSplit and ReceiveFold proofs and then pass those exact artifacts here.
///
/// # Errors
///
/// Rejects an empty sequence, any invalid handoff, reused credit identity, broken aggregate-state
/// continuity, or any canonical component-size drift.
pub fn verify_kagemusha_handoff_evidence_sequence_v1<V: KagemushaRecursiveVerifierV1>(
    verifier: &V,
    artifacts: KagemushaRecursionArtifactsV1,
    evidence: &[KagemushaHandoffEvidenceV1<'_>],
) -> Result<KagemushaHandoffSequenceVerificationV1, KagemushaRecursionErrorV1> {
    let (first, rest) = evidence
        .split_first()
        .ok_or_else(|| binding_error("handoff evidence sequence is empty"))?;
    let constant_sizes = verify_kagemusha_handoff_evidence_v1(verifier, artifacts, *first)?;
    let mut prior_receiver_successor = &first.receiver_public_inputs.successor;
    let mut credit_ids = BTreeSet::new();
    credit_ids.insert(first.payment.output.credit_id);

    for handoff in rest {
        let sender_predecessor = handoff
            .sender_public_inputs
            .predecessor
            .as_ref()
            .ok_or_else(|| binding_error("handoff sender predecessor is absent"))?;
        if sender_predecessor != prior_receiver_successor {
            return Err(binding_error(
                "handoff sequence does not continue from the prior receiver successor",
            ));
        }
        if !credit_ids.insert(handoff.payment.output.credit_id) {
            return Err(binding_error("handoff sequence reuses a credit identity"));
        }
        let sizes = verify_kagemusha_handoff_evidence_v1(verifier, artifacts, *handoff)?;
        if sizes != constant_sizes {
            return Err(binding_error(
                "canonical handoff component sizes changed with recursive history",
            ));
        }
        prior_receiver_successor = &handoff.receiver_public_inputs.successor;
    }

    Ok(KagemushaHandoffSequenceVerificationV1 {
        verified_handoffs: evidence.len(),
        constant_sizes,
    })
}

fn monetary_predecessor(
    public: &KagemushaStateRelationPublicInputsV1,
    expected_operation: KagemushaOperationV1,
) -> Result<&KagemushaStateV1, KagemushaRecursionErrorV1> {
    if public.operation != expected_operation || public.amount == 0 {
        return Err(binding_error(
            "handoff evidence is not the required positive-value operation",
        ));
    }
    if public.journal_revision_after
        != public
            .journal_revision_before
            .checked_add(1)
            .ok_or(KagemushaRecursionErrorV1::JournalOverflow)?
    {
        return Err(KagemushaRecursionErrorV1::NonExactSuccessor);
    }
    public
        .predecessor
        .as_ref()
        .ok_or_else(|| binding_error("monetary transition predecessor is absent"))
}

fn validate_send_transition(
    predecessor: &KagemushaStateV1,
    successor: &KagemushaStateV1,
    amount: u128,
) -> Result<(), KagemushaRecursionErrorV1> {
    validate_unchanged_lane_context(predecessor, successor)?;
    if predecessor.consumed_credit_root != successor.consumed_credit_root
        || successor.balance
            != predecessor
                .balance
                .checked_sub(amount)
                .ok_or_else(|| binding_error("sender balance is insufficient"))?
    {
        return Err(binding_error("invalid SendSplit state relation"));
    }
    Ok(())
}

fn validate_receive_transition(
    predecessor: &KagemushaStateV1,
    successor: &KagemushaStateV1,
    amount: u128,
) -> Result<(), KagemushaRecursionErrorV1> {
    validate_unchanged_lane_context(predecessor, successor)?;
    if predecessor.consumed_credit_root == successor.consumed_credit_root
        || successor.balance
            != predecessor
                .balance
                .checked_add(amount)
                .ok_or_else(|| binding_error("receiver balance overflow"))?
    {
        return Err(binding_error("invalid ReceiveFold state relation"));
    }
    Ok(())
}

fn validate_unchanged_lane_context(
    predecessor: &KagemushaStateV1,
    successor: &KagemushaStateV1,
) -> Result<(), KagemushaRecursionErrorV1> {
    predecessor
        .validate()
        .map_err(|error| KagemushaRecursionErrorV1::StateStatement(error.to_string()))?;
    successor
        .validate()
        .map_err(|error| KagemushaRecursionErrorV1::StateStatement(error.to_string()))?;
    if predecessor.context() != successor.context()
        || predecessor.lane != successor.lane
        || predecessor.hardware_epoch != successor.hardware_epoch
        || predecessor.device_policy_binding != successor.device_policy_binding
        || predecessor.state_nonce_commitment == successor.state_nonce_commitment
        || predecessor.state_commitment == successor.state_commitment
        || successor.logical_sequence
            != predecessor
                .logical_sequence
                .checked_add(1)
                .ok_or(KagemushaRecursionErrorV1::SequenceOverflow)?
    {
        return Err(KagemushaRecursionErrorV1::NonExactSuccessor);
    }
    Ok(())
}

fn validate_common_asset_context(
    sender: &KagemushaStateV1,
    receiver: &KagemushaStateV1,
) -> Result<(), KagemushaRecursionErrorV1> {
    if sender.protocol_version != receiver.protocol_version
        || sender.suite_id != receiver.suite_id
        || sender.vk_digest != receiver.vk_digest
        || sender.release_id != receiver.release_id
        || sender.asset_incarnation != receiver.asset_incarnation
        || sender.liability_pool_id != receiver.liability_pool_id
        || sender.lane.network_id != receiver.lane.network_id
        || sender.lane.asset != receiver.lane.asset
        || sender.lane.scale != receiver.lane.scale
        || sender.lane.device_lane_id == receiver.lane.device_lane_id
    {
        return Err(binding_error("sender and receiver asset contexts disagree"));
    }
    Ok(())
}

fn validate_receiver_request_context(
    receiver: &KagemushaStateV1,
    request: &KagemushaPaymentRequestV1,
) -> Result<(), KagemushaRecursionErrorV1> {
    request
        .validate_shape()
        .map_err(|error| KagemushaRecursionErrorV1::TransportBinding(error.to_string()))?;
    let credential = &request.hardware_credential;
    if request.release_id != receiver.release_id
        || request.network_id != receiver.lane.network_id
        || request.asset != receiver.lane.asset
        || request.asset_incarnation != receiver.asset_incarnation
        || request.scale != receiver.lane.scale
        || request.liability_pool_id != receiver.liability_pool_id
        || credential.lane_commitment != receiver.lane.device_lane_id
        || credential.hardware_profile_id != receiver.hardware_profile_id
        || credential.suite_id != receiver.suite_id
        || credential.policy_epoch != receiver.policy_epoch
    {
        return Err(binding_error(
            "payment request does not name the receiver lane",
        ));
    }
    Ok(())
}

fn validate_artifact_bindings(
    artifacts: KagemushaRecursionArtifactsV1,
    sender: &KagemushaStateRelationPublicInputsV1,
    receiver: &KagemushaStateRelationPublicInputsV1,
    payment: &KagemushaPaymentV1,
) -> Result<(), KagemushaRecursionErrorV1> {
    let state_bindings_match = |public: &KagemushaStateRelationPublicInputsV1| {
        public.successor.release_id == artifacts.release_id
            && public.eq_protocol_digest == artifacts.eq_protocol_digest
            && public.ep_protocol_digest == artifacts.ep_protocol_digest
            && public.guard_eq_protocol_digest == artifacts.guard_bundle_eq_protocol_digest
            && public.guard_ep_protocol_digest == artifacts.guard_bundle_ep_protocol_digest
            && public.mint_eq_protocol_digest == artifacts.mint_finality_eq_protocol_digest
            && public.mint_ep_protocol_digest == artifacts.mint_finality_ep_protocol_digest
            && public.mint_authorization_eq_protocol_digest
                == artifacts.mint_authorization_eq_protocol_digest
            && public.mint_authorization_ep_protocol_digest
                == artifacts.mint_authorization_ep_protocol_digest
            && public.commit_wrapper_eq_protocol_digest
                == artifacts.commit_wrapper_eq_protocol_digest
            && public.commit_wrapper_ep_protocol_digest
                == artifacts.commit_wrapper_ep_protocol_digest
    };
    if !state_bindings_match(sender)
        || !state_bindings_match(receiver)
        || payment.proof.eq_protocol_digest != artifacts.commit_wrapper_eq_protocol_digest
        || payment.proof.ep_protocol_digest != artifacts.commit_wrapper_ep_protocol_digest
    {
        return Err(KagemushaRecursionErrorV1::ArtifactSubstitution);
    }
    Ok(())
}

fn canonical_sizes(
    evidence: KagemushaHandoffEvidenceV1<'_>,
) -> Result<KagemushaHandoffEvidenceSizesV1, KagemushaRecursionErrorV1> {
    fn encoded_len<T: norito::codec::Encode>(
        value: &T,
    ) -> Result<usize, KagemushaRecursionErrorV1> {
        norito::encode_canonical(value)
            .map(|bytes| bytes.len())
            .map_err(|error| KagemushaRecursionErrorV1::Codec(error.to_string()))
    }
    Ok(KagemushaHandoffEvidenceSizesV1 {
        payment_request_bytes: encoded_len(evidence.payment_request)?,
        payment_bytes: encoded_len(evidence.payment)?,
        sender_state_proof_bytes: encoded_len(evidence.sender_state_proof)?,
        receiver_state_proof_bytes: encoded_len(evidence.receiver_state_proof)?,
    })
}

fn binding_error(reason: &'static str) -> KagemushaRecursionErrorV1 {
    KagemushaRecursionErrorV1::TransportBinding(reason.to_owned())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use iroha_data_model::kagemusha::{
        KAGEMUSHA_WIRE_VERSION_V1, KagemushaDevicePublicKeyV1, KagemushaPairedProofV1,
        KagemushaPastaStateCommitmentV1, KagemushaPaymentRequestV1, KagemushaPaymentV1,
        kagemusha_device_key_reference_v1, kagemusha_payment_body_digest_v1,
        kagemusha_prepared_transfer_digest_v1,
    };

    use crate::zk::{
        kagemusha_v1_recursion::{
            KagemushaMintFinalityHelperVerificationRequestV1, KagemushaParityVerificationRequestV1,
            KagemushaRecursionArtifactsV1, KagemushaRecursiveVerifierV1,
            KagemushaStateProofVerificationRequestV1, KagemushaStateRelationPublicInputsV1,
            tests::{
                artifacts, ep_history, eq_history, incoming_payment_fixture, p256_signing_key, sign,
            },
        },
        kagemusha_v1_state::{
            CreditIdV1, DevicePolicyBindingV1, HardwareEpochV1, KAGEMUSHA_STATE_VERSION_V1,
            KagemushaLaneIdV1, KagemushaStateContextV1,
        },
    };

    use super::*;

    #[derive(Default)]
    struct RecordingVerifier {
        state_calls: Cell<usize>,
        payment_calls: Cell<usize>,
    }

    impl KagemushaRecursiveVerifierV1 for RecordingVerifier {
        fn verify_state_proof_and_decide(
            &self,
            _request: &KagemushaStateProofVerificationRequestV1<'_>,
        ) -> Result<(), String> {
            self.state_calls.set(self.state_calls.get() + 1);
            Ok(())
        }

        fn verify_payment_and_decide(
            &self,
            _request: &KagemushaPaymentRequestV1,
            _payment: &KagemushaPaymentV1,
        ) -> Result<(), String> {
            self.payment_calls.set(self.payment_calls.get() + 1);
            Ok(())
        }

        fn verify_mint_finality_helper(
            &self,
            _request: &KagemushaMintFinalityHelperVerificationRequestV1<'_>,
        ) -> Result<(), String> {
            Err("handoff fixture has no mint proof".to_owned())
        }

        fn verify_terminal_authorization_and_decide(
            &self,
            _request: &KagemushaParityVerificationRequestV1<'_>,
        ) -> Result<(), String> {
            Err("handoff fixture has no standalone terminal proof".to_owned())
        }
    }

    struct OwnedHandoff {
        sender_public: KagemushaStateRelationPublicInputsV1,
        sender_proof: KagemushaPairedProofV1,
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
        receive_credit: ReceiveFoldCreditV1,
        receiver_public: KagemushaStateRelationPublicInputsV1,
        receiver_proof: KagemushaPairedProofV1,
    }

    impl OwnedHandoff {
        fn evidence(&self) -> KagemushaHandoffEvidenceV1<'_> {
            KagemushaHandoffEvidenceV1 {
                sender_public_inputs: &self.sender_public,
                sender_state_proof: &self.sender_proof,
                payment_request: &self.request,
                payment: &self.payment,
                receive_credit: self.receive_credit,
                receiver_public_inputs: &self.receiver_public,
                receiver_state_proof: &self.receiver_proof,
            }
        }
    }

    fn digest(tag: u8) -> [u8; 32] {
        let mut digest = [0; 32];
        digest[0] = tag;
        digest
    }

    fn replay_root(tag: u8) -> KagemushaPastaStateCommitmentV1 {
        KagemushaPastaStateCommitmentV1 {
            eq: digest(tag),
            ep: digest(tag.wrapping_add(1)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn state(
        request: &KagemushaPaymentRequestV1,
        lane_id: [u8; 32],
        hardware_profile_id: [u8; 32],
        policy_epoch: u64,
        device_key_reference: [u8; 32],
        balance: u128,
        sequence: u128,
        replay_root: KagemushaPastaStateCommitmentV1,
        nonce: [u8; 32],
    ) -> KagemushaStateV1 {
        KagemushaStateV1::build(
            KagemushaStateContextV1 {
                protocol_version: KAGEMUSHA_STATE_VERSION_V1,
                suite_id: request.hardware_credential.suite_id,
                vk_digest: digest(0x31),
                release_id: request.release_id,
                asset_incarnation: request.asset_incarnation,
                hardware_profile_id,
                policy_epoch,
            },
            request.liability_pool_id,
            KagemushaLaneIdV1 {
                network_id: request.network_id,
                device_lane_id: lane_id,
                asset: request.asset.clone(),
                scale: request.scale,
            },
            balance,
            sequence,
            HardwareEpochV1 {
                generation: 1,
                epoch_id: digest(lane_id[0].wrapping_add(0x40)),
            },
            DevicePolicyBindingV1 {
                device_key_reference,
                hardware_policy_id: digest(lane_id[0].wrapping_add(0x50)),
            },
            nonce,
            replay_root,
        )
        .expect("valid handoff fixture state")
    }

    fn retarget_request(
        mut request: KagemushaPaymentRequestV1,
        receiver: &KagemushaStateV1,
        key_seed: u8,
        request_tag: u8,
    ) -> KagemushaPaymentRequestV1 {
        let device_key = p256_signing_key(key_seed);
        let device_public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            device_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        )
        .expect("canonical device key");
        request.release_id = receiver.release_id;
        request.network_id = receiver.lane.network_id;
        request.asset = receiver.lane.asset.clone();
        request.asset_incarnation = receiver.asset_incarnation;
        request.scale = receiver.lane.scale;
        request.liability_pool_id = receiver.liability_pool_id;
        request.request_id = digest(request_tag);
        request.hardware_credential.network_id = receiver.lane.network_id;
        request.hardware_credential.hardware_profile_id = receiver.hardware_profile_id;
        request.hardware_credential.suite_id = receiver.suite_id;
        request.hardware_credential.policy_epoch = receiver.policy_epoch;
        request.hardware_credential.lane_commitment = receiver.lane.device_lane_id;
        request.hardware_credential.hardware_epoch_id = receiver.hardware_epoch.epoch_id;
        request.hardware_credential.hardware_epoch_generation =
            u64::try_from(receiver.hardware_epoch.generation).expect("fixture epoch fits u64");
        request.hardware_credential.device_public_key = device_public_key;
        request.hardware_credential.device_key_reference =
            kagemusha_device_key_reference_v1(&device_public_key);
        request.hardware_credential = request
            .hardware_credential
            .seal_credential_id()
            .expect("credential identity");
        let governance_key = p256_signing_key(8);
        request.hardware_credential.governance_signature = sign(
            &governance_key,
            &request
                .hardware_credential
                .canonical_signing_bytes()
                .expect("credential signing bytes"),
        );
        request.signature = sign(
            &device_key,
            &request
                .canonical_signing_bytes()
                .expect("request signing bytes"),
        );
        request.validate_shape().expect("valid retargeted request");
        request
    }

    fn public_inputs(
        artifacts: KagemushaRecursionArtifactsV1,
        operation: KagemushaOperationV1,
        predecessor: KagemushaStateV1,
        successor: KagemushaStateV1,
        amount: u128,
        journal_revision_before: u128,
        lifecycle_binding_digest: [u8; 32],
        prepared_transition_binding_digest: [u8; 32],
        peer_credit_id: [u8; 32],
        recipient_encryption_key_binding: [u8; 32],
        receive_credit_binding_digest: [u8; 32],
        tag: u8,
    ) -> KagemushaStateRelationPublicInputsV1 {
        KagemushaStateRelationPublicInputsV1 {
            operation,
            predecessor: Some(predecessor),
            successor,
            amount,
            journal_revision_before,
            journal_revision_after: journal_revision_before + 1,
            transition_effect_digest: digest(tag.wrapping_add(1)),
            mint_finality_semantic_digest: [0; 32],
            mint_finality_proof_binding_digest: [0; 32],
            peer_credit_id,
            recipient_encryption_key_binding,
            receive_credit_binding_digest,
            lifecycle_binding_digest,
            prepared_transition_binding_digest,
            transport_semantic_digest: digest(tag.wrapping_add(2)),
            guard_statement_digest: digest(tag.wrapping_add(3)),
            eq_protocol_digest: artifacts.eq_protocol_digest,
            ep_protocol_digest: artifacts.ep_protocol_digest,
            guard_eq_protocol_digest: artifacts.guard_bundle_eq_protocol_digest,
            guard_ep_protocol_digest: artifacts.guard_bundle_ep_protocol_digest,
            mint_eq_protocol_digest: artifacts.mint_finality_eq_protocol_digest,
            mint_ep_protocol_digest: artifacts.mint_finality_ep_protocol_digest,
            mint_authorization_eq_protocol_digest: artifacts.mint_authorization_eq_protocol_digest,
            mint_authorization_ep_protocol_digest: artifacts.mint_authorization_ep_protocol_digest,
            commit_wrapper_eq_protocol_digest: artifacts.commit_wrapper_eq_protocol_digest,
            commit_wrapper_ep_protocol_digest: artifacts.commit_wrapper_ep_protocol_digest,
            guard_eq_credential_audit: digest(tag.wrapping_add(4)),
            guard_ep_credential_audit: digest(tag.wrapping_add(5)),
            eq_deferred_audit: digest(tag.wrapping_add(6)),
            ep_deferred_audit: digest(tag.wrapping_add(7)),
        }
    }

    fn state_proof(
        public: &KagemushaStateRelationPublicInputsV1,
        artifacts: KagemushaRecursionArtifactsV1,
        tag: u8,
    ) -> KagemushaPairedProofV1 {
        KagemushaPairedProofV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            eq_protocol_digest: artifacts.eq_protocol_digest,
            ep_protocol_digest: artifacts.ep_protocol_digest,
            semantic_digest: public.transport_semantic_digest,
            guard_eq_credential_audit: public.guard_eq_credential_audit,
            guard_ep_credential_audit: public.guard_ep_credential_audit,
            eq_deferred_audit: public.eq_deferred_audit,
            ep_deferred_audit: public.ep_deferred_audit,
            eq_proof: vec![tag; 64],
            ep_proof: vec![tag.wrapping_add(1); 64],
            eq_history: eq_history(u64::from(tag)).as_bytes().to_vec(),
            ep_history: ep_history(u64::from(tag) + 1).as_bytes().to_vec(),
        }
    }

    fn handoff(
        artifacts: KagemushaRecursionArtifactsV1,
        request: KagemushaPaymentRequestV1,
        mut payment: KagemushaPaymentV1,
        sender_predecessor: KagemushaStateV1,
        sender_successor: KagemushaStateV1,
        receiver_predecessor: KagemushaStateV1,
        receiver_successor: KagemushaStateV1,
        journal_revision: u128,
        tag: u8,
    ) -> OwnedHandoff {
        payment.output.request_digest = request.canonical_digest().expect("request digest");
        payment.output.sender_before_commitment = sender_predecessor.state_commitment;
        payment.output.sender_after_commitment = sender_successor.state_commitment;
        payment.output.transition_nullifier = digest(tag.wrapping_add(0x70));
        payment.output = payment
            .output
            .seal_credit_id_against(&request)
            .expect("credit identity");
        payment.commit_certificate.transition_nullifier = payment.output.transition_nullifier;
        payment.commit_certificate.commit_evidence = payment.output.commit_evidence;
        payment.commit_certificate.hardware_profile_id = sender_successor.hardware_profile_id;
        payment.commit_certificate.policy_epoch = sender_successor.policy_epoch;
        payment.commit_certificate = payment
            .commit_certificate
            .seal_certificate_id()
            .expect("certificate identity");
        payment.proof.semantic_digest =
            kagemusha_payment_body_digest_v1(&payment.output, &payment.encrypted_credit)
                .expect("payment body digest");
        payment.proof.commit_certificate_digest = payment
            .commit_certificate
            .canonical_digest()
            .expect("certificate digest");
        payment
            .validate_shape_against(&request)
            .expect("valid handoff payment");

        let receive_credit = ReceiveFoldCreditV1 {
            amount: request.amount,
            credit_id: CreditIdV1(payment.output.credit_id),
            recipient_lane_id: receiver_successor.lane.device_lane_id,
            incoming_proof_binding_digest: kagemusha_incoming_proof_binding_digest_v1(
                &request, &payment,
            )
            .expect("incoming payment binding"),
            receiver_binding_digest: request.hardware_credential.credential_id,
            payment_output_digest: payment
                .output
                .canonical_digest_against(&request)
                .expect("payment output digest"),
            envelope_digest: canonical_sha256_digest(CREDIT_ENVELOPE_DOMAIN, &payment)
                .expect("payment envelope digest"),
        };
        let receive_credit_binding_digest = ReceiveFoldV1::try_new(receive_credit)
            .expect("receive fold")
            .canonical_transcript_digest();
        let prepared_transfer_digest = kagemusha_prepared_transfer_digest_v1(
            &request,
            payment.output.sender_before_commitment,
            payment.output.sender_after_commitment,
            payment.output.transition_nullifier,
            payment.output.ciphertext_commitment,
        )
        .expect("prepared transfer digest");
        let sender_public = public_inputs(
            artifacts,
            KagemushaOperationV1::SendSplit,
            sender_predecessor,
            sender_successor,
            request.amount,
            journal_revision,
            payment.commit_certificate.lifecycle_binding_digest,
            prepared_transfer_digest,
            payment.output.credit_id,
            request.recipient_encryption_key,
            [0; 32],
            tag,
        );
        let receiver_public = public_inputs(
            artifacts,
            KagemushaOperationV1::ReceiveFold,
            receiver_predecessor,
            receiver_successor,
            request.amount,
            journal_revision,
            digest(tag.wrapping_add(0x30)),
            [0; 32],
            [0; 32],
            [0; 32],
            receive_credit_binding_digest,
            tag.wrapping_add(8),
        );
        let sender_proof = state_proof(&sender_public, artifacts, tag.wrapping_add(0x10));
        let receiver_proof = state_proof(&receiver_public, artifacts, tag.wrapping_add(0x20));
        OwnedHandoff {
            sender_public,
            sender_proof,
            request,
            payment,
            receive_credit,
            receiver_public,
            receiver_proof,
        }
    }

    fn two_handoff_fixture() -> (KagemushaRecursionArtifactsV1, OwnedHandoff, OwnedHandoff) {
        let artifacts = artifacts();
        let template = incoming_payment_fixture(0x41, 9, 7, 11, 128, 128);
        let request_b = template.request;
        let lane_a = digest(0x61);
        let lane_b = request_b.hardware_credential.lane_commitment;
        let key_a = p256_signing_key(9);
        let public_key_a = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            key_a.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .expect("canonical A key");
        let key_ref_a = kagemusha_device_key_reference_v1(&public_key_a);
        let key_ref_b = request_b.hardware_credential.device_key_reference;

        let a0 = state(
            &request_b,
            lane_a,
            template.payment.commit_certificate.hardware_profile_id,
            template.payment.commit_certificate.policy_epoch,
            key_ref_a,
            7,
            0,
            replay_root(1),
            digest(0x81),
        );
        let a1 = state(
            &request_b,
            lane_a,
            a0.hardware_profile_id,
            a0.policy_epoch,
            key_ref_a,
            0,
            1,
            replay_root(1),
            digest(0x82),
        );
        let b0 = state(
            &request_b,
            lane_b,
            request_b.hardware_credential.hardware_profile_id,
            request_b.hardware_credential.policy_epoch,
            key_ref_b,
            0,
            0,
            replay_root(3),
            digest(0x83),
        );
        let b1 = state(
            &request_b,
            lane_b,
            b0.hardware_profile_id,
            b0.policy_epoch,
            key_ref_b,
            7,
            1,
            replay_root(5),
            digest(0x84),
        );
        let first = handoff(
            artifacts,
            request_b.clone(),
            template.payment.clone(),
            a0,
            a1.clone(),
            b0,
            b1.clone(),
            0,
            1,
        );

        let b2 = state(
            &request_b,
            lane_b,
            b1.hardware_profile_id,
            b1.policy_epoch,
            key_ref_b,
            0,
            2,
            replay_root(5),
            digest(0x85),
        );
        let a2 = state(
            &request_b,
            lane_a,
            a1.hardware_profile_id,
            a1.policy_epoch,
            key_ref_a,
            7,
            2,
            replay_root(7),
            digest(0x86),
        );
        let request_a = retarget_request(request_b, &a1, 9, 0x29);
        let second = handoff(artifacts, request_a, template.payment, b1, b2, a1, a2, 1, 2);
        (artifacts, first, second)
    }

    #[test]
    fn verifies_exact_positive_value_handoff_and_dispatches_all_three_proofs() {
        let (artifacts, first, _) = two_handoff_fixture();
        let verifier = RecordingVerifier::default();
        let sizes = verify_kagemusha_handoff_evidence_v1(&verifier, artifacts, first.evidence())
            .expect("valid recorded handoff evidence");
        assert_eq!(verifier.state_calls.get(), 2);
        assert_eq!(verifier.payment_calls.get(), 1);
        assert!(sizes.payment_request_bytes > 0);
        assert!(sizes.payment_bytes > sizes.payment_request_bytes);
        assert_eq!(
            sizes.sender_state_proof_bytes,
            sizes.receiver_state_proof_bytes
        );
    }

    #[test]
    fn rejects_receive_envelope_substitution_before_cryptographic_dispatch() {
        let (artifacts, mut first, _) = two_handoff_fixture();
        first.receive_credit.envelope_digest[0] ^= 1;
        let verifier = RecordingVerifier::default();
        assert!(
            verify_kagemusha_handoff_evidence_v1(&verifier, artifacts, first.evidence()).is_err()
        );
        assert_eq!(verifier.state_calls.get(), 0);
        assert_eq!(verifier.payment_calls.get(), 0);
    }

    #[test]
    fn verifies_continuous_sequence_and_rejects_size_or_state_discontinuity() {
        let (artifacts, first, second) = two_handoff_fixture();
        let evidence = [first.evidence(), second.evidence()];
        let verifier = RecordingVerifier::default();
        let verified =
            verify_kagemusha_handoff_evidence_sequence_v1(&verifier, artifacts, &evidence)
                .expect("continuous constant-size handoff sequence");
        assert_eq!(verified.verified_handoffs, 2);
        assert_eq!(verifier.state_calls.get(), 4);
        assert_eq!(verifier.payment_calls.get(), 2);

        let broken = [first.evidence(), first.evidence()];
        let verifier = RecordingVerifier::default();
        assert!(
            verify_kagemusha_handoff_evidence_sequence_v1(&verifier, artifacts, &broken).is_err()
        );
        assert_eq!(verifier.state_calls.get(), 2);
        assert_eq!(verifier.payment_calls.get(), 1);
    }
}
