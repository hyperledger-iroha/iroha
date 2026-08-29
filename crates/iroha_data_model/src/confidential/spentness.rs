//! Permanent sparse spentness checkpoints for confidential assets.
//!
//! The public surface deliberately supports only a point proof for one typed
//! nullifier. It contains no collection, cursor, prefix scan, or nullifier-list
//! response from which a Torii endpoint could grow an enumerable spentness API.

use crate::{
    AssetDefinitionId, NetworkId,
    block::BlockHeader,
    privacy::{
        GoldilocksDigest384V1, PRIVACY_EXACT12_CATALOG_ID_V1, PrivacyNullifierV1,
        PrivacyTransactionIntentDigestV1,
    },
};
use iroha_crypto::HashOf;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

/// Number of key bits and sibling digests in every V1 spentness point proof.
pub const CONFIDENTIAL_SPENTNESS_TREE_DEPTH_V1: usize = 256;
/// Exact encoded byte width of the fixed V1 sibling path.
pub const CONFIDENTIAL_SPENTNESS_PATH_BYTES_V1: usize =
    CONFIDENTIAL_SPENTNESS_TREE_DEPTH_V1 * GoldilocksDigest384V1::BYTES;

const CONFIDENTIAL_SPENTNESS_PROTOCOL_ID_V1: &[u8] = b"confidential-asset-spentness-v1";
const CONFIDENTIAL_SPENTNESS_PROFILE_ID_V1: &[u8] =
    b"poseidon-x7-goldilocks-digest384-sparse-depth256-v1";

macro_rules! define_spentness_digest {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Clone,
            Copy,
            Debug,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[repr(transparent)]
        #[norito(decode_from_slice)]
        pub struct $name(GoldilocksDigest384V1);

        impl $name {
            /// Wrap an already canonical six-lane digest.
            #[must_use]
            pub const fn from_digest(digest: GoldilocksDigest384V1) -> Self {
                Self(digest)
            }

            /// Return the canonical six-lane digest.
            #[must_use]
            pub const fn digest(self) -> GoldilocksDigest384V1 {
                self.0
            }

            /// Borrow the canonical six-field-element encoding.
            #[must_use]
            pub fn as_bytes(&self) -> &[u8; GoldilocksDigest384V1::BYTES] {
                self.0.as_ref()
            }

            /// Return `true` only for the reserved all-zero placeholder.
            #[must_use]
            pub fn is_zero(&self) -> bool {
                self.0.words().iter().all(|word| *word == 0)
            }
        }

        #[cfg(feature = "json")]
        impl norito::json::FastJsonWrite for $name {
            fn write_json(&self, output: &mut String) {
                norito::json::FastJsonWrite::write_json(&self.0, output);
            }

            fn write_json_to(
                &self,
                output: &mut dyn norito::json::JsonWriteSink,
            ) -> Result<(), norito::json::BoundedJsonError> {
                norito::json::FastJsonWrite::write_json_to(&self.0, output)
            }
        }

        #[cfg(feature = "json")]
        impl norito::json::JsonDeserialize for $name {
            fn json_deserialize(
                parser: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                GoldilocksDigest384V1::json_deserialize(parser).map(Self)
            }
        }
    };
}

define_spentness_digest!(
    /// Root of one asset-scoped permanent sparse spentness tree.
    ConfidentialSpentnessRootV1
);
define_spentness_digest!(
    /// Digest of one finalized, network-bound spentness checkpoint.
    ConfidentialSpentnessCheckpointDigestV1
);

/// Fixed 256-sibling path ordered from leaf level to root level.
///
/// The fixed array is intentional. Binary decoding therefore consumes exactly
/// 12,288 bytes and cannot honor an attacker-selected sequence length. JSON has
/// a hand-written exact-cardinality decoder for the same reason.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(decode_from_slice)]
pub struct ConfidentialSpentnessPathV1(
    [GoldilocksDigest384V1; CONFIDENTIAL_SPENTNESS_TREE_DEPTH_V1],
);

