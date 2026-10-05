//! Differential tests of the Pallas and Vesta implementations against
//! `pasta_curves` 0.5.2: encodings, the group law, scalar multiplication, the
//! GLV endomorphism and hash-to-curve.
//!
//! Hash-to-curve is checked for every `Halo2-Parameters` generator index
//! below 2^16 on both curves, plus the `w`/`u` messages and other domains.

use ff::{Field, PrimeField, WithSmallOrderMulGroup};
use group::{Curve, Group, GroupEncoding, prime::PrimeCurveAffine};
use iroha_pasta::{PastaAffine, PastaCurve};
use pasta_curves::arithmetic::CurveExt;
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::{RngCore, SeedableRng};
use rayon::prelude::*;

macro_rules! curve_suite {
    ($mine:ty, $mine_affine:ty, $theirs:ty, $theirs_affine:ty, $scalar:ty, $tscalar:ty, $seed:expr) => {{
        type M = $mine;
        type MA = $mine_affine;
        type T = $theirs;
        type TA = $theirs_affine;
        type S = $scalar;
        type TS = $tscalar;

        fn to_t(p: &M) -> T {
            T::from_bytes(&p.to_bytes()).unwrap()
        }
        fn same(p: &M, q: &T) -> bool {
            p.to_bytes() == q.to_bytes()
        }
        fn ts(s: &S) -> TS {
            TS::from_repr(s.to_repr()).unwrap()
        }

        assert!(same(&M::generator(), &T::generator()));
        assert_eq!(MA::identity().to_bytes(), TA::identity().to_bytes());
        assert_eq!(<M as PastaCurve>::CURVE_ID, <T as CurveExt>::CURVE_ID);

        // Seeded random points consume the RNG identically.
        let mut r1 = ChaCha20Rng::seed_from_u64($seed);
        let mut r2 = ChaCha20Rng::seed_from_u64($seed);
        let points: Vec<M> = (0..200).map(|_| M::random(&mut r1)).collect();
        for p in &points {
            assert!(same(p, &T::random(&mut r2)));
        }

        let mut rng = ChaCha20Rng::seed_from_u64($seed + 1);
        for (i, p) in points.iter().enumerate() {
            let q = &points[(i * 7 + 3) % points.len()];
            let (tp, tq) = (to_t(p), to_t(q));
            assert!(same(&(p + q), &(tp + tq)));
            assert!(same(&(p - q), &(tp - tq)));
            assert!(same(&p.double(), &tp.double()));
            assert!(same(&(p + p), &(tp + tp)));
            assert!(same(&(p - p), &(tp - tp)));
            assert!(same(&(p + M::identity()), &(tp + T::identity())));
            assert!(same(&(-p), &(-tp)));
            assert!(same(&p.endo(), &tp.endo()));
            let pa = p.to_affine();
            let qa = q.to_affine();
            assert!(same(&(p + qa), &(tp + tq.to_affine())));
            assert!(same(&(pa + qa), &(tp.to_affine() + tq.to_affine())));
            assert!(same(&(pa - qa), &(tp.to_affine() - tq.to_affine())));
            let k = S::random(&mut rng);
            assert!(same(&(p * k), &(tp * ts(&k))));
            assert!(same(&p.mul_vartime(&k), &(tp * ts(&k))));
            assert!(same(&(pa * k), &(tp.to_affine() * ts(&k))));
            // GLV decomposition reconstructs the scalar.
            let d = M::glv_decompose(&k).unwrap();
            let k1 = S::from_u128(d.k1);
            let k2 = S::from_u128(d.k2);
            let k1 = if d.k1_neg { -k1 } else { k1 };
            let k2 = if d.k2_neg { -k2 } else { k2 };
            assert_eq!(k1 + k2 * S::ZETA, k);
            // Affine coordinates agree.
            let (x, y) = Option::<(_, _)>::from(PastaAffine::coordinates(&pa)).unwrap();
            let tc = Option::from(pasta_curves::arithmetic::CurveAffine::coordinates(
                &tp.to_affine(),
            ))
            .unwrap();
            let tc: pasta_curves::arithmetic::Coordinates<TA> = tc;
            assert_eq!(x.to_repr(), tc.x().to_repr());
            assert_eq!(y.to_repr(), tc.y().to_repr());
        }
        // Edge scalars.
        let g = M::generator();
        let tg = T::generator();
        for k in [
            S::ZERO,
            S::ONE,
            -S::ONE,
            S::ZETA,
            S::from(2u64),
            S::TWO_INV,
            S::ROOT_OF_UNITY,
        ] {
            assert!(same(&(g * k), &(tg * ts(&k))));
            assert!(same(&g.mul_vartime(&k), &(tg * ts(&k))));
        }

        // Batch normalisation equals pointwise conversion.
        let mut with_identity = points.clone();
        with_identity[17] = M::identity();
        let mut ours = vec![MA::identity(); with_identity.len()];
        M::batch_normalize(&with_identity, &mut ours);
        let parallel = iroha_pasta::curve::batch_normalize_vartime(&with_identity);
        for ((p, a), b) in with_identity.iter().zip(&ours).zip(&parallel) {
            assert_eq!(a.to_bytes(), to_t(p).to_affine().to_bytes());
            assert_eq!(a, b);
        }

        // Decoding of arbitrary byte strings agrees, including the sign bit
        // on x = 0 and non-canonical x.
        let mut buf = [0u8; 32];
        for i in 0..20_000 {
            rng.fill_bytes(&mut buf);
            if i % 5 == 0 {
                buf[31] &= 0x3F; // in-range x more often
            }
            let ours = Option::<MA>::from(MA::from_bytes(&buf));
            let theirs = Option::<TA>::from(TA::from_bytes(&buf));
            assert_eq!(
                ours.map(|p| p.to_bytes()),
                theirs.map(|p| p.to_bytes()),
                "bytes {buf:?}"
            );
        }
        for special in [
            [0u8; 32],
            {
                let mut b = [0u8; 32];
                b[31] = 0x80;
                b
            },
            [0xFF; 32],
        ] {
            let ours = Option::<MA>::from(MA::from_bytes(&special));
            let theirs = Option::<TA>::from(TA::from_bytes(&special));
            assert_eq!(ours.map(|p| p.to_bytes()), theirs.map(|p| p.to_bytes()));
        }

        // Hash-to-curve: every ParamsIPA generator index below 2^16, w and u.
        let mine = iroha_pasta::curve::hash_to_curve::hasher::<M>("Halo2-Parameters").unwrap();
        let mismatches: usize = (0u32..256)
            .into_par_iter()
            .map(|chunk| {
                // The reference hasher is not Sync; build one per chunk.
                let theirs = T::hash_to_curve("Halo2-Parameters");
                (chunk * 256..(chunk + 1) * 256)
                    .filter(|i| {
                        let mut message = [0u8; 5];
                        message[1..5].copy_from_slice(&i.to_le_bytes());
                        mine(&message).to_bytes() != theirs(&message).to_bytes()
                    })
                    .count()
            })
            .sum();
        assert_eq!(mismatches, 0);
        let theirs = T::hash_to_curve("Halo2-Parameters");
        for message in [
            &[1u8][..],
            &[2u8][..],
            &[][..],
            b"z.cash:test",
            &[0xFF; 300][..],
        ] {
            assert_eq!(mine(message).to_bytes(), theirs(message).to_bytes());
        }
        for domain in ["", "z.cash:test", "iroha", &"d".repeat(200)] {
            let a = M::hash_to_curve(domain, b"message").unwrap();
            let b = T::hash_to_curve(domain)(b"message");
            assert!(same(&a, &b));
        }
    }};
}

