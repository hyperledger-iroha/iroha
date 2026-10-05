//! Differential tests of `iroha_pasta::{Fp, Fq}` against `pasta_curves` 0.5.2.
//!
//! Every operation result is compared through the canonical 32-byte encoding.
//! The random loop performs well over 10^6 field operations per run, and the
//! edge-value sweep covers 0, 1, m - 1, limb boundaries and the
//! non-canonical range `[m, 2^256)`.

use ff::{Field, FromUniformBytes, PrimeField, PrimeFieldBits, WithSmallOrderMulGroup};
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::{RngCore, SeedableRng};

/// Runs the differential suite for one field pair.
macro_rules! field_suite {
    ($mine:ty, $theirs:ty, $modulus:expr, $seed:expr) => {{
        type M = $mine;
        type T = $theirs;

        fn to_t(a: &M) -> T {
            T::from_repr(a.to_repr()).unwrap()
        }
        fn same(a: &M, b: &T) -> bool {
            a.to_repr() == b.to_repr()
        }

        // Constants.
        assert!(same(&M::ZERO, &T::ZERO));
        assert!(same(&M::ONE, &T::ONE));
        assert!(same(&M::TWO_INV, &T::TWO_INV));
        assert!(same(
            &M::MULTIPLICATIVE_GENERATOR,
            &T::MULTIPLICATIVE_GENERATOR
        ));
        assert!(same(&M::ROOT_OF_UNITY, &T::ROOT_OF_UNITY));
        assert!(same(&M::ROOT_OF_UNITY_INV, &T::ROOT_OF_UNITY_INV));
        assert!(same(&M::DELTA, &T::DELTA));
        assert!(same(
            &<M as WithSmallOrderMulGroup<3>>::ZETA,
            &<T as WithSmallOrderMulGroup<3>>::ZETA
        ));
        assert_eq!(M::MODULUS, T::MODULUS);
        assert_eq!(M::NUM_BITS, T::NUM_BITS);
        assert_eq!(M::CAPACITY, T::CAPACITY);
        assert_eq!(M::S, T::S);
        assert_eq!(
            M::char_le_bits().into_inner(),
            T::char_le_bits().into_inner()
        );

        // Seeded sampling consumes the RNG identically.
        let mut r1 = ChaCha20Rng::seed_from_u64($seed);
        let mut r2 = ChaCha20Rng::seed_from_u64($seed);
        for _ in 0..1000 {
            assert!(same(&M::random(&mut r1), &T::random(&mut r2)));
        }

        // Edge values.
        let m: [u64; 4] = $modulus;
        let m_bytes = limbs_to_bytes(&m);
        let mut edges: Vec<[u8; 32]> = Vec::new();
        for delta in 0u64..4 {
            edges.push(limbs_to_bytes(&sub_small(&m, delta + 1))); // m - 1 .. m - 4
            edges.push(limbs_to_bytes(&[delta, 0, 0, 0]));
        }
        let half = shr1(&m);
        edges.push(limbs_to_bytes(&half)); // (m - 1) / 2
        edges.push(limbs_to_bytes(&add_small(&half, 1))); // (m + 1) / 2
        for limb in 0..4 {
            let mut v = [0u64; 4];
            v[limb] = u64::MAX;
            if limb < 3 {
                edges.push(limbs_to_bytes(&v));
            }
            let mut w = [0u64; 4];
            w[limb] = 1;
            edges.push(limbs_to_bytes(&w));
        }
        edges.push(limbs_to_bytes(&[
            u64::MAX,
            u64::MAX,
            u64::MAX,
            (1 << 62) - 1,
        ]));
        edges.push(limbs_to_bytes(&[0, 0, 0, 1 << 62])); // 2^254 < m
        let edge_values: Vec<M> = edges
            .iter()
            .map(|b| Option::<M>::from(M::from_repr(*b)).expect("edge value is canonical"))
            .collect();
        for a in &edge_values {
            for b in &edge_values {
                let (ta, tb) = (to_t(a), to_t(b));
                assert!(same(&(*a + b), &(ta + tb)));
                assert!(same(&(*a - b), &(ta - tb)));
                assert!(same(&(*a * b), &(ta * tb)));
            }
            let ta = to_t(a);
            assert!(same(&a.square(), &ta.square()));
            assert!(same(&-*a, &-ta));
            assert!(same(&a.double(), &ta.double()));
            let inv = Option::<M>::from(a.invert());
            let tinv = Option::<T>::from(ta.invert());
            assert_eq!(inv.map(|v| v.to_repr()), tinv.map(|v| v.to_repr()));
            assert_eq!(
                a.invert_vartime().map(|v| v.to_repr()),
                tinv.map(|v| v.to_repr())
            );
            let sq = Option::<M>::from(a.sqrt());
            let tsq = Option::<T>::from(ta.sqrt());
            assert_eq!(sq.map(|v| v.to_repr()), tsq.map(|v| v.to_repr()));
            assert_eq!(bool::from(a.is_odd()), bool::from(ta.is_odd()));
            assert_eq!(format!("{a:?}"), format!("{ta:?}"));
        }

        // Non-canonical encodings in [m, 2^256) are rejected by both.
        let mut noncanonical = vec![m_bytes, limbs_to_bytes(&add_small(&m, 1)), [0xFF; 32]];
        noncanonical.push(limbs_to_bytes(&[
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX >> 1,
        ]));
        let two_m_minus_1 = sub_small(&add_limbs(&m, &m), 1);
        noncanonical.push(limbs_to_bytes(&two_m_minus_1));
        for b in &noncanonical {
            assert!(bool::from(M::from_repr(*b).is_none()));
            assert!(bool::from(T::from_repr(*b).is_none()));
        }
        // Wide reduction agrees on boundary inputs.
        for b in noncanonical.iter().chain(edges.iter()) {
            let mut wide = [0u8; 64];
            wide[..32].copy_from_slice(b);
            assert!(same(
                &M::from_uniform_bytes(&wide),
                &T::from_uniform_bytes(&wide)
            ));
            wide[32..].copy_from_slice(b);
            assert!(same(
                &M::from_uniform_bytes(&wide),
                &T::from_uniform_bytes(&wide)
            ));
        }
        assert!(same(
            &M::from_uniform_bytes(&[0xFF; 64]),
            &T::from_uniform_bytes(&[0xFF; 64])
        ));

        // Random operations: 7 operation kinds per iteration on two fields,
        // plus inversions and square roots.
        let mut rng = ChaCha20Rng::seed_from_u64($seed ^ 0xABCD);
        let mut ops = 0u64;
        let mut acc_m = M::ONE;
        let mut acc_t = T::ONE;
        for i in 0..80_000u64 {
            let a = M::random(&mut rng);
            let b = M::random(&mut rng);
            let (ta, tb) = (to_t(&a), to_t(&b));
            assert!(same(&(a + b), &(ta + tb)));
            assert!(same(&(a - b), &(ta - tb)));
            assert!(same(&(a * b), &(ta * tb)));
            assert!(same(&a.square(), &ta.square()));
            assert!(same(&-a, &-ta));
            assert!(same(&a.double(), &ta.double()));
            // A running product chains results through many operations.
            acc_m = acc_m * a + b;
            acc_t = acc_t * ta + tb;
            ops += 8;
            if i % 8 == 0 {
                let inv = a.invert().unwrap();
                assert!(same(&inv, &ta.invert().unwrap()));
                assert_eq!(a.invert_vartime().unwrap(), inv);
                let (sq_m, root_m) = M::sqrt_ratio(&a, &b);
                let (sq_t, root_t) = T::sqrt_ratio(&ta, &tb);
                assert_eq!(bool::from(sq_m), bool::from(sq_t));
                assert!(same(&root_m, &root_t));
                assert_eq!(a.cmp(&b), ta.cmp(&tb));
                assert_eq!(a.to_le_bits().into_inner(), ta.to_le_bits().into_inner());
                let e = [rng.next_u64(), rng.next_u64()];
                assert!(same(&a.pow_vartime(e), &ta.pow_vartime(e)));
                assert!(same(&a.pow(e), &ta.pow_vartime(e)));
                let v = (u128::from(rng.next_u64()) << 64) | u128::from(rng.next_u64());
                assert!(same(&M::from_u128(v), &T::from_u128(v)));
                let w = rng.next_u64();
                assert!(same(&M::from(w), &T::from(w)));
                let mut wide = [0u8; 64];
                rng.fill_bytes(&mut wide);
                assert!(same(
                    &M::from_uniform_bytes(&wide),
                    &T::from_uniform_bytes(&wide)
                ));
                ops += 10;
            }
        }
        assert!(same(&acc_m, &acc_t));
        ops
    }};
}

