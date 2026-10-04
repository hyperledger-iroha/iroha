//! Exact sparse-wire intervals compared with original real Processed-key polynomials.
//!
//! These tiny plaintext fixtures test parsing, bounded I/O and destination cleanup only.
//! They do not authenticate the reader, establish an immutable artifact owner, or qualify a
//! proof consumer, the whole process memory bound, hardware, or production readiness.

use super::super::indexed::reads::IndexedKeyPolynomialV1 as PolynomialId;
use super::super::indexed_io_test_plan as io_plan;
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

fn assert_io(
    reader: &CountedReader<'_>,
    plan: &[io_plan::Operation],
    chunk: usize,
    boundary: Option<usize>,
) {
    let expected = io_plan::observation(plan, chunk, boundary);
    assert_eq!(reader.seeks, expected.seeks);
    assert_eq!(reader.reads, expected.reads);
    assert_eq!(reader.delivered, expected.delivered);
    assert_eq!(reader.largest_request, expected.largest);
    assert_eq!(reader.first_byte, expected.first);
    assert_eq!(reader.last_byte, expected.last);
    assert_eq!(reader.inner.position(), expected.position);
}

fn interval_parity<C: SerdeCurveAffine, const EMPTY: bool>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    for k in [4, 5] {
        for compressed in [false, true] {
            for arbitrary_masks in [false, true] {
                let (generated, bytes) = fixture::<C, EMPTY>(k, compressed, arbitrary_masks);
                let original = processed::<C, EMPTY>(&generated);
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
                                let plan = io_plan::plan(&bytes, &key, polynomial, start, length);
                                assert_io(&reader, &plan, usize::MAX, None);
                                assert!(
                                    plan.iter()
                                        .all(|op| op.at + op.bytes as u64 <= key.frame_bytes())
                                );
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
    let (generated, bytes) = fixture::<C, false>(4, true, true);
    let original = processed::<C, false>(&generated);
    let key = index::<C, false, _, _>(
        &mut bytes.as_slice(),
        4,
        bytes.len() as u64,
        &mut io::sink(),
    )
    .unwrap();
    for polynomial in polynomial_ids(&original) {
        for (start, length) in [(0, key.rows()), (5, 9), (key.rows() - 1, 1)] {
            let plan = io_plan::plan(&bytes, &key, polynomial, start, length);
            let total_bytes = io_plan::total(&plan);
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
                    assert_io(&reader, &plan, 1, Some(boundary));
                }
            }
            for fault in [Fault::SeekError, Fault::SeekPanic, Fault::WrongSeekPosition] {
                if plan.is_empty() {
                    continue;
                }
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
                assert_io(&reader, &plan, chunk, None);
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
    let (generated, bytes) = fixture::<C, false>(4, true, true);
    let original = processed::<C, false>(&generated);
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
            3 => {
                for at in 0..record.nonzero as usize {
                    cases.push((
                        PolynomialId::FixedLagrange(column),
                        record.payload.offset as usize + 4 + at * (4 + width) + 4,
                        width,
                    ));
                }
            }
            1 => {}
            _ => unreachable!(),
        }
    }
    for (column, record) in m.permutations.iter().enumerate() {
        let (stride, prefix) = if record.mode == Mode::Sparse {
            (8, 4)
        } else {
            (4, 0)
        };
        let count = record.targets.length as usize / stride;
        if count > 0 {
            for target in [0, count / 2, count - 1] {
                cases.push((
                    PolynomialId::PermutationLagrange(column),
                    record.targets.offset as usize + target * stride + prefix,
                    4,
                ));
            }
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
        assert!(reader.seeks >= 1);
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

// The original fixture links rows 0..7, leaving each last row as an identity target.
// Consequently its intervals never contain adjacent targets n-1,n inside one column.

fn cross_coset_boundaries<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let (mut generated, mut bytes) = fixture::<C, false>(4, true, true);
    let initial = index::<C, false, _, _>(
        &mut bytes.as_slice(),
        4,
        bytes.len() as u64,
        &mut io::sink(),
    )
    .unwrap();
    let n = initial.rows();
    let columns = initial.metadata().permutation_columns;
    assert!(columns >= 2);
    let cells = n * columns;
    let inverse = InverseIndex::new(n, columns, generated.vk.domain.get_omega()).unwrap();
    let mut targets = generated
        .permutation
        .permutations
        .iter()
        .flat_map(|column| column.iter().map(|value| inverse.target(*value).unwrap()))
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), cells);

    // Preserve the entire target bijection using swaps, rather than introducing duplicate
    // labels. Both transitions are interior to one requested source column: n-1 -> n and
    // the final target column's last row -> the first target column's first row.
    let start = 3;
    let wanted =
        [n - 2, n - 1, n, cells - 2, cells - 1, 0].map(|target| u32::try_from(target).unwrap());
    assert!(start + wanted.len() < n);
    for (offset, target) in wanted.iter().copied().enumerate() {
        let from = targets.iter().position(|value| *value == target).unwrap();
        targets.swap(start + offset, from);
    }
    assert_eq!(&targets[start..start + wanted.len()], &wanted);
    let mut sorted = targets.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        (0..u32::try_from(cells).unwrap()).collect::<Vec<_>>()
    );
    let omega = generated.vk.domain.get_omega();
    for (column, chunk) in targets.chunks(n).enumerate() {
        for (value, target) in generated.permutation.permutations[column]
            .values
            .iter_mut()
            .zip(chunk)
        {
            *value = C::Scalar::DELTA.pow_vartime([(*target as usize / n) as u64])
                * omega.pow_vartime([(*target as usize % n) as u64]);
        }
        generated.permutation.polys[column] = coefficients(
            &generated.vk.domain,
            &generated.permutation.permutations[column],
        )
        .unwrap();
    }
    bytes.clear();
    generated.write_structured_v1(&mut bytes).unwrap();

    // Real Processed PK and fresh index scan preserve this exact structurally
    // canonical frame. This tests parser arithmetic, not a valid proof or artifact authority.
    let original = processed::<C, false>(&generated);
    let mut canonical = Vec::new();
    let key = index::<C, false, _, _>(&mut bytes.as_slice(), 4, bytes.len() as u64, &mut canonical)
        .unwrap();
    assert_eq!(canonical, bytes);
    check_index(&original, &bytes, &key);
    let expected = original_polynomial(&original, PolynomialId::PermutationLagrange(0));
    let omega = original.vk.domain.get_omega();
    assert_eq!(expected[start] * omega, expected[start + 1]);
    assert_ne!(expected[start + 1] * omega, expected[start + 2]);
    assert_eq!(expected[start + 3] * omega, expected[start + 4]);
    assert_ne!(expected[start + 4] * omega, expected[start + 5]);

    for (begin, length) in [(start, 6), (start + 1, 2), (start + 4, 2), (0, n)] {
        for chunk in [1, 3, usize::MAX] {
            let marker = C::Scalar::from(233);
            let mut guarded = vec![marker; length + 2];
            let plan = io_plan::plan(
                &bytes,
                &key,
                PolynomialId::PermutationLagrange(0),
                begin,
                length,
            );
            let total_bytes = io_plan::total(&plan);
            let mut reader = CountedReader::new(&bytes, Fault::ReadPanic(total_bytes), chunk);
            key.copy_native_interval(
                &mut reader,
                PolynomialId::PermutationLagrange(0),
                begin,
                &mut guarded[1..length + 1],
            )
            .unwrap();
            assert_eq!(&guarded[1..length + 1], &expected[begin..begin + length]);
            assert_eq!(guarded[0], marker);
            assert_eq!(guarded[length + 1], marker);
            assert_io(&reader, &plan, chunk, None);
        }
    }
}

