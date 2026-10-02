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
    ordinary_state_reserved::kagemusha_ordinary_state_reserved_guard_positions_v1,
    state_relation::{RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT, public_instance as s},
    terminal_authorization::canonical_terminal_authorization_candidate_digest_v1,
};
use crate::kagemusha_v1_poseidon::{decode, from_u128};
use ff::PrimeField;
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

#[path = "ordinary_lineage_bundle_admission.rs"]
mod bundle_admission;
pub use bundle_admission::{
    KagemushaOrdinaryCashOutgoingOriginalV1, KagemushaOrdinaryLineageCommitProofBundleV1,
    KagemushaOrdinaryLineageOutgoingOriginalsV1, KagemushaOrdinaryLineageStateProofBundleV1,
    KagemushaOrdinaryLineageStatementOriginalV1, KagemushaVerifiedOrdinaryLineageAnchorProofV1,
    KagemushaVerifiedOrdinaryLineageCommitProofV1,
    KagemushaVerifiedOrdinaryLineageReservationProofV1, verify_ordinary_lineage_anchor_v1,
    verify_ordinary_lineage_commit_v1, verify_ordinary_lineage_reservation_v1,
};

pub(crate) use bundle_admission::{
    KagemushaVerifiedOrdinaryReceivedCashOutputV1, verify_ordinary_received_cash_output_v1,
};

const CELLS: usize = RECURSIVE_SEMANTIC_PUBLIC_INSTANCE_COUNT;
const PROJECTION_MAX: usize = 16 * 1024;
type Column = [[u8; 32]; CELLS];
type Result<T> = core::result::Result<T, String>;

/// Public, data-only State projection. Each parity has exactly93 canonical field encodings.
/// Only cells30/31 are parity-native full field elements; all others are shared128-bit cells.
/// This record has no constructor that admits a proof or grants a Native financial capability.
#[derive(Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::OrdinaryLineageStateProjectionV1",
    frame = "iroha.kagemusha.core.v1.ordinary-lineage-state-projection"
)]
pub struct KagemushaOrdinaryLineageStateProjectionV1 {
    version: u16,
    eq: Column,
    ep: Column,
}
impl KagemushaOrdinaryLineageStateProjectionV1 {
    /// Encode the actual already-admitted Native candidate's public projection, without secrets.
    pub(crate) fn from_admitted_candidate(
        candidate: &super::KagemushaAuthenticatedOrdinaryCashCandidateV1,
    ) -> Result<Self> {
        let p = candidate.public_inputs();
        Self::from_fields(
            p.recursive_semantic_public_instances::<Fp>()?,
            p.recursive_semantic_public_instances::<Fq>()?,
        )
    }
    fn from_fields(eq: Vec<Fp>, ep: Vec<Fq>) -> Result<Self> {
        if eq.len() != CELLS || ep.len() != CELLS {
            return reject();
        }
        let value = Self {
            version: 1,
            eq: core::array::from_fn(|i| eq[i].to_repr()),
            ep: core::array::from_fn(|i| ep[i].to_repr()),
        };
        value.fields()?;
        Ok(value)
    }
    /// Strict bounded decoder for public data; it performs no proof or authority admission.
    /// # Errors
    /// Refuses oversized, noncanonical, substituted parity or scalar encodings.
    pub fn decode_original(original: &[u8]) -> Result<Self> {
        if original.is_empty() || original.len() > PROJECTION_MAX {
            return reject();
        }
        let value: Self = norito::decode_canonical_with_limits(
            original,
            norito::canonical_decode_limits(original.len()),
        )
        .map_err(|e| e.to_string())?;
        if value.canonical_bytes()? != original {
            return reject();
        }
        Ok(value)
    }
    /// Encode the sole bounded public data original; this does not authenticate the projection.
    /// # Errors
    /// Refuses invalid scalar/pair shape or a canonical codec failure.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.fields()?;
        let raw = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if raw.len() > PROJECTION_MAX {
            return reject();
        }
        Ok(raw)
    }
    fn fields(&self) -> Result<(Vec<Fp>, Vec<Fq>)> {
        if self.version != 1 {
            return reject();
        }
        let eq = self
            .eq
            .iter()
            .map(|b| decode::<Fp>(*b).ok_or_else(rejection))
            .collect::<Result<Vec<_>>>()?;
        let ep = self
            .ep
            .iter()
            .map(|b| decode::<Fq>(*b).ok_or_else(rejection))
            .collect::<Result<Vec<_>>>()?;
        for i in 0..CELLS {
            if matches!(i, s::PREDECESSOR_STATE | s::SUCCESSOR_STATE) {
                continue;
            }
            if self.eq[i] != self.ep[i] || self.eq[i][16..].iter().any(|b| *b != 0) {
                return reject();
            }
        }
        for (column, before, after) in [
            (
                &self.eq,
                s::PREDECESSOR_EQ_COMPONENT_LO,
                s::SUCCESSOR_EQ_COMPONENT_LO,
            ),
            (
                &self.ep,
                s::PREDECESSOR_EP_COMPONENT_LO,
                s::SUCCESSOR_EP_COMPONENT_LO,
            ),
        ] {
            if digest_at(column, before)? != column[s::PREDECESSOR_STATE]
                || digest_at(column, after)? != column[s::SUCCESSOR_STATE]
            {
                return reject();
            }
        }
        Ok((eq, ep))
    }
    fn digest(&self, low: usize) -> Result<DigestV1> {
        digest_at(&self.eq, low)
    }
    fn integer(&self, at: usize) -> Result<u128> {
        if at >= CELLS || self.eq[at][16..].iter().any(|b| *b != 0) {
            return reject();
        }
        Ok(u128::from_le_bytes(
            self.eq[at][..16].try_into().map_err(|_| rejection())?,
        ))
    }
    fn require_digest(&self, low: usize, expected: DigestV1) -> Result<()> {
        if self.digest(low)? != expected {
            return reject();
        }
        Ok(())
    }
}