impl ConfidentialSpentnessPathV1 {
    /// Construct an exact path, rejecting the reserved zero digest.
    ///
    /// # Errors
    ///
    /// Returns [`ConfidentialSpentnessErrorV1::ZeroSibling`] if any position
    /// contains the all-zero placeholder.
    pub fn new(
        siblings: [GoldilocksDigest384V1; CONFIDENTIAL_SPENTNESS_TREE_DEPTH_V1],
    ) -> Result<Self, ConfidentialSpentnessErrorV1> {
        if siblings.iter().any(digest_is_zero) {
            return Err(ConfidentialSpentnessErrorV1::ZeroSibling);
        }
        Ok(Self(siblings))
    }

    /// Return the exact V1 path length.
    #[must_use]
    pub const fn len(&self) -> usize {
        CONFIDENTIAL_SPENTNESS_TREE_DEPTH_V1
    }

    /// Return `false`; every V1 path contains all 256 siblings.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        false
    }

    /// Borrow one sibling by its leaf-to-root level.
    #[must_use]
    pub fn get(&self, level: usize) -> Option<GoldilocksDigest384V1> {
        self.0.get(level).copied()
    }

    /// Iterate over all siblings in canonical leaf-to-root order.
    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = GoldilocksDigest384V1> + DoubleEndedIterator + '_ {
        self.0.iter().copied()
    }

    /// Consume the path and return its fixed digest array.
    #[must_use]
    pub fn into_digests(self) -> [GoldilocksDigest384V1; CONFIDENTIAL_SPENTNESS_TREE_DEPTH_V1] {
        self.0
    }

    fn validate(&self) -> Result<(), ConfidentialSpentnessErrorV1> {
        if self.0.iter().any(digest_is_zero) {
            return Err(ConfidentialSpentnessErrorV1::ZeroSibling);
        }
        Ok(())
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for ConfidentialSpentnessPathV1 {
    fn write_json(&self, output: &mut String) {
        output.push('[');
        for (index, sibling) in self.0.iter().enumerate() {
            if index != 0 {
                output.push(',');
            }
            norito::json::FastJsonWrite::write_json(sibling, output);
        }
        output.push(']');
    }

    fn write_json_to(
        &self,
        output: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        output.begin_container()?;
        output.push('[')?;
        for (index, sibling) in self.0.iter().enumerate() {
            if index != 0 {
                output.push(',')?;
            }
            norito::json::FastJsonWrite::write_json_to(sibling, output)?;
        }
        output.push(']')?;
        output.end_container();
        Ok(())
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ConfidentialSpentnessPathV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let mut sequence = norito::json::SeqVisitor::new(parser)?;
        let mut siblings = Vec::with_capacity(CONFIDENTIAL_SPENTNESS_TREE_DEPTH_V1);
        while !sequence.is_finished() {
            if siblings.len() == CONFIDENTIAL_SPENTNESS_TREE_DEPTH_V1 {
                return Err(norito::json::Error::Message(
                    "confidential spentness path must contain exactly 256 siblings".to_owned(),
                ));
            }
            let sibling = sequence
                .next_element::<GoldilocksDigest384V1>()?
                .ok_or_else(|| {
                    norito::json::Error::Message(
                        "expected confidential spentness sibling".to_owned(),
                    )
                })?;
            if digest_is_zero(&sibling) {
                return Err(norito::json::Error::Message(
                    "confidential spentness sibling must not be zero".to_owned(),
                ));
            }
            siblings.push(sibling);
        }
        sequence.finish()?;
        let siblings = siblings.try_into().map_err(|_| {
            norito::json::Error::Message(
                "confidential spentness path must contain exactly 256 siblings".to_owned(),
            )
        })?;
        Ok(Self(siblings))
    }
}

/// Closed state tag committed at one confidential spentness leaf.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "state", content = "value", deny_unknown_fields)
)]
pub enum ConfidentialSpentnessStateKindV1 {
    /// The position is empty and has never been consumed.
    Unspent,
    /// The nullifier has been permanently consumed.
    Spent,
}

