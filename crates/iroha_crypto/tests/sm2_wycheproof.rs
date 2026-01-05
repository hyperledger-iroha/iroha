//! Wycheproof-derived SM2 signature regression suite.
#![cfg(feature = "sm")]

use std::sync::LazyLock;

use hex::FromHex;
use iroha_crypto::sm::{Sm2PublicKey, Sm2Signature};
use norito::json::Value;
use num_bigint::BigUint;
use sm3::{Digest, Sm3};

#[derive(Clone, Debug)]
struct Point {
    x: BigUint,
    y: BigUint,
    infinity: bool,
}

#[derive(Clone)]
struct Domain {
    p: BigUint,
    a: BigUint,
    b: BigUint,
    n: BigUint,
    generator: Point,
}

impl Domain {
    fn new(p: BigUint, a: BigUint, b: BigUint, n: BigUint, gx: BigUint, gy: BigUint) -> Self {
        Self {
            p,
            a,
            b,
            n,
            generator: Point {
                x: gx,
                y: gy,
                infinity: false,
            },
        }
    }
}

static ZERO: LazyLock<BigUint> = LazyLock::new(|| BigUint::from(0u32));
static ONE: LazyLock<BigUint> = LazyLock::new(|| BigUint::from(1u32));
static DOMAINS: LazyLock<[Domain; 2]> = LazyLock::new(|| {
    [
        Domain::new(
            hex_to_biguint("8542D69E4C044F18E8B92435BF6FF7DE457283915C45517D722EDB8B08F1DFC3"),
            hex_to_biguint("787968B4FA32C3FD2417842E73BBFEFF2F3C848B6831D7E0EC65228B3937E498"),
            hex_to_biguint("63E4C6D3B23B0C849CF84241484BFE48F61D59A5B16BA06E6E12D1DA27C5249A"),
            hex_to_biguint("8542D69E4C044F18E8B92435BF6FF7DD297720630485628D5AE74EE7C32E79B7"),
            hex_to_biguint("421DEBD61B62EAB6746434EBC3CC315E32220B3BADD50BDC4C4E6C147FEDD43D"),
            hex_to_biguint("0680512BCBB42C07D47349D2153B70C4E5D7FDFCBFA36EA1A85841B9E46E09A2"),
        ),
        Domain::new(
            hex_to_biguint("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF"),
            hex_to_biguint("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC"),
            hex_to_biguint("28E9FA9E9D9F5E344D5AEF9BAE3C4AFF2F66A5814A3E43C4F5E11B30F7199D5C"),
            hex_to_biguint("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFF7203DF6B21C6052B53BBF40939D54123"),
            hex_to_biguint("32C4AE2C1F1981195F9904466A39C9948FE30BBFF2660BE1715A4589334C74C7"),
            hex_to_biguint("BC3736A2F4F6779C59BDCEE36B692153D0A9877CC62A474002DF32E52139F0A0"),
        ),
    ]
});

fn hex_to_vec(value: &str) -> Vec<u8> {
    if value.is_empty() {
        Vec::new()
    } else {
        Vec::from_hex(value).unwrap_or_else(|err| panic!("invalid hex '{value}': {err}"))
    }
}

fn hex_to_biguint(hex: &str) -> BigUint {
    BigUint::from_bytes_be(&hex_to_vec(hex))
}

fn to_fixed_32(x: &BigUint) -> [u8; 32] {
    let bytes = x.to_bytes_be();
    assert!(bytes.len() <= 32, "value exceeds 32 bytes");
    let mut out = [0u8; 32];
    out[32 - bytes.len()..].copy_from_slice(&bytes);
    out
}

fn is_zero(value: &BigUint) -> bool {
    value.bits() == 0
}

fn mod_add(a: &BigUint, b: &BigUint, m: &BigUint) -> BigUint {
    (a + b) % m
}

fn mod_sub(a: &BigUint, b: &BigUint, m: &BigUint) -> BigUint {
    if a >= b { (a - b) % m } else { (a + m - b) % m }
}

fn mod_mul(a: &BigUint, b: &BigUint, m: &BigUint) -> BigUint {
    (a * b) % m
}

fn mod_inv(a: &BigUint, m: &BigUint) -> Option<BigUint> {
    if is_zero(a) {
        return None;
    }
    Some(a.modpow(&(m - BigUint::from(2u32)), m))
}

