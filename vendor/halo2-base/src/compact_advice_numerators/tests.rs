//! Exact-value, storage-accounting and mutation checks for numerator segments.

use super::*;
use crate::ff::Field as _;
use crate::halo2_proofs::{
    halo2curves::pasta::{Fp, Fq},
    plonk::Assigned,
};
use crate::virtual_region::copy_constraints::SharedCopyConstraintManager;
use crate::{CompactAdvice, Context, FIRST_PHASE_CELL_TYPE_ID};

fn assert_assigned<F: ScalarField>(actual: Assigned<F>, expected: Assigned<F>) {
    match (actual, expected) {
        (Assigned::Zero, Assigned::Zero) => {}
        (Assigned::Trivial(a), Assigned::Trivial(b)) => assert_eq!(a, b),
        (Assigned::Rational(a, b), Assigned::Rational(c, d)) => {
            assert_eq!(a, c);
            assert_eq!(b, d);
        }
        (actual, expected) => panic!("exact Assigned variant changed: {actual:?} != {expected:?}"),
    }
}

fn matrix<F: ScalarField>() -> Vec<Assigned<F>> {
    let values = [F::ZERO, F::ONE, F::from(17)];
    let mut result = vec![Assigned::Zero];
    result.extend(values.map(Assigned::Trivial));
    for numerator in values {
        for denominator in values {
            result.push(Assigned::Rational(numerator, denominator));
        }
    }
    result
}

fn check_sequence<F: ScalarField>(advice: &CompactAdvice<F>, expected: &[Assigned<F>]) {
    assert_eq!(advice.len(), expected.len());
    let mut iter = advice.iter();
    for (index, expected) in expected.iter().copied().enumerate() {
        assert_eq!(iter.len(), advice.len() - index);
        assert_assigned(advice.get(index).unwrap(), expected);
        assert_assigned(iter.next().unwrap(), expected);
    }
    assert_eq!(iter.size_hint(), (0, Some(0)));
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
    assert!(advice.get(expected.len()).is_none());
    assert!(advice.get(usize::MAX).is_none());
}

fn assert_storage<F: ScalarField>(source: &SegmentedNumerators<F>) {
    let stats = source.storage();
    let mut direct = source.segments.capacity() * size_of::<NumeratorSegment<F>>();
    for segment in &source.segments {
        match segment {
            NumeratorSegment::Dense(dense) => direct += dense.values.capacity() * size_of::<F>(),
            NumeratorSegment::Tagged(tagged) => {
                direct += tagged.tags.capacity()
                    + tagged.rank.capacity() * size_of::<u16>()
                    + tagged.others.capacity() * size_of::<F>();
                assert!(tagged.others.capacity() < COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN);
            }
        }
    }
    assert_eq!(stats.owned_capacity_bytes, direct);
    assert_eq!(
        stats.owned_capacity_bytes,
        stats.field_slots_capacity.iter().sum::<usize>() * size_of::<F>()
            + stats.tag_bytes[1]
            + stats.rank_bytes[1]
            + stats.segment_header_capacity_bytes
    );
    assert_eq!(
        stats.segment_counts.iter().sum::<usize>(),
        source.segment_count()
    );
    for index in 0..2 {
        assert!(stats.field_slots_len[index] <= stats.field_slots_capacity[index]);
    }
    assert!(stats.tag_bytes[0] <= stats.tag_bytes[1]);
    assert!(stats.rank_bytes[0] <= stats.rank_bytes[1]);
}

fn differential<F: ScalarField>() {
    let pattern = matrix::<F>();
    let n = COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN;
    let expected = (0..2 * n + 9)
        .map(|index| pattern[index % pattern.len()])
        .collect::<Vec<_>>();
    let mut advice = CompactAdvice::default();
    for value in expected.iter().copied() {
        advice.push(value);
    }
    assert_eq!(advice.numerators.storage().segment_counts, [1, 2]);
    assert!(advice.capacities()[0] < 3 * n);
    assert_storage(&advice.numerators);
    check_sequence(&advice, &expected);
    let mut state = 0xd1b5_4a32_d192_ed03_u64;
    for _ in 0..4096 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let index = (state % expected.len() as u64) as usize;
        assert_assigned(advice.get(index).unwrap(), expected[index]);
    }
    for index in [0, 3, 4, 255, 256, 257, n - 1, n, n + 255, 2 * n - 1, 2 * n] {
        assert_assigned(advice.get(index).unwrap(), expected[index]);
    }
}

