//! State-machine preparation and atomic installation for one KAGEMUSHA `ReceiveFold`.

use iroha_data_model::kagemusha::{
    KagemushaCreditOpeningV1, KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1,
    KagemushaPastaStateCommitmentV1, kagemusha_device_key_reference_v1,
    kagemusha_prepared_transfer_digest_v1,
};
use norito::codec::Encode;

use crate::zk::kagemusha_v1_recursion::{
    KagemushaReceiveFoldCreditV1, KagemushaReplayInsertWitnessV1,
};

use super::{
    ConsumedCreditInsertWitnessV1, CreditIdV1, DigestV1, KagemushaGuardBundleVerifierV1,
    KagemushaHistoryAbortOutcomeV1, KagemushaHistoryCommitOutcomeV1,
    KagemushaHistoryDualInsertPreparationV1, KagemushaHistoryPrepareOutcomeV1,
    KagemushaHistoryProofRootBridgeRequestV1, KagemushaHistoryRecoveryOutcomeV1,
    KagemushaHistoryRootSelectionCertificateV1, KagemushaHistoryRootSelectionSubjectV1,
    KagemushaHistoryTransitionAuthorizationV1, KagemushaHistoryTreeV1,
    KagemushaPreparedHistoryCasV1, KagemushaRecursiveVerifierV1, KagemushaStateErrorV1,
    KagemushaStateMachineV1, KagemushaStateV1, KagemushaTransitionKindV1,
    TransitionAuthorizationV1, TransitionAuxiliaryBindingsV1, TransitionPreviewV1,
    canonical_sha256_digest, kagemusha_incoming_proof_binding_digest_v1,
    local_transition_transport_digest, map_authenticated_history_error,
    receive_fold::{ReceiveFoldCreditV1, ReceiveFoldErrorV1, ReceiveFoldV1},
    receiver_sequence_entry_bytes, require_history_proof_root_bridge_v1,
    sparse_merkle::{ExactConsumedCreditIndex, PreparedConsumedCreditInsertV1},
};

/// Complete private input for one staged peer credit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerCreditFoldInputV1 {
    /// Positive credit amount in atomic units.
    pub amount: u128,
    /// Exact receiver-bound credit identity.
    pub credit_id: CreditIdV1,
    /// Local recipient lane opened from the signed request credential.
    pub recipient_lane_id: DigestV1,
    /// Binding of the exact incoming paired sender proof and history.
    pub incoming_proof_binding_digest: DigestV1,
    /// Exact signed request digest authenticated by the incoming proof.
    pub request_digest: DigestV1,
    /// Request-bound prepared-transfer digest authenticated by the sender proof.
    pub prepared_transfer_digest: DigestV1,
    /// Unique transition nullifier opened by the payment output.
    pub transition_nullifier: DigestV1,
    /// Recipient encryption key authenticated by the signed request.
    pub recipient_encryption_key: DigestV1,
    /// Pre-ID opening commitment carried by the payment output.
    pub ciphertext_commitment: DigestV1,
    /// Exact receiver-only plaintext recovered after authenticated decryption.
    pub credit_opening: KagemushaCreditOpeningV1,
    /// Digest binding the receiver hardware credential.
    pub receiver_binding_digest: DigestV1,
    /// Digest of the sender's compact terminal public payment output.
    pub payment_output_digest: DigestV1,
    /// Digest of the canonical payment envelope staged in the durable inbox.
    pub envelope_digest: DigestV1,
    /// Exact empty-to-present replay insertion consumed by the recursive relation.
    pub replay_insert_witness: ConsumedCreditInsertWitnessV1,
}

impl PeerCreditFoldInputV1 {
    fn transcript_credit(&self) -> ReceiveFoldCreditV1 {
        ReceiveFoldCreditV1 {
            amount: self.amount,
            credit_id: self.credit_id,
            recipient_lane_id: self.recipient_lane_id,
            incoming_proof_binding_digest: self.incoming_proof_binding_digest,
            receiver_binding_digest: self.receiver_binding_digest,
            payment_output_digest: self.payment_output_digest,
            envelope_digest: self.envelope_digest,
        }
    }

