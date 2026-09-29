//! Canonical Norito wire types for the Sumeragi v2 consensus protocol.
//!
//! Sumeragi v2 deliberately keeps its global Prepare/Commit protocol separate
//! from the lane-local [`super::consensus::CertPhase`] protocol.  The types in
//! this module are therefore versioned independently and do not replace or
//! reinterpret the first-release wire types in [`super::consensus`].
use super::Header as BlockHeader;

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    NetworkId, account::AccountId, nexus::PublicLaneValidatorRecord,
    transaction::signed::TransactionEntrypoint,
};
use core::fmt;
#[cfg(test)]
use iroha_crypto::{Algorithm, KeyPair};
use iroha_crypto::{Hash, HashOf, MerkleTreeCommitment};
use iroha_model_base::peer::PeerId;
use iroha_model_base::topology::LaneId;
use iroha_schema::{EnumMeta, EnumVariant, Ident, IntoSchema, MetaMap, Metadata, TypeId};
use norito::codec::{Decode, Encode};
use std::{collections::BTreeSet, vec::Vec};
/// Durable finality artifacts associated with canonical Sumeragi v2 blocks.
pub mod finality;
/// Canonical genesis/handshake fingerprint projection.
pub mod fingerprint;
mod messages;
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
/// Allocation bound for one consensus signature or aggregate.
///
/// Ordinary BLS signatures remain compact. A Commit vote for a block containing
/// KAGEMUSHA V1 top-ups additionally carries one paired Pasta finality-seal
/// share, while its `CommitQC` carries the exact `2f + 1` share bundle. The
/// bound covers the largest admitted 31-validator committee without making the
/// auxiliary proof payload unbounded.
pub const MAX_CONSENSUS_SIGNATURE_BYTES: usize = 16 * 1024;
/// Reserved envelope kind for one KAGEMUSHA V1 Commit-vote seal share.
pub const KAGEMUSHA_COMMIT_VOTE_SIGNATURE_ENVELOPE_KIND_V1: u8 = 1;
/// Reserved envelope kind for an KAGEMUSHA V1 `CommitQC` seal bundle.
pub const KAGEMUSHA_COMMIT_QC_SIGNATURE_ENVELOPE_KIND_V1: u8 = 2;
const KAGEMUSHA_CONSENSUS_SIGNATURE_ENVELOPE_MAGIC_V1: [u8; 16] = *b"iroha-kgm-sig-v1";
const KAGEMUSHA_CONSENSUS_SIGNATURE_ENVELOPE_HEADER_BYTES_V1: usize = 16 + 1 + 2 + 4;
const KAGEMUSHA_CONSENSUS_BLS_SIGNATURE_MAX_BYTES_V1: usize = 256;
const HEIGHT_CONTEXT_IDENTITY_VERSION: u16 = 6;
/// Permissioned Sumeragi v2 handshake and domain-separation tag.
pub const PERMISSIONED_TAG: &str = "iroha2-consensus::permissioned-sumeragi@v2";
/// `NPoS` Sumeragi v2 handshake and domain-separation tag.
pub const NPOS_TAG: &str = "iroha2-consensus::npos-sumeragi@v2";
/// BLS domain selected by a permissioned v2 genesis.
pub const PERMISSIONED_BLS_DOMAIN: &str = "bls-iroha2:permissioned-sumeragi:v2";
/// BLS domain selected by an `NPoS` v2 genesis.
pub const NPOS_BLS_DOMAIN: &str = "bls-iroha2:npos-sumeragi:v2";
/// Consensus-wide upper bound for the canonical result-bearing block wire.
///
/// This is the protocol authority shared by execution-commitment admission and
/// durable canonical-block storage. It deliberately matches the first-release
/// Kura hard limit; runtime configuration may select a lower bound but must
/// never admit a larger consensus value.
pub const MAX_EXECUTED_BLOCK_WIRE_BYTES: u64 = 256 * 1024 * 1024;
const KAGEMUSHA_TOP_UP_POST_STATE_ROOT_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:post-state-root";
/// Canonical Nexus/AMX context commitment for the repository's recommended
/// single-lane defaults and no staged public-lane validators.
///
/// `iroha_config` owns the projection and pins this value with a golden test.
/// Keeping the bytes here lets configuration-independent genesis builders emit
/// a valid signed template without introducing a data-model/config cycle.
pub const RECOMMENDED_NEXUS_AMX_CONTEXT_HASH: [u8; 32] = [
    91, 38, 248, 103, 86, 84, 235, 0, 186, 36, 255, 38, 66, 136, 73, 143, 217, 61, 247, 57, 245,
    196, 18, 254, 31, 190, 33, 199, 229, 145, 52, 179,
];
/// Canonical V1 boot execution-policy identity emitted by the recommended genesis template.
///
/// Genesis materialization replaces this template value with the identity derived from the
/// complete staged runtime policy before signing. Startup never treats it as a fallback.
pub const RECOMMENDED_EXECUTION_POLICY_HASH: [u8; 32] = [
    63, 148, 116, 83, 117, 143, 142, 233, 11, 44, 102, 67, 122, 18, 143, 194, 45, 147, 196, 210,
    224, 202, 96, 194, 97, 216, 40, 183, 224, 184, 151, 195,
];
/// Recommended deterministic data-availability layout.
#[must_use]
pub const fn recommended_data_availability_layout() -> DataAvailabilityLayout {
    DataAvailabilityLayout {
        encoding: PayloadEncoding::ReedSolomon16,
        chunk_size_bytes: MAX_DA_CHUNK_SIZE_BYTES,
        data_shards: 4,
        parity_shards: 2,
        max_payload_size_bytes: MAX_DA_PAYLOAD_SIZE_BYTES,
        max_chunk_count: MAX_DA_CHUNK_COUNT,
    }
}
/// Block height in the v2 protocol.
pub type Height = u64;
/// View number within one block height.
pub type View = u64;
/// Index into the ordered voting roster frozen in a [`HeightContext`].
pub type ValidatorIndex = u32;
// TODO(WP9): `ConsensusMode` moved to `crate::parameter::system`; this re-export and the v2-only
// helpers below (`tag`, `bls_domain`) exist only while the Sumeragi v2 runtime still compiles and
// disappear with the `consensus_v2` family.
pub use crate::parameter::system::ConsensusMode;
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
}
/// A validator and its consensus vote at one height.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::ValidatorPower")]
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
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::DualQuorum")]
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
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::DataAvailabilityLayout")]
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
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(
    tag = "encoding",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::PayloadEncoding")]
pub enum PayloadEncoding {
    /// Encode payload stripes with the deterministic RS16 layout.
    ReedSolomon16,
}
/// Genesis-selected transport inputs needed to construct every Sumeragi v2
/// height context.
///
/// The value is embedded in the signed consensus-genesis parameters. Live v2
/// startup must reject a genesis which omits it; it must never reconstruct
/// these fields from a node's mutable runtime configuration. The separately
/// signed, network-independent KAGEMUSHA authority templates live beside
/// this value in [`crate::parameter::system::ConsensusHandshakeMetadata`]; they
/// are deliberately absent from this snapshot-reconstructible context and its
/// secondary consensus fingerprint.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters")]
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
    pub const fn recommended() -> Self {
        Self {
            da_layout: recommended_data_availability_layout(),
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
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::SnapshotBootstrapAnchor")]
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
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::SnapshotV2BootstrapRecord")]
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
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::HeightContext")]
pub struct HeightContext {
    /// Exact genesis-derived network identity used for replay protection.
    pub network_id: NetworkId,
    /// Wire protocol version; must equal [`PROTOCOL_VERSION`].
    pub protocol_version: u16,
    /// Height governed by this context.
    pub height: Height,
    /// Finalized validator-election epoch.
    pub epoch: u64,
    /// Complete scheduling authorization certified by the preceding boundary or signed genesis.
    pub kagemusha_mint_finality_authorization:
        crate::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1,
    /// Complete immutable paired-Pasta generation selected by the scheduling authorization.
    /// Retaining these exact keys across epochs requires an explicit certified retention.
    pub kagemusha_mint_finality_authority:
        crate::isi::kagemusha_v1::KagemushaMintFinalityAuthorityGenerationV1,
    /// Last height governed by this epoch's frozen election snapshot.
    pub epoch_end_height: Height,
    /// Complete transition selected from the committed pre-state when this is
    /// the last height of an epoch and a successor height is representable. The
    /// `CommitQC` authenticates these bytes through [`Self::id`]; non-boundary
    /// contexts and the terminal `u64::MAX` height, which has no representable
    /// successor, must carry `None`.
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
            kagemusha_mint_finality_authorization: self.kagemusha_mint_finality_authorization,
            kagemusha_mint_finality_authority: self.kagemusha_mint_finality_authority.clone(),
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
        self.quorum.validate_roster(&self.roster)?;
        let authority = &self.kagemusha_mint_finality_authority;
        let authorization = &self.kagemusha_mint_finality_authorization;
        if authorization.validate_against_authority(authority).is_err()
            || authorization.network_id != self.network_id
            || authorization.epoch != self.epoch
            || authorization.first_height > self.height
        {
            return Err(ValidationError::InvalidKagemushaMintFinalityAuthorization);
        }
        // Current height lies inside the interval ending at this epoch boundary.
        if authorization.last_height != self.epoch_end_height {
            return Err(ValidationError::InvalidKagemushaMintFinalityAuthorization);
        }
        if authority.validators.len() != self.roster.len()
            || authority
                .validators
                .iter()
                .zip(&self.roster)
                .any(|(mint, consensus)| mint.validator != consensus.validator)
        {
            return Err(ValidationError::InvalidKagemushaMintFinalityAuthorityGeneration);
        }
        if self.nexus_amx_context_hash == Hash::prehashed([0; Hash::LENGTH]) {
            return Err(ValidationError::InvalidNexusAmxContextHash);
        }
        if self.execution_policy_hash == Hash::prehashed([0; Hash::LENGTH]) {
            return Err(ValidationError::InvalidExecutionPolicyHash);
        }
        let is_terminal_context = self.height == u64::MAX
            && self.epoch_end_height == u64::MAX
            && self.next_epoch_snapshot.is_none();
        match (
            self.height == self.epoch_end_height,
            self.next_epoch_snapshot.as_ref(),
        ) {
            (true, Some(snapshot)) => snapshot.validate_against(self)?,
            (true, None) if is_terminal_context => {}
            (true, None) => return Err(ValidationError::MissingNextEpochSnapshot),
            (false, Some(_)) => return Err(ValidationError::UnexpectedNextEpochSnapshot),
            (false, None) => {}
        }
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
#[derive(Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::HeightContextIdentity")]
struct HeightContextIdentity {
    identity_version: u16,
    network_id: NetworkId,
    protocol_version: u16,
    height: Height,
    epoch: u64,
    kagemusha_mint_finality_authorization:
        crate::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1,
    kagemusha_mint_finality_authority:
        crate::isi::kagemusha_v1::KagemushaMintFinalityAuthorityGenerationV1,
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
#[derive(Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::ParentCommitIdentity")]
struct ParentCommitIdentity {
    context_id: HeightContextId,
    height: Height,
    phase: GlobalPhase,
    subject: BlockSubject,
    execution_commitment: ExecutionCommitment,
}
/// Typed identifier of a complete [`HeightContext`].
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[repr(transparent)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::HeightContextId")]
pub struct HeightContextId(
    /// Norito hash of the context's semantic identity projection.
    pub HashOf<HeightContext>,
);
/// Consensus round identity under a frozen height context.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::ConsensusRound")]
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
/// by timeout certificates.  It is also distinct from lane-local phases.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(
    tag = "phase",
    content = "details",
    rename_all = "snake_case",
    deny_unknown_fields
)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::GlobalPhase")]
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
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::BlockSubject")]
pub struct BlockSubject {
    /// Parent block hash, absent only for the genesis block.
    #[norito(required)]
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
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::ExecutionCommitment")]
pub struct ExecutionCommitment {
    /// Root of the witnessed pre-state values for keys changed by the block.
    pub parent_state_root: Hash,
    /// Root of the complete deterministic post-state projection.
    pub post_state_root: Hash,
    /// Root of all canonical last-write-wins writes other than KAGEMUSHA V1 top-ups.
    pub ordinary_writes_root: Hash,
    /// Root of the canonical balanced KAGEMUSHA V1 top-up tree, when present.
    #[norito(required)]
    pub kagemusha_top_up_root: Option<Hash>,
    /// Number of real KAGEMUSHA V1 top-up leaves committed by `kagemusha_top_up_root`.
    pub kagemusha_top_up_count: u32,
    /// Exact non-zero byte length of the canonical result-bearing block wire.
    pub executed_block_wire_len: u64,
    /// Hash of the canonical result-bearing block wire produced by deterministic execution.
    pub executed_block_wire_hash: Hash,
    /// Exact Network input tree derived from the validated executed block.
    /// Root AND leaf count are CommitQC-authenticated; omission is not a decoder default.
    #[norito(required)]
    pub transaction_input_commitment: Option<MerkleTreeCommitment<TransactionEntrypoint>>,
    /// Exact complete typed-output tree, including internal invocation outputs.
    #[norito(required)]
    pub transaction_output_commitment:
        Option<MerkleTreeCommitment<crate::block::execution_output::ExecutionOutputV1>>,
}
impl ExecutionCommitment {
    /// Bind the mandatory selective commitments from the same full native wire
    /// whose identity the execution witness already commits. Only validator-side
    /// execution may call this before constructing/signing a Commit vote.
    ///
    /// # Errors
    ///
    /// Returns an error if the canonical wire cannot be encoded, the block results or
    /// output cache are invalid, the wire identity differs, or commitment validation fails.
    pub fn with_transaction_commitments_from_block(
        mut self,
        block: &crate::block::SignedBlock,
    ) -> Result<Self, ValidationError> {
        let wire = block
            .encode_wire()
            .map_err(|_| ValidationError::InvalidExecutionCommitment)?;
        if !block.has_results()
            || block.validate_output_merkle_cache().is_err()
            || u64::try_from(wire.len()).ok() != Some(self.executed_block_wire_len)
            || Hash::new(&wire) != self.executed_block_wire_hash
        {
            return Err(ValidationError::InvalidExecutionCommitment);
        }
        self.transaction_input_commitment = block.network_input_merkle_commitment();
        self.transaction_output_commitment = block.output_merkle_commitment();
        self.validate()?;
        Ok(self)
    }

    /// Construct a transition that contains no KAGEMUSHA V1 top-ups.
    #[must_use]
    pub fn without_kagemusha_top_ups_or_merge_carrier(
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
            kagemusha_top_up_root: None,
            kagemusha_top_up_count: 0,
            executed_block_wire_len,
            executed_block_wire_hash,
            transaction_input_commitment: None,
            transaction_output_commitment: None,
        }
    }
    /// Construct a commitment and enforce its canonical top-up projection.
    ///
    /// # Errors
    ///
    /// Returns an error when root presence disagrees with the count or the combined post-state
    /// root is not the canonical hash of the advertised top-up projection.
    pub fn new_without_merge_carrier(
        parent_state_root: Hash,
        post_state_root: Hash,
        ordinary_writes_root: Hash,
        kagemusha_top_up_root: Option<Hash>,
        kagemusha_top_up_count: u32,
        executed_block_wire_len: u64,
        executed_block_wire_hash: Hash,
    ) -> Result<Self, ValidationError> {
        let commitment = Self {
            parent_state_root,
            post_state_root,
            ordinary_writes_root,
            kagemusha_top_up_root,
            kagemusha_top_up_count,
            executed_block_wire_len,
            executed_block_wire_hash,
            transaction_input_commitment: None,
            transaction_output_commitment: None,
        };
        commitment.validate()?;
        Ok(commitment)
    }
    /// Validate the canonical count/root relationship and combined top-up root.
    ///
    /// # Errors
    ///
    /// Returns an execution-commitment error when a root/count pair is
    /// inconsistent, a protocol bound is exceeded, or a combined state root is
    /// incorrect.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.transaction_input_commitment.is_some_and(|inputs| {
            self.transaction_output_commitment
                .is_none_or(|outputs| outputs.leaf_count() < inputs.leaf_count())
        }) {
            return Err(ValidationError::InvalidExecutionCommitment);
        }
        if self.executed_block_wire_len == 0
            || self.executed_block_wire_len > MAX_EXECUTED_BLOCK_WIRE_BYTES
        {
            return Err(ValidationError::InvalidExecutedBlockWireLength);
        }
        match (self.kagemusha_top_up_count, self.kagemusha_top_up_root) {
            (0, None) => {}
            (0, Some(_)) | (_, None) => {
                return Err(ValidationError::InvalidExecutionCommitment);
            }
            (count, Some(root)) => {
                if self.post_state_root
                    != Self::kagemusha_post_state_root_v1(count, self.ordinary_writes_root, root)
                {
                    return Err(ValidationError::ExecutionCommitmentPostRootMismatch);
                }
            }
        }
        Ok(())
    }
    /// Derive the canonical combined post-state root for a non-empty top-up tree.
    #[must_use]
    pub fn kagemusha_post_state_root_v1(
        kagemusha_top_up_count: u32,
        ordinary_writes_root: Hash,
        kagemusha_top_up_root: Hash,
    ) -> Hash {
        let mut preimage = Vec::with_capacity(
            KAGEMUSHA_TOP_UP_POST_STATE_ROOT_DOMAIN_V1.len()
                + 1
                + core::mem::size_of::<u32>()
                + 2 * Hash::LENGTH,
        );
        preimage.extend_from_slice(KAGEMUSHA_TOP_UP_POST_STATE_ROOT_DOMAIN_V1);
        preimage.push(0);
        preimage.extend_from_slice(&kagemusha_top_up_count.to_le_bytes());
        preimage.extend_from_slice(ordinary_writes_root.as_ref());
        preimage.extend_from_slice(kagemusha_top_up_root.as_ref());
        Hash::new(preimage)
    }
}

/// Borrowed components of one KAGEMUSHA V1 consensus-signature envelope.
///
/// The framing deliberately keeps the ordinary BLS signature first-class so
/// generic finality verification can authenticate the same vote preimage while
/// the KAGEMUSHA verifier separately checks the paired Pasta payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaConsensusSignatureEnvelopePartsV1<'a> {
    /// Whether the auxiliary payload is a Commit-vote share or `CommitQC` bundle.
    pub kind: u8,
    /// Ordinary BLS signature or aggregate signature.
    pub bls_signature: &'a [u8],
    /// Canonical Norito bytes of the paired Pasta share or bundle.
    pub auxiliary_payload: &'a [u8],
}

/// Encode one bounded KAGEMUSHA V1 consensus-signature envelope.
///
/// # Errors
///
/// Returns an error when the kind, BLS signature, auxiliary payload, or total
/// length is outside the sole V1 framing contract.
pub fn encode_kagemusha_consensus_signature_envelope_v1(
    kind: u8,
    bls_signature: &[u8],
    auxiliary_payload: &[u8],
) -> Result<Vec<u8>, ValidationError> {
    if !matches!(
        kind,
        KAGEMUSHA_COMMIT_VOTE_SIGNATURE_ENVELOPE_KIND_V1
            | KAGEMUSHA_COMMIT_QC_SIGNATURE_ENVELOPE_KIND_V1
    ) || bls_signature.is_empty()
        || bls_signature.len() > KAGEMUSHA_CONSENSUS_BLS_SIGNATURE_MAX_BYTES_V1
        || auxiliary_payload.is_empty()
    {
        return Err(ValidationError::InvalidKagemushaSignatureEnvelope);
    }
    let bls_len = u16::try_from(bls_signature.len())
        .map_err(|_| ValidationError::InvalidKagemushaSignatureEnvelope)?;
    let auxiliary_len = u32::try_from(auxiliary_payload.len())
        .map_err(|_| ValidationError::InvalidKagemushaSignatureEnvelope)?;
    let total = KAGEMUSHA_CONSENSUS_SIGNATURE_ENVELOPE_HEADER_BYTES_V1
        .checked_add(bls_signature.len())
        .and_then(|value| value.checked_add(auxiliary_payload.len()))
        .ok_or(ValidationError::SignatureTooLarge)?;
    if total > MAX_CONSENSUS_SIGNATURE_BYTES {
        return Err(ValidationError::SignatureTooLarge);
    }
    let mut envelope = Vec::with_capacity(total);
    envelope.extend_from_slice(&KAGEMUSHA_CONSENSUS_SIGNATURE_ENVELOPE_MAGIC_V1);
    envelope.push(kind);
    envelope.extend_from_slice(&bls_len.to_le_bytes());
    envelope.extend_from_slice(&auxiliary_len.to_le_bytes());
    envelope.extend_from_slice(bls_signature);
    envelope.extend_from_slice(auxiliary_payload);
    Ok(envelope)
}

/// Decode one reserved KAGEMUSHA V1 consensus-signature envelope.
///
/// A byte string without the complete 128-bit reserved prefix is an ordinary
/// BLS signature and returns `Ok(None)`. A prefixed but non-canonical frame
/// fails closed.
///
/// # Errors
///
/// Returns an error for an unknown kind, empty component, length mismatch, or
/// oversized frame.
pub fn decode_kagemusha_consensus_signature_envelope_v1(
    bytes: &[u8],
) -> Result<Option<KagemushaConsensusSignatureEnvelopePartsV1<'_>>, ValidationError> {
    if !bytes.starts_with(&KAGEMUSHA_CONSENSUS_SIGNATURE_ENVELOPE_MAGIC_V1) {
        return Ok(None);
    }
    if bytes.len() < KAGEMUSHA_CONSENSUS_SIGNATURE_ENVELOPE_HEADER_BYTES_V1
        || bytes.len() > MAX_CONSENSUS_SIGNATURE_BYTES
    {
        return Err(ValidationError::InvalidKagemushaSignatureEnvelope);
    }
    let kind = bytes[16];
    if !matches!(
        kind,
        KAGEMUSHA_COMMIT_VOTE_SIGNATURE_ENVELOPE_KIND_V1
            | KAGEMUSHA_COMMIT_QC_SIGNATURE_ENVELOPE_KIND_V1
    ) {
        return Err(ValidationError::InvalidKagemushaSignatureEnvelope);
    }
    let bls_len = usize::from(u16::from_le_bytes([bytes[17], bytes[18]]));
    let auxiliary_len = usize::try_from(u32::from_le_bytes([
        bytes[19], bytes[20], bytes[21], bytes[22],
    ]))
    .map_err(|_| ValidationError::InvalidKagemushaSignatureEnvelope)?;
    let bls_start = KAGEMUSHA_CONSENSUS_SIGNATURE_ENVELOPE_HEADER_BYTES_V1;
    let bls_end = bls_start
        .checked_add(bls_len)
        .ok_or(ValidationError::InvalidKagemushaSignatureEnvelope)?;
    let auxiliary_end = bls_end
        .checked_add(auxiliary_len)
        .ok_or(ValidationError::InvalidKagemushaSignatureEnvelope)?;
    if bls_len == 0
        || bls_len > KAGEMUSHA_CONSENSUS_BLS_SIGNATURE_MAX_BYTES_V1
        || auxiliary_len == 0
        || auxiliary_end != bytes.len()
    {
        return Err(ValidationError::InvalidKagemushaSignatureEnvelope);
    }
    Ok(Some(KagemushaConsensusSignatureEnvelopePartsV1 {
        kind,
        bls_signature: &bytes[bls_start..bls_end],
        auxiliary_payload: &bytes[bls_end..auxiliary_end],
    }))
}
/// One global Prepare or Commit vote.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::Vote")]
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
/// Canonical same-message fields authenticated by Prepare and Commit votes.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::VoteSignaturePayload")]
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
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::QuorumCertificateRef")]
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
/// Aggregate Prepare or Commit certificate.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus_v2::QuorumCertificate")]
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
    /// A representable epoch-ending context omitted its old-roster-authenticated transition.
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
    /// The mandatory KAGEMUSHA V1 mint-finality epoch-roster commitment is zero.
    InvalidKagemushaMintFinalityAuthorization,
    /// The embedded public Pasta roster does not exactly match its context commitment/election.
    InvalidKagemushaMintFinalityAuthorityGeneration,
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
    /// A top-up execution commitment's combined post root is not canonical.
    ExecutionCommitmentPostRootMismatch,
    /// An aggregate certificate carries no aggregate signature.
    MissingAggregateSignature,
    /// A signature or aggregate exceeds the protocol allocation bound.
    SignatureTooLarge,
    /// A paired-Pasta KAGEMUSHA V1 signature envelope is missing,
    /// unexpected, malformed, or uses the wrong vote/certificate kind.
    InvalidKagemushaSignatureEnvelope,
    /// Too few distinct validators signed.
    InsufficientSignerCount,
    /// The redundant signed-vote projection is not a strict supermajority.
    InsufficientVotingPower,
    /// An RS16 layout's encoded chunk count exceeds the wire index range.
    ChunkCountTooLarge,
    /// A vote or certificate is split across distinct proposal and vote rounds.
    InvalidProposalRound,
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
            Self::InvalidKagemushaMintFinalityAuthorization => {
                f.write_str("height context has an invalid KAGEMUSHA V1 mint-finality epoch id")
            }
            Self::InvalidKagemushaMintFinalityAuthorityGeneration => f.write_str(
                "height context KAGEMUSHA V1 mint-finality roster does not match its commitment",
            ),
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
            Self::ExecutionCommitmentPostRootMismatch => {
                f.write_str("execution commitment post-state root is not canonical")
            }
            Self::MissingAggregateSignature => {
                f.write_str("certificate has an empty aggregate signature")
            }
            Self::SignatureTooLarge => f.write_str("consensus signature exceeds protocol bound"),
            Self::InvalidKagemushaSignatureEnvelope => {
                f.write_str("KAGEMUSHA V1 consensus signature envelope is invalid")
            }
            Self::InsufficientSignerCount => {
                f.write_str("insufficient distinct validator signatures")
            }
            Self::InsufficientVotingPower => {
                f.write_str("inconsistent redundant signed-vote projection")
            }
            Self::ChunkCountTooLarge => f.write_str("payload chunk count exceeds the wire range"),
            Self::InvalidProposalRound => {
                f.write_str("proposal and certified rounds must be identical")
            }
        }
    }
}
impl std::error::Error for ValidationError {}
fn encoded_chunk_count_for_validated_layout(
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
        encoded_chunk_count_for_validated_layout(layout.max_payload_size_bytes, layout)
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

/// Build deterministic paired-Pasta authority aligned to a unit-test consensus roster.
#[cfg(test)]
pub(crate) fn test_kagemusha_mint_finality_authority(
    network_id: NetworkId,
    generation: u64,
    roster: &[ValidatorPower],
) -> crate::isi::kagemusha_v1::KagemushaMintFinalityAuthorityGenerationV1 {
    use crate::isi::kagemusha_v1::{
        KAGEMUSHA_CHAIN_VERSION_V1, KagemushaMintFinalityAuthorityGenerationV1,
        KagemushaMintFinalityValidatorKeysV1,
    };

    KagemushaMintFinalityAuthorityGenerationV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        network_id,
        generation,
        validators: roster
            .iter()
            .enumerate()
            .map(|(index, validator)| KagemushaMintFinalityValidatorKeysV1 {
                validator: validator.validator.clone(),
                eq_proof_public_key: [u8::try_from(index + 1).expect("small fixture roster"); 32],
                ep_proof_public_key: [u8::try_from(index + 17).expect("small fixture roster"); 32],
            })
            .collect(),
    }
}

