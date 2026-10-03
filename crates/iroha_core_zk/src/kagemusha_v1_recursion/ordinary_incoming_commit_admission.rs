//! Independent full incoming Commit composition: genuine prepared State and both purpose Guards.
//! No outgoing Wrapper/output role is aliased. All current proofs and entire carried histories
//! are independently verified under actual released material; global effects remain owner-only.
use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaAuthenticatedOrdinaryIncomingTerminalGuardV1,
    verify_ordinary_incoming_terminal_guard_v1,
};
use crate::kagemusha_v1_state::{
    KagemushaAuthenticatedOrdinaryIncomingReservationReceiptV1,
    KagemushaAuthenticatedOrdinaryIncomingTerminalApprovalSelectionV1,
    KagemushaAuthenticatedOrdinaryServiceIncomingReservationAssertionV1, KagemushaStateErrorV1,
    KagemushaStateV1,
};
use zeroize::Zeroize as _;

/// Structural complete frame bound retaining all existing maxima: the whole reservation bundle
/// (three signed clocks/finalized debit) plus two genuine W1 signed clocks, same-source Reserve
/// signed original, fresh FI/PI/W1/Guard originals, and finite framing. Exact proof transcript
/// lengths remain additionally pinned to the admitted protocols before actual Native allocation.
pub const KAGEMUSHA_ORDINARY_INCOMING_COMMIT_BUNDLE_MAX_BYTES_V1: usize =
    KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1
        + 2 * KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1
        + KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1
        + KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1
        + 4096
        + KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1
        + KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1
        + 64 * 1024;

/// Sole complete portable incoming Commit DATA. Decoding creates no financial, source, time,
/// Reserve or proof capability. Independent admission requires those separately authentic owners.
#[derive(Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::KagemushaOrdinaryIncomingCommitProofBundleV1")]
pub struct KagemushaOrdinaryIncomingCommitProofBundleV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Acyclic global Commit selectors; the proof-bundle selector is the earlier W2 bundle SHA.
    pub commit: KagemushaOrdinaryIncomingCommitV1,
    /// Complete previously admitted source/predecessor/current State/W2 Guard/clock/FI bundle.
    pub reservation_proof_original: Vec<u8>,
    /// Full purpose1 normalized Guard derived from the exact incoming body.
    pub terminal_normalized: KagemushaNormalizedGuardStatementV1,
    /// Sole full693-byte Model body; its complete661-byte intent is contained unchanged.
    pub terminal_body: KagemushaOrdinaryIncomingTerminalBodyV1,
    /// Complete actual purpose1 platform approval, including original DER/App Attest assertion.
    pub terminal_approval_original: Vec<u8>,
    /// Real revised ordinary Guard pair and both full histories, not an outgoing Wrapper.
    pub terminal_guard_original: Vec<u8>,
    /// Exact selected W1 PI refresh, independent from older Mint and W2 leases.
    pub terminal_integrity_original: Option<Vec<u8>>,
    /// Complete independently acknowledged Reserve signed result selected by actual Native CAS.
    pub reserve_receipt_original: Vec<u8>,
    /// Full fresh W1 FI-control original, independently captured after State proving.
    pub terminal_financial_control_original: Vec<u8>,
    /// Complete four signed originals behind the exact W1 intent clock context.
    pub terminal_intent_clock_signed_original: Vec<u8>,
    /// Actual post-platform fsynced CaptureAck context, not offered timestamp scalars.
    pub terminal_admission_clock_context: KagemushaOrdinaryCashClockContextV1,
    /// Complete four signed originals behind that actual W1 CaptureAck.
    pub terminal_admission_clock_signed_original: Vec<u8>,
}
impl KagemushaOrdinaryIncomingCommitProofBundleV1 {
    fn validate_data(&self) -> Result<()> {
        if self.version != 1 {
            return reject();
        }
        self.commit.validate_shape()?;
        self.terminal_body.intent.validate_shape()?;
        self.terminal_normalized
            .canonical_digest()
            .map_err(|e| e.to_string())?;
        self.terminal_admission_clock_context.validate_shape()?;
        for (raw, max) in [
            (
                &self.reservation_proof_original,
                KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1,
            ),
            (
                &self.terminal_approval_original,
                KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1,
            ),
            (
                &self.terminal_guard_original,
                KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1,
            ),
            (
                &self.reserve_receipt_original,
                KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1,
            ),
            (
                &self.terminal_financial_control_original,
                KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
            ),
            (
                &self.terminal_intent_clock_signed_original,
                KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1,
            ),
            (
                &self.terminal_admission_clock_signed_original,
                KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1,
            ),
        ] {
            if raw.is_empty() || raw.len() > max {
                return reject();
            }
        }
        if self
            .terminal_integrity_original
            .as_ref()
            .is_some_and(|r| r.is_empty() || r.len() > 4096)
        {
            return reject();
        }
        Ok(())
    }
    /// Complete sole bounded canonical original. DATA shape is not a proof/effect admission.
    /// # Errors
    /// Refuses unsupported/missing/oversized originals or serialization failure.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate_data()?;
        let raw = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if raw.len() > KAGEMUSHA_ORDINARY_INCOMING_COMMIT_BUNDLE_MAX_BYTES_V1 {
            return reject();
        }
        Ok(raw)
    }
    /// Strict bounded DATA decoder; no constructor upgrades decoded data into an admission.
    /// # Errors
    /// Refuses trailing/noncanonical/oversized/malformed complete originals.
    pub fn decode_original(raw: &[u8]) -> Result<Self> {
        if raw.is_empty() || raw.len() > KAGEMUSHA_ORDINARY_INCOMING_COMMIT_BUNDLE_MAX_BYTES_V1 {
            return reject();
        }
        let v: Self =
            norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
                .map_err(|e| e.to_string())?;
        if v.canonical_bytes()? != raw {
            return reject();
        }
        Ok(v)
    }
    /// Same complete earlier W2 data, never a reconstructed State or source authority.
    /// # Errors
    /// Refuses another reservation bundle original.
    pub fn reservation_bundle(&self) -> Result<KagemushaOrdinaryIncomingReservationProofBundleV1> {
        KagemushaOrdinaryIncomingReservationProofBundleV1::decode_original(
            &self.reservation_proof_original,
        )
    }
}

