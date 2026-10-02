//! Private durable outgoing operands selected from the actual Main and genuine Math Commit.
//! Serialized bytes never reconstruct proof/Native authority; semantic replay re-admits them.
use super::*;
use crate::kagemusha_v1_recursion::{
    GeneratedOrdinaryCashCommitOriginalsV1, KagemushaOrdinaryCashOutgoingOriginalV1,
    KagemushaOrdinaryLineageCommitProofBundleV1,
};
use iroha_data_model::kagemusha::KagemushaOrdinaryLineageCommitV1;

/// An owned mathematical admission survives only the authenticated Main chronology that
/// constructed it. Its decoded persistence counterpart cannot construct this in-memory owner.
pub(super) struct PreparedCommitAdmission {
    pub(super) originals: PreparedCommitOriginals,
    pub(super) generated: GeneratedOrdinaryCashCommitOriginalsV1,
}

/// Only genuine Main StateAdvance and its authentic global Commit can construct this owner.
pub(super) struct RetainedStateAdvance {
    pub(super) generated: GeneratedOrdinaryCashCommitOriginalsV1,
    pub(super) commit_request_original_sha256: DigestV1,
}

/// Complete acknowledged outgoing transport retained in the physically reserved Native slot.
/// The acknowledgment is separate from the original StateAdvance and never a current FI loan.
pub(super) struct RetainedDelivery {
    commit: KagemushaOrdinaryLineageCommitV1,
    delivery: FinalizedDeliveryOriginals,
    acknowledgment: Option<StateAdvanceAcknowledgment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::OrdinaryCashStateAdvanceAcknowledgmentV1"
)]
pub(super) struct StateAdvanceAcknowledgment {
    financial_control: CapturedFinancialControlIdentity,
    clock: KagemushaOrdinaryCashClockContextV1,
}

