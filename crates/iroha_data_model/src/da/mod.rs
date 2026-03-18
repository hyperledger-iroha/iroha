//! Data availability (DA) data model scaffolding.
//!
//! This module hosts forward-looking types for the DA ingest pipeline (Torii
//! blob submission, manifest generation, replication metadata) described in
//! `docs/source/da/ingest_plan.md`. The actual networking/storage plumbing
//! lives outside the data model; these structures exist purely for Norito
//! encoding, schema documentation, and cross-crate integration.

/// Commitment structures embedded into blocks and RPC responses.
pub mod commitment;
/// Confidential-compute lane policies.
pub mod confidential_compute;
/// Norito request/receipt payloads for the Torii DA ingest API.
pub mod ingest;
/// Canonical manifest structure emitted after chunking/replication.
pub mod manifest;
/// Intent records used to populate the `SoraFS` registry from DA ingest artefacts.
pub mod pin_intent;
/// Common DA type aliases and shared structs (blob identifiers, policies, etc.).
pub mod types;

/// Convenience re-export of frequently used DA types.
pub mod prelude {
    pub use super::{
        commitment::{
            DaCommitmentBundle, DaCommitmentKey, DaCommitmentLocation, DaCommitmentProof,
            DaCommitmentRecord, DaCommitmentWithLocation, DaProofPolicy, DaProofPolicyBundle,
            DaProofScheme, KzgCommitment, MerkleDirection, MerklePathItem, RetentionClass,
        },
        confidential_compute::{ConfidentialComputeMechanism, ConfidentialComputePolicy},
        ingest::{DaIngestReceipt, DaIngestRequest, DaStripeLayout},
        manifest::{ChunkCommitment, ChunkRole, DaManifestV1},
        pin_intent::{DaPinIntent, DaPinIntentBundle, DaPinIntentWithLocation},
        types::{
            BlobClass, BlobCodec, BlobDigest, ChunkDigest, Compression, DaRentError,
            DaRentLedgerProjection, DaRentPolicyV1, DaRentQuote, ErasureProfile, ExtraMetadata,
            FecScheme, GovernanceTag, MetadataCipherEnvelope, MetadataEncryption, MetadataEntry,
            MetadataVisibility, RetentionPolicy, StorageTicketId,
        },
    };
}
