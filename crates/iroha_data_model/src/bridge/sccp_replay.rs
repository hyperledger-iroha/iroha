//! Canonical sparse-Merkle replay accumulator for first-release SCCP routes.
//!
//! Consensus and destination contracts retain only sharded roots and counters.
//! Full leaves and witnesses are reconstructible from authenticated replay-delta
//! archives and are not part of the safety trust boundary.

#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use std::{collections::BTreeMap, vec::Vec};
use thiserror::Error;

use super::{SccpNetworkV1, SccpRouteKeyV1};
use crate::account::AccountId;

/// Domain prefix for every SCCP replay-accumulator hash.
pub const SCCP_REPLAY_SMT_MAGIC_V1: &[u8; 18] = b"SCCP-REPLAY-SMT-V1";
/// Number of independently updatable replay shards.
pub const SCCP_REPLAY_SMT_SHARD_COUNT_V1: usize = 256;
/// Depth below the one-byte shard selector.
pub const SCCP_REPLAY_SMT_DEPTH_V1: usize = 248;
/// Maximum number of explicitly encoded non-default siblings.
pub const SCCP_REPLAY_SMT_MAX_SIBLINGS_V1: usize = SCCP_REPLAY_SMT_DEPTH_V1;

/// Closed replay boundary and operation inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[norito(tag = "boundary", content = "operation")]
pub enum SccpReplayBoundaryV1 {
    /// SORA-side outbound lock/admission.
    #[codec(index = 0x01)]
    SoraOutboundLock,
    /// SORA-side native inbound release.
    #[codec(index = 0x02)]
    SoraInboundRelease,
    /// EVM/BSC external-to-Taira burn.
    #[codec(index = 0x10)]
    EvmSourceBurn,
    /// EVM/BSC Taira-to-external mint.
    #[codec(index = 0x11)]
    EvmDestinationMint,
    /// TRON external-to-Taira burn.
    #[codec(index = 0x20)]
    TronSourceBurn,
    /// TRON Taira-to-external mint.
    #[codec(index = 0x21)]
    TronDestinationMint,
    /// TON bridge inbound mint admission.
    #[codec(index = 0x30)]
    TonBridgeInboundMint,
    /// TON bridge outbound burn admission.
    #[codec(index = 0x31)]
    TonBridgeOutboundBurn,
    /// TON Jetton-master mint admission.
    #[codec(index = 0x32)]
    TonMasterMint,
    /// TON Jetton-master burn admission.
    #[codec(index = 0x33)]
    TonMasterBurn,
    /// TON recipient-wallet mint credit.
    #[codec(index = 0x34)]
    TonWalletMintCredit,
    /// TON custody-wallet SCCP burn debit.
    #[codec(index = 0x35)]
    TonWalletBurnDebit,
    /// TON custody-wallet refund debit.
    #[codec(index = 0x36)]
    TonWalletRefundDebit,
    /// TON recipient-wallet refund credit.
    #[codec(index = 0x37)]
    TonWalletRefundCredit,
}

impl SccpReplayBoundaryV1 {
    /// Return the byte committed by domain and record hashes.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::SoraOutboundLock => 0x01,
            Self::SoraInboundRelease => 0x02,
            Self::EvmSourceBurn => 0x10,
            Self::EvmDestinationMint => 0x11,
            Self::TronSourceBurn => 0x20,
            Self::TronDestinationMint => 0x21,
            Self::TonBridgeInboundMint => 0x30,
            Self::TonBridgeOutboundBurn => 0x31,
            Self::TonMasterMint => 0x32,
            Self::TonMasterBurn => 0x33,
            Self::TonWalletMintCredit => 0x34,
            Self::TonWalletBurnDebit => 0x35,
            Self::TonWalletRefundDebit => 0x36,
            Self::TonWalletRefundCredit => 0x37,
        }
    }
}

/// Canonical TON workchain and account identifier used by replay domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpTonAccountV1 {
    /// Signed TON workchain.
    pub workchain: i32,
    /// Raw TON account identifier.
    pub account: [u8; 32],
}

impl SccpTonAccountV1 {
    fn canonical_bytes(&self) -> [u8; 36] {
        let mut bytes = [0_u8; 36];
        bytes[..4].copy_from_slice(&self.workchain.to_be_bytes());
        bytes[4..].copy_from_slice(&self.account);
        bytes
    }
}

/// Contract or route identity that owns one replay boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[norito(tag = "actor", content = "identity")]
pub enum SccpReplayActorV1 {
    /// Route-wide SORA accumulator; no contract actor is encoded.
    #[codec(index = 0)]
    Route,
    /// Raw 20-byte EVM address.
    #[codec(index = 1)]
    Evm([u8; 20]),
    /// Raw 20-byte TRON account payload, excluding the `0x41` prefix.
    #[codec(index = 2)]
    Tron([u8; 20]),
    /// Canonical TON workchain and account identifier.
    #[codec(index = 3)]
    Ton(SccpTonAccountV1),
}

