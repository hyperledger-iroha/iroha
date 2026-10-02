//! Separate ordinary cash Terminal and CommitWrapper proof admission.
//!
//! This first-release family names its logical terminal record explicitly. It has no hardware
//! certificate field, decoder-to-owner conversion, or prepared/Bootstrap approval upgrade. The
//! private result is constructed only after the same actual Native selection, candidate, both
//! purpose-scoped Guards, both Terminal proofs, both Wrapper proofs and every whole history pass.

use super::{
    DigestV1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1,
    KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1, KagemushaEpAccumulatorV1, KagemushaEqAccumulatorV1,
    decide_kagemusha_ep_accumulator_v1, decide_kagemusha_eq_accumulator_v1,
    deferred_parent::ordinary_ipa_proof_profile_v1,
    native_backend::{verify_ep_succinct_protocol, verify_eq_succinct_protocol},
    ordinary_guard_verifier::KagemushaAuthenticatedOrdinaryTerminalGuardV1,
};
use crate::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, decode, digest_limbs, from_u128},
    kagemusha_v1_state::{
        KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1, KagemushaStateErrorV1,
        KagemushaStateV1,
    },
};
use halo2_proofs::{
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    poly::ipa::commitment::ParamsIPA,
};
use iroha_data_model::kagemusha::{
    KagemushaOrdinaryCashTerminalRecordV1, KagemushaReleasePurposeV1,
    kagemusha_asset_identity_digest_v1, kagemusha_ciphertext_digest_v1,
    kagemusha_ordinary_output_binding_digest_v1, kagemusha_ordinary_transition_nullifier_v1,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use snark_verifier::verifier::plonk::PlonkProtocol;

type Result<T> = core::result::Result<T, KagemushaStateErrorV1>;
type History = [u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1];
/// Private retained inner proofs have a separate bound; they are never compact payment transport.
pub(super) const ORDINARY_INNER_TERMINAL_MAX_BYTES_V1: usize = 2 * 1024 * 1024;
pub(super) const ORDINARY_TERMINAL_PUBLIC_PREFIX_V1: usize = 49;
pub(super) const ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1: usize = 83;

/// Genuine immutable protocols recompiled from the authenticated distinct released key roles.
pub(super) struct OrdinaryCashTerminalMaterialV1<'a> {
    pub(super) eq_parameters: &'a ParamsIPA<EqAffine>,
    pub(super) ep_parameters: &'a ParamsIPA<EpAffine>,
    pub(super) terminal_eq_protocol: &'a PlonkProtocol<EqAffine>,
    pub(super) terminal_ep_protocol: &'a PlonkProtocol<EpAffine>,
    pub(super) wrapper_eq_protocol: &'a PlonkProtocol<EqAffine>,
    pub(super) wrapper_ep_protocol: &'a PlonkProtocol<EpAffine>,
    pub(super) release_id: DigestV1,
    pub(super) suite_id: DigestV1,
    pub(super) vk_set_digest: DigestV1,
    pub(super) artifact_manifest_digest: DigestV1,
    pub(super) terminal_protocol_digests: [DigestV1; 2],
    pub(super) wrapper_protocol_digests: [DigestV1; 2],
}

/// Data-only paired proof original. Relation1 is private Terminal; relation2 is compact Wrapper.
/// The explicit phase, schema and exact protocol pair prevent a cross-family decoder upgrade.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::OrdinaryCashProofPairV1",
    frame = "iroha.kagemusha.core.v1.ordinary-cash-proof-pair"
)]
pub(super) struct OrdinaryCashProofPairWireV1 {
    pub(super) version: u16,
    pub(super) relation: u8,
    pub(super) release_id: DigestV1,
    pub(super) artifact_manifest_digest: DigestV1,
    pub(super) eq_protocol_digest: DigestV1,
    pub(super) ep_protocol_digest: DigestV1,
    pub(super) eq_deferred_audit: DigestV1,
    pub(super) ep_deferred_audit: DigestV1,
    pub(super) eq_proof: Vec<u8>,
    pub(super) ep_proof: Vec<u8>,
    pub(super) eq_history: History,
    pub(super) ep_history: History,
}

