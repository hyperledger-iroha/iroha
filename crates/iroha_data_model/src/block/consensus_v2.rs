//! Canonical Norito wire types for the Sumeragi v2 consensus protocol.
//!
//! Sumeragi v2 deliberately keeps its global Prepare/Commit protocol separate
//! from the lane-local [`super::consensus::CertPhase`] protocol.  The types in
//! this module are therefore versioned independently and do not replace or
//! reinterpret the first-release wire types in [`super::consensus`].

use core::fmt;
use std::{collections::BTreeSet, vec::Vec};

use iroha_crypto::{Hash, HashOf, MerkleTree};
use iroha_schema::{EnumMeta, EnumVariant, Ident, IntoSchema, MetaMap, Metadata, TypeId};
use norito::codec::{Decode, Encode};

use super::Header as BlockHeader;
use crate::{
    ChainId,
    account::AccountId,
    nexus::{LaneId, PublicLaneValidatorRecord},
    peer::PeerId,
};

/// Durable finality artifacts associated with canonical Sumeragi v2 blocks.
pub mod finality;
/// Canonical genesis/handshake fingerprint projection.
pub mod fingerprint;

/// Sumeragi v2 wire protocol version.
pub const PROTOCOL_VERSION: u16 = 3;
/// Consensus-wide upper bound for one voting roster.
pub const MAX_VALIDATORS_PER_HEIGHT: usize = 4_096;
/// Tight allocation bound for one consensus signature or aggregate.
pub const MAX_CONSENSUS_SIGNATURE_BYTES: usize = 256;
const HEIGHT_CONTEXT_IDENTITY_VERSION: u16 = 3;
/// Permissioned Sumeragi v2 handshake and domain-separation tag.
pub const PERMISSIONED_TAG: &str = "iroha2-consensus::permissioned-sumeragi@v2";
/// NPoS Sumeragi v2 handshake and domain-separation tag.
pub const NPOS_TAG: &str = "iroha2-consensus::npos-sumeragi@v2";
/// BLS domain selected by a permissioned v2 genesis.
pub const PERMISSIONED_BLS_DOMAIN: &str = "bls-iroha2:permissioned-sumeragi:v2";
/// BLS domain selected by an NPoS v2 genesis.
pub const NPOS_BLS_DOMAIN: &str = "bls-iroha2:npos-sumeragi:v2";
/// Maximum block-local Kagemusha top-up anchors authenticated by one execution commitment.
pub const MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK: u32 = 16;
const KAGEMUSHA_TOPUP_POST_STATE_ROOT_DOMAIN: &[u8] = b"iroha:kagemusha:v2:post-state-root";
/// Canonical Nexus/AMX context commitment for the repository's recommended
/// single-lane defaults and no staged public-lane validators.
///
/// `iroha_config` owns the projection and pins this value with a golden test.
/// Keeping the bytes here lets configuration-independent genesis builders emit
/// a valid signed template without introducing a data-model/config cycle.
pub const RECOMMENDED_NEXUS_AMX_CONTEXT_HASH: [u8; 32] = [
    212, 70, 210, 25, 235, 128, 26, 231, 82, 205, 1, 104, 224, 244, 123, 58, 207, 83, 186, 77, 92,
    150, 95, 210, 152, 63, 79, 147, 244, 218, 110, 167,
];

/// Block height in the v2 protocol.
pub type Height = u64;

/// View number within one block height.
pub type View = u64;

/// Index into the ordered voting roster frozen in a [`HeightContext`].
pub type ValidatorIndex = u32;

/// Consensus mode used to construct the frozen voting-power snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(
    tag = "mode",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ConsensusMode {
    /// Every validator has voting power one.
    Permissioned,
    /// Voting powers come from the finalized epoch stake snapshot.
    Npos,
}

/// A validator and its voting power at one height.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct ValidatorPower {
    /// Validator identity and consensus public key.
    pub validator: PeerId,
    /// Positive voting power frozen for this height.
    pub power: u64,
}

/// Count-and-power quorum parameters frozen in a height context.
///
/// A certificate must satisfy both thresholds.  The count threshold is the
/// smallest integer strictly greater than two thirds of the voting roster;
/// signed power must be strictly greater than two thirds of total power.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct DualQuorum {
    /// Required number of distinct validator signatures.
    pub min_signers: u32,
    /// Total voting power represented by the ordered roster.
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

    /// Construct the canonical dual quorum for an ordered voting roster.
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
        if signers.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ValidationError::SignersNotStrictlySorted);
        }
        let signed_count =
            u32::try_from(signers.len()).map_err(|_| ValidationError::TooManySigners)?;
        if signed_count < self.min_signers {
            return Err(ValidationError::InsufficientSignerCount);
        }

        let mut signed_power = 0_u64;
        for signer in signers {
            let index = usize::try_from(*signer).map_err(|_| ValidationError::SignerOutOfRange)?;
            let entry = roster.get(index).ok_or(ValidationError::SignerOutOfRange)?;
            signed_power = signed_power
                .checked_add(entry.power)
                .ok_or(ValidationError::VotingPowerOverflow)?;
        }

        let signed_scaled = u128::from(signed_power) * 3;
        let total_scaled = u128::from(self.total_power) * 2;
        if signed_scaled <= total_scaled {
            return Err(ValidationError::InsufficientVotingPower);
        }
        Ok(())
    }
}

/// Payload chunking parameters frozen for one block height.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct DataAvailabilityLayout {
    /// Payload encoding used before chunk dissemination.
    pub encoding: PayloadEncoding,
    /// Maximum encoded chunk size in bytes.
    pub chunk_size_bytes: u32,
    /// Data shards per stripe; zero for plain chunking.
    pub data_shards: u16,
    /// Parity shards per stripe; zero for plain chunking.
    pub parity_shards: u16,
    /// Maximum canonical body size accepted at this height.
    pub max_payload_size_bytes: u64,
    /// Maximum number of encoded chunks accepted for one body.
    pub max_chunk_count: u32,
}

/// Payload encoding used by v2 data dissemination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(
    tag = "encoding",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum PayloadEncoding {
    /// Split the canonical payload into unencoded chunks.
    Plain,
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
                chunk_size_bytes: 256 * 1024,
                data_shards: 4,
                parity_shards: 2,
                max_payload_size_bytes: 16 * 1024 * 1024,
                max_chunk_count: 1024,
            },
            nexus_amx_context_hash: RECOMMENDED_NEXUS_AMX_CONTEXT_HASH,
        }
    }

    /// Validate the signed context parameters using the same structural rules
    /// enforced for a full height context.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidDataAvailabilityLayout`] for a zero
    /// limit or an encoding/shard mismatch.
    pub fn validate(&self) -> Result<(), ValidationError> {
        let layout = self.da_layout;
        if layout.chunk_size_bytes == 0
            || layout.max_payload_size_bytes == 0
            || layout.max_chunk_count == 0
        {
            return Err(ValidationError::InvalidDataAvailabilityLayout);
        }
        match layout.encoding {
            PayloadEncoding::Plain if layout.data_shards != 0 || layout.parity_shards != 0 => {
                Err(ValidationError::InvalidDataAvailabilityLayout)
            }
            PayloadEncoding::ReedSolomon16
                if layout.data_shards == 0 || layout.parity_shards == 0 =>
            {
                Err(ValidationError::InvalidDataAvailabilityLayout)
            }
            PayloadEncoding::Plain | PayloadEncoding::ReedSolomon16 => Ok(()),
        }
    }
}

/// Canonical staged active-lane record committed by v2 genesis metadata.
pub type GenesisActiveNexusLaneRecord = ((LaneId, AccountId), PublicLaneValidatorRecord);

/// Audited snapshot boundary which explicitly replaces an unavailable parent CommitQC.
///
/// The complete [`SnapshotV2BootstrapRecord`] is carried inside the signed or digest-pinned
/// snapshot payload. These fields bind its frozen context to the exact restored ledger
/// geometry and WSV, so an appended self-signed artifact cannot introduce a different trust root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    /// Cryptographic PoP validation and comparison with restored live consensus keys are performed
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct HeightContext {
    /// Chain identifier used for replay protection.
    pub chain_id: ChainId,
    /// Wire protocol version; must equal [`PROTOCOL_VERSION`].
    pub protocol_version: u16,
    /// Height governed by this context.
    pub height: Height,
    /// Finalized validator-election epoch.
    pub epoch: u64,
    /// Last height governed by this epoch's frozen election snapshot.
    pub epoch_end_height: Height,
    /// Complete transition selected from the committed pre-state when this is
    /// the last height of an epoch. The CommitQC authenticates these bytes
    /// through [`Self::id`]; non-boundary contexts must carry `None`.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub next_epoch_snapshot: Option<finality::FinalizedNextEpochSnapshot>,
    /// Consensus mode that produced the voting-power snapshot.
    pub mode: ConsensusMode,
    /// Commit certificate for the parent block, absent only at genesis or an audited snapshot
    /// bootstrap boundary.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub parent_commit_qc: Option<QuorumCertificate>,
    /// Explicit authenticated snapshot boundary used when the parent block body and v2 CommitQC
    /// predate the first-release v2 ledger. Mutually exclusive with `parent_commit_qc`.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub snapshot_bootstrap: Option<SnapshotBootstrapAnchor>,
    /// Deterministically ordered voting roster; observers are excluded.
    pub roster: Vec<ValidatorPower>,
    /// Canonical dual quorum derived from `roster`.
    pub quorum: DualQuorum,
    /// Hash of all frozen Nexus/AMX inputs that proposal assembly and
    /// deterministic validation must bind.
    pub nexus_amx_context_hash: Hash,
    /// Data-availability layout used by proposals at this height.
    pub da_layout: DataAvailabilityLayout,
    /// Finalized seed used to choose the view-zero roster offset.
    pub leader_seed: [u8; 32],
}