impl SccpReplayActorV1 {
    fn canonical_parts(&self) -> (u8, Vec<u8>) {
        match self {
            Self::Route => (0, Vec::new()),
            Self::Evm(address) => (1, address.to_vec()),
            Self::Tron(address) => (2, address.to_vec()),
            Self::Ton(account) => (3, account.canonical_bytes().to_vec()),
        }
    }
}

/// Economic principal committed by one occupied replay leaf.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[norito(tag = "principal", content = "identity")]
pub enum SccpReplayPrincipalV1 {
    /// Canonical domainless SORA account identifier.
    #[codec(index = 1)]
    SoraAccount(AccountId),
    /// Raw 20-byte EVM address.
    #[codec(index = 2)]
    Evm([u8; 20]),
    /// Raw 20-byte TRON account payload, excluding the `0x41` prefix.
    #[codec(index = 3)]
    Tron([u8; 20]),
    /// Canonical TON workchain and account identifier.
    #[codec(index = 4)]
    Ton(SccpTonAccountV1),
}

impl SccpReplayPrincipalV1 {
    fn canonical_parts(&self) -> Result<(u8, Vec<u8>), SccpReplayAccumulatorError> {
        let (kind, bytes) = match self {
            Self::SoraAccount(account) => (1, account.encode()),
            Self::Evm(address) => (2, address.to_vec()),
            Self::Tron(address) => (3, address.to_vec()),
            Self::Ton(account) => (4, account.canonical_bytes().to_vec()),
        };
        if bytes.is_empty() || u16::try_from(bytes.len()).is_err() {
            return Err(SccpReplayAccumulatorError::InvalidPrincipal);
        }
        Ok((kind, bytes))
    }
}

/// Complete domain for one replay forest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpReplayDomainV1 {
    /// Network where the economic operation originated.
    pub source_network: SccpNetworkV1,
    /// Network where the economic operation is consumed.
    pub target_network: SccpNetworkV1,
    /// Immutable replay boundary.
    pub boundary: SccpReplayBoundaryV1,
    /// Nonzero governed route revision.
    pub route_revision: u32,
    /// Complete governed route-configuration hash.
    pub route_configuration_hash: [u8; 32],
    /// Contract or route identity owning this forest.
    pub actor: SccpReplayActorV1,
}

/// Consensus key selecting one route-scoped replay forest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpReplayAccumulatorIdV1 {
    /// Exact immutable governed route.
    pub route_key: SccpRouteKeyV1,
    /// Replay boundary retained independently for this route.
    pub boundary: SccpReplayBoundaryV1,
}

/// Semantic material committed by an occupied replay leaf.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpReplayRecordV1 {
    /// Must equal the forest's boundary.
    pub operation: SccpReplayBoundaryV1,
    /// Existing SCCP message, source-event, or refund identifier.
    pub replay_id: [u8; 32],
    /// SHA-256 of exact canonical SCCP payload bytes.
    pub payload_sha256: [u8; 32],
    /// Positive scale-9 amount in canonical SCCP units.
    pub amount: u128,
    /// Economic recipient or owner.
    pub principal: SccpReplayPrincipalV1,
    /// SHA-256 of the operation-specific canonical auxiliary identity.
    pub auxiliary_identity_sha256: [u8; 32],
}

/// Canonically compressed sparse-Merkle membership or non-membership witness.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpSparseMerkleWitnessV1 {
    /// Root against which the caller prepared this witness.
    pub expected_shard_root: [u8; 32],
    /// Zero for non-membership, otherwise the exact occupied record digest.
    pub prior_record_digest: [u8; 32],
    /// Big-endian 256-bit bitmap; bits 248 through 255 are reserved and zero.
    pub sibling_bitmap: [u8; 32],
    /// Non-default siblings in strictly increasing leaf-up level order.
    pub siblings: Vec<[u8; 32]>,
}

impl SccpSparseMerkleWitnessV1 {
    /// Construct the unique canonical non-membership witness for an empty shard.
    #[must_use]
    pub fn empty_shard() -> Self {
        Self {
            expected_shard_root: sccp_replay_empty_hashes_v1()[SCCP_REPLAY_SMT_DEPTH_V1],
            prior_record_digest: [0; 32],
            sibling_bitmap: [0; 32],
            siblings: Vec::new(),
        }
    }
}

/// Constant-size consensus replay state for one route boundary.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpReplayForestV1 {
    /// Shards whose roots differ from the canonical empty root.
    pub nonempty_shard_roots: BTreeMap<u8, [u8; 32]>,
    /// Lifetime number of occupied leaves.
    pub leaf_count: u64,
    /// Lifetime number of root updates.
    pub update_sequence: u64,
}

