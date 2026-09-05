//! Bounded private checkpoints for the native recursive State carrier.
//!
//! This is a plaintext payload for a qualified, atomic sealing service, not an export format.
//! Canonical decoding grants neither monetary authority nor freshness. Restoration authenticates
//! the release, the caller's expected Core state, both retained inner proofs and their complete
//! histories, and the paired transport proof. The owner must separately authenticate the latest
//! sealed checkpoint together with its Core snapshot, operation WAL and response journal.

use ff::PrimeField;
use halo2_proofs::{
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    poly::ipa::commitment::ParamsIPA,
};
use iroha_data_model::kagemusha::KagemushaPairedProofV1;
use norito::{
    DecodeLimits,
    codec::{Decode, Encode},
};
use snark_verifier::verifier::plonk::PlonkProtocol;
use thiserror::Error;

use super::{
    DigestV1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1,
    KagemushaAuthenticatedRecursiveVerifierV1, KagemushaEpAccumulatorV1, KagemushaEqAccumulatorV1,
    KagemushaGeneratedRecursiveStateProofV1, KagemushaRecursionArtifactsV1,
    KagemushaStateRelationPublicInputsV1, decide_kagemusha_ep_accumulator_v1,
    decide_kagemusha_eq_accumulator_v1,
    deferred_parent::ordinary_ipa_proof_profile_v1,
    native_backend::{verify_ep_succinct_protocol, verify_eq_succinct_protocol},
    state_relation::{PUBLIC_INSTANCE_COUNT, public_instance},
    transport_decider::KAGEMUSHA_TRANSPORT_DECIDER_PUBLIC_INSTANCE_COUNT_V1,
    verify_kagemusha_state_proof_v1,
};
use crate::zk::kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, from_u128};

const COLUMN_CELLS: usize = KAGEMUSHA_TRANSPORT_DECIDER_PUBLIC_INSTANCE_COUNT_V1;
const VERSION: u16 = 1;
type ColumnBytes = [[u8; 32]; COLUMN_CELLS];
type HistoryBytes = [u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1];
type Result<T> = core::result::Result<T, KagemushaStateCheckpointErrorV1>;

/// Rejection at the private recursive State checkpoint boundary.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum KagemushaStateCheckpointErrorV1 {
    /// A proof, column, field, history, or decode resource budget has the wrong shape.
    #[error("invalid recursive State checkpoint shape or resource bound")]
    Shape,
    /// A canonical Norito frame could not be decoded or encoded exactly.
    #[error("noncanonical recursive State checkpoint encoding")]
    Encoding,
    /// Retained data differs from the authenticated release or expected Core state.
    #[error("recursive State checkpoint binding mismatch")]
    Binding,
    /// An actual retained proof or accumulator failed native verification.
    #[error("recursive State checkpoint proof rejected: {0}")]
    Proof(String),
}

/// Role-separated identities obtained only from the authenticated verifier loader.
#[derive(Clone, PartialEq, Eq, Decode, Encode)]
pub(super) struct KagemushaStateCheckpointBindingV1 {
    pub(super) release_id: DigestV1,
    pub(super) suite_id: DigestV1,
    pub(super) vk_set_digest: DigestV1,
    pub(super) artifact_manifest_digest: DigestV1,
    pub(super) inner_eq_protocol_digest: DigestV1,
    pub(super) inner_ep_protocol_digest: DigestV1,
    pub(super) outer_eq_protocol_digest: DigestV1,
    pub(super) outer_ep_protocol_digest: DigestV1,
}

/// Borrowed immutable verifier material; no public constructor or serialized key authority.
pub(super) struct KagemushaStateCheckpointVerifierMaterialV1<'a> {
    pub(super) eq_parameters: &'a ParamsIPA<EqAffine>,
    pub(super) ep_parameters: &'a ParamsIPA<EpAffine>,
    pub(super) inner_eq_protocol: &'a PlonkProtocol<EqAffine>,
    pub(super) inner_ep_protocol: &'a PlonkProtocol<EpAffine>,
    pub(super) outer_eq_protocol: &'a PlonkProtocol<EqAffine>,
    pub(super) outer_ep_protocol: &'a PlonkProtocol<EpAffine>,
    pub(super) binding: KagemushaStateCheckpointBindingV1,
    pub(super) artifacts: KagemushaRecursionArtifactsV1,
}