fn limbs_to_bytes(l: &[u64; 4]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (chunk, limb) in out.chunks_exact_mut(8).zip(l) {
        chunk.copy_from_slice(&limb.to_le_bytes());
    }
    out
}

fn add_limbs(a: &[u64; 4], b: &[u64; 4]) -> [u64; 4] {
    let mut out = [0u64; 4];
    let mut carry = false;
    for i in 0..4 {
        let (s1, c1) = a[i].overflowing_add(b[i]);
        let (s2, c2) = s1.overflowing_add(u64::from(carry));
        out[i] = s2;
        carry = c1 || c2;
    }
    out
}

fn add_small(a: &[u64; 4], v: u64) -> [u64; 4] {
    add_limbs(a, &[v, 0, 0, 0])
}

fn sub_small(a: &[u64; 4], v: u64) -> [u64; 4] {
    let mut out = *a;
    let mut borrow = v;
    for limb in &mut out {
        let (d, b) = limb.overflowing_sub(borrow);
        *limb = d;
        borrow = u64::from(b);
        if borrow == 0 {
            break;
        }
    }
    out
}

fn shr1(a: &[u64; 4]) -> [u64; 4] {
    [
        (a[0] >> 1) | (a[1] << 63),
        (a[1] >> 1) | (a[2] << 63),
        (a[2] >> 1) | (a[3] << 63),
        a[3] >> 1,
    ]
}

