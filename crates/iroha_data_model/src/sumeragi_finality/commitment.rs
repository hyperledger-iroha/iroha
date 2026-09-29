//! Current consensus result commitments shared by execution and independent proof readers.
use super::{NativeLaneStateProof, ScheduleOutcome};
use crate::{
    block::execution_output::ExecutionOutputV1, parameter::system::SumeragiParameters,
    transaction::signed::TransactionEntrypoint,
};
use crate::{
    consensus::FinalizedGlobalThresholdBeaconPulseV1, isi::kagemusha_v1::BeaconEpochBindingV1,
    parameter::system::ConsensusMode,
};
use iroha_crypto::{Hash, MerkleTreeCommitment};
use iroha_sumeragi::{
    api::ConfigError,
    pacemaker::{FRAME_OVERHEAD, validate_chain},
    types::{ChainParams, Hash32},
};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    derive::{JsonDeserialize, JsonSerialize},
};
use thiserror::Error;
/// Largest consensus frame every node's transport accepts, the chain-wide bound that on-chain
/// chain parameters are validated against (§9.4, O10): 16 MiB of payload plus the core's frame
/// overhead. It is a protocol constant, not node configuration, so validation is deterministic;
/// it equals the driver's default frame limit.
pub const CHAIN_TRANSPORT_FRAME_LIMIT: u64 = 16 * 1024 * 1024 + FRAME_OVERHEAD as u64;

/// Chain parameters of one height as stored in World and committed in `R` (§10.1, §12.4): the
/// Norito and JSON form of the core's [`ChainParams`].
#[derive(norito::NoritoSchema, iroha_schema::IntoSchema)]
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

/// Hard bound for the canonical result preimage, including complete bounded epoch contexts.
pub const MAX_RESULT_PREIMAGE_BYTES: usize = 64 * 1024;

/// The chain hash `H` (§1): `iroha_crypto::Hash` as a core [`Hash32`].
#[must_use]
pub fn chain_hash(bytes: &[u8]) -> Hash32 {
    Hash32(<[u8; 32]>::from(Hash::new(bytes)))
}

/// `R` of a canonical result preimage: `H(RESULT_TAG ‖ preimage)`.
/// Streams the domain and borrowed preimage without allocating a second payload buffer.
#[must_use]
pub fn result_of_preimage(preimage: &[u8]) -> Hash32 {
    Hash32(Hash::new_from_chunks(&[RESULT_TAG, preimage]).into())
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

/// The canonical preimage of `R`: exact executed height, execution, complete native schedule
/// graph and the finalized beacon pulse consumed by this execution, when present.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ExecutionResultCommitment {
    /// Exact height whose execution and schedule this result authenticates.
    pub height: u64,
    /// What executing the block produced.
    pub execution: ExecutionCommitment,
    /// Full current context and authenticated successor schedule, including boundary decisions.
    pub schedule: ScheduleOutcome,
    /// Finalized unique threshold-beacon pulse verified against execution prestate.
    /// Historical readers check its canonical public bindings; its threshold verification is
    /// attested by the exact current quorum, not reconstructed without the full session.
    pub beacon: Option<FinalizedGlobalThresholdBeaconPulseV1>,
    /// Complete native context-set proof bound to the same network, height and write root.
    pub native_lanes: NativeLaneStateProof,
}

impl norito::NoritoSchema for ExecutionResultCommitment {
    fn nominal_name() -> String {
        "iroha_data_model::sumeragi_finality::ExecutionResultCommitment".to_owned()
    }
    fn static_frame_name() -> Option<&'static str> {
        // Canonical streaming must not allocate a String merely to hash this declared identity.
        Some("iroha_data_model::sumeragi_finality::ExecutionResultCommitment")
    }
}