#[derive(Clone, PartialEq, Eq, Decode, Encode)]
struct CheckpointWire {
    version: u16,
    binding: KagemushaStateCheckpointBindingV1,
    eq_column: ColumnBytes,
    ep_column: ColumnBytes,
    eq_inner_proof: Vec<u8>,
    ep_inner_proof: Vec<u8>,
    eq_history: HistoryBytes,
    ep_history: HistoryBytes,
    proof: KagemushaPairedProofV1,
}

/// Canonical, unsealed private payload for one recursive State checkpoint.
///
/// This type deliberately has no public fields or blanket decoder. Use the bounded decoder,
/// then [`Self::restore`] with the release-authenticated verifier and expected Core state.
/// Successful restoration authenticates proofs, not the age of the checkpoint. The native owner
/// must authenticate sealing, wallet identity and rollback protection before using the result.
#[derive(Clone, PartialEq, Eq)]
pub struct KagemushaRecursiveStateCheckpointV1(CheckpointWire);

impl core::fmt::Debug for KagemushaRecursiveStateCheckpointV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KagemushaRecursiveStateCheckpointV1")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy)]
struct ProofLengths {
    inner_eq: usize,
    inner_ep: usize,
    outer_eq: usize,
    outer_ep: usize,
}

impl ProofLengths {
    fn authenticated(material: &KagemushaStateCheckpointVerifierMaterialV1<'_>) -> Result<Self> {
        let lengths = Self {
            inner_eq: ordinary_ipa_proof_profile_v1(material.inner_eq_protocol)
                .map_err(KagemushaStateCheckpointErrorV1::Proof)?
                .byte_len,
            inner_ep: ordinary_ipa_proof_profile_v1(material.inner_ep_protocol)
                .map_err(KagemushaStateCheckpointErrorV1::Proof)?
                .byte_len,
            outer_eq: ordinary_ipa_proof_profile_v1(material.outer_eq_protocol)
                .map_err(KagemushaStateCheckpointErrorV1::Proof)?
                .byte_len,
            outer_ep: ordinary_ipa_proof_profile_v1(material.outer_ep_protocol)
                .map_err(KagemushaStateCheckpointErrorV1::Proof)?
                .byte_len,
        };
        if lengths.outer_eq > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
            || lengths.outer_ep > KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1
        {
            return Err(KagemushaStateCheckpointErrorV1::Shape);
        }
        Ok(lengths)
    }

    // Budgets depend on authenticated protocol inventories, never payload-declared lengths.
    // The fixed allowance covers the two array columns, four histories, identity fields and
    // Norito framing. The per-proof allowance also covers canonical sequence framing. Exact
    // transcript lengths are checked independently after bounded decoding and before parsing.
    fn limits(self) -> Result<(usize, DecodeLimits)> {
        let lengths = [self.inner_eq, self.inner_ep, self.outer_eq, self.outer_ep];
        if lengths.contains(&0) {
            return Err(KagemushaStateCheckpointErrorV1::Shape);
        }
        let proof_bytes = lengths
            .into_iter()
            .try_fold(0_usize, usize::checked_add)
            .ok_or(KagemushaStateCheckpointErrorV1::Shape)?;
        let maximum = proof_bytes
            .checked_mul(4)
            .and_then(|n| n.checked_add(64 * 1024))
            .ok_or(KagemushaStateCheckpointErrorV1::Shape)?;
        let total_budget = maximum
            .checked_mul(4)
            .ok_or(KagemushaStateCheckpointErrorV1::Shape)?;
        let sequence = lengths
            .into_iter()
            .max()
            .unwrap_or(0)
            .max(KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1)
            .max(COLUMN_CELLS);
        Ok((
            maximum,
            DecodeLimits::new(sequence, maximum, total_budget, total_budget, 16),
        ))
    }
}

