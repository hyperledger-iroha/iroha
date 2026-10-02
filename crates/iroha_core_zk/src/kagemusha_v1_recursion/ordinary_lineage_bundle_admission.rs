//! Sole bounded public State/Guard bundle and closed Anchor/Reserve selector joins.
//! DATA custody, current FI/PI and certified issuer purpose remain in the service owner.
use super::*;
use crate::kagemusha_v1_state::{BootstrapStatementV1, TransitionProofStatementV1};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1, KAGEMUSHA_ENCRYPTED_CREDIT_CANONICAL_BYTES_V1,
    KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1, KagemushaOperationKindV1,
    KagemushaOrdinaryCashClockContextV1, KagemushaOrdinaryFinancialLineageV1,
    KagemushaOrdinaryLineageAnchorV1, KagemushaOrdinaryLineageReservationV1,
    KagemushaOrdinaryPaymentOutputV1, KagemushaOrdinaryPaymentRequestV1,
    KagemushaOrdinaryRedemptionOutputV1, KagemushaOutboxReservationV1,
    kagemusha_asset_identity_digest_v1, kagemusha_ciphertext_digest_v1,
    kagemusha_ordinary_app_account_binding_v1, kagemusha_ordinary_payment_body_digest_v1,
    kagemusha_ordinary_transition_nullifier_v1,
};
#[path = "ordinary_lineage_commit_admission.rs"]
mod commit_admission;
pub(in crate::kagemusha_v1_recursion) use commit_admission::terminal_public;
pub use commit_admission::{
    KagemushaOrdinaryCashOutgoingOriginalV1, KagemushaOrdinaryLineageCommitProofBundleV1,
    KagemushaVerifiedOrdinaryLineageCommitProofV1,
    KagemushaVerifiedOrdinaryServiceReceivedCashOutputV1, verify_ordinary_lineage_commit_v1,
    verify_service_ordinary_received_cash_output_v1,
};

pub(crate) use commit_admission::{
    KagemushaVerifiedOrdinaryReceivedCashOutputV1,
    readmit_historical_ordinary_received_cash_output_v1, verify_ordinary_received_cash_output_v1,
};