#[test]
fn both_pasta_indexed_native_target_runs_cross_cosets_without_reusing_the_wrong_label() {
    cross_coset_boundaries::<EqAffine>();
    cross_coset_boundaries::<EpAffine>();
}

fn coefficient_columns<C: SerdeCurveAffine, const EMPTY: bool>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    for k in [4, 5] {
        for compressed in [false, true] {
            for arbitrary_masks in [false, true] {
                let (generated, bytes) = fixture::<C, EMPTY>(k, compressed, arbitrary_masks);
                let original = processed::<C, EMPTY>(&generated);
                let key = index::<C, EMPTY, _, _>(
                    &mut bytes.as_slice(),
                    k,
                    bytes.len() as u64,
                    &mut io::sink(),
                )
                .unwrap();
                for polynomial in polynomial_ids(&original) {
                    let expected: &[C::Scalar] = match polynomial {
                        PolynomialId::MaskCoefficient(0) => &original.l0,
                        PolynomialId::MaskCoefficient(1) => &original.l_last,
                        PolynomialId::MaskCoefficient(2) => &original.l_active_row,
                        PolynomialId::MaskCoefficient(_) => unreachable!(),
                        PolynomialId::FixedLagrange(column) => &original.fixed_polys[column],
                        PolynomialId::PermutationLagrange(column) => {
                            &original.permutation.polys[column]
                        }
                    };
                    let n = key.rows();
                    let plan = io_plan::plan(&bytes, &key, polynomial, 0, n);
                    let encoded = io_plan::total(&plan);
                    for chunk in [1, 3, usize::MAX] {
                        let marker = C::Scalar::from(229);
                        let mut output = vec![marker; n + 2];
                        let pointer = output.as_ptr();
                        let capacity = output.capacity();
                        let mut reader =
                            CountedReader::new(&bytes, Fault::ReadPanic(encoded), chunk);
                        key.copy_coefficient_column(&mut reader, polynomial, &mut output[1..n + 1])
                            .unwrap();
                        assert_eq!(&output[1..n + 1], expected);
                        assert_eq!(output[0], marker);
                        assert_eq!(output[n + 1], marker);
                        assert_eq!(output.as_ptr(), pointer);
                        assert_eq!(output.capacity(), capacity);
                        assert_io(&reader, &plan, chunk, None);
                    }
                }
            }
        }
    }
}

