//! One-block SHA-256 leaves for the fixed KAGEMUSHA mint proof DAG.
//!
//! The mint-authority relation has a finite, fixed certificate transcript, but putting every
//! compression into the monetary circuit makes its key and witness geometry depend on the sum of
//! that work.  This module isolates exactly one FIPS 180-4 compression into a small reusable
//! circuit.  A leaf is not monetary authority: its public statement binds a release, a caller-
//! constrained plan, an exact position, the predecessor state, the big-endian block words, and the
//! successor state.  A later claim fold must verify an ordered, complete plan before exposing the
//! terminal mint claim.
//!
//! One block per leaf is intentional.  It keeps the Table8 circuit at `k = 12`, does not widen the
//! five-lane monetary circuits, and places no limit on the number of leaves a plan can fold.
//!
//! Release-pinned leaf keys and the alternating claim fold constrain exact typed-plan
//! completeness before the monetary circuit accepts the terminal claim. A leaf alone never
//! carries monetary authority.

use ff::PrimeField;
use halo2_base::{
    AssignedValue,
    QuantumCell::{Constant, Existing},
    gates::{
        GateInstructions as _, RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
    virtual_region::copy_constraints::{CopyConstraintManager, SharedCopyConstraintManager},
};
use halo2_proofs::{
    circuit::{Layouter, V1, Value},
    plonk::{Circuit, ConstraintSystem, Error as PlonkError},
};
use sha2::{compress256, digest::generic_array::GenericArray};

use super::{DigestV1, KagemushaPastaParityV1};
use crate::zk::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, digest_limbs, from_u128},
    pasta_sha256::PastaSha256JobsV1,
    pasta_sha256_table8::{
        AssignedBits, AssignedBlockWord, BLOCK_BYTE_SIZE, BLOCK_SIZE, DIGEST_SIZE, IV,
        Sha256Instructions as _, TABLE8_COMPRESSION_ROWS_ESTIMATE_UNMEASURED, Table8Chip,
        Table8Config, canonical_padding_suffix,
    },
};

const MINIMUM_UNUSABLE_ROWS: usize = 9;
/// Domain exponent of the one-block helper.  This is an internal proof stage, not the transported
/// `k = 16` monetary proof.
pub(crate) const KAGEMUSHA_MINT_HASH_SHARD_K_V1: u32 = 12;
/// Exact public-cell count of one hash leaf in either Pasta parity.
pub(crate) const KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1: usize = 43;

/// Public-instance offsets shared by both one-block hash parities.
pub(crate) mod public_instance {
    pub(crate) const VERSION: usize = 0;
    pub(crate) const PARITY: usize = 1;
    pub(crate) const RELEASE_LO: usize = 2;
    pub(crate) const PLAN_LO: usize = 4;
    pub(crate) const STAGE: usize = 6;
    pub(crate) const JOB: usize = 7;
    pub(crate) const BLOCK: usize = 8;
    pub(crate) const JOB_BLOCKS: usize = 9;
    pub(crate) const INITIAL_STATE: usize = 10;
    pub(crate) const BLOCK_WORDS: usize = 18;
    pub(crate) const OUTPUT_STATE: usize = 34;
    pub(crate) const END: usize = 42;
    /// The final cell is an explicit terminal-block bit.
    pub(crate) const FINAL_BLOCK: usize = END;
}

/// Field-neutral public statement for one exact SHA-256 compression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaMintHashShardStatementV1 {
    pub(crate) parity: KagemushaPastaParityV1,
    pub(crate) release_id: DigestV1,
    /// Commitment produced by the typed mint-plan relation.  This leaf only binds it; it does not
    /// grant authority to a host-selected commitment.
    pub(crate) plan_binding: DigestV1,
    /// Globally ordered compression position in this plan.
    pub(crate) stage_index: u64,
    pub(crate) job_index: u32,
    pub(crate) block_index: u32,
    pub(crate) job_block_count: u32,
    pub(crate) initial_state: [u32; DIGEST_SIZE],
    pub(crate) block_words: [u32; BLOCK_SIZE],
    pub(crate) output_state: [u32; DIGEST_SIZE],
}

