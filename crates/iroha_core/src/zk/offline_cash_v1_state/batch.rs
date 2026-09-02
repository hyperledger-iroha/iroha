//! Fixed-shape host preparation for aggregate receive folds.

use std::collections::BTreeSet;

use iroha_data_model::offline::OFFLINE_CASH_RECEIVE_FOLD_BATCH_WIDTH_V1;
use norito::codec::Encode;
use sha2::{Digest as _, Sha256};

use super::{
    ConsumedCreditInsertWitnessV1, CreditIdV1, DigestV1, OfflineCashGuardBundleVerifierV1,
    OfflineCashRecursiveVerifierV1, OfflineCashStateErrorV1, OfflineCashStateMachineV1,
    OfflineCashStateV1, OfflineCashTransitionKindV1, TransitionAuthorizationV1,
    TransitionAuxiliaryBindingsV1, TransitionPreviewV1, canonical_sha256_digest,
    local_transition_transport_digest, offline_cash_incoming_proof_binding_digest_v1,
    peer_payment_public_output, receiver_snapshot_capacity_usage_v1,
    sparse_merkle::PreparedConsumedCreditBatchV1,
};

const RECEIVE_BATCH_BINDING_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:receive-fold-batch\0";

/// One active slot in a fixed-width receive-fold preview.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreditFoldBatchSlotPreviewV1 {
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

/// A fixed sixteen-slot receive transition plus its opaque atomic replay-tree plan.
///
/// Active slots always form a non-empty prefix. Every remaining slot is canonical `None`
/// padding, so proof shape and public verification work do not depend on backlog size or prior
/// payment history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreditFoldBatchPreviewV1 {
    /// Deterministic public transition statements and expected aggregate successor.
    pub transition: TransitionPreviewV1,
    /// Number of active prefix slots, from one through sixteen.
    pub active_count: u8,
    /// Fixed-width active-prefix receive slots with canonical inactive padding.
    pub slots: Box<
        [Option<CreditFoldBatchSlotPreviewV1>; OFFLINE_CASH_RECEIVE_FOLD_BATCH_WIDTH_V1 as usize],
    >,
    /// Binding of the active prefix and every canonical inactive slot.
    pub receive_batch_binding_digest: DigestV1,
    trusted_commit_time_ms: u64,
    prepared_replay: PreparedConsumedCreditBatchV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode)]
struct ReceiveFoldBatchEffectV1 {
    active_count: u8,
    receive_batch_binding_digest: DigestV1,
}

