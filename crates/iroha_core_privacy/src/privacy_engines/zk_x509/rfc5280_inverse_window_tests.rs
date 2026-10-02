//! Test-only fixed-window inversion with bounded, clearing private ownership.
//!
//! The production provider still selects scalar inversion. Pair order and count
//! depend only on public column descriptors; no private equality deduplicates work.
//! TODO: Require complete native parity, real same-shape timing and resource/erasure
//! review before considering production admission. This is not a constant-time
//! claim for the existing private row constructors or the whole prover.

use super::super::super::private_table::inspection;
use super::*;

const PAIRS: usize = 3 * BATCH;
const GATE: usize = 0;
const DENOMINATOR: usize = 1;
const ACTIVE: usize = 2;
const NONZERO: usize = 3;
const NEUTRAL: usize = 4;
const PREFIX: usize = 5;
const INVERSE: usize = 6;
const ZERO: usize = 7;

/// Each gate, denominator, normalized factor, prefix and result has one owner.
/// The four working cells own product, inverse product, temporary inverse and mask.
struct InverseWindowV1 {
    pairs: [[F; 8]; PAIRS],
    work: [F; 4],
    count: usize,
    cursor: usize,
}
impl InverseWindowV1 {
    fn new_v1() -> Self {
        Self {
            pairs: [[F::ZERO; 8]; PAIRS],
            work: [F::ZERO; 4],
            count: 0,
            cursor: 0,
        }
    }
    fn collect_v1(&mut self, gate: F, denominator: F) -> (F, F) {
        assert!(self.count < PAIRS, "public inverse window capacity");
        let pair = &mut self.pairs[self.count];
        pair[GATE] = gate;
        pair[DENOMINATOR] = denominator;
        self.count += 1;
        // Preserve the original panic and its ordering for malformed nonzero
        // factors. Inactive noncanonical factors are ignored by the old helper.
        // Bitwise OR evaluates both predicates; valid factors never take this path.
        assert!(
            (gate == F::ZERO) | (F::canonical(denominator.0).is_some()),
            "nonzero canonical Goldilocks value is invertible"
        );
        (F::ZERO, F::ZERO)
    }
    fn invert_v1(&mut self) {
        // The empty-window branch is determined solely by public descriptors.
        if self.count == 0 {
            return;
        }
        self.work[0] = F::ONE;
        for pair in &mut self.pairs[..self.count] {
            pair[ACTIVE] = F(u64::from(pair[GATE] != F::ZERO));
            pair[NONZERO] = F(u64::from(pair[DENOMINATOR] != F::ZERO));
            self.work[3].0 = 0_u64.wrapping_sub(pair[ACTIVE].0 & pair[NONZERO].0);
            // A private inactive/noncanonical denominator never enters field
            // arithmetic. Every factor actually multiplied here is canonical nonzero.
            pair[NEUTRAL].0 = (pair[DENOMINATOR].0 & self.work[3].0) | (1 & !self.work[3].0);
            pair[PREFIX] = self.work[0];
            self.work[0] = self.work[0].mul(pair[NEUTRAL]);
        }
        self.work[1] = self.work[0].inverse_or_zero_canonical_v1();
        for pair in self.pairs[..self.count].iter_mut().rev() {
            self.work[2] = self.work[1].mul(pair[PREFIX]);
            self.work[1] = self.work[1].mul(pair[NEUTRAL]);
            pair[INVERSE] = self.work[2].mul(pair[ACTIVE]).mul(pair[NONZERO]);
            pair[ZERO] = pair[ACTIVE].mul(F::ONE.sub(pair[NONZERO]));
        }
    }
    fn replay_v1(&mut self, gate: F, denominator: F) -> (F, F) {
        assert!(
            self.cursor < self.count,
            "public inverse window replay count"
        );
        let pair = &self.pairs[self.cursor];
        self.cursor += 1;
        // Invariant check only: the same immutable row and public descriptor
        // must reproduce both inputs. Never print private fields on a failure.
        assert!(
            (pair[GATE] == gate) & (pair[DENOMINATOR] == denominator),
            "inverse replay inputs changed"
        );
        (pair[ZERO], pair[INVERSE])
    }
    fn finish_v1(&self) {
        assert_eq!(self.cursor, self.count);
    }
}
impl Drop for InverseWindowV1 {
    fn drop(&mut self) {
        for pair in &mut self.pairs {
            zeroize_fields_v1(pair);
        }
        zeroize_fields_v1(&mut self.work);
    }
}