impl KagemushaMintHashShardStatementV1 {
    pub(super) fn validate_shape(&self) -> Result<(), String> {
        if self.release_id == [0; 32] || self.plan_binding == [0; 32] {
            return Err("mint hash shard release or plan binding is zero".to_owned());
        }
        if self.job_block_count == 0 || self.block_index >= self.job_block_count {
            return Err("mint hash shard block position is outside its job".to_owned());
        }
        if self.block_index == 0 && self.initial_state != IV {
            return Err(
                "mint hash shard first block does not start from the SHA-256 IV".to_owned(),
            );
        }
        if compress_block(self.initial_state, self.block_words) != self.output_state {
            return Err("mint hash shard output is not the stated SHA-256 compression".to_owned());
        }
        Ok(())
    }

    pub(super) fn is_final_block(&self) -> bool {
        self.block_index + 1 == self.job_block_count
    }

    fn public_instances<F: KagemushaPoseidonFieldV1>(&self) -> Result<Vec<F>, String> {
        self.validate_shape()?;
        let parity = match self.parity {
            KagemushaPastaParityV1::Eq => 0_u64,
            KagemushaPastaParityV1::Ep => 1_u64,
        };
        let mut instances = Vec::with_capacity(KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1);
        instances.push(F::ONE);
        instances.push(F::from(parity));
        instances.extend(digest_limbs::<F>(self.release_id));
        instances.extend(digest_limbs::<F>(self.plan_binding));
        instances.push(F::from(self.stage_index));
        instances.push(F::from(u64::from(self.job_index)));
        instances.push(F::from(u64::from(self.block_index)));
        instances.push(F::from(u64::from(self.job_block_count)));
        instances.extend(self.initial_state.map(|word| F::from(u64::from(word))));
        instances.extend(self.block_words.map(|word| F::from(u64::from(word))));
        instances.extend(self.output_state.map(|word| F::from(u64::from(word))));
        instances.push(F::from(u64::from(self.is_final_block())));
        if instances.len() != KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1 {
            return Err("mint hash shard public instance shape drifted".to_owned());
        }
        Ok(instances)
    }
}

/// An ephemeral exact plan derived from the actual queued SHA messages.
///
/// The plan retains raw messages only while producing private leaf witnesses.  It is deliberately
/// neither serializable nor public protocol state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaMintHashPlanV1 {
    release_id: DigestV1,
    parity: KagemushaPastaParityV1,
    plan_binding: DigestV1,
    messages: Vec<Vec<u8>>,
    leaves: Vec<KagemushaMintHashShardStatementV1>,
}

impl KagemushaMintHashPlanV1 {
    /// Derive leaves from the exact messages already queued by the typed monetary relation.
    pub(crate) fn from_sha_jobs<F: KagemushaPoseidonFieldV1>(
        release_id: DigestV1,
        parity: KagemushaPastaParityV1,
        plan_binding: DigestV1,
        jobs: &PastaSha256JobsV1<F>,
    ) -> Result<Self, String> {
        Self::from_messages(release_id, parity, plan_binding, jobs.canonical_messages()?)
    }

    /// Canonically pad every exact message and produce one ordered compression leaf per block.
    pub(crate) fn from_messages(
        release_id: DigestV1,
        parity: KagemushaPastaParityV1,
        plan_binding: DigestV1,
        messages: Vec<Vec<u8>>,
    ) -> Result<Self, String> {
        let plan = Self::from_messages_unchecked(release_id, parity, plan_binding, messages)?;
        plan.validate()?;
        Ok(plan)
    }

    /// Return the exact ordered one-block statements consumed by the claim fold.
    pub(crate) fn leaves(&self) -> &[KagemushaMintHashShardStatementV1] {
        &self.leaves
    }

    /// Re-derive the plan from its retained canonical messages and require byte-for-byte equality.
    pub(crate) fn validate(&self) -> Result<(), String> {
        let rebuilt = Self::from_messages_unchecked(
            self.release_id,
            self.parity,
            self.plan_binding,
            self.messages.clone(),
        )?;
        if rebuilt.leaves != self.leaves {
            return Err(
                "mint hash plan leaves are missing, reordered, duplicated, or altered".into(),
            );
        }
        Ok(())
    }

