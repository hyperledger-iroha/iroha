//! Canonical state key encoding and advisory read/write access sets.
//!
//! This module defines a portable, Norito-encoded representation of keys in
//! the World State View (WSV) alongside an advisory read/write access set
//! format suitable for inclusion in on-wire envelopes. These types are used to
//! drive conflict-aware scheduling and can be round-tripped losslessly.
//!
//! Notes
//! - Canonical bytes are the Norito encoding of the `CanonicalStateKey` value.
//!   This provides an unambiguous, stable ordering across nodes.
//! - The advisory access set is optional and never authoritative; executors
//!   must enforce actual access at runtime. Supersets are safe; subsets may be
//!   rejected or routed to a quarantine lane by policy.

use std::{string::String, vec::Vec};

#[cfg(feature = "json")]
use base64::engine::general_purpose::STANDARD;
use derive_more::Constructor;
use iroha_crypto::HashOf;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize};

use crate::{
    account::AccountId,
    asset::id::{AssetDefinitionId, AssetId},
    domain::DomainId,
    name::Name,
    nft::NftId,
    role::RoleId,
    rwa::RwaId,
    transaction::signed::SignedTransaction,
    trigger::TriggerId,
};

#[cfg(feature = "json")]
macro_rules! impl_state_json_via_norito_bytes {
    ($($ty:path),+ $(,)?) => {
        $(
            impl JsonSerialize for $ty {
                fn json_serialize(&self, out: &mut String) {
                    let bytes = norito::to_bytes(self)
                        .expect("Norito serialization must succeed");
                    let encoded = base64::Engine::encode(
                        &STANDARD,
                        bytes,
                    );
                    JsonSerialize::json_serialize(&encoded, out);
                }
            }

            impl JsonDeserialize for $ty {
                fn json_deserialize(
                    parser: &mut json::Parser<'_>,
                ) -> Result<Self, json::Error> {
                    let encoded = parser.parse_string()?;
                    let bytes = base64::Engine::decode(
                        &STANDARD,
                        encoded.as_str(),
                    )
                        .map_err(|err| json::Error::Message(err.to_string()))?;
                    let archived = norito::from_bytes::<$ty>(&bytes)
                        .map_err(|err| json::Error::Message(err.to_string()))?;
                    norito::core::NoritoDeserialize::try_deserialize(archived)
                        .map_err(|err| json::Error::Message(err.to_string()))
                }
            }
        )+
    };
}

/// Metadata entry key for a Domain.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
pub struct DomainMetadataKey {
    /// Domain identifier this metadata entry belongs to.
    pub id: DomainId,
    /// Metadata entry name scoped within the domain.
    pub key: Name,
}

/// Metadata entry key for an Account.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
pub struct AccountMetadataKey {
    /// Account identifier owning the metadata entry.
    pub id: AccountId,
    /// Metadata entry name scoped within the account.
    pub key: Name,
}

/// Metadata entry key for an `AssetDefinition`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
pub struct AssetDefinitionMetadataKey {
    /// Asset definition identifier associated with the metadata entry.
    pub id: AssetDefinitionId,
    /// Metadata entry name scoped within the asset definition.
    pub key: Name,
}

/// Metadata entry key for an Asset.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
pub struct AssetMetadataKey {
    /// Asset identifier (definition + account) associated with this metadata entry.
    pub id: AssetId,
    /// Metadata entry name scoped within the asset.
    pub key: Name,
}

/// Metadata entry key for an NFT.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
pub struct NftMetadataKey {
    /// NFT identifier owning the metadata entry.
    pub id: NftId,
    /// Metadata entry name scoped within the NFT.
    pub key: Name,
}

/// Metadata entry key for an RWA.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
pub struct RwaMetadataKey {
    /// RWA identifier owning the metadata entry.
    pub id: RwaId,
    /// Metadata entry name scoped within the RWA.
    pub key: Name,
}

/// Metadata entry key for a Trigger.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
pub struct TriggerMetadataKey {
    /// Trigger identifier associated with the metadata entry.
    pub id: TriggerId,
    /// Metadata entry name scoped within the trigger.
    pub key: Name,
}

/// Role membership binding for an account.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
pub struct AccountRoleKey {
    /// Account identifier that holds the role.
    pub account: AccountId,
    /// Role identifier granted to the account.
    pub role: RoleId,
}