#[test]
fn both_pasta_indexed_coefficient_columns_match_original_dense_transform_without_replacing_storage()
{
    coefficient_columns::<EqAffine, false>();
    coefficient_columns::<EpAffine, false>();
    coefficient_columns::<EqAffine, true>();
    coefficient_columns::<EpAffine, true>();
}

fn coefficient_refusals<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let (original, bytes) = fixture::<C, false>(4, true, true);
    let key = index::<C, false, _, _>(
        &mut bytes.as_slice(),
        4,
        bytes.len() as u64,
        &mut io::sink(),
    )
    .unwrap();
    let n = key.rows();
    for polynomial in polynomial_ids(&original) {
        for length in [0, 1, n - 1, n + 1, n + 7] {
            let marker = C::Scalar::from(233);
            let mut output = vec![marker; length + 2];
            let mut reader = CountedReader::new(&bytes, Fault::ReadPanic(0), 1);
            let result =
                key.copy_coefficient_column(&mut reader, polynomial, &mut output[1..length + 1]);
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
            assert!(
                output[1..length + 1]
                    .iter()
                    .all(|value| *value == C::Scalar::ZERO)
            );
            assert_eq!(output[0], marker);
            assert_eq!(output[length + 1], marker);
            reader.assert_no_io();
        }
        let plan = io_plan::plan(&bytes, &key, polynomial, 0, n);
        let encoded = io_plan::total(&plan);
        if encoded == 0 {
            continue;
        }
        for at in [0, encoded / 2, encoded - 1] {
            for fault in [
                Fault::ReadError(at),
                Fault::ReadEof(at),
                Fault::ReadPanic(at),
            ] {
                let marker = C::Scalar::from(239);
                let mut output = vec![marker; n + 2];
                let mut reader = CountedReader::new(&bytes, fault, 1);
                let result = catch_unwind(AssertUnwindSafe(|| {
                    key.copy_coefficient_column(&mut reader, polynomial, &mut output[1..n + 1])
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
                    output[1..n + 1]
                        .iter()
                        .all(|value| *value == C::Scalar::ZERO)
                );
                assert_eq!(output[0], marker);
                assert_eq!(output[n + 1], marker);
                assert_eq!(reader.delivered, at);
                assert_eq!(reader.reads, at + 1);
            }
        }
    }
    for polynomial in [
        PolynomialId::MaskCoefficient(3),
        PolynomialId::FixedLagrange(original.fixed_values.len()),
        PolynomialId::PermutationLagrange(original.permutation.permutations.len()),
    ] {
        let mut output = vec![C::Scalar::ONE; n];
        let mut reader = CountedReader::new(&bytes, Fault::ReadPanic(0), 1);
        assert_eq!(
            key.copy_coefficient_column(&mut reader, polynomial, &mut output)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert!(output.iter().all(|value| *value == C::Scalar::ZERO));
        reader.assert_no_io();
    }
}

#[test]
fn both_pasta_indexed_coefficient_shape_and_source_failures_clear_all_output_before_return() {
    coefficient_refusals::<EqAffine>();
    coefficient_refusals::<EpAffine>();
}

fn coefficient_transform_boundary_unwind<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    use super::super::indexed::reads::{
        CoefficientTransformBoundaryPanic, with_coefficient_transform_panic,
    };

    use crate::poly::commitment::{Params as _, ParamsProver as _};
    use group::Curve as _;

    let (mut generated, _) = fixture::<C, false>(4, true, true);
    // Zero-only columns now canonically use a bitset at this tiny domain. Populate
    // every such unused parser-fixture column so the boundary observes nonzero decoded
    // data before erasing it. Preserve the real polynomial banks, commitments,
    // and Processed-key oracle; this fixture does not qualify a proof consumer.
    let mut populated = 0;
    for (values, coefficients_out) in generated
        .fixed_values
        .iter_mut()
        .zip(&mut generated.fixed_polys)
    {
        if values.values.iter().all(|value| *value == C::Scalar::ZERO) {
            values.values[0] = C::Scalar::ONE;
            *coefficients_out = coefficients(&generated.vk.domain, values).unwrap();
            populated += 1;
        }
    }
    assert!(populated > 0);
    let params = ParamsIPA::<C>::new(4);
    let commitments = generated
        .fixed_values
        .iter()
        .map(|values| params.commit_lagrange(values, Blind::default()).to_affine())
        .collect();
    generated.vk = VerifyingKey::from_parts(
        generated.vk.domain.clone(),
        commitments,
        generated.vk.permutation.clone(),
        generated.vk.cs.clone(),
        generated.vk.selectors.clone(),
        true,
    );
    let mut bytes = Vec::new();
    generated.write_structured_v1(&mut bytes).unwrap();
    let initial = index::<C, false, _, _>(
        &mut bytes.as_slice(),
        4,
        bytes.len() as u64,
        &mut io::sink(),
    )
    .unwrap();
    for mode in [0, 1, 2, 3] {
        assert!(
            initial
                .metadata()
                .fixed
                .iter()
                .any(|record| record.mode == mode)
        );
    }
    let original = processed::<C, false>(&generated);
    let mut canonical = Vec::new();
    let key = index::<C, false, _, _>(&mut bytes.as_slice(), 4, bytes.len() as u64, &mut canonical)
        .unwrap();
    assert_eq!(canonical, bytes);
    let n = key.rows();
    for polynomial in polynomial_ids(&original) {
        if matches!(polynomial, PolynomialId::MaskCoefficient(_)) {
            continue;
        }
        let expected_native = original_polynomial(&original, polynomial);
        let expected_nonzero = expected_native
            .iter()
            .filter(|value| **value != C::Scalar::ZERO)
            .count();
        assert!(expected_nonzero > 0);
        let expected_coefficients: &[C::Scalar] = match polynomial {
            PolynomialId::FixedLagrange(column) => &original.fixed_polys[column],
            PolynomialId::PermutationLagrange(column) => &original.permutation.polys[column],
            PolynomialId::MaskCoefficient(_) => unreachable!(),
        };
        let plan = io_plan::plan(&bytes, &key, polynomial, 0, n);
        let encoded = io_plan::total(&plan);
        with_coefficient_transform_panic(|| {
            // Masks already contain coefficients and must neither invoke nor consume the hook.
            let mut mask = vec![C::Scalar::ZERO; n];
            let mut mask_reader = CountedReader::new(&bytes, Fault::None, 3);
            key.copy_coefficient_column(
                &mut mask_reader,
                PolynomialId::MaskCoefficient(0),
                &mut mask,
            )
            .unwrap();
            assert_eq!(mask, original.l0[..]);

            let marker = C::Scalar::from(241);
            let mut output = vec![marker; n + 2];
            let pointer = output.as_ptr();
            let capacity = output.capacity();
            let mut reader = CountedReader::new(&bytes, Fault::ReadPanic(encoded), 3);
            let panic = catch_unwind(AssertUnwindSafe(|| {
                key.copy_coefficient_column(&mut reader, polynomial, &mut output[1..n + 1])
                    .unwrap();
            }))
            .expect_err("synthetic post-decode/pre-transform hook must panic");
            let observed = panic
                .downcast_ref::<CoefficientTransformBoundaryPanic>()
                .expect("the panic must come from the transform boundary, not the reader");
            assert_eq!(observed.rows, n);
            assert_eq!(observed.nonzero, expected_nonzero);
            assert!(
                output[1..n + 1]
                    .iter()
                    .all(|value| *value == C::Scalar::ZERO)
            );
            assert_eq!(output[0], marker);
            assert_eq!(output[n + 1], marker);
            assert_eq!(output.as_ptr(), pointer);
            assert_eq!(output.capacity(), capacity);
            assert_io(&reader, &plan, 3, None);

            // The one-shot flag resets before the panic, even while the arming scope is alive.
            // A new parser call on the same key succeeds; no persistent authority poisoning
            // property is asserted by this internal unauthenticated decoder test.
            let mut followup = CountedReader::new(&bytes, Fault::ReadPanic(encoded), 3);
            key.copy_coefficient_column(&mut followup, polynomial, &mut output[1..n + 1])
                .unwrap();
            assert_eq!(&output[1..n + 1], expected_coefficients);
            assert_eq!(output[0], marker);
            assert_eq!(output[n + 1], marker);
            assert_eq!(output.as_ptr(), pointer);
            assert_eq!(output.capacity(), capacity);
            assert_io(&followup, &plan, 3, None);
        });
    }

    // Scopes that never reach a transform must also disarm on return or unrelated unwind.
    with_coefficient_transform_panic(|| ());
    let abandoned_scope = catch_unwind(AssertUnwindSafe(|| {
        with_coefficient_transform_panic(|| panic!("synthetic armed-scope unwind"));
    }));
    assert!(abandoned_scope.is_err());
    let mut output = vec![C::Scalar::ZERO; n];
    key.copy_coefficient_column(
        &mut Cursor::new(bytes.as_slice()),
        PolynomialId::PermutationLagrange(0),
        &mut output,
    )
    .unwrap();
    assert_eq!(output, original.permutation.polys[0][..]);
}

#[test]
fn both_pasta_indexed_coefficient_synthetic_post_decode_transform_panic_erases_output_and_resets_hook()
 {
    coefficient_transform_boundary_unwind::<EqAffine>();
    coefficient_transform_boundary_unwind::<EpAffine>();
}

fn sparse_intervals<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let (original, bytes) = fixture::<C, false>(6, false, false);
    let key = index::<C, false, _, _>(
        &mut bytes.as_slice(),
        6,
        bytes.len() as u64,
        &mut io::sink(),
    )
    .unwrap();
    for (column, record) in key
        .metadata()
        .fixed
        .iter()
        .enumerate()
        .filter(|(_, record)| record.mode == SPARSE_ZERO)
    {
        for (start, length) in [(0, 0), (0, 1), (0, 64), (1, 8), (7, 9), (8, 32), (63, 1)] {
            let mut output = vec![C::Scalar::from(99); length];
            key.copy_native_interval(
                &mut std::io::Cursor::new(&bytes),
                PolynomialId::FixedLagrange(column),
                start,
                &mut output,
            )
            .unwrap();
            assert_eq!(output, original.fixed_values[column][start..start + length]);
        }
        if record.nonzero > 1 {
            let width = scalar_bytes::<C::Scalar>();
            let at = record.payload.offset as usize;
            let mut cases = Vec::new();
            let mut bad = bytes.clone();
            bad[at..at + 4].copy_from_slice(&(record.nonzero - 1).to_le_bytes());
            cases.push(bad);
            let mut bad = bytes.clone();
            bad[at + 8..at + 8 + width].fill(255);
            cases.push(bad);
            let mut bad = bytes.clone();
            bad[at + 8..at + 8 + width].copy_from_slice(C::Scalar::ZERO.to_repr().as_ref());
            cases.push(bad);
            let mut bad = bytes.clone();
            bad[at + 4 + (4 + width)..at + 8 + (4 + width)].copy_from_slice(&0_u32.to_le_bytes());
            cases.push(bad);
            for bad in cases {
                let mut output = vec![C::Scalar::from(99); 64];
                assert!(
                    key.copy_native_interval(
                        &mut std::io::Cursor::new(&bad),
                        PolynomialId::FixedLagrange(column),
                        0,
                        &mut output
                    )
                    .is_err()
                );
                assert!(output.iter().all(|value| *value == C::Scalar::ZERO));
            }
        }
    }
}
#[test]
fn both_fields_sparse_fixed_original_intervals_zero_gaps_and_changed_source_failures_clear_destinations()
 {
    sparse_intervals::<EqAffine>();
    sparse_intervals::<EpAffine>();
}
