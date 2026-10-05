//! Closed retained-bootstrap outcomes; no remote body or signing material enters diagnostics.

/// Why the exact retained bootstrap cannot advance under this worker's authorization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ManagedBootstrapFailure {
    /// Original local custody or previously published component material is missing or invalid.
    #[error("original retained service material is missing or invalid; recovery cannot replace it")]
    RetainedMaterial,
    /// The original worker was stopped; cancellation cannot authorize any successor.
    #[error("bootstrap startup was cancelled")]
    Cancelled,
    /// The original worker clock or UTC cap elapsed.
    #[error("bootstrap authorization expired; a new managed start may authorize unsigned work")]
    AuthorizationExpired,
    /// A retained unsigned transition still needs the exact authorized writer.
    #[error("bootstrap has an incomplete unsigned authorization transition")]
    TransitionPending,
    /// The sole retained payload expired and cannot be replaced by an authorization epoch.
    #[error("the retained bootstrap payload expired; its original intent remains unresolved")]
    PayloadExpired,
    /// An original signed transaction has no authenticated terminal resolution.
    #[error("the original signed bootstrap transaction remains unresolved")]
    SignedUnresolved,
    /// The outer attester body expired, independently of the paid wallet request.
    #[error("the original custody enrollment body expired; wallet retirement cannot replace it")]
    EnrollmentExpired,
    /// Original anchor-observation freshness elapsed while its paid request remains retained.
    #[error(
        "the retained custody observation expired; its original paid authorization remains bounded"
    )]
    EnrollmentObservationExpired,
    /// Fresh native custody differs from the original approved body predecessor.
    #[error("the original custody enrollment predecessor changed")]
    EnrollmentPredecessorChanged,
    /// Epochs cannot extend original generated credentials or policy validity.
    #[error("the original generated service profile expired")]
    ProfileExpired,
    /// The bounded immutable authorization history is exhausted.
    #[error("the generated bootstrap authorization history reached its finite limit")]
    EpochLimit,
    /// This worker already selected another original unsigned replacement.
    #[error("this startup already selected another unsigned bootstrap replacement")]
    ReplacementLimit,
}