    fn from_messages_unchecked(
        release_id: DigestV1,
        parity: KagemushaPastaParityV1,
        plan_binding: DigestV1,
        messages: Vec<Vec<u8>>,
    ) -> Result<Self, String> {
        // Use the checked constructor's implementation without recursively validating the result.
        if release_id == [0; 32] || plan_binding == [0; 32] {
            return Err("mint hash plan release or typed-plan binding is zero".to_owned());
        }
        if messages.is_empty() {
            return Err("mint hash plan contains no jobs".to_owned());
        }
        let mut leaves = Vec::new();
        let mut stage_index = 0_u64;
        for (job_index, message) in messages.iter().enumerate() {
            let suffix = canonical_padding_suffix(message.len())
                .ok_or_else(|| format!("mint hash job {job_index} length is not encodable"))?;
            let padded_len = message
                .len()
                .checked_add(suffix.len())
                .ok_or_else(|| format!("mint hash job {job_index} padded length overflowed"))?;
            let mut padded = Vec::new();
            padded
                .try_reserve_exact(padded_len)
                .map_err(|_| format!("mint hash job {job_index} padded allocation failed"))?;
            padded.extend_from_slice(message);
            padded.extend_from_slice(&suffix);
            if padded.len() % BLOCK_BYTE_SIZE != 0 {
                return Err(format!(
                    "mint hash job {job_index} canonical padding did not end on a block boundary"
                ));
            }
            let job_block_count = u32::try_from(padded.len() / BLOCK_BYTE_SIZE)
                .map_err(|_| format!("mint hash job {job_index} block count exceeds u32"))?;
            let mut state = IV;
            for (block_index, block) in padded.chunks_exact(BLOCK_BYTE_SIZE).enumerate() {
                let block_words = bytes_to_block_words(block)?;
                let output_state = compress_block(state, block_words);
                leaves.push(KagemushaMintHashShardStatementV1 {
                    parity,
                    release_id,
                    plan_binding,
                    stage_index,
                    job_index: u32::try_from(job_index)
                        .map_err(|_| "mint hash plan job count exceeds u32".to_owned())?,
                    block_index: u32::try_from(block_index)
                        .map_err(|_| "mint hash job block index exceeds u32".to_owned())?,
                    job_block_count,
                    initial_state: state,
                    block_words,
                    output_state,
                });
                state = output_state;
                stage_index = stage_index
                    .checked_add(1)
                    .ok_or_else(|| "mint hash plan stage index overflowed u64".to_owned())?;
            }
        }
        Ok(Self {
            release_id,
            parity,
            plan_binding,
            messages,
            leaves,
        })
    }
}

#[derive(Clone, Debug)]
struct AssignedCompressionV1<F: KagemushaPoseidonFieldV1> {
    initial_halves: [[AssignedValue<F>; 2]; DIGEST_SIZE],
    block_words: [AssignedValue<F>; BLOCK_SIZE],
    output_words: [AssignedValue<F>; DIGEST_SIZE],
    initial_values: [u32; DIGEST_SIZE],
    block_values: [u32; BLOCK_SIZE],
    use_unknown: bool,
}

impl<F: KagemushaPoseidonFieldV1> AssignedCompressionV1<F> {
    fn unknown(&self) -> Self {
        let mut clone = self.clone();
        clone.use_unknown = true;
        clone
    }

