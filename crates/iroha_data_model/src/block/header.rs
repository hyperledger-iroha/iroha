use std::{fmt, num::NonZeroU64, time::Duration};

use iroha_crypto::{Hash, HashOf, MerkleTree, Signature, SignatureOf};
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use norito::{codec::Encode, core as ncore};

use crate::{
    confidential::{ConfidentialFeatureDigest, DEFAULT_CONFIDENTIAL_FEATURE_DIGEST},
    consensus::PreviousRosterEvidence,
    da::{
        commitment::{DaCommitmentBundle, DaProofPolicyBundle},
        pin_intent::DaPinIntentBundle,
    },
    transaction::signed::{TransactionEntrypoint, TransactionResult},
};

#[model]
mod model {
    use getset::{CopyGetters, Getters, Setters};
    use norito::codec::{Decode, Encode};

    use super::*;
    use crate::{confidential::ConfidentialFeatureDigest, da::commitment::DaProofPolicyBundle};

    /// Essential metadata for a block in the chain.
    #[derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        CopyGetters,
        Getters,
        Setters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct BlockHeader {
        /// Number of blocks in the chain including this block.
        #[getset(get_copy = "pub", set = "pub")]
        pub height: NonZeroU64,
        /// Hash of the previous block in the chain.
        #[getset(get_copy = "pub", set = "pub")]
        pub prev_block_hash: Option<HashOf<BlockHeader>>,
        /// Merkle root of this block's external transaction entrypoints.
        /// None if there are no external transactions.
        #[getset(get_copy = "pub")]
        pub merkle_root: Option<HashOf<MerkleTree<TransactionEntrypoint>>>,
        /// Merkle root of this block's transaction results (external transactions + time triggers).
        /// None if there are no entrypoints.
        #[getset(get_copy = "pub")]
        pub result_merkle_root: Option<HashOf<MerkleTree<TransactionResult>>>,
        /// Optional hash covering the DA proof policy bundle embedded in the block payload.
        #[getset(get_copy = "pub", set = "pub")]
        pub da_proof_policies_hash: Option<HashOf<DaProofPolicyBundle>>,
        /// Optional hash covering the DA commitment bundle embedded in this block.
        #[getset(get_copy = "pub", set = "pub")]
        pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
        /// Optional hash covering the DA pin intent bundle embedded in this block.
        #[getset(get_copy = "pub", set = "pub")]
        pub da_pin_intents_hash: Option<HashOf<DaPinIntentBundle>>,
        /// Optional hash covering previous-height roster evidence embedded in the block payload.
        #[getset(get_copy = "pub", set = "pub")]
        pub prev_roster_evidence_hash: Option<HashOf<PreviousRosterEvidence>>,
        /// Optional SCCP commitment root finalized in this block.
        #[getset(get_copy = "pub", set = "pub")]
        pub sccp_commitment_root: Option<[u8; 32]>,
        /// Creation timestamp as Unix time in milliseconds.
        #[getset(skip)]
        pub creation_time_ms: u64,
        /// Value of view change index. Used to resolve soft forks.
        #[getset(get_copy = "pub", set = "pub")]
        pub view_change_index: u64,
        /// Optional digest advertising the confidential feature set applied in this block.
        #[getset(get_copy = "pub", set = "pub")]
        pub confidential_features: Option<ConfidentialFeatureDigest>,
    }

    /// The validator index and its corresponding signature on the block header.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, CopyGetters, Getters, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct BlockSignature {
        /// Validator index in the network topology.
        #[getset(get_copy = "pub")]
        pub(super) index: u64,
        /// Validator signature on the block header.
        #[getset(get = "pub")]
        pub(super) signature: SignatureOf<BlockHeader>,
    }
}

pub use self::model::{BlockHeader, BlockSignature};

/// Internal wire helper with a stable Norito tuple layout used by codecs and tests.
pub mod wire {
    use norito::core as ncore;

    use super::*;

