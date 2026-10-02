//! Closed ordinary receiver admission from a genuine compact Wrapper and installed DATA assertion.
//! The retained request key remains in Native custody. This result grants no funding, current
//! sender time, local replay exemption or receiver State advance.
use super::*;
use crate::kagemusha_v1_state::{
    KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1,
    KagemushaAuthenticatedOrdinaryReceiverRequestCustodyV1,
    KagemushaHistoricalOrdinaryReceiverRequestCustodyV1,
};
use iroha_data_model::kagemusha::{
    KagemushaHardwarePlatformClassV1, KagemushaOrdinaryAppCredentialV1,
    KagemushaOrdinaryPaymentRequestV1,
};

/// Exact proof-admitted receiver output. Its constructor requires actual Native request custody,
/// an independently installed immutable DATA assertion and a verified full clock original.
/// It does not expose the request private key, plaintext or a monetary effect capability.
pub(crate) struct KagemushaVerifiedOrdinaryReceivedCashOutputV1 {
    request: Box<KagemushaOrdinaryPaymentRequestV1>,
    output: KagemushaOrdinaryPaymentOutputV1,
    encrypted_credit: Vec<u8>,
    preparation_clock: KagemushaOrdinaryCashClockContextV1,
    outgoing_original: Vec<u8>,
    request_original: Vec<u8>,
    commit: KagemushaOrdinaryLineageCommitV1,
    received_assertion_original_sha256: DigestV1,
}
impl KagemushaVerifiedOrdinaryReceivedCashOutputV1 {
    /// Exact pre-receipt frame whose SHA is asserted by the actual committed DATA record.
    pub(crate) fn outgoing_original(&self) -> &[u8] {
        &self.outgoing_original
    }
    /// Exact originally retained signed receiver request, including platform evidence.
    pub(crate) fn request_original(&self) -> &[u8] {
        &self.request_original
    }
    /// Same proof-bound request; the Native custody loan owns its private key separately.
    pub(crate) fn request(&self) -> &KagemushaOrdinaryPaymentRequestV1 {
        &self.request
    }
    /// Complete same384-byte encrypted envelope, never plaintext or an imported key.
    pub(crate) fn encrypted_credit(&self) -> &[u8] {
        &self.encrypted_credit
    }
    /// Same output opened by the genuine recursively verified Wrapper.
    pub(crate) fn output(&self) -> &KagemushaOrdinaryPaymentOutputV1 {
        &self.output
    }
    /// Exact preparation clock opened by that output and used by the sole Model AAD formula.
    pub(crate) fn preparation_clock(&self) -> &KagemushaOrdinaryCashClockContextV1 {
        &self.preparation_clock
    }
    /// Exact immutable globally committed selector, without converting it into sender custody.
    pub(crate) fn commit(&self) -> &KagemushaOrdinaryLineageCommitV1 {
        &self.commit
    }
    /// SHA256 of the complete canonical received envelope (signature, DATA row and finality).
    /// This deliberately differs from SHA256 of only the signed-result sub-original.
    pub(crate) fn received_assertion_original_sha256(&self) -> DigestV1 {
        self.received_assertion_original_sha256
    }
    /// Exact same enrolled receiver C original digest, independent of sender C.
    pub(crate) fn receiver_credential_digest(&self) -> DigestV1 {
        self.request().body.recipient_credential_digest
    }
    /// Proof-verified amount. Actual inbox/State effects remain separately gated.
    pub(crate) fn amount(&self) -> u128 {
        self.output().amount
    }
    /// Exact request-bound credit identity selected by the sender State proof.
    pub(crate) fn credit_id(&self) -> DigestV1 {
        self.output().credit_id
    }
}

/// Admit a Send output against actual retained receiver request custody, a genuine independently
/// installed historical Commit assertion and the exact authenticated admission clock original.
/// The Wrapper recursively verifies the whole inner Terminal, State and both Guard histories;
/// the full Core service separately admits the exact submitted inner original/folds. This
/// compact admission does not claim to carry those absent inner originals or renew sender time.
pub(crate) fn verify_ordinary_received_cash_output_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    assertion: &KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1<'_>,
    receiver: &KagemushaAuthenticatedOrdinaryReceiverRequestCustodyV1<'_>,
    outgoing_original: &[u8],
    admission_clock: &KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
) -> Result<KagemushaVerifiedOrdinaryReceivedCashOutputV1> {
    verify_received_cash_output_with_custody(
        verifier,
        assertion,
        &ReceiverProofCustody::Current(receiver),
        outgoing_original,
        admission_clock,
    )
}