impl HeightContext {
    /// Return the typed hash that identifies every round in this context.
    ///
    /// The identity commits to the parent CommitQC's semantic finality key
    /// (parent context, height, phase, subject, and execution commitment), rather than its view,
    /// aggregate signature, or signer subset. Two nodes that finalized the same
    /// parent through different valid CommitQCs must derive the same next-height
    /// context.
    #[must_use]
    pub fn id(&self) -> HeightContextId {
        let identity = HeightContextIdentity {
            identity_version: HEIGHT_CONTEXT_IDENTITY_VERSION,
            chain_id: self.chain_id.clone(),
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
        if self.mode == ConsensusMode::Permissioned
            && self.roster.iter().any(|validator| validator.power != 1)
        {
            return Err(ValidationError::PermissionedPowerNotOne);
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
            (0, _, _) | (1, _, _) | (_, Some(_), Some(_)) | (_, None, None) => {
                return Err(ValidationError::InvalidParentCommit);
            }
            (_, Some(parent), None)
                if parent.phase != GlobalPhase::Commit
                    || parent.round.height.checked_add(1) != Some(self.height) =>
            {
                return Err(ValidationError::InvalidParentCommit);
            }
            (_, Some(_), None) | (_, None, Some(_)) => {}
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
        if self.da_layout.chunk_size_bytes == 0
            || self.da_layout.max_payload_size_bytes == 0
            || self.da_layout.max_chunk_count == 0
        {
            return Err(ValidationError::InvalidDataAvailabilityLayout);
        }
        match self.da_layout.encoding {
            PayloadEncoding::Plain
                if self.da_layout.data_shards != 0 || self.da_layout.parity_shards != 0 =>
            {
                return Err(ValidationError::InvalidDataAvailabilityLayout);
            }
            PayloadEncoding::ReedSolomon16
                if self.da_layout.data_shards == 0 || self.da_layout.parity_shards == 0 =>
            {
                return Err(ValidationError::InvalidDataAvailabilityLayout);
            }
            PayloadEncoding::Plain | PayloadEncoding::ReedSolomon16 => {}
        }
        Ok(())
    }

    /// Validate that a canonical signer list satisfies both quorum thresholds.
    ///
    /// # Errors
    ///
    /// Returns a structural or quorum error when the context or signer list is
    /// invalid.
    pub fn validate_signers(&self, signers: &[ValidatorIndex]) -> Result<(), ValidationError> {
        self.validate()?;
        self.quorum.validate_signers(signers, &self.roster)
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
            (u128::from(remainder) * 256 + u128::from(*byte)).rem_euclid(u128::from(modulus)) as u64
        });
        u32::try_from((start + view % modulus) % modulus)
            .expect("validated roster length fits ValidatorIndex")
    }
}

#[derive(Encode)]
struct HeightContextIdentity {
    identity_version: u16,
    chain_id: ChainId,
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[repr(transparent)]
pub struct HeightContextId(
    /// Norito hash of the context's semantic identity projection.
    pub HashOf<HeightContext>,
);

/// Consensus round identity under a frozen height context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
/// This enum intentionally has no NewView variant: view changes are certified
/// by [`TimeoutCertificate`].  It is also distinct from lane-local phases.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
                discriminant: Self::Prepare as u8,
                ty: None,
            },
            EnumVariant {
                tag: "Commit".to_owned(),
                discriminant: Self::Commit as u8,
                ty: None,
            },
        ];
        metamap.insert::<Self>(Metadata::Enum(EnumMeta { variants }));
    }
}

/// Proposal subject bound by votes and certificates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct BlockSubject {
    /// Parent block hash, absent only for the genesis block.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub parent_block_hash: Option<HashOf<BlockHeader>>,
    /// Proposed block hash.
    pub block_hash: HashOf<BlockHeader>,
    /// Hash of the canonical payload bytes.
    pub payload_hash: Hash,
}

/// Deterministic state-transition commitment authenticated by every Prepare and Commit vote.
///
/// The commitment is derived from the exact state-block execution witness
/// after deterministic candidate validation.  It is never
/// reconstructed from the proposal header or supplied by an untrusted caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct ExecutionCommitment {
    /// Root of the witnessed pre-state values for keys changed by the block.
    pub parent_state_root: Hash,
    /// Root of the complete deterministic post-state projection.
    pub post_state_root: Hash,
    /// Root of all canonical last-write-wins writes other than Kagemusha top-up anchors.
    pub ordinary_writes_root: Hash,
    /// Root of the canonical balanced Kagemusha top-up tree, when the block has top-ups.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub topup_anchor_root: Option<Hash>,
    /// Number of real Kagemusha top-up leaves committed by `topup_anchor_root`.
    pub topup_anchor_count: u32,
    /// Hash of the canonical result-bearing block wire produced by deterministic execution.
    pub executed_block_wire_hash: Hash,
}

impl ExecutionCommitment {
    /// Construct a transition that contains no Kagemusha top-up anchors.
    #[must_use]
    pub const fn without_topups(
        parent_state_root: Hash,
        post_state_root: Hash,
        ordinary_writes_root: Hash,
        executed_block_wire_hash: Hash,
    ) -> Self {
        Self {
            parent_state_root,
            post_state_root,
            ordinary_writes_root,
            topup_anchor_root: None,
            topup_anchor_count: 0,
            executed_block_wire_hash,
        }
    }

    /// Construct a commitment and enforce its canonical top-up projection.
    ///
    /// # Errors
    ///
    /// Returns an error when root presence disagrees with the count, the
    /// bounded top-up count is exceeded, or the combined post-state root is
    /// not the canonical hash of the advertised top-up projection.
    pub fn new(
        parent_state_root: Hash,
        post_state_root: Hash,
        ordinary_writes_root: Hash,
        topup_anchor_root: Option<Hash>,
        topup_anchor_count: u32,
        executed_block_wire_hash: Hash,
    ) -> Result<Self, ValidationError> {
        let commitment = Self {
            parent_state_root,
            post_state_root,
            ordinary_writes_root,
            topup_anchor_root,
            topup_anchor_count,
            executed_block_wire_hash,
        };
        commitment.validate()?;
        Ok(commitment)
    }

    /// Validate the canonical count/root relationship and combined top-up root.
    pub fn validate(&self) -> Result<(), ValidationError> {
        match (self.topup_anchor_count, self.topup_anchor_root) {
            (0, None) => Ok(()),
            (0, Some(_)) | (_, None) => Err(ValidationError::InvalidExecutionCommitment),
            (count, Some(root)) if count <= MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK => {
                if self.post_state_root
                    != Self::topup_post_state_root(count, self.ordinary_writes_root, root)
                {
                    return Err(ValidationError::ExecutionCommitmentPostRootMismatch);
                }
                Ok(())
            }
            (_, Some(_)) => Err(ValidationError::TooManyKagemushaTopupAnchors),
        }
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct Vote {
    /// Round in which the vote was issued.
    pub round: ConsensusRound,
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
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        validate_validator_index(self.signer, context)?;
        self.execution_commitment.validate()?;
        require_signature(&self.signature)
    }
}

/// Canonical same-message fields authenticated by Prepare and Commit votes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct VoteSignaturePayload {
    /// Sumeragi protocol revision.
    pub protocol_version: u16,
    /// Exact round being voted in.
    pub round: ConsensusRound,
    /// Prepare or Commit phase.
    pub phase: GlobalPhase,
    /// Exact block and payload subject.
    pub subject: BlockSubject,
    /// Exact deterministic execution result.
    pub execution_commitment: ExecutionCommitment,
}

/// Stable reference to a full quorum certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct QuorumCertificateRef {
    /// Certified round.
    pub round: ConsensusRound,
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
    /// CommitQCs for one parent may be assembled in different views and from
    /// different signer subsets. Their stable decision identity is the parent
    /// height context, height, Commit phase, and subject; the view is not part
    /// of that identity.
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct QuorumCertificate {
    /// Certified round.
    pub round: ConsensusRound,
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
            phase: self.phase,
            subject: self.subject,
            execution_commitment: self.execution_commitment,
        }
    }

    /// Validate the certificate's context binding and dual quorum.
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
        self.execution_commitment.validate()?;
        context
            .quorum
            .validate_signers(&self.signers, &context.roster)?;
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TimeoutVote {
    /// Round whose timer expired.
    pub round: ConsensusRound,
    /// Highest PrepareQC known to the signer, if any.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TimeoutVoteSignaturePayload {
    /// Sumeragi protocol revision.
    pub protocol_version: u16,
    /// Timed-out round.
    pub round: ConsensusRound,
    /// Highest PrepareQC reported by every signer in this group.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub highest_prepare_qc: Option<QuorumCertificateRef>,
}

/// Aggregate timeout signatures that reported the same highest PrepareQC.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TimeoutVoteGroup {
    /// Highest PrepareQC reported by this group, or none when no lock exists.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub highest_prepare_qc: Option<QuorumCertificate>,
    /// Strictly increasing signer indices in this group.
    pub signers: Vec<ValidatorIndex>,
    /// Aggregate BLS signature for this group's timeout votes.
    pub aggregate_signature: Vec<u8>,
}

/// Certificate authorizing a transition out of one timed-out view.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct TimeoutCertificate {
    /// Round whose timeout was certified.
    pub round: ConsensusRound,
    /// Groups ordered strictly by their optional PrepareQC reference.
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

    /// Select the highest reported PrepareQC deterministically.
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

    /// Validate grouping, disjoint signers, context binding, and dual quorum.
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
        let mut highest_at_view: Option<(View, BlockSubject)> = None;
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
                    Some((view, subject)) if view == highest.round.view => {
                        if subject != highest.subject {
                            return Err(ValidationError::ConflictingHighestPrepare);
                        }
                    }
                    Some((view, _)) if view > highest.round.view => {
                        return Err(ValidationError::TimeoutGroupsNotStrictlySorted);
                    }
                    _ => highest_at_view = Some((highest.round.view, highest.subject)),
                }
            }
            for signer in &group.signers {
                if !all_signers.insert(*signer) {
                    return Err(ValidationError::OverlappingTimeoutSigners);
                }
            }
        }
        let all_signers: Vec<_> = all_signers.into_iter().collect();
        context
            .quorum
            .validate_signers(&all_signers, &context.roster)
    }
}

/// Stable reference to a full timeout certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct TimeoutCertificateRef {
    /// Timed-out round certified by the TC.
    pub round: ConsensusRound,
    /// Highest PrepareQC selected from the grouped timeout votes.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub highest_prepare_qc: Option<QuorumCertificateRef>,
    /// Norito hash of the full timeout certificate.
    pub certificate_hash: HashOf<TimeoutCertificate>,
}

