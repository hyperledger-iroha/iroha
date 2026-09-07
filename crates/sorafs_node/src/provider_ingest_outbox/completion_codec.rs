//! Canonical codec delegation for pointer-sized provider-ingest completion storage.

use super::{BoxedStoredCompletionDeliveryV1, StoredCompletionDeliveryV1};

impl norito::core::NoritoSerialize for BoxedStoredCompletionDeliveryV1 {
    fn schema_hash() -> [u8; 16] {
        <StoredCompletionDeliveryV1 as norito::core::NoritoSerialize>::schema_hash()
    }
}

impl norito::core::SerializePayload for BoxedStoredCompletionDeliveryV1 {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        norito::core::SerializePayload::serialize(self.0.as_ref(), writer)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        norito::core::SerializePayload::encoded_len_hint(self.0.as_ref())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        norito::core::SerializePayload::encoded_len_exact(self.0.as_ref())
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for BoxedStoredCompletionDeliveryV1 {
    fn schema_hash() -> [u8; 16] {
        <StoredCompletionDeliveryV1 as norito::core::NoritoDeserialize<'a>>::schema_hash()
    }
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("boxed provider-ingest completion decode")
    }
    fn try_deserialize(
        archived: &'a norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let completion =
            <StoredCompletionDeliveryV1 as norito::core::NoritoDeserialize<'a>>::try_deserialize(
                archived.cast::<StoredCompletionDeliveryV1>(),
            )?;
        Ok(Self::new(completion))
    }
}
