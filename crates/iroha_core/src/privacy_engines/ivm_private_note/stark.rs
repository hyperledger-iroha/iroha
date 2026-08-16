//! Extension-domain adapter for the fixed native private-note AIR.
//!
//! The verifier reconstructs every selector and constant column from the
//! public statement. The prover only commits witness-bearing base columns and
//! one carry bridge needed by the alternating VM previous/next row layout.
use super::{
    air::{
        COPY_OFFSET, IvmPrivateNoteAirErrorV1, PRIVATE_NOTE_BASE_WIDTH_V1,
        PRIVATE_NOTE_COPY_WIDTH_V1, PRIVATE_NOTE_SHA_BIT_COLUMNS_V1,
        PRIVATE_NOTE_SHA_BITS_PER_GROUP_V1, PRIVATE_NOTE_SHA_SCHEDULE_WORDS_V1,
        PRIVATE_NOTE_SHA_STATE_WORDS_V1, PRIVATE_NOTE_TRACE_LOG2_V1, PRIVATE_NOTE_TRACE_SIZE_V1,
        PrivateNoteBaseTraceV1, PrivateNoteFixedRowV1, SCRATCH_BYTE_BITS_OFFSET,
        SCRATCH_NONZERO_BIT_SELECT_OFFSET, SCRATCH_NONZERO_BYTE_SELECT_OFFSET,
        SCRATCH_RELATION_CARRY_AFTER, SCRATCH_RELATION_CARRY_BEFORE,
        SCRATCH_RELATION_CARRY_BITS_OFFSET, SCRATCH_RUNNING_AFTER, SCRATCH_RUNNING_BEFORE,
        SCRATCH_VM_CARRY_AFTER, SCRATCH_VM_CARRY_BEFORE, SCRATCH_VM_DESTINATION_SELECT_OFFSET,
        SCRATCH_VM_DIFFERENCE, SCRATCH_VM_DIFFERENCE_BITS_OFFSET, SCRATCH_VM_HALTED_AFTER,
        SCRATCH_VM_HALTED_BEFORE, SCRATCH_VM_IMMEDIATE_OFFSET, SCRATCH_VM_LEFT_SELECT_OFFSET,
        SCRATCH_VM_OPCODE_SELECT_OFFSET, SCRATCH_VM_RESULT, SCRATCH_VM_RESULT_BITS_OFFSET,
        SCRATCH_VM_RIGHT_SELECT_OFFSET, SHA_BITS_OFFSET, SHA_CARRY_OFFSET, SHA_CARRY_WIDTH,
        SHA_INITIAL_STATE_OFFSET, SHA_SCHEDULE_OFFSET, SHA_STATE_OFFSET, SHA_T1_OFFSET,
        SHA_T2_OFFSET, SHA256_INITIAL_STATE_V1, SHA256_ROUND_CONSTANTS_V1, SumSideV1,
        build_private_note_copy_schedule_v1, build_private_note_fixed_trace_v1,
        validate_private_note_base_trace_v1,
    },
    relation::IvmPrivateNoteWitnessV1,
};
#[cfg(test)]
use crate::privacy_engines::proof_managed_note_stark::proof_managed_note_stark_profile_digest_v1;
use crate::privacy_engines::{
    aggregate_stark as aggregate,
    proof_managed_note_stark::{
        NOTE_COPY_AUX_WIDTH_V1, NOTE_COPY_FIXED_WIDTH_V1, NoteCopyChallengesV1, NoteCopyScheduleV1,
        PROOF_MANAGED_NOTE_BLOWUP_LOG2_V1, PROOF_MANAGED_NOTE_COMPOSITION_DEGREE_CHUNKS_V1,
        PROOF_MANAGED_NOTE_MAX_CONSTRAINT_DEGREE_V1, PROOF_MANAGED_NOTE_QUERY_COUNT_V1,
        PROOF_MANAGED_NOTE_SECURITY_LANES_V1, PROOF_MANAGED_NOTE_TERMINAL_DEGREE_BOUND_V1,
        PROOF_MANAGED_NOTE_TERMINAL_LOG2_V1, ProofManagedNoteStarkAdapterV1,
        ProofManagedNoteStarkErrorV1, ProofManagedNoteStarkProtocolV1,
        prove_proof_managed_note_stark_v1_with_rng, verify_proof_managed_note_stark_v1,
    },
    transparent_stark::{GoldilocksFieldV1 as F, TransparentTranscriptV1, sha256_frame_v1},
};
use iroha_data_model::privacy::{
    IrohaIvmPrivateNoteStarkStatementV1, PrivacyConsensusLimitsV1,
    PrivacyNativeConsensusBindingDigestV1, PrivacyNativeConsensusBindingV1,
};
use rand::TryRngCore;
const TYPE_SHA_ROUND: usize = 0;
const TYPE_SHA_END: usize = 1;
const TYPE_NODE_SELECT: usize = 2;
const TYPE_DISTINCT: usize = 3;
const TYPE_NONZERO: usize = 4;
const TYPE_SUM_IO: usize = 5;
const TYPE_SUM_CONSERVATION: usize = 6;
const TYPE_VM_HEADER: usize = 7;
const TYPE_VM_PROGRAM: usize = 8;
const TYPE_VM_PREVIOUS: usize = 9;
const TYPE_VM_NEXT: usize = 10;
const TYPE_PADDING: usize = 11;
const TYPE_COLUMN_COUNT: usize = 12;
const FIXED_ROUND_SELECTOR_OFFSET: usize = TYPE_COLUMN_COUNT;
const FIXED_FIRST_BLOCK_ROUND_ZERO: usize = FIXED_ROUND_SELECTOR_OFFSET + 64;
const FIXED_SHA_END_TERMINAL: usize = FIXED_FIRST_BLOCK_ROUND_ZERO + 1;
const FIXED_SHA_END_CONTINUE: usize = FIXED_SHA_END_TERMINAL + 1;
const FIXED_SHA_END_NEXT_BLOCK: usize = FIXED_SHA_END_CONTINUE + 1;
const FIXED_TERMINAL_CHUNK_OFFSET: usize = FIXED_SHA_END_NEXT_BLOCK + 1;
const FIXED_SHA_END_PUBLIC_SELECTOR: usize = FIXED_TERMINAL_CHUNK_OFFSET + 4;
const FIXED_SHA_END_PUBLIC_BYTE_OFFSET: usize = FIXED_SHA_END_PUBLIC_SELECTOR + 1;
const FIXED_SEQUENCE_FIRST: usize = FIXED_SHA_END_PUBLIC_BYTE_OFFSET + PRIVATE_NOTE_COPY_WIDTH_V1;
const FIXED_SEQUENCE_LAST: usize = FIXED_SEQUENCE_FIRST + 1;
const FIXED_SEQUENCE_TRANSITION: usize = FIXED_SEQUENCE_LAST + 1;
const FIXED_SUM_FIRST: usize = FIXED_SEQUENCE_TRANSITION + 1;
const FIXED_SUM_LAST: usize = FIXED_SUM_FIRST + 1;
const FIXED_SUM_TRANSITION: usize = FIXED_SUM_LAST + 1;
const FIXED_VM_BYTE_SELECTOR_OFFSET: usize = FIXED_SUM_TRANSITION + 1;
const FIXED_VM_PROGRAM_FIRST: usize = FIXED_VM_BYTE_SELECTOR_OFFSET + 16;
const FIXED_VM_PROGRAM_LAST: usize = FIXED_VM_PROGRAM_FIRST + 1;
const FIXED_VM_COMMON_TRANSITION: usize = FIXED_VM_PROGRAM_LAST + 1;
const FIXED_VM_INSTRUCTION_TRANSITION: usize = FIXED_VM_COMMON_TRANSITION + 1;
const FIXED_VM_ACTION_LIMB_ZERO_BYTE: usize = FIXED_VM_INSTRUCTION_TRANSITION + 1;
const FIXED_VM_ACTION_LIMB_ONE_BYTE: usize = FIXED_VM_ACTION_LIMB_ZERO_BYTE + 1;
const FIXED_VM_EXECUTION_EPOCH_BYTE: usize = FIXED_VM_ACTION_LIMB_ONE_BYTE + 1;
pub(super) const PRIVATE_NOTE_PROFILE_FIXED_WIDTH_V1: usize = FIXED_VM_EXECUTION_EPOCH_BYTE + 1;
pub(super) const PRIVATE_NOTE_PROFILE_AUX_WIDTH_V1: usize = 1;
pub(super) const PRIVATE_NOTE_PROFILE_CONSTRAINT_COUNT_V1: usize = 1_372;
/// Audited maximum algebraic degree across the complete shared/profile AIR.
pub(super) const PRIVATE_NOTE_PROFILE_CONSTRAINT_DEGREE_V1: u8 =
    PROOF_MANAGED_NOTE_MAX_CONSTRAINT_DEGREE_V1;
