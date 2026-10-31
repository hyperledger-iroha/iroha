#[macro_export]
macro_rules! test_pairing {
    (
    $engine:ident,
    $g1:ident,
    $g1affine:ident,
    $g2:ident,
    $g2affine:ident,
    $base:ident,
    $target:ident,
    $scalar:ident
    ) => {
        #[test]
        fn test_miller_loop_identity() {
            use ff::Field;
            assert_eq!($base::ONE.final_exponentiation(), $target::identity());

            assert_eq!(
                multi_miller_loop(&[(&$g1affine::identity(), &$g2affine::generator().into())]),
                $base::one()
            );
            assert_eq!(
                multi_miller_loop(&[(&$g1affine::generator(), &$g2affine::identity().into())]),
                $base::one()
            );
            assert_ne!(
                multi_miller_loop(&[
                    (&$g1affine::generator(), &$g2affine::generator().into()),
                    (&-$g1affine::generator(), &$g2affine::generator().into())
                ]),
                $base::one()
            );
            assert_eq!(
                multi_miller_loop(&[
                    (&$g1affine::generator(), &$g2affine::generator().into()),
                    (&-$g1affine::generator(), &$g2affine::generator().into())
                ])
                .final_exponentiation(),
                $target::identity()
            );
        }

        #[test]
        fn test_unitary() {
            let g = $g1affine::generator();
            let h = $g2affine::generator();
            let p = -$engine::pairing(&g, &h);
            let q = $engine::pairing(&g, &-h);
            let r = $engine::pairing(&-g, &h);
            assert_eq!(p, q);
            assert_eq!(q, r);
        }

        #[test]
        fn test_bilinearity() {
            use $crate::ff::Field;

            let a = $scalar::random(OsRng);
            let b = $scalar::random(OsRng);

            let g1 = $g1::generator();
            let g2 = $g2::generator();

            let a1 = g1 * a;
            let b2 = g2 * b;
            let u0 = $engine::pairing(&a1.into(), &b2.into());

            let b1 = g1 * b;
            let a2 = g2 * a;
            let u1 = $engine::pairing(&b1.into(), &a2.into());
            assert_eq!(u0, u1);

            let u1 = $engine::pairing(&g1.into(), &g2.into()) * (a * b);
            assert_eq!(u0, u1);
        }

        #[test]
        pub fn engine_tests() {
            for _ in 0..10 {
                let a: $g1affine = $g1::random(OsRng).into();
                let b: $g2affine = $g2::random(OsRng).into();

                assert!(a.pairing_with(&b) == b.pairing_with(&a));
            }

            for _ in 0..1000 {
                let z1 = $g1affine::identity();
                let z2 = $g2affine::identity();

                let a = $g1::random(OsRng).into();
                let b = $g2::random(OsRng).into();
                let c = $g1::random(OsRng).into();
                let d = $g2::random(OsRng).into();

                assert_eq!(
                    $base::one(),
                    multi_miller_loop(&[(&z1, &b)]).final_exponentiation().0,
                );

                assert_eq!(
                    $base::one(),
                    multi_miller_loop(&[(&a, &z2)]).final_exponentiation().0,
                );

                assert_eq!(
                    multi_miller_loop(&[(&z1, &b), (&c, &d)]).final_exponentiation(),
                    multi_miller_loop(&[(&a, &z2), (&c, &d)]).final_exponentiation(),
                );

                assert_eq!(
                    multi_miller_loop(&[(&a, &b), (&z1, &d)]).final_exponentiation(),
                    multi_miller_loop(&[(&a, &b), (&c, &z2)]).final_exponentiation(),
                );
            }
        }

        #[test]
        fn test_pairing_check() {
            #[cfg(not(feature = "std"))]
            extern crate alloc;
            #[cfg(not(feature = "std"))]
            use alloc::vec::Vec;

            let n = 10;
            let g1 = $g1::generator().to_affine();
            let g2 = $g2::generator().to_affine();
            let scalars = (0..n)
                .map(|_| ($scalar::random(OsRng), $scalar::random(OsRng)))
                .collect::<Vec<_>>();
            let terms = scalars
                .iter()
                .map(|(a, b)| ((g1 * a).to_affine(), (g2 * b).to_affine()))
                .collect::<Vec<_>>();
            let mut terms = terms.iter().map(|(a, b)| (a, b)).collect::<Vec<_>>();
            let gt = $engine::pairing(&g1, &g2);
            let u0 = scalars
                .iter()
                .fold($target::identity(), |acc, (a, b)| acc + gt * a * b);
            let u1 = multi_miller_loop(&terms[..]).final_exponentiation();
            assert_eq!(u1, u0);

            let last = scalars
                .iter()
                .fold($scalar::ZERO, |acc, (u0, u1)| acc + u0 * u1);
            let negg1 = -g1;
            let accg2 = (g2 * last).into();
            terms.push((&negg1, &accg2));
            let must_be_one = multi_miller_loop(&terms[..]).final_exponentiation();
            assert_eq!(must_be_one, $target::identity());
        }
    };
}