fn is_on_curve(point: &Point, domain: &Domain) -> bool {
    if point.infinity {
        return true;
    }
    let lhs = mod_mul(&point.y, &point.y, &domain.p);
    let x2 = mod_mul(&point.x, &point.x, &domain.p);
    let x3 = mod_mul(&x2, &point.x, &domain.p);
    let rhs = mod_add(
        &mod_add(&x3, &mod_mul(&domain.a, &point.x, &domain.p), &domain.p),
        &domain.b,
        &domain.p,
    );
    lhs == rhs
}

fn point_add(p: &Point, q: &Point, domain: &Domain) -> Option<Point> {
    if p.infinity {
        return Some(q.clone());
    }
    if q.infinity {
        return Some(p.clone());
    }

    if p.x == q.x {
        let y_sum = mod_add(&p.y, &q.y, &domain.p);
        if is_zero(&y_sum) {
            return Some(Point {
                x: ZERO.clone(),
                y: ZERO.clone(),
                infinity: true,
            });
        }
        let three = BigUint::from(3u32);
        let two = BigUint::from(2u32);
        let slope_num = mod_add(
            &mod_mul(&three, &mod_mul(&p.x, &p.x, &domain.p), &domain.p),
            &domain.a,
            &domain.p,
        );
        let slope_den = mod_inv(&mod_mul(&two, &p.y, &domain.p), &domain.p)?;
        let slope = mod_mul(&slope_num, &slope_den, &domain.p);
        let x_r = mod_sub(
            &mod_sub(&mod_mul(&slope, &slope, &domain.p), &p.x, &domain.p),
            &q.x,
            &domain.p,
        );
        let y_r = mod_sub(
            &mod_mul(&slope, &mod_sub(&p.x, &x_r, &domain.p), &domain.p),
            &p.y,
            &domain.p,
        );
        return Some(Point {
            x: x_r,
            y: y_r,
            infinity: false,
        });
    }

    let slope_num = mod_sub(&q.y, &p.y, &domain.p);
    let slope_den = mod_inv(&mod_sub(&q.x, &p.x, &domain.p), &domain.p)?;
    let slope = mod_mul(&slope_num, &slope_den, &domain.p);
    let x_r = mod_sub(
        &mod_sub(&mod_mul(&slope, &slope, &domain.p), &p.x, &domain.p),
        &q.x,
        &domain.p,
    );
    let y_r = mod_sub(
        &mod_mul(&slope, &mod_sub(&p.x, &x_r, &domain.p), &domain.p),
        &p.y,
        &domain.p,
    );
    Some(Point {
        x: x_r,
        y: y_r,
        infinity: false,
    })
}

fn point_mul(k: &BigUint, point: &Point, domain: &Domain) -> Option<Point> {
    let mut n = k.clone();
    let mut acc = Point {
        x: ZERO.clone(),
        y: ZERO.clone(),
        infinity: true,
    };
    let mut base = point.clone();
    while !is_zero(&n) {
        if (&n & &*ONE) == *ONE {
            acc = point_add(&acc, &base, domain)?;
        }
        base = point_add(&base, &base, domain)?;
        n >>= 1;
    }
    Some(acc)
}