impl KagemushaRecursiveStateCheckpointV1 {
    /// Verify generated output and retain only the data needed for exact native restoration.
    ///
    /// Current opening claims and transport columns are rederived and compared, then omitted
    /// from the payload. The caller must atomically seal the resulting bytes before durability
    /// acknowledgment. No recovery seed or capability is serialized.
    ///
    /// # Errors
    /// Rejects malformed, substituted or cryptographically invalid generated output.
    pub fn capture(
        generated: &KagemushaGeneratedRecursiveStateProofV1,
        verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
        expected: &KagemushaStateRelationPublicInputsV1,
    ) -> Result<Self> {
        let material = verifier.state_checkpoint_material();
        let lengths = ProofLengths::authenticated(&material)?;
        if generated.eq_inner_proof.len() != lengths.inner_eq
            || generated.ep_inner_proof.len() != lengths.inner_ep
            || generated.proof.eq_proof.len() != lengths.outer_eq
            || generated.proof.ep_proof.len() != lengths.outer_ep
            || generated.eq_public_instances.len() != COLUMN_CELLS
            || generated.ep_public_instances.len() != COLUMN_CELLS
            || generated.eq_transport_public_instances.len() != COLUMN_CELLS
            || generated.ep_transport_public_instances.len() != COLUMN_CELLS
        {
            return Err(KagemushaStateCheckpointErrorV1::Shape);
        }
        generated
            .proof
            .validate_shape_for_semantic_digest(expected.transport_semantic_digest)
            .map_err(|_| KagemushaStateCheckpointErrorV1::Shape)?;
        let checkpoint = Self(CheckpointWire {
            version: VERSION,
            binding: material.binding,
            eq_column: encode_column(&generated.eq_public_instances)?,
            ep_column: encode_column(&generated.ep_public_instances)?,
            eq_inner_proof: generated.eq_inner_proof.clone(),
            ep_inner_proof: generated.ep_inner_proof.clone(),
            eq_history: *generated.eq_history.as_bytes(),
            ep_history: *generated.ep_history.as_bytes(),
            proof: generated.proof.clone(),
        });
        let restored = checkpoint.restore(verifier, expected)?;
        if &restored != generated {
            return Err(KagemushaStateCheckpointErrorV1::Binding);
        }
        Ok(checkpoint)
    }

    /// Encode the bounded canonical Norito payload for a qualified sealing service.
    ///
    /// # Errors
    /// Rejects a release substitution, malformed shape, or encoding exceeding its release budget.
    pub fn encode_canonical(
        &self,
        verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    ) -> Result<Vec<u8>> {
        let material = verifier.state_checkpoint_material();
        let lengths = ProofLengths::authenticated(&material)?;
        self.validate_shape(&material.binding, lengths)?;
        self.encode_with_lengths(lengths)
    }

    /// Decode one exact canonical private payload with release-derived resource limits.
    ///
    /// Collection counts and allocation budgets are enforced by Norito before allocation.
    /// This checks framing only; call [`Self::restore`] to authenticate retained proofs.
    ///
    /// # Errors
    /// Rejects oversized, noncanonical, malformed or differently bound payloads.
    pub fn decode_canonical_exact(
        bytes: &[u8],
        verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    ) -> Result<Self> {
        let material = verifier.state_checkpoint_material();
        Self::decode_with_lengths(
            bytes,
            &material.binding,
            ProofLengths::authenticated(&material)?,
        )
    }

