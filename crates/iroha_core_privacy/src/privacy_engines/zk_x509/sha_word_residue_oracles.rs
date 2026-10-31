//! Preserved pre-streaming polynomial bodies; test-only independent collection.

use super::*;

/// Evaluate the preserved pre-streaming equation body.
#[allow(clippy::too_many_lines)]
pub(crate) fn original_evaluate_zk_x509_sha_word_stark_residues_over_field_v1<
    A: PolynomialAirFieldV1,
>(
    current: &[A; SHA_WORD_BASE_WIDTH_V1],
    next: &[A; SHA_WORD_BASE_WIDTH_V1],
    current_aux: &[A; SHA_WORD_AUX_WIDTH_V1],
    next_aux: &[A; SHA_WORD_AUX_WIDTH_V1],
    fixed: &[A; SHA_WORD_STARK_FIXED_WIDTH_V1],
    challenges: ZkX509ShaWordStarkChallengesV1,
) -> Result<Vec<A>, ZkX509ShaWordStarkErrorV1> {
    validate_zk_x509_sha_word_stark_challenges_v1(challenges)?;
    if current
        .iter()
        .chain(next)
        .chain(current_aux)
        .chain(next_aux)
        .chain(fixed)
        .any(|value| !value.is_canonical())
    {
        return Err(ZkX509ShaWordStarkErrorV1::Topology);
    }
    let word = fixed[FIX_WORD];
    let sigma_small_zero = fixed[FIX_SIGMA_SMALL_ZERO];
    let sigma_small_one = fixed[FIX_SIGMA_SMALL_ONE];
    let sigma_big_zero = fixed[FIX_SIGMA_BIG_ZERO];
    let sigma_big_one = fixed[FIX_SIGMA_BIG_ONE];
    let choose = fixed[FIX_CHOOSE];
    let majority = fixed[FIX_MAJORITY];
    let digest = fixed[FIX_DIGEST];
    let memory = fixed[FIX_MEMORY];
    let padding = fixed[FIX_PADDING];
    let sigma_any = fixed_sum_v1(
        fixed,
        [
            FIX_SIGMA_SMALL_ZERO,
            FIX_SIGMA_SMALL_ONE,
            FIX_SIGMA_BIG_ZERO,
            FIX_SIGMA_BIG_ONE,
        ],
    );
    let boolean_any = choose.add(majority);
    let add_any = fixed[FIX_ADD_ARITY_TWO].add(fixed[FIX_ADD_ARITY_FOUR]);
    let word_or_digest = word.add(digest);
    let local = word_or_digest.add(sigma_any).add(boolean_any).add(add_any);
    let mut folded_base = [A::ZERO; SHA_WORD_COPY_LANES_V1];
    let mut base_error_count = 0_usize;
    let mut push_base_error = |error: A| {
        for (folded, challenge) in folded_base.iter_mut().zip(challenges.base_folding) {
            *folded = folded.mul_base(challenge).add(error);
        }
        base_error_count += 1;
    };
    for column in 33..SHA_WORD_BASE_WIDTH_V1 {
        push_base_error(word_or_digest.mul(current[column]));
    }
    for column in 40..SHA_WORD_BASE_WIDTH_V1 {
        push_base_error(boolean_any.mul(current[column]));
    }
    for column in 41..SHA_WORD_BASE_WIDTH_V1 {
        push_base_error(add_any.mul(current[column]));
    }
    for column in 6..SHA_WORD_BASE_WIDTH_V1 {
        push_base_error(memory.mul(current[column]));
    }
    for value in current {
        push_base_error(padding.mul(*value));
    }
    let mut word_value = A::ZERO;
    for bit in 0..32 {
        let value = current[1 + bit];
        push_base_error(word_or_digest.mul(value.mul(value.sub(A::ONE))));
        word_value = word_value.add(value.mul(A::from_base(F(1_u64 << bit))));
    }
    push_base_error(word_or_digest.mul(current[0].sub(word_value)));
    for byte in 0..4 {
        let byte_value = pack_bits(&current[1 + byte * 8..1 + (byte + 1) * 8]);
        push_base_error(
            fixed[FIX_WORD_BYTE_MASK + byte]
                .mul(byte_value.sub(fixed[FIX_WORD_BYTE_EXPECTED + byte])),
        );
    }
    push_base_error(digest.mul(current[0].sub(fixed[FIX_DIGEST_EXPECTED])));
    for value in current {
        push_base_error(sigma_any.mul(value.mul(value.sub(A::ONE))));
    }
    for bit in 0..32 {
        let mut error = sigma_any.mul(current[32 + bit]);
        for (selector, rotate_first, rotate_second, third) in [
            (sigma_small_zero, 7_usize, 18_usize, SigmaThirdV1::Shift(3)),
            (sigma_small_one, 17_usize, 19_usize, SigmaThirdV1::Shift(10)),
            (sigma_big_zero, 2_usize, 13_usize, SigmaThirdV1::Rotate(22)),
            (sigma_big_one, 6_usize, 11_usize, SigmaThirdV1::Rotate(25)),
        ] {
            error = error.sub(selector.mul(sigma_expected_bit_v1(
                &current[..32],
                bit,
                rotate_first,
                rotate_second,
                third,
            )));
        }
        push_base_error(error);
    }
    for bit in &current[4..36] {
        push_base_error(boolean_any.mul(bit.mul(bit.sub(A::ONE))));
    }
    for within in 0..8 {
        let x = current[4 + within];
        let y = current[12 + within];
        let z = current[20 + within];
        let output = current[28 + within];
        let choose_value = x.mul(y).add(A::ONE.sub(x).mul(z));
        let xy = x.mul(y);
        let xz = x.mul(z);
        let yz = y.mul(z);
        let majority_value = xy.add(xz).add(yz).sub(A::from_base(F(2)).mul(xy.mul(z)));
        push_base_error(
            boolean_any
                .mul(output)
                .sub(choose.mul(choose_value))
                .sub(majority.mul(majority_value)),
        );
    }
    let boolean_first = boolean_any.mul(fixed[FIX_BOOLEAN_FIRST]);
    let boolean_continue = boolean_any.mul(fixed[FIX_BOOLEAN_CONTINUE]);
    let boolean_last = boolean_any.mul(fixed[FIX_BOOLEAN_LAST]);
    for operand in 0..4 {
        push_base_error(boolean_continue.mul(next[operand].sub(current[operand])));
        let contribution = current[4 + operand * 8..4 + (operand + 1) * 8]
            .iter()
            .copied()
            .enumerate()
            .fold(A::ZERO, |sum, (within, bit)| {
                sum.add(bit.mul(A::from_base(F(1_u64 << within))))
            })
            .mul(fixed[FIX_BOOLEAN_SCALE]);
        push_base_error(boolean_first.mul(current[36 + operand].sub(contribution)));
        let next_contribution = next[4 + operand * 8..4 + (operand + 1) * 8]
            .iter()
            .copied()
            .enumerate()
            .fold(A::ZERO, |sum, (within, bit)| {
                sum.add(bit.mul(A::from_base(F(1_u64 << within))))
            })
            .mul(fixed[FIX_BOOLEAN_NEXT_SCALE]);
        push_base_error(
            boolean_continue
                .mul(next[36 + operand].sub(current[36 + operand].add(next_contribution))),
        );
        push_base_error(boolean_last.mul(current[36 + operand].sub(current[operand])));
    }
    for bit in &current[6..9] {
        push_base_error(add_any.mul(bit.mul(bit.sub(A::ONE))));
    }
    let mut add_output = A::ZERO;
    for bit in 0..32 {
        let value = current[9 + bit];
        push_base_error(add_any.mul(value.mul(value.sub(A::ONE))));
        add_output = add_output.add(value.mul(A::from_base(F(1_u64 << bit))));
    }
    push_base_error(add_any.mul(current[5].sub(add_output)));
    push_base_error(fixed[FIX_ADD_ARITY_TWO].mul(current[2]));
    push_base_error(fixed[FIX_ADD_ARITY_TWO].mul(current[3]));
    push_base_error(add_any.mul(current[4]));
    let carry = current[6]
        .add(A::from_base(F(2)).mul(current[7]))
        .add(A::from_base(F(4)).mul(current[8]));
    let add_sum = current[..5]
        .iter()
        .copied()
        .fold(A::ZERO, A::add)
        .add(fixed[FIX_ADD_CONSTANT]);
    push_base_error(
        add_any.mul(
            add_sum
                .sub(current[5])
                .sub(A::from_base(F(1_u64 << 32)).mul(carry)),
        ),
    );
    push_base_error(memory.mul(current[2].mul(current[2].sub(A::ONE))));
    push_base_error(memory.mul(current[5].mul(current[5].sub(A::ONE))));
    push_base_error(memory.mul(current[0].sub(fixed[FIX_MEMORY_EXECUTION_ADDRESS])));
    push_base_error(memory.mul(current[2].sub(fixed[FIX_MEMORY_EXECUTION_WRITE])));
    push_base_error(memory.mul(current[3].sub(fixed[FIX_MEMORY_SORTED_ADDRESS])));
    push_base_error(memory.mul(current[5].sub(fixed[FIX_MEMORY_SORTED_WRITE])));
    let memory_same = fixed[FIX_MEMORY_SAME_NEXT];
    push_base_error(memory_same.mul(next[3].sub(current[3])));
    push_base_error(memory_same.mul(next[4].sub(current[4])));
    push_base_error(memory_same.mul(next[5]));
    let memory_new = fixed[FIX_MEMORY_NEW_NEXT];
    push_base_error(memory_new.mul(next[3].sub(current[3].add(A::ONE))));
    push_base_error(memory_new.mul(next[5].sub(A::ONE)));
    if base_error_count != SHA_WORD_STARK_BASE_ERROR_COUNT_V1 {
        return Err(ZkX509ShaWordStarkErrorV1::Topology);
    }
    let mut residues = Vec::with_capacity(SHA_WORD_STARK_CONSTRAINT_COUNT_V1);
    residues.extend(folded_base);
    for lane in 0..SHA_WORD_COPY_LANES_V1 {
        let challenge = challenges.memory.lanes[lane];
        let packed_input = pack_bits(&current[..32]);
        let packed_output = pack_bits(&current[32..64]);
        let pair_descriptors = [
            (LOCAL_PAIR_01, [0_usize, 1_usize]),
            (LOCAL_PAIR_23, [2_usize, 3_usize]),
            (LOCAL_PAIR_45, [4_usize, 5_usize]),
        ];
        for (pair_offset, pair_slots) in pair_descriptors {
            let pair = current_aux[pair_offset + lane];
            let mut error = word.mul(local_pair_error_v1(
                pair,
                pair_slots,
                fixed,
                [current[0], A::ZERO, A::ZERO, A::ZERO, A::ZERO, A::ZERO],
                [true, false, false, false, false, false],
                1,
                challenge,
            ));
            error = error.add(digest.mul(local_pair_error_v1(
                pair,
                pair_slots,
                fixed,
                [current[0], A::ZERO, A::ZERO, A::ZERO, A::ZERO, A::ZERO],
                [false; 6],
                1,
                challenge,
            )));
            error = error.add(sigma_any.mul(local_pair_error_v1(
                pair,
                pair_slots,
                fixed,
                [
                    packed_input,
                    packed_output,
                    A::ZERO,
                    A::ZERO,
                    A::ZERO,
                    A::ZERO,
                ],
                [false, true, false, false, false, false],
                2,
                challenge,
            )));
            error = error.add(boolean_first.mul(local_pair_error_v1(
                pair,
                pair_slots,
                fixed,
                [
                    current[0],
                    current[1],
                    current[2],
                    current[3],
                    A::ZERO,
                    A::ZERO,
                ],
                [false, false, false, true, false, false],
                4,
                challenge,
            )));
            for (selector, arity) in [(FIX_ADD_ARITY_TWO, 2_usize), (FIX_ADD_ARITY_FOUR, 4)] {
                let mut values = [A::ZERO; 6];
                let mut writes = [false; 6];
                values[..arity].copy_from_slice(&current[..arity]);
                values[arity] = current[5];
                writes[arity] = true;
                error = error.add(fixed[selector].mul(local_pair_error_v1(
                    pair,
                    pair_slots,
                    fixed,
                    values,
                    writes,
                    arity + 1,
                    challenge,
                )));
            }
            let no_events = boolean_any.mul(A::ONE.sub(fixed[FIX_BOOLEAN_FIRST]));
            error = error.add(no_events.mul(pair.sub(A::ONE)));
            residues.push(error);
        }
        residues.push(
            local.mul(
                current_aux[LOCAL_QUAD + lane]
                    .sub(current_aux[LOCAL_PAIR_01 + lane].mul(current_aux[LOCAL_PAIR_23 + lane])),
            ),
        );
        residues.push(
            local.mul(
                current_aux[LOCAL_PRODUCT_AFTER + lane].sub(
                    current_aux[LOCAL_PRODUCT_BEFORE + lane]
                        .mul(current_aux[LOCAL_QUAD + lane])
                        .mul(current_aux[LOCAL_PAIR_45 + lane]),
                ),
            ),
        );
        residues
            .push(fixed[FIX_LOCAL_FIRST].mul(current_aux[LOCAL_PRODUCT_BEFORE + lane].sub(A::ONE)));
        residues.push(fixed[FIX_LOCAL_CONTINUE].mul(
            next_aux[LOCAL_PRODUCT_BEFORE + lane].sub(current_aux[LOCAL_PRODUCT_AFTER + lane]),
        ));
        residues.push(
            fixed[FIX_LOCAL_LAST].mul(
                current_aux[LOCAL_PRODUCT_AFTER + lane]
                    .sub(current_aux[GLOBAL_LOCAL_PRODUCT_END + lane]),
            ),
        );
        let execution_factor = memory_factor_v1(current[0], current[1], current[2], challenge);
        let sorted_factor = memory_factor_v1(current[3], current[4], current[5], challenge);
        residues.push(
            memory.mul(
                current_aux[MEMORY_EXEC_AFTER + lane]
                    .sub(current_aux[MEMORY_EXEC_BEFORE + lane].mul(execution_factor)),
            ),
        );
        residues.push(
            memory.mul(
                current_aux[MEMORY_SORT_AFTER + lane]
                    .sub(current_aux[MEMORY_SORT_BEFORE + lane].mul(sorted_factor)),
            ),
        );
        residues.push(
            fixed[FIX_MEMORY_CONTINUE].mul(
                next_aux[MEMORY_EXEC_BEFORE + lane].sub(current_aux[MEMORY_EXEC_AFTER + lane]),
            ),
        );
        residues.push(
            fixed[FIX_MEMORY_CONTINUE].mul(
                next_aux[MEMORY_SORT_BEFORE + lane].sub(current_aux[MEMORY_SORT_AFTER + lane]),
            ),
        );
        residues.push(
            fixed[FIX_MEMORY_FIRST_SEGMENT].mul(
                current_aux[MEMORY_EXEC_BEFORE + lane].sub(current_aux[CONT_EXEC_START + lane]),
            ),
        );
        residues.push(
            fixed[FIX_MEMORY_FIRST_SEGMENT].mul(
                current_aux[MEMORY_SORT_BEFORE + lane].sub(current_aux[CONT_SORT_START + lane]),
            ),
        );
        residues.push(
            fixed[FIX_MEMORY_LAST_SEGMENT]
                .mul(current_aux[MEMORY_EXEC_AFTER + lane].sub(current_aux[CONT_EXEC_END + lane])),
        );
        residues.push(
            fixed[FIX_MEMORY_LAST_SEGMENT]
                .mul(current_aux[MEMORY_SORT_AFTER + lane].sub(current_aux[CONT_SORT_END + lane])),
        );
        residues.push(
            fixed[FIX_LAST_AGGREGATE_ROW]
                .mul(current_aux[CONT_EXEC_END + lane].sub(current_aux[CONT_SORT_END + lane])),
        );
        residues.push(fixed[FIX_LAST_AGGREGATE_ROW].mul(
            current_aux[GLOBAL_LOCAL_PRODUCT_END + lane].sub(current_aux[CONT_EXEC_END + lane]),
        ));
        residues.push(
            fixed[FIX_FIRST_AGGREGATE_ROW].mul(current_aux[CONT_EXEC_START + lane].sub(A::ONE)),
        );
        residues.push(
            fixed[FIX_FIRST_AGGREGATE_ROW].mul(current_aux[CONT_SORT_START + lane].sub(A::ONE)),
        );
        residues.push(
            fixed[FIX_PHYSICAL_BOUNDARY]
                .mul(next_aux[CONT_EXEC_START + lane].sub(current_aux[CONT_EXEC_END + lane])),
        );
        residues.push(
            fixed[FIX_PHYSICAL_BOUNDARY]
                .mul(next_aux[CONT_SORT_START + lane].sub(current_aux[CONT_SORT_END + lane])),
        );
        residues.push(
            fixed[FIX_PHYSICAL_BOUNDARY].mul(
                next_aux[GLOBAL_LOCAL_PRODUCT_END + lane]
                    .sub(current_aux[GLOBAL_LOCAL_PRODUCT_END + lane]),
            ),
        );
    }
    for index in 0..7 {
        residues.push(
            current_aux[CONT_SEGMENT_INDEX + index].sub(fixed[FIX_CONTINUATION_PUBLIC + index]),
        );
    }
    for index in CONT_EXEC_START..SHA_WORD_AUX_WIDTH_V1 {
        residues
            .push(fixed[FIX_CONTINUATION_WITHIN_SLOT].mul(next_aux[index].sub(current_aux[index])));
    }
    for value in &current_aux[..SHA_WORD_LOCAL_PRODUCT_WIDTH_V1] {
        residues.push(padding.mul(*value));
    }
    for value in &current_aux[SHA_WORD_MEMORY_PRODUCT_WIDTH_V1..SHA_WORD_LOCAL_PRODUCT_WIDTH_V1] {
        residues.push(memory.mul(*value));
    }
    if residues.len() != SHA_WORD_STARK_CONSTRAINT_COUNT_V1 {
        return Err(ZkX509ShaWordStarkErrorV1::Topology);
    }
    Ok(residues)
}

