//! Single-credit host preparation for aggregate receive folds.

use norito::codec::Encode;

use super::{
    ConsumedCreditInsertWitnessV1, CreditIdV1, DigestV1, KagemushaGuardBundleVerifierV1,
    KagemushaRecursiveVerifierV1, KagemushaStateErrorV1, KagemushaStateMachineV1,
    KagemushaStateV1, KagemushaTransitionKindV1, TransitionAuthorizationV1,
    TransitionAuxiliaryBindingsV1, TransitionPreviewV1, canonical_sha256_digest,
    local_transition_transport_digest, kagemusha_incoming_proof_binding_digest_v1,
    peer_payment_public_output, receiver_snapshot_capacity_usage_v1,
    sparse_merkle::PreparedConsumedCreditBatchV1,
};

/// Exact staged-credit material consumed by one `ReceiveFold` transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceiveFoldCreditPreviewV1 {
    /// Positive credit amount in atomic units.
    pub amount: u128,
    /// Exact receiver-bound credit identity.
    pub credit_id: CreditIdV1,
    /// Recipient lane committed by the incoming sender proof.
    pub recipient_lane_id: DigestV1,
    /// Binding of the exact incoming paired sender proof and history.
    pub incoming_proof_binding_digest: DigestV1,
    /// Digest of the exact canonical payment envelope staged in the durable inbox.
    pub envelope_digest: DigestV1,
    /// Exact empty-to-present replay insertion consumed by the recursive relation.
    pub replay_insert_witness: ConsumedCreditInsertWitnessV1,
}

/// One single-credit receive transition plus its opaque atomic replay-tree plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceiveFoldPreviewV1 {
    /// Deterministic public transition statements and expected aggregate successor.
    pub transition: TransitionPreviewV1,
    /// Exact staged credit consumed by this transition.
    pub credit: ReceiveFoldCreditPreviewV1,
    trusted_commit_time_ms: u64,
    prepared_replay: PreparedConsumedCreditBatchV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode)]
struct ReceiveFoldEffectV1 {
    amount: u128,
    credit_id: CreditIdV1,
    recipient_lane_id: DigestV1,
    incoming_proof_binding_digest: DigestV1,
    envelope_digest: DigestV1,
}

impl<R, G> KagemushaStateMachineV1<R, G>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
{
    /// Preview folding exactly one durably staged credit into the aggregate successor.
    ///
    /// A wallet drains any backlog by invoking this fixed-shape operation repeatedly. There is
    /// no protocol count, fan-in, or history-depth admission limit.
    pub fn preview_receive_fold(
        &self,
        credit_id: CreditIdV1,
        successor_state_nonce_commitment: DigestV1,
        trusted_commit_time_ms: u64,
    ) -> Result<ReceiveFoldPreviewV1, KagemushaStateErrorV1> {
        let staged = self
            .pending_credits
            .get(&credit_id)
            .ok_or(KagemushaStateErrorV1::CreditNotStaged(credit_id))?;
        let balance = self
            .state
            .balance
            .checked_add(staged.payment.statement.amount)
            .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)?;
        let prepared_replay = self
            .consumed_credits
            .prepare_batch_inserts(&[(credit_id, staged.envelope_digest)])?;
        let replay_insert_witness = prepared_replay
            .witness(0)
            .ok_or(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness)?
            .clone();
        let sender_output = peer_payment_public_output(&staged.request, &staged.payment)?;
        let incoming_proof_binding_digest =
            kagemusha_incoming_proof_binding_digest_v1(&sender_output, &staged.payment.proof)
                .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        let credit = ReceiveFoldCreditPreviewV1 {
            amount: staged.payment.statement.amount,
            credit_id,
            recipient_lane_id: self.state.lane.device_lane_id,
            incoming_proof_binding_digest,
            envelope_digest: staged.envelope_digest,
            replay_insert_witness,
        };
        let successor = self.next_state(
            balance,
            self.state.hardware_epoch,
            self.state.device_policy_binding,
            successor_state_nonce_commitment,
            prepared_replay.final_root(),
        )?;
        let effect_digest = receive_fold_effect_digest(&credit)?;
        let successor_commitment = successor.state_commitment;
        let transition = self.transition_preview(
            KagemushaTransitionKindV1::ReceiveFold,
            successor,
            effect_digest,
            [0; 32],
            [0; 32],
            credit_id.0,
            credit.recipient_lane_id,
            TransitionAuxiliaryBindingsV1::default(),
            trusted_commit_time_ms,
            |normalized_guard_statement_digest| {
                local_transition_transport_digest(
                    KagemushaTransitionKindV1::ReceiveFold,
                    self.state.release_id,
                    self.state.liability_pool_id,
                    effect_digest,
                    self.state.state_commitment,
                    successor_commitment,
                    normalized_guard_statement_digest,
                )
            },
        )?;

