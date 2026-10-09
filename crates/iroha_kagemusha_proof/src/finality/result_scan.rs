//! Complete native result hashing over one authenticated byte tape.
//!
//! Three source layouts prove initialization, one consecutive 128-byte
//! compression, and completion. Every continuation commits all eight chaining
//! words and exact byte progress. A fixed 515-leaf schedule constrains completed
//! streams to unchanged padding steps, so every frame uses the same source keys;
//! a terminal owner must join the entire interval and pin both boundary digests.
//! This source authenticates a tape against a claimed R digest only. Consensus
//! authorization, canonical result parsing and carried IPA decisions remain the
//! finality owner's obligations.

use std::sync::Arc;

use ff::Field;
use iroha_pasta::{Ep, Fp, poseidon::hash_with_domain};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
};
use iroha_plonk_gadgets::{
    GlueChip, Uint, Word,
    blake2b::{Blake2bChip, Blake2bConfig},
};
use iroha_plonk_recursion::verifier::{VerifierChip, VerifierConfig};

use super::{
    continuity::{SourceCheckpoint, SourceEndpoints, leaf_frame_native},
    result::{MAX_RESULT_BYTES, RESULT_TAG},
    schedule::tape::{ResultTape, ResultTapeWitness, TapeResultHashStream},
};

mod witness;
pub(crate) use witness::marked_hash_one_block;
pub use witness::prepare_result_scan;

/// Fixed source program identity.
pub const RESULT_SCAN_PROGRAM: u64 = u64::from_le_bytes(*b"kgwrscp1");
/// Domain committing the tape, exact length and certified result digest.
pub const RESULT_SCAN_CONTEXT: u64 = u64::from_le_bytes(*b"kgwrscc1");
/// Domain committing every continuation register and phase boundary.
pub const RESULT_SCAN_STATE: u64 = u64::from_le_bytes(*b"kgwrscs1");
/// Fixed number of compression slots, sufficient for every bounded frame.
pub const RESULT_SCAN_BLOCKS: u32 = 513;
/// Complete interval length, including initialization and completion.
pub const RESULT_SCAN_LEAVES: u32 = RESULT_SCAN_BLOCKS + 2;

/// Three separately qualified layouts; witnesses never select an opcode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResultScanPlan {
    /// Pin the native IV and zero processed bytes at interval 0..1.
    #[default]
    Start,
    /// Compress the next block, or preserve a completely consumed stream.
    Absorb,
    /// Require full consumption and equality to the expected marked R digest.
    Finish,
}

/// Untrusted immutable context shared by every scan leaf.
#[derive(Clone, Copy, Debug)]
pub struct ResultScanContext {
    /// Internal Poseidon commitment to the complete source byte tape.
    pub root: Fp,
    /// Exact original Norito frame length, excluding the result domain tag.
    pub frame_len: u32,
    /// Expected native marked result digest; the terminal owner binds it to QC R.
    pub expected: [u8; 32],
}
impl ResultScanContext {
    /// Exact context commitment, with no authenticity implied by computing it.
    pub fn digest(&self) -> Fp {
        let mut words = vec![
            Fp::from(RESULT_SCAN_PROGRAM),
            self.root,
            Fp::from(u64::from(self.frame_len)),
        ];
        words.extend(self.expected.map(|byte| Fp::from(u64::from(byte))));
        hash_with_domain(RESULT_SCAN_CONTEXT, &words)
    }
    /// Deterministic empty initial or completed state for a terminal owner.
    pub fn boundary_digest(&self, terminal: bool) -> Fp {
        let cursor = if terminal { RESULT_SCAN_LEAVES } else { 0 };
        hash_with_domain(
            RESULT_SCAN_STATE,
            &[
                Fp::from(RESULT_SCAN_PROGRAM),
                self.digest(),
                Fp::from(u64::from(cursor)),
                Fp::from(if terminal { 2 } else { 0 }),
            ],
        )
    }
}