/// Opaque independent full incoming Commit proof admission. No decoder, public constructor or
/// Clone exists. Fresh effect FI/clock and authentic DATA Commit are separate owner capabilities.
pub struct KagemushaVerifiedOrdinaryIncomingCommitProofV1 {
    commit: KagemushaOrdinaryIncomingCommitV1,
    reservation: KagemushaVerifiedOrdinaryIncomingReservationProofV1,
    original: Vec<u8>,
}
impl KagemushaVerifiedOrdinaryIncomingCommitProofV1 {
    /// Exact full source/head/successor selectors independently admitted here.
    pub fn commit(&self) -> &KagemushaOrdinaryIncomingCommitV1 {
        &self.commit
    }
    /// Actual admitted State/Guard/Mint release, never a caller-supplied profile ID.
    pub fn release_id(&self) -> DigestV1 {
        self.reservation.release_id()
    }
    /// Genuine earlier source/State/W2 Guard admission including full immutable predecessor.
    pub fn reservation_proof(&self) -> &KagemushaVerifiedOrdinaryIncomingReservationProofV1 {
        &self.reservation
    }
    /// Exact full canonical complete Commit carrier retained by this actual admission.
    pub fn original(&self) -> &[u8] {
        &self.original
    }
    /// Raw SHA of that complete original; this grants no DATA or Native authority.
    pub fn original_sha256(&self) -> DigestV1 {
        sha(&self.original)
    }
}

