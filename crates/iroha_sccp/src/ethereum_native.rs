//! Protocol-native Ethereum consensus light-client verification for SCCP.
//!
//! The module models the fixed SSZ types used by Ethereum light-client updates,
//! validates a governed fork schedule and genesis validators root, and advances
//! a light-client state without consulting a wall clock. Accepted updates must
//! carry finality, the next sync committee, and at least the Ethereum mainnet
//! two-thirds sync-committee threshold (342 of 512 positions).
//!
//! This file is intentionally self-contained so it can be reviewed and tested
//! before it is wired into the existing SCCP proof envelope.

use core::fmt;

use iroha_crypto::{ethereum_bls_pop_fast_aggregate_verify, ethereum_bls_pop_validate_public_key};
use sha2::{Digest as _, Sha256};

/// A 32-byte Ethereum SSZ root.
pub type Root = [u8; 32];

/// Ethereum mainnet sync-committee size.
pub const SYNC_COMMITTEE_SIZE: usize = 512;
/// Number of bytes in a `Bitvector[512]`.
pub const SYNC_COMMITTEE_BITS_BYTES: usize = SYNC_COMMITTEE_SIZE / 8;
/// Minimum participant count satisfying `participants * 3 >= 512 * 2`.
pub const FINALITY_PARTICIPANT_THRESHOLD: usize = 342;
/// Slots in an Ethereum epoch.
pub const SLOTS_PER_EPOCH: u64 = 32;
/// Epochs in an Ethereum sync-committee period.
pub const EPOCHS_PER_SYNC_COMMITTEE_PERIOD: u64 = 256;
/// Slots in an Ethereum sync-committee period.
pub const SLOTS_PER_SYNC_COMMITTEE_PERIOD: u64 = SLOTS_PER_EPOCH * EPOCHS_PER_SYNC_COMMITTEE_PERIOD;

/// `DOMAIN_SYNC_COMMITTEE` from the Ethereum consensus specification.
pub const DOMAIN_SYNC_COMMITTEE: [u8; 4] = [0x07, 0x00, 0x00, 0x00];
/// `finalized_checkpoint.root` generalized index before Electra.
pub const FINALIZED_ROOT_GINDEX_PRE_ELECTRA: u64 = 105;
/// `current_sync_committee` generalized index before Electra.
pub const CURRENT_SYNC_COMMITTEE_GINDEX_PRE_ELECTRA: u64 = 54;
/// `next_sync_committee` generalized index before Electra.
pub const NEXT_SYNC_COMMITTEE_GINDEX_PRE_ELECTRA: u64 = 55;
/// `finalized_checkpoint.root` generalized index from Electra onward.
pub const FINALIZED_ROOT_GINDEX_ELECTRA: u64 = 169;
/// `current_sync_committee` generalized index from Electra onward.
pub const CURRENT_SYNC_COMMITTEE_GINDEX_ELECTRA: u64 = 86;
/// `next_sync_committee` generalized index from Electra onward.
pub const NEXT_SYNC_COMMITTEE_GINDEX_ELECTRA: u64 = 87;
/// `execution_payload` generalized index in `BeaconBlockBody`.
pub const EXECUTION_PAYLOAD_GINDEX: u64 = 25;

const ZERO_ROOT: Root = [0; 32];

/// Errors returned while validating governed Ethereum light-client data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EthereumLightClientError {
    /// The governed fork schedule is malformed.
    InvalidForkSchedule(&'static str),
    /// The governed genesis validators root is the zero sentinel.
    ZeroGenesisValidatorsRoot,
    /// The slot precedes the first supported fork.
    UnsupportedSlot(u64),
    /// A header's closed fork variant disagrees with the governed schedule.
    HeaderForkMismatch {
        /// Fork selected by the governed schedule.
        expected: EthereumFork,
        /// Fork variant supplied by the update.
        actual: EthereumFork,
    },
    /// Execution payload extra data exceeded its SSZ `ByteList[32]` bound.
    ExtraDataTooLong(usize),
    /// A Capella-or-later execution payload branch was invalid.
    InvalidExecutionBranch,
    /// The trusted block root did not match the bootstrap beacon header.
    InvalidTrustedBlockRoot,
    /// The bootstrap current-committee branch used the wrong fork shape.
    CurrentCommitteeBranchForkMismatch,
    /// The bootstrap current-committee proof was invalid.
    InvalidCurrentCommitteeBranch,
    /// The finality branch used the wrong fork shape.
    FinalityBranchForkMismatch,
    /// The finality proof was invalid.
    InvalidFinalityBranch,
    /// The next-committee branch used the wrong fork shape.
    NextCommitteeBranchForkMismatch,
    /// The next sync-committee proof was invalid.
    InvalidNextCommitteeBranch,
    /// A sync-committee public key failed BLS `KeyValidate`.
    InvalidCommitteePublicKey(usize),
    /// A sync committee's aggregate public key failed BLS `KeyValidate`.
    InvalidCommitteeAggregatePublicKey,
    /// Update slots did not satisfy `signature > attested >= finalized`.
    InvalidSlotOrder,
    /// The update did not meet the 342-of-512 finality threshold.
    InsufficientParticipation(usize),
    /// The update skipped a sync-committee period.
    SkippedSyncCommitteePeriod,
    /// The finalized header did not advance the immutable anchor.
    StaleFinalizedHeader,
    /// A period transition required a next committee that was not anchored.
    MissingNextSyncCommittee,
    /// An update for the current period changed an already anchored next committee.
    ConflictingNextSyncCommittee,
    /// The standard Ethereum BLS aggregate signature was invalid.
    InvalidSyncCommitteeSignature,
    /// A previously validated update was applied to a different state snapshot.
    UpdateForDifferentState,
}

impl fmt::Display for EthereumLightClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidForkSchedule(reason) => {
                write!(formatter, "invalid fork schedule: {reason}")
            }
            Self::ZeroGenesisValidatorsRoot => {
                formatter.write_str("genesis validators root must not be zero")
            }
            Self::UnsupportedSlot(slot) => write!(formatter, "unsupported pre-Altair slot {slot}"),
            Self::HeaderForkMismatch { expected, actual } => write!(
                formatter,
                "header fork mismatch: schedule requires {expected:?}, got {actual:?}"
            ),
            Self::ExtraDataTooLong(len) => {
                write!(
                    formatter,
                    "execution extra_data length {len} exceeds 32 bytes"
                )
            }
            Self::InvalidExecutionBranch => formatter.write_str("invalid execution payload branch"),
            Self::InvalidTrustedBlockRoot => formatter.write_str("invalid trusted block root"),
            Self::CurrentCommitteeBranchForkMismatch => {
                formatter.write_str("current committee branch does not match the active fork")
            }
            Self::InvalidCurrentCommitteeBranch => {
                formatter.write_str("invalid current sync committee branch")
            }
            Self::FinalityBranchForkMismatch => {
                formatter.write_str("finality branch does not match the active fork")
            }
            Self::InvalidFinalityBranch => formatter.write_str("invalid finality branch"),
            Self::NextCommitteeBranchForkMismatch => {
                formatter.write_str("next committee branch does not match the active fork")
            }
            Self::InvalidNextCommitteeBranch => {
                formatter.write_str("invalid next sync committee branch")
            }
            Self::InvalidCommitteePublicKey(position) => {
                write!(
                    formatter,
                    "invalid sync committee public key at position {position}"
                )
            }
            Self::InvalidCommitteeAggregatePublicKey => {
                formatter.write_str("invalid sync committee aggregate public key")
            }
            Self::InvalidSlotOrder => formatter.write_str("invalid light-client update slot order"),
            Self::InsufficientParticipation(actual) => write!(
                formatter,
                "sync committee participation {actual} is below the required 342"
            ),
            Self::SkippedSyncCommitteePeriod => {
                formatter.write_str("light-client update skipped a sync committee period")
            }
            Self::StaleFinalizedHeader => {
                formatter.write_str("light-client update did not advance finality")
            }
            Self::MissingNextSyncCommittee => {
                formatter.write_str("next sync committee is not anchored")
            }
            Self::ConflictingNextSyncCommittee => formatter
                .write_str("light-client update conflicts with the anchored next committee"),
            Self::InvalidSyncCommitteeSignature => {
                formatter.write_str("invalid Ethereum sync committee signature")
            }
            Self::UpdateForDifferentState => {
                formatter.write_str("validated update belongs to a different state snapshot")
            }
        }
    }
}

impl std::error::Error for EthereumLightClientError {}

/// Closed set of Ethereum consensus forks understood by this verifier.
///
/// A future fork that changes any relevant SSZ type must be added explicitly;
/// unknown fork names cannot silently reuse an older layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EthereumFork {
    /// Altair.
    Altair,
    /// Bellatrix.
    Bellatrix,
    /// Capella.
    Capella,
    /// Deneb.
    Deneb,
    /// Electra.
    Electra,
    /// Fulu.
    Fulu,
}

impl EthereumFork {
    const ALL: [Self; 6] = [
        Self::Altair,
        Self::Bellatrix,
        Self::Capella,
        Self::Deneb,
        Self::Electra,
        Self::Fulu,
    ];

    const fn uses_electra_state_layout(self) -> bool {
        matches!(self, Self::Electra | Self::Fulu)
    }
}

/// Governed activation parameters for one fixed Ethereum fork.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForkActivation {
    epoch: u64,
    version: [u8; 4],
}

impl ForkActivation {
    /// Construct activation parameters.
    pub const fn new(epoch: u64, version: [u8; 4]) -> Self {
        Self { epoch, version }
    }

    /// Return the activation epoch.
    pub const fn epoch(self) -> u64 {
        self.epoch
    }

    /// Return the four-byte fork version.
    pub const fn version(self) -> [u8; 4] {
        self.version
    }
}

/// Validated governed Ethereum fork schedule and genesis validators root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForkSchedule {
    genesis_validators_root: Root,
    activations: [ForkActivation; 6],
}

