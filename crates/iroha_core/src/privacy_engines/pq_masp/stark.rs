//! Extension-domain adapter for the fixed native PQ-MASP AIR.
//!
//! The verifier reconstructs every selector and constant column from the
//! public statement. The prover commits only witness-bearing base columns;
//! the sole profile auxiliary column is reserved as a zero bridge so the
//! shared first-release note proof driver has one exact topology.

use iroha_data_model::privacy::{
    PqMaspStarkStatementV1, PrivacyConsensusLimitsV1, PrivacyNativeConsensusBindingV1,
};
use rand::TryRngCore;

use super::{
    air::{
        COPY_OFFSET, PQ_MASP_BASE_WIDTH_V1, PQ_MASP_COPY_WIDTH_V1, PQ_MASP_SHA_BIT_COLUMNS_V1,
        PQ_MASP_SHA_BITS_PER_GROUP_V1, PQ_MASP_SHA_SCHEDULE_WORDS_V1, PQ_MASP_SHA_STATE_WORDS_V1,
        PQ_MASP_TRACE_LOG2_V1, PQ_MASP_TRACE_SIZE_V1, PqMaspAirErrorV1, PqMaspBaseTraceV1,
        PqMaspFixedRowV1, SCRATCH_BYTE_BITS_OFFSET, SCRATCH_NONZERO_BIT_SELECT_OFFSET,
        SCRATCH_NONZERO_BYTE_SELECT_OFFSET, SCRATCH_RELATION_CARRY_AFTER,
        SCRATCH_RELATION_CARRY_BEFORE, SCRATCH_RELATION_CARRY_BITS_OFFSET, SCRATCH_RUNNING_AFTER,
        SCRATCH_RUNNING_BEFORE, SCRATCH_VM_CARRY_AFTER, SCRATCH_VM_CARRY_BEFORE,
        SCRATCH_VM_DESTINATION_SELECT_OFFSET, SCRATCH_VM_DIFFERENCE,
        SCRATCH_VM_DIFFERENCE_BITS_OFFSET, SCRATCH_VM_HALTED_AFTER, SCRATCH_VM_HALTED_BEFORE,
        SCRATCH_VM_IMMEDIATE_OFFSET, SCRATCH_VM_LEFT_SELECT_OFFSET,
        SCRATCH_VM_OPCODE_SELECT_OFFSET, SCRATCH_VM_RESULT, SCRATCH_VM_RESULT_BITS_OFFSET,
        SCRATCH_VM_RIGHT_SELECT_OFFSET, SHA_BITS_OFFSET, SHA_CARRY_OFFSET, SHA_CARRY_WIDTH,
        SHA_INITIAL_STATE_OFFSET, SHA_SCHEDULE_OFFSET, SHA_STATE_OFFSET, SHA_T1_OFFSET,
        SHA_T2_OFFSET, SHA256_INITIAL_STATE_V1, SHA256_ROUND_CONSTANTS_V1, SumSideV1,
        build_pq_masp_copy_schedule_v1, build_pq_masp_fixed_trace_v1,
    },
    relation::PqMaspWitnessV1,
};
use crate::privacy_engines::{
    aggregate_stark as aggregate,
    proof_managed_note_stark::{
        NOTE_COPY_AUX_WIDTH_V1, NOTE_COPY_FIXED_WIDTH_V1, NoteCopyChallengesV1, NoteCopyScheduleV1,
        PROOF_MANAGED_NOTE_BLOWUP_LOG2_V1, PROOF_MANAGED_NOTE_COMPOSITION_DEGREE_CHUNKS_V1,
        PROOF_MANAGED_NOTE_MAX_CONSTRAINT_DEGREE_V1, PROOF_MANAGED_NOTE_QUERY_COUNT_V1,
        PROOF_MANAGED_NOTE_SECURITY_LANES_V1, PROOF_MANAGED_NOTE_TERMINAL_DEGREE_BOUND_V1,
        PROOF_MANAGED_NOTE_TERMINAL_LOG2_V1, ProofManagedNoteStarkAdapterV1,
        ProofManagedNoteStarkErrorV1, ProofManagedNoteStarkProtocolV1,
        prove_proof_managed_note_stark_v1, prove_proof_managed_note_stark_v1_with_rng,
        verify_proof_managed_note_stark_v1,
    },
    transparent_stark::{GoldilocksFieldV1 as F, TransparentTranscriptV1, sha256_frame_v1},
};

#[cfg(test)]
use crate::privacy_engines::proof_managed_note_stark::proof_managed_note_stark_profile_digest_v1;

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
const FIXED_SEQUENCE_FIRST: usize = FIXED_SHA_END_PUBLIC_BYTE_OFFSET + PQ_MASP_COPY_WIDTH_V1;
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

pub(super) const PQ_MASP_PROFILE_FIXED_WIDTH_V1: usize = FIXED_VM_EXECUTION_EPOCH_BYTE + 1;
pub(super) const PQ_MASP_PROFILE_AUX_WIDTH_V1: usize = 1;
pub(super) const PQ_MASP_PROFILE_CONSTRAINT_COUNT_V1: usize = 1_372;
/// Audited maximum algebraic degree across the complete shared/profile AIR.
pub(super) const PQ_MASP_PROFILE_CONSTRAINT_DEGREE_V1: u8 =
    PROOF_MANAGED_NOTE_MAX_CONSTRAINT_DEGREE_V1;
const PROFILE_AUX_VM_CARRY_BRIDGE: usize = 0;

/// Relation-local descriptor combined with the shared proof-driver geometry.
pub(crate) const PQ_MASP_STARK_PROFILE_DESCRIPTOR_V1: &[u8] = b"pq-masp-stark-v0:relation=proof-managed-note:wire=PQA1-outer+PQS1-inner-v1:trace=2^14:base=556:profile-aux=1:profile-fixed=122:profile-constraints=1372:constraint-degree=4:max-inner-proof=9431915:sha256-wide-air:public-digest=sha256-frame(canonical-statement,PrivacyNativeConsensusBindingDigestV1):tree-depth=32:authorization=ML-DSA-65(statement-digest+native-consensus-binding-digest-v1+inner-proof-digest):encryption=ML-KEM-768+XChaCha20Poly1305:value=u128-checked:fee=separate:legacy=unrepresentable:governance=typed-lifecycle";

// Framed SHA-256 of the shared proof geometry followed by the relation
// descriptor above. Any compiled profile change must intentionally update the
// relevant descriptor and this digest.
pub(crate) const PQ_MASP_STARK_PROFILE_DIGEST_V1: [u8; 32] = [
    0x2c, 0x18, 0xd6, 0x46, 0x72, 0xba, 0xb6, 0xe8, 0x4c, 0x83, 0x69, 0xd2, 0x6b, 0xbd, 0xc5, 0x3f,
    0x5c, 0x6f, 0x49, 0x36, 0xa4, 0x22, 0x51, 0xb8, 0x52, 0x0b, 0x6b, 0x1c, 0xc7, 0xb2, 0x77, 0xaf,
];

/// SHA-256 of the deterministic full-domain proof generated by the canonical
/// test fixture rebound to the deterministic
/// `pq-masp-full-facade-v1` ML-DSA key and `[0xC2; 32]` genesis hash, with
/// `StdRng::from_seed([0xB7; 32])`.
pub(crate) const PQ_MASP_STARK_KAT_PROOF_SHA256_V1: [u8; 32] = [
    0xe8, 0x0b, 0xce, 0xa4, 0xbc, 0xf6, 0x97, 0xb3, 0x87, 0x57, 0xbd, 0x66, 0xdb, 0x00, 0xc5, 0x58,
    0x49, 0x52, 0x07, 0x13, 0xfa, 0x11, 0x63, 0x8e, 0x4b, 0x2c, 0x7a, 0x21, 0x9b, 0xa1, 0x08, 0xd6,
];

/// SHA-256 of the complete deterministic `PQA1` facade proof from the same
/// canonical full-domain fixture. This pins the reserved block-two hedge,
/// ML-DSA authorization, and exact inner proof as one end-to-end producer.
pub(crate) const PQ_MASP_AUTHORIZED_KAT_PROOF_SHA256_V1: [u8; 32] = [
    0x85, 0x02, 0x21, 0x13, 0x1b, 0x26, 0x2f, 0x04, 0x3f, 0x3b, 0x1d, 0xba, 0x6e, 0x97, 0x7d, 0xec,
    0xfe, 0xf5, 0xcd, 0x3a, 0xa3, 0x93, 0x5e, 0xe2, 0x99, 0xdd, 0xff, 0x5b, 0x62, 0x34, 0x07, 0x60,
];

const PQ_MASP_PARAMETERS_V1: aggregate::AggregateStarkParametersV1 =
    aggregate::AggregateStarkParametersV1 {
        proof_magic: *b"PQS1",
        proof_version: 1,
        security_lanes: PROOF_MANAGED_NOTE_SECURITY_LANES_V1,
        query_count: PROOF_MANAGED_NOTE_QUERY_COUNT_V1,
        blowup_log2: PROOF_MANAGED_NOTE_BLOWUP_LOG2_V1,
        terminal_log2: PROOF_MANAGED_NOTE_TERMINAL_LOG2_V1,
        terminal_degree_bound: PROOF_MANAGED_NOTE_TERMINAL_DEGREE_BOUND_V1,
        composition_degree_chunks: PROOF_MANAGED_NOTE_COMPOSITION_DEGREE_CHUNKS_V1,
        minimum_trace_log2: PQ_MASP_TRACE_LOG2_V1,
        maximum_trace_log2: PQ_MASP_TRACE_LOG2_V1,
        maximum_trace_groups: 1,
        maximum_segment_instances: 1,
        maximum_base_columns_per_instance: PQ_MASP_BASE_WIDTH_V1,
        maximum_aux_columns_per_instance: NOTE_COPY_AUX_WIDTH_V1 + PQ_MASP_PROFILE_AUX_WIDTH_V1,
        maximum_proof_bytes: super::wire::PQ_MASP_MAX_STARK_PROOF_BYTES_V1,
    };

const PQ_MASP_DOMAINS_V1: aggregate::AggregateStarkDomainsV1 = aggregate::AggregateStarkDomainsV1 {
    base_leaf: b"pq-masp-stark-base-leaf-v1",
    base_node: b"pq-masp-stark-base-node-v1",
    aux_leaf: b"pq-masp-stark-aux-leaf-v1",
    aux_node: b"pq-masp-stark-aux-node-v1",
    composition_leaf: b"pq-masp-stark-composition-leaf-v1",
    composition_node: b"pq-masp-stark-composition-node-v1",
    fri_leaf: b"pq-masp-stark-fri-leaf-v1",
    fri_node: b"pq-masp-stark-fri-node-v1",
    layout_label: b"pq-masp-stark-layout-v1",
    base_root_label: b"pq-masp-stark-base-root-v1",
    aux_root_label: b"pq-masp-stark-aux-root-v1",
    composition_root_label: b"pq-masp-stark-composition-root-v1",
    fri_root_label: b"pq-masp-stark-fri-root-v1",
    fri_beta_label: b"pq-masp-stark-fri-beta-v1",
    query_seed: b"pq-masp-stark-query-seed-v1",
};

