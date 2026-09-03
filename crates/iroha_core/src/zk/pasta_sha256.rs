//! Fixed-shape Table8 SHA-256 jobs for paired-Pasta circuits.
//!
//! The Base circuit records each hash relation while it is built. After Base
//! synthesis has established the virtual-to-physical cell map, five Table8
//! lanes realize those relations. Source bytes and digest words are
//! copy-constrained across the two layouts.
use super::pasta_sha256_table8::{
    AssignedBlockWord, AssignedByte, BLOCK_BYTE_SIZE, DIGEST_SIZE, IV, PaddedByte,
    Sha256Instructions, TABLE8_SPREAD_TABLE_ROWS, Table8Chip, Table8Config,
    canonical_padding_suffix,
};
use ff::PrimeField;
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateChip, GateInstructions, RangeChip, RangeInstructions},
    halo2_proofs::{
        circuit::{Layouter, Value},
        plonk::{ConstraintSystem, Error},
    },
    utils::{BigPrimeField, ScalarField, fe_to_biguint},
    virtual_region::copy_constraints::{CopyConstraintManager, SharedCopyConstraintManager},
};
use sha2::{Digest as _, Sha256, compress256, digest::generic_array::GenericArray};
/// Independent Table8 lanes fixed by the V1 circuit identity.
pub(crate) const PASTA_SHA256_LANES_V1: usize = 5;
// Table8 uses about 2,267 rows per compression block. Keep a small explicit
// margin for layout evolution, plus per-job IV and digest regions.
const SHA256_ROWS_PER_BLOCK_V1: usize = 2_304;
const SHA256_ROWS_PER_JOB_V1: usize = 64;
// A bounded job exposes every chaining state, not only its final one. The
// existing per-job allowance already includes one four-row digest region.
const SHA256_ROWS_PER_EXTRA_SNAPSHOT_V1: usize = 4;
const SHA256_TABLE_ROWS_V1: usize = TABLE8_SPREAD_TABLE_ROWS;
/// One Boolean cell produced or checked by the SHA-byte provenance API.
///
/// The assigned cell is deliberately private: callers can only obtain this
/// token through [`PastaSha256BitV1::decompose`], so composing a
/// [`PastaSha256ByteV1`] from eight tokens carries a real Boolean proof.
#[derive(Clone, Copy, Debug)]
pub(super) struct PastaSha256BitV1<F: ScalarField> {
    assigned: AssignedValue<F>,
}
impl<F: BigPrimeField> PastaSha256BitV1<F> {
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
enum PastaSha256ByteSourceV1<F: ScalarField> {
    Constant(u8),
    Constrained(AssignedValue<F>),
}
/// A SHA-256 message byte carrying its circuit provenance.
///
/// Constants remain Table8 constants. Dynamic bytes can only be constructed
/// from eight proven Boolean cells or an explicit caller-side Range8 check.
/// The test-only generic digest helper applies the same checked fallback.
/// Consequently
/// [`PastaSha256JobsV1::digest_constrained`] does not repeat a Range8 lookup
/// for these values.
#[derive(Clone, Copy, Debug)]
pub(super) struct PastaSha256ByteV1<F: ScalarField> {
    source: PastaSha256ByteSourceV1<F>,
}
impl<F: BigPrimeField> PastaSha256ByteV1<F> {
    /// Construct a literal message byte.
    pub(super) const fn constant(byte: u8) -> Self {
        Self {
            source: PastaSha256ByteSourceV1::Constant(byte),
        }
    }
    /// Compose one little-endian byte from eight Boolean-proven cells.
    pub(super) fn from_bits_le(
        ctx: &mut Context<F>,
        gate: &GateChip<F>,
        bits: &[PastaSha256BitV1<F>],
    ) -> Self {
        assert_eq!(bits.len(), 8, "one SHA-256 byte has exactly eight bits");
        let assigned = gate.inner_product(
            ctx,
            bits.iter().copied().map(|bit| bit.assigned()),
            (0..8).map(|bit| QuantumCell::Constant(F::from(1_u64 << bit))),
        );
        Self {
            source: PastaSha256ByteSourceV1::Constrained(assigned),
        }
    }
    /// Decompose one constrained byte back into Boolean-proven cells.
    pub(super) fn decompose_bits_le(
        self,
        ctx: &mut Context<F>,
        gate: &GateChip<F>,
    ) -> [PastaSha256BitV1<F>; 8] {
        match self.source {
            PastaSha256ByteSourceV1::Constant(byte) => {
                std::array::from_fn(|bit| PastaSha256BitV1 {
                    assigned: ctx.load_constant(F::from(u64::from((byte >> bit) & 1))),
                })
            }
            PastaSha256ByteSourceV1::Constrained(assigned) => {
                PastaSha256BitV1::decompose(ctx, gate, assigned, 8)
                    .try_into()
                    .expect("eight-bit decomposition has fixed length")
            }
        }
    }
    /// Return the underlying assigned cell when this is a dynamic byte.
    pub(super) fn assigned(self) -> Option<AssignedValue<F>> {
        match self.source {
            PastaSha256ByteSourceV1::Constant(_) => None,
            PastaSha256ByteSourceV1::Constrained(assigned) => Some(assigned),
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
            source: PastaSha256ByteSourceV1::Constrained(assigned),
        }
    }
    /// Return this proven byte as a linear-combination input.
    pub(super) fn quantum_cell(self) -> QuantumCell<F> {
        match self.source {
            PastaSha256ByteSourceV1::Constant(byte) => {
                QuantumCell::Constant(F::from(u64::from(byte)))
            }
            PastaSha256ByteSourceV1::Constrained(assigned) => QuantumCell::Existing(assigned),
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
            PastaSha256ByteSourceV1::Constant(byte) => Ok(byte),
            PastaSha256ByteSourceV1::Constrained(assigned) => {
                if assigned.cell.is_none() {
                    return Err(format!(
                        "Paired Pasta SHA-256 message cell {index} has no virtual-cell identity"
                    ));
                }
                u8::try_from(fe_to_biguint(assigned.value())).map_err(|_| {
                    format!("Paired Pasta SHA-256 message cell {index} is not a canonical byte")
                })
            }
        }
    }
}
#[derive(Clone, Debug)]
struct PastaSha256JobV1<F: ScalarField> {
    message: Vec<PastaSha256ByteV1<F>>,
    output_words: [AssignedValue<F>; DIGEST_SIZE],
    bounded: Option<PastaSha256BoundedJobV1<F>>,
}
#[derive(Clone, Debug)]
struct PastaSha256BoundedJobV1<F: ScalarField> {
    padded: Vec<PastaSha256ByteV1<F>>,
    block_outputs: Vec<[AssignedValue<F>; DIGEST_SIZE]>,
    #[cfg(test)]
    final_block_selectors: Vec<AssignedValue<F>>,
}
/// Explicit, circuit-owned SHA jobs. There is deliberately no global or
/// thread-local queue: witness stripping clones this exact job shape.
#[derive(Clone, Debug)]
pub(crate) struct PastaSha256JobsV1<F: ScalarField> {
    jobs: Vec<PastaSha256JobV1<F>>,
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
impl<F: ScalarField> Default for PastaSha256JobsV1<F> {
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
impl<F> PastaSha256JobsV1<F>
where
    F: BigPrimeField + PrimeField + From<u64>,
{
    /// Range-constrain every Base source byte, queue its exact SHA-256
    /// relation, and return eight Base placeholders for the digest words.
    #[cfg(test)]
    pub(crate) fn digest(
        &mut self,
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        message: &[AssignedValue<F>],
    ) -> Result<[AssignedValue<F>; DIGEST_SIZE], String> {
        let constrained = message
            .iter()
            .copied()
            .map(|assigned| PastaSha256ByteV1::range_checked(ctx, range, assigned))
            .collect::<Vec<_>>();
        self.digest_constrained(ctx, &constrained)
    }
    /// Queue bytes whose constant or Boolean-decomposition provenance already
    /// proves they are canonical bytes.
    pub(super) fn digest_constrained(
        &mut self,
        ctx: &mut Context<F>,
        message: &[PastaSha256ByteV1<F>],
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
            return Err("Paired Pasta SHA-256 output has no virtual-cell identity".to_owned());
        }
        self.jobs.push(PastaSha256JobV1 {
            message: message.to_vec(),
            output_words,
            bounded: None,
        });
        Ok(output_words)
    }
    /// Hash an active prefix of a fixed-capacity message without changing the key shape.
    ///
    /// `message.len()` and each source byte's constant/assigned provenance must be fixed by the
    /// calling circuit. `message_len` is a byte length, constrained to at most that capacity;
    /// every inactive source byte is constrained to zero. Padding bytes, the full big-endian
    /// 64-bit bit length, and the selected final block are constrained in Base. Table8 always
    /// compresses the capacity's maximum padded block count and copy-binds every intermediate
    /// chaining state. Later zero blocks do not contribute to the selected digest.
    ///
    /// The return value uses the same eight big-endian u32 words as `digest_constrained`.
    /// Domain separators and any application length framing belong in `message`; this method
    /// neither changes an application hash nor introduces an application-specific length cap.
    pub(super) fn digest_bounded_constrained(
        &mut self,
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        message: &[PastaSha256ByteV1<F>],
        message_len: AssignedValue<F>,
    ) -> Result<[AssignedValue<F>; DIGEST_SIZE], String> {
        let capacity = u64::try_from(message.len())
            .map_err(|_| "Paired Pasta bounded SHA-256 capacity exceeds u64".to_owned())?;
        let suffix = canonical_padding_suffix(message.len())
            .ok_or_else(|| "Paired Pasta bounded SHA-256 capacity is not encodable".to_owned())?;
        let padded_len = message
            .len()
            .checked_add(suffix.len())
            .ok_or_else(|| "Paired Pasta bounded SHA-256 padded capacity overflow".to_owned())?;
        let max_blocks = padded_len / BLOCK_BYTE_SIZE;
        let max_blocks_u64 = u64::try_from(max_blocks)
            .map_err(|_| "Paired Pasta bounded SHA-256 block count exceeds u64".to_owned())?;
        if message_len.cell.is_none() {
            return Err(
                "Paired Pasta bounded SHA-256 length has no virtual-cell identity".to_owned(),
            );
        }
        let native_len = u64::try_from(fe_to_biguint(message_len.value()))
            .map_err(|_| "Paired Pasta bounded SHA-256 length exceeds u64".to_owned())?;
        if native_len > capacity {
            return Err("Paired Pasta bounded SHA-256 length exceeds its capacity".to_owned());
        }
        for (index, byte) in message.iter().copied().enumerate() {
            byte.value(index)?;
        }

        let gate = range.gate();
        // canonical_padding_suffix has already established capacity <= u64::MAX / 8. All
        // padding arithmetic below therefore fits an ordinary u64 without Pasta-field wrap.
        range.check_less_than_safe(ctx, message_len, capacity + 1);
        let length_bits = (u64::BITS - capacity.leading_zeros()).max(1) as usize;
        range.range_check(ctx, message_len, length_bits);
        let native_blocks = (native_len + 9).div_ceil(BLOCK_BYTE_SIZE as u64);
        let blocks = ctx.load_witness(F::from(native_blocks));
        range.check_less_than_safe(ctx, blocks, max_blocks_u64 + 1);
        let blocks_zero = gate.is_zero(ctx, blocks);
        gate.assert_is_const(ctx, &blocks_zero, &F::ZERO);
        let padded_length = gate.mul(
            ctx,
            blocks,
            QuantumCell::Constant(F::from(BLOCK_BYTE_SIZE as u64)),
        );
        let message_and_trailer = gate.add(ctx, message_len, QuantumCell::Constant(F::from(9)));
        let zero_padding = gate.sub(ctx, padded_length, message_and_trailer);
        range.range_check(ctx, zero_padding, 6);
        let final_block_selectors = (1..=max_blocks_u64)
            .map(|block| gate.is_equal(ctx, blocks, QuantumCell::Constant(F::from(block))))
            .collect::<Vec<_>>();
        let one_final_block = gate.sum(ctx, final_block_selectors.iter().copied());
        gate.assert_is_const(ctx, &one_final_block, &F::ONE);

        let bit_length = gate.mul(ctx, message_len, QuantumCell::Constant(F::from(8)));
        let bit_length_bits = PastaSha256BitV1::decompose(ctx, gate, bit_length, 64);
        let bit_length_bytes: [PastaSha256ByteV1<F>; 8] = std::array::from_fn(|index| {
            let start = (7 - index) * 8;
            PastaSha256ByteV1::from_bits_le(ctx, gate, &bit_length_bits[start..start + 8])
        });
        let mut padded = Vec::new();
        padded
            .try_reserve_exact(padded_len)
            .map_err(|_| "Paired Pasta bounded SHA-256 padded allocation failed".to_owned())?;
        for index in 0..padded_len {
            let index_u64 = u64::try_from(index)
                .map_err(|_| "Paired Pasta bounded SHA-256 offset exceeds u64".to_owned())?;
            let mut terms = Vec::with_capacity(3);
            if let Some(byte) = message.get(index) {
                let active = range.is_less_than(
                    ctx,
                    QuantumCell::Constant(F::from(index_u64)),
                    message_len,
                    length_bits,
                );
                let inactive_byte = gate.mul_not(ctx, active, byte.quantum_cell());
                gate.assert_is_const(ctx, &inactive_byte, &F::ZERO);
                terms.push(gate.mul(ctx, active, byte.quantum_cell()));
            }
            if index <= message.len() {
                let marker =
                    gate.is_equal(ctx, message_len, QuantumCell::Constant(F::from(index_u64)));
                terms.push(gate.mul(ctx, marker, QuantumCell::Constant(F::from(0x80))));
            }
            let block_offset = index % BLOCK_BYTE_SIZE;
            if block_offset >= BLOCK_BYTE_SIZE - 8 {
                terms.push(gate.mul(
                    ctx,
                    final_block_selectors[index / BLOCK_BYTE_SIZE],
                    bit_length_bytes[block_offset - (BLOCK_BYTE_SIZE - 8)].quantum_cell(),
                ));
            }
            // Always retain an assigned source, including zero bytes outside the actual final
            // block. Neither witness values nor length can change Table8's fixed constants.
            let value = gate.sum(ctx, terms);
            padded.push(PastaSha256ByteV1::range_checked(ctx, range, value));
        }

        let native_padded = padded
            .iter()
            .copied()
            .enumerate()
            .map(|(index, byte)| byte.value(index))
            .collect::<Result<Vec<_>, _>>()?;
        let mut state = IV;
        let mut block_outputs = Vec::new();
        block_outputs
            .try_reserve_exact(max_blocks)
            .map_err(|_| "Paired Pasta bounded SHA-256 snapshot allocation failed".to_owned())?;
        for block in native_padded.chunks_exact(BLOCK_BYTE_SIZE) {
            compress256(
                &mut state,
                core::slice::from_ref(GenericArray::from_slice(block)),
            );
            block_outputs.push(state.map(|word| ctx.load_witness(F::from(u64::from(word)))));
        }
        let output_words = std::array::from_fn(|word| {
            gate.inner_product(
                ctx,
                block_outputs.iter().map(|output| output[word]),
                final_block_selectors
                    .iter()
                    .copied()
                    .map(QuantumCell::Existing),
            )
        });
        if output_words
            .iter()
            .chain(block_outputs.iter().flatten())
            .any(|word| word.cell.is_none())
        {
            return Err(
                "Paired Pasta bounded SHA-256 output has no virtual-cell identity".to_owned(),
            );
        }
        self.jobs.push(PastaSha256JobV1 {
            message: message.to_vec(),
            output_words,
            bounded: Some(PastaSha256BoundedJobV1 {
                padded,
                block_outputs,
                #[cfg(test)]
                final_block_selectors,
            }),
        });
        Ok(output_words)
    }
    /// Preserve job count, lengths, and virtual cells while hiding all raw
    /// Table8 witnesses during key generation.
    pub(crate) fn unknown(&self) -> Self {
        let mut clone = self.clone();
        clone.use_unknown = true;
        clone
    }
    pub(crate) fn compression_blocks(&self) -> Result<usize, String> {
        self.jobs.iter().try_fold(0_usize, |total, job| {
            let blocks = if let Some(bounded) = &job.bounded {
                bounded.block_outputs.len()
            } else {
                let suffix = canonical_padding_suffix(job.message.len()).ok_or_else(|| {
                    "Paired Pasta SHA-256 message length is not encodable".to_owned()
                })?;
                job.message
                    .len()
                    .checked_add(suffix.len())
                    .ok_or_else(|| "Paired Pasta SHA-256 padded length overflow".to_owned())?
                    / BLOCK_BYTE_SIZE
            };
            total
                .checked_add(blocks)
                .ok_or_else(|| "Paired Pasta SHA-256 block count overflow".to_owned())
        })
    }
    /// Copy the exact queued message bytes for private proof-stage planning.
    ///
    /// This does not serialize or publish the preimages.  It lets a bounded helper plan derive
    /// its canonical padding and ordered compression leaves from the same typed cells that built
    /// the monolithic relation, preventing a second host encoder from drifting from circuit
    /// semantics.
    pub(crate) fn canonical_messages(&self) -> Result<Vec<Vec<u8>>, String> {
        self.jobs
            .iter()
            .enumerate()
            .map(|(job_index, job)| {
                job.message
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(byte_index, byte)| {
                        byte.value(byte_index).map_err(|error| {
                            format!("Paired Pasta SHA-256 job {job_index}: {error}")
                        })
                    })
                    .collect()
            })
            .collect()
    }
    /// Return the exact queued-job, compression-block, and per-lane row
    /// geometry used by the authenticated composite-circuit capacity check.
    pub(crate) fn capacity_profile(&self) -> Result<(usize, usize, usize), String> {
        let blocks = self.compression_blocks()?;
        let lane_blocks = blocks.div_ceil(PASTA_SHA256_LANES_V1);
        let extra_snapshots = self.jobs.iter().try_fold(0_usize, |count, job| {
            count.checked_add(
                job.bounded
                    .as_ref()
                    .map_or(0, |bounded| bounded.block_outputs.len().saturating_sub(1)),
            )
        });
        let required = lane_blocks
            .checked_mul(SHA256_ROWS_PER_BLOCK_V1)
            .and_then(|rows| {
                self.jobs
                    .len()
                    .checked_mul(SHA256_ROWS_PER_JOB_V1)
                    .and_then(|job_rows| rows.checked_add(job_rows))
            })
            .and_then(|rows| {
                extra_snapshots?
                    .checked_mul(SHA256_ROWS_PER_EXTRA_SNAPSHOT_V1)
                    .and_then(|snapshot_rows| rows.checked_add(snapshot_rows))
            })
            .ok_or_else(|| "Paired Pasta SHA-256 row count overflow".to_owned())?
            .max(SHA256_TABLE_ROWS_V1);
        Ok((self.jobs.len(), blocks, required))
    }
    /// Conservative per-lane capacity bound for authenticated usable rows.
    pub(crate) fn validate_capacity(&self, usable_rows: usize) -> Result<(), String> {
        let (_, blocks, required) = self.capacity_profile()?;
        if required > usable_rows {
            return Err(format!(
                "Paired Pasta SHA-256 requires {required} rows per Table8 lane for {blocks} blocks, \
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
        config: &PastaSha256ConfigV1,
        layouter: &mut impl Layouter<F>,
        copy_manager: &SharedCopyConstraintManager<F>,
        usable_rows: usize,
    ) -> Result<(), Error> {
        self.validate_capacity(usable_rows)
            .map_err(|_| Error::Synthesis)?;
        Table8Chip::<F>::load(config.lanes[0].clone(), layouter)?;
        let chips = config.lanes.clone().map(Table8Chip::<F>::construct);
        // Base synthesis has finished populating this immutable map. Borrow it
        // for the SHA pass instead of cloning millions of virtual-to-physical
        // entries while the populated circuit and keygen assembly are live.
        let physical_cells = copy_manager.lock().unwrap();
        let mut global_block_index = 0_usize;
        #[cfg(test)]
        let mut previous_job_state = None;
        for (job_index, job) in self.jobs.iter().enumerate() {
            let (message, suffix) = if let Some(bounded) = &job.bounded {
                if bounded.block_outputs.is_empty()
                    || bounded.block_outputs.len().checked_mul(BLOCK_BYTE_SIZE)
                        != Some(bounded.padded.len())
                {
                    return Err(Error::Synthesis);
                }
                (bounded.padded.as_slice(), Vec::new())
            } else {
                (
                    job.message.as_slice(),
                    canonical_padding_suffix(job.message.len()).ok_or(Error::Synthesis)?,
                )
            };
            let padded_len = message
                .len()
                .checked_add(suffix.len())
                .ok_or(Error::Synthesis)?;
            let mut padded = Vec::new();
            padded
                .try_reserve_exact(padded_len)
                .map_err(|_| Error::Synthesis)?;
            for (byte_index, byte) in message.iter().copied().enumerate() {
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
                    PastaSha256ByteSourceV1::Constant(byte) => {
                        padded.push(PaddedByte::Constant(byte ^ source_xor));
                    }
                    PastaSha256ByteSourceV1::Constrained(assigned) => {
                        let virtual_cell = assigned.cell.ok_or_else(|| {
                            iroha_logger::error!(
                                job_index,
                                byte_index,
                                "Paired Pasta SHA-256 source lost its Base virtual-cell identity"
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
                                    "Paired Pasta SHA-256 source is missing from the Base physical-cell map"
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
            let first_lane = global_block_index % PASTA_SHA256_LANES_V1;
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
            if let Some(bounded) = &job.bounded {
                let digest = chips[first_lane].digest(layouter, &state)?;
                bind_sha256_digest_v1(
                    layouter,
                    &physical_cells,
                    &digest,
                    &bounded.block_outputs[0],
                    format!("bind Paired Pasta bounded SHA-256 digest {job_index} block 0"),
                )?;
            }
            let mut final_lane = first_lane;
            global_block_index += 1;
            for (block_index, block) in blocks.enumerate() {
                let lane = global_block_index % PASTA_SHA256_LANES_V1;
                let block: [PaddedByte<F>; BLOCK_BYTE_SIZE] =
                    block.to_vec().try_into().map_err(|_| Error::Synthesis)?;
                let words = chips[lane].assign_padded_block(layouter, block, global_block_index)?;
                #[cfg(test)]
                if self.break_chain_at_block == Some(global_block_index) {
                    state = chips[lane].initialization_vector(layouter)?;
                }
                state = chips[lane].compress(layouter, &state, words)?;
                if let Some(bounded) = &job.bounded {
                    let digest = chips[lane].digest(layouter, &state)?;
                    bind_sha256_digest_v1(
                        layouter,
                        &physical_cells,
                        &digest,
                        &bounded.block_outputs[block_index + 1],
                        format!(
                            "bind Paired Pasta bounded SHA-256 digest {job_index} block {}",
                            block_index + 1
                        ),
                    )?;
                }
                final_lane = lane;
                global_block_index += 1;
            }
            if job.bounded.is_none() {
                let digest = chips[final_lane].digest(layouter, &state)?;
                bind_sha256_digest_v1(
                    layouter,
                    &physical_cells,
                    &digest,
                    &job.output_words,
                    format!("bind Paired Pasta SHA-256 digest {job_index}"),
                )?;
            }
            #[cfg(test)]
            {
                previous_job_state = Some(state);
            }
        }
        Ok(())
    }
}

fn bind_sha256_digest_v1<F: BigPrimeField>(
    layouter: &mut impl Layouter<F>,
    physical_cells: &CopyConstraintManager<F>,
    digest: &[AssignedBlockWord<F>; DIGEST_SIZE],
    output_words: &[AssignedValue<F>; DIGEST_SIZE],
    label: String,
) -> Result<(), Error> {
    layouter.assign_region(
        || label.clone(),
        |mut region| {
            for (word_index, (actual, expected)) in digest.iter().zip(output_words).enumerate() {
                let virtual_cell = expected.cell.ok_or_else(|| {
                    iroha_logger::error!(
                        word_index,
                        "Paired Pasta SHA-256 output lost its Base virtual-cell identity"
                    );
                    Error::Synthesis
                })?;
                let physical_cell = *physical_cells
                    .assigned_advices
                    .get(&virtual_cell)
                    .ok_or_else(|| {
                        iroha_logger::error!(
                            word_index,
                            ?virtual_cell,
                            "Paired Pasta SHA-256 output is missing from the Base physical-cell map"
                        );
                        Error::Synthesis
                    })?;
                region.constrain_equal(actual.cell(), physical_cell);
            }
            Ok(())
        },
    )
}
/// Five Table8 lanes sharing one spread table and one fixed constant column.
#[derive(Clone, Debug)]
pub(crate) struct PastaSha256ConfigV1 {
    lanes: [Table8Config; PASTA_SHA256_LANES_V1],
}
impl PastaSha256ConfigV1 {
    pub(crate) fn configure<F>(meta: &mut ConstraintSystem<F>) -> Self
    where
        F: PrimeField,
    {
        Self {
            lanes: Table8Chip::<F>::configure_lanes::<PASTA_SHA256_LANES_V1>(meta),
        }
    }
}
#[cfg(test)]
#[path = "pasta_sha256_bounded_tests.rs"]
mod bounded_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder};
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        halo2curves::pasta::Fp,
        plonk::{Circuit, ConstraintSystem, Error},
    };
    const TEST_K: u32 = 17;
    const TEST_UNUSABLE_ROWS: usize = 9;
    #[derive(Clone, Debug)]
    struct QueueConfig<F: ScalarField> {
        base: BaseConfig<F>,
        sha: PastaSha256ConfigV1,
    }
    #[derive(Clone)]
    struct QueueCircuit<F: BigPrimeField + PrimeField + From<u64>> {
        builder: BaseCircuitBuilder<F>,
        jobs: PastaSha256JobsV1<F>,
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
                sha: PastaSha256ConfigV1::configure(meta),
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
        let mut jobs = PastaSha256JobsV1::default();
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
        let mut jobs = PastaSha256JobsV1::default();
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
        let bits = PastaSha256BitV1::decompose(builder.main(0), range.gate(), word, 32);
        let mut message = vec![
            PastaSha256ByteV1::constant(0xaa),
            PastaSha256ByteV1::constant(0xbb),
        ];
        message.extend((0..4).map(|byte| {
            PastaSha256ByteV1::from_bits_le(
                builder.main(0),
                range.gate(),
                &bits[byte * 8..byte * 8 + 8],
            )
        }));
        let mut jobs = PastaSha256JobsV1::default();
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
        let mut jobs = PastaSha256JobsV1::default();
        let output_words = if typed {
            let word = builder.main(0).load_witness(Fp::from(0x4433_2211_u64));
            let bits = PastaSha256BitV1::decompose(builder.main(0), range.gate(), word, 32);
            let mut message = vec![
                PastaSha256ByteV1::constant(0xaa),
                PastaSha256ByteV1::constant(0xbb),
            ];
            message.extend((0..4).map(|byte| {
                PastaSha256ByteV1::from_bits_le(
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
            .map(PastaSha256ByteV1::test_value)
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
                .map(|block| block % PASTA_SHA256_LANES_V1)
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
        let mut jobs = PastaSha256JobsV1::default();
        assert!(
            jobs.digest(builder.main(0), &range, &[source]).is_err(),
            "host construction and the queued Base range check must both reject a non-byte"
        );
        assert!(jobs.jobs.is_empty());
    }
    #[test]
    fn capacity_uses_only_the_loaded_spread_table_rows() {
        let jobs = PastaSha256JobsV1::<Fp>::default();
        assert_eq!(TABLE8_SPREAD_TABLE_ROWS, 1 << 8);
        assert_eq!(jobs.validate_capacity(TABLE8_SPREAD_TABLE_ROWS), Ok(()));
        assert!(
            jobs.validate_capacity(TABLE8_SPREAD_TABLE_ROWS - 1)
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
