use crate::ff::{FromUniformBytes, PrimeField};
#[cfg(not(feature = "halo2-axiom"))]
use crate::halo2_proofs::arithmetic::CurveAffine;
use crate::halo2_proofs::circuit::Value;
#[cfg(feature = "halo2-axiom")]
pub use crate::halo2_proofs::halo2curves::CurveAffineExt;

use num_bigint::BigInt;
use num_bigint::BigUint;
use num_bigint::Sign;
use num_traits::Signed;
use num_traits::{One, Zero};

/// Helper functions for raw halo2 operations to unify slight differences in API for halo2-axiom and halo2-pse
pub mod halo2;
#[cfg(any(test, feature = "test-utils"))]
pub mod testing;

/// Helper trait to convert to and from a [BigPrimeField] by converting a list of [u64] digits
#[cfg(feature = "halo2-axiom")]
pub trait BigPrimeField: ScalarField {
    /// Converts a slice of [u64] to [BigPrimeField]
    /// * `val`: the slice of u64
    ///
    /// # Assumptions
    /// * `val` has the correct length for the implementation
    /// * The integer value of `val` is already less than the modulus of `Self`
    fn from_u64_digits(val: &[u64]) -> Self;
}
#[cfg(feature = "halo2-axiom")]
impl<F: ScalarField> BigPrimeField for F {
    #[inline(always)]
    fn from_u64_digits(val: &[u64]) -> Self {
        debug_assert!(val.len() <= 4);
        let mut repr = F::Repr::default();
        for (chunk, digit) in repr.as_mut().chunks_exact_mut(8).zip(val) {
            chunk.copy_from_slice(&digit.to_le_bytes());
        }
        F::from_repr(repr).unwrap()
    }
}

/// Helper trait to represent a field element that can be converted into [u64] limbs.
///
/// Note: Since the number of bits necessary to represent a field element is larger than the number of bits in a u64, we decompose the integer representation of the field element into multiple [u64] values e.g. `limbs`.
pub trait ScalarField: PrimeField + FromUniformBytes<64> + From<bool> + Ord {
    /// Returns the base `2<sup>bit_len</sup>` little endian representation of the [ScalarField] element up to `num_limbs` number of limbs (truncates any extra limbs).
    ///
    /// Assumes `bit_len < 64`.
    /// * `num_limbs`: number of limbs to return
    /// * `bit_len`: number of bits in each limb
    fn to_u64_limbs(self, num_limbs: usize, bit_len: usize) -> Vec<u64>;

    /// Returns the little endian byte representation of the element.
    fn to_bytes_le(&self) -> Vec<u8>;

    /// Creates a field element from a little endian byte representation.
    ///
    /// The default implementation assumes that `PrimeField::from_repr` is implemented for little-endian.
    /// It should be overriden if this is not the case.
    fn from_bytes_le(bytes: &[u8]) -> Self {
        let mut repr = Self::Repr::default();
        repr.as_mut()[..bytes.len()].copy_from_slice(bytes);
        Self::from_repr(repr).unwrap()
    }

    /// Gets the least significant 32 bits of the field element.
    fn get_lower_32(&self) -> u32 {
        let bytes = self.to_bytes_le();
        let mut lower_32 = 0u32;
        for (i, byte) in bytes.into_iter().enumerate().take(4) {
            lower_32 |= (byte as u32) << (i * 8);
        }
        lower_32
    }

    /// Gets the least significant 64 bits of the field element.
    fn get_lower_64(&self) -> u64 {
        let bytes = self.to_bytes_le();
        let mut lower_64 = 0u64;
        for (i, byte) in bytes.into_iter().enumerate().take(8) {
            lower_64 |= (byte as u64) << (i * 8);
        }
        lower_64
    }
}
// See below for implementations

// Later: will need to separate BigPrimeField from ScalarField when Goldilocks is introduced

/// [ScalarField] that is ~256 bits long
#[cfg(feature = "halo2-pse")]
pub trait BigPrimeField: PrimeField<Repr = [u8; 32]> + ScalarField {}
#[cfg(feature = "halo2-pse")]
impl<F: PrimeField<Repr = [u8; 32]> + ScalarField> BigPrimeField for F {}