/// Re-admit immutable received proof operands from the same actual historical Main request.
/// This admits cryptography only; it cannot create a current FI loan or incoming State effect.
pub(crate) fn readmit_historical_ordinary_received_cash_output_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    assertion: &KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1<'_>,
    receiver: &KagemushaHistoricalOrdinaryReceiverRequestCustodyV1<'_>,
    outgoing_original: &[u8],
    admission_clock: &KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
) -> Result<KagemushaVerifiedOrdinaryReceivedCashOutputV1> {
    verify_received_cash_output_with_custody(
        verifier,
        assertion,
        &ReceiverProofCustody::Historical(receiver),
        outgoing_original,
        admission_clock,
    )
}

// Both variants contain genuine Native loans, never decoded data or a caller verifier hook.
enum ReceiverProofCustody<'loan, 'owner> {
    Current(&'loan KagemushaAuthenticatedOrdinaryReceiverRequestCustodyV1<'owner>),
    Historical(&'loan KagemushaHistoricalOrdinaryReceiverRequestCustodyV1<'owner>),
}
impl ReceiverProofCustody<'_, '_> {
    fn recheck_proof_custody(
        &self,
    ) -> std::result::Result<(), crate::kagemusha_v1_state::KagemushaStateErrorV1> {
        match self {
            Self::Current(c) => c.recheck_current_custody(),
            Self::Historical(c) => c.recheck_historical_custody(),
        }
    }
    fn request_original(
        &self,
    ) -> std::result::Result<&[u8], crate::kagemusha_v1_state::KagemushaStateErrorV1> {
        match self {
            Self::Current(c) => c.request_original(),
            Self::Historical(c) => c.request_original(),
        }
    }
    fn enrollment(
        &self,
    ) -> std::result::Result<
        &iroha_data_model::kagemusha::KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
        crate::kagemusha_v1_state::KagemushaStateErrorV1,
    > {
        match self {
            Self::Current(c) => c.enrollment(),
            Self::Historical(c) => c.enrollment(),
        }
    }
    fn previous_app_attest_counter(
        &self,
    ) -> std::result::Result<Option<u32>, crate::kagemusha_v1_state::KagemushaStateErrorV1> {
        match self {
            Self::Current(c) => c.previous_app_attest_counter(),
            Self::Historical(c) => c.previous_app_attest_counter(),
        }
    }
    fn financial_owner(
        &self,
    ) -> std::result::Result<
        &crate::kagemusha_v1_state::KagemushaOrdinaryEnrolledFinancialOwnerV1,
        crate::kagemusha_v1_state::KagemushaStateErrorV1,
    > {
        match self {
            Self::Current(c) => c.financial_owner(),
            Self::Historical(c) => c.financial_owner(),
        }
    }
}

