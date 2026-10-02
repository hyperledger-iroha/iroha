//! Incoming purpose1 custody after actual source/W2/State/Guard and acknowledged global Reserve.
//! The sole Main WAL retains fresh W1 control, clock, entropy and full originals separately.
//! Neither a decoded selector nor a captured W2 can invoke or satisfy this platform approval.
use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaAuthenticatedOrdinaryIncomingCandidateV1,
    KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    verify_ordinary_incoming_candidate_v1, verify_ordinary_incoming_preparation_guard_v1,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1, KagemushaOrdinaryIncomingTerminalBodyV1,
    KagemushaOrdinaryIncomingTerminalIntentV1,
    kagemusha_ordinary_terminal_guard_commit_binding_digest_v1,
};

#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryIncomingTerminalSelectedV1")]
pub(super) struct IncomingTerminalSelected {
    preselection_prefix: KagemushaRecoveryJournalPrefixV1,
    financial_control: CapturedFinancialControlIdentity,
    public_state_original: Vec<u8>,
    private_state_checkpoint_original: Vec<u8>,
    preparation_guard_original: Vec<u8>,
    intent: KagemushaOrdinaryIncomingTerminalIntentV1,
    body: KagemushaOrdinaryIncomingTerminalBodyV1,
    context: KagemushaGuardContextV1,
    normalized: KagemushaNormalizedGuardStatementV1,
    challenge: KagemushaAppOperationApprovalChallengeV1,
    lease_original: Option<Vec<u8>>,
    previous_counter: Option<u32>,
}