/// Explicit ordinary 49+34 column. The common width reserves slot26 for this exact logical
/// terminal record, slot22 for the complete ordinary body and slot17 for the app credential
/// profile. These names do not claim an OEM certificate or hardware non-forking enforcement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct OrdinaryCashTerminalPublicV1 {
    pub(super) operation: u8,
    pub(super) suite_id: DigestV1,
    pub(super) vk_set_digest: DigestV1,
    pub(super) release_id: DigestV1,
    pub(super) network_id: DigestV1,
    pub(super) asset_id: DigestV1,
    pub(super) asset_incarnation: DigestV1,
    pub(super) asset_scale: u32,
    pub(super) liability_pool_id: DigestV1,
    pub(super) app_credential_profile_id: DigestV1,
    pub(super) policy_epoch: u64,
    pub(super) lifecycle_digest: DigestV1,
    pub(super) body_digest: DigestV1,
    pub(super) candidate_digest: DigestV1,
    pub(super) terminal_record_digest: DigestV1,
    pub(super) transition_nullifier: DigestV1,
    pub(super) request_digest: DigestV1,
    pub(super) receiver_credential_digest: DigestV1,
    pub(super) ciphertext_commitment: DigestV1,
    pub(super) amount: u128,
    pub(super) output_binding_digest: DigestV1,
    pub(super) redemption_manifest_digest: DigestV1,
    pub(super) eq_deferred_audit: DigestV1,
    pub(super) ep_deferred_audit: DigestV1,
    pub(super) eq_protocol_digest: DigestV1,
    pub(super) ep_protocol_digest: DigestV1,
}
impl OrdinaryCashTerminalPublicV1 {
    pub(super) fn validate(&self) -> core::result::Result<(), String> {
        if !matches!(self.operation, 2 | 4) || self.amount == 0 || self.policy_epoch == 0 {
            return Err("ordinary Terminal requires a positive Send/Redeem".into());
        }
        for value in [
            self.suite_id,
            self.vk_set_digest,
            self.release_id,
            self.network_id,
            self.asset_id,
            self.asset_incarnation,
            self.liability_pool_id,
            self.app_credential_profile_id,
            self.lifecycle_digest,
            self.body_digest,
            self.candidate_digest,
            self.terminal_record_digest,
            self.transition_nullifier,
            self.output_binding_digest,
            self.eq_deferred_audit,
            self.ep_deferred_audit,
            self.eq_protocol_digest,
            self.ep_protocol_digest,
        ] {
            if value == [0; 32] {
                return Err("ordinary Terminal binding absent".into());
            }
        }
        if self.eq_deferred_audit == self.ep_deferred_audit
            || self.eq_protocol_digest == self.ep_protocol_digest
            || decode::<Fp>(self.eq_protocol_digest).is_none()
            || decode::<Fq>(self.ep_protocol_digest).is_none()
        {
            return Err("ordinary Terminal parity roles are noncanonical".into());
        }
        let send = self.operation == 2;
        if [
            self.request_digest,
            self.receiver_credential_digest,
            self.ciphertext_commitment,
        ]
        .into_iter()
        .any(|v| (v != [0; 32]) != send)
            || (self.redemption_manifest_digest != [0; 32]) == send
        {
            return Err("ordinary Terminal operation-specific slots differ".into());
        }
        Ok(())
    }
    pub(super) fn public_prefix<F: KagemushaPoseidonFieldV1>(
        &self,
    ) -> core::result::Result<Vec<F>, String> {
        self.validate()?;
        let mut out = vec![F::from(u64::from(self.operation)), F::ONE];
        for value in [
            self.suite_id,
            self.vk_set_digest,
            self.release_id,
            self.network_id,
            self.asset_id,
            self.asset_incarnation,
        ] {
            out.extend(digest_limbs::<F>(value));
        }
        out.push(F::from(u64::from(self.asset_scale)));
        for value in [self.liability_pool_id, self.app_credential_profile_id] {
            out.extend(digest_limbs::<F>(value));
        }
        out.push(F::from(self.policy_epoch));
        for value in [
            self.lifecycle_digest,
            self.body_digest,
            self.candidate_digest,
            self.terminal_record_digest,
            self.transition_nullifier,
            self.request_digest,
            self.receiver_credential_digest,
            self.ciphertext_commitment,
        ] {
            out.extend(digest_limbs::<F>(value));
        }
        out.push(from_u128::<F>(self.amount));
        for value in [
            self.output_binding_digest,
            self.redemption_manifest_digest,
            self.eq_deferred_audit,
            self.ep_deferred_audit,
            self.eq_protocol_digest,
            self.ep_protocol_digest,
        ] {
            out.extend(digest_limbs::<F>(value));
        }
        if out.len() != ORDINARY_TERMINAL_PUBLIC_PREFIX_V1 {
            return Err("ordinary Terminal prefix width differs".into());
        }
        Ok(out)
    }
    pub(super) fn public_column<F: KagemushaPoseidonFieldV1>(
        &self,
        history: &History,
    ) -> core::result::Result<Vec<F>, String> {
        let mut out = self.public_prefix::<F>()?;
        for chunk in history.chunks_exact(16) {
            out.push(from_u128::<F>(u128::from_le_bytes(
                chunk.try_into().expect("fixed 16-byte history limb"),
            )));
        }
        if out.len() != ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1 {
            return Err("ordinary Terminal history width differs".into());
        }
        Ok(out)
    }
}

