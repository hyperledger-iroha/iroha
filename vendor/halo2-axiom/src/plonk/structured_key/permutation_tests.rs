//! Canonical modes, directed-target bijection and original-reader rank-window controls.

use super::*;
use super::{fixture, index, processed};
use crate::halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq};
use indexed::reads::IndexedKeyPolynomialV1 as Id;
use std::panic::{AssertUnwindSafe, catch_unwind};

struct Targets(Vec<Vec<u32>>);
impl<F: PrimeField> StructuredValues<F> for Targets {
    fn begin_permutation(&mut self, rows: usize) -> io::Result<()> {
        self.0.push(reserved(rows)?);
        Ok(())
    }
    fn permutation_target(&mut self, target: u32, _: usize) -> io::Result<()> {
        self.0.last_mut().unwrap().push(target);
        Ok(())
    }
}
fn scan<F: PrimeField>(bytes: &[u8], rows: usize, columns: usize) -> io::Result<Vec<Vec<u32>>> {
    let mut frame = bytes.take(bytes.len() as u64);
    let mut seen = Seen::new(rows * columns)?;
    let mut canonical = Vec::new();
    let mut targets = Targets(Vec::new());
    for column in 0..columns {
        scan_permutation_column::<F, _, _, _>(
            &mut frame,
            bytes.len() as u64,
            rows,
            column,
            &mut canonical,
            &mut seen,
            &mut targets,
        )?;
    }
    if frame.limit() != 0 {
        return Err(invalid("test column frame has trailing bytes"));
    }
    assert_eq!(canonical, bytes);
    Ok(targets.0)
}
fn literal(mode: Mode, targets: &[u32], column: usize) -> Vec<u8> {
    let n = targets.len();
    let exceptions = targets
        .iter()
        .enumerate()
        .filter(|(row, target)| **target as usize != column * n + *row)
        .collect::<Vec<_>>();
    let mut bytes = vec![mode as u8];
    match mode {
        Mode::Identity => {}
        Mode::Sparse => {
            bytes.extend_from_slice(&(exceptions.len() as u32).to_le_bytes());
            for (row, target) in exceptions {
                bytes.extend_from_slice(&(row as u32).to_le_bytes());
                bytes.extend_from_slice(&target.to_le_bytes());
            }
        }
        Mode::Bitmap => {
            let mut bitmap = vec![0; n.div_ceil(8)];
            for (row, _) in &exceptions {
                bitmap[row / 8] |= 1 << (row % 8);
            }
            bytes.extend_from_slice(&bitmap);
            for (_, target) in exceptions {
                bytes.extend_from_slice(&target.to_le_bytes());
            }
        }
        Mode::Dense => {
            for target in targets {
                bytes.extend_from_slice(&target.to_le_bytes());
            }
        }
    }
    bytes
}
fn modes<F: PrimeField>() {
    let n = 128;
    let id = (0..n as u32).collect::<Vec<_>>();
    for (density, mode) in [
        (0, Mode::Identity),
        (2, Mode::Sparse),
        (16, Mode::Bitmap),
        (128, Mode::Dense),
    ] {
        let mut targets = id.clone();
        if density > 0 {
            targets[..density].rotate_left(1);
        }
        let bytes = literal(mode, &targets, 0);
        assert_eq!(scan::<F>(&bytes, n, 1).unwrap(), vec![targets.clone()]);
        for other in [Mode::Identity, Mode::Sparse, Mode::Bitmap, Mode::Dense] {
            if other != mode {
                if other == Mode::Identity {
                    assert_ne!(
                        scan::<F>(&literal(other, &targets, 0), n, 1).unwrap(),
                        vec![targets.clone()]
                    );
                } else {
                    assert!(scan::<F>(&literal(other, &targets, 0), n, 1).is_err());
                }
            }
        }
        for end in 0..bytes.len() {
            assert!(scan::<F>(&bytes[..end], n, 1).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(scan::<F>(&trailing, n, 1).is_err());
    }
    // One exceptional source in each of two columns: Sparse beats/ties Bitmap at n=64.
    let n = 64;
    let mut left = (0..n as u32).collect::<Vec<_>>();
    let mut right = (n as u32..2 * n as u32).collect::<Vec<_>>();
    std::mem::swap(&mut left[3], &mut right[11]);
    let mut bytes = literal(Mode::Sparse, &left, 0);
    bytes.extend(literal(Mode::Sparse, &right, 1));
    assert_eq!(scan::<F>(&bytes, n, 2).unwrap(), vec![left.clone(), right]);
    // The same explicit target plus implied identities cannot escape the global bitmap.
    let mut duplicate = literal(Mode::Sparse, &left, 0);
    duplicate.extend(literal(
        Mode::Identity,
        &(n as u32..2 * n as u32).collect::<Vec<_>>(),
        1,
    ));
    assert!(scan::<F>(&duplicate, n, 2).is_err());
    let mut pair = literal(Mode::Sparse, &left, 0);
    pair[1..5].copy_from_slice(&65_u32.to_le_bytes());
    assert!(scan::<F>(&pair, n, 1).is_err());
    let mut pair = literal(Mode::Sparse, &left, 0);
    pair[5..9].copy_from_slice(&64_u32.to_le_bytes());
    assert!(scan::<F>(&pair, n, 1).is_err());
    let mut pair = literal(Mode::Sparse, &left, 0);
    pair[9..13].copy_from_slice(&3_u32.to_le_bytes());
    assert!(scan::<F>(&pair, n, 1).is_err());
    let mut targets = (0..128_u32).collect::<Vec<_>>();
    targets[..2].swap(0, 1);
    let mut unordered = literal(Mode::Sparse, &targets, 0);
    unordered[13..17].copy_from_slice(&0_u32.to_le_bytes());
    assert!(scan::<F>(&unordered, 128, 1).is_err());
    let mut short = (0..4_u32).collect::<Vec<_>>();
    short.swap(0, 1);
    let good = literal(Mode::Bitmap, &short, 0);
    assert_eq!(scan::<F>(&good, 4, 1).unwrap(), vec![short]);
    let mut bad = good;
    bad[1] |= 1 << 4;
    assert!(scan::<F>(&bad, 4, 1).is_err());
    assert!(scan::<F>(&[255], 128, 1).is_err());
}
#[test]
fn both_fields_sparse_columns_require_unique_modes_exact_pairs_padding_and_global_implied_bijection()
 {
    modes::<Fp>();
    modes::<Fq>();
}

pub(super) fn mode_fixture<C: SerdeCurveAffine>(k: u32) -> (ProvingKey<C>, Vec<u8>)
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let (mut pk, _) = fixture::<C, false>(k, true, true);
    let n = 1_usize << k;
    assert!(pk.permutation.permutations.len() >= 4);
    let omega = pk.vk.domain.get_omega();
    for (column, poly) in pk.permutation.permutations.iter_mut().enumerate() {
        let mut targets = (column * n..(column + 1) * n)
            .map(|v| v as u32)
            .collect::<Vec<_>>();
        let count = match column {
            0 => 0,
            1 => 2,
            2 => n / 8,
            3 => n,
            _ => 0,
        };
        if count > 0 {
            targets[..count].rotate_left(1);
        }
        for (value, target) in poly.values.iter_mut().zip(targets) {
            *value = C::Scalar::DELTA.pow_vartime([(target as usize / n) as u64])
                * omega.pow_vartime([(target as usize % n) as u64]);
        }
    }
    pk.permutation.polys = reconstruct(&pk.vk.domain, &pk.permutation.permutations).unwrap();
    let mut bytes = Vec::new();
    pk.write_structured_v1(&mut bytes).unwrap();
    (pk, bytes)
}
fn intervals<C: SerdeCurveAffine>()
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    // k7 covers all modes; k13 additionally crosses genuine 4096-row rank boundaries.
    for k in [7, 13] {
        let (pk, bytes) = mode_fixture::<C>(k);
        let expected = processed::<C, false>(&pk);
        let key = index::<C, false, _, _>(
            &mut bytes.as_slice(),
            k,
            bytes.len() as u64,
            &mut io::sink(),
        )
        .unwrap();
        assert_eq!(
            key.metadata.permutations[..4]
                .iter()
                .map(|r| r.mode)
                .collect::<Vec<_>>(),
            vec![Mode::Identity, Mode::Sparse, Mode::Bitmap, Mode::Dense]
        );
        let mut consuming = Vec::new();
        pk.write_structured_v1_consuming(&mut consuming).unwrap();
        assert_eq!(consuming, bytes);
        for column in 0..key.metadata.permutation_columns {
            let marker = C::Scalar::from(83);
            let mut coefficients = vec![marker; key.rows() + 2];
            let ptr = coefficients.as_ptr();
            let capacity = coefficients.capacity();
            key.copy_coefficient_column(
                &mut io::Cursor::new(bytes.as_slice()),
                Id::PermutationLagrange(column),
                &mut coefficients[1..key.rows() + 1],
            )
            .unwrap();
            assert_eq!(
                &coefficients[1..key.rows() + 1],
                &expected.permutation.polys[column][..]
            );
            assert_eq!(coefficients[0], marker);
            assert_eq!(coefficients[key.rows() + 1], marker);
            assert_eq!(coefficients.as_ptr(), ptr);
            assert_eq!(coefficients.capacity(), capacity);
            let starts = if k == 7 {
                vec![0, 1, 7, 8, 63, 64, 127, 128]
            } else {
                vec![0, 1, 511, 512, 4095, 4096, 4097, 8191, 8192]
            };
            for start in starts {
                for length in [0, 1, 7, 33, key.rows() - start] {
                    if start + length > key.rows() {
                        continue;
                    }
                    let mut output = vec![C::Scalar::from(81); length + 2];
                    key.copy_native_interval(
                        &mut io::Cursor::new(bytes.as_slice()),
                        Id::PermutationLagrange(column),
                        start,
                        &mut output[1..length + 1],
                    )
                    .unwrap();
                    assert_eq!(
                        &output[1..length + 1],
                        &expected.permutation.permutations[column][start..start + length]
                    );
                    assert_eq!(output[0], C::Scalar::from(81));
                    assert_eq!(output[length + 1], C::Scalar::from(81));
                }
            }
        }
        let r = &key.metadata.permutations[2];
        let mut changed = bytes.clone();
        changed[r.bitmap.offset as usize] ^= 1;
        let mut output = vec![C::Scalar::ONE; key.rows()];
        assert!(
            key.copy_native_interval(
                &mut io::Cursor::new(changed.as_slice()),
                Id::PermutationLagrange(2),
                0,
                &mut output
            )
            .is_err()
        );
        assert!(output.iter().all(|v| *v == C::Scalar::ZERO));
        // Same ranked bitmap but an explicit self/out-of-range target is refused on reread.
        for target in [self_target(key.rows(), 2, 0).unwrap(), u32::MAX] {
            let mut changed = bytes.clone();
            changed[r.targets.offset as usize..r.targets.offset as usize + 4]
                .copy_from_slice(&target.to_le_bytes());
            output.fill(C::Scalar::ONE);
            assert!(
                key.copy_native_interval(
                    &mut io::Cursor::new(changed.as_slice()),
                    Id::PermutationLagrange(2),
                    0,
                    &mut output
                )
                .is_err()
            );
            assert!(output.iter().all(|v| *v == C::Scalar::ZERO));
        }
        let sparse = &key.metadata.permutations[1];
        for (at, value) in [
            (sparse.targets.offset, u32::try_from(key.rows()).unwrap()),
            (sparse.targets.offset + 8, 0),
            (
                sparse.targets.offset + 4,
                self_target(key.rows(), 1, 0).unwrap(),
            ),
            (sparse.targets.offset + 4, u32::MAX),
        ] {
            let mut changed = bytes.clone();
            changed[at as usize..at as usize + 4].copy_from_slice(&value.to_le_bytes());
            output.fill(C::Scalar::ONE);
            assert!(
                key.copy_native_interval(
                    &mut io::Cursor::new(changed.as_slice()),
                    Id::PermutationLagrange(1),
                    0,
                    &mut output
                )
                .is_err()
            );
            assert!(output.iter().all(|v| *v == C::Scalar::ZERO));
        }
        struct PanicRead;
        impl Read for PanicRead {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                panic!("rank-window read unwind")
            }
        }
        impl io::Seek for PanicRead {
            fn seek(&mut self, position: io::SeekFrom) -> io::Result<u64> {
                match position {
                    io::SeekFrom::Start(at) => Ok(at),
                    _ => unreachable!(),
                }
            }
        }
        output.fill(C::Scalar::ONE);
        key.copy_native_interval(&mut PanicRead, Id::PermutationLagrange(0), 0, &mut output)
            .unwrap();
        assert_eq!(output, expected.permutation.permutations[0][..]);
        for column in 1..=3 {
            output.fill(C::Scalar::ONE);
            assert!(
                catch_unwind(AssertUnwindSafe(|| key.copy_native_interval(
                    &mut PanicRead,
                    Id::PermutationLagrange(column),
                    0,
                    &mut output
                )))
                .is_err()
            );
            assert!(output.iter().all(|v| *v == C::Scalar::ZERO));
        }
    }
}
#[test]
fn both_pasta_original_sparse_bitmap_dense_and_identity_intervals_preserve_labels_and_clear_on_rank_failure()
 {
    intervals::<EqAffine>();
    intervals::<EpAffine>();
}