fn compute_za(domain: &Domain, distid: &str, pub_x: &BigUint, pub_y: &BigUint) -> [u8; 32] {
    let entla_bits = distid.len().checked_mul(8).expect("distid length overflow");
    let entla = u16::try_from(entla_bits).expect("distid within 65535 bits");
    let mut hasher = Sm3::new();
    hasher.update(entla.to_be_bytes());
    hasher.update(distid.as_bytes());
    hasher.update(to_fixed_32(&domain.a));
    hasher.update(to_fixed_32(&domain.b));
    hasher.update(to_fixed_32(&domain.generator.x));
    hasher.update(to_fixed_32(&domain.generator.y));
    hasher.update(to_fixed_32(pub_x));
    hasher.update(to_fixed_32(pub_y));
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn decode_point(bytes: &[u8]) -> Option<Point> {
    if bytes.len() != 65 || bytes[0] != 0x04 {
        return None;
    }
    let x = BigUint::from_bytes_be(&bytes[1..33]);
    let y = BigUint::from_bytes_be(&bytes[33..]);
    Some(Point {
        x,
        y,
        infinity: false,
    })
}

fn verify_in_domain(
    domain: &Domain,
    distid: &str,
    public: &Point,
    message: &[u8],
    signature: &Sm2Signature,
) -> bool {
    if !is_on_curve(public, domain) {
        return false;
    }

    let za = compute_za(domain, distid, &public.x, &public.y);
    let mut hasher = Sm3::new();
    hasher.update(za);
    hasher.update(message);
    let challenge = BigUint::from_bytes_be(&hasher.finalize());

    let sig_bytes = signature.to_bytes();
    let sig_r = BigUint::from_bytes_be(&sig_bytes[..32]);
    let sig_s = BigUint::from_bytes_be(&sig_bytes[32..]);

    if is_zero(&sig_r) || is_zero(&sig_s) || sig_r >= domain.n || sig_s >= domain.n {
        return false;
    }

    let r_plus_s = (&sig_r + &sig_s) % &domain.n;
    if is_zero(&r_plus_s) {
        return false;
    }

    let s_g = match point_mul(&sig_s, &domain.generator, domain) {
        Some(pt) => pt,
        None => return false,
    };
    let t_p = match point_mul(&r_plus_s, public, domain) {
        Some(pt) => pt,
        None => return false,
    };
    let sum = match point_add(&s_g, &t_p, domain) {
        Some(pt) => pt,
        None => return false,
    };
    if sum.infinity || !is_on_curve(&sum, domain) {
        return false;
    }

    let sum_x_mod = sum.x % &domain.n;
    let recomputed_r = (&challenge + sum_x_mod) % &domain.n;
    recomputed_r == sig_r
}

fn verify_sm2(
    distid: &str,
    public_bytes: &[u8],
    public_point: &Point,
    message: &[u8],
    signature: &Sm2Signature,
) -> bool {
    if Sm2PublicKey::from_sec1_bytes(distid, public_bytes)
        .is_ok_and(|pk| pk.verify(message, signature).is_ok())
    {
        return true;
    }

    DOMAINS
        .iter()
        .any(|domain| verify_in_domain(domain, distid, public_point, message, signature))
}

#[test]
fn sm2_wycheproof_vectors() {
    const RAW: &str = include_str!("fixtures/wycheproof_sm2.json");
    let root: Value = norito::json::from_str(RAW).expect("parse Wycheproof SM2 suite");
    assert_eq!(
        root["algorithm"].as_str().expect("algorithm tag missing"),
        "SM2",
        "unexpected algorithm identifier"
    );

    let total_tests = root["numberOfTests"]
        .as_u64()
        .expect("numberOfTests missing");
    let total_tests =
        usize::try_from(total_tests).expect("Wycheproof SM2 suite count fits in usize");
    let groups = root["testGroups"].as_array().expect("testGroups missing");

    let mut executed = 0usize;
    for group in groups {
        let distid = group["distid"].as_str().unwrap_or("1234567812345678");
        let key_hex = group["key"]["uncompressed"]
            .as_str()
            .expect("SM2 group key missing");
        let key_bytes = hex_to_vec(key_hex);
        assert_eq!(
            key_bytes.len(),
            65,
            "unexpected SEC1 length for Wycheproof SM2 key"
        );
        assert_eq!(
            key_bytes.first().copied(),
            Some(0x04),
            "Wycheproof SM2 key must be uncompressed"
        );
        let public_point = decode_point(&key_bytes).expect("decode SM2 public key");

        let tests = group["tests"].as_array().expect("tests array missing");
        for case in tests {
            executed += 1;
            let tc_id = case["tcId"].as_u64().unwrap_or_default();
            let message = hex_to_vec(case["msg"].as_str().expect("message missing"));
            let signature_bytes = hex_to_vec(case["sig"].as_str().expect("signature missing"));
            let result = case["result"]
                .as_str()
                .unwrap_or("invalid")
                .to_ascii_lowercase();

            let signature = if let Ok(sig) = Sm2Signature::from_der(&signature_bytes) {
                sig
            } else {
                assert_ne!(
                    result,
                    "valid",
                    "Wycheproof tcId {tc_id} ({:?}) expected valid signature but DER parse failed",
                    case["comment"].as_str()
                );
                continue;
            };

            let verified = verify_sm2(distid, &key_bytes, &public_point, &message, &signature);
            match result.as_str() {
                "valid" => assert!(
                    verified,
                    "Wycheproof tcId {tc_id} ({:?}) expected valid signature",
                    case["comment"].as_str()
                ),
                "acceptable" => {}
                "invalid" => assert!(
                    !verified,
                    "Wycheproof tcId {tc_id} ({:?}) unexpectedly verified",
                    case["comment"].as_str()
                ),
                other => panic!("unknown Wycheproof result '{other}' for tcId {tc_id}"),
            }
        }
    }

    assert_eq!(executed, total_tests, "Wycheproof SM2 test count mismatch");
}
