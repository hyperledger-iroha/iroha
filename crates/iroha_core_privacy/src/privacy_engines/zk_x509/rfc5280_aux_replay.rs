//! Bounded row-major replay of the canonical RFC auxiliary column recurrences.

use super::*;
use crate::privacy_engines::aggregate_stark::MASKED_TRACE_LDE_COLUMN_BATCH_V1 as BATCH;

/// One shared row is cleared on success, ordinary failure, and unwind.
struct RowContextV1 {
    base: ZkX509Rfc5280StarkBaseRowV1,
    fixed: ZkX509Rfc5280StarkFixedRowV1,
    family: ZkX509Rfc5280StarkFamilyV1,
}
impl RowContextV1 {
    fn family_gate_v1(&self, family: ZkX509Rfc5280StarkFamilyV1) -> F {
        // Preserve the original Boolean equality, including malformed activity.
        F(u64::from(
            self.family == family && self.base[BASE_ACTIVE] == F::ONE,
        ))
    }
}
impl Drop for RowContextV1 {
    fn drop(&mut self) {
        zeroize_fields_v1(&mut self.base);
        zeroize_fields_v1(&mut self.fixed);
    }
}

/// Only three private field cells per requested output are retained.
struct ColumnStateV1 {
    product: F,
    sums: [F; 2],
}
impl ColumnStateV1 {
    fn new_v1() -> Self {
        Self {
            product: F::ONE,
            sums: [F::ZERO; 2],
        }
    }
    fn step_v1(
        &mut self,
        column: usize,
        context: &RowContextV1,
        last: bool,
        der_challenges: ZkX509DerStarkChallengesV1,
        challenges: ZkX509Rfc5280StarkChallengesV1,
        sha_union: &ZkX509ShaUnionCentersV1,
    ) -> Result<F, ZkX509Rfc5280StarkErrorV1> {
        let row = &context.base;
        let fixed = &context.fixed;
        if (AUX_NUMERIC_INVERSE..AUX_NUMERIC_ZERO_SUM + numeric::LOOKUP_LANES_V1).contains(&column)
        {
            let offset = column - AUX_NUMERIC_INVERSE;
            let event = numeric_lookup_event_v1(row, fixed);
            let mut values = numeric_replay::step_v1(
                event,
                challenges,
                offset % numeric::LOOKUP_LANES_V1,
                &mut self.sums,
            )?;
            let result = values[offset / numeric::LOOKUP_LANES_V1];
            zeroize_fields_v1(&mut values);
            return Ok(result);
        }
        if (AUX_SHA_UNION_CENTERS..AUX_SERIAL_SOURCE_BEFORE).contains(&column) {
            let index = column - AUX_SHA_UNION_CENTERS;
            return Ok(sha_union.products[index / 4][index % 4]);
        }
        if let Some((relation, lane, after_column)) = product_aux_column_descriptor_v1(column) {
            let family = product_relation_family_v1(relation)?;

            let before = self.product;
            let gate = context.family_gate_v1(family);
            if gate == F::ONE {
                self.product = self.product.mul(product_relation_factor_v1(
                    relation,
                    &row,
                    lane,
                    der_challenges,
                )?);
            }
            let value = if after_column { self.product } else { before };

            return Ok(value);
        }
        if let Some((role_index, consumer, lane)) = output_role_aux_column_descriptor_v1(column) {
            let value = self.product;
            let gate = row[BASE_ACTIVE]
                .mul(fixed[output_role_fixed_selector_column_v1(role_index, consumer)]);
            if gate == F::ONE {
                self.product = self.product.mul(output_role_product_factor_v1(
                    &row, role_index, consumer, lane, challenges,
                ));
            } else if gate != F::ZERO {
                return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
            }

            return Ok(value);
        }
        if let Some((consumer, lane, after_column)) =
            serial_product_aux_column_descriptor_v1(column)
        {
            let before = self.product;
            let gate = if consumer {
                row[BASE_COPY_CONSUMER_ACTIVE]
            } else {
                row[BASE_COPY_SOURCE_ACTIVE]
            };
            if gate == F::ONE {
                self.product = self
                    .product
                    .mul(normalized_copy_factor_v1(&row, lane, challenges));
            }
            let value = if after_column { self.product } else { before };

            return Ok(value);
        }
        if let Some((table, lane, after_column)) =
            grammar_ordinal_product_aux_column_descriptor_v1(column)
        {
            let before = self.product;
            let gate = if table {
                row[BASE_ACTIVE].mul(fixed[FIX_GRAMMAR_ORDINAL_TABLE])
            } else {
                row[BASE_ACTIVE].mul(fixed[FIX_SOURCE_NODE_NON_ROOT])
            };
            if gate == F::ONE {
                let child_count = if table { row[BASE_D] } else { row[BASE_G] };
                self.product = self.product.mul(grammar_ordinal_factor_v1(
                    row[BASE_DOCUMENT],
                    row[BASE_PARENT],
                    row[BASE_CHILD],
                    child_count,
                    lane,
                    challenges,
                ));
            }
            let value = if after_column { self.product } else { before };

            return Ok(value);
        }
        if let Some((kind, lane)) = profile_lookup_aux_column_descriptor_v1(column) {
            let table_gate = row[BASE_PROFILE_TABLE_ACTIVE];
            let table_factor = fixed[FIX_PROFILE_TABLE]
                .mul(profile_byte_factor_v1(&row, lane, challenges))
                .add(
                    fixed[ZkX509Rfc5280StarkFamilyV1::SourceNode as usize]
                        .mul(profile_topology_source_factor_v1(&row, lane, challenges)),
                );
            let multiplicity = row[BASE_PROFILE_TABLE_MULTIPLICITY];
            let query_gate = context.family_gate_v1(ZkX509Rfc5280StarkFamilyV1::FixedByte);
            let query_factor = profile_byte_factor_v1(&row, lane, challenges);
            let topology_query_gate = row[BASE_PROFILE_TOPOLOGY_QUERY_ACTIVE];
            let topology_query_factor = profile_topology_query_factor_v1(&row, lane, challenges);
            let (table_zero, table_inverse) = zero_safe_inverse_v1(table_gate, table_factor);
            let (query_zero, query_inverse) = zero_safe_inverse_v1(query_gate, query_factor);
            let (topology_query_zero, topology_query_inverse) =
                zero_safe_inverse_v1(topology_query_gate, topology_query_factor);
            let value = match kind {
                0 => self.sums[0],
                1 => table_inverse,
                2 => query_inverse,
                3 => self.sums[1],
                4 => table_zero,
                5 => query_zero,
                6 => topology_query_inverse,
                7 => topology_query_zero,
                _ => return Err(ZkX509Rfc5280StarkErrorV1::Shape),
            };
            if !last {
                self.sums[0] = self.sums[0]
                    .add(table_gate.mul(multiplicity).mul(table_inverse))
                    .sub(query_gate.mul(query_inverse))
                    .sub(topology_query_gate.mul(topology_query_inverse));
                self.sums[1] = self.sums[1]
                    .add(table_gate.mul(multiplicity).mul(table_zero))
                    .sub(query_gate.mul(query_zero))
                    .sub(topology_query_gate.mul(topology_query_zero));
            }

            return Ok(value);
        }
        if let Some(lookup) = grammar_lookup_aux_column_descriptor_v1(column) {
            let source_node_gate = context.family_gate_v1(ZkX509Rfc5280StarkFamilyV1::SourceNode);
            let (table_gate, query_gate, table_factor, query_factor, multiplicity) =
                if lookup.parent {
                    (
                        source_node_gate,
                        row[BASE_ACTIVE].mul(fixed[FIX_SOURCE_NODE_NON_ROOT]),
                        grammar_parent_table_factor_v1(&row, lookup.lane, challenges),
                        grammar_parent_query_factor_v1(&row, lookup.lane, challenges),
                        row[BASE_D],
                    )
                } else {
                    (
                        row[BASE_ACTIVE].mul(fixed[FIX_GRAMMAR_RULE_TABLE]),
                        source_node_gate,
                        grammar_rule_table_factor_v1(&fixed, lookup.lane, challenges),
                        grammar_rule_query_factor_v1(&row, lookup.lane, challenges),
                        row[BASE_A],
                    )
                };
            let (table_zero, table_inverse) = zero_safe_inverse_v1(table_gate, table_factor);
            let (query_zero, query_inverse) = zero_safe_inverse_v1(query_gate, query_factor);
            let value = match lookup.kind {
                0 => self.sums[0],
                1 => table_inverse,
                2 => query_inverse,
                3 => self.sums[1],
                4 => table_zero,
                5 => query_zero,
                _ => return Err(ZkX509Rfc5280StarkErrorV1::Shape),
            };
            if !last {
                self.sums[0] = self.sums[0]
                    .add(table_gate.mul(multiplicity).mul(table_inverse))
                    .sub(query_gate.mul(query_inverse));
                self.sums[1] = self.sums[1]
                    .add(table_gate.mul(multiplicity).mul(table_zero))
                    .sub(query_gate.mul(query_zero));
            }

            return Ok(value);
        }
        let lookup =
            lookup_aux_column_descriptor_v1(column).ok_or(ZkX509Rfc5280StarkErrorV1::Shape)?;

        let table_family = if lookup.node {
            ZkX509Rfc5280StarkFamilyV1::SourceNode
        } else {
            ZkX509Rfc5280StarkFamilyV1::SourceByte
        };
        let table_gate = context.family_gate_v1(table_family);
        let serial_gate = context.family_gate_v1(ZkX509Rfc5280StarkFamilyV1::SerialSource);

        let query_gate = if lookup.node {
            serial_gate.add(output_source_node_query_gate_v1(&row, &fixed))
        } else {
            row[BASE_SERIAL_BYTE_QUERY_ACTIVE]
        };
        let table_factor = if lookup.node {
            serial_node_lookup_factor_v1(&row, lookup.lane, challenges)
        } else {
            serial_byte_lookup_factor_v1(
                row[BASE_DOCUMENT],
                row[BASE_ADDRESS],
                row[BASE_VALUE],
                lookup.lane,
                challenges,
            )
        };
        let query_factor = if lookup.node {
            node_query_factor_v1(&row, &fixed, lookup.lane, challenges)
        } else {
            serial_byte_lookup_factor_v1(
                row[BASE_DOCUMENT],
                row[BASE_ADDRESS],
                row[BASE_SERIAL_BYTE_QUERY_VALUE],
                lookup.lane,
                challenges,
            )
        };
        let (table_zero, table_inverse) = zero_safe_inverse_v1(table_gate, table_factor);
        let (query_zero, query_inverse) = zero_safe_inverse_v1(query_gate, query_factor);
        let value = match lookup.kind {
            0 => self.sums[0],
            1 => table_inverse,
            2 => query_inverse,
            3 => self.sums[1],
            4 => table_zero,
            5 => query_zero,
            _ => return Err(ZkX509Rfc5280StarkErrorV1::Shape),
        };
        if !last {
            let multiplicity = if lookup.node {
                row[SERIAL_NODE_TABLE_MULTIPLICITY]
            } else {
                row[SERIAL_BYTE_TABLE_MULTIPLICITY]
            };
            self.sums[0] = self.sums[0]
                .add(table_gate.mul(multiplicity).mul(table_inverse))
                .sub(query_gate.mul(query_inverse));
            self.sums[1] = self.sums[1]
                .add(table_gate.mul(multiplicity).mul(table_zero))
                .sub(query_gate.mul(query_zero));
        }

        Ok(value)
    }
    fn finish_v1(&self, column: usize, last_value: F) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
        if column >= AUX_NUMERIC_INVERSE {
            if self.sums != [F::ZERO; 2] {
                return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
            }
        } else if let Some((kind, _)) = profile_lookup_aux_column_descriptor_v1(column) {
            if matches!(kind, 0 | 3) && last_value != F::ZERO {
                return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
            }
        } else if let Some(lookup) = grammar_lookup_aux_column_descriptor_v1(column) {
            if matches!(lookup.kind, 0 | 3) && last_value != F::ZERO {
                return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
            }
        } else if let Some(lookup) = lookup_aux_column_descriptor_v1(column) {
            if matches!(lookup.kind, 0 | 3) && last_value != F::ZERO {
                return Err(ZkX509Rfc5280StarkErrorV1::Source);
            }
        }
        Ok(())
    }
}
impl Drop for ColumnStateV1 {
    fn drop(&mut self) {
        self.product.zeroize_v1();
        zeroize_fields_v1(&mut self.sums);
    }
}