impl Default for SccpReplayForestV1 {
    fn default() -> Self {
        Self {
            nonempty_shard_roots: BTreeMap::new(),
            leaf_count: 0,
            update_sequence: 0,
        }
    }
}

/// Authenticated replay transition emitted to rebuild witness indexes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpReplayDeltaV1 {
    /// Domain hash selecting the exact replay boundary.
    pub domain_hash: [u8; 32],
    /// First byte of the replay key.
    pub shard: u8,
    /// Complete leaf key.
    pub key: [u8; 32],
    /// Newly occupied semantic record digest.
    pub record_digest: [u8; 32],
    /// Root before occupation.
    pub old_root: [u8; 32],
    /// Root after occupation.
    pub new_root: [u8; 32],
    /// Forest leaf count after occupation.
    pub leaf_count: u64,
    /// Forest update sequence after occupation.
    pub update_sequence: u64,
}

/// Replay witness or accumulator validation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SccpReplayAccumulatorError {
    /// Domain has an unsupported or contradictory shape.
    #[error("invalid SCCP replay domain")]
    InvalidDomain,
    /// Principal has an empty or oversized canonical representation.
    #[error("invalid SCCP replay principal")]
    InvalidPrincipal,
    /// Replay identifier, payload commitment, or amount is zero.
    #[error("invalid SCCP replay record")]
    InvalidRecord,
    /// Record operation and forest boundary disagree.
    #[error("SCCP replay operation does not match its forest boundary")]
    WrongBoundary,
    /// Witness uses a non-canonical compressed representation.
    #[error("non-canonical SCCP sparse-Merkle witness")]
    NonCanonicalWitness,
    /// Witness was prepared against a different current shard root.
    #[error("stale SCCP sparse-Merkle witness root")]
    StaleRoot,
    /// Witness does not reconstruct its claimed root.
    #[error("invalid SCCP sparse-Merkle witness path")]
    InvalidPath,
    /// Fresh admission attempted to reuse an occupied leaf.
    #[error("SCCP replay leaf is already occupied")]
    Occupied,
    /// A lifetime counter cannot represent another update.
    #[error("SCCP replay accumulator counter exhausted")]
    CounterExhausted,
    /// Stored root/counter representation is not canonical.
    #[error("invalid SCCP replay forest state")]
    InvalidForest,
}

fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

fn network_tag(network: SccpNetworkV1) -> u32 {
    match network {
        SccpNetworkV1::SoraTaira => 0x40,
        SccpNetworkV1::EthereumMainnet => 0x41,
        SccpNetworkV1::BscMainnet => 0x42,
        SccpNetworkV1::TronMainnet => 0x43,
        SccpNetworkV1::TonMainnet => 0x44,
    }
}

fn domain_boundary_is_valid(domain: &SccpReplayDomainV1) -> bool {
    use SccpNetworkV1::{BscMainnet, EthereumMainnet, SoraTaira, TonMainnet, TronMainnet};
    use SccpReplayActorV1::{Evm, Route, Ton, Tron};
    use SccpReplayBoundaryV1::{
        EvmDestinationMint, EvmSourceBurn, SoraInboundRelease, SoraOutboundLock,
        TonBridgeInboundMint, TonBridgeOutboundBurn, TonMasterBurn, TonMasterMint,
        TonWalletBurnDebit, TonWalletMintCredit, TonWalletRefundCredit, TonWalletRefundDebit,
        TronDestinationMint, TronSourceBurn,
    };

    match (
        domain.boundary,
        domain.source_network,
        domain.target_network,
        &domain.actor,
    ) {
        (
            SoraOutboundLock,
            SoraTaira,
            EthereumMainnet | BscMainnet | TronMainnet | TonMainnet,
            Route,
        )
        | (
            SoraInboundRelease,
            EthereumMainnet | BscMainnet | TronMainnet | TonMainnet,
            SoraTaira,
            Route,
        )
        | (EvmDestinationMint, SoraTaira, EthereumMainnet | BscMainnet, Evm(_))
        | (EvmSourceBurn, EthereumMainnet | BscMainnet, SoraTaira, Evm(_))
        | (TronDestinationMint, SoraTaira, TronMainnet, Tron(_))
        | (TronSourceBurn, TronMainnet, SoraTaira, Tron(_))
        | (
            TonBridgeInboundMint
            | TonMasterMint
            | TonWalletMintCredit
            | TonWalletRefundDebit
            | TonWalletRefundCredit,
            SoraTaira,
            TonMainnet,
            Ton(_),
        )
        | (
            TonBridgeOutboundBurn | TonMasterBurn | TonWalletBurnDebit,
            TonMainnet,
            SoraTaira,
            Ton(_),
        ) => true,
        _ => false,
    }
}