/// Fixed-shape state committed at one confidential nullifier's sparse-tree leaf.
///
/// Unspent leaves canonically carry a zero transaction digest and height zero;
/// spent leaves carry both non-zero fields. Keeping both fields present avoids
/// a size-skewed enum and gives every leaf one unambiguous wire shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ConfidentialSpentnessStateV1 {
    kind: ConfidentialSpentnessStateKindV1,
    transaction_intent_digest: PrivacyTransactionIntentDigestV1,
    spent_at_height: u64,
}

impl ConfidentialSpentnessStateV1 {
    /// Construct the unique empty-leaf state.
    #[must_use]
    pub fn unspent() -> Self {
        Self {
            kind: ConfidentialSpentnessStateKindV1::Unspent,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0; 32]),
            spent_at_height: 0,
        }
    }

    /// Construct one permanently spent leaf.
    ///
    /// # Errors
    ///
    /// Rejects a zero transaction digest or height zero.
    pub fn spent(
        transaction_intent_digest: PrivacyTransactionIntentDigestV1,
        spent_at_height: u64,
    ) -> Result<Self, ConfidentialSpentnessErrorV1> {
        let state = Self {
            kind: ConfidentialSpentnessStateKindV1::Spent,
            transaction_intent_digest,
            spent_at_height,
        };
        state.validate()?;
        Ok(state)
    }

    /// Return the closed spentness tag.
    #[must_use]
    pub const fn kind(self) -> ConfidentialSpentnessStateKindV1 {
        self.kind
    }

    /// Return the consuming intent digest, zero only for an unspent leaf.
    #[must_use]
    pub const fn transaction_intent_digest(self) -> PrivacyTransactionIntentDigestV1 {
        self.transaction_intent_digest
    }

    /// Return the finalized consuming height, zero only for an unspent leaf.
    #[must_use]
    pub const fn spent_at_height(self) -> u64 {
        self.spent_at_height
    }

    fn validate(self) -> Result<(), ConfidentialSpentnessErrorV1> {
        match self.kind {
            ConfidentialSpentnessStateKindV1::Unspent
                if self.transaction_intent_digest.is_zero() && self.spent_at_height == 0 =>
            {
                Ok(())
            }
            ConfidentialSpentnessStateKindV1::Spent
                if !self.transaction_intent_digest.is_zero() && self.spent_at_height != 0 =>
            {
                Ok(())
            }
            ConfidentialSpentnessStateKindV1::Unspent => {
                Err(ConfidentialSpentnessErrorV1::InvalidUnspentLeaf)
            }
            ConfidentialSpentnessStateKindV1::Spent => {
                Err(ConfidentialSpentnessErrorV1::InvalidSpentLeaf)
            }
        }
    }
}

/// One immutable consensus checkpoint for an asset's permanent spentness tree.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ConfidentialSpentnessCheckpointV1 {
    network_id: NetworkId,
    asset_definition_id: AssetDefinitionId,
    finalized_height: u64,
    finalized_block_hash: HashOf<BlockHeader>,
    root: ConfidentialSpentnessRootV1,
    previous_checkpoint: Option<ConfidentialSpentnessCheckpointDigestV1>,
}