fn verify_received_cash_output_with_custody(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    assertion: &KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1<'_>,
    receiver: &ReceiverProofCustody<'_, '_>,
    outgoing_original: &[u8],
    admission_clock: &KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
) -> Result<KagemushaVerifiedOrdinaryReceivedCashOutputV1> {
    receiver
        .recheck_proof_custody()
        .map_err(|e| e.to_string())?;
    let financial = receiver.financial_owner().map_err(|e| e.to_string())?;
    assertion
        .recheck_historical(financial)
        .map_err(|e| e.to_string())?;
    let commit = assertion.commit().map_err(|e| e.to_string())?;
    commit.validate_shape()?;
    let outgoing = KagemushaOrdinaryCashOutgoingOriginalV1::decode_original(outgoing_original)?;
    require_commit_original_selectors(commit, outgoing_original, &outgoing)?;
    let KagemushaOrdinaryLineageOutgoingOriginalsV1::Send {
        request,
        output,
        encrypted_credit,
        preparation_clock,
    } = &outgoing.outgoing
    else {
        return reject();
    };
    let request_original = request.canonical_bytes()?;
    if request_original != receiver.request_original().map_err(|e| e.to_string())? {
        return reject();
    }
    let enrolled = receiver.enrollment().map_err(|e| e.to_string())?;
    let receiver_credential = enrolled.app_credential();
    request.authenticate_receiver_signature(
        receiver_credential,
        receiver
            .previous_app_attest_counter()
            .map_err(|e| e.to_string())?,
    )?;
    let n = &outgoing.normalized_preparation;
    let selected = &commit.reservation.selection;
    let sender = &selected.lineage;
    let release = verifier.monetary_release()?;
    let material = verifier.ordinary_cash_terminal_verifier_material()?;
    let credential =
        KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(&outgoing.credential_original)?;
    let c = &credential.subject;
    let sender_credential_digest = credential.canonical_digest()?;
    let financial_epoch = kagemusha_ordinary_financial_epoch_id_v1(c)?;
    let w1 = outgoing.approval()?;
    let subject = &w1.challenge.subject;
    let intent = &outgoing.intent;
    let record = &outgoing.record;
    let body = &record.body;
    let prepared = &outgoing.prepared;
    record.validate_against_originals(intent, prepared)?;
    let body_digest = body.binding_digest()?;
    let record_digest = record.binding_digest()?;
    let request_digest = request.canonical_original_digest()?;
    let nullifier = kagemusha_ordinary_transition_nullifier_v1(
        n.predecessor_state_commitment,
        subject.secure_index_before,
        financial_epoch,
        n.network_id,
        n.lane_id,
        n.liability_pool_id,
    )?;
    let receiver_owner = &enrolled.certificate().subject.owner;
    if n.operation != KagemushaOperationV1::SendSplit
        || selected.operation != KagemushaOperationKindV1::SendSplit
        || body.operation != 2
        || n.release_id != release.release_id()
        || c.release_id != n.release_id
        || c.suite_id != material.suite_id
        || n.successor_suite_id != material.suite_id
        || n.successor_vk_digest != material.vk_set_digest
        || c.hardware_profile_id != n.hardware_profile_id
        || c.policy_epoch != n.policy_epoch
        || c.network_id != n.network_id
        || c.lane_id != n.lane_id
        || sender.financial_epoch_id != financial_epoch
        || sender.financial_authority_commitment != c.financial_authority_commitment
        || sender.owner.lane_id != c.lane_id
        || sender.owner.runtime.network_id.as_bytes() != &n.network_id
        || sender.owner.runtime.asset_incarnation != n.asset_incarnation
        || sender.owner.runtime.scale != n.asset_scale
        || kagemusha_asset_identity_digest_v1(&sender.owner.runtime.asset)
            .map_err(|e| e.to_string())?
            != n.asset_id
        || kagemusha_ordinary_app_account_binding_v1(&sender.owner.account_id) != c.account_binding
        || selected.predecessor.state_commitment != n.predecessor_state_commitment
        || selected.predecessor.logical_sequence != n.predecessor_logical_sequence
        || commit.reservation.successor.state_commitment != n.successor_state_commitment
        || commit.reservation.successor.logical_sequence != n.successor_logical_sequence
        || selected.amount != n.amount
        || selected.scale != n.asset_scale
        || n.amount != output.amount
        || request.body.amount != n.amount
        || body.amount != n.amount
        || request.body.release_id != n.release_id
        || request.body.network_id != n.network_id
        || request.body.normalized_asset_id != n.asset_id
        || request.body.asset_incarnation != *n.asset_incarnation.as_bytes()
        || request.body.scale != n.asset_scale
        || request.body.reserve_pool_id != n.liability_pool_id
        || receiver_owner.runtime.network_id.as_bytes() != &n.network_id
        || receiver_owner.runtime.asset_incarnation != n.asset_incarnation
        || receiver_owner.runtime.scale != n.asset_scale
        || kagemusha_asset_identity_digest_v1(&receiver_owner.runtime.asset)
            .map_err(|e| e.to_string())?
            != n.asset_id
        || selected.receiver_request_original_sha256 != request_digest
        || selected.output_body_original_sha256
            != <DigestV1>::from(Sha256::digest(output.canonical_bytes()?))
        || selected.neutral_reservation_digest != prepared.reservation_digest
        || output.sender_before_commitment != n.predecessor_state_commitment
        || output.sender_after_commitment != n.successor_state_commitment
        || output.transition_nullifier != nullifier
        || commit.transition_nullifier != nullifier
        || output.credit_id != n.peer_credit_id
        || request.body.recipient_encryption_key != n.recipient_encryption_key_binding
        || request.body.recipient_credential_digest != receiver_credential.digest()
        || body.request_digest != request_digest
        || output.request_digest != request_digest
        || prepared.request_digest != request_digest
        || body.recipient_credential_digest != receiver_credential.digest()
        || body.send_output_digest != output.binding_digest()?
        || body.encrypted_credit_digest != kagemusha_ciphertext_digest_v1(encrypted_credit)
        || output.encrypted_credit_digest != body.encrypted_credit_digest
        || body.artifact_manifest_digest != [0; 32]
        || body.prepared_projection_semantic_digest != prepared.projection_semantic_digest
        || prepared.projection_semantic_digest
            != kagemusha_ordinary_payment_body_digest_v1(
                output.binding_digest()?,
                body.encrypted_credit_digest,
            )?
        || preparation_clock.lower_at_ms < request.body.issued_at_ms
        || preparation_clock.upper_at_ms >= request.body.expires_at_ms
        || body.candidate_digest == [0; 32]
        || subject.transition_statement_digest
            != outgoing.statement.digest().map_err(|e| e.to_string())?
        || body.state_statement_digest != subject.transition_statement_digest
        || body.preparation_id != prepared.binding_digest()?
        || body.secure_index_before != subject.secure_index_before
        || body.secure_index_after != subject.secure_index_after
        || u128::from(body.logical_journal_sequence_before) != n.journal_revision_before
        || u128::from(body.logical_journal_sequence_after) != n.journal_revision_after
    {
        return reject();
    }
    output.encrypted_credit_aad_against(request, preparation_clock)?;
    require_complete_statement(&outgoing, verifier)?;
    let mut terminal_normalized = *n;
    terminal_normalized.terminal_commit_binding_digest =
        kagemusha_ordinary_terminal_guard_commit_binding_digest_v1(
            body_digest,
            body.candidate_digest,
            body.state_statement_digest,
            prepared.reservation_digest,
        )?;
    terminal_normalized.sender_one_time_authorization_digest =
        record.preparation_authorization_digest;
    terminal_normalized.transition_intent_digest = body_digest;
    terminal_normalized.recovery_record_digest = intent.binding_digest()?;
    let normalized_digest = terminal_normalized
        .canonical_digest()
        .map_err(|e| e.to_string())?;
    let subject_digest: DigestV1 =
        Sha256::digest(w1.challenge.canonical_subject_signing_bytes()?).into();
    let authorization = kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
        kagemusha_ordinary_app_approval_proof_binding_digest_v1(&w1)?,
        outgoing
            .purpose1_integrity_original
            .as_ref()
            .map(|p| Sha256::digest(p).into()),
    )?;
    if w1.challenge.purpose != KagemushaAppOperationApprovalPurposeV1::MonetaryTransition
        || w1.challenge.account_binding != c.account_binding
        || w1.challenge.enrollment_digest != sender_credential_digest
        || w1.challenge.attested_key_id != c.attested_key_id
        || w1.challenge.authority_policy_digest != c.app_authority_policy_digest
        || w1.challenge.normalized_guard_digest != normalized_digest
        || w1.challenge.subject_signing_digest != subject_digest
        || subject.operation_kind != KagemushaOperationKindV1::SendSplit
        || subject.release_id != n.release_id
        || subject.provider_policy_root != release.provider_policy_root()
        || subject.app_policy_digest != n.successor_hardware_policy_id
        || subject.credential_id != sender_credential_digest
        || subject.network_id.as_bytes() != &n.network_id
        || subject.lane_commitment != n.lane_id
        || subject.hardware_profile_id != n.hardware_profile_id
        || subject.policy_epoch != n.policy_epoch
        || subject.hardware_epoch_id != financial_epoch
        || subject.hardware_epoch_generation != c.hardware_epoch
        || subject.candidate_envelope_digest != body.candidate_digest
        || subject.terminal_body_commitment != body_digest
        || intent.native_operation_id != w1.challenge.operation_id
        || intent.native_nonce != w1.challenge.nonce
        || record.sender_credential_digest != sender_credential_digest
        || record.terminal_subject_digest != subject_digest
        || record.terminal_authorization_digest != authorization
        || record.approval_issued_at_ms != w1.challenge.issued_at_ms
        || record.approval_expires_at_ms != w1.challenge.expires_at_ms
    {
        return reject();
    }
    let original_counter_floor = match c.platform_class {
        KagemushaHardwarePlatformClassV1::AndroidKeyMint => None,
        KagemushaHardwarePlatformClassV1::AppleAppAttest => Some(c.app_attest_counter_floor),
        _ => return reject(),
    };
    // This rechecks the genuine original equation. The complete Wrapper/Core admission, not
    // this enrollment floor, supplies the selected operation's historical replay-floor join.
    w1.evidence.authenticate_signature(
        c.platform_class,
        &c.app_public_key,
        c.app_signing_identity_digest,
        c.app_release_digest,
        original_counter_floor,
        &w1.challenge.canonical_signing_bytes()?,
    )?;
    admission_clock
        .recheck_cash_context(&record.admission_clock_context)
        .map_err(|e| e.to_string())?;
    if admission_clock.original() != outgoing.admission_clock_signed_original {
        return reject();
    }
    let policy = assertion.issuer_policy().map_err(|e| e.to_string())?;
    let certificate: KagemushaOrdinaryRetailEnrollmentCertificateV1 = decode_data(
        &outgoing.financial_certificate_original,
        KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
    )?;
    let control = outgoing.financial_control()?;
    certificate
        .signature
        .verify(
            &policy.issuer_public_key,
            &certificate.subject.approval_payload()?,
        )
        .map_err(|e| e.to_string())?;
    control.verify_for_request(&control.subject.request, policy)?;
    if certificate.subject.owner != sender.owner
        || certificate.subject.issuer_policy_id != policy.issuer_policy_id
        || certificate.subject.issuer_audience != policy.issuer_audience
        || policy.runtime != sender.owner.runtime
        || certificate.subject.ordinary_app_credential_digest != sender_credential_digest
        || certificate.subject.issuance.credential != credential
        || certificate.subject.issuance.release_id != n.release_id
        || certificate.subject.issuance.hardware_policy_digest != release.hardware_policy_digest()
        || control.subject.request.owner != sender.owner
        || control.subject.request.enrollment_original_sha256
            != <DigestV1>::from(Sha256::digest(&outgoing.financial_certificate_original))
        || control.subject.request.credential_original_sha256
            != <DigestV1>::from(Sha256::digest(&outgoing.credential_original))
        || control.subject.release_id != n.release_id
        || control.subject.hardware_profile_id != n.hardware_profile_id
        || control.subject.profile_policy_epoch != n.policy_epoch
        || control.subject.ordinary_trust_policy_digest != c.trust_policy_digest
        || control.subject.app_authority_policy_digest != c.app_authority_policy_digest
        || control.subject.latest_integrity_lease_original != outgoing.purpose1_integrity_original
        || record.admission_clock_context.lower_at_ms < control.subject.issued_at_ms
        || record.admission_clock_context.upper_at_ms >= control.subject.expires_at_ms
        || record.admission_clock_context.lower_at_ms < body.clock_context.lower_at_ms
        || record.admission_clock_context.upper_at_ms < body.clock_context.upper_at_ms
    {
        return reject();
    }
    let wrapper = decode_stateless_original_v1(&outgoing.wrapper_original, 2, &material)
        .map_err(|e| e.to_string())?;
    let public = terminal_public(
        &material,
        n,
        body_digest,
        body.candidate_digest,
        record_digest,
        nullifier,
        body,
        output.ciphertext_commitment,
        &wrapper,
    )?;
    verify_stateless_public_v1(&material, &public, &wrapper).map_err(|e| e.to_string())?;
    let received_assertion_original_sha256 =
        Sha256::digest(assertion.transport_original().map_err(|e| e.to_string())?).into();
    assertion
        .recheck_historical(financial)
        .map_err(|e| e.to_string())?;
    receiver
        .recheck_proof_custody()
        .map_err(|e| e.to_string())?;
    Ok(KagemushaVerifiedOrdinaryReceivedCashOutputV1 {
        request: request.clone(),
        output: *output,
        encrypted_credit: encrypted_credit.clone(),
        preparation_clock: *preparation_clock,
        outgoing_original: outgoing_original.to_vec(),
        request_original,
        commit: commit.clone(),
        received_assertion_original_sha256,
    })
}