/// Justification carried by a proposal.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "kind", content = "justification", rename_all = "snake_case")]
pub enum ProposalJustification {
    /// View-zero justification from the parent CommitQC.
    ParentCommit(ParentCommitJustification),
    /// Later-view justification from the immediately preceding timeout.
    Timeout(TimeoutJustification),
}

/// View-zero proposal justification from the parent CommitQC.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ParentCommitJustification {
    /// Parent CommitQC; absent only for the genesis block.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub certificate: Option<QuorumCertificate>,
}

/// Later-view proposal justification from a timeout certificate.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TimeoutJustification {
    /// Certificate authorizing the new view.
    pub timeout_certificate: TimeoutCertificate,
    /// Full highest PrepareQC selected from the timeout groups.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub highest_prepare_qc: Option<QuorumCertificate>,
}

/// Manifest committing to a complete encoded block payload.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
        for (index, chunk) in encoded_chunks.iter().enumerate() {
            validate_encoded_chunk_len(&manifest, index, chunk.len())?;
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
        validate_encoded_chunk_len(manifest, index, self.bytes.len())?;
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
                        // one view. Context identity deliberately ignores that
                        // view and the signer subset, so view-zero proposal
                        // admission must use the same semantic finality key or
                        // equal next-height contexts could reject each other.
                        carried.as_ref().same_commit_decision(frozen.as_ref())
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
                if timeout
                    .timeout_certificate
                    .highest_prepare_qc()
                    .map(QuorumCertificate::as_ref)
                    != timeout
                        .highest_prepare_qc
                        .as_ref()
                        .map(QuorumCertificate::as_ref)
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
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "kind", content = "artifacts", rename_all = "snake_case")]
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

/// Authenticated request for a body covered by a PrepareQC or CommitQC.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct CertifiedBodyRequest {
    /// Round in which the body was proposed.
    pub round: ConsensusRound,
    /// Requested subject.
    pub subject: BlockSubject,
    /// Certificate proving that validators should retain the body.
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
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        validate_round(self.round, context)?;
        self.certificate.validate(context)?;
        if self.certificate.round != self.round || self.certificate.subject != self.subject {
            return Err(ValidationError::CertifiedBodyCertificateMismatch);
        }
        require_signature(&self.signature)
    }
}

/// Authenticated response carrying a certified body and its manifest.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
            request_hash: self.request_hash.clone(),
            manifest: self.manifest.clone(),
            body_hash: Hash::new(&self.body),
            responder: self.responder.clone(),
        };
        signature_preimage(
            b"iroha:sumeragi:v2:certified-body-response",
            &payload.encode(),
        )
    }

    /// Validate the response against the frozen context and signature
    /// presence. The caller additionally matches `request_hash` to an
    /// outstanding authenticated request.
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
    /// Returns an error when the response is replayed across requests,
    /// changes round/subject, or comes from a validator not certified by the
    /// request QC.
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
        if request
            .certificate
            .signers
            .binary_search(&self.responder)
            .is_err()
        {
            return Err(ValidationError::ResponderNotCertified);
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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

/// Authenticated request for the durable CommitQC of one exact height context.
///
/// A lagging peer already reconstructs its next immutable [`HeightContext`]
/// from the preceding committed block.  This request deliberately names only
/// that context: responders cannot skip heights or substitute a certificate
/// governed by another roster.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct CommitCertificateRequest {
    /// Consensus protocol revision included in the signed request.
    pub protocol_version: u16,
    /// Chain identifier included explicitly for replay rejection at ingress.
    pub chain_id: ChainId,
    /// Exact frozen context whose durable CommitQC is requested.
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
    pub fn validate(&self, context: &HeightContext) -> Result<(), ValidationError> {
        context.validate()?;
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ValidationError::UnsupportedProtocolVersion {
                expected: PROTOCOL_VERSION,
                actual: self.protocol_version,
            });
        }
        if self.chain_id != context.chain_id
            || self.context_id != context.id()
            || self.height != context.height
        {
            return Err(ValidationError::WrongHeightContext);
        }
        require_signature(&self.signature)
    }
}

/// Authenticated response carrying the CommitQC for an exact outstanding
/// [`CommitCertificateRequest`].
///
/// The response never carries a block body.  Its certificate is admitted as a
/// normal v2 CommitQC through the authoritative reducer; the reducer then
/// initiates the existing certified-body fetch and WAL/apply sequence.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct CommitCertificateResponse {
    /// Hash of the exact signed request being answered.
    pub request_hash: HashOf<CommitCertificateRequest>,
    /// Durable CommitQC recovered from the responder's canonical finality artifact.
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CommitCertificateResponseSignaturePayload {
    /// Sumeragi protocol revision.
    pub protocol_version: u16,
    /// Exact signed request being answered.
    pub request_hash: HashOf<CommitCertificateRequest>,
    /// Exact CommitQC supplied to the authoritative reducer.
    pub certificate: QuorumCertificate,
    /// Current authenticated network identity serving the artifact.
    pub responder: PeerId,
}

/// Payload variants accepted by the Sumeragi v2 network envelope.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    /// Aggregate PrepareQC or CommitQC.
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
    /// Request the durable CommitQC for the active height context.
    CommitCertificateRequest(CommitCertificateRequest),
    /// Response carrying the active height context's durable CommitQC.
    CommitCertificateResponse(CommitCertificateResponse),
}

/// Explicitly versioned Sumeragi v2 network envelope.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct ConsensusMessageV2 {
    /// Protocol version; must equal [`PROTOCOL_VERSION`].
    pub protocol_version: u16,
    /// Canonical v2 message payload.
    pub payload: ConsensusMessageV2Payload,
}

/// High-level reducer phase exported by the compact Sumeragi v2 status API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    /// A CommitQC is durable and the body is awaiting application.
    PendingApply,
}

/// Local availability/application state for the current proposal body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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

/// Compact Norito payload returned by the Sumeragi v2 status endpoint.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    /// Persisted PrepareQC lock, if any.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub locked_prepare_qc: Option<QuorumCertificateRef>,
    /// Highest verified PrepareQC known locally, if any.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub highest_prepare_qc: Option<QuorumCertificateRef>,
    /// Most recently installed timeout certificate, including its view.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_timeout_certificate: Option<TimeoutCertificateRef>,
    /// Local body availability/application state.
    pub body_state: SumeragiV2BodyState,
    /// WAL persistence operation blocking the reducer, if any.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub pending_persistence_id: Option<u64>,
    /// Last locally committed block height.
    pub last_committed_height: Height,
    /// Last locally committed block subject, absent before the first commit.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_committed_subject: Option<BlockSubject>,
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
            if self.last_committed_height != self.height || self.last_committed_subject.is_none() {
                return Err(Error::PendingApplyCommitMismatch);
            }
        } else if self.last_committed_height >= self.height {
            return Err(Error::CommittedHeightNotBehindActiveHeight);
        }
        if self.last_committed_height == 0 && self.last_committed_subject.is_some() {
            return Err(Error::GenesisCommitCarriesSubject);
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
    /// The reducer phase cannot emit the reported body state.
    PhaseBodyMismatch,
    /// Commit collection requires a persisted PrepareQC lock.
    CommitWithoutLock,
    /// Prepare collection cannot retain a prior PrepareQC lock.
    PrepareWithLock,
    /// Pending-apply state did not report the current decided subject and height.
    PendingApplyCommitMismatch,
    /// A non-decided active height reported its commit frontier at or ahead of itself.
    CommittedHeightNotBehindActiveHeight,
    /// The pre-genesis commit frontier carried a block subject.
    GenesisCommitCarriesSubject,
    /// A QC or TC reference was bound to another height context.
    CertificateContextMismatch,
    /// A QC or TC reference was bound to another height.
    CertificateHeightMismatch,
    /// A status QC reference was not a PrepareQC.
    CertificatePhaseMismatch,
    /// A QC reference came from a view above the current view.
    CertificateFromFutureView,
    /// A persisted lock was present without a highest PrepareQC.
    LockedCertificateWithoutHighest,
    /// The persisted lock was above the reported highest PrepareQC.
    LockedCertificateAboveHighest,
    /// Lock and highest references disagreed at the same view.
    ConflictingCertificatesAtSameView,
    /// A timeout certificate did not precede the current view.
    TimeoutNotBeforeCurrentView,
    /// A timeout certificate reported a PrepareQC from above its timed-out view.
    TimeoutCarriesFuturePrepare,
}

impl fmt::Display for SumeragiV2StatusValidationError {
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
                f.write_str("pre-genesis commit frontier cannot carry a subject")
            }
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
    /// Permissioned contexts must assign unit power to every validator.
    PermissionedPowerNotOne,
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
    /// Next-epoch PoPs are not aligned one-for-one with its roster.
    NextEpochProofOfPossessionCount,
    /// A next-epoch roster slot contains no proof of possession.
    MissingNextEpochProofOfPossession,
    /// A next-epoch proof of possession exceeds the protocol bound.
    NextEpochProofOfPossessionTooLarge,
    /// A permissioned next-epoch snapshot assigned non-unit voting power.
    NextEpochPermissionedPowerNotOne,
    /// The parent certificate is not a CommitQC for the previous height.
    InvalidParentCommit,
    /// The audited snapshot bootstrap record or its height/anchor relationship is malformed.
    InvalidSnapshotBootstrap,
    /// The mandatory data-availability layout is internally inconsistent.
    InvalidDataAvailabilityLayout,
    /// A certificate or message is bound to another height context.
    WrongHeightContext,
    /// Signer count cannot be represented on the wire.
    TooManySigners,
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
    /// The advertised Kagemusha top-up count exceeds the consensus bound.
    TooManyKagemushaTopupAnchors,
    /// A top-up execution commitment's combined post root is not canonical.
    ExecutionCommitmentPostRootMismatch,
    /// An aggregate certificate or timeout group carries no aggregate signature.
    MissingAggregateSignature,
    /// A signature or aggregate exceeds the protocol allocation bound.
    SignatureTooLarge,
    /// Too few distinct validators signed.
    InsufficientSignerCount,
    /// Signed voting power is not strictly greater than two thirds.
    InsufficientVotingPower,
    /// A timeout certificate contains no groups.
    EmptyTimeoutCertificate,
    /// A timeout group contains no signatures.
    EmptyTimeoutGroup,
    /// Timeout groups are duplicated or not canonically ordered.
    TimeoutGroupsNotStrictlySorted,
    /// The same validator appears in more than one timeout group.
    OverlappingTimeoutSigners,
    /// A timeout group reported a CommitQC instead of a PrepareQC.
    TimeoutCarriesNonPrepareQc,
    /// A timeout group reported a QC from a future view.
    QcFromFutureView,
    /// Timeout groups report conflicting PrepareQCs from the same view.
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
    /// A certified body request's QC does not certify its requested subject and round.
    CertifiedBodyCertificateMismatch,
    /// Certified body bytes do not match the manifest payload hash.
    CertifiedBodyHashMismatch,
    /// A certified response does not answer the exact outstanding request.
    CertifiedBodyRequestMismatch,
    /// A certified response came from a validator outside the QC signer set.
    ResponderNotCertified,
    /// The authenticated transport sender differs from the claimed responder.
    ResponderIdentityMismatch,
    /// A commit-certificate response did not carry a CommitQC for this context.
    CommitCertificateMismatch,
    /// A commit-certificate response did not answer the exact outstanding request.
    CommitCertificateRequestMismatch,
}