fn pairs_for_column_v1(column: usize) -> usize {
    if (AUX_NUMERIC_INVERSE..AUX_NUMERIC_ZERO_SUM + numeric::LOOKUP_LANES_V1).contains(&column) {
        1
    } else if profile_lookup_aux_column_descriptor_v1(column).is_some() {
        3
    } else if grammar_lookup_aux_column_descriptor_v1(column).is_some()
        || lookup_aux_column_descriptor_v1(column).is_some()
    {
        2
    } else {
        0
    }
}

/// Additional resident private storage is counted explicitly; the existing
/// output batch, row context, numeric event and recurrence values remain charged.
pub(in super::super) const fn scratch_payload_bytes_v1() -> usize {
    super::scratch_payload_bytes_v1()
        + core::mem::size_of::<InverseWindowV1>()
        + core::mem::size_of::<ColumnStateV1>()
        + core::mem::size_of::<F>()
}

#[allow(clippy::too_many_arguments)]
fn step_window_v1(
    states: &mut [ColumnStateV1; BATCH],
    first: usize,
    outputs: &mut [&mut [F]],
    index: usize,
    context: &RowContextV1,
    last: bool,
    der: ZkX509DerStarkChallengesV1,
    challenges: ZkX509Rfc5280StarkChallengesV1,
    centers: &ZkX509ShaUnionCentersV1,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    let mut window = InverseWindowV1::new_v1();
    for (offset, (state, target)) in states.iter_mut().zip(outputs.iter_mut()).enumerate() {
        let column = first + offset;
        if pairs_for_column_v1(column) != 0 {
            // Every temporary recurrence cell uses the same clearing owner.
            // Factors depend on the row/challenges, not these prefix sums.
            let mut copy = ColumnStateV1 {
                product: state.product,
                sums: state.sums,
            };
            let before = window.count;
            let mut discarded = copy.step_with_inverse_v1(
                column,
                context,
                last,
                der,
                challenges,
                centers,
                &mut |gate, factor| window.collect_v1(gate, factor),
            )?;
            discarded.zeroize_v1();
            assert_eq!(window.count - before, pairs_for_column_v1(column));
        } else {
            // Execute noninverse columns once, in their original order. This
            // preserves an earlier product/shape error before a later malformed
            // inverse factor. OutputGuard clears these outputs if any later step fails.
            target[index] = state.step_v1(column, context, last, der, challenges, centers)?;
        }
    }
    window.invert_v1();
    for (offset, (state, target)) in states.iter_mut().zip(outputs.iter_mut()).enumerate() {
        if pairs_for_column_v1(first + offset) != 0 {
            target[index] = state.step_with_inverse_v1(
                first + offset,
                context,
                last,
                der,
                challenges,
                centers,
                &mut |gate, factor| window.replay_v1(gate, factor),
            )?;
        }
    }
    window.finish_v1();
    Ok(())
}

/// Fill at most the existing admitted eight-column replay batch without a heap scratch matrix.
pub(in super::super) fn fill_columns_v1(
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
            step_window_v1(
                &mut states,
                first,
                output.outputs,
                index,
                &context,
                index + 1 == rows,
                der_challenges,
                challenges,
                sha_union,
            )?;
        }
    }
    for ((column, target), state) in (first..end).zip(output.outputs.iter()).zip(states.iter()) {
        state.finish_v1(column, target[rows - 1])?;
    }
    output.committed = true;
    Ok(())
}

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
    let mut row = RowContextV1 {
        base: [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1],
        fixed: [F::ZERO; ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1],
        family: ZkX509Rfc5280StarkFamilyV1::Padding,
    };
    row.base[BASE_DOCUMENT] = F(73);
    row
}

