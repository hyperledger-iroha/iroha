//! Fail-closed boundary for Kagemusha Pasta-cycle recursion.
//!
//! The reviewed Axiom `PoseidonTranscript` hashes in `C::Scalar` and explicitly
//! assumes that field is native to the verifier circuit.  A generic
//! `Halo2Loader` adapter across the Pasta cycle therefore emulates every
//! transcript scalar.  The measured Ep-to-Fp prototype required 39,275,522
//! advice cells and 7,436,318 lookup cells (about 4.1 GiB live RSS); bounded
//! CRT batching and native curve coordinates still required 18,040,862 advice
//! cells, 2,669,809 lookup cells, 100.35 seconds to construct, and
//! 2,414,559,232 bytes peak RSS.  Proof parsing consumed 8,287,023 advice cells
//! and fold-transcript parsing another 5,835,004.  That construction is
//! structurally outside the wallet's 128 MiB preparation gate and is not kept
//! as a production fallback.
//!
//! The compact wire below retains only the newest and predecessor proofs. The
//! fixed verifier derives every transcript challenge, residual coefficient,
//! and IPA accumulator from those proof bytes; none is caller-selected wire
//! data. Tests retain the smallest sound boundary supported by the pinned
//! dependencies: fixed-key Poseidon proof wires for both Pasta parities,
//! canonical BGH19 IPA folding, exact bounded proof bytes, and native terminal
//! decisions. Production availability stays false until the fixed-VK
//! cross-field leapfrog constrains those same operations without generic
//! scalar emulation and passes the complete archive and device gates.
//!
//! ## First-release single-parent contract
//!
//! The ABI-19 release accepts exactly one parent per transition. A fragmented
//! balance is therefore represented by multiple independently spendable notes,
//! never by hashing an unproved second parent into a one-parent proof.
//!
//! Each split must be represented by one recursive transition proof that binds
//! the parent, exact recipient and optional change outputs, and a shared output
//! root. Recipient and change bundles then select distinct leaves with
//! proof-bound membership witnesses. Generating two branch-specific recursive
//! proofs from the same host-validated split is not an acceptable substitute;
//! production remains unavailable until the single-proof boundary is wired.
//!
//! The eventual native API is one coherent proof family: `prove_init_v3`
//! returns a bundle and output-membership witness; `prove_split_v3` accepts
//! one terminally valid parent and returns independently spendable
//! recipient/change bundles with witnesses; `verify_v3` returns only after the
//! native terminal decision; and `prove_redemption_change_v3` returns the
//! optional proof-bound change bundle and witness. No digest-only receipt or
//! host-validated parent is an accepted implementation of those operations.

use iroha_data_model::offline::{
    KAGEMUSHA_RECURSIVE_SPEND_MAX_PROOF_STEPS_V2,
    KAGEMUSHA_RECURSIVE_SPEND_STEP_PUBLIC_INPUT_ROWS_V3, KagemushaPastaCycleParityV3,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

use ff::PrimeField;
use halo2_proofs::halo2curves::pasta::{Fp, Fq};

/// Version of the compact leapfrog proof window.
pub const KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V3: u16 = 3;
/// Exact augmented IPA proof bytes for either fixed Kagemusha step circuit.
///
/// Both parities use the identical degree-12 relation and 1,536-byte proof
/// shape. Truncation, extension, or a future circuit-shape change is a new
/// authenticated release contract, not variable headroom in this wire.
pub const KAGEMUSHA_LEAPFROG_STEP_PROOF_BYTES_V3: usize = 1_536;
/// Exact canonical Norito bytes for the one-proof initialization window.
pub const KAGEMUSHA_LEAPFROG_INIT_WINDOW_BYTES_V3: usize = 2_108;
/// Exact canonical Norito bytes for the steady newest/predecessor window.
///
/// This is the payload embedded in `KagemushaRecursiveSpendProofV2::proof`;
/// statement, branch-conflict, and output-membership data have a separate
/// budget in the complete peer archive.
pub const KAGEMUSHA_LEAPFROG_PROOF_WINDOW_BYTES_V3: usize = 4_172;
/// Domain separator for identities of complete compact proof windows.
pub const KAGEMUSHA_LEAPFROG_PROOF_WINDOW_DIGEST_DOMAIN_V3: &[u8] =
    b"iroha:kagemusha:leapfrog-proof-window:v3";
/// Number of non-zero, source-combined terms in the fixed degree-12 residual.
///
/// This count is extracted from the exact fixed verifier below. A key or
/// circuit shape that changes it requires a new authenticated release and wire
/// schema; accepting a variable residual would make packet-size and circuit
/// shape claims non-reproducible.
pub const KAGEMUSHA_DEFERRED_EQUATION_TERM_COUNT_V3: usize = 38;
/// Domain separator for the cross-layer deferred-equation binding.
pub const KAGEMUSHA_DEFERRED_EQUATION_DIGEST_DOMAIN_V3: &[u8] =
    b"iroha:kagemusha:deferred-equation:v3";

/// One canonical non-zero coefficient in the fixed verifier's point namespace.
///
/// This is prover/circuit material and is never serialized into a peer proof
/// window. The next two circuit layers recompute and bind its digest.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaDeferredEquationTermV3 {
    /// Index into transcript points followed by authenticated fixed-VK points.
    pub point_source_index: u16,
    /// Canonical scalar bytes in the proof curve's scalar field.
    pub coefficient: [u8; 32],
}

/// Complete deterministic residual selected by one fixed proof transcript.
///
/// The native-point half of layer `i + 1` consumes this equation and exposes
/// its digest. The native-scalar half of layer `i + 2` reconstructs the same
/// value from proof `i` and requires digest equality. This joins the two
/// deferred verifier halves without trusting host-provided coefficients.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaDeferredEquationBindingV3 {
    /// Parity of the proof whose residual is described.
    pub parity: KagemushaPastaCycleParityV3,
    /// SHA-256 of the exact augmented proof bytes.
    pub proof_sha256: [u8; 32],
    /// SHA-256 of the exact public-input schema.
    pub public_inputs_schema_sha256: [u8; 32],
    /// SHA-256 of the authenticated fixed verifying key.
    pub verifier_key_sha256: [u8; 32],
    /// SHA-256 of the exact canonical instance columns.
    pub instances_sha256: [u8; 32],
    /// SHA-256 of the authenticated artifact manifest.
    pub manifest_sha256: [u8; 32],
    /// Strictly source-ordered, duplicate-free residual terms.
    pub terms: Vec<KagemushaDeferredEquationTermV3>,
}

fn canonical_nonzero_scalar<F: PrimeField>(bytes: &[u8; 32]) -> bool {
    let mut repr = F::Repr::default();
    if repr.as_ref().len() != bytes.len() {
        return false;
    }
    repr.as_mut().copy_from_slice(bytes);
    Option::<F>::from(F::from_repr(repr)).is_some_and(|value| value != F::ZERO)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

impl KagemushaDeferredEquationBindingV3 {
    /// Validate the exact fixed-verifier equation shape and scalar field.
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.proof_sha256,
            self.public_inputs_schema_sha256,
            self.verifier_key_sha256,
            self.instances_sha256,
            self.manifest_sha256,
        ]
        .contains(&[0; 32])
            || self.terms.len() != KAGEMUSHA_DEFERRED_EQUATION_TERM_COUNT_V3
        {
            return Err("Kagemusha deferred equation binding shape mismatch".to_owned());
        }
        for (index, term) in self.terms.iter().enumerate() {
            if index > 0 && self.terms[index - 1].point_source_index >= term.point_source_index {
                return Err(
                    "Kagemusha deferred equation point sources are not canonical".to_owned(),
                );
            }
            let canonical = match self.parity {
                KagemushaPastaCycleParityV3::StepEq => {
                    canonical_nonzero_scalar::<Fp>(&term.coefficient)
                }
                KagemushaPastaCycleParityV3::StepEp => {
                    canonical_nonzero_scalar::<Fq>(&term.coefficient)
                }
            };
            if !canonical {
                return Err("Kagemusha deferred equation coefficient is invalid".to_owned());
            }
        }
        Ok(())
    }

    /// Return the cross-layer binding digest for this exact residual.
    pub fn digest(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        let encoded = norito::to_bytes(self)
            .map_err(|error| format!("failed to encode Kagemusha deferred equation: {error}"))?;
        let mut hasher = Sha256::new();
        hasher.update(KAGEMUSHA_DEFERRED_EQUATION_DIGEST_DOMAIN_V3);
        hasher.update([0]);
        hasher.update(encoded);
        Ok(hasher.finalize().into())
    }

    /// Bind a reconstructed residual to every terminal-verifier input.
    ///
    /// Callers pass canonical instance bytes, not a host object hash. The
    /// terminal path and the native-scalar leapfrog half must invoke this with
    /// byte-identical encodings so proof, VK, instance, manifest, or transcript
    /// substitution cannot select another residual equation.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_against_context(
        &self,
        expected_parity: KagemushaPastaCycleParityV3,
        proof_bytes: &[u8],
        public_inputs_schema: &[u8],
        verifier_key_bytes: &[u8],
        canonical_instance_bytes: &[u8],
        expected_manifest_sha256: [u8; 32],
        expected_deferred_digest: [u8; 32],
    ) -> Result<(), String> {
        self.validate()?;
        if self.parity != expected_parity
            || self.proof_sha256 != sha256(proof_bytes)
            || self.public_inputs_schema_sha256 != sha256(public_inputs_schema)
            || self.verifier_key_sha256 != sha256(verifier_key_bytes)
            || self.instances_sha256 != sha256(canonical_instance_bytes)
            || self.manifest_sha256 != expected_manifest_sha256
            || self.digest()? != expected_deferred_digest
        {
            return Err("Kagemusha deferred equation context mismatch".to_owned());
        }
        Ok(())
    }
}

/// Exact semantic public-instance column of one alternating-Pasta step proof.
///
/// Every 32-byte value expands to four little-endian `u64` limbs in declaration
/// order, followed by the six integer rows. The direct representation avoids a
/// host-only statement hash and lets either fixed circuit bind every semantic
/// field without a cross-field hash gadget.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaLeapfrogStepPublicInputsV3 {
    /// Canonical digest of the chain identifier.
    pub chain_id_digest: [u8; 32],
    /// Canonical digest of the asset-definition identifier.
    pub asset_definition_id_digest: [u8; 32],
    /// Consumed parent/finalized-shield root.
    pub input_root: [u8; 32],
    /// Fresh proof-bound output root.
    pub final_root: [u8; 32],
    /// Single finalized top-up operation identifier.
    pub topup_operation_id: [u8; 32],
    /// Single finalized top-up anchor digest.
    pub topup_anchor_digest: [u8; 32],
    /// Transition binding/tag digest; zero only for initialization.
    pub transition_binding_digest: [u8; 32],
    /// Nonce-bound receiver request digest; zero outside peer split.
    pub recipient_request_digest: [u8; 32],
    /// Current transition operation identifier; zero only for initialization.
    pub operation_id: [u8; 32],
    /// Consumed bundle digest, present only for redemption-change.
    pub parent_bundle_digest: [u8; 32],
    /// Consumed branch-claim digest; zero only for initialization.
    pub parent_branch_claim_digest: [u8; 32],
    /// SHA-256 of the authenticated artifact manifest used by this proof.
    pub manifest_sha256: [u8; 32],
    /// Canonical digest of the selected verifier-key identifier.
    pub verifier_key_id_digest: [u8; 32],
    /// SHA-256 of the predecessor's exact augmented proof bytes.
    pub predecessor_proof_sha256: [u8; 32],
    /// Deferred-equation digest reconstructed from the predecessor proof.
    pub predecessor_deferred_equation_digest: [u8; 32],
    /// Authoritative asset-definition scale.
    pub asset_scale: u32,
    /// Recursive transition count including initialization.
    pub proof_step_count: u32,
    /// Peer transfer count excluding initialization/redemption-change.
    pub peer_hop_count: u32,
    /// `0` init, `1` peer split, `2` redemption-change.
    pub transition_profile: u8,
    /// Parent recursive transition count; zero only at initialization.
    pub parent_proof_step_count: u32,
    /// Parent peer-transfer count; zero at initialization and valid hop zero.
    pub parent_peer_hop_count: u32,
}