/// Converts an [Iterator] of u64 digits into `number_of_limbs` limbs of `bit_len` bits returned as a [Vec].
///
/// Assumes: `bit_len < 64`.
/// * `e`: Iterator of [u64] digits
/// * `number_of_limbs`: number of limbs to return
/// * `bit_len`: number of bits in each limb
#[inline(always)]
pub(crate) fn decompose_u64_digits_to_limbs(
    e: impl IntoIterator<Item = u64>,
    number_of_limbs: usize,
    bit_len: usize,
) -> Vec<u64> {
    debug_assert!(bit_len < 64);

    let mut e = e.into_iter();
    // Mask to extract the bits from each digit
    let mask: u64 = (1u64 << bit_len) - 1u64;
    let mut u64_digit = e.next().unwrap_or(0);
    let mut rem = 64;

    // For each digit, we extract its individual limbs by repeatedly masking and shifting the digit based on how many bits we have left to extract.
    (0..number_of_limbs)
        .map(|_| match rem.cmp(&bit_len) {
            // If `rem` > `bit_len`, we mask the bits from the `u64_digit` to return the first limb.
            // We shift the digit to the right by `bit_len` bits and subtract `bit_len` from `rem`
            core::cmp::Ordering::Greater => {
                let limb = u64_digit & mask;
                u64_digit >>= bit_len;
                rem -= bit_len;
                limb
            }
            // If `rem` == `bit_len`, then we mask the bits from the `u64_digit` to return the first limb
            // We retrieve the next digit and reset `rem` to 64
            core::cmp::Ordering::Equal => {
                let limb = u64_digit & mask;
                u64_digit = e.next().unwrap_or(0);
                rem = 64;
                limb
            }
            // If `rem` < `bit_len`, we retrieve the next digit, mask it, and shift left `rem` bits from the `u64_digit` to return the first limb.
            // we shift the digit to the right by `bit_len` - `rem` bits to retrieve the start of the next limb and add 64 - bit_len to `rem` to get the remainder.
            core::cmp::Ordering::Less => {
                let mut limb = u64_digit;
                u64_digit = e.next().unwrap_or(0);
                limb |= (u64_digit & ((1u64 << (bit_len - rem)) - 1u64)) << rem;
                u64_digit >>= bit_len - rem;
                rem += 64 - bit_len;
                limb
            }
        })
        .collect()
}

/// Returns the number of bits needed to represent the value of `x`.
pub const fn bit_length(x: u64) -> usize {
    (u64::BITS - x.leading_zeros()) as usize
}

/// Returns the ceiling of the base 2 logarithm of `x`.
///
/// `log2_ceil(0)` returns 0.
pub fn log2_ceil(x: u64) -> usize {
    (u64::BITS - x.leading_zeros()) as usize - usize::from(x.is_power_of_two())
}

/// Returns the modulus of [BigPrimeField].
pub fn modulus<F: BigPrimeField>() -> BigUint {
    fe_to_biguint(&-F::ONE) + 1u64
}

/// Returns the [BigPrimeField] element of 2<sup>n</sup>.
/// * `n`: the desired power of 2.
pub fn power_of_two<F: BigPrimeField>(n: usize) -> F {
    biguint_to_fe(&(BigUint::one() << n))
}

/// Converts an immutable reference to [BigUint] to a [BigPrimeField].
/// * `e`: immutable reference to [BigUint]
///
/// # Assumptions:
/// * `e` is less than the modulus of `F`
pub fn biguint_to_fe<F: BigPrimeField>(e: &BigUint) -> F {
    #[cfg(feature = "halo2-axiom")]
    {
        F::from_u64_digits(&e.to_u64_digits())
    }

    #[cfg(feature = "halo2-pse")]
    {
        let bytes = e.to_bytes_le();
        F::from_bytes_le(&bytes)
    }
}

