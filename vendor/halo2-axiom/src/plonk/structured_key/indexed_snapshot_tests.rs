//! Indexed coefficient snapshots against the frozen original dense structured decoder.
//!
//! The writer records plaintext only for test assertions. Its padding and drop observations
//! model the trait contract; they do not qualify Core authentication, key provenance, storage
//! cleanup, process RSS, or production proof admission.

use super::super::indexed::{reads::IndexedKeyPolynomialV1 as PolynomialId, snapshot};
use super::*;
use crate::poly::stored_advice::{
    STORED_SCALARS_PER_CHUNK_V1, StoredKeyMaskV1, StoredPastaFieldV1,
    StoredPolynomialBasisV1 as Basis, StoredPolynomialErrorV1 as StoreError,
    StoredPolynomialLayoutV1 as Layout, StoredPolynomialRoleV1 as Role, StoredPolynomialSnapshotV1,
    StoredPolynomialWriterV1, assignment::StoredAssignmentFieldV1,
};
use std::{
    cell::RefCell,
    io::{Cursor, Seek, SeekFrom},
    rc::Rc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    None,
    ReadError(usize, io::ErrorKind),
    ReadPanic(usize),
    SeekError,
    SeekPanic,
    WriterLayout(usize),
    DriftOnRead,
    WriteError(u64),
    WritePanic(u64),
    DriftAfterWrite(u64),
    SealError,
    SealPanic,
    DriftOnSeal,
    SnapshotLayoutPanic,
}

struct Record {
    layout: Layout,
    fault: Fault,
    writer_layouts: usize,
    reads: usize,
    seeks: usize,
    delivered: usize,
    largest_request: usize,
    first_byte: Option<u64>,
    last_byte: Option<u64>,
    writes: Vec<(u64, Vec<[u8; 32]>)>,
    padded: Vec<Vec<[u8; 32]>>,
    seals: usize,
    writer_drops: usize,
    snapshot_drops: usize,
}
impl Record {
    fn shared(layout: Layout, fault: Fault) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            layout,
            fault,
            writer_layouts: 0,
            reads: 0,
            seeks: 0,
            delivered: 0,
            largest_request: 0,
            first_byte: None,
            last_byte: None,
            writes: vec![],
            padded: vec![],
            seals: 0,
            writer_drops: 0,
            snapshot_drops: 0,
        }))
    }
    fn no_io(&self) {
        assert_eq!((self.reads, self.seeks, self.delivered), (0, 0, 0));
        assert!(self.writes.is_empty());
        assert_eq!(self.seals, 0);
    }
}
fn changed(layout: Layout) -> Layout {
    Layout::new(
        [91; 32],
        layout.ordinal() + 1,
        layout.field(),
        layout.basis(),
        layout.k(),
        layout.role(),
    )
    .unwrap()
}

struct Source<'a> {
    cursor: Cursor<&'a [u8]>,
    record: Rc<RefCell<Record>>,
}
impl<'a> Source<'a> {
    fn new(bytes: &'a [u8], record: &Rc<RefCell<Record>>) -> Self {
        let mut cursor = Cursor::new(bytes);
        cursor.set_position(13);
        Self {
            cursor,
            record: Rc::clone(record),
        }
    }
}
impl Read for Source<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let mut r = self.record.borrow_mut();
        r.reads += 1;
        r.largest_request = r.largest_request.max(output.len());
        if r.fault == Fault::DriftOnRead {
            r.layout = changed(r.layout);
        }
        let boundary = match r.fault {
            Fault::ReadError(at, _) | Fault::ReadPanic(at) => Some(at),
            _ => None,
        };
        if boundary == Some(r.delivered) {
            match r.fault {
                Fault::ReadError(_, kind) => {
                    return Err(io::Error::new(kind, "injected source read"));
                }
                Fault::ReadPanic(_) => panic!("injected source read unwind"),
                _ => unreachable!(),
            }
        }
        let mut count = output.len();
        if let Some(at) = boundary {
            count = count.min(at - r.delivered);
        }
        let before = self.cursor.position();
        let actual = self.cursor.read(&mut output[..count])?;
        if actual != 0 {
            r.first_byte.get_or_insert(before);
            r.last_byte = Some(before + actual as u64);
        }
        r.delivered += actual;
        Ok(actual)
    }
}
impl Seek for Source<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let mut r = self.record.borrow_mut();
        r.seeks += 1;
        match r.fault {
            Fault::SeekError => Err(io::Error::other("injected source seek")),
            Fault::SeekPanic => panic!("injected source seek unwind"),
            _ => self.cursor.seek(position),
        }
    }
}