/// Recompute the immutable context from cells linked to a terminal owner.
/// # Errors
/// Layout errors; out-of-range lengths or non-byte digest inputs fail.
pub fn context_digest_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    root: &Word<Fp>,
    frame_len: &Uint<Fp, 32>,
    expected: &[Word<Fp>; 32],
) -> Result<Word<Fp>, Error> {
    let maximum = chip
        .uint()
        .constant::<32>(region, u128::from(MAX_RESULT_BYTES))?;
    chip.uint().assert_le(region, frame_len, &maximum)?;
    let program = chip
        .uint()
        .glue()
        .constant(region, Fp::from(RESULT_SCAN_PROGRAM))?;
    let mut words = vec![program, root.clone(), frame_len.word().clone()];
    for byte in expected {
        chip.uint().range_check::<8>(region, byte)?;
        words.push(byte.clone());
    }
    chip.hash_words(region, RESULT_SCAN_CONTEXT, &words)
}

/// Bound the frame length and return the fixed complete scan end cursor.
/// # Errors
/// Layout errors; invalid frame lengths fail.
pub fn end_cursor_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    frame_len: &Uint<Fp, 32>,
) -> Result<Uint<Fp, 32>, Error> {
    let maximum = chip
        .uint()
        .constant::<32>(region, u128::from(MAX_RESULT_BYTES))?;
    chip.uint().assert_le(region, frame_len, &maximum)?;
    chip.uint().constant(region, u128::from(RESULT_SCAN_LEAVES))
}

/// Bind a deterministic empty program boundary. The phase is circuit-fixed.
/// The initial and terminal cursors are constrained to their fixed constants.
/// # Errors
/// Layout errors.
pub fn boundary_digest_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    context: &Word<Fp>,
    cursor: &Uint<Fp, 32>,
    terminal: bool,
) -> Result<Word<Fp>, Error> {
    GlueChip::assert_constant(
        region,
        cursor.word(),
        Fp::from(if terminal {
            u64::from(RESULT_SCAN_LEAVES)
        } else {
            0
        }),
    )?;
    let program = chip
        .uint()
        .glue()
        .constant(region, Fp::from(RESULT_SCAN_PROGRAM))?;
    let tag = chip
        .uint()
        .glue()
        .constant(region, Fp::from(if terminal { 2 } else { 0 }))?;
    chip.hash_words(
        region,
        RESULT_SCAN_STATE,
        &[program, context.clone(), cursor.word().clone(), tag],
    )
}

#[derive(Clone, Copy, Debug, Default)]
struct ScanState {
    words: [u64; 8],
    processed: u32,
}
fn state_digest_native(context: &ResultScanContext, cursor: u32, state: &ScanState) -> Fp {
    let mut words = vec![
        Fp::from(RESULT_SCAN_PROGRAM),
        context.digest(),
        Fp::from(u64::from(cursor)),
        Fp::ONE,
        context.root,
        Fp::from(u64::from(context.frame_len)),
        Fp::from(u64::from(context.frame_len) + RESULT_TAG.len() as u64),
        Fp::from(u64::from(state.processed)),
    ];
    words.extend(state.words.map(Fp::from));
    hash_with_domain(RESULT_SCAN_STATE, &words)
}
fn state_digest_cells(
    chip: &mut VerifierChip<Ep>,
    blake: &mut Blake2bChip<'_, Fp>,
    region: &mut Region<'_, Fp>,
    context: &Word<Fp>,
    cursor: &Uint<Fp, 32>,
    stream: &TapeResultHashStream,
) -> Result<Word<Fp>, Error> {
    let program = chip
        .uint()
        .glue()
        .constant(region, Fp::from(RESULT_SCAN_PROGRAM))?;
    let tag = chip.uint().glue().constant(region, Fp::ONE)?;
    let total = chip.uint().checked_add_constant(
        region,
        stream.tape().frame_len(),
        RESULT_TAG.len() as u128,
    )?;
    let mut words = vec![
        program,
        context.clone(),
        cursor.word().clone(),
        tag,
        stream.tape().root().clone(),
        stream.tape().frame_len().word().clone(),
        total.word().clone(),
        stream.stream().processed().word().clone(),
    ];
    words.extend(
        blake
            .state_words(region, stream.stream().state())?
            .into_iter()
            .map(|word| word.word().clone()),
    );
    chip.hash_words(region, RESULT_SCAN_STATE, &words)
}

