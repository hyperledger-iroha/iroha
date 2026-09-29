//! Shared consensus parameter and payload-availability wire values.
//!
//! The active consensus state machine is `iroha_sumeragi`. Native certified block
//! proofs and execution commitments are owned by [`crate::sumeragi_finality`].
#[cfg(test)]
use super::Header as BlockHeader;

#[cfg(test)]
use crate::NetworkId;
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{account::AccountId, nexus::PublicLaneValidatorRecord};
use core::fmt;
#[cfg(test)]
use iroha_crypto::{Algorithm, KeyPair};
use iroha_crypto::{Hash, HashOf};
use iroha_model_base::peer::PeerId;
use iroha_model_base::topology::LaneId;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::vec::Vec;
/// Canonical genesis/handshake fingerprint projection.
pub mod fingerprint;
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
/// must stay aligned with the production reducer and the formal Sumeragi
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
/// Permissioned Sumeragi handshake and domain-separation tag.
pub const PERMISSIONED_TAG: &str = "iroha3-consensus::permissioned-sumeragi@v1";
/// `NPoS` Sumeragi handshake and domain-separation tag.
pub const NPOS_TAG: &str = "iroha3-consensus::npos-sumeragi@v1";
/// BLS domain selected by a permissioned genesis.
pub const PERMISSIONED_BLS_DOMAIN: &str = "bls-iroha3:permissioned-sumeragi:v1";
/// BLS domain selected by an `NPoS` genesis.
pub const NPOS_BLS_DOMAIN: &str = "bls-iroha3:npos-sumeragi:v1";
/// Consensus-wide upper bound for the canonical result-bearing block wire.
///
/// This is the protocol authority shared by execution-commitment admission and
/// durable canonical-block storage. It deliberately matches the first-release
/// Kura hard limit; runtime configuration may select a lower bound but must
/// never admit a larger consensus value.
pub const MAX_EXECUTED_BLOCK_WIRE_BYTES: u64 = 256 * 1024 * 1024;
/// Canonical Nexus/AMX context commitment for the repository's recommended
/// single-lane defaults and no staged public-lane validators.
///
/// `iroha_config` owns the projection and pins this value with a golden test.
/// Keeping the bytes here lets configuration-independent genesis builders emit
/// a valid signed template without introducing a data-model/config cycle.
pub const RECOMMENDED_NEXUS_AMX_CONTEXT_HASH: [u8; 32] = [
    220, 232, 211, 211, 61, 114, 186, 115, 100, 1, 0, 110, 240, 35, 151, 111, 88, 0, 178, 133, 41,
    51, 110, 13, 124, 189, 155, 111, 238, 246, 21, 21,
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
    /// Consensus vote count. The native protocol requires this to be exactly one.
    pub power: u64,
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
/// Payload encoding used by RS16 data dissemination.
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
/// Genesis-selected transport inputs needed to construct every Sumeragi
/// height context.
///
/// The value is embedded in the signed consensus-genesis parameters. Live
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
/// Canonical staged active-lane record committed by genesis metadata.
pub type GenesisActiveNexusLaneRecord = ((LaneId, AccountId), PublicLaneValidatorRecord);
/// Typed identity of the native frozen chain parameters.
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
    pub HashOf<crate::sumeragi_finality::ChainParamsRecord>,
);
/// Invalid signed consensus metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationError {
    /// RS16 dimensions or resource bounds are invalid.
    InvalidDataAvailabilityLayout,
    /// The staged Nexus commitment is not a canonical nonzero hash.
    InvalidNexusAmxContextHash,
    /// The staged execution-policy commitment is not a canonical nonzero hash.
    InvalidExecutionPolicyHash,
    /// The RS16 encoded chunk count exceeds its wire representation.
    ChunkCountTooLarge,
}
impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidDataAvailabilityLayout => "invalid data-availability layout",
            Self::InvalidNexusAmxContextHash => "invalid Nexus context hash",
            Self::InvalidExecutionPolicyHash => "invalid execution-policy hash",
            Self::ChunkCountTooLarge => "payload chunk count exceeds the wire range",
        })
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
        Hash::new(b"Sumeragi unit-test genesis"),
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
mod captured_consensus_v2_schema_tests;
