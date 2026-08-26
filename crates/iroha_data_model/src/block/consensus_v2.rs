//! Canonical Norito wire types for the Sumeragi v2 consensus protocol.
//!
//! Sumeragi v2 deliberately keeps its global Prepare/Commit protocol separate
//! from the lane-local [`super::consensus::CertPhase`] protocol.  The types in
//! this module are therefore versioned independently and do not replace or
//! reinterpret the first-release wire types in [`super::consensus`].
use super::Header as BlockHeader;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    NetworkId,
    account::AccountId,
    block::consensus::LaneBlockCommitment,
    consensus::GlobalThresholdBeaconPartialSignatureV1,
    merge::MergeLedgerEntry,
    nexus::{DataSpaceId, LaneFinalityStatement, LaneId, PublicLaneValidatorRecord},
    peer::PeerId,
    transaction::signed::{TransactionEntrypoint, TransactionResult},
};
use core::fmt;
use iroha_crypto::{Hash, HashOf, MerkleTree, MerkleTreeCommitment};
use iroha_primitives::erasure::rs16;
use iroha_schema::{EnumMeta, EnumVariant, Ident, IntoSchema, MetaMap, Metadata, TypeId};
use norito::codec::{Decode, Encode};
use std::{collections::BTreeSet, vec::Vec};
/// Durable finality artifacts associated with canonical Sumeragi v2 blocks.
pub mod finality;
/// Canonical genesis/handshake fingerprint projection.
pub mod fingerprint;
/// Sumeragi v2 wire protocol version.
pub const PROTOCOL_VERSION: u16 = 4;
/// Consensus-wide lower bound for one voting roster.
///
/// Every production committee has the exact `3f + 1` shape and tolerates at
/// least one Byzantine validator.
pub const MIN_VALIDATORS_PER_HEIGHT: usize = 4;
/// Maximum Byzantine validators tolerated by one frozen height context.
pub const MAX_FAULTS_PER_HEIGHT: usize = 10;
/// Consensus-wide upper bound for one voting roster.
///
/// This is a protocol admission limit, not a local resource-tuning knob.  It
/// must stay aligned with the production reducer and the formal Sumeragi v2
/// model so every admitted wire value has a representable verified state.
pub const MAX_VALIDATORS_PER_HEIGHT: usize = 3 * MAX_FAULTS_PER_HEIGHT + 1;
/// Returns whether `validator_count` has the production `3f + 1` geometry.
#[must_use]
pub const fn is_valid_committee_size(validator_count: usize) -> bool {
    validator_count >= MIN_VALIDATORS_PER_HEIGHT
        && validator_count <= MAX_VALIDATORS_PER_HEIGHT
        && (validator_count - 1).is_multiple_of(3)
}
/// Protocol-wide upper bound for one authenticated RS16 chunk.
pub const MAX_DA_CHUNK_SIZE_BYTES: u32 = 256 * 1024;
/// Protocol-wide upper bound for data shards in one RS16 stripe.
pub const MAX_DA_DATA_SHARDS: u16 = 16;
/// Protocol-wide upper bound for parity shards in one RS16 stripe.
pub const MAX_DA_PARITY_SHARDS: u16 = 16;
/// Protocol-wide upper bound for total shards in one RS16 stripe.
pub const MAX_DA_STRIPE_WIDTH: u16 = MAX_DA_DATA_SHARDS + MAX_DA_PARITY_SHARDS;
/// Protocol-wide upper bound for one canonical consensus payload.
pub const MAX_DA_PAYLOAD_SIZE_BYTES: u64 = 16 * 1024 * 1024;
/// Protocol-wide upper bound for all encoded shards of one maximum payload.
pub const MAX_DA_ENCODED_PAYLOAD_BYTES: u64 = 32 * 1024 * 1024;
/// Protocol-wide upper bound for encoded chunks committed by one manifest.
pub const MAX_DA_CHUNK_COUNT: u32 = 1024;
/// Maximum exact Commit vote groups exposed by one active-height liveness snapshot.
///
/// A reducer may retain one historical exact-lock group while the current
/// round contains at most one distinct subject group per validator.
pub const MAX_COMMIT_QUORUM_GROUPS_PER_HEIGHT: usize = MAX_VALIDATORS_PER_HEIGHT + 1;
const MAX_LIVENESS_IGNORE_REASONS: usize = 12;
/// Tight allocation bound for one consensus signature or aggregate.
pub const MAX_CONSENSUS_SIGNATURE_BYTES: usize = 256;
const HEIGHT_CONTEXT_IDENTITY_VERSION: u16 = 5;
/// Permissioned Sumeragi v2 handshake and domain-separation tag.
pub const PERMISSIONED_TAG: &str = "iroha2-consensus::permissioned-sumeragi@v2";
/// `NPoS` Sumeragi v2 handshake and domain-separation tag.
pub const NPOS_TAG: &str = "iroha2-consensus::npos-sumeragi@v2";
/// BLS domain selected by a permissioned v2 genesis.
pub const PERMISSIONED_BLS_DOMAIN: &str = "bls-iroha2:permissioned-sumeragi:v2";
/// BLS domain selected by an `NPoS` v2 genesis.
pub const NPOS_BLS_DOMAIN: &str = "bls-iroha2:npos-sumeragi:v2";
/// Maximum block-local Kagemusha top-up anchors authenticated by one execution commitment.
pub const MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK: u32 = 16;
/// Consensus-wide upper bound for the canonical result-bearing block wire.
///
/// This is the protocol authority shared by execution-commitment admission and
/// durable canonical-block storage. It deliberately matches the first-release
/// Kura hard limit; runtime configuration may select a lower bound but must
/// never admit a larger consensus value.
pub const MAX_EXECUTED_BLOCK_WIRE_BYTES: u64 = 256 * 1024 * 1024;
const KAGEMUSHA_TOPUP_POST_STATE_ROOT_DOMAIN: &[u8] = b"iroha:kagemusha:v2:post-state-root";
/// Canonical Native AMX application-manifest wire version.
pub const NATIVE_AMX_APPLICATION_MANIFEST_VERSION: u16 = 1;
/// Maximum participant route/incarnation leaves committed by one global block.
///
/// `MAX_ACTIVE_EXECUTION_LANES` is a fixed protocol limit of 1,024, so it is
/// representable by the wire-level `u32` field.
#[allow(clippy::cast_possible_truncation)]
pub const MAX_NATIVE_AMX_APPLICATION_MANIFEST_LEAVES: u32 =
    crate::nexus::MAX_ACTIVE_EXECUTION_LANES as u32;
/// Maximum lane-finality statements committed by one global block.
///
/// A canonical execution emits at most one statement per active lane route,
/// and the active-lane protocol bound is fixed at 1,024.
#[allow(clippy::cast_possible_truncation)]
pub const MAX_LANE_FINALITY_STATEMENTS_PER_BLOCK: u32 =
    crate::nexus::MAX_ACTIVE_EXECUTION_LANES as u32;
/// Maximum ordered source/result members in one participant application leaf.
pub const MAX_NATIVE_AMX_APPLICATION_MANIFEST_MEMBERS: usize = 4_096;
const NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT_DOMAIN: &[u8] =
    b"iroha:sumeragi:v2:native-amx-application-manifest:v1:empty";
