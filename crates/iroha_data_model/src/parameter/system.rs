//! Built-in parameter definitions and validation logic.

use core::{
    convert::TryFrom,
    num::{NonZeroU16, NonZeroU64},
    time::Duration,
};
#[cfg(feature = "json")]
use std::collections::BTreeMap;
use std::sync::LazyLock;

use iroha_crypto::Algorithm;
use iroha_data_model_derive::model;
use iroha_primitives::{json::Json, numeric::Quantity};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize};

pub use self::model::*;
#[cfg(feature = "json")]
use super::custom::json_helpers;
use super::custom::{CustomParameter, CustomParameterId, CustomParameters};

/// Raw 32-byte consensus fingerprint with one canonical JSON representation.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
)]
pub struct ConsensusFingerprint(pub [u8; 32]);

impl ConsensusFingerprint {
    /// Construct from the canonical raw digest bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return the raw digest bytes.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl core::fmt::Display for ConsensusFingerprint {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "0x{}", hex::encode(self.0))
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for ConsensusFingerprint {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&format!("0x{}", hex::encode(self.0)), out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for ConsensusFingerprint {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let encoded = String::json_deserialize(parser)?;
        let body = encoded
            .strip_prefix("0x")
            .ok_or_else(|| json::Error::InvalidField {
                field: "consensus_fingerprint".to_owned(),
                message: "expected lowercase 0x-prefixed 32-byte hexadecimal digest".to_owned(),
            })?;
        if body.len() != 64
            || !body
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(json::Error::InvalidField {
                field: "consensus_fingerprint".to_owned(),
                message: "expected lowercase 0x-prefixed 32-byte hexadecimal digest".to_owned(),
            });
        }
        let mut bytes = [0; 32];
        hex::decode_to_slice(body, &mut bytes).map_err(|error| json::Error::InvalidField {
            field: "consensus_fingerprint".to_owned(),
            message: error.to_string(),
        })?;
        Ok(Self(bytes))
    }
}

/// Canonical signed Sumeragi v2 handshake metadata stored in genesis.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
)]
#[norito(deny_unknown_fields)]
pub struct ConsensusHandshakeMetadata {
    /// Signed first-release consensus mode.
    pub mode: SumeragiConsensusMode,
    /// Signed immutable block cadence in milliseconds.
    pub block_cadence_ms: NonZeroU64,
    /// First-release consensus wire version.
    pub wire_protocol_version: u32,
    /// Canonical consensus fingerprint.
    pub consensus_fingerprint: ConsensusFingerprint,
    /// Signed inputs for the first Sumeragi v2 height context.
    pub sumeragi_v2: crate::block::consensus_v2::SumeragiV2GenesisContextParameters,
}

impl ConsensusHandshakeMetadata {
    /// Validate the first-release signed handshake envelope.
    ///
    pub fn validate(&self) -> Result<(), String> {
        let expected_version = u32::from(crate::block::consensus_v2::PROTOCOL_VERSION);
        if self.wire_protocol_version != expected_version {
            return Err(
                "wire_protocol_version must equal the first-release protocol version".to_owned(),
            );
        }
        self.sumeragi_v2
            .validate()
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}

#[cfg(feature = "json")]
mod json_support {
    use std::string::String;

    use super::*;

    pub(super) type Map = BTreeMap<String, json::Value>;

    pub(super) fn write_field<T: JsonSerialize>(
        out: &mut String,
        first: &mut bool,
        key: &str,
        value: &T,
    ) {
        if *first {
            *first = false;
        } else {
            out.push(',');
        }
        json::write_json_string(key, out);
        out.push(':');
        value.json_serialize(out);
    }

    pub(super) fn expect_object(value: json::Value, context: &str) -> Result<Map, json::Error> {
        match value {
            json::Value::Object(map) => Ok(map),
            _ => Err(json::Error::InvalidField {
                field: context.into(),
                message: String::from("expected object"),
            }),
        }
    }

    pub(super) fn ensure_no_extra(map: Map) -> Result<(), json::Error> {
        if let Some((field, _)) = map.into_iter().next() {
            return Err(json::Error::UnknownField { field });
        }
        Ok(())
    }

    pub(super) fn expect_u64(value: &json::Value, field: &str) -> Result<u64, json::Error> {
        match value {
            json::Value::Number(num) => num.as_u64().ok_or_else(|| json::Error::InvalidField {
                field: field.to_owned(),
                message: String::from("expected unsigned integer"),
            }),
            _ => Err(json::Error::InvalidField {
                field: field.to_owned(),
                message: String::from("expected unsigned integer"),
            }),
        }
    }

    pub(super) fn expect_nonzero_u64(
        value: &json::Value,
        field: &str,
    ) -> Result<NonZeroU64, json::Error> {
        let raw = expect_u64(value, field)?;
        NonZeroU64::new(raw).ok_or_else(|| json::Error::InvalidField {
            field: field.to_owned(),
            message: String::from("expected non-zero integer"),
        })
    }

    pub(super) fn expect_bool(value: &json::Value, field: &str) -> Result<bool, json::Error> {
        match value {
            json::Value::Bool(b) => Ok(*b),
            _ => Err(json::Error::InvalidField {
                field: field.to_owned(),
                message: String::from("expected boolean"),
            }),
        }
    }

    pub(super) fn expect_u8(value: &json::Value, field: &str) -> Result<u8, json::Error> {
        let raw = expect_u64(value, field)?;
        u8::try_from(raw).map_err(|_| json::Error::InvalidField {
            field: field.to_owned(),
            message: String::from("value out of range for u8"),
        })
    }

    pub(super) fn parse_value_as<T>(value: &json::Value) -> Result<T, json::Error>
    where
        T: JsonDeserialize,
    {
        let json =
            norito::json::to_json(value).map_err(|err| json::Error::Message(err.to_string()))?;
        let mut parser = norito::json::Parser::new(&json);
        T::json_deserialize(&mut parser)
    }
}

#[cfg(feature = "json")]
fn parse_custom_parameters_value(value: json::Value) -> Result<CustomParameters, json::Error> {
    let map = json_support::expect_object(value, "Parameters.custom")?;
    let value = json::Value::Object(map);
    json_helpers::from_value(value)
}
#[model]
mod model {
    use derive_more::Display;
    use getset::{CopyGetters, Getters};
    use iroha_schema::IntoSchema;
    use norito::codec::{Decode, Encode};

    use super::*;

