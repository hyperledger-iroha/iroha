/// Total prospective state transition for one durable leader-wire admission.
///
/// The projection retains the complete hashed lifecycle identity, both
/// immutable ordinals, the bounded-slot status, replay-dormant membership,
/// and both high-watermarks. It therefore distinguishes exact retry
/// coalescing from restart reactivation and from strictly newer terminal-slot
/// replacement before the persistence gate mutates state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductionLeaderWireAdmissionTraceProjection {
    pub(crate) operation: u8,
    pub(crate) incoming_identity: CanonicalIdentityProjection,
    pub(crate) incumbent_identity: CanonicalIdentityProjection,
    pub(crate) stored_identity: CanonicalIdentityProjection,
    pub(crate) incoming_view: u64,
    pub(crate) incumbent_view: u64,
    pub(crate) stored_view: u64,
    pub(crate) incoming_admission_ordinal: u128,
    pub(crate) incumbent_admission_ordinal: u128,
    pub(crate) stored_admission_ordinal: u128,
    pub(crate) incoming_scheduler_ordinal: u128,
    pub(crate) incumbent_scheduler_ordinal: u128,
    pub(crate) stored_scheduler_ordinal: u128,
    pub(crate) last_admission_ordinal_before: u128,
    pub(crate) last_admission_ordinal_after: u128,
    pub(crate) scheduler_ordinal_high_watermark_before: u128,
    pub(crate) scheduler_ordinal_high_watermark_after: u128,
    pub(crate) records_before: u64,
    pub(crate) records_after: u64,
    pub(crate) capacity: u64,
    pub(crate) status_before: u8,
    pub(crate) status_after: u8,
    pub(crate) replay_dormant_before: bool,
    pub(crate) replay_dormant_after: bool,
    pub(crate) runtime_owner_before: bool,
    pub(crate) runtime_owner_after: bool,
    pub(crate) terminal_evidence_before: bool,
    pub(crate) terminal_evidence_after: bool,
    pub(crate) incoming_phase_is_timeout_certificate: bool,
    pub(crate) incumbent_phase_is_timeout_certificate: bool,
}