struct Writer {
    record: Rc<RefCell<Record>>,
}
struct Snapshot {
    layout: Layout,
    record: Rc<RefCell<Record>>,
}
impl Drop for Writer {
    fn drop(&mut self) {
        self.record.borrow_mut().writer_drops += 1;
    }
}
impl Drop for Snapshot {
    fn drop(&mut self) {
        self.record.borrow_mut().snapshot_drops += 1;
    }
}
impl StoredPolynomialWriterV1 for Writer {
    type Snapshot = Snapshot;
    fn layout(&self) -> Layout {
        let mut r = self.record.borrow_mut();
        let call = r.writer_layouts;
        r.writer_layouts += 1;
        if r.fault == Fault::WriterLayout(call) {
            changed(r.layout)
        } else {
            r.layout
        }
    }
    fn write_chunk(&mut self, chunk: u64, scalars: &[[u8; 32]]) -> Result<(), StoreError> {
        let mut r = self.record.borrow_mut();
        assert_eq!(chunk as usize, r.writes.len());
        assert_eq!(scalars.len(), r.layout.chunk_scalar_count(chunk).unwrap());
        // This mock enforces the same logical-count contract as the real confidential store.
        if r.fault == Fault::WriteError(chunk) {
            return Err(StoreError::Storage);
        }
        assert_ne!(r.fault, Fault::WritePanic(chunk), "injected writer unwind");
        let mut padded = vec![[0; 32]; STORED_SCALARS_PER_CHUNK_V1];
        padded[..scalars.len()].copy_from_slice(scalars);
        r.padded.push(padded);
        r.writes.push((chunk, scalars.to_vec()));
        if r.fault == Fault::DriftAfterWrite(chunk) {
            r.layout = changed(r.layout);
        }
        Ok(())
    }
    fn seal(self) -> Result<Self::Snapshot, StoreError> {
        let mut r = self.record.borrow_mut();
        r.seals += 1;
        assert_eq!(r.writes.len(), r.layout.chunk_count());
        if r.fault == Fault::SealError {
            return Err(StoreError::Storage);
        }
        assert_ne!(r.fault, Fault::SealPanic, "injected seal unwind");
        let layout = if r.fault == Fault::DriftOnSeal {
            changed(r.layout)
        } else {
            r.layout
        };
        drop(r);
        Ok(Snapshot {
            layout,
            record: Rc::clone(&self.record),
        })
    }
}
impl StoredPolynomialSnapshotV1 for Snapshot {
    fn layout(&self) -> Layout {
        assert_ne!(
            self.record.borrow().fault,
            Fault::SnapshotLayoutPanic,
            "injected snapshot-layout unwind"
        );
        self.layout
    }
    fn with_chunk<R>(
        &mut self,
        expected: Layout,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoreError>,
    ) -> Result<R, StoreError> {
        if expected != self.layout {
            return Err(StoreError::Context);
        }
        self.layout.chunk_scalar_count(chunk)?;
        consume(&self.record.borrow().writes[chunk as usize].1)
    }
    fn with_column<R>(
        &mut self,
        _expected: Layout,
        _consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoreError>,
    ) -> Result<R, StoreError> {
        panic!("indexed snapshot adapter must never materialize its output")
    }
}