const BUNDLE_MAX: usize = 256 * 1024;
/// Public statement original, selected explicitly before proof admission; no private State fields.
#[derive(Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::OrdinaryLineageStatementOriginalV1"
)]
pub enum KagemushaOrdinaryLineageStatementOriginalV1 {
    /// Immutable zero Bootstrap only.
    Zero(Box<BootstrapStatementV1>),
    /// Exact full public outgoing State statement, with both financial128-bit sequences.
    Outgoing(Box<TransitionProofStatementV1>),
}
/// Exact public pre-W2 outgoing originals. These data do not admit receiver FI or clock authority.
#[derive(Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::OrdinaryLineageOutgoingOriginalsV1"
)]
pub enum KagemushaOrdinaryLineageOutgoingOriginalsV1 {
    /// Full signed receiver request and full actual encrypted credit, separately from the output.
    Send {
        /// Sole original signed request.
        request: Box<KagemushaOrdinaryPaymentRequestV1>,
        /// Actual pre-candidate output.
        output: KagemushaOrdinaryPaymentOutputV1,
        /// Complete canonical ciphertext envelope;384 bytes remains the transport capacity.
        encrypted_credit: Vec<u8>,
        /// Original preparation interval; signed observations are authenticated by the service owner.
        preparation_clock: KagemushaOrdinaryCashClockContextV1,
    },
    /// Full actual release manifest and exact beneficiary, preceding the candidate and W1.
    Redeem {
        /// Sole original acyclic redemption output.
        output: KagemushaOrdinaryRedemptionOutputV1,
        /// Exact initial first-release financial owner beneficiary.
        beneficiary: iroha_data_model::account::AccountId,
        /// Complete canonical manifest from the threshold-admitted release.
        manifest_original: Vec<u8>,
        /// Original preparation interval, without manufacturing clock custody.
        preparation_clock: KagemushaOrdinaryCashClockContextV1,
    },
}
impl KagemushaOrdinaryLineageOutgoingOriginalsV1 {
    fn validate_data(&self) -> Result<()> {
        match self {
            Self::Send {
                request,
                output,
                encrypted_credit,
                preparation_clock,
            } => {
                request.canonical_bytes()?;
                output.canonical_bytes()?;
                output.validate_against_clock(preparation_clock)?;
                if encrypted_credit.len() != KAGEMUSHA_ENCRYPTED_CREDIT_CANONICAL_BYTES_V1 {
                    return reject();
                }
            }
            Self::Redeem {
                output,
                manifest_original,
                preparation_clock,
                ..
            } => {
                output.validate_against_clock(preparation_clock)?;
                if manifest_original.is_empty() || manifest_original.len() > 64 * 1024 {
                    return reject();
                }
            }
        }
        Ok(())
    }
}
/// Sole public proof bundle. Parsing/hashing grants no Native or DATA authority.
#[derive(Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::OrdinaryLineageStateProofBundleV1",
    frame = "iroha.kagemusha.core.v1.ordinary-lineage-state-proof-bundle"
)]
pub struct KagemushaOrdinaryLineageStateProofBundleV1 {
    version: u16,
    normalized: KagemushaNormalizedGuardStatementV1,
    statement: KagemushaOrdinaryLineageStatementOriginalV1,
    state_original: Vec<u8>,
    credential_original: Vec<u8>,
    approval_original: Vec<u8>,
    selected_integrity_original: Option<Vec<u8>>,
    guard_original: Vec<u8>,
    prepared: Option<KagemushaOrdinaryPreparedOutgoingV1>,
    outgoing_originals: Option<KagemushaOrdinaryLineageOutgoingOriginalsV1>,
}
impl KagemushaOrdinaryLineageStateProofBundleV1 {
    /// Assemble neutral public data using the sole encoder; this is not proof/owner admission.
    /// # Errors
    /// Refuses unsupported phase, frame shape or any original exceeding its finite bound.
    pub fn from_public_parts(
        normalized: KagemushaNormalizedGuardStatementV1,
        statement: KagemushaOrdinaryLineageStatementOriginalV1,
        state_original: Vec<u8>,
        credential_original: Vec<u8>,
        approval_original: Vec<u8>,
        selected_integrity_original: Option<Vec<u8>>,
        guard_original: Vec<u8>,
        prepared: Option<KagemushaOrdinaryPreparedOutgoingV1>,
        outgoing_originals: Option<KagemushaOrdinaryLineageOutgoingOriginalsV1>,
    ) -> Result<Self> {
        let value = Self {
            version: 1,
            normalized,
            statement,
            state_original,
            credential_original,
            approval_original,
            selected_integrity_original,
            guard_original,
            prepared,
            outgoing_originals,
        };
        value.validate_data()?;
        Ok(value)
    }
    fn validate_data(&self) -> Result<()> {
        if self.version != 1
            || self.state_original.len() > 32 * 1024
            || self.state_original.is_empty()
            || self.credential_original.is_empty()
            || self.credential_original.len() > KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1
            || self.approval_original.is_empty()
            || self.approval_original.len() > KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1
            || self.guard_original.is_empty()
            || self.guard_original.len()
                > crate::kagemusha_v1_state::KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1
            || self
                .selected_integrity_original
                .as_ref()
                .is_some_and(|v| v.is_empty() || v.len() > 4096)
        {
            return reject();
        }
        KagemushaOrdinaryLineageStateOriginalV1::decode_original(&self.state_original)?;
        self.normalized
            .canonical_digest()
            .map_err(|e| e.to_string())?;
        let zero = matches!(
            self.statement,
            KagemushaOrdinaryLineageStatementOriginalV1::Zero(_)
        );
        if zero != (self.normalized.operation == KagemushaOperationV1::Bootstrap)
            || zero == self.prepared.is_some()
            || zero == self.outgoing_originals.is_some()
        {
            return reject();
        }
        if let Some(v) = &self.prepared {
            v.validate_shape()?;
        }
        if let Some(v) = &self.outgoing_originals {
            v.validate_data()?;
        }
        self.approval()?;
        Ok(())
    }
    /// Sole bounded complete original. It remains untrusted data until actual verifier admission.
    /// # Errors
    /// Refuses invalid phase/shape, oversized originals or encoding failure.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate_data()?;
        let raw = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if raw.len() > BUNDLE_MAX {
            return reject();
        }
        Ok(raw)
    }
    /// Strict data decoder with a fixed256KiB budget, independent of declared vector lengths.
    /// # Errors
    /// Refuses noncanonical, oversized or malformed original data.
    pub fn decode_original(raw: &[u8]) -> Result<Self> {
        if raw.is_empty() || raw.len() > BUNDLE_MAX {
            return reject();
        }
        let value: Self =
            norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
                .map_err(|e| e.to_string())?;
        if value.canonical_bytes()? != raw {
            return reject();
        }
        Ok(value)
    }
    /// Exact complete public State frame: both semantic parity columns and current proof histories.
    /// It contains no private State, secret or restoration witness and creates no proof authority.
    pub fn state_original(&self) -> &[u8] {
        &self.state_original
    }
    /// Same full C original, for independent real governed issuer admission by the service.
    pub fn credential_original(&self) -> &[u8] {
        &self.credential_original
    }
    /// Same full originally selected PI original; no new lease or current grant is manufactured.
    pub fn selected_integrity_original(&self) -> Option<&[u8]> {
        self.selected_integrity_original.as_deref()
    }
    /// Exact raw full W original, without historical or current approval authority.
    pub fn approval_original(&self) -> &[u8] {
        &self.approval_original
    }
    /// Exact public late purpose2 record; decoding this data never lends a Native preparation.
    pub fn prepared(&self) -> Option<&KagemushaOrdinaryPreparedOutgoingV1> {
        self.prepared.as_ref()
    }
    /// Full public acyclic Send/Redeem originals, without receiver possession or financial custody.
    pub fn outgoing_originals(&self) -> Option<&KagemushaOrdinaryLineageOutgoingOriginalsV1> {
        self.outgoing_originals.as_ref()
    }
    /// Same original Native preparation interval projection. The service must independently
    /// authenticate its full signed observations; this data cannot create a current clock.
    pub fn preparation_clock_context(&self) -> Option<&KagemushaOrdinaryCashClockContextV1> {
        match self.outgoing_originals.as_ref()? {
            KagemushaOrdinaryLineageOutgoingOriginalsV1::Send {
                preparation_clock, ..
            }
            | KagemushaOrdinaryLineageOutgoingOriginalsV1::Redeem {
                preparation_clock, ..
            } => Some(preparation_clock),
        }
    }
    /// Decode exact original W for historical clock/signature policy checks. This grants no capture.
    /// # Errors
    /// Refuses malformed or noncanonical original W.
    pub fn approval(&self) -> Result<KagemushaAppOperationApprovalV1> {
        let value: KagemushaAppOperationApprovalV1 = norito::decode_canonical_with_limits(
            &self.approval_original,
            norito::canonical_decode_limits(self.approval_original.len()),
        )
        .map_err(|e| e.to_string())?;
        if norito::encode_canonical(&value).map_err(|e| e.to_string())? != self.approval_original {
            return reject();
        }
        value.challenge.canonical_signing_bytes()?;
        Ok(value)
    }
    fn admit(
        &self,
        verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
        credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
        lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    ) -> Result<KagemushaVerifiedOrdinaryLineageStateProofV1> {
        self.validate_data()?;
        if credential.original() != self.credential_original
            || lease.map(|v| v.original()) != self.selected_integrity_original.as_deref()
        {
            return reject();
        }
        let statement = match &self.statement {
            KagemushaOrdinaryLineageStatementOriginalV1::Zero(s) => {
                KagemushaOrdinaryLineageStatementV1::Zero(s)
            }
            KagemushaOrdinaryLineageStatementOriginalV1::Outgoing(s) => {
                KagemushaOrdinaryLineageStatementV1::Outgoing(s)
            }
        };
        verify_ordinary_lineage_state_proof_v1(
            verifier,
            &self.normalized,
            credential,
            &self.approval()?,
            lease,
            &self.guard_original,
            &self.state_original,
            statement,
            self.prepared.as_ref(),
        )
    }
}
/// Closed mathematical zero anchor. Current FI and actual durable DATA insertion remain separate.
pub struct KagemushaVerifiedOrdinaryLineageAnchorProofV1 {
    anchor: KagemushaOrdinaryLineageAnchorV1,
    state: KagemushaVerifiedOrdinaryLineageStateProofV1,
}
impl KagemushaVerifiedOrdinaryLineageAnchorProofV1 {
    /// Exact Model anchor independently matched to both current proofs and their full histories.
    pub fn anchor(&self) -> &KagemushaOrdinaryLineageAnchorV1 {
        &self.anchor
    }
    /// Actual admitted public mathematical State, carrying no Native/DATA mutation authority.
    pub fn state_proof(&self) -> &KagemushaVerifiedOrdinaryLineageStateProofV1 {
        &self.state
    }
}
/// Admit exactly one zero State/Guard bundle against the immutable Model lineage anchor.
/// # Errors
/// Refuses another scope/head/sequence/original bundle or any failed genuine paired proof.
pub fn verify_ordinary_lineage_anchor_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    anchor: &KagemushaOrdinaryLineageAnchorV1,
    bundle_original: &[u8],
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
) -> Result<KagemushaVerifiedOrdinaryLineageAnchorProofV1> {
    anchor.validate_shape()?;
    let bundle = KagemushaOrdinaryLineageStateProofBundleV1::decode_original(bundle_original)?;
    let state = bundle.admit(verifier, credential, lease)?;
    let n = state.normalized_statement();
    require_lineage(&anchor.lineage, &state, credential)?;
    if n.operation != KagemushaOperationV1::Bootstrap
        || n.amount != 0
        || n.predecessor_state_commitment != [0; 32]
        || n.successor_logical_sequence != 0
        || anchor.initial_head.state_commitment != n.successor_state_commitment
        || anchor.initial_head.state_original_sha256 != state.state_original_sha256()
        || anchor.proof_bundle_original_sha256 != <DigestV1>::from(Sha256::digest(bundle_original))
    {
        return reject();
    }
    Ok(KagemushaVerifiedOrdinaryLineageAnchorProofV1 {
        anchor: anchor.clone(),
        state,
    })
}
/// Closed genuine purpose2 reserve proof selection. Only the DATA owner may subsequently issue CAS.
pub struct KagemushaVerifiedOrdinaryLineageReservationProofV1 {
    reservation: KagemushaOrdinaryLineageReservationV1,
    state: KagemushaVerifiedOrdinaryLineageStateProofV1,
}
impl KagemushaVerifiedOrdinaryLineageReservationProofV1 {
    /// Exact complete immutable reservation data, admitted mathematically without a CAS grant.
    pub fn reservation(&self) -> &KagemushaOrdinaryLineageReservationV1 {
        &self.reservation
    }
    /// Actual candidate State/Guard admission, without any Native financial secret or capability.
    pub fn state_proof(&self) -> &KagemushaVerifiedOrdinaryLineageStateProofV1 {
        &self.state
    }
}
/// Verify genuine purpose2 proof plus its exact public predecessor and real neutral reservation.
/// The DATA owner separately compares this result to its exclusively held current head before CAS.
/// # Errors
/// Refuses any predecessor/successor/amount/operation/original/lineage mismatch or invalid proof.
pub fn verify_ordinary_lineage_reservation_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    reservation: &KagemushaOrdinaryLineageReservationV1,
    bundle_original: &[u8],
    predecessor_state_original: &[u8],
    neutral_reservation_original: &[u8],
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
) -> Result<KagemushaVerifiedOrdinaryLineageReservationProofV1> {
    reservation.validate_shape()?;
    let bundle = KagemushaOrdinaryLineageStateProofBundleV1::decode_original(bundle_original)?;
    let state = bundle.admit(verifier, credential, lease)?;
    let n = state.normalized_statement();
    let select = &reservation.selection;
    require_lineage(&select.lineage, &state, credential)?;
    let prepared = bundle.prepared.as_ref().ok_or_else(rejection)?;
    let w = bundle.approval()?;
    if KagemushaOperationKindV1::from(n.operation) != select.operation
        || n.amount != select.amount
        || n.asset_scale != select.scale
        || select.operation_id != w.challenge.operation_id
        || select.predecessor.state_commitment != n.predecessor_state_commitment
        || select.predecessor.logical_sequence != n.predecessor_logical_sequence
        || reservation.successor.state_commitment != n.successor_state_commitment
        || reservation.successor.logical_sequence != n.successor_logical_sequence
        || reservation.successor.state_original_sha256 != state.state_original_sha256()
        || reservation.purpose2_approval_original_sha256 != state.approval_original_sha256()
        || reservation.candidate_original_sha256 != state.projection_original_sha256()
        || reservation.proof_bundle_original_sha256
            != <DigestV1>::from(Sha256::digest(bundle_original))
        || select.neutral_reservation_digest != prepared.reservation_digest
        || select.receiver_request_original_sha256 != prepared.request_digest
    {
        return reject();
    }
    require_outgoing_originals(&bundle, reservation, verifier)?;
    let before =
        KagemushaOrdinaryLineageStateOriginalV1::decode_original(predecessor_state_original)?;
    if select.predecessor.state_original_sha256
        != <DigestV1>::from(Sha256::digest(predecessor_state_original))
        || before.projection.digest(s::SUCCESSOR_OUTER_LO)? != n.predecessor_state_commitment
    {
        return reject();
    }
    // The predecessor's complete proof/history is reverified under the same genuinely installed
    // key family. Its actual logical coordinate is additionally compared to the held DATA head,
    // never inferred from an offered column (that coordinate is private inside the State head).
    let (mut eq, mut ep) = before.projection.fields()?;
    append_history(&mut eq, &before.proof.eq_history)?;
    append_history(&mut ep, &before.proof.ep_history)?;
    let m = verifier.state_checkpoint_material();
    require_predecessor_material(&before, &m, n)?;
    verify_state_histories(&m, &before.proof, &eq, &ep)?;
    if neutral_reservation_original.is_empty() || neutral_reservation_original.len() > 4096 {
        return reject();
    }
    let neutral: KagemushaOutboxReservationV1 = norito::decode_canonical_with_limits(
        neutral_reservation_original,
        norito::canonical_decode_limits(neutral_reservation_original.len()),
    )
    .map_err(|e| e.to_string())?;
    if norito::encode_canonical(&neutral).map_err(|e| e.to_string())?
        != neutral_reservation_original
        || neutral.canonical_commitment().map_err(|e| e.to_string())?
            != select.neutral_reservation_digest
        || neutral.operation_kind != select.operation
        || select.neutral_reservation_original_sha256
            != <DigestV1>::from(Sha256::digest(neutral_reservation_original))
    {
        return reject();
    }
    Ok(KagemushaVerifiedOrdinaryLineageReservationProofV1 {
        reservation: reservation.clone(),
        state,
    })
}
fn require_outgoing_originals(
    bundle: &KagemushaOrdinaryLineageStateProofBundleV1,
    reservation: &KagemushaOrdinaryLineageReservationV1,
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
) -> Result<()> {
    let n = &bundle.normalized;
    let prepared = bundle.prepared.as_ref().ok_or_else(rejection)?;
    let selected = &reservation.selection;
    let w = bundle.approval()?;
    let nullifier = kagemusha_ordinary_transition_nullifier_v1(
        n.predecessor_state_commitment,
        w.challenge.subject.secure_index_before,
        selected.lineage.financial_epoch_id,
        n.network_id,
        n.lane_id,
        n.liability_pool_id,
    )?;
    match bundle.outgoing_originals.as_ref().ok_or_else(rejection)? {
        KagemushaOrdinaryLineageOutgoingOriginalsV1::Send {
            request,
            output,
            encrypted_credit,
            preparation_clock,
        } if n.operation == KagemushaOperationV1::SendSplit => {
            output.validate_against_clock(preparation_clock)?;
            request.body.validate_shape()?;
            let request_digest = request.canonical_original_digest()?;
            let output_original = output.canonical_bytes()?;
            let encrypted_digest = kagemusha_ciphertext_digest_v1(encrypted_credit);
            let rb = &request.body;
            if request_digest != prepared.request_digest
                || selected.receiver_request_original_sha256 != request_digest
                || selected.output_body_original_sha256
                    != <DigestV1>::from(Sha256::digest(output_original))
                || output.request_digest != request_digest
                || output.amount != n.amount
                || rb.amount != n.amount
                || output.sender_before_commitment != n.predecessor_state_commitment
                || output.sender_after_commitment != n.successor_state_commitment
                || output.transition_nullifier != nullifier
                || output.credit_id != n.peer_credit_id
                || rb.recipient_encryption_key != n.recipient_encryption_key_binding
                || rb.release_id != n.release_id
                || rb.network_id != n.network_id
                || rb.normalized_asset_id != n.asset_id
                || rb.asset_incarnation != *n.asset_incarnation.as_bytes()
                || rb.scale != n.asset_scale
                || rb.reserve_pool_id != n.liability_pool_id
                || output.encrypted_credit_digest != encrypted_digest
                || preparation_clock.lower_at_ms < rb.issued_at_ms
                || preparation_clock.upper_at_ms >= rb.expires_at_ms
                || kagemusha_ordinary_payment_body_digest_v1(
                    output.binding_digest()?,
                    encrypted_digest,
                )? != prepared.projection_semantic_digest
            {
                return reject();
            }
        }
        KagemushaOrdinaryLineageOutgoingOriginalsV1::Redeem {
            output,
            beneficiary,
            manifest_original,
            preparation_clock,
        } if n.operation == KagemushaOperationV1::RedeemSplit => {
            let release = verifier.monetary_release()?;
            output.validate_against_originals(beneficiary, preparation_clock, &release)?;
            let output_original = norito::encode_canonical(output).map_err(|e| e.to_string())?;
            if selected.output_body_original_sha256
                != <DigestV1>::from(Sha256::digest(output_original))
                || selected.receiver_request_original_sha256 != [0; 32]
                || beneficiary != &selected.lineage.owner.account_id
                || manifest_original.as_slice()
                    != release
                        .canonical_manifest_original()
                        .map_err(|e| e.to_string())?
                || output.amount != n.amount
                || output.release_id != n.release_id
                || output.network_id != n.network_id
                || output.normalized_asset_id != n.asset_id
                || output.asset_incarnation != *n.asset_incarnation.as_bytes()
                || output.scale != n.asset_scale
                || output.reserve_pool_id != n.liability_pool_id
                || output.sender_before_commitment != n.predecessor_state_commitment
                || output.sender_after_commitment != n.successor_state_commitment
                || output.transition_nullifier != nullifier
                || output.lifecycle_digest != n.lifecycle_binding_digest
                || output.artifact_manifest_digest != prepared.artifact_manifest_digest
                || output.binding_digest()? != prepared.projection_semantic_digest
            {
                return reject();
            }
        }
        _ => return reject(),
    }
    Ok(())
}
pub(super) fn require_lineage(
    lineage: &KagemushaOrdinaryFinancialLineageV1,
    state: &KagemushaVerifiedOrdinaryLineageStateProofV1,
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
) -> Result<()> {
    lineage.validate_shape()?;
    let n = state.normalized_statement();
    let owner = &lineage.owner;
    if lineage.financial_epoch_id != state.financial_epoch_id()
        || lineage.financial_authority_commitment != state.financial_authority_commitment()
        || owner.lane_id != n.lane_id
        || owner.runtime.network_id.as_bytes() != &n.network_id
        || owner.runtime.scale != n.asset_scale
        || owner.runtime.asset_incarnation != n.asset_incarnation
        || kagemusha_asset_identity_digest_v1(&owner.runtime.asset).map_err(|e| e.to_string())?
            != n.asset_id
        || kagemusha_ordinary_app_account_binding_v1(&owner.account_id)
            != credential.subject().account_binding
    {
        return reject();
    }
    Ok(())
}
fn require_predecessor_material(
    before: &KagemushaOrdinaryLineageStateOriginalV1,
    m: &super::super::state_checkpoint::KagemushaStateCheckpointVerifierMaterialV1<'_>,
    n: &KagemushaNormalizedGuardStatementV1,
) -> Result<()> {
    let p = &before.projection;
    let proof = &before.proof;
    let a = m.artifacts;
    let (req, rep) = (
        m.binding.outer_eq_protocol_digest,
        m.binding.outer_ep_protocol_digest,
    );
    if proof.eq_protocol_digest != m.binding.outer_eq_protocol_digest
        || proof.ep_protocol_digest != m.binding.outer_ep_protocol_digest
        || proof.guard_eq_credential_audit != req
        || proof.guard_ep_credential_audit != rep
    {
        return reject();
    }
    for (at, expected) in [
        (s::RELEASE_LO, m.binding.release_id),
        (s::SUCCESSOR_SUITE_LO, m.binding.suite_id),
        (s::SUCCESSOR_VK_LO, m.binding.vk_set_digest),
        (s::EQ_PROTOCOL_LO, proof.eq_protocol_digest),
        (s::EP_PROTOCOL_LO, proof.ep_protocol_digest),
        (s::TRANSPORT_LO, proof.semantic_digest),
        (s::EQ_DEFERRED_AUDIT_LO, proof.eq_deferred_audit),
        (s::EP_DEFERRED_AUDIT_LO, proof.ep_deferred_audit),
        (s::GUARD_EQ_CREDENTIAL_AUDIT_LO, req),
        (s::GUARD_EP_CREDENTIAL_AUDIT_LO, rep),
        (s::MINT_EQ_PROTOCOL_LO, a.mint_finality_eq_protocol_digest),
        (s::MINT_EP_PROTOCOL_LO, a.mint_finality_ep_protocol_digest),
        (
            s::MINT_AUTHORIZATION_EQ_PROTOCOL_LO,
            a.mint_authorization_eq_protocol_digest,
        ),
        (
            s::MINT_AUTHORIZATION_EP_PROTOCOL_LO,
            a.mint_authorization_ep_protocol_digest,
        ),
        (
            s::COMMIT_WRAPPER_EQ_PROTOCOL_LO,
            a.commit_wrapper_eq_protocol_digest,
        ),
        (
            s::COMMIT_WRAPPER_EP_PROTOCOL_LO,
            a.commit_wrapper_ep_protocol_digest,
        ),
        (s::NETWORK_LO, n.network_id),
        (s::ASSET_LO, n.asset_id),
        (s::ASSET_INCARNATION_LO, *n.asset_incarnation.as_bytes()),
        (s::LIABILITY_POOL_LO, n.liability_pool_id),
        (s::HARDWARE_PROFILE_LO, n.hardware_profile_id),
    ] {
        p.require_digest(at, expected)?;
    }
    if p.integer(s::POLICY_EPOCH)? != u128::from(n.policy_epoch)
        || p.integer(s::ASSET_SCALE)? != u128::from(n.asset_scale)
        || p.integer(s::PROTOCOL_VERSION)? != 1
    {
        return reject();
    }
    Ok(())
}

