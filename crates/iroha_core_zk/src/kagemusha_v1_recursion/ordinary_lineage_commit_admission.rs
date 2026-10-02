//! Exact ordinary whole Commit mathematical admission, separate from current FI and DATA effects.
use super::*;
use crate::kagemusha_v1_recursion::ordinary_cash_terminal_verifier::{
    OrdinaryCashProofPairWireV1, OrdinaryCashTerminalMaterialV1, OrdinaryCashTerminalPublicV1,
    decode_stateless_original_v1, verify_stateless_public_v1,
};
use crate::kagemusha_v1_state::{
    KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1,
    KagemushaOrdinaryNativeSignedClockOriginalV1,
    KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1, KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
    KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1, KagemushaOrdinaryCashTerminalIntentV1,
    KagemushaOrdinaryCashTerminalRecordV1, KagemushaOrdinaryLineageCommitV1,
    KagemushaOrdinaryRetailEnrollmentCertificateV1, KagemushaSignedOrdinaryCurrentControlV1,
    kagemusha_ordinary_output_binding_digest_v1,
    kagemusha_ordinary_terminal_guard_commit_binding_digest_v1,
};
#[path = "ordinary_lineage_received_output_admission.rs"]
mod received_output_admission;
pub(crate) use received_output_admission::{
    KagemushaVerifiedOrdinaryReceivedCashOutputV1, verify_ordinary_received_cash_output_v1,
};

const COMPACT_META_MAX: usize = 256 * 1024;
const COMPACT_ORIGINAL_MAX: usize =
    KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1 + COMPACT_META_MAX;
const COMMIT_BUNDLE_MAX: usize =
    3 * KAGEMUSHA_ORDINARY_NATIVE_SIGNED_CLOCK_ORIGINAL_MAX_BYTES_V1 + 3 * 1024 * 1024;