fn role(polynomial: PolynomialId) -> Role {
    match polynomial {
        PolynomialId::MaskCoefficient(mask) => Role::KeyMask {
            kind: match mask {
                0 => StoredKeyMaskV1::L0,
                1 => StoredKeyMaskV1::LLast,
                2 => StoredKeyMaskV1::LActiveRow,
                _ => unreachable!(),
            },
        },
        PolynomialId::FixedLagrange(column) => Role::KeyFixed {
            column: column as u32,
        },
        PolynomialId::PermutationLagrange(column) => Role::KeyPermutation {
            column: column as u32,
        },
    }
}
fn layout<C: SerdeCurveAffine>(k: u32, polynomial: PolynomialId, ordinal: u64) -> Layout
where
    C::Scalar: StoredAssignmentFieldV1,
{
    Layout::new(
        [37; 32],
        ordinal,
        C::Scalar::STORED_FIELD,
        Basis::Coefficient,
        k,
        role(polynomial),
    )
    .unwrap()
}
fn ids<C: SerdeCurveAffine>(key: &ProvingKey<C>) -> Vec<PolynomialId> {
    (0..3)
        .map(PolynomialId::MaskCoefficient)
        .chain((0..key.fixed_polys.len()).map(PolynomialId::FixedLagrange))
        .chain((0..key.permutation.polys.len()).map(PolynomialId::PermutationLagrange))
        .collect()
}
fn coefficients<C: SerdeCurveAffine>(key: &ProvingKey<C>, id: PolynomialId) -> &[C::Scalar] {
    match id {
        PolynomialId::MaskCoefficient(0) => &key.l0,
        PolynomialId::MaskCoefficient(1) => &key.l_last,
        PolynomialId::MaskCoefficient(2) => &key.l_active_row,
        PolynomialId::MaskCoefficient(_) => unreachable!(),
        PolynomialId::FixedLagrange(column) => &key.fixed_polys[column],
        PolynomialId::PermutationLagrange(column) => &key.permutation.polys[column],
    }
}
fn interval<C: SerdeCurveAffine>(
    key: &IndexedStructuredProvingKeyV1<C>,
    id: PolynomialId,
) -> (u64, usize, usize)
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let m = key.metadata();
    match id {
        PolynomialId::MaskCoefficient(mask) => (m.masks[mask].offset, m.rows * 32, 32),
        PolynomialId::FixedLagrange(column) => {
            let f = &m.fixed[column];
            let unit = if f.mode == 1 { 1 } else { 32 };
            (f.payload.offset, f.payload.length as usize, unit)
        }
        PolynomialId::PermutationLagrange(column) => (
            m.permutation_targets.offset + (column * m.rows * 4) as u64,
            m.rows * 4,
            4,
        ),
    }
}
fn assert_cleanup(rows: usize, allocated: Option<bool>) {
    let c = snapshot::cleanup_observation();
    assert_eq!(c[2], 0, "owned field slots were not cleared");
    assert_eq!(c[5], 0, "owned encoded bytes were not cleared");
    assert_eq!(
        c[0], c[3],
        "both scratch guards must be created before source access"
    );
    assert!(c[0] <= 1);
    if let Some(allocated) = allocated {
        assert_eq!(c[0], usize::from(allocated));
    }
    assert_eq!(c[1], rows * c[0]);
    assert_eq!(c[4], STORED_SCALARS_PER_CHUNK_V1 * 32 * c[3]);
}