    /// Reverify both actual inner proofs and complete histories against the expected Core state.
    ///
    /// Both transport proofs and their histories are also verified. The retained inner pair is
    /// independently authenticated for the same exact state; the generation result does not
    /// retain the randomized transport-fold transcripts, and this factory does not reconstruct
    /// them. Complete local histories are terminally decided rather than trusted as bytes.
    ///
    /// `expected` must come from the owner's restored Core transition and include this proof's
    /// exact transport audits. This method neither loads a wallet nor authenticates freshness.
    ///
    /// # Errors
    /// Rejects any release/state/pair substitution or failed proof or accumulator decision.
    pub fn restore(
        &self,
        verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
        expected: &KagemushaStateRelationPublicInputsV1,
    ) -> Result<KagemushaGeneratedRecursiveStateProofV1> {
        let material = verifier.state_checkpoint_material();
        self.validate_shape(&material.binding, ProofLengths::authenticated(&material)?)?;
        validate_expected_release(expected, verifier, &material.binding)?;
        let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&self.0.eq_history)
            .map_err(|_| KagemushaStateCheckpointErrorV1::Shape)?;
        let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&self.0.ep_history)
            .map_err(|_| KagemushaStateCheckpointErrorV1::Shape)?;
        let (eq_column, ep_column) = self.validate_columns(expected)?;
        verify_kagemusha_state_proof_v1(verifier, material.artifacts, expected, &self.0.proof)
            .map_err(|error| KagemushaStateCheckpointErrorV1::Proof(error.to_string()))?;
        let eq_current = verify_eq_succinct_protocol(
            material.eq_parameters,
            material.inner_eq_protocol,
            &self.0.eq_inner_proof,
            &eq_column,
        )
        .map_err(KagemushaStateCheckpointErrorV1::Proof)?;
        let ep_current = verify_ep_succinct_protocol(
            material.ep_parameters,
            material.inner_ep_protocol,
            &self.0.ep_inner_proof,
            &ep_column,
        )
        .map_err(KagemushaStateCheckpointErrorV1::Proof)?;
        let eq_current_accumulator = KagemushaEqAccumulatorV1::from_native(&eq_current)
            .map_err(|error| KagemushaStateCheckpointErrorV1::Proof(error.to_string()))?;
        let ep_current_accumulator = KagemushaEpAccumulatorV1::from_native(&ep_current)
            .map_err(|error| KagemushaStateCheckpointErrorV1::Proof(error.to_string()))?;
        decide_kagemusha_eq_accumulator_v1(material.eq_parameters, &eq_current_accumulator)
            .and_then(|()| decide_kagemusha_eq_accumulator_v1(material.eq_parameters, &eq_history))
            .map_err(|error| KagemushaStateCheckpointErrorV1::Proof(error.to_string()))?;
        decide_kagemusha_ep_accumulator_v1(material.ep_parameters, &ep_current_accumulator)
            .and_then(|()| decide_kagemusha_ep_accumulator_v1(material.ep_parameters, &ep_history))
            .map_err(|error| KagemushaStateCheckpointErrorV1::Proof(error.to_string()))?;
        Ok(KagemushaGeneratedRecursiveStateProofV1 {
            eq_public_instances: eq_column,
            ep_public_instances: ep_column,
            eq_transport_public_instances: column_with_history::<Fp>(
                expected,
                &self.0.proof.eq_history,
            )?,
            ep_transport_public_instances: column_with_history::<Fq>(
                expected,
                &self.0.proof.ep_history,
            )?,
            eq_inner_proof: self.0.eq_inner_proof.clone(),
            ep_inner_proof: self.0.ep_inner_proof.clone(),
            proof: self.0.proof.clone(),
            eq_current_accumulator,
            ep_current_accumulator,
            eq_history,
            ep_history,
        })
    }

    fn encode_with_lengths(&self, lengths: ProofLengths) -> Result<Vec<u8>> {
        let (maximum, _) = lengths.limits()?;
        let bytes = norito::encode_canonical(&self.0)
            .map_err(|_| KagemushaStateCheckpointErrorV1::Encoding)?;
        if bytes.len() > maximum {
            return Err(KagemushaStateCheckpointErrorV1::Shape);
        }
        Ok(bytes)
    }

    fn decode_with_lengths(
        bytes: &[u8],
        binding: &KagemushaStateCheckpointBindingV1,
        lengths: ProofLengths,
    ) -> Result<Self> {
        let (maximum, limits) = lengths.limits()?;
        if bytes.is_empty() || bytes.len() > maximum {
            return Err(KagemushaStateCheckpointErrorV1::Shape);
        }
        let wire = norito::decode_canonical_with_limits(bytes, limits)
            .map_err(|_| KagemushaStateCheckpointErrorV1::Encoding)?;
        let checkpoint = Self(wire);
        checkpoint.validate_shape(binding, lengths)?;
        Ok(checkpoint)
    }

    fn validate_shape(
        &self,
        binding: &KagemushaStateCheckpointBindingV1,
        lengths: ProofLengths,
    ) -> Result<()> {
        if self.0.version != VERSION
            || self.0.eq_inner_proof.len() != lengths.inner_eq
            || self.0.ep_inner_proof.len() != lengths.inner_ep
            || self.0.proof.eq_proof.len() != lengths.outer_eq
            || self.0.proof.ep_proof.len() != lengths.outer_ep
        {
            return Err(KagemushaStateCheckpointErrorV1::Shape);
        }
        if &self.0.binding != binding
            || self.0.proof.eq_protocol_digest != binding.outer_eq_protocol_digest
            || self.0.proof.ep_protocol_digest != binding.outer_ep_protocol_digest
        {
            return Err(KagemushaStateCheckpointErrorV1::Binding);
        }
        self.0
            .proof
            .validate_shape_for_semantic_digest(self.0.proof.semantic_digest)
            .map_err(|_| KagemushaStateCheckpointErrorV1::Shape)?;
        decode_column::<Fp>(&self.0.eq_column)?;
        decode_column::<Fq>(&self.0.ep_column)?;
        KagemushaEqAccumulatorV1::try_from_bytes(&self.0.eq_history)
            .map_err(|_| KagemushaStateCheckpointErrorV1::Shape)?;
        KagemushaEpAccumulatorV1::try_from_bytes(&self.0.ep_history)
            .map_err(|_| KagemushaStateCheckpointErrorV1::Shape)?;
        KagemushaEqAccumulatorV1::try_from_bytes(&self.0.proof.eq_history)
            .map_err(|_| KagemushaStateCheckpointErrorV1::Shape)?;
        KagemushaEpAccumulatorV1::try_from_bytes(&self.0.proof.ep_history)
            .map_err(|_| KagemushaStateCheckpointErrorV1::Shape)?;
        Ok(())
    }

    fn validate_columns(
        &self,
        expected: &KagemushaStateRelationPublicInputsV1,
    ) -> Result<(Vec<Fp>, Vec<Fq>)> {
        let mut inner = expected.clone();
        inner.eq_protocol_digest = self.0.binding.inner_eq_protocol_digest;
        inner.ep_protocol_digest = self.0.binding.inner_ep_protocol_digest;
        inner.eq_deferred_audit =
            digest_from_column::<Fp>(&self.0.eq_column, public_instance::EQ_DEFERRED_AUDIT_LO)?;
        inner.ep_deferred_audit =
            digest_from_column::<Fq>(&self.0.eq_column, public_instance::EP_DEFERRED_AUDIT_LO)?;
        if inner.eq_deferred_audit == inner.ep_deferred_audit {
            return Err(KagemushaStateCheckpointErrorV1::Binding);
        }
        let eq = column_with_history::<Fp>(&inner, &self.0.eq_history)?;
        let ep = column_with_history::<Fq>(&inner, &self.0.ep_history)?;
        if encode_column(&eq)? != self.0.eq_column || encode_column(&ep)? != self.0.ep_column {
            return Err(KagemushaStateCheckpointErrorV1::Binding);
        }
        Ok((eq, ep))
    }
}