#[test]
fn pasta_fp_exact_assigned_matrix_and_historical_reads() {
    differential::<Fp>();
}
#[test]
fn pasta_fq_exact_assigned_matrix_and_historical_reads() {
    differential::<Fq>();
}

fn replacement_clone_wipe<F: ScalarField>() {
    let n = COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN;
    let pattern = matrix::<F>();
    let mut expected = (0..n + 5)
        .map(|index| pattern[index % pattern.len()])
        .collect::<Vec<_>>();
    let mut original = CompactAdvice::default();
    for value in expected.iter().copied() {
        original.push(value);
    }
    let original_stats = original.numerators.storage();
    let mut cloned = original.clone();
    assert_eq!(cloned.capacities()[0], original.capacities()[0]);
    assert_eq!(
        cloned.numerators.storage().owned_capacity_bytes,
        original_stats.owned_capacity_bytes
    );
    for (index, value) in [
        (4, F::ONE),
        (5, F::ZERO),
        (6, F::from(19)),
        (n - 1, F::ONE),
        (n, F::ZERO),
        (0, F::from(23)),
        (0, F::ZERO),
        (0, F::ONE),
    ] {
        cloned.replace_with_trivial(index, value);
        expected[index] = Assigned::Trivial(value);
    }
    assert_eq!(cloned.numerators.storage().segment_counts, [2, 0]);
    assert!(
        cloned
            .numerators
            .storage()
            .max_transition_owned_capacity_bytes
            >= original_stats.owned_capacity_bytes + n * size_of::<F>()
    );
    check_sequence(&cloned, &expected);
    for index in 0..original.len() {
        assert_assigned(original.get(index).unwrap(), pattern[index % pattern.len()]);
    }

    // Wipe a still-tagged owner, not only the expanded mutation clone.
    let capacities = original.capacities();
    let prior = original.numerators.storage();
    original.wipe();
    assert_eq!(original.capacities(), capacities);
    let wiped = original.numerators.storage();
    assert_eq!(wiped.owned_capacity_bytes, prior.owned_capacity_bytes);
    assert_eq!(wiped.field_slots_len[1], 0);
    assert_eq!(wiped.max_transition_owned_capacity_bytes, 0);
    for value in original.iter() {
        assert_assigned(value, Assigned::Trivial(F::ZERO));
    }
    let mut wiped_clone = original.clone();
    assert_eq!(wiped_clone.capacities()[0], original.capacities()[0]);
    assert_eq!(wiped_clone.numerators.storage(), wiped);
    // Appending after a wipe retains the original logical history.
    for _ in 0..n {
        wiped_clone.push(Assigned::Trivial(F::ONE));
    }
    for index in 0..n + 5 {
        assert_assigned(wiped_clone.get(index).unwrap(), Assigned::Trivial(F::ZERO));
    }
    for index in n + 5..2 * n + 5 {
        assert_assigned(wiped_clone.get(index).unwrap(), Assigned::Trivial(F::ONE));
    }
    assert_storage(&wiped_clone.numerators);
}

#[test]
fn pasta_fp_replacement_clone_wipe_and_append() {
    replacement_clone_wipe::<Fp>();
}
#[test]
fn pasta_fq_replacement_clone_wipe_and_append() {
    replacement_clone_wipe::<Fq>();
}

