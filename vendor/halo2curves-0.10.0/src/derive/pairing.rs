#[macro_export]
macro_rules! impl_miller_loop_components {
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
        #[derive(Clone, Debug)]
        pub struct $engine;

        impl Engine for $engine {
            type Fr = $scalar;
            type G1 = $g1;
            type G1Affine = $g1affine;
            type G2 = $g2;
            type G2Affine = $g2affine;
            type Gt = Gt;

            fn pairing(p: &Self::G1Affine, q: &Self::G2Affine) -> Self::Gt {
                $engine::multi_miller_loop(&[(p, q)]).final_exponentiation()
            }
        }

        impl MultiMillerLoop for $engine {
            type G2Prepared = $g2affine;
            type Result = $base;

            fn multi_miller_loop(terms: &[(&Self::G1Affine, &Self::G2Prepared)]) -> Self::Result {
                multi_miller_loop(terms)
            }
        }

        impl PairingCurveAffine for $g1affine {
            type Pair = $g2affine;
            type PairingResult = $target;

            fn pairing_with(&self, other: &Self::Pair) -> Self::PairingResult {
                $engine::pairing(&self, &other)
            }
        }

        impl PairingCurveAffine for $g2affine {
            type Pair = $g1affine;
            type PairingResult = $target;

            fn pairing_with(&self, other: &Self::Pair) -> Self::PairingResult {
                $engine::pairing(&other, &self)
            }
        }

        fn double(f: &mut $base, r: &mut $g2, p: &$g1affine) {
            use ff::Field;
            let t0 = r.x.square();
            let t1 = r.y.square();
            let t2 = t1.square();
            let t3 = (t1 + r.x).square() - t0 - t2;
            let t3 = t3 + t3;
            let t4 = t0 + t0 + t0;
            let t6 = r.x + t4;
            let t5 = t4.square();
            let zsquared = r.z.square();
            r.x = t5 - t3 - t3;
            r.z = (r.z + r.y).square() - t1 - zsquared;
            r.y = (t3 - r.x) * t4;
            let t2 = t2 + t2;
            let t2 = t2 + t2;
            let t2 = t2 + t2;
            r.y -= t2;
            let t3 = t4 * zsquared;
            let t3 = t3 + t3;
            let t3 = -t3;
            let t6 = t6.square() - t0 - t5;
            let t1 = t1 + t1;
            let t1 = t1 + t1;
            let t6 = t6 - t1;
            let t0 = r.z * zsquared;
            let t0 = t0 + t0;

            ell(f, &(t0, t3, t6), p);
        }

        fn add(f: &mut $base, r: &mut $g2, q: &$g2affine, p: &$g1affine) {
            use ff::Field;
            let zsquared = r.z.square();
            let ysquared = q.y.square();
            let t0 = zsquared * q.x;
            let t1 = ((q.y + r.z).square() - ysquared - zsquared) * zsquared;
            let t2 = t0 - r.x;
            let t3 = t2.square();
            let t4 = t3 + t3;
            let t4 = t4 + t4;
            let t5 = t4 * t2;
            let t6 = t1 - r.y - r.y;
            let t9 = t6 * q.x;
            let t7 = t4 * r.x;
            r.x = t6.square() - t5 - t7 - t7;
            r.z = (r.z + t2).square() - zsquared - t3;
            let t10 = q.y + r.z;
            let t8 = (t7 - r.x) * t6;
            let t0 = r.y * t5;
            let t0 = t0 + t0;
            r.y = t8 - t0;
            let t10 = t10.square() - ysquared;
            let ztsquared = r.z.square();
            let t10 = t10 - ztsquared;
            let t9 = t9 + t9 - t10;
            let t10 = r.z + r.z;
            let t6 = -t6;
            let t1 = t6 + t6;

            ell(f, &(t10, t1, t9), p);
        }
    };
}

#[macro_export]
macro_rules! impl_gt {
    (
        $target:ident,
        $base:ident,
        $scalar:ident
    ) => {
        #[derive(Copy, Clone, Debug, Default)]
        pub struct $target(pub(crate) $base);

        impl ConstantTimeEq for $target {
            fn ct_eq(&self, other: &Self) -> Choice {
                self.0.ct_eq(&other.0)
            }
        }

        impl ConditionallySelectable for $target {
            fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
                $target($base::conditional_select(&a.0, &b.0, choice))
            }
        }

        impl Eq for $target {}
        impl PartialEq for $target {
            #[inline]
            fn eq(&self, other: &Self) -> bool {
                bool::from(self.ct_eq(other))
            }
        }

        impl $target {
            /// Returns the group identity, which is $1$.
            pub fn identity() -> $target {
                $target($base::one())
            }

            /// Doubles this group element.
            pub fn double(&self) -> $target {
                use ff::Field;
                $target(self.0.square())
            }
        }

        impl<'a> Neg for &'a $target {
            type Output = $target;

            #[inline]
            fn neg(self) -> $target {
                // The element is unitary, so we just conjugate.
                let mut u = self.0;
                u.conjugate();
                $target(u)
            }
        }

        impl Neg for $target {
            type Output = $target;

            #[inline]
            fn neg(self) -> $target {
                -&self
            }
        }

        impl<'a, 'b> Add<&'b $target> for &'a $target {
            type Output = $target;

            #[inline]
            #[allow(clippy::suspicious_arithmetic_impl)]
            fn add(self, rhs: &'b $target) -> $target {
                $target(self.0 * rhs.0)
            }
        }

        impl<'a, 'b> Sub<&'b $target> for &'a $target {
            type Output = $target;

            #[inline]
            fn sub(self, rhs: &'b $target) -> $target {
                self + (-rhs)
            }
        }

        #[allow(clippy::suspicious_arithmetic_impl)]
        impl<'a, 'b> Mul<&'b $scalar> for &'a $target {
            type Output = $target;

            fn mul(self, other: &'b $scalar) -> Self::Output {
                let mut acc = $target::identity();

                for bit in other
                    .to_repr()
                    .as_ref()
                    .iter()
                    .rev()
                    .flat_map(|byte| (0..8).rev().map(move |i| Choice::from((byte >> i) & 1u8)))
                    .skip(1)
                {
                    acc = acc.double();
                    acc = $target::conditional_select(&acc, &(acc + self), bit);
                }

                acc
            }
        }

        $crate::impl_binops_additive!($target, $target);
        $crate::impl_binops_multiplicative!($target, $scalar);

        impl<T> Sum<T> for $target
        where
            T: Borrow<$target>,
        {
            fn sum<I>(iter: I) -> Self
            where
                I: Iterator<Item = T>,
            {
                iter.fold(Self::identity(), |acc, item| acc + item.borrow())
            }
        }

        impl Group for $target {
            type Scalar = $scalar;

            fn random(rng: impl RngCore) -> Self {
                use ff::Field;
                $base::random(rng).final_exponentiation()
            }

            fn identity() -> Self {
                Self::identity()
            }

            fn generator() -> Self {
                unimplemented!()
            }

            fn is_identity(&self) -> Choice {
                self.ct_eq(&Self::identity())
            }

            #[must_use]
            fn double(&self) -> Self {
                self.double()
            }
        }
    };
}
