//! Bounded source program for checked epoch fields and the exact ordered roster.
//!
//! Every continuation retains all live byte ranges and roster leaves. The
//! initial state is empty; only parsing the original result can create ranges.
//! The terminal owner must join the complete program to the same full-R scan,
//! context-hash source, authorized predecessor or genesis, quorum and signature.
//! Neither a proposed context nor an isolated parser leaf supplies authority.

use ff::Field;
use iroha_pasta::{Ep, Fp, poseidon::hash_with_domain};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
};
use iroha_plonk_gadgets::{GlueChip, Uint, Word, WordHasher};
use iroha_plonk_recursion::verifier::{VerifierChip, VerifierConfig};

use super::{
    authorization::AuthorizationRanges,
    context_hash::{self, ContextHashInput},
    decode::{FieldSpan, ScheduleReader},
    epoch::EpochRanges,
    graph::{BoundaryRanges, ScheduleRanges, SlotCells},
    tape::{ResultTape, ResultTapeWitness},
};
use crate::finality::{
    continuity::{SourceCheckpoint, SourceEndpoints, leaf_frame_native},
    roster::{key_leaf_at_cells, key_leaf_cells, key_node_cells},
};

/// Fixed ordered source identity; no host evaluator can authorize its outputs.
pub const PROGRAM_ID: u64 = u64::from_le_bytes(*b"kgwepsp1");
/// Ten parser stages, 31 exact seats, one tree closure and one terminal cleanup.
pub const PROGRAM_LENGTH: u32 = 43;
/// Domain binding the exact result, selected context and all extracted claims.
pub const CONTEXT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwepsc1");
/// Domain committing every retained source range and roster leaf.
pub const STATE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwepss1");
/// Height, four graph spans, selected body, seven epoch spans, eleven
/// authorization spans, and the complete 32-leaf roster accumulator.
pub const STATE_WORDS: usize = 102;
const GRAPH: usize = 1;
const BODY: usize = 13;
const EPOCH: usize = 16;
const AUTHORIZATION: usize = 37;
const ROSTER: usize = 70;
const SCALARS: usize = 27;
const IDENTITIES: usize = 11;

