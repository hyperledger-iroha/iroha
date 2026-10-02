//! Distinct incoming financial commits in the sole Main WAL.
//! Full mathematical admission and authentic global Commit precede StateAdvance; a separate
//! post-fsync acknowledgement retires the exact source key. Decoded rows lend no authority.
use super::incoming::SourceLocator;
use super::*;
use crate::kagemusha_v1_recursion::{
    GeneratedOrdinaryIncomingCommitOriginalsV1,
    KAGEMUSHA_ORDINARY_INCOMING_COMMIT_BUNDLE_MAX_BYTES_V1,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1, KagemushaOrdinaryIncomingCommitV1,
};

/// One latest financial head, regardless of whether the actual operation was outgoing or incoming.
pub(super) enum RetainedFinancialStateAdvance {
    Outgoing(RetainedStateAdvance),
    Incoming(RetainedIncomingStateAdvance),
}
impl RetainedFinancialStateAdvance {
    pub(super) fn with_successor_checkpoint(
        &self,
        verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
        state: &KagemushaStateV1,
        public_state_original: &[u8],
        consume: &mut dyn for<'a> FnMut(
            &'a crate::kagemusha_v1_recursion::KagemushaGeneratedRecursiveStateProofV1,
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> core::result::Result<(), KagemushaStateErrorV1> {
        match self {
            Self::Incoming(value) => {
                if value.generated.selected_successor_state() != state
                    || value.generated.successor_public_state_original() != public_state_original
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                value.generated.with_successor_checkpoint(verifier, consume)
            }
            Self::Outgoing(value) => {
                if value.generated.selected_successor_state() != state
                    || value.generated.successor_public_state_original() != public_state_original
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                value.generated.with_successor_checkpoint(verifier, consume)
            }
        }
    }
    pub(super) fn outgoing(&self) -> Option<&RetainedStateAdvance> {
        match self {
            Self::Outgoing(value) => Some(value),
            Self::Incoming(_) => None,
        }
    }
    fn incoming(&self) -> Option<&RetainedIncomingStateAdvance> {
        match self {
            Self::Incoming(value) => Some(value),
            Self::Outgoing(_) => None,
        }
    }
}

/// Owned, independently admitted full incoming proofs; a decoded persistence row cannot make it.
pub(super) struct IncomingPreparedCommitAdmission {
    originals: IncomingPreparedCommitOriginals,
    generated: GeneratedOrdinaryIncomingCommitOriginalsV1,
}
pub(super) struct RetainedIncomingStateAdvance {
    generated: GeneratedOrdinaryIncomingCommitOriginalsV1,
    commit_request_original_sha256: DigestV1,
    terminal_admission_clock: KagemushaOrdinaryCashClockContextV1,
}

/// Bounded historical receipts remain after key retirement so exact retries and replay cannot
/// create another credit. Only the latest financial head retains the complete owned proof cap.
pub(super) struct RetainedIncomingCommit {
    commit: KagemushaOrdinaryIncomingCommitV1,
    advance: IncomingStateAdvanceOriginals,
    terminal_admission_clock: KagemushaOrdinaryCashClockContextV1,
    acknowledgement: Option<IncomingStateAdvanceAcknowledgment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryIncomingStateAdvanceAckV1")]
pub(super) struct IncomingStateAdvanceAcknowledgment {
    financial_control: CapturedFinancialControlIdentity,
    clock: KagemushaOrdinaryCashClockContextV1,
}

/// Complete public proof originals and private State held before the global irreversible request.
#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryIncomingPreparedCommitV1")]
pub(super) struct IncomingPreparedCommitOriginals {
    reserve_request_original_sha256: DigestV1,
    source: SourceLocator,
    commit: KagemushaOrdinaryIncomingCommitV1,
    successor: KagemushaStateV1,
    public_state_original: Vec<u8>,
    private_state_checkpoint_original: Vec<u8>,
    private_service_original: Vec<u8>,
}
impl core::fmt::Debug for IncomingPreparedCommitOriginals {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IncomingPreparedCommitOriginals")
            .field(
                "reserve_request_original_sha256",
                &self.reserve_request_original_sha256,
            )
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}
impl Drop for IncomingPreparedCommitOriginals {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        self.successor.balance.zeroize();
        self.successor.state_nonce_commitment.zeroize();
        self.private_service_original.zeroize();
        self.private_state_checkpoint_original.zeroize();
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryIncomingStateAdvanceV1")]
pub(super) struct IncomingStateAdvanceOriginals {
    prepared_original_sha256: DigestV1,
    commit_request_original_sha256: DigestV1,
    commit_original: Vec<u8>,
    source: SourceLocator,
    capacity_charge_bytes: u64,
}

impl IncomingPreparedCommitOriginals {
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
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        Ok(Sha256::digest(original.as_slice()).into())
    }

