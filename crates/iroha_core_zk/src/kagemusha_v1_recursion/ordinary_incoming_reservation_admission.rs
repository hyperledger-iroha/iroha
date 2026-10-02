//! Independent genuine MintFold reservation admission under full original custody.
//! Portable data never constructs Native source, time, FI, replay, or DATA authority.
use super::*;
#[path = "ordinary_incoming_commit_admission.rs"]
mod commit_admission;
use crate::kagemusha_v1_state::{
    KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1,
    KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1,
    KagemushaAuthenticatedOrdinaryFinalizedMintSourceV1,
    KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1,
    KagemushaAuthenticatedOrdinaryServiceFinalizedMintSourceV1, KagemushaTransitionKindV1,
    KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1, TransitionProofStatementV1,
};
pub(crate) use commit_admission::{
    GeneratedOrdinaryIncomingCommitOriginalsV1, assemble_ordinary_incoming_commit_v1,
    readmit_ordinary_incoming_commit_v1,
};
pub use commit_admission::{
    KAGEMUSHA_ORDINARY_INCOMING_COMMIT_BUNDLE_MAX_BYTES_V1,
    KagemushaOrdinaryIncomingCommitProofBundleV1, KagemushaVerifiedOrdinaryIncomingCommitProofV1,
    ordinary_incoming_commit_carrier_max_bytes_v1, verify_ordinary_incoming_commit_v1,
};
use iroha_data_model::kagemusha::*;
use sha2::{Digest as _, Sha256};

/// Full structural upper bound: three genuine signed clock originals, complete existing
/// finalized debit/finality original, both public State checkpoints, and all actual source/FI/
/// app/Guard originals. This retains existing protocol maxima; it never truncates an original.
/// Actual Guard/State proof widths must additionally equal the independently admitted profiles.
pub const KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1: usize =
    KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1
        + 3 * KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1
        + 2 * 32 * 1024
        + KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1
        + KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1
        + KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1
        + 2 * 4096
        + KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1
        + 2 * KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1
        + KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1
        + 64 * 1024;