#[test]
fn pallas_matches_pasta_curves() {
    curve_suite!(
        iroha_pasta::Ep,
        iroha_pasta::EpAffine,
        pasta_curves::Ep,
        pasta_curves::EpAffine,
        iroha_pasta::Fq,
        pasta_curves::Fq,
        21
    );
}

#[test]
fn vesta_matches_pasta_curves() {
    curve_suite!(
        iroha_pasta::Eq,
        iroha_pasta::EqAffine,
        pasta_curves::Eq,
        pasta_curves::EqAffine,
        iroha_pasta::Fp,
        pasta_curves::Fp,
        22
    );
}

#[test]
fn batch_scalar_multiplication_matches_pasta_curves() {
    let mut rng = ChaCha20Rng::seed_from_u64(31);
    let points: Vec<iroha_pasta::EqAffine> = (0..1500)
        .map(|_| iroha_pasta::Eq::random(&mut rng).to_affine())
        .collect();
    let scalars: Vec<iroha_pasta::Fp> = (0..1500)
        .map(|_| iroha_pasta::Fp::random(&mut rng))
        .collect();
    let out = iroha_pasta::curve::batch_mul_vartime::<iroha_pasta::Eq>(&points, &scalars).unwrap();
    for ((p, k), r) in points.iter().zip(&scalars).zip(&out) {
        let tp = pasta_curves::EqAffine::from_bytes(&p.to_bytes()).unwrap();
        let tk = pasta_curves::Fp::from_repr(k.to_repr()).unwrap();
        assert_eq!(r.to_bytes(), (tp * tk).to_affine().to_bytes());
    }
}