    /// Consensus runtime mode
    #[derive(
        Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema,
    )]
    pub enum SumeragiConsensusMode {
        /// Classic permissioned Sumeragi
        Permissioned,
        /// Nominated Proof‑of‑Stake mode
        Npos,
    }

    // Bridge Norito codec to core slice decoding for strict-safe consumers.
    impl<'a> norito::core::DecodeFromSlice<'a> for SumeragiConsensusMode {
        fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
            let mut s: &'a [u8] = bytes;
            let value = <Self as norito::codec::DecodeAll>::decode_all(&mut s)
                .map_err(|e| norito::core::Error::Message(format!("codec decode error: {e}")))?;
            let used = bytes.len() - s.len();
            Ok((value, used))
        }
    }

    /// Limits that govern consensus operation
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[display("{block_cadence_ms},{max_clock_drift_ms}_SL")]
    pub struct SumeragiParameters {
        /// Signed-genesis block cadence in milliseconds.
        ///
        /// A block is created if this limit or [`BlockParameters::max_transactions`] limit is reached,
        /// whichever comes first. Regardless of the limits, an empty block is never created.
        /// Sumeragi v2 freezes this value at startup for both permissioned and
        /// `NPoS` operation; post-genesis updates are rejected.
        #[norito(default = "defaults::sumeragi::block_cadence_ms")]
        pub block_cadence_ms: NonZeroU64,
        /// Maximal allowed random deviation from the nominal rate
        ///
        /// # Warning
        ///
        /// This value should be kept as low as possible to not affect soundness of the consensus
        #[norito(default = "defaults::sumeragi::max_clock_drift_ms")]
        pub max_clock_drift_ms: u64,
        /// Minimum lead time (blocks) between publishing a new consensus key and activation.
        #[norito(default = "defaults::sumeragi::key_activation_lead_blocks")]
        pub key_activation_lead_blocks: u64,
        /// Overlap/grace window (blocks) permitting dual-signing during rotation.
        #[norito(default = "defaults::sumeragi::key_overlap_grace_blocks")]
        pub key_overlap_grace_blocks: u64,
        /// Expiry grace window (blocks) after declared expiry.
        #[norito(default = "defaults::sumeragi::key_expiry_grace_blocks")]
        pub key_expiry_grace_blocks: u64,
        /// Require HSM binding for consensus/committee keys.
        #[norito(default = "defaults::sumeragi::key_require_hsm")]
        pub key_require_hsm: bool,
        /// Allowed algorithms for consensus/committee keys.
        #[norito(default = "defaults::sumeragi::key_allowed_algorithms")]
        pub key_allowed_algorithms: Vec<iroha_crypto::Algorithm>,
        /// Allowed HSM providers for consensus/committee keys.
        #[norito(default = "defaults::sumeragi::key_allowed_hsm_providers")]
        pub key_allowed_hsm_providers: Vec<String>,
    }

    /// NPoS-specific consensus parameters persisted as a custom parameter payload.
    #[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    #[norito(deny_unknown_fields)]
    pub struct SumeragiNposParameters {
        /// Deterministic epoch seed used for PRF-based leader and validator selection.
        pub epoch_seed: [u8; 32],
        /// VRF commit window length in blocks.
        pub vrf_commit_window_blocks: u64,
        /// VRF reveal window length in blocks.
        pub vrf_reveal_window_blocks: u64,
        /// Maximum validators to elect for the next epoch (0 = unlimited).
        pub max_validators: u32,
        /// Minimum self-bond required for validator eligibility.
        pub min_self_bond: Quantity,
        /// Minimum nomination bond required for delegators.
        pub min_nomination_bond: Quantity,
        /// Maximum nominator concentration percentage.
        pub max_nominator_concentration_pct: u8,
        /// Seat allocation variance band percentage.
        pub seat_band_pct: u8,
        /// Maximum correlation percentage across validator entities.
        pub max_entity_correlation_pct: u8,
        /// Finality margin in blocks before activating a newly elected set.
        pub finality_margin_blocks: u64,
        /// Evidence retention horizon in blocks.
        pub evidence_horizon_blocks: u64,
        /// Activation lag in blocks for newly scheduled validator sets.
        pub activation_lag_blocks: u64,
        /// Slashing delay in blocks before evidence penalties apply.
        pub slashing_delay_blocks: u64,
        /// Epoch length in blocks.
        pub epoch_length_blocks: NonZeroU64,
    }

    impl SumeragiNposParameters {
        /// Identifier used for the custom parameter storing `NPoS` tunables.
        pub const PARAMETER_ID_STR: &'static str = "sumeragi_npos_parameters";

        /// Construct the [`CustomParameterId`] associated with this payload.
        #[must_use]
        pub fn parameter_id() -> CustomParameterId {
            Self::PARAMETER_ID_STR
                .parse()
                .expect("valid sumeragi_npos custom parameter identifier")
        }

        /// Convert the payload into a [`CustomParameter`].
        #[must_use]
        pub fn into_custom_parameter(self) -> CustomParameter {
            CustomParameter::new(Self::parameter_id(), Json::new(self))
        }

        /// Attempt to decode this payload from a [`CustomParameter`].
        #[must_use]
        pub fn from_custom_parameter(custom: &CustomParameter) -> Option<Self> {
            if custom.id != Self::parameter_id() {
                return None;
            }
            let value = norito::json::from_str::<Self>(custom.payload().get()).ok()?;
            value.validate().ok()?;
            Some(value)
        }

        /// VRF commit window measured in blocks.
        #[must_use]
        pub fn vrf_commit_window_blocks(&self) -> u64 {
            self.vrf_commit_window_blocks
        }

        /// VRF reveal window measured in blocks.
        #[must_use]
        pub fn vrf_reveal_window_blocks(&self) -> u64 {
            self.vrf_reveal_window_blocks
        }

        /// Maximum validators to elect for the next epoch (0 = unlimited).
        #[must_use]
        pub fn max_validators(&self) -> u32 {
            self.max_validators
        }

        /// Minimum self-bonded stake required for validators.
        #[must_use]
        pub fn min_self_bond(&self) -> &Quantity {
            &self.min_self_bond
        }

        /// Minimum nomination bond required for delegators.
        #[must_use]
        pub fn min_nomination_bond(&self) -> &Quantity {
            &self.min_nomination_bond
        }

        /// Maximum percentage of stake concentrated under a single nominator.
        #[must_use]
        pub fn max_nominator_concentration_pct(&self) -> u8 {
            self.max_nominator_concentration_pct
        }

        /// Seat band percentage used when selecting validators.
        #[must_use]
        pub fn seat_band_pct(&self) -> u8 {
            self.seat_band_pct
        }

        /// Maximum correlation percentage allowed across validator entities.
        #[must_use]
        pub fn max_entity_correlation_pct(&self) -> u8 {
            self.max_entity_correlation_pct
        }

        /// Finality margin in blocks before activating a newly elected set.
        #[must_use]
        pub fn finality_margin_blocks(&self) -> u64 {
            self.finality_margin_blocks
        }

        /// Number of blocks for which slashing evidence remains valid.
        #[must_use]
        pub fn evidence_horizon_blocks(&self) -> u64 {
            self.evidence_horizon_blocks
        }

        /// Number of blocks to wait before activating newly elected validators.
        #[must_use]
        pub fn activation_lag_blocks(&self) -> u64 {
            self.activation_lag_blocks
        }

        /// Slashing delay (blocks) before evidence penalties apply.
        #[must_use]
        pub fn slashing_delay_blocks(&self) -> u64 {
            self.slashing_delay_blocks
        }

        /// Epoch length measured in blocks.
        #[must_use]
        pub fn epoch_length_blocks(&self) -> NonZeroU64 {
            self.epoch_length_blocks
        }

        /// Epoch seed used for randomness-dependent protocols.
        #[must_use]
        pub fn epoch_seed(&self) -> [u8; 32] {
            self.epoch_seed
        }

        /// Override the epoch seed and return the updated payload.
        #[must_use]
        pub fn with_epoch_seed(mut self, value: [u8; 32]) -> Self {
            self.epoch_seed = value;
            self
        }

        /// Validate all signed NPoS election and reconfiguration invariants.
        ///
        /// # Errors
        /// Returns a stable diagnostic when a seed, window, bond, percentage,
        /// or reconfiguration bound is invalid.
        pub fn validate(&self) -> Result<(), &'static str> {
            if self.epoch_seed == [0; 32] {
                return Err("epoch_seed must not be all zero");
            }
            if self.vrf_commit_window_blocks == 0 || self.vrf_reveal_window_blocks == 0 {
                return Err("VRF commit and reveal windows must be greater than zero");
            }
            if self
                .vrf_commit_window_blocks
                .checked_add(self.vrf_reveal_window_blocks)
                .is_none_or(|total| total > self.epoch_length_blocks.get())
            {
                return Err("VRF commit and reveal windows must fit within the epoch");
            }
            if self.min_self_bond.is_zero() || self.min_nomination_bond.is_zero() {
                return Err("NPoS minimum bond values must be greater than zero");
            }
            if self.max_nominator_concentration_pct > 100
                || self.seat_band_pct > 100
                || self.max_entity_correlation_pct > 100
            {
                return Err("NPoS election percentages must be in 0..=100");
            }
            if self.finality_margin_blocks == 0
                || self.evidence_horizon_blocks == 0
                || self.activation_lag_blocks == 0
                || self.slashing_delay_blocks == 0
            {
                return Err("NPoS finality and reconfiguration bounds must be greater than zero");
            }
            Ok(())
        }
    }

    impl Default for SumeragiNposParameters {
        fn default() -> Self {
            use defaults::sumeragi::npos::*;
            Self {
                epoch_seed: [0xA5; 32],
                vrf_commit_window_blocks: vrf_commit_window_blocks(),
                vrf_reveal_window_blocks: vrf_reveal_window_blocks(),
                max_validators: max_validators(),
                min_self_bond: min_self_bond(),
                min_nomination_bond: min_nomination_bond(),
                max_nominator_concentration_pct: max_nominator_concentration_pct(),
                seat_band_pct: seat_band_pct(),
                max_entity_correlation_pct: max_entity_correlation_pct(),
                finality_margin_blocks: finality_margin_blocks(),
                evidence_horizon_blocks: evidence_horizon_blocks(),
                activation_lag_blocks: activation_lag_blocks(),
                slashing_delay_blocks: slashing_delay_blocks(),
                epoch_length_blocks: epoch_length_blocks(),
            }
        }
    }

    impl From<SumeragiNposParameters> for CustomParameter {
        fn from(value: SumeragiNposParameters) -> Self {
            value.into_custom_parameter()
        }
    }

    /// Single Sumeragi parameter
    ///
    /// Check [`SumeragiParameters`] for more details
    #[derive(
        Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
    )]
    pub enum SumeragiParameter {
        /// Maximum admitted clock drift. Block cadence is frozen in genesis and
        /// therefore deliberately has no mutable parameter variant.
        MaxClockDriftMs(u64),
    }

    /// Limits that a block must obey to be accepted.
    #[derive(
        Debug,
        Display,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        CopyGetters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[display("{max_transactions}_BL")]
    #[getset(get_copy = "pub")]
    pub struct BlockParameters {
        /// Maximal number of transactions in a block.
        ///
        /// A block is created if this limit is reached or [`SumeragiParameters::block_cadence_ms`] has expired,
        /// whichever comes first. Regardless of the limits, an empty block is never created.
        pub max_transactions: NonZeroU64,
    }

    /// Single block parameter
    ///
    /// Check [`BlockParameters`] for more details
    #[derive(
        Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
    )]
    pub enum BlockParameter {
        MaxTransactions(NonZeroU64),
    }

    /// Limits that a transaction must obey to be accepted.
    #[derive(
        Debug,
        Display,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        CopyGetters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[display(
        "{max_signatures},{max_instructions},{ivm_bytecode_size},{max_tx_bytes},{max_decompressed_bytes},{max_metadata_depth}_TL"
    )]
    #[getset(get_copy = "pub")]
    pub struct TransactionParameters {
        /// Maximum number of signatures allowed per transaction (non-zero).
        pub max_signatures: NonZeroU64,
        /// Maximum number of instructions per transaction
        pub max_instructions: NonZeroU64,
        /// Maximum size of IVM bytecode, in bytes
        pub ivm_bytecode_size: NonZeroU64,
        /// Maximum encoded transaction size accepted at admission (bytes).
        pub max_tx_bytes: NonZeroU64,
        /// Maximum post-decompression size accepted at admission (bytes).
        pub max_decompressed_bytes: NonZeroU64,
        /// Maximum allowed JSON nesting depth across transaction metadata values.
        pub max_metadata_depth: NonZeroU16,
        /// Enforce height-based TTL metadata (`expires_at_height`) at admission.
        #[norito(default)]
        pub require_height_ttl: bool,
        /// Enforce strictly monotonic per-sender sequence metadata (`tx_sequence`) at admission.
        #[norito(default)]
        pub require_sequence: bool,
    }

    /// Single transaction parameter
    ///
    /// Check [`TransactionParameters`] for more details
    #[derive(
        Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
    )]
    pub enum TransactionParameter {
        MaxSignatures(NonZeroU64),
        MaxInstructions(NonZeroU64),
        IvmBytecodeSize(NonZeroU64),
        MaxTxBytes(NonZeroU64),
        MaxDecompressedBytes(NonZeroU64),
        MaxMetadataDepth(NonZeroU16),
        RequireHeightTtl(bool),
        RequireSequence(bool),
    }

    /// Limits that a smart contract must obey at runtime to considered valid.
    #[derive(
        Debug,
        Display,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        CopyGetters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[display("{fuel},{memory}_SCL")]
    #[getset(get_copy = "pub")]
    pub struct SmartContractParameters {
        /// Maximum amount of fuel that a smart contract can consume
        pub fuel: NonZeroU64,
        /// Maximum amount of memory that a smart contract can use
        pub memory: NonZeroU64,
        /// Maximum length of chained data trigger executions
        pub execution_depth: u8,
    }

    /// Single smart contract parameter
    ///
    /// Check [`SmartContractParameters`] for more details
    #[derive(
        Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
    )]
    pub enum SmartContractParameter {
        Fuel(NonZeroU64),
        Memory(NonZeroU64),
        ExecutionDepth(u8),
    }

    /// Set of all current blockchain parameter values
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Default,
        Getters,
        CopyGetters,
        Decode,
        Encode,
        IntoSchema,
    )]
    pub struct Parameters {
        /// Sumeragi parameters
        #[getset(get = "pub")]
        #[norito(default)]
        pub sumeragi: SumeragiParameters,
        /// Block parameters
        #[getset(get_copy = "pub")]
        #[norito(default)]
        pub block: BlockParameters,
        /// Transaction parameters
        #[getset(get_copy = "pub")]
        #[norito(default)]
        pub transaction: TransactionParameters,
        /// Executor parameters
        #[getset(get_copy = "pub")]
        #[norito(default)]
        pub executor: SmartContractParameters,
        /// Smart contract parameters
        #[getset(get_copy = "pub")]
        #[norito(default)]
        pub smart_contract: SmartContractParameters,
        /// Collection of blockchain specific parameters
        #[getset(get = "pub")]
        #[norito(default)]
        #[norito(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
        pub custom: CustomParameters,
    }

    /// Single blockchain parameter.
    ///
    /// Check [`Parameters`] for more details
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub enum Parameter {
        Sumeragi(SumeragiParameter),
        Block(BlockParameter),
        Transaction(TransactionParameter),
        SmartContract(SmartContractParameter),
        Executor(SmartContractParameter),
        Custom(CustomParameter),
    }
}