/// Converts an immutable reference to [BigInt] to a [BigPrimeField].
/// * `e`: immutable reference to [BigInt]
///
/// # Assumptions:
/// * The absolute value of `e` is less than the modulus of `F`
pub fn bigint_to_fe<F: BigPrimeField>(e: &BigInt) -> F {
    #[cfg(feature = "halo2-axiom")]
    {
        let (sign, digits) = e.to_u64_digits();
        if sign == Sign::Minus {
            -F::from_u64_digits(&digits)
        } else {
            F::from_u64_digits(&digits)
        }
    }
    #[cfg(feature = "halo2-pse")]
    {
        let (sign, bytes) = e.to_bytes_le();
        let f_abs = F::from_bytes_le(&bytes);
        if sign == Sign::Minus {
            -f_abs
        } else {
            f_abs
        }
    }
}

/// Converts an immutable reference to an PrimeField element into a [BigUint] element.
/// * `fe`: immutable reference to PrimeField element to convert
pub fn fe_to_biguint<F: ScalarField>(fe: &F) -> BigUint {
    BigUint::from_bytes_le(fe.to_bytes_le().as_ref())
}

/// Converts a [BigPrimeField] element into a [BigInt] element by sending `fe` in `[0, F::modulus())` to
/// ```ignore
/// fe,                 if fe < F::modulus() / 2
/// fe - F::modulus(),  otherwise
/// ```
pub fn fe_to_bigint<F: BigPrimeField>(fe: &F) -> BigInt {
    // TODO: `F` should just have modulus as lazy_static or something
    let modulus = modulus::<F>();
    let e = fe_to_biguint(fe);
    if e <= &modulus / 2u32 {
        BigInt::from_biguint(Sign::Plus, e)
    } else {
        BigInt::from_biguint(Sign::Minus, modulus - e)
    }
}

/// Decomposes an immutable reference to a [BigPrimeField] element into `number_of_limbs` limbs of `bit_len` bits each and returns a [Vec] of [BigPrimeField] represented by those limbs.
///
/// Assumes `bit_len < 128`.
/// * `e`: immutable reference to [BigPrimeField] element to decompose
/// * `number_of_limbs`: number of limbs to decompose `e` into
/// * `bit_len`: number of bits in each limb
pub fn decompose<F: BigPrimeField>(e: &F, number_of_limbs: usize, bit_len: usize) -> Vec<F> {
    if bit_len >= 64 {
        decompose_biguint(&fe_to_biguint(e), number_of_limbs, bit_len)
    } else {
        decompose_fe_to_u64_limbs(e, number_of_limbs, bit_len)
            .into_iter()
            .map(F::from)
            .collect()
    }
}

/// Decomposes an immutable reference to a [ScalarField] element into `number_of_limbs` limbs of `bit_len` bits each and returns a [Vec] of [u64] represented by those limbs.
///
/// Assumes `bit_len` < 64
/// * `e`: immutable reference to [ScalarField] element to decompose
/// * `number_of_limbs`: number of limbs to decompose `e` into
/// * `bit_len`: number of bits in each limb
pub fn decompose_fe_to_u64_limbs<F: ScalarField>(
    e: &F,
    number_of_limbs: usize,
    bit_len: usize,
) -> Vec<u64> {
    #[cfg(feature = "halo2-axiom")]
    {
        e.to_u64_limbs(number_of_limbs, bit_len)
    }

    #[cfg(feature = "halo2-pse")]
    {
        decompose_u64_digits_to_limbs(fe_to_biguint(e).iter_u64_digits(), number_of_limbs, bit_len)
    }
}