    fn synthesize(
        &self,
        config: Table8Config,
        layouter: &mut impl Layouter<F>,
        copy_manager: &SharedCopyConstraintManager<F>,
    ) -> Result<(), PlonkError> {
        Table8Chip::<F>::load(config.clone(), layouter)?;
        let chip = Table8Chip::<F>::construct(config);
        let physical_cells = copy_manager.lock().unwrap();
        let mut initial_halves = Vec::with_capacity(DIGEST_SIZE);
        for word in 0..DIGEST_SIZE {
            let mut halves = Vec::with_capacity(2);
            for half in 0..2 {
                let value = ((self.initial_values[word] >> (16 * half)) & 0xffff) as u16;
                halves.push(assigned_bits_from_base::<16, F>(
                    self.initial_halves[word][half],
                    Value::known(u16_bits(value)),
                    &physical_cells,
                    self.use_unknown,
                )?);
            }
            initial_halves.push(halves.try_into().map_err(|_| PlonkError::Synthesis)?);
        }
        let initial_halves: [[AssignedBits<16, F>; 2]; DIGEST_SIZE] = initial_halves
            .try_into()
            .map_err(|_| PlonkError::Synthesis)?;
        let initial_state = Table8Chip::<F>::state_from_dense_halves(initial_halves);

        let block_words = self
            .block_words
            .iter()
            .copied()
            .zip(self.block_values)
            .map(|(assigned, value)| {
                assigned_bits_from_base::<32, F>(
                    assigned,
                    Value::known(crate::zk::pasta_sha256_table8::Bits::from(value)),
                    &physical_cells,
                    self.use_unknown,
                )
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| PlonkError::Synthesis)?;
        let output_state = chip.compress(layouter, &initial_state, block_words)?;
        let digest = chip.digest(layouter, &output_state)?;
        bind_digest_to_base(layouter, &digest, &self.output_words, &physical_cells)
    }
}

fn assigned_bits_from_base<const BITS: usize, F: PrimeField + Ord>(
    assigned: AssignedValue<F>,
    value: Value<crate::zk::pasta_sha256_table8::Bits<BITS>>,
    physical_cells: &CopyConstraintManager<F>,
    use_unknown: bool,
) -> Result<AssignedBits<BITS, F>, PlonkError> {
    let virtual_cell = assigned.cell.ok_or(PlonkError::Synthesis)?;
    let cell = *physical_cells
        .assigned_advices
        .get(&virtual_cell)
        .ok_or(PlonkError::Synthesis)?;
    Ok(AssignedBits::from_range_checked_cell(
        if use_unknown { Value::unknown() } else { value },
        cell,
    ))
}

fn bind_digest_to_base<F: PrimeField + Ord>(
    layouter: &mut impl Layouter<F>,
    digest: &[AssignedBlockWord<F>; DIGEST_SIZE],
    expected: &[AssignedValue<F>; DIGEST_SIZE],
    physical_cells: &CopyConstraintManager<F>,
) -> Result<(), PlonkError> {
    layouter.assign_region(
        || "bind KAGEMUSHA mint hash shard output",
        |mut region| {
            for (actual, expected) in digest.iter().zip(expected) {
                let virtual_cell = expected.cell.ok_or(PlonkError::Synthesis)?;
                let expected = *physical_cells
                    .assigned_advices
                    .get(&virtual_cell)
                    .ok_or(PlonkError::Synthesis)?;
                region.constrain_equal(actual.cell(), expected);
            }
            Ok(())
        },
    )
}

/// Base/Table8 configuration of one mint hash leaf.
#[derive(Clone, Debug)]
pub(crate) struct KagemushaMintHashShardConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    sha: Table8Config,
}

/// One fixed-size Eq/Fp or Ep/Fq mint hash leaf.
#[derive(Clone, Debug)]
pub(crate) struct KagemushaMintHashShardCircuitV1<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    compression: AssignedCompressionV1<F>,
}