/// Hash the canonical replay domain.
pub fn sccp_replay_domain_hash_v1(
    domain: &SccpReplayDomainV1,
) -> Result<[u8; 32], SccpReplayAccumulatorError> {
    if domain.route_revision == 0
        || domain.route_configuration_hash == [0; 32]
        || !domain_boundary_is_valid(domain)
    {
        return Err(SccpReplayAccumulatorError::InvalidDomain);
    }
    let source_network_tag = network_tag(domain.source_network);
    let target_network_tag = network_tag(domain.target_network);
    let (actor_kind, actor) = domain.actor.canonical_parts();
    let actor_len = u16::try_from(actor.len())
        .map_err(|_| SccpReplayAccumulatorError::InvalidDomain)?
        .to_be_bytes();
    Ok(sha256(&[
        SCCP_REPLAY_SMT_MAGIC_V1,
        &[0x00],
        &source_network_tag.to_be_bytes(),
        &target_network_tag.to_be_bytes(),
        &[domain.boundary.tag()],
        &domain.route_revision.to_be_bytes(),
        &domain.route_configuration_hash,
        &[actor_kind],
        &actor_len,
        &actor,
    ]))
}

/// Derive a replay leaf key from its domain and replay identifier.
#[must_use]
pub fn sccp_replay_key_v1(domain_hash: [u8; 32], replay_id: [u8; 32]) -> [u8; 32] {
    sha256(&[SCCP_REPLAY_SMT_MAGIC_V1, &[0x01], &domain_hash, &replay_id])
}

/// Hash one canonical occupied-leaf record.
pub fn sccp_replay_record_digest_v1(
    record: &SccpReplayRecordV1,
) -> Result<[u8; 32], SccpReplayAccumulatorError> {
    if record.replay_id == [0; 32]
        || record.payload_sha256 == [0; 32]
        || record.amount == 0
        || record.auxiliary_identity_sha256 == [0; 32]
    {
        return Err(SccpReplayAccumulatorError::InvalidRecord);
    }
    let (principal_kind, principal) = record.principal.canonical_parts()?;
    let principal_len = u16::try_from(principal.len())
        .map_err(|_| SccpReplayAccumulatorError::InvalidPrincipal)?
        .to_be_bytes();
    let principal_digest = sha256(&[
        SCCP_REPLAY_SMT_MAGIC_V1,
        &[0x03, principal_kind],
        &principal_len,
        &principal,
    ]);
    let auxiliary_digest = sha256(&[
        SCCP_REPLAY_SMT_MAGIC_V1,
        &[0x04, record.operation.tag()],
        &record.auxiliary_identity_sha256,
    ]);
    let digest = sha256(&[
        SCCP_REPLAY_SMT_MAGIC_V1,
        &[0x02, record.operation.tag()],
        &record.replay_id,
        &record.payload_sha256,
        &record.amount.to_be_bytes(),
        &principal_digest,
        &auxiliary_digest,
    ]);
    Ok(digest)
}

fn parent_hash(level: usize, left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    let level = u16::try_from(level).expect("SCCP replay depth fits u16");
    sha256(&[
        SCCP_REPLAY_SMT_MAGIC_V1,
        &[0x12],
        &level.to_be_bytes(),
        &left,
        &right,
    ])
}

/// Return all canonical empty hashes, indexed by leaf-up level.
#[must_use]
pub fn sccp_replay_empty_hashes_v1() -> [[u8; 32]; SCCP_REPLAY_SMT_DEPTH_V1 + 1] {
    let mut hashes = [[0; 32]; SCCP_REPLAY_SMT_DEPTH_V1 + 1];
    hashes[0] = sha256(&[SCCP_REPLAY_SMT_MAGIC_V1, &[0x10]]);
    for level in 0..SCCP_REPLAY_SMT_DEPTH_V1 {
        hashes[level + 1] = parent_hash(level, hashes[level], hashes[level]);
    }
    hashes
}

fn occupied_leaf_hash(key: [u8; 32], record_digest: [u8; 32]) -> [u8; 32] {
    sha256(&[SCCP_REPLAY_SMT_MAGIC_V1, &[0x11], &key, &record_digest])
}

fn bitmap_bit(bitmap: &[u8; 32], level: usize) -> bool {
    let byte = 31 - level / 8;
    let bit = level % 8;
    bitmap[byte] & (1 << bit) != 0
}

fn key_bit_leaf_up(key: &[u8; 32], level: usize) -> bool {
    let byte = 31 - level / 8;
    let bit = level % 8;
    key[byte] & (1 << bit) != 0
}

