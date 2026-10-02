//! Separately fresh purpose2 incoming approval, owned by the sole Main WAL.
//! Source proofs and decoded selectors do not authorize a platform call or install a balance.
use super::incoming::{IncomingIntentOriginals, SourceLocator};
use super::*;
#[path = "ordinary_incoming_capacity.rs"]
mod capacity;
use crate::kagemusha_v1_state::ordinary_incoming_preview::{
    OrdinaryIncomingMathSourceV1, OrdinaryIncomingPreviewV1, derive_ordinary_incoming_preview_v1,
};
use crate::kagemusha_v1_state::sparse_merkle::PreparedConsumedCreditInsertV1;
pub(super) use capacity::{
    INCOMING_COMPLETION_MAIN_ROWS_V1, mint_incoming_prepared_byte_budget_v1,
    require_prepared_frame_quota_v1, require_recorded_allowance_v1,
};
use iroha_data_model::kagemusha::{
    KagemushaMintCreditV1, KagemushaOrdinaryIncomingPreparationV1,
    KagemushaOrdinaryIncomingReservationV1,
};

/// Full immutable proof data plus the separately acknowledged incoming FI/time/entropy.
/// Original source FI/clock is retained independently in Main's source attempt.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryIncomingPreparedOriginalsV1")]
pub(super) struct IncomingPreparedOriginals {
    reservation: KagemushaOrdinaryIncomingReservationV1,
    mint_originals: Option<(Vec<u8>, Vec<u8>)>,
    financial_control: CapturedFinancialControlIdentity,
    clock: KagemushaOrdinaryCashClockContextV1,
    nonce: DigestV1,
    successor_nonce: DigestV1,
    preparation: KagemushaOrdinaryIncomingPreparationV1,
    statement: TransitionProofStatementV1,
    successor: KagemushaStateV1,
    normalized: KagemushaNormalizedGuardStatementV1,
    context: KagemushaGuardContextV1,
    challenge: KagemushaAppOperationApprovalChallengeV1,
    lease_original: Option<Vec<u8>>,
    previous_counter: Option<u32>,
    private_checkpoint_maximum_bytes: u64,
    maximum_record_payload_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryIncomingApprovalRecordV1")]
pub(super) enum IncomingApprovalRecord {
    Prepared(IncomingPreparedOriginals),
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
    Capture {
        operation: DigestV1,
        clock: KagemushaOrdinaryCashClockContextV1,
        authorization: DigestV1,
    },
}

pub(super) struct PendingIncomingApproval {
    originals: IncomingPreparedOriginals,
    selected: Selected,
    replay_insert: PreparedConsumedCreditInsertV1,
    fenced: bool,
    retained: Option<(
        KagemushaOrdinaryCashClockContextV1,
        KagemushaVerifiedAppOperationApprovalV1,
    )>,
    // Retain both authenticated cuts: capture cannot erase the original admission clock.
    capture: Option<(
        KagemushaOrdinaryCashClockContextV1,
        KagemushaOrdinaryCashClockContextV1,
        KagemushaVerifiedAppOperationApprovalV1,
    )>,
}

impl PendingIncomingApproval {
    /// Count the complete captured W2 suffix; the Prepared frame is separately reserved.
    pub(super) fn captured_suffix_charge_bytes(&self) -> Result<u64, KagemushaStateErrorV1> {
        // Capture moves the retained Original into this tuple; the pre-capture slot is empty.
        let (clock, captured_clock, original) = self
            .capture
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let operation = self.selected.challenge.operation_id;
        let records = [
            IncomingApprovalRecord::PlatformFence { operation },
            IncomingApprovalRecord::Original {
                operation,
                clock: *clock,
                original: original.original().to_vec(),
                authorization: [0; 32],
                accepted_counter: original.app_attest_counter(),
            },
            IncomingApprovalRecord::Capture {
                operation,
                clock: *captured_clock,
                authorization: [0; 32],
            },
        ];
        super::incoming_state_commit::checked_record_capacity_charge(
            records.into_iter().map(Record::IncomingApproval),
            self.originals.maximum_record_payload_bytes,
        )
    }
    pub(super) fn capacity_charge_bytes(&self) -> Result<u64, KagemushaStateErrorV1> {
        capacity::incoming_prepared_capacity_v1(&self.originals)
    }
    pub(super) fn completion_capacity_bytes(&self) -> Result<u64, KagemushaStateErrorV1> {
        capacity::incoming_completion_capacity_v1(&self.originals)
    }
    // Only this separately retained incoming approval supplies its proving FI identity.
    // The earlier source selection and outgoing approval remain distinct private operands.
    pub(super) fn proving_financial_control_identity(&self) -> CapturedFinancialControlIdentity {
        self.originals.financial_control
    }
}

/// A distinct genuine incoming proof loan. No decoded field, outgoing approval or Mint
/// credential creates it. Every use retains the actual Main prefix and fresh incoming FI.
pub(crate) struct KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'a> {
    owner: &'a KagemushaNativeOrdinaryCashOwnerV1,
    prefix: KagemushaRecoveryJournalPrefixV1,
}

