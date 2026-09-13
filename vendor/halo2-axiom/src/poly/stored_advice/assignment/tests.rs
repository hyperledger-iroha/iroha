//! Discard-only assignment equivalence, admission and partial-failure tests.
//!
//! The recording backend deliberately retains plaintext only as a test oracle. These tests do
//! not replace the Core backend's encryption, authentication or filesystem lifecycle tests.

use std::{cell::RefCell, rc::Rc};

use ff::Field;

use super::*;
use crate::poly::batch_invert_assigned;

#[derive(Default)]
struct Recording {
    chunks: Vec<Vec<[u8; 32]>>,
    failure_chunk: Option<u64>,
    panic_chunk: Option<u64>,
    seal_error: bool,
    changed_writer_layout: Option<StoredAdviceLayoutV1>,
    changed_snapshot_layout: Option<StoredAdviceLayoutV1>,
    writer_drops: usize,
    snapshot_drops: usize,
    seals: usize,
}

struct Writer {
    layout: StoredAdviceLayoutV1,
    recording: Rc<RefCell<Recording>>,
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.recording.borrow_mut().writer_drops += 1;
    }
}

struct Snapshot {
    layout: StoredAdviceLayoutV1,
    recording: Rc<RefCell<Recording>>,
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        self.recording.borrow_mut().snapshot_drops += 1;
    }
}

impl StoredAdviceWriterV1 for Writer {
    type Snapshot = Snapshot;

    fn layout(&self) -> StoredAdviceLayoutV1 {
        self.recording
            .borrow()
            .changed_writer_layout
            .unwrap_or(self.layout)
    }

    fn write_chunk(&mut self, chunk: u64, scalars: &[[u8; 32]]) -> Result<(), StoredAdviceErrorV1> {
        let mut recording = self.recording.borrow_mut();
        assert_ne!(recording.panic_chunk, Some(chunk), "injected write unwind");
        if recording.failure_chunk == Some(chunk) {
            return Err(StoredAdviceErrorV1::Storage);
        }
        assert_eq!(chunk as usize, recording.chunks.len());
        assert_eq!(
            scalars.len(),
            self.layout.chunk_scalar_count(chunk).unwrap()
        );
        assert!(
            scalars
                .iter()
                .all(|scalar| self.layout.field().is_canonical(scalar))
        );
        recording.chunks.push(scalars.to_vec());
        Ok(())
    }

    fn seal(self) -> Result<Snapshot, StoredAdviceErrorV1> {
        let mut recording = self.recording.borrow_mut();
        recording.seals += 1;
        if recording.seal_error {
            return Err(StoredAdviceErrorV1::Storage);
        }
        assert_eq!(recording.chunks.len(), self.layout.chunk_count());
        Ok(Snapshot {
            layout: recording.changed_snapshot_layout.unwrap_or(self.layout),
            recording: Rc::clone(&self.recording),
        })
    }
}

impl StoredAdviceSnapshotV1 for Snapshot {
    fn layout(&self) -> StoredAdviceLayoutV1 {
        self.layout
    }

    fn with_chunk<R>(
        &mut self,
        expected: StoredAdviceLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredAdviceErrorV1>,
    ) -> Result<R, StoredAdviceErrorV1> {
        if expected != self.layout {
            return Err(StoredAdviceErrorV1::Context);
        }
        let recording = self.recording.borrow();
        consume(
            recording
                .chunks
                .get(chunk as usize)
                .ok_or(StoredAdviceErrorV1::ChunkIndex)?,
        )
    }

    fn with_column<R>(
        &mut self,
        expected: StoredAdviceLayoutV1,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredAdviceErrorV1>,
    ) -> Result<R, StoredAdviceErrorV1> {
        if expected != self.layout {
            return Err(StoredAdviceErrorV1::Context);
        }
        let values = self
            .recording
            .borrow()
            .chunks
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        consume(&values)
    }
}

fn layout<F: StoredAssignmentFieldV1>(k: u32, column: u32) -> StoredAdviceLayoutV1 {
    StoredAdviceLayoutV1::new(
        [31; 32],
        column as u64,
        F::STORED_FIELD,
        StoredPolynomialBasisV1::Lagrange,
        k,
        column,
        1,
    )
    .unwrap()
}

fn writer(layout: StoredAdviceLayoutV1) -> (Writer, Rc<RefCell<Recording>>) {
    let recording = Rc::new(RefCell::new(Recording::default()));
    (
        Writer {
            layout,
            recording: Rc::clone(&recording),
        },
        recording,
    )
}