#[test]
fn private_inverse_window_preserves_zero_any_nonzero_gate_and_canonical_domain() {
    let modulus = crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
    let cases = [
        (F::ZERO, F::ZERO),
        (F::ZERO, F(u64::MAX)),
        (F::ONE, F::ZERO),
        (F(2), F::ZERO),
        (F(u64::MAX), F::ZERO),
        (F::ONE, F::ONE),
        (F(2), F(2)),
        (F(modulus - 1), F(modulus - 1)),
        (F(u64::MAX), F(257)),
    ];
    for count in 1..=PAIRS {
        for shift in 0..cases.len() {
            let mut window = InverseWindowV1::new_v1();
            for index in 0..count {
                let (gate, denominator) = cases[(index + shift) % cases.len()];
                assert_eq!(window.collect_v1(gate, denominator), (F::ZERO, F::ZERO));
            }
            window.invert_v1();
            for index in 0..count {
                let (gate, denominator) = cases[(index + shift) % cases.len()];
                assert_eq!(
                    window.replay_v1(gate, denominator),
                    zero_safe_inverse_v1(gate, denominator)
                );
            }
            window.finish_v1();
        }
    }
    for gate in [F::ONE, F(2), F(u64::MAX)] {
        for denominator in [F(modulus), F(u64::MAX)] {
            assert!(std::panic::catch_unwind(|| zero_safe_inverse_v1(gate, denominator)).is_err());
            let (result, observed) = inspection::observe_v1(|| {
                std::panic::catch_unwind(|| {
                    let mut window = InverseWindowV1::new_v1();
                    window.collect_v1(F::ONE, F(19));
                    window.collect_v1(gate, denominator);
                })
            });
            assert!(result.is_err());
            assert!(observed.iter().all(|item| item.nonzero_after == 0));
            assert!(
                observed
                    .iter()
                    .any(|item| item.cells == 8 && item.nonzero_before > 0)
            );
        }
    }
}

#[test]
fn private_inverse_window_all_widths_descriptors_and_resource_cells_match() {
    let centers = ZkX509ShaUnionCentersV1::identity_fixture_v1();
    for width in 1..=BATCH {
        for first in 0..=ZK_X509_RFC5280_STARK_AUX_WIDTH_V1 - width {
            let pairs = (first..first + width)
                .map(pairs_for_column_v1)
                .sum::<usize>();
            assert!(pairs <= PAIRS);
            let mut expected = vec![vec![F(91); 3]; width];
            let mut actual = vec![vec![F(91); 3]; width];
            let mut expected_refs: Vec<_> = expected.iter_mut().map(Vec::as_mut_slice).collect();
            let mut actual_refs: Vec<_> = actual.iter_mut().map(Vec::as_mut_slice).collect();
            super::fill_with_v1(
                3,
                first,
                &mut expected_refs,
                der_v1(),
                challenges_v1(),
                &centers,
                |_| Ok(row_v1()),
            )
            .unwrap();
            fill_with_v1(
                3,
                first,
                &mut actual_refs,
                der_v1(),
                challenges_v1(),
                &centers,
                |_| Ok(row_v1()),
            )
            .unwrap();
            assert_eq!(actual, expected, "first{first} width{width}");
        }
    }
    assert_eq!((0..280).map(pairs_for_column_v1).sum::<usize>(), 304);
    assert_eq!(
        core::mem::size_of::<InverseWindowV1>(),
        (PAIRS * 8 + 4) * core::mem::size_of::<F>() + 2 * core::mem::size_of::<usize>()
    );
    assert_eq!(
        scratch_payload_bytes_v1() - super::scratch_payload_bytes_v1(),
        core::mem::size_of::<InverseWindowV1>() + 4 * core::mem::size_of::<F>()
    );
    assert!(scratch_payload_bytes_v1() < 8192);
}

