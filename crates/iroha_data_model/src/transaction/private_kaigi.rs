//! Authority-free private Kaigi transaction forms.

use std::{num::NonZeroU32, time::Duration, vec::Vec};

use derive_more::Display;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use iroha_version::Version;
use norito::codec::{Decode, DecodeAll, Encode};

use crate::{
    AssetDefinitionId, ChainId,
    kaigi::{
        KaigiId, KaigiParticipantCommitment, KaigiParticipantNullifier, KaigiPrivacyMode,
        KaigiRelayManifest, KaigiRoomPolicy,
    },
    metadata::Metadata,
    transaction::signed::TransactionEntrypoint,
};

#[model]
mod model {
    use super::*;

    /// Opaque template describing the call created by a private Kaigi host.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct PrivateKaigiTemplate {
        /// Identifier of the call.
        pub id: KaigiId,
        /// Optional human readable title.
        pub title: Option<String>,
        /// Optional description of the session.
        pub description: Option<String>,
        /// Maximum number of concurrent participants (excluding the host).
        pub max_participants: Option<u32>,
        /// Gas rate charged per minute of call time.
        pub gas_rate_per_minute: u64,
        /// Optional per-call metadata for additional signalling.
        pub metadata: Metadata,
        /// Optional scheduled start timestamp (milliseconds since epoch).
        pub scheduled_start_ms: Option<u64>,
        /// Privacy configuration requested by the host.
        pub privacy_mode: KaigiPrivacyMode,
        /// Viewer authentication policy enforced by the relays.
        #[norito(default)]
        pub room_policy: KaigiRoomPolicy,
        /// Optional relay manifest snapshot carrying structured relay metadata.
        pub relay_manifest: Option<KaigiRelayManifest>,
    }

    /// Create payload for a private Kaigi transaction.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct PrivateCreateKaigi {
        /// Opaque template describing the new call.
        pub call: PrivateKaigiTemplate,
    }

    /// Join payload for a private Kaigi transaction.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct PrivateJoinKaigi {
        /// Identifier of the call to join.
        pub call_id: KaigiId,
    }

    /// End payload for a private Kaigi transaction.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct PrivateEndKaigi {
        /// Identifier of the call to end.
        pub call_id: KaigiId,
        /// Optional timestamp in milliseconds when the call ended.
        pub ended_at_ms: Option<u64>,
    }

    /// Privacy artifacts authorizing a private Kaigi action.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct PrivateKaigiArtifacts {
        /// Commitment describing the acting participant.
        pub commitment: KaigiParticipantCommitment,
        /// Nullifier preventing replay.
        pub nullifier: KaigiParticipantNullifier,
        /// Merkle root the participant used when generating the proof.
        pub roster_root: Hash,
        /// Proof bytes attesting ownership of the commitment.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub proof: Vec<u8>,
    }

    /// Confidential XOR fee spend envelope bound to a private Kaigi action.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct PrivateKaigiFeeSpend {
        /// Shielded asset pool being spent.
        pub asset_definition_id: AssetDefinitionId,
        /// Anchor root committed to by the spend proof.
        pub anchor_root: Hash,
        /// Spent nullifiers.
        pub nullifiers: Vec<[u8; 32]>,
        /// Output commitments created by the spend.
        pub output_commitments: Vec<[u8; 32]>,
        /// Encrypted payloads for any change notes.
        pub encrypted_change_payloads: Vec<Vec<u8>>,
        /// Proof bytes binding the spend to the action hash and chain.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub proof: Vec<u8>,
    }

    /// Authority-free private Kaigi action carried by a dedicated entrypoint.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[cfg_attr(feature = "json", norito(tag = "kind", content = "state"))]
    pub enum PrivateKaigiAction {
        /// Create a new private call.
        Create(PrivateCreateKaigi),
        /// Join an existing private call.
        Join(PrivateJoinKaigi),
        /// End an existing private call.
        End(PrivateEndKaigi),
    }

    /// Dedicated authority-free private Kaigi transaction.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Display, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[display("{}", self.hash())]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct PrivateKaigiTransaction {
        /// Unique id of the blockchain. Used for replay protection.
        pub chain: ChainId,
        /// Creation timestamp (unix time in milliseconds).
        pub creation_time_ms: u64,
        /// Random value to make different hashes for repeated actions.
        pub nonce: Option<NonZeroU32>,
        /// Store for additional opaque signalling metadata.
        pub metadata: Metadata,
        /// Requested private Kaigi action.
        pub action: PrivateKaigiAction,
        /// Proof-backed Kaigi privacy artifacts.
        pub artifacts: PrivateKaigiArtifacts,
        /// Confidential XOR fee spend envelope funding the action.
        pub fee_spend: PrivateKaigiFeeSpend,
    }
}