impl ConfidentialSpentnessCheckpointV1 {
    /// Construct and validate one immutable checkpoint.
    ///
    /// Height zero is the sole checkpoint without a predecessor. Every later
    /// checkpoint must name its exact predecessor, making spentness history
    /// append-only and fork-specific.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a zero binding or invalid predecessor shape.
    pub fn new(
        network_id: NetworkId,
        asset_definition_id: AssetDefinitionId,
        finalized_height: u64,
        finalized_block_hash: HashOf<BlockHeader>,
        root: ConfidentialSpentnessRootV1,
        previous_checkpoint: Option<ConfidentialSpentnessCheckpointDigestV1>,
    ) -> Result<Self, ConfidentialSpentnessErrorV1> {
        let checkpoint = Self {
            network_id,
            asset_definition_id,
            finalized_height,
            finalized_block_hash,
            root,
            previous_checkpoint,
        };
        checkpoint.validate()?;
        Ok(checkpoint)
    }

    /// Validate the immutable checkpoint chain shape and non-zero bindings.
    pub fn validate(&self) -> Result<(), ConfidentialSpentnessErrorV1> {
        if self.root.is_zero()
            || self
                .finalized_block_hash
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(ConfidentialSpentnessErrorV1::ZeroBinding);
        }
        match (self.finalized_height, self.previous_checkpoint) {
            (0, None) => Ok(()),
            (0, Some(_)) | (_, None) => Err(ConfidentialSpentnessErrorV1::InvalidPredecessor),
            (_, Some(previous)) if previous.is_zero() => {
                Err(ConfidentialSpentnessErrorV1::ZeroBinding)
            }
            (_, Some(_)) => Ok(()),
        }
    }

    /// Compute the typed commitment to every checkpoint field.
    #[must_use]
    pub fn digest(&self) -> ConfidentialSpentnessCheckpointDigestV1 {
        let asset = self.asset_definition_id.encode();
        let height = self.finalized_height.to_le_bytes();
        let predecessor_present = [u8::from(self.previous_checkpoint.is_some())];
        let predecessor = self
            .previous_checkpoint
            .map_or([0_u8; GoldilocksDigest384V1::BYTES], |digest| {
                *digest.as_bytes()
            });
        ConfidentialSpentnessCheckpointDigestV1::from_digest(spentness_digest(
            b"checkpoint",
            b"finalized",
            0,
            self.finalized_height,
            &[
                self.network_id.as_bytes(),
                &asset,
                &height,
                self.finalized_block_hash.as_ref(),
                self.root.as_bytes(),
                &predecessor_present,
                &predecessor,
            ],
        ))
    }

    /// Return the exact deployment identity.
    #[must_use]
    pub const fn network_id(&self) -> &NetworkId {
        &self.network_id
    }

    /// Return the asset whose spentness state is committed.
    #[must_use]
    pub const fn asset_definition_id(&self) -> &AssetDefinitionId {
        &self.asset_definition_id
    }

    /// Return the finalized checkpoint height.
    #[must_use]
    pub const fn finalized_height(&self) -> u64 {
        self.finalized_height
    }

    /// Return the exact finalized block hash.
    #[must_use]
    pub const fn finalized_block_hash(&self) -> &HashOf<BlockHeader> {
        &self.finalized_block_hash
    }

    /// Return the asset-scoped sparse-tree root.
    #[must_use]
    pub const fn root(&self) -> ConfidentialSpentnessRootV1 {
        self.root
    }

    /// Return the predecessor commitment, absent only at genesis.
    #[must_use]
    pub const fn previous_checkpoint(&self) -> Option<ConfidentialSpentnessCheckpointDigestV1> {
        self.previous_checkpoint
    }
}

/// Exact point proof against one permanent spentness checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ConfidentialSpentnessProofV1 {
    checkpoint_digest: ConfidentialSpentnessCheckpointDigestV1,
    nullifier: PrivacyNullifierV1,
    state: ConfidentialSpentnessStateV1,
    path: ConfidentialSpentnessPathV1,
}