    fn require_actual_generated(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        generated: &GeneratedOrdinaryIncomingCommitOriginalsV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        owner.recheck_proving_history(ProvingHistoryOperation::IncomingTerminal)?;
        let selection = owner.captured_incoming_terminal()?;
        let w2 = selection.preparation_selection()?;
        let intent = selection.terminal_intent()?;
        let pending = owner
            .pending_incoming
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let reserve = owner
            .lineage_cas
            .incoming_reservation_receipt(
                self.reserve_request_original_sha256,
                owner.publication.cash_financial(),
                &self.commit.reservation,
            )
            .map_err(material)?;
        selection.recheck_incoming_reservation(&reserve)?;
        self.commit.validate_shape().map_err(material)?;
        self.successor.validate()?;
        if self.source != pending.intent.source
            || self.reserve_request_original_sha256 != intent.reserve_request_original_sha256
            || &self.commit != generated.commit()
            || generated.proof().commit() != &self.commit
            || generated.proof().release_id() != owner.state.release_id
            || &self.successor != generated.selected_successor_state()
            || self.public_state_original.is_empty()
            || self.public_state_original.len() > 32 * 1024
            || self.public_state_original != generated.successor_public_state_original()
            || self.private_state_checkpoint_original
                != generated.successor_private_checkpoint_original()
            || self.private_service_original != generated.private_service_original()
            || generated.proof().original() != self.private_service_original
            || self.private_service_original.is_empty()
            || self.private_service_original.len()
                > KAGEMUSHA_ORDINARY_INCOMING_COMMIT_BUNDLE_MAX_BYTES_V1
            || &self.commit.reservation != w2.reservation()?
            || self.commit.reservation.selection != pending.intent.selection
            || &self.successor != w2.selected_successor_state()?
            || self.commit.reservation.selection.predecessor != owner.incoming_current_head()
            || self.commit.successor.state_commitment != self.successor.state_commitment
            || self.commit.successor.logical_sequence != self.successor.logical_sequence
            || self.commit.successor.state_original_sha256 != sha(&self.public_state_original)
            || intent.state_original_sha256 != sha(&self.public_state_original)
            || intent.candidate_original_sha256
                != selection.candidate()?.candidate_original_sha256()
            || self.commit.transition_statement_original_sha256
                != intent.transition_statement_original_sha256
            || self.commit.purpose1_approval_original_sha256 != sha(selection.original()?)
            || self.commit.financial_control_original_sha256
                != sha(&selection.financial_control_original()?)
            || self.commit.admission_clock_context_original_sha256
                != sha(
                    &norito::encode_canonical(selection.admission_clock_context()?)
                        .map_err(material)?,
                )
            || owner.incoming_consumed.root() != owner.state.consumed_credit_root
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        require_incoming_financial_edge(
            &owner.state,
            &self.successor,
            self.commit.reservation.selection.amount,
            owner.financial_journal_revision,
            intent.logical_journal_sequence_after,
        )?;
        let insertion = w2.prepared_replay_insert()?;
        let witness = insertion.witness();
        if witness.credit_id != CreditIdV1(self.commit.reservation.selection.credit_id)
            || witness.envelope_digest != self.commit.reservation.digest().map_err(material)?
            || witness.predecessor_root != owner.state.consumed_credit_root
            || witness.successor_root != self.successor.consumed_credit_root
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        reserve
            .recheck_historical(owner.publication.cash_financial())
            .map_err(material)?;
        selection.recheck_selected_originals_and_current_custody()
    }

