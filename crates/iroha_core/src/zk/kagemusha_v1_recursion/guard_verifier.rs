//! Release-authenticated diagnostics for complete monetary GuardBundle proofs.
//!
//! Both current proofs and their complete credential/SHA histories are terminally decided. The
//! caller supplies Core's exact normalized statement; decoded archive fields never select that
//! statement or a verifying key. Each parity also authenticates the exact predecessor/successor
//! credential digests in its 44-cell public column. The complete provider/credential/State/terminal
//! authority corridor remains unqualified, so these diagnostic checks cannot authorize Bootstrap
//! or monetary transitions and production construction remains fail-closed.
//! Independent mint reservation/staging, peer staging and recovery anchors also require their own
//! qualified hardware transaction evidence.

use std::sync::Arc;

use ff::PrimeField;
use halo2_proofs::{
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    poly::ipa::commitment::ParamsIPA,
};
use norito::{
    DecodeLimits,
    codec::{Decode, Encode},
};
use snark_verifier::verifier::plonk::PlonkProtocol;
use thiserror::Error;

use super::{
    DigestV1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KagemushaAuthenticatedRecursiveVerifierV1,
    KagemushaEpAccumulatorV1, KagemushaEqAccumulatorV1, KagemushaGuardContextV1,
    KagemushaNormalizedGuardStatementV1, KagemushaOperationV1, decide_kagemusha_ep_accumulator_v1,
    decide_kagemusha_eq_accumulator_v1,
    deferred_parent::ordinary_ipa_proof_profile_v1,
    guard_bundle::{
        GUARD_HISTORY_OFFSET_V1, GUARD_PREDECESSOR_CREDENTIAL_OFFSET_V1,
        GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1, GUARD_SUCCESSOR_CREDENTIAL_OFFSET_V1,
    },
    native_backend::{verify_ep_succinct_protocol, verify_eq_succinct_protocol},
};
use crate::zk::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, digest_limbs, from_u128},
    kagemusha_v1_state::{
        BootstrapStatementV1, CreditStageStatementV1, DurabilityAnchorStatementV1,
        HardwareTransitionStatementV1, KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1,
        KagemushaGuardBundleVerifierV1, MintReservationStatementV1, MintStageStatementV1,
        TransitionProofStatementV1,
    },
};

type HistoryBytes = [u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1];
type Result<T> = core::result::Result<T, KagemushaGuardVerificationErrorV1>;

/// Closed failures at the authenticated native GuardBundle boundary.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum KagemushaGuardVerificationErrorV1 {
    /// Invalid lengths, noncanonical fields, or malformed fixed histories.
    #[error("invalid native GuardBundle shape or resource bound")]
    Shape,
    /// A frame has another schema, trailing bytes or a noncanonical encoding.
    #[error("noncanonical native GuardBundle encoding")]
    Encoding,
    /// Proof data differs from the authenticated release or caller-derived Core statement.
    #[error("native GuardBundle release or statement binding mismatch")]
    Binding,
    /// A retained proof or complete history failed native verification.
    #[error("native GuardBundle proof rejected: {0}")]
    Proof(String),
    /// The complete provider-policy authority corridor has not been qualified for runtime use.
    #[error(
        "qualified provider-policy authority is unavailable for monetary GuardBundle verification"
    )]
    ProviderPolicyAuthorityUnavailable,
    /// This monetary proof relation cannot certify the named hardware transaction.
    #[error("qualified hardware transaction verifier is unavailable for {0}")]
    HardwareTransactionUnavailable(&'static str),
}

#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub(super) struct KagemushaGuardVerifierBindingV1 {
    pub(super) release_id: DigestV1,
    pub(super) suite_id: DigestV1,
    pub(super) vk_set_digest: DigestV1,
    pub(super) artifact_manifest_digest: DigestV1,
    pub(super) eq_protocol_digest: DigestV1,
    pub(super) ep_protocol_digest: DigestV1,
}

/// Only the authenticated loader can supply the immutable protocols used by the public factory.
pub(super) struct KagemushaGuardVerifierMaterialV1<'a> {
    pub(super) eq_parameters: &'a ParamsIPA<EqAffine>,
    pub(super) ep_parameters: &'a ParamsIPA<EpAffine>,
    pub(super) eq_protocol: &'a PlonkProtocol<EqAffine>,
    pub(super) ep_protocol: &'a PlonkProtocol<EpAffine>,
    pub(super) binding: KagemushaGuardVerifierBindingV1,
    pub(super) canonical_empty_effect_digest: DigestV1,
    pub(super) provider_policy_root: DigestV1,
}

#[derive(Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_recursion::guard_verifier::GuardProofWire",
    frame = "iroha.kagemusha.core.v1.monetary-guard-proof"
)]
struct GuardProofWire {
    version: u16,
    binding: KagemushaGuardVerifierBindingV1,
    statement_digest: DigestV1,
    eq_credential_audit: DigestV1,
    ep_credential_audit: DigestV1,
    credential_digests: [DigestV1; 2],
    eq_proof: Vec<u8>,
    ep_proof: Vec<u8>,
    eq_history: HistoryBytes,
    ep_history: HistoryBytes,
}