fn pq_masp_protocol_v1() -> ProofManagedNoteStarkProtocolV1 {
    ProofManagedNoteStarkProtocolV1 {
        parameters: PQ_MASP_PARAMETERS_V1,
        domains: PQ_MASP_DOMAINS_V1,
        maximum_constraint_degree: PQ_MASP_PROFILE_CONSTRAINT_DEGREE_V1,
        profile_digest: PQ_MASP_STARK_PROFILE_DIGEST_V1,
        profile_binding_label: b"pq-masp-stark-profile-v1",
        profile_descriptor: PQ_MASP_STARK_PROFILE_DESCRIPTOR_V1,
        relation_layout_domain: b"pq-masp-stark-relation-layout-v1",
    }
}

/// Validate the complete compiled proof-system and relation profile.
pub(crate) fn validate_pq_masp_stark_profile_v1() -> Result<(), ProofManagedNoteStarkErrorV1> {
    pq_masp_protocol_v1().validate()
}

fn map_air_error_v1(error: PqMaspAirErrorV1) -> ProofManagedNoteStarkErrorV1 {
    match error {
        PqMaspAirErrorV1::Resource => ProofManagedNoteStarkErrorV1::Resource,
        PqMaspAirErrorV1::Copy => ProofManagedNoteStarkErrorV1::Copy,
        PqMaspAirErrorV1::Relation
        | PqMaspAirErrorV1::Topology
        | PqMaspAirErrorV1::Assignment
        | PqMaspAirErrorV1::Sha256 => ProofManagedNoteStarkErrorV1::Constraint,
    }
}

fn f(value: impl Into<u64>) -> F {
    F(value.into())
}

fn set(columns: &mut [Vec<F>], column: usize, row: usize, value: F) {
    columns[column][row] = value;
}

fn vm_same_instruction_transition(current: &PqMaspFixedRowV1, next: &PqMaspFixedRowV1) -> bool {
    match (current, next) {
        (
            PqMaspFixedRowV1::VmProgram { instruction },
            PqMaspFixedRowV1::VmPrevious {
                instruction: next_instruction,
                byte: 0,
            },
        ) => instruction == next_instruction,
        (
            PqMaspFixedRowV1::VmPrevious { instruction, byte },
            PqMaspFixedRowV1::VmNext {
                instruction: next_instruction,
                byte: next_byte,
            },
        ) => instruction == next_instruction && byte == next_byte,
        (
            PqMaspFixedRowV1::VmNext { instruction, byte },
            PqMaspFixedRowV1::VmPrevious {
                instruction: next_instruction,
                byte: next_byte,
            },
        ) => instruction == next_instruction && usize::from(*byte) + 1 == usize::from(*next_byte),
        _ => false,
    }
}

