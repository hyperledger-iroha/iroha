//! Genuine historical ordinary Mint source for proof publication, never a new debit.
//!
//! The constructor owns a real immutable State view. It admits only the complete original
//! successful signed instruction whose transaction hash and immutable reserve receipt are
//! certified by the same Kura/native execution. Offered finality or a DATA decoder cannot
//! construct this capability. Old signed windows are checked at their authentic original
//! clock endpoints; no current financial/effect authority is lent by this source.
use super::ordinary_mint_clock::admit_retained_ordinary_mint_signed_clock_v1;
use super::ordinary_mint_permission::{
    KagemushaWorldOrdinaryMintIssuerPurposeV1, admit_retained_ordinary_mint_issuer_purpose_v1,
};
use super::ordinary_mint_submission::{
    admit_control, decode_exact, decode_issuer_purpose_original, integrity_originals,
};
use super::*;
use crate::{
    state::{StateReadOnly, StateView, WorldReadOnly as _},
    sumeragi::certified_chain::committed_block,
};
use iroha_core_zk::kagemusha_v1_recursion::KagemushaVerifiedOrdinaryMintAuthorizationV1;
use iroha_data_model::{
    block::{SharedSignedBlock, SignedBlock},
    isi::kagemusha_v1::TopUpKagemushaOrdinaryV1,
    kagemusha::*,
    transaction::{Executable, ExecutableBatchItem, TransactionEntrypoint},
};
use mv::storage::StorageReadOnly as _;
use std::{any::Any, num::NonZeroUsize};

