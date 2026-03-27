//! Implement Norito slice-based decoding for data model types used inside
//! packed sequences and option fields.
//!
//! These impls simply reinterpret the provided slice as an archived payload of
//! the target type and delegate to `NoritoDeserialize`. The encoded lengths are
//! tracked by container decoders (e.g., Vec/Option) so we can return the full
//! slice length as bytes consumed.

use norito::core::{DecodeFromSlice, Error, decode_field_canonical};

fn decode_via_canonical<T>(bytes: &[u8]) -> Result<(T, usize), Error>
where
    T: for<'de> norito::NoritoDeserialize<'de> + norito::NoritoSerialize,
{
    decode_field_canonical::<T>(bytes)
}

// Helper macro to implement `DecodeFromSlice` for many local types.
macro_rules! impl_decode_from_slice_via_archived {
    ($($ty:path),+ $(,)?) => {
        $(
            impl<'a> DecodeFromSlice<'a> for $ty {
                fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), Error> {
                    decode_via_canonical(bytes)
                }
            }
        )+
    };
}

// Core ID and value types
impl_decode_from_slice_via_archived! {
    crate::ipfs::IpfsPath,
    crate::sorafs_uri::SorafsUri,
    crate::peer::PeerId,
    crate::domain::DomainId,
    crate::asset::id::AssetId,
    crate::asset::id::AssetDefinitionId,
    crate::asset::alias::AssetDefinitionAlias,
    crate::nft::NftId,
    crate::rwa::RwaId,
    crate::trigger::TriggerId,
    crate::role::RoleId,
    crate::parameter::CustomParameterId,
    crate::sorafs::capacity::ProviderId,
}

// State / canonical keys (DecodeFromSlice derived on the type; no shim needed)

// Transaction-related
// Transaction-related (derive already supplies DecodeFromSlice for these)

// Trigger entrypoints and related types
impl_decode_from_slice_via_archived! {
    crate::trigger::time::TimeTriggerEntrypoint,
}

// Block-related
impl_decode_from_slice_via_archived! {
    crate::block::header::BlockHeader,
    crate::block::header::BlockSignature,
    crate::block::payload::BlockResult,
    crate::smart_contract::manifest::ContractManifest,
    crate::smart_contract::manifest::AccessSetHints,
}

// Proof-related
impl_decode_from_slice_via_archived! {
    crate::proof::ProofId,
    crate::proof::VerifyingKeyId,
    crate::proof::ProofAttachmentList,
    crate::proof::ProofRecord,
    crate::nexus::LanePrivacyProof,
    crate::nexus::LanePrivacyWitness,
    crate::nexus::LanePrivacyMerkleWitness,
    crate::nexus::LanePrivacySnarkWitness,
    // Also cover event payloads that may appear in option/sequence contexts
    crate::events::data::proof::ProofVerified,
    crate::events::data::proof::ProofRejected,
    crate::runtime::RuntimeUpgradeId,
}

// Kaigi components referenced by query/data-model responses
impl_decode_from_slice_via_archived! {
    crate::kaigi::KaigiParticipantCommitment,
    crate::kaigi::KaigiParticipantNullifier,
    crate::kaigi::KaigiRelayHop,
    crate::kaigi::KaigiRelayManifest,
    crate::kaigi::KaigiRelayRegistration,
}

// Query parameter and DSL types; keep only those not covered by derives
impl_decode_from_slice_via_archived! {
    crate::query::parameters::ForwardCursor,
    crate::query::parameters::Pagination,
    crate::query::parameters::Sorting,
    crate::query::parameters::SortOrder,
    crate::query::parameters::FetchSize,
    crate::parameter::Parameters,
    crate::query::proof::FindProofRecordById,
    crate::query::AnyQueryBox,
    crate::query::QueryResponse,
}

// Events and statuses
impl_decode_from_slice_via_archived! {
    crate::events::pipeline::BlockStatus,
    crate::events::pipeline::TransactionStatus,
    crate::events::trigger_completed::TriggerCompletedOutcomeType,
}