#[test]
fn dense_fallback_and_all_zero_one_compression_preserve_capacity_truth() {
    let n = COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN;
    for value in [Fp::ZERO, Fp::ONE, Fp::from(7)] {
        let mut source = SegmentedNumerators::default();
        for _ in 0..n + 1 {
            let prepared = source.prepare_push();
            source.push_prepared(value, prepared);
        }
        assert_eq!(
            source.storage().segment_counts,
            if value == Fp::from(7) { [2, 0] } else { [1, 1] }
        );
        assert_storage(&source);
        if value == Fp::from(7) {
            assert_eq!(source.checked_capacity(), Some(2 * n));
            assert_eq!(source.storage().tag_bytes, [0, 0]);
            assert_eq!(source.storage().rank_bytes, [0, 0]);
        } else {
            assert_eq!(source.storage().field_slots_len, [1, 0]);
            assert_eq!(source.storage().field_slots_capacity, [n, 0]);
            assert_eq!(source.storage().tag_bytes[0], n / 4);
            assert_eq!(source.storage().rank_bytes[0], n / 256 * 2);
        }
        assert!(source.iter().all(|decoded| decoded == value));
    }
}

#[test]
fn full_u16_rank_boundary_uses_usize_payload_count() {
    let n = COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN;
    let mut tagged = TaggedNumerators::with_capacities(n / 4, n / 256, n);
    // The internal encoder supports the full rank range even though the public
    // compression policy retains dense storage for this high-entropy segment.
    let values = (0..n)
        .map(|index| Fq::from(index as u64 + 2))
        .collect::<Vec<_>>();
    tagged.encode_full_segment(&values);
    assert_eq!(tagged.rank.len(), 256);
    assert_eq!(tagged.rank[255], 65280);
    assert_eq!(tagged.others.len(), 65536);
    for index in [0, 3, 4, 255, 256, 65279, 65280, 65534, 65535] {
        assert_eq!(tagged.get(index), values[index]);
    }
    tagged.wipe();
    assert!(tagged.tags.iter().all(|byte| *byte == 0));
    assert!(tagged.rank.iter().all(|rank| *rank == 0));
    assert!(tagged.others.is_empty());
    assert_eq!(tagged.get(65535), Fq::ZERO);
}

#[test]
fn boundary_ticket_accounts_old_candidate_and_next_and_can_be_discarded() {
    let n = COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN;
    let mut source = SegmentedNumerators::<Fp>::default();
    for index in 0..n {
        let prepared = source.prepare_push();
        source.push_prepared(
            if index % 3 == 0 {
                Fp::from(17)
            } else {
                Fp::ONE
            },
            prepared,
        );
    }
    let before = source.storage();
    let prepared = source.prepare_push().unwrap();
    let owned = source.storage().owned_capacity_bytes;
    let extra = prepared
        .previous
        .as_ref()
        .unwrap()
        .checked_capacity_bytes()
        .unwrap()
        + prepared.next.values.capacity() * size_of::<Fp>();
    assert_eq!(
        source.storage().max_transition_owned_capacity_bytes,
        owned + extra
    );
    assert_eq!(source.len(), n);
    assert_eq!(source.storage().segment_counts, [1, 0]);
    assert_eq!(source.storage().field_slots_len, before.field_slots_len);
    drop(prepared);
    // Discarded preparation never changes values or the logical sequence.
    for index in 0..n {
        assert_eq!(
            source.get(index),
            Some(if index % 3 == 0 {
                Fp::from(17)
            } else {
                Fp::ONE
            })
        );
    }
    let prepared = source.prepare_push();
    source.push_prepared(Fp::ZERO, prepared);
    assert_eq!(source.storage().segment_counts, [1, 1]);
    assert_eq!(source.get(n), Some(Fp::ZERO));
    assert_storage(&source);
}

#[test]
fn replacement_preparation_keeps_rational_tables_and_values_intact() {
    let n = COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN;
    let mut advice = CompactAdvice::<Fq>::default();
    for _ in 0..n + 1 {
        advice.push(Assigned::Rational(Fq::ONE, Fq::ZERO));
    }
    let positions = advice.rational_positions.clone();
    let denominators = advice.denominators.clone();
    advice.numerators.prepare_replacement(65535);
    assert_eq!(advice.rational_positions, positions);
    assert_eq!(advice.denominators, denominators);
    assert_assigned(
        advice.get(65535).unwrap(),
        Assigned::Rational(Fq::ONE, Fq::ZERO),
    );
    advice.replace_with_trivial(65535, Fq::from(31));
    assert_eq!(advice.rational_positions.len(), positions.len() - 1);
    assert_eq!(advice.denominators.len(), denominators.len() - 1);
    assert_assigned(advice.get(65535).unwrap(), Assigned::Trivial(Fq::from(31)));
    assert_assigned(
        advice.get(65536).unwrap(),
        Assigned::Rational(Fq::ONE, Fq::ZERO),
    );
}