/// Sole self-contained public MintFold reservation proof carrier. Decoding creates only data;
/// the independent verifier requires actual finalized source and signed-clock capabilities.
/// Receive has a distinct source/proof relation and is refused until that relation is complete.
#[derive(Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::KagemushaOrdinaryIncomingReservationProofBundleV1")]
pub struct KagemushaOrdinaryIncomingReservationProofBundleV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Exact finalized source/head selection; no pending DATA authority is implied.
    pub reservation: KagemushaOrdinaryIncomingReservationV1,
    /// Fresh incoming purpose2 normalized statement; not the pre-debit Mint statement.
    pub normalized: KagemushaNormalizedGuardStatementV1,
    /// Full canonical actual financial transition statement, with both u128 State sequences.
    pub statement: TransitionProofStatementV1,
    /// Full 306-byte fresh incoming preparation selected before W2.
    pub preparation: KagemushaOrdinaryIncomingPreparationV1,
    /// Full previous PUBLIC State original, including both current proofs and all histories.
    pub predecessor_state_original: Vec<u8>,
    /// Full selected successor PUBLIC State original, including both current proofs/histories.
    pub state_original: Vec<u8>,
    /// Full same enrolled app credential original, never a hardware-key handle.
    pub credential_original: Vec<u8>,
    /// Full separately captured fresh incoming purpose2 platform approval.
    pub approval_original: Vec<u8>,
    /// Full independently verified Guard pair and both complete histories.
    pub guard_original: Vec<u8>,
    /// Exact selected incoming PI lease; Android baseline selection is represented by None.
    pub selected_integrity_original: Option<Vec<u8>>,
    /// Same immutable FI enrollment certificate authenticated by the real source owner.
    pub financial_certificate_original: Vec<u8>,
    /// Exact fresh incoming FI decision, distinct from original Mint preparation decision.
    pub financial_control_original: Vec<u8>,
    /// Actual fresh incoming preparation interval; full signed observations are separate.
    pub preparation_clock_context: KagemushaOrdinaryCashClockContextV1,
    /// Full signed clock observations retained for that fresh preparation.
    pub preparation_clock_signed_original: Vec<u8>,
    /// Actual post-platform fsynced capture interval, retained by Native CaptureAck.
    pub approval_admission_clock_context: KagemushaOrdinaryCashClockContextV1,
    /// Full signed observations behind that actual capture, never offered lower/upper scalars.
    pub approval_admission_clock_signed_original: Vec<u8>,
    /// Full request, signed pre-debit decision and actual debit receipt/finality/membership.
    pub finalized_mint_original: Vec<u8>,
    /// Full neutral MintCredit canonical original with genuine MintAuthority proofs/histories.
    pub mint_credit_original: Vec<u8>,
    /// Exact old Mint preparation FI-control original, authenticated by the genuine source cap.
    pub mint_preparation_financial_control_original: Vec<u8>,
    /// Exact selected old Mint PI original, independently from the incoming selection.
    pub mint_selected_integrity_original: Option<Vec<u8>>,
    /// Full original signed observations from the original pre-debit Mint113 authorization.
    pub mint_preparation_clock_signed_original: Vec<u8>,
}
impl KagemushaOrdinaryIncomingReservationProofBundleV1 {
    fn validate_data(&self) -> Result<()> {
        if self.version != 1
            || self.statement.kind != KagemushaTransitionKindV1::MintFold
            || self.normalized.operation != KagemushaOperationV1::MintFold
        {
            return reject();
        }
        self.reservation.validate_shape()?;
        self.preparation.validate_shape()?;
        self.preparation_clock_context.validate_shape()?;
        self.approval_admission_clock_context.validate_shape()?;
        self.normalized
            .canonical_digest()
            .map_err(|e| e.to_string())?;
        self.statement.digest().map_err(|e| e.to_string())?;
        for (raw, max) in [
            (&self.predecessor_state_original, 32 * 1024),
            (&self.state_original, 32 * 1024),
            (
                &self.credential_original,
                KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1,
            ),
            (
                &self.approval_original,
                KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1,
            ),
            (&self.guard_original, KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1),
            (
                &self.financial_certificate_original,
                KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
            ),
            (
                &self.financial_control_original,
                KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
            ),
            (
                &self.mint_preparation_financial_control_original,
                KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
            ),
            (
                &self.preparation_clock_signed_original,
                KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1,
            ),
            (
                &self.approval_admission_clock_signed_original,
                KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1,
            ),
            (
                &self.mint_preparation_clock_signed_original,
                KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1,
            ),
            (
                &self.finalized_mint_original,
                KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1,
            ),
            (
                &self.mint_credit_original,
                KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1,
            ),
        ] {
            if raw.is_empty() || raw.len() > max {
                return reject();
            }
        }
        for lease in [
            &self.selected_integrity_original,
            &self.mint_selected_integrity_original,
        ] {
            if lease
                .as_ref()
                .is_some_and(|b| b.is_empty() || b.len() > 4096)
            {
                return reject();
            }
        }
        self.approval()?;
        KagemushaOrdinaryLineageStateOriginalV1::decode_original(&self.state_original)?;
        KagemushaOrdinaryLineageStateOriginalV1::decode_original(&self.predecessor_state_original)?;
        Ok(())
    }
    /// Encode the full bounded sole carrier; this does not authenticate any original.
    /// # Errors
    /// Refuses unsupported source, shape, oversized originals or serialization failure.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate_data()?;
        let raw = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if raw.len() > KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1 {
            return reject();
        }
        Ok(raw)
    }
    /// Strict bounded data decoder. No constructor upgrades its result into a source or proof cap.
    /// # Errors
    /// Refuses malformed, oversized, truncated, trailing or noncanonical originals.
    pub fn decode_original(raw: &[u8]) -> Result<Self> {
        if raw.is_empty() || raw.len() > KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1
        {
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
    /// Decode the same exact original W2 for independent historical signature/context checking.
    /// # Errors
    /// Refuses a noncanonical or malformed platform original.
    pub fn approval(&self) -> Result<KagemushaAppOperationApprovalV1> {
        data(
            &self.approval_original,
            KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1,
        )
    }
}

/// Closed independent full-source/State/Guard admission under actual installed release protocols.
/// No decoder, public constructor, Clone or financial secret capability exists. Current service
/// FI/effect clock, exclusive DATA pending head and later purpose1 Commit remain mandatory.
pub struct KagemushaVerifiedOrdinaryIncomingReservationProofV1 {
    reservation: KagemushaOrdinaryIncomingReservationV1,
    state: KagemushaVerifiedOrdinaryLineageStateProofV1,
    bundle_original: Vec<u8>,
    bundle_original_sha256: DigestV1,
}
impl KagemushaVerifiedOrdinaryIncomingReservationProofV1 {
    /// Same full independently admitted finalized source and original predecessor selection.
    pub fn reservation(&self) -> &KagemushaOrdinaryIncomingReservationV1 {
        &self.reservation
    }
    /// Actual installed release under which both State and Guard pairs were admitted.
    pub fn release_id(&self) -> DigestV1 {
        self.state.normalized_statement().release_id
    }
    /// Both-parity State/current-history admission for this same incoming edge.
    pub fn state_proof(&self) -> &KagemushaVerifiedOrdinaryLineageStateProofV1 {
        &self.state
    }
    /// Exact complete sole self-contained bundle admitted here; contains no private State/opening.
    pub fn original(&self) -> &[u8] {
        &self.bundle_original
    }
    /// Raw SHA256 of that exact admitted public bundle.
    pub fn original_sha256(&self) -> DigestV1 {
        self.bundle_original_sha256
    }
}

enum MintSource<'loan, 'owner> {
    Native(&'loan KagemushaAuthenticatedOrdinaryFinalizedMintSourceV1<'owner>),
    Service(&'loan KagemushaAuthenticatedOrdinaryServiceFinalizedMintSourceV1<'owner>),
}
macro_rules! source_getter {
    ($self:expr, $method:ident) => {
        match $self {
            MintSource::Native(v) => v.$method().map_err(|e| e.to_string()),
            MintSource::Service(v) => v.$method().map_err(|e| e.to_string()),
        }
    };
}

/// Independently admit the complete incoming reservation using genuine service source custody
/// and two independently admitted complete W2 clock originals. The source already owns the
/// distinct Mint113 admission and original Mint clock/finality under actual installed purpose.
/// Signed context consistency does not certify the sender's measured elapsed time. A fresh
/// service effect clock and DATA CAS remain separate from this mathematical result.
/// # Errors
/// Refuses source/request/FI/clock/purpose/original substitution, malformed full public parent,
/// mismatched installed protocols, invalid Guard/current State proof or any bad whole history.
pub fn verify_ordinary_incoming_reservation_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    bundle: &KagemushaOrdinaryIncomingReservationProofBundleV1,
    source: &KagemushaAuthenticatedOrdinaryServiceFinalizedMintSourceV1<'_>,
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    incoming_lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    incoming_clocks: [&KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1; 2],
) -> Result<KagemushaVerifiedOrdinaryIncomingReservationProofV1> {
    admit(
        verifier,
        bundle,
        MintSource::Service(source),
        credential,
        incoming_lease,
        incoming_clocks,
    )
}
fn admit(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    bundle: &KagemushaOrdinaryIncomingReservationProofBundleV1,
    source: MintSource<'_, '_>,
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    incoming_lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    incoming_clocks: [&KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1; 2],
) -> Result<KagemushaVerifiedOrdinaryIncomingReservationProofV1> {
    bundle.validate_data()?;
    source_getter!(&source, recheck_retained_custody)?;
    let authorization = source_getter!(&source, authorization)?;
    let request =
        KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(authorization.request_original())?;
    let reservation = &bundle.reservation;
    reservation.selection.validate_against_topup(&request)?;
    let credit: KagemushaMintCreditV1 = data(
        &bundle.mint_credit_original,
        KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1,
    )?;
    if bundle.finalized_mint_original != source_getter!(&source, finalized_original)?
        || bundle.financial_certificate_original
            != source_getter!(&source, financial_enrollment_original)?
        || bundle.mint_preparation_financial_control_original
            != source_getter!(&source, preparation_financial_control_original)?
        || bundle.mint_preparation_clock_signed_original
            != authorization.preparation_clock_original()
        || bundle.mint_selected_integrity_original.as_deref()
            != authorization.selected_integrity_original()
        || bundle.credential_original != authorization.credential_original()
        || bundle.credential_original != credential.original()
        || bundle.selected_integrity_original.as_deref() != incoming_lease.map(|l| l.original())
        || credit.statement != *source_getter!(&source, credit_statement)?
        || credit.encrypted_credit != request.encrypted_credit
        || request.authorization != *authorization.authorization()
    {
        return reject();
    }
    require_source_selectors(
        reservation,
        Sha256::digest(&bundle.finalized_mint_original).into(),
        Sha256::digest(&bundle.mint_credit_original).into(),
        source_getter!(&source, source_semantic_digest)?,
        Sha256::digest(&bundle.predecessor_state_original).into(),
    )?;
    let material = verifier.state_checkpoint_material();
    let mint = super::super::verify_kagemusha_mint_finality_helper_v1(
        verifier,
        material.artifacts,
        &credit,
    )
    .map_err(|e| e.to_string())?;
    let n = &bundle.normalized;
    let statement = &bundle.statement;
    if statement.mint_finality_semantic_digest != mint.semantic_digest()
        || statement.mint_finality_proof_binding_digest != mint.proof_binding_digest()
        || statement.mint_finality_semantic_digest != reservation.source_semantic_digest
        || statement.lifecycle_binding_digest
            != credit
                .statement
                .lifecycle
                .canonical_digest()
                .map_err(|e| e.to_string())?
        || statement.receive_credit_binding_digest != [0; 32]
        || statement.prepared_transition_binding_digest != [0; 32]
        || statement.peer_credit_id != [0; 32]
        || statement.recipient_encryption_key_binding != [0; 32]
        || statement.amount != reservation.selection.amount
        || statement.predecessor_sequence != reservation.selection.predecessor.logical_sequence
        || statement.predecessor_commitment != reservation.selection.predecessor.state_commitment
        || statement.effect_digest != reservation.digest()?
    {
        return reject();
    }
    require_incoming_preparation(bundle)?;
    for (clock, raw, context) in [
        (
            incoming_clocks[0],
            &bundle.preparation_clock_signed_original,
            &bundle.preparation_clock_context,
        ),
        (
            incoming_clocks[1],
            &bundle.approval_admission_clock_signed_original,
            &bundle.approval_admission_clock_context,
        ),
    ] {
        if clock.original() != raw {
            return reject();
        }
        clock
            .recheck_cash_context(context)
            .map_err(|e| e.to_string())?;
    }
    require_financial_control(
        bundle,
        credential,
        source_getter!(&source, issuer_policy)?,
        verifier,
    )?;
    let approval = bundle.approval()?;
    let admission = &bundle.approval_admission_clock_context;
    let preparation = &bundle.preparation_clock_context;
    let c = credential.subject();
    if admission.lower_at_ms < preparation.lower_at_ms
        || admission.upper_at_ms < preparation.upper_at_ms
        || preparation.lower_at_ms < approval.challenge.issued_at_ms
        || admission.upper_at_ms >= approval.challenge.expires_at_ms
        || approval.challenge.issued_at_ms < c.issued_at_ms
        || approval.challenge.expires_at_ms > c.expires_at_ms
    {
        return reject();
    }
    for now in [
        preparation.lower_at_ms,
        preparation.upper_at_ms,
        admission.lower_at_ms,
        admission.upper_at_ms,
    ] {
        match incoming_lease {
            Some(lease) => credential.recheck_with_integrity_lease(lease, now)?,
            None => credential.recheck_at_trusted_time(now)?,
        }
    }
    let minimum = match c.platform_class {
        KagemushaHardwarePlatformClassV1::AndroidKeyMint => None,
        KagemushaHardwarePlatformClassV1::AppleAppAttest => Some(c.app_attest_counter_floor),
        _ => return reject(),
    };
    // Reconstruct scope, nonce, statement and original window from the independently checked
    // State/source/preparation/FI/clock operands. This never copies the received challenge.
    let expected =
        expected_incoming_preparation_challenge(bundle, credential, incoming_lease, verifier)?;
    for now in [
        preparation.lower_at_ms,
        preparation.upper_at_ms,
        admission.lower_at_ms,
        admission.upper_at_ms,
    ] {
        match incoming_lease {
            Some(lease) => approval
                .authenticate_with_integrity_lease(&expected, credential, lease, minimum, now)?,
            None => approval.authenticate(&expected, credential, minimum, now)?,
        };
    }
    bundle_admission::require_ordinary_incoming_predecessor_v1(
        verifier,
        &bundle.predecessor_state_original,
        n,
    )?;
    let state = verify_incoming_state(
        verifier,
        bundle,
        credential,
        &approval,
        incoming_lease,
        IncomingStateSourceMetadata {
            semantic: mint.semantic_digest(),
            proof_binding: mint.proof_binding_digest(),
        },
    )?;
    bundle_admission::require_lineage(&reservation.selection.lineage, &state, credential)?;
    source_getter!(&source, recheck_retained_custody)?;
    let original = bundle.canonical_bytes()?;
    Ok(KagemushaVerifiedOrdinaryIncomingReservationProofV1 {
        reservation: reservation.clone(),
        state,
        bundle_original_sha256: Sha256::digest(&original).into(),
        bundle_original: original,
    })
}
fn expected_incoming_preparation_challenge(
    b: &KagemushaOrdinaryIncomingReservationProofBundleV1,
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
) -> Result<KagemushaAppOperationApprovalChallengeV1> {
    let c = credential.subject();
    let p = &b.preparation;
    let n = &b.normalized;
    let certificate: KagemushaOrdinaryRetailEnrollmentCertificateV1 = data(
        &b.financial_certificate_original,
        KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
    )?;
    let control: KagemushaSignedOrdinaryCurrentControlV1 = data(
        &b.financial_control_original,
        KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
    )?;
    let integrity_expiry = lease.map_or_else(
        || {
            c.play_integrity
                .map_or(c.expires_at_ms, |pi| pi.refresh_before_ms)
        },
        |lease| {
            lease
                .subject()
                .expires_at_ms
                .min(lease.subject().binding.refresh_before_ms)
        },
    );
    let issued = b.preparation_clock_context.lower_at_ms;
    let expires = incoming_preparation_expiry(
        issued,
        c.expires_at_ms,
        certificate.subject.expires_at_ms,
        integrity_expiry,
        control.subject.expires_at_ms,
    )?;
    let release = verifier.monetary_release()?;
    let subject = KagemushaHardwareTransitionSelectionV1 {
        version: 1,
        release_id: release.release_id(),
        provider_policy_root: release.provider_policy_root(),
        app_policy_digest: credential.static_binding_digest(),
        credential_id: credential.digest(),
        network_id: b.statement.lane.network_id,
        lane_commitment: b.statement.lane.device_lane_id,
        hardware_profile_id: c.hardware_profile_id,
        policy_epoch: c.policy_epoch,
        hardware_epoch_id: kagemusha_ordinary_financial_epoch_id_v1(c)?,
        hardware_epoch_generation: c.hardware_epoch,
        operation_kind: KagemushaOperationKindV1::from(n.operation),
        transition_statement_digest: b.statement.digest().map_err(|e| e.to_string())?,
        candidate_envelope_digest: [0; 32],
        terminal_body_commitment: [0; 32],
        secure_index_before: p.financial_index_before,
        secure_index_after: p.financial_index_after,
    };
    let expected = KagemushaAppOperationApprovalChallengeV1 {
        version: 1,
        purpose: KagemushaAppOperationApprovalPurposeV1::PrepareTransition,
        operation_id: p.operation_id,
        nonce: p.nonce,
        account_binding: c.account_binding,
        authority_policy_digest: c.app_authority_policy_digest,
        attested_key_id: c.attested_key_id,
        enrollment_digest: credential.digest(),
        subject_signing_digest: Sha256::digest(
            subject
                .canonical_prepare_signing_bytes()
                .map_err(|e| e.to_string())?,
        )
        .into(),
        normalized_guard_digest: n.canonical_digest().map_err(|e| e.to_string())?,
        issued_at_ms: issued,
        expires_at_ms: expires,
        subject,
    };
    expected.canonical_signing_bytes()?;
    Ok(expected)
}