impl core::fmt::Debug for IncomingTerminalSelected {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IncomingTerminalSelected")
            .finish_non_exhaustive()
    }
}
impl Drop for IncomingTerminalSelected {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        self.private_state_checkpoint_original.zeroize();
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryIncomingTerminalCapturedV1")]
pub(super) struct IncomingTerminalCaptured {
    operation: DigestV1,
    body_original_sha256: DigestV1,
    intent_original_sha256: DigestV1,
    approval_original_sha256: DigestV1,
    authorization: DigestV1,
    clock: KagemushaOrdinaryCashClockContextV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryIncomingTerminalRecordV1")]
pub(super) enum IncomingTerminalRecord {
    Select(IncomingTerminalSelected),
    PlatformFence {
        operation: DigestV1,
    },
    Original {
        operation: DigestV1,
        clock: KagemushaOrdinaryCashClockContextV1,
        original: Vec<u8>,
        authorization: DigestV1,
        accepted_counter: Option<u32>,
    },
    Capture(IncomingTerminalCaptured),
}

pub(super) struct IncomingTerminalPending {
    selected: IncomingTerminalSelected,
    candidate: KagemushaAuthenticatedOrdinaryIncomingCandidateV1,
    guard: KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
    lease: Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
    fenced: bool,
    retained: Option<(
        KagemushaOrdinaryCashClockContextV1,
        KagemushaVerifiedAppOperationApprovalV1,
    )>,
    captured: Option<(
        KagemushaOrdinaryCashClockContextV1,
        IncomingTerminalCaptured,
        KagemushaVerifiedAppOperationApprovalV1,
    )>,
}

impl IncomingTerminalPending {
    /// Count actual already-authenticated W1 rows under the same held original; no new grant.
    pub(super) fn captured_wal_charge_bytes(
        &self,
        maximum_payload_bytes: u64,
    ) -> Result<u64, KagemushaStateErrorV1> {
        // Capture owns both the original clock and approval after draining the retained slot.
        let (clock, capture, original) = self
            .captured
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let operation = self.selected.intent.native_operation_id;
        let records = [
            IncomingTerminalRecord::Select(self.selected.clone()),
            IncomingTerminalRecord::PlatformFence { operation },
            IncomingTerminalRecord::Original {
                operation,
                clock: *clock,
                original: original.original().to_vec(),
                authorization: [0; 32],
                accepted_counter: original.app_attest_counter(),
            },
            IncomingTerminalRecord::Capture(capture.clone()),
        ];
        super::incoming_state_commit::checked_record_capacity_charge(
            records.into_iter().map(Record::IncomingTerminal),
            maximum_payload_bytes,
        )
    }
    pub(super) fn proving_financial_control_identity(&self) -> CapturedFinancialControlIdentity {
        self.selected.financial_control
    }
}

/// A borrow of the actual captured incoming purpose1 owner and its current Main descriptor.
/// It has no decoder, cloning constructor, caller clock or raw financial-secret field.
pub(crate) struct KagemushaAuthenticatedOrdinaryIncomingTerminalApprovalSelectionV1<'a> {
    owner: &'a KagemushaNativeOrdinaryCashOwnerV1,
    prefix: KagemushaRecoveryJournalPrefixV1,
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    pub(crate) fn select_incoming_terminal(
        &mut self,
        candidate: KagemushaAuthenticatedOrdinaryIncomingCandidateV1,
        guard: KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
        reserve_request_original_sha256: DigestV1,
    ) -> Result<KagemushaAppOperationApprovalChallengeV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if let Some(p) = self
            .pending_incoming
            .as_ref()
            .and_then(|p| p.terminal.as_ref())
        {
            candidate.recheck_incoming_selection(&self.captured_incoming_approval()?, &guard)?;
            if candidate.candidate_original_sha256() != p.selected.intent.candidate_original_sha256
                || guard.original() != p.selected.preparation_guard_original
                || reserve_request_original_sha256
                    != p.selected.intent.reserve_request_original_sha256
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            let operation = p.selected.intent.native_operation_id;
            self.require_live_incoming_terminal(operation)?;
            return Ok(self.incoming_terminal_pending()?.selected.challenge);
        }
        let preparation = self.captured_incoming_approval()?;
        candidate.recheck_incoming_selection(&preparation, &guard)?;
        let reservation = preparation.reservation()?.clone();
        let receipt = self
            .lineage_cas
            .incoming_reservation_receipt(
                reserve_request_original_sha256,
                self.publication.cash_financial(),
                &reservation,
            )
            .map_err(material)?;
        receipt
            .recheck_historical(self.publication.cash_financial())
            .map_err(material)?;
        let reserve_receipt_original_sha256 = sha(receipt.original().map_err(material)?);
        let preparation_digest = preparation
            .preparation()?
            .binding_digest()
            .map_err(material)?;
        let transition_sha =
            sha(&norito::encode_canonical(preparation.transition_statement()?).map_err(material)?);
        let purpose2_sha = sha(preparation.original()?);
        let operation_kind = match preparation.transition_statement()?.kind {
            KagemushaTransitionKindV1::MintFold => 1,
            KagemushaTransitionKindV1::ReceiveFold => 3,
            _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
        };
        let previous_challenge = *preparation.challenge()?;
        let previous_financial_control =
            preparation.preparation()?.financial_control_original_sha256;
        let previous_clock = *preparation.preparation_clock_context()?;
        let previous_capture_clock = *preparation.approval_admission_clock_context()?;
        // W1 control is independently acknowledged after the potentially slow State proof.
        let captured = self
            .control
            .capture_proof_decision(self.publication.cash_financial())
            .map_err(material)?;
        let financial_control = CapturedFinancialControlIdentity {
            original_sha256: captured.original_sha256().map_err(material)?,
            lower_ms: captured.captured_lower_ms(),
            upper_ms: captured.captured_upper_ms(),
        };
        if financial_control.original_sha256 == previous_financial_control {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let (fi_issued, fi_expires) = self
            .control
            .recheck_retained_capture_original_window(
                self.publication.cash_financial(),
                financial_control.original_sha256,
                financial_control.lower_ms,
                financial_control.upper_ms,
            )
            .map_err(material)?;
        let clock = self
            .publication
            .cash_financial()
            .current_cash_clock_context()
            .map_err(material)?;
        clock
            .validate_within_original_window(fi_issued, fi_expires)
            .map_err(material)?;
        if clock.lower_at_ms < financial_control.lower_ms
            || clock.upper_at_ms < financial_control.upper_ms
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        require_fresh_terminal_clock(&previous_clock, &previous_capture_clock, &clock)?;
        let issued = clock.lower_at_ms;
        let expires = issued
            .checked_add(KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1)
            .ok_or(KagemushaStateErrorV1::InvalidTrustedCommitTime)?
            .min(fi_expires)
            .min(self.credential_floor()?.approval_valid_until_ms());
        clock
            .validate_within_original_window(issued, expires)
            .map_err(material)?;
        let mut entropy = zeroize::Zeroizing::new([0u8; 64]);
        OsRng.try_fill_bytes(entropy.as_mut()).map_err(material)?;
        let mut hash = Sha256::new();
        hash.update(b"iroha:kagemusha:v1:ordinary-incoming-terminal-native-operation\0");
        hash.update(self.prefix.head);
        hash.update(self.prefix.sequence.to_le_bytes());
        hash.update(candidate.candidate_original_sha256());
        hash.update(&entropy[..32]);
        let operation: DigestV1 = hash.finalize().into();
        let nonce: DigestV1 = entropy[32..].try_into().map_err(material)?;
        if operation == [0; 32]
            || nonce == [0; 32]
            || operation == nonce
            || operation == previous_challenge.operation_id
            || nonce == previous_challenge.nonce
            || self.used_operations.contains(&operation)
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let intent = KagemushaOrdinaryIncomingTerminalIntentV1 {
            version: 1,
            operation: operation_kind,
            native_operation_id: operation,
            native_nonce: nonce,
            preparation_digest,
            reservation_digest: reservation.digest().map_err(material)?,
            finalized_source_original_sha256: reservation.finalized_source_original_sha256,
            source_proof_original_sha256: reservation.source_proof_original_sha256,
            state_original_sha256: sha(candidate.public_state_original()),
            transition_statement_original_sha256: transition_sha,
            preparation_guard_original_sha256: sha(guard.original()),
            candidate_original_sha256: candidate.candidate_original_sha256(),
            purpose2_approval_original_sha256: purpose2_sha,
            financial_control_original_sha256: financial_control.original_sha256,
            predecessor_descriptor_prefix_digest: financial_prefix_digest(self, self.prefix)?,
            reserve_request_original_sha256,
            reserve_receipt_original_sha256,
            clock_context: clock,
            financial_index_before: self.state.secure_index,
            financial_index_after: candidate.successor_state().secure_index,
            financial_sequence_before: self.state.logical_sequence,
            financial_sequence_after: candidate.successor_state().logical_sequence,
            logical_journal_sequence_before: self.financial_journal_revision,
            logical_journal_sequence_after: self
                .financial_journal_revision
                .checked_add(1)
                .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?,
            issued_at_ms: issued,
            expires_at_ms: expires,
        };
        let body = KagemushaOrdinaryIncomingTerminalBodyV1 { intent };
        let (context, normalized, challenge) = challenge_fields(self, &intent, &body)?;
        let lease = self
            .publication
            .cash_financial()
            .retained_integrity_lease()
            .cloned();
        let selected = IncomingTerminalSelected {
            preselection_prefix: self.prefix,
            financial_control,
            public_state_original: candidate.public_state_original().to_vec(),
            private_state_checkpoint_original: candidate.private_checkpoint_original().to_vec(),
            preparation_guard_original: guard.original().to_vec(),
            intent,
            body,
            context,
            normalized,
            challenge,
            lease_original: lease.as_ref().map(|l| l.original().to_vec()),
            previous_counter: self.counter_floor,
        };
        require_selected(self, &selected, &candidate, &guard)?;
        self.persist(&Record::IncomingTerminal(IncomingTerminalRecord::Select(
            selected.clone(),
        )))?;
        self.used_operations.insert(operation);
        self.pending_incoming
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .terminal = Some(IncomingTerminalPending {
            selected,
            candidate,
            guard,
            lease,
            fenced: false,
            retained: None,
            captured: None,
        });
        self.require_live_incoming_terminal(operation)?;
        Ok(challenge)
    }

