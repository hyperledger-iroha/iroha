// Shared byte-assignment, SHA-256 and copy-cell trace construction for the IVM private-note
// and PQ-MASP AIRs. Both engines include this file and invoke the macro with their own error,
// role, fixed-row, width and depth names; every other name resolves in the including engine.
macro_rules! define_note_air_trace_core_v1 {
    (
        error: $error:ident,
        role: $role:ident,
        fixed_row: $fixed_row:ident,
        base_width: $base_width:ident,
        copy_width: $copy_width:ident,
        sha_bits_per_group: $sha_bits_per_group:ident,
        tree_depth: $tree_depth:ident $(,)?
    ) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
        struct ByteVariableV1(usize);
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[allow(variant_size_differences)]
        enum ByteExpressionV1 {
            Constant(u8),
            Variable(ByteVariableV1),
        }
        impl ByteExpressionV1 {
            fn value(self, assignment: &[u8]) -> Result<u8, $error> {
                match self {
                    Self::Constant(value) => Ok(value),
                    Self::Variable(variable) => assignment
                        .get(variable.0)
                        .copied()
                        .ok_or($error::Assignment),
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
            fn value(self, assignment: &[u8]) -> Result<F, $error> {
                match self {
                    Self::Inactive => Ok(F::ZERO),
                    Self::Constant(value) => Ok(F(u64::from(value))),
                    Self::Variable(variable) => assignment
                        .get(variable.0)
                        .copied()
                        .map(|value| F(u64::from(value)))
                        .ok_or($error::Assignment),
                }
            }
        }
        impl TraceBuilderV1<'_> {
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
            ) -> Result<(), $error> {
                for (variable, byte) in variables.into_iter().zip(bytes) {
                    let assigned = self
                        .assignment
                        .get_mut(variable.0)
                        .ok_or($error::Assignment)?;
                    if self.witness.is_some() && *assigned != byte {
                        return Err($error::Assignment);
                    }
                    *assigned = byte;
                }
                Ok(())
            }
            fn push_row(
                &mut self,
                fixed: $fixed_row,
                cells: [CopyCellV1; $copy_width],
                mut row: Vec<F>,
            ) -> Result<(), $error> {
                if row.len() != $base_width {
                    return Err($error::Topology);
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
                vec![F::ZERO; $base_width]
            }
            fn check_invocation(
                &mut self,
                role: $role,
                message: &[u8],
                digest: [u8; 32],
            ) -> Result<(), $error> {
                if self.witness.is_none() {
                    return Ok(());
                }
                let expected = self
                    .invocation_oracle
                    .get(self.invocation_cursor)
                    .ok_or($error::Topology)?;
                if expected.role != role
                    || expected.preimage != message
                    || expected.digest != digest
                {
                    return Err($error::Assignment);
                }
                self.invocation_cursor += 1;
                Ok(())
            }
            fn push_hash(
                &mut self,
                role: $role,
                message: Vec<ByteExpressionV1>,
                digest_variables: [ByteVariableV1; 32],
                public_digest: Option<[u8; 32]>,
            ) -> Result<(), $error> {
                let invocation =
                    u8::try_from(self.hash_invocation_count).map_err(|_| $error::Resource)?;
                let padded = sha256_padding_v1(&message)?;
                let block_count = u8::try_from(padded.len() / 64).map_err(|_| $error::Resource)?;
                if block_count == 0 || padded.len() % 64 != 0 {
                    return Err($error::Topology);
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
                        for (group, value) in
                            [a, b, c, e, f, g, schedule[round]].into_iter().enumerate()
                        {
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
                            u32::try_from(t1_wide >> 32).map_err(|_| $error::Sha256)?,
                            3,
                        );
                        write_u32_carry(
                            &mut row,
                            SHA_CARRY_OFFSET + 3,
                            u32::try_from(t2_wide >> 32).map_err(|_| $error::Sha256)?,
                            1,
                        );
                        write_u32_carry(
                            &mut row,
                            SHA_CARRY_OFFSET + 4,
                            u32::try_from(new_a_wide >> 32).map_err(|_| $error::Sha256)?,
                            1,
                        );
                        write_u32_carry(
                            &mut row,
                            SHA_CARRY_OFFSET + 5,
                            u32::try_from(new_e_wide >> 32).map_err(|_| $error::Sha256)?,
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
                                u32::try_from(schedule_wide >> 32).map_err(|_| $error::Sha256)?,
                                2,
                            );
                        }
                        if round == 63 {
                            for index in 0..8 {
                                let feed_forward =
                                    u64::from(initial[index]) + u64::from(next[index]);
                                write_u32_carry(
                                    &mut row,
                                    SHA_CARRY_OFFSET + 8 + index,
                                    u32::try_from(feed_forward >> 32)
                                        .map_err(|_| $error::Sha256)?,
                                    1,
                                );
                            }
                        }
                        let cells = if round < 16 {
                            copy_cells_for_word(&block[round * 4..round * 4 + 4])?
                        } else {
                            [CopyCellV1::Inactive; $copy_width]
                        };
                        self.push_row(
                            $fixed_row::ShaRound {
                                round: u8::try_from(round).map_err(|_| $error::Resource)?,
                                invocation,
                                block: u8::try_from(block_index).map_err(|_| $error::Resource)?,
                                block_count,
                            },
                            cells,
                            row,
                        )?;
                        working = next;
                    }
                    state =
                        core::array::from_fn(|index| initial[index].wrapping_add(working[index]));
                    let terminal = block_index + 1 == usize::from(block_count);
                    for digest_chunk in 0..4 {
                        let mut row = Self::empty_row();
                        for (index, value) in state.iter().copied().enumerate() {
                            row[SHA_STATE_OFFSET + index] = F(u64::from(value));
                            write_word_bits(&mut row, index, value);
                        }
                        let mut cells = [CopyCellV1::Inactive; $copy_width];
                        if terminal {
                            let first = digest_chunk * $copy_width;
                            for (cell, variable) in cells
                                .iter_mut()
                                .zip(digest_variables[first..first + $copy_width].iter())
                            {
                                *cell = CopyCellV1::Variable(*variable);
                            }
                        }
                        self.push_row(
                            $fixed_row::ShaEnd {
                                invocation,
                                block: u8::try_from(block_index).map_err(|_| $error::Resource)?,
                                block_count,
                                digest_chunk: u8::try_from(digest_chunk)
                                    .map_err(|_| $error::Resource)?,
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
                    .map_err(|_| $error::Sha256)?;
                self.assign_bytes(digest_variables, digest)?;
                if public_digest.is_some_and(|expected| expected != digest)
                    && self.witness.is_some()
                {
                    return Err($error::Assignment);
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
                    .ok_or($error::Resource)?;
                Ok(())
            }
            fn push_node_select(
                &mut self,
                input: u8,
                level: u8,
                position_bit: ByteVariableV1,
                current: [ByteVariableV1; 32],
                sibling: [ByteVariableV1; 32],
            ) -> Result<([ByteVariableV1; 32], [ByteVariableV1; 32]), $error> {
                let bit = self.assignment[position_bit.0];
                if self.witness.is_some() && bit > 1 {
                    return Err($error::Assignment);
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
                        $fixed_row::NodeSelect {
                            input,
                            level,
                            byte: u8::try_from(byte).map_err(|_| $error::Resource)?,
                        },
                        cells,
                        Self::empty_row(),
                    )?;
                }
                Ok((left, right))
            }
            fn push_nonzero(
                &mut self,
                component: u16,
                variables: &[ByteVariableV1],
            ) -> Result<(), $error> {
                if variables.is_empty() || variables.len() > 32 {
                    return Err($error::Topology);
                }
                let chunks = variables.len().div_ceil($copy_width);
                let selected = variables
                    .iter()
                    .position(|variable| self.assignment[variable.0] != 0);
                if self.witness.is_some() && selected.is_none() {
                    return Err($error::Assignment);
                }
                let mut running = 0_u8;
                for chunk in 0..chunks {
                    let start = chunk * $copy_width;
                    let end = (start + $copy_width).min(variables.len());
                    let mut cells = [CopyCellV1::Inactive; $copy_width];
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
                        $fixed_row::NonZero {
                            component,
                            chunk: u8::try_from(chunk).map_err(|_| $error::Resource)?,
                            chunks: u8::try_from(chunks).map_err(|_| $error::Resource)?,
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
            ) -> Result<[ByteVariableV1; 16], $error> {
                if operands.is_empty() || operands.len() > 2 {
                    return Err($error::Topology);
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
                        return Err($error::Assignment);
                    }
                    let next_carry = wide >> 8;
                    let mut cells = [CopyCellV1::Inactive; $copy_width];
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
                        $fixed_row::Sum {
                            side,
                            byte: u8::try_from(little_byte).map_err(|_| $error::Resource)?,
                        },
                        cells,
                        row,
                    )?;
                    carry = next_carry;
                }
                if self.witness.is_some() && carry != 0 {
                    return Err($error::Assignment);
                }
                Ok(sum_variables)
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
        ) -> Result<Vec<ByteExpressionV1>, $error> {
            let domain_len = u16::try_from(domain.len()).map_err(|_| $error::Resource)?;
            let field_count = u16::try_from(fields.len()).map_err(|_| $error::Resource)?;
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
                .ok_or($error::Resource)?;
            let mut message = Vec::new();
            message
                .try_reserve_exact(capacity)
                .map_err(|_| $error::Resource)?;
            message.extend(constants_as_expressions(HASH_FRAME_DOMAIN_V1));
            message.extend(constants_as_expressions(&domain_len.to_be_bytes()));
            message.extend(constants_as_expressions(domain));
            message.extend(constants_as_expressions(&field_count.to_be_bytes()));
            for field in fields {
                let length = u64::try_from(field.len()).map_err(|_| $error::Resource)?;
                message.extend(constants_as_expressions(&length.to_be_bytes()));
                message.extend(field.iter().copied());
            }
            if message.len() != capacity {
                return Err($error::Topology);
            }
            Ok(message)
        }
        fn sha256_padding_v1(
            message: &[ByteExpressionV1],
        ) -> Result<Vec<ByteExpressionV1>, $error> {
            let bit_len = u64::try_from(message.len())
                .map_err(|_| $error::Resource)?
                .checked_mul(8)
                .ok_or($error::Resource)?;
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
            let start = SHA_BITS_OFFSET + group * $sha_bits_per_group;
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
        ) -> Result<[CopyCellV1; $copy_width], $error> {
            if bytes.len() != 4 {
                return Err($error::Topology);
            }
            let mut cells = [CopyCellV1::Inactive; $copy_width];
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
        ) -> Result<u32, $error> {
            if bytes.len() != 4 {
                return Err($error::Topology);
            }
            Ok(u32::from_be_bytes([
                bytes[0].value(assignment)?,
                bytes[1].value(assignment)?,
                bytes[2].value(assignment)?,
                bytes[3].value(assignment)?,
            ]))
        }
        fn build_copy_sigma_v1(
            cells: &[[CopyCellV1; $copy_width]],
        ) -> Result<Vec<[u32; $copy_width]>, $error> {
            let mut occurrences = BTreeMap::<ByteVariableV1, Vec<(usize, usize)>>::new();
            for (row, cells) in cells.iter().enumerate() {
                for (column, cell) in cells.iter().copied().enumerate() {
                    if let CopyCellV1::Variable(variable) = cell {
                        occurrences.entry(variable).or_default().push((row, column));
                    }
                }
            }
            let mut sigma = vec![[0_u32; $copy_width]; cells.len()];
            for (row, row_sigma) in sigma.iter_mut().enumerate() {
                for (column, value) in row_sigma.iter_mut().enumerate() {
                    let identity = row
                        .checked_mul($copy_width)
                        .and_then(|value| value.checked_add(column))
                        .and_then(|value| value.checked_add(1))
                        .ok_or($error::Resource)?;
                    *value = u32::try_from(identity).map_err(|_| $error::Resource)?;
                }
            }
            for positions in occurrences.values() {
                for (index, &(row, column)) in positions.iter().enumerate() {
                    let (next_row, next_column) = positions[(index + 1) % positions.len()];
                    let label = next_row
                        .checked_mul($copy_width)
                        .and_then(|value| value.checked_add(next_column))
                        .and_then(|value| value.checked_add(1))
                        .ok_or($error::Resource)?;
                    sigma[row][column] = u32::try_from(label).map_err(|_| $error::Resource)?;
                }
            }
            Ok(sigma)
        }
        fn dummy_path() -> [[u8; 32]; $tree_depth] {
            [[0; 32]; $tree_depth]
        }
    };
}