const P: [u64; 4] = [
    0x992d_30ed_0000_0001,
    0x2246_98fc_094c_f91b,
    0,
    0x4000_0000_0000_0000,
];
const Q: [u64; 4] = [
    0x8c46_eb21_0000_0001,
    0x2246_98fc_0994_a8dd,
    0,
    0x4000_0000_0000_0000,
];

#[test]
fn fp_matches_pasta_curves() {
    let ops = field_suite!(iroha_pasta::Fp, pasta_curves::Fp, P, 11);
    assert!(ops >= 500_000, "ran {ops} operations");
}

#[test]
fn fq_matches_pasta_curves() {
    let ops = field_suite!(iroha_pasta::Fq, pasta_curves::Fq, Q, 12);
    assert!(ops >= 500_000, "ran {ops} operations");
}

#[test]
fn batch_inversion_matches_elementwise() {
    let mut rng = ChaCha20Rng::seed_from_u64(5);
    let mut v: Vec<iroha_pasta::Fp> = (0..257)
        .map(|_| iroha_pasta::Fp::random(&mut rng))
        .collect();
    v[3] = iroha_pasta::Fp::ZERO;
    v[200] = iroha_pasta::Fp::ZERO;
    let expected: Vec<_> = v
        .iter()
        .map(|x| x.invert().unwrap_or(iroha_pasta::Fp::ZERO))
        .collect();
    let mut a = v.clone();
    let mut b = v;
    iroha_pasta::field::batch_invert(&mut a);
    iroha_pasta::field::batch_invert_vartime(&mut b);
    assert_eq!(a, expected);
    assert_eq!(b, expected);
}
