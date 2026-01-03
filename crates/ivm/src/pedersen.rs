use std::sync::LazyLock;

use blstrs::{G1Projective, Scalar};
use group::{Curve, Group};

static H_GENERATOR: LazyLock<G1Projective> = LazyLock::new(|| {
    // Fixed second generator derived from the curve generator.
    G1Projective::generator() * Scalar::from(7u64)
});

fn to_u64(bytes: &[u8]) -> u64 {
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&bytes[..8]);
    u64::from_le_bytes(arr)
}

/// Compute a Pedersen commitment C = value*G + blind*H on BLS12-381.
/// Returns the full compressed G1 bytes to preserve precision for callers
/// that need the complete point.
pub fn pedersen_commit(value: u64, blind: u64) -> [u8; 48] {
    let part1 = G1Projective::generator() * Scalar::from(value);
    let part2 = *H_GENERATOR * Scalar::from(blind);
    let res = part1 + part2;
    res.to_affine().to_compressed()
}

/// Helper retaining the truncated representation used by VM registers.
/// Encodes the compressed point and returns the low 64 bits.
pub fn pedersen_commit_truncated(value: u64, blind: u64) -> u64 {
    let bytes = pedersen_commit(value, blind);
    to_u64(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pedersen_commit_preserves_bytes() {
        let bytes = pedersen_commit(5, 7);
        assert_eq!(bytes.len(), 48);
        let truncated = pedersen_commit_truncated(5, 7);
        assert_eq!(truncated, to_u64(&bytes));
    }
}