impl fmt::Display for ValidationError {
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
            Self::PermissionedPowerNotOne => {
                f.write_str("permissioned validators must each have voting power one")
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
            Self::NextEpochPermissionedPowerNotOne => {
                f.write_str("permissioned next-epoch validators must each have voting power one")
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
            Self::WrongHeightContext => f.write_str("message is bound to another height context"),
            Self::TooManySigners => f.write_str("signer count exceeds the wire range"),
            Self::SignersNotStrictlySorted => {
                f.write_str("signer indices are not strictly increasing")
            }
            Self::SignerOutOfRange => f.write_str("signer index is outside the voting roster"),
            Self::SignerNotInCertificate => f.write_str("signer is not present in the certificate"),
            Self::MissingSignature => f.write_str("signed message has an empty signature"),
            Self::InvalidExecutionCommitment => {
                f.write_str("execution commitment top-up count/root presence is inconsistent")
            }
            Self::TooManyKagemushaTopupAnchors => {
                f.write_str("execution commitment exceeds the Kagemusha top-up anchor limit")
            }
            Self::ExecutionCommitmentPostRootMismatch => {
                f.write_str("execution commitment post-state root is not canonical")
            }
            Self::MissingAggregateSignature => {
                f.write_str("certificate has an empty aggregate signature")
            }
            Self::SignatureTooLarge => f.write_str("consensus signature exceeds protocol bound"),
            Self::InsufficientSignerCount => {
                f.write_str("insufficient distinct validator signatures")
            }
            Self::InsufficientVotingPower => f.write_str("insufficient signed voting power"),
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
            Self::CertifiedBodyHashMismatch => {
                f.write_str("certified body bytes do not match the manifest payload hash")
            }
            Self::CertifiedBodyRequestMismatch => {
                f.write_str("certified body response does not match the outstanding request")
            }
            Self::ResponderNotCertified => {
                f.write_str("certified body responder is not a certificate signer")
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
        PayloadEncoding::Plain => payload.div_ceil(chunk_size),
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

fn validate_encoded_chunk_len(
    manifest: &PayloadManifest,
    index: usize,
    actual: usize,
) -> Result<(), ValidationError> {
    let chunk_size = usize::try_from(manifest.layout.chunk_size_bytes)
        .map_err(|_| ValidationError::InvalidChunkLength)?;
    if actual == 0 || actual > chunk_size {
        return Err(ValidationError::InvalidChunkLength);
    }
    if manifest.layout.encoding == PayloadEncoding::Plain {
        let offset = index
            .checked_mul(chunk_size)
            .ok_or(ValidationError::InvalidChunkLength)?;
        let payload_size = usize::try_from(manifest.payload_size_bytes)
            .map_err(|_| ValidationError::InvalidChunkLength)?;
        let expected = payload_size.saturating_sub(offset).min(chunk_size);
        if actual != expected {
            return Err(ValidationError::InvalidChunkLength);
        }
    } else if actual != chunk_size {
        return Err(ValidationError::InvalidChunkLength);
    }
    Ok(())
}

fn validated_total_power(roster: &[ValidatorPower]) -> Result<u64, ValidationError> {
    if roster.is_empty() {
        return Err(ValidationError::EmptyRoster);
    }
    if roster.len() > MAX_VALIDATORS_PER_HEIGHT {
        return Err(ValidationError::RosterTooLarge);
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

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use norito::codec::DecodeAll as _;

    use super::*;

    #[test]
    fn global_phase_wire_tags_are_explicit_and_schema_aligned() {
        let prepare = GlobalPhase::Prepare.encode();
        let commit = GlobalPhase::Commit.encode();
        assert_eq!(prepare, u32::from(GlobalPhase::Prepare as u8).to_le_bytes());
        assert_eq!(commit, u32::from(GlobalPhase::Commit as u8).to_le_bytes());
        assert_eq!(prepare, 1_u32.to_le_bytes());
        assert_eq!(commit, 2_u32.to_le_bytes());

        let mut prepare_cursor = prepare.as_slice();
        let mut commit_cursor = commit.as_slice();
        assert_eq!(
            GlobalPhase::decode_all(&mut prepare_cursor).expect("decode Prepare"),
            GlobalPhase::Prepare
        );
        assert_eq!(
            GlobalPhase::decode_all(&mut commit_cursor).expect("decode Commit"),
            GlobalPhase::Commit
        );
        let legacy_implicit_zero_bytes = 0_u32.to_le_bytes();
        let mut legacy_implicit_zero = legacy_implicit_zero_bytes.as_slice();
        assert!(GlobalPhase::decode_all(&mut legacy_implicit_zero).is_err());
    }

    #[test]
    fn execution_commitment_enforces_topup_shape_count_and_combined_root() {
        let parent = Hash::new(b"parent");
        let ordinary = Hash::new(b"ordinary writes");
        let topup = Hash::new(b"topup tree");
        let executed = Hash::new(b"executed block wire");
        let post = ExecutionCommitment::topup_post_state_root(2, ordinary, topup);
        let canonical = ExecutionCommitment::new(parent, post, ordinary, Some(topup), 2, executed)
            .expect("canonical top-up commitment");
        assert_eq!(canonical.validate(), Ok(()));
        assert_eq!(canonical.executed_block_wire_hash, executed);

        let encoded = canonical.encode();
        let mut cursor = encoded.as_slice();
        assert_eq!(
            ExecutionCommitment::decode_all(&mut cursor).expect("decode execution commitment"),
            canonical
        );

        assert_eq!(
            ExecutionCommitment::new(
                parent,
                Hash::new(b"wrong"),
                ordinary,
                Some(topup),
                2,
                executed,
            ),
            Err(ValidationError::ExecutionCommitmentPostRootMismatch)
        );
        assert_eq!(
            ExecutionCommitment::new(parent, post, ordinary, Some(topup), 0, executed),
            Err(ValidationError::InvalidExecutionCommitment)
        );
        assert_eq!(
            ExecutionCommitment::new(
                parent,
                post,
                ordinary,
                Some(topup),
                MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK + 1,
                executed,
            ),
            Err(ValidationError::TooManyKagemushaTopupAnchors)
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn genesis_context_json_uses_nexus_amx_context_name_only() {
        let parameters = SumeragiV2GenesisContextParameters::recommended();
        let json = norito::json::to_json(&parameters).expect("serialize v2 genesis context");
        assert!(json.contains("\"nexus_amx_context_hash\""));
        assert!(!json.contains("active_nexus_lane_hash"));

        let obsolete = json.replace("nexus_amx_context_hash", "active_nexus_lane_hash");
        assert!(
            norito::json::from_str::<SumeragiV2GenesisContextParameters>(&obsolete).is_err(),
            "the unreleased misleading field name must not remain an accepted live schema"
        );

        let unknown = json.replacen('{', "{\"unknown\":1,", 1);
        assert!(
            norito::json::from_str::<SumeragiV2GenesisContextParameters>(&unknown).is_err(),
            "signed v2 genesis context must reject unknown fields"
        );
    }

    fn peer(seed: u8) -> PeerId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive deterministic Sumeragi v2 fixture keypair");
        PeerId::new(key_pair.public_key().clone())
    }

    fn roster(powers: &[u64]) -> Vec<ValidatorPower> {
        let mut validators = (0..powers.len())
            .map(|index| peer(u8::try_from(index + 1).expect("small fixture roster")))
            .collect::<Vec<_>>();
        validators.sort();
        validators
            .into_iter()
            .zip(powers.iter().copied())
            .map(|(validator, power)| ValidatorPower { validator, power })
            .collect()
    }

    fn context(powers: &[u64]) -> HeightContext {
        let roster = roster(powers);
        HeightContext {
            chain_id: ChainId::from("sumeragi-v2-test"),
            protocol_version: PROTOCOL_VERSION,
            height: 1,
            epoch: 2,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Npos,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("valid fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"nexus amx context"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::Plain,
                chunk_size_bytes: 4,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1024,
                max_chunk_count: 256,
            },
            leader_seed: [0xA5; 32],
        }
    }

    fn round(context: &HeightContext, view: View) -> ConsensusRound {
        ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view,
        }
    }

    fn subject(seed: u8) -> BlockSubject {
        BlockSubject {
            parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new([seed, 0]))),
            block_hash: HashOf::from_untyped_unchecked(Hash::new([seed, 1])),
            payload_hash: Hash::new([seed, 2]),
        }
    }

    fn execution_commitment(seed: u8) -> ExecutionCommitment {
        ExecutionCommitment::new(
            Hash::new([seed, 3]),
            Hash::new([seed, 4]),
            Hash::new([seed, 5]),
            None,
            0,
            Hash::new([seed, 6]),
        )
        .expect("canonical fixture execution commitment")
    }

    fn qc(
        context: &HeightContext,
        view: View,
        phase: GlobalPhase,
        signers: Vec<ValidatorIndex>,
    ) -> QuorumCertificate {
        QuorumCertificate {
            round: round(context, view),
            phase,
            subject: subject(u8::try_from(view + 1).expect("small fixture view")),
            execution_commitment: execution_commitment(
                u8::try_from(view + 1).expect("small fixture view"),
            ),
            signers,
            aggregate_signature: vec![0x5A; 48],
        }
    }

    fn manifest(context: &HeightContext) -> PayloadManifest {
        let subject = subject(9);
        PayloadManifest::derive(context, round(context, 1), subject, 4, &[b"body".to_vec()])
            .expect("valid canonical manifest")
    }

    #[test]
    fn dual_quorum_requires_count_and_power() {
        let context = context(&[70, 10, 10, 10]);

        assert_eq!(context.quorum.min_signers, 3);
        assert_eq!(context.validate_signers(&[0, 1, 2]), Ok(()));
        assert_eq!(
            context.validate_signers(&[1, 2, 3]),
            Err(ValidationError::InsufficientVotingPower)
        );
        assert_eq!(
            context.validate_signers(&[0, 1]),
            Err(ValidationError::InsufficientSignerCount)
        );
        assert_eq!(
            context.validate_signers(&[0, 1, 1]),
            Err(ValidationError::SignersNotStrictlySorted)
        );
    }

    #[test]
    fn height_context_rejects_noncanonical_rosters_and_quorums() {
        let mut empty = context(&[1, 1, 1, 1]);
        empty.roster.clear();
        assert_eq!(empty.leader(u64::MAX), 0);
        assert_eq!(empty.validate(), Err(ValidationError::EmptyRoster));

        let mut invalid = context(&[1, 1, 1, 1]);
        invalid.roster[1].validator = invalid.roster[0].validator.clone();
        assert_eq!(invalid.validate(), Err(ValidationError::DuplicateValidator));

        let mut invalid = context(&[1, 1, 1, 1]);
        invalid.quorum.min_signers = 2;
        assert_eq!(
            invalid.validate(),
            Err(ValidationError::CountThresholdMismatch)
        );

        let mut oversized = context(&[1, 1, 1, 1]);
        let repeated = oversized.roster[0].clone();
        oversized
            .roster
            .resize(MAX_VALIDATORS_PER_HEIGHT + 1, repeated);
        assert_eq!(oversized.validate(), Err(ValidationError::RosterTooLarge));

        let mut invalid_parent_execution = context(&[1, 1, 1, 1]);
        invalid_parent_execution.height = 2;
        invalid_parent_execution.parent_commit_qc = Some(QuorumCertificate {
            round: ConsensusRound {
                context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                    b"invalid parent execution context",
                ))),
                height: 1,
                view: 0,
            },
            phase: GlobalPhase::Commit,
            subject: subject(0x61),
            execution_commitment: ExecutionCommitment {
                parent_state_root: Hash::new(b"parent state"),
                post_state_root: Hash::new(b"post state"),
                ordinary_writes_root: Hash::new(b"ordinary writes"),
                topup_anchor_root: None,
                topup_anchor_count: 1,
                executed_block_wire_hash: Hash::new(b"executed block wire"),
            },
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x62; 48],
        });
        assert_eq!(
            invalid_parent_execution.validate(),
            Err(ValidationError::InvalidExecutionCommitment)
        );
    }

    #[test]
    fn snapshot_bootstrap_is_an_explicit_mutually_exclusive_parent_authority() {
        let mut anchored = context(&[1, 1, 1, 1]);
        anchored.height = 11;
        anchored.snapshot_bootstrap = Some(SnapshotBootstrapAnchor {
            snapshot_height: 10,
            snapshot_block_hash: HashOf::from_untyped_unchecked(Hash::new(b"audited snapshot tip")),
            snapshot_block_creation_time_ms: 1_000,
            snapshot_state_hash: Hash::new(b"audited snapshot WSV"),
        });
        anchored
            .validate()
            .expect("exact post-snapshot context is structurally valid");
        let record = SnapshotV2BootstrapRecord {
            version: SnapshotV2BootstrapRecord::VERSION,
            context: anchored.clone(),
            validator_set_pops: vec![vec![0xA5]; anchored.roster.len()],
        };
        record.validate().expect("complete bootstrap record");

        let mut wrong_height = record.clone();
        wrong_height.context.height = 12;
        assert_eq!(
            wrong_height.validate(),
            Err(ValidationError::InvalidParentCommit)
        );

        let mut ambiguous = anchored;
        ambiguous.parent_commit_qc = Some(qc(
            &context(&[1, 1, 1, 1]),
            0,
            GlobalPhase::Commit,
            vec![0, 1, 2],
        ));
        assert_eq!(
            ambiguous.validate(),
            Err(ValidationError::InvalidParentCommit)
        );

        let mut unsupported = record;
        unsupported.version = SnapshotV2BootstrapRecord::VERSION + 1;
        assert_eq!(
            unsupported.validate(),
            Err(ValidationError::InvalidSnapshotBootstrap)
        );
    }

    #[test]
    fn non_boundary_height_context_id_is_pinned() {
        let context = context(&[7, 5, 3, 1]);
        context.validate().expect("valid non-boundary context");
        assert_eq!(
            *context.id().0.as_ref(),
            [
                0xad, 0x99, 0x8b, 0x5a, 0x9f, 0x19, 0xea, 0x89, 0xdf, 0xb4, 0x3c, 0xdc, 0x9d, 0xdd,
                0xb3, 0xf5, 0x10, 0x91, 0x64, 0xc0, 0xb4, 0x97, 0xa2, 0xfb, 0x8e, 0x67, 0x26, 0x81,
                0xea, 0x7e, 0x21, 0x9b,
            ],
            "intentional identity-projection changes require updating this golden"
        );
    }

    #[test]
    fn boundary_height_context_id_pins_the_complete_transition() {
        let mut context = context(&[7, 5, 3, 1]);
        context.epoch_end_height = context.height;
        let next_roster = roster(&[11, 9, 7, 5]);
        context.next_epoch_snapshot = Some(finality::FinalizedNextEpochSnapshot {
            epoch: context.epoch + 1,
            epoch_end_height: 41,
            mode: context.mode,
            quorum: DualQuorum::from_roster(&next_roster).expect("valid next-epoch quorum"),
            roster: next_roster,
            validator_set_pops: vec![vec![0x81], vec![0x82, 0x83], vec![0x84], vec![0x85, 0x86]],
            leader_seed: [0x87; 32],
        });
        context.validate().expect("valid boundary context");
        assert_eq!(
            *context.id().0.as_ref(),
            [
                0xfa, 0xc2, 0x0e, 0xa9, 0xbb, 0xf7, 0xba, 0xe7, 0x7e, 0xee, 0x55, 0xec, 0xbe, 0x68,
                0x98, 0x95, 0xf9, 0x8f, 0x35, 0x0b, 0xcb, 0x9b, 0x05, 0x8d, 0x99, 0xce, 0x05, 0x07,
                0x78, 0x6a, 0xa9, 0x55,
            ],
            "intentional transition-identity changes require updating this golden"
        );
    }

    #[test]
    fn height_context_id_ignores_equivalent_parent_qc_signer_subsets() {
        let mut left = context(&[1, 1, 1, 1]);
        left.height = 2;
        let parent_round = ConsensusRound {
            context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"parent context",
            ))),
            height: left.height - 1,
            view: 3,
        };
        let parent_subject = subject(0x44);
        left.parent_commit_qc = Some(QuorumCertificate {
            round: parent_round,
            phase: GlobalPhase::Commit,
            subject: parent_subject,
            execution_commitment: execution_commitment(0x44),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x11; 48],
        });
        let mut right = left.clone();
        right.parent_commit_qc = Some(QuorumCertificate {
            round: ConsensusRound {
                view: parent_round.view + 1,
                ..parent_round
            },
            phase: GlobalPhase::Commit,
            subject: parent_subject,
            execution_commitment: execution_commitment(0x44),
            signers: vec![0, 1, 3],
            aggregate_signature: vec![0x22; 48],
        });

        assert_ne!(left.parent_commit_qc, right.parent_commit_qc);
        assert_eq!(left.id(), right.id());

        let mut different_execution = right.clone();
        different_execution
            .parent_commit_qc
            .as_mut()
            .expect("parent certificate")
            .execution_commitment = execution_commitment(0x45);
        assert_ne!(left.id(), different_execution.id());

        let mut different_subject = right.clone();
        different_subject
            .parent_commit_qc
            .as_mut()
            .expect("parent certificate")
            .subject = subject(0x45);
        assert_ne!(left.id(), different_subject.id());

        right
            .parent_commit_qc
            .as_mut()
            .expect("parent certificate")
            .round
            .context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"different parent context",
        )));
        assert_ne!(left.id(), right.id());

        let mut oversized_parent = left;
        oversized_parent
            .parent_commit_qc
            .as_mut()
            .expect("parent certificate")
            .aggregate_signature = vec![0x33; MAX_CONSENSUS_SIGNATURE_BYTES + 1];
        assert_eq!(
            oversized_parent.validate(),
            Err(ValidationError::SignatureTooLarge)
        );
    }

    #[test]
    fn timeout_certificate_requires_disjoint_dual_quorum() {
        let context = context(&[1, 1, 1, 1]);
        let prepare = qc(&context, 1, GlobalPhase::Prepare, vec![0, 1, 2]);
        let certificate = TimeoutCertificate {
            round: round(&context, 2),
            groups: vec![
                TimeoutVoteGroup {
                    highest_prepare_qc: None,
                    signers: vec![0],
                    aggregate_signature: vec![1],
                },
                TimeoutVoteGroup {
                    highest_prepare_qc: Some(prepare.clone()),
                    signers: vec![1, 2],
                    aggregate_signature: vec![2],
                },
            ],
        };
        assert_eq!(certificate.validate(&context), Ok(()));
        assert_eq!(certificate.highest_prepare_qc(), Some(&prepare));

        let mut overlapping = certificate.clone();
        overlapping.groups[1].signers = vec![0, 2];
        assert_eq!(
            overlapping.validate(&context),
            Err(ValidationError::OverlappingTimeoutSigners)
        );
    }

    #[test]
    fn highest_prepare_qc_uses_view_then_semantic_reference() {
        let context = context(&[1, 1, 1, 1]);
        let lower = qc(&context, 1, GlobalPhase::Prepare, vec![0, 1, 2]);
        let higher = qc(&context, 3, GlobalPhase::Prepare, vec![0, 1, 2]);
        let certificate = TimeoutCertificate {
            round: round(&context, 4),
            groups: vec![
                TimeoutVoteGroup {
                    highest_prepare_qc: Some(lower),
                    signers: vec![0],
                    aggregate_signature: vec![1],
                },
                TimeoutVoteGroup {
                    highest_prepare_qc: Some(higher.clone()),
                    signers: vec![1, 2],
                    aggregate_signature: vec![2],
                },
            ],
        };
        assert_eq!(certificate.highest_prepare_qc(), Some(&higher));
    }

    #[test]
    fn timeout_certificate_rejects_conflicting_prepare_qcs_at_one_view() {
        let context = context(&[1, 1, 1, 1]);
        let left = qc(&context, 1, GlobalPhase::Prepare, vec![0, 1, 2]);
        let mut right = left.clone();
        right.subject = subject(0x7E);
        right.aggregate_signature = vec![0x7E; 48];
        let mut groups = vec![
            TimeoutVoteGroup {
                highest_prepare_qc: Some(left),
                signers: vec![0],
                aggregate_signature: vec![1],
            },
            TimeoutVoteGroup {
                highest_prepare_qc: Some(right),
                signers: vec![1, 2],
                aggregate_signature: vec![2],
            },
        ];
        groups.sort_by_key(|group| {
            group
                .highest_prepare_qc
                .as_ref()
                .map(QuorumCertificate::as_ref)
        });
        let certificate = TimeoutCertificate {
            round: round(&context, 2),
            groups,
        };

        assert_eq!(
            certificate.validate(&context),
            Err(ValidationError::ConflictingHighestPrepare)
        );
    }

    #[test]
    fn qc_reference_and_timeout_preimage_ignore_equivalent_quorum_subsets() {
        let context = context(&[1, 1, 1, 1]);
        let left = qc(&context, 1, GlobalPhase::Prepare, vec![0, 1, 2]);
        let right = qc(&context, 1, GlobalPhase::Prepare, vec![0, 1, 3]);
        assert_ne!(HashOf::new(&left), HashOf::new(&right));
        assert_eq!(left.as_ref(), right.as_ref());

        let left_vote = TimeoutVote {
            round: round(&context, 2),
            highest_prepare_qc: Some(left),
            signer: 0,
            signature: vec![1],
        };
        let right_vote = TimeoutVote {
            round: round(&context, 2),
            highest_prepare_qc: Some(right),
            signer: 1,
            signature: vec![2],
        };
        assert_eq!(
            left_vote.signature_preimage(),
            right_vote.signature_preimage()
        );
    }

    #[test]
    fn v2_envelope_norito_roundtrip() {
        let context = context(&[1, 1, 1, 1]);
        let manifest = manifest(&context);
        let proposal = Proposal {
            round: manifest.round,
            proposer: 2,
            subject: manifest.subject,
            manifest,
            justification: ProposalJustification::ParentCommit(ParentCommitJustification {
                certificate: None,
            }),
            signature: vec![0x22; 48],
        };
        let message = ConsensusMessageV2::new(ConsensusMessageV2Payload::Proposal(proposal));

        let encoded = message.encode();
        let decoded = ConsensusMessageV2::decode(&mut &encoded[..])
            .expect("decode canonical Sumeragi v2 envelope");
        assert_eq!(decoded, message);
        assert_eq!(decoded.validate_version(), Ok(()));
    }

    #[test]
    fn every_v2_payload_variant_roundtrips() {
        let context = context(&[1, 1, 1, 1]);
        let prepare = qc(&context, 1, GlobalPhase::Prepare, vec![0, 1, 2]);
        let timeout = TimeoutCertificate {
            round: round(&context, 2),
            groups: vec![TimeoutVoteGroup {
                highest_prepare_qc: Some(prepare.clone()),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x33; 48],
            }],
        };
        let manifest = manifest(&context);
        let request = CertifiedBodyRequest {
            round: manifest.round,
            subject: manifest.subject,
            certificate: prepare.clone(),
            requester: context.roster[3].validator.clone(),
            signature: vec![0x44; 48],
        };
        let commit = QuorumCertificate {
            phase: GlobalPhase::Commit,
            ..prepare.clone()
        };
        let commit_request = CommitCertificateRequest {
            protocol_version: PROTOCOL_VERSION,
            chain_id: context.chain_id.clone(),
            context_id: context.id(),
            height: context.height,
            requester: context.roster[3].validator.clone(),
            signature: vec![0x45; 48],
        };
        let proposal = Proposal {
            round: manifest.round,
            proposer: 2,
            subject: manifest.subject,
            manifest: manifest.clone(),
            justification: ProposalJustification::Timeout(TimeoutJustification {
                timeout_certificate: timeout.clone(),
                highest_prepare_qc: Some(prepare.clone()),
            }),
            signature: vec![0x55; 48],
        };
        let variants = vec![
            ConsensusMessageV2Payload::Proposal(proposal),
            ConsensusMessageV2Payload::Vote(Vote {
                round: manifest.round,
                phase: GlobalPhase::Prepare,
                subject: manifest.subject,
                execution_commitment: prepare.execution_commitment,
                signer: 0,
                signature: vec![1],
            }),
            ConsensusMessageV2Payload::QuorumCertificate(prepare.clone()),
            ConsensusMessageV2Payload::TimeoutVote(TimeoutVote {
                round: timeout.round,
                highest_prepare_qc: Some(prepare.clone()),
                signer: 0,
                signature: vec![2],
            }),
            ConsensusMessageV2Payload::TimeoutCertificate(timeout),
            ConsensusMessageV2Payload::PayloadManifest(manifest.clone()),
            ConsensusMessageV2Payload::PayloadChunk(PayloadChunk {
                manifest_hash: HashOf::new(&manifest),
                index: 0,
                bytes: b"body".to_vec(),
                sender: 0,
                signature: vec![0x66; 48],
            }),
            ConsensusMessageV2Payload::CertifiedBodyRequest(request.clone()),
            ConsensusMessageV2Payload::CertifiedBodyResponse(CertifiedBodyResponse {
                request_hash: HashOf::new(&request),
                manifest,
                body: b"body".to_vec(),
                responder: 0,
                signature: vec![3],
            }),
            ConsensusMessageV2Payload::CommitCertificateRequest(commit_request.clone()),
            ConsensusMessageV2Payload::CommitCertificateResponse(CommitCertificateResponse {
                request_hash: HashOf::new(&commit_request),
                certificate: commit,
                responder: context.roster[0].validator.clone(),
                signature: vec![4],
            }),
        ];

        for payload in variants {
            let message = ConsensusMessageV2::new(payload);
            let encoded = message.encode();
            let decoded = ConsensusMessageV2::decode(&mut &encoded[..])
                .expect("decode Sumeragi v2 payload variant");
            assert_eq!(decoded, message);
        }
    }

    #[test]
    fn voting_power_sum_fails_closed_on_u64_overflow() {
        let mut roster = vec![
            ValidatorPower {
                validator: peer(1),
                power: u64::MAX,
            },
            ValidatorPower {
                validator: peer(2),
                power: 1,
            },
        ];
        roster.sort();
        assert_eq!(
            DualQuorum::from_roster(&roster),
            Err(ValidationError::VotingPowerOverflow)
        );
    }

    #[test]
    fn signed_payload_chunk_binds_session_and_manifest_fields() {
        let context = context(&[1, 1, 1, 1]);
        let manifest = manifest(&context);
        let chunk = PayloadChunk {
            manifest_hash: HashOf::new(&manifest),
            index: 0,
            bytes: b"body".to_vec(),
            sender: 1,
            signature: vec![0x77; 48],
        };

        let payload = chunk
            .signature_payload(&context, &manifest)
            .expect("valid chunk signature payload");
        assert_eq!(payload.context_id, context.id());
        assert_eq!(payload.epoch, context.epoch);
        assert_eq!(payload.height, context.height);
        assert_eq!(payload.view, manifest.round.view);
        assert_eq!(payload.subject, manifest.subject);
        assert_eq!(payload.total_chunks, 1);
        assert_eq!(payload.chunk_hash, Hash::new(b"body"));
        assert!(
            chunk
                .signature_preimage(&context, &manifest)
                .expect("valid signature preimage")
                .starts_with(b"iroha:sumeragi:v2:payload-chunk")
        );

        let mut unsigned = chunk.clone();
        unsigned.signature.clear();
        assert!(unsigned.signature_preimage(&context, &manifest).is_ok());
        assert_eq!(
            unsigned.validate(&context, &manifest),
            Err(ValidationError::MissingChunkSignature)
        );

        let mut corrupted = chunk.clone();
        corrupted.bytes.push(0);
        assert_eq!(
            corrupted.signature_payload(&context, &manifest),
            Err(ValidationError::InvalidChunkLength)
        );
    }

    #[test]
    fn manifest_rejects_mutated_root_size_count_and_chunk_length() {
        let context = context(&[1, 1, 1, 1]);
        let canonical = manifest(&context);
        assert_eq!(canonical.validate(&context), Ok(()));

        let mut wrong_root = canonical.clone();
        wrong_root.chunk_root = Hash::new(b"not the canonical root");
        assert_eq!(
            wrong_root.validate(&context),
            Err(ValidationError::ChunkRootMismatch)
        );

        let mut wrong_count = canonical.clone();
        wrong_count.payload_size_bytes = 5;
        assert_eq!(
            wrong_count.validate(&context),
            Err(ValidationError::PayloadSizeMismatch)
        );

        let mut oversized = canonical.clone();
        oversized.payload_size_bytes = context.da_layout.max_payload_size_bytes + 1;
        assert_eq!(
            oversized.validate(&context),
            Err(ValidationError::PayloadTooLarge)
        );

        let short_chunk = PayloadChunk {
            manifest_hash: HashOf::new(&canonical),
            index: 0,
            bytes: b"bod".to_vec(),
            sender: 0,
            signature: vec![0x44; 48],
        };
        assert_eq!(
            short_chunk.validate(&context, &canonical),
            Err(ValidationError::InvalidChunkLength)
        );
    }

    #[test]
    fn compact_v2_status_norito_roundtrip() {
        let context = context(&[1, 1, 1, 1]);
        let prepare = qc(&context, 1, GlobalPhase::Prepare, vec![0, 1, 2]);
        let timeout = TimeoutCertificate {
            round: round(&context, 2),
            groups: vec![TimeoutVoteGroup {
                highest_prepare_qc: Some(prepare.clone()),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x88; 48],
            }],
        };
        let status = SumeragiV2Status {
            protocol_version: PROTOCOL_VERSION,
            node_fingerprint: Hash::new(b"node"),
            build_fingerprint: Hash::new(b"build"),
            config_fingerprint: Hash::new(b"config"),
            restart_required: false,
            height_context_id: context.id(),
            height: context.height,
            view: 3,
            phase: SumeragiV2StatusPhase::Prepare,
            leader: 2,
            locked_prepare_qc: Some(prepare.as_ref()),
            highest_prepare_qc: Some(prepare.as_ref()),
            last_timeout_certificate: Some(timeout.as_ref()),
            body_state: SumeragiV2BodyState::Validated,
            pending_persistence_id: Some(17),
            last_committed_height: context.height - 1,
            last_committed_subject: Some(prepare.subject),
        };

        let encoded = status.encode();
        let decoded =
            SumeragiV2Status::decode(&mut &encoded[..]).expect("decode compact Sumeragi v2 status");
        assert_eq!(decoded, status);
    }

    #[test]
    fn leader_rotation_is_power_independent_and_wraps_roster() {
        let equal = context(&[1, 1, 1, 1]);
        let weighted = context(&[70, 10, 10, 10]);
        let start = equal.leader(0);

        assert_eq!(weighted.leader(0), start);
        assert_eq!(equal.leader(4), start);
        assert_eq!(weighted.leader(17), equal.leader(17));
        assert_eq!(
            (0..4)
                .map(|view| equal.leader(view))
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([0, 1, 2, 3])
        );
    }

    #[test]
    fn signed_control_messages_have_canonical_domain_separated_preimages() {
        let context = context(&[1, 1, 1, 1]);
        let proposal_round = round(&context, 0);
        let mut manifest = manifest(&context);
        manifest.round = proposal_round;
        let proposal = Proposal {
            round: proposal_round,
            proposer: context.leader(0),
            subject: manifest.subject,
            manifest: manifest.clone(),
            justification: ProposalJustification::ParentCommit(ParentCommitJustification {
                certificate: None,
            }),
            signature: vec![0x11; 48],
        };
        assert_eq!(proposal.validate(&context), Ok(()));
        assert!(
            proposal
                .signature_preimage()
                .starts_with(b"iroha:sumeragi:v2:proposal")
        );
        let mut changed_signature = proposal.clone();
        changed_signature.signature = vec![0x22; 48];
        assert_eq!(
            changed_signature.signature_preimage(),
            proposal.signature_preimage()
        );

        let vote = Vote {
            round: proposal_round,
            phase: GlobalPhase::Prepare,
            subject: proposal.subject,
            execution_commitment: execution_commitment(0x33),
            signer: 0,
            signature: vec![0x33; 48],
        };
        assert_eq!(vote.validate(&context), Ok(()));
        assert!(
            vote.signature_preimage()
                .starts_with(b"iroha:sumeragi:v2:vote")
        );
        let mut different_execution = vote.clone();
        different_execution.execution_commitment = execution_commitment(0x34);
        assert_ne!(
            different_execution.signature_preimage(),
            vote.signature_preimage(),
            "vote signatures must authenticate the deterministic execution result"
        );

        let timeout = TimeoutVote {
            round: proposal_round,
            highest_prepare_qc: None,
            signer: 1,
            signature: vec![0x44; 48],
        };
        assert_eq!(timeout.validate(&context), Ok(()));
        assert!(
            timeout
                .signature_preimage()
                .starts_with(b"iroha:sumeragi:v2:timeout-vote")
        );

        let mut oversized = vote.clone();
        oversized.signature = vec![0x45; MAX_CONSENSUS_SIGNATURE_BYTES + 1];
        assert_eq!(
            oversized.validate(&context),
            Err(ValidationError::SignatureTooLarge)
        );

        let mut unsigned = vote;
        unsigned.signature.clear();
        assert_eq!(
            unsigned.validate(&context),
            Err(ValidationError::MissingSignature)
        );
    }

    #[test]
    fn view_zero_proposal_accepts_equivalent_parent_finality_across_views() {
        let mut context = context(&[1, 1, 1, 1]);
        context.height = 2;
        let parent_subject = subject(0x70);
        let parent_round = ConsensusRound {
            context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"proposal parent context",
            ))),
            height: context.height - 1,
            view: 2,
        };
        context.parent_commit_qc = Some(QuorumCertificate {
            round: parent_round,
            phase: GlobalPhase::Commit,
            subject: parent_subject,
            execution_commitment: execution_commitment(0x70),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0x31; 48],
        });
        let proposal_round = round(&context, 0);
        let mut payload_manifest = manifest(&context);
        payload_manifest.round = proposal_round;
        let carried = QuorumCertificate {
            round: parent_round,
            phase: GlobalPhase::Commit,
            subject: parent_subject,
            execution_commitment: execution_commitment(0x70),
            signers: vec![0, 1, 3],
            aggregate_signature: vec![0x32; 48],
        };
        let frozen_parent = context
            .parent_commit_qc
            .as_ref()
            .expect("fixture parent certificate");
        assert!(
            carried
                .as_ref()
                .same_commit_decision(frozen_parent.as_ref())
        );
        let mut prepare_ref = carried.as_ref();
        prepare_ref.phase = GlobalPhase::Prepare;
        assert!(!prepare_ref.same_commit_decision(frozen_parent.as_ref()));
        let mut proposal = Proposal {
            round: proposal_round,
            proposer: context.leader(0),
            subject: payload_manifest.subject,
            manifest: payload_manifest,
            justification: ProposalJustification::ParentCommit(ParentCommitJustification {
                certificate: Some(carried),
            }),
            signature: vec![0x33; 48],
        };

        assert_eq!(proposal.validate(&context), Ok(()));
        if let ProposalJustification::ParentCommit(parent) = &mut proposal.justification {
            parent
                .certificate
                .as_mut()
                .expect("carried parent certificate")
                .round
                .view += 1;
        } else {
            unreachable!("fixture uses a parent justification")
        }
        assert_eq!(
            proposal.validate(&context),
            Ok(()),
            "the same parent subject may have a valid CommitQC in another view"
        );
        if let ProposalJustification::ParentCommit(parent) = &mut proposal.justification {
            let carried = parent
                .certificate
                .as_mut()
                .expect("carried parent certificate");
            carried.round = parent_round;
            carried.round.context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"different proposal parent context",
            )));
        } else {
            unreachable!("fixture uses a parent justification")
        }
        assert_eq!(
            proposal.validate(&context),
            Err(ValidationError::InvalidProposalJustification)
        );
        if let ProposalJustification::ParentCommit(parent) = &mut proposal.justification {
            let carried = parent
                .certificate
                .as_mut()
                .expect("carried parent certificate");
            carried.round = parent_round;
            carried.subject = subject(0x71);
        } else {
            unreachable!("fixture uses a parent justification")
        }
        assert_eq!(
            proposal.validate(&context),
            Err(ValidationError::InvalidProposalJustification)
        );
    }

    #[test]
    fn certified_body_response_binds_request_manifest_and_body_hash() {
        let context = context(&[1, 1, 1, 1]);
        let body = b"certified body".to_vec();
        let round = round(&context, 1);
        let mut response_subject = subject(9);
        response_subject.payload_hash = Hash::new(&body);
        let chunks = body.chunks(4).map(<[u8]>::to_vec).collect::<Vec<_>>();
        let manifest = PayloadManifest::derive(
            &context,
            round,
            response_subject,
            u64::try_from(body.len()).expect("small body"),
            &chunks,
        )
        .expect("valid response manifest");
        let request = CertifiedBodyRequest {
            round: manifest.round,
            subject: manifest.subject,
            certificate: QuorumCertificate {
                round: manifest.round,
                phase: GlobalPhase::Prepare,
                subject: manifest.subject,
                execution_commitment: execution_commitment(9),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0x55; 48],
            },
            requester: context.roster[3].validator.clone(),
            signature: vec![0x66; 48],
        };
        assert_eq!(request.validate(&context), Ok(()));
        let observer_request = CertifiedBodyRequest {
            requester: peer(99),
            ..request.clone()
        };
        assert_eq!(observer_request.validate(&context), Ok(()));
        let response = CertifiedBodyResponse {
            request_hash: HashOf::new(&request),
            manifest,
            body,
            responder: 0,
            signature: vec![0x77; 48],
        };
        assert_eq!(response.validate(&context), Ok(()));
        assert_eq!(
            response.validate_against(&context, &request, &context.roster[0].validator),
            Ok(())
        );
        assert_eq!(
            response.validate_against(&context, &request, &context.roster[1].validator),
            Err(ValidationError::ResponderIdentityMismatch)
        );
        let mut uncertified = response.clone();
        uncertified.responder = 3;
        assert_eq!(
            uncertified.validate_against(&context, &request, &context.roster[3].validator),
            Err(ValidationError::ResponderNotCertified)
        );
        let mut wrong_request = response.clone();
        wrong_request.request_hash = HashOf::from_untyped_unchecked(Hash::new(b"wrong request"));
        assert_eq!(
            wrong_request.validate_against(&context, &request, &context.roster[0].validator),
            Err(ValidationError::CertifiedBodyRequestMismatch)
        );
        assert!(
            response
                .signature_preimage()
                .starts_with(b"iroha:sumeragi:v2:certified-body-response")
        );

        let mut corrupted = response;
        corrupted.body.push(0);
        assert_eq!(
            corrupted.validate(&context),
            Err(ValidationError::CertifiedBodyHashMismatch)
        );
    }

    #[test]
    fn commit_certificate_discovery_binds_chain_context_request_and_commit_phase() {
        let context = context(&[1, 1, 1, 1]);
        let commit = qc(&context, 9, GlobalPhase::Commit, vec![0, 1, 2]);
        let request = CommitCertificateRequest {
            protocol_version: PROTOCOL_VERSION,
            chain_id: context.chain_id.clone(),
            context_id: context.id(),
            height: context.height,
            requester: peer(99),
            signature: vec![0x81; 48],
        };
        assert_eq!(request.validate(&context), Ok(()));
        assert!(
            request
                .signature_preimage()
                .starts_with(b"iroha:sumeragi:v2:commit-certificate-request")
        );

        let response = CommitCertificateResponse {
            request_hash: HashOf::new(&request),
            certificate: commit.clone(),
            responder: peer(100),
            signature: vec![0x82; 48],
        };
        assert_eq!(response.validate_against(&context, &request), Ok(()));
        assert!(
            response
                .signature_preimage()
                .starts_with(b"iroha:sumeragi:v2:commit-certificate-response")
        );

        let mut cross_chain = request.clone();
        cross_chain.chain_id = ChainId::from("other-chain");
        assert_eq!(
            cross_chain.validate(&context),
            Err(ValidationError::WrongHeightContext)
        );
        let mut wrong_height = request.clone();
        wrong_height.height += 1;
        assert_eq!(
            wrong_height.validate(&context),
            Err(ValidationError::WrongHeightContext)
        );
        let mut wrong_protocol = request.clone();
        wrong_protocol.protocol_version += 1;
        assert!(matches!(
            wrong_protocol.validate(&context),
            Err(ValidationError::UnsupportedProtocolVersion { .. })
        ));

        let mut wrong_request = response.clone();
        wrong_request.request_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"another exact request"));
        assert_eq!(
            wrong_request.validate_against(&context, &request),
            Err(ValidationError::CommitCertificateRequestMismatch)
        );
        let mut prepare = response;
        prepare.certificate.phase = GlobalPhase::Prepare;
        assert_eq!(
            prepare.validate(&context),
            Err(ValidationError::CommitCertificateMismatch)
        );

        let mut changed_responder = CommitCertificateResponse {
            request_hash: HashOf::new(&request),
            certificate: commit,
            responder: peer(100),
            signature: vec![0x82; 48],
        };
        let original_preimage = changed_responder.signature_preimage();
        changed_responder.responder = peer(101);
        assert_ne!(changed_responder.signature_preimage(), original_preimage);
    }

    fn status(context: &HeightContext) -> SumeragiV2Status {
        SumeragiV2Status {
            protocol_version: PROTOCOL_VERSION,
            node_fingerprint: Hash::new(b"status-node"),
            build_fingerprint: Hash::new(b"status-build"),
            config_fingerprint: Hash::new(b"status-config"),
            restart_required: false,
            height_context_id: context.id(),
            height: context.height,
            view: 3,
            phase: SumeragiV2StatusPhase::AwaitingProposal,
            leader: 0,
            locked_prepare_qc: None,
            highest_prepare_qc: None,
            last_timeout_certificate: None,
            body_state: SumeragiV2BodyState::Missing,
            pending_persistence_id: None,
            last_committed_height: 0,
            last_committed_subject: None,
        }
    }

    #[test]
    fn status_validation_rejects_impossible_scalar_and_phase_states() {
        use SumeragiV2StatusValidationError as Error;

        let context = context(&[1, 1, 1, 1]);
        let baseline = status(&context);
        assert_eq!(baseline.validate(), Ok(()));

        let mut wrong_protocol = baseline.clone();
        wrong_protocol.protocol_version += 1;
        assert!(matches!(
            wrong_protocol.validate(),
            Err(Error::UnsupportedProtocolVersion { .. })
        ));

        let mut wrong_body = baseline.clone();
        wrong_body.body_state = SumeragiV2BodyState::Validated;
        assert_eq!(wrong_body.validate(), Err(Error::PhaseBodyMismatch));

        let mut commit_without_lock = baseline.clone();
        commit_without_lock.phase = SumeragiV2StatusPhase::Commit;
        commit_without_lock.body_state = SumeragiV2BodyState::Validated;
        assert_eq!(
            commit_without_lock.validate(),
            Err(Error::CommitWithoutLock)
        );

        let mut zero_persistence = baseline.clone();
        zero_persistence.pending_persistence_id = Some(0);
        assert_eq!(zero_persistence.validate(), Err(Error::ZeroPersistenceId));

        let mut committed_ahead = baseline.clone();
        committed_ahead.last_committed_height = committed_ahead.height;
        assert_eq!(
            committed_ahead.validate(),
            Err(Error::CommittedHeightNotBehindActiveHeight)
        );

        let mut pending_apply = baseline;
        pending_apply.phase = SumeragiV2StatusPhase::PendingApply;
        pending_apply.body_state = SumeragiV2BodyState::PendingApply;
        assert_eq!(
            pending_apply.validate(),
            Err(Error::PendingApplyCommitMismatch)
        );
        pending_apply.last_committed_height = pending_apply.height;
        pending_apply.last_committed_subject = Some(subject(90));
        assert_eq!(pending_apply.validate(), Ok(()));
    }

    #[test]
    fn status_validation_rejects_cross_context_and_future_certificates() {
        use SumeragiV2StatusValidationError as Error;

        let context = context(&[1, 1, 1, 1]);
        let baseline = status(&context);
        let prepare = qc(&context, 2, GlobalPhase::Prepare, vec![0, 1, 2]).as_ref();

        let mut with_certificates = baseline.clone();
        with_certificates.locked_prepare_qc = Some(prepare);
        with_certificates.highest_prepare_qc = Some(prepare);
        assert_eq!(with_certificates.validate(), Ok(()));

        let mut prepare_with_lock = with_certificates.clone();
        prepare_with_lock.phase = SumeragiV2StatusPhase::Prepare;
        prepare_with_lock.body_state = SumeragiV2BodyState::Validated;
        assert_eq!(prepare_with_lock.validate(), Err(Error::PrepareWithLock));

        let mut conflicting_same_view = with_certificates.clone();
        conflicting_same_view
            .highest_prepare_qc
            .as_mut()
            .unwrap()
            .subject = subject(91);
        assert_eq!(
            conflicting_same_view.validate(),
            Err(Error::ConflictingCertificatesAtSameView)
        );

        let mut missing_highest = with_certificates.clone();
        missing_highest.highest_prepare_qc = None;
        assert_eq!(
            missing_highest.validate(),
            Err(Error::LockedCertificateWithoutHighest)
        );

        let mut wrong_phase = with_certificates.clone();
        wrong_phase.highest_prepare_qc.as_mut().unwrap().phase = GlobalPhase::Commit;
        assert_eq!(wrong_phase.validate(), Err(Error::CertificatePhaseMismatch));

        let mut wrong_context = with_certificates.clone();
        wrong_context
            .highest_prepare_qc
            .as_mut()
            .unwrap()
            .round
            .context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"wrong-status-context",
        )));
        assert_eq!(
            wrong_context.validate(),
            Err(Error::CertificateContextMismatch)
        );

        let mut future = with_certificates.clone();
        future.highest_prepare_qc.as_mut().unwrap().round.view = future.view + 1;
        assert_eq!(future.validate(), Err(Error::CertificateFromFutureView));

        let mut timeout_not_past = baseline;
        timeout_not_past.last_timeout_certificate = Some(TimeoutCertificateRef {
            round: round(&context, timeout_not_past.view),
            highest_prepare_qc: Some(prepare),
            certificate_hash: HashOf::from_untyped_unchecked(Hash::new(b"status-timeout")),
        });
        assert_eq!(
            timeout_not_past.validate(),
            Err(Error::TimeoutNotBeforeCurrentView)
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn status_and_consensus_envelope_json_reject_unknown_nested_fields() {
        let context = context(&[1, 1, 1, 1]);
        let mut snapshot = status(&context);
        snapshot.highest_prepare_qc =
            Some(qc(&context, 2, GlobalPhase::Prepare, vec![0, 1, 2]).as_ref());

        let mut top = norito::json::to_value(&snapshot).expect("serialize status");
        top.as_object_mut()
            .expect("status object")
            .insert("unknown".to_owned(), norito::json::Value::Bool(true));
        assert!(norito::json::from_value::<SumeragiV2Status>(top).is_err());

        let mut nested = norito::json::to_value(&snapshot).expect("serialize status");
        nested
            .as_object_mut()
            .expect("status object")
            .get_mut("highest_prepare_qc")
            .and_then(norito::json::Value::as_object_mut)
            .expect("QC reference object")
            .insert("unknown".to_owned(), norito::json::Value::Bool(true));
        assert!(norito::json::from_value::<SumeragiV2Status>(nested).is_err());

        let envelope =
            ConsensusMessageV2::new(ConsensusMessageV2Payload::PayloadChunk(PayloadChunk {
                manifest_hash: HashOf::from_untyped_unchecked(Hash::new(b"manifest")),
                index: 0,
                bytes: vec![1],
                sender: 0,
                signature: vec![2],
            }));
        let mut nested_envelope =
            norito::json::to_value(&envelope).expect("serialize nested envelope");
        nested_envelope
            .as_object_mut()
            .expect("envelope object")
            .get_mut("payload")
            .and_then(norito::json::Value::as_object_mut)
            .expect("payload variant object")
            .get_mut("message")
            .and_then(norito::json::Value::as_object_mut)
            .expect("payload message object")
            .insert("unknown".to_owned(), norito::json::Value::Bool(true));
        assert!(
            norito::json::from_value::<ConsensusMessageV2>(nested_envelope).is_err(),
            "nested consensus payload must reject unknown fields"
        );

        let mut envelope_json = norito::json::to_value(&envelope).expect("serialize envelope");
        envelope_json
            .as_object_mut()
            .expect("envelope object")
            .insert("unknown".to_owned(), norito::json::Value::Bool(true));
        assert!(norito::json::from_value::<ConsensusMessageV2>(envelope_json).is_err());
    }
}