impl ConfidentialSpentnessProofV1 {
    /// Construct a fixed-shape point proof.
    ///
    /// # Errors
    ///
    /// Rejects zero bindings and any zero sibling placeholder.
    pub fn new(
        checkpoint_digest: ConfidentialSpentnessCheckpointDigestV1,
        nullifier: PrivacyNullifierV1,
        state: ConfidentialSpentnessStateV1,
        path: ConfidentialSpentnessPathV1,
    ) -> Result<Self, ConfidentialSpentnessErrorV1> {
        let proof = Self {
            checkpoint_digest,
            nullifier,
            state,
            path,
        };
        proof.validate_shape()?;
        Ok(proof)
    }

    /// Verify the exact checkpoint binding, leaf state, and all 256 siblings.
    pub fn verify(
        &self,
        checkpoint: &ConfidentialSpentnessCheckpointV1,
    ) -> Result<(), ConfidentialSpentnessErrorV1> {
        checkpoint.validate()?;
        self.validate_shape()?;
        if self.checkpoint_digest != checkpoint.digest() {
            return Err(ConfidentialSpentnessErrorV1::CheckpointMismatch);
        }
        if self.state.kind() == ConfidentialSpentnessStateKindV1::Spent
            && self.state.spent_at_height() > checkpoint.finalized_height
        {
            return Err(ConfidentialSpentnessErrorV1::InvalidSpentHeight);
        }
        let computed =
            self.compute_root(checkpoint.network_id(), checkpoint.asset_definition_id())?;
        if computed != checkpoint.root() {
            return Err(ConfidentialSpentnessErrorV1::RootMismatch);
        }
        Ok(())
    }

    /// Recompute the root for an exact network and asset context.
    ///
    /// This is used by consensus while constructing the next checkpoint as
    /// well as by point-proof verification.
    pub fn compute_root(
        &self,
        network_id: &NetworkId,
        asset_definition_id: &AssetDefinitionId,
    ) -> Result<ConfidentialSpentnessRootV1, ConfidentialSpentnessErrorV1> {
        self.validate_shape()?;
        let context = tree_context_digest(network_id, asset_definition_id);
        let mut node_index = *self.nullifier.as_bytes();
        let mut current = leaf_digest(&context, &node_index, self.nullifier, self.state);
        for (level, sibling) in self.path.iter().enumerate() {
            let direction = nullifier_bit_lsb_first(self.nullifier.as_bytes(), level);
            shift_big_endian_right_one(&mut node_index);
            let parent_level = u64::try_from(level + 1).expect("tree depth fits u64");
            current = if direction == 0 {
                node_digest(&context, parent_level, &node_index, current, sibling)
            } else {
                node_digest(&context, parent_level, &node_index, sibling, current)
            };
        }
        Ok(ConfidentialSpentnessRootV1::from_digest(current))
    }

    /// Return the bound checkpoint commitment.
    #[must_use]
    pub const fn checkpoint_digest(&self) -> ConfidentialSpentnessCheckpointDigestV1 {
        self.checkpoint_digest
    }

    /// Return the single queried nullifier.
    #[must_use]
    pub const fn nullifier(&self) -> PrivacyNullifierV1 {
        self.nullifier
    }

    /// Return the committed leaf state.
    #[must_use]
    pub const fn state(&self) -> ConfidentialSpentnessStateV1 {
        self.state
    }

    /// Borrow the fixed 256-sibling path.
    #[must_use]
    pub const fn path(&self) -> &ConfidentialSpentnessPathV1 {
        &self.path
    }

    fn validate_shape(&self) -> Result<(), ConfidentialSpentnessErrorV1> {
        if self.checkpoint_digest.is_zero() || self.nullifier.is_zero() {
            return Err(ConfidentialSpentnessErrorV1::ZeroBinding);
        }
        self.state.validate()?;
        self.path.validate()
    }
}