/// Decomposes an immutable reference to a [BigUint] into `num_limbs` limbs of `bit_len` bits each and returns a [Vec] of [BigPrimeField] represented by those limbs.
///
/// Assumes 64 <= `bit_len` < 128.
/// * `e`: immutable reference to [BigInt] to decompose
/// * `num_limbs`: number of limbs to decompose `e` into
/// * `bit_len`: number of bits in each limb
///
/// Truncates to `num_limbs` limbs if `e` is too large.
pub fn decompose_biguint<F: BigPrimeField>(
    e: &BigUint,
    num_limbs: usize,
    bit_len: usize,
) -> Vec<F> {
    // bit_len must be between 64` and 128
    debug_assert!((64..128).contains(&bit_len));
    let mut e = e.iter_u64_digits();

    // Grab first 128-bit limb from iterator
    let mut limb0 = e.next().unwrap_or(0) as u128;
    let mut rem = bit_len - 64;
    let mut u64_digit = e.next().unwrap_or(0);
    // Extract second limb (bit length 64) from e
    limb0 |= ((u64_digit & ((1u64 << rem) - 1u64)) as u128) << 64u32;
    u64_digit >>= rem;
    rem = 64 - rem;

    // Convert `limb0` into field element `F` and create an iterator by chaining `limb0` with the computing the remaining limbs
    core::iter::once(F::from_u128(limb0))
        .chain((1..num_limbs).map(|_| {
            let mut limb = u64_digit as u128;
            let mut bits = rem;
            u64_digit = e.next().unwrap_or(0);
            if bit_len >= 64 + bits {
                limb |= (u64_digit as u128) << bits;
                u64_digit = e.next().unwrap_or(0);
                bits += 64;
            }
            rem = bit_len - bits;
            limb |= ((u64_digit & ((1u64 << rem) - 1u64)) as u128) << bits;
            u64_digit >>= rem;
            rem = 64 - rem;
            F::from_u128(limb)
        }))
        .collect()
}

/// Decomposes an immutable reference to a [BigInt] into `num_limbs` limbs of `bit_len` bits each and returns a [Vec] of [BigPrimeField] represented by those limbs.
///
/// Assumes `bit_len < 128`.
/// * `e`: immutable reference to `BigInt` to decompose
/// * `num_limbs`: number of limbs to decompose `e` into
/// * `bit_len`: number of bits in each limb
pub fn decompose_bigint<F: BigPrimeField>(e: &BigInt, num_limbs: usize, bit_len: usize) -> Vec<F> {
    if e.is_negative() {
        decompose_biguint::<F>(e.magnitude(), num_limbs, bit_len)
            .into_iter()
            .map(|x| -x)
            .collect()
    } else {
        decompose_biguint(e.magnitude(), num_limbs, bit_len)
    }
}

/// Decomposes an immutable reference to a [BigInt] into `num_limbs` limbs of `bit_len` bits each and returns a [Vec] of [BigPrimeField] represented by those limbs wrapped in [Value].
///
/// Assumes `bit_len` < 128.
/// * `e`: immutable reference to `BigInt` to decompose
/// * `num_limbs`: number of limbs to decompose `e` into
/// * `bit_len`: number of bits in each limb
pub fn decompose_bigint_option<F: BigPrimeField>(
    value: Value<&BigInt>,
    number_of_limbs: usize,
    bit_len: usize,
) -> Vec<Value<F>> {
    value
        .map(|e| decompose_bigint(e, number_of_limbs, bit_len))
        .transpose_vec(number_of_limbs)
}

/// Wraps the internal value of `value` in an [Option].
/// If the value is [None], then the function returns [None].
/// * `value`: Value to convert.
pub fn value_to_option<V>(value: Value<V>) -> Option<V> {
    let mut v = None;
    value.map(|val| {
        v = Some(val);
    });
    v
}

/// Computes the value of an integer by passing as `input` a [Vec] of its limb values and the `bit_len` (bit length) used.
///
/// Returns the sum of all limbs scaled by 2<sup>(bit_len * i)</sup> where i is the index of the limb.
/// * `input`: Limb values of the integer.
/// * `bit_len`: Length of limb in bits
pub fn compose(input: Vec<BigUint>, bit_len: usize) -> BigUint {
    input
        .iter()
        .rev()
        .fold(BigUint::zero(), |acc, val| (acc << bit_len) + val)
}

/// Helper trait
#[cfg(feature = "halo2-pse")]
pub trait CurveAffineExt: CurveAffine {
    /// Returns the raw affine (X, Y) coordinantes
    fn into_coordinates(self) -> (Self::Base, Self::Base) {
        let coordinates = self.coordinates().unwrap();
        (*coordinates.x(), *coordinates.y())
    }
}
#[cfg(feature = "halo2-pse")]
impl<C: CurveAffine> CurveAffineExt for C {}