/// One bounded scan leaf. Native trace generation supplies witnesses only;
/// synthesis derives each output from the full constrained input state.
#[derive(Clone, Debug)]
pub struct ResultScanCircuit {
    plan: ResultScanPlan,
    cursor: u32,
    context: ResultScanContext,
    before: ScanState,
    after: ScanState,
    tape: Arc<ResultTapeWitness>,
    known: bool,
}
impl ResultScanCircuit {
    /// Witnessless source layout for original-key qualification.
    /// # Errors
    /// Failure to initialize the bounded unknown tape witness.
    pub fn for_source(plan: ResultScanPlan) -> Result<Self, Error> {
        Ok(Self {
            plan,
            cursor: 0,
            context: ResultScanContext {
                root: Fp::ZERO,
                frame_len: 0,
                expected: [0; 32],
            },
            before: ScanState::default(),
            after: ScanState::default(),
            tape: Arc::new(ResultTapeWitness::from_frame(&Value::unknown())?),
            known: false,
        })
    }
    /// Fixed operation whose original circuit tables qualify this leaf.
    pub const fn plan(&self) -> ResultScanPlan {
        self.plan
    }
    /// Untrusted context, to be joined to the consensus and parsing sources.
    pub const fn context(&self) -> &ResultScanContext {
        &self.context
    }
    /// Exact endpoint openings carried by the source wrapper.
    pub fn endpoints(&self) -> [Fp; 6] {
        let start = self.cursor;
        let before = if self.plan == ResultScanPlan::Start {
            self.context.boundary_digest(false)
        } else {
            state_digest_native(&self.context, start, &self.before)
        };
        let after = if self.plan == ResultScanPlan::Finish {
            self.context.boundary_digest(true)
        } else {
            state_digest_native(&self.context, start + 1, &self.after)
        };
        [
            Fp::from(RESULT_SCAN_PROGRAM),
            self.context.digest(),
            Fp::from(u64::from(start)),
            Fp::from(u64::from(start) + 1),
            before,
            after,
        ]
    }
    /// Exact A69 frame with explicit trivial leaf accumulation claims.
    /// # Errors
    /// Invalid internal generator encoding or endpoint geometry.
    pub fn instances(&self) -> Result<Vec<Vec<Fp>>, Error> {
        Ok(vec![leaf_frame_native(self.endpoints())?.to_vec()])
    }
    fn value<T>(&self, value: T) -> Value<T> {
        if self.known {
            Value::known(value)
        } else {
            Value::unknown()
        }
    }
}

