//! Typed ordered-range failures, retaining original local allocation custody.

/// Failure to build or verify an ordered raw-Norito-key commitment.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NoritoKeyRangeError {
    /// A table domain is empty or exceeds its byte limit.
    #[error("ordered Norito-key table domain is invalid")]
    InvalidDomain,
    /// The half-open interval is empty, inverted, or has oversized endpoints.
    #[error("ordered Norito-key range bounds are invalid")]
    InvalidBounds,
    /// A builder received repeated or descending raw canonical key bytes.
    #[error("ordered Norito-key table rows are not strictly increasing")]
    UnsortedKeys,
    /// The requested row or proof-byte ceiling exceeds the fixed V1 ceiling.
    #[error("ordered Norito-key range limit exceeds the V1 ceiling")]
    InvalidLimit,
    /// An entry, table, or witness exceeds its admitted bound.
    #[error("ordered Norito-key table or range exceeds its admitted bound")]
    Capacity,
    /// A local allocation was refused before construction or growth.
    #[error("ordered Norito-key table or range allocation failed")]
    Allocation,
    /// The original local allocation pool refused owned tree backing.
    /// This retains its exact release observation and never means invalid state.
    #[error("ordered Norito-key allocation admission failed: {0}")]
    Admission(iroha_allocation::AllocationRefusal),
    /// The original prepaid parent cannot cover the complete fixed level geometry.
    #[error("ordered Norito-key prepaid capacity failed: {0}")]
    PrepaidCapacity(iroha_allocation::InsufficientReservation),
    /// A staged external node store refused a write or lost a required node.
    #[error("ordered Norito-key external node store failed")]
    NodeStore,
    /// A path or the empty-tree commitment differs from the supplied root.
    #[error("ordered Norito-key range root differs from its authenticated owner")]
    RootMismatch,
    /// Interior rows, boundary neighbors, indices, or membership paths are invalid.
    #[error("ordered Norito-key range witness is incomplete or malformed")]
    InvalidProof,
}