/// Current merge-carrier commitment layout authenticated by global finality.
pub const MERGE_CARRIER_COMMITMENT_VERSION_V1: u16 = 1;
/// Canonical Nexus/AMX context commitment for the repository's recommended
/// single-lane defaults and no staged public-lane validators.
///
/// `iroha_config` owns the projection and pins this value with a golden test.
/// Keeping the bytes here lets configuration-independent genesis builders emit
/// a valid signed template without introducing a data-model/config cycle.
pub const RECOMMENDED_NEXUS_AMX_CONTEXT_HASH: [u8; 32] = [
    227, 185, 109, 139, 5, 226, 144, 128, 127, 248, 158, 128, 128, 197, 220, 195, 180, 113, 16,
    141, 61, 94, 144, 205, 65, 235, 216, 159, 48, 162, 211, 1,
];
/// Canonical V1 boot execution-policy identity emitted by the recommended genesis template.
///
/// Genesis materialization replaces this template value with the identity derived from the
/// complete staged runtime policy before signing. Startup never treats it as a fallback.
pub const RECOMMENDED_EXECUTION_POLICY_HASH: [u8; 32] = [
    63, 148, 116, 83, 117, 143, 142, 233, 11, 44, 102, 67, 122, 18, 143, 194, 45, 147, 196, 210,
    224, 202, 96, 194, 97, 216, 40, 183, 224, 184, 151, 195,
];
/// Block height in the v2 protocol.
pub type Height = u64;
/// View number within one block height.
pub type View = u64;
/// Index into the ordered voting roster frozen in a [`HeightContext`].
pub type ValidatorIndex = u32;
/// Consensus mode used to select the frozen equal-vote committee.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "mode",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ConsensusMode {
    /// Every validator has voting power one.
    Permissioned,
    /// Stake selects the finalized epoch committee; every member has one vote.
    Npos,
}
impl ConsensusMode {
    /// Return the canonical handshake and signing-domain tag for this mode.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Permissioned => PERMISSIONED_TAG,
            Self::Npos => NPOS_TAG,
        }
    }
    /// Return the canonical BLS domain for this mode.
    #[must_use]
    pub const fn bls_domain(self) -> &'static str {
        match self {
            Self::Permissioned => PERMISSIONED_BLS_DOMAIN,
            Self::Npos => NPOS_BLS_DOMAIN,
        }
    }
    /// Return whether this is permissioned consensus.
    #[must_use]
    pub const fn is_permissioned(self) -> bool {
        matches!(self, Self::Permissioned)
    }
}
impl From<crate::parameter::system::SumeragiConsensusMode> for ConsensusMode {
    fn from(mode: crate::parameter::system::SumeragiConsensusMode) -> Self {
        match mode {
            crate::parameter::system::SumeragiConsensusMode::Permissioned => Self::Permissioned,
            crate::parameter::system::SumeragiConsensusMode::Npos => Self::Npos,
        }
    }
}
impl From<ConsensusMode> for crate::parameter::system::SumeragiConsensusMode {
    fn from(mode: ConsensusMode) -> Self {
        match mode {
            ConsensusMode::Permissioned => Self::Permissioned,
            ConsensusMode::Npos => Self::Npos,
        }
    }
}
/// A validator and its consensus vote at one height.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct ValidatorPower {
    /// Validator identity and consensus public key.
    pub validator: PeerId,
    /// Consensus vote count. Protocol v4 requires this to be exactly one.
    pub power: u64,
}
/// Equal-vote quorum parameters frozen in a height context.
///
/// The roster has exact `n = 3f + 1` geometry and a certificate requires
/// `2f + 1` distinct signers. `total_power` is a redundant integrity
/// projection equal to the validator count because every member has one vote.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct DualQuorum {
    /// Required number of distinct validator signatures.
    pub min_signers: u32,
    /// Redundant total vote count represented by the ordered roster.
    pub total_power: u64,
}
impl DualQuorum {
    /// Compute the strict two-thirds count threshold for `validator_count`.
    #[must_use]
    pub fn count_threshold(validator_count: u32) -> Option<u32> {
        (validator_count != 0)
            .then(|| u64::from(validator_count) * 2 / 3 + 1)
            .and_then(|threshold| u32::try_from(threshold).ok())
    }
    /// Construct the canonical quorum projection for an ordered voting roster.
    ///
    /// # Errors
    ///
    /// Returns an error when the roster is empty, contains an invalid power,
    /// or its total power cannot be represented by `u64`.
    pub fn from_roster(roster: &[ValidatorPower]) -> Result<Self, ValidationError> {
        let validator_count =
            u32::try_from(roster.len()).map_err(|_| ValidationError::RosterTooLarge)?;
        let min_signers =
            Self::count_threshold(validator_count).ok_or(ValidationError::EmptyRoster)?;
        let total_power = validated_total_power(roster)?;
        Ok(Self {
            min_signers,
            total_power,
        })
    }
    fn validate_roster(&self, roster: &[ValidatorPower]) -> Result<(), ValidationError> {
        let canonical = Self::from_roster(roster)?;
        if self.min_signers != canonical.min_signers {
            return Err(ValidationError::CountThresholdMismatch);
        }
        if self.total_power != canonical.total_power {
            return Err(ValidationError::TotalPowerMismatch);
        }
        Ok(())
    }
    fn validate_signers(
        &self,
        signers: &[ValidatorIndex],
        roster: &[ValidatorPower],
    ) -> Result<(), ValidationError> {
        let signed_count = Self::validate_signer_set(signers, roster)?;
        if signed_count < self.min_signers {
            return Err(ValidationError::InsufficientSignerCount);
        }
        Ok(())
    }
    fn validate_certificate_signers(
        &self,
        signers: &[ValidatorIndex],
        roster: &[ValidatorPower],
    ) -> Result<(), ValidationError> {
        let signed_count = Self::validate_signer_set(signers, roster)?;
        if signed_count != self.min_signers {
            return Err(ValidationError::SignerCountMismatch {
                expected: self.min_signers,
                actual: signed_count,
            });
        }
        Ok(())
    }
    fn validate_signer_set(
        signers: &[ValidatorIndex],
        roster: &[ValidatorPower],
    ) -> Result<u32, ValidationError> {
        let signed_count =
            u32::try_from(signers.len()).map_err(|_| ValidationError::TooManySigners)?;
        if signers.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ValidationError::SignersNotStrictlySorted);
        }
        for signer in signers {
            let index = usize::try_from(*signer).map_err(|_| ValidationError::SignerOutOfRange)?;
            let entry = roster.get(index).ok_or(ValidationError::SignerOutOfRange)?;
            if entry.power != 1 {
                return Err(ValidationError::VotingPowerNotOne);
            }
        }
        Ok(signed_count)
    }
}
/// Payload chunking parameters frozen for one block height.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct DataAvailabilityLayout {
    /// Payload encoding used before chunk dissemination.
    pub encoding: PayloadEncoding,
    /// Maximum encoded chunk size in bytes.
    pub chunk_size_bytes: u32,
    /// Data shards per RS16 stripe.
    pub data_shards: u16,
    /// Parity shards per RS16 stripe.
    pub parity_shards: u16,
    /// Maximum canonical body size accepted at this height.
    pub max_payload_size_bytes: u64,
    /// Maximum number of encoded chunks accepted for one body.
    pub max_chunk_count: u32,
}
/// Payload encoding used by v2 data dissemination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "encoding",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum PayloadEncoding {
    /// Encode payload stripes with the deterministic RS16 layout.
    ReedSolomon16,
}
/// Genesis-selected transport inputs needed to construct every Sumeragi v2
/// height context.
///
/// The value is embedded in the signed consensus-genesis parameters. Live v2
/// startup must reject a genesis which omits it; it must never reconstruct
/// these fields from a node's mutable runtime configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiV2GenesisContextParameters {
    /// Mandatory deterministic data-availability layout for proposal bodies.
    pub da_layout: DataAvailabilityLayout,
    /// Canonical commitment to the staged Nexus/AMX consensus context.
    ///
    /// This binds enabled state, lane geometry and visibility, dataspace and
    /// routing policy, deterministic AMX budgets, and active public-lane
    /// validator records after staged genesis execution.
    pub nexus_amx_context_hash: [u8; 32],
    /// Canonical V1 identity of every process-local policy input which can affect execution.
    pub execution_policy_hash: [u8; 32],
}
impl SumeragiV2GenesisContextParameters {
    /// Recommended profile emitted by programmatic genesis builders.
    ///
    /// This value is serialized into, fingerprinted by, and signed with the
    /// genesis block. It is not a live-node fallback.
    #[must_use]
    pub fn recommended() -> Self {
        Self {
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: MAX_DA_CHUNK_SIZE_BYTES,
                data_shards: 4,
                parity_shards: 2,
                max_payload_size_bytes: MAX_DA_PAYLOAD_SIZE_BYTES,
                max_chunk_count: MAX_DA_CHUNK_COUNT,
            },
            nexus_amx_context_hash: RECOMMENDED_NEXUS_AMX_CONTEXT_HASH,
            execution_policy_hash: RECOMMENDED_EXECUTION_POLICY_HASH,
        }
    }
    /// Validate the signed context parameters using the same structural rules
    /// enforced for a full height context.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidDataAvailabilityLayout`] for a zero
    /// limit or an encoding/shard mismatch, and rejects zero or non-canonical
    /// policy commitments.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.nexus_amx_context_hash == [0; 32]
            || <[u8; Hash::LENGTH]>::from(Hash::prehashed(self.nexus_amx_context_hash))
                != self.nexus_amx_context_hash
        {
            return Err(ValidationError::InvalidNexusAmxContextHash);
        }
        if self.execution_policy_hash == [0; 32]
            || <[u8; Hash::LENGTH]>::from(Hash::prehashed(self.execution_policy_hash))
                != self.execution_policy_hash
        {
            return Err(ValidationError::InvalidExecutionPolicyHash);
        }
        validate_data_availability_layout(self.da_layout)
    }
}
/// Canonical staged active-lane record committed by v2 genesis metadata.
pub type GenesisActiveNexusLaneRecord = ((LaneId, AccountId), PublicLaneValidatorRecord);
/// Audited snapshot boundary which explicitly replaces an unavailable parent `CommitQC`.
///
/// The complete [`SnapshotV2BootstrapRecord`] is carried inside the signed or digest-pinned
/// snapshot payload. These fields bind its frozen context to the exact restored ledger
/// geometry and WSV, so an appended self-signed artifact cannot introduce a different trust root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SnapshotBootstrapAnchor {
    /// Last audited hash-only ledger height represented by the snapshot.
    pub snapshot_height: Height,
    /// Exact canonical block hash at `snapshot_height`.
    pub snapshot_block_hash: HashOf<BlockHeader>,
    /// Exact canonical ledger timestamp of the unavailable block at `snapshot_height`.
    ///
    /// The first executable successor derives its timestamp from this value and the committed
    /// block cadence; it must never fall back to a leader's local clock.
    pub snapshot_block_creation_time_ms: u64,
    /// Canonical WSV hash reconstructed from the authenticated snapshot payload.
    pub snapshot_state_hash: Hash,
}
/// Complete frozen Sumeragi-v2 trust root authenticated by an audited snapshot payload.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SnapshotV2BootstrapRecord {
    /// Record layout version; currently [`Self::VERSION`].
    pub version: u16,
    /// Exact first post-snapshot height context, including mode, seed, DA layout, and anchor.
    pub context: HeightContext,
    /// Roster-aligned BLS proofs of possession authenticated by the snapshot payload.
    pub validator_set_pops: Vec<Vec<u8>>,
}
impl SnapshotV2BootstrapRecord {
    /// Current record layout version.
    pub const VERSION: u16 = 1;
    /// Validate the record's structural context and snapshot-anchor relationship.
    ///
    /// Cryptographic `PoP` validation and comparison with restored live consensus keys are performed
    /// by the snapshot reader, which owns the authenticated WSV needed for those checks.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported version, a malformed context, a missing anchor, or a
    /// context height that is not the exact successor of the audited snapshot height.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.version != Self::VERSION {
            return Err(ValidationError::InvalidSnapshotBootstrap);
        }
        self.context.validate()?;
        let anchor = self
            .context
            .snapshot_bootstrap
            .as_ref()
            .ok_or(ValidationError::InvalidSnapshotBootstrap)?;
        if anchor.snapshot_height == 0
            || anchor.snapshot_height.checked_add(1) != Some(self.context.height)
            || self.validator_set_pops.len() != self.context.roster.len()
        {
            return Err(ValidationError::InvalidSnapshotBootstrap);
        }
        Ok(())
    }
}
/// Immutable inputs to consensus at one block height.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct HeightContext {
    /// Exact genesis-derived network identity used for replay protection.
    pub network_id: NetworkId,
    /// Wire protocol version; must equal [`PROTOCOL_VERSION`].
    pub protocol_version: u16,
    /// Height governed by this context.
    pub height: Height,
    /// Finalized validator-election epoch.
    pub epoch: u64,
    /// Last height governed by this epoch's frozen election snapshot.
    pub epoch_end_height: Height,
    /// Complete transition selected from the committed pre-state when this is
    /// the last height of an epoch. The `CommitQC` authenticates these bytes
    /// through [`Self::id`]; non-boundary contexts must carry `None`.
    #[norito(required)]
    pub next_epoch_snapshot: Option<finality::FinalizedNextEpochSnapshot>,
    /// Consensus mode that selected the equal-vote committee.
    pub mode: ConsensusMode,
    /// Commit certificate for the parent block, absent only at genesis or an audited snapshot
    /// bootstrap boundary.
    #[norito(required)]
    pub parent_commit_qc: Option<QuorumCertificate>,
    /// Explicit authenticated snapshot boundary used when the parent block body and v2 `CommitQC`
    /// predate the first-release v2 ledger. Mutually exclusive with `parent_commit_qc`.
    #[norito(required)]
    pub snapshot_bootstrap: Option<SnapshotBootstrapAnchor>,
    /// Deterministically ordered voting roster; observers are excluded.
    pub roster: Vec<ValidatorPower>,
    /// Canonical equal-vote quorum derived from `roster`.
    pub quorum: DualQuorum,
    /// Hash of all frozen Nexus/AMX inputs that proposal assembly and
    /// deterministic validation must bind.
    pub nexus_amx_context_hash: Hash,
    /// Canonical V1 identity of process-local execution policy.
    pub execution_policy_hash: Hash,
    /// Data-availability layout used by proposals at this height.
    pub da_layout: DataAvailabilityLayout,
    /// Finalized seed used to choose the view-zero roster offset.
    pub leader_seed: [u8; 32],
}
impl HeightContext {
    /// Return the typed hash that identifies every round in this context.
    ///
    /// The identity commits to the parent `CommitQC`'s semantic decision key
    /// (parent context, height, phase, subject, and execution commitment),
    /// rather than its round, aggregate signature, or signer subset. Two nodes
    /// that decide the same immutable body before or after an unchanged
    /// re-proposal therefore derive the same next-height context.
    #[must_use]
    pub fn id(&self) -> HeightContextId {
        let identity = HeightContextIdentity {
            identity_version: HEIGHT_CONTEXT_IDENTITY_VERSION,
            network_id: self.network_id,
            protocol_version: self.protocol_version,
            height: self.height,
            epoch: self.epoch,
            epoch_end_height: self.epoch_end_height,
            next_epoch_snapshot: self.next_epoch_snapshot.clone(),
            mode: self.mode,
            parent_commit: self
                .parent_commit_qc
                .as_ref()
                .map(|certificate| ParentCommitIdentity {
                    context_id: certificate.round.context_id,
                    height: certificate.round.height,
                    phase: certificate.phase,
                    subject: certificate.subject,
                    execution_commitment: certificate.execution_commitment,
                }),
            snapshot_bootstrap: self.snapshot_bootstrap,
            roster: self.roster.clone(),
            quorum: self.quorum,
            nexus_amx_context_hash: self.nexus_amx_context_hash,
            execution_policy_hash: self.execution_policy_hash,
            da_layout: self.da_layout,
            leader_seed: self.leader_seed,
        };
        HeightContextId(HashOf::from_untyped_unchecked(Hash::new(identity.encode())))
    }
    /// Validate the immutable context and its quorum snapshot.
    ///
    /// This does not verify the parent certificate's cryptographic signature.
    ///
    /// # Errors
    ///
    /// Returns a structural validation error for an unsupported protocol
    /// version, malformed roster, or non-canonical quorum.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ValidationError::UnsupportedProtocolVersion {
                expected: PROTOCOL_VERSION,
                actual: self.protocol_version,
            });
        }
        if self.epoch_end_height < self.height {
            return Err(ValidationError::EpochEndsBeforeHeight);
        }
        if self.nexus_amx_context_hash == Hash::prehashed([0; Hash::LENGTH]) {
            return Err(ValidationError::InvalidNexusAmxContextHash);
        }
        if self.execution_policy_hash == Hash::prehashed([0; Hash::LENGTH]) {
            return Err(ValidationError::InvalidExecutionPolicyHash);
        }
        match (
            self.height == self.epoch_end_height,
            self.next_epoch_snapshot.as_ref(),
        ) {
            (true, Some(snapshot)) => snapshot.validate_against(self)?,
            (true, None) => return Err(ValidationError::MissingNextEpochSnapshot),
            (false, Some(_)) => return Err(ValidationError::UnexpectedNextEpochSnapshot),
            (false, None) => {}
        }
        self.quorum.validate_roster(&self.roster)?;
        if self.roster.iter().any(|validator| validator.power != 1) {
            return Err(ValidationError::VotingPowerNotOne);
        }
        match (
            self.height,
            self.parent_commit_qc.as_ref(),
            self.snapshot_bootstrap.as_ref(),
        ) {
            (1, None, None) => {}
            (height, None, Some(anchor))
                if height > 1
                    && anchor.snapshot_height > 0
                    && anchor.snapshot_height.checked_add(1) == Some(height) => {}
            (0 | 1, _, _) | (_, Some(_), Some(_)) | (_, None, None) => {
                return Err(ValidationError::InvalidParentCommit);
            }
            (_, Some(parent), None)
                if parent.phase != GlobalPhase::Commit
                    || parent.round.height.checked_add(1) != Some(self.height)
                    || parent.proposal_round != parent.round =>
            {
                return Err(ValidationError::InvalidParentCommit);
            }
            (_, Some(_), None) => {}
            (_, None, Some(_)) => return Err(ValidationError::InvalidParentCommit),
        }
        if let Some(parent) = &self.parent_commit_qc {
            parent.execution_commitment.validate()?;
            if parent.signers.len() > MAX_VALIDATORS_PER_HEIGHT {
                return Err(ValidationError::TooManySigners);
            }
            if parent.signers.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ValidationError::SignersNotStrictlySorted);
            }
            require_aggregate_signature(&parent.aggregate_signature)?;
        }
        validate_data_availability_layout(self.da_layout)
    }
    /// Validate that a canonical signer list satisfies the equal-vote quorum.
    ///
    /// # Errors
    ///
    /// Returns a structural or quorum error when the context or signer list is
    /// invalid.
    pub fn validate_signers(&self, signers: &[ValidatorIndex]) -> Result<(), ValidationError> {
        self.validate()?;
        self.quorum.validate_signers(signers, &self.roster)
    }
    /// Validate that a canonical wire-certificate signer list has exactly `2f + 1` members.
    ///
    /// # Errors
    ///
    /// Returns a structural or exact-cardinality error when the context or
    /// signer list is invalid.
    pub fn validate_certificate_signers(
        &self,
        signers: &[ValidatorIndex],
    ) -> Result<(), ValidationError> {
        self.validate()?;
        self.quorum
            .validate_certificate_signers(signers, &self.roster)
    }
    /// Return the deterministic leader index for `view`.
    ///
    /// The view-zero offset is the full-width reduction of
    /// `H(leader_seed, height)` modulo the frozen roster length. Every later
    /// view advances by one roster position; voting power never changes leader
    /// frequency.
    #[must_use]
    pub fn leader(&self, view: View) -> ValidatorIndex {
        if self.roster.is_empty() {
            // Empty rosters are rejected by `validate`; retaining a total
            // function here keeps hostile decoded-but-unvalidated values from
            // turning an admission error into a modulo-by-zero panic.
            return 0;
        }
        let digest = Hash::new((self.leader_seed, self.height).encode());
        let modulus = u64::try_from(self.roster.len()).unwrap_or(u64::MAX);
        let start = digest.as_ref().iter().fold(0_u64, |remainder, byte| {
            let reduced =
                (u128::from(remainder) * 256 + u128::from(*byte)).rem_euclid(u128::from(modulus));
            u64::try_from(reduced).expect("a remainder modulo a u64 modulus always fits u64")
        });
        u32::try_from((start + view % modulus) % modulus)
            .expect("validated roster length fits ValidatorIndex")
    }
}
#[derive(Encode)]
struct HeightContextIdentity {
    identity_version: u16,
    network_id: NetworkId,
    protocol_version: u16,
    height: Height,
    epoch: u64,
    epoch_end_height: Height,
    next_epoch_snapshot: Option<finality::FinalizedNextEpochSnapshot>,
    mode: ConsensusMode,
    parent_commit: Option<ParentCommitIdentity>,
    snapshot_bootstrap: Option<SnapshotBootstrapAnchor>,
    roster: Vec<ValidatorPower>,
    quorum: DualQuorum,
    nexus_amx_context_hash: Hash,
    execution_policy_hash: Hash,
    da_layout: DataAvailabilityLayout,
    leader_seed: [u8; 32],
}
#[derive(Encode)]
struct ParentCommitIdentity {
    context_id: HeightContextId,
    height: Height,
    phase: GlobalPhase,
    subject: BlockSubject,
    execution_commitment: ExecutionCommitment,
}
/// Typed identifier of a complete [`HeightContext`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[repr(transparent)]
pub struct HeightContextId(
    /// Norito hash of the context's semantic identity projection.
    pub HashOf<HeightContext>,
);
/// Consensus round identity under a frozen height context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct ConsensusRound {
    /// Context governing this round.
    pub context_id: HeightContextId,
    /// Block height, repeated to support early wire rejection.
    pub height: Height,
    /// View number within the height.
    pub view: View,
}
/// Global Sumeragi v2 voting phase.
///
/// This enum intentionally has no `NewView` variant: view changes are certified
/// by [`TimeoutCertificate`].  It is also distinct from lane-local phases.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "phase",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum GlobalPhase {
    /// Certifies durable availability and deterministic validation.
    #[codec(index = 1)]
    Prepare = 1,
    /// Certifies finality for a prepared block.
    #[codec(index = 2)]
    Commit = 2,
}
impl TypeId for GlobalPhase {
    fn id() -> Ident {
        "SumeragiV2GlobalPhase".to_owned()
    }
}
impl IntoSchema for GlobalPhase {
    fn type_name() -> Ident {
        "SumeragiV2GlobalPhase".to_owned()
    }
    fn update_schema_map(metamap: &mut MetaMap) {
        let variants = vec![
            EnumVariant {
                tag: "Prepare".to_owned(),
                discriminant: Self::Prepare as u32,
                ty: None,
            },
            EnumVariant {
                tag: "Commit".to_owned(),
                discriminant: Self::Commit as u32,
                ty: None,
            },
        ];
        metamap.insert::<Self>(Metadata::Enum(EnumMeta { variants }));
    }
}
/// Proposal subject bound by votes and certificates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct BlockSubject {
    /// Parent block hash, absent only for the genesis block.
    #[norito(required)]
    pub parent_block_hash: Option<HashOf<BlockHeader>>,
    /// Proposed block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Hash of the canonical payload bytes.
    pub payload_hash: Hash,
}
/// Ordered result-bearing membership in one Native AMX participant application.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct NativeAmxApplicationManifestMemberV1 {
    /// Zero-based index of the source entrypoint in the canonical external block payload.
    pub entrypoint_index: u64,
    /// Source transaction identity authenticated by both participant QCs.
    pub source_id: [u8; Hash::LENGTH],
    /// Typed hash of the exact canonical transaction entrypoint.
    pub entrypoint_hash: HashOf<TransactionEntrypoint>,
    /// Typed hash of the exact deterministic transaction result.
    pub result_hash: HashOf<TransactionResult>,
}
/// Canonical result-bearing Native AMX participant application leaf.
///
/// A leaf is control evidence only. It binds one separate participant route to
/// the global carrier that executed its economic effects; it does not authorize
/// the participant lane to mutate WSV independently.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct NativeAmxApplicationManifestLeafV1 {
    /// Exact leaf schema version. No legacy layout is decoded implicitly.
    pub version: u16,
    /// Participant lane route.
    pub lane_id: LaneId,
    /// Participant dataspace route.
    pub dataspace_id: DataSpaceId,
    /// Exact active participant-lane incarnation.
    pub lane_incarnation: Hash,
    /// Contiguous participant-local block height.
    pub participant_height: u64,
    /// Participant-local consensus view.
    pub participant_view: u64,
    /// Exact predecessor participant-local height.
    pub predecessor_height: u64,
    /// Descriptor hash of the predecessor, absent only at the incarnation genesis.
    #[norito(required)]
    pub predecessor_descriptor_hash: Option<Hash>,
    /// Exact certified participant descriptor hash.
    pub descriptor_hash: Hash,
    /// Exact certified participant proposal hash.
    pub proposal_hash: Hash,
    /// Hash of the exact zero-effect participant control settlement.
    pub settlement_hash: HashOf<LaneBlockCommitment>,
    /// Ordered source, entrypoint, and result membership in canonical block order.
    pub members: Vec<NativeAmxApplicationManifestMemberV1>,
    /// Height of the canonical global application block.
    pub application_block_height: u64,
    /// Header hash of the canonical global application block.
    pub application_block_hash: HashOf<BlockHeader>,
    /// Hash of the canonical result-bearing global block wire.
    pub executed_block_wire_hash: Hash,
}
impl NativeAmxApplicationManifestLeafV1 {
    /// Validate the bounded, canonical leaf layout.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported version, a non-contiguous
    /// predecessor, malformed or duplicate membership, or a missing identity.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.version != NATIVE_AMX_APPLICATION_MANIFEST_VERSION {
            return Err(ValidationError::InvalidNativeAmxApplicationManifestVersion);
        }
        if self.participant_height == 0
            || self.application_block_height == 0
            || self.predecessor_height.checked_add(1) != Some(self.participant_height)
            || (self.predecessor_height == 0) != self.predecessor_descriptor_hash.is_none()
        {
            return Err(ValidationError::InvalidNativeAmxApplicationManifestLeaf);
        }
        if self.members.is_empty()
            || self.members.len() > MAX_NATIVE_AMX_APPLICATION_MANIFEST_MEMBERS
            || self
                .members
                .windows(2)
                .any(|pair| pair[0].entrypoint_index >= pair[1].entrypoint_index)
            || self
                .members
                .iter()
                .map(|member| member.source_id)
                .collect::<BTreeSet<_>>()
                .len()
                != self.members.len()
        {
            return Err(ValidationError::InvalidNativeAmxApplicationManifestMembership);
        }
        let identities = [
            self.lane_incarnation,
            self.descriptor_hash,
            self.proposal_hash,
            Hash::from(self.settlement_hash),
            Hash::from(self.application_block_hash),
            self.executed_block_wire_hash,
        ];
        let is_zero_like = |hash: &Hash| {
            let bytes = hash.as_ref();
            bytes[..Hash::LENGTH - 1].iter().all(|byte| *byte == 0) && bytes[Hash::LENGTH - 1] <= 1
        };
        if identities.iter().any(is_zero_like)
            || self
                .predecessor_descriptor_hash
                .is_some_and(|hash| is_zero_like(&hash))
            || self.members.iter().any(|member| {
                member.source_id.iter().all(|byte| *byte == 0)
                    || is_zero_like(&Hash::from(member.entrypoint_hash))
                    || is_zero_like(&Hash::from(member.result_hash))
            })
        {
            return Err(ValidationError::InvalidNativeAmxApplicationManifestLeaf);
        }
        Ok(())
    }
}
/// Canonical commitment used when a global block contains no separate
/// Native AMX participant applications.
#[must_use]
pub fn native_amx_application_manifest_empty_root() -> Hash {
    Hash::new(NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT_DOMAIN)
}
/// Exact merge-ledger identity authenticated by a global execution commitment.
///
/// The compact block reference remains the complete carrier proof while the
/// block body is locally available. This small projection preserves the same
/// association after body pruning through retained header/finality evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct MergeCarrierCommitmentV1 {
    /// Exact first-release projection version.
    pub version: u16,
    /// Canonical hash of the complete merge-ledger entry carried by the block.
    pub entry_hash: HashOf<MergeLedgerEntry>,
}
impl MergeCarrierCommitmentV1 {
    /// Construct the current exact merge-carrier projection.
    #[must_use]
    pub const fn new(entry_hash: HashOf<MergeLedgerEntry>) -> Self {
        Self {
            version: MERGE_CARRIER_COMMITMENT_VERSION_V1,
            entry_hash,
        }
    }
    /// Validate the current-only projection layout.
    ///
    /// # Errors
    ///
    /// Returns an error when a legacy or future projection version is used.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.version != MERGE_CARRIER_COMMITMENT_VERSION_V1 {
            return Err(ValidationError::InvalidMergeCarrierCommitmentVersion);
        }
        Ok(())
    }
}
/// Deterministic state-transition commitment authenticated by every Prepare and Commit vote.
///
/// The commitment is derived from the exact state-block execution witness
/// after deterministic candidate validation.  It is never
/// reconstructed from the proposal header or supplied by an untrusted caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct ExecutionCommitment {
    /// Root of the witnessed pre-state values for keys changed by the block.
    pub parent_state_root: Hash,
    /// Root of the complete deterministic post-state projection.
    pub post_state_root: Hash,
    /// Root of all canonical last-write-wins writes other than Kagemusha top-up anchors.
    pub ordinary_writes_root: Hash,
    /// Root of the canonical balanced Kagemusha top-up tree, when the block has top-ups.
    #[norito(required)]
    pub topup_anchor_root: Option<Hash>,
    /// Number of real Kagemusha top-up leaves committed by `topup_anchor_root`.
    pub topup_anchor_count: u32,
    /// Exact Native AMX application-manifest schema version.
    pub native_amx_application_manifest_version: u16,
    /// Merkle root of canonical separate-participant application leaves.
    pub native_amx_application_manifest_root: Hash,
    /// Number of leaves committed by `native_amx_application_manifest_root`.
    pub native_amx_application_manifest_count: u32,
    /// Exact root and leaf count of canonical post-execution lane effects.
    ///
    /// Absence is canonical only when the result-bearing block contains no
    /// lane-finality statements. A present commitment is derived from the
    /// executed block by validators; it is never supplied by relay callers.
    #[norito(required)]
    pub lane_finality_manifest: Option<MerkleTreeCommitment<LaneFinalityStatement>>,
    /// Exact compact merge-carrier identity, explicitly absent for ordinary blocks.
    ///
    /// This option is mandatory on the current wire. JSON must carry either an
    /// explicit `null` or a commitment; omission cannot decode as an implicitly
    /// carrier-free transition.
    #[norito(required)]
    pub merge_carrier: Option<MergeCarrierCommitmentV1>,
    /// Exact non-zero byte length of the canonical result-bearing block wire.
    pub executed_block_wire_len: u64,
    /// Hash of the canonical result-bearing block wire produced by deterministic execution.
    pub executed_block_wire_hash: Hash,
}
impl ExecutionCommitment {
    /// Construct a transition that contains neither Kagemusha top-up anchors
    /// nor a compact merge carrier.
    #[must_use]
    pub fn without_topups_or_merge_carrier(
        parent_state_root: Hash,
        post_state_root: Hash,
        ordinary_writes_root: Hash,
        executed_block_wire_len: u64,
        executed_block_wire_hash: Hash,
    ) -> Self {
        Self {
            parent_state_root,
            post_state_root,
            ordinary_writes_root,
            topup_anchor_root: None,
            topup_anchor_count: 0,
            native_amx_application_manifest_version: NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            native_amx_application_manifest_root: native_amx_application_manifest_empty_root(),
            native_amx_application_manifest_count: 0,
            lane_finality_manifest: None,
            merge_carrier: None,
            executed_block_wire_len,
            executed_block_wire_hash,
        }
    }
    /// Construct a carrier-free commitment and enforce its canonical top-up projection.
    ///
    /// # Errors
    ///
    /// Returns an error when root presence disagrees with the count, the
    /// bounded top-up count is exceeded, or the combined post-state root is
    /// not the canonical hash of the advertised top-up projection.
    pub fn new_without_merge_carrier(
        parent_state_root: Hash,
        post_state_root: Hash,
        ordinary_writes_root: Hash,
        topup_anchor_root: Option<Hash>,
        topup_anchor_count: u32,
        executed_block_wire_len: u64,
        executed_block_wire_hash: Hash,
    ) -> Result<Self, ValidationError> {
        Self::new_with_native_amx_application_manifest_without_merge_carrier(
            parent_state_root,
            post_state_root,
            ordinary_writes_root,
            topup_anchor_root,
            topup_anchor_count,
            NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            native_amx_application_manifest_empty_root(),
            0,
            executed_block_wire_len,
            executed_block_wire_hash,
        )
    }
    /// Construct a carrier-free commitment with an explicit Native AMX application manifest.
    ///
    /// # Errors
    ///
    /// Returns an error when either the state-transition projection or the
    /// versioned Native AMX manifest commitment is non-canonical.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the current clean-break execution-commitment wire"
    )]
    pub fn new_with_native_amx_application_manifest_without_merge_carrier(
        parent_state_root: Hash,
        post_state_root: Hash,
        ordinary_writes_root: Hash,
        topup_anchor_root: Option<Hash>,
        topup_anchor_count: u32,
        native_amx_application_manifest_version: u16,
        native_amx_application_manifest_root: Hash,
        native_amx_application_manifest_count: u32,
        executed_block_wire_len: u64,
        executed_block_wire_hash: Hash,
    ) -> Result<Self, ValidationError> {
        Self::new_with_native_amx_application_manifest_and_merge_carrier(
            parent_state_root,
            post_state_root,
            ordinary_writes_root,
            topup_anchor_root,
            topup_anchor_count,
            native_amx_application_manifest_version,
            native_amx_application_manifest_root,
            native_amx_application_manifest_count,
            None,
            executed_block_wire_len,
            executed_block_wire_hash,
        )
    }
    /// Construct a commitment with explicit Native AMX and merge-carrier projections.
    ///
    /// # Errors
    ///
    /// Returns an error when any state-transition, Native AMX, or merge-carrier
    /// projection is non-canonical.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the current clean-break execution-commitment wire"
    )]
    pub fn new_with_native_amx_application_manifest_and_merge_carrier(
        parent_state_root: Hash,
        post_state_root: Hash,
        ordinary_writes_root: Hash,
        topup_anchor_root: Option<Hash>,
        topup_anchor_count: u32,
        native_amx_application_manifest_version: u16,
        native_amx_application_manifest_root: Hash,
        native_amx_application_manifest_count: u32,
        merge_carrier: Option<MergeCarrierCommitmentV1>,
        executed_block_wire_len: u64,
        executed_block_wire_hash: Hash,
    ) -> Result<Self, ValidationError> {
        Self::new_with_manifests(
            parent_state_root,
            post_state_root,
            ordinary_writes_root,
            topup_anchor_root,
            topup_anchor_count,
            native_amx_application_manifest_version,
            native_amx_application_manifest_root,
            native_amx_application_manifest_count,
            None,
            merge_carrier,
            executed_block_wire_len,
            executed_block_wire_hash,
        )
    }
    /// Construct a commitment with all explicit execution manifests.
    ///
    /// # Errors
    ///
    /// Returns an error when the state-transition projection or any bounded
    /// manifest or merge-carrier commitment is non-canonical.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the canonical execution-commitment wire fields"
    )]
    pub fn new_with_manifests(
        parent_state_root: Hash,
        post_state_root: Hash,
        ordinary_writes_root: Hash,
        topup_anchor_root: Option<Hash>,
        topup_anchor_count: u32,
        native_amx_application_manifest_version: u16,
        native_amx_application_manifest_root: Hash,
        native_amx_application_manifest_count: u32,
        lane_finality_manifest: Option<MerkleTreeCommitment<LaneFinalityStatement>>,
        merge_carrier: Option<MergeCarrierCommitmentV1>,
        executed_block_wire_len: u64,
        executed_block_wire_hash: Hash,
    ) -> Result<Self, ValidationError> {
        let commitment = Self {
            parent_state_root,
            post_state_root,
            ordinary_writes_root,
            topup_anchor_root,
            topup_anchor_count,
            native_amx_application_manifest_version,
            native_amx_application_manifest_root,
            native_amx_application_manifest_count,
            lane_finality_manifest,
            merge_carrier,
            executed_block_wire_len,
            executed_block_wire_hash,
        };
        commitment.validate()?;
        Ok(commitment)
    }
    /// Validate the canonical count/root relationship and combined top-up root.
    ///
    /// # Errors
    ///
    /// Returns an execution-commitment error when a root/count pair is
    /// inconsistent, a protocol bound is exceeded, a combined state root is
    /// incorrect, or the Native AMX manifest version or empty-root convention
    /// is non-canonical.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.executed_block_wire_len == 0
            || self.executed_block_wire_len > MAX_EXECUTED_BLOCK_WIRE_BYTES
        {
            return Err(ValidationError::InvalidExecutedBlockWireLength);
        }
        if let Some(merge_carrier) = self.merge_carrier {
            merge_carrier.validate()?;
        }
        match (self.topup_anchor_count, self.topup_anchor_root) {
            (0, None) => {}
            (0, Some(_)) | (_, None) => {
                return Err(ValidationError::InvalidExecutionCommitment);
            }
            (count, Some(root)) if count <= MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK => {
                if self.post_state_root
                    != Self::topup_post_state_root(count, self.ordinary_writes_root, root)
                {
                    return Err(ValidationError::ExecutionCommitmentPostRootMismatch);
                }
            }
            (_, Some(_)) => return Err(ValidationError::TooManyKagemushaTopupAnchors),
        }
        if self.native_amx_application_manifest_version != NATIVE_AMX_APPLICATION_MANIFEST_VERSION {
            return Err(ValidationError::InvalidNativeAmxApplicationManifestVersion);
        }
        let empty_root = native_amx_application_manifest_empty_root();
        match self.native_amx_application_manifest_count {
            0 if self.native_amx_application_manifest_root == empty_root => {}
            0 => return Err(ValidationError::InvalidNativeAmxApplicationManifestCommitment),
            count if count > MAX_NATIVE_AMX_APPLICATION_MANIFEST_LEAVES => {
                return Err(ValidationError::TooManyNativeAmxApplicationManifestLeaves);
            }
            _ if self.native_amx_application_manifest_root == empty_root => {
                return Err(ValidationError::InvalidNativeAmxApplicationManifestCommitment);
            }
            _ => {}
        }
        if self.lane_finality_manifest.is_some_and(|commitment| {
            commitment.leaf_count().get() > u64::from(MAX_LANE_FINALITY_STATEMENTS_PER_BLOCK)
        }) {
            return Err(ValidationError::TooManyLaneFinalityStatements);
        }
        Ok(())
    }
    /// Derive the canonical combined post-state root for a non-empty top-up tree.
    #[must_use]
    pub fn topup_post_state_root(
        topup_anchor_count: u32,
        ordinary_writes_root: Hash,
        topup_anchor_root: Hash,
    ) -> Hash {
        let mut preimage = Vec::with_capacity(
            KAGEMUSHA_TOPUP_POST_STATE_ROOT_DOMAIN.len()
                + 1
                + core::mem::size_of::<u32>()
                + 2 * Hash::LENGTH,
        );
        preimage.extend_from_slice(KAGEMUSHA_TOPUP_POST_STATE_ROOT_DOMAIN);
        preimage.push(0);
        preimage.extend_from_slice(&topup_anchor_count.to_le_bytes());
        preimage.extend_from_slice(ordinary_writes_root.as_ref());
        preimage.extend_from_slice(topup_anchor_root.as_ref());
        Hash::new(preimage)
    }
}
/// One global Prepare or Commit vote.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct Vote {
    /// Round in which the vote was issued.
    pub round: ConsensusRound,
    /// Proposal round authenticated by the vote; equal to [`Self::round`].
    pub proposal_round: ConsensusRound,
    /// Prepare or Commit phase.
    pub phase: GlobalPhase,
    /// Exact proposal subject.
    pub subject: BlockSubject,
    /// Exact deterministic execution result certified by this vote.
    pub execution_commitment: ExecutionCommitment,
    /// Signer index in the height context roster.
    pub signer: ValidatorIndex,
    /// BLS signature over the canonical vote preimage.
    pub signature: Vec<u8>,
}
impl Vote {
    /// Return the domain-separated canonical bytes authenticated by this vote.
    ///
    /// The signature and signer fields are excluded so every signer of the
    /// same certificate signs the same BLS message. The authenticated-ingress
    /// adapter still selects the public key by `signer`, and the certificate
    /// binds its strictly ordered signer set, so a share cannot be reassigned
    /// to another key.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let payload = VoteSignaturePayload {
            protocol_version: PROTOCOL_VERSION,
            round: self.round,
            proposal_round: self.proposal_round,
            phase: self.phase,
            subject: self.subject,
            execution_commitment: self.execution_commitment,
        };
        signature_preimage(b"iroha:sumeragi:v2:vote", &payload.encode())
    }
    /// Validate the vote's context, signer, and signature presence.
    ///
    /// Cryptographic verification remains the authenticated-ingress adapter's
    /// responsibility and must use [`Self::signature_preimage`].
    ///
    /// # Errors
    ///
    /// Returns a structural validation error when the vote or proposal origin
    /// belongs to another context, its signer is outside the frozen roster,
    /// its execution commitment is invalid, or its signature is missing or
    /// oversized.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        validate_proposal_round(self.proposal_round, self.round, context)?;
        validate_validator_index(self.signer, context)?;
        self.execution_commitment.validate()?;
        require_signature(&self.signature)
    }
}
/// Canonical same-message fields authenticated by Prepare and Commit votes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct VoteSignaturePayload {
    /// Sumeragi protocol revision.
    pub protocol_version: u16,
    /// Exact round being voted in.
    pub round: ConsensusRound,
    /// Proposal round authenticated by the vote; equal to [`Self::round`].
    pub proposal_round: ConsensusRound,
    /// Prepare or Commit phase.
    pub phase: GlobalPhase,
    /// Exact block and payload subject.
    pub subject: BlockSubject,
    /// Exact deterministic execution result.
    pub execution_commitment: ExecutionCommitment,
}
/// Stable reference to a full quorum certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct QuorumCertificateRef {
    /// Certified round.
    pub round: ConsensusRound,
    /// Proposal round authenticated by the certificate; equal to [`Self::round`].
    pub proposal_round: ConsensusRound,
    /// Certified phase.
    pub phase: GlobalPhase,
    /// Certified subject.
    pub subject: BlockSubject,
    /// Certified deterministic execution result.
    pub execution_commitment: ExecutionCommitment,
}
impl QuorumCertificateRef {
    /// Return whether both references certify the same committed decision.
    ///
    /// `CommitQC`s for one immutable body may be assembled before or after an
    /// unchanged re-proposal. Their stable decision identity excludes the
    /// round and signer evidence while retaining context, height, subject, and
    /// deterministic execution.
    #[must_use]
    pub fn same_commit_decision(self, other: Self) -> bool {
        self.phase == GlobalPhase::Commit
            && other.phase == GlobalPhase::Commit
            && self.round.context_id == other.round.context_id
            && self.round.height == other.round.height
            && self.subject == other.subject
            && self.execution_commitment == other.execution_commitment
    }
}
/// Aggregate Prepare or Commit certificate.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct QuorumCertificate {
    /// Certified round.
    pub round: ConsensusRound,
    /// Proposal round shared by every aggregated vote; equal to [`Self::round`].
    pub proposal_round: ConsensusRound,
    /// Certified phase.
    pub phase: GlobalPhase,
    /// Certified proposal subject.
    pub subject: BlockSubject,
    /// Certified deterministic execution result shared by every signature.
    pub execution_commitment: ExecutionCommitment,
    /// Strictly increasing signer indices.
    pub signers: Vec<ValidatorIndex>,
    /// BLS aggregate signature for the canonical signer sequence.
    pub aggregate_signature: Vec<u8>,
}
impl QuorumCertificate {
    /// Return a stable reference to this certificate.
    #[must_use]
    pub fn as_ref(&self) -> QuorumCertificateRef {
        QuorumCertificateRef {
            round: self.round,
            proposal_round: self.proposal_round,
            phase: self.phase,
            subject: self.subject,
            execution_commitment: self.execution_commitment,
        }
    }
    /// Validate the certificate's context binding and equal-vote quorum.
    ///
    /// Cryptographic aggregate-signature verification remains the caller's
    /// responsibility.
    ///
    /// # Errors
    ///
    /// Returns a structural or quorum error if the certificate cannot be
    /// valid under `context`.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        validate_proposal_round(self.proposal_round, self.round, context)?;
        self.execution_commitment.validate()?;
        context.validate_certificate_signers(&self.signers)?;
        require_aggregate_signature(&self.aggregate_signature)
    }
    /// Reconstruct the canonical vote preimage for one certified signer.
    ///
    /// # Errors
    ///
    /// Returns an error when `signer` is not part of this certificate or is
    /// outside the frozen roster.
    pub fn signer_preimage(
        &self,
        context: &HeightContext,
        signer: ValidatorIndex,
    ) -> Result<Vec<u8>, ValidationError> {
        self.validate(context)?;
        if self.signers.binary_search(&signer).is_err() {
            return Err(ValidationError::SignerNotInCertificate);
        }
        Ok(Vote {
            round: self.round,
            proposal_round: self.proposal_round,
            phase: self.phase,
            subject: self.subject,
            execution_commitment: self.execution_commitment,
            signer,
            signature: Vec::new(),
        }
        .signature_preimage())
    }
}
/// One durable timeout vote for a view.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct TimeoutVote {
    /// Round whose timer expired.
    pub round: ConsensusRound,
    /// Highest `PrepareQC` known to the signer, if any.
    #[norito(required)]
    pub highest_prepare_qc: Option<QuorumCertificate>,
    /// Signer index in the height context roster.
    pub signer: ValidatorIndex,
    /// BLS signature over the canonical timeout-vote preimage.
    pub signature: Vec<u8>,
}
impl TimeoutVote {
    /// Return the domain-separated canonical bytes authenticated by this
    /// timeout vote, excluding the signature itself.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let payload = TimeoutVoteSignaturePayload {
            protocol_version: PROTOCOL_VERSION,
            round: self.round,
            highest_prepare_qc: self
                .highest_prepare_qc
                .as_ref()
                .map(QuorumCertificate::as_ref),
        };
        signature_preimage(b"iroha:sumeragi:v2:timeout-vote", &payload.encode())
    }
    /// Validate context binding, high-QC reference, signer, and signature
    /// presence.
    ///
    /// # Errors
    ///
    /// Returns a structural or quorum error when the timeout round or signer
    /// is invalid, the reported high certificate is not a valid non-future
    /// `PrepareQC` for the same context, or the signature is missing or
    /// oversized.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        validate_validator_index(self.signer, context)?;
        if let Some(highest) = &self.highest_prepare_qc {
            if highest.phase != GlobalPhase::Prepare {
                return Err(ValidationError::TimeoutCarriesNonPrepareQc);
            }
            if highest.round.context_id != self.round.context_id
                || highest.round.height != self.round.height
            {
                return Err(ValidationError::WrongHeightContext);
            }
            if highest.round.view > self.round.view {
                return Err(ValidationError::QcFromFutureView);
            }
            highest.validate(context)?;
        }
        require_signature(&self.signature)
    }
}
/// Canonical same-message fields authenticated by one timeout-vote group.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct TimeoutVoteSignaturePayload {
    /// Sumeragi protocol revision.
    pub protocol_version: u16,
    /// Timed-out round.
    pub round: ConsensusRound,
    /// Highest `PrepareQC` reported by every signer in this group.
    #[norito(required)]
    pub highest_prepare_qc: Option<QuorumCertificateRef>,
}
/// Aggregate timeout signatures that reported the same highest `PrepareQC`.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct TimeoutVoteGroup {
    /// Highest `PrepareQC` reported by this group, or none when no lock exists.
    #[norito(required)]
    pub highest_prepare_qc: Option<QuorumCertificate>,
    /// Strictly increasing signer indices in this group.
    pub signers: Vec<ValidatorIndex>,
    /// Aggregate BLS signature for this group's timeout votes.
    pub aggregate_signature: Vec<u8>,
}
/// Certificate authorizing a transition out of one timed-out view.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct TimeoutCertificate {
    /// Round whose timeout was certified.
    pub round: ConsensusRound,
    /// Groups ordered strictly by their optional `PrepareQC` reference.
    pub groups: Vec<TimeoutVoteGroup>,
}
impl TimeoutCertificate {
    /// Return a stable reference to this timeout certificate.
    #[must_use]
    pub fn as_ref(&self) -> TimeoutCertificateRef {
        TimeoutCertificateRef {
            round: self.round,
            highest_prepare_qc: self.highest_prepare_qc().map(QuorumCertificate::as_ref),
            certificate_hash: HashOf::new(self),
        }
    }
    /// Select the highest reported `PrepareQC` deterministically.
    ///
    /// View is the primary ordering key. The semantic certificate reference
    /// breaks impossible conflicting-subject ties without depending on which
    /// valid quorum subset happened to be aggregated.
    #[must_use]
    pub fn highest_prepare_qc(&self) -> Option<&QuorumCertificate> {
        self.groups
            .iter()
            .filter_map(|group| group.highest_prepare_qc.as_ref())
            .max_by(|left, right| {
                left.round
                    .view
                    .cmp(&right.round.view)
                    .then_with(|| left.as_ref().cmp(&right.as_ref()))
            })
    }
    /// Validate grouping, disjoint signers, context binding, and equal-vote quorum.
    ///
    /// Cryptographic aggregate-signature verification remains the caller's
    /// responsibility.
    ///
    /// # Errors
    ///
    /// Returns a structural or quorum error if the timeout certificate cannot
    /// be valid under `context`.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        if self.groups.is_empty() {
            return Err(ValidationError::EmptyTimeoutCertificate);
        }
        if self.groups.windows(2).any(|pair| {
            pair[0]
                .highest_prepare_qc
                .as_ref()
                .map(QuorumCertificate::as_ref)
                >= pair[1]
                    .highest_prepare_qc
                    .as_ref()
                    .map(QuorumCertificate::as_ref)
        }) {
            return Err(ValidationError::TimeoutGroupsNotStrictlySorted);
        }
        let mut all_signers = BTreeSet::new();
        let mut highest_at_view: Option<(View, BlockSubject, ExecutionCommitment)> = None;
        for group in &self.groups {
            if group.signers.is_empty() {
                return Err(ValidationError::EmptyTimeoutGroup);
            }
            require_aggregate_signature(&group.aggregate_signature)?;
            if group.signers.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ValidationError::SignersNotStrictlySorted);
            }
            if let Some(highest) = &group.highest_prepare_qc {
                if highest.phase != GlobalPhase::Prepare {
                    return Err(ValidationError::TimeoutCarriesNonPrepareQc);
                }
                if highest.round.context_id != self.round.context_id
                    || highest.round.height != self.round.height
                {
                    return Err(ValidationError::WrongHeightContext);
                }
                if highest.round.view > self.round.view {
                    return Err(ValidationError::QcFromFutureView);
                }
                highest.validate(context)?;
                match highest_at_view {
                    Some((view, subject, execution_commitment)) if view == highest.round.view => {
                        if subject != highest.subject
                            || execution_commitment != highest.execution_commitment
                        {
                            return Err(ValidationError::ConflictingHighestPrepare);
                        }
                    }
                    Some((view, _, _)) if view > highest.round.view => {
                        return Err(ValidationError::TimeoutGroupsNotStrictlySorted);
                    }
                    _ => {
                        highest_at_view = Some((
                            highest.round.view,
                            highest.subject,
                            highest.execution_commitment,
                        ));
                    }
                }
            }
            for signer in &group.signers {
                if !all_signers.insert(*signer) {
                    return Err(ValidationError::OverlappingTimeoutSigners);
                }
            }
        }
        let all_signers: Vec<_> = all_signers.into_iter().collect();
        context.validate_certificate_signers(&all_signers)
    }
}
/// Stable reference to a full timeout certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct TimeoutCertificateRef {
    /// Timed-out round certified by the TC.
    pub round: ConsensusRound,
    /// Highest `PrepareQC` selected from the grouped timeout votes.
    #[norito(required)]
    pub highest_prepare_qc: Option<QuorumCertificateRef>,
    /// Norito hash of the full timeout certificate.
    pub certificate_hash: HashOf<TimeoutCertificate>,
}
/// Justification carried by a proposal.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "kind",
    content = "justification",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ProposalJustification {
    /// View-zero justification from the parent `CommitQC`.
    ParentCommit(ParentCommitJustification),
    /// Later-view justification from the immediately preceding timeout.
    Timeout(TimeoutJustification),
}
/// View-zero proposal justification from the parent `CommitQC`.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct ParentCommitJustification {
    /// Parent `CommitQC`; absent only for the genesis block.
    #[norito(required)]
    pub certificate: Option<QuorumCertificate>,
}
/// Later-view proposal justification from a timeout certificate.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct TimeoutJustification {
    /// Certificate authorizing the new view.
    pub timeout_certificate: TimeoutCertificate,
    /// Full highest `PrepareQC` selected from the timeout groups.
    ///
    /// When present, the proposal must re-propose this certificate's exact
    /// subject. The value is repeated outside the grouped timeout votes so a
    /// proposal authenticates the complete `PrepareQC` used by its safe-value
    /// rule without requiring a receiver to reconstruct a signer subset.
    #[norito(required)]
    pub highest_prepare_qc: Option<QuorumCertificate>,
}
/// Manifest committing to a complete encoded block payload.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct PayloadManifest {
    /// Round for which the payload was proposed.
    pub round: ConsensusRound,
    /// Proposal subject containing the canonical payload hash.
    pub subject: BlockSubject,
    /// Canonical payload length before erasure-code padding.
    pub payload_size_bytes: u64,
    /// Chunking and erasure-code layout.
    pub layout: DataAvailabilityLayout,
    /// Hashes of encoded chunks in index order.
    pub chunk_hashes: Vec<Hash>,
    /// Merkle root over `chunk_hashes`.
    pub chunk_root: Hash,
}
/// Encode a canonical payload into the complete ordered RS16 chunk sequence
/// committed by [`PayloadManifest`].
///
/// This is the single owner of layout-driven striping, zero padding, and
/// parity ordering. Callers must never substitute raw payload slices for the
/// encoded chunk sequence accepted by [`PayloadManifest::derive`].
///
/// # Errors
///
/// Returns an error when the layout is invalid, the payload is empty or over
/// the frozen size bound, or the complete encoded sequence cannot be formed.
pub fn encode_payload_chunks(
    layout: DataAvailabilityLayout,
    payload: &[u8],
) -> Result<Vec<Vec<u8>>, ValidationError> {
    validate_data_availability_layout(layout)?;
    let payload_size =
        u64::try_from(payload.len()).map_err(|_| ValidationError::PayloadTooLarge)?;
    if payload.is_empty() {
        return Err(ValidationError::PayloadSizeMismatch);
    }
    if payload_size > layout.max_payload_size_bytes {
        return Err(ValidationError::PayloadTooLarge);
    }
    let chunk_size = usize::try_from(layout.chunk_size_bytes)
        .map_err(|_| ValidationError::InvalidChunkLength)?;
    let data_shards = usize::from(layout.data_shards);
    let parity_shards = usize::from(layout.parity_shards);
    let data_chunk_count = payload.len().div_ceil(chunk_size);
    let stripe_count = data_chunk_count.div_ceil(data_shards);
    let stripe_width = data_shards
        .checked_add(parity_shards)
        .ok_or(ValidationError::ChunkCountTooLarge)?;
    let encoded_chunk_count = stripe_count
        .checked_mul(stripe_width)
        .ok_or(ValidationError::ChunkCountTooLarge)?;
    let expected_chunk_count = usize::try_from(expected_encoded_chunk_count(payload_size, layout)?)
        .map_err(|_| ValidationError::ChunkCountTooLarge)?;
    if encoded_chunk_count != expected_chunk_count {
        return Err(ValidationError::PayloadSizeMismatch);
    }
    let mut encoded = Vec::with_capacity(encoded_chunk_count);
    let symbol_count = chunk_size / 2;
    for stripe in 0..stripe_count {
        let mut data = Vec::with_capacity(data_shards);
        let mut symbols = Vec::with_capacity(data_shards);
        for within in 0..data_shards {
            let data_index = stripe
                .checked_mul(data_shards)
                .and_then(|base| base.checked_add(within))
                .ok_or(ValidationError::ChunkCountTooLarge)?;
            let offset = data_index
                .checked_mul(chunk_size)
                .ok_or(ValidationError::PayloadTooLarge)?;
            let mut chunk = vec![0_u8; chunk_size];
            if offset < payload.len() {
                let end = offset.saturating_add(chunk_size).min(payload.len());
                chunk[..end - offset].copy_from_slice(&payload[offset..end]);
            }
            symbols.push(rs16::symbols_from_chunk(symbol_count, &chunk));
            data.push(chunk);
        }
        let parity = rs16::encode_parity(&symbols, parity_shards)
            .map_err(|_| ValidationError::InvalidDataAvailabilityLayout)?;
        encoded.extend(data);
        for shard in parity {
            encoded.push(
                rs16::chunk_from_symbols(&shard, chunk_size)
                    .map_err(|_| ValidationError::InvalidChunkLength)?,
            );
        }
    }
    Ok(encoded)
}
impl PayloadManifest {
    /// Derive the only canonical manifest for an encoded chunk sequence.
    ///
    /// # Errors
    ///
    /// Returns an error when the body or encoded chunks violate the frozen
    /// context limits/layout.
    pub fn derive(
        context: &HeightContext,
        round: ConsensusRound,
        subject: BlockSubject,
        payload_size_bytes: u64,
        encoded_chunks: &[Vec<u8>],
    ) -> Result<Self, ValidationError> {
        let chunk_hashes = encoded_chunks.iter().map(Hash::new).collect::<Vec<_>>();
        let chunk_root =
            payload_chunk_root(&chunk_hashes).ok_or(ValidationError::EmptyPayloadManifest)?;
        let manifest = Self {
            round,
            subject,
            payload_size_bytes,
            layout: context.da_layout,
            chunk_hashes,
            chunk_root,
        };
        manifest.validate(context)?;
        for chunk in encoded_chunks {
            validate_encoded_chunk_len(&manifest, chunk.len())?;
        }
        Ok(manifest)
    }
    /// Validate this manifest against its immutable height context.
    ///
    /// # Errors
    ///
    /// Returns a structural validation error when the round, DA layout, or
    /// chunk count cannot be valid under `context`.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        if self.layout != context.da_layout {
            return Err(ValidationError::WrongDataAvailabilityLayout);
        }
        if self.chunk_hashes.is_empty() {
            return Err(ValidationError::EmptyPayloadManifest);
        }
        if self.payload_size_bytes == 0 {
            return Err(ValidationError::PayloadSizeMismatch);
        }
        if self.payload_size_bytes > self.layout.max_payload_size_bytes {
            return Err(ValidationError::PayloadTooLarge);
        }
        let chunk_count = u32::try_from(self.chunk_hashes.len())
            .map_err(|_| ValidationError::ChunkCountTooLarge)?;
        if chunk_count > self.layout.max_chunk_count {
            return Err(ValidationError::ChunkCountTooLarge);
        }
        if chunk_count != expected_encoded_chunk_count(self.payload_size_bytes, self.layout)? {
            return Err(ValidationError::PayloadSizeMismatch);
        }
        if payload_chunk_root(&self.chunk_hashes) != Some(self.chunk_root) {
            return Err(ValidationError::ChunkRootMismatch);
        }
        Ok(())
    }
}
/// One encoded payload chunk.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct PayloadChunk {
    /// Manifest to which this chunk belongs.
    pub manifest_hash: HashOf<PayloadManifest>,
    /// Zero-based index in the manifest's chunk hash sequence.
    pub index: u32,
    /// Encoded chunk bytes.
    pub bytes: Vec<u8>,
    /// Sender index in the height context roster.
    pub sender: ValidatorIndex,
    /// Sender signature over [`Self::signature_preimage`].
    pub signature: Vec<u8>,
}
impl PayloadChunk {
    /// Validate the chunk's structural commitments and signature presence.
    ///
    /// Cryptographic signature verification remains the caller's
    /// responsibility and must use [`Self::signature_preimage`].
    ///
    /// # Errors
    ///
    /// Returns a structural validation error when this chunk does not match
    /// `context` and `manifest` or carries no signature.
    pub fn validate(
        &self,
        context: &HeightContext,
        manifest: &PayloadManifest,
    ) -> Result<(), ValidationError> {
        if self.signature.is_empty() {
            return Err(ValidationError::MissingChunkSignature);
        }
        if self.signature.len() > MAX_CONSENSUS_SIGNATURE_BYTES {
            return Err(ValidationError::SignatureTooLarge);
        }
        self.signature_payload(context, manifest).map(|_| ())
    }
    /// Build the canonical signature payload for this chunk.
    ///
    /// The total chunk count is deliberately not duplicated in
    /// [`PayloadChunk`]: it is committed by `manifest_hash` and obtained from
    /// `manifest.chunk_hashes`.  This payload binds context, epoch, height,
    /// view, subject, manifest, encoding, index, total count, chunk hash, and
    /// sender so a valid signature cannot be replayed into another session.
    ///
    /// # Errors
    ///
    /// Returns a structural validation error when this chunk does not match
    /// `context` and `manifest`.
    pub fn signature_payload(
        &self,
        context: &HeightContext,
        manifest: &PayloadManifest,
    ) -> Result<PayloadChunkSignaturePayload, ValidationError> {
        manifest.validate(context)?;
        if self.manifest_hash != HashOf::new(manifest) {
            return Err(ValidationError::ManifestHashMismatch);
        }
        let total_chunks = u32::try_from(manifest.chunk_hashes.len())
            .map_err(|_| ValidationError::ChunkCountTooLarge)?;
        let index =
            usize::try_from(self.index).map_err(|_| ValidationError::ChunkIndexOutOfRange)?;
        let expected_hash = manifest
            .chunk_hashes
            .get(index)
            .ok_or(ValidationError::ChunkIndexOutOfRange)?;
        validate_encoded_chunk_len(manifest, self.bytes.len())?;
        let chunk_hash = Hash::new(&self.bytes);
        if &chunk_hash != expected_hash {
            return Err(ValidationError::ChunkHashMismatch);
        }
        if usize::try_from(self.sender)
            .ok()
            .is_none_or(|sender| sender >= context.roster.len())
        {
            return Err(ValidationError::SignerOutOfRange);
        }
        Ok(PayloadChunkSignaturePayload {
            protocol_version: PROTOCOL_VERSION,
            context_id: manifest.round.context_id,
            epoch: context.epoch,
            height: manifest.round.height,
            view: manifest.round.view,
            subject: manifest.subject,
            manifest_hash: self.manifest_hash,
            encoding: manifest.layout.encoding,
            index: self.index,
            total_chunks,
            chunk_hash,
            sender: self.sender,
        })
    }
    /// Return the domain-separated bytes that the sender must sign.
    ///
    /// # Errors
    ///
    /// Returns a structural validation error when this chunk does not match
    /// `context` and `manifest`.
    pub fn signature_preimage(
        &self,
        context: &HeightContext,
        manifest: &PayloadManifest,
    ) -> Result<Vec<u8>, ValidationError> {
        const DOMAIN: &[u8] = b"iroha:sumeragi:v2:payload-chunk";
        let payload = self.signature_payload(context, manifest)?;
        let encoded = payload.encode();
        let mut preimage = Vec::with_capacity(DOMAIN.len() + encoded.len());
        preimage.extend_from_slice(DOMAIN);
        preimage.extend_from_slice(&encoded);
        Ok(preimage)
    }
}
/// Canonical fields authenticated by a v2 payload-chunk signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct PayloadChunkSignaturePayload {
    /// Sumeragi protocol version.
    pub protocol_version: u16,
    /// Complete immutable height-context identity.
    pub context_id: HeightContextId,
    /// Finalized validator-election epoch.
    pub epoch: u64,
    /// Proposed block height.
    pub height: Height,
    /// Proposal view.
    pub view: View,
    /// Exact block and payload subject.
    pub subject: BlockSubject,
    /// Manifest committing to the chunk sequence.
    pub manifest_hash: HashOf<PayloadManifest>,
    /// Chunk encoding bound by the manifest.
    pub encoding: PayloadEncoding,
    /// Zero-based chunk index.
    pub index: u32,
    /// Total chunks committed by the manifest.
    pub total_chunks: u32,
    /// Hash of this chunk's encoded bytes.
    pub chunk_hash: Hash,
    /// Sender index in the height context roster.
    pub sender: ValidatorIndex,
}
/// Signed proposal for one round.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct Proposal {
    /// Proposed round.
    pub round: ConsensusRound,
    /// Validator index of the expected leader.
    pub proposer: ValidatorIndex,
    /// Exact block and payload subject.
    pub subject: BlockSubject,
    /// Manifest used to reconstruct and durably store the payload.
    pub manifest: PayloadManifest,
    /// Parent or timeout justification for this view.
    pub justification: ProposalJustification,
    /// Leader signature over the canonical proposal preimage.
    pub signature: Vec<u8>,
}
impl Proposal {
    /// Return the domain-separated canonical bytes authenticated by the
    /// expected leader, excluding the signature itself.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        signature_preimage(b"iroha:sumeragi:v2:proposal", &unsigned.encode())
    }
    /// Validate the complete structural proposal contract against a frozen
    /// height context.
    ///
    /// Cryptographic verification remains the authenticated-ingress adapter's
    /// responsibility and must use [`Self::signature_preimage`].
    ///
    /// # Errors
    ///
    /// Returns a structural validation error when the proposal, manifest,
    /// leader, parent/timeout justification, or signature is not valid under
    /// the frozen height context.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        self.manifest.validate(context)?;
        if self.manifest.round != self.round || self.manifest.subject != self.subject {
            return Err(ValidationError::ProposalManifestMismatch);
        }
        validate_validator_index(self.proposer, context)?;
        if self.proposer != context.leader(self.round.view) {
            return Err(ValidationError::WrongProposer);
        }
        match &self.justification {
            ProposalJustification::ParentCommit(parent) => {
                let same_finalized_parent = match (
                    parent.certificate.as_ref(),
                    context.parent_commit_qc.as_ref(),
                ) {
                    (None, None) => true,
                    (Some(carried), Some(frozen)) => {
                        // A subject can acquire valid CommitQCs in more than
                        // one same-round certificate before or after an
                        // unchanged re-proposal. Context identity deliberately
                        // ignores that round and the signer evidence, so
                        // view-zero admission uses the semantic decision key.
                        // The previous roster is unavailable here, but all
                        // context-independent certificate shape checks remain
                        // mandatory before authenticated ingress verifies it.
                        carried.proposal_round == carried.round
                            && carried.round.height.checked_add(1) == Some(context.height)
                            && carried.execution_commitment.validate().is_ok()
                            && !carried.signers.is_empty()
                            && carried.signers.len() <= MAX_VALIDATORS_PER_HEIGHT
                            && carried.signers.windows(2).all(|pair| pair[0] < pair[1])
                            && require_aggregate_signature(&carried.aggregate_signature).is_ok()
                            && carried.as_ref().same_commit_decision(frozen.as_ref())
                    }
                    (None, Some(_)) | (Some(_), None) => false,
                };
                if self.round.view != 0 || !same_finalized_parent {
                    return Err(ValidationError::InvalidProposalJustification);
                }
            }
            ProposalJustification::Timeout(timeout) => {
                if self.round.view == 0
                    || timeout.timeout_certificate.round.context_id != self.round.context_id
                    || timeout.timeout_certificate.round.height != self.round.height
                    || timeout.timeout_certificate.round.view.checked_add(1)
                        != Some(self.round.view)
                {
                    return Err(ValidationError::InvalidProposalJustification);
                }
                timeout.timeout_certificate.validate(context)?;
                let selected_highest = timeout.timeout_certificate.highest_prepare_qc();
                if selected_highest != timeout.highest_prepare_qc.as_ref()
                    || selected_highest.is_some_and(|highest| highest.subject != self.subject)
                {
                    return Err(ValidationError::InvalidProposalJustification);
                }
            }
        }
        require_signature(&self.signature)
    }
}
/// Exact pair of individually authenticated Sumeragi v2 messages proving one
/// validator signed conflicting statements for the same consensus slot.
///
/// The complete signed messages are retained instead of an offender/round
/// summary so evidence consumers can independently re-run context, roster, and
/// signature checks before applying a penalty. Pair order is not semantic;
/// persistence code canonicalizes it by canonical Norito bytes.
#[expect(
    clippy::large_enum_variant,
    reason = "equivocation evidence retains both complete signed artifacts inline; boxing a variant would change the canonical Norito V1 wire shape"
)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "kind",
    content = "artifacts",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SumeragiV2Equivocation {
    /// Two different leader proposals for one round.
    Proposal {
        /// First authenticated proposal.
        first: Proposal,
        /// Second authenticated proposal.
        second: Proposal,
    },
    /// Two different subjects voted for by one signer in one phase and round.
    PhaseVote {
        /// First authenticated phase vote.
        first: Vote,
        /// Second authenticated phase vote.
        second: Vote,
    },
    /// Two different highest-PrepareQC claims signed for one timed-out round.
    TimeoutVote {
        /// First authenticated timeout vote.
        first: TimeoutVote,
        /// Second authenticated timeout vote.
        second: TimeoutVote,
    },
}
/// Schema projection for
/// [`SumeragiV2Equivocation::Proposal`]'s exact signed pair.
#[derive(IntoSchema)]
pub struct SumeragiV2ProposalEquivocationSchema {
    /// First authenticated proposal.
    pub first: Proposal,
    /// Conflicting authenticated proposal.
    pub second: Proposal,
}
/// Schema projection for
/// [`SumeragiV2Equivocation::PhaseVote`]'s exact signed pair.
#[derive(IntoSchema)]
pub struct SumeragiV2PhaseVoteEquivocationSchema {
    /// First authenticated phase vote.
    pub first: Vote,
    /// Conflicting authenticated phase vote.
    pub second: Vote,
}
/// Schema projection for
/// [`SumeragiV2Equivocation::TimeoutVote`]'s exact signed pair.
#[derive(IntoSchema)]
pub struct SumeragiV2TimeoutVoteEquivocationSchema {
    /// First authenticated timeout vote.
    pub first: TimeoutVote,
    /// Conflicting authenticated timeout vote.
    pub second: TimeoutVote,
}
impl TypeId for SumeragiV2Equivocation {
    fn id() -> Ident {
        "SumeragiV2Equivocation".to_owned()
    }
}
impl IntoSchema for SumeragiV2Equivocation {
    fn type_name() -> Ident {
        "SumeragiV2Equivocation".to_owned()
    }
    fn update_schema_map(metamap: &mut MetaMap) {
        if metamap.contains_key::<Self>() {
            return;
        }
        SumeragiV2ProposalEquivocationSchema::update_schema_map(metamap);
        SumeragiV2PhaseVoteEquivocationSchema::update_schema_map(metamap);
        SumeragiV2TimeoutVoteEquivocationSchema::update_schema_map(metamap);
        metamap.insert::<Self>(Metadata::Enum(EnumMeta {
            variants: vec![
                EnumVariant {
                    tag: "proposal".to_owned(),
                    discriminant: 0,
                    ty: Some(core::any::TypeId::of::<SumeragiV2ProposalEquivocationSchema>()),
                },
                EnumVariant {
                    tag: "phase_vote".to_owned(),
                    discriminant: 1,
                    ty: Some(core::any::TypeId::of::<SumeragiV2PhaseVoteEquivocationSchema>()),
                },
                EnumVariant {
                    tag: "timeout_vote".to_owned(),
                    discriminant: 2,
                    ty: Some(core::any::TypeId::of::<
                        SumeragiV2TimeoutVoteEquivocationSchema,
                    >()),
                },
            ],
        }));
    }
}
/// Authenticated request for a body covered by a `PrepareQC` or `CommitQC`.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct CertifiedBodyRequest {
    /// Round in which the body was proposed.
    pub round: ConsensusRound,
    /// Requested subject.
    pub subject: BlockSubject,
    /// Same-round certificate proving that validators should retain the body.
    pub certificate: QuorumCertificate,
    /// Authenticated requester identity. Observers are allowed to fetch a
    /// certified body even though they are absent from the voting roster.
    pub requester: PeerId,
    /// Requester signature over the canonical request preimage.
    pub signature: Vec<u8>,
}
impl CertifiedBodyRequest {
    /// Return the canonical request bytes authenticated by the requester.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        signature_preimage(
            b"iroha:sumeragi:v2:certified-body-request",
            &unsigned.encode(),
        )
    }
    /// Validate context, certificate, requester, and signature presence.
    ///
    /// # Errors
    ///
    /// Returns a structural or quorum error when the requested round or
    /// certificate is invalid under `context`, the certificate identifies a
    /// different proposal, or the requester signature is missing or oversized.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        self.certificate.validate(context)?;
        if self.certificate.proposal_round != self.round || self.certificate.subject != self.subject
        {
            return Err(ValidationError::CertifiedBodyCertificateMismatch);
        }
        require_signature(&self.signature)
    }
}
/// Authenticated response carrying a certified body and its manifest.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct CertifiedBodyResponse {
    /// Hash of the exact request being answered.
    pub request_hash: HashOf<CertifiedBodyRequest>,
    /// Manifest committing to the returned body.
    pub manifest: PayloadManifest,
    /// Canonical full payload bytes.
    pub body: Vec<u8>,
    /// Responder index in the height context roster.
    pub responder: ValidatorIndex,
    /// Responder signature over the request hash, manifest, and body hash.
    pub signature: Vec<u8>,
}
impl CertifiedBodyResponse {
    /// Return the canonical response bytes authenticated by the responder.
    ///
    /// The body is represented by its payload hash in the signed payload so
    /// implementations need not duplicate large bytes during signing.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let payload = CertifiedBodyResponseSignaturePayload {
            protocol_version: PROTOCOL_VERSION,
            request_hash: self.request_hash,
            manifest: self.manifest.clone(),
            body_hash: Hash::new(&self.body),
            responder: self.responder,
        };
        signature_preimage(
            b"iroha:sumeragi:v2:certified-body-response",
            &payload.encode(),
        )
    }
    /// Validate the response against the frozen context and signature
    /// presence. The caller additionally matches `request_hash` to an
    /// outstanding authenticated request.
    ///
    /// # Errors
    ///
    /// Returns a structural validation error when the manifest is invalid,
    /// the body hash or length differs from the manifest, the responder is
    /// outside the frozen roster, or the response signature is missing or
    /// oversized.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        self.manifest.validate(context)?;
        if Hash::new(&self.body) != self.manifest.subject.payload_hash {
            return Err(ValidationError::CertifiedBodyHashMismatch);
        }
        if u64::try_from(self.body.len()).ok() != Some(self.manifest.payload_size_bytes) {
            return Err(ValidationError::PayloadSizeMismatch);
        }
        validate_validator_index(self.responder, context)?;
        require_signature(&self.signature)
    }
    /// Validate this response against the exact outstanding request and the
    /// authenticated outer transport sender.
    ///
    /// # Errors
    ///
    /// Returns an error when the response is replayed across requests, changes
    /// round/subject, or the claimed frozen-roster responder differs from the
    /// authenticated transport sender.
    ///
    /// The responder need not be one of the request QC signers. Historical
    /// archive service is safe because the exact request carries the verified
    /// QC while the response body and manifest are hash-bound to that QC's
    /// subject. The serving path additionally proves that the frozen-roster
    /// peer has the canonical applied block in durable storage.
    pub fn validate_against(
        &self,
        context: &HeightContext,
        request: &CertifiedBodyRequest,
        authenticated_sender: &PeerId,
    ) -> Result<(), ValidationError> {
        request.validate(context)?;
        self.validate(context)?;
        if self.request_hash != HashOf::new(request)
            || self.manifest.round != request.round
            || self.manifest.subject != request.subject
        {
            return Err(ValidationError::CertifiedBodyRequestMismatch);
        }
        let responder = context
            .roster
            .get(usize::try_from(self.responder).map_err(|_| ValidationError::SignerOutOfRange)?)
            .ok_or(ValidationError::SignerOutOfRange)?;
        if &responder.validator != authenticated_sender {
            return Err(ValidationError::ResponderIdentityMismatch);
        }
        Ok(())
    }
}
/// Canonical fields authenticated by a certified-body response signature.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct CertifiedBodyResponseSignaturePayload {
    /// Sumeragi protocol revision.
    pub protocol_version: u16,
    /// Exact certified request being answered.
    pub request_hash: HashOf<CertifiedBodyRequest>,
    /// Manifest committing to the complete body.
    pub manifest: PayloadManifest,
    /// Hash of the returned canonical body bytes.
    pub body_hash: Hash,
    /// Responder index in the frozen roster.
    pub responder: ValidatorIndex,
}
/// Authenticated request for the durable `CommitQC` of one exact height context.
///
/// A lagging peer already reconstructs its next immutable [`HeightContext`]
/// from the preceding committed block.  This request deliberately names only
/// that context: responders cannot skip heights or substitute a certificate
/// governed by another roster.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct CommitCertificateRequest {
    /// Consensus protocol revision included in the signed request.
    pub protocol_version: u16,
    /// Exact genesis-derived network identity included for replay rejection at ingress.
    pub network_id: NetworkId,
    /// Exact frozen context whose durable `CommitQC` is requested.
    pub context_id: HeightContextId,
    /// Height repeated for bounded serving and early rejection.
    pub height: Height,
    /// Authenticated peer requesting the certificate. Observers may catch up.
    pub requester: PeerId,
    /// Requester signature over the canonical request preimage.
    pub signature: Vec<u8>,
}
impl CommitCertificateRequest {
    /// Return the canonical bytes authenticated by the requester.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let mut unsigned = self.clone();
        unsigned.signature.clear();
        signature_preimage(
            b"iroha:sumeragi:v2:commit-certificate-request",
            &unsigned.encode(),
        )
    }
    /// Validate the request against the one active frozen context.
    ///
    /// Cryptographic signature and outer-transport identity verification are
    /// performed by the transport adapter.
    ///
    /// # Errors
    ///
    /// Returns an error when the context itself is invalid, the request uses
    /// another protocol, chain, context, or height, or its signature is missing
    /// or oversized.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        context.validate()?;
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ValidationError::UnsupportedProtocolVersion {
                expected: PROTOCOL_VERSION,
                actual: self.protocol_version,
            });
        }
        if self.network_id != context.network_id
            || self.context_id != context.id()
            || self.height != context.height
        {
            return Err(ValidationError::WrongHeightContext);
        }
        require_signature(&self.signature)
    }
}
/// Authenticated response carrying the `CommitQC` for an exact outstanding
/// [`CommitCertificateRequest`].
///
/// The response never carries a block body.  Its certificate is admitted as a
/// normal v2 `CommitQC` through the authoritative reducer; the reducer then
/// initiates the existing certified-body fetch and WAL/apply sequence.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct CommitCertificateResponse {
    /// Hash of the exact signed request being answered.
    pub request_hash: HashOf<CommitCertificateRequest>,
    /// Durable `CommitQC` recovered from the responder's canonical finality artifact.
    pub certificate: QuorumCertificate,
    /// Current authenticated network identity serving the durable artifact.
    ///
    /// This need not be an identity in the historical height roster: validator
    /// key rotation must not make old canonical finality artifacts unservable.
    pub responder: PeerId,
    /// Responder signature over the request hash and exact certificate.
    pub signature: Vec<u8>,
}
impl CommitCertificateResponse {
    /// Return the canonical bytes authenticated by the responder.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let payload = CommitCertificateResponseSignaturePayload {
            protocol_version: PROTOCOL_VERSION,
            request_hash: self.request_hash,
            certificate: self.certificate.clone(),
            responder: self.responder.clone(),
        };
        signature_preimage(
            b"iroha:sumeragi:v2:commit-certificate-response",
            &payload.encode(),
        )
    }
    /// Validate the certificate and response structure against one context.
    ///
    /// Cryptographic aggregate and responder signatures are verified by the
    /// transport and consensus adapters respectively.
    ///
    /// # Errors
    ///
    /// Returns a structural or quorum error when the certificate is invalid,
    /// is not a `CommitQC` for `context`, or the response signature is missing
    /// or oversized.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        self.certificate.validate(context)?;
        if self.certificate.phase != GlobalPhase::Commit
            || self.certificate.round.context_id != context.id()
            || self.certificate.round.height != context.height
        {
            return Err(ValidationError::CommitCertificateMismatch);
        }
        require_signature(&self.signature)
    }
    /// Validate the response against the exact outstanding request.
    ///
    /// # Errors
    ///
    /// Returns an error when either artifact is invalid under `context` or the
    /// response does not carry the hash of the exact signed request.
    pub fn validate_against(
        &self,
        context: &HeightContext,
        request: &CommitCertificateRequest,
    ) -> Result<(), ValidationError> {
        request.validate(context)?;
        self.validate(context)?;
        if self.request_hash != HashOf::new(request) {
            return Err(ValidationError::CommitCertificateRequestMismatch);
        }
        Ok(())
    }
}
/// Canonical fields authenticated by a commit-certificate response.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct CommitCertificateResponseSignaturePayload {
    /// Sumeragi protocol revision.
    pub protocol_version: u16,
    /// Exact signed request being answered.
    pub request_hash: HashOf<CommitCertificateRequest>,
    /// Exact `CommitQC` supplied to the authoritative reducer.
    pub certificate: QuorumCertificate,
    /// Current authenticated network identity serving the artifact.
    pub responder: PeerId,
}
/// Authenticated `NPoS` randomness commitment for one frozen epoch roster.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct VrfCommit {
    /// Epoch index to which the commitment applies.
    pub epoch: u64,
    /// Hiding commitment to the validator's reveal.
    pub commitment: [u8; 32],
    /// Signer index in the immutable height-context roster.
    pub signer: ValidatorIndex,
    /// Signature over the canonical `NPoS` `VRF`-commit preimage.
    pub bls_sig: Vec<u8>,
}
/// Authenticated `NPoS` randomness reveal for one frozen epoch roster.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct VrfReveal {
    /// Epoch index to which the reveal applies.
    pub epoch: u64,
    /// Revealed preimage whose hash must equal the prior commitment.
    pub reveal: [u8; 32],
    /// Signer index in the immutable height-context roster.
    pub signer: ValidatorIndex,
    /// Canonical Norito-encoded VRF proof whose verified output equals `reveal`.
    pub vrf_proof: Vec<u8>,
    /// Signature over the canonical `NPoS` `VRF`-reveal preimage.
    pub bls_sig: Vec<u8>,
}
/// One adaptive threshold-beacon signature share for an exact consensus round.
///
/// The outer authenticated transport sender must be the validator occupying
/// the one-based DKG seat named by [`GlobalThresholdBeaconPartialSignatureV1::signer_index`].
/// The representation proof inside `partial` additionally binds the share to
/// the active public DKG session and the fixed pulse-slot payload reconstructed
/// from the height and finalized parent anchor. The outer consensus view only
/// routes retries; it is not included in the threshold-signed payload.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct GlobalBeaconPartialSignature {
    /// Exact active height context and view whose candidate will carry the pulse.
    pub round: ConsensusRound,
    /// Proof-carrying adaptive threshold-BLS signature share.
    pub partial: GlobalThresholdBeaconPartialSignatureV1,
}

