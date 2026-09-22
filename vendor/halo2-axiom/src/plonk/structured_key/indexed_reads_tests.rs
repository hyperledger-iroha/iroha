//! Direct native-basis interval reads against the frozen original dense codec.
//!
//! These tiny plaintext fixtures test parsing, bounded I/O and destination cleanup only.
//! They do not authenticate the reader, establish an immutable artifact owner, or qualify a
//! proof consumer, the whole process memory bound, hardware, or production readiness.

use super::super::indexed::reads::IndexedKeyPolynomialV1 as PolynomialId;
use super::*;
use std::io::{Cursor, Seek, SeekFrom};

#[derive(Clone, Copy, Debug)]
enum Fault {
    None,
    ReadError(usize),
    ReadEof(usize),
    ReadPanic(usize),
    SeekError,
    SeekPanic,
    WrongSeekPosition,
}

/// Counts actual source bytes and the largest request, without allocating per-operation logs.
struct CountedReader<'a> {
    inner: Cursor<&'a [u8]>,
    fault: Fault,
    chunk: usize,
    reads: usize,
    seeks: usize,
    delivered: usize,
    largest_request: usize,
    first_byte: Option<u64>,
    last_byte: Option<u64>,
}

impl<'a> CountedReader<'a> {
    fn new(bytes: &'a [u8], fault: Fault, chunk: usize) -> Self {
        assert!(chunk > 0);
        let mut inner = Cursor::new(bytes);
        // Every nonempty operation must address the frame, independently of this cursor.
        inner.set_position(13);
        Self {
            inner,
            fault,
            chunk,
            reads: 0,
            seeks: 0,
            delivered: 0,
            largest_request: 0,
            first_byte: None,
            last_byte: None,
        }
    }

    fn assert_no_io(&self) {
        assert_eq!(self.reads, 0);
        assert_eq!(self.seeks, 0);
        assert_eq!(self.delivered, 0);
        assert_eq!(self.largest_request, 0);
        assert_eq!(self.first_byte, None);
        assert_eq!(self.last_byte, None);
        assert_eq!(self.inner.position(), 13);
    }
}

impl Read for CountedReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.reads += 1;
        self.largest_request = self.largest_request.max(output.len());
        let boundary = match self.fault {
            Fault::ReadError(at) | Fault::ReadEof(at) | Fault::ReadPanic(at) => Some(at),
            _ => None,
        };
        if boundary == Some(self.delivered) {
            match self.fault {
                Fault::ReadError(_) => {
                    return Err(io::Error::other("indexed source injected read error"));
                }
                Fault::ReadEof(_) => return Ok(0),
                Fault::ReadPanic(_) => panic!("indexed source injected read panic"),
                _ => unreachable!(),
            }
        }
        let mut count = output.len().min(self.chunk);
        if let Some(at) = boundary {
            count = count.min(at - self.delivered);
        }
        let before = self.inner.position();
        let actual = self.inner.read(&mut output[..count])?;
        if actual != 0 {
            self.first_byte.get_or_insert(before);
            self.last_byte = Some(before + actual as u64);
        }
        self.delivered += actual;
        Ok(actual)
    }
}

impl Seek for CountedReader<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.seeks += 1;
        assert!(matches!(position, SeekFrom::Start(_)));
        match self.fault {
            Fault::SeekError => Err(io::Error::other("indexed source injected seek error")),
            Fault::SeekPanic => panic!("indexed source injected seek panic"),
            Fault::WrongSeekPosition => self.inner.seek(position).map(|actual| actual + 1),
            _ => self.inner.seek(position),
        }
    }
}

fn original_polynomial<C: SerdeCurveAffine>(
    key: &ProvingKey<C>,
    polynomial: PolynomialId,
) -> &[C::Scalar] {
    match polynomial {
        PolynomialId::MaskCoefficient(0) => &key.l0,
        PolynomialId::MaskCoefficient(1) => &key.l_last,
        PolynomialId::MaskCoefficient(2) => &key.l_active_row,
        PolynomialId::MaskCoefficient(_) => unreachable!(),
        PolynomialId::FixedLagrange(column) => &key.fixed_values[column],
        PolynomialId::PermutationLagrange(column) => &key.permutation.permutations[column],
    }
}

