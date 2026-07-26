use blstrs::{Compress, G1Projective, G2Projective, Scalar, pairing};
use group::{Curve, Group};

fn to_u64(bytes: &[u8]) -> u64 {
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&bytes[..8]);
    u64::from_le_bytes(arr)
}

/// EC add on BLS12-381 G1 returning the full compressed point bytes.
pub fn ec_add(a: u64, b: u64) -> [u8; 48] {
    let p = G1Projective::generator() * Scalar::from(a);
    let q = G1Projective::generator() * Scalar::from(b);
    let r = p + q;
    r.to_affine().to_compressed()
}

/// EC add returning the truncated u64 used by VM registers.
pub fn ec_add_truncated(a: u64, b: u64) -> u64 {
    to_u64(&ec_add(a, b))
}

/// Scalar multiplication on BLS12-381 G1 returning the full compressed point bytes.
pub fn ec_mul(point_scalar: u64, scalar: u64) -> [u8; 48] {
    let p = G1Projective::generator() * Scalar::from(point_scalar);
    let r = p * Scalar::from(scalar);
    r.to_affine().to_compressed()
}

/// Scalar multiplication returning the truncated u64 used by VM registers.
pub fn ec_mul_truncated(point_scalar: u64, scalar: u64) -> u64 {
    to_u64(&ec_mul(point_scalar, scalar))
}

/// Pairing check output compressed GT bytes (length defined by blstrs).
pub fn pairing_check(a: u64, b: u64) -> Vec<u8> {
    let p = G1Projective::generator() * Scalar::from(a);
    let q = G2Projective::generator() * Scalar::from(b);
    let gt = pairing(&p.to_affine(), &q.to_affine());
    let mut buf = Vec::new();
    gt.write_compressed(&mut buf).unwrap();
    buf
}

/// Pairing check returning the truncated u64 used by VM registers.
pub fn pairing_check_truncated(a: u64, b: u64) -> u64 {
    let buf = pairing_check(a, b);
    to_u64(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ec_ops_truncate_match_bytes() {
        let add_bytes = ec_add(2, 3);
        assert_eq!(ec_add_truncated(2, 3), to_u64(&add_bytes));

        let mul_bytes = ec_mul(5, 7);
        assert_eq!(ec_mul_truncated(5, 7), to_u64(&mul_bytes));

        let pair_bytes = pairing_check(2, 3);
        assert!(pair_bytes.len() >= 8);
        assert_eq!(pairing_check_truncated(2, 3), to_u64(&pair_bytes));
    }
}
