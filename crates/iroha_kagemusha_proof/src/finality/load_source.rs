//! Bounded ordinary Load receipt, exact typed event and counted inclusion source.
//!
//! All 35 leaves are required: parse the original 282-byte receipt, advance 32
//! canonical path slots, read the original result prefix and close the root.
//! Each endpoint commits the complete live path and unchanged source context.
//! The finality owner must also join the complete result scan of this exact tape
//! and length, its certificate and the genesis-rooted validator schedule. Neither
//! a native witness proposal nor this inclusion program grants Load authority.

use std::sync::Arc;

use ff::{Field, PrimeField};
use iroha_pasta::{Ep, Fp, poseidon::hash_with_domain};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
};
use iroha_plonk_gadgets::{
    GlueChip, Uint, Word,
    blake2b::{Blake2bChip, Blake2bConfig},
    bytes::tape::{BytesChip, BytesConfig},
    merkle::PathCells,
};
use iroha_plonk_recursion::verifier::{VerifierChip, VerifierConfig};

use super::{
    LoadEventCells, LoadReceiptCells,
    continuity::{SourceCheckpoint, SourceEndpoints, leaf_frame_native},
    result::{MAX_RESULT_BYTES, RESULT_TAG, ResultPrefix},
    schedule::tape::{ResultTape, ResultTapeWitness},
};

mod witness;
pub use witness::prepare_load_source;

/// Fixed ordinary Load inclusion program identity.
pub const PROGRAM_ID: u64 = u64::from_le_bytes(*b"kgwldsp1");
/// Start, 32 counted path slots, original result prefix, and closure.
pub const PROGRAM_LENGTH: u32 = 35;
/// Domain committing every original receipt term and result-source proposal.
pub const CONTEXT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwldsc1");
/// Domain committing the complete live counted path and program cursor.
pub const STATE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwldss1");
const RECEIPT_WORDS: usize = 18;

/// Original receipt terms proposed by a witness generator. The Start circuit
/// binds every field to the same constrained canonical receipt transcript.
#[derive(Clone, Copy, Debug, Default)]
pub struct LoadReceiptProjection {
    /// Canonical packed-byte receipt identity.
    pub digest: Fp,
    /// Scheme identity.
    pub scheme: [u8; 32],
    /// Asset identity.
    pub asset: [u8; 32],
    /// Offline wallet identity.
    pub wallet: [u8; 32],
    /// Original request identity.
    pub request: [u8; 32],
    /// Original successful signed transaction hash.
    pub transaction: [u8; 32],
    /// Digest of the original canonical payer account.
    pub payer: [u8; 32],
    /// Exact accepted ordinal, whose successor must fit u128.
    pub ordinal: u128,
    /// Positive offline principal.
    pub amount: u128,
    /// Additional online charge.
    pub online_charge: u128,
    /// Canonical quote identity, zero exactly when the online charge is zero.
    pub charge_quote: Fp,
    /// Original successful block height, at least two.
    pub height: u64,
}
impl LoadReceiptProjection {
    fn words(&self) -> [Fp; RECEIPT_WORDS] {
        let mut out = [Fp::ZERO; RECEIPT_WORDS];
        out[0] = self.digest;
        for (i, identity) in [
            self.scheme,
            self.asset,
            self.wallet,
            self.request,
            self.transaction,
            self.payer,
        ]
        .iter()
        .enumerate()
        {
            out[1 + 2 * i] = pack(&identity[..16]);
            out[2 + 2 * i] = pack(&identity[16..]);
        }
        out[13..].copy_from_slice(&[
            Fp::from_u128(self.ordinal),
            Fp::from_u128(self.amount),
            Fp::from_u128(self.online_charge),
            self.charge_quote,
            Fp::from(self.height),
        ]);
        out
    }
}
fn pack(bytes: &[u8]) -> Fp {
    bytes
        .iter()
        .rev()
        .fold(Fp::ZERO, |n, b| n * Fp::from(256) + Fp::from(u64::from(*b)))
}
fn receipt_words(receipt: &LoadReceiptCells) -> [Word<Fp>; RECEIPT_WORDS] {
    let ids = [
        receipt.scheme(),
        receipt.asset(),
        receipt.wallet(),
        receipt.request(),
        receipt.transaction(),
        receipt.payer(),
    ];
    core::array::from_fn(|i| match i {
        0 => receipt.digest().clone(),
        1..=12 => ids[(i - 1) / 2][(i - 1) % 2].clone(),
        13 => receipt.ordinal().word().clone(),
        14 => receipt.amount().word().clone(),
        15 => receipt.online_charge().word().clone(),
        16 => receipt.charge_quote().clone(),
        _ => receipt.height().word().clone(),
    })
}