    /// Stable transport for `BlockHeader` mapping typed hashes to raw bytes.
    #[derive(Clone, Copy)]
    pub struct BlockHeaderWire(
        /// Height of the block being represented.
        pub NonZeroU64,
        /// Optional hash of the previous block.
        pub Option<[u8; 32]>,
        /// Optional Merkle root for the block's transactions.
        pub Option<[u8; 32]>,
        /// Optional Merkle root for transaction results.
        pub Option<[u8; 32]>,
        /// Optional hash of the DA proof policy bundle embedded in the block payload.
        pub Option<[u8; 32]>,
        /// Block creation timestamp in milliseconds since Unix epoch.
        pub u64,
        /// View change index recorded in the block header.
        pub u64,
        /// Optional hash of the DA commitment bundle embedded in the block payload.
        pub Option<[u8; 32]>,
        /// Optional hash of the DA pin intent bundle embedded in the block payload.
        pub Option<[u8; 32]>,
        /// Optional hash of previous-height roster evidence embedded in the block payload.
        pub Option<[u8; 32]>,
        /// Optional SCCP commitment root finalized in this block.
        pub Option<[u8; 32]>,
        /// Optional confidential feature digest committed in the header.
        pub Option<ConfidentialFeatureDigestWire>,
    );

    impl ncore::NoritoSerialize for BlockHeaderWire {
        fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), ncore::Error> {
            let tuple = (
                self.0, self.1, self.2, self.3, self.4, self.5, self.6, self.7, self.8, self.9,
                self.10, self.11,
            );
            <(
                NonZeroU64,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                u64,
                u64,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<ConfidentialFeatureDigestWire>,
            ) as ncore::NoritoSerialize>::serialize(&tuple, writer)
        }
        fn encoded_len_hint(&self) -> Option<usize> {
            let tuple = (
                self.0, self.1, self.2, self.3, self.4, self.5, self.6, self.7, self.8, self.9,
                self.10, self.11,
            );
            <(
                NonZeroU64,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                u64,
                u64,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<ConfidentialFeatureDigestWire>,
            ) as ncore::NoritoSerialize>::encoded_len_hint(&tuple)
        }
        fn encoded_len_exact(&self) -> Option<usize> {
            let tuple = (
                self.0, self.1, self.2, self.3, self.4, self.5, self.6, self.7, self.8, self.9,
                self.10, self.11,
            );
            <(
                NonZeroU64,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                u64,
                u64,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<ConfidentialFeatureDigestWire>,
            ) as ncore::NoritoSerialize>::encoded_len_exact(&tuple)
        }
    }
    impl<'de> ncore::NoritoDeserialize<'de> for BlockHeaderWire {
        #[allow(clippy::many_single_char_names, clippy::type_complexity)]
        fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
            let tuple: (
                NonZeroU64,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                u64,
                u64,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<ConfidentialFeatureDigestWire>,
            ) = <(
                NonZeroU64,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                u64,
                u64,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<[u8; 32]>,
                Option<ConfidentialFeatureDigestWire>,
            ) as ncore::NoritoDeserialize>::deserialize(archived.cast());
            let (h, p, m, r, proof_hash, t, v, d, pins, prev_roster, sccp_root, f) = tuple;
            Self(
                h,
                p,
                m,
                r,
                proof_hash,
                t,
                v,
                d,
                pins,
                prev_roster,
                sccp_root,
                f,
            )
        }
    }
    /// Wire mapping for `ConfidentialFeatureDigest`.
    #[derive(Clone, Copy)]
    pub struct ConfidentialFeatureDigestWire(
        pub Option<[u8; 32]>,
        pub Option<u32>,
        pub Option<u32>,
        pub Option<u32>,
    );

    impl ncore::NoritoSerialize for ConfidentialFeatureDigestWire {
        fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), ncore::Error> {
            <(
                Option<[u8; 32]>,
                Option<u32>,
                Option<u32>,
                Option<u32>,
            ) as ncore::NoritoSerialize>::serialize(&(self.0, self.1, self.2, self.3), writer)
        }

        fn encoded_len_hint(&self) -> Option<usize> {
            <(
                Option<[u8; 32]>,
                Option<u32>,
                Option<u32>,
                Option<u32>,
            ) as ncore::NoritoSerialize>::encoded_len_hint(&(self.0, self.1, self.2, self.3))
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            <(
                Option<[u8; 32]>,
                Option<u32>,
                Option<u32>,
                Option<u32>,
            ) as ncore::NoritoSerialize>::encoded_len_exact(&(self.0, self.1, self.2, self.3))
        }
    }

    impl<'de> ncore::NoritoDeserialize<'de> for ConfidentialFeatureDigestWire {
        fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
            let (vk, poseidon, pedersen, rules): (
                Option<[u8; 32]>,
                Option<u32>,
                Option<u32>,
                Option<u32>,
            ) = <(
                Option<[u8; 32]>,
                Option<u32>,
                Option<u32>,
                Option<u32>,
            ) as ncore::NoritoDeserialize>::deserialize(archived.cast());
            Self(vk, poseidon, pedersen, rules)
        }

        fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
            let (vk, poseidon, pedersen, rules) = <(
                Option<[u8; 32]>,
                Option<u32>,
                Option<u32>,
                Option<u32>,
            ) as ncore::NoritoDeserialize>::try_deserialize(
                archived.cast()
            )?;
            Ok(Self(vk, poseidon, pedersen, rules))
        }
    }

    impl<'a> ncore::DecodeFromSlice<'a> for ConfidentialFeatureDigestWire {
        fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
            ncore::decode_field_canonical(bytes)
        }
    }

    impl From<ConfidentialFeatureDigest> for ConfidentialFeatureDigestWire {
        fn from(digest: ConfidentialFeatureDigest) -> Self {
            Self(
                digest.vk_set_hash,
                digest.poseidon_params_id,
                digest.pedersen_params_id,
                digest.conf_rules_version,
            )
        }
    }

    impl From<ConfidentialFeatureDigestWire> for ConfidentialFeatureDigest {
        fn from(wire: ConfidentialFeatureDigestWire) -> Self {
            Self {
                vk_set_hash: wire.0,
                poseidon_params_id: wire.1,
                pedersen_params_id: wire.2,
                conf_rules_version: wire.3,
            }
        }
    }
    /// Stable wire representation for `BlockSignature` consisting of the
    /// validator index and raw signature bytes. This avoids relying on packed
    /// struct memcopies during Norito decoding and keeps the on-wire tuple
    /// layout explicit.
    #[derive(Clone)]
    pub struct BlockSignatureWire(
        /// Validator index in the topology ordering.
        pub u64,
        /// Validator signature bytes encoded as Norito payload.
        pub Vec<u8>,
    );

    impl ncore::NoritoSerialize for BlockSignatureWire {
        fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), ncore::Error> {
            <(u64, Vec<u8>) as ncore::NoritoSerialize>::serialize(&(self.0, self.1.clone()), writer)
        }

        fn encoded_len_hint(&self) -> Option<usize> {
            <(u64, Vec<u8>) as ncore::NoritoSerialize>::encoded_len_hint(&(self.0, self.1.clone()))
        }

        fn encoded_len_exact(&self) -> Option<usize> {
            <(u64, Vec<u8>) as ncore::NoritoSerialize>::encoded_len_exact(&(self.0, self.1.clone()))
        }
    }

    impl<'de> ncore::NoritoDeserialize<'de> for BlockSignatureWire {
        fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
            let (index, payload): (u64, Vec<u8>) =
                <(u64, Vec<u8>) as ncore::NoritoDeserialize>::deserialize(archived.cast());
            Self(index, payload)
        }

        fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
            let (index, payload) =
                <(u64, Vec<u8>) as ncore::NoritoDeserialize>::try_deserialize(archived.cast())?;
            Ok(Self(index, payload))
        }
    }
}

