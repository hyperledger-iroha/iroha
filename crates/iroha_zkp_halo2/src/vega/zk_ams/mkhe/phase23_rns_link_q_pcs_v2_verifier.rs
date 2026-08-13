//! Borrowed, fixed-scratch verifier for the ten-row qPCS V2 envelope.
//!
//! Merkle leaves and nodes bind the parameter digest, tree role, FRI layer,
//! tree length, and height. Multiproof values stay borrowed from the bounded
//! proof; the largest live path frontier is exactly 320 nodes. Successful
//! verification authenticates C0, Cq, and all 18 FRI layers before checking
//! every one-point quotient, ten-row batch, FRI fold, and terminal equation.
use super::super::{Fq2ParametersV1, Fq2V1};
use super::*;
const MERKLE_LEAF_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-leaf\0";
const MERKLE_NODE_DOMAIN_V2: &[u8] = b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-node\0";
const MAX_FRONTIER_NODES_V2: usize = 2 * QUERY_COUNT_V2;
const C0_SECTION_V2: usize = 0;
const CQ_SECTION_V2: usize = 1;
const FRI_SECTION_START_V2: usize = 2;
const TERMINAL_OFFSET_V2: usize =
    HEADER_BYTES_V2 + EVALUATION_BYTES_V2 + QUOTIENT_ROOT_BYTES_V2 + FRI_ROOT_BYTES_V2;