fn oracle<C: SerdeCurveAffine, const EMPTY: bool>()
where
    C::Scalar: StoredAssignmentFieldV1 + SerdePrimeField + FromUniformBytes<64>,
{
    for (k, compressed, arbitrary) in [(4, false, false), (4, true, true), (9, true, true)] {
        let (_, bytes) = fixture::<C, EMPTY>(k, compressed, arbitrary);
        let old = old::<C, EMPTY>(&bytes, k, bytes.len() as u64).unwrap();
        let key = index::<C, EMPTY, _, _>(
            &mut bytes.as_slice(),
            k,
            bytes.len() as u64,
            &mut io::sink(),
        )
        .unwrap();
        for (i, id) in ids(&old).into_iter().enumerate() {
            let expected = layout::<C>(k, id, 11 + i as u64);
            let record = Record::shared(expected, Fault::None);
            let mut source = Source::new(&bytes, &record);
            let writer = Writer {
                record: Rc::clone(&record),
            };
            snapshot::reset_cleanup_observation();
            let limit = snapshot::scratch_payload_for_tests::<C::Scalar>(key.rows()).unwrap();
            let mut output = key
                .into_coefficient_snapshot(&mut source, id, writer, expected, limit)
                .unwrap();
            assert_cleanup(key.rows(), Some(true));
            assert_eq!(output.layout(), expected);
            let values: Vec<_> = coefficients(&old, id)
                .iter()
                .map(|v| {
                    let mut bytes = [0; 32];
                    bytes.copy_from_slice(v.to_repr().as_ref());
                    bytes
                })
                .collect();
            for chunk in 0..expected.chunk_count() as u64 {
                output
                    .with_chunk(expected, chunk, |actual| {
                        let start = chunk as usize * STORED_SCALARS_PER_CHUNK_V1;
                        assert_eq!(actual, &values[start..start + actual.len()]);
                        Ok(())
                    })
                    .unwrap();
            }
            let r = record.borrow();
            let (at, bytes, unit) = interval(&key, id);
            assert_eq!((r.seeks, r.delivered, r.reads), (1, bytes, bytes / unit));
            assert_eq!(r.largest_request, unit);
            assert_eq!(
                (r.first_byte, r.last_byte),
                (Some(at), Some(at + bytes as u64))
            );
            assert_eq!(r.writes.len(), expected.chunk_count());
            for ((chunk, logical), padded) in r.writes.iter().zip(&r.padded) {
                assert_eq!(logical.len(), expected.chunk_scalar_count(*chunk).unwrap());
                assert_eq!(&padded[..logical.len()], logical);
                assert!(padded[logical.len()..].iter().all(|v| *v == [0; 32]));
            }
            assert_eq!((r.seals, r.writer_drops, r.snapshot_drops), (1, 1, 0));
            drop(r);
            drop(output);
            assert_eq!(record.borrow().snapshot_drops, 1);
        }
    }
}
#[test]
fn both_pasta_indexed_coefficient_snapshots_match_original_masks_fixed_and_sigma() {
    oracle::<EqAffine, false>();
    oracle::<EpAffine, false>();
    oracle::<EqAffine, true>();
    oracle::<EpAffine, true>();
}