pub use self::model::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
struct PrivateKaigiActionHashPayload {
    chain: ChainId,
    creation_time_ms: u64,
    nonce: Option<NonZeroU32>,
    metadata: Metadata,
    action: PrivateKaigiAction,
    artifacts: PrivateKaigiArtifacts,
}

impl PrivateKaigiTransaction {
    /// Creation timestamp as [`core::time::Duration`].
    #[inline]
    pub fn creation_time(&self) -> Duration {
        Duration::from_millis(self.creation_time_ms)
    }

    /// Canonical action hash excluding the fee-spend envelope.
    #[inline]
    pub fn action_hash(&self) -> Hash {
        let payload = PrivateKaigiActionHashPayload {
            chain: self.chain.clone(),
            creation_time_ms: self.creation_time_ms,
            nonce: self.nonce,
            metadata: self.metadata.clone(),
            action: self.action.clone(),
            artifacts: self.artifacts.clone(),
        };
        Hash::new(norito::codec::encode_adaptive(&payload))
    }

    /// Canonical hash for this external private Kaigi transaction.
    #[inline]
    pub fn hash(&self) -> HashOf<Self> {
        let entry_hash = self.hash_as_entrypoint();
        HashOf::from_untyped_unchecked(Hash::from(entry_hash))
    }

    /// Hash for this transaction as a [`TransactionEntrypoint`].
    #[inline]
    pub fn hash_as_entrypoint(&self) -> HashOf<TransactionEntrypoint> {
        HashOf::new(&TransactionEntrypoint::PrivateKaigi(self.clone()))
    }
}

impl Version for PrivateKaigiTransaction {
    fn version(&self) -> u8 {
        1
    }

    fn supported_versions() -> core::ops::Range<u8> {
        1..2
    }
}

impl iroha_version::codec::EncodeVersioned for PrivateKaigiTransaction {
    fn encode_versioned(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(1);
        bytes.push(self.version());
        bytes.extend(norito::codec::encode_adaptive(self));
        bytes
    }
}