/// Pending or queued transaction identified by its hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
pub struct TxQueueKey {
    /// Hash of the queued transaction.
    pub hash: HashOf<SignedTransaction>,
}

/// Canonical key for addressing items in the World State View.
///
/// The Norito encoding of this enum value is the canonical byte sequence used
/// for ordering, hashing and conflict detection. Variants are intentionally
/// explicit to avoid collisions across namespaces.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
pub enum CanonicalStateKey {
    /// Domain entity by id.
    Domain(DomainId),
    /// Account entity by id.
    Account(AccountId),
    /// Asset by concrete id (definition + account).
    Asset(AssetId),
    /// Asset definition by id.
    AssetDefinition(AssetDefinitionId),
    /// Non-fungible token by id.
    Nft(NftId),
    /// Real-world asset lot by id.
    Rwa(RwaId),
    /// Trigger by id.
    Trigger(TriggerId),
    /// Role definition by id.
    Role(RoleId),
    /// Permission set entry for an account.
    AccountPermissions(AccountId),
    /// Account-role binding.
    AccountRole(AccountRoleKey),
    /// Queued transaction entry.
    TxQueue(TxQueueKey),
    /// Domain metadata entry `(domain, key)`.
    DomainMetadata(DomainMetadataKey),
    /// Account metadata entry `(account, key)`.
    AccountMetadata(AccountMetadataKey),
    /// Asset definition metadata entry `(asset_definition, key)`.
    AssetDefinitionMetadata(AssetDefinitionMetadataKey),
    /// Asset metadata entry `(asset_id, key)`.
    AssetMetadata(AssetMetadataKey),
    /// NFT metadata entry `(nft_id, key)`.
    NftMetadata(NftMetadataKey),
    /// RWA metadata entry `(rwa_id, key)`.
    RwaMetadata(RwaMetadataKey),
    /// Trigger metadata entry `(trigger_id, key)`.
    TriggerMetadata(TriggerMetadataKey),
}

impl CanonicalStateKey {
    /// Produce canonical bytes for this key using Norito encoding.
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        Encode::encode(self)
    }
}

/// Advisory read/write access set for a transaction or sub-transaction.
///
/// - `reads`: keys that may be read.
/// - `writes`: keys that may be written (implies read).
///
/// The set is advisory: executors MUST enforce actual accesses at runtime and
/// reject or quarantine transactions that violate their declared sets per node
/// policy. Producers SHOULD sort and deduplicate keys for compactness.
#[derive(Debug, Clone, PartialEq, Eq, Default, Encode, Decode, IntoSchema, Constructor)]
pub struct StateAccessSetAdvisory {
    /// Canonical keys that may be read during execution.
    pub reads: Vec<CanonicalStateKey>,
    /// Canonical keys that may be written (and therefore read) during execution.
    pub writes: Vec<CanonicalStateKey>,
}

#[cfg(feature = "json")]
impl_state_json_via_norito_bytes!(
    DomainMetadataKey,
    AccountMetadataKey,
    AssetDefinitionMetadataKey,
    AssetMetadataKey,
    NftMetadataKey,
    RwaMetadataKey,
    TriggerMetadataKey,
    AccountRoleKey,
    TxQueueKey,
    CanonicalStateKey,
    StateAccessSetAdvisory,
);

impl StateAccessSetAdvisory {
    /// Sort keys by canonical bytes and deduplicate within reads and writes.
    ///
    /// Returns the number of elements removed.
    pub fn canonicalize(&mut self) -> usize {
        fn sort_dedup(vec: &mut Vec<CanonicalStateKey>) -> usize {
            vec.sort_by_key(CanonicalStateKey::to_canonical_bytes);
            let before = vec.len();
            vec.dedup_by(|a, b| a.to_canonical_bytes() == b.to_canonical_bytes());
            before - vec.len()
        }
        sort_dedup(&mut self.reads) + sort_dedup(&mut self.writes)
    }