/// Complete pre-receipt compact outgoing original. The later globally committed receipt remains
/// outside this frame, so a commit selector never hashes its own future signed result.
/// Data parsing does not verify a Wrapper, lend receiver keys or fund a Receive State.
#[derive(Clone, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::OrdinaryCashOutgoingOriginalV1",
    frame = "iroha.kagemusha.core.v1.ordinary-cash-outgoing-original"
)]
pub struct KagemushaOrdinaryCashOutgoingOriginalV1 {
    version: u16,
    normalized_preparation: KagemushaNormalizedGuardStatementV1,
    statement: Box<TransitionProofStatementV1>,
    outgoing: KagemushaOrdinaryLineageOutgoingOriginalsV1,
    prepared: KagemushaOrdinaryPreparedOutgoingV1,
    intent: KagemushaOrdinaryCashTerminalIntentV1,
    record: KagemushaOrdinaryCashTerminalRecordV1,
    credential_original: Vec<u8>,
    purpose1_approval_original: Vec<u8>,
    purpose1_integrity_original: Option<Vec<u8>>,
    financial_certificate_original: Vec<u8>,
    financial_control_original: Vec<u8>,
    admission_clock_signed_original: Vec<u8>,
    wrapper_original: Vec<u8>,
}
impl KagemushaOrdinaryCashOutgoingOriginalV1 {
    /// Assemble neutral complete data; a Native producer must lend all actual originals first.
    /// This constructor cannot create a proof result, current FI/clock or receiver capability.
    /// # Errors
    /// Refuses malformed originals, unsupported operation or a carrier outside its finite bounds.
    pub fn from_public_parts(
        normalized_preparation: KagemushaNormalizedGuardStatementV1,
        statement: TransitionProofStatementV1,
        outgoing: KagemushaOrdinaryLineageOutgoingOriginalsV1,
        prepared: KagemushaOrdinaryPreparedOutgoingV1,
        intent: KagemushaOrdinaryCashTerminalIntentV1,
        record: KagemushaOrdinaryCashTerminalRecordV1,
        credential_original: Vec<u8>,
        purpose1_approval_original: Vec<u8>,
        purpose1_integrity_original: Option<Vec<u8>>,
        financial_certificate_original: Vec<u8>,
        financial_control_original: Vec<u8>,
        admission_clock_signed_original: Vec<u8>,
        wrapper_original: Vec<u8>,
    ) -> Result<Self> {
        let value = Self {
            version: 1,
            normalized_preparation,
            statement: Box::new(statement),
            outgoing,
            prepared,
            intent,
            record,
            credential_original,
            purpose1_approval_original,
            purpose1_integrity_original,
            financial_certificate_original,
            financial_control_original,
            admission_clock_signed_original,
            wrapper_original,
        };
        value.validate_data()?;
        Ok(value)
    }
    fn validate_data(&self) -> Result<()> {
        if self.version != 1
            || !matches!(
                self.normalized_preparation.operation,
                KagemushaOperationV1::SendSplit | KagemushaOperationV1::RedeemSplit
            )
            || self.credential_original.is_empty()
            || self.credential_original.len() > KAGEMUSHA_ORDINARY_APP_ENROLLMENT_MAX_BYTES_V1
            || self.purpose1_approval_original.is_empty()
            || self.purpose1_approval_original.len() > KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1
            || self
                .purpose1_integrity_original
                .as_ref()
                .is_some_and(|v| v.is_empty() || v.len() > 4096)
            || self.financial_certificate_original.is_empty()
            || self.financial_certificate_original.len()
                > KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1
            || self.financial_control_original.is_empty()
            || self.financial_control_original.len()
                > KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1
            || self.wrapper_original.is_empty()
            || self.wrapper_original.len() > KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1
        {
            return reject();
        }
        self.normalized_preparation
            .canonical_digest()
            .map_err(|e| e.to_string())?;
        self.outgoing.validate_data()?;
        self.prepared.validate_shape()?;
        self.record
            .validate_against_originals(&self.intent, &self.prepared)?;
        KagemushaOrdinaryNativeSignedClockOriginalV1::decode_original(
            &self.admission_clock_signed_original,
        )
        .map_err(|e| e.to_string())?;
        self.approval()?;
        self.financial_control()?;
        Ok(())
    }
    /// Sole canonical pre-receipt public outgoing frame. It carries no plaintext or credit secrets.
    /// # Errors
    /// Refuses shape, codec or complete-frame capacity failure.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate_data()?;
        let raw = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if raw.len() > COMPACT_ORIGINAL_MAX {
            return reject();
        }
        Ok(raw)
    }
    /// Strict bounded data decoder, without current authority or proof admission.
    /// # Errors
    /// Refuses malformed, oversized, noncanonical or trailing data.
    pub fn decode_original(raw: &[u8]) -> Result<Self> {
        if raw.is_empty() || raw.len() > COMPACT_ORIGINAL_MAX {
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
    /// Exact issuer-authenticated C data for the service's independent historical issuer checks.
    pub fn credential_original(&self) -> &[u8] {
        &self.credential_original
    }
    /// Originally selected full W1 PI original; this never lends a refreshed/current lease.
    pub fn purpose1_integrity_original(&self) -> Option<&[u8]> {
        self.purpose1_integrity_original.as_deref()
    }
    /// Complete signed FI control data. The Core owner must independently authenticate issuer/policy.
    pub fn financial_control_original(&self) -> &[u8] {
        &self.financial_control_original
    }
    /// Same complete original FI certificate, never an imported Native financial secret.
    pub fn financial_certificate_original(&self) -> &[u8] {
        &self.financial_certificate_original
    }
    /// Full four-observation signed original, separate from its interval-context projection.
    pub fn admission_clock_signed_original(&self) -> &[u8] {
        &self.admission_clock_signed_original
    }
    /// Immutable admitted-record interval data; it grants no current time.
    pub fn admission_clock_context(&self) -> &KagemushaOrdinaryCashClockContextV1 {
        &self.record.admission_clock_context
    }
    /// Exact terminal record data opened by the actual whole proof; parsing alone is not admission.
    pub fn terminal_record(&self) -> &KagemushaOrdinaryCashTerminalRecordV1 {
        &self.record
    }
    /// Exact full purpose1 approval original, independently historical platform-authenticated.
    pub fn purpose1_approval_original(&self) -> &[u8] {
        &self.purpose1_approval_original
    }
    /// Same public full transition statement whose whole canonical SHA is opened by State.
    pub fn transition_statement(&self) -> &TransitionProofStatementV1 {
        &self.statement
    }
    /// Full public pre-candidate output data; no plaintext or credit secrets are included.
    pub fn outgoing_originals(&self) -> &KagemushaOrdinaryLineageOutgoingOriginalsV1 {
        &self.outgoing
    }
    /// Exact purpose2 normalized statement admitted by the independently verified State bundle.
    pub fn normalized_preparation(&self) -> &KagemushaNormalizedGuardStatementV1 {
        &self.normalized_preparation
    }
    /// Complete immutable prepared data, without a decoded-to-Native approval conversion.
    pub fn prepared(&self) -> &KagemushaOrdinaryPreparedOutgoingV1 {
        &self.prepared
    }
    /// Original purpose1 intent, before the one-call OS fence.
    pub fn terminal_intent(&self) -> &KagemushaOrdinaryCashTerminalIntentV1 {
        &self.intent
    }
    /// Complete actual Wrapper proof original; data access never constitutes proof admission.
    pub fn wrapper_original(&self) -> &[u8] {
        &self.wrapper_original
    }
    /// Decode complete W1 data for independent historical platform/policy checks.
    /// # Errors
    /// Refuses malformed or noncanonical approval data.
    pub fn approval(&self) -> Result<KagemushaAppOperationApprovalV1> {
        let w: KagemushaAppOperationApprovalV1 = decode_data(
            &self.purpose1_approval_original,
            KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1,
        )?;
        w.challenge.canonical_signing_bytes()?;
        Ok(w)
    }
    fn financial_control(&self) -> Result<KagemushaSignedOrdinaryCurrentControlV1> {
        let control: KagemushaSignedOrdinaryCurrentControlV1 = decode_data(
            &self.financial_control_original,
            KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
        )?;
        if control.canonical_bytes()? != self.financial_control_original {
            return reject();
        }
        Ok(control)
    }
}
/// Complete private Core Commit proof carrier. Neither private State/balance nor the Native
/// financial secret/sealed recovery streams enter this original. Its inner Terminal is retained
/// separately from the compact Wrapper so the service can verify every current proof/history.
#[derive(Clone, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::OrdinaryLineageCommitProofBundleV1",
    frame = "iroha.kagemusha.core.v1.ordinary-lineage-commit-proof-bundle"
)]
pub struct KagemushaOrdinaryLineageCommitProofBundleV1 {
    version: u16,
    reservation_bundle_original: Vec<u8>,
    predecessor_state_original: Vec<u8>,
    neutral_reservation_original: Vec<u8>,
    compact_outgoing_original: Vec<u8>,
    terminal_guard_original: Vec<u8>,
    inner_terminal_original: Vec<u8>,
    preparation_clock_signed_original: Vec<u8>,
    intent_clock_signed_original: Vec<u8>,
    wrapper_history_fold_originals: [Vec<u8>; 2],
}
impl KagemushaOrdinaryLineageCommitProofBundleV1 {
    /// Sole neutral assembly. Typed mathematical/Native ownership admission remains separate.
    /// # Errors
    /// Refuses invalid complete original shapes or finite source/frame bounds.
    pub fn from_public_parts(
        reservation_bundle_original: Vec<u8>,
        predecessor_state_original: Vec<u8>,
        neutral_reservation_original: Vec<u8>,
        compact_outgoing_original: Vec<u8>,
        terminal_guard_original: Vec<u8>,
        inner_terminal_original: Vec<u8>,
        preparation_clock_signed_original: Vec<u8>,
        intent_clock_signed_original: Vec<u8>,
        wrapper_history_fold_originals: [Vec<u8>; 2],
    ) -> Result<Self> {
        let value = Self {
            version: 1,
            reservation_bundle_original,
            predecessor_state_original,
            neutral_reservation_original,
            compact_outgoing_original,
            terminal_guard_original,
            inner_terminal_original,
            preparation_clock_signed_original,
            intent_clock_signed_original,
            wrapper_history_fold_originals,
        };
        value.validate_data()?;
        Ok(value)
    }
    fn validate_data(&self) -> Result<()> {
        if self
            .wrapper_history_fold_originals
            .iter()
            .any(|raw| raw.len() != super::super::super::KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1)
        {
            return reject();
        }
        if self.version != 1
            || self.terminal_guard_original.is_empty()
            || self.terminal_guard_original.len() > crate::kagemusha_v1_state::KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1
            || self.inner_terminal_original.is_empty()
            || self.inner_terminal_original.len() > crate::kagemusha_v1_recursion::ordinary_cash_terminal_verifier::ORDINARY_INNER_TERMINAL_MAX_BYTES_V1
            || self.neutral_reservation_original.is_empty()
            || self.neutral_reservation_original.len() > KAGEMUSHA_ORDINARY_CASH_RECORD_MAX_BYTES_V1
        {
            return reject();
        }
        KagemushaOrdinaryLineageStateProofBundleV1::decode_original(
            &self.reservation_bundle_original,
        )?;
        KagemushaOrdinaryLineageStateOriginalV1::decode_original(&self.predecessor_state_original)?;
        KagemushaOrdinaryCashOutgoingOriginalV1::decode_original(&self.compact_outgoing_original)?;
        for raw in [
            &self.preparation_clock_signed_original,
            &self.intent_clock_signed_original,
        ] {
            KagemushaOrdinaryNativeSignedClockOriginalV1::decode_original(raw)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    /// Sole finite complete private service frame; no genuine proof is admitted by encoding.
    /// # Errors
    /// Refuses codec/shape or full service-frame size failure.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate_data()?;
        let raw = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if raw.len() > COMMIT_BUNDLE_MAX {
            return reject();
        }
        Ok(raw)
    }
    /// Strict complete private service decoder, separate from the compact outgoing decoder.
    /// # Errors
    /// Refuses oversized, malformed, noncanonical or trailing data.
    pub fn decode_original(raw: &[u8]) -> Result<Self> {
        if raw.is_empty() || raw.len() > COMMIT_BUNDLE_MAX {
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
    /// Exact previously selected public State/Guard bundle, for real service Reserve admission.
    pub fn reservation_bundle_original(&self) -> &[u8] {
        &self.reservation_bundle_original
    }
    /// Full prior paired public State original; service rechecks its same held DATA head SHA.
    pub fn predecessor_state_original(&self) -> &[u8] {
        &self.predecessor_state_original
    }
    /// Real pre-W2 neutral reservation original, without a DATA reservation grant.
    pub fn neutral_reservation_original(&self) -> &[u8] {
        &self.neutral_reservation_original
    }
    /// Complete pre-receipt compact output, including full W1 FI/signed clock originals.
    /// # Errors
    /// Refuses invalid compact original data.
    pub fn outgoing(&self) -> Result<KagemushaOrdinaryCashOutgoingOriginalV1> {
        KagemushaOrdinaryCashOutgoingOriginalV1::decode_original(&self.compact_outgoing_original)
    }
    /// Exact complete pre-receipt compact frame whose SHA is bound by the Commit selector.
    pub fn outgoing_original(&self) -> &[u8] {
        &self.compact_outgoing_original
    }
    /// Full actual purpose1 Guard proof; every current proof/history is separately verified.
    pub fn terminal_guard_original(&self) -> &[u8] {
        &self.terminal_guard_original
    }
    /// Full actual inner Terminal proof pair, separate from the compact Wrapper.
    pub fn inner_terminal_original(&self) -> &[u8] {
        &self.inner_terminal_original
    }
    /// Exact actual Eq/Ep Wrapper history-fold transcripts, each fixed1280 bytes.
    pub fn wrapper_history_fold_originals(&self) -> [&[u8]; 2] {
        [
            &self.wrapper_history_fold_originals[0],
            &self.wrapper_history_fold_originals[1],
        ]
    }
    /// Complete signed preparation clock original; it must be independently root-authenticated.
    pub fn preparation_clock_signed_original(&self) -> &[u8] {
        &self.preparation_clock_signed_original
    }
    /// Complete signed intent clock original, before the distinct W1 platform call.
    pub fn intent_clock_signed_original(&self) -> &[u8] {
        &self.intent_clock_signed_original
    }
}
/// Closed genuine whole Commit mathematical proof. Current FI/clock, installed issuer purpose,
/// pending DATA reservation and durable global CAS are independently required before any effect.
pub struct KagemushaVerifiedOrdinaryLineageCommitProofV1 {
    commit: KagemushaOrdinaryLineageCommitV1,
    proof_bundle_original_sha256: DigestV1,
    terminal_guard_original_sha256: DigestV1,
}
impl KagemushaVerifiedOrdinaryLineageCommitProofV1 {
    /// Exact immutable complete selector admitted against State, both Guards, Terminal and Wrapper.
    pub fn commit(&self) -> &KagemushaOrdinaryLineageCommitV1 {
        &self.commit
    }
    /// Exact whole private service-frame SHA, after every current/history proof has passed.
    pub fn proof_bundle_original_sha256(&self) -> DigestV1 {
        self.proof_bundle_original_sha256
    }
    /// Exact whole W1 Guard original, separate from its platform approval proof transcript.
    pub fn terminal_guard_original_sha256(&self) -> DigestV1 {
        self.terminal_guard_original_sha256
    }
}
/// Verify the exact whole ordinary Commit against a previously genuine closed Reserve proof.
/// Full signed observations are independently root-authenticated typed data; their projection
/// creates no current elapsed clock. The service additionally requires its freshly held FI/clock
/// and the actual exclusively pending DATA predecessor before signing any committed result.
/// # Errors
/// Refuses original/purpose/scope/output/clock/financial joins, any forged current proof or history.
pub fn verify_ordinary_lineage_commit_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    commit: &KagemushaOrdinaryLineageCommitV1,
    bundle_original: &[u8],
    reservation: &KagemushaVerifiedOrdinaryLineageReservationProofV1,
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    terminal_lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    clocks: [&KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1; 3],
) -> Result<KagemushaVerifiedOrdinaryLineageCommitProofV1> {
    commit.validate_shape()?;
    let bundle = KagemushaOrdinaryLineageCommitProofBundleV1::decode_original(bundle_original)?;
    let preparation = KagemushaOrdinaryLineageStateProofBundleV1::decode_original(
        &bundle.reservation_bundle_original,
    )?;
    let outgoing = bundle.outgoing()?;
    let state = reservation.state_proof();
    let n = state.normalized_statement();
    let release = verifier.monetary_release()?;
    let guard_material = verifier.ordinary_guard_verifier_material();
    let terminal_material = verifier.ordinary_cash_terminal_verifier_material()?;
    if &commit.reservation != reservation.reservation()
        || commit.reservation.proof_bundle_original_sha256
            != <DigestV1>::from(Sha256::digest(&bundle.reservation_bundle_original))
        || commit
            .reservation
            .selection
            .predecessor
            .state_original_sha256
            != <DigestV1>::from(Sha256::digest(&bundle.predecessor_state_original))
        || commit
            .reservation
            .selection
            .neutral_reservation_original_sha256
            != <DigestV1>::from(Sha256::digest(&bundle.neutral_reservation_original))
        || outgoing.normalized_preparation != *n
        || preparation.normalized != *n
        || outgoing.credential_original != credential.original()
        || preparation.credential_original != credential.original()
        || outgoing.purpose1_integrity_original.as_deref() != terminal_lease.map(|p| p.original())
        || outgoing.prepared != *preparation.prepared.as_ref().ok_or_else(rejection)?
        || outgoing.statement.as_ref()
            != match &preparation.statement {
                KagemushaOrdinaryLineageStatementOriginalV1::Outgoing(s) => s.as_ref(),
                _ => return reject(),
            }
        || Some(&outgoing.outgoing) != preparation.outgoing_originals.as_ref()
        || n.release_id != release.release_id()
        || n.successor_suite_id != terminal_material.suite_id
        || n.successor_vk_digest != terminal_material.vk_set_digest
    {
        return reject();
    }
    require_outgoing_originals(&preparation, &commit.reservation, verifier)?;
    let prepared = &outgoing.prepared;
    let intent = &outgoing.intent;
    let record = &outgoing.record;
    let body = &record.body;
    record.validate_against_originals(intent, prepared)?;
    let w2 = preparation.approval()?;
    let w1 = outgoing.approval()?;
    let body_digest = body.binding_digest()?;
    let intent_digest = intent.binding_digest()?;
    let record_digest = record.binding_digest()?;
    let authorization = kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
        kagemusha_ordinary_app_approval_proof_binding_digest_v1(&w1)?,
        terminal_lease.map(|p| Sha256::digest(p.original()).into()),
    )?;
    let mut normalized = *n;
    normalized.terminal_commit_binding_digest =
        kagemusha_ordinary_terminal_guard_commit_binding_digest_v1(
            body_digest,
            state.candidate_digest(),
            state.state_statement_digest(),
            prepared.reservation_digest,
        )?;
    normalized.sender_one_time_authorization_digest = if prepared.operation == 2 {
        state.authorization_digest()
    } else {
        [0; 32]
    };
    normalized.transition_intent_digest = body_digest;
    normalized.recovery_record_digest = intent_digest;
    let guard_digest = normalized.canonical_digest().map_err(|e| e.to_string())?;
    let subject_digest: DigestV1 =
        Sha256::digest(w1.challenge.canonical_subject_signing_bytes()?).into();
    let mut subject = w2.challenge.subject;
    subject.candidate_envelope_digest = state.candidate_digest();
    subject.terminal_body_commitment = body_digest;
    let c = credential.subject();
    if w1.challenge.purpose != KagemushaAppOperationApprovalPurposeV1::MonetaryTransition
        || w1.challenge.subject != subject
        || w1.challenge.subject_signing_digest != subject_digest
        || w1.challenge.normalized_guard_digest != guard_digest
        || w1.challenge.account_binding != c.account_binding
        || w1.challenge.authority_policy_digest != c.app_authority_policy_digest
        || w1.challenge.attested_key_id != c.attested_key_id
        || w1.challenge.enrollment_digest != credential.digest()
        || w1.challenge.operation_id == w2.challenge.operation_id
        || w1.challenge.nonce == w2.challenge.nonce
        || body.operation != operation_tag(n.operation)?
        || body.amount != n.amount
        || body.state_statement_digest != state.state_statement_digest()
        || body.candidate_digest != state.candidate_digest()
        || body.preparation_id != prepared.binding_digest()?
        || body.prepared_projection_semantic_digest != prepared.projection_semantic_digest
        || body.secure_index_before != subject.secure_index_before
        || body.secure_index_after != subject.secure_index_after
        || u128::from(body.logical_journal_sequence_before) != n.journal_revision_before
        || u128::from(body.logical_journal_sequence_after) != n.journal_revision_after
        || record.sender_credential_digest != credential.digest()
        || record.preparation_authorization_digest != state.authorization_digest()
        || record.terminal_authorization_digest != authorization
        || record.terminal_subject_digest != subject_digest
        || intent.native_operation_id != w1.challenge.operation_id
        || intent.native_nonce != w1.challenge.nonce
        || record.approval_issued_at_ms != w1.challenge.issued_at_ms
        || record.approval_expires_at_ms != w1.challenge.expires_at_ms
    {
        return reject();
    }
    let digests = [
        guard_digest,
        credential.digest(),
        authorization,
        subject_digest,
        release.provider_policy_root(),
    ];
    ordinary_guard_verifier::verify_stateless_original_v1(
        &bundle.terminal_guard_original,
        &guard_material,
        digests,
    )
    .map_err(|e| e.to_string())?;
    let preparation_clock = match &outgoing.outgoing {
        KagemushaOrdinaryLineageOutgoingOriginalsV1::Send {
            preparation_clock, ..
        }
        | KagemushaOrdinaryLineageOutgoingOriginalsV1::Redeem {
            preparation_clock, ..
        } => preparation_clock,
    };
    for (clock, raw, context) in [
        (
            clocks[0],
            bundle.preparation_clock_signed_original.as_slice(),
            preparation_clock,
        ),
        (
            clocks[1],
            bundle.intent_clock_signed_original.as_slice(),
            &body.clock_context,
        ),
        (
            clocks[2],
            outgoing.admission_clock_signed_original.as_slice(),
            &record.admission_clock_context,
        ),
    ] {
        if clock.original() != raw {
            return reject();
        }
        clock
            .recheck_cash_context(context)
            .map_err(|e| e.to_string())?;
    }
    let control = outgoing.financial_control()?;
    let certificate: KagemushaOrdinaryRetailEnrollmentCertificateV1 = decode_data(
        &outgoing.financial_certificate_original,
        KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
    )?;
    if certificate.canonical_bytes().map_err(|e| e.to_string())?
        != outgoing.financial_certificate_original
        || certificate.subject.owner != commit.reservation.selection.lineage.owner
        || certificate.subject.ordinary_app_credential_digest != credential.digest()
        || certificate.subject.issuance.credential.canonical_bytes()? != credential.original()
        || certificate.subject.issuance.release_id != n.release_id
        || certificate.subject.issuance.hardware_policy_digest != release.hardware_policy_digest()
        || control.subject.request.owner != commit.reservation.selection.lineage.owner
        || control.subject.request.enrollment_original_sha256
            != <DigestV1>::from(Sha256::digest(&outgoing.financial_certificate_original))
        || control.subject.request.credential_original_sha256
            != <DigestV1>::from(Sha256::digest(credential.original()))
        || control.subject.release_id != n.release_id
        || control.subject.hardware_profile_id != n.hardware_profile_id
        || control.subject.profile_policy_epoch != n.policy_epoch
        || control.subject.ordinary_trust_policy_digest != c.trust_policy_digest
        || control.subject.app_authority_policy_digest != c.app_authority_policy_digest
        || control.subject.latest_integrity_lease_original.as_deref()
            != terminal_lease.map(|p| p.original())
        || record.admission_clock_context.lower_at_ms < control.subject.issued_at_ms
        || record.admission_clock_context.upper_at_ms >= control.subject.expires_at_ms
        || record.admission_clock_context.lower_at_ms < body.clock_context.lower_at_ms
        || record.admission_clock_context.upper_at_ms < body.clock_context.upper_at_ms
    {
        return reject();
    }
    let nullifier = kagemusha_ordinary_transition_nullifier_v1(
        n.predecessor_state_commitment,
        subject.secure_index_before,
        state.financial_epoch_id(),
        n.network_id,
        n.lane_id,
        n.liability_pool_id,
    )?;
    let output = match &outgoing.outgoing {
        KagemushaOrdinaryLineageOutgoingOriginalsV1::Send {
            request,
            output,
            encrypted_credit,
            ..
        } => {
            if body.request_digest != request.canonical_original_digest()?
                || body.recipient_credential_digest != request.body.recipient_credential_digest
                || body.send_output_digest != output.binding_digest()?
                || body.encrypted_credit_digest != kagemusha_ciphertext_digest_v1(encrypted_credit)
                || body.artifact_manifest_digest != [0; 32]
            {
                return reject();
            }
            output.ciphertext_commitment
        }
        KagemushaOrdinaryLineageOutgoingOriginalsV1::Redeem { output, .. } => {
            if body.request_digest != [0; 32]
                || body.recipient_credential_digest != [0; 32]
                || body.send_output_digest != [0; 32]
                || body.encrypted_credit_digest != [0; 32]
                || body.artifact_manifest_digest != output.artifact_manifest_digest
                || body.artifact_manifest_digest != release.manifest_digest()
            {
                return reject();
            }
            [0; 32]
        }
    };
    let inner =
        decode_stateless_original_v1(&bundle.inner_terminal_original, 1, &terminal_material)
            .map_err(|e| e.to_string())?;
    let wrapper = decode_stateless_original_v1(&outgoing.wrapper_original, 2, &terminal_material)
        .map_err(|e| e.to_string())?;
    let inner_public = terminal_public(
        &terminal_material,
        n,
        body_digest,
        state.candidate_digest(),
        record_digest,
        nullifier,
        body,
        output,
        &inner,
    )?;
    let wrapper_public = terminal_public(
        &terminal_material,
        n,
        body_digest,
        state.candidate_digest(),
        record_digest,
        nullifier,
        body,
        output,
        &wrapper,
    )?;
    verify_stateless_public_v1(&terminal_material, &inner_public, &inner)
        .map_err(|e| e.to_string())?;
    verify_stateless_public_v1(&terminal_material, &wrapper_public, &wrapper)
        .map_err(|e| e.to_string())?;
    super::super::super::ordinary_cash_commit_wrapper::require_exact_ordinary_cash_wrapper_inner_v1(
        &terminal_material, &inner_public, &wrapper_public, &inner, &wrapper,
        bundle.wrapper_history_fold_originals(),
    )?;
    let clock_context_original =
        norito::encode_canonical(&record.admission_clock_context).map_err(|e| e.to_string())?;
    if commit.transition_nullifier != nullifier
        || commit.purpose1_approval_original_sha256
            != <DigestV1>::from(Sha256::digest(&outgoing.purpose1_approval_original))
        || commit.terminal_record_original_sha256
            != <DigestV1>::from(Sha256::digest(record.canonical_bytes()?))
        || commit.terminal_proofs_original_sha256
            != <DigestV1>::from(Sha256::digest(&bundle.inner_terminal_original))
        || commit.wrapper_proofs_original_sha256
            != <DigestV1>::from(Sha256::digest(&outgoing.wrapper_original))
        || commit.outgoing_original_sha256
            != <DigestV1>::from(Sha256::digest(&bundle.compact_outgoing_original))
        || commit.purpose1_financial_control_original_sha256
            != <DigestV1>::from(Sha256::digest(&outgoing.financial_control_original))
        || commit.purpose1_clock_context_original_sha256
            != <DigestV1>::from(Sha256::digest(clock_context_original))
    {
        return reject();
    }
    Ok(KagemushaVerifiedOrdinaryLineageCommitProofV1 {
        commit: commit.clone(),
        proof_bundle_original_sha256: Sha256::digest(bundle_original).into(),
        terminal_guard_original_sha256: Sha256::digest(&bundle.terminal_guard_original).into(),
    })
}
fn terminal_public(
    m: &OrdinaryCashTerminalMaterialV1<'_>,
    n: &KagemushaNormalizedGuardStatementV1,
    body_digest: DigestV1,
    candidate: DigestV1,
    record: DigestV1,
    nullifier: DigestV1,
    body: &iroha_data_model::kagemusha::KagemushaOrdinaryCashTerminalBodyV1,
    ciphertext: DigestV1,
    wire: &OrdinaryCashProofPairWireV1,
) -> Result<OrdinaryCashTerminalPublicV1> {
    let value = OrdinaryCashTerminalPublicV1 {
        operation: body.operation,
        suite_id: m.suite_id,
        vk_set_digest: m.vk_set_digest,
        release_id: m.release_id,
        network_id: n.network_id,
        asset_id: n.asset_id,
        asset_incarnation: *n.asset_incarnation.as_bytes(),
        asset_scale: n.asset_scale,
        liability_pool_id: n.liability_pool_id,
        app_credential_profile_id: n.hardware_profile_id,
        policy_epoch: n.policy_epoch,
        lifecycle_digest: n.lifecycle_binding_digest,
        body_digest,
        candidate_digest: candidate,
        terminal_record_digest: record,
        transition_nullifier: nullifier,
        request_digest: body.request_digest,
        receiver_credential_digest: body.recipient_credential_digest,
        ciphertext_commitment: ciphertext,
        amount: body.amount,
        output_binding_digest: kagemusha_ordinary_output_binding_digest_v1(
            body.prepared_projection_semantic_digest,
            candidate,
            record,
        )?,
        redemption_manifest_digest: body.artifact_manifest_digest,
        eq_deferred_audit: wire.eq_deferred_audit,
        ep_deferred_audit: wire.ep_deferred_audit,
        eq_protocol_digest: wire.eq_protocol_digest,
        ep_protocol_digest: wire.ep_protocol_digest,
    };
    value.validate()?;
    Ok(value)
}
fn decode_data<T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize>(
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

#[cfg(test)]
#[path = "ordinary_lineage_commit_admission_tests.rs"]
mod tests;