impl ForkSchedule {
    /// Validate and construct a complete Altair-through-Fulu schedule.
    ///
    /// Activation epochs must be nondecreasing (multiple forks at genesis are
    /// valid on development networks), and all fork versions must be unique.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero genesis root, unordered activations, or
    /// duplicate fork versions.
    pub fn new(
        genesis_validators_root: Root,
        activations: [ForkActivation; 6],
    ) -> Result<Self, EthereumLightClientError> {
        if genesis_validators_root == ZERO_ROOT {
            return Err(EthereumLightClientError::ZeroGenesisValidatorsRoot);
        }
        if activations
            .windows(2)
            .any(|pair| pair[0].epoch > pair[1].epoch)
        {
            return Err(EthereumLightClientError::InvalidForkSchedule(
                "activation epochs must be nondecreasing",
            ));
        }
        for (index, activation) in activations.iter().enumerate() {
            if activations[..index]
                .iter()
                .any(|prior| prior.version == activation.version)
            {
                return Err(EthereumLightClientError::InvalidForkSchedule(
                    "fork versions must be unique",
                ));
            }
        }
        Ok(Self {
            genesis_validators_root,
            activations,
        })
    }

    /// Return the governed genesis validators root.
    pub const fn genesis_validators_root(&self) -> Root {
        self.genesis_validators_root
    }

    /// Return the activation parameters for a supported fork.
    pub const fn activation(&self, fork: EthereumFork) -> ForkActivation {
        self.activations[fork as usize]
    }

    /// Select the active fork for a slot.
    ///
    /// # Errors
    ///
    /// Returns an error when the slot precedes the governed Altair activation.
    pub fn fork_at_slot(
        &self,
        slot: u64,
    ) -> Result<(EthereumFork, ForkActivation), EthereumLightClientError> {
        let epoch = slot / SLOTS_PER_EPOCH;
        let mut selected = None;
        for (fork, activation) in EthereumFork::ALL.into_iter().zip(self.activations) {
            if activation.epoch <= epoch {
                selected = Some((fork, activation));
            }
        }
        selected.ok_or(EthereumLightClientError::UnsupportedSlot(slot))
    }

    fn commitment(&self) -> Root {
        let mut hasher = Sha256::new();
        hasher.update(b"sccp:ethereum-fork-schedule:v1");
        hasher.update(self.genesis_validators_root);
        for (fork, activation) in EthereumFork::ALL.into_iter().zip(self.activations) {
            hasher.update([fork as u8]);
            hasher.update(activation.epoch.to_le_bytes());
            hasher.update(activation.version);
        }
        hasher.finalize().into()
    }
}

/// Fork-dependent Ethereum light-client generalized indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LightClientGeneralizedIndices {
    /// `finalized_checkpoint.root` generalized index.
    pub finalized_root: u64,
    /// `current_sync_committee` generalized index.
    pub current_sync_committee: u64,
    /// `next_sync_committee` generalized index.
    pub next_sync_committee: u64,
}

/// Return the light-client generalized indices for a supported fork.
pub const fn generalized_indices(fork: EthereumFork) -> LightClientGeneralizedIndices {
    if fork.uses_electra_state_layout() {
        LightClientGeneralizedIndices {
            finalized_root: FINALIZED_ROOT_GINDEX_ELECTRA,
            current_sync_committee: CURRENT_SYNC_COMMITTEE_GINDEX_ELECTRA,
            next_sync_committee: NEXT_SYNC_COMMITTEE_GINDEX_ELECTRA,
        }
    } else {
        LightClientGeneralizedIndices {
            finalized_root: FINALIZED_ROOT_GINDEX_PRE_ELECTRA,
            current_sync_committee: CURRENT_SYNC_COMMITTEE_GINDEX_PRE_ELECTRA,
            next_sync_committee: NEXT_SYNC_COMMITTEE_GINDEX_PRE_ELECTRA,
        }
    }
}

/// Return the sync-committee period containing `slot`.
pub const fn sync_committee_period_at_slot(slot: u64) -> u64 {
    slot / SLOTS_PER_SYNC_COMMITTEE_PERIOD
}

/// Official SSZ `BeaconBlockHeader`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BeaconBlockHeader {
    /// Beacon slot.
    pub slot: u64,
    /// Proposer validator index.
    pub proposer_index: u64,
    /// Parent beacon block root.
    pub parent_root: Root,
    /// Beacon state root.
    pub state_root: Root,
    /// Beacon block body root.
    pub body_root: Root,
}

impl BeaconBlockHeader {
    /// Compute the canonical SSZ `hash_tree_root`.
    pub fn hash_tree_root(&self) -> Root {
        merkleize(&[
            uint64_root(self.slot),
            uint64_root(self.proposer_index),
            self.parent_root,
            self.state_root,
            self.body_root,
        ])
    }
}

/// A bounded SSZ `ByteList[32]` used for execution payload extra data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExtraData(Vec<u8>);

impl ExtraData {
    /// Validate and construct bounded extra data.
    ///
    /// # Errors
    ///
    /// Returns an error when `bytes` exceeds the SSZ `ByteList[32]` limit.
    pub fn new(bytes: Vec<u8>) -> Result<Self, EthereumLightClientError> {
        if bytes.len() > 32 {
            return Err(EthereumLightClientError::ExtraDataTooLong(bytes.len()));
        }
        Ok(Self(bytes))
    }

    /// Borrow the extra-data bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    fn hash_tree_root(&self) -> Root {
        let mut data = [0; 32];
        data[..self.0.len()].copy_from_slice(&self.0);
        hash_nodes(&data, &usize_root(self.0.len()))
    }
}

/// Official Capella SSZ `ExecutionPayloadHeader`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapellaExecutionPayloadHeader {
    /// Parent execution block hash.
    pub parent_hash: Root,
    /// Execution fee recipient.
    pub fee_recipient: [u8; 20],
    /// Execution state root.
    pub state_root: Root,
    /// Execution receipts root.
    pub receipts_root: Root,
    /// Execution logs bloom.
    pub logs_bloom: [u8; 256],
    /// Previous RANDAO mix.
    pub prev_randao: Root,
    /// Execution block number.
    pub block_number: u64,
    /// Execution gas limit.
    pub gas_limit: u64,
    /// Execution gas used.
    pub gas_used: u64,
    /// Execution timestamp.
    pub timestamp: u64,
    /// Bounded execution extra data.
    pub extra_data: ExtraData,
    /// Base fee encoded as SSZ little-endian `uint256`.
    pub base_fee_per_gas: [u8; 32],
    /// Execution block hash.
    pub block_hash: Root,
    /// Transactions list root.
    pub transactions_root: Root,
    /// Withdrawals list root.
    pub withdrawals_root: Root,
}

impl CapellaExecutionPayloadHeader {
    /// Compute the canonical Capella SSZ `hash_tree_root`.
    pub fn hash_tree_root(&self) -> Root {
        merkleize(&[
            self.parent_hash,
            byte_vector_root(&self.fee_recipient),
            self.state_root,
            self.receipts_root,
            byte_vector_root(&self.logs_bloom),
            self.prev_randao,
            uint64_root(self.block_number),
            uint64_root(self.gas_limit),
            uint64_root(self.gas_used),
            uint64_root(self.timestamp),
            self.extra_data.hash_tree_root(),
            self.base_fee_per_gas,
            self.block_hash,
            self.transactions_root,
            self.withdrawals_root,
        ])
    }
}

/// Official Deneb SSZ `ExecutionPayloadHeader`, also used by Electra and Fulu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DenebExecutionPayloadHeader {
    /// Parent execution block hash.
    pub parent_hash: Root,
    /// Execution fee recipient.
    pub fee_recipient: [u8; 20],
    /// Execution state root.
    pub state_root: Root,
    /// Execution receipts root.
    pub receipts_root: Root,
    /// Execution logs bloom.
    pub logs_bloom: [u8; 256],
    /// Previous RANDAO mix.
    pub prev_randao: Root,
    /// Execution block number.
    pub block_number: u64,
    /// Execution gas limit.
    pub gas_limit: u64,
    /// Execution gas used.
    pub gas_used: u64,
    /// Execution timestamp.
    pub timestamp: u64,
    /// Bounded execution extra data.
    pub extra_data: ExtraData,
    /// Base fee encoded as SSZ little-endian `uint256`.
    pub base_fee_per_gas: [u8; 32],
    /// Execution block hash.
    pub block_hash: Root,
    /// Transactions list root.
    pub transactions_root: Root,
    /// Withdrawals list root.
    pub withdrawals_root: Root,
    /// Blob gas used by the execution block.
    pub blob_gas_used: u64,
    /// Excess blob gas after the execution block.
    pub excess_blob_gas: u64,
}

impl DenebExecutionPayloadHeader {
    /// Compute the canonical Deneb SSZ `hash_tree_root`.
    pub fn hash_tree_root(&self) -> Root {
        merkleize(&[
            self.parent_hash,
            byte_vector_root(&self.fee_recipient),
            self.state_root,
            self.receipts_root,
            byte_vector_root(&self.logs_bloom),
            self.prev_randao,
            uint64_root(self.block_number),
            uint64_root(self.gas_limit),
            uint64_root(self.gas_used),
            uint64_root(self.timestamp),
            self.extra_data.hash_tree_root(),
            self.base_fee_per_gas,
            self.block_hash,
            self.transactions_root,
            self.withdrawals_root,
            uint64_root(self.blob_gas_used),
            uint64_root(self.excess_blob_gas),
        ])
    }
}

/// Fixed execution-payload Merkle branch at generalized index 25.
pub type ExecutionBranch = [Root; 4];