fn invalid<C: SerdeCurveAffine>()
where
    C::Scalar: StoredAssignmentFieldV1 + SerdePrimeField + FromUniformBytes<64>,
{
    let (_, bytes) = fixture::<C, false>(4, true, true);
    let key = index::<C, false, _, _>(
        &mut bytes.as_slice(),
        4,
        bytes.len() as u64,
        &mut io::sink(),
    )
    .unwrap();
    let original = layout::<C>(4, PolynomialId::FixedLagrange(0), 7);
    let minimum = snapshot::scratch_payload_for_tests::<C::Scalar>(key.rows()).unwrap();
    for case in 0..12 {
        let mut expected = original;
        let mut id = PolynomialId::FixedLagrange(0);
        let mut limit = minimum;
        let mut writer_layout = original;
        match case {
            0 => {
                expected = Layout::new(
                    [37; 32],
                    7,
                    original.field(),
                    Basis::Coefficient,
                    4,
                    Role::Advice {
                        column: 0,
                        phase: 0,
                    },
                )
                .unwrap()
            }
            1 => expected = layout::<C>(4, PolynomialId::FixedLagrange(1), 7),
            2 => {
                expected = Layout::new(
                    [37; 32],
                    7,
                    original.field(),
                    Basis::Lagrange,
                    4,
                    original.role(),
                )
                .unwrap()
            }
            3 => {
                expected = Layout::new(
                    [37; 32],
                    7,
                    if original.field() == StoredPastaFieldV1::Fp {
                        StoredPastaFieldV1::Fq
                    } else {
                        StoredPastaFieldV1::Fp
                    },
                    Basis::Coefficient,
                    4,
                    original.role(),
                )
                .unwrap()
            }
            4 => expected = layout::<C>(5, id, 7),
            5 => id = PolynomialId::FixedLagrange(usize::MAX),
            6 => id = PolynomialId::PermutationLagrange(usize::MAX),
            7 => id = PolynomialId::MaskCoefficient(3),
            8 => limit = 0,
            9 => limit -= 1,
            10 => writer_layout = changed(original),
            11 => {
                id = PolynomialId::MaskCoefficient(0);
                expected = layout::<C>(4, PolynomialId::MaskCoefficient(1), 7);
            }
            _ => unreachable!(),
        }
        if case != 10 {
            writer_layout = expected;
        }
        let record = Record::shared(writer_layout, Fault::None);
        let mut source = Source::new(&bytes, &record);
        snapshot::reset_cleanup_observation();
        let result = key.into_coefficient_snapshot(
            &mut source,
            id,
            Writer {
                record: Rc::clone(&record),
            },
            expected,
            limit,
        );
        assert!(result.is_err(), "accepted invalid preflight case {case}");
        if matches!(case, 8 | 9) {
            assert!(matches!(result, Err(StoreError::Allocation)));
        }
        record.borrow().no_io();
        assert_eq!(source.cursor.position(), 13);
        assert_eq!(record.borrow().writer_drops, 1);
        assert_cleanup(key.rows(), Some(false));
    }
}
#[test]
fn both_pasta_indexed_snapshot_invalid_metadata_indices_and_budget_precede_io() {
    invalid::<EqAffine>();
    invalid::<EpAffine>();
}