/// Closed proof-publication source owned by the authentic native State/Kura snapshot.
/// No decoder, Clone, signing/effect constructor or user financial-secret borrower exists.
pub struct KagemushaAuthenticatedOrdinaryNodeFinalizedMintSourceV1<'state> {
    view: StateView<'state>,
    record: kagemusha_v1_reserve::KagemushaOrdinaryTopUpRecordV1,
    finalized: KagemushaOrdinaryTopUpFinalizedOriginalV1,
    anchor: KagemushaFinalityTrustAnchorV1,
    submission: KagemushaOrdinaryNodeMintSubmissionV1,
    authorization: KagemushaVerifiedOrdinaryMintAuthorizationV1,
    purpose: KagemushaWorldOrdinaryMintIssuerPurposeV1,
    carrier: SharedSignedBlock,
}
impl<'state> KagemushaAuthenticatedOrdinaryNodeFinalizedMintSourceV1<'state> {
    pub(super) fn authenticate(
        view: StateView<'state>,
        height: u64,
        operation_id: [u8; 32],
    ) -> Result<Self, String> {
        if height < 2 || operation_id == [0; 32] {
            return Err("ordinary publication requires an actual successor execution".into());
        }
        let record = match view.world.kagemusha_reserve_operations.get(&operation_id) {
            Some(KagemushaReserveOperationRecordV1::OrdinaryTopUp(record)) => {
                record.as_ref().clone()
            }
            _ => return Err("ordinary publication has no actual ordinary operation".into()),
        };
        record.validate_basic().map_err(|e| e.to_string())?;
        let finalized =
            kagemusha_v1_reserve::read_finalized_ordinary_top_up_v1(&view, height, operation_id)?
                .ok_or("ordinary publication lacks authentic finalized source")?;
        let anchor = KagemushaFinalityTrustAnchorV1 {
            network_id: *view.network_id(),
            checkpoint: crate::sumeragi::finality::build_checkpoint(&view, height)
                .map_err(|e| e.to_string())?,
        };
        finalized.validate_against(&anchor)?;
        let actual = committed_block(&view, height).map_err(|e| e.to_string())?;
        let carrier = actual.block().clone();
        let submission = archived_submission(&carrier, &record, view.network_id())?;
        let token = decode_issuer_purpose_original(&submission.issuer_purpose_original)?;
        let purpose = admit_retained_ordinary_mint_issuer_purpose_v1(&view, &token)?;
        let native = actual_runtime(&view)?;
        let authorization = authenticate_historical_authorization(
            &view,
            native,
            &record.payer,
            &purpose,
            &submission,
        )?;
        let source = Self {
            view,
            record,
            finalized,
            anchor,
            submission,
            authorization,
            purpose,
            carrier,
        };
        source.recheck_retained_custody()?;
        Ok(source)
    }
    /// Actual same installed runtime; no replacement or offered verifier is accepted.
    pub(crate) fn runtime(&self) -> Result<&AuthenticatedKagemushaV1RuntimeVerifier, String> {
        self.recheck_retained_custody()?;
        actual_runtime(&self.view)
    }
    /// Join an adapter's runtime to the exact independently installed owner retained here.
    pub(super) fn recheck_runtime(
        &self,
        offered: &AuthenticatedKagemushaV1RuntimeVerifier,
    ) -> Result<(), String> {
        self.recheck_retained_custody()?;
        if !std::ptr::eq(actual_runtime(&self.view)?, offered) {
            return Err(
                "ordinary publication adapter substitutes the installed runtime owner".into(),
            );
        }
        Ok(())
    }
    pub(crate) fn view(&self) -> &StateView<'state> {
        &self.view
    }
    /// Exact closed Mint113 admission of the complete original request.
    pub fn authorization(&self) -> Result<&KagemushaVerifiedOrdinaryMintAuthorizationV1, String> {
        self.recheck_retained_custody()?;
        Ok(&self.authorization)
    }
    /// Complete finalized ordinary source, including request/decision/native finality originals.
    pub fn finalized(&self) -> Result<&KagemushaOrdinaryTopUpFinalizedOriginalV1, String> {
        self.recheck_retained_custody()?;
        Ok(&self.finalized)
    }
    /// Independently derived actual native prefix anchor from the same retained State view.
    pub fn trust_anchor(&self) -> Result<&KagemushaFinalityTrustAnchorV1, String> {
        self.recheck_retained_custody()?;
        Ok(&self.anchor)
    }
    /// Exact actual reserve operation, distinct from any offered source data.
    pub(crate) fn record(
        &self,
    ) -> Result<&kagemusha_v1_reserve::KagemushaOrdinaryTopUpRecordV1, String> {
        self.recheck_retained_custody()?;
        Ok(&self.record)
    }
    /// Recheck immutable original custody, without renewing original windows or reproving.
    pub fn recheck_retained_custody(&self) -> Result<(), String> {
        self.purpose.recheck_retained_scope(&self.view)?;
        let runtime = actual_runtime(&self.view)?;
        let context = &self.authorization.authorization().statement.context;
        if context.release_id != self.record.release_id
            || context.lineage.owner.runtime.network_id != *self.view.network_id()
            || self.authorization.request_original() != self.record.request_original
            || self.submission.topup_request_original != self.finalized.request_original
            || self.submission.debit_decision_original != self.finalized.issuer_decision_original
            || self.finalized.finality.reserve_receipt_witness.receipt
                != self.record.reserve_receipt
        {
            return Err("ordinary publication retained original selectors differ".into());
        }
        let current = self
            .view
            .world
            .kagemusha_reserve_operations
            .get(&self.record.operation_id);
        if !matches!(current, Some(KagemushaReserveOperationRecordV1::OrdinaryTopUp(r)) if r.as_ref() == &self.record)
        {
            return Err("ordinary publication actual immutable operation differs".into());
        }
        if self
            .view
            .world
            .kagemusha_mint_credit_operations
            .get(&self.record.credit_id)
            .copied()
            != Some(self.record.operation_id)
            || self
                .view
                .world
                .kagemusha_issuance_operations
                .get(&self.record.issuance_commitment)
                .copied()
                != Some(self.record.operation_id)
        {
            return Err("ordinary publication original credit/issuance indexes differ".into());
        }
        let height = usize::try_from(self.finalized.finality.finality_proof.height())
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or("ordinary publication height invalid")?;
        let actual = self
            .view
            .canonical_block_by_height(height)
            .map_err(|e| e.to_string())?;
        if actual.as_ref() != self.carrier.as_ref() {
            return Err("ordinary publication original Kura carrier changed".into());
        }
        let _ = ordinary_mint_runtime::selected_retained_runtime(
            runtime,
            &self.record.request_original,
        )?;
        Ok(())
    }
}

