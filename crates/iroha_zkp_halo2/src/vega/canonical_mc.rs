//! Canonical Microsoft Vega-MC circuit adapter for the Figure 9 relation.
//!
//! The eight uniform step instances prove the two SHA-256 compressions of the
//! private birth-date item and the six compressions of the private issuer
//! `Sig_structure`. The core instance proves every remaining Figure 9
//! constraint over the same committed witness. A public one-hot step index
//! selects exactly one block and chaining transition inside a fixed step
//! shape; the proof wrapper must require the returned indices to be exactly
//! `0..8` in order.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex, MutexGuard},
};

use bellpepper::gadgets::{sha256::sha256_compression_function, uint32::UInt32};
use bellpepper_core::{
    ConstraintSystem, LinearCombination, SynthesisError,
    boolean::{AllocatedBit, Boolean},
    num::AllocatedNum,
};
use ff::{Field as _, PrimeField as _};
use once_cell::sync::Lazy;
use vega_prover::{
    provider::T256HyraxEngine,
    traits::{Engine, circuit::VegaCircuit, snark::DigestHelperTrait},
    vega_mc_zkp::{VegaMcProofDimensions, VegaMcProverKey, VegaMcVerifierKey, VegaMcZkSNARK},
};

use super::{
    MAX_VEGA_PROOF_BYTES_V1, VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1, VegaT256ScalarV1,
    engine::{
        VEGA_MDL_ACTION_INDEX_V1, VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1, VegaMdlProofContextV1,
        VegaMdlProofErrorV1, VegaMdlProverConfigV1, VegaRandomSourceV1,
    },
    figure9::{
        Figure9McMaterial, VEGA_MDL_FIGURE9_SHA256_STEPS_V1, VegaMdlFigure9WitnessV1,
        synthesize_figure9_mc_material,
    },
    figure9_layout::FIGURE9_LAYOUT,
    r1cs::{Shape, SparseMatrix},
    sponge::keccak256,
};

type McEngine = T256HyraxEngine;
type McScalar = <McEngine as Engine>::Scalar;

const SHA256_IV: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

const ENVELOPE_MAGIC: &[u8; 8] = b"IROVEGMC";
const ENVELOPE_VERSION: u8 = 1;
const ENVELOPE_HEADER_BYTES: usize = ENVELOPE_MAGIC.len() + 1 + 32;
const CONTEXT_DOMAIN: &[u8] = b"iroha.vega.figure9.microsoft-mc.context.v1";
const PINNED_SOURCE_COMMIT: &[u8] = b"c0ee259053cd12eaf43ed71b5cde375452b3ee4d";
const CONTEXT_PUBLIC_SCALARS: usize = 4;

static CANONICAL_KEYS: Lazy<Result<CanonicalKeys, VegaMdlProofErrorV1>> =
    Lazy::new(build_canonical_keys);
static LAST_PROVER_SEED_DIGEST: Mutex<Option<[u8; 32]>> = Mutex::new(None);

struct CanonicalKeys {
    pk: VegaMcProverKey<McEngine>,
    vk: VegaMcVerifierKey<McEngine>,
    prototype: Arc<Figure9McMaterial>,
}

#[derive(Clone, Copy)]
enum BitSource {
    Shared(usize),
    Constant(bool),
}

#[derive(Clone)]
struct CompressionSources {
    block_be: [BitSource; 512],
    state_before_le: [[BitSource; 32]; 8],
    state_after_le: [[BitSource; 32]; 8],
}

#[derive(Clone)]
pub(super) struct Figure9StepCircuit {
    material: Arc<Figure9McMaterial>,
    step_index: usize,
}

impl Figure9StepCircuit {
    pub(super) fn new(
        material: Arc<Figure9McMaterial>,
        step_index: usize,
    ) -> Result<Self, SynthesisError> {
        if step_index >= VEGA_MDL_FIGURE9_SHA256_STEPS_V1 {
            return Err(SynthesisError::Unsatisfiable);
        }
        Ok(Self {
            material,
            step_index,
        })
    }
}

#[derive(Clone)]
pub(super) struct Figure9CoreCircuit {
    material: Arc<Figure9McMaterial>,
    public_values: Vec<McScalar>,
}

impl Figure9CoreCircuit {
    pub(super) fn new(
        material: Arc<Figure9McMaterial>,
        context_public_values: &[McScalar],
    ) -> Result<Self, SynthesisError> {
        let mut public_values = material
            .assignment
            .public_inputs
            .iter()
            .copied()
            .map(to_mc_scalar)
            .collect::<Result<Vec<_>, _>>()?;
        public_values.extend_from_slice(context_public_values);
        Ok(Self {
            material,
            public_values,
        })
    }
}

impl VegaCircuit<McEngine> for Figure9StepCircuit {
    fn public_values(&self) -> Result<Vec<McScalar>, SynthesisError> {
        Ok(vec![McScalar::from(self.step_index as u64)])
    }

    fn shared<CS: ConstraintSystem<McScalar>>(
        &self,
        cs: &mut CS,
    ) -> Result<Vec<AllocatedNum<McScalar>>, SynthesisError> {
        allocate_shared(cs, &self.material)
    }

    fn precommitted<CS: ConstraintSystem<McScalar>>(
        &self,
        cs: &mut CS,
        _: &[AllocatedNum<McScalar>],
    ) -> Result<Vec<AllocatedNum<McScalar>>, SynthesisError> {
        allocate_public_values(cs, &self.public_values()?, "step public")
    }

    fn num_challenges(&self) -> usize {
        0
    }