impl<F: KagemushaPoseidonFieldV1> KagemushaMintHashShardCircuitV1<F> {
    pub(crate) fn build(statement: &KagemushaMintHashShardStatementV1) -> Result<Self, String> {
        statement.validate_shape()?;
        let expected_parity = if F::IS_EQ_PARITY {
            KagemushaPastaParityV1::Eq
        } else {
            KagemushaPastaParityV1::Ep
        };
        if statement.parity != expected_parity {
            return Err("mint hash shard statement uses the other Pasta parity".to_owned());
        }
        let mut builder = BaseCircuitBuilder::new(false)
            .use_k(KAGEMUSHA_MINT_HASH_SHARD_K_V1 as usize)
            .use_lookup_bits((KAGEMUSHA_MINT_HASH_SHARD_K_V1 - 1) as usize)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let gate = range.gate();
        let ctx = builder.main(0);

        let version = ctx.load_constant(F::ONE);
        let parity = ctx.load_constant(F::from(if F::IS_EQ_PARITY { 0 } else { 1 }));
        let release = digest_limbs::<F>(statement.release_id).map(|limb| {
            let assigned = ctx.load_witness(limb);
            range.range_check(ctx, assigned, 128);
            assigned
        });
        let plan = digest_limbs::<F>(statement.plan_binding).map(|limb| {
            let assigned = ctx.load_witness(limb);
            range.range_check(ctx, assigned, 128);
            assigned
        });
        for digest in [&release, &plan] {
            let sum = gate.add(ctx, Existing(digest[0]), Existing(digest[1]));
            let zero = gate.is_zero(ctx, sum);
            gate.assert_is_const(ctx, &zero, &F::ZERO);
        }
        let stage = assign_uint(ctx, &range, u128::from(statement.stage_index), 64);
        let job = assign_uint(ctx, &range, u128::from(statement.job_index), 32);
        let block = assign_uint(ctx, &range, u128::from(statement.block_index), 32);
        let job_blocks = assign_uint(ctx, &range, u128::from(statement.job_block_count), 32);
        let nonzero_job_blocks = gate.is_zero(ctx, job_blocks);
        gate.assert_is_const(ctx, &nonzero_job_blocks, &F::ZERO);
        let position_valid = range.is_less_than(ctx, block, job_blocks, 32);
        gate.assert_is_const(ctx, &position_valid, &F::ONE);
        let first = gate.is_zero(ctx, block);
        let last_block = gate.sub(ctx, Existing(job_blocks), Constant(F::ONE));
        let final_block = gate.is_equal(ctx, block, last_block);

        let initial_words = statement
            .initial_state
            .map(|word| assign_uint(ctx, &range, u128::from(word), 32));
        for (actual, expected) in initial_words.iter().zip(IV) {
            let difference = gate.sub(
                ctx,
                Existing(*actual),
                Constant(F::from(u64::from(expected))),
            );
            let selected = gate.mul(ctx, Existing(difference), Existing(first));
            gate.assert_is_const(ctx, &selected, &F::ZERO);
        }
        let initial_halves = std::array::from_fn(|word| {
            let bits = gate.num_to_bits(ctx, initial_words[word], 32);
            std::array::from_fn(|half| {
                gate.inner_product(
                    ctx,
                    bits[half * 16..half * 16 + 16].iter().copied(),
                    (0..16).map(|bit| Constant(F::from(1_u64 << bit))),
                )
            })
        });
        let block_words = statement
            .block_words
            .map(|word| assign_uint(ctx, &range, u128::from(word), 32));
        let output_words = statement
            .output_state
            .map(|word| assign_uint(ctx, &range, u128::from(word), 32));

        builder.assigned_instances = vec![
            [version, parity]
                .into_iter()
                .chain(release)
                .chain(plan)
                .chain([stage, job, block, job_blocks])
                .chain(initial_words)
                .chain(block_words)
                .chain(output_words)
                .chain([final_block])
                .collect(),
        ];
        if builder.assigned_instances[0].len() != KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1
        {
            return Err("mint hash shard assigned instance shape drifted".to_owned());
        }
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        let usable_rows = (1_usize << KAGEMUSHA_MINT_HASH_SHARD_K_V1) - MINIMUM_UNUSABLE_ROWS;
        if TABLE8_COMPRESSION_ROWS_ESTIMATE_UNMEASURED > usable_rows {
            return Err("one Table8 compression does not fit the mint hash shard".to_owned());
        }
        Ok(Self {
            builder,
            compression: AssignedCompressionV1 {
                initial_halves,
                block_words,
                output_words,
                initial_values: statement.initial_state,
                block_values: statement.block_words,
                use_unknown: false,
            },
        })
    }

    pub(crate) fn instances(
        statement: &KagemushaMintHashShardStatementV1,
    ) -> Result<Vec<F>, String> {
        statement.public_instances()
    }
}

impl<F: KagemushaPoseidonFieldV1> Circuit<F> for KagemushaMintHashShardCircuitV1<F> {
    type Config = KagemushaMintHashShardConfigV1<F>;
    type FloorPlanner = V1;
    type Params = BaseCircuitParams;

    fn params(&self) -> Self::Params {
        self.builder.config_params.clone()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            builder: self.builder.deep_clone().unknown(true),
            compression: self.compression.unknown(),
        }
    }

    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let usable_rows = (1_usize << params.k) - MINIMUM_UNUSABLE_ROWS;
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows(usable_rows);
        KagemushaMintHashShardConfigV1 {
            base,
            sha: Table8Chip::<F>::configure_lanes::<1>(meta)[0].clone(),
        }
    }

    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("KAGEMUSHA mint hash shard uses authenticated Base parameters")
    }

    fn synthesize_for_measurement(
        &self,
        config: Self::Config,
        layouter: impl Layouter<F>,
    ) -> Result<(), PlonkError> {
        let result = self.synthesize(config, layouter);
        self.builder.reset_synthesis_state();
        result
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), PlonkError> {
        <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
            &self.builder,
            config.base,
            layouter.namespace(|| "KAGEMUSHA mint hash shard Base"),
        )?;
        self.compression
            .synthesize(config.sha, &mut layouter, &self.builder.core().copy_manager)
    }
}

