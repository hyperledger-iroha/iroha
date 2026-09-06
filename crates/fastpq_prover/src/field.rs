//! Canonical Goldilocks degree-four extension used by FASTPQ binary FRI.

use fastpq_isi::GoldilocksDigest384V1;
use norito::{NoritoDeserialize, NoritoSerialize};

/// Goldilocks prime `2^64 - 2^32 + 1`.
pub const GOLDILOCKS_MODULUS_V1: u64 = 0xffff_ffff_0000_0001;
/// Non-residue in the irreducible extension polynomial `X^4 - 7`.
const FP4_NON_RESIDUE_V1: u64 = 7;

/// Canonically encoded element of `Goldilocks[X] / (X^4 - 7)`.
///
/// The Norito payload is exactly four little-endian field limbs, without an
/// inner struct length or field table. Both archive and slice readers reject
/// coefficients outside the base field before constructing this type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct GoldilocksFp4V1 {
    coefficients: [u64; 4],
}

impl GoldilocksFp4V1 {
    /// Exact byte length of the final V1 polynomial-basis encoding.
    pub const BYTES: usize = 32;
    /// Additive identity.
    pub const ZERO: Self = Self {
        coefficients: [0; 4],
    };
    /// Multiplicative identity.
    pub const ONE: Self = Self {
        coefficients: [1, 0, 0, 0],
    };

    /// Construct an element only when all four coefficients are canonical.
    #[must_use]
    pub fn new(coefficients: [u64; 4]) -> Option<Self> {
        coefficients
            .iter()
            .all(|coefficient| *coefficient < GOLDILOCKS_MODULUS_V1)
            .then_some(Self { coefficients })
    }

    /// Embed one base-field element into the extension.
    #[must_use]
    pub fn from_base(value: u64) -> Option<Self> {
        (value < GOLDILOCKS_MODULUS_V1).then_some(Self {
            coefficients: [value, 0, 0, 0],
        })
    }

    /// Derive an extension element from the first four independent digest lanes.
    #[must_use]
    pub fn from_digest(digest: GoldilocksDigest384V1) -> Self {
        let words = digest.words();
        Self {
            coefficients: [words[0], words[1], words[2], words[3]],
        }
    }

    /// Return the four canonical polynomial-basis coefficients.
    #[must_use]
    pub const fn coefficients(self) -> [u64; 4] {
        self.coefficients
    }

    /// Return whether this is the additive identity.
    #[must_use]
    pub fn is_zero(self) -> bool {
        self == Self::ZERO
    }

    /// Encode four canonical little-endian field coefficients.
    #[must_use]
    pub fn to_le_bytes(self) -> [u8; 32] {
        let mut bytes = [0_u8; 32];
        for (index, coefficient) in self.coefficients.iter().enumerate() {
            bytes[index * 8..index * 8 + 8].copy_from_slice(&coefficient.to_le_bytes());
        }
        bytes
    }

    /// Decode four canonical little-endian field coefficients.
    #[must_use]
    pub fn from_le_bytes(bytes: [u8; 32]) -> Option<Self> {
        let mut coefficients = [0_u64; 4];
        for (coefficient, chunk) in coefficients.iter_mut().zip(bytes.chunks_exact(8)) {
            *coefficient = u64::from_le_bytes(chunk.try_into().expect("chunk length is eight"));
        }
        Self::new(coefficients)
    }

    #[cfg(test)]
    pub(crate) const fn from_coefficients_unchecked_for_test(coefficients: [u64; 4]) -> Self {
        Self { coefficients }
    }

    /// Add two extension elements.
    #[must_use]
    pub fn add(self, other: Self) -> Self {
        Self {
            coefficients: core::array::from_fn(|index| {
                add_base(self.coefficients[index], other.coefficients[index])
            }),
        }
    }

    /// Subtract two extension elements.
    #[must_use]
    pub fn sub(self, other: Self) -> Self {
        Self {
            coefficients: core::array::from_fn(|index| {
                sub_base(self.coefficients[index], other.coefficients[index])
            }),
        }
    }

