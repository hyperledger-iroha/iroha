#[macro_export]
macro_rules! impl_tower2 {
    (
        $base:ident,
        $field:ident
    ) => {
        impl $field {
            pub const SIZE: usize = $base::SIZE * 2;
        }

        impl Ord for $field {
            #[inline(always)]
            fn cmp(&self, other: &$field) -> Ordering {
                match self.c1.cmp(&other.c1) {
                    Ordering::Greater => Ordering::Greater,
                    Ordering::Less => Ordering::Less,
                    Ordering::Equal => self.c0.cmp(&other.c0),
                }
            }
        }

        impl PartialOrd for $field {
            #[inline(always)]
            fn partial_cmp(&self, other: &$field) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl From<u64> for $field {
            fn from(val: u64) -> Self {
                $field {
                    c0: $base::from(val),
                    c1: $base::ZERO,
                }
            }
        }

        impl $field {
            /// Attempts to convert a little-endian byte representation of
            /// a scalar into a `$base`, failing if the input is not canonical.
            pub fn from_bytes(bytes: &[u8; $base::SIZE * 2]) -> CtOption<$field> {
                let c0 = $base::from_bytes(bytes[0..$base::SIZE].try_into().unwrap());
                let c1 = $base::from_bytes(bytes[$base::SIZE..$base::SIZE * 2].try_into().unwrap());
                CtOption::new(
                    $field {
                        c0: c0.unwrap(),
                        c1: c1.unwrap(),
                    },
                    c0.is_some() & c1.is_some(),
                )
            }

            /// Converts an element of `$base` into a byte representation in
            /// little-endian byte order.
            #[allow(clippy::wrong_self_convention)]
            pub fn to_bytes(&self) -> [u8; $base::SIZE * 2] {
                let mut res = [0u8; $base::SIZE * 2];
                let c0_bytes = self.c0.to_bytes();
                let c1_bytes = self.c1.to_bytes();
                res[0..$base::SIZE].copy_from_slice(&c0_bytes[..]);
                res[$base::SIZE..$base::SIZE * 2].copy_from_slice(&c1_bytes[..]);
                res
            }

            #[inline]
            /// Returns whether or not this element is strictly lexicographically
            /// larger than its negation.
            pub fn lexicographically_largest(&self) -> Choice {
                // If this element's c1 coefficient is lexicographically largest
                // then it is lexicographically largest. Otherwise, in the event
                // the c1 coefficient is zero and the c0 coefficient is
                // lexicographically largest, then this element is lexicographically
                // largest.

                self.c1.lexicographically_largest()
                    | (self.c1.is_zero() & self.c0.lexicographically_largest())
            }
        }

        impl WithSmallOrderMulGroup<3> for $field {
            const ZETA: Self = $field {
                c0: $base::ZETA.mul_const(&$base::ZETA),
                c1: $base::ZERO,
            };
        }

        impl Legendre for $field {
            fn legendre(&self) -> i64 {
                self.norm().legendre()
            }
        }

        impl PrimeField for $field {
            type Repr = $crate::serde::Repr<{ $base::SIZE * 2 }>;

            const MODULUS: &'static str = <$base as PrimeField>::MODULUS;
            const MULTIPLICATIVE_GENERATOR: Self = $field {
                c0: $base::MULTIPLICATIVE_GENERATOR,
                c1: $base::ZERO,
            };
            const NUM_BITS: u32 = $base::NUM_BITS;
            const CAPACITY: u32 = $base::NUM_BITS;
            const S: u32 = $base::S;

            // Unused variables (Because this is not a Prime Field).
            // These are just used to pass the constants test.
            const ROOT_OF_UNITY: Self = $field {
                c0: $base::ROOT_OF_UNITY,
                c1: $base::ZERO,
            };
            const ROOT_OF_UNITY_INV: Self = $field {
                c0: $base::ROOT_OF_UNITY_INV,
                c1: $base::ZERO,
            };

            const DELTA: Self = $field {
                c0: $base::DELTA,
                c1: $base::ZERO,
            };

            const TWO_INV: Self = $field {
                c0: $base::TWO_INV,
                c1: $base::ZERO,
            };

            fn from_repr(repr: Self::Repr) -> CtOption<Self> {
                let c0: [u8; $base::SIZE] = repr[..$base::SIZE].try_into().unwrap();
                let c0: <$base as PrimeField>::Repr = c0.into();
                let c0 = $base::from_repr(c0);

                let c1: [u8; $base::SIZE] = repr[$base::SIZE..].try_into().unwrap();
                let c1: <$base as PrimeField>::Repr = c1.into();
                let c1 = $base::from_repr(c1);

                CtOption::new($field::new(c0.unwrap(), c1.unwrap()), Choice::from(1))
            }

            fn to_repr(&self) -> Self::Repr {
                let mut res = Self::Repr::default();
                let c0 = self.c0.to_repr();
                let c1 = self.c1.to_repr();
                res[0..$base::SIZE].copy_from_slice(&c0.as_ref()[..]);
                res[$base::SIZE..$base::SIZE * 2].copy_from_slice(&c1.as_ref()[..]);
                res
            }

            fn is_odd(&self) -> Choice {
                Choice::from(self.to_repr().as_ref()[0] & 1)
            }
        }

        impl $crate::serde::SerdeObject for $field {
            fn from_raw_bytes_unchecked(bytes: &[u8]) -> Self {
                debug_assert_eq!(bytes.len(), $base::SIZE * 2);
                let [c0, c1] = [0, $base::SIZE]
                    .map(|i| $base::from_raw_bytes_unchecked(&bytes[i..i + $base::SIZE]));
                Self { c0, c1 }
            }
            fn from_raw_bytes(bytes: &[u8]) -> Option<Self> {
                if bytes.len() != $base::SIZE * 2 {
                    return None;
                }
                let [c0, c1] =
                    [0, $base::SIZE].map(|i| $base::from_raw_bytes(&bytes[i..i + $base::SIZE]));
                c0.zip(c1).map(|(c0, c1)| Self { c0, c1 })
            }

            #[cfg(feature = "std")]
            fn to_raw_bytes(&self) -> Vec<u8> {
                let mut res = Vec::with_capacity($base::SIZE * 2);
                for limb in self.c0.0.iter().chain(self.c1.0.iter()) {
                    res.extend_from_slice(&limb.to_le_bytes());
                }
                res
            }
            #[cfg(feature = "std")]
            fn read_raw_unchecked<R: std::io::Read>(reader: &mut R) -> Self {
                let [c0, c1] = [(); 2].map(|_| $base::read_raw_unchecked(reader));
                Self { c0, c1 }
            }
            #[cfg(feature = "std")]
            fn read_raw<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
                let c0 = $base::read_raw(reader)?;
                let c1 = $base::read_raw(reader)?;
                Ok(Self { c0, c1 })
            }
            #[cfg(feature = "std")]
            fn write_raw<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
                self.c0.write_raw(writer)?;
                self.c1.write_raw(writer)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_tower2_from_uniform_bytes {
    (
        $base:ident,
        $field:ident,
        $size:expr
    ) => {
        impl FromUniformBytes<{ $size }> for $field {
            fn from_uniform_bytes(bytes: &[u8; $size]) -> Self {
                assert!($size % 2 == 0);
                const SIZE: usize = $size / 2;
                let c0: [u8; SIZE] = bytes[SIZE..].try_into().unwrap();
                let c1: [u8; SIZE] = bytes[..SIZE].try_into().unwrap();
                Self::new(
                    $base::from_uniform_bytes(&c0),
                    $base::from_uniform_bytes(&c1),
                )
            }
        }
    };
}

#[macro_export]
macro_rules! impl_cyclotomic_square {
    (
        $tower2:ident,
        $tower12:ident
    ) => {
        impl $tower12 {
            pub fn cyclotomic_square(&mut self) {
                fn fp4_square(c0: &mut $tower2, c1: &mut $tower2, a0: &$tower2, a1: &$tower2) {
                    use ff::Field;
                    let t0 = a0.square();
                    let t1 = a1.square();
                    *c0 = t1.mul_by_nonresidue() + t0;
                    *c1 = (a0 + a1).square() - t0 - t1;
                }

                let mut t3 = $tower2::zero();
                let mut t4 = $tower2::zero();
                let mut t5 = $tower2::zero();
                let mut t6 = $tower2::zero();
                fp4_square(&mut t3, &mut t4, &self.c0.c0, &self.c1.c1);

                self.c0.c0 = (t3 - self.c0.c0).double() + t3;
                self.c1.c1 = (t4 + self.c1.c1).double() + t4;

                fp4_square(&mut t3, &mut t4, &self.c1.c0, &self.c0.c2);
                fp4_square(&mut t5, &mut t6, &self.c0.c1, &self.c1.c2);

                self.c0.c1 = (t3 - self.c0.c1).double() + t3;
                self.c1.c2 = (t4 + self.c1.c2).double() + t4;

                let t3 = t6.mul_by_nonresidue();
                self.c1.c0 = (t3 + self.c1.c0).double() + t3;
                self.c0.c2 = (t5 - self.c0.c2).double() + t5;
            }
        }
    };
}