/// Original scalar and identity claims from one selected epoch and its schedule.
/// These are untrusted proposals; the complete parser program checks every field.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScheduleProjection {
    /// Exact active normal-validator count and fault bound.
    pub members: u8,
    /// Exact positive equal-vote fault bound.
    pub faults: u8,
    /// Native permissioned/NPoS policy tag.
    pub mode: u8,
    /// Native scheduling epoch number.
    pub epoch: u64,
    /// First governed height, inclusive.
    pub first: u64,
    /// Last governed height, inclusive.
    pub last: u64,
    /// Native validator generation number.
    pub generation: u64,
    /// Native genesis/activate/retain/retain-and-cancel tag.
    pub decision: u8,
    /// Native genesis-derived network identity.
    pub network: [u8; 32],
    /// Exact epoch leader seed.
    pub seed: [u8; 32],
    /// Validator generation, predecessor authorization, transition attempt IDs.
    pub authorization_ids: [[u8; 32]; 3],
    /// Whether the authenticated authorization installs a beacon.
    pub beacon_installed: bool,
    /// Exact installed session and transcript, zero for Bootstrap.
    pub beacon_ids: [[u8; 32]; 2],
    /// Presence of the original schedule's boundary outcome.
    pub boundary_present: bool,
    /// Original boundary height or zero when absent.
    pub boundary_height: u64,
    /// Predecessor context ID and selection anchor, zero when absent.
    pub boundary_ids: [[u8; 32]; 2],
    /// Exact h+1 and h+2 original slot projections.
    pub slots: [SlotProjection; 2],
}
/// Original compact successor slot; its height is derived from the result height.
#[derive(Clone, Copy, Debug, Default)]
pub struct SlotProjection {
    /// Native `PendingBoundary` tag.
    pub pending: bool,
    /// Pending boundary height or zero for Ready.
    pub boundary_height: u64,
    /// Pending predecessor identity or zero for Ready.
    pub predecessor: [u8; 32],
    /// Block/retry/execution/apply times, max payload bytes, epoch length.
    pub parameters: [u64; 6],
}
impl ScheduleProjection {
    fn scalars(&self) -> [u64; SCALARS] {
        let mut out = [0; SCALARS];
        out[..11].copy_from_slice(&[
            u64::from(self.members),
            u64::from(self.faults),
            u64::from(self.mode),
            self.epoch,
            self.first,
            self.last,
            self.generation,
            u64::from(self.decision),
            u64::from(self.boundary_present),
            self.boundary_height,
            u64::from(self.beacon_installed),
        ]);
        for (slot, start) in self.slots.iter().zip([11, 19]) {
            out[start] = u64::from(slot.pending);
            out[start + 1] = slot.boundary_height;
            out[start + 2..start + 8].copy_from_slice(&slot.parameters);
        }
        out
    }
    fn identities(&self) -> [[u8; 32]; IDENTITIES] {
        [
            self.network,
            self.seed,
            self.authorization_ids[0],
            self.authorization_ids[1],
            self.authorization_ids[2],
            self.beacon_ids[0],
            self.beacon_ids[1],
            self.boundary_ids[0],
            self.boundary_ids[1],
            self.slots[0].predecessor,
            self.slots[1].predecessor,
        ]
    }
}
/// Exact joined source context. The native hash source proves `epoch_hash`;
/// this parser proves which original context it names and extracts the roster.
#[derive(Clone, Copy, Debug)]
pub struct ScheduleSourceInput {
    /// Same root, length and selected body as the complete epoch-hash program.
    pub epoch_hash: ContextHashInput,
    /// Exact current result height.
    pub height: u64,
    /// False selects current; true selects the boundary-authorized successor
    /// when present and the current context otherwise.
    pub authorized: bool,
    /// Ordered 32-leaf key root shared with the normal-quorum aggregation source.
    pub roster_root: Fp,
    /// Every original metadata projection checked by this parser program.
    pub projection: ScheduleProjection,
}
fn pack_native(bytes: &[u8]) -> Fp {
    bytes
        .iter()
        .rev()
        .fold(Fp::ZERO, |n, b| n * Fp::from(256) + Fp::from(u64::from(*b)))
}
impl ScheduleSourceInput {
    /// Commitment for witness generation, not native schedule authority.
    pub fn digest(&self) -> Fp {
        let mut words = vec![
            Fp::from(PROGRAM_ID),
            self.epoch_hash.digest(),
            Fp::from(self.height),
            Fp::from(u64::from(self.authorized)),
            self.roster_root,
        ];
        words.extend(self.projection.scalars().map(Fp::from));
        for identity in self.projection.identities() {
            words.extend(identity.chunks_exact(16).map(pack_native));
        }
        hash_with_domain(CONTEXT_DOMAIN, &words)
    }
}
fn state_native(context: Fp, cursor: u32, words: &[Fp; STATE_WORDS]) -> Fp {
    let mut values = vec![Fp::from(PROGRAM_ID), context, Fp::from(u64::from(cursor))];
    values.extend(words);
    hash_with_domain(STATE_DOMAIN, &values)
}
/// Exact empty initial or terminal state; every source leaf must be joined.
pub fn boundary_digest_native(input: &ScheduleSourceInput, terminal: bool) -> Fp {
    state_native(
        input.digest(),
        if terminal { PROGRAM_LENGTH } else { 0 },
        &[Fp::ZERO; STATE_WORDS],
    )
}
/// One fixed arithmetic class. Roster seat is a constrained dynamic cursor so
/// all 31 ordered seats use the same witnessless layout and pinned key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleSourceStage {
    /// Canonical R and schedule containers.
    Graph,
    /// Exact optional boundary and selected original epoch body.
    Boundary,
    /// Seven exact native epoch fields.
    Epoch,
    /// Version, network, mode, seed, count and fault bound.
    Header,
    /// Eleven exact native authorization fields.
    Authorization,
    /// Original scheduling epoch, height bounds, generation and decision.
    Scalars,
    /// Original network and authorization identities.
    Identities,
    /// Original Bootstrap/Installed beacon binding.
    Beacon,
    /// Original h+1 slot and parameters.
    Next,
    /// Original h+2 slot and parameters.
    AfterNext,
    /// One exact ordered roster seat selected by the program cursor.
    Member,
    /// Exact fixed 32-leaf roster closure.
    Roster,
    /// Clear all retained ranges only after completing every preceding stage.
    Terminal,
}
impl ScheduleSourceStage {
    const fn first_cursor(self) -> u32 {
        match self {
            Self::Graph => 0,
            Self::Boundary => 1,
            Self::Epoch => 2,
            Self::Header => 3,
            Self::Authorization => 4,
            Self::Scalars => 5,
            Self::Identities => 6,
            Self::Beacon => 7,
            Self::Next => 8,
            Self::AfterNext => 9,
            Self::Member => 10,
            Self::Roster => 41,
            Self::Terminal => 42,
        }
    }
    /// Select the fixed source stage for a program cursor, without granting authority.
    pub fn at(cursor: u32) -> Option<Self> {
        Some(match cursor {
            0 => Self::Graph,
            1 => Self::Boundary,
            2 => Self::Epoch,
            3 => Self::Header,
            4 => Self::Authorization,
            5 => Self::Scalars,
            6 => Self::Identities,
            7 => Self::Beacon,
            8 => Self::Next,
            9 => Self::AfterNext,
            10..=40 => Self::Member,
            41 => Self::Roster,
            42 => Self::Terminal,
            _ => return None,
        })
    }
}
/// One bounded parser transition with complete before/after witness openings.
#[derive(Clone, Debug)]
pub struct ScheduleSourceCircuit {
    stage: ScheduleSourceStage,
    cursor: u32,
    input: ScheduleSourceInput,
    before: [Fp; STATE_WORDS],
    after: [Fp; STATE_WORDS],
    frame: std::sync::Arc<[u8]>,
    known: bool,
}
/// Shared native verifier arithmetic/transcript lanes and source frame.
#[derive(Clone, Debug)]
pub struct ScheduleSourceConfig {
    verifier: VerifierConfig<Ep>,
    public: Column<Instance>,
}
impl ScheduleSourceCircuit {
    /// Witnessless fixed source layout for independent original-key import.
    /// No live result, decoded metadata, existing proof or checkpoint is needed.
    pub fn for_source(stage: ScheduleSourceStage) -> Self {
        Self {
            stage,
            cursor: stage.first_cursor(),
            input: ScheduleSourceInput {
                epoch_hash: ContextHashInput {
                    tape_root: Fp::ZERO,
                    result_len: 0,
                    payload_start: 0,
                    payload_len: 1,
                    checksum: 0,
                    context_id: [0; 32],
                },
                height: 0,
                authorized: false,
                roster_root: Fp::ZERO,
                projection: ScheduleProjection::default(),
            },
            before: [Fp::ZERO; STATE_WORDS],
            after: [Fp::ZERO; STATE_WORDS],
            frame: std::sync::Arc::from([]),
            known: false,
        }
    }
    /// Exact shared R, native-context and metadata proposal for terminal linkage.
    pub const fn input(&self) -> &ScheduleSourceInput {
        &self.input
    }