        Ok(ReceiveFoldPreviewV1 {
            transition,
            credit,
            trusted_commit_time_ms,
            prepared_replay,
        })
    }

    /// Verify and atomically install one previously prepared single-credit receive fold.
    ///
    /// The staged envelope, sender proof binding, replay insertion, transition, and capacity
    /// conversion are rederived against the current state before anything is mutated.
    pub fn receive_fold_prepared(
        &mut self,
        preview: ReceiveFoldPreviewV1,
        authorization: TransitionAuthorizationV1,
    ) -> Result<KagemushaStateV1, KagemushaStateErrorV1> {
        if preview.prepared_replay.len() != 1
            || preview.prepared_replay.starting_root() != self.consumed_credits.root()
            || preview.prepared_replay.final_root()
                != preview.transition.successor.consumed_credit_root
        {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }
        let staged = self.pending_credits.get(&preview.credit.credit_id).ok_or(
            KagemushaStateErrorV1::CreditNotStaged(preview.credit.credit_id),
        )?;
        let replay_insert_witness = preview
            .prepared_replay
            .witness(0)
            .ok_or(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness)?;
        let sender_output = peer_payment_public_output(&staged.request, &staged.payment)?;
        let expected_credit = ReceiveFoldCreditPreviewV1 {
            amount: staged.payment.statement.amount,
            credit_id: preview.credit.credit_id,
            recipient_lane_id: self.state.lane.device_lane_id,
            incoming_proof_binding_digest: kagemusha_incoming_proof_binding_digest_v1(
                &sender_output,
                &staged.payment.proof,
            )
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?,
            envelope_digest: staged.envelope_digest,
            replay_insert_witness: replay_insert_witness.clone(),
        };
        if expected_credit != preview.credit {
            return Err(KagemushaStateErrorV1::InvalidPeerCredit);
        }

        let balance = self
            .state
            .balance
            .checked_add(expected_credit.amount)
            .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)?;
        let successor = self.next_state(
            balance,
            self.state.hardware_epoch,
            self.state.device_policy_binding,
            preview.transition.successor.state_nonce_commitment,
            preview.prepared_replay.final_root(),
        )?;
        let effect_digest = receive_fold_effect_digest(&expected_credit)?;
        let successor_commitment = successor.state_commitment;
        let expected_transition = self.transition_preview(
            KagemushaTransitionKindV1::ReceiveFold,
            successor,
            effect_digest,
            [0; 32],
            [0; 32],
            expected_credit.credit_id.0,
            expected_credit.recipient_lane_id,
            TransitionAuxiliaryBindingsV1::default(),
            preview.trusted_commit_time_ms,
            |normalized_guard_statement_digest| {
                local_transition_transport_digest(
                    KagemushaTransitionKindV1::ReceiveFold,
                    self.state.release_id,
                    self.state.liability_pool_id,
                    effect_digest,
                    self.state.state_commitment,
                    successor_commitment,
                    normalized_guard_statement_digest,
                )
            },
        )?;
        if expected_transition != preview.transition {
            return Err(KagemushaStateErrorV1::StateInvariant);
        }
        self.verify_transition_authorization(&preview.transition, &authorization)?;

        let payment_digest = staged
            .payment
            .canonical_digest_against(&staged.request)
            .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?;
        let folded_ticket = (
            staged.payment.acceptance_ticket.acceptance_ticket_id,
            payment_digest,
        );
        let mut next_consumed_credits = self.consumed_credits.clone();
        next_consumed_credits.install_prepared_batch(preview.prepared_replay)?;
        let mut next_pending_credits = self.pending_credits.clone();
        if next_pending_credits
            .remove(&expected_credit.credit_id)
            .is_none()
        {
            return Err(KagemushaStateErrorV1::StateInvariant);
        }
        let snapshot_usage = receiver_snapshot_capacity_usage_v1(
            &next_pending_credits,
            &self.accepted_payment_receipts,
            &next_consumed_credits,
        )?;
        let next_ticket_book = self
            .acceptance_ticket_book
            .receiver_snapshot_folded_successor(
                snapshot_usage.live_bytes,
                snapshot_usage.retained_bytes,
                &[folded_ticket],
            )?;
        self.consumed_credits = next_consumed_credits;
        self.pending_credits = next_pending_credits;
        self.acceptance_ticket_book = next_ticket_book;
        self.commit_preview(preview.transition);
        Ok(self.state.clone())
    }

    /// Preview, authorize, and atomically fold one staged credit.
    pub fn receive_fold(
        &mut self,
        credit_id: CreditIdV1,
        successor_state_nonce_commitment: DigestV1,
        trusted_commit_time_ms: u64,
        authorization: TransitionAuthorizationV1,
    ) -> Result<KagemushaStateV1, KagemushaStateErrorV1> {
        let preview = self.preview_receive_fold(
            credit_id,
            successor_state_nonce_commitment,
            trusted_commit_time_ms,
        )?;
        self.receive_fold_prepared(preview, authorization)
    }
}

fn receive_fold_effect_digest(
    credit: &ReceiveFoldCreditPreviewV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    canonical_sha256_digest(
        super::TRANSITION_EFFECT_DOMAIN,
        &ReceiveFoldEffectV1 {
            amount: credit.amount,
            credit_id: credit.credit_id,
            recipient_lane_id: credit.recipient_lane_id,
            incoming_proof_binding_digest: credit.incoming_proof_binding_digest,
            envelope_digest: credit.envelope_digest,
        },
    )
}