/// Borrowed outputs of the real paired GuardBundle prover.
///
/// These fields are untrusted proof material. [`KagemushaGuardProofDiagnosticVerifierV1::encode_proof`]
/// checks the cryptographic relation before emitting a frame; it grants no hardware authority.
pub struct KagemushaGuardBundleProofPartsV1<'a> {
    /// Canonical Eq audit shared by both proof parities.
    pub eq_credential_audit: DigestV1,
    /// Canonical Ep audit shared by both proof parities.
    pub ep_credential_audit: DigestV1,
    /// Exact predecessor/successor credential statement digests exposed by both actual proofs.
    pub credential_digests: [DigestV1; 2],
    /// Actual Eq GuardBundle ordinary IPA proof.
    pub eq_proof: &'a [u8],
    /// Actual Ep GuardBundle ordinary IPA proof.
    pub ep_proof: &'a [u8],
    /// Complete Eq history, including both credential proofs and their SHA histories.
    pub eq_history: &'a KagemushaEqAccumulatorV1,
    /// Complete Ep history, including both credential proofs and their SHA histories.
    pub ep_history: &'a KagemushaEpAccumulatorV1,
}

/// Unavailable production monetary Guard verifier awaiting authenticated provider-policy authority.
///
/// Release-authenticated proof keys alone do not qualify the complete provider/credential/State/
/// terminal authority corridor. Construction and every monetary acceptance method reject. No arbitrary
/// root, caller boolean, or successful diagnostic proof check can create this capability.
#[derive(Clone, Copy)]
pub struct KagemushaAuthenticatedGuardBundleVerifierV1 {
    _private: (),
}

impl KagemushaAuthenticatedGuardBundleVerifierV1 {
    /// Reject construction until the complete provider authority corridor is qualified.
    ///
    /// # Errors
    /// Always returns [`KagemushaGuardVerificationErrorV1::ProviderPolicyAuthorityUnavailable`].
    pub fn new(_verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>) -> Result<Self> {
        // TODO: Require a nonserializable qualification capability covering the complete actual
        // provider/credential/State/terminal chain before introducing an accepting constructor.
        Err(KagemushaGuardVerificationErrorV1::ProviderPolicyAuthorityUnavailable)
    }
}

/// Non-authorizing proof checks for the actual paired Guard prover and qualification tooling.
///
/// This type deliberately does not implement [`KagemushaGuardBundleVerifierV1`]. Passing its
/// cryptographic checks proves the circuit relation under the release's configured provider root;
/// it does not establish qualified hardware operation or permission to spend.
#[derive(Clone)]
pub struct KagemushaGuardProofDiagnosticVerifierV1 {
    verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    lengths: GuardProofLengths,
}

#[derive(Clone, Copy)]
struct GuardProofLengths {
    eq: usize,
    ep: usize,
}

impl GuardProofLengths {
    fn from_material(material: &KagemushaGuardVerifierMaterialV1<'_>) -> Result<Self> {
        if material.eq_protocol.num_instance != [GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1]
            || material.ep_protocol.num_instance != [GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1]
        {
            return Err(KagemushaGuardVerificationErrorV1::Shape);
        }
        let lengths = Self {
            eq: ordinary_ipa_proof_profile_v1(material.eq_protocol)
                .map_err(KagemushaGuardVerificationErrorV1::Proof)?
                .byte_len,
            ep: ordinary_ipa_proof_profile_v1(material.ep_protocol)
                .map_err(KagemushaGuardVerificationErrorV1::Proof)?
                .byte_len,
        };
        lengths.limits()?;
        Ok(lengths)
    }

    fn limits(self) -> Result<DecodeLimits> {
        let maximum = KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1;
        if self.eq == 0
            || self.ep == 0
            || self.eq.checked_add(self.ep).is_none_or(|sum| sum > maximum)
        {
            return Err(KagemushaGuardVerificationErrorV1::Shape);
        }
        Ok(DecodeLimits::new(
            self.eq
                .max(self.ep)
                .max(KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1),
            maximum,
            maximum * 4,
            maximum * 4,
            16,
        ))
    }
}

impl KagemushaGuardProofDiagnosticVerifierV1 {
    /// Bind non-authorizing diagnostics to keys already admitted by the release loader.
    ///
    /// # Errors
    /// Rejects an incompatible public shape or a proof inventory exceeding Core's Guard budget.
    pub fn new(verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>) -> Result<Self> {
        let lengths = GuardProofLengths::from_material(&verifier.guard_verifier_material())?;
        Ok(Self { verifier, lengths })
    }

    /// Verify genuine prover output and encode its bounded canonical private Guard frame.
    ///
    /// This frame contains no secret authority or serialized capability. Successful encoding
    /// does not qualify provider hardware or authorize a monetary operation.
    ///
    /// # Errors
    /// Rejects incorrect statement/release bindings, malformed proofs or failed decisions.
    pub fn encode_proof(
        &self,
        normalized: &KagemushaNormalizedGuardStatementV1,
        parts: KagemushaGuardBundleProofPartsV1<'_>,
    ) -> Result<Vec<u8>> {
        if parts.eq_proof.len() != self.lengths.eq || parts.ep_proof.len() != self.lengths.ep {
            return Err(KagemushaGuardVerificationErrorV1::Shape);
        }
        let material = self.verifier.guard_verifier_material();
        let proof = GuardProofWire {
            version: 1,
            binding: material.binding.clone(),
            statement_digest: normalized
                .canonical_digest()
                .map_err(|_| KagemushaGuardVerificationErrorV1::Binding)?,
            eq_credential_audit: parts.eq_credential_audit,
            ep_credential_audit: parts.ep_credential_audit,
            credential_digests: parts.credential_digests,
            eq_proof: parts.eq_proof.to_vec(),
            ep_proof: parts.ep_proof.to_vec(),
            eq_history: *parts.eq_history.as_bytes(),
            ep_history: *parts.ep_history.as_bytes(),
        };
        self.verify_wire(normalized, parts.credential_digests, &proof)?;
        let bytes = norito::encode_canonical(&proof)
            .map_err(|_| KagemushaGuardVerificationErrorV1::Encoding)?;
        if bytes.len() > KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1 {
            return Err(KagemushaGuardVerificationErrorV1::Shape);
        }
        Ok(bytes)
    }