impl IncomingPreparedOriginals {
    fn derive(
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        intent: &IncomingIntentOriginals,
        reservation: &KagemushaOrdinaryIncomingReservationV1,
        mint: Option<(&[u8], &[u8])>,
        fi: CapturedFinancialControlIdentity,
        clock: KagemushaOrdinaryCashClockContextV1,
        nonce: DigestV1,
        successor_nonce: DigestV1,
    ) -> Result<(OrdinaryIncomingPreviewV1, PreparedConsumedCreditInsertV1), KagemushaStateErrorV1>
    {
        intent.recheck_historical(owner)?;
        if reservation.selection != intent.selection
            || nonce == [0; 32]
            || successor_nonce == [0; 32]
            || successor_nonce == owner.state.state_nonce_commitment
            || nonce == successor_nonce
            || clock.lower_at_ms < fi.lower_ms
            || clock.upper_at_ms < fi.upper_ms
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        owner
            .control
            .recheck_retained_capture_identity(
                owner.publication.cash_financial(),
                fi.original_sha256,
                fi.lower_ms,
                fi.upper_ms,
            )
            .map_err(material)?;
        let f = owner.publication.cash_financial();
        let (fi_issued, fi_expires) = owner
            .control
            .recheck_retained_capture_original_window(
                f,
                fi.original_sha256,
                fi.lower_ms,
                fi.upper_ms,
            )
            .map_err(material)?;
        if clock.lower_at_ms < fi_issued || clock.upper_at_ms >= fi_expires {
            return Err(KagemushaStateErrorV1::InvalidTrustedCommitTime);
        }
        let signed_clock = f
            .verified_retained_cash_clock_originals(&clock)
            .map_err(material)?;
        let insertion = owner.incoming_consumed.prepare_insert(
            CreditIdV1(intent.selection.credit_id),
            reservation.digest().map_err(material)?,
        )?;
        let preview = match (intent.source, mint) {
            (SourceLocator::Mint, Some((finalized, credit_raw))) => {
                let authorization = owner
                    .readmit_selected_mint_request(owner.retained_predebit_request_original()?)?;
                let old = owner.retained_mint_source_control_identity()?;
                let capture = owner
                    .control
                    .borrow_captured_proof_decision(
                        f,
                        old.original_sha256,
                        old.lower_ms,
                        old.upper_ms,
                    )
                    .map_err(material)?;
                let source = f
                    .authenticate_finalized_mint_source(&capture, authorization, finalized)
                    .map_err(material)?;
                let credit: KagemushaMintCreditV1 = decode_credit(credit_raw)?;
                derive_ordinary_incoming_preview_v1(
                    &owner.verifier,
                    &owner.state,
                    &owner.public_state_original,
                    f.enrollment().app_credential(),
                    reservation,
                    OrdinaryIncomingMathSourceV1::Mint {
                        source: &source,
                        credit: &credit,
                    },
                    insertion.witness(),
                    successor_nonce,
                    owner.financial_journal_revision,
                    fi.original_sha256,
                    &clock,
                    &signed_clock,
                    nonce,
                )?
            }
            (SourceLocator::Receive { request_id }, None) => {
                let (_, source) = owner.retained_incoming_received_source(request_id)?;
                derive_ordinary_incoming_preview_v1(
                    &owner.verifier,
                    &owner.state,
                    &owner.public_state_original,
                    f.enrollment().app_credential(),
                    reservation,
                    OrdinaryIncomingMathSourceV1::Receive(source),
                    insertion.witness(),
                    successor_nonce,
                    owner.financial_journal_revision,
                    fi.original_sha256,
                    &clock,
                    &signed_clock,
                    nonce,
                )?
            }
            _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
        };
        Ok((preview, insertion))
    }
    fn rederive(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        intent: &IncomingIntentOriginals,
    ) -> Result<PreparedConsumedCreditInsertV1, KagemushaStateErrorV1> {
        if self.maximum_record_payload_bytes != owner.maximum_record_payload_bytes {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        if self.private_checkpoint_maximum_bytes != u64::try_from(crate::kagemusha_v1_recursion::KagemushaRecursiveStateCheckpointV1::maximum_encoded_bytes(&owner.verifier).map_err(material)?).map_err(material)? {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        let mint = self
            .mint_originals
            .as_ref()
            .map(|(a, b)| (a.as_slice(), b.as_slice()));
        let (p, insertion) = Self::derive(
            owner,
            intent,
            &self.reservation,
            mint,
            self.financial_control,
            self.clock,
            self.nonce,
            self.successor_nonce,
        )?;
        if p.preparation != self.preparation
            || p.statement != self.statement
            || p.successor != self.successor
            || p.normalized != self.normalized
            || p.guard_context != self.context
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.challenge.canonical_signing_bytes().map_err(material)?;
        let c = owner
            .publication
            .cash_financial()
            .enrollment()
            .app_credential();
        let (_, fi_expires) = owner
            .control
            .recheck_retained_capture_original_window(
                owner.publication.cash_financial(),
                self.financial_control.original_sha256,
                self.financial_control.lower_ms,
                self.financial_control.upper_ms,
            )
            .map_err(material)?;
        if self.challenge.expires_at_ms > fi_expires
            || self.challenge.issued_at_ms != self.clock.lower_at_ms
            || self.clock.upper_at_ms >= self.challenge.expires_at_ms
            || self.challenge
                != incoming_challenge(
                    owner,
                    &p,
                    self.challenge.issued_at_ms,
                    self.challenge.expires_at_ms,
                )?
            || self.challenge.enrollment_digest != c.digest()
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(insertion)
    }
}

fn decode_credit(raw: &[u8]) -> Result<KagemushaMintCreditV1, KagemushaStateErrorV1> {
    if raw.is_empty() || raw.len() > iroha_data_model::kagemusha::KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let c = norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
        .map_err(material)?;
    if norito::encode_canonical(&c).map_err(material)? != raw {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(c)
}
fn incoming_challenge(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    p: &OrdinaryIncomingPreviewV1,
    issued: u64,
    expires: u64,
) -> Result<KagemushaAppOperationApprovalChallengeV1, KagemushaStateErrorV1> {
    let c = owner
        .publication
        .cash_financial()
        .enrollment()
        .app_credential();
    if issued < c.subject().issued_at_ms
        || expires <= issued
        || expires
            > issued
                .checked_add(ORDINARY_PREPARATION_LIFETIME_MS)
                .ok_or(KagemushaStateErrorV1::InvalidTrustedCommitTime)?
        || expires > c.subject().expires_at_ms
    {
        return Err(KagemushaStateErrorV1::InvalidTrustedCommitTime);
    }
    let mut subject = preparation_subject(&owner.state, &p.successor, &p.statement, c)?;
    // The separate subject helper admits only incoming kinds; no outgoing terminal body exists.
    subject.operation_kind = match p.statement.kind {
        KagemushaTransitionKindV1::MintFold => KagemushaOperationKindV1::MintFold,
        KagemushaTransitionKindV1::ReceiveFold => KagemushaOperationKindV1::ReceiveFold,
        _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
    };
    let challenge = KagemushaAppOperationApprovalChallengeV1 {
        version: 1,
        purpose: KagemushaAppOperationApprovalPurposeV1::PrepareTransition,
        operation_id: p.preparation.operation_id,
        nonce: p.preparation.nonce,
        account_binding: c.subject().account_binding,
        authority_policy_digest: c.subject().app_authority_policy_digest,
        attested_key_id: c.subject().attested_key_id,
        enrollment_digest: c.digest(),
        subject_signing_digest: Sha256::digest(
            subject
                .canonical_prepare_signing_bytes()
                .map_err(material)?,
        )
        .into(),
        normalized_guard_digest: p.normalized.canonical_digest().map_err(material)?,
        issued_at_ms: issued,
        expires_at_ms: expires,
        subject,
    };
    challenge.canonical_signing_bytes().map_err(material)?;
    Ok(challenge)
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    /// Select only the held finalized Mint source plus genuine independent MintAuthority proofs.
    /// A data response must pass source admission and both proof parities before this WAL write.
    pub(crate) fn prepare_incoming_mint_approval(
        &mut self,
        finalized: &[u8],
        credit: &[u8],
    ) -> Result<KagemushaAppOperationApprovalChallengeV1, KagemushaStateErrorV1> {
        self.prepare_incoming_approval(Some((finalized, credit)))
    }
    /// A received source was already proof/finality/AEAD admitted and durably retained by Main.
    pub(crate) fn prepare_incoming_receive_approval(
        &mut self,
    ) -> Result<KagemushaAppOperationApprovalChallengeV1, KagemushaStateErrorV1> {
        self.prepare_incoming_approval(None)
    }
    fn prepare_incoming_approval(
        &mut self,
        mint: Option<(&[u8], &[u8])>,
    ) -> Result<KagemushaAppOperationApprovalChallengeV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let pending = self
            .pending_incoming
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        pending.intent.recheck_historical(self)?;
        if let Some(p) = &pending.approval {
            if p.originals
                .mint_originals
                .as_ref()
                .map(|(a, b)| (a.as_slice(), b.as_slice()))
                != mint
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            self.require_live_incoming_approval()?;
            return Ok(p.selected.challenge);
        }
        self.require_incoming_completion_rows()?;
        let intent = pending.intent.clone();
        if mint.is_some_and(|(a, b)| {
            a.is_empty()
                || b.is_empty()
                || a.len()
                    > iroha_data_model::kagemusha::KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1
                || b.len() > iroha_data_model::kagemusha::KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1
        }) {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let (fi, clock) = self.capture_incoming_selection_control()?;
        let mut entropy = zeroize::Zeroizing::new([0u8; 64]);
        OsRng.try_fill_bytes(entropy.as_mut()).map_err(material)?;
        let nonce: DigestV1 = entropy[..32].try_into().map_err(material)?;
        let mut h = Sha256::new();
        h.update(b"iroha:kagemusha:v1:ordinary-incoming-successor-nonce\0");
        h.update(self.state.state_commitment);
        h.update(intent.selection.operation_id);
        h.update(&entropy[32..]);
        let successor_nonce: DigestV1 = h.finalize().into();
        let (finalized_sha, proof_sha, semantic) = match (intent.source, mint) {
            (SourceLocator::Mint, Some((finalized, credit_raw))) => {
                let f = self.publication.cash_financial();
                let old = self.retained_mint_source_control_identity()?;
                let captured = self
                    .control
                    .borrow_captured_proof_decision(
                        f,
                        old.original_sha256,
                        old.lower_ms,
                        old.upper_ms,
                    )
                    .map_err(material)?;
                let auth =
                    self.readmit_selected_mint_request(self.retained_predebit_request_original()?)?;
                let source = f
                    .authenticate_finalized_mint_source(&captured, auth, finalized)
                    .map_err(material)?;
                (
                    Sha256::digest(source.finalized_original().map_err(material)?).into(),
                    Sha256::digest(credit_raw).into(),
                    source.source_semantic_digest().map_err(material)?,
                )
            }
            (SourceLocator::Receive { request_id }, None) => {
                let (_, source) = self.retained_incoming_received_source(request_id)?;
                (
                    source.received_assertion_original_sha256(),
                    Sha256::digest(source.outgoing_original()).into(),
                    source.output().binding_digest().map_err(material)?,
                )
            }
            _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
        };
        let reservation = KagemushaOrdinaryIncomingReservationV1 {
            selection: intent.selection.clone(),
            finalized_source_original_sha256: finalized_sha,
            source_proof_original_sha256: proof_sha,
            source_semantic_digest: semantic,
        };
        let (preview, replay_insert) = IncomingPreparedOriginals::derive(
            self,
            &intent,
            &reservation,
            mint,
            fi,
            clock,
            nonce,
            successor_nonce,
        )?;
        let floor = self.credential_floor()?;
        let (_, fi_expires) = self
            .control
            .recheck_retained_capture_original_window(
                self.publication.cash_financial(),
                fi.original_sha256,
                fi.lower_ms,
                fi.upper_ms,
            )
            .map_err(material)?;
        let issued = clock.lower_at_ms;
        let expires = issued
            .checked_add(ORDINARY_PREPARATION_LIFETIME_MS)
            .ok_or(KagemushaStateErrorV1::InvalidTrustedCommitTime)?
            .min(floor.approval_valid_until_ms())
            .min(fi_expires);
        self.publication
            .cash_financial()
            .trusted_time_interval()
            .map_err(material)?
            .require_validity(issued, expires)
            .map_err(material)?;
        let challenge = incoming_challenge(self, &preview, issued, expires)?;
        let lease = self
            .publication
            .cash_financial()
            .retained_integrity_lease()
            .cloned();
        let originals = IncomingPreparedOriginals {
            reservation,
            mint_originals: mint.map(|(a, b)| (a.to_vec(), b.to_vec())),
            financial_control: fi,
            clock,
            nonce,
            successor_nonce,
            preparation: preview.preparation,
            statement: preview.statement.clone(),
            successor: preview.successor.clone(),
            normalized: preview.normalized.clone(),
            context: preview.guard_context,
            challenge,
            lease_original: lease.as_ref().map(|l| l.original().to_vec()),
            previous_counter: self.counter_floor,
            maximum_record_payload_bytes: self.maximum_record_payload_bytes,
            private_checkpoint_maximum_bytes: u64::try_from(crate::kagemusha_v1_recursion::KagemushaRecursiveStateCheckpointV1::maximum_encoded_bytes(&self.verifier).map_err(material)?).map_err(material)?,
        };
        let selected = Selected {
            statement: preview.statement,
            successor: preview.successor,
            normalized: preview.normalized,
            context: preview.guard_context,
            challenge,
            lease,
            counter_floor: self.counter_floor,
        };
        let record = Record::IncomingApproval(IncomingApprovalRecord::Prepared(originals.clone()));
        capacity::require_prepared_record_capacity_v1(self, &record)?;
        self.persist(&record)?;
        self.pending_incoming
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .approval = Some(PendingIncomingApproval {
            originals,
            selected,
            replay_insert,
            fenced: false,
            retained: None,
            capture: None,
        });
        self.require_live_incoming_approval()?;
        Ok(challenge)
    }
    fn pending_incoming_approval(&self) -> Result<&PendingIncomingApproval, KagemushaStateErrorV1> {
        self.pending_incoming
            .as_ref()
            .and_then(|p| p.approval.as_ref())
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }
    fn require_live_incoming_approval(&self) -> Result<(), KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let p = self.pending_incoming_approval()?;
        p.originals.rederive(
            self,
            &self
                .pending_incoming
                .as_ref()
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                .intent,
        )?;
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
    /// Persist the invocation fence before returning. No retry can invoke the app key again.
    pub(crate) fn fence_incoming_approval_platform(&mut self) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_incoming_approval()?;
        let p = self.pending_incoming_approval()?;
        if p.fenced {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let operation = p.selected.challenge.operation_id;
        self.persist(&Record::IncomingApproval(
            IncomingApprovalRecord::PlatformFence { operation },
        ))?;
        self.pending_incoming
            .as_mut()
            .and_then(|p| p.approval.as_mut())
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .fenced = true;
        self.require_live_incoming_approval()
    }
    /// Complete exact raw platform original, independently authenticated under the selected C/PI.
    pub(crate) fn capture_incoming_approval_original(
        &mut self,
        raw: &[u8],
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_incoming_approval()?;
        let p = self.pending_incoming_approval()?;
        if !p.fenced {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        if let Some(a) = p
            .retained
            .as_ref()
            .map(|(_, a)| a)
            .or_else(|| p.capture.as_ref().map(|(_, _, a)| a))
        {
            if a.original() != raw {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            return if p.capture.is_some() {
                self.captured_incoming_approval()?
                    .recheck_selected_originals_and_current_custody()
            } else {
                self.acknowledge_incoming_approval_capture()
            };
        }
        let clock = self
            .publication
            .cash_financial()
            .current_cash_clock_context()
            .map_err(material)?;
        let a = authenticate_incoming_approval(self, p, raw, &clock)?;
        let digest = authorization(&a, p.selected.lease.as_deref())?;
        let operation = p.selected.challenge.operation_id;
        let counter = a.app_attest_counter();
        self.persist(&Record::IncomingApproval(
            IncomingApprovalRecord::Original {
                operation,
                clock,
                original: raw.to_vec(),
                authorization: digest,
                accepted_counter: counter,
            },
        ))?;
        self.counter_floor = counter.or(self.counter_floor);
        self.pending_incoming
            .as_mut()
            .and_then(|p| p.approval.as_mut())
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .retained = Some((clock, a));
        self.acknowledge_incoming_approval_capture()
    }
    pub(crate) fn acknowledge_incoming_approval_capture(
        &mut self,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_incoming_approval()?;
        let p = self.pending_incoming_approval()?;
        if p.capture.is_some() {
            return self
                .captured_incoming_approval()?
                .recheck_selected_originals_and_current_custody();
        }
        let (original_clock, a) = p
            .retained
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let clock = self
            .publication
            .cash_financial()
            .current_cash_clock_context()
            .map_err(material)?;
        require_incoming_approval_capture_clock(self, p, original_clock, &clock)?;
        a.recheck_at_trusted_time(clock.lower_at_ms)
            .map_err(material)?;
        a.recheck_at_trusted_time(clock.upper_at_ms)
            .map_err(material)?;
        let operation = p.selected.challenge.operation_id;
        let digest = authorization(a, p.selected.lease.as_deref())?;
        self.persist(&Record::IncomingApproval(IncomingApprovalRecord::Capture {
            operation,
            clock,
            authorization: digest,
        }))?;
        let p = self
            .pending_incoming
            .as_mut()
            .and_then(|p| p.approval.as_mut())
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        super::incoming_state_commit::install_captured_approval(
            &mut p.retained,
            &mut p.capture,
            clock,
        )?;
        self.captured_incoming_approval()?
            .recheck_selected_originals_and_current_custody()
    }
    pub(crate) fn captured_incoming_approval(
        &self,
    ) -> Result<KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>, KagemushaStateErrorV1>
    {
        let loan = KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1 {
            owner: self,
            prefix: self.prefix,
        };
        loan.recheck_selected_originals_and_current_custody()?;
        Ok(loan)
    }
    pub(super) fn replay_incoming_approval(
        &mut self,
        r: IncomingApprovalRecord,
        leases: &[Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>],
    ) -> Result<(), KagemushaStateErrorV1> {
        match r {
            IncomingApprovalRecord::Prepared(originals) => {
                let p = self
                    .pending_incoming
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if p.approval.is_some() || originals.previous_counter != self.counter_floor {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                let record =
                    Record::IncomingApproval(IncomingApprovalRecord::Prepared(originals.clone()));
                capacity::require_prepared_record_capacity_v1(self, &record)?;
                let replay_insert = originals.rederive(self, &p.intent)?;
                let lease = match &originals.lease_original {
                    None => None,
                    Some(raw) => Some(Arc::clone(
                        leases
                            .iter()
                            .find(|l| l.original() == raw)
                            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
                    )),
                };
                let selected = Selected {
                    statement: originals.statement.clone(),
                    successor: originals.successor.clone(),
                    normalized: originals.normalized.clone(),
                    context: originals.context,
                    challenge: originals.challenge,
                    lease,
                    counter_floor: originals.previous_counter,
                };
                self.pending_incoming
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .approval = Some(PendingIncomingApproval {
                    originals,
                    selected,
                    replay_insert,
                    fenced: false,
                    retained: None,
                    capture: None,
                });
            }
            IncomingApprovalRecord::PlatformFence { operation } => {
                let p = self
                    .pending_incoming
                    .as_mut()
                    .and_then(|p| p.approval.as_mut())
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if p.fenced || p.selected.challenge.operation_id != operation {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                p.fenced = true;
            }
            IncomingApprovalRecord::Original {
                operation,
                clock,
                original,
                authorization: digest,
                accepted_counter,
            } => {
                let p = self.pending_incoming_approval()?;
                if !p.fenced
                    || p.retained.is_some()
                    || p.capture.is_some()
                    || p.selected.challenge.operation_id != operation
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                let a = authenticate_incoming_approval(self, p, &original, &clock)?;
                if a.app_attest_counter() != accepted_counter
                    || authorization(&a, p.selected.lease.as_deref())? != digest
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                self.counter_floor = accepted_counter.or(self.counter_floor);
                self.pending_incoming
                    .as_mut()
                    .and_then(|p| p.approval.as_mut())
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .retained = Some((clock, a));
            }
            IncomingApprovalRecord::Capture {
                operation,
                clock,
                authorization: digest,
            } => {
                let p = self.pending_incoming_approval()?;
                let (original_clock, a) = p
                    .retained
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if p.capture.is_some()
                    || !p.fenced
                    || p.selected.challenge.operation_id != operation
                    || authorization(a, p.selected.lease.as_deref())? != digest
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                require_incoming_approval_capture_clock(self, p, original_clock, &clock)?;
                a.recheck_at_trusted_time(clock.lower_at_ms)
                    .map_err(material)?;
                a.recheck_at_trusted_time(clock.upper_at_ms)
                    .map_err(material)?;
                let p = self
                    .pending_incoming
                    .as_mut()
                    .and_then(|p| p.approval.as_mut())
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                super::incoming_state_commit::install_captured_approval(
                    &mut p.retained,
                    &mut p.capture,
                    clock,
                )?;
            }
        }
        Ok(())
    }
}

/// Arithmetic correlation only; the private owner authenticates full signed originals below.
fn require_incoming_approval_clock_interval(
    preparation_clock: &KagemushaOrdinaryCashClockContextV1,
    clock: &KagemushaOrdinaryCashClockContextV1,
    issued_at_ms: u64,
    expires_at_ms: u64,
) -> Result<(), KagemushaStateErrorV1> {
    preparation_clock
        .validate_within_original_window(issued_at_ms, expires_at_ms)
        .map_err(material)?;
    clock
        .validate_within_original_window(issued_at_ms, expires_at_ms)
        .map_err(material)?;
    require_incoming_approval_clock_nonregression(preparation_clock, clock)
}
fn require_incoming_approval_clock_nonregression(
    previous: &KagemushaOrdinaryCashClockContextV1,
    clock: &KagemushaOrdinaryCashClockContextV1,
) -> Result<(), KagemushaStateErrorV1> {
    if clock.lower_at_ms < previous.lower_at_ms || clock.upper_at_ms < previous.upper_at_ms {
        return Err(KagemushaStateErrorV1::SnapshotRollback);
    }
    Ok(())
}
fn require_incoming_approval_clock(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    p: &PendingIncomingApproval,
    clock: &KagemushaOrdinaryCashClockContextV1,
) -> Result<(), KagemushaStateErrorV1> {
    require_incoming_approval_clock_interval(
        &p.originals.clock,
        clock,
        p.selected.challenge.issued_at_ms,
        p.selected.challenge.expires_at_ms,
    )?;
    let signed_clock = owner
        .publication
        .cash_financial()
        .verified_retained_cash_clock_originals(clock)
        .map_err(material)?;
    signed_clock.recheck_cash_context(clock).map_err(material)
}
fn require_incoming_approval_capture_clock(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    p: &PendingIncomingApproval,
    original_clock: &KagemushaOrdinaryCashClockContextV1,
    capture_clock: &KagemushaOrdinaryCashClockContextV1,
) -> Result<(), KagemushaStateErrorV1> {
    // Retain and re-admit both full cuts, including after cold replay and before every loan.
    require_incoming_approval_clock(owner, p, original_clock)?;
    require_incoming_approval_clock(owner, p, capture_clock)?;
    require_incoming_approval_clock_nonregression(original_clock, capture_clock)
}
fn authenticate_incoming_approval(
    owner: &KagemushaNativeOrdinaryCashOwnerV1,
    p: &PendingIncomingApproval,
    original: &[u8],
    clock: &KagemushaOrdinaryCashClockContextV1,
) -> Result<KagemushaVerifiedAppOperationApprovalV1, KagemushaStateErrorV1> {
    require_incoming_approval_clock(owner, p, clock)?;
    let approval = owner.authenticate(original, &p.selected, clock.lower_at_ms)?;
    owner.authenticate(original, &p.selected, clock.upper_at_ms)?;
    Ok(approval)
}

impl KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_> {
    fn pending(&self) -> Result<&PendingIncomingApproval, KagemushaStateErrorV1> {
        self.owner.pending_incoming_approval()
    }
    pub(crate) fn recheck_selected_originals_and_current_custody(
        &self,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.owner
            .recheck_proving_history(ProvingHistoryOperation::IncomingApproval)?;
        if self.owner.prefix != self.prefix {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let pending = self
            .owner
            .pending_incoming
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let p = self.pending()?;
        let (original_clock, capture_clock, a) = p
            .capture
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if !p.fenced
            || a.challenge() != &p.selected.challenge
            || p.originals.rederive(self.owner, &pending.intent)? != p.replay_insert
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        require_incoming_approval_capture_clock(self.owner, p, original_clock, capture_clock)?;
        for clock in [original_clock, capture_clock] {
            a.recheck_at_trusted_time(clock.lower_at_ms)
                .map_err(material)?;
            a.recheck_at_trusted_time(clock.upper_at_ms)
                .map_err(material)?;
        }
        Ok(())
    }
    pub(crate) fn with_retained_predecessor_checkpoint(
        &self,
        consume: &mut dyn for<'a> FnMut(
            &'a crate::kagemusha_v1_recursion::KagemushaGeneratedRecursiveStateProofV1,
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> core::result::Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        self.owner.with_retained_predecessor_checkpoint(consume)?;
        self.recheck_selected_originals_and_current_custody()
    }

    pub(crate) fn transition_statement(
        &self,
    ) -> Result<&TransitionProofStatementV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.selected.statement)
    }
    pub(crate) fn selected_predecessor_state(
        &self,
    ) -> Result<&KagemushaStateV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.owner.state)
    }
    pub(crate) fn selected_successor_state(
        &self,
    ) -> Result<&KagemushaStateV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.selected.successor)
    }
    pub(crate) fn predecessor_public_state_original(&self) -> Result<&[u8], KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.owner.public_state_original)
    }
    pub(crate) fn normalized_guard_statement(
        &self,
    ) -> Result<&KagemushaNormalizedGuardStatementV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.selected.normalized)
    }
    pub(crate) fn normalized_guard_context(
        &self,
    ) -> Result<&KagemushaGuardContextV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.selected.context)
    }
    pub(crate) fn challenge(
        &self,
    ) -> Result<&KagemushaAppOperationApprovalChallengeV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.selected.challenge)
    }
    pub(crate) fn approval(
        &self,
    ) -> Result<&KagemushaVerifiedAppOperationApprovalV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self
            .pending()?
            .capture
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .2)
    }
    pub(crate) fn original(&self) -> Result<&[u8], KagemushaStateErrorV1> {
        Ok(self.approval()?.original())
    }
    pub(crate) fn enrollment(
        &self,
    ) -> Result<&KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1, KagemushaStateErrorV1>
    {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(self.owner.publication.cash_financial().enrollment())
    }
    pub(crate) fn credential(
        &self,
    ) -> Result<&KagemushaVerifiedOrdinaryAppCredentialV1, KagemushaStateErrorV1> {
        Ok(self.enrollment()?.app_credential())
    }
    pub(crate) fn authenticated_release(
        &self,
    ) -> Result<Arc<KagemushaAuthenticatedReleaseV1>, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        admitted_release(&self.owner.verifier)
    }
    pub(crate) fn recursive_verifier(&self) -> &KagemushaAuthenticatedRecursiveVerifierV1 {
        &self.owner.verifier
    }
    pub(crate) fn authorization_binding_digest(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        authorization(self.approval()?, self.pending()?.selected.lease.as_deref())
    }
    pub(crate) fn selected_integrity_lease(
        &self,
    ) -> Result<Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(self.pending()?.selected.lease.as_deref())
    }
    pub(crate) fn previous_app_attest_counter(&self) -> Result<Option<u32>, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(self.pending()?.selected.counter_floor)
    }
    pub(crate) fn approval_admission_interval_ms(
        &self,
    ) -> Result<(u64, u64), KagemushaStateErrorV1> {
        let clock = self.approval_admission_clock_context()?;
        Ok((clock.lower_at_ms, clock.upper_at_ms))
    }
    /// Exact capture clock data from this private selection; offered scalars cannot create it.
    pub(crate) fn approval_admission_clock_context(
        &self,
    ) -> Result<&KagemushaOrdinaryCashClockContextV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self
            .pending()?
            .capture
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .1)
    }
    /// Lend the same authentic signed capture clock, without granting a current elapsed clock.
    pub(crate) fn with_verified_approval_clock(
        &self,
        visitor: &mut dyn for<'clock> FnMut(
            &'clock KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let clock = self
            .owner
            .publication
            .cash_financial()
            .verified_retained_cash_clock_originals(self.approval_admission_clock_context()?)
            .map_err(material)?;
        visitor(&clock)?;
        self.recheck_selected_originals_and_current_custody()
    }
    pub(crate) fn reservation(
        &self,
    ) -> Result<&KagemushaOrdinaryIncomingReservationV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.originals.reservation)
    }
    pub(crate) fn preparation(
        &self,
    ) -> Result<&KagemushaOrdinaryIncomingPreparationV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.originals.preparation)
    }
    pub(crate) fn replay_witness(
        &self,
    ) -> Result<&ConsumedCreditInsertWitnessV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(self.pending()?.replay_insert.witness())
    }
    pub(crate) fn financial_control_original(&self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let i = self.pending()?.originals.financial_control;
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
    pub(crate) fn transport_semantic_digest(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let p = self.pending()?;
        let statement = &p.selected.statement;
        Ok(local_transition_transport_digest(
            statement.kind,
            statement.release_id,
            statement.liability_pool_id,
            statement.effect_digest,
            self.owner.state.state_commitment,
            p.selected.successor.state_commitment,
            p.selected.normalized.canonical_digest().map_err(material)?,
        )?)
    }
    pub(super) fn prepared_replay_insert(
        &self,
    ) -> Result<&PreparedConsumedCreditInsertV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.replay_insert)
    }
    pub(crate) fn preparation_clock_context(
        &self,
    ) -> Result<&KagemushaOrdinaryCashClockContextV1, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(&self.pending()?.originals.clock)
    }
    pub(crate) fn with_verified_preparation_clock(
        &self,
        visitor: &mut dyn for<'clock> FnMut(
            &'clock KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let clock = self
            .owner
            .publication
            .cash_financial()
            .verified_retained_cash_clock_originals(&self.pending()?.originals.clock)
            .map_err(material)?;
        visitor(&clock)?;
        self.recheck_selected_originals_and_current_custody()
    }
    pub(crate) fn with_borrowed_financial_secret(
        &self,
        visitor: &mut dyn for<'secret> FnMut(
            &'secret [u8; 32],
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let f = self.owner.publication.cash_financial();
        let i = self.pending()?.originals.financial_control;
        let cap = self
            .owner
            .control
            .borrow_captured_proof_decision(f, i.original_sha256, i.lower_ms, i.upper_ms)
            .map_err(material)?;
        let secret = cap.financial_secret().map_err(material)?;
        if crate::kagemusha_v1_recursion::device_authority_commitment_v1(*secret)
            != f.enrollment()
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
    /// Source opening is a separate cryptographic loan; the fresh incoming financial secret
    /// above is mandatory and cannot be borrowed from an old sender or Mint authorization.
    pub(crate) fn with_borrowed_credit_opening(
        &self,
        visitor: &mut dyn for<'secret> FnMut(
            &'secret KagemushaCreditOpeningV1,
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        match self
            .owner
            .pending_incoming
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .intent
            .source
        {
            SourceLocator::Mint => self
                .owner
                .captured_mint_selection()?
                .with_borrowed_mint_secrets(&mut |_, opening| visitor(opening))?,
            SourceLocator::Receive { request_id } => self
                .owner
                .received_source_custody(request_id)?
                .with_borrowed_received_credit_opening(visitor)?,
        }
        self.recheck_selected_originals_and_current_custody()
    }
    pub(crate) fn with_finalized_mint_source(
        &self,
        visitor: &mut dyn for<'source, 'owner> FnMut(
            &'source KagemushaAuthenticatedOrdinaryFinalizedMintSourceV1<'owner>,
            &'source KagemushaMintCreditV1,
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let p = self.pending()?;
        let (original, credit_raw) = p
            .originals
            .mint_originals
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let f = self.owner.publication.cash_financial();
        let i = self.owner.retained_mint_source_control_identity()?;
        let captured = self
            .owner
            .control
            .borrow_captured_proof_decision(f, i.original_sha256, i.lower_ms, i.upper_ms)
            .map_err(material)?;
        let auth = self
            .owner
            .readmit_selected_mint_request(self.owner.retained_predebit_request_original()?)?;
        let source = f
            .authenticate_finalized_mint_source(&captured, auth, original)
            .map_err(material)?;
        let credit = decode_credit(credit_raw)?;
        visitor(&source, &credit)?;
        self.recheck_selected_originals_and_current_custody()
    }
    /// The actual W2 Receive selection lends only its same Main-retained historical source
    /// and RequestCapture. Fresh W2 PI/FI is independently required by this approval owner.
    pub(crate) fn with_received_source_and_request(
        &self,
        visitor: &mut dyn for<'loan, 'owner> FnMut(
            &'loan crate::kagemusha_v1_recursion::KagemushaVerifiedOrdinaryReceivedCashOutputV1,
            &'loan KagemushaHistoricalOrdinaryReceiverRequestCustodyV1<'owner>,
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let SourceLocator::Receive { request_id } = self
            .owner
            .pending_incoming
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .intent
            .source
        else {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        };
        self.owner
            .received_source_custody(request_id)?
            .with_received_source_and_request(visitor)?;
        self.recheck_selected_originals_and_current_custody()
    }
    pub(crate) fn with_received_source(
        &self,
        visitor: &mut dyn for<'source> FnMut(
            &'source crate::kagemusha_v1_recursion::KagemushaVerifiedOrdinaryReceivedCashOutputV1,
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let SourceLocator::Receive { request_id } = self
            .owner
            .pending_incoming
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .intent
            .source
        else {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        };
        let (_, source) = self.owner.retained_incoming_received_source(request_id)?;
        visitor(source)?;
        self.recheck_selected_originals_and_current_custody()
    }
}

#[cfg(test)]
mod approval_clock_tests {
    use super::*;

    // Public data vectors only. These do not construct a signed clock, Native custody or approval.
    fn data_clock(
        tag: u8,
        lower_at_ms: u64,
        upper_at_ms: u64,
    ) -> KagemushaOrdinaryCashClockContextV1 {
        KagemushaOrdinaryCashClockContextV1 {
            version: 1,
            request_nonce: [tag; 32],
            signed_observations_original_digest: [tag + 1; 32],
            lower_at_ms,
            upper_at_ms,
        }
    }

    #[test]
    fn incoming_approval_clock_rejects_either_regressing_bound() {
        let prepared = data_clock(1, 100, 110);
        for (lower, upper) in [(99, 115), (105, 109)] {
            assert_eq!(
                require_incoming_approval_clock_interval(
                    &prepared,
                    &data_clock(3, lower, upper),
                    90,
                    200,
                ),
                Err(KagemushaStateErrorV1::SnapshotRollback),
            );
        }
        assert!(
            require_incoming_approval_clock_interval(&prepared, &data_clock(3, 100, 110), 90, 200,)
                .is_ok()
        );
        assert!(
            require_incoming_approval_clock_interval(&prepared, &data_clock(3, 111, 120), 90, 200,)
                .is_ok()
        );
    }

    #[test]
    fn incoming_approval_capture_cannot_move_behind_original_admission() {
        let original = data_clock(1, 120, 130);
        for (lower, upper) in [(119, 135), (125, 129)] {
            assert_eq!(
                require_incoming_approval_clock_nonregression(
                    &original,
                    &data_clock(3, lower, upper),
                ),
                Err(KagemushaStateErrorV1::SnapshotRollback),
            );
        }
        assert!(
            require_incoming_approval_clock_nonregression(&original, &data_clock(3, 120, 130),)
                .is_ok()
        );
        assert!(
            require_incoming_approval_clock_nonregression(&original, &data_clock(3, 130, 140),)
                .is_ok()
        );
    }

    #[test]
    fn incoming_approval_clock_requires_complete_original_window_and_shape() {
        let prepared = data_clock(1, 100, 110);
        let valid = data_clock(3, 120, 130);
        let mut absent_nonce = valid;
        absent_nonce.request_nonce = [0; 32];
        let mut absent_original = valid;
        absent_original.signed_observations_original_digest = [0; 32];
        let mut foreign_version = valid;
        foreign_version.version = 2;
        for clock in [
            absent_nonce,
            absent_original,
            foreign_version,
            data_clock(3, 0, 130),
            data_clock(3, 130, 120),
            data_clock(3, 120, 200),
            data_clock(3, 120, 201),
            data_clock(3, 89, 130),
        ] {
            assert!(require_incoming_approval_clock_interval(&prepared, &clock, 90, 200).is_err());
        }
        for (issued, expires) in [(0, 200), (200, 200), (201, 200)] {
            assert!(
                require_incoming_approval_clock_interval(&prepared, &valid, issued, expires)
                    .is_err()
            );
        }
        assert!(
            require_incoming_approval_clock_interval(&prepared, &data_clock(3, 120, 199), 90, 200)
                .is_ok()
        );
    }

    #[test]
    fn incoming_approval_wal_roundtrip_retains_full_original_and_capture_clock() {
        let original_clock = data_clock(1, 120, 130);
        let capture_clock = data_clock(3, 140, 150);
        for (record, expected_clock) in [
            (
                IncomingApprovalRecord::Original {
                    operation: [7; 32],
                    clock: original_clock,
                    original: vec![8; 20],
                    authorization: [9; 32],
                    accepted_counter: Some(10),
                },
                original_clock,
            ),
            (
                IncomingApprovalRecord::Capture {
                    operation: [7; 32],
                    clock: capture_clock,
                    authorization: [9; 32],
                },
                capture_clock,
            ),
        ] {
            let raw = norito::encode_canonical(&record).expect("encode data-only WAL vector");
            let decoded: IncomingApprovalRecord = norito::decode_canonical_with_limits(
                &raw,
                norito::canonical_decode_limits(raw.len()),
            )
            .expect("decode data-only WAL vector");
            assert_eq!(decoded, record);
            let mut other_identity = expected_clock;
            other_identity.request_nonce = [11; 32];
            other_identity.signed_observations_original_digest = [12; 32];
            let changed = match decoded {
                IncomingApprovalRecord::Original {
                    operation,
                    original,
                    authorization,
                    accepted_counter,
                    clock,
                } => {
                    assert_eq!(clock, expected_clock);
                    IncomingApprovalRecord::Original {
                        operation,
                        original,
                        authorization,
                        accepted_counter,
                        clock: other_identity,
                    }
                }
                IncomingApprovalRecord::Capture {
                    operation,
                    authorization,
                    clock,
                } => {
                    assert_eq!(clock, expected_clock);
                    IncomingApprovalRecord::Capture {
                        operation,
                        authorization,
                        clock: other_identity,
                    }
                }
                _ => panic!("unexpected data-only vector"),
            };
            // Identical scalar bounds with different full selectors change the persisted original.
            assert_ne!(
                norito::encode_canonical(&changed).expect("encode changed data-only selector"),
                raw
            );
        }
    }
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    // Private data projection from actual Native selection. The driver cannot offer W or C.
    pub(super) fn incoming_preparation_platform_state(
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
        let p = self.pending_incoming_approval()?;
        if p.capture.is_some() {
            self.captured_incoming_approval()?
                .recheck_selected_originals_and_current_custody()?;
        } else {
            self.require_live_incoming_approval()?;
        }
        let original = p
            .capture
            .as_ref()
            .map(|(_, _, a)| a.original().to_vec())
            .or_else(|| p.retained.as_ref().map(|(_, a)| a.original().to_vec()));
        Ok((
            p.selected.challenge,
            p.fenced,
            original,
            p.capture.is_some(),
        ))
    }
}
