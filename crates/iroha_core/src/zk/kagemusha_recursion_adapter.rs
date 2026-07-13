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
//! The supported same-scalar-field `Eq/Fp` tuple avoids that trait boundary but
//! not the resource bound: the fixed verifier still measured 4,659,490 advice
//! cells at degree 12, while a degree-18 outer proof measured 7,296 bytes
//! ordinary and 7,328 bytes with its folded generator (about 4 GiB live RSS).
//! Both exceed the fixed 1,600-byte step-proof contract by construction.
//!
//! The production wire carries the current Eq/Fp and Ep/Fq proofs together,
//! with one exact 889-`u32` predecessor state and one exact resulting state.
//! The fixed verifier derives every transcript challenge, residual coefficient,
//! and IPA accumulator from proof bytes; none is caller-selected wire data.
//! The production build retains the native terminal Eq/Vesta and Ep/Pallas
//! decisions over authenticated parameters and verifier keys. Tests retain the
//! fixed-key Poseidon proof wires, canonical BGH19 IPA folding, and exact
//! bounded proof bytes. Production availability stays false until both
//! recursive fixed-VK verifier halves constrain those same operations and pass
//! the complete archive, review, and device gates.

#[cfg(test)]
use iroha_data_model::offline::KagemushaPastaCycleParityV1;
use norito::codec::{Decode, Encode};
#[cfg(test)]
use sha2::{Digest as _, Sha256};

use ff::PrimeField;
use halo2_proofs::halo2curves::pasta::{Fp, Fq};

/// Version of the compact leapfrog proof window.
#[cfg(test)]
pub const KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1: u16 = 1;
/// Maximum augmented IPA proof bytes for one fixed Kagemusha step circuit.
///
/// The reciprocal degree-12 proof tests measure both Pasta parities against
/// this release cap. It does not allow the old 4 KiB-per-proof envelope to
/// silently consume the complete peer budget.
pub const KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1: usize = 1_600;
/// Maximum canonical Norito bytes for the complete newest/predecessor window.
///
/// This is the payload embedded in `KagemushaRecursiveSpendProofV2::proof`;
/// statement, branch-conflict, and output-membership data have a separate
/// budget in the complete peer archive.
#[cfg(test)]
pub const KAGEMUSHA_LEAPFROG_PROOF_WINDOW_MAX_BYTES_V1: usize = 3_584;
/// Domain separator for identities of complete compact proof windows.
#[cfg(test)]
pub const KAGEMUSHA_LEAPFROG_PROOF_WINDOW_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:leapfrog-proof-window:v1";
/// Version of the exact two-proof Pasta recursion pair.
pub const KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V1: u16 = 1;
/// Maximum canonical Norito bytes for one Eq/Ep proof pair and its exact state.
pub const KAGEMUSHA_PASTA_PROOF_PAIR_MAX_BYTES_V1: usize = 16_890;
/// Domain separator for identities of complete Eq/Ep proof pairs.
#[cfg(test)]
pub const KAGEMUSHA_PASTA_PROOF_PAIR_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:pasta-proof-pair:exact-state:v1";
/// Number of non-zero, source-combined terms in the fixed degree-12 residual.
///
/// This count is extracted from the exact fixed verifier below. A key or
/// circuit shape that changes it requires a new authenticated release and wire
/// schema; accepting a variable residual would make packet-size and circuit
/// shape claims non-reproducible.
#[cfg(test)]
pub const KAGEMUSHA_DEFERRED_EQUATION_TERM_COUNT_V1: usize = 38;
/// Domain separator for the cross-layer deferred-equation binding.
#[cfg(test)]
pub const KAGEMUSHA_DEFERRED_EQUATION_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:deferred-equation:v1";
/// Fixed header limbs preceding the exact dynamic transcript in the KAT oracle.
#[cfg(test)]
pub const KAGEMUSHA_DEFERRED_EQUATION_EXACT_HEADER_LIMBS_V1: usize = 8;
/// Fixed identity limbs following the exact KAT-oracle header.
#[cfg(test)]
pub const KAGEMUSHA_DEFERRED_EQUATION_FIXED_IDENTITY_LIMBS_V1: usize = 3 * 8;
/// Fixed source/coefficient limbs following the exact transcript and instances.
#[cfg(test)]
pub const KAGEMUSHA_DEFERRED_EQUATION_DERIVED_LIMBS_V1: usize =
    KAGEMUSHA_DEFERRED_EQUATION_TERM_COUNT_V1 * (1 + 8);
/// Defensive host bound for the exact fixed transcript-scalar preimage.
#[cfg(test)]
pub const KAGEMUSHA_DEFERRED_TRANSCRIPT_SCALAR_MAX_V1: usize = 512;

/// Maximum exact parent states consumed by one recursive transition.
pub const KAGEMUSHA_PASTA_PARENT_SLOTS_V1: usize = 2;

/// Field-neutral public inputs shared by the current Eq/Fp and Ep/Fq proofs.
///
/// Both proofs constrain these exact `u32` limbs. No digest substitutes for a
/// consumed parent's state or the resulting recursive state. Absent parent
/// slots and their join digests are represented by mandatory all-zero padding.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaPastaCyclePublicInputsV1 {
    /// Canonical public-statement digest as eight unreduced little-endian limbs.
    pub public_statement_digest: [u32; 8],
    /// Number of consumed parent proof pairs.
    pub parent_count: u32,
    /// Complete ordered parent result states with exact zero padding.
    pub parent_states: [Vec<u32>; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    /// Complete state resulting from the current transition.
    pub result_state: Vec<u32>,
    /// Authenticated artifact-manifest SHA-256 as eight unreduced limbs.
    pub manifest_sha256: [u32; 8],
    /// SHA-256 joins for the Eq parent's scalar and point verifier halves.
    pub parent_eq_deferred_sha256: [[u32; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    /// SHA-256 joins for the Ep parent's scalar and point verifier halves.
    pub parent_ep_deferred_sha256: [[u32; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
}

impl KagemushaPastaCyclePublicInputsV1 {
    /// Convert the complete field-neutral vector to one Halo2 instance column.
    #[must_use]
    pub fn instance_column<F>(&self) -> Vec<F>
    where
        F: PrimeField + From<u64>,
    {
        self.public_statement_digest
            .iter()
            .chain(std::iter::once(&self.parent_count))
            .chain(self.parent_states.iter().flatten())
            .chain(&self.result_state)
            .chain(&self.manifest_sha256)
            .chain(self.parent_eq_deferred_sha256.iter().flatten())
            .chain(self.parent_ep_deferred_sha256.iter().flatten())
            .copied()
            .map(|limb| F::from(u64::from(limb)))
            .collect()
    }

    /// Validate exact lengths, layout markers, and initialization semantics.
    pub fn validate(&self, proof_step_count: u32) -> Result<(), String> {
        use super::kagemusha_v2::KagemushaRecursiveSpendStateVectorV1;
        use iroha_data_model::offline::{
            KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V1,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V1,
        };

        let parent_count = usize::try_from(self.parent_count)
            .map_err(|_| "Kagemusha parent count does not fit usize".to_owned())?;
        let initializing = proof_step_count == 1;
        if proof_step_count == 0
            || self.public_statement_digest == [0; 8]
            || self.manifest_sha256 == [0; 8]
            || parent_count > KAGEMUSHA_PASTA_PARENT_SLOTS_V1
            || initializing != (parent_count == 0)
            || (!initializing && parent_count == 0)
            || self
                .parent_states
                .iter()
                .any(|state| state.len() != KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V1)
            || self.result_state.len() != KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V1
            || self.result_state.first().copied()
                != Some(KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V1)
        {
            return Err("Kagemusha exact-state public-instance shape mismatch".to_owned());
        }
        for slot in 0..KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
            let present = slot < parent_count;
            let state = &self.parent_states[slot];
            let eq_digest = self.parent_eq_deferred_sha256[slot];
            let ep_digest = self.parent_ep_deferred_sha256[slot];
            if present {
                if state.first().copied()
                    != Some(KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V1)
                    || state == &self.result_state
                    || eq_digest == [0; 8]
                    || ep_digest == [0; 8]
                    || eq_digest == ep_digest
                {
                    return Err("Kagemusha present parent slot is invalid".to_owned());
                }
            } else if state.iter().any(|limb| *limb != 0)
                || eq_digest != [0; 8]
                || ep_digest != [0; 8]
            {
                return Err("Kagemusha absent parent slot has non-zero padding".to_owned());
            }
        }
        if parent_count == KAGEMUSHA_PASTA_PARENT_SLOTS_V1
            && self.parent_states[0] >= self.parent_states[1]
        {
            return Err("Kagemusha parent states are not in canonical order".to_owned());
        }
        let result_vector = KagemushaRecursiveSpendStateVectorV1 {
            limbs: self
                .result_state
                .clone()
                .try_into()
                .map_err(|_| "Kagemusha result state has the wrong length".to_owned())?,
        };
        if result_vector.proof_step_count() != proof_step_count
            || result_vector.peer_hop_count() > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
            || result_vector.manifest_sha256_limbs() != self.manifest_sha256
        {
            return Err("Kagemusha result-state counters or manifest mismatch".to_owned());
        }
        let mut maximum_parent_step = 0_u32;
        let mut maximum_parent_hop = 0_u32;
        for state in self.parent_states.iter().take(parent_count) {
            let vector = KagemushaRecursiveSpendStateVectorV1 {
                limbs: state
                    .clone()
                    .try_into()
                    .map_err(|_| "Kagemusha parent state has the wrong length".to_owned())?,
            };
            let parent_step = vector.proof_step_count();
            let parent_hop = vector.peer_hop_count();
            if parent_step == 0
                || parent_step >= proof_step_count
                || parent_hop > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
                || vector.manifest_sha256_limbs() != self.manifest_sha256
            {
                return Err("Kagemusha parent-state counters or manifest mismatch".to_owned());
            }
            maximum_parent_step = maximum_parent_step.max(parent_step);
            maximum_parent_hop = maximum_parent_hop.max(parent_hop);
        }
        if initializing {
            if result_vector.peer_hop_count() != 0 {
                return Err("Kagemusha initialization state has a peer hop".to_owned());
            }
        } else if maximum_parent_step.checked_add(1) != Some(proof_step_count)
            || !matches!(
                result_vector
                    .peer_hop_count()
                    .checked_sub(maximum_parent_hop),
                Some(0 | 1)
            )
        {
            return Err("Kagemusha parent/result step or hop relation mismatch".to_owned());
        }
        Ok(())
    }
}

/// Current Eq/Fp and Ep/Fq recursive proofs for one logical transition.
///
/// This pair replaces the unsound alternating temporal window: every bundle
/// carries both current parities, and each next pair recursively closes both
/// halves of its parent before advancing the exact shared state.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaPastaCycleProofPairV1 {
    /// Pair wire-layout version.
    pub version: u16,
    /// Logical recursive transition count proved by both halves.
    pub proof_step_count: u32,
    /// Exact common public instances used by both proofs.
    pub public_inputs: KagemushaPastaCyclePublicInputsV1,
    /// Current Eq/Fp proof bytes.
    pub step_eq_proof_bytes: Vec<u8>,
    /// Current Ep/Fq proof bytes.
    pub step_ep_proof_bytes: Vec<u8>,
}

impl KagemushaPastaCycleProofPairV1 {
    /// Validate the exact paired-proof shape and bounded canonical archive.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V1
            || self.step_eq_proof_bytes.is_empty()
            || self.step_ep_proof_bytes.is_empty()
            || self.step_eq_proof_bytes.len() > KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1
            || self.step_ep_proof_bytes.len() > KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1
            || self.step_eq_proof_bytes == self.step_ep_proof_bytes
        {
            return Err("Kagemusha Eq/Ep proof-pair shape mismatch".to_owned());
        }
        self.public_inputs.validate(self.proof_step_count)?;
        let encoded = norito::to_bytes(self)
            .map_err(|error| format!("failed to encode Kagemusha proof pair: {error}"))?;
        if encoded.len() > KAGEMUSHA_PASTA_PROOF_PAIR_MAX_BYTES_V1 {
            return Err(format!(
                "Kagemusha proof pair is {} bytes; maximum is {}",
                encoded.len(),
                KAGEMUSHA_PASTA_PROOF_PAIR_MAX_BYTES_V1
            ));
        }
        Ok(())
    }

    /// Return a domain-separated identity of the exact canonical pair.
    #[cfg(test)]
    pub fn digest(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        let encoded = norito::to_bytes(self)
            .map_err(|error| format!("failed to encode Kagemusha proof pair: {error}"))?;
        let mut hasher = Sha256::new();
        hasher.update(KAGEMUSHA_PASTA_PROOF_PAIR_DIGEST_DOMAIN_V1);
        hasher.update([0]);
        hasher.update(encoded);
        Ok(hasher.finalize().into())
    }
}

const KAGEMUSHA_POSEIDON_WIDTH: usize = 3;
const KAGEMUSHA_POSEIDON_RATE: usize = 2;
const KAGEMUSHA_POSEIDON_FULL_ROUNDS: usize = 8;
const KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS: usize = 57;
const KAGEMUSHA_POSEIDON_SECURE_MDS: usize = 0;

/// Produce an augmented Poseidon/IPA Eq proof and immediately self-verify it.
#[cfg(test)]
pub(crate) fn prove_step_eq<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    proving_key: &halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    circuit: C,
    public_inputs: &KagemushaPastaCyclePublicInputsV1,
    proof_step_count: u32,
) -> Result<Vec<u8>, String>
where
    C: halo2_proofs::plonk::Circuit<Fp>,
{
    use halo2_proofs::{
        halo2curves::{group::GroupEncoding as _, pasta::EqAffine},
        plonk::{create_proof, verify_proof},
        poly::{
            VerificationStrategy as _,
            ipa::{
                commitment::IPACommitmentScheme,
                multiopen::{ProverIPA, VerifierIPA},
            },
        },
    };
    use rand_core_06::OsRng;
    use snark_verifier::{
        loader::native::NativeLoader,
        system::halo2::{
            strategy::ipa::SingleStrategy,
            transcript::halo2::{ChallengeScalar, PoseidonTranscript},
        },
    };

    public_inputs.validate(proof_step_count)?;
    type Transcript<S> = PoseidonTranscript<
        EqAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let column = public_inputs.instance_column::<Fp>();
    let columns: [&[Fp]; 1] = [&column];
    let proofs_instances: [&[&[Fp]]; 1] = [&columns];
    let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(Vec::new());
    create_proof::<
        IPACommitmentScheme<EqAffine>,
        ProverIPA<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
        _,
    >(
        params,
        proving_key,
        &[circuit],
        &proofs_instances,
        OsRng,
        &mut transcript,
    )
    .map_err(|error| format!("failed to create Kagemusha Eq proof: {error}"))?;
    let mut proof = transcript.finalize();
    let mut verification_transcript =
        Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(proof.as_slice());
    let folded_generator = verify_proof::<
        IPACommitmentScheme<EqAffine>,
        VerifierIPA<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
    >(
        params,
        proving_key.get_vk(),
        SingleStrategy::new(params),
        &proofs_instances,
        &mut verification_transcript,
    )
    .map_err(|error| format!("failed to derive Kagemusha Eq folded generator: {error}"))?;
    proof.extend_from_slice(folded_generator.to_bytes().as_ref());
    if proof.len() > KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1 {
        return Err("Kagemusha Eq proof exceeds the fixed release bound".to_owned());
    }
    terminal_verify_step_eq(
        params,
        proving_key.get_vk(),
        &proof,
        public_inputs,
        proof_step_count,
    )?;
    Ok(proof)
}