// Transaction-related shims required by versioned types and proofs
impl_decode_from_slice_via_archived! {
    crate::transaction::error::TransactionRejectionReason,
    crate::ValidationFail,
}

// Additional model and crypto types referenced by query responses and versioned wrappers
impl_decode_from_slice_via_archived! {
    // Core model objects (use public re-exports)
    crate::domain::Domain,
    crate::account::Account,
    crate::asset::definition::AssetDefinition,
    crate::asset::definition::AssetConfidentialPolicy,
    crate::asset::definition::ConfidentialPolicyTransition,
    crate::asset::definition::ConfidentialPolicyMode,
    crate::confidential::ConfidentialFeatureDigest,
    crate::asset::value::Asset,
    crate::nft::Nft,
    crate::rwa::Rwa,
    crate::permission::Permission,
    crate::parameter::system::Parameter,
    // Triggers and actions
    crate::trigger::model::Trigger,
    crate::trigger::action::Action,
    crate::trigger::data::DataTriggerStep,
    // Transactions, blocks and query outputs
    crate::transaction::signed::TransactionEntrypoint,
    crate::transaction::signed::TransactionResult,
    crate::block::SignedBlock,
    crate::query::CommittedTransaction,
    crate::query::QueryOutputBatchBox,
    // Taikai metadata and envelopes
    crate::taikai::TaikaiEventId,
    crate::taikai::TaikaiStreamId,
    crate::taikai::TaikaiRenditionId,
    crate::taikai::TaikaiTrackKind,
    crate::taikai::TaikaiAudioLayout,
    crate::taikai::TaikaiCodec,
    crate::taikai::TaikaiResolution,
    crate::taikai::TaikaiTrackMetadata,
    crate::taikai::TaikaiCarPointer,
    crate::taikai::TaikaiIngestPointer,
    crate::taikai::TaikaiInstrumentation,
    crate::taikai::TaikaiSegmentEnvelopeV1,
    crate::taikai::TaikaiTimeIndexKey,
    crate::taikai::TaikaiCidIndexKey,
    crate::taikai::TaikaiEnvelopeIndexes,
    // SoraDNS resolver attestation data
    crate::soradns::GatewayHostSet,
    crate::soradns::HttpTransportV1,
    crate::soradns::TlsTransportV1,
    crate::soradns::QuicTransportV1,
    crate::soradns::OdohRelayV1,
    crate::soradns::SoranetBridgeConfigV1,
    crate::soradns::PaddingPolicyV1,
    crate::soradns::TlsProvisioningProfile,
    crate::soradns::ResolverTransportBundle,
    crate::soradns::ResolverTlsBundle,
    crate::soradns::RotationPolicyV1,
    crate::soradns::DirectoryDraftSubmittedEventV1,
    crate::soradns::DirectoryPolicyUpdatedEventV1,
    crate::soradns::DirectoryPublishedEventV1,
    crate::soradns::DirectoryReleaseSignerEventV1,
    crate::soradns::DirectoryRevokedEventV1,
    crate::soradns::DirectoryRotationPolicyV1,
    crate::soradns::DirectoryUnrevokedEventV1,
    crate::soradns::PendingDirectoryDraftV1,
    crate::soradns::ResolverAttestationDocumentV1,
    crate::soradns::ResolverDirectoryEventV1,
    crate::soradns::ResolverDirectoryRecordV1,
    crate::soradns::ResolverRevocationRecordV1,
    crate::soradns::RadRevokeReason,
    crate::offline::AggregateProofEnvelope,
    crate::offline::AndroidIntegrityPolicy,
    crate::offline::OfflineVerdictSnapshot,
    crate::offline::OfflinePlatformTokenSnapshot,
    crate::offline::PoseidonDigest,
}

// Governance types (feature-gated, but default-enabled)
#[cfg(feature = "governance")]
impl_decode_from_slice_via_archived! {
    crate::governance::types::ProposalId,
    crate::governance::types::AtWindow,
}

// ISI Governance enums used in instruction params
#[cfg(feature = "governance")]
impl_decode_from_slice_via_archived! {
    crate::isi::governance::VotingMode,
}
