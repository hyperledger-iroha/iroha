//! Genuine outgoing preparation and platform lifecycle on the retained Native cash owner.
//! Business originals select no State, statement, policy, clock, signer or financial authority.
use super::*;
use iroha_data_model::kagemusha::{
    KagemushaAppOperationApprovalEvidenceV1, KagemushaHardwarePlatformClassV1,
    KagemushaOrdinaryPaymentRequestV1, KagemushaOrdinaryPreparedTransitionV1,
};

/// Exclusive process-owned borrow of one actual Native cash preparation.
/// No constructor, clone, decoding or serialized capability exists. Public projections are data only;
/// monetary proof admission, terminal capture, lineage CAS and outbox release remain separate.
pub struct KagemushaNativeOrdinaryPreparedCashApprovalV1<'a> {
    owner: &'a mut KagemushaNativeOrdinaryCashOwnerV1,
    operation: DigestV1,
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    /// Derive Send State/statement/W from the actual held predecessor and an exact receiver
    /// request. The receiver and any refresh lease are independently authenticated Native holders.
    /// Exact retries reuse the fsynced ciphertext and reserved nonce; incomplete reservation
    /// outcomes remain frozen. A request cannot create balance or authenticate its offered key.
    /// # Errors
    /// Refuses insufficient actual balance, unknown outcomes, stale custody or mixed originals.
    pub fn prepare_send_platform(
        &mut self,
        request_original: &[u8],
        receiver: Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
        receiver_lease: Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
        receiver_counter_floor: Option<u32>,
    ) -> Result<KagemushaNativeOrdinaryPreparedCashApprovalV1<'_>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let request = KagemushaOrdinaryPaymentRequestV1::decode_canonical_exact(request_original)
            .map_err(material)?;
        require_amount(&self.state, request.body.amount)?;
        let operation = if let Some(pending) = self.pending.as_ref() {
            let retained = pending
                .send_credit
                .as_ref()
                .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
            if pending.redeem_credit.is_some()
                || retained.originals.request_original() != request_original
                || retained.originals.receiver_counter_floor() != receiver_counter_floor
                || !retained.originals.matches_receiver(&receiver)?
                || retained.originals.receiver_lease_original()
                    != receiver_lease.as_ref().map(|l| l.original())
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            retained.originals.recheck_originals(
                self,
                pending.operation,
                &retained.successor,
                &receiver,
                receiver_lease.as_deref(),
            )?;
            pending.operation
        } else {
            self.reserve_preparation(KagemushaOperationKindV1::SendSplit)?
        };
        if self
            .pending
            .as_ref()
            .and_then(|p| p.selected.as_ref())
            .is_none()
        {
            let pending = self
                .pending
                .as_ref()
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
            let successor =
                derive_successor(&self.state, pending.nonce, operation, request.body.amount)?;
            let output = self.retain_send_credit(
                operation,
                successor.clone(),
                request_original,
                receiver,
                receiver_lease,
                receiver_counter_floor,
            )?;
            let mut lifecycle = terminal_lifecycle_binding_v1(
                &self.state,
                KagemushaOperationKindV1::SendSplit,
                request.body.request_id,
                request.body.recipient_lane_id,
                output.encrypted_credit_digest,
            );
            lifecycle.credit_id = output.credit_id;
            let lifecycle = lifecycle.canonical_digest().map_err(material)?;
            let statement = self.derive_preparation_statement(
                operation,
                &successor,
                KagemushaTransitionKindV1::SendSplit,
                output.amount,
                lifecycle,
                output.request_digest,
                output.credit_id,
                request.body.recipient_encryption_key,
                output.binding_digest().map_err(material)?,
            )?;
            self.select_native_statement(operation, statement, successor)?;
        }
        self.prepared_cash_approval(operation)
    }

    /// Select only the held account's enrolled redemption beneficiary and real release manifest.
    /// A business amount supplies no State, finality proof, balance, clock or signing subject.
    /// # Errors
    /// Refuses insufficient actual balance, stale custody or an incomplete/different attempt.
    pub fn prepare_redemption_platform(
        &mut self,
        amount: u128,
    ) -> Result<KagemushaNativeOrdinaryPreparedCashApprovalV1<'_>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        require_amount(&self.state, amount)?;
        let operation = if let Some(pending) = self.pending.as_ref() {
            let (_, retained) = pending
                .redeem_credit
                .as_ref()
                .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
            if pending.send_credit.is_some() || retained.output().amount != amount {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            pending.operation
        } else {
            self.reserve_preparation(KagemushaOperationKindV1::RedeemSplit)?
        };
        if self
            .pending
            .as_ref()
            .and_then(|p| p.selected.as_ref())
            .is_none()
        {
            let pending = self
                .pending
                .as_ref()
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
            let successor = derive_successor(&self.state, pending.nonce, operation, amount)?;
            let output = self.retain_redeem_credit(operation, amount, successor.clone())?;
            let statement = self.derive_preparation_statement(
                operation,
                &successor,
                KagemushaTransitionKindV1::RedeemSplit,
                amount,
                output.lifecycle_digest,
                [0; 32],
                [0; 32],
                [0; 32],
                output.binding_digest().map_err(material)?,
            )?;
            self.select_native_statement(operation, statement, successor)?;
        }
        self.prepared_cash_approval(operation)
    }

    /// Borrow only the exact already selected Native operation; an identifier creates no owner.
    /// # Errors
    /// Rejects a different or unselected operation and expired pre-capture W/current custody.
    pub fn prepared_cash_approval(
        &mut self,
        operation: DigestV1,
    ) -> Result<KagemushaNativeOrdinaryPreparedCashApprovalV1<'_>, KagemushaStateErrorV1> {
        let loan = KagemushaNativeOrdinaryPreparedCashApprovalV1 {
            owner: self,
            operation,
        };
        loan.recheck()?;
        Ok(loan)
    }

    /// Select only the retained ticket on this exclusive actual cash owner.
    /// # Errors
    /// Rejects decoded/foreign tickets, changed selection or unavailable current Native custody.
    pub fn prepared_cash_approval_by_ticket(
        &mut self,
        ticket: u64,
    ) -> Result<KagemushaNativeOrdinaryPreparedCashApprovalV1<'_>, KagemushaStateErrorV1> {
        let operation = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .operation;
        let loan = self.prepared_cash_approval(operation)?;
        if loan.ticket()? != ticket {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(loan)
    }

    fn derive_preparation_statement(
        &self,
        operation: DigestV1,
        after: &KagemushaStateV1,
        kind: KagemushaTransitionKindV1,
        amount: u128,
        lifecycle: DigestV1,
        request: DigestV1,
        peer_credit_id: DigestV1,
        recipient_key: DigestV1,
        effect_digest: DigestV1,
    ) -> Result<TransitionProofStatementV1, KagemushaStateErrorV1> {
        let before = &self.state;
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if pending.operation != operation {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let prepared = KagemushaOrdinaryPreparedTransitionV1 {
            version: 1,
            operation: if kind == KagemushaTransitionKindV1::SendSplit {
                2
            } else {
                4
            },
            lifecycle_digest: lifecycle,
            request_digest: request,
            predecessor_state: before.state_commitment,
            successor_state: after.state_commitment,
            amount,
            reservation_digest: pending
                .reservation
                .canonical_commitment()
                .map_err(material)?,
            native_preparation_operation_id: operation,
        };
        let statement = TransitionProofStatementV1 {
            version: 1,
            protocol_version: before.protocol_version,
            predecessor_suite_id: before.suite_id,
            predecessor_vk_digest: before.vk_digest,
            successor_suite_id: after.suite_id,
            successor_vk_digest: after.vk_digest,
            kind,
            amount,
            mint_finality_semantic_digest: [0; 32],
            mint_finality_proof_binding_digest: [0; 32],
            peer_credit_id,
            recipient_encryption_key_binding: recipient_key,
            lifecycle_binding_digest: lifecycle,
            prepared_transition_binding_digest: prepared.binding_digest().map_err(material)?,
            receive_credit_binding_digest: [0; 32],
            predecessor_release_id: before.release_id,
            release_id: after.release_id,
            asset_incarnation: before.asset_incarnation,
            liability_pool_id: before.liability_pool_id,
            hardware_profile_id: before.hardware_profile_id,
            policy_epoch: before.policy_epoch,
            lane: before.lane.clone(),
            predecessor_commitment: before.state_commitment,
            successor_commitment: after.state_commitment,
            predecessor_sequence: before.logical_sequence,
            successor_sequence: after.logical_sequence,
            predecessor_epoch: before.hardware_epoch,
            successor_epoch: after.hardware_epoch,
            predecessor_device_policy_binding: before.device_policy_binding,
            successor_device_policy_binding: after.device_policy_binding,
            predecessor_state_nonce_commitment: before.state_nonce_commitment,
            successor_state_nonce_commitment: after.state_nonce_commitment,
            journal_revision_before: u128::from(self.financial_journal_revision),
            journal_revision_after: u128::from(
                self.financial_journal_revision
                    .checked_add(1)
                    .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?,
            ),
            effect_digest,
        };
        require_outgoing(before, after, &statement, self.financial_journal_revision)?;
        Ok(statement)
    }

    pub(super) fn recheck_native_preparation_derivation(
        &self,
        operation: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let selected = pending
            .selected
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        self.require_native_preparation_derivation(
            operation,
            &selected.statement,
            &selected.successor,
            selected.context,
        )?;
        require_preparation_challenge_window(&selected.challenge)?;
        Ok(())
    }

    pub(super) fn require_native_preparation_derivation(
        &self,
        operation: DigestV1,
        proposed: &TransitionProofStatementV1,
        proposed_successor: &KagemushaStateV1,
        proposed_context: KagemushaGuardContextV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if pending.operation != operation {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let successor = derive_successor(&self.state, pending.nonce, operation, proposed.amount)?;
        let statement = match (&pending.send_credit, &pending.redeem_credit) {
            (Some(send), None) => {
                let request = send.originals.request()?;
                let output = send.originals.output();
                let mut lifecycle = terminal_lifecycle_binding_v1(
                    &self.state,
                    KagemushaOperationKindV1::SendSplit,
                    request.body.request_id,
                    request.body.recipient_lane_id,
                    output.encrypted_credit_digest,
                );
                lifecycle.credit_id = output.credit_id;
                self.derive_preparation_statement(
                    operation,
                    &successor,
                    KagemushaTransitionKindV1::SendSplit,
                    output.amount,
                    lifecycle.canonical_digest().map_err(material)?,
                    output.request_digest,
                    output.credit_id,
                    request.body.recipient_encryption_key,
                    output.binding_digest().map_err(material)?,
                )?
            }
            (None, Some((_, redeem))) => {
                let output = redeem.output();
                self.derive_preparation_statement(
                    operation,
                    &successor,
                    KagemushaTransitionKindV1::RedeemSplit,
                    output.amount,
                    output.lifecycle_digest,
                    [0; 32],
                    [0; 32],
                    [0; 32],
                    output.binding_digest().map_err(material)?,
                )?
            }
            _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
        };
        let release = KagemushaStateProofReleaseV1::from_authenticated_ordinary_release(
            admitted_release(&self.verifier)?.as_ref(),
        )?;
        let context = transition_guard_context(
            release.artifacts,
            &statement,
            pending.preparation_clock.upper_at_ms,
        )?;
        if *proposed_successor != successor || *proposed != statement || proposed_context != context
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    fn select_native_statement(
        &mut self,
        operation: DigestV1,
        statement: TransitionProofStatementV1,
        successor: KagemushaStateV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let release = KagemushaStateProofReleaseV1::from_authenticated_ordinary_release(
            admitted_release(&self.verifier)?.as_ref(),
        )?;
        let context = transition_guard_context(
            release.artifacts,
            &statement,
            pending.preparation_clock.upper_at_ms,
        )?;
        self.select_preparation(operation, statement, successor, context)?;
        Ok(())
    }
}

impl KagemushaNativeOrdinaryPreparedCashApprovalV1<'_> {
    /// Read-only original operation selector; these bytes cannot recreate this Native borrow.
    pub fn operation_id(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(self.operation)
    }
    /// Exact four public fields selected before the platform invocation: ticket, W, scope, S.
    /// # Errors
    /// Rejects stale custody or a different actual preparation; never supplies a raw owner.
    pub fn preparation_fields(&self) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.recheck()?;
        let challenge = self.selected()?.challenge;
        let fields = vec![
            self.ticket()?.to_le_bytes().to_vec(),
            challenge.canonical_signing_bytes().map_err(material)?,
            self.scope()?.to_vec(),
            challenge
                .canonical_subject_signing_bytes()
                .map_err(material)?,
        ];
        self.recheck()?;
        Ok(fields)
    }
    /// The same genuinely enrolled C/credential/possession holder; callers cannot replace it.
    pub fn enrollment(
        &self,
    ) -> Result<&KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1, KagemushaStateErrorV1>
    {
        self.recheck()?;
        Ok(self.owner.publication.cash_financial().enrollment())
    }
    /// Actual independently retained pre-invocation Apple counter floor, never a financial index.
    pub fn previous_app_attest_counter(&self) -> Result<Option<u32>, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(self.selected()?.counter_floor)
    }
    /// Recheck current C/FI/PI/Native clock and exact owner; after capture retain its original instant.
    /// # Errors
    /// Rejects any different pending operation, changed State, stale current custody or invalid W.
    pub fn recheck(&self) -> Result<(), KagemushaStateErrorV1> {
        self.owner.require_current_financial_control()?;
        let pending = self.pending()?;
        self.owner
            .recheck_native_preparation_derivation(self.operation)?;
        if pending.capture.is_some() {
            self.owner
                .captured_preparation()?
                .recheck_selected_originals_and_current_custody()
        } else {
            self.owner.require_live_preparation(self.operation)
        }
    }
    /// Fsync the sole actual platform fence. Unknown fenced outcomes cannot invoke again.
    /// # Errors
    /// Rejects uncertain invocation; a retained original is recovered without another OS call.
    pub fn fence(&mut self) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.recheck()?;
        if self.pending()?.capture.is_some() || self.pending()?.retained.is_some() {
            let mut fields = self.recover()?;
            fields[0][0] += 1;
            return Ok(fields);
        }
        self.owner.fence_preparation_platform(self.operation)?;
        Ok(vec![vec![1], vec![], vec![]])
    }
    /// Wrap exact DER/CBOR in the Native-held W, authenticate and fsync the full original.
    /// # Errors
    /// Rejects foreign evidence, unknown non-fenced input, expired W or changed current custody.
    pub fn retain_platform_original(
        &mut self,
        raw: &[u8],
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.recheck()?;
        if raw.is_empty() || raw.len() > 4096 {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        if self.pending()?.capture.is_some() || self.pending()?.retained.is_some() {
            if self.raw_original()? != raw {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        } else {
            let class = self.enrollment()?.app_credential().subject().platform_class;
            let evidence = match class {
                KagemushaHardwarePlatformClassV1::AndroidKeyMint => {
                    KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                        signature_der: raw.to_vec(),
                    }
                }
                KagemushaHardwarePlatformClassV1::AppleAppAttest => {
                    KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
                        raw_assertion: raw.to_vec(),
                    }
                }
                _ => return Err(KagemushaStateErrorV1::InvalidHardwareProfile),
            };
            let approval = KagemushaAppOperationApprovalV1 {
                challenge: self.selected()?.challenge,
                evidence,
            };
            let original = norito::encode_canonical(&approval).map_err(material)?;
            self.owner
                .capture_preparation_original(self.operation, &original)?;
        }
        self.recheck()?;
        Ok(Sha256::digest(raw).into())
    }
    /// Complete a retained post-fsync live capture and return only its deterministic receipt data.
    /// # Errors
    /// Rejects absent originals, expired pending capture or substituted Native custody.
    pub fn consume(&mut self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.recheck()?;
        if self.pending()?.capture.is_none() {
            self.owner.acknowledge_preparation_capture(self.operation)?;
        }
        let receipt = self.receipt()?;
        self.recheck()?;
        Ok(receipt)
    }
    /// Recover complete original evidence/receipt. A durable unknown fence stays frozen.
    /// # Errors
    /// Refuses unretained fenced outcomes instead of authorizing a second platform signature.
    pub fn recover(&self) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.recheck()?;
        let pending = self.pending()?;
        if pending.capture.is_some() {
            Ok(vec![vec![2], self.raw_original()?, self.receipt()?])
        } else if pending.retained.is_some() {
            Ok(vec![vec![1], self.raw_original()?, vec![]])
        } else if pending.fenced {
            Err(KagemushaStateErrorV1::InvalidCandidateStage)
        } else {
            Ok(vec![vec![0], vec![], vec![]])
        }
    }
    /// Same immutable process scope and exact original W digest; data is not authority.
    pub fn scope_fields(&self) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(vec![
            self.scope()?.to_vec(),
            preparation_message_digest(&self.selected()?.challenge)?.to_vec(),
        ])
    }
    /// Cancel only an uninvoked Native preparation. A lost OS result never resets the fence.
    /// # Errors
    /// Rejects fenced, retained or captured originals.
    pub fn cancel(self) -> Result<(), KagemushaStateErrorV1> {
        self.recheck()?;
        let pending = self.pending()?;
        if pending.fenced || pending.retained.is_some() || pending.capture.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.owner.cancel_preparation(self.operation)
    }
    fn pending(&self) -> Result<&Pending, KagemushaStateErrorV1> {
        let pending = self
            .owner
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if pending.operation != self.operation || pending.selected.is_none() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        Ok(pending)
    }
    fn selected(&self) -> Result<&Selected, KagemushaStateErrorV1> {
        self.pending()?
            .selected
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }
    fn scope(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        let mut hash = Sha256::new();
        hash.update(b"iroha:kagemusha:v1:ordinary-cash-platform-scope\0");
        hash.update(
            self.selected()?
                .challenge
                .canonical_signing_bytes()
                .map_err(material)?,
        );
        hash.update(
            self.owner
                .publication
                .cash_financial()
                .enrollment()
                .certificate()
                .canonical_bytes()
                .map_err(material)?,
        );
        let scope: DigestV1 = hash.finalize().into();
        if scope == [0; 32] {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(scope)
    }
    fn ticket(&self) -> Result<u64, KagemushaStateErrorV1> {
        let ticket = u64::from_le_bytes(self.scope()?[..8].try_into().map_err(material)?);
        if ticket == 0 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(ticket)
    }
    fn approved(&self) -> Result<&KagemushaVerifiedAppOperationApprovalV1, KagemushaStateErrorV1> {
        let pending = self.pending()?;
        pending
            .capture
            .as_ref()
            .or(pending.retained.as_ref())
            .map(|value| &value.2)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }
    fn raw_original(&self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        let original = self.approved()?.original();
        let approval: KagemushaAppOperationApprovalV1 = norito::decode_canonical_with_limits(
            original,
            norito::canonical_decode_limits(original.len()),
        )
        .map_err(material)?;
        if norito::encode_canonical(&approval).map_err(material)? != original {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        if approval.challenge != self.selected()?.challenge {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        match approval.evidence {
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
                Ok(signature_der)
            }
            KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
                Ok(raw_assertion)
            }
        }
    }
    fn receipt(&self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        if self.pending()?.capture.is_none() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.owner
            .captured_preparation()?
            .recheck_selected_originals_and_current_custody()?;
        let challenge = self.selected()?.challenge;
        let mut bytes = b"KGMAPP1\0".to_vec();
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&self.ticket()?.to_le_bytes());
        for digest in [
            self.operation,
            self.scope()?,
            preparation_message_digest(&challenge)?,
            Sha256::digest(self.raw_original()?).into(),
            challenge.enrollment_digest,
        ] {
            bytes.extend_from_slice(&digest);
        }
        let counter = self.approved()?.app_attest_counter();
        bytes.push(u8::from(counter.is_some()));
        bytes.extend_from_slice(&counter.unwrap_or(0).to_le_bytes());
        if bytes.len() != 184 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(bytes)
    }
}

