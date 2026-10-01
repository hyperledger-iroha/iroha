//! Canonical fixed-arity FRI fiber values for the inactive DEEP proof.

use std::ops::Deref;

use super::{Fp4, Result, shape};

/// One compressed FRI fiber. Its arity byte is followed by exactly arity minus one
/// canonical 32-byte extension-field values, without sequence or cell framing.
#[allow(
    clippy::large_enum_variant,
    reason = "fibers are inline fixed-arity values (at most 16 * 32 bytes); boxing the \
              sixteen-value variant would add one heap allocation per opened fiber"
)]
#[derive(Clone, Debug, PartialEq, Eq, norito::NoritoSchema)]
#[norito_schema(name = "fastpq_prover::deep_compact::OmittedKnownFriFiberValuesV1")]
pub(in crate::backend) enum FriValues {
    /// Three transmitted values from an arity-four fiber.
    Four([Fp4; 3]),
    /// Seven transmitted values from an arity-eight fiber.
    Eight([Fp4; 7]),
    /// Fifteen transmitted values from an arity-sixteen fiber.
    Sixteen([Fp4; 15]),
}

impl FriValues {
    /// Omit the verifier-selected independently known coordinate without copying
    /// the full private fiber into an intermediate unguarded heap allocation.
    pub(in crate::backend) fn omit(values: &[Fp4], omitted: usize) -> Result<Self> {
        if !matches!(values.len(), 4 | 8 | 16) || omitted >= values.len() {
            return Err(shape("compact FRI omission has another fixed shape"));
        }
        let at = |i| values[if i < omitted { i } else { i + 1 }];
        Ok(match values.len() {
            4 => Self::Four(core::array::from_fn(at)),
            8 => Self::Eight(core::array::from_fn(at)),
            16 => Self::Sixteen(core::array::from_fn(at)),
            _ => unreachable!(),
        })
    }
    /// Reinsert an authenticated incoming value into one bounded caller-owned
    /// array. Only its arity-sized prefix is part of the reconstructed oracle leaf.
    pub(in crate::backend) fn expand(&self, omitted: usize, known: Fp4) -> Result<[Fp4; 16]> {
        if omitted >= self.arity() {
            return Err(shape("compact FRI omitted coordinate outside arity"));
        }
        let mut full = [Fp4::ZERO; 16];
        for (i, value) in full.iter_mut().take(self.arity()).enumerate() {
            *value = if i == omitted {
                known
            } else {
                self[if i < omitted { i } else { i - 1 }]
            };
        }
        Ok(full)
    }
    /// Full authenticated oracle arity, distinct from the transmitted cell count.
    pub(in crate::backend) const fn arity(&self) -> usize {
        match self {
            Self::Four(_) => 4,
            Self::Eight(_) => 8,
            Self::Sixteen(_) => 16,
        }
    }

    /// Exact payload width at one protocol-selected arity.
    pub(super) const fn encoded_bytes(arity: usize) -> usize {
        1 + (arity - 1) * Fp4::BYTES
    }

    /// Canonical leading arity byte of this fixed variant.
    const fn arity_byte(&self) -> u8 {
        match self {
            Self::Four(_) => 4,
            Self::Eight(_) => 8,
            Self::Sixteen(_) => 16,
        }
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
        writer.write_all(&[self.arity_byte()])?;
        for value in self.iter() {
            writer.write_all(&value.to_le_bytes())?;
        }
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(Self::encoded_bytes(self.arity()))
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        Some(Self::encoded_bytes(self.arity()))
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
        for value in values.iter_mut().take(usize::from(arity) - 1) {
            let bytes =
                norito::core::decode_context_byte_array::<{ Fp4::BYTES }>(pointer, &mut offset)?;
            *value = Fp4::from_le_bytes(bytes).ok_or_else(|| {
                norito::Error::Message("noncanonical DEEP FRI fiber scalar".into())
            })?;
        }
        norito::core::finish_context_fields(pointer, offset)?;
        Ok(match arity {
            4 => Self::Four(values[..3].try_into().expect("four decoded values")),
            8 => Self::Eight(values[..7].try_into().expect("eight decoded values")),
            16 => Self::Sixteen(values[..15].try_into().expect("fifteen decoded values")),
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
            .map(|i| Fp4::new([i as u64, 3, GOLDILOCKS_MODULUS - 1, 7]).unwrap())
            .collect()
    }
    #[test]
    fn fixed_fibers_roundtrip_exact_raw_bytes_under_every_norito_layout() {
        for arity in [4, 8, 16] {
            for omitted in 0..arity {
                let values = values(arity);
                let fiber = FriValues::omit(&values, omitted).unwrap();
                let mut expected = vec![arity as u8];
                for (i, value) in values.iter().enumerate() {
                    if i != omitted {
                        expected.extend_from_slice(&value.to_le_bytes());
                    }
                }
                assert_eq!(expected.len(), FriValues::encoded_bytes(arity));
                assert_eq!(fiber.arity(), arity);
                assert_eq!(fiber.len(), arity - 1);
                assert_eq!(
                    &fiber.expand(omitted, values[omitted]).unwrap()[..arity],
                    values
                );
                for flags in
                    (u8::MIN..=u8::MAX).filter(|&f| norito::core::validate_header_flags(f).is_ok())
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
    }
    #[test]
    fn fixed_fibers_reject_wrong_arity_length_full_fiber_and_noncanonical_limbs() {
        for arity in [0, 1, 2, 3, 5, 7, 9, 15, 17] {
            assert!(FriValues::omit(&values(arity), 0).is_err());
        }
        for arity in [4, 8, 16] {
            assert!(FriValues::omit(&values(arity), arity).is_err());
            let fiber = FriValues::omit(&values(arity), 0).unwrap();
            assert!(fiber.expand(arity, Fp4::ZERO).is_err());
            let raw = norito::codec::encode_with_header_flags(&fiber).0;
            for tag in [0, 1, 2, 3, 5, 8 + (arity as u8), 32] {
                if tag != arity as u8 {
                    let mut changed = raw.clone();
                    changed[0] = tag;
                    assert!(norito::core::decode_field_canonical::<FriValues>(&changed).is_err());
                }
            }
            for len in [0, 1, raw.len() - 1, raw.len() + 1, raw.len() + 32] {
                let mut changed = raw.clone();
                changed.resize(len, 0);
                assert!(norito::core::decode_field_canonical::<FriValues>(&changed).is_err());
            }
            for position in [0, arity - 2] {
                for limb in [0, 3] {
                    let mut changed = raw.clone();
                    let start = 1 + position * Fp4::BYTES + limb * 8;
                    changed[start..start + 8].copy_from_slice(&GOLDILOCKS_MODULUS.to_le_bytes());
                    assert!(norito::core::decode_field_canonical::<FriValues>(&changed).is_err());
                }
            }
            let mut full = vec![arity as u8];
            for value in values(arity) {
                full.extend_from_slice(&value.to_le_bytes());
            }
            assert!(norito::core::decode_field_canonical::<FriValues>(&full).is_err());
            assert!(
                norito::core::decode_field_canonical::<FriValues>(
                    &norito::codec::encode_with_header_flags(&values(arity)).0
                )
                .is_err()
            );
        }
    }
}
