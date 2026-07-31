//! Aggregate fixed-topology AIR for the native private-note relation.
//!
//! The circuit keeps every intermediate SHA-256 digest private.  A fixed
//! byte-copy permutation connects note fields, hash inputs and outputs,
//! accumulator children, value arithmetic, and VM state.  Public statement
//! bytes are fixed constraints at their final endpoints only.

use std::collections::BTreeMap;

use iroha_data_model::privacy::IrohaIvmPrivateNoteStarkStatementV1;
use thiserror::Error;

use super::{
    codec::{PRIVATE_PROGRAM_BYTES_V1, decode_private_program_v1, encode_private_program_v1},
    relation::{
        ACCUMULATOR_LEAF_DOMAIN_V1, ACCUMULATOR_NODE_DOMAIN_V1, HASH_FRAME_DOMAIN_V1,
        IvmPrivateNoteWitnessV1, NOTE_AUTHORITY_DOMAIN_V1, NOTE_COMMITMENT_DOMAIN_V1,
        NOTE_NULLIFIER_DOMAIN_V1, PRIVATE_NOTE_MAX_INPUTS_V1, PRIVATE_NOTE_MAX_OUTPUTS_V1,
        PRIVATE_NOTE_TREE_DEPTH_V1, PRIVATE_PROGRAM_INSTRUCTION_COUNT_V1, PROGRAM_ID_DOMAIN_V1,
        PrivateInstructionV1, PrivateOpcodeV1, Sha256InvocationRoleV1, Sha256InvocationV1,
        namespace_v1, public_balance_sides, validate_private_note_relation_v1,
        validate_statement_v1,
    },
};
use crate::privacy_engines::transparent_stark::{GOLDILOCKS_MODULUS_V1, GoldilocksFieldV1 as F};

pub(super) const PRIVATE_NOTE_TRACE_LOG2_V1: u8 = 14;
pub(super) const PRIVATE_NOTE_TRACE_SIZE_V1: usize = 1 << PRIVATE_NOTE_TRACE_LOG2_V1;
pub(super) const PRIVATE_NOTE_COPY_WIDTH_V1: usize = 8;
pub(super) const PRIVATE_NOTE_SHA_SCHEDULE_WORDS_V1: usize = 64;
pub(super) const PRIVATE_NOTE_SHA_STATE_WORDS_V1: usize = 8;
pub(super) const PRIVATE_NOTE_SHA_BIT_GROUPS_V1: usize = 11;
pub(super) const PRIVATE_NOTE_SHA_BITS_PER_GROUP_V1: usize = 32;
pub(super) const PRIVATE_NOTE_SHA_BIT_COLUMNS_V1: usize =
    PRIVATE_NOTE_SHA_BIT_GROUPS_V1 * PRIVATE_NOTE_SHA_BITS_PER_GROUP_V1;

pub(super) const COPY_OFFSET: usize = 0;
pub(super) const SHA_SCHEDULE_OFFSET: usize = COPY_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1;
pub(super) const SHA_INITIAL_STATE_OFFSET: usize =
    SHA_SCHEDULE_OFFSET + PRIVATE_NOTE_SHA_SCHEDULE_WORDS_V1;
pub(super) const SHA_STATE_OFFSET: usize =
    SHA_INITIAL_STATE_OFFSET + PRIVATE_NOTE_SHA_STATE_WORDS_V1;
pub(super) const SHA_BITS_OFFSET: usize = SHA_STATE_OFFSET + PRIVATE_NOTE_SHA_STATE_WORDS_V1;
pub(super) const SHA_T1_OFFSET: usize = SHA_BITS_OFFSET + PRIVATE_NOTE_SHA_BIT_COLUMNS_V1;
pub(super) const SHA_T2_OFFSET: usize = SHA_T1_OFFSET + 1;
pub(super) const SHA_CARRY_OFFSET: usize = SHA_T2_OFFSET + 1;
pub(super) const SHA_CARRY_WIDTH: usize = 18;
pub(super) const SCRATCH_OFFSET: usize = SHA_CARRY_OFFSET + SHA_CARRY_WIDTH;
pub(super) const SCRATCH_WIDTH: usize = 96;
pub(super) const PRIVATE_NOTE_BASE_WIDTH_V1: usize = SCRATCH_OFFSET + SCRATCH_WIDTH;

pub(super) const SCRATCH_NONZERO_BYTE_SELECT_OFFSET: usize = SCRATCH_OFFSET;
pub(super) const SCRATCH_NONZERO_BIT_SELECT_OFFSET: usize =
    SCRATCH_NONZERO_BYTE_SELECT_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1;
pub(super) const SCRATCH_BYTE_BITS_OFFSET: usize = SCRATCH_NONZERO_BIT_SELECT_OFFSET + 8;
pub(super) const SCRATCH_RUNNING_BEFORE: usize = SCRATCH_BYTE_BITS_OFFSET + 8;
pub(super) const SCRATCH_RUNNING_AFTER: usize = SCRATCH_RUNNING_BEFORE + 1;
pub(super) const SCRATCH_RELATION_CARRY_BEFORE: usize = SCRATCH_RUNNING_AFTER + 1;
pub(super) const SCRATCH_RELATION_CARRY_AFTER: usize = SCRATCH_RELATION_CARRY_BEFORE + 1;
pub(super) const SCRATCH_RELATION_CARRY_BITS_OFFSET: usize = SCRATCH_RELATION_CARRY_AFTER + 1;
pub(super) const SCRATCH_VM_OPCODE_SELECT_OFFSET: usize = SCRATCH_RELATION_CARRY_BITS_OFFSET + 2;
pub(super) const SCRATCH_VM_DESTINATION_SELECT_OFFSET: usize = SCRATCH_VM_OPCODE_SELECT_OFFSET + 9;
pub(super) const SCRATCH_VM_LEFT_SELECT_OFFSET: usize = SCRATCH_VM_DESTINATION_SELECT_OFFSET + 8;
pub(super) const SCRATCH_VM_RIGHT_SELECT_OFFSET: usize = SCRATCH_VM_LEFT_SELECT_OFFSET + 8;
pub(super) const SCRATCH_VM_IMMEDIATE_OFFSET: usize = SCRATCH_VM_RIGHT_SELECT_OFFSET + 8;
pub(super) const SCRATCH_VM_HALTED_BEFORE: usize = SCRATCH_VM_IMMEDIATE_OFFSET + 4;
pub(super) const SCRATCH_VM_HALTED_AFTER: usize = SCRATCH_VM_HALTED_BEFORE + 1;
pub(super) const SCRATCH_VM_CARRY_BEFORE: usize = SCRATCH_VM_HALTED_AFTER + 1;
pub(super) const SCRATCH_VM_CARRY_AFTER: usize = SCRATCH_VM_CARRY_BEFORE + 1;
pub(super) const SCRATCH_VM_DIFFERENCE: usize = SCRATCH_VM_CARRY_AFTER + 1;
pub(super) const SCRATCH_VM_RESULT: usize = SCRATCH_VM_DIFFERENCE + 1;
pub(super) const SCRATCH_VM_RESULT_BITS_OFFSET: usize = SCRATCH_VM_RESULT + 1;
pub(super) const SCRATCH_VM_DIFFERENCE_BITS_OFFSET: usize = SCRATCH_VM_RESULT_BITS_OFFSET + 8;

pub(super) const SHA256_INITIAL_STATE_V1: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