/// Independently verify genuine full State/W2/W1 Guard composition using actual installed
/// source/Reserve/C/PI/clock capabilities. Ordered clocks are W2prep,W2capture,W1intent,W1capture;
/// the earlier Mint clock remains separately authenticated by the actual finalized-source owner.
/// This authenticates original signed intervals, never sender elapsed time or a service effect.
/// # Errors
/// Refuses any original/protocol/source/Reserve/FI/clock/body/state/approval substitution, wrong
/// purpose or failed actual proof/full-history verification; each source variant is independently admitted.
pub fn verify_ordinary_incoming_commit_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    bundle: &KagemushaOrdinaryIncomingCommitProofBundleV1,
    source: &KagemushaOrdinaryIncomingServiceSourceV1<'_, '_>,
    reserve: &KagemushaAuthenticatedOrdinaryServiceIncomingReservationAssertionV1<'_>,
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    w2_lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    w1_lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    clocks: [&KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1; 4],
) -> Result<KagemushaVerifiedOrdinaryIncomingCommitProofV1> {
    bundle.validate_data()?;
    reserve
        .recheck_retained_custody()
        .map_err(|e| e.to_string())?;
    let reservation_bundle = bundle.reservation_bundle()?;
    let reservation = verify_ordinary_incoming_reservation_v1(
        verifier,
        &reservation_bundle,
        source,
        credential,
        w2_lease,
        [clocks[0], clocks[1]],
    )?;
    if reserve.reservation().map_err(|e| e.to_string())? != reservation.reservation()
        || reserve.original().map_err(|e| e.to_string())? != bundle.reserve_receipt_original
        || reserve
            .request_original_sha256()
            .map_err(|e| e.to_string())?
            != bundle.terminal_body.intent.reserve_request_original_sha256
        || reserve.issuer_policy().map_err(|e| e.to_string())? != source.issuer_policy()?
    {
        return reject();
    }
    require_terminal(
        verifier,
        bundle,
        &reservation_bundle,
        &reservation,
        credential,
        w1_lease,
        source.issuer_policy()?,
        [clocks[2], clocks[3]],
    )?;
    source.recheck()?;
    reserve
        .recheck_retained_custody()
        .map_err(|e| e.to_string())?;
    let original = bundle.canonical_bytes()?;
    Ok(KagemushaVerifiedOrdinaryIncomingCommitProofV1 {
        commit: bundle.commit.clone(),
        reservation,
        original,
    })
}
fn require_terminal(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    b: &KagemushaOrdinaryIncomingCommitProofBundleV1,
    p: &KagemushaOrdinaryIncomingReservationProofBundleV1,
    proof: &KagemushaVerifiedOrdinaryIncomingReservationProofV1,
    c: &KagemushaVerifiedOrdinaryAppCredentialV1,
    lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    issuer: &KagemushaRetailEnrollmentIssuerPolicyV1,
    clocks: [&KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1; 2],
) -> Result<()> {
    b.validate_data()?;
    let i = &b.terminal_body.intent;
    let normalized = &b.terminal_normalized;
    let approval: KagemushaAppOperationApprovalV1 = data(
        &b.terminal_approval_original,
        KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1,
    )?;
    let challenge = &approval.challenge;
    let body_digest = b.terminal_body.binding_digest()?;
    let intent_digest = i.binding_digest()?;
    let mut expected_normalized = p.normalized;
    expected_normalized.terminal_commit_binding_digest =
        kagemusha_ordinary_terminal_guard_commit_binding_digest_v1(
            body_digest,
            sha(&p.state_original),
            sha(&p.state_original),
            p.reservation.digest()?,
        )?;
    expected_normalized.sender_one_time_authorization_digest = [0; 32];
    expected_normalized.transition_intent_digest = body_digest;
    expected_normalized.recovery_record_digest = intent_digest;
    let mut expected_subject =
        expected_incoming_preparation_challenge(p, c, lease, verifier)?.subject;
    expected_subject.candidate_envelope_digest = sha(&p.state_original);
    expected_subject.terminal_body_commitment = body_digest;
    let transition_original = norito::encode_canonical(&p.statement).map_err(|e| e.to_string())?;
    let signed: KagemushaSignedOrdinaryLineageResultV1 = data(
        &b.reserve_receipt_original,
        KAGEMUSHA_ORDINARY_LINEAGE_ORIGINAL_MAX_BYTES_V1,
    )?;
    signed.verify_for_request(&signed.subject.request, &issuer.issuer_public_key)?;
    let requested = match &signed.subject.request.operation {
        KagemushaOrdinaryLineageRequestOperationV1::ReserveIncoming(r) => r.as_ref(),
        _ => return reject(),
    };
    let capture = &b.terminal_admission_clock_context;
    if requested != proof.reservation()
        || proof.reservation() != &b.commit.reservation
        || p.reservation != b.commit.reservation
        || signed.subject.release_id != proof.release_id()
        || i.reserve_request_original_sha256 != sha(&signed.subject.request.canonical_bytes()?)
        || i.reserve_receipt_original_sha256 != sha(&b.reserve_receipt_original)
        || i.preparation_digest != p.preparation.binding_digest()?
        || i.reservation_digest != p.reservation.digest()?
        || i.finalized_source_original_sha256 != p.reservation.finalized_source_original_sha256
        || i.source_proof_original_sha256 != p.reservation.source_proof_original_sha256
        || i.state_original_sha256 != sha(&p.state_original)
        || i.candidate_original_sha256 != sha(&p.state_original)
        || i.transition_statement_original_sha256 != sha(&transition_original)
        || i.preparation_guard_original_sha256 != sha(&p.guard_original)
        || i.purpose2_approval_original_sha256 != sha(&p.approval_original)
        || i.financial_control_original_sha256 != sha(&b.terminal_financial_control_original)
        || b.terminal_financial_control_original == p.financial_control_original
        || i.clock_context.request_nonce == p.preparation_clock_context.request_nonce
        || i.clock_context.request_nonce == p.approval_admission_clock_context.request_nonce
        || i.clock_context.lower_at_ms < p.approval_admission_clock_context.lower_at_ms
        || i.clock_context.upper_at_ms < p.approval_admission_clock_context.upper_at_ms
        || i.clock_context.lower_at_ms < p.preparation_clock_context.lower_at_ms
        || i.clock_context.upper_at_ms < p.preparation_clock_context.upper_at_ms
        || capture.lower_at_ms < i.clock_context.lower_at_ms
        || capture.upper_at_ms < i.clock_context.upper_at_ms
        || i.native_operation_id == p.preparation.operation_id
        || i.native_nonce == p.preparation.nonce
        || i.financial_index_before != p.preparation.financial_index_before
        || i.financial_index_after != p.preparation.financial_index_after
        || i.financial_sequence_before != p.statement.predecessor_sequence
        || i.financial_sequence_after != p.statement.successor_sequence
        || i.logical_journal_sequence_before != p.preparation.logical_journal_sequence_before
        || i.logical_journal_sequence_after != p.preparation.logical_journal_sequence_after
        || i.operation != 1
        || p.statement.kind != KagemushaTransitionKindV1::MintFold
        || normalized != &expected_normalized
        || challenge.subject != expected_subject
        || challenge.purpose != KagemushaAppOperationApprovalPurposeV1::MonetaryTransition
        || challenge.operation_id != i.native_operation_id
        || challenge.nonce != i.native_nonce
        || challenge.enrollment_digest != c.digest()
        || challenge.normalized_guard_digest
            != normalized.canonical_digest().map_err(|e| e.to_string())?
        || challenge.account_binding != c.subject().account_binding
        || challenge.authority_policy_digest != c.subject().app_authority_policy_digest
        || challenge.attested_key_id != c.subject().attested_key_id
        || challenge.issued_at_ms != i.issued_at_ms
        || challenge.expires_at_ms != i.expires_at_ms
        || b.terminal_integrity_original.as_deref() != lease.map(|l| l.original())
        || b.commit.state_proof_bundle_original_sha256 != proof.original_sha256()
        || b.reservation_proof_original != proof.original()
        || b.commit.successor.state_commitment != p.normalized.successor_state_commitment
        || b.commit.successor.logical_sequence != p.normalized.successor_logical_sequence
        || b.commit.successor.state_original_sha256 != sha(&p.state_original)
        || b.commit.transition_statement_original_sha256 != sha(&transition_original)
        || b.commit.purpose1_approval_original_sha256 != sha(&b.terminal_approval_original)
        || b.commit.financial_control_original_sha256 != sha(&b.terminal_financial_control_original)
        || b.commit.admission_clock_context_original_sha256
            != sha(&norito::encode_canonical(capture).map_err(|e| e.to_string())?)
    {
        return reject();
    }
    for (clock, raw, context) in [
        (
            clocks[0],
            &b.terminal_intent_clock_signed_original,
            &i.clock_context,
        ),
        (
            clocks[1],
            &b.terminal_admission_clock_signed_original,
            capture,
        ),
    ] {
        if clock.original() != raw {
            return reject();
        }
        clock
            .recheck_cash_context(context)
            .map_err(|e| e.to_string())?;
        context.validate_within_original_window(i.issued_at_ms, i.expires_at_ms)?;
    }
    // Reuse exactly the full certificate/control equation with the separately fresh W1 original
    // and actual W1 signed intervals. The older pre-debit Mint control remains distinct.
    require_financial_control_originals(
        p,
        c,
        issuer,
        verifier,
        normalized,
        &b.terminal_financial_control_original,
        b.terminal_integrity_original.as_deref(),
        &i.clock_context,
        capture,
    )?;
    if i.issued_at_ms < c.subject().issued_at_ms || i.expires_at_ms > c.subject().expires_at_ms {
        return reject();
    }
    for now in [
        i.clock_context.lower_at_ms,
        i.clock_context.upper_at_ms,
        capture.lower_at_ms,
        capture.upper_at_ms,
    ] {
        match lease {
            Some(l) => c.recheck_with_integrity_lease(l, now)?,
            None => c.recheck_at_trusted_time(now)?,
        }
    }
    let floor = match c.subject().platform_class {
        KagemushaHardwarePlatformClassV1::AndroidKeyMint => None,
        KagemushaHardwarePlatformClassV1::AppleAppAttest => {
            Some(c.subject().app_attest_counter_floor)
        }
        _ => return reject(),
    };
    let mut expected = expected_incoming_preparation_challenge(p, c, lease, verifier)?;
    expected.purpose = KagemushaAppOperationApprovalPurposeV1::MonetaryTransition;
    expected.operation_id = i.native_operation_id;
    expected.nonce = i.native_nonce;
    expected.subject = expected_subject;
    expected.subject_signing_digest = sha(&expected
        .subject
        .canonical_ordinary_incoming_terminal_signing_bytes()
        .map_err(|e| e.to_string())?);
    expected.normalized_guard_digest = normalized.canonical_digest().map_err(|e| e.to_string())?;
    let certificate: KagemushaOrdinaryRetailEnrollmentCertificateV1 = data(
        &p.financial_certificate_original,
        KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
    )?;
    let control: KagemushaSignedOrdinaryCurrentControlV1 = data(
        &b.terminal_financial_control_original,
        KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
    )?;
    let integrity_expiry = lease.map_or_else(
        || {
            c.subject()
                .play_integrity
                .map_or(c.subject().expires_at_ms, |pi| pi.refresh_before_ms)
        },
        |l| {
            l.subject()
                .expires_at_ms
                .min(l.subject().binding.refresh_before_ms)
        },
    );
    expected.issued_at_ms = i.clock_context.lower_at_ms;
    expected.expires_at_ms = expected
        .issued_at_ms
        .checked_add(KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1)
        .ok_or_else(rejection)?
        .min(c.subject().expires_at_ms)
        .min(certificate.subject.expires_at_ms)
        .min(integrity_expiry)
        .min(control.subject.expires_at_ms);
    if expected.issued_at_ms != i.issued_at_ms || expected.expires_at_ms != i.expires_at_ms {
        return reject();
    }
    for now in [
        i.clock_context.lower_at_ms,
        i.clock_context.upper_at_ms,
        capture.lower_at_ms,
        capture.upper_at_ms,
    ] {
        match lease {
            Some(l) => approval.authenticate_with_integrity_lease(&expected, c, l, floor, now)?,
            None => approval.authenticate(&expected, c, floor, now)?,
        };
    }
    let subject_digest = sha(&challenge.canonical_subject_signing_bytes()?);
    if subject_digest != challenge.subject_signing_digest {
        return reject();
    }
    let authorization = kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
        kagemusha_ordinary_app_approval_proof_binding_digest_v1(&approval)?,
        lease.map(|l| sha(l.original())),
    )?;
    ordinary_guard_verifier::verify_stateless_original_v1(
        &b.terminal_guard_original,
        &verifier.ordinary_guard_verifier_material(),
        [
            challenge.normalized_guard_digest,
            c.digest(),
            authorization,
            subject_digest,
            verifier.monetary_release()?.provider_policy_root(),
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn sha(raw: &[u8]) -> DigestV1 {
    Sha256::digest(raw).into()
}

/// The sole finite canonical incoming Commit maximum for Native physical byte accounting.
/// It includes five full signed-clock frames and full finalized debit, not timestamp hashes.
/// Actual accepted proof lengths remain fixed by the installed protocols, and actual serialized
/// WAL framing must be counted independently before dispatching any global Commit/effect.
#[must_use]
pub const fn ordinary_incoming_commit_carrier_max_bytes_v1() -> usize {
    KAGEMUSHA_ORDINARY_INCOMING_COMMIT_BUNDLE_MAX_BYTES_V1
}

/// Actual immutable proof result and selected private/public successor. It cannot be decoded or
/// copied into authority; only actual loans, full proof verification and genuine CAS custody
/// construct it. A later fresh FI/clock plus actual acknowledged global Commit remain mandatory.
pub(crate) struct GeneratedOrdinaryIncomingCommitOriginalsV1 {
    proof: KagemushaVerifiedOrdinaryIncomingCommitProofV1,
    selected_successor_state: KagemushaStateV1,
    successor_public_state_original: Vec<u8>,
    successor_private_checkpoint_original: Vec<u8>,
    successor_public_inputs: crate::kagemusha_v1_recursion::KagemushaStateRelationPublicInputsV1,
    private_service_original: Vec<u8>,
}
impl Drop for GeneratedOrdinaryIncomingCommitOriginalsV1 {
    fn drop(&mut self) {
        if let Some(before) = self.successor_public_inputs.predecessor.as_mut() {
            before.balance.zeroize();
            before.state_nonce_commitment.zeroize();
        }
        self.successor_public_inputs.successor.balance.zeroize();
        self.successor_public_inputs
            .successor
            .state_nonce_commitment
            .zeroize();
        self.successor_private_checkpoint_original.zeroize();
        self.selected_successor_state.balance.zeroize();
        self.selected_successor_state
            .state_nonce_commitment
            .zeroize();
    }
}
impl GeneratedOrdinaryIncomingCommitOriginalsV1 {
    pub(crate) fn proof(&self) -> &KagemushaVerifiedOrdinaryIncomingCommitProofV1 {
        &self.proof
    }
    pub(crate) fn commit(&self) -> &KagemushaOrdinaryIncomingCommitV1 {
        self.proof.commit()
    }
    pub(crate) fn selected_successor_state(&self) -> &KagemushaStateV1 {
        &self.selected_successor_state
    }
    pub(crate) fn successor_public_state_original(&self) -> &[u8] {
        &self.successor_public_state_original
    }
    pub(crate) fn successor_private_checkpoint_original(&self) -> &[u8] {
        &self.successor_private_checkpoint_original
    }
    pub(crate) fn with_successor_checkpoint(
        &self,
        verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
        consume: &mut dyn for<'a> FnMut(
            &'a crate::kagemusha_v1_recursion::KagemushaGeneratedRecursiveStateProofV1,
        ) -> core::result::Result<(), KagemushaStateErrorV1>,
    ) -> core::result::Result<(), KagemushaStateErrorV1> {
        let restored = crate::kagemusha_v1_recursion::KagemushaRecursiveStateCheckpointV1::decode_canonical_exact(
            &self.successor_private_checkpoint_original,
            verifier,
        )
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?
        .restore(verifier, &self.successor_public_inputs)
        .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
        consume(&restored)
    }
    pub(crate) fn private_service_original(&self) -> &[u8] {
        &self.private_service_original
    }
}

/// Assemble/reverify all actual retained incoming proofs and originals. This performs no key
/// resolution, generation or randomized proving and is also the exact durable readmission kernel.
/// Mint and Receive require their distinct genuine source loans and exact same immutable carrier.
pub(crate) fn assemble_ordinary_incoming_commit_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    selection: &KagemushaAuthenticatedOrdinaryIncomingTerminalApprovalSelectionV1<'_>,
    terminal_guard: &KagemushaAuthenticatedOrdinaryIncomingTerminalGuardV1,
    reserve: &KagemushaAuthenticatedOrdinaryIncomingReservationReceiptV1<'_>,
) -> Result<GeneratedOrdinaryIncomingCommitOriginalsV1> {
    selection
        .recheck_selected_originals_and_current_custody()
        .map_err(|e| e.to_string())?;
    selection
        .recheck_incoming_reservation(reserve)
        .map_err(|e| e.to_string())?;
    terminal_guard
        .recheck_terminal_selection(selection)
        .map_err(|e| e.to_string())?;
    let w2 = selection
        .preparation_selection()
        .map_err(|e| e.to_string())?;
    if !core::ptr::eq(verifier, w2.recursive_verifier()) {
        return reject();
    }
    let candidate = selection.candidate().map_err(|e| e.to_string())?;
    let guard = selection.preparation_guard().map_err(|e| e.to_string())?;
    let reservation = assemble_ordinary_incoming_reservation_v1(&w2, candidate, guard)?;
    let prior =
        KagemushaOrdinaryIncomingReservationProofBundleV1::decode_original(reservation.original())?;
    let body = *selection.terminal_body().map_err(|e| e.to_string())?;
    let approval_original = selection.original().map_err(|e| e.to_string())?.to_vec();
    let control_original = selection
        .financial_control_original()
        .map_err(|e| e.to_string())?;
    let capture = *selection
        .admission_clock_context()
        .map_err(|e| e.to_string())?;
    let state = candidate.successor_state();
    let commit = KagemushaOrdinaryIncomingCommitV1 {
        reservation: reservation.reservation().clone(),
        successor: KagemushaOrdinaryFinancialHeadV1 {
            state_commitment: state.state_commitment,
            logical_sequence: state.logical_sequence,
            state_original_sha256: sha(candidate.public_state_original()),
        },
        state_proof_bundle_original_sha256: reservation.original_sha256(),
        transition_statement_original_sha256: sha(
            &norito::encode_canonical(&prior.statement).map_err(|e| e.to_string())?
        ),
        purpose1_approval_original_sha256: sha(&approval_original),
        financial_control_original_sha256: sha(&control_original),
        admission_clock_context_original_sha256: sha(
            &norito::encode_canonical(&capture).map_err(|e| e.to_string())?
        ),
    };
    let mut accepted = None;
    let mut sources = 0;
    with_native_incoming_source(&w2, &mut |_source_originals, source| {
        sources += 1;
        if sources != 1 {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        source.recheck().map_err(native_error)?;
        let mut clocks_count = 0;
        selection.with_retained_verified_signed_clock_originals(&mut |clocks| {
            clocks_count += 1;
            if clocks_count != 1 {
                return Err(KagemushaStateErrorV1::InvalidCandidateStage);
            }
            if clocks[0].original() != prior.preparation_clock_signed_original {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            let bundle = KagemushaOrdinaryIncomingCommitProofBundleV1 {
                version: 1,
                commit: commit.clone(),
                reservation_proof_original: reservation.original().to_vec(),
                terminal_normalized: *selection.normalized_guard_statement()?,
                terminal_body: body,
                terminal_approval_original: approval_original.clone(),
                terminal_guard_original: terminal_guard.original().to_vec(),
                terminal_integrity_original: selection
                    .original_approval_integrity_lease()?
                    .map(|l| l.original().to_vec()),
                reserve_receipt_original: reserve.original().map_err(native_error)?.to_vec(),
                terminal_financial_control_original: control_original.clone(),
                terminal_intent_clock_signed_original: clocks[1].original().to_vec(),
                terminal_admission_clock_context: capture,
                terminal_admission_clock_signed_original: clocks[2].original().to_vec(),
            };
            require_terminal(
                verifier,
                &bundle,
                &prior,
                &reservation,
                w2.credential()?,
                selection.original_approval_integrity_lease()?,
                source.issuer_policy().map_err(native_error)?,
                [clocks[1], clocks[2]],
            )
            .map_err(native_error)?;
            if reserve.reservation().map_err(native_error)? != reservation.reservation()
                || reserve.request_original_sha256() != body.intent.reserve_request_original_sha256
                || reserve.original().map_err(native_error)?
                    != selection.reserve_receipt_original()?
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            accepted = Some(bundle);
            Ok(())
        })?;
        if clocks_count != 1 {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        source.recheck().map_err(native_error)?;
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    if sources != 1 {
        return reject();
    }
    let bundle = accepted.ok_or_else(rejection)?;
    let original = bundle.canonical_bytes()?;
    require_selected_successor(&commit, state, candidate.public_state_original(), &body)?;
    candidate
        .recheck_incoming_selection(&w2, guard)
        .map_err(|e| e.to_string())?;
    terminal_guard
        .recheck_terminal_selection(selection)
        .map_err(|e| e.to_string())?;
    selection
        .recheck_incoming_reservation(reserve)
        .map_err(|e| e.to_string())?;
    selection
        .recheck_selected_originals_and_current_custody()
        .map_err(|e| e.to_string())?;
    Ok(GeneratedOrdinaryIncomingCommitOriginalsV1 {
        proof: KagemushaVerifiedOrdinaryIncomingCommitProofV1 {
            commit,
            reservation,
            original: original.clone(),
        },
        selected_successor_state: state.clone(),
        successor_public_state_original: candidate.public_state_original().to_vec(),
        successor_private_checkpoint_original: candidate.private_checkpoint_original().to_vec(),
        successor_public_inputs: candidate.public_inputs().clone(),
        private_service_original: original,
    })
}
fn require_selected_successor(
    commit: &KagemushaOrdinaryIncomingCommitV1,
    state: &KagemushaStateV1,
    public_original: &[u8],
    body: &KagemushaOrdinaryIncomingTerminalBodyV1,
) -> Result<()> {
    state.validate().map_err(|e| e.to_string())?;
    if commit.successor.state_commitment != state.state_commitment
        || commit.successor.logical_sequence != state.logical_sequence
        || commit.successor.state_original_sha256 != sha(public_original)
        || body.intent.state_original_sha256 != sha(public_original)
        || body.intent.candidate_original_sha256 != sha(public_original)
        || body.intent.financial_sequence_after != state.logical_sequence
        || body.intent.financial_index_after != state.secure_index
    {
        return reject();
    }
    Ok(())
}

/// Durable verification-only constructor. Persisted data supplies no owner, selected State,
/// clock, FI/source or Reserve capability. Exact genuine original loans are reborrowed and all
/// both-parity State/W2/W1 current proofs and full histories reverified before owned custody.
pub(crate) fn readmit_ordinary_incoming_commit_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    selection: &KagemushaAuthenticatedOrdinaryIncomingTerminalApprovalSelectionV1<'_>,
    reserve: &KagemushaAuthenticatedOrdinaryIncomingReservationReceiptV1<'_>,
    persisted_commit: &KagemushaOrdinaryIncomingCommitV1,
    private_service_original: &[u8],
    successor_public_state_original: &[u8],
) -> Result<GeneratedOrdinaryIncomingCommitOriginalsV1> {
    let persisted =
        KagemushaOrdinaryIncomingCommitProofBundleV1::decode_original(private_service_original)?;
    if persisted.commit != *persisted_commit {
        return reject();
    }
    let terminal_guard =
        verify_ordinary_incoming_terminal_guard_v1(selection, &persisted.terminal_guard_original)
            .map_err(|e| e.to_string())?;
    let actual =
        assemble_ordinary_incoming_commit_v1(verifier, selection, &terminal_guard, reserve)?;
    if actual.commit() != persisted_commit
        || actual.private_service_original() != private_service_original
        || actual.successor_public_state_original() != successor_public_state_original
    {
        return reject();
    }
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_incoming_commit_capacity_retains_five_clock_originals_and_finalized_source() {
        // Actual supported protocol component maxima, not proof authority or a phone benchmark.
        assert_eq!(
            KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1,
            88_948_480
        );
        assert_eq!(ordinary_incoming_commit_carrier_max_bytes_v1(), 122_715_904);
        let clock_and_source = KAGEMUSHA_ORDINARY_FINALIZED_TOPUP_MAX_BYTES_V1
            + 5 * KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1;
        assert!(ordinary_incoming_commit_carrier_max_bytes_v1() > clock_and_source);
        let raw = ordinary_incoming_commit_carrier_max_bytes_v1();
        let base64 = raw
            .checked_add(2)
            .unwrap()
            .checked_div(3)
            .unwrap()
            .checked_mul(4)
            .unwrap();
        assert_eq!(base64, 163_621_208);
        assert!(u32::try_from(raw).is_ok());
        // Physical StateAdvance also retains public successor, Commit receipt and exact WAL
        // framing. The sole private carrier maximum must not stand in for that full byte count.
        assert!(raw.checked_add(32 * 1024).unwrap() > raw);
    }

    #[test]
    fn incoming_commit_decoder_rejects_empty_and_foreign_unbounded_data() {
        assert!(KagemushaOrdinaryIncomingCommitProofBundleV1::decode_original(&[]).is_err());
        assert!(
            KagemushaOrdinaryIncomingCommitProofBundleV1::decode_original(
                b"offered clock or state DTO"
            )
            .is_err()
        );
    }
}