fn make_column<F: StoredAssignmentFieldV1>(
    k: u32,
    index: u32,
    usable: usize,
) -> (StoredAdviceAssignmentV1<F, Writer>, Rc<RefCell<Recording>>) {
    let layout = layout::<F>(k, index);
    let (writer, recording) = writer(layout);
    (
        StoredAdviceAssignmentV1::new(writer, layout, usable).unwrap(),
        recording,
    )
}

fn interleaved_columns_match_borrowed<F: StoredAssignmentFieldV1>() {
    let usable = 1017;
    let (mut left, left_recording) = make_column::<F>(10, 4, usable);
    let (mut right, right_recording) = make_column::<F>(10, 8, usable);
    let mut expected = [vec![Assigned::Zero; 1024], vec![Assigned::Zero; 1024]];
    // Alternating columns, same-chunk gaps, exact boundaries, complete skipped chunks,
    // denominator zero, and nontrivial denominators all share one bounded flush path.
    for (index, row, value) in [
        (0, 0, Assigned::Trivial(F::from(19))),
        (1, 2, Assigned::Rational(F::from(25), F::from(5))),
        (0, 254, Assigned::Rational(F::from(33), F::from(3))),
        (1, 255, Assigned::Rational(F::from(91), F::ZERO)),
        (0, 256, Assigned::Zero),
        (1, 256, Assigned::Rational(F::ZERO, F::from(7))),
        (0, 777, Assigned::Rational(F::from(65), F::from(13))),
        (1, 1000, Assigned::Trivial(F::from(43))),
    ] {
        let destination = if index == 0 { &mut left } else { &mut right };
        destination.assign_discarding_value(row, value).unwrap();
        expected[index][row] = value;
        let active = destination.active.as_ref().unwrap();
        assert_eq!(active.numerators.0.len(), STORED_SCALARS_PER_CHUNK_V1);
        assert_eq!(active.denominators.0.len(), STORED_SCALARS_PER_CHUNK_V1);
        assert!(active.next_row - active.next_chunk as usize * STORED_SCALARS_PER_CHUNK_V1 < 256);
    }
    let mut tail_calls = Vec::new();
    let mut snapshots = Vec::new();
    for (index, column) in [left, right].into_iter().enumerate() {
        snapshots.push(
            column
                .finish_with_tail(|row| {
                    tail_calls.push((index, row));
                    let value = F::from((index * 1024 + row + 100) as u64);
                    expected[index][row] = Assigned::Trivial(value);
                    Ok(value)
                })
                .unwrap(),
        );
    }
    assert_eq!(
        tail_calls,
        (0..2)
            .flat_map(|index| (usable..1024).map(move |row| (index, row)))
            .collect::<Vec<_>>()
    );
    let borrowed = batch_invert_assigned(expected.iter().map(Vec::as_slice).collect());
    for (index, snapshot) in snapshots.iter_mut().enumerate() {
        let expected_layout = layout::<F>(10, if index == 0 { 4 } else { 8 });
        snapshot
            .with_column(expected_layout, |scalars| {
                assert_eq!(
                    scalars,
                    borrowed[index]
                        .iter()
                        .map(|scalar| scalar.to_repr())
                        .collect::<Vec<_>>()
                );
                Ok(())
            })
            .unwrap();
    }
    for recording in [left_recording, right_recording] {
        assert_eq!(recording.borrow().chunks.len(), 4);
        assert_eq!(recording.borrow().seals, 1);
    }
}

#[test]
fn interleaved_assignment_matches_existing_borrowed_inversion_in_both_pasta_fields() {
    interleaved_columns_match_borrowed::<Fp>();
    interleaved_columns_match_borrowed::<Fq>();
}

#[test]
fn empty_and_subchunk_domains_emit_exact_logical_lengths_and_zero_gaps() {
    for k in 0..=8 {
        let n = 1_usize << k;
        for usable in [0, n / 2, n] {
            let (column, recording) = make_column::<Fp>(k, 0, usable);
            let mut calls = Vec::new();
            let _snapshot = column
                .finish_with_tail(|row| {
                    calls.push(row);
                    Ok(Fp::from((row + 1) as u64))
                })
                .unwrap();
            assert_eq!(calls, (usable..n).collect::<Vec<_>>());
            let recording = recording.borrow();
            assert_eq!(recording.chunks.len(), 1);
            assert_eq!(recording.chunks[0].len(), n);
            for (row, bytes) in recording.chunks[0].iter().enumerate() {
                let expected = if row < usable {
                    Fp::ZERO
                } else {
                    Fp::from((row + 1) as u64)
                };
                assert_eq!(*bytes, expected.to_repr());
            }
        }
    }
}