pub(super) const SHA256_ROUND_CONSTANTS_V1: [u32; 64] = [
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

/// Stable aggregate AIR descriptor.
pub(crate) const IVM_PRIVATE_NOTE_AGGREGATE_AIR_DESCRIPTOR_V1: &[u8] = b"ivm-private-note-aggregate-air-v1:trace=16384:copy-width=8:copy-lanes=3:sha256-wide-round64-private-io:value=u128-byte-carry:vm=fixed16-private-opcode-byte-state:tree=depth32-private-direction";

/// Aggregate trace construction or algebraic failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(super) enum IvmPrivateNoteAirErrorV1 {
    #[error("private-note AIR relation is invalid")]
    Relation,
    #[error("private-note AIR topology is invalid")]
    Topology,
    #[error("private-note AIR byte assignment is inconsistent")]
    Assignment,
    #[error("private-note AIR resource bound is exceeded")]
    Resource,
    #[error("private-note AIR SHA-256 schedule is invalid")]
    Sha256,
    #[error("private-note AIR copy permutation is invalid")]
    Copy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ByteVariableV1(usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(variant_size_differences)]
enum ByteExpressionV1 {
    Constant(u8),
    Variable(ByteVariableV1),
}

impl ByteExpressionV1 {
    fn value(self, assignment: &[u8]) -> Result<u8, IvmPrivateNoteAirErrorV1> {
        match self {
            Self::Constant(value) => Ok(value),
            Self::Variable(variable) => assignment
                .get(variable.0)
                .copied()
                .ok_or(IvmPrivateNoteAirErrorV1::Assignment),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(variant_size_differences)]
enum CopyCellV1 {
    Inactive,
    Constant(u8),
    Variable(ByteVariableV1),
}

impl CopyCellV1 {
    fn value(self, assignment: &[u8]) -> Result<F, IvmPrivateNoteAirErrorV1> {
        match self {
            Self::Inactive => Ok(F::ZERO),
            Self::Constant(value) => Ok(F(u64::from(value))),
            Self::Variable(variable) => assignment
                .get(variable.0)
                .copied()
                .map(|value| F(u64::from(value)))
                .ok_or(IvmPrivateNoteAirErrorV1::Assignment),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(variant_size_differences)]
pub(super) enum PrivateNoteFixedRowV1 {
    ShaRound {
        round: u8,
        invocation: u8,
        block: u8,
        block_count: u8,
    },
    ShaEnd {
        invocation: u8,
        block: u8,
        block_count: u8,
        digest_chunk: u8,
        public_digest: Option<[u8; 32]>,
    },
    NodeSelect {
        input: u8,
        level: u8,
        byte: u8,
    },
    Distinct {
        comparison: u8,
        chunk: u8,
        chunks: u8,
    },
    NonZero {
        component: u16,
        chunk: u8,
        chunks: u8,
    },
    Sum {
        side: SumSideV1,
        byte: u8,
    },
    VmHeader,
    VmProgram {
        instruction: u8,
    },
    VmPrevious {
        instruction: u8,
        byte: u8,
    },
    VmNext {
        instruction: u8,
        byte: u8,
    },
    Padding,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SumSideV1 {
    Inputs,
    Outputs,
    Conservation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PrivateNoteFixedTraceV1 {
    pub(super) rows: Vec<PrivateNoteFixedRowV1>,
    copy_cells: Vec<[CopyCellV1; PRIVATE_NOTE_COPY_WIDTH_V1]>,
    pub(super) copy_sigma: Vec<[u32; PRIVATE_NOTE_COPY_WIDTH_V1]>,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct PrivateNoteBaseTraceV1 {
    pub(super) fixed: PrivateNoteFixedTraceV1,
    pub(super) rows: Vec<Vec<F>>,
}

impl core::fmt::Debug for PrivateNoteBaseTraceV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PrivateNoteBaseTraceV1")
            .field("row_count", &self.rows.len())
            .field("witness_columns", &"<redacted>")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct NoteVariablesV1 {
    value: [ByteVariableV1; 16],
    authority: [ByteVariableV1; 32],
    rho: [ByteVariableV1; 32],
    blinding: [ByteVariableV1; 32],
    memo: [ByteVariableV1; 32],
}

#[derive(Clone)]
struct InputVariablesV1 {
    note: NoteVariablesV1,
    secret: [ByteVariableV1; 32],
    position_bits: [ByteVariableV1; 32],
    path: [[ByteVariableV1; 32]; PRIVATE_NOTE_TREE_DEPTH_V1],
    commitment: Option<[ByteVariableV1; 32]>,
}

#[derive(Clone)]
struct OutputVariablesV1 {
    note: NoteVariablesV1,
    commitment: Option<[ByteVariableV1; 32]>,
}

struct TraceBuilderV1<'a> {
    statement: &'a IrohaIvmPrivateNoteStarkStatementV1,
    witness: Option<&'a IvmPrivateNoteWitnessV1>,
    assignment: Vec<u8>,
    rows: Vec<Vec<F>>,
    fixed_rows: Vec<PrivateNoteFixedRowV1>,
    copy_cells: Vec<[CopyCellV1; PRIVATE_NOTE_COPY_WIDTH_V1]>,
    hash_invocation_count: usize,
    invocation_oracle: Vec<Sha256InvocationV1>,
    invocation_cursor: usize,
    expected_final_registers: Option<[u128; 8]>,
}

impl<'a> TraceBuilderV1<'a> {
    fn new(
        statement: &'a IrohaIvmPrivateNoteStarkStatementV1,
        witness: Option<&'a IvmPrivateNoteWitnessV1>,
    ) -> Result<Self, IvmPrivateNoteAirErrorV1> {
        let (invocation_oracle, expected_final_registers) = if let Some(witness) = witness {
            let relation = validate_private_note_relation_v1(statement, witness)
                .map_err(|_| IvmPrivateNoteAirErrorV1::Relation)?;
            (relation.invocations, Some(relation.final_registers))
        } else {
            (Vec::new(), None)
        };
        Ok(Self {
            statement,
            witness,
            assignment: Vec::new(),
            rows: Vec::new(),
            fixed_rows: Vec::new(),
            copy_cells: Vec::new(),
            hash_invocation_count: 0,
            invocation_oracle,
            invocation_cursor: 0,
            expected_final_registers,
        })
    }

    fn allocate_bytes<const N: usize>(&mut self, bytes: [u8; N]) -> [ByteVariableV1; N] {
        core::array::from_fn(|index| {
            let variable = ByteVariableV1(self.assignment.len());
            self.assignment.push(bytes[index]);
            variable
        })
    }

    fn assign_bytes<const N: usize>(
        &mut self,
        variables: [ByteVariableV1; N],
        bytes: [u8; N],
    ) -> Result<(), IvmPrivateNoteAirErrorV1> {
        for (variable, byte) in variables.into_iter().zip(bytes) {
            let assigned = self
                .assignment
                .get_mut(variable.0)
                .ok_or(IvmPrivateNoteAirErrorV1::Assignment)?;
            if self.witness.is_some() && *assigned != byte {
                return Err(IvmPrivateNoteAirErrorV1::Assignment);
            }
            *assigned = byte;
        }
        Ok(())
    }

    fn push_row(
        &mut self,
        fixed: PrivateNoteFixedRowV1,
        cells: [CopyCellV1; PRIVATE_NOTE_COPY_WIDTH_V1],
        mut row: Vec<F>,
    ) -> Result<(), IvmPrivateNoteAirErrorV1> {
        if row.len() != PRIVATE_NOTE_BASE_WIDTH_V1 {
            return Err(IvmPrivateNoteAirErrorV1::Topology);
        }
        for (index, cell) in cells.iter().copied().enumerate() {
            row[COPY_OFFSET + index] = cell.value(&self.assignment)?;
        }
        self.fixed_rows.push(fixed);
        self.copy_cells.push(cells);
        self.rows.push(row);
        Ok(())
    }

    fn empty_row() -> Vec<F> {
        vec![F::ZERO; PRIVATE_NOTE_BASE_WIDTH_V1]
    }

    fn check_invocation(
        &mut self,
        role: Sha256InvocationRoleV1,
        message: &[u8],
        digest: [u8; 32],
    ) -> Result<(), IvmPrivateNoteAirErrorV1> {
        if self.witness.is_none() {
            return Ok(());
        }
        let expected = self
            .invocation_oracle
            .get(self.invocation_cursor)
            .ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
        if expected.role != role || expected.preimage != message || expected.digest != digest {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
        self.invocation_cursor += 1;
        Ok(())
    }

    fn push_hash(
        &mut self,
        role: Sha256InvocationRoleV1,
        message: Vec<ByteExpressionV1>,
        digest_variables: [ByteVariableV1; 32],
        public_digest: Option<[u8; 32]>,
    ) -> Result<(), IvmPrivateNoteAirErrorV1> {
        let invocation = u8::try_from(self.hash_invocation_count)
            .map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?;
        let padded = sha256_padding_v1(&message)?;
        let block_count =
            u8::try_from(padded.len() / 64).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?;
        if block_count == 0 || padded.len() % 64 != 0 {
            return Err(IvmPrivateNoteAirErrorV1::Topology);
        }
        let mut state = SHA256_INITIAL_STATE_V1;
        for (block_index, block) in padded.chunks_exact(64).enumerate() {
            let mut schedule = [0_u32; 64];
            for (index, bytes) in block.chunks_exact(4).enumerate() {
                schedule[index] = word_from_expressions(bytes, &self.assignment)?;
            }
            for round in 16..64 {
                schedule[round] = sigma_small_1(schedule[round - 2])
                    .wrapping_add(schedule[round - 7])
                    .wrapping_add(sigma_small_0(schedule[round - 15]))
                    .wrapping_add(schedule[round - 16]);
            }
            let initial = state;
            let mut working = state;
            for round in 0..64 {
                let [a, b, c, d, e, f, g, h] = working;
                let big_1 = sigma_big_1(e);
                let choose = sha_choose(e, f, g);
                let t1_wide = u64::from(h)
                    + u64::from(big_1)
                    + u64::from(choose)
                    + u64::from(SHA256_ROUND_CONSTANTS_V1[round])
                    + u64::from(schedule[round]);
                let t1 = t1_wide as u32;
                let big_0 = sigma_big_0(a);
                let majority = sha_majority(a, b, c);
                let t2_wide = u64::from(big_0) + u64::from(majority);
                let t2 = t2_wide as u32;
                let new_a_wide = u64::from(t1) + u64::from(t2);
                let new_e_wide = u64::from(d) + u64::from(t1);
                let next = [new_a_wide as u32, a, b, c, new_e_wide as u32, e, f, g];
                let mut row = Self::empty_row();
                for (index, value) in schedule.iter().copied().enumerate() {
                    row[SHA_SCHEDULE_OFFSET + index] = F(u64::from(value));
                }
                for (index, value) in initial.iter().copied().enumerate() {
                    row[SHA_INITIAL_STATE_OFFSET + index] = F(u64::from(value));
                }
                for (index, value) in working.iter().copied().enumerate() {
                    row[SHA_STATE_OFFSET + index] = F(u64::from(value));
                }
                for (group, value) in [a, b, c, e, f, g, schedule[round]].into_iter().enumerate() {
                    write_word_bits(&mut row, group, value);
                }
                if round >= 16 {
                    write_word_bits(&mut row, 7, schedule[round - 2]);
                    write_word_bits(&mut row, 8, schedule[round - 15]);
                }
                write_word_bits(&mut row, 9, t1);
                write_word_bits(&mut row, 10, t2);
                row[SHA_T1_OFFSET] = F(u64::from(t1));
                row[SHA_T2_OFFSET] = F(u64::from(t2));
                write_u32_carry(
                    &mut row,
                    SHA_CARRY_OFFSET,
                    u32::try_from(t1_wide >> 32).map_err(|_| IvmPrivateNoteAirErrorV1::Sha256)?,
                    3,
                );
                write_u32_carry(
                    &mut row,
                    SHA_CARRY_OFFSET + 3,
                    u32::try_from(t2_wide >> 32).map_err(|_| IvmPrivateNoteAirErrorV1::Sha256)?,
                    1,
                );
                write_u32_carry(
                    &mut row,
                    SHA_CARRY_OFFSET + 4,
                    u32::try_from(new_a_wide >> 32)
                        .map_err(|_| IvmPrivateNoteAirErrorV1::Sha256)?,
                    1,
                );
                write_u32_carry(
                    &mut row,
                    SHA_CARRY_OFFSET + 5,
                    u32::try_from(new_e_wide >> 32)
                        .map_err(|_| IvmPrivateNoteAirErrorV1::Sha256)?,
                    1,
                );
                if round >= 16 {
                    let schedule_wide = u64::from(sigma_small_1(schedule[round - 2]))
                        + u64::from(schedule[round - 7])
                        + u64::from(sigma_small_0(schedule[round - 15]))
                        + u64::from(schedule[round - 16]);
                    write_u32_carry(
                        &mut row,
                        SHA_CARRY_OFFSET + 6,
                        u32::try_from(schedule_wide >> 32)
                            .map_err(|_| IvmPrivateNoteAirErrorV1::Sha256)?,
                        2,
                    );
                }
                if round == 63 {
                    for index in 0..8 {
                        let feed_forward = u64::from(initial[index]) + u64::from(next[index]);
                        write_u32_carry(
                            &mut row,
                            SHA_CARRY_OFFSET + 8 + index,
                            u32::try_from(feed_forward >> 32)
                                .map_err(|_| IvmPrivateNoteAirErrorV1::Sha256)?,
                            1,
                        );
                    }
                }
                let cells = if round < 16 {
                    copy_cells_for_word(&block[round * 4..round * 4 + 4])?
                } else {
                    [CopyCellV1::Inactive; PRIVATE_NOTE_COPY_WIDTH_V1]
                };
                self.push_row(
                    PrivateNoteFixedRowV1::ShaRound {
                        round: u8::try_from(round)
                            .map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                        invocation,
                        block: u8::try_from(block_index)
                            .map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                        block_count,
                    },
                    cells,
                    row,
                )?;
                working = next;
            }
            state = core::array::from_fn(|index| initial[index].wrapping_add(working[index]));
            let terminal = block_index + 1 == usize::from(block_count);
            for digest_chunk in 0..4 {
                let mut row = Self::empty_row();
                for (index, value) in state.iter().copied().enumerate() {
                    row[SHA_STATE_OFFSET + index] = F(u64::from(value));
                    write_word_bits(&mut row, index, value);
                }
                let mut cells = [CopyCellV1::Inactive; PRIVATE_NOTE_COPY_WIDTH_V1];
                if terminal {
                    let first = digest_chunk * PRIVATE_NOTE_COPY_WIDTH_V1;
                    for (cell, variable) in cells
                        .iter_mut()
                        .zip(digest_variables[first..first + PRIVATE_NOTE_COPY_WIDTH_V1].iter())
                    {
                        *cell = CopyCellV1::Variable(*variable);
                    }
                }
                self.push_row(
                    PrivateNoteFixedRowV1::ShaEnd {
                        invocation,
                        block: u8::try_from(block_index)
                            .map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                        block_count,
                        digest_chunk: u8::try_from(digest_chunk)
                            .map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                        public_digest: terminal.then_some(public_digest).flatten(),
                    },
                    cells,
                    row,
                )?;
            }
        }
        let digest: [u8; 32] = state
            .into_iter()
            .flat_map(u32::to_be_bytes)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| IvmPrivateNoteAirErrorV1::Sha256)?;
        self.assign_bytes(digest_variables, digest)?;
        if public_digest.is_some_and(|expected| expected != digest) && self.witness.is_some() {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
        let raw_message = message
            .iter()
            .copied()
            .map(|byte| byte.value(&self.assignment))
            .collect::<Result<Vec<_>, _>>()?;
        self.check_invocation(role, &raw_message, digest)?;
        self.hash_invocation_count = self
            .hash_invocation_count
            .checked_add(1)
            .ok_or(IvmPrivateNoteAirErrorV1::Resource)?;
        Ok(())
    }

    fn next_oracle_digest(
        &self,
        role: Sha256InvocationRoleV1,
    ) -> Result<[u8; 32], IvmPrivateNoteAirErrorV1> {
        if self.witness.is_none() {
            return Ok([0; 32]);
        }
        self.invocation_oracle
            .get(self.invocation_cursor)
            .filter(|invocation| invocation.role == role)
            .map(|invocation| invocation.digest)
            .ok_or(IvmPrivateNoteAirErrorV1::Topology)
    }

    fn allocate_note(
        &mut self,
        note: Option<super::relation::PrivateNotePlaintextV1>,
    ) -> NoteVariablesV1 {
        let note = note.unwrap_or(super::relation::PrivateNotePlaintextV1 {
            value: 0,
            spending_authority: [0; 32],
            rho: [0; 32],
            blinding: [0; 32],
            memo_digest: [0; 32],
        });
        NoteVariablesV1 {
            value: self.allocate_bytes(note.value.to_be_bytes()),
            authority: self.allocate_bytes(note.spending_authority),
            rho: self.allocate_bytes(note.rho),
            blinding: self.allocate_bytes(note.blinding),
            memo: self.allocate_bytes(note.memo_digest),
        }
    }

    fn push_node_select(
        &mut self,
        input: u8,
        level: u8,
        position_bit: ByteVariableV1,
        current: [ByteVariableV1; 32],
        sibling: [ByteVariableV1; 32],
    ) -> Result<([ByteVariableV1; 32], [ByteVariableV1; 32]), IvmPrivateNoteAirErrorV1> {
        let bit = self.assignment[position_bit.0];
        if self.witness.is_some() && bit > 1 {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
        let left_bytes = core::array::from_fn(|index| {
            if bit == 0 {
                self.assignment[current[index].0]
            } else {
                self.assignment[sibling[index].0]
            }
        });
        let right_bytes = core::array::from_fn(|index| {
            if bit == 0 {
                self.assignment[sibling[index].0]
            } else {
                self.assignment[current[index].0]
            }
        });
        let left = self.allocate_bytes(left_bytes);
        let right = self.allocate_bytes(right_bytes);
        for byte in 0..32 {
            let cells = [
                CopyCellV1::Variable(current[byte]),
                CopyCellV1::Variable(sibling[byte]),
                CopyCellV1::Variable(left[byte]),
                CopyCellV1::Variable(right[byte]),
                CopyCellV1::Variable(position_bit),
                CopyCellV1::Inactive,
                CopyCellV1::Inactive,
                CopyCellV1::Inactive,
            ];
            self.push_row(
                PrivateNoteFixedRowV1::NodeSelect {
                    input,
                    level,
                    byte: u8::try_from(byte).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                },
                cells,
                Self::empty_row(),
            )?;
        }
        Ok((left, right))
    }

    fn push_distinct(
        &mut self,
        comparison: u8,
        left: &[ByteVariableV1],
        right: &[ByteVariableV1],
    ) -> Result<(), IvmPrivateNoteAirErrorV1> {
        const PAIRS_PER_ROW: usize = PRIVATE_NOTE_COPY_WIDTH_V1 / 2;

        if left.is_empty() || left.len() != right.len() || left.len() > 32 {
            return Err(IvmPrivateNoteAirErrorV1::Topology);
        }
        let chunks = left.len().div_ceil(PAIRS_PER_ROW);
        let selected = left
            .iter()
            .zip(right)
            .position(|(left, right)| self.assignment[left.0] != self.assignment[right.0]);
        if self.witness.is_some() && selected.is_none() {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
        let mut running = 0_u8;
        for chunk in 0..chunks {
            let start = chunk * PAIRS_PER_ROW;
            let end = (start + PAIRS_PER_ROW).min(left.len());
            let mut cells = [CopyCellV1::Inactive; PRIVATE_NOTE_COPY_WIDTH_V1];
            for pair in start..end {
                let within = pair - start;
                cells[within * 2] = CopyCellV1::Variable(left[pair]);
                cells[within * 2 + 1] = CopyCellV1::Variable(right[pair]);
            }
            let mut row = Self::empty_row();
            row[SCRATCH_RUNNING_BEFORE] = F(u64::from(running));
            if let Some(selected) = selected
                && (start..end).contains(&selected)
            {
                let within = selected - start;
                let left_byte = self.assignment[left[selected].0];
                let right_byte = self.assignment[right[selected].0];
                let selected_bit = (left_byte ^ right_byte).trailing_zeros() as usize;
                row[SCRATCH_NONZERO_BYTE_SELECT_OFFSET + within] = F::ONE;
                row[SCRATCH_NONZERO_BIT_SELECT_OFFSET + selected_bit] = F::ONE;
                for bit in 0..8 {
                    row[SCRATCH_BYTE_BITS_OFFSET + bit] = F(u64::from((left_byte >> bit) & 1));
                    row[SCRATCH_VM_DIFFERENCE_BITS_OFFSET + bit] =
                        F(u64::from((right_byte >> bit) & 1));
                }
                running = 1;
            }
            row[SCRATCH_RUNNING_AFTER] = F(u64::from(running));
            self.push_row(
                PrivateNoteFixedRowV1::Distinct {
                    comparison,
                    chunk: u8::try_from(chunk).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                    chunks: u8::try_from(chunks).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                },
                cells,
                row,
            )?;
        }
        Ok(())
    }

    fn push_nonzero(
        &mut self,
        component: u16,
        variables: &[ByteVariableV1],
    ) -> Result<(), IvmPrivateNoteAirErrorV1> {
        if variables.is_empty() || variables.len() > 32 {
            return Err(IvmPrivateNoteAirErrorV1::Topology);
        }
        let chunks = variables.len().div_ceil(PRIVATE_NOTE_COPY_WIDTH_V1);
        let selected = variables
            .iter()
            .position(|variable| self.assignment[variable.0] != 0);
        if self.witness.is_some() && selected.is_none() {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
        let mut running = 0_u8;
        for chunk in 0..chunks {
            let start = chunk * PRIVATE_NOTE_COPY_WIDTH_V1;
            let end = (start + PRIVATE_NOTE_COPY_WIDTH_V1).min(variables.len());
            let mut cells = [CopyCellV1::Inactive; PRIVATE_NOTE_COPY_WIDTH_V1];
            for (cell, variable) in cells.iter_mut().zip(&variables[start..end]) {
                *cell = CopyCellV1::Variable(*variable);
            }
            let mut row = Self::empty_row();
            row[SCRATCH_RUNNING_BEFORE] = F(u64::from(running));
            if let Some(selected) = selected
                && (start..end).contains(&selected)
            {
                let within = selected - start;
                row[SCRATCH_NONZERO_BYTE_SELECT_OFFSET + within] = F::ONE;
                let byte = self.assignment[variables[selected].0];
                let selected_bit = byte.trailing_zeros() as usize;
                row[SCRATCH_NONZERO_BIT_SELECT_OFFSET + selected_bit] = F::ONE;
                for bit in 0..8 {
                    row[SCRATCH_BYTE_BITS_OFFSET + bit] = F(u64::from((byte >> bit) & 1));
                }
                running = 1;
            }
            row[SCRATCH_RUNNING_AFTER] = F(u64::from(running));
            self.push_row(
                PrivateNoteFixedRowV1::NonZero {
                    component,
                    chunk: u8::try_from(chunk).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                    chunks: u8::try_from(chunks).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                },
                cells,
                row,
            )?;
        }
        Ok(())
    }

    fn push_sum(
        &mut self,
        side: SumSideV1,
        operands: &[[ByteVariableV1; 16]],
        sum: u128,
    ) -> Result<[ByteVariableV1; 16], IvmPrivateNoteAirErrorV1> {
        if operands.is_empty() || operands.len() > 2 {
            return Err(IvmPrivateNoteAirErrorV1::Topology);
        }
        let sum_variables = self.allocate_bytes(sum.to_be_bytes());
        let mut carry = 0_u16;
        for little_byte in 0..16 {
            let byte = 15 - little_byte;
            let wide = operands.iter().fold(u16::from(carry), |value, operand| {
                value + u16::from(self.assignment[operand[byte].0])
            });
            let output = self.assignment[sum_variables[byte].0];
            if self.witness.is_some() && u16::from(output) != (wide & 0xff) {
                return Err(IvmPrivateNoteAirErrorV1::Assignment);
            }
            let next_carry = wide >> 8;
            let mut cells = [CopyCellV1::Inactive; PRIVATE_NOTE_COPY_WIDTH_V1];
            for (cell, operand) in cells.iter_mut().zip(operands) {
                *cell = CopyCellV1::Variable(operand[byte]);
            }
            cells[2] = CopyCellV1::Variable(sum_variables[byte]);
            let mut row = Self::empty_row();
            row[SCRATCH_RELATION_CARRY_BEFORE] = F(u64::from(carry));
            row[SCRATCH_RELATION_CARRY_AFTER] = F(u64::from(next_carry));
            for bit in 0..8 {
                row[SCRATCH_BYTE_BITS_OFFSET + bit] = F(u64::from((output >> bit) & 1));
            }
            for bit in 0..2 {
                row[SCRATCH_RELATION_CARRY_BITS_OFFSET + bit] =
                    F(u64::from((next_carry >> bit) & 1));
            }
            self.push_row(
                PrivateNoteFixedRowV1::Sum {
                    side,
                    byte: u8::try_from(little_byte)
                        .map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                },
                cells,
                row,
            )?;
            carry = next_carry;
        }
        if self.witness.is_some() && carry != 0 {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
        Ok(sum_variables)
    }

    fn push_conservation(
        &mut self,
        input_sum: [ByteVariableV1; 16],
        output_sum: [ByteVariableV1; 16],
    ) -> Result<(), IvmPrivateNoteAirErrorV1> {
        let (public_in, public_out) = public_balance_sides(self.statement.value_balance);
        let public_in = public_in.to_be_bytes();
        let public_out = public_out.to_be_bytes();
        let mut carry = 0_i16;
        for little_byte in 0..16 {
            let byte = 15 - little_byte;
            let left =
                i16::from(self.assignment[input_sum[byte].0]) + i16::from(public_in[byte]) + carry;
            let right =
                i16::from(self.assignment[output_sum[byte].0]) + i16::from(public_out[byte]);
            let difference = left - right;
            if self.witness.is_some() && difference.rem_euclid(256) != 0 {
                return Err(IvmPrivateNoteAirErrorV1::Assignment);
            }
            let next_carry = difference.div_euclid(256);
            if !(-1..=1).contains(&next_carry) {
                return Err(IvmPrivateNoteAirErrorV1::Assignment);
            }
            let cells = [
                CopyCellV1::Variable(input_sum[byte]),
                CopyCellV1::Variable(output_sum[byte]),
                CopyCellV1::Constant(public_in[byte]),
                CopyCellV1::Constant(public_out[byte]),
                CopyCellV1::Inactive,
                CopyCellV1::Inactive,
                CopyCellV1::Inactive,
                CopyCellV1::Inactive,
            ];
            let mut row = Self::empty_row();
            row[SCRATCH_RELATION_CARRY_BEFORE] = signed_small_field(carry);
            row[SCRATCH_RELATION_CARRY_AFTER] = signed_small_field(next_carry);
            self.push_row(
                PrivateNoteFixedRowV1::Sum {
                    side: SumSideV1::Conservation,
                    byte: u8::try_from(little_byte)
                        .map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                },
                cells,
                row,
            )?;
            carry = next_carry;
        }
        if self.witness.is_some() && carry != 0 {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
        Ok(())
    }

    fn write_vm_scratch(
        row: &mut [F],
        instruction: PrivateInstructionV1,
        halted_before: bool,
        halted_after: bool,
    ) {
        row[SCRATCH_VM_OPCODE_SELECT_OFFSET + instruction.opcode as usize] = F::ONE;
        row[SCRATCH_VM_DESTINATION_SELECT_OFFSET + usize::from(instruction.destination)] = F::ONE;
        row[SCRATCH_VM_LEFT_SELECT_OFFSET + usize::from(instruction.left)] = F::ONE;
        row[SCRATCH_VM_RIGHT_SELECT_OFFSET + usize::from(instruction.right)] = F::ONE;
        for (index, byte) in instruction.immediate.to_be_bytes().into_iter().enumerate() {
            row[SCRATCH_VM_IMMEDIATE_OFFSET + index] = F(u64::from(byte));
        }
        row[SCRATCH_VM_HALTED_BEFORE] = F(u64::from(halted_before));
        row[SCRATCH_VM_HALTED_AFTER] = F(u64::from(halted_after));
    }

    fn execute_vm_states(
        &self,
        instructions: [PrivateInstructionV1; PRIVATE_PROGRAM_INSTRUCTION_COUNT_V1],
        input_sum: u128,
        output_sum: u128,
    ) -> Result<Vec<[u128; 8]>, IvmPrivateNoteAirErrorV1> {
        let (public_in, public_out) = public_balance_sides(self.statement.value_balance);
        let mut state = [
            input_sum,
            output_sum,
            public_in,
            public_out,
            0,
            u128::from(self.statement.execution_epoch),
            0,
            1,
        ];
        let mut states = Vec::with_capacity(PRIVATE_PROGRAM_INSTRUCTION_COUNT_V1 + 1);
        states.push(state);
        for instruction in instructions {
            let destination = usize::from(instruction.destination);
            let left = usize::from(instruction.left);
            let right = usize::from(instruction.right);
            match instruction.opcode {
                PrivateOpcodeV1::Halt => {}
                PrivateOpcodeV1::MoveImmediate => {
                    state[destination] = u128::from(instruction.immediate);
                }
                PrivateOpcodeV1::Move => state[destination] = state[left],
                PrivateOpcodeV1::AddChecked => {
                    state[destination] = state[left]
                        .checked_add(state[right])
                        .ok_or(IvmPrivateNoteAirErrorV1::Assignment)?;
                }
                PrivateOpcodeV1::SubChecked => {
                    state[destination] = state[left]
                        .checked_sub(state[right])
                        .ok_or(IvmPrivateNoteAirErrorV1::Assignment)?;
                }
                PrivateOpcodeV1::AssertEqual => {
                    if state[left] != state[right] {
                        return Err(IvmPrivateNoteAirErrorV1::Assignment);
                    }
                }
                PrivateOpcodeV1::AssertLessOrEqual => {
                    if state[left] > state[right] {
                        return Err(IvmPrivateNoteAirErrorV1::Assignment);
                    }
                }
                PrivateOpcodeV1::LoadActionLimb => {
                    let start = usize::try_from(instruction.immediate)
                        .map_err(|_| IvmPrivateNoteAirErrorV1::Assignment)?
                        .checked_mul(16)
                        .ok_or(IvmPrivateNoteAirErrorV1::Assignment)?;
                    let limb = self
                        .statement
                        .action_digest
                        .as_bytes()
                        .get(start..start + 16)
                        .ok_or(IvmPrivateNoteAirErrorV1::Assignment)?;
                    state[destination] = u128::from_be_bytes(
                        limb.try_into()
                            .map_err(|_| IvmPrivateNoteAirErrorV1::Assignment)?,
                    );
                }
                PrivateOpcodeV1::LoadExecutionEpoch => {
                    state[destination] = u128::from(self.statement.execution_epoch);
                }
            }
            states.push(state);
        }
        Ok(states)
    }

    fn push_vm(
        &mut self,
        program_variables: &[ByteVariableV1; super::codec::PRIVATE_PROGRAM_BYTES_V1],
        input_sum_variables: [ByteVariableV1; 16],
        output_sum_variables: [ByteVariableV1; 16],
        input_sum: u128,
        output_sum: u128,
    ) -> Result<(), IvmPrivateNoteAirErrorV1> {
        let instructions = self.witness.map_or(
            [PrivateInstructionV1::HALT; PRIVATE_PROGRAM_INSTRUCTION_COUNT_V1],
            |witness| witness.program.instructions,
        );
        let states = self.execute_vm_states(instructions, input_sum, output_sum)?;
        if self
            .expected_final_registers
            .is_some_and(|expected| states.last().copied() != Some(expected))
        {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
        let (public_in, public_out) = public_balance_sides(self.statement.value_balance);
        let initial_constants = [
            None,
            None,
            Some(public_in.to_be_bytes()),
            Some(public_out.to_be_bytes()),
            Some(0_u128.to_be_bytes()),
            Some(u128::from(self.statement.execution_epoch).to_be_bytes()),
            Some(0_u128.to_be_bytes()),
            Some(1_u128.to_be_bytes()),
        ];
        let mut registers: [[ByteExpressionV1; 16]; 8] = [[ByteExpressionV1::Constant(0); 16]; 8];
        registers[0] = input_sum_variables.map(ByteExpressionV1::Variable);
        registers[1] = output_sum_variables.map(ByteExpressionV1::Variable);
        for register in 2..8 {
            let bytes = initial_constants[register].ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
            registers[register] = bytes.map(ByteExpressionV1::Constant);
        }
        let mut header_cells = [CopyCellV1::Inactive; PRIVATE_NOTE_COPY_WIDTH_V1];
        for (cell, variable) in header_cells.iter_mut().zip(&program_variables[..8]) {
            *cell = CopyCellV1::Variable(*variable);
        }
        self.push_row(
            PrivateNoteFixedRowV1::VmHeader,
            header_cells,
            Self::empty_row(),
        )?;
        let mut halted = false;
        for (instruction_index, instruction) in instructions.into_iter().enumerate() {
            let halted_before = halted;
            halted |= instruction.opcode == PrivateOpcodeV1::Halt;
            let program_offset = 8 + instruction_index * 8;
            let mut program_cells = [CopyCellV1::Inactive; PRIVATE_NOTE_COPY_WIDTH_V1];
            for (cell, variable) in program_cells
                .iter_mut()
                .zip(&program_variables[program_offset..program_offset + 8])
            {
                *cell = CopyCellV1::Variable(*variable);
            }
            let mut program_row = Self::empty_row();
            Self::write_vm_scratch(&mut program_row, instruction, halted_before, halted);
            self.push_row(
                PrivateNoteFixedRowV1::VmProgram {
                    instruction: u8::try_from(instruction_index)
                        .map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                },
                program_cells,
                program_row,
            )?;

            let next_state = states[instruction_index + 1];
            let next_registers: [[ByteVariableV1; 16]; 8] = core::array::from_fn(|register| {
                self.allocate_bytes(next_state[register].to_be_bytes())
            });
            let left_value = states[instruction_index][usize::from(instruction.left)];
            let right_value = states[instruction_index][usize::from(instruction.right)];
            let difference = if instruction.opcode == PrivateOpcodeV1::AssertLessOrEqual {
                right_value
                    .checked_sub(left_value)
                    .ok_or(IvmPrivateNoteAirErrorV1::Assignment)?
            } else {
                0
            };
            let difference_bytes = difference.to_be_bytes();
            let writes = matches!(
                instruction.opcode,
                PrivateOpcodeV1::MoveImmediate
                    | PrivateOpcodeV1::Move
                    | PrivateOpcodeV1::AddChecked
                    | PrivateOpcodeV1::SubChecked
                    | PrivateOpcodeV1::LoadActionLimb
                    | PrivateOpcodeV1::LoadExecutionEpoch
            );
            let mut carry = 0_u16;
            for little_byte in 0..16 {
                let byte = 15 - little_byte;
                let mut previous_cells = [CopyCellV1::Inactive; PRIVATE_NOTE_COPY_WIDTH_V1];
                for (cell, register) in previous_cells.iter_mut().zip(&registers) {
                    *cell = match register[byte] {
                        ByteExpressionV1::Constant(value) => CopyCellV1::Constant(value),
                        ByteExpressionV1::Variable(variable) => CopyCellV1::Variable(variable),
                    };
                }
                let mut previous_row = Self::empty_row();
                Self::write_vm_scratch(&mut previous_row, instruction, halted_before, halted);
                let mut previous_bytes = [0_u8; 8];
                for register in 0..8 {
                    previous_bytes[register] = registers[register][byte].value(&self.assignment)?;
                }
                let left_byte = previous_bytes[usize::from(instruction.left)];
                let right_byte = previous_bytes[usize::from(instruction.right)];
                let result = if writes {
                    next_state[usize::from(instruction.destination)].to_be_bytes()[byte]
                } else {
                    0
                };
                let wide = match instruction.opcode {
                    PrivateOpcodeV1::AddChecked => {
                        u16::from(left_byte) + u16::from(right_byte) + carry
                    }
                    PrivateOpcodeV1::SubChecked => {
                        u16::from(result) + u16::from(right_byte) + carry
                    }
                    PrivateOpcodeV1::AssertLessOrEqual => {
                        u16::from(difference_bytes[byte]) + u16::from(left_byte) + carry
                    }
                    _ => u16::from(result),
                };
                let next_carry = match instruction.opcode {
                    PrivateOpcodeV1::AddChecked => wide >> 8,
                    PrivateOpcodeV1::SubChecked => {
                        let left = u16::from(left_byte);
                        if self.witness.is_some() && (wide & 0xff) != left {
                            return Err(IvmPrivateNoteAirErrorV1::Assignment);
                        }
                        wide >> 8
                    }
                    PrivateOpcodeV1::AssertLessOrEqual => {
                        let right = u16::from(right_byte);
                        if self.witness.is_some() && (wide & 0xff) != right {
                            return Err(IvmPrivateNoteAirErrorV1::Assignment);
                        }
                        wide >> 8
                    }
                    _ => 0,
                };
                previous_row[SCRATCH_VM_CARRY_BEFORE] = F(u64::from(carry));
                previous_row[SCRATCH_VM_CARRY_AFTER] = F(u64::from(next_carry));
                previous_row[SCRATCH_VM_DIFFERENCE] = F(u64::from(difference_bytes[byte]));
                previous_row[SCRATCH_VM_RESULT] = F(u64::from(result));
                for bit in 0..8 {
                    previous_row[SCRATCH_VM_DIFFERENCE_BITS_OFFSET + bit] =
                        F(u64::from((difference_bytes[byte] >> bit) & 1));
                    previous_row[SCRATCH_VM_RESULT_BITS_OFFSET + bit] =
                        F(u64::from((result >> bit) & 1));
                }
                self.push_row(
                    PrivateNoteFixedRowV1::VmPrevious {
                        instruction: u8::try_from(instruction_index)
                            .map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                        byte: u8::try_from(little_byte)
                            .map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                    },
                    previous_cells,
                    previous_row,
                )?;

                let mut next_cells = [CopyCellV1::Inactive; PRIVATE_NOTE_COPY_WIDTH_V1];
                for (cell, register) in next_cells.iter_mut().zip(&next_registers) {
                    *cell = CopyCellV1::Variable(register[byte]);
                }
                let mut next_row = Self::empty_row();
                Self::write_vm_scratch(&mut next_row, instruction, halted_before, halted);
                self.push_row(
                    PrivateNoteFixedRowV1::VmNext {
                        instruction: u8::try_from(instruction_index)
                            .map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                        byte: u8::try_from(little_byte)
                            .map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                    },
                    next_cells,
                    next_row,
                )?;
                carry = next_carry;
            }
            if self.witness.is_some()
                && matches!(
                    instruction.opcode,
                    PrivateOpcodeV1::AddChecked
                        | PrivateOpcodeV1::SubChecked
                        | PrivateOpcodeV1::AssertLessOrEqual
                )
                && carry != 0
            {
                return Err(IvmPrivateNoteAirErrorV1::Assignment);
            }
            registers = next_registers.map(|register| register.map(ByteExpressionV1::Variable));
        }
        Ok(())
    }
}

fn variables_as_expressions<const N: usize>(
    variables: &[ByteVariableV1; N],
) -> Vec<ByteExpressionV1> {
    variables
        .iter()
        .copied()
        .map(ByteExpressionV1::Variable)
        .collect()
}

fn constants_as_expressions(bytes: &[u8]) -> Vec<ByteExpressionV1> {
    bytes
        .iter()
        .copied()
        .map(ByteExpressionV1::Constant)
        .collect()
}

fn frame_expressions_v1(
    domain: &[u8],
    fields: &[Vec<ByteExpressionV1>],
) -> Result<Vec<ByteExpressionV1>, IvmPrivateNoteAirErrorV1> {
    let domain_len = u16::try_from(domain.len()).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?;
    let field_count =
        u16::try_from(fields.len()).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?;
    let capacity = HASH_FRAME_DOMAIN_V1
        .len()
        .checked_add(2)
        .and_then(|value| value.checked_add(domain.len()))
        .and_then(|value| value.checked_add(2))
        .and_then(|value| {
            fields.iter().try_fold(value, |length, field| {
                length.checked_add(8)?.checked_add(field.len())
            })
        })
        .ok_or(IvmPrivateNoteAirErrorV1::Resource)?;
    let mut message = Vec::new();
    message
        .try_reserve_exact(capacity)
        .map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?;
    message.extend(constants_as_expressions(HASH_FRAME_DOMAIN_V1));
    message.extend(constants_as_expressions(&domain_len.to_be_bytes()));
    message.extend(constants_as_expressions(domain));
    message.extend(constants_as_expressions(&field_count.to_be_bytes()));
    for field in fields {
        let length = u64::try_from(field.len()).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?;
        message.extend(constants_as_expressions(&length.to_be_bytes()));
        message.extend(field.iter().copied());
    }
    if message.len() != capacity {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    Ok(message)
}

fn note_commitment_fields(note: &NoteVariablesV1) -> Vec<Vec<ByteExpressionV1>> {
    vec![
        variables_as_expressions(&note.value),
        variables_as_expressions(&note.authority),
        variables_as_expressions(&note.rho),
        variables_as_expressions(&note.blinding),
        variables_as_expressions(&note.memo),
    ]
}

fn sha256_padding_v1(
    message: &[ByteExpressionV1],
) -> Result<Vec<ByteExpressionV1>, IvmPrivateNoteAirErrorV1> {
    let bit_len = u64::try_from(message.len())
        .map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?
        .checked_mul(8)
        .ok_or(IvmPrivateNoteAirErrorV1::Resource)?;
    let mut padded = message.to_vec();
    padded.push(ByteExpressionV1::Constant(0x80));
    while padded.len() % 64 != 56 {
        padded.push(ByteExpressionV1::Constant(0));
    }
    padded.extend(constants_as_expressions(&bit_len.to_be_bytes()));
    Ok(padded)
}

fn sigma_small_0(value: u32) -> u32 {
    value.rotate_right(7) ^ value.rotate_right(18) ^ (value >> 3)
}

fn sigma_small_1(value: u32) -> u32 {
    value.rotate_right(17) ^ value.rotate_right(19) ^ (value >> 10)
}

fn sigma_big_0(value: u32) -> u32 {
    value.rotate_right(2) ^ value.rotate_right(13) ^ value.rotate_right(22)
}

fn sigma_big_1(value: u32) -> u32 {
    value.rotate_right(6) ^ value.rotate_right(11) ^ value.rotate_right(25)
}

fn sha_choose(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (!x & z)
}

fn sha_majority(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (x & z) ^ (y & z)
}

fn write_word_bits(row: &mut [F], group: usize, value: u32) {
    let start = SHA_BITS_OFFSET + group * PRIVATE_NOTE_SHA_BITS_PER_GROUP_V1;
    for bit in 0..32 {
        row[start + bit] = F(u64::from((value >> bit) & 1));
    }
}

fn write_u32_carry(row: &mut [F], offset: usize, value: u32, bits: usize) {
    for bit in 0..bits {
        row[offset + bit] = F(u64::from((value >> bit) & 1));
    }
}

fn signed_small_field(value: i16) -> F {
    if value >= 0 {
        F(value as u64)
    } else {
        F::ZERO.sub(F(u64::from(value.unsigned_abs())))
    }
}

fn copy_cells_for_word(
    bytes: &[ByteExpressionV1],
) -> Result<[CopyCellV1; PRIVATE_NOTE_COPY_WIDTH_V1], IvmPrivateNoteAirErrorV1> {
    if bytes.len() != 4 {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    let mut cells = [CopyCellV1::Inactive; PRIVATE_NOTE_COPY_WIDTH_V1];
    for (cell, expression) in cells.iter_mut().zip(bytes.iter().copied()) {
        *cell = match expression {
            ByteExpressionV1::Constant(value) => CopyCellV1::Constant(value),
            ByteExpressionV1::Variable(variable) => CopyCellV1::Variable(variable),
        };
    }
    Ok(cells)
}

fn word_from_expressions(
    bytes: &[ByteExpressionV1],
    assignment: &[u8],
) -> Result<u32, IvmPrivateNoteAirErrorV1> {
    if bytes.len() != 4 {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    Ok(u32::from_be_bytes([
        bytes[0].value(assignment)?,
        bytes[1].value(assignment)?,
        bytes[2].value(assignment)?,
        bytes[3].value(assignment)?,
    ]))
}

fn build_copy_sigma_v1(
    cells: &[[CopyCellV1; PRIVATE_NOTE_COPY_WIDTH_V1]],
) -> Result<Vec<[u32; PRIVATE_NOTE_COPY_WIDTH_V1]>, IvmPrivateNoteAirErrorV1> {
    let mut occurrences = BTreeMap::<ByteVariableV1, Vec<(usize, usize)>>::new();
    for (row, cells) in cells.iter().enumerate() {
        for (column, cell) in cells.iter().copied().enumerate() {
            if let CopyCellV1::Variable(variable) = cell {
                occurrences.entry(variable).or_default().push((row, column));
            }
        }
    }
    let mut sigma = vec![[0_u32; PRIVATE_NOTE_COPY_WIDTH_V1]; cells.len()];
    for (row, row_sigma) in sigma.iter_mut().enumerate() {
        for (column, value) in row_sigma.iter_mut().enumerate() {
            let identity = row
                .checked_mul(PRIVATE_NOTE_COPY_WIDTH_V1)
                .and_then(|value| value.checked_add(column))
                .and_then(|value| value.checked_add(1))
                .ok_or(IvmPrivateNoteAirErrorV1::Resource)?;
            *value = u32::try_from(identity).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?;
        }
    }
    for positions in occurrences.values() {
        for (index, &(row, column)) in positions.iter().enumerate() {
            let (next_row, next_column) = positions[(index + 1) % positions.len()];
            let label = next_row
                .checked_mul(PRIVATE_NOTE_COPY_WIDTH_V1)
                .and_then(|value| value.checked_add(next_column))
                .and_then(|value| value.checked_add(1))
                .ok_or(IvmPrivateNoteAirErrorV1::Resource)?;
            sigma[row][column] =
                u32::try_from(label).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?;
        }
    }
    Ok(sigma)
}

fn dummy_path() -> [[u8; 32]; PRIVATE_NOTE_TREE_DEPTH_V1] {
    [[0; 32]; PRIVATE_NOTE_TREE_DEPTH_V1]
}

fn build_private_note_trace_v1(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
    witness: Option<&IvmPrivateNoteWitnessV1>,
) -> Result<PrivateNoteBaseTraceV1, IvmPrivateNoteAirErrorV1> {
    validate_statement_v1(statement).map_err(|_| IvmPrivateNoteAirErrorV1::Relation)?;
    if statement.nullifiers.is_empty()
        || statement.nullifiers.len() > PRIVATE_NOTE_MAX_INPUTS_V1
        || statement.output_commitments.is_empty()
        || statement.output_commitments.len() > PRIVATE_NOTE_MAX_OUTPUTS_V1
    {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    let mut builder = TraceBuilderV1::new(statement, witness)?;
    let program_bytes = witness
        .map(|witness| encode_private_program_v1(&witness.program))
        .transpose()
        .map_err(|_| IvmPrivateNoteAirErrorV1::Relation)?
        .unwrap_or([0; super::codec::PRIVATE_PROGRAM_BYTES_V1]);
    let program_variables = builder.allocate_bytes(program_bytes);

    let mut input_variables = Vec::with_capacity(statement.nullifiers.len());
    for index in 0..statement.nullifiers.len() {
        let input = witness.and_then(|witness| witness.inputs.get(index));
        let note = builder.allocate_note(input.map(|input| input.note.clone()));
        let secret = builder.allocate_bytes(input.map_or([0; 32], |input| input.spending_secret));
        let position = input.map_or(0, |input| input.leaf_position);
        let position_bits =
            builder.allocate_bytes(core::array::from_fn(|bit| ((position >> bit) & 1) as u8));
        let path_bytes = input.map_or_else(dummy_path, |input| input.authentication_path);
        let path = core::array::from_fn(|level| builder.allocate_bytes(path_bytes[level]));
        input_variables.push(InputVariablesV1 {
            note,
            secret,
            position_bits,
            path,
            commitment: None,
        });
    }
    let mut output_variables = Vec::with_capacity(statement.output_commitments.len());
    for index in 0..statement.output_commitments.len() {
        let output = witness.and_then(|witness| witness.outputs.get(index));
        output_variables.push(OutputVariablesV1 {
            note: builder.allocate_note(output.map(|output| output.note.clone())),
            commitment: None,
        });
    }

    let program_digest = builder.allocate_bytes(*statement.program_id.as_bytes());
    let program_message = frame_expressions_v1(
        PROGRAM_ID_DOMAIN_V1,
        &[variables_as_expressions(&program_variables)],
    )?;
    builder.push_hash(
        Sha256InvocationRoleV1::Program,
        program_message,
        program_digest,
        Some(*statement.program_id.as_bytes()),
    )?;

    let namespace = norito::to_bytes(&namespace_v1(statement))
        .map_err(|_| IvmPrivateNoteAirErrorV1::Topology)?;
    let mut nonzero_components = Vec::<Vec<ByteVariableV1>>::new();
    for (index, input) in input_variables.iter_mut().enumerate() {
        let input_index = u8::try_from(index).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?;
        let authority_message = frame_expressions_v1(
            NOTE_AUTHORITY_DOMAIN_V1,
            &[variables_as_expressions(&input.secret)],
        )?;
        builder.push_hash(
            Sha256InvocationRoleV1::Authority { input: input_index },
            authority_message,
            input.note.authority,
            None,
        )?;

        let commitment_digest = builder
            .next_oracle_digest(Sha256InvocationRoleV1::InputCommitment { input: input_index })?;
        let commitment_variables = builder.allocate_bytes(commitment_digest);
        let commitment_message = frame_expressions_v1(
            NOTE_COMMITMENT_DOMAIN_V1,
            &note_commitment_fields(&input.note),
        )?;
        builder.push_hash(
            Sha256InvocationRoleV1::InputCommitment { input: input_index },
            commitment_message,
            commitment_variables,
            None,
        )?;
        input.commitment = Some(commitment_variables);

        let nullifier_variables = builder.allocate_bytes(*statement.nullifiers[index].as_bytes());
        let nullifier_message = frame_expressions_v1(
            NOTE_NULLIFIER_DOMAIN_V1,
            &[
                variables_as_expressions(&input.secret),
                variables_as_expressions(&input.note.rho),
                variables_as_expressions(&commitment_variables),
                constants_as_expressions(statement.pool_id.as_bytes()),
                constants_as_expressions(statement.program_id.as_bytes()),
            ],
        )?;
        builder.push_hash(
            Sha256InvocationRoleV1::Nullifier { input: input_index },
            nullifier_message,
            nullifier_variables,
            Some(*statement.nullifiers[index].as_bytes()),
        )?;

        let leaf_digest = builder
            .next_oracle_digest(Sha256InvocationRoleV1::AccumulatorLeaf { input: input_index })?;
        let mut current = builder.allocate_bytes(leaf_digest);
        let mut leaf_message = Vec::new();
        leaf_message.extend(constants_as_expressions(ACCUMULATOR_LEAF_DOMAIN_V1));
        leaf_message.extend(constants_as_expressions(
            &u64::try_from(namespace.len())
                .map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?
                .to_be_bytes(),
        ));
        leaf_message.extend(constants_as_expressions(&namespace));
        leaf_message.extend(variables_as_expressions(&commitment_variables));
        builder.push_hash(
            Sha256InvocationRoleV1::AccumulatorLeaf { input: input_index },
            leaf_message,
            current,
            None,
        )?;

        for level in 0..PRIVATE_NOTE_TREE_DEPTH_V1 {
            let level_u8 = u8::try_from(level).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?;
            let (left, right) = builder.push_node_select(
                input_index,
                level_u8,
                input.position_bits[level],
                current,
                input.path[level],
            )?;
            let digest = builder.next_oracle_digest(Sha256InvocationRoleV1::AccumulatorNode {
                input: input_index,
                level: level_u8,
            })?;
            let next = builder.allocate_bytes(digest);
            let mut node_message = Vec::new();
            node_message.extend(constants_as_expressions(ACCUMULATOR_NODE_DOMAIN_V1));
            node_message.push(ByteExpressionV1::Constant(level_u8));
            node_message.extend(variables_as_expressions(&left));
            node_message.extend(variables_as_expressions(&right));
            builder.push_hash(
                Sha256InvocationRoleV1::AccumulatorNode {
                    input: input_index,
                    level: level_u8,
                },
                node_message,
                next,
                (level + 1 == PRIVATE_NOTE_TREE_DEPTH_V1)
                    .then_some(*statement.state_root.as_bytes()),
            )?;
            current = next;
        }

        nonzero_components.push(input.note.value.to_vec());
        nonzero_components.push(input.note.authority.to_vec());
        nonzero_components.push(input.note.rho.to_vec());
        nonzero_components.push(input.note.blinding.to_vec());
        nonzero_components.push(input.secret.to_vec());
        for sibling in input.path {
            nonzero_components.push(sibling.to_vec());
        }
    }

    for (index, output) in output_variables.iter_mut().enumerate() {
        let output_index = u8::try_from(index).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?;
        let commitment_variables =
            builder.allocate_bytes(*statement.output_commitments[index].as_bytes());
        let commitment_message = frame_expressions_v1(
            NOTE_COMMITMENT_DOMAIN_V1,
            &note_commitment_fields(&output.note),
        )?;
        builder.push_hash(
            Sha256InvocationRoleV1::OutputCommitment {
                output: output_index,
            },
            commitment_message,
            commitment_variables,
            Some(*statement.output_commitments[index].as_bytes()),
        )?;
        output.commitment = Some(commitment_variables);
        nonzero_components.push(output.note.value.to_vec());
        nonzero_components.push(output.note.authority.to_vec());
        nonzero_components.push(output.note.rho.to_vec());
        nonzero_components.push(output.note.blinding.to_vec());
    }

    let mut comparison = 0_u8;
    if input_variables.len() == 2 {
        let left_commitment = input_variables[0]
            .commitment
            .ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
        let right_commitment = input_variables[1]
            .commitment
            .ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
        builder.push_distinct(comparison, &left_commitment, &right_commitment)?;
        comparison = comparison
            .checked_add(1)
            .ok_or(IvmPrivateNoteAirErrorV1::Resource)?;
        builder.push_distinct(
            comparison,
            &input_variables[0].position_bits,
            &input_variables[1].position_bits,
        )?;
        comparison = comparison
            .checked_add(1)
            .ok_or(IvmPrivateNoteAirErrorV1::Resource)?;
    }
    for input in &input_variables {
        let input_commitment = input.commitment.ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
        for output in &output_variables {
            let output_commitment = output
                .commitment
                .ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
            builder.push_distinct(comparison, &input_commitment, &output_commitment)?;
            comparison = comparison
                .checked_add(1)
                .ok_or(IvmPrivateNoteAirErrorV1::Resource)?;
        }
    }
    if output_variables.len() == 2 {
        let left_commitment = output_variables[0]
            .commitment
            .ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
        let right_commitment = output_variables[1]
            .commitment
            .ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
        builder.push_distinct(comparison, &left_commitment, &right_commitment)?;
    }

    for (component, variables) in nonzero_components.iter().enumerate() {
        builder.push_nonzero(
            u16::try_from(component).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
            variables,
        )?;
    }

    let input_values = input_variables
        .iter()
        .map(|input| input.note.value)
        .collect::<Vec<_>>();
    let output_values = output_variables
        .iter()
        .map(|output| output.note.value)
        .collect::<Vec<_>>();
    let input_sum = witness.map_or(Ok(0_u128), |witness| {
        witness.inputs.iter().try_fold(0_u128, |sum, input| {
            sum.checked_add(input.note.value)
                .ok_or(IvmPrivateNoteAirErrorV1::Assignment)
        })
    })?;
    let output_sum = witness.map_or(Ok(0_u128), |witness| {
        witness.outputs.iter().try_fold(0_u128, |sum, output| {
            sum.checked_add(output.note.value)
                .ok_or(IvmPrivateNoteAirErrorV1::Assignment)
        })
    })?;
    let input_sum_variables = builder.push_sum(SumSideV1::Inputs, &input_values, input_sum)?;
    let output_sum_variables = builder.push_sum(SumSideV1::Outputs, &output_values, output_sum)?;
    builder.push_conservation(input_sum_variables, output_sum_variables)?;
    builder.push_vm(
        &program_variables,
        input_sum_variables,
        output_sum_variables,
        input_sum,
        output_sum,
    )?;

    if witness.is_some() && builder.invocation_cursor != builder.invocation_oracle.len() {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    if builder.rows.len() > PRIVATE_NOTE_TRACE_SIZE_V1 {
        return Err(IvmPrivateNoteAirErrorV1::Resource);
    }
    while builder.rows.len() < PRIVATE_NOTE_TRACE_SIZE_V1 {
        builder.push_row(
            PrivateNoteFixedRowV1::Padding,
            [CopyCellV1::Inactive; PRIVATE_NOTE_COPY_WIDTH_V1],
            TraceBuilderV1::empty_row(),
        )?;
    }
    let copy_sigma = build_copy_sigma_v1(&builder.copy_cells)?;
    Ok(PrivateNoteBaseTraceV1 {
        fixed: PrivateNoteFixedTraceV1 {
            rows: builder.fixed_rows,
            copy_cells: builder.copy_cells,
            copy_sigma,
        },
        rows: builder.rows,
    })
}

/// Compile the complete prover trace after checking the native differential
/// oracle.
pub(super) fn build_private_note_base_trace_v1(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
    witness: &IvmPrivateNoteWitnessV1,
) -> Result<PrivateNoteBaseTraceV1, IvmPrivateNoteAirErrorV1> {
    build_private_note_trace_v1(statement, Some(witness))
}

/// Compile verifier-fixed topology without requiring wallet material.
pub(super) fn build_private_note_fixed_trace_v1(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
) -> Result<PrivateNoteFixedTraceV1, IvmPrivateNoteAirErrorV1> {
    Ok(build_private_note_trace_v1(statement, None)?.fixed)
}

/// Compile witness-allocation identities into the shared copy-chip policy.
pub(super) fn build_private_note_copy_schedule_v1(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
) -> Result<
    crate::privacy_engines::proof_managed_note_stark::NoteCopyScheduleV1,
    IvmPrivateNoteAirErrorV1,
> {
    use crate::privacy_engines::proof_managed_note_stark::{
        NoteCopyCellPolicyV1, NoteCopyScheduleV1,
    };

    let fixed = build_private_note_fixed_trace_v1(statement)?;
    let policies = fixed
        .copy_cells
        .iter()
        .map(|row| {
            row.map(|cell| match cell {
                CopyCellV1::Inactive => NoteCopyCellPolicyV1::Inactive,
                CopyCellV1::Constant(value) => NoteCopyCellPolicyV1::Constant(value),
                CopyCellV1::Variable(_) => NoteCopyCellPolicyV1::Variable,
            })
        })
        .collect();
    Ok(NoteCopyScheduleV1 {
        policies,
        sigma: fixed.copy_sigma,
    })
}

fn ensure_canonical_row(row: &[F]) -> Result<(), IvmPrivateNoteAirErrorV1> {
    if row.len() != PRIVATE_NOTE_BASE_WIDTH_V1
        || row.iter().any(|value| F::canonical(value.0).is_none())
    {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    Ok(())
}

fn ensure_boolean(value: F) -> Result<(), IvmPrivateNoteAirErrorV1> {
    if value.mul(value.sub(F::ONE)) != F::ZERO {
        return Err(IvmPrivateNoteAirErrorV1::Assignment);
    }
    Ok(())
}

fn pack_bits(bits: &[F]) -> Result<F, IvmPrivateNoteAirErrorV1> {
    bits.iter()
        .copied()
        .enumerate()
        .try_fold(F::ZERO, |sum, (bit, value)| {
            ensure_boolean(value)?;
            let weight = 1_u64
                .checked_shl(u32::try_from(bit).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?)
                .ok_or(IvmPrivateNoteAirErrorV1::Resource)?;
            Ok(sum.add(value.mul(F(weight))))
        })
}

fn packed_word_bits(row: &[F], group: usize) -> Result<F, IvmPrivateNoteAirErrorV1> {
    let start = SHA_BITS_OFFSET
        .checked_add(
            group
                .checked_mul(PRIVATE_NOTE_SHA_BITS_PER_GROUP_V1)
                .ok_or(IvmPrivateNoteAirErrorV1::Resource)?,
        )
        .ok_or(IvmPrivateNoteAirErrorV1::Resource)?;
    let end = start
        .checked_add(PRIVATE_NOTE_SHA_BITS_PER_GROUP_V1)
        .ok_or(IvmPrivateNoteAirErrorV1::Resource)?;
    pack_bits(
        row.get(start..end)
            .ok_or(IvmPrivateNoteAirErrorV1::Topology)?,
    )
}

fn ensure_zero_outside(
    row: &[F],
    allowed: &[(usize, usize)],
) -> Result<(), IvmPrivateNoteAirErrorV1> {
    for (index, value) in row.iter().copied().enumerate() {
        if value != F::ZERO
            && !allowed
                .iter()
                .any(|&(start, end)| start <= index && index < end)
        {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
    }
    Ok(())
}

fn copy_allowed() -> (usize, usize) {
    (COPY_OFFSET, COPY_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1)
}

fn validate_copy_cells_v1(
    fixed: &PrivateNoteFixedTraceV1,
    rows: &[Vec<F>],
) -> Result<(), IvmPrivateNoteAirErrorV1> {
    if fixed.copy_cells.len() != rows.len() || fixed.copy_sigma.len() != rows.len() {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    if build_copy_sigma_v1(&fixed.copy_cells)? != fixed.copy_sigma {
        return Err(IvmPrivateNoteAirErrorV1::Copy);
    }
    let maximum_label = rows
        .len()
        .checked_mul(PRIVATE_NOTE_COPY_WIDTH_V1)
        .ok_or(IvmPrivateNoteAirErrorV1::Resource)?;
    let mut values = BTreeMap::<ByteVariableV1, F>::new();
    for (row_index, (cells, row)) in fixed.copy_cells.iter().zip(rows).enumerate() {
        for (column, cell) in cells.iter().copied().enumerate() {
            let value = row[COPY_OFFSET + column];
            match cell {
                CopyCellV1::Inactive if value == F::ZERO => {}
                CopyCellV1::Constant(expected) if value == F(u64::from(expected)) => {}
                CopyCellV1::Variable(variable) => {
                    if value.0 > u64::from(u8::MAX) {
                        return Err(IvmPrivateNoteAirErrorV1::Copy);
                    }
                    match values.entry(variable) {
                        std::collections::btree_map::Entry::Vacant(entry) => {
                            entry.insert(value);
                        }
                        std::collections::btree_map::Entry::Occupied(entry)
                            if *entry.get() == value => {}
                        std::collections::btree_map::Entry::Occupied(_) => {
                            return Err(IvmPrivateNoteAirErrorV1::Copy);
                        }
                    }
                }
                _ => return Err(IvmPrivateNoteAirErrorV1::Copy),
            }
            let identity = row_index
                .checked_mul(PRIVATE_NOTE_COPY_WIDTH_V1)
                .and_then(|value| value.checked_add(column))
                .and_then(|value| value.checked_add(1))
                .ok_or(IvmPrivateNoteAirErrorV1::Resource)?;
            if fixed.copy_sigma[row_index][column] == 0
                || usize::try_from(fixed.copy_sigma[row_index][column])
                    .map_err(|_| IvmPrivateNoteAirErrorV1::Copy)?
                    > maximum_label
                || u32::try_from(identity).is_err()
            {
                return Err(IvmPrivateNoteAirErrorV1::Copy);
            }
        }
    }
    Ok(())
}

fn field_to_u32(value: F) -> Result<u32, IvmPrivateNoteAirErrorV1> {
    u32::try_from(value.0).map_err(|_| IvmPrivateNoteAirErrorV1::Assignment)
}

fn field_to_u8(value: F) -> Result<u8, IvmPrivateNoteAirErrorV1> {
    u8::try_from(value.0).map_err(|_| IvmPrivateNoteAirErrorV1::Assignment)
}

fn validate_word_group(
    row: &[F],
    group: usize,
    expected: F,
) -> Result<(), IvmPrivateNoteAirErrorV1> {
    if packed_word_bits(row, group)? != expected {
        return Err(IvmPrivateNoteAirErrorV1::Sha256);
    }
    Ok(())
}

fn validate_sha_round_v1(
    fixed: &PrivateNoteFixedRowV1,
    next_fixed: &PrivateNoteFixedRowV1,
    row: &[F],
    next: &[F],
) -> Result<(), IvmPrivateNoteAirErrorV1> {
    let PrivateNoteFixedRowV1::ShaRound {
        round,
        invocation,
        block,
        block_count,
    } = fixed
    else {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    };
    let round = usize::from(*round);
    if round >= 64 || *block >= *block_count || *block_count == 0 {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    ensure_zero_outside(
        row,
        &[
            copy_allowed(),
            (
                SHA_SCHEDULE_OFFSET,
                SHA_SCHEDULE_OFFSET + PRIVATE_NOTE_SHA_SCHEDULE_WORDS_V1,
            ),
            (
                SHA_INITIAL_STATE_OFFSET,
                SHA_INITIAL_STATE_OFFSET + PRIVATE_NOTE_SHA_STATE_WORDS_V1,
            ),
            (
                SHA_STATE_OFFSET,
                SHA_STATE_OFFSET + PRIVATE_NOTE_SHA_STATE_WORDS_V1,
            ),
            (
                SHA_BITS_OFFSET,
                SHA_BITS_OFFSET + PRIVATE_NOTE_SHA_BIT_COLUMNS_V1,
            ),
            (SHA_T1_OFFSET, SHA_T1_OFFSET + 1),
            (SHA_T2_OFFSET, SHA_T2_OFFSET + 1),
            (SHA_CARRY_OFFSET, SHA_CARRY_OFFSET + SHA_CARRY_WIDTH),
        ],
    )?;
    let schedule = row
        .get(SHA_SCHEDULE_OFFSET..SHA_SCHEDULE_OFFSET + PRIVATE_NOTE_SHA_SCHEDULE_WORDS_V1)
        .ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
    let initial = row
        .get(SHA_INITIAL_STATE_OFFSET..SHA_INITIAL_STATE_OFFSET + PRIVATE_NOTE_SHA_STATE_WORDS_V1)
        .ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
    let state = row
        .get(SHA_STATE_OFFSET..SHA_STATE_OFFSET + PRIVATE_NOTE_SHA_STATE_WORDS_V1)
        .ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
    for value in schedule.iter().chain(initial).chain(state) {
        field_to_u32(*value)?;
    }
    if round == 0 {
        if initial != state {
            return Err(IvmPrivateNoteAirErrorV1::Sha256);
        }
        if *block == 0
            && !state
                .iter()
                .copied()
                .eq(SHA256_INITIAL_STATE_V1.map(|value| F(u64::from(value))))
        {
            return Err(IvmPrivateNoteAirErrorV1::Sha256);
        }
    }
    let a = field_to_u32(state[0])?;
    let b = field_to_u32(state[1])?;
    let c = field_to_u32(state[2])?;
    let d = field_to_u32(state[3])?;
    let e = field_to_u32(state[4])?;
    let f = field_to_u32(state[5])?;
    let g = field_to_u32(state[6])?;
    let h = field_to_u32(state[7])?;
    for (group, value) in [a, b, c, e, f, g].into_iter().enumerate() {
        validate_word_group(row, group, F(u64::from(value)))?;
    }
    let w = field_to_u32(schedule[round])?;
    validate_word_group(row, 6, F(u64::from(w)))?;
    if round < 16 {
        if packed_word_bits(row, 7)? != F::ZERO || packed_word_bits(row, 8)? != F::ZERO {
            return Err(IvmPrivateNoteAirErrorV1::Sha256);
        }
        if row[SHA_CARRY_OFFSET + 6..SHA_CARRY_OFFSET + 8]
            .iter()
            .any(|value| *value != F::ZERO)
        {
            return Err(IvmPrivateNoteAirErrorV1::Sha256);
        }
        let message_word = u32::from_be_bytes([
            field_to_u8(row[COPY_OFFSET])?,
            field_to_u8(row[COPY_OFFSET + 1])?,
            field_to_u8(row[COPY_OFFSET + 2])?,
            field_to_u8(row[COPY_OFFSET + 3])?,
        ]);
        if message_word != w {
            return Err(IvmPrivateNoteAirErrorV1::Sha256);
        }
    } else {
        let w_minus_2 = field_to_u32(schedule[round - 2])?;
        let w_minus_15 = field_to_u32(schedule[round - 15])?;
        validate_word_group(row, 7, F(u64::from(w_minus_2)))?;
        validate_word_group(row, 8, F(u64::from(w_minus_15)))?;
        let wide = u64::from(sigma_small_1(w_minus_2))
            + u64::from(field_to_u32(schedule[round - 7])?)
            + u64::from(sigma_small_0(w_minus_15))
            + u64::from(field_to_u32(schedule[round - 16])?);
        let carry =
            field_to_u32(row[SHA_CARRY_OFFSET + 6])? + 2 * field_to_u32(row[SHA_CARRY_OFFSET + 7])?;
        for value in &row[SHA_CARRY_OFFSET + 6..SHA_CARRY_OFFSET + 8] {
            ensure_boolean(*value)?;
        }
        if wide != u64::from(w) + (u64::from(carry) << 32) {
            return Err(IvmPrivateNoteAirErrorV1::Sha256);
        }
    }
    let t1 = field_to_u32(row[SHA_T1_OFFSET])?;
    let t2 = field_to_u32(row[SHA_T2_OFFSET])?;
    validate_word_group(row, 9, row[SHA_T1_OFFSET])?;
    validate_word_group(row, 10, row[SHA_T2_OFFSET])?;
    let t1_carry = (0..3).try_fold(0_u32, |carry, bit| {
        ensure_boolean(row[SHA_CARRY_OFFSET + bit])?;
        Ok::<_, IvmPrivateNoteAirErrorV1>(
            carry + (field_to_u32(row[SHA_CARRY_OFFSET + bit])? << bit),
        )
    })?;
    let t2_carry = field_to_u32(row[SHA_CARRY_OFFSET + 3])?;
    let a_carry = field_to_u32(row[SHA_CARRY_OFFSET + 4])?;
    let e_carry = field_to_u32(row[SHA_CARRY_OFFSET + 5])?;
    for value in &row[SHA_CARRY_OFFSET + 3..SHA_CARRY_OFFSET + 6] {
        ensure_boolean(*value)?;
    }
    let t1_wide = u64::from(h)
        + u64::from(sigma_big_1(e))
        + u64::from(sha_choose(e, f, g))
        + u64::from(SHA256_ROUND_CONSTANTS_V1[round])
        + u64::from(w);
    let t2_wide = u64::from(sigma_big_0(a)) + u64::from(sha_majority(a, b, c));
    if t1_wide != u64::from(t1) + (u64::from(t1_carry) << 32)
        || t2_wide != u64::from(t2) + (u64::from(t2_carry) << 32)
    {
        return Err(IvmPrivateNoteAirErrorV1::Sha256);
    }
    let new_a_wide = u64::from(t1) + u64::from(t2);
    let new_e_wide = u64::from(d) + u64::from(t1);
    if new_a_wide != u64::from(new_a_wide as u32) + (u64::from(a_carry) << 32)
        || new_e_wide != u64::from(new_e_wide as u32) + (u64::from(e_carry) << 32)
    {
        return Err(IvmPrivateNoteAirErrorV1::Sha256);
    }
    let working_next = [new_a_wide as u32, a, b, c, new_e_wide as u32, e, f, g];
    if row[SHA_CARRY_OFFSET + 16..SHA_CARRY_OFFSET + SHA_CARRY_WIDTH]
        .iter()
        .any(|value| *value != F::ZERO)
    {
        return Err(IvmPrivateNoteAirErrorV1::Sha256);
    }
    if round < 63 {
        if next_fixed
            != &(PrivateNoteFixedRowV1::ShaRound {
                round: u8::try_from(round + 1).map_err(|_| IvmPrivateNoteAirErrorV1::Resource)?,
                invocation: *invocation,
                block: *block,
                block_count: *block_count,
            })
            || &next[SHA_SCHEDULE_OFFSET..SHA_SCHEDULE_OFFSET + PRIVATE_NOTE_SHA_SCHEDULE_WORDS_V1]
                != schedule
            || &next[SHA_INITIAL_STATE_OFFSET
                ..SHA_INITIAL_STATE_OFFSET + PRIVATE_NOTE_SHA_STATE_WORDS_V1]
                != initial
            || !next[SHA_STATE_OFFSET..SHA_STATE_OFFSET + PRIVATE_NOTE_SHA_STATE_WORDS_V1]
                .iter()
                .copied()
                .eq(working_next.map(|value| F(u64::from(value))))
            || row[SHA_CARRY_OFFSET + 8..SHA_CARRY_OFFSET + 16]
                .iter()
                .any(|value| *value != F::ZERO)
        {
            return Err(IvmPrivateNoteAirErrorV1::Sha256);
        }
    } else {
        if !matches!(
            next_fixed,
            PrivateNoteFixedRowV1::ShaEnd {
                invocation: next_invocation,
                block: next_block,
                block_count: next_block_count,
                digest_chunk: 0,
                ..
            } if next_invocation == invocation
                && next_block == block
                && next_block_count == block_count
        ) {
            return Err(IvmPrivateNoteAirErrorV1::Topology);
        }
        for index in 0..8 {
            ensure_boolean(row[SHA_CARRY_OFFSET + 8 + index])?;
            let feed_forward =
                u64::from(field_to_u32(initial[index])?) + u64::from(working_next[index]);
            let expected = u64::from(field_to_u32(next[SHA_STATE_OFFSET + index])?)
                + (row[SHA_CARRY_OFFSET + 8 + index].0 << 32);
            if feed_forward != expected {
                return Err(IvmPrivateNoteAirErrorV1::Sha256);
            }
        }
    }
    Ok(())
}

fn validate_sha_end_v1(
    fixed: &PrivateNoteFixedRowV1,
    next_fixed: &PrivateNoteFixedRowV1,
    row: &[F],
    next: &[F],
) -> Result<(), IvmPrivateNoteAirErrorV1> {
    let PrivateNoteFixedRowV1::ShaEnd {
        invocation,
        block,
        block_count,
        digest_chunk,
        public_digest,
    } = fixed
    else {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    };
    if *block_count == 0 || *block >= *block_count || usize::from(*digest_chunk) >= 4 {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    ensure_zero_outside(
        row,
        &[
            copy_allowed(),
            (
                SHA_STATE_OFFSET,
                SHA_STATE_OFFSET + PRIVATE_NOTE_SHA_STATE_WORDS_V1,
            ),
            (
                SHA_BITS_OFFSET,
                SHA_BITS_OFFSET + PRIVATE_NOTE_SHA_BIT_COLUMNS_V1,
            ),
        ],
    )?;
    let state = row
        .get(SHA_STATE_OFFSET..SHA_STATE_OFFSET + PRIVATE_NOTE_SHA_STATE_WORDS_V1)
        .ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
    for (group, value) in state.iter().copied().enumerate() {
        field_to_u32(value)?;
        validate_word_group(row, group, value)?;
    }
    for group in PRIVATE_NOTE_SHA_STATE_WORDS_V1..PRIVATE_NOTE_SHA_BIT_GROUPS_V1 {
        if packed_word_bits(row, group)? != F::ZERO {
            return Err(IvmPrivateNoteAirErrorV1::Sha256);
        }
    }

    let terminal = usize::from(*block) + 1 == usize::from(*block_count);
    if terminal {
        let first_word = usize::from(*digest_chunk) * 2;
        let expected_bytes = [
            field_to_u32(state[first_word])?.to_be_bytes(),
            field_to_u32(state[first_word + 1])?.to_be_bytes(),
        ]
        .concat();
        for (actual, expected) in row[COPY_OFFSET..COPY_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1]
            .iter()
            .copied()
            .zip(expected_bytes)
        {
            if field_to_u8(actual)? != expected {
                return Err(IvmPrivateNoteAirErrorV1::Sha256);
            }
        }
        if let Some(public_digest) = public_digest {
            let start = usize::from(*digest_chunk) * PRIVATE_NOTE_COPY_WIDTH_V1;
            if row[COPY_OFFSET..COPY_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1]
                .iter()
                .copied()
                .map(field_to_u8)
                .collect::<Result<Vec<_>, _>>()?
                .as_slice()
                != &public_digest[start..start + PRIVATE_NOTE_COPY_WIDTH_V1]
            {
                return Err(IvmPrivateNoteAirErrorV1::Assignment);
            }
        }
    } else if row[COPY_OFFSET..COPY_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1]
        .iter()
        .any(|value| *value != F::ZERO)
    {
        return Err(IvmPrivateNoteAirErrorV1::Sha256);
    }

    if *digest_chunk < 3 {
        if !matches!(
            next_fixed,
            PrivateNoteFixedRowV1::ShaEnd {
                invocation: next_invocation,
                block: next_block,
                block_count: next_block_count,
                digest_chunk: next_chunk,
                public_digest: next_public,
            } if next_invocation == invocation
                && next_block == block
                && next_block_count == block_count
                && *next_chunk == digest_chunk + 1
                && next_public == public_digest
        ) || next[SHA_STATE_OFFSET..SHA_STATE_OFFSET + PRIVATE_NOTE_SHA_STATE_WORDS_V1] != *state
        {
            return Err(IvmPrivateNoteAirErrorV1::Sha256);
        }
    } else if !terminal {
        if !matches!(
            next_fixed,
            PrivateNoteFixedRowV1::ShaRound {
                round: 0,
                invocation: next_invocation,
                block: next_block,
                block_count: next_block_count,
            } if next_invocation == invocation
                && usize::from(*next_block) == usize::from(*block) + 1
                && next_block_count == block_count
        ) || next
            [SHA_INITIAL_STATE_OFFSET..SHA_INITIAL_STATE_OFFSET + PRIVATE_NOTE_SHA_STATE_WORDS_V1]
            != *state
            || next[SHA_STATE_OFFSET..SHA_STATE_OFFSET + PRIVATE_NOTE_SHA_STATE_WORDS_V1] != *state
        {
            return Err(IvmPrivateNoteAirErrorV1::Sha256);
        }
    } else if let PrivateNoteFixedRowV1::ShaRound {
        round: 0,
        invocation: next_invocation,
        block: 0,
        ..
    } = next_fixed
    {
        if usize::from(*next_invocation) != usize::from(*invocation) + 1 {
            return Err(IvmPrivateNoteAirErrorV1::Topology);
        }
    }
    Ok(())
}

fn validate_node_select_v1(row: &[F]) -> Result<(), IvmPrivateNoteAirErrorV1> {
    ensure_zero_outside(row, &[copy_allowed()])?;
    let current = field_to_u8(row[COPY_OFFSET])?;
    let sibling = field_to_u8(row[COPY_OFFSET + 1])?;
    let left = field_to_u8(row[COPY_OFFSET + 2])?;
    let right = field_to_u8(row[COPY_OFFSET + 3])?;
    let direction = row[COPY_OFFSET + 4];
    ensure_boolean(direction)?;
    let expected_left = if direction == F::ZERO {
        current
    } else {
        sibling
    };
    let expected_right = if direction == F::ZERO {
        sibling
    } else {
        current
    };
    if left != expected_left || right != expected_right {
        return Err(IvmPrivateNoteAirErrorV1::Assignment);
    }
    Ok(())
}

fn validate_running_transition_v1(
    before: F,
    after: F,
    selected: F,
    first: bool,
    last: bool,
    next_before: Option<F>,
) -> Result<(), IvmPrivateNoteAirErrorV1> {
    ensure_boolean(before)?;
    ensure_boolean(after)?;
    ensure_boolean(selected)?;
    if (first && before != F::ZERO)
        || after != before.add(selected)
        || (last && after != F::ONE)
        || (!last && next_before != Some(after))
    {
        return Err(IvmPrivateNoteAirErrorV1::Assignment);
    }
    Ok(())
}

fn validate_nonzero_v1(
    fixed: &PrivateNoteFixedRowV1,
    next_fixed: &PrivateNoteFixedRowV1,
    row: &[F],
    next: &[F],
) -> Result<(), IvmPrivateNoteAirErrorV1> {
    let PrivateNoteFixedRowV1::NonZero {
        component,
        chunk,
        chunks,
    } = fixed
    else {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    };
    if *chunks == 0 || *chunk >= *chunks {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    ensure_zero_outside(
        row,
        &[
            copy_allowed(),
            (
                SCRATCH_NONZERO_BYTE_SELECT_OFFSET,
                SCRATCH_NONZERO_BYTE_SELECT_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1,
            ),
            (
                SCRATCH_NONZERO_BIT_SELECT_OFFSET,
                SCRATCH_NONZERO_BIT_SELECT_OFFSET + 8,
            ),
            (SCRATCH_BYTE_BITS_OFFSET, SCRATCH_BYTE_BITS_OFFSET + 8),
            (SCRATCH_RUNNING_BEFORE, SCRATCH_RUNNING_AFTER + 1),
        ],
    )?;
    let byte_selectors = &row[SCRATCH_NONZERO_BYTE_SELECT_OFFSET
        ..SCRATCH_NONZERO_BYTE_SELECT_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1];
    let bit_selectors =
        &row[SCRATCH_NONZERO_BIT_SELECT_OFFSET..SCRATCH_NONZERO_BIT_SELECT_OFFSET + 8];
    let selected_count = byte_selectors
        .iter()
        .copied()
        .try_fold(F::ZERO, |sum, selector| {
            ensure_boolean(selector)?;
            Ok::<_, IvmPrivateNoteAirErrorV1>(sum.add(selector))
        })?;
    ensure_boolean(selected_count)?;
    let bit_count = bit_selectors
        .iter()
        .copied()
        .try_fold(F::ZERO, |sum, selector| {
            ensure_boolean(selector)?;
            Ok::<_, IvmPrivateNoteAirErrorV1>(sum.add(selector))
        })?;
    if bit_count != selected_count {
        return Err(IvmPrivateNoteAirErrorV1::Assignment);
    }
    let selected_byte = byte_selectors
        .iter()
        .copied()
        .zip(&row[COPY_OFFSET..COPY_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1])
        .fold(F::ZERO, |sum, (selector, byte)| {
            sum.add(selector.mul(*byte))
        });
    let byte_bits = &row[SCRATCH_BYTE_BITS_OFFSET..SCRATCH_BYTE_BITS_OFFSET + 8];
    if pack_bits(byte_bits)? != selected_byte {
        return Err(IvmPrivateNoteAirErrorV1::Assignment);
    }
    for (selector, bit) in bit_selectors.iter().copied().zip(byte_bits) {
        if selector != F::ZERO && *bit != F::ONE {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
    }
    let last = usize::from(*chunk) + 1 == usize::from(*chunks);
    let next_before = (!last).then(|| next[SCRATCH_RUNNING_BEFORE]);
    if !last
        && !matches!(
            next_fixed,
            PrivateNoteFixedRowV1::NonZero {
                component: next_component,
                chunk: next_chunk,
                chunks: next_chunks,
            } if next_component == component
                && usize::from(*next_chunk) == usize::from(*chunk) + 1
                && next_chunks == chunks
        )
    {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    validate_running_transition_v1(
        row[SCRATCH_RUNNING_BEFORE],
        row[SCRATCH_RUNNING_AFTER],
        selected_count,
        *chunk == 0,
        last,
        next_before,
    )
}

fn validate_distinct_v1(
    fixed: &PrivateNoteFixedRowV1,
    next_fixed: &PrivateNoteFixedRowV1,
    row: &[F],
    next: &[F],
) -> Result<(), IvmPrivateNoteAirErrorV1> {
    const PAIRS_PER_ROW: usize = PRIVATE_NOTE_COPY_WIDTH_V1 / 2;

    let PrivateNoteFixedRowV1::Distinct {
        comparison,
        chunk,
        chunks,
    } = fixed
    else {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    };
    if *chunks == 0 || *chunk >= *chunks {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    ensure_zero_outside(
        row,
        &[
            copy_allowed(),
            (
                SCRATCH_NONZERO_BYTE_SELECT_OFFSET,
                SCRATCH_NONZERO_BYTE_SELECT_OFFSET + PAIRS_PER_ROW,
            ),
            (
                SCRATCH_NONZERO_BIT_SELECT_OFFSET,
                SCRATCH_NONZERO_BIT_SELECT_OFFSET + 8,
            ),
            (SCRATCH_BYTE_BITS_OFFSET, SCRATCH_BYTE_BITS_OFFSET + 8),
            (SCRATCH_RUNNING_BEFORE, SCRATCH_RUNNING_AFTER + 1),
            (
                SCRATCH_VM_DIFFERENCE_BITS_OFFSET,
                SCRATCH_VM_DIFFERENCE_BITS_OFFSET + 8,
            ),
        ],
    )?;
    let pair_selectors = &row
        [SCRATCH_NONZERO_BYTE_SELECT_OFFSET..SCRATCH_NONZERO_BYTE_SELECT_OFFSET + PAIRS_PER_ROW];
    let bit_selectors =
        &row[SCRATCH_NONZERO_BIT_SELECT_OFFSET..SCRATCH_NONZERO_BIT_SELECT_OFFSET + 8];
    let selected_count = pair_selectors
        .iter()
        .copied()
        .try_fold(F::ZERO, |sum, selector| {
            ensure_boolean(selector)?;
            Ok::<_, IvmPrivateNoteAirErrorV1>(sum.add(selector))
        })?;
    ensure_boolean(selected_count)?;
    let bit_count = bit_selectors
        .iter()
        .copied()
        .try_fold(F::ZERO, |sum, selector| {
            ensure_boolean(selector)?;
            Ok::<_, IvmPrivateNoteAirErrorV1>(sum.add(selector))
        })?;
    if bit_count != selected_count {
        return Err(IvmPrivateNoteAirErrorV1::Assignment);
    }
    let selected_left = pair_selectors
        .iter()
        .copied()
        .enumerate()
        .fold(F::ZERO, |sum, (pair, selector)| {
            sum.add(selector.mul(row[COPY_OFFSET + pair * 2]))
        });
    let selected_right = pair_selectors
        .iter()
        .copied()
        .enumerate()
        .fold(F::ZERO, |sum, (pair, selector)| {
            sum.add(selector.mul(row[COPY_OFFSET + pair * 2 + 1]))
        });
    let left_bits = &row[SCRATCH_BYTE_BITS_OFFSET..SCRATCH_BYTE_BITS_OFFSET + 8];
    let right_bits = &row[SCRATCH_VM_DIFFERENCE_BITS_OFFSET..SCRATCH_VM_DIFFERENCE_BITS_OFFSET + 8];
    if pack_bits(left_bits)? != selected_left || pack_bits(right_bits)? != selected_right {
        return Err(IvmPrivateNoteAirErrorV1::Assignment);
    }
    for ((selector, left), right) in bit_selectors.iter().copied().zip(left_bits).zip(right_bits) {
        if selector != F::ZERO && left == right {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
    }
    let last = usize::from(*chunk) + 1 == usize::from(*chunks);
    let next_before = (!last).then(|| next[SCRATCH_RUNNING_BEFORE]);
    if !last
        && !matches!(
            next_fixed,
            PrivateNoteFixedRowV1::Distinct {
                comparison: next_comparison,
                chunk: next_chunk,
                chunks: next_chunks,
            } if next_comparison == comparison
                && usize::from(*next_chunk) == usize::from(*chunk) + 1
                && next_chunks == chunks
        )
    {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    validate_running_transition_v1(
        row[SCRATCH_RUNNING_BEFORE],
        row[SCRATCH_RUNNING_AFTER],
        selected_count,
        *chunk == 0,
        last,
        next_before,
    )
}

fn signed_small(value: F) -> Result<i16, IvmPrivateNoteAirErrorV1> {
    if value == F::ZERO {
        Ok(0)
    } else if value == F::ONE {
        Ok(1)
    } else if value == F(GOLDILOCKS_MODULUS_V1 - 1) {
        Ok(-1)
    } else {
        Err(IvmPrivateNoteAirErrorV1::Assignment)
    }
}

fn validate_sum_v1(
    fixed: &PrivateNoteFixedRowV1,
    next_fixed: &PrivateNoteFixedRowV1,
    row: &[F],
    next: &[F],
) -> Result<(), IvmPrivateNoteAirErrorV1> {
    let PrivateNoteFixedRowV1::Sum { side, byte } = fixed else {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    };
    if usize::from(*byte) >= 16 {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    let last = *byte == 15;
    if !last
        && !matches!(
            next_fixed,
            PrivateNoteFixedRowV1::Sum {
                side: next_side,
                byte: next_byte,
            } if next_side == side && usize::from(*next_byte) == usize::from(*byte) + 1
        )
    {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    match side {
        SumSideV1::Inputs | SumSideV1::Outputs => {
            ensure_zero_outside(
                row,
                &[
                    copy_allowed(),
                    (SCRATCH_BYTE_BITS_OFFSET, SCRATCH_BYTE_BITS_OFFSET + 8),
                    (
                        SCRATCH_RELATION_CARRY_BEFORE,
                        SCRATCH_RELATION_CARRY_AFTER + 1,
                    ),
                    (
                        SCRATCH_RELATION_CARRY_BITS_OFFSET,
                        SCRATCH_RELATION_CARRY_BITS_OFFSET + 2,
                    ),
                ],
            )?;
            let operand_0 = u16::from(field_to_u8(row[COPY_OFFSET])?);
            let operand_1 = u16::from(field_to_u8(row[COPY_OFFSET + 1])?);
            let output = u16::from(field_to_u8(row[COPY_OFFSET + 2])?);
            let carry_before = u16::try_from(row[SCRATCH_RELATION_CARRY_BEFORE].0)
                .map_err(|_| IvmPrivateNoteAirErrorV1::Assignment)?;
            ensure_boolean(row[SCRATCH_RELATION_CARRY_BEFORE])?;
            let carry_after_bits =
                &row[SCRATCH_RELATION_CARRY_BITS_OFFSET..SCRATCH_RELATION_CARRY_BITS_OFFSET + 2];
            let carry_after_field = pack_bits(carry_after_bits)?;
            if row[SCRATCH_RELATION_CARRY_AFTER] != carry_after_field {
                return Err(IvmPrivateNoteAirErrorV1::Assignment);
            }
            let carry_after = u16::try_from(carry_after_field.0)
                .map_err(|_| IvmPrivateNoteAirErrorV1::Assignment)?;
            if pack_bits(&row[SCRATCH_BYTE_BITS_OFFSET..SCRATCH_BYTE_BITS_OFFSET + 8])?
                != F(u64::from(output))
                || operand_0 + operand_1 + carry_before != output + carry_after * 256
                || (*byte == 0 && carry_before != 0)
                || (last && carry_after != 0)
                || (!last
                    && next[SCRATCH_RELATION_CARRY_BEFORE] != row[SCRATCH_RELATION_CARRY_AFTER])
            {
                return Err(IvmPrivateNoteAirErrorV1::Assignment);
            }
        }
        SumSideV1::Conservation => {
            ensure_zero_outside(
                row,
                &[
                    copy_allowed(),
                    (
                        SCRATCH_RELATION_CARRY_BEFORE,
                        SCRATCH_RELATION_CARRY_AFTER + 1,
                    ),
                ],
            )?;
            let input = i16::from(field_to_u8(row[COPY_OFFSET])?);
            let output = i16::from(field_to_u8(row[COPY_OFFSET + 1])?);
            let public_in = i16::from(field_to_u8(row[COPY_OFFSET + 2])?);
            let public_out = i16::from(field_to_u8(row[COPY_OFFSET + 3])?);
            let carry_before = signed_small(row[SCRATCH_RELATION_CARRY_BEFORE])?;
            let carry_after = signed_small(row[SCRATCH_RELATION_CARRY_AFTER])?;
            if input + public_in + carry_before != output + public_out + 256 * carry_after
                || (*byte == 0 && carry_before != 0)
                || (last && carry_after != 0)
                || (!last
                    && next[SCRATCH_RELATION_CARRY_BEFORE] != row[SCRATCH_RELATION_CARRY_AFTER])
            {
                return Err(IvmPrivateNoteAirErrorV1::Assignment);
            }
        }
    }
    Ok(())
}

fn extract_private_program_v1(
    fixed: &PrivateNoteFixedTraceV1,
    rows: &[Vec<F>],
) -> Result<super::relation::PrivateProgramV1, IvmPrivateNoteAirErrorV1> {
    let mut encoded = [0_u8; PRIVATE_PROGRAM_BYTES_V1];
    let mut header_seen = false;
    let mut instruction_seen = [false; PRIVATE_PROGRAM_INSTRUCTION_COUNT_V1];
    for (fixed_row, row) in fixed.rows.iter().zip(rows) {
        match *fixed_row {
            PrivateNoteFixedRowV1::VmHeader => {
                if header_seen {
                    return Err(IvmPrivateNoteAirErrorV1::Topology);
                }
                for (target, value) in encoded[..8]
                    .iter_mut()
                    .zip(&row[COPY_OFFSET..COPY_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1])
                {
                    *target = field_to_u8(*value)?;
                }
                header_seen = true;
            }
            PrivateNoteFixedRowV1::VmProgram { instruction } => {
                let instruction = usize::from(instruction);
                let seen = instruction_seen
                    .get_mut(instruction)
                    .ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
                if *seen {
                    return Err(IvmPrivateNoteAirErrorV1::Topology);
                }
                let start = 8 + instruction * PRIVATE_NOTE_COPY_WIDTH_V1;
                for (target, value) in encoded[start..start + PRIVATE_NOTE_COPY_WIDTH_V1]
                    .iter_mut()
                    .zip(&row[COPY_OFFSET..COPY_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1])
                {
                    *target = field_to_u8(*value)?;
                }
                *seen = true;
            }
            _ => {}
        }
    }
    if !header_seen || instruction_seen.iter().any(|seen| !*seen) {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    decode_private_program_v1(&encoded).map_err(|_| IvmPrivateNoteAirErrorV1::Assignment)
}

fn validate_vm_common_v1(
    row: &[F],
    instruction: PrivateInstructionV1,
    halted_before: bool,
    halted_after: bool,
) -> Result<(), IvmPrivateNoteAirErrorV1> {
    let opcode_selectors =
        &row[SCRATCH_VM_OPCODE_SELECT_OFFSET..SCRATCH_VM_OPCODE_SELECT_OFFSET + 9];
    let destination_selectors =
        &row[SCRATCH_VM_DESTINATION_SELECT_OFFSET..SCRATCH_VM_DESTINATION_SELECT_OFFSET + 8];
    let left_selectors = &row[SCRATCH_VM_LEFT_SELECT_OFFSET..SCRATCH_VM_LEFT_SELECT_OFFSET + 8];
    let right_selectors = &row[SCRATCH_VM_RIGHT_SELECT_OFFSET..SCRATCH_VM_RIGHT_SELECT_OFFSET + 8];
    for (selectors, expected) in [
        (opcode_selectors, instruction.opcode as usize),
        (destination_selectors, usize::from(instruction.destination)),
        (left_selectors, usize::from(instruction.left)),
        (right_selectors, usize::from(instruction.right)),
    ] {
        if selectors
            .iter()
            .copied()
            .enumerate()
            .try_fold(F::ZERO, |sum, (index, selector)| {
                ensure_boolean(selector)?;
                if selector != F::ZERO && index != expected {
                    return Err(IvmPrivateNoteAirErrorV1::Assignment);
                }
                Ok(sum.add(selector))
            })?
            != F::ONE
        {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
    }
    for (actual, expected) in row[SCRATCH_VM_IMMEDIATE_OFFSET..SCRATCH_VM_IMMEDIATE_OFFSET + 4]
        .iter()
        .copied()
        .zip(instruction.immediate.to_be_bytes())
    {
        if field_to_u8(actual)? != expected {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
    }
    ensure_boolean(row[SCRATCH_VM_HALTED_BEFORE])?;
    ensure_boolean(row[SCRATCH_VM_HALTED_AFTER])?;
    if row[SCRATCH_VM_HALTED_BEFORE] != F(u64::from(halted_before))
        || row[SCRATCH_VM_HALTED_AFTER] != F(u64::from(halted_after))
    {
        return Err(IvmPrivateNoteAirErrorV1::Assignment);
    }
    Ok(())
}

fn vm_common_ranges() -> [(usize, usize); 6] {
    [
        (
            SCRATCH_VM_OPCODE_SELECT_OFFSET,
            SCRATCH_VM_OPCODE_SELECT_OFFSET + 9,
        ),
        (
            SCRATCH_VM_DESTINATION_SELECT_OFFSET,
            SCRATCH_VM_DESTINATION_SELECT_OFFSET + 8,
        ),
        (
            SCRATCH_VM_LEFT_SELECT_OFFSET,
            SCRATCH_VM_LEFT_SELECT_OFFSET + 8,
        ),
        (
            SCRATCH_VM_RIGHT_SELECT_OFFSET,
            SCRATCH_VM_RIGHT_SELECT_OFFSET + 8,
        ),
        (SCRATCH_VM_IMMEDIATE_OFFSET, SCRATCH_VM_IMMEDIATE_OFFSET + 4),
        (SCRATCH_VM_HALTED_BEFORE, SCRATCH_VM_HALTED_AFTER + 1),
    ]
}

fn vm_halted_flags(
    program: &super::relation::PrivateProgramV1,
    instruction: usize,
) -> Result<(bool, bool), IvmPrivateNoteAirErrorV1> {
    let current = program
        .instructions
        .get(instruction)
        .ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
    let before = program.instructions[..instruction]
        .iter()
        .any(|value| value.opcode == PrivateOpcodeV1::Halt);
    Ok((before, before || current.opcode == PrivateOpcodeV1::Halt))
}

fn validate_vm_header_v1(
    next_fixed: &PrivateNoteFixedRowV1,
    row: &[F],
) -> Result<(), IvmPrivateNoteAirErrorV1> {
    ensure_zero_outside(row, &[copy_allowed()])?;
    if !matches!(
        next_fixed,
        PrivateNoteFixedRowV1::VmProgram { instruction: 0 }
    ) {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    Ok(())
}

fn validate_vm_program_v1(
    fixed: &PrivateNoteFixedRowV1,
    next_fixed: &PrivateNoteFixedRowV1,
    row: &[F],
    program: &super::relation::PrivateProgramV1,
) -> Result<(), IvmPrivateNoteAirErrorV1> {
    let PrivateNoteFixedRowV1::VmProgram { instruction } = fixed else {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    };
    let instruction_index = usize::from(*instruction);
    let instruction_value = *program
        .instructions
        .get(instruction_index)
        .ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
    let mut allowed = vec![copy_allowed()];
    allowed.extend(vm_common_ranges());
    ensure_zero_outside(row, &allowed)?;
    let encoded = instruction_value.to_bytes();
    for (actual, expected) in row[COPY_OFFSET..COPY_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1]
        .iter()
        .copied()
        .zip(encoded)
    {
        if field_to_u8(actual)? != expected {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
    }
    let (halted_before, halted_after) = vm_halted_flags(program, instruction_index)?;
    validate_vm_common_v1(row, instruction_value, halted_before, halted_after)?;
    if !matches!(
        next_fixed,
        PrivateNoteFixedRowV1::VmPrevious {
            instruction: next_instruction,
            byte: 0,
        } if next_instruction == instruction
    ) {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    Ok(())
}

fn validate_vm_previous_v1(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
    fixed: &PrivateNoteFixedRowV1,
    next_fixed: &PrivateNoteFixedRowV1,
    row: &[F],
    next: &[F],
    following: Option<(&PrivateNoteFixedRowV1, &[F])>,
    program: &super::relation::PrivateProgramV1,
) -> Result<(), IvmPrivateNoteAirErrorV1> {
    let PrivateNoteFixedRowV1::VmPrevious { instruction, byte } = fixed else {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    };
    let instruction_index = usize::from(*instruction);
    let little_byte = usize::from(*byte);
    if little_byte >= 16 {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    let instruction_value = *program
        .instructions
        .get(instruction_index)
        .ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
    let mut allowed = vec![
        copy_allowed(),
        (SCRATCH_VM_CARRY_BEFORE, SCRATCH_VM_RESULT + 1),
        (
            SCRATCH_VM_RESULT_BITS_OFFSET,
            SCRATCH_VM_RESULT_BITS_OFFSET + 8,
        ),
        (
            SCRATCH_VM_DIFFERENCE_BITS_OFFSET,
            SCRATCH_VM_DIFFERENCE_BITS_OFFSET + 8,
        ),
    ];
    allowed.extend(vm_common_ranges());
    ensure_zero_outside(row, &allowed)?;
    let (halted_before, halted_after) = vm_halted_flags(program, instruction_index)?;
    validate_vm_common_v1(row, instruction_value, halted_before, halted_after)?;
    if !matches!(
        next_fixed,
        PrivateNoteFixedRowV1::VmNext {
            instruction: next_instruction,
            byte: next_byte,
        } if next_instruction == instruction && next_byte == byte
    ) {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    let previous = row[COPY_OFFSET..COPY_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1]
        .iter()
        .copied()
        .map(field_to_u8)
        .collect::<Result<Vec<_>, _>>()?;
    let next_registers = next[COPY_OFFSET..COPY_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1]
        .iter()
        .copied()
        .map(field_to_u8)
        .collect::<Result<Vec<_>, _>>()?;
    let destination = usize::from(instruction_value.destination);
    let left = usize::from(instruction_value.left);
    let right = usize::from(instruction_value.right);
    let byte_index = 15 - little_byte;
    let result = field_to_u8(row[SCRATCH_VM_RESULT])?;
    let difference = field_to_u8(row[SCRATCH_VM_DIFFERENCE])?;
    if pack_bits(&row[SCRATCH_VM_RESULT_BITS_OFFSET..SCRATCH_VM_RESULT_BITS_OFFSET + 8])?
        != F(u64::from(result))
        || pack_bits(
            &row[SCRATCH_VM_DIFFERENCE_BITS_OFFSET..SCRATCH_VM_DIFFERENCE_BITS_OFFSET + 8],
        )? != F(u64::from(difference))
    {
        return Err(IvmPrivateNoteAirErrorV1::Assignment);
    }

    let writes = matches!(
        instruction_value.opcode,
        PrivateOpcodeV1::MoveImmediate
            | PrivateOpcodeV1::Move
            | PrivateOpcodeV1::AddChecked
            | PrivateOpcodeV1::SubChecked
            | PrivateOpcodeV1::LoadActionLimb
            | PrivateOpcodeV1::LoadExecutionEpoch
    );
    for register in 0..8 {
        if (!writes || register != destination) && previous[register] != next_registers[register] {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
    }
    if writes && next_registers[destination] != result {
        return Err(IvmPrivateNoteAirErrorV1::Assignment);
    }

    let expected_result = match instruction_value.opcode {
        PrivateOpcodeV1::Halt
        | PrivateOpcodeV1::AssertEqual
        | PrivateOpcodeV1::AssertLessOrEqual => 0,
        PrivateOpcodeV1::MoveImmediate => {
            u128::from(instruction_value.immediate).to_be_bytes()[byte_index]
        }
        PrivateOpcodeV1::Move => previous[left],
        PrivateOpcodeV1::AddChecked | PrivateOpcodeV1::SubChecked => next_registers[destination],
        PrivateOpcodeV1::LoadActionLimb => {
            let limb = usize::try_from(instruction_value.immediate)
                .map_err(|_| IvmPrivateNoteAirErrorV1::Assignment)?;
            let index = limb
                .checked_mul(16)
                .and_then(|start| start.checked_add(byte_index))
                .ok_or(IvmPrivateNoteAirErrorV1::Assignment)?;
            *statement
                .action_digest
                .as_bytes()
                .get(index)
                .ok_or(IvmPrivateNoteAirErrorV1::Assignment)?
        }
        PrivateOpcodeV1::LoadExecutionEpoch => {
            u128::from(statement.execution_epoch).to_be_bytes()[byte_index]
        }
    };
    if result != expected_result {
        return Err(IvmPrivateNoteAirErrorV1::Assignment);
    }

    let carry_before = row[SCRATCH_VM_CARRY_BEFORE];
    let carry_after = row[SCRATCH_VM_CARRY_AFTER];
    let arithmetic = matches!(
        instruction_value.opcode,
        PrivateOpcodeV1::AddChecked
            | PrivateOpcodeV1::SubChecked
            | PrivateOpcodeV1::AssertLessOrEqual
    );
    if arithmetic {
        ensure_boolean(carry_before)?;
        ensure_boolean(carry_after)?;
        let carry_before_u16 =
            u16::try_from(carry_before.0).map_err(|_| IvmPrivateNoteAirErrorV1::Assignment)?;
        let carry_after_u16 =
            u16::try_from(carry_after.0).map_err(|_| IvmPrivateNoteAirErrorV1::Assignment)?;
        let valid_equation = match instruction_value.opcode {
            PrivateOpcodeV1::AddChecked => {
                u16::from(previous[left]) + u16::from(previous[right]) + carry_before_u16
                    == u16::from(result) + 256 * carry_after_u16
                    && difference == 0
            }
            PrivateOpcodeV1::SubChecked => {
                u16::from(result) + u16::from(previous[right]) + carry_before_u16
                    == u16::from(previous[left]) + 256 * carry_after_u16
                    && difference == 0
            }
            PrivateOpcodeV1::AssertLessOrEqual => {
                u16::from(difference) + u16::from(previous[left]) + carry_before_u16
                    == u16::from(previous[right]) + 256 * carry_after_u16
            }
            _ => false,
        };
        if !valid_equation
            || (little_byte == 0 && carry_before != F::ZERO)
            || (little_byte == 15 && carry_after != F::ZERO)
        {
            return Err(IvmPrivateNoteAirErrorV1::Assignment);
        }
        if little_byte < 15 {
            let Some((
                PrivateNoteFixedRowV1::VmPrevious {
                    instruction: following_instruction,
                    byte: following_byte,
                },
                following_row,
            )) = following
            else {
                return Err(IvmPrivateNoteAirErrorV1::Topology);
            };
            if following_instruction != instruction
                || usize::from(*following_byte) != little_byte + 1
                || following_row[SCRATCH_VM_CARRY_BEFORE] != carry_after
            {
                return Err(IvmPrivateNoteAirErrorV1::Assignment);
            }
        }
    } else if carry_before != F::ZERO || carry_after != F::ZERO || difference != 0 {
        return Err(IvmPrivateNoteAirErrorV1::Assignment);
    }
    if instruction_value.opcode == PrivateOpcodeV1::AssertEqual && previous[left] != previous[right]
    {
        return Err(IvmPrivateNoteAirErrorV1::Assignment);
    }
    Ok(())
}

fn validate_vm_next_v1(
    fixed: &PrivateNoteFixedRowV1,
    next_fixed: &PrivateNoteFixedRowV1,
    row: &[F],
    program: &super::relation::PrivateProgramV1,
) -> Result<(), IvmPrivateNoteAirErrorV1> {
    let PrivateNoteFixedRowV1::VmNext { instruction, byte } = fixed else {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    };
    let instruction_index = usize::from(*instruction);
    let little_byte = usize::from(*byte);
    if little_byte >= 16 {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    let instruction_value = *program
        .instructions
        .get(instruction_index)
        .ok_or(IvmPrivateNoteAirErrorV1::Topology)?;
    let mut allowed = vec![copy_allowed()];
    allowed.extend(vm_common_ranges());
    ensure_zero_outside(row, &allowed)?;
    for value in &row[COPY_OFFSET..COPY_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1] {
        field_to_u8(*value)?;
    }
    let (halted_before, halted_after) = vm_halted_flags(program, instruction_index)?;
    validate_vm_common_v1(row, instruction_value, halted_before, halted_after)?;
    let topology_valid = if little_byte < 15 {
        matches!(
            next_fixed,
            PrivateNoteFixedRowV1::VmPrevious {
                instruction: next_instruction,
                byte: next_byte,
            } if next_instruction == instruction
                && usize::from(*next_byte) == little_byte + 1
        )
    } else if instruction_index + 1 < PRIVATE_PROGRAM_INSTRUCTION_COUNT_V1 {
        matches!(
            next_fixed,
            PrivateNoteFixedRowV1::VmProgram {
                instruction: next_instruction,
            } if usize::from(*next_instruction) == instruction_index + 1
        )
    } else {
        matches!(next_fixed, PrivateNoteFixedRowV1::Padding)
    };
    if !topology_valid {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    Ok(())
}

/// Evaluate every native private-note AIR constraint against one complete
/// canonical base trace. This is deliberately proof-format neutral; the shared
/// aggregate SHA-256/Goldilocks engine consumes the same fixed and base
/// columns.
pub(super) fn validate_private_note_base_trace_v1(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
    trace: &PrivateNoteBaseTraceV1,
) -> Result<(), IvmPrivateNoteAirErrorV1> {
    validate_statement_v1(statement).map_err(|_| IvmPrivateNoteAirErrorV1::Relation)?;
    if trace.rows.len() != PRIVATE_NOTE_TRACE_SIZE_V1
        || trace.fixed.rows.len() != PRIVATE_NOTE_TRACE_SIZE_V1
        || trace.fixed.copy_cells.len() != PRIVATE_NOTE_TRACE_SIZE_V1
        || trace.fixed.copy_sigma.len() != PRIVATE_NOTE_TRACE_SIZE_V1
    {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    for row in &trace.rows {
        ensure_canonical_row(row)?;
    }
    let expected_fixed = build_private_note_fixed_trace_v1(statement)?;
    if trace.fixed != expected_fixed {
        return Err(IvmPrivateNoteAirErrorV1::Topology);
    }
    validate_copy_cells_v1(&trace.fixed, &trace.rows)?;
    let program = extract_private_program_v1(&trace.fixed, &trace.rows)?;

    for index in 0..PRIVATE_NOTE_TRACE_SIZE_V1 {
        let fixed = &trace.fixed.rows[index];
        let row = &trace.rows[index];
        let next_index = (index + 1).min(PRIVATE_NOTE_TRACE_SIZE_V1 - 1);
        let next_fixed = &trace.fixed.rows[next_index];
        let next = &trace.rows[next_index];
        match fixed {
            PrivateNoteFixedRowV1::ShaRound { .. } => {
                validate_sha_round_v1(fixed, next_fixed, row, next)?;
            }
            PrivateNoteFixedRowV1::ShaEnd { .. } => {
                validate_sha_end_v1(fixed, next_fixed, row, next)?;
            }
            PrivateNoteFixedRowV1::NodeSelect { .. } => {
                validate_node_select_v1(row)?;
            }
            PrivateNoteFixedRowV1::Distinct { .. } => {
                validate_distinct_v1(fixed, next_fixed, row, next)?;
            }
            PrivateNoteFixedRowV1::NonZero { .. } => {
                validate_nonzero_v1(fixed, next_fixed, row, next)?;
            }
            PrivateNoteFixedRowV1::Sum { .. } => {
                validate_sum_v1(fixed, next_fixed, row, next)?;
            }
            PrivateNoteFixedRowV1::VmHeader => {
                validate_vm_header_v1(next_fixed, row)?;
            }
            PrivateNoteFixedRowV1::VmProgram { .. } => {
                validate_vm_program_v1(fixed, next_fixed, row, &program)?;
            }
            PrivateNoteFixedRowV1::VmPrevious { byte, .. } => {
                let following = (usize::from(*byte) < 15)
                    .then(|| {
                        trace
                            .fixed
                            .rows
                            .get(index + 2)
                            .zip(trace.rows.get(index + 2))
                            .map(|(fixed, row)| (fixed, row.as_slice()))
                    })
                    .flatten();
                validate_vm_previous_v1(
                    statement, fixed, next_fixed, row, next, following, &program,
                )?;
            }
            PrivateNoteFixedRowV1::VmNext { .. } => {
                validate_vm_next_v1(fixed, next_fixed, row, &program)?;
            }
            PrivateNoteFixedRowV1::Padding => {
                ensure_zero_outside(row, &[])?;
                if !matches!(next_fixed, PrivateNoteFixedRowV1::Padding) {
                    return Err(IvmPrivateNoteAirErrorV1::Topology);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use iroha_data_model::privacy::{PrivacyActionDigestV1, PrivacyRootV1};
    use rand_08::{SeedableRng as _, rngs::StdRng};

    use super::*;
    use crate::privacy_engines::ivm_private_note::{
        derive_note_authority_v1, derive_note_commitment_v1, derive_note_nullifier_v1,
        encrypt_ivm_private_wallet_note_v1, ivm_private_recipient_public_key_v1,
        relation::{
            IvmPrivateNoteInputWitnessV1, IvmPrivateNoteOutputWitnessV1, PrivateNotePlaintextV1,
            accumulator_leaf_invocation_v1, accumulator_node_invocation_v1,
        },
        tests::fixture,
    };

    fn changed(value: F) -> F {
        if value == F::ZERO { F::ONE } else { F::ZERO }
    }

    fn row_index(
        trace: &PrivateNoteBaseTraceV1,
        predicate: impl Fn(&PrivateNoteFixedRowV1) -> bool,
    ) -> usize {
        trace
            .fixed
            .rows
            .iter()
            .position(predicate)
            .expect("fixture contains requested AIR row")
    }

    fn reject_cell_mutation(
        statement: &IrohaIvmPrivateNoteStarkStatementV1,
        trace: &mut PrivateNoteBaseTraceV1,
        row: usize,
        column: usize,
    ) {
        let original = trace.rows[row][column];
        trace.rows[row][column] = changed(original);
        assert!(
            validate_private_note_base_trace_v1(statement, trace).is_err(),
            "mutation at row {row}, column {column} must fail"
        );
        trace.rows[row][column] = original;
    }

    #[test]
    fn canonical_trace_is_exact_and_keeps_intermediate_hashes_private() {
        let value = fixture();
        let trace = build_private_note_base_trace_v1(&value.statement, &value.witness)
            .expect("canonical trace");
        assert_eq!(trace.rows.len(), PRIVATE_NOTE_TRACE_SIZE_V1);
        assert_eq!(trace.fixed.rows.len(), PRIVATE_NOTE_TRACE_SIZE_V1);
        assert!(
            trace
                .rows
                .iter()
                .all(|row| row.len() == PRIVATE_NOTE_BASE_WIDTH_V1)
        );
        assert_eq!(
            trace.fixed,
            build_private_note_fixed_trace_v1(&value.statement).expect("fixed topology")
        );
        validate_private_note_base_trace_v1(&value.statement, &trace)
            .expect("native AIR evaluator");

        let public_endpoints = trace
            .fixed
            .rows
            .iter()
            .filter_map(|row| match row {
                PrivateNoteFixedRowV1::ShaEnd {
                    public_digest: Some(digest),
                    ..
                } => Some(*digest),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(public_endpoints.len(), 4 * 4);
        let allowed = BTreeSet::from([
            *value.statement.program_id.as_bytes(),
            *value.statement.nullifiers[0].as_bytes(),
            *value.statement.state_root.as_bytes(),
            *value.statement.output_commitments[0].as_bytes(),
        ]);
        assert!(
            public_endpoints
                .iter()
                .all(|digest| allowed.contains(digest))
        );
        assert_eq!(
            public_endpoints.into_iter().collect::<BTreeSet<_>>(),
            allowed
        );
        assert!(trace.fixed.rows.iter().any(|row| {
            matches!(
                row,
                PrivateNoteFixedRowV1::ShaEnd {
                    public_digest: None,
                    ..
                }
            )
        }));
    }

    #[test]
    fn mutations_across_every_constraint_family_fail_closed() {
        let value = fixture();
        let mut trace = build_private_note_base_trace_v1(&value.statement, &value.witness)
            .expect("canonical trace");
        validate_private_note_base_trace_v1(&value.statement, &trace)
            .expect("canonical native AIR");

        let sha_round = row_index(&trace, |row| {
            matches!(row, PrivateNoteFixedRowV1::ShaRound { round: 16, .. })
        });
        reject_cell_mutation(
            &value.statement,
            &mut trace,
            sha_round,
            SHA_SCHEDULE_OFFSET + 16,
        );
        reject_cell_mutation(&value.statement, &mut trace, sha_round, SHA_BITS_OFFSET);
        reject_cell_mutation(
            &value.statement,
            &mut trace,
            sha_round,
            SHA_CARRY_OFFSET + 6,
        );

        let sha_end = row_index(&trace, |row| {
            matches!(
                row,
                PrivateNoteFixedRowV1::ShaEnd {
                    digest_chunk: 0,
                    ..
                }
            )
        });
        reject_cell_mutation(&value.statement, &mut trace, sha_end, SHA_STATE_OFFSET);

        let node = row_index(&trace, |row| {
            matches!(row, PrivateNoteFixedRowV1::NodeSelect { .. })
        });
        reject_cell_mutation(&value.statement, &mut trace, node, COPY_OFFSET + 2);

        let distinct = row_index(&trace, |row| {
            matches!(row, PrivateNoteFixedRowV1::Distinct { .. })
        });
        reject_cell_mutation(
            &value.statement,
            &mut trace,
            distinct,
            SCRATCH_RUNNING_AFTER,
        );

        let nonzero = row_index(&trace, |row| {
            matches!(row, PrivateNoteFixedRowV1::NonZero { .. })
        });
        reject_cell_mutation(
            &value.statement,
            &mut trace,
            nonzero,
            SCRATCH_RUNNING_BEFORE,
        );

        let sum = row_index(&trace, |row| {
            matches!(
                row,
                PrivateNoteFixedRowV1::Sum {
                    side: SumSideV1::Inputs,
                    ..
                }
            )
        });
        reject_cell_mutation(
            &value.statement,
            &mut trace,
            sum,
            SCRATCH_RELATION_CARRY_AFTER,
        );

        let vm_program = row_index(&trace, |row| {
            matches!(row, PrivateNoteFixedRowV1::VmProgram { instruction: 0 })
        });
        reject_cell_mutation(
            &value.statement,
            &mut trace,
            vm_program,
            SCRATCH_VM_OPCODE_SELECT_OFFSET,
        );

        let vm_previous = row_index(&trace, |row| {
            matches!(
                row,
                PrivateNoteFixedRowV1::VmPrevious {
                    instruction: 0,
                    byte: 0,
                }
            )
        });
        reject_cell_mutation(
            &value.statement,
            &mut trace,
            vm_previous,
            SCRATCH_VM_RESULT_BITS_OFFSET,
        );

        let padding = row_index(&trace, |row| matches!(row, PrivateNoteFixedRowV1::Padding));
        reject_cell_mutation(&value.statement, &mut trace, padding, SCRATCH_OFFSET);
    }

    #[test]
    fn malformed_shape_fixed_topology_sigma_and_noncanonical_fields_fail() {
        let value = fixture();
        let mut trace = build_private_note_base_trace_v1(&value.statement, &value.witness)
            .expect("canonical trace");

        let removed = trace.rows.pop().expect("fixed trace is nonempty");
        assert!(validate_private_note_base_trace_v1(&value.statement, &trace).is_err());
        trace.rows.push(removed);

        let removed = trace.rows[0].pop().expect("fixed row is nonempty");
        assert!(validate_private_note_base_trace_v1(&value.statement, &trace).is_err());
        trace.rows[0].push(removed);

        let original = trace.rows[0][0];
        trace.rows[0][0] = F(GOLDILOCKS_MODULUS_V1);
        assert!(validate_private_note_base_trace_v1(&value.statement, &trace).is_err());
        trace.rows[0][0] = original;

        let original = trace.fixed.copy_sigma[0][0];
        trace.fixed.copy_sigma[0][0] = 0;
        assert!(validate_private_note_base_trace_v1(&value.statement, &trace).is_err());
        trace.fixed.copy_sigma[0][0] = original;

        let original = trace.fixed.rows[0].clone();
        trace.fixed.rows[0] = PrivateNoteFixedRowV1::Padding;
        assert!(validate_private_note_base_trace_v1(&value.statement, &trace).is_err());
        trace.fixed.rows[0] = original;

        validate_private_note_base_trace_v1(&value.statement, &trace)
            .expect("restored trace remains canonical");
    }

    fn maximum_fixture() -> (IrohaIvmPrivateNoteStarkStatementV1, IvmPrivateNoteWitnessV1) {
        let mut value = fixture();
        let first = value.witness.inputs[0].clone();
        let second_secret = [0x81; 32];
        let second_note = PrivateNotePlaintextV1 {
            value: 10,
            spending_authority: derive_note_authority_v1(&second_secret).expect("second authority"),
            rho: [0x82; 32],
            blinding: [0x83; 32],
            memo_digest: [0x84; 32],
        };
        let second_commitment =
            derive_note_commitment_v1(&second_note).expect("second input commitment");
        let first_commitment =
            derive_note_commitment_v1(&first.note).expect("first input commitment");

        let second_output_secret = [0x91; 32];
        let second_output_note = PrivateNotePlaintextV1 {
            value: 10,
            spending_authority: derive_note_authority_v1(&second_output_secret)
                .expect("second output authority"),
            rho: [0x92; 32],
            blinding: [0x93; 32],
            memo_digest: [0x94; 32],
        };
        let second_output_commitment =
            derive_note_commitment_v1(&second_output_note).expect("second output commitment");
        let recipient_public_key =
            ivm_private_recipient_public_key_v1(&[0x95; 32]).expect("second recipient public key");
        let second_encrypted_output = encrypt_ivm_private_wallet_note_v1(
            &mut StdRng::seed_from_u64(0x49_50_4e_45_02),
            value.statement.pool_id,
            value.statement.program_id,
            &second_output_note,
            recipient_public_key,
        )
        .expect("second canonical encrypted output");
        value
            .statement
            .output_commitments
            .push(second_output_commitment);
        value
            .statement
            .encrypted_outputs
            .push(second_encrypted_output);

        let leaf_0 = accumulator_leaf_invocation_v1(&value.statement, 0, first_commitment)
            .expect("first leaf")
            .digest;
        let leaf_1 = accumulator_leaf_invocation_v1(&value.statement, 1, second_commitment)
            .expect("second leaf")
            .digest;
        let mut path_0 = [[0_u8; 32]; PRIVATE_NOTE_TREE_DEPTH_V1];
        let mut path_1 = [[0_u8; 32]; PRIVATE_NOTE_TREE_DEPTH_V1];
        path_0[0] = leaf_1;
        path_1[0] = leaf_0;
        for level in 1..PRIVATE_NOTE_TREE_DEPTH_V1 {
            let seed = u8::try_from(level)
                .expect("tree depth fits u8")
                .wrapping_add(0xa0);
            path_0[level] = [seed; 32];
            path_1[level] = [seed; 32];
        }
        let mut root = accumulator_node_invocation_v1(0, 0, &leaf_0, &leaf_1)
            .expect("sibling leaves")
            .digest;
        for (level, sibling) in path_0.iter().enumerate().skip(1) {
            root = accumulator_node_invocation_v1(
                0,
                u8::try_from(level).expect("tree depth fits u8"),
                &root,
                sibling,
            )
            .expect("shared upper path")
            .digest;
        }
        value.statement.state_root = PrivacyRootV1::new(root);
        value.witness.inputs[0].leaf_position = 0;
        value.witness.inputs[0].authentication_path = path_0;
        let second_rho = second_note.rho;
        value.witness.inputs.push(IvmPrivateNoteInputWitnessV1 {
            note: second_note,
            spending_secret: second_secret,
            leaf_position: 1,
            authentication_path: path_1,
        });
        value.witness.outputs.push(IvmPrivateNoteOutputWitnessV1 {
            note: second_output_note,
        });
        value.statement.nullifiers = vec![
            derive_note_nullifier_v1(
                &value.statement,
                &first.spending_secret,
                &first.note.rho,
                first_commitment,
            )
            .expect("first nullifier"),
            derive_note_nullifier_v1(
                &value.statement,
                &second_secret,
                &second_rho,
                second_commitment,
            )
            .expect("second nullifier"),
        ];
        value.statement.action_digest = PrivacyActionDigestV1::new([0; 32]);
        value.statement.action_digest = value
            .statement
            .computed_action_digest()
            .expect("maximum action digest");
        (value.statement, value.witness)
    }

    #[test]
    fn maximum_two_by_two_relation_fits_the_exact_trace_bound() {
        let (statement, witness) = maximum_fixture();
        let trace =
            build_private_note_base_trace_v1(&statement, &witness).expect("maximum trace fits");
        let first_padding = row_index(&trace, |row| matches!(row, PrivateNoteFixedRowV1::Padding));
        assert!(first_padding < PRIVATE_NOTE_TRACE_SIZE_V1);
        assert!(first_padding > PRIVATE_NOTE_TRACE_SIZE_V1 / 2);
        assert!(
            trace.fixed.rows[first_padding..]
                .iter()
                .all(|row| matches!(row, PrivateNoteFixedRowV1::Padding))
        );
        validate_private_note_base_trace_v1(&statement, &trace).expect("maximum native AIR trace");
    }
}
