//! Separate purpose1 custody selected only after genuine purpose2 Guard and State admission.
//!
//! Complete retained proofs are checked again on recovery. The single cash journal and original
//! publication remain locked by one Native cash owner. A signed original is historical only after
//! its complete bytes have been fsynced and a new actual Native interval still lies inside W1.

use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaAuthenticatedOrdinaryCashCandidateV1,
    KagemushaAuthenticatedOrdinaryPreparationGuardV1, verify_ordinary_cash_candidate_v1,
    verify_ordinary_preparation_guard_v1,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1, KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1,
    KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1, KagemushaEncryptedCreditEnvelopeV1,
    KagemushaOrdinaryCashClockContextV1, KagemushaOrdinaryCashTerminalBodyV1,
    KagemushaOrdinaryCashTerminalIntentV1, KagemushaOrdinaryCashTerminalRecordV1,
    KagemushaOrdinaryPaymentOutputV1, KagemushaOrdinaryPaymentRequestV1,
    KagemushaOrdinaryPreparedOutgoingV1, kagemusha_asset_identity_digest_v1,
    kagemusha_ciphertext_digest_v1, kagemusha_ordinary_payment_body_digest_v1,
    kagemusha_ordinary_sealed_recovery_seeds_digest_v1,
    kagemusha_ordinary_sealed_transition_inputs_digest_v1,
    kagemusha_ordinary_terminal_guard_commit_binding_digest_v1,
    kagemusha_ordinary_transition_nullifier_v1,
};

/// Original transport inputs; these private data carriers grant no financial or receiver custody.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryCashTransportOriginalsV1")]
pub(super) enum TransportOriginals {
    Send {
        request: KagemushaOrdinaryPaymentRequestV1,
        output: KagemushaOrdinaryPaymentOutputV1,
        encrypted_credit: Vec<u8>,
        receiver_counter_floor: Option<u32>,
        receiver_lease_original: Option<Vec<u8>>,
    },
    Redeem {
        output: iroha_data_model::kagemusha::KagemushaOrdinaryRedemptionOutputV1,
        beneficiary: iroha_data_model::account::AccountId,
        manifest_original: Vec<u8>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::OrdinaryCashTerminalSelectionOriginalsV1"
)]
struct SelectionOriginals {
    preselection_cash_prefix: KagemushaRecoveryJournalPrefixV1,
    financial_control: CapturedFinancialControlIdentity,
    prepared: KagemushaOrdinaryPreparedOutgoingV1,
    state_proof: KagemushaPairedProofV1,
    preparation_guard_original: Vec<u8>,
    intent: KagemushaOrdinaryCashTerminalIntentV1,
    body: KagemushaOrdinaryCashTerminalBodyV1,
    normalized: KagemushaNormalizedGuardStatementV1,
    challenge: KagemushaAppOperationApprovalChallengeV1,
    counter_floor: Option<u32>,
    lease_original: Option<Vec<u8>>,
    transport: TransportOriginals,
    transition_stream: Vec<u8>,
    recovery_stream: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryCashTerminalRecordV1")]
pub(super) enum TerminalRecord {
    Select(SelectionOriginals),
    PlatformFence {
        operation: DigestV1,
    },
    ApprovalOriginal {
        operation: DigestV1,
        clock: KagemushaOrdinaryCashClockContextV1,
        original: Vec<u8>,
        authorization_digest: DigestV1,
        accepted_counter: Option<u32>,
    },
    Capture(KagemushaOrdinaryCashTerminalRecordV1),
    Cancel {
        operation: DigestV1,
    },
}

impl TerminalRecord {
    pub(super) fn preselection_prefix(&self) -> Option<KagemushaRecoveryJournalPrefixV1> {
        match self {
            Self::Select(originals) => Some(originals.preselection_cash_prefix),
            _ => None,
        }
    }
    pub(super) fn new_operation(&self) -> Option<DigestV1> {
        match self {
            Self::Select(originals) => Some(originals.intent.native_operation_id),
            _ => None,
        }
    }
    pub(super) fn accepted_counter(&self) -> Option<u32> {
        match self {
            Self::ApprovalOriginal {
                accepted_counter, ..
            } => *accepted_counter,
            _ => None,
        }
    }
}

struct TerminalPending {
    originals: SelectionOriginals,
    candidate: KagemushaAuthenticatedOrdinaryCashCandidateV1,
    guard: KagemushaAuthenticatedOrdinaryPreparationGuardV1,
    lease: Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
    receiver: Option<Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>>,
    receiver_lease: Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
    fenced: bool,
    retained: Option<(
        KagemushaOrdinaryCashClockContextV1,
        KagemushaVerifiedAppOperationApprovalV1,
    )>,
    captured: Option<(
        KagemushaOrdinaryCashTerminalRecordV1,
        KagemushaVerifiedAppOperationApprovalV1,
    )>,
}

pub(super) struct TerminalJournal {
    pending: Option<TerminalPending>,
    used_operations: BTreeSet<DigestV1>,
}

/// Exact captured purpose1 loan. Its private constructor borrows the actual retained owner;
/// purpose2 evidence and a decoded terminal record can never construct this selection.
pub(crate) struct KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'a> {
    owner: &'a KagemushaNativeOrdinaryCashOwnerV1,
    cash_prefix: KagemushaRecoveryJournalPrefixV1,
}

