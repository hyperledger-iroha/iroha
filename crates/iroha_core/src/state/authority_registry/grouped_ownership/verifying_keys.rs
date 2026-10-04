//! Retain both original registry and inverse readers through canonical encoding.

use super::*;
use crate::state::verifying_key_index_validation::{self as relation, Image, Work};
use iroha_data_model::proof::{VerifyingKeyId, VerifyingKeyRecord};

const INDEX: &str = "world.verifying_keys_by_circuit";

/// Exact verifier registry with its complete circuit/version inverse.
pub(in super::super) struct CheckedVerifyingKeys<'world> {
    world: &'world World,
    rows: CommittedStorageView<'world, VerifyingKeyId, VerifyingKeyRecord>,
    index: CommittedStorageView<'world, (String, u32), VerifyingKeyId>,
}
impl<'world> CheckedVerifyingKeys<'world> {
    /// Admit both-image work before canonical capture without cloning any key or row.
    pub(in super::super) fn capture(
        world: &'world World,
        max_work: u64,
    ) -> Result<Self, GroupedOwnershipError> {
        let checked = Self::retain(world)?;
        let result =
            relation::validate(&checked.rows, &checked.index, &mut Work::bounded(max_work))
                .map_err(GroupedOwnershipError::from_verifying_key_relation);
        checked.finish_validation(result)
    }
    fn retain(world: &'world World) -> Result<Self, GroupedOwnershipError> {
        Ok(Self {
            world,
            rows: world.verifying_keys.try_committed_view_nonblocking()?,
            index: world
                .verifying_keys_by_circuit
                .try_committed_view_nonblocking()?,
        })
    }
    fn finish_validation(
        self,
        result: Result<(), GroupedOwnershipError>,
    ) -> Result<Self, GroupedOwnershipError> {
        if !self.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(self)
    }
    /// Borrow exactly the checked canonical reader for the catalog serializer.
    pub(in super::super) fn rows(
        &self,
    ) -> &CommittedStorageView<'world, VerifyingKeyId, VerifyingKeyRecord> {
        &self.rows
    }
    /// Observe both original identities even after validation or encoding refuses.
    pub(in super::super) fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        let rows = self.rows.try_matches_current(&self.world.verifying_keys);
        let index = self
            .index
            .try_matches_current(&self.world.verifying_keys_by_circuit);
        Ok(rows? && index?)
    }
}
impl GroupedOwnershipError {
    /// Preserve the same structural or local-work result for both original source forms.
    pub(in crate::state) fn from_verifying_key_relation(error: relation::Error) -> Self {
        match error {
            relation::Error::WorkLimit => GroupedOwnershipError::WorkLimit,
            relation::Error::Index { image, missing } => GroupedOwnershipError::Corrupt {
                index: INDEX,
                image: match image {
                    Image::Current => GroupImage::Current,
                    Image::Predecessor => GroupImage::Predecessor,
                },
                mismatch: if missing {
                    GroupMismatch::MissingMember
                } else {
                    GroupMismatch::ForeignMember
                },
            },
        }
    }
}
#[cfg(test)]
mod tests;