/// Official fork-specific SSZ `LightClientHeader`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LightClientHeader {
    /// Altair header (beacon header only).
    Altair {
        /// Beacon header.
        beacon: BeaconBlockHeader,
    },
    /// Bellatrix header (beacon header only).
    Bellatrix {
        /// Beacon header.
        beacon: BeaconBlockHeader,
    },
    /// Capella header with its execution payload proof.
    Capella {
        /// Beacon header.
        beacon: BeaconBlockHeader,
        /// Capella execution payload header.
        execution: Box<CapellaExecutionPayloadHeader>,
        /// Execution payload branch in the beacon block body.
        execution_branch: ExecutionBranch,
    },
    /// Deneb header with its execution payload proof.
    Deneb {
        /// Beacon header.
        beacon: BeaconBlockHeader,
        /// Deneb execution payload header.
        execution: Box<DenebExecutionPayloadHeader>,
        /// Execution payload branch in the beacon block body.
        execution_branch: ExecutionBranch,
    },
    /// Electra header with the unchanged Deneb execution payload layout.
    Electra {
        /// Beacon header.
        beacon: BeaconBlockHeader,
        /// Deneb-format execution payload header.
        execution: Box<DenebExecutionPayloadHeader>,
        /// Execution payload branch in the beacon block body.
        execution_branch: ExecutionBranch,
    },
    /// Fulu header with the unchanged Deneb execution payload layout.
    Fulu {
        /// Beacon header.
        beacon: BeaconBlockHeader,
        /// Deneb-format execution payload header.
        execution: Box<DenebExecutionPayloadHeader>,
        /// Execution payload branch in the beacon block body.
        execution_branch: ExecutionBranch,
    },
}

/// Execution-layer roots authenticated by a Capella-or-later light-client header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthenticatedExecutionBlock {
    /// Consensus fork whose light-client header authenticated the payload.
    pub fork: EthereumFork,
    /// Execution state trie root.
    pub state_root: Root,
    /// Execution receipts trie root.
    pub receipts_root: Root,
    /// Execution block number.
    pub block_number: u64,
    /// Execution block hash.
    pub block_hash: Root,
}

impl LightClientHeader {
    /// Return the beacon header common to all fork variants.
    pub const fn beacon(&self) -> &BeaconBlockHeader {
        match self {
            Self::Altair { beacon }
            | Self::Bellatrix { beacon }
            | Self::Capella { beacon, .. }
            | Self::Deneb { beacon, .. }
            | Self::Electra { beacon, .. }
            | Self::Fulu { beacon, .. } => beacon,
        }
    }

    /// Return the closed fork variant carried by the header.
    pub const fn fork(&self) -> EthereumFork {
        match self {
            Self::Altair { .. } => EthereumFork::Altair,
            Self::Bellatrix { .. } => EthereumFork::Bellatrix,
            Self::Capella { .. } => EthereumFork::Capella,
            Self::Deneb { .. } => EthereumFork::Deneb,
            Self::Electra { .. } => EthereumFork::Electra,
            Self::Fulu { .. } => EthereumFork::Fulu,
        }
    }

    /// Return execution-layer fields authenticated by this header, if present.
    ///
    /// Altair and Bellatrix light-client headers do not carry an execution
    /// payload proof and therefore return `None`.
    pub const fn authenticated_execution_block(&self) -> Option<AuthenticatedExecutionBlock> {
        match self {
            Self::Altair { .. } | Self::Bellatrix { .. } => None,
            Self::Capella { execution, .. } => Some(AuthenticatedExecutionBlock {
                fork: EthereumFork::Capella,
                state_root: execution.state_root,
                receipts_root: execution.receipts_root,
                block_number: execution.block_number,
                block_hash: execution.block_hash,
            }),
            Self::Deneb { execution, .. } => Some(AuthenticatedExecutionBlock {
                fork: EthereumFork::Deneb,
                state_root: execution.state_root,
                receipts_root: execution.receipts_root,
                block_number: execution.block_number,
                block_hash: execution.block_hash,
            }),
            Self::Electra { execution, .. } => Some(AuthenticatedExecutionBlock {
                fork: EthereumFork::Electra,
                state_root: execution.state_root,
                receipts_root: execution.receipts_root,
                block_number: execution.block_number,
                block_hash: execution.block_hash,
            }),
            Self::Fulu { execution, .. } => Some(AuthenticatedExecutionBlock {
                fork: EthereumFork::Fulu,
                state_root: execution.state_root,
                receipts_root: execution.receipts_root,
                block_number: execution.block_number,
                block_hash: execution.block_hash,
            }),
        }
    }

    /// Compute the canonical fork-specific SSZ `hash_tree_root`.
    pub fn hash_tree_root(&self) -> Root {
        match self {
            Self::Altair { beacon } | Self::Bellatrix { beacon } => beacon.hash_tree_root(),
            Self::Capella {
                beacon,
                execution,
                execution_branch,
            } => merkleize(&[
                beacon.hash_tree_root(),
                execution.hash_tree_root(),
                merkleize(execution_branch),
            ]),
            Self::Deneb {
                beacon,
                execution,
                execution_branch,
            }
            | Self::Electra {
                beacon,
                execution,
                execution_branch,
            }
            | Self::Fulu {
                beacon,
                execution,
                execution_branch,
            } => merkleize(&[
                beacon.hash_tree_root(),
                execution.hash_tree_root(),
                merkleize(execution_branch),
            ]),
        }
    }

    fn validate(&self, schedule: &ForkSchedule) -> Result<(), EthereumLightClientError> {
        let (expected, _) = schedule.fork_at_slot(self.beacon().slot)?;
        let actual = self.fork();
        if expected != actual {
            return Err(EthereumLightClientError::HeaderForkMismatch { expected, actual });
        }

        let (execution_root, execution_branch) = match self {
            Self::Altair { .. } | Self::Bellatrix { .. } => return Ok(()),
            Self::Capella {
                execution,
                execution_branch,
                ..
            } => (execution.hash_tree_root(), execution_branch),
            Self::Deneb {
                execution,
                execution_branch,
                ..
            }
            | Self::Electra {
                execution,
                execution_branch,
                ..
            }
            | Self::Fulu {
                execution,
                execution_branch,
                ..
            } => (execution.hash_tree_root(), execution_branch),
        };
        if merkle_root_from_branch(execution_root, EXECUTION_PAYLOAD_GINDEX, execution_branch)
            != Some(self.beacon().body_root)
        {
            return Err(EthereumLightClientError::InvalidExecutionBranch);
        }
        Ok(())
    }
}

/// Canonically encoded compressed BLS12-381 min-pk public key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlsPublicKey([u8; 48]);

impl BlsPublicKey {
    /// Wrap 48 compressed public-key bytes.
    ///
    /// Curve and subgroup validation is performed when the containing sync
    /// committee is admitted.
    pub const fn new(bytes: [u8; 48]) -> Self {
        Self(bytes)
    }

    /// Return compressed public-key bytes.
    pub const fn to_bytes(self) -> [u8; 48] {
        self.0
    }

    fn hash_tree_root(self) -> Root {
        byte_vector_root(&self.0)
    }
}

/// Canonically encoded compressed BLS12-381 min-pk signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlsSignature([u8; 96]);

impl BlsSignature {
    /// Wrap 96 compressed signature bytes.
    pub const fn new(bytes: [u8; 96]) -> Self {
        Self(bytes)
    }

    /// Return compressed signature bytes.
    pub const fn to_bytes(self) -> [u8; 96] {
        self.0
    }

    fn hash_tree_root(self) -> Root {
        byte_vector_root(&self.0)
    }
}

/// Official SSZ `SyncCommittee` with exactly 512 positions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncCommittee {
    pubkeys: Box<[BlsPublicKey; SYNC_COMMITTEE_SIZE]>,
    aggregate_pubkey: BlsPublicKey,
}

impl SyncCommittee {
    /// Construct a fixed-size sync committee.
    pub fn new(
        pubkeys: Box<[BlsPublicKey; SYNC_COMMITTEE_SIZE]>,
        aggregate_pubkey: BlsPublicKey,
    ) -> Self {
        Self {
            pubkeys,
            aggregate_pubkey,
        }
    }

    /// Borrow all 512 committee positions in canonical order.
    pub fn pubkeys(&self) -> &[BlsPublicKey; SYNC_COMMITTEE_SIZE] {
        &self.pubkeys
    }

    /// Return the aggregate public key committed by the beacon state.
    pub const fn aggregate_pubkey(&self) -> BlsPublicKey {
        self.aggregate_pubkey
    }

    /// Compute the canonical SSZ `hash_tree_root`.
    pub fn hash_tree_root(&self) -> Root {
        let pubkey_roots: Vec<_> = self
            .pubkeys
            .iter()
            .copied()
            .map(BlsPublicKey::hash_tree_root)
            .collect();
        hash_nodes(
            &merkleize(&pubkey_roots),
            &self.aggregate_pubkey.hash_tree_root(),
        )
    }

    fn validate(&self) -> Result<(), EthereumLightClientError> {
        for (position, public_key) in self.pubkeys.iter().enumerate() {
            ethereum_bls_pop_validate_public_key(&public_key.0)
                .map_err(|_| EthereumLightClientError::InvalidCommitteePublicKey(position))?;
        }
        ethereum_bls_pop_validate_public_key(&self.aggregate_pubkey.0)
            .map_err(|_| EthereumLightClientError::InvalidCommitteeAggregatePublicKey)
    }
}

/// Official SSZ `SyncAggregate` (`Bitvector[512]` plus one BLS signature).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyncAggregate {
    sync_committee_bits: [u8; SYNC_COMMITTEE_BITS_BYTES],
    sync_committee_signature: BlsSignature,
}

impl SyncAggregate {
    /// Construct a fixed-size sync aggregate.
    pub const fn new(
        sync_committee_bits: [u8; SYNC_COMMITTEE_BITS_BYTES],
        sync_committee_signature: BlsSignature,
    ) -> Self {
        Self {
            sync_committee_bits,
            sync_committee_signature,
        }
    }

    /// Return the SSZ bitvector bytes (least-significant bit first per byte).
    pub const fn bits(&self) -> &[u8; SYNC_COMMITTEE_BITS_BYTES] {
        &self.sync_committee_bits
    }

    /// Return the aggregate signature.
    pub const fn signature(&self) -> BlsSignature {
        self.sync_committee_signature
    }

