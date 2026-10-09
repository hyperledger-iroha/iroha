//! Fixed bounded scans of the exact native epoch-context hash preimage.
//!
//! CRC leaves first reconstruct the canonical Norito header checksum from the
//! selected original payload. Blake leaves then hash the exact domain, header
//! and that same payload. The complete two-phase interval is required. This
//! source authenticates neither the tape nor the payload selection: the finality
//! owner must bind them to the complete result scan and checked epoch parser,
//! then authenticate the context from genesis or an authorized predecessor.

use ff::Field;
use iroha_pasta::{Ep, Fp, poseidon::hash_with_domain};
use iroha_plonk::{
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Error, Layouter, Region, SimpleFloorPlanner, Value},
};
use iroha_plonk_gadgets::{
    Bit, GlueChip, Uint, UintChip, Word,
    blake2b::{Blake2bChip, Blake2bConfig},
    crc64::Crc64State,
};
use iroha_plonk_recursion::verifier::{VerifierChip, VerifierConfig};

use super::tape::{ResultTape, ResultTapeWitness};
use crate::finality::continuity::{SourceCheckpoint, SourceEndpoints, leaf_frame_native};

/// One source program, with CRC leaves followed by Blake leaves.
pub const PROGRAM_ID: u64 = u64::from_le_bytes(*b"kgwecsp1");
/// Complete selected payload and expected native context commitment.
pub const CONTEXT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwecsc1");
/// Complete live state, including the program cursor and phase.
pub const STATE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwecss1");
/// Production native validator epoch domain including its explicit separator.
pub const EPOCH_TAG: &[u8] = b"iroha:native-validator-epoch:v1\0";
/// Exact native context codec identity, captured from the production codec.
pub const EPOCH_CODEC_ID: [u8; 16] = [
    0x2f, 0x8d, 0xdb, 0xce, 0x70, 0xdf, 0xe2, 0x01, 0xd8, 0x82, 0xe5, 0xbb, 0x2d, 0x3f, 0x88, 0x89,
];
const PREFIX: usize = EPOCH_TAG.len() + 40;
/// Fixed CRC leaf count covering the complete native 64 KiB result bound.
pub const CRC_LEAVES: u32 = 2_048;
/// Fixed Blake leaf count covering the canonical prefix plus the same payload bound.
pub const BLAKE_LEAVES: u32 = 513;
/// Fixed complete source interval, independent of the authenticated payload length.
pub const PROGRAM_LENGTH: u32 = CRC_LEAVES + BLAKE_LEAVES;

/// An untrusted selection which must match the checked epoch parser's span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextHashInput {
    /// Result tape commitment shared with the complete R hash scan.
    pub tape_root: Fp,
    /// Exact original result frame length.
    pub result_len: u32,
    /// Context payload start inside that result, excluding the result domain.
    pub payload_start: u32,
    /// Exact context payload length, excluding its canonical header.
    pub payload_len: u32,
    /// Canonical header checksum, proved by the complete first phase.
    pub checksum: u64,
    /// Native marked Blake2b-256 context identifier, proved by the second phase.
    pub context_id: [u8; 32],
}
impl ContextHashInput {
    /// Number of real 32-byte CRC leaves before constrained no-op padding.
    pub const fn crc_steps(&self) -> u32 {
        self.payload_len.div_ceil(32)
    }
    /// Number of real compression blocks before constrained no-op padding.
    pub fn blake_steps(&self) -> u32 {
        let prefix = u64::try_from(PREFIX).expect("fixed context prefix fits u64");
        u32::try_from((prefix + u64::from(self.payload_len)).div_ceil(128))
            .expect("bounded context block count fits u32")
    }
    /// Native witness commitment; its value is not a parser or finality verdict.
    pub fn digest(&self) -> Fp {
        let mut words = vec![
            Fp::from(PROGRAM_ID),
            self.tape_root,
            Fp::from(u64::from(self.result_len)),
            Fp::from(u64::from(self.payload_start)),
            Fp::from(u64::from(self.payload_len)),
            Fp::from(self.checksum),
        ];
        words.extend(self.context_id.map(|b| Fp::from(u64::from(b))));
        hash_with_domain(CONTEXT_DOMAIN, &words)
    }
}

