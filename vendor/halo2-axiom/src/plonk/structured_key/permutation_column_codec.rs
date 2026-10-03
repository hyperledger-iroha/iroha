//! Canonical mode and payload sizes for exact directed permutation-column targets.
//!
//! This helper chooses a representation only. The writer and readers retain
//! each exact target and own ordered exceptions, bitmap padding, target bounds,
//! global bijection, authentication and complete-frame consumption.

use std::io;

/// Canonical wire modes, in the tie-breaking order used by the writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum PermutationColumnMode {
    /// Every row targets its own cell; there is no payload.
    Identity = 0,
    /// LE32 exception count followed by ordered LE32 source/target pairs.
    Sparse = 1,
    /// A low-bit-first source bitmap followed by exact LE32 exception targets.
    Bitmap = 2,
    /// One exact LE32 directed target for every row.
    Dense = 3,
}

/// A canonical column representation and its payload size, excluding its mode tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PermutationColumnEncoding {
    /// Canonical mode selected by exact byte size and the fixed tie order.
    pub(crate) mode: PermutationColumnMode,
    /// Exact payload size in bytes; it does not depend on the host pointer width.
    pub(crate) payload_bytes: u64,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn checked_shape(rows: u64, exceptions: u64) -> io::Result<()> {
    if rows == 0 || !rows.is_power_of_two() || u32::try_from(rows).is_err() {
        return Err(invalid("unsupported permutation-column row shape"));
    }
    if exceptions > rows {
        return Err(invalid("permutation-column exception count exceeds rows"));
    }
    Ok(())
}

fn checked_payload(prefix: u64, stride: u64, count: u64) -> io::Result<u64> {
    stride
        .checked_mul(count)
        .and_then(|bytes| prefix.checked_add(bytes))
        .ok_or_else(|| invalid("permutation-column payload size overflow"))
}

/// Return an exact mode payload size for a supported power-of-two row shape.
///
/// `exceptions` counts sources whose exact directed target differs from their
/// own global cell ID. Identity is valid only when that count is zero. This
/// function does not make a nonminimal mode canonical.
pub(crate) fn permutation_column_payload_bytes(
    mode: PermutationColumnMode,
    rows: u64,
    exceptions: u64,
) -> io::Result<u64> {
    checked_shape(rows, exceptions)?;
    match mode {
        PermutationColumnMode::Identity if exceptions == 0 => Ok(0),
        PermutationColumnMode::Identity => Err(invalid(
            "identity permutation column contains exceptional targets",
        )),
        PermutationColumnMode::Sparse => checked_payload(4, 8, exceptions),
        PermutationColumnMode::Bitmap => checked_payload(rows.div_ceil(8), 4, exceptions),
        PermutationColumnMode::Dense => checked_payload(0, 4, rows),
    }
}