#[test]
fn admission_rejects_layout_field_basis_and_usable_range_substitution() {
    let expected = layout::<Fp>(10, 5);
    for actual in [
        layout::<Fp>(9, 5),
        layout::<Fp>(10, 6),
        layout::<Fq>(10, 5),
        StoredAdviceLayoutV1::new(
            [32; 32],
            5,
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            10,
            5,
            1,
        )
        .unwrap(),
        StoredAdviceLayoutV1::new(
            [31; 32],
            6,
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            10,
            5,
            1,
        )
        .unwrap(),
        StoredAdviceLayoutV1::new(
            [31; 32],
            5,
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Lagrange,
            10,
            5,
            2,
        )
        .unwrap(),
    ] {
        let (writer, recording) = writer(actual);
        assert!(matches!(
            StoredAdviceAssignmentV1::<Fp, _>::new(writer, expected, 1000),
            Err(StoredAssignmentErrorV1::Store(StoredAdviceErrorV1::Context))
        ));
        assert_eq!(recording.borrow().writer_drops, 1);
    }
    for actual in [
        layout::<Fq>(10, 5),
        StoredAdviceLayoutV1::new(
            [31; 32],
            5,
            StoredPastaFieldV1::Fp,
            StoredPolynomialBasisV1::Coefficient,
            10,
            5,
            1,
        )
        .unwrap(),
    ] {
        let (writer, _) = writer(actual);
        assert!(matches!(
            StoredAdviceAssignmentV1::<Fp, _>::new(writer, actual, 1000),
            Err(StoredAssignmentErrorV1::Store(StoredAdviceErrorV1::Context))
        ));
    }
    let (writer, _) = writer(expected);
    assert!(matches!(
        StoredAdviceAssignmentV1::<Fp, _>::new(writer, expected, 1025),
        Err(StoredAssignmentErrorV1::Store(StoredAdviceErrorV1::Layout))
    ));
}

#[test]
fn duplicate_backward_reserved_tail_and_reference_requests_never_downgrade() {
    let (mut column, recording) = make_column::<Fp>(9, 0, 508);
    column
        .assign_discarding_value(255, Assigned::Trivial(Fp::from(7)))
        .unwrap();
    assert_eq!(recording.borrow().chunks.len(), 1);
    for row in [255, 254, 0] {
        assert_eq!(
            column.assign_discarding_value(row, Assigned::Trivial(Fp::from(99))),
            Err(StoredAssignmentErrorV1::NonMonotonic)
        );
    }
    for row in [508, 512, usize::MAX] {
        assert_eq!(
            column.assign_discarding_value(row, Assigned::Zero),
            Err(StoredAssignmentErrorV1::Row)
        );
    }
    assert_eq!(
        column.assign_returning_reference(256, Assigned::Trivial(Fp::from(99))),
        Err(StoredAssignmentErrorV1::ReferenceReturn)
    );
    assert_eq!(column.active.as_ref().unwrap().next_row, 256);
    assert_eq!(recording.borrow().chunks.len(), 1);
    column
        .assign_discarding_value(256, Assigned::Trivial(Fp::from(11)))
        .unwrap();
    let _snapshot = column.finish_with_tail(|_| Ok(Fp::ZERO)).unwrap();
    assert_eq!(recording.borrow().chunks[0][255], Fp::from(7).to_repr());
    assert_eq!(recording.borrow().chunks[1][0], Fp::from(11).to_repr());
}

#[test]
fn late_gap_write_failure_destroys_owner_and_cannot_seal_or_retry() {
    let (mut column, recording) = make_column::<Fp>(10, 0, 1020);
    recording.borrow_mut().failure_chunk = Some(2);
    column
        .assign_discarding_value(0, Assigned::Trivial(Fp::from(5)))
        .unwrap();
    assert_eq!(
        column.assign_discarding_value(900, Assigned::Trivial(Fp::from(6))),
        Err(StoredAssignmentErrorV1::Store(StoredAdviceErrorV1::Storage))
    );
    assert!(column.active.is_none());
    assert_eq!(recording.borrow().chunks.len(), 2);
    assert_eq!(recording.borrow().writer_drops, 1);
    assert_eq!(
        column.assign_discarding_value(901, Assigned::Zero),
        Err(StoredAssignmentErrorV1::Poisoned)
    );
    assert_eq!(
        column.assign_returning_reference(901, Assigned::Zero),
        Err(StoredAssignmentErrorV1::Poisoned)
    );
    assert!(matches!(
        column.finish_with_tail(|_| panic!("tail must not run")),
        Err(StoredAssignmentErrorV1::Poisoned)
    ));
    assert_eq!(recording.borrow().seals, 0);
}