/// Produce an augmented Poseidon/IPA Ep proof and immediately self-verify it.
#[cfg(test)]
pub(crate) fn prove_step_ep<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    proving_key: &halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    circuit: C,
    public_inputs: &KagemushaPastaCyclePublicInputsV1,
    proof_step_count: u32,
) -> Result<Vec<u8>, String>
where
    C: halo2_proofs::plonk::Circuit<Fq>,
{
    use halo2_proofs::{
        halo2curves::{group::GroupEncoding as _, pasta::EpAffine},
        plonk::{create_proof, verify_proof},
        poly::{
            VerificationStrategy as _,
            ipa::{
                commitment::IPACommitmentScheme,
                multiopen::{ProverIPA, VerifierIPA},
            },
        },
    };
    use rand_core_06::OsRng;
    use snark_verifier::{
        loader::native::NativeLoader,
        system::halo2::{
            strategy::ipa::SingleStrategy,
            transcript::halo2::{ChallengeScalar, PoseidonTranscript},
        },
    };

    public_inputs.validate(proof_step_count)?;
    type Transcript<S> = PoseidonTranscript<
        EpAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let column = public_inputs.instance_column::<Fq>();
    let columns: [&[Fq]; 1] = [&column];
    let proofs_instances: [&[&[Fq]]; 1] = [&columns];
    let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(Vec::new());
    create_proof::<
        IPACommitmentScheme<EpAffine>,
        ProverIPA<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
        _,
    >(
        params,
        proving_key,
        &[circuit],
        &proofs_instances,
        OsRng,
        &mut transcript,
    )
    .map_err(|error| format!("failed to create Kagemusha Ep proof: {error}"))?;
    let mut proof = transcript.finalize();
    let mut verification_transcript =
        Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(proof.as_slice());
    let folded_generator = verify_proof::<
        IPACommitmentScheme<EpAffine>,
        VerifierIPA<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
    >(
        params,
        proving_key.get_vk(),
        SingleStrategy::new(params),
        &proofs_instances,
        &mut verification_transcript,
    )
    .map_err(|error| format!("failed to derive Kagemusha Ep folded generator: {error}"))?;
    proof.extend_from_slice(folded_generator.to_bytes().as_ref());
    if proof.len() > KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1 {
        return Err("Kagemusha Ep proof exceeds the fixed release bound".to_owned());
    }
    terminal_verify_step_ep(
        params,
        proving_key.get_vk(),
        &proof,
        public_inputs,
        proof_step_count,
    )?;
    Ok(proof)
}

/// Fully verify and terminally decide the current Eq/Vesta proof.
pub(crate) fn terminal_verify_step_eq(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    proof: &[u8],
    public_inputs: &KagemushaPastaCyclePublicInputsV1,
    proof_step_count: u32,
) -> Result<(), String> {
    public_inputs.validate(proof_step_count)?;
    let instances = vec![public_inputs.instance_column::<Fp>()];
    terminal_verify_step_eq_instances(params, verifying_key, proof, &instances)
}

fn terminal_verify_step_eq_instances(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    proof: &[u8],
    instances: &[Vec<Fp>],
) -> Result<(), String> {
    use halo2_proofs::{
        halo2curves::{
            CurveExt as _,
            group::Curve as _,
            pasta::{Eq, EqAffine},
        },
        poly::commitment::{Params as _, ParamsProver as _},
    };
    use snark_verifier::{
        loader::native::NativeLoader,
        pcs::ipa::{Bgh19, IpaAs, IpaDecidingKey, IpaSuccinctVerifyingKey},
        system::halo2::{Config, compile, transcript::halo2::PoseidonTranscript},
        util::arithmetic::{Domain, root_of_unity},
        verifier::{SnarkVerifier as _, plonk::PlonkVerifier},
    };

    if proof.is_empty() || proof.len() > KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1 {
        return Err("Kagemusha Eq proof length is invalid".to_owned());
    }
    type Scheme = IpaAs<EqAffine, Bgh19>;
    type Transcript<S> = PoseidonTranscript<
        EqAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    let w = hash_to_curve(&[1]).to_affine();
    let u = hash_to_curve(&[2]).to_affine();
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(
            usize::try_from(params.k()).map_err(|_| "Eq parameter degree does not fit usize")?,
            root_of_unity(
                usize::try_from(params.k())
                    .map_err(|_| "Eq parameter degree does not fit usize")?,
            ),
        ),
        params.get_g()[0],
        u,
        Some(w),
    );
    let deciding_key = IpaDecidingKey::new(svk, params.get_g().to_vec());
    let protocol = compile(
        params,
        verifying_key,
        Config::ipa().with_num_instance(vec![instances[0].len()]),
    );
    let mut cursor = std::io::Cursor::new(proof);
    {
        let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(&mut cursor);
        let parsed = PlonkVerifier::<Scheme>::read_proof(
            &deciding_key,
            &protocol,
            instances,
            &mut transcript,
        )
        .map_err(|error| format!("failed to parse Kagemusha Eq proof: {error:?}"))?;
        PlonkVerifier::<Scheme>::verify(&deciding_key, &protocol, instances, &parsed)
            .map_err(|error| format!("Kagemusha Eq terminal decision failed: {error:?}"))?;
    }
    if cursor.position()
        != u64::try_from(proof.len()).map_err(|_| "Eq proof length does not fit u64")?
    {
        return Err("Kagemusha Eq proof has trailing bytes".to_owned());
    }
    Ok(())
}

/// Fully verify and terminally decide the current Ep/Pallas proof.
pub(crate) fn terminal_verify_step_ep(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    proof: &[u8],
    public_inputs: &KagemushaPastaCyclePublicInputsV1,
    proof_step_count: u32,
) -> Result<(), String> {
    public_inputs.validate(proof_step_count)?;
    let instances = vec![public_inputs.instance_column::<Fq>()];
    terminal_verify_step_ep_instances(params, verifying_key, proof, &instances)
}

fn terminal_verify_step_ep_instances(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    proof: &[u8],
    instances: &[Vec<Fq>],
) -> Result<(), String> {
    use halo2_proofs::{
        halo2curves::{
            CurveExt as _,
            group::Curve as _,
            pasta::{Ep, EpAffine},
        },
        poly::commitment::{Params as _, ParamsProver as _},
    };
    use snark_verifier::{
        loader::native::NativeLoader,
        pcs::ipa::{Bgh19, IpaAs, IpaDecidingKey, IpaSuccinctVerifyingKey},
        system::halo2::{Config, compile, transcript::halo2::PoseidonTranscript},
        util::arithmetic::{Domain, root_of_unity},
        verifier::{SnarkVerifier as _, plonk::PlonkVerifier},
    };

    if proof.is_empty() || proof.len() > KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1 {
        return Err("Kagemusha Ep proof length is invalid".to_owned());
    }
    type Scheme = IpaAs<EpAffine, Bgh19>;
    type Transcript<S> = PoseidonTranscript<
        EpAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
    let w = hash_to_curve(&[1]).to_affine();
    let u = hash_to_curve(&[2]).to_affine();
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(
            usize::try_from(params.k()).map_err(|_| "Ep parameter degree does not fit usize")?,
            root_of_unity(
                usize::try_from(params.k())
                    .map_err(|_| "Ep parameter degree does not fit usize")?,
            ),
        ),
        params.get_g()[0],
        u,
        Some(w),
    );
    let deciding_key = IpaDecidingKey::new(svk, params.get_g().to_vec());
    let protocol = compile(
        params,
        verifying_key,
        Config::ipa().with_num_instance(vec![instances[0].len()]),
    );
    let mut cursor = std::io::Cursor::new(proof);
    {
        let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(&mut cursor);
        let parsed = PlonkVerifier::<Scheme>::read_proof(
            &deciding_key,
            &protocol,
            instances,
            &mut transcript,
        )
        .map_err(|error| format!("failed to parse Kagemusha Ep proof: {error:?}"))?;
        PlonkVerifier::<Scheme>::verify(&deciding_key, &protocol, instances, &parsed)
            .map_err(|error| format!("Kagemusha Ep terminal decision failed: {error:?}"))?;
    }
    if cursor.position()
        != u64::try_from(proof.len()).map_err(|_| "Ep proof length does not fit u64")?
    {
        return Err("Kagemusha Ep proof has trailing bytes".to_owned());
    }
    Ok(())
}

/// Fully verify and terminally decide both current recursion halves.
pub(crate) fn terminal_verify_proof_pair(
    step_eq_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_eq_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_ep_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    step_ep_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    pair: &KagemushaPastaCycleProofPairV1,
) -> Result<(), String> {
    pair.validate()?;
    terminal_verify_step_eq(
        step_eq_params,
        step_eq_verifying_key,
        &pair.step_eq_proof_bytes,
        &pair.public_inputs,
        pair.proof_step_count,
    )?;
    terminal_verify_step_ep(
        step_ep_params,
        step_ep_verifying_key,
        &pair.step_ep_proof_bytes,
        &pair.public_inputs,
        pair.proof_step_count,
    )
}

/// Authenticated terminal-verifier material for the complete Eq/Ep pair.
///
/// The constructor accepts only already authenticated, unframed artifact
/// payloads. It parses both parameter sets and both processed verifier keys,
/// rejects trailing bytes, and retains the complete pair as one indivisible
/// verifier object.
pub(crate) struct KagemushaPastaCycleTerminalVerifierV1 {
    step_eq_params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_eq_verifying_key:
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_ep_params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EpAffine>,
    step_ep_verifying_key:
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
}

impl KagemushaPastaCycleTerminalVerifierV1 {
    /// Parse the exact Eq/Ep verifier material rebound to one manifest.
    pub(crate) fn from_authenticated_artifacts<StepEqCircuit, StepEpCircuit>(
        artifacts: &super::kagemusha_v2::KagemushaPastaCycleVerifierArtifactsV3,
        release: &iroha_data_model::offline::KagemushaAuthenticatedReleaseV3,
    ) -> Result<Self, String>
    where
        StepEqCircuit: halo2_proofs::plonk::Circuit<Fp>,
        StepEqCircuit::Params: Default,
        StepEpCircuit: halo2_proofs::plonk::Circuit<Fq>,
        StepEpCircuit::Params: Default,
    {
        if artifacts.manifest_sha256() != release.manifest_sha256() {
            return Err(
                "Kagemusha terminal verifier artifacts do not bind the authenticated release"
                    .to_owned(),
            );
        }
        Self::from_authenticated_payloads::<StepEqCircuit, StepEpCircuit>(
            artifacts.step_eq_parameters(),
            artifacts.step_eq_verifying_key(),
            artifacts.step_ep_parameters(),
            artifacts.step_ep_verifying_key(),
        )
    }

    /// Parse an exact pair of authenticated parameter and processed-VK payloads.
    fn from_authenticated_payloads<StepEqCircuit, StepEpCircuit>(
        step_eq_params: &[u8],
        step_eq_verifying_key: &[u8],
        step_ep_params: &[u8],
        step_ep_verifying_key: &[u8],
    ) -> Result<Self, String>
    where
        StepEqCircuit: halo2_proofs::plonk::Circuit<Fp>,
        StepEqCircuit::Params: Default,
        StepEpCircuit: halo2_proofs::plonk::Circuit<Fq>,
        StepEpCircuit::Params: Default,
    {
        use halo2_proofs::{
            SerdeFormat,
            halo2curves::pasta::{EpAffine, EqAffine},
            plonk::VerifyingKey,
            poly::{commitment::Params as _, ipa::commitment::ParamsIPA},
        };

        fn parse_params<C>(bytes: &[u8], role: &str) -> Result<ParamsIPA<C>, String>
        where
            C: halo2_proofs::halo2curves::CurveAffine,
        {
            let mut cursor = std::io::Cursor::new(bytes);
            let params = ParamsIPA::<C>::read(&mut cursor)
                .map_err(|error| format!("failed to parse Kagemusha {role} parameters: {error}"))?;
            if cursor.position()
                != u64::try_from(bytes.len())
                    .map_err(|_| format!("Kagemusha {role} parameter length does not fit u64"))?
            {
                return Err(format!("Kagemusha {role} parameters have trailing bytes"));
            }
            Ok(params)
        }

        fn parse_eq_vk<CircuitT>(bytes: &[u8]) -> Result<VerifyingKey<EqAffine>, String>
        where
            CircuitT: halo2_proofs::plonk::Circuit<Fp>,
            CircuitT::Params: Default,
        {
            let mut cursor = std::io::Cursor::new(bytes);
            #[cfg(feature = "circuit-params")]
            let key = VerifyingKey::<EqAffine>::read::<_, CircuitT>(
                &mut cursor,
                SerdeFormat::Processed,
                CircuitT::Params::default(),
            )
            .map_err(|error| format!("failed to parse Kagemusha Eq verifier key: {error}"))?;
            #[cfg(not(feature = "circuit-params"))]
            let key =
                VerifyingKey::<EqAffine>::read::<_, CircuitT>(&mut cursor, SerdeFormat::Processed)
                    .map_err(|error| {
                        format!("failed to parse Kagemusha Eq verifier key: {error}")
                    })?;
            if cursor.position()
                != u64::try_from(bytes.len())
                    .map_err(|_| "Kagemusha Eq verifier-key length does not fit u64")?
            {
                return Err("Kagemusha Eq verifier key has trailing bytes".to_owned());
            }
            Ok(key)
        }

        fn parse_ep_vk<CircuitT>(bytes: &[u8]) -> Result<VerifyingKey<EpAffine>, String>
        where
            CircuitT: halo2_proofs::plonk::Circuit<Fq>,
            CircuitT::Params: Default,
        {
            let mut cursor = std::io::Cursor::new(bytes);
            #[cfg(feature = "circuit-params")]
            let key = VerifyingKey::<EpAffine>::read::<_, CircuitT>(
                &mut cursor,
                SerdeFormat::Processed,
                CircuitT::Params::default(),
            )
            .map_err(|error| format!("failed to parse Kagemusha Ep verifier key: {error}"))?;
            #[cfg(not(feature = "circuit-params"))]
            let key =
                VerifyingKey::<EpAffine>::read::<_, CircuitT>(&mut cursor, SerdeFormat::Processed)
                    .map_err(|error| {
                        format!("failed to parse Kagemusha Ep verifier key: {error}")
                    })?;
            if cursor.position()
                != u64::try_from(bytes.len())
                    .map_err(|_| "Kagemusha Ep verifier-key length does not fit u64")?
            {
                return Err("Kagemusha Ep verifier key has trailing bytes".to_owned());
            }
            Ok(key)
        }

        let step_eq_params = parse_params::<EqAffine>(step_eq_params, "Eq")?;
        let step_ep_params = parse_params::<EpAffine>(step_ep_params, "Ep")?;
        let expected_k = iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1;
        if step_eq_params.k() != expected_k || step_ep_params.k() != expected_k {
            return Err("Kagemusha Eq/Ep parameter degree mismatch".to_owned());
        }
        Ok(Self {
            step_eq_params,
            step_eq_verifying_key: parse_eq_vk::<StepEqCircuit>(step_eq_verifying_key)?,
            step_ep_params,
            step_ep_verifying_key: parse_ep_vk::<StepEpCircuit>(step_ep_verifying_key)?,
        })
    }