fn assign_uint<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    value: u128,
    bits: usize,
) -> AssignedValue<F> {
    let assigned = ctx.load_witness(from_u128(value));
    range.range_check(ctx, assigned, bits);
    assigned
}

fn bytes_to_block_words(block: &[u8]) -> Result<[u32; BLOCK_SIZE], String> {
    if block.len() != BLOCK_BYTE_SIZE {
        return Err("mint hash shard block is not 64 bytes".to_owned());
    }
    block
        .chunks_exact(4)
        .map(|word| u32::from_be_bytes(word.try_into().expect("four-byte SHA word")))
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "mint hash shard block word count drifted".to_owned())
}

fn compress_block(
    mut state: [u32; DIGEST_SIZE],
    block_words: [u32; BLOCK_SIZE],
) -> [u32; DIGEST_SIZE] {
    let mut bytes = [0_u8; BLOCK_BYTE_SIZE];
    for (index, word) in block_words.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    compress256(
        &mut state,
        core::slice::from_ref(GenericArray::from_slice(&bytes)),
    );
    state
}

fn u16_bits(value: u16) -> crate::zk::pasta_sha256_table8::Bits<16> {
    crate::zk::pasta_sha256_table8::Bits::from(value)
}

const _: () = {
    assert!(KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1 == public_instance::END + 1);
    assert!(TABLE8_COMPRESSION_ROWS_ESTIMATE_UNMEASURED < (1 << KAGEMUSHA_MINT_HASH_SHARD_K_V1));
};