    pub(crate) fn fence_incoming_terminal_platform(
        &mut self,
        operation: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_incoming_terminal(operation)?;
        if self.incoming_terminal_pending()?.fenced {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.persist(&Record::IncomingTerminal(
            IncomingTerminalRecord::PlatformFence { operation },
        ))?;
        self.incoming_terminal_pending_mut()?.fenced = true;
        self.require_live_incoming_terminal(operation)
    }

    pub(crate) fn capture_incoming_terminal_original(
        &mut self,
        operation: DigestV1,
        original: &[u8],
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_incoming_terminal(operation)?;
        let p = self.incoming_terminal_pending()?;
        if !p.fenced || p.retained.is_some() || p.captured.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let clock = self
            .publication
            .cash_financial()
            .current_cash_clock_context()
            .map_err(material)?;
        let approved = authenticate(self, p, original, &clock)?;
        let auth = authorization(&approved, p.lease.as_deref())?;
        self.persist(&Record::IncomingTerminal(
            IncomingTerminalRecord::Original {
                operation,
                clock,
                original: original.to_vec(),
                authorization: auth,
                accepted_counter: approved.app_attest_counter(),
            },
        ))?;
        // Preserve this durable platform counter even if the post-fsync time grant expires.
        self.counter_floor = approved.app_attest_counter().or(self.counter_floor);
        self.incoming_terminal_pending_mut()?.retained = Some((clock, approved));
        self.acknowledge_incoming_terminal_capture(operation)
    }

    pub(crate) fn acknowledge_incoming_terminal_capture(
        &mut self,
        operation: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_incoming_terminal(operation)?;
        let p = self.incoming_terminal_pending()?;
        if p.captured.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let (old_clock, a) = p
            .retained
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let clock = self
            .publication
            .cash_financial()
            .current_cash_clock_context()
            .map_err(material)?;
        require_clock(self, p, old_clock)?;
        require_clock_nondecreasing(old_clock, &clock)?;
        let captured = captured_record(self, p, a, clock)?;
        self.persist(&Record::IncomingTerminal(IncomingTerminalRecord::Capture(
            captured.clone(),
        )))?;
        let p = self.incoming_terminal_pending_mut()?;
        super::incoming_state_commit::install_captured_approval(
            &mut p.retained,
            &mut p.captured,
            captured,
        )?;
        self.captured_incoming_terminal()?
            .recheck_selected_originals_and_current_custody()
    }

    pub(crate) fn captured_incoming_terminal(
        &self,
    ) -> Result<
        KagemushaAuthenticatedOrdinaryIncomingTerminalApprovalSelectionV1<'_>,
        KagemushaStateErrorV1,
    > {
        let loan = KagemushaAuthenticatedOrdinaryIncomingTerminalApprovalSelectionV1 {
            owner: self,
            prefix: self.prefix,
        };
        loan.recheck_selected_originals_and_current_custody()?;
        Ok(loan)
    }
    fn incoming_terminal_pending(&self) -> Result<&IncomingTerminalPending, KagemushaStateErrorV1> {
        self.pending_incoming
            .as_ref()
            .and_then(|p| p.terminal.as_ref())
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }
    fn incoming_terminal_pending_mut(
        &mut self,
    ) -> Result<&mut IncomingTerminalPending, KagemushaStateErrorV1> {
        self.pending_incoming
            .as_mut()
            .and_then(|p| p.terminal.as_mut())
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }
    fn require_live_incoming_terminal(
        &self,
        operation: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let p = self.incoming_terminal_pending()?;
        if p.selected.intent.native_operation_id != operation {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        require_selected(self, &p.selected, &p.candidate, &p.guard)?;
        self.publication
            .cash_financial()
            .trusted_time_interval()
            .map_err(material)?
            .require_validity(
                p.selected.challenge.issued_at_ms,
                p.selected.challenge.expires_at_ms,
            )
            .map_err(material)
    }

    pub(super) fn replay_incoming_terminal(
        &mut self,
        record: IncomingTerminalRecord,
        preceding: Option<KagemushaRecoveryJournalPrefixV1>,
        leases: &[Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>],
    ) -> Result<(), KagemushaStateErrorV1> {
        match record {
            IncomingTerminalRecord::Select(selected) => {
                if self
                    .pending_incoming
                    .as_ref()
                    .is_none_or(|p| p.terminal.is_some())
                    || preceding != Some(selected.preselection_prefix)
                    || selected.previous_counter != self.counter_floor
                    || self
                        .used_operations
                        .contains(&selected.intent.native_operation_id)
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                let preparation = self.captured_incoming_approval()?;
                let guard = verify_ordinary_incoming_preparation_guard_v1(
                    &preparation,
                    &selected.preparation_guard_original,
                )?;
                let candidate = verify_ordinary_incoming_candidate_v1(
                    &self.verifier,
                    &preparation,
                    &selected.public_state_original,
                    &guard,
                    &selected.private_state_checkpoint_original,
                )?;
                require_selected(self, &selected, &candidate, &guard)?;
                let lease = match &selected.lease_original {
                    None => None,
                    Some(raw) => Some(Arc::clone(
                        leases
                            .iter()
                            .find(|l| l.original() == raw)
                            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
                    )),
                };
                self.used_operations
                    .insert(selected.intent.native_operation_id);
                self.pending_incoming
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .terminal = Some(IncomingTerminalPending {
                    selected,
                    candidate,
                    guard,
                    lease,
                    fenced: false,
                    retained: None,
                    captured: None,
                });
            }
            IncomingTerminalRecord::PlatformFence { operation } => {
                let p = self.incoming_terminal_pending_mut()?;
                if p.selected.intent.native_operation_id != operation || p.fenced {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                p.fenced = true;
            }
            IncomingTerminalRecord::Original {
                operation,
                clock,
                original,
                authorization: expected,
                accepted_counter,
            } => {
                let p = self.incoming_terminal_pending()?;
                if !p.fenced
                    || p.retained.is_some()
                    || p.captured.is_some()
                    || operation != p.selected.intent.native_operation_id
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                let a = authenticate(self, p, &original, &clock)?;
                if authorization(&a, p.lease.as_deref())? != expected
                    || a.app_attest_counter() != accepted_counter
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                self.counter_floor = accepted_counter.or(self.counter_floor);
                self.incoming_terminal_pending_mut()?.retained = Some((clock, a));
            }
            IncomingTerminalRecord::Capture(record) => {
                let p = self.incoming_terminal_pending()?;
                let (clock, a) = p
                    .retained
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                require_clock(self, p, clock)?;
                require_clock_nondecreasing(clock, &record.clock)?;
                if p.captured.is_some()
                    || !p.fenced
                    || captured_record(self, p, a, record.clock)? != record
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                let p = self.incoming_terminal_pending_mut()?;
                super::incoming_state_commit::install_captured_approval(
                    &mut p.retained,
                    &mut p.captured,
                    record,
                )?;
            }
        }
        Ok(())
    }
}

fn challenge_fields(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    intent: &KagemushaOrdinaryIncomingTerminalIntentV1,
    body: &KagemushaOrdinaryIncomingTerminalBodyV1,
) -> Result<
    (
        KagemushaGuardContextV1,
        KagemushaNormalizedGuardStatementV1,
        KagemushaAppOperationApprovalChallengeV1,
    ),
    KagemushaStateErrorV1,
> {
    intent.validate_shape().map_err(material)?;
    if body.intent != *intent {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let w2 = owner.captured_incoming_approval()?;
    let mut context = *w2.normalized_guard_context()?;
    context.transition_intent_digest = body.binding_digest().map_err(material)?;
    context.recovery_record_digest = intent.binding_digest().map_err(material)?;
    context.terminal_commit_binding_digest =
        kagemusha_ordinary_terminal_guard_commit_binding_digest_v1(
            body.binding_digest().map_err(material)?,
            intent.candidate_original_sha256,
            intent.state_original_sha256,
            intent.reservation_digest,
        )
        .map_err(material)?;
    context.sender_one_time_authorization_digest = [0; 32];
    let normalized = KagemushaNormalizedGuardStatementV1::derive_from_transition(
        w2.transition_statement()?,
        context,
    )
    .map_err(material)?;
    let mut subject = w2.challenge()?.subject;
    subject.candidate_envelope_digest = intent.candidate_original_sha256;
    subject.terminal_body_commitment = body.binding_digest().map_err(material)?;
    let c = w2.credential()?;
    let challenge = KagemushaAppOperationApprovalChallengeV1 {
        version: 1,
        purpose: KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
        operation_id: intent.native_operation_id,
        nonce: intent.native_nonce,
        account_binding: c.subject().account_binding,
        authority_policy_digest: c.subject().app_authority_policy_digest,
        attested_key_id: c.subject().attested_key_id,
        enrollment_digest: c.digest(),
        subject_signing_digest: sha(&subject
            .canonical_ordinary_incoming_terminal_signing_bytes()
            .map_err(material)?),
        normalized_guard_digest: normalized.canonical_digest().map_err(material)?,
        issued_at_ms: intent.issued_at_ms,
        expires_at_ms: intent.expires_at_ms,
        subject,
    };
    challenge.canonical_signing_bytes().map_err(material)?;
    Ok((context, normalized, challenge))
}

fn require_selected(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    selected: &IncomingTerminalSelected,
    candidate: &KagemushaAuthenticatedOrdinaryIncomingCandidateV1,
    guard: &KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
) -> Result<(), KagemushaStateErrorV1> {
    let w2 = owner.captured_incoming_approval()?;
    candidate.recheck_incoming_selection(&w2, guard)?;
    let reservation = w2.reservation()?;
    let intent = &selected.intent;
    let receipt = owner
        .lineage_cas
        .incoming_reservation_receipt(
            intent.reserve_request_original_sha256,
            owner.publication.cash_financial(),
            reservation,
        )
        .map_err(material)?;
    receipt
        .recheck_historical(owner.publication.cash_financial())
        .map_err(material)?;
    let fi = selected.financial_control;
    owner
        .control
        .recheck_retained_capture_identity(
            owner.publication.cash_financial(),
            fi.original_sha256,
            fi.lower_ms,
            fi.upper_ms,
        )
        .map_err(material)?;
    let (issued, expires) = owner
        .control
        .recheck_retained_capture_original_window(
            owner.publication.cash_financial(),
            fi.original_sha256,
            fi.lower_ms,
            fi.upper_ms,
        )
        .map_err(material)?;
    owner
        .publication
        .cash_financial()
        .verified_retained_cash_clock_originals(&intent.clock_context)
        .map_err(material)?;
    intent
        .clock_context
        .validate_within_original_window(issued, expires)
        .map_err(material)?;
    require_fresh_terminal_clock(
        w2.preparation_clock_context()?,
        w2.approval_admission_clock_context()?,
        &intent.clock_context,
    )?;
    let (context, normalized, challenge) = challenge_fields(owner, intent, &selected.body)?;
    let operation = match w2.transition_statement()?.kind {
        KagemushaTransitionKindV1::MintFold => 1,
        KagemushaTransitionKindV1::ReceiveFold => 3,
        _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
    };
    if intent.operation != operation
        || selected.context != context
        || selected.normalized != normalized
        || selected.challenge != challenge
        || selected.public_state_original != candidate.public_state_original()
        || selected.private_state_checkpoint_original != candidate.private_checkpoint_original()
        || selected.preparation_guard_original != guard.original()
        || intent.preparation_digest != w2.preparation()?.binding_digest().map_err(material)?
        || intent.reservation_digest != reservation.digest().map_err(material)?
        || intent.finalized_source_original_sha256 != reservation.finalized_source_original_sha256
        || intent.source_proof_original_sha256 != reservation.source_proof_original_sha256
        || intent.state_original_sha256 != sha(candidate.public_state_original())
        || intent.transition_statement_original_sha256
            != sha(&norito::encode_canonical(w2.transition_statement()?).map_err(material)?)
        || intent.preparation_guard_original_sha256 != sha(guard.original())
        || intent.candidate_original_sha256 != candidate.candidate_original_sha256()
        || intent.purpose2_approval_original_sha256 != sha(w2.original()?)
        || intent.financial_control_original_sha256 != fi.original_sha256
        || fi.original_sha256 == w2.preparation()?.financial_control_original_sha256
        || intent.predecessor_descriptor_prefix_digest
            != financial_prefix_digest(owner, selected.preselection_prefix)?
        || intent.reserve_receipt_original_sha256 != sha(receipt.original().map_err(material)?)
        || receipt.request_original_sha256() != intent.reserve_request_original_sha256
        || intent.financial_index_before != owner.state.secure_index
        || intent.financial_index_after != candidate.successor_state().secure_index
        || intent.financial_sequence_before != owner.state.logical_sequence
        || intent.financial_sequence_after != candidate.successor_state().logical_sequence
        || intent.logical_journal_sequence_before != owner.financial_journal_revision
        || intent.logical_journal_sequence_after
            != owner
                .financial_journal_revision
                .checked_add(1)
                .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?
        || intent.native_operation_id == w2.challenge()?.operation_id
        || intent.native_nonce == w2.challenge()?.nonce
        || intent.clock_context.lower_at_ms < fi.lower_ms
        || intent.clock_context.upper_at_ms < fi.upper_ms
        || intent.issued_at_ms != intent.clock_context.lower_at_ms
        || intent.issued_at_ms < issued
        || intent.expires_at_ms > expires
        || selected.challenge.expires_at_ms
            > owner
                .publication
                .cash_financial()
                .enrollment()
                .app_credential()
                .subject()
                .expires_at_ms
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

fn require_clock_nondecreasing(
    previous: &KagemushaOrdinaryCashClockContextV1,
    current: &KagemushaOrdinaryCashClockContextV1,
) -> Result<(), KagemushaStateErrorV1> {
    previous.validate_shape().map_err(material)?;
    current.validate_shape().map_err(material)?;
    if current.lower_at_ms < previous.lower_at_ms || current.upper_at_ms < previous.upper_at_ms {
        return Err(KagemushaStateErrorV1::SnapshotRollback);
    }
    Ok(())
}

fn require_fresh_terminal_clock(
    preparation: &KagemushaOrdinaryCashClockContextV1,
    capture: &KagemushaOrdinaryCashClockContextV1,
    terminal: &KagemushaOrdinaryCashClockContextV1,
) -> Result<(), KagemushaStateErrorV1> {
    require_clock_nondecreasing(preparation, terminal)?;
    require_clock_nondecreasing(capture, terminal)?;
    if terminal.request_nonce == preparation.request_nonce
        || terminal.request_nonce == capture.request_nonce
    {
        return Err(KagemushaStateErrorV1::SnapshotRollback);
    }
    Ok(())
}

fn require_clock(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    p: &IncomingTerminalPending,
    clock: &KagemushaOrdinaryCashClockContextV1,
) -> Result<(), KagemushaStateErrorV1> {
    clock
        .validate_within_original_window(
            p.selected.challenge.issued_at_ms,
            p.selected.challenge.expires_at_ms,
        )
        .map_err(material)?;
    let signed_clock = owner
        .publication
        .cash_financial()
        .verified_retained_cash_clock_originals(clock)
        .map_err(material)?;
    signed_clock.recheck_cash_context(clock).map_err(material)?;
    if clock.lower_at_ms < p.selected.intent.clock_context.lower_at_ms
        || clock.upper_at_ms < p.selected.intent.clock_context.upper_at_ms
    {
        return Err(KagemushaStateErrorV1::SnapshotRollback);
    }
    Ok(())
}
fn authenticate(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    p: &IncomingTerminalPending,
    raw: &[u8],
    clock: &KagemushaOrdinaryCashClockContextV1,
) -> Result<KagemushaVerifiedAppOperationApprovalV1, KagemushaStateErrorV1> {
    require_clock(owner, p, clock)?;
    let selected = Selected {
        statement: owner
            .captured_incoming_approval()?
            .transition_statement()?
            .clone(),
        successor: p.candidate.successor_state().clone(),
        normalized: p.selected.normalized,
        context: p.selected.context,
        challenge: p.selected.challenge,
        lease: p.lease.clone(),
        counter_floor: p.selected.previous_counter,
    };
    let a = owner.authenticate(raw, &selected, clock.lower_at_ms)?;
    owner.authenticate(raw, &selected, clock.upper_at_ms)?;
    Ok(a)
}
fn captured_record(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    p: &IncomingTerminalPending,
    a: &KagemushaVerifiedAppOperationApprovalV1,
    clock: KagemushaOrdinaryCashClockContextV1,
) -> Result<IncomingTerminalCaptured, KagemushaStateErrorV1> {
    require_clock(owner, p, &clock)?;
    if a.challenge() != &p.selected.challenge {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    a.recheck_at_trusted_time(clock.lower_at_ms)
        .map_err(material)?;
    a.recheck_at_trusted_time(clock.upper_at_ms)
        .map_err(material)?;
    Ok(IncomingTerminalCaptured {
        operation: p.selected.intent.native_operation_id,
        body_original_sha256: sha(&norito::encode_canonical(&p.selected.body).map_err(material)?),
        intent_original_sha256: sha(
            &norito::encode_canonical(&p.selected.intent).map_err(material)?
        ),
        approval_original_sha256: sha(a.original()),
        authorization: authorization(a, p.lease.as_deref())?,
        clock,
    })
}
fn sha(raw: &[u8]) -> DigestV1 {
    Sha256::digest(raw).into()
}
fn financial_prefix_digest(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    prefix: KagemushaRecoveryJournalPrefixV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    if !owner
        .journal
        .contains_recovery_prefix(prefix)
        .map_err(storage)?
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:ordinary-financial-descriptor-prefix\0");
    hash.update(owner.state.state_commitment);
    hash.update(owner.financial_journal_revision.to_le_bytes());
    hash.update(norito::encode_canonical(&prefix).map_err(material)?);
    for digest in owner.publication.historical_original_commitments()? {
        hash.update(digest);
    }
    Ok(hash.finalize().into())
}

impl KagemushaAuthenticatedOrdinaryIncomingTerminalApprovalSelectionV1<'_> {
    pub(crate) fn recheck_selected_originals_and_current_custody(
        &self,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.owner
            .recheck_proving_history(ProvingHistoryOperation::IncomingTerminal)?;
        if self.prefix != self.owner.prefix {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let p = self.owner.incoming_terminal_pending()?;
        let (original_clock, record, a) = p
            .captured
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        require_selected(self.owner, &p.selected, &p.candidate, &p.guard)?;
        require_clock(self.owner, p, original_clock)?;
        require_clock_nondecreasing(original_clock, &record.clock)?;
        // Keep the exact Original cut independently authenticated after CaptureAck and cold replay.
        authenticate(self.owner, p, a.original(), original_clock)?;
        if !p.fenced || &captured_record(self.owner, p, a, record.clock)? != record {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
    fn pending(&self) -> Result<&IncomingTerminalPending, KagemushaStateErrorV1> {
        self.owner.incoming_terminal_pending()
    }
    pub(crate) fn preparation_selection(
        &self,
    ) -> Result<KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>, KagemushaStateErrorV1>
    {
        self.recheck_selected_originals_and_current_custody()?;
        self.owner.captured_incoming_approval()
    }
    pub(crate) fn candidate(
        &self,
    ) -> Result<&KagemushaAuthenticatedOrdinaryIncomingCandidateV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.candidate)
    }
    pub(crate) fn preparation_guard(
        &self,
    ) -> Result<&KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1, KagemushaStateErrorV1>
    {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.guard)
    }
    pub(crate) fn terminal_intent(
        &self,
    ) -> Result<&KagemushaOrdinaryIncomingTerminalIntentV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.selected.intent)
    }
    pub(crate) fn terminal_body(
        &self,
    ) -> Result<&KagemushaOrdinaryIncomingTerminalBodyV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.selected.body)
    }
    pub(crate) fn normalized_guard_context(
        &self,
    ) -> Result<&KagemushaGuardContextV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.selected.context)
    }
    pub(crate) fn normalized_guard_statement(
        &self,
    ) -> Result<&KagemushaNormalizedGuardStatementV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.selected.normalized)
    }
    pub(crate) fn challenge(
        &self,
    ) -> Result<&KagemushaAppOperationApprovalChallengeV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.selected.challenge)
    }
    pub(crate) fn original(&self) -> Result<&[u8], KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(self
            .pending()?
            .captured
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .2
            .original())
    }
    pub(crate) fn original_approval_integrity_lease(
        &self,
    ) -> Result<Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(self.pending()?.lease.as_deref())
    }
    pub(crate) fn previous_app_attest_counter(&self) -> Result<Option<u32>, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(self.pending()?.selected.previous_counter)
    }
    pub(crate) fn authorization_binding_digest(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(self
            .pending()?
            .captured
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .1
            .authorization)
    }
    pub(crate) fn admission_clock_context(
        &self,
    ) -> Result<&KagemushaOrdinaryCashClockContextV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self
            .pending()?
            .captured
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .1
            .clock)
    }
    pub(crate) fn reserve_receipt_original(&self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let intent = &self.pending()?.selected.intent;
        let w2 = self.owner.captured_incoming_approval()?;
        let receipt = self
            .owner
            .lineage_cas
            .incoming_reservation_receipt(
                intent.reserve_request_original_sha256,
                self.owner.publication.cash_financial(),
                w2.reservation()?,
            )
            .map_err(material)?;
        Ok(receipt.original().map_err(material)?.to_vec())
    }
    pub(crate) fn recheck_incoming_reservation(
        &self,
        receipt: &KagemushaAuthenticatedOrdinaryIncomingReservationReceiptV1<'_>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        receipt
            .recheck_historical(self.owner.publication.cash_financial())
            .map_err(material)?;
        let preparation = self.owner.captured_incoming_approval()?;
        if receipt.reservation().map_err(material)? != preparation.reservation()?
            || receipt.request_original_sha256()
                != self.terminal_intent()?.reserve_request_original_sha256
            || receipt.original().map_err(material)? != self.reserve_receipt_original()?
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        receipt
            .recheck_historical(self.owner.publication.cash_financial())
            .map_err(material)?;
        self.recheck_selected_originals_and_current_custody()
    }
    pub(crate) fn financial_control_original(&self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let i = self.pending()?.selected.financial_control;
        let cap = self
            .owner
            .control
            .borrow_captured_proof_decision(
                self.owner.publication.cash_financial(),
                i.original_sha256,
                i.lower_ms,
                i.upper_ms,
            )
            .map_err(material)?;
        let original = cap.original().map_err(material)?.to_vec();
        self.recheck_selected_originals_and_current_custody()?;
        Ok(original)
    }
    pub(crate) fn with_retained_verified_signed_clock_originals(
        &self,
        visitor: &mut dyn for<'clock> FnMut(
            [&'clock KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1; 3],
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let w2 = self.owner.captured_incoming_approval()?;
        let preparation = self
            .owner
            .publication
            .cash_financial()
            .verified_retained_cash_clock_originals(w2.preparation_clock_context()?)
            .map_err(material)?;
        let intent = self
            .owner
            .publication
            .cash_financial()
            .verified_retained_cash_clock_originals(&self.terminal_intent()?.clock_context)
            .map_err(material)?;
        let admission = self
            .owner
            .publication
            .cash_financial()
            .verified_retained_cash_clock_originals(self.admission_clock_context()?)
            .map_err(material)?;
        w2.recheck_selected_originals_and_current_custody()?;
        visitor([&preparation, &intent, &admission])?;
        self.recheck_selected_originals_and_current_custody()
    }
    pub(crate) fn with_borrowed_financial_secret(
        &self,
        visitor: &mut dyn for<'secret> FnMut(
            &'secret [u8; 32],
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let i = self.pending()?.selected.financial_control;
        let financial = self.owner.publication.cash_financial();
        let cap = self
            .owner
            .control
            .borrow_captured_proof_decision(financial, i.original_sha256, i.lower_ms, i.upper_ms)
            .map_err(material)?;
        let secret = cap.financial_secret().map_err(material)?;
        if crate::kagemusha_v1_recursion::device_authority_commitment_v1(*secret)
            != financial
                .enrollment()
                .app_credential()
                .subject()
                .financial_authority_commitment
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let result = visitor(secret);
        self.recheck_selected_originals_and_current_custody()?;
        result
    }
}