/// Assigned parser-selected fields of one native epoch context source.
#[derive(Clone, Copy, Debug)]
pub struct ContextHashInputCells<'a> {
    /// Shared original-result tape commitment.
    pub tape_root: &'a Word<Fp>,
    /// Original result frame length.
    pub result_len: &'a Word<Fp>,
    /// Context payload offset inside that result.
    pub payload_start: &'a Word<Fp>,
    /// Context payload length.
    pub payload_len: &'a Word<Fp>,
    /// Canonical context header checksum.
    pub checksum: &'a Word<Fp>,
    /// Native marked context identifier bytes.
    pub context_id: &'a [Word<Fp>; 32],
}

/// Bind an exact parser-selected payload to this source's context digest.
/// The caller retains responsibility for authenticating the tape and deriving
/// the span from the checked current or authorized successor epoch container.
/// # Errors
/// Layout errors; malformed lengths, checksum or digest bytes are unsatisfied.
pub fn context_digest_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    input: ContextHashInputCells<'_>,
) -> Result<Word<Fp>, Error> {
    let result_len = chip.uint().range_check::<32>(region, input.result_len)?;
    let start = chip.uint().range_check::<32>(region, input.payload_start)?;
    let len = chip.uint().range_check::<32>(region, input.payload_len)?;
    chip.uint().glue().assert_nonzero(region, len.word())?;
    let end = chip.uint().checked_add(region, &start, &len)?;
    chip.uint().assert_le(region, &end, &result_len)?;
    let _ = ResultTape::new(&mut chip.uint(), region, input.tape_root, &result_len)?;
    chip.uint().range_check::<64>(region, input.checksum)?;
    let mut words = vec![
        chip.uint().glue().constant(region, Fp::from(PROGRAM_ID))?,
        input.tape_root.clone(),
        result_len.word().clone(),
        start.word().clone(),
        len.word().clone(),
        input.checksum.clone(),
    ];
    for byte in input.context_id {
        chip.uint().range_check::<8>(region, byte)?;
    }
    words.extend(input.context_id.iter().cloned());
    chip.hash_words(region, CONTEXT_DOMAIN, &words)
}

/// One of two separately pinned fixed arithmetic layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextHashPhase {
    /// Consume up to 32 consecutive payload bytes into CRC64-XZ.
    Crc,
    /// Compress 128 consecutive canonical preimage bytes into Blake2b-256.
    Blake,
}

/// Full running registers: one CRC word and all eight Blake chaining words.
/// Unused registers and every phase boundary are constrained to zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContextHashState {
    /// Unfinalized CRC state between first-phase leaves.
    pub crc: u64,
    /// Complete Blake state between second-phase leaves.
    pub blake: [u64; 8],
}
fn state_native(context: Fp, cursor: u32, phase: u64, state: ContextHashState) -> Fp {
    let mut words = vec![
        Fp::from(PROGRAM_ID),
        context,
        Fp::from(u64::from(cursor)),
        Fp::from(phase),
        Fp::from(state.crc),
    ];
    words.extend(state.blake.map(Fp::from));
    hash_with_domain(STATE_DOMAIN, &words)
}
/// Deterministic zero-register boundary state required at full start/end.
pub fn boundary_digest_native(input: &ContextHashInput, terminal: bool) -> Fp {
    state_native(
        input.digest(),
        if terminal { PROGRAM_LENGTH } else { 0 },
        0,
        ContextHashState::default(),
    )
}