fn faults<C: SerdeCurveAffine>()
where
    C::Scalar: StoredAssignmentFieldV1 + SerdePrimeField + FromUniformBytes<64>,
{
    let (_, bytes) = fixture::<C, false>(9, true, true);
    let key = index::<C, false, _, _>(
        &mut bytes.as_slice(),
        9,
        bytes.len() as u64,
        &mut io::sink(),
    )
    .unwrap();
    let id = PolynomialId::MaskCoefficient(0);
    let expected = layout::<C>(9, id, 29);
    let limit = snapshot::scratch_payload_for_tests::<C::Scalar>(key.rows()).unwrap();
    let total = key.rows() * 32;
    let baseline = Record::shared(expected, Fault::None);
    let mut source = Source::new(&bytes, &baseline);
    drop(
        key.into_coefficient_snapshot(
            &mut source,
            id,
            Writer {
                record: Rc::clone(&baseline),
            },
            expected,
            limit,
        )
        .unwrap(),
    );
    let layout_calls = baseline.borrow().writer_layouts;
    assert!(layout_calls >= 3);
    let mut cases = vec![
        Fault::SeekError,
        Fault::SeekPanic,
        Fault::DriftOnRead,
        Fault::SealError,
        Fault::SealPanic,
        Fault::DriftOnSeal,
        Fault::SnapshotLayoutPanic,
    ];
    for at in [0, 1, total - 1] {
        cases.push(Fault::ReadError(at, io::ErrorKind::Other));
        cases.push(Fault::ReadError(at, io::ErrorKind::InvalidData));
        cases.push(Fault::ReadError(at, io::ErrorKind::OutOfMemory));
        cases.push(Fault::ReadError(at, io::ErrorKind::UnexpectedEof));
        cases.push(Fault::ReadPanic(at));
    }
    for chunk in 0..expected.chunk_count() as u64 {
        cases.extend([
            Fault::WriteError(chunk),
            Fault::WritePanic(chunk),
            Fault::DriftAfterWrite(chunk),
        ]);
    }
    cases.extend((0..layout_calls).map(Fault::WriterLayout));
    for fault in cases {
        let record = Record::shared(expected, fault);
        let mut source = Source::new(&bytes, &record);
        snapshot::reset_cleanup_observation();
        let result = catch_unwind(AssertUnwindSafe(|| {
            key.into_coefficient_snapshot(
                &mut source,
                id,
                Writer {
                    record: Rc::clone(&record),
                },
                expected,
                limit,
            )
        }));
        let panic = matches!(
            fault,
            Fault::SeekPanic
                | Fault::ReadPanic(_)
                | Fault::WritePanic(_)
                | Fault::SealPanic
                | Fault::SnapshotLayoutPanic
        );
        if panic {
            assert!(result.is_err(), "missing unwind for {fault:?}");
        } else {
            let error = match fault {
                Fault::ReadError(_, io::ErrorKind::InvalidData) => StoreError::Encoding,
                Fault::ReadError(_, io::ErrorKind::OutOfMemory) => StoreError::Allocation,
                Fault::WriterLayout(_)
                | Fault::DriftOnRead
                | Fault::DriftAfterWrite(_)
                | Fault::DriftOnSeal => StoreError::Context,
                _ => StoreError::Storage,
            };
            assert!(
                matches!(result.unwrap(), Err(actual) if actual == error),
                "wrong failure for {fault:?}"
            );
        }
        let r = record.borrow();
        assert_eq!(r.writer_drops, 1);
        assert_eq!(
            r.snapshot_drops,
            usize::from(matches!(
                fault,
                Fault::DriftOnSeal | Fault::SnapshotLayoutPanic
            ))
        );
        match fault {
            Fault::ReadError(at, _) | Fault::ReadPanic(at) => {
                assert_eq!(r.delivered, at);
                assert!(r.writes.is_empty());
                assert_eq!(r.seals, 0);
            }
            Fault::SeekError | Fault::SeekPanic => {
                assert_eq!(r.delivered, 0);
                assert!(r.writes.is_empty());
                assert_eq!(r.seals, 0);
            }
            Fault::DriftOnRead => {
                assert!(r.writes.is_empty());
                assert_eq!(r.seals, 0);
            }
            Fault::WriteError(chunk) | Fault::WritePanic(chunk) => {
                assert_eq!(r.writes.len(), chunk as usize);
                assert_eq!(r.seals, 0);
                assert_eq!(r.delivered, total);
            }
            Fault::DriftAfterWrite(chunk) => {
                assert_eq!(r.writes.len(), chunk as usize + 1);
                assert_eq!(r.seals, 0);
            }
            Fault::SealError
            | Fault::SealPanic
            | Fault::DriftOnSeal
            | Fault::SnapshotLayoutPanic => {
                assert_eq!(r.seals, 1);
            }
            _ => (),
        }
        let allocated = if matches!(fault, Fault::WriterLayout(_)) {
            None
        } else {
            Some(true)
        };
        assert_cleanup(key.rows(), allocated);
    }
}
#[test]
fn both_pasta_indexed_snapshot_failures_unwinds_and_layout_drift_clear_owned_scratch() {
    faults::<EqAffine>();
    faults::<EpAffine>();
}