/// Validation failure for confidential spentness checkpoints and point proofs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ConfidentialSpentnessErrorV1 {
    /// A digest, nullifier, or block binding used the reserved zero value.
    #[error("confidential spentness binding must not be zero")]
    ZeroBinding,
    /// A sparse sibling used the reserved zero digest.
    #[error("confidential spentness sibling must not be zero")]
    ZeroSibling,
    /// Genesis/predecessor shape does not form an append-only checkpoint chain.
    #[error("confidential spentness predecessor shape is invalid")]
    InvalidPredecessor,
    /// An unspent leaf carried forbidden transaction or height material.
    #[error("confidential spentness unspent leaf is invalid")]
    InvalidUnspentLeaf,
    /// A spent leaf omitted its transaction or finalized-height binding.
    #[error("confidential spentness spent leaf is invalid")]
    InvalidSpentLeaf,
    /// The spent height is later than the checkpoint being proven.
    #[error("confidential spentness height is outside the checkpoint")]
    InvalidSpentHeight,
    /// The proof names another checkpoint.
    #[error("confidential spentness checkpoint digest mismatch")]
    CheckpointMismatch,
    /// The recomputed sparse root does not match the finalized checkpoint.
    #[error("confidential spentness sparse root mismatch")]
    RootMismatch,
}

fn digest_is_zero(digest: &GoldilocksDigest384V1) -> bool {
    digest.words().iter().all(|word| *word == 0)
}

fn spentness_domain(
    role: &'static [u8],
    phase: &'static [u8],
    level: u64,
    index: u64,
) -> fastpq_isi::GoldilocksDigestDomainV1<'static> {
    fastpq_isi::GoldilocksDigestDomainV1 {
        catalog: PRIVACY_EXACT12_CATALOG_ID_V1,
        protocol: CONFIDENTIAL_SPENTNESS_PROTOCOL_ID_V1,
        profile: CONFIDENTIAL_SPENTNESS_PROFILE_ID_V1,
        role,
        phase,
        level,
        index,
        counter: 0,
    }
}

fn spentness_digest(
    role: &'static [u8],
    phase: &'static [u8],
    level: u64,
    index: u64,
    fields: &[&[u8]],
) -> GoldilocksDigest384V1 {
    fastpq_isi::hash_bytes_384_v1(spentness_domain(role, phase, level, index), fields)
        .expect("fixed confidential spentness fields fit the canonical digest frame")
        .into()
}

fn tree_context_digest(
    network_id: &NetworkId,
    asset_definition_id: &AssetDefinitionId,
) -> GoldilocksDigest384V1 {
    let asset = asset_definition_id.encode();
    spentness_digest(
        b"tree-context",
        b"asset-scope",
        0,
        0,
        &[network_id.as_bytes(), &asset],
    )
}

fn leaf_digest(
    context: &GoldilocksDigest384V1,
    node_index: &[u8; 32],
    nullifier: PrivacyNullifierV1,
    state: ConfidentialSpentnessStateV1,
) -> GoldilocksDigest384V1 {
    let state_tag = match state.kind() {
        ConfidentialSpentnessStateKindV1::Unspent => [0_u8],
        ConfidentialSpentnessStateKindV1::Spent => [1_u8],
    };
    spentness_digest(
        b"sparse-tree",
        b"leaf",
        0,
        low_u64(node_index),
        &[
            context.as_ref(),
            node_index,
            nullifier.as_bytes(),
            &state_tag,
            state.transaction_intent_digest().as_bytes(),
            &state.spent_at_height().to_le_bytes(),
        ],
    )
}

fn node_digest(
    context: &GoldilocksDigest384V1,
    level: u64,
    node_index: &[u8; 32],
    left: GoldilocksDigest384V1,
    right: GoldilocksDigest384V1,
) -> GoldilocksDigest384V1 {
    spentness_digest(
        b"sparse-tree",
        b"node",
        level,
        low_u64(node_index),
        &[context.as_ref(), node_index, left.as_ref(), right.as_ref()],
    )
}

fn low_u64(bytes: &[u8; 32]) -> u64 {
    u64::from_be_bytes(
        bytes[24..]
            .try_into()
            .expect("32-byte index has an eight-byte suffix"),
    )
}