fn validate_and_expand_siblings(
    witness: &SccpSparseMerkleWitnessV1,
    empty: &[[u8; 32]; SCCP_REPLAY_SMT_DEPTH_V1 + 1],
) -> Result<[[u8; 32]; SCCP_REPLAY_SMT_DEPTH_V1], SccpReplayAccumulatorError> {
    if witness.siblings.len() > SCCP_REPLAY_SMT_MAX_SIBLINGS_V1
        || witness.sibling_bitmap[0] != 0
        || witness
            .sibling_bitmap
            .iter()
            .map(|byte| byte.count_ones() as usize)
            .sum::<usize>()
            != witness.siblings.len()
    {
        return Err(SccpReplayAccumulatorError::NonCanonicalWitness);
    }
    let mut expanded = [[0; 32]; SCCP_REPLAY_SMT_DEPTH_V1];
    let mut supplied = witness.siblings.iter();
    for level in 0..SCCP_REPLAY_SMT_DEPTH_V1 {
        expanded[level] = if bitmap_bit(&witness.sibling_bitmap, level) {
            let sibling = *supplied
                .next()
                .ok_or(SccpReplayAccumulatorError::NonCanonicalWitness)?;
            if sibling == empty[level] {
                return Err(SccpReplayAccumulatorError::NonCanonicalWitness);
            }
            sibling
        } else {
            empty[level]
        };
    }
    if supplied.next().is_some() {
        return Err(SccpReplayAccumulatorError::NonCanonicalWitness);
    }
    Ok(expanded)
}

fn fold_path(
    key: &[u8; 32],
    leaf: [u8; 32],
    siblings: &[[u8; 32]; SCCP_REPLAY_SMT_DEPTH_V1],
) -> [u8; 32] {
    siblings
        .iter()
        .enumerate()
        .fold(leaf, |current, (level, sibling)| {
            if key_bit_leaf_up(key, level) {
                parent_hash(level, *sibling, current)
            } else {
                parent_hash(level, current, *sibling)
            }
        })
}

impl SccpReplayForestV1 {
    /// Return the current effective root for a shard.
    #[must_use]
    pub fn shard_root(&self, shard: u8) -> [u8; 32] {
        self.nonempty_shard_roots
            .get(&shard)
            .copied()
            .unwrap_or_else(|| sccp_replay_empty_hashes_v1()[SCCP_REPLAY_SMT_DEPTH_V1])
    }

    /// Validate the constant-size stored representation.
    pub fn validate(&self) -> Result<(), SccpReplayAccumulatorError> {
        let empty_root = sccp_replay_empty_hashes_v1()[SCCP_REPLAY_SMT_DEPTH_V1];
        if self.nonempty_shard_roots.len() > SCCP_REPLAY_SMT_SHARD_COUNT_V1
            || self
                .nonempty_shard_roots
                .values()
                .any(|root| *root == empty_root)
            || (self.leaf_count == 0) != self.nonempty_shard_roots.is_empty()
            || self.leaf_count < u64::try_from(self.nonempty_shard_roots.len()).unwrap_or(u64::MAX)
            || self.update_sequence != self.leaf_count
        {
            return Err(SccpReplayAccumulatorError::InvalidForest);
        }
        Ok(())
    }

    /// Occupy one previously empty replay leaf atomically.
    pub fn occupy(
        &mut self,
        domain: &SccpReplayDomainV1,
        record: &SccpReplayRecordV1,
        witness: &SccpSparseMerkleWitnessV1,
    ) -> Result<SccpReplayDeltaV1, SccpReplayAccumulatorError> {
        self.validate()?;
        if record.operation != domain.boundary {
            return Err(SccpReplayAccumulatorError::WrongBoundary);
        }
        if witness.prior_record_digest != [0; 32] {
            return Err(SccpReplayAccumulatorError::Occupied);
        }
        let domain_hash = sccp_replay_domain_hash_v1(domain)?;
        let key = sccp_replay_key_v1(domain_hash, record.replay_id);
        let shard = key[0];
        let current_root = self.shard_root(shard);
        if witness.expected_shard_root != current_root {
            return Err(SccpReplayAccumulatorError::StaleRoot);
        }
        let empty = sccp_replay_empty_hashes_v1();
        let siblings = validate_and_expand_siblings(witness, &empty)?;
        let old_root = fold_path(&key, empty[0], &siblings);
        if old_root != current_root {
            return Err(SccpReplayAccumulatorError::InvalidPath);
        }
        let record_digest = sccp_replay_record_digest_v1(record)?;
        let new_root = fold_path(&key, occupied_leaf_hash(key, record_digest), &siblings);
        let leaf_count = self
            .leaf_count
            .checked_add(1)
            .ok_or(SccpReplayAccumulatorError::CounterExhausted)?;
        let update_sequence = self
            .update_sequence
            .checked_add(1)
            .ok_or(SccpReplayAccumulatorError::CounterExhausted)?;
        self.nonempty_shard_roots.insert(shard, new_root);
        self.leaf_count = leaf_count;
        self.update_sequence = update_sequence;
        Ok(SccpReplayDeltaV1 {
            domain_hash,
            shard,
            key,
            record_digest,
            old_root,
            new_root,
            leaf_count,
            update_sequence,
        })
    }