    /// Count participating sync-committee positions.
    pub fn participant_count(&self) -> usize {
        self.sync_committee_bits
            .iter()
            .map(|byte| byte.count_ones() as usize)
            .sum()
    }

    /// Compute the canonical SSZ `hash_tree_root`.
    pub fn hash_tree_root(&self) -> Root {
        hash_nodes(
            &byte_vector_root(&self.sync_committee_bits),
            &self.sync_committee_signature.hash_tree_root(),
        )
    }
}

/// Fork-shaped current sync-committee branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurrentSyncCommitteeBranch {
    /// Altair through Deneb (`floorlog2(54) == 5`).
    PreElectra([Root; 5]),
    /// Electra and Fulu (`floorlog2(86) == 6`).
    Electra([Root; 6]),
}

impl CurrentSyncCommitteeBranch {
    fn as_slice_for_fork(&self, fork: EthereumFork) -> Result<&[Root], EthereumLightClientError> {
        match (fork.uses_electra_state_layout(), self) {
            (false, Self::PreElectra(branch)) => Ok(branch),
            (true, Self::Electra(branch)) => Ok(branch),
            _ => Err(EthereumLightClientError::CurrentCommitteeBranchForkMismatch),
        }
    }

    fn hash_tree_root(&self) -> Root {
        match self {
            Self::PreElectra(branch) => merkleize(branch),
            Self::Electra(branch) => merkleize(branch),
        }
    }
}

/// Fork-shaped finalized-checkpoint branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalityBranch {
    /// Altair through Deneb (`floorlog2(105) == 6`).
    PreElectra([Root; 6]),
    /// Electra and Fulu (`floorlog2(169) == 7`).
    Electra([Root; 7]),
}

impl FinalityBranch {
    fn as_slice_for_fork(&self, fork: EthereumFork) -> Result<&[Root], EthereumLightClientError> {
        match (fork.uses_electra_state_layout(), self) {
            (false, Self::PreElectra(branch)) => Ok(branch),
            (true, Self::Electra(branch)) => Ok(branch),
            _ => Err(EthereumLightClientError::FinalityBranchForkMismatch),
        }
    }

    fn hash_tree_root(&self) -> Root {
        match self {
            Self::PreElectra(branch) => merkleize(branch),
            Self::Electra(branch) => merkleize(branch),
        }
    }
}

/// Fork-shaped next sync-committee branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NextSyncCommitteeBranch {
    /// Altair through Deneb (`floorlog2(55) == 5`).
    PreElectra([Root; 5]),
    /// Electra and Fulu (`floorlog2(87) == 6`).
    Electra([Root; 6]),
}

impl NextSyncCommitteeBranch {
    fn as_slice_for_fork(&self, fork: EthereumFork) -> Result<&[Root], EthereumLightClientError> {
        match (fork.uses_electra_state_layout(), self) {
            (false, Self::PreElectra(branch)) => Ok(branch),
            (true, Self::Electra(branch)) => Ok(branch),
            _ => Err(EthereumLightClientError::NextCommitteeBranchForkMismatch),
        }
    }

    fn hash_tree_root(&self) -> Root {
        match self {
            Self::PreElectra(branch) => merkleize(branch),
            Self::Electra(branch) => merkleize(branch),
        }
    }
}

/// Official SSZ `LightClientBootstrap` used to validate a governed anchor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LightClientBootstrap {
    /// Header matching the governed trusted block root.
    pub header: LightClientHeader,
    /// Current sync committee committed by the header's beacon state.
    pub current_sync_committee: SyncCommittee,
    /// Fork-shaped current sync-committee branch.
    pub current_sync_committee_branch: CurrentSyncCommitteeBranch,
}

impl LightClientBootstrap {
    /// Compute the canonical fork-specific SSZ `hash_tree_root`.
    pub fn hash_tree_root(&self) -> Root {
        merkleize(&[
            self.header.hash_tree_root(),
            self.current_sync_committee.hash_tree_root(),
            self.current_sync_committee_branch.hash_tree_root(),
        ])
    }
}

/// Official finalized SSZ `LightClientUpdate` subset admitted by SCCP.
///
/// Ethereum's network type also permits zero/default finality or next-committee
/// fields. SCCP intentionally admits only full updates: both branches must be
/// present and valid, and finality must advance. The field order and each field's
/// SSZ root remain identical to the consensus `LightClientUpdate` container.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LightClientUpdate {
    /// Header signed by the sync committee.
    pub attested_header: LightClientHeader,
    /// Next sync committee committed by the attested beacon state.
    pub next_sync_committee: SyncCommittee,
    /// Fork-shaped next sync-committee branch.
    pub next_sync_committee_branch: NextSyncCommitteeBranch,
    /// Finalized header committed by the attested beacon state.
    pub finalized_header: LightClientHeader,
    /// Fork-shaped finalized-checkpoint branch.
    pub finality_branch: FinalityBranch,
    /// Sync committee participation and aggregate signature.
    pub sync_aggregate: SyncAggregate,
    /// Slot at which the aggregate signature was created.
    pub signature_slot: u64,
}

impl LightClientUpdate {
    /// Compute the canonical fork-specific SSZ `hash_tree_root`.
    pub fn hash_tree_root(&self) -> Root {
        merkleize(&[
            self.attested_header.hash_tree_root(),
            self.next_sync_committee.hash_tree_root(),
            self.next_sync_committee_branch.hash_tree_root(),
            self.finalized_header.hash_tree_root(),
            self.finality_branch.hash_tree_root(),
            self.sync_aggregate.hash_tree_root(),
            uint64_root(self.signature_slot),
        ])
    }
}

/// Immutable validated Ethereum light-client state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EthereumLightClientState {
    schedule: ForkSchedule,
    finalized_header: LightClientHeader,
    current_sync_committee: SyncCommittee,
    next_sync_committee: Option<SyncCommittee>,
}

impl EthereumLightClientState {
    /// Validate a bootstrap against a governed trusted beacon block root.
    ///
    /// # Errors
    ///
    /// Returns an error when the fork layout, trusted root, committee keys, or
    /// current-committee state branch is invalid.
    pub fn from_trusted_anchor(
        schedule: ForkSchedule,
        trusted_block_root: Root,
        bootstrap: LightClientBootstrap,
    ) -> Result<Self, EthereumLightClientError> {
        bootstrap.header.validate(&schedule)?;
        if bootstrap.header.beacon().hash_tree_root() != trusted_block_root {
            return Err(EthereumLightClientError::InvalidTrustedBlockRoot);
        }
        bootstrap.current_sync_committee.validate()?;

        let (fork, _) = schedule.fork_at_slot(bootstrap.header.beacon().slot)?;
        let indices = generalized_indices(fork);
        let branch = bootstrap
            .current_sync_committee_branch
            .as_slice_for_fork(fork)?;
        if merkle_root_from_branch(
            bootstrap.current_sync_committee.hash_tree_root(),
            indices.current_sync_committee,
            branch,
        ) != Some(bootstrap.header.beacon().state_root)
        {
            return Err(EthereumLightClientError::InvalidCurrentCommitteeBranch);
        }

        Ok(Self {
            schedule,
            finalized_header: bootstrap.header,
            current_sync_committee: bootstrap.current_sync_committee,
            next_sync_committee: None,
        })
    }

    /// Return the governed fork schedule.
    pub const fn schedule(&self) -> &ForkSchedule {
        &self.schedule
    }

    /// Return the latest validated finalized header.
    pub const fn finalized_header(&self) -> &LightClientHeader {
        &self.finalized_header
    }

    /// Return the committee for the state's current sync-committee period.
    pub const fn current_sync_committee(&self) -> &SyncCommittee {
        &self.current_sync_committee
    }

    /// Return the anchored next sync committee, when learned.
    pub const fn next_sync_committee(&self) -> Option<&SyncCommittee> {
        self.next_sync_committee.as_ref()
    }

    /// Validate an update against this exact immutable state snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid fork layouts, slot/period transitions,
    /// committee branches, participant thresholds, keys, or BLS signatures.
    pub fn validate_update(
        &self,
        update: LightClientUpdate,
    ) -> Result<ValidatedLightClientUpdate, EthereumLightClientError> {
        update.attested_header.validate(&self.schedule)?;
        update.finalized_header.validate(&self.schedule)?;

        let attested_slot = update.attested_header.beacon().slot;
        let finalized_slot = update.finalized_header.beacon().slot;
        if update.signature_slot <= attested_slot || attested_slot < finalized_slot {
            return Err(EthereumLightClientError::InvalidSlotOrder);
        }
        if finalized_slot <= self.finalized_header.beacon().slot {
            return Err(EthereumLightClientError::StaleFinalizedHeader);
        }

        let participants = update.sync_aggregate.participant_count();
        if participants < FINALITY_PARTICIPANT_THRESHOLD {
            return Err(EthereumLightClientError::InsufficientParticipation(
                participants,
            ));
        }

        let store_period = sync_committee_period_at_slot(self.finalized_header.beacon().slot);
        let signature_period = sync_committee_period_at_slot(update.signature_slot);
        let attested_period = sync_committee_period_at_slot(attested_slot);
        let finalized_period = sync_committee_period_at_slot(finalized_slot);

        if signature_period < store_period
            || signature_period > store_period.saturating_add(1)
            || attested_period < store_period
            || attested_period > store_period.saturating_add(1)
            || finalized_period < store_period
            || finalized_period > store_period.saturating_add(1)
        {
            return Err(EthereumLightClientError::SkippedSyncCommitteePeriod);
        }
        if signature_period == store_period.saturating_add(1) && self.next_sync_committee.is_none()
        {
            return Err(EthereumLightClientError::MissingNextSyncCommittee);
        }
        if finalized_period == store_period.saturating_add(1) && self.next_sync_committee.is_none()
        {
            return Err(EthereumLightClientError::MissingNextSyncCommittee);
        }

        let (attested_fork, _) = self.schedule.fork_at_slot(attested_slot)?;
        let indices = generalized_indices(attested_fork);
        let finality_branch = update.finality_branch.as_slice_for_fork(attested_fork)?;
        if merkle_root_from_branch(
            update.finalized_header.beacon().hash_tree_root(),
            indices.finalized_root,
            finality_branch,
        ) != Some(update.attested_header.beacon().state_root)
        {
            return Err(EthereumLightClientError::InvalidFinalityBranch);
        }

        update.next_sync_committee.validate()?;
        let next_branch = update
            .next_sync_committee_branch
            .as_slice_for_fork(attested_fork)?;
        if merkle_root_from_branch(
            update.next_sync_committee.hash_tree_root(),
            indices.next_sync_committee,
            next_branch,
        ) != Some(update.attested_header.beacon().state_root)
        {
            return Err(EthereumLightClientError::InvalidNextCommitteeBranch);
        }

        if attested_period == store_period
            && let Some(anchored_next) = &self.next_sync_committee
            && anchored_next != &update.next_sync_committee
        {
            return Err(EthereumLightClientError::ConflictingNextSyncCommittee);
        }

        let signing_committee = if signature_period == store_period {
            &self.current_sync_committee
        } else {
            self.next_sync_committee
                .as_ref()
                .ok_or(EthereumLightClientError::MissingNextSyncCommittee)?
        };
        let participant_public_keys =
            selected_participant_public_keys(signing_committee, update.sync_aggregate.bits());

        let signing_root = sync_committee_signing_root(
            &update.attested_header,
            update.signature_slot,
            &self.schedule,
        )?;
        let signature = update.sync_aggregate.signature().to_bytes();
        ethereum_bls_pop_fast_aggregate_verify(&participant_public_keys, &signing_root, &signature)
            .map_err(|_| EthereumLightClientError::InvalidSyncCommitteeSignature)?;

        Ok(ValidatedLightClientUpdate {
            parent_state_commitment: self.state_commitment(),
            update,
        })
    }