/// Untrusted immutable input shared by every inclusion source leaf.
#[derive(Clone, Copy, Debug)]
pub struct LoadSourceContext {
    /// Same complete byte tape as the result scan and epoch parser.
    pub result_root: Fp,
    /// Exact canonical result frame length, excluding its hash-domain prefix.
    pub result_frame_len: u32,
    /// Every original receipt term, all checked by Start.
    pub receipt: LoadReceiptProjection,
    /// Native counted event root, checked against the exact result prefix.
    pub event_root: [u8; 32],
    /// Exact nonzero event count, at most 2^32 for this native path capacity.
    pub event_count: u64,
    /// Exact u32 leaf index within the event sequence.
    pub event_index: u32,
}
impl LoadSourceContext {
    /// Compute the context commitment for witness generation, not authority.
    pub fn digest(&self) -> Fp {
        let mut words = vec![
            Fp::from(PROGRAM_ID),
            self.result_root,
            Fp::from(u64::from(self.result_frame_len)),
        ];
        words.extend(self.receipt.words());
        words.extend(self.event_root.map(|b| Fp::from(u64::from(b))));
        words.extend([
            Fp::from(self.event_count),
            Fp::from(u64::from(self.event_index)),
        ]);
        hash_with_domain(CONTEXT_DOMAIN, &words)
    }
    /// Fixed empty initial or terminal state; all intervening leaves are required.
    pub fn boundary_digest(&self, terminal: bool) -> Fp {
        hash_with_domain(
            STATE_DOMAIN,
            &[
                Fp::from(PROGRAM_ID),
                self.digest(),
                Fp::from(if terminal {
                    u64::from(PROGRAM_LENGTH)
                } else {
                    0
                }),
                Fp::from(if terminal { 2 } else { 0 }),
            ],
        )
    }
}

fn digest_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    tape: &ResultTape,
    receipt: &[Word<Fp>; RECEIPT_WORDS],
    root: &[Word<Fp>; 32],
    count: &Uint<Fp, 64>,
    index: &Uint<Fp, 32>,
) -> Result<Word<Fp>, Error> {
    let program = chip.uint().glue().constant(region, Fp::from(PROGRAM_ID))?;
    let mut words = vec![
        program,
        tape.root().clone(),
        tape.frame_len().word().clone(),
    ];
    words.extend_from_slice(receipt);
    for byte in root {
        chip.uint().range_check::<8>(region, byte)?;
        words.push(byte.clone());
    }
    let max = chip.uint().constant::<64>(region, 1_u128 << 32)?;
    chip.uint().assert_le(region, count, &max)?;
    let wide = iroha_plonk_gadgets::UintChip::widen(index);
    chip.uint().assert_lt(region, &wide, count)?;
    words.extend([count.word().clone(), index.word().clone()]);
    chip.hash_words(region, CONTEXT_DOMAIN, &words)
}

/// Bind the source context to the owner's exact parsed receipt and source tape.
/// The enclosing owner must authenticate the complete Start..Finish interval,
/// and join the complete result scan of this same tape root and frame length.
/// # Errors
/// Layout errors; invalid count, index, byte ranges or result length fail.
pub fn context_digest_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    tape: &ResultTape,
    receipt: &LoadReceiptCells,
    root: &[Word<Fp>; 32],
    count: &Uint<Fp, 64>,
    index: &Uint<Fp, 32>,
) -> Result<Word<Fp>, Error> {
    digest_cells(
        chip,
        region,
        tape,
        &receipt_words(receipt),
        root,
        count,
        index,
    )
}