    /// Multiply two extension elements modulo `X^4 - 7`.
    #[must_use]
    pub fn mul(self, other: Self) -> Self {
        let mut product = [0_u64; 7];
        for left in 0..4 {
            for right in 0..4 {
                product[left + right] = add_base(
                    product[left + right],
                    mul_base(self.coefficients[left], other.coefficients[right]),
                );
            }
        }
        for degree in (4..=6).rev() {
            product[degree - 4] = add_base(
                product[degree - 4],
                mul_base(product[degree], FP4_NON_RESIDUE_V1),
            );
        }
        Self {
            coefficients: [product[0], product[1], product[2], product[3]],
        }
    }

    /// Multiply every coefficient by one base-field element.
    #[must_use]
    pub fn mul_base(self, scalar: u64) -> Self {
        debug_assert!(scalar < GOLDILOCKS_MODULUS_V1);
        Self {
            coefficients: self.coefficients.map(|value| mul_base(value, scalar)),
        }
    }
}

impl NoritoSerialize for GoldilocksFp4V1 {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(Self::BYTES)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        Some(Self::BYTES)
    }
}

impl<'de> NoritoDeserialize<'de> for GoldilocksFp4V1 {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("canonical GoldilocksFp4V1 decode")
    }

    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
        let bytes = <[u8; Self::BYTES] as NoritoDeserialize>::try_deserialize(archived.cast())?;
        Self::from_le_bytes(bytes).ok_or_else(|| {
            norito::Error::Message("non-canonical GoldilocksFp4V1 coefficient".into())
        })
    }
}

impl<'de> norito::core::DecodeFromSlice<'de> for GoldilocksFp4V1 {
    fn decode_from_slice(bytes: &'de [u8]) -> Result<(Self, usize), norito::Error> {
        let prefix = bytes
            .get(..Self::BYTES)
            .ok_or(norito::Error::LengthMismatch)?;
        let mut encoded = [0_u8; Self::BYTES];
        encoded.copy_from_slice(prefix);
        let value = Self::from_le_bytes(encoded).ok_or_else(|| {
            norito::Error::Message("non-canonical GoldilocksFp4V1 coefficient".into())
        })?;
        norito::core::note_payload_access(bytes, Self::BYTES);
        Ok((value, Self::BYTES))
    }
}

fn add_base(left: u64, right: u64) -> u64 {
    let sum = u128::from(left) + u128::from(right);
    u64::try_from(sum % u128::from(GOLDILOCKS_MODULUS_V1)).expect("reduction fits u64")
}

fn sub_base(left: u64, right: u64) -> u64 {
    let difference = (u128::from(left) + u128::from(GOLDILOCKS_MODULUS_V1) - u128::from(right))
        % u128::from(GOLDILOCKS_MODULUS_V1);
    u64::try_from(difference).expect("reduction fits u64")
}

