//! Native P-256 (secp256r1) and SHA-256 reference arithmetic for the
//! operation census: witness values for the in-circuit ECDSA and SHA-256
//! gadgets, checked against these native computations.

use num_bigint::{BigInt, BigUint, Sign};
use num_integer::Integer;
use num_traits::{One, Zero};

fn hex(s: &str) -> BigUint {
    BigUint::parse_bytes(s.as_bytes(), 16).expect("hex constant")
}

pub fn p() -> BigUint {
    hex("ffffffff00000001000000000000000000000000ffffffffffffffffffffffff")
}
pub fn n() -> BigUint {
    hex("ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551")
}
pub fn b() -> BigUint {
    hex("5ac635d8aa3a93e7b3ebbd55769886bc651d06b0cc53b0f63bce3c3e27d2604b")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: BigUint,
    pub y: BigUint,
}

pub fn generator() -> Point {
    Point {
        x: hex("6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"),
        y: hex("4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"),
    }
}

pub fn modp(x: &BigInt, m: &BigUint) -> BigUint {
    let m = BigInt::from(m.clone());
    x.mod_floor(&m).to_biguint().expect("non-negative")
}

pub fn inv(x: &BigUint, m: &BigUint) -> BigUint {
    x.modpow(&(m - 2u32), m)
}

fn sub_mod(a: &BigUint, b: &BigUint, m: &BigUint) -> BigUint {
    ((a + m) - (b % m)) % m
}

pub fn on_curve(pt: &Point) -> bool {
    let p = p();
    let lhs = (&pt.y * &pt.y) % &p;
    let rhs = (&pt.x * &pt.x * &pt.x + (&p - 3u32) * &pt.x + b()) % &p;
    lhs == rhs
}

/// Affine addition of two points with distinct x (incomplete; `None` result
/// would be the point at infinity, which callers never produce).
pub fn add(a: &Point, c: &Point) -> Point {
    let p = p();
    assert_ne!(a.x, c.x, "incomplete addition: distinct x");
    let lam = sub_mod(&c.y, &a.y, &p) * inv(&sub_mod(&c.x, &a.x, &p), &p) % &p;
    let x3 = sub_mod(&sub_mod(&(&lam * &lam % &p), &a.x, &p), &c.x, &p);
    let y3 = sub_mod(&(&lam * sub_mod(&a.x, &x3, &p) % &p), &a.y, &p);
    Point { x: x3, y: y3 }
}

pub fn double(a: &Point) -> Point {
    let p = p();
    let num = (BigUint::from(3u32) * &a.x * &a.x + (&p - 3u32)) % &p;
    let lam = num * inv(&(BigUint::from(2u32) * &a.y % &p), &p) % &p;
    let x3 = sub_mod(&(&lam * &lam % &p), &(BigUint::from(2u32) * &a.x % &p), &p);
    let y3 = sub_mod(&(&lam * sub_mod(&a.x, &x3, &p) % &p), &a.y, &p);
    Point { x: x3, y: y3 }
}

pub fn neg(a: &Point) -> Point {
    let p = p();
    Point {
        x: a.x.clone(),
        y: (&p - &a.y) % &p,
    }
}

/// Complete scalar multiplication (handles the identity internally).
pub fn mul(base: &Point, k: &BigUint) -> Option<Point> {
    let mut acc: Option<Point> = None;
    for i in (0..k.bits()).rev() {
        acc = acc.map(|a| double(&a));
        if k.bit(i) {
            acc = Some(match acc {
                None => base.clone(),
                Some(a) if a.x == base.x => {
                    if a.y == base.y {
                        double(&a)
                    } else {
                        return None;
                    }
                }
                Some(a) => add(&a, base),
            });
        }
    }
    acc
}

pub fn mul_signed(base: &Point, k: &BigInt) -> Option<Point> {
    let m = mul(base, &k.magnitude().clone())?;
    Some(if k.sign() == Sign::Minus { neg(&m) } else { m })
}

pub fn add_any(a: Option<Point>, c: Option<Point>) -> Option<Point> {
    match (a, c) {
        (None, x) | (x, None) => x,
        (Some(a), Some(c)) => {
            if a.x == c.x {
                if a.y == c.y { Some(double(&a)) } else { None }
            } else {
                Some(add(&a, &c))
            }
        }
    }
}

pub struct Signature {
    pub r: BigUint,
    pub s: BigUint,
}

pub fn sign(d: &BigUint, z: &BigUint, k: &BigUint) -> Signature {
    let n = n();
    let rp = mul(&generator(), k).expect("k != 0");
    let r = &rp.x % &n;
    let s = inv(k, &n) * ((z + &r * d) % &n) % &n;
    assert!(!r.is_zero() && !s.is_zero());
    Signature { r, s }
}

/// Standard ECDSA verification: x(u1 G + u2 Q) mod n == r.
pub fn verify(q: &Point, z: &BigUint, sig: &Signature) -> bool {
    verified_point(q, z, &sig.r, &sig.s).is_some()
}

