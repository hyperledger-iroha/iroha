//! Error types produced by query execution.
//!
//! Module containing errors that can occur during query execution.
#[cfg(feature = "json")]
use iroha_crypto::HashOf;
use iroha_data_model_derive::model;
use iroha_macro::FromVariant;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
pub use self::model::*;
use super::*;
use crate::prelude::*;
#[model]
mod model {
    use super::*;
    /// Query errors.
    #[derive(
        Debug,
        displaydoc::Display,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        FromVariant,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    /// High-level failure reasons for query execution.
    #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
    #[derive(thiserror::Error)]
    pub enum QueryExecutionFail {
        /// {0}
        #[error(transparent)]
        Find(FindError),
        /// Query found wrong type of asset: {0}
        Conversion(
            #[skip_from]
            #[skip_try_from]
            String,
        ),
        /// Query not found in the live query store.
        NotFound,
        /// The server's cursor does not match the provided cursor.
        CursorMismatch,
        /// There aren't enough items for the cursor to proceed.
        CursorDone,
        /// `fetch_size` must not exceed [`MAX_FETCH_SIZE`](crate::query::parameters::MAX_FETCH_SIZE).
        FetchSizeTooBig,
        /// Query execution exceeded the configured gas/materialization budget.
        GasBudgetExceeded,
        /// Some of the specified parameters (`filter/pagination/fetch_size/sorting`) are not applicable to singular queries
        InvalidSingularParameters,
        /// Reached the limit of parallel queries. Either wait for previous queries to complete, or increase the limit in the config.
        CapacityLimit,
        /// The stored cursor has expired and was removed from the server.
        Expired,
        /// The authority reached the per-tenant limit of stored cursors.
        AuthorityQuotaExceeded,
    }
    /// Stable identity carried by a missing chain-authoritative `SoraFS` proof outcome.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[norito(deny_unknown_fields)]
    pub struct SorafsProofOutcomeFindErrorV1 {
        /// Proof protocol namespace.
        pub kind: crate::sorafs::proof_ledger::ProofOutcomeKindV1,
        /// Protocol-scoped challenge or request identity.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
        pub identity_digest: [u8; 32],
    }
    /// Type assertion error
    #[derive(
        Debug,
        displaydoc::Display,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
    #[derive(thiserror::Error)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    /// Item-level errors returned when resolving query inputs.
    pub enum FindError {
        /// Failed to find asset: `{0}`
        Asset(Box<AssetId>),
        /// Failed to find asset definition: `{0}`
        AssetDefinition(AssetDefinitionId),
        /// Failed to find NFT: `{0}`
        Nft(NftId),
        /// Failed to find RWA: `{0}`
        Rwa(RwaId),
        /// Failed to find account: `{0}`
        Account(AccountId),
        /// Failed to find domain: `{0}`
        Domain(DomainId),
        /// Failed to find metadata key: `{0}`
        MetadataKey(Name),
        /// Block with hash `{0}` not found
        Block(HashOf<BlockHeader>),
        /// Transaction with hash `{0}` not found
        Transaction(HashOf<SignedTransaction>),
        /// Peer with id `{0}` not found
        Peer(PeerId),
        /// Trigger with id `{0}` not found
        Trigger(TriggerId),
        /// Role with id `{0}` not found
        Role(RoleId),
        /// Failed to find [`Permission`] by id.
        Permission(Box<Permission>),
        /// Failed to find public key: `{0}`
        PublicKey(PublicKey),
        /// Failed to find twitter binding for keyed hash `{0:?}`
        TwitterBinding(crate::oracle::KeyedHash),
        /// Failed to find oracle feed `{0}`
        OracleFeed(crate::oracle::FeedId),
        /// Failed to find oracle dispute `{0:?}`
        OracleDispute(crate::oracle::OracleDisputeId),
        /// Failed to find oracle change `{0:?}`
        OracleChange(crate::oracle::OracleChangeId),
        /// Failed to find oracle provider stats `{0:?}`
        OracleProviderStats(crate::oracle::OracleProviderKey),
        /// Failed to find `DeFi` oracle attestation `{0:?}`
        DefiOracleAttestation(crate::oracle::DefiOracleAttestationKey),
        /// Failed to find native asset escrow: `{0:?}`
        AssetEscrow(crate::escrow::EscrowId),
        /// Failed to find chain-authoritative `SoraFS` pin manifest: `{0:?}`
        SorafsPinManifest(crate::sorafs::pin_registry::ManifestDigest),
        /// Failed to find the active authoritative `SoraFS` orderbook policy
        SorafsOrderbookPolicy,
        /// Failed to find authoritative `SoraFS` orderbook order: `{0:?}`
        SorafsOrderbookOrder([u8; 32]),
        /// Failed to find authoritative `SoraFS` orderbook cancellation: `{0:?}`
        SorafsOrderbookCancellation([u8; 32]),
        /// Failed to find authoritative `SoraFS` orderbook receipt: `{0:?}`
        SorafsOrderbookReceipt([u8; 32]),
        /// Failed to find authoritative `SoraFS` orderbook trade: `{0:?}`
        SorafsOrderbookTrade([u8; 32]),
        /// Failed to find authoritative `SoraFS` orderbook channel: `{0:?}`
        SorafsOrderbookChannel([u8; 32]),
        /// Failed to find authoritative `SoraFS` orderbook status
        SorafsOrderbookStatus,
        /// Failed to find the active authoritative `SoraFS` reserve policy
        SorafsReservePolicy,
        /// Failed to find authoritative `SoraFS` provider reserve account: `{0:?}`
        SorafsReserveProvider(crate::sorafs::capacity::ProviderId),
        /// Failed to find authoritative `SoraFS` reserve movement: `{0:?}`
        SorafsReserveMovement([u8; 32]),
        /// Failed to find authoritative `SoraFS` reserve appeal: `{0:?}`
        SorafsReserveAppeal([u8; 32]),
        /// Failed to find the active authoritative `SoraFS` `PoP` issuer policy
        SorafsPopIssuerPolicy,
        /// Failed to find authoritative `SoraFS` `PoP` credential commitment: `{0:?}`
        SorafsPopCredentialCommitment([u8; 32]),
        /// Failed to find authoritative `SoraFS` `PoP` commitment root version `{0}`
        SorafsPopCommitmentRoot(u64),
        /// Failed to find authoritative `SoraFS` `PoP` revocation publication version `{0}`
        SorafsPopRevocationPublication(u64),
        /// Failed to find authoritative `SoraFS` `PoP` revocation commitment: `{0:?}`
        SorafsPopRevocation([u8; 32]),
        /// Failed to find authoritative `SoraFS` `PoP` registry audit sequence `{0}`
        SorafsPopAuditDigest(u64),
        /// Failed to find authoritative `SoraFS` `PoP` registry status
        SorafsPopRegistryStatus,
        /// Failed to find chain-authoritative `SoraFS` repair task `{0}`
        SorafsRepairTask(String),
        /// Failed to find chain-authoritative `SoraFS` repair status
        SorafsRepairStatus,
        /// Failed to find chain-authoritative `SoraFS` proof outcome `{0:?}`
        SorafsProofOutcome(SorafsProofOutcomeFindErrorV1),
        /// Failed to find the active authoritative `SoraFS` reputation-journal authority policy
        SorafsReputationJournalAuthorityPolicy,
        /// Failed to find a finalized `SoraFS` reputation-journal event for source `{0:?}`
        SorafsReputationJournalEvent(crate::sorafs::reputation::ReputationJournalSourceIdV1),
        /// Failed to find the active authoritative `SoraFS` moderation policy
        SorafsModerationPolicy,
        /// Failed to find authoritative `SoraFS` moderation appeal `{0}`
        SorafsModerationAppeal(String),
        /// Failed to find authoritative `SoraFS` moderation juror eligibility `{0}`
        SorafsModerationJurorEligibility(String),
        /// Failed to find authoritative `SoraFS` moderation case `{0}`
        SorafsModerationCase(String),
        /// Failed to find authoritative `SoraFS` moderation commit `{0}`
        SorafsModerationCommit(String),
        /// Failed to find authoritative `SoraFS` moderation reveal `{0}`
        SorafsModerationReveal(String),
        /// Failed to find authoritative `SoraFS` moderation challenge `{0}`
        SorafsModerationChallenge(String),
        /// Failed to find authoritative `SoraFS` moderation outcome `{0}`
        SorafsModerationOutcome(String),
        /// Failed to find authoritative `SoraFS` moderation no-show `{0}`
        SorafsModerationNoShow(String),
        /// Failed to find authoritative `SoraFS` moderation status
        SorafsModerationStatus,
    }
}