#[test]
fn context_get_offsets_cells_and_statistics_survive_rollover() {
    let n = COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN;
    let mut context = Context::<Fp>::new(
        false,
        0,
        FIRST_PHASE_CELL_TYPE_ID,
        19,
        SharedCopyConstraintManager::default(),
    );
    for index in 0..n + 1 {
        context.load_witness(if index % 2 == 0 { Fp::ZERO } else { Fp::ONE });
    }
    assert_eq!(context.advice_numerator_storage().segment_counts, [1, 1]);
    for index in [0, 255, 256, 65535, 65536] {
        let value = context.get(index as isize);
        assert_assigned(
            value.value,
            Assigned::Trivial(if index % 2 == 0 { Fp::ZERO } else { Fp::ONE }),
        );
        let cell = value.cell.unwrap();
        assert_eq!(cell.offset(), index);
        assert_eq!(cell.context_id(), 19);
        assert_eq!(cell.type_id(), FIRST_PHASE_CELL_TYPE_ID);
    }
    assert_eq!(context.get(-1).cell, context.get(n as isize).cell);
    assert_eq!(context.advice_len(), n + 1);
    assert_eq!(
        context.advice_storage_capacities()[0],
        context
            .advice_numerator_storage()
            .field_slots_capacity
            .iter()
            .sum::<usize>()
    );
}

#[test]
fn actual_capacity_guard_rejects_oversized_tagged_buffer() {
    let n = COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN;
    let dense_bytes = n * size_of::<Fp>();
    let candidate = TaggedNumerators::<Fp>::with_capacities(n / 4, n / 256, 0);
    assert!(candidate.fits_dense_capacity(dense_bytes));
    // Deliberately oversized test-only allocation models a capacity larger
    // than the compressed request. Production requests only OTHER count.
    let oversized = TaggedNumerators::<Fp>::with_capacities(n / 4, n / 256, n);
    assert!(!oversized.fits_dense_capacity(dense_bytes));
    assert!(oversized.checked_capacity_bytes().unwrap() > dense_bytes);
}

#[test]
fn outer_header_growth_bound_counts_old_and_new_capacity_separately() {
    let n = COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN;
    let mut source = SegmentedNumerators::<Fp>::default();
    let prepared = source.prepare_push();
    source.push_prepared(Fp::ZERO, prepared);
    let header_capacity = source.segments.capacity();
    for _ in 1..header_capacity * n {
        let prepared = source.prepare_push();
        source.push_prepared(Fp::ZERO, prepared);
    }
    assert_eq!(source.segments.len(), header_capacity);
    let old_header_bytes = header_capacity * size_of::<NumeratorSegment<Fp>>();
    let prepared = source.prepare_push().unwrap();
    let stats = source.storage();
    assert!(source.segments.capacity() > header_capacity);
    assert_eq!(
        stats.max_outer_growth_capacity_upper_bound_bytes,
        stats.owned_capacity_bytes + old_header_bytes
    );
    assert_eq!(
        stats.max_transition_owned_capacity_bytes,
        stats.owned_capacity_bytes
            + prepared
                .previous
                .as_ref()
                .unwrap()
                .checked_capacity_bytes()
                .unwrap()
            + prepared.next.values.capacity() * size_of::<Fp>()
    );
    drop(prepared);
    assert_storage(&source);
}