    /// Verify exact membership without mutating the forest.
    pub fn verify_membership(
        &self,
        domain: &SccpReplayDomainV1,
        record: &SccpReplayRecordV1,
        witness: &SccpSparseMerkleWitnessV1,
    ) -> Result<(), SccpReplayAccumulatorError> {
        self.validate()?;
        if record.operation != domain.boundary {
            return Err(SccpReplayAccumulatorError::WrongBoundary);
        }
        let expected_digest = sccp_replay_record_digest_v1(record)?;
        if witness.prior_record_digest != expected_digest {
            return Err(SccpReplayAccumulatorError::InvalidPath);
        }
        let domain_hash = sccp_replay_domain_hash_v1(domain)?;
        let key = sccp_replay_key_v1(domain_hash, record.replay_id);
        let current_root = self.shard_root(key[0]);
        if witness.expected_shard_root != current_root {
            return Err(SccpReplayAccumulatorError::StaleRoot);
        }
        let empty = sccp_replay_empty_hashes_v1();
        let siblings = validate_and_expand_siblings(witness, &empty)?;
        let root = fold_path(&key, occupied_leaf_hash(key, expected_digest), &siblings);
        if root != current_root {
            return Err(SccpReplayAccumulatorError::InvalidPath);
        }
        Ok(())
    }

    /// Verify an exact replay key and prior-record digest without re-deriving
    /// either value from higher-level protocol fields.
    ///
    /// This is the narrow verifier used at untrusted witness-service
    /// boundaries. A zero `record_digest` proves non-membership; a nonzero
    /// digest proves membership. Every other witness-canonicality and current
    /// shard-root rule is identical to [`Self::occupy`]. The complete 256-bit
    /// key space is valid, including the all-zero key if SHA-256 happens to
    /// derive it.
    pub fn verify_key_digest(
        &self,
        key: [u8; 32],
        record_digest: [u8; 32],
        witness: &SccpSparseMerkleWitnessV1,
    ) -> Result<(), SccpReplayAccumulatorError> {
        self.validate()?;
        if witness.prior_record_digest != record_digest {
            return Err(SccpReplayAccumulatorError::InvalidPath);
        }
        let current_root = self.shard_root(key[0]);
        if witness.expected_shard_root != current_root {
            return Err(SccpReplayAccumulatorError::StaleRoot);
        }
        let empty = sccp_replay_empty_hashes_v1();
        let siblings = validate_and_expand_siblings(witness, &empty)?;
        let leaf = if record_digest == [0; 32] {
            empty[0]
        } else {
            occupied_leaf_hash(key, record_digest)
        };
        if fold_path(&key, leaf, &siblings) != current_root {
            return Err(SccpReplayAccumulatorError::InvalidPath);
        }
        Ok(())
    }