    /// Apply a validated update and return a new immutable state.
    ///
    /// # Errors
    ///
    /// Returns an error when the validated update belongs to another state or
    /// its required next committee is unavailable.
    pub fn apply_validated_update(
        &self,
        validated: ValidatedLightClientUpdate,
    ) -> Result<Self, EthereumLightClientError> {
        if validated.parent_state_commitment != self.state_commitment() {
            return Err(EthereumLightClientError::UpdateForDifferentState);
        }

        let update = validated.update;
        let store_period = sync_committee_period_at_slot(self.finalized_header.beacon().slot);
        let finalized_period = sync_committee_period_at_slot(update.finalized_header.beacon().slot);

        let (current_sync_committee, next_sync_committee) = if self.next_sync_committee.is_none() {
            if finalized_period != store_period {
                return Err(EthereumLightClientError::MissingNextSyncCommittee);
            }
            (
                self.current_sync_committee.clone(),
                Some(update.next_sync_committee),
            )
        } else if finalized_period == store_period.saturating_add(1) {
            (
                self.next_sync_committee
                    .clone()
                    .ok_or(EthereumLightClientError::MissingNextSyncCommittee)?,
                Some(update.next_sync_committee),
            )
        } else {
            (
                self.current_sync_committee.clone(),
                self.next_sync_committee.clone(),
            )
        };

        Ok(Self {
            schedule: self.schedule,
            finalized_header: update.finalized_header,
            current_sync_committee,
            next_sync_committee,
        })
    }

    /// Validate and atomically derive the next immutable state.
    ///
    /// # Errors
    ///
    /// Returns any update-validation or immutable-state application error.
    pub fn validate_and_apply(
        &self,
        update: LightClientUpdate,
    ) -> Result<Self, EthereumLightClientError> {
        let validated = self.validate_update(update)?;
        self.apply_validated_update(validated)
    }

    /// Return a deterministic commitment to this exact state snapshot.
    pub fn state_commitment(&self) -> Root {
        let mut hasher = Sha256::new();
        hasher.update(b"sccp:ethereum-light-client-state:v1");
        hasher.update(self.schedule.commitment());
        hasher.update(self.finalized_header.hash_tree_root());
        hasher.update(self.current_sync_committee.hash_tree_root());
        if let Some(committee) = &self.next_sync_committee {
            hasher.update([1]);
            hasher.update(committee.hash_tree_root());
        } else {
            hasher.update([0]);
            hasher.update(ZERO_ROOT);
        }
        hasher.finalize().into()
    }
}

/// An update validated against one exact immutable light-client state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedLightClientUpdate {
    parent_state_commitment: Root,
    update: LightClientUpdate,
}

impl ValidatedLightClientUpdate {
    /// Return the state commitment against which the update was validated.
    pub const fn parent_state_commitment(&self) -> Root {
        self.parent_state_commitment
    }

    /// Borrow the validated protocol update.
    pub const fn update(&self) -> &LightClientUpdate {
        &self.update
    }
}

/// Compute the native Ethereum `DOMAIN_SYNC_COMMITTEE` signing root.
///
/// The fork version is selected at `max(signature_slot, 1) - 1`, exactly as in
/// the consensus light-client specification. The returned root is
/// `hash_tree_root(SigningData{hash_tree_root(attested.beacon), domain})`.
///
/// # Errors
///
/// Returns an error when the governed schedule has no supported fork at the
/// signature domain's previous slot.
pub fn sync_committee_signing_root(
    attested_header: &LightClientHeader,
    signature_slot: u64,
    schedule: &ForkSchedule,
) -> Result<Root, EthereumLightClientError> {
    let fork_version_slot = signature_slot.max(1) - 1;
    let (_, activation) = schedule.fork_at_slot(fork_version_slot)?;
    let domain = compute_domain(
        DOMAIN_SYNC_COMMITTEE,
        activation.version,
        schedule.genesis_validators_root,
    );
    Ok(hash_nodes(
        &attested_header.beacon().hash_tree_root(),
        &domain,
    ))
}

/// Compute an Ethereum consensus signature domain.
pub fn compute_domain(
    domain_type: [u8; 4],
    fork_version: [u8; 4],
    genesis_validators_root: Root,
) -> Root {
    let fork_data_root = hash_nodes(&byte_vector_root(&fork_version), &genesis_validators_root);
    let mut domain = [0; 32];
    domain[..4].copy_from_slice(&domain_type);
    domain[4..].copy_from_slice(&fork_data_root[..28]);
    domain
}

fn selected_participant_public_keys(
    committee: &SyncCommittee,
    bits: &[u8; SYNC_COMMITTEE_BITS_BYTES],
) -> Vec<[u8; 48]> {
    let mut selected = Vec::with_capacity(SYNC_COMMITTEE_SIZE);
    for (position, public_key) in committee.pubkeys.iter().enumerate() {
        let mask = 1_u8 << (position % 8);
        if bits[position / 8] & mask != 0 {
            selected.push(public_key.to_bytes());
        }
    }
    selected
}