/// Borrowed output ownership commits only after every requested terminal passes.
struct OutputGuardV1<'a, 'b> {
    outputs: &'a mut [&'b mut [F]],
    committed: bool,
}
impl Drop for OutputGuardV1<'_, '_> {
    fn drop(&mut self) {
        if !self.committed {
            for output in self.outputs.iter_mut() {
                zeroize_fields_v1(output);
            }
        }
    }
}

/// Named resident stack owners; output payload is charged by the caller's existing batch.
/// No heap scratch, additional field column, or parallel replay task is created.
pub(super) const fn scratch_payload_bytes_v1() -> usize {
    core::mem::size_of::<RowContextV1>()
        + core::mem::size_of::<[ColumnStateV1; BATCH]>()
        + core::mem::size_of::<OutputGuardV1<'static, 'static>>()
        + core::mem::size_of::<[&mut [F]; BATCH]>()
        + core::mem::size_of::<numeric::NumericLookupEventV1<F>>()
        + core::mem::size_of::<[F; 4]>()
}

/// Fill at most the existing admitted eight-column replay batch without a heap scratch matrix.
pub(super) fn fill_columns_v1(
    material: &ZkX509Rfc5280StarkBaseMaterialV1,
    der_challenges: ZkX509DerStarkChallengesV1,
    challenges: ZkX509Rfc5280StarkChallengesV1,
    first: usize,
    outputs: &mut [&mut [F]],
    sha_union: &ZkX509ShaUnionCentersV1,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    fill_with_v1(
        ZK_X509_RFC5280_STARK_TRACE_SIZE_V1,
        first,
        outputs,
        der_challenges,
        challenges,
        sha_union,
        |index| {
            // Adopt the private row before another fallible source operation.
            let mut context = RowContextV1 {
                base: material.base_row(index)?,
                fixed: [F::ZERO; ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1],
                family: ZkX509Rfc5280StarkFamilyV1::Padding,
            };
            context.fixed = material.fixed_row(index)?;
            context.family = material.schedule.family_and_ordinal(index)?.0;
            Ok(context)
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn fill_with_v1(
    rows: usize,
    first: usize,
    outputs: &mut [&mut [F]],
    der_challenges: ZkX509DerStarkChallengesV1,
    challenges: ZkX509Rfc5280StarkChallengesV1,
    sha_union: &ZkX509ShaUnionCentersV1,
    mut row_at: impl FnMut(usize) -> Result<RowContextV1, ZkX509Rfc5280StarkErrorV1>,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    der_challenges.validate()?;
    challenges.validate()?;
    sha_union.validate_v1()?;
    let end = first
        .checked_add(outputs.len())
        .filter(|&end| end <= ZK_X509_RFC5280_STARK_AUX_WIDTH_V1)
        .ok_or(ZkX509Rfc5280StarkErrorV1::Shape)?;
    if rows == 0
        || rows > ZK_X509_RFC5280_STARK_TRACE_SIZE_V1
        || outputs.is_empty()
        || outputs.len() > BATCH
        || outputs.iter().any(|output| output.len() != rows)
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Shape);
    }
    let mut output = OutputGuardV1 {
        outputs,
        committed: false,
    };
    let mut states: [ColumnStateV1; BATCH] = core::array::from_fn(|_| ColumnStateV1::new_v1());
    if first >= AUX_SHA_UNION_CENTERS && end <= AUX_SERIAL_SOURCE_BEFORE {
        for (column, target) in (first..end).zip(output.outputs.iter_mut()) {
            let index = column - AUX_SHA_UNION_CENTERS;
            target.fill(sha_union.products[index / 4][index % 4]);
        }
    } else {
        for index in 0..rows {
            let context = row_at(index)?;
            for ((column, target), state) in (first..end)
                .zip(output.outputs.iter_mut())
                .zip(states.iter_mut())
            {
                target[index] = state.step_v1(
                    column,
                    &context,
                    index + 1 == rows,
                    der_challenges,
                    challenges,
                    sha_union,
                )?;
            }
        }
    }
    for ((column, target), state) in (first..end).zip(output.outputs.iter()).zip(states.iter()) {
        state.finish_v1(column, target[rows - 1])?;
    }
    output.committed = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::super::private_table::inspection;
    use super::*;

    fn challenges_v1() -> ZkX509Rfc5280StarkChallengesV1 {
        ZkX509Rfc5280StarkChallengesV1 {
            tuple: core::array::from_fn(|lane| {
                core::array::from_fn(|slot| F(7 + (12 * lane + slot) as u64))
            }),
        }
    }
    fn der_v1() -> ZkX509DerStarkChallengesV1 {
        ZkX509DerStarkChallengesV1 {
            tuple: core::array::from_fn(|lane| {
                core::array::from_fn(|slot| F(101 + (13 * lane + slot) as u64))
            }),
            byte_lookup: core::array::from_fn(|lane| F(301 + lane as u64)),
        }
    }
    fn row_v1() -> RowContextV1 {
        let mut context = RowContextV1 {
            base: [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1],
            fixed: [F::ZERO; ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1],
            family: ZkX509Rfc5280StarkFamilyV1::Padding,
        };
        context.base[BASE_DOCUMENT] = F(73);
        context
    }
    #[test]
    fn auxiliary_batch_geometry_fails_before_destination_mutation() {
        let centers = ZkX509ShaUnionCentersV1::identity_fixture_v1();
        for (first, count, rows, actual_rows) in [
            (0, 0, 3, 3),
            (0, 9, 3, 3),
            (280, 1, 3, 3),
            (usize::MAX, 1, 3, 3),
            (279, 2, 3, 3),
            (0, 1, 0, 0),
            (0, 1, 3, 2),
        ] {
            let mut columns = vec![vec![F(91); actual_rows]; count];
            let mut outputs: Vec<_> = columns.iter_mut().map(Vec::as_mut_slice).collect();
            assert!(
                fill_with_v1(
                    rows,
                    first,
                    &mut outputs,
                    der_v1(),
                    challenges_v1(),
                    &centers,
                    |_| panic!("invalid geometry must not read source")
                )
                .is_err()
            );
            assert!(columns.iter().flatten().all(|value| *value == F(91)));
        }
        assert!(scratch_payload_bytes_v1() < 4096);
        assert_eq!(
            core::mem::size_of::<[ColumnStateV1; BATCH]>(),
            8 * 3 * core::mem::size_of::<F>()
        );
    }
    #[test]
    fn auxiliary_batch_reuses_each_row_and_clears_outputs_on_error_and_unwind() {
        let centers = ZkX509ShaUnionCentersV1::identity_fixture_v1();
        for unwind in [false, true] {
            let mut columns = vec![vec![F(91); 4]; 8];
            let mut outputs: Vec<_> = columns.iter_mut().map(Vec::as_mut_slice).collect();
            let mut calls = 0;
            let (result, observed) = inspection::observe_v1(|| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    fill_with_v1(
                        4,
                        0,
                        &mut outputs,
                        der_v1(),
                        challenges_v1(),
                        &centers,
                        |index| {
                            calls += 1;
                            if index == 2 {
                                assert!(!unwind, "synthetic late row failure");
                                return Err(ZkX509Rfc5280StarkErrorV1::Source);
                            }
                            Ok(row_v1())
                        },
                    )
                }))
            });
            if unwind {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), Err(ZkX509Rfc5280StarkErrorV1::Source));
            }
            drop(outputs);
            assert_eq!(calls, 3);
            assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
            assert!(observed.iter().all(|record| record.nonzero_after == 0));
            assert_eq!(
                observed
                    .iter()
                    .filter(|record| record.cells == 4 && record.nonzero_before > 0)
                    .count(),
                8
            );
            assert_eq!(
                observed
                    .iter()
                    .filter(|record| record.cells == ZK_X509_RFC5280_STARK_BASE_WIDTH_V1
                        && record.nonzero_before > 0)
                    .count(),
                2
            );
        }
        let mut columns = vec![vec![F::ZERO; 4]; 8];
        let mut outputs: Vec<_> = columns.iter_mut().map(Vec::as_mut_slice).collect();
        let mut calls = 0;
        let (_, observed) = inspection::observe_v1(|| {
            fill_with_v1(
                4,
                0,
                &mut outputs,
                der_v1(),
                challenges_v1(),
                &centers,
                |_| {
                    calls += 1;
                    Ok(row_v1())
                },
            )
            .unwrap()
        });
        drop(outputs);
        assert_eq!(calls, 4);
        assert!(columns.iter().flatten().all(|value| *value == F::ONE));
        assert!(observed.iter().all(|record| record.nonzero_after == 0));
        assert_eq!(
            observed
                .iter()
                .filter(|record| record.cells == ZK_X509_RFC5280_STARK_BASE_WIDTH_V1)
                .count(),
            4
        );
    }
    #[test]
    fn auxiliary_batch_constant_centers_skip_rows_and_boolean_family_gate_is_exact() {
        let mut centers = ZkX509ShaUnionCentersV1::identity_fixture_v1();
        for (index, value) in centers.products.iter_mut().flatten().enumerate() {
            *value = F(100 + index as u64);
        }
        for first in [AUX_SHA_UNION_CENTERS, AUX_SHA_UNION_CENTERS + 8] {
            let mut columns = vec![vec![F::ZERO; 3]; 8];
            let mut outputs: Vec<_> = columns.iter_mut().map(Vec::as_mut_slice).collect();
            fill_with_v1(
                3,
                first,
                &mut outputs,
                der_v1(),
                challenges_v1(),
                &centers,
                |_| panic!("constant centers need no row reconstruction"),
            )
            .unwrap();
            for (offset, column) in columns.iter().enumerate() {
                assert_eq!(
                    column,
                    &vec![F(100 + (first - AUX_SHA_UNION_CENTERS + offset) as u64); 3]
                );
            }
        }
        let mut context = row_v1();
        context.family = ZkX509Rfc5280StarkFamilyV1::SourceNode;
        for (active, gate) in [(F::ZERO, F::ZERO), (F::ONE, F::ONE), (F(2), F::ZERO)] {
            context.base[BASE_ACTIVE] = active;
            assert_eq!(context.family_gate_v1(context.family), gate);
            assert_eq!(
                context.family_gate_v1(ZkX509Rfc5280StarkFamilyV1::Padding),
                F::ZERO
            );
        }
    }
    #[test]
    fn auxiliary_batch_preserves_selected_terminal_kinds_and_numeric_complete_census() {
        let mut state = ColumnStateV1::new_v1();
        state.sums = [F(9), F(10)];
        for column in 0..ZK_X509_RFC5280_STARK_AUX_WIDTH_V1 {
            let result = state.finish_v1(column, F(11));
            let expected = if column >= AUX_NUMERIC_INVERSE {
                Some(ZkX509Rfc5280StarkErrorV1::Semantic)
            } else if profile_lookup_aux_column_descriptor_v1(column)
                .is_some_and(|(kind, _)| matches!(kind, 0 | 3))
            {
                Some(ZkX509Rfc5280StarkErrorV1::Semantic)
            } else if grammar_lookup_aux_column_descriptor_v1(column)
                .is_some_and(|d| matches!(d.kind, 0 | 3))
            {
                Some(ZkX509Rfc5280StarkErrorV1::Grammar)
            } else if lookup_aux_column_descriptor_v1(column)
                .is_some_and(|d| matches!(d.kind, 0 | 3))
            {
                Some(ZkX509Rfc5280StarkErrorV1::Source)
            } else {
                None
            };
            assert_eq!(result, expected.map_or(Ok(()), Err), "column{column}");
        }
        // Every numeric kind checks post-final sums; ordinary prefix terminals
        // depend on last output rather than a final update of those sums.
        state.sums = [F::ZERO; 2];
        for column in AUX_NUMERIC_INVERSE..280 {
            assert_eq!(state.finish_v1(column, F(99)), Ok(()));
        }
        assert_eq!(
            state.finish_v1(AUX_PROFILE_LOOKUP_ACCUMULATOR, F(1)),
            Err(ZkX509Rfc5280StarkErrorV1::Semantic)
        );
    }
    #[test]
    fn auxiliary_batch_all_widths_cover_every_descriptor_and_boundary() {
        let centers = ZkX509ShaUnionCentersV1::identity_fixture_v1();
        for width in 1..=8 {
            for first in 0..=280 - width {
                let mut columns = vec![vec![F(99); 3]; width];
                let mut outputs: Vec<_> = columns.iter_mut().map(Vec::as_mut_slice).collect();
                fill_with_v1(
                    3,
                    first,
                    &mut outputs,
                    der_v1(),
                    challenges_v1(),
                    &centers,
                    |_| Ok(row_v1()),
                )
                .unwrap();
                for (offset, actual) in columns.iter().enumerate() {
                    let column = first + offset;
                    // Closed independent partition: inactive product columns are1;
                    // all inactive lookup columns are0. Centers are the identity fixture.
                    let expected = if column < 16
                        || (32..64).contains(&column)
                        || (176..264).contains(&column)
                    {
                        F::ONE
                    } else {
                        F::ZERO
                    };
                    assert_eq!(
                        actual,
                        &vec![expected; 3],
                        "first{first} width{width} column{column}"
                    );
                }
            }
        }
    }

    fn numeric_context_v1(index: usize, value: F) -> RowContextV1 {
        let mut context = row_v1();
        context.base[BASE_NUMERIC_SOURCE] = F(u64::from(index == 0));
        context.base[BASE_NUMERIC_QUERY] = F(u64::from(index != 0));
        context.base[BASE_NUMERIC_MULTIPLICITY] = if index == 0 { F(2) } else { F::ZERO };
        context.base[BASE_G] = value;
        context.fixed[FIX_TIME_VALUE] = F::ONE;
        context
    }
    #[test]
    fn auxiliary_batch_numeric_singular_lanes_include_final_delta_for_all_kinds() {
        let challenges = challenges_v1();
        let centers = ZkX509ShaUnionCentersV1::identity_fixture_v1();
        for singular in [None, Some(0), Some(1), Some(2), Some(3)] {
            let value = singular.map_or(F(23), |lane| {
                let zero = numeric_context_v1(0, F::ZERO);
                let one = numeric_context_v1(0, F::ONE);
                let a = numeric::lookup_factor_v1(
                    numeric_lookup_event_v1(&zero.base, &zero.fixed).tuple,
                    challenges.tuple[lane],
                );
                let b = numeric::lookup_factor_v1(
                    numeric_lookup_event_v1(&one.base, &one.fixed).tuple,
                    challenges.tuple[lane],
                );
                F::ZERO.sub(a).mul(b.sub(a).inv().unwrap())
            });
            let context = numeric_context_v1(0, value);
            let tuple = numeric_lookup_event_v1(&context.base, &context.fixed).tuple;
            let factors = core::array::from_fn::<_, 4, _>(|lane| {
                numeric::lookup_factor_v1(tuple, challenges.tuple[lane])
            });
            if let Some(lane) = singular {
                assert_eq!(factors[lane], F::ZERO);
            }
            for first in [AUX_NUMERIC_INVERSE, AUX_NUMERIC_SUM] {
                let mut columns = vec![vec![F::ZERO; 3]; 8];
                let mut outputs: Vec<_> = columns.iter_mut().map(Vec::as_mut_slice).collect();
                fill_with_v1(
                    3,
                    first,
                    &mut outputs,
                    der_v1(),
                    challenges,
                    &centers,
                    |index| Ok(numeric_context_v1(index, value)),
                )
                .unwrap();
                for (offset, column) in columns.iter().enumerate() {
                    let local = first + offset - AUX_NUMERIC_INVERSE;
                    let lane = local % 4;
                    let kind = local / 4;
                    let inverse = factors[lane].inv().unwrap_or(F::ZERO);
                    let zero = F(u64::from(factors[lane] == F::ZERO));
                    let expected = match kind {
                        0 => vec![inverse; 3],
                        1 => vec![zero; 3],
                        2 => vec![F::ZERO, inverse.mul(F(2)), inverse],
                        3 => vec![F::ZERO, zero.mul(F(2)), zero],
                        _ => unreachable!(),
                    };
                    assert_eq!(column, &expected, "kind{kind} lane{lane}");
                }
            }
            // Missing the last consumer rejects every kind, including inverse/zero,
            // and no partially completed output may survive the rejection.
            for column in AUX_NUMERIC_INVERSE..280 {
                let mut output = [F(91); 2];
                assert_eq!(
                    fill_with_v1(
                        2,
                        column,
                        &mut [&mut output],
                        der_v1(),
                        challenges,
                        &centers,
                        |index| Ok(numeric_context_v1(index, value))
                    ),
                    Err(ZkX509Rfc5280StarkErrorV1::Semantic)
                );
                assert_eq!(output, [F::ZERO; 2]);
            }
        }
    }
    #[test]
    fn auxiliary_batch_output_role_nonboolean_gate_rejects_and_clears() {
        let centers = ZkX509ShaUnionCentersV1::identity_fixture_v1();
        let mut columns = vec![vec![F(91); 2]; 8];
        let mut outputs: Vec<_> = columns.iter_mut().map(Vec::as_mut_slice).collect();
        assert_eq!(
            fill_with_v1(
                2,
                AUX_OUTPUT_ROLE_PRODUCTS,
                &mut outputs,
                der_v1(),
                challenges_v1(),
                &centers,
                |index| {
                    let mut context = row_v1();
                    context.base[BASE_ACTIVE] = if index == 0 { F::ZERO } else { F(2) };
                    context.fixed[output_role_fixed_selector_column_v1(0, false)] = F::ONE;
                    Ok(context)
                }
            ),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );
        assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
    }
    #[test]
    fn auxiliary_batch_ordinary_lookup_singular_steps_preserve_final_row_exclusion() {
        let challenges = challenges_v1();
        let centers = ZkX509ShaUnionCentersV1::identity_fixture_v1();
        for column in 0..280 {
            let (family, kind, lane) =
                if let Some((kind, lane)) = profile_lookup_aux_column_descriptor_v1(column) {
                    (0, kind, lane)
                } else if let Some(lookup) = grammar_lookup_aux_column_descriptor_v1(column) {
                    (
                        if lookup.parent { 1 } else { 2 },
                        usize::from(lookup.kind),
                        lookup.lane,
                    )
                } else if let Some(lookup) = lookup_aux_column_descriptor_v1(column) {
                    (
                        if lookup.node { 4 } else { 3 },
                        usize::from(lookup.kind),
                        lookup.lane,
                    )
                } else {
                    continue;
                };
            let context_at = |value| {
                let mut context = row_v1();
                match family {
                    0 => {
                        context.base[BASE_PROFILE_TABLE_ACTIVE] = F::ONE;
                        context.base[BASE_PROFILE_TABLE_MULTIPLICITY] = F(3);
                        context.fixed[FIX_PROFILE_TABLE] = F::ONE;
                        context.base[BASE_VALUE] = value;
                    }
                    1 => {
                        context.family = ZkX509Rfc5280StarkFamilyV1::SourceNode;
                        context.base[BASE_ACTIVE] = F::ONE;
                        context.base[BASE_D] = F(3);
                        context.base[BASE_NODE] = value;
                    }
                    2 => {
                        context.base[BASE_ACTIVE] = F::ONE;
                        context.fixed[FIX_GRAMMAR_RULE_TABLE] = F::ONE;
                        context.base[BASE_A] = F(3);
                        context.fixed[FIX_EXPECTED] = value;
                    }
                    3 => {
                        context.family = ZkX509Rfc5280StarkFamilyV1::SourceByte;
                        context.base[BASE_ACTIVE] = F::ONE;
                        context.base[SERIAL_BYTE_TABLE_MULTIPLICITY] = F(3);
                        context.base[BASE_VALUE] = value;
                    }
                    4 => {
                        context.family = ZkX509Rfc5280StarkFamilyV1::SourceNode;
                        context.base[BASE_ACTIVE] = F::ONE;
                        context.base[SERIAL_NODE_TABLE_MULTIPLICITY] = F(3);
                        context.base[BASE_NODE] = value;
                    }
                    _ => unreachable!(),
                }
                context
            };
            // These table-only rows have multiplicity3 and no query. Compute
            // the old ordinary lookup equation directly, outside step_v1.
            let factor_at = |context: &RowContextV1| match family {
                0 => profile_byte_factor_v1(&context.base, lane, challenges),
                1 => grammar_parent_table_factor_v1(&context.base, lane, challenges),
                2 => grammar_rule_table_factor_v1(&context.fixed, lane, challenges),
                3 => serial_byte_lookup_factor_v1(
                    context.base[BASE_DOCUMENT],
                    context.base[BASE_ADDRESS],
                    context.base[BASE_VALUE],
                    lane,
                    challenges,
                ),
                4 => serial_node_lookup_factor_v1(&context.base, lane, challenges),
                _ => unreachable!(),
            };
            for singular in [false, true] {
                let value = if singular {
                    let a = factor_at(&context_at(F::ZERO));
                    let b = factor_at(&context_at(F::ONE));
                    F::ZERO.sub(a).mul(b.sub(a).inv().unwrap())
                } else {
                    F(23)
                };
                let context = context_at(value);
                let factor = factor_at(&context);
                assert_eq!(factor == F::ZERO, singular);
                let inverse = factor.inv().unwrap_or(F::ZERO);
                let zero = F(u64::from(singular));
                let expected = match kind {
                    0 => F(17),
                    1 => inverse,
                    3 => F(19),
                    4 => zero,
                    2 | 5 | 6 | 7 => F::ZERO,
                    _ => unreachable!(),
                };
                for last in [false, true] {
                    let mut state = ColumnStateV1::new_v1();
                    state.sums = [F(17), F(19)];
                    assert_eq!(
                        state.step_v1(column, &context, last, der_v1(), challenges, &centers),
                        Ok(expected),
                        "column{column} singular{singular} last{last}"
                    );
                    assert_eq!(
                        state.sums,
                        if last {
                            [F(17), F(19)]
                        } else {
                            [F(17).add(F(3).mul(inverse)), F(19).add(F(3).mul(zero))]
                        }
                    );
                }
            }
        }
    }
}