/// Actual complete cash proof admission. No decoder or generic proof-byte constructor exists.
pub(crate) struct KagemushaAuthenticatedOrdinaryCashTerminalV1 {
    operation_id: DigestV1,
    nonce: DigestV1,
    predecessor: KagemushaStateV1,
    successor: KagemushaStateV1,
    record: KagemushaOrdinaryCashTerminalRecordV1,
    candidate_digest: DigestV1,
    preparation_guard_original_sha256: DigestV1,
    terminal_guard_original_sha256: DigestV1,
    output_originals_digest: DigestV1,
    inner_terminal_original: Vec<u8>,
    commit_wrapper_original: Vec<u8>,
    wrapper_history_fold_originals: [[u8; super::KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1]; 2],
}
impl KagemushaAuthenticatedOrdinaryCashTerminalV1 {
    pub(crate) fn operation_id(&self) -> DigestV1 {
        self.operation_id
    }
    pub(crate) fn predecessor_state(&self) -> &KagemushaStateV1 {
        &self.predecessor
    }
    pub(crate) fn successor_state(&self) -> &KagemushaStateV1 {
        &self.successor
    }
    pub(crate) fn terminal_record(&self) -> &KagemushaOrdinaryCashTerminalRecordV1 {
        &self.record
    }
    pub(crate) fn candidate_envelope_digest(&self) -> DigestV1 {
        self.candidate_digest
    }
    pub(crate) fn output_originals_digest(&self) -> DigestV1 {
        self.output_originals_digest
    }
    pub(crate) fn inner_terminal_original(&self) -> &[u8] {
        &self.inner_terminal_original
    }
    pub(crate) fn commit_wrapper_original(&self) -> &[u8] {
        &self.commit_wrapper_original
    }
    /// Actual fixed1280-byte Eq/Ep Wrapper folds retained from the same genuine producer.
    pub(crate) fn wrapper_history_fold_originals(&self) -> [&[u8]; 2] {
        [
            &self.wrapper_history_fold_originals[0],
            &self.wrapper_history_fold_originals[1],
        ]
    }
    /// Reverify restored data against the same retained Native attempt before an actual CAS.
    pub(crate) fn recheck_terminal_selection(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
        guard: &KagemushaAuthenticatedOrdinaryTerminalGuardV1,
    ) -> Result<()> {
        verify_selection(
            selection,
            guard,
            &self.inner_terminal_original,
            &self.commit_wrapper_original,
            self.wrapper_history_fold_originals(),
        )?;
        if self.operation_id != selection.challenge().operation_id
            || self.nonce != selection.challenge().nonce
            || self.predecessor != *selection.selected_predecessor_state()
            || self.successor != *selection.selected_successor_state()
            || self.record != *selection.terminal_record()
            || self.candidate_digest != selection.candidate().candidate_envelope_digest()
            || self.preparation_guard_original_sha256
                != <DigestV1>::from(Sha256::digest(selection.preparation_guard().original()))
            || self.terminal_guard_original_sha256
                != <DigestV1>::from(Sha256::digest(guard.original()))
            || self.output_originals_digest != output_originals_digest(selection)?
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        selection.recheck_selected_originals_and_current_custody()
    }
}
pub(crate) fn verify_ordinary_cash_terminal_v1(
    selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
    guard: &KagemushaAuthenticatedOrdinaryTerminalGuardV1,
    inner_terminal_original: &[u8],
    commit_wrapper_original: &[u8],
    wrapper_history_fold_originals: [&[u8]; 2],
) -> Result<KagemushaAuthenticatedOrdinaryCashTerminalV1> {
    verify_selection(
        selection,
        guard,
        inner_terminal_original,
        commit_wrapper_original,
        wrapper_history_fold_originals,
    )?;
    let output_originals_digest = output_originals_digest(selection)?;
    selection.recheck_selected_originals_and_current_custody()?;
    Ok(KagemushaAuthenticatedOrdinaryCashTerminalV1 {
        operation_id: selection.challenge().operation_id,
        nonce: selection.challenge().nonce,
        predecessor: selection.selected_predecessor_state().clone(),
        successor: selection.selected_successor_state().clone(),
        record: *selection.terminal_record(),
        candidate_digest: selection.candidate().candidate_envelope_digest(),
        preparation_guard_original_sha256: Sha256::digest(selection.preparation_guard().original())
            .into(),
        terminal_guard_original_sha256: Sha256::digest(guard.original()).into(),
        output_originals_digest,
        inner_terminal_original: inner_terminal_original.to_vec(),
        commit_wrapper_original: commit_wrapper_original.to_vec(),
        wrapper_history_fold_originals: [
            wrapper_history_fold_originals[0]
                .try_into()
                .map_err(integrity)?,
            wrapper_history_fold_originals[1]
                .try_into()
                .map_err(integrity)?,
        ],
    })
}

