//! Alias resolution data structures.
//!
//! This module currently provides data model primitives for alias records,
//! attestation tracking, and Merkle snapshot bookkeeping. The cryptographic
//! plumbing (VOPRF evaluation, STARK proofs, etc.) is intentionally left as a
//! follow-up; the data model intentionally leaves cryptographic wiring for a
//! subsequent integration tracked in the alias roadmap.

use std::vec::Vec;

use iroha_crypto::{Hash, HashOf, Signature};
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use norito::{Decode, codec::Encode};

pub use self::model::*;

#[model]
mod model {
    use super::*;
    use crate::{account::AccountId, asset::AssetId, peer::PeerId};

    /// Unique identifier assigned to an alias record in the Merkle store.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    pub struct AliasIndex(pub u64);

    /// Alias to entity mapping recorded on-chain.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    pub struct AliasRecord {
        /// Human-readable alias maintained by governance.
        pub alias: crate::name::Name,
        /// Owner responsible for the alias lifecycle.
        pub owner: AccountId,
        /// Entity targeted by the alias.
        pub target: AliasTarget,
        /// Stable Merkle index.
        pub index: AliasIndex,
        /// Hashes of attestations that ratify this alias.
        pub attestation_hashes: Vec<HashOf<AliasAttestation>>,
    }

    /// Types of entities an alias may resolve to.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    pub enum AliasTarget {
        /// Account level alias (e.g., DID-like mapping).
        Account(AccountId),
        /// Asset level alias (e.g., ticker).
        Asset(AssetId),
        /// Peer level alias (e.g., RPC endpoint mapping).
        Peer(PeerId),
        /// Arbitrary opaque payload (reserved for future integrations).
        Custom(Vec<u8>),
    }

    /// Attestation emitted by an authorised attester.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    pub struct AliasAttestation {
        /// Alias covered by the attestation.
        pub alias: crate::name::Name,
        /// Authority that produced the attestation.
        pub attester: AccountId,
        /// Signature over the attestation payload.
        pub signature: Signature,
        /// Optional context for the signature domain separation (future use).
        pub context: Vec<u8>,
    }

    /// Merkle bookkeeping snapshot for alias storage.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    pub struct AliasMerkleSnapshot {
        /// Root hash covering all alias records.
        pub root: HashOf<AliasRecord>,
        /// Leaf hashes required for incremental verification.
        pub frontier: Vec<HashOf<AliasRecord>>,
    }

    /// Audit/event stream entries produced by alias attesters.
    /// Payload describing an alias record alongside the attestation that produced it.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    pub struct AliasRecordedEvent {
        /// Alias record materialised in storage.
        pub record: AliasRecord,
        /// Attestation responsible for the update.
        pub attestation: AliasAttestation,
    }

    /// Payload describing alias revocation details.
    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    pub struct AliasRevokedEvent {
        /// Alias name.
        pub alias: crate::name::Name,
        /// Authority performing the revoke operation.
        pub attester: AccountId,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
    pub enum AliasEvent {
        /// Alias recorded or updated with attestation.
        Recorded(AliasRecordedEvent),
        /// Alias attestation revoked (attester action).
        Revoked(AliasRevokedEvent),
        /// Merkle frontier snapshot persisted for auditability.
        FrontierCheckpoint(AliasMerkleSnapshot),
    }
}

impl AliasRecord {
    /// Construct a new alias record.
    #[must_use]
    pub fn new(
        alias: crate::name::Name,
        owner: crate::account::AccountId,
        target: AliasTarget,
        index: AliasIndex,
    ) -> Self {
        Self {
            alias,
            owner,
            target,
            index,
            attestation_hashes: Vec::new(),
        }
    }

    /// Register an attestation hash if it is not already tracked.
    pub fn push_attestation(&mut self, hash: HashOf<AliasAttestation>) {
        if !self.attestation_hashes.contains(&hash) {
            self.attestation_hashes.push(hash);
        }
    }
}

impl AliasAttestation {
    /// Construct a new attestation payload.
    #[must_use]
    pub fn new(
        alias: crate::name::Name,
        attester: crate::account::AccountId,
        signature: Signature,
        context: Vec<u8>,
    ) -> Self {
        Self {
            alias,
            attester,
            signature,
            context,
        }
    }
}