impl TerminalJournal {
    pub(super) fn new() -> Self {
        Self {
            pending: None,
            used_operations: BTreeSet::new(),
        }
    }
    /// Consume only the captured terminal admitted by the exact genuine Commit capability.
    /// The caller already verified the real global CAS acknowledgment before StateAdvance.
    pub(super) fn consume_actual_commit(
        &mut self,
        generated: &crate::kagemusha_v1_recursion::GeneratedOrdinaryCashCommitOriginalsV1,
        receipt: &KagemushaAuthenticatedOrdinaryLineageCommitReceiptV1<'_>,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck()?;
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let (record, approval) = pending
            .captured
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let commit = generated.commit();
        receipt.recheck_historical(financial).map_err(material)?;
        if receipt.commit().map_err(material)? != commit {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        if !pending.fenced
            || commit.reservation.successor.state_commitment
                != pending.candidate.successor_state().state_commitment
            || commit.reservation.successor.logical_sequence
                != pending.candidate.successor_state().logical_sequence
            || generated.selected_successor_state() != pending.candidate.successor_state()
            || commit.purpose1_approval_original_sha256
                != <DigestV1>::from(Sha256::digest(approval.original()))
            || commit.terminal_record_original_sha256
                != <DigestV1>::from(Sha256::digest(
                    norito::encode_canonical(record).map_err(material)?,
                ))
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        // Used operation identities and monotonic logical floors are deliberately retained.
        self.pending = None;
        self.recheck()
    }
    pub(super) fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub(super) fn recheck(&self) -> Result<(), KagemushaStateErrorV1> {
        if self.used_operations.len() > MAX_ROWS as usize {
            return Err(KagemushaStateErrorV1::JournalRevisionOverflow);
        }
        if let Some(pending) = &self.pending {
            if !self
                .used_operations
                .contains(&pending.originals.intent.native_operation_id)
                || ((pending.retained.is_some() || pending.captured.is_some()) && !pending.fenced)
                || (pending.retained.is_some() && pending.captured.is_some())
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        }
        Ok(())
    }
    pub(super) fn replay(
        &mut self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        record: TerminalRecord,
        leases: &[Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>],
        receivers: &[Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>],
    ) -> Result<(), KagemushaStateErrorV1> {
        match record {
            TerminalRecord::Select(originals) if self.pending.is_none() => {
                let operation = originals.intent.native_operation_id;
                if self.used_operations.contains(&operation)
                    || owner.used_operations.contains(&operation)
                    || originals.counter_floor != owner.counter_floor
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                let preparation = owner.captured_preparation()?;
                let guard = verify_ordinary_preparation_guard_v1(
                    &preparation,
                    &originals.preparation_guard_original,
                )?;
                let candidate = verify_ordinary_cash_candidate_v1(
                    &preparation,
                    &guard,
                    originals.prepared,
                    originals.state_proof.clone(),
                )?;
                let (receiver, receiver_lease) = match &originals.transport {
                    TransportOriginals::Redeem { .. } => (None, None),
                    TransportOriginals::Send {
                        request,
                        receiver_lease_original,
                        ..
                    } => (
                        Some(
                            receivers
                                .iter()
                                .find(|r| {
                                    r.app_credential().digest()
                                        == request.body.recipient_credential_digest
                                })
                                .cloned()
                                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
                        ),
                        selected_lease(receiver_lease_original, leases)?,
                    ),
                };
                require_selection(owner, &originals, &candidate, &guard)?;
                require_receiver(
                    owner,
                    &originals,
                    receiver.as_deref(),
                    receiver_lease.as_deref(),
                )?;
                let lease = selected_lease(&originals.lease_original, leases)?;
                self.used_operations.insert(operation);
                self.pending = Some(TerminalPending {
                    originals,
                    candidate,
                    guard,
                    lease,
                    receiver,
                    receiver_lease,
                    fenced: false,
                    retained: None,
                    captured: None,
                });
            }
            TerminalRecord::PlatformFence { operation } => {
                let pending = self
                    .pending
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if pending.originals.intent.native_operation_id != operation || pending.fenced {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                pending.fenced = true;
            }
            TerminalRecord::ApprovalOriginal {
                operation,
                clock,
                original,
                authorization_digest,
                accepted_counter,
            } => {
                let pending = self
                    .pending
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                require_pending_operation(pending, operation)?;
                if !pending.fenced || pending.retained.is_some() || pending.captured.is_some() {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                require_admission_clock(&pending.originals, &clock)?;
                let approved = authenticate_terminal(owner, pending, &original, &clock)?;
                if authorization(&approved, pending.lease.as_deref())? != authorization_digest
                    || approved.app_attest_counter() != accepted_counter
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                self.pending
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .retained = Some((clock, approved));
            }
            TerminalRecord::Capture(record) => {
                let pending = self
                    .pending
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                let (original_clock, approved) = pending
                    .retained
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                let expected =
                    terminal_record(owner, pending, approved, record.admission_clock_context)?;
                if !pending.fenced
                    || pending.captured.is_some()
                    || record != expected
                    || record.admission_clock_context.lower_at_ms < original_clock.lower_at_ms
                    || record.admission_clock_context.upper_at_ms < original_clock.upper_at_ms
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                approved
                    .recheck_at_trusted_time(record.admission_clock_context.lower_at_ms)
                    .map_err(material)?;
                approved
                    .recheck_at_trusted_time(record.admission_clock_context.upper_at_ms)
                    .map_err(material)?;
                let pending = self
                    .pending
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                let (_, approved) = pending
                    .retained
                    .take()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                pending.captured = Some((record, approved));
            }
            TerminalRecord::Cancel { operation } => {
                let pending = self
                    .pending
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                require_pending_operation(pending, operation)?;
                if pending.captured.is_some() {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                self.pending = None;
            }
            _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
        }
        Ok(())
    }
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    /// Reserve a distinct actual purpose1 nonce only after real preparation Guard and State proofs.
    /// The transport inputs remain private Native originals; managed code supplies no owner/time/key.
    pub(crate) fn select_terminal(
        &mut self,
        candidate: KagemushaAuthenticatedOrdinaryCashCandidateV1,
        guard: KagemushaAuthenticatedOrdinaryPreparationGuardV1,
        transport: TransportOriginals,
        receiver: Option<Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>>,
        receiver_lease: Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
        transition_stream: Vec<u8>,
        recovery_stream: Vec<u8>,
    ) -> Result<KagemushaAppOperationApprovalChallengeV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let terminal = self
            .terminal
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if terminal.pending.is_some() || self.pending_mint.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        // W1 selects a separately acknowledged actual FI original after slow W2 proving.
        // Reusing W2 or replacing this retained original during later proving is refused.
        let captured = self
            .control
            .capture_proof_decision(self.publication.cash_financial())
            .map_err(material)?;
        let financial_control = CapturedFinancialControlIdentity {
            original_sha256: captured.original_sha256().map_err(material)?,
            lower_ms: captured.captured_lower_ms(),
            upper_ms: captured.captured_upper_ms(),
        };
        let (control_issued_at_ms, control_expires_at_ms) = self
            .control
            .recheck_retained_capture_original_window(
                self.publication.cash_financial(),
                financial_control.original_sha256,
                financial_control.lower_ms,
                financial_control.upper_ms,
            )
            .map_err(material)?;
        let preparation = self.captured_preparation()?;
        candidate.recheck_preparation_selection(&preparation, &guard)?;
        let prepared = *candidate.prepared_record();
        require_streams(&prepared, &transition_stream, &recovery_stream)?;
        let clock = self
            .publication
            .cash_financial()
            .current_cash_clock_context()
            .map_err(material)?;
        if clock.lower_at_ms < financial_control.lower_ms
            || clock.upper_at_ms < financial_control.upper_ms
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        clock
            .validate_within_original_window(control_issued_at_ms, control_expires_at_ms)
            .map_err(material)?;
        let issued_at_ms = clock.lower_at_ms;
        let expires_at_ms = issued_at_ms
            .checked_add(KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1)
            .ok_or(KagemushaStateErrorV1::InvalidTrustedCommitTime)?
            .min(self.credential_floor()?.approval_valid_until_ms())
            .min(control_expires_at_ms);
        clock
            .validate_within_original_window(issued_at_ms, expires_at_ms)
            .map_err(material)?;
        let mut entropy = [0; 64];
        OsRng.try_fill_bytes(&mut entropy).map_err(material)?;
        let mut hash = Sha256::new();
        hash.update(b"iroha:kagemusha:v1:ordinary-cash-terminal-native-operation\0");
        hash.update(self.prefix.head);
        hash.update(self.prefix.sequence.to_le_bytes());
        hash.update(candidate.candidate_envelope_digest());
        hash.update(&entropy[..32]);
        let operation: DigestV1 = hash.finalize().into();
        let nonce: DigestV1 = entropy[32..].try_into().map_err(material)?;
        if operation == [0; 32]
            || nonce == [0; 32]
            || operation == preparation.challenge().operation_id
            || nonce == preparation.challenge().nonce
            || terminal.used_operations.contains(&operation)
            || self.used_operations.contains(&operation)
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let intent = KagemushaOrdinaryCashTerminalIntentV1 {
            version: 1,
            operation: prepared.operation,
            native_operation_id: operation,
            native_nonce: nonce,
            preparation_id: candidate.preparation_id()?,
            candidate_digest: candidate.candidate_envelope_digest(),
            state_statement_digest: candidate.full_state_sha256(),
            predecessor_descriptor_prefix_digest: financial_prefix_digest(self, self.prefix)?,
            sender_credential_digest: preparation.enrollment().app_credential().digest(),
            reservation_digest: prepared.reservation_digest,
            secure_index_before: self.state.secure_index,
            secure_index_after: candidate.successor_state().secure_index,
            logical_journal_sequence_before: self.financial_journal_revision,
            logical_journal_sequence_after: self
                .financial_journal_revision
                .checked_add(1)
                .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?,
            issued_at_ms,
            expires_at_ms,
        };
        let (request_digest, receiver_digest, output_digest, encrypted_digest) =
            transport_body_components(&transport)?;
        let body = KagemushaOrdinaryCashTerminalBodyV1 {
            version: 1,
            operation: prepared.operation,
            amount: preparation.transition_statement().amount,
            state_statement_digest: candidate.full_state_sha256(),
            candidate_digest: candidate.candidate_envelope_digest(),
            preparation_id: candidate.preparation_id()?,
            prepared_projection_semantic_digest: prepared.projection_semantic_digest,
            lifecycle_digest: prepared.lifecycle_binding_digest,
            request_digest,
            recipient_credential_digest: receiver_digest,
            send_output_digest: output_digest,
            encrypted_credit_digest: encrypted_digest,
            artifact_manifest_digest: prepared.artifact_manifest_digest,
            reservation_digest: prepared.reservation_digest,
            native_operation_id: operation,
            terminal_intent_digest: intent.binding_digest().map_err(material)?,
            predecessor_descriptor_prefix_digest: intent.predecessor_descriptor_prefix_digest,
            stream_lengths: prepared.stream_lengths,
            stream_digests: prepared.stream_digests,
            clock_context: clock,
            secure_index_before: intent.secure_index_before,
            secure_index_after: intent.secure_index_after,
            logical_journal_sequence_before: intent.logical_journal_sequence_before,
            logical_journal_sequence_after: intent.logical_journal_sequence_after,
        };
        body.validate_against_intent(&intent).map_err(material)?;
        body.validate_against_prepared(&prepared)
            .map_err(material)?;
        let mut context = preparation.selected().context;
        context.terminal_commit_binding_digest =
            kagemusha_ordinary_terminal_guard_commit_binding_digest_v1(
                body.binding_digest().map_err(material)?,
                intent.candidate_digest,
                intent.state_statement_digest,
                intent.reservation_digest,
            )
            .map_err(material)?;
        // This position commits the genuine earlier W2 for Send. It does not claim an OEM
        // hardware one-use authorization. W1/PI is separately authenticated in Guard column3.
        context.sender_one_time_authorization_digest = if prepared.operation == 2 {
            prepared.preparation_authorization_digest
        } else {
            [0; 32]
        };
        context.transition_intent_digest = body.binding_digest().map_err(material)?;
        context.recovery_record_digest = intent.binding_digest().map_err(material)?;
        let normalized = KagemushaNormalizedGuardStatementV1::derive_from_transition(
            preparation.transition_statement(),
            context,
        )
        .map_err(material)?;
        let mut subject = preparation.challenge().subject;
        subject.candidate_envelope_digest = intent.candidate_digest;
        subject.terminal_body_commitment = body.binding_digest().map_err(material)?;
        let c = preparation.enrollment().app_credential();
        let challenge = KagemushaAppOperationApprovalChallengeV1 {
            version: 1,
            purpose: KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
            operation_id: operation,
            nonce,
            account_binding: c.subject().account_binding,
            authority_policy_digest: c.subject().app_authority_policy_digest,
            attested_key_id: c.subject().attested_key_id,
            enrollment_digest: c.digest(),
            subject_signing_digest: Sha256::digest(
                subject.canonical_signing_bytes().map_err(material)?,
            )
            .into(),
            normalized_guard_digest: normalized.canonical_digest().map_err(material)?,
            issued_at_ms,
            expires_at_ms,
            subject,
        };
        challenge.canonical_signing_bytes().map_err(material)?;
        let lease = self
            .publication
            .cash_financial()
            .retained_integrity_lease()
            .cloned();
        let originals = SelectionOriginals {
            preselection_cash_prefix: self.prefix,
            financial_control,
            prepared,
            state_proof: candidate.proof().clone(),
            preparation_guard_original: guard.original().to_vec(),
            intent,
            body,
            normalized,
            challenge,
            counter_floor: self.counter_floor,
            lease_original: lease.as_ref().map(|l| l.original().to_vec()),
            transport,
            transition_stream,
            recovery_stream,
        };
        require_selection(self, &originals, &candidate, &guard)?;
        require_receiver(
            self,
            &originals,
            receiver.as_deref(),
            receiver_lease.as_deref(),
        )?;
        self.persist(&Record::Terminal(TerminalRecord::Select(originals.clone())))?;
        self.used_operations.insert(operation);
        let terminal = self
            .terminal
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        terminal.used_operations.insert(operation);
        terminal.pending = Some(TerminalPending {
            originals,
            candidate,
            guard,
            lease,
            receiver,
            receiver_lease,
            fenced: false,
            retained: None,
            captured: None,
        });
        self.require_live_terminal(operation)?;
        Ok(challenge)
    }

    pub(crate) fn fence_terminal_platform(
        &mut self,
        operation: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_terminal(operation)?;
        let terminal = self
            .terminal
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if terminal
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .fenced
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.persist(&Record::Terminal(TerminalRecord::PlatformFence {
            operation,
        }))?;
        self.terminal
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .pending
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .fenced = true;
        self.require_live_terminal(operation)
    }
    pub(crate) fn capture_terminal_original(
        &mut self,
        operation: DigestV1,
        original: &[u8],
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_terminal(operation)?;
        let pending = self.terminal_pending()?;
        if !pending.fenced || pending.retained.is_some() || pending.captured.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let clock = self
            .publication
            .cash_financial()
            .current_cash_clock_context()
            .map_err(material)?;
        let approved = authenticate_terminal(self, pending, original, &clock)?;
        let digest = authorization(&approved, pending.lease.as_deref())?;
        self.persist(&Record::Terminal(TerminalRecord::ApprovalOriginal {
            operation,
            clock,
            original: original.to_vec(),
            authorization_digest: digest,
            accepted_counter: approved.app_attest_counter(),
        }))?;
        self.counter_floor = approved.app_attest_counter().or(self.counter_floor);
        self.terminal
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .pending
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .retained = Some((clock, approved));
        self.acknowledge_terminal_capture(operation)
    }
    pub(crate) fn acknowledge_terminal_capture(
        &mut self,
        operation: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_terminal(operation)?;
        let pending = self.terminal_pending()?;
        if pending.captured.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let (previous_clock, approved) = pending
            .retained
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let clock = self
            .publication
            .cash_financial()
            .current_cash_clock_context()
            .map_err(material)?;
        if clock.lower_at_ms < previous_clock.lower_at_ms
            || clock.upper_at_ms < previous_clock.upper_at_ms
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        approved
            .recheck_at_trusted_time(clock.lower_at_ms)
            .map_err(material)?;
        approved
            .recheck_at_trusted_time(clock.upper_at_ms)
            .map_err(material)?;
        let record = terminal_record(self, pending, approved, clock)?;
        self.persist(&Record::Terminal(TerminalRecord::Capture(record)))?;
        let pending = self
            .terminal
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .pending
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let (_, approved) = pending
            .retained
            .take()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        pending.captured = Some((record, approved));
        self.captured_terminal()?
            .recheck_selected_originals_and_current_custody()
    }
    pub(crate) fn captured_terminal(
        &self,
    ) -> Result<
        KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
        KagemushaStateErrorV1,
    > {
        let loan = KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1 {
            owner: self,
            cash_prefix: self.prefix,
        };
        loan.recheck_selected_originals_and_current_custody()?;
        Ok(loan)
    }
    fn terminal_pending(&self) -> Result<&TerminalPending, KagemushaStateErrorV1> {
        self.terminal
            .as_ref()
            .and_then(|t| t.pending.as_ref())
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }
    fn require_live_terminal(&self, operation: DigestV1) -> Result<(), KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let pending = self.terminal_pending()?;
        require_pending_operation(pending, operation)?;
        require_selection(self, &pending.originals, &pending.candidate, &pending.guard)?;
        require_receiver(
            self,
            &pending.originals,
            pending.receiver.as_deref(),
            pending.receiver_lease.as_deref(),
        )?;
        self.publication
            .cash_financial()
            .trusted_time_interval()
            .map_err(material)?
            .require_validity(
                pending.originals.challenge.issued_at_ms,
                pending.originals.challenge.expires_at_ms,
            )
            .map_err(material)
    }
}

impl KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_> {
    pub(crate) fn recheck_selected_originals_and_current_custody(
        &self,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.owner.recheck_proving_history()?;
        if self.cash_prefix != self.owner.prefix {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let pending = self.owner.terminal_pending()?;
        let (record, approved) = pending
            .captured
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if !pending.fenced
            || record
                != &terminal_record(
                    self.owner,
                    pending,
                    approved,
                    record.admission_clock_context,
                )?
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        require_selection(
            self.owner,
            &pending.originals,
            &pending.candidate,
            &pending.guard,
        )?;
        require_receiver(
            self.owner,
            &pending.originals,
            pending.receiver.as_deref(),
            pending.receiver_lease.as_deref(),
        )?;
        approved
            .recheck_at_trusted_time(record.admission_clock_context.lower_at_ms)
            .map_err(material)?;
        approved
            .recheck_at_trusted_time(record.admission_clock_context.upper_at_ms)
            .map_err(material)?;
        Ok(())
    }
    fn pending(&self) -> &TerminalPending {
        self.owner
            .terminal_pending()
            .expect("closed retained terminal selection")
    }
    fn captured(
        &self,
    ) -> &(
        KagemushaOrdinaryCashTerminalRecordV1,
        KagemushaVerifiedAppOperationApprovalV1,
    ) {
        self.pending()
            .captured
            .as_ref()
            .expect("closed retained terminal capture")
    }
    pub(crate) fn enrollment(&self) -> &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1 {
        self.owner.publication.cash_financial().enrollment()
    }
    pub(crate) fn authenticated_release(
        &self,
    ) -> Result<Arc<KagemushaAuthenticatedReleaseV1>, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(Arc::clone(
            self.owner.publication.cash_approvals().retained_release(),
        ))
    }
    pub(crate) fn recursive_verifier(&self) -> &KagemushaAuthenticatedRecursiveVerifierV1 {
        &self.owner.verifier
    }
    pub(crate) fn selected_predecessor_state(&self) -> &KagemushaStateV1 {
        &self.owner.state
    }
    /// Exact current PUBLIC paired State original retained by the genuine Main journal.
    pub(crate) fn selected_predecessor_public_state_original(
        &self,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let original = self.owner.public_state_original.clone();
        if original.is_empty() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.recheck_selected_originals_and_current_custody()?;
        Ok(original)
    }
    /// Require the supplied closed receipt to be the exact acknowledgment in Main's CAS WAL.
    /// Genuine independent Reserve proof admission remains mandatory for its full bundle joins.
    pub(crate) fn recheck_lineage_reservation(
        &self,
        receipt: &KagemushaAuthenticatedOrdinaryLineageReservationReceiptV1<'_>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let financial = self.owner.publication.cash_financial();
        receipt.recheck_historical(financial).map_err(material)?;
        let reservation = receipt.reservation().map_err(material)?;
        let actual = self
            .owner
            .lineage_cas
            .reservation_receipt(receipt.request_original_sha256(), financial, reservation)
            .map_err(material)?;
        actual.recheck_historical(financial).map_err(material)?;
        let preparation = self.preparation_selection()?;
        let head = &reservation.selection.predecessor;
        if actual.original().map_err(material)? != receipt.original().map_err(material)?
            || reservation.selection.operation_id != preparation.challenge().operation_id
            || head.state_commitment != self.selected_predecessor_state().state_commitment
            || head.logical_sequence != self.selected_predecessor_state().logical_sequence
            || head.state_original_sha256
                != <DigestV1>::from(Sha256::digest(&self.owner.public_state_original))
            || reservation.selection.amount != self.transition_statement().amount
            || reservation.selection.neutral_reservation_digest
                != self.pending().originals.prepared.reservation_digest
            || reservation.successor.state_commitment
                != self.selected_successor_state().state_commitment
            || reservation.successor.logical_sequence
                != self.selected_successor_state().logical_sequence
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        actual.recheck_historical(financial).map_err(material)?;
        self.recheck_selected_originals_and_current_custody()
    }
    pub(crate) fn selected_successor_state(&self) -> &KagemushaStateV1 {
        self.pending().candidate.successor_state()
    }
    pub(crate) fn transition_statement(&self) -> &TransitionProofStatementV1 {
        &self
            .owner
            .pending
            .as_ref()
            .expect("held preparation")
            .selected
            .as_ref()
            .expect("held preparation selection")
            .statement
    }
    pub(crate) fn candidate(&self) -> &KagemushaAuthenticatedOrdinaryCashCandidateV1 {
        &self.pending().candidate
    }
    pub(crate) fn preparation_guard(&self) -> &KagemushaAuthenticatedOrdinaryPreparationGuardV1 {
        &self.pending().guard
    }
    pub(crate) fn preparation_selection(
        &self,
    ) -> Result<KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>, KagemushaStateErrorV1>
    {
        self.owner.captured_preparation()
    }
    pub(crate) fn terminal_intent(&self) -> &KagemushaOrdinaryCashTerminalIntentV1 {
        &self.pending().originals.intent
    }
    pub(crate) fn terminal_body(&self) -> &KagemushaOrdinaryCashTerminalBodyV1 {
        &self.pending().originals.body
    }
    pub(crate) fn terminal_record(&self) -> &KagemushaOrdinaryCashTerminalRecordV1 {
        &self.captured().0
    }
    pub(crate) fn admission_clock_context(&self) -> &KagemushaOrdinaryCashClockContextV1 {
        &self.captured().0.admission_clock_context
    }
    pub(crate) fn normalized_guard_statement(&self) -> &KagemushaNormalizedGuardStatementV1 {
        &self.pending().originals.normalized
    }
    pub(crate) fn challenge(&self) -> &KagemushaAppOperationApprovalChallengeV1 {
        &self.pending().originals.challenge
    }
    pub(crate) fn original(&self) -> &[u8] {
        self.captured().1.original()
    }
    pub(crate) fn original_approval_integrity_lease(
        &self,
    ) -> Option<&Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>> {
        self.pending().lease.as_ref()
    }
    pub(crate) fn previous_app_attest_counter(&self) -> Option<u32> {
        self.pending().originals.counter_floor
    }
    pub(crate) fn authorization_binding_digest(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        authorization(&self.captured().1, self.pending().lease.as_deref())
    }
    pub(crate) fn approval_admission_time_ms(&self) -> u64 {
        self.admission_interval_lower_ms()
    }
    pub(crate) fn admission_interval_lower_ms(&self) -> u64 {
        self.admission_clock_context().lower_at_ms
    }
    pub(crate) fn admission_interval_upper_ms(&self) -> u64 {
        self.admission_clock_context().upper_at_ms
    }
    pub(crate) fn transition_stream(&self) -> &[u8] {
        &self.pending().originals.transition_stream
    }
    /// Borrow the same private opening that produced the fsynced ciphertext before W2.
    /// This closed captured selection lends no raw secret constructor to managed callers.
    pub(crate) fn send_credit_opening(
        &self,
    ) -> Option<&iroha_data_model::kagemusha::KagemushaCreditOpeningV1> {
        if !matches!(
            &self.pending().originals.transport,
            TransportOriginals::Send { .. }
        ) {
            return None;
        }
        Some(
            self.owner
                .pending
                .as_ref()?
                .send_credit
                .as_ref()?
                .originals
                .credit_opening(),
        )
    }
    pub(crate) fn preparation_clock_context(&self) -> &KagemushaOrdinaryCashClockContextV1 {
        &self
            .owner
            .pending
            .as_ref()
            .expect("held exact preparation")
            .preparation_clock
    }
    /// Return the four authentic signed samples for each retained preparation/W1/capture cut.
    /// These complete historical originals lend no new clock or financial authority.
    pub(crate) fn retained_signed_clock_originals(
        &self,
    ) -> Result<[Vec<u8>; 3], KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let financial = self.owner.publication.cash_financial();
        let contexts = [
            self.preparation_clock_context(),
            &self.terminal_body().clock_context,
            self.admission_clock_context(),
        ];
        let mut originals = [Vec::new(), Vec::new(), Vec::new()];
        for (original, context) in originals.iter_mut().zip(contexts) {
            let loan = financial
                .retained_cash_clock_originals(context)
                .map_err(material)?;
            financial
                .recheck_retained_cash_clock_originals(&loan)
                .map_err(material)?;
            *original = loan.canonical_original().to_vec();
            financial
                .recheck_retained_cash_clock_originals(&loan)
                .map_err(material)?;
        }
        self.recheck_selected_originals_and_current_custody()?;
        Ok(originals)
    }
    /// Lend three independently verified historical sample originals from actual Native WAL.
    /// The private callback receives no financial or current elapsed-clock authority.
    pub(crate) fn with_retained_verified_signed_clock_originals(
        &self,
        visitor: &mut dyn for<'clock> FnMut(
            [&'clock KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1; 3],
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let financial = self.owner.publication.cash_financial();
        let preparation = financial
            .verified_retained_cash_clock_originals(self.preparation_clock_context())
            .map_err(material)?;
        let intent = financial
            .verified_retained_cash_clock_originals(&self.terminal_body().clock_context)
            .map_err(material)?;
        let admission = financial
            .verified_retained_cash_clock_originals(self.admission_clock_context())
            .map_err(material)?;
        self.recheck_selected_originals_and_current_custody()?;
        visitor([&preparation, &intent, &admission])?;
        self.recheck_selected_originals_and_current_custody()
    }
    /// Copy the exact acknowledged W1 FI original from this same Native owner.
    /// Historical proof custody does not renew its original window or permit a live effect.
    pub(crate) fn financial_control_original(&self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let identity = self.pending().originals.financial_control;
        let loan = self
            .owner
            .control
            .borrow_captured_proof_decision(
                self.owner.publication.cash_financial(),
                identity.original_sha256,
                identity.lower_ms,
                identity.upper_ms,
            )
            .map_err(material)?;
        let original = loan.original().map_err(material)?.to_vec();
        loan.recheck_historical_originals().map_err(material)?;
        self.recheck_selected_originals_and_current_custody()?;
        Ok(original)
    }
    pub(crate) fn outbox_reservation_original(
        &self,
    ) -> &iroha_data_model::kagemusha::KagemushaOutboxReservationV1 {
        &self
            .owner
            .pending
            .as_ref()
            .expect("held exact preparation")
            .reservation
    }
    pub(crate) fn recovery_stream(&self) -> &[u8] {
        &self.pending().originals.recovery_stream
    }
    /// Borrow the exact retained Send transport and independently authenticated receiver C.
    /// The caller rechecks this closed selection before and after proof admission.
    pub(crate) fn send_transport_originals(
        &self,
    ) -> Option<(
        &KagemushaOrdinaryPaymentRequestV1,
        &KagemushaOrdinaryPaymentOutputV1,
        &[u8],
        &KagemushaVerifiedOrdinaryAppCredentialV1,
        Option<u32>,
    )> {
        let pending = self.pending();
        match &pending.originals.transport {
            TransportOriginals::Send {
                request,
                output,
                encrypted_credit,
                receiver_counter_floor,
                ..
            } => Some((
                request,
                output,
                encrypted_credit.as_slice(),
                pending.receiver.as_ref()?.app_credential(),
                *receiver_counter_floor,
            )),
            TransportOriginals::Redeem { .. } => None,
        }
    }
    /// Borrow the independently admitted receiver PI original from this same closed Send.
    /// The mathematical consumer still authenticates every original and issuer binding.
    pub(crate) fn send_receiver_integrity_lease_original(
        &self,
    ) -> Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1> {
        match &self.pending().originals.transport {
            TransportOriginals::Send { .. } => self.pending().receiver_lease.as_deref(),
            TransportOriginals::Redeem { .. } => None,
        }
    }
    /// Borrow full beneficiary/manifest originals from the same fsynced pre-W2 selection.
    pub(crate) fn redeem_transport_originals(
        &self,
    ) -> Option<(
        &iroha_data_model::kagemusha::KagemushaOrdinaryRedemptionOutputV1,
        &iroha_data_model::account::AccountId,
        &[u8],
    )> {
        match &self.pending().originals.transport {
            TransportOriginals::Redeem {
                output,
                beneficiary,
                manifest_original,
            } => Some((output, beneficiary, manifest_original)),
            TransportOriginals::Send { .. } => None,
        }
    }
    pub(crate) fn with_borrowed_financial_secret(
        &self,
        consume: &mut dyn for<'secret> FnMut(
            &'secret [u8; 32],
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let result = self
            .preparation_selection()?
            .with_borrowed_financial_secret(consume);
        self.recheck_selected_originals_and_current_custody()?;
        result
    }
}

fn financial_prefix_digest(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    selected_prefix: KagemushaRecoveryJournalPrefixV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    if !owner
        .journal
        .contains_recovery_prefix(selected_prefix)
        .map_err(storage)?
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:ordinary-financial-descriptor-prefix\0");
    hash.update(owner.state.state_commitment);
    hash.update(owner.financial_journal_revision.to_le_bytes());
    hash.update(norito::encode_canonical(&selected_prefix).map_err(material)?);
    for digest in owner.publication.historical_original_commitments()? {
        hash.update(digest);
    }
    Ok(hash.finalize().into())
}
fn require_streams(
    prepared: &KagemushaOrdinaryPreparedOutgoingV1,
    transition: &[u8],
    recovery: &[u8],
) -> Result<(), KagemushaStateErrorV1> {
    for (i, stream, maximum) in [
        (
            0,
            transition,
            KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1,
        ),
        (1, recovery, KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1),
    ] {
        let actual = if i == 0 {
            kagemusha_ordinary_sealed_transition_inputs_digest_v1(stream)
        } else {
            kagemusha_ordinary_sealed_recovery_seeds_digest_v1(stream)
        }
        .map_err(material)?;
        if stream.is_empty()
            || stream.len() as u64 > u64::from(maximum)
            || stream.len() as u64 != prepared.stream_lengths[i]
            || actual != prepared.stream_digests[i]
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
    }
    Ok(())
}
fn transport_body_components(
    transport: &TransportOriginals,
) -> Result<(DigestV1, DigestV1, DigestV1, DigestV1), KagemushaStateErrorV1> {
    match transport {
        TransportOriginals::Redeem { output, .. } => {
            output.validate_shape().map_err(material)?;
            Ok(([0; 32], [0; 32], [0; 32], [0; 32]))
        }
        TransportOriginals::Send {
            request,
            output,
            encrypted_credit,
            ..
        } => {
            request.validate_shape().map_err(material)?;
            output.validate_shape().map_err(material)?;
            KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
                encrypted_credit,
                request.body.recipient_encryption_key,
            )
            .map_err(material)?;
            let request_digest = request.canonical_original_digest().map_err(material)?;
            let encrypted_digest = kagemusha_ciphertext_digest_v1(encrypted_credit);
            if output.request_digest != request_digest
                || output.amount != request.body.amount
                || output.encrypted_credit_digest != encrypted_digest
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            Ok((
                request_digest,
                request.body.recipient_credential_digest,
                output.binding_digest().map_err(material)?,
                encrypted_digest,
            ))
        }
    }
}
fn require_selection(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    originals: &SelectionOriginals,
    candidate: &KagemushaAuthenticatedOrdinaryCashCandidateV1,
    guard: &KagemushaAuthenticatedOrdinaryPreparationGuardV1,
) -> Result<(), KagemushaStateErrorV1> {
    let preparation = owner.captured_preparation()?;
    candidate.recheck_preparation_selection(&preparation, guard)?;
    let identity = originals.financial_control;
    let (control_issued_at_ms, control_expires_at_ms) = owner
        .control
        .recheck_retained_capture_original_window(
            owner.publication.cash_financial(),
            identity.original_sha256,
            identity.lower_ms,
            identity.upper_ms,
        )
        .map_err(material)?;
    originals
        .body
        .clock_context
        .validate_within_original_window(control_issued_at_ms, control_expires_at_ms)
        .map_err(material)?;
    if originals.body.clock_context.lower_at_ms < identity.lower_ms
        || originals.body.clock_context.upper_at_ms < identity.upper_ms
        || originals.intent.issued_at_ms < control_issued_at_ms
        || originals.intent.expires_at_ms > control_expires_at_ms
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let intent = &originals.intent;
    let body = &originals.body;
    let challenge = &originals.challenge;
    body.validate_against_intent(intent).map_err(material)?;
    body.validate_against_prepared(candidate.prepared_record())
        .map_err(material)?;
    require_streams(
        &originals.prepared,
        &originals.transition_stream,
        &originals.recovery_stream,
    )?;
    let components = transport_body_components(&originals.transport)?;
    if originals.prepared != *candidate.prepared_record()
        || originals.preparation_guard_original != guard.original()
        || originals.state_proof != *candidate.proof()
        || intent.predecessor_descriptor_prefix_digest
            != financial_prefix_digest(owner, originals.preselection_cash_prefix)?
        || intent.sender_credential_digest != preparation.enrollment().app_credential().digest()
        || intent.candidate_digest != candidate.candidate_envelope_digest()
        || intent.state_statement_digest != candidate.full_state_sha256()
        || intent.preparation_id != candidate.preparation_id()?
        || intent.secure_index_before != owner.state.secure_index
        || intent.secure_index_after != candidate.successor_state().secure_index
        || intent.logical_journal_sequence_before != owner.financial_journal_revision
        || intent.logical_journal_sequence_after
            != owner
                .financial_journal_revision
                .checked_add(1)
                .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?
        || intent.native_operation_id == preparation.challenge().operation_id
        || intent.native_nonce == preparation.challenge().nonce
        || (
            body.request_digest,
            body.recipient_credential_digest,
            body.send_output_digest,
            body.encrypted_credit_digest,
        ) != components
        || challenge.purpose != KagemushaAppOperationApprovalPurposeV1::MonetaryTransition
        || challenge.operation_id != intent.native_operation_id
        || challenge.nonce != intent.native_nonce
        || challenge.issued_at_ms != intent.issued_at_ms
        || challenge.expires_at_ms != intent.expires_at_ms
        || challenge.enrollment_digest != intent.sender_credential_digest
        || challenge.normalized_guard_digest
            != originals.normalized.canonical_digest().map_err(material)?
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let mut subject = preparation.challenge().subject;
    subject.candidate_envelope_digest = intent.candidate_digest;
    subject.terminal_body_commitment = body.binding_digest().map_err(material)?;
    let c = preparation.enrollment().app_credential();
    let mut context = preparation.selected().context;
    context.terminal_commit_binding_digest =
        kagemusha_ordinary_terminal_guard_commit_binding_digest_v1(
            body.binding_digest().map_err(material)?,
            intent.candidate_digest,
            intent.state_statement_digest,
            intent.reservation_digest,
        )
        .map_err(material)?;
    context.sender_one_time_authorization_digest = if intent.operation == 2 {
        originals.prepared.preparation_authorization_digest
    } else {
        [0; 32]
    };
    context.transition_intent_digest = body.binding_digest().map_err(material)?;
    context.recovery_record_digest = intent.binding_digest().map_err(material)?;
    if challenge.subject != subject
        || challenge.account_binding != c.subject().account_binding
        || challenge.authority_policy_digest != c.subject().app_authority_policy_digest
        || challenge.attested_key_id != c.subject().attested_key_id
        || originals.normalized
            != KagemushaNormalizedGuardStatementV1::derive_from_transition(
                preparation.transition_statement(),
                context,
            )
            .map_err(material)?
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    challenge.canonical_signing_bytes().map_err(material)?;
    let pending = owner
        .pending
        .as_ref()
        .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
    if pending
        .reservation
        .canonical_commitment()
        .map_err(material)?
        != originals.prepared.reservation_digest
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    originals
        .body
        .clock_context
        .validate_within_original_window(
            pending.reservation.issued_at_ms,
            pending.reservation.expires_at_ms,
        )
        .map_err(material)?;
    if let TransportOriginals::Send {
        request,
        output,
        encrypted_credit,
        receiver_counter_floor,
        receiver_lease_original,
    } = &originals.transport
    {
        let retained = pending
            .send_credit
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        retained.originals.recheck_at_replay_position(
            owner,
            pending.operation,
            &retained.successor,
            &retained.receiver,
            retained.receiver_lease.as_deref(),
        )?;
        if retained.successor != *candidate.successor_state()
            || retained.originals.request()? != *request
            || retained.originals.output() != output
            || retained.originals.encrypted_credit() != encrypted_credit.as_slice()
            || retained.originals.receiver_counter_floor() != *receiver_counter_floor
            || retained.originals.receiver_lease_original() != receiver_lease_original.as_deref()
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let before = &owner.state;
        let statement = preparation.transition_statement();
        output
            .validate_against_clock(preparation.preparation_clock_context())
            .map_err(material)?;
        preparation
            .preparation_clock_context()
            .validate_within_original_window(request.body.issued_at_ms, request.body.expires_at_ms)
            .map_err(material)?;
        let expected_nullifier = kagemusha_ordinary_transition_nullifier_v1(
            before.state_commitment,
            before.secure_index,
            before.hardware_epoch.epoch_id,
            *before.lane.network_id.as_bytes(),
            before.lane.device_lane_id,
            before.liability_pool_id,
        )
        .map_err(material)?;
        if intent.operation != 2
            || body.amount != request.body.amount
            || request.body.release_id != before.release_id
            || request.body.network_id != *before.lane.network_id.as_bytes()
            || request.body.normalized_asset_id
                != kagemusha_asset_identity_digest_v1(&before.lane.asset).map_err(material)?
            || request.body.asset_incarnation != *before.asset_incarnation.as_bytes()
            || request.body.scale != before.lane.scale
            || request.body.reserve_pool_id != before.liability_pool_id
            || request.body.recipient_encryption_key != statement.recipient_encryption_key_binding
            || output.sender_before_commitment != before.state_commitment
            || output.sender_after_commitment != candidate.successor_state().state_commitment
            || output.transition_nullifier != expected_nullifier
            || output.credit_id != statement.peer_credit_id
            || kagemusha_ordinary_payment_body_digest_v1(
                body.send_output_digest,
                body.encrypted_credit_digest,
            )
            .map_err(material)?
                != body.prepared_projection_semantic_digest
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
    } else {
        let TransportOriginals::Redeem {
            output,
            beneficiary,
            manifest_original,
        } = &originals.transport
        else {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        };
        let (successor, retained) = pending
            .redeem_credit
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        retained.recheck_original_data(owner, pending.operation, successor)?;
        if intent.operation != 4
            || *successor != *candidate.successor_state()
            || retained.output() != output
            || retained.beneficiary() != beneficiary
            || retained.manifest_original() != manifest_original.as_slice()
            || output.binding_digest().map_err(material)?
                != body.prepared_projection_semantic_digest
            || output.amount != body.amount
            || output.lifecycle_digest != body.lifecycle_digest
            || output.artifact_manifest_digest != body.artifact_manifest_digest
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
    }
    Ok(())
}
// Receiver C/FI/possession and selected PI are independently authenticated holders, never WAL DTOs.
// The local Native interval selects admission. A signed remote clock projection grants no local time.
fn require_receiver(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    originals: &SelectionOriginals,
    receiver: Option<&KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
    lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
) -> Result<(), KagemushaStateErrorV1> {
    match &originals.transport {
        TransportOriginals::Redeem { .. } if receiver.is_none() && lease.is_none() => Ok(()),
        TransportOriginals::Send {
            request,
            receiver_counter_floor,
            receiver_lease_original,
            ..
        } => {
            let receiver = receiver.ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
            if receiver_lease_original.as_deref() != lease.map(|l| l.original()) {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            let runtime = &receiver.certificate().subject.owner.runtime;
            if runtime.network_id != owner.state.lane.network_id
                || runtime.asset != owner.state.lane.asset
                || runtime.asset_incarnation != owner.state.asset_incarnation
                || runtime.scale != owner.state.lane.scale
                || receiver.certificate().subject.issuance.release_id != owner.state.release_id
                || receiver.app_credential().digest() != request.body.recipient_credential_digest
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            for now in [
                originals.body.clock_context.lower_at_ms,
                originals.body.clock_context.upper_at_ms,
            ] {
                if now < request.body.issued_at_ms || now >= request.body.expires_at_ms {
                    return Err(KagemushaStateErrorV1::InvalidTrustedCommitTime);
                }
                match lease {
                    Some(lease) => receiver.recheck_with_integrity_lease(lease, now),
                    None => receiver.recheck_at_trusted_time(now),
                }
                .map_err(material)?;
            }
            request
                .authenticate_receiver_signature(receiver.app_credential(), *receiver_counter_floor)
                .map_err(material)?;
            Ok(())
        }
        _ => Err(KagemushaStateErrorV1::SnapshotIntegrity),
    }
}
fn require_pending_operation(
    pending: &TerminalPending,
    operation: DigestV1,
) -> Result<(), KagemushaStateErrorV1> {
    if pending.originals.intent.native_operation_id != operation {
        Err(KagemushaStateErrorV1::InvalidCandidateStage)
    } else {
        Ok(())
    }
}
fn require_admission_clock(
    originals: &SelectionOriginals,
    clock: &KagemushaOrdinaryCashClockContextV1,
) -> Result<(), KagemushaStateErrorV1> {
    clock
        .validate_within_original_window(
            originals.challenge.issued_at_ms,
            originals.challenge.expires_at_ms,
        )
        .map_err(material)?;
    if clock.lower_at_ms < originals.body.clock_context.lower_at_ms
        || clock.upper_at_ms < originals.body.clock_context.upper_at_ms
    {
        return Err(KagemushaStateErrorV1::SnapshotRollback);
    }
    Ok(())
}
fn authenticate_terminal(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    pending: &TerminalPending,
    original: &[u8],
    clock: &KagemushaOrdinaryCashClockContextV1,
) -> Result<KagemushaVerifiedAppOperationApprovalV1, KagemushaStateErrorV1> {
    require_admission_clock(&pending.originals, clock)?;
    let preparation = owner.captured_preparation()?;
    let selected = Selected {
        statement: preparation.transition_statement().clone(),
        successor: pending.candidate.successor_state().clone(),
        normalized: pending.originals.normalized,
        context: preparation.selected().context,
        challenge: pending.originals.challenge,
        lease: pending.lease.clone(),
        counter_floor: pending.originals.counter_floor,
    };
    let approved = owner.authenticate(original, &selected, clock.lower_at_ms)?;
    owner.authenticate(original, &selected, clock.upper_at_ms)?;
    Ok(approved)
}
fn terminal_record(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    pending: &TerminalPending,
    approved: &KagemushaVerifiedAppOperationApprovalV1,
    clock: KagemushaOrdinaryCashClockContextV1,
) -> Result<KagemushaOrdinaryCashTerminalRecordV1, KagemushaStateErrorV1> {
    require_admission_clock(&pending.originals, &clock)?;
    if approved.challenge() != &pending.originals.challenge {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let record = KagemushaOrdinaryCashTerminalRecordV1 {
        version: 1,
        body: pending.originals.body,
        sender_credential_digest: owner
            .publication
            .cash_financial()
            .enrollment()
            .app_credential()
            .digest(),
        preparation_authorization_digest: pending
            .originals
            .prepared
            .preparation_authorization_digest,
        terminal_authorization_digest: authorization(approved, pending.lease.as_deref())?,
        terminal_subject_digest: approved.challenge().subject_signing_digest,
        admission_clock_context: clock,
        approval_issued_at_ms: approved.challenge().issued_at_ms,
        approval_expires_at_ms: approved.challenge().expires_at_ms,
    };
    record
        .validate_against_originals(&pending.originals.intent, &pending.originals.prepared)
        .map_err(material)?;
    Ok(record)
}
fn selected_lease(
    original: &Option<Vec<u8>>,
    leases: &[Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>],
) -> Result<Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>, KagemushaStateErrorV1> {
    original
        .as_ref()
        .map(|raw| {
            leases
                .iter()
                .find(|lease| lease.original() == raw)
                .cloned()
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{NetworkId, block::BlockHeader};
    use std::num::NonZeroU64;

    // Public data descriptors exercise only stream correlation, never Native financial authority.
    fn stream_descriptor(
        transition: &[u8],
        recovery: &[u8],
    ) -> KagemushaOrdinaryPreparedOutgoingV1 {
        KagemushaOrdinaryPreparedOutgoingV1 {
            version: 1,
            operation: 2,
            predecessor_state: [1; 32],
            successor_state: [2; 32],
            transition_digest: [3; 32],
            prepared_transition_binding_digest: [4; 32],
            projection_semantic_digest: [5; 32],
            lifecycle_binding_digest: [6; 32],
            request_digest: [7; 32],
            artifact_manifest_digest: [0; 32],
            preparation_guard_digest: [8; 32],
            reservation_digest: [9; 32],
            preparation_authorization_digest: [10; 32],
            stream_lengths: [
                u64::try_from(transition.len()).unwrap(),
                u64::try_from(recovery.len()).unwrap(),
            ],
            stream_digests: [
                kagemusha_ordinary_sealed_transition_inputs_digest_v1(transition).unwrap(),
                kagemusha_ordinary_sealed_recovery_seeds_digest_v1(recovery).unwrap(),
            ],
        }
    }

    #[test]
    fn terminal_streams_accept_exact_profile_bounds_and_reject_empty_or_oversized_originals() {
        let transition =
            vec![11; usize::try_from(KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1).unwrap()];
        let recovery = vec![12; usize::try_from(KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1).unwrap()];
        let prepared = stream_descriptor(&transition, &recovery);
        require_streams(&prepared, &transition, &recovery).unwrap();
        assert!(require_streams(&prepared, &[], &recovery).is_err());
        assert!(require_streams(&prepared, &transition, &[]).is_err());
        let mut oversized_transition = transition.clone();
        oversized_transition.push(13);
        let mut claimed = prepared;
        claimed.stream_lengths[0] = u64::try_from(oversized_transition.len()).unwrap();
        assert!(require_streams(&claimed, &oversized_transition, &recovery).is_err());
        let mut oversized_recovery = recovery.clone();
        oversized_recovery.push(14);
        claimed = prepared;
        claimed.stream_lengths[1] = u64::try_from(oversized_recovery.len()).unwrap();
        assert!(require_streams(&claimed, &transition, &oversized_recovery).is_err());
    }

    #[test]
    fn terminal_streams_bind_each_complete_length_byte_and_domain() {
        let transition = [11; 4];
        let recovery = [12; 4];
        let prepared = stream_descriptor(&transition, &recovery);
        require_streams(&prepared, &transition, &recovery).unwrap();
        for index in 0..2 {
            let mut changed = prepared;
            changed.stream_lengths[index] += 1;
            assert!(matches!(
                require_streams(&changed, &transition, &recovery),
                Err(KagemushaStateErrorV1::SnapshotIntegrity)
            ));
            changed = prepared;
            changed.stream_digests[index][0] ^= 1;
            assert!(matches!(
                require_streams(&changed, &transition, &recovery),
                Err(KagemushaStateErrorV1::SnapshotIntegrity)
            ));
        }
        let mut changed_transition = transition;
        changed_transition[3] ^= 1;
        assert!(require_streams(&prepared, &changed_transition, &recovery).is_err());
        let mut changed_recovery = recovery;
        changed_recovery[3] ^= 1;
        assert!(require_streams(&prepared, &transition, &changed_recovery).is_err());
        assert!(require_streams(&prepared, &recovery, &transition).is_err());
        let equal_originals = stream_descriptor(&transition, &transition);
        require_streams(&equal_originals, &transition, &transition).unwrap();
        assert_ne!(
            equal_originals.stream_digests[0],
            equal_originals.stream_digests[1]
        );
        let mut substituted_domain = equal_originals;
        substituted_domain.stream_digests.swap(0, 1);
        assert!(require_streams(&substituted_domain, &transition, &transition).is_err());
    }

    #[test]
    fn terminal_network_projection_binds_the_exact_canonical_genesis_hash() {
        // Canonical header data supplies identity bytes; no signed-chain admission is claimed.
        let header = BlockHeader::new(NonZeroU64::MIN, None, None, 100, 0);
        let network = NetworkId::from_genesis_hash(header.hash());
        let changed_network = NetworkId::from_genesis_hash(
            BlockHeader::new(NonZeroU64::MIN, None, None, 101, 0).hash(),
        );
        let original = *network.as_bytes();
        assert_eq!(&original[..], header.hash().as_ref());
        assert_eq!(original, *network.as_bytes());
        assert_ne!(original, *changed_network.as_bytes());
        let actual = kagemusha_ordinary_transition_nullifier_v1(
            [1; 32],
            u128::MAX,
            [2; 32],
            original,
            [3; 32],
            [4; 32],
        )
        .unwrap();
        let mut transcript =
            iroha_data_model::kagemusha::KAGEMUSHA_ORDINARY_TRANSITION_NULLIFIER_DOMAIN_V1.to_vec();
        transcript.extend_from_slice(&[1; 32]);
        transcript.extend_from_slice(&u128::MAX.to_le_bytes());
        for selector in [[2; 32], original, [3; 32], [4; 32]] {
            transcript.extend_from_slice(&selector);
        }
        assert_eq!(actual, <DigestV1>::from(Sha256::digest(transcript)));
        assert_ne!(
            actual,
            kagemusha_ordinary_transition_nullifier_v1(
                [1; 32],
                u128::MAX,
                [2; 32],
                *changed_network.as_bytes(),
                [3; 32],
                [4; 32],
            )
            .unwrap()
        );
    }
}