// Conversions between BlockHeader and its wire mapping for tests and explicit transports.
impl From<BlockHeader> for wire::BlockHeaderWire {
    fn from(b: BlockHeader) -> Self {
        fn opt_hash_to_bytes<T>(h: Option<HashOf<T>>) -> Option<[u8; 32]> {
            h.map(|hv| *hv.as_ref())
        }
        fn digest_to_wire(
            digest: Option<ConfidentialFeatureDigest>,
        ) -> Option<wire::ConfidentialFeatureDigestWire> {
            digest.map(Into::into)
        }
        wire::BlockHeaderWire(
            b.height,
            opt_hash_to_bytes(b.prev_block_hash),
            opt_hash_to_bytes(b.merkle_root),
            opt_hash_to_bytes(b.result_merkle_root),
            opt_hash_to_bytes(b.da_proof_policies_hash),
            b.creation_time_ms,
            b.view_change_index,
            opt_hash_to_bytes(b.da_commitments_hash),
            opt_hash_to_bytes(b.da_pin_intents_hash),
            opt_hash_to_bytes(b.prev_roster_evidence_hash),
            b.sccp_commitment_root,
            digest_to_wire(b.confidential_features),
        )
    }
}

impl From<wire::BlockHeaderWire> for BlockHeader {
    fn from(w: wire::BlockHeaderWire) -> Self {
        fn opt_hash_from_bytes<T>(b: Option<[u8; 32]>) -> Option<HashOf<T>> {
            b.map(|arr| HashOf::from_untyped_unchecked(Hash::new(arr)))
        }
        fn digest_from_wire(
            digest: Option<wire::ConfidentialFeatureDigestWire>,
        ) -> Option<ConfidentialFeatureDigest> {
            digest.map(Into::into)
        }
        let mut header = BlockHeader::new(
            w.0,
            opt_hash_from_bytes::<BlockHeader>(w.1),
            opt_hash_from_bytes::<MerkleTree<TransactionEntrypoint>>(w.2),
            opt_hash_from_bytes::<MerkleTree<TransactionResult>>(w.3),
            w.5,
            w.6,
        );
        header.set_da_proof_policies_hash(opt_hash_from_bytes::<DaProofPolicyBundle>(w.4));
        header.set_da_commitments_hash(opt_hash_from_bytes::<DaCommitmentBundle>(w.7));
        header.set_da_pin_intents_hash(opt_hash_from_bytes::<DaPinIntentBundle>(w.8));
        header.set_prev_roster_evidence_hash(opt_hash_from_bytes::<PreviousRosterEvidence>(w.9));
        header.set_sccp_commitment_root(w.10);
        header.set_confidential_features(digest_from_wire(w.11));
        header
    }
}