fn actual_runtime<'a>(
    view: &'a StateView<'_>,
) -> Result<&'a AuthenticatedKagemushaV1RuntimeVerifier, String> {
    let native = view.kagemusha_v1_runtime_verifier.as_ref();
    let native_any: &dyn Any = native;
    runtime_matches_governed_registry(native_any, view.world.kagemusha_verifier_registry.get())?;
    native_any
        .downcast_ref::<AuthenticatedKagemushaV1RuntimeVerifier>()
        .ok_or_else(|| "ordinary publication genuine installed release owner absent".into())
}

// Locate the full actual instruction, never a request-SHA-only reconstructed submission.
fn archived_submission(
    block: &SignedBlock,
    record: &kagemusha_v1_reserve::KagemushaOrdinaryTopUpRecordV1,
    network: &NetworkId,
) -> Result<KagemushaOrdinaryNodeMintSubmissionV1, String> {
    let mut selected = None;
    for (index, entry) in block.network_entrypoints().enumerate() {
        let TransactionEntrypoint::External(signed) = entry else {
            continue;
        };
        if *signed.hash().as_ref() != record.reserve_receipt.transaction_hash {
            continue;
        }
        if signed.authority() != &record.payer || signed.network_id() != Some(network) {
            return Err("ordinary publication original payer/network differs".into());
        }
        signed.verify_signature().map_err(|e| e.to_string())?;
        let output = block
            .network_output_at(u32::try_from(index).map_err(|e| e.to_string())?)
            .ok_or("ordinary publication actual execution output absent")?
            .1;
        if !output.result.is_ok() {
            return Err("ordinary publication original transaction did not succeed".into());
        }
        let instructions: Vec<&iroha_data_model::isi::InstructionBox> = match signed.instructions()
        {
            Executable::Instructions(items) => items.iter().collect(),
            Executable::Batch(items) => items
                .iter()
                .filter_map(|item| match item {
                    ExecutableBatchItem::Instruction(instruction) => Some(instruction),
                    ExecutableBatchItem::ContractCall(_) => None,
                })
                .collect(),
            _ => return Err("ordinary publication original instruction body absent".into()),
        };
        for instruction in instructions {
            let Some(top_up) = instruction
                .as_any()
                .downcast_ref::<TopUpKagemushaOrdinaryV1>()
            else {
                continue;
            };
            if top_up.request.topup_request_original != record.request_original {
                continue;
            }
            top_up.request.canonical_bytes()?;
            if top_up.request.debit_decision_original != record.issuer_decision_original
                || selected.is_some()
            {
                return Err(
                    "ordinary publication ambiguous or substituted complete submission".into(),
                );
            }
            selected = Some(top_up.request.clone());
        }
    }
    selected.ok_or_else(|| {
        "ordinary publication complete original successful instruction absent".into()
    })
}