    /// Convert the checked host input into the private recursive-relation credit.
    pub(crate) fn relation_credit(&self) -> KagemushaReceiveFoldCreditV1 {
        KagemushaReceiveFoldCreditV1 {
            amount: self.amount,
            credit_id: self.credit_id.0,
            recipient_lane_id: self.recipient_lane_id,
            incoming_proof_binding_digest: self.incoming_proof_binding_digest,
            request_digest: self.request_digest,
            prepared_transfer_digest: self.prepared_transfer_digest,
            transition_nullifier: self.transition_nullifier,
            recipient_encryption_key: self.recipient_encryption_key,
            ciphertext_commitment: self.ciphertext_commitment,
            credit_opening: self.credit_opening,
            receiver_binding_digest: self.receiver_binding_digest,
            payment_output_digest: self.payment_output_digest,
            replay_insert: KagemushaReplayInsertWitnessV1::from(&self.replay_insert_witness),
        }
    }
}

/// One singular receive transition plus its recoverable replay-tree plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerCreditFoldPreviewV1 {
    /// Deterministic transition statements and expected aggregate successor.
    pub transition: TransitionPreviewV1,
    /// Exact staged credit and replay witness.
    pub credit: PeerCreditFoldInputV1,
    /// SHA-256 binding of the exact received credit.
    pub receive_credit_binding_digest: DigestV1,
    /// Prepared dual-root external-history transaction retained in the byte-bounded WAL.
    pub(crate) authenticated_history_transaction: KagemushaPreparedHistoryCasV1,
    /// Exact SHA-256/Pasta root association required from proof and hardware.
    pub(crate) proof_root_bridge_request: KagemushaHistoryProofRootBridgeRequestV1,
    trusted_commit_time_ms: u64,
    prepared_replay: PreparedConsumedCreditV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PreparedConsumedCreditV1 {
    starting_root: KagemushaPastaStateCommitmentV1,
    final_root: KagemushaPastaStateCommitmentV1,
    insert: PreparedConsumedCreditInsertV1,
}

impl PreparedConsumedCreditV1 {
    fn prepare(
        index: &ExactConsumedCreditIndex,
        credit_id: CreditIdV1,
        envelope_digest: DigestV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let starting_root = index.root();
        let insert = index.prepare_insert(credit_id, envelope_digest)?;
        let final_root = insert.witness().successor_root;
        if final_root.is_zero() || final_root == starting_root {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }
        Ok(Self {
            starting_root,
            final_root,
            insert,
        })
    }

    fn witness(&self) -> &ConsumedCreditInsertWitnessV1 {
        self.insert.witness()
    }

    fn install_into(
        &self,
        index: &mut ExactConsumedCreditIndex,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.starting_root != index.root()
            || self.final_root.is_zero()
            || self.final_root == self.starting_root
        {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }
        index.install_prepared_insert(self.insert.clone())?;
        if index.root() != self.final_root {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode)]
struct ReceiveFoldEffectV1 {
    credit_id: CreditIdV1,
    amount: u128,
    receive_credit_binding_digest: DigestV1,
}

const RECEIVE_FOLD_DECISION_ID_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:receive-fold:terminal-decision-id\0";
const RECEIVE_FOLD_DECISION_VALUE_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:receive-fold:terminal-decision-value\0";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode)]
struct ReceiveFoldDecisionIdV1 {
    predecessor_state_commitment: DigestV1,
    credit_id: CreditIdV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode)]
struct ReceiveFoldDecisionValueV1 {
    successor_state_commitment: DigestV1,
    credit_id: CreditIdV1,
    receive_credit_binding_digest: DigestV1,
    effect_digest: DigestV1,
}