impl ExecutionResultCommitment {
    /// Bind an execution to its exact native epoch graph and finalized beacon pulse.
    ///
    /// # Errors
    /// The height, complete epoch contexts, schedule graph or public pulse bindings are invalid.
    pub fn new(
        height: u64,
        execution: ExecutionCommitment,
        schedule: ScheduleOutcome,
        beacon: Option<FinalizedGlobalThresholdBeaconPulseV1>,
        native_lanes: NativeLaneStateProof,
    ) -> Result<Self, CommitmentError> {
        let value = Self {
            height,
            execution,
            schedule,
            beacon,
            native_lanes,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate the complete graph and public pulse bindings before trusting its authority.
    /// This does not independently verify a threshold signature: execution must verify that
    /// signature with the authenticated complete beacon session before certifying this result.
    ///
    /// # Errors
    /// An inconsistent height, invalid graph, missing required pulse or malformed pulse.
    pub fn validate(&self) -> Result<(), CommitmentError> {
        if self.height == 0 || self.schedule.height != self.height {
            return Err(CommitmentError::Schedule(
                "result and schedule heights differ".into(),
            ));
        }
        self.schedule
            .validate()
            .map_err(|error| CommitmentError::Schedule(error.to_string()))?;
        let current = &self.schedule.current;
        if !self.native_lanes.verify(
            current.network_id,
            self.height,
            self.execution.ordinary_writes_root,
        ) {
            return Err(CommitmentError::NativeLaneState);
        }
        let required = current.mode == ConsensusMode::Npos
            && self.height > 1
            && self.height.checked_add(1) == Some(current.authorization.last_height);
        let Some(pulse) = self.beacon.as_ref() else {
            return if required {
                Err(CommitmentError::Beacon(
                    "missing boundary-selection pulse".into(),
                ))
            } else {
                Ok(())
            };
        };
        super::validate_beacon_pulse_shape(pulse)
            .map_err(|error| CommitmentError::Beacon(error.to_string()))?;
        if pulse.network_id != current.network_id
            || pulse.height != self.height
            || pulse.finalized_chain_anchor.height.checked_add(1) != Some(self.height)
        {
            return Err(CommitmentError::Beacon(
                "pulse belongs to another network, height or anchor".into(),
            ));
        }
        if let BeaconEpochBindingV1::Installed(binding) = current.authorization.beacon {
            if pulse.session_id != binding.session_id
                || pulse.transcript_hash != binding.transcript_hash
            {
                return Err(CommitmentError::Beacon(
                    "pulse differs from authenticated epoch session".into(),
                ));
            }
        }
        Ok(())
    }

    /// The canonical preimage bytes (stored as `CommitCertificate.result_preimage`).
    ///
    /// # Errors
    /// A Norito serialization failure.
    pub fn preimage(&self) -> Result<Vec<u8>, CommitmentError> {
        let len = norito::canonical_frame_len(self)
            .map_err(|error| CommitmentError::Encoding(error.to_string()))?;
        if len > MAX_RESULT_PREIMAGE_BYTES {
            return Err(CommitmentError::PreimageLength(len));
        }
        norito::encode_canonical(self).map_err(|error| CommitmentError::Encoding(error.to_string()))
    }

    /// Decode a canonical preimage (e.g. from a stored or received certificate).
    ///
    /// # Errors
    /// The bytes are not one canonical frame of this type.
    pub fn decode(preimage: &[u8]) -> Result<Self, CommitmentError> {
        if preimage.len() > MAX_RESULT_PREIMAGE_BYTES {
            return Err(CommitmentError::PreimageLength(preimage.len()));
        }
        let decoded: Self = norito::decode_canonical_with_limits(
            preimage,
            norito::DecodeLimits::new(
                96,
                MAX_RESULT_PREIMAGE_BYTES,
                8192,
                4 * MAX_RESULT_PREIMAGE_BYTES,
                32,
            ),
        )
        .map_err(|error| CommitmentError::Encoding(error.to_string()))?;
        decoded.validate()?;
        Ok(decoded)
    }

    /// `R` of this commitment.
    ///
    /// # Errors
    /// A Norito serialization failure.
    pub fn result(&self) -> Result<Hash32, CommitmentError> {
        self.preimage().map(|bytes| result_of_preimage(&bytes))
    }
}

/// Why `R` could not be computed. Every variant is a deterministic function of the executed
/// block and its witness (a local bug, never the proposer's fault alone).
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum CommitmentError {
    /// The executed block carries no execution result.
    #[error("the executed block has no execution result")]
    MissingResult,
    /// The executed block already carries a commit certificate.
    #[error("the executed block already carries a commit certificate")]
    CertifiedBlock,
    /// The block's outputs or their Merkle cache are malformed.
    #[error("malformed execution outputs: {0}")]
    InvalidOutputs(String),
    /// The result-bearing block wire is empty or above the protocol bound.
    #[error("the executed block wire length {0} is out of range")]
    WireLength(u64),
    /// The witness carries malformed or duplicate KAGEMUSHA receipts.
    #[error("invalid KAGEMUSHA top-ups: {0}")]
    KagemushaTopUps(String),
    /// The complete native epoch schedule is invalid.
    #[error("invalid epoch schedule: {0}")]
    Schedule(String),
    /// Complete context proof differs from the exact native execution carrier.
    #[error("invalid native context proof")]
    NativeLaneState,
    /// A finalized pulse has malformed or inconsistent public bindings.
    #[error("invalid finalized beacon pulse: {0}")]
    Beacon(String),
    /// A canonical result preimage exceeds the protocol byte bound.
    #[error("result preimage exceeds its byte bound: {0}")]
    PreimageLength(usize),
    /// A Norito encoding or decoding failure.
    #[error("encoding: {0}")]
    Encoding(String),
}
