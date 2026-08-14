//! Low-cardinality operational metrics for the Musubi V1 package ecosystem.
//!
//! Every label is selected from a closed Rust enum. Package identities,
//! namespaces, accounts, aliases, archive IDs, provider IDs, transaction
//! hashes, operation IDs, URLs, and raw error strings therefore cannot enter
//! the Prometheus cardinality surface through this API.
use prometheus::{
    IntCounterVec, Opts, Registry,
    core::{AtomicU64, Collector, GenericGauge, GenericGaugeVec},
};
/// One of the seven resumable Musubi publication phases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MusubiPublicationPhaseMetricV1 {
    /// Clean package, compiler, and exact graph validation.
    Validation,
    /// Authenticated SoraFS seed ingress.
    SeedIngress,
    /// Idempotent archive, permanent pin, and order registration.
    ArchiveRegistration,
    /// Finalized distinct-provider replication quorum.
    Replication,
    /// Complete readback through two distinct providers.
    Readback,
    /// Native AMX release submission.
    ReleaseSubmission,
    /// Exact finalized home and universal-index verification.
    FinalVerification,
}
impl MusubiPublicationPhaseMetricV1 {
    const ALL: [Self; 7] = [
        Self::Validation,
        Self::SeedIngress,
        Self::ArchiveRegistration,
        Self::Replication,
        Self::Readback,
        Self::ReleaseSubmission,
        Self::FinalVerification,
    ];
    const fn label(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::SeedIngress => "seed_ingress",
            Self::ArchiveRegistration => "archive_registration",
            Self::Replication => "replication",
            Self::Readback => "readback",
            Self::ReleaseSubmission => "release_submission",
            Self::FinalVerification => "final_verification",
        }
    }
}
/// Bounded terminal class for authenticated seed-ingress deadletters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MusubiIngestDeadletterReasonV1 {
    /// Receipt signature, binding, or signer policy was invalid.
    ReceiptInvalid,
    /// Receipt expired before it could be committed.
    ReceiptExpired,
    /// Receipt nonce or idempotency identity was replayed inconsistently.
    ReceiptReplay,
    /// Broker or seed provider was not admitted.
    AdmissionRejected,
    /// The canonical body could not be durably staged.
    StorageRejected,
    /// A bounded fallback class for a new, not-yet-separated failure.
    Other,
}
impl MusubiIngestDeadletterReasonV1 {
    const fn label(self) -> &'static str {
        match self {
            Self::ReceiptInvalid => "receipt_invalid",
            Self::ReceiptExpired => "receipt_expired",
            Self::ReceiptReplay => "receipt_replay",
            Self::AdmissionRejected => "admission_rejected",
            Self::StorageRejected => "storage_rejected",
            Self::Other => "other",
        }
    }
}
/// Bounded surface at which a Musubi integrity check failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MusubiIntegritySurfaceV1 {
    /// Archive commitment or derived `ArchiveId`.
    ArchiveCommitment,
    /// Canonical semantic bundle.
    Bundle,
    /// Typed artifact descriptor.
    Descriptor,
    /// Normalized source tree.
    SourceTree,
    /// Normalized exact verification lock.
    VerificationLock,
    /// Provider-signed parsed-bundle attestation.
    ProviderAttestation,
    /// Two-provider canonical archive readback.
    ProviderReadback,
    /// No-follow immutable cache extraction.
    CacheExtraction,
    /// A bounded fallback class for a new, not-yet-separated surface.
    Other,
}
impl MusubiIntegritySurfaceV1 {
    const fn label(self) -> &'static str {
        match self {
            Self::ArchiveCommitment => "archive_commitment",
            Self::Bundle => "bundle",
            Self::Descriptor => "descriptor",
            Self::SourceTree => "source_tree",
            Self::VerificationLock => "verification_lock",
            Self::ProviderAttestation => "provider_attestation",
            Self::ProviderReadback => "provider_readback",
            Self::CacheExtraction => "cache_extraction",
            Self::Other => "other",
        }
    }
}
/// Bounded cache operation that detected corruption.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MusubiCacheOperationV1 {
    /// Archive fetch or initial extraction.
    Fetch,
    /// Read-only commitment verification.
    Verify,
    /// Validated quarantine and refetch repair.
    Repair,
    /// Trusted retained-set pruning.
    Prune,
}
impl MusubiCacheOperationV1 {
    const fn label(self) -> &'static str {
        match self {
            Self::Fetch => "fetch",
            Self::Verify => "verify",
            Self::Repair => "repair",
            Self::Prune => "prune",
        }
    }
}
/// Bounded finalized-query cursor failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MusubiCursorFailureReasonV1 {
    /// Cursor bytes or structural fields were invalid.
    Invalid,
    /// Finalized height or block hash changed.
    StaleAnchor,
    /// Universal resolver-index revision changed.
    StaleRevision,
    /// Cursor was replayed against another typed query.
    WrongQuery,
    /// Caller-bound cursor was replayed by another caller.
    WrongCaller,
    /// Last-key boundary was absent or outside the requested prefix.
    Boundary,
    /// A bounded fallback class for a new, not-yet-separated reason.
    Other,
}
impl MusubiCursorFailureReasonV1 {
    const fn label(self) -> &'static str {
        match self {
            Self::Invalid => "invalid",
            Self::StaleAnchor => "stale_anchor",
            Self::StaleRevision => "stale_revision",
            Self::WrongQuery => "wrong_query",
            Self::WrongCaller => "wrong_caller",
            Self::Boundary => "boundary",
            Self::Other => "other",
        }
    }
}
/// Bounded package, alias, or registry governance action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MusubiGovernanceActionV1 {
    /// Register or replay an immutable namespace binding.
    NamespaceBinding,
    /// Register or replay an immutable archive commitment.
    ArchiveRegistration,
    /// Claim or publish a release.
    Publish,
    /// Yank or unyank a release.
    Yank,
    /// Change package metadata.
    Metadata,
    /// Invite a package member.
    Invite,
    /// Accept a package invitation.
    Accept,
    /// Change an accepted member role.
    SetRole,
    /// Remove an accepted member.
    Remove,
    /// Add or retire an archive location.
    ArchiveLocation,
    /// Register or resolve a permanent alias mutation.
    Alias,
    /// Parliament package-owner recovery.
    Recovery,
    /// Parliament artifact takedown.
    Takedown,
    /// Parliament registry admission or pricing policy change.
    Policy,
}
impl MusubiGovernanceActionV1 {
    const fn label(self) -> &'static str {
        match self {
            Self::NamespaceBinding => "namespace_binding",
            Self::ArchiveRegistration => "archive_registration",
            Self::Publish => "publish",
            Self::Yank => "yank",
            Self::Metadata => "metadata",
            Self::Invite => "invite",
            Self::Accept => "accept",
            Self::SetRole => "set_role",
            Self::Remove => "remove",
            Self::ArchiveLocation => "archive_location",
            Self::Alias => "alias",
            Self::Recovery => "recovery",
            Self::Takedown => "takedown",
            Self::Policy => "policy",
        }
    }
}
/// Bounded reason for rejecting a governance mutation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MusubiGovernanceRejectionReasonV1 {
    /// The authority lacks the accepted role or capability.
    Unauthorized,
    /// Compare-and-set governance or metadata revision was stale.
    StaleRevision,
    /// The mutation would remove the last package owner.
    LastOwner,
    /// Registry policy blocks this class of new mutation.
    PolicyClosed,
    /// Parliament decision was absent, delayed, or action-mismatched.
    InvalidDecision,
    /// An enacted Parliament decision was already consumed.
    Replay,
    /// Alias price revision or atomic XOR payment was invalid.
    Payment,
    /// A bounded fallback class for a new, not-yet-separated reason.
    Other,
}
impl MusubiGovernanceRejectionReasonV1 {
    const fn label(self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::StaleRevision => "stale_revision",
            Self::LastOwner => "last_owner",
            Self::PolicyClosed => "policy_closed",
            Self::InvalidDecision => "invalid_decision",
            Self::Replay => "replay",
            Self::Payment => "payment",
            Self::Other => "other",
        }
    }
}
/// Registered Musubi V1 metrics with a closed label surface.
#[derive(Clone)]
pub struct MusubiMetrics {
    publication_phase_age_seconds: GenericGaugeVec<AtomicU64>,
    replication_shortfall_releases: GenericGauge<AtomicU64>,
    ingest_deadletters_total: IntCounterVec,
    integrity_failures_total: IntCounterVec,
    cache_corruption_total: IntCounterVec,
    cursor_failures_total: IntCounterVec,
    governance_rejections_total: IntCounterVec,
    storage_bytes_used: GenericGauge<AtomicU64>,
    storage_bytes_capacity: GenericGauge<AtomicU64>,
}
impl MusubiMetrics {
    pub(super) fn new(registry: &Registry) -> Self {
        let publication_phase_age_seconds = GenericGaugeVec::new(
            Opts::new(
                "musubi_publication_phase_age_seconds",
                "Age in seconds of the oldest active Musubi publication in each bounded phase",
            ),
            &["phase"],
        )
        .expect("Musubi metric definition is valid");
        let replication_shortfall_releases = GenericGauge::new(
            "musubi_replication_shortfall_releases",
            "Musubi release references bound to archives below the fresh-selection replica quorum",
        )
        .expect("Musubi metric definition is valid");
        let ingest_deadletters_total = IntCounterVec::new(
            Opts::new(
                "musubi_ingest_deadletters_total",
                "Terminal authenticated Musubi seed-ingress failures by bounded reason",
            ),
            &["reason"],
        )
        .expect("Musubi metric definition is valid");
        let integrity_failures_total = IntCounterVec::new(
            Opts::new(
                "musubi_integrity_failures_total",
                "Musubi commitment verification failures by bounded surface",
            ),
            &["surface"],
        )
        .expect("Musubi metric definition is valid");
        let cache_corruption_total = IntCounterVec::new(
            Opts::new(
                "musubi_cache_corruption_total",
                "Musubi immutable-cache corruption detections by bounded operation",
            ),
            &["operation"],
        )
        .expect("Musubi metric definition is valid");
        let cursor_failures_total = IntCounterVec::new(
            Opts::new(
                "musubi_cursor_failures_total",
                "Musubi finalized-query cursor failures by bounded reason",
            ),
            &["reason"],
        )
        .expect("Musubi metric definition is valid");
        let governance_rejections_total = IntCounterVec::new(
            Opts::new(
                "musubi_governance_rejections_total",
                "Rejected Musubi governance mutations by bounded action and reason",
            ),
            &["action", "reason"],
        )
        .expect("Musubi metric definition is valid");
        let storage_bytes_used = GenericGauge::new(
            "musubi_storage_bytes_used",
            "Bytes occupied by the measured Musubi archive or immutable-cache root",
        )
        .expect("Musubi metric definition is valid");
        let storage_bytes_capacity = GenericGauge::new(
            "musubi_storage_bytes_capacity",
            "Configured byte capacity of the measured Musubi archive or immutable-cache root",
        )
        .expect("Musubi metric definition is valid");
        register(registry, &publication_phase_age_seconds);
        register(registry, &replication_shortfall_releases);
        register(registry, &ingest_deadletters_total);
        register(registry, &integrity_failures_total);
        register(registry, &cache_corruption_total);
        register(registry, &cursor_failures_total);
        register(registry, &governance_rejections_total);
        register(registry, &storage_bytes_used);
        register(registry, &storage_bytes_capacity);
        Self {
            publication_phase_age_seconds,
            replication_shortfall_releases,
            ingest_deadletters_total,
            integrity_failures_total,
            cache_corruption_total,
            cursor_failures_total,
            governance_rejections_total,
            storage_bytes_used,
            storage_bytes_capacity,
        }
    }
    /// Reset every publication phase gauge before projecting a fresh journal snapshot.
    pub fn reset_publication_phase_ages(&self) {
        for phase in MusubiPublicationPhaseMetricV1::ALL {
            self.publication_phase_age_seconds
                .with_label_values(&[phase.label()])
                .set(0);
        }
    }
    /// Set the age of the oldest active publication in one bounded phase.
    pub fn set_publication_phase_age(
        &self,
        phase: MusubiPublicationPhaseMetricV1,
        age_seconds: u64,
    ) {
        self.publication_phase_age_seconds
            .with_label_values(&[phase.label()])
            .set(age_seconds);
    }
    /// Set the number of releases currently excluded for replica shortfall.
    pub fn set_replication_shortfall_releases(&self, releases: u64) {
        self.replication_shortfall_releases.set(releases);
    }
    /// Record one terminal authenticated seed-ingress failure.
    pub fn inc_ingest_deadletter(&self, reason: MusubiIngestDeadletterReasonV1) {
        self.ingest_deadletters_total
            .with_label_values(&[reason.label()])
            .inc();
    }
    /// Record one commitment verification failure.
    pub fn inc_integrity_failure(&self, surface: MusubiIntegritySurfaceV1) {
        self.integrity_failures_total
            .with_label_values(&[surface.label()])
            .inc();
    }
    /// Record one immutable-cache corruption detection.
    pub fn inc_cache_corruption(&self, operation: MusubiCacheOperationV1) {
        self.cache_corruption_total
            .with_label_values(&[operation.label()])
            .inc();
    }
    /// Record one finalized-query cursor failure.
    pub fn inc_cursor_failure(&self, reason: MusubiCursorFailureReasonV1) {
        self.cursor_failures_total
            .with_label_values(&[reason.label()])
            .inc();
    }
    /// Record one rejected package, alias, or Parliament mutation.
    pub fn inc_governance_rejection(
        &self,
        action: MusubiGovernanceActionV1,
        reason: MusubiGovernanceRejectionReasonV1,
    ) {
        self.governance_rejections_total
            .with_label_values(&[action.label(), reason.label()])
            .inc();
    }
    /// Project one measurement pair used to calculate storage pressure.
    pub fn set_storage_usage(&self, used_bytes: u64, capacity_bytes: u64) {
        self.storage_bytes_capacity.set(capacity_bytes);
        self.storage_bytes_used.set(used_bytes);
    }
}
fn register<C: Collector + Clone + 'static>(registry: &Registry, metric: &C) {
    registry
        .register(Box::new(metric.clone()))
        .expect("Musubi metric names are unique and valid");
}
#[cfg(test)]
mod tests {
    use prometheus::{Encoder as _, TextEncoder};
    use super::*;
    #[test]
    fn phase_age_series_remain_absent_until_a_successful_projection() {
        let registry = Registry::new();
        let metrics = MusubiMetrics::new(&registry);
        let mut encoded = Vec::new();
        TextEncoder::new()
            .encode(&registry.gather(), &mut encoded)
            .expect("encode empty Musubi metrics");
        let encoded = String::from_utf8(encoded).expect("Prometheus output is UTF-8");
        assert!(!encoded.contains("musubi_publication_phase_age_seconds"));
        metrics.reset_publication_phase_ages();
        let mut projected = Vec::new();
        TextEncoder::new()
            .encode(&registry.gather(), &mut projected)
            .expect("encode projected Musubi metrics");
        let projected = String::from_utf8(projected).expect("Prometheus output is UTF-8");
        assert_eq!(
            projected
                .lines()
                .filter(|line| line.starts_with("musubi_publication_phase_age_seconds{"))
                .count(),
            MusubiPublicationPhaseMetricV1::ALL.len()
        );
    }
    #[test]
    fn exports_only_bounded_musubi_labels() {
        let registry = Registry::new();
        let metrics = MusubiMetrics::new(&registry);
        metrics.set_publication_phase_age(MusubiPublicationPhaseMetricV1::Readback, 31);
        metrics.set_replication_shortfall_releases(2);
        metrics.set_replication_shortfall_releases(4);
        metrics.inc_ingest_deadletter(MusubiIngestDeadletterReasonV1::ReceiptExpired);
        metrics.inc_integrity_failure(MusubiIntegritySurfaceV1::ProviderReadback);
        metrics.inc_cache_corruption(MusubiCacheOperationV1::Verify);
        metrics.inc_cursor_failure(MusubiCursorFailureReasonV1::StaleRevision);
        metrics.inc_governance_rejection(
            MusubiGovernanceActionV1::SetRole,
            MusubiGovernanceRejectionReasonV1::Unauthorized,
        );
        metrics.inc_governance_rejection(
            MusubiGovernanceActionV1::NamespaceBinding,
            MusubiGovernanceRejectionReasonV1::Unauthorized,
        );
        metrics.inc_governance_rejection(
            MusubiGovernanceActionV1::ArchiveRegistration,
            MusubiGovernanceRejectionReasonV1::StaleRevision,
        );
        metrics.set_storage_usage(90, 100);
        let mut encoded = Vec::new();
        TextEncoder::new()
            .encode(&registry.gather(), &mut encoded)
            .expect("encode Musubi metrics");
        let encoded = String::from_utf8(encoded).expect("Prometheus output is UTF-8");
        for expected in [
            "musubi_publication_phase_age_seconds{phase=\"readback\"} 31",
            "musubi_replication_shortfall_releases 4",
            "musubi_ingest_deadletters_total{reason=\"receipt_expired\"} 1",
            "musubi_integrity_failures_total{surface=\"provider_readback\"} 1",
            "musubi_cache_corruption_total{operation=\"verify\"} 1",
            "musubi_cursor_failures_total{reason=\"stale_revision\"} 1",
            "musubi_governance_rejections_total{action=\"set_role\",reason=\"unauthorized\"} 1",
            "musubi_governance_rejections_total{action=\"namespace_binding\",reason=\"unauthorized\"} 1",
            "musubi_governance_rejections_total{action=\"archive_registration\",reason=\"stale_revision\"} 1",
            "musubi_storage_bytes_used 90",
            "musubi_storage_bytes_capacity 100",
        ] {
            assert!(encoded.contains(expected), "missing metric: {expected}");
        }
        for forbidden in [
            "package=",
            "namespace=",
            "account=",
            "archive_id=",
            "provider_id=",
            "operation_id=",
            "url=",
        ] {
            assert!(!encoded.contains(forbidden), "forbidden label: {forbidden}");
        }
    }
}