impl From<&BlockSignature> for wire::BlockSignatureWire {
    fn from(value: &BlockSignature) -> Self {
        wire::BlockSignatureWire(value.index, value.signature.payload().to_vec())
    }
}

impl From<BlockSignature> for wire::BlockSignatureWire {
    fn from(value: BlockSignature) -> Self {
        (&value).into()
    }
}

impl From<wire::BlockSignatureWire> for BlockSignature {
    fn from(value: wire::BlockSignatureWire) -> Self {
        let signature = Signature::from_bytes(&value.1);
        BlockSignature::new(value.0, SignatureOf::from_signature(signature))
    }
}

impl BlockHeader {
    /// Create a new [`BlockHeader`].
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        height: NonZeroU64,
        prev_block_hash: Option<HashOf<BlockHeader>>,
        merkle_root: Option<HashOf<MerkleTree<TransactionEntrypoint>>>,
        result_merkle_root: Option<HashOf<MerkleTree<TransactionResult>>>,
        creation_time_ms: u64,
        view_change_index: u64,
    ) -> Self {
        Self {
            height,
            prev_block_hash,
            merkle_root,
            result_merkle_root,
            da_proof_policies_hash: None,
            da_commitments_hash: None,
            da_pin_intents_hash: None,
            prev_roster_evidence_hash: None,
            sccp_commitment_root: None,
            creation_time_ms,
            view_change_index,
            confidential_features: Some(DEFAULT_CONFIDENTIAL_FEATURE_DIGEST),
        }
    }

    /// Checks if it's a header of a genesis block.
    #[inline]
    pub const fn is_genesis(&self) -> bool {
        self.height.get() == 1
    }

    /// Creation timestamp.
    pub const fn creation_time(&self) -> Duration {
        Duration::from_millis(self.creation_time_ms)
    }

    /// Returns the consensus-level hash of the block header,
    /// excluding the `result_merkle_root` field.
    #[inline]
    pub fn hash(&self) -> HashOf<BlockHeader> {
        self.hash_without_results()
    }

    /// Computes the header hash without including `result_merkle_root`.
    #[inline]
    fn hash_without_results(&self) -> HashOf<BlockHeader> {
        /// A view of `BlockHeader` used for consensus hashing, omitting the execution results.
        #[derive(Encode)]
        struct BlockHeaderForConsensus {
            height: NonZeroU64,
            prev_block_hash: Option<HashOf<BlockHeader>>,
            /// Merkle root over externally submitted transactions (time-trigger entrypoints live in [`BlockResult`]).
            merkle_root: Option<HashOf<MerkleTree<TransactionEntrypoint>>>,
            /// Optional DA proof policy bundle hash.
            da_proof_policies_hash: Option<HashOf<DaProofPolicyBundle>>,
            /// Optional DA commitment bundle hash.
            da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
            /// Optional DA pin intent bundle hash.
            da_pin_intents_hash: Option<HashOf<DaPinIntentBundle>>,
            /// Optional previous-roster evidence hash.
            prev_roster_evidence_hash: Option<HashOf<PreviousRosterEvidence>>,
            /// Optional SCCP commitment root.
            sccp_commitment_root: Option<[u8; 32]>,
            creation_time_ms: u64,
            view_change_index: u64,
            confidential_features: Option<ConfidentialFeatureDigest>,
        }

        impl From<&BlockHeader> for BlockHeaderForConsensus {
            fn from(value: &BlockHeader) -> Self {
                let BlockHeader {
                    height,
                    prev_block_hash,
                    merkle_root,
                    result_merkle_root: _,
                    da_proof_policies_hash,
                    da_commitments_hash,
                    da_pin_intents_hash,
                    prev_roster_evidence_hash,
                    sccp_commitment_root,
                    creation_time_ms,
                    view_change_index,
                    confidential_features,
                } = *value;

                Self {
                    height,
                    prev_block_hash,
                    merkle_root,
                    da_proof_policies_hash,
                    da_commitments_hash,
                    da_pin_intents_hash,
                    prev_roster_evidence_hash,
                    sccp_commitment_root,
                    creation_time_ms,
                    view_change_index,
                    confidential_features,
                }
            }
        }

        HashOf::from_untyped_unchecked(HashOf::new(&BlockHeaderForConsensus::from(self)).into())
    }
}