    /// Check allocation and fixed source shape. Proposed states carry no authority.
    /// # Errors
    /// Invalid cursor, source class or oversized original frame.
    pub fn new(
        stage: ScheduleSourceStage,
        cursor: u32,
        input: &ScheduleSourceInput,
        before: &[Fp; STATE_WORDS],
        after: &[Fp; STATE_WORDS],
        frame: Vec<u8>,
    ) -> Result<Self, Error> {
        if ScheduleSourceStage::at(cursor) != Some(stage) || frame.len() > 65_536 {
            return Err(Error::Synthesis);
        }
        Ok(Self {
            stage,
            cursor,
            input: *input,
            before: *before,
            after: *after,
            frame: frame.into(),
            known: true,
        })
    }
    /// Untrusted exact endpoint opening consumed by the qualified source wrapper.
    pub fn endpoints(&self) -> [Fp; 6] {
        let context = self.input.digest();
        [
            Fp::from(PROGRAM_ID),
            context,
            Fp::from(u64::from(self.cursor)),
            Fp::from(u64::from(self.cursor + 1)),
            state_native(context, self.cursor, &self.before),
            state_native(context, self.cursor + 1, &self.after),
        ]
    }
    /// Exact native internal source frame.
    /// # Errors
    /// Invalid fixed filler or endpoint encoding.
    pub fn instances(&self) -> Result<Vec<Vec<Fp>>, Error> {
        Ok(vec![leaf_frame_native(self.endpoints())?.to_vec()])
    }
    fn value<T: Clone>(&self, value: T) -> Value<T> {
        if self.known {
            Value::known(value)
        } else {
            Value::unknown()
        }
    }
}

/// Assigned untrusted claims linked to one recomputed source context.
/// Authentication requires complete qualified source proofs; assignment alone
/// proves only the scalar/byte ranges and exact commitment encodings.
#[derive(Clone, Debug)]
pub struct ScheduleSourceBinding {
    context: Word<Fp>,
    epoch_hash: Word<Fp>,
    context_id: [Word<Fp>; 32],
    checksum: Word<Fp>,
    tape: ResultTape,
    height: Uint<Fp, 64>,
    authorized: iroha_plonk_gadgets::Bit<Fp>,
    start: Word<Fp>,
    len: Word<Fp>,
    roster: Word<Fp>,
    scalars: [Word<Fp>; SCALARS],
    identities: [[Word<Fp>; 32]; IDENTITIES],
}
fn packed(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    bytes: &[Word<Fp>],
) -> Result<Word<Fp>, Error> {
    let mut out = chip.uint().glue().constant(region, Fp::ZERO)?;
    for byte in bytes.iter().rev() {
        chip.uint().range_check::<8>(region, byte)?;
        out = chip.uint().glue().linear(
            region,
            &[(Fp::from(256), &out), (Fp::ONE, byte)],
            Fp::ZERO,
        )?;
    }
    Ok(out)
}
impl ScheduleSourceBinding {
    /// Assign and range-check a proposal, deriving its complete parser and
    /// epoch-hash contexts in circuit. No host-computed digest is accepted.
    /// # Errors
    /// Layout errors; invalid byte/scalar spans or frame bounds are unsatisfied.
    pub fn assign(
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        input: &Value<ScheduleSourceInput>,
    ) -> Result<Self, Error> {
        let values = [
            input.map(|i| i.epoch_hash.tape_root),
            input.map(|i| Fp::from(u64::from(i.epoch_hash.result_len))),
            input.map(|i| Fp::from(u64::from(i.epoch_hash.payload_start))),
            input.map(|i| Fp::from(u64::from(i.epoch_hash.payload_len))),
            input.map(|i| Fp::from(i.epoch_hash.checksum)),
        ];
        let hash = chip.uint().glue().witnesses(region, &values)?;
        let id_values: [Value<Fp>; 32] = core::array::from_fn(|b| {
            input.map(|i| Fp::from(u64::from(i.epoch_hash.context_id[b])))
        });
        let id: [Word<Fp>; 32] = chip
            .uint()
            .glue()
            .witnesses(region, &id_values)?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let epoch_hash = context_hash::context_digest_cells(
            chip,
            region,
            context_hash::ContextHashInputCells {
                tape_root: &hash[0],
                result_len: &hash[1],
                payload_start: &hash[2],
                payload_len: &hash[3],
                checksum: &hash[4],
                context_id: &id,
            },
        )?;
        let result_len = chip.uint().range_check::<32>(region, &hash[1])?;
        let tape = ResultTape::new(&mut chip.uint(), region, &hash[0], &result_len)?;
        let height = chip
            .uint()
            .assign::<64>(region, input.map(|i| u128::from(i.height)))?;
        let authorized = chip
            .uint()
            .glue()
            .boolean(region, input.map(|i| i.authorized))?;
        let roster = chip
            .uint()
            .glue()
            .witness(region, input.map(|i| i.roster_root))?;
        let mut scalars = Vec::with_capacity(SCALARS);
        for field in 0..SCALARS {
            scalars.push(
                chip.uint()
                    .assign::<64>(
                        region,
                        input.map(|i| u128::from(i.projection.scalars()[field])),
                    )?
                    .word()
                    .clone(),
            );
        }
        let scalars: [Word<Fp>; SCALARS] = scalars.try_into().map_err(|_| Error::Synthesis)?;
        let mut identities = Vec::with_capacity(IDENTITIES);
        for identity in 0..IDENTITIES {
            let mut bytes = Vec::with_capacity(32);
            for byte in 0..32 {
                bytes.push(
                    chip.uint()
                        .assign::<8>(
                            region,
                            input.map(|i| u128::from(i.projection.identities()[identity][byte])),
                        )?
                        .word()
                        .clone(),
                );
            }
            identities.push(bytes.try_into().map_err(|_| Error::Synthesis)?);
        }
        let identities: [[Word<Fp>; 32]; IDENTITIES] =
            identities.try_into().map_err(|_| Error::Synthesis)?;
        let mut words = vec![
            chip.uint().glue().constant(region, Fp::from(PROGRAM_ID))?,
            epoch_hash.clone(),
            height.word().clone(),
            authorized.word().clone(),
            roster.clone(),
        ];
        words.extend(scalars.iter().cloned());
        for id in &identities {
            for half in id.chunks_exact(16) {
                words.push(packed(chip, region, half)?);
            }
        }
        let context = chip.hash_words(region, CONTEXT_DOMAIN, &words)?;
        Ok(Self {
            context,
            epoch_hash,
            context_id: id,
            checksum: hash[4].clone(),
            tape,
            height,
            authorized,
            start: hash[2].clone(),
            len: hash[3].clone(),
            roster,
            scalars,
            identities,
        })
    }
}