/// Build deterministic signed-genesis context parameters for unit tests.
#[cfg(test)]
pub(crate) fn test_genesis_context_parameters() -> SumeragiV2GenesisContextParameters {
    SumeragiV2GenesisContextParameters::recommended()
}

/// Build deterministic network-independent KAGEMUSHA genesis authority for unit tests.
#[cfg(test)]
pub(crate) fn test_kagemusha_mint_finality_genesis_parameters()
-> crate::isi::kagemusha_v1::KagemushaMintFinalityGenesisParametersV1 {
    use crate::isi::kagemusha_v1::{
        KagemushaMintFinalityAuthorityGenerationTemplateV1,
        KagemushaMintFinalityGenesisParametersV1,
    };

    let network_id = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::new(b"Sumeragi v2 unit-test genesis"),
    ));
    let mut roster = (1_u8..=4)
        .map(|seed| {
            let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("derive deterministic test validator");
            ValidatorPower {
                validator: PeerId::new(key_pair.public_key().clone()),
                power: 1,
            }
        })
        .collect::<Vec<_>>();
    roster.sort_by(|left, right| left.validator.cmp(&right.validator));
    let bound = test_kagemusha_mint_finality_authority(network_id, 0, &roster);
    KagemushaMintFinalityGenesisParametersV1 {
        authority_generation: KagemushaMintFinalityAuthorityGenerationTemplateV1 {
            version: bound.version,
            generation: bound.generation,
            validators: bound.validators,
        },
    }
}
#[cfg(test)]
#[path = "consensus_v2_tests.rs"]
mod tests;

