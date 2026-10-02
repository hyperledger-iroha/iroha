//! Main-owned funding lifecycle before the existing genuine incoming State consumer.
//! Complete proofs, consent, exclusive predecessor intent and transaction originals precede
//! transport. A raw Core decision/submission cannot fund State; only held Node finality can.
use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaArtifactByteResolverV1, KagemushaProductionProverV1,
    KagemushaRecursiveVerifierProfileV1,
};
use iroha_crypto::Signature;
use iroha_data_model::{
    isi::kagemusha_v1::TopUpKagemushaOrdinaryV1,
    kagemusha::*,
    transaction::{Executable, SignedTransaction, TransactionPayload},
};

const TRANSACTION_MAX_BYTES: usize = 64_000_000;
pub(super) const FUNDING_CAPACITY_BYTES: u64 = KAGEMUSHA_ORDINARY_NODE_MINT_SUBMISSION_MAX_BYTES_V1
    as u64
    + 2 * TRANSACTION_MAX_BYTES as u64
    + KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1 as u64
    + 2 * KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1 as u64
    + 512 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_v1_state::OrdinaryMintFundingRecordV1")]
pub(super) enum MintFundingRecord {
    ConsentFence,
    Consent([u8; 64]),
    PreDebit(Vec<Vec<u8>>),
    PreDebitInvoked,
    Decision {
        signed: Vec<u8>,
        clock: Vec<u8>,
        control: Vec<u8>,
        data: Vec<u8>,
    },
    NodeSubmission(Vec<u8>),
    TransactionFence {
        clock: KagemushaOrdinaryCashClockContextV1,
    },
    TransactionOriginal {
        canonical: Vec<u8>,
        wire: Vec<u8>,
    },
    TransactionDispatched,
    FinalizedOriginal(Vec<u8>),
    FinalizedAcknowledged,
}
#[derive(Default)]
pub(super) struct MintFundingState {
    consent_fenced: bool,
    consent: Option<[u8; 64]>,
    predebit: Option<Vec<Vec<u8>>>,
    predebit_invoked: bool,
    decision: Option<[Vec<u8>; 4]>,
    submission: Option<Vec<u8>>,
    transaction_fence: Option<KagemushaOrdinaryCashClockContextV1>,
    transaction: Option<Vec<u8>>,
    transaction_wire: Option<Vec<u8>>,
    dispatched: bool,
    finalized: Option<Vec<u8>>,
    finalized_acknowledged: bool,
}
impl MintFundingState {
    // This is a projection of already admitted private WAL rows, never an effect grant.
    // Every known signature/original follows its own earlier invocation fence. A gap is
    // corrupt custody, not permission to infer a missing signature or start another call.
    fn progress_stage(&self, request_proven: bool) -> Result<u8, KagemushaStateErrorV1> {
        if self.transaction.is_some() != self.transaction_wire.is_some() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let retained = [
            request_proven,
            self.consent_fenced,
            self.consent.is_some(),
            self.predebit.is_some(),
            self.predebit_invoked,
            self.decision.is_some(),
            self.submission.is_some(),
            self.transaction_fence.is_some(),
            self.transaction.is_some(),
            self.dispatched,
            self.finalized.is_some(),
            self.finalized_acknowledged,
        ];
        if retained.windows(2).any(|pair| !pair[0] && pair[1]) {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        u8::try_from(retained.iter().take_while(|present| **present).count()).map_err(material)
    }
    fn require_next(&self, r: &MintFundingRecord) -> Result<(), KagemushaStateErrorV1> {
        let allowed = match r {
            MintFundingRecord::ConsentFence => !self.consent_fenced && self.consent.is_none(),
            MintFundingRecord::Consent(_) => self.consent_fenced && self.consent.is_none(),
            MintFundingRecord::PreDebit(_) => self.consent.is_some() && self.predebit.is_none(),
            MintFundingRecord::PreDebitInvoked => self.predebit.is_some() && !self.predebit_invoked,
            MintFundingRecord::Decision { .. } => self.predebit_invoked && self.decision.is_none(),
            MintFundingRecord::NodeSubmission(_) => {
                self.decision.is_some() && self.submission.is_none()
            }
            MintFundingRecord::TransactionFence { .. } => {
                self.submission.is_some() && self.transaction_fence.is_none()
            }
            MintFundingRecord::TransactionOriginal { .. } => {
                self.transaction_fence.is_some() && self.transaction.is_none()
            }
            MintFundingRecord::TransactionDispatched => {
                self.transaction.is_some() && !self.dispatched
            }
            MintFundingRecord::FinalizedOriginal(_) => self.dispatched && self.finalized.is_none(),
            MintFundingRecord::FinalizedAcknowledged => {
                self.finalized.is_some() && !self.finalized_acknowledged
            }
        };
        if !allowed {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        Ok(())
    }
    fn install(&mut self, r: MintFundingRecord) {
        match r {
            MintFundingRecord::ConsentFence => self.consent_fenced = true,
            MintFundingRecord::Consent(s) => self.consent = Some(s),
            MintFundingRecord::PreDebit(v) => self.predebit = Some(v),
            MintFundingRecord::PreDebitInvoked => self.predebit_invoked = true,
            MintFundingRecord::Decision {
                signed,
                clock,
                control,
                data,
            } => self.decision = Some([signed, clock, control, data]),
            MintFundingRecord::NodeSubmission(v) => self.submission = Some(v),
            MintFundingRecord::TransactionFence { clock } => self.transaction_fence = Some(clock),
            MintFundingRecord::TransactionOriginal { canonical, wire } => {
                self.transaction = Some(canonical);
                self.transaction_wire = Some(wire);
            }
            MintFundingRecord::TransactionDispatched => self.dispatched = true,
            MintFundingRecord::FinalizedOriginal(v) => self.finalized = Some(v),
            MintFundingRecord::FinalizedAcknowledged => self.finalized_acknowledged = true,
        }
    }
}

/// One genuine Main invocation after its durable consent fence. No public constructor/decoder.
pub struct KagemushaAuthenticatedOrdinaryMintAccountSigningV1<'a> {
    owner: &'a KagemushaNativeOrdinaryCashOwnerV1,
    prefix: KagemushaRecoveryJournalPrefixV1,
}
impl KagemushaAuthenticatedOrdinaryMintAccountSigningV1<'_> {
    /// Exact actual retained complete unsigned Mint113 original.
    /// # Errors
    /// Refuses changed Main/prefix or absent real Mint proof custody.
    pub fn request_original(&self) -> Result<&[u8], KagemushaStateErrorV1> {
        self.recheck()?;
        self.owner.retained_predebit_request_original()
    }
    /// Sole account-consent message; no offered message or key can select it.
    /// # Errors
    /// Refuses changed custody or original codec.
    pub fn account_signing_message(&self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(self.request_original()?)
            .map_err(material)?
            .account_signing_message()
            .map_err(material)
    }
    /// Actual invocation and current FI/account custody, without a debit grant.
    /// # Errors
    /// Refuses unknown, changed, already consumed or expired invocation.
    pub fn recheck(&self) -> Result<(), KagemushaStateErrorV1> {
        self.owner.require_mint_funding_custody()?;
        let f = self.owner.mint_funding_state()?;
        if self.prefix != self.owner.prefix || !f.consent_fenced || f.consent.is_some() {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.owner.require_current_financial_control()
    }
}
/// Exact Node submission selected by Main before the sole transaction account invocation.
pub struct KagemushaAuthenticatedOrdinaryMintTransactionSigningV1<'a> {
    owner: &'a KagemushaNativeOrdinaryCashOwnerV1,
    prefix: KagemushaRecoveryJournalPrefixV1,
}
impl KagemushaAuthenticatedOrdinaryMintTransactionSigningV1<'_> {
    /// Full selected Node submission data, independently enforced by Node World/proof admission.
    /// # Errors
    /// Refuses absent invocation or changed same-owner exact originals.
    pub fn submission(
        &self,
    ) -> Result<KagemushaOrdinaryNodeMintSubmissionV1, KagemushaStateErrorV1> {
        self.recheck()?;
        KagemushaOrdinaryNodeMintSubmissionV1::decode_canonical_exact(
            self.owner
                .mint_funding_state()?
                .submission
                .as_deref()
                .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?,
        )
        .map_err(material)
    }
    /// Exact Native clock sample already fsynced with this signing invocation.
    /// # Errors
    /// Refuses changed invocation or current validity.
    pub fn creation_time_ms(&self) -> Result<u64, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(self
            .owner
            .mint_funding_state()?
            .transaction_fence
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .lower_at_ms)
    }
    /// Mandatory short original decision budget; cannot be widened by the signer.
    /// # Errors
    /// Refuses expiry or a changed authentic Core decision.
    pub fn maximum_ttl_ms(&self) -> Result<u64, KagemushaStateErrorV1> {
        self.recheck()?;
        let d = self.owner.mint_funding_decision()?;
        d.subject
            .expires_at_ms
            .checked_sub(self.creation_time_ms()?)
            .filter(|v| *v > 0)
            .ok_or(KagemushaStateErrorV1::InvalidTrustedCommitTime)
    }
    /// Validate exact W/network, sole original ISI and captured Native time before signing.
    /// Fees are independently quoted by the actual account client; no extra effect is allowed.
    /// # Errors
    /// Rejects foreign instructions/metadata/time/domain or mutable originals.
    pub fn validate_payload(
        &self,
        payload: &TransactionPayload,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck()?;
        let submission = self.submission()?;
        require_transaction_payload(
            payload,
            &submission,
            self.creation_time_ms()?,
            self.maximum_ttl_ms()?,
        )
    }
    /// Recheck authentic same-process invocation and current Native FI/clock.
    /// # Errors
    /// An uncertain signature invocation cannot be repeated or replaced by a new nonce.
    pub fn recheck(&self) -> Result<(), KagemushaStateErrorV1> {
        self.owner.require_mint_funding_custody()?;
        let f = self.owner.mint_funding_state()?;
        if self.prefix != self.owner.prefix
            || f.transaction_fence.is_none()
            || f.transaction.is_some()
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.owner.require_live_mint_funding_decision()
    }
}
/// Actual Main-retained dispatch/read loan after the exact signed transaction fence.
/// No public constructor, offered wire or decoded selector creates this type.
pub struct KagemushaAuthenticatedOrdinaryMintFundingTransportV1<'a> {
    owner: &'a KagemushaNativeOrdinaryCashOwnerV1,
    prefix: KagemushaRecoveryJournalPrefixV1,
}
impl KagemushaAuthenticatedOrdinaryMintFundingTransportV1<'_> {
    /// Exact signed transaction canonical original, without regeneration.
    /// # Errors
    /// Refuses changed Main/current custody or absent retained signature.
    pub fn transaction_original(&self) -> Result<&[u8], KagemushaStateErrorV1> {
        self.recheck()?;
        self.owner
            .mint_funding_state()?
            .transaction
            .as_deref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }
    /// Sole public network wire, byte-identical through uncertain dispatch.
    /// # Errors
    /// Refuses changed custody or an absent original dispatch.
    pub fn transaction_wire(&self) -> Result<&[u8], KagemushaStateErrorV1> {
        self.recheck()?;
        self.owner
            .mint_funding_state()?
            .transaction_wire
            .as_deref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }
    /// Exact network/W/operation/requestSHA/decisionSHA from actual Main originals.
    /// # Errors
    /// Refuses changed originals/current financial custody.
    pub fn finality_read_fields(&self) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.recheck()?;
        self.owner.mint_funding_finality_read_fields()
    }
    /// Fresh same-owner Native sample for HTTP authentication only, without a funding grant.
    /// # Errors
    /// Refuses unavailable current FI/PI/clock; no original decision is renewed.
    pub fn http_timestamp_ms(&self) -> Result<u64, KagemushaStateErrorV1> {
        self.recheck()?;
        self.owner
            .publication
            .cash_financial()
            .trusted_time_interval()
            .map(|v| v.lower_ms())
            .map_err(material)
    }
    /// Actual owned WAL/dispatch plus separately current Native financial controls.
    /// # Errors
    /// Refuses changed prefix/owner, unknown dispatch or stale controls.
    pub fn recheck(&self) -> Result<(), KagemushaStateErrorV1> {
        self.owner.require_mint_funding_custody()?;
        if self.prefix != self.owner.prefix || !self.owner.mint_funding_state()?.dispatched {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.owner.require_current_financial_control()
    }
}
fn require_transaction_payload(
    payload: &TransactionPayload,
    s: &KagemushaOrdinaryNodeMintSubmissionV1,
    created: u64,
    ttl: u64,
) -> Result<(), KagemushaStateErrorV1> {
    let req = KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(&s.topup_request_original)
        .map_err(material)?;
    let ctx = &req.authorization.statement.context;
    let Executable::Instructions(instructions) = payload.instructions() else {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    };
    let [instruction] = instructions.as_ref() else {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    };
    let expected: iroha_data_model::isi::InstructionBox = TopUpKagemushaOrdinaryV1::new(s.clone())
        .map_err(material)?
        .into();
    if instruction != &expected
        || payload.authority() != &ctx.lineage.owner.account_id
        || payload.network_id() != Some(&ctx.lineage.owner.runtime.network_id)
        || payload.creation_time_ms != created
        || payload.nonce.is_none()
        || payload.time_to_live_ms.is_none_or(|v| v.get() > ttl)
        || !payload.metadata.is_empty()
        || payload.attachments.is_some()
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    fn mint_funding_state(&self) -> Result<&MintFundingState, KagemushaStateErrorV1> {
        Ok(&self
            .pending_mint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .funding)
    }
    fn require_mint_funding_custody(&self) -> Result<(), KagemushaStateErrorV1> {
        self.captured_mint_selection()?.recheck()?;
        self.publication.recheck_historical_cash_custody()?;
        Ok(())
    }
    fn mint_funding_request(
        &self,
    ) -> Result<KagemushaOrdinaryTopUpRequestV1, KagemushaStateErrorV1> {
        KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(
            self.retained_predebit_request_original()?,
        )
        .map_err(material)
    }
    fn mint_funding_decision(
        &self,
    ) -> Result<KagemushaSignedOrdinaryMintDebitDecisionV1, KagemushaStateErrorV1> {
        let f = self.mint_funding_state()?;
        KagemushaSignedOrdinaryMintDebitDecisionV1::decode_canonical_exact(
            &f.decision
                .as_ref()
                .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?[0],
        )
        .map_err(material)
    }
    fn require_live_mint_funding_decision(&self) -> Result<(), KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let d = self.mint_funding_decision()?;
        self.publication
            .cash_financial()
            .trusted_time_interval()
            .map_err(material)?
            .require_validity(d.subject.issued_at_ms, d.subject.expires_at_ms)
            .map_err(material)
    }
    fn validate_mint_funding_record(
        &self,
        r: &MintFundingRecord,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_mint_funding_custody()?;
        // Validate the prefix as it stood when the fence was recorded: a later proof row
        // cannot retroactively authorize an earlier account invocation during cold replay.
        if matches!(r, MintFundingRecord::ConsentFence) {
            self.retained_predebit_request_original()?;
        }
        let f = self.mint_funding_state()?;
        f.require_next(r)?;
        match r {
            MintFundingRecord::Consent(s) => self
                .mint_funding_request()?
                .verify_account_signature(&Signature::from_bytes(s))
                .map_err(material)?,
            MintFundingRecord::PreDebit(fields) => {
                if fields.len() != 8
                    || fields[0] != self.retained_incoming_selection_original_historical()?
                    || fields[1] != self.retained_predebit_request_original()?
                    || fields[2].as_slice()
                        != f.consent.ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    || fields[3]
                        != self
                            .publication
                            .cash_financial()
                            .enrollment()
                            .app_credential()
                            .original()
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                let loan = self.captured_mint_selection()?;
                if fields[4].as_slice()
                    != loan
                        .selected_integrity_lease()?
                        .map_or(&[][..], |v| v.original())
                    || fields[5] != loan.preparation_clock_original()?
                    || fields[6] != loan.financial_control_original()?
                    || fields[7].is_empty()
                    || fields[7].len() > KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                let current: KagemushaSignedOrdinaryCurrentControlV1 = decode_funding_original(
                    &fields[7],
                    KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
                )?;
                current
                    .verify_for_request(
                        &current.subject.request,
                        self.publication
                            .cash_financial()
                            .mint_funding_issuer_policy()
                            .map_err(material)?,
                    )
                    .map_err(material)?;
            }
            MintFundingRecord::Decision {
                signed,
                clock,
                control,
                data,
            } => {
                if data.is_empty()
                    || data.len() > 128 * 1024
                    || control.is_empty()
                    || control.len() > KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                let d = KagemushaSignedOrdinaryMintDebitDecisionV1::decode_canonical_exact(signed)
                    .map_err(material)?;
                let sel = self
                    .pending_incoming
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .intent
                    .selection
                    .clone();
                let fin = self.publication.cash_financial();
                d.verify_for_request(
                    &self.mint_funding_request()?,
                    &sel,
                    fin.mint_funding_issuer_policy().map_err(material)?,
                )
                .map_err(material)?;
                if d.subject.reserved_data_record_original_sha256
                    != <DigestV1>::from(Sha256::digest(data))
                    || d.subject.current_financial_control_original_sha256
                        != <DigestV1>::from(Sha256::digest(control))
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                fin.authenticate_mint_funding_clock(clock, &d.subject.decision_clock_context)
                    .map_err(material)?;
                let ctl: KagemushaSignedOrdinaryCurrentControlV1 = decode_funding_original(
                    control,
                    KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
                )?;
                ctl.verify_for_request(
                    &ctl.subject.request,
                    fin.mint_funding_issuer_policy().map_err(material)?,
                )
                .map_err(material)?;
                if ctl.subject.request.owner
                    != self
                        .mint_funding_request()?
                        .authorization
                        .statement
                        .context
                        .lineage
                        .owner
                    || d.subject.decision_clock_context.lower_at_ms < ctl.subject.issued_at_ms
                    || d.subject.decision_clock_context.upper_at_ms >= ctl.subject.expires_at_ms
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
            }
            MintFundingRecord::NodeSubmission(raw) => {
                let s = KagemushaOrdinaryNodeMintSubmissionV1::decode_canonical_exact(raw)
                    .map_err(material)?;
                let d = f
                    .decision
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                let p = f
                    .predebit
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if s.topup_request_original != p[1]
                    || s.account_consent.payload() != p[2]
                    || s.credential_original != p[3]
                    || s.preparation_clock_original != p[5]
                    || s.preparation_control_original != p[6]
                    || s.debit_decision_original != d[0]
                    || s.decision_clock_original != d[1]
                    || s.current_control_original != d[2]
                    || s.financial_enrollment_original
                        != self
                            .publication
                            .cash_financial()
                            .retained_finalized_source_enrollment_original()
                            .map_err(material)?
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                if s.preparation_integrity
                    .as_ref()
                    .map(|i| i.lease_original.as_slice())
                    .unwrap_or(&[])
                    != p[4].as_slice()
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                self.mint_funding_request()?
                    .verify_account_signature(&s.account_consent)
                    .map_err(material)?;
            }
            MintFundingRecord::TransactionFence { clock } => {
                let d = self.mint_funding_decision()?;
                if clock.lower_at_ms < d.subject.issued_at_ms
                    || clock.upper_at_ms >= d.subject.expires_at_ms
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                self.publication
                    .cash_financial()
                    .verified_retained_cash_clock_originals(clock)
                    .map_err(material)?
                    .recheck_cash_context(clock)
                    .map_err(material)?;
            }
            MintFundingRecord::TransactionOriginal { canonical, wire } => {
                let tx = decode_transaction(canonical)?;
                if wire.is_empty()
                    || wire.len() > TRANSACTION_MAX_BYTES
                    || tx.encode_wire_v1().map_err(material)? != *wire
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                let s = KagemushaOrdinaryNodeMintSubmissionV1::decode_canonical_exact(
                    f.submission
                        .as_deref()
                        .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
                )
                .map_err(material)?;
                let c = f
                    .transaction_fence
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                let ttl = self
                    .mint_funding_decision()?
                    .subject
                    .expires_at_ms
                    .checked_sub(c.lower_at_ms)
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                require_transaction_payload(tx.payload(), &s, c.lower_at_ms, ttl)?;
                tx.verify_signature().map_err(material)?;
            }
            MintFundingRecord::FinalizedOriginal(raw) => {
                self.require_mint_finalized_original(raw)?
            }
            MintFundingRecord::FinalizedAcknowledged => self.require_mint_finalized_original(
                f.finalized
                    .as_deref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
            )?,
            _ => {}
        }
        Ok(())
    }
    fn persist_mint_funding(&mut self, r: MintFundingRecord) -> Result<(), KagemushaStateErrorV1> {
        self.validate_mint_funding_record(&r)?;
        self.persist(&Record::Mint(MintRecord::Funding(Box::new(r.clone()))))?;
        self.pending_mint
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .funding
            .install(r);
        // Memory follows the known durable row BEFORE a post-fsync current recheck. A failed
        // post-check cannot authorize a repeat account/platform call or erase its fence.
        self.require_mint_funding_custody()
    }
    pub(super) fn replay_mint_funding(
        &mut self,
        r: MintFundingRecord,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.validate_mint_funding_record(&r)?;
        self.pending_mint
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .funding
            .install(r);
        Ok(())
    }
    fn retained_incoming_selection_original_historical(
        &self,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        let p = self
            .pending_incoming
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        p.intent.recheck_historical(self)?;
        norito::encode_canonical(&p.intent.selection).map_err(material)
    }
    fn require_mint_finalized_original(&self, raw: &[u8]) -> Result<(), KagemushaStateErrorV1> {
        let f = self.mint_funding_state()?;
        let v = KagemushaOrdinaryTopUpFinalizedOriginalV1::decode_canonical_exact(raw)
            .map_err(material)?;
        let d = f
            .decision
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let tx = decode_transaction(
            f.transaction
                .as_deref()
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
        )?;
        if v.request_original != self.retained_predebit_request_original()?
            || v.issuer_decision_original != d[0]
            || v.finality.reserve_receipt_witness.receipt.transaction_hash
                != <[u8; 32]>::from(iroha_crypto::Hash::from(tx.hash()))
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let old = self.retained_mint_source_control_identity()?;
        let fin = self.publication.cash_financial();
        let captured = self
            .control
            .borrow_captured_proof_decision(fin, old.original_sha256, old.lower_ms, old.upper_ms)
            .map_err(material)?;
        let proof = self.verified_retained_mint_request()?;
        fin.authenticate_finalized_mint_source(&captured, proof, raw)
            .map_err(material)?
            .recheck_retained_custody()
            .map_err(material)
    }

    /// Prepare actual dedicated Mint platform data under the genuine current owner.
    /// # Errors
    /// Refuses stale FI, another pending operation or unavailable capacity/key/proof release.
    pub fn prepare_mint_funding_platform(
        &mut self,
        amount: u128,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        let op = self.reserve_mint_request(amount)?;
        Ok(vec![
            op.to_vec(),
            self.mint_approval_signing_message(op)?,
            self.publication
                .cash_financial()
                .enrollment()
                .app_credential()
                .original()
                .to_vec(),
        ])
    }
    /// The original platform fence precedes the one actual OS call.
    /// # Errors
    /// Refuses uncertainty, expiry and repeated invocation.
    pub fn fence_mint_funding_platform(&mut self) -> Result<(), KagemushaStateErrorV1> {
        let op = self.mint_funding_operation_id()?;
        self.fence_mint_platform(op)
    }
    /// Native authenticates the complete dedicated approval and fsyncs original then capture.
    /// # Errors
    /// Refuses a foreign/repeated raw original or expired original capture window.
    pub fn retain_mint_funding_platform(
        &mut self,
        raw: &[u8],
    ) -> Result<(), KagemushaStateErrorV1> {
        let op = self.mint_funding_operation_id()?;
        self.capture_mint_approval_original(op, raw)
    }
    /// Real released Mint113 proving followed by independent admission and durable full request.
    /// # Errors
    /// Refuses unavailable actual ordinary keys/proof relation or changed historical custody.
    pub fn prove_mint_funding_request<R: KagemushaArtifactByteResolverV1 + Clone>(
        &mut self,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        if self.retained_predebit_request_original().is_ok() {
            return Ok(self
                .verified_retained_mint_request()?
                .request_original_sha256());
        }
        let selection = self.captured_mint_selection()?;
        let prover = KagemushaProductionProverV1::load_ordinary_mint(&selection, profile, resolver)
            .map_err(material)?;
        let proof = prover
            .prove_ordinary_mint_authorization(&selection)
            .map_err(material)?;
        let digest = proof.request_original_sha256();
        self.retain_proven_mint_request(&proof)?;
        Ok(digest)
    }
    /// Durable same-owner consent fence, exact Native account callback, immutable Ed64 capture.
    /// # Errors
    /// An interrupted/uncertain callback cannot be retried with another signature invocation.
    pub fn sign_mint_funding_consent(
        &mut self,
        sign: impl FnOnce(
            &KagemushaAuthenticatedOrdinaryMintAccountSigningV1<'_>,
        ) -> Result<[u8; 64], KagemushaStateErrorV1>,
    ) -> Result<[u8; 64], KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        // The durable consent fence follows the already admitted ProvenRequest row.
        // Refuse a missing request before recording any irreversible invocation fence.
        self.retained_predebit_request_original()?;
        if let Some(s) = self.mint_funding_state()?.consent {
            self.mint_funding_request()?
                .verify_account_signature(&Signature::from_bytes(&s))
                .map_err(material)?;
            return Ok(s);
        }
        self.persist_mint_funding(MintFundingRecord::ConsentFence)?;
        let original = KagemushaAuthenticatedOrdinaryMintAccountSigningV1 {
            owner: self,
            prefix: self.prefix,
        };
        let s = sign(&original)?;
        original.recheck()?;
        self.persist_mint_funding(MintFundingRecord::Consent(s))?;
        self.require_current_financial_control()?;
        Ok(s)
    }
    /// Fsync the genuine pre-debit incoming head intent BEFORE exposing the Core request.
    /// Exact retry returns the same complete eight original fields; no fresh nonce/proof is made.
    /// # Errors
    /// Refuses missing actual proof/consent, stale FI or another financial attempt/head.
    pub fn prepare_mint_funding_predebit(&mut self) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if let Some(fields) = &self.mint_funding_state()?.predebit {
            return Ok(fields.clone());
        }
        self.reserve_incoming_mint()?;
        let sel = self.retained_incoming_selection_original_historical()?;
        let selected = self.captured_mint_selection()?;
        let fields = vec![
            sel,
            self.retained_predebit_request_original()?.to_vec(),
            self.mint_funding_state()?
                .consent
                .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
                .to_vec(),
            self.publication
                .cash_financial()
                .enrollment()
                .app_credential()
                .original()
                .to_vec(),
            selected
                .selected_integrity_lease()?
                .map_or(Vec::new(), |v| v.original().to_vec()),
            selected.preparation_clock_original()?,
            selected.financial_control_original()?,
            self.control
                .loan(self.publication.cash_financial())
                .map_err(material)?
                .original()
                .map_err(material)?
                .to_vec(),
        ];
        self.persist_mint_funding(MintFundingRecord::PreDebit(fields.clone()))?;
        self.require_current_financial_control()?;
        Ok(fields)
    }
    /// Retain dispatch uncertainty and expose only the exact previously fsynced Core request.
    /// # Errors
    /// Refuses absent original, changed current custody or uncertain persistence.
    pub fn fence_mint_funding_predebit(&mut self) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        let fields = self.prepare_mint_funding_predebit()?;
        if !self.mint_funding_state()?.predebit_invoked {
            self.persist_mint_funding(MintFundingRecord::PreDebitInvoked)?;
        }
        self.require_current_financial_control()?;
        Ok(fields)
    }
    /// Read the genuine retained funding stage without invoking a key, transport or clock.
    /// Returns the same operation ID and one byte: 0 no proven request, 1 proven request,
    /// 2 unknown consent signature, 3 retained consent, 4 prepared pre-debit original,
    /// 5 unknown Core response, 6 retained decision without Node packet, 7 retained packet,
    /// 8 unknown transaction signature, 9 retained transaction, 10 dispatched transaction,
    /// 11 retained finalized raw without acknowledgment, 12 acknowledged finalized raw.
    /// Platform state is separately read by `recover_mint_funding_platform` for stage 0.
    /// Stages 2 and 8 never authorize another signature call. Every effect still requires
    /// its existing current FI/PI/account/clock checks; this does not renew an old decision.
    /// # Errors
    /// Refuses incomplete cold semantic recovery, uncertain persistence, changed owned WAL,
    /// mismatched historical Mint custody or an inconsistent retained row chronology.
    pub fn recover_mint_funding_progress(&self) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        let require_custody = || -> Result<Vec<u8>, KagemushaStateErrorV1> {
            if self.recovery_catalog.is_some() || self.recovery_failed {
                return Err(KagemushaStateErrorV1::InvalidCandidateStage);
            }
            self.journal.check_owned().map_err(storage)?;
            if self.journal.recovery_prefix().map_err(storage)? != self.prefix {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            // This authenticates the actual original C/FI/PI and retained platform rows at
            // their captured historical bounds, without promoting a finite current loan.
            let fields = self.recover_mint_funding_platform()?;
            fields
                .get(1)
                .cloned()
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)
        };
        let operation = require_custody()?;
        let request_proven = match self.retained_predebit_request_original() {
            Ok(_) => {
                // Admission/reopen already verifies the full retained Mint proof. Routine
                // progress reads recheck its actual closed capture and private owner identity.
                self.require_mint_funding_custody()?;
                true
            }
            Err(KagemushaStateErrorV1::InvalidCandidateStage) => false,
            Err(error) => return Err(error),
        };
        let stage = self.mint_funding_state()?.progress_stage(request_proven)?;
        if require_custody()? != operation {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        if request_proven {
            self.require_mint_funding_custody()?;
        }
        Ok(vec![operation, vec![stage]])
    }
    /// Read only the exact already dispatched Core request for protected decision recovery.
    /// No archive/head/nonce/control replacement or new dispatch fence is selected here.
    /// # Errors
    /// Refuses an absent invocation or changed actual historical Main/Mint/journal custody.
    pub fn recover_mint_funding_predebit_originals(
        &self,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.require_mint_funding_custody()?;
        let state = self.mint_funding_state()?;
        if !state.predebit_invoked {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let fields = state
            .predebit
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .clone();
        self.require_mint_funding_custody()?;
        Ok(fields)
    }
    /// Retain/authenticate the four exact issuer response originals. This is still pre-debit data.
    /// # Errors
    /// Rejects a foreign signature/selection/clock/control/DATA row or substituted retry response.
    pub fn retain_mint_funding_decision(
        &mut self,
        signed: &[u8],
        clock: &[u8],
        control: &[u8],
        data: &[u8],
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if signed.is_empty()
            || signed.len() > KAGEMUSHA_ORDINARY_MINT_DEBIT_DECISION_MAX_BYTES_V1
            || clock.is_empty()
            || clock.len() > KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1
            || control.is_empty()
            || control.len() > KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1
            || data.is_empty()
            || data.len() > 128 * 1024
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let offered = [
            signed.to_vec(),
            clock.to_vec(),
            control.to_vec(),
            data.to_vec(),
        ];
        if let Some(old) = &self.mint_funding_state()?.decision {
            return if old == &offered {
                Ok(())
            } else {
                Err(KagemushaStateErrorV1::SnapshotIntegrity)
            };
        }
        self.persist_mint_funding(MintFundingRecord::Decision {
            signed: offered[0].clone(),
            clock: offered[1].clone(),
            control: offered[2].clone(),
            data: offered[3].clone(),
        })?;
        self.require_current_financial_control()
    }
    /// Retain complete Node packet data, joined to actual Main/Mint/issuer originals.
    /// Node independently admits the installed World purpose and all public evidence before debit.
    /// # Errors
    /// Refuses partial packet, another request/consent/FI/clock or changed exact retry original.
    pub fn retain_mint_funding_node_submission(
        &mut self,
        raw: &[u8],
    ) -> Result<(), KagemushaStateErrorV1> {
        if raw.is_empty() || raw.len() > KAGEMUSHA_ORDINARY_NODE_MINT_SUBMISSION_MAX_BYTES_V1 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.require_live_mint_funding_decision()?;
        if let Some(old) = &self.mint_funding_state()?.submission {
            return if old == raw {
                Ok(())
            } else {
                Err(KagemushaStateErrorV1::SnapshotIntegrity)
            };
        }
        self.persist_mint_funding(MintFundingRecord::NodeSubmission(raw.to_vec()))?;
        self.require_live_mint_funding_decision()
    }
    /// Fence actual Native transaction signing, then retain exact signed versioned original.
    /// # Errors
    /// Refuses unknown signature result, changed ISI/fees/time or an expired original decision.
    pub fn sign_mint_funding_transaction(
        &mut self,
        sign: impl FnOnce(
            &KagemushaAuthenticatedOrdinaryMintTransactionSigningV1<'_>,
        ) -> Result<[Vec<u8>; 2], KagemushaStateErrorV1>,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_live_mint_funding_decision()?;
        if let Some(raw) = self.mint_funding_state()?.transaction.as_deref() {
            return Ok(iroha_crypto::Hash::from(decode_transaction(raw)?.hash()).into());
        }
        let clock = self
            .publication
            .cash_financial()
            .current_cash_clock_context()
            .map_err(material)?;
        self.persist_mint_funding(MintFundingRecord::TransactionFence { clock })?;
        let original = KagemushaAuthenticatedOrdinaryMintTransactionSigningV1 {
            owner: self,
            prefix: self.prefix,
        };
        let raw = sign(&original)?;
        original.recheck()?;
        let [canonical, wire] = raw;
        self.persist_mint_funding(MintFundingRecord::TransactionOriginal { canonical, wire })?;
        self.require_live_mint_funding_decision()?;
        Ok(iroha_crypto::Hash::from(
            decode_transaction(
                self.mint_funding_state()?
                    .transaction
                    .as_deref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
            )?
            .hash(),
        )
        .into())
    }
    /// Fsync dispatch fence and lend only the exact retained transaction for same-byte retry.
    /// # Errors
    /// Rejects expiry before first dispatch, absent transaction or changed current FI/custody.
    pub fn dispatch_mint_funding_transaction_original(
        &mut self,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if !self.mint_funding_state()?.dispatched {
            self.require_live_mint_funding_decision()?;
            self.persist_mint_funding(MintFundingRecord::TransactionDispatched)?;
        }
        let raw = self
            .mint_funding_state()?
            .transaction_wire
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .clone();
        self.require_current_financial_control()?;
        Ok(raw)
    }
    /// Lend exact Main transport after durable dispatch. Unknown outcomes retain the original;
    /// later calls may send the same wire or read finality, without another signature or nonce.
    /// # Errors
    /// Refuses stale custody, absent signature or uncertain persistence.
    pub fn with_mint_funding_transport<T>(
        &mut self,
        transport: impl FnOnce(
            &KagemushaAuthenticatedOrdinaryMintFundingTransportV1<'_>,
        ) -> Result<T, KagemushaStateErrorV1>,
    ) -> Result<T, KagemushaStateErrorV1> {
        self.dispatch_mint_funding_transaction_original()?;
        let loan = KagemushaAuthenticatedOrdinaryMintFundingTransportV1 {
            owner: self,
            prefix: self.prefix,
        };
        loan.recheck()?;
        let result = transport(&loan)?;
        loan.recheck()?;
        Ok(result)
    }
    /// Exact payer/operation/request/decision fields for the separate signed finality read.
    /// A read or pending status is data only and cannot establish a finalized source.
    /// # Errors
    /// Rejects absent genuine dispatch or changed actual Main custody.
    pub fn mint_funding_finality_read_fields(&self) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.require_mint_funding_custody()?;
        self.require_current_financial_control()?;
        if !self.mint_funding_state()?.dispatched {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let req = self.mint_funding_request()?;
        let c = &req.authorization.statement.context;
        let d = self
            .mint_funding_state()?
            .decision
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        Ok(vec![
            norito::encode_canonical(&c.lineage.owner.runtime.network_id).map_err(material)?,
            norito::encode_canonical(&c.lineage.owner.account_id).map_err(material)?,
            c.operation_id.to_vec(),
            Sha256::digest(self.retained_predebit_request_original()?).to_vec(),
            Sha256::digest(&d[0]).to_vec(),
        ])
    }
    /// Authenticate full Node/Kura finality under the actual retained Native prefix; raw fsync
    /// then a distinct acknowledgment preserve an uncertain read/capture through cold recovery.
    /// # Errors
    /// Refuses foreign transaction/request/decision, invalid finality or changed actual owner.
    pub fn retain_mint_funding_finalized_original(
        &mut self,
        raw: &[u8],
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if raw.is_empty() || raw.len() > KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        if let Some(old) = self.mint_funding_state()?.finalized.as_deref() {
            if old != raw {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        } else {
            self.persist_mint_funding(MintFundingRecord::FinalizedOriginal(raw.to_vec()))?;
        }
        self.require_mint_finalized_original(raw)?;
        self.require_current_financial_control()?;
        if !self.mint_funding_state()?.finalized_acknowledged {
            self.persist_mint_funding(MintFundingRecord::FinalizedAcknowledged)?;
        }
        self.require_current_financial_control()
    }
}
fn decode_funding_original<T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>>(
    raw: &[u8],
    max: usize,
) -> Result<T, KagemushaStateErrorV1> {
    if raw.is_empty() || raw.len() > max {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let v: T =
        norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
            .map_err(material)?;
    if norito::encode_canonical(&v).map_err(material)? != raw {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(v)
}
fn decode_transaction(raw: &[u8]) -> Result<SignedTransaction, KagemushaStateErrorV1> {
    if raw.is_empty() || raw.len() > TRANSACTION_MAX_BYTES {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let v: SignedTransaction = decode_funding_original(raw, TRANSACTION_MAX_BYTES)?;
    v.verify_signature().map_err(material)?;
    Ok(v)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn funding_chronology_rejects_orphan_funding_and_unknown_signature_reinvocation() {
        let mut s = MintFundingState::default();
        assert!(
            s.require_next(&MintFundingRecord::Consent([1; 64]))
                .is_err()
        );
        s.require_next(&MintFundingRecord::ConsentFence).unwrap();
        s.install(MintFundingRecord::ConsentFence);
        assert!(s.require_next(&MintFundingRecord::ConsentFence).is_err());
        assert!(
            s.require_next(&MintFundingRecord::FinalizedAcknowledged)
                .is_err()
        );
        s.require_next(&MintFundingRecord::Consent([1; 64]))
            .unwrap();
        s.install(MintFundingRecord::Consent([1; 64]));
        assert!(
            s.require_next(&MintFundingRecord::Consent([2; 64]))
                .is_err()
        );
    }
    #[test]
    fn funding_progress_distinguishes_unknown_signatures_dispatch_and_unacknowledged_finality() {
        let mut state = MintFundingState::default();
        assert_eq!(state.progress_stage(false).unwrap(), 0);
        assert_eq!(state.progress_stage(true).unwrap(), 1);
        // Only the structural WAL projection is under test. These inert payloads do not
        // pass production consent/proof/transaction/finality admission or create a loan.
        let clock = KagemushaOrdinaryCashClockContextV1 {
            version: 1,
            request_nonce: [1; 32],
            signed_observations_original_digest: [2; 32],
            lower_at_ms: 1,
            upper_at_ms: 2,
        };
        let rows = [
            MintFundingRecord::ConsentFence,
            MintFundingRecord::Consent([1; 64]),
            MintFundingRecord::PreDebit(vec![vec![1]; 8]),
            MintFundingRecord::PreDebitInvoked,
            MintFundingRecord::Decision {
                signed: vec![1],
                clock: vec![2],
                control: vec![3],
                data: vec![4],
            },
            MintFundingRecord::NodeSubmission(vec![1]),
            MintFundingRecord::TransactionFence { clock },
            MintFundingRecord::TransactionOriginal {
                canonical: vec![1],
                wire: vec![2],
            },
            MintFundingRecord::TransactionDispatched,
            MintFundingRecord::FinalizedOriginal(vec![1]),
            MintFundingRecord::FinalizedAcknowledged,
        ];
        for (index, row) in rows.into_iter().enumerate() {
            state.require_next(&row).unwrap();
            state.install(row);
            let stage = u8::try_from(index + 2).unwrap();
            assert_eq!(state.progress_stage(true).unwrap(), stage);
            assert!(state.progress_stage(false).is_err());
            if stage == 2 {
                assert!(
                    state
                        .require_next(&MintFundingRecord::ConsentFence)
                        .is_err()
                );
            }
            if stage == 8 {
                assert!(
                    state
                        .require_next(&MintFundingRecord::TransactionFence {
                            clock: KagemushaOrdinaryCashClockContextV1 {
                                version: 1,
                                request_nonce: [3; 32],
                                signed_observations_original_digest: [4; 32],
                                lower_at_ms: 3,
                                upper_at_ms: 4,
                            },
                        })
                        .is_err()
                );
            }
        }
    }
    #[test]
    fn funding_progress_rejects_orphan_ack_or_partial_transaction_original() {
        let orphan = MintFundingState {
            finalized_acknowledged: true,
            ..Default::default()
        };
        assert!(orphan.progress_stage(true).is_err());
        let partial = MintFundingState {
            transaction: Some(vec![1]),
            ..Default::default()
        };
        assert!(partial.progress_stage(true).is_err());
        let consent_without_fence = MintFundingState {
            consent: Some([1; 64]),
            ..Default::default()
        };
        assert!(consent_without_fence.progress_stage(true).is_err());
    }
    #[test]
    fn pending_finality_never_installs_ack_or_transaction_original() {
        let s = MintFundingState::default();
        assert!(
            s.require_next(&MintFundingRecord::TransactionDispatched)
                .is_err()
        );
        assert!(
            s.require_next(&MintFundingRecord::FinalizedOriginal(vec![1]))
                .is_err()
        );
        assert!(decode_transaction(&[]).is_err());
        assert!(decode_transaction(&[1, 2, 3]).is_err());
    }
}