    /// Check the caller's exact normalized statement, credential pair and complete histories.
    ///
    /// The supplied digests must identify the caller's expected predecessor/successor credentials.
    /// Both are verified as actual proof instances, not accepted as detached archive annotations.
    /// This diagnostic method grants no monetary authority.
    ///
    /// # Errors
    /// Rejects noncanonical archives, substitutions, or invalid actual proofs/accumulators.
    pub fn verify_normalized(
        &self,
        normalized: &KagemushaNormalizedGuardStatementV1,
        expected_credential_digests: [DigestV1; 2],
        bytes: &[u8],
    ) -> Result<()> {
        let wire = decode_wire(bytes, self.lengths)?;
        self.verify_wire(normalized, expected_credential_digests, &wire)
    }

    /// Check Core's bootstrap projection and exact credential pair without granting hardware authority.
    ///
    /// # Errors
    /// Rejects mismatched Core context, malformed frames, or invalid cryptographic proofs.
    pub fn verify_bootstrap_proof(
        &self,
        statement: &BootstrapStatementV1,
        normalized: &KagemushaNormalizedGuardStatementV1,
        expected_credential_digests: [DigestV1; 2],
        bytes: &[u8],
    ) -> Result<()> {
        validate_bootstrap_binding(
            statement,
            normalized,
            self.verifier
                .guard_verifier_material()
                .canonical_empty_effect_digest,
        )?;
        self.verify_normalized(normalized, expected_credential_digests, bytes)
    }

    /// Check Core's transition projection and exact credential pair without granting spend authority.
    ///
    /// # Errors
    /// Rejects mismatched Core/hardware context, malformed frames, or invalid proofs.
    pub fn verify_transition_proof(
        &self,
        statement: &HardwareTransitionStatementV1,
        proof_statement: &TransitionProofStatementV1,
        normalized: &KagemushaNormalizedGuardStatementV1,
        expected_credential_digests: [DigestV1; 2],
        bytes: &[u8],
    ) -> Result<()> {
        validate_transition_binding(
            statement,
            proof_statement,
            normalized,
            self.verifier
                .guard_verifier_material()
                .canonical_empty_effect_digest,
        )?;
        self.verify_normalized(normalized, expected_credential_digests, bytes)
    }

    fn verify_wire(
        &self,
        normalized: &KagemushaNormalizedGuardStatementV1,
        expected_credential_digests: [DigestV1; 2],
        wire: &GuardProofWire,
    ) -> Result<()> {
        let material = self.verifier.guard_verifier_material();
        validate_normalized_provider_policy_root_v1(normalized, material.provider_policy_root)?;
        validate_normalized_release(
            normalized,
            &material.binding,
            material.canonical_empty_effect_digest,
        )?;
        validate_wire(
            wire,
            &material.binding,
            normalized
                .canonical_digest()
                .map_err(|_| KagemushaGuardVerificationErrorV1::Binding)?,
            expected_credential_digests,
            self.lengths,
        )?;
        let eq_instances = public_column::<Fp>(
            wire.statement_digest,
            wire.eq_credential_audit,
            wire.ep_credential_audit,
            wire.credential_digests,
            &wire.eq_history,
        );
        let ep_instances = public_column::<Fq>(
            wire.statement_digest,
            wire.eq_credential_audit,
            wire.ep_credential_audit,
            wire.credential_digests,
            &wire.ep_history,
        );
        let eq_current = verify_eq_succinct_protocol(
            material.eq_parameters,
            material.eq_protocol,
            &wire.eq_proof,
            &eq_instances,
        )
        .map_err(KagemushaGuardVerificationErrorV1::Proof)?;
        let ep_current = verify_ep_succinct_protocol(
            material.ep_parameters,
            material.ep_protocol,
            &wire.ep_proof,
            &ep_instances,
        )
        .map_err(KagemushaGuardVerificationErrorV1::Proof)?;
        let eq_current = KagemushaEqAccumulatorV1::from_native(&eq_current)
            .map_err(|error| KagemushaGuardVerificationErrorV1::Proof(error.to_string()))?;
        let ep_current = KagemushaEpAccumulatorV1::from_native(&ep_current)
            .map_err(|error| KagemushaGuardVerificationErrorV1::Proof(error.to_string()))?;
        let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&wire.eq_history)
            .map_err(|_| KagemushaGuardVerificationErrorV1::Shape)?;
        let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&wire.ep_history)
            .map_err(|_| KagemushaGuardVerificationErrorV1::Shape)?;
        decide_kagemusha_eq_accumulator_v1(material.eq_parameters, &eq_current)
            .and_then(|()| decide_kagemusha_eq_accumulator_v1(material.eq_parameters, &eq_history))
            .map_err(|error| KagemushaGuardVerificationErrorV1::Proof(error.to_string()))?;
        decide_kagemusha_ep_accumulator_v1(material.ep_parameters, &ep_current)
            .and_then(|()| decide_kagemusha_ep_accumulator_v1(material.ep_parameters, &ep_history))
            .map_err(|error| KagemushaGuardVerificationErrorV1::Proof(error.to_string()))
    }
}

impl KagemushaGuardBundleVerifierV1 for KagemushaAuthenticatedGuardBundleVerifierV1 {
    fn verify_bootstrap(
        &self,
        _statement: &BootstrapStatementV1,
        _normalized: &KagemushaNormalizedGuardStatementV1,
        _guard_bundle: &[u8],
    ) -> core::result::Result<(), String> {
        Err(KagemushaGuardVerificationErrorV1::ProviderPolicyAuthorityUnavailable.to_string())
    }

