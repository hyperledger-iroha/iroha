//! Fixed 301-column canonical scalar storage for the inactive DEEP frame.

use std::ops::Deref;

use super::{COMMITTED_COLUMN_COUNT, shape};
use crate::Result as ProverResult;
use crate::backend::{GOLDILOCKS_MODULUS, polynomial_field::PolynomialField};

/// Ordered retained base-field values; complete inline storage is allocation-charged.
#[derive(Clone, Debug, PartialEq, Eq, norito::NoritoSchema)]
#[norito_schema(name = "fastpq_prover::deep_compact::RetainedRowValuesV1")]
pub(in crate::backend) struct RowValues([u64; COMMITTED_COLUMN_COUNT]);

impl RowValues {
    /// Canonical raw payload width, with no count or per-scalar length prefixes.
    pub(in crate::backend) const BYTES: usize = COMMITTED_COLUMN_COUNT * size_of::<u64>();

    /// Construct only from the complete canonical 301-column base-field opening.
    pub(in crate::backend) fn new(values: Vec<u64>) -> ProverResult<Self> {
        let values = values
            .try_into()
            .map_err(|_| shape("DEEP row needs exactly 301 cells"))?;
        let row = Self(values);
        row.validate()?;
        Ok(row)
    }

    pub(super) fn validate(&self) -> ProverResult<()> {
        for (column, &value) in self.0.iter().enumerate() {
            value.validate("deep_retained_row", &[column])?;
        }
        Ok(())
    }
}

impl Deref for RowValues {
    type Target = [u64];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl norito::core::SerializePayload for RowValues {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        for value in self.0 {
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
        Self::try_deserialize(archived).expect("canonical retained DEEP row decode")
    }
    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
        let pointer = std::ptr::from_ref(archived).cast::<u8>();
        let mut offset = 0;
        let mut values = [0; COMMITTED_COLUMN_COUNT];
        for value in &mut values {
            *value = u64::from_le_bytes(norito::core::decode_context_byte_array::<8>(
                pointer,
                &mut offset,
            )?);
            if *value >= GOLDILOCKS_MODULUS {
                return Err(norito::Error::Message(
                    "noncanonical DEEP row scalar".into(),
                ));
            }
        }
        norito::core::finish_context_fields(pointer, offset)?;
        Ok(Self(values))
    }
}
