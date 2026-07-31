/// Certification status of one exact autonomous reservation attempt.
///
/// `Exact` carries the complete independently validated prepare/commit
/// artifact. Callers must never classify a certified attempt as a terminal
/// loser, even when its canonical global carrier has not yet been found.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AutonomousLaneReservationCertificationV1 {
    /// Neither the exact indexed slot nor its mandatory frontier certifies the
    /// requested immutable origin proposal.
    Uncertified,
    /// The exact immutable origin proposal has a restart-verifiable lane-local
    /// commit certificate.
    Exact(CertifiedLaneBlockArtifact),
}

impl AutonomousLaneReservationCertificationV1 {
    /// Return whether lane-local certification forbids terminal-loser release.
    #[must_use]
    pub(crate) const fn is_certified(&self) -> bool {
        matches!(self, Self::Exact(_))
    }
}

/// Strict read-only Kura classification for one complete Queue reservation
/// group.
///
/// Every non-absent variant has already matched the complete FIFO-ordered
/// reservation vector against the producer-authenticated payload's canonical
/// bytes. No variant is derived from a latest-attempt pointer.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AutonomousLaneReservationEvidenceV1 {
    /// No artifact, view state, pointer, or other proposal-height attempt
    /// occupies the requested active lane-height slot.
    StrictlyAbsent,
    /// The exact proposal-height attempt remains live.
    ExactLive {
        /// Complete producer-authenticated executable payload.
        payload: LaneExecutablePayloadV1,
        /// Exact lane-local certification status.
        certification: AutonomousLaneReservationCertificationV1,
    },
    /// The exact proposal-height attempt has one durable terminal retirement.
    ExactRetired {
        /// Complete producer-authenticated executable payload retained to bind
        /// the tombstone and ordered reservation vector.
        payload: LaneExecutablePayloadV1,
        /// Exact durable terminal identity.
        retirement: AutonomousLaneSlotRetirementV1,
        /// Exact lane-local certification status. A certified retirement is a
        /// fail-closed conflict for release callers.
        certification: AutonomousLaneReservationCertificationV1,
    },
}

/// Typed failure returned by exact autonomous reservation classification.
#[derive(Debug, thiserror::Error)]
pub(crate) enum AutonomousLaneReservationEvidenceError {
    /// Queue supplied a malformed or internally inconsistent group: {0}
    #[error("invalid autonomous reservation reconciliation group: {0}")]
    InvalidGroup(&'static str),
    /// The producer-authenticated payload does not carry the exact canonical
    /// FIFO-ordered reservation vector supplied by Queue.
    #[error("autonomous reservation group conflicts with the durable payload reservation vector")]
    ReservationVectorConflict,
    /// Another proposal-height attempt or orphan view/pointer occupies the
    /// requested lane-local slot.
    #[error("autonomous reservation group conflicts with other durable proposal-height evidence")]
    OtherAttemptConflict,
    /// A certified artifact at the requested lane-local height names another
    /// attempt, or the indexed artifact and mandatory frontier disagree.
    #[error("autonomous reservation group conflicts with durable lane certification evidence")]
    CertifiedArtifactConflict,
    /// An exact durable entrypoint owner is missing, belongs to another
    /// payload, has an incompatible release state, or has a conflicting crash
    /// stage. The path is retained for deterministic operator diagnosis.
    #[error("autonomous reservation group conflicts with entrypoint claim evidence at {path:?}")]
    EntrypointClaimConflict {
        /// Exact main or staged claim path which failed preflight.
        path: PathBuf,
    },
    /// A crash-recovery artifact is present. Classification is deliberately
    /// read-only, so startup recovery must resolve it before retrying.
    #[error(
        "autonomous reservation classification found unresolved temporary evidence at {path:?}"
    )]
    UnresolvedTemporary {
        /// Exact temporary path which requires authenticated recovery.
        path: PathBuf,
    },
    /// The batch or the evidence it requested exceeds the fixed startup scan
    /// and decode budget.
    #[error("autonomous reservation classification exceeds its aggregate evidence budget")]
    AggregateBudgetExceeded,
    /// Exact bounded Kura validation or I/O failed.
    #[error(transparent)]
    Kura(#[from] Error),
}

#[derive(Debug, Default)]
struct AutonomousReservationLaneInventory {
    attempts: BTreeMap<(u64, u64), u64>,
    view_states: BTreeMap<(u64, u64), u64>,
    lane_latest: BTreeMap<u64, u64>,
    route_latest_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
struct AutonomousReservationAttemptRead {
    payload: LaneExecutablePayloadV1,
    retirement: Option<AutonomousLaneSlotRetirementV1>,
}

#[derive(Debug, Default)]
struct AutonomousReservationCertifiedLaneSnapshot {
    requested: BTreeMap<u64, CertifiedLaneBlockArtifact>,
    frontier: Option<CertifiedLaneBlockArtifact>,
}