impl iroha_version::codec::DecodeVersioned for PrivateKaigiTransaction {
    fn decode_all_versioned(input: &[u8]) -> iroha_version::error::Result<Self> {
        use iroha_version::error::Error;

        let Some((&version, payload)) = input.split_first() else {
            return Err(Error::NotVersioned);
        };

        if !Self::supported_versions().contains(&version) {
            return Err(Error::UnsupportedVersion(Box::new(
                iroha_version::UnsupportedVersion::new(
                    version,
                    iroha_version::RawVersioned::NoritoBytes(input.to_vec()),
                ),
            )));
        }

        let payload_guard = norito::core::PayloadCtxGuard::enter(payload);
        let mut cursor = payload;
        let decoded = <Self as DecodeAll>::decode_all(&mut cursor).map_err(Error::from)?;
        drop(payload_guard);
        if cursor.is_empty() {
            Ok(decoded)
        } else {
            Err(Error::NoritoCodec(
                "PrivateKaigiTransaction payload contains trailing bytes".into(),
            ))
        }
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for PrivateKaigiTransaction {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let _guard = norito::core::PayloadCtxGuard::enter(bytes);
        let mut cursor = std::io::Cursor::new(bytes);
        let decoded: PrivateKaigiTransaction = norito::codec::Decode::decode(&mut cursor)?;
        let used =
            usize::try_from(cursor.position()).map_err(|_| norito::core::Error::LengthMismatch)?;
        Ok((decoded, used))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use iroha_crypto::KeyPair;

    use super::*;
    use crate::{domain::DomainId, kaigi::KaigiRelayHop, name::Name};

    fn sample_template() -> PrivateKaigiTemplate {
        PrivateKaigiTemplate {
            id: KaigiId::new(
                DomainId::from_str("kaigi").expect("domain"),
                Name::from_str("private-room").expect("call name"),
            ),
            title: Some("Private room".to_owned()),
            description: Some("No public authority".to_owned()),
            max_participants: Some(4),
            gas_rate_per_minute: 25,
            metadata: Metadata::default(),
            scheduled_start_ms: Some(1_700_000_000_000),
            privacy_mode: KaigiPrivacyMode::ZkRosterV1,
            room_policy: KaigiRoomPolicy::Authenticated,
            relay_manifest: Some(KaigiRelayManifest {
                hops: vec![KaigiRelayHop {
                    relay_id: crate::account::AccountId::new(
                        KeyPair::random().public_key().clone(),
                    ),
                    hpke_public_key: vec![1, 2, 3],
                    weight: 1,
                }],
                expiry_ms: 1_700_000_100_000,
            }),
        }
    }

    fn sample_transaction() -> PrivateKaigiTransaction {
        PrivateKaigiTransaction {
            chain: "test-chain".parse().expect("chain"),
            creation_time_ms: 42,
            nonce: Some(NonZeroU32::new(7).expect("nonce")),
            metadata: Metadata::default(),
            action: PrivateKaigiAction::Create(PrivateCreateKaigi {
                call: sample_template(),
            }),
            artifacts: PrivateKaigiArtifacts {
                commitment: KaigiParticipantCommitment {
                    commitment: Hash::new(b"host-commitment"),
                    alias_tag: Some("host".to_owned()),
                },
                nullifier: KaigiParticipantNullifier {
                    digest: Hash::new(b"private-kaigi-nullifier"),
                    issued_at_ms: 42,
                },
                roster_root: Hash::new(b"roster-root"),
                proof: vec![0xAA, 0xBB, 0xCC],
            },
            fee_spend: PrivateKaigiFeeSpend {
                asset_definition_id: AssetDefinitionId::new(
                    DomainId::from_str("wonderland").expect("domain"),
                    Name::from_str("xor").expect("name"),
                ),
                anchor_root: Hash::new(b"anchor-root"),
                nullifiers: vec![[0x11; 32]],
                output_commitments: vec![[0x22; 32]],
                encrypted_change_payloads: vec![vec![0x33, 0x44]],
                proof: vec![0x55, 0x66],
            },
        }
    }

    #[test]
    fn private_kaigi_transaction_roundtrip() {
        let tx = sample_transaction();
        let bytes = norito::codec::encode_adaptive(&tx);
        let decoded: PrivateKaigiTransaction =
            norito::codec::decode_adaptive(&bytes).expect("decode private tx");
        assert_eq!(decoded, tx);
    }

    #[test]
    fn private_kaigi_action_hash_excludes_fee_envelope() {
        let mut first = sample_transaction();
        let mut second = first.clone();
        second.fee_spend.proof = vec![0x99, 0x88];
        second.fee_spend.output_commitments = vec![[0x44; 32]];

        assert_eq!(first.action_hash(), second.action_hash());
        assert_ne!(first.hash(), second.hash());

        first.metadata.insert(
            Name::from_str("answer").expect("metadata key"),
            "ciphertext",
        );
        assert_ne!(first.action_hash(), second.action_hash());
    }

    #[test]
    fn private_kaigi_hash_matches_entrypoint_hash() {
        let tx = sample_transaction();
        let entry = TransactionEntrypoint::PrivateKaigi(tx.clone());

        assert_eq!(HashOf::new(&entry), entry.hash());
        assert_eq!(tx.hash_as_entrypoint(), entry.hash());
        assert_eq!(Hash::from(tx.hash()), Hash::from(tx.hash_as_entrypoint()));
    }
}