    /// Reserve the exact complete current Prepared frame plus full supported future receipt/Ack
    /// framing. The deliberately unauthenticated future row is used only for a numeric count.
    fn capacity_charge(&self, maximum_payload_bytes: u64) -> Result<u64, KagemushaStateErrorV1> {
        let prepared = frame_bytes(
            &Record::IncomingPrepareCommit(self.clone()),
            maximum_payload_bytes,
        )?;
        let advance = frame_bytes(
            &Record::IncomingStateAdvance(IncomingStateAdvanceOriginals {
                prepared_original_sha256: [0; 32],
                commit_request_original_sha256: [0; 32],
                commit_original: vec![0; KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1],
                source: self.source,
                capacity_charge_bytes: u64::MAX,
            }),
            maximum_payload_bytes,
        )?;
        let ack = frame_bytes(
            &Record::IncomingStateAdvanceAcknowledged {
                commit_request_original_sha256: [0; 32],
                acknowledgement: IncomingStateAdvanceAcknowledgment {
                    financial_control: CapturedFinancialControlIdentity {
                        original_sha256: [0; 32],
                        lower_ms: u64::MAX,
                        upper_ms: u64::MAX,
                    },
                    clock: KagemushaOrdinaryCashClockContextV1 {
                        version: 1,
                        request_nonce: [0; 32],
                        signed_observations_original_digest: [0; 32],
                        lower_at_ms: u64::MAX,
                        upper_at_ms: u64::MAX,
                    },
                },
            },
            maximum_payload_bytes,
        )?;
        checked_frame_charge([prepared, advance, ack], maximum_payload_bytes)
    }
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    pub(crate) fn retain_generated_incoming_commit(
        &mut self,
        generated: GeneratedOrdinaryIncomingCommitOriginalsV1,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let selection = self.captured_incoming_terminal()?;
        let originals = IncomingPreparedCommitOriginals {
            reserve_request_original_sha256: selection
                .terminal_intent()?
                .reserve_request_original_sha256,
            source: self
                .pending_incoming
                .as_ref()
                .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
                .intent
                .source,
            commit: generated.commit().clone(),
            successor: generated.selected_successor_state().clone(),
            public_state_original: generated.successor_public_state_original().to_vec(),
            private_state_checkpoint_original: generated
                .successor_private_checkpoint_original()
                .to_vec(),
            private_service_original: generated.private_service_original().to_vec(),
        };
        originals.require_actual_generated(self, &generated)?;
        self.require_incoming_commit_capacity(&originals)?;
        let digest = originals.original_sha256(self.maximum_record_payload_bytes)?;
        if let Some(retained) = &self.prepared_incoming_commit {
            if retained.originals != originals {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            retained
                .originals
                .require_actual_generated(self, &retained.generated)?;
            return Ok(digest);
        }
        if self.prepared_commit.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.require_incoming_rows(3)?;
        self.persist(&Record::IncomingPrepareCommit(originals.clone()))?;
        self.prepared_incoming_commit = Some(IncomingPreparedCommitAdmission {
            originals,
            generated,
        });
        self.require_current_financial_control()?;
        Ok(digest)
    }

    /// No account fence/dispatch occurs until the entire exact cap and future WAL quota are held.
    pub(crate) fn reserve_retained_incoming_commit(
        &mut self,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let prepared = self
            .prepared_incoming_commit
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        prepared
            .originals
            .require_actual_generated(self, &prepared.generated)?;
        self.require_incoming_commit_capacity(&prepared.originals)?;
        self.require_incoming_rows(2)?;
        let financial = self.publication.cash_financial();
        let current = self.control.loan(financial).map_err(material)?;
        let original = self
            .lineage_cas
            .reserve_incoming_commit(
                financial,
                &current,
                prepared.generated.proof(),
                prepared.originals.reserve_request_original_sha256,
            )
            .map_err(material)?;
        self.require_current_financial_control()?;
        Ok(original)
    }

    pub(crate) fn advance_acknowledged_incoming_commit(
        &mut self,
        key: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_current_storage()?;
        if let Some(retained) = self.incoming_commits.get(&key) {
            if retained.acknowledgement.is_some() {
                self.require_current_financial_control()?;
                return Ok(());
            }
            return self.acknowledge_incoming_state_advance(key);
        }
        self.require_current_financial_control()?;
        let prepared = self
            .prepared_incoming_commit
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        prepared
            .originals
            .require_actual_generated(self, &prepared.generated)?;
        self.require_incoming_commit_capacity(&prepared.originals)?;
        let financial = self.publication.cash_financial();
        let receipt = self
            .lineage_cas
            .incoming_commit_receipt(key, financial, prepared.generated.commit())
            .map_err(material)?;
        receipt
            .recheck_for_effect(financial, &self.control.loan(financial).map_err(material)?)
            .map_err(material)?;
        self.require_incoming_rows(2)?;
        let advance = IncomingStateAdvanceOriginals {
            prepared_original_sha256: prepared
                .originals
                .original_sha256(self.maximum_record_payload_bytes)?,
            commit_request_original_sha256: key,
            commit_original: receipt.original().map_err(material)?.to_vec(),
            source: prepared.originals.source,
            capacity_charge_bytes: self
                .incoming_operation_capacity_charge(prepared.originals.source)?,
        };
        self.require_incoming_source_retirement(&advance.source, prepared.generated.commit())?;
        self.persist(&Record::IncomingStateAdvance(advance.clone()))?;
        if let Err(error) = self.install_actual_incoming_state_advance(advance) {
            self.recovery_failed = true;
            return Err(error);
        }
        self.acknowledge_incoming_state_advance(key)
    }

    /// The full StateAdvance is durable already. Fresh FI and full clock custody are separately
    /// captured after that fsync; key retirement only follows the actual Ack row durability.
    pub(crate) fn acknowledge_incoming_state_advance(
        &mut self,
        key: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_current_storage()?;
        if self.recovery_catalog.is_some() || self.recovery_failed {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let latest = self
            .state_advance
            .as_ref()
            .and_then(RetainedFinancialStateAdvance::incoming)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if latest.commit_request_original_sha256 != key {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let retained = self
            .incoming_commits
            .get(&key)
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if retained.acknowledgement.is_some() {
            self.require_current_financial_control()?;
            return Ok(());
        }
        let financial = self.publication.cash_financial();
        let receipt = self
            .lineage_cas
            .incoming_commit_receipt(key, financial, &retained.commit)
            .map_err(material)?;
        receipt
            .recheck_for_effect(financial, &self.control.loan(financial).map_err(material)?)
            .map_err(material)?;
        self.require_incoming_rows(1)?;
        let captured = self
            .control
            .capture_proof_decision(financial)
            .map_err(material)?;
        let clock = financial.current_cash_clock_context().map_err(material)?;
        receipt
            .recheck_captured_effect(financial, &captured, &clock)
            .map_err(material)?;
        let old = latest.terminal_admission_clock;
        require_clock_order(&old, &clock)?;
        let acknowledgement = IncomingStateAdvanceAcknowledgment {
            financial_control: CapturedFinancialControlIdentity {
                original_sha256: captured.original_sha256().map_err(material)?,
                lower_ms: captured.captured_lower_ms(),
                upper_ms: captured.captured_upper_ms(),
            },
            clock,
        };
        self.require_incoming_source_retirement(&retained.advance.source, &retained.commit)?;
        self.persist(&Record::IncomingStateAdvanceAcknowledged {
            commit_request_original_sha256: key,
            acknowledgement,
        })?;
        if let Err(error) = self.install_incoming_state_advance_ack(key, acknowledgement) {
            self.recovery_failed = true;
            return Err(error);
        }
        self.require_current_financial_control()
    }

    pub(super) fn replay_incoming_prepared_commit(
        &mut self,
        originals: IncomingPreparedCommitOriginals,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.prepared_incoming_commit.is_some()
            || self.prepared_commit.is_some()
            || self.pending_receiver_request.is_some()
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.require_state_advance_acknowledged()?;
        let selection = self.captured_incoming_terminal()?;
        let reserve = self
            .lineage_cas
            .incoming_reservation_receipt(
                originals.reserve_request_original_sha256,
                self.publication.cash_financial(),
                &originals.commit.reservation,
            )
            .map_err(material)?;
        let generated = crate::kagemusha_v1_recursion::readmit_ordinary_incoming_commit_v1(
            &self.verifier,
            &selection,
            &reserve,
            &originals.commit,
            &originals.private_service_original,
            &originals.public_state_original,
        )
        .map_err(material)?;
        originals.require_actual_generated(self, &generated)?;
        self.require_incoming_commit_capacity(&originals)?;
        self.prepared_incoming_commit = Some(IncomingPreparedCommitAdmission {
            originals,
            generated,
        });
        Ok(())
    }

    pub(super) fn install_actual_incoming_state_advance(
        &mut self,
        advance: IncomingStateAdvanceOriginals,
    ) -> Result<(), KagemushaStateErrorV1> {
        let prepared = self
            .prepared_incoming_commit
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        prepared
            .originals
            .require_actual_generated(self, &prepared.generated)?;
        self.require_incoming_commit_capacity(&prepared.originals)?;
        if advance.prepared_original_sha256
            != prepared
                .originals
                .original_sha256(self.maximum_record_payload_bytes)?
            || advance.source != prepared.originals.source
            || advance.capacity_charge_bytes
                != self.incoming_operation_capacity_charge(prepared.originals.source)?
            || self
                .incoming_commits
                .contains_key(&advance.commit_request_original_sha256)
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let receipt = self
            .lineage_cas
            .incoming_commit_receipt(
                advance.commit_request_original_sha256,
                self.publication.cash_financial(),
                prepared.generated.commit(),
            )
            .map_err(material)?;
        receipt
            .recheck_historical(self.publication.cash_financial())
            .map_err(material)?;
        if receipt.original().map_err(material)? != advance.commit_original {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.require_incoming_source_retirement(&advance.source, prepared.generated.commit())?;
        let terminal_admission_clock = *self
            .captured_incoming_terminal()?
            .admission_clock_context()?;
        let revision = self
            .financial_journal_revision
            .checked_add(1)
            .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?;
        let insertion = self
            .captured_incoming_approval()?
            .prepared_replay_insert()?
            .clone();
        let prepared = self
            .prepared_incoming_commit
            .take()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        // The sole replay insertion is prepared by actual W2 against the exact predecessor.
        // This kernel validates all paths before any index mutation; any later error freezes Main.
        self.incoming_consumed.install_prepared_insert(insertion)?;
        use zeroize::Zeroize as _;
        self.state.balance.zeroize();
        self.state.state_nonce_commitment.zeroize();
        self.state = prepared.generated.selected_successor_state().clone();
        self.public_state_original = prepared
            .generated
            .successor_public_state_original()
            .to_vec();
        self.financial_journal_revision = revision;
        self.pending_incoming = None;
        self.incoming_reservation_candidate = None;
        let key = advance.commit_request_original_sha256;
        self.incoming_commits.insert(
            key,
            RetainedIncomingCommit {
                commit: prepared.generated.commit().clone(),
                advance,
                terminal_admission_clock,
                acknowledgement: None,
            },
        );
        self.state_advance = Some(RetainedFinancialStateAdvance::Incoming(
            RetainedIncomingStateAdvance {
                generated: prepared.generated,
                commit_request_original_sha256: key,
                terminal_admission_clock,
            },
        ));
        self.recheck_state_advance_historical()
    }

    pub(super) fn install_incoming_state_advance_ack(
        &mut self,
        key: DigestV1,
        acknowledgement: IncomingStateAdvanceAcknowledgment,
    ) -> Result<(), KagemushaStateErrorV1> {
        let latest = self
            .state_advance
            .as_ref()
            .and_then(RetainedFinancialStateAdvance::incoming)
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let retained = self
            .incoming_commits
            .get(&key)
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if latest.commit_request_original_sha256 != key
            || retained.acknowledgement.is_some()
            || self.pending_incoming.is_some()
            || self.prepared_incoming_commit.is_some()
            || self.pending.is_some()
            || self.prepared_commit.is_some()
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let financial = self.publication.cash_financial();
        let identity = acknowledgement.financial_control;
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
            .incoming_commit_receipt(key, financial, &retained.commit)
            .map_err(material)?
            .recheck_captured_effect(financial, &captured, &acknowledgement.clock)
            .map_err(material)?;
        require_clock_order(&latest.terminal_admission_clock, &acknowledgement.clock)?;
        let source = retained.advance.source;
        let commit = retained.commit.clone();
        self.require_incoming_source_retirement(&source, &commit)?;
        // No key is removed merely because a proof/decrypt/global transport succeeded.
        // The live caller has fsynced Ack; recovery reaches only the authenticated Ack row.
        match source {
            SourceLocator::Mint => self.pending_mint = None,
            SourceLocator::Receive { request_id } => {
                self.received_sources.remove(&request_id);
                self.retained_receiver_requests.remove(&request_id);
            }
        }
        self.incoming_commits
            .get_mut(&key)
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .acknowledgement = Some(acknowledgement);
        Ok(())
    }

    fn require_incoming_source_retirement(
        &self,
        source: &SourceLocator,
        commit: &KagemushaOrdinaryIncomingCommitV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        match *source {
            SourceLocator::Mint => self.require_held_mint_incoming_source(&commit.reservation),
            SourceLocator::Receive { request_id } => {
                self.require_held_received_incoming_source(request_id, &commit.reservation)
            }
        }
    }

    pub(super) fn require_incoming_state_advance_acknowledged(
        &self,
        latest: &RetainedIncomingStateAdvance,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self
            .incoming_commits
            .get(&latest.commit_request_original_sha256)
            .is_none_or(|v| v.acknowledgement.is_none())
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        Ok(())
    }

    pub(super) fn recheck_latest_incoming_state_advance(
        &self,
        latest: &RetainedIncomingStateAdvance,
    ) -> Result<(), KagemushaStateErrorV1> {
        let retained = self
            .incoming_commits
            .get(&latest.commit_request_original_sha256)
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if self.state != *latest.generated.selected_successor_state()
            || self.public_state_original != latest.generated.successor_public_state_original()
            || &retained.commit != latest.generated.commit()
            || retained.terminal_admission_clock != latest.terminal_admission_clock
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.lineage_cas
            .incoming_commit_receipt(
                latest.commit_request_original_sha256,
                self.publication.cash_financial(),
                latest.generated.commit(),
            )
            .map_err(material)?
            .recheck_historical(self.publication.cash_financial())
            .map_err(material)
    }

    pub(super) fn recheck_incoming_state_advance_history(
        &self,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.incoming_consumed.root() != self.state.consumed_credit_root {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let financial = self.publication.cash_financial();
        for (key, retained) in &self.incoming_commits {
            let receipt = self
                .lineage_cas
                .incoming_commit_receipt(*key, financial, &retained.commit)
                .map_err(material)?;
            receipt.recheck_historical(financial).map_err(material)?;
            if *key != retained.advance.commit_request_original_sha256
                || receipt.original().map_err(material)? != retained.advance.commit_original
                || self
                    .incoming_consumed
                    .get(CreditIdV1(retained.commit.reservation.selection.credit_id))
                    != Some(retained.commit.reservation.digest().map_err(material)?)
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            if let Some(ack) = retained.acknowledgement {
                let i = ack.financial_control;
                let captured = self
                    .control
                    .borrow_captured_proof_decision(
                        financial,
                        i.original_sha256,
                        i.lower_ms,
                        i.upper_ms,
                    )
                    .map_err(material)?;
                receipt
                    .recheck_captured_effect(financial, &captured, &ack.clock)
                    .map_err(material)?;
                require_clock_order(&retained.terminal_admission_clock, &ack.clock)?;
            } else {
                self.require_incoming_source_retirement(
                    &retained.advance.source,
                    &retained.commit,
                )?;
            }
        }
        Ok(())
    }

    /// Permanent physical history is charged after Ack replaces the still-held source/key
    /// reservation. Before Ack that exact source remains charged, including the future suffix.
    /// Thus neither double counting nor key retirement releases the authenticated WAL quota.
    pub(super) fn retained_incoming_commit_capacity_charge(
        &self,
    ) -> Result<u64, KagemushaStateErrorV1> {
        self.incoming_commits
            .values()
            .filter(|v| v.acknowledgement.is_some())
            .try_fold(0u64, |sum, value| {
                if value.advance.capacity_charge_bytes == 0 {
                    return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
                }
                sum.checked_add(value.advance.capacity_charge_bytes)
                    .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)
            })
    }

    /// Receive has no online debit before W2; reserve its whole supported suffix when the real
    /// W2 Prepared is selected, before either platform fence. After StateAdvance, its same
    /// validated recorded suffix stays reserved until Ack replaces source/key by full history.
    pub(super) fn retained_incoming_receive_suffix_capacity(
        &self,
    ) -> Result<u64, KagemushaStateErrorV1> {
        if let Some(pending) = &self.pending_incoming {
            if matches!(pending.intent.source, SourceLocator::Receive { .. }) {
                return pending
                    .approval
                    .as_ref()
                    .map_or(Ok(0), |p| p.capacity_charge_bytes());
            }
        }
        if let Some(latest) = self
            .state_advance
            .as_ref()
            .and_then(RetainedFinancialStateAdvance::incoming)
        {
            let held = self
                .incoming_commits
                .get(&latest.commit_request_original_sha256)
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
            if held.acknowledgement.is_none() {
                if let SourceLocator::Receive { request_id } = held.advance.source {
                    return held
                        .advance
                        .capacity_charge_bytes
                        .checked_sub(self.held_received_source_capacity_charge(request_id)?)
                        .filter(|n| *n != 0)
                        .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity);
                }
            }
        }
        Ok(0)
    }

    fn incoming_operation_capacity_charge(
        &self,
        source: SourceLocator,
    ) -> Result<u64, KagemushaStateErrorV1> {
        match source {
            SourceLocator::Mint => self
                .pending_mint
                .as_ref()
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                .capacity_charge_bytes(),
            SourceLocator::Receive { request_id } => {
                let prepared = self
                    .pending_incoming
                    .as_ref()
                    .and_then(|p| p.approval.as_ref())
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                self.held_received_source_capacity_charge(request_id)?
                    .checked_add(prepared.capacity_charge_bytes()?)
                    .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)
            }
        }
    }

    pub(super) fn require_incoming_completion_rows(&self) -> Result<(), KagemushaStateErrorV1> {
        self.require_incoming_rows(incoming_preparation::INCOMING_COMPLETION_MAIN_ROWS_V1)
    }

    pub(super) fn require_incoming_rows(&self, rows: u64) -> Result<(), KagemushaStateErrorV1> {
        if self
            .prefix
            .sequence
            .checked_add(rows)
            .is_none_or(|v| v > MAX_ROWS)
        {
            return Err(KagemushaStateErrorV1::JournalRevisionOverflow);
        }
        self.financial_journal_revision
            .checked_add(1)
            .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?;
        Ok(())
    }

    fn require_incoming_commit_capacity(
        &self,
        originals: &IncomingPreparedCommitOriginals,
    ) -> Result<(), KagemushaStateErrorV1> {
        let pending = self
            .pending_incoming
            .as_ref()
            .and_then(|p| p.approval.as_ref())
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let intent = &self
            .pending_incoming
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .intent;
        let terminal = self
            .pending_incoming
            .as_ref()
            .and_then(|p| p.terminal.as_ref())
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let terminal_bytes =
            terminal.captured_wal_charge_bytes(self.maximum_record_payload_bytes)?;
        let intent_bytes = frame_bytes(
            &Record::IncomingIntent(intent.clone()),
            self.maximum_record_payload_bytes,
        )?;
        let candidate_bytes = self
            .incoming_reservation_candidate
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .capacity_charge(self)?;
        let actual = originals
            .capacity_charge(self.maximum_record_payload_bytes)?
            .checked_add(pending.captured_suffix_charge_bytes()?)
            .and_then(|n| n.checked_add(candidate_bytes))
            .and_then(|n| n.checked_add(terminal_bytes))
            .and_then(|n| n.checked_add(intent_bytes))
            .and_then(|n| n.checked_add(256))
            .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
        if actual > pending.completion_capacity_bytes()? {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        // This rechecks the same aggregate original inbox and whole pre-held suffix; adding the
        // complete proof again would charge the same reservation twice. No quota is released.
        self.recheck_receiver_request_storage()?;
        if self.incoming_commits.len() >= MAX_ROWS as usize {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        self.financial_journal_revision
            .checked_add(1)
            .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?;
        Ok(())
    }
}

fn sha(raw: &[u8]) -> DigestV1 {
    Sha256::digest(raw).into()
}
fn frame_bytes(record: &Record, maximum_payload_bytes: u64) -> Result<u64, KagemushaStateErrorV1> {
    let bytes =
        u64::try_from(norito::canonical_frame_len(record).map_err(material)?).map_err(material)?;
    if bytes == 0 || bytes > maximum_payload_bytes {
        return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
    }
    Ok(bytes)
}

/// Count complete retained rows with their physical framing, preserving each row's actual bound.
pub(super) fn checked_record_capacity_charge(
    records: impl IntoIterator<Item = Record>,
    maximum_payload_bytes: u64,
) -> Result<u64, KagemushaStateErrorV1> {
    records.into_iter().try_fold(0u64, |sum, record| {
        sum.checked_add(frame_bytes(&record, maximum_payload_bytes)?)
            .and_then(|n| n.checked_add(256))
            .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)
    })
}

/// Move the authenticated original into its sole post-capture slot without losing its first cut.
/// Live capture and replay share this move; it performs no authentication or authority admission.
pub(super) fn install_captured_approval<Approval, Capture>(
    retained: &mut Option<(KagemushaOrdinaryCashClockContextV1, Approval)>,
    captured: &mut Option<(KagemushaOrdinaryCashClockContextV1, Capture, Approval)>,
    capture: Capture,
) -> Result<(), KagemushaStateErrorV1> {
    if captured.is_some() {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let (original_clock, approval) = retained
        .take()
        .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
    *captured = Some((original_clock, capture, approval));
    Ok(())
}
fn checked_frame_charge(
    frames: [u64; 3],
    maximum_payload_bytes: u64,
) -> Result<u64, KagemushaStateErrorV1> {
    frames.into_iter().try_fold(0u64, |sum, frame| {
        if frame == 0 || frame > maximum_payload_bytes {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        sum.checked_add(frame)
            .and_then(|v| v.checked_add(256))
            .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)
    })
}
fn require_clock_order(
    old: &KagemushaOrdinaryCashClockContextV1,
    new: &KagemushaOrdinaryCashClockContextV1,
) -> Result<(), KagemushaStateErrorV1> {
    old.validate_shape().map_err(material)?;
    new.validate_shape().map_err(material)?;
    if new.lower_at_ms < old.lower_at_ms || new.upper_at_ms < old.upper_at_ms {
        return Err(KagemushaStateErrorV1::SnapshotRollback);
    }
    Ok(())
}
fn require_incoming_financial_edge(
    before: &KagemushaStateV1,
    after: &KagemushaStateV1,
    amount: u128,
    journal_before: u64,
    journal_after: u64,
) -> Result<(), KagemushaStateErrorV1> {
    require_incoming_scalar_edge(
        (
            before.balance,
            before.secure_index,
            before.logical_sequence,
            journal_before,
        ),
        (
            after.balance,
            after.secure_index,
            after.logical_sequence,
            journal_after,
        ),
        amount,
    )?;
    if before.consumed_credit_root == after.consumed_credit_root {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

/// Separate full-u128 financial arithmetic from the sole u64 physical journal edge. This pure
/// comparison returns no proof/current/receipt capability; its caller also verifies real State.
fn require_incoming_scalar_edge(
    before: (u128, u128, u128, u64),
    after: (u128, u128, u128, u64),
    amount: u128,
) -> Result<(), KagemushaStateErrorV1> {
    if amount == 0
        || before.0.checked_add(amount) != Some(after.0)
        || before.1.checked_add(1) != Some(after.1)
        || before.2.checked_add(1) != Some(after.2)
        || before.3.checked_add(1) != Some(after.3)
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

// These tests use plain public DATA/scalars only. They construct neither an owner, receipt,
// source grant, generated proof nor a funded State. Full chronological owner controls require
// actual released proof material and independently acknowledged global receipts separately.
#[cfg(test)]
mod tests {
    use super::*;

    fn clock(tag: u8, lower: u64, upper: u64) -> KagemushaOrdinaryCashClockContextV1 {
        KagemushaOrdinaryCashClockContextV1 {
            version: 1,
            request_nonce: [tag; 32],
            signed_observations_original_digest: [tag + 1; 32],
            lower_at_ms: lower,
            upper_at_ms: upper,
        }
    }

    #[test]
    fn incoming_capture_move_retains_original_cut_for_live_and_replay_row_charges() {
        // Plain retained original data exercises the shared move used by all four authentic
        // live/replay callers. It constructs no verified approval, Native owner or proof grant.
        for tag in [1, 4] {
            let original_clock = clock(tag, 100, 110);
            let capture_clock = clock(tag + 1, 120, 130);
            let original = vec![tag; 97];
            let mut retained = Some((original_clock, original.clone()));
            let mut captured = None;
            install_captured_approval(&mut retained, &mut captured, capture_clock).unwrap();
            assert!(retained.is_none());
            let (first_cut, capture_cut, held_original) = captured.as_ref().unwrap();
            assert_eq!(*first_cut, original_clock);
            assert_eq!(*capture_cut, capture_clock);
            assert_eq!(held_original, &original);
            let rows = [
                Record::IncomingApproval(IncomingApprovalRecord::Original {
                    operation: [7; 32],
                    clock: *first_cut,
                    original: held_original.clone(),
                    authorization: [8; 32],
                    accepted_counter: Some(9),
                }),
                Record::IncomingApproval(IncomingApprovalRecord::Capture {
                    operation: [7; 32],
                    clock: *capture_cut,
                    authorization: [8; 32],
                }),
            ];
            let expected = rows
                .iter()
                .map(|row| u64::try_from(norito::canonical_frame_len(row).unwrap()).unwrap() + 256)
                .sum::<u64>();
            assert!(expected > original.len() as u64 + 2 * 256);
            assert_eq!(
                checked_record_capacity_charge(rows, 4096).unwrap(),
                expected
            );
            let before = captured.clone();
            retained = Some((clock(tag + 2, 140, 150), vec![10; 97]));
            assert_eq!(
                install_captured_approval(&mut retained, &mut captured, capture_clock),
                Err(KagemushaStateErrorV1::SnapshotIntegrity)
            );
            assert_eq!(captured, before);
            assert!(retained.is_some());
        }
    }

    #[test]
    fn incoming_captured_row_charge_keeps_each_physical_bound_after_original_move() {
        let mut retained: Option<(KagemushaOrdinaryCashClockContextV1, Vec<u8>)> = None;
        let mut captured = None;
        assert_eq!(
            install_captured_approval(&mut retained, &mut captured, clock(2, 120, 130)),
            Err(KagemushaStateErrorV1::SnapshotIntegrity)
        );
        assert!(retained.is_none());
        assert!(captured.is_none());
        let row = Record::IncomingTerminal(incoming_terminal::IncomingTerminalRecord::Original {
            operation: [7; 32],
            clock: clock(1, 100, 110),
            original: vec![8; 97],
            authorization: [9; 32],
            accepted_counter: Some(10),
        });
        let exact = u64::try_from(norito::canonical_frame_len(&row).unwrap()).unwrap();
        assert_eq!(
            checked_record_capacity_charge([row.clone(), row.clone()], exact).unwrap(),
            2 * (exact + 256)
        );
        assert!(checked_record_capacity_charge([row.clone()], exact - 1).is_err());
        assert!(checked_record_capacity_charge([row], 0).is_err());
    }

    #[test]
    fn incoming_financial_sequence_is_full_u128_and_journal_is_separate_u64() {
        let high = u128::from(u64::MAX) + 7;
        let before = (high, high + 1, high + 2, 19);
        let after = (high + 5, high + 2, high + 3, 20);
        assert!(require_incoming_scalar_edge(before, after, 5).is_ok());
        for substituted in [
            (high + 4, high + 2, high + 3, 20),
            (high + 5, high + 3, high + 3, 20),
            (high + 5, high + 2, high + 4, 20),
            (high + 5, high + 2, high + 3, 21),
        ] {
            assert!(require_incoming_scalar_edge(before, substituted, 5).is_err());
        }
        assert!(require_incoming_scalar_edge(before, after, 0).is_err());
    }

    #[test]
    fn incoming_all_financial_overflows_and_physical_journal_overflow_refuse() {
        assert!(require_incoming_scalar_edge((u128::MAX, 1, 1, 1), (0, 2, 2, 2), 1).is_err());
        assert!(require_incoming_scalar_edge((1, u128::MAX, 1, 1), (2, 0, 2, 2), 1).is_err());
        assert!(require_incoming_scalar_edge((1, 1, u128::MAX, 1), (2, 2, 0, 2), 1).is_err());
        assert!(require_incoming_scalar_edge((1, 1, 1, u64::MAX), (2, 2, 2, 0), 1).is_err());
    }

    #[test]
    fn incoming_ack_requires_nondecreasing_both_actual_clock_bounds() {
        let original = clock(1, 100, 110);
        assert!(require_clock_order(&original, &clock(3, 100, 110)).is_ok());
        assert!(require_clock_order(&original, &clock(3, 110, 120)).is_ok());
        assert!(require_clock_order(&original, &clock(3, 99, 120)).is_err());
        assert!(require_clock_order(&original, &clock(3, 101, 109)).is_err());
        assert!(require_clock_order(&original, &clock(3, 120, 110)).is_err());
        let missing = KagemushaOrdinaryCashClockContextV1 {
            request_nonce: [0; 32],
            ..original
        };
        assert!(require_clock_order(&missing, &clock(3, 110, 120)).is_err());
    }

    #[test]
    fn incoming_suffix_accounts_whole_frames_and_rejects_overflow() {
        assert_eq!(
            checked_frame_charge([1, 2, 3], 16 * 1024).unwrap(),
            6 + 3 * 256
        );
        assert!(checked_frame_charge([0, 2, 3], 16 * 1024).is_err());
        assert!(checked_frame_charge([16 * 1024u64 + 1, 2, 3], 16 * 1024).is_err());
        let maximum = 16 * 1024u64;
        assert_eq!(
            checked_frame_charge([maximum; 3], 16 * 1024).unwrap(),
            3 * (maximum + 256)
        );
    }

    #[test]
    fn incoming_advance_original_retains_full_receipt_source_and_recorded_quota() {
        let original = IncomingStateAdvanceOriginals {
            prepared_original_sha256: [1; 32],
            commit_request_original_sha256: [2; 32],
            commit_original: vec![3; 97],
            source: SourceLocator::Mint,
            capacity_charge_bytes: 123_456_789,
        };
        let row = Record::IncomingStateAdvance(original.clone());
        let raw = norito::encode_canonical(&row).unwrap();
        let decoded: Record =
            norito::decode_canonical_with_limits(&raw, norito::canonical_decode_limits(raw.len()))
                .unwrap();
        let Record::IncomingStateAdvance(decoded) = decoded else {
            panic!("wrong retained row role")
        };
        assert_eq!(decoded, original);
        assert!(frame_bytes(&row, 16 * 1024).unwrap() > original.commit_original.len() as u64);
        let mut altered = original.clone();
        altered.commit_original[1] ^= 1;
        assert_ne!(
            raw,
            norito::encode_canonical(&Record::IncomingStateAdvance(altered)).unwrap()
        );
        let mut altered = original.clone();
        altered.source = SourceLocator::Receive {
            request_id: [4; 32],
        };
        assert_ne!(
            raw,
            norito::encode_canonical(&Record::IncomingStateAdvance(altered)).unwrap()
        );
        let mut altered = original;
        altered.capacity_charge_bytes -= 1;
        assert_ne!(
            raw,
            norito::encode_canonical(&Record::IncomingStateAdvance(altered)).unwrap()
        );
    }

    #[test]
    fn incoming_postfsync_ack_preserves_clock_original_identity_not_only_same_interval() {
        let ack = IncomingStateAdvanceAcknowledgment {
            financial_control: CapturedFinancialControlIdentity {
                original_sha256: [1; 32],
                lower_ms: 100,
                upper_ms: 105,
            },
            clock: clock(2, 110, 115),
        };
        let raw = norito::encode_canonical(&Record::IncomingStateAdvanceAcknowledged {
            commit_request_original_sha256: [5; 32],
            acknowledgement: ack,
        })
        .unwrap();
        let altered = IncomingStateAdvanceAcknowledgment {
            clock: clock(4, 110, 115),
            ..ack
        };
        assert_ne!(
            raw,
            norito::encode_canonical(&Record::IncomingStateAdvanceAcknowledged {
                commit_request_original_sha256: [5; 32],
                acknowledgement: altered
            })
            .unwrap()
        );
        let decoded: Record =
            norito::decode_canonical_with_limits(&raw, norito::canonical_decode_limits(raw.len()))
                .unwrap();
        let Record::IncomingStateAdvanceAcknowledged {
            commit_request_original_sha256,
            acknowledgement,
        } = decoded
        else {
            panic!("wrong retained acknowledgement role")
        };
        assert_eq!(commit_request_original_sha256, [5; 32]);
        assert_eq!(acknowledgement, ack);
    }
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    pub(super) fn retained_incoming_commit_digest(
        &self,
    ) -> Result<Option<DigestV1>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        self.prepared_incoming_commit
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
    pub(super) fn retained_incoming_commit_transport_originals(
        &mut self,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        let request = self.reserve_retained_incoming_commit()?;
        let prepared = self
            .prepared_incoming_commit
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        prepared
            .originals
            .require_actual_generated(self, &prepared.generated)?;
        Ok(vec![
            request,
            prepared.generated.private_service_original().to_vec(),
        ])
    }
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    /// Signed exact Commit request plus full retained 49 proof; acknowledged originals are
    /// recovered without another account invocation or a new request nonce.
    /// # Errors
    /// Rejects uncertainty, absent generated custody, stale FI or ambiguous acknowledgements.
    pub fn sign_incoming_mint_commit_transport(
        &mut self,
        sign: impl FnOnce(
            &KagemushaAuthenticatedOrdinaryLineageAccountSigningV1<'_>,
        ) -> Result<[u8; 64], KagemushaStateErrorV1>,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.reserve_retained_incoming_commit()?;
        let prepared = self
            .prepared_incoming_commit
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
            .acknowledged_incoming_commit_request(financial, &current, prepared.generated.proof())
            .map_err(material)?;
        let (status, request, signature) = if let Some((request, signature)) = acknowledged {
            (2, request, signature.to_vec())
        } else {
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
