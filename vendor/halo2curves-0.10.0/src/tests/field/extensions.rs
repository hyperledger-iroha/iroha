// F2 tests
#[macro_export]
macro_rules! f2_test {
    ($ext_field:ident,
     $base_field:ident) => {
        #[test]
        fn f2_ordering_test() {
            use ark_std::cmp::Ordering;
            let mut a = $ext_field {
                c0: $base_field::zero(),
                c1: $base_field::zero(),
            };

            let mut b = a;

            assert!(a.cmp(&b) == Ordering::Equal);
            b.c0 += &$base_field::one();
            assert!(a.cmp(&b) == Ordering::Less);
            a.c0 += &$base_field::one();
            assert!(a.cmp(&b) == Ordering::Equal);
            b.c1 += &$base_field::one();
            assert!(a.cmp(&b) == Ordering::Less);
            a.c0 += &$base_field::one();
            assert!(a.cmp(&b) == Ordering::Less);
            a.c1 += &$base_field::one();
            assert!(a.cmp(&b) == Ordering::Greater);
            b.c0 += &$base_field::one();
            assert!(a.cmp(&b) == Ordering::Equal);
        }

        #[test]
        fn f2_zero_one_test() {
            assert_eq!(
                $ext_field {
                    c0: $base_field::zero(),
                    c1: $base_field::zero(),
                },
                $ext_field::ZERO
            );
            assert_eq!(
                $ext_field {
                    c0: $base_field::one(),
                    c1: $base_field::zero(),
                },
                $ext_field::ONE
            );
            assert_eq!($ext_field::ZERO.is_zero().unwrap_u8(), 1);
            assert_eq!($ext_field::ONE.is_zero().unwrap_u8(), 0);
            assert_eq!(
                $ext_field {
                    c0: $base_field::zero(),
                    c1: $base_field::one(),
                }
                .is_zero()
                .unwrap_u8(),
                0
            );
        }
    };
}

// F6 tests
#[macro_export]
macro_rules! setup_f6_test_funcs {
    ($ext_field:ident,
     $base_field:ident) => {
        fn f6_mul_nonresidue_(mut rng: impl RngCore, n: usize) {
            let nqr = $ext_field {
                c0: $base_field::zero(),
                c1: $base_field::one(),
                c2: $base_field::zero(),
            };

            for _ in 0..n {
                let mut a = $ext_field::random(&mut rng);
                let mut b = a;
                a = a.mul_by_nonresidue();
                b.mul_assign(&nqr);

                assert_eq!(a, b);
            }
        }

        fn f6_mul_by_1_(mut rng: impl RngCore, n: usize) {
            for _ in 0..n {
                let c1 = $base_field::random(&mut rng);
                let mut a = $ext_field::random(&mut rng);
                let mut b = a;

                a = <$ext_field as CubicSparseMul>::mul_by_1(&a, &c1);
                b.mul_assign(&$ext_field {
                    c0: $base_field::zero(),
                    c1,
                    c2: $base_field::zero(),
                });

                assert_eq!(a, b);
            }
        }

        fn f6_mul_by_01_(mut rng: impl RngCore, n: usize) {
            for _ in 0..n {
                let c0 = $base_field::random(&mut rng);
                let c1 = $base_field::random(&mut rng);
                let mut a = $ext_field::random(&mut rng);
                let mut b = a;

                a = <$ext_field as CubicSparseMul>::mul_by_01(&a, &c0, &c1);
                b.mul_assign(&$ext_field {
                    c0,
                    c1,
                    c2: $base_field::zero(),
                });

                assert_eq!(a, b);
            }
        }
    };
}

// F12 tests
#[macro_export]
macro_rules! setup_f12_test_funcs {
    ($ext_field:ident,
     $base_field_1:ident,
     $base_field_2:ident) => {
        fn f12_mul_by_014_(mut rng: impl RngCore, n: usize) {
            for _ in 0..n {
                let c0 = $base_field_2::random(&mut rng);
                let c1 = $base_field_2::random(&mut rng);
                let c5 = $base_field_2::random(&mut rng);
                let mut a = $ext_field::random(&mut rng);
                let mut b = a;
                <$ext_field as QuadSparseMul>::mul_by_014(&mut a, &c0, &c1, &c5);
                b.mul_assign(&$ext_field {
                    c0: $base_field_1 {
                        c0,
                        c1,
                        c2: $base_field_2::zero(),
                    },
                    c1: $base_field_1 {
                        c0: $base_field_2::zero(),
                        c1: c5,
                        c2: $base_field_2::zero(),
                    },
                });

                assert_eq!(a, b);
            }
        }

        fn f12_mul_by_034_(mut rng: impl RngCore, n: usize) {
            for _ in 0..n {
                let c0 = $base_field_2::random(&mut rng);
                let c3 = $base_field_2::random(&mut rng);
                let c4 = $base_field_2::random(&mut rng);
                let mut a = $ext_field::random(&mut rng);
                let mut b = a;

                <$ext_field as QuadSparseMul>::mul_by_034(&mut a, &c0, &c3, &c4);
                b.mul_assign(&$ext_field {
                    c0: $base_field_1 {
                        c0,
                        c1: $base_field_2::zero(),
                        c2: $base_field_2::zero(),
                    },
                    c1: $base_field_1 {
                        c0: c3,
                        c1: c4,
                        c2: $base_field_2::zero(),
                    },
                });
                assert_eq!(a, b);
            }
        }
    };
}

#[macro_export]
macro_rules! frobenius_test {
    ($field:ident, $base: ident, $size: expr) => {
        fn test_frobenius(mut rng: impl RngCore, n: usize) {
            for _ in 0..n {
                for i in 0..12 {
                    let mut a = $field::random(&mut rng);
                    let mut b = a;

                    for _ in 0..i {
                        a = a.pow($base::MODULUS_LIMBS);
                    }
                    b.frobenius_map(i);

                    assert_eq!(a, b);
                }
            }
        }

        #[test]
        fn frobenius_test() {
            use rand_core::SeedableRng;
            use rand_xorshift::XorShiftRng;
            let mut rng = XorShiftRng::from_seed($crate::tests::SEED);
            test_frobenius(&mut rng, $size);
        }
    };
}