    fn verify_transition(
        &self,
        _statement: &HardwareTransitionStatementV1,
        _proof_statement: &TransitionProofStatementV1,
        _normalized: &KagemushaNormalizedGuardStatementV1,
        _guard_bundle: &[u8],
    ) -> core::result::Result<(), String> {
        Err(KagemushaGuardVerificationErrorV1::ProviderPolicyAuthorityUnavailable.to_string())
    }

    fn verify_mint_reservation(
        &self,
        _statement: &MintReservationStatementV1,
        _guard_bundle: &[u8],
    ) -> core::result::Result<(), String> {
        Err(unavailable("mint reservation").to_string())
    }

    fn verify_mint_stage(
        &self,
        _statement: &MintStageStatementV1,
        _guard_bundle: &[u8],
    ) -> core::result::Result<(), String> {
        Err(unavailable("mint staging").to_string())
    }

    fn verify_credit_stage(
        &self,
        _statement: &CreditStageStatementV1,
        _guard_bundle: &[u8],
    ) -> core::result::Result<(), String> {
        Err(unavailable("peer credit staging").to_string())
    }

    fn verify_durability_anchor(
        &self,
        _statement: &DurabilityAnchorStatementV1,
        _guard_bundle: &[u8],
    ) -> core::result::Result<(), String> {
        Err(unavailable("recovery anchor").to_string())
    }
}

fn unavailable(operation: &'static str) -> KagemushaGuardVerificationErrorV1 {
    KagemushaGuardVerificationErrorV1::HardwareTransactionUnavailable(operation)
}

fn validate_bootstrap_binding(
    statement: &BootstrapStatementV1,
    normalized: &KagemushaNormalizedGuardStatementV1,
    empty: DigestV1,
) -> Result<()> {
    let reconstructed = KagemushaNormalizedGuardStatementV1::from_bootstrap_state(
        statement,
        guard_context(normalized, empty),
    )
    .map_err(|_| KagemushaGuardVerificationErrorV1::Binding)?;
    if &reconstructed != normalized {
        return Err(KagemushaGuardVerificationErrorV1::Binding);
    }
    Ok(())
}

fn validate_transition_binding(
    statement: &HardwareTransitionStatementV1,
    proof: &TransitionProofStatementV1,
    normalized: &KagemushaNormalizedGuardStatementV1,
    empty: DigestV1,
) -> Result<()> {
    let reconstructed = KagemushaNormalizedGuardStatementV1::from_transition(
        proof,
        statement,
        guard_context(normalized, empty),
    )
    .map_err(|_| KagemushaGuardVerificationErrorV1::Binding)?;
    if &reconstructed != normalized {
        return Err(KagemushaGuardVerificationErrorV1::Binding);
    }
    Ok(())
}

fn guard_context(
    normalized: &KagemushaNormalizedGuardStatementV1,
    empty: DigestV1,
) -> KagemushaGuardContextV1 {
    KagemushaGuardContextV1 {
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
        canonical_empty_effect_digest: empty,
    }
}

fn validate_normalized_release(
    normalized: &KagemushaNormalizedGuardStatementV1,
    binding: &KagemushaGuardVerifierBindingV1,
    empty: DigestV1,
) -> Result<()> {
    normalized
        .canonical_digest()
        .map_err(|_| KagemushaGuardVerificationErrorV1::Binding)?;
    normalized
        .validate_release_effects(empty)
        .map_err(|_| KagemushaGuardVerificationErrorV1::Binding)?;
    let bootstrap = normalized.operation == KagemushaOperationV1::Bootstrap;
    if normalized.release_id != binding.release_id
        || normalized.successor_suite_id != binding.suite_id
        || normalized.successor_vk_digest != binding.vk_set_digest
        || (!bootstrap
            && (normalized.predecessor_release_id != binding.release_id
                || normalized.predecessor_suite_id != binding.suite_id
                || normalized.predecessor_vk_digest != binding.vk_set_digest))
    {
        return Err(KagemushaGuardVerificationErrorV1::Binding);
    }
    Ok(())
}

fn validate_normalized_provider_policy_root_v1(
    normalized: &KagemushaNormalizedGuardStatementV1,
    expected: DigestV1,
) -> Result<()> {
    let predecessor = if normalized.operation == KagemushaOperationV1::Bootstrap {
        [0; 32]
    } else {
        expected
    };
    if expected == [0; 32]
        || normalized.successor_hardware_policy_id != expected
        || normalized.predecessor_hardware_policy_id != predecessor
    {
        return Err(KagemushaGuardVerificationErrorV1::Binding);
    }
    Ok(())
}

fn decode_wire(bytes: &[u8], lengths: GuardProofLengths) -> Result<GuardProofWire> {
    if bytes.is_empty() || bytes.len() > KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1 {
        return Err(KagemushaGuardVerificationErrorV1::Shape);
    }
    norito::decode_canonical_with_limits(bytes, lengths.limits()?)
        .map_err(|_| KagemushaGuardVerificationErrorV1::Encoding)
}

fn validate_wire(
    wire: &GuardProofWire,
    binding: &KagemushaGuardVerifierBindingV1,
    digest: DigestV1,
    expected_credential_digests: [DigestV1; 2],
    lengths: GuardProofLengths,
) -> Result<()> {
    if wire.version != 1 || wire.eq_proof.len() != lengths.eq || wire.ep_proof.len() != lengths.ep {
        return Err(KagemushaGuardVerificationErrorV1::Shape);
    }
    if &wire.binding != binding
        || wire.statement_digest != digest
        || wire.credential_digests != expected_credential_digests
    {
        return Err(KagemushaGuardVerificationErrorV1::Binding);
    }
    if wire.statement_digest == [0; 32]
        || wire.eq_credential_audit == [0; 32]
        || wire.ep_credential_audit == [0; 32]
        || wire.eq_credential_audit == wire.ep_credential_audit
        || wire.credential_digests.contains(&[0; 32])
        || Option::<Fp>::from(Fp::from_repr(wire.eq_credential_audit)).is_none()
        || Option::<Fq>::from(Fq::from_repr(wire.ep_credential_audit)).is_none()
    {
        return Err(KagemushaGuardVerificationErrorV1::Shape);
    }
    KagemushaEqAccumulatorV1::try_from_bytes(&wire.eq_history)
        .map_err(|_| KagemushaGuardVerificationErrorV1::Shape)?;
    KagemushaEpAccumulatorV1::try_from_bytes(&wire.ep_history)
        .map_err(|_| KagemushaGuardVerificationErrorV1::Shape)?;
    Ok(())
}