    /// Fully verify and terminally decide both proofs with the owned key pair.
    pub(crate) fn verify_pair(&self, pair: &KagemushaPastaCycleProofPairV1) -> Result<(), String> {
        terminal_verify_proof_pair(
            &self.step_eq_params,
            &self.step_eq_verifying_key,
            &self.step_ep_params,
            &self.step_ep_verifying_key,
            pair,
        )
    }
}

/// Authenticated Eq/Ep proving material with embedded-VK consistency checks.
///
/// The processed proving key for each parity embeds a verifier key. Parsing
/// rejects a release where that embedded key differs byte-for-byte from the
/// separately authenticated verifier-key role.
#[cfg(test)]
pub(crate) struct KagemushaPastaCycleProverV1 {
    step_eq_params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_eq_proving_key:
        halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_ep_params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EpAffine>,
    step_ep_proving_key:
        halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
}

#[cfg(test)]
impl KagemushaPastaCycleProverV1 {
    /// Parse all six authenticated roles and reject trailing or cross-key bytes.
    pub(crate) fn from_authenticated_artifacts<StepEqCircuit, StepEpCircuit>(
        artifacts: &super::kagemusha_v2::KagemushaPastaCycleProverArtifactsV3,
    ) -> Result<Self, String>
    where
        StepEqCircuit: halo2_proofs::plonk::Circuit<Fp>,
        StepEqCircuit::Params: Default,
        StepEpCircuit: halo2_proofs::plonk::Circuit<Fq>,
        StepEpCircuit::Params: Default,
    {
        use halo2_proofs::{
            SerdeFormat,
            halo2curves::pasta::{EpAffine, EqAffine},
            plonk::ProvingKey,
            poly::{commitment::Params as _, ipa::commitment::ParamsIPA},
        };

        fn parse_params<C>(bytes: &[u8], role: &str) -> Result<ParamsIPA<C>, String>
        where
            C: halo2_proofs::halo2curves::CurveAffine,
        {
            let mut cursor = std::io::Cursor::new(bytes);
            let params = ParamsIPA::<C>::read(&mut cursor)
                .map_err(|error| format!("failed to parse Kagemusha {role} parameters: {error}"))?;
            if cursor.position()
                != u64::try_from(bytes.len())
                    .map_err(|_| format!("Kagemusha {role} parameter length does not fit u64"))?
            {
                return Err(format!("Kagemusha {role} parameters have trailing bytes"));
            }
            Ok(params)
        }

        fn parse_eq_pk<CircuitT>(bytes: &[u8]) -> Result<ProvingKey<EqAffine>, String>
        where
            CircuitT: halo2_proofs::plonk::Circuit<Fp>,
            CircuitT::Params: Default,
        {
            let mut cursor = std::io::Cursor::new(bytes);
            #[cfg(feature = "circuit-params")]
            let key = ProvingKey::<EqAffine>::read::<_, CircuitT>(
                &mut cursor,
                SerdeFormat::Processed,
                CircuitT::Params::default(),
            )
            .map_err(|error| format!("failed to parse Kagemusha Eq proving key: {error}"))?;
            #[cfg(not(feature = "circuit-params"))]
            let key =
                ProvingKey::<EqAffine>::read::<_, CircuitT>(&mut cursor, SerdeFormat::Processed)
                    .map_err(|error| {
                        format!("failed to parse Kagemusha Eq proving key: {error}")
                    })?;
            if cursor.position()
                != u64::try_from(bytes.len())
                    .map_err(|_| "Kagemusha Eq proving-key length does not fit u64")?
            {
                return Err("Kagemusha Eq proving key has trailing bytes".to_owned());
            }
            Ok(key)
        }

        fn parse_ep_pk<CircuitT>(bytes: &[u8]) -> Result<ProvingKey<EpAffine>, String>
        where
            CircuitT: halo2_proofs::plonk::Circuit<Fq>,
            CircuitT::Params: Default,
        {
            let mut cursor = std::io::Cursor::new(bytes);
            #[cfg(feature = "circuit-params")]
            let key = ProvingKey::<EpAffine>::read::<_, CircuitT>(
                &mut cursor,
                SerdeFormat::Processed,
                CircuitT::Params::default(),
            )
            .map_err(|error| format!("failed to parse Kagemusha Ep proving key: {error}"))?;
            #[cfg(not(feature = "circuit-params"))]
            let key =
                ProvingKey::<EpAffine>::read::<_, CircuitT>(&mut cursor, SerdeFormat::Processed)
                    .map_err(|error| {
                        format!("failed to parse Kagemusha Ep proving key: {error}")
                    })?;
            if cursor.position()
                != u64::try_from(bytes.len())
                    .map_err(|_| "Kagemusha Ep proving-key length does not fit u64")?
            {
                return Err("Kagemusha Ep proving key has trailing bytes".to_owned());
            }
            Ok(key)
        }

        let verifier = artifacts.verifier();
        let step_eq_params = parse_params::<EqAffine>(verifier.step_eq_parameters(), "Eq")?;
        let step_ep_params = parse_params::<EpAffine>(verifier.step_ep_parameters(), "Ep")?;
        let expected_k = iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_IPA_K_V1;
        if step_eq_params.k() != expected_k || step_ep_params.k() != expected_k {
            return Err("Kagemusha Eq/Ep parameter degree mismatch".to_owned());
        }
        let step_eq_proving_key = parse_eq_pk::<StepEqCircuit>(artifacts.step_eq_proving_key())?;
        let step_ep_proving_key = parse_ep_pk::<StepEpCircuit>(artifacts.step_ep_proving_key())?;
        if step_eq_proving_key
            .get_vk()
            .to_bytes(SerdeFormat::Processed)
            != verifier.step_eq_verifying_key()
            || step_ep_proving_key
                .get_vk()
                .to_bytes(SerdeFormat::Processed)
                != verifier.step_ep_verifying_key()
        {
            return Err("Kagemusha proving key embeds a different verifier key".to_owned());
        }
        Ok(Self {
            step_eq_params,
            step_eq_proving_key,
            step_ep_params,
            step_ep_proving_key,
        })
    }

    /// Produce and immediately terminally decide both current proofs.
    pub(crate) fn prove_pair<StepEqCircuit, StepEpCircuit>(
        &self,
        step_eq_circuit: StepEqCircuit,
        step_ep_circuit: StepEpCircuit,
        public_inputs: KagemushaPastaCyclePublicInputsV1,
        proof_step_count: u32,
    ) -> Result<KagemushaPastaCycleProofPairV1, String>
    where
        StepEqCircuit: halo2_proofs::plonk::Circuit<Fp>,
        StepEpCircuit: halo2_proofs::plonk::Circuit<Fq>,
    {
        public_inputs.validate(proof_step_count)?;
        let step_eq_proof_bytes = prove_step_eq(
            &self.step_eq_params,
            &self.step_eq_proving_key,
            step_eq_circuit,
            &public_inputs,
            proof_step_count,
        )?;
        let step_ep_proof_bytes = prove_step_ep(
            &self.step_ep_params,
            &self.step_ep_proving_key,
            step_ep_circuit,
            &public_inputs,
            proof_step_count,
        )?;
        let pair = KagemushaPastaCycleProofPairV1 {
            version: KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V1,
            proof_step_count,
            public_inputs,
            step_eq_proof_bytes,
            step_ep_proof_bytes,
        };
        pair.validate()?;
        Ok(pair)
    }
}

/// Bit-exact SHA-256 gadget used to join the two Pasta verifier halves.
///
/// The input length is part of the fixed circuit shape. Every input byte is
/// range constrained, SHA padding is inserted as circuit constants, and every
/// Boolean and modular-addition relation is constrained. The returned words
/// are the standard big-endian SHA-256 digest words.
pub struct KagemushaSha256Chip;

impl KagemushaSha256Chip {
    /// Constrain the identity of already-assigned deferred-equation bytes.
    ///
    /// Fixed verifier halves must construct `exact_bytes` exclusively from
    /// their already-constrained transcript, instance, key, and residual
    /// values. This API deliberately cannot assign a host-side binding as a
    /// free witness.
    pub fn deferred_equation_identity<F>(
        ctx: &mut halo2_base::Context<F>,
        range: &halo2_base::gates::RangeChip<F>,
        exact_bytes: &[halo2_base::AssignedValue<F>],
    ) -> [halo2_base::AssignedValue<F>; 8]
    where
        F: halo2_base::utils::BigPrimeField,
    {
        Self::digest(ctx, range, exact_bytes)
    }

    /// Constrain SHA-256 over one fixed-length byte slice.
    pub fn digest<F>(
        ctx: &mut halo2_base::Context<F>,
        range: &halo2_base::gates::RangeChip<F>,
        message: &[halo2_base::AssignedValue<F>],
    ) -> [halo2_base::AssignedValue<F>; 8]
    where
        F: halo2_base::utils::BigPrimeField,
    {
        use halo2_base::gates::{GateInstructions as _, RangeInstructions as _};

        #[derive(Clone)]
        struct Word<F: halo2_base::utils::BigPrimeField> {
            value: halo2_base::AssignedValue<F>,
            bits: Vec<halo2_base::AssignedValue<F>>,
        }

        fn word_from_bits<F>(
            ctx: &mut halo2_base::Context<F>,
            range: &halo2_base::gates::RangeChip<F>,
            bits: Vec<halo2_base::AssignedValue<F>>,
        ) -> Word<F>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            use halo2_base::{QuantumCell::Existing, gates::GateInstructions as _};
            assert_eq!(bits.len(), 32, "SHA-256 words contain 32 bits");
            let gate = range.gate();
            let value = gate.inner_product(
                ctx,
                bits.iter().copied().map(Existing),
                gate.pow_of_two()[..32]
                    .iter()
                    .copied()
                    .map(halo2_base::QuantumCell::Constant),
            );
            Word { value, bits }
        }