fn polynomial_ids<C: SerdeCurveAffine>(key: &ProvingKey<C>) -> Vec<PolynomialId> {
    (0..3)
        .map(PolynomialId::MaskCoefficient)
        .chain((0..key.fixed_values.len()).map(PolynomialId::FixedLagrange))
        .chain((0..key.permutation.permutations.len()).map(PolynomialId::PermutationLagrange))
        .collect()
}

/// The original full decoder determines values; metadata only identifies the encoded interval.
fn encoded_interval<C: SerdeCurveAffine>(
    key: &IndexedStructuredProvingKeyV1<C>,
    polynomial: PolynomialId,
    start: usize,
    length: usize,
) -> (u64, usize, usize)
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let m = key.metadata();
    let width = <C::Scalar as PrimeField>::Repr::default().as_ref().len();
    match polynomial {
        PolynomialId::MaskCoefficient(mask) => (
            m.masks[mask].offset + (start * width) as u64,
            length * width,
            width,
        ),
        PolynomialId::FixedLagrange(column) => {
            let record = &m.fixed[column];
            match record.mode {
                0 => (record.payload.offset, width, width),
                1 => (
                    record.payload.offset + (start / 8) as u64,
                    (start + length).div_ceil(8) - start / 8,
                    1,
                ),
                2 => (
                    record.payload.offset + (start * width) as u64,
                    length * width,
                    width,
                ),
                _ => unreachable!(),
            }
        }
        PolynomialId::PermutationLagrange(column) => (
            m.permutation_targets.offset + ((column * m.rows + start) * 4) as u64,
            length * 4,
            4,
        ),
    }
}

