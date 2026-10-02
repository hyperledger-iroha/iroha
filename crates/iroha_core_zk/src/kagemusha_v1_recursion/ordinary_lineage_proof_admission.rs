//! Stateless ordinary proof admission for the governed DATA lineage service.
//!
//! These closed results authenticate mathematical originals, not current FI, DATA finality,
//! a mutable Native wallet, or offline non-forking custody. The DATA owner separately admits
//! the signed FI-control original and atomically fences the exact predecessor lineage.
//! No private balance, secret, replay path or sealed recovery stream is exported here.

use super::{
    DigestV1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1,
    KagemushaAuthenticatedRecursiveVerifierV1, KagemushaEpAccumulatorV1, KagemushaEqAccumulatorV1,
    KagemushaNormalizedGuardStatementV1, KagemushaOperationV1, KagemushaPairedProofV1,
    decide_kagemusha_ep_accumulator_v1, decide_kagemusha_eq_accumulator_v1,
    deferred_parent::ordinary_ipa_proof_profile_v1,
    native_backend::{verify_ep_succinct_protocol, verify_eq_succinct_protocol},
    ordinary_guard_verifier,
    state_relation::{RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT, public_instance as s},
    terminal_authorization::canonical_terminal_authorization_candidate_digest_v1,
};
use crate::kagemusha_v1_poseidon::from_u128;
use halo2_proofs::halo2curves::pasta::{Fp, Fq};
use iroha_data_model::kagemusha::{
    KagemushaAppOperationApprovalPurposeV1, KagemushaAppOperationApprovalV1,
    KagemushaOperationKindV1, KagemushaOrdinaryPreparedOutgoingV1,
    KagemushaOrdinaryPreparedTransitionV1, KagemushaVerifiedOrdinaryAppCredentialV1,
    KagemushaVerifiedPlayIntegrityRefreshLeaseV1,
    kagemusha_ordinary_app_approval_proof_binding_digest_v1,
    kagemusha_ordinary_financial_authorization_proof_binding_digest_v1,
    kagemusha_ordinary_financial_epoch_id_v1,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

#[path = "ordinary_incoming_reservation_admission.rs"]
mod incoming_admission;
pub(crate) use incoming_admission::{
    GeneratedOrdinaryIncomingCommitOriginalsV1, assemble_ordinary_incoming_commit_v1,
    assemble_ordinary_incoming_reservation_v1, readmit_ordinary_incoming_commit_v1,
};
pub use incoming_admission::{
    KAGEMUSHA_ORDINARY_INCOMING_COMMIT_BUNDLE_MAX_BYTES_V1,
    KAGEMUSHA_ORDINARY_INCOMING_RESERVATION_BUNDLE_MAX_BYTES_V1,
    KagemushaOrdinaryIncomingCommitProofBundleV1,
    KagemushaOrdinaryIncomingReservationProofBundleV1,
    KagemushaVerifiedOrdinaryIncomingCommitProofV1,
    KagemushaVerifiedOrdinaryIncomingReservationProofV1,
    ordinary_incoming_commit_carrier_max_bytes_v1, verify_ordinary_incoming_commit_v1,
    verify_ordinary_incoming_reservation_v1,
};

#[path = "ordinary_lineage_bundle_admission.rs"]
mod bundle_admission;
pub(super) use bundle_admission::terminal_public;
pub use bundle_admission::{
    KagemushaOrdinaryCashOutgoingOriginalV1, KagemushaOrdinaryLineageCommitProofBundleV1,
    KagemushaOrdinaryLineageOutgoingOriginalsV1, KagemushaOrdinaryLineageStateProofBundleV1,
    KagemushaOrdinaryLineageStatementOriginalV1, KagemushaVerifiedOrdinaryLineageAnchorProofV1,
    KagemushaVerifiedOrdinaryLineageCommitProofV1,
    KagemushaVerifiedOrdinaryLineageReservationProofV1,
    KagemushaVerifiedOrdinaryServiceReceivedCashOutputV1, verify_ordinary_lineage_anchor_v1,
    verify_ordinary_lineage_commit_v1, verify_ordinary_lineage_reservation_v1,
    verify_service_ordinary_received_cash_output_v1,
};

pub(crate) use bundle_admission::{
    KagemushaVerifiedOrdinaryReceivedCashOutputV1, ordinary_incoming_artifacts_v1,
    readmit_historical_ordinary_received_cash_output_v1, require_ordinary_incoming_predecessor_v1,
    verify_ordinary_received_cash_output_v1,
};

use super::ordinary_lineage_state_original::{
    KagemushaOrdinaryLineageStateOriginalV1, KagemushaOrdinaryLineageStateProjectionV1,
};
const CELLS: usize = RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT;
type Column = [[u8; 32]; CELLS];
type Result<T> = core::result::Result<T, String>;

/// Verified mathematical lineage State; only the actual installed protocol verifier constructs it.
/// It cannot become a Native selection or a DATA reservation without independent owner admission.
pub struct KagemushaVerifiedOrdinaryLineageStateProofV1 {
    normalized: KagemushaNormalizedGuardStatementV1,
    credential: DigestV1,
    financial_epoch: DigestV1,
    financial_authority: DigestV1,
    candidate: DigestV1,
    state_statement: DigestV1,
    projection_original_sha256: DigestV1,
    state_proof_original_sha256: DigestV1,
    state_original_sha256: DigestV1,
    guard_original_sha256: DigestV1,
    approval_original_sha256: DigestV1,
    authorization: DigestV1,
}
impl KagemushaVerifiedOrdinaryLineageStateProofV1 {
    /// Exact proof-bound public normalized selection, including financial sequence and heads.
    pub fn normalized_statement(&self) -> &KagemushaNormalizedGuardStatementV1 {
        &self.normalized
    }
    /// Same retained sender credential original digest.
    pub fn credential_digest(&self) -> DigestV1 {
        self.credential
    }
    /// Original financial epoch from the same issuer-authenticated credential.
    pub fn financial_epoch_id(&self) -> DigestV1 {
        self.financial_epoch
    }
    /// Separate financial authority commitment, never an app-key scalar.
    pub fn financial_authority_commitment(&self) -> DigestV1 {
        self.financial_authority
    }
    /// Exact normalized two-parity93-cell candidate name.
    pub fn candidate_digest(&self) -> DigestV1 {
        self.candidate
    }
    /// Full canonical public Transition/Bootstrap statement digest actually opened by the proof.
    pub fn state_statement_digest(&self) -> DigestV1 {
        self.state_statement
    }
    /// Exact bounded public State projection original SHA.
    pub fn projection_original_sha256(&self) -> DigestV1 {
        self.projection_original_sha256
    }
    /// Exact paired current State original SHA.
    pub fn state_proof_original_sha256(&self) -> DigestV1 {
        self.state_proof_original_sha256
    }
    /// Exact complete public State original SHA, including both current proofs and histories.
    pub fn state_original_sha256(&self) -> DigestV1 {
        self.state_original_sha256
    }
    /// Exact complete purpose-scoped Guard original SHA.
    pub fn guard_original_sha256(&self) -> DigestV1 {
        self.guard_original_sha256
    }
    /// Exact complete platform approval original SHA, separately from its proof transcript.
    pub fn approval_original_sha256(&self) -> DigestV1 {
        self.approval_original_sha256
    }
    /// Exact W plus originally selected full PI binding opened by the Guard.
    pub fn authorization_digest(&self) -> DigestV1 {
        self.authorization
    }
}

/// Full public statement data opened by the State proof, without private balances or secret.
pub enum KagemushaOrdinaryLineageStatementV1<'a> {
    /// Exact zero-Bootstrap statement; this cannot select a monetary edge.
    Zero(&'a crate::kagemusha_v1_state::BootstrapStatementV1),
    /// Exact outgoing State transition, including both full financial128-bit sequences.
    Outgoing(&'a crate::kagemusha_v1_state::TransitionProofStatementV1),
}

/// Admit a zero anchor or purpose2 outgoing State against original data and genuine released keys.
/// A service must separately authenticate current FI, signed DATA policy/finality and perform CAS.
/// # Errors
/// Refuses scope/purpose/protocol substitution, malformed public data, any forged current proof,
/// or a bad whole carried history. It has no accepting-verifier/callback overload.
pub fn verify_ordinary_lineage_state_proof_v1(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    normalized: &KagemushaNormalizedGuardStatementV1,
    credential: &KagemushaVerifiedOrdinaryAppCredentialV1,
    approval: &KagemushaAppOperationApprovalV1,
    original_lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    guard_original: &[u8],
    state_original: &[u8],
    statement: KagemushaOrdinaryLineageStatementV1<'_>,
    prepared: Option<&KagemushaOrdinaryPreparedOutgoingV1>,
) -> Result<KagemushaVerifiedOrdinaryLineageStateProofV1> {
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
    let operation = operation_tag(normalized.operation)?;
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
    let zero = operation == 0;
    require_ordinary_state_nonces(
        normalized.predecessor_state_nonce_commitment,
        normalized.successor_state_nonce_commitment,
        zero,
    )?;
    if (zero
        && (normalized.amount != 0
            || normalized.predecessor_state_commitment != [0; 32]
            || normalized.predecessor_logical_sequence != 0
            || normalized.successor_logical_sequence != 0
            || subject.secure_index_before != 0
            || subject.secure_index_after != 0
            || challenge.purpose != KagemushaAppOperationApprovalPurposeV1::MonetaryTransition
            || prepared.is_some()))
        || (!zero
            && (normalized.amount == 0
                || normalized.predecessor_hardware_epoch_id != epoch
                || normalized.predecessor_hardware_epoch_generation
                    != u128::from(c.hardware_epoch)
                || normalized.predecessor_hardware_policy_id != release.provider_policy_root()
                || normalized.predecessor_key_reference != c.app_key_reference
                || normalized.predecessor_state_commitment == [0; 32]
                || normalized.predecessor_logical_sequence.checked_add(1)
                    != Some(normalized.successor_logical_sequence)
                || subject.secure_index_before.checked_add(1) != Some(subject.secure_index_after)
                || challenge.purpose != KagemushaAppOperationApprovalPurposeV1::PrepareTransition
                || prepared.is_none()))
        || subject.candidate_envelope_digest != [0; 32]
        || subject.terminal_body_commitment != [0; 32]
    {
        return reject();
    }
    let context = super::KagemushaGuardContextV1 {
        release_id: normalized.release_id,
        liability_pool_id: normalized.liability_pool_id,
        lifecycle_binding_digest: normalized.lifecycle_binding_digest,
        prepared_transition_binding_digest: normalized.prepared_transition_binding_digest,
        terminal_commit_binding_digest: normalized.terminal_commit_binding_digest,
        sender_one_time_authorization_digest: normalized.sender_one_time_authorization_digest,
        receive_credit_binding_digest: normalized.receive_credit_binding_digest,
        transition_intent_digest: normalized.transition_intent_digest,
        transition_effect_digest: normalized.transition_effect_digest,
        recovery_record_digest: normalized.recovery_record_digest,
        durable_inbox_effect_digest: normalized.durable_inbox_effect_digest,
        durable_outbox_effect_digest: normalized.durable_outbox_effect_digest,
        canonical_empty_effect_digest: material.artifacts.canonical_empty_effect_digest,
    };
    let (derived, statement_digest) = match statement {
        KagemushaOrdinaryLineageStatementV1::Zero(s) if zero => (
            KagemushaNormalizedGuardStatementV1::from_bootstrap_state(s, context)
                .map_err(|e| e.to_string())?,
            s.proof_statement_digest().map_err(|e| e.to_string())?,
        ),
        KagemushaOrdinaryLineageStatementV1::Outgoing(s) if !zero => (
            KagemushaNormalizedGuardStatementV1::derive_from_transition(s, context)
                .map_err(|e| e.to_string())?,
            s.digest().map_err(|e| e.to_string())?,
        ),
        _ => return reject(),
    };
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
        prepared,
        authorization,
        challenge.operation_id,
        None,
    )?;
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

fn operation_tag(operation: KagemushaOperationV1) -> Result<u8> {
    match operation {
        KagemushaOperationV1::Bootstrap => Ok(0),
        KagemushaOperationV1::SendSplit => Ok(2),
        KagemushaOperationV1::RedeemSplit => Ok(4),
        _ => reject(),
    }
}
#[derive(Clone, Copy)]
struct IncomingStateSourceMetadata {
    semantic: DigestV1,
    proof_binding: DigestV1,
}
fn require_state_metadata(
    p: &KagemushaOrdinaryLineageStateProjectionV1,
    proof: &KagemushaPairedProofV1,
    m: &super::state_checkpoint::KagemushaStateCheckpointVerifierMaterialV1<'_>,
    gm: &ordinary_guard_verifier::OrdinaryGuardMaterialV1<'_>,
    n: &KagemushaNormalizedGuardStatementV1,
    guard_digest: DigestV1,
    statement_digest: DigestV1,
    prepared: Option<&KagemushaOrdinaryPreparedOutgoingV1>,
    authorization: DigestV1,
    preparation_operation: DigestV1,
    incoming: Option<IncomingStateSourceMetadata>,
) -> Result<()> {
    let (reserved_eq, reserved_ep) = (
        m.binding.outer_eq_protocol_digest,
        m.binding.outer_ep_protocol_digest,
    );
    let a = m.artifacts;
    if proof.eq_protocol_digest != m.binding.outer_eq_protocol_digest
        || proof.ep_protocol_digest != m.binding.outer_ep_protocol_digest
        || proof.guard_eq_credential_audit != reserved_eq
        || proof.guard_ep_credential_audit != reserved_ep
        || p.integer(s::OPERATION)?
            != u128::from(match incoming {
                Some(_) if n.operation == KagemushaOperationV1::MintFold => 1,
                Some(_) => return reject(),
                None => operation_tag(n.operation)?,
            })
        || p.integer(s::AMOUNT)? != n.amount
        || p.integer(s::PROTOCOL_VERSION)? != 1
        || p.integer(s::ASSET_SCALE)? != u128::from(n.asset_scale)
        || p.integer(s::POLICY_EPOCH)? != u128::from(n.policy_epoch)
    {
        return reject();
    }
    for (at, expected) in [
        (s::TRANSPORT_LO, proof.semantic_digest),
        (s::GUARD_LO, guard_digest),
        (s::PREDECESSOR_OUTER_LO, n.predecessor_state_commitment),
        (s::SUCCESSOR_OUTER_LO, n.successor_state_commitment),
        (s::RELEASE_LO, m.binding.release_id),
        (s::LIABILITY_POOL_LO, n.liability_pool_id),
        (s::PEER_CREDIT_LO, n.peer_credit_id),
        (
            s::RECIPIENT_ENCRYPTION_KEY_LO,
            n.recipient_encryption_key_binding,
        ),
        (s::EQ_PROTOCOL_LO, proof.eq_protocol_digest),
        (s::EP_PROTOCOL_LO, proof.ep_protocol_digest),
        (s::GUARD_EQ_PROTOCOL_LO, gm.eq_protocol_digest),
        (s::GUARD_EP_PROTOCOL_LO, gm.ep_protocol_digest),
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
        (s::GUARD_EQ_CREDENTIAL_AUDIT_LO, reserved_eq),
        (s::GUARD_EP_CREDENTIAL_AUDIT_LO, reserved_ep),
        (s::EQ_DEFERRED_AUDIT_LO, proof.eq_deferred_audit),
        (s::EP_DEFERRED_AUDIT_LO, proof.ep_deferred_audit),
        (
            s::MINT_SEMANTIC_LO,
            incoming.map_or([0; 32], |v| v.semantic),
        ),
        (
            s::MINT_PROOF_BINDING_LO,
            incoming.map_or([0; 32], |v| v.proof_binding),
        ),
        (s::RECEIVE_CREDIT_BINDING_LO, [0; 32]),
        (s::LIFECYCLE_LO, n.lifecycle_binding_digest),
        (
            s::PREPARED_TRANSITION_LO,
            n.prepared_transition_binding_digest,
        ),
        (s::PREDECESSOR_SUITE_LO, n.predecessor_suite_id),
        (s::PREDECESSOR_VK_LO, n.predecessor_vk_digest),
        (s::SUCCESSOR_SUITE_LO, n.successor_suite_id),
        (s::SUCCESSOR_VK_LO, n.successor_vk_digest),
        (s::ASSET_INCARNATION_LO, *n.asset_incarnation.as_bytes()),
        (s::HARDWARE_PROFILE_LO, n.hardware_profile_id),
        (s::NETWORK_LO, n.network_id),
        (s::ASSET_LO, n.asset_id),
        (s::TRANSITION_STATEMENT_LO, statement_digest),
    ] {
        p.require_digest(at, expected)?;
    }
    match prepared {
        Some(v) => {
            v.validate_shape()?;
            let transition = KagemushaOrdinaryPreparedTransitionV1 {
                version: 1,
                operation: operation_tag(n.operation)?,
                lifecycle_digest: n.lifecycle_binding_digest,
                request_digest: v.request_digest,
                predecessor_state: n.predecessor_state_commitment,
                successor_state: n.successor_state_commitment,
                amount: n.amount,
                reservation_digest: v.reservation_digest,
                native_preparation_operation_id: preparation_operation,
            };
            v.validate_against_transition(&transition)?;
            if v.operation != operation_tag(n.operation)?
                || v.predecessor_state != n.predecessor_state_commitment
                || v.successor_state != n.successor_state_commitment
                || v.prepared_transition_binding_digest != n.prepared_transition_binding_digest
                || v.lifecycle_binding_digest != n.lifecycle_binding_digest
                || v.transition_digest != statement_digest
                || v.preparation_guard_digest != guard_digest
                || v.preparation_authorization_digest != authorization
                || v.projection_semantic_digest != proof.semantic_digest
                || v.artifact_manifest_digest
                    != if v.operation == 4 {
                        m.binding.artifact_manifest_digest
                    } else {
                        [0; 32]
                    }
            {
                return reject();
            }
            p.require_digest(s::PREPARATION_ID_LO, v.binding_digest()?)?;
            p.require_digest(s::SEALED_TRANSITION_INPUTS_LO, v.stream_digests[0])?;
            p.require_digest(s::SEALED_RECOVERY_SEEDS_LO, v.stream_digests[1])?;
        }
        None => {
            for at in [
                s::PREPARATION_ID_LO,
                s::SEALED_TRANSITION_INPUTS_LO,
                s::SEALED_RECOVERY_SEEDS_LO,
            ] {
                p.require_digest(at, [0; 32])?;
            }
        }
    }
    Ok(())
}
fn decode_state_proof(raw: &[u8]) -> Result<KagemushaPairedProofV1> {
    if raw.is_empty() || raw.len() > KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1 {
        return reject();
    }
    let p: KagemushaPairedProofV1 =
        norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
            .map_err(|e| e.to_string())?;
    if norito::encode_canonical(&p).map_err(|e| e.to_string())? != raw {
        return reject();
    }
    p.validate_shape_for_semantic_digest(p.semantic_digest)
        .map_err(|e| e.to_string())?;
    Ok(p)
}
fn verify_state_histories(
    m: &super::state_checkpoint::KagemushaStateCheckpointVerifierMaterialV1<'_>,
    p: &KagemushaPairedProofV1,
    eq: &[Fp],
    ep: &[Fq],
) -> Result<()> {
    if m.outer_eq_protocol.num_instance != [CELLS + 34]
        || m.outer_ep_protocol.num_instance != [CELLS + 34]
        || p.eq_proof.len() != ordinary_ipa_proof_profile_v1(m.outer_eq_protocol)?.byte_len
        || p.ep_proof.len() != ordinary_ipa_proof_profile_v1(m.outer_ep_protocol)?.byte_len
    {
        return reject();
    }
    let eq_current =
        verify_eq_succinct_protocol(m.eq_parameters, m.outer_eq_protocol, &p.eq_proof, eq)?;
    let ep_current =
        verify_ep_succinct_protocol(m.ep_parameters, m.outer_ep_protocol, &p.ep_proof, ep)?;
    for a in [
        KagemushaEqAccumulatorV1::from_native(&eq_current).map_err(|e| e.to_string())?,
        KagemushaEqAccumulatorV1::try_from_bytes(&p.eq_history).map_err(|e| e.to_string())?,
    ] {
        decide_kagemusha_eq_accumulator_v1(m.eq_parameters, &a).map_err(|e| e.to_string())?;
    }
    for a in [
        KagemushaEpAccumulatorV1::from_native(&ep_current).map_err(|e| e.to_string())?,
        KagemushaEpAccumulatorV1::try_from_bytes(&p.ep_history).map_err(|e| e.to_string())?,
    ] {
        decide_kagemusha_ep_accumulator_v1(m.ep_parameters, &a).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn append_history<F: crate::kagemusha_v1_poseidon::KagemushaPoseidonFieldV1>(
    out: &mut Vec<F>,
    history: &[u8],
) -> Result<()> {
    if out.len() != CELLS || history.len() != KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1 {
        return reject();
    }
    out.extend(
        history
            .chunks_exact(16)
            .map(|b| from_u128::<F>(u128::from_le_bytes(b.try_into().expect("fixed limb")))),
    );
    Ok(())
}
fn digest_at(column: &Column, low: usize) -> Result<DigestV1> {
    if low + 1 >= CELLS
        || column[low][16..]
            .iter()
            .chain(&column[low + 1][16..])
            .any(|b| *b != 0)
    {
        return reject();
    }
    let mut digest = [0; 32];
    digest[..16].copy_from_slice(&column[low][..16]);
    digest[16..].copy_from_slice(&column[low + 1][..16]);
    Ok(digest)
}
fn rejection() -> String {
    "ordinary stateless lineage proof binding rejected".into()
}
fn reject<T>() -> Result<T> {
    Err(rejection())
}

/// Private data-coordinate check; financial possession is a separate C/State proof equation.
/// A hiding State nonce is never equated to the financial authority commitment.
fn require_ordinary_state_nonces(before: DigestV1, after: DigestV1, zero: bool) -> Result<()> {
    if after == [0; 32]
        || (zero && before != [0; 32])
        || (!zero && (before == [0; 32] || before == after))
    {
        return reject();
    }
    Ok(())
}
#[cfg(test)]
mod ordinary_nonce_tests {
    use super::*;
    #[test]
    fn hiding_state_nonce_does_not_alias_financial_authority_or_repeat_predecessor() {
        // Pure public coordinates, no Native owner or accepted State/proof fixture.
        let financial_authority_commitment = [11; 32];
        let initial_nonce = [12; 32];
        let next_nonce = [13; 32];
        assert_ne!(initial_nonce, financial_authority_commitment);
        assert_ne!(next_nonce, financial_authority_commitment);
        require_ordinary_state_nonces([0; 32], initial_nonce, true).unwrap();
        require_ordinary_state_nonces(initial_nonce, next_nonce, false).unwrap();
        assert!(require_ordinary_state_nonces([0; 32], next_nonce, false).is_err());
        assert!(require_ordinary_state_nonces(initial_nonce, next_nonce, true).is_err());
        assert!(require_ordinary_state_nonces(initial_nonce, initial_nonce, false).is_err());
        assert!(require_ordinary_state_nonces(initial_nonce, [0; 32], false).is_err());
    }
}