fn preparation_message_digest(
    challenge: &KagemushaAppOperationApprovalChallengeV1,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    Ok(Sha256::digest(challenge.canonical_signing_bytes().map_err(material)?).into())
}

pub(super) fn require_preparation_challenge_window(
    challenge: &KagemushaAppOperationApprovalChallengeV1,
) -> Result<(), KagemushaStateErrorV1> {
    let subject_digest: DigestV1 = Sha256::digest(
        challenge
            .subject
            .canonical_prepare_signing_bytes()
            .map_err(material)?,
    )
    .into();
    if challenge.purpose != KagemushaAppOperationApprovalPurposeV1::PrepareTransition
        || !matches!(
            challenge.subject.operation_kind,
            KagemushaOperationKindV1::SendSplit | KagemushaOperationKindV1::RedeemSplit
        )
        || challenge
            .expires_at_ms
            .checked_sub(challenge.issued_at_ms)
            .is_none_or(|duration| duration == 0 || duration > ORDINARY_PREPARATION_LIFETIME_MS)
        || challenge.subject_signing_digest != subject_digest
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    challenge.canonical_signing_bytes().map_err(material)?;
    Ok(())
}

fn require_amount(before: &KagemushaStateV1, amount: u128) -> Result<(), KagemushaStateErrorV1> {
    if amount == 0 {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    }
    before
        .balance
        .checked_sub(amount)
        .ok_or(KagemushaStateErrorV1::InsufficientBalance)?;
    Ok(())
}
fn derive_successor(
    before: &KagemushaStateV1,
    nonce: DigestV1,
    operation: DigestV1,
    amount: u128,
) -> Result<KagemushaStateV1, KagemushaStateErrorV1> {
    require_amount(before, amount)?;
    if nonce == [0; 32] || operation == [0; 32] {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:ordinary-cash-successor-nonce\0");
    hash.update(before.state_commitment);
    hash.update(operation);
    hash.update(nonce);
    let successor_nonce: DigestV1 = hash.finalize().into();
    if successor_nonce == [0; 32] || successor_nonce == before.state_nonce_commitment {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    KagemushaStateV1::build(
        before.context(),
        before.liability_pool_id,
        before.lane.clone(),
        before.balance - amount,
        before
            .logical_sequence
            .checked_add(1)
            .ok_or(KagemushaStateErrorV1::SequenceOverflow)?,
        before
            .secure_index
            .checked_add(1)
            .ok_or(KagemushaStateErrorV1::SequenceOverflow)?,
        before.hardware_epoch,
        before.device_policy_binding,
        successor_nonce,
        before.consumed_credit_root,
    )
}

#[cfg(test)]
#[path = "ordinary_cash_platform_preparation_tests.rs"]
mod tests;