impl core::fmt::Display for Parameter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Sumeragi(v) => core::fmt::Display::fmt(&v, f),
            Self::Block(v) => core::fmt::Display::fmt(&v, f),
            Self::Transaction(v) => core::fmt::Display::fmt(&v, f),
            Self::SmartContract(v) | Self::Executor(v) => core::fmt::Display::fmt(&v, f),
            Self::Custom(v) => write!(f, "{}({})", v.id, v.payload),
        }
    }
}

#[cfg(feature = "json")]
#[derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SumeragiNposParametersJson {
    epoch_seed: [u8; 32],
    vrf_commit_window_blocks: u64,
    vrf_reveal_window_blocks: u64,
    max_validators: u32,
    min_self_bond: Quantity,
    min_nomination_bond: Quantity,
    max_nominator_concentration_pct: u8,
    seat_band_pct: u8,
    max_entity_correlation_pct: u8,
    finality_margin_blocks: u64,
    evidence_horizon_blocks: u64,
    activation_lag_blocks: u64,
    slashing_delay_blocks: u64,
    epoch_length_blocks: NonZeroU64,
}

#[cfg(feature = "json")]
impl From<SumeragiNposParameters> for SumeragiNposParametersJson {
    fn from(value: SumeragiNposParameters) -> Self {
        Self {
            epoch_seed: value.epoch_seed,
            vrf_commit_window_blocks: value.vrf_commit_window_blocks,
            vrf_reveal_window_blocks: value.vrf_reveal_window_blocks,
            max_validators: value.max_validators,
            min_self_bond: value.min_self_bond,
            min_nomination_bond: value.min_nomination_bond,
            max_nominator_concentration_pct: value.max_nominator_concentration_pct,
            seat_band_pct: value.seat_band_pct,
            max_entity_correlation_pct: value.max_entity_correlation_pct,
            finality_margin_blocks: value.finality_margin_blocks,
            evidence_horizon_blocks: value.evidence_horizon_blocks,
            activation_lag_blocks: value.activation_lag_blocks,
            slashing_delay_blocks: value.slashing_delay_blocks,
            epoch_length_blocks: value.epoch_length_blocks,
        }
    }
}

