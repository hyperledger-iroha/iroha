//! Exact-width native row storage with an explicit canonical little-endian codec.
//!
//! The wire field contains no vector count or scalar length prefixes. Only the
//! native field array is retained; decoding uses one eight-byte scratch value.
//! The enclosing `Vec<SharedRow>` funds its complete inline allocation through
//! Norito's ordinary sequence limits. No private trace or domain is reconstructed.

use std::ops::Deref;

use super::{GOLDILOCKS_MODULUS, Result as ProverResult, canonical_base, shape};

/// Complete row of the sole fixed compact profile.
#[derive(Clone, Debug, PartialEq, Eq, norito::NoritoSchema)]
#[norito_schema(name = "fastpq_prover::compact_v1::FixedRowValuesV1")]
pub(super) struct RowValues([u64; 342]);

impl RowValues {
    pub(super) const WIDTH: usize = 342;
    pub(super) const BYTES: usize = Self::WIDTH * size_of::<u64>();

    pub(super) fn from_vec(values: Vec<u64>) -> ProverResult<Self> {
        let values: [u64; Self::WIDTH] = values
            .try_into()
            .map_err(|_| shape("shared complete row has another width"))?;
        for (column, &value) in values.iter().enumerate() {
            canonical_base(value, "shared_complete_row", &[column])?;
        }
        Ok(Self(values))
    }

    #[cfg(test)]
    pub(super) const fn zero() -> Self {
        Self([0; Self::WIDTH])
    }
}

impl Deref for RowValues {
    type Target = [u64];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Malformed-value tests may change a scalar before framing it. Production
// construction and decoding both reject noncanonical field elements.
#[cfg(test)]
impl std::ops::DerefMut for RowValues {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl norito::core::SerializePayload for RowValues {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        for value in &self.0 {
            writer.write_all(&value.to_le_bytes())?;
        }
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(Self::BYTES)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        Some(Self::BYTES)
    }
}

impl<'de> norito::core::DeserializePayload<'de> for RowValues {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("canonical fixed compact row decode")
    }

    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
        let ptr = std::ptr::from_ref(archived).cast::<u8>();
        let mut offset = 0;
        let mut values = [0; Self::WIDTH];
        for value in &mut values {
            let bytes = norito::core::decode_context_byte_array::<8>(ptr, &mut offset)?;
            *value = u64::from_le_bytes(bytes);
            if *value >= GOLDILOCKS_MODULUS {
                return Err(norito::Error::Message(
                    "noncanonical compact row field element".into(),
                ));
            }
        }
        norito::core::finish_context_fields(ptr, offset)?;
        Ok(Self(values))
    }
}

#[cfg(test)]
#[path = "row_values/tests.rs"]
mod tests;