fn validate_expected_release(
    expected: &KagemushaStateRelationPublicInputsV1,
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    binding: &KagemushaStateCheckpointBindingV1,
) -> Result<()> {
    if expected.successor.release_id != binding.release_id
        || expected.successor.suite_id != binding.suite_id
        || expected.successor.vk_digest != binding.vk_set_digest
        || expected.mint_authorization_eq_protocol_digest
            != verifier.mint_authorization_eq_protocol_digest()
        || expected.mint_authorization_ep_protocol_digest
            != verifier.mint_authorization_ep_protocol_digest()
        || expected.predecessor.as_ref().is_some_and(|state| {
            state.release_id != binding.release_id
                || state.suite_id != binding.suite_id
                || state.vk_digest != binding.vk_set_digest
        })
    {
        return Err(KagemushaStateCheckpointErrorV1::Binding);
    }
    expected
        .successor
        .validate()
        .map_err(|_| KagemushaStateCheckpointErrorV1::Binding)?;
    if let Some(state) = &expected.predecessor {
        state
            .validate()
            .map_err(|_| KagemushaStateCheckpointErrorV1::Binding)?;
    }
    Ok(())
}

fn encode_column<F: PrimeField<Repr = [u8; 32]>>(column: &[F]) -> Result<ColumnBytes> {
    if column.len() != COLUMN_CELLS {
        return Err(KagemushaStateCheckpointErrorV1::Shape);
    }
    Ok(core::array::from_fn(|index| column[index].to_repr()))
}