fn incoming_preparation_expiry(
    issued: u64,
    credential_expiry: u64,
    certificate_expiry: u64,
    integrity_expiry: u64,
    control_expiry: u64,
) -> Result<u64> {
    Ok(issued
        .checked_add(crate::kagemusha_v1_state::ORDINARY_PREPARATION_LIFETIME_MS)
        .ok_or_else(|| "incoming approval window overflow".to_owned())?
        .min(credential_expiry)
        .min(certificate_expiry)
        .min(integrity_expiry)
        .min(control_expiry))
}

fn require_source_selectors(
    reservation: &KagemushaOrdinaryIncomingReservationV1,
    finalized: DigestV1,
    proof: DigestV1,
    semantic: DigestV1,
    predecessor: DigestV1,
) -> Result<()> {
    reservation.validate_shape()?;
    if reservation.finalized_source_original_sha256 != finalized
        || reservation.source_proof_original_sha256 != proof
        || reservation.source_semantic_digest != semantic
        || reservation.selection.predecessor.state_original_sha256 != predecessor
    {
        return reject();
    }
    Ok(())
}
fn require_incoming_preparation(
    b: &KagemushaOrdinaryIncomingReservationProofBundleV1,
) -> Result<()> {
    let p = &b.preparation;
    let n = &b.normalized;
    let s = &b.statement;
    let approval = b.approval()?;
    if p.reservation_digest != b.reservation.digest()?
        || p.operation_id != b.reservation.selection.operation_id
        || p.operation_id != approval.challenge.operation_id
        || p.nonce != approval.challenge.nonce
        || p.transition_statement_digest != s.digest().map_err(|e| e.to_string())?
        || p.predecessor_state_commitment != s.predecessor_commitment
        || p.successor_state_commitment != s.successor_commitment
        || p.financial_index_before != approval.challenge.subject.secure_index_before
        || p.financial_index_after != approval.challenge.subject.secure_index_after
        || u128::from(p.logical_journal_sequence_before) != s.journal_revision_before
        || u128::from(p.logical_journal_sequence_after) != s.journal_revision_after
        || p.financial_control_original_sha256
            != <DigestV1>::from(Sha256::digest(&b.financial_control_original))
        || p.clock_context_digest != b.preparation_clock_context.binding_digest()?
        || n.transition_intent_digest != p.binding_digest()?
        || n.recovery_record_digest != p.recovery_binding_digest()?
        || n.transition_effect_digest != b.reservation.digest()?
        || n.terminal_commit_binding_digest != [0; 32]
        || n.sender_one_time_authorization_digest != [0; 32]
    {
        return reject();
    }
    Ok(())
}
fn require_financial_control(
    b: &KagemushaOrdinaryIncomingReservationProofBundleV1,
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
) -> Result<()> {
    require_financial_control_originals(
        b,
        credential,
        policy,
        verifier,
        &b.normalized,
        &b.financial_control_original,
        b.selected_integrity_original.as_deref(),
        &b.preparation_clock_context,
        &b.approval_admission_clock_context,
    )
}
fn require_financial_control_originals(
    b: &KagemushaOrdinaryIncomingReservationProofBundleV1,
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    policy: &KagemushaRetailEnrollmentIssuerPolicyV1,
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    normalized: &KagemushaNormalizedGuardStatementV1,
    control_original: &[u8],
    selected_integrity_original: Option<&[u8]>,
    preparation_clock: &KagemushaOrdinaryCashClockContextV1,
    admission_clock: &KagemushaOrdinaryCashClockContextV1,
) -> Result<()> {
    let certificate: KagemushaOrdinaryRetailEnrollmentCertificateV1 = data(
        &b.financial_certificate_original,
        KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
    )?;
    let control: KagemushaSignedOrdinaryCurrentControlV1 = data(
        control_original,
        KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
    )?;
    certificate
        .signature
        .verify(
            &policy.issuer_public_key,
            &certificate.subject.approval_payload()?,
        )
        .map_err(|e| e.to_string())?;
    control.verify_for_request(&control.subject.request, policy)?;
    let n = normalized;
    let c = credential.subject();
    let owner = &b.reservation.selection.lineage.owner;
    let release = verifier.monetary_release()?;
    if certificate.canonical_bytes().map_err(|e| e.to_string())? != b.financial_certificate_original
        || certificate.subject.owner != *owner
        || certificate.subject.issuer_policy_id != policy.issuer_policy_id
        || certificate.subject.issuer_audience != policy.issuer_audience
        || policy.runtime != owner.runtime
        || certificate.subject.ordinary_app_credential_digest != credential.digest()
        || certificate.subject.issuance.credential.canonical_bytes()? != credential.original()
        || certificate.subject.issuance.release_id != n.release_id
        || certificate.subject.issuance.hardware_policy_digest != release.hardware_policy_digest()
        || control.subject.request.owner != *owner
        || control.subject.request.enrollment_original_sha256
            != <DigestV1>::from(Sha256::digest(&b.financial_certificate_original))
        || control.subject.request.credential_original_sha256
            != <DigestV1>::from(Sha256::digest(credential.original()))
        || control.subject.release_id != n.release_id
        || control.subject.hardware_profile_id != n.hardware_profile_id
        || control.subject.profile_policy_epoch != n.policy_epoch
        || control.subject.ordinary_trust_policy_digest != c.trust_policy_digest
        || control.subject.app_authority_policy_digest != c.app_authority_policy_digest
        || control.subject.latest_integrity_lease_original.as_deref() != selected_integrity_original
        || control_original == b.mint_preparation_financial_control_original
        || preparation_clock.lower_at_ms < certificate.subject.issued_at_ms
        || admission_clock.upper_at_ms >= certificate.subject.expires_at_ms
        || preparation_clock.lower_at_ms < control.subject.issued_at_ms
        || admission_clock.upper_at_ms >= control.subject.expires_at_ms
    {
        return reject();
    }
    Ok(())
}
fn data<T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize>(
    raw: &[u8],
    max: usize,
) -> Result<T> {
    if raw.is_empty() || raw.len() > max {
        return reject();
    }
    let value: T =
        norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
            .map_err(|e| e.to_string())?;
    if norito::encode_canonical(&value).map_err(|e| e.to_string())? != raw {
        return reject();
    }
    Ok(value)
}
fn verify_incoming_state(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    bundle: &KagemushaOrdinaryIncomingReservationProofBundleV1,
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    approval: &KagemushaAppOperationApprovalV1,
    original_lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    incoming: IncomingStateSourceMetadata,
) -> Result<KagemushaVerifiedOrdinaryLineageStateProofV1> {
    let normalized = &bundle.normalized;
    let guard_original = bundle.guard_original.as_slice();
    let state_original = bundle.state_original.as_slice();
    let release = verifier.monetary_release()?;
    let material = verifier.state_checkpoint_material();
    let guard_material = verifier.ordinary_guard_verifier_material();
    let carrier = KagemushaOrdinaryLineageStateOriginalV1::decode_original(state_original)?;
    let projection_original = carrier.projection.canonical_bytes()?;
    let state_proof_original =
        norito::encode_canonical(&carrier.proof).map_err(|e| e.to_string())?;
    let projection = carrier.projection;
    let proof = decode_state_proof(&state_proof_original)?;
    let c = credential.subject();
    let challenge = &approval.challenge;
    let subject = &challenge.subject;
    let normalized_digest = normalized.canonical_digest().map_err(|e| e.to_string())?;
    let epoch = kagemusha_ordinary_financial_epoch_id_v1(c)?;
    if normalized.operation != KagemushaOperationV1::MintFold {
        return reject();
    }
    if c.release_id != release.release_id()
        || c.network_id != normalized.network_id
        || c.lane_id != normalized.lane_id
        || c.hardware_profile_id != normalized.hardware_profile_id
        || c.policy_epoch != normalized.policy_epoch
        || c.account_binding != challenge.account_binding
        || c.app_authority_policy_digest != challenge.authority_policy_digest
        || c.attested_key_id != challenge.attested_key_id
        || credential.digest() != challenge.enrollment_digest
        || credential.digest() != subject.credential_id
        || subject.release_id != normalized.release_id
        || subject.provider_policy_root != release.provider_policy_root()
        || subject.app_policy_digest != credential.static_binding_digest()
        || subject.network_id.as_bytes() != &normalized.network_id
        || subject.lane_commitment != normalized.lane_id
        || subject.hardware_profile_id != normalized.hardware_profile_id
        || subject.policy_epoch != normalized.policy_epoch
        || subject.hardware_epoch_id != epoch
        || u128::from(subject.hardware_epoch_generation)
            != normalized.successor_hardware_epoch_generation
        || subject.operation_kind != KagemushaOperationKindV1::from(normalized.operation)
        || challenge.normalized_guard_digest != normalized_digest
        || normalized.successor_hardware_epoch_generation != u128::from(c.hardware_epoch)
        || normalized.successor_hardware_epoch_id != epoch
        || normalized.successor_hardware_policy_id != release.provider_policy_root()
        || normalized.successor_key_reference != c.app_key_reference
        || normalized.release_id != material.binding.release_id
        || normalized.successor_suite_id != material.binding.suite_id
        || normalized.successor_vk_digest != material.binding.vk_set_digest
        || release
            .enabled_profile(c.hardware_profile_id)
            .is_none_or(|p| p.policy_epoch != c.policy_epoch)
    {
        return reject();
    }
    require_ordinary_state_nonces(
        normalized.predecessor_state_nonce_commitment,
        normalized.successor_state_nonce_commitment,
        false,
    )?;
    if normalized.amount == 0
        || normalized.predecessor_hardware_epoch_id != epoch
        || normalized.predecessor_hardware_epoch_generation != u128::from(c.hardware_epoch)
        || normalized.predecessor_hardware_policy_id != release.provider_policy_root()
        || normalized.predecessor_key_reference != c.app_key_reference
        || normalized.predecessor_state_commitment == [0; 32]
        || normalized.predecessor_logical_sequence.checked_add(1)
            != Some(normalized.successor_logical_sequence)
        || subject.secure_index_before.checked_add(1) != Some(subject.secure_index_after)
        || challenge.purpose != KagemushaAppOperationApprovalPurposeV1::PrepareTransition
        || subject.candidate_envelope_digest != [0; 32]
        || subject.terminal_body_commitment != [0; 32]
    {
        return reject();
    }
    let context = crate::kagemusha_v1_state::ordinary_incoming_guard_context_v1(
        material.artifacts,
        &bundle.statement,
        &bundle.preparation,
        bundle.preparation_clock_context.upper_at_ms,
    )
    .map_err(|e| e.to_string())?;
    let derived =
        KagemushaNormalizedGuardStatementV1::derive_from_transition(&bundle.statement, context)
            .map_err(|e| e.to_string())?;
    let statement_digest = bundle.statement.digest().map_err(|e| e.to_string())?;
    if derived != *normalized || statement_digest != subject.transition_statement_digest {
        return reject();
    }
    let full_s = challenge.canonical_subject_signing_bytes()?;
    let subject_digest: DigestV1 = Sha256::digest(full_s).into();
    if subject_digest != challenge.subject_signing_digest {
        return reject();
    }
    let authorization = kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
        kagemusha_ordinary_app_approval_proof_binding_digest_v1(approval)?,
        original_lease.map(|p| Sha256::digest(p.original()).into()),
    )?;
    let digests = [
        normalized_digest,
        credential.digest(),
        authorization,
        subject_digest,
        release.provider_policy_root(),
    ];
    ordinary_guard_verifier::verify_stateless_original_v1(guard_original, &guard_material, digests)
        .map_err(|e| e.to_string())?;
    require_state_metadata(
        &projection,
        &proof,
        &material,
        &guard_material,
        normalized,
        normalized_digest,
        subject.transition_statement_digest,
        None,
        authorization,
        challenge.operation_id,
        Some(incoming),
    )?;
    let transport = crate::kagemusha_v1_state::ordinary_incoming_transport_semantic_digest_v1(
        &bundle.statement,
        normalized_digest,
    )
    .map_err(|e| e.to_string())?;
    if proof.semantic_digest != transport {
        return reject();
    }
    let (mut eq, mut ep) = projection.fields()?;
    let candidate_eq = canonical_terminal_authorization_candidate_digest_v1(&[eq.clone()])?;
    let candidate_ep = canonical_terminal_authorization_candidate_digest_v1(&[ep.clone()])?;
    if candidate_eq != candidate_ep {
        return reject();
    }
    append_history(&mut eq, &proof.eq_history)?;
    append_history(&mut ep, &proof.ep_history)?;
    verify_state_histories(&material, &proof, &eq, &ep)?;
    let approval_original = norito::encode_canonical(approval).map_err(|e| e.to_string())?;
    Ok(KagemushaVerifiedOrdinaryLineageStateProofV1 {
        normalized: *normalized,
        credential: credential.digest(),
        financial_epoch: epoch,
        financial_authority: c.financial_authority_commitment,
        candidate: candidate_eq,
        state_statement: subject.transition_statement_digest,
        projection_original_sha256: Sha256::digest(&projection_original).into(),
        state_proof_original_sha256: Sha256::digest(&state_proof_original).into(),
        state_original_sha256: Sha256::digest(state_original).into(),
        guard_original_sha256: Sha256::digest(guard_original).into(),
        approval_original_sha256: Sha256::digest(approval_original).into(),
        authorization,
    })
}

