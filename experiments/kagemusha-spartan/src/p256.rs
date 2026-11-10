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
    let n = n();
    if sig.r.is_zero() || sig.s.is_zero() || sig.r >= n || sig.s >= n {
        return false;
    }
    let w = inv(&sig.s, &n);
    let u1 = z * &w % &n;
    let u2 = &sig.r * &w % &n;
    match add_any(mul(&generator(), &u1), mul(q, &u2)) {
        None => false,
        Some(rp) => &rp.x % &n == sig.r,
    }
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
}