/// Evaluate the preserved pre-streaming equation body.
#[allow(clippy::too_many_lines)]
pub(crate) fn original_evaluate_zk_x509_sha_word_capacity_residues_over_field_v1<
    A: PolynomialAirFieldV1,
>(
    current: &[A; SHA_WORD_CAPACITY_BASE_WIDTH_V1],
    next: &[A; SHA_WORD_CAPACITY_BASE_WIDTH_V1],
    current_aux: &[A; SHA_WORD_CAPACITY_AUX_WIDTH_V1],
    next_aux: &[A; SHA_WORD_CAPACITY_AUX_WIDTH_V1],
    fixed: &[A; SHA_WORD_CAPACITY_FIXED_WIDTH_V1],
    challenges: ZkX509ShaWordStarkChallengesV1,
) -> Result<Vec<A>, ZkX509ShaWordStarkErrorV1> {
    validate_zk_x509_sha_word_stark_challenges_v1(challenges)?;
    if current
        .iter()
        .chain(next)
        .chain(current_aux)
        .chain(next_aux)
        .chain(fixed)
        .any(|value| !value.is_canonical())
    {
        return Err(ZkX509ShaWordStarkErrorV1::Topology);
    }
    let row_active = current[SHA_WORD_CAPACITY_ROW_ACTIVE_V1];
    let next_active = next[SHA_WORD_CAPACITY_ROW_ACTIVE_V1];
    let final_block = current[SHA_WORD_CAPACITY_FINAL_BLOCK_V1];
    let next_final_block = next[SHA_WORD_CAPACITY_FINAL_BLOCK_V1];
    let sorted_same_next = current[SHA_WORD_CAPACITY_SORTED_SAME_NEXT_V1];
    let input_word = fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1];
    let digest = fixed[FIX_DIGEST];
    let memory = fixed[FIX_MEMORY];
    let local_compute = fixed_sum_v1(
        fixed[..SHA_WORD_STARK_FIXED_WIDTH_V1]
            .try_into()
            .expect("raw fixed prefix"),
        [
            FIX_WORD,
            FIX_SIGMA_SMALL_ZERO,
            FIX_SIGMA_SMALL_ONE,
            FIX_SIGMA_BIG_ZERO,
            FIX_SIGMA_BIG_ONE,
            FIX_CHOOSE,
            FIX_MAJORITY,
            FIX_ADD_ARITY_TWO,
            FIX_ADD_ARITY_FOUR,
        ],
    );
    let mut effective_fixed: [A; SHA_WORD_STARK_FIXED_WIDTH_V1] = fixed
        [..SHA_WORD_STARK_FIXED_WIDTH_V1]
        .try_into()
        .expect("raw fixed prefix");
    for selector in [
        FIX_WORD,
        FIX_SIGMA_SMALL_ZERO,
        FIX_SIGMA_SMALL_ONE,
        FIX_SIGMA_BIG_ZERO,
        FIX_SIGMA_BIG_ONE,
        FIX_CHOOSE,
        FIX_MAJORITY,
        FIX_ADD_ARITY_TWO,
        FIX_ADD_ARITY_FOUR,
        FIX_DIGEST,
    ] {
        effective_fixed[selector] = effective_fixed[selector].mul(row_active);
    }
    effective_fixed[FIX_MEMORY] = A::ZERO;
    effective_fixed[FIX_PADDING] = A::ZERO;
    for selector in [
        FIX_MEMORY_CONTINUE,
        FIX_MEMORY_SAME_NEXT,
        FIX_MEMORY_NEW_NEXT,
        FIX_MEMORY_FIRST_SEGMENT,
        FIX_MEMORY_LAST_SEGMENT,
        FIX_FIRST_AGGREGATE_ROW,
        FIX_LAST_AGGREGATE_ROW,
        FIX_PHYSICAL_BOUNDARY,
        FIX_CONTINUATION_WITHIN_SLOT,
    ] {
        effective_fixed[selector] = A::ZERO;
    }
    for byte in 0..4 {
        effective_fixed[FIX_WORD_BYTE_MASK + byte] = effective_fixed[FIX_WORD_BYTE_MASK + byte]
            .mul(row_active)
            .mul(A::ONE.sub(input_word));
    }
    effective_fixed[FIX_DIGEST_EXPECTED] = current[0];
    // FIX_DIGEST is Boolean only on the native schedule. Its LDE and OODS
    // values must select algebraically; an equality branch changes the AIR.
    effective_fixed[FIX_EVENT_ADDRESS] = digest
        .mul(current[SHA_WORD_CAPACITY_DYNAMIC_ADDRESS_V1])
        .add(A::ONE.sub(digest).mul(effective_fixed[FIX_EVENT_ADDRESS]));
    for index in 0..7 {
        effective_fixed[FIX_CONTINUATION_PUBLIC + index] = current_aux[CONT_SEGMENT_INDEX + index];
    }
    let current_raw: &[A; SHA_WORD_BASE_WIDTH_V1] = current[..SHA_WORD_BASE_WIDTH_V1]
        .try_into()
        .expect("raw base prefix");
    let next_raw: &[A; SHA_WORD_BASE_WIDTH_V1] = next[..SHA_WORD_BASE_WIDTH_V1]
        .try_into()
        .expect("raw base prefix");
    let current_aux_raw: &[A; SHA_WORD_AUX_WIDTH_V1] = current_aux[..SHA_WORD_AUX_WIDTH_V1]
        .try_into()
        .expect("raw aux prefix");
    let next_aux_raw: &[A; SHA_WORD_AUX_WIDTH_V1] = next_aux[..SHA_WORD_AUX_WIDTH_V1]
        .try_into()
        .expect("raw aux prefix");
    let mut residues = original_evaluate_zk_x509_sha_word_stark_residues_over_field_v1(
        current_raw,
        next_raw,
        current_aux_raw,
        next_aux_raw,
        &effective_fixed,
        challenges,
    )?;
    let boolean_error = |value: A| value.mul(value.sub(A::ONE));
    residues.push(boolean_error(row_active));
    residues.push(boolean_error(final_block));
    residues.push(boolean_error(sorted_same_next));
    residues.push(final_block.mul(A::ONE.sub(row_active)));
    for byte in 0..4 {
        let message = current[SHA_WORD_CAPACITY_MESSAGE_MASK_V1 + byte];
        let marker = current[SHA_WORD_CAPACITY_MARKER_MASK_V1 + byte];
        residues.push(boolean_error(message));
        residues.push(boolean_error(marker));
        residues.push(message.mul(marker));
        residues.push(message.mul(A::ONE.sub(fixed[SHA_WORD_CAPACITY_MESSAGE_ALLOWED_V1 + byte])));
        residues.push(A::ONE.sub(row_active).mul(message));
        residues.push(A::ONE.sub(row_active).mul(marker));
    }
    residues.push(A::ONE.sub(memory).mul(sorted_same_next));
    residues.push(A::ONE.sub(local_compute).mul(final_block));
    for byte in 0..4 {
        residues.push(
            A::ONE
                .sub(input_word)
                .mul(current[SHA_WORD_CAPACITY_MESSAGE_MASK_V1 + byte]),
        );
        residues.push(
            A::ONE
                .sub(input_word)
                .mul(current[SHA_WORD_CAPACITY_MARKER_MASK_V1 + byte]),
        );
    }
    residues.push(
        A::ONE
            .sub(digest)
            .mul(current[SHA_WORD_CAPACITY_DYNAMIC_ADDRESS_V1]),
    );
    residues.push(digest.mul(row_active.sub(A::ONE)));
    let inactive_compute = local_compute.mul(A::ONE.sub(row_active));
    let inactive_memory = memory.mul(A::ONE.sub(row_active));
    let mut folded_inactive_compute = [A::ZERO; SHA_WORD_COPY_LANES_V1];
    let mut folded_inactive_memory = [A::ZERO; SHA_WORD_COPY_LANES_V1];
    for value in current_raw {
        for lane in 0..SHA_WORD_COPY_LANES_V1 {
            folded_inactive_compute[lane] = folded_inactive_compute[lane]
                .mul_base(challenges.base_folding[lane])
                .add(inactive_compute.mul(*value));
            folded_inactive_memory[lane] = folded_inactive_memory[lane]
                .mul_base(challenges.base_folding[lane])
                .add(inactive_memory.mul(*value));
        }
    }
    residues.extend(folded_inactive_compute);
    residues.extend(folded_inactive_memory);
    let block_first = fixed[SHA_WORD_CAPACITY_BLOCK_FIRST_V1];
    let block_continue = fixed[SHA_WORD_CAPACITY_BLOCK_CONTINUE_V1];
    let block_last = fixed[SHA_WORD_CAPACITY_BLOCK_LAST_V1];
    let maximum_block_last = fixed[SHA_WORD_CAPACITY_MAX_BLOCK_LAST_V1];
    residues.push(block_continue.mul(next_active.sub(row_active)));
    residues.push(block_continue.mul(next_final_block.sub(final_block)));
    residues.push(fixed[SHA_WORD_CAPACITY_CALL_FIRST_V1].mul(row_active.sub(A::ONE)));
    residues.push(
        block_last
            .mul(A::ONE.sub(maximum_block_last))
            .mul(final_block.sub(row_active.mul(A::ONE.sub(next_active)))),
    );
    residues.push(
        block_last
            .mul(A::ONE.sub(maximum_block_last))
            .mul(next_active)
            .mul(A::ONE.sub(row_active)),
    );
    residues.push(maximum_block_last.mul(final_block.sub(row_active)));
    let message_count = current_aux[SHA_WORD_CAPACITY_MESSAGE_COUNT_V1];
    let next_message_count = next_aux[SHA_WORD_CAPACITY_MESSAGE_COUNT_V1];
    let padding_phase = current_aux[SHA_WORD_CAPACITY_PADDING_PHASE_V1];
    let next_padding_phase = next_aux[SHA_WORD_CAPACITY_PADDING_PHASE_V1];
    let active_blocks = current_aux[SHA_WORD_CAPACITY_ACTIVE_BLOCKS_V1];
    let next_active_blocks = next_aux[SHA_WORD_CAPACITY_ACTIVE_BLOCKS_V1];
    let call_first = fixed[SHA_WORD_CAPACITY_CALL_FIRST_V1];
    let call_last = fixed[SHA_WORD_CAPACITY_CALL_LAST_V1];
    // Every logical call row has one public operation selector, including
    // privately inactive capacity rows. Physical padding has none. This linear
    // gate keeps carries within a call and excludes the cyclic padding wrap.
    let call_continue = local_compute.add(digest).add(memory).sub(call_last);
    residues.push(boolean_error(padding_phase));
    residues.push(call_first.mul(message_count));
    residues.push(call_first.mul(padding_phase));
    residues.push(call_first.mul(active_blocks));
    residues.push(call_last.mul(padding_phase.sub(A::ONE)));
    residues.push(
        call_last
            .mul(fixed[SHA_WORD_CAPACITY_EXACT_LENGTH_V1])
            .mul(message_count.sub(fixed[SHA_WORD_CAPACITY_MAXIMUM_MESSAGE_LEN_V1])),
    );
    residues.push(
        call_continue.mul(next_active_blocks.sub(active_blocks.add(block_first.mul(row_active)))),
    );
    let mut phase_after = padding_phase;
    let mut message_increment = A::ZERO;
    let mut marker_increment = A::ZERO;
    let length_word = fixed[SHA_WORD_CAPACITY_LENGTH_HIGH_WORD_V1]
        .add(fixed[SHA_WORD_CAPACITY_LENGTH_LOW_WORD_V1]);
    let length_gate = input_word.mul(row_active).mul(final_block).mul(length_word);
    let ordinary_gate = input_word
        .mul(row_active)
        .mul(A::ONE.sub(final_block.mul(length_word)));
    for byte in 0..4 {
        let message = current[SHA_WORD_CAPACITY_MESSAGE_MASK_V1 + byte];
        let marker = current[SHA_WORD_CAPACITY_MARKER_MASK_V1 + byte];
        let bits_start = 1 + (3 - byte) * 8;
        let byte_value = pack_bits(&current[bits_start..bits_start + 8]);
        residues.push(ordinary_gate.mul(message.add(marker).sub(A::ONE.sub(phase_after))));
        residues.push(
            ordinary_gate.mul(
                A::ONE
                    .sub(message)
                    .mul(byte_value)
                    .sub(A::from_base(F(128)).mul(marker)),
            ),
        );
        residues.push(length_gate.mul(message.add(marker)));
        residues.push(length_gate.mul(phase_after.sub(A::ONE)));
        phase_after = phase_after.add(marker);
        message_increment = message_increment.add(message);
        marker_increment = marker_increment.add(marker);
    }
    residues.push(
        input_word
            .mul(row_active)
            .mul(final_block)
            .mul(fixed[SHA_WORD_CAPACITY_LENGTH_HIGH_WORD_V1])
            .mul(current[0]),
    );
    residues.push(
        input_word
            .mul(row_active)
            .mul(final_block)
            .mul(fixed[SHA_WORD_CAPACITY_LENGTH_LOW_WORD_V1])
            .mul(current[0].sub(message_count.mul(A::from_base(F(8))))),
    );
    residues.push(
        call_continue.mul(
            next_message_count
                .sub(message_count.add(input_word.mul(row_active).mul(message_increment))),
        ),
    );
    residues.push(call_continue.mul(
        next_padding_phase.sub(padding_phase.add(input_word.mul(row_active).mul(marker_increment))),
    ));
    residues.push(
        digest.mul(
            current[SHA_WORD_CAPACITY_DYNAMIC_ADDRESS_V1].sub(
                active_blocks
                    .mul(A::from_base(F(u64::try_from(
                        SHA_WORD_CAPACITY_WORD_IDS_PER_BLOCK_V1,
                    )
                    .expect("SHA word-id stride fits u64"))))
                    .add(fixed[SHA_WORD_CAPACITY_DIGEST_WORD_INDEX_V1]),
            ),
        ),
    );
    for lane in 0..SHA_WORD_COPY_LANES_V1 {
        residues.push(inactive_compute.mul(current_aux[LOCAL_PAIR_01 + lane].sub(A::ONE)));
        residues.push(inactive_compute.mul(current_aux[LOCAL_PAIR_23 + lane].sub(A::ONE)));
        residues.push(inactive_compute.mul(current_aux[LOCAL_PAIR_45 + lane].sub(A::ONE)));
        residues.push(inactive_compute.mul(current_aux[LOCAL_QUAD + lane].sub(A::ONE)));
        residues.push(inactive_compute.mul(
            current_aux[LOCAL_PRODUCT_AFTER + lane].sub(current_aux[LOCAL_PRODUCT_BEFORE + lane]),
        ));
    }
    let memory_continue = fixed[FIX_MEMORY_CONTINUE];
    residues.push(memory.mul(boolean_error(current[2])));
    residues.push(memory.mul(boolean_error(current[5])));
    residues.push(inactive_memory.mul(sorted_same_next));
    residues.push(
        memory
            .mul(A::ONE.sub(row_active))
            .mul(A::ONE.sub(call_last))
            .mul(next[SHA_WORD_CAPACITY_ROW_ACTIVE_V1]),
    );
    residues.push(
        memory
            .mul(row_active)
            .mul(A::ONE.sub(next_active))
            .mul(sorted_same_next),
    );
    let adjacent_active = memory_continue.mul(row_active).mul(next_active);
    residues.push(adjacent_active.mul(next[3].sub(current[3].add(A::ONE.sub(sorted_same_next)))));
    residues.push(
        adjacent_active
            .mul(sorted_same_next)
            .mul(next[4].sub(current[4])),
    );
    residues.push(adjacent_active.mul(sorted_same_next).mul(next[5]));
    residues.push(
        adjacent_active
            .mul(A::ONE.sub(sorted_same_next))
            .mul(next[5].sub(A::ONE)),
    );
    residues.push(fixed[FIX_MEMORY_FIRST_SEGMENT].mul(current[3]));
    residues.push(fixed[FIX_MEMORY_FIRST_SEGMENT].mul(current[5].sub(A::ONE)));
    for lane in 0..SHA_WORD_COPY_LANES_V1 {
        let challenge = challenges.memory.lanes[lane];
        let execution_factor = memory_factor_v1(current[0], current[1], current[2], challenge);
        let sorted_factor = memory_factor_v1(current[3], current[4], current[5], challenge);
        let gated_execution_factor = A::ONE.add(row_active.mul(execution_factor.sub(A::ONE)));
        let gated_sorted_factor = A::ONE.add(row_active.mul(sorted_factor.sub(A::ONE)));
        residues.push(
            memory.mul(
                current_aux[MEMORY_EXEC_AFTER + lane]
                    .sub(current_aux[MEMORY_EXEC_BEFORE + lane].mul(gated_execution_factor)),
            ),
        );
        residues.push(
            memory.mul(
                current_aux[MEMORY_SORT_AFTER + lane]
                    .sub(current_aux[MEMORY_SORT_BEFORE + lane].mul(gated_sorted_factor)),
            ),
        );
        residues.push(
            memory_continue.mul(
                next_aux[MEMORY_EXEC_BEFORE + lane].sub(current_aux[MEMORY_EXEC_AFTER + lane]),
            ),
        );
        residues.push(
            memory_continue.mul(
                next_aux[MEMORY_SORT_BEFORE + lane].sub(current_aux[MEMORY_SORT_AFTER + lane]),
            ),
        );
        residues.push(
            fixed[FIX_MEMORY_FIRST_SEGMENT].mul(current_aux[MEMORY_EXEC_BEFORE + lane].sub(A::ONE)),
        );
        residues.push(
            fixed[FIX_MEMORY_FIRST_SEGMENT].mul(current_aux[MEMORY_SORT_BEFORE + lane].sub(A::ONE)),
        );
        residues.push(call_last.mul(
            current_aux[MEMORY_EXEC_AFTER + lane].sub(current_aux[GLOBAL_LOCAL_PRODUCT_END + lane]),
        ));
        residues.push(call_last.mul(
            current_aux[MEMORY_SORT_AFTER + lane].sub(current_aux[GLOBAL_LOCAL_PRODUCT_END + lane]),
        ));
        residues.push(
            call_continue.mul(
                next_aux[GLOBAL_LOCAL_PRODUCT_END + lane]
                    .sub(current_aux[GLOBAL_LOCAL_PRODUCT_END + lane]),
            ),
        );
    }
    for column in SHA_WORD_MEMORY_PRODUCT_WIDTH_V1..SHA_WORD_LOCAL_PRODUCT_WIDTH_V1 {
        residues.push(memory.mul(current_aux[column]));
    }
    for column in CONTINUATION_OFFSET..GLOBAL_LOCAL_PRODUCT_END {
        residues.push(current_aux[column]);
    }
    if residues.len() != SHA_WORD_CAPACITY_CONSTRAINT_COUNT_V1 {
        return Err(ZkX509ShaWordStarkErrorV1::Topology);
    }
    Ok(residues)
}