/// Source layout with shared verifier lanes and one exclusive Blake chip.
#[derive(Clone, Debug)]
pub struct ResultScanConfig {
    verifier: VerifierConfig<Ep>,
    blake: Blake2bConfig,
    public: Column<Instance>,
    plan: ResultScanPlan,
}
impl ResultScanCircuit {
    // Every original transition is shared by arithmetic component checks and
    // the installed paired source; no completed padding operation is skipped.
    fn assign_transition(
        &self,
        chip: &mut VerifierChip<Ep>,
        blake: &mut Blake2bChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
    ) -> Result<SourceEndpoints, Error> {
        let root = chip
            .uint()
            .glue()
            .witness(region, self.value(self.context.root))?;
        let length = chip
            .uint()
            .assign::<32>(region, self.value(u128::from(self.context.frame_len)))?;
        let expected: [Word<Fp>; 32] = chip
            .uint()
            .glue()
            .witnesses(
                region,
                &self
                    .context
                    .expected
                    .map(|byte| self.value(Fp::from(u64::from(byte)))),
            )?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let context = context_digest_cells(chip, region, &root, &length, &expected)?;
        let tape = ResultTape::new(&mut chip.uint(), region, &root, &length)?;
        let (start, end, before, after) = if self.plan == ResultScanPlan::Start {
            let start = chip.uint().constant::<32>(region, 0)?;
            let end = chip.uint().constant::<32>(region, 1)?;
            let before = boundary_digest_cells(chip, region, &context, &start, false)?;
            let stream = TapeResultHashStream::start(&mut chip.uint(), blake, region, &tape)?;
            let after = state_digest_cells(chip, blake, region, &context, &end, &stream)?;
            (start, end, before, after)
        } else {
            let processed = chip
                .uint()
                .assign::<32>(region, self.value(u128::from(self.before.processed)))?;
            let words: [Word<Fp>; 8] = chip
                .uint()
                .glue()
                .witnesses(
                    region,
                    &self.before.words.map(|word| self.value(Fp::from(word))),
                )?
                .try_into()
                .map_err(|_| Error::Synthesis)?;
            let stream = TapeResultHashStream::resume(
                &mut chip.uint(),
                blake,
                region,
                &tape,
                &processed,
                &words,
            )?;
            if self.plan == ResultScanPlan::Absorb {
                let start = chip
                    .uint()
                    .assign::<32>(region, self.value(u128::from(self.cursor)))?;
                let one = chip.uint().constant::<32>(region, 1)?;
                let maximum = chip
                    .uint()
                    .constant::<32>(region, u128::from(RESULT_SCAN_BLOCKS))?;
                chip.uint().assert_le(region, &one, &start)?;
                chip.uint().assert_le(region, &start, &maximum)?;
                let block = chip.uint().checked_sub(region, &start, &one)?;
                let offset = chip.uint().glue().linear(
                    region,
                    &[(Fp::from(128), block.word())],
                    Fp::ZERO,
                )?;
                let offset = chip.uint().range_check::<32>(region, &offset)?;
                let total =
                    chip.uint()
                        .checked_add_constant(region, &length, RESULT_TAG.len() as u128)?;
                let active = chip.uint().lt(region, &offset, &total)?;
                let expected_progress =
                    chip.uint()
                        .glue()
                        .select(region, &active, offset.word(), total.word())?;
                GlueChip::assert_equal(region, &expected_progress, processed.word())?;
                let end = chip.uint().checked_add_constant(region, &start, 1)?;
                let before = state_digest_cells(chip, blake, region, &context, &start, &stream)?;
                let mut openings = Vec::with_capacity(4);
                for i in 0..4 {
                    openings.push(
                        self.tape.opening(
                            chip.uint().glue(),
                            region,
                            processed
                                .value()
                                .zip(total.value())
                                .map(|(n, total)| if n < total { n / 32 + i } else { i }),
                        )?,
                    );
                }
                let openings = openings.try_into().map_err(|_| Error::Synthesis)?;
                let (mut uint, hash) = chip.uint_and_hasher()?;
                let next = stream.absorb_padded(&mut uint, hash, blake, region, &openings)?;
                let after = state_digest_cells(chip, blake, region, &context, &end, &next)?;
                (start, end, before, after)
            } else {
                let end = end_cursor_cells(chip, region, &length)?;
                let one = chip.uint().constant::<32>(region, 1)?;
                let start = chip.uint().checked_sub(region, &end, &one)?;
                let before = state_digest_cells(chip, blake, region, &context, &start, &stream)?;
                let digest = stream.finish(blake, region)?;
                for (actual, expected) in digest.bytes().iter().zip(&expected) {
                    GlueChip::assert_equal(region, actual, expected)?;
                }
                let after = boundary_digest_cells(chip, region, &context, &end, true)?;
                (start, end, before, after)
            }
        };
        let program = chip
            .uint()
            .glue()
            .constant(region, Fp::from(RESULT_SCAN_PROGRAM))?;
        let endpoints = SourceEndpoints::from_words(
            chip,
            region,
            &[
                program,
                context,
                start.word().clone(),
                end.word().clone(),
                before,
                after,
            ],
        )?;
        Ok(endpoints)
    }
}
impl Circuit<Fp> for ResultScanCircuit {
    type Config = ResultScanConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ResultScanPlan;
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn params(&self) -> Self::Params {
        self.plan
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        Self::configure_with_params(meta, ResultScanPlan::Start)
    }
    fn configure_with_params(
        meta: &mut ConstraintSystem<Fp>,
        plan: ResultScanPlan,
    ) -> Self::Config {
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, 3)
            .expect("fixed three-bank source profile");
        let advice = core::array::from_fn(|_| meta.advice_column());
        let constants = meta.fixed_column();
        let blake = Blake2bConfig::configure(meta, advice, constants);
        let public = meta.instance_column(69);
        meta.enable_equality(public);
        ResultScanConfig {
            verifier,
            blake,
            public,
            plan,
        }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        if self.plan != config.plan {
            return Err(Error::Synthesis);
        }
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let mut blake = Blake2bChip::new(&config.blake);
        let frame = layouter.assign_region(
            || "complete result hash source",
            |mut region| {
                let endpoints = self.assign_transition(&mut chip, &mut blake, &mut region)?;
                SourceCheckpoint::leaf(&mut chip, &mut region, endpoints)?
                    .frame(&mut chip, &mut region)
            },
        )?;
        for (i, word) in frame.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

mod batch;
pub use batch::{
    RESULT_BATCH_LENGTH, ResultScanBatchCircuit, ResultScanBatchPlan, prepare_result_batches,
};