/// An actual fixed arithmetic transition. Host fields supply witnesses only.
#[derive(Clone, Debug)]
pub struct ContextHashLeafCircuit {
    phase: ContextHashPhase,
    cursor: u32,
    input: ContextHashInput,
    before: ContextHashState,
    after: ContextHashState,
    tape: std::sync::Arc<ResultTapeWitness>,
    known: bool,
}
/// Native verifier lanes plus an independent constrained Blake lane.
#[derive(Clone, Debug)]
pub struct ContextHashConfig {
    verifier: VerifierConfig<Ep>,
    blake: Blake2bConfig,
    public: Column<Instance>,
}
impl ContextHashLeafCircuit {
    /// Witnessless phase layout for independent original proving-key import.
    pub fn for_source(phase: ContextHashPhase) -> Self {
        Self {
            phase,
            cursor: if phase == ContextHashPhase::Crc {
                0
            } else {
                CRC_LEAVES
            },
            input: ContextHashInput {
                tape_root: Fp::ZERO,
                result_len: 0,
                payload_start: 0,
                payload_len: 1,
                checksum: 0,
                context_id: [0; 32],
            },
            before: ContextHashState::default(),
            after: ContextHashState::default(),
            tape: std::sync::Arc::new(ResultTapeWitness::unknown()),
            known: false,
        }
    }

    /// Exact untrusted context retained by every leaf of this scan.
    pub const fn input(&self) -> &ContextHashInput {
        &self.input
    }