        fn constant_word<F>(ctx: &mut halo2_base::Context<F>, value: u32) -> Word<F>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            let bits = (0..32)
                .map(|bit| ctx.load_constant(F::from(u64::from((value >> bit) & 1))))
                .collect();
            let value = ctx.load_constant(F::from(u64::from(value)));
            Word { value, bits }
        }

        fn xor_bit<F>(
            ctx: &mut halo2_base::Context<F>,
            range: &halo2_base::gates::RangeChip<F>,
            lhs: halo2_base::AssignedValue<F>,
            rhs: halo2_base::AssignedValue<F>,
        ) -> halo2_base::AssignedValue<F>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            use halo2_base::{QuantumCell::Existing, gates::GateInstructions as _};
            let gate = range.gate();
            let product = gate.mul(ctx, Existing(lhs), Existing(rhs));
            let sum = gate.add(ctx, Existing(lhs), Existing(rhs));
            let twice = gate.mul(
                ctx,
                Existing(product),
                halo2_base::QuantumCell::Constant(F::from(2)),
            );
            gate.sub(ctx, Existing(sum), Existing(twice))
        }

        fn rotate_right<F>(word: &Word<F>, amount: usize) -> Vec<halo2_base::AssignedValue<F>>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            (0..32).map(|bit| word.bits[(bit + amount) % 32]).collect()
        }

        fn shift_right<F>(
            ctx: &mut halo2_base::Context<F>,
            word: &Word<F>,
            amount: usize,
        ) -> Vec<halo2_base::AssignedValue<F>>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            let zero = ctx.load_constant(F::ZERO);
            (0..32)
                .map(|bit| word.bits.get(bit + amount).copied().unwrap_or(zero))
                .collect()
        }

        fn xor_bit_vectors<F>(
            ctx: &mut halo2_base::Context<F>,
            range: &halo2_base::gates::RangeChip<F>,
            vectors: &[Vec<halo2_base::AssignedValue<F>>],
        ) -> Word<F>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            assert!(vectors.len() >= 2);
            let bits = (0..32)
                .map(|bit| {
                    vectors[1..].iter().fold(vectors[0][bit], |acc, vector| {
                        xor_bit(ctx, range, acc, vector[bit])
                    })
                })
                .collect();
            word_from_bits(ctx, range, bits)
        }

        fn choice<F>(
            ctx: &mut halo2_base::Context<F>,
            range: &halo2_base::gates::RangeChip<F>,
            e: &Word<F>,
            f: &Word<F>,
            g: &Word<F>,
        ) -> Word<F>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            use halo2_base::{QuantumCell::Existing, gates::GateInstructions as _};
            let gate = range.gate();
            let bits = (0..32)
                .map(|bit| {
                    let selected = gate.and(ctx, Existing(e.bits[bit]), Existing(f.bits[bit]));
                    let not_e = gate.not(ctx, Existing(e.bits[bit]));
                    let fallback = gate.and(ctx, Existing(not_e), Existing(g.bits[bit]));
                    gate.add(ctx, Existing(selected), Existing(fallback))
                })
                .collect();
            word_from_bits(ctx, range, bits)
        }

        fn majority<F>(
            ctx: &mut halo2_base::Context<F>,
            range: &halo2_base::gates::RangeChip<F>,
            a: &Word<F>,
            b: &Word<F>,
            c: &Word<F>,
        ) -> Word<F>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            use halo2_base::{QuantumCell::Existing, gates::GateInstructions as _};
            let gate = range.gate();
            let bits = (0..32)
                .map(|bit| {
                    let ab = gate.and(ctx, Existing(a.bits[bit]), Existing(b.bits[bit]));
                    let ac = gate.and(ctx, Existing(a.bits[bit]), Existing(c.bits[bit]));
                    let bc = gate.and(ctx, Existing(b.bits[bit]), Existing(c.bits[bit]));
                    let partial = xor_bit(ctx, range, ab, ac);
                    xor_bit(ctx, range, partial, bc)
                })
                .collect();
            word_from_bits(ctx, range, bits)
        }

        fn add_words<F>(
            ctx: &mut halo2_base::Context<F>,
            range: &halo2_base::gates::RangeChip<F>,
            words: &[&Word<F>],
        ) -> Word<F>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            use halo2_base::{
                QuantumCell::{Constant, Existing},
                gates::{GateInstructions as _, RangeInstructions as _},
            };
            assert!(!words.is_empty());
            let host_sum = words
                .iter()
                .fold(0_u64, |sum, word| sum + word.value.value().get_lower_64());
            let result = ctx.load_witness(F::from(host_sum & 0xffff_ffff));
            let quotient = ctx.load_witness(F::from(host_sum >> 32));
            let gate = range.gate();
            let total = gate.sum(ctx, words.iter().map(|word| Existing(word.value)));
            let reconstructed = gate.mul_add(
                ctx,
                Existing(quotient),
                Constant(F::from(1_u64 << 32)),
                Existing(result),
            );
            ctx.constrain_equal(&total, &reconstructed);
            range.range_check(ctx, quotient, 3);
            let bits = gate.num_to_bits(ctx, result, 32);
            Word {
                value: result,
                bits,
            }
        }

        const INITIAL: [u32; 8] = [
            0x6a09_e667,
            0xbb67_ae85,
            0x3c6e_f372,
            0xa54f_f53a,
            0x510e_527f,
            0x9b05_688c,
            0x1f83_d9ab,
            0x5be0_cd19,
        ];
        const ROUND: [u32; 64] = [
            0x428a_2f98,
            0x7137_4491,
            0xb5c0_fbcf,
            0xe9b5_dba5,
            0x3956_c25b,
            0x59f1_11f1,
            0x923f_82a4,
            0xab1c_5ed5,
            0xd807_aa98,
            0x1283_5b01,
            0x2431_85be,
            0x550c_7dc3,
            0x72be_5d74,
            0x80de_b1fe,
            0x9bdc_06a7,
            0xc19b_f174,
            0xe49b_69c1,
            0xefbe_4786,
            0x0fc1_9dc6,
            0x240c_a1cc,
            0x2de9_2c6f,
            0x4a74_84aa,
            0x5cb0_a9dc,
            0x76f9_88da,
            0x983e_5152,
            0xa831_c66d,
            0xb003_27c8,
            0xbf59_7fc7,
            0xc6e0_0bf3,
            0xd5a7_9147,
            0x06ca_6351,
            0x1429_2967,
            0x27b7_0a85,
            0x2e1b_2138,
            0x4d2c_6dfc,
            0x5338_0d13,
            0x650a_7354,
            0x766a_0abb,
            0x81c2_c92e,
            0x9272_2c85,
            0xa2bf_e8a1,
            0xa81a_664b,
            0xc24b_8b70,
            0xc76c_51a3,
            0xd192_e819,
            0xd699_0624,
            0xf40e_3585,
            0x106a_a070,
            0x19a4_c116,
            0x1e37_6c08,
            0x2748_774c,
            0x34b0_bcb5,
            0x391c_0cb3,
            0x4ed8_aa4a,
            0x5b9c_ca4f,
            0x682e_6ff3,
            0x748f_82ee,
            0x78a5_636f,
            0x84c8_7814,
            0x8cc7_0208,
            0x90be_fffa,
            0xa450_6ceb,
            0xbef9_a3f7,
            0xc671_78f2,
        ];

        let mut byte_bits = Vec::with_capacity(message.len() + 72);
        for byte in message {
            range.range_check(ctx, *byte, 8);
            byte_bits.push(range.gate().num_to_bits(ctx, *byte, 8));
        }
        let bit_length = u64::try_from(message.len())
            .expect("fixed SHA-256 message length fits u64")
            .checked_mul(8)
            .expect("fixed SHA-256 bit length fits u64");
        let mut padding = vec![0x80_u8];
        while (message.len() + padding.len()) % 64 != 56 {
            padding.push(0);
        }
        padding.extend_from_slice(&bit_length.to_be_bytes());
        for byte in padding {
            byte_bits.push(
                (0..8)
                    .map(|bit| ctx.load_constant(F::from(u64::from((byte >> bit) & 1))))
                    .collect(),
            );
        }
        assert_eq!(byte_bits.len() % 64, 0);

        let mut state = INITIAL.map(|value| constant_word(ctx, value));
        for block in byte_bits.chunks_exact(64) {
            let mut schedule = Vec::with_capacity(64);
            for bytes in block.chunks_exact(4) {
                let bits = bytes
                    .iter()
                    .rev()
                    .flat_map(|byte| byte.iter().copied())
                    .collect();
                schedule.push(word_from_bits(ctx, range, bits));
            }
            for index in 16..64 {
                let shifted_15 = shift_right(ctx, &schedule[index - 15], 3);
                let s0 = xor_bit_vectors(
                    ctx,
                    range,
                    &[
                        rotate_right(&schedule[index - 15], 7),
                        rotate_right(&schedule[index - 15], 18),
                        shifted_15,
                    ],
                );
                let shifted_2 = shift_right(ctx, &schedule[index - 2], 10);
                let s1 = xor_bit_vectors(
                    ctx,
                    range,
                    &[
                        rotate_right(&schedule[index - 2], 17),
                        rotate_right(&schedule[index - 2], 19),
                        shifted_2,
                    ],
                );
                let next = add_words(
                    ctx,
                    range,
                    &[&schedule[index - 16], &s0, &schedule[index - 7], &s1],
                );
                schedule.push(next);
            }

            let mut working = state.clone();
            for round in 0..64 {
                let sigma1 = xor_bit_vectors(
                    ctx,
                    range,
                    &[
                        rotate_right(&working[4], 6),
                        rotate_right(&working[4], 11),
                        rotate_right(&working[4], 25),
                    ],
                );
                let choose = choice(ctx, range, &working[4], &working[5], &working[6]);
                let round_constant = constant_word(ctx, ROUND[round]);
                let t1 = add_words(
                    ctx,
                    range,
                    &[
                        &working[7],
                        &sigma1,
                        &choose,
                        &round_constant,
                        &schedule[round],
                    ],
                );
                let sigma0 = xor_bit_vectors(
                    ctx,
                    range,
                    &[
                        rotate_right(&working[0], 2),
                        rotate_right(&working[0], 13),
                        rotate_right(&working[0], 22),
                    ],
                );
                let majority = majority(ctx, range, &working[0], &working[1], &working[2]);
                let t2 = add_words(ctx, range, &[&sigma0, &majority]);
                let next_a = add_words(ctx, range, &[&t1, &t2]);
                let next_e = add_words(ctx, range, &[&working[3], &t1]);
                working = [
                    next_a,
                    working[0].clone(),
                    working[1].clone(),
                    working[2].clone(),
                    next_e,
                    working[4].clone(),
                    working[5].clone(),
                    working[6].clone(),
                ];
            }
            state = std::array::from_fn(|index| {
                add_words(ctx, range, &[&state[index], &working[index]])
            });
        }

        state.map(|word| word.value)
    }
}

/// Exact fixed public instances carried beside one recursive step proof.
///
/// Terminal verification cannot recover the predecessor's public instances
/// from the newest statement. Carrying these fixed sixteen 64-bit limbs keeps
/// the proof window constant-size while preventing a host from verifying a
/// valid proof against substituted instances. The layout is exactly the V3
/// StepEq/StepEp schema declared by the authenticated verifier records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
#[cfg(test)]
pub struct KagemushaLeapfrogPublicInputsV1 {
    /// Canonical current public-statement digest limbs.
    pub public_statement_digest: [u64; 4],
    /// Previous recursive-state digest; zero only at initialization.
    pub previous_state_digest: [u64; 4],
    /// Resulting recursive-state digest.
    pub result_state_digest: [u64; 4],
    /// Authenticated artifact-manifest SHA-256 limbs.
    pub manifest_sha256: [u64; 4],
}

#[cfg(test)]
impl KagemushaLeapfrogPublicInputsV1 {
    /// Convert the exact fixed wire into one Halo2 instance column.
    #[must_use]
    pub fn instance_column<F: PrimeField>(&self) -> Vec<F> {
        self.public_statement_digest
            .into_iter()
            .chain(self.previous_state_digest)
            .chain(self.result_state_digest)
            .chain(self.manifest_sha256)
            .map(F::from)
            .collect()
    }

    /// Validate the non-zero fixed public bindings for one step.
    pub fn validate(&self, proof_step_count: u32) -> Result<(), String> {
        if proof_step_count == 0
            || self.public_statement_digest == [0; 4]
            || self.result_state_digest == [0; 4]
            || self.manifest_sha256 == [0; 4]
            || (proof_step_count == 1) != (self.previous_state_digest == [0; 4])
        {
            return Err("Kagemusha leapfrog public-instance shape mismatch".to_owned());
        }
        Ok(())
    }
}

/// One canonical non-zero coefficient in the fixed verifier's point namespace.
///
/// This is prover/circuit material and is never serialized into a peer proof
/// window. The next two circuit layers recompute and bind its digest.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[cfg(test)]
pub struct KagemushaDeferredEquationTermV1 {
    /// Index into transcript points followed by authenticated fixed-VK points.
    pub point_source_index: u16,
    /// Canonical scalar bytes in the proof curve's scalar field.
    pub coefficient: [u8; 32],
}

/// Complete deterministic residual selected by one fixed proof transcript.
///
/// The native-point and native-scalar halves independently reconstruct the
/// exact fixed `u32` vector and constrain every limb equal. No cross-field hash
/// or caller-provided coefficient joins the halves.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[cfg(test)]
pub struct KagemushaDeferredEquationBindingV1 {
    /// Parity of the proof whose residual is described.
    pub parity: KagemushaPastaCycleParityV1,
    /// Exact canonical augmented proof transcript consumed by both half verifiers.
    pub proof_bytes: Vec<u8>,
    /// Exact instance-column lengths in verifier order.
    pub instance_column_lengths: Vec<u32>,
    /// Exact field-neutral `u32` instance limbs, flattened by column.
    pub instance_limbs: Vec<u32>,
    /// SHA-256 of the exact public-input schema.
    pub public_inputs_schema_sha256: [u8; 32],
    /// SHA-256 of the authenticated fixed verifying key.
    pub verifier_key_sha256: [u8; 32],
    /// SHA-256 of the authenticated artifact manifest.
    pub manifest_sha256: [u8; 32],
    /// Exact canonical scalar sequence absorbed by the Poseidon transcript.
    ///
    /// Point coordinates appear here after the protocol's specified reduction
    /// into the proof scalar field. The reciprocal native-point half derives
    /// the same sequence from canonical proof/VK point bytes before hashing,
    /// preventing free transcript-coordinate witnesses in the scalar half.
    pub transcript_scalars: Vec<[u8; 32]>,
    /// Strictly source-ordered, duplicate-free residual terms.
    pub terms: Vec<KagemushaDeferredEquationTermV1>,
}

/// Exact cross-field transport of one deferred equation.
///
/// Every value is represented by canonical little-endian `u32` limbs, so the
/// Fp and Fq half circuits can equality-constrain the same bytes without field
/// reduction or a cross-field hash assumption.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg(test)]
pub struct KagemushaDeferredEquationVectorV1 {
    /// Exact vector used only as the constrained SHA gadget's KAT oracle.
    pub limbs: Vec<u32>,
}

#[cfg(test)]
fn write_u32_limbs(target: &mut [u32], bytes: &[u8; 32]) {
    assert_eq!(target.len(), 8, "32-byte values use eight exact u32 limbs");
    for (limb, chunk) in target.iter_mut().zip(bytes.chunks_exact(4)) {
        *limb = u32::from_le_bytes(chunk.try_into().expect("four-byte exact limb"));
    }
}

#[cfg(test)]
fn append_exact_byte_limbs(target: &mut Vec<u32>, bytes: &[u8]) {
    for chunk in bytes.chunks(4) {
        let mut padded = [0_u8; 4];
        padded[..chunk.len()].copy_from_slice(chunk);
        target.push(u32::from_le_bytes(padded));
    }
}

#[cfg(test)]
fn canonical_nonzero_scalar<F: PrimeField>(bytes: &[u8; 32]) -> bool {
    let mut repr = F::Repr::default();
    if repr.as_ref().len() != bytes.len() {
        return false;
    }
    repr.as_mut().copy_from_slice(bytes);
    Option::<F>::from(F::from_repr(repr)).is_some_and(|value| value != F::ZERO)
}

#[cfg(test)]
impl KagemushaDeferredEquationBindingV1 {
    /// Validate the exact fixed-verifier equation shape and scalar field.
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.public_inputs_schema_sha256,
            self.verifier_key_sha256,
            self.manifest_sha256,
        ]
        .contains(&[0; 32])
            || self.proof_bytes.is_empty()
            || self.proof_bytes.len() > KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1
            || self.instance_column_lengths.is_empty()
            || self.instance_column_lengths.contains(&0)
            || self
                .instance_column_lengths
                .iter()
                .try_fold(0_usize, |sum, len| {
                    usize::try_from(*len)
                        .ok()
                        .and_then(|len| sum.checked_add(len))
                })
                != Some(self.instance_limbs.len())
            || self.terms.len() != KAGEMUSHA_DEFERRED_EQUATION_TERM_COUNT_V1
            || self.transcript_scalars.is_empty()
            || self.transcript_scalars.len() > KAGEMUSHA_DEFERRED_TRANSCRIPT_SCALAR_MAX_V1
        {
            return Err("Kagemusha deferred equation binding shape mismatch".to_owned());
        }
        for scalar in &self.transcript_scalars {
            let canonical = match self.parity {
                KagemushaPastaCycleParityV1::StepEq => {
                    let mut repr = <Fp as PrimeField>::Repr::default();
                    repr.as_mut().copy_from_slice(scalar);
                    Option::<Fp>::from(Fp::from_repr(repr)).is_some()
                }
                KagemushaPastaCycleParityV1::StepEp => {
                    let mut repr = <Fq as PrimeField>::Repr::default();
                    repr.as_mut().copy_from_slice(scalar);
                    Option::<Fq>::from(Fq::from_repr(repr)).is_some()
                }
            };
            if !canonical {
                return Err("Kagemusha deferred transcript scalar is invalid".to_owned());
            }
        }
        for (index, term) in self.terms.iter().enumerate() {
            if index > 0 && self.terms[index - 1].point_source_index >= term.point_source_index {
                return Err(
                    "Kagemusha deferred equation point sources are not canonical".to_owned(),
                );
            }
            let canonical = match self.parity {
                KagemushaPastaCycleParityV1::StepEq => {
                    canonical_nonzero_scalar::<Fp>(&term.coefficient)
                }
                KagemushaPastaCycleParityV1::StepEp => {
                    canonical_nonzero_scalar::<Fq>(&term.coefficient)
                }
            };
            if !canonical {
                return Err("Kagemusha deferred equation coefficient is invalid".to_owned());
            }
        }
        Ok(())
    }

    /// Return a host-side SHA-256 identity for diagnostics and caches.
    pub fn host_identity_sha256(&self) -> Result<[u8; 32], String> {
        self.validate()?;
        let encoded = norito::to_bytes(self)
            .map_err(|error| format!("failed to encode Kagemusha deferred equation: {error}"))?;
        let mut hasher = Sha256::new();
        hasher.update(KAGEMUSHA_DEFERRED_EQUATION_DIGEST_DOMAIN_V1);
        hasher.update([0]);
        hasher.update(encoded);
        Ok(hasher.finalize().into())
    }

    /// Return the non-production exact transcript/instance KAT oracle.
    ///
    /// Production half circuits must constrain a shared SHA-256 gadget over
    /// these exact bytes. This host builder is never accepted as proof identity.
    pub fn exact_vector(&self) -> Result<KagemushaDeferredEquationVectorV1, String> {
        self.validate()?;
        let proof_limb_count = self.proof_bytes.len().div_ceil(4);
        let capacity = KAGEMUSHA_DEFERRED_EQUATION_EXACT_HEADER_LIMBS_V1
            + KAGEMUSHA_DEFERRED_EQUATION_FIXED_IDENTITY_LIMBS_V1
            + proof_limb_count
            + self.instance_column_lengths.len()
            + self.instance_limbs.len()
            + self.transcript_scalars.len() * 8
            + KAGEMUSHA_DEFERRED_EQUATION_DERIVED_LIMBS_V1;
        let mut limbs = Vec::with_capacity(capacity);
        limbs.push(1);
        limbs.push(match self.parity {
            KagemushaPastaCycleParityV1::StepEq => 1,
            KagemushaPastaCycleParityV1::StepEp => 2,
        });
        limbs.push(
            u32::try_from(self.proof_bytes.len())
                .map_err(|_| "Kagemusha deferred proof length does not fit u32".to_owned())?,
        );
        limbs.push(
            u32::try_from(proof_limb_count)
                .map_err(|_| "Kagemusha deferred proof limb count does not fit u32".to_owned())?,
        );
        limbs.push(
            u32::try_from(self.instance_column_lengths.len()).map_err(|_| {
                "Kagemusha deferred instance column count does not fit u32".to_owned()
            })?,
        );
        limbs.push(
            u32::try_from(self.instance_limbs.len())
                .map_err(|_| "Kagemusha deferred instance count does not fit u32".to_owned())?,
        );
        limbs.push(
            u32::try_from(self.terms.len()).map_err(|_| {
                "Kagemusha deferred equation term count does not fit u32".to_owned()
            })?,
        );
        limbs.push(u32::try_from(self.transcript_scalars.len()).map_err(|_| {
            "Kagemusha deferred transcript scalar count does not fit u32".to_owned()
        })?);
        for digest in [
            self.public_inputs_schema_sha256,
            self.verifier_key_sha256,
            self.manifest_sha256,
        ] {
            let mut digest_limbs = [0_u32; 8];
            write_u32_limbs(&mut digest_limbs, &digest);
            limbs.extend(digest_limbs);
        }
        append_exact_byte_limbs(&mut limbs, &self.proof_bytes);
        limbs.extend(&self.instance_column_lengths);
        limbs.extend(&self.instance_limbs);
        for scalar in &self.transcript_scalars {
            let mut scalar_limbs = [0_u32; 8];
            write_u32_limbs(&mut scalar_limbs, scalar);
            limbs.extend(scalar_limbs);
        }
        for term in &self.terms {
            limbs.push(u32::from(term.point_source_index));
            let mut coefficient_limbs = [0_u32; 8];
            write_u32_limbs(&mut coefficient_limbs, &term.coefficient);
            limbs.extend(coefficient_limbs);
        }
        debug_assert_eq!(limbs.len(), capacity);
        Ok(KagemushaDeferredEquationVectorV1 { limbs })
    }

    /// Return the exact little-endian bytes constrained by both verifier halves.
    pub fn exact_bytes(&self) -> Result<Vec<u8>, String> {
        Ok(self
            .exact_vector()?
            .limbs
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect())
    }
}

