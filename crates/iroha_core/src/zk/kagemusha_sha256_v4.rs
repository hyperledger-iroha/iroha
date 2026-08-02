//! Fixed-shape Table16 SHA-256 jobs for Kagemusha V4 Step circuits.
//!
//! The Base circuit records each hash relation while it is built. After Base
//! synthesis has established the virtual-to-physical cell map, five Table16
//! lanes realize those relations. Source bytes and digest words are
//! copy-constrained across the two layouts.

use ff::PrimeField;
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateChip, GateInstructions, RangeChip, RangeInstructions},
    halo2_proofs::{
        circuit::{Layouter, Value},
        plonk::{ConstraintSystem, Error},
    },
    utils::{BigPrimeField, ScalarField, fe_to_biguint},
    virtual_region::copy_constraints::SharedCopyConstraintManager,
};
use sha2::{Digest as _, Sha256};

use super::kagemusha_sha256_table16_v4::{
    AssignedByte, BLOCK_BYTE_SIZE, DIGEST_SIZE, PaddedByte, Sha256Instructions,
    TABLE16_SPREAD_TABLE_ROWS, Table16Chip, Table16Config, canonical_padding_suffix,
};

/// Independent Table16 lanes fixed by the V4 circuit identity.
pub(crate) const KAGEMUSHA_SHA256_LANES_V4: usize = 5;

// Table16 uses about 2,267 rows per compression block. Keep a small explicit
// margin for layout evolution, plus per-job IV and digest regions.
const SHA256_ROWS_PER_BLOCK_V4: usize = 2_304;
const SHA256_ROWS_PER_JOB_V4: usize = 64;
const SHA256_TABLE_ROWS_V4: usize = TABLE16_SPREAD_TABLE_ROWS;

/// One Boolean cell produced or checked by the SHA-byte provenance API.
///
/// The assigned cell is deliberately private: callers can only obtain this
/// token through [`KagemushaSha256BitV4::decompose`], so composing a
/// [`KagemushaSha256ByteV4`] from eight tokens carries a real Boolean proof.
#[derive(Clone, Copy, Debug)]
pub(super) struct KagemushaSha256BitV4<F: ScalarField> {
    assigned: AssignedValue<F>,
}

impl<F: BigPrimeField> KagemushaSha256BitV4<F> {
    /// Decompose one assigned integer into little-endian Boolean cells.
    pub(super) fn decompose(
        ctx: &mut Context<F>,
        gate: &GateChip<F>,
        value: AssignedValue<F>,
        bit_len: usize,
    ) -> Vec<Self> {
        gate.num_to_bits(ctx, value, bit_len)
            .into_iter()
            .map(|assigned| Self { assigned })
            .collect()
    }

    /// Require this bit to be the constant zero.
    pub(super) fn assert_zero(self, ctx: &mut Context<F>, gate: &GateChip<F>) {
        gate.assert_is_const(ctx, &self.assigned, &F::ZERO);
    }

    fn assigned(self) -> AssignedValue<F> {
        self.assigned
    }
}

#[derive(Clone, Copy, Debug)]
enum KagemushaSha256ByteSourceV4<F: ScalarField> {
    Constant(u8),
    Constrained(AssignedValue<F>),
}

/// A SHA-256 message byte carrying its circuit provenance.
///
/// Constants remain Table16 constants. Dynamic bytes can only be constructed
/// from eight proven Boolean cells or the checked fallback in
/// [`KagemushaSha256JobsV4::digest`]. Consequently
/// [`KagemushaSha256JobsV4::digest_constrained`] does not repeat a Range8
/// lookup for these values.
#[derive(Clone, Copy, Debug)]
pub(super) struct KagemushaSha256ByteV4<F: ScalarField> {
    source: KagemushaSha256ByteSourceV4<F>,
}

impl<F: BigPrimeField> KagemushaSha256ByteV4<F> {
    /// Construct a literal message byte.
    pub(super) const fn constant(byte: u8) -> Self {
        Self {
            source: KagemushaSha256ByteSourceV4::Constant(byte),
        }
    }

    /// Compose one little-endian byte from eight Boolean-proven cells.
    pub(super) fn from_bits_le(
        ctx: &mut Context<F>,
        gate: &GateChip<F>,
        bits: &[KagemushaSha256BitV4<F>],
    ) -> Self {
        assert_eq!(bits.len(), 8, "one SHA-256 byte has exactly eight bits");
        let assigned = gate.inner_product(
            ctx,
            bits.iter().copied().map(|bit| bit.assigned()),
            (0..8).map(|bit| QuantumCell::Constant(F::from(1_u64 << bit))),
        );
        Self {
            source: KagemushaSha256ByteSourceV4::Constrained(assigned),
        }
    }