const PROFILE_AUX_VM_CARRY_BRIDGE: usize = 0;
/// Relation-local descriptor combined with the shared proof-driver geometry.
pub(crate) const IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1: &[u8] = b"iroha-ivm-private-note-stark-v1:relation=proof-managed-note:wire=IPS1-v1:trace=2^14:base=556:profile-aux=1:profile-fixed=122:profile-constraints=1372:constraint-degree=4:max-proof=8388608:sha256-wide-air:public-digest=sha256-frame(canonical-statement,PrivacyNativeConsensusBindingDigestV1):tree-depth=32:vm=16x8:ciphertext=IPNE-v1:fee=separate:legacy=unrepresentable:governance=typed-lifecycle";
// Framed SHA-256 of the shared proof geometry followed by the relation
// descriptor above. Any compiled profile change must intentionally update the
// relevant descriptor and this digest.
pub(crate) const IVM_PRIVATE_NOTE_STARK_PROFILE_DIGEST_V1: [u8; 32] = [
    0x7e, 0xca, 0x75, 0x19, 0x40, 0xd1, 0x09, 0xb9, 0xfd, 0xe6, 0xfb, 0xdf, 0x44, 0xd0, 0xb5, 0xaa,
    0x49, 0xbf, 0x78, 0x36, 0xa8, 0x53, 0x31, 0x49, 0xb3, 0x12, 0x17, 0x14, 0xd2, 0xd7, 0x9b, 0xd0,
];
/// SHA-256 of the deterministic full-domain proof generated by the canonical
/// test fixture with `StdRng::from_seed([0xA9; 32])`.
pub(crate) const IVM_PRIVATE_NOTE_STARK_KAT_PROOF_SHA256_V1: [u8; 32] = [
    0xe6, 0x15, 0x2d, 0x40, 0xcc, 0x36, 0x17, 0xe1, 0xd2, 0x4d, 0x27, 0x41, 0x1e, 0x64, 0x55, 0x24,
    0x6c, 0x87, 0xc1, 0x54, 0xef, 0x07, 0x7f, 0xdf, 0xcc, 0x93, 0x01, 0xfd, 0x05, 0x66, 0xab, 0x74,
];
/// Exact first-release proof ceiling enforced by the private-note verifier.
pub const IVM_PRIVATE_NOTE_MAX_PROOF_BYTES_V1: usize = 8 * 1024 * 1024;
const PRIVATE_NOTE_PARAMETERS_V1: aggregate::AggregateStarkParametersV1 =
    aggregate::AggregateStarkParametersV1 {
        proof_magic: *b"IPS1",
        proof_version: 1,
        security_lanes: PROOF_MANAGED_NOTE_SECURITY_LANES_V1,
        query_count: PROOF_MANAGED_NOTE_QUERY_COUNT_V1,
        blowup_log2: PROOF_MANAGED_NOTE_BLOWUP_LOG2_V1,
        terminal_log2: PROOF_MANAGED_NOTE_TERMINAL_LOG2_V1,
        terminal_degree_bound: PROOF_MANAGED_NOTE_TERMINAL_DEGREE_BOUND_V1,
        composition_degree_chunks: PROOF_MANAGED_NOTE_COMPOSITION_DEGREE_CHUNKS_V1,
        minimum_trace_log2: PRIVATE_NOTE_TRACE_LOG2_V1,
        maximum_trace_log2: PRIVATE_NOTE_TRACE_LOG2_V1,
        maximum_trace_groups: 1,
        maximum_segment_instances: 1,
        maximum_base_columns_per_instance: PRIVATE_NOTE_BASE_WIDTH_V1,
        maximum_aux_columns_per_instance: NOTE_COPY_AUX_WIDTH_V1
            + PRIVATE_NOTE_PROFILE_AUX_WIDTH_V1,
        maximum_proof_bytes: IVM_PRIVATE_NOTE_MAX_PROOF_BYTES_V1,
    };
const PRIVATE_NOTE_DOMAINS_V1: aggregate::AggregateStarkDomainsV1 =
    aggregate::AggregateStarkDomainsV1 {
        base_leaf: b"ivm-private-note-stark-base-leaf-v1",
        base_node: b"ivm-private-note-stark-base-node-v1",
        aux_leaf: b"ivm-private-note-stark-aux-leaf-v1",
        aux_node: b"ivm-private-note-stark-aux-node-v1",
        composition_leaf: b"ivm-private-note-stark-composition-leaf-v1",
        composition_node: b"ivm-private-note-stark-composition-node-v1",
        fri_leaf: b"ivm-private-note-stark-fri-leaf-v1",
        fri_node: b"ivm-private-note-stark-fri-node-v1",
        layout_label: b"ivm-private-note-stark-layout-v1",
        base_root_label: b"ivm-private-note-stark-base-root-v1",
        aux_root_label: b"ivm-private-note-stark-aux-root-v1",
        composition_root_label: b"ivm-private-note-stark-composition-root-v1",
        fri_root_label: b"ivm-private-note-stark-fri-root-v1",
        fri_beta_label: b"ivm-private-note-stark-fri-beta-v1",
        query_seed: b"ivm-private-note-stark-query-seed-v1",
    };