#[cfg(test)]
impl KagemushaDeferredEquationVectorV1 {
    /// Reject any substituted, omitted, reordered, or non-canonical limb.
    pub fn validate_against(
        &self,
        binding: &KagemushaDeferredEquationBindingV1,
    ) -> Result<(), String> {
        if self != &binding.exact_vector()? {
            return Err("Kagemusha deferred equation exact-vector mismatch".to_owned());
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
#[cfg(test)]
pub struct KagemushaLeapfrogStepProofV1 {
    /// Curve/circuit parity of this proof.
    pub parity: KagemushaPastaCycleParityV1,
    /// Recursive transition count proved by this layer.
    pub proof_step_count: u32,
    /// Exact public instance column used to verify `proof_bytes`.
    pub public_inputs: KagemushaLeapfrogPublicInputsV1,
    /// Ordinary Poseidon Halo2/IPA proof plus the canonical folded generator.
    pub proof_bytes: Vec<u8>,
}

#[cfg(test)]
impl KagemushaLeapfrogStepProofV1 {
    /// Validate the bounded, non-empty fixed-circuit wire shape.
    pub fn validate(&self) -> Result<(), String> {
        if self.proof_step_count == 0
            || self.proof_bytes.is_empty()
            || self.proof_bytes.len() > KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1
        {
            return Err("Kagemusha leapfrog step proof shape mismatch".to_owned());
        }
        self.public_inputs.validate(self.proof_step_count)?;
        Ok(())
    }
}

/// Constant-size newest/predecessor proof window transported by one bundle.
///
/// The production two-half window carries current `Eq_i` and `Ep_i`. `Eq_i`
/// proves the application and closes each parent `Eq_j`; `Ep_i` closes each
/// parent `Ep_j`. Exact deferred-equation vectors join the native point/scalar
/// halves. A terminal verifier fully verifies and decides both current proofs.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[cfg(test)]
pub struct KagemushaLeapfrogProofWindowV1 {
    /// Wire layout version.
    pub version: u16,
    /// Proof for the current public statement.
    pub newest: KagemushaLeapfrogStepProofV1,
    /// Previous proof, absent only for recursive step one.
    pub predecessor: Option<KagemushaLeapfrogStepProofV1>,
}

#[cfg(test)]
fn opposite_parity(parity: KagemushaPastaCycleParityV1) -> KagemushaPastaCycleParityV1 {
    match parity {
        KagemushaPastaCycleParityV1::StepEq => KagemushaPastaCycleParityV1::StepEp,
        KagemushaPastaCycleParityV1::StepEp => KagemushaPastaCycleParityV1::StepEq,
    }
}

#[cfg(test)]
impl KagemushaLeapfrogProofWindowV1 {
    /// Validate the exact two-layer window and its canonical archive budget.
    pub fn validate(&self) -> Result<(), String> {
        if self.version != KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1 {
            return Err("Kagemusha leapfrog proof-window version mismatch".to_owned());
        }
        self.newest.validate()?;
        match (&self.predecessor, self.newest.proof_step_count) {
            (None, 1) => {}
            (Some(predecessor), newest_step) if newest_step > 1 => {
                predecessor.validate()?;
                if predecessor.proof_step_count.checked_add(1) != Some(newest_step)
                    || predecessor.parity != opposite_parity(self.newest.parity)
                    || predecessor.proof_bytes == self.newest.proof_bytes
                    || predecessor.public_inputs.result_state_digest
                        != self.newest.public_inputs.previous_state_digest
                    || predecessor.public_inputs.manifest_sha256
                        != self.newest.public_inputs.manifest_sha256
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
        if encoded.len() > KAGEMUSHA_LEAPFROG_PROOF_WINDOW_MAX_BYTES_V1 {
            return Err(format!(
                "Kagemusha leapfrog proof window is {} bytes; maximum is {}",
                encoded.len(),
                KAGEMUSHA_LEAPFROG_PROOF_WINDOW_MAX_BYTES_V1
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
    pub fn advance(previous: &Self, newest: KagemushaLeapfrogStepProofV1) -> Result<Self, String> {
        previous.validate()?;
        newest.validate()?;
        if newest.proof_step_count != previous.newest.proof_step_count.saturating_add(1)
            || newest.parity != opposite_parity(previous.newest.parity)
            || newest.proof_bytes == previous.newest.proof_bytes
        {
            return Err("Kagemusha leapfrog window advance mismatch".to_owned());
        }
        let window = Self {
            version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1,
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
        hasher.update(KAGEMUSHA_LEAPFROG_PROOF_WINDOW_DIGEST_DOMAIN_V1);
        hasher.update([0]);
        hasher.update(encoded);
        Ok(hasher.finalize().into())
    }
}

#[cfg(test)]
mod tests {
    use std::{mem, rc::Rc};

    use super::*;
    use norito::to_bytes;

    use halo2_proofs::{
        arithmetic::Field,
        circuit::{Layouter, SimpleFloorPlanner, Value},
        plonk::{Advice, Circuit, Column, ConstraintSystem, Error as PlonkError, Instance},
    };

    use crate::zk::halo2_backend::assign_advice_compat;

    /// Keep the exact same-field Pasta recursion tuples executable.
    ///
    /// An Eq IPA proof uses `ParamsIPA<EqAffine>` and has scalar field `Fp`, so
    /// its direct Axiom circuit verifier must also be an `Fp` circuit with a
    /// `Halo2Loader<EqAffine, BaseFieldEccChip<EqAffine>>`. The reciprocal Ep
    /// tuple is `ParamsIPA<EpAffine>` / `Fq` /
    /// `Halo2Loader<EpAffine, BaseFieldEccChip<EpAffine>>`. This test is a
    /// compile-time guard against accidentally diagnosing that supported path
    /// as a Pasta trait mismatch.
    #[test]
    fn same_field_pasta_loader_type_tuples_compile() {
        use halo2_base::gates::circuit::{BaseCircuitParams, builder::BaseCircuitBuilder};
        use halo2_ecc::{ecc::BaseFieldEccChip, fields::fp::FpChip};
        use halo2_proofs::halo2curves::pasta::{EpAffine, EqAffine};
        use snark_verifier::loader::halo2::Halo2Loader;

        const LIMB_BITS: usize = 86;
        const LIMBS: usize = 3;
        let seed = BaseCircuitParams {
            k: 12,
            num_advice_per_phase: vec![1],
            num_lookup_advice_per_phase: vec![1],
            num_fixed: 1,
            lookup_bits: Some(11),
            num_instance_columns: 1,
        };

        let mut eq_outer = BaseCircuitBuilder::<Fp>::new(false).use_params(seed.clone());
        let eq_range = eq_outer.range_chip();
        let eq_base = FpChip::<Fp, Fq>::new(&eq_range, LIMB_BITS, LIMBS);
        let eq_loader = Halo2Loader::new(
            BaseFieldEccChip::<EqAffine>::new(&eq_base),
            mem::take(eq_outer.pool(0)),
        );
        fn require_eq_tuple(_: &Rc<Halo2Loader<EqAffine, BaseFieldEccChip<'_, EqAffine>>>) {}
        require_eq_tuple(&eq_loader);
        *eq_outer.pool(0) = eq_loader.take_ctx();

        let mut ep_outer = BaseCircuitBuilder::<Fq>::new(false).use_params(seed);
        let ep_range = ep_outer.range_chip();
        let ep_base = FpChip::<Fq, Fp>::new(&ep_range, LIMB_BITS, LIMBS);
        let ep_loader = Halo2Loader::new(
            BaseFieldEccChip::<EpAffine>::new(&ep_base),
            mem::take(ep_outer.pool(0)),
        );
        fn require_ep_tuple(_: &Rc<Halo2Loader<EpAffine, BaseFieldEccChip<'_, EpAffine>>>) {}
        require_ep_tuple(&ep_loader);
        *ep_outer.pool(0) = ep_loader.take_ctx();
    }

    fn leapfrog_step(
        parity: KagemushaPastaCycleParityV1,
        proof_step_count: u32,
        byte: u8,
    ) -> KagemushaLeapfrogStepProofV1 {
        let state = |step: u32| {
            let base = u64::from(step) * 10;
            [base + 1, base + 2, base + 3, base + 4]
        };
        KagemushaLeapfrogStepProofV1 {
            parity,
            proof_step_count,
            public_inputs: KagemushaLeapfrogPublicInputsV1 {
                public_statement_digest: state(proof_step_count + 100),
                previous_state_digest: if proof_step_count == 1 {
                    [0; 4]
                } else {
                    state(proof_step_count - 1)
                },
                result_state_digest: state(proof_step_count),
                manifest_sha256: [501, 502, 503, 504],
            },
            proof_bytes: vec![byte; 1_536],
        }
    }

    #[test]
    fn compact_leapfrog_window_is_constant_through_step_64() {
        let mut window = KagemushaLeapfrogProofWindowV1 {
            version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1,
            newest: leapfrog_step(KagemushaPastaCycleParityV1::StepEq, 1, 1),
            predecessor: None,
        };
        window.validate().expect("valid initialization window");
        let init_size = to_bytes(&window).expect("encode init window").len();

        let mut steady_size = None;
        for step in 2_u32..=64 {
            let parity = opposite_parity(window.newest.parity);
            window = KagemushaLeapfrogProofWindowV1::advance(
                &window,
                leapfrog_step(parity, step, u8::try_from(step).expect("bounded step")),
            )
            .expect("advance leapfrog window");
            let encoded = to_bytes(&window).expect("encode steady window");
            assert!(encoded.len() > init_size);
            assert!(encoded.len() <= KAGEMUSHA_LEAPFROG_PROOF_WINDOW_MAX_BYTES_V1);
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
                    .proof_step_count,
                step - 1
            );
        }
    }

    #[test]
    fn compact_leapfrog_window_rejects_parity_step_and_proof_substitution() {
        let init = KagemushaLeapfrogProofWindowV1 {
            version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1,
            newest: leapfrog_step(KagemushaPastaCycleParityV1::StepEq, 1, 1),
            predecessor: None,
        };
        let valid = KagemushaLeapfrogProofWindowV1::advance(
            &init,
            leapfrog_step(KagemushaPastaCycleParityV1::StepEp, 2, 2),
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
            .proof_step_count = 2;
        assert!(wrong_step.validate().is_err());

        let mut wrong_parity = valid.clone();
        wrong_parity
            .predecessor
            .as_mut()
            .expect("predecessor")
            .parity = KagemushaPastaCycleParityV1::StepEp;
        assert!(wrong_parity.validate().is_err());

        let mut duplicated_proof = valid.clone();
        let newest_proof = duplicated_proof.newest.proof_bytes.clone();
        duplicated_proof
            .predecessor
            .as_mut()
            .expect("predecessor")
            .proof_bytes = newest_proof;
        assert!(duplicated_proof.validate().is_err());

        let original_digest = valid.digest().expect("valid digest");
        let mut substituted = valid;
        substituted.newest.proof_bytes[0] ^= 1;
        assert_ne!(
            original_digest,
            substituted
                .digest()
                .expect("substituted window remains shaped")
        );

        let valid = KagemushaLeapfrogProofWindowV1::advance(
            &init,
            leapfrog_step(KagemushaPastaCycleParityV1::StepEp, 2, 2),
        )
        .expect("valid second layer");
        for field in ["statement", "previous_state", "result_state", "manifest"] {
            let mut substituted = valid.clone();
            match field {
                "statement" => substituted.newest.public_inputs.public_statement_digest[0] ^= 1,
                "previous_state" => substituted.newest.public_inputs.previous_state_digest[0] ^= 1,
                "result_state" => substituted.newest.public_inputs.result_state_digest[0] ^= 1,
                "manifest" => substituted.newest.public_inputs.manifest_sha256[0] ^= 1,
                _ => unreachable!(),
            }
            let validation = substituted.validate();
            if matches!(field, "previous_state" | "manifest") {
                assert!(
                    validation.is_err(),
                    "cross-proof {field} substitution must fail structurally"
                );
            } else {
                assert_ne!(
                    valid.digest().expect("valid digest"),
                    substituted.digest().expect("independently bound instances"),
                    "{field} substitution must change the proof-window identity"
                );
            }
        }
    }

    #[test]
    fn compact_leapfrog_window_rejects_per_step_and_total_budget_overflow() {
        let oversized = KagemushaLeapfrogProofWindowV1 {
            version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1,
            newest: KagemushaLeapfrogStepProofV1 {
                parity: KagemushaPastaCycleParityV1::StepEq,
                proof_step_count: 1,
                public_inputs: leapfrog_step(KagemushaPastaCycleParityV1::StepEq, 1, 1)
                    .public_inputs,
                proof_bytes: vec![0xA5; KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1 + 1],
            },
            predecessor: None,
        };
        assert!(oversized.validate().is_err());

        let maximum = KagemushaLeapfrogProofWindowV1 {
            version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1,
            newest: KagemushaLeapfrogStepProofV1 {
                parity: KagemushaPastaCycleParityV1::StepEp,
                proof_step_count: 2,
                public_inputs: leapfrog_step(KagemushaPastaCycleParityV1::StepEp, 2, 2)
                    .public_inputs,
                proof_bytes: vec![0xA5; KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1],
            },
            predecessor: Some(KagemushaLeapfrogStepProofV1 {
                parity: KagemushaPastaCycleParityV1::StepEq,
                proof_step_count: 1,
                public_inputs: leapfrog_step(KagemushaPastaCycleParityV1::StepEq, 1, 1)
                    .public_inputs,
                proof_bytes: vec![0x5A; KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1],
            }),
        };
        let encoded_len = to_bytes(&maximum).expect("encode maximum window").len();
        assert!(
            encoded_len <= KAGEMUSHA_LEAPFROG_PROOF_WINDOW_MAX_BYTES_V1,
            "declared per-step maxima must fit the complete window: {encoded_len}"
        );
        maximum.validate().expect("bounded maximum window");
    }

    fn exact_state(step: u32) -> Vec<u32> {
        let mut state =
            vec![0; iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V1];
        state[0] =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V1;
        state[1] = step;
        for (index, limb) in state.iter_mut().enumerate().skip(2) {
            *limb = step
                .wrapping_mul(1_003)
                .wrapping_add(u32::try_from(index).expect("state-vector index fits u32"));
        }
        let offset = |field: &str| {
            crate::zk::kagemusha_v2::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_V1
                .iter()
                .find_map(|(name, start, _)| (*name == field).then_some(*start))
                .expect("state fixture field exists")
        };
        state[offset("proof_step_count")] = step;
        state[offset("peer_hop_count")] = step
            .saturating_sub(1)
            .min(iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2);
        let manifest = offset("artifact_manifest_sha256");
        for (index, limb) in state[manifest..manifest + 8].iter_mut().enumerate() {
            *limb = 0xA500_0000 | u32::try_from(index + 1).expect("digest index fits u32");
        }
        state
    }

    fn proof_pair(step: u32) -> KagemushaPastaCycleProofPairV1 {
        let parent_count = if step == 1 {
            0
        } else if step > 2 && step % 3 == 0 {
            2
        } else {
            1
        };
        let mut parent_states = std::array::from_fn(|_| {
            vec![0; iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V1]
        });
        let mut parent_eq_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
        let mut parent_ep_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
        for slot in 0..usize::try_from(parent_count).expect("bounded parent count") {
            parent_states[slot] =
                exact_state(step - parent_count + u32::try_from(slot).expect("slot fits"));
            parent_eq_deferred_sha256[slot] = std::array::from_fn(|index| {
                0xE100_0000
                    | (u32::try_from(slot).expect("slot fits") << 8)
                    | u32::try_from(index + 1).expect("digest index fits")
            });
            parent_ep_deferred_sha256[slot] = std::array::from_fn(|index| {
                0xE200_0000
                    | (u32::try_from(slot).expect("slot fits") << 8)
                    | u32::try_from(index + 1).expect("digest index fits")
            });
        }
        KagemushaPastaCycleProofPairV1 {
            version: KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V1,
            proof_step_count: step,
            public_inputs: KagemushaPastaCyclePublicInputsV1 {
                public_statement_digest: std::array::from_fn(|index| {
                    step.wrapping_add(u32::try_from(index + 1).expect("digest index fits u32"))
                }),
                parent_count,
                parent_states,
                result_state: exact_state(step),
                manifest_sha256: std::array::from_fn(|index| {
                    0xA500_0000 | u32::try_from(index + 1).expect("digest index fits u32")
                }),
                parent_eq_deferred_sha256,
                parent_ep_deferred_sha256,
            },
            step_eq_proof_bytes: vec![u8::try_from(step).expect("bounded fixture step"); 1_536],
            step_ep_proof_bytes: vec![
                u8::try_from(step).expect("bounded fixture step") ^ 0x80;
                1_536
            ],
        }
    }

    #[test]
    fn exact_state_proof_pair_is_constant_size_through_step_64() {
        let mut pair = proof_pair(1);
        pair.validate().expect("valid initial proof pair");
        let expected_size = to_bytes(&pair).expect("encode proof pair").len();
        eprintln!("Kagemusha maximum-shape exact proof pair bytes: {expected_size}");
        assert!(expected_size <= KAGEMUSHA_PASTA_PROOF_PAIR_MAX_BYTES_V1);
        for step in 2..=64 {
            pair = proof_pair(step);
            pair.validate().expect("validate exact-state proof pair");
            assert_eq!(
                to_bytes(&pair).expect("encode proof pair").len(),
                expected_size
            );
        }
    }

    #[test]
    fn exact_state_proof_pair_rejects_every_binding_substitution() {
        let valid = proof_pair(2);
        valid.validate().expect("valid proof pair");
        let digest = valid.digest().expect("valid pair digest");
        for mutation in [
            "version",
            "step",
            "statement",
            "parent_count",
            "parent_length",
            "parent_limb",
            "parent_padding",
            "parent_eq_join",
            "parent_ep_join",
            "result_length",
            "result_layout",
            "manifest",
            "eq_empty",
            "ep_empty",
            "duplicate_proofs",
            "eq_oversize",
            "ep_oversize",
        ] {
            let mut candidate = valid.clone();
            match mutation {
                "version" => candidate.version += 1,
                "step" => candidate.proof_step_count = 1,
                "statement" => candidate.public_inputs.public_statement_digest[0] ^= 1,
                "parent_count" => candidate.public_inputs.parent_count = 0,
                "parent_length" => {
                    candidate.public_inputs.parent_states[0].pop();
                }
                "parent_limb" => candidate.public_inputs.parent_states[0][17] ^= 1,
                "parent_padding" => candidate.public_inputs.parent_states[1][17] ^= 1,
                "parent_eq_join" => candidate.public_inputs.parent_eq_deferred_sha256[0] = [0; 8],
                "parent_ep_join" => candidate.public_inputs.parent_ep_deferred_sha256[0] = [0; 8],
                "result_length" => {
                    candidate.public_inputs.result_state.push(0);
                }
                "result_layout" => candidate.public_inputs.result_state[0] ^= 1,
                "manifest" => candidate.public_inputs.manifest_sha256[0] ^= 1,
                "eq_empty" => candidate.step_eq_proof_bytes.clear(),
                "ep_empty" => candidate.step_ep_proof_bytes.clear(),
                "duplicate_proofs" => {
                    candidate.step_ep_proof_bytes = candidate.step_eq_proof_bytes.clone();
                }
                "eq_oversize" => {
                    candidate.step_eq_proof_bytes =
                        vec![1; KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1 + 1];
                }
                "ep_oversize" => {
                    candidate.step_ep_proof_bytes =
                        vec![1; KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1 + 1];
                }
                _ => unreachable!(),
            }
            if matches!(mutation, "statement" | "parent_limb" | "manifest") {
                assert_ne!(
                    candidate.digest().expect("well-shaped substitution"),
                    digest,
                    "{mutation} must change the complete pair identity"
                );
            } else {
                assert!(candidate.validate().is_err(), "{mutation} must reject");
            }
        }

        let mut duplicate_parent = proof_pair(3);
        duplicate_parent.public_inputs.parent_states[1] =
            duplicate_parent.public_inputs.parent_states[0].clone();
        assert!(duplicate_parent.validate().is_err());

        let mut reversed_parents = proof_pair(3);
        reversed_parents.public_inputs.parent_states.swap(0, 1);
        reversed_parents
            .public_inputs
            .parent_eq_deferred_sha256
            .swap(0, 1);
        reversed_parents
            .public_inputs
            .parent_ep_deferred_sha256
            .swap(0, 1);
        assert!(reversed_parents.validate().is_err());
    }

    fn constrained_sha_builder<F>(
        message: &[u8],
        k: usize,
    ) -> halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>
    where
        F: halo2_base::utils::BigPrimeField,
    {
        let mut builder = halo2_base::gates::circuit::builder::BaseCircuitBuilder::new(false)
            .use_k(k)
            .use_lookup_bits(k - 1);
        let range = builder.range_chip();
        let digest = {
            let ctx = builder.main(0);
            let bytes =
                ctx.assign_witnesses(message.iter().copied().map(|byte| F::from(u64::from(byte))));
            KagemushaSha256Chip::digest(ctx, &range, &bytes)
        };
        builder.assigned_instances = vec![digest.to_vec()];
        builder.calculate_params(Some(9));
        builder
    }

    fn sha256_words(message: &[u8]) -> [u32; 8] {
        let digest: [u8; 32] = Sha256::digest(message).into();
        std::array::from_fn(|index| {
            u32::from_be_bytes(
                digest[index * 4..index * 4 + 4]
                    .try_into()
                    .expect("SHA-256 word"),
            )
        })
    }

    #[test]
    fn constrained_sha256_matches_fips_and_padding_boundaries_in_both_pasta_fields() {
        use halo2_proofs::{
            dev::MockProver,
            halo2curves::pasta::{Fp, Fq},
        };

        const K: usize = 20;
        fn check<F>()
        where
            F: halo2_base::utils::BigPrimeField,
        {
            for message in [
                Vec::new(),
                b"abc".to_vec(),
                vec![0x5A; 55],
                vec![0xA5; 56],
                vec![0x11; 63],
                vec![0x22; 64],
                vec![0x33; 65],
            ] {
                let expected = sha256_words(&message)
                    .into_iter()
                    .map(|word| F::from(u64::from(word)))
                    .collect::<Vec<_>>();
                let builder = constrained_sha_builder::<F>(&message, K);
                MockProver::run(K as u32, &builder, vec![expected])
                    .expect("constrained SHA-256 mock prover")
                    .assert_satisfied();
            }
        }

        assert_eq!(
            sha256_words(b""),
            [
                0xe3b0_c442,
                0x98fc_1c14,
                0x9afb_f4c8,
                0x996f_b924,
                0x27ae_41e4,
                0x649b_934c,
                0xa495_991b,
                0x7852_b855,
            ]
        );
        assert_eq!(
            sha256_words(b"abc"),
            [
                0xba78_16bf,
                0x8f01_cfea,
                0x4141_40de,
                0x5dae_2223,
                0xb003_61a3,
                0x9617_7a9c,
                0xb410_ff61,
                0xf200_15ad,
            ]
        );
        check::<Fp>();
        check::<Fq>();
    }

    #[test]
    fn constrained_sha256_rejects_message_and_digest_substitution() {
        use halo2_proofs::{dev::MockProver, halo2curves::pasta::Fp};

        const K: usize = 20;
        let expected = sha256_words(b"abc")
            .into_iter()
            .map(|word| Fp::from(u64::from(word)))
            .collect::<Vec<_>>();
        let substituted_message = constrained_sha_builder::<Fp>(b"abd", K);
        assert!(
            MockProver::run(K as u32, &substituted_message, vec![expected.clone()])
                .expect("message-substitution prover")
                .verify()
                .is_err()
        );

        let original = constrained_sha_builder::<Fp>(b"abc", K);
        let mut substituted_digest = expected;
        substituted_digest[7] += Fp::ONE;
        assert!(
            MockProver::run(K as u32, &original, vec![substituted_digest])
                .expect("digest-substitution prover")
                .verify()
                .is_err()
        );
    }

    #[test]
    fn split_deferred_equation_constrains_scalar_join_and_reciprocal_msm() {
        use std::mem;

        use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
        use halo2_ecc::fields::fp::FpChip;
        use halo2_proofs::{
            dev::MockProver,
            halo2curves::{
                CurveAffine,
                group::{Curve as _, Group as _},
                pasta::{EqAffine, Fp, Fq},
            },
        };
        use snark_verifier::loader::halo2::{EccInstructions, IntegerInstructions};

        use crate::zk::kagemusha_cycle_loader::{
            DeferredScalarEccChip, LIMB_BITS, LIMBS, PastaCycleEccChip,
        };

        const K: usize = 20;
        let generator = EqAffine::generator();
        let doubled = (generator.to_curve() + generator.to_curve()).to_affine();

        let mut scalar_builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(K)
            .use_lookup_bits(K - 1);
        let scalar_range = scalar_builder.range_chip();
        let coordinate = FpChip::<Fp, Fq>::new(&scalar_range, LIMB_BITS, LIMBS);
        let scalar_integer = FpChip::<Fp, Fp>::new(&scalar_range, LIMB_BITS, LIMBS);
        let mut scalar_chip = DeferredScalarEccChip::<EqAffine>::new(&coordinate, &scalar_integer);
        let mut scalar_ctx = mem::take(scalar_builder.pool(0));
        let assigned_generator = scalar_chip.assign_point(&mut scalar_ctx, generator);
        let assigned_doubled = scalar_chip.assign_point(&mut scalar_ctx, doubled);
        let two = scalar_chip
            .scalar_chip()
            .assign_integer(&mut scalar_ctx, Fp::from(2));
        let minus_one = scalar_chip
            .scalar_chip()
            .assign_integer(&mut scalar_ctx, -Fp::ONE);
        let result = scalar_chip.variable_base_msm(
            &mut scalar_ctx,
            &[(&two, &assigned_generator), (&minus_one, &assigned_doubled)],
        );
        let identity = scalar_chip.assign_constant(&mut scalar_ctx, EqAffine::identity());
        scalar_chip.assert_equal(&mut scalar_ctx, &result, &identity);
        let scalar_join = scalar_chip.assigned_equation_bytes(&mut scalar_ctx);
        let scalar_digest =
            KagemushaSha256Chip::digest(scalar_ctx.main(), &scalar_range, &scalar_join);
        let expected_words = sha256_words(
            &scalar_join
                .iter()
                .map(|byte| u8::try_from(byte.value().get_lower_64()).expect("assigned byte"))
                .collect::<Vec<_>>(),
        );
        let equation_witness = scalar_chip.audit().witness();
        *scalar_builder.pool(0) = scalar_ctx;
        scalar_builder.assigned_instances = vec![scalar_digest.to_vec()];
        scalar_builder.calculate_params(Some(9));

        let scalar_instances = expected_words
            .into_iter()
            .map(|word| Fp::from(u64::from(word)))
            .collect::<Vec<_>>();
        MockProver::run(K as u32, &scalar_builder, vec![scalar_instances])
            .expect("deferred scalar-half mock prover")
            .assert_satisfied();

        let mut point_builder = BaseCircuitBuilder::<Fq>::new(false)
            .use_k(K)
            .use_lookup_bits(K - 1);
        let point_range = point_builder.range_chip();
        let base = FpChip::<Fq, Fq>::new(&point_range, LIMB_BITS, LIMBS);
        let scalar = FpChip::<Fq, Fp>::new(&point_range, LIMB_BITS, LIMBS);
        let mut point_chip = PastaCycleEccChip::<EqAffine>::new(&base, &scalar);
        let mut point_ctx = mem::take(point_builder.pool(0));
        let point_audit = point_chip
            .constrain_deferred_equations(&mut point_ctx, &equation_witness)
            .expect("canonical reciprocal point witness");
        let point_join = point_chip.assigned_equation_bytes(&mut point_ctx, &point_audit);
        let point_digest = KagemushaSha256Chip::digest(point_ctx.main(), &point_range, &point_join);
        assert_eq!(
            scalar_join
                .iter()
                .map(|byte| byte.value().get_lower_64())
                .collect::<Vec<_>>(),
            point_join
                .iter()
                .map(|byte| byte.value().get_lower_64())
                .collect::<Vec<_>>(),
            "both constrained halves must hash the exact same bytes"
        );
        *point_builder.pool(0) = point_ctx;
        point_builder.assigned_instances = vec![point_digest.to_vec()];
        point_builder.calculate_params(Some(9));
        let point_instances = expected_words
            .into_iter()
            .map(|word| Fq::from(u64::from(word)))
            .collect::<Vec<_>>();
        MockProver::run(K as u32, &point_builder, vec![point_instances])
            .expect("deferred point-half mock prover")
            .assert_satisfied();

        let mut substituted = equation_witness;
        substituted.equations[0][0].1 += Fp::ONE;
        let mut rejected_builder = BaseCircuitBuilder::<Fq>::new(false)
            .use_k(K)
            .use_lookup_bits(K - 1);
        let rejected_range = rejected_builder.range_chip();
        let rejected_base = FpChip::<Fq, Fq>::new(&rejected_range, LIMB_BITS, LIMBS);
        let rejected_scalar = FpChip::<Fq, Fp>::new(&rejected_range, LIMB_BITS, LIMBS);
        let mut rejected_chip =
            PastaCycleEccChip::<EqAffine>::new(&rejected_base, &rejected_scalar);
        let mut rejected_ctx = mem::take(rejected_builder.pool(0));
        let rejected_audit = rejected_chip
            .constrain_deferred_equations(&mut rejected_ctx, &substituted)
            .expect("shape-preserving substituted witness");
        let rejected_join =
            rejected_chip.assigned_equation_bytes(&mut rejected_ctx, &rejected_audit);
        let rejected_digest =
            KagemushaSha256Chip::digest(rejected_ctx.main(), &rejected_range, &rejected_join);
        *rejected_builder.pool(0) = rejected_ctx;
        rejected_builder.assigned_instances = vec![rejected_digest.to_vec()];
        rejected_builder.calculate_params(Some(9));
        let expected_digest = expected_words
            .into_iter()
            .map(|word| Fq::from(u64::from(word)))
            .collect::<Vec<_>>();
        assert!(
            MockProver::run(K as u32, &rejected_builder, vec![expected_digest])
                .expect("substituted deferred point-half mock prover")
                .verify()
                .is_err(),
            "a coefficient substitution must fail both the MSM and the shared join"
        );
    }

    fn deferred_equation(
        parity: KagemushaPastaCycleParityV1,
    ) -> KagemushaDeferredEquationBindingV1 {
        let terms = (0..KAGEMUSHA_DEFERRED_EQUATION_TERM_COUNT_V1)
            .map(|index| {
                let mut coefficient = [0_u8; 32];
                match parity {
                    KagemushaPastaCycleParityV1::StepEq => {
                        let repr =
                            Fp::from(u64::try_from(index + 1).expect("bounded term")).to_repr();
                        coefficient.copy_from_slice(repr.as_ref());
                    }
                    KagemushaPastaCycleParityV1::StepEp => {
                        let repr =
                            Fq::from(u64::try_from(index + 1).expect("bounded term")).to_repr();
                        coefficient.copy_from_slice(repr.as_ref());
                    }
                }
                KagemushaDeferredEquationTermV1 {
                    point_source_index: u16::try_from(index).expect("bounded source"),
                    coefficient,
                }
            })
            .collect();
        let transcript_scalars = (0..9_u64)
            .map(|index| {
                let mut scalar = [0_u8; 32];
                match parity {
                    KagemushaPastaCycleParityV1::StepEq => {
                        scalar.copy_from_slice(Fp::from(index).to_repr().as_ref());
                    }
                    KagemushaPastaCycleParityV1::StepEp => {
                        scalar.copy_from_slice(Fq::from(index).to_repr().as_ref());
                    }
                }
                scalar
            })
            .collect();
        KagemushaDeferredEquationBindingV1 {
            parity,
            proof_bytes: (0_u8..=12).collect(),
            instance_column_lengths: vec![4, 3],
            instance_limbs: vec![11, 12, 13, 14, 21, 22, 23],
            public_inputs_schema_sha256: [2; 32],
            verifier_key_sha256: [3; 32],
            manifest_sha256: [5; 32],
            transcript_scalars,
            terms,
        }
    }

    #[test]
    fn deferred_equation_exact_oracle_layout_is_canonical() {
        let binding = deferred_equation(KagemushaPastaCycleParityV1::StepEq);
        let vector = binding.exact_vector().expect("exact KAT oracle");
        let proof_limbs = binding.proof_bytes.len().div_ceil(4);
        assert_eq!(vector.limbs[0], 1);
        assert_eq!(vector.limbs[1], 1);
        assert_eq!(
            vector.limbs[2],
            u32::try_from(binding.proof_bytes.len()).unwrap()
        );
        assert_eq!(vector.limbs[3], u32::try_from(proof_limbs).unwrap());
        assert_eq!(vector.limbs[4], 2);
        assert_eq!(vector.limbs[5], 7);
        assert_eq!(vector.limbs[6], 38);
        assert_eq!(vector.limbs[7], 9);
        assert_eq!(
            vector.limbs.len(),
            KAGEMUSHA_DEFERRED_EQUATION_EXACT_HEADER_LIMBS_V1
                + KAGEMUSHA_DEFERRED_EQUATION_FIXED_IDENTITY_LIMBS_V1
                + proof_limbs
                + binding.instance_column_lengths.len()
                + binding.instance_limbs.len()
                + binding.transcript_scalars.len() * 8
                + KAGEMUSHA_DEFERRED_EQUATION_DERIVED_LIMBS_V1
        );
        // Thirteen proof bytes require three canonical zero padding bytes.
        let proof_end = KAGEMUSHA_DEFERRED_EQUATION_EXACT_HEADER_LIMBS_V1
            + KAGEMUSHA_DEFERRED_EQUATION_FIXED_IDENTITY_LIMBS_V1
            + proof_limbs;
        assert_eq!(vector.limbs[proof_end - 1] & 0xFFFF_FF00, 0);
    }

    #[test]
    fn deferred_equation_vector_rejects_omission_reordering_and_substitution() {
        for parity in [
            KagemushaPastaCycleParityV1::StepEq,
            KagemushaPastaCycleParityV1::StepEp,
        ] {
            let binding = deferred_equation(parity);
            binding.validate().expect("canonical deferred equation");
            let digest = binding
                .host_identity_sha256()
                .expect("deferred equation host identity");
            let vector = binding
                .exact_vector()
                .expect("deferred equation exact vector");
            assert!(vector.limbs.iter().any(|limb| *limb != 0));
            assert_eq!(
                vector,
                binding.exact_vector().expect("deterministic exact vector")
            );
            vector
                .validate_against(&binding)
                .expect("exact vector binding");

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
            substituted.proof_bytes[0] ^= 1;
            assert_ne!(
                digest,
                substituted
                    .host_identity_sha256()
                    .expect("bound substitution")
            );
            assert_ne!(
                vector,
                substituted
                    .exact_vector()
                    .expect("exact-vector substitution")
            );
        }

        let eq = deferred_equation(KagemushaPastaCycleParityV1::StepEq)
            .exact_vector()
            .expect("Eq point-half vector");
        let ep = deferred_equation(KagemushaPastaCycleParityV1::StepEp)
            .exact_vector()
            .expect("Ep point-half vector");
        assert_ne!(eq.limbs, ep.limbs, "parity vectors must not alias");

        let mut substituted = eq.clone();
        for index in [0, 1, 2, 7, 31, 35, 42, eq.limbs.len() - 1] {
            substituted.limbs[index] ^= 1;
            assert!(
                substituted
                    .validate_against(&deferred_equation(KagemushaPastaCycleParityV1::StepEq))
                    .is_err(),
                "deferred-vector substitution at limb {index} must reject"
            );
            substituted = eq.clone();
        }
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
            pub(super) point_sources: Vec<Vec<u8>>,
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
                    point_sources: Vec::new(),
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
                let mut repr = <C::Scalar as PrimeField>::Repr::default();
                self.stream.read_exact(repr.as_mut()).map_err(|error| {
                    Error::Transcript(error.kind(), "truncated scalar field".to_owned())
                })?;
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

    /// Fixed-key compatibility and soundness checks for the Eq proof/fold wire.
    mod pasta_ipa_poseidon_wire {
        use std::panic::{AssertUnwindSafe, catch_unwind};

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

        use super::deferred_audit::{RecordingLoader, RecordingPoseidonTranscript};
        use super::{
            KAGEMUSHA_DEFERRED_EQUATION_TERM_COUNT_V1,
            KAGEMUSHA_LEAPFROG_PROOF_WINDOW_MAX_BYTES_V1,
            KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1, KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1,
            KagemushaLeapfrogProofWindowV1, KagemushaLeapfrogPublicInputsV1,
            KagemushaLeapfrogStepProofV1, KagemushaPastaCycleParityV1, PublicValue,
        };
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

        pub(super) struct Fixture {
            pub(super) params: ParamsIPA<EqAffine>,
            pub(super) verifying_key: halo2_base::halo2_proofs::plonk::VerifyingKey<EqAffine>,
            protocol: snark_verifier::verifier::plonk::PlonkProtocol<EqAffine>,
            deciding_key: IpaDecidingKey<EqAffine>,
            proof_without_folded_generator: Vec<u8>,
            pub(super) augmented_proof: Vec<u8>,
            pub(super) instances: Vec<Vec<Fp>>,
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

        pub(super) fn fixture() -> Fixture {
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
                verifying_key: vk,
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

        /// Measure the direct recursion tuple which keeps the proof scalar
        /// field native (`Eq/Fp`), while emulating only Eq's Fq coordinates.
        ///
        /// This is intentionally explicit and ignored: it constructs the full
        /// fixed-VK PLONK/IPA succinct verifier and can consume substantial
        /// memory. Release engineering runs it when changing the recursion
        /// degree or column budget; ordinary workspace tests retain the cheap
        /// type-tuple guard above.
        #[test]
        #[ignore = "explicit direct Eq/Fp recursive-verifier resource measurement"]
        fn direct_eq_fp_parent_verifier_cells_are_measured() {
            use std::{mem, rc::Rc};

            use halo2_base::gates::circuit::{BaseCircuitParams, builder::BaseCircuitBuilder};
            use halo2_ecc::{ecc::BaseFieldEccChip, fields::fp::FpChip};
            use halo2_proofs::halo2curves::pasta::Fq;
            use snark_verifier::loader::halo2::Halo2Loader;

            const OUTER_K: usize = 12;
            const LIMB_BITS: usize = 86;
            const LIMBS: usize = 3;
            type DirectChip<'chip> = BaseFieldEccChip<'chip, EqAffine>;
            type DirectLoader<'chip> = Halo2Loader<EqAffine, DirectChip<'chip>>;

            let fixture = fixture();
            let seed = BaseCircuitParams {
                k: OUTER_K,
                num_advice_per_phase: vec![1],
                num_lookup_advice_per_phase: vec![1],
                num_fixed: 1,
                lookup_bits: Some(OUTER_K - 1),
                num_instance_columns: 1,
            };
            let mut builder = BaseCircuitBuilder::<Fp>::new(false).use_params(seed);
            let range = builder.range_chip();
            let base = FpChip::<Fp, Fq>::new(&range, LIMB_BITS, LIMBS);
            let loader = DirectLoader::new(DirectChip::new(&base), mem::take(builder.pool(0)));
            let loaded_protocol = fixture.protocol.loaded(&loader);
            let loaded_instances = fixture
                .instances
                .iter()
                .map(|column| {
                    column
                        .iter()
                        .map(|value| loader.assign_scalar(*value))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut transcript = Transcript::<Rc<DirectLoader<'_>>, _>::new::<SECURE_MDS>(
                &loader,
                fixture.augmented_proof.as_slice(),
            );
            let parsed = SuccinctVerifier::read_proof(
                fixture.deciding_key.as_ref(),
                &loaded_protocol,
                &loaded_instances,
                &mut transcript,
            )
            .expect("parse the direct Eq/Fp parent proof");
            let accumulators = SuccinctVerifier::verify(
                fixture.deciding_key.as_ref(),
                &loaded_protocol,
                &loaded_instances,
                &parsed,
            )
            .expect("constrain the direct Eq/Fp PLONK/IPA residual");
            assert_eq!(accumulators.len(), 1);
            *builder.pool(0) = loader.take_ctx();

            let statistics = builder.statistics();
            eprintln!(
                "Kagemusha direct Eq/Fp verifier: advice={:?} lookup={:?} fixed={}",
                statistics.gate.total_advice_per_phase,
                statistics.total_lookup_advice_per_phase,
                statistics.gate.total_fixed,
            );
            let calculated = builder.calculate_params(Some(9));
            eprintln!("Kagemusha direct Eq/Fp verifier columns: {calculated:?}");
            assert_eq!(calculated.k, OUTER_K);
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
        fn fixed_eq_scalar_half_derives_the_real_deferred_residual_in_circuit() {
            use std::mem;

            use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
            use halo2_ecc::fields::fp::FpChip;
            use halo2_proofs::{dev::MockProver, halo2curves::group::Group as _};
            use snark_verifier::loader::halo2::Halo2Loader;

            use crate::zk::kagemusha_cycle_loader::{
                DeferredScalarEccChip, LIMB_BITS, LIMBS,
            };

            const OUTER_K: usize = 16;
            let fixture = fixture();
            let mut builder = BaseCircuitBuilder::<Fp>::new(false)
                .use_k(OUTER_K)
                .use_lookup_bits(OUTER_K - 1);
            let range = builder.range_chip();
            let coordinate = FpChip::<Fp, Fq>::new(&range, LIMB_BITS, LIMBS);
            let scalar_integer = FpChip::<Fp, Fp>::new(&range, LIMB_BITS, LIMBS);
            let chip = DeferredScalarEccChip::<EqAffine>::new(&coordinate, &scalar_integer);
            let loader = Halo2Loader::new(chip, mem::take(builder.pool(0)));
            let loaded_protocol = fixture.protocol.loaded(&loader);
            let loaded_instances = fixture
                .instances
                .iter()
                .map(|column| {
                    column
                        .iter()
                        .map(|value| loader.assign_scalar(*value))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut transcript = Transcript::<_, _>::new::<SECURE_MDS>(
                &loader,
                fixture.augmented_proof.as_slice(),
            );
            let parsed = SuccinctVerifier::read_proof(
                fixture.deciding_key.as_ref(),
                &loaded_protocol,
                &loaded_instances,
                &mut transcript,
            )
            .expect("parse fixed Eq proof in the native-scalar half");
            let accumulators = SuccinctVerifier::verify(
                fixture.deciding_key.as_ref(),
                &loaded_protocol,
                &loaded_instances,
                &parsed,
            )
            .expect("constrain fixed Eq transcript and residual coefficients");
            assert_eq!(accumulators.len(), 1);
            let audit = loader.ecc_chip().audit();
            assert_eq!(audit.equations.len(), 1);
            assert!(!audit.sources.is_empty());
            assert!(!audit.equations[0].terms.is_empty());
            assert!(
                audit.equations[0]
                    .terms
                    .windows(2)
                    .all(|pair| pair[0].source_index < pair[1].source_index),
                "the deferred residual must have one deterministic coefficient per source"
            );
            let witness = audit.witness();
            let residual = witness.equations[0]
                .iter()
                .fold(Eq::identity(), |sum, (source_index, coefficient)| {
                    sum + witness.sources[*source_index] * *coefficient
                });
            assert!(
                bool::from(residual.is_identity()),
                "the point-half witness must be the exact valid residual"
            );

            *builder.pool(0) = loader.take_ctx();
            let params = builder.calculate_params(Some(9));
            MockProver::run(params.k as u32, &builder, vec![])
                .expect("native-scalar deferred verifier mock prover")
                .assert_satisfied();
        }

        #[test]
        fn transition_proof_omits_recomputable_deferred_material_from_the_wire() {
            use crate::zk::kagemusha_v2::{
                KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS,
                KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_CELLS,
                KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_COLUMNS,
                KagemushaRecursiveSpendTransitionCircuitV2,
                kagemusha_recursive_spend_transition_instance_columns_v2,
            };

            const PRODUCTION_K: u32 = 12;
            let params = params_new(PRODUCTION_K);
            let circuit = KagemushaRecursiveSpendTransitionCircuitV2::default();
            let instance_columns =
                kagemusha_recursive_spend_transition_instance_columns_v2(&circuit.values);
            assert_eq!(
                instance_columns.len(),
                KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_COLUMNS,
                "the streaming transition uses exactly three instance columns"
            );
            assert!(instance_columns.iter().all(|column| {
                column.len() == instance_columns[0].len()
                    && column.len() > KAGEMUSHA_RECURSIVE_SPEND_V2_INSTANCE_ROWS
            }));
            assert_eq!(
                instance_columns.iter().map(Vec::len).sum::<usize>(),
                KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_CELLS
            );
            let vk = keygen_vk(&params, &circuit).expect("transition deferred-packet VK");
            let pk =
                keygen_pk(&params, vk.clone(), &circuit).expect("transition deferred-packet PK");
            let columns = instance_columns
                .iter()
                .map(Vec::as_slice)
                .collect::<Vec<_>>();
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
                Config::ipa().with_num_instance(instance_columns.iter().map(Vec::len).collect()),
            );
            assert!(
                protocol
                    .evaluations
                    .iter()
                    .chain(&protocol.queries)
                    .all(|query| query.rotation.0 == 0),
                "the production transition protocol must never reintroduce long rotations"
            );
            assert_eq!(
                protocol.num_instance.len(),
                KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_COLUMNS
            );
            assert_eq!(
                protocol.num_witness.iter().sum::<usize>(),
                KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_COLUMNS
            );
            let instances = instance_columns;
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
            assert_eq!(
                coefficient_count, KAGEMUSHA_DEFERRED_EQUATION_TERM_COUNT_V1,
                "the authenticated fixed verifier residual width changed"
            );
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
                "Kagemusha compact proof={} trace_rows={} scalars={} points={} explicit_challenges={} preprocessed={} residual_equations={} residual_coefficients={} point_sources={} derived_not_transported={}",
                proof_bytes.len(),
                instances[0].len(),
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
                proof_bytes.len() <= KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1,
                "the measured fixed step proof must fit its exact wire slot"
            );

            let predecessor_bytes = proof_bytes.clone();
            let mut newest_bytes = proof_bytes;
            newest_bytes[0] ^= 1;
            let window = KagemushaLeapfrogProofWindowV1 {
                version: KAGEMUSHA_LEAPFROG_PROOF_WINDOW_VERSION_V1,
                newest: KagemushaLeapfrogStepProofV1 {
                    parity: KagemushaPastaCycleParityV1::StepEp,
                    proof_step_count: 2,
                    public_inputs: KagemushaLeapfrogPublicInputsV1 {
                        public_statement_digest: [21, 22, 23, 24],
                        previous_state_digest: [11, 12, 13, 14],
                        result_state_digest: [31, 32, 33, 34],
                        manifest_sha256: [41, 42, 43, 44],
                    },
                    proof_bytes: newest_bytes,
                },
                predecessor: Some(KagemushaLeapfrogStepProofV1 {
                    parity: KagemushaPastaCycleParityV1::StepEq,
                    proof_step_count: 1,
                    public_inputs: KagemushaLeapfrogPublicInputsV1 {
                        public_statement_digest: [1, 2, 3, 4],
                        previous_state_digest: [0; 4],
                        result_state_digest: [11, 12, 13, 14],
                        manifest_sha256: [41, 42, 43, 44],
                    },
                    proof_bytes: predecessor_bytes,
                }),
            };
            window.validate().expect("bounded two-proof window");
            assert!(
                norito::to_bytes(&window)
                    .expect("encode compact proof window")
                    .len()
                    <= KAGEMUSHA_LEAPFROG_PROOF_WINDOW_MAX_BYTES_V1,
                "the newest/predecessor proof window must fit its reserved archive budget"
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
            plonk::{Circuit, ProvingKey, create_proof, keygen_pk, keygen_vk, verify_proof},
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

        use super::{
            KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1, PublicValue,
            terminal_verify_step_ep_instances,
        };

        const T: usize = 3;
        const RATE: usize = 2;
        const R_F: usize = 8;
        const R_P: usize = 57;
        const SECURE_MDS: usize = 0;
        const INNER_K: u32 = 5;

        type As = IpaAs<EpAffine, Bgh19>;
        type SuccinctVerifier = PlonkSuccinctVerifier<As>;
        type Transcript<L, S> = PoseidonTranscript<EpAffine, L, S, T, RATE, R_F, R_P>;

        pub(super) struct Fixture {
            pub(super) params: ParamsIPA<EpAffine>,
            pub(super) verifying_key: halo2_base::halo2_proofs::plonk::VerifyingKey<EpAffine>,
            protocol: snark_verifier::verifier::plonk::PlonkProtocol<EpAffine>,
            deciding_key: IpaDecidingKey<EpAffine>,
            proof_without_folded_generator: Vec<u8>,
            pub(super) augmented_proof: Vec<u8>,
            pub(super) instances: Vec<Vec<Fq>>,
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

        fn create_poseidon_proof<CircuitT>(
            params: &ParamsIPA<EpAffine>,
            pk: &ProvingKey<EpAffine>,
            circuit: CircuitT,
            instances: &[&[&[Fq]]],
        ) -> Vec<u8>
        where
            CircuitT: Circuit<Fq>,
        {
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

        pub(super) fn fixture() -> Fixture {
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
                verifying_key: vk,
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
        fn fixed_ep_scalar_half_derives_the_real_deferred_residual_in_circuit() {
            use std::mem;

            use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
            use halo2_ecc::fields::fp::FpChip;
            use halo2_proofs::{
                dev::MockProver,
                halo2curves::{group::Group as _, pasta::Fp},
            };
            use snark_verifier::loader::halo2::Halo2Loader;

            use crate::zk::kagemusha_cycle_loader::{
                DeferredScalarEccChip, LIMB_BITS, LIMBS,
            };

            const OUTER_K: usize = 16;
            let fixture = fixture();
            let mut builder = BaseCircuitBuilder::<Fq>::new(false)
                .use_k(OUTER_K)
                .use_lookup_bits(OUTER_K - 1);
            let range = builder.range_chip();
            let coordinate = FpChip::<Fq, Fp>::new(&range, LIMB_BITS, LIMBS);
            let scalar_integer = FpChip::<Fq, Fq>::new(&range, LIMB_BITS, LIMBS);
            let chip = DeferredScalarEccChip::<EpAffine>::new(&coordinate, &scalar_integer);
            let loader = Halo2Loader::new(chip, mem::take(builder.pool(0)));
            let loaded_protocol = fixture.protocol.loaded(&loader);
            let loaded_instances = fixture
                .instances
                .iter()
                .map(|column| {
                    column
                        .iter()
                        .map(|value| loader.assign_scalar(*value))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut transcript = Transcript::<_, _>::new::<SECURE_MDS>(
                &loader,
                fixture.augmented_proof.as_slice(),
            );
            let parsed = SuccinctVerifier::read_proof(
                fixture.deciding_key.as_ref(),
                &loaded_protocol,
                &loaded_instances,
                &mut transcript,
            )
            .expect("parse fixed Ep proof in the native-scalar half");
            let accumulators = SuccinctVerifier::verify(
                fixture.deciding_key.as_ref(),
                &loaded_protocol,
                &loaded_instances,
                &parsed,
            )
            .expect("constrain fixed Ep transcript and residual coefficients");
            assert_eq!(accumulators.len(), 1);
            let audit = loader.ecc_chip().audit();
            assert_eq!(audit.equations.len(), 1);
            assert!(!audit.sources.is_empty());
            assert!(!audit.equations[0].terms.is_empty());
            assert!(
                audit.equations[0]
                    .terms
                    .windows(2)
                    .all(|pair| pair[0].source_index < pair[1].source_index)
            );
            let witness = audit.witness();
            let residual = witness.equations[0]
                .iter()
                .fold(Ep::identity(), |sum, (source_index, coefficient)| {
                    sum + witness.sources[*source_index] * *coefficient
                });
            assert!(bool::from(residual.is_identity()));

            *builder.pool(0) = loader.take_ctx();
            let params = builder.calculate_params(Some(9));
            MockProver::run(params.k as u32, &builder, vec![])
                .expect("reciprocal native-scalar deferred verifier mock prover")
                .assert_satisfied();
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

        #[test]
        fn reciprocal_transition_proof_fits_the_fixed_wire_slot_without_long_rotations() {
            use crate::zk::kagemusha_v2::{
                KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_CELLS,
                KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_COLUMNS,
                KagemushaRecursiveSpendTransitionCircuitV2,
                kagemusha_recursive_spend_transition_instance_columns_v2,
            };

            const PRODUCTION_K: u32 = 12;
            let params = ParamsIPA::<EpAffine>::new(PRODUCTION_K);
            let circuit = KagemushaRecursiveSpendTransitionCircuitV2::<Fq>::default();
            let instance_columns =
                kagemusha_recursive_spend_transition_instance_columns_v2(&circuit.values);
            assert_eq!(
                instance_columns.len(),
                KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_COLUMNS
            );
            assert_eq!(
                instance_columns.iter().map(Vec::len).sum::<usize>(),
                KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_CELLS
            );
            let vk = keygen_vk(&params, &circuit).expect("reciprocal transition VK");
            let pk = keygen_pk(&params, vk.clone(), &circuit).expect("reciprocal transition PK");
            let columns = instance_columns
                .iter()
                .map(Vec::as_slice)
                .collect::<Vec<_>>();
            let proof_instances: [&[&[Fq]]; 1] = [&columns];
            let proof_without_generator =
                create_poseidon_proof(&params, &pk, circuit, &proof_instances);
            let generator =
                folded_generator(&params, &vk, &proof_without_generator, &proof_instances);
            let mut proof = proof_without_generator;
            proof.extend_from_slice(generator.to_bytes().as_ref());
            eprintln!(
                "Kagemusha reciprocal compact proof={} trace_rows={}",
                proof.len(),
                instance_columns[0].len()
            );

            let protocol = compile(
                &params,
                &vk,
                Config::ipa().with_num_instance(instance_columns.iter().map(Vec::len).collect()),
            );
            assert!(
                protocol
                    .evaluations
                    .iter()
                    .chain(&protocol.queries)
                    .all(|query| query.rotation.0 == 0),
                "the reciprocal transition protocol must not use long rotations"
            );
            assert_eq!(
                protocol.num_instance.len(),
                KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_COLUMNS
            );
            assert_eq!(
                protocol.num_witness.iter().sum::<usize>(),
                KAGEMUSHA_RECURSIVE_SPEND_V2_TRANSITION_INSTANCE_COLUMNS
            );
            assert!(
                proof.len() <= KAGEMUSHA_LEAPFROG_STEP_PROOF_MAX_BYTES_V1,
                "the reciprocal transition proof must fit its fixed wire slot"
            );
            terminal_verify_step_ep_instances(&params, &vk, &proof, &instance_columns)
                .expect("the reciprocal transition proof must terminally decide");
        }
    }

    #[test]
    fn terminal_pair_decider_rejects_order_cross_pair_and_trailing_substitution() {
        let eq = pasta_ipa_poseidon_wire::fixture();
        let ep = pasta_ipa_poseidon_wire_ep::fixture();

        terminal_verify_step_eq_instances(
            &eq.params,
            &eq.verifying_key,
            &eq.augmented_proof,
            &eq.instances,
        )
        .expect("canonical Eq proof must terminally decide");
        terminal_verify_step_ep_instances(
            &ep.params,
            &ep.verifying_key,
            &ep.augmented_proof,
            &ep.instances,
        )
        .expect("canonical Ep proof must terminally decide");

        assert!(
            terminal_verify_step_eq_instances(
                &eq.params,
                &eq.verifying_key,
                &ep.augmented_proof,
                &eq.instances,
            )
            .is_err(),
            "an Ep proof must not substitute for the ordered Eq half"
        );
        assert!(
            terminal_verify_step_ep_instances(
                &ep.params,
                &ep.verifying_key,
                &eq.augmented_proof,
                &ep.instances,
            )
            .is_err(),
            "an Eq proof must not substitute for the ordered Ep half"
        );

        let mut wrong_eq_instances = eq.instances.clone();
        wrong_eq_instances[0][0] += Fp::ONE;
        assert!(
            terminal_verify_step_eq_instances(
                &eq.params,
                &eq.verifying_key,
                &eq.augmented_proof,
                &wrong_eq_instances,
            )
            .is_err(),
            "one Eq proof cannot cross-pair with substituted public instances"
        );
        let mut wrong_ep_instances = ep.instances.clone();
        wrong_ep_instances[0][0] += Fq::ONE;
        assert!(
            terminal_verify_step_ep_instances(
                &ep.params,
                &ep.verifying_key,
                &ep.augmented_proof,
                &wrong_ep_instances,
            )
            .is_err(),
            "one Ep proof cannot cross-pair with substituted public instances"
        );

        let mut trailing_eq = eq.augmented_proof.clone();
        trailing_eq.push(0);
        assert!(
            terminal_verify_step_eq_instances(
                &eq.params,
                &eq.verifying_key,
                &trailing_eq,
                &eq.instances,
            )
            .is_err(),
            "Eq proof trailing bytes must reject"
        );
        let mut trailing_ep = ep.augmented_proof.clone();
        trailing_ep.push(0);
        assert!(
            terminal_verify_step_ep_instances(
                &ep.params,
                &ep.verifying_key,
                &trailing_ep,
                &ep.instances,
            )
            .is_err(),
            "Ep proof trailing bytes must reject"
        );
    }
}