    /// Decompose one constrained byte back into Boolean-proven cells.
    pub(super) fn decompose_bits_le(
        self,
        ctx: &mut Context<F>,
        gate: &GateChip<F>,
    ) -> [KagemushaSha256BitV4<F>; 8] {
        match self.source {
            KagemushaSha256ByteSourceV4::Constant(byte) => {
                std::array::from_fn(|bit| KagemushaSha256BitV4 {
                    assigned: ctx.load_constant(F::from(u64::from((byte >> bit) & 1))),
                })
            }
            KagemushaSha256ByteSourceV4::Constrained(assigned) => {
                KagemushaSha256BitV4::decompose(ctx, gate, assigned, 8)
                    .try_into()
                    .expect("eight-bit decomposition has fixed length")
            }
        }
    }

    /// Return the underlying assigned cell when this is a dynamic byte.
    pub(super) fn assigned(self) -> Option<AssignedValue<F>> {
        match self.source {
            KagemushaSha256ByteSourceV4::Constant(_) => None,
            KagemushaSha256ByteSourceV4::Constrained(assigned) => Some(assigned),
        }
    }

    /// Range-check one assigned source exactly once and retain that proof for
    /// the SHA queue.
    pub(super) fn range_checked(
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        assigned: AssignedValue<F>,
    ) -> Self {
        range.range_check(ctx, assigned, 8);
        Self {
            source: KagemushaSha256ByteSourceV4::Constrained(assigned),
        }
    }

    /// Return this proven byte as a linear-combination input.
    pub(super) fn quantum_cell(self) -> QuantumCell<F> {
        match self.source {
            KagemushaSha256ByteSourceV4::Constant(byte) => {
                QuantumCell::Constant(F::from(u64::from(byte)))
            }
            KagemushaSha256ByteSourceV4::Constrained(assigned) => QuantumCell::Existing(assigned),
        }
    }

    /// Read one valid typed byte in focused preimage-parity tests.
    #[cfg(test)]
    pub(super) fn test_value(self) -> u8 {
        self.value(0)
            .expect("typed SHA-256 test message contains canonical bytes")
    }

    fn value(self, index: usize) -> Result<u8, String> {
        match self.source {
            KagemushaSha256ByteSourceV4::Constant(byte) => Ok(byte),
            KagemushaSha256ByteSourceV4::Constrained(assigned) => {
                if assigned.cell.is_none() {
                    return Err(format!(
                        "Kagemusha SHA-256 message cell {index} has no virtual-cell identity"
                    ));
                }
                u8::try_from(fe_to_biguint(assigned.value())).map_err(|_| {
                    format!("Kagemusha SHA-256 message cell {index} is not a canonical byte")
                })
            }
        }
    }
}

#[derive(Clone, Debug)]
struct KagemushaSha256JobV4<F: ScalarField> {
    message: Vec<KagemushaSha256ByteV4<F>>,
    output_words: [AssignedValue<F>; DIGEST_SIZE],
}

/// Explicit, circuit-owned SHA jobs. There is deliberately no global or
/// thread-local queue: witness stripping clones this exact job shape.
#[derive(Clone, Debug)]
pub(crate) struct KagemushaSha256JobsV4<F: ScalarField> {
    jobs: Vec<KagemushaSha256JobV4<F>>,
    use_unknown: bool,
    #[cfg(test)]
    padding_xor: Option<(usize, u8)>,
    #[cfg(test)]
    source_xor: Option<(usize, usize, u8)>,
    #[cfg(test)]
    output_word_xor: Option<(usize, usize, u32)>,
    #[cfg(test)]
    swap_block_endian: Option<usize>,
    #[cfg(test)]
    break_chain_at_block: Option<usize>,
    #[cfg(test)]
    skip_iv_reset_at_job: Option<usize>,
}

impl<F: ScalarField> Default for KagemushaSha256JobsV4<F> {
    fn default() -> Self {
        Self {
            jobs: Vec::new(),
            use_unknown: false,
            #[cfg(test)]
            padding_xor: None,
            #[cfg(test)]
            source_xor: None,
            #[cfg(test)]
            output_word_xor: None,
            #[cfg(test)]
            swap_block_endian: None,
            #[cfg(test)]
            break_chain_at_block: None,
            #[cfg(test)]
            skip_iv_reset_at_job: None,
        }
    }
}