#[cfg(test)]
mod incoming_terminal_clock_tests {
    use super::*;

    // Synthetic public clock DATA only; these do not create a signed Native clock or owner.
    fn clock(nonce: u8, lower: u64, upper: u64) -> KagemushaOrdinaryCashClockContextV1 {
        KagemushaOrdinaryCashClockContextV1 {
            version: 1,
            request_nonce: [nonce; 32],
            signed_observations_original_digest: [nonce + 10; 32],
            lower_at_ms: lower,
            upper_at_ms: upper,
        }
    }

    #[test]
    fn incoming_terminal_clock_requires_fresh_nonce_and_both_w2_cuts() {
        let preparation = clock(1, 100, 101);
        let capture = clock(2, 110, 111);
        let terminal = clock(3, 120, 121);
        assert_eq!(
            require_fresh_terminal_clock(&preparation, &capture, &terminal),
            Ok(())
        );
        for previous in [preparation, capture] {
            let reused_nonce = KagemushaOrdinaryCashClockContextV1 {
                request_nonce: previous.request_nonce,
                ..terminal
            };
            assert_eq!(
                require_fresh_terminal_clock(&preparation, &capture, &reused_nonce),
                Err(KagemushaStateErrorV1::SnapshotRollback)
            );
        }
        for regressing in [clock(3, 109, 121), clock(3, 110, 110), clock(3, 100, 101)] {
            assert_eq!(
                require_fresh_terminal_clock(&preparation, &capture, &regressing),
                Err(KagemushaStateErrorV1::SnapshotRollback)
            );
        }
    }

