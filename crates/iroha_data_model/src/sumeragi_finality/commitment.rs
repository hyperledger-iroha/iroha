//! Current consensus result commitments shared by execution and independent proof readers.
use crate::{
    block::execution_output::ExecutionOutputV1, parameter::system::SumeragiParameters,
    transaction::signed::TransactionEntrypoint,
};
use iroha_crypto::{Hash, MerkleTreeCommitment};
use iroha_sumeragi::{
    api::ConfigError,
    pacemaker::{FRAME_OVERHEAD, validate_chain},
    preimage::committee_digest_preimage,
    types::{ChainParams, Hash32, HeightConfig},
};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    derive::{JsonDeserialize, JsonSerialize},
};
/// A malformed canonical current execution commitment.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommitmentCodecError {
    /// The bytes do not have the canonical result layout.
    #[error("execution commitment encoding: {0}")]
    Encoding(String),
}
/// Largest consensus frame every node's transport accepts, the chain-wide bound that on-chain
/// chain parameters are validated against (§9.4, O10): 16 MiB of payload plus the core's frame
/// overhead. It is a protocol constant, not node configuration, so validation is deterministic;
/// it equals the driver's default frame limit.
pub const CHAIN_TRANSPORT_FRAME_LIMIT: u64 = 16 * 1024 * 1024 + FRAME_OVERHEAD as u64;

/// Chain parameters of one height as stored in World and committed in `R` (§10.1, §12.4): the
/// Norito and JSON form of the core's [`ChainParams`].
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::ChainParamsRecord")]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
pub struct ChainParamsRecord {
    /// Target block time in milliseconds (`block_cadence_ms`).
    pub block_time_ms: u64,
    /// Bounded payload rebuild retry interval in milliseconds.
    pub payload_retry_interval_ms: u64,
    /// Execution budget `E_max` in milliseconds.
    pub exec_budget_ms: u64,
    /// Apply budget `A_max` in milliseconds.
    pub apply_budget_ms: u64,
    /// Largest block payload in bytes.
    pub max_block_bytes: u32,
    /// Epoch length in heights.
    pub epoch_length_blocks: u64,
}

impl ChainParamsRecord {
    /// The chain parameters the on-chain Sumeragi parameters define.
    #[must_use]
    pub fn from_parameters(params: &SumeragiParameters) -> Self {
        Self {
            block_time_ms: params.block_cadence_ms.get(),
            payload_retry_interval_ms: params.payload_retry_interval_ms.get(),
            exec_budget_ms: params.exec_budget_ms.get(),
            apply_budget_ms: params.apply_budget_ms.get(),
            max_block_bytes: params.max_block_bytes.get(),
            epoch_length_blocks: params.epoch_length_blocks.get(),
        }
    }

    /// The core's chain parameters.
    #[must_use]
    pub fn to_core(&self) -> ChainParams {
        ChainParams {
            block_time: self.block_time_ms,
            payload_retry_interval: self.payload_retry_interval_ms,
            e_max: self.exec_budget_ms,
            a_max: self.apply_budget_ms,
            max_block_bytes: self.max_block_bytes,
            epoch_length: self.epoch_length_blocks,
        }
    }

    /// The record of the core's chain parameters.
    #[must_use]
    pub fn from_core(params: &ChainParams) -> Self {
        Self {
            block_time_ms: params.block_time,
            payload_retry_interval_ms: params.payload_retry_interval,
            exec_budget_ms: params.e_max,
            apply_budget_ms: params.a_max,
            max_block_bytes: params.max_block_bytes,
            epoch_length_blocks: params.epoch_length,
        }
    }

    /// §9.4 validation against [`CHAIN_TRANSPORT_FRAME_LIMIT`].
    ///
    /// # Errors
    /// The first violated rule.
    pub fn validate(&self) -> Result<(), ConfigError> {
        validate_chain(&self.to_core(), CHAIN_TRANSPORT_FRAME_LIMIT)
    }
}

/// Domain tag of `R` (§4.1).
pub const RESULT_TAG: &[u8] = b"iroha/sumeragi/result/v1";