/// Immutable data from the actual installed ordinary monetary State verifier, not a new grant.
pub(crate) fn ordinary_incoming_artifacts_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
) -> Result<(super::super::KagemushaRecursionArtifactsV1, DigestV1)> {
    let release = verifier.monetary_release()?;
    let material = verifier.state_checkpoint_material();
    if release.release_id() != material.artifacts.release_id {
        return reject();
    }
    Ok((material.artifacts, release.provider_policy_root()))
}
/// Verify both genuine current State proofs and full histories of the exact held public predecessor.
/// The actual Native owner separately binds its private State/full head and financial sequence.
pub(crate) fn require_ordinary_incoming_predecessor_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    predecessor_original: &[u8],
    normalized: &KagemushaNormalizedGuardStatementV1,
) -> Result<()> {
    let before = KagemushaOrdinaryLineageStateOriginalV1::decode_original(predecessor_original)?;
    if before.projection.digest(s::SUCCESSOR_OUTER_LO)? != normalized.predecessor_state_commitment
        || !matches!(
            normalized.operation,
            KagemushaOperationV1::MintFold | KagemushaOperationV1::ReceiveFold
        )
    {
        return reject();
    }
    let material = verifier.state_checkpoint_material();
    require_predecessor_material(&before, &material, normalized)?;
    let (mut eq, mut ep) = before.projection.fields()?;
    append_history(&mut eq, &before.proof.eq_history)?;
    append_history(&mut ep, &before.proof.ep_history)?;
    verify_state_histories(&material, &before.proof, &eq, &ep)
}