fn bad_encoding<C: SerdeCurveAffine>()
where
    C::Scalar: StoredAssignmentFieldV1 + SerdePrimeField + FromUniformBytes<64>,
{
    let (_, original) = fixture::<C, false>(4, true, true);
    let key = index::<C, false, _, _>(
        &mut original.as_slice(),
        4,
        original.len() as u64,
        &mut io::sink(),
    )
    .unwrap();
    for id in [
        PolynomialId::MaskCoefficient(0),
        PolynomialId::PermutationLagrange(0),
    ] {
        let mut bytes = original.clone();
        let (at, _, width) = interval(&key, id);
        bytes[at as usize..at as usize + width].fill(255);
        let expected = layout::<C>(4, id, 41);
        let record = Record::shared(expected, Fault::None);
        let mut source = Source::new(&bytes, &record);
        snapshot::reset_cleanup_observation();
        assert!(matches!(
            key.into_coefficient_snapshot(
                &mut source,
                id,
                Writer {
                    record: Rc::clone(&record)
                },
                expected,
                usize::MAX
            ),
            Err(StoreError::Encoding)
        ));
        assert!(record.borrow().writes.is_empty());
        assert_eq!(record.borrow().seals, 0);
        assert_eq!(record.borrow().writer_drops, 1);
        assert_cleanup(key.rows(), Some(true));
    }
}
#[test]
fn both_pasta_indexed_snapshot_rejects_noncanonical_scalars_and_invalid_sigma_targets() {
    bad_encoding::<EqAffine>();
    bad_encoding::<EpAffine>();
}

fn transform_unwind<C: SerdeCurveAffine>()
where
    C::Scalar: StoredAssignmentFieldV1 + SerdePrimeField + FromUniformBytes<64>,
{
    use super::super::indexed::reads::{
        CoefficientTransformBoundaryPanic, with_coefficient_transform_panic,
    };

    let (_, bytes) = fixture::<C, false>(4, true, true);
    let old = old::<C, false>(&bytes, 4, bytes.len() as u64).unwrap();
    let key = index::<C, false, _, _>(
        &mut bytes.as_slice(),
        4,
        bytes.len() as u64,
        &mut io::sink(),
    )
    .unwrap();
    for id in [
        PolynomialId::FixedLagrange(0),
        PolynomialId::PermutationLagrange(0),
    ] {
        let expected = layout::<C>(4, id, 67);
        let record = Record::shared(expected, Fault::None);
        let mut source = Source::new(&bytes, &record);
        snapshot::reset_cleanup_observation();
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_coefficient_transform_panic(|| {
                key.into_coefficient_snapshot(
                    &mut source,
                    id,
                    Writer {
                        record: Rc::clone(&record),
                    },
                    expected,
                    usize::MAX,
                )
            })
        }));
        let payload = match result {
            Err(payload) => payload,
            Ok(_) => panic!("indexed snapshot did not reach the armed transform boundary"),
        };
        let boundary = payload
            .downcast::<CoefficientTransformBoundaryPanic>()
            .expect("unexpected panic before the post-decode transform boundary");
        let native = match id {
            PolynomialId::FixedLagrange(column) => &old.fixed_values[column][..],
            PolynomialId::PermutationLagrange(column) => &old.permutation.permutations[column][..],
            PolynomialId::MaskCoefficient(_) => unreachable!(),
        };
        assert_eq!(boundary.rows, key.rows());
        assert_eq!(
            boundary.nonzero,
            native
                .iter()
                .filter(|value| **value != C::Scalar::ZERO)
                .count(),
        );
        assert!(boundary.nonzero > 0);
        let (at, bytes, unit) = interval(&key, id);
        let r = record.borrow();
        assert_eq!((r.seeks, r.delivered, r.reads), (1, bytes, bytes / unit));
        assert_eq!(r.largest_request, unit);
        assert_eq!(
            (r.first_byte, r.last_byte),
            (Some(at), Some(at + bytes as u64)),
        );
        assert_eq!(source.cursor.position(), at + bytes as u64);
        assert!(r.writes.is_empty());
        assert!(r.padded.is_empty());
        assert_eq!((r.seals, r.writer_drops, r.snapshot_drops), (0, 1, 0));
        assert_cleanup(key.rows(), Some(true));
    }
}

#[test]
fn both_pasta_indexed_snapshot_transform_boundary_unwind_clears_decoded_scratch_before_writes() {
    transform_unwind::<EqAffine>();
    transform_unwind::<EpAffine>();
}