pub(super) fn pq_masp_profile_fixed_columns_v1(
    statement: &PqMaspStarkStatementV1,
) -> Result<Vec<Vec<F>>, ProofManagedNoteStarkErrorV1> {
    let fixed = build_pq_masp_fixed_trace_v1(statement).map_err(map_air_error_v1)?;
    if fixed.rows.len() != PQ_MASP_TRACE_SIZE_V1 {
        return Err(ProofManagedNoteStarkErrorV1::InvalidProfile);
    }
    let mut columns = vec![vec![F::ZERO; PQ_MASP_TRACE_SIZE_V1]; PQ_MASP_PROFILE_FIXED_WIDTH_V1];
    for (row, current) in fixed.rows.iter().enumerate() {
        let next = fixed
            .rows
            .get((row + 1).min(PQ_MASP_TRACE_SIZE_V1 - 1))
            .ok_or(ProofManagedNoteStarkErrorV1::InvalidProfile)?;
        match current {
            PqMaspFixedRowV1::ShaRound { round, block, .. } => {
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
            PqMaspFixedRowV1::ShaEnd {
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
                        .checked_mul(PQ_MASP_COPY_WIDTH_V1)
                        .ok_or(ProofManagedNoteStarkErrorV1::Resource)?;
                    let public_chunk = public_digest
                        .get(start..start + PQ_MASP_COPY_WIDTH_V1)
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
            PqMaspFixedRowV1::NodeSelect { .. } => {
                set(&mut columns, TYPE_NODE_SELECT, row, F::ONE);
            }
            PqMaspFixedRowV1::Distinct { chunk, chunks, .. } => {
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
            PqMaspFixedRowV1::NonZero { chunk, chunks, .. } => {
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
            PqMaspFixedRowV1::Sum { side, byte } => {
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
            PqMaspFixedRowV1::VmHeader
            | PqMaspFixedRowV1::VmProgram { .. }
            | PqMaspFixedRowV1::VmPrevious { .. }
            | PqMaspFixedRowV1::VmNext { .. } => {
                return Err(ProofManagedNoteStarkErrorV1::InvalidProfile);
            }
            PqMaspFixedRowV1::Padding => {
                set(&mut columns, TYPE_PADDING, row, F::ONE);
            }
        }
        if vm_same_instruction_transition(current, next) {
            set(&mut columns, FIXED_VM_COMMON_TRANSITION, row, F::ONE);
        }
    }
    Ok(columns)
}

pub(super) fn pq_masp_profile_aux_columns_v1(
    statement: &PqMaspStarkStatementV1,
    base_columns: &[Vec<F>],
) -> Result<Vec<Vec<F>>, ProofManagedNoteStarkErrorV1> {
    if base_columns.len() != PQ_MASP_BASE_WIDTH_V1
        || base_columns
            .iter()
            .any(|column| column.len() != PQ_MASP_TRACE_SIZE_V1)
    {
        return Err(ProofManagedNoteStarkErrorV1::InvalidTrace);
    }
    let fixed = build_pq_masp_fixed_trace_v1(statement).map_err(map_air_error_v1)?;
    let mut bridge = vec![F::ZERO; PQ_MASP_TRACE_SIZE_V1];
    for row in 1..PQ_MASP_TRACE_SIZE_V1 {
        if matches!(fixed.rows[row], PqMaspFixedRowV1::VmNext { .. }) {
            if !matches!(fixed.rows[row - 1], PqMaspFixedRowV1::VmPrevious { .. }) {
                return Err(ProofManagedNoteStarkErrorV1::InvalidProfile);
            }
            bridge[row] = base_columns[SCRATCH_VM_CARRY_AFTER][row - 1];
        }
    }
    Ok(vec![bridge])
}

pub(super) fn pq_masp_base_columns_v1(
    trace: &PqMaspBaseTraceV1,
) -> Result<Vec<Vec<F>>, ProofManagedNoteStarkErrorV1> {
    if trace.rows.len() != PQ_MASP_TRACE_SIZE_V1
        || trace
            .rows
            .iter()
            .any(|row| row.len() != PQ_MASP_BASE_WIDTH_V1)
    {
        return Err(ProofManagedNoteStarkErrorV1::InvalidTrace);
    }
    Ok((0..PQ_MASP_BASE_WIDTH_V1)
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
    let start = SHA_BITS_OFFSET + group * PQ_MASP_SHA_BITS_PER_GROUP_V1;
    &row[start..start + PQ_MASP_SHA_BITS_PER_GROUP_V1]
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
    if (SHA_SCHEDULE_OFFSET..SHA_SCHEDULE_OFFSET + PQ_MASP_SHA_SCHEDULE_WORDS_V1).contains(&column)
        || (SHA_INITIAL_STATE_OFFSET..SHA_INITIAL_STATE_OFFSET + PQ_MASP_SHA_STATE_WORDS_V1)
            .contains(&column)
        || (SHA_T1_OFFSET..SHA_CARRY_OFFSET + SHA_CARRY_WIDTH).contains(&column)
    {
        sha_round
    } else if (SHA_STATE_OFFSET..SHA_STATE_OFFSET + PQ_MASP_SHA_STATE_WORDS_V1).contains(&column)
        || (SHA_BITS_OFFSET..SHA_BITS_OFFSET + PQ_MASP_SHA_BIT_COLUMNS_V1).contains(&column)
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

fn pq_masp_profile_constraint_residues_inner_v1(
    current: &[F],
    next: &[F],
    current_aux: &[F],
    next_aux: &[F],
    fixed: &[F],
) -> Result<Vec<F>, ProofManagedNoteStarkErrorV1> {
    if current.len() != PQ_MASP_BASE_WIDTH_V1
        || next.len() != PQ_MASP_BASE_WIDTH_V1
        || current_aux.len() != NOTE_COPY_AUX_WIDTH_V1 + PQ_MASP_PROFILE_AUX_WIDTH_V1
        || next_aux.len() != NOTE_COPY_AUX_WIDTH_V1 + PQ_MASP_PROFILE_AUX_WIDTH_V1
        || fixed.len() != NOTE_COPY_FIXED_WIDTH_V1 + PQ_MASP_PROFILE_FIXED_WIDTH_V1
    {
        return Err(ProofManagedNoteStarkErrorV1::InvalidTrace);
    }
    let fixed = &fixed[NOTE_COPY_FIXED_WIDTH_V1..];
    let bridge = current_aux[NOTE_COPY_AUX_WIDTH_V1 + PROFILE_AUX_VM_CARRY_BRIDGE];
    let next_bridge = next_aux[NOTE_COPY_AUX_WIDTH_V1 + PROFILE_AUX_VM_CARRY_BRIDGE];
    let mut residues = Vec::new();

    // Every base cell outside its row-family layout is exactly zero. This
    // closes alternate encodings and makes all scratch reuse unambiguous.
    for (column, value) in current.iter().copied().enumerate().skip(8) {
        let allowed = allowed_selector_for_column(fixed, column);
        residues.push(F::ONE.sub(allowed).mul(value));
    }

    let sha_round = fixed[TYPE_SHA_ROUND];
    let sha_end = fixed[TYPE_SHA_END];
    let sha_any = sha_round.add(sha_end);
    for bit in &current[SHA_BITS_OFFSET..SHA_BITS_OFFSET + PQ_MASP_SHA_BIT_COLUMNS_V1] {
        push_boolean(&mut residues, sha_any, *bit);
    }

    let round_zero = fixed[FIXED_ROUND_SELECTOR_OFFSET];
    let round_initial = selector_sum(
        fixed,
        FIXED_ROUND_SELECTOR_OFFSET..FIXED_ROUND_SELECTOR_OFFSET + 16,
    );
    let round_extended = selector_sum(
        fixed,
        FIXED_ROUND_SELECTOR_OFFSET + 16..FIXED_ROUND_SELECTOR_OFFSET + 64,
    );
    let round_nonlast = selector_sum(
        fixed,
        FIXED_ROUND_SELECTOR_OFFSET..FIXED_ROUND_SELECTOR_OFFSET + 63,
    );
    let round_last = fixed[FIXED_ROUND_SELECTOR_OFFSET + 63];

    for (group, state_index) in [0_usize, 1, 2, 4, 5, 6].into_iter().enumerate() {
        push_weighted(
            &mut residues,
            sha_round,
            pack_bits(bits_group(current, group)).sub(current[SHA_STATE_OFFSET + state_index]),
        );
    }
    let selected_w = selected_schedule(current, fixed, Some);
    residues.push(
        sha_round
            .mul(pack_bits(bits_group(current, 6)))
            .sub(selected_w),
    );
    let selected_minus_2 =
        selected_schedule(current, fixed, |round| (round >= 16).then(|| round - 2));
    let selected_minus_15 =
        selected_schedule(current, fixed, |round| (round >= 16).then(|| round - 15));
    residues.push(
        round_extended
            .mul(pack_bits(bits_group(current, 7)))
            .sub(selected_minus_2),
    );
    residues.push(
        round_extended
            .mul(pack_bits(bits_group(current, 8)))
            .sub(selected_minus_15),
    );
    push_weighted(
        &mut residues,
        round_initial,
        pack_bits(bits_group(current, 7)),
    );
    push_weighted(
        &mut residues,
        round_initial,
        pack_bits(bits_group(current, 8)),
    );
    push_weighted(
        &mut residues,
        sha_round,
        pack_bits(bits_group(current, 9)).sub(current[SHA_T1_OFFSET]),
    );
    push_weighted(
        &mut residues,
        sha_round,
        pack_bits(bits_group(current, 10)).sub(current[SHA_T2_OFFSET]),
    );

    let message_word = current[COPY_OFFSET]
        .mul(F(1 << 24))
        .add(current[COPY_OFFSET + 1].mul(F(1 << 16)))
        .add(current[COPY_OFFSET + 2].mul(F(1 << 8)))
        .add(current[COPY_OFFSET + 3]);
    push_weighted(
        &mut residues,
        round_initial,
        pack_bits(bits_group(current, 6)).sub(message_word),
    );

    let schedule_minus_7 =
        selected_schedule(current, fixed, |round| (round >= 16).then(|| round - 7));
    let schedule_minus_16 =
        selected_schedule(current, fixed, |round| (round >= 16).then(|| round - 16));
    let schedule_carry = current[SHA_CARRY_OFFSET + 6].add(current[SHA_CARRY_OFFSET + 7].mul(F(2)));
    residues.push(
        round_extended
            .mul(
                pack_bits(bits_group(current, 6))
                    .sub(sigma_small_1_bits(bits_group(current, 7)))
                    .sub(sigma_small_0_bits(bits_group(current, 8)))
                    .add(schedule_carry.mul(F(1_u64 << 32))),
            )
            .sub(schedule_minus_7)
            .sub(schedule_minus_16),
    );

    for index in 0..8 {
        push_weighted(
            &mut residues,
            round_zero,
            current[SHA_INITIAL_STATE_OFFSET + index].sub(current[SHA_STATE_OFFSET + index]),
        );
        push_weighted(
            &mut residues,
            fixed[FIXED_FIRST_BLOCK_ROUND_ZERO],
            current[SHA_STATE_OFFSET + index].sub(F(u64::from(SHA256_INITIAL_STATE_V1[index]))),
        );
    }
    for index in 0..PQ_MASP_SHA_SCHEDULE_WORDS_V1 {
        push_weighted(
            &mut residues,
            round_nonlast,
            next[SHA_SCHEDULE_OFFSET + index].sub(current[SHA_SCHEDULE_OFFSET + index]),
        );
    }
    for index in 0..PQ_MASP_SHA_STATE_WORDS_V1 {
        push_weighted(
            &mut residues,
            round_nonlast,
            next[SHA_INITIAL_STATE_OFFSET + index].sub(current[SHA_INITIAL_STATE_OFFSET + index]),
        );
    }

    for carry in 0..6 {
        push_boolean(&mut residues, sha_round, current[SHA_CARRY_OFFSET + carry]);
    }
    for carry in 6..8 {
        push_boolean(
            &mut residues,
            round_extended,
            current[SHA_CARRY_OFFSET + carry],
        );
        push_weighted(
            &mut residues,
            round_initial,
            current[SHA_CARRY_OFFSET + carry],
        );
    }
    for carry in 8..16 {
        push_boolean(&mut residues, round_last, current[SHA_CARRY_OFFSET + carry]);
        push_weighted(
            &mut residues,
            round_nonlast,
            current[SHA_CARRY_OFFSET + carry],
        );
    }
    for carry in 16..SHA_CARRY_WIDTH {
        push_weighted(&mut residues, sha_round, current[SHA_CARRY_OFFSET + carry]);
    }

    let t1_carry = current[SHA_CARRY_OFFSET]
        .add(current[SHA_CARRY_OFFSET + 1].mul(F(2)))
        .add(current[SHA_CARRY_OFFSET + 2].mul(F(4)));
    let t1_equation = current[SHA_STATE_OFFSET + 7]
        .add(sigma_big_1_bits(bits_group(current, 3)))
        .add(choose_word(
            bits_group(current, 3),
            bits_group(current, 4),
            bits_group(current, 5),
        ))
        .add(selected_round_constant(fixed))
        .add(pack_bits(bits_group(current, 6)))
        .sub(current[SHA_T1_OFFSET])
        .sub(t1_carry.mul(F(1_u64 << 32)));
    push_weighted(&mut residues, sha_round, t1_equation);
    let t2_equation = sigma_big_0_bits(bits_group(current, 0))
        .add(majority_word(
            bits_group(current, 0),
            bits_group(current, 1),
            bits_group(current, 2),
        ))
        .sub(current[SHA_T2_OFFSET])
        .sub(current[SHA_CARRY_OFFSET + 3].mul(F(1_u64 << 32)));
    push_weighted(&mut residues, sha_round, t2_equation);

    let new_a = current[SHA_T1_OFFSET]
        .add(current[SHA_T2_OFFSET])
        .sub(current[SHA_CARRY_OFFSET + 4].mul(F(1_u64 << 32)));
    let new_e = current[SHA_STATE_OFFSET + 3]
        .add(current[SHA_T1_OFFSET])
        .sub(current[SHA_CARRY_OFFSET + 5].mul(F(1_u64 << 32)));
    let working_next = [
        new_a,
        current[SHA_STATE_OFFSET],
        current[SHA_STATE_OFFSET + 1],
        current[SHA_STATE_OFFSET + 2],
        new_e,
        current[SHA_STATE_OFFSET + 4],
        current[SHA_STATE_OFFSET + 5],
        current[SHA_STATE_OFFSET + 6],
    ];
    for (index, expected) in working_next.iter().copied().enumerate() {
        push_weighted(
            &mut residues,
            round_nonlast,
            next[SHA_STATE_OFFSET + index].sub(expected),
        );
        push_weighted(
            &mut residues,
            round_last,
            next[SHA_STATE_OFFSET + index].sub(
                current[SHA_INITIAL_STATE_OFFSET + index]
                    .add(expected)
                    .sub(current[SHA_CARRY_OFFSET + 8 + index].mul(F(1_u64 << 32))),
            ),
        );
    }

    // SHA end rows expose terminal digest bytes only through the copy cells.
    for group in 0..8 {
        push_weighted(
            &mut residues,
            sha_end,
            pack_bits(bits_group(current, group)).sub(current[SHA_STATE_OFFSET + group]),
        );
    }
    for group in 8..PQ_MASP_SHA_BIT_COLUMNS_V1 / 32 {
        push_weighted(
            &mut residues,
            sha_end,
            pack_bits(bits_group(current, group)),
        );
    }
    for cell in 0..PQ_MASP_COPY_WIDTH_V1 {
        let mut selected_byte = F::ZERO;
        for chunk in 0..4 {
            let word = chunk * 2 + cell / 4;
            let byte_in_word = cell % 4;
            let first_bit = (3 - byte_in_word) * 8;
            let byte = pack_bits(&bits_group(current, word)[first_bit..first_bit + 8]);
            selected_byte = selected_byte.add(fixed[FIXED_TERMINAL_CHUNK_OFFSET + chunk].mul(byte));
        }
        residues.push(
            fixed[FIXED_SHA_END_TERMINAL]
                .mul(current[COPY_OFFSET + cell])
                .sub(selected_byte),
        );
        residues.push(
            fixed[FIXED_SHA_END_PUBLIC_SELECTOR]
                .mul(current[COPY_OFFSET + cell])
                .sub(fixed[FIXED_SHA_END_PUBLIC_BYTE_OFFSET + cell]),
        );
    }
    for index in 0..8 {
        push_weighted(
            &mut residues,
            fixed[FIXED_SHA_END_CONTINUE],
            next[SHA_STATE_OFFSET + index].sub(current[SHA_STATE_OFFSET + index]),
        );
        push_weighted(
            &mut residues,
            fixed[FIXED_SHA_END_NEXT_BLOCK],
            next[SHA_INITIAL_STATE_OFFSET + index].sub(current[SHA_STATE_OFFSET + index]),
        );
        push_weighted(
            &mut residues,
            fixed[FIXED_SHA_END_NEXT_BLOCK],
            next[SHA_STATE_OFFSET + index].sub(current[SHA_STATE_OFFSET + index]),
        );
    }

    let node = fixed[TYPE_NODE_SELECT];
    let direction = current[COPY_OFFSET + 4];
    push_boolean(&mut residues, node, direction);
    push_weighted(
        &mut residues,
        node,
        current[COPY_OFFSET + 2].sub(
            F::ONE
                .sub(direction)
                .mul(current[COPY_OFFSET])
                .add(direction.mul(current[COPY_OFFSET + 1])),
        ),
    );
    push_weighted(
        &mut residues,
        node,
        current[COPY_OFFSET + 3].sub(
            F::ONE
                .sub(direction)
                .mul(current[COPY_OFFSET + 1])
                .add(direction.mul(current[COPY_OFFSET])),
        ),
    );

    let distinct = fixed[TYPE_DISTINCT];
    let nonzero = fixed[TYPE_NONZERO];
    let sequence = distinct.add(nonzero);
    let running_before = current[SCRATCH_RUNNING_BEFORE];
    let running_after = current[SCRATCH_RUNNING_AFTER];
    push_boolean(&mut residues, sequence, running_before);
    push_boolean(&mut residues, sequence, running_after);
    push_weighted(&mut residues, fixed[FIXED_SEQUENCE_FIRST], running_before);
    push_weighted(
        &mut residues,
        fixed[FIXED_SEQUENCE_LAST],
        running_after.sub(F::ONE),
    );
    push_weighted(
        &mut residues,
        fixed[FIXED_SEQUENCE_TRANSITION],
        next[SCRATCH_RUNNING_BEFORE].sub(running_after),
    );

    let pair_selectors =
        &current[SCRATCH_NONZERO_BYTE_SELECT_OFFSET..SCRATCH_NONZERO_BYTE_SELECT_OFFSET + 4];
    let byte_selectors =
        &current[SCRATCH_NONZERO_BYTE_SELECT_OFFSET..SCRATCH_NONZERO_BYTE_SELECT_OFFSET + 8];
    let bit_selectors =
        &current[SCRATCH_NONZERO_BIT_SELECT_OFFSET..SCRATCH_NONZERO_BIT_SELECT_OFFSET + 8];
    let left_bits = &current[SCRATCH_BYTE_BITS_OFFSET..SCRATCH_BYTE_BITS_OFFSET + 8];
    let right_bits =
        &current[SCRATCH_VM_DIFFERENCE_BITS_OFFSET..SCRATCH_VM_DIFFERENCE_BITS_OFFSET + 8];
    for selector in pair_selectors {
        push_boolean(&mut residues, distinct, *selector);
    }
    for selector in byte_selectors {
        push_boolean(&mut residues, nonzero, *selector);
    }
    for selector in bit_selectors {
        push_boolean(&mut residues, sequence, *selector);
    }
    for bit in left_bits {
        push_boolean(&mut residues, sequence, *bit);
    }
    for bit in right_bits {
        push_boolean(&mut residues, distinct, *bit);
    }
    let distinct_selected = pair_selectors.iter().copied().fold(F::ZERO, F::add);
    let nonzero_selected = byte_selectors.iter().copied().fold(F::ZERO, F::add);
    let selected_bit_count = bit_selectors.iter().copied().fold(F::ZERO, F::add);
    push_boolean(&mut residues, distinct, distinct_selected);
    push_boolean(&mut residues, nonzero, nonzero_selected);
    push_weighted(
        &mut residues,
        distinct,
        selected_bit_count.sub(distinct_selected),
    );
    push_weighted(
        &mut residues,
        nonzero,
        selected_bit_count.sub(nonzero_selected),
    );
    push_weighted(
        &mut residues,
        distinct,
        running_after.sub(running_before).sub(distinct_selected),
    );
    push_weighted(
        &mut residues,
        nonzero,
        running_after.sub(running_before).sub(nonzero_selected),
    );
    let selected_left = pair_selectors
        .iter()
        .copied()
        .enumerate()
        .fold(F::ZERO, |sum, (pair, selector)| {
            sum.add(selector.mul(current[COPY_OFFSET + pair * 2]))
        });
    let selected_right = pair_selectors
        .iter()
        .copied()
        .enumerate()
        .fold(F::ZERO, |sum, (pair, selector)| {
            sum.add(selector.mul(current[COPY_OFFSET + pair * 2 + 1]))
        });
    push_weighted(
        &mut residues,
        distinct,
        pack_bits(left_bits).sub(selected_left),
    );
    push_weighted(
        &mut residues,
        distinct,
        pack_bits(right_bits).sub(selected_right),
    );
    let selected_byte = byte_selectors
        .iter()
        .copied()
        .enumerate()
        .fold(F::ZERO, |sum, (cell, selector)| {
            sum.add(selector.mul(current[COPY_OFFSET + cell]))
        });
    push_weighted(
        &mut residues,
        nonzero,
        pack_bits(left_bits).sub(selected_byte),
    );
    for bit in 0..8 {
        push_weighted(
            &mut residues,
            distinct,
            bit_selectors[bit].mul(left_bits[bit].add(right_bits[bit]).sub(F::ONE)),
        );
        push_weighted(
            &mut residues,
            nonzero,
            bit_selectors[bit].mul(left_bits[bit].sub(F::ONE)),
        );
    }

    let sum_io = fixed[TYPE_SUM_IO];
    let sum_conservation = fixed[TYPE_SUM_CONSERVATION];
    let sum_selector = sum_io.add(sum_conservation);
    let relation_carry_before = current[SCRATCH_RELATION_CARRY_BEFORE];
    let relation_carry_after = current[SCRATCH_RELATION_CARRY_AFTER];
    push_weighted(&mut residues, fixed[FIXED_SUM_FIRST], relation_carry_before);
    push_weighted(&mut residues, fixed[FIXED_SUM_LAST], relation_carry_after);
    push_weighted(
        &mut residues,
        fixed[FIXED_SUM_TRANSITION],
        next[SCRATCH_RELATION_CARRY_BEFORE].sub(relation_carry_after),
    );
    let relation_carry_bits =
        &current[SCRATCH_RELATION_CARRY_BITS_OFFSET..SCRATCH_RELATION_CARRY_BITS_OFFSET + 2];
    for bit in &current[SCRATCH_BYTE_BITS_OFFSET..SCRATCH_BYTE_BITS_OFFSET + 8] {
        push_boolean(&mut residues, sum_io, *bit);
    }
    for bit in relation_carry_bits {
        push_boolean(&mut residues, sum_io, *bit);
    }
    push_boolean(&mut residues, sum_io, relation_carry_before);
    push_weighted(
        &mut residues,
        sum_io,
        relation_carry_after.sub(pack_bits(relation_carry_bits)),
    );
    push_weighted(
        &mut residues,
        sum_io,
        pack_bits(&current[SCRATCH_BYTE_BITS_OFFSET..SCRATCH_BYTE_BITS_OFFSET + 8])
            .sub(current[COPY_OFFSET + 2]),
    );
    push_weighted(
        &mut residues,
        sum_io,
        current[COPY_OFFSET]
            .add(current[COPY_OFFSET + 1])
            .add(relation_carry_before)
            .sub(current[COPY_OFFSET + 2])
            .sub(relation_carry_after.mul(F(256))),
    );
    push_weighted(
        &mut residues,
        sum_conservation,
        relation_carry_before
            .mul(relation_carry_before.sub(F::ONE))
            .mul(relation_carry_before.add(F::ONE)),
    );
    push_weighted(
        &mut residues,
        sum_conservation,
        relation_carry_after
            .mul(relation_carry_after.sub(F::ONE))
            .mul(relation_carry_after.add(F::ONE)),
    );
    push_weighted(
        &mut residues,
        sum_conservation,
        current[COPY_OFFSET]
            .add(current[COPY_OFFSET + 2])
            .add(relation_carry_before)
            .sub(current[COPY_OFFSET + 1])
            .sub(current[COPY_OFFSET + 3])
            .sub(relation_carry_after.mul(F(256))),
    );
    // Keep the family selector consumed even if a future compiler emits no
    // sum rows; the fixed profile still has one exact residue shape.
    residues.push(sum_selector.mul(F::ZERO));

    let vm_header = fixed[TYPE_VM_HEADER];
    let vm_program = fixed[TYPE_VM_PROGRAM];
    let vm_previous = fixed[TYPE_VM_PREVIOUS];
    let vm_next = fixed[TYPE_VM_NEXT];
    let vm_common = vm_program.add(vm_previous).add(vm_next);
    let header = [b'I', b'P', b'N', b'1', 0, 1, 0, 0];
    for (cell, expected) in header.into_iter().enumerate() {
        push_weighted(
            &mut residues,
            vm_header,
            current[COPY_OFFSET + cell].sub(F(u64::from(expected))),
        );
    }

    let opcodes = &current[SCRATCH_VM_OPCODE_SELECT_OFFSET..SCRATCH_VM_OPCODE_SELECT_OFFSET + 9];
    let destinations =
        &current[SCRATCH_VM_DESTINATION_SELECT_OFFSET..SCRATCH_VM_DESTINATION_SELECT_OFFSET + 8];
    let left_selectors = &current[SCRATCH_VM_LEFT_SELECT_OFFSET..SCRATCH_VM_LEFT_SELECT_OFFSET + 8];
    let right_selectors =
        &current[SCRATCH_VM_RIGHT_SELECT_OFFSET..SCRATCH_VM_RIGHT_SELECT_OFFSET + 8];
    for selectors in [opcodes, destinations, left_selectors, right_selectors] {
        for selector in selectors {
            push_boolean(&mut residues, vm_common, *selector);
        }
        push_weighted(
            &mut residues,
            vm_common,
            selectors.iter().copied().fold(F::ZERO, F::add).sub(F::ONE),
        );
    }
    let encoded_selector = |selectors: &[F]| {
        selectors
            .iter()
            .copied()
            .enumerate()
            .fold(F::ZERO, |sum, (index, selector)| {
                sum.add(selector.mul(F(index as u64)))
            })
    };
    push_weighted(
        &mut residues,
        vm_program,
        current[COPY_OFFSET].sub(encoded_selector(opcodes)),
    );
    push_weighted(
        &mut residues,
        vm_program,
        current[COPY_OFFSET + 1].sub(encoded_selector(destinations)),
    );
    push_weighted(
        &mut residues,
        vm_program,
        current[COPY_OFFSET + 2].sub(encoded_selector(left_selectors)),
    );
    push_weighted(
        &mut residues,
        vm_program,
        current[COPY_OFFSET + 3].sub(encoded_selector(right_selectors)),
    );
    for byte in 0..4 {
        push_weighted(
            &mut residues,
            vm_program,
            current[COPY_OFFSET + 4 + byte].sub(current[SCRATCH_VM_IMMEDIATE_OFFSET + byte]),
        );
    }

    let destination = encoded_selector(destinations);
    let left_register = encoded_selector(left_selectors);
    let right_register = encoded_selector(right_selectors);
    let immediate = &current[SCRATCH_VM_IMMEDIATE_OFFSET..SCRATCH_VM_IMMEDIATE_OFFSET + 4];
    let immediate_zero = |residues: &mut Vec<F>, selector: F| {
        for value in immediate {
            push_weighted(residues, selector, *value);
        }
    };
    let halt = opcodes[0];
    let move_immediate = opcodes[1];
    let move_register = opcodes[2];
    let add_checked = opcodes[3];
    let sub_checked = opcodes[4];
    let assert_equal = opcodes[5];
    let assert_less_equal = opcodes[6];
    let load_action = opcodes[7];
    let load_epoch = opcodes[8];
    let program_halt = vm_program.mul(halt);
    push_weighted(&mut residues, program_halt, destination);
    push_weighted(&mut residues, program_halt, left_register);
    push_weighted(&mut residues, program_halt, right_register);
    immediate_zero(&mut residues, program_halt);
    push_weighted(&mut residues, vm_program.mul(move_immediate), left_register);
    push_weighted(
        &mut residues,
        vm_program.mul(move_immediate),
        right_register,
    );
    push_weighted(&mut residues, vm_program.mul(move_register), right_register);
    immediate_zero(&mut residues, vm_program.mul(move_register));
    immediate_zero(&mut residues, vm_program.mul(add_checked.add(sub_checked)));
    push_weighted(
        &mut residues,
        vm_program.mul(assert_equal.add(assert_less_equal)),
        destination,
    );
    immediate_zero(
        &mut residues,
        vm_program.mul(assert_equal.add(assert_less_equal)),
    );
    push_weighted(&mut residues, vm_program.mul(load_action), left_register);
    push_weighted(&mut residues, vm_program.mul(load_action), right_register);
    for value in &immediate[..3] {
        push_weighted(&mut residues, vm_program.mul(load_action), *value);
    }
    push_boolean(&mut residues, vm_program.mul(load_action), immediate[3]);
    push_weighted(&mut residues, vm_program.mul(load_epoch), left_register);
    push_weighted(&mut residues, vm_program.mul(load_epoch), right_register);
    immediate_zero(&mut residues, vm_program.mul(load_epoch));

    let halted_before = current[SCRATCH_VM_HALTED_BEFORE];
    let halted_after = current[SCRATCH_VM_HALTED_AFTER];
    push_boolean(&mut residues, vm_common, halted_before);
    push_boolean(&mut residues, vm_common, halted_after);
    push_weighted(
        &mut residues,
        vm_program,
        halted_after
            .sub(halted_before)
            .sub(halt)
            .add(halted_before.mul(halt)),
    );
    push_weighted(&mut residues, fixed[FIXED_VM_PROGRAM_FIRST], halted_before);
    push_weighted(
        &mut residues,
        fixed[FIXED_VM_PROGRAM_LAST],
        halted_after.sub(F::ONE),
    );
    push_weighted(
        &mut residues,
        vm_program,
        halted_before.mul(F::ONE.sub(halt)),
    );
    for column in SCRATCH_VM_OPCODE_SELECT_OFFSET..SCRATCH_VM_HALTED_AFTER + 1 {
        push_weighted(
            &mut residues,
            fixed[FIXED_VM_COMMON_TRANSITION],
            next[column].sub(current[column]),
        );
    }
    push_weighted(
        &mut residues,
        fixed[FIXED_VM_INSTRUCTION_TRANSITION],
        next[SCRATCH_VM_HALTED_BEFORE].sub(halted_after),
    );

    let result = current[SCRATCH_VM_RESULT];
    let difference = current[SCRATCH_VM_DIFFERENCE];
    let result_bits = &current[SCRATCH_VM_RESULT_BITS_OFFSET..SCRATCH_VM_RESULT_BITS_OFFSET + 8];
    let difference_bits =
        &current[SCRATCH_VM_DIFFERENCE_BITS_OFFSET..SCRATCH_VM_DIFFERENCE_BITS_OFFSET + 8];
    for bit in result_bits.iter().chain(difference_bits) {
        push_boolean(&mut residues, vm_previous, *bit);
    }
    push_weighted(
        &mut residues,
        vm_previous,
        pack_bits(result_bits).sub(result),
    );
    push_weighted(
        &mut residues,
        vm_previous,
        pack_bits(difference_bits).sub(difference),
    );

    let selected_previous = |selectors: &[F]| {
        selectors
            .iter()
            .copied()
            .enumerate()
            .fold(F::ZERO, |sum, (register, selector)| {
                sum.add(selector.mul(current[COPY_OFFSET + register]))
            })
    };
    let selected_next = |selectors: &[F]| {
        selectors
            .iter()
            .copied()
            .enumerate()
            .fold(F::ZERO, |sum, (register, selector)| {
                sum.add(selector.mul(next[COPY_OFFSET + register]))
            })
    };
    let previous_left = selected_previous(left_selectors);
    let previous_right = selected_previous(right_selectors);
    let next_destination = selected_next(destinations);
    let writes = move_immediate
        .add(move_register)
        .add(add_checked)
        .add(sub_checked)
        .add(load_action)
        .add(load_epoch);
    for register in 0..8 {
        push_weighted(
            &mut residues,
            vm_previous,
            F::ONE
                .sub(writes.mul(destinations[register]))
                .mul(next[COPY_OFFSET + register].sub(current[COPY_OFFSET + register])),
        );
    }
    push_weighted(
        &mut residues,
        vm_previous,
        writes.mul(next_destination.sub(result)),
    );

    let immediate_byte = fixed[FIXED_VM_BYTE_SELECTOR_OFFSET]
        .mul(immediate[3])
        .add(fixed[FIXED_VM_BYTE_SELECTOR_OFFSET + 1].mul(immediate[2]))
        .add(fixed[FIXED_VM_BYTE_SELECTOR_OFFSET + 2].mul(immediate[1]))
        .add(fixed[FIXED_VM_BYTE_SELECTOR_OFFSET + 3].mul(immediate[0]));
    push_weighted(
        &mut residues,
        vm_previous.mul(move_immediate),
        result.sub(immediate_byte),
    );
    push_weighted(
        &mut residues,
        vm_previous.mul(move_register),
        result.sub(previous_left),
    );
    push_weighted(
        &mut residues,
        vm_previous.mul(halt.add(assert_equal).add(assert_less_equal)),
        result,
    );
    let expected_action = F::ONE
        .sub(immediate[3])
        .mul(fixed[FIXED_VM_ACTION_LIMB_ZERO_BYTE])
        .add(immediate[3].mul(fixed[FIXED_VM_ACTION_LIMB_ONE_BYTE]));
    push_weighted(
        &mut residues,
        vm_previous.mul(load_action),
        result.sub(expected_action),
    );
    push_weighted(
        &mut residues,
        vm_previous.mul(load_epoch),
        result.sub(fixed[FIXED_VM_EXECUTION_EPOCH_BYTE]),
    );
    push_weighted(
        &mut residues,
        vm_previous.mul(assert_equal),
        previous_left.sub(previous_right),
    );

    let vm_carry_before = current[SCRATCH_VM_CARRY_BEFORE];
    let vm_carry_after = current[SCRATCH_VM_CARRY_AFTER];
    let arithmetic = add_checked.add(sub_checked).add(assert_less_equal);
    push_boolean(&mut residues, vm_previous.mul(arithmetic), vm_carry_before);
    push_boolean(&mut residues, vm_previous.mul(arithmetic), vm_carry_after);
    let non_arithmetic = F::ONE.sub(arithmetic);
    push_weighted(
        &mut residues,
        vm_previous.mul(non_arithmetic),
        vm_carry_before,
    );
    push_weighted(
        &mut residues,
        vm_previous.mul(non_arithmetic),
        vm_carry_after,
    );
    push_weighted(
        &mut residues,
        vm_previous.mul(F::ONE.sub(assert_less_equal)),
        difference,
    );
    push_weighted(
        &mut residues,
        vm_previous.mul(add_checked),
        previous_left
            .add(previous_right)
            .add(vm_carry_before)
            .sub(result)
            .sub(vm_carry_after.mul(F(256))),
    );
    push_weighted(
        &mut residues,
        vm_previous.mul(sub_checked),
        result
            .add(previous_right)
            .add(vm_carry_before)
            .sub(previous_left)
            .sub(vm_carry_after.mul(F(256))),
    );
    push_weighted(
        &mut residues,
        vm_previous.mul(assert_less_equal),
        difference
            .add(previous_left)
            .add(vm_carry_before)
            .sub(previous_right)
            .sub(vm_carry_after.mul(F(256))),
    );
    let vm_byte_zero = fixed[FIXED_VM_BYTE_SELECTOR_OFFSET];
    let vm_byte_last = fixed[FIXED_VM_BYTE_SELECTOR_OFFSET + 15];
    push_weighted(
        &mut residues,
        vm_previous.mul(vm_byte_zero),
        vm_carry_before,
    );
    push_weighted(&mut residues, vm_previous.mul(vm_byte_last), vm_carry_after);

    residues.push(F::ONE.sub(vm_next).mul(bridge));
    push_boolean(&mut residues, vm_next, bridge);
    push_weighted(&mut residues, vm_previous, next_bridge.sub(vm_carry_after));
    push_weighted(
        &mut residues,
        vm_next.mul(F::ONE.sub(vm_byte_last)),
        next[SCRATCH_VM_CARRY_BEFORE].sub(bridge),
    );
    push_weighted(&mut residues, vm_next.mul(vm_byte_last), bridge);

    Ok(residues)
}

/// Statement-and-consensus-binding adapter used by both prover and verifier.
pub(super) struct PqMaspStarkAdapterV1<'a> {
    statement: &'a PqMaspStarkStatementV1,
    consensus_binding: &'a PrivacyNativeConsensusBindingV1,
    consensus_limits: &'a PrivacyConsensusLimitsV1,
}

impl<'a> PqMaspStarkAdapterV1<'a> {
    pub(super) const fn new(
        statement: &'a PqMaspStarkStatementV1,
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

impl ProofManagedNoteStarkAdapterV1 for PqMaspStarkAdapterV1<'_> {
    type ProfileChallenges = ();

    fn protocol_v1(&self) -> ProofManagedNoteStarkProtocolV1 {
        pq_masp_protocol_v1()
    }

    fn public_input_digest_v1(&self) -> Result<[u8; 32], ProofManagedNoteStarkErrorV1> {
        self.consensus_binding
            .validate_against_context(&self.statement.context, self.consensus_limits)
            .map_err(|_| ProofManagedNoteStarkErrorV1::InvalidProfile)?;
        let encoded = norito::to_bytes(self.statement)
            .map_err(|_| ProofManagedNoteStarkErrorV1::InvalidProfile)?;
        let binding_digest = self
            .consensus_binding
            .digest()
            .map_err(|_| ProofManagedNoteStarkErrorV1::InvalidProfile)?;
        sha256_frame_v1(
            b"pq-masp-stark-public-input-with-consensus-binding-v1",
            &[&encoded, binding_digest.as_bytes()],
        )
        .map_err(|_| ProofManagedNoteStarkErrorV1::Internal)
    }

    fn trace_log2_v1(&self) -> u8 {
        PQ_MASP_TRACE_LOG2_V1
    }

    fn base_width_v1(&self) -> usize {
        PQ_MASP_BASE_WIDTH_V1
    }

    fn profile_aux_width_v1(&self) -> usize {
        PQ_MASP_PROFILE_AUX_WIDTH_V1
    }

    fn profile_fixed_width_v1(&self) -> usize {
        PQ_MASP_PROFILE_FIXED_WIDTH_V1
    }

    fn profile_constraint_count_v1(&self) -> usize {
        PQ_MASP_PROFILE_CONSTRAINT_COUNT_V1
    }

    fn copy_schedule_v1(&self) -> Result<NoteCopyScheduleV1, ProofManagedNoteStarkErrorV1> {
        build_pq_masp_copy_schedule_v1(self.statement).map_err(map_air_error_v1)
    }

    fn profile_fixed_columns_v1(&self) -> Result<Vec<Vec<F>>, ProofManagedNoteStarkErrorV1> {
        pq_masp_profile_fixed_columns_v1(self.statement)
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
        pq_masp_profile_aux_columns_v1(self.statement, base_columns)
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
        pq_masp_profile_constraint_residues_inner_v1(
            current_base,
            next_base,
            current_aux,
            next_aux,
            fixed,
        )
    }
}

/// Compile and natively validate one canonical prover base trace.
pub(super) fn compile_pq_masp_prover_columns_v1(
    statement: &PqMaspStarkStatementV1,
    witness: &PqMaspWitnessV1,
) -> Result<Vec<Vec<F>>, ProofManagedNoteStarkErrorV1> {
    let trace =
        super::air::build_pq_masp_base_trace_v1(statement, witness).map_err(map_air_error_v1)?;
    pq_masp_base_columns_v1(&trace)
}

/// Construct the canonical PQ-MASP proof with injected masking entropy.
///
/// The witness is compiled and checked by the native interpreter before the
/// shared proof driver sees any columns. The proof driver then checks the same
/// relation algebraically on the native and extension domains and
/// self-verifies the encoded proof before returning it.
pub(super) fn prove_pq_masp_stark_v1_with_rng<R: TryRngCore>(
    statement: &PqMaspStarkStatementV1,
    consensus_binding: &PrivacyNativeConsensusBindingV1,
    consensus_limits: &PrivacyConsensusLimitsV1,
    witness: &PqMaspWitnessV1,
    rng: &mut R,
) -> Result<Vec<u8>, ProofManagedNoteStarkErrorV1> {
    let base_columns = compile_pq_masp_prover_columns_v1(statement, witness)?;
    prove_proof_managed_note_stark_v1_with_rng(
        &PqMaspStarkAdapterV1::new(statement, consensus_binding, consensus_limits),
        &base_columns,
        rng,
    )
}

/// Construct the canonical PQ-MASP proof with operating-system entropy.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn prove_pq_masp_stark_v1(
    statement: &PqMaspStarkStatementV1,
    consensus_binding: &PrivacyNativeConsensusBindingV1,
    consensus_limits: &PrivacyConsensusLimitsV1,
    witness: &PqMaspWitnessV1,
) -> Result<Vec<u8>, ProofManagedNoteStarkErrorV1> {
    let base_columns = compile_pq_masp_prover_columns_v1(statement, witness)?;
    prove_proof_managed_note_stark_v1(
        &PqMaspStarkAdapterV1::new(statement, consensus_binding, consensus_limits),
        &base_columns,
    )
}

/// Verify the exact PQ-MASP proof against the statement and consensus binding.
pub(crate) fn verify_pq_masp_stark_v1(
    statement: &PqMaspStarkStatementV1,
    consensus_binding: &PrivacyNativeConsensusBindingV1,
    consensus_limits: &PrivacyConsensusLimitsV1,
    proof_bytes: &[u8],
) -> Result<(), ProofManagedNoteStarkErrorV1> {
    verify_proof_managed_note_stark_v1(
        &PqMaspStarkAdapterV1::new(statement, consensus_binding, consensus_limits),
        proof_bytes,
    )
}

#[cfg(test)]
mod tests {
    use rand::{RngCore as _, SeedableRng as _, rngs::StdRng};
    use sha2::{Digest as _, Sha256};
    use soranet_pq::{HedgedRngSeed, MlDsaSuite, generate_mldsa_keypair_from_seed};

    use super::*;
    use crate::privacy_engines::{
        pq_masp::{
            derive_pq_masp_authorization_key_digest_v1,
            relation::tests::{
                valid_fixture, valid_fixture_with_authorization_key_digest,
                valid_two_by_two_fixture,
            },
            wire::{
                PqMaspWireErrorV1, authorize_pq_masp_stark_proof_v1,
                decode_pq_masp_authorization_proof_v1, verify_pq_masp_authorization_v1,
            },
        },
        proof_managed_note_stark::{
            NoteCopyCellPolicyV1, build_note_copy_aux_columns_v1, derive_note_copy_challenges_v1,
            note_copy_constraint_residues_v1,
        },
    };

    fn row(columns: &[Vec<F>], index: usize) -> Vec<F> {
        columns.iter().map(|column| column[index]).collect()
    }

    fn consensus_material(
        statement: &PqMaspStarkStatementV1,
    ) -> (PrivacyNativeConsensusBindingV1, PrivacyConsensusLimitsV1) {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let binding = PrivacyNativeConsensusBindingV1::new(&statement.context, [0xC2; 32], &limits)
            .expect("valid PQ-MASP consensus binding");
        (binding, limits)
    }

    fn prepared_columns(
        statement: &PqMaspStarkStatementV1,
        witness: &PqMaspWitnessV1,
        transcript_domain: &'static [u8],
    ) -> (
        super::super::air::PqMaspBaseTraceV1,
        Vec<Vec<F>>,
        Vec<Vec<F>>,
        Vec<Vec<F>>,
        NoteCopyChallengesV1,
    ) {
        let trace = super::super::air::build_pq_masp_base_trace_v1(statement, witness)
            .expect("canonical PQ-MASP trace");
        let base = pq_masp_base_columns_v1(&trace).expect("base columns");
        let (binding, limits) = consensus_material(statement);
        let adapter = PqMaspStarkAdapterV1::new(statement, &binding, &limits);
        let copy_schedule = adapter.copy_schedule_v1().expect("copy schedule");
        let mut transcript = TransparentTranscriptV1::new(transcript_domain, &[1; 32], &[2; 32])
            .expect("test transcript");
        let copy_challenges =
            derive_note_copy_challenges_v1(&mut transcript).expect("copy challenges");
        let mut fixed = copy_schedule
            .fixed_columns_v1(PQ_MASP_TRACE_SIZE_V1)
            .expect("copy fixed columns");
        fixed.extend(pq_masp_profile_fixed_columns_v1(statement).expect("profile fixed columns"));
        let mut aux =
            build_note_copy_aux_columns_v1(&base, &fixed, copy_challenges, PQ_MASP_TRACE_SIZE_V1)
                .expect("copy auxiliary columns");
        aux.extend(
            pq_masp_profile_aux_columns_v1(statement, &base).expect("profile auxiliary columns"),
        );
        (trace, base, aux, fixed, copy_challenges)
    }

    #[test]
    fn compiled_profile_count_digest_and_outer_wire_budget_are_exact() {
        validate_pq_masp_stark_profile_v1().expect("compiled PQ-MASP soundness profile");
        let residues = pq_masp_profile_constraint_residues_inner_v1(
            &vec![F::ZERO; PQ_MASP_BASE_WIDTH_V1],
            &vec![F::ZERO; PQ_MASP_BASE_WIDTH_V1],
            &vec![F::ZERO; NOTE_COPY_AUX_WIDTH_V1 + PQ_MASP_PROFILE_AUX_WIDTH_V1],
            &vec![F::ZERO; NOTE_COPY_AUX_WIDTH_V1 + PQ_MASP_PROFILE_AUX_WIDTH_V1],
            &vec![F::ZERO; NOTE_COPY_FIXED_WIDTH_V1 + PQ_MASP_PROFILE_FIXED_WIDTH_V1],
        )
        .expect("zero rows have the compiled profile shape");
        assert_eq!(residues.len(), PQ_MASP_PROFILE_CONSTRAINT_COUNT_V1);
        assert_eq!(
            proof_managed_note_stark_profile_digest_v1(PQ_MASP_STARK_PROFILE_DESCRIPTOR_V1),
            PQ_MASP_STARK_PROFILE_DIGEST_V1
        );
        let expected_descriptor = format!(
            "pq-masp-stark-v0:relation=proof-managed-note:wire=PQA1-outer+PQS1-inner-v1:trace=2^{}:base={}:profile-aux={}:profile-fixed={}:profile-constraints={}:constraint-degree={}:max-inner-proof={}:sha256-wide-air:public-digest=sha256-frame(canonical-statement,PrivacyNativeConsensusBindingDigestV1):tree-depth=32:authorization=ML-DSA-65(statement-digest+native-consensus-binding-digest-v1+inner-proof-digest):encryption=ML-KEM-768+XChaCha20Poly1305:value=u128-checked:fee=separate:legacy=unrepresentable:governance=typed-lifecycle",
            PQ_MASP_TRACE_LOG2_V1,
            PQ_MASP_BASE_WIDTH_V1,
            PQ_MASP_PROFILE_AUX_WIDTH_V1,
            PQ_MASP_PROFILE_FIXED_WIDTH_V1,
            PQ_MASP_PROFILE_CONSTRAINT_COUNT_V1,
            PQ_MASP_PROFILE_CONSTRAINT_DEGREE_V1,
            super::super::wire::PQ_MASP_MAX_STARK_PROOF_BYTES_V1,
        );
        assert_eq!(
            PQ_MASP_STARK_PROFILE_DESCRIPTOR_V1,
            expected_descriptor.as_bytes()
        );
        assert!(
            PQ_MASP_STARK_PROFILE_DESCRIPTOR_V1.starts_with(b"pq-masp-stark-v0:"),
            "the proof profile must use the sole canonical protocol identifier"
        );
        assert!(PQ_MASP_STARK_PROFILE_DESCRIPTOR_V1.ends_with(b":governance=typed-lifecycle"));
        assert_eq!(
            PQ_MASP_PARAMETERS_V1.maximum_proof_bytes,
            super::super::wire::PQ_MASP_MAX_STARK_PROOF_BYTES_V1
        );
    }

    #[test]
    fn declared_constraint_degree_matches_affine_finite_differences() {
        let measured = crate::privacy_engines::proof_managed_note_stark::degree_audit::
            measured_maximum_affine_degree_v1(
                [0xE4; 32],
                [
                    PQ_MASP_BASE_WIDTH_V1,
                    PQ_MASP_BASE_WIDTH_V1,
                    NOTE_COPY_AUX_WIDTH_V1 + PQ_MASP_PROFILE_AUX_WIDTH_V1,
                    NOTE_COPY_AUX_WIDTH_V1 + PQ_MASP_PROFILE_AUX_WIDTH_V1,
                    NOTE_COPY_FIXED_WIDTH_V1 + PQ_MASP_PROFILE_FIXED_WIDTH_V1,
                ],
                21,
                PQ_MASP_PROFILE_CONSTRAINT_DEGREE_V1,
                pq_masp_profile_constraint_residues_inner_v1,
            );
        assert_eq!(
            measured,
            usize::from(PQ_MASP_PROFILE_CONSTRAINT_DEGREE_V1),
            "the PQ-MASP AIR's declared maximum degree must be exact"
        );
    }

    #[test]
    fn full_domain_authorized_facade_roundtrip_and_adversarial_wires_fail_closed() {
        let authorization_keys = generate_mldsa_keypair_from_seed(
            MlDsaSuite::MlDsa65,
            HedgedRngSeed::from_entropy([0xB6; 32]),
            b"pq-masp-full-facade-v1",
        )
        .expect("ML-DSA authorization key");
        let key_digest =
            derive_pq_masp_authorization_key_digest_v1(authorization_keys.public_key())
                .expect("authorization key digest");
        let (statement, witness) = valid_fixture_with_authorization_key_digest(key_digest);
        let (binding, limits) = consensus_material(&statement);
        let wrong_authorization_keys = generate_mldsa_keypair_from_seed(
            MlDsaSuite::MlDsa65,
            HedgedRngSeed::from_entropy([0xB8; 32]),
            b"pq-masp-wrong-facade-key-v1",
        )
        .expect("wrong ML-DSA authorization key");
        let mut untouched_rng = StdRng::from_seed([0xB9; 32]);
        let mut expected_rng = StdRng::from_seed([0xB9; 32]);
        assert_eq!(
            super::super::prove_pq_masp_v1_with_rng(
                &statement,
                &binding,
                &limits,
                &witness,
                wrong_authorization_keys.secret_key(),
                &mut untouched_rng,
            ),
            Err(super::super::PqMaspProofErrorV1::Authorization(
                PqMaspWireErrorV1::AuthorizationKeyMismatch,
            ))
        );
        assert_eq!(
            untouched_rng.next_u64(),
            expected_rng.next_u64(),
            "invalid authorization keys must fail before entropy is consumed"
        );
        let mut rng = StdRng::from_seed([0xB7; 32]);
        let authorized_proof = super::super::prove_pq_masp_v1_with_rng(
            &statement,
            &binding,
            &limits,
            &witness,
            authorization_keys.secret_key(),
            &mut rng,
        )
        .expect("full-domain authorized PQ-MASP facade proof");
        super::super::verify_pq_masp_v1(&statement, &binding, &limits, &authorized_proof)
            .expect("full-domain PQ-MASP facade verification");
        let decoded = decode_pq_masp_authorization_proof_v1(&authorized_proof)
            .expect("canonical authorization wrapper");
        let proof = decoded.stark_proof.to_vec();
        assert!(!proof.is_empty());
        assert!(proof.len() <= super::super::wire::PQ_MASP_MAX_STARK_PROOF_BYTES_V1);
        verify_pq_masp_stark_v1(&statement, &binding, &limits, &proof)
            .expect("full-domain PQ-MASP verification");
        let proof_digest: [u8; 32] = Sha256::digest(&proof).into();
        let authorized_proof_digest: [u8; 32] = Sha256::digest(&authorized_proof).into();
        assert_eq!(proof_digest, PQ_MASP_STARK_KAT_PROOF_SHA256_V1);
        assert_eq!(
            authorized_proof_digest,
            PQ_MASP_AUTHORIZED_KAT_PROOF_SHA256_V1
        );

        assert!(
            verify_pq_masp_stark_v1(&statement, &binding, &limits, &vec![0; proof.len()],).is_err(),
            "same-length all-zero inner proof wire was accepted"
        );
        assert!(
            verify_pq_masp_stark_v1(
                &statement,
                &binding,
                &limits,
                &vec![0; PQ_MASP_PARAMETERS_V1.maximum_proof_bytes + 1],
            )
            .is_err(),
            "oversized inner proof wire was accepted"
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
                verify_pq_masp_stark_v1(&statement, &binding, &limits, &wire).is_err(),
                "malformed inner proof wire was accepted"
            );
        }

        let mut changed_statement = statement.clone();
        let mut nullifier = *changed_statement.nullifiers[0].as_bytes();
        nullifier[0] ^= 1;
        changed_statement.nullifiers[0] =
            iroha_data_model::privacy::PrivacyNullifierV1::new(nullifier);
        assert!(
            verify_pq_masp_stark_v1(&changed_statement, &binding, &limits, &proof).is_err(),
            "proof was replayed under a substituted public statement"
        );

        for offset in [0, 8, authorized_proof.len() / 2, authorized_proof.len() - 1] {
            let mut tampered = authorized_proof.clone();
            tampered[offset] ^= 1;
            assert!(
                super::super::verify_pq_masp_v1(&statement, &binding, &limits, &tampered,).is_err(),
                "tampered complete PQ-MASP proof byte {offset} was accepted"
            );
        }
        let mut wrong_key_statement = statement.clone();
        wrong_key_statement.authorization_key_digest =
            iroha_data_model::privacy::PrivacyAuthorizationKeyDigestV1::new([0xD1; 32]);
        assert_eq!(
            super::super::verify_pq_masp_v1(
                &wrong_key_statement,
                &binding,
                &limits,
                &authorized_proof,
            ),
            Err(super::super::PqMaspProofErrorV1::Authorization(
                PqMaspWireErrorV1::AuthorizationKeyMismatch,
            ))
        );
        assert!(
            super::super::verify_pq_masp_v1(
                &statement,
                &binding,
                &limits,
                &vec![0; super::super::wire::PQ_MASP_MAX_AUTHORIZATION_PROOF_BYTES_V1 + 1],
            )
            .is_err()
        );

        let mut context_substitutions = Vec::new();
        macro_rules! push_context_substitution {
            ($axis:literal, $field:ident, $value:expr) => {{
                let mut substituted_statement = statement.clone();
                substituted_statement.context.$field = $value;
                let mut substituted_binding = binding.clone();
                substituted_binding.$field = substituted_statement.context.$field.clone();
                context_substitutions.push(($axis, substituted_statement, substituted_binding));
            }};
        }
        push_context_substitution!(
            "chain_id",
            chain_id,
            "pq-masp-substituted-chain"
                .parse()
                .expect("substituted chain id")
        );
        push_context_substitution!(
            "transaction_intent_digest",
            transaction_intent_digest,
            iroha_data_model::privacy::PrivacyTransactionIntentDigestV1::new([0xD2; 32])
        );
        push_context_substitution!(
            "parameter_id",
            parameter_id,
            iroha_data_model::privacy::PrivacyParameterIdV1::new([0xD3; 32])
        );
        push_context_substitution!(
            "parameter_digest",
            parameter_digest,
            iroha_data_model::privacy::PrivacyParameterDigestV1::new([0xD4; 32])
        );
        push_context_substitution!(
            "verifier_digest",
            verifier_digest,
            iroha_data_model::privacy::PrivacyVerifierDigestV1::new([0xD5; 32])
        );
        push_context_substitution!(
            "statement_schema_digest",
            statement_schema_digest,
            iroha_data_model::privacy::PrivacyStatementSchemaDigestV1::new([0xD6; 32])
        );
        push_context_substitution!(
            "engine_manifest_digest",
            engine_manifest_digest,
            iroha_data_model::privacy::PrivacyEngineManifestDigestV1::new([0xD7; 32])
        );
        for (axis, substituted_statement, substituted_binding) in context_substitutions {
            assert_eq!(
                super::super::verify_pq_masp_v1(
                    &substituted_statement,
                    &substituted_binding,
                    &limits,
                    &authorized_proof,
                ),
                Err(super::super::PqMaspProofErrorV1::Authorization(
                    PqMaspWireErrorV1::AuthorizationFailed,
                )),
                "matched statement/binding substitution on {axis} was accepted"
            );
        }

        let mut substituted_action_statement = statement.clone();
        substituted_action_statement.context.action_index = statement.context.action_index + 1;
        let mut substituted_action_binding = binding.clone();
        substituted_action_binding.action_index = substituted_action_statement.context.action_index;
        assert_eq!(
            super::super::verify_pq_masp_v1(
                &substituted_action_statement,
                &substituted_action_binding,
                &limits,
                &authorized_proof,
            ),
            Err(super::super::PqMaspProofErrorV1::ConsensusBinding(
                iroha_data_model::privacy::PrivacyNativeConsensusBindingValidationErrorV1::InvalidContext(
                    iroha_data_model::privacy::PrivacyStatementValidationError::ActionIndexOutOfBounds {
                        index: substituted_action_statement.context.action_index,
                        max_actions: limits.max_actions_per_transaction,
                    },
                ),
            )),
            "the only alternate first-release action index escaped its hard ceiling"
        );

        let mut other_genesis = binding.clone();
        other_genesis.genesis_hash[0] ^= 1;
        assert!(
            other_genesis.genesis_hash.iter().any(|byte| *byte != 0),
            "adversarial genesis must remain intrinsically valid"
        );
        assert_eq!(
            super::super::verify_pq_masp_v1(&statement, &other_genesis, &limits, &authorized_proof,),
            Err(super::super::PqMaspProofErrorV1::Authorization(
                PqMaspWireErrorV1::AuthorizationFailed,
            )),
            "the unchanged complete proof was replayed under another nonzero genesis"
        );

        let statement_digest =
            iroha_data_model::privacy::PrivacyStatementV1::PqMaspStarkV0(statement.clone())
                .digest()
                .expect("canonical PQ-MASP statement digest");
        let other_genesis_digest = other_genesis
            .digest()
            .expect("canonical changed-genesis binding digest");
        let resigned_outer = authorize_pq_masp_stark_proof_v1(
            statement_digest,
            other_genesis_digest,
            key_digest,
            authorization_keys.secret_key(),
            &proof,
            HedgedRngSeed::from_entropy([0xDA; 32]),
        )
        .expect("fresh changed-genesis outer authorization");
        let resigned_authorization = verify_pq_masp_authorization_v1(
            statement_digest,
            other_genesis_digest,
            key_digest,
            &resigned_outer,
        )
        .expect("freshly re-signed outer proof is independently valid");
        assert_eq!(resigned_authorization.stark_proof, proof.as_slice());
        assert_eq!(
            super::super::verify_pq_masp_v1(&statement, &other_genesis, &limits, &resigned_outer,),
            Err(super::super::PqMaspProofErrorV1::InvalidProof),
            "fresh outer authorization rescued an inner proof from another genesis"
        );
    }

    #[test]
    fn every_native_row_satisfies_copy_and_profile_residues() {
        let (statement, witness) = valid_fixture();
        let (_trace, base, aux, fixed, copy_challenges) =
            prepared_columns(&statement, &witness, b"pq-masp-residue-test-v1");
        for index in 0..PQ_MASP_TRACE_SIZE_V1 {
            let next = (index + 1) % PQ_MASP_TRACE_SIZE_V1;
            let copy_residues = note_copy_constraint_residues_v1(
                &row(&base, index),
                &row(&aux, index),
                &row(&aux, next),
                &row(&fixed, index),
                copy_challenges,
            )
            .expect("copy residue shape");
            assert!(
                copy_residues.iter().all(|residue| *residue == F::ZERO),
                "row {index} has nonzero copy residue at {:?}",
                copy_residues.iter().position(|residue| *residue != F::ZERO)
            );
            let residues = pq_masp_profile_constraint_residues_inner_v1(
                &row(&base, index),
                &row(&base, next),
                &row(&aux, index),
                &row(&aux, next),
                &row(&fixed, index),
            )
            .expect("profile residue shape");
            assert_eq!(residues.len(), PQ_MASP_PROFILE_CONSTRAINT_COUNT_V1);
            assert!(
                residues.iter().all(|residue| *residue == F::ZERO),
                "row {index} has nonzero profile residue at {:?}",
                residues.iter().position(|residue| *residue != F::ZERO)
            );
        }
    }

    #[test]
    fn hostile_prover_cannot_substitute_public_nullifier_after_rebuilding_aux() {
        let (statement, witness) = valid_fixture();
        let trace = super::super::air::build_pq_masp_base_trace_v1(&statement, &witness)
            .expect("honest trace");
        let base = pq_masp_base_columns_v1(&trace).expect("honest base columns");
        let honest_nullifier = statement.nullifiers[0];
        let public_row = trace
            .fixed
            .rows
            .iter()
            .enumerate()
            .find_map(|(row, fixed)| match fixed {
                PqMaspFixedRowV1::ShaEnd {
                    digest_chunk: 0,
                    public_digest: Some(digest),
                    ..
                } if digest == honest_nullifier.as_bytes() => Some(row),
                _ => None,
            })
            .expect("public nullifier endpoint");

        let mut target = statement.clone();
        let mut substituted = *honest_nullifier.as_bytes();
        substituted[0] ^= 1;
        target.nullifiers[0] = iroha_data_model::privacy::PrivacyNullifierV1::new(substituted);
        let (binding, limits) = consensus_material(&target);
        let adapter = PqMaspStarkAdapterV1::new(&target, &binding, &limits);
        let copy_schedule = adapter.copy_schedule_v1().expect("target copy schedule");
        let mut fixed = copy_schedule
            .fixed_columns_v1(PQ_MASP_TRACE_SIZE_V1)
            .expect("target copy fixed");
        fixed.extend(pq_masp_profile_fixed_columns_v1(&target).expect("target profile fixed"));
        let mut transcript =
            TransparentTranscriptV1::new(b"pq-masp-hostile-prover-v1", &[5; 32], &[6; 32])
                .expect("transcript");
        let copy_challenges =
            derive_note_copy_challenges_v1(&mut transcript).expect("copy challenges");
        let mut aux =
            build_note_copy_aux_columns_v1(&base, &fixed, copy_challenges, PQ_MASP_TRACE_SIZE_V1)
                .expect("hostile prover rebuilds copy auxiliary columns");
        aux.extend(
            pq_masp_profile_aux_columns_v1(&target, &base)
                .expect("hostile prover rebuilds profile auxiliary columns"),
        );
        let next = (public_row + 1) % PQ_MASP_TRACE_SIZE_V1;
        let copy_residues = note_copy_constraint_residues_v1(
            &row(&base, public_row),
            &row(&aux, public_row),
            &row(&aux, next),
            &row(&fixed, public_row),
            copy_challenges,
        )
        .expect("copy residue shape");
        assert!(
            copy_residues.iter().all(|residue| *residue == F::ZERO),
            "hostile trace remains copy-consistent"
        );
        let profile_residues = pq_masp_profile_constraint_residues_inner_v1(
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
            "only the verifier-fixed public byte binding rejects this rebuilt trace"
        );
    }

    #[test]
    fn hostile_prover_cannot_substitute_fixed_or_inactive_copy_bytes_after_rebuilding_aux() {
        let (statement, witness) = valid_fixture();
        let trace = super::super::air::build_pq_masp_base_trace_v1(&statement, &witness)
            .expect("honest trace");
        let honest_base = pq_masp_base_columns_v1(&trace).expect("honest base columns");
        let (binding, limits) = consensus_material(&statement);
        let adapter = PqMaspStarkAdapterV1::new(&statement, &binding, &limits);
        let schedule = adapter.copy_schedule_v1().expect("copy schedule");
        let mut fixed = schedule
            .fixed_columns_v1(PQ_MASP_TRACE_SIZE_V1)
            .expect("copy fixed");
        fixed.extend(pq_masp_profile_fixed_columns_v1(&statement).expect("profile fixed"));
        let mut transcript =
            TransparentTranscriptV1::new(b"pq-masp-fixed-copy-hostile-v1", &[7; 32], &[8; 32])
                .expect("transcript");
        let challenges = derive_note_copy_challenges_v1(&mut transcript).expect("copy challenges");

        for policy_kind in [0_u8, 1] {
            let (mutated_row, cell, replacement) = schedule
                .policies
                .iter()
                .enumerate()
                .find_map(|(row, policies)| {
                    policies
                        .iter()
                        .copied()
                        .enumerate()
                        .find_map(|(cell, policy)| match (policy_kind, policy) {
                            (0, NoteCopyCellPolicyV1::Constant(value)) => {
                                Some((row, cell, value ^ 1))
                            }
                            (1, NoteCopyCellPolicyV1::Inactive) => Some((row, cell, 1)),
                            _ => None,
                        })
                })
                .expect("requested fixed policy exists");
            let mut base = honest_base.clone();
            base[COPY_OFFSET + cell][mutated_row] = F(u64::from(replacement));
            let mut aux =
                build_note_copy_aux_columns_v1(&base, &fixed, challenges, PQ_MASP_TRACE_SIZE_V1)
                    .expect("hostile prover rebuilds copy auxiliary columns");
            aux.extend(
                pq_masp_profile_aux_columns_v1(&statement, &base)
                    .expect("hostile prover rebuilds profile auxiliary columns"),
            );
            let next = (mutated_row + 1) % PQ_MASP_TRACE_SIZE_V1;
            let residues = note_copy_constraint_residues_v1(
                &row(&base, mutated_row),
                &row(&aux, mutated_row),
                &row(&aux, next),
                &row(&fixed, mutated_row),
                challenges,
            )
            .expect("copy residue shape");
            assert!(
                residues.iter().any(|residue| *residue != F::ZERO),
                "fixed policy {policy_kind} accepted a rebuilt hostile trace"
            );
        }
    }

    fn profile_mutation_is_detected(
        base: &[Vec<F>],
        aux: &[Vec<F>],
        fixed: &[Vec<F>],
        mutated_row: usize,
    ) -> bool {
        let previous = (mutated_row + PQ_MASP_TRACE_SIZE_V1 - 1) % PQ_MASP_TRACE_SIZE_V1;
        [previous, mutated_row].into_iter().any(|index| {
            let next = (index + 1) % PQ_MASP_TRACE_SIZE_V1;
            pq_masp_profile_constraint_residues_inner_v1(
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

    #[test]
    fn adversarial_mutations_are_rejected_across_every_active_constraint_family() {
        let (statement, witness) = valid_fixture();
        let (trace, mut base, mut aux, fixed, copy_challenges) =
            prepared_columns(&statement, &witness, b"pq-masp-mutation-test-v1");
        let row_types = trace.fixed.rows;
        let row_index = |predicate: &dyn Fn(&PqMaspFixedRowV1) -> bool| {
            row_types
                .iter()
                .position(predicate)
                .expect("canonical row family")
        };
        let mutations = [
            (
                row_index(&|row| matches!(row, PqMaspFixedRowV1::ShaRound { round: 16, .. })),
                SHA_SCHEDULE_OFFSET + 16,
                "SHA schedule",
            ),
            (
                row_index(&|row| {
                    matches!(
                        row,
                        PqMaspFixedRowV1::ShaEnd {
                            digest_chunk: 0,
                            ..
                        }
                    )
                }),
                SHA_STATE_OFFSET,
                "SHA endpoint",
            ),
            (
                row_index(&|row| matches!(row, PqMaspFixedRowV1::NodeSelect { .. })),
                COPY_OFFSET + 2,
                "Merkle direction selection",
            ),
            (
                row_index(&|row| matches!(row, PqMaspFixedRowV1::Distinct { .. })),
                SCRATCH_RUNNING_AFTER,
                "distinctness",
            ),
            (
                row_index(&|row| matches!(row, PqMaspFixedRowV1::NonZero { .. })),
                SCRATCH_RUNNING_BEFORE,
                "nonzero component",
            ),
            (
                row_index(&|row| {
                    matches!(
                        row,
                        PqMaspFixedRowV1::Sum {
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
                    matches!(
                        row,
                        PqMaspFixedRowV1::Sum {
                            side: SumSideV1::Conservation,
                            ..
                        }
                    )
                }),
                SCRATCH_RELATION_CARRY_AFTER,
                "value conservation",
            ),
            (
                row_index(&|row| matches!(row, PqMaspFixedRowV1::Padding)),
                SCRATCH_RUNNING_BEFORE,
                "padding zeroization",
            ),
        ];
        for (mutated_row, column, family) in mutations {
            let original = base[column][mutated_row];
            base[column][mutated_row] = original.add(F::ONE);
            assert!(
                profile_mutation_is_detected(&base, &aux, &fixed, mutated_row),
                "{family} mutation was accepted"
            );
            base[column][mutated_row] = original;
        }

        let profile_aux_column = NOTE_COPY_AUX_WIDTH_V1 + PROFILE_AUX_VM_CARRY_BRIDGE;
        let padding = row_index(&|row| matches!(row, PqMaspFixedRowV1::Padding));
        aux[profile_aux_column][padding] = F::ONE;
        assert!(
            profile_mutation_is_detected(&base, &aux, &fixed, padding),
            "reserved profile auxiliary mutation was accepted"
        );
        aux[profile_aux_column][padding] = F::ZERO;

        let copy_row = row_index(&|row| matches!(row, PqMaspFixedRowV1::NodeSelect { .. }));
        base[COPY_OFFSET][copy_row] = base[COPY_OFFSET][copy_row].add(F::ONE);
        let next = (copy_row + 1) % PQ_MASP_TRACE_SIZE_V1;
        let copy_residues = note_copy_constraint_residues_v1(
            &row(&base, copy_row),
            &row(&aux, copy_row),
            &row(&aux, next),
            &row(&fixed, copy_row),
            copy_challenges,
        )
        .expect("copy residue shape");
        assert!(
            copy_residues.iter().any(|residue| *residue != F::ZERO),
            "copy-permutation mutation was accepted"
        );
    }

    #[test]
    fn maximum_two_by_two_topology_fits_and_satisfies_the_full_domain() {
        let (statement, witness) = valid_two_by_two_fixture();
        let (trace, base, aux, fixed, _copy_challenges) =
            prepared_columns(&statement, &witness, b"pq-masp-max-topology-test-v1");
        assert_eq!(trace.rows.len(), PQ_MASP_TRACE_SIZE_V1);
        let active_rows = trace
            .fixed
            .rows
            .iter()
            .take_while(|row| !matches!(row, PqMaspFixedRowV1::Padding))
            .count();
        assert!(active_rows < PQ_MASP_TRACE_SIZE_V1);
        for index in 0..PQ_MASP_TRACE_SIZE_V1 {
            let next = (index + 1) % PQ_MASP_TRACE_SIZE_V1;
            let residues = pq_masp_profile_constraint_residues_inner_v1(
                &row(&base, index),
                &row(&base, next),
                &row(&aux, index),
                &row(&aux, next),
                &row(&fixed, index),
            )
            .expect("profile residue shape");
            assert!(
                residues.iter().all(|residue| *residue == F::ZERO),
                "maximum topology row {index} has nonzero residue at {:?}",
                residues.iter().position(|residue| *residue != F::ZERO)
            );
        }
    }
}