    /// Construct a fixed phase with bounded host allocation. Byte membership,
    /// progress, initial state, checksum and output are circuit constraints.
    /// # Errors
    /// The frame exceeds the native bound or the proposed cursor/phase is invalid.
    pub fn new(
        phase: ContextHashPhase,
        cursor: u32,
        input: ContextHashInput,
        before: ContextHashState,
        after: ContextHashState,
        frame: Vec<u8>,
    ) -> Result<Self, Error> {
        if frame.len() > 65_536
            || input.payload_len == 0
            || input.payload_len > 65_536
            || input
                .payload_start
                .checked_add(input.payload_len)
                .is_none_or(|n| n > input.result_len)
            || cursor >= PROGRAM_LENGTH
            || (phase == ContextHashPhase::Crc) != (cursor < CRC_LEAVES)
        {
            return Err(Error::Synthesis);
        }
        Ok(Self {
            phase,
            cursor,
            input,
            before,
            after,
            tape: std::sync::Arc::new(ResultTapeWitness::from_frame(&Value::known(frame))?),
            known: true,
        })
    }
    /// Context and exact transition endpoints, untrusted until the source proves them.
    pub fn endpoints(&self) -> [Fp; 6] {
        let c = self.input.digest();
        let kind = |cursor| {
            if cursor == 0 || cursor == CRC_LEAVES || cursor == PROGRAM_LENGTH {
                0
            } else if cursor < CRC_LEAVES {
                1
            } else {
                2
            }
        };
        [
            Fp::from(PROGRAM_ID),
            c,
            Fp::from(u64::from(self.cursor)),
            Fp::from(u64::from(self.cursor + 1)),
            state_native(c, self.cursor, kind(self.cursor), self.before),
            state_native(c, self.cursor + 1, kind(self.cursor + 1), self.after),
        ]
    }
    /// Internal 69-word source frame, with explicitly decided empty obligations.
    /// # Errors
    /// Invalid source endpoint or fixed filler encoding.
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

fn scaled(
    uint: &mut UintChip<'_, Fp>,
    region: &mut Region<'_, Fp>,
    n: &Uint<Fp, 32>,
    scale: u64,
) -> Result<Uint<Fp, 32>, Error> {
    let word = uint
        .glue()
        .linear(region, &[(Fp::from(scale), n.word())], Fp::ZERO)?;
    uint.range_check::<32>(region, &word)
}
pub(super) fn ceil_steps<const B: usize>(
    uint: &mut UintChip<'_, Fp>,
    region: &mut Region<'_, Fp>,
    len: &Uint<Fp, 32>,
) -> Result<Uint<Fp, 32>, Error> {
    let d = 1_u128
        .checked_shl(u32::try_from(B).map_err(|_| Error::Synthesis)?)
        .ok_or(Error::Synthesis)?;
    let sum = uint.checked_add_constant(region, len, d - 1)?;
    let q = uint.assign::<32>(region, sum.value().map(|v| v / d))?;
    let r = uint.assign::<B>(region, sum.value().map(|v| v % d))?;
    let joined = uint.glue().linear(
        region,
        &[
            (
                Fp::from(u64::try_from(d).map_err(|_| Error::Synthesis)?),
                q.word(),
            ),
            (Fp::ONE, r.word()),
        ],
        Fp::ZERO,
    )?;
    GlueChip::assert_equal(region, &joined, sum.word())?;
    Ok(q)
}
fn equal_when(
    uint: &mut UintChip<'_, Fp>,
    region: &mut Region<'_, Fp>,
    bit: &Bit<Fp>,
    a: &Word<Fp>,
    b: &Word<Fp>,
) -> Result<(), Error> {
    let delta = uint.glue().sub(region, a, b)?;
    let selected = uint.glue().mul(region, bit.word(), &delta)?;
    GlueChip::assert_constant(region, &selected, Fp::ZERO)
}
pub(super) fn state_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    context: &Word<Fp>,
    cursor: &Uint<Fp, 32>,
    kind: &Word<Fp>,
    registers: &[Word<Fp>; 9],
) -> Result<Word<Fp>, Error> {
    let mut words = vec![
        chip.uint().glue().constant(region, Fp::from(PROGRAM_ID))?,
        context.clone(),
        cursor.word().clone(),
        kind.clone(),
    ];
    words.extend(registers.iter().cloned());
    chip.hash_words(region, STATE_DOMAIN, &words)
}
fn little_bytes<const N: usize>(
    uint: &mut UintChip<'_, Fp>,
    region: &mut Region<'_, Fp>,
    word: &Word<Fp>,
) -> Result<[Word<Fp>; N], Error> {
    let mut out = Vec::new();
    for i in 0..N {
        out.push(
            uint.assign::<8>(
                region,
                word.value()
                    .map(|v| (iroha_plonk_gadgets::cells::low_u128(&v) >> (8 * i)) & 255),
            )?
            .word()
            .clone(),
        );
    }
    let mut packed = uint.glue().constant(region, Fp::ZERO)?;
    for byte in out.iter().rev() {
        packed = uint.glue().linear(
            region,
            &[(Fp::from(256), &packed), (Fp::ONE, byte)],
            Fp::ZERO,
        )?;
    }
    GlueChip::assert_equal(region, &packed, word)?;
    out.try_into().map_err(|_| Error::Synthesis)
}

