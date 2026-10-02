//! Dedicated ordinary Mint preparation in the actual Main WAL. Decoded data cannot fund State.
//! Native retains one-use recipient key/credit entropy before the sole platform invocation;
//! complete approval fsync precedes a separately measured bounded capture acknowledgment.
use super::*;
use crate::kagemusha_v1_crypto::seal_kagemusha_credit_v1_with_rng;
use iroha_crypto::kagemusha::kagemusha_x25519_public_key_v1;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1, KagemushaCreditOpeningV1,
    KagemushaOrdinaryFinancialHeadV1, KagemushaOrdinaryMintApprovalChallengeV1,
    KagemushaOrdinaryMintApprovalV1, KagemushaOrdinaryMintAuthorizationContextV1,
    KagemushaOrdinaryMintAuthorizationStatementV1, KagemushaSignedOrdinaryCurrentControlV1,
    kagemusha_ciphertext_digest_v1, kagemusha_mint_credit_opening_commitment_v1,
    kagemusha_recipient_credential_commitment_v1,
};
use rand::rand_core::{TryCryptoRng, TryRngCore};
use zeroize::{Zeroize as _, Zeroizing};

#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryMintSecretV1")]
struct MintSecret([u8; 32]);
impl Drop for MintSecret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryMintSealingEntropyV1")]
struct MintEntropy(Vec<u8>);
impl Drop for MintEntropy {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryMintOpeningV1")]
struct MintOpening(KagemushaCreditOpeningV1);
impl Drop for MintOpening {
    fn drop(&mut self) {
        self.0.credit_commitment_opening.zeroize();
        self.0.recipient_binding_opening.zeroize();
        self.0.recovery_nonce.zeroize();
    }
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryMintOriginalsV1")]
pub(super) struct MintOriginals {
    publication_originals: [DigestV1; 8],
    financial_control: CapturedFinancialControlIdentity,
    credential_original: Vec<u8>,
    lease_original: Option<Vec<u8>>,
    previous_counter: Option<u32>,
    statement: KagemushaOrdinaryMintAuthorizationStatementV1,
    challenge: KagemushaOrdinaryMintApprovalChallengeV1,
    request_capacity: u64,
    // Mandatory complete incoming Prepared frame allowance, captured before Mint reserve/fence.
    // An old/missing layout cannot enlarge an already fenced reservation or reconstruct custody.
    incoming_prepared_capacity: u64,
    private_key: MintSecret,
    opening: MintOpening,
    sealing_entropy: MintEntropy,
    encrypted_credit: Vec<u8>,
}
impl core::fmt::Debug for MintOriginals {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MintOriginals")
            .field("operation", &self.challenge.operation_id)
            .field("request_capacity", &self.request_capacity)
            .finish_non_exhaustive()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryMintRecordV1")]
pub(super) enum MintRecord {
    Reserve(MintOriginals),
    Fence {
        operation: DigestV1,
    },
    ApprovalOriginal {
        operation: DigestV1,
        original: Vec<u8>,
        lower: u64,
        upper: u64,
        counter: Option<u32>,
    },
    Capture {
        operation: DigestV1,
        lower: u64,
        upper: u64,
    },
    ProvenRequest {
        operation: DigestV1,
        original: Vec<u8>,
    },
    Cancel {
        operation: DigestV1,
    },
}
pub(super) struct PendingMint {
    originals: MintOriginals,
    lease: Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
    fenced: bool,
    retained: Option<(Vec<u8>, u64, u64)>,
    capture: Option<(u64, u64)>,
    proven_request: Option<Vec<u8>>,
}

/// Historical proof borrow from a durably acknowledged dedicated Mint, never a debit/current grant.
pub(crate) struct KagemushaAuthenticatedOrdinaryMintApprovalSelectionV1<'a> {
    owner: &'a KagemushaNativeOrdinaryCashOwnerV1,
    prefix: KagemushaRecoveryJournalPrefixV1,
}

impl MintOriginals {
    fn create(
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        amount: u128,
        control: CapturedFinancialControlIdentity,
    ) -> Result<Self, KagemushaStateErrorV1> {
        owner.require_current_financial_control()?;
        owner.require_initial_lineage_anchor_current()?;
        let financial = owner.publication.cash_financial();
        let enrollment = financial.enrollment();
        let c = enrollment.app_credential();
        let s = c.subject();
        let captured = owner
            .control
            .borrow_captured_proof_decision(
                financial,
                control.original_sha256,
                control.lower_ms,
                control.upper_ms,
            )
            .map_err(material)?;
        let raw_control = captured.original().map_err(material)?;
        let signed: KagemushaSignedOrdinaryCurrentControlV1 = norito::decode_canonical_with_limits(
            raw_control,
            norito::canonical_decode_limits(raw_control.len()),
        )
        .map_err(material)?;
        if norito::encode_canonical(&signed).map_err(material)? != raw_control {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let clock = financial.current_cash_clock_context().map_err(material)?;
        if clock.lower_at_ms < control.lower_ms || clock.upper_at_ms < control.upper_ms {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        let mut entropy = Zeroizing::new([0_u8; 248]);
        rand::rngs::OsRng
            .try_fill_bytes(entropy.as_mut())
            .map_err(material)?;
        let mut h = Sha256::new();
        h.update(b"iroha:kagemusha:v1:ordinary-native-mint-operation\0");
        h.update(owner.prefix.head);
        h.update(owner.prefix.sequence.to_le_bytes());
        h.update(owner.state.state_commitment);
        h.update(&entropy[..32]);
        let operation: DigestV1 = h.finalize().into();
        if operation == [0; 32] || owner.used_operations.contains(&operation) {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let private_key = MintSecret(entropy[64..96].try_into().map_err(material)?);
        let oneuse = kagemusha_x25519_public_key_v1(&private_key.0).map_err(material)?;
        let rt = &owner.initial_lineage_anchor.lineage.owner.runtime;
        let recipient_opening: DigestV1 = entropy[128..160].try_into().map_err(material)?;
        let credit_opening: DigestV1 = entropy[96..128].try_into().map_err(material)?;
        let context = KagemushaOrdinaryMintAuthorizationContextV1 {
            version: 1,
            operation_id: operation,
            lineage: owner.initial_lineage_anchor.lineage.clone(),
            predecessor: KagemushaOrdinaryFinancialHeadV1 {
                state_commitment: owner.state.state_commitment,
                logical_sequence: owner.state.logical_sequence,
                state_original_sha256: Sha256::digest(&owner.public_state_original).into(),
            },
            release_id: owner.state.release_id,
            suite_id: owner.state.suite_id,
            vk_digest: owner.state.vk_digest,
            artifact_manifest_digest: admitted_release(&owner.verifier)?.manifest_digest(),
            recipient_app_credential_digest: c.digest(),
            app_credential_profile_id: s.hardware_profile_id,
            policy_epoch: s.policy_epoch,
            amount,
            recipient_credential_commitment: kagemusha_recipient_credential_commitment_v1(
                operation,
                c.digest(),
                recipient_opening,
            )
            .map_err(material)?,
            credit_commitment: kagemusha_mint_credit_opening_commitment_v1(
                &rt.network_id,
                &rt.asset,
                rt.asset_incarnation,
                rt.scale,
                owner.state.liability_pool_id,
                amount,
                &owner.initial_lineage_anchor.lineage.owner.account_id,
                oneuse,
                credit_opening,
            )
            .map_err(material)?,
            recipient_one_time_key: oneuse,
            clock_context: clock,
            financial_control_original_sha256: control.original_sha256,
        };
        context.validate_against_credential(c).map_err(material)?;
        let capacity = crate::kagemusha_v1_recursion::ordinary_mint_request_byte_budget_v1(
            &owner.verifier,
            &context,
            c,
        )
        .map_err(material)?
        .maximum_original_bytes();
        let opening = MintOpening(KagemushaCreditOpeningV1 {
            version: 1,
            credit_id: context.credit_id().map_err(material)?,
            amount,
            credit_commitment_opening: credit_opening,
            recipient_binding_opening: recipient_opening,
            recovery_nonce: entropy[160..192].try_into().map_err(material)?,
        });
        let sealing_entropy = MintEntropy(entropy[192..].to_vec());
        let encrypted_credit = seal(&context, &opening.0, &sealing_entropy.0)?;
        let statement = KagemushaOrdinaryMintAuthorizationStatementV1 {
            version: 1,
            issuance_commitment: context.issuance_commitment().map_err(material)?,
            credit_id: context.credit_id().map_err(material)?,
            ciphertext_digest: kagemusha_ciphertext_digest_v1(&encrypted_credit),
            context,
        };
        let challenge = KagemushaOrdinaryMintApprovalChallengeV1 {
            version: 1,
            operation_id: operation,
            nonce: entropy[32..64].try_into().map_err(material)?,
            credential_digest: c.digest(),
            statement_digest: statement.binding_digest().map_err(material)?,
            clock_context_digest: clock.binding_digest().map_err(material)?,
            financial_control_original_sha256: control.original_sha256,
            issued_at_ms: clock.lower_at_ms,
            expires_at_ms: clock
                .lower_at_ms
                .checked_add(KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1)
                .ok_or(KagemushaStateErrorV1::InvalidTrustedCommitTime)?
                .min(owner.credential_floor()?.approval_valid_until_ms())
                .min(signed.subject.expires_at_ms),
        };
        let incoming_prepared_capacity =
            super::incoming_preparation::mint_incoming_prepared_byte_budget_v1(
                owner,
                &statement.context,
            )?;
        let this = Self {
            publication_originals: owner.publication.historical_original_commitments()?,
            financial_control: control,
            credential_original: c.original().to_vec(),
            lease_original: financial
                .retained_integrity_lease()
                .map(|l| l.original().to_vec()),
            previous_counter: owner.counter_floor,
            statement,
            challenge,
            request_capacity: u64::try_from(capacity).map_err(material)?,
            incoming_prepared_capacity,
            private_key,
            opening,
            sealing_entropy,
            encrypted_credit,
        };
        this.recheck_originals(
            owner,
            financial.retained_integrity_lease().map(Arc::as_ref),
            true,
        )?;
        this.require_at(
            owner,
            financial.retained_integrity_lease().map(Arc::as_ref),
            financial
                .trusted_time_interval()
                .map_err(material)?
                .lower_ms(),
        )?;
        this.require_at(
            owner,
            financial.retained_integrity_lease().map(Arc::as_ref),
            financial
                .trusted_time_interval()
                .map_err(material)?
                .upper_ms(),
        )?;
        owner.require_current_financial_control()?;
        Ok(this)
    }
    fn recheck_originals(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
        require_floor: bool,
    ) -> Result<(), KagemushaStateErrorV1> {
        let financial = owner.publication.cash_financial();
        let c = financial.enrollment().app_credential();
        let ctx = &self.statement.context;
        if self.publication_originals != owner.publication.historical_original_commitments()?
            || self.credential_original != c.original()
            || self.lease_original.as_deref() != lease.map(|l| l.original())
            || (require_floor && self.previous_counter != owner.counter_floor)
            || ctx.lineage != owner.initial_lineage_anchor.lineage
            || ctx.predecessor.state_commitment != owner.state.state_commitment
            || ctx.predecessor.logical_sequence != owner.state.logical_sequence
            || ctx.predecessor.state_original_sha256
                != <DigestV1>::from(Sha256::digest(&owner.public_state_original))
            || ctx.financial_control_original_sha256 != self.financial_control.original_sha256
            || ctx.clock_context.lower_at_ms < self.financial_control.lower_ms
            || ctx.clock_context.upper_at_ms < self.financial_control.upper_ms
            || ctx.recipient_one_time_key
                != kagemusha_x25519_public_key_v1(&self.private_key.0).map_err(material)?
            || self.encrypted_credit != seal(ctx, &self.opening.0, &self.sealing_entropy.0)?
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        ctx.validate_against_credential(c).map_err(material)?;
        ctx.validate_credit_opening(&self.opening.0)
            .map_err(material)?;
        self.statement
            .validate_encrypted_credit(&self.encrypted_credit)
            .map_err(material)?;
        self.challenge
            .validate_against_statement(&self.statement)
            .map_err(material)?;
        let actual_capacity = crate::kagemusha_v1_recursion::ordinary_mint_request_byte_budget_v1(
            &owner.verifier,
            ctx,
            c,
        )
        .map_err(material)?
        .maximum_original_bytes();
        if self.request_capacity != u64::try_from(actual_capacity).map_err(material)?
            || self.incoming_prepared_capacity
                != super::incoming_preparation::mint_incoming_prepared_byte_budget_v1(
                    owner,
                    &self.statement.context,
                )?
        {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        let retained = owner
            .control
            .borrow_captured_proof_decision(
                financial,
                self.financial_control.original_sha256,
                self.financial_control.lower_ms,
                self.financial_control.upper_ms,
            )
            .map_err(material)?;
        retained.recheck_historical_originals().map_err(material)?;
        let (control_issued, control_expires) = owner
            .control
            .recheck_retained_capture_original_window(
                financial,
                self.financial_control.original_sha256,
                self.financial_control.lower_ms,
                self.financial_control.upper_ms,
            )
            .map_err(material)?;
        if self.challenge.issued_at_ms < control_issued
            || self.challenge.expires_at_ms > control_expires
            || ctx.release_id != owner.state.release_id
            || ctx.suite_id != owner.state.suite_id
            || ctx.vk_digest != owner.state.vk_digest
            || ctx.artifact_manifest_digest != admitted_release(&owner.verifier)?.manifest_digest()
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }

        let clock = financial
            .verified_retained_cash_clock_originals(&ctx.clock_context)
            .map_err(material)?;
        clock
            .recheck_cash_context(&ctx.clock_context)
            .map_err(material)?;
        self.require_at(owner, lease, ctx.clock_context.lower_at_ms)?;
        self.require_at(owner, lease, ctx.clock_context.upper_at_ms)
    }
    fn require_at(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
        now: u64,
    ) -> Result<(), KagemushaStateErrorV1> {
        if now < self.challenge.issued_at_ms || now >= self.challenge.expires_at_ms {
            return Err(KagemushaStateErrorV1::InvalidTrustedCommitTime);
        }
        let enrollment = owner.publication.cash_financial().enrollment();
        match lease {
            Some(l) => enrollment.recheck_with_integrity_lease(l, now),
            None => enrollment.recheck_at_trusted_time(now),
        }
        .map_err(material)
    }
    fn authenticate(
        &self,
        owner: &KagemushaNativeOrdinaryCashOwnerV1,
        lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
        raw: &[u8],
        lower: u64,
        upper: u64,
    ) -> Result<Option<u32>, KagemushaStateErrorV1> {
        if raw.is_empty()
            || raw.len() > KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1
            || lower == 0
            || lower > upper
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let approval: KagemushaOrdinaryMintApprovalV1 =
            norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
                .map_err(material)?;
        if approval.canonical_bytes().map_err(material)? != raw
            || approval.challenge != self.challenge
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.require_at(owner, lease, lower)?;
        self.require_at(owner, lease, upper)?;
        approval
            .authenticate_platform_equation(
                owner
                    .publication
                    .cash_financial()
                    .enrollment()
                    .app_credential(),
                self.previous_counter,
            )
            .map(|p| p.0)
            .map_err(material)
    }
    fn capacity_charge(&self) -> Result<u64, KagemushaStateErrorV1> {
        let raw = Zeroizing::new(norito::encode_canonical(self).map_err(material)?);
        u64::try_from(raw.len())
            .map_err(material)?
            .checked_add(self.request_capacity)
            .and_then(|n| n.checked_add(self.incoming_prepared_capacity))
            .and_then(|n| n.checked_add(1024))
            .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)
    }
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    /// Reserve a dedicated Mint attempt; all entropy and ciphertext are retained before signing.
    /// This selects no finalized source and changes no financial State.
    pub(crate) fn reserve_mint_request(
        &mut self,
        amount: u128,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if let Some(p) = &self.pending_mint {
            if p.originals.statement.context.amount != amount {
                return Err(KagemushaStateErrorV1::InvalidCandidateStage);
            }
            p.originals
                .recheck_originals(self, p.lease.as_deref(), p.retained.is_none())?;
            return Ok(p.originals.challenge.operation_id);
        }
        self.require_incoming_rows(20)?;
        if self.pending_incoming.is_some()
            || self.pending.is_some()
            || self.pending_receiver_request.is_some()
            || self.terminal.as_ref().is_none_or(|t| t.has_pending())
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let captured = self
            .control
            .capture_proof_decision(self.publication.cash_financial())
            .map_err(material)?;
        let identity = CapturedFinancialControlIdentity {
            original_sha256: captured.original_sha256().map_err(material)?,
            lower_ms: captured.captured_lower_ms(),
            upper_ms: captured.captured_upper_ms(),
        };
        let originals = MintOriginals::create(self, amount, identity)?;
        if self.mint_capacity_charge(&originals)? > self.capacity.inbox_bytes {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        let lease = self
            .publication
            .cash_financial()
            .retained_integrity_lease()
            .cloned();
        let operation = originals.challenge.operation_id;
        self.persist(&Record::Mint(MintRecord::Reserve(originals.clone())))?;
        self.used_operations.insert(operation);
        self.pending_mint = Some(PendingMint {
            originals,
            lease,
            fenced: false,
            retained: None,
            capture: None,
            proven_request: None,
        });
        self.require_current_financial_control()?;
        Ok(operation)
    }
    pub(crate) fn mint_approval_signing_message(
        &self,
        operation: DigestV1,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.require_live_mint(operation)?;
        let p = self
            .pending_mint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if p.fenced {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        p.originals
            .challenge
            .canonical_signing_bytes()
            .map_err(material)
    }
    pub(crate) fn fence_mint_platform(
        &mut self,
        operation: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_mint(operation)?;
        if self.pending_mint.as_ref().is_none_or(|p| p.fenced) {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.persist(&Record::Mint(MintRecord::Fence { operation }))?;
        self.pending_mint
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .fenced = true;
        self.require_live_mint(operation)
    }
    pub(crate) fn capture_mint_approval_original(
        &mut self,
        operation: DigestV1,
        raw: &[u8],
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_mint(operation)?;
        let p = self
            .pending_mint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if !p.fenced || p.retained.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let interval = self
            .publication
            .cash_financial()
            .trusted_time_interval()
            .map_err(material)?;
        let counter = p.originals.authenticate(
            self,
            p.lease.as_deref(),
            raw,
            interval.lower_ms(),
            interval.upper_ms(),
        )?;
        self.persist(&Record::Mint(MintRecord::ApprovalOriginal {
            operation,
            original: raw.to_vec(),
            lower: interval.lower_ms(),
            upper: interval.upper_ms(),
            counter,
        }))?;
        self.counter_floor = counter.or(self.counter_floor);
        self.pending_mint
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .retained = Some((raw.to_vec(), interval.lower_ms(), interval.upper_ms()));
        self.acknowledge_mint_capture(operation)
    }
    pub(crate) fn acknowledge_mint_capture(
        &mut self,
        operation: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_mint(operation)?;
        let p = self
            .pending_mint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let (raw, old_lower, old_upper) = p
            .retained
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if p.capture.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let interval = self
            .publication
            .cash_financial()
            .trusted_time_interval()
            .map_err(material)?;
        if interval.lower_ms() < *old_lower || interval.upper_ms() < *old_upper {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        p.originals.authenticate(
            self,
            p.lease.as_deref(),
            raw,
            interval.lower_ms(),
            interval.upper_ms(),
        )?;
        self.persist(&Record::Mint(MintRecord::Capture {
            operation,
            lower: interval.lower_ms(),
            upper: interval.upper_ms(),
        }))?;
        self.pending_mint
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .capture = Some((interval.lower_ms(), interval.upper_ms()));
        self.captured_mint_selection()?.recheck()
    }
    pub(crate) fn cancel_uninvoked_mint(
        &mut self,
        operation: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_mint(operation)?;
        if self.pending_mint.as_ref().is_none_or(|p| p.fenced) {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.persist(&Record::Mint(MintRecord::Cancel { operation }))?;
        self.pending_mint = None;
        self.require_current_financial_control()
    }
    pub(crate) fn captured_mint_selection(
        &self,
    ) -> Result<KagemushaAuthenticatedOrdinaryMintApprovalSelectionV1<'_>, KagemushaStateErrorV1>
    {
        let loan = KagemushaAuthenticatedOrdinaryMintApprovalSelectionV1 {
            owner: self,
            prefix: self.prefix,
        };
        loan.recheck()?;
        Ok(loan)
    }
    fn require_live_mint(&self, operation: DigestV1) -> Result<(), KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let p = self
            .pending_mint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if p.originals.challenge.operation_id != operation {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        p.originals
            .recheck_originals(self, p.lease.as_deref(), p.retained.is_none())?;
        let interval = self
            .publication
            .cash_financial()
            .trusted_time_interval()
            .map_err(material)?;
        p.originals
            .require_at(self, p.lease.as_deref(), interval.lower_ms())?;
        p.originals
            .require_at(self, p.lease.as_deref(), interval.upper_ms())
    }
    fn mint_capacity_charge(
        &self,
        originals: &MintOriginals,
    ) -> Result<u64, KagemushaStateErrorV1> {
        // The same descriptor-held inbox also retains complete independently admitted
        // received sources. Their real full-original/framing charge must be reserved
        // before Mint's platform fence, and checked again during chronological replay.
        let mut sum = self
            .retained_received_source_capacity_charge()?
            .checked_add(originals.capacity_charge()?)
            .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
        for retained in self.retained_receiver_requests.values() {
            sum = sum
                .checked_add(retained.captured.reservation().capacity_charge_bytes()?)
                .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
        }
        Ok(sum)
    }
    /// Require a complete incoming Prepared frame under this exact pre-fence whole-chronology reservation.
    /// Numeric capacity never admits source/finality/current authority or exposes private originals.
    pub(super) fn require_mint_incoming_prepared_capacity(
        &self,
        actual_frame_bytes: u64,
    ) -> Result<(), KagemushaStateErrorV1> {
        let p = self
            .pending_mint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let expected = super::incoming_preparation::mint_incoming_prepared_byte_budget_v1(
            self,
            &p.originals.statement.context,
        )?;
        super::incoming_preparation::require_recorded_allowance_v1(
            p.originals.incoming_prepared_capacity,
            expected,
        )?;
        super::incoming_preparation::require_prepared_frame_quota_v1(
            actual_frame_bytes,
            p.originals.incoming_prepared_capacity,
            self.mint_capacity_charge(&p.originals)?,
            self.capacity.inbox_bytes,
            self.maximum_record_payload_bytes,
        )
    }
    /// Retain only the genuinely admitted exact complete request, without current debit authority.
    pub(crate) fn retain_proven_mint_request(
        &mut self,
        proved: &crate::kagemusha_v1_recursion::KagemushaVerifiedOrdinaryMintAuthorizationV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let selected = self.captured_mint_selection()?;
        selected.recheck()?;
        let operation = selected.statement()?.context.operation_id;
        self.readmit_selected_mint_request(proved.request_original())?;
        let p = self
            .pending_mint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if let Some(original) = &p.proven_request {
            return if original == proved.request_original() {
                Ok(())
            } else {
                Err(KagemushaStateErrorV1::SnapshotIntegrity)
            };
        }
        self.persist(&Record::Mint(MintRecord::ProvenRequest {
            operation,
            original: proved.request_original().to_vec(),
        }))?;
        self.pending_mint
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .proven_request = Some(proved.request_original().to_vec());
        self.captured_mint_selection()?.recheck()
    }
    pub(super) fn retained_predebit_request_original(
        &self,
    ) -> Result<&[u8], KagemushaStateErrorV1> {
        self.pending_mint
            .as_ref()
            .and_then(|p| p.proven_request.as_deref())
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }
    /// Re-admit full retained proofs without a resolver/proving key or fresh randomized proof.
    pub(crate) fn verified_retained_mint_request(
        &self,
    ) -> Result<
        crate::kagemusha_v1_recursion::KagemushaVerifiedOrdinaryMintAuthorizationV1,
        KagemushaStateErrorV1,
    > {
        self.captured_mint_selection()?.recheck()?;
        let original = self
            .pending_mint
            .as_ref()
            .and_then(|p| p.proven_request.as_deref())
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let proved = self.readmit_selected_mint_request(original)?;
        self.captured_mint_selection()?.recheck()?;
        Ok(proved)
    }
    pub(super) fn readmit_selected_mint_request(
        &self,
        raw: &[u8],
    ) -> Result<
        crate::kagemusha_v1_recursion::KagemushaVerifiedOrdinaryMintAuthorizationV1,
        KagemushaStateErrorV1,
    > {
        let p = self
            .pending_mint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let (lower, upper) = p.capture.ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let (approval, _, _) = p
            .retained
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if !p.fenced
            || raw.len() > usize::try_from(p.originals.request_capacity).map_err(material)?
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        p.originals
            .recheck_originals(self, p.lease.as_deref(), false)?;
        p.originals
            .authenticate(self, p.lease.as_deref(), approval, lower, upper)?;
        let request =
            iroha_data_model::kagemusha::KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(
                raw,
            )
            .map_err(material)?;
        if request.authorization.statement != p.originals.statement
            || request
                .authorization
                .approval
                .canonical_bytes()
                .map_err(material)?
                != *approval
            || request.encrypted_credit != p.originals.encrypted_credit
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let financial = self.publication.cash_financial();
        let clock = financial
            .verified_retained_cash_clock_originals(&p.originals.statement.context.clock_context)
            .map_err(material)?;
        crate::kagemusha_v1_recursion::verify_ordinary_mint_authorization_v1(
            &self.verifier,
            raw,
            financial.enrollment().app_credential(),
            p.lease.as_deref(),
            &clock,
        )
        .map_err(material)
    }
    pub(super) fn replay_mint(
        &mut self,
        record: MintRecord,
        leases: &[Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>],
    ) -> Result<(), KagemushaStateErrorV1> {
        match record {
            MintRecord::Reserve(originals) => {
                if self.pending_mint.is_some()
                    || self.pending_incoming.is_some()
                    || self.pending.is_some()
                    || self.pending_receiver_request.is_some()
                    || self.terminal.as_ref().is_none_or(|t| t.has_pending())
                    || self
                        .used_operations
                        .contains(&originals.challenge.operation_id)
                    || self.anchor_request_sha256.is_none()
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                self.require_state_advance_acknowledged()?;
                let lease = match originals.lease_original.as_deref() {
                    None => None,
                    Some(raw) => Some(Arc::clone(
                        leases
                            .iter()
                            .find(|l| l.original() == raw)
                            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
                    )),
                };
                originals.recheck_originals(self, lease.as_deref(), true)?;
                if self.mint_capacity_charge(&originals)? > self.capacity.inbox_bytes {
                    return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
                }
                self.used_operations
                    .insert(originals.challenge.operation_id);
                self.pending_mint = Some(PendingMint {
                    originals,
                    lease,
                    fenced: false,
                    retained: None,
                    capture: None,
                    proven_request: None,
                });
            }
            MintRecord::Fence { operation } => {
                let p = self
                    .pending_mint
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if p.originals.challenge.operation_id != operation || p.fenced {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                p.fenced = true;
            }
            MintRecord::ApprovalOriginal {
                operation,
                original,
                lower,
                upper,
                counter,
            } => {
                let p = self
                    .pending_mint
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if p.originals.challenge.operation_id != operation
                    || !p.fenced
                    || p.retained.is_some()
                    || p.capture.is_some()
                    || p.originals.authenticate(
                        self,
                        p.lease.as_deref(),
                        &original,
                        lower,
                        upper,
                    )? != counter
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                self.counter_floor = counter.or(self.counter_floor);
                self.pending_mint
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .retained = Some((original, lower, upper));
            }
            MintRecord::Capture {
                operation,
                lower,
                upper,
            } => {
                let p = self
                    .pending_mint
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                let (raw, l, u) = p
                    .retained
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if p.originals.challenge.operation_id != operation
                    || !p.fenced
                    || p.capture.is_some()
                    || lower < *l
                    || upper < *u
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                p.originals
                    .authenticate(self, p.lease.as_deref(), raw, lower, upper)?;
                self.pending_mint
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .capture = Some((lower, upper));
            }
            MintRecord::ProvenRequest {
                operation,
                original,
            } => {
                let p = self
                    .pending_mint
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if p.originals.challenge.operation_id != operation || p.proven_request.is_some() {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                self.readmit_selected_mint_request(&original)?;
                self.pending_mint
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .proven_request = Some(original);
            }
            MintRecord::Cancel { operation } => {
                let p = self
                    .pending_mint
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if p.originals.challenge.operation_id != operation || p.fenced {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                self.pending_mint = None;
            }
        }
        Ok(())
    }
}

impl KagemushaAuthenticatedOrdinaryMintApprovalSelectionV1<'_> {
    pub(crate) fn recheck(&self) -> Result<(), KagemushaStateErrorV1> {
        let o = self.owner;
        if o.recovery_catalog.is_some() || o.recovery_failed || o.prefix != self.prefix {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        o.publication.recheck_historical_cash_custody()?;
        o.recheck_lineage_retained_custody()?;
        o.journal.check_owned().map_err(storage)?;
        if o.journal.recovery_prefix().map_err(storage)? != self.prefix {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let p = o
            .pending_mint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let (lower, upper) = p
            .capture
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let (raw, _, _) = p
            .retained
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if !p.fenced {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        p.originals
            .recheck_originals(o, p.lease.as_deref(), false)?;
        p.originals
            .authenticate(o, p.lease.as_deref(), raw, lower, upper)?;
        Ok(())
    }
    fn pending(&self) -> Result<&PendingMint, KagemushaStateErrorV1> {
        self.owner
            .pending_mint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }
    pub(crate) fn statement(
        &self,
    ) -> Result<&KagemushaOrdinaryMintAuthorizationStatementV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(&self.pending()?.originals.statement)
    }
    pub(crate) fn approval_original(&self) -> Result<&[u8], KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(&self
            .pending()?
            .retained
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .0)
    }
    pub(crate) fn selected_integrity_lease(
        &self,
    ) -> Result<Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(self.pending()?.lease.as_deref())
    }
    pub(crate) fn previous_app_attest_counter(&self) -> Result<Option<u32>, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(self.pending()?.originals.previous_counter)
    }
    pub(crate) fn encrypted_credit(&self) -> Result<&[u8], KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(&self.pending()?.originals.encrypted_credit)
    }
    pub(crate) fn enrollment(
        &self,
    ) -> Result<&KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1, KagemushaStateErrorV1>
    {
        self.recheck()?;
        Ok(self.owner.publication.cash_financial().enrollment())
    }
    pub(crate) fn authenticated_release(
        &self,
    ) -> Result<
        Arc<iroha_data_model::kagemusha::KagemushaAuthenticatedReleaseV1>,
        KagemushaStateErrorV1,
    > {
        self.recheck()?;
        admitted_release(&self.owner.verifier)
    }
    pub(crate) fn verifier(
        &self,
    ) -> Result<&KagemushaAuthenticatedRecursiveVerifierV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(&self.owner.verifier)
    }
    pub(crate) fn request_capacity(&self) -> Result<usize, KagemushaStateErrorV1> {
        self.recheck()?;
        usize::try_from(self.pending()?.originals.request_capacity).map_err(material)
    }
    pub(crate) fn financial_control_original(&self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.recheck()?;
        let i = self.pending()?.originals.financial_control;
        let f = self.owner.publication.cash_financial();
        let cap = self
            .owner
            .control
            .borrow_captured_proof_decision(f, i.original_sha256, i.lower_ms, i.upper_ms)
            .map_err(material)?;
        let raw = cap.original().map_err(material)?.to_vec();
        self.recheck()?;
        Ok(raw)
    }
    pub(crate) fn with_verified_preparation_clock(
        &self,
        visitor: &mut dyn for<'clock> FnMut(
            &'clock KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck()?;
        let clock = self
            .owner
            .publication
            .cash_financial()
            .verified_retained_cash_clock_originals(
                &self.pending()?.originals.statement.context.clock_context,
            )
            .map_err(material)?;
        visitor(&clock)?;
        self.recheck()
    }
    pub(crate) fn with_borrowed_mint_secrets(
        &self,
        visitor: &mut dyn for<'secret> FnMut(
            &'secret [u8; 32],
            &'secret KagemushaCreditOpeningV1,
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck()?;
        let p = self.pending()?;
        let f = self.owner.publication.cash_financial();
        let i = p.originals.financial_control;
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
        visitor(secret, &p.originals.opening.0)?;
        self.recheck()
    }
}

fn seal(
    context: &KagemushaOrdinaryMintAuthorizationContextV1,
    opening: &KagemushaCreditOpeningV1,
    entropy: &[u8],
) -> Result<Vec<u8>, KagemushaStateErrorV1> {
    if entropy.len() != 56 {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    context.validate_credit_opening(opening).map_err(material)?;
    let mut rng = RetainedMintEntropy {
        bytes: entropy,
        offset: 0,
    };
    let envelope = seal_kagemusha_credit_v1_with_rng(
        opening,
        &context.encrypted_credit_aad().map_err(material)?,
        context.recipient_one_time_key,
        &mut rng,
    )
    .map_err(material)?;
    if rng.offset != 56 {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    envelope
        .canonical_bytes_against_recipient_key(context.recipient_one_time_key)
        .map_err(material)
}
struct RetainedMintEntropy<'a> {
    bytes: &'a [u8],
    offset: usize,
}
#[derive(Debug)]
struct Exhausted;
impl core::fmt::Display for Exhausted {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("retained Mint entropy exhausted")
    }
}
impl TryRngCore for RetainedMintEntropy<'_> {
    type Error = Exhausted;
    fn try_next_u32(&mut self) -> Result<u32, Exhausted> {
        let mut b = [0; 4];
        self.try_fill_bytes(&mut b)?;
        Ok(u32::from_le_bytes(b))
    }
    fn try_next_u64(&mut self) -> Result<u64, Exhausted> {
        let mut b = [0; 8];
        self.try_fill_bytes(&mut b)?;
        Ok(u64::from_le_bytes(b))
    }
    fn try_fill_bytes(&mut self, d: &mut [u8]) -> Result<(), Exhausted> {
        let end = self.offset.checked_add(d.len()).ok_or(Exhausted)?;
        d.copy_from_slice(self.bytes.get(self.offset..end).ok_or(Exhausted)?);
        self.offset = end;
        Ok(())
    }
}
impl TryCryptoRng for RetainedMintEntropy<'_> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha_v1_crypto::open_kagemusha_credit_v1;
    use iroha_data_model::kagemusha::KagemushaEncryptedCreditEnvelopeV1;
    fn data() -> (
        KagemushaOrdinaryMintAuthorizationContextV1,
        KagemushaCreditOpeningV1,
        MintSecret,
    ) {
        let fixture =
            iroha_data_model::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_v1();
        let mut context = fixture.request.authorization.statement.context;
        let key = MintSecret([31; 32]);
        context.recipient_one_time_key = kagemusha_x25519_public_key_v1(&key.0).unwrap();
        let rt = &context.lineage.owner.runtime;
        context.credit_commitment = kagemusha_mint_credit_opening_commitment_v1(
            &rt.network_id,
            &rt.asset,
            rt.asset_incarnation,
            rt.scale,
            iroha_data_model::kagemusha::kagemusha_liability_pool_id_v1(
                &rt.network_id,
                &rt.asset,
                rt.asset_incarnation,
            )
            .unwrap(),
            context.amount,
            &context.lineage.owner.account_id,
            context.recipient_one_time_key,
            [50; 32],
        )
        .unwrap();
        let opening = KagemushaCreditOpeningV1 {
            version: 1,
            credit_id: context.credit_id().unwrap(),
            amount: context.amount,
            credit_commitment_opening: [50; 32],
            recipient_binding_opening: [51; 32],
            recovery_nonce: [52; 32],
        };
        context.validate_credit_opening(&opening).unwrap();
        (context, opening, key)
    }
    #[test]
    fn mint_crypto_original_reproduces_exact_ciphertext_and_opens_same_actual_plaintext() {
        let (context, opening, key) = data();
        let mut entropy = [29; 56];
        entropy[32..].fill(30);
        let original = seal(&context, &opening, &entropy).unwrap();
        assert_eq!(
            original.len(),
            iroha_data_model::kagemusha::KAGEMUSHA_ENCRYPTED_CREDIT_CANONICAL_BYTES_V1
        );
        assert_eq!(seal(&context, &opening, &entropy).unwrap(), original);
        let envelope =
            KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
                &original,
                context.recipient_one_time_key,
            )
            .unwrap();
        let plain = open_kagemusha_credit_v1(
            &envelope,
            &context.encrypted_credit_aad().unwrap(),
            context.recipient_one_time_key,
            &key.0,
        )
        .unwrap();
        assert_eq!(
            plain.canonical_bytes().unwrap(),
            opening.canonical_bytes().unwrap()
        );
        // X25519 clamps the low three scalar bits. Change an effective scalar
        // bit, then independently change the AEAD nonce; both must affect bytes.
        for index in [1, 32] {
            let mut changed = entropy;
            changed[index] ^= 1;
            assert_ne!(seal(&context, &opening, &changed).unwrap(), original);
        }
        // Transport padding/truncation is never part of the actual AEAD original.
        let mut padded = original.clone();
        padded.resize(
            iroha_data_model::kagemusha::KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1,
            0,
        );
        assert!(
            KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
                &padded,
                context.recipient_one_time_key,
            )
            .is_err()
        );
        for end in 0..original.len() {
            assert!(
                KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
                    &original[..end],
                    context.recipient_one_time_key,
                )
                .is_err()
            );
        }
        let mut suffix = original.clone();
        suffix.push(0);
        assert!(
            KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
                &suffix,
                context.recipient_one_time_key,
            )
            .is_err()
        );
        let mut clamped = entropy;
        clamped[0] ^= 1;
        assert_eq!(seal(&context, &opening, &clamped).unwrap(), original);
        let mut changed = entropy;
        // X25519 discards the lowest three scalar bits; change an effective bit.
        changed[0] ^= 0x08;
        assert_ne!(seal(&context, &opening, &changed).unwrap(), original);
    }
    #[test]
    fn mint_crypto_original_rejects_foreign_context_key_and_credit_openings() {
        let (context, opening, key) = data();
        let raw = seal(&context, &opening, &[29; 56]).unwrap();
        let envelope =
            KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
                &raw,
                context.recipient_one_time_key,
            )
            .unwrap();
        let mut changed = context.clone();
        changed.financial_control_original_sha256[0] ^= 1;
        assert!(
            open_kagemusha_credit_v1(
                &envelope,
                &changed.encrypted_credit_aad().unwrap(),
                context.recipient_one_time_key,
                &key.0
            )
            .is_err()
        );
        let mut changed_opening = opening.clone();
        changed_opening.recipient_binding_opening[0] ^= 1;
        assert!(seal(&context, &changed_opening, &[29; 56]).is_err());
        assert!(
            open_kagemusha_credit_v1(
                &envelope,
                &context.encrypted_credit_aad().unwrap(),
                context.recipient_one_time_key,
                &[32; 32]
            )
            .is_err()
        );
        assert!(seal(&context, &opening, &[29; 55]).is_err());
    }
    #[test]
    fn mint_separate_approval_data_rejects_generic_cash_challenge() {
        let fixture =
            iroha_data_model::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_v1();
        let raw = fixture
            .request
            .authorization
            .approval
            .canonical_bytes()
            .unwrap();
        let decoded: KagemushaOrdinaryMintApprovalV1 =
            norito::decode_canonical_with_limits(&raw, norito::canonical_decode_limits(raw.len()))
                .unwrap();
        assert_eq!(decoded.canonical_bytes().unwrap(), raw);
        let generic = norito::decode_canonical_with_limits::<KagemushaAppOperationApprovalV1>(
            &raw,
            norito::canonical_decode_limits(raw.len()),
        );
        assert!(generic.is_err() || norito::encode_canonical(&generic.unwrap()).unwrap() != raw);
        let mut trailing = raw;
        trailing.push(0);
        let decoded = norito::decode_canonical_with_limits::<KagemushaOrdinaryMintApprovalV1>(
            &trailing,
            norito::canonical_decode_limits(trailing.len()),
        );
        assert!(decoded.is_err() || decoded.unwrap().canonical_bytes().unwrap() != trailing);
    }
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    pub(super) fn retained_mint_source_control_identity(
        &self,
    ) -> Result<CapturedFinancialControlIdentity, KagemushaStateErrorV1> {
        let p = self
            .pending_mint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if p.capture.is_none() || p.proven_request.is_none() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        p.originals
            .recheck_originals(self, p.lease.as_deref(), false)?;
        Ok(p.originals.financial_control)
    }
}

impl PendingMint {
    pub(super) fn capacity_charge_bytes(&self) -> Result<u64, KagemushaStateErrorV1> {
        self.originals.capacity_charge()
    }
}
impl KagemushaNativeOrdinaryCashOwnerV1 {
    pub(super) fn require_held_mint_incoming_source(
        &self,
        reservation: &iroha_data_model::kagemusha::KagemushaOrdinaryIncomingReservationV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let p = self
            .pending_mint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let request = p
            .proven_request
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let expected = match reservation.selection.source {
            iroha_data_model::kagemusha::KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
                topup_request_original_sha256,
            } => topup_request_original_sha256,
            _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
        };
        if !p.fenced
            || p.capture.is_none()
            || p.retained.is_none()
            || <DigestV1>::from(Sha256::digest(request)) != expected
            || p.originals.statement.context.operation_id != reservation.selection.operation_id
            || p.originals.opening.0.credit_id != reservation.selection.credit_id
            || p.originals.opening.0.amount != reservation.selection.amount
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
}