impl KagemushaLeapfrogStepPublicInputsV3 {
    /// Number of scalar rows in the fixed step public-instance column.
    pub const LIMB_COUNT: usize = KAGEMUSHA_RECURSIVE_SPEND_STEP_PUBLIC_INPUT_ROWS_V3;

    /// Expand the fifteen digests into the exact little-endian limb order committed
    /// by both V3 public-input schemas.
    #[must_use]
    pub fn canonical_limbs(&self) -> [u64; Self::LIMB_COUNT] {
        let mut limbs = [0_u64; Self::LIMB_COUNT];
        for (digest_index, digest) in [
            self.chain_id_digest,
            self.asset_definition_id_digest,
            self.input_root,
            self.final_root,
            self.topup_operation_id,
            self.topup_anchor_digest,
            self.transition_binding_digest,
            self.recipient_request_digest,
            self.operation_id,
            self.parent_bundle_digest,
            self.parent_branch_claim_digest,
            self.manifest_sha256,
            self.verifier_key_id_digest,
            self.predecessor_proof_sha256,
            self.predecessor_deferred_equation_digest,
        ]
        .iter()
        .enumerate()
        {
            for (limb_index, chunk) in digest.chunks_exact(8).enumerate() {
                limbs[digest_index * 4 + limb_index] =
                    u64::from_le_bytes(chunk.try_into().expect("eight-byte digest limb"));
            }
        }
        limbs[60] = u64::from(self.asset_scale);
        limbs[61] = u64::from(self.proof_step_count);
        limbs[62] = u64::from(self.peer_hop_count);
        limbs[63] = u64::from(self.transition_profile);
        limbs[64] = u64::from(self.parent_proof_step_count);
        limbs[65] = u64::from(self.parent_peer_hop_count);
        limbs
    }

    /// Construct the exact one-column Pasta instance vector for either parity.
    #[must_use]
    pub fn canonical_instance_column<F: PrimeField + From<u64>>(&self) -> Vec<F> {
        self.canonical_limbs().into_iter().map(F::from).collect()
    }

    /// Canonical byte encoding hashed into a deferred-equation context.
    ///
    /// These are field representations, not host-endian integers, so the
    /// deferred verifier binds the exact scalar column consumed by Halo2.
    #[must_use]
    pub fn canonical_instance_bytes<F: PrimeField + From<u64>>(&self) -> Vec<u8> {
        self.canonical_instance_column::<F>()
            .into_iter()
            .flat_map(|value| value.to_repr().as_ref().to_vec())
            .collect()
    }

    fn validate_semantics(&self) -> Result<(), String> {
        if [
            self.chain_id_digest,
            self.asset_definition_id_digest,
            self.input_root,
            self.final_root,
            self.topup_operation_id,
            self.topup_anchor_digest,
            self.manifest_sha256,
            self.verifier_key_id_digest,
        ]
        .contains(&[0; 32])
            || self.proof_step_count == 0
            || self.transition_profile > 2
            || match self.transition_profile {
                0 => {
                    self.proof_step_count != 1
                        || self.peer_hop_count != 0
                        || self.parent_proof_step_count != 0
                        || self.parent_peer_hop_count != 0
                        || [
                            self.transition_binding_digest,
                            self.recipient_request_digest,
                            self.operation_id,
                            self.parent_bundle_digest,
                            self.parent_branch_claim_digest,
                            self.predecessor_proof_sha256,
                            self.predecessor_deferred_equation_digest,
                        ]
                        .iter()
                        .any(|value| *value != [0; 32])
                }
                1 => {
                    self.proof_step_count != self.parent_proof_step_count.saturating_add(1)
                        || self.peer_hop_count != self.parent_peer_hop_count.saturating_add(1)
                        || [
                            self.transition_binding_digest,
                            self.recipient_request_digest,
                            self.operation_id,
                            self.parent_branch_claim_digest,
                            self.predecessor_proof_sha256,
                            self.predecessor_deferred_equation_digest,
                        ]
                        .contains(&[0; 32])
                        || self.parent_bundle_digest != [0; 32]
                }
                2 => {
                    self.proof_step_count != self.parent_proof_step_count.saturating_add(1)
                        || self.peer_hop_count != self.parent_peer_hop_count
                        || [
                            self.transition_binding_digest,
                            self.operation_id,
                            self.parent_bundle_digest,
                            self.parent_branch_claim_digest,
                            self.predecessor_proof_sha256,
                            self.predecessor_deferred_equation_digest,
                        ]
                        .contains(&[0; 32])
                        || self.recipient_request_digest != [0; 32]
                }
                _ => true,
            }
        {
            return Err("Kagemusha leapfrog step public-input semantics mismatch".to_owned());
        }
        Ok(())
    }
}

/// One fixed-circuit proof retained by the alternating Pasta leapfrog.
///
/// The proof's public instances, fixed verifier key, and authenticated release
/// determine the complete deferred MSM equation. Coefficients, point-source
/// indices, transcript challenges, and IPA accumulator limbs are therefore
/// deliberately absent: accepting caller-serialized copies would both waste
/// the peer budget and permit the circuit and terminal decider to consume
/// different equations.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaLeapfrogStepProofV3 {
    /// Curve/circuit parity of this proof.
    pub parity: KagemushaPastaCycleParityV3,
    /// Exact typed 66-limb public instance column.
    pub public_inputs: KagemushaLeapfrogStepPublicInputsV3,
    /// Ordinary Poseidon Halo2/IPA proof plus the canonical folded generator.
    pub proof_bytes: Vec<u8>,
}

impl KagemushaLeapfrogStepProofV3 {
    /// Validate the bounded, non-empty fixed-circuit wire shape.
    pub fn validate(&self) -> Result<(), String> {
        let expected_parity = if self.public_inputs.proof_step_count % 2 == 1 {
            KagemushaPastaCycleParityV3::StepEq
        } else {
            KagemushaPastaCycleParityV3::StepEp
        };
        self.public_inputs.validate_semantics()?;
        if self.public_inputs.proof_step_count > KAGEMUSHA_RECURSIVE_SPEND_MAX_PROOF_STEPS_V2
            || self.parity != expected_parity
            || self.proof_bytes.is_empty()
            || self.proof_bytes.len() != KAGEMUSHA_LEAPFROG_STEP_PROOF_BYTES_V3
        {
            return Err("Kagemusha leapfrog step proof shape mismatch".to_owned());
        }
        Ok(())
    }
}

/// Constant-size newest/predecessor proof window transported by one bundle.
///
/// Layer `i` proves the application transition and performs the native-point
/// half of proof `i - 1` plus the native-scalar half of proof `i - 2`. The
/// halves are joined by the exact deferred-equation digest exposed by layer
/// `i - 1`. A terminal verifier fully verifies the newest two ordinary proofs;
/// induction then covers every older layer. Initialization is the only
/// single-proof window and is a circuit base case bound to finalized top-up
/// evidence.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaLeapfrogProofWindowV3 {
    /// Wire layout version.
    pub version: u16,
    /// Proof for the current public statement.
    pub newest: KagemushaLeapfrogStepProofV3,
    /// Previous proof, absent only for recursive step one.
    pub predecessor: Option<KagemushaLeapfrogStepProofV3>,
}

fn opposite_parity(parity: KagemushaPastaCycleParityV3) -> KagemushaPastaCycleParityV3 {
    match parity {
        KagemushaPastaCycleParityV3::StepEq => KagemushaPastaCycleParityV3::StepEp,
        KagemushaPastaCycleParityV3::StepEp => KagemushaPastaCycleParityV3::StepEq,
    }
}

