//! Shared data-availability helpers used by Torii and client SDKs.
//!
//! This module exposes deterministic sampling and assignment logic so the
//! server, CLI, and SDKs can agree on which chunks to probe for DA proofs.
mod manifest;
mod queries;
pub mod sampling;
pub use manifest::DaManifestResponse;
pub use queries::{
    DA_QUERY_REQUEST_MAX_BYTES, DEFAULT_DA_QUERY_PAGE_SIZE, DaCommitmentListCursor,
    DaCommitmentListRequest, DaCommitmentListResponse, DaCommitmentProofRequest,
    DaCommitmentProofResponse, DaCommitmentVerifyResponse, DaListSnapshot, DaPinIntentListCursor,
    DaPinIntentListRequest, DaPinIntentListResponse, DaPinIntentQueryRequest,
    DaPinIntentVerifyResponse, DaQueryValidationError, MAX_DA_QUERY_PAGE_SIZE,
};