fn read_spans<const N: usize, H: WordHasher<Fp>>(
    reader: &mut ScheduleReader<'_, '_, H>,
    region: &mut Region<'_, Fp>,
    state: &[Word<Fp>; STATE_WORDS],
    start: usize,
) -> Result<[FieldSpan; N], Error> {
    let mut out = Vec::with_capacity(N);
    for i in 0..N {
        let words = core::array::from_fn(|j| state[start + 3 * i + j].clone());
        out.push(FieldSpan::from_source_words(
            reader.uint,
            region,
            reader.tape,
            &words,
        )?);
    }
    out.try_into().map_err(|_| Error::Synthesis)
}
fn graph<H: WordHasher<Fp>>(
    reader: &mut ScheduleReader<'_, '_, H>,
    region: &mut Region<'_, Fp>,
    state: &[Word<Fp>; STATE_WORDS],
) -> Result<ScheduleRanges, Error> {
    let height = reader.uint.range_check::<64>(region, &state[0])?;
    Ok(ScheduleRanges::from_source_parts(
        height,
        read_spans(reader, region, state, GRAPH)?,
    ))
}
fn epoch<H: WordHasher<Fp>>(
    reader: &mut ScheduleReader<'_, '_, H>,
    region: &mut Region<'_, Fp>,
    state: &[Word<Fp>; STATE_WORDS],
) -> Result<EpochRanges, Error> {
    let [body] = read_spans::<1, _>(reader, region, state, BODY)?;
    Ok(EpochRanges::from_source_parts(
        body,
        read_spans(reader, region, state, EPOCH)?,
    ))
}
fn authorization<H: WordHasher<Fp>>(
    reader: &mut ScheduleReader<'_, '_, H>,
    region: &mut Region<'_, Fp>,
    state: &[Word<Fp>; STATE_WORDS],
) -> Result<AuthorizationRanges, Error> {
    let [body] = read_spans::<1, _>(reader, region, state, BODY)?;
    Ok(AuthorizationRanges::from_source_parts(
        body,
        read_spans(reader, region, state, AUTHORIZATION)?,
    ))
}
fn write_spans<T: std::borrow::Borrow<FieldSpan>>(
    state: &mut [Word<Fp>; STATE_WORDS],
    start: usize,
    spans: impl IntoIterator<Item = T>,
) {
    for (i, span) in spans.into_iter().enumerate() {
        let span = span.borrow();
        state[start + 3 * i..start + 3 * i + 3].clone_from_slice(&span.source_words());
    }
}
fn equal_bytes(
    region: &mut Region<'_, Fp>,
    a: &[Word<Fp>; 32],
    b: &[Word<Fp>; 32],
) -> Result<(), Error> {
    for (a, b) in a.iter().zip(b) {
        GlueChip::assert_equal(region, a, b)?;
    }
    Ok(())
}
fn state_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    context: &Word<Fp>,
    cursor: &Uint<Fp, 32>,
    state: &[Word<Fp>; STATE_WORDS],
) -> Result<Word<Fp>, Error> {
    let mut words = vec![
        chip.uint().glue().constant(region, Fp::from(PROGRAM_ID))?,
        context.clone(),
        cursor.word().clone(),
    ];
    words.extend(state.iter().cloned());
    chip.hash_words(region, STATE_DOMAIN, &words)
}

