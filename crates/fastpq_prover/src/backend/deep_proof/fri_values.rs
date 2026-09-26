//! Canonical fixed-arity FRI fiber values for the inactive DEEP proof.

use std::ops::Deref;

use super::{Fp4, Result, shape};

/// One complete FRI fiber. Its arity byte is followed by exactly that many
/// canonical 32-byte extension-field values, without sequence or cell framing.
#[derive(Clone, Debug, PartialEq, Eq, norito::NoritoSchema)]
#[norito_schema(name = "fastpq_prover::deep_compact::FriFiberValuesV1")]
pub(in crate::backend) enum FriValues {
    /// Four values in transcript order.
    Four([Fp4; 4]),
    /// Eight values in transcript order.
    Eight([Fp4; 8]),
    /// Sixteen values in transcript order.
    Sixteen([Fp4; 16]),
}

impl FriValues {
    /// Accept only arities in the fixed five-round FRI geometry.
    pub(in crate::backend) fn new(values: Vec<Fp4>) -> Result<Self> {
        match values.len() {
            4 => Ok(Self::Four(values.try_into().expect("checked four values"))),
            8 => Ok(Self::Eight(
                values.try_into().expect("checked eight values"),
            )),
            16 => Ok(Self::Sixteen(
                values.try_into().expect("checked sixteen values"),
            )),
            _ => Err(shape("DEEP FRI fiber has an unsupported arity")),
        }
    }

    /// Exact payload width at one protocol-selected arity.
    pub(super) const fn encoded_bytes(arity: usize) -> usize {
        1 + arity * Fp4::BYTES
    }
}

impl Deref for FriValues {
    type Target = [Fp4];

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Four(values) => values,
            Self::Eight(values) => values,
            Self::Sixteen(values) => values,
        }
    }
}

#[cfg(test)]
impl std::ops::DerefMut for FriValues {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Four(values) => values,
            Self::Eight(values) => values,
            Self::Sixteen(values) => values,
        }
    }
}

impl norito::core::SerializePayload for FriValues {
    fn serialize(
        &self,
        writer: &mut norito::core::Encoder<'_>,
    ) -> std::result::Result<(), norito::Error> {
        writer.write_all(&[self.len() as u8])?;
        for value in self.iter() {
            writer.write_all(&value.to_le_bytes())?;
        }
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(Self::encoded_bytes(self.len()))
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        Some(Self::encoded_bytes(self.len()))
    }
}

impl<'de> norito::core::DeserializePayload<'de> for FriValues {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("canonical DEEP FRI fiber decode")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> std::result::Result<Self, norito::Error> {
        let pointer = std::ptr::from_ref(archived).cast::<u8>();
        let mut offset = 0;
        let [arity] = norito::core::decode_context_byte_array::<1>(pointer, &mut offset)?;
        if !matches!(arity, 4 | 8 | 16) {
            return Err(norito::Error::Message(
                "invalid DEEP FRI fiber arity".into(),
            ));
        }
        let mut values = [Fp4::ZERO; 16];
        for value in values.iter_mut().take(usize::from(arity)) {
            let bytes =
                norito::core::decode_context_byte_array::<{ Fp4::BYTES }>(pointer, &mut offset)?;
            *value = Fp4::from_le_bytes(bytes).ok_or_else(|| {
                norito::Error::Message("noncanonical DEEP FRI fiber scalar".into())
            })?;
        }
        norito::core::finish_context_fields(pointer, offset)?;
        Ok(match arity {
            4 => Self::Four(values[..4].try_into().expect("four decoded values")),
            8 => Self::Eight(values[..8].try_into().expect("eight decoded values")),
            16 => Self::Sixteen(values),
            _ => unreachable!("arity checked before decoding"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::GOLDILOCKS_MODULUS;

    fn values(arity: usize) -> Vec<Fp4> {
        (0..arity)
            .map(|index| Fp4::new([index as u64, 3, GOLDILOCKS_MODULUS - 1, 7]).unwrap())
            .collect()
    }

    #[test]
    fn fixed_fibers_roundtrip_exact_raw_bytes_under_every_norito_layout() {
        for arity in [4, 8, 16] {
            let values = values(arity);
            let fiber = FriValues::new(values.clone()).unwrap();
            let mut expected = vec![arity as u8];
            for value in &values {
                expected.extend_from_slice(&value.to_le_bytes());
            }
            assert_eq!(expected.len(), FriValues::encoded_bytes(arity));
            assert_eq!(&*fiber, values);
            for flags in (u8::MIN..=u8::MAX)
                .filter(|&flags| norito::core::validate_header_flags(flags).is_ok())
            {
                let _flags = norito::core::DecodeFlagsGuard::enter(flags);
                assert_eq!(norito::codec::encode_with_header_flags(&fiber).0, expected);
                let (decoded, consumed) =
                    norito::core::decode_field_canonical::<FriValues>(&expected).unwrap();
                assert_eq!(decoded, fiber);
                assert_eq!(consumed, expected.len());
            }
        }
    }

    #[test]
    fn fixed_fibers_reject_wrong_arity_length_legacy_vector_and_noncanonical_limbs() {
        for arity in [0, 1, 2, 3, 5, 7, 9, 15, 17] {
            assert!(FriValues::new(values(arity)).is_err());
        }
        for arity in [4, 8, 16] {
            let fiber = FriValues::new(values(arity)).unwrap();
            let mut raw = norito::codec::encode_with_header_flags(&fiber).0;
            for tag in [0, 1, 2, 3, 5, 8_u8.wrapping_add(arity as u8), 32] {
                if tag == arity as u8 {
                    continue;
                }
                raw[0] = tag;
                assert!(norito::core::decode_field_canonical::<FriValues>(&raw).is_err());
            }
            raw[0] = arity as u8;
            for length in [0, 1, raw.len() - 1, raw.len() + 1] {
                let mut changed = raw.clone();
                changed.resize(length, 0);
                assert!(norito::core::decode_field_canonical::<FriValues>(&changed).is_err());
            }
            for position in [0, arity - 1] {
                for limb in [0, 3] {
                    let mut changed = raw.clone();
                    let start = 1 + position * Fp4::BYTES + limb * 8;
                    changed[start..start + 8].copy_from_slice(&GOLDILOCKS_MODULUS.to_le_bytes());
                    assert!(norito::core::decode_field_canonical::<FriValues>(&changed).is_err());
                }
            }
            let legacy = norito::codec::encode_with_header_flags(&values(arity)).0;
            assert!(norito::core::decode_field_canonical::<FriValues>(&legacy).is_err());
        }
    }
}