impl KagemushaLeapfrogProofWindowV3 {
    /// Validate the exact two-layer window and its canonical archive budget.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V3 {
            return Err("Kagemusha leapfrog proof-window version mismatch".to_owned());
        }
        self.newest.validate()?;
        match (
            &self.predecessor,
            self.newest.public_inputs.proof_step_count,
        ) {
            (None, 1) => {}
            (Some(predecessor), newest_step) if newest_step > 1 => {
                predecessor.validate()?;
                if predecessor.public_inputs.proof_step_count.checked_add(1) != Some(newest_step)
                    || predecessor.parity != opposite_parity(self.newest.parity)
                    || predecessor.proof_bytes == self.newest.proof_bytes
                    || predecessor.public_inputs.manifest_sha256
                        != self.newest.public_inputs.manifest_sha256
                    || predecessor.public_inputs.chain_id_digest
                        != self.newest.public_inputs.chain_id_digest
                    || predecessor.public_inputs.asset_definition_id_digest
                        != self.newest.public_inputs.asset_definition_id_digest
                    || predecessor.public_inputs.asset_scale
                        != self.newest.public_inputs.asset_scale
                    || predecessor.public_inputs.final_root != self.newest.public_inputs.input_root
                    || predecessor.public_inputs.topup_operation_id
                        != self.newest.public_inputs.topup_operation_id
                    || predecessor.public_inputs.topup_anchor_digest
                        != self.newest.public_inputs.topup_anchor_digest
                    || predecessor.public_inputs.proof_step_count
                        != self.newest.public_inputs.parent_proof_step_count
                    || predecessor.public_inputs.peer_hop_count
                        != self.newest.public_inputs.parent_peer_hop_count
                    || sha256(&predecessor.proof_bytes)
                        != self.newest.public_inputs.predecessor_proof_sha256
                {
                    return Err("Kagemusha leapfrog predecessor binding mismatch".to_owned());
                }
            }
            _ => {
                return Err("Kagemusha leapfrog predecessor presence mismatch".to_owned());
            }
        }
        let encoded = norito::to_bytes(self)
            .map_err(|error| format!("failed to encode Kagemusha proof window: {error}"))?;
        let expected_len = if self.predecessor.is_some() {
            KAGEMUSHA_LEAPFROG_PROOF_WINDOW_BYTES_V3
        } else {
            KAGEMUSHA_LEAPFROG_INIT_WINDOW_BYTES_V3
        };
        if encoded.len() != expected_len {
            return Err(format!(
                "Kagemusha leapfrog proof window is {} bytes; expected {}",
                encoded.len(),
                expected_len
            ));
        }
        Ok(())
    }

    /// Construct the next constant-size window from one newly generated proof.
    ///
    /// Cryptographic callers must first prove that `newest` binds the old
    /// window's newest proof digest, deferred equation, result state, manifest,
    /// and application transition. This method only performs the canonical
    /// lossless window rotation after that proof has been generated.
    pub fn advance(previous: &Self, newest: KagemushaLeapfrogStepProofV3) -> Result<Self, String> {
        previous.validate()?;
        newest.validate()?;
        if previous
            .newest
            .public_inputs
            .proof_step_count
            .checked_add(1)
            != Some(newest.public_inputs.proof_step_count)
            || newest.parity != opposite_parity(previous.newest.parity)
            || newest.proof_bytes == previous.newest.proof_bytes
        {
            return Err("Kagemusha leapfrog window advance mismatch".to_owned());
        }
        let window = Self {
            version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V3,
            newest,
            predecessor: Some(previous.newest.clone()),
        };
        window.validate()?;
        Ok(window)
    }

    /// Return a domain-separated identity of the exact canonical window.
    pub fn digest(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        let encoded = norito::to_bytes(self)
            .map_err(|error| format!("failed to encode Kagemusha proof window: {error}"))?;
        let mut hasher = Sha256::new();
        hasher.update(KAGEMUSHA_LEAPFROG_PROOF_WINDOW_DIGEST_DOMAIN_V3);
        hasher.update([0]);
        hasher.update(encoded);
        Ok(hasher.finalize().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::to_bytes;

    use halo2_proofs::{
        arithmetic::Field,
        circuit::{Layouter, SimpleFloorPlanner, Value},
        plonk::{Advice, Circuit, Column, ConstraintSystem, Error as PlonkError, Instance},
    };

    use crate::zk::halo2_backend::assign_advice_compat;

    fn leapfrog_step(
        parity: KagemushaPastaCycleParityV3,
        proof_step_count: u32,
        byte: u8,
        predecessor: Option<&KagemushaLeapfrogStepProofV3>,
    ) -> KagemushaLeapfrogStepProofV3 {
        let (
            input_root,
            parent_proof_step_count,
            parent_peer_hop_count,
            predecessor_proof_sha256,
            predecessor_deferred_equation_digest,
        ) = predecessor.map_or(([0x20; 32], 0, 0, [0; 32], [0; 32]), |predecessor| {
            (
                predecessor.public_inputs.final_root,
                predecessor.public_inputs.proof_step_count,
                predecessor.public_inputs.peer_hop_count,
                sha256(&predecessor.proof_bytes),
                sha256(
                    &[
                        b"test-deferred".as_slice(),
                        predecessor.proof_bytes.as_slice(),
                    ]
                    .concat(),
                ),
            )
        });
        let is_init = predecessor.is_none();
        KagemushaLeapfrogStepProofV3 {
            parity,
            public_inputs: KagemushaLeapfrogStepPublicInputsV3 {
                chain_id_digest: [0x11; 32],
                asset_definition_id_digest: [0x12; 32],
                input_root,
                final_root: [byte.wrapping_add(0x40); 32],
                topup_operation_id: [0x13; 32],
                topup_anchor_digest: [0x14; 32],
                transition_binding_digest: if is_init { [0; 32] } else { [byte; 32] },
                recipient_request_digest: if is_init { [0; 32] } else { [0x15; 32] },
                operation_id: if is_init { [0; 32] } else { [0x16; 32] },
                parent_bundle_digest: [0; 32],
                parent_branch_claim_digest: if is_init { [0; 32] } else { [0x17; 32] },
                manifest_sha256: [0xEE; 32],
                verifier_key_id_digest: [0x18; 32],
                predecessor_proof_sha256,
                predecessor_deferred_equation_digest,
                asset_scale: 9,
                proof_step_count,
                peer_hop_count: proof_step_count - 1,
                transition_profile: u8::from(!is_init),
                parent_proof_step_count,
                parent_peer_hop_count,
            },
            proof_bytes: vec![byte; 1_536],
        }
    }

    #[test]
    fn typed_step_public_inputs_have_one_canonical_little_endian_instance_column() {
        let mut inputs = KagemushaLeapfrogStepPublicInputsV3 {
            chain_id_digest: [0; 32],
            asset_definition_id_digest: [0; 32],
            input_root: [0; 32],
            final_root: [0; 32],
            topup_operation_id: [0; 32],
            topup_anchor_digest: [0; 32],
            transition_binding_digest: [0; 32],
            recipient_request_digest: [0; 32],
            operation_id: [0; 32],
            parent_bundle_digest: [0; 32],
            parent_branch_claim_digest: [0; 32],
            manifest_sha256: [0; 32],
            verifier_key_id_digest: [0; 32],
            predecessor_proof_sha256: [0; 32],
            predecessor_deferred_equation_digest: [0; 32],
            asset_scale: 9,
            proof_step_count: 2,
            peer_hop_count: 1,
            transition_profile: 1,
            parent_proof_step_count: 1,
            parent_peer_hop_count: 0,
        };
        for (digest_index, digest) in [
            &mut inputs.chain_id_digest,
            &mut inputs.asset_definition_id_digest,
            &mut inputs.input_root,
            &mut inputs.final_root,
            &mut inputs.topup_operation_id,
            &mut inputs.topup_anchor_digest,
            &mut inputs.transition_binding_digest,
            &mut inputs.recipient_request_digest,
            &mut inputs.operation_id,
            &mut inputs.parent_bundle_digest,
            &mut inputs.parent_branch_claim_digest,
            &mut inputs.manifest_sha256,
            &mut inputs.verifier_key_id_digest,
            &mut inputs.predecessor_proof_sha256,
            &mut inputs.predecessor_deferred_equation_digest,
        ]
        .into_iter()
        .enumerate()
        {
            for (byte_index, byte) in digest.iter_mut().enumerate() {
                *byte = u8::try_from((digest_index * 32 + byte_index) % 256).expect("fixture byte");
            }
        }

        let limbs = inputs.canonical_limbs();
        assert_eq!(limbs.len(), KagemushaLeapfrogStepPublicInputsV3::LIMB_COUNT);
        assert_eq!(limbs[0], 0x0706_0504_0302_0100);
        assert_eq!(limbs[4], 0x2726_2524_2322_2120);
        assert_eq!(limbs[59], 0xdfde_dddc_dbda_d9d8);
        assert_eq!(&limbs[60..], &[9, 2, 1, 1, 1, 0]);

        let eq_column = inputs.canonical_instance_column::<Fp>();
        let ep_column = inputs.canonical_instance_column::<Fq>();
        assert_eq!(eq_column.len(), 66);
        assert_eq!(ep_column.len(), 66);
        assert_eq!(eq_column[0], Fp::from(limbs[0]));
        assert_eq!(ep_column[0], Fq::from(limbs[0]));

        let mut endian_substitution = inputs.clone();
        endian_substitution.chain_id_digest[..8].reverse();
        assert_ne!(
            inputs.canonical_instance_bytes::<Fp>(),
            endian_substitution.canonical_instance_bytes::<Fp>(),
            "changing limb endianness must select different proof instances"
        );
    }

    #[test]
    fn compact_leapfrog_window_is_constant_through_step_65() {
        let mut window = KagemushaLeapfrogProofWindowV3 {
            version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V3,
            newest: leapfrog_step(KagemushaPastaCycleParityV3::StepEq, 1, 1, None),
            predecessor: None,
        };
        window.validate().expect("valid initialization window");
        let init_size = to_bytes(&window).expect("encode init window").len();
        assert_eq!(init_size, KAGEMUSHA_LEAPFROG_INIT_WINDOW_BYTES_V3);

        let mut steady_size = None;
        for step in 2_u32..=KAGEMUSHA_RECURSIVE_SPEND_MAX_PROOF_STEPS_V2 {
            let parity = opposite_parity(window.newest.parity);
            window = KagemushaLeapfrogProofWindowV3::advance(
                &window,
                leapfrog_step(
                    parity,
                    step,
                    u8::try_from(step).expect("bounded step"),
                    Some(&window.newest),
                ),
            )
            .expect("advance leapfrog window");
            let encoded = to_bytes(&window).expect("encode steady window");
            assert!(encoded.len() > init_size);
            assert_eq!(encoded.len(), KAGEMUSHA_LEAPFROG_PROOF_WINDOW_BYTES_V3);
            assert_eq!(
                *steady_size.get_or_insert(encoded.len()),
                encoded.len(),
                "the proof window must not grow with recursive depth"
            );
            assert_eq!(
                window
                    .predecessor
                    .as_ref()
                    .expect("predecessor")
                    .public_inputs
                    .proof_step_count,
                step - 1
            );
        }
    }

    #[test]
    fn compact_leapfrog_window_rejects_parity_step_and_proof_substitution() {
        let init = KagemushaLeapfrogProofWindowV3 {
            version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V3,
            newest: leapfrog_step(KagemushaPastaCycleParityV3::StepEq, 1, 1, None),
            predecessor: None,
        };
        let valid = KagemushaLeapfrogProofWindowV3::advance(
            &init,
            leapfrog_step(
                KagemushaPastaCycleParityV3::StepEp,
                2,
                2,
                Some(&init.newest),
            ),
        )
        .expect("valid second layer");

        let mut wrong_version = valid.clone();
        wrong_version.version = wrong_version.version.saturating_add(1);
        assert!(wrong_version.validate().is_err());

        let mut missing_predecessor = valid.clone();
        missing_predecessor.predecessor = None;
        assert!(missing_predecessor.validate().is_err());

        let mut wrong_step = valid.clone();
        wrong_step
            .predecessor
            .as_mut()
            .expect("predecessor")
            .public_inputs
            .proof_step_count = 2;
        assert!(wrong_step.validate().is_err());

        let mut wrong_parity = valid.clone();
        wrong_parity
            .predecessor
            .as_mut()
            .expect("predecessor")
            .parity = KagemushaPastaCycleParityV3::StepEp;
        assert!(wrong_parity.validate().is_err());

        let mut duplicated_proof = valid.clone();
        let newest_proof = duplicated_proof.newest.proof_bytes.clone();
        duplicated_proof
            .predecessor
            .as_mut()
            .expect("predecessor")
            .proof_bytes = newest_proof;
        assert!(duplicated_proof.validate().is_err());

        let mut wrong_manifest = valid.clone();
        wrong_manifest
            .predecessor
            .as_mut()
            .expect("predecessor")
            .public_inputs
            .manifest_sha256[0] ^= 1;
        assert!(wrong_manifest.validate().is_err());

        let mut wrong_state = valid.clone();
        wrong_state.newest.public_inputs.input_root[0] ^= 1;
        assert!(wrong_state.validate().is_err());

        let mut wrong_predecessor_digest = valid.clone();
        wrong_predecessor_digest
            .newest
            .public_inputs
            .predecessor_proof_sha256[0] ^= 1;
        assert!(wrong_predecessor_digest.validate().is_err());

        let original_digest = valid.digest().expect("valid digest");
        let mut truncated = valid.clone();
        truncated
            .predecessor
            .as_mut()
            .expect("predecessor")
            .proof_bytes
            .pop();
        assert!(truncated.validate().is_err());

        let mut substituted = valid.clone();
        substituted.newest.proof_bytes[0] ^= 1;
        assert_ne!(
            original_digest,
            substituted
                .digest()
                .expect("substituted window remains shaped")
        );

        let mut predecessor_substituted = valid;
        predecessor_substituted
            .predecessor
            .as_mut()
            .expect("predecessor")
            .proof_bytes[0] ^= 1;
        assert!(
            predecessor_substituted.validate().is_err(),
            "predecessor substitution must break the newest proof-digest binding"
        );
    }

    #[test]
    fn compact_leapfrog_window_rejects_per_step_and_total_budget_overflow() {
        let mut oversized_step = leapfrog_step(KagemushaPastaCycleParityV3::StepEq, 1, 0xA5, None);
        oversized_step.proof_bytes.push(0);
        let oversized = KagemushaLeapfrogProofWindowV3 {
            version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V3,
            newest: oversized_step,
            predecessor: None,
        };
        assert!(oversized.validate().is_err());

        let mut excessive_depth = leapfrog_step(
            KagemushaPastaCycleParityV3::StepEp,
            KAGEMUSHA_RECURSIVE_SPEND_MAX_PROOF_STEPS_V2 + 1,
            0xA6,
            None,
        );
        excessive_depth.public_inputs.predecessor_proof_sha256 = [0x11; 32];
        excessive_depth
            .public_inputs
            .predecessor_deferred_equation_digest = [0x12; 32];
        assert!(excessive_depth.validate().is_err());

        let predecessor = leapfrog_step(KagemushaPastaCycleParityV3::StepEq, 1, 0x5A, None);
        let newest = leapfrog_step(
            KagemushaPastaCycleParityV3::StepEp,
            2,
            0xA5,
            Some(&predecessor),
        );
        let maximum = KagemushaLeapfrogProofWindowV3 {
            version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V3,
            newest,
            predecessor: Some(predecessor),
        };
        let encoded_len = to_bytes(&maximum).expect("encode maximum window").len();
        assert!(
            encoded_len == KAGEMUSHA_LEAPFROG_PROOF_WINDOW_BYTES_V3,
            "fixed step proofs must produce the exact complete window: {encoded_len}"
        );
        maximum.validate().expect("bounded maximum window");
    }

    fn deferred_equation(
        parity: KagemushaPastaCycleParityV3,
    ) -> KagemushaDeferredEquationBindingV3 {
        let terms = (0..KAGEMUSHA_DEFERRED_EQUATION_TERM_COUNT_V3)
            .map(|index| {
                let mut coefficient = [0_u8; 32];
                match parity {
                    KagemushaPastaCycleParityV3::StepEq => {
                        let repr =
                            Fp::from(u64::try_from(index + 1).expect("bounded term")).to_repr();
                        coefficient.copy_from_slice(repr.as_ref());
                    }
                    KagemushaPastaCycleParityV3::StepEp => {
                        let repr =
                            Fq::from(u64::try_from(index + 1).expect("bounded term")).to_repr();
                        coefficient.copy_from_slice(repr.as_ref());
                    }
                }
                KagemushaDeferredEquationTermV3 {
                    point_source_index: u16::try_from(index).expect("bounded source"),
                    coefficient,
                }
            })
            .collect();
        KagemushaDeferredEquationBindingV3 {
            parity,
            proof_sha256: sha256(b"proof"),
            public_inputs_schema_sha256: sha256(b"schema"),
            verifier_key_sha256: sha256(b"verifier-key"),
            instances_sha256: sha256(b"instances"),
            manifest_sha256: [5; 32],
            terms,
        }
    }

    #[test]
    fn deferred_equation_digest_rejects_omission_reordering_and_substitution() {
        for parity in [
            KagemushaPastaCycleParityV3::StepEq,
            KagemushaPastaCycleParityV3::StepEp,
        ] {
            let binding = deferred_equation(parity);
            binding.validate().expect("canonical deferred equation");
            let digest = binding.digest().expect("deferred equation digest");

            let mut omitted = binding.clone();
            omitted.terms.pop();
            assert!(omitted.validate().is_err());

            let mut duplicate_source = binding.clone();
            duplicate_source.terms[1].point_source_index =
                duplicate_source.terms[0].point_source_index;
            assert!(duplicate_source.validate().is_err());

            let mut reordered = binding.clone();
            reordered.terms.swap(0, 1);
            assert!(reordered.validate().is_err());

            let mut noncanonical = binding.clone();
            noncanonical.terms[0].coefficient = [0xFF; 32];
            assert!(noncanonical.validate().is_err());

            let mut zero = binding.clone();
            zero.terms[0].coefficient = [0; 32];
            assert!(zero.validate().is_err());

            let mut substituted = binding;
            substituted.proof_sha256[0] ^= 1;
            assert_ne!(digest, substituted.digest().expect("bound substitution"));
        }
    }

    #[test]
    fn deferred_equation_context_rejects_every_terminal_substitution() {
        let binding = deferred_equation(KagemushaPastaCycleParityV3::StepEq);
        let digest = binding.digest().expect("canonical deferred digest");
        let validate =
            |parity, proof: &[u8], schema: &[u8], vk: &[u8], instances: &[u8], manifest| {
                binding.validate_against_context(
                    parity, proof, schema, vk, instances, manifest, digest,
                )
            };
        validate(
            KagemushaPastaCycleParityV3::StepEq,
            b"proof",
            b"schema",
            b"verifier-key",
            b"instances",
            [5; 32],
        )
        .expect("exact deferred context");

        assert!(
            validate(
                KagemushaPastaCycleParityV3::StepEp,
                b"proof",
                b"schema",
                b"verifier-key",
                b"instances",
                [5; 32],
            )
            .is_err(),
            "parity substitution must reject"
        );
        for (proof, schema, vk, instances, manifest, label) in [
            (
                b"proof-substituted".as_slice(),
                b"schema".as_slice(),
                b"verifier-key".as_slice(),
                b"instances".as_slice(),
                [5; 32],
                "proof",
            ),
            (
                b"proof".as_slice(),
                b"schema-substituted".as_slice(),
                b"verifier-key".as_slice(),
                b"instances".as_slice(),
                [5; 32],
                "schema",
            ),
            (
                b"proof".as_slice(),
                b"schema".as_slice(),
                b"verifier-key-substituted".as_slice(),
                b"instances".as_slice(),
                [5; 32],
                "verifier key",
            ),
            (
                b"proof".as_slice(),
                b"schema".as_slice(),
                b"verifier-key".as_slice(),
                b"instances-substituted".as_slice(),
                [5; 32],
                "instances",
            ),
            (
                b"proof".as_slice(),
                b"schema".as_slice(),
                b"verifier-key".as_slice(),
                b"instances".as_slice(),
                [6; 32],
                "manifest",
            ),
        ] {
            assert!(
                validate(
                    KagemushaPastaCycleParityV3::StepEq,
                    proof,
                    schema,
                    vk,
                    instances,
                    manifest,
                )
                .is_err(),
                "{label} substitution must reject"
            );
        }

        assert!(
            binding
                .validate_against_context(
                    KagemushaPastaCycleParityV3::StepEq,
                    b"proof",
                    b"schema",
                    b"verifier-key",
                    b"instances",
                    [5; 32],
                    [0xFF; 32],
                )
                .is_err(),
            "deferred-digest mismatch must reject"
        );
    }

    /// Native-value loader which preserves every MSM as a canonical linear
    /// equation instead of evaluating it away.  This is audit instrumentation
    /// for the fixed-VK deferred-verifier wire: scalar arithmetic remains the
    /// exact field arithmetic used by `snark-verifier`, while every curve
    /// assertion records the complete base/coefficient vector that the
    /// opposite-field circuit would have to authenticate.
    mod deferred_audit {
        use std::{
            cell::RefCell,
            fmt,
            io::Read,
            marker::PhantomData,
            ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
            rc::Rc,
        };

        use snark_verifier::{
            Error,
            loader::{EcPointLoader, LoadedEcPoint, LoadedScalar, Loader, ScalarLoader},
            util::{
                arithmetic::{
                    Curve, CurveAffine, Field, FieldExt, FieldOps, Group, PrimeField, fe_to_fe,
                },
                hash::Poseidon,
                transcript::{Transcript, TranscriptRead},
            },
        };

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(super) struct EquationTerm {
            pub(super) point: Vec<u8>,
            pub(super) coefficient: Vec<u8>,
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(super) struct Equation {
            pub(super) annotation: String,
            pub(super) terms: Vec<EquationTerm>,
        }

        struct State {
            equations: Vec<Equation>,
        }

        #[derive(Clone)]
        pub(super) struct RecordingLoader<C: CurveAffine> {
            state: Rc<RefCell<State>>,
            _curve: PhantomData<C>,
        }

        impl<C: CurveAffine> fmt::Debug for RecordingLoader<C> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("RecordingLoader").finish_non_exhaustive()
            }
        }

        impl<C: CurveAffine> RecordingLoader<C> {
            pub(super) fn new() -> Self {
                Self {
                    state: Rc::new(RefCell::new(State {
                        equations: Vec::new(),
                    })),
                    _curve: PhantomData,
                }
            }

            pub(super) fn equations(&self) -> Vec<Equation> {
                self.state.borrow().equations.clone()
            }

            fn same(&self, other: &Self) {
                assert!(
                    Rc::ptr_eq(&self.state, &other.state),
                    "deferred audit values cannot cross loader instances"
                );
            }
        }

        #[derive(Clone)]
        pub(super) struct RecordedScalar<C: CurveAffine> {
            value: C::Scalar,
            loader: RecordingLoader<C>,
        }

        impl<C: CurveAffine> fmt::Debug for RecordedScalar<C> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple("RecordedScalar").field(&self.value).finish()
            }
        }

        impl<C: CurveAffine> PartialEq for RecordedScalar<C> {
            fn eq(&self, other: &Self) -> bool {
                self.loader.same(&other.loader);
                self.value == other.value
            }
        }

        impl<C: CurveAffine> RecordedScalar<C> {
            pub(super) fn canonical_bytes(&self) -> Vec<u8> {
                self.value.to_repr().as_ref().to_vec()
            }
        }

        macro_rules! scalar_binop {
            ($trait:ident, $method:ident, $assign_trait:ident, $assign_method:ident, $op:tt) => {
                impl<C: CurveAffine> $trait for RecordedScalar<C> {
                    type Output = Self;

                    fn $method(mut self, rhs: Self) -> Self::Output {
                        self.loader.same(&rhs.loader);
                        self.value = self.value $op rhs.value;
                        self
                    }
                }

                impl<C: CurveAffine> $trait<&Self> for RecordedScalar<C> {
                    type Output = Self;

                    fn $method(mut self, rhs: &Self) -> Self::Output {
                        self.loader.same(&rhs.loader);
                        self.value = self.value $op rhs.value;
                        self
                    }
                }

                impl<C: CurveAffine> $assign_trait for RecordedScalar<C> {
                    fn $assign_method(&mut self, rhs: Self) {
                        self.loader.same(&rhs.loader);
                        self.value = self.value $op rhs.value;
                    }
                }

                impl<C: CurveAffine> $assign_trait<&Self> for RecordedScalar<C> {
                    fn $assign_method(&mut self, rhs: &Self) {
                        self.loader.same(&rhs.loader);
                        self.value = self.value $op rhs.value;
                    }
                }
            };
        }

        scalar_binop!(Add, add, AddAssign, add_assign, +);
        scalar_binop!(Sub, sub, SubAssign, sub_assign, -);
        scalar_binop!(Mul, mul, MulAssign, mul_assign, *);

        impl<C: CurveAffine> Neg for RecordedScalar<C> {
            type Output = Self;

            fn neg(mut self) -> Self::Output {
                self.value = -self.value;
                self
            }
        }

        impl<C: CurveAffine> FieldOps for RecordedScalar<C> {
            fn invert(&self) -> Option<Self> {
                Option::<C::Scalar>::from(Field::invert(&self.value)).map(|value| Self {
                    value,
                    loader: self.loader.clone(),
                })
            }
        }

        impl<C: CurveAffine> LoadedScalar<C::Scalar> for RecordedScalar<C> {
            type Loader = RecordingLoader<C>;

            fn loader(&self) -> &Self::Loader {
                &self.loader
            }

            fn pow_var(&self, exp: &Self, _: usize) -> Self {
                self.loader.same(&exp.loader);
                let repr = exp.value.to_repr();
                let mut limbs = Vec::with_capacity(repr.as_ref().len().div_ceil(8));
                for chunk in repr.as_ref().chunks(8) {
                    let mut limb = [0_u8; 8];
                    limb[..chunk.len()].copy_from_slice(chunk);
                    limbs.push(u64::from_le_bytes(limb));
                }
                Self {
                    value: self.value.pow_vartime(limbs),
                    loader: self.loader.clone(),
                }
            }
        }

        #[derive(Clone)]
        struct LinearTerm<C: CurveAffine> {
            point: C,
            coefficient: C::Scalar,
        }

        #[derive(Clone)]
        pub(super) struct RecordedPoint<C: CurveAffine> {
            value: C,
            terms: Vec<LinearTerm<C>>,
            loader: RecordingLoader<C>,
        }

        impl<C: CurveAffine> fmt::Debug for RecordedPoint<C> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("RecordedPoint")
                    .field("value", &self.value)
                    .field("terms", &self.terms.len())
                    .finish()
            }
        }

        impl<C: CurveAffine> PartialEq for RecordedPoint<C> {
            fn eq(&self, other: &Self) -> bool {
                self.loader.same(&other.loader);
                self.value == other.value
            }
        }

        impl<C: CurveAffine> RecordedPoint<C> {
            pub(super) fn canonical_bytes(&self) -> Vec<u8> {
                self.value.to_bytes().as_ref().to_vec()
            }
        }

        impl<C: CurveAffine> LoadedEcPoint<C> for RecordedPoint<C> {
            type Loader = RecordingLoader<C>;

            fn loader(&self) -> &Self::Loader {
                &self.loader
            }
        }

        fn push_term<C: CurveAffine>(
            terms: &mut Vec<LinearTerm<C>>,
            point: C,
            coefficient: C::Scalar,
        ) {
            if coefficient == C::Scalar::ZERO {
                return;
            }
            if let Some(existing) = terms.iter_mut().find(|term| term.point == point) {
                existing.coefficient += coefficient;
                if existing.coefficient == C::Scalar::ZERO {
                    let index = terms
                        .iter()
                        .position(|term| term.point == point)
                        .expect("existing term index");
                    terms.remove(index);
                }
            } else {
                terms.push(LinearTerm { point, coefficient });
            }
        }

        impl<C: CurveAffine> ScalarLoader<C::Scalar> for RecordingLoader<C> {
            type LoadedScalar = RecordedScalar<C>;

            fn load_const(&self, value: &C::Scalar) -> Self::LoadedScalar {
                RecordedScalar {
                    value: *value,
                    loader: self.clone(),
                }
            }

            fn assert_eq(
                &self,
                annotation: &str,
                lhs: &Self::LoadedScalar,
                rhs: &Self::LoadedScalar,
            ) {
                lhs.loader.same(self);
                rhs.loader.same(self);
                assert_eq!(lhs.value, rhs.value, "{annotation}");
            }
        }

        impl<C: CurveAffine> EcPointLoader<C> for RecordingLoader<C> {
            type LoadedEcPoint = RecordedPoint<C>;

            fn ec_point_load_const(&self, value: &C) -> Self::LoadedEcPoint {
                RecordedPoint {
                    value: *value,
                    terms: vec![LinearTerm {
                        point: *value,
                        coefficient: C::Scalar::ONE,
                    }],
                    loader: self.clone(),
                }
            }

            fn ec_point_assert_eq(
                &self,
                annotation: &str,
                lhs: &Self::LoadedEcPoint,
                rhs: &Self::LoadedEcPoint,
            ) {
                lhs.loader.same(self);
                rhs.loader.same(self);
                assert_eq!(lhs.value, rhs.value, "{annotation}");
                let mut terms = Vec::new();
                for term in &lhs.terms {
                    push_term(&mut terms, term.point, term.coefficient);
                }
                for term in &rhs.terms {
                    push_term(&mut terms, term.point, -term.coefficient);
                }
                let terms = terms
                    .into_iter()
                    .map(|term| EquationTerm {
                        point: term.point.to_bytes().as_ref().to_vec(),
                        coefficient: term.coefficient.to_repr().as_ref().to_vec(),
                    })
                    .collect();
                self.state.borrow_mut().equations.push(Equation {
                    annotation: annotation.to_owned(),
                    terms,
                });
            }

            fn multi_scalar_multiplication(
                pairs: &[(
                    &<Self as ScalarLoader<C::Scalar>>::LoadedScalar,
                    &Self::LoadedEcPoint,
                )],
            ) -> Self::LoadedEcPoint {
                let (first_scalar, first_point) = pairs.first().expect("non-empty MSM");
                let loader = first_scalar.loader.clone();
                first_point.loader.same(&loader);
                let mut value = C::Curve::identity();
                let mut terms = Vec::new();
                for (scalar, point) in pairs {
                    scalar.loader.same(&loader);
                    point.loader.same(&loader);
                    value += point.value * scalar.value;
                    for term in &point.terms {
                        push_term(&mut terms, term.point, term.coefficient * scalar.value);
                    }
                }
                RecordedPoint {
                    value: value.to_affine(),
                    terms,
                    loader,
                }
            }
        }

        impl<C: CurveAffine> Loader<C> for RecordingLoader<C> {}

        pub(super) struct RecordingPoseidonTranscript<
            C: CurveAffine,
            R,
            const T: usize,
            const RATE: usize,
            const R_F: usize,
            const R_P: usize,
        > {
            loader: RecordingLoader<C>,
            stream: R,
            poseidon: Poseidon<C::Scalar, RecordedScalar<C>, T, RATE>,
            pub(super) scalar_count: usize,
            pub(super) point_count: usize,
            pub(super) scalar_offsets: Vec<usize>,
            pub(super) point_sources: Vec<Vec<u8>>,
            bytes_read: usize,
        }

        impl<
            C: CurveAffine,
            R,
            const T: usize,
            const RATE: usize,
            const R_F: usize,
            const R_P: usize,
        > RecordingPoseidonTranscript<C, R, T, RATE, R_F, R_P>
        where
            C::Scalar: FieldExt,
        {
            pub(super) fn new<const SECURE_MDS: usize>(
                loader: RecordingLoader<C>,
                stream: R,
            ) -> Self {
                let poseidon = Poseidon::new::<R_F, R_P, SECURE_MDS>(&loader);
                Self {
                    loader,
                    stream,
                    poseidon,
                    scalar_count: 0,
                    point_count: 0,
                    scalar_offsets: Vec::new(),
                    point_sources: Vec::new(),
                    bytes_read: 0,
                }
            }
        }

        impl<
            C: CurveAffine,
            R,
            const T: usize,
            const RATE: usize,
            const R_F: usize,
            const R_P: usize,
        > Transcript<C, RecordingLoader<C>> for RecordingPoseidonTranscript<C, R, T, RATE, R_F, R_P>
        where
            C::Scalar: FieldExt,
        {
            fn loader(&self) -> &RecordingLoader<C> {
                &self.loader
            }

            fn squeeze_challenge(&mut self) -> RecordedScalar<C> {
                self.poseidon.squeeze()
            }

            fn common_ec_point(&mut self, point: &RecordedPoint<C>) -> Result<(), Error> {
                point.loader.same(&self.loader);
                let coordinates: Option<snark_verifier::util::arithmetic::Coordinates<C>> =
                    point.value.coordinates().into();
                let coordinates = coordinates.ok_or_else(|| {
                    Error::Transcript(
                        std::io::ErrorKind::InvalidData,
                        "identity point cannot enter the Poseidon transcript".to_owned(),
                    )
                })?;
                let x = self.loader.load_const(&fe_to_fe(*coordinates.x()));
                let y = self.loader.load_const(&fe_to_fe(*coordinates.y()));
                self.poseidon.update(&[x, y]);
                Ok(())
            }

            fn common_scalar(&mut self, scalar: &RecordedScalar<C>) -> Result<(), Error> {
                scalar.loader.same(&self.loader);
                self.poseidon.update(std::slice::from_ref(scalar));
                Ok(())
            }
        }

        impl<
            C: CurveAffine,
            R: Read,
            const T: usize,
            const RATE: usize,
            const R_F: usize,
            const R_P: usize,
        > TranscriptRead<C, RecordingLoader<C>>
            for RecordingPoseidonTranscript<C, R, T, RATE, R_F, R_P>
        where
            C::Scalar: FieldExt,
        {
            fn read_scalar(&mut self) -> Result<RecordedScalar<C>, Error> {
                self.scalar_offsets.push(self.bytes_read);
                let mut repr = <C::Scalar as PrimeField>::Repr::default();
                self.stream.read_exact(repr.as_mut()).map_err(|error| {
                    Error::Transcript(error.kind(), "truncated scalar field".to_owned())
                })?;
                self.bytes_read = self
                    .bytes_read
                    .checked_add(repr.as_ref().len())
                    .expect("proof transcript byte count fits usize");
                let value = C::Scalar::from_repr_vartime(repr).ok_or_else(|| {
                    Error::Transcript(
                        std::io::ErrorKind::InvalidData,
                        "non-canonical scalar field".to_owned(),
                    )
                })?;
                let value = self.loader.load_const(&value);
                self.common_scalar(&value)?;
                self.scalar_count += 1;
                Ok(value)
            }

            fn read_ec_point(&mut self) -> Result<RecordedPoint<C>, Error> {
                let mut repr = C::Repr::default();
                self.stream.read_exact(repr.as_mut()).map_err(|error| {
                    Error::Transcript(error.kind(), "truncated curve point".to_owned())
                })?;
                self.bytes_read = self
                    .bytes_read
                    .checked_add(repr.as_ref().len())
                    .expect("proof transcript byte count fits usize");
                let value = Option::<C>::from(C::from_bytes(&repr)).ok_or_else(|| {
                    Error::Transcript(
                        std::io::ErrorKind::InvalidData,
                        "non-canonical curve point".to_owned(),
                    )
                })?;
                self.point_sources.push(repr.as_ref().to_vec());
                let value = self.loader.ec_point_load_const(&value);
                self.common_ec_point(&value)?;
                self.point_count += 1;
                Ok(value)
            }
        }
    }

    #[derive(Clone, Default)]
    struct PublicValue<F: Field> {
        value: F,
    }

    impl<F: Field> Circuit<F> for PublicValue<F> {
        type Config = (Column<Advice>, Column<Instance>);
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();

        fn without_witnesses(&self) -> Self {
            Self::default()
        }

        fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
            let advice = meta.advice_column();
            let instance = meta.instance_column();
            meta.enable_equality(advice);
            meta.enable_equality(instance);
            (advice, instance)
        }

        fn synthesize(
            &self,
            (advice, instance): Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), PlonkError> {
            let cell = layouter.assign_region(
                || "public value",
                |mut region| {
                    let cell = assign_advice_compat(
                        &mut region,
                        || "value",
                        advice,
                        0,
                        || Value::known(self.value),
                    )?;
                    Ok(cell.cell())
                },
            )?;
            layouter.constrain_instance(cell, instance, 0);
            Ok(())
        }
    }

    /// Fixed-key consistency, interoperability, and soundness checks for the Eq proof/fold wire.
    mod pasta_ipa_poseidon_wire {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        use super::super::{
            KAGEMUSHA_LEAPFROG_PROOF_WINDOW_BYTES_V3, KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V3,
            KAGEMUSHA_LEAPFROG_STEP_PROOF_BYTES_V3, KagemushaLeapfrogProofWindowV3,
            KagemushaPastaCycleParityV3, sha256,
        };

        use halo2_base::halo2_proofs::{
            halo2curves::{
                CurveExt as _,
                group::{Curve as _, GroupEncoding},
                pasta::{Eq, EqAffine, Fp},
            },
            plonk::{Circuit, ProvingKey, create_proof, verify_proof},
            poly::{
                VerificationStrategy as _,
                commitment::{Params as _, ParamsProver as _},
                ipa::{
                    commitment::{IPACommitmentScheme, ParamsIPA},
                    multiopen::{ProverIPA, VerifierIPA},
                },
            },
        };
        use rand_core_06::OsRng;
        use snark_verifier::{
            loader::ScalarLoader,
            loader::native::NativeLoader,
            pcs::{
                AccumulationDecider, AccumulationScheme, AccumulationSchemeProver,
                ipa::{
                    Bgh19, IpaAccumulator, IpaAs, IpaDecidingKey, IpaProvingKey,
                    IpaSuccinctVerifyingKey,
                },
            },
            system::halo2::{
                Config, compile,
                strategy::ipa::SingleStrategy as FoldedGeneratorStrategy,
                transcript::halo2::{ChallengeScalar, PoseidonTranscript, TranscriptObject},
            },
            util::arithmetic::{Domain, root_of_unity},
            verifier::{
                SnarkVerifier,
                plonk::{PlonkSuccinctVerifier, PlonkVerifier},
            },
        };

        use super::PublicValue;
        use super::deferred_audit::{RecordingLoader, RecordingPoseidonTranscript};
        use crate::zk::halo2_backend::{Scalar, keygen_pk, keygen_vk, params_new};
        use snark_verifier::util::arithmetic::PrimeCurveAffine as _;

        const T: usize = 3;
        const RATE: usize = 2;
        const R_F: usize = 8;
        const R_P: usize = 57;
        const SECURE_MDS: usize = 0;
        const INNER_K: u32 = 5;

        type As = IpaAs<EqAffine, Bgh19>;
        type FullVerifier = PlonkVerifier<As>;
        type SuccinctVerifier = PlonkSuccinctVerifier<As>;
        type Transcript<L, S> = PoseidonTranscript<EqAffine, L, S, T, RATE, R_F, R_P>;

        struct Fixture {
            params: ParamsIPA<EqAffine>,
            protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
            deciding_key: IpaDecidingKey<EqAffine>,
            proof_without_folded_generator: Vec<u8>,
            augmented_proof: Vec<u8>,
            instances: Vec<Vec<Fp>>,
        }

        fn canonical_svk(params: &ParamsIPA<EqAffine>) -> IpaSuccinctVerifyingKey<EqAffine> {
            let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
            let w = hash_to_curve(&[1]).to_affine();
            let u = hash_to_curve(&[2]).to_affine();
            IpaSuccinctVerifyingKey::new(
                Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
                params.get_g()[0],
                u,
                Some(w),
            )
        }

        fn canonical_folding_key(params: &ParamsIPA<EqAffine>) -> IpaProvingKey<EqAffine> {
            let svk = canonical_svk(params);
            IpaProvingKey::new(svk.domain.clone(), params.get_g().to_vec(), svk.h, svk.s)
        }

        fn create_poseidon_proof<CircuitT>(
            params: &ParamsIPA<EqAffine>,
            pk: &ProvingKey<EqAffine>,
            circuit: CircuitT,
            instances: &[&[&[Scalar]]],
        ) -> Vec<u8>
        where
            CircuitT: Circuit<Scalar>,
        {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(Vec::<u8>::new());
            create_proof::<
                IPACommitmentScheme<EqAffine>,
                ProverIPA<'_, EqAffine>,
                ChallengeScalar<EqAffine>,
                _,
                _,
                _,
            >(params, pk, &[circuit], instances, OsRng, &mut transcript)
            .expect("create Pasta IPA Poseidon proof");
            transcript.finalize()
        }

        fn folded_generator(
            params: &ParamsIPA<EqAffine>,
            vk: &halo2_base::halo2_proofs::plonk::VerifyingKey<EqAffine>,
            proof: &[u8],
            instances: &[&[&[Scalar]]],
        ) -> EqAffine {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof);
            verify_proof::<
                IPACommitmentScheme<EqAffine>,
                VerifierIPA<'_, EqAffine>,
                ChallengeScalar<EqAffine>,
                _,
                _,
            >(
                params,
                vk,
                FoldedGeneratorStrategy::new(params),
                instances,
                &mut transcript,
            )
            .expect("complete native verification computes folded generator")
        }

        fn fixture() -> Fixture {
            let params = params_new(INNER_K);
            let value = Scalar::from(7);
            let circuit = PublicValue { value };
            let vk = keygen_vk(&params, &circuit).expect("tiny Pasta verifier key");
            let pk = keygen_pk(&params, vk.clone(), &circuit).expect("tiny Pasta proving key");
            let column = [value];
            let columns: [&[Scalar]; 1] = [&column];
            let proof_instances: [&[&[Scalar]]; 1] = [&columns];
            let proof_without_folded_generator =
                create_poseidon_proof(&params, &pk, circuit, &proof_instances);
            let generator = folded_generator(
                &params,
                &vk,
                &proof_without_folded_generator,
                &proof_instances,
            );
            let mut augmented_proof = proof_without_folded_generator.clone();
            augmented_proof.extend_from_slice(generator.to_bytes().as_ref());
            let svk = canonical_svk(&params);
            let deciding_key = IpaDecidingKey::new(svk, params.get_g().to_vec());
            let protocol = compile(&params, &vk, Config::ipa().with_num_instance(vec![1]));
            Fixture {
                params,
                protocol,
                deciding_key,
                proof_without_folded_generator,
                augmented_proof,
                instances: vec![vec![value]],
            }
        }

        fn succinct_accumulator(fixture: &Fixture) -> IpaAccumulator<EqAffine, NativeLoader> {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(
                fixture.augmented_proof.as_slice(),
            );
            let parsed = SuccinctVerifier::read_proof(
                fixture.deciding_key.as_ref(),
                &fixture.protocol,
                &fixture.instances,
                &mut transcript,
            )
            .expect("parse augmented Axiom IPA proof as BGH19");
            let mut accumulators = SuccinctVerifier::verify(
                fixture.deciding_key.as_ref(),
                &fixture.protocol,
                &fixture.instances,
                &parsed,
            )
            .expect("verify the full PLONK residual and produce an IPA accumulator");
            assert_eq!(accumulators.len(), 1, "one proof yields one accumulator");
            accumulators.pop().expect("one accumulator")
        }

        fn create_fold_proof(
            params: &ParamsIPA<EqAffine>,
            accumulators: &[IpaAccumulator<EqAffine, NativeLoader>],
        ) -> (Vec<u8>, IpaAccumulator<EqAffine, NativeLoader>) {
            let key = canonical_folding_key(params);
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(Vec::<u8>::new());
            let folded = <As as AccumulationSchemeProver<EqAffine>>::create_proof(
                &key,
                accumulators,
                &mut transcript,
                OsRng,
            )
            .expect("create canonical Pasta IPA fold proof");
            (transcript.finalize(), folded)
        }

        #[test]
        fn transition_proof_omits_recomputable_deferred_material_from_the_wire() {
            use crate::zk::kagemusha_v2::{
                KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS,
                KagemushaRecursiveSpendTransitionCircuitV2,
                kagemusha_recursive_spend_transition_instance_column_v2,
            };

            const PRODUCTION_K: u32 = 12;
            let params = params_new(PRODUCTION_K);
            let circuit = KagemushaRecursiveSpendTransitionCircuitV2::default();
            let instance_column =
                kagemusha_recursive_spend_transition_instance_column_v2(&circuit.values);
            assert_eq!(
                instance_column.len(),
                KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS
            );
            let vk = keygen_vk(&params, &circuit).expect("transition deferred-packet VK");
            let pk =
                keygen_pk(&params, vk.clone(), &circuit).expect("transition deferred-packet PK");
            let columns: [&[Scalar]; 1] = [&instance_column];
            let proof_instances: [&[&[Scalar]]; 1] = [&columns];
            let proof_without_generator =
                create_poseidon_proof(&params, &pk, circuit, &proof_instances);
            let generator =
                folded_generator(&params, &vk, &proof_without_generator, &proof_instances);
            let mut proof_bytes = proof_without_generator;
            proof_bytes.extend_from_slice(generator.to_bytes().as_ref());

            let svk = canonical_svk(&params);
            let deciding_key = IpaDecidingKey::new(svk, params.get_g().to_vec());
            let protocol = compile(
                &params,
                &vk,
                Config::ipa().with_num_instance(vec![instance_column.len()]),
            );
            let instances = vec![instance_column];
            let mut transcript =
                Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof_bytes.as_slice());
            let parsed = SuccinctVerifier::read_proof(
                deciding_key.as_ref(),
                &protocol,
                &instances,
                &mut transcript,
            )
            .expect("parse fixed transition proof");
            let scalar_count = transcript
                .loaded_stream
                .iter()
                .filter(|object| matches!(object, TranscriptObject::Scalar(_)))
                .count();
            let point_count = transcript
                .loaded_stream
                .iter()
                .filter(|object| matches!(object, TranscriptObject::EcPoint(_)))
                .count();
            let explicit_challenge_count = parsed.challenges.len() + 1;
            let mut accumulators =
                SuccinctVerifier::verify(deciding_key.as_ref(), &protocol, &instances, &parsed)
                    .expect("verify fixed transition proof");
            assert_eq!(accumulators.len(), 1);
            <As as AccumulationDecider<EqAffine, NativeLoader>>::decide(
                &deciding_key,
                accumulators.pop().expect("one transition accumulator"),
            )
            .expect("terminal transition decision");

            // Re-run the exact fixed-key verifier with native scalar
            // arithmetic and symbolic curve arithmetic. This extracts the
            // complete MSM coefficient vectors rather than guessing from the
            // number of transcript objects.
            let recording_loader = RecordingLoader::<EqAffine>::new();
            let loaded_protocol = protocol.loaded(&recording_loader);
            let loaded_instances = instances
                .iter()
                .map(|column| {
                    column
                        .iter()
                        .map(|value| recording_loader.load_const(value))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut recording_transcript =
                RecordingPoseidonTranscript::<EqAffine, _, T, RATE, R_F, R_P>::new::<SECURE_MDS>(
                    recording_loader.clone(),
                    proof_bytes.as_slice(),
                );
            let recorded = SuccinctVerifier::read_proof(
                deciding_key.as_ref(),
                &loaded_protocol,
                &loaded_instances,
                &mut recording_transcript,
            )
            .expect("parse fixed transition proof for deferred audit");
            let recorded_accumulators = SuccinctVerifier::verify(
                deciding_key.as_ref(),
                &loaded_protocol,
                &loaded_instances,
                &recorded,
            )
            .expect("extract fixed transition residual equations");
            assert_eq!(recorded_accumulators.len(), 1);
            let recorded_accumulator = &recorded_accumulators[0];
            assert_eq!(recorded_accumulator.xi.len(), PRODUCTION_K as usize);
            let equations = recording_loader.equations();
            assert_eq!(
                equations.len(),
                1,
                "the fixed IPA verifier must expose exactly one opening-residual MSM"
            );

            let scalar_offset = *recording_transcript
                .scalar_offsets
                .first()
                .expect("fixed proof contains transcript scalars");
            let mut noncanonical_transcript = proof_bytes.clone();
            noncanonical_transcript[scalar_offset..scalar_offset + 32].fill(0xFF);
            let rejected = catch_unwind(AssertUnwindSafe(|| {
                let mut transcript =
                    RecordingPoseidonTranscript::<EqAffine, _, T, RATE, R_F, R_P>::new::<SECURE_MDS>(
                        recording_loader.clone(),
                        noncanonical_transcript.as_slice(),
                    );
                SuccinctVerifier::read_proof(
                    deciding_key.as_ref(),
                    &loaded_protocol,
                    &loaded_instances,
                    &mut transcript,
                )
            }));
            assert!(
                rejected.is_err() || rejected.expect("no panic").is_err(),
                "a non-canonical transcript scalar must fail before residual derivation"
            );

            // Canonical point-source namespace: transcript points first in
            // transcript order, followed by fixed protocol/SVK points. The
            // packet carries only a u16 source index plus a canonical scalar;
            // proof and artifact bytes supply the points themselves.
            let mut point_sources = recording_transcript.point_sources.clone();
            let svk = deciding_key.as_ref();
            let mut add_fixed_source = |point: EqAffine| {
                let bytes = point.to_bytes().as_ref().to_vec();
                if !point_sources.iter().any(|existing| existing == &bytes) {
                    point_sources.push(bytes);
                }
            };
            for point in &protocol.preprocessed {
                add_fixed_source(*point);
            }
            add_fixed_source(svk.g);
            add_fixed_source(svk.h);
            if let Some(point) = svk.s {
                add_fixed_source(point);
            }
            add_fixed_source(EqAffine::generator());
            if let Some(instance_key) = &protocol.instance_committing_key {
                for point in &instance_key.bases {
                    add_fixed_source(*point);
                }
                if let Some(point) = instance_key.constant {
                    add_fixed_source(point);
                }
            }
            assert!(
                point_sources.len() <= usize::from(u16::MAX),
                "deferred packet point namespace must fit u16"
            );

            let mut coefficient_count = 0_usize;
            for equation in &equations {
                assert!(!equation.terms.is_empty());
                for term in &equation.terms {
                    assert_eq!(term.point.len(), 32);
                    assert_eq!(term.coefficient.len(), 32);
                    assert!(
                        point_sources.iter().any(|source| source == &term.point),
                        "every residual base must resolve to proof or fixed-VK material"
                    );
                }
                coefficient_count += equation.terms.len();
            }
            let accumulator_u = recorded_accumulator.u.canonical_bytes();
            assert!(
                point_sources.iter().any(|source| source == &accumulator_u),
                "the output accumulator point must be a proof point"
            );
            for xi in &recorded_accumulator.xi {
                assert_eq!(xi.canonical_bytes().len(), 32);
            }

            // Coefficients and accumulator limbs are verifier-derived material,
            // not peer wire fields. Both the fixed leapfrog circuit and the
            // native terminal verifier reconstruct them from these proof bytes,
            // the authenticated fixed VK/protocol, and the exact instances.
            // This removes a redundant 1,858 bytes per proof and, more
            // importantly, prevents a serialized-equation substitution from
            // selecting a different MSM than the proof transcript selects.
            const EQUATION_HEADER_BYTES: usize = 2;
            const EQUATION_TERM_BYTES: usize = 2 + 32;
            let recomputed_material_bytes = equations.len() * EQUATION_HEADER_BYTES
                + coefficient_count * EQUATION_TERM_BYTES
                + recorded_accumulator.xi.len() * 32
                + 2;
            eprintln!(
                "Kagemusha compact proof={} scalars={} points={} explicit_challenges={} preprocessed={} residual_equations={} residual_coefficients={} point_sources={} derived_not_transported={}",
                proof_bytes.len(),
                scalar_count,
                point_count,
                explicit_challenge_count,
                protocol.preprocessed.len(),
                equations.len(),
                coefficient_count,
                point_sources.len(),
                recomputed_material_bytes,
            );
            assert!(
                proof_bytes.len() == KAGEMUSHA_LEAPFROG_STEP_PROOF_BYTES_V3,
                "the measured fixed step proof must fit its exact wire slot"
            );

            let predecessor_bytes = proof_bytes.clone();
            let mut newest_bytes = proof_bytes;
            newest_bytes[0] ^= 1;
            let mut predecessor =
                super::leapfrog_step(KagemushaPastaCycleParityV3::StepEq, 1, 0x5A, None);
            predecessor.proof_bytes = predecessor_bytes;
            let mut newest = super::leapfrog_step(
                KagemushaPastaCycleParityV3::StepEp,
                2,
                0xA5,
                Some(&predecessor),
            );
            newest.proof_bytes = newest_bytes;
            newest.public_inputs.predecessor_proof_sha256 = sha256(&predecessor.proof_bytes);
            let window = KagemushaLeapfrogProofWindowV3 {
                version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V3,
                newest,
                predecessor: Some(predecessor),
            };
            window.validate().expect("bounded two-proof window");
            assert!(
                norito::to_bytes(&window)
                    .expect("encode compact proof window")
                    .len()
                    == KAGEMUSHA_LEAPFROG_PROOF_WINDOW_BYTES_V3,
                "the newest/predecessor proof window must have its exact canonical size"
            );
        }

        #[test]
        fn canonical_ipa_fold_is_constant_size_decidable_and_substitution_safe() {
            let fixture = fixture();
            let accumulator = succinct_accumulator(&fixture);
            let inputs = [accumulator.clone(), accumulator];
            let (proof_bytes, expected) = create_fold_proof(&fixture.params, &inputs);
            let expected_wire_bytes = (8 + 2 * INNER_K as usize) * 32;
            assert_eq!(
                proof_bytes.len(),
                expected_wire_bytes,
                "the canonical Poseidon IPA fold wire must not gain metadata or a host receipt"
            );
            assert!(
                proof_bytes.len() <= 4_096,
                "canonical IPA fold proof must fit the recursive proof budget"
            );

            let svk = canonical_svk(&fixture.params);
            let mut transcript =
                Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof_bytes.as_slice());
            let proof = <As as AccumulationScheme<EqAffine, NativeLoader>>::read_proof(
                &svk,
                &inputs,
                &mut transcript,
            )
            .expect("parse canonical IPA fold proof");
            let folded =
                <As as AccumulationScheme<EqAffine, NativeLoader>>::verify(&svk, &inputs, &proof)
                    .expect("verify canonical IPA fold proof");
            assert_eq!(folded.xi, expected.xi);
            assert_eq!(folded.u, expected.u);
            <As as AccumulationDecider<EqAffine, NativeLoader>>::decide(
                &fixture.deciding_key,
                folded,
            )
            .expect("terminally decide folded IPA accumulator");

            let mut substituted_inputs = inputs;
            substituted_inputs[0].u = fixture.params.get_g()[1];
            let rejected = catch_unwind(AssertUnwindSafe(|| {
                let mut transcript =
                    Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof_bytes.as_slice());
                let proof = <As as AccumulationScheme<EqAffine, NativeLoader>>::read_proof(
                    &svk,
                    &substituted_inputs,
                    &mut transcript,
                )
                .expect("a canonical substituted point remains parseable");
                <As as AccumulationScheme<EqAffine, NativeLoader>>::verify(
                    &svk,
                    &substituted_inputs,
                    &proof,
                )
            }));
            assert!(
                rejected.is_err() || rejected.expect("no panic").is_err(),
                "an input-accumulator substitution must invalidate the fold"
            );
        }

        #[test]
        fn axiom_poseidon_wire_appends_exactly_one_folded_generator() {
            let fixture = fixture();
            assert_eq!(
                fixture.augmented_proof.len(),
                fixture.proof_without_folded_generator.len()
                    + std::mem::size_of::<<EqAffine as GroupEncoding>::Repr>(),
                "the recursion wire is the ordinary Axiom proof plus one compressed point"
            );

            let accumulator = succinct_accumulator(&fixture);
            <As as AccumulationDecider<EqAffine, NativeLoader>>::decide(
                &fixture.deciding_key,
                accumulator.clone(),
            )
            .expect("terminal decision recomputes the folded canonical generator basis");

            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(
                fixture.augmented_proof.as_slice(),
            );
            let parsed = FullVerifier::read_proof(
                &fixture.deciding_key,
                &fixture.protocol,
                &fixture.instances,
                &mut transcript,
            )
            .expect("full verifier parses augmented proof");
            FullVerifier::verify(
                &fixture.deciding_key,
                &fixture.protocol,
                &fixture.instances,
                &parsed,
            )
            .expect("full verifier includes terminal IPA decision");

            let substituted =
                IpaAccumulator::new(accumulator.xi.clone(), fixture.params.get_g()[1]);
            assert!(
                <As as AccumulationDecider<EqAffine, NativeLoader>>::decide(
                    &fixture.deciding_key,
                    substituted,
                )
                .is_err(),
                "carrying a substituted accumulator point is not a terminal decision"
            );
        }

        #[test]
        fn folded_generator_is_constrained_by_the_plonk_opening_residual() {
            let fixture = fixture();
            let mut substituted = fixture.augmented_proof.clone();
            let replacement = fixture.params.get_g()[1].to_bytes();
            let offset = substituted.len() - replacement.as_ref().len();
            substituted[offset..].copy_from_slice(replacement.as_ref());

            let rejected = catch_unwind(AssertUnwindSafe(|| {
                let mut transcript =
                    Transcript::<NativeLoader, _>::new::<SECURE_MDS>(substituted.as_slice());
                let parsed = SuccinctVerifier::read_proof(
                    fixture.deciding_key.as_ref(),
                    &fixture.protocol,
                    &fixture.instances,
                    &mut transcript,
                )
                .expect("a substituted canonical point remains parseable");
                SuccinctVerifier::verify(
                    fixture.deciding_key.as_ref(),
                    &fixture.protocol,
                    &fixture.instances,
                    &parsed,
                )
            }));
            assert!(
                rejected.is_err() || rejected.expect("no panic").is_err(),
                "a substituted folded generator must fail the constrained residual"
            );
        }
    }

    /// Reciprocal Pasta parity.  The production cycle is sound only if an
    /// Ep/Pallas proof over Fq is authenticated inside an Fp circuit with the
    /// same transcript, VK, public-instance, and fold bindings as Eq/Vesta.
    mod pasta_ipa_poseidon_wire_ep {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        use halo2_base::halo2_proofs::{
            halo2curves::{
                CurveExt as _,
                group::{Curve as _, GroupEncoding},
                pasta::{Ep, EpAffine, Fq},
            },
            plonk::{ProvingKey, create_proof, keygen_pk, keygen_vk, verify_proof},
            poly::{
                VerificationStrategy as _,
                commitment::{Params as _, ParamsProver as _},
                ipa::{
                    commitment::{IPACommitmentScheme, ParamsIPA},
                    multiopen::{ProverIPA, VerifierIPA},
                },
            },
        };
        use rand_core_06::OsRng;
        use snark_verifier::{
            loader::native::NativeLoader,
            pcs::{
                AccumulationDecider, AccumulationScheme, AccumulationSchemeProver,
                ipa::{
                    Bgh19, IpaAccumulator, IpaAs, IpaDecidingKey, IpaProvingKey,
                    IpaSuccinctVerifyingKey,
                },
            },
            system::halo2::{
                Config, compile,
                strategy::ipa::SingleStrategy as FoldedGeneratorStrategy,
                transcript::halo2::{ChallengeScalar, PoseidonTranscript},
            },
            util::arithmetic::{Domain, root_of_unity},
            verifier::{SnarkVerifier, plonk::PlonkSuccinctVerifier},
        };

        use super::PublicValue;

        const T: usize = 3;
        const RATE: usize = 2;
        const R_F: usize = 8;
        const R_P: usize = 57;
        const SECURE_MDS: usize = 0;
        const INNER_K: u32 = 5;

        type As = IpaAs<EpAffine, Bgh19>;
        type SuccinctVerifier = PlonkSuccinctVerifier<As>;
        type Transcript<L, S> = PoseidonTranscript<EpAffine, L, S, T, RATE, R_F, R_P>;

        struct Fixture {
            params: ParamsIPA<EpAffine>,
            protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
            deciding_key: IpaDecidingKey<EpAffine>,
            proof_without_folded_generator: Vec<u8>,
            augmented_proof: Vec<u8>,
            instances: Vec<Vec<Fq>>,
        }

        fn canonical_svk(params: &ParamsIPA<EpAffine>) -> IpaSuccinctVerifyingKey<EpAffine> {
            let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
            let w = hash_to_curve(&[1]).to_affine();
            let u = hash_to_curve(&[2]).to_affine();
            IpaSuccinctVerifyingKey::new(
                Domain::new(params.k() as usize, root_of_unity(params.k() as usize)),
                params.get_g()[0],
                u,
                Some(w),
            )
        }

        fn canonical_folding_key(params: &ParamsIPA<EpAffine>) -> IpaProvingKey<EpAffine> {
            let svk = canonical_svk(params);
            IpaProvingKey::new(svk.domain.clone(), params.get_g().to_vec(), svk.h, svk.s)
        }

        fn create_poseidon_proof(
            params: &ParamsIPA<EpAffine>,
            pk: &ProvingKey<EpAffine>,
            circuit: PublicValue<Fq>,
            instances: &[&[&[Fq]]],
        ) -> Vec<u8> {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(Vec::<u8>::new());
            create_proof::<
                IPACommitmentScheme<EpAffine>,
                ProverIPA<'_, EpAffine>,
                ChallengeScalar<EpAffine>,
                _,
                _,
                _,
            >(params, pk, &[circuit], instances, OsRng, &mut transcript)
            .expect("create reciprocal Pasta IPA Poseidon proof");
            transcript.finalize()
        }

        fn folded_generator(
            params: &ParamsIPA<EpAffine>,
            vk: &halo2_base::halo2_proofs::plonk::VerifyingKey<EpAffine>,
            proof: &[u8],
            instances: &[&[&[Fq]]],
        ) -> EpAffine {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(proof);
            verify_proof::<
                IPACommitmentScheme<EpAffine>,
                VerifierIPA<'_, EpAffine>,
                ChallengeScalar<EpAffine>,
                _,
                _,
            >(
                params,
                vk,
                FoldedGeneratorStrategy::new(params),
                instances,
                &mut transcript,
            )
            .expect("complete reciprocal native verification computes folded generator")
        }

        fn fixture() -> Fixture {
            let params = ParamsIPA::<EpAffine>::new(INNER_K);
            let value = Fq::from(11);
            let circuit = PublicValue { value };
            let vk = keygen_vk(&params, &circuit).expect("tiny reciprocal Pasta verifier key");
            let pk = keygen_pk(&params, vk.clone(), &circuit)
                .expect("tiny reciprocal Pasta proving key");
            let column = [value];
            let columns: [&[Fq]; 1] = [&column];
            let proof_instances: [&[&[Fq]]; 1] = [&columns];
            let proof_without_folded_generator =
                create_poseidon_proof(&params, &pk, circuit, &proof_instances);
            let generator = folded_generator(
                &params,
                &vk,
                &proof_without_folded_generator,
                &proof_instances,
            );
            let mut augmented_proof = proof_without_folded_generator.clone();
            augmented_proof.extend_from_slice(generator.to_bytes().as_ref());
            let svk = canonical_svk(&params);
            let deciding_key = IpaDecidingKey::new(svk, params.get_g().to_vec());
            let protocol = compile(&params, &vk, Config::ipa().with_num_instance(vec![1]));
            Fixture {
                params,
                protocol,
                deciding_key,
                proof_without_folded_generator,
                augmented_proof,
                instances: vec![vec![value]],
            }
        }

        fn succinct_accumulator(fixture: &Fixture) -> IpaAccumulator<EpAffine, NativeLoader> {
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(
                fixture.augmented_proof.as_slice(),
            );
            let parsed = SuccinctVerifier::read_proof(
                fixture.deciding_key.as_ref(),
                &fixture.protocol,
                &fixture.instances,
                &mut transcript,
            )
            .expect("parse reciprocal augmented IPA proof");
            let mut accumulators = SuccinctVerifier::verify(
                fixture.deciding_key.as_ref(),
                &fixture.protocol,
                &fixture.instances,
                &parsed,
            )
            .expect("verify reciprocal PLONK residual");
            assert_eq!(accumulators.len(), 1);
            accumulators.pop().expect("one reciprocal accumulator")
        }

        fn create_fold_proof(
            params: &ParamsIPA<EpAffine>,
            accumulators: &[IpaAccumulator<EpAffine, NativeLoader>],
        ) -> (Vec<u8>, IpaAccumulator<EpAffine, NativeLoader>) {
            let key = canonical_folding_key(params);
            let mut transcript = Transcript::<NativeLoader, _>::new::<SECURE_MDS>(Vec::<u8>::new());
            let folded = <As as AccumulationSchemeProver<EpAffine>>::create_proof(
                &key,
                accumulators,
                &mut transcript,
                OsRng,
            )
            .expect("create reciprocal Pasta IPA fold proof");
            (transcript.finalize(), folded)
        }

        #[test]
        fn reciprocal_poseidon_wire_fold_and_tamper_contract() {
            let fixture = fixture();
            assert_eq!(
                fixture.augmented_proof.len(),
                fixture.proof_without_folded_generator.len()
                    + std::mem::size_of::<<EpAffine as GroupEncoding>::Repr>()
            );
            let accumulator = succinct_accumulator(&fixture);
            let inputs = [accumulator.clone(), accumulator];
            let (fold_bytes, expected) = create_fold_proof(&fixture.params, &inputs);
            assert_eq!(fold_bytes.len(), (8 + 2 * INNER_K as usize) * 32);

            let svk = canonical_svk(&fixture.params);
            let mut transcript =
                Transcript::<NativeLoader, _>::new::<SECURE_MDS>(fold_bytes.as_slice());
            let proof = <As as AccumulationScheme<EpAffine, NativeLoader>>::read_proof(
                &svk,
                &inputs,
                &mut transcript,
            )
            .expect("parse reciprocal fold proof");
            let folded =
                <As as AccumulationScheme<EpAffine, NativeLoader>>::verify(&svk, &inputs, &proof)
                    .expect("verify reciprocal fold proof");
            assert_eq!(folded.xi, expected.xi);
            assert_eq!(folded.u, expected.u);
            <As as AccumulationDecider<EpAffine, NativeLoader>>::decide(
                &fixture.deciding_key,
                folded,
            )
            .expect("terminally decide reciprocal folded accumulator");

            let mut substituted = fixture.augmented_proof.clone();
            let replacement = fixture.params.get_g()[1].to_bytes();
            let offset = substituted.len() - replacement.as_ref().len();
            substituted[offset..].copy_from_slice(replacement.as_ref());
            let rejected = catch_unwind(AssertUnwindSafe(|| {
                let mut transcript =
                    Transcript::<NativeLoader, _>::new::<SECURE_MDS>(substituted.as_slice());
                let parsed = SuccinctVerifier::read_proof(
                    fixture.deciding_key.as_ref(),
                    &fixture.protocol,
                    &fixture.instances,
                    &mut transcript,
                )
                .expect("a reciprocal substituted canonical point remains parseable");
                SuccinctVerifier::verify(
                    fixture.deciding_key.as_ref(),
                    &fixture.protocol,
                    &fixture.instances,
                    &parsed,
                )
            }));
            assert!(
                rejected.is_err() || rejected.expect("no panic").is_err(),
                "a reciprocal folded-generator substitution must reject"
            );
        }
    }
}