impl Circuit<Fp> for ScheduleSourceCircuit {
    type Config = ScheduleSourceConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, 3)
            .expect("fixed parser source profile");
        let public = meta.instance_column(69);
        meta.enable_equality(public);
        ScheduleSourceConfig { verifier, public }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let witness = ResultTapeWitness::from_frame(&self.value(self.frame.to_vec()))?;
        let output = layouter.assign_region(
            || "bounded native schedule source",
            |mut region| {
                let cells =
                    ScheduleSourceBinding::assign(&mut chip, &mut region, &self.value(self.input))?;
                let cursor = chip
                    .uint()
                    .assign::<32>(&mut region, self.value(u128::from(self.cursor)))?;
                let next = chip.uint().checked_add_constant(&mut region, &cursor, 1)?;
                if self.stage == ScheduleSourceStage::Member {
                    let lo = chip.uint().constant::<32>(&mut region, 10)?;
                    let hi = chip.uint().constant::<32>(&mut region, 40)?;
                    chip.uint().assert_le(&mut region, &lo, &cursor)?;
                    chip.uint().assert_le(&mut region, &cursor, &hi)?;
                } else {
                    GlueChip::assert_constant(
                        &mut region,
                        cursor.word(),
                        Fp::from(u64::from(self.stage.first_cursor())),
                    )?;
                }
                let before: [Word<Fp>; STATE_WORDS] = chip
                    .uint()
                    .glue()
                    .witnesses(&mut region, &self.before.map(|v| self.value(v)))?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let mut after = before.clone();
                if self.stage == ScheduleSourceStage::Graph {
                    for word in &before {
                        GlueChip::assert_constant(&mut region, word, Fp::ZERO)?;
                    }
                }
                let member = {
                    let (mut uint, hash) = chip.uint_and_hasher()?;
                    let mut reader = ScheduleReader {
                        tape: &cells.tape,
                        witness: &witness,
                        uint: &mut uint,
                        hash,
                    };
                    match self.stage {
                        ScheduleSourceStage::Graph => {
                            let schedule = ScheduleRanges::parse(&mut reader, &mut region)?;
                            GlueChip::assert_equal(
                                &mut region,
                                schedule.height().word(),
                                cells.height.word(),
                            )?;
                            after[0] = schedule.height().word().clone();
                            write_spans(&mut after, GRAPH, schedule.source_parts());
                            None
                        }
                        ScheduleSourceStage::Boundary => {
                            let schedule = graph(&mut reader, &mut region, &before)?;
                            let boundary =
                                BoundaryRanges::parse(&mut reader, &mut region, &schedule)?;
                            GlueChip::assert_equal(
                                &mut region,
                                boundary.present().word(),
                                &cells.scalars[8],
                            )?;
                            GlueChip::assert_equal(
                                &mut region,
                                boundary.height().word(),
                                &cells.scalars[9],
                            )?;
                            equal_bytes(&mut region, boundary.predecessor(), &cells.identities[7])?;
                            equal_bytes(
                                &mut region,
                                boundary.selection_anchor(),
                                &cells.identities[8],
                            )?;
                            let authorized =
                                boundary.authorized(&mut reader, &mut region, &schedule)?;
                            let selected = reader.select_span(
                                &mut region,
                                &cells.authorized,
                                &authorized,
                                schedule.current(),
                            )?;
                            GlueChip::assert_constant(
                                &mut region,
                                selected.present().word(),
                                Fp::ONE,
                            )?;
                            GlueChip::assert_equal(
                                &mut region,
                                selected.start().word(),
                                &cells.start,
                            )?;
                            GlueChip::assert_equal(&mut region, selected.len().word(), &cells.len)?;
                            write_spans(&mut after, BODY, [&selected]);
                            None
                        }
                        ScheduleSourceStage::Epoch => {
                            let [body] =
                                read_spans::<1, _>(&mut reader, &mut region, &before, BODY)?;
                            let epoch = EpochRanges::parse(&mut reader, &mut region, &body)?;
                            write_spans(&mut after, EPOCH, epoch.source_parts());
                            None
                        }
                        ScheduleSourceStage::Header => {
                            let epoch = epoch(&mut reader, &mut region, &before)?;
                            let header = epoch.header(&mut reader, &mut region)?;
                            for (actual, expected) in [
                                header.count().word(),
                                header.faults().word(),
                                header.mode().word(),
                            ]
                            .into_iter()
                            .zip(&cells.scalars[..3])
                            {
                                GlueChip::assert_equal(&mut region, actual, expected)?;
                            }
                            equal_bytes(&mut region, header.network(), &cells.identities[0])?;
                            equal_bytes(&mut region, header.seed(), &cells.identities[1])?;
                            None
                        }
                        ScheduleSourceStage::Authorization => {
                            let epoch = epoch(&mut reader, &mut region, &before)?;
                            let authorization =
                                AuthorizationRanges::parse(&mut reader, &mut region, &epoch)?;
                            write_spans(&mut after, AUTHORIZATION, authorization.source_parts());
                            None
                        }
                        ScheduleSourceStage::Scalars => {
                            let authorization = authorization(&mut reader, &mut region, &before)?;
                            let scalars = authorization.scalars(&mut reader, &mut region)?;
                            for (actual, expected) in [
                                scalars.epoch().word(),
                                scalars.first().word(),
                                scalars.last().word(),
                                scalars.generation().word(),
                                scalars.decision().word(),
                            ]
                            .into_iter()
                            .zip(&cells.scalars[3..8])
                            {
                                GlueChip::assert_equal(&mut region, actual, expected)?;
                            }
                            // Current governs h; the selected successor governs h+1.
                            // This binds the selected context to its native interval.
                            let role = reader
                                .uint
                                .range_check::<64>(&mut region, cells.authorized.word())?;
                            let governed =
                                reader.uint.checked_add(&mut region, &cells.height, &role)?;
                            reader
                                .uint
                                .assert_le(&mut region, scalars.first(), &governed)?;
                            reader
                                .uint
                                .assert_le(&mut region, &governed, scalars.last())?;
                            None
                        }
                        ScheduleSourceStage::Identities => {
                            let authorization = authorization(&mut reader, &mut region, &before)?;
                            let identities = authorization.identities_for_source_network(
                                &mut reader,
                                &mut region,
                                &cells.identities[0],
                            )?;
                            for (actual, expected) in [
                                identities.authority(),
                                identities.previous(),
                                identities.transition(),
                            ]
                            .into_iter()
                            .zip(&cells.identities[2..5])
                            {
                                equal_bytes(&mut region, actual, expected)?;
                            }
                            None
                        }
                        ScheduleSourceStage::Beacon => {
                            let authorization = authorization(&mut reader, &mut region, &before)?;
                            let beacon = authorization.beacon(&mut reader, &mut region)?;
                            GlueChip::assert_equal(
                                &mut region,
                                beacon.installed().word(),
                                &cells.scalars[10],
                            )?;
                            equal_bytes(&mut region, beacon.session(), &cells.identities[5])?;
                            equal_bytes(&mut region, beacon.transcript(), &cells.identities[6])?;
                            None
                        }
                        ScheduleSourceStage::Next | ScheduleSourceStage::AfterNext => {
                            let index = usize::from(self.stage == ScheduleSourceStage::AfterNext);
                            let schedule = graph(&mut reader, &mut region, &before)?;
                            let source = if index == 0 {
                                schedule.next()
                            } else {
                                schedule.after_next()
                            };
                            let slot = SlotCells::parse(&mut reader, &mut region, source)?;
                            let height = reader.uint.checked_add_constant(
                                &mut region,
                                &cells.height,
                                (index + 1) as u128,
                            )?;
                            GlueChip::assert_equal(
                                &mut region,
                                slot.height().word(),
                                height.word(),
                            )?;
                            let offset = 11 + 8 * index;
                            GlueChip::assert_equal(
                                &mut region,
                                slot.pending().word(),
                                &cells.scalars[offset],
                            )?;
                            GlueChip::assert_equal(
                                &mut region,
                                slot.boundary_height().word(),
                                &cells.scalars[offset + 1],
                            )?;
                            for (actual, expected) in slot
                                .params()
                                .values()
                                .iter()
                                .zip(&cells.scalars[offset + 2..offset + 8])
                            {
                                GlueChip::assert_equal(&mut region, actual.word(), expected)?;
                            }
                            equal_bytes(
                                &mut region,
                                slot.predecessor(),
                                &cells.identities[9 + index],
                            )?;
                            None
                        }
                        ScheduleSourceStage::Member => {
                            let epoch = epoch(&mut reader, &mut region, &before)?;
                            let ten = reader.uint.constant::<32>(&mut region, 10)?;
                            let seat = reader.uint.checked_sub(&mut region, &cursor, &ten)?;
                            let seat = reader.uint.range_check::<5>(&mut region, seat.word())?;
                            let count = reader
                                .uint
                                .range_check::<5>(&mut region, &cells.scalars[0])?;
                            Some(epoch.member_for_source_count(
                                &mut reader,
                                &mut region,
                                &count,
                                &seat,
                            )?)
                        }
                        ScheduleSourceStage::Roster | ScheduleSourceStage::Terminal => None,
                    }
                };
                if let Some(member) = member {
                    let leaf =
                        key_leaf_at_cells(&mut chip, &mut region, member.index(), member.key())?;
                    for (i, out) in after[ROSTER..].iter_mut().enumerate() {
                        let index = chip
                            .uint()
                            .glue()
                            .constant(&mut region, Fp::from(i as u64))?;
                        let selected = chip.uint().glue().is_equal(
                            &mut region,
                            member.index().word(),
                            &index,
                        )?;
                        *out = chip.uint().glue().select(
                            &mut region,
                            &selected,
                            &leaf,
                            &before[ROSTER + i],
                        )?;
                    }
                }
                if self.stage == ScheduleSourceStage::Roster {
                    let zero = chip.uint().glue().constant(&mut region, Fp::ZERO)?;
                    GlueChip::assert_equal(&mut region, &before[ROSTER + 31], &zero)?;
                    let key = core::array::from_fn(|_| zero.clone());
                    let padding = key_leaf_cells(&mut chip, &mut region, 31, &key)?;
                    let mut level = before[ROSTER..ROSTER + 31].to_vec();
                    level.push(padding);
                    for depth in 0..5 {
                        let mut next_level = Vec::with_capacity(level.len() / 2);
                        for pair in level.chunks_exact(2) {
                            next_level.push(key_node_cells(
                                &mut chip,
                                &mut region,
                                depth,
                                &pair[0],
                                &pair[1],
                            )?);
                        }
                        level = next_level;
                    }
                    GlueChip::assert_equal(&mut region, &level[0], &cells.roster)?;
                    after[ROSTER] = level[0].clone();
                }
                if self.stage == ScheduleSourceStage::Terminal {
                    GlueChip::assert_equal(&mut region, &before[ROSTER], &cells.roster)?;
                    let zero = chip.uint().glue().constant(&mut region, Fp::ZERO)?;
                    after = core::array::from_fn(|_| zero.clone());
                }
                let start_digest =
                    state_cells(&mut chip, &mut region, &cells.context, &cursor, &before)?;
                let end_digest =
                    state_cells(&mut chip, &mut region, &cells.context, &next, &after)?;
                let program = chip
                    .uint()
                    .glue()
                    .constant(&mut region, Fp::from(PROGRAM_ID))?;
                let endpoints = SourceEndpoints::from_words(
                    &mut chip,
                    &mut region,
                    &[
                        program,
                        cells.context,
                        cursor.word().clone(),
                        next.word().clone(),
                        start_digest,
                        end_digest,
                    ],
                )?;
                SourceCheckpoint::leaf(&mut chip, &mut region, endpoints)?
                    .frame(&mut chip, &mut region)
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}

mod native;
pub use native::prepare_schedule_source;

#[cfg(test)]
mod tests;

impl ScheduleSourceBinding {
    /// Complete parser-context commitment which the qualified child must export.
    pub const fn digest(&self) -> &Word<Fp> {
        &self.context
    }
    /// Exact root and original length shared by the full result hash scan.
    pub const fn tape(&self) -> &ResultTape {
        &self.tape
    }
    /// Native context-hash source commitment derived from these same assigned cells.
    pub const fn epoch_hash_digest(&self) -> &Word<Fp> {
        &self.epoch_hash
    }
    /// Exact marked native context identifier to bind to QC or promised successor.
    pub const fn context_id(&self) -> &[Word<Fp>; 32] {
        &self.context_id
    }
    /// Original selected payload start, already range checked to32 bits.
    pub const fn payload_start(&self) -> &Word<Fp> {
        &self.start
    }
    /// Original selected payload length, already range checked to32 bits.
    pub const fn payload_len(&self) -> &Word<Fp> {
        &self.len
    }
    /// CRC64 proven by the separate complete epoch context scan.
    pub const fn checksum(&self) -> &Word<Fp> {
        &self.checksum
    }
    /// Current result height, to bind to the exact certified Commit message.
    pub const fn height(&self) -> &Uint<Fp, 64> {
        &self.height
    }
    /// Assigned role bit; full source verification proves the corresponding selection.
    pub const fn authorized(&self) -> &iroha_plonk_gadgets::Bit<Fp> {
        &self.authorized
    }
    /// Exact ordered roster commitment to bind to quorum aggregation.
    pub const fn roster_root(&self) -> &Word<Fp> {
        &self.roster
    }
    /// Original committee size, checked by the complete parser to equal 3f+1.
    pub const fn members(&self) -> &Word<Fp> {
        &self.scalars[0]
    }
    /// Original fault bound, checked by the complete parser.
    pub const fn faults(&self) -> &Word<Fp> {
        &self.scalars[1]
    }
    /// Native permissioned/NPoS policy tag.
    pub const fn mode(&self) -> &Word<Fp> {
        &self.scalars[2]
    }
    /// Exact epoch number to bind to the Commit message.
    pub const fn epoch(&self) -> &Word<Fp> {
        &self.scalars[3]
    }
    /// Exact first height governed by this selected native context.
    pub const fn first_height(&self) -> &Word<Fp> {
        &self.scalars[4]
    }
    /// Exact last height governed by this selected native context.
    pub const fn last_height(&self) -> &Word<Fp> {
        &self.scalars[5]
    }
    /// Native validator generation number.
    pub const fn generation(&self) -> &Word<Fp> {
        &self.scalars[6]
    }
    /// Native genesis/activation/retention decision tag.
    pub const fn decision(&self) -> &Word<Fp> {
        &self.scalars[7]
    }
    /// Decoded boundary presence; authentication requires the full source proof.
    pub const fn boundary_present(&self) -> &Word<Fp> {
        &self.scalars[8]
    }
    /// Original boundary height, zero when absent.
    pub const fn boundary_height(&self) -> &Word<Fp> {
        &self.scalars[9]
    }
    /// Original installed-beacon flag.
    pub const fn beacon_installed(&self) -> &Word<Fp> {
        &self.scalars[10]
    }
    /// Exact genesis-derived network identity.
    pub const fn network(&self) -> &[Word<Fp>; 32] {
        &self.identities[0]
    }
    /// Original epoch leader seed.
    pub const fn leader_seed(&self) -> &[Word<Fp>; 32] {
        &self.identities[1]
    }
    /// Exact validator-generation authority identity.
    pub const fn authority_id(&self) -> &[Word<Fp>; 32] {
        &self.identities[2]
    }
    /// Exact predecessor authorization identity.
    pub const fn previous_authorization_id(&self) -> &[Word<Fp>; 32] {
        &self.identities[3]
    }
    /// Exact activation or cancellation attempt identity.
    pub const fn transition_id(&self) -> &[Word<Fp>; 32] {
        &self.identities[4]
    }
    /// Original installed-beacon session or zero for Bootstrap.
    pub const fn beacon_session(&self) -> &[Word<Fp>; 32] {
        &self.identities[5]
    }
    /// Original installed-beacon transcript or zero for Bootstrap.
    pub const fn beacon_transcript(&self) -> &[Word<Fp>; 32] {
        &self.identities[6]
    }
    /// Boundary's exact predecessor context, to equate to the authenticated incumbent.
    pub const fn boundary_predecessor(&self) -> &[Word<Fp>; 32] {
        &self.identities[7]
    }
    /// Original B-1 committed selection anchor or zero when absent.
    pub const fn boundary_selection_anchor(&self) -> &[Word<Fp>; 32] {
        &self.identities[8]
    }
    /// h+1 (index 0) or h+2 (index 1) slot; there are no other scheduling slots.
    pub fn slot(&self, index: usize) -> Option<ScheduleSlotBinding<'_>> {
        if index >= 2 {
            return None;
        }
        let offset = 11 + 8 * index;
        Some(ScheduleSlotBinding {
            scalars: &self.scalars[offset..offset + 8],
            predecessor: &self.identities[9 + index],
        })
    }
    /// Exact complete 43-step parser endpoints with empty initial/final states.
    /// The terminal must compare all six words to a qualified child proof.
    /// # Errors
    /// Circuit layout errors.
    pub fn complete_parser_endpoints(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
    ) -> Result<SourceEndpoints, Error> {
        let zero = chip.uint().glue().constant(region, Fp::ZERO)?;
        let state = core::array::from_fn(|_| zero.clone());
        let start = chip.uint().constant::<32>(region, 0)?;
        let end = chip
            .uint()
            .constant::<32>(region, u128::from(PROGRAM_LENGTH))?;
        let before = state_cells(chip, region, &self.context, &start, &state)?;
        let after = state_cells(chip, region, &self.context, &end, &state)?;
        SourceEndpoints::leaf(
            chip,
            region,
            Fp::from(PROGRAM_ID),
            &self.context,
            0,
            PROGRAM_LENGTH,
            &before,
            &after,
        )
    }
    /// Exact complete fixed CRC-plus-Blake endpoints under this same assigned context.
    /// Actual payload length selects constrained work within that fixed source interval.
    /// # Errors
    /// Circuit layout errors.
    pub fn complete_epoch_hash_endpoints(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
    ) -> Result<SourceEndpoints, Error> {
        let end = chip
            .uint()
            .constant::<32>(region, u128::from(context_hash::PROGRAM_LENGTH))?;
        let start = chip.uint().constant::<32>(region, 0)?;
        let zero = chip.uint().glue().constant(region, Fp::ZERO)?;
        let state = core::array::from_fn(|_| zero.clone());
        let before =
            context_hash::state_cells(chip, region, &self.epoch_hash, &start, &zero, &state)?;
        let after = context_hash::state_cells(chip, region, &self.epoch_hash, &end, &zero, &state)?;
        let program = chip
            .uint()
            .glue()
            .constant(region, Fp::from(context_hash::PROGRAM_ID))?;
        SourceEndpoints::from_words(
            chip,
            region,
            &[
                program,
                self.epoch_hash.clone(),
                start.word().clone(),
                end.word().clone(),
                before,
                after,
            ],
        )
    }
}

/// Assigned h+1/h+2 projection bound into the complete parser context.
#[derive(Clone, Copy, Debug)]
pub struct ScheduleSlotBinding<'a> {
    scalars: &'a [Word<Fp>],
    predecessor: &'a [Word<Fp>; 32],
}
impl ScheduleSlotBinding<'_> {
    /// Original `PendingBoundary` flag.
    pub fn pending(&self) -> &Word<Fp> {
        &self.scalars[0]
    }
    /// Pending boundary height, zero for Ready.
    pub fn boundary_height(&self) -> &Word<Fp> {
        &self.scalars[1]
    }
    /// Exact pending predecessor context, zero for Ready.
    pub const fn predecessor(&self) -> &[Word<Fp>; 32] {
        self.predecessor
    }
    /// Block/retry/execution/apply times, max payload bytes and epoch length.
    pub fn parameters(&self) -> &[Word<Fp>] {
        &self.scalars[2..8]
    }
}