impl<R, G> OfflineCashStateMachineV1<R, G>
where
    R: OfflineCashRecursiveVerifierV1,
    G: OfflineCashGuardBundleVerifierV1,
{
    /// Preview folding one through sixteen staged credits into one aggregate successor.
    ///
    /// The fixed width is one recursive-operation shape, not a history or backlog limit. A
    /// caller folds any larger backlog by invoking this operation repeatedly.
    pub fn preview_receive_fold_batch(
        &self,
        credit_ids: &[CreditIdV1],
        successor_state_nonce_commitment: DigestV1,
        trusted_commit_time_ms: u64,
    ) -> Result<CreditFoldBatchPreviewV1, OfflineCashStateErrorV1> {
        let width = usize::from(OFFLINE_CASH_RECEIVE_FOLD_BATCH_WIDTH_V1);
        if credit_ids.is_empty() || credit_ids.len() > width {
            return Err(OfflineCashStateErrorV1::InvalidPeerCredit);
        }

        let mut seen = BTreeSet::new();
        let mut inserts = Vec::with_capacity(credit_ids.len());
        let mut balance = self.state.balance;
        for &credit_id in credit_ids {
            if !seen.insert(credit_id) {
                return Err(OfflineCashStateErrorV1::CreditAlreadyConsumed(credit_id));
            }
            let staged = self
                .pending_credits
                .get(&credit_id)
                .ok_or(OfflineCashStateErrorV1::CreditNotStaged(credit_id))?;
            balance = balance
                .checked_add(staged.payment.statement.amount)
                .ok_or(OfflineCashStateErrorV1::ArithmeticOverflow)?;
            inserts.push((credit_id, staged.envelope_digest));
        }

        let prepared_replay = self.consumed_credits.prepare_batch_inserts(&inserts)?;
        let slots = self.receive_batch_slots(credit_ids, &prepared_replay)?;
        let active_count = u8::try_from(credit_ids.len())
            .map_err(|_| OfflineCashStateErrorV1::InvalidPeerCredit)?;
        let receive_batch_binding_digest =
            receive_batch_binding_digest(active_count, slots.as_ref());
        if receive_batch_binding_digest == [0; 32] {
            return Err(OfflineCashStateErrorV1::InvalidPeerCredit);
        }
        let successor = self.next_state(
            balance,
            self.state.hardware_epoch,
            self.state.device_policy_binding,
            successor_state_nonce_commitment,
            prepared_replay.final_root(),
        )?;
        let effect_digest =
            receive_batch_effect_digest(active_count, receive_batch_binding_digest)?;
        let successor_commitment = successor.state_commitment;
        let transition = self.transition_preview(
            OfflineCashTransitionKindV1::ReceiveFoldBatch,
            successor,
            effect_digest,
            [0; 32],
            [0; 32],
            [0; 32],
            [0; 32],
            TransitionAuxiliaryBindingsV1 {
                receive_active_count: active_count,
                receive_batch_binding_digest,
                ..TransitionAuxiliaryBindingsV1::default()
            },
            trusted_commit_time_ms,
            |normalized_guard_statement_digest| {
                local_transition_transport_digest(
                    OfflineCashTransitionKindV1::ReceiveFoldBatch,
                    self.state.release_id,
                    self.state.liability_pool_id,
                    effect_digest,
                    self.state.state_commitment,
                    successor_commitment,
                    normalized_guard_statement_digest,
                )
            },
        )?;

        Ok(CreditFoldBatchPreviewV1 {
            transition,
            active_count,
            slots,
            receive_batch_binding_digest,
            trusted_commit_time_ms,
            prepared_replay,
        })
    }

    /// Verify and atomically install a previously prepared receive-fold batch.
    ///
    /// All staged-credit semantics and the complete transition are rederived against the current
    /// state before authorization. The replay dictionary and inbox are not mutated unless the
    /// authorization succeeds and the opaque plan still names the exact current sparse-tree path.
    pub fn receive_fold_batch_prepared(
        &mut self,
        preview: CreditFoldBatchPreviewV1,
        authorization: TransitionAuthorizationV1,
    ) -> Result<OfflineCashStateV1, OfflineCashStateErrorV1> {
        let active_count = usize::from(preview.active_count);
        if active_count == 0
            || active_count > usize::from(OFFLINE_CASH_RECEIVE_FOLD_BATCH_WIDTH_V1)
            || preview.prepared_replay.len() != active_count
            || preview.prepared_replay.starting_root() != self.consumed_credits.root()
            || preview.prepared_replay.final_root()
                != preview.transition.successor.consumed_credit_root
        {
            return Err(OfflineCashStateErrorV1::InvalidConsumedCreditInsertWitness);
        }

        let mut credit_ids = Vec::with_capacity(active_count);
        for (index, slot) in preview.slots.iter().enumerate() {
            if (index < active_count) != slot.is_some() {
                return Err(OfflineCashStateErrorV1::InvalidPeerCredit);
            }
            if let Some(slot) = slot {
                credit_ids.push(slot.credit_id);
            }
        }
        let expected_slots = self.receive_batch_slots(&credit_ids, &preview.prepared_replay)?;
        if expected_slots != preview.slots
            || receive_batch_binding_digest(preview.active_count, expected_slots.as_ref())
                != preview.receive_batch_binding_digest
        {
            return Err(OfflineCashStateErrorV1::InvalidPeerCredit);
        }

        let mut balance = self.state.balance;
        for slot in expected_slots.iter().flatten() {
            balance = balance
                .checked_add(slot.amount)
                .ok_or(OfflineCashStateErrorV1::ArithmeticOverflow)?;
        }
        let successor = self.next_state(
            balance,
            self.state.hardware_epoch,
            self.state.device_policy_binding,
            preview.transition.successor.state_nonce_commitment,
            preview.prepared_replay.final_root(),
        )?;
        let effect_digest = receive_batch_effect_digest(
            preview.active_count,
            preview.receive_batch_binding_digest,
        )?;
        let successor_commitment = successor.state_commitment;
        let expected_transition = self.transition_preview(
            OfflineCashTransitionKindV1::ReceiveFoldBatch,
            successor,
            effect_digest,
            [0; 32],
            [0; 32],
            [0; 32],
            [0; 32],
            TransitionAuxiliaryBindingsV1 {
                receive_active_count: preview.active_count,
                receive_batch_binding_digest: preview.receive_batch_binding_digest,
                ..TransitionAuxiliaryBindingsV1::default()
            },
            preview.trusted_commit_time_ms,
            |normalized_guard_statement_digest| {
                local_transition_transport_digest(
                    OfflineCashTransitionKindV1::ReceiveFoldBatch,
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
            return Err(OfflineCashStateErrorV1::StateInvariant);
        }
        self.verify_transition_authorization(&preview.transition, &authorization)?;

        let maximum_committed_bytes = self.acceptance_ticket_book.committed_inbox_bytes();
        let mut folded_tickets = Vec::with_capacity(credit_ids.len());
        for credit_id in &credit_ids {
            let staged = self
                .pending_credits
                .get(credit_id)
                .ok_or(OfflineCashStateErrorV1::CreditNotStaged(*credit_id))?;
            let payment_digest = staged
                .payment
                .canonical_digest_against(&staged.request)
                .map_err(|_| OfflineCashStateErrorV1::InvalidPeerCredit)?;
            folded_tickets.push((
                staged.payment.acceptance_ticket.acceptance_ticket_id,
                payment_digest,
            ));
        }
        let mut next_consumed_credits = self.consumed_credits.clone();
        next_consumed_credits.install_prepared_batch(preview.prepared_replay)?;
        let mut next_pending_credits = self.pending_credits.clone();
        for credit_id in &credit_ids {
            if next_pending_credits.remove(credit_id).is_none() {
                return Err(OfflineCashStateErrorV1::StateInvariant);
            }
        }
        let snapshot_usage = receiver_snapshot_capacity_usage_v1(
            &next_pending_credits,
            &self.accepted_payment_receipts,
            &next_consumed_credits,
        )?;
        let mut next_ticket_book = self.acceptance_ticket_book.clone();
        next_ticket_book.prepare_receiver_snapshot_fold(snapshot_usage.live_bytes)?;
        for (ticket_id, payment_digest) in folded_tickets {
            next_ticket_book.release_folded(ticket_id, payment_digest)?;
        }
        next_ticket_book.reconcile_receiver_snapshot_usage(
            snapshot_usage.live_bytes,
            snapshot_usage.retained_bytes,
            maximum_committed_bytes,
        )?;
        self.consumed_credits = next_consumed_credits;
        self.pending_credits = next_pending_credits;
        self.acceptance_ticket_book = next_ticket_book;
        self.commit_preview(preview.transition);
        Ok(self.state.clone())
    }

    /// Preview, authorize, and atomically fold one fixed-width active prefix.
    pub fn receive_fold_batch(
        &mut self,
        credit_ids: &[CreditIdV1],
        successor_state_nonce_commitment: DigestV1,
        trusted_commit_time_ms: u64,
        authorization: TransitionAuthorizationV1,
    ) -> Result<OfflineCashStateV1, OfflineCashStateErrorV1> {
        let preview = self.preview_receive_fold_batch(
            credit_ids,
            successor_state_nonce_commitment,
            trusted_commit_time_ms,
        )?;
        self.receive_fold_batch_prepared(preview, authorization)
    }

    fn receive_batch_slots(
        &self,
        credit_ids: &[CreditIdV1],
        prepared_replay: &PreparedConsumedCreditBatchV1,
    ) -> Result<
        Box<
            [Option<CreditFoldBatchSlotPreviewV1>;
                OFFLINE_CASH_RECEIVE_FOLD_BATCH_WIDTH_V1 as usize],
        >,
        OfflineCashStateErrorV1,
    > {
        if credit_ids.is_empty()
            || credit_ids.len() > usize::from(OFFLINE_CASH_RECEIVE_FOLD_BATCH_WIDTH_V1)
            || prepared_replay.len() != credit_ids.len()
        {
            return Err(OfflineCashStateErrorV1::InvalidPeerCredit);
        }
        let mut seen = BTreeSet::new();
        // Each active slot owns a 256-node replay path. Build the fixed-width collection in a
        // heap allocation so normal 16-credit folds do not reserve several copies of the entire
        // batch (hundreds of KiB each) in the caller and callee stack frames.
        let mut slots = (0..usize::from(OFFLINE_CASH_RECEIVE_FOLD_BATCH_WIDTH_V1))
            .map(|_| None)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        for (index, &credit_id) in credit_ids.iter().enumerate() {
            if !seen.insert(credit_id) {
                return Err(OfflineCashStateErrorV1::CreditAlreadyConsumed(credit_id));
            }
            let staged = self
                .pending_credits
                .get(&credit_id)
                .ok_or(OfflineCashStateErrorV1::CreditNotStaged(credit_id))?;
            let replay_insert_witness = prepared_replay
                .witness(index)
                .ok_or(OfflineCashStateErrorV1::InvalidConsumedCreditInsertWitness)?;
            if replay_insert_witness.credit_id != credit_id
                || replay_insert_witness.envelope_digest != staged.envelope_digest
            {
                return Err(OfflineCashStateErrorV1::InvalidConsumedCreditInsertWitness);
            }
            let sender_output = peer_payment_public_output(&staged.request, &staged.payment)?;
            let incoming_proof_binding_digest = offline_cash_incoming_proof_binding_digest_v1(
                &sender_output,
                &staged.payment.proof,
            )
            .map_err(|error| OfflineCashStateErrorV1::ProofRejected(error.to_string()))?;
            slots[index] = Some(CreditFoldBatchSlotPreviewV1 {
                amount: staged.payment.statement.amount,
                credit_id,
                recipient_lane_id: self.state.lane.device_lane_id,
                incoming_proof_binding_digest,
                envelope_digest: staged.envelope_digest,
                replay_insert_witness: replay_insert_witness.clone(),
            });
        }
        slots
            .try_into()
            .map_err(|_| OfflineCashStateErrorV1::StateInvariant)
    }
}

fn receive_batch_effect_digest(
    active_count: u8,
    receive_batch_binding_digest: DigestV1,
) -> Result<DigestV1, OfflineCashStateErrorV1> {
    canonical_sha256_digest(
        super::TRANSITION_EFFECT_DOMAIN,
        &ReceiveFoldBatchEffectV1 {
            active_count,
            receive_batch_binding_digest,
        },
    )
}

fn receive_batch_binding_digest(
    active_count: u8,
    slots: &[Option<CreditFoldBatchSlotPreviewV1>],
) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(RECEIVE_BATCH_BINDING_DOMAIN_V1);
    hasher.update([active_count]);
    for slot in slots {
        if let Some(slot) = slot {
            hasher.update(slot.amount.to_le_bytes());
            hasher.update(slot.credit_id.0);
            hasher.update(slot.recipient_lane_id);
            hasher.update(slot.incoming_proof_binding_digest);
            hasher.update(slot.envelope_digest);
        } else {
            hasher.update([0; 16]);
            for _ in 0..4 {
                hasher.update([0; 32]);
            }
        }
    }
    hasher.finalize().into()
}