fn require_commit_original_selectors(
    commit: &KagemushaOrdinaryLineageCommitV1,
    original: &[u8],
    outgoing: &KagemushaOrdinaryCashOutgoingOriginalV1,
) -> Result<()> {
    require_same_digest_selectors(
        [
            commit.outgoing_original_sha256,
            commit.wrapper_proofs_original_sha256,
            commit.purpose1_approval_original_sha256,
            commit.terminal_record_original_sha256,
            commit.purpose1_financial_control_original_sha256,
            commit.purpose1_clock_context_original_sha256,
        ],
        [
            Sha256::digest(original).into(),
            Sha256::digest(&outgoing.wrapper_original).into(),
            Sha256::digest(&outgoing.purpose1_approval_original).into(),
            Sha256::digest(outgoing.record.canonical_bytes()?).into(),
            Sha256::digest(&outgoing.financial_control_original).into(),
            Sha256::digest(outgoing.record.admission_clock_context.canonical_bytes()?).into(),
        ],
    )
}
fn require_same_digest_selectors(selected: [DigestV1; 6], actual: [DigestV1; 6]) -> Result<()> {
    if selected != actual {
        return reject();
    }
    Ok(())
}

fn require_complete_statement(
    outgoing: &KagemushaOrdinaryCashOutgoingOriginalV1,
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
) -> Result<()> {
    let n = &outgoing.normalized_preparation;
    let material = verifier.state_checkpoint_material();
    let rebuilt = KagemushaNormalizedGuardStatementV1::derive_from_transition(
        &outgoing.statement,
        crate::kagemusha_v1_recursion::KagemushaGuardContextV1 {
            release_id: n.release_id,
            liability_pool_id: n.liability_pool_id,
            lifecycle_binding_digest: n.lifecycle_binding_digest,
            prepared_transition_binding_digest: n.prepared_transition_binding_digest,
            terminal_commit_binding_digest: n.terminal_commit_binding_digest,
            sender_one_time_authorization_digest: n.sender_one_time_authorization_digest,
            receive_credit_binding_digest: n.receive_credit_binding_digest,
            transition_intent_digest: n.transition_intent_digest,
            transition_effect_digest: n.transition_effect_digest,
            recovery_record_digest: n.recovery_record_digest,
            durable_inbox_effect_digest: n.durable_inbox_effect_digest,
            durable_outbox_effect_digest: n.durable_outbox_effect_digest,
            canonical_empty_effect_digest: material.artifacts.canonical_empty_effect_digest,
        },
    )
    .map_err(|e| e.to_string())?;
    if rebuilt != *n {
        return reject();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn received_output_original_selectors_refuse_each_substituted_complete_original() {
        // Actual complete original hashes; this tests the selector join, not an assertion,
        // proof, request key holder or financial admission.
        let actual: [DigestV1; 6] = core::array::from_fn(|i| {
            let raw = norito::encode_canonical(&(1_u16, i as u64, vec![i as u8; 33])).unwrap();
            Sha256::digest(raw).into()
        });
        assert!(require_same_digest_selectors(actual, actual).is_ok());
        for i in 0..actual.len() {
            let mut selected = actual;
            selected[i][17] ^= 1;
            assert!(require_same_digest_selectors(selected, actual).is_err());
        }
        let mut swapped = actual;
        swapped.swap(1, 2);
        assert!(require_same_digest_selectors(swapped, actual).is_err());
    }
    #[test]
    fn received_output_decoder_refuses_data_without_complete_clock_and_wrapper_frame() {
        let raw = norito::encode_canonical(&KagemushaOrdinaryCashClockContextV1 {
            version: 1,
            request_nonce: [4; 32],
            signed_observations_original_digest: [5; 32],
            lower_at_ms: 6,
            upper_at_ms: 7,
        })
        .unwrap();
        for offered in [&[][..], &[0][..], raw.as_slice()] {
            assert!(KagemushaOrdinaryCashOutgoingOriginalV1::decode_original(offered).is_err());
        }
        let mut trailing = raw;
        trailing.push(0);
        assert!(KagemushaOrdinaryCashOutgoingOriginalV1::decode_original(&trailing).is_err());
    }
}
