#![allow(clippy::needless_range_loop)]
#![allow(clippy::non_std_lazy_statics)]
//! SM2 Annex D Example 1 verification using the original Fp‑256 parameters.
//!
//! This harness keeps the canonical GM/T 0003‑2012 Annex D Example 1 vector in
//! the test suite without depending on external CLI tools. It reproduces the
//! curve parameters, the ZA computation (SM3), and the SM2 verification steps.
//! This code is **not** constant‑time and is intended for regression testing only.

#[cfg(feature = "sm")]
use std::fmt::Write as _;

#[cfg(feature = "sm")]
use hex::FromHex;
#[cfg(feature = "sm")]
use num_bigint::BigUint;
#[cfg(feature = "sm")]
use sm3::{Digest, Sm3};

#[cfg(feature = "sm")]
#[derive(Clone, Debug)]
struct Point {
    x: BigUint,
    y: BigUint,
    infinity: bool,
}

#[cfg(feature = "sm")]
fn bu(hex_: &str) -> BigUint {
    BigUint::from_bytes_be(&Vec::from_hex(hex_).expect("hex to bytes"))
}

#[cfg(feature = "sm")]
fn be(bytes: &[u8]) -> BigUint {
    BigUint::from_bytes_be(bytes)
}

#[cfg(feature = "sm")]
fn to_fixed_32(x: &BigUint) -> [u8; 32] {
    let v = x.to_bytes_be();
    assert!(v.len() <= 32, "value does not fit into 32 bytes");
    let mut out = [0u8; 32];
    out[32 - v.len()..].copy_from_slice(&v);
    out
}

// Annex D Example 1 (older Fp-256 parameters, see GM/T 0003-2012).
#[cfg(feature = "sm")]
lazy_static::lazy_static! {
    static ref ZERO: BigUint = BigUint::default();
    static ref ONE: BigUint = BigUint::from(1u32);
    static ref P:  BigUint = bu("8542D69E4C044F18E8B92435BF6FF7DE457283915C45517D722EDB8B08F1DFC3");
    static ref A:  BigUint = bu("787968B4FA32C3FD2417842E73BBFEFF2F3C848B6831D7E0EC65228B3937E498");
    static ref B:  BigUint = bu("63E4C6D3B23B0C849CF84241484BFE48F61D59A5B16BA06E6E12D1DA27C5249A");
    static ref N:  BigUint = bu("8542D69E4C044F18E8B92435BF6FF7DD297720630485628D5AE74EE7C32E79B7");
    static ref GX: BigUint = bu("421DEBD61B62EAB6746434EBC3CC315E32220B3BADD50BDC4C4E6C147FEDD43D");
    static ref GY: BigUint = bu("0680512BCBB42C07D47349D2153B70C4E5D7FDFCBFA36EA1A85841B9E46E09A2");
    static ref G:  Point   = Point { x: GX.clone(), y: GY.clone(), infinity: false };
}

#[cfg(feature = "sm")]
fn mod_add(a: &BigUint, b: &BigUint, m: &BigUint) -> BigUint {
    (a + b) % m
}

#[cfg(feature = "sm")]
fn mod_sub(a: &BigUint, b: &BigUint, m: &BigUint) -> BigUint {
    if a >= b { (a - b) % m } else { (a + m - b) % m }
}

#[cfg(feature = "sm")]
fn mod_mul(a: &BigUint, b: &BigUint, m: &BigUint) -> BigUint {
    (a * b) % m
}

#[cfg(feature = "sm")]
fn mod_inv(a: &BigUint, m: &BigUint) -> BigUint {
    // Since p is prime, we can use Fermat's little theorem.
    a.modpow(&(m - BigUint::from(2u32)), m)
}

#[cfg(feature = "sm")]
fn is_on_curve(p: &Point) -> bool {
    if p.infinity {
        return true;
    }
    let lhs = mod_mul(&p.y, &p.y, &P);
    let x2 = mod_mul(&p.x, &p.x, &P);
    let x3 = mod_mul(&x2, &p.x, &P);
    let rhs = mod_add(&mod_add(&x3, &mod_mul(&A, &p.x, &P), &P), &B, &P);
    lhs == rhs
}

#[cfg(feature = "sm")]
fn point_add(p: &Point, q: &Point) -> Point {
    if p.infinity {
        return q.clone();
    }
    if q.infinity {
        return p.clone();
    }

    if p.x == q.x {
        let y_sum = mod_add(&p.y, &q.y, &P);
        if y_sum == *ZERO {
            return Point {
                x: ZERO.clone(),
                y: ZERO.clone(),
                infinity: true,
            };
        }
        let three = BigUint::from(3u32);
        let two = BigUint::from(2u32);
        let slope_num = mod_add(&mod_mul(&three, &mod_mul(&p.x, &p.x, &P), &P), &A, &P);
        let slope_den = mod_inv(&mod_mul(&two, &p.y, &P), &P);
        let slope = mod_mul(&slope_num, &slope_den, &P);
        let x_r = mod_sub(&mod_sub(&mod_mul(&slope, &slope, &P), &p.x, &P), &q.x, &P);
        let y_r = mod_sub(&mod_mul(&slope, &mod_sub(&p.x, &x_r, &P), &P), &p.y, &P);
        return Point {
            x: x_r,
            y: y_r,
            infinity: false,
        };
    }

    let slope_num = mod_sub(&q.y, &p.y, &P);
    let slope_den = mod_inv(&mod_sub(&q.x, &p.x, &P), &P);
    let slope = mod_mul(&slope_num, &slope_den, &P);
    let x_r = mod_sub(&mod_sub(&mod_mul(&slope, &slope, &P), &p.x, &P), &q.x, &P);
    let y_r = mod_sub(&mod_mul(&slope, &mod_sub(&p.x, &x_r, &P), &P), &p.y, &P);
    Point {
        x: x_r,
        y: y_r,
        infinity: false,
    }
}