/// Full public paired State original: exact semantic projection and both actual outer proofs/history.
/// This is an untrusted canonical data carrier, never a decoded Native owner or DATA authority.
#[derive(Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::OrdinaryLineageStateOriginalV1",
    frame = "iroha.kagemusha.core.v1.ordinary-lineage-state-original"
)]
pub struct KagemushaOrdinaryLineageStateOriginalV1 {
    version: u16,
    projection: KagemushaOrdinaryLineageStateProjectionV1,
    proof: KagemushaPairedProofV1,
}
impl KagemushaOrdinaryLineageStateOriginalV1 {
    /// Data-only full original from the zero selection's already-verified public instance.
    /// The caller retains the actual Bootstrap owner; this does not convert its W into money.
    pub(crate) fn from_bootstrap_public_inputs(
        inputs: &super::KagemushaStateRelationPublicInputsV1,
        proof: &KagemushaPairedProofV1,
    ) -> Result<Self> {
        let projection = KagemushaOrdinaryLineageStateProjectionV1::from_fields(
            inputs.recursive_semantic_public_instances::<Fp>()?,
            inputs.recursive_semantic_public_instances::<Fq>()?,
        )?;
        let value = Self {
            version: 1,
            projection,
            proof: proof.clone(),
        };
        value.canonical_bytes()?;
        Ok(value)
    }

    /// Copy public data from the genuine already-admitted Native candidate, granting no new loan.
    pub(crate) fn from_admitted_candidate(
        candidate: &super::KagemushaAuthenticatedOrdinaryCashCandidateV1,
    ) -> Result<Self> {
        let value = Self {
            version: 1,
            projection: KagemushaOrdinaryLineageStateProjectionV1::from_admitted_candidate(
                candidate,
            )?,
            proof: candidate.proof().clone(),
        };
        value.canonical_bytes()?;
        Ok(value)
    }
    /// Sole bounded complete public original, including exact current proofs and full histories.
    /// # Errors
    /// Refuses malformed version/scalars/proof envelope or noncanonical encoding.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        if self.version != 1 {
            return reject();
        }
        self.projection.fields()?;
        self.proof
            .validate_shape_for_semantic_digest(self.proof.semantic_digest)
            .map_err(|e| e.to_string())?;
        let raw = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if raw.len() > 32 * 1024 {
            return reject();
        }
        Ok(raw)
    }
    /// Strict bounded data decoder; callers must still admit all actual proofs and original joins.
    /// # Errors
    /// Refuses oversized, noncanonical or malformed public originals.
    pub fn decode_original(raw: &[u8]) -> Result<Self> {
        if raw.is_empty() || raw.len() > 32 * 1024 {
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
}

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
        || normalized.successor_hardware_policy_id != credential.static_binding_digest()
        || normalized.successor_state_nonce_commitment != c.financial_authority_commitment
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
                || normalized.predecessor_hardware_policy_id != credential.static_binding_digest()
                || normalized.predecessor_state_nonce_commitment
                    != c.financial_authority_commitment
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
) -> Result<()> {
    let (reserved_eq, reserved_ep) = kagemusha_ordinary_state_reserved_guard_positions_v1();
    let a = m.artifacts;
    if proof.eq_protocol_digest != m.binding.outer_eq_protocol_digest
        || proof.ep_protocol_digest != m.binding.outer_ep_protocol_digest
        || proof.guard_eq_credential_audit != reserved_eq
        || proof.guard_ep_credential_audit != reserved_ep
        || p.integer(s::OPERATION)? != u128::from(operation_tag(n.operation)?)
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
        (s::MINT_SEMANTIC_LO, [0; 32]),
        (s::MINT_PROOF_BINDING_LO, [0; 32]),
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

#[cfg(test)]
mod tests {
    use super::*;
    use ff::Field as _;
    #[test]
    fn public_projection_rejects_noncanonical_shared_and_parity_component_cells() {
        let value = KagemushaOrdinaryLineageStateProjectionV1::from_fields(
            vec![Fp::ZERO; CELLS],
            vec![Fq::ZERO; CELLS],
        )
        .unwrap();
        let raw = value.canonical_bytes().unwrap();
        assert!(KagemushaOrdinaryLineageStateProjectionV1::decode_original(&raw).is_ok());
        for change in 0..5 {
            let mut bad = value.clone();
            match change {
                0 => bad.version = 2,
                1 => bad.ep[4][0] = 1,
                2 => {
                    bad.eq[4][16] = 1;
                    bad.ep[4][16] = 1;
                }
                3 => bad.eq[s::PREDECESSOR_STATE][0] = 1,
                _ => bad.ep[s::SUCCESSOR_STATE] = [0xff; 32],
            }
            assert!(bad.fields().is_err());
        }
        let mut trailing = raw;
        trailing.push(0);
        assert!(KagemushaOrdinaryLineageStateProjectionV1::decode_original(&trailing).is_err());
        assert!(
            KagemushaOrdinaryLineageStateProjectionV1::decode_original(&vec![
                0;
                PROJECTION_MAX + 1
            ])
            .is_err()
        );
    }
}