    #[test]
    fn incoming_terminal_capture_clock_never_regresses_either_original_bound() {
        let original = clock(3, 120, 125);
        assert_eq!(
            require_clock_nondecreasing(&original, &clock(4, 120, 125)),
            Ok(())
        );
        assert_eq!(
            require_clock_nondecreasing(&original, &clock(4, 130, 135)),
            Ok(())
        );
        for regressing in [clock(4, 119, 125), clock(4, 120, 124)] {
            assert_eq!(
                require_clock_nondecreasing(&original, &regressing),
                Err(KagemushaStateErrorV1::SnapshotRollback)
            );
        }
        let malformed = KagemushaOrdinaryCashClockContextV1 {
            request_nonce: [0; 32],
            ..original
        };
        assert!(require_clock_nondecreasing(&malformed, &clock(4, 130, 135)).is_err());
    }

    #[test]
    fn incoming_terminal_original_wal_keeps_full_signed_clock_selector() {
        let original_clock = clock(3, 120, 125);
        let original = IncomingTerminalRecord::Original {
            operation: [4; 32],
            clock: original_clock,
            original: vec![5; 32],
            authorization: [6; 32],
            accepted_counter: Some(7),
        };
        let raw = norito::encode_canonical(&original).unwrap();
        let decoded: IncomingTerminalRecord =
            norito::decode_canonical_with_limits(&raw, norito::canonical_decode_limits(raw.len()))
                .unwrap();
        assert_eq!(decoded, original);
        let same_bounds_different_originals = IncomingTerminalRecord::Original {
            operation: [4; 32],
            clock: clock(8, 120, 125),
            original: vec![5; 32],
            authorization: [6; 32],
            accepted_counter: Some(7),
        };
        assert_ne!(
            raw,
            norito::encode_canonical(&same_bounds_different_originals).unwrap()
        );
    }
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    // Private data projection from exact purpose1 incoming terminal owner; no offered subjects.
    pub(super) fn incoming_terminal_platform_state(
        &self,
    ) -> Result<
        (
            KagemushaAppOperationApprovalChallengeV1,
            bool,
            Option<Vec<u8>>,
            bool,
        ),
        KagemushaStateErrorV1,
    > {
        let p = self.incoming_terminal_pending()?;
        if p.captured.is_some() {
            self.captured_incoming_terminal()?
                .recheck_selected_originals_and_current_custody()?;
        } else {
            self.require_live_incoming_terminal(p.selected.challenge.operation_id)?;
        }
        let original = p
            .captured
            .as_ref()
            .map(|(_, _, a)| a.original().to_vec())
            .or_else(|| p.retained.as_ref().map(|(_, a)| a.original().to_vec()));
        Ok((
            p.selected.challenge,
            p.fenced,
            original,
            p.captured.is_some(),
        ))
    }
}