impl<R, G, H> KagemushaStateMachineV1<R, G, H>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
    H: super::KagemushaAuthenticatedHistoryStoreV1,
{
    /// Preview folding exactly one staged credit into the aggregate successor.
    pub fn preview_receive_fold(
        &mut self,
        credit_id: CreditIdV1,
        successor_state_nonce_commitment: DigestV1,
        trusted_commit_time_ms: u64,
    ) -> Result<PeerCreditFoldPreviewV1, KagemushaStateErrorV1> {
        let staged = self
            .pending_credits
            .get(&credit_id)
            .ok_or(KagemushaStateErrorV1::CreditNotStaged(credit_id))?;
        let balance = self
            .state
            .balance
            .checked_add(staged.request.amount)
            .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)?;
        let prepared_replay = PreparedConsumedCreditV1::prepare(
            &self.consumed_credits,
            credit_id,
            staged.envelope_digest,
        )?;
        let credit = self.receive_fold_credit(credit_id, &prepared_replay)?;
        let transcript = ReceiveFoldV1::try_new(credit.transcript_credit())
            .map_err(receive_fold_error)?;
        let receive_credit_binding_digest = transcript.canonical_transcript_digest();
        if receive_credit_binding_digest == [0; 32] {
            return Err(KagemushaStateErrorV1::InvalidPeerCredit);
        }
        let successor = self.next_state(
            balance,
            self.state.hardware_epoch,
            self.state.device_policy_binding,
            successor_state_nonce_commitment,
            prepared_replay.final_root,
        )?;
        let effect_digest = receive_fold_effect_digest(&credit, receive_credit_binding_digest)?;
        let successor_commitment = successor.state_commitment;
        let transition = self.transition_preview(
            KagemushaTransitionKindV1::ReceiveFold,
            successor,
            effect_digest,
            [0; 32],
            [0; 32],
            [0; 32],
            [0; 32],
            TransitionAuxiliaryBindingsV1 {
                receive_credit_binding_digest,
                ..TransitionAuxiliaryBindingsV1::default()
            },
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

        let decision_id = canonical_sha256_digest(
            RECEIVE_FOLD_DECISION_ID_DOMAIN_V1,
            &ReceiveFoldDecisionIdV1 {
                predecessor_state_commitment: self.state.state_commitment,
                credit_id,
            },
        )?;
        let decision_digest = canonical_sha256_digest(
            RECEIVE_FOLD_DECISION_VALUE_DOMAIN_V1,
            &ReceiveFoldDecisionValueV1 {
                successor_state_commitment: transition.successor.state_commitment,
                credit_id,
                receive_credit_binding_digest,
                effect_digest,
            },
        )?;
        let insert = [(credit_id, staged.envelope_digest)];
        let authenticated_history_transaction = match self
            .authenticated_history
            .prepare_replay_batch_and_terminal_decision(&insert, decision_id, decision_digest)
            .map_err(map_authenticated_history_error)?
        {
            KagemushaHistoryDualInsertPreparationV1::Prepared {
                transaction,
                outcome:
                    KagemushaHistoryPrepareOutcomeV1::Prepared
                    | KagemushaHistoryPrepareOutcomeV1::AlreadyPrepared,
            } => transaction,
            KagemushaHistoryDualInsertPreparationV1::Prepared {
                outcome: KagemushaHistoryPrepareOutcomeV1::AlreadyCommitted { .. },
                ..
            }
            | KagemushaHistoryDualInsertPreparationV1::ExactDuplicate => {
                return Err(KagemushaStateErrorV1::CreditAlreadyConsumed(credit_id));
            }
            KagemushaHistoryDualInsertPreparationV1::Prepared {
                outcome: KagemushaHistoryPrepareOutcomeV1::AlreadyAborted,
                ..
            } => return Err(KagemushaStateErrorV1::StateInvariant),
            KagemushaHistoryDualInsertPreparationV1::Conflict { tree, key, .. } => {
                return Err(match tree {
                    KagemushaHistoryTreeV1::Replay => {
                        KagemushaStateErrorV1::CreditConflict(CreditIdV1(key))
                    }
                    KagemushaHistoryTreeV1::TerminalDecision => {
                        KagemushaStateErrorV1::StateInvariant
                    }
                });
            }
        };
        let proof_root_bridge_request = self
            .authenticated_history
            .proof_root_bridge_request(
                &authenticated_history_transaction,
                effect_digest,
                prepared_replay.starting_root,
                prepared_replay.final_root,
            )
            .map_err(map_authenticated_history_error)?;

        Ok(PeerCreditFoldPreviewV1 {
            transition,
            credit,
            receive_credit_binding_digest,
            authenticated_history_transaction,
            proof_root_bridge_request,
            trusted_commit_time_ms,
            prepared_replay,
        })
    }

    /// Return the hardware signing request selecting this fold's dual history roots.
    pub fn receive_fold_history_root_selection_signing_bytes(
        &self,
        preview: &PeerCreditFoldPreviewV1,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.validate_receive_fold_history_preview(preview)?;
        KagemushaHistoryRootSelectionSubjectV1::new(
            &preview.authenticated_history_transaction,
            self.state.hardware_profile_id,
            self.state.hardware_epoch.generation,
            preview.transition.journal_revision_after,
        )
        .signing_bytes()
        .map_err(map_authenticated_history_error)
    }

    /// Attach the hardware root selection after verifying the same paired state proof.
    pub fn authorize_receive_fold_history(
        &self,
        preview: &PeerCreditFoldPreviewV1,
        mut authorization: TransitionAuthorizationV1,
        device_public_key: &KagemushaDevicePublicKeyV1,
        root_selection_signature: KagemushaDeviceSignatureV1,
    ) -> Result<TransitionAuthorizationV1, KagemushaStateErrorV1> {
        self.validate_receive_fold_history_preview(preview)?;
        if authorization.authenticated_history.is_some()
            || kagemusha_device_key_reference_v1(device_public_key)
                != self.state.device_policy_binding.device_key_reference
        {
            return Err(KagemushaStateErrorV1::InvalidDevicePolicyBinding);
        }
        self.verify_transition_authorization(&preview.transition, &authorization)?;
        let subject = KagemushaHistoryRootSelectionSubjectV1::new(
            &preview.authenticated_history_transaction,
            self.state.hardware_profile_id,
            self.state.hardware_epoch.generation,
            preview.transition.journal_revision_after,
        );
        let root_selection =
            KagemushaHistoryRootSelectionCertificateV1::new(subject, root_selection_signature)
                .verify(self.state.hardware_profile_id, device_public_key)
                .map_err(map_authenticated_history_error)?;
        let proof_root_bridge = require_history_proof_root_bridge_v1(
            preview.proof_root_bridge_request,
            preview.transition.proof_statement.effect_digest,
        )
        .map_err(|_| KagemushaStateErrorV1::AuthenticatedHistoryProofRootBridgeUnavailable)?;
        authorization.authenticated_history = Some(KagemushaHistoryTransitionAuthorizationV1 {
            root_selection,
            proof_root_bridge,
        });
        Ok(authorization)
    }

    /// Verify and atomically install one prepared receive fold.
    pub fn receive_fold_prepared(
        &mut self,
        preview: PeerCreditFoldPreviewV1,
        authorization: TransitionAuthorizationV1,
    ) -> Result<KagemushaStateV1, KagemushaStateErrorV1> {
        self.install_receive_fold(preview, authorization, false)
    }

    /// Resume the same authorized fold after a crash at the external CAS boundary.
    pub fn recover_receive_fold_prepared(
        &mut self,
        preview: PeerCreditFoldPreviewV1,
        authorization: TransitionAuthorizationV1,
    ) -> Result<KagemushaStateV1, KagemushaStateErrorV1> {
        self.install_receive_fold(preview, authorization, true)
    }

    /// Release the WAL entry for an abandoned, uncommitted fold preview.
    pub fn abandon_receive_fold_preview(
        &mut self,
        preview: &PeerCreditFoldPreviewV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        match self
            .authenticated_history
            .abort_prepared(preview.authenticated_history_transaction.transaction_id())
            .map_err(map_authenticated_history_error)?
        {
            KagemushaHistoryAbortOutcomeV1::Aborted
            | KagemushaHistoryAbortOutcomeV1::AlreadyAborted => Ok(()),
            KagemushaHistoryAbortOutcomeV1::AlreadyCommitted { .. } => {
                Err(KagemushaStateErrorV1::StateInvariant)
            }
        }
    }

    fn install_receive_fold(
        &mut self,
        preview: PeerCreditFoldPreviewV1,
        authorization: TransitionAuthorizationV1,
        recovering: bool,
    ) -> Result<KagemushaStateV1, KagemushaStateErrorV1> {
        if preview.prepared_replay.starting_root != self.consumed_credits.root()
            || preview.prepared_replay.final_root
                != preview.transition.successor.consumed_credit_root
        {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }
        let credit_id = preview.credit.credit_id;
        let expected_credit = self.receive_fold_credit(credit_id, &preview.prepared_replay)?;
        let expected_fold = ReceiveFoldV1::try_new(expected_credit.transcript_credit())
            .map_err(receive_fold_error)?;
        if expected_credit != preview.credit
            || expected_fold.canonical_transcript_digest()
                != preview.receive_credit_binding_digest
        {
            return Err(KagemushaStateErrorV1::InvalidPeerCredit);
        }

        let balance = self
            .state
            .balance
            .checked_add(expected_fold.amount())
            .ok_or(KagemushaStateErrorV1::ArithmeticOverflow)?;
        let successor = self.next_state(
            balance,
            self.state.hardware_epoch,
            self.state.device_policy_binding,
            preview.transition.successor.state_nonce_commitment,
            preview.prepared_replay.final_root,
        )?;
        let effect_digest =
            receive_fold_effect_digest(&expected_credit, preview.receive_credit_binding_digest)?;
        let successor_commitment = successor.state_commitment;
        let expected_transition = self.transition_preview(
            KagemushaTransitionKindV1::ReceiveFold,
            successor,
            effect_digest,
            [0; 32],
            [0; 32],
            [0; 32],
            [0; 32],
            TransitionAuxiliaryBindingsV1 {
                receive_credit_binding_digest: preview.receive_credit_binding_digest,
                ..TransitionAuxiliaryBindingsV1::default()
            },
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

        let staged = self
            .pending_credits
            .get(&credit_id)
            .ok_or(KagemushaStateErrorV1::CreditNotStaged(credit_id))?;
        let receipt = self
            .accepted_payment_receipts
            .get(&credit_id)
            .ok_or(KagemushaStateErrorV1::StateInvariant)?;
        let next_capacity = self.receiver_inbox_capacity.receiver_snapshot_folded_successor(
            receiver_sequence_entry_bytes(staged)?,
            receiver_sequence_entry_bytes(receipt)?,
        )?;
        let mut next_consumed_credits = self.consumed_credits.clone();
        preview
            .prepared_replay
            .install_into(&mut next_consumed_credits)?;
        let mut next_pending_credits = self.pending_credits.clone();
        if next_pending_credits.remove(&credit_id).is_none() {
            return Err(KagemushaStateErrorV1::StateInvariant);
        }
        self.verify_transition_authorization(&preview.transition, &authorization)?;

        let history_authorization = authorization
            .authenticated_history
            .ok_or(KagemushaStateErrorV1::AuthenticatedHistoryProofRootBridgeUnavailable)?;
        let bridge_request = preview.proof_root_bridge_request;
        let external_predecessor_roots = bridge_request.external_predecessor_roots();
        let external_successor_roots = bridge_request.external_successor_roots();
        let current_external_roots = self.authenticated_history.committed_roots();
        if history_authorization.root_selection.transaction_id()
            != preview.authenticated_history_transaction.transaction_id()
            || history_authorization.root_selection.root_selection()
                != preview.authenticated_history_transaction.root_selection()
            || history_authorization.root_selection.hardware_profile_id()
                != self.state.hardware_profile_id
            || history_authorization.root_selection.hardware_epoch()
                != self.state.hardware_epoch.generation
            || history_authorization.root_selection.monotonic_counter()
                != preview.transition.journal_revision_after
            || history_authorization.proof_root_bridge.request() != bridge_request
            || bridge_request.transaction_id()
                != preview.authenticated_history_transaction.transaction_id()
            || bridge_request.pasta_predecessor_replay_root()
                != preview.prepared_replay.starting_root
            || bridge_request.pasta_successor_replay_root() != preview.prepared_replay.final_root
            || preview
                .authenticated_history_transaction
                .successor_roots_from(external_predecessor_roots)
                .map_err(map_authenticated_history_error)?
                != external_successor_roots
            || (current_external_roots != external_predecessor_roots
                && (!recovering || current_external_roots != external_successor_roots))
        {
            return Err(KagemushaStateErrorV1::AuthenticatedHistoryProofRootBridgeUnavailable);
        }

        let committed_roots = if recovering {
            match self
                .authenticated_history
                .recover_prepared(history_authorization.root_selection)
                .map_err(map_authenticated_history_error)?
            {
                KagemushaHistoryRecoveryOutcomeV1::Committed { committed_roots }
                | KagemushaHistoryRecoveryOutcomeV1::AlreadyCommitted { committed_roots } => {
                    committed_roots
                }
                KagemushaHistoryRecoveryOutcomeV1::Aborted => {
                    return Err(KagemushaStateErrorV1::StateInvariant);
                }
            }
        } else {
            match self
                .authenticated_history
                .commit_prepared(history_authorization.root_selection)
                .map_err(map_authenticated_history_error)?
            {
                KagemushaHistoryCommitOutcomeV1::Committed { committed_roots }
                | KagemushaHistoryCommitOutcomeV1::AlreadyCommitted { committed_roots } => {
                    committed_roots
                }
                KagemushaHistoryCommitOutcomeV1::Aborted => {
                    return Err(KagemushaStateErrorV1::StateInvariant);
                }
            }
        };
        if committed_roots != external_successor_roots {
            return Err(KagemushaStateErrorV1::AuthenticatedHistoryUnavailable);
        }

        self.consumed_credits = next_consumed_credits;
        self.pending_credits = next_pending_credits;
        self.receiver_inbox_capacity = next_capacity;
        self.commit_preview(preview.transition);
        Ok(self.state.clone())
    }

    fn validate_receive_fold_history_preview(
        &self,
        preview: &PeerCreditFoldPreviewV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let bridge_request = preview.proof_root_bridge_request;
        if preview.transition.proof_statement.kind != KagemushaTransitionKindV1::ReceiveFold
            || preview.transition.proof_statement.predecessor_commitment
                != self.state.state_commitment
            || preview.transition.proof_statement.journal_revision_before != self.journal_revision
            || bridge_request.transaction_id()
                != preview.authenticated_history_transaction.transaction_id()
            || bridge_request.operation_binding_digest()
                != preview.transition.proof_statement.effect_digest
            || bridge_request.external_predecessor_roots()
                != self.authenticated_history.committed_roots()
            || bridge_request.pasta_predecessor_replay_root()
                != preview.prepared_replay.starting_root
            || bridge_request.pasta_successor_replay_root() != preview.prepared_replay.final_root
        {
            return Err(KagemushaStateErrorV1::AuthenticatedHistoryProofRootBridgeUnavailable);
        }
        Ok(())
    }

    fn receive_fold_credit(
        &self,
        credit_id: CreditIdV1,
        prepared_replay: &PreparedConsumedCreditV1,
    ) -> Result<PeerCreditFoldInputV1, KagemushaStateErrorV1> {
        let staged = self
            .pending_credits
            .get(&credit_id)
            .ok_or(KagemushaStateErrorV1::CreditNotStaged(credit_id))?;
        let replay_insert_witness = prepared_replay.witness();
        if replay_insert_witness.credit_id != credit_id
            || replay_insert_witness.envelope_digest != staged.envelope_digest
        {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }
        let incoming_proof_binding_digest =
            kagemusha_incoming_proof_binding_digest_v1(&staged.request, &staged.payment)
                .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        let request_digest = staged
            .request
            .canonical_digest()
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        let receiver_binding_digest = staged.request.hardware_credential.credential_id;
        let payment_output_digest = staged
            .payment
            .output
            .canonical_digest_against(&staged.request)
            .map_err(|error| KagemushaStateErrorV1::ProofRejected(error.to_string()))?;
        Ok(PeerCreditFoldInputV1 {
            amount: staged.request.amount,
            credit_id,
            recipient_lane_id: self.state.lane.device_lane_id,
            incoming_proof_binding_digest,
            request_digest,
            prepared_transfer_digest: kagemusha_prepared_transfer_digest_v1(
                &staged.request,
                staged.payment.output.sender_before_commitment,
                staged.payment.output.sender_after_commitment,
                staged.payment.output.transition_nullifier,
                staged.payment.output.ciphertext_commitment,
            )
            .map_err(|_| KagemushaStateErrorV1::InvalidPeerCredit)?,
            transition_nullifier: staged.payment.output.transition_nullifier,
            recipient_encryption_key: staged.request.recipient_encryption_key,
            ciphertext_commitment: staged.payment.output.ciphertext_commitment,
            credit_opening: staged.credit_opening,
            receiver_binding_digest,
            payment_output_digest,
            envelope_digest: staged.envelope_digest,
            replay_insert_witness: replay_insert_witness.clone(),
        })
    }
}

fn receive_fold_effect_digest(
    credit: &PeerCreditFoldInputV1,
    receive_credit_binding_digest: DigestV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    canonical_sha256_digest(
        super::TRANSITION_EFFECT_DOMAIN,
        &ReceiveFoldEffectV1 {
            credit_id: credit.credit_id,
            amount: credit.amount,
            receive_credit_binding_digest,
        },
    )
}

fn receive_fold_error(_: ReceiveFoldErrorV1) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::InvalidPeerCredit
}