/// Return the ECDSA scalars `u1 = z/s`, `u2 = r/s` and the checked point
/// `u1 G + u2 Q` for comparison with the circuit's independent comb walk.
pub(crate) fn verified_point(
    q: &Point,
    z: &BigUint,
    r: &BigUint,
    s: &BigUint,
) -> Option<(BigUint, BigUint, Point)> {
    use ::p256::elliptic_curve::{
        group::ff::PrimeField, ops::LinearCombination, point::AffineCoordinates,
    };
    use ::p256::{AffinePoint, FieldBytes, ProjectivePoint, Scalar};

    let n = n();
    if r.is_zero() || s.is_zero() || r >= &n || s >= &n {
        return None;
    }
    let w = inv(s, &n);
    let u1 = z * &w % &n;
    let u2 = r * &w % &n;
    let field_bytes = |value: &BigUint| {
        let bytes = value.to_bytes_be();
        let mut word = FieldBytes::default();
        let start = word.len().checked_sub(bytes.len())?;
        word[start..].copy_from_slice(&bytes);
        Some(word)
    };
    let key = field_bytes(&q.x)
        .zip(field_bytes(&q.y))
        .and_then(|(x, y)| AffinePoint::from_coordinates(&x, &y).into_option());
    let point = if let Some(key) = key {
        let scalar = |u: &BigUint| {
            Scalar::from_repr(field_bytes(u).expect("reduced P-256 scalar fits 32 bytes"))
                .into_option()
                .expect("scalar below the group order")
        };
        let sum = ProjectivePoint::lincomb(&[
            (ProjectivePoint::GENERATOR, scalar(&u1)),
            (ProjectivePoint::from(key), scalar(&u2)),
        ]);
        if sum == ProjectivePoint::IDENTITY {
            return None;
        }
        let sum = sum.to_affine();
        Point {
            x: BigUint::from_bytes_be(&sum.x()),
            y: BigUint::from_bytes_be(&sum.y()),
        }
    } else {
        // Protocol callers validate their keys. Direct arithmetic callers can
        // supply raw coordinates, on which the original affine formulas operate.
        add_any(mul(&generator(), &u1), mul(q, &u2))?
    };
    (&point.x % &n == *r).then_some((u1, u2, point))
}

/// Short (v, v') with v = v' u (mod n), both odd, v' != 0 (Lagrange-Gauss
/// reduction of the lattice {(a, b): a = b u mod n}); |v|, |v'| ~ sqrt(n).
pub fn half_scalars(u: &BigUint) -> (BigInt, BigInt) {
    let n = BigInt::from(n());
    let dot = |a: &(BigInt, BigInt), b: &(BigInt, BigInt)| &a.0 * &b.0 + &a.1 * &b.1;
    let mut e1 = (n.clone(), BigInt::zero());
    let mut e2 = (BigInt::from(u.clone()), BigInt::one());
    loop {
        if dot(&e1, &e1) > dot(&e2, &e2) {
            std::mem::swap(&mut e1, &mut e2);
        }
        // m = round(<e1, e2> / <e1, e1>)
        let num = dot(&e1, &e2);
        let den = dot(&e1, &e1);
        let m = (BigInt::from(2) * &num + &den).div_floor(&(BigInt::from(2) * &den));
        if m.is_zero() {
            break;
        }
        e2 = (&e2.0 - &m * &e1.0, &e2.1 - &m * &e1.1);
    }
    let odd = |x: &BigInt| x.is_odd();
    let sum = (&e1.0 + &e2.0, &e1.1 + &e2.1);
    for cand in [e1.clone(), e2.clone(), sum] {
        if odd(&cand.0) && odd(&cand.1) && !cand.1.is_zero() {
            let check = (&cand.0 - &cand.1 * BigInt::from(u.clone())).mod_floor(&n);
            assert!(check.is_zero(), "lattice vector satisfies v = v' u mod n");
            return cand;
        }
    }
    unreachable!("one of e1, e2, e1 + e2 has both coordinates odd")
}

/// Deterministic test scalar below `m` from a label.
pub fn scalar_below(label: &str, m: &BigUint) -> BigUint {
    let mut out = Vec::new();
    for i in 0u32..2 {
        let mut h = blake3::Hasher::new();
        h.update(label.as_bytes());
        h.update(&i.to_le_bytes());
        out.extend_from_slice(h.finalize().as_bytes());
    }
    BigUint::from_bytes_le(&out) % m
}

// --- SHA-256 (FIPS 180-4) ---

pub const SHA_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];
pub const SHA_IV: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