fn nullifier_bit_lsb_first(bytes: &[u8; 32], level: usize) -> u8 {
    let byte = 31 - level / 8;
    (bytes[byte] >> (level % 8)) & 1
}

fn shift_big_endian_right_one(bytes: &mut [u8; 32]) {
    let mut carry = 0_u8;
    for byte in bytes {
        let next_carry = *byte & 1;
        *byte = (*byte >> 1) | (carry << 7);
        carry = next_carry;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::Hash;

    fn network(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([seed; Hash::LENGTH]),
        ))
    }

    fn asset(seed: u8) -> AssetDefinitionId {
        let mut bytes = [seed; 16];
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        AssetDefinitionId::from_uuid_bytes(bytes).expect("fixture UUIDv4")
    }

    fn path() -> ConfidentialSpentnessPathV1 {
        ConfidentialSpentnessPathV1::new(core::array::from_fn(|index| {
            let word = u64::try_from(index + 1).expect("small fixture index");
            GoldilocksDigest384V1::new([word; 6]).expect("fixture field words are canonical")
        }))
        .expect("non-zero path")
    }

    fn proof_with_placeholder_checkpoint() -> ConfidentialSpentnessProofV1 {
        ConfidentialSpentnessProofV1::new(
            ConfidentialSpentnessCheckpointDigestV1::from_digest(
                GoldilocksDigest384V1::new([9; 6]).expect("canonical fixture digest"),
            ),
            PrivacyNullifierV1::new([0x42; 32]),
            ConfidentialSpentnessStateV1::spent(
                PrivacyTransactionIntentDigestV1::new([0x33; 32]),
                7,
            )
            .expect("valid spent fixture"),
            path(),
        )
        .expect("fixture proof shape")
    }

    fn checkpoint_and_proof() -> (
        ConfidentialSpentnessCheckpointV1,
        ConfidentialSpentnessProofV1,
    ) {
        let network = network(1);
        let asset = asset(2);
        let provisional = proof_with_placeholder_checkpoint();
        let root = provisional
            .compute_root(&network, &asset)
            .expect("compute fixture root");
        let previous = ConfidentialSpentnessCheckpointDigestV1::from_digest(
            GoldilocksDigest384V1::new([11; 6]).expect("canonical fixture predecessor"),
        );
        let checkpoint = ConfidentialSpentnessCheckpointV1::new(
            network,
            asset,
            8,
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x55; Hash::LENGTH])),
            root,
            Some(previous),
        )
        .expect("fixture checkpoint");
        let proof = ConfidentialSpentnessProofV1::new(
            checkpoint.digest(),
            provisional.nullifier(),
            provisional.state(),
            provisional.path().clone(),
        )
        .expect("checkpoint-bound proof");
        (checkpoint, proof)
    }

    #[test]
    fn fixed_path_and_checkpoint_roundtrip_and_verify() {
        let (checkpoint, proof) = checkpoint_and_proof();
        proof.verify(&checkpoint).expect("valid point proof");
        assert_eq!(proof.path().len(), CONFIDENTIAL_SPENTNESS_TREE_DEPTH_V1);

        let checkpoint_bytes = checkpoint.encode();
        let decoded_checkpoint =
            ConfidentialSpentnessCheckpointV1::decode(&mut checkpoint_bytes.as_slice())
                .expect("decode checkpoint");
        let proof_bytes = proof.encode();
        let decoded_proof = ConfidentialSpentnessProofV1::decode(&mut proof_bytes.as_slice())
            .expect("decode proof");
        assert_eq!(decoded_checkpoint, checkpoint);
        assert_eq!(decoded_proof, proof);
        decoded_proof
            .verify(&decoded_checkpoint)
            .expect("decoded proof verifies");
    }

    #[test]
    fn sibling_network_asset_and_checkpoint_substitution_fail() {
        let (checkpoint, proof) = checkpoint_and_proof();

        let mut siblings = proof.path().clone().into_digests();
        siblings[127] = GoldilocksDigest384V1::new([999; 6]).expect("canonical mutation");
        let mutated = ConfidentialSpentnessProofV1::new(
            proof.checkpoint_digest(),
            proof.nullifier(),
            proof.state(),
            ConfidentialSpentnessPathV1::new(siblings).expect("non-zero mutated path"),
        )
        .expect("mutated proof shape");
        assert_eq!(
            mutated.verify(&checkpoint),
            Err(ConfidentialSpentnessErrorV1::RootMismatch)
        );

        assert_ne!(
            proof
                .compute_root(&network(9), checkpoint.asset_definition_id())
                .expect("other-network root"),
            checkpoint.root()
        );
        assert_ne!(
            proof
                .compute_root(checkpoint.network_id(), &asset(9))
                .expect("other-asset root"),
            checkpoint.root()
        );

        let other_checkpoint = ConfidentialSpentnessCheckpointV1::new(
            *checkpoint.network_id(),
            checkpoint.asset_definition_id().clone(),
            checkpoint.finalized_height(),
            *checkpoint.finalized_block_hash(),
            checkpoint.root(),
            Some(ConfidentialSpentnessCheckpointDigestV1::from_digest(
                GoldilocksDigest384V1::new([12; 6]).expect("canonical predecessor"),
            )),
        )
        .expect("other checkpoint");
        assert_eq!(
            proof.verify(&other_checkpoint),
            Err(ConfidentialSpentnessErrorV1::CheckpointMismatch)
        );
    }

    #[test]
    fn genesis_chain_and_spent_height_are_fail_closed() {
        let (checkpoint, proof) = checkpoint_and_proof();
        assert_eq!(
            ConfidentialSpentnessCheckpointV1::new(
                *checkpoint.network_id(),
                checkpoint.asset_definition_id().clone(),
                0,
                *checkpoint.finalized_block_hash(),
                checkpoint.root(),
                checkpoint.previous_checkpoint(),
            ),
            Err(ConfidentialSpentnessErrorV1::InvalidPredecessor)
        );

        let future = ConfidentialSpentnessProofV1::new(
            proof.checkpoint_digest(),
            proof.nullifier(),
            ConfidentialSpentnessStateV1::spent(
                PrivacyTransactionIntentDigestV1::new([0x33; 32]),
                checkpoint.finalized_height() + 1,
            )
            .expect("valid future spent leaf"),
            proof.path().clone(),
        )
        .expect("future proof has a structurally valid leaf");
        assert_eq!(
            future.verify(&checkpoint),
            Err(ConfidentialSpentnessErrorV1::InvalidSpentHeight)
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn json_path_requires_exactly_256_canonical_digests() {
        let path = path();
        let json = norito::json::to_json(&path).expect("serialize fixed path");
        let decoded: ConfidentialSpentnessPathV1 =
            norito::json::from_str(&json).expect("decode fixed path");
        assert_eq!(decoded, path);

        let value: norito::json::Value =
            norito::json::from_str(&json).expect("parse path JSON value");
        let norito::json::Value::Array(mut entries) = value else {
            panic!("path JSON must be an array");
        };
        entries.pop();
        let short = norito::json::to_json(&norito::json::Value::Array(entries.clone()))
            .expect("serialize short path");
        assert!(norito::json::from_str::<ConfidentialSpentnessPathV1>(&short).is_err());

        entries.push(norito::json::Value::String("00".repeat(48)));
        entries.push(norito::json::Value::String("00".repeat(48)));
        let long = norito::json::to_json(&norito::json::Value::Array(entries))
            .expect("serialize long path");
        assert!(norito::json::from_str::<ConfidentialSpentnessPathV1>(&long).is_err());
    }
}