#[cfg(test)]
mod tests {
    use super::*;
    use ff::Field as _;
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
    };
    use sha2::{Digest as _, Sha256};

    fn plan<F: KagemushaPoseidonFieldV1>() -> KagemushaMintHashPlanV1 {
        KagemushaMintHashPlanV1::from_messages(
            [0x11; 32],
            if F::IS_EQ_PARITY {
                KagemushaPastaParityV1::Eq
            } else {
                KagemushaPastaParityV1::Ep
            },
            [0x22; 32],
            vec![b"abc".to_vec(), vec![0x5a; 130]],
        )
        .expect("canonical test plan")
    }

    fn prove_leaf<F: KagemushaPoseidonFieldV1>(
        statement: &KagemushaMintHashShardStatementV1,
    ) -> MockProver<F> {
        let circuit =
            KagemushaMintHashShardCircuitV1::<F>::build(statement).expect("valid mint hash shard");
        let instances = KagemushaMintHashShardCircuitV1::<F>::instances(statement)
            .expect("valid public instances");
        MockProver::run(KAGEMUSHA_MINT_HASH_SHARD_K_V1, &circuit, vec![instances])
            .expect("mint hash shard synthesis")
    }

    fn assert_first_and_continuation<F: KagemushaPoseidonFieldV1>() {
        let plan = plan::<F>();
        assert_eq!(plan.leaves().len(), 4);
        prove_leaf::<F>(&plan.leaves()[0]).assert_satisfied();
        prove_leaf::<F>(&plan.leaves()[2]).assert_satisfied();
        assert_eq!(plan.leaves()[0].initial_state, IV);
        assert_eq!(plan.leaves()[1].initial_state, IV);
        assert_eq!(
            plan.leaves()[2].initial_state,
            plan.leaves()[1].output_state
        );

        let digest = plan.leaves()[0]
            .output_state
            .into_iter()
            .flat_map(u32::to_be_bytes)
            .collect::<Vec<_>>();
        assert_eq!(digest, Sha256::digest(b"abc").to_vec());
    }

    #[test]
    fn one_block_leaf_proves_first_and_continuation_in_both_parities() {
        assert_first_and_continuation::<Fp>();
        assert_first_and_continuation::<Fq>();
    }

    #[test]
    fn plan_rejects_reordering_duplication_and_missing_leaf() {
        let plan = plan::<Fp>();
        let mut reordered = plan.clone();
        reordered.leaves.swap(0, 1);
        assert!(reordered.validate().is_err());
        let mut duplicated = plan.clone();
        duplicated.leaves[1] = duplicated.leaves[0].clone();
        assert!(duplicated.validate().is_err());
        let mut missing = plan;
        missing.leaves.pop();
        assert!(missing.validate().is_err());
    }

    #[test]
    fn plan_uses_the_exact_typed_sha_queue_messages() {
        use crate::zk::pasta_sha256::PastaSha256ByteV1;

        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(KAGEMUSHA_MINT_HASH_SHARD_K_V1 as usize)
            .use_lookup_bits((KAGEMUSHA_MINT_HASH_SHARD_K_V1 - 1) as usize);
        let mut jobs = PastaSha256JobsV1::default();
        let message = b"typed mint transcript"
            .iter()
            .copied()
            .map(PastaSha256ByteV1::constant)
            .collect::<Vec<_>>();
        jobs.digest_constrained(builder.main(0), &message)
            .expect("queue exact typed message");
        let plan = KagemushaMintHashPlanV1::from_sha_jobs(
            [0x11; 32],
            KagemushaPastaParityV1::Eq,
            [0x22; 32],
            &jobs,
        )
        .expect("source-coupled hash plan");
        assert_eq!(plan.messages, vec![b"typed mint transcript".to_vec()]);
        assert_eq!(plan.leaves.len(), 1);
    }

    #[test]
    fn public_output_and_block_substitution_are_rejected() {
        let plan = plan::<Fp>();
        let statement = &plan.leaves()[0];
        let circuit = KagemushaMintHashShardCircuitV1::<Fp>::build(statement).unwrap();
        let mut wrong_output = KagemushaMintHashShardCircuitV1::<Fp>::instances(statement).unwrap();
        wrong_output[public_instance::OUTPUT_STATE] += Fp::ONE;
        assert!(
            MockProver::run(KAGEMUSHA_MINT_HASH_SHARD_K_V1, &circuit, vec![wrong_output])
                .unwrap()
                .verify()
                .is_err()
        );
        let mut wrong_block = KagemushaMintHashShardCircuitV1::<Fp>::instances(statement).unwrap();
        wrong_block[public_instance::BLOCK_WORDS] += Fp::ONE;
        assert!(
            MockProver::run(KAGEMUSHA_MINT_HASH_SHARD_K_V1, &circuit, vec![wrong_block])
                .unwrap()
                .verify()
                .is_err()
        );
    }

    #[test]
    fn table8_state_and_block_witnesses_are_copy_bound_to_base() {
        let plan = plan::<Fp>();
        let statement = &plan.leaves()[2];
        let instances = KagemushaMintHashShardCircuitV1::<Fp>::instances(statement).unwrap();

        let mut wrong_state = KagemushaMintHashShardCircuitV1::<Fp>::build(statement).unwrap();
        wrong_state.compression.initial_values[0] ^= 1;
        assert!(
            MockProver::run(
                KAGEMUSHA_MINT_HASH_SHARD_K_V1,
                &wrong_state,
                vec![instances.clone()],
            )
            .unwrap()
            .verify()
            .is_err()
        );

        let mut wrong_block = KagemushaMintHashShardCircuitV1::<Fp>::build(statement).unwrap();
        wrong_block.compression.block_values[0] ^= 1;
        assert!(
            MockProver::run(
                KAGEMUSHA_MINT_HASH_SHARD_K_V1,
                &wrong_block,
                vec![instances],
            )
            .unwrap()
            .verify()
            .is_err()
        );
    }

    #[test]
    fn wrong_parity_and_non_iv_first_block_fail_closed() {
        let plan = plan::<Fp>();
        let mut wrong_parity = plan.leaves()[0].clone();
        wrong_parity.parity = KagemushaPastaParityV1::Ep;
        assert!(KagemushaMintHashShardCircuitV1::<Fp>::build(&wrong_parity).is_err());
        let mut wrong_iv = plan.leaves()[0].clone();
        wrong_iv.initial_state[0] ^= 1;
        wrong_iv.output_state = compress_block(wrong_iv.initial_state, wrong_iv.block_words);
        assert!(wrong_iv.validate_shape().is_err());
    }
}