mod scalar_field_impls {
    use super::{decompose_u64_digits_to_limbs, ScalarField};
    use crate::ff::PrimeField;
    use crate::halo2_proofs::halo2curves::{
        bn256::{Fq as bn254Fq, Fr as bn254Fr},
        pasta::{Fp as pastaFp, Fq as pastaFq},
        secp256k1::{Fp as secpFp, Fq as secpFq},
        secp256r1::{Fp as p256Fp, Fq as p256Fq},
    };

    /// To ensure `ScalarField` is only implemented for `ff:Field` where `Repr` is little endian, we use the following macro
    /// to implement the trait for each field.
    #[cfg(feature = "halo2-axiom")]
    #[macro_export]
    macro_rules! impl_scalar_field {
        ($field:ident) => {
            impl ScalarField for $field {
                #[inline(always)]
                fn to_u64_limbs(self, num_limbs: usize, bit_len: usize) -> Vec<u64> {
                    // Basically same as `to_repr` but does not go further into bytes
                    let tmp: [u64; 4] = self.into();
                    decompose_u64_digits_to_limbs(tmp, num_limbs, bit_len)
                }

                #[inline(always)]
                fn to_bytes_le(&self) -> Vec<u8> {
                    let tmp: [u64; 4] = (*self).into();
                    tmp.iter().flat_map(|x| x.to_le_bytes()).collect()
                }

                #[inline(always)]
                fn get_lower_32(&self) -> u32 {
                    let tmp: [u64; 4] = (*self).into();
                    tmp[0] as u32
                }

                #[inline(always)]
                fn get_lower_64(&self) -> u64 {
                    let tmp: [u64; 4] = (*self).into();
                    tmp[0]
                }
            }
        };
    }

    #[cfg(feature = "halo2-axiom")]
    macro_rules! impl_scalar_field_from_repr {
        ($field:ident) => {
            impl ScalarField for $field {
                #[inline(always)]
                fn to_u64_limbs(self, num_limbs: usize, bit_len: usize) -> Vec<u64> {
                    let repr = self.to_repr();
                    let digits = repr.as_ref().chunks_exact(8).map(|chunk| {
                        u64::from_le_bytes(chunk.try_into().expect("eight-byte field limb"))
                    });
                    decompose_u64_digits_to_limbs(digits, num_limbs, bit_len)
                }

                #[inline(always)]
                fn to_bytes_le(&self) -> Vec<u8> {
                    self.to_repr().as_ref().to_vec()
                }

                #[inline(always)]
                fn get_lower_32(&self) -> u32 {
                    let repr = self.to_repr();
                    u32::from_le_bytes(repr.as_ref()[..4].try_into().unwrap())
                }

                #[inline(always)]
                fn get_lower_64(&self) -> u64 {
                    let repr = self.to_repr();
                    u64::from_le_bytes(repr.as_ref()[..8].try_into().unwrap())
                }
            }
        };
    }

    /// To ensure `ScalarField` is only implemented for `ff:Field` where `Repr` is little endian, we use the following macro
    /// to implement the trait for each field.
    #[cfg(feature = "halo2-pse")]
    #[macro_export]
    macro_rules! impl_scalar_field {
        ($field:ident) => {
            impl ScalarField for $field {
                #[inline(always)]
                fn to_u64_limbs(self, num_limbs: usize, bit_len: usize) -> Vec<u64> {
                    let bytes = self.to_repr();
                    let digits = (0..4)
                        .map(|i| u64::from_le_bytes(bytes[i * 8..(i + 1) * 8].try_into().unwrap()));
                    decompose_u64_digits_to_limbs(digits, num_limbs, bit_len)
                }

                #[inline(always)]
                fn to_bytes_le(&self) -> Vec<u8> {
                    self.to_repr().to_vec()
                }
            }
        };
    }

    impl_scalar_field!(bn254Fr);
    impl_scalar_field!(bn254Fq);
    impl_scalar_field!(secpFp);
    impl_scalar_field!(secpFq);
    #[cfg(feature = "halo2-axiom")]
    impl_scalar_field_from_repr!(pastaFp);
    #[cfg(feature = "halo2-axiom")]
    impl_scalar_field_from_repr!(pastaFq);
    #[cfg(feature = "halo2-axiom")]
    impl_scalar_field_from_repr!(p256Fp);
    #[cfg(feature = "halo2-axiom")]
    impl_scalar_field_from_repr!(p256Fq);
}