    /// Verify exact non-membership without mutating the forest.
    pub fn verify_non_membership(
        &self,
        domain: &SccpReplayDomainV1,
        replay_id: [u8; 32],
        witness: &SccpSparseMerkleWitnessV1,
    ) -> Result<(), SccpReplayAccumulatorError> {
        self.validate()?;
        if replay_id == [0; 32] || witness.prior_record_digest != [0; 32] {
            return Err(SccpReplayAccumulatorError::InvalidPath);
        }
        let domain_hash = sccp_replay_domain_hash_v1(domain)?;
        let key = sccp_replay_key_v1(domain_hash, replay_id);
        let current_root = self.shard_root(key[0]);
        if witness.expected_shard_root != current_root {
            return Err(SccpReplayAccumulatorError::StaleRoot);
        }
        let empty = sccp_replay_empty_hashes_v1();
        let siblings = validate_and_expand_siblings(witness, &empty)?;
        if fold_path(&key, empty[0], &siblings) != current_root {
            return Err(SccpReplayAccumulatorError::InvalidPath);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn domain(boundary: SccpReplayBoundaryV1) -> SccpReplayDomainV1 {
        SccpReplayDomainV1 {
            source_network: SccpNetworkV1::SoraTaira,
            target_network: SccpNetworkV1::EthereumMainnet,
            boundary,
            route_revision: 7,
            route_configuration_hash: [0x44; 32],
            actor: SccpReplayActorV1::Route,
        }
    }

    fn record(boundary: SccpReplayBoundaryV1) -> SccpReplayRecordV1 {
        SccpReplayRecordV1 {
            operation: boundary,
            replay_id: [0x11; 32],
            payload_sha256: [0x22; 32],
            amount: 9,
            principal: SccpReplayPrincipalV1::Evm([0x33; 20]),
            auxiliary_identity_sha256: [0x55; 32],
        }
    }

    fn empty_witness() -> SccpSparseMerkleWitnessV1 {
        SccpSparseMerkleWitnessV1::empty_shard()
    }

    fn hex32(value: &str) -> [u8; 32] {
        let mut bytes = [0; 32];
        hex::decode_to_slice(value, &mut bytes).expect("golden hash is canonical hex");
        bytes
    }

    #[test]
    fn matches_cross_language_golden_vector() {
        // Keep in lockstep with fixtures/sccp/replay_forest_v1.json.
        let domain = domain(SccpReplayBoundaryV1::SoraOutboundLock);
        let record = record(SccpReplayBoundaryV1::SoraOutboundLock);
        let domain_hash = sccp_replay_domain_hash_v1(&domain).expect("golden domain is valid");
        assert_eq!(
            domain_hash,
            hex32("de11cbd183f55063fe715fcf120773d799dfb1185e057f758c126306832fdc3d")
        );
        assert_eq!(
            sccp_replay_key_v1(domain_hash, record.replay_id),
            hex32("139f57881d055a13ecf390d7441dadfc065ded40181c42a7aa3ab0a27469f17b")
        );
        assert_eq!(
            sccp_replay_record_digest_v1(&record).expect("golden record is valid"),
            hex32("35ab8613a0be06397609861d3cb3383770948b24b1cf098f4006c232240a2c07")
        );
        assert_eq!(
            empty_witness().expected_shard_root,
            hex32("cefd4f39c0d2ba5c33835008c6c3e7bca47d6ea1c4da5bfc8a63f09dbc66651f")
        );
        let mut forest = SccpReplayForestV1::default();
        let delta = forest
            .occupy(&domain, &record, &empty_witness())
            .expect("golden leaf occupies an empty shard");
        assert_eq!(delta.shard, 19);
        assert_eq!(
            delta.new_root,
            hex32("7b47c79900f052fd4b73691e2fe2230fdf170225d54e9a248e176f30495ac918")
        );
    }

    #[test]
    fn replay_network_tags_are_mainnet_only_and_final_v1() {
        assert_eq!(network_tag(SccpNetworkV1::SoraTaira), 0x40);
        assert_eq!(network_tag(SccpNetworkV1::EthereumMainnet), 0x41);
        assert_eq!(network_tag(SccpNetworkV1::BscMainnet), 0x42);
        assert_eq!(network_tag(SccpNetworkV1::TronMainnet), 0x43);
        assert_eq!(network_tag(SccpNetworkV1::TonMainnet), 0x44);
    }

    #[test]
    fn empty_witness_occupies_once_and_exact_membership_verifies() {
        let boundary = SccpReplayBoundaryV1::SoraOutboundLock;
        let domain = domain(boundary);
        let record = record(boundary);
        let mut forest = SccpReplayForestV1::default();
        let delta = forest
            .occupy(&domain, &record, &empty_witness())
            .expect("empty leaf is occupied");
        assert_eq!(forest.leaf_count, 1);
        assert_eq!(forest.update_sequence, 1);
        assert_eq!(forest.shard_root(delta.shard), delta.new_root);
        assert_ne!(delta.old_root, delta.new_root);

        let membership = SccpSparseMerkleWitnessV1 {
            expected_shard_root: delta.new_root,
            prior_record_digest: delta.record_digest,
            ..empty_witness()
        };
        forest
            .verify_membership(&domain, &record, &membership)
            .expect("exact occupied record verifies");
        assert_eq!(
            forest.occupy(&domain, &record, &membership),
            Err(SccpReplayAccumulatorError::Occupied)
        );
    }

    #[test]
    fn rejects_noncanonical_bitmap_default_and_stale_root() {
        let boundary = SccpReplayBoundaryV1::SoraOutboundLock;
        let domain = domain(boundary);
        let record = record(boundary);
        let mut forest = SccpReplayForestV1::default();

        let mut reserved = empty_witness();
        reserved.sibling_bitmap[0] = 1;
        reserved.siblings.push([0x77; 32]);
        assert_eq!(
            forest.occupy(&domain, &record, &reserved),
            Err(SccpReplayAccumulatorError::NonCanonicalWitness)
        );

        let mut explicit_default = empty_witness();
        explicit_default.sibling_bitmap[31] = 1;
        explicit_default
            .siblings
            .push(sccp_replay_empty_hashes_v1()[0]);
        assert_eq!(
            forest.occupy(&domain, &record, &explicit_default),
            Err(SccpReplayAccumulatorError::NonCanonicalWitness)
        );

        let mut stale = empty_witness();
        stale.expected_shard_root = [0x88; 32];
        assert_eq!(
            forest.occupy(&domain, &record, &stale),
            Err(SccpReplayAccumulatorError::StaleRoot)
        );
        assert_eq!(forest, SccpReplayForestV1::default());
    }

    #[test]
    fn exact_non_membership_rejects_occupied_and_wrong_paths() {
        let boundary = SccpReplayBoundaryV1::SoraOutboundLock;
        let domain = domain(boundary);
        let record = record(boundary);
        let mut forest = SccpReplayForestV1::default();
        forest
            .verify_non_membership(&domain, record.replay_id, &empty_witness())
            .expect("empty forest proves exact non-membership");
        let delta = forest
            .occupy(&domain, &record, &empty_witness())
            .expect("record occupies its leaf");
        let occupied = SccpSparseMerkleWitnessV1 {
            expected_shard_root: delta.new_root,
            prior_record_digest: delta.record_digest,
            ..empty_witness()
        };
        assert_eq!(
            forest.verify_non_membership(&domain, record.replay_id, &occupied),
            Err(SccpReplayAccumulatorError::InvalidPath)
        );
    }

    #[test]
    fn generic_key_digest_verifier_accepts_the_complete_key_space() {
        let forest = SccpReplayForestV1::default();
        forest
            .verify_key_digest([0; 32], [0; 32], &empty_witness())
            .expect("the all-zero derived key is not a sentinel");

        let mut wrong_digest = empty_witness();
        wrong_digest.prior_record_digest = [0x55; 32];
        assert_eq!(
            forest.verify_key_digest([0; 32], [0; 32], &wrong_digest),
            Err(SccpReplayAccumulatorError::InvalidPath)
        );
    }

    #[test]
    fn domain_and_record_changes_are_separated() {
        let boundary = SccpReplayBoundaryV1::SoraOutboundLock;
        let base_domain = domain(boundary);
        let base_record = record(boundary);
        let domain_hash = sccp_replay_domain_hash_v1(&base_domain).expect("valid domain");
        let record_hash = sccp_replay_record_digest_v1(&base_record).expect("valid record");

        let mut other_domain = base_domain.clone();
        other_domain.route_revision += 1;
        assert_ne!(
            domain_hash,
            sccp_replay_domain_hash_v1(&other_domain).expect("valid changed domain")
        );

        let mut other_record = base_record;
        other_record.amount += 1;
        assert_ne!(
            record_hash,
            sccp_replay_record_digest_v1(&other_record).expect("valid changed record")
        );

        let mut invalid_domain = base_domain;
        invalid_domain.target_network = SccpNetworkV1::SoraTaira;
        assert_eq!(
            sccp_replay_domain_hash_v1(&invalid_domain),
            Err(SccpReplayAccumulatorError::InvalidDomain)
        );

        let mut invalid_record = record(boundary);
        invalid_record.auxiliary_identity_sha256 = [0; 32];
        assert_eq!(
            sccp_replay_record_digest_v1(&invalid_record),
            Err(SccpReplayAccumulatorError::InvalidRecord)
        );
    }

    #[test]
    fn malformed_forest_and_counter_exhaustion_fail_without_mutation() {
        let empty_root = sccp_replay_empty_hashes_v1()[SCCP_REPLAY_SMT_DEPTH_V1];
        let malformed = SccpReplayForestV1 {
            nonempty_shard_roots: BTreeMap::from([(3, empty_root)]),
            leaf_count: 1,
            update_sequence: 1,
        };
        assert_eq!(
            malformed.validate(),
            Err(SccpReplayAccumulatorError::InvalidForest)
        );
        let impossible_shard_count = SccpReplayForestV1 {
            nonempty_shard_roots: BTreeMap::from([(3, [0x88; 32]), (4, [0x99; 32])]),
            leaf_count: 1,
            update_sequence: 1,
        };
        assert_eq!(
            impossible_shard_count.validate(),
            Err(SccpReplayAccumulatorError::InvalidForest)
        );

        let boundary = SccpReplayBoundaryV1::SoraOutboundLock;
        let domain = domain(boundary);
        let record = record(boundary);
        let occupied_other_shard = sccp_replay_key_v1(
            sccp_replay_domain_hash_v1(&domain).expect("valid domain"),
            record.replay_id,
        )[0]
        .wrapping_add(1);
        let mut exhausted = SccpReplayForestV1 {
            nonempty_shard_roots: BTreeMap::from([(occupied_other_shard, [0x99; 32])]),
            leaf_count: u64::MAX,
            update_sequence: u64::MAX,
        };
        let before = exhausted.clone();
        assert_eq!(
            exhausted.occupy(&domain, &record, &empty_witness()),
            Err(SccpReplayAccumulatorError::CounterExhausted)
        );
        assert_eq!(exhausted, before);
    }
}