fn authenticate_historical_authorization(
    view: &StateView<'_>,
    native: &AuthenticatedKagemushaV1RuntimeVerifier,
    authority: &AccountId,
    purpose: &KagemushaWorldOrdinaryMintIssuerPurposeV1,
    submission: &KagemushaOrdinaryNodeMintSubmissionV1,
) -> Result<KagemushaVerifiedOrdinaryMintAuthorizationV1, String> {
    submission.canonical_bytes()?;
    let request = KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(
        &submission.topup_request_original,
    )?;
    let context = &request.authorization.statement.context;
    if authority != &context.lineage.owner.account_id
        || purpose.release_id() != context.release_id
        || purpose.issuer_policy().runtime != context.lineage.owner.runtime
    {
        return Err("ordinary publication original authority/World purpose differs".into());
    }
    request.verify_account_signature(&submission.account_consent)?;
    let runtime = ordinary_mint_runtime::selected_retained_runtime(
        native,
        &submission.topup_request_original,
    )?;
    let release = &runtime.release;
    let decision = KagemushaSignedOrdinaryMintDebitDecisionV1::decode_canonical_exact(
        &submission.debit_decision_original,
    )?;
    decision.verify_for_request(
        &request,
        &decision.subject.selection,
        purpose.issuer_policy(),
    )?;
    let preparation_clock = admit_retained_ordinary_mint_signed_clock_v1(
        view,
        purpose,
        &submission.clock_selection_original,
        &submission.preparation_clock_original,
        &submission.preparation_clock_parent_originals,
        &context.clock_context,
    )?;
    let decision_clock = admit_retained_ordinary_mint_signed_clock_v1(
        view,
        purpose,
        &submission.clock_selection_original,
        &submission.decision_clock_original,
        &submission.decision_clock_parent_originals,
        &decision.subject.decision_clock_context,
    )?;
    let actual_decision =
        committed_block(view, decision_clock.height()).map_err(|e| e.to_string())?;
    if decision.subject.authority_height != decision_clock.height()
        || decision.subject.authority_context_id != decision_clock.context_id()
        || actual_decision
            .commitment()
            .execution
            .world_state_root
            .as_ref()
            != &decision.subject.world_root
    {
        return Err("ordinary publication original decision/certified World cut differs".into());
    }
    let policy = KagemushaSignedOrdinaryAppIdentityPolicyV1::decode_canonical_exact(
        &submission.identity_policy_original,
    )?
    .authenticate(
        purpose.app_identity_authority(),
        decision.subject.decision_clock_context.lower_at_ms,
    )?;
    policy.recheck_at_trusted_time(decision.subject.decision_clock_context.upper_at_ms)?;
    let core = KagemushaOrdinaryEnrollmentIssuerPolicyV1::decode_canonical_exact(
        &submission.core_enrollment_issuer_policy_original,
    )?;
    let core_issuer = core.authenticate_under_policy(
        &policy,
        core.lane_namespace_id,
        decision.subject.decision_clock_context.lower_at_ms,
    )?;
    core_issuer.recheck_current(
        &policy,
        core.lane_namespace_id,
        decision.subject.decision_clock_context.upper_at_ms,
    )?;
    if core.enrollment_issuer_key == purpose.issuer_policy().issuer_public_key
        || core.app_authority_key == purpose.issuer_policy().issuer_public_key
        || core.derive_enrollment_lane(&purpose.issuer_policy().runtime.fi_id, authority)?
            != context.lineage.owner.lane_id
    {
        return Err("ordinary publication independent original Core/App/FI roles differ".into());
    }
    let c = KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(
        &submission.enrollment_challenge_original,
    )?;
    let credential =
        KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(&submission.credential_original)?;
    let enrollment = || KagemushaOrdinaryEnrollmentProofOriginalsV1 {
        preparation_original: &submission.enrollment_challenge_original,
        expected_preparation: &c.challenge,
        raw_admission_original: &submission.raw_admission_original,
        platform_original: &submission.platform_attestation_original,
        possession_original: &submission.enrollment_possession_original,
        credential_original: &submission.credential_original,
        selected_key: &credential.subject.app_public_key,
    };
    let historical = policy.authenticate_proof_enrollment_originals(
        enrollment(),
        release,
        integrity_originals(submission.preparation_integrity.as_ref()),
        context.clock_context.lower_at_ms,
        context.clock_context.upper_at_ms,
    )?;
    let admitted_decision = policy.authenticate_proof_enrollment_originals(
        enrollment(),
        release,
        integrity_originals(submission.decision_integrity.as_ref()),
        decision.subject.decision_clock_context.lower_at_ms,
        decision.subject.decision_clock_context.upper_at_ms,
    )?;
    let fi: KagemushaOrdinaryRetailEnrollmentCertificateV1 = decode_exact(
        &submission.financial_enrollment_original,
        KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
    )?;
    fi.signature
        .verify(
            &purpose.issuer_policy().issuer_public_key,
            &fi.subject.approval_payload()?,
        )
        .map_err(|_| "ordinary publication full FI signature rejected")?;
    let subject = &fi.subject;
    if subject.owner != context.lineage.owner
        || subject.issuer_policy_id != purpose.issuer_policy().issuer_policy_id
        || subject.issuer_audience != purpose.issuer_policy().issuer_audience
        || subject.issuance.credential.canonical_bytes()? != submission.credential_original
        || subject.ordinary_app_credential_digest != historical.credential().digest()
        || subject.issuance.release_id != release.release_id()
        || subject.issuance.hardware_policy_digest != release.hardware_policy_digest()
        || subject.issuance.core_authorization_key_reference == [0; 32]
        || subject.issued_at_ms < purpose.issuer_policy().valid_from_ms
        || subject.expires_at_ms > purpose.issuer_policy().expires_at_ms
        || subject
            .expires_at_ms
            .checked_sub(subject.issued_at_ms)
            .is_none_or(|n| n == 0 || n > purpose.issuer_policy().maximum_certificate_lifetime_ms)
    {
        return Err("ordinary publication complete FI scope/original equation differs".into());
    }
    for interval in [
        context.clock_context,
        decision.subject.decision_clock_context,
    ] {
        if interval.lower_at_ms < subject.issued_at_ms
            || interval.upper_at_ms >= subject.expires_at_ms
        {
            return Err(
                "ordinary publication FI does not cover both authentic historical endpoints".into(),
            );
        }
    }
    admit_control(
        submission,
        &fi,
        purpose.issuer_policy(),
        historical.credential(),
        historical.selected_integrity_lease(),
        &context.clock_context,
        &submission.preparation_control_original,
    )?;
    if <[u8; 32]>::from(Sha256::digest(&submission.preparation_control_original))
        != context.financial_control_original_sha256
    {
        return Err("ordinary publication historical FI original differs".into());
    }
    let control = admit_control(
        submission,
        &fi,
        purpose.issuer_policy(),
        admitted_decision.credential(),
        admitted_decision.selected_integrity_lease(),
        &decision.subject.decision_clock_context,
        &submission.current_control_original,
    )?;
    // The whole submission codec also joins this selector. Retain it explicitly at the
    // authority boundary rather than accepting another correctly signed status original.
    if <[u8; 32]>::from(Sha256::digest(&submission.current_control_original))
        != decision.subject.current_financial_control_original_sha256
    {
        return Err(
            "ordinary publication exact original FI/debit decision selector differs".into(),
        );
    }
    let current_c = admitted_decision.credential().subject();
    let pi_deadline = admitted_decision
        .selected_integrity_lease()
        .map(|lease| {
            lease
                .subject()
                .expires_at_ms
                .min(lease.subject().binding.refresh_before_ms)
        })
        .unwrap_or(current_c.expires_at_ms);
    let original_effect_deadline = [
        control.subject.expires_at_ms,
        subject.expires_at_ms,
        current_c.expires_at_ms,
        policy.policy().profile.expires_at_ms,
        pi_deadline,
    ]
    .into_iter()
    .min()
    .ok_or("ordinary publication original effect deadline absent")?;
    if decision.subject.expires_at_ms > original_effect_deadline {
        return Err("ordinary publication original debit outlived authentic FI/C/PI".into());
    }
    if control.subject.authority_height != decision_clock.height()
        || *control.subject.authority_context_id.as_ref() != decision_clock.context_id()
        || *control.subject.world_root.as_ref() != decision.subject.world_root
        || control.subject.data_incarnation_digest != decision.subject.data_incarnation_digest
        || control.subject.data_policy_epoch != decision.subject.data_policy_epoch
        || control.subject.data_schema_epoch != decision.subject.data_schema_epoch
    {
        return Err(
            "ordinary publication historical FI/decision certified identities differ".into(),
        );
    }
    historical.recheck_originals(&policy)?;
    admitted_decision.recheck_originals(&policy)?;
    let proof = ordinary_mint_runtime::verify_retained(
        native,
        &submission.topup_request_original,
        historical.credential(),
        historical.selected_integrity_lease(),
        preparation_clock.verified_original(),
    )?;
    purpose.recheck_retained_scope(view)?;
    Ok(proof)
}