fn verify_selection(
    selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
    guard: &KagemushaAuthenticatedOrdinaryTerminalGuardV1,
    inner: &[u8],
    wrapper: &[u8],
    wrapper_history_fold_originals: [&[u8]; 2],
) -> Result<()> {
    selection.recheck_selected_originals_and_current_custody()?;
    let preparation = selection.preparation_selection()?;
    selection
        .preparation_guard()
        .recheck_preparation_selection(&preparation)?;
    selection
        .candidate()
        .recheck_preparation_selection(&preparation, selection.preparation_guard())?;
    guard.recheck_terminal_selection(selection)?;
    let release = selection.authenticated_release()?;
    let material = selection
        .recursive_verifier()
        .ordinary_cash_terminal_verifier_material()
        .map_err(integrity)?;
    if release.purpose() != KagemushaReleasePurposeV1::Production
        || release.release_id() != material.release_id
        || release.vk_set_digest() != material.vk_set_digest
        || release.manifest_digest() != material.artifact_manifest_digest
    {
        return Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool);
    }
    let inner = decode_exact(inner, 1, &material)?;
    let wrapper = decode_exact(wrapper, 2, &material)?;
    let inner_public = selected_public(selection, &material, &inner)?;
    let wrapper_public = selected_public(selection, &material, &wrapper)?;
    verify_stateless_public_v1(&material, &inner_public, &inner)?;
    verify_stateless_public_v1(&material, &wrapper_public, &wrapper)?;
    super::ordinary_cash_commit_wrapper::require_exact_ordinary_cash_wrapper_inner_v1(
        &material,
        &inner_public,
        &wrapper_public,
        &inner,
        &wrapper,
        wrapper_history_fold_originals,
    )
    .map_err(integrity)?;
    // Same exact originals and both complete histories are now joined through the unchanged
    // actual paired Wrapper audits, rather than independent same-semantic proof acceptance.
    guard.recheck_terminal_selection(selection)?;
    selection.recheck_selected_originals_and_current_custody()
}