/// Recompute the fixed initial or terminal state from the owner's context.
/// # Errors
/// Layout errors.
pub fn boundary_digest_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    context: &Word<Fp>,
    terminal: bool,
) -> Result<Word<Fp>, Error> {
    let program = chip.uint().glue().constant(region, Fp::from(PROGRAM_ID))?;
    let cursor = chip.uint().glue().constant(
        region,
        Fp::from(if terminal {
            u64::from(PROGRAM_LENGTH)
        } else {
            0
        }),
    )?;
    let tag = chip
        .uint()
        .glue()
        .constant(region, Fp::from(if terminal { 2 } else { 0 }))?;
    chip.hash_words(
        region,
        STATE_DOMAIN,
        &[program, context.clone(), cursor, tag],
    )
}

/// Fixed arithmetic classes. Path has one layout for all 32 dynamic cursors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoadSourcePlan {
    /// Parse the original receipt and derive its typed event and Merkle leaf.
    #[default]
    Start,
    /// One exact counted path slot, with zero padding after reaching the root.
    Path,
    /// Open and parse the original result's fixed header and event commitment.
    Prefix,
    /// Require the exact terminal root and clear the fully consumed path state.
    Finish,
}
#[derive(Clone, Copy, Debug, Default)]
struct PathState {
    index: u32,
    width: u64,
    digest: [u8; 32],
}
fn state_native(context: &LoadSourceContext, cursor: u32, state: &PathState) -> Fp {
    let mut words = vec![
        Fp::from(PROGRAM_ID),
        context.digest(),
        Fp::from(u64::from(cursor)),
        Fp::ONE,
        Fp::from(u64::from(state.index)),
        Fp::from(state.width),
    ];
    words.extend(state.digest.map(|b| Fp::from(u64::from(b))));
    hash_with_domain(STATE_DOMAIN, &words)
}
fn state_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    context: &Word<Fp>,
    cursor: &Uint<Fp, 32>,
    path: &PathCells<Fp>,
) -> Result<Word<Fp>, Error> {
    let program = chip.uint().glue().constant(region, Fp::from(PROGRAM_ID))?;
    let tag = chip.uint().glue().constant(region, Fp::ONE)?;
    let mut words = vec![
        program,
        context.clone(),
        cursor.word().clone(),
        tag,
        path.geometry().index().word().clone(),
        path.geometry().width().word().clone(),
    ];
    words.extend_from_slice(path.digest());
    chip.hash_words(region, STATE_DOMAIN, &words)
}

/// One source-qualified arithmetic layout with untrusted complete state openings.
#[derive(Clone, Debug)]
pub struct LoadSourceCircuit {
    plan: LoadSourcePlan,
    context: LoadSourceContext,
    cursor: u32,
    before: PathState,
    after: PathState,
    sibling: [u8; 32],
    receipt: [u8; LoadReceiptCells::BYTES],
    tape: Arc<ResultTapeWitness>,
    known: bool,
}
impl LoadSourceCircuit {
    /// Witnessless original layout for installation-time source qualification.
    /// # Errors
    /// Failure to construct the fixed unknown tape witness.
    pub fn for_source(plan: LoadSourcePlan) -> Result<Self, Error> {
        Ok(Self {
            plan,
            context: LoadSourceContext {
                result_root: Fp::ZERO,
                result_frame_len: 0,
                receipt: LoadReceiptProjection::default(),
                event_root: [0; 32],
                event_count: 0,
                event_index: 0,
            },
            cursor: 0,
            before: PathState::default(),
            after: PathState::default(),
            sibling: [0; 32],
            receipt: [0; LoadReceiptCells::BYTES],
            tape: Arc::new(ResultTapeWitness::from_frame(&Value::unknown())?),
            known: false,
        })
    }
    /// The original fixed source class.
    pub const fn plan(&self) -> LoadSourcePlan {
        self.plan
    }
    /// Complete unchanged context proposal.
    pub const fn context(&self) -> &LoadSourceContext {
        &self.context
    }
    /// Native endpoint prediction, independently constrained during synthesis.
    pub fn endpoints(&self) -> [Fp; 6] {
        let before = if self.plan == LoadSourcePlan::Start {
            self.context.boundary_digest(false)
        } else {
            state_native(&self.context, self.cursor, &self.before)
        };
        let after = if self.plan == LoadSourcePlan::Finish {
            self.context.boundary_digest(true)
        } else {
            state_native(&self.context, self.cursor + 1, &self.after)
        };
        [
            Fp::from(PROGRAM_ID),
            self.context.digest(),
            Fp::from(u64::from(self.cursor)),
            Fp::from(u64::from(self.cursor) + 1),
            before,
            after,
        ]
    }
    /// Exact 69-word source frame with explicit trivial leaf IPA obligations.
    /// # Errors
    /// Invalid fixed generator encoding or endpoint geometry.
    pub fn instances(&self) -> Result<Vec<Vec<Fp>>, Error> {
        Ok(vec![leaf_frame_native(self.endpoints())?.to_vec()])
    }
    fn value<T>(&self, v: T) -> Value<T> {
        if self.known {
            Value::known(v)
        } else {
            Value::unknown()
        }
    }
    fn words<const N: usize>(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        values: [Fp; N],
    ) -> Result<[Word<Fp>; N], Error> {
        chip.uint()
            .glue()
            .witnesses(region, &values.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| Error::Synthesis)
    }
}