#[test]
fn private_inverse_window_active_recurrences_and_numeric_validation_match() {
    let centers = ZkX509ShaUnionCentersV1::identity_fixture_v1();
    for family in [
        ZkX509Rfc5280StarkFamilyV1::SourceByte,
        ZkX509Rfc5280StarkFamilyV1::SourceNode,
        ZkX509Rfc5280StarkFamilyV1::FixedByte,
        ZkX509Rfc5280StarkFamilyV1::NameValue,
        ZkX509Rfc5280StarkFamilyV1::SerialSource,
    ] {
        for active in [F::ZERO, F::ONE, F(2)] {
            for last in [false, true] {
                let mut context = row_v1();
                context.family = family;
                context.base[BASE_ACTIVE] = active;
                context.fixed[family as usize] = F::ONE;
                for first in (0..280).step_by(BATCH) {
                    let count = (280 - first).min(BATCH);
                    let mut expected = [F::ZERO; BATCH];
                    let mut actual = vec![vec![F::ZERO; 1]; count];
                    let mut states: [ColumnStateV1; BATCH] =
                        core::array::from_fn(|_| ColumnStateV1 {
                            product: F(17),
                            sums: [F(19), F(23)],
                        });
                    let mut originals: [ColumnStateV1; BATCH] =
                        core::array::from_fn(|_| ColumnStateV1 {
                            product: F(17),
                            sums: [F(19), F(23)],
                        });
                    let mut reference = Ok(());
                    for offset in 0..count {
                        match originals[offset].step_v1(
                            first + offset,
                            &context,
                            last,
                            der_v1(),
                            challenges_v1(),
                            &centers,
                        ) {
                            Ok(value) => expected[offset] = value,
                            Err(error) => {
                                reference = Err(error);
                                break;
                            }
                        }
                    }
                    let mut refs: Vec<_> = actual.iter_mut().map(Vec::as_mut_slice).collect();
                    let result = step_window_v1(
                        &mut states,
                        first,
                        &mut refs,
                        0,
                        &context,
                        last,
                        der_v1(),
                        challenges_v1(),
                        &centers,
                    );
                    assert_eq!(
                        result, reference,
                        "first{first} active{active:?} family{family:?}"
                    );
                    if result.is_ok() {
                        for offset in 0..count {
                            assert_eq!(actual[offset][0], expected[offset]);
                            assert_eq!(states[offset].sums, originals[offset].sums);
                            assert_eq!(states[offset].product, originals[offset].product);
                        }
                    }
                }
            }
        }
    }
    for mutation in 0..5 {
        let mut event = numeric::NumericLookupEventV1 {
            source: F::ONE,
            query: F::ZERO,
            multiplicity: F::ONE,
            tuple: [F::ZERO; 12],
        };
        match mutation {
            0 => event.source = F(2),
            1 => event.query = F::ONE,
            2 => event.multiplicity = F(u64::MAX),
            3 => event.tuple[5] = F(u64::MAX),
            _ => {}
        }
        let mut scalar = [F::ZERO; 2];
        let mut candidate = [F::ZERO; 2];
        let mut calls = 0;
        let duplicate = numeric::NumericLookupEventV1 {
            source: event.source,
            query: event.query,
            multiplicity: event.multiplicity,
            tuple: event.tuple,
        };
        let expected = numeric_replay::step_v1(duplicate, challenges_v1(), 0, &mut scalar);
        let actual = numeric_replay::step_with_inverse_v1(
            event,
            challenges_v1(),
            0,
            &mut candidate,
            &mut |gate, factor| {
                calls += 1;
                zero_safe_inverse_v1(gate, factor)
            },
        );
        assert_eq!(actual, expected);
        assert_eq!(scalar, candidate);
        assert_eq!(calls, usize::from(mutation == 4));
    }
}