pub(super) fn selected_public(
    selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
    material: &OrdinaryCashTerminalMaterialV1<'_>,
    wire: &OrdinaryCashProofPairWireV1,
) -> Result<OrdinaryCashTerminalPublicV1> {
    selected_public_values_v1(
        selection,
        material,
        [wire.eq_protocol_digest, wire.ep_protocol_digest],
        [wire.eq_deferred_audit, wire.ep_deferred_audit],
    )
}
/// Same exact Native public metadata constructor for discovery, generation and admission.
/// Discovery supplies provisional audit data only; the real producer replaces both values with
/// complete scalar audits before proof writing. This helper grants no decoded proof authority.
pub(super) fn selected_public_values_v1(
    selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
    material: &OrdinaryCashTerminalMaterialV1<'_>,
    protocols: [DigestV1; 2],
    audits: [DigestV1; 2],
) -> Result<OrdinaryCashTerminalPublicV1> {
    let before = selection.selected_predecessor_state();
    let after = selection.selected_successor_state();
    let candidate = selection.candidate();
    let prepared = candidate.prepared_record();
    let body = selection.terminal_body();
    let record = selection.terminal_record();
    record
        .validate_against_originals(selection.terminal_intent(), prepared)
        .map_err(integrity)?;
    if body != &record.body
        || body.state_statement_digest != candidate.full_state_sha256()
        || body.candidate_digest != candidate.candidate_envelope_digest()
        || body.preparation_id != candidate.preparation_id()?
        || body.operation != prepared.operation
        || body.lifecycle_digest != prepared.lifecycle_binding_digest
        || body.amount != candidate.public_inputs().amount
        || body.secure_index_before != before.secure_index
        || body.secure_index_after != after.secure_index
        || before.suite_id != material.suite_id
        || after.suite_id != material.suite_id
        || before.vk_digest != material.vk_set_digest
        || after.vk_digest != material.vk_set_digest
        || before.release_id != material.release_id
        || after.release_id != material.release_id
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let nullifier = kagemusha_ordinary_transition_nullifier_v1(
        before.state_commitment,
        before.secure_index,
        before.hardware_epoch.epoch_id,
        *before.lane.network_id.as_bytes(),
        before.lane.device_lane_id,
        before.liability_pool_id,
    )
    .map_err(integrity)?;
    let ciphertext_commitment = match selection.send_transport_originals() {
        Some((request, output, encrypted, _, _)) => {
            if body.operation != 2
                || request.canonical_original_digest().map_err(integrity)? != body.request_digest
                || output.binding_digest().map_err(integrity)? != body.send_output_digest
                || output.encrypted_credit_digest != body.encrypted_credit_digest
                || kagemusha_ciphertext_digest_v1(encrypted) != body.encrypted_credit_digest
                || output.transition_nullifier != nullifier
                || output.sender_before_commitment != before.state_commitment
                || output.sender_after_commitment != after.state_commitment
                || output.amount != body.amount
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            output.ciphertext_commitment
        }
        None if body.operation == 4 => [0; 32],
        None => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
    };
    let value = OrdinaryCashTerminalPublicV1 {
        operation: body.operation,
        suite_id: material.suite_id,
        vk_set_digest: material.vk_set_digest,
        release_id: material.release_id,
        network_id: *before.lane.network_id.as_bytes(),
        asset_id: kagemusha_asset_identity_digest_v1(&before.lane.asset).map_err(integrity)?,
        asset_incarnation: *before.asset_incarnation.as_bytes(),
        asset_scale: before.lane.scale,
        liability_pool_id: before.liability_pool_id,
        app_credential_profile_id: before.hardware_profile_id,
        policy_epoch: before.policy_epoch,
        lifecycle_digest: body.lifecycle_digest,
        body_digest: body.binding_digest().map_err(integrity)?,
        candidate_digest: body.candidate_digest,
        terminal_record_digest: record.binding_digest().map_err(integrity)?,
        transition_nullifier: nullifier,
        request_digest: body.request_digest,
        receiver_credential_digest: body.recipient_credential_digest,
        ciphertext_commitment,
        amount: body.amount,
        output_binding_digest: kagemusha_ordinary_output_binding_digest_v1(
            prepared.projection_semantic_digest,
            body.candidate_digest,
            record.binding_digest().map_err(integrity)?,
        )
        .map_err(integrity)?,
        redemption_manifest_digest: body.artifact_manifest_digest,
        eq_deferred_audit: audits[0],
        ep_deferred_audit: audits[1],
        eq_protocol_digest: protocols[0],
        ep_protocol_digest: protocols[1],
    };
    value.validate().map_err(integrity)?;
    Ok(value)
}
fn output_originals_digest(
    selection: &KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1<'_>,
) -> Result<DigestV1> {
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:ordinary-cash-output-originals\0");
    hash.update([selection.terminal_body().operation]);
    if let Some((request, output, encrypted, _, _)) = selection.send_transport_originals() {
        let request = request.canonical_bytes().map_err(integrity)?;
        let output = output.canonical_bytes().map_err(integrity)?;
        for original in [request.as_slice(), output.as_slice(), encrypted] {
            hash.update(
                u64::try_from(original.len())
                    .map_err(integrity)?
                    .to_le_bytes(),
            );
            hash.update(original);
        }
    } else if selection.terminal_body().operation == 4 {
        let (output, beneficiary, manifest) = selection
            .redeem_transport_originals()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        output
            .validate_against_originals(
                beneficiary,
                selection.preparation_clock_context(),
                selection.authenticated_release()?.as_ref(),
            )
            .map_err(integrity)?;
        for original in [
            norito::encode_canonical(output).map_err(integrity)?,
            norito::encode_canonical(beneficiary).map_err(integrity)?,
            manifest.to_vec(),
        ] {
            hash.update(
                u64::try_from(original.len())
                    .map_err(integrity)?
                    .to_le_bytes(),
            );
            hash.update(&original);
        }
    } else {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(hash.finalize().into())
}
/// Stateless bounded mathematical decoder under the same actual authenticated protocol material.
/// It produces data only; genuine current/history decisions remain mandatory below.
pub(super) fn decode_stateless_original_v1(
    original: &[u8],
    relation: u8,
    material: &OrdinaryCashTerminalMaterialV1<'_>,
) -> Result<OrdinaryCashProofPairWireV1> {
    decode_exact(original, relation, material)
}
/// Same real ordinary Terminal/Wrapper proof and complete carried-history decisions for Core.
/// No caller-provided verifier callback or Native money result is introduced.
pub(super) fn verify_stateless_public_v1(
    material: &OrdinaryCashTerminalMaterialV1<'_>,
    public: &OrdinaryCashTerminalPublicV1,
    wire: &OrdinaryCashProofPairWireV1,
) -> Result<()> {
    public.validate().map_err(integrity)?;
    let expected = match wire.relation {
        1 => material.terminal_protocol_digests,
        2 => material.wrapper_protocol_digests,
        _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
    };
    if wire.release_id != material.release_id
        || wire.artifact_manifest_digest != material.artifact_manifest_digest
        || [wire.eq_protocol_digest, wire.ep_protocol_digest] != expected
        || public.release_id != material.release_id
        || public.suite_id != material.suite_id
        || public.vk_set_digest != material.vk_set_digest
        || public.eq_deferred_audit != wire.eq_deferred_audit
        || public.ep_deferred_audit != wire.ep_deferred_audit
        || [public.eq_protocol_digest, public.ep_protocol_digest] != expected
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    verify_wire(material, public, wire)
}

fn decode_exact(
    original: &[u8],
    relation: u8,
    m: &OrdinaryCashTerminalMaterialV1<'_>,
) -> Result<OrdinaryCashProofPairWireV1> {
    let (eq, ep, protocols, maximum) = match relation {
        1 => (
            m.terminal_eq_protocol,
            m.terminal_ep_protocol,
            m.terminal_protocol_digests,
            ORDINARY_INNER_TERMINAL_MAX_BYTES_V1,
        ),
        2 => (
            m.wrapper_eq_protocol,
            m.wrapper_ep_protocol,
            m.wrapper_protocol_digests,
            KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1,
        ),
        _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
    };
    if original.is_empty()
        || original.len() > maximum
        || eq.num_instance != [ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1]
        || ep.num_instance != [ORDINARY_TERMINAL_PUBLIC_INSTANCES_V1]
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let lengths = [
        ordinary_ipa_proof_profile_v1(eq)
            .map_err(integrity)?
            .byte_len,
        ordinary_ipa_proof_profile_v1(ep)
            .map_err(integrity)?
            .byte_len,
    ];
    if lengths.contains(&0)
        || lengths
            .iter()
            .try_fold(0usize, |a, b| a.checked_add(*b))
            .is_none_or(|v| v > maximum)
        || (relation == 2
            && lengths
                .into_iter()
                .any(|v| v > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1))
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    decode_profile_exact(
        original,
        relation,
        m.release_id,
        m.artifact_manifest_digest,
        protocols,
        lengths,
    )
}
/// Sole exact DATA decoder. Admission supplies authenticated protocol geometry; a recursive
/// consumer supplies its exact protocol identities/geometry and subsequently proves them.
/// This function authenticates no issuer, Native source, history or monetary operation.
pub(super) fn decode_profile_exact(
    original: &[u8],
    relation: u8,
    release_id: DigestV1,
    manifest: DigestV1,
    protocols: [DigestV1; 2],
    lengths: [usize; 2],
) -> Result<OrdinaryCashProofPairWireV1> {
    let maximum = if relation == 1 {
        ORDINARY_INNER_TERMINAL_MAX_BYTES_V1
    } else if relation == 2 {
        KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1
    } else {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    };
    if original.is_empty() || original.len() > maximum {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let w: OrdinaryCashProofPairWireV1 = norito::decode_canonical_with_limits(
        original,
        norito::canonical_decode_limits(original.len()),
    )
    .map_err(integrity)?;
    if norito::encode_canonical(&w).map_err(integrity)? != original
        || w.version != 1
        || w.relation != relation
        || w.release_id != release_id
        || w.artifact_manifest_digest != manifest
        || [w.eq_protocol_digest, w.ep_protocol_digest] != protocols
        || lengths.contains(&0)
        || w.eq_proof.len() != lengths[0]
        || w.ep_proof.len() != lengths[1]
        || w.eq_deferred_audit == [0; 32]
        || w.ep_deferred_audit == [0; 32]
        || w.eq_deferred_audit == w.ep_deferred_audit
        || (relation == 2
            && lengths
                .into_iter()
                .any(|v| v > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1))
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(w)
}

fn verify_wire(
    m: &OrdinaryCashTerminalMaterialV1<'_>,
    p: &OrdinaryCashTerminalPublicV1,
    w: &OrdinaryCashProofPairWireV1,
) -> Result<()> {
    let (eqp, epp) = if w.relation == 1 {
        (m.terminal_eq_protocol, m.terminal_ep_protocol)
    } else {
        (m.wrapper_eq_protocol, m.wrapper_ep_protocol)
    };
    let eq = verify_eq_succinct_protocol(
        m.eq_parameters,
        eqp,
        &w.eq_proof,
        &p.public_column::<Fp>(&w.eq_history).map_err(integrity)?,
    )
    .map_err(integrity)?;
    let ep = verify_ep_succinct_protocol(
        m.ep_parameters,
        epp,
        &w.ep_proof,
        &p.public_column::<Fq>(&w.ep_history).map_err(integrity)?,
    )
    .map_err(integrity)?;
    let eq = KagemushaEqAccumulatorV1::from_native(&eq).map_err(integrity)?;
    let ep = KagemushaEpAccumulatorV1::from_native(&ep).map_err(integrity)?;
    let eqh = KagemushaEqAccumulatorV1::try_from_bytes(&w.eq_history).map_err(integrity)?;
    let eph = KagemushaEpAccumulatorV1::try_from_bytes(&w.ep_history).map_err(integrity)?;
    decide_kagemusha_eq_accumulator_v1(m.eq_parameters, &eq)
        .and_then(|()| decide_kagemusha_eq_accumulator_v1(m.eq_parameters, &eqh))
        .map_err(integrity)?;
    decide_kagemusha_ep_accumulator_v1(m.ep_parameters, &ep)
        .and_then(|()| decide_kagemusha_ep_accumulator_v1(m.ep_parameters, &eph))
        .map_err(integrity)
}
fn integrity(_: impl core::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::SnapshotIntegrity
}

#[cfg(test)]
#[path = "ordinary_cash_terminal_verifier_tests.rs"]
mod tests;