fn decode_column<F: PrimeField<Repr = [u8; 32]>>(column: &ColumnBytes) -> Result<Vec<F>> {
    column
        .iter()
        .map(|bytes| {
            Option::<F>::from(F::from_repr(*bytes)).ok_or(KagemushaStateCheckpointErrorV1::Shape)
        })
        .collect()
}

fn digest_from_column<F: PrimeField<Repr = [u8; 32]>>(
    column: &ColumnBytes,
    low: usize,
) -> Result<DigestV1> {
    let mut digest = [0; 32];
    for limb in 0..2 {
        if column[low + limb][16..].iter().any(|byte| *byte != 0) {
            return Err(KagemushaStateCheckpointErrorV1::Shape);
        }
        digest[limb * 16..(limb + 1) * 16].copy_from_slice(&column[low + limb][..16]);
    }
    if digest == [0; 32] || Option::<F>::from(F::from_repr(digest)).is_none() {
        return Err(KagemushaStateCheckpointErrorV1::Shape);
    }
    Ok(digest)
}

fn column_with_history<F: KagemushaPoseidonFieldV1>(
    public: &KagemushaStateRelationPublicInputsV1,
    history: &[u8],
) -> Result<Vec<F>> {
    if history.len() != KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1 {
        return Err(KagemushaStateCheckpointErrorV1::Shape);
    }
    let mut column = public
        .public_instances::<F>()
        .map_err(KagemushaStateCheckpointErrorV1::Proof)?;
    if column.len() != PUBLIC_INSTANCE_COUNT {
        return Err(KagemushaStateCheckpointErrorV1::Shape);
    }
    for bytes in history.chunks_exact(16) {
        column.push(from_u128::<F>(u128::from_le_bytes(
            bytes
                .try_into()
                .map_err(|_| KagemushaStateCheckpointErrorV1::Shape)?,
        )));
    }
    Ok(column)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ff::Field;

    // Mock transcripts exercise framing and binding only. No accepting verifier is constructed.
    fn fixture() -> (
        KagemushaRecursiveStateCheckpointV1,
        KagemushaStateRelationPublicInputsV1,
        ProofLengths,
    ) {
        let (public, proof) = super::super::tests::state_verification_fixture();
        let mut inner = public.clone();
        inner.eq_protocol_digest = Fp::from(7001).to_repr();
        inner.ep_protocol_digest = Fq::from(7002).to_repr();
        inner.eq_deferred_audit = Fp::from(7003).to_repr();
        inner.ep_deferred_audit = Fq::from(7004).to_repr();
        let eq_history = proof
            .eq_history
            .as_slice()
            .try_into()
            .expect("fixed Eq history");
        let ep_history = proof
            .ep_history
            .as_slice()
            .try_into()
            .expect("fixed Ep history");
        let checkpoint = KagemushaRecursiveStateCheckpointV1(CheckpointWire {
            version: VERSION,
            binding: KagemushaStateCheckpointBindingV1 {
                release_id: public.successor.release_id,
                suite_id: public.successor.suite_id,
                vk_set_digest: public.successor.vk_digest,
                artifact_manifest_digest: [9; 32],
                inner_eq_protocol_digest: inner.eq_protocol_digest,
                inner_ep_protocol_digest: inner.ep_protocol_digest,
                outer_eq_protocol_digest: public.eq_protocol_digest,
                outer_ep_protocol_digest: public.ep_protocol_digest,
            },
            eq_column: encode_column(
                &column_with_history::<Fp>(&inner, &proof.eq_history).expect("Eq projection"),
            )
            .expect("Eq column"),
            ep_column: encode_column(
                &column_with_history::<Fq>(&inner, &proof.ep_history).expect("Ep projection"),
            )
            .expect("Ep column"),
            eq_inner_proof: vec![1; 32],
            ep_inner_proof: vec![2; 64],
            eq_history,
            ep_history,
            proof,
        });
        (
            checkpoint,
            public,
            ProofLengths {
                inner_eq: 32,
                inner_ep: 64,
                outer_eq: 1,
                outer_ep: 1,
            },
        )
    }

    #[test]
    fn checkpoint_shape_roundtrip_is_not_proof_authority() {
        let (checkpoint, public, lengths) = fixture();
        checkpoint
            .validate_shape(&checkpoint.0.binding, lengths)
            .expect("bounded shape");
        checkpoint
            .validate_columns(&public)
            .expect("exact native projection");
        let bytes = checkpoint
            .encode_with_lengths(lengths)
            .expect("canonical frame");
        let decoded = KagemushaRecursiveStateCheckpointV1::decode_with_lengths(
            &bytes,
            &checkpoint.0.binding,
            lengths,
        )
        .expect("bounded decode");
        assert_eq!(decoded, checkpoint);
        assert_eq!(
            decoded
                .encode_with_lengths(lengths)
                .expect("canonical reencode"),
            bytes
        );
        assert!(!format!("{decoded:?}").contains("7001"));
    }

    #[test]
    fn checkpoint_rejects_wrong_version_truncation_trailing_and_oversize() {
        let (checkpoint, _, lengths) = fixture();
        let bytes = checkpoint.encode_with_lengths(lengths).expect("frame");
        for end in [0, 1, bytes.len() / 2, bytes.len() - 1] {
            assert!(
                KagemushaRecursiveStateCheckpointV1::decode_with_lengths(
                    &bytes[..end],
                    &checkpoint.0.binding,
                    lengths
                )
                .is_err()
            );
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(
            KagemushaRecursiveStateCheckpointV1::decode_with_lengths(
                &trailing,
                &checkpoint.0.binding,
                lengths
            )
            .is_err()
        );
        let (maximum, _) = lengths.limits().expect("limits");
        assert_eq!(
            KagemushaRecursiveStateCheckpointV1::decode_with_lengths(
                &vec![0; maximum + 1],
                &checkpoint.0.binding,
                lengths
            ),
            Err(KagemushaStateCheckpointErrorV1::Shape)
        );
        let mut changed = checkpoint.clone();
        changed.0.version = 2;
        assert_eq!(
            changed.validate_shape(&checkpoint.0.binding, lengths),
            Err(KagemushaStateCheckpointErrorV1::Shape)
        );
    }

    #[test]
    fn checkpoint_release_and_four_protocol_identities_are_exact() {
        let (checkpoint, _, lengths) = fixture();
        for index in 0..8 {
            let mut changed = checkpoint.clone();
            let mut identities = [
                &mut changed.0.binding.release_id,
                &mut changed.0.binding.suite_id,
                &mut changed.0.binding.vk_set_digest,
                &mut changed.0.binding.artifact_manifest_digest,
                &mut changed.0.binding.inner_eq_protocol_digest,
                &mut changed.0.binding.inner_ep_protocol_digest,
                &mut changed.0.binding.outer_eq_protocol_digest,
                &mut changed.0.binding.outer_ep_protocol_digest,
            ];
            identities[index][0] ^= 1;
            assert_eq!(
                changed.validate_shape(&checkpoint.0.binding, lengths),
                Err(KagemushaStateCheckpointErrorV1::Binding),
                "identity {index}"
            );
        }
        let mut changed = checkpoint.clone();
        changed.0.proof.eq_protocol_digest = changed.0.binding.inner_eq_protocol_digest;
        assert_eq!(
            changed.validate_shape(&checkpoint.0.binding, lengths),
            Err(KagemushaStateCheckpointErrorV1::Binding)
        );
    }

    #[test]
    fn checkpoint_rejects_every_proof_length_substitution() {
        let (checkpoint, _, lengths) = fixture();
        for index in 0..4 {
            for extend in [false, true] {
                let mut changed = checkpoint.clone();
                let mut proofs = [
                    &mut changed.0.eq_inner_proof,
                    &mut changed.0.ep_inner_proof,
                    &mut changed.0.proof.eq_proof,
                    &mut changed.0.proof.ep_proof,
                ];
                if extend {
                    proofs[index].push(0);
                } else {
                    proofs[index].pop();
                }
                assert_eq!(
                    changed.validate_shape(&checkpoint.0.binding, lengths),
                    Err(KagemushaStateCheckpointErrorV1::Shape)
                );
            }
        }
    }

    #[test]
    fn checkpoint_decoder_enforces_sequence_budget_before_shape_validation() {
        let (checkpoint, _, lengths) = fixture();
        let mut changed = checkpoint.clone();
        let (_, limits) = lengths.limits().expect("limits");
        changed.0.eq_inner_proof = vec![1; limits.max_sequence_elements() + 1];
        let bytes = norito::encode_canonical(&changed.0).expect("oversized sequence frame");
        // The frame is below the byte budget, but the decoder rejects its collection count.
        assert!(bytes.len() < lengths.limits().expect("limits").0);
        assert_eq!(
            KagemushaRecursiveStateCheckpointV1::decode_with_lengths(
                &bytes,
                &checkpoint.0.binding,
                lengths
            ),
            Err(KagemushaStateCheckpointErrorV1::Encoding)
        );
        assert!(
            ProofLengths {
                inner_eq: usize::MAX,
                ..lengths
            }
            .limits()
            .is_err()
        );
        assert!(
            ProofLengths {
                inner_eq: 0,
                ..lengths
            }
            .limits()
            .is_err()
        );
    }

    #[test]
    fn checkpoint_rejects_noncanonical_fields_and_history_points() {
        let (checkpoint, _, lengths) = fixture();
        for eq in [true, false] {
            let mut changed = checkpoint.clone();
            if eq {
                changed.0.eq_column[0] = [0xff; 32];
            } else {
                changed.0.ep_column[0] = [0xff; 32];
            }
            assert_eq!(
                changed.validate_shape(&checkpoint.0.binding, lengths),
                Err(KagemushaStateCheckpointErrorV1::Shape)
            );
        }
        for index in 0..4 {
            let mut changed = checkpoint.clone();
            let mut histories: [&mut [u8]; 4] = [
                &mut changed.0.eq_history,
                &mut changed.0.ep_history,
                &mut changed.0.proof.eq_history,
                &mut changed.0.proof.ep_history,
            ];
            histories[index][512..].fill(0);
            assert_eq!(
                changed.validate_shape(&checkpoint.0.binding, lengths),
                Err(KagemushaStateCheckpointErrorV1::Shape)
            );
        }
        assert!(encode_column(&vec![Fp::ZERO; COLUMN_CELLS - 1]).is_err());
        assert!(encode_column(&vec![Fp::ZERO; COLUMN_CELLS + 1]).is_err());
    }

    #[test]
    fn checkpoint_binds_all_columns_and_cross_parity_audits() {
        let (checkpoint, public, _) = fixture();
        for eq in [true, false] {
            for index in 0..COLUMN_CELLS {
                let mut changed = checkpoint.clone();
                let column = if eq {
                    &mut changed.0.eq_column
                } else {
                    &mut changed.0.ep_column
                };
                column[index][0] ^= 1;
                assert!(
                    changed.validate_columns(&public).is_err(),
                    "parity Eq={eq}, cell={index}"
                );
            }
        }
        let mut changed = checkpoint.clone();
        core::mem::swap(&mut changed.0.eq_column, &mut changed.0.ep_column);
        assert!(changed.validate_columns(&public).is_err());
        let mut changed = checkpoint.clone();
        changed.0.eq_column[public_instance::EQ_DEFERRED_AUDIT_LO][16] = 1;
        assert!(changed.validate_columns(&public).is_err());
    }

    #[test]
    fn checkpoint_rejects_a_validly_encoded_other_local_history() {
        let (checkpoint, public, lengths) = fixture();
        let mut changed = checkpoint.clone();
        changed.0.eq_history[0] ^= 1;
        changed
            .validate_shape(&checkpoint.0.binding, lengths)
            .expect("different canonical scalar remains shape-only");
        assert!(changed.validate_columns(&public).is_err());
        let mut changed_public = public.clone();
        changed_public.successor.lane.device_lane_id[0] ^= 1;
        // The lifecycle/public state components are independently reconstructed by Core. An
        // exact instance mismatch, such as the state head below, cannot be hidden by valid bytes.
        changed_public.successor.state_commitment[0] ^= 1;
        assert!(checkpoint.validate_columns(&changed_public).is_err());
    }
}
