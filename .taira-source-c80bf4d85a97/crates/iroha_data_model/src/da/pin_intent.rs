use core::cmp::Ordering;

use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    to_bytes,
};

#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    account::AccountId,
    da::{commitment::DaCommitmentLocation, types::StorageTicketId},
    nexus::LaneId,
    sorafs::pin_registry::ManifestDigest,
};

/// Pin intent emitted by the DA ingest pipeline to seed the `SoraFS` registry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaPinIntent {
    /// Lane associated with the blob.
    pub lane_id: LaneId,
    /// Epoch the blob belongs to.
    pub epoch: u64,
    /// Monotonic sequence within the lane/epoch scope.
    pub sequence: u64,
    /// Storage ticket issued for this blob.
    pub storage_ticket: StorageTicketId,
    /// Canonical manifest digest (BLAKE3 over encoded `DaManifestV1`).
    pub manifest_hash: ManifestDigest,
    /// Optional alias to register in the `SoraFS` registry.
    #[norito(default)]
    pub alias: Option<String>,
    /// Optional owner account for governance/registry tracking.
    #[norito(default)]
    pub owner: Option<AccountId>,
}

impl DaPinIntent {
    /// Construct a pin intent without optional alias/owner fields.
    #[must_use]
    pub fn new(
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        storage_ticket: StorageTicketId,
        manifest_hash: ManifestDigest,
    ) -> Self {
        Self {
            lane_id,
            epoch,
            sequence,
            storage_ticket,
            manifest_hash,
            alias: None,
            owner: None,
        }
    }
}

/// Bundle of pin intents embedded into a block payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaPinIntentBundle {
    /// Bundle layout version.
    pub version: u16,
    /// Ordered pin intent entries.
    pub intents: Vec<DaPinIntent>,
}

impl DaPinIntentBundle {
    /// Initial version identifier for on-chain pin intent bundles.
    pub const VERSION_V1: u16 = 1;

    /// Construct a bundle using the latest supported version.
    #[must_use]
    pub fn new(intents: Vec<DaPinIntent>) -> Self {
        let mut intents = intents;
        intents.sort_by(|a, b| {
            (
                a.lane_id.as_u32(),
                a.epoch,
                a.sequence,
                *a.storage_ticket.as_bytes(),
                *a.manifest_hash.as_bytes(),
                a.alias.as_deref(),
                a.owner.as_ref(),
            )
                .cmp(&(
                    b.lane_id.as_u32(),
                    b.epoch,
                    b.sequence,
                    *b.storage_ticket.as_bytes(),
                    *b.manifest_hash.as_bytes(),
                    b.alias.as_deref(),
                    b.owner.as_ref(),
                ))
        });
        Self {
            version: Self::VERSION_V1,
            intents,
        }
    }

    /// Returns `true` if there are no pin intents in the bundle.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.intents.is_empty()
    }

    /// Canonical Merkle root over the bundled pin intents.
    ///
    /// Leaves are `hash(norito::to_bytes(DaPinIntent))`. Odd leaves are
    /// promoted unchanged to the next layer instead of being duplicated.
    #[must_use]
    pub fn merkle_root(&self) -> Option<Hash> {
        if self.intents.is_empty() {
            return None;
        }

        let mut layer: Vec<Hash> = self
            .intents
            .iter()
            .map(|intent| {
                let encoded = to_bytes(intent).expect("DaPinIntent Norito encoding must succeed");
                Hash::new(encoded)
            })
            .collect();

        while layer.len() > 1 {
            let mut next = Vec::with_capacity(layer.len().div_ceil(2));
            let mut iter = layer.chunks(2);
            for pair in iter.by_ref() {
                let combined = if pair.len() == 1 {
                    pair[0]
                } else {
                    let mut buf = Vec::with_capacity(Hash::LENGTH * 2);
                    buf.extend_from_slice(pair[0].as_ref());
                    buf.extend_from_slice(pair[1].as_ref());
                    Hash::new(buf)
                };
                next.push(combined);
            }
            layer = next;
        }

        layer.pop()
    }
}

impl Default for DaPinIntentBundle {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl PartialOrd for DaPinIntentBundle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DaPinIntentBundle {
    fn cmp(&self, other: &Self) -> Ordering {
        self.version
            .cmp(&other.version)
            .then_with(|| self.intents.cmp(&other.intents))
    }
}

/// Pin intent annotated with its position inside a block payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaPinIntentWithLocation {
    /// Pin intent contents.
    pub intent: DaPinIntent,
    /// Block height and index of the intent within the bundled payload.
    pub location: DaCommitmentLocation,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{da::types::StorageTicketId, nexus::LaneId, sorafs::pin_registry::ManifestDigest};

    #[test]
    fn bundle_sorts_intents_deterministically() {
        let mut first = DaPinIntent::new(
            LaneId::new(2),
            2,
            1,
            StorageTicketId::new([2; 32]),
            ManifestDigest::new([2; 32]),
        );
        first.alias = Some("z-alias".to_string());
        let mut second = DaPinIntent::new(
            LaneId::new(1),
            2,
            3,
            StorageTicketId::new([1; 32]),
            ManifestDigest::new([1; 32]),
        );
        second.alias = Some("a-alias".to_string());
        let third = DaPinIntent::new(
            LaneId::new(1),
            1,
            9,
            StorageTicketId::new([3; 32]),
            ManifestDigest::new([3; 32]),
        );

        let bundle = DaPinIntentBundle::new(vec![first.clone(), second.clone(), third.clone()]);
        let ordered: Vec<(u32, u64, u64, StorageTicketId)> = bundle
            .intents
            .iter()
            .map(|intent| {
                (
                    intent.lane_id.as_u32(),
                    intent.epoch,
                    intent.sequence,
                    intent.storage_ticket,
                )
            })
            .collect();

        assert_eq!(
            ordered,
            vec![
                (
                    third.lane_id.as_u32(),
                    third.epoch,
                    third.sequence,
                    third.storage_ticket
                ),
                (
                    second.lane_id.as_u32(),
                    second.epoch,
                    second.sequence,
                    second.storage_ticket
                ),
                (
                    first.lane_id.as_u32(),
                    first.epoch,
                    first.sequence,
                    first.storage_ticket
                )
            ]
        );
    }
}