    fn synthesize<CS: ConstraintSystem<McScalar>>(
        &self,
        cs: &mut CS,
        shared: &[AllocatedNum<McScalar>],
        precommitted: &[AllocatedNum<McScalar>],
        challenges: Option<&[McScalar]>,
    ) -> Result<(), SynthesisError> {
        if shared.len() != self.material.assignment.witness.len()
            || precommitted.len() != 1
            || challenges.is_some_and(|values| !values.is_empty())
        {
            return Err(SynthesisError::Unsatisfiable);
        }

        let selectors = allocate_one_hot_selector(
            &mut cs.namespace(|| "canonical compression selector"),
            self.step_index,
            &precommitted[0],
        )?;
        let sources = compression_sources(&self.material)?;

        let block_bits = (0..512)
            .map(|bit| {
                let candidates = core::array::from_fn(|step| sources[step].block_be[bit]);
                select_bit(
                    &mut cs.namespace(|| format!("selected block bit {bit}")),
                    shared,
                    &selectors,
                    candidates,
                    self.step_index,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let current_state = (0..8)
            .map(|word| {
                let bits = (0..32)
                    .map(|bit| {
                        let candidates =
                            core::array::from_fn(|step| sources[step].state_before_le[word][bit]);
                        select_bit(
                            &mut cs.namespace(|| format!("selected state {word} bit {bit}")),
                            shared,
                            &selectors,
                            candidates,
                            self.step_index,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(UInt32::from_bits(&bits))
            })
            .collect::<Result<Vec<_>, SynthesisError>>()?;

        let expected_state = (0..8)
            .map(|word| {
                (0..32)
                    .map(|bit| {
                        let candidates =
                            core::array::from_fn(|step| sources[step].state_after_le[word][bit]);
                        select_bit(
                            &mut cs
                                .namespace(|| format!("selected expected state {word} bit {bit}")),
                            shared,
                            &selectors,
                            candidates,
                            self.step_index,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, SynthesisError>>()?;

        let actual_state = sha256_compression_function(
            cs.namespace(|| "canonical SHA-256 compression"),
            &block_bits,
            &current_state,
        )?;
        for (word, (actual, expected)) in actual_state.into_iter().zip(expected_state).enumerate() {
            for (bit, (actual, expected)) in
                actual.into_bits().into_iter().zip(expected).enumerate()
            {
                Boolean::enforce_equal(
                    cs.namespace(|| format!("bind next state {word} bit {bit}")),
                    &actual,
                    &expected,
                )?;
            }
        }
        Ok(())
    }
}

impl VegaCircuit<McEngine> for Figure9CoreCircuit {
    fn public_values(&self) -> Result<Vec<McScalar>, SynthesisError> {
        Ok(self.public_values.clone())
    }

    fn shared<CS: ConstraintSystem<McScalar>>(
        &self,
        cs: &mut CS,
    ) -> Result<Vec<AllocatedNum<McScalar>>, SynthesisError> {
        allocate_shared(cs, &self.material)
    }

    fn precommitted<CS: ConstraintSystem<McScalar>>(
        &self,
        cs: &mut CS,
        _: &[AllocatedNum<McScalar>],
    ) -> Result<Vec<AllocatedNum<McScalar>>, SynthesisError> {
        allocate_public_values(cs, &self.public_values, "core public")
    }

    fn num_challenges(&self) -> usize {
        0
    }

    fn synthesize<CS: ConstraintSystem<McScalar>>(
        &self,
        cs: &mut CS,
        shared: &[AllocatedNum<McScalar>],
        precommitted: &[AllocatedNum<McScalar>],
        challenges: Option<&[McScalar]>,
    ) -> Result<(), SynthesisError> {
        if shared.len() != self.material.assignment.witness.len()
            || precommitted.len() != self.public_values.len()
            || precommitted.len() < VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1
            || challenges.is_some_and(|values| !values.is_empty())
        {
            return Err(SynthesisError::Unsatisfiable);
        }
        let shape = &self.material.assignment.shape;
        for row in 0..shape.constraint_count() {
            if self
                .material
                .excluded_sha256_rows
                .iter()
                .any(|range| range.contains(&row))
                || (shape.a.row_is_empty(row)
                    && shape.b.row_is_empty(row)
                    && shape.c.row_is_empty(row))
            {
                continue;
            }
            let a = relation_lc::<CS>(&shape.a, row, shape, shared, precommitted)?;
            let b = relation_lc::<CS>(&shape.b, row, shape, shared, precommitted)?;
            let c = relation_lc::<CS>(&shape.c, row, shape, shared, precommitted)?;
            cs.enforce(|| format!("Figure 9 core row {row}"), |_| a, |_| b, |_| c);
        }
        Ok(())
    }
}

fn allocate_shared<CS: ConstraintSystem<McScalar>>(
    cs: &mut CS,
    material: &Figure9McMaterial,
) -> Result<Vec<AllocatedNum<McScalar>>, SynthesisError> {
    material
        .assignment
        .witness
        .iter()
        .copied()
        .enumerate()
        .map(|(index, value)| {
            let value = to_mc_scalar(value)?;
            AllocatedNum::alloc(
                cs.namespace(|| format!("shared Figure 9 witness {index}")),
                || Ok(value),
            )
        })
        .collect()
}

fn allocate_public_values<CS: ConstraintSystem<McScalar>>(
    cs: &mut CS,
    values: &[McScalar],
    label: &str,
) -> Result<Vec<AllocatedNum<McScalar>>, SynthesisError> {
    values
        .iter()
        .copied()
        .enumerate()
        .map(|(index, value)| {
            let allocated =
                AllocatedNum::alloc(cs.namespace(|| format!("{label} value {index}")), || {
                    Ok(value)
                })?;
            allocated.inputize(cs.namespace(|| format!("inputize {label} value {index}")))?;
            Ok(allocated)
        })
        .collect()
}

fn allocate_one_hot_selector<CS: ConstraintSystem<McScalar>>(
    cs: &mut CS,
    selected: usize,
    public_index: &AllocatedNum<McScalar>,
) -> Result<Vec<AllocatedBit>, SynthesisError> {
    let selectors = (0..VEGA_MDL_FIGURE9_SHA256_STEPS_V1)
        .map(|index| {
            AllocatedBit::alloc(
                cs.namespace(|| format!("step selector {index}")),
                Some(index == selected),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let one_hot = selectors.iter().fold(
        LinearCombination::<McScalar>::zero() - CS::one(),
        |lc, selector| lc + selector.get_variable(),
    );
    cs.enforce(
        || "selector is one-hot",
        |_| one_hot,
        |lc| lc + CS::one(),
        |lc| lc,
    );
    let packed = selectors.iter().enumerate().fold(
        LinearCombination::<McScalar>::zero() - public_index.get_variable(),
        |lc, (index, selector)| lc + (McScalar::from(index as u64), selector.get_variable()),
    );
    cs.enforce(
        || "selector equals public index",
        |_| packed,
        |lc| lc + CS::one(),
        |lc| lc,
    );
    Ok(selectors)
}

fn select_bit<CS: ConstraintSystem<McScalar>>(
    cs: &mut CS,
    shared: &[AllocatedNum<McScalar>],
    selectors: &[AllocatedBit],
    candidates: [BitSource; VEGA_MDL_FIGURE9_SHA256_STEPS_V1],
    selected: usize,
) -> Result<Boolean, SynthesisError> {
    let selected_value = bit_source_value(candidates[selected], shared)?;
    let output = AllocatedBit::alloc(cs.namespace(|| "selected bit"), selected_value)?;
    let mut selected_lc = LinearCombination::<McScalar>::zero();
    for (index, (selector, candidate)) in selectors.iter().zip(candidates).enumerate() {
        match candidate {
            BitSource::Constant(false) => {}
            BitSource::Constant(true) => {
                selected_lc = selected_lc + selector.get_variable();
            }
            BitSource::Shared(shared_index) => {
                let shared_value = shared
                    .get(shared_index)
                    .ok_or(SynthesisError::Unsatisfiable)?;
                let product_value =
                    selector
                        .get_value()
                        .zip(shared_value.get_value())
                        .map(
                            |(selector, value)| {
                                if selector { value } else { McScalar::ZERO }
                            },
                        );
                let product = AllocatedNum::alloc(
                    cs.namespace(|| format!("selector product {index}")),
                    || product_value.ok_or(SynthesisError::AssignmentMissing),
                )?;
                cs.enforce(
                    || format!("bind selector product {index}"),
                    |lc| lc + selector.get_variable(),
                    |lc| lc + shared_value.get_variable(),
                    |lc| lc + product.get_variable(),
                );
                selected_lc = selected_lc + product.get_variable();
            }
        }
    }
    selected_lc = selected_lc - output.get_variable();
    cs.enforce(
        || "bind selected bit",
        |_| selected_lc,
        |lc| lc + CS::one(),
        |lc| lc,
    );
    Ok(Boolean::from(output))
}

fn bit_source_value(
    source: BitSource,
    shared: &[AllocatedNum<McScalar>],
) -> Result<Option<bool>, SynthesisError> {
    match source {
        BitSource::Constant(value) => Ok(Some(value)),
        BitSource::Shared(index) => shared
            .get(index)
            .ok_or(SynthesisError::Unsatisfiable)?
            .get_value()
            .map(|value| {
                if value == McScalar::ZERO {
                    Ok(false)
                } else if value == McScalar::ONE {
                    Ok(true)
                } else {
                    Err(SynthesisError::Unsatisfiable)
                }
            })
            .transpose(),
    }
}

fn compression_sources(
    material: &Figure9McMaterial,
) -> Result<[CompressionSources; VEGA_MDL_FIGURE9_SHA256_STEPS_V1], SynthesisError> {
    let birth_blocks = padded_blocks(&material.birth_byte_bits_le)?;
    let issuer_blocks = padded_blocks(&material.issuer_byte_bits_le)?;
    if birth_blocks.len() != 2 || issuer_blocks.len() != 6 {
        return Err(SynthesisError::Unsatisfiable);
    }
    let mut sources = Vec::with_capacity(VEGA_MDL_FIGURE9_SHA256_STEPS_V1);
    append_hash_sources(
        &mut sources,
        &birth_blocks,
        &material.birth_states_after_blocks_le,
    )?;
    append_hash_sources(
        &mut sources,
        &issuer_blocks,
        &material.issuer_states_after_blocks_le,
    )?;
    sources
        .try_into()
        .map_err(|_| SynthesisError::Unsatisfiable)
}

fn append_hash_sources(
    output: &mut Vec<CompressionSources>,
    blocks: &[[BitSource; 512]],
    states_after: &[[[usize; 32]; 8]],
) -> Result<(), SynthesisError> {
    if blocks.len() != states_after.len() {
        return Err(SynthesisError::Unsatisfiable);
    }
    for (block_index, block) in blocks.iter().enumerate() {
        let state_before_le = if block_index == 0 {
            SHA256_IV
                .map(|word| core::array::from_fn(|bit| BitSource::Constant(word & (1 << bit) != 0)))
        } else {
            states_after[block_index - 1].map(|word| word.map(BitSource::Shared))
        };
        let state_after_le = states_after[block_index].map(|word| word.map(BitSource::Shared));
        output.push(CompressionSources {
            block_be: *block,
            state_before_le,
            state_after_le,
        });
    }
    Ok(())
}

fn padded_blocks(message: &[[usize; 8]]) -> Result<Vec<[BitSource; 512]>, SynthesisError> {
    let bit_length = u64::try_from(message.len())
        .ok()
        .and_then(|length| length.checked_mul(8))
        .ok_or(SynthesisError::Unsatisfiable)?;
    let padded_len = message
        .len()
        .checked_add(9)
        .and_then(|length| length.checked_add(63))
        .map(|length| length / 64 * 64)
        .ok_or(SynthesisError::Unsatisfiable)?;
    let mut bytes = message
        .iter()
        .copied()
        .map(|bits| bits.map(BitSource::Shared))
        .collect::<Vec<_>>();
    bytes.push(constant_byte(0x80));
    while bytes.len() + 8 < padded_len {
        bytes.push(constant_byte(0));
    }
    bytes.extend(bit_length.to_be_bytes().map(constant_byte));
    if bytes.len() != padded_len {
        return Err(SynthesisError::Unsatisfiable);
    }
    bytes
        .chunks_exact(64)
        .map(|block| {
            block
                .iter()
                .flat_map(|byte| byte.iter().rev().copied())
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| SynthesisError::Unsatisfiable)
        })
        .collect()
}

fn constant_byte(byte: u8) -> [BitSource; 8] {
    core::array::from_fn(|bit| BitSource::Constant(byte & (1 << bit) != 0))
}

fn relation_lc<CS: ConstraintSystem<McScalar>>(
    matrix: &SparseMatrix,
    row: usize,
    shape: &Shape,
    shared: &[AllocatedNum<McScalar>],
    public: &[AllocatedNum<McScalar>],
) -> Result<LinearCombination<McScalar>, SynthesisError> {
    let mut lc = LinearCombination::zero();
    for (column, coefficient) in matrix
        .row_entries(row)
        .ok_or(SynthesisError::Unsatisfiable)?
    {
        let variable = if column < shape.variable_count() {
            shared
                .get(column)
                .ok_or(SynthesisError::Unsatisfiable)?
                .get_variable()
        } else if column == shape.variable_count() {
            CS::one()
        } else {
            let public_index = column
                .checked_sub(shape.variable_count() + 1)
                .ok_or(SynthesisError::Unsatisfiable)?;
            public
                .get(public_index)
                .ok_or(SynthesisError::Unsatisfiable)?
                .get_variable()
        };
        lc = lc + (to_mc_scalar(coefficient)?, variable);
    }
    Ok(lc)
}

pub(super) fn to_mc_scalar(value: VegaT256ScalarV1) -> Result<McScalar, SynthesisError> {
    Option::<McScalar>::from(McScalar::from_repr(value.to_le_bytes().into()))
        .ok_or(SynthesisError::Unsatisfiable)
}

pub(super) fn prove_figure9_mc<R: VegaRandomSourceV1>(
    context: &VegaMdlProofContextV1<'_>,
    public_inputs: &[VegaT256ScalarV1; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    witness: &VegaMdlFigure9WitnessV1<'_>,
    config: VegaMdlProverConfigV1,
    random: &mut R,
) -> Result<Vec<u8>, VegaMdlProofErrorV1> {
    let context_digest = context_digest(context)?;
    let context_public = context_public_values(context_digest);
    let keys = canonical_keys()?;
    let verifier_digest = keys
        .vk
        .digest()
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    if context.verifier_digest != verifier_digest {
        return Err(VegaMdlProofErrorV1::InvalidContext);
    }

    let material = Arc::new(
        synthesize_figure9_mc_material(public_inputs, witness)
            .map_err(|_| VegaMdlProofErrorV1::UnsatisfiedWitness)?,
    );
    material
        .assignment
        .shape
        .validate_relaxed_assignment(
            &material.assignment.witness,
            VegaT256ScalarV1::one(),
            &material.assignment.public_inputs,
            &vec![VegaT256ScalarV1::zero(); material.assignment.shape.constraint_count()],
        )
        .map_err(|_| VegaMdlProofErrorV1::UnsatisfiedWitness)?;
    validate_material_shape(&material, &keys.prototype)?;

    let steps = (0..VEGA_MDL_FIGURE9_SHA256_STEPS_V1)
        .map(|index| Figure9StepCircuit::new(Arc::clone(&material), index))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    let core = Figure9CoreCircuit::new(Arc::clone(&material), &context_public)
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    let seed = take_prover_seed(random)?;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(config.worker_count())
        .thread_name(|index| format!("iroha-vega-mc-{index}"))
        .build()
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    let proof = vega_prover::iroha_rng::with_external_seed(seed, || {
        pool.install(|| {
            let prep = VegaMcZkSNARK::<McEngine>::prep_prove(&keys.pk, &steps, &core, false)?;
            VegaMcZkSNARK::<McEngine>::prove(&keys.pk, &steps, &core, prep, false)
                .map(|(proof, _)| proof)
        })
    })
    .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?
    .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    let proof_bytes = proof
        .encode_iroha_canonical()
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    let total_len = ENVELOPE_HEADER_BYTES.checked_add(proof_bytes.len()).ok_or(
        VegaMdlProofErrorV1::ProofTooLarge {
            actual: usize::MAX,
            max: MAX_VEGA_PROOF_BYTES_V1,
        },
    )?;
    if total_len > MAX_VEGA_PROOF_BYTES_V1 {
        return Err(VegaMdlProofErrorV1::ProofTooLarge {
            actual: total_len,
            max: MAX_VEGA_PROOF_BYTES_V1,
        });
    }
    let mut envelope = Vec::with_capacity(total_len);
    envelope.extend_from_slice(ENVELOPE_MAGIC);
    envelope.push(ENVELOPE_VERSION);
    envelope.extend_from_slice(&context_digest);
    envelope.extend_from_slice(&proof_bytes);
    Ok(envelope)
}

pub(super) fn verify_figure9_mc(
    context: &VegaMdlProofContextV1<'_>,
    public_inputs: &[VegaT256ScalarV1; VEGA_MDL_FIGURE9_PUBLIC_INPUTS_V1],
    envelope: &[u8],
) -> Result<(), VegaMdlProofErrorV1> {
    if envelope.len() > MAX_VEGA_PROOF_BYTES_V1 {
        return Err(VegaMdlProofErrorV1::ProofTooLarge {
            actual: envelope.len(),
            max: MAX_VEGA_PROOF_BYTES_V1,
        });
    }
    if envelope.len() < ENVELOPE_HEADER_BYTES
        || &envelope[..ENVELOPE_MAGIC.len()] != ENVELOPE_MAGIC
        || envelope[ENVELOPE_MAGIC.len()] != ENVELOPE_VERSION
    {
        return Err(VegaMdlProofErrorV1::InvalidProofEncoding);
    }
    let expected_context_digest = context_digest(context)?;
    if envelope[ENVELOPE_MAGIC.len() + 1..ENVELOPE_HEADER_BYTES] != expected_context_digest {
        return Err(VegaMdlProofErrorV1::VerificationFailed);
    }
    let keys = canonical_keys()?;
    if context.verifier_digest
        != keys
            .vk
            .digest()
            .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?
    {
        return Err(VegaMdlProofErrorV1::InvalidContext);
    }
    let proof_bytes = &envelope[ENVELOPE_HEADER_BYTES..];
    scan_proof_wire(proof_bytes, &keys.vk.proof_dimensions())?;
    let proof = VegaMcZkSNARK::<McEngine>::decode_iroha_canonical(proof_bytes, proof_bytes.len())
        .map_err(|_| VegaMdlProofErrorV1::InvalidProofEncoding)?;
    let canonical = proof
        .encode_iroha_canonical()
        .map_err(|_| VegaMdlProofErrorV1::InvalidProofEncoding)?;
    if canonical != proof_bytes {
        return Err(VegaMdlProofErrorV1::InvalidProofEncoding);
    }
    let (step_public, core_public) = catch_unwind(AssertUnwindSafe(|| {
        proof.verify(&keys.vk, VEGA_MDL_FIGURE9_SHA256_STEPS_V1)
    }))
    .map_err(|_| VegaMdlProofErrorV1::VerificationFailed)?
    .map_err(|_| VegaMdlProofErrorV1::VerificationFailed)?;
    if step_public.len() != VEGA_MDL_FIGURE9_SHA256_STEPS_V1
        || step_public
            .iter()
            .enumerate()
            .any(|(index, values)| values.as_slice() != [McScalar::from(index as u64)])
    {
        return Err(VegaMdlProofErrorV1::VerificationFailed);
    }
    let mut expected_core = public_inputs
        .iter()
        .copied()
        .map(to_mc_scalar)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| VegaMdlProofErrorV1::InvalidContext)?;
    expected_core.extend_from_slice(&context_public_values(expected_context_digest));
    if core_public != expected_core {
        return Err(VegaMdlProofErrorV1::VerificationFailed);
    }
    Ok(())
}

pub(super) fn verifier_digest() -> Result<[u8; 32], VegaMdlProofErrorV1> {
    canonical_keys()?
        .vk
        .digest()
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)
}

pub(super) fn proof_dimensions() -> Result<VegaMcProofDimensions, VegaMdlProofErrorV1> {
    Ok(canonical_keys()?.vk.proof_dimensions())
}

fn canonical_keys() -> Result<&'static CanonicalKeys, VegaMdlProofErrorV1> {
    CANONICAL_KEYS.as_ref().map_err(|error| *error)
}

fn build_canonical_keys() -> Result<CanonicalKeys, VegaMdlProofErrorV1> {
    let public_inputs = [
        VegaT256ScalarV1::from_u64(1),
        VegaT256ScalarV1::from_u64(1),
        VegaT256ScalarV1::zero(),
        VegaT256ScalarV1::zero(),
        VegaT256ScalarV1::zero(),
        VegaT256ScalarV1::zero(),
        VegaT256ScalarV1::zero(),
        VegaT256ScalarV1::zero(),
        VegaT256ScalarV1::zero(),
        VegaT256ScalarV1::zero(),
        VegaT256ScalarV1::from_u64(2026),
        VegaT256ScalarV1::from_u64(7),
        VegaT256ScalarV1::from_u64(26),
        VegaT256ScalarV1::from_u64(18),
    ];
    let one = [1_u8; 32];
    let witness = VegaMdlFigure9WitnessV1::new(
        &FIGURE9_LAYOUT.issuer_template,
        &FIGURE9_LAYOUT.birth_template,
        &one,
        &one,
        &one,
        &one,
    )
    .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    let prototype = Arc::new(
        synthesize_figure9_mc_material(&public_inputs, &witness)
            .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?,
    );
    let step = Figure9StepCircuit::new(Arc::clone(&prototype), 0)
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    let core = Figure9CoreCircuit::new(
        Arc::clone(&prototype),
        &[McScalar::ZERO; CONTEXT_PUBLIC_SCALARS],
    )
    .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    let (pk, vk) = catch_unwind(AssertUnwindSafe(|| {
        VegaMcZkSNARK::<McEngine>::setup(&step, &core, VEGA_MDL_FIGURE9_SHA256_STEPS_V1)
    }))
    .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?
    .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?;
    if vk
        .digest()
        .map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?
        != VEGA_MDL_CANONICAL_VERIFIER_DIGEST_V1
    {
        return Err(VegaMdlProofErrorV1::InvalidCompiledProfile);
    }
    Ok(CanonicalKeys { pk, vk, prototype })
}

fn validate_material_shape(
    actual: &Figure9McMaterial,
    prototype: &Figure9McMaterial,
) -> Result<(), VegaMdlProofErrorV1> {
    if actual.assignment.shape != prototype.assignment.shape
        || actual.issuer_byte_bits_le != prototype.issuer_byte_bits_le
        || actual.birth_byte_bits_le != prototype.birth_byte_bits_le
        || actual.issuer_states_after_blocks_le != prototype.issuer_states_after_blocks_le
        || actual.birth_states_after_blocks_le != prototype.birth_states_after_blocks_le
        || actual.excluded_sha256_rows != prototype.excluded_sha256_rows
    {
        return Err(VegaMdlProofErrorV1::InvalidCompiledProfile);
    }
    Ok(())
}

fn take_prover_seed<R: VegaRandomSourceV1>(
    random: &mut R,
) -> Result<[u8; 32], VegaMdlProofErrorV1> {
    let mut seed = [0_u8; 32];
    random.fill_bytes(&mut seed)?;
    if seed.iter().all(|byte| *byte == seed[0]) {
        return Err(VegaMdlProofErrorV1::DegenerateRandomness);
    }
    let digest = keccak256(&seed);
    let mut previous = lock(&LAST_PROVER_SEED_DIGEST);
    if previous.as_ref() == Some(&digest) {
        return Err(VegaMdlProofErrorV1::DegenerateRandomness);
    }
    *previous = Some(digest);
    Ok(seed)
}

fn context_digest(context: &VegaMdlProofContextV1<'_>) -> Result<[u8; 32], VegaMdlProofErrorV1> {
    if context.action_index != VEGA_MDL_ACTION_INDEX_V1
        || context.chain_id.is_empty()
        || context.chain_id.len() > 255
        || [
            context.genesis_hash,
            context.parameter_id,
            context.parameter_digest,
            context.verifier_digest,
            context.statement_schema_digest,
            context.engine_manifest_digest,
        ]
        .iter()
        .any(|digest| *digest == [0; 32])
    {
        return Err(VegaMdlProofErrorV1::InvalidContext);
    }
    let mut frame = Vec::with_capacity(320);
    push_context_field(&mut frame, CONTEXT_DOMAIN)?;
    push_context_field(&mut frame, PINNED_SOURCE_COMMIT)?;
    push_context_field(&mut frame, context.chain_id)?;
    push_context_field(&mut frame, &context.genesis_hash)?;
    push_context_field(&mut frame, &context.action_index.to_le_bytes())?;
    push_context_field(&mut frame, &context.parameter_id)?;
    push_context_field(&mut frame, &context.parameter_digest)?;
    push_context_field(&mut frame, &context.verifier_digest)?;
    push_context_field(&mut frame, &context.statement_schema_digest)?;
    push_context_field(&mut frame, &context.engine_manifest_digest)?;
    Ok(keccak256(&frame))
}

fn push_context_field(output: &mut Vec<u8>, field: &[u8]) -> Result<(), VegaMdlProofErrorV1> {
    output.extend_from_slice(
        &u64::try_from(field.len())
            .map_err(|_| VegaMdlProofErrorV1::InvalidContext)?
            .to_le_bytes(),
    );
    output.extend_from_slice(field);
    Ok(())
}

fn context_public_values(digest: [u8; 32]) -> [McScalar; CONTEXT_PUBLIC_SCALARS] {
    core::array::from_fn(|index| {
        McScalar::from(u64::from_le_bytes(
            digest[index * 8..(index + 1) * 8]
                .try_into()
                .expect("fixed context digest chunk"),
        ))
    })
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn scan_proof_wire(
    proof: &[u8],
    dimensions: &VegaMcProofDimensions,
) -> Result<(), VegaMdlProofErrorV1> {
    let mut cursor = ProofCursor::new(proof);
    cursor.option_commitment(
        (dimensions.shared_commitment_points != 0).then_some(dimensions.shared_commitment_points),
    )?;
    cursor.sequence_len(dimensions.num_steps)?;
    for _ in 0..dimensions.num_steps {
        scan_split_instance(
            &mut cursor,
            dimensions.step_precommitted_points,
            dimensions.step_rest_points,
            dimensions.step_public_values,
            dimensions.step_challenges,
        )?;
    }
    scan_split_instance(
        &mut cursor,
        dimensions.core_precommitted_points,
        dimensions.core_rest_points,
        dimensions.core_public_values,
        dimensions.core_challenges,
    )?;
    cursor.skip_points(2)?;
    cursor.scalar_sequence(dimensions.evaluation_response_scalars)?;
    cursor.skip_scalars(2)?;

    cursor.sequence_len(dimensions.verifier_round_commitment_points.len())?;
    for points in &dimensions.verifier_round_commitment_points {
        cursor.commitment(*points)?;
    }
    cursor.scalar_sequence(dimensions.verifier_public_values)?;
    cursor.sequence_len(dimensions.verifier_challenges_per_round.len())?;
    for challenges in &dimensions.verifier_challenges_per_round {
        cursor.scalar_sequence(*challenges)?;
    }
    cursor.commitment(dimensions.nova_cross_term_points)?;
    cursor.commitment(dimensions.random_witness_commitment_points)?;
    cursor.commitment(dimensions.random_error_commitment_points)?;
    cursor.scalar_sequence(dimensions.random_public_values)?;
    cursor.skip_scalars(1)?;

    cursor.sumcheck(
        dimensions.relaxed_outer_rounds,
        dimensions.relaxed_outer_coefficients,
    )?;
    cursor.skip_scalars(3)?;
    cursor.sumcheck(
        dimensions.relaxed_inner_rounds,
        dimensions.relaxed_inner_coefficients,
    )?;
    cursor.scalar_sequence(dimensions.relaxed_opening_scalars)?;
    cursor.skip_scalars(1)?;
    cursor.scalar_sequence(dimensions.relaxed_opening_scalars)?;
    cursor.skip_scalars(1)?;
    cursor.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::engine::VegaRandomSourceErrorV1;

    const PYTHON_VK: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/vega-prover/reference/fixtures/cubic/python_vk.bin"
    ));
    const PYTHON_PROOF: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/vega-prover/reference/fixtures/cubic/python_standalone_proof.bin"
    ));

    struct SeedSource {
        seed: [u8; 32],
        fail: bool,
    }

    impl VegaRandomSourceV1 for SeedSource {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), VegaRandomSourceErrorV1> {
            if self.fail {
                return Err(VegaRandomSourceErrorV1::Unavailable);
            }
            if destination.len() != self.seed.len() {
                return Err(VegaRandomSourceErrorV1::Unavailable);
            }
            destination.copy_from_slice(&self.seed);
            Ok(())
        }
    }

    #[test]
    fn independent_python_proof_passes_nonallocating_dimension_scan() {
        let vk = VegaMcVerifierKey::<McEngine>::decode_iroha_canonical(PYTHON_VK, PYTHON_VK.len())
            .expect("pinned Python verifier key");
        scan_proof_wire(PYTHON_PROOF, &vk.proof_dimensions())
            .expect("pinned Python proof has exact key-derived structure");
    }

    #[test]
    fn structural_scan_rejects_bombs_truncation_trailing_and_wrong_shape() {
        let vk = VegaMcVerifierKey::<McEngine>::decode_iroha_canonical(PYTHON_VK, PYTHON_VK.len())
            .unwrap();
        let dimensions = vk.proof_dimensions();

        let mut bomb = PYTHON_PROOF.to_vec();
        // The cubic fixture has no shared segment, so byte zero is `None` and
        // the following eight bytes are the step-instance sequence length.
        bomb[1..9].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            scan_proof_wire(&bomb, &dimensions),
            Err(VegaMdlProofErrorV1::InvalidProofEncoding)
        );

        for cut in [0, 1, 8, 64, PYTHON_PROOF.len() / 2, PYTHON_PROOF.len() - 1] {
            assert_eq!(
                scan_proof_wire(&PYTHON_PROOF[..cut], &dimensions),
                Err(VegaMdlProofErrorV1::InvalidProofEncoding)
            );
        }
        let mut trailing = PYTHON_PROOF.to_vec();
        trailing.push(0);
        assert_eq!(
            scan_proof_wire(&trailing, &dimensions),
            Err(VegaMdlProofErrorV1::InvalidProofEncoding)
        );

        let mut wrong_dimensions = dimensions.clone();
        wrong_dimensions.num_steps += 1;
        assert_eq!(
            scan_proof_wire(PYTHON_PROOF, &wrong_dimensions),
            Err(VegaMdlProofErrorV1::InvalidProofEncoding)
        );
        let mut duplicate_shared = PYTHON_PROOF.to_vec();
        duplicate_shared[0] = 1;
        assert_eq!(
            scan_proof_wire(&duplicate_shared, &dimensions),
            Err(VegaMdlProofErrorV1::InvalidProofEncoding)
        );
    }

    #[test]
    fn rng_boundary_rejects_failure_constant_seed_and_immediate_reuse() {
        let mut failed = SeedSource {
            seed: [0x42; 32],
            fail: true,
        };
        assert_eq!(
            take_prover_seed(&mut failed),
            Err(VegaMdlProofErrorV1::RandomSource(
                VegaRandomSourceErrorV1::Unavailable
            ))
        );

        let mut constant = SeedSource {
            seed: [0x42; 32],
            fail: false,
        };
        assert_eq!(
            take_prover_seed(&mut constant),
            Err(VegaMdlProofErrorV1::DegenerateRandomness)
        );

        let mut seed = core::array::from_fn(|index| index as u8);
        seed[0] = 0xa5;
        let mut first = SeedSource { seed, fail: false };
        assert_eq!(take_prover_seed(&mut first), Ok(seed));
        let mut repeated = SeedSource { seed, fail: false };
        assert_eq!(
            take_prover_seed(&mut repeated),
            Err(VegaMdlProofErrorV1::DegenerateRandomness)
        );
    }

    #[test]
    fn context_frame_rejects_nonzero_action_index_and_binds_every_field() {
        let baseline = VegaMdlProofContextV1 {
            chain_id: b"taira",
            genesis_hash: [1; 32],
            action_index: VEGA_MDL_ACTION_INDEX_V1,
            parameter_id: [2; 32],
            parameter_digest: [3; 32],
            verifier_digest: [4; 32],
            statement_schema_digest: [5; 32],
            engine_manifest_digest: [6; 32],
        };
        let digest = context_digest(&baseline).unwrap();
        let mut wrong_index = baseline;
        wrong_index.action_index = 1;
        assert_eq!(
            context_digest(&wrong_index),
            Err(VegaMdlProofErrorV1::InvalidContext)
        );

        let mut mutations = Vec::new();
        let mut changed = baseline;
        changed.genesis_hash[0] ^= 1;
        mutations.push(changed);
        let mut changed = baseline;
        changed.parameter_id[0] ^= 1;
        mutations.push(changed);
        let mut changed = baseline;
        changed.parameter_digest[0] ^= 1;
        mutations.push(changed);
        let mut changed = baseline;
        changed.verifier_digest[0] ^= 1;
        mutations.push(changed);
        let mut changed = baseline;
        changed.statement_schema_digest[0] ^= 1;
        mutations.push(changed);
        let mut changed = baseline;
        changed.engine_manifest_digest[0] ^= 1;
        mutations.push(changed);
        assert!(
            mutations
                .iter()
                .all(|changed| context_digest(changed).unwrap() != digest)
        );
    }

    #[test]
    fn fixed_figure9_message_widths_produce_two_and_six_sha_blocks() {
        let birth = vec![[0_usize; 8]; 92];
        let issuer = vec![[0_usize; 8]; 368];
        assert_eq!(padded_blocks(&birth).unwrap().len(), 2);
        assert_eq!(padded_blocks(&issuer).unwrap().len(), 6);
    }

    #[test]
    fn compiled_profile_manifest_matches_its_pinned_keccak_digest() {
        assert_eq!(
            keccak256(super::super::engine::VEGA_MDL_COMPILED_PROFILE_MANIFEST_V1),
            super::super::engine::VEGA_MDL_COMPILED_PROFILE_DIGEST_V1,
        );
    }

    #[test]
    #[ignore = "expensive canonical Figure 9 MC setup"]
    fn emit_canonical_figure9_mc_governance_values() {
        let dimensions = proof_dimensions().expect("canonical MC dimensions");
        let verifier_digest = verifier_digest().expect("canonical MC verifier digest");
        eprintln!("VEGA_MC_VERIFIER_DIGEST={}", hex::encode(verifier_digest));
        eprintln!("VEGA_MC_DIMENSIONS={dimensions:#?}");
    }
}

fn scan_split_instance(
    cursor: &mut ProofCursor<'_>,
    precommitted_points: usize,
    rest_points: usize,
    public_values: usize,
    challenges: usize,
) -> Result<(), VegaMdlProofErrorV1> {
    cursor.option_commitment(None)?;
    cursor.option_commitment((precommitted_points != 0).then_some(precommitted_points))?;
    cursor.commitment(rest_points)?;
    cursor.scalar_sequence(public_values)?;
    cursor.scalar_sequence(challenges)
}

struct ProofCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ProofCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn finish(self) -> Result<(), VegaMdlProofErrorV1> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(VegaMdlProofErrorV1::InvalidProofEncoding)
        }
    }

    fn option_commitment(&mut self, expected: Option<usize>) -> Result<(), VegaMdlProofErrorV1> {
        let tag = self.take(1)?[0];
        match (tag, expected) {
            (0, None) => Ok(()),
            (1, Some(points)) => self.commitment(points),
            _ => Err(VegaMdlProofErrorV1::InvalidProofEncoding),
        }
    }

    fn commitment(&mut self, points: usize) -> Result<(), VegaMdlProofErrorV1> {
        self.sequence_len(points)?;
        self.skip_points(points)
    }

    fn scalar_sequence(&mut self, scalars: usize) -> Result<(), VegaMdlProofErrorV1> {
        self.sequence_len(scalars)?;
        self.skip_scalars(scalars)
    }

    fn sumcheck(&mut self, rounds: usize, coefficients: usize) -> Result<(), VegaMdlProofErrorV1> {
        self.sequence_len(rounds)?;
        for _ in 0..rounds {
            self.scalar_sequence(coefficients)?;
        }
        Ok(())
    }

    fn sequence_len(&mut self, expected: usize) -> Result<(), VegaMdlProofErrorV1> {
        let encoded = u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| VegaMdlProofErrorV1::InvalidProofEncoding)?,
        );
        if encoded
            != u64::try_from(expected).map_err(|_| VegaMdlProofErrorV1::InvalidCompiledProfile)?
        {
            return Err(VegaMdlProofErrorV1::InvalidProofEncoding);
        }
        Ok(())
    }

    fn skip_points(&mut self, points: usize) -> Result<(), VegaMdlProofErrorV1> {
        self.skip_elements(points, 33)
    }

    fn skip_scalars(&mut self, scalars: usize) -> Result<(), VegaMdlProofErrorV1> {
        self.skip_elements(scalars, 32)
    }

    fn skip_elements(&mut self, count: usize, width: usize) -> Result<(), VegaMdlProofErrorV1> {
        let bytes = count
            .checked_mul(width)
            .ok_or(VegaMdlProofErrorV1::InvalidProofEncoding)?;
        self.take(bytes).map(|_| ())
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], VegaMdlProofErrorV1> {
        let end = self
            .offset
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(VegaMdlProofErrorV1::InvalidProofEncoding)?;
        let result = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(result)
    }
}