impl fmt::Display for BlockHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (№{})", self.hash(), self.height)
    }
}

impl BlockSignature {
    /// Create a new [`BlockSignature`].
    pub fn new(index: u64, signature: SignatureOf<BlockHeader>) -> Self {
        Self { index, signature }
    }
}

impl ncore::NoritoSerialize for BlockSignature {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), ncore::Error> {
        let wire_repr = wire::BlockSignatureWire::from(self);
        ncore::NoritoSerialize::serialize(&wire_repr, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        let wire_repr = wire::BlockSignatureWire::from(self);
        ncore::NoritoSerialize::encoded_len_hint(&wire_repr)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        let wire_repr = wire::BlockSignatureWire::from(self);
        ncore::NoritoSerialize::encoded_len_exact(&wire_repr)
    }
}

impl<'de> ncore::NoritoDeserialize<'de> for BlockSignature {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        let wire_repr = wire::BlockSignatureWire::deserialize(archived.cast());
        wire_repr.into()
    }

    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let wire_repr = wire::BlockSignatureWire::try_deserialize(archived.cast())?;
        Ok(wire_repr.into())
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, KeyPair, Signature};
    use nonzero_ext::nonzero;
    use norito::{
        codec::{DecodeAll as _, decode_adaptive, encode_with_header_flags},
        core::NoritoSerialize,
    };

    use super::*;

    struct SamplePayload {
        payload: Vec<u8>,
        flags: u8,
    }

    fn sample_block_signature_set() -> std::collections::BTreeSet<BlockSignature> {
        let mut set = std::collections::BTreeSet::new();
        for (idx, fill) in [0x11_u8, 0x7f_u8, 0xe3_u8].into_iter().enumerate() {
            let payload = [fill; 64];
            let signature = Signature::from_bytes(&payload);
            let block_signature =
                BlockSignature::new(idx as u64, SignatureOf::from_signature(signature));
            set.insert(block_signature);
        }
        set
    }

    fn encode_sample<T: NoritoSerialize>(value: &T) -> SamplePayload {
        let (payload, flags) = encode_with_header_flags(value);
        SamplePayload { payload, flags }
    }

    fn sample_block_signature_vec() -> Vec<BlockSignature> {
        sample_block_signature_set().into_iter().collect()
    }

    fn sample_block_signature_payload() -> SamplePayload {
        let set = sample_block_signature_set();
        encode_sample(&set)
    }

    #[test]
    fn block_signature_getters_and_roundtrip() {
        let keypair = KeyPair::random();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let signature = SignatureOf::from_hash(keypair.private_key(), header.hash());
        let block_signature = BlockSignature::new(42, signature.clone());
        assert_eq!(block_signature.index(), 42);
        assert_eq!(block_signature.signature(), &signature);
        let wire_repr = wire::BlockSignatureWire::from(&block_signature);
        assert_eq!(BlockSignature::from(wire_repr.clone()), block_signature);

        let encoded = norito::to_bytes(&block_signature).expect("encode block signature");
        let payload = &encoded[norito::core::Header::SIZE..];

        let archived_sig = norito::from_bytes::<BlockSignature>(&encoded).expect("archived sig");
        let decoded_sig = norito::core::NoritoDeserialize::deserialize(archived_sig);
        assert_eq!(decoded_sig, block_signature);

        let decoded_try = norito::core::NoritoDeserialize::try_deserialize(archived_sig)
            .expect("try_deserialize BlockSignature");
        assert_eq!(decoded_try, block_signature);

        let decoded_adaptive =
            decode_adaptive::<BlockSignature>(payload).expect("decode BlockSignature payload");
        assert_eq!(decoded_adaptive, block_signature);

        let bare_bytes = block_signature.encode();
        let mut bare_cursor = bare_bytes.as_slice();
        let decoded_bare =
            BlockSignature::decode_all(&mut bare_cursor).expect("decode bare BlockSignature");
        assert_eq!(decoded_bare, block_signature);

        let ok = decoded_sig
            .signature()
            .verify_hash(keypair.public_key(), header.hash());
        assert!(ok.is_ok(), "decoded signature must verify the header hash");
    }

    #[test]
    fn block_header_setters_work() {
        let mut header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        header.set_height(nonzero!(2_u64));
        header.set_view_change_index(10);
        assert_eq!(header.height(), nonzero!(2_u64));
        assert_eq!(header.view_change_index(), 10);
    }

    #[test]
    fn block_header_defaults_confidential_digest() {
        let header = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
        assert_eq!(
            header.confidential_features(),
            Some(crate::confidential::DEFAULT_CONFIDENTIAL_FEATURE_DIGEST)
        );
    }

    #[test]
    fn block_signature_roundtrip_diagnostics() {
        // Diagnostic helper to print byte layouts and decoded indices for bare and header-framed paths.
        fn hex_prefix(bytes: &[u8], n: usize) -> String {
            let mut s = String::new();
            for b in bytes.iter().take(n) {
                use std::fmt::Write as _;
                let _ = write!(&mut s, "{b:02X}");
            }
            s
        }
        if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
            eprintln!(
                "Skipping: packed-struct diagnostic remains flaky. Set IROHA_RUN_IGNORED=1 to run."
            );
            return;
        }

        let keypair = KeyPair::random();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let signature = SignatureOf::from_hash(keypair.private_key(), header.hash());
        let bs = BlockSignature::new(42, signature.clone());

        // Bare codec bytes and decode
        let bare = bs.encode();
        let decoded_bare = BlockSignature::decode_all(&mut bare.as_slice()).expect("bare decode");

        // Header-framed bytes and decode via the Norito archive for inspection
        let header_bytes = norito::to_bytes(&bs).expect("encode block signature");
        let archived_sig = norito::from_bytes::<BlockSignature>(&header_bytes).expect("from_bytes");
        let decoded_hdr = norito::core::NoritoDeserialize::deserialize(archived_sig);
        let decoded_try =
            norito::core::NoritoDeserialize::try_deserialize(archived_sig).expect("fallible");

        eprintln!("BS bare len={} hdr len={}", bare.len(), header_bytes.len());
        eprintln!("bare[0..32] = {}", hex_prefix(&bare, 32));
        eprintln!("hdr[0..32]  = {}", hex_prefix(&header_bytes, 32));
        eprintln!(
            "decoded: bare.index={} hdr.index={} try.index={}",
            decoded_bare.index(),
            decoded_hdr.index(),
            decoded_try.index()
        );
        // Don't assert equality here; this test is for diagnostics under --nocapture.
        // The actual equality is checked in block_signature_getters_and_roundtrip.
    }

    #[test]
    fn block_signature_btreeset_roundtrip() {
        let keypair = KeyPair::random();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let signature = SignatureOf::from_hash(keypair.private_key(), header.hash());
        let block_signature = BlockSignature::new(7, signature);

        let mut set = std::collections::BTreeSet::new();
        set.insert(block_signature.clone());

        let encoded = norito::to_bytes(&set).expect("encode btreeset");
        let payload = &encoded[norito::core::Header::SIZE..];
        let decoded = decode_adaptive::<std::collections::BTreeSet<BlockSignature>>(payload)
            .expect("decode btreeset");
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded.iter().next().unwrap(), &block_signature);
    }

    #[test]
    fn block_signature_packed_decode_repro() {
        let sample = sample_block_signature_payload();
        let _flags_guard = norito::core::DecodeFlagsGuard::enter(sample.flags);
        let decoded =
            decode_adaptive::<std::collections::BTreeSet<BlockSignature>>(&sample.payload)
                .expect("decode packed BTreeSet<BlockSignature>");
        assert_eq!(decoded, sample_block_signature_set());
    }

    #[test]
    fn block_signature_vec_decode_vec_ptr_repro() {
        let vec = sample_block_signature_vec();
        let SamplePayload { payload, flags } = encode_sample(&vec);

        let _flags_guard = norito::core::DecodeFlagsGuard::enter(flags);
        let archived = norito::core::archived_from_slice_unchecked::<Vec<BlockSignature>>(&payload);
        let _payload_guard = norito::core::PayloadCtxGuard::enter(archived.bytes());
        let decoded =
            <Vec<BlockSignature> as norito::core::NoritoDeserialize<'_>>::try_deserialize(
                archived.as_ref(),
            )
            .expect("decode packed Vec<BlockSignature>");
        assert_eq!(decoded, vec);
    }

    #[test]
    fn block_header_da_commitments_roundtrip() {
        let mut header = BlockHeader::new(nonzero!(4_u64), None, None, None, 99, 1);
        let da_hash =
            HashOf::<DaCommitmentBundle>::from_untyped_unchecked(Hash::new([0xA5; Hash::LENGTH]));
        header.set_da_commitments_hash(Some(da_hash));
        let encoded = header.encode();
        let mut cursor = encoded.as_slice();
        let decoded = BlockHeader::decode_all(&mut cursor).expect("decode header");
        assert_eq!(decoded.da_commitments_hash(), Some(da_hash));
    }

    #[test]
    fn block_header_da_proof_policies_roundtrip() {
        let mut header = BlockHeader::new(nonzero!(4_u64), None, None, None, 99, 1);
        let da_hash =
            HashOf::<DaProofPolicyBundle>::from_untyped_unchecked(Hash::new([0xC4; Hash::LENGTH]));
        header.set_da_proof_policies_hash(Some(da_hash));
        let encoded = header.encode();
        let mut cursor = encoded.as_slice();
        let decoded = BlockHeader::decode_all(&mut cursor).expect("decode header");
        assert_eq!(decoded.da_proof_policies_hash(), Some(da_hash));
    }

    #[test]
    fn block_header_da_pin_intents_roundtrip() {
        let mut header = BlockHeader::new(nonzero!(4_u64), None, None, None, 99, 1);
        let da_hash =
            HashOf::<DaPinIntentBundle>::from_untyped_unchecked(Hash::new([0xB6; Hash::LENGTH]));
        header.set_da_pin_intents_hash(Some(da_hash));
        let encoded = header.encode();
        let mut cursor = encoded.as_slice();
        let decoded = BlockHeader::decode_all(&mut cursor).expect("decode header");
        assert_eq!(decoded.da_pin_intents_hash(), Some(da_hash));
    }

    #[test]
    fn block_header_prev_roster_evidence_hash_roundtrip() {
        let mut header = BlockHeader::new(nonzero!(4_u64), None, None, None, 99, 1);
        let evidence_hash = HashOf::<PreviousRosterEvidence>::from_untyped_unchecked(Hash::new(
            [0x7A; Hash::LENGTH],
        ));
        header.set_prev_roster_evidence_hash(Some(evidence_hash));
        let encoded = header.encode();
        let mut cursor = encoded.as_slice();
        let decoded = BlockHeader::decode_all(&mut cursor).expect("decode header");
        assert_eq!(decoded.prev_roster_evidence_hash(), Some(evidence_hash));
    }

    #[test]
    fn header_hash_captures_da_commitment_hash() {
        let mut header = BlockHeader::new(nonzero!(6_u64), None, None, None, 123, 0);
        let base = header.hash();
        let da_hash =
            HashOf::<DaCommitmentBundle>::from_untyped_unchecked(Hash::new([0xD1; Hash::LENGTH]));
        header.set_da_commitments_hash(Some(da_hash));
        let with_da = header.hash();
        assert_ne!(
            base, with_da,
            "DA commitment hash must influence header hash"
        );
    }

    #[test]
    fn header_hash_captures_da_proof_policy_hash() {
        let mut header = BlockHeader::new(nonzero!(6_u64), None, None, None, 123, 0);
        let base = header.hash();
        let da_hash =
            HashOf::<DaProofPolicyBundle>::from_untyped_unchecked(Hash::new([0xE2; Hash::LENGTH]));
        header.set_da_proof_policies_hash(Some(da_hash));
        let with_da = header.hash();
        assert_ne!(
            base, with_da,
            "DA proof policy hash must influence header hash"
        );
    }

    #[test]
    fn header_hash_captures_da_pin_intents_hash() {
        let mut header = BlockHeader::new(nonzero!(6_u64), None, None, None, 123, 0);
        let base = header.hash();
        let pin_hash =
            HashOf::<DaPinIntentBundle>::from_untyped_unchecked(Hash::new([0xE2; Hash::LENGTH]));
        header.set_da_pin_intents_hash(Some(pin_hash));
        let with_pins = header.hash();
        assert_ne!(
            base, with_pins,
            "DA pin intent hash must influence header hash"
        );
    }

    #[test]
    fn block_header_hash_captures_prev_roster_evidence_hash() {
        let mut header = BlockHeader::new(nonzero!(6_u64), None, None, None, 123, 0);
        let base = header.hash();
        let evidence_hash = HashOf::<PreviousRosterEvidence>::from_untyped_unchecked(Hash::new(
            [0x91; Hash::LENGTH],
        ));
        header.set_prev_roster_evidence_hash(Some(evidence_hash));
        let with_evidence = header.hash();
        assert_ne!(
            base, with_evidence,
            "previous roster evidence hash must influence header hash"
        );
    }

    #[test]
    fn header_hash_ignores_result_merkle_root_and_roundtrips() {
        // Build a header without result_merkle_root
        let header1 = BlockHeader::new(nonzero!(5_u64), None, None, None, 12345, 0);
        // Compute its consensus hash
        let h1 = header1.hash();

        // Same header but with a result_merkle_root set must have the same consensus hash
        // Use Hash::new (safe) to avoid relying on prehashed layout in tests
        let fake_root = HashOf::from_untyped_unchecked(Hash::new([9_u8; Hash::LENGTH]));
        let header2 = BlockHeader::new(nonzero!(5_u64), None, None, Some(fake_root), 12345, 0);
        let h2 = header2.hash();
        assert_eq!(h1, h2, "consensus hash must ignore result_merkle_root");

        // NOTE: Norito roundtrip for header2 is validated in integration tests.
        // Keep this unit test focused on consensus hashing behavior.
    }
}