/// Exact originals retained before reserving the global Commit. On recovery every proof is
/// independently re-admitted against the genuine retained W1/W2, clocks and CAS reservation.
#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryCashPreparedCommitV1")]
pub(super) struct PreparedCommitOriginals {
    reserve_request_original_sha256: DigestV1,
    commit: KagemushaOrdinaryLineageCommitV1,
    successor: KagemushaStateV1,
    public_state_original: Vec<u8>,
    private_state_checkpoint_original: Vec<u8>,
    private_service_original: Vec<u8>,
    pre_receipt_outgoing_original: Vec<u8>,
}
impl core::fmt::Debug for PreparedCommitOriginals {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PreparedCommitOriginals")
            .finish_non_exhaustive()
    }
}
impl Drop for PreparedCommitOriginals {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        self.private_state_checkpoint_original.zeroize();
        self.successor.balance.zeroize();
        self.successor.state_nonce_commitment.zeroize();
    }
}
impl PreparedCommitOriginals {
    /// Digest the bounded canonical private operands while zeroizing their temporary encoding.
    fn original_sha256(
        &self,
        maximum_payload_bytes: u64,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        if u64::try_from(norito::canonical_frame_len(self).map_err(material)?).map_err(material)?
            > maximum_payload_bytes
        {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        let original = zeroize::Zeroizing::new(norito::encode_canonical(self).map_err(material)?);
        if original.is_empty() || original.len() as u64 > maximum_payload_bytes {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(Sha256::digest(original.as_slice()).into())
    }

    pub(super) fn from_actual_generated(
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        generated: &GeneratedOrdinaryCashCommitOriginalsV1,
        reserve_request_original_sha256: DigestV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        owner.require_current_financial_control()?;
        let original = Self {
            reserve_request_original_sha256,
            commit: generated.commit().clone(),
            successor: generated.selected_successor_state().clone(),
            public_state_original: generated.successor_public_state_original().to_vec(),
            private_state_checkpoint_original: generated
                .successor_private_checkpoint_original()
                .to_vec(),
            private_service_original: generated.private_service_original().to_vec(),
            pre_receipt_outgoing_original: generated.pre_receipt_outgoing_original().to_vec(),
        };
        original.require_actual_generated(owner, generated)?;
        owner.require_current_financial_control()?;
        Ok(original)
    }

    /// Compare every immutable persisted operand to a genuinely constructed or independently
    /// re-admitted mathematical capability. No data-only decoded operands satisfy this check.
    pub(super) fn require_actual_generated(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        generated: &GeneratedOrdinaryCashCommitOriginalsV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        owner.recheck_proving_history(ProvingHistoryOperation::TerminalApproval)?;
        if self.reserve_request_original_sha256 == [0; 32]
            || &self.commit != generated.commit()
            || &self.successor != generated.selected_successor_state()
            || self.public_state_original != generated.successor_public_state_original()
            || self.private_state_checkpoint_original
                != generated.successor_private_checkpoint_original()
            || self.private_service_original != generated.private_service_original()
            || self.pre_receipt_outgoing_original != generated.pre_receipt_outgoing_original()
            || generated.proof().proof_bundle_original_sha256()
                != <DigestV1>::from(Sha256::digest(&self.private_service_original))
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let selection = owner.captured_terminal()?;
        let financial = owner.publication.cash_financial();
        let reservation = owner
            .lineage_cas
            .reservation_receipt(
                self.reserve_request_original_sha256,
                financial,
                &self.commit.reservation,
            )
            .map_err(material)?;
        selection.recheck_lineage_reservation(&reservation)?;
        if selection.selected_successor_state() != &self.successor
            || self
                .commit
                .reservation
                .selection
                .predecessor
                .state_commitment
                != owner.state.state_commitment
            || self
                .commit
                .reservation
                .selection
                .predecessor
                .logical_sequence
                != owner.state.logical_sequence
            || self
                .commit
                .reservation
                .selection
                .predecessor
                .state_original_sha256
                != <DigestV1>::from(Sha256::digest(&owner.public_state_original))
            || self.commit.reservation.successor.state_commitment != self.successor.state_commitment
            || self.commit.reservation.successor.logical_sequence != self.successor.logical_sequence
            || self.commit.reservation.successor.state_original_sha256
                != <DigestV1>::from(Sha256::digest(&self.public_state_original))
            || self.commit.purpose1_approval_original_sha256
                != <DigestV1>::from(Sha256::digest(selection.original()))
            || self.commit.terminal_record_original_sha256
                != <DigestV1>::from(Sha256::digest(
                    norito::encode_canonical(selection.terminal_record()).map_err(material)?,
                ))
            || self.commit.outgoing_original_sha256
                != <DigestV1>::from(Sha256::digest(&self.pre_receipt_outgoing_original))
            || self.commit.purpose1_financial_control_original_sha256
                != <DigestV1>::from(Sha256::digest(selection.financial_control_original()?))
            || self.commit.purpose1_clock_context_original_sha256
                != <DigestV1>::from(Sha256::digest(
                    norito::encode_canonical(selection.admission_clock_context())
                        .map_err(material)?,
                ))
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let outgoing = KagemushaOrdinaryCashOutgoingOriginalV1::decode_original(
            &self.pre_receipt_outgoing_original,
        )
        .map_err(material)?;
        let service = KagemushaOrdinaryLineageCommitProofBundleV1::decode_original(
            &self.private_service_original,
        )
        .map_err(material)?;
        if service.outgoing_original() != self.pre_receipt_outgoing_original
            || self.commit.terminal_proofs_original_sha256
                != <DigestV1>::from(Sha256::digest(service.inner_terminal_original()))
            || self.commit.wrapper_proofs_original_sha256
                != <DigestV1>::from(Sha256::digest(outgoing.wrapper_original()))
            || outgoing.credential_original() != selection.enrollment().app_credential().original()
            || outgoing.transition_statement() != selection.transition_statement()
            || outgoing.terminal_record() != selection.terminal_record()
            || outgoing.terminal_intent() != selection.terminal_intent()
            || outgoing.wrapper_original().is_empty()
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        owner
            .carrier_budget
            .require_reserved_bytes(
                selection
                    .outbox_reservation_original()
                    .reserved_outbox_bytes,
            )
            .map_err(material)?;
        // Full frames are re-budgeted after the actual acknowledged receipt is present.
        // Nothing here exposes the incomplete pre-receipt frame as a deliverable payment.
        reservation
            .recheck_historical(financial)
            .map_err(material)?;
        selection.recheck_selected_originals_and_current_custody()
    }
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    /// Retain the complete genuinely generated Commit before any global Commit request.
    /// The cap is owned; a caller's decoded private State or proof bytes cannot enter this API.
    pub(crate) fn retain_generated_commit(
        &mut self,
        generated: GeneratedOrdinaryCashCommitOriginalsV1,
        reserve_request_original_sha256: DigestV1,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let originals = PreparedCommitOriginals::from_actual_generated(
            self,
            &generated,
            reserve_request_original_sha256,
        )?;
        let digest: DigestV1 = originals.original_sha256(self.maximum_record_payload_bytes)?;
        if let Some(retained) = &self.prepared_commit {
            if retained.originals != originals {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            retained
                .originals
                .require_actual_generated(self, &retained.generated)?;
            self.require_current_financial_control()?;
            return Ok(digest);
        }
        self.require_outbox_capacity_for_new_slot()?;
        self.persist(&Record::PrepareCommit(originals.clone()))?;
        self.prepared_commit = Some(PreparedCommitAdmission {
            originals,
            generated,
        });
        // An expired post-fsync loan cannot cause regenerated proof bytes on retry.
        self.require_current_financial_control()?;
        Ok(digest)
    }

    /// Reserve global Commit only from the same durably retained cap and opaque Reserve receipt.
    pub(crate) fn reserve_retained_commit(&mut self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let prepared = self
            .prepared_commit
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        prepared
            .originals
            .require_actual_generated(self, &prepared.generated)?;
        let financial = self.publication.cash_financial();
        let current = self.control.loan(financial).map_err(material)?;
        let original = self
            .lineage_cas
            .reserve_commit(
                financial,
                &current,
                prepared.generated.proof(),
                prepared.originals.reserve_request_original_sha256,
            )
            .map_err(material)?;
        self.require_current_financial_control()?;
        Ok(original)
    }

    /// Apply an already acknowledged global Commit to the exact selected private financial State.
    /// This action performs only StateAdvance. A separately refreshed FI read and the distinct
    /// acknowledgment action must follow its fsync before any complete delivery is available.
    pub(crate) fn advance_acknowledged_commit(
        &mut self,
        commit_request_original_sha256: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_current_storage()?;
        if self.outbox.contains_key(&commit_request_original_sha256) {
            // Exact retry never creates another proof, global operation or local StateAdvance.
            if self
                .outbox
                .get(&commit_request_original_sha256)
                .is_some_and(|retained| retained.acknowledgment.is_some())
            {
                self.acknowledged_outgoing_original(commit_request_original_sha256)?;
                return Ok(());
            }
            return self
                .recheck_unacknowledged_outgoing_state_advance(commit_request_original_sha256);
        }
        self.require_current_financial_control()?;
        let prepared = self
            .prepared_commit
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let delivery = prepared.originals.complete_actual_delivery(
            self,
            &prepared.generated,
            commit_request_original_sha256,
        )?;
        let prepared_original_sha256 = prepared
            .originals
            .original_sha256(self.maximum_record_payload_bytes)?;
        self.require_outbox_capacity_for_new_slot()?;
        self.financial_journal_revision
            .checked_add(1)
            .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?;
        self.persist(&Record::StateAdvance {
            prepared_original_sha256,
            delivery: delivery.clone(),
        })?;
        // After this durable row, chronology must advance even if the following live read
        // expires. Reopening independently re-admits the exact persisted proof originals.
        if let Err(error) = self.install_actual_state_advance(prepared_original_sha256, delivery) {
            self.recovery_failed = true;
            return Err(error);
        }
        self.recheck_unacknowledged_outgoing_state_advance(commit_request_original_sha256)
    }

    /// Admit only the exact latest durable outgoing StateAdvance without lending money authority.
    /// This private phase boundary checks the original storage/receipt and current FI, but cannot
    /// consume the W1 proof capture or create the separately owned post-State acknowledgment.
    fn recheck_unacknowledged_outgoing_state_advance(
        &self,
        commit_request_original_sha256: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck()?;
        let advance = self
            .state_advance
            .as_ref()
            .and_then(RetainedFinancialStateAdvance::outgoing)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let retained = self
            .outbox
            .get(&commit_request_original_sha256)
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if advance.commit_request_original_sha256 != commit_request_original_sha256
            || retained.acknowledgment.is_some()
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let financial = self.publication.cash_financial();
        self.lineage_cas
            .commit_receipt(commit_request_original_sha256, financial, &retained.commit)
            .map_err(material)?
            .recheck_for_effect(financial, &self.control.loan(financial).map_err(material)?)
            .map_err(material)
    }

    /// Acknowledge only the actual retained StateAdvance after a fresh FI capture and clock sample.
    /// A new FI read may be needed; neither the old proof decision nor its expiry is renewed.
    pub(crate) fn acknowledge_state_advance(
        &mut self,
        commit_request_original_sha256: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_current_storage()?;
        if self.recovery_catalog.is_some() || self.recovery_failed {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let advance = self
            .state_advance
            .as_ref()
            .and_then(RetainedFinancialStateAdvance::outgoing)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if advance.commit_request_original_sha256 != commit_request_original_sha256 {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let retained = self
            .outbox
            .get(&commit_request_original_sha256)
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if retained.acknowledgment.is_some() {
            self.require_current_financial_control()?;
            return Ok(());
        }
        let financial = self.publication.cash_financial();
        let receipt = self
            .lineage_cas
            .commit_receipt(commit_request_original_sha256, financial, &retained.commit)
            .map_err(material)?;
        receipt
            .recheck_for_effect(financial, &self.control.loan(financial).map_err(material)?)
            .map_err(material)?;
        let captured = self
            .control
            .capture_proof_decision(financial)
            .map_err(material)?;
        let identity = CapturedFinancialControlIdentity {
            original_sha256: captured.original_sha256().map_err(material)?,
            lower_ms: captured.captured_lower_ms(),
            upper_ms: captured.captured_upper_ms(),
        };
        let clock = financial.current_cash_clock_context().map_err(material)?;
        receipt
            .recheck_captured_effect(financial, &captured, &clock)
            .map_err(material)?;
        let acknowledgment = StateAdvanceAcknowledgment {
            financial_control: identity,
            clock,
        };
        self.persist(&Record::StateAdvanceAcknowledged {
            commit_request_original_sha256,
            acknowledgment,
        })?;
        self.outbox
            .get_mut(&commit_request_original_sha256)
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .acknowledgment = Some(acknowledgment);
        self.require_current_financial_control()
    }

    /// Return only the complete original whose financial State/global effect was durably acknowledged.
    /// Every exposure separately checks current FI/PI/clock; historical Ack never lends a live grant.
    pub(crate) fn acknowledged_outgoing_original(
        &self,
        commit_request_original_sha256: DigestV1,
    ) -> Result<&[u8], KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let retained = self
            .outbox
            .get(&commit_request_original_sha256)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if retained.acknowledgment.is_none() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let financial = self.publication.cash_financial();
        let receipt = self
            .lineage_cas
            .commit_receipt(commit_request_original_sha256, financial, &retained.commit)
            .map_err(material)?;
        receipt
            .recheck_for_effect(financial, &self.control.loan(financial).map_err(material)?)
            .map_err(material)?;
        self.require_current_financial_control()?;
        Ok(&retained.delivery.complete_outgoing_original)
    }

    pub(super) fn require_state_advance_acknowledged(&self) -> Result<(), KagemushaStateErrorV1> {
        if let Some(RetainedFinancialStateAdvance::Incoming(advance)) = &self.state_advance {
            return self.require_incoming_state_advance_acknowledged(advance);
        }
        if let Some(RetainedFinancialStateAdvance::Outgoing(advance)) = &self.state_advance {
            if self
                .outbox
                .get(&advance.commit_request_original_sha256)
                .is_none_or(|retained| retained.acknowledgment.is_none())
            {
                return Err(KagemushaStateErrorV1::InvalidCandidateStage);
            }
        }
        Ok(())
    }

    pub(super) fn require_outbox_capacity_for_new_slot(&self) -> Result<(), KagemushaStateErrorV1> {
        require_slot_capacity(
            self.outbox.len(),
            self.outgoing_completion_slot_bytes()?,
            self.capacity.outbox_bytes,
        )
    }

    pub(super) fn recheck_state_advance_historical(&self) -> Result<(), KagemushaStateErrorV1> {
        let financial = self.publication.cash_financial();
        match &self.state_advance {
            None => {
                if self.state != *self.publication.historical_initial_state()?
                    || <DigestV1>::from(Sha256::digest(&self.public_state_original))
                        != self.lineage_originals[3]
                    || !self.outbox.is_empty()
                    || !self.incoming_commits.is_empty()
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
            }
            Some(RetainedFinancialStateAdvance::Incoming(advance)) => {
                self.recheck_latest_incoming_state_advance(advance)?;
            }
            Some(RetainedFinancialStateAdvance::Outgoing(advance)) => {
                let key = advance.commit_request_original_sha256;
                let retained = self
                    .outbox
                    .get(&key)
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if self.state != *advance.generated.selected_successor_state()
                    || self.public_state_original
                        != advance.generated.successor_public_state_original()
                    || &retained.commit != advance.generated.commit()
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                self.lineage_cas
                    .commit_receipt(key, financial, advance.generated.commit())
                    .map_err(material)?
                    .recheck_historical(financial)
                    .map_err(material)?;
            }
        }
        self.recheck_incoming_state_advance_history()?;
        for (key, retained) in &self.outbox {
            let receipt = self
                .lineage_cas
                .commit_receipt(*key, financial, &retained.commit)
                .map_err(material)?;
            receipt.recheck_historical(financial).map_err(material)?;
            if receipt.receiver_original(financial).map_err(material)?
                != retained.delivery.receiver_assertion_original
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            if let Some(acknowledgment) = &retained.acknowledgment {
                let identity = acknowledgment.financial_control;
                let captured = self
                    .control
                    .borrow_captured_proof_decision(
                        financial,
                        identity.original_sha256,
                        identity.lower_ms,
                        identity.upper_ms,
                    )
                    .map_err(material)?;
                receipt
                    .recheck_captured_effect(financial, &captured, &acknowledgment.clock)
                    .map_err(material)?;
            }
        }
        Ok(())
    }

    pub(super) fn replay_prepared_commit(
        &mut self,
        originals: PreparedCommitOriginals,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.prepared_commit.is_some() || self.pending_receiver_request.is_some() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.require_state_advance_acknowledged()?;
        let financial = self.publication.cash_financial();
        let reservation = self
            .lineage_cas
            .reservation_receipt(
                originals.reserve_request_original_sha256,
                financial,
                &originals.commit.reservation,
            )
            .map_err(material)?;
        let selection = self.captured_terminal()?;
        let generated = crate::kagemusha_v1_recursion::readmit_ordinary_cash_commit_v1(
            &self.verifier,
            &selection,
            &reservation,
            &originals.commit,
            &originals.private_service_original,
            &originals.pre_receipt_outgoing_original,
            &originals.public_state_original,
        )
        .map_err(material)?;
        originals.require_actual_generated(self, &generated)?;
        self.require_outbox_capacity_for_new_slot()?;
        self.prepared_commit = Some(PreparedCommitAdmission {
            originals,
            generated,
        });
        Ok(())
    }

    pub(super) fn install_actual_state_advance(
        &mut self,
        prepared_original_sha256: DigestV1,
        delivery: FinalizedDeliveryOriginals,
    ) -> Result<(), KagemushaStateErrorV1> {
        let prepared = self
            .prepared_commit
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if prepared
            .originals
            .original_sha256(self.maximum_record_payload_bytes)?
            != prepared_original_sha256
            || self
                .outbox
                .contains_key(&delivery.commit_request_original_sha256)
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        prepared
            .originals
            .require_actual_generated(self, &prepared.generated)?;
        prepared
            .originals
            .recheck_historical_delivery(self, &delivery)?;
        self.require_outbox_capacity_for_new_slot()?;
        let next_revision = self
            .financial_journal_revision
            .checked_add(1)
            .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?;
        // Clear only the actually captured terminal under that exact proof-admitted Commit.
        // This private method checks the cap itself; no external success flag can clear it.
        let receipt = self
            .lineage_cas
            .commit_receipt(
                delivery.commit_request_original_sha256,
                self.publication.cash_financial(),
                prepared.generated.commit(),
            )
            .map_err(material)?;
        self.terminal
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .consume_actual_commit(
                &prepared.generated,
                &receipt,
                self.publication.cash_financial(),
            )?;
        let prepared = self
            .prepared_commit
            .take()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let key = delivery.commit_request_original_sha256;
        use zeroize::Zeroize as _;
        self.state.balance.zeroize();
        self.state.state_nonce_commitment.zeroize();
        self.state = prepared.generated.selected_successor_state().clone();
        self.public_state_original = prepared
            .generated
            .successor_public_state_original()
            .to_vec();
        self.pending = None;
        self.outgoing_proof_operands = None;
        self.outgoing_reservation_candidate = None;
        self.financial_journal_revision = next_revision;
        self.outbox.insert(
            key,
            RetainedDelivery {
                commit: prepared.generated.commit().clone(),
                delivery,
                acknowledgment: None,
            },
        );
        self.state_advance = Some(RetainedFinancialStateAdvance::Outgoing(
            RetainedStateAdvance {
                generated: prepared.generated,
                commit_request_original_sha256: key,
            },
        ));
        self.recheck_state_advance_historical()
    }

    pub(super) fn replay_state_advance_acknowledgment(
        &mut self,
        key: DigestV1,
        acknowledgment: StateAdvanceAcknowledgment,
    ) -> Result<(), KagemushaStateErrorV1> {
        let advance = self
            .state_advance
            .as_ref()
            .and_then(RetainedFinancialStateAdvance::outgoing)
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let retained = self
            .outbox
            .get(&key)
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if advance.commit_request_original_sha256 != key
            || retained.acknowledgment.is_some()
            || self.pending.is_some()
            || self.prepared_commit.is_some()
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let financial = self.publication.cash_financial();
        let identity = acknowledgment.financial_control;
        let captured = self
            .control
            .borrow_captured_proof_decision(
                financial,
                identity.original_sha256,
                identity.lower_ms,
                identity.upper_ms,
            )
            .map_err(material)?;
        self.lineage_cas
            .commit_receipt(key, financial, &retained.commit)
            .map_err(material)?
            .recheck_captured_effect(financial, &captured, &acknowledgment.clock)
            .map_err(material)?;
        self.outbox
            .get_mut(&key)
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .acknowledgment = Some(acknowledgment);
        Ok(())
    }
}

fn require_slot_capacity(
    retained: usize,
    slot_bytes: u32,
    capacity: u64,
) -> Result<(), KagemushaStateErrorV1> {
    if slot_bytes == 0
        || u64::try_from(retained)
            .ok()
            .and_then(|count| count.checked_add(1))
            .and_then(|count| count.checked_mul(u64::from(slot_bytes)))
            .is_none_or(|reserved| reserved > capacity)
    {
        return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_outbox_slots_conserve_the_same_native_physical_capacity() {
        assert!(require_slot_capacity(0, 100, 100).is_ok());
        assert!(require_slot_capacity(1, 100, 199).is_err());
        assert!(require_slot_capacity(1, 100, 200).is_ok());
        assert!(require_slot_capacity(0, 0, u64::MAX).is_err());
        assert!(require_slot_capacity(usize::MAX, u32::MAX, u64::MAX).is_err());
    }
}

/// Completed public delivery data after the genuine sender CAS acknowledgment. This remains
/// data until Main durably commits/acknowledges the actual selected private financial State.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryCashFinalizedDeliveryV1")]
pub(super) struct FinalizedDeliveryOriginals {
    commit_request_original_sha256: DigestV1,
    receiver_assertion_original: Vec<u8>,
    complete_outgoing_original: Vec<u8>,
}
impl PreparedCommitOriginals {
    pub(super) fn complete_actual_delivery(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        generated: &GeneratedOrdinaryCashCommitOriginalsV1,
        commit_request_original_sha256: DigestV1,
    ) -> Result<FinalizedDeliveryOriginals, KagemushaStateErrorV1> {
        self.require_actual_generated(owner, generated)?;
        owner.require_current_financial_control()?;
        let financial = owner.publication.cash_financial();
        let current = owner.control.loan(financial).map_err(material)?;
        let receipt = owner
            .lineage_cas
            .commit_receipt(commit_request_original_sha256, financial, &self.commit)
            .map_err(material)?;
        receipt
            .recheck_for_effect(financial, &current)
            .map_err(material)?;
        let receiver_assertion_original = receipt.receiver_original(financial).map_err(material)?;
        // The outer vector holds two complete independently authenticated originals. Neither
        // selector hashes this future receipt into its original acyclic Commit preimage.
        let complete_outgoing_original = norito::encode_canonical(&vec![
            self.pre_receipt_outgoing_original.clone(),
            receiver_assertion_original.clone(),
        ])
        .map_err(material)?;
        let selection = owner.captured_terminal()?;
        let reserved = selection
            .outbox_reservation_original()
            .reserved_outbox_bytes;
        owner
            .carrier_budget
            .require_assembled_frames(
                &self.private_service_original,
                &complete_outgoing_original,
                reserved,
            )
            .map_err(material)?;
        if u64::from(reserved) > owner.capacity.outbox_bytes
            || complete_outgoing_original.len() as u64 > owner.maximum_record_payload_bytes
        {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        receipt
            .recheck_for_effect(financial, &current)
            .map_err(material)?;
        owner.require_current_financial_control()?;
        Ok(FinalizedDeliveryOriginals {
            commit_request_original_sha256,
            receiver_assertion_original,
            complete_outgoing_original,
        })
    }

    /// Authenticate persisted delivery against the same real historical CAS receipt. A replay
    /// record can supply bytes but cannot manufacture the sender receipt, FI or Native clock.
    pub(super) fn recheck_historical_delivery(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        delivery: &FinalizedDeliveryOriginals,
    ) -> Result<(), KagemushaStateErrorV1> {
        let financial = owner.publication.cash_financial();
        let receipt = owner
            .lineage_cas
            .commit_receipt(
                delivery.commit_request_original_sha256,
                financial,
                &self.commit,
            )
            .map_err(material)?;
        receipt.recheck_historical(financial).map_err(material)?;
        if receipt.receiver_original(financial).map_err(material)?
            != delivery.receiver_assertion_original
            || norito::encode_canonical(&vec![
                self.pre_receipt_outgoing_original.clone(),
                delivery.receiver_assertion_original.clone(),
            ])
            .map_err(material)?
                != delivery.complete_outgoing_original
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        // Recovery conserves the same actual W2 physical reservation; a large decoded row
        // never supplies a new slot or truncates either complete proof/receipt original.
        let selected = owner.captured_terminal()?;
        owner
            .carrier_budget
            .require_assembled_frames(
                &self.private_service_original,
                &delivery.complete_outgoing_original,
                selected.outbox_reservation_original().reserved_outbox_bytes,
            )
            .map_err(material)?;
        receipt.recheck_historical(financial).map_err(material)
    }
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    pub(super) fn retained_outgoing_commit_digest(
        &self,
    ) -> Result<Option<DigestV1>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        self.prepared_commit
            .as_ref()
            .map(|prepared| {
                prepared
                    .originals
                    .require_actual_generated(self, &prepared.generated)?;
                prepared
                    .originals
                    .original_sha256(self.maximum_record_payload_bytes)
            })
            .transpose()
    }
    pub(super) fn sign_retained_outgoing_commit_transport(
        &mut self,
        sign: impl FnOnce(
            &KagemushaAuthenticatedOrdinaryLineageAccountSigningV1<'_>,
        ) -> Result<[u8; 64], KagemushaStateErrorV1>,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let prepared = self
            .prepared_commit
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        prepared
            .originals
            .require_actual_generated(self, &prepared.generated)?;
        let service = prepared.generated.private_service_original().to_vec();
        let financial = self.publication.cash_financial();
        let current = self.control.loan(financial).map_err(material)?;
        let acknowledged = self
            .lineage_cas
            .acknowledged_outgoing_commit_request(financial, &current, prepared.generated.proof())
            .map_err(material)?;
        let (status, request, signature) = if let Some((request, signature)) = acknowledged {
            (2, request, signature.to_vec())
        } else {
            self.reserve_retained_commit()?;
            let fields = self.sign_retained_lineage_request(sign)?;
            match fields.as_slice() {
                [request, signature] => (0, request.clone(), signature.clone()),
                _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
            }
        };
        let key: DigestV1 = Sha256::digest(&request).into();
        self.require_current_financial_control()?;
        Ok(vec![
            vec![status],
            request,
            signature,
            service,
            key.to_vec(),
        ])
    }
}