fn public_column<F: KagemushaPoseidonFieldV1>(
    digest: DigestV1,
    eq_audit: DigestV1,
    ep_audit: DigestV1,
    credential_digests: [DigestV1; 2],
    history: &HistoryBytes,
) -> Vec<F> {
    let mut column = Vec::with_capacity(GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1);
    column.extend(digest_limbs::<F>(digest));
    column.extend(digest_limbs::<F>(eq_audit));
    column.extend(digest_limbs::<F>(ep_audit));
    // Both parity verifications consume these actual public statement cells;
    // comparing detached archive metadata alone would not authenticate them.
    debug_assert_eq!(column.len(), GUARD_PREDECESSOR_CREDENTIAL_OFFSET_V1);
    column.extend(digest_limbs::<F>(credential_digests[0]));
    debug_assert_eq!(column.len(), GUARD_SUCCESSOR_CREDENTIAL_OFFSET_V1);
    column.extend(digest_limbs::<F>(credential_digests[1]));
    debug_assert_eq!(column.len(), GUARD_HISTORY_OFFSET_V1);
    for chunk in history.chunks_exact(16) {
        column.push(from_u128::<F>(u128::from_le_bytes(
            chunk.try_into().expect("fixed history chunk"),
        )));
    }
    column
}

#[cfg(test)]
#[path = "frame_identity_tests.rs"]
pub(super) mod frame_identity_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::kagemusha_v1_state::KagemushaTransitionKindV1;

    const EMPTY: DigestV1 = [0x75; 32];

    // These synthetic transcripts test shape and exact statement bindings only. Actual proof
    // acceptance must be exercised by the authenticated real-proof qualification corridor.
    fn fixture() -> (
        BootstrapStatementV1,
        KagemushaNormalizedGuardStatementV1,
        GuardProofWire,
        GuardProofLengths,
    ) {
        let (public, paired) = super::super::tests::state_verification_fixture();
        let state = public.successor;
        let bootstrap = BootstrapStatementV1 {
            version: 1,
            protocol_version: 1,
            suite_id: state.suite_id,
            vk_digest: state.vk_digest,
            release_id: state.release_id,
            asset_incarnation: state.asset_incarnation,
            liability_pool_id: state.liability_pool_id,
            hardware_profile_id: state.hardware_profile_id,
            policy_epoch: state.policy_epoch,
            lane: state.lane,
            hardware_epoch: state.hardware_epoch,
            device_policy_binding: state.device_policy_binding,
            state_nonce_commitment: state.state_nonce_commitment,
            state_commitment: state.state_commitment,
        };
        let normalized = KagemushaNormalizedGuardStatementV1::from_bootstrap_state(
            &bootstrap,
            KagemushaGuardContextV1 {
                release_id: bootstrap.release_id,
                liability_pool_id: bootstrap.liability_pool_id,
                lifecycle_binding_digest: [0x71; 32],
                prepared_transition_binding_digest: [0; 32],
                terminal_commit_binding_digest: [0; 32],
                sender_one_time_authorization_digest: [0; 32],
                receive_credit_binding_digest: [0; 32],
                transition_intent_digest: [0x72; 32],
                transition_effect_digest: [0x73; 32],
                recovery_record_digest: [0x74; 32],
                durable_inbox_effect_digest: EMPTY,
                durable_outbox_effect_digest: EMPTY,
                canonical_empty_effect_digest: EMPTY,
            },
        )
        .expect("normalized bootstrap fixture");
        let wire = GuardProofWire {
            version: 1,
            binding: KagemushaGuardVerifierBindingV1 {
                release_id: bootstrap.release_id,
                suite_id: bootstrap.suite_id,
                vk_set_digest: bootstrap.vk_digest,
                artifact_manifest_digest: [0x76; 32],
                eq_protocol_digest: Fp::from(7001).to_repr(),
                ep_protocol_digest: Fq::from(7002).to_repr(),
            },
            statement_digest: normalized.canonical_digest().expect("statement digest"),
            eq_credential_audit: Fp::from(7003).to_repr(),
            ep_credential_audit: Fq::from(7004).to_repr(),
            credential_digests: [[0xA1; 32], [0xB2; 32]],
            eq_proof: vec![1; 32],
            ep_proof: vec![2; 64],
            eq_history: paired.eq_history.as_slice().try_into().expect("Eq history"),
            ep_history: paired.ep_history.as_slice().try_into().expect("Ep history"),
        };
        (
            bootstrap,
            normalized,
            wire,
            GuardProofLengths { eq: 32, ep: 64 },
        )
    }

    fn transition_fixture() -> (
        TransitionProofStatementV1,
        HardwareTransitionStatementV1,
        KagemushaNormalizedGuardStatementV1,
    ) {
        let (bootstrap, normalized, _, _) = fixture();
        let proof = TransitionProofStatementV1 {
            version: 1,
            protocol_version: 1,
            predecessor_suite_id: bootstrap.suite_id,
            predecessor_vk_digest: bootstrap.vk_digest,
            successor_suite_id: bootstrap.suite_id,
            successor_vk_digest: bootstrap.vk_digest,
            kind: KagemushaTransitionKindV1::MintFold,
            amount: 1,
            mint_finality_semantic_digest: [0x81; 32],
            mint_finality_proof_binding_digest: [0x82; 32],
            peer_credit_id: [0; 32],
            recipient_encryption_key_binding: [0; 32],
            lifecycle_binding_digest: normalized.lifecycle_binding_digest,
            prepared_transition_binding_digest: [0; 32],
            receive_credit_binding_digest: [0; 32],
            predecessor_release_id: bootstrap.release_id,
            release_id: bootstrap.release_id,
            asset_incarnation: bootstrap.asset_incarnation,
            liability_pool_id: bootstrap.liability_pool_id,
            hardware_profile_id: bootstrap.hardware_profile_id,
            policy_epoch: bootstrap.policy_epoch,
            lane: bootstrap.lane,
            predecessor_commitment: bootstrap.state_commitment,
            successor_commitment: [0x83; 32],
            predecessor_sequence: 0,
            successor_sequence: 1,
            predecessor_epoch: bootstrap.hardware_epoch,
            successor_epoch: bootstrap.hardware_epoch,
            predecessor_device_policy_binding: bootstrap.device_policy_binding,
            successor_device_policy_binding: bootstrap.device_policy_binding,
            predecessor_state_nonce_commitment: bootstrap.state_nonce_commitment,
            successor_state_nonce_commitment: [0x84; 32],
            journal_revision_before: 0,
            journal_revision_after: 1,
            effect_digest: [0x85; 32],
        };
        let mut context = guard_context(&normalized, EMPTY);
        context.transition_effect_digest = proof.effect_digest;
        context.durable_inbox_effect_digest = [0x86; 32];
        let normalized =
            KagemushaNormalizedGuardStatementV1::derive_from_transition(&proof, context)
                .expect("normalized transition");
        let hardware = HardwareTransitionStatementV1 {
            version: proof.version,
            kind: proof.kind,
            amount: proof.amount,
            lane: proof.lane.clone(),
            predecessor_commitment: proof.predecessor_commitment,
            successor_commitment: proof.successor_commitment,
            predecessor_sequence: proof.predecessor_sequence,
            successor_sequence: proof.successor_sequence,
            predecessor_epoch: proof.predecessor_epoch,
            successor_epoch: proof.successor_epoch,
            predecessor_device_policy_binding: proof.predecessor_device_policy_binding,
            successor_device_policy_binding: proof.successor_device_policy_binding,
            predecessor_state_nonce_commitment: proof.predecessor_state_nonce_commitment,
            successor_state_nonce_commitment: proof.successor_state_nonce_commitment,
            journal_revision_before: proof.journal_revision_before,
            journal_revision_after: proof.journal_revision_after,
            state_transition_digest: proof.digest().expect("Core statement digest"),
            normalized_guard_statement_digest: normalized
                .canonical_digest()
                .expect("normalized digest"),
        };
        (proof, hardware, normalized)
    }

    #[test]
    fn captured_recursive_guard_frame_identity() {
        let (_, _, wire, lengths) = fixture();
        super::frame_identity_tests::check("GuardProofWire", &wire);
        let bytes = norito::encode_canonical(&wire).unwrap();
        assert!(decode_wire(&bytes, lengths).unwrap() == wire);
    }

    #[test]
    fn guard_archive_canonical_roundtrip_does_not_authorize_mock_proofs() {
        let (_, normalized, wire, lengths) = fixture();
        validate_normalized_release(&normalized, &wire.binding, EMPTY).expect("release projection");
        validate_wire(
            &wire,
            &wire.binding,
            wire.statement_digest,
            wire.credential_digests,
            lengths,
        )
        .expect("shape only");
        let bytes = norito::encode_canonical(&wire).expect("frame");
        let decoded = decode_wire(&bytes, lengths).expect("bounded decode");
        assert!(decoded == wire);
        assert_eq!(
            norito::encode_canonical(&decoded).expect("canonical reencode"),
            bytes
        );
        let header = norito::core::Header::read(std::io::Cursor::new(bytes)).expect("header");
        assert_eq!(
            header.schema,
            norito::core::schema_hash_for_name("iroha.kagemusha.core.v1.monetary-guard-proof")
        );
    }

    #[test]
    fn guard_archive_rejects_schema_trailing_data_and_allocation_amplification() {
        let (_, _, wire, lengths) = fixture();
        let mut bytes = norito::encode_canonical(&wire).expect("frame");
        bytes[6] ^= 1;
        assert!(matches!(
            decode_wire(&bytes, lengths),
            Err(KagemushaGuardVerificationErrorV1::Encoding)
        ));
        let mut bytes = norito::encode_canonical(&wire).expect("frame");
        bytes.push(0);
        assert!(decode_wire(&bytes, lengths).is_err());
        assert!(decode_wire(&[], lengths).is_err());
        assert!(matches!(
            decode_wire(&vec![0; KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1 + 1], lengths),
            Err(KagemushaGuardVerificationErrorV1::Shape)
        ));
        let mut large = wire.clone();
        large.eq_proof = vec![1; lengths.limits().expect("limits").max_sequence_elements() + 1];
        let bytes = norito::encode_canonical(&large).expect("bounded frame with excessive vector");
        assert!(bytes.len() < KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1);
        assert!(matches!(
            decode_wire(&bytes, lengths),
            Err(KagemushaGuardVerificationErrorV1::Encoding)
        ));
        assert!(
            GuardProofLengths {
                eq: usize::MAX,
                ep: 1
            }
            .limits()
            .is_err()
        );
        assert!(GuardProofLengths { eq: 0, ep: 1 }.limits().is_err());
    }

    #[test]
    fn guard_archive_rejects_the_old_archive_without_proof_bound_credential_slots() {
        #[derive(Encode)]
        struct OldGuardProofWire {
            version: u16,
            binding: KagemushaGuardVerifierBindingV1,
            statement_digest: DigestV1,
            eq_credential_audit: DigestV1,
            ep_credential_audit: DigestV1,
            eq_proof: Vec<u8>,
            ep_proof: Vec<u8>,
            eq_history: HistoryBytes,
            ep_history: HistoryBytes,
        }
        let (_, _, wire, lengths) = fixture();
        let old = OldGuardProofWire {
            version: wire.version,
            binding: wire.binding,
            statement_digest: wire.statement_digest,
            eq_credential_audit: wire.eq_credential_audit,
            ep_credential_audit: wire.ep_credential_audit,
            eq_proof: wire.eq_proof,
            ep_proof: wire.ep_proof,
            eq_history: wire.eq_history,
            ep_history: wire.ep_history,
        };
        let (payload, flags) = norito::codec::encode_with_header_flags(&old);
        let bytes = norito::core::frame_bare_with_header_flags::<GuardProofWire>(&payload, flags)
            .expect("frame old archive payload under the actual guard owner");
        let view = norito::core::from_bytes_view(&bytes).expect("valid frame and checksum");
        assert_eq!(
            view.schema(),
            norito::schema::identity::frame_hash::<GuardProofWire>()
        );
        assert_eq!(view.as_bytes(), payload);
        assert!(decode_wire(&bytes, lengths).is_err());
    }

    #[test]
    fn guard_archive_requires_the_exact_nonzero_predecessor_successor_pair() {
        let (_, _, wire, lengths) = fixture();
        for index in 0..2 {
            let mut changed = wire.clone();
            changed.credential_digests[index][0] ^= 1;
            assert_eq!(
                validate_wire(
                    &changed,
                    &wire.binding,
                    wire.statement_digest,
                    wire.credential_digests,
                    lengths
                ),
                Err(KagemushaGuardVerificationErrorV1::Binding)
            );
            assert_eq!(
                validate_wire(
                    &wire,
                    &wire.binding,
                    wire.statement_digest,
                    changed.credential_digests,
                    lengths
                ),
                Err(KagemushaGuardVerificationErrorV1::Binding)
            );
            changed.credential_digests[index] = [0; 32];
            assert_eq!(
                validate_wire(
                    &changed,
                    &wire.binding,
                    wire.statement_digest,
                    changed.credential_digests,
                    lengths
                ),
                Err(KagemushaGuardVerificationErrorV1::Shape)
            );
        }
        let mut swapped = wire.credential_digests;
        swapped.swap(0, 1);
        assert_eq!(
            validate_wire(
                &wire,
                &wire.binding,
                wire.statement_digest,
                swapped,
                lengths
            ),
            Err(KagemushaGuardVerificationErrorV1::Binding)
        );
    }

    #[test]
    fn guard_archive_rejects_exact_lengths_version_and_every_identity_substitution() {
        let (_, _, wire, lengths) = fixture();
        for index in 0..6 {
            let mut changed = wire.clone();
            let identities = [
                &mut changed.binding.release_id,
                &mut changed.binding.suite_id,
                &mut changed.binding.vk_set_digest,
                &mut changed.binding.artifact_manifest_digest,
                &mut changed.binding.eq_protocol_digest,
                &mut changed.binding.ep_protocol_digest,
            ];
            identities[index][0] ^= 1;
            assert_eq!(
                validate_wire(
                    &changed,
                    &wire.binding,
                    wire.statement_digest,
                    wire.credential_digests,
                    lengths
                ),
                Err(KagemushaGuardVerificationErrorV1::Binding)
            );
        }
        for index in 0..3 {
            let mut changed = wire.clone();
            match index {
                0 => changed.version = 2,
                1 => changed.eq_proof.push(0),
                _ => {
                    changed.ep_proof.pop();
                }
            }
            assert_eq!(
                validate_wire(
                    &changed,
                    &wire.binding,
                    wire.statement_digest,
                    wire.credential_digests,
                    lengths
                ),
                Err(KagemushaGuardVerificationErrorV1::Shape)
            );
        }
        let mut digest = wire.statement_digest;
        digest[0] ^= 1;
        assert_eq!(
            validate_wire(
                &wire,
                &wire.binding,
                digest,
                wire.credential_digests,
                lengths
            ),
            Err(KagemushaGuardVerificationErrorV1::Binding)
        );
    }

    #[test]
    fn guard_archive_rejects_noncanonical_audits_and_invalid_histories() {
        let (_, _, wire, lengths) = fixture();
        for index in 0..5 {
            let mut changed = wire.clone();
            match index {
                0 => changed.eq_credential_audit = [0xff; 32],
                1 => changed.ep_credential_audit = [0; 32],
                2 => changed.ep_credential_audit = changed.eq_credential_audit,
                3 => changed.eq_history[512..].fill(0),
                _ => changed.ep_history[..32].fill(0xff),
            }
            assert_eq!(
                validate_wire(
                    &changed,
                    &wire.binding,
                    wire.statement_digest,
                    wire.credential_digests,
                    lengths
                ),
                Err(KagemushaGuardVerificationErrorV1::Shape)
            );
        }
    }

    #[test]
    fn guard_public_column_preserves_statement_shared_audits_and_own_history() {
        let (_, _, wire, _) = fixture();
        assert_eq!(GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1, 44);
        let eq = public_column::<Fp>(
            wire.statement_digest,
            wire.eq_credential_audit,
            wire.ep_credential_audit,
            wire.credential_digests,
            &wire.eq_history,
        );
        let ep = public_column::<Fq>(
            wire.statement_digest,
            wire.eq_credential_audit,
            wire.ep_credential_audit,
            wire.credential_digests,
            &wire.ep_history,
        );
        assert_eq!(eq.len(), 44);
        assert_eq!(ep.len(), 44);
        assert_eq!(
            &eq[GUARD_PREDECESSOR_CREDENTIAL_OFFSET_V1..GUARD_SUCCESSOR_CREDENTIAL_OFFSET_V1],
            &digest_limbs::<Fp>(wire.credential_digests[0])
        );
        assert_eq!(
            &eq[GUARD_SUCCESSOR_CREDENTIAL_OFFSET_V1..GUARD_HISTORY_OFFSET_V1],
            &digest_limbs::<Fp>(wire.credential_digests[1])
        );
        for index in 0..10 {
            assert_eq!(eq[index].to_repr(), ep[index].to_repr());
        }
        for (index, chunk) in wire.eq_history.chunks_exact(16).enumerate() {
            assert_eq!(&eq[GUARD_HISTORY_OFFSET_V1 + index].to_repr()[..16], chunk);
        }
        for (index, chunk) in wire.ep_history.chunks_exact(16).enumerate() {
            assert_eq!(&ep[GUARD_HISTORY_OFFSET_V1 + index].to_repr()[..16], chunk);
        }
    }

    #[test]
    fn guard_bootstrap_binds_the_caller_normalized_state_and_release() {
        let (bootstrap, normalized, wire, _) = fixture();
        validate_bootstrap_binding(&bootstrap, &normalized, EMPTY).expect("matching bootstrap");
        let mut changed = normalized;
        changed.lane_id[0] ^= 1;
        assert!(validate_bootstrap_binding(&bootstrap, &changed, EMPTY).is_err());
        let mut changed = normalized;
        changed.predecessor_state_commitment = [1; 32];
        assert!(validate_normalized_release(&changed, &wire.binding, EMPTY).is_err());
        for index in 0..3 {
            let mut binding = wire.binding.clone();
            let identities = [
                &mut binding.release_id,
                &mut binding.suite_id,
                &mut binding.vk_set_digest,
            ];
            identities[index][0] ^= 1;
            assert!(validate_normalized_release(&normalized, &binding, EMPTY).is_err());
        }
        assert!(validate_normalized_release(&normalized, &wire.binding, [0x01; 32]).is_err());
    }

    #[test]
    fn guard_transition_rejects_core_hardware_and_normalized_substitution() {
        let (proof, hardware, normalized) = transition_fixture();
        validate_transition_binding(&hardware, &proof, &normalized, EMPTY)
            .expect("matching Core hardware tuple");
        let mut changed = proof.clone();
        changed.mint_finality_semantic_digest[0] ^= 1;
        assert!(validate_transition_binding(&hardware, &changed, &normalized, EMPTY).is_err());
        let mut changed = hardware.clone();
        changed.amount += 1;
        assert!(validate_transition_binding(&changed, &proof, &normalized, EMPTY).is_err());
        let mut changed = normalized;
        changed.transition_intent_digest[0] ^= 1;
        assert!(validate_transition_binding(&hardware, &proof, &changed, EMPTY).is_err());
        let mut changed = normalized;
        changed.successor_key_reference[0] ^= 1;
        assert!(validate_transition_binding(&hardware, &proof, &changed, EMPTY).is_err());
    }

    #[test]
    fn production_guard_rejects_even_shape_valid_proof_material_without_policy_authority() {
        let (bootstrap, normalized, wire, lengths) = fixture();
        validate_wire(
            &wire,
            &wire.binding,
            wire.statement_digest,
            wire.credential_digests,
            lengths,
        )
        .expect("shape-valid material is not provider authority");
        let bytes = norito::encode_canonical(&wire).expect("shape-valid frame");
        // The private sentinel exercises the production trait's independent fail-closed check.
        // No authenticated constructor or test authority bypass is introduced.
        let verifier = KagemushaAuthenticatedGuardBundleVerifierV1 { _private: () };
        let expected =
            KagemushaGuardVerificationErrorV1::ProviderPolicyAuthorityUnavailable.to_string();
        assert_eq!(
            verifier.verify_bootstrap(&bootstrap, &normalized, &bytes),
            Err(expected.clone())
        );
        let (proof, hardware, normalized) = transition_fixture();
        assert_eq!(
            verifier.verify_transition(&hardware, &proof, &normalized, &bytes),
            Err(expected.clone())
        );
        let mut selected_root = normalized;
        selected_root.successor_hardware_policy_id = [0xEE; 32];
        assert_eq!(
            verifier.verify_transition(&hardware, &proof, &selected_root, &[]),
            Err(expected)
        );
    }

    #[test]
    fn normalized_policy_root_requires_one_release_root_and_null_bootstrap_predecessor() {
        let (_, normalized, _, _) = fixture();
        let root = normalized.successor_hardware_policy_id;
        validate_normalized_provider_policy_root_v1(&normalized, root).expect("bootstrap root");
        assert!(validate_normalized_provider_policy_root_v1(&normalized, [0; 32]).is_err());
        assert!(validate_normalized_provider_policy_root_v1(&normalized, [0xEE; 32]).is_err());
        let mut nonnull = normalized;
        nonnull.predecessor_hardware_policy_id = root;
        assert!(validate_normalized_provider_policy_root_v1(&nonnull, root).is_err());
        let (_, _, transition) = transition_fixture();
        validate_normalized_provider_policy_root_v1(&transition, root)
            .expect("same release transition root");
        let mut changed = transition;
        changed.predecessor_hardware_policy_id = [0xEE; 32];
        assert!(validate_normalized_provider_policy_root_v1(&changed, root).is_err());
    }
}