impl<F> KagemushaSha256JobsV4<F>
where
    F: BigPrimeField + PrimeField + From<u64>,
{
    /// Range-constrain every Base source byte, queue its exact SHA-256
    /// relation, and return eight Base placeholders for the digest words.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn digest(
        &mut self,
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        message: &[AssignedValue<F>],
    ) -> Result<[AssignedValue<F>; DIGEST_SIZE], String> {
        let constrained = message
            .iter()
            .copied()
            .map(|assigned| KagemushaSha256ByteV4::range_checked(ctx, range, assigned))
            .collect::<Vec<_>>();
        self.digest_constrained(ctx, &constrained)
    }

    /// Queue bytes whose constant or Boolean-decomposition provenance already
    /// proves they are canonical bytes.
    pub(super) fn digest_constrained(
        &mut self,
        ctx: &mut Context<F>,
        message: &[KagemushaSha256ByteV4<F>],
    ) -> Result<[AssignedValue<F>; DIGEST_SIZE], String> {
        let bytes = message
            .iter()
            .copied()
            .enumerate()
            .map(|(index, byte)| byte.value(index))
            .collect::<Result<Vec<_>, _>>()?;
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        #[cfg(test)]
        let job_index = self.jobs.len();
        let output_words = std::array::from_fn(|index| {
            let word = u32::from_be_bytes(
                digest[index * 4..index * 4 + 4]
                    .try_into()
                    .expect("fixed SHA-256 word"),
            );
            #[cfg(test)]
            let word = if let Some((target_job, target_word, xor)) = self.output_word_xor {
                if target_job == job_index && target_word == index {
                    word ^ xor
                } else {
                    word
                }
            } else {
                word
            };
            ctx.load_witness(F::from(u64::from(word)))
        });
        if output_words.iter().any(|word| word.cell.is_none()) {
            return Err("Kagemusha SHA-256 output has no virtual-cell identity".to_owned());
        }
        self.jobs.push(KagemushaSha256JobV4 {
            message: message.to_vec(),
            output_words,
        });
        Ok(output_words)
    }

    /// Preserve job count, lengths, and virtual cells while hiding all raw
    /// Table16 witnesses during key generation.
    pub(crate) fn unknown(&self) -> Self {
        let mut clone = self.clone();
        clone.use_unknown = true;
        clone
    }

    pub(crate) fn compression_blocks(&self) -> Result<usize, String> {
        self.jobs.iter().try_fold(0_usize, |total, job| {
            let suffix = canonical_padding_suffix(job.message.len())
                .ok_or_else(|| "Kagemusha SHA-256 message length is not encodable".to_owned())?;
            let blocks = job
                .message
                .len()
                .checked_add(suffix.len())
                .ok_or_else(|| "Kagemusha SHA-256 padded length overflow".to_owned())?
                / BLOCK_BYTE_SIZE;
            total
                .checked_add(blocks)
                .ok_or_else(|| "Kagemusha SHA-256 block count overflow".to_owned())
        })
    }

    /// Return the exact queued-job, compression-block, and per-lane row
    /// geometry used by the authenticated composite-circuit capacity check.
    pub(crate) fn capacity_profile(&self) -> Result<(usize, usize, usize), String> {
        let blocks = self.compression_blocks()?;
        let lane_blocks = blocks.div_ceil(KAGEMUSHA_SHA256_LANES_V4);
        let required = lane_blocks
            .checked_mul(SHA256_ROWS_PER_BLOCK_V4)
            .and_then(|rows| {
                self.jobs
                    .len()
                    .checked_mul(SHA256_ROWS_PER_JOB_V4)
                    .and_then(|job_rows| rows.checked_add(job_rows))
            })
            .ok_or_else(|| "Kagemusha SHA-256 row count overflow".to_owned())?
            .max(SHA256_TABLE_ROWS_V4);
        Ok((self.jobs.len(), blocks, required))
    }

    /// Conservative per-lane capacity bound for authenticated usable rows.
    pub(crate) fn validate_capacity(&self, usable_rows: usize) -> Result<(), String> {
        let (_, blocks, required) = self.capacity_profile()?;
        if required > usable_rows {
            return Err(format!(
                "Kagemusha SHA-256 requires {required} rows per Table16 lane for {blocks} blocks, \
                 exceeding {usable_rows} authenticated usable rows"
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn with_padding_xor(mut self, padded_offset: usize, xor: u8) -> Self {
        self.padding_xor = Some((padded_offset, xor));
        self
    }

    #[cfg(test)]
    fn with_source_xor(mut self, job: usize, byte: usize, xor: u8) -> Self {
        self.source_xor = Some((job, byte, xor));
        self
    }

    #[cfg(test)]
    fn with_output_word_xor(mut self, job: usize, word: usize, xor: u32) -> Self {
        self.output_word_xor = Some((job, word, xor));
        self
    }

    #[cfg(test)]
    fn with_swapped_block_endian(mut self, global_block: usize) -> Self {
        self.swap_block_endian = Some(global_block);
        self
    }

    #[cfg(test)]
    fn with_broken_chain(mut self, global_block: usize) -> Self {
        self.break_chain_at_block = Some(global_block);
        self
    }

    #[cfg(test)]
    fn with_skipped_iv_reset(mut self, job: usize) -> Self {
        self.skip_iv_reset_at_job = Some(job);
        self
    }

    #[cfg(test)]
    fn shape(&self) -> Vec<usize> {
        self.jobs.iter().map(|job| job.message.len()).collect()
    }

    /// Realize all jobs after Base synthesis populated the physical cell map.
    ///
    /// Blocks are routed globally in job/block order across lanes 0..4.
    /// Every job starts from the standard IV; multi-block chaining is
    /// copy-constrained even when consecutive blocks use different lanes.
    pub(crate) fn synthesize(
        &self,
        config: &KagemushaSha256ConfigV4,
        layouter: &mut impl Layouter<F>,
        copy_manager: &SharedCopyConstraintManager<F>,
        usable_rows: usize,
    ) -> Result<(), Error> {
        self.validate_capacity(usable_rows)
            .map_err(|_| Error::Synthesis)?;
        Table16Chip::<F>::load(config.lanes[0].clone(), layouter)?;
        let chips = config.lanes.clone().map(Table16Chip::<F>::construct);
        // Base synthesis has finished populating this immutable map. Borrow it
        // for the SHA pass instead of cloning millions of virtual-to-physical
        // entries while the populated circuit and keygen assembly are live.
        let physical_cells = copy_manager.lock().unwrap();
        let mut global_block_index = 0_usize;
        #[cfg(test)]
        let mut previous_job_state = None;

        for (job_index, job) in self.jobs.iter().enumerate() {
            let suffix = canonical_padding_suffix(job.message.len()).ok_or(Error::Synthesis)?;
            let padded_len = job
                .message
                .len()
                .checked_add(suffix.len())
                .ok_or(Error::Synthesis)?;
            let mut padded = Vec::new();
            padded
                .try_reserve_exact(padded_len)
                .map_err(|_| Error::Synthesis)?;

            for (byte_index, byte) in job.message.iter().copied().enumerate() {
                #[cfg(not(test))]
                let _ = byte_index;
                let source_xor = {
                    #[cfg(test)]
                    {
                        self.source_xor
                            .filter(|(target_job, target_byte, _)| {
                                *target_job == job_index && *target_byte == byte_index
                            })
                            .map_or(0, |(_, _, xor)| xor)
                    }
                    #[cfg(not(test))]
                    {
                        0
                    }
                };
                match byte.source {
                    KagemushaSha256ByteSourceV4::Constant(byte) => {
                        padded.push(PaddedByte::Constant(byte ^ source_xor));
                    }
                    KagemushaSha256ByteSourceV4::Constrained(assigned) => {
                        let virtual_cell = assigned.cell.ok_or_else(|| {
                            iroha_logger::error!(
                                job_index,
                                byte_index,
                                "Kagemusha SHA-256 source lost its Base virtual-cell identity"
                            );
                            Error::Synthesis
                        })?;
                        let physical_cell = *physical_cells
                            .assigned_advices
                            .get(&virtual_cell)
                            .ok_or_else(|| {
                                iroha_logger::error!(
                                    job_index,
                                    byte_index,
                                    ?virtual_cell,
                                    "Kagemusha SHA-256 source is missing from the Base physical-cell map"
                                );
                                Error::Synthesis
                            })?;
                        let byte = u8::try_from(fe_to_biguint(assigned.value()))
                            .map_err(|_| Error::Synthesis)?
                            ^ source_xor;
                        let value = if self.use_unknown {
                            Value::unknown()
                        } else {
                            Value::known(byte)
                        };
                        padded.push(PaddedByte::Source(AssignedByte::from_range_checked_cell(
                            value,
                            physical_cell,
                        )));
                    }
                }
            }
            padded.extend(suffix.into_iter().map(PaddedByte::Constant));

            #[cfg(test)]
            if let Some(target_block) = self.swap_block_endian {
                let job_blocks = padded.len() / BLOCK_BYTE_SIZE;
                if (global_block_index..global_block_index + job_blocks).contains(&target_block) {
                    let local = (target_block - global_block_index) * BLOCK_BYTE_SIZE;
                    padded.swap(local, local + 3);
                    padded.swap(local + 1, local + 2);
                }
            }

            #[cfg(test)]
            if job_index == 0 {
                if let Some((offset, xor)) = self.padding_xor {
                    let target = padded.get_mut(offset).ok_or(Error::Synthesis)?;
                    match target {
                        PaddedByte::Constant(byte) => *byte ^= xor,
                        PaddedByte::Source(_) => return Err(Error::Synthesis),
                    }
                }
            }

            let mut blocks = padded.chunks_exact(BLOCK_BYTE_SIZE);
            let first = blocks.next().ok_or(Error::Synthesis)?;
            if !blocks.remainder().is_empty() {
                return Err(Error::Synthesis);
            }
            let first_lane = global_block_index % KAGEMUSHA_SHA256_LANES_V4;
            let first_block: [PaddedByte<F>; BLOCK_BYTE_SIZE] =
                first.to_vec().try_into().map_err(|_| Error::Synthesis)?;
            let first_words =
                chips[first_lane].assign_padded_block(layouter, first_block, global_block_index)?;
            let mut state = {
                #[cfg(test)]
                {
                    if self.skip_iv_reset_at_job == Some(job_index) {
                        previous_job_state.clone().ok_or(Error::Synthesis)?
                    } else {
                        chips[first_lane].initialization_vector(layouter)?
                    }
                }
                #[cfg(not(test))]
                {
                    chips[first_lane].initialization_vector(layouter)?
                }
            };
            state = chips[first_lane].compress(layouter, &state, first_words)?;
            let mut final_lane = first_lane;
            global_block_index += 1;

            for block in blocks {
                let lane = global_block_index % KAGEMUSHA_SHA256_LANES_V4;
                let block: [PaddedByte<F>; BLOCK_BYTE_SIZE] =
                    block.to_vec().try_into().map_err(|_| Error::Synthesis)?;
                let words = chips[lane].assign_padded_block(layouter, block, global_block_index)?;
                #[cfg(test)]
                if self.break_chain_at_block == Some(global_block_index) {
                    state = chips[lane].initialization_vector(layouter)?;
                }
                state = chips[lane].compress(layouter, &state, words)?;
                final_lane = lane;
                global_block_index += 1;
            }

            let digest = chips[final_lane].digest(layouter, &state)?;
            #[cfg(test)]
            {
                previous_job_state = Some(state);
            }
            layouter.assign_region(
                || format!("bind Kagemusha SHA-256 digest {job_index}"),
                |mut region| {
                    for (word_index, (actual, expected)) in
                        digest.iter().zip(&job.output_words).enumerate()
                    {
                        let virtual_cell = expected.cell.ok_or_else(|| {
                            iroha_logger::error!(
                                job_index,
                                word_index,
                                "Kagemusha SHA-256 output lost its Base virtual-cell identity"
                            );
                            Error::Synthesis
                        })?;
                        let physical_cell = *physical_cells
                            .assigned_advices
                            .get(&virtual_cell)
                            .ok_or_else(|| {
                                iroha_logger::error!(
                                    job_index,
                                    word_index,
                                    ?virtual_cell,
                                    "Kagemusha SHA-256 output is missing from the Base physical-cell map"
                                );
                                Error::Synthesis
                            })?;
                        region.constrain_equal(actual.cell(), physical_cell);
                    }
                    Ok(())
                },
            )?;
        }
        Ok(())
    }
}

/// Five Table16 lanes sharing one spread table and one fixed constant column.
#[derive(Clone, Debug)]
pub(crate) struct KagemushaSha256ConfigV4 {
    lanes: [Table16Config; KAGEMUSHA_SHA256_LANES_V4],
}

impl KagemushaSha256ConfigV4 {
    pub(crate) fn configure<F>(meta: &mut ConstraintSystem<F>) -> Self
    where
        F: PrimeField,
    {
        Self {
            lanes: Table16Chip::<F>::configure_lanes::<KAGEMUSHA_SHA256_LANES_V4>(meta),
        }
    }
}

#[cfg(test)]
mod tests {
    use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder};
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        halo2curves::pasta::Fp,
        plonk::{Circuit, ConstraintSystem, Error},
    };

    use super::*;

    const TEST_K: u32 = 17;
    const TEST_UNUSABLE_ROWS: usize = 9;

    #[derive(Clone, Debug)]
    struct QueueConfig<F: ScalarField> {
        base: BaseConfig<F>,
        sha: KagemushaSha256ConfigV4,
    }

    #[derive(Clone)]
    struct QueueCircuit<F: BigPrimeField + PrimeField + From<u64>> {
        builder: BaseCircuitBuilder<F>,
        jobs: KagemushaSha256JobsV4<F>,
    }

    impl<F> Circuit<F> for QueueCircuit<F>
    where
        F: BigPrimeField + PrimeField + From<u64>,
    {
        type Config = QueueConfig<F>;
        type FloorPlanner = V1;
        type Params = BaseCircuitParams;

        fn params(&self) -> Self::Params {
            self.builder.config_params.clone()
        }

        fn without_witnesses(&self) -> Self {
            Self {
                builder: self.builder.deep_clone().unknown(true),
                jobs: self.jobs.unknown(),
            }
        }

        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let usable_rows = (1_usize << params.k) - TEST_UNUSABLE_ROWS;
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows(usable_rows);
            QueueConfig {
                base,
                sha: KagemushaSha256ConfigV4::configure(meta),
            }
        }

        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("queue test uses parameterized Base config")
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.builder,
                config.base,
                layouter.namespace(|| "queue Base"),
            )?;
            self.jobs.synthesize(
                &config.sha,
                &mut layouter,
                &self.builder.core().copy_manager,
                (1_usize << TEST_K) - TEST_UNUSABLE_ROWS,
            )
        }
    }

    #[derive(Clone, Copy)]
    enum Mutation {
        None,
        Padding,
        Source,
        Output,
        Endian,
        Chain,
        IvReset,
    }

    fn queue_circuit(mutation: Mutation) -> QueueCircuit<Fp> {
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(TEST_K as usize)
            .use_lookup_bits(16);
        let range = builder.range_chip();
        let mut jobs = KagemushaSha256JobsV4::default();
        if matches!(mutation, Mutation::Output) {
            jobs = jobs.with_output_word_xor(0, 0, 1);
        }
        let messages = match mutation {
            Mutation::None => vec![
                (0..65).map(|index| index as u8).collect::<Vec<_>>(),
                b"abc".to_vec(),
                (0..256)
                    .map(|index| (index as u8).wrapping_mul(17))
                    .collect::<Vec<_>>(),
            ],
            Mutation::Chain => vec![(0..65).map(|index| index as u8).collect()],
            Mutation::IvReset => vec![b"first job".to_vec(), b"second job".to_vec()],
            Mutation::Padding | Mutation::Source | Mutation::Output | Mutation::Endian => {
                vec![b"abc".to_vec()]
            }
        };
        for message in messages {
            let assigned = builder
                .main(0)
                .assign_witnesses(message.into_iter().map(|byte| Fp::from(u64::from(byte))));
            jobs.digest(builder.main(0), &range, &assigned)
                .expect("canonical queue test bytes");
        }
        jobs = match mutation {
            Mutation::None | Mutation::Output => jobs,
            Mutation::Padding => jobs.with_padding_xor(3, 1),
            Mutation::Source => jobs.with_source_xor(0, 0, 1),
            Mutation::Endian => jobs.with_swapped_block_endian(0),
            Mutation::Chain => jobs.with_broken_chain(1),
            Mutation::IvReset => jobs.with_skipped_iv_reset(1),
        };
        builder.calculate_params(Some(TEST_UNUSABLE_ROWS));
        QueueCircuit { builder, jobs }
    }

    fn single_job_queue_circuit(
        mut builder: BaseCircuitBuilder<Fp>,
        calculate_params: bool,
    ) -> QueueCircuit<Fp> {
        let range = builder.range_chip();
        let assigned = builder
            .main(0)
            .assign_witnesses(b"abc".map(|byte| Fp::from(u64::from(byte))));
        let mut jobs = KagemushaSha256JobsV4::default();
        jobs.digest(builder.main(0), &range, &assigned)
            .expect("canonical single-job queue bytes");
        if calculate_params {
            builder.calculate_params(Some(TEST_UNUSABLE_ROWS));
        }
        QueueCircuit { builder, jobs }
    }

    fn verify(mutation: Mutation) -> Result<(), Vec<halo2_proofs::dev::VerifyFailure>> {
        let circuit = queue_circuit(mutation);
        MockProver::run(TEST_K, &circuit, vec![])
            .expect("five-lane queue synthesis")
            .verify()
    }

    fn typed_queue_circuit(source_xor: bool) -> QueueCircuit<Fp> {
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(TEST_K as usize)
            .use_lookup_bits(15);
        let range = builder.range_chip();
        let word = builder.main(0).load_witness(Fp::from(0x4433_2211_u64));
        let bits = KagemushaSha256BitV4::decompose(builder.main(0), range.gate(), word, 32);
        let mut message = vec![
            KagemushaSha256ByteV4::constant(0xaa),
            KagemushaSha256ByteV4::constant(0xbb),
        ];
        message.extend((0..4).map(|byte| {
            KagemushaSha256ByteV4::from_bits_le(
                builder.main(0),
                range.gate(),
                &bits[byte * 8..byte * 8 + 8],
            )
        }));
        let mut jobs = KagemushaSha256JobsV4::default();
        jobs.digest_constrained(builder.main(0), &message)
            .expect("Boolean-derived typed queue bytes");
        if source_xor {
            jobs = jobs.with_source_xor(0, 2, 1);
        }
        builder.calculate_params(Some(TEST_UNUSABLE_ROWS));
        QueueCircuit { builder, jobs }
    }

    fn queued_observation(typed: bool) -> (Vec<u8>, [u64; DIGEST_SIZE], usize) {
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(TEST_K as usize)
            .use_lookup_bits(15);
        let range = builder.range_chip();
        let mut jobs = KagemushaSha256JobsV4::default();
        let output_words = if typed {
            let word = builder.main(0).load_witness(Fp::from(0x4433_2211_u64));
            let bits = KagemushaSha256BitV4::decompose(builder.main(0), range.gate(), word, 32);
            let mut message = vec![
                KagemushaSha256ByteV4::constant(0xaa),
                KagemushaSha256ByteV4::constant(0xbb),
            ];
            message.extend((0..4).map(|byte| {
                KagemushaSha256ByteV4::from_bits_le(
                    builder.main(0),
                    range.gate(),
                    &bits[byte * 8..byte * 8 + 8],
                )
            }));
            jobs.digest_constrained(builder.main(0), &message)
                .expect("Boolean-derived typed observation bytes")
        } else {
            let message: [u8; 6] = [0xaa, 0xbb, 0x11, 0x22, 0x33, 0x44];
            let assigned = builder
                .main(0)
                .assign_witnesses(message.map(|byte| Fp::from(u64::from(byte))));
            jobs.digest(builder.main(0), &range, &assigned)
                .expect("generic observation bytes")
        };
        let preimage = jobs.jobs[0]
            .message
            .iter()
            .copied()
            .map(KagemushaSha256ByteV4::test_value)
            .collect();
        let output_words =
            output_words.map(|word| u64::try_from(fe_to_biguint(word.value())).unwrap());
        let lookup_rows = builder
            .statistics()
            .total_lookup_advice_per_phase
            .into_iter()
            .sum();
        (preimage, output_words, lookup_rows)
    }

    #[test]
    fn five_lane_round_robin_cross_lane_chaining_and_job_iv_reset_are_valid() {
        let circuit = queue_circuit(Mutation::None);
        assert_eq!(circuit.jobs.shape(), vec![65, 3, 256]);
        assert_eq!(circuit.jobs.compression_blocks().unwrap(), 8);
        assert_eq!(
            (0..8)
                .map(|block| block % KAGEMUSHA_SHA256_LANES_V4)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4, 0, 1, 2],
        );
        assert_eq!(verify(Mutation::None), Ok(()));
    }

    #[test]
    fn padding_tamper_is_rejected() {
        assert!(verify(Mutation::Padding).is_err());
    }

    #[test]
    fn source_copy_tamper_is_rejected() {
        assert!(verify(Mutation::Source).is_err());
    }

    #[test]
    fn constrained_byte_path_matches_generic_preimage_and_removes_range8_lookups() {
        let generic = queued_observation(false);
        let constrained = queued_observation(true);
        assert_eq!(generic.0, vec![0xaa, 0xbb, 0x11, 0x22, 0x33, 0x44]);
        assert_eq!(constrained.0, generic.0);
        assert_eq!(constrained.1, generic.1);
        assert_eq!(generic.2, 12, "six Range8 checks cost two rows each");
        assert_eq!(
            constrained.2, 0,
            "Boolean decomposition and literal bytes need no range lookup"
        );

        let circuit = typed_queue_circuit(false);
        MockProver::run(TEST_K, &circuit, vec![])
            .expect("typed SHA-256 queue synthesis")
            .assert_satisfied();
    }

    #[test]
    fn constrained_dynamic_byte_copy_tamper_is_rejected() {
        let circuit = typed_queue_circuit(true);
        assert!(
            MockProver::run(TEST_K, &circuit, vec![])
                .expect("typed SHA-256 queue synthesis")
                .verify()
                .is_err()
        );
    }

    #[test]
    fn digest_output_copy_tamper_is_rejected() {
        assert!(verify(Mutation::Output).is_err());
    }

    #[test]
    fn block_endian_tamper_is_rejected() {
        assert!(verify(Mutation::Endian).is_err());
    }

    #[test]
    fn cross_lane_chain_break_is_rejected() {
        assert!(verify(Mutation::Chain).is_err());
    }

    #[test]
    fn per_job_iv_reset_skip_is_rejected() {
        assert!(verify(Mutation::IvReset).is_err());
    }

    #[test]
    fn non_byte_base_source_is_rejected_before_queueing() {
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(TEST_K as usize)
            .use_lookup_bits(16);
        let range = builder.range_chip();
        let source = builder.main(0).load_witness(Fp::from(256));
        let mut jobs = KagemushaSha256JobsV4::default();
        assert!(
            jobs.digest(builder.main(0), &range, &[source]).is_err(),
            "host construction and the queued Base range check must both reject a non-byte"
        );
        assert!(jobs.jobs.is_empty());
    }

    #[test]
    fn k16_capacity_uses_only_the_loaded_spread_table_rows() {
        let jobs = KagemushaSha256JobsV4::<Fp>::default();
        assert_eq!(TABLE16_SPREAD_TABLE_ROWS, (1 << 16) - TEST_UNUSABLE_ROWS);
        assert_eq!(jobs.validate_capacity(TABLE16_SPREAD_TABLE_ROWS), Ok(()));
        assert!(
            jobs.validate_capacity(TABLE16_SPREAD_TABLE_ROWS - 1)
                .is_err()
        );
    }

    #[test]
    fn witnessless_clone_preserves_exact_keygen_job_shape() {
        let circuit = queue_circuit(Mutation::None);
        let witnessless = circuit.without_witnesses();
        assert_eq!(witnessless.jobs.shape(), circuit.jobs.shape());
        assert_eq!(
            witnessless.jobs.compression_blocks().unwrap(),
            circuit.jobs.compression_blocks().unwrap()
        );
        assert!(witnessless.jobs.use_unknown);
        let actual = &witnessless.builder.config_params;
        let expected = &circuit.builder.config_params;
        assert_eq!(actual.k, expected.k);
        assert_eq!(actual.num_advice_per_phase, expected.num_advice_per_phase);
        assert_eq!(actual.num_fixed, expected.num_fixed);
        assert_eq!(
            actual.num_lookup_advice_per_phase,
            expected.num_lookup_advice_per_phase
        );
        assert_eq!(actual.lookup_bits, expected.lookup_bits);
        assert_eq!(actual.num_instance_columns, expected.num_instance_columns);
    }

    #[test]
    fn witness_only_prover_shape_populates_direct_sha_physical_map() {
        let keygen = single_job_queue_circuit(
            BaseCircuitBuilder::<Fp>::new(false)
                .use_k(TEST_K as usize)
                .use_lookup_bits(16),
            true,
        );
        MockProver::run(TEST_K, &keygen, vec![])
            .expect("keygen-style SHA queue synthesis")
            .assert_satisfied();

        let prover = single_job_queue_circuit(
            BaseCircuitBuilder::prover(
                keygen.builder.config_params.clone(),
                keygen.builder.break_points(),
            ),
            false,
        );
        assert!(prover.builder.witness_gen_only());
        let expected_virtual_cells = prover.jobs.jobs[0]
            .message
            .iter()
            .filter_map(|byte| byte.assigned())
            .chain(prover.jobs.jobs[0].output_words)
            .map(|assigned| {
                assigned
                    .cell
                    .expect("witness-only Base values retain virtual identities")
            })
            .collect::<Vec<_>>();
        let mock =
            MockProver::run(TEST_K, &prover, vec![]).expect("witness-only SHA queue synthesis");
        {
            let copy_manager = prover.builder.core().copy_manager.lock().unwrap();
            assert!(
                expected_virtual_cells
                    .iter()
                    .all(|cell| copy_manager.assigned_advices.contains_key(cell)),
                "witness-only Base synthesis must materialize every SHA source and output"
            );
        }
        mock.assert_satisfied();
    }
}