/// Domain separation tag for alias frontier hashing.
pub const ALIAS_FRONTIER_HASH_DOMAIN: &[u8] = b"iroha:alias:frontier:v1|";

/// Deterministic digest for alias frontier checkpoints.
///
/// Combines the domain tag, chain id, block height, root hash, and a
/// lexicographically sorted frontier list to keep the digest stable across
/// hardware and ordering differences.
#[must_use]
pub fn alias_frontier_digest(
    chain_id: &crate::id::ChainId,
    height: u64,
    snapshot: &AliasMerkleSnapshot,
) -> Hash {
    let mut frontier = snapshot
        .frontier
        .iter()
        .map(|h| {
            let mut arr = [0u8; Hash::LENGTH];
            arr.copy_from_slice(h.as_ref());
            arr
        })
        .collect::<Vec<_>>();
    frontier.sort_unstable();

    let mut buf = Vec::with_capacity(
        ALIAS_FRONTIER_HASH_DOMAIN.len()
            + chain_id.as_str().len()
            + 1
            + std::mem::size_of::<u64>()
            + Hash::LENGTH
            + frontier.len() * Hash::LENGTH,
    );
    buf.extend_from_slice(ALIAS_FRONTIER_HASH_DOMAIN);
    buf.extend_from_slice(chain_id.as_str().as_bytes());
    buf.push(b'|');
    buf.extend_from_slice(&height.to_be_bytes());
    buf.extend_from_slice(snapshot.root.as_ref());
    for node in frontier {
        buf.extend_from_slice(&node);
    }
    Hash::new(buf)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use iroha_crypto::KeyPair;

    use super::*;
    use crate::{account::AccountId, domain::DomainId, name::Name};

    fn sample_account_id() -> AccountId {
        let key_pair = KeyPair::random();
        let domain = DomainId::from_str("wonderland").expect("valid domain");
        AccountId::new(key_pair.public_key().clone())
    }

    #[test]
    fn alias_record_attestation_tracking() {
        let alias = Name::from_str("demo").expect("valid alias");
        let owner = sample_account_id();
        let mut record = AliasRecord::new(
            alias.clone(),
            owner.clone(),
            AliasTarget::Account(owner.clone()),
            AliasIndex(0),
        );
        let attestation = AliasAttestation::new(
            alias.clone(),
            owner,
            {
                let sig_bytes = [0u8; 64];
                Signature::from_bytes(&sig_bytes)
            },
            Vec::new(),
        );
        let hash = HashOf::from_untyped_unchecked(iroha_crypto::Hash::new([0u8; 32]));
        record.push_attestation(hash);
        assert_eq!(record.attestation_hashes.len(), 1);
        // second insert should be ignored
        record.push_attestation(hash);
        assert_eq!(record.attestation_hashes.len(), 1);
        let event = AliasEvent::Recorded(AliasRecordedEvent {
            record: record.clone(),
            attestation,
        });
        match event {
            AliasEvent::Recorded(payload) => {
                assert_eq!(payload.record.index, AliasIndex(0));
            }
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn frontier_digest_is_order_independent_and_height_scoped() {
        let alias = Name::from_str("demo").expect("valid alias");
        let owner = sample_account_id();
        let record = AliasRecord::new(
            alias.clone(),
            owner.clone(),
            AliasTarget::Account(owner),
            AliasIndex(7),
        );
        let root = HashOf::<AliasRecord>::new(&record);
        let f1 = HashOf::<AliasRecord>::from_untyped_unchecked(Hash::new(b"frontier-1"));
        let f2 = HashOf::<AliasRecord>::from_untyped_unchecked(Hash::new(b"frontier-2"));
        let chain_id = crate::id::ChainId::from("demo-chain");

        let snap_ab = AliasMerkleSnapshot {
            root,
            frontier: vec![f1, f2],
        };
        let snap_ba = AliasMerkleSnapshot {
            root,
            frontier: vec![f2, f1],
        };

        let digest_ab = alias_frontier_digest(&chain_id, 42, &snap_ab);
        let digest_ba = alias_frontier_digest(&chain_id, 42, &snap_ba);
        assert_eq!(digest_ab, digest_ba);

        let digest_height_variation = alias_frontier_digest(&chain_id, 43, &snap_ab);
        assert_ne!(digest_ab, digest_height_variation);
    }
}