#[cfg(feature = "sm")]
fn point_mul(k: &BigUint, p: &Point) -> Point {
    let mut n = k.clone();
    let mut acc = Point {
        x: ZERO.clone(),
        y: ZERO.clone(),
        infinity: true,
    };
    let mut base = p.clone();
    while n != *ZERO {
        if (&n & &*ONE) == *ONE {
            acc = point_add(&acc, &base);
        }
        base = point_add(&base, &base);
        n >>= 1;
    }
    acc
}

#[cfg(feature = "sm")]
fn compute_za(id: &[u8], pub_x: &BigUint, pub_y: &BigUint) -> [u8; 32] {
    let mut z = Sm3::new();
    let entla_bits = u32::try_from(id.len()).expect("distid length fits in u32 bytes") * 8;
    let entla = u16::try_from(entla_bits)
        .expect("distid length fits in u16 bits")
        .to_be_bytes();
    z.update(entla);
    z.update(id);

    z.update(to_fixed_32(&A));
    z.update(to_fixed_32(&B));
    z.update(to_fixed_32(&GX));
    z.update(to_fixed_32(&GY));
    z.update(to_fixed_32(pub_x));
    z.update(to_fixed_32(pub_y));

    let out = z.finalize();
    let mut za = [0u8; 32];
    za.copy_from_slice(&out);
    za
}

#[cfg(feature = "sm")]
fn verify_annex(distid: &[u8], message: &[u8], public_key_sec1: &[u8], sig_rs: &[u8]) -> bool {
    assert!(public_key_sec1.len() == 65 && public_key_sec1[0] == 0x04);
    assert!(sig_rs.len() == 64);

    let pub_key_x = be(&public_key_sec1[1..33]);
    let pub_key_y = be(&public_key_sec1[33..65]);
    let public_point = Point {
        x: pub_key_x.clone(),
        y: pub_key_y.clone(),
        infinity: false,
    };
    if !is_on_curve(&public_point) {
        return false;
    }

    let za = compute_za(distid, &pub_key_x, &pub_key_y);
    let mut message_hasher = Sm3::new();
    message_hasher.update(za.as_slice());
    message_hasher.update(message);
    let e_digest = be(&message_hasher.finalize());

    let sig_r = be(&sig_rs[..32]);
    let sig_s_scalar = be(&sig_rs[32..]);
    if sig_r == *ZERO || sig_r >= *N || sig_s_scalar == *ZERO || sig_s_scalar >= *N {
        return false;
    }

    let r_plus_s = (&sig_r + &sig_s_scalar) % &*N;
    if r_plus_s == *ZERO {
        return false;
    }

    let sig_s_times_g = point_mul(&sig_s_scalar, &G);
    let r_plus_s_times_public = point_mul(&r_plus_s, &public_point);
    let combined = point_add(&sig_s_times_g, &r_plus_s_times_public);
    if combined.infinity {
        return false;
    }

    let expected_r = (e_digest + &combined.x) % &*N;
    expected_r == sig_r
}

#[cfg(feature = "sm")]
#[test]
fn annex_example_1_verifies() {
    let distid = b"ALICE123@YAHOO.COM";
    let message = b"message digest";

    // Public key (uncompressed SEC1).
    let pubkey =
        Vec::from_hex("040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857")
            .expect("public key hex");

    // Signature r||s.
    let sig =
        Vec::from_hex("40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D16FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7")
            .expect("signature hex");

    assert!(verify_annex(distid, message, &pubkey, &sig));
}

#[cfg(feature = "sm")]
#[test]
fn annex_example_1_za_and_e_match_fixture() {
    let distid = b"ALICE123@YAHOO.COM";
    let message = b"message digest";
    let pubkey =
        Vec::from_hex("040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857")
            .expect("public key hex");

    let pub_key_x = be(&pubkey[1..33]);
    let pub_key_y = be(&pubkey[33..65]);

    let za = compute_za(distid, &pub_key_x, &pub_key_y);
    let mut za_hex = String::with_capacity(za.len() * 2);
    for byte in &za {
        write!(za_hex, "{byte:02X}").expect("write hex");
    }
    assert_eq!(
        za_hex, "F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A",
        "ZA must match Annex D Example 1 fixture"
    );

    let mut message_hasher = Sm3::new();
    message_hasher.update(za.as_slice());
    message_hasher.update(message);
    let e_bytes = message_hasher.finalize();
    let mut e_hex = String::with_capacity(e_bytes.len() * 2);
    for byte in &e_bytes {
        write!(e_hex, "{byte:02X}").expect("write hex");
    }
    assert_eq!(
        e_hex, "B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76",
        "e digest must match Annex D Example 1 fixture"
    );
}