fn mul_base(left: u64, right: u64) -> u64 {
    let product = u128::from(left) * u128::from(right);
    u64::try_from(product % u128::from(GOLDILOCKS_MODULUS_V1)).expect("reduction fits u64")
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::{codec::Encode, core::DecodeFromSlice};

    #[test]
    fn norito_wire_is_exactly_four_little_endian_limbs() {
        let value = GoldilocksFp4V1::new([1, 2, 3, GOLDILOCKS_MODULUS_V1 - 1]).unwrap();
        assert_eq!(value.encode(), value.to_le_bytes());
        assert_eq!(value.encoded_len_exact(), Some(GoldilocksFp4V1::BYTES));
        assert_eq!(value.encoded_len_hint(), Some(GoldilocksFp4V1::BYTES));
        let encoded = norito::core::to_bytes(&value).expect("encode extension element");
        assert_eq!(
            norito::decode_from_bytes::<GoldilocksFp4V1>(&encoded).unwrap(),
            value
        );
        for flags in [0, norito::core::default_encode_flags()] {
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            assert_eq!(value.encode(), value.to_le_bytes());
        }
    }

    #[test]
    fn norito_archive_rejects_each_noncanonical_coefficient() {
        for coefficient in 0..4 {
            for invalid in [GOLDILOCKS_MODULUS_V1, u64::MAX] {
                let mut coefficients = [1, 2, 3, 4];
                coefficients[coefficient] = invalid;
                let invalid = GoldilocksFp4V1::from_coefficients_unchecked_for_test(coefficients);
                let encoded = norito::core::to_bytes(&invalid).expect("encode adversarial fixture");
                assert!(
                    norito::decode_from_bytes::<GoldilocksFp4V1>(&encoded).is_err(),
                    "noncanonical coefficient {coefficient} must not enter the typed field"
                );
            }
        }
    }

    #[test]
    fn norito_rejects_the_retired_struct_framing() {
        #[derive(NoritoSerialize)]
        #[norito(schema_name = "fastpq_prover::field::GoldilocksFp4V1")]
        struct RetiredStructFrame {
            coefficients: [u64; 4],
        }

        assert_eq!(
            <RetiredStructFrame as NoritoSerialize>::schema_hash(),
            <GoldilocksFp4V1 as NoritoSerialize>::schema_hash(),
            "exercise payload rejection under the same field schema"
        );
        let retired = RetiredStructFrame {
            coefficients: [1, 2, 3, 4],
        };
        let encoded = norito::core::to_bytes(&retired).expect("encode retired struct fixture");
        assert!(norito::decode_from_bytes::<GoldilocksFp4V1>(&encoded).is_err());
    }

    #[test]
    fn norito_slice_decode_is_bounded_and_canonical() {
        let value = GoldilocksFp4V1::new([1, 2, 3, 4]).unwrap();
        let encoded = value.to_le_bytes();
        for length in 0..GoldilocksFp4V1::BYTES {
            assert!(GoldilocksFp4V1::decode_from_slice(&encoded[..length]).is_err());
        }
        let mut with_suffix = encoded.to_vec();
        with_suffix.extend_from_slice(&[0xA5; 8]);
        assert_eq!(
            GoldilocksFp4V1::decode_from_slice(&with_suffix).unwrap(),
            (value, GoldilocksFp4V1::BYTES)
        );
        for coefficient in 0..4 {
            let mut invalid = encoded;
            invalid[coefficient * 8..coefficient * 8 + 8]
                .copy_from_slice(&GOLDILOCKS_MODULUS_V1.to_le_bytes());
            assert!(GoldilocksFp4V1::decode_from_slice(&invalid).is_err());
        }
    }

    #[test]
    fn canonical_wire_round_trip_and_rejection() {
        let value = GoldilocksFp4V1::new([1, 2, 3, 4]).expect("canonical element");
        assert_eq!(
            GoldilocksFp4V1::from_le_bytes(value.to_le_bytes()),
            Some(value)
        );
        let mut invalid = value.to_le_bytes();
        invalid[8..16].copy_from_slice(&GOLDILOCKS_MODULUS_V1.to_le_bytes());
        assert!(GoldilocksFp4V1::from_le_bytes(invalid).is_none());
    }

    #[test]
    fn multiplication_reduces_x_four_to_seven() {
        let x = GoldilocksFp4V1::new([0, 1, 0, 0]).expect("canonical element");
        let x_squared = x.mul(x);
        assert_eq!(x_squared.coefficients(), [0, 0, 1, 0]);
        assert_eq!(x_squared.mul(x_squared).coefficients(), [7, 0, 0, 0]);
    }

    #[test]
    fn base_embedding_obeys_field_identities() {
        let value = GoldilocksFp4V1::new([9, 8, 7, 6]).expect("canonical element");
        assert_eq!(value.add(GoldilocksFp4V1::ZERO), value);
        assert_eq!(value.mul(GoldilocksFp4V1::ONE), value);
        assert_eq!(value.sub(value), GoldilocksFp4V1::ZERO);
        assert_eq!(
            value.mul_base(3),
            value.mul(GoldilocksFp4V1::from_base(3).unwrap())
        );
    }
}