#[derive(Clone, Copy)]
#[repr(u8)]
enum TreeKindV2 {
    Initial = 1,
    OpeningQuotient = 2,
    Fri = 3,
}
#[derive(Clone, Copy)]
struct BorrowedSectionV2<'a> {
    values: &'a [u8],
    authentication: &'a [u8],
}
#[derive(Clone, Copy)]
struct FrontierNodeV2 {
    index: u32,
    digest: [u8; 32],
}
const EMPTY_FRONTIER_NODE_V2: FrontierNodeV2 = FrontierNodeV2 {
    index: 0,
    digest: [0; 32],
};
struct AuthenticatedEquationsV2<'a> {
    live: Option<LiveProtocolV2<'a>>,
}
fn merkle_leaf_hash_v2(
    parameter_digest: [u8; 32],
    kind: TreeKindV2,
    layer: usize,
    length: usize,
    values: &[u8],
) -> Result<[u8; 32], SoundnessErrorV2> {
    if values.len() != LEAF_BYTES_V2 || layer > u8::MAX as usize {
        return Err(SoundnessErrorV2::InvalidMerklePath);
    }
    let mut frame = FrameV2::<6_240>::new();
    frame.push(MERKLE_LEAF_DOMAIN_V2)?;
    frame.push(&[VERSION_V2])?;
    frame.push(&parameter_digest)?;
    frame.push(&[kind as u8, layer as u8])?;
    frame.push(
        &u32::try_from(length)
            .map_err(|_| SoundnessErrorV2::ArithmeticOverflow)?
            .to_be_bytes(),
    )?;
    frame.push(&(COORDINATE_COUNT_V2 as u16).to_be_bytes())?;
    frame.push(values)?;
    Ok(keccak256(frame.bytes()))
}
fn merkle_node_hash_v2(
    parameter_digest: [u8; 32],
    kind: TreeKindV2,
    layer: usize,
    height: usize,
    left: [u8; 32],
    right: [u8; 32],
) -> Result<[u8; 32], SoundnessErrorV2> {
    if layer > u8::MAX as usize || height > u8::MAX as usize {
        return Err(SoundnessErrorV2::InvalidMerklePath);
    }
    let mut frame = FrameV2::<176>::new();
    frame.push(MERKLE_NODE_DOMAIN_V2)?;
    frame.push(&[VERSION_V2])?;
    frame.push(&parameter_digest)?;
    frame.push(&[kind as u8, layer as u8, height as u8])?;
    frame.push(&left)?;
    frame.push(&right)?;
    Ok(keccak256(frame.bytes()))
}
#[cfg(test)]
pub(super) fn initial_leaf_hash_for_prover_parity_v2(
    parameter_digest: [u8; 32],
    length: usize,
    values: &[u8],
) -> Result<[u8; 32], SoundnessErrorV2> {
    merkle_leaf_hash_v2(parameter_digest, TreeKindV2::Initial, 0, length, values)
}
#[cfg(test)]
pub(super) fn initial_node_hash_for_prover_parity_v2(
    parameter_digest: [u8; 32],
    height: usize,
    left: [u8; 32],
    right: [u8; 32],
) -> Result<[u8; 32], SoundnessErrorV2> {
    merkle_node_hash_v2(
        parameter_digest,
        TreeKindV2::Initial,
        0,
        height,
        left,
        right,
    )
}
#[cfg(test)]
pub(super) fn quotient_leaf_hash_for_prover_parity_v2(
    parameter_digest: [u8; 32],
    length: usize,
    values: &[u8],
) -> Result<[u8; 32], SoundnessErrorV2> {
    merkle_leaf_hash_v2(
        parameter_digest,
        TreeKindV2::OpeningQuotient,
        0,
        length,
        values,
    )
}
#[cfg(test)]
pub(super) fn quotient_node_hash_for_prover_parity_v2(
    parameter_digest: [u8; 32],
    height: usize,
    left: [u8; 32],
    right: [u8; 32],
) -> Result<[u8; 32], SoundnessErrorV2> {
    merkle_node_hash_v2(
        parameter_digest,
        TreeKindV2::OpeningQuotient,
        0,
        height,
        left,
        right,
    )
}
fn take_borrowed_section_v2<'a>(
    live: &mut LiveProtocolV2<'a>,
    indices: &IndexSetV2,
    length: usize,
) -> Result<BorrowedSectionV2<'a>, SoundnessErrorV2> {
    let opened = read_u32_v2(live.wire, live.offset)? as usize;
    let authentication = read_u32_v2(live.wire, live.offset + 4)? as usize;
    if opened != indices.len || authentication != exact_authentication_count_v2(indices, length)? {
        return Err(SoundnessErrorV2::InvalidSectionCount);
    }
    live.offset = live
        .offset
        .checked_add(SECTION_HEADER_BYTES_V2)
        .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
    let values_end = live
        .offset
        .checked_add(
            opened
                .checked_mul(LEAF_BYTES_V2)
                .ok_or(SoundnessErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
    let values = live
        .wire
        .get(live.offset..values_end)
        .ok_or(SoundnessErrorV2::Truncated)?;
    validate_leaf_values_v2(values)?;
    let authentication_end = values_end
        .checked_add(
            authentication
                .checked_mul(32)
                .ok_or(SoundnessErrorV2::ArithmeticOverflow)?,
        )
        .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
    let authentication = live
        .wire
        .get(values_end..authentication_end)
        .ok_or(SoundnessErrorV2::Truncated)?;
    live.offset = authentication_end;
    Ok(BorrowedSectionV2 {
        values,
        authentication,
    })
}
fn authenticate_section_v2(
    section: BorrowedSectionV2<'_>,
    indices: &IndexSetV2,
    length: usize,
    expected_root: [u8; 32],
    parameter_digest: [u8; 32],
    kind: TreeKindV2,
    layer: usize,
) -> Result<(), SoundnessErrorV2> {
    if indices.len == 0
        || indices.len > MAX_FRONTIER_NODES_V2
        || !length.is_power_of_two()
        || length < 2
        || section.values.len() != indices.len * LEAF_BYTES_V2
    {
        return Err(SoundnessErrorV2::InvalidMerklePath);
    }
    let mut current = [EMPTY_FRONTIER_NODE_V2; MAX_FRONTIER_NODES_V2];
    let mut next = [EMPTY_FRONTIER_NODE_V2; MAX_FRONTIER_NODES_V2];
    for (position, current_node) in current.iter_mut().take(indices.len).enumerate() {
        let index = indices.values[position];
        if index as usize >= length {
            return Err(SoundnessErrorV2::InvalidMerklePath);
        }
        let start = position * LEAF_BYTES_V2;
        *current_node = FrontierNodeV2 {
            index,
            digest: merkle_leaf_hash_v2(
                parameter_digest,
                kind,
                layer,
                length,
                &section.values[start..start + LEAF_BYTES_V2],
            )?,
        };
    }
    let mut current_len = indices.len;
    let mut nodes_at_height = length;
    let mut height = 1_usize;
    let mut authentication_cursor = 0_usize;
    while nodes_at_height > 1 {
        let mut cursor = 0_usize;
        let mut next_len = 0_usize;
        while cursor < current_len {
            let node = current[cursor];
            let sibling = node.index ^ 1;
            let (left, right);
            if node.index.is_multiple_of(2)
                && cursor + 1 < current_len
                && current[cursor + 1].index == sibling
            {
                left = node.digest;
                right = current[cursor + 1].digest;
                cursor += 2;
            } else {
                let start = authentication_cursor
                    .checked_mul(32)
                    .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
                let digest = read_digest_v2(section.authentication, start)
                    .map_err(|_| SoundnessErrorV2::InvalidMerklePath)?;
                authentication_cursor += 1;
                if node.index.is_multiple_of(2) {
                    left = node.digest;
                    right = digest;
                } else {
                    left = digest;
                    right = node.digest;
                }
                cursor += 1;
            }
            next[next_len] = FrontierNodeV2 {
                index: node.index / 2,
                digest: merkle_node_hash_v2(parameter_digest, kind, layer, height, left, right)?,
            };
            next_len += 1;
        }
        current[..next_len].copy_from_slice(&next[..next_len]);
        current_len = next_len;
        nodes_at_height /= 2;
        height += 1;
    }
    if current_len != 1
        || current[0].index != 0
        || current[0].digest != expected_root
        || authentication_cursor * 32 != section.authentication.len()
    {
        return Err(SoundnessErrorV2::InvalidMerklePath);
    }
    Ok(())
}
fn read_section_value_v2(
    section: BorrowedSectionV2<'_>,
    indices: &IndexSetV2,
    index: u32,
    coordinate: usize,
) -> Result<Fq2V1, SoundnessErrorV2> {
    if coordinate >= COORDINATE_COUNT_V2 {
        return Err(SoundnessErrorV2::InvalidSectionCount);
    }
    let position = indices.values[..indices.len]
        .binary_search(&index)
        .map_err(|_| SoundnessErrorV2::InvalidSectionCount)?;
    let offset = position
        .checked_mul(LEAF_BYTES_V2)
        .and_then(|value| value.checked_add(coordinate * FQ2_BYTES_V2))
        .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
    Ok(Fq2V1 {
        c0: read_u64_v2(section.values, offset)?,
        c1: read_u64_v2(section.values, offset + 8)?,
    })
}
fn read_terminal_value_v2(
    wire: &[u8],
    leaf: usize,
    coordinate: usize,
) -> Result<Fq2V1, SoundnessErrorV2> {
    if leaf >= 2 || coordinate >= COORDINATE_COUNT_V2 {
        return Err(SoundnessErrorV2::InvalidTerminal);
    }
    let offset = TERMINAL_OFFSET_V2
        .checked_add(leaf * LEAF_BYTES_V2)
        .and_then(|value| value.checked_add(coordinate * FQ2_BYTES_V2))
        .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
    Ok(Fq2V1 {
        c0: read_u64_v2(wire, offset)?,
        c1: read_u64_v2(wire, offset + 8)?,
    })
}
fn fq2_v1(value: Fq2V2) -> Fq2V1 {
    Fq2V1 {
        c0: value.c0,
        c1: value.c1,
    }
}
fn field_parameters_v2() -> Result<[Fq2ParametersV1; LIMBS_V2], SoundnessErrorV2> {
    let first = Fq2ParametersV1::derive(RELEASE_MODULI_V1[0], DOMAIN_LOG_V2 as usize)
        .map_err(|_| SoundnessErrorV2::InvalidChallenge)?;
    let mut parameters = [first; LIMBS_V2];
    for limb in 1..LIMBS_V2 {
        parameters[limb] = Fq2ParametersV1::derive(RELEASE_MODULI_V1[limb], DOMAIN_LOG_V2 as usize)
            .map_err(|_| SoundnessErrorV2::InvalidChallenge)?;
    }
    Ok(parameters)
}
fn post_quotient_transcript_v2(live: &LiveProtocolV2<'_>) -> Result<[u8; 32], SoundnessErrorV2> {
    let evaluations = live
        .wire
        .get(HEADER_BYTES_V2..HEADER_BYTES_V2 + EVALUATION_BYTES_V2)
        .ok_or(SoundnessErrorV2::Truncated)?;
    let transcript = absorb_evaluations_v2(initial_transcript_v2(live.header)?, evaluations)?;
    let root = read_digest_v2(live.wire, HEADER_BYTES_V2 + EVALUATION_BYTES_V2)?;
    absorb_root_v2(QUOTIENT_DOMAIN_V2, transcript, 0, root)
}
fn opening_equation_holds_v2(
    field: Fq2ParametersV1,
    x: Fq2V1,
    point: u64,
    evaluation: u64,
    committed: Fq2V1,
    quotient: Fq2V1,
) -> bool {
    field.sub(committed, Fq2V1::base(evaluation))
        == field.mul(field.sub(x, Fq2V1::base(point)), quotient)
}
fn batch_value_v2(
    field: Fq2ParametersV1,
    x: Fq2V1,
    committed: Fq2V1,
    quotient: Fq2V1,
    a: Fq2V1,
    b: Fq2V1,
    row: usize,
) -> Fq2V1 {
    let (committed_power, quotient_power) = if row.is_multiple_of(2) {
        (Fq2V1::ONE, x)
    } else {
        let x_to_n = field.pow(x, N_V2 as u128);
        (x_to_n, field.mul(x_to_n, x))
    };
    field.add(
        field.mul(a, field.mul(committed_power, committed)),
        field.mul(b, field.mul(quotient_power, quotient)),
    )
}
fn fold_value_v2(
    field: Fq2ParametersV1,
    x: Fq2V1,
    positive: Fq2V1,
    negative: Fq2V1,
    alpha: Fq2V1,
) -> Result<Fq2V1, SoundnessErrorV2> {
    let inverse_two = mod_pow_v1(2, field.modulus - 2, field.modulus);
    let inverse_two_x = field.scale(
        field
            .inverse(x)
            .map_err(|_| SoundnessErrorV2::InvalidFriEquation)?,
        inverse_two,
    );
    let even = field.scale(field.add(positive, negative), inverse_two);
    let odd = field.mul(field.sub(positive, negative), inverse_two_x);
    Ok(field.add(even, field.mul(alpha, odd)))
}
fn verify_initial_equations_v2(
    live: &LiveProtocolV2<'_>,
    sections: &[BorrowedSectionV2<'_>; SECTION_COUNT_V2],
    initial_indices: &IndexSetV2,
    parameters: &[Fq2ParametersV1; LIMBS_V2],
    batch_transcript: [u8; 32],
) -> Result<(), SoundnessErrorV2> {
    if derive_batch_schedule_v2(batch_transcript)? != live.batch_schedule_digest {
        return Err(SoundnessErrorV2::InvalidChallenge);
    }
    for (limb, &field) in parameters.iter().enumerate() {
        let mut batch_coefficients = [[Fq2V1::ZERO; 2]; ROWS_PER_LIMB_V2];
        for (row, coefficients) in batch_coefficients.iter_mut().enumerate() {
            coefficients[0] = fq2_v1(derive_fq2_challenge_v2(
                BATCH_DOMAIN_V2,
                batch_transcript,
                limb,
                row,
                0,
                0,
            )?);
            coefficients[1] = fq2_v1(derive_fq2_challenge_v2(
                BATCH_DOMAIN_V2,
                batch_transcript,
                limb,
                row,
                1,
                0,
            )?);
        }
        for &index in &initial_indices.values[..initial_indices.len] {
            let x = field.pow(field.domain_root, index as u128);
            for (row, coefficients) in batch_coefficients.iter().enumerate() {
                let repetition = row / ROWS_PER_REPETITION_V2;
                let role = row % ROWS_PER_REPETITION_V2;
                let relation = limb * REPETITIONS_V2 + repetition;
                let evaluation_offset = HEADER_BYTES_V2 + relation * 16 + role * 8;
                let evaluation = read_u64_v2(live.wire, evaluation_offset)?;
                let committed = read_section_value_v2(
                    sections[C0_SECTION_V2],
                    initial_indices,
                    index,
                    limb * ROWS_PER_LIMB_V2 + row,
                )?;
                let quotient = read_section_value_v2(
                    sections[CQ_SECTION_V2],
                    initial_indices,
                    index,
                    limb * ROWS_PER_LIMB_V2 + row,
                )?;
                if !opening_equation_holds_v2(
                    field,
                    x,
                    live.relation_points[relation],
                    evaluation,
                    committed,
                    quotient,
                ) {
                    return Err(SoundnessErrorV2::InvalidOpeningQuotient);
                }
                let batch = read_section_value_v2(
                    sections[FRI_SECTION_START_V2],
                    initial_indices,
                    index,
                    limb * ROWS_PER_LIMB_V2 + row,
                )?;
                if batch
                    != batch_value_v2(
                        field,
                        x,
                        committed,
                        quotient,
                        coefficients[0],
                        coefficients[1],
                        row,
                    )
                {
                    return Err(SoundnessErrorV2::InvalidBatchEquation);
                }
            }
        }
    }
    Ok(())
}
fn verify_fri_equations_v2(
    live: &LiveProtocolV2<'_>,
    sections: &[BorrowedSectionV2<'_>; SECTION_COUNT_V2],
    parameters: &[Fq2ParametersV1; LIMBS_V2],
    mut transcript: [u8; 32],
) -> Result<(), SoundnessErrorV2> {
    let mut schedule = live.batch_schedule_digest;
    let mut queries = live.queries;
    let mut length = DOMAIN_SIZE_V2;
    for layer in 0..FRI_ROUNDS_V2 {
        let root_offset =
            HEADER_BYTES_V2 + EVALUATION_BYTES_V2 + QUOTIENT_ROOT_BYTES_V2 + layer * 32;
        let root = read_digest_v2(live.wire, root_offset)?;
        transcript = absorb_root_v2(FRI_ROOT_DOMAIN_V2, transcript, layer as u8, root)?;
        let indices = query_pair_indices_v2(&queries, length);
        let half = length / 2;
        let mut next_queries = queries;
        for query in &mut next_queries {
            *query %= half as u32;
        }
        let next_indices = if length == 4 {
            None
        } else {
            Some(query_pair_indices_v2(&next_queries, half))
        };
        for (limb, &field) in parameters.iter().enumerate() {
            let layer_root = field.pow(
                field.domain_root,
                1_u128 << (u32::from(field.domain_log) - length.ilog2()),
            );
            let mut alphas = [Fq2V1::ZERO; ROWS_PER_LIMB_V2];
            for (row, alpha) in alphas.iter_mut().enumerate() {
                *alpha = fq2_v1(derive_fq2_challenge_v2(
                    FOLD_DOMAIN_V2,
                    transcript,
                    limb,
                    row,
                    0,
                    layer,
                )?);
                schedule = absorb_schedule_value_v2(
                    schedule,
                    1,
                    limb,
                    row,
                    0,
                    layer,
                    Fq2V2 {
                        c0: alpha.c0,
                        c1: alpha.c1,
                    },
                )?;
            }
            for &query in &queries {
                let base = query % half as u32;
                let x = field.pow(layer_root, base as u128);
                for (row, &alpha) in alphas.iter().enumerate() {
                    let coordinate = limb * ROWS_PER_LIMB_V2 + row;
                    let positive = read_section_value_v2(
                        sections[FRI_SECTION_START_V2 + layer],
                        &indices,
                        base,
                        coordinate,
                    )?;
                    let negative = read_section_value_v2(
                        sections[FRI_SECTION_START_V2 + layer],
                        &indices,
                        base + half as u32,
                        coordinate,
                    )?;
                    let next = if length == 4 {
                        read_terminal_value_v2(live.wire, base as usize, coordinate)?
                    } else {
                        read_section_value_v2(
                            sections[FRI_SECTION_START_V2 + layer + 1],
                            next_indices
                                .as_ref()
                                .ok_or(SoundnessErrorV2::InvalidFriEquation)?,
                            base,
                            coordinate,
                        )?
                    };
                    if next != fold_value_v2(field, x, positive, negative, alpha)? {
                        return Err(SoundnessErrorV2::InvalidFriEquation);
                    }
                }
            }
        }
        queries = next_queries;
        length = half;
    }
    if length != 2 || schedule != live.fold_schedule_digest {
        return Err(SoundnessErrorV2::InvalidFriEquation);
    }
    Ok(())
}
impl<'a> FriTranscriptBoundV2<'a> {
    fn verify_authenticated_equations_v2(
        &mut self,
    ) -> Result<AuthenticatedEquationsV2<'a>, SoundnessErrorV2> {
        let mut live = self.live.take().ok_or(SoundnessErrorV2::Poisoned)?;
        if live.offset != FIXED_BEFORE_SECTIONS_V2 {
            return Err(SoundnessErrorV2::InvalidSectionCount);
        }
        let empty = BorrowedSectionV2 {
            values: &[],
            authentication: &[],
        };
        let mut sections = [empty; SECTION_COUNT_V2];
        let initial_indices = query_pair_indices_v2(&live.queries, DOMAIN_SIZE_V2);
        sections[C0_SECTION_V2] =
            take_borrowed_section_v2(&mut live, &initial_indices, DOMAIN_SIZE_V2)?;
        if sections[C0_SECTION_V2].authentication.len() / 32 > MAX_INITIAL_AUTH_HASHES_PER_TREE_V2 {
            return Err(SoundnessErrorV2::InvalidSectionCount);
        }
        authenticate_section_v2(
            sections[C0_SECTION_V2],
            &initial_indices,
            DOMAIN_SIZE_V2,
            live.header.initial_root,
            live.header.parameter_digest,
            TreeKindV2::Initial,
            0,
        )?;
        sections[CQ_SECTION_V2] =
            take_borrowed_section_v2(&mut live, &initial_indices, DOMAIN_SIZE_V2)?;
        if sections[CQ_SECTION_V2].authentication.len() / 32 > MAX_INITIAL_AUTH_HASHES_PER_TREE_V2 {
            return Err(SoundnessErrorV2::InvalidSectionCount);
        }
        authenticate_section_v2(
            sections[CQ_SECTION_V2],
            &initial_indices,
            DOMAIN_SIZE_V2,
            read_digest_v2(live.wire, HEADER_BYTES_V2 + EVALUATION_BYTES_V2)?,
            live.header.parameter_digest,
            TreeKindV2::OpeningQuotient,
            0,
        )?;
        let mut queries = live.queries;
        let mut length = DOMAIN_SIZE_V2;
        let mut fri_opened = 0_usize;
        let mut fri_authentication = 0_usize;
        for layer in 0..FRI_ROUNDS_V2 {
            let indices = query_pair_indices_v2(&queries, length);
            let section = take_borrowed_section_v2(&mut live, &indices, length)?;
            fri_opened = fri_opened
                .checked_add(indices.len)
                .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
            fri_authentication = fri_authentication
                .checked_add(section.authentication.len() / 32)
                .ok_or(SoundnessErrorV2::ArithmeticOverflow)?;
            let root_offset =
                HEADER_BYTES_V2 + EVALUATION_BYTES_V2 + QUOTIENT_ROOT_BYTES_V2 + layer * 32;
            authenticate_section_v2(
                section,
                &indices,
                length,
                read_digest_v2(live.wire, root_offset)?,
                live.header.parameter_digest,
                TreeKindV2::Fri,
                layer,
            )?;
            sections[FRI_SECTION_START_V2 + layer] = section;
            let half = length / 2;
            for query in &mut queries {
                *query %= half as u32;
            }
            length = half;
        }
        checked_fri_multiproof_bytes_v2(fri_opened, fri_authentication)?;
        if length != 2 || live.offset != live.wire.len() {
            return Err(if live.offset == live.wire.len() {
                SoundnessErrorV2::InvalidSectionCount
            } else {
                SoundnessErrorV2::TrailingBytes
            });
        }
        let parameters = field_parameters_v2()?;
        let transcript = post_quotient_transcript_v2(&live)?;
        verify_initial_equations_v2(&live, &sections, &initial_indices, &parameters, transcript)?;
        verify_fri_equations_v2(&live, &sections, &parameters, transcript)?;
        Ok(AuthenticatedEquationsV2 { live: Some(live) })
    }
}
#[cfg(test)]
#[path = "phase23_rns_link_q_pcs_v2_verifier_tests.rs"]
mod tests;