/// Shared source verifier lanes, receipt byte tape and one `BLAKE2b` bank.
#[derive(Clone, Debug)]
pub struct LoadSourceConfig {
    verifier: VerifierConfig<Ep>,
    bytes: BytesConfig,
    blake: Blake2bConfig,
    public: Column<Instance>,
    plan: LoadSourcePlan,
}
impl Circuit<Fp> for LoadSourceCircuit {
    type Config = LoadSourceConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = LoadSourcePlan;
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
        Self::configure_with_params(meta, LoadSourcePlan::Start)
    }
    fn configure_with_params(
        meta: &mut ConstraintSystem<Fp>,
        plan: LoadSourcePlan,
    ) -> Self::Config {
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, 3)
            .expect("fixed three-bank source profile");
        let primary = meta.advice_column();
        let secondary = meta.advice_column();
        let bytes = BytesConfig::configure(meta, primary, secondary);
        let advice = core::array::from_fn(|_| meta.advice_column());
        let constants = meta.fixed_column();
        let blake = Blake2bConfig::configure(meta, advice, constants);
        let public = meta.instance_column(69);
        meta.enable_equality(public);
        LoadSourceConfig {
            verifier,
            bytes,
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
        let mut bytes = BytesChip::new(config.bytes);
        bytes.load_table(&mut layouter)?;
        let mut blake = Blake2bChip::new(&config.blake);
        let frame = layouter.assign_region(
            || "ordinary Load inclusion source",
            |mut region| {
                let root = chip
                    .uint()
                    .glue()
                    .witness(&mut region, self.value(self.context.result_root))?;
                let length = chip.uint().assign::<32>(
                    &mut region,
                    self.value(u128::from(self.context.result_frame_len)),
                )?;
                let tape = ResultTape::new(&mut chip.uint(), &mut region, &root, &length)?;
                let projected = self.words(&mut chip, &mut region, self.context.receipt.words())?;
                let event_root = self.words(
                    &mut chip,
                    &mut region,
                    self.context.event_root.map(|b| Fp::from(u64::from(b))),
                )?;
                let count = chip.uint().assign::<64>(
                    &mut region,
                    self.value(u128::from(self.context.event_count)),
                )?;
                let index = chip.uint().assign::<32>(
                    &mut region,
                    self.value(u128::from(self.context.event_index)),
                )?;
                let context = digest_cells(
                    &mut chip,
                    &mut region,
                    &tape,
                    &projected,
                    &event_root,
                    &count,
                    &index,
                )?;
                let cursor = chip
                    .uint()
                    .assign::<32>(&mut region, self.value(u128::from(self.cursor)))?;
                let end = chip.uint().checked_add_constant(&mut region, &cursor, 1)?;
                let (before, after) = if self.plan == LoadSourcePlan::Start {
                    GlueChip::assert_constant(&mut region, cursor.word(), Fp::ZERO)?;
                    let input = self.receipt.map(|b| self.value(b));
                    let run = bytes.run(
                        &mut region,
                        &input,
                        &LoadReceiptCells::primary_segments(),
                        &LoadReceiptCells::secondary_segments(),
                    )?;
                    let (mut uint, hash) = chip.uint_and_hasher()?;
                    let receipt = LoadReceiptCells::from_run(&mut uint, hash, &mut region, &run)?;
                    for (original, proposed) in receipt_words(&receipt).iter().zip(&projected) {
                        GlueChip::assert_equal(&mut region, original, proposed)?;
                    }
                    let event = LoadEventCells::from_receipt(
                        &mut chip.uint(),
                        &mut blake,
                        &mut region,
                        &receipt,
                    )?;
                    let path = PathCells::start(
                        &mut blake,
                        &mut chip.uint(),
                        &mut region,
                        &index,
                        &count,
                        event.hash().bytes(),
                    )?;
                    (
                        boundary_digest_cells(&mut chip, &mut region, &context, false)?,
                        state_cells(&mut chip, &mut region, &context, &end, &path)?,
                    )
                } else {
                    let path_index = chip
                        .uint()
                        .assign::<32>(&mut region, self.value(u128::from(self.before.index)))?;
                    let width = chip
                        .uint()
                        .assign::<64>(&mut region, self.value(u128::from(self.before.width)))?;
                    let digest = self.words(
                        &mut chip,
                        &mut region,
                        self.before.digest.map(|b| Fp::from(u64::from(b))),
                    )?;
                    let path = PathCells::resume(
                        &mut chip.uint(),
                        &mut region,
                        &path_index,
                        &width,
                        &digest,
                    )?;
                    let before = state_cells(&mut chip, &mut region, &context, &cursor, &path)?;
                    let after = match self.plan {
                        LoadSourcePlan::Path => {
                            let first = chip.uint().constant::<32>(&mut region, 1)?;
                            let last = chip.uint().constant::<32>(&mut region, 32)?;
                            chip.uint().assert_le(&mut region, &first, &cursor)?;
                            chip.uint().assert_le(&mut region, &cursor, &last)?;
                            let sibling = self.words(
                                &mut chip,
                                &mut region,
                                self.sibling.map(|b| Fp::from(u64::from(b))),
                            )?;
                            let next = path.step_padded(
                                &mut blake,
                                &mut chip.uint(),
                                &mut region,
                                &sibling,
                            )?;
                            state_cells(&mut chip, &mut region, &context, &end, &next)?
                        }
                        LoadSourcePlan::Prefix => {
                            GlueChip::assert_constant(&mut region, cursor.word(), Fp::from(33))?;
                            path.finish(&mut region, &event_root)?;
                            let mut original = Vec::with_capacity(9 * 32);
                            for i in 0..9 {
                                let opening = self.tape.opening(
                                    chip.uint().glue(),
                                    &mut region,
                                    Value::known(i),
                                )?;
                                let chunk = chip.uint().constant::<12>(&mut region, i)?;
                                let (mut uint, hash) = chip.uint_and_hasher()?;
                                tape.open_chunk(&mut uint, hash, &mut region, &chunk, &opening)?;
                                original.extend_from_slice(&opening.bytes);
                            }
                            let minimum = chip.uint().constant::<32>(&mut region, 261)?;
                            chip.uint().assert_le(&mut region, &minimum, &length)?;
                            let prefix = ResultPrefix::parse(
                                &mut chip.uint(),
                                &mut region,
                                &length,
                                &original[RESULT_TAG.len()..],
                            )?;
                            GlueChip::assert_equal(
                                &mut region,
                                prefix.height.word(),
                                &projected[17],
                            )?;
                            GlueChip::assert_equal(
                                &mut region,
                                prefix.event_count.word(),
                                count.word(),
                            )?;
                            for (actual, expected) in prefix.event_root.iter().zip(&event_root) {
                                GlueChip::assert_equal(&mut region, actual, expected)?;
                            }
                            state_cells(&mut chip, &mut region, &context, &end, &path)?
                        }
                        LoadSourcePlan::Finish => {
                            GlueChip::assert_constant(&mut region, cursor.word(), Fp::from(34))?;
                            path.finish(&mut region, &event_root)?;
                            boundary_digest_cells(&mut chip, &mut region, &context, true)?
                        }
                        LoadSourcePlan::Start => return Err(Error::Synthesis),
                    };
                    (before, after)
                };
                let program = chip
                    .uint()
                    .glue()
                    .constant(&mut region, Fp::from(PROGRAM_ID))?;
                let endpoints = SourceEndpoints::from_words(
                    &mut chip,
                    &mut region,
                    &[
                        program,
                        context,
                        cursor.word().clone(),
                        end.word().clone(),
                        before,
                        after,
                    ],
                )?;
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