fn hash_nodes(left: &Root, right: &Root) -> Root {
    let mut hasher = Sha256::new();
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn uint64_root(value: u64) -> Root {
    let mut root = ZERO_ROOT;
    root[..8].copy_from_slice(&value.to_le_bytes());
    root
}

fn usize_root(value: usize) -> Root {
    let mut root = ZERO_ROOT;
    let bytes = value.to_le_bytes();
    root[..bytes.len()].copy_from_slice(&bytes);
    root
}

fn byte_vector_root(bytes: &[u8]) -> Root {
    let chunks: Vec<Root> = bytes
        .chunks(32)
        .map(|chunk| {
            let mut root = ZERO_ROOT;
            root[..chunk.len()].copy_from_slice(chunk);
            root
        })
        .collect();
    merkleize(&chunks)
}

fn merkleize(leaves: &[Root]) -> Root {
    if leaves.is_empty() {
        return ZERO_ROOT;
    }
    let width = leaves.len().next_power_of_two();
    let mut level = Vec::with_capacity(width);
    level.extend_from_slice(leaves);
    level.resize(width, ZERO_ROOT);
    while level.len() > 1 {
        let mut parent = Vec::with_capacity(level.len() / 2);
        for pair in level.chunks_exact(2) {
            parent.push(hash_nodes(&pair[0], &pair[1]));
        }
        level = parent;
    }
    level[0]
}

fn merkle_root_from_branch(leaf: Root, gindex: u64, branch: &[Root]) -> Option<Root> {
    if gindex < 2 {
        return None;
    }
    let depth = (u64::BITS - 1 - gindex.leading_zeros()) as usize;
    if branch.len() != depth {
        return None;
    }
    let mut root = leaf;
    for (height, sibling) in branch.iter().enumerate() {
        root = if (gindex >> height) & 1 == 0 {
            hash_nodes(&root, sibling)
        } else {
            hash_nodes(sibling, &root)
        };
    }
    Some(root)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    const GENERATOR_PUBLIC_KEY: [u8; 48] = [
        0x97, 0xf1, 0xd3, 0xa7, 0x31, 0x97, 0xd7, 0x94, 0x26, 0x95, 0x63, 0x8c, 0x4f, 0xa9, 0xac,
        0x0f, 0xc3, 0x68, 0x8c, 0x4f, 0x97, 0x74, 0xb9, 0x05, 0xa1, 0x4e, 0x3a, 0x3f, 0x17, 0x1b,
        0xac, 0x58, 0x6c, 0x55, 0xe8, 0x3f, 0xf9, 0x7a, 0x1a, 0xef, 0xfb, 0x3a, 0xf0, 0x0a, 0xdb,
        0x22, 0xc6, 0xbb,
    ];
    const NEGATED_GENERATOR_PUBLIC_KEY: [u8; 48] = [
        0xb7, 0xf1, 0xd3, 0xa7, 0x31, 0x97, 0xd7, 0x94, 0x26, 0x95, 0x63, 0x8c, 0x4f, 0xa9, 0xac,
        0x0f, 0xc3, 0x68, 0x8c, 0x4f, 0x97, 0x74, 0xb9, 0x05, 0xa1, 0x4e, 0x3a, 0x3f, 0x17, 0x1b,
        0xac, 0x58, 0x6c, 0x55, 0xe8, 0x3f, 0xf9, 0x7a, 0x1a, 0xef, 0xfb, 0x3a, 0xf0, 0x0a, 0xdb,
        0x22, 0xc6, 0xbb,
    ];
    const FIXTURE_SIGNING_ROOT: Root = [
        0xd1, 0xeb, 0x73, 0x73, 0xa5, 0x5d, 0x8b, 0xf6, 0xd5, 0x10, 0x0d, 0x75, 0x36, 0x3d, 0x1d,
        0x01, 0x27, 0x22, 0xfe, 0x73, 0x57, 0x24, 0xd2, 0x3d, 0xc2, 0x39, 0x4a, 0x77, 0xc2, 0x5d,
        0xe8, 0xe9,
    ];
    // Standard POP-DST signature by secret key 1, added 342 times for 342
    // duplicate committee positions over `FIXTURE_SIGNING_ROOT`.
    const FIXTURE_AGGREGATE_SIGNATURE: [u8; 96] = [
        0xa6, 0x49, 0x4c, 0x5b, 0xc8, 0x3e, 0x50, 0xc9, 0x35, 0xa9, 0xb5, 0xac, 0x35, 0x8d, 0x53,
        0x24, 0x03, 0xad, 0x21, 0x6d, 0xad, 0xcb, 0x9e, 0xe8, 0x20, 0x9e, 0x43, 0xb1, 0x81, 0x6c,
        0xe1, 0xca, 0x50, 0x18, 0x42, 0x32, 0x14, 0xf2, 0x9e, 0x8a, 0x02, 0xfc, 0x9e, 0xa4, 0x3d,
        0xeb, 0x66, 0x6f, 0x01, 0x27, 0x6c, 0xb7, 0x9b, 0x6b, 0xcf, 0xdc, 0xb8, 0xf0, 0xcc, 0xf7,
        0x85, 0x0c, 0xa2, 0xb5, 0xc0, 0xc0, 0x5d, 0x14, 0x29, 0x65, 0x08, 0x38, 0xe2, 0xa4, 0xa8,
        0xa9, 0x01, 0xfd, 0x89, 0x7f, 0xca, 0x47, 0x82, 0x5d, 0xaa, 0x51, 0xc0, 0x13, 0x7a, 0xa5,
        0xb8, 0x66, 0x71, 0x96, 0xc9, 0x1a,
    ];

    fn root(tag: u8) -> Root {
        [tag; 32]
    }

    fn schedule_with_epochs(epochs: [u64; 6]) -> ForkSchedule {
        ForkSchedule::new(
            root(0xa5),
            [
                ForkActivation::new(epochs[0], [1, 0, 0, 0]),
                ForkActivation::new(epochs[1], [2, 0, 0, 0]),
                ForkActivation::new(epochs[2], [3, 0, 0, 0]),
                ForkActivation::new(epochs[3], [4, 0, 0, 0]),
                ForkActivation::new(epochs[4], [5, 0, 0, 0]),
                ForkActivation::new(epochs[5], [6, 0, 0, 0]),
            ],
        )
        .expect("valid test schedule")
    }

    fn altair_schedule() -> ForkSchedule {
        schedule_with_epochs([0, u64::MAX, u64::MAX, u64::MAX, u64::MAX, u64::MAX])
    }

    fn boxed_public_keys(public_key: [u8; 48]) -> Box<[BlsPublicKey; SYNC_COMMITTEE_SIZE]> {
        vec![BlsPublicKey::new(public_key); SYNC_COMMITTEE_SIZE]
            .into_boxed_slice()
            .try_into()
            .expect("sync committee vector has the fixed protocol length")
    }

    fn committee(public_key: [u8; 48]) -> SyncCommittee {
        SyncCommittee::new(boxed_public_keys(public_key), BlsPublicKey::new(public_key))
    }

    fn sparse_node(gindex: u64, max_depth: usize, explicit: &BTreeMap<u64, Root>) -> Root {
        if let Some(value) = explicit.get(&gindex) {
            return *value;
        }
        let depth = (u64::BITS - 1 - gindex.leading_zeros()) as usize;
        if depth == max_depth {
            return ZERO_ROOT;
        }
        hash_nodes(
            &sparse_node(gindex * 2, max_depth, explicit),
            &sparse_node(gindex * 2 + 1, max_depth, explicit),
        )
    }

    fn sparse_branch(target: u64, max_depth: usize, explicit: &BTreeMap<u64, Root>) -> Vec<Root> {
        let depth = (u64::BITS - 1 - target.leading_zeros()) as usize;
        let mut branch = Vec::with_capacity(depth);
        let mut node = target;
        for _ in 0..depth {
            branch.push(sparse_node(node ^ 1, max_depth, explicit));
            node >>= 1;
        }
        branch
    }

    fn altair_header(slot: u64, state_root: Root) -> LightClientHeader {
        LightClientHeader::Altair {
            beacon: BeaconBlockHeader {
                slot,
                proposer_index: slot + 10,
                parent_root: root(0x31),
                state_root,
                body_root: root(0x32),
            },
        }
    }

    fn anchored_state() -> EthereumLightClientState {
        let schedule = altair_schedule();
        let current = committee(GENERATOR_PUBLIC_KEY);
        let mut explicit = BTreeMap::new();
        explicit.insert(
            CURRENT_SYNC_COMMITTEE_GINDEX_PRE_ELECTRA,
            current.hash_tree_root(),
        );
        let state_root = sparse_node(1, 5, &explicit);
        let header = altair_header(1, state_root);
        let trusted = header.beacon().hash_tree_root();
        let branch: [Root; 5] =
            sparse_branch(CURRENT_SYNC_COMMITTEE_GINDEX_PRE_ELECTRA, 5, &explicit)
                .try_into()
                .expect("current branch length");
        EthereumLightClientState::from_trusted_anchor(
            schedule,
            trusted,
            LightClientBootstrap {
                header,
                current_sync_committee: current,
                current_sync_committee_branch: CurrentSyncCommitteeBranch::PreElectra(branch),
            },
        )
        .expect("valid anchor")
    }

    fn participant_bits(count: usize) -> [u8; SYNC_COMMITTEE_BITS_BYTES] {
        let mut bits = [0; SYNC_COMMITTEE_BITS_BYTES];
        for position in 0..count {
            bits[position / 8] |= 1 << (position % 8);
        }
        bits
    }

    fn unsigned_update(
        finalized_slot: u64,
        attested_slot: u64,
        signature_slot: u64,
        next: SyncCommittee,
        signature: [u8; 96],
    ) -> LightClientUpdate {
        let finalized_header = altair_header(finalized_slot, root(0x41));
        let mut explicit = BTreeMap::new();
        explicit.insert(
            FINALIZED_ROOT_GINDEX_PRE_ELECTRA,
            finalized_header.beacon().hash_tree_root(),
        );
        explicit.insert(
            NEXT_SYNC_COMMITTEE_GINDEX_PRE_ELECTRA,
            next.hash_tree_root(),
        );
        let state_root = sparse_node(1, 6, &explicit);
        let attested_header = altair_header(attested_slot, state_root);
        let finality_branch: [Root; 6] =
            sparse_branch(FINALIZED_ROOT_GINDEX_PRE_ELECTRA, 6, &explicit)
                .try_into()
                .expect("finality branch length");
        let next_branch: [Root; 5] =
            sparse_branch(NEXT_SYNC_COMMITTEE_GINDEX_PRE_ELECTRA, 6, &explicit)
                .try_into()
                .expect("next branch length");

        LightClientUpdate {
            attested_header,
            next_sync_committee: next,
            next_sync_committee_branch: NextSyncCommitteeBranch::PreElectra(next_branch),
            finalized_header,
            finality_branch: FinalityBranch::PreElectra(finality_branch),
            sync_aggregate: SyncAggregate::new(
                participant_bits(FINALITY_PARTICIPANT_THRESHOLD),
                BlsSignature::new(signature),
            ),
            signature_slot,
        }
    }

    fn blank_capella_execution() -> CapellaExecutionPayloadHeader {
        CapellaExecutionPayloadHeader {
            parent_hash: root(1),
            fee_recipient: [2; 20],
            state_root: root(3),
            receipts_root: root(4),
            logs_bloom: [5; 256],
            prev_randao: root(6),
            block_number: 7,
            gas_limit: 8,
            gas_used: 9,
            timestamp: 10,
            extra_data: ExtraData::new(vec![11, 12]).expect("bounded extra data"),
            base_fee_per_gas: root(13),
            block_hash: root(14),
            transactions_root: root(15),
            withdrawals_root: root(16),
        }
    }

    fn blank_deneb_execution() -> DenebExecutionPayloadHeader {
        let capella = blank_capella_execution();
        DenebExecutionPayloadHeader {
            parent_hash: capella.parent_hash,
            fee_recipient: capella.fee_recipient,
            state_root: capella.state_root,
            receipts_root: capella.receipts_root,
            logs_bloom: capella.logs_bloom,
            prev_randao: capella.prev_randao,
            block_number: capella.block_number,
            gas_limit: capella.gas_limit,
            gas_used: capella.gas_used,
            timestamp: capella.timestamp,
            extra_data: capella.extra_data,
            base_fee_per_gas: capella.base_fee_per_gas,
            block_hash: capella.block_hash,
            transactions_root: capella.transactions_root,
            withdrawals_root: capella.withdrawals_root,
            blob_gas_used: 17,
            excess_blob_gas: 18,
        }
    }

    #[test]
    fn generalized_indices_switch_only_at_electra() {
        assert_eq!(
            generalized_indices(EthereumFork::Deneb),
            LightClientGeneralizedIndices {
                finalized_root: 105,
                current_sync_committee: 54,
                next_sync_committee: 55,
            }
        );
        assert_eq!(
            generalized_indices(EthereumFork::Electra),
            LightClientGeneralizedIndices {
                finalized_root: 169,
                current_sync_committee: 86,
                next_sync_committee: 87,
            }
        );
        assert_eq!(
            generalized_indices(EthereumFork::Fulu),
            generalized_indices(EthereumFork::Electra)
        );
    }

    #[test]
    fn ssz_roots_match_official_consensus_spec_vectors() {
        let beacon_header = BeaconBlockHeader {
            slot: 16_101_738_745_833_384_750,
            proposer_index: 15_310_703_651_237_606_601,
            parent_root: [
                0xca, 0xf0, 0xdb, 0x22, 0x47, 0x0a, 0x8e, 0x98, 0x7f, 0xef, 0x3d, 0x78, 0xce, 0x7a,
                0x14, 0xbb, 0xd7, 0x71, 0x45, 0x70, 0xb7, 0x8d, 0x5d, 0x5b, 0xc4, 0x42, 0x1e, 0xa2,
                0xb6, 0x57, 0xf9, 0x86,
            ],
            state_root: [
                0x77, 0x13, 0x4d, 0xe1, 0xd4, 0xdc, 0x7a, 0x8f, 0xfb, 0x09, 0x8e, 0x30, 0x5a, 0xe5,
                0xcc, 0xfb, 0x7c, 0xb2, 0x7c, 0xba, 0x58, 0x66, 0x71, 0x2f, 0x95, 0x75, 0x6d, 0xdb,
                0xc0, 0x44, 0xd3, 0x28,
            ],
            body_root: [
                0x93, 0x4a, 0xf8, 0x49, 0x56, 0xd1, 0xa2, 0x52, 0xaa, 0x76, 0x74, 0x06, 0xa8, 0xba,
                0xe9, 0xe2, 0x6b, 0x08, 0x3a, 0x81, 0xbe, 0x4b, 0x17, 0x03, 0xf1, 0x57, 0xa7, 0x0a,
                0xbc, 0xfb, 0x60, 0x85,
            ],
        };
        assert_eq!(
            beacon_header.hash_tree_root(),
            [
                0xa2, 0x67, 0x27, 0x69, 0x76, 0x3d, 0x19, 0xc7, 0x9d, 0xd7, 0xa5, 0x84, 0xf3, 0x7f,
                0xd3, 0x39, 0x1c, 0x05, 0x10, 0xc1, 0xfd, 0x6e, 0xba, 0x54, 0xec, 0x0c, 0xd7, 0x87,
                0x48, 0xa8, 0x48, 0x00,
            ]
        );

        let aggregate = SyncAggregate::new(
            [
                0x1a, 0x3c, 0x06, 0x9c, 0xd6, 0x2b, 0x40, 0x60, 0x7c, 0x5c, 0xff, 0xe6, 0xc1, 0xa4,
                0x49, 0x5e, 0x35, 0xa5, 0x92, 0xa4, 0x02, 0xc4, 0x48, 0x7e, 0x7a, 0xfc, 0x06, 0xa7,
                0x2a, 0x94, 0x52, 0xe1, 0xb9, 0x95, 0xb1, 0x6a, 0x15, 0xb8, 0x50, 0x8e, 0xe3, 0x56,
                0xec, 0xfa, 0xcd, 0x08, 0xc1, 0xa0, 0x6c, 0x7a, 0x03, 0xd6, 0x19, 0xd5, 0x5c, 0x9e,
                0x45, 0x3d, 0x14, 0xf3, 0xcf, 0x6f, 0x7e, 0x01,
            ],
            BlsSignature::new([
                0xf9, 0x8c, 0xbd, 0x1e, 0x49, 0x57, 0xd4, 0xb2, 0xd7, 0xdd, 0x0f, 0x50, 0x9e, 0x5e,
                0xe1, 0x56, 0x85, 0x91, 0xe5, 0x67, 0x44, 0xfe, 0xe3, 0x1d, 0x24, 0x48, 0xc9, 0xcb,
                0x81, 0xbe, 0xc4, 0x2d, 0x49, 0xc8, 0x06, 0xd8, 0xb0, 0xef, 0x8f, 0x18, 0x76, 0xb0,
                0x6c, 0xb0, 0xe1, 0xdd, 0xd9, 0xcf, 0x37, 0x82, 0x3a, 0xee, 0xc1, 0x55, 0xb0, 0x51,
                0x93, 0x0b, 0x36, 0x49, 0x50, 0xab, 0xa8, 0x5c, 0x9d, 0x96, 0x51, 0x2a, 0x7c, 0x42,
                0x15, 0x11, 0x8a, 0x5f, 0xba, 0x5f, 0x8e, 0x80, 0x49, 0xed, 0xb4, 0x71, 0xa8, 0x4d,
                0xed, 0x72, 0xc2, 0x65, 0xa7, 0x7b, 0x08, 0x2b, 0x35, 0x48, 0x40, 0x24,
            ]),
        );
        assert_eq!(
            aggregate.hash_tree_root(),
            [
                0xa9, 0x4d, 0x16, 0x49, 0x1c, 0x1a, 0x75, 0x69, 0x9d, 0x1b, 0xb7, 0xed, 0x7e, 0x37,
                0x79, 0x68, 0xfd, 0x99, 0xd7, 0x7c, 0x77, 0x13, 0xb2, 0xc1, 0xae, 0x13, 0x5f, 0x26,
                0x7b, 0x10, 0x70, 0xa6,
            ]
        );
    }

    #[test]
    fn fork_schedule_is_closed_ordered_and_governed() {
        assert_eq!(
            ForkSchedule::new(root(1), [ForkActivation::new(0, [0; 4]); 6]),
            Err(EthereumLightClientError::InvalidForkSchedule(
                "fork versions must be unique"
            ))
        );
        let mut activations = [ForkActivation::new(0, [0; 4]); 6];
        for (index, activation) in activations.iter_mut().enumerate() {
            let index = u8::try_from(index).expect("six fork activations fit in u8");
            *activation = ForkActivation::new(u64::from(index), [index, 0, 0, 1]);
        }
        activations[3] = ForkActivation::new(1, [3, 0, 0, 1]);
        assert_eq!(
            ForkSchedule::new(root(1), activations),
            Err(EthereumLightClientError::InvalidForkSchedule(
                "activation epochs must be nondecreasing"
            ))
        );
        assert_eq!(
            ForkSchedule::new(ZERO_ROOT, [ForkActivation::new(0, [1, 0, 0, 0]); 6]),
            Err(EthereumLightClientError::ZeroGenesisValidatorsRoot)
        );
    }

    #[test]
    fn execution_header_is_bound_at_gindex_25() {
        let schedule = schedule_with_epochs([0, 1, 2, u64::MAX, u64::MAX, u64::MAX]);
        let execution = blank_capella_execution();
        let branch = [root(21), root(22), root(23), root(24)];
        let body_root = merkle_root_from_branch(
            execution.hash_tree_root(),
            EXECUTION_PAYLOAD_GINDEX,
            &branch,
        )
        .expect("fixed execution branch");
        let header = LightClientHeader::Capella {
            beacon: BeaconBlockHeader {
                slot: 2 * SLOTS_PER_EPOCH,
                proposer_index: 1,
                parent_root: root(25),
                state_root: root(26),
                body_root,
            },
            execution: Box::new(execution),
            execution_branch: branch,
        };
        // Consensus-spec Capella `LightClientHeader` is a three-field SSZ
        // container.  Its fourth padded leaf is zero; `execution` must not be
        // duplicated (which would produce a different root while leaving the
        // execution-payload branch itself apparently valid).
        let expected_header_root = hash_nodes(
            &hash_nodes(
                &header.beacon().hash_tree_root(),
                &blank_capella_execution().hash_tree_root(),
            ),
            &hash_nodes(&merkleize(&branch), &ZERO_ROOT),
        );
        assert_eq!(header.hash_tree_root(), expected_header_root);
        header.validate(&schedule).expect("valid execution branch");

        let mut tampered = header.clone();
        if let LightClientHeader::Capella { beacon, .. } = &mut tampered {
            beacon.body_root[0] ^= 1;
        }
        assert_eq!(
            tampered.validate(&schedule),
            Err(EthereumLightClientError::InvalidExecutionBranch)
        );

        let wrong_variant = altair_header(2 * SLOTS_PER_EPOCH, root(2));
        assert_eq!(
            wrong_variant.validate(&schedule),
            Err(EthereumLightClientError::HeaderForkMismatch {
                expected: EthereumFork::Capella,
                actual: EthereumFork::Altair,
            })
        );
    }

    #[test]
    fn electra_anchor_requires_the_electra_branch_shape_and_gindex() {
        let schedule = schedule_with_epochs([0, 0, 0, 0, 0, u64::MAX]);
        let current = committee(GENERATOR_PUBLIC_KEY);
        let mut explicit = BTreeMap::new();
        explicit.insert(
            CURRENT_SYNC_COMMITTEE_GINDEX_ELECTRA,
            current.hash_tree_root(),
        );
        let state_root = sparse_node(1, 6, &explicit);
        let execution = blank_deneb_execution();
        let execution_branch = [root(71), root(72), root(73), root(74)];
        let body_root = merkle_root_from_branch(
            execution.hash_tree_root(),
            EXECUTION_PAYLOAD_GINDEX,
            &execution_branch,
        )
        .expect("fixed execution branch");
        let header = LightClientHeader::Electra {
            beacon: BeaconBlockHeader {
                slot: 1,
                proposer_index: 2,
                parent_root: root(75),
                state_root,
                body_root,
            },
            execution: Box::new(execution),
            execution_branch,
        };
        let trusted = header.beacon().hash_tree_root();
        let branch: [Root; 6] = sparse_branch(CURRENT_SYNC_COMMITTEE_GINDEX_ELECTRA, 6, &explicit)
            .try_into()
            .expect("Electra current branch length");
        let state = EthereumLightClientState::from_trusted_anchor(
            schedule,
            trusted,
            LightClientBootstrap {
                header: header.clone(),
                current_sync_committee: current.clone(),
                current_sync_committee_branch: CurrentSyncCommitteeBranch::Electra(branch),
            },
        )
        .expect("Electra anchor validates with gindex 86");
        assert_eq!(state.finalized_header().fork(), EthereumFork::Electra);

        assert_eq!(
            EthereumLightClientState::from_trusted_anchor(
                schedule,
                trusted,
                LightClientBootstrap {
                    header,
                    current_sync_committee: current,
                    current_sync_committee_branch: CurrentSyncCommitteeBranch::PreElectra(
                        [ZERO_ROOT; 5],
                    ),
                },
            ),
            Err(EthereumLightClientError::CurrentCommitteeBranchForkMismatch)
        );
    }

    #[test]
    fn extra_data_bound_is_strict() {
        assert!(ExtraData::new(vec![0; 32]).is_ok());
        assert_eq!(
            ExtraData::new(vec![0; 33]),
            Err(EthereumLightClientError::ExtraDataTooLong(33))
        );
    }

    #[test]
    fn bootstrap_rejects_wrong_trust_root_branch_and_key() {
        let state = anchored_state();
        assert_eq!(state.finalized_header().beacon().slot, 1);

        let schedule = altair_schedule();
        let current = committee(GENERATOR_PUBLIC_KEY);
        let header = altair_header(1, root(9));
        assert_eq!(
            EthereumLightClientState::from_trusted_anchor(
                schedule,
                root(8),
                LightClientBootstrap {
                    header: header.clone(),
                    current_sync_committee: current.clone(),
                    current_sync_committee_branch: CurrentSyncCommitteeBranch::PreElectra(
                        [ZERO_ROOT; 5]
                    ),
                },
            ),
            Err(EthereumLightClientError::InvalidTrustedBlockRoot)
        );
        assert_eq!(
            EthereumLightClientState::from_trusted_anchor(
                schedule,
                header.beacon().hash_tree_root(),
                LightClientBootstrap {
                    header: header.clone(),
                    current_sync_committee: current,
                    current_sync_committee_branch: CurrentSyncCommitteeBranch::PreElectra(
                        [ZERO_ROOT; 5]
                    ),
                },
            ),
            Err(EthereumLightClientError::InvalidCurrentCommitteeBranch)
        );

        let invalid = committee([0xff; 48]);
        assert_eq!(
            EthereumLightClientState::from_trusted_anchor(
                schedule,
                header.beacon().hash_tree_root(),
                LightClientBootstrap {
                    header,
                    current_sync_committee: invalid,
                    current_sync_committee_branch: CurrentSyncCommitteeBranch::PreElectra(
                        [ZERO_ROOT; 5]
                    ),
                },
            ),
            Err(EthereumLightClientError::InvalidCommitteePublicKey(0))
        );
    }

    #[test]
    fn update_rejects_threshold_slot_branch_and_signature_attacks() {
        let state = anchored_state();
        let mut update = unsigned_update(2, 3, 4, committee(GENERATOR_PUBLIC_KEY), [0; 96]);
        assert_eq!(
            state.validate_update(update.clone()),
            Err(EthereumLightClientError::InvalidSyncCommitteeSignature)
        );

        update.sync_aggregate = SyncAggregate::new(
            participant_bits(FINALITY_PARTICIPANT_THRESHOLD - 1),
            BlsSignature::new([0; 96]),
        );
        assert_eq!(
            state.validate_update(update.clone()),
            Err(EthereumLightClientError::InsufficientParticipation(341))
        );

        update.sync_aggregate = SyncAggregate::new(
            participant_bits(FINALITY_PARTICIPANT_THRESHOLD),
            BlsSignature::new([0; 96]),
        );
        update.signature_slot = update.attested_header.beacon().slot;
        assert_eq!(
            state.validate_update(update.clone()),
            Err(EthereumLightClientError::InvalidSlotOrder)
        );

        let mut stale_update = unsigned_update(1, 3, 4, committee(GENERATOR_PUBLIC_KEY), [0; 96]);
        assert_eq!(
            state.validate_update(stale_update.clone()),
            Err(EthereumLightClientError::StaleFinalizedHeader)
        );
        stale_update.finality_branch = FinalityBranch::Electra([ZERO_ROOT; 7]);
        // Staleness is intentionally rejected before untrusted branch work.
        assert_eq!(
            state.validate_update(stale_update),
            Err(EthereumLightClientError::StaleFinalizedHeader)
        );

        let mut bad_branch = unsigned_update(2, 3, 4, committee(GENERATOR_PUBLIC_KEY), [0; 96]);
        if let FinalityBranch::PreElectra(branch) = &mut bad_branch.finality_branch {
            branch[0][0] ^= 1;
        }
        assert_eq!(
            state.validate_update(bad_branch),
            Err(EthereumLightClientError::InvalidFinalityBranch)
        );

        let mut bad_next_branch =
            unsigned_update(2, 3, 4, committee(GENERATOR_PUBLIC_KEY), [0; 96]);
        if let NextSyncCommitteeBranch::PreElectra(branch) =
            &mut bad_next_branch.next_sync_committee_branch
        {
            branch[0][0] ^= 1;
        }
        assert_eq!(
            state.validate_update(bad_next_branch),
            Err(EthereumLightClientError::InvalidNextCommitteeBranch)
        );

        let invalid_next = committee([0xff; 48]);
        let invalid_key_update = unsigned_update(2, 3, 4, invalid_next, [0; 96]);
        assert_eq!(
            state.validate_update(invalid_key_update),
            Err(EthereumLightClientError::InvalidCommitteePublicKey(0))
        );

        let mut wrong_shape = unsigned_update(2, 3, 4, committee(GENERATOR_PUBLIC_KEY), [0; 96]);
        wrong_shape.next_sync_committee_branch = NextSyncCommitteeBranch::Electra([ZERO_ROOT; 6]);
        assert_eq!(
            state.validate_update(wrong_shape),
            Err(EthereumLightClientError::NextCommitteeBranchForkMismatch)
        );

        let transition_without_next = unsigned_update(
            SLOTS_PER_SYNC_COMMITTEE_PERIOD,
            SLOTS_PER_SYNC_COMMITTEE_PERIOD + 1,
            SLOTS_PER_SYNC_COMMITTEE_PERIOD + 2,
            committee(GENERATOR_PUBLIC_KEY),
            [0; 96],
        );
        assert_eq!(
            state.validate_update(transition_without_next),
            Err(EthereumLightClientError::MissingNextSyncCommittee)
        );

        let skipped = unsigned_update(
            2,
            3,
            2 * SLOTS_PER_SYNC_COMMITTEE_PERIOD,
            committee(GENERATOR_PUBLIC_KEY),
            [0; 96],
        );
        assert_eq!(
            state.validate_update(skipped),
            Err(EthereumLightClientError::SkippedSyncCommitteePeriod)
        );
    }

    #[test]
    fn exact_threshold_update_with_duplicate_positions_advances_immutably() {
        let state = anchored_state();
        let update = unsigned_update(
            2,
            3,
            4,
            committee(GENERATOR_PUBLIC_KEY),
            FIXTURE_AGGREGATE_SIGNATURE,
        );
        assert_eq!(
            sync_committee_signing_root(
                &update.attested_header,
                update.signature_slot,
                state.schedule(),
            ),
            Ok(FIXTURE_SIGNING_ROOT)
        );
        assert_eq!(
            update.sync_aggregate.participant_count(),
            FINALITY_PARTICIPANT_THRESHOLD
        );
        assert!(state.next_sync_committee().is_none());

        let validated = state
            .validate_update(update)
            .expect("342 duplicate positions form a valid aggregate");
        assert_eq!(
            validated.parent_state_commitment(),
            state.state_commitment()
        );
        let next = state
            .apply_validated_update(validated.clone())
            .expect("validated update applies to its parent snapshot");
        assert_eq!(state.finalized_header().beacon().slot, 1);
        assert_eq!(next.finalized_header().beacon().slot, 2);
        assert!(state.next_sync_committee().is_none());
        assert!(next.next_sync_committee().is_some());
        assert_eq!(
            next.apply_validated_update(validated),
            Err(EthereumLightClientError::UpdateForDifferentState)
        );

        let conflicting_committee = SyncCommittee::new(
            boxed_public_keys(GENERATOR_PUBLIC_KEY),
            BlsPublicKey::new(NEGATED_GENERATOR_PUBLIC_KEY),
        );
        let conflicting = unsigned_update(3, 4, 5, conflicting_committee, [0; 96]);
        assert_eq!(
            next.validate_update(conflicting),
            Err(EthereumLightClientError::ConflictingNextSyncCommittee)
        );
    }

    #[test]
    fn signing_root_uses_previous_slot_fork_and_governed_genesis_root() {
        let schedule = schedule_with_epochs([0, 1, u64::MAX, u64::MAX, u64::MAX, u64::MAX]);
        let header = LightClientHeader::Altair {
            beacon: BeaconBlockHeader {
                slot: 31,
                ..BeaconBlockHeader::default()
            },
        };
        let at_boundary = sync_committee_signing_root(&header, 32, &schedule)
            .expect("previous slot remains Altair");
        let after_boundary = sync_committee_signing_root(&header, 33, &schedule)
            .expect("previous slot selects Bellatrix");
        assert_ne!(at_boundary, after_boundary);

        let other_schedule =
            ForkSchedule::new(root(0xa6), schedule.activations).expect("second governed schedule");
        assert_ne!(
            at_boundary,
            sync_committee_signing_root(&header, 32, &other_schedule)
                .expect("different genesis root")
        );
    }

    #[test]
    fn fixture_signing_root_is_stable() {
        let state = anchored_state();
        let update = unsigned_update(2, 3, 4, committee(GENERATOR_PUBLIC_KEY), [0; 96]);
        let signing_root = sync_committee_signing_root(
            &update.attested_header,
            update.signature_slot,
            state.schedule(),
        )
        .expect("fixture root");
        assert_eq!(signing_root, FIXTURE_SIGNING_ROOT);
    }
}