fn private_note_protocol_v1() -> ProofManagedNoteStarkProtocolV1 {
    ProofManagedNoteStarkProtocolV1 {
        parameters: PRIVATE_NOTE_PARAMETERS_V1,
        domains: PRIVATE_NOTE_DOMAINS_V1,
        maximum_constraint_degree: PRIVATE_NOTE_PROFILE_CONSTRAINT_DEGREE_V1,
        profile_digest: IVM_PRIVATE_NOTE_STARK_PROFILE_DIGEST_V1,
        profile_binding_label: b"ivm-private-note-stark-profile-v1",
        profile_descriptor: IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1,
        relation_layout_domain: b"ivm-private-note-stark-relation-layout-v1",
    }
}
/// Validate the complete compiled proof-system and relation profile.
pub(crate) fn validate_ivm_private_note_stark_profile_v1()
-> Result<(), ProofManagedNoteStarkErrorV1> {
    private_note_protocol_v1().validate()
}
fn map_air_error_v1(error: IvmPrivateNoteAirErrorV1) -> ProofManagedNoteStarkErrorV1 {
    match error {
        IvmPrivateNoteAirErrorV1::Resource => ProofManagedNoteStarkErrorV1::Resource,
        IvmPrivateNoteAirErrorV1::Copy => ProofManagedNoteStarkErrorV1::Copy,
        IvmPrivateNoteAirErrorV1::Relation
        | IvmPrivateNoteAirErrorV1::Topology
        | IvmPrivateNoteAirErrorV1::Assignment
        | IvmPrivateNoteAirErrorV1::Sha256 => ProofManagedNoteStarkErrorV1::Constraint,
    }
}
fn f(value: impl Into<u64>) -> F {
    F(value.into())
}
fn set(columns: &mut [Vec<F>], column: usize, row: usize, value: F) {
    columns[column][row] = value;
}
fn vm_same_instruction_transition(
    current: &PrivateNoteFixedRowV1,
    next: &PrivateNoteFixedRowV1,
) -> bool {
    match (current, next) {
        (
            PrivateNoteFixedRowV1::VmProgram { instruction },
            PrivateNoteFixedRowV1::VmPrevious {
                instruction: next_instruction,
                byte: 0,
            },
        ) => instruction == next_instruction,
        (
            PrivateNoteFixedRowV1::VmPrevious { instruction, byte },
            PrivateNoteFixedRowV1::VmNext {
                instruction: next_instruction,
                byte: next_byte,
            },
        ) => instruction == next_instruction && byte == next_byte,
        (
            PrivateNoteFixedRowV1::VmNext { instruction, byte },
            PrivateNoteFixedRowV1::VmPrevious {
                instruction: next_instruction,
                byte: next_byte,
            },
        ) => instruction == next_instruction && usize::from(*byte) + 1 == usize::from(*next_byte),
        _ => false,
    }
}
pub(super) fn private_note_profile_fixed_columns_v1(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
) -> Result<Vec<Vec<F>>, ProofManagedNoteStarkErrorV1> {
    let fixed = build_private_note_fixed_trace_v1(statement).map_err(map_air_error_v1)?;
    if fixed.rows.len() != PRIVATE_NOTE_TRACE_SIZE_V1 {
        return Err(ProofManagedNoteStarkErrorV1::InvalidProfile);
    }
    let mut columns =
        vec![vec![F::ZERO; PRIVATE_NOTE_TRACE_SIZE_V1]; PRIVATE_NOTE_PROFILE_FIXED_WIDTH_V1];
    let action = statement.action_digest.as_bytes();
    let epoch = u128::from(statement.execution_epoch).to_be_bytes();
    for (row, current) in fixed.rows.iter().enumerate() {
        let next = fixed
            .rows
            .get((row + 1).min(PRIVATE_NOTE_TRACE_SIZE_V1 - 1))
            .ok_or(ProofManagedNoteStarkErrorV1::InvalidProfile)?;
        match current {
            PrivateNoteFixedRowV1::ShaRound { round, block, .. } => {
                set(&mut columns, TYPE_SHA_ROUND, row, F::ONE);
                set(
                    &mut columns,
                    FIXED_ROUND_SELECTOR_OFFSET + usize::from(*round),
                    row,
                    F::ONE,
                );
                if *round == 0 && *block == 0 {
                    set(&mut columns, FIXED_FIRST_BLOCK_ROUND_ZERO, row, F::ONE);
                }
            }
            PrivateNoteFixedRowV1::ShaEnd {
                block,
                block_count,
                digest_chunk,
                public_digest,
                ..
            } => {
                set(&mut columns, TYPE_SHA_END, row, F::ONE);
                let terminal = usize::from(*block) + 1 == usize::from(*block_count);
                if terminal {
                    set(&mut columns, FIXED_SHA_END_TERMINAL, row, F::ONE);
                    set(
                        &mut columns,
                        FIXED_TERMINAL_CHUNK_OFFSET + usize::from(*digest_chunk),
                        row,
                        F::ONE,
                    );
                }
                if *digest_chunk < 3 {
                    set(&mut columns, FIXED_SHA_END_CONTINUE, row, F::ONE);
                } else if !terminal {
                    set(&mut columns, FIXED_SHA_END_NEXT_BLOCK, row, F::ONE);
                }
                if let Some(public_digest) = public_digest {
                    if !terminal {
                        return Err(ProofManagedNoteStarkErrorV1::InvalidProfile);
                    }
                    set(&mut columns, FIXED_SHA_END_PUBLIC_SELECTOR, row, F::ONE);
                    let start = usize::from(*digest_chunk)
                        .checked_mul(PRIVATE_NOTE_COPY_WIDTH_V1)
                        .ok_or(ProofManagedNoteStarkErrorV1::Resource)?;
                    let public_chunk = public_digest
                        .get(start..start + PRIVATE_NOTE_COPY_WIDTH_V1)
                        .ok_or(ProofManagedNoteStarkErrorV1::InvalidProfile)?;
                    for (cell, byte) in public_chunk.iter().copied().enumerate() {
                        set(
                            &mut columns,
                            FIXED_SHA_END_PUBLIC_BYTE_OFFSET + cell,
                            row,
                            f(byte),
                        );
                    }
                }
            }
            PrivateNoteFixedRowV1::NodeSelect { .. } => {
                set(&mut columns, TYPE_NODE_SELECT, row, F::ONE);
            }
            PrivateNoteFixedRowV1::Distinct { chunk, chunks, .. } => {
                set(&mut columns, TYPE_DISTINCT, row, F::ONE);
                set(
                    &mut columns,
                    FIXED_SEQUENCE_FIRST,
                    row,
                    f(u64::from(*chunk == 0)),
                );
                let last = usize::from(*chunk) + 1 == usize::from(*chunks);
                set(&mut columns, FIXED_SEQUENCE_LAST, row, f(u64::from(last)));
                set(
                    &mut columns,
                    FIXED_SEQUENCE_TRANSITION,
                    row,
                    f(u64::from(!last)),
                );
            }
            PrivateNoteFixedRowV1::NonZero { chunk, chunks, .. } => {
                set(&mut columns, TYPE_NONZERO, row, F::ONE);
                set(
                    &mut columns,
                    FIXED_SEQUENCE_FIRST,
                    row,
                    f(u64::from(*chunk == 0)),
                );
                let last = usize::from(*chunk) + 1 == usize::from(*chunks);
                set(&mut columns, FIXED_SEQUENCE_LAST, row, f(u64::from(last)));
                set(
                    &mut columns,
                    FIXED_SEQUENCE_TRANSITION,
                    row,
                    f(u64::from(!last)),
                );
            }
            PrivateNoteFixedRowV1::Sum { side, byte } => {
                let kind = match side {
                    SumSideV1::Inputs | SumSideV1::Outputs => TYPE_SUM_IO,
                    SumSideV1::Conservation => TYPE_SUM_CONSERVATION,
                };
                set(&mut columns, kind, row, F::ONE);
                set(&mut columns, FIXED_SUM_FIRST, row, f(u64::from(*byte == 0)));
                set(&mut columns, FIXED_SUM_LAST, row, f(u64::from(*byte == 15)));
                set(
                    &mut columns,
                    FIXED_SUM_TRANSITION,
                    row,
                    f(u64::from(*byte < 15)),
                );
            }
            PrivateNoteFixedRowV1::VmHeader => {
                set(&mut columns, TYPE_VM_HEADER, row, F::ONE);
            }
            PrivateNoteFixedRowV1::VmProgram { instruction } => {
                set(&mut columns, TYPE_VM_PROGRAM, row, F::ONE);
                set(
                    &mut columns,
                    FIXED_VM_PROGRAM_FIRST,
                    row,
                    f(u64::from(*instruction == 0)),
                );
                set(
                    &mut columns,
                    FIXED_VM_PROGRAM_LAST,
                    row,
                    f(u64::from(usize::from(*instruction) + 1 == 16)),
                );
            }
            PrivateNoteFixedRowV1::VmPrevious { byte, .. } => {
                set(&mut columns, TYPE_VM_PREVIOUS, row, F::ONE);
                set(
                    &mut columns,
                    FIXED_VM_BYTE_SELECTOR_OFFSET + usize::from(*byte),
                    row,
                    F::ONE,
                );
                let byte_index = 15 - usize::from(*byte);
                set(
                    &mut columns,
                    FIXED_VM_ACTION_LIMB_ZERO_BYTE,
                    row,
                    f(action[byte_index]),
                );
                set(
                    &mut columns,
                    FIXED_VM_ACTION_LIMB_ONE_BYTE,
                    row,
                    f(action[16 + byte_index]),
                );
                set(
                    &mut columns,
                    FIXED_VM_EXECUTION_EPOCH_BYTE,
                    row,
                    f(epoch[byte_index]),
                );
            }
            PrivateNoteFixedRowV1::VmNext { byte, .. } => {
                set(&mut columns, TYPE_VM_NEXT, row, F::ONE);
                set(
                    &mut columns,
                    FIXED_VM_BYTE_SELECTOR_OFFSET + usize::from(*byte),
                    row,
                    F::ONE,
                );
                if *byte == 15 && matches!(next, PrivateNoteFixedRowV1::VmProgram { .. }) {
                    set(&mut columns, FIXED_VM_INSTRUCTION_TRANSITION, row, F::ONE);
                }
            }
            PrivateNoteFixedRowV1::Padding => {
                set(&mut columns, TYPE_PADDING, row, F::ONE);
            }
        }
        if vm_same_instruction_transition(current, next) {
            set(&mut columns, FIXED_VM_COMMON_TRANSITION, row, F::ONE);
        }
    }
    Ok(columns)
}
pub(super) fn private_note_profile_aux_columns_v1(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
    base_columns: &[Vec<F>],
) -> Result<Vec<Vec<F>>, ProofManagedNoteStarkErrorV1> {
    if base_columns.len() != PRIVATE_NOTE_BASE_WIDTH_V1
        || base_columns
            .iter()
            .any(|column| column.len() != PRIVATE_NOTE_TRACE_SIZE_V1)
    {
        return Err(ProofManagedNoteStarkErrorV1::InvalidTrace);
    }
    let fixed = build_private_note_fixed_trace_v1(statement).map_err(map_air_error_v1)?;
    let mut bridge = vec![F::ZERO; PRIVATE_NOTE_TRACE_SIZE_V1];
    for row in 1..PRIVATE_NOTE_TRACE_SIZE_V1 {
        if matches!(fixed.rows[row], PrivateNoteFixedRowV1::VmNext { .. }) {
            if !matches!(
                fixed.rows[row - 1],
                PrivateNoteFixedRowV1::VmPrevious { .. }
            ) {
                return Err(ProofManagedNoteStarkErrorV1::InvalidProfile);
            }
            bridge[row] = base_columns[SCRATCH_VM_CARRY_AFTER][row - 1];
        }
    }
    Ok(vec![bridge])
}
pub(super) fn private_note_base_columns_v1(
    trace: &PrivateNoteBaseTraceV1,
) -> Result<Vec<Vec<F>>, ProofManagedNoteStarkErrorV1> {
    if trace.rows.len() != PRIVATE_NOTE_TRACE_SIZE_V1
        || trace
            .rows
            .iter()
            .any(|row| row.len() != PRIVATE_NOTE_BASE_WIDTH_V1)
    {
        return Err(ProofManagedNoteStarkErrorV1::InvalidTrace);
    }
    Ok((0..PRIVATE_NOTE_BASE_WIDTH_V1)
        .map(|column| trace.rows.iter().map(|row| row[column]).collect())
        .collect())
}
fn boolean(value: F) -> F {
    value.mul(value.sub(F::ONE))
}
fn pack_bits(bits: &[F]) -> F {
    bits.iter()
        .copied()
        .enumerate()
        .fold(F::ZERO, |sum, (bit, value)| {
            sum.add(value.mul(F(1_u64 << bit)))
        })
}
fn xor_three(x: F, y: F, z: F) -> F {
    x.add(y)
        .add(z)
        .sub(F(2).mul(x.mul(y).add(x.mul(z)).add(y.mul(z))))
        .add(F(4).mul(x.mul(y).mul(z)))
}
fn choose(x: F, y: F, z: F) -> F {
    x.mul(y).add(F::ONE.sub(x).mul(z))
}
fn majority(x: F, y: F, z: F) -> F {
    x.mul(y)
        .add(x.mul(z))
        .add(y.mul(z))
        .sub(F(2).mul(x.mul(y).mul(z)))
}
fn bits_group(row: &[F], group: usize) -> &[F] {
    let start = SHA_BITS_OFFSET + group * PRIVATE_NOTE_SHA_BITS_PER_GROUP_V1;
    &row[start..start + PRIVATE_NOTE_SHA_BITS_PER_GROUP_V1]
}
fn bit_at(bits: &[F], index: usize) -> F {
    bits[index % 32]
}
fn rotr(bits: &[F], shift: usize, index: usize) -> F {
    bit_at(bits, index + shift)
}
fn shr(bits: &[F], shift: usize, index: usize) -> F {
    if index + shift < 32 {
        bits[index + shift]
    } else {
        F::ZERO
    }
}
fn sigma_small_0_bits(bits: &[F]) -> F {
    (0..32).fold(F::ZERO, |sum, index| {
        sum.add(
            xor_three(
                rotr(bits, 7, index),
                rotr(bits, 18, index),
                shr(bits, 3, index),
            )
            .mul(F(1_u64 << index)),
        )
    })
}
fn sigma_small_1_bits(bits: &[F]) -> F {
    (0..32).fold(F::ZERO, |sum, index| {
        sum.add(
            xor_three(
                rotr(bits, 17, index),
                rotr(bits, 19, index),
                shr(bits, 10, index),
            )
            .mul(F(1_u64 << index)),
        )
    })
}
fn sigma_big_0_bits(bits: &[F]) -> F {
    (0..32).fold(F::ZERO, |sum, index| {
        sum.add(
            xor_three(
                rotr(bits, 2, index),
                rotr(bits, 13, index),
                rotr(bits, 22, index),
            )
            .mul(F(1_u64 << index)),
        )
    })
}
fn sigma_big_1_bits(bits: &[F]) -> F {
    (0..32).fold(F::ZERO, |sum, index| {
        sum.add(
            xor_three(
                rotr(bits, 6, index),
                rotr(bits, 11, index),
                rotr(bits, 25, index),
            )
            .mul(F(1_u64 << index)),
        )
    })
}
fn choose_word(e: &[F], f_bits: &[F], g: &[F]) -> F {
    (0..32).fold(F::ZERO, |sum, index| {
        sum.add(choose(e[index], f_bits[index], g[index]).mul(F(1_u64 << index)))
    })
}
fn majority_word(a: &[F], b: &[F], c: &[F]) -> F {
    (0..32).fold(F::ZERO, |sum, index| {
        sum.add(majority(a[index], b[index], c[index]).mul(F(1_u64 << index)))
    })
}
fn selector_sum(fixed: &[F], range: core::ops::Range<usize>) -> F {
    fixed[range].iter().copied().fold(F::ZERO, F::add)
}
fn selected_schedule(current: &[F], fixed: &[F], index: impl Fn(usize) -> Option<usize>) -> F {
    (0..64).fold(F::ZERO, |sum, round| {
        let Some(schedule_index) = index(round) else {
            return sum;
        };
        sum.add(
            fixed[FIXED_ROUND_SELECTOR_OFFSET + round]
                .mul(current[SHA_SCHEDULE_OFFSET + schedule_index]),
        )
    })
}
fn selected_round_constant(fixed: &[F]) -> F {
    (0..64).fold(F::ZERO, |sum, round| {
        sum.add(
            fixed[FIXED_ROUND_SELECTOR_OFFSET + round]
                .mul(F(u64::from(SHA256_ROUND_CONSTANTS_V1[round]))),
        )
    })
}
fn allowed_selector_for_column(current_fixed: &[F], column: usize) -> F {
    let sha_round = current_fixed[TYPE_SHA_ROUND];
    let sha_end = current_fixed[TYPE_SHA_END];
    let distinct = current_fixed[TYPE_DISTINCT];
    let nonzero = current_fixed[TYPE_NONZERO];
    let sum_io = current_fixed[TYPE_SUM_IO];
    let sum_conservation = current_fixed[TYPE_SUM_CONSERVATION];
    let vm_program = current_fixed[TYPE_VM_PROGRAM];
    let vm_previous = current_fixed[TYPE_VM_PREVIOUS];
    let vm_next = current_fixed[TYPE_VM_NEXT];
    let vm_common = vm_program.add(vm_previous).add(vm_next);
    if (SHA_SCHEDULE_OFFSET..SHA_SCHEDULE_OFFSET + PRIVATE_NOTE_SHA_SCHEDULE_WORDS_V1)
        .contains(&column)
        || (SHA_INITIAL_STATE_OFFSET..SHA_INITIAL_STATE_OFFSET + PRIVATE_NOTE_SHA_STATE_WORDS_V1)
            .contains(&column)
        || (SHA_T1_OFFSET..SHA_CARRY_OFFSET + SHA_CARRY_WIDTH).contains(&column)
    {
        sha_round
    } else if (SHA_STATE_OFFSET..SHA_STATE_OFFSET + PRIVATE_NOTE_SHA_STATE_WORDS_V1)
        .contains(&column)
        || (SHA_BITS_OFFSET..SHA_BITS_OFFSET + PRIVATE_NOTE_SHA_BIT_COLUMNS_V1).contains(&column)
    {
        sha_round.add(sha_end)
    } else if (SCRATCH_NONZERO_BYTE_SELECT_OFFSET..SCRATCH_NONZERO_BYTE_SELECT_OFFSET + 4)
        .contains(&column)
    {
        distinct.add(nonzero)
    } else if (SCRATCH_NONZERO_BYTE_SELECT_OFFSET + 4..SCRATCH_NONZERO_BYTE_SELECT_OFFSET + 8)
        .contains(&column)
    {
        nonzero
    } else if (SCRATCH_NONZERO_BIT_SELECT_OFFSET..SCRATCH_NONZERO_BIT_SELECT_OFFSET + 8)
        .contains(&column)
    {
        distinct.add(nonzero)
    } else if (SCRATCH_BYTE_BITS_OFFSET..SCRATCH_BYTE_BITS_OFFSET + 8).contains(&column) {
        distinct.add(nonzero).add(sum_io)
    } else if (SCRATCH_RUNNING_BEFORE..SCRATCH_RUNNING_AFTER + 1).contains(&column) {
        distinct.add(nonzero)
    } else if (SCRATCH_RELATION_CARRY_BEFORE..SCRATCH_RELATION_CARRY_AFTER + 1).contains(&column) {
        sum_io.add(sum_conservation)
    } else if (SCRATCH_RELATION_CARRY_BITS_OFFSET..SCRATCH_RELATION_CARRY_BITS_OFFSET + 2)
        .contains(&column)
    {
        sum_io
    } else if (SCRATCH_VM_OPCODE_SELECT_OFFSET..SCRATCH_VM_HALTED_AFTER + 1).contains(&column) {
        vm_common
    } else if (SCRATCH_VM_CARRY_BEFORE..SCRATCH_VM_DIFFERENCE_BITS_OFFSET).contains(&column) {
        vm_previous
    } else if (SCRATCH_VM_DIFFERENCE_BITS_OFFSET..SCRATCH_VM_DIFFERENCE_BITS_OFFSET + 8)
        .contains(&column)
    {
        vm_previous.add(distinct)
    } else {
        F::ZERO
    }
}
fn push_weighted(residues: &mut Vec<F>, selector: F, value: F) {
    residues.push(selector.mul(value));
}
fn push_boolean(residues: &mut Vec<F>, selector: F, value: F) {
    push_weighted(residues, selector, boolean(value));
}
const BASE_WIDTH: usize = PRIVATE_NOTE_BASE_WIDTH_V1;
const PROFILE_AUX_WIDTH: usize = PRIVATE_NOTE_PROFILE_AUX_WIDTH_V1;
const PROFILE_FIXED_WIDTH: usize = PRIVATE_NOTE_PROFILE_FIXED_WIDTH_V1;
const SHA_BIT_COLUMNS: usize = PRIVATE_NOTE_SHA_BIT_COLUMNS_V1;
const SHA_STATE_WORDS: usize = PRIVATE_NOTE_SHA_STATE_WORDS_V1;
const SHA_SCHEDULE_WORDS: usize = PRIVATE_NOTE_SHA_SCHEDULE_WORDS_V1;
const COPY_WIDTH: usize = PRIVATE_NOTE_COPY_WIDTH_V1;
const DISTINCT_RIGHT_BITS_OFFSET: usize = SCRATCH_VM_DIFFERENCE_BITS_OFFSET;
const VM_DIFFERENCE_BITS_OFFSET: usize = SCRATCH_VM_DIFFERENCE_BITS_OFFSET;
include!("../shared_note_profile_constraints.rs");
define_note_profile_constraint_residues_v1!(private_note_profile_constraint_residues_inner_v1);
/// Statement-and-consensus-binding adapter used by both prover and verifier.
pub(super) struct PrivateNoteStarkAdapterV1<'a> {
    statement: &'a IrohaIvmPrivateNoteStarkStatementV1,
    consensus_binding: &'a PrivacyNativeConsensusBindingV1,
    consensus_limits: &'a PrivacyConsensusLimitsV1,
}
impl<'a> PrivateNoteStarkAdapterV1<'a> {
    pub(super) const fn new(
        statement: &'a IrohaIvmPrivateNoteStarkStatementV1,
        consensus_binding: &'a PrivacyNativeConsensusBindingV1,
        consensus_limits: &'a PrivacyConsensusLimitsV1,
    ) -> Self {
        Self {
            statement,
            consensus_binding,
            consensus_limits,
        }
    }
}
fn private_note_public_input_digest_v1(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
    consensus_binding_digest: PrivacyNativeConsensusBindingDigestV1,
) -> Result<[u8; 32], ProofManagedNoteStarkErrorV1> {
    let encoded =
        norito::to_bytes(statement).map_err(|_| ProofManagedNoteStarkErrorV1::InvalidProfile)?;
    sha256_frame_v1(
        b"ivm-private-note-stark-public-input-with-consensus-binding-v1",
        &[&encoded, consensus_binding_digest.as_bytes()],
    )
    .map_err(|_| ProofManagedNoteStarkErrorV1::Internal)
}
impl ProofManagedNoteStarkAdapterV1 for PrivateNoteStarkAdapterV1<'_> {
    type ProfileChallenges = ();
    fn protocol_v1(&self) -> ProofManagedNoteStarkProtocolV1 {
        private_note_protocol_v1()
    }
    fn public_input_digest_v1(&self) -> Result<[u8; 32], ProofManagedNoteStarkErrorV1> {
        self.consensus_binding
            .validate_against_context(&self.statement.context, self.consensus_limits)
            .map_err(|_| ProofManagedNoteStarkErrorV1::InvalidProfile)?;
        let binding_digest: PrivacyNativeConsensusBindingDigestV1 = self
            .consensus_binding
            .digest()
            .map_err(|_| ProofManagedNoteStarkErrorV1::InvalidProfile)?;
        private_note_public_input_digest_v1(self.statement, binding_digest)
    }
    fn trace_log2_v1(&self) -> u8 {
        PRIVATE_NOTE_TRACE_LOG2_V1
    }
    fn base_width_v1(&self) -> usize {
        PRIVATE_NOTE_BASE_WIDTH_V1
    }
    fn profile_aux_width_v1(&self) -> usize {
        PRIVATE_NOTE_PROFILE_AUX_WIDTH_V1
    }
    fn profile_fixed_width_v1(&self) -> usize {
        PRIVATE_NOTE_PROFILE_FIXED_WIDTH_V1
    }
    fn profile_constraint_count_v1(&self) -> usize {
        PRIVATE_NOTE_PROFILE_CONSTRAINT_COUNT_V1
    }
    fn copy_schedule_v1(&self) -> Result<NoteCopyScheduleV1, ProofManagedNoteStarkErrorV1> {
        build_private_note_copy_schedule_v1(self.statement).map_err(map_air_error_v1)
    }
    fn profile_fixed_columns_v1(&self) -> Result<Vec<Vec<F>>, ProofManagedNoteStarkErrorV1> {
        private_note_profile_fixed_columns_v1(self.statement)
    }
    fn derive_profile_challenges_v1(
        &self,
        _transcript: &mut TransparentTranscriptV1,
        _copy_challenges: NoteCopyChallengesV1,
    ) -> Result<Self::ProfileChallenges, ProofManagedNoteStarkErrorV1> {
        Ok(())
    }
    fn build_profile_aux_columns_v1(
        &self,
        base_columns: &[Vec<F>],
        _copy_aux_columns: &[Vec<F>],
        _fixed_columns: &[Vec<F>],
        _copy_challenges: NoteCopyChallengesV1,
        _profile_challenges: &Self::ProfileChallenges,
    ) -> Result<Vec<Vec<F>>, ProofManagedNoteStarkErrorV1> {
        private_note_profile_aux_columns_v1(self.statement, base_columns)
    }
    fn profile_constraint_residues_v1(
        &self,
        current_base: &[F],
        next_base: &[F],
        current_aux: &[F],
        next_aux: &[F],
        fixed: &[F],
        _copy_challenges: NoteCopyChallengesV1,
        _profile_challenges: &Self::ProfileChallenges,
    ) -> Result<Vec<F>, ProofManagedNoteStarkErrorV1> {
        private_note_profile_constraint_residues_inner_v1(
            current_base,
            next_base,
            current_aux,
            next_aux,
            fixed,
        )
    }
}
/// Compile and natively validate one canonical prover base trace.
pub(super) fn compile_private_note_prover_columns_v1(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
    witness: &IvmPrivateNoteWitnessV1,
) -> Result<Vec<Vec<F>>, ProofManagedNoteStarkErrorV1> {
    let trace = super::air::build_private_note_base_trace_v1(statement, witness)
        .map_err(map_air_error_v1)?;
    validate_private_note_base_trace_v1(statement, &trace).map_err(map_air_error_v1)?;
    private_note_base_columns_v1(&trace)
}
/// Construct the canonical private-note proof with injected masking entropy.
///
/// The witness is compiled and checked by the native interpreter before the shared proof driver
/// sees any columns. The proof driver then checks the same relation algebraically on the native and
/// extension domains and self-verifies the encoded proof before returning it.
pub(super) fn prove_private_note_stark_v1_with_rng<R: TryRngCore>(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
    consensus_binding: &PrivacyNativeConsensusBindingV1,
    consensus_limits: &PrivacyConsensusLimitsV1,
    witness: &IvmPrivateNoteWitnessV1,
    rng: &mut R,
) -> Result<Vec<u8>, ProofManagedNoteStarkErrorV1> {
    let base_columns = compile_private_note_prover_columns_v1(statement, witness)?;
    prove_proof_managed_note_stark_v1_with_rng(
        &PrivateNoteStarkAdapterV1::new(statement, consensus_binding, consensus_limits),
        &base_columns,
        rng,
    )
}
/// Verify the exact private-note proof against the statement and consensus binding.
pub(crate) fn verify_private_note_stark_v1(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
    consensus_binding: &PrivacyNativeConsensusBindingV1,
    consensus_limits: &PrivacyConsensusLimitsV1,
    proof_bytes: &[u8],
) -> Result<(), ProofManagedNoteStarkErrorV1> {
    verify_proof_managed_note_stark_v1(
        &PrivateNoteStarkAdapterV1::new(statement, consensus_binding, consensus_limits),
        proof_bytes,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::ivm_private_note::tests::fixture;
    use iroha_data_model::privacy::{
        PrivacyActionDigestV1, PrivacyEngineManifestDigestV1,
        PrivacyNativeConsensusBindingValidationErrorV1, PrivacyParameterDigestV1,
        PrivacyParameterIdV1, PrivacyStatementContextV1, PrivacyStatementSchemaDigestV1,
        PrivacyTransactionIntentDigestV1, PrivacyVerifierDigestV1,
    };
    use rand::{SeedableRng as _, rngs::StdRng};
    use sha2::{Digest as _, Sha256};
    fn consensus_material(
        statement: &IrohaIvmPrivateNoteStarkStatementV1,
    ) -> (PrivacyNativeConsensusBindingV1, PrivacyConsensusLimitsV1) {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let binding = PrivacyNativeConsensusBindingV1::new(&statement.context, [0xC1; 32], &limits)
            .expect("valid IVM private-note consensus binding");
        (binding, limits)
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ConsensusBindingAxisV1 {
        NetworkId,
        GenesisHash,
        ActionIndex,
        TransactionIntentDigest,
        ParameterId,
        ParameterDigest,
        VerifierDigest,
        StatementSchemaDigest,
        EngineManifestDigest,
    }
    const CONSENSUS_BINDING_AXES_V1: [ConsensusBindingAxisV1; 9] = [
        ConsensusBindingAxisV1::NetworkId,
        ConsensusBindingAxisV1::GenesisHash,
        ConsensusBindingAxisV1::ActionIndex,
        ConsensusBindingAxisV1::TransactionIntentDigest,
        ConsensusBindingAxisV1::ParameterId,
        ConsensusBindingAxisV1::ParameterDigest,
        ConsensusBindingAxisV1::VerifierDigest,
        ConsensusBindingAxisV1::StatementSchemaDigest,
        ConsensusBindingAxisV1::EngineManifestDigest,
    ];
    impl ConsensusBindingAxisV1 {
        fn mutate_binding(self, binding: &mut PrivacyNativeConsensusBindingV1) {
            match self {
                Self::NetworkId => {
                    binding.network_id =
                        iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                            iroha_data_model::block::BlockHeader,
                        >::from_untyped_unchecked(
                            iroha_crypto::Hash::prehashed([0xD0; 32]),
                        ));
                    binding.genesis_hash = [0xD0; 32];
                }
                Self::GenesisHash => binding.genesis_hash = [0xD0; 32],
                Self::ActionIndex => binding.action_index ^= 1,
                Self::TransactionIntentDigest => {
                    binding.transaction_intent_digest =
                        PrivacyTransactionIntentDigestV1::new([0xD1; 32]);
                }
                Self::ParameterId => {
                    binding.parameter_id = PrivacyParameterIdV1::new([0xD2; 32]);
                }
                Self::ParameterDigest => {
                    binding.parameter_digest = PrivacyParameterDigestV1::new([0xD3; 32]);
                }
                Self::VerifierDigest => {
                    binding.verifier_digest = PrivacyVerifierDigestV1::new([0xD4; 32]);
                }
                Self::StatementSchemaDigest => {
                    binding.statement_schema_digest =
                        PrivacyStatementSchemaDigestV1::new([0xD5; 32]);
                }
                Self::EngineManifestDigest => {
                    binding.engine_manifest_digest = PrivacyEngineManifestDigestV1::new([0xD6; 32]);
                }
            }
        }
        fn mutate_context(self, context: &mut PrivacyStatementContextV1) {
            match self {
                Self::NetworkId => {
                    context.network_id =
                        iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                            iroha_data_model::block::BlockHeader,
                        >::from_untyped_unchecked(
                            iroha_crypto::Hash::prehashed([0xD0; 32]),
                        ));
                }
                Self::GenesisHash => {}
                Self::ActionIndex => context.action_index ^= 1,
                Self::TransactionIntentDigest => {
                    context.transaction_intent_digest =
                        PrivacyTransactionIntentDigestV1::new([0xD1; 32]);
                }
                Self::ParameterId => {
                    context.parameter_id = PrivacyParameterIdV1::new([0xD2; 32]);
                }
                Self::ParameterDigest => {
                    context.parameter_digest = PrivacyParameterDigestV1::new([0xD3; 32]);
                }
                Self::VerifierDigest => {
                    context.verifier_digest = PrivacyVerifierDigestV1::new([0xD4; 32]);
                }
                Self::StatementSchemaDigest => {
                    context.statement_schema_digest =
                        PrivacyStatementSchemaDigestV1::new([0xD5; 32]);
                }
                Self::EngineManifestDigest => {
                    context.engine_manifest_digest = PrivacyEngineManifestDigestV1::new([0xD6; 32]);
                }
            }
        }
        fn mismatch_error(self) -> Option<PrivacyNativeConsensusBindingValidationErrorV1> {
            match self {
                Self::NetworkId => {
                    Some(PrivacyNativeConsensusBindingValidationErrorV1::NetworkIdMismatch)
                }
                Self::GenesisHash => {
                    Some(PrivacyNativeConsensusBindingValidationErrorV1::NetworkGenesisMismatch)
                }
                Self::ActionIndex => {
                    Some(PrivacyNativeConsensusBindingValidationErrorV1::ActionIndexMismatch)
                }
                Self::TransactionIntentDigest => Some(
                    PrivacyNativeConsensusBindingValidationErrorV1::TransactionIntentDigestMismatch,
                ),
                Self::ParameterId => {
                    Some(PrivacyNativeConsensusBindingValidationErrorV1::ParameterIdMismatch)
                }
                Self::ParameterDigest => {
                    Some(PrivacyNativeConsensusBindingValidationErrorV1::ParameterDigestMismatch)
                }
                Self::VerifierDigest => {
                    Some(PrivacyNativeConsensusBindingValidationErrorV1::VerifierDigestMismatch)
                }
                Self::StatementSchemaDigest => Some(
                    PrivacyNativeConsensusBindingValidationErrorV1::StatementSchemaDigestMismatch,
                ),
                Self::EngineManifestDigest => Some(
                    PrivacyNativeConsensusBindingValidationErrorV1::EngineManifestDigestMismatch,
                ),
            }
        }
    }
    fn redigest_statement_v1(statement: &mut IrohaIvmPrivateNoteStarkStatementV1) {
        statement.action_digest = PrivacyActionDigestV1::new([0; 32]);
        statement.action_digest = statement
            .computed_action_digest()
            .expect("canonical substituted action digest");
    }
    fn coordinated_substitution_v1(
        axis: ConsensusBindingAxisV1,
        statement: &IrohaIvmPrivateNoteStarkStatementV1,
        binding: &PrivacyNativeConsensusBindingV1,
        limits: &PrivacyConsensusLimitsV1,
    ) -> Option<(
        IrohaIvmPrivateNoteStarkStatementV1,
        PrivacyNativeConsensusBindingV1,
    )> {
        if axis == ConsensusBindingAxisV1::ActionIndex {
            // The closed Taira profile admits exactly one action, so there is
            // no second valid action index under the same consensus limits.
            return None;
        }
        let mut substituted_statement = statement.clone();
        let substituted_binding = if axis == ConsensusBindingAxisV1::GenesisHash {
            let mut substituted_binding = binding.clone();
            axis.mutate_binding(&mut substituted_binding);
            substituted_binding
        } else {
            axis.mutate_context(&mut substituted_statement.context);
            redigest_statement_v1(&mut substituted_statement);
            let genesis_hash = if axis == ConsensusBindingAxisV1::NetworkId {
                *substituted_statement.context.network_id.as_bytes()
            } else {
                binding.genesis_hash
            };
            PrivacyNativeConsensusBindingV1::new(
                &substituted_statement.context,
                genesis_hash,
                limits,
            )
            .expect("coordinated substituted consensus binding")
        };
        Some((substituted_statement, substituted_binding))
    }
    fn assert_proof_equation_rejection_v1(
        axis: &str,
        result: Result<(), ProofManagedNoteStarkErrorV1>,
    ) {
        assert!(
            matches!(
                result,
                Err(ProofManagedNoteStarkErrorV1::Copy
                    | ProofManagedNoteStarkErrorV1::Constraint
                    | ProofManagedNoteStarkErrorV1::TraceOpening
                    | ProofManagedNoteStarkErrorV1::Composition
                    | ProofManagedNoteStarkErrorV1::Fri
                    | ProofManagedNoteStarkErrorV1::Transcript)
            ),
            "{axis} substitution did not reach a cryptographic proof-equation rejection: {result:?}"
        );
    }
    #[test]
    fn public_input_digest_commits_the_typed_consensus_binding_on_every_axis() {
        let value = fixture();
        let (binding, limits) = consensus_material(&value.statement);
        let canonical_binding_digest: PrivacyNativeConsensusBindingDigestV1 =
            binding.digest().expect("canonical binding digest");
        let expected =
            private_note_public_input_digest_v1(&value.statement, canonical_binding_digest)
                .expect("canonical framed public input");
        assert_eq!(
            PrivateNoteStarkAdapterV1::new(&value.statement, &binding, &limits)
                .public_input_digest_v1(),
            Ok(expected)
        );
        for axis in CONSENSUS_BINDING_AXES_V1 {
            let mut binding_only = binding.clone();
            axis.mutate_binding(&mut binding_only);
            if let Some(expected_error) = axis.mismatch_error() {
                assert_eq!(
                    binding_only.validate_against_context(&value.statement.context, &limits),
                    Err(expected_error),
                    "{axis:?} binding-only substitution reported the wrong mismatch"
                );
                assert_eq!(
                    PrivateNoteStarkAdapterV1::new(&value.statement, &binding_only, &limits,)
                        .public_input_digest_v1(),
                    Err(ProofManagedNoteStarkErrorV1::InvalidProfile),
                    "{axis:?} binding-only substitution entered the proof transcript"
                );
            }
            let Some((substituted_statement, substituted_binding)) =
                coordinated_substitution_v1(axis, &value.statement, &binding, &limits)
            else {
                let mut invalid_statement = value.statement.clone();
                axis.mutate_context(&mut invalid_statement.context);
                redigest_statement_v1(&mut invalid_statement);
                let mut invalid_binding = binding.clone();
                axis.mutate_binding(&mut invalid_binding);
                assert!(
                    invalid_binding
                        .validate_against_context(&invalid_statement.context, &limits)
                        .is_err(),
                    "the one-action profile admitted a second action index"
                );
                continue;
            };
            super::super::relation::validate_statement_v1(&substituted_statement).unwrap_or_else(
                |error| panic!("{axis:?} statement substitution is invalid: {error}"),
            );
            substituted_binding
                .validate_against_context(&substituted_statement.context, &limits)
                .unwrap_or_else(|error| {
                    panic!("{axis:?} binding substitution is invalid: {error}")
                });
            let substituted_digest = PrivateNoteStarkAdapterV1::new(
                &substituted_statement,
                &substituted_binding,
                &limits,
            )
            .public_input_digest_v1()
            .unwrap_or_else(|error| panic!("{axis:?} public input could not be derived: {error}"));
            assert_ne!(
                substituted_digest, expected,
                "{axis:?} did not alter the cryptographic public input"
            );
        }
    }
    #[test]
    fn compiled_profile_count_and_digest_are_exact() {
        validate_ivm_private_note_stark_profile_v1()
            .expect("compiled IVM private-note soundness profile");
        let residues = private_note_profile_constraint_residues_inner_v1(
            &vec![F::ZERO; PRIVATE_NOTE_BASE_WIDTH_V1],
            &vec![F::ZERO; PRIVATE_NOTE_BASE_WIDTH_V1],
            &vec![F::ZERO; NOTE_COPY_AUX_WIDTH_V1 + PRIVATE_NOTE_PROFILE_AUX_WIDTH_V1],
            &vec![F::ZERO; NOTE_COPY_AUX_WIDTH_V1 + PRIVATE_NOTE_PROFILE_AUX_WIDTH_V1],
            &vec![F::ZERO; NOTE_COPY_FIXED_WIDTH_V1 + PRIVATE_NOTE_PROFILE_FIXED_WIDTH_V1],
        )
        .expect("zero rows have the compiled profile shape");
        assert_eq!(residues.len(), PRIVATE_NOTE_PROFILE_CONSTRAINT_COUNT_V1);
        assert_eq!(
            proof_managed_note_stark_profile_digest_v1(
                IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1
            ),
            IVM_PRIVATE_NOTE_STARK_PROFILE_DIGEST_V1
        );
        let expected_descriptor = format!(
            "iroha-ivm-private-note-stark-v1:relation=proof-managed-note:wire=IPS1-v1:trace=2^{}:base={}:profile-aux={}:profile-fixed={}:profile-constraints={}:constraint-degree={}:max-proof={}:sha256-wide-air:public-digest=sha256-frame(canonical-statement,PrivacyNativeConsensusBindingDigestV1):tree-depth=32:vm=16x8:ciphertext=IPNE-v1:fee=separate:legacy=unrepresentable:governance=typed-lifecycle",
            PRIVATE_NOTE_TRACE_LOG2_V1,
            PRIVATE_NOTE_BASE_WIDTH_V1,
            PRIVATE_NOTE_PROFILE_AUX_WIDTH_V1,
            PRIVATE_NOTE_PROFILE_FIXED_WIDTH_V1,
            PRIVATE_NOTE_PROFILE_CONSTRAINT_COUNT_V1,
            PRIVATE_NOTE_PROFILE_CONSTRAINT_DEGREE_V1,
            IVM_PRIVATE_NOTE_MAX_PROOF_BYTES_V1,
        );
        assert_eq!(
            IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1,
            expected_descriptor.as_bytes()
        );
        assert!(
            IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1
                .starts_with(b"iroha-ivm-private-note-stark-v1:")
        );
        assert!(
            IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1.ends_with(b":governance=typed-lifecycle")
        );
        assert!(
            IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1
                .windows(b":wire=IPS1-v1:".len())
                .any(|window| window == b":wire=IPS1-v1:"),
            "the sole first-release proof wire must use the semantic IPS1 tag"
        );
        assert!(
            !IVM_PRIVATE_NOTE_STARK_PROFILE_DESCRIPTOR_V1
                .windows(b"IPN2".len())
                .any(|window| window == b"IPN2"),
            "the superseded draft proof magic must not remain in the profile"
        );
    }
    #[test]
    fn declared_constraint_degree_matches_affine_finite_differences() {
        let measured = crate::privacy_engines::proof_managed_note_stark::degree_audit::
            measured_maximum_affine_degree_v1(
                [0xD4; 32],
                [
                    PRIVATE_NOTE_BASE_WIDTH_V1,
                    PRIVATE_NOTE_BASE_WIDTH_V1,
                    NOTE_COPY_AUX_WIDTH_V1 + PRIVATE_NOTE_PROFILE_AUX_WIDTH_V1,
                    NOTE_COPY_AUX_WIDTH_V1 + PRIVATE_NOTE_PROFILE_AUX_WIDTH_V1,
                    NOTE_COPY_FIXED_WIDTH_V1 + PRIVATE_NOTE_PROFILE_FIXED_WIDTH_V1,
                ],
                21,
                PRIVATE_NOTE_PROFILE_CONSTRAINT_DEGREE_V1,
                private_note_profile_constraint_residues_inner_v1,
            );
        assert_eq!(
            measured,
            usize::from(PRIVATE_NOTE_PROFILE_CONSTRAINT_DEGREE_V1),
            "the IVM private-note AIR's declared maximum degree must be exact"
        );
    }
    #[test]
    #[ignore = "release gate: generates and verifies the full-domain IVM private-note proof"]
    fn full_domain_stark_roundtrip_and_adversarial_wires_fail_closed() {
        let value = fixture();
        let (binding, limits) = consensus_material(&value.statement);
        let mut rng = StdRng::from_seed([0xA9; 32]);
        let proof = super::super::prove_ivm_private_note_v1_with_rng(
            &value.statement,
            &binding,
            &limits,
            &value.witness,
            &mut rng,
        )
        .expect("full-domain private-note facade proof");
        assert!(!proof.is_empty());
        assert!(proof.len() <= PRIVATE_NOTE_PARAMETERS_V1.maximum_proof_bytes);
        super::super::verify_ivm_private_note_v1(&value.statement, &binding, &limits, &proof)
            .expect("full-domain private-note facade verification");
        let mut rejected_draft_magic = proof.clone();
        rejected_draft_magic[..4].copy_from_slice(b"IPN2");
        assert!(
            verify_private_note_stark_v1(
                &value.statement,
                &binding,
                &limits,
                &rejected_draft_magic,
            )
            .is_err(),
            "the superseded IPN2 draft magic was accepted"
        );
        let proof_digest: [u8; 32] = Sha256::digest(&proof).into();
        assert_eq!(proof_digest, IVM_PRIVATE_NOTE_STARK_KAT_PROOF_SHA256_V1);
        assert!(
            verify_private_note_stark_v1(
                &value.statement,
                &binding,
                &limits,
                &vec![0; proof.len()],
            )
            .is_err(),
            "same-length all-zero proof wire was accepted"
        );
        assert!(
            verify_private_note_stark_v1(
                &value.statement,
                &binding,
                &limits,
                &vec![0; PRIVATE_NOTE_PARAMETERS_V1.maximum_proof_bytes + 1],
            )
            .is_err(),
            "oversized proof wire was accepted"
        );
        let mut wires = Vec::new();
        wires.push(Vec::new());
        wires.push(proof[..proof.len() - 1].to_vec());
        let mut trailing = proof.clone();
        trailing.push(0);
        wires.push(trailing);
        for offset in [0, 4, proof.len() / 2, proof.len() - 1] {
            let mut changed = proof.clone();
            changed[offset] ^= 1;
            wires.push(changed);
        }
        for wire in wires {
            assert!(
                verify_private_note_stark_v1(&value.statement, &binding, &limits, &wire).is_err(),
                "malformed private-note proof wire was accepted"
            );
        }
        let mut changed_statement = value.statement.clone();
        let mut nullifier = *changed_statement.nullifiers[0].as_bytes();
        nullifier[0] ^= 1;
        changed_statement.nullifiers[0] =
            iroha_data_model::privacy::PrivacyNullifierV1::new(nullifier);
        changed_statement.action_digest =
            iroha_data_model::privacy::PrivacyActionDigestV1::new([0; 32]);
        changed_statement.action_digest = changed_statement
            .computed_action_digest()
            .expect("recompute substituted action digest");
        super::super::relation::validate_statement_v1(&changed_statement)
            .expect("substituted public statement remains intrinsically valid");
        binding
            .validate_against_context(&changed_statement.context, &limits)
            .expect("non-context statement substitution preserves consensus binding");
        assert_proof_equation_rejection_v1(
            "public nullifier",
            verify_private_note_stark_v1(&changed_statement, &binding, &limits, &proof),
        );
        let mut other_genesis = binding.clone();
        other_genesis.genesis_hash[0] ^= 1;
        other_genesis
            .validate_against_context(&value.statement.context, &limits)
            .expect("changed nonzero genesis remains a valid consensus binding");
        assert_ne!(
            binding
                .digest()
                .expect("canonical consensus-binding digest"),
            other_genesis
                .digest()
                .expect("changed-genesis consensus-binding digest"),
            "changed nonzero genesis did not change the typed consensus-binding digest"
        );
        assert_proof_equation_rejection_v1(
            "nonzero genesis hash",
            verify_private_note_stark_v1(&value.statement, &other_genesis, &limits, &proof),
        );
        for axis in CONSENSUS_BINDING_AXES_V1 {
            if matches!(
                axis,
                ConsensusBindingAxisV1::GenesisHash | ConsensusBindingAxisV1::ActionIndex
            ) {
                continue;
            }
            let (substituted_statement, substituted_binding) =
                coordinated_substitution_v1(axis, &value.statement, &binding, &limits)
                    .expect("this consensus axis has another valid canonical value");
            super::super::relation::validate_statement_v1(&substituted_statement).unwrap_or_else(
                |error| panic!("{axis:?} coordinated statement substitution is invalid: {error}"),
            );
            substituted_binding
                .validate_against_context(&substituted_statement.context, &limits)
                .unwrap_or_else(|error| {
                    panic!("{axis:?} coordinated binding substitution is invalid: {error}")
                });
            assert_proof_equation_rejection_v1(
                &format!("{axis:?}"),
                verify_private_note_stark_v1(
                    &substituted_statement,
                    &substituted_binding,
                    &limits,
                    &proof,
                ),
            );
        }
    }
    #[test]
    fn native_rows_satisfy_every_extension_residue_on_the_trace_domain() {
        let value = fixture();
        let (binding, limits) = consensus_material(&value.statement);
        let trace =
            super::super::air::build_private_note_base_trace_v1(&value.statement, &value.witness)
                .expect("base trace");
        validate_private_note_base_trace_v1(&value.statement, &trace).expect("native oracle");
        let base = private_note_base_columns_v1(&trace).expect("columns");
        let adapter = PrivateNoteStarkAdapterV1::new(&value.statement, &binding, &limits);
        let copy_schedule = adapter.copy_schedule_v1().expect("copy schedule");
        let mut transcript =
            TransparentTranscriptV1::new(b"private-note-residue-test-v1", &[1; 32], &[2; 32])
                .expect("transcript");
        let copy_challenges =
            crate::privacy_engines::proof_managed_note_stark::derive_note_copy_challenges_v1(
                &mut transcript,
            )
            .expect("copy challenges");
        let copy_fixed = copy_schedule
            .fixed_columns_v1(PRIVATE_NOTE_TRACE_SIZE_V1)
            .expect("copy fixed");
        let mut fixed = copy_fixed;
        fixed.extend(
            private_note_profile_fixed_columns_v1(&value.statement).expect("profile fixed"),
        );
        let copy_aux =
            crate::privacy_engines::proof_managed_note_stark::build_note_copy_aux_columns_v1(
                &base,
                &fixed,
                copy_challenges,
                PRIVATE_NOTE_TRACE_SIZE_V1,
            )
            .expect("copy aux");
        let profile_aux =
            private_note_profile_aux_columns_v1(&value.statement, &base).expect("profile aux");
        let mut aux = copy_aux;
        aux.extend(profile_aux);
        let row = |columns: &[Vec<F>], index: usize| {
            columns
                .iter()
                .map(|column| column[index])
                .collect::<Vec<_>>()
        };
        for index in 0..PRIVATE_NOTE_TRACE_SIZE_V1 {
            let next = (index + 1) % PRIVATE_NOTE_TRACE_SIZE_V1;
            let residues = private_note_profile_constraint_residues_inner_v1(
                &row(&base, index),
                &row(&base, next),
                &row(&aux, index),
                &row(&aux, next),
                &row(&fixed, index),
            )
            .expect("residue shape");
            assert_eq!(residues.len(), PRIVATE_NOTE_PROFILE_CONSTRAINT_COUNT_V1);
            assert!(
                residues.iter().all(|residue| *residue == F::ZERO),
                "row {index} has nonzero profile residue at {:?}",
                residues.iter().position(|residue| *residue != F::ZERO)
            );
        }
    }
    #[test]
    fn hostile_prover_cannot_substitute_a_statement_nullifier_after_rebuilding_aux() {
        let value = fixture();
        let trace =
            super::super::air::build_private_note_base_trace_v1(&value.statement, &value.witness)
                .expect("honest base trace");
        let base = private_note_base_columns_v1(&trace).expect("honest base columns");
        let honest_nullifier = value.statement.nullifiers[0];
        let public_row = trace
            .fixed
            .rows
            .iter()
            .enumerate()
            .find_map(|(row, fixed)| match fixed {
                PrivateNoteFixedRowV1::ShaEnd {
                    digest_chunk: 0,
                    public_digest: Some(digest),
                    ..
                } if digest == honest_nullifier.as_bytes() => Some(row),
                _ => None,
            })
            .expect("public nullifier SHA endpoint");
        // Model a hostile prover that keeps a valid private relation trace but
        // asks the verifier to admit a different public replay marker. The
        // complete copy auxiliary trace is rebuilt for the fixed target
        // statement, so rejection cannot be attributed to stale products.
        let mut target_statement = value.statement.clone();
        let mut substituted = *honest_nullifier.as_bytes();
        substituted[0] ^= 1;
        target_statement.nullifiers[0] =
            iroha_data_model::privacy::PrivacyNullifierV1::new(substituted);
        target_statement.action_digest =
            iroha_data_model::privacy::PrivacyActionDigestV1::new([0; 32]);
        target_statement.action_digest = target_statement
            .computed_action_digest()
            .expect("recompute target action digest");
        let (binding, limits) = consensus_material(&target_statement);
        let adapter = PrivateNoteStarkAdapterV1::new(&target_statement, &binding, &limits);
        let copy_schedule = adapter.copy_schedule_v1().expect("target copy schedule");
        let mut fixed = copy_schedule
            .fixed_columns_v1(PRIVATE_NOTE_TRACE_SIZE_V1)
            .expect("target copy fixed");
        fixed.extend(
            private_note_profile_fixed_columns_v1(&target_statement).expect("target profile fixed"),
        );
        let mut transcript =
            TransparentTranscriptV1::new(b"private-note-hostile-prover-v1", &[5; 32], &[6; 32])
                .expect("transcript");
        let copy_challenges =
            crate::privacy_engines::proof_managed_note_stark::derive_note_copy_challenges_v1(
                &mut transcript,
            )
            .expect("copy challenges");
        let copy_aux =
            crate::privacy_engines::proof_managed_note_stark::build_note_copy_aux_columns_v1(
                &base,
                &fixed,
                copy_challenges,
                PRIVATE_NOTE_TRACE_SIZE_V1,
            )
            .expect("hostile prover rebuilds copy auxiliary columns");
        let profile_aux = private_note_profile_aux_columns_v1(&target_statement, &base)
            .expect("hostile prover rebuilds profile auxiliary columns");
        let mut aux = copy_aux;
        aux.extend(profile_aux);
        let row = |columns: &[Vec<F>], index: usize| {
            columns
                .iter()
                .map(|column| column[index])
                .collect::<Vec<_>>()
        };
        let next = (public_row + 1) % PRIVATE_NOTE_TRACE_SIZE_V1;
        let copy_residues =
            crate::privacy_engines::proof_managed_note_stark::note_copy_constraint_residues_v1(
                &row(&base, public_row),
                &row(&aux, public_row),
                &row(&aux, next),
                &row(&fixed, public_row),
                copy_challenges,
            )
            .expect("copy residue shape");
        assert!(
            copy_residues.iter().all(|residue| *residue == F::ZERO),
            "the hostile trace must remain copy-consistent"
        );
        let profile_residues = private_note_profile_constraint_residues_inner_v1(
            &row(&base, public_row),
            &row(&base, next),
            &row(&aux, public_row),
            &row(&aux, next),
            &row(&fixed, public_row),
        )
        .expect("profile residue shape");
        let nonzero = profile_residues
            .iter()
            .copied()
            .filter(|residue| *residue != F::ZERO)
            .collect::<Vec<_>>();
        assert_eq!(
            nonzero,
            vec![F(u64::from(honest_nullifier.as_bytes()[0])).sub(F(u64::from(substituted[0])))],
            "only the verifier-fixed public digest byte binding must reject the rebuilt trace"
        );
    }
    fn mutation_is_detected(
        base: &[Vec<F>],
        aux: &[Vec<F>],
        fixed: &[Vec<F>],
        mutated_row: usize,
    ) -> bool {
        let row = |columns: &[Vec<F>], index: usize| {
            columns
                .iter()
                .map(|column| column[index])
                .collect::<Vec<_>>()
        };
        let previous = (mutated_row + PRIVATE_NOTE_TRACE_SIZE_V1 - 1) % PRIVATE_NOTE_TRACE_SIZE_V1;
        [previous, mutated_row].into_iter().any(|index| {
            let next = (index + 1) % PRIVATE_NOTE_TRACE_SIZE_V1;
            private_note_profile_constraint_residues_inner_v1(
                &row(base, index),
                &row(base, next),
                &row(aux, index),
                &row(aux, next),
                &row(fixed, index),
            )
            .expect("mutation preserves row shape")
            .into_iter()
            .any(|residue| residue != F::ZERO)
        })
    }
    fn assert_base_mutation_detected(
        base: &mut [Vec<F>],
        aux: &[Vec<F>],
        fixed: &[Vec<F>],
        row: usize,
        column: usize,
        family: &str,
    ) {
        let original = base[column][row];
        base[column][row] = original.add(F::ONE);
        assert!(
            mutation_is_detected(base, aux, fixed, row),
            "{family} base mutation was accepted"
        );
        base[column][row] = original;
    }
    fn assert_aux_mutation_detected(
        base: &[Vec<F>],
        aux: &mut [Vec<F>],
        fixed: &[Vec<F>],
        row: usize,
        column: usize,
        family: &str,
    ) {
        let original = aux[column][row];
        aux[column][row] = original.add(F::ONE);
        assert!(
            mutation_is_detected(base, aux, fixed, row),
            "{family} auxiliary mutation was accepted"
        );
        aux[column][row] = original;
    }
    #[test]
    fn extension_residues_reject_mutations_across_every_constraint_family() {
        let value = fixture();
        let (binding, limits) = consensus_material(&value.statement);
        let trace =
            super::super::air::build_private_note_base_trace_v1(&value.statement, &value.witness)
                .expect("base trace");
        let row_types = trace.fixed.rows.clone();
        let mut base = private_note_base_columns_v1(&trace).expect("columns");
        let adapter = PrivateNoteStarkAdapterV1::new(&value.statement, &binding, &limits);
        let copy_schedule = adapter.copy_schedule_v1().expect("copy schedule");
        let mut transcript =
            TransparentTranscriptV1::new(b"private-note-mutation-test-v1", &[3; 32], &[4; 32])
                .expect("transcript");
        let copy_challenges =
            crate::privacy_engines::proof_managed_note_stark::derive_note_copy_challenges_v1(
                &mut transcript,
            )
            .expect("copy challenges");
        let mut fixed = copy_schedule
            .fixed_columns_v1(PRIVATE_NOTE_TRACE_SIZE_V1)
            .expect("copy fixed");
        fixed.extend(
            private_note_profile_fixed_columns_v1(&value.statement).expect("profile fixed"),
        );
        let mut aux =
            crate::privacy_engines::proof_managed_note_stark::build_note_copy_aux_columns_v1(
                &base,
                &fixed,
                copy_challenges,
                PRIVATE_NOTE_TRACE_SIZE_V1,
            )
            .expect("copy aux");
        aux.extend(
            private_note_profile_aux_columns_v1(&value.statement, &base).expect("profile aux"),
        );
        let row_index = |predicate: &dyn Fn(&PrivateNoteFixedRowV1) -> bool| {
            row_types
                .iter()
                .position(predicate)
                .expect("canonical row family")
        };
        let mutations = [
            (
                row_index(&|row| matches!(row, PrivateNoteFixedRowV1::ShaRound { round: 16, .. })),
                SHA_SCHEDULE_OFFSET + 16,
                "SHA schedule",
            ),
            (
                row_index(&|row| {
                    matches!(
                        row,
                        PrivateNoteFixedRowV1::ShaEnd {
                            digest_chunk: 0,
                            ..
                        }
                    )
                }),
                SHA_STATE_OFFSET,
                "SHA endpoint",
            ),
            (
                row_index(&|row| matches!(row, PrivateNoteFixedRowV1::NodeSelect { .. })),
                COPY_OFFSET + 2,
                "Merkle node selection",
            ),
            (
                row_index(&|row| matches!(row, PrivateNoteFixedRowV1::Distinct { .. })),
                SCRATCH_RUNNING_AFTER,
                "distinctness",
            ),
            (
                row_index(&|row| matches!(row, PrivateNoteFixedRowV1::NonZero { .. })),
                SCRATCH_RUNNING_BEFORE,
                "nonzero",
            ),
            (
                row_index(&|row| {
                    matches!(
                        row,
                        PrivateNoteFixedRowV1::Sum {
                            side: SumSideV1::Inputs,
                            ..
                        }
                    )
                }),
                SCRATCH_RELATION_CARRY_AFTER,
                "value sum",
            ),
            (
                row_index(&|row| {
                    matches!(row, PrivateNoteFixedRowV1::VmProgram { instruction: 0 })
                }),
                SCRATCH_VM_OPCODE_SELECT_OFFSET,
                "VM program",
            ),
            (
                row_index(&|row| {
                    matches!(
                        row,
                        PrivateNoteFixedRowV1::VmPrevious {
                            instruction: 0,
                            byte: 0,
                        }
                    )
                }),
                SCRATCH_VM_RESULT_BITS_OFFSET,
                "VM execution",
            ),
            (
                row_index(&|row| matches!(row, PrivateNoteFixedRowV1::Padding)),
                SCRATCH_RUNNING_BEFORE,
                "padding zeroization",
            ),
        ];
        for (row, column, family) in mutations {
            assert_base_mutation_detected(&mut base, &aux, &fixed, row, column, family);
        }
        let vm_next = row_index(&|row| {
            matches!(
                row,
                PrivateNoteFixedRowV1::VmNext {
                    instruction: 0,
                    byte: 0,
                }
            )
        });
        assert_aux_mutation_detected(
            &base,
            &mut aux,
            &fixed,
            vm_next,
            NOTE_COPY_AUX_WIDTH_V1 + PROFILE_AUX_VM_CARRY_BRIDGE,
            "VM carry bridge",
        );
    }
}