/// The chain hash `H` (§1): `iroha_crypto::Hash` as a core [`Hash32`].
#[must_use]
pub fn chain_hash(bytes: &[u8]) -> Hash32 {
    Hash32(<[u8; 32]>::from(Hash::new(bytes)))
}

/// `R` of a canonical result preimage: `H(RESULT_TAG ‖ preimage)`.
#[must_use]
pub fn result_of_preimage(preimage: &[u8]) -> Hash32 {
    let mut bytes = Vec::with_capacity(RESULT_TAG.len() + preimage.len());
    bytes.extend_from_slice(RESULT_TAG);
    bytes.extend_from_slice(preimage);
    chain_hash(&bytes)
}

/// The deterministic outcome of executing one block: roots over the execution witness and the
/// identity of the result-bearing block. The v2 commitment without its native-AMX, lane-finality
/// and merge-carrier fields.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::ExecutionCommitment")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ExecutionCommitment {
    /// Root of the witnessed pre-state values of the keys the block changed.
    pub parent_state_root: Hash,
    /// Post-state root of the witnessed writes (combined with the KAGEMUSHA top-up root when
    /// the block carries top-ups).
    pub post_state_root: Hash,
    /// Root of the canonical last-write-wins witnessed writes.
    pub ordinary_writes_root: Hash,
    /// Root of the KAGEMUSHA top-up tree, when the block carries top-ups.
    pub kagemusha_top_up_root: Option<Hash>,
    /// Number of KAGEMUSHA top-ups.
    pub kagemusha_top_up_count: u32,
    /// Byte length of the canonical result-bearing block wire.
    pub executed_block_wire_len: u64,
    /// Hash of the canonical result-bearing block wire (every transaction result and output).
    pub executed_block_wire_hash: Hash,
    /// Network-input Merkle commitment of the block.
    pub transaction_input_commitment: Option<MerkleTreeCommitment<TransactionEntrypoint>>,
    /// Typed-output Merkle commitment of the block, including internal invocations.
    pub transaction_output_commitment: Option<MerkleTreeCommitment<ExecutionOutputV1>>,
}

/// The preimage of `R` (§4.1): the execution commitment and the configuration of `h + 2`.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::ExecutionResultCommitment")]
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ExecutionResultCommitment {
    /// What executing the block produced.
    pub execution: ExecutionCommitment,
    /// `committee_digest(C_{h+2})` (§2.1) under the chain hash.
    pub next_committee_digest: [u8; 32],
    /// `ChainParams_{h+2}`.
    pub next_params: ChainParamsRecord,
}

impl ExecutionResultCommitment {
    /// Bind `execution` to the configuration `next` scheduled for `h + 2`.
    #[must_use]
    pub fn new(execution: ExecutionCommitment, next: &HeightConfig) -> Self {
        Self {
            execution,
            next_committee_digest: chain_hash(&committee_digest_preimage(&next.committee)).0,
            next_params: ChainParamsRecord::from_core(&next.params),
        }
    }

    /// The canonical preimage bytes (stored as `CommitCertificate.result_preimage`).
    ///
    /// # Errors
    /// A Norito serialization failure.
    pub fn preimage(&self) -> Result<Vec<u8>, CommitmentCodecError> {
        norito::encode_canonical(self)
            .map_err(|error| CommitmentCodecError::Encoding(error.to_string()))
    }

    /// Decode a canonical preimage (e.g. from a stored or received certificate).
    ///
    /// # Errors
    /// The bytes are not one canonical frame of this type.
    pub fn decode(preimage: &[u8]) -> Result<Self, CommitmentCodecError> {
        norito::decode_canonical(preimage)
            .map_err(|error| CommitmentCodecError::Encoding(error.to_string()))
    }

    /// `R` of this commitment.
    ///
    /// # Errors
    /// A Norito serialization failure.
    pub fn result(&self) -> Result<Hash32, CommitmentCodecError> {
        self.preimage().map(|bytes| result_of_preimage(&bytes))
    }
}