impl GlobalBeaconPartialSignature {
    /// Validate the round and one-based DKG signer seat against a frozen context.
    ///
    /// Cryptographic proof verification is deliberately performed by the
    /// threshold-beacon reducer after it reconstructs the exact pulse payload.
    ///
    /// # Errors
    ///
    /// Returns a structural validation error for another height context or an
    /// out-of-range signer seat.
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        let zero_based = self
            .partial
            .signer_index
            .checked_sub(1)
            .ok_or(ValidationError::SignerOutOfRange)?;
        if usize::from(zero_based) >= context.roster.len() {
            return Err(ValidationError::SignerOutOfRange);
        }
        Ok(())
    }
}
/// Payload variants accepted by the Sumeragi v2 network envelope.
#[expect(
    clippy::large_enum_variant,
    reason = "consensus variants retain their canonical V1 Norito payloads inline; introducing indirection would change the signed wire representation"
)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "kind",
    content = "message",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ConsensusMessageV2Payload {
    /// Leader proposal.
    Proposal(Proposal),
    /// Prepare or Commit vote.
    Vote(Vote),
    /// Aggregate `PrepareQC` or `CommitQC`.
    QuorumCertificate(QuorumCertificate),
    /// Individual timeout vote.
    TimeoutVote(TimeoutVote),
    /// Aggregate timeout certificate.
    TimeoutCertificate(TimeoutCertificate),
    /// Payload manifest announcement or retransmission.
    PayloadManifest(PayloadManifest),
    /// Encoded payload chunk.
    PayloadChunk(PayloadChunk),
    /// Request for a certified body.
    CertifiedBodyRequest(CertifiedBodyRequest),
    /// Response carrying a certified body.
    CertifiedBodyResponse(CertifiedBodyResponse),
    /// Request the durable `CommitQC` for the active height context.
    CommitCertificateRequest(CommitCertificateRequest),
    /// Response carrying the active height context's durable `CommitQC`.
    CommitCertificateResponse(CommitCertificateResponse),
    /// `NPoS` epoch-randomness commitment.
    VrfCommit(VrfCommit),
    /// `NPoS` epoch-randomness reveal.
    VrfReveal(VrfReveal),
    /// Adaptive global threshold-beacon share for one exact height and view.
    GlobalBeaconPartialSignature(GlobalBeaconPartialSignature),
}
/// Explicitly versioned Sumeragi v2 network envelope.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct ConsensusMessageV2 {
    /// Protocol version; must equal [`PROTOCOL_VERSION`].
    pub protocol_version: u16,
    /// Canonical v2 message payload.
    pub payload: ConsensusMessageV2Payload,
}
/// High-level reducer phase exported by the compact Sumeragi v2 status API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "phase",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SumeragiV2StatusPhase {
    /// Waiting for the expected leader's proposal.
    AwaitingProposal,
    /// Reconstructing a payload from authenticated chunks.
    ReconstructingPayload,
    /// Running deterministic block validation.
    ValidatingPayload,
    /// Collecting or processing Prepare votes.
    Prepare,
    /// Collecting or processing Commit votes.
    Commit,
    /// A `CommitQC` is durable and the body is awaiting application.
    PendingApply,
}
/// Local availability/application state for the current proposal body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "state",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SumeragiV2BodyState {
    /// No manifest or body is held locally.
    Missing,
    /// Authenticated chunks are being reconstructed.
    Reconstructing,
    /// The exact body is durably stored but not yet validated.
    Stored,
    /// The durably stored body passed deterministic validation.
    Validated,
    /// The decided body is waiting for state application.
    PendingApply,
    /// The decided body has been applied locally.
    Applied,
}
/// Frozen election and equal-vote quorum inputs governing the active status height.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiV2HeightContextStatus {
    /// Finalized validator-election epoch.
    pub epoch: u64,
    /// Last height governed by this epoch's frozen election snapshot.
    pub epoch_end_height: Height,
    /// Consensus mode which selected the equal-vote committee.
    pub mode: ConsensusMode,
    /// Finalized seed used to select the view-zero leader.
    pub epoch_seed: [u8; 32],
    /// Number of voting validators in the frozen roster.
    pub validator_count: u32,
    /// Canonical `2f + 1` quorum derived from the frozen `3f + 1` roster.
    pub quorum: DualQuorum,
}
/// Equal-vote summary of the latest authenticated durable `CommitQC`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiV2CommitQcStatus {
    /// Stable reference to the exact durable `CommitQC`.
    pub certificate: QuorumCertificateRef,
    /// Number of validators in the certificate's frozen roster.
    pub validator_count: u32,
    /// Number of distinct certificate signers.
    pub signer_count: u32,
    /// Canonical strict-supermajority signer threshold.
    pub min_signers: u32,
    /// Redundant vote total; equal to `signer_count` in protocol v4.
    pub signed_power: u64,
    /// Redundant roster vote total; equal to `validator_count` in protocol v4.
    pub total_power: u64,
}
/// Reducer generation owning volatile Sumeragi v2 consumer state.
///
/// The generation advances whenever a transition, such as timeout-certificate
/// installation, replaces vote pools or asynchronous completion ownership.
pub type SumeragiV2Generation = u64;
/// Partial equal-vote quorum state for one exact voting round and proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiV2VoteQuorumStatus {
    /// Exact height-context round whose vote pool is summarized.
    pub round: ConsensusRound,
    /// Immutable proposal-body origin authenticated by every vote in the pool.
    pub proposal_round: ConsensusRound,
    /// Exact proposal subject accepted into the pool.
    pub subject: BlockSubject,
    /// Deterministic execution result authenticated by the votes.
    pub execution_commitment: ExecutionCommitment,
    /// Number of distinct authenticated voting validators in the pool.
    pub signer_count: u32,
    /// Redundant unit-vote projection equal to `signer_count`.
    pub signed_power: u64,
    /// Required number of distinct voting validators.
    pub min_signers: u32,
    /// Redundant unit-vote projection equal to the frozen roster length.
    pub total_power: u64,
}
/// Partial timeout quorum state for one exact round.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiV2TimeoutQuorumStatus {
    /// Exact round whose timeout votes are summarized.
    pub round: ConsensusRound,
    /// Number of distinct authenticated voting validators in the pool.
    pub signer_count: u32,
    /// Redundant unit-vote projection equal to `signer_count`.
    pub signed_power: u64,
    /// Required number of distinct voting validators.
    pub min_signers: u32,
    /// Redundant unit-vote projection equal to the frozen roster length.
    pub total_power: u64,
    /// Whether the partial pool has produced a verified timeout certificate.
    pub certificate_formed: bool,
}
/// Durable protocol intent retained for fair outbound service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "kind",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SumeragiV2OutboundIntentKind {
    /// Leader proposal intent.
    Proposal,
    /// Prepare vote intent.
    PrepareVote,
    /// Commit vote intent.
    CommitVote,
    /// Formed `PrepareQC` intent.
    PrepareQc,
    /// Formed `CommitQC` intent.
    CommitQc,
    /// Timeout vote intent.
    TimeoutVote,
    /// Formed timeout-certificate intent.
    TimeoutCertificate,
}
/// Current lifecycle stage of a durable outbound intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "stage",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SumeragiV2OutboundIntentStage {
    /// The intent is fenced behind a safety-WAL append.
    PendingPersistence,
    /// Durable state is waiting for a local signature.
    PendingSignature,
    /// A signed intent is owned by a reserved outbound queue.
    Queued,
    /// The intent has been broadcast and remains eligible for retransmission.
    Sent,
}
/// Exact durable outbound intent visible to liveness diagnostics.
///
/// All three optional evidence slots are required on the JSON wire. Intent
/// shape determines whether their explicit value is `null`; omission is not a
/// first-release representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiV2OutboundIntentStatus {
    /// Protocol role of the retained intent.
    pub kind: SumeragiV2OutboundIntentKind,
    /// Exact round to which the intent belongs.
    pub round: ConsensusRound,
    /// Immutable proposal-body origin for proposal-authenticating intents.
    /// Timeout votes and timeout certificates carry `None`.
    #[norito(required)]
    pub proposal_round: Option<ConsensusRound>,
    /// Proposal subject, when the intent authenticates one.
    #[norito(required)]
    pub subject: Option<BlockSubject>,
    /// Execution result, when the intent authenticates one.
    #[norito(required)]
    pub execution_commitment: Option<ExecutionCommitment>,
    /// Current durable-delivery stage.
    pub stage: SumeragiV2OutboundIntentStage,
}
/// State of one terminating local-work stage.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "stage",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SumeragiV2LocalWorkStage {
    /// No work is required for the active height.
    #[default]
    Idle,
    /// Work is durably scheduled but has not begun local execution.
    Queued,
    /// The local operation is executing.
    Running,
    /// The stage completed for the active proposal or decision.
    Complete,
}
/// Local body, validation, application, and handoff pipeline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiV2WorkStatus {
    /// Local candidate construction or proposal-admission work.
    pub candidate: SumeragiV2LocalWorkStage,
    /// Certified-body fetch or reconstruction work.
    pub body_recovery: SumeragiV2LocalWorkStage,
    /// Durable body-store work.
    pub body_store: SumeragiV2LocalWorkStage,
    /// Deterministic candidate-validation work.
    pub validation: SumeragiV2LocalWorkStage,
    /// Durable decision application work.
    pub application: SumeragiV2LocalWorkStage,
    /// Activation of the successor height after application.
    pub successor_height: SumeragiV2LocalWorkStage,
}
/// Bounded queue contributing to Sumeragi v2 progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "queue",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SumeragiV2QueueKind {
    /// Authenticated semantic-admission/equivocation table.
    Ingress,
    /// Adapter lane for ordinary protocol traffic.
    DeferredNormal,
    /// Adapter lane reserved for progress-relevant protocol traffic.
    DeferredProgress,
    /// Adapter lane reserved for asynchronous completions.
    DeferredCompletion,
    /// Serialized runtime lane for ordinary protocol traffic.
    RuntimeNormal,
    /// Serialized runtime lane reserved for progress-relevant traffic.
    RuntimeProgress,
    /// Serialized runtime lane reserved for asynchronous completions.
    RuntimeCompletion,
    /// Effect-executor completion queue.
    EffectCompletion,
    /// Bounded transport-to-runner network ingress.
    NetworkIngress,
    /// Reducer-to-effect dispatch suffix retained for pending-work capacity.
    ///
    /// This reserved FIFO runs before another reducer transition. Capacity
    /// retries are therefore not scheduler-skip debt.
    EffectDispatch,
}
/// Occupancy and fairness state for one bounded local queue.
///
/// `oldest_age_ms` is an explicit nullable JSON slot: an empty queue reports
/// `null` and never omits the current field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiV2QueueStatus {
    /// Queue being summarized.
    pub queue: SumeragiV2QueueKind,
    /// Number of currently owned items.
    pub depth: u32,
    /// Maximum number of owned items.
    pub capacity: u32,
    /// Age of the oldest owned item, in local monotonic milliseconds.
    #[norito(required)]
    pub oldest_age_ms: Option<u64>,
    /// Saturating count of eligible dispatches skipped by the oldest item.
    pub service_debt: u64,
}
/// Reducer transition retained for diagnostic transition age.
///
/// A transition resets `no_progress_age_ms` only when it advances the bounded
/// height-wide semantic high-water. Timeout traffic and reconstruction of an
/// already observed partial vote pool remain visible here without refreshing
/// that clock, so repeated view churn cannot mask a stall.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "transition",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SumeragiV2ProgressTransition {
    /// A leader proposal entered the reducer.
    ProposalAdmitted,
    /// The exact proposal body became locally available.
    BodyAvailable,
    /// The exact proposal body became durable.
    BodyStored,
    /// Deterministic candidate validation completed.
    BodyValidated,
    /// An authenticated Prepare vote increased an exact partial pool.
    PrepareVoteAdmitted,
    /// An authenticated Commit vote increased an exact partial pool.
    CommitVoteAdmitted,
    /// An authenticated timeout vote increased an exact partial pool.
    TimeoutVoteAdmitted,
    /// A Prepare equal-vote quorum formed or arrived.
    PrepareQuorum,
    /// A `PrepareQC` lock became durable.
    LockInstalled,
    /// A Commit equal-vote quorum formed or arrived.
    CommitQuorum,
    /// A timeout certificate installed a successor view.
    TimeoutCertificateInstalled,
    /// The exact `CommitQC` decision became durable.
    DecisionPersisted,
    /// The decided block was applied locally.
    Applied,
    /// This height became active after its predecessor applied, its live
    /// clocks were armed, and authenticated ingress opened.
    SuccessorHeightActivated,
    /// WAL recovery reconstructed a pending progress path.
    RecoveryReplayed,
}
/// Last tracked reducer transition and its local age.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiV2ProgressTransitionStatus {
    /// Reducer generation which emitted the transition.
    pub generation: SumeragiV2Generation,
    /// Exact round active at the transition.
    pub round: ConsensusRound,
    /// Semantic reducer event retained for diagnostics.
    pub transition: SumeragiV2ProgressTransition,
    /// Local monotonic milliseconds elapsed since the transition.
    pub age_ms: u64,
}
/// Classified cause of an active no-progress interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "blocker",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SumeragiV2LivenessBlocker {
    /// The current view has not admitted its expected proposal.
    MissingProposal,
    /// A certified or locked proposal body is unavailable locally.
    BodyUnavailable,
    /// The exact Prepare pool lacks the required `2f + 1` distinct votes.
    PrepareQuorumMissing,
    /// The exact Commit pool lacks the required `2f + 1` distinct votes.
    CommitQuorumMissing,
    /// Timeout votes have not produced the required timeout certificate.
    TimeoutCertificateMissing,
    /// Reserved local work is not reducing its service debt.
    SchedulerStarvation,
    /// A durable decision is waiting for terminating local application work.
    ApplicationPending,
    /// Durable application completed but successor activation has not advanced.
    SuccessorActivationPending,
    /// The reducer is waiting for safety-WAL persistence or consensus signing.
    LocalControlPending,
}
/// Closed reducer reason for safely ignoring an input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "reason",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SumeragiV2IgnoreReason {
    /// Input belongs to another height.
    WrongHeight,
    /// Input was tagged for a different current view.
    WrongView,
    /// Completion belongs to an old local generation.
    StaleGeneration,
    /// The reducer is waiting for persistence or signing.
    Busy,
    /// The message or completion has already been handled.
    Duplicate,
    /// No outstanding body operation matches the completion.
    NoMatchingWork,
    /// The node is an observer and cannot vote or time out.
    Observer,
    /// A durable timeout intent closed the view to new votes.
    ViewClosed,
    /// A finalized decision makes the input irrelevant.
    AlreadyDecided,
    /// WAL replay awaits its one authorized resumption event.
    RecoveryPending,
    /// The input's round or safe-value rank cannot affect local state.
    IrrelevantView,
    /// A durable lock makes the proposal's subject unsafe to prepare.
    UnsafeProposal,
}
/// Per-height counter for one closed input-ignore reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiV2IgnoreCount {
    /// Reason whose occurrences are counted.
    pub reason: SumeragiV2IgnoreReason,
    /// Number of occurrences at the active height.
    pub count: u64,
}
/// Authoritative progress diagnostics for the active Sumeragi v2 height.
///
/// Local ages and queue measurements are observation-only: they are never
/// inputs to protocol transitions, wire fingerprints, or deterministic state.
/// A lagging node may report a later-view `CommitQC` intent or
/// Commit-quorum/decision transition for this exact active height; all other
/// diagnostics remain bounded by the status snapshot's current view.
/// Nullable diagnostic slots are always present in JSON and use explicit
/// `null` when no transition or blocker exists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiV2LivenessStatus {
    /// Reducer generation which owns all reported volatile state.
    pub generation: SumeragiV2Generation,
    /// Partial Prepare pools keyed by exact round, subject, and execution result.
    pub prepare_quorums: Vec<SumeragiV2VoteQuorumStatus>,
    /// Partial Commit pools keyed by exact round, subject, and execution result.
    pub commit_quorums: Vec<SumeragiV2VoteQuorumStatus>,
    /// Partial timeout pools keyed by exact round.
    pub timeout_quorums: Vec<SumeragiV2TimeoutQuorumStatus>,
    /// Durable progress-relevant intents owned by outbound service.
    pub outbound_intents: Vec<SumeragiV2OutboundIntentStatus>,
    /// Current local terminating-work stages.
    pub work: SumeragiV2WorkStatus,
    /// Bounded queue occupancy and service debt.
    pub queues: Vec<SumeragiV2QueueStatus>,
    /// Most recent tracked reducer transition.
    #[norito(required)]
    pub last_progress: Option<SumeragiV2ProgressTransitionStatus>,
    /// Local monotonic milliseconds without meaningful height progress.
    pub no_progress_age_ms: u64,
    /// Classified delay cause after the watchdog threshold is crossed.
    #[norito(required)]
    pub blocker: Option<SumeragiV2LivenessBlocker>,
    /// Per-height counters for every observed reducer ignore reason.
    pub ignore_counts: Vec<SumeragiV2IgnoreCount>,
}
/// Canonical response returned by `GET /v1/sumeragi/qc`.
///
/// Both nullable slots are required on the wire so peers never infer missing
/// fields as `None`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiV2QcResponse {
    /// Highest verified `PrepareQC` known to the reducer.
    #[norito(required)]
    pub highest_prepare_qc: Option<QuorumCertificateRef>,
    /// Persisted `PrepareQC` lock, if any.
    #[norito(required)]
    pub locked_prepare_qc: Option<QuorumCertificateRef>,
}
/// Compact Norito payload returned by the Sumeragi v2 status endpoint.
///
/// Every field belongs to the first-release JSON schema. Nullable consensus
/// evidence is encoded as an explicit value or `null`; missing fields are
/// rejected rather than interpreted as compatibility defaults.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiV2Status {
    /// Active wire protocol version.
    pub protocol_version: u16,
    /// Fingerprint of the node's consensus identity.
    pub node_fingerprint: Hash,
    /// Fingerprint of the running binary build.
    pub build_fingerprint: Hash,
    /// Fingerprint of all consensus-relevant configuration.
    pub config_fingerprint: Hash,
    /// Whether consensus has fail-stopped and requires a process restart.
    pub restart_required: bool,
    /// Immutable context governing the reported height.
    pub height_context_id: HeightContextId,
    /// Current persisted consensus height.
    pub height: Height,
    /// Current persisted view.
    pub view: View,
    /// Current reducer phase.
    pub phase: SumeragiV2StatusPhase,
    /// Expected leader index for the current view.
    pub leader: ValidatorIndex,
    /// Persisted `PrepareQC` lock, if any.
    #[norito(required)]
    pub locked_prepare_qc: Option<QuorumCertificateRef>,
    /// Highest verified `PrepareQC` known locally, if any.
    #[norito(required)]
    pub highest_prepare_qc: Option<QuorumCertificateRef>,
    /// Most recently installed timeout certificate, including its view.
    #[norito(required)]
    pub last_timeout_certificate: Option<TimeoutCertificateRef>,
    /// Local body availability/application state.
    pub body_state: SumeragiV2BodyState,
    /// WAL persistence operation blocking the reducer, if any.
    #[norito(required)]
    pub pending_persistence_id: Option<u64>,
    /// Last locally committed block height.
    pub last_committed_height: Height,
    /// Last locally committed block subject, absent before the first commit.
    #[norito(required)]
    pub last_committed_subject: Option<BlockSubject>,
    /// Frozen election context governing the active height.
    pub height_context: SumeragiV2HeightContextStatus,
    /// Latest authenticated durable `CommitQC` summary, when its frozen roster is available.
    #[norito(required)]
    pub last_commit_qc: Option<SumeragiV2CommitQcStatus>,
    /// Authoritative progress and no-progress diagnostics for the active height.
    pub liveness: SumeragiV2LivenessStatus,
}
impl SumeragiV2Status {
    /// Validate scalar and cross-field invariants which do not require the
    /// frozen validator roster or signature-verification context.
    ///
    /// This deliberately does not authenticate certificate signatures. It
    /// only rejects a status snapshot which cannot have been emitted by the
    /// authoritative reducer.
    ///
    /// # Errors
    ///
    /// Returns a structural error for an unsupported version, impossible
    /// phase/body pairing, inconsistent commit frontier, or a QC/TC reference
    /// bound to another context, height, phase, or future view.
    #[expect(
        clippy::too_many_lines,
        reason = "the ordered status validator preserves stable first-error precedence across the complete public V1 diagnostics contract"
    )]
    pub fn validate(&self) -> Result<(), SumeragiV2StatusValidationError> {
        use SumeragiV2StatusValidationError as Error;
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(Error::UnsupportedProtocolVersion {
                expected: PROTOCOL_VERSION,
                actual: self.protocol_version,
            });
        }
        if self.height == 0 {
            return Err(Error::ZeroHeight);
        }
        if self.pending_persistence_id == Some(0) {
            return Err(Error::ZeroPersistenceId);
        }
        if self.height_context.epoch_end_height < self.height {
            return Err(Error::EpochEndsBeforeHeight);
        }
        if !usize::try_from(self.height_context.validator_count).is_ok_and(is_valid_committee_size)
        {
            return Err(Error::InvalidValidatorCount);
        }
        if self.leader >= self.height_context.validator_count {
            return Err(Error::LeaderOutOfRange);
        }
        if DualQuorum::count_threshold(self.height_context.validator_count)
            != Some(self.height_context.quorum.min_signers)
        {
            return Err(Error::InvalidHeightContextQuorum);
        }
        let validator_count = u64::from(self.height_context.validator_count);
        if self.height_context.quorum.total_power != validator_count {
            return Err(Error::InvalidHeightContextQuorum);
        }
        let phase_body_is_valid = matches!(
            (self.phase, self.body_state),
            (
                SumeragiV2StatusPhase::AwaitingProposal,
                SumeragiV2BodyState::Missing
            ) | (
                SumeragiV2StatusPhase::ReconstructingPayload,
                SumeragiV2BodyState::Reconstructing
            ) | (
                SumeragiV2StatusPhase::ValidatingPayload,
                SumeragiV2BodyState::Stored
            ) | (
                SumeragiV2StatusPhase::Prepare | SumeragiV2StatusPhase::Commit,
                SumeragiV2BodyState::Validated
            ) | (
                SumeragiV2StatusPhase::PendingApply,
                SumeragiV2BodyState::PendingApply | SumeragiV2BodyState::Applied
            )
        );
        if !phase_body_is_valid {
            return Err(Error::PhaseBodyMismatch);
        }
        match self.phase {
            SumeragiV2StatusPhase::Commit if self.locked_prepare_qc.is_none() => {
                return Err(Error::CommitWithoutLock);
            }
            SumeragiV2StatusPhase::Prepare if self.locked_prepare_qc.is_some() => {
                return Err(Error::PrepareWithLock);
            }
            _ => {}
        }
        if self.phase == SumeragiV2StatusPhase::PendingApply {
            if self.last_committed_height != self.height
                || self.last_committed_subject.is_none()
                || self.last_commit_qc.is_none()
            {
                return Err(Error::PendingApplyCommitMismatch);
            }
        } else if self.last_committed_height >= self.height {
            return Err(Error::CommittedHeightNotBehindActiveHeight);
        }
        if self.last_committed_height == 0
            && (self.last_committed_subject.is_some() || self.last_commit_qc.is_some())
        {
            return Err(Error::GenesisCommitCarriesSubject);
        }
        if self.last_committed_subject.is_some() != self.last_commit_qc.is_some() {
            return Err(Error::CommitFrontierAuthenticationMismatch);
        }
        if let Some(summary) = &self.last_commit_qc {
            let subject = self
                .last_committed_subject
                .ok_or(Error::CommitFrontierAuthenticationMismatch)?;
            if summary.certificate.phase != GlobalPhase::Commit
                || summary.certificate.round.height != self.last_committed_height
                || summary.certificate.proposal_round.context_id
                    != summary.certificate.round.context_id
                || summary.certificate.proposal_round != summary.certificate.round
                || summary.certificate.subject != subject
                || summary.certificate.execution_commitment.validate().is_err()
            {
                return Err(Error::CommitSummaryCertificateMismatch);
            }
            if self.last_committed_height == self.height
                && summary.certificate.round.context_id != self.height_context_id
            {
                return Err(Error::CommitSummaryCertificateMismatch);
            }
            let canonical_min_signers = DualQuorum::count_threshold(summary.validator_count);
            if !usize::try_from(summary.validator_count).is_ok_and(is_valid_committee_size)
                || canonical_min_signers != Some(summary.min_signers)
                || summary.signer_count != summary.min_signers
                || summary.signer_count > summary.validator_count
                || summary.total_power != u64::from(summary.validator_count)
                || summary.signed_power != u64::from(summary.signer_count)
            {
                return Err(Error::InvalidCommitSummaryQuorum);
            }
            if summary.certificate.round.context_id == self.height_context_id
                && (summary.validator_count != self.height_context.validator_count
                    || summary.min_signers != self.height_context.quorum.min_signers
                    || summary.total_power != self.height_context.quorum.total_power)
            {
                return Err(Error::CommitSummaryContextMismatch);
            }
        }
        let validate_prepare = |certificate: &QuorumCertificateRef| {
            if certificate.round.context_id != self.height_context_id {
                return Err(Error::CertificateContextMismatch);
            }
            if certificate.round.height != self.height {
                return Err(Error::CertificateHeightMismatch);
            }
            if certificate.phase != GlobalPhase::Prepare {
                return Err(Error::CertificatePhaseMismatch);
            }
            if certificate.proposal_round != certificate.round {
                return Err(Error::InvalidProposalRound);
            }
            if certificate.round.view > self.view {
                return Err(Error::CertificateFromFutureView);
            }
            Ok(())
        };
        if let Some(locked) = &self.locked_prepare_qc {
            validate_prepare(locked)?;
        }
        if let Some(highest) = &self.highest_prepare_qc {
            validate_prepare(highest)?;
        }
        match (&self.locked_prepare_qc, &self.highest_prepare_qc) {
            (Some(_), None) => return Err(Error::LockedCertificateWithoutHighest),
            (Some(locked), Some(highest)) if locked.round.view > highest.round.view => {
                return Err(Error::LockedCertificateAboveHighest);
            }
            (Some(locked), Some(highest))
                if locked.round.view == highest.round.view && locked != highest =>
            {
                return Err(Error::ConflictingCertificatesAtSameView);
            }
            _ => {}
        }
        if let Some(timeout) = &self.last_timeout_certificate {
            if timeout.round.context_id != self.height_context_id {
                return Err(Error::CertificateContextMismatch);
            }
            if timeout.round.height != self.height {
                return Err(Error::CertificateHeightMismatch);
            }
            if timeout.round.view >= self.view {
                return Err(Error::TimeoutNotBeforeCurrentView);
            }
            if let Some(highest) = &timeout.highest_prepare_qc {
                validate_prepare(highest)?;
                if highest.round.view > timeout.round.view {
                    return Err(Error::TimeoutCarriesFuturePrepare);
                }
            }
        }
        if self.liveness.prepare_quorums.len() > MAX_VALIDATORS_PER_HEIGHT
            || self.liveness.commit_quorums.len() > MAX_COMMIT_QUORUM_GROUPS_PER_HEIGHT
            || self.liveness.timeout_quorums.len() > MAX_VALIDATORS_PER_HEIGHT
            || self.liveness.outbound_intents.len() > 7
            || self.liveness.queues.len() > 10
            || self.liveness.ignore_counts.len() > MAX_LIVENESS_IGNORE_REASONS
        {
            return Err(Error::LivenessCollectionTooLarge);
        }
        let validate_round_binding = |round: ConsensusRound| {
            let belongs_to_active_context = round.context_id == self.height_context_id;
            let belongs_to_active_height = round.height == self.height;
            if !belongs_to_active_context || !belongs_to_active_height {
                return Err(Error::LivenessRoundMismatch);
            }
            Ok(())
        };
        let validate_nonfuture_round = |round: ConsensusRound| {
            validate_round_binding(round)?;
            if round.view > self.view {
                return Err(Error::LivenessRoundFromFutureView);
            }
            Ok(())
        };
        let validate_partial_quorum =
            |signer_count: u32, signed_power: u64, min_signers: u32, total_power: u64| {
                if min_signers != self.height_context.quorum.min_signers
                    || total_power != self.height_context.quorum.total_power
                    || signer_count > self.height_context.validator_count
                    || signed_power != u64::from(signer_count)
                {
                    return Err(Error::InvalidLivenessQuorum);
                }
                Ok(())
            };
        let validate_vote_quorum = |quorum: &SumeragiV2VoteQuorumStatus| {
            validate_nonfuture_round(quorum.round)?;
            validate_round_binding(quorum.proposal_round)?;
            if quorum.proposal_round != quorum.round {
                return Err(Error::InvalidProposalRound);
            }
            quorum
                .execution_commitment
                .validate()
                .map_err(|_| Error::InvalidLivenessExecutionCommitment)?;
            validate_partial_quorum(
                quorum.signer_count,
                quorum.signed_power,
                quorum.min_signers,
                quorum.total_power,
            )
        };
        for quorum in &self.liveness.prepare_quorums {
            validate_vote_quorum(quorum)?;
        }
        for quorum in &self.liveness.commit_quorums {
            validate_vote_quorum(quorum)?;
        }
        for quorum in &self.liveness.timeout_quorums {
            validate_nonfuture_round(quorum.round)?;
            validate_partial_quorum(
                quorum.signer_count,
                quorum.signed_power,
                quorum.min_signers,
                quorum.total_power,
            )?;
            if quorum.certificate_formed && quorum.signer_count < quorum.min_signers {
                return Err(Error::InvalidLivenessQuorum);
            }
        }
        for intent in &self.liveness.outbound_intents {
            validate_round_binding(intent.round)?;
            if intent.kind != SumeragiV2OutboundIntentKind::CommitQc
                && intent.round.view > self.view
            {
                return Err(Error::LivenessRoundFromFutureView);
            }
            let carries_execution = matches!(
                intent.kind,
                SumeragiV2OutboundIntentKind::PrepareVote
                    | SumeragiV2OutboundIntentKind::CommitVote
                    | SumeragiV2OutboundIntentKind::PrepareQc
                    | SumeragiV2OutboundIntentKind::CommitQc
            );
            let shape_is_valid = match intent.kind {
                SumeragiV2OutboundIntentKind::Proposal => {
                    intent.proposal_round.is_some()
                        && intent.subject.is_some()
                        && intent.execution_commitment.is_none()
                }
                SumeragiV2OutboundIntentKind::TimeoutVote
                | SumeragiV2OutboundIntentKind::TimeoutCertificate => {
                    intent.proposal_round.is_none()
                        && intent.subject.is_none()
                        && intent.execution_commitment.is_none()
                }
                _ => {
                    carries_execution
                        && intent.proposal_round.is_some()
                        && intent.subject.is_some()
                        && intent.execution_commitment.is_some()
                }
            };
            if !shape_is_valid {
                return Err(Error::InvalidOutboundIntentShape);
            }
            if let Some(proposal_round) = intent.proposal_round {
                validate_round_binding(proposal_round)?;
                if proposal_round != intent.round {
                    return Err(Error::InvalidProposalRound);
                }
            }
            if let Some(commitment) = intent.execution_commitment {
                commitment
                    .validate()
                    .map_err(|_| Error::InvalidLivenessExecutionCommitment)?;
            }
        }
        let mut queue_kinds = BTreeSet::new();
        for queue in &self.liveness.queues {
            if queue.capacity == 0
                || queue.depth > queue.capacity
                || (queue.depth == 0 && queue.oldest_age_ms.is_some())
                || (queue.depth != 0 && queue.oldest_age_ms.is_none())
                || !queue_kinds.insert(queue.queue)
            {
                return Err(Error::InvalidLivenessQueue);
            }
        }
        let mut ignore_reasons = BTreeSet::new();
        if self
            .liveness
            .ignore_counts
            .iter()
            .any(|entry| !ignore_reasons.insert(entry.reason))
        {
            return Err(Error::DuplicateLivenessIgnoreReason);
        }
        if let Some(progress) = self.liveness.last_progress {
            validate_round_binding(progress.round)?;
            if progress.round.view > self.view
                && !matches!(
                    progress.transition,
                    SumeragiV2ProgressTransition::CommitQuorum
                        | SumeragiV2ProgressTransition::DecisionPersisted
                )
            {
                return Err(Error::LivenessRoundFromFutureView);
            }
            if progress.generation > self.liveness.generation {
                return Err(Error::LivenessGenerationFromFuture);
            }
        }
        Ok(())
    }
}
/// Roster-independent structural failures in an exact Sumeragi v2 status snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SumeragiV2StatusValidationError {
    /// The snapshot declared another consensus protocol version.
    UnsupportedProtocolVersion {
        /// Required first-release version.
        expected: u16,
        /// Received version.
        actual: u16,
    },
    /// An active consensus height must be positive.
    ZeroHeight,
    /// WAL operation identifiers are non-zero.
    ZeroPersistenceId,
    /// The compact height context's epoch does not cover the active height.
    EpochEndsBeforeHeight,
    /// The compact height context declared a non-`3f + 1` validator roster.
    InvalidValidatorCount,
    /// The expected leader does not index the frozen validator roster.
    LeaderOutOfRange,
    /// The compact height context's equal-vote quorum is not structurally canonical.
    InvalidHeightContextQuorum,
    /// The reducer phase cannot emit the reported body state.
    PhaseBodyMismatch,
    /// Commit collection requires a persisted `PrepareQC` lock.
    CommitWithoutLock,
    /// Prepare collection cannot retain a prior `PrepareQC` lock.
    PrepareWithLock,
    /// Pending-apply state did not report the current decided subject and height.
    PendingApplyCommitMismatch,
    /// A non-decided active height reported its commit frontier at or ahead of itself.
    CommittedHeightNotBehindActiveHeight,
    /// The pre-genesis commit frontier carried a block subject.
    GenesisCommitCarriesSubject,
    /// The committed subject and authenticated `CommitQC` summary were not present together.
    CommitFrontierAuthenticationMismatch,
    /// The `CommitQC` summary did not certify the reported committed subject and height.
    CommitSummaryCertificateMismatch,
    /// The `CommitQC` summary did not satisfy its frozen equal-vote quorum.
    InvalidCommitSummaryQuorum,
    /// A `CommitQC` for the active context reported different frozen quorum inputs.
    CommitSummaryContextMismatch,
    /// A QC or TC reference was bound to another height context.
    CertificateContextMismatch,
    /// A QC or TC reference was bound to another height.
    CertificateHeightMismatch,
    /// A status QC reference was not a `PrepareQC`.
    CertificatePhaseMismatch,
    /// A QC reference came from a view above the current view.
    CertificateFromFutureView,
    /// A vote-pool or certificate reference was not bound to one exact round.
    InvalidProposalRound,
    /// A persisted lock was present without a highest `PrepareQC`.
    LockedCertificateWithoutHighest,
    /// The persisted lock was above the reported highest `PrepareQC`.
    LockedCertificateAboveHighest,
    /// Lock and highest references disagreed at the same view.
    ConflictingCertificatesAtSameView,
    /// A timeout certificate did not precede the current view.
    TimeoutNotBeforeCurrentView,
    /// A timeout certificate reported a `PrepareQC` from above its timed-out view.
    TimeoutCarriesFuturePrepare,
    /// A liveness status collection exceeded its fixed protocol bound.
    LivenessCollectionTooLarge,
    /// A liveness diagnostic was bound to another height or context.
    LivenessRoundMismatch,
    /// A non-finality liveness diagnostic was bound to a view above the current view.
    LivenessRoundFromFutureView,
    /// A partial liveness quorum disagreed with the frozen height context.
    InvalidLivenessQuorum,
    /// A liveness diagnostic carried a malformed execution commitment.
    InvalidLivenessExecutionCommitment,
    /// An outbound intent's subject fields disagreed with its protocol kind.
    InvalidOutboundIntentShape,
    /// A queue diagnostic declared invalid occupancy, age, or duplicate identity.
    InvalidLivenessQueue,
    /// An ignore reason appeared more than once in the per-height counters.
    DuplicateLivenessIgnoreReason,
    /// A progress record referred to a reducer generation not yet installed.
    LivenessGenerationFromFuture,
}
impl fmt::Display for SumeragiV2StatusValidationError {
    #[expect(
        clippy::too_many_lines,
        reason = "the exhaustive display table keeps every public status-validation code paired with its stable diagnostic"
    )]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SumeragiV2StatusValidationError as Error;
        match self {
            Error::UnsupportedProtocolVersion { expected, actual } => write!(
                f,
                "unsupported Sumeragi status protocol version {actual}; expected {expected}"
            ),
            Error::ZeroHeight => f.write_str("Sumeragi status height must be positive"),
            Error::ZeroPersistenceId => {
                f.write_str("Sumeragi status persistence identifier must be non-zero")
            }
            Error::EpochEndsBeforeHeight => {
                f.write_str("Sumeragi status epoch end must cover the active height")
            }
            Error::InvalidValidatorCount => {
                f.write_str("Sumeragi status validator count does not have bounded 3f + 1 geometry")
            }
            Error::LeaderOutOfRange => {
                f.write_str("Sumeragi status leader does not index the frozen validator roster")
            }
            Error::InvalidHeightContextQuorum => {
                f.write_str("Sumeragi status height-context quorum is not canonical")
            }
            Error::PhaseBodyMismatch => {
                f.write_str("Sumeragi status phase and body state are inconsistent")
            }
            Error::CommitWithoutLock => {
                f.write_str("Sumeragi status Commit phase requires a PrepareQC lock")
            }
            Error::PrepareWithLock => {
                f.write_str("Sumeragi status Prepare phase cannot carry a PrepareQC lock")
            }
            Error::PendingApplyCommitMismatch => f.write_str(
                "pending-apply status must carry the current decided height and subject",
            ),
            Error::CommittedHeightNotBehindActiveHeight => f.write_str(
                "non-decided Sumeragi status must have a committed height below the active height",
            ),
            Error::GenesisCommitCarriesSubject => {
                f.write_str("pre-genesis commit frontier cannot carry a subject or CommitQC")
            }
            Error::CommitFrontierAuthenticationMismatch => {
                f.write_str("Sumeragi status committed subject and CommitQC must be paired")
            }
            Error::CommitSummaryCertificateMismatch => {
                f.write_str("Sumeragi status CommitQC does not certify the committed frontier")
            }
            Error::InvalidCommitSummaryQuorum => {
                f.write_str("Sumeragi status CommitQC summary does not satisfy its frozen quorum")
            }
            Error::CommitSummaryContextMismatch => f.write_str(
                "Sumeragi status CommitQC quorum differs from the active height context",
            ),
            Error::CertificateContextMismatch => {
                f.write_str("Sumeragi status certificate context does not match the active context")
            }
            Error::CertificateHeightMismatch => {
                f.write_str("Sumeragi status certificate height does not match the active height")
            }
            Error::CertificatePhaseMismatch => {
                f.write_str("Sumeragi status QC reference must be a PrepareQC")
            }
            Error::CertificateFromFutureView => {
                f.write_str("Sumeragi status QC reference is from a future view")
            }
            Error::InvalidProposalRound => {
                f.write_str("Sumeragi status carries split-round proposal evidence")
            }
            Error::LockedCertificateWithoutHighest => {
                f.write_str("Sumeragi status lock requires a highest PrepareQC")
            }
            Error::LockedCertificateAboveHighest => {
                f.write_str("Sumeragi status lock is above its highest PrepareQC")
            }
            Error::ConflictingCertificatesAtSameView => {
                f.write_str("Sumeragi status lock and highest PrepareQC conflict at the same view")
            }
            Error::TimeoutNotBeforeCurrentView => {
                f.write_str("Sumeragi status timeout certificate must precede the current view")
            }
            Error::TimeoutCarriesFuturePrepare => f.write_str(
                "Sumeragi status timeout certificate carries a PrepareQC from a future view",
            ),
            Error::LivenessCollectionTooLarge => {
                f.write_str("Sumeragi liveness status exceeds a fixed collection bound")
            }
            Error::LivenessRoundMismatch => {
                f.write_str("Sumeragi liveness status round does not match the active height")
            }
            Error::LivenessRoundFromFutureView => {
                f.write_str("Sumeragi non-finality liveness round is from a future view")
            }
            Error::InvalidLivenessQuorum => {
                f.write_str("Sumeragi liveness status quorum is not structurally valid")
            }
            Error::InvalidLivenessExecutionCommitment => {
                f.write_str("Sumeragi liveness status contains an invalid execution commitment")
            }
            Error::InvalidOutboundIntentShape => {
                f.write_str("Sumeragi liveness outbound intent has inconsistent subject fields")
            }
            Error::InvalidLivenessQueue => {
                f.write_str("Sumeragi liveness queue occupancy or identity is invalid")
            }
            Error::DuplicateLivenessIgnoreReason => {
                f.write_str("Sumeragi liveness status repeats an ignore reason")
            }
            Error::LivenessGenerationFromFuture => {
                f.write_str("Sumeragi liveness progress record is from a future generation")
            }
        }
    }
}
impl std::error::Error for SumeragiV2StatusValidationError {}
impl ConsensusMessageV2 {
    /// Wrap a v2 payload with the canonical protocol version.
    #[must_use]
    pub const fn new(payload: ConsensusMessageV2Payload) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            payload,
        }
    }
    /// Reject envelopes from any other consensus wire version.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::UnsupportedProtocolVersion`] when the
    /// explicit version is not v2.
    pub fn validate_version(&self) -> Result<(), ValidationError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ValidationError::UnsupportedProtocolVersion {
                expected: PROTOCOL_VERSION,
                actual: self.protocol_version,
            });
        }
        Ok(())
    }
}
/// Structural validation failures for Sumeragi v2 wire values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationError {
    /// An envelope or context declared an unsupported protocol version.
    UnsupportedProtocolVersion {
        /// Required version.
        expected: u16,
        /// Received version.
        actual: u16,
    },
    /// The voting roster is empty.
    EmptyRoster,
    /// The roster cannot be indexed by [`ValidatorIndex`].
    RosterTooLarge,
    /// A validator occurs more than once in the frozen roster.
    DuplicateValidator,
    /// The frozen roster is not in canonical validator-identity order.
    RosterNotStrictlySorted,
    /// A voting power is zero or negative.
    InvalidVotingPower,
    /// Voting-power arithmetic overflowed.
    VotingPowerOverflow,
    /// Encoded total power differs from the roster sum.
    TotalPowerMismatch,
    /// Encoded count threshold is not the canonical strict supermajority.
    CountThresholdMismatch,
    /// Every committee member must have exactly one consensus vote.
    VotingPowerNotOne,
    /// The frozen epoch end precedes the height governed by this context.
    EpochEndsBeforeHeight,
    /// An epoch-ending context omitted its old-roster-authenticated transition.
    MissingNextEpochSnapshot,
    /// A non-boundary context attempted to install an epoch transition.
    UnexpectedNextEpochSnapshot,
    /// The next-epoch number is not the immediate successor or overflowed.
    InvalidNextEpoch,
    /// The next epoch ends before the first height it would govern.
    NextEpochEndsBeforeSuccessor,
    /// The next-epoch snapshot changes the genesis-selected consensus mode.
    NextEpochModeMismatch,
    /// The next-epoch quorum is not canonically derived from its roster.
    NextEpochQuorumMismatch,
    /// Next-epoch `PoPs` are not aligned one-for-one with its roster.
    NextEpochProofOfPossessionCount,
    /// A next-epoch roster slot contains no proof of possession.
    MissingNextEpochProofOfPossession,
    /// A next-epoch proof of possession exceeds the protocol bound.
    NextEpochProofOfPossessionTooLarge,
    /// A next-epoch snapshot assigned non-unit consensus voting power.
    NextEpochVotingPowerNotOne,
    /// The voting roster cannot tolerate at least one Byzantine validator.
    RosterTooSmall,
    /// The voting roster does not have the exact `3f + 1` shape.
    InvalidCommitteeGeometry,
    /// The parent certificate is not a `CommitQC` for the previous height.
    InvalidParentCommit,
    /// The audited snapshot bootstrap record or its height/anchor relationship is malformed.
    InvalidSnapshotBootstrap,
    /// The mandatory data-availability layout is internally inconsistent.
    InvalidDataAvailabilityLayout,
    /// The mandatory Nexus/AMX context commitment is zero or non-canonical.
    InvalidNexusAmxContextHash,
    /// The mandatory process-local execution-policy commitment is zero or non-canonical.
    InvalidExecutionPolicyHash,
    /// A certificate or message is bound to another height context.
    WrongHeightContext,
    /// Signer count cannot be represented on the wire.
    TooManySigners,
    /// A wire certificate does not carry exactly the canonical signer count.
    SignerCountMismatch {
        /// Canonical signer count required by the height context.
        expected: u32,
        /// Signer count carried by the certificate.
        actual: u32,
    },
    /// Signer indices are duplicated or not in strictly increasing order.
    SignersNotStrictlySorted,
    /// A signer index lies outside the frozen roster.
    SignerOutOfRange,
    /// A requested signer is not present in the certificate signer set.
    SignerNotInCertificate,
    /// A signed message carries no signature bytes.
    MissingSignature,
    /// Execution commitment count/root presence is not canonical.
    InvalidExecutionCommitment,
    /// The result-bearing block wire commitment declares a zero or oversized byte length.
    InvalidExecutedBlockWireLength,
    /// The advertised Kagemusha top-up count exceeds the consensus bound.
    TooManyKagemushaTopupAnchors,
    /// A top-up execution commitment's combined post root is not canonical.
    ExecutionCommitmentPostRootMismatch,
    /// A Native AMX application manifest declared an unsupported version.
    InvalidNativeAmxApplicationManifestVersion,
    /// A Native AMX application manifest count/root pair is not canonical.
    InvalidNativeAmxApplicationManifestCommitment,
    /// A merge-carrier commitment declared an unsupported projection version.
    InvalidMergeCarrierCommitmentVersion,
    /// A Native AMX application manifest exceeds the route-leaf bound.
    TooManyNativeAmxApplicationManifestLeaves,
    /// A lane-finality manifest exceeds the active-lane protocol bound.
    TooManyLaneFinalityStatements,
    /// A Native AMX application leaf carries an invalid route or block identity.
    InvalidNativeAmxApplicationManifestLeaf,
    /// A Native AMX application leaf carries malformed ordered membership.
    InvalidNativeAmxApplicationManifestMembership,
    /// An aggregate certificate or timeout group carries no aggregate signature.
    MissingAggregateSignature,
    /// A signature or aggregate exceeds the protocol allocation bound.
    SignatureTooLarge,
    /// Too few distinct validators signed.
    InsufficientSignerCount,
    /// The redundant signed-vote projection is not a strict supermajority.
    InsufficientVotingPower,
    /// A timeout certificate contains no groups.
    EmptyTimeoutCertificate,
    /// A timeout group contains no signatures.
    EmptyTimeoutGroup,
    /// Timeout groups are duplicated or not canonically ordered.
    TimeoutGroupsNotStrictlySorted,
    /// The same validator appears in more than one timeout group.
    OverlappingTimeoutSigners,
    /// A timeout group reported a `CommitQC` instead of a `PrepareQC`.
    TimeoutCarriesNonPrepareQc,
    /// A timeout group reported a QC from a future view.
    QcFromFutureView,
    /// Timeout groups report conflicting `PrepareQCs` from the same view.
    ConflictingHighestPrepare,
    /// A proposal was not issued by the deterministic leader for its view.
    WrongProposer,
    /// A proposal's round, subject, and manifest do not describe one payload.
    ProposalManifestMismatch,
    /// A proposal's parent or timeout justification is invalid for its view.
    InvalidProposalJustification,
    /// A manifest's DA layout differs from its height context.
    WrongDataAvailabilityLayout,
    /// A payload manifest contains no chunk commitments.
    EmptyPayloadManifest,
    /// A payload manifest has more chunks than the wire index can represent.
    ChunkCountTooLarge,
    /// A canonical body exceeds the frozen per-height payload limit.
    PayloadTooLarge,
    /// Payload size and encoded chunk count/body length are inconsistent.
    PayloadSizeMismatch,
    /// The committed chunk root is not derived from the ordered chunk hashes.
    ChunkRootMismatch,
    /// A payload chunk references another manifest.
    ManifestHashMismatch,
    /// A payload chunk index is outside its manifest.
    ChunkIndexOutOfRange,
    /// Encoded chunk bytes do not match the manifest commitment.
    ChunkHashMismatch,
    /// Encoded chunk length is inconsistent with the frozen layout.
    InvalidChunkLength,
    /// A payload chunk does not carry a sender signature.
    MissingChunkSignature,
    /// A certified body request's QC does not certify its requested subject
    /// in the exact requested round.
    CertifiedBodyCertificateMismatch,
    /// A vote or certificate is split across distinct proposal and vote rounds.
    InvalidProposalRound,
    /// Certified body bytes do not match the manifest payload hash.
    CertifiedBodyHashMismatch,
    /// A certified response does not answer the exact outstanding request.
    CertifiedBodyRequestMismatch,
    /// The authenticated transport sender differs from the claimed responder.
    ResponderIdentityMismatch,
    /// A commit-certificate response did not carry a `CommitQC` for this context.
    CommitCertificateMismatch,
    /// A commit-certificate response did not answer the exact outstanding request.
    CommitCertificateRequestMismatch,
}
impl fmt::Display for ValidationError {
    #[expect(
        clippy::too_many_lines,
        reason = "the exhaustive display table keeps every public consensus-validation code paired with its stable diagnostic"
    )]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedProtocolVersion { expected, actual } => write!(
                f,
                "unsupported Sumeragi protocol version {actual}; expected {expected}"
            ),
            Self::EmptyRoster => f.write_str("voting roster is empty"),
            Self::RosterTooLarge => f.write_str("voting roster exceeds validator-index range"),
            Self::DuplicateValidator => f.write_str("voting roster contains a duplicate validator"),
            Self::RosterNotStrictlySorted => f.write_str("voting roster is not strictly ordered"),
            Self::InvalidVotingPower => f.write_str("voting power must be positive"),
            Self::VotingPowerOverflow => f.write_str("voting-power arithmetic overflow"),
            Self::TotalPowerMismatch => f.write_str("total voting power does not match roster"),
            Self::CountThresholdMismatch => {
                f.write_str("count threshold is not the canonical strict supermajority")
            }
            Self::VotingPowerNotOne => {
                f.write_str("every consensus validator must have voting power one")
            }
            Self::EpochEndsBeforeHeight => {
                f.write_str("height context epoch ends before its governed height")
            }
            Self::MissingNextEpochSnapshot => {
                f.write_str("epoch-ending height context is missing its next-epoch snapshot")
            }
            Self::UnexpectedNextEpochSnapshot => {
                f.write_str("non-boundary height context carries a next-epoch snapshot")
            }
            Self::InvalidNextEpoch => {
                f.write_str("next-epoch snapshot is not for the immediate successor epoch")
            }
            Self::NextEpochEndsBeforeSuccessor => {
                f.write_str("next epoch ends before its first governed height")
            }
            Self::NextEpochModeMismatch => {
                f.write_str("next-epoch snapshot changes the frozen consensus mode")
            }
            Self::NextEpochQuorumMismatch => {
                f.write_str("next-epoch quorum is not canonical for its roster")
            }
            Self::NextEpochProofOfPossessionCount => {
                f.write_str("next-epoch PoP count does not match its roster")
            }
            Self::MissingNextEpochProofOfPossession => {
                f.write_str("next-epoch snapshot contains an empty PoP")
            }
            Self::NextEpochProofOfPossessionTooLarge => {
                f.write_str("next-epoch snapshot contains an oversized PoP")
            }
            Self::NextEpochVotingPowerNotOne => {
                f.write_str("every next-epoch consensus validator must have voting power one")
            }
            Self::RosterTooSmall => {
                f.write_str("voting roster must contain at least four validators")
            }
            Self::InvalidCommitteeGeometry => {
                f.write_str("voting roster must contain exactly 3f + 1 validators")
            }
            Self::InvalidParentCommit => {
                f.write_str("height context parent is not the previous height CommitQC")
            }
            Self::InvalidSnapshotBootstrap => {
                f.write_str("height context has an invalid audited snapshot bootstrap")
            }
            Self::InvalidDataAvailabilityLayout => {
                f.write_str("height context has an invalid data-availability layout")
            }
            Self::InvalidNexusAmxContextHash => {
                f.write_str("height context has an invalid Nexus/AMX context hash")
            }
            Self::InvalidExecutionPolicyHash => {
                f.write_str("height context has an invalid execution-policy hash")
            }
            Self::WrongHeightContext => f.write_str("message is bound to another height context"),
            Self::TooManySigners => f.write_str("signer count exceeds the wire range"),
            Self::SignerCountMismatch { expected, actual } => write!(
                f,
                "certificate signer count mismatch: expected exactly {expected}, got {actual}"
            ),
            Self::SignersNotStrictlySorted => {
                f.write_str("signer indices are not strictly increasing")
            }
            Self::SignerOutOfRange => f.write_str("signer index is outside the voting roster"),
            Self::SignerNotInCertificate => f.write_str("signer is not present in the certificate"),
            Self::MissingSignature => f.write_str("signed message has an empty signature"),
            Self::InvalidExecutionCommitment => {
                f.write_str("execution commitment top-up count/root presence is inconsistent")
            }
            Self::InvalidExecutedBlockWireLength => {
                write!(
                    f,
                    "execution commitment block wire length must be between 1 and {MAX_EXECUTED_BLOCK_WIRE_BYTES} bytes"
                )
            }
            Self::TooManyKagemushaTopupAnchors => {
                f.write_str("execution commitment exceeds the Kagemusha top-up anchor limit")
            }
            Self::ExecutionCommitmentPostRootMismatch => {
                f.write_str("execution commitment post-state root is not canonical")
            }
            Self::InvalidNativeAmxApplicationManifestVersion => {
                f.write_str("Native AMX application manifest version is unsupported")
            }
            Self::InvalidNativeAmxApplicationManifestCommitment => {
                f.write_str("Native AMX application manifest count/root is not canonical")
            }
            Self::InvalidMergeCarrierCommitmentVersion => {
                f.write_str("merge-carrier execution commitment version is unsupported")
            }
            Self::TooManyNativeAmxApplicationManifestLeaves => {
                f.write_str("Native AMX application manifest exceeds the route-leaf limit")
            }
            Self::TooManyLaneFinalityStatements => {
                f.write_str("lane-finality manifest exceeds the active-lane limit")
            }
            Self::InvalidNativeAmxApplicationManifestLeaf => {
                f.write_str("Native AMX application manifest leaf identity is malformed")
            }
            Self::InvalidNativeAmxApplicationManifestMembership => {
                f.write_str("Native AMX application manifest membership is malformed")
            }
            Self::MissingAggregateSignature => {
                f.write_str("certificate has an empty aggregate signature")
            }
            Self::SignatureTooLarge => f.write_str("consensus signature exceeds protocol bound"),
            Self::InsufficientSignerCount => {
                f.write_str("insufficient distinct validator signatures")
            }
            Self::InsufficientVotingPower => {
                f.write_str("inconsistent redundant signed-vote projection")
            }
            Self::EmptyTimeoutCertificate => f.write_str("timeout certificate has no groups"),
            Self::EmptyTimeoutGroup => f.write_str("timeout vote group has no signers"),
            Self::TimeoutGroupsNotStrictlySorted => {
                f.write_str("timeout groups are not strictly ordered")
            }
            Self::OverlappingTimeoutSigners => {
                f.write_str("timeout vote groups contain overlapping signers")
            }
            Self::TimeoutCarriesNonPrepareQc => {
                f.write_str("timeout vote carries a non-Prepare QC")
            }
            Self::QcFromFutureView => f.write_str("timeout vote carries a QC from a future view"),
            Self::ConflictingHighestPrepare => {
                f.write_str("timeout groups carry conflicting PrepareQCs from one view")
            }
            Self::WrongProposer => f.write_str("proposal was not issued by the expected leader"),
            Self::ProposalManifestMismatch => {
                f.write_str("proposal round or subject does not match its manifest")
            }
            Self::InvalidProposalJustification => {
                f.write_str("proposal justification is invalid for its view")
            }
            Self::WrongDataAvailabilityLayout => {
                f.write_str("payload manifest uses another data-availability layout")
            }
            Self::EmptyPayloadManifest => f.write_str("payload manifest has no chunks"),
            Self::ChunkCountTooLarge => f.write_str("payload chunk count exceeds the wire range"),
            Self::PayloadTooLarge => f.write_str("payload exceeds the frozen size limit"),
            Self::PayloadSizeMismatch => {
                f.write_str("payload size is inconsistent with its encoded chunks")
            }
            Self::ChunkRootMismatch => {
                f.write_str("payload chunk root does not match ordered chunk hashes")
            }
            Self::ManifestHashMismatch => f.write_str("payload chunk references another manifest"),
            Self::ChunkIndexOutOfRange => f.write_str("payload chunk index is out of range"),
            Self::ChunkHashMismatch => f.write_str("payload chunk hash does not match manifest"),
            Self::InvalidChunkLength => {
                f.write_str("payload chunk length is inconsistent with the layout")
            }
            Self::MissingChunkSignature => f.write_str("payload chunk signature is empty"),
            Self::CertifiedBodyCertificateMismatch => {
                f.write_str("certified body request does not match its certificate")
            }
            Self::InvalidProposalRound => {
                f.write_str("proposal and certified rounds must be identical")
            }
            Self::CertifiedBodyHashMismatch => {
                f.write_str("certified body bytes do not match the manifest payload hash")
            }
            Self::CertifiedBodyRequestMismatch => {
                f.write_str("certified body response does not match the outstanding request")
            }
            Self::ResponderIdentityMismatch => {
                f.write_str("certified body responder does not match the transport sender")
            }
            Self::CommitCertificateMismatch => {
                f.write_str("commit-certificate response does not carry this context's CommitQC")
            }
            Self::CommitCertificateRequestMismatch => {
                f.write_str("commit-certificate response does not match the outstanding request")
            }
        }
    }
}
impl std::error::Error for ValidationError {}
fn payload_chunk_root(chunk_hashes: &[Hash]) -> Option<Hash> {
    let leaves = chunk_hashes
        .iter()
        .map(|hash| *hash.as_ref())
        .collect::<Vec<[u8; 32]>>();
    MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves)
        .root()
        .map(Hash::from)
}
fn expected_encoded_chunk_count(
    payload_size_bytes: u64,
    layout: DataAvailabilityLayout,
) -> Result<u32, ValidationError> {
    let payload = u128::from(payload_size_bytes);
    let chunk_size = u128::from(layout.chunk_size_bytes);
    let count = match layout.encoding {
        PayloadEncoding::ReedSolomon16 => {
            let data_shards = u128::from(layout.data_shards);
            let stripe_payload = chunk_size
                .checked_mul(data_shards)
                .ok_or(ValidationError::ChunkCountTooLarge)?;
            let stripes = payload.div_ceil(stripe_payload);
            stripes
                .checked_mul(u128::from(layout.data_shards) + u128::from(layout.parity_shards))
                .ok_or(ValidationError::ChunkCountTooLarge)?
        }
    };
    u32::try_from(count).map_err(|_| ValidationError::ChunkCountTooLarge)
}
fn validate_data_availability_layout(
    layout: DataAvailabilityLayout,
) -> Result<(), ValidationError> {
    if layout.chunk_size_bytes == 0
        || layout.chunk_size_bytes > MAX_DA_CHUNK_SIZE_BYTES
        || !layout.chunk_size_bytes.is_multiple_of(2)
        || layout.data_shards == 0
        || layout.data_shards > MAX_DA_DATA_SHARDS
        || layout.parity_shards == 0
        || layout.parity_shards > MAX_DA_PARITY_SHARDS
        || layout.data_shards.saturating_add(layout.parity_shards) > MAX_DA_STRIPE_WIDTH
        || layout.max_payload_size_bytes == 0
        || layout.max_payload_size_bytes > MAX_DA_PAYLOAD_SIZE_BYTES
        || layout.max_chunk_count == 0
        || layout.max_chunk_count > MAX_DA_CHUNK_COUNT
    {
        return Err(ValidationError::InvalidDataAvailabilityLayout);
    }
    let required_chunk_capacity =
        expected_encoded_chunk_count(layout.max_payload_size_bytes, layout)
            .map_err(|_| ValidationError::InvalidDataAvailabilityLayout)?;
    let required_encoded_bytes = u64::from(required_chunk_capacity)
        .checked_mul(u64::from(layout.chunk_size_bytes))
        .ok_or(ValidationError::InvalidDataAvailabilityLayout)?;
    if required_chunk_capacity > layout.max_chunk_count
        || required_encoded_bytes > MAX_DA_ENCODED_PAYLOAD_BYTES
    {
        return Err(ValidationError::InvalidDataAvailabilityLayout);
    }
    Ok(())
}
fn validate_encoded_chunk_len(
    manifest: &PayloadManifest,
    actual: usize,
) -> Result<(), ValidationError> {
    let chunk_size = usize::try_from(manifest.layout.chunk_size_bytes)
        .map_err(|_| ValidationError::InvalidChunkLength)?;
    match manifest.layout.encoding {
        PayloadEncoding::ReedSolomon16 if actual == chunk_size => Ok(()),
        PayloadEncoding::ReedSolomon16 => Err(ValidationError::InvalidChunkLength),
    }
}
fn validated_total_power(roster: &[ValidatorPower]) -> Result<u64, ValidationError> {
    if roster.is_empty() {
        return Err(ValidationError::EmptyRoster);
    }
    if roster.len() < MIN_VALIDATORS_PER_HEIGHT {
        return Err(ValidationError::RosterTooSmall);
    }
    if roster.len() > MAX_VALIDATORS_PER_HEIGHT {
        return Err(ValidationError::RosterTooLarge);
    }
    if !is_valid_committee_size(roster.len()) {
        return Err(ValidationError::InvalidCommitteeGeometry);
    }
    let mut seen = BTreeSet::new();
    let mut total = 0_u64;
    for pair in roster.windows(2) {
        if pair[0].validator == pair[1].validator {
            return Err(ValidationError::DuplicateValidator);
        }
        if pair[0].validator > pair[1].validator {
            return Err(ValidationError::RosterNotStrictlySorted);
        }
    }
    for entry in roster {
        if !seen.insert(entry.validator.clone()) {
            return Err(ValidationError::DuplicateValidator);
        }
        if entry.power == 0 {
            return Err(ValidationError::InvalidVotingPower);
        }
        total = total
            .checked_add(entry.power)
            .ok_or(ValidationError::VotingPowerOverflow)?;
    }
    Ok(total)
}
fn validate_round(round: ConsensusRound, context: &HeightContext) -> Result<(), ValidationError> {
    context.validate()?;
    if round.context_id != context.id() || round.height != context.height {
        return Err(ValidationError::WrongHeightContext);
    }
    Ok(())
}
fn validate_proposal_round(
    proposal_round: ConsensusRound,
    certified_round: ConsensusRound,
    context: &HeightContext,
) -> Result<(), ValidationError> {
    validate_round(proposal_round, context)?;
    if proposal_round != certified_round {
        return Err(ValidationError::InvalidProposalRound);
    }
    Ok(())
}
fn validate_validator_index(
    index: ValidatorIndex,
    context: &HeightContext,
) -> Result<(), ValidationError> {
    let index = usize::try_from(index).map_err(|_| ValidationError::SignerOutOfRange)?;
    if index >= context.roster.len() {
        return Err(ValidationError::SignerOutOfRange);
    }
    Ok(())
}
fn require_signature(signature: &[u8]) -> Result<(), ValidationError> {
    if signature.is_empty() {
        Err(ValidationError::MissingSignature)
    } else if signature.len() > MAX_CONSENSUS_SIGNATURE_BYTES {
        Err(ValidationError::SignatureTooLarge)
    } else {
        Ok(())
    }
}
fn require_aggregate_signature(signature: &[u8]) -> Result<(), ValidationError> {
    if signature.is_empty() {
        Err(ValidationError::MissingAggregateSignature)
    } else if signature.len() > MAX_CONSENSUS_SIGNATURE_BYTES {
        Err(ValidationError::SignatureTooLarge)
    } else {
        Ok(())
    }
}
fn signature_preimage(domain: &[u8], encoded_payload: &[u8]) -> Vec<u8> {
    let mut preimage = Vec::with_capacity(domain.len() + encoded_payload.len());
    preimage.extend_from_slice(domain);
    preimage.extend_from_slice(encoded_payload);
    preimage
}
include!("consensus_v2_tests.rs");