/// Native-only assembly and admission from the same captured incoming operation and genuine
/// candidate/Guard/source/clock loans. No caller State, time, FI decision or offered source enters.
pub(crate) fn assemble_ordinary_incoming_reservation_v1(
    selection: &KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1<'_>,
    candidate: &super::super::KagemushaAuthenticatedOrdinaryIncomingCandidateV1,
    guard: &super::super::KagemushaAuthenticatedOrdinaryIncomingPreparationGuardV1,
) -> Result<KagemushaVerifiedOrdinaryIncomingReservationProofV1> {
    use crate::kagemusha_v1_state::KagemushaStateErrorV1;
    selection
        .recheck_selected_originals_and_current_custody()
        .map_err(|e| e.to_string())?;
    candidate
        .recheck_incoming_selection(selection, guard)
        .map_err(|e| e.to_string())?;
    let mut accepted = None;
    let mut source_visits = 0;
    selection
        .with_finalized_mint_source(&mut |source, credit| {
            source_visits += 1;
            if source_visits != 1 {
                return Err(KagemushaStateErrorV1::InvalidCandidateStage);
            }
            let mut preparation_visits = 0;
            selection.with_verified_preparation_clock(&mut |preparation_clock| {
                preparation_visits += 1;
                if preparation_visits != 1 {
                    return Err(KagemushaStateErrorV1::InvalidCandidateStage);
                }
                let mut capture_visits = 0;
                selection.with_verified_approval_clock(&mut |capture_clock| {
                    capture_visits += 1;
                    if capture_visits != 1 {
                        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
                    }
                    let authorization = source.authorization().map_err(native_error)?;
                    let bundle = KagemushaOrdinaryIncomingReservationProofBundleV1 {
                        version: 1,
                        reservation: selection.reservation()?.clone(),
                        normalized: *selection.normalized_guard_statement()?,
                        statement: selection.transition_statement()?.clone(),
                        preparation: *selection.preparation()?,
                        predecessor_state_original: selection
                            .predecessor_public_state_original()?
                            .to_vec(),
                        state_original: candidate.public_state_original().to_vec(),
                        credential_original: selection.credential()?.original().to_vec(),
                        approval_original: selection.original()?.to_vec(),
                        guard_original: guard.original().to_vec(),
                        selected_integrity_original: selection
                            .selected_integrity_lease()?
                            .map(|l| l.original().to_vec()),
                        financial_certificate_original: selection
                            .enrollment()?
                            .certificate()
                            .canonical_bytes()
                            .map_err(native_error)?,
                        financial_control_original: selection.financial_control_original()?,
                        preparation_clock_context: selection.preparation_clock_context()?.clone(),
                        preparation_clock_signed_original: preparation_clock.original().to_vec(),
                        approval_admission_clock_context: selection
                            .approval_admission_clock_context()?
                            .clone(),
                        approval_admission_clock_signed_original: capture_clock.original().to_vec(),
                        finalized_mint_original: source
                            .finalized_original()
                            .map_err(native_error)?
                            .to_vec(),
                        mint_credit_original: norito::encode_canonical(credit)
                            .map_err(native_error)?,
                        mint_preparation_financial_control_original: source
                            .preparation_financial_control_original()
                            .map_err(native_error)?
                            .to_vec(),
                        mint_selected_integrity_original: authorization
                            .selected_integrity_original()
                            .map(<[u8]>::to_vec),
                        mint_preparation_clock_signed_original: authorization
                            .preparation_clock_original()
                            .to_vec(),
                    };
                    let admitted = admit(
                        selection.recursive_verifier(),
                        &bundle,
                        MintSource::Native(source),
                        selection.credential()?,
                        selection.selected_integrity_lease()?,
                        [preparation_clock, capture_clock],
                    )
                    .map_err(native_error)?;
                    if admitted.state_proof().state_original_sha256()
                        != candidate.candidate_original_sha256()
                        || admitted.state_proof().guard_original_sha256()
                            != candidate.guard_original_sha256()
                    {
                        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                    }
                    accepted = Some(admitted);
                    Ok(())
                })?;
                if capture_visits != 1 {
                    return Err(KagemushaStateErrorV1::InvalidCandidateStage);
                }
                Ok(())
            })?;
            if preparation_visits != 1 {
                return Err(KagemushaStateErrorV1::InvalidCandidateStage);
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    if source_visits != 1 {
        return reject();
    }
    candidate
        .recheck_incoming_selection(selection, guard)
        .map_err(|e| e.to_string())?;
    selection
        .recheck_selected_originals_and_current_custody()
        .map_err(|e| e.to_string())?;
    accepted.ok_or_else(rejection)
}
fn native_error(e: impl core::fmt::Display) -> crate::kagemusha_v1_state::KagemushaStateErrorV1 {
    crate::kagemusha_v1_state::KagemushaStateErrorV1::ProofRejected(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incoming_approval_uses_native_preparation_window_and_each_authentic_expiry() {
        let issued = 1_000;
        assert_eq!(
            incoming_preparation_expiry(issued, 200_000, 200_000, 200_000, 200_000).unwrap(),
            11_000
        );
        for expiry in [2_000, 10_999, 11_000, 200_000] {
            let expected = expiry.min(11_000);
            for index in 0..4 {
                let mut bounds = [200_000; 4];
                bounds[index] = expiry;
                assert_eq!(
                    incoming_preparation_expiry(issued, bounds[0], bounds[1], bounds[2], bounds[3])
                        .unwrap(),
                    expected
                );
            }
        }
        assert!(
            incoming_preparation_expiry(u64::MAX - 9_999, u64::MAX, u64::MAX, u64::MAX, u64::MAX)
                .is_err()
        );
    }

    fn reservation() -> KagemushaOrdinaryIncomingReservationV1 {
        let f =
            iroha_data_model::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_v1();
        let request = f.request;
        let context = &request.authorization.statement.context;
        KagemushaOrdinaryIncomingReservationV1 {
            selection: KagemushaOrdinaryIncomingSelectionV1 {
                version: 1,
                lineage: context.lineage.clone(),
                operation_id: context.operation_id,
                predecessor: context.predecessor.clone(),
                source: KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
                    topup_request_original_sha256: Sha256::digest(
                        request.canonical_bytes().unwrap(),
                    )
                    .into(),
                },
                credit_id: request.authorization.statement.credit_id,
                amount: context.amount,
                scale: context.lineage.owner.runtime.scale,
                recipient_app_credential_digest: context.recipient_app_credential_digest,
                financial_control_original_sha256: context.financial_control_original_sha256,
                clock_context_digest: context.clock_context.binding_digest().unwrap(),
            },
            finalized_source_original_sha256: [21; 32],
            source_proof_original_sha256: [22; 32],
            source_semantic_digest: [23; 32],
        }
    }
    #[test]
    fn incoming_source_and_whole_public_predecessor_original_identity_are_independent() {
        // Data-only transcript fixture; inert source/proofs never create an admitted capability.
        let r = reservation();
        let parent = r.selection.predecessor.state_original_sha256;
        require_source_selectors(&r, [21; 32], [22; 32], [23; 32], parent).unwrap();
        for mut digests in [[[21; 32], [22; 32], [23; 32], parent]; 4]
            .into_iter()
            .enumerate()
        {
            digests.1[digests.0][0] ^= 1;
            assert!(
                require_source_selectors(
                    &r,
                    digests.1[0],
                    digests.1[1],
                    digests.1[2],
                    digests.1[3]
                )
                .is_err()
            );
        }
        let mut changed = r.clone();
        changed.selection.predecessor.state_original_sha256[0] ^= 1;
        assert!(require_source_selectors(&changed, [21; 32], [22; 32], [23; 32], parent).is_err());
        let envelope = r.digest().unwrap();
        let mut changed = r;
        changed.source_proof_original_sha256[0] ^= 1;
        assert_ne!(changed.digest().unwrap(), envelope);
        assert_eq!(
            changed.selection.credit_id,
            reservation().selection.credit_id
        );
    }
    #[test]
    fn incoming_carrier_budget_retains_full_three_clock_and_finality_maxima() {
        let original_parts = KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1
            + 3 * KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1;
        assert!(KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1 > original_parts);
        assert!(original_parts > 84 * 1024 * 1024);
        // No giant allocation: finite maximum arithmetic only. It admits no proof or device.
        let base64 = KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1
            .checked_add(2)
            .unwrap()
            .checked_div(3)
            .unwrap()
            .checked_mul(4)
            .unwrap();
        assert!(base64 > 112 * 1024 * 1024);
    }
}
