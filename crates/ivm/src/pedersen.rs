use blstrs::{G1Affine, G1Projective, Scalar};
use group::{Curve, Group};
use ivm_abi::private_input::PRIVATE_NUMERIC_VALCOM_H_COMPRESSED_V1;
use std::sync::LazyLock;
static H_GENERATOR: LazyLock<G1Projective> = LazyLock::new(|| {
    let affine = Option::<G1Affine>::from(G1Affine::from_compressed(
        &PRIVATE_NUMERIC_VALCOM_H_COMPRESSED_V1,
    ))
    .expect("ABI V1 Pedersen H generator must be a canonical subgroup point");
    G1Projective::from(affine)
});
/// Compute a Pedersen commitment C = value*G + blind*H on BLS12-381. Returns the full compressed G1
/// bytes to preserve precision for callers that need the complete point.
pub fn pedersen_commit(value: u64, blind: u64) -> [u8; 48] {
    pedersen_commit_scalars(Scalar::from(value), Scalar::from(blind))
}
/// Compute a full-width Pedersen commitment from canonical BLS12-381 scalars.
///
/// This helper never truncates the compressed point and is used by the typed
/// private-input commitment boundary.
pub fn pedersen_commit_scalars(value: Scalar, blind: Scalar) -> [u8; 48] {
    let part1 = G1Projective::generator() * value;
    let part2 = *H_GENERATOR * blind;
    let res = part1 + part2;
    res.to_affine().to_compressed()
}
#[cfg(test)]
mod tests {
    use super::*;
    use ivm_abi::private_input::{
        PRIVATE_NUMERIC_VALCOM_H_DST_V1, PRIVATE_NUMERIC_VALCOM_H_MESSAGE_V1,
    };
    #[test]
    fn pedersen_commit_preserves_bytes() {
        let bytes = pedersen_commit(5, 7);
        assert_eq!(bytes.len(), 48);
        assert!(bool::from(G1Affine::from_compressed(&bytes).is_some()));
        assert_eq!(
            bytes,
            pedersen_commit_scalars(Scalar::from(5_u64), Scalar::from(7_u64))
        );
    }
    #[test]
    fn independent_generator_is_neither_identity_nor_g() {
        assert!(!bool::from(H_GENERATOR.is_identity()));
        assert_ne!(*H_GENERATOR, G1Projective::generator());
    }
    #[test]
    fn fixed_generator_matches_the_v1_hash_to_curve_derivation() {
        let derived = G1Projective::hash_to_curve(
            PRIVATE_NUMERIC_VALCOM_H_MESSAGE_V1,
            PRIVATE_NUMERIC_VALCOM_H_DST_V1,
            &[],
        )
        .to_affine()
        .to_compressed();
        assert_eq!(derived, PRIVATE_NUMERIC_VALCOM_H_COMPRESSED_V1);
        assert_eq!(H_GENERATOR.to_affine().to_compressed(), derived);
    }
    #[test]
    fn retired_known_relation_collision_no_longer_holds() {
        // With the retired H = 7G construction these pairs both reduced to
        // (v + 7b)G: (5, 3) and (12, 2).
        assert_ne!(pedersen_commit(5, 3), pedersen_commit(12, 2));
    }
}