/// Choose the smallest exact payload, breaking ties Identity, Sparse, Bitmap, Dense.
///
/// All calculations use checked fixed-width byte counts. No target is changed,
/// discarded or inferred by selecting its storage representation.
pub(crate) fn canonical_permutation_column_encoding(
    rows: u64,
    exceptions: u64,
) -> io::Result<PermutationColumnEncoding> {
    checked_shape(rows, exceptions)?;
    if exceptions == 0 {
        return Ok(PermutationColumnEncoding {
            mode: PermutationColumnMode::Identity,
            payload_bytes: 0,
        });
    }
    let mut best = PermutationColumnEncoding {
        mode: PermutationColumnMode::Sparse,
        payload_bytes: permutation_column_payload_bytes(
            PermutationColumnMode::Sparse,
            rows,
            exceptions,
        )?,
    };
    for mode in [PermutationColumnMode::Bitmap, PermutationColumnMode::Dense] {
        let payload_bytes = permutation_column_payload_bytes(mode, rows, exceptions)?;
        if payload_bytes < best.payload_bytes {
            best = PermutationColumnEncoding {
                mode,
                payload_bytes,
            };
        }
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_exception_sets_have_only_the_identity_canonical_encoding() {
        for rows in [1, 2, 8, 64, 65_536, 1 << 31] {
            let selected = canonical_permutation_column_encoding(rows, 0).unwrap();
            assert_eq!(selected.mode, PermutationColumnMode::Identity);
            assert_eq!(selected.payload_bytes, 0);
            assert_eq!(
                permutation_column_payload_bytes(PermutationColumnMode::Identity, rows, 0).unwrap(),
                0
            );
        }
        assert!(permutation_column_payload_bytes(PermutationColumnMode::Identity, 64, 1).is_err());
    }

    #[test]
    fn exact_payload_lengths_include_sparse_count_and_bitmap_rounding() {
        assert_eq!(
            permutation_column_payload_bytes(PermutationColumnMode::Sparse, 64, 1).unwrap(),
            12
        );
        assert_eq!(
            permutation_column_payload_bytes(PermutationColumnMode::Bitmap, 64, 1).unwrap(),
            12
        );
        assert_eq!(
            permutation_column_payload_bytes(PermutationColumnMode::Dense, 64, 1).unwrap(),
            256
        );
        for rows in [1, 2, 4, 8] {
            assert_eq!(
                permutation_column_payload_bytes(PermutationColumnMode::Bitmap, rows, 0).unwrap(),
                1
            );
        }
        assert_eq!(
            permutation_column_payload_bytes(PermutationColumnMode::Sparse, 8, 0).unwrap(),
            4
        );
    }

    #[test]
    fn density_boundaries_and_equal_payloads_use_the_declared_tie_order() {
        for (rows, exceptions, mode, bytes) in [
            (1, 1, PermutationColumnMode::Dense, 4),
            (8, 1, PermutationColumnMode::Bitmap, 5),
            (64, 1, PermutationColumnMode::Sparse, 12),
            (64, 2, PermutationColumnMode::Bitmap, 16),
            (64, 62, PermutationColumnMode::Bitmap, 256),
            (64, 63, PermutationColumnMode::Dense, 256),
            (128, 3, PermutationColumnMode::Sparse, 28),
            (256, 7, PermutationColumnMode::Sparse, 60),
        ] {
            assert_eq!(
                canonical_permutation_column_encoding(rows, exceptions).unwrap(),
                PermutationColumnEncoding {
                    mode,
                    payload_bytes: bytes
                }
            );
        }
    }

    #[test]
    fn full_density_preserves_the_exact_four_bytes_per_row_payload() {
        for rows in [1, 2, 8, 64, 65_536, 1 << 31] {
            assert_eq!(
                canonical_permutation_column_encoding(rows, rows).unwrap(),
                PermutationColumnEncoding {
                    mode: PermutationColumnMode::Dense,
                    payload_bytes: 4 * rows
                }
            );
        }
        assert_eq!(
            canonical_permutation_column_encoding(1 << 31, 1)
                .unwrap()
                .payload_bytes,
            12
        );
    }

    #[test]
    fn unsupported_shapes_and_excess_exception_counts_are_rejected() {
        for (rows, exceptions) in [
            (0, 0),
            (3, 0),
            (6, 1),
            (1 << 32, 0),
            (u64::MAX, 0),
            (8, 9),
            (8, u64::MAX),
        ] {
            assert_eq!(
                canonical_permutation_column_encoding(rows, exceptions)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidData
            );
            for mode in [
                PermutationColumnMode::Identity,
                PermutationColumnMode::Sparse,
                PermutationColumnMode::Bitmap,
                PermutationColumnMode::Dense,
            ] {
                assert!(permutation_column_payload_bytes(mode, rows, exceptions).is_err());
            }
        }
    }

    #[test]
    fn checked_payload_rejects_multiplication_and_prefix_addition_overflow() {
        assert!(checked_payload(0, 8, u64::MAX).is_err());
        assert!(checked_payload(4, 1, u64::MAX).is_err());
        assert_eq!(checked_payload(4, 8, 1).unwrap(), 12);
    }
}