#[cfg(test)]
mod terminal_height_context_tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};

    fn terminal_context() -> HeightContext {
        let mut roster = (1_u8..=4)
            .map(|seed| {
                let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("derive deterministic terminal-height fixture keypair");
                ValidatorPower {
                    validator: PeerId::new(key_pair.public_key().clone()),
                    power: 1,
                }
            })
            .collect::<Vec<_>>();
        roster.sort_by(|left, right| left.validator.cmp(&right.validator));
        let parent_round = ConsensusRound {
            context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                b"terminal-height parent context",
            ))),
            height: u64::MAX - 1,
            view: 0,
        };
        let parent_commit_qc = QuorumCertificate {
            round: parent_round,
            proposal_round: parent_round,
            phase: GlobalPhase::Commit,
            subject: BlockSubject {
                parent_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
                    b"terminal-height grandparent block",
                ))),
                block_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"terminal-height parent block",
                )),
                payload_hash: Hash::new(b"terminal-height parent payload"),
            },
            execution_commitment: ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
                Hash::new(b"terminal-height parent state"),
                Hash::new(b"terminal-height post state"),
                Hash::new(b"terminal-height ordinary writes"),
                1,
                Hash::new(b"terminal-height executed block wire"),
            ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xA5; 48],
        };
        let network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"terminal-height genesis")),
        );
        let mint_finality_roster = test_kagemusha_mint_finality_authority(network_id, 0, &roster);
        let mint_finality_authorization =
            crate::isi::kagemusha_v1::KagemushaMintFinalityEpochAuthorizationV1::genesis(
                &mint_finality_roster,
                u64::MAX,
            )
            .expect("valid terminal mint-finality authorization");
        HeightContext {
            network_id,
            protocol_version: PROTOCOL_VERSION,
            height: u64::MAX,
            epoch: 0,
            kagemusha_mint_finality_authorization: mint_finality_authorization,
            kagemusha_mint_finality_authority: mint_finality_roster,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: Some(parent_commit_qc),
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("valid terminal-height quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"terminal-height nexus AMX context"),
            execution_policy_hash: Hash::new(b"terminal-height execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [0x7A; 32],
        }
    }

    #[test]
    fn only_terminal_epoch_boundary_may_omit_the_successor_snapshot() {
        let terminal = terminal_context();
        assert_eq!(terminal.validate(), Ok(()));

        let mut nonterminal_boundary = terminal;
        nonterminal_boundary.height = u64::MAX - 1;
        nonterminal_boundary.epoch_end_height = u64::MAX - 1;
        nonterminal_boundary
            .kagemusha_mint_finality_authorization
            .last_height = u64::MAX - 1;
        let parent = nonterminal_boundary
            .parent_commit_qc
            .as_mut()
            .expect("terminal fixture has a parent CommitQC");
        parent.round.height = u64::MAX - 2;
        parent.proposal_round = parent.round;
        assert_eq!(
            nonterminal_boundary.validate(),
            Err(ValidationError::MissingNextEpochSnapshot)
        );
    }
}

#[cfg(test)]
mod captured_consensus_v2_schema_tests;