impl ContextHashLeafCircuit {
    // Shared atomic transition, including complete state and original-tape bindings.
    // Batch owners execute every transition and join these exact endpoints.
    fn assign_transition(
        &self,
        chip: &mut VerifierChip<Ep>,
        blake: &mut Blake2bChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
    ) -> Result<SourceEndpoints, Error> {
        let witness = &self.tape;
        let values = [
            self.input.tape_root,
            Fp::from(u64::from(self.input.result_len)),
            Fp::from(u64::from(self.input.payload_start)),
            Fp::from(u64::from(self.input.payload_len)),
            Fp::from(self.input.checksum),
        ];
        let words = chip
            .uint()
            .glue()
            .witnesses(region, &values.map(|v| self.value(v)))?;
        let id: [Word<Fp>; 32] = chip
            .uint()
            .glue()
            .witnesses(
                region,
                &self
                    .input
                    .context_id
                    .map(|b| self.value(Fp::from(u64::from(b)))),
            )?
            .try_into()
            .map_err(|_| Error::Synthesis)?;
        let context = context_digest_cells(
            chip,
            region,
            ContextHashInputCells {
                tape_root: &words[0],
                result_len: &words[1],
                payload_start: &words[2],
                payload_len: &words[3],
                checksum: &words[4],
                context_id: &id,
            },
        )?;
        let result_len = chip.uint().range_check::<32>(region, &words[1])?;
        let start = chip.uint().range_check::<32>(region, &words[2])?;
        let len = chip.uint().range_check::<32>(region, &words[3])?;
        let tape = ResultTape::new(&mut chip.uint(), region, &words[0], &result_len)?;
        let cursor = chip
            .uint()
            .assign::<32>(region, self.value(u128::from(self.cursor)))?;
        let next = chip.uint().checked_add_constant(region, &cursor, 1)?;
        let crc_steps = ceil_steps::<5>(&mut chip.uint(), region, &len)?;
        let total = chip
            .uint()
            .checked_add_constant(region, &len, PREFIX as u128)?;
        let blake_steps = ceil_steps::<7>(&mut chip.uint(), region, &total)?;
        let crc_end = chip.uint().constant::<32>(region, u128::from(CRC_LEAVES))?;
        let end = chip
            .uint()
            .constant::<32>(region, u128::from(PROGRAM_LENGTH))?;
        chip.uint().assert_lt(region, &cursor, &end)?;
        let zero = chip.uint().glue().constant(region, Fp::ZERO)?;
        let mut before = Vec::with_capacity(9);
        for value in core::iter::once(self.before.crc).chain(self.before.blake) {
            before.push(
                chip.uint()
                    .assign::<64>(region, self.value(u128::from(value)))?
                    .word()
                    .clone(),
            );
        }
        let before: [Word<Fp>; 9] = before.try_into().map_err(|_| Error::Synthesis)?;
        let (first, last, kind, after) = match self.phase {
            ContextHashPhase::Crc => {
                chip.uint().assert_lt(region, &cursor, &crc_end)?;
                let first = chip.uint().glue().is_zero(region, cursor.word())?;
                let last = chip
                    .uint()
                    .glue()
                    .is_equal(region, next.word(), crc_steps.word())?;
                equal_when(&mut chip.uint(), region, &first, &before[0], &zero)?;
                for word in &before[1..] {
                    GlueChip::assert_equal(region, word, &zero)?;
                }
                let initial = chip.uint().glue().constant(region, Fp::from(u64::MAX))?;
                let imported = chip
                    .uint()
                    .glue()
                    .select(region, &first, &initial, &before[0])?;
                let mut state = Crc64State::from_word(&mut chip.uint(), region, &imported)?;
                let consumed = scaled(&mut chip.uint(), region, &cursor, 32)?;
                let in_payload = chip.uint().lt(region, &consumed, &len)?;
                let safe =
                    chip.uint()
                        .glue()
                        .select(region, &in_payload, consumed.word(), len.word())?;
                let safe = chip.uint().range_check::<32>(region, &safe)?;
                let inactive = chip.uint().glue().not(region, &in_payload)?;
                equal_when(&mut chip.uint(), region, &inactive, &before[0], &zero)?;
                let offset = chip.uint().checked_add(region, &start, &safe)?;
                let bytes = {
                    let (mut uint, hash) = chip.uint_and_hasher()?;
                    witness.read_padded::<32>(&tape, &mut uint, hash, region, &offset)?
                };
                for (i, byte) in bytes.iter().enumerate() {
                    let position = chip
                        .uint()
                        .checked_add_constant(region, &consumed, i as u128)?;
                    let active = chip.uint().lt(region, &position, &len)?;
                    state = state.update(&mut chip.uint(), region, byte, &active)?;
                }
                let checksum = state.checksum(&mut chip.uint(), region)?;
                equal_when(&mut chip.uint(), region, &last, checksum.word(), &words[4])?;
                let raw = state.word(&mut chip.uint(), region)?;
                let mut after = core::array::from_fn(|_| zero.clone());
                after[0] = chip
                    .uint()
                    .glue()
                    .select(region, &last, &zero, raw.word())?;
                let phase_last =
                    chip.uint()
                        .glue()
                        .is_equal(region, next.word(), crc_end.word())?;
                (first, phase_last, 1_u64, after)
            }
            ContextHashPhase::Blake => {
                let local = chip.uint().checked_sub(region, &cursor, &crc_end)?;
                let local_next = chip.uint().checked_add_constant(region, &local, 1)?;
                let block_active = chip.uint().lt(region, &local, &blake_steps)?;
                let first = chip.uint().glue().is_zero(region, local.word())?;
                let last =
                    chip.uint()
                        .glue()
                        .is_equal(region, local_next.word(), blake_steps.word())?;
                GlueChip::assert_equal(region, &before[0], &zero)?;
                for word in &before[1..] {
                    equal_when(&mut chip.uint(), region, &first, word, &zero)?;
                }
                let initial = blake.initial_state(region)?;
                let initial = blake.state_words(region, &initial)?;
                let mut imported = Vec::with_capacity(8);
                for (word, initial) in before[1..].iter().zip(&initial) {
                    imported.push(chip.uint().glue().select(
                        region,
                        &first,
                        initial.word(),
                        word,
                    )?);
                }
                let imported = blake.state_from_words(
                    region,
                    &imported.try_into().map_err(|_| Error::Synthesis)?,
                )?;
                let consumed = scaled(&mut chip.uint(), region, &local, 128)?;
                let not_first = chip.uint().glue().not(region, &first)?;
                let mut prefix = Vec::with_capacity(PREFIX);
                for byte in EPOCH_TAG
                    .iter()
                    .chain(b"NRT0\0\0".iter())
                    .chain(EPOCH_CODEC_ID.iter())
                    .chain(core::iter::once(&0))
                {
                    prefix.push(
                        chip.uint()
                            .glue()
                            .constant(region, Fp::from(u64::from(*byte)))?,
                    );
                }
                prefix.extend(little_bytes::<8>(&mut chip.uint(), region, len.word())?);
                prefix.extend(little_bytes::<8>(&mut chip.uint(), region, &words[4])?);
                prefix.push(chip.uint().glue().constant(region, Fp::from(2))?);
                if prefix.len() != PREFIX {
                    return Err(Error::Synthesis);
                }
                let mut block = Vec::with_capacity(128);
                for chunk in 0..4 {
                    let relative = if chunk * 32 < PREFIX {
                        chip.uint().glue().linear(
                            region,
                            &[
                                (Fp::ONE, consumed.word()),
                                (-Fp::from((PREFIX - chunk * 32) as u64), not_first.word()),
                            ],
                            Fp::ZERO,
                        )?
                    } else {
                        chip.uint().glue().add_constant(
                            region,
                            consumed.word(),
                            Fp::from((chunk * 32 - PREFIX) as u64),
                        )?
                    };
                    let relative = chip.uint().range_check::<32>(region, &relative)?;
                    let in_payload = chip.uint().lt(region, &relative, &len)?;
                    let safe = chip.uint().glue().select(
                        region,
                        &in_payload,
                        relative.word(),
                        len.word(),
                    )?;
                    let safe = chip.uint().range_check::<32>(region, &safe)?;
                    let offset = chip.uint().checked_add(region, &start, &safe)?;
                    let source = {
                        let (mut uint, hash) = chip.uint_and_hasher()?;
                        witness.read_padded::<32>(&tape, &mut uint, hash, region, &offset)?
                    };
                    for i in 0..32 {
                        let absolute = chunk * 32 + i;
                        let on_first = if absolute < PREFIX {
                            prefix[absolute].clone()
                        } else if chunk * 32 < PREFIX {
                            source[absolute - PREFIX].clone()
                        } else {
                            source[i].clone()
                        };
                        let byte = chip
                            .uint()
                            .glue()
                            .select(region, &first, &on_first, &source[i])?;
                        let position = chip.uint().checked_add_constant(
                            region,
                            &consumed,
                            absolute as u128,
                        )?;
                        let active = chip.uint().lt(region, &position, &total)?;
                        block.push(chip.uint().glue().mul(region, active.word(), &byte)?);
                    }
                }
                let block = block.try_into().map_err(|_| Error::Synthesis)?;
                let following = chip.uint().checked_add_constant(region, &consumed, 128)?;
                let counter =
                    chip.uint()
                        .glue()
                        .select(region, &last, total.word(), following.word())?;
                let after_state = blake.compress_block(
                    region,
                    &imported,
                    &block,
                    &[counter, zero.clone()],
                    &last,
                )?;
                let digest = blake.digest_marked(region, &after_state)?;
                for (byte, expected) in digest.bytes().iter().zip(&id) {
                    equal_when(&mut chip.uint(), region, &last, byte, expected)?;
                }
                let state = blake.state_words(region, &after_state)?;
                let mut after = core::array::from_fn(|_| zero.clone());
                for (out, word) in after[1..].iter_mut().zip(&state) {
                    let advanced = chip
                        .uint()
                        .glue()
                        .select(region, &last, &zero, word.word())?;
                    *out = chip
                        .uint()
                        .glue()
                        .mul(region, block_active.word(), &advanced)?;
                }
                let inactive = chip.uint().glue().not(region, &block_active)?;
                for word in &before[1..] {
                    equal_when(&mut chip.uint(), region, &inactive, word, &zero)?;
                }
                let phase_last = chip
                    .uint()
                    .glue()
                    .is_equal(region, next.word(), end.word())?;
                (first, phase_last, 2_u64, after)
            }
        };
        let nonfirst = chip.uint().glue().not(region, &first)?;
        let nonlast = chip.uint().glue().not(region, &last)?;
        let before_kind =
            chip.uint()
                .glue()
                .linear(region, &[(Fp::from(kind), nonfirst.word())], Fp::ZERO)?;
        let after_kind =
            chip.uint()
                .glue()
                .linear(region, &[(Fp::from(kind), nonlast.word())], Fp::ZERO)?;
        let before_digest = state_cells(chip, region, &context, &cursor, &before_kind, &before)?;
        let after_digest = state_cells(chip, region, &context, &next, &after_kind, &after)?;
        let program = chip.uint().glue().constant(region, Fp::from(PROGRAM_ID))?;
        let endpoints = SourceEndpoints::from_words(
            chip,
            region,
            &[
                program,
                context,
                cursor.word().clone(),
                next.word().clone(),
                before_digest,
                after_digest,
            ],
        )?;
        Ok(endpoints)
    }
}

impl Circuit<Fp> for ContextHashLeafCircuit {
    type Config = ContextHashConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            tape: std::sync::Arc::new(ResultTapeWitness::unknown()),
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Self::Config {
        let verifier = VerifierConfig::configure_serialized_foreign_tagged(meta, 3)
            .expect("fixed epoch source profile");
        let columns = core::array::from_fn(|_| meta.advice_column());
        let constants = meta.fixed_column();
        let blake = Blake2bConfig::configure(meta, columns, constants);
        let public = meta.instance_column(69);
        meta.enable_equality(public);
        ContextHashConfig {
            verifier,
            blake,
            public,
        }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let mut blake = Blake2bChip::new(&config.blake);
        let output = layouter.assign_region(
            || "native epoch canonical context source",
            |mut region| {
                let endpoints = self.assign_transition(&mut chip, &mut blake, &mut region)?;
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

#[cfg(test)]
mod tests;

mod native;
pub use native::prepare_context_hash;

mod batch;
pub use batch::{
    BATCH_LENGTH, ContextHashBatchCircuit, ContextHashBatchPlan, prepare_context_batches,
};