/// Module for reading parameters for Halo2 proving system from the file system.
pub mod fs {
    use std::{
        env::var,
        fs::{self, File},
        io::{BufReader, BufWriter},
    };

    use crate::halo2_proofs::{
        halo2curves::{
            bn256::{Bn256, G1Affine},
            CurveAffine,
        },
        poly::{
            commitment::{Params, ParamsProver},
            kzg::commitment::ParamsKZG,
        },
    };
    use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};

    /// Reads the srs from a file found in `./params/kzg_bn254_{k}.srs` or `{dir}/kzg_bn254_{k}.srs` if `PARAMS_DIR` env var is specified.
    /// * `k`: degree that expresses the size of circuit (i.e., 2^<sup>k</sup> is the number of rows in the circuit)
    pub fn read_params(k: u32) -> ParamsKZG<Bn256> {
        let dir = var("PARAMS_DIR").unwrap_or_else(|_| "./params".to_string());
        ParamsKZG::<Bn256>::read(&mut BufReader::new(
            File::open(format!("{dir}/kzg_bn254_{k}.srs").as_str())
                .expect("Params file does not exist"),
        ))
        .unwrap()
    }

    /// Attempts to read the srs from a file found in `./params/kzg_bn254_{k}.srs` or `{dir}/kzg_bn254_{k}.srs` if `PARAMS_DIR` env var is specified, creates a file it if it does not exist.
    /// * `k`: degree that expresses the size of circuit (i.e., 2^<sup>k</sup> is the number of rows in the circuit)
    /// * `setup`: a function that creates the srs
    pub fn read_or_create_srs<'a, C: CurveAffine, P: ParamsProver<'a, C>>(
        k: u32,
        setup: impl Fn(u32) -> P,
    ) -> P {
        let dir = var("PARAMS_DIR").unwrap_or_else(|_| "./params".to_string());
        let path = format!("{dir}/kzg_bn254_{k}.srs");
        match File::open(path.as_str()) {
            Ok(f) => {
                #[cfg(feature = "display")]
                println!("read params from {path}");
                let mut reader = BufReader::new(f);
                P::read(&mut reader).unwrap()
            }
            Err(_) => {
                #[cfg(feature = "display")]
                println!("creating params for {k}");
                fs::create_dir_all(dir).unwrap();
                let params = setup(k);
                params
                    .write(&mut BufWriter::new(File::create(path).unwrap()))
                    .unwrap();
                params
            }
        }
    }

    /// Generates the SRS for the KZG scheme and writes it to a file found in "./params/kzg_bn2_{k}.srs` or `{dir}/kzg_bn254_{k}.srs` if `PARAMS_DIR` env var is specified, creates a file it if it does not exist"
    /// * `k`: degree that expresses the size of circuit (i.e., 2^<sup>k</sup> is the number of rows in the circuit)
    pub fn gen_srs(k: u32) -> ParamsKZG<Bn256> {
        read_or_create_srs::<G1Affine, _>(k, |k| {
            ParamsKZG::<Bn256>::setup(k, ChaCha20Rng::from_seed(Default::default()))
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::halo2_proofs::halo2curves::{
        bn256::Fr,
        secp256r1::{Fp as P256Fp, Fq as P256Fq},
    };
    use num_bigint::RandomBits;
    use rand::{
        rngs::{OsRng, StdRng},
        Rng, SeedableRng,
    };
    use std::ops::Shl;

    use super::*;

    fn assert_p256_little_endian_adapter<F>()
    where
        F: BigPrimeField,
    {
        let integer = (BigUint::from(0x0123_4567_89ab_cdef_u64) << 64)
            + BigUint::from(0xfedc_ba98_7654_3210_u64);
        let value = biguint_to_fe::<F>(&integer);
        let bytes = value.to_bytes_le();
        assert_eq!(bytes.as_slice(), value.to_repr().as_ref());
        assert_eq!(BigUint::from_bytes_le(&bytes), integer);

        let limbs = value.to_u64_limbs(20, 13);
        let recomposed = limbs.iter().rev().fold(BigUint::zero(), |acc, limb| {
            (acc << 13) + BigUint::from(*limb)
        });
        assert_eq!(recomposed, integer);
        assert_eq!(value.get_lower_32(), 0x7654_3210);
        assert_eq!(value.get_lower_64(), 0xfedc_ba98_7654_3210);
    }

    #[test]
    fn p256_fields_use_little_endian_scalar_field_adapter() {
        assert_p256_little_endian_adapter::<P256Fp>();
        assert_p256_little_endian_adapter::<P256Fq>();
    }

    #[test]
    fn test_signed_roundtrip() {
        use crate::halo2_proofs::halo2curves::bn256::Fr;
        assert_eq!(
            fe_to_bigint(&bigint_to_fe::<Fr>(&-BigInt::one())),
            -BigInt::one()
        );
    }

    #[test]
    fn test_decompose_biguint() {
        let mut rng = OsRng;
        const MAX_LIMBS: u64 = 5;
        for bit_len in 64..128usize {
            for num_limbs in 1..=MAX_LIMBS {
                for _ in 0..10_000usize {
                    let mut e: BigUint = rng.sample(RandomBits::new(num_limbs * bit_len as u64));
                    let limbs = decompose_biguint::<Fr>(&e, num_limbs as usize, bit_len);

                    let limbs2 = {
                        let mut limbs = vec![];
                        let mask = BigUint::one().shl(bit_len) - 1usize;
                        for _ in 0..num_limbs {
                            let limb = &e & &mask;
                            let mut bytes_le = limb.to_bytes_le();
                            bytes_le.resize(32, 0u8);
                            limbs.push(Fr::from_bytes(&bytes_le.try_into().unwrap()).unwrap());
                            e >>= bit_len;
                        }
                        limbs
                    };
                    assert_eq!(limbs, limbs2);
                }
            }
        }
    }

    #[test]
    fn test_decompose_bit_len_64() {
        let e = Fr::from(1u64) + Fr::from_u128(1u128 << 64);
        assert_eq!(decompose(&e, 2, 64), vec![Fr::from(1u64), Fr::from(1u64)]);
    }

    #[test]
    fn test_decompose_u64_digits_to_limbs() {
        let mut rng = OsRng;
        const MAX_LIMBS: u64 = 5;
        for bit_len in 0..64usize {
            for num_limbs in 1..=MAX_LIMBS {
                for _ in 0..10_000usize {
                    let mut e: BigUint = rng.sample(RandomBits::new(num_limbs * bit_len as u64));
                    let limbs = decompose_u64_digits_to_limbs(
                        e.to_u64_digits(),
                        num_limbs as usize,
                        bit_len,
                    );
                    let limbs2 = {
                        let mut limbs = vec![];
                        let mask = BigUint::one().shl(bit_len) - 1usize;
                        for _ in 0..num_limbs {
                            let limb = &e & &mask;
                            limbs.push(u64::try_from(limb).unwrap());
                            e >>= bit_len;
                        }
                        limbs
                    };
                    assert_eq!(limbs, limbs2);
                }
            }
        }
    }

    #[test]
    fn test_log2_ceil_zero() {
        assert_eq!(log2_ceil(0), 0);
    }

    #[test]
    fn test_get_lower_32() {
        let mut rng = StdRng::seed_from_u64(0);
        for _ in 0..10_000usize {
            let e: u32 = rng.gen_range(0..u32::MAX);
            assert_eq!(Fr::from(e as u64).get_lower_32(), e);
        }
        assert_eq!(Fr::from((1u64 << 32_i32) + 1).get_lower_32(), 1);
    }

    #[test]
    fn test_get_lower_64() {
        let mut rng = StdRng::seed_from_u64(0);
        for _ in 0..10_000usize {
            let e: u64 = rng.gen_range(0..u64::MAX);
            assert_eq!(Fr::from(e).get_lower_64(), e);
        }
    }
}
