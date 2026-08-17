// Shared exact profile constraints for the IVM private-note and PQ-MASP AIRs.
macro_rules! define_note_profile_constraint_residues_v1 {
    ($function_name:ident) => {
        fn $function_name(
            current: &[F],
            next: &[F],
            current_aux: &[F],
            next_aux: &[F],
            fixed: &[F],
        ) -> Result<Vec<F>, ProofManagedNoteStarkErrorV1> {
            if current.len() != BASE_WIDTH
                || next.len() != BASE_WIDTH
                || current_aux.len() != NOTE_COPY_AUX_WIDTH_V1 + PROFILE_AUX_WIDTH
                || next_aux.len() != NOTE_COPY_AUX_WIDTH_V1 + PROFILE_AUX_WIDTH
                || fixed.len() != NOTE_COPY_FIXED_WIDTH_V1 + PROFILE_FIXED_WIDTH
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
            for bit in &current[SHA_BITS_OFFSET..SHA_BITS_OFFSET + SHA_BIT_COLUMNS] {
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
            for index in 0..SHA_SCHEDULE_WORDS {
                push_weighted(
                    &mut residues,
                    round_nonlast,
                    next[SHA_SCHEDULE_OFFSET + index].sub(current[SHA_SCHEDULE_OFFSET + index]),
                );
            }
            for index in 0..SHA_STATE_WORDS {
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
            for group in 8..SHA_BIT_COLUMNS / 32 {
                push_weighted(
                    &mut residues,
                    sha_end,
                    pack_bits(bits_group(current, group)),
                );
            }
            for cell in 0..COPY_WIDTH {
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
                &current[DISTINCT_RIGHT_BITS_OFFSET..DISTINCT_RIGHT_BITS_OFFSET + 8];
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
                &current[VM_DIFFERENCE_BITS_OFFSET..VM_DIFFERENCE_BITS_OFFSET + 8];
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
    };
}