fn interval_parity<C: SerdeCurveAffine, const EMPTY: bool>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    for k in [4, 5] {
        for compressed in [false, true] {
            for arbitrary_masks in [false, true] {
                let (generated, bytes) = fixture::<C, EMPTY>(k, compressed, arbitrary_masks);
                let original = old::<C, EMPTY>(&bytes, k, bytes.len() as u64).unwrap();
                assert_eq!(
                    original.to_bytes(SerdeFormat::Processed),
                    generated.to_bytes(SerdeFormat::Processed)
                );
                let key = index::<C, EMPTY, _, _>(
                    &mut bytes.as_slice(),
                    k,
                    bytes.len() as u64,
                    &mut io::sink(),
                )
                .unwrap();
                check_index(&original, &bytes, &key);
                let mut source = bytes.clone();
                source.extend_from_slice(b"not part of the indexed frame");
                let n = key.rows();
                for polynomial in polynomial_ids(&original) {
                    let expected = original_polynomial(&original, polynomial);
                    // Exhaust every interval at n=16. At n=32, retain full, empty, endpoint
                    // and every byte-boundary crossing interval, including offset starts.
                    for start in 0..=n {
                        for end in start..=n {
                            if n > 16
                                && start != 0
                                && start != end
                                && end != n
                                && ![1, 7, 8, 9, 15, 16, 17, 23, 24, 25, 31].contains(&start)
                            {
                                continue;
                            }
                            let length = end - start;
                            let marker = C::Scalar::from(197);
                            let mut guarded = vec![marker; length + 2];
                            let address = guarded.as_ptr();
                            let capacity = guarded.capacity();
                            let mut reader = CountedReader::new(&source, Fault::None, usize::MAX);
                            key.copy_native_interval(
                                &mut reader,
                                polynomial,
                                start,
                                &mut guarded[1..length + 1],
                            )
                            .unwrap();
                            assert_eq!(&guarded[1..length + 1], &expected[start..end]);
                            assert_eq!(guarded[0], marker);
                            assert_eq!(guarded[length + 1], marker);
                            assert_eq!(guarded.as_ptr(), address);
                            assert_eq!(guarded.capacity(), capacity);
                            if length == 0 {
                                reader.assert_no_io();
                            } else {
                                let (at, bytes, unit) =
                                    encoded_interval(&key, polynomial, start, length);
                                assert_eq!(reader.seeks, 1);
                                assert_eq!(reader.delivered, bytes);
                                assert_eq!(reader.reads, bytes / unit);
                                assert_eq!(reader.largest_request, unit);
                                assert_eq!(reader.first_byte, Some(at));
                                assert_eq!(reader.last_byte, Some(at + bytes as u64));
                                assert_eq!(reader.inner.position(), at + bytes as u64);
                                assert!(at + bytes as u64 <= key.frame_bytes());
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn both_pasta_indexed_native_intervals_match_original_dense_masks_fixed_modes_and_permutations() {
    interval_parity::<EqAffine, false>();
    interval_parity::<EpAffine, false>();
    interval_parity::<EqAffine, true>();
    interval_parity::<EpAffine, true>();
}

fn invalid_requests<C: SerdeCurveAffine, const EMPTY: bool>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let (original, bytes) = fixture::<C, EMPTY>(4, true, true);
    let key = index::<C, EMPTY, _, _>(
        &mut bytes.as_slice(),
        4,
        bytes.len() as u64,
        &mut io::sink(),
    )
    .unwrap();
    let n = key.rows();
    let mut cases = Vec::new();
    for polynomial in polynomial_ids(&original) {
        cases.extend([
            (polynomial, n + 1, 0),
            (polynomial, n, 1),
            (polynomial, n - 1, 2),
            (polynomial, 0, n + 1),
            (polynomial, usize::MAX, 0),
            (polynomial, usize::MAX, 2),
        ]);
    }
    for polynomial in [
        PolynomialId::MaskCoefficient(3),
        PolynomialId::MaskCoefficient(usize::MAX),
        PolynomialId::FixedLagrange(original.fixed_values.len()),
        PolynomialId::FixedLagrange(usize::MAX),
        PolynomialId::PermutationLagrange(original.permutation.permutations.len()),
        PolynomialId::PermutationLagrange(usize::MAX),
    ] {
        for start in [0, n] {
            cases.push((polynomial, start, 0));
            cases.push((polynomial, start, 3));
        }
    }
    for (polynomial, start, length) in cases {
        let marker = C::Scalar::from(199);
        let mut guarded = vec![marker; length + 2];
        let mut reader = CountedReader::new(&bytes, Fault::ReadPanic(0), 1);
        let error = key
            .copy_native_interval(&mut reader, polynomial, start, &mut guarded[1..length + 1])
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            guarded[1..length + 1]
                .iter()
                .all(|value| *value == C::Scalar::ZERO)
        );
        assert_eq!(guarded[0], marker);
        assert_eq!(guarded[length + 1], marker);
        reader.assert_no_io();
    }
}

#[test]
fn both_pasta_indexed_native_invalid_columns_ranges_and_addition_overflow_clear_without_io() {
    invalid_requests::<EqAffine, false>();
    invalid_requests::<EpAffine, false>();
    invalid_requests::<EqAffine, true>();
    invalid_requests::<EpAffine, true>();
}

fn fault_boundaries<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let (_, bytes) = fixture::<C, false>(4, true, true);
    let original = old::<C, false>(&bytes, 4, bytes.len() as u64).unwrap();
    let key = index::<C, false, _, _>(
        &mut bytes.as_slice(),
        4,
        bytes.len() as u64,
        &mut io::sink(),
    )
    .unwrap();
    for polynomial in polynomial_ids(&original) {
        for (start, length) in [(0, key.rows()), (5, 9), (key.rows() - 1, 1)] {
            let (at, total_bytes, unit) = encoded_interval(&key, polynomial, start, length);
            // A one-byte source exercises failures inside each scalar/target and after every
            // already-written output prefix. All byte boundaries before completion must fail.
            for boundary in 0..total_bytes {
                for fault in [
                    Fault::ReadError(boundary),
                    Fault::ReadEof(boundary),
                    Fault::ReadPanic(boundary),
                ] {
                    let marker = C::Scalar::from(211);
                    let mut guarded = vec![marker; length + 2];
                    let address = guarded.as_ptr();
                    let capacity = guarded.capacity();
                    let mut reader = CountedReader::new(&bytes, fault, 1);
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        key.copy_native_interval(
                            &mut reader,
                            polynomial,
                            start,
                            &mut guarded[1..length + 1],
                        )
                    }));
                    match fault {
                        Fault::ReadPanic(_) => assert!(result.is_err()),
                        Fault::ReadError(_) => {
                            assert_eq!(result.unwrap().unwrap_err().kind(), io::ErrorKind::Other)
                        }
                        Fault::ReadEof(_) => assert_eq!(
                            result.unwrap().unwrap_err().kind(),
                            io::ErrorKind::UnexpectedEof
                        ),
                        _ => unreachable!(),
                    }
                    assert!(
                        guarded[1..length + 1]
                            .iter()
                            .all(|value| *value == C::Scalar::ZERO)
                    );
                    assert_eq!(guarded[0], marker);
                    assert_eq!(guarded[length + 1], marker);
                    assert_eq!(guarded.as_ptr(), address);
                    assert_eq!(guarded.capacity(), capacity);
                    assert_eq!(reader.seeks, 1);
                    assert_eq!(reader.reads, boundary + 1);
                    assert_eq!(reader.delivered, boundary);
                    assert_eq!(reader.largest_request, unit);
                    assert_eq!(reader.inner.position(), at + boundary as u64);
                }
            }
            for fault in [Fault::SeekError, Fault::SeekPanic, Fault::WrongSeekPosition] {
                let marker = C::Scalar::from(223);
                let mut guarded = vec![marker; length + 2];
                let mut reader = CountedReader::new(&bytes, fault, 1);
                let result = catch_unwind(AssertUnwindSafe(|| {
                    key.copy_native_interval(
                        &mut reader,
                        polynomial,
                        start,
                        &mut guarded[1..length + 1],
                    )
                }));
                match fault {
                    Fault::SeekPanic => assert!(result.is_err()),
                    Fault::SeekError => {
                        assert_eq!(result.unwrap().unwrap_err().kind(), io::ErrorKind::Other)
                    }
                    Fault::WrongSeekPosition => assert_eq!(
                        result.unwrap().unwrap_err().kind(),
                        io::ErrorKind::InvalidData
                    ),
                    _ => unreachable!(),
                }
                assert!(
                    guarded[1..length + 1]
                        .iter()
                        .all(|value| *value == C::Scalar::ZERO)
                );
                assert_eq!(guarded[0], marker);
                assert_eq!(guarded[length + 1], marker);
                assert_eq!(reader.seeks, 1);
                assert_eq!(reader.reads, 0);
                assert_eq!(reader.delivered, 0);
            }
            // A fault immediately beyond the requested bytes must remain untouched.
            for chunk in [1, 3, usize::MAX] {
                let mut output = vec![C::Scalar::ZERO; length];
                let mut reader = CountedReader::new(&bytes, Fault::ReadPanic(total_bytes), chunk);
                key.copy_native_interval(&mut reader, polynomial, start, &mut output)
                    .unwrap();
                assert_eq!(
                    output,
                    original_polynomial(&original, polynomial)[start..start + length]
                );
                assert_eq!(reader.delivered, total_bytes);
                assert_eq!(reader.seeks, 1);
                assert_eq!(reader.largest_request, unit);
                assert_eq!(reader.reads, (total_bytes / unit) * unit.div_ceil(chunk));
            }
        }
    }
}

#[test]
fn both_pasta_indexed_native_read_and_seek_errors_eof_and_unwind_clear_the_entire_destination() {
    fault_boundaries::<EqAffine>();
    fault_boundaries::<EpAffine>();
}

fn source_mutations<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let (_, bytes) = fixture::<C, false>(4, true, true);
    let original = old::<C, false>(&bytes, 4, bytes.len() as u64).unwrap();
    let key = index::<C, false, _, _>(
        &mut bytes.as_slice(),
        4,
        bytes.len() as u64,
        &mut io::sink(),
    )
    .unwrap();
    let m = key.metadata();
    let n = key.rows();
    let width = <C::Scalar as PrimeField>::Repr::default().as_ref().len();
    let mut cases = Vec::new();
    for (mask, range) in m.masks.iter().enumerate() {
        for row in [0, 1, n - 1] {
            cases.push((
                PolynomialId::MaskCoefficient(mask),
                range.offset as usize + row * width,
                width,
            ));
        }
    }
    for (column, record) in m.fixed.iter().enumerate() {
        match record.mode {
            0 => cases.push((
                PolynomialId::FixedLagrange(column),
                record.payload.offset as usize,
                width,
            )),
            2 => {
                for row in [0, 1, n - 1] {
                    cases.push((
                        PolynomialId::FixedLagrange(column),
                        record.payload.offset as usize + row * width,
                        width,
                    ));
                }
            }
            1 => {}
            _ => unreachable!(),
        }
    }
    for column in 0..m.permutation_columns {
        for row in [0, 1, n - 1] {
            cases.push((
                PolynomialId::PermutationLagrange(column),
                m.permutation_targets.offset as usize + (column * n + row) * 4,
                4,
            ));
        }
    }
    for (polynomial, at, encoded_width) in cases {
        let mut changed = bytes.clone();
        match polynomial {
            PolynomialId::PermutationLagrange(_) => changed[at..at + 4].copy_from_slice(
                &u32::try_from(n * m.permutation_columns)
                    .unwrap()
                    .to_le_bytes(),
            ),
            _ => {
                let mut repr = <C::Scalar as PrimeField>::Repr::default();
                repr.as_mut().fill(255);
                assert!(bool::from(C::Scalar::from_repr(repr).is_none()));
                changed[at..at + encoded_width].fill(255);
            }
        }
        let marker = C::Scalar::from(227);
        let mut guarded = vec![marker; n + 2];
        let mut reader = CountedReader::new(&changed, Fault::None, usize::MAX);
        let error = key
            .copy_native_interval(&mut reader, polynomial, 0, &mut guarded[1..n + 1])
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            guarded[1..n + 1]
                .iter()
                .all(|value| *value == C::Scalar::ZERO)
        );
        assert_eq!(guarded[0], marker);
        assert_eq!(guarded[n + 1], marker);
        assert_eq!(reader.seeks, 1);
        assert_eq!(reader.inner.position(), (at + encoded_width) as u64);
        assert_eq!(reader.last_byte, Some((at + encoded_width) as u64));
    }

    // This deliberately demonstrates the parser's authority boundary: a different canonical
    // source value is readable. Only the consuming authenticated owner can reject this swap;
    // successful bounded decoding must never be mistaken for source identity validation.
    let mut changed = bytes.clone();
    let replacement = original.l0[0] + C::Scalar::ONE;
    let at = m.masks[0].offset as usize;
    changed[at..at + width].copy_from_slice(replacement.to_repr().as_ref());
    let mut reader = CountedReader::new(&changed, Fault::None, usize::MAX);
    let mut output = [C::Scalar::ZERO];
    key.copy_native_interval(
        &mut reader,
        PolynomialId::MaskCoefficient(0),
        0,
        &mut output,
    )
    .unwrap();
    assert_eq!(output, [replacement]);
    assert_ne!(output[0], original.l0[0]);
}

#[test]
fn both_pasta_indexed_native_noncanonical_scalars_and_invalid_targets_clear_without_claiming_source_authentication()
 {
    source_mutations::<EqAffine>();
    source_mutations::<EpAffine>();
}