pub fn sha256_compress(state: &mut [u32; 8], block: &[u32; 16]) {
    let mut w = [0u32; 64];
    w[..16].copy_from_slice(block);
    for t in 16..64 {
        let s0 = w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ (w[t - 15] >> 3);
        let s1 = w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ (w[t - 2] >> 10);
        w[t] = w[t - 16]
            .wrapping_add(s0)
            .wrapping_add(w[t - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for t in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(SHA_K[t])
            .wrapping_add(w[t]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (s, v) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
        *s = s.wrapping_add(v);
    }
}

/// SHA-256 of a message of at most 55 bytes (one block).
pub fn sha256_one_block(msg: &[u8]) -> [u8; 32] {
    assert!(msg.len() <= 55);
    let mut bytes = [0u8; 64];
    bytes[..msg.len()].copy_from_slice(msg);
    bytes[msg.len()] = 0x80;
    bytes[56..].copy_from_slice(&((msg.len() as u64) * 8).to_be_bytes());
    let mut block = [0u32; 16];
    for (i, w) in block.iter_mut().enumerate() {
        *w = u32::from_be_bytes(bytes[4 * i..4 * i + 4].try_into().unwrap());
    }
    let mut state = SHA_IV;
    sha256_compress(&mut state, &block);
    let mut out = [0u8; 32];
    for (i, w) in state.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&w.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_abc() {
        let d = sha256_one_block(b"abc");
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(
            d.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            expected
        );
    }

    #[test]
    fn generator_order() {
        assert!(on_curve(&generator()));
        assert!(mul(&generator(), &n()).is_none());
    }

    #[test]
    fn ecdsa_roundtrip_and_half_scalars() {
        let n = n();
        let d = scalar_below("d", &n);
        let q = mul(&generator(), &d).unwrap();
        let z = scalar_below("z", &n);
        let sig = sign(&d, &z, &scalar_below("k", &n));
        assert!(verify(&q, &z, &sig));
        let u2 = &sig.r * inv(&sig.s, &n) % &n;
        let (v, vp) = half_scalars(&u2);
        assert!(v.bits() <= 130 && vp.bits() <= 130);
    }

    #[test]
    fn verified_point_matches_affine_scalar_products() {
        let n = n();
        let d = scalar_below("verified key", &n);
        let q = mul(&generator(), &d).unwrap();
        let k = scalar_below("verified nonce", &n);
        let nonce_point = mul(&generator(), &k).unwrap();
        for z in [BigUint::zero(), &n + scalar_below("verified digest", &n)] {
            let sig = sign(&d, &z, &k);
            for (s, expected) in [
                (sig.s.clone(), nonce_point.clone()),
                (&n - &sig.s, neg(&nonce_point)),
            ] {
                let (u1, u2, checked) = verified_point(&q, &z, &sig.r, &s).unwrap();
                let w = inv(&s, &n);
                let scalars = (&z * &w % &n, &sig.r * &w % &n);
                let affine = add_any(mul(&generator(), &scalars.0), mul(&q, &scalars.1));
                assert_eq!((u1, u2), scalars);
                assert_eq!(Some(&checked), affine.as_ref());
                assert_eq!(checked, expected);
            }
        }
        // A canonical coordinate can exceed the group order. Compare x modulo n.
        let three = BigUint::from(3u32);
        let q = Point {
            x: &n + &three,
            y: hex("484f0c0fda434ef0a808458914f328715d7a545e198ac7eee31dffe861b5d23f"),
        };
        assert!(on_curve(&q));
        assert_eq!(
            verified_point(&q, &BigUint::zero(), &three, &three),
            Some((BigUint::zero(), BigUint::one(), q))
        );
    }

    #[test]
    fn verified_point_rejects_invalid_scalars_and_points() {
        let n = n();
        let g = generator();
        let z = BigUint::from(9u32);
        let sig = sign(&BigUint::one(), &z, &BigUint::from(7u32));
        let rejects = |q: &Point, z: &BigUint, r: &BigUint, s: &BigUint| {
            assert!(verified_point(q, z, r, s).is_none());
            assert!(!verify(
                q,
                z,
                &Signature {
                    r: r.clone(),
                    s: s.clone()
                }
            ));
        };
        for invalid in [BigUint::zero(), n.clone(), &n + 1u32] {
            rejects(&g, &z, &invalid, &sig.s);
            rejects(&g, &z, &sig.r, &invalid);
        }
        rejects(&g, &(&z + 1u32), &sig.r, &sig.s);
        // u1 G + u2 Q = (n - 1)G + G is the identity.
        rejects(&g, &(&n - 1u32), &BigUint::one(), &BigUint::one());

        // Arbitrary coordinates can reach the arithmetic API directly.
        // Match its affine reference on off-curve and oversized coordinates.
        let w = inv(&sig.s, &n);
        let u1 = &z * &w % &n;
        let u2 = &sig.r * &w % &n;
        for key in [
            Point {
                x: g.x.clone(),
                y: &g.y + 1u32,
            },
            Point {
                x: (&g.x << 256) + &g.x,
                y: g.y.clone(),
            },
        ] {
            let affine =
                add_any(mul(&g, &u1), mul(&key, &u2)).filter(|point| &point.x % &n == sig.r);
            assert_eq!(
                verified_point(&key, &z, &sig.r, &sig.s).map(|v| v.2),
                affine
            );
        }
    }
}