    /// Merge another advisory set into `self` and canonicalize the result.
    pub fn extend_and_canonicalize(&mut self, other: StateAccessSetAdvisory) {
        self.reads.extend(other.reads);
        self.writes.extend(other.writes);
        self.canonicalize();
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::Hash;

    use super::*;
    use crate::{prelude::*, role::RoleId};

    #[test]
    fn key_roundtrip_and_ordering() {
        let domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let alice = AccountId::new(KeyPair::random().public_key().clone());
        let asset_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_id = AssetId::new(asset_def.clone(), alice.clone());
        let nft_id: NftId = "nft0$wonderland.universal".parse().unwrap();
        let rwa_id: RwaId = format!(
            "{}$wonderland.universal",
            Hash::prehashed([3; Hash::LENGTH])
        )
        .parse()
        .unwrap();
        let trig_id: TriggerId = "trigger0".parse().unwrap();
        let key: Name = "color".parse().unwrap();
        let role_id: RoleId = "auditor".parse().unwrap();
        let queue_hash =
            HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed([2; Hash::LENGTH]));

        let mut keys = vec![
            CanonicalStateKey::Account(alice.clone()),
            CanonicalStateKey::Domain(domain.clone()),
            CanonicalStateKey::Asset(asset_id.clone()),
            CanonicalStateKey::AssetDefinition(asset_def.clone()),
            CanonicalStateKey::Nft(nft_id.clone()),
            CanonicalStateKey::Rwa(rwa_id.clone()),
            CanonicalStateKey::Trigger(trig_id.clone()),
            CanonicalStateKey::Role(role_id.clone()),
            CanonicalStateKey::AccountPermissions(alice.clone()),
            CanonicalStateKey::AccountRole(AccountRoleKey {
                account: alice.clone(),
                role: role_id.clone(),
            }),
            CanonicalStateKey::TxQueue(TxQueueKey { hash: queue_hash }),
            CanonicalStateKey::DomainMetadata(DomainMetadataKey {
                id: domain.clone(),
                key: key.clone(),
            }),
            CanonicalStateKey::AccountMetadata(AccountMetadataKey {
                id: alice.clone(),
                key: key.clone(),
            }),
            CanonicalStateKey::AssetDefinitionMetadata(AssetDefinitionMetadataKey {
                id: asset_def.clone(),
                key: key.clone(),
            }),
            CanonicalStateKey::AssetMetadata(AssetMetadataKey {
                id: asset_id.clone(),
                key: key.clone(),
            }),
            CanonicalStateKey::NftMetadata(NftMetadataKey {
                id: nft_id.clone(),
                key: key.clone(),
            }),
            CanonicalStateKey::RwaMetadata(RwaMetadataKey {
                id: rwa_id,
                key: key.clone(),
            }),
            CanonicalStateKey::TriggerMetadata(TriggerMetadataKey {
                id: trig_id.clone(),
                key: key.clone(),
            }),
        ];

        // Ensure bare Norito codec roundtrips remain aligned with canonical bytes.
        for original in &keys {
            let bytes = Encode::encode(original);
            let mut cursor: &[u8] = &bytes;
            let decoded = CanonicalStateKey::decode(&mut cursor)
                .expect("bare Norito decode should succeed for canonical keys");
            assert!(
                cursor.is_empty(),
                "bare decode must consume the full payload for {original:?}"
            );
            assert_eq!(
                *original, decoded,
                "bare Norito roundtrip must preserve the canonical key"
            );
        }

        // Sorting by canonical bytes is stable and total
        keys.sort_by_key(super::CanonicalStateKey::to_canonical_bytes);
        for pair in keys.windows(2) {
            let a = &pair[0];
            let b = &pair[1];
            assert!(a.to_canonical_bytes() <= b.to_canonical_bytes());
        }
    }

    #[test]
    fn advisory_set_canonicalization_dedups() {
        let domain: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let key: Name = "k".parse().unwrap();
        let a = CanonicalStateKey::Domain(domain.clone());
        let b = CanonicalStateKey::DomainMetadata(DomainMetadataKey { id: domain, key });

        let mut set =
            StateAccessSetAdvisory::new(vec![a.clone(), a.clone()], vec![b.clone(), b.clone()]);
        let removed = set.canonicalize();
        assert!(removed >= 2, "expected at least 2 duplicates removed");
        assert_eq!(set.reads.len(), 1);
        assert_eq!(set.writes.len(), 1);
    }
}