#[test]
fn caught_storage_unwind_leaves_no_reusable_owner() {
    let (mut column, recording) = make_column::<Fq>(10, 0, 1020);
    recording.borrow_mut().panic_chunk = Some(1);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        column
            .assign_discarding_value(700, Assigned::Trivial(Fq::from(7)))
            .unwrap();
    }));
    assert!(result.is_err());
    assert!(column.active.is_none());
    assert_eq!(recording.borrow().chunks.len(), 1);
    assert_eq!(recording.borrow().writer_drops, 1);
    assert_eq!(
        column.assign_discarding_value(701, Assigned::Zero),
        Err(StoredAssignmentErrorV1::Poisoned)
    );
}

#[test]
fn tail_failure_and_seal_failure_return_no_snapshot() {
    let (column, recording) = make_column::<Fp>(9, 0, 508);
    let mut rows = Vec::new();
    let result = column.finish_with_tail(|row| {
        rows.push(row);
        if row == 510 {
            Err(StoredAdviceErrorV1::Backend.into())
        } else {
            Ok(Fp::from(12))
        }
    });
    assert!(matches!(
        result,
        Err(StoredAssignmentErrorV1::Store(StoredAdviceErrorV1::Backend))
    ));
    assert_eq!(rows, [508, 509, 510]);
    assert_eq!(recording.borrow().chunks.len(), 1);
    assert_eq!(recording.borrow().writer_drops, 1);
    assert_eq!(recording.borrow().seals, 0);
    let (column, recording) = make_column::<Fp>(8, 0, 250);
    recording.borrow_mut().seal_error = true;
    assert!(matches!(
        column.finish_with_tail(|_| Ok(Fp::ONE)),
        Err(StoredAssignmentErrorV1::Store(StoredAdviceErrorV1::Storage))
    ));
    assert_eq!(recording.borrow().writer_drops, 1);
    assert_eq!(recording.borrow().snapshot_drops, 0);
}

#[test]
fn backend_identity_changes_are_rejected_before_write_and_after_seal() {
    let (mut column, recording) = make_column::<Fp>(8, 0, 250);
    recording.borrow_mut().changed_writer_layout = Some(layout::<Fp>(8, 1));
    assert_eq!(column.layout(), layout::<Fp>(8, 0));
    // A buffered assignment is not an authenticated publication. Flushing must detect the
    // changed backend identity before a single write can be accepted.
    column
        .assign_discarding_value(0, Assigned::Trivial(Fp::ONE))
        .unwrap();
    assert!(matches!(
        column.finish_with_tail(|_| Ok(Fp::ONE)),
        Err(StoredAssignmentErrorV1::Store(StoredAdviceErrorV1::Context))
    ));
    assert!(recording.borrow().chunks.is_empty());
    assert_eq!(recording.borrow().writer_drops, 1);
    let (column, recording) = make_column::<Fp>(8, 0, 250);
    recording.borrow_mut().changed_snapshot_layout = Some(layout::<Fp>(8, 1));
    assert!(matches!(
        column.finish_with_tail(|_| Ok(Fp::ONE)),
        Err(StoredAssignmentErrorV1::Store(StoredAdviceErrorV1::Context))
    ));
    assert_eq!(recording.borrow().snapshot_drops, 1);
}

#[test]
fn successful_flush_clears_owned_field_slots_and_debug_exposes_no_values() {
    let (mut column, _) = make_column::<Fp>(9, 0, 508);
    column
        .assign_discarding_value(255, Assigned::Rational(Fp::from(71), Fp::from(13)))
        .unwrap();
    let active = column.active.as_ref().unwrap();
    assert!(active.numerators.0.iter().all(|value| *value == Fp::ZERO));
    assert!(active.denominators.0.iter().all(|value| *value == Fp::ZERO));
    let debug = format!("{column:?}");
    assert!(debug.contains("StoredAdviceAssignmentV1"));
    assert!(!debug.contains("numerator"));
    assert!(!debug.contains("denominator"));
    assert!(!debug.contains("recording"));
}