#[cfg(feature = "json")]
impl From<SumeragiNposParametersJson> for SumeragiNposParameters {
    fn from(value: SumeragiNposParametersJson) -> Self {
        Self {
            epoch_seed: value.epoch_seed,
            vrf_commit_window_blocks: value.vrf_commit_window_blocks,
            vrf_reveal_window_blocks: value.vrf_reveal_window_blocks,
            max_validators: value.max_validators,
            min_self_bond: value.min_self_bond,
            min_nomination_bond: value.min_nomination_bond,
            max_nominator_concentration_pct: value.max_nominator_concentration_pct,
            seat_band_pct: value.seat_band_pct,
            max_entity_correlation_pct: value.max_entity_correlation_pct,
            finality_margin_blocks: value.finality_margin_blocks,
            evidence_horizon_blocks: value.evidence_horizon_blocks,
            activation_lag_blocks: value.activation_lag_blocks,
            slashing_delay_blocks: value.slashing_delay_blocks,
            epoch_length_blocks: value.epoch_length_blocks,
        }
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for SumeragiNposParameters {
    fn json_serialize(&self, out: &mut String) {
        SumeragiNposParametersJson::from(self.clone()).json_serialize(out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for SumeragiNposParameters {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = Self::from(SumeragiNposParametersJson::json_deserialize(parser)?);
        value
            .validate()
            .map_err(|message| json::Error::InvalidField {
                field: "SumeragiNposParameters".to_owned(),
                message: message.to_owned(),
            })?;
        Ok(value)
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for Parameter {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        match self {
            Parameter::Sumeragi(value) => {
                json::write_json_string("Sumeragi", out);
                out.push(':');
                value.json_serialize(out);
            }
            Parameter::Block(value) => {
                json::write_json_string("Block", out);
                out.push(':');
                value.json_serialize(out);
            }
            Parameter::Transaction(value) => {
                json::write_json_string("Transaction", out);
                out.push(':');
                value.json_serialize(out);
            }
            Parameter::SmartContract(value) => {
                json::write_json_string("SmartContract", out);
                out.push(':');
                value.json_serialize(out);
            }
            Parameter::Executor(value) => {
                json::write_json_string("Executor", out);
                out.push(':');
                value.json_serialize(out);
            }
            Parameter::Custom(value) => {
                json::write_json_string("Custom", out);
                out.push(':');
                value.json_serialize(out);
            }
        }
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for Parameter {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let map = json_support::expect_object(value, "Parameter")?;
        let mut iter = map.into_iter();
        let (field, payload) = iter.next().ok_or_else(|| json::Error::InvalidField {
            field: String::from("Parameter"),
            message: String::from("expected single-key object"),
        })?;
        if let Some((extra, _)) = iter.next() {
            return Err(json::Error::UnknownField { field: extra });
        }
        match field.as_str() {
            "Sumeragi" => Ok(Self::Sumeragi(json_support::parse_value_as::<
                SumeragiParameter,
            >(&payload)?)),
            "Block" => Ok(Self::Block(json_support::parse_value_as::<BlockParameter>(
                &payload,
            )?)),
            "Transaction" => Ok(Self::Transaction(json_support::parse_value_as::<
                TransactionParameter,
            >(&payload)?)),
            "SmartContract" => Ok(Self::SmartContract(json_support::parse_value_as::<
                SmartContractParameter,
            >(&payload)?)),
            "Executor" => Ok(Self::Executor(json_support::parse_value_as::<
                SmartContractParameter,
            >(&payload)?)),
            "Custom" => Ok(Self::Custom(
                json_support::parse_value_as::<CustomParameter>(&payload)?,
            )),
            other => Err(json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for SumeragiConsensusMode {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            SumeragiConsensusMode::Permissioned => "Permissioned",
            SumeragiConsensusMode::Npos => "Npos",
        };
        json::write_json_string(label, out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for SumeragiConsensusMode {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "Permissioned" => Ok(SumeragiConsensusMode::Permissioned),
            "Npos" => Ok(SumeragiConsensusMode::Npos),
            other => Err(json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}

// (Codecs are provided by derives and candidate decoders. Tests use wrappers where needed.)

impl SumeragiParameters {
    /// Signed, immutable block cadence in milliseconds.
    #[must_use]
    pub fn block_cadence_ms(&self) -> NonZeroU64 {
        self.block_cadence_ms
    }

    /// Raw clock drift bound in milliseconds.
    #[must_use]
    pub fn max_clock_drift_ms(&self) -> u64 {
        self.max_clock_drift_ms
    }

    /// Maximal allowed random deviation from the nominal rate
    ///
    /// # Warning
    ///
    /// This value should be kept as low as possible to not affect soundness of the consensus
    pub fn max_clock_drift(&self) -> Duration {
        Duration::from_millis(self.max_clock_drift_ms)
    }

    /// Signed interval between block-production opportunities.
    ///
    /// A block is created if this limit or [`BlockParameters::max_transactions`] limit is reached,
    /// whichever comes first. Regardless of the limits, an empty block is never created.
    /// This value is authoritative for both permissioned and `NPoS` pacemaker timing.
    pub fn block_cadence(&self) -> Duration {
        Duration::from_millis(self.block_cadence_ms.get())
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for SumeragiParameter {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        match self {
            SumeragiParameter::MaxClockDriftMs(v) => {
                json::write_json_string("MaxClockDriftMs", out);
                out.push(':');
                v.json_serialize(out);
            }
        }
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for SumeragiParameter {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let map = json_support::expect_object(value, "SumeragiParameter")?;
        let mut iter = map.into_iter();
        let (field, payload) = iter.next().ok_or_else(|| json::Error::InvalidField {
            field: String::from("SumeragiParameter"),
            message: String::from("expected single-key object"),
        })?;
        if let Some((extra, _)) = iter.next() {
            return Err(json::Error::UnknownField { field: extra });
        }
        match field.as_str() {
            "MaxClockDriftMs" => Ok(Self::MaxClockDriftMs(json_support::expect_u64(
                &payload,
                "MaxClockDriftMs",
            )?)),
            other => Err(json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for SumeragiParameters {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        json_support::write_field(out, &mut first, "block_cadence_ms", &self.block_cadence_ms);
        json_support::write_field(
            out,
            &mut first,
            "max_clock_drift_ms",
            &self.max_clock_drift_ms,
        );
        json_support::write_field(
            out,
            &mut first,
            "key_activation_lead_blocks",
            &self.key_activation_lead_blocks,
        );
        json_support::write_field(
            out,
            &mut first,
            "key_overlap_grace_blocks",
            &self.key_overlap_grace_blocks,
        );
        json_support::write_field(
            out,
            &mut first,
            "key_expiry_grace_blocks",
            &self.key_expiry_grace_blocks,
        );
        json_support::write_field(out, &mut first, "key_require_hsm", &self.key_require_hsm);
        json_support::write_field(
            out,
            &mut first,
            "key_allowed_algorithms",
            &self.key_allowed_algorithms,
        );
        json_support::write_field(
            out,
            &mut first,
            "key_allowed_hsm_providers",
            &self.key_allowed_hsm_providers,
        );
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for SumeragiParameters {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let mut map = json_support::expect_object(value, "SumeragiParameters")?;

        let block_cadence_ms = map
            .remove("block_cadence_ms")
            .map(|value| json_support::expect_nonzero_u64(&value, "block_cadence_ms"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::block_cadence_ms);
        let max_clock_drift_ms = map
            .remove("max_clock_drift_ms")
            .map(|value| json_support::expect_u64(&value, "max_clock_drift_ms"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::max_clock_drift_ms);
        let key_activation_lead_blocks = map
            .remove("key_activation_lead_blocks")
            .map(|value| json_support::expect_u64(&value, "key_activation_lead_blocks"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::key_activation_lead_blocks);
        let key_overlap_grace_blocks = map
            .remove("key_overlap_grace_blocks")
            .map(|value| json_support::expect_u64(&value, "key_overlap_grace_blocks"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::key_overlap_grace_blocks);
        let key_expiry_grace_blocks = map
            .remove("key_expiry_grace_blocks")
            .map(|value| json_support::expect_u64(&value, "key_expiry_grace_blocks"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::key_expiry_grace_blocks);
        let key_require_hsm = map
            .remove("key_require_hsm")
            .map(|value| json_support::expect_bool(&value, "key_require_hsm"))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::key_require_hsm);
        let key_allowed_algorithms = map
            .remove("key_allowed_algorithms")
            .map(|value| json_support::parse_value_as::<Vec<Algorithm>>(&value))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::key_allowed_algorithms);
        let key_allowed_hsm_providers = map
            .remove("key_allowed_hsm_providers")
            .map(|value| json_support::parse_value_as::<Vec<String>>(&value))
            .transpose()?
            .unwrap_or_else(defaults::sumeragi::key_allowed_hsm_providers);

        json_support::ensure_no_extra(map)?;

        let params = Self {
            block_cadence_ms,
            max_clock_drift_ms,
            key_activation_lead_blocks,
            key_overlap_grace_blocks,
            key_expiry_grace_blocks,
            key_require_hsm,
            key_allowed_algorithms,
            key_allowed_hsm_providers,
        };

        Ok(params)
    }
}

mod defaults {
    pub mod sumeragi {
        use core::num::NonZeroU64;

        use iroha_crypto::Algorithm;

        pub const fn block_cadence_ms() -> NonZeroU64 {
            nonzero_ext::nonzero!(100_u64)
        }
        pub const fn max_clock_drift_ms() -> u64 {
            1_000
        }
        pub const fn key_activation_lead_blocks() -> u64 {
            1
        }
        pub const fn key_overlap_grace_blocks() -> u64 {
            8
        }
        pub const fn key_expiry_grace_blocks() -> u64 {
            0
        }
        pub const fn key_require_hsm() -> bool {
            false
        }
        pub fn key_allowed_algorithms() -> Vec<Algorithm> {
            vec![Algorithm::BlsNormal]
        }
        pub fn key_allowed_hsm_providers() -> Vec<String> {
            vec!["pkcs11".into(), "softkey".into(), "yubihsm".into()]
        }

        pub mod npos {
            use core::num::NonZeroU64;
            use iroha_primitives::numeric::Quantity;

            pub const fn vrf_commit_window_blocks() -> u64 {
                100
            }
            pub const fn vrf_reveal_window_blocks() -> u64 {
                40
            }
            pub const fn max_validators() -> u32 {
                128
            }
            pub fn min_self_bond() -> Quantity {
                Quantity::from(1_000_u64)
            }
            pub fn min_nomination_bond() -> Quantity {
                Quantity::one()
            }
            pub const fn max_nominator_concentration_pct() -> u8 {
                25
            }
            pub const fn seat_band_pct() -> u8 {
                5
            }
            pub const fn max_entity_correlation_pct() -> u8 {
                25
            }
            pub const fn finality_margin_blocks() -> u64 {
                8
            }
            pub const fn evidence_horizon_blocks() -> u64 {
                7_200
            }
            pub const fn activation_lag_blocks() -> u64 {
                1
            }
            pub const fn slashing_delay_blocks() -> u64 {
                259_200
            }
            pub const fn epoch_length_blocks() -> NonZeroU64 {
                nonzero_ext::nonzero!(3_600_u64)
            }
        }
    }

    pub mod block {
        use core::num::NonZeroU64;

        use nonzero_ext::nonzero;

        /// Default value for [`Parameters::MaxTransactionsInBlock`]
        pub const fn max_transactions() -> NonZeroU64 {
            nonzero!(2_u64.pow(9))
        }
    }

    pub mod transaction {
        use core::num::{NonZeroU16, NonZeroU64};

        use nonzero_ext::nonzero;

        pub const fn max_signatures() -> NonZeroU64 {
            nonzero!(16_u64)
        }
        pub const fn max_instructions() -> NonZeroU64 {
            // Keep in sync with iroha_config defaults::transaction::max_instructions.
            nonzero!(100_000_u64)
        }
        pub const fn ivm_bytecode_size() -> NonZeroU64 {
            nonzero!(4 * 2_u64.pow(20))
        }
        pub const fn max_tx_bytes() -> NonZeroU64 {
            // Default total transaction wire-size cap set to 10 MiB.
            nonzero!(10_485_760_u64)
        }
        pub const fn max_decompressed_bytes() -> NonZeroU64 {
            // Default post-decompression budget aligns with max_tx_bytes until
            // additional compression formats are supported.
            max_tx_bytes()
        }
        pub const fn max_metadata_depth() -> NonZeroU16 {
            // Default metadata nesting limit: 1 (metadata map) + up to 7 nested structures.
            nonzero!(8_u16)
        }
    }

    pub mod smart_contract {
        use core::num::NonZeroU64;

        use nonzero_ext::nonzero;

        pub const fn fuel() -> NonZeroU64 {
            nonzero!(55_000_000_u64)
        }
        pub const fn memory() -> NonZeroU64 {
            nonzero!(55_000_000_u64)
        }
        pub const fn execution_depth() -> u8 {
            3
        }
    }
}

impl Default for SumeragiParameters {
    fn default() -> Self {
        use defaults::sumeragi::*;
        Self {
            block_cadence_ms: block_cadence_ms(),
            max_clock_drift_ms: max_clock_drift_ms(),
            key_activation_lead_blocks: key_activation_lead_blocks(),
            key_overlap_grace_blocks: key_overlap_grace_blocks(),
            key_expiry_grace_blocks: key_expiry_grace_blocks(),
            key_require_hsm: key_require_hsm(),
            key_allowed_algorithms: key_allowed_algorithms(),
            key_allowed_hsm_providers: key_allowed_hsm_providers(),
        }
    }
}
impl Default for BlockParameters {
    fn default() -> Self {
        Self::new(defaults::block::max_transactions())
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for BlockParameters {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        json_support::write_field(out, &mut first, "max_transactions", &self.max_transactions);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for BlockParameters {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let mut map = json_support::expect_object(value, "BlockParameters")?;
        let max_transactions = map
            .remove("max_transactions")
            .map(|value| json_support::expect_nonzero_u64(&value, "max_transactions"))
            .transpose()?
            .unwrap_or_else(defaults::block::max_transactions);
        json_support::ensure_no_extra(map)?;
        Ok(Self::new(max_transactions))
    }
}

impl Default for TransactionParameters {
    fn default() -> Self {
        use defaults::transaction::*;
        Self::with_max_signatures(
            max_signatures(),
            max_instructions(),
            ivm_bytecode_size(),
            max_tx_bytes(),
            max_decompressed_bytes(),
            max_metadata_depth(),
        )
    }
}

impl Default for SmartContractParameters {
    fn default() -> Self {
        use defaults::smart_contract::*;
        Self {
            fuel: fuel(),
            memory: memory(),
            execution_depth: execution_depth(),
        }
    }
}

impl FromIterator<Parameter> for Parameters {
    fn from_iter<T: IntoIterator<Item = Parameter>>(iter: T) -> Self {
        iter.into_iter().fold(Parameters::default(), |mut acc, x| {
            acc.set_parameter(x);
            acc
        })
    }
}

impl Parameters {
    /// Convert [`Self`] into iterator of individual parameters
    pub fn parameters(&self) -> impl Iterator<Item = Parameter> + '_ {
        self.sumeragi
            .parameters()
            .map(Parameter::Sumeragi)
            .chain(self.block.parameters().map(Parameter::Block))
            .chain(self.transaction.parameters().map(Parameter::Transaction))
            .chain(self.executor.parameters().map(Parameter::Executor))
            .chain(
                self.smart_contract
                    .parameters()
                    .map(Parameter::SmartContract),
            )
            .chain(self.custom.values().cloned().map(Parameter::Custom))
    }

    /// Set `parameter` value to corresponding parameter in `self`
    pub fn set_parameter(&mut self, parameter: Parameter) {
        macro_rules! apply_parameter {
            ($($container:ident($param:ident.$field:ident) => $single:ident::$variant:ident),* $(,)?) => {
                match parameter {
                    $(
                    Parameter::$container($single::$variant(next)) => {
                        self.$param.$field = next;
                    }
                    )*
                    Parameter::Custom(next) => {
                        self.custom.insert(next.id.clone(), next);
                    }
                }
            };
        }

        apply_parameter!(
            Sumeragi(sumeragi.max_clock_drift_ms) => SumeragiParameter::MaxClockDriftMs,

            Block(block.max_transactions) => BlockParameter::MaxTransactions,

            Transaction(transaction.max_signatures) => TransactionParameter::MaxSignatures,
            Transaction(transaction.max_instructions) => TransactionParameter::MaxInstructions,
            Transaction(transaction.ivm_bytecode_size) => TransactionParameter::IvmBytecodeSize,
            Transaction(transaction.max_tx_bytes) => TransactionParameter::MaxTxBytes,
            Transaction(transaction.max_decompressed_bytes) => TransactionParameter::MaxDecompressedBytes,
            Transaction(transaction.max_metadata_depth) => TransactionParameter::MaxMetadataDepth,
            Transaction(transaction.require_height_ttl) => TransactionParameter::RequireHeightTtl,
            Transaction(transaction.require_sequence) => TransactionParameter::RequireSequence,

            SmartContract(smart_contract.fuel) => SmartContractParameter::Fuel,
            SmartContract(smart_contract.memory) => SmartContractParameter::Memory,
            SmartContract(smart_contract.execution_depth) => SmartContractParameter::ExecutionDepth,

            Executor(executor.fuel) => SmartContractParameter::Fuel,
            Executor(executor.memory) => SmartContractParameter::Memory,
            Executor(executor.execution_depth) => SmartContractParameter::ExecutionDepth,
        );
    }
}

/// Consensus handshake metadata helpers used during genesis provisioning.
pub mod consensus_metadata {
    use core::str::FromStr as _;

    use super::*;
    use crate::Name;

    static HANDSHAKE_META_ID: LazyLock<CustomParameterId> = LazyLock::new(|| {
        CustomParameterId::new(
            Name::from_str("consensus_handshake_meta")
                .expect("consensus_handshake_meta is a valid Name"),
        )
    });

    /// Identifier of the custom parameter embedding consensus handshake metadata.
    pub fn handshake_meta_id() -> CustomParameterId {
        HANDSHAKE_META_ID.clone()
    }
}

/// Cryptography snapshot metadata helpers used during genesis provisioning.
pub mod crypto_metadata {
    use core::str::FromStr as _;

    use super::*;
    use crate::Name;

    static MANIFEST_META_ID: LazyLock<CustomParameterId> = LazyLock::new(|| {
        CustomParameterId::new(
            Name::from_str("crypto_manifest_meta").expect("crypto_manifest_meta is a valid Name"),
        )
    });

    /// Identifier of the custom parameter anchoring the genesis crypto snapshot.
    pub fn manifest_meta_id() -> CustomParameterId {
        MANIFEST_META_ID.clone()
    }
}

/// Confidential registry metadata helpers used during genesis provisioning.
pub mod confidential_metadata {
    use core::str::FromStr as _;

    use super::*;
    use crate::Name;

    static REGISTRY_ROOT_ID: LazyLock<CustomParameterId> = LazyLock::new(|| {
        CustomParameterId::new(
            Name::from_str("confidential_registry_root")
                .expect("confidential_registry_root is a valid Name"),
        )
    });

    /// Identifier of the custom parameter anchoring the confidential registry fingerprint.
    pub fn registry_root_id() -> CustomParameterId {
        REGISTRY_ROOT_ID.clone()
    }
}

/// IVM metadata helpers stored in the custom parameter registry.
pub mod ivm_metadata {
    use core::str::FromStr as _;

    use super::*;
    use crate::Name;

    static PUBLIC_INPUTS_ID: LazyLock<CustomParameterId> = LazyLock::new(|| {
        CustomParameterId::new(
            Name::from_str("ivm_public_inputs").expect("ivm_public_inputs is a valid Name"),
        )
    });

    /// Identifier of the custom parameter embedding the IVM public input registry.
    pub fn public_inputs_id() -> CustomParameterId {
        PUBLIC_INPUTS_ID.clone()
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for Parameters {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        json_support::write_field(out, &mut first, "sumeragi", &self.sumeragi);
        json_support::write_field(out, &mut first, "block", &self.block);
        json_support::write_field(out, &mut first, "transaction", &self.transaction);
        json_support::write_field(out, &mut first, "executor", &self.executor);
        json_support::write_field(out, &mut first, "smart_contract", &self.smart_contract);
        if !self.custom.is_empty() {
            json_support::write_field(out, &mut first, "custom", &self.custom);
        }
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for Parameters {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let mut map = json_support::expect_object(value, "Parameters")?;
        let sumeragi = map
            .remove("sumeragi")
            .map(|value| json_support::parse_value_as::<SumeragiParameters>(&value))
            .transpose()?
            .unwrap_or_default();
        let block = map
            .remove("block")
            .map(|value| json_support::parse_value_as::<BlockParameters>(&value))
            .transpose()?
            .unwrap_or_default();
        let transaction = map
            .remove("transaction")
            .map(|value| json_support::parse_value_as::<TransactionParameters>(&value))
            .transpose()?
            .unwrap_or_default();
        let executor = map
            .remove("executor")
            .map(|value| json_support::parse_value_as::<SmartContractParameters>(&value))
            .transpose()?
            .unwrap_or_default();
        let smart_contract = map
            .remove("smart_contract")
            .map(|value| json_support::parse_value_as::<SmartContractParameters>(&value))
            .transpose()?
            .unwrap_or_default();
        let custom = map
            .remove("custom")
            .map(parse_custom_parameters_value)
            .transpose()?
            .unwrap_or_default();
        json_support::ensure_no_extra(map)?;
        Ok(Self {
            sumeragi,
            block,
            transaction,
            executor,
            smart_contract,
            custom,
        })
    }
}

impl SumeragiParameters {
    /// Construct [`Self`]
    pub fn new(block_cadence: Duration, max_clock_drift: Duration) -> Self {
        let block_cadence_ms = block_cadence
            .as_millis()
            .try_into()
            .expect("INTERNAL BUG: Time should fit into u64");
        Self {
            block_cadence_ms: NonZeroU64::new(block_cadence_ms)
                .expect("INTERNAL BUG: block cadence must be non-zero"),
            max_clock_drift_ms: max_clock_drift
                .as_millis()
                .try_into()
                .expect("INTERNAL BUG: Time should fit into u64"),
            key_activation_lead_blocks: defaults::sumeragi::key_activation_lead_blocks(),
            key_overlap_grace_blocks: defaults::sumeragi::key_overlap_grace_blocks(),
            key_expiry_grace_blocks: defaults::sumeragi::key_expiry_grace_blocks(),
            key_require_hsm: defaults::sumeragi::key_require_hsm(),
            key_allowed_algorithms: defaults::sumeragi::key_allowed_algorithms(),
            key_allowed_hsm_providers: defaults::sumeragi::key_allowed_hsm_providers(),
        }
    }

    /// Convert [`Self`] into iterator of individual parameters
    pub fn parameters(&self) -> impl Iterator<Item = SumeragiParameter> {
        [SumeragiParameter::MaxClockDriftMs(self.max_clock_drift_ms)].into_iter()
    }
}

impl BlockParameters {
    /// Construct [`Self`]
    pub const fn new(max_transactions: NonZeroU64) -> Self {
        Self { max_transactions }
    }

    /// Convert [`Self`] into iterator of individual parameters
    pub fn parameters(&self) -> impl Iterator<Item = BlockParameter> {
        [BlockParameter::MaxTransactions(self.max_transactions)].into_iter()
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for BlockParameter {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        match self {
            BlockParameter::MaxTransactions(value) => {
                json::write_json_string("MaxTransactions", out);
                out.push(':');
                value.json_serialize(out);
            }
        }
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for BlockParameter {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let map = json_support::expect_object(value, "BlockParameter")?;
        let mut iter = map.into_iter();
        let (field, payload) = iter.next().ok_or_else(|| json::Error::InvalidField {
            field: String::from("BlockParameter"),
            message: String::from("expected single-key object"),
        })?;
        if let Some((extra, _)) = iter.next() {
            return Err(json::Error::UnknownField { field: extra });
        }
        match field.as_str() {
            "MaxTransactions" => Ok(Self::MaxTransactions(json_support::expect_nonzero_u64(
                &payload,
                "MaxTransactions",
            )?)),
            other => Err(json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}

impl TransactionParameters {
    /// Construct [`Self`] with an explicit maximum signature count.
    pub const fn with_max_signatures(
        max_signatures: NonZeroU64,
        max_instructions: NonZeroU64,
        ivm_bytecode_size: NonZeroU64,
        max_tx_bytes: NonZeroU64,
        max_decompressed_bytes: NonZeroU64,
        max_metadata_depth: NonZeroU16,
    ) -> Self {
        Self {
            max_signatures,
            max_instructions,
            ivm_bytecode_size,
            max_tx_bytes,
            max_decompressed_bytes,
            max_metadata_depth,
            require_height_ttl: false,
            require_sequence: false,
        }
    }

    /// Configure ingress metadata enforcement (height-based TTL and per-sender sequence checks).
    #[must_use]
    pub const fn with_ingress_enforcement(
        mut self,
        require_height_ttl: bool,
        require_sequence: bool,
    ) -> Self {
        self.require_height_ttl = require_height_ttl;
        self.require_sequence = require_sequence;
        self
    }

    /// Construct [`Self`] using the default signature limit.
    pub const fn new(
        max_instructions: NonZeroU64,
        ivm_bytecode_size: NonZeroU64,
        max_tx_bytes: NonZeroU64,
        max_decompressed_bytes: NonZeroU64,
        max_metadata_depth: NonZeroU16,
    ) -> Self {
        Self::with_max_signatures(
            defaults::transaction::max_signatures(),
            max_instructions,
            ivm_bytecode_size,
            max_tx_bytes,
            max_decompressed_bytes,
            max_metadata_depth,
        )
    }

    /// Convert [`Self`] into iterator of individual parameters
    pub fn parameters(&self) -> impl Iterator<Item = TransactionParameter> {
        [
            TransactionParameter::MaxSignatures(self.max_signatures),
            TransactionParameter::MaxInstructions(self.max_instructions),
            TransactionParameter::IvmBytecodeSize(self.ivm_bytecode_size),
            TransactionParameter::MaxTxBytes(self.max_tx_bytes),
            TransactionParameter::MaxDecompressedBytes(self.max_decompressed_bytes),
            TransactionParameter::MaxMetadataDepth(self.max_metadata_depth),
            TransactionParameter::RequireHeightTtl(self.require_height_ttl),
            TransactionParameter::RequireSequence(self.require_sequence),
        ]
        .into_iter()
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for TransactionParameters {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        json_support::write_field(out, &mut first, "max_signatures", &self.max_signatures);
        json_support::write_field(out, &mut first, "max_instructions", &self.max_instructions);
        json_support::write_field(
            out,
            &mut first,
            "ivm_bytecode_size",
            &self.ivm_bytecode_size,
        );
        json_support::write_field(out, &mut first, "max_tx_bytes", &self.max_tx_bytes);
        json_support::write_field(
            out,
            &mut first,
            "max_decompressed_bytes",
            &self.max_decompressed_bytes,
        );
        json_support::write_field(
            out,
            &mut first,
            "max_metadata_depth",
            &self.max_metadata_depth,
        );
        json_support::write_field(
            out,
            &mut first,
            "require_height_ttl",
            &self.require_height_ttl,
        );
        json_support::write_field(out, &mut first, "require_sequence", &self.require_sequence);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for TransactionParameters {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let mut map = json_support::expect_object(value, "TransactionParameters")?;
        let max_signatures = map
            .remove("max_signatures")
            .map(|value| json_support::expect_nonzero_u64(&value, "max_signatures"))
            .transpose()?
            .unwrap_or_else(defaults::transaction::max_signatures);
        let max_instructions = map
            .remove("max_instructions")
            .map(|value| json_support::expect_nonzero_u64(&value, "max_instructions"))
            .transpose()?
            .unwrap_or_else(defaults::transaction::max_instructions);
        let ivm_bytecode_size = map
            .remove("ivm_bytecode_size")
            .map(|value| json_support::expect_nonzero_u64(&value, "ivm_bytecode_size"))
            .transpose()?
            .unwrap_or_else(defaults::transaction::ivm_bytecode_size);
        let max_tx_bytes = map
            .remove("max_tx_bytes")
            .map(|value| json_support::expect_nonzero_u64(&value, "max_tx_bytes"))
            .transpose()?
            .unwrap_or_else(defaults::transaction::max_tx_bytes);
        let max_decompressed_bytes = map
            .remove("max_decompressed_bytes")
            .map(|value| json_support::expect_nonzero_u64(&value, "max_decompressed_bytes"))
            .transpose()?
            .unwrap_or_else(defaults::transaction::max_decompressed_bytes);
        let require_height_ttl = map
            .remove("require_height_ttl")
            .map(|value| json_support::expect_bool(&value, "require_height_ttl"))
            .transpose()?
            .unwrap_or(false);
        let require_sequence = map
            .remove("require_sequence")
            .map(|value| json_support::expect_bool(&value, "require_sequence"))
            .transpose()?
            .unwrap_or(false);
        let max_metadata_depth = map
            .remove("max_metadata_depth")
            .map(|value| {
                let depth = json_support::expect_nonzero_u64(&value, "max_metadata_depth")?;
                let depth_u16 =
                    u16::try_from(depth.get()).map_err(|_| json::Error::InvalidField {
                        field: String::from("max_metadata_depth"),
                        message: String::from("value exceeds u16::MAX"),
                    })?;
                NonZeroU16::new(depth_u16).ok_or_else(|| json::Error::InvalidField {
                    field: String::from("max_metadata_depth"),
                    message: String::from("value must be non-zero"),
                })
            })
            .transpose()?
            .unwrap_or_else(defaults::transaction::max_metadata_depth);
        json_support::ensure_no_extra(map)?;
        Ok(Self::with_max_signatures(
            max_signatures,
            max_instructions,
            ivm_bytecode_size,
            max_tx_bytes,
            max_decompressed_bytes,
            max_metadata_depth,
        )
        .with_ingress_enforcement(require_height_ttl, require_sequence))
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for TransactionParameter {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        match self {
            TransactionParameter::MaxSignatures(value) => {
                json::write_json_string("MaxSignatures", out);
                out.push(':');
                value.json_serialize(out);
            }
            TransactionParameter::MaxInstructions(value) => {
                json::write_json_string("MaxInstructions", out);
                out.push(':');
                value.json_serialize(out);
            }
            TransactionParameter::IvmBytecodeSize(value) => {
                json::write_json_string("IvmBytecodeSize", out);
                out.push(':');
                value.json_serialize(out);
            }
            TransactionParameter::MaxTxBytes(value) => {
                json::write_json_string("MaxTxBytes", out);
                out.push(':');
                value.json_serialize(out);
            }
            TransactionParameter::MaxDecompressedBytes(value) => {
                json::write_json_string("MaxDecompressedBytes", out);
                out.push(':');
                value.json_serialize(out);
            }
            TransactionParameter::MaxMetadataDepth(value) => {
                json::write_json_string("MaxMetadataDepth", out);
                out.push(':');
                value.json_serialize(out);
            }
            TransactionParameter::RequireHeightTtl(value) => {
                json::write_json_string("RequireHeightTtl", out);
                out.push(':');
                value.json_serialize(out);
            }
            TransactionParameter::RequireSequence(value) => {
                json::write_json_string("RequireSequence", out);
                out.push(':');
                value.json_serialize(out);
            }
        }
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for TransactionParameter {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let map = json_support::expect_object(value, "TransactionParameter")?;
        let mut iter = map.into_iter();
        let (field, payload) = iter.next().ok_or_else(|| json::Error::InvalidField {
            field: String::from("TransactionParameter"),
            message: String::from("expected single-key object"),
        })?;
        if let Some((extra, _)) = iter.next() {
            return Err(json::Error::UnknownField { field: extra });
        }
        match field.as_str() {
            "MaxSignatures" => Ok(Self::MaxSignatures(json_support::expect_nonzero_u64(
                &payload,
                "MaxSignatures",
            )?)),
            "MaxInstructions" => Ok(Self::MaxInstructions(json_support::expect_nonzero_u64(
                &payload,
                "MaxInstructions",
            )?)),
            "IvmBytecodeSize" => Ok(Self::IvmBytecodeSize(json_support::expect_nonzero_u64(
                &payload,
                "IvmBytecodeSize",
            )?)),
            "MaxTxBytes" => Ok(Self::MaxTxBytes(json_support::expect_nonzero_u64(
                &payload,
                "MaxTxBytes",
            )?)),
            "MaxDecompressedBytes" => Ok(Self::MaxDecompressedBytes(
                json_support::expect_nonzero_u64(&payload, "MaxDecompressedBytes")?,
            )),
            "MaxMetadataDepth" => Ok(Self::MaxMetadataDepth({
                let depth = json_support::expect_nonzero_u64(&payload, "MaxMetadataDepth")?;
                let value = u16::try_from(depth.get()).map_err(|_| json::Error::InvalidField {
                    field: String::from("MaxMetadataDepth"),
                    message: String::from("value exceeds u16::MAX"),
                })?;
                NonZeroU16::new(value).ok_or_else(|| json::Error::InvalidField {
                    field: String::from("MaxMetadataDepth"),
                    message: String::from("value must be non-zero"),
                })?
            })),
            "RequireHeightTtl" => Ok(Self::RequireHeightTtl(json_support::expect_bool(
                &payload,
                "RequireHeightTtl",
            )?)),
            "RequireSequence" => Ok(Self::RequireSequence(json_support::expect_bool(
                &payload,
                "RequireSequence",
            )?)),
            other => Err(json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}

impl SmartContractParameters {
    /// Convert [`Self`] into iterator of individual parameters
    pub fn parameters(&self) -> impl Iterator<Item = SmartContractParameter> {
        [
            SmartContractParameter::Fuel(self.fuel),
            SmartContractParameter::Memory(self.memory),
            SmartContractParameter::ExecutionDepth(self.execution_depth),
        ]
        .into_iter()
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for SmartContractParameters {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        json_support::write_field(out, &mut first, "fuel", &self.fuel);
        json_support::write_field(out, &mut first, "memory", &self.memory);
        json_support::write_field(out, &mut first, "execution_depth", &self.execution_depth);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for SmartContractParameters {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let mut map = json_support::expect_object(value, "SmartContractParameters")?;
        let fuel = map
            .remove("fuel")
            .map(|value| json_support::expect_nonzero_u64(&value, "fuel"))
            .transpose()?
            .unwrap_or_else(defaults::smart_contract::fuel);
        let memory = map
            .remove("memory")
            .map(|value| json_support::expect_nonzero_u64(&value, "memory"))
            .transpose()?
            .unwrap_or_else(defaults::smart_contract::memory);
        let execution_depth = map
            .remove("execution_depth")
            .map(|value| json_support::expect_u8(&value, "execution_depth"))
            .transpose()?
            .unwrap_or_else(defaults::smart_contract::execution_depth);
        json_support::ensure_no_extra(map)?;
        Ok(Self {
            fuel,
            memory,
            execution_depth,
        })
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for SmartContractParameter {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        match self {
            SmartContractParameter::Fuel(value) => {
                json::write_json_string("Fuel", out);
                out.push(':');
                value.json_serialize(out);
            }
            SmartContractParameter::Memory(value) => {
                json::write_json_string("Memory", out);
                out.push(':');
                value.json_serialize(out);
            }
            SmartContractParameter::ExecutionDepth(value) => {
                json::write_json_string("ExecutionDepth", out);
                out.push(':');
                value.json_serialize(out);
            }
        }
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for SmartContractParameter {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let map = json_support::expect_object(value, "SmartContractParameter")?;
        let mut iter = map.into_iter();
        let (field, payload) = iter.next().ok_or_else(|| json::Error::InvalidField {
            field: String::from("SmartContractParameter"),
            message: String::from("expected single-key object"),
        })?;
        if let Some((extra, _)) = iter.next() {
            return Err(json::Error::UnknownField { field: extra });
        }
        match field.as_str() {
            "Fuel" => Ok(Self::Fuel(json_support::expect_nonzero_u64(
                &payload, "Fuel",
            )?)),
            "Memory" => Ok(Self::Memory(json_support::expect_nonzero_u64(
                &payload, "Memory",
            )?)),
            "ExecutionDepth" => Ok(Self::ExecutionDepth(json_support::expect_u8(
                &payload,
                "ExecutionDepth",
            )?)),
            other => Err(json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}

// Norito decoding now relies on derived implementations for parameter types.

#[cfg(test)]
mod tests {
    use core::str::FromStr as _;

    use iroha_primitives::json::Json;
    use norito::codec::{DecodeAll as _, Encode as _};
    use norito::json::{Number, Value};

    use super::*;
    use crate::{
        name::Name,
        parameter::custom::{CustomParameter, CustomParameterId},
    };

    #[test]
    fn set_custom_parameter() {
        let mut params = Parameters::default();
        let id = CustomParameterId::new(Name::from_str("test").expect("valid"));
        let payload = Json::new(1u32);
        let custom = CustomParameter::new(id.clone(), payload);
        params.set_parameter(Parameter::Custom(custom.clone()));
        assert_eq!(params.custom().get(&id), Some(&custom));
    }

    #[test]
    fn set_block_parameter() {
        let mut params = Parameters::default();
        let max_transactions = NonZeroU64::new(10).unwrap();
        params.set_parameter(Parameter::Block(BlockParameter::MaxTransactions(
            max_transactions,
        )));
        assert_eq!(params.block().max_transactions(), max_transactions);
    }

    #[test]
    fn sumeragi_parameter_norito_roundtrip() {
        let value = SumeragiParameter::MaxClockDriftMs(1_000);
        let bytes = value.encode();
        let decoded = SumeragiParameter::decode_all(&mut bytes.as_slice())
            .expect("decode Sumeragi parameter");
        assert_eq!(value, decoded);
    }

    #[test]
    fn sumeragi_parameters_norito_roundtrip() {
        let params = SumeragiParameters::default();
        // Encode as a stable tuple and reconstruct (append-only order)
        let bytes = params.encode();
        let mut cursor = bytes.as_slice();
        let dec = SumeragiParameters::decode_all(&mut cursor).expect("decode sumeragi parameters");
        assert_eq!(params, dec);

        // Skip bare codec path; headerful Norito is the canonical test path

        // JSON path: ensure defaults parse and roundtrip
        #[cfg(feature = "json")]
        {
            let j = norito::json::to_json(&params).expect("json ser");
            let parsed: SumeragiParameters = norito::json::from_str(&j).expect("json de");
            assert_eq!(params, parsed);
        }
    }

    #[cfg(feature = "json")]
    #[test]
    fn sumeragi_parameters_json_rejects_zero_cadence() {
        let json = r#"{"block_cadence_ms":0}"#;
        norito::json::from_str::<SumeragiParameters>(json).expect_err("zero cadence must fail");
    }

    #[cfg(feature = "json")]
    #[test]
    fn sumeragi_parameters_json_rejects_every_retired_field() {
        for field in [
            "block_time_ms",
            "commit_time_ms",
            "min_finality_ms",
            "pacing_factor_bps",
            "collectors_k",
            "collectors_redundant_send_r",
            "da_enabled",
            "next_mode",
            "mode_activation_height",
        ] {
            let json = format!(r#"{{"block_cadence_ms":100,"{field}":1}}"#);
            assert!(
                norito::json::from_str::<SumeragiParameters>(&json).is_err(),
                "retired field `{field}` must fail"
            );
        }
    }

    #[test]
    fn sumeragi_npos_from_custom_parameter_rejects_string_wrapped_payload() {
        let expected = SumeragiNposParameters::default();
        let wrapped = Json::new(norito::json::to_json(&expected).expect("serialize npos payload"));
        let custom = CustomParameter::new(SumeragiNposParameters::parameter_id(), wrapped);

        assert!(
            SumeragiNposParameters::from_custom_parameter(&custom).is_none(),
            "string-wrapped compatibility payload must be rejected"
        );
    }

    #[test]
    fn sumeragi_npos_from_custom_parameter_rejects_numeric_string_payload() {
        let expected = SumeragiNposParameters::default();
        let mut payload =
            norito::json::to_value(&expected).expect("serialize npos payload to json value");
        let map = payload
            .as_object_mut()
            .expect("npos payload should serialize as object");
        for field in [
            "vrf_commit_window_blocks",
            "vrf_reveal_window_blocks",
            "max_validators",
            "min_self_bond",
            "min_nomination_bond",
            "max_nominator_concentration_pct",
            "seat_band_pct",
            "max_entity_correlation_pct",
            "finality_margin_blocks",
            "evidence_horizon_blocks",
            "activation_lag_blocks",
            "slashing_delay_blocks",
            "epoch_length_blocks",
        ] {
            let value = map
                .get_mut(field)
                .unwrap_or_else(|| panic!("missing `{field}` in npos payload"));
            let number = value
                .as_u64()
                .unwrap_or_else(|| panic!("`{field}` should serialize as number"));
            *value = norito::json::Value::String(number.to_string());
        }
        let custom = CustomParameter::new(
            SumeragiNposParameters::parameter_id(),
            Json::from_norito_value_ref(&payload).expect("serialize compatibility payload"),
        );

        assert!(
            SumeragiNposParameters::from_custom_parameter(&custom).is_none(),
            "numeric-string compatibility payload must be rejected"
        );
    }

    #[test]
    fn sumeragi_npos_from_custom_parameter_accepts_epoch_seed_hex_string_payload() {
        let expected = SumeragiNposParameters::default();
        let mut payload =
            norito::json::to_value(&expected).expect("serialize npos payload to json value");
        let map = payload
            .as_object_mut()
            .expect("npos payload should serialize as object");
        let mut hex_seed = String::with_capacity(64);
        for byte in expected.epoch_seed() {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            hex_seed.push(HEX[(byte >> 4) as usize] as char);
            hex_seed.push(HEX[(byte & 0x0F) as usize] as char);
        }
        map.insert(
            "epoch_seed".to_owned(),
            norito::json::Value::String(hex_seed),
        );
        let custom = CustomParameter::new(
            SumeragiNposParameters::parameter_id(),
            Json::from_norito_value_ref(&payload).expect("serialize compatibility payload"),
        );
        let decoded = SumeragiNposParameters::from_custom_parameter(&custom)
            .expect("decode hex epoch seed payload");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn sumeragi_npos_from_custom_parameter_rejects_epoch_seed_with_nested_quotes() {
        let expected = SumeragiNposParameters::default();
        let mut hex_seed = String::with_capacity(64);
        for byte in expected.epoch_seed() {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            hex_seed.push(HEX[(byte >> 4) as usize] as char);
            hex_seed.push(HEX[(byte & 0x0F) as usize] as char);
        }
        let mut payload =
            norito::json::to_value(&expected).expect("serialize npos payload to json value");
        let map = payload
            .as_object_mut()
            .expect("npos payload should serialize as object");
        map.insert(
            "epoch_seed".to_owned(),
            norito::json::Value::String(format!("\"{hex_seed}\"")),
        );

        let custom = CustomParameter::new(
            SumeragiNposParameters::parameter_id(),
            Json::from_norito_value_ref(&payload).expect("serialize compatibility payload"),
        );
        assert!(
            SumeragiNposParameters::from_custom_parameter(&custom).is_none(),
            "nested-quoted compatibility payload must be rejected"
        );
    }

    #[test]
    fn sumeragi_npos_from_custom_parameter_accepts_valid_payload() {
        let payload = r#"{"epoch_seed":"1111111111111111111111111111111111111111111111111111111111111111","vrf_commit_window_blocks":100,"vrf_reveal_window_blocks":40,"max_validators":128,"min_self_bond":"1000","min_nomination_bond":"1","max_nominator_concentration_pct":25,"seat_band_pct":5,"max_entity_correlation_pct":25,"finality_margin_blocks":8,"evidence_horizon_blocks":7200,"activation_lag_blocks":1,"slashing_delay_blocks":259200,"epoch_length_blocks":3600}"#;
        let custom = CustomParameter::new(
            SumeragiNposParameters::parameter_id(),
            payload
                .parse::<Json>()
                .expect("fixture payload should be valid json"),
        );
        assert!(
            SumeragiNposParameters::from_custom_parameter(&custom).is_some(),
            "real chain payload should decode"
        );
    }

    #[test]
    fn sumeragi_npos_json_rejects_missing_and_zero_epoch_seed() {
        let valid = norito::json::to_value(&SumeragiNposParameters::default())
            .expect("serialize valid NPoS parameters");
        for retired in ["k_aggregators", "redundant_send_r", "unknown"] {
            let mut payload = valid.clone();
            payload
                .as_object_mut()
                .expect("NPoS payload object")
                .insert(retired.to_owned(), norito::json::Value::from(1_u64));
            assert!(
                norito::json::value::from_value::<SumeragiNposParameters>(payload).is_err(),
                "unknown or retired field `{retired}` must fail closed"
            );
        }
        let mut missing = valid.clone();
        missing
            .as_object_mut()
            .expect("NPoS payload object")
            .remove("epoch_seed");
        assert!(
            norito::json::value::from_value::<SumeragiNposParameters>(missing).is_err(),
            "missing signed epoch seed must fail"
        );

        let mut zero = valid;
        zero.as_object_mut().expect("NPoS payload object").insert(
            "epoch_seed".to_owned(),
            norito::json::Value::String("00".repeat(32)),
        );
        assert!(
            norito::json::value::from_value::<SumeragiNposParameters>(zero).is_err(),
            "all-zero signed epoch seed must fail"
        );
    }

    #[cfg(feature = "json")]
    fn handshake_metadata_fixture() -> ConsensusHandshakeMetadata {
        ConsensusHandshakeMetadata {
            mode: SumeragiConsensusMode::Permissioned,
            block_cadence_ms: NonZeroU64::new(1_000).unwrap(),
            wire_protocol_version: u32::from(crate::block::consensus_v2::PROTOCOL_VERSION),
            consensus_fingerprint: ConsensusFingerprint::new([0xab; 32]),
            sumeragi_v2:
                crate::block::consensus_v2::SumeragiV2GenesisContextParameters::recommended(),
        }
    }

    #[cfg(feature = "json")]
    #[test]
    fn handshake_metadata_rejects_arbitrary_bls_domain() {
        let mut value = norito::json::to_value(&handshake_metadata_fixture())
            .expect("serialize handshake metadata");
        value.as_object_mut().expect("metadata object").insert(
            "bls_domain".to_owned(),
            Value::String("attacker-selected-domain".to_owned()),
        );
        norito::json::value::from_value::<ConsensusHandshakeMetadata>(value)
            .expect_err("BLS domain is mode-derived and must not be accepted from genesis");
    }

    #[cfg(feature = "json")]
    #[test]
    fn handshake_metadata_validation_is_strict() {
        let baseline = handshake_metadata_fixture();
        baseline.validate().expect("canonical metadata");

        let mut bad_version = baseline.clone();
        bad_version.wire_protocol_version = 99;
        assert!(bad_version.validate().is_err());

        let mut bad_context = baseline;
        bad_context.sumeragi_v2.da_layout.parity_shards = 0;
        assert!(bad_context.validate().is_err());
    }

    #[cfg(feature = "json")]
    #[test]
    fn handshake_metadata_rejects_legacy_and_noncanonical_fingerprint_shapes() {
        let baseline = handshake_metadata_fixture();
        for malformed in [
            "ab".repeat(32),
            format!("0x{}", "AB".repeat(32)),
            format!("0x{}", "ab".repeat(31)),
        ] {
            let mut value = norito::json::to_value(&baseline).expect("serialize metadata");
            value
                .as_object_mut()
                .expect("metadata object")
                .insert("consensus_fingerprint".to_owned(), Value::String(malformed));
            norito::json::value::from_value::<ConsensusHandshakeMetadata>(value)
                .expect_err("noncanonical fingerprint must fail");
        }

        let mut legacy = norito::json::to_value(&baseline).expect("serialize metadata");
        let fields = legacy.as_object_mut().expect("metadata object");
        fields.insert(
            "wire_proto_versions".to_owned(),
            Value::Array(vec![Value::Number(Number::U64(u64::from(
                crate::block::consensus_v2::PROTOCOL_VERSION,
            )))]),
        );
        norito::json::value::from_value::<ConsensusHandshakeMetadata>(legacy)
            .expect_err("legacy plural protocol list must fail");
    }

    #[test]
    fn sumeragi_npos_from_custom_parameter_rejects_trailing_comma_payload() {
        let payload = r#"{"epoch_seed":"1111111111111111111111111111111111111111111111111111111111111111","vrf_commit_window_blocks":100,"vrf_reveal_window_blocks":40,"max_validators":128,"min_self_bond":"1","min_nomination_bond":"1","max_nominator_concentration_pct":25,"seat_band_pct":100,"max_entity_correlation_pct":25,"finality_margin_blocks":8,"evidence_horizon_blocks":7200,"activation_lag_blocks":1,"slashing_delay_blocks":259200,"epoch_length_blocks":3600,}"#;
        let custom = CustomParameter::new(
            SumeragiNposParameters::parameter_id(),
            Json::from_string_unchecked(payload.to_owned()),
        );
        assert!(
            SumeragiNposParameters::from_custom_parameter(&custom).is_none(),
            "trailing-comma compatibility payload must be rejected"
        );
    }
}
