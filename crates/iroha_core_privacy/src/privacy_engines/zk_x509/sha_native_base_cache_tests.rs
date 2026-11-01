//! Lossless all-column controls, malformed input refusal and real packed erasure.

use super::*;
use crate::privacy_engines::zk_x509::private_table::inspection;

fn original_row(index: usize) -> zeroize::Zeroizing<Row> {
    zeroize::Zeroizing::new(core::array::from_fn(|column| {
        if matches!(column, 0..=5 | 36..=39 | 75) {
            F(u64::from(u32::MAX) - u64::try_from(index * 89 + column).unwrap())
        } else {
            F(u64::try_from((index + column) % 2).unwrap())
        }
    }))
}

fn cache(rows: usize) -> ShaNativeBaseCacheV1 {
    ShaNativeBaseCacheV1::build_v1(rows, |visitor| {
        for index in 0..rows {
            visitor(index, &original_row(index))?;
        }
        Ok(())
    })
    .unwrap()
}

#[test]
fn native_base_cache_exact_forecast_and_all_column_projections() {
    assert_eq!(core::mem::size_of::<PackedRowV1>(), 64);
    assert!(ShaNativeBaseCacheV1::heap_forecast_v1(0).is_err());
    assert!(ShaNativeBaseCacheV1::heap_forecast_v1(usize::MAX).is_err());
    assert_eq!(
        ZkX509ShaBatchSegmentBaseSourceV1::native_base_cache_forecast_all_v1().unwrap(),
        134_217_728
    );
    let cache = cache(9);
    assert_eq!(
        cache.heap_bytes_v1(),
        ShaNativeBaseCacheV1::heap_forecast_v1(9).unwrap()
    );
    for index in 0..9 {
        assert_eq!(*cache.row_v1(index), *original_row(index));
        assert_eq!(cache.rows[index].words[11], 0);
    }
    for width in [1, 3, 8] {
        for first in (0..ZK_X509_SHA_BATCH_BASE_WIDTH_V1).step_by(width) {
            let count = width.min(ZK_X509_SHA_BATCH_BASE_WIDTH_V1 - first);
            let mut columns =
                PrivateTableV1::new(vec![vec![F(99); 9]; count], zeroize_field_rows_v1);
            let mut targets = columns
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>();
            let mut fills = sha_column_fill_batch_v1(&mut targets).unwrap();
            cache.fill_columns_v1(first, &mut fills).unwrap();
            finish_sha_column_fill_batch_v1(fills).unwrap();
            for (offset, column) in columns.iter().enumerate() {
                for (index, value) in column.iter().enumerate() {
                    assert_eq!(*value, original_row(index)[first + offset]);
                }
            }
        }
    }
}

#[test]
fn native_base_cache_rejects_nonboolean_and_overflow_in_every_original_column() {
    for column in 0..ZK_X509_SHA_BATCH_BASE_WIDTH_V1 {
        for invalid in [
            if matches!(column, 0..=5 | 36..=39 | 75) {
                1_u64 << 32
            } else {
                2
            },
            u64::MAX,
        ] {
            let mut original = original_row(0);
            original[column] = F(invalid);
            assert!(
                ShaNativeBaseCacheV1::build_v1(1, |visitor| visitor(0, &original)).is_err(),
                "column {column}"
            );
        }
    }
}

#[test]
fn native_base_cache_rejects_missing_extra_reordered_and_swallowed_errors() {
    for indices in [vec![], vec![0], vec![0, 1, 2], vec![1, 0], vec![0, 0, 1]] {
        assert!(
            ShaNativeBaseCacheV1::build_v1(2, |visitor| {
                for index in indices {
                    let _ = visitor(index, &original_row(0));
                }
                Ok(())
            })
            .is_err()
        );
    }
    assert!(
        ShaNativeBaseCacheV1::build_v1(1, |visitor| {
            let mut invalid = original_row(0);
            invalid[88] = F(2);
            let _ = visitor(0, &invalid);
            let _ = visitor(0, &original_row(0));
            Ok(())
        })
        .is_err()
    );
}

#[test]
fn native_base_cache_invalid_projection_clears_owned_destinations() {
    let cache = cache(3);
    for (first, count, rows) in [
        (89, 1, 3),
        (usize::MAX, 1, 3),
        (88, 2, 3),
        (0, 9, 3),
        (0, 1, 2),
    ] {
        let mut columns = vec![vec![F(99); rows]; count];
        let mut targets = columns
            .iter_mut()
            .map(Vec::as_mut_slice)
            .collect::<Vec<_>>();
        let mut fills = sha_column_fill_batch_v1(&mut targets).unwrap();
        assert!(cache.fill_columns_v1(first, &mut fills).is_err());
        drop(fills);
        assert!(columns.iter().flatten().all(|field| *field == F::ZERO));
    }
    assert!(cache.fill_columns_v1(0, &mut []).is_err());
}

#[test]
fn native_base_cache_clears_packed_words_bits_and_reserved_words_on_all_exits() {
    for exit in 0..3 {
        let ((), observations) = inspection::observe_v1(|| {
            let result = std::panic::catch_unwind(|| {
                let result = ShaNativeBaseCacheV1::build_v1(3, |visitor| {
                    visitor(0, &original_row(0))?;
                    if exit == 1 {
                        return Err(Error::Terminal);
                    }
                    assert!(exit != 2, "test unwind after private packed row");
                    visitor(1, &original_row(1))?;
                    visitor(2, &[F::ZERO; ZK_X509_SHA_BATCH_BASE_WIDTH_V1])?;
                    Ok(())
                });
                if exit == 0 {
                    assert!(result.is_ok());
                }
                if exit == 1 {
                    assert!(matches!(result, Err(Error::Terminal)));
                }
                drop(result);
            });
            assert_eq!(result.is_err(), exit == 2);
        });
        // Includes the explicitly initialized reserved word, both bitsets, and
        // the entire preinitialized tail even when construction fails early.
        assert_eq!(
            observations.iter().map(|entry| entry.cells).sum::<usize>(),
            3 * (12 + 2)
        );
        assert!(observations.iter().any(|entry| entry.nonzero_before > 0));
        assert!(observations.iter().all(|entry| entry.nonzero_after == 0));
    }
}