#[test]
fn portable_rank_words_match_byte_reference_for_every_prefix() {
    let valid = (0_u16..=255)
        .map(|byte| byte as u8)
        .filter(|byte| (0..4).all(|pair| ((byte >> (pair * 2)) & 3) != 3))
        .collect::<Vec<_>>();
    for start in 0..valid.len() {
        let bytes = (0..64)
            .map(|index| valid[(start + index) % valid.len()])
            .collect::<Vec<_>>();
        for prefix in 0..=63 {
            let expected = bytes[..prefix]
                .iter()
                .map(|byte| {
                    (0..4)
                        .filter(|pair| ((byte >> (pair * 2)) & 3) == OTHER_TAG)
                        .count()
                })
                .sum::<usize>();
            assert_eq!(count_other_tags(&bytes[..prefix]), expected);
        }
    }
}

#[test]
fn volatile_erasure_precedes_discard_replacement_unwind_and_drop() {
    let n = COMPACT_ADVICE_NUMERATOR_SEGMENT_LEN;
    let mut advice = CompactAdvice::<Fp>::default();
    for index in 0..n {
        advice.push(Assigned::Rational(
            if index % 3 == 0 {
                Fp::from(17)
            } else {
                Fp::ONE
            },
            Fp::from(19),
        ));
    }
    let positions = advice.rational_positions.clone();
    let denominators = advice.denominators.clone();
    DROP_OBSERVATIONS.with(|observer| *observer.borrow_mut() = Some(Vec::new()));
    struct ObserverGuard;
    impl Drop for ObserverGuard {
        fn drop(&mut self) {
            let _ = DROP_OBSERVATIONS.try_with(|observer| *observer.borrow_mut() = None);
        }
    }
    let _observer_guard = ObserverGuard;
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _prepared = advice.numerators.prepare_push().unwrap();
        panic!("test-only failure after all numerator preparation");
    }));
    assert!(unwind.is_err());
    assert_eq!(advice.len(), n);
    assert_eq!(advice.rational_positions, positions);
    assert_eq!(advice.denominators, denominators);
    for index in 0..n {
        assert_assigned(
            advice.get(index).unwrap(),
            Assigned::Rational(
                if index % 3 == 0 {
                    Fp::from(17)
                } else {
                    Fp::ONE
                },
                Fp::from(19),
            ),
        );
    }
    let unwind_events = DROP_OBSERVATIONS.with(|observer| {
        observer
            .borrow_mut()
            .as_mut()
            .unwrap()
            .drain(..)
            .collect::<Vec<_>>()
    });
    assert!(unwind_events.iter().all(|(_, _, erased)| *erased));
    for kind in ["tags", "rank", "others"] {
        assert!(
            unwind_events
                .iter()
                .any(|(observed, len, _)| *observed == kind && *len > 0)
        );
    }
    assert!(
        unwind_events
            .iter()
            .any(|(kind, len, _)| *kind == "dense" && *len == 0)
    );

    // Committing compression discards a populated dense owner.
    advice.push(Assigned::Zero);
    let compressed_events = DROP_OBSERVATIONS.with(|observer| {
        observer
            .borrow_mut()
            .as_mut()
            .unwrap()
            .drain(..)
            .collect::<Vec<_>>()
    });
    assert!(
        compressed_events
            .iter()
            .any(|(kind, len, erased)| *kind == "dense" && *len == n && *erased)
    );
    // Replacing a historical cell discards a populated tagged owner.
    advice.replace_with_trivial(5, Fp::from(23));
    let replacement_events = DROP_OBSERVATIONS.with(|observer| {
        observer
            .borrow_mut()
            .as_mut()
            .unwrap()
            .drain(..)
            .collect::<Vec<_>>()
    });
    for kind in ["tags", "rank", "others"] {
        assert!(
            replacement_events
                .iter()
                .any(|(observed, len, erased)| *observed == kind && *len > 0 && *erased)
        );
    }
    drop(advice);
    let final_events = DROP_OBSERVATIONS.with(|observer| observer.borrow_mut().take().unwrap());
    assert!(
        final_events
            .iter()
            .any(|(kind, len, erased)| *kind == "dense" && *len == n && *erased)
    );
    assert!(final_events.iter().all(|(_, _, erased)| *erased));
}