#[test]
fn private_inverse_window_clears_owned_pairs_outputs_and_state_on_failure_unwind_success() {
    let centers = ZkX509ShaUnionCentersV1::identity_fixture_v1();
    for unwind in [false, true] {
        let mut columns = vec![vec![F(91); 4]; BATCH];
        let mut refs: Vec<_> = columns.iter_mut().map(Vec::as_mut_slice).collect();
        let mut calls = 0;
        let (result, observed) = inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                fill_with_v1(
                    4,
                    AUX_PROFILE_LOOKUP_ACCUMULATOR,
                    &mut refs,
                    der_v1(),
                    challenges_v1(),
                    &centers,
                    |index| {
                        calls += 1;
                        if index == 2 {
                            assert!(!unwind, "synthetic late source unwind");
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
        drop(refs);
        assert_eq!(calls, 3);
        assert!(columns.iter().flatten().all(|x| *x == F::ZERO));
        assert!(observed.iter().all(|item| item.nonzero_after == 0));
        assert!(
            observed
                .iter()
                .any(|item| item.cells == 8 && item.nonzero_before > 0)
        );
        assert!(
            observed
                .iter()
                .any(|item| item.cells == 4 && item.nonzero_before > 0)
        );
    }
    let ((), observed) = inspection::observe_v1(|| {
        let mut window = InverseWindowV1::new_v1();
        for _ in 0..PAIRS {
            window.collect_v1(F(2), F(19));
        }
        window.invert_v1();
        for _ in 0..PAIRS {
            assert_eq!(
                window.replay_v1(F(2), F(19)),
                zero_safe_inverse_v1(F(2), F(19))
            );
        }
        window.finish_v1();
    });
    assert_eq!(
        observed
            .iter()
            .filter(|item| item.cells == 8 && item.nonzero_before > 0)
            .count(),
        PAIRS
    );
    assert!(observed.iter().all(|item| item.nonzero_after == 0));
    for malformed in [0, 1, 2] {
        let (result, observed) = inspection::observe_v1(|| {
            std::panic::catch_unwind(|| {
                let mut window = InverseWindowV1::new_v1();
                window.collect_v1(F::ONE, F(19));
                window.invert_v1();
                match malformed {
                    0 => {
                        window.replay_v1(F::ONE, F(23));
                    }
                    1 => {
                        window.finish_v1();
                    }
                    _ => {
                        for _ in 0..PAIRS {
                            window.collect_v1(F::ONE, F(19));
                        }
                    }
                }
            })
        });
        assert!(result.is_err());
        assert!(observed.iter().all(|item| item.nonzero_after == 0));
    }
}

#[test]
fn private_inverse_window_preserves_geometry_and_earliest_original_error() {
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
        let mut refs: Vec<_> = columns.iter_mut().map(Vec::as_mut_slice).collect();
        assert_eq!(
            fill_with_v1(
                rows,
                first,
                &mut refs,
                der_v1(),
                challenges_v1(),
                &centers,
                |_| panic!("invalid geometry before source")
            ),
            Err(ZkX509Rfc5280StarkErrorV1::Shape)
        );
        assert!(columns.iter().flatten().all(|value| *value == F(91)));
    }
    // The last output-role product precedes numeric columns in this window.
    // Both are malformed, but the original TerminalClaim must win before the
    // later numeric Semantic failure; both variants clear every output.
    for window in [false, true] {
        let mut columns = vec![vec![F(91); 1]; 8];
        let mut refs: Vec<_> = columns.iter_mut().map(Vec::as_mut_slice).collect();
        let row = |_| {
            let mut row = row_v1();
            row.base[BASE_ACTIVE] = F(2);
            row.base[BASE_NUMERIC_SOURCE] = F(2);
            let (role, consumer, _) = output_role_aux_column_descriptor_v1(260).unwrap();
            row.fixed[output_role_fixed_selector_column_v1(role, consumer)] = F::ONE;
            Ok(row)
        };
        let result = if window {
            fill_with_v1(1, 260, &mut refs, der_v1(), challenges_v1(), &centers, row)
        } else {
            super::fill_with_v1(1, 260, &mut refs, der_v1(), challenges_v1(), &centers, row)
        };
        assert_eq!(result, Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim));
        assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
    }
}