#[cfg(test)]
mod canonical_ciphertext_tests {
    use super::*;
    use iroha_data_model::kagemusha::{
        KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1, KagemushaAppOperationApprovalEvidenceV1,
        KagemushaOrdinaryPaymentRequestBodyV1, kagemusha_ordinary_credit_id_v1,
    };
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

    #[test]
    fn outgoing_send_data_accepts_actual_canonical_cipher_and_refuses_transport_padding() {
        // Genuine codec/signature data only: this constructs no admitted receiver or Native source.
        let f =
            iroha_data_model::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_v1();
        let context = &f.request.authorization.statement.context;
        let clock = context.clock_context;
        let body = KagemushaOrdinaryPaymentRequestBodyV1 {
            version: 1,
            release_id: context.release_id,
            network_id: [1; 32],
            normalized_asset_id: [2; 32],
            asset_incarnation: [3; 32],
            scale: 4,
            reserve_pool_id: [4; 32],
            recipient_account_binding: [5; 32],
            amount: context.amount,
            recipient_encryption_key: context.recipient_one_time_key,
            recipient_credential_digest: context.recipient_app_credential_digest,
            recipient_lane_id: [6; 32],
            request_id: [7; 32],
            clock_context: clock,
            issued_at_ms: clock.lower_at_ms,
            expires_at_ms: clock.upper_at_ms + 100,
        };
        let signer = SigningKey::from_bytes((&[61; 32]).into()).unwrap();
        let signature: Signature = signer.sign(&body.canonical_signing_bytes().unwrap());
        let request = KagemushaOrdinaryPaymentRequestV1 {
            body,
            evidence: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                signature_der: signature.to_der().as_bytes().to_vec(),
            },
        };
        let request_digest = request.canonical_original_digest().unwrap();
        let nullifier = [8; 32];
        let output = KagemushaOrdinaryPaymentOutputV1 {
            version: 1,
            request_digest,
            amount: context.amount,
            sender_before_commitment: [9; 32],
            sender_after_commitment: [10; 32],
            transition_nullifier: nullifier,
            credit_id: kagemusha_ordinary_credit_id_v1(nullifier, request_digest),
            ciphertext_commitment: [11; 32],
            encrypted_credit_digest: kagemusha_ciphertext_digest_v1(&f.request.encrypted_credit),
            clock_context_digest: clock.binding_digest().unwrap(),
            prepared_at_ms: clock.upper_at_ms,
        };
        let original = f.request.encrypted_credit;
        let data = |encrypted_credit: Vec<u8>| KagemushaOrdinaryLineageOutgoingOriginalsV1::Send {
            request: Box::new(request.clone()),
            output,
            encrypted_credit,
            preparation_clock: clock,
        };
        data(original.clone()).validate_data().unwrap();
        assert_eq!(
            original.len(),
            KAGEMUSHA_ENCRYPTED_CREDIT_CANONICAL_BYTES_V1
        );
        assert_eq!(KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1, 384);
        for end in 0..original.len() {
            assert!(data(original[..end].to_vec()).validate_data().is_err());
        }
        let mut suffix = original.clone();
        suffix.push(0);
        assert!(data(suffix).validate_data().is_err());
        let mut padded = original;
        padded.resize(KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1, 0);
        assert!(data(padded).validate_data().is_err());
    }
}
