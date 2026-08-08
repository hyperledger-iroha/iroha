//! Bounded authenticated `SoraFS` archive fetching for exact Musubi lock nodes.
//!
//! The registry supplies finalized archive commitments and renewable provider
//! locations. A runtime transport supplies only bounded storage plans and
//! authenticated CAR readers; it never returns credentials to this module.
//! Every successful stream still crosses [`MusubiCache::install`], so provider
//! authentication cannot replace commitment, CAR, `PoR`, bundle, or source-tree
//! verification.

use std::{
    collections::BTreeSet,
    error::Error,
    fmt,
    io::{self, Read},
    path::{Path, PathBuf},
};

use iroha_data_model::{
    ChainId,
    musubi::{
        ArchiveId, MUSUBI_MAX_ARCHIVE_LOCATIONS_V1, MUSUBI_MIN_HEALTHY_REPLICAS_V1,
        MusubiArchiveCommitmentV1, MusubiArchiveLocationIdV1, MusubiArchiveLocationQueryV1,
        MusubiArchiveLocationStateV1, MusubiArchiveLocationV1, MusubiPageRequestV1,
        MusubiRegistrySnapshotV1,
    },
    sorafs::{capacity::ProviderId, pin_registry::ManifestDigest},
};
use sorafs_car::{
    CarBuildPlan, CarStreamingWriter, CarWriteError, ProfileId, compute_chunk_plan_digest_sha3,
};

use crate::{
    cache::{CacheError, InstallOutcome, MusubiCache},
    registry::{RegistryFailureClassV1, RegistryReadClientV1},
};

const RELEASE_PATH: &str = ".musubi/semantic-release.norito";
const DESCRIPTOR_PATH: &str = ".musubi/artifact-descriptor.norito";
const VERIFICATION_LOCK_PATH: &str = ".musubi/verification-lock.norito";

/// Stable classification for a secret-redacted archive fetch failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveFetchFailureClassV1 {
    /// Repeating the exact authenticated request may succeed.
    Retryable,
    /// The selected provider returned bytes or metadata that failed integrity checks.
    Integrity,
    /// No finalized current location can serve the exact archive.
    Unavailable,
    /// Configuration, authoritative evidence, or local state must change.
    Permanent,
}

/// Closed integrity surface reported by the authenticated consumer fetch path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveFetchIntegritySurfaceV1 {
    /// Provider manifest, plan, CAR, or chunk evidence violated the exact archive commitment.
    ArchiveCommitment,
    /// Authenticated control evidence failed outside the immutable archive commitment.
    Other,
}

/// Deployment-owned observer for authoritative consumer-fetch integrity attempts.
///
/// The one-shot CLI intentionally installs no observer. A long-lived host may map
/// these closed values to its telemetry registry without exposing package,
/// provider, archive, URL, token, or raw-error labels. Implementations must not
/// block, panic, or affect fetch selection.
pub trait ArchiveFetchIntegrityObserverV1: Send + Sync {
    /// Record one failed provider attempt admitted into deterministic failover.
    fn record_integrity_failure(&self, surface: ArchiveFetchIntegritySurfaceV1);
}

/// Stable archive fetch error that never contains endpoints, tokens, or response bodies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchiveFetchErrorV1 {
    class: ArchiveFetchFailureClassV1,
    code: &'static str,
}

impl ArchiveFetchErrorV1 {
    const fn new(class: ArchiveFetchFailureClassV1, code: &'static str) -> Self {
        Self { class, code }
    }

    /// Return the retry and integrity classification.
    #[must_use]
    pub const fn class(self) -> ArchiveFetchFailureClassV1 {
        self.class
    }

    /// Return the stable public error code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        self.code
    }
}

impl fmt::Display for ArchiveFetchErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code)
    }
}

impl Error for ArchiveFetchErrorV1 {}

/// Stable transport-only failure returned by an authenticated `SoraFS` runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchiveTransportErrorV1 {
    class: ArchiveFetchFailureClassV1,
    code: &'static str,
    integrity_surface: Option<ArchiveFetchIntegritySurfaceV1>,
}

impl ArchiveTransportErrorV1 {
    /// Construct a retryable transport failure.
    #[must_use]
    pub const fn retryable(code: &'static str) -> Self {
        Self::new(ArchiveFetchFailureClassV1::Retryable, code, None)
    }

    /// Construct a provider-integrity failure.
    #[must_use]
    pub const fn integrity(code: &'static str) -> Self {
        Self::new(
            ArchiveFetchFailureClassV1::Integrity,
            code,
            Some(ArchiveFetchIntegritySurfaceV1::ArchiveCommitment),
        )
    }

    /// Construct an authenticated control-integrity failure outside the archive commitment.
    #[must_use]
    pub const fn other_integrity(code: &'static str) -> Self {
        Self::new(
            ArchiveFetchFailureClassV1::Integrity,
            code,
            Some(ArchiveFetchIntegritySurfaceV1::Other),
        )
    }

    /// Construct an unavailable provider/archive failure.
    #[must_use]
    pub const fn unavailable(code: &'static str) -> Self {
        Self::new(ArchiveFetchFailureClassV1::Unavailable, code, None)
    }

    /// Construct a permanent transport/configuration failure.
    #[must_use]
    pub const fn permanent(code: &'static str) -> Self {
        Self::new(ArchiveFetchFailureClassV1::Permanent, code, None)
    }

    const fn new(
        class: ArchiveFetchFailureClassV1,
        code: &'static str,
        integrity_surface: Option<ArchiveFetchIntegritySurfaceV1>,
    ) -> Self {
        let code = if stable_code(code) {
            code
        } else {
            "MUSUBI_ARCHIVE_TRANSPORT_FAILED"
        };
        Self {
            class,
            code,
            integrity_surface,
        }
    }

    /// Return the transport retry classification.
    #[must_use]
    pub const fn class(self) -> ArchiveFetchFailureClassV1 {
        self.class
    }

    /// Return the stable redacted code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        self.code
    }

    /// Return the typed integrity surface without classifying the public code.
    #[must_use]
    pub const fn integrity_surface(self) -> Option<ArchiveFetchIntegritySurfaceV1> {
        self.integrity_surface
    }
}

impl fmt::Display for ArchiveTransportErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code)
    }
}

impl Error for ArchiveTransportErrorV1 {}

/// Runtime boundary for paged storage-plan reads and authenticated provider CAR streams.
pub trait AuthenticatedSorafsArchiveTransportV1 {
    /// Load the complete bounded storage plan from the assigned provider.
    ///
    /// Implementations must page `/v1/sorafs/storage/plan/{manifest_id}` until
    /// the exact declared file and chunk counts are present, reject duplicates
    /// or inconsistent pages, bind `manifest_id` to `pin_manifest`, and enforce
    /// the supplied commitment bounds before allocating.
    ///
    /// # Errors
    ///
    /// Returns a redacted transport error when the assigned provider's complete
    /// bounded plan cannot be loaded or authenticated.
    fn storage_plan(
        &mut self,
        pin_manifest: &ManifestDigest,
        provider: ProviderId,
        commitment: &MusubiArchiveCommitmentV1,
    ) -> Result<CarBuildPlan, ArchiveTransportErrorV1>;

    /// Open an authenticated, timeout-bounded CAR stream for one assigned provider.
    ///
    /// Stream tokens and provider endpoints remain runtime-only. Implementations
    /// must use the provider-advertised HTTPS origin, disable redirects and
    /// proxies, pin a public DNS resolution for the request lifetime, bind the
    /// token to the provider, manifest, and chunker, and cap response reads at
    /// `commitment.car_size + 1` bytes. The extra byte lets the cache reject
    /// trailing data.
    ///
    /// # Errors
    ///
    /// Returns a redacted transport error when an authenticated, bounded stream
    /// cannot be opened for the exact provider, manifest, commitment, and plan.
    fn open_authenticated_car(
        &mut self,
        pin_manifest: &ManifestDigest,
        provider: ProviderId,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &CarBuildPlan,
    ) -> Result<Box<dyn Read + Send + 'static>, ArchiveTransportErrorV1>;

    /// Consume a transport failure raised after the returned reader was opened.
    ///
    /// The default is appropriate for transports whose readers cannot fail for
    /// network reasons. Production transports use this side channel only after
    /// the reader has been dropped, preserving retryable provider failover
    /// without exposing response bodies or credentials through [`std::io::Error`].
    fn take_stream_failure(&mut self) -> Option<ArchiveTransportErrorV1> {
        None
    }
}

/// Fail-closed transport used when provider discovery/token/stream services are absent.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnavailableSorafsArchiveTransportV1;

impl AuthenticatedSorafsArchiveTransportV1 for UnavailableSorafsArchiveTransportV1 {
    fn storage_plan(
        &mut self,
        _pin_manifest: &ManifestDigest,
        _provider: ProviderId,
        _commitment: &MusubiArchiveCommitmentV1,
    ) -> Result<CarBuildPlan, ArchiveTransportErrorV1> {
        // TODO: Connect bounded paged storage-plan retrieval to finalized provider adverts.
        Err(ArchiveTransportErrorV1::permanent(
            "SORAFS_ARCHIVE_PLAN_SERVICE_NOT_CONFIGURED",
        ))
    }

    fn open_authenticated_car(
        &mut self,
        _pin_manifest: &ManifestDigest,
        _provider: ProviderId,
        _commitment: &MusubiArchiveCommitmentV1,
        _plan: &CarBuildPlan,
    ) -> Result<Box<dyn Read + Send + 'static>, ArchiveTransportErrorV1> {
        // TODO: Connect the bounded streaming gateway to finalized provider adverts.
        Err(ArchiveTransportErrorV1::permanent(
            "SORAFS_ARCHIVE_STREAM_SERVICE_NOT_CONFIGURED",
        ))
    }
}

/// Production authenticated `SoraFS` archive transport.
pub type ProductionSorafsArchiveTransportV1 =
    iroha::musubi_archive_fetch::AuthenticatedMusubiArchiveFetchClientV1;

/// Parsed secret-free fetch configuration that defers tokens, DNS, and HTTP clients.
pub type PreparedProductionSorafsArchiveTransportV1 =
    iroha::musubi_archive_fetch::PreparedMusubiArchiveFetchConfigV1;

/// Parse the fetch subtree from the same bounded `client.toml` image used by registry reads.
///
/// # Errors
/// Returns a stable redacted configuration error without opening token files or contacting DNS.
pub fn prepare_production_archive_transport_v1(
    config_path: &Path,
    config_bytes: &[u8],
) -> Result<PreparedProductionSorafsArchiveTransportV1, ArchiveTransportErrorV1> {
    PreparedProductionSorafsArchiveTransportV1::from_platform_config_bytes(
        config_path,
        config_bytes,
    )
    .map_err(runtime_error)
}

/// Materialize a prepared fetch configuration after the immutable cache reports a miss.
///
/// # Errors
/// Returns a stable redacted configuration error when runtime token, DNS, or client setup fails.
pub fn build_production_archive_transport_v1(
    prepared: &PreparedProductionSorafsArchiveTransportV1,
) -> Result<ProductionSorafsArchiveTransportV1, ArchiveTransportErrorV1> {
    prepared.build_client().map_err(runtime_error)
}

/// Load the signer-free production archive transport from `client.toml`.
///
/// Only `[musubi.fetch]` is admitted. Account keys, identities, mutation
/// credentials, environment variables, project manifests, and argv secrets are
/// never parsed by this boundary.
///
/// # Errors
/// Returns a stable redacted configuration error.
pub fn load_production_archive_transport_v1(
    config: Option<&Path>,
) -> Result<ProductionSorafsArchiveTransportV1, ArchiveTransportErrorV1> {
    let path = config.map_or_else(|| PathBuf::from("client.toml"), Path::to_path_buf);
    ProductionSorafsArchiveTransportV1::load_platform_file(&path).map_err(runtime_error)
}

impl AuthenticatedSorafsArchiveTransportV1 for ProductionSorafsArchiveTransportV1 {
    fn storage_plan(
        &mut self,
        pin_manifest: &ManifestDigest,
        provider: ProviderId,
        commitment: &MusubiArchiveCommitmentV1,
    ) -> Result<CarBuildPlan, ArchiveTransportErrorV1> {
        iroha::musubi_archive_fetch::AuthenticatedMusubiArchiveFetchClientV1::storage_plan(
            self,
            pin_manifest,
            provider,
            commitment,
        )
        .map_err(runtime_error)
    }

    fn open_authenticated_car(
        &mut self,
        pin_manifest: &ManifestDigest,
        provider: ProviderId,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &CarBuildPlan,
    ) -> Result<Box<dyn Read + Send + 'static>, ArchiveTransportErrorV1> {
        iroha::musubi_archive_fetch::AuthenticatedMusubiArchiveFetchClientV1::open_authenticated_car(
            self,
            pin_manifest,
            provider,
            commitment,
            plan,
        )
        .map_err(runtime_error)
    }

    fn take_stream_failure(&mut self) -> Option<ArchiveTransportErrorV1> {
        iroha::musubi_archive_fetch::AuthenticatedMusubiArchiveFetchClientV1::take_stream_failure(
            self,
        )
        .map(runtime_error)
    }
}

/// Exact cache-install result and finalized provider evidence used for the fetch.
#[derive(Debug)]
pub struct ArchiveFetchOutcomeV1 {
    /// Commitment-derived archive identity installed or found in cache.
    pub archive_id: ArchiveId,
    /// Finalized renewable location selected deterministically.
    pub location_id: MusubiArchiveLocationIdV1,
    /// Assigned provider that supplied the accepted plan/CAR.
    pub provider: ProviderId,
    /// Immutable cache publication result.
    pub cache: InstallOutcome,
}

/// Exact finalized archive commitment and provider plan selected without reading CAR bytes.
#[derive(Clone, Debug)]
pub struct PreparedArchivePlanV1 {
    /// Commitment-derived archive identity.
    pub archive_id: ArchiveId,
    /// Exact finalized commitment validated from the registry page.
    pub commitment: MusubiArchiveCommitmentV1,
    /// Finalized renewable location selected deterministically.
    pub location_id: MusubiArchiveLocationIdV1,
    /// Exact renewable `SoraFS` pin manifest.
    pub pin_manifest: ManifestDigest,
    /// Assigned provider whose plan was accepted.
    pub provider: ProviderId,
    /// Complete canonical plan bound to the commitment.
    pub plan: CarBuildPlan,
}

#[derive(Clone, Copy, Debug)]
struct ArchiveCandidateV1 {
    rank: u8,
    location_id: MusubiArchiveLocationIdV1,
    pin_manifest: ManifestDigest,
    provider: ProviderId,
}

/// Fetch adapter joining finalized registry evidence to the immutable cache boundary.
#[derive(Clone)]
pub struct MusubiArchiveFetchAdapterV1<'client> {
    registry: &'client RegistryReadClientV1,
    cache: &'client MusubiCache,
    integrity_observer: Option<&'client dyn ArchiveFetchIntegrityObserverV1>,
    expected_deployment: Option<ArchiveFetchDeploymentBindingV1>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ArchiveFetchDeploymentBindingV1 {
    chain_id: ChainId,
    genesis_hash: [u8; 32],
    minimum_snapshot: MusubiRegistrySnapshotV1,
}

impl fmt::Debug for MusubiArchiveFetchAdapterV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MusubiArchiveFetchAdapterV1")
            .field("integrity_observer", &self.integrity_observer.is_some())
            .finish_non_exhaustive()
    }
}

impl<'client> MusubiArchiveFetchAdapterV1<'client> {
    /// Bind a signer-free finalized registry reader and an explicit user cache.
    #[must_use]
    pub const fn new(registry: &'client RegistryReadClientV1, cache: &'client MusubiCache) -> Self {
        Self {
            registry,
            cache,
            integrity_observer: None,
            expected_deployment: None,
        }
    }

    /// Bind finalized archive-location evidence to an exact lock deployment and minimum snapshot.
    ///
    /// Locked graph fetches use this boundary so a changed endpoint cannot supply provider
    /// authorization from another chain or from a finalized view older than the graph anchor.
    /// Maintenance commands whose configured registry is itself the authority may remain unbound.
    #[must_use]
    pub fn with_expected_deployment(
        mut self,
        chain_id: &ChainId,
        genesis_hash: [u8; 32],
        minimum_snapshot: MusubiRegistrySnapshotV1,
    ) -> Self {
        self.expected_deployment = Some(ArchiveFetchDeploymentBindingV1 {
            chain_id: chain_id.clone(),
            genesis_hash,
            minimum_snapshot,
        });
        self
    }

    /// Attach the long-lived host's bounded integrity observer.
    #[must_use]
    pub const fn with_integrity_observer(
        mut self,
        observer: &'client dyn ArchiveFetchIntegrityObserverV1,
    ) -> Self {
        self.integrity_observer = Some(observer);
        self
    }

    /// Fetch one exact archive with deterministic location/provider failover.
    ///
    /// Healthy locations must still have the V1 quorum. Degraded locations are
    /// accepted only for exact locked fetches, preserving already-locked builds
    /// while at least one attested provider remains. Pending and retired
    /// locations are never used. Within each health rank, distinct providers
    /// are tried before an alternate pin assigned to an already-tried provider.
    ///
    /// # Errors
    ///
    /// Returns a stable redacted error when finalized registry evidence is
    /// unavailable or invalid, every provider attempt fails, or cache
    /// verification or publication cannot be completed safely.
    #[expect(
        clippy::too_many_lines,
        reason = "the fail-closed fetch state machine keeps provider attempt, cache verification, stream fallback, and error precedence visible together"
    )]
    pub fn fetch_exact(
        &self,
        archive_id: ArchiveId,
        transport: &mut dyn AuthenticatedSorafsArchiveTransportV1,
    ) -> Result<ArchiveFetchOutcomeV1, ArchiveFetchErrorV1> {
        let (commitment, candidates) = self.finalized_candidates(archive_id)?;

        let mut retryable = None;
        let mut integrity = None;
        let mut permanent = None;
        for candidate in candidates {
            let plan = match transport.storage_plan(
                &candidate.pin_manifest,
                candidate.provider,
                &commitment,
            ) {
                Ok(plan) => match validate_transport_plan(&commitment, &plan) {
                    Ok(()) => plan,
                    Err(error) => {
                        record_transport_failure(
                            self.integrity_observer,
                            error,
                            &mut retryable,
                            &mut integrity,
                            &mut permanent,
                        );
                        continue;
                    }
                },
                Err(error) => {
                    record_transport_failure(
                        self.integrity_observer,
                        error,
                        &mut retryable,
                        &mut integrity,
                        &mut permanent,
                    );
                    continue;
                }
            };
            match self.cache.verify(&commitment, &plan) {
                Ok(entry) => {
                    return Ok(ArchiveFetchOutcomeV1 {
                        archive_id,
                        location_id: candidate.location_id,
                        provider: candidate.provider,
                        cache: InstallOutcome::AlreadyPresent(entry),
                    });
                }
                Err(CacheError::Io { source, .. })
                    if source.kind() == std::io::ErrorKind::NotFound => {}
                Err(CacheError::InvalidPlan(_)) => {
                    record_transport_failure(
                        self.integrity_observer,
                        ArchiveTransportErrorV1::integrity(
                            "SORAFS_ARCHIVE_PLAN_COMMITMENT_MISMATCH",
                        ),
                        &mut retryable,
                        &mut integrity,
                        &mut permanent,
                    );
                    continue;
                }
                Err(
                    CacheError::InvalidArchive(_)
                    | CacheError::CorruptEntry(_)
                    | CacheError::UnsafeDescendant(_),
                ) => {
                    return Err(ArchiveFetchErrorV1::new(
                        ArchiveFetchFailureClassV1::Permanent,
                        "MUSUBI_CACHE_CORRUPT",
                    ));
                }
                Err(
                    CacheError::UnsupportedPlatform
                    | CacheError::Io { .. }
                    | CacheError::UnsafeRoot(_),
                ) => {
                    return Err(ArchiveFetchErrorV1::new(
                        ArchiveFetchFailureClassV1::Permanent,
                        "MUSUBI_CACHE_READ_FAILED",
                    ));
                }
            }
            let reader = match transport.open_authenticated_car(
                &candidate.pin_manifest,
                candidate.provider,
                &commitment,
                &plan,
            ) {
                Ok(reader) => reader,
                Err(error) => {
                    record_transport_failure(
                        self.integrity_observer,
                        error,
                        &mut retryable,
                        &mut integrity,
                        &mut permanent,
                    );
                    continue;
                }
            };
            let bounded = reader.take(commitment.car_size.saturating_add(1));
            match self.cache.install(&commitment, &plan, bounded) {
                Ok(cache) => {
                    return Ok(ArchiveFetchOutcomeV1 {
                        archive_id,
                        location_id: candidate.location_id,
                        provider: candidate.provider,
                        cache,
                    });
                }
                Err(CacheError::InvalidPlan(_) | CacheError::InvalidArchive(_)) => {
                    if let Some(error) = transport.take_stream_failure() {
                        record_transport_failure(
                            self.integrity_observer,
                            error,
                            &mut retryable,
                            &mut integrity,
                            &mut permanent,
                        );
                    } else {
                        record_transport_failure(
                            self.integrity_observer,
                            ArchiveTransportErrorV1::integrity(
                                "SORAFS_ARCHIVE_COMMITMENT_MISMATCH",
                            ),
                            &mut retryable,
                            &mut integrity,
                            &mut permanent,
                        );
                    }
                }
                Err(CacheError::CorruptEntry(_) | CacheError::UnsafeDescendant(_)) => {
                    return Err(ArchiveFetchErrorV1::new(
                        ArchiveFetchFailureClassV1::Permanent,
                        "MUSUBI_CACHE_CORRUPT",
                    ));
                }
                Err(
                    CacheError::UnsupportedPlatform
                    | CacheError::Io { .. }
                    | CacheError::UnsafeRoot(_),
                ) => {
                    return Err(ArchiveFetchErrorV1::new(
                        ArchiveFetchFailureClassV1::Permanent,
                        "MUSUBI_CACHE_WRITE_FAILED",
                    ));
                }
            }
        }

        Err(integrity
            .or(retryable)
            .or(permanent)
            .map_or_else(archive_unavailable, transport_error))
    }

    /// Select and validate one exact finalized provider plan without reading CAR bytes.
    ///
    /// This shares the same registry/location/attestation and transport-plan trust boundary as
    /// [`Self::fetch_exact`], allowing cache verification and repair to avoid untrusted local
    /// plans. The chunk plan, registered profile, bundle metadata inventory, and file-plan-derived
    /// root CID are checked without reading CAR body bytes. Providers are tried in deterministic
    /// healthy-then-degraded order.
    ///
    /// # Errors
    ///
    /// Returns a stable redacted error when finalized registry evidence is
    /// unavailable or invalid, or no provider supplies an authenticated plan
    /// matching the exact archive commitment.
    pub fn prepare_exact(
        &self,
        archive_id: ArchiveId,
        transport: &mut dyn AuthenticatedSorafsArchiveTransportV1,
    ) -> Result<PreparedArchivePlanV1, ArchiveFetchErrorV1> {
        let (commitment, candidates) = self.finalized_candidates(archive_id)?;
        let mut retryable = None;
        let mut integrity = None;
        let mut permanent = None;
        for candidate in candidates {
            match transport.storage_plan(&candidate.pin_manifest, candidate.provider, &commitment) {
                Ok(plan) if validate_transport_plan(&commitment, &plan).is_ok() => {
                    return Ok(PreparedArchivePlanV1 {
                        archive_id,
                        commitment,
                        location_id: candidate.location_id,
                        pin_manifest: candidate.pin_manifest,
                        provider: candidate.provider,
                        plan,
                    });
                }
                Ok(_) => {
                    record_transport_failure(
                        self.integrity_observer,
                        ArchiveTransportErrorV1::integrity(
                            "SORAFS_ARCHIVE_PLAN_COMMITMENT_MISMATCH",
                        ),
                        &mut retryable,
                        &mut integrity,
                        &mut permanent,
                    );
                }
                Err(error) => {
                    record_transport_failure(
                        self.integrity_observer,
                        error,
                        &mut retryable,
                        &mut integrity,
                        &mut permanent,
                    );
                }
            }
        }
        Err(integrity
            .or(retryable)
            .or(permanent)
            .map_or_else(archive_unavailable, transport_error))
    }

    fn finalized_candidates(
        &self,
        archive_id: ArchiveId,
    ) -> Result<(MusubiArchiveCommitmentV1, Vec<ArchiveCandidateV1>), ArchiveFetchErrorV1> {
        if archive_id.is_zero() {
            return Err(invalid_evidence());
        }
        let query = MusubiArchiveLocationQueryV1 {
            archive_id,
            page: MusubiPageRequestV1 {
                limit: u32::try_from(MUSUBI_MAX_ARCHIVE_LOCATIONS_V1)
                    .expect("archive-location bound fits u32"),
                cursor: None,
            },
        };
        let page = self
            .registry
            .archive_locations(&query)
            .map_err(registry_error)?
            .ok_or_else(archive_unavailable)?;
        if let Some(expected) = &self.expected_deployment {
            validate_deployment_binding(
                expected,
                &page.chain_id,
                page.genesis_hash,
                page.snapshot,
            )?;
        }
        // The V1 directory contains at most four locations and this first-page request asks for
        // all four. A cursor or a shorter/different identity list is therefore incomplete rather
        // than a legitimate continuation.
        if page.next_cursor.is_some()
            || page.archive.archive_id != archive_id
            || page.archive.commitment.archive_id() != archive_id
            || page.archive.location_ids.len() != page.items.len()
            || page
                .archive
                .location_ids
                .iter()
                .zip(&page.items)
                .any(|(expected, location)| *expected != location.location_id)
        {
            return Err(invalid_evidence());
        }
        let commitment = page.archive.commitment.clone();
        commitment.validate().map_err(|_| invalid_evidence())?;
        let mut candidates = Vec::new();
        for location in &page.items {
            validate_location_evidence(
                page.archive.archive_id,
                page.snapshot.finalized_height,
                location,
            )?;
            let rank = match location.state {
                MusubiArchiveLocationStateV1::Healthy => {
                    if location.providers.len() < usize::from(MUSUBI_MIN_HEALTHY_REPLICAS_V1) {
                        return Err(invalid_evidence());
                    }
                    0_u8
                }
                MusubiArchiveLocationStateV1::Degraded => 1,
                MusubiArchiveLocationStateV1::Pending => continue,
                MusubiArchiveLocationStateV1::Retired => return Err(invalid_evidence()),
            };
            for provider in &location.providers {
                candidates.push(ArchiveCandidateV1 {
                    rank,
                    location_id: location.location_id,
                    pin_manifest: location.pin_manifest,
                    provider: *provider,
                });
            }
        }
        let candidates = prioritize_distinct_providers(candidates);
        if candidates.is_empty() {
            return Err(archive_unavailable());
        }
        Ok((commitment, candidates))
    }
}

fn validate_deployment_binding(
    expected: &ArchiveFetchDeploymentBindingV1,
    observed_chain_id: &ChainId,
    observed_genesis_hash: [u8; 32],
    observed_snapshot: MusubiRegistrySnapshotV1,
) -> Result<(), ArchiveFetchErrorV1> {
    let minimum = expected.minimum_snapshot;
    if expected.genesis_hash.iter().all(|byte| *byte == 0)
        || minimum.validate().is_err()
        || observed_snapshot.validate().is_err()
        || observed_chain_id != &expected.chain_id
        || observed_genesis_hash != expected.genesis_hash
        || observed_snapshot.finalized_height < minimum.finalized_height
        || observed_snapshot.index_revision < minimum.index_revision
        || (observed_snapshot.finalized_height == minimum.finalized_height
            && observed_snapshot != minimum)
    {
        return Err(invalid_evidence());
    }
    Ok(())
}

fn validate_location_evidence(
    expected_archive: ArchiveId,
    snapshot_height: u64,
    location: &MusubiArchiveLocationV1,
) -> Result<(), ArchiveFetchErrorV1> {
    location.validate().map_err(|_| invalid_evidence())?;
    if location.archive_id != expected_archive || location.finalized_height > snapshot_height {
        return Err(invalid_evidence());
    }
    Ok(())
}

fn prioritize_distinct_providers(
    mut candidates: Vec<ArchiveCandidateV1>,
) -> Vec<ArchiveCandidateV1> {
    candidates.sort_by_key(|candidate| (candidate.rank, candidate.location_id, candidate.provider));
    let mut ordered = Vec::with_capacity(candidates.len());
    let mut seen = BTreeSet::new();
    for rank in [0_u8, 1_u8] {
        let mut repeated = Vec::new();
        for candidate in candidates
            .iter()
            .copied()
            .filter(|candidate| candidate.rank == rank)
        {
            if seen.insert(candidate.provider) {
                ordered.push(candidate);
            } else {
                repeated.push(candidate);
            }
        }
        ordered.extend(repeated);
    }
    ordered
}

fn validate_transport_plan(
    commitment: &MusubiArchiveCommitmentV1,
    plan: &CarBuildPlan,
) -> Result<(), ArchiveTransportErrorV1> {
    let invalid = || ArchiveTransportErrorV1::integrity("SORAFS_ARCHIVE_PLAN_COMMITMENT_MISMATCH");
    commitment.validate().map_err(|_| invalid())?;
    plan.validate().map_err(|_| invalid())?;
    if plan.content_length != commitment.content_length
        || plan.chunks.len() != usize::try_from(commitment.chunk_count).unwrap_or(usize::MAX)
        || plan
            .chunks
            .iter()
            .any(|chunk| chunk.taikai_segment_hint.is_some())
        || compute_chunk_plan_digest_sha3(&plan.chunks) != *commitment.chunk_plan_digest.as_bytes()
    {
        return Err(invalid());
    }
    let descriptor = sorafs_car::chunker_registry::lookup(ProfileId(commitment.chunker.profile_id))
        .ok_or_else(invalid)?;
    if descriptor.namespace != commitment.chunker.namespace
        || descriptor.name != commitment.chunker.name
        || descriptor.semver != commitment.chunker.semver
        || descriptor.multihash_code != commitment.chunker.multihash_code
        || descriptor.profile != plan.chunk_profile
    {
        return Err(invalid());
    }

    let mut source_count = 0_usize;
    let mut release_count = 0_u8;
    let mut descriptor_count = 0_u8;
    let mut lock_count = 0_u8;
    for file in &plan.files {
        match file.path.join("/").as_str() {
            RELEASE_PATH => release_count = release_count.saturating_add(1),
            DESCRIPTOR_PATH => descriptor_count = descriptor_count.saturating_add(1),
            VERIFICATION_LOCK_PATH => lock_count = lock_count.saturating_add(1),
            path if path.starts_with(".musubi/") => return Err(invalid()),
            _ => source_count = source_count.saturating_add(1),
        }
    }
    if source_count != usize::try_from(commitment.file_count).unwrap_or(usize::MAX)
        || release_count != 1
        || descriptor_count != 1
        || lock_count != 1
    {
        return Err(invalid());
    }
    // `CarStreamingWriter` computes the complete file/directory DAG and compares expected roots
    // before reading a body byte. Every valid Musubi bundle is non-empty, so a matching root must
    // reach this empty reader and fail with `UnexpectedEof`; a root mismatch fails earlier. Plan
    // validation and the exact registered profile cap the reserved probe buffer at the profile's
    // maximum chunk size.
    let roots = vec![commitment.root_cid.as_bytes().to_vec()];
    let mut empty = io::empty();
    match CarStreamingWriter::with_expected_roots(plan, roots)
        .write_from_reader(&mut empty, io::sink())
    {
        Err(CarWriteError::Io(source)) if source.kind() == io::ErrorKind::UnexpectedEof => {}
        Ok(_) | Err(_) => return Err(invalid()),
    }
    Ok(())
}

fn record_transport_failure(
    observer: Option<&dyn ArchiveFetchIntegrityObserverV1>,
    error: ArchiveTransportErrorV1,
    retryable: &mut Option<ArchiveTransportErrorV1>,
    integrity: &mut Option<ArchiveTransportErrorV1>,
    permanent: &mut Option<ArchiveTransportErrorV1>,
) {
    match error.class() {
        ArchiveFetchFailureClassV1::Retryable => *retryable = Some(error),
        ArchiveFetchFailureClassV1::Integrity => {
            if let (Some(observer), Some(surface)) = (observer, error.integrity_surface()) {
                observer.record_integrity_failure(surface);
            }
            *integrity = Some(error);
        }
        ArchiveFetchFailureClassV1::Permanent | ArchiveFetchFailureClassV1::Unavailable => {
            *permanent = Some(error);
        }
    }
}

fn registry_error(error: crate::registry::RegistryErrorV1) -> ArchiveFetchErrorV1 {
    let class = match error.class() {
        RegistryFailureClassV1::Retryable => ArchiveFetchFailureClassV1::Retryable,
        RegistryFailureClassV1::NotFound => ArchiveFetchFailureClassV1::Unavailable,
        RegistryFailureClassV1::Permanent | RegistryFailureClassV1::StaleCursor => {
            ArchiveFetchFailureClassV1::Permanent
        }
    };
    ArchiveFetchErrorV1::new(class, error.code())
}

const fn transport_error(error: ArchiveTransportErrorV1) -> ArchiveFetchErrorV1 {
    ArchiveFetchErrorV1::new(error.class(), error.code())
}

fn runtime_error(
    error: iroha::musubi_archive_fetch::MusubiArchiveRuntimeErrorV1,
) -> ArchiveTransportErrorV1 {
    use iroha::musubi_archive_fetch::{
        MusubiArchiveRuntimeFailureClassV1, MusubiArchiveRuntimeIntegritySurfaceV1,
    };

    match error.class() {
        MusubiArchiveRuntimeFailureClassV1::Retryable => {
            ArchiveTransportErrorV1::retryable(error.code())
        }
        MusubiArchiveRuntimeFailureClassV1::Integrity => match error.integrity_surface() {
            Some(MusubiArchiveRuntimeIntegritySurfaceV1::ArchiveCommitment) => {
                ArchiveTransportErrorV1::integrity(error.code())
            }
            Some(MusubiArchiveRuntimeIntegritySurfaceV1::Other) | None => {
                ArchiveTransportErrorV1::other_integrity(error.code())
            }
        },
        MusubiArchiveRuntimeFailureClassV1::Unavailable => {
            ArchiveTransportErrorV1::unavailable(error.code())
        }
        MusubiArchiveRuntimeFailureClassV1::Permanent => {
            ArchiveTransportErrorV1::permanent(error.code())
        }
    }
}

const fn archive_unavailable() -> ArchiveFetchErrorV1 {
    ArchiveFetchErrorV1::new(
        ArchiveFetchFailureClassV1::Unavailable,
        "MUSUBI_ARCHIVE_UNAVAILABLE",
    )
}

const fn invalid_evidence() -> ArchiveFetchErrorV1 {
    ArchiveFetchErrorV1::new(
        ArchiveFetchFailureClassV1::Permanent,
        "MUSUBI_ARCHIVE_LOCATION_EVIDENCE_INVALID",
    )
}

const fn stable_code(code: &str) -> bool {
    let bytes = code.as_bytes();
    if bytes.is_empty() || bytes.len() > 96 {
        return false;
    }
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if !(byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_') {
            return false;
        }
        index += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    use iroha_data_model::{
        musubi::{MusubiContentDigestV1, MusubiProviderBundleAttestationSetDigestV1},
        sorafs::pin_registry::{ChunkerProfileHandle, ManifestRootCid, ReplicationOrderId},
    };
    use sorafs_car::FileEntry;

    use super::*;

    #[derive(Default)]
    struct CountingIntegrityObserver {
        archive_commitment: AtomicUsize,
        other: AtomicUsize,
    }

    impl ArchiveFetchIntegrityObserverV1 for CountingIntegrityObserver {
        fn record_integrity_failure(&self, surface: ArchiveFetchIntegritySurfaceV1) {
            match surface {
                ArchiveFetchIntegritySurfaceV1::ArchiveCommitment => {
                    self.archive_commitment.fetch_add(1, Ordering::SeqCst);
                }
                ArchiveFetchIntegritySurfaceV1::Other => {
                    self.other.fetch_add(1, Ordering::SeqCst);
                }
            }
        }
    }

    fn commitment() -> MusubiArchiveCommitmentV1 {
        MusubiArchiveCommitmentV1 {
            root_cid: ManifestRootCid::from_blake3_digest([1; 32]).expect("root CID"),
            chunker: ChunkerProfileHandle {
                profile_id: 1,
                namespace: "sorafs".to_owned(),
                name: "sf1".to_owned(),
                semver: "1.0.0".to_owned(),
                multihash_code: 0x1f,
            },
            chunk_plan_digest: MusubiContentDigestV1::new([2; 32]),
            por_root: MusubiContentDigestV1::new([3; 32]),
            content_length: 4,
            car_digest: MusubiContentDigestV1::new([4; 32]),
            car_size: 128,
            bundle_digest: MusubiContentDigestV1::new([5; 32]),
            source_tree_digest: MusubiContentDigestV1::new([6; 32]),
            descriptor_digest: MusubiContentDigestV1::new([7; 32]),
            file_count: 1,
            chunk_count: 1,
        }
    }

    fn compact_location(archive_id: ArchiveId) -> MusubiArchiveLocationV1 {
        MusubiArchiveLocationV1 {
            location_id: MusubiArchiveLocationIdV1::new([0x31; 32]),
            archive_id,
            pin_manifest: ManifestDigest::new([0x32; 32]),
            replication_order: ReplicationOrderId::new([0x33; 32]),
            providers: vec![ProviderId::new([0x34; 32])],
            provider_attestation_set_digest: MusubiProviderBundleAttestationSetDigestV1::new(
                [0x35; 32],
            ),
            renew_after_epoch: 10,
            expires_at_epoch: 20,
            finalized_height: 7,
            revision: 1,
            state: MusubiArchiveLocationStateV1::Healthy,
        }
    }

    fn transport_plan_fixture() -> (MusubiArchiveCommitmentV1, CarBuildPlan) {
        let files = [
            (
                vec!["src".to_owned(), "lib.ko".to_owned()],
                b"fn main() {}".to_vec(),
            ),
            (
                RELEASE_PATH.split('/').map(str::to_owned).collect(),
                vec![1],
            ),
            (
                DESCRIPTOR_PATH.split('/').map(str::to_owned).collect(),
                vec![2],
            ),
            (
                VERIFICATION_LOCK_PATH
                    .split('/')
                    .map(str::to_owned)
                    .collect(),
                vec![3],
            ),
        ]
        .into_iter()
        .map(|(path, data)| FileEntry { path, data })
        .collect();
        let (plan, payload) = CarBuildPlan::from_files(files).expect("canonical bundle plan");
        let stats = sorafs_car::CarWriter::new(&plan, &payload)
            .expect("fixture CAR writer")
            .write_to(io::sink())
            .expect("fixture CAR stats");
        let descriptor = sorafs_car::chunker_registry::default_descriptor();
        let commitment = MusubiArchiveCommitmentV1 {
            root_cid: ManifestRootCid::try_from(stats.root_cids[0].clone())
                .expect("canonical root CID"),
            chunker: ChunkerProfileHandle {
                profile_id: descriptor.id.0,
                namespace: descriptor.namespace.to_owned(),
                name: descriptor.name.to_owned(),
                semver: descriptor.semver.to_owned(),
                multihash_code: descriptor.multihash_code,
            },
            chunk_plan_digest: MusubiContentDigestV1::new(compute_chunk_plan_digest_sha3(
                &plan.chunks,
            )),
            por_root: MusubiContentDigestV1::new(
                sorafs_car::compute_por_root(&payload, &plan).expect("fixture PoR"),
            ),
            content_length: plan.content_length,
            car_digest: MusubiContentDigestV1::new(*stats.car_archive_digest.as_bytes()),
            car_size: stats.car_size,
            bundle_digest: MusubiContentDigestV1::new([5; 32]),
            source_tree_digest: MusubiContentDigestV1::new([6; 32]),
            descriptor_digest: MusubiContentDigestV1::new([7; 32]),
            file_count: 1,
            chunk_count: u32::try_from(plan.chunks.len()).expect("fixture chunk count"),
        };
        (commitment, plan)
    }

    #[test]
    fn transport_error_codes_are_closed_and_redacted() {
        let invalid = ArchiveTransportErrorV1::retryable("token=secret");
        assert_eq!(invalid.code(), "MUSUBI_ARCHIVE_TRANSPORT_FAILED");
        assert_eq!(invalid.class(), ArchiveFetchFailureClassV1::Retryable);

        let valid = ArchiveTransportErrorV1::integrity("SORAFS_CHUNK_DIGEST_MISMATCH");
        assert_eq!(valid.code(), "SORAFS_CHUNK_DIGEST_MISMATCH");
        assert_eq!(valid.class(), ArchiveFetchFailureClassV1::Integrity);
        assert_eq!(
            valid.integrity_surface(),
            Some(ArchiveFetchIntegritySurfaceV1::ArchiveCommitment)
        );
    }

    #[test]
    fn integrity_observer_records_each_admitted_attempt_exactly_once() {
        let observer = CountingIntegrityObserver::default();
        let mut retryable = None;
        let mut integrity = None;
        let mut permanent = None;

        for error in [
            ArchiveTransportErrorV1::integrity("SORAFS_FIRST_PROVIDER_INVALID"),
            ArchiveTransportErrorV1::retryable("SORAFS_RETRYABLE"),
            ArchiveTransportErrorV1::permanent("SORAFS_PERMANENT"),
            ArchiveTransportErrorV1::integrity("SORAFS_SECOND_PROVIDER_INVALID"),
            ArchiveTransportErrorV1::other_integrity("SORAFS_TOKEN_CONTROL_INVALID"),
        ] {
            record_transport_failure(
                Some(&observer),
                error,
                &mut retryable,
                &mut integrity,
                &mut permanent,
            );
        }

        assert_eq!(observer.archive_commitment.load(Ordering::SeqCst), 2);
        assert_eq!(observer.other.load(Ordering::SeqCst), 1);
        assert!(retryable.is_some());
        assert!(integrity.is_some());
        assert!(permanent.is_some());
    }

    #[test]
    fn transport_plan_is_revalidated_before_cache_or_maintenance_use() {
        let (commitment, plan) = transport_plan_fixture();
        validate_transport_plan(&commitment, &plan).expect("exact provider plan");

        let mut substituted = plan.clone();
        substituted.chunks[0].digest[0] ^= 0xff;
        let error = validate_transport_plan(&commitment, &substituted)
            .expect_err("substituted chunk plan must fail");
        assert_eq!(error.class(), ArchiveFetchFailureClassV1::Integrity);
        assert_eq!(
            error.integrity_surface(),
            Some(ArchiveFetchIntegritySurfaceV1::ArchiveCommitment)
        );

        let mut hinted = plan.clone();
        hinted.chunks[0].taikai_segment_hint = Some(sorafs_car::TaikaiSegmentHint {
            event: "event".to_owned(),
            stream: "stream".to_owned(),
            rendition: "rendition".to_owned(),
            sequence: 0,
            payload_len: None,
            payload_digest: None,
        });
        assert!(
            validate_transport_plan(&commitment, &hinted).is_err(),
            "Musubi V1 provider plans must reject uncommitted Taikai hints"
        );

        let mut wrong_root = commitment.clone();
        wrong_root.root_cid = ManifestRootCid::from_blake3_digest([0xaa; 32]).expect("other CID");
        assert!(validate_transport_plan(&wrong_root, &plan).is_err());

        let mut substituted_files = plan;
        let source = substituted_files
            .files
            .iter_mut()
            .find(|file| file.path.join("/") == "src/lib.ko")
            .expect("source plan entry");
        source.path = vec!["src".to_owned(), "other.ko".to_owned()];
        substituted_files
            .validate()
            .expect("substituted file geometry remains structurally valid");
        assert!(validate_transport_plan(&commitment, &substituted_files).is_err());
    }

    #[test]
    fn candidate_order_prefers_distinct_providers_within_each_health_rank() {
        let candidate = |rank, location: u8, provider| ArchiveCandidateV1 {
            rank,
            location_id: MusubiArchiveLocationIdV1::new([location; 32]),
            pin_manifest: ManifestDigest::new([location.saturating_add(20); 32]),
            provider: ProviderId::new([provider; 32]),
        };
        let candidates = vec![
            candidate(1, 2, 3),
            candidate(0, 3, 2),
            candidate(0, 2, 1),
            candidate(1, 1, 1),
            candidate(0, 1, 1),
        ];
        let candidates = prioritize_distinct_providers(candidates);
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.provider)
                .collect::<Vec<_>>(),
            vec![
                ProviderId::new([1; 32]),
                ProviderId::new([2; 32]),
                ProviderId::new([1; 32]),
                ProviderId::new([3; 32]),
                ProviderId::new([1; 32]),
            ]
        );
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.rank)
                .collect::<Vec<_>>(),
            vec![0, 0, 0, 1, 1]
        );
    }

    fn deployment_binding() -> ArchiveFetchDeploymentBindingV1 {
        ArchiveFetchDeploymentBindingV1 {
            chain_id: ChainId::from("musubi-fetch-binding-test"),
            genesis_hash: [0x51; 32],
            minimum_snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 10,
                finalized_block_hash: [0x52; 32],
                index_revision: 7,
            },
        }
    }

    #[test]
    fn locked_fetch_binding_rejects_another_chain_or_genesis() {
        let expected = deployment_binding();
        for (chain_id, genesis_hash) in [
            (ChainId::from("another-musubi-chain"), expected.genesis_hash),
            (expected.chain_id.clone(), [0x53; 32]),
        ] {
            let error = validate_deployment_binding(
                &expected,
                &chain_id,
                genesis_hash,
                expected.minimum_snapshot,
            )
            .expect_err("another deployment must not authorize a locked fetch");
            assert_eq!(error.code(), "MUSUBI_ARCHIVE_LOCATION_EVIDENCE_INVALID");
            assert_eq!(error.class(), ArchiveFetchFailureClassV1::Permanent);
        }
    }

    #[test]
    fn locked_fetch_binding_rejects_a_regressing_snapshot() {
        let expected = deployment_binding();
        for snapshot in [
            MusubiRegistrySnapshotV1 {
                finalized_height: expected.minimum_snapshot.finalized_height - 1,
                finalized_block_hash: [0x54; 32],
                index_revision: expected.minimum_snapshot.index_revision,
            },
            MusubiRegistrySnapshotV1 {
                finalized_height: expected.minimum_snapshot.finalized_height + 1,
                finalized_block_hash: [0x55; 32],
                index_revision: expected.minimum_snapshot.index_revision - 1,
            },
        ] {
            let error = validate_deployment_binding(
                &expected,
                &expected.chain_id,
                expected.genesis_hash,
                snapshot,
            )
            .expect_err("a finalized view older than the lock anchor must fail");
            assert_eq!(error.code(), "MUSUBI_ARCHIVE_LOCATION_EVIDENCE_INVALID");
        }
    }

    #[test]
    fn locked_fetch_binding_rejects_a_same_height_snapshot_conflict() {
        let expected = deployment_binding();
        let conflicting = MusubiRegistrySnapshotV1 {
            finalized_block_hash: [0x56; 32],
            ..expected.minimum_snapshot
        };
        let error = validate_deployment_binding(
            &expected,
            &expected.chain_id,
            expected.genesis_hash,
            conflicting,
        )
        .expect_err("a conflicting finalized block at the anchor height must fail");
        assert_eq!(error.code(), "MUSUBI_ARCHIVE_LOCATION_EVIDENCE_INVALID");
    }

    #[test]
    fn locked_fetch_binding_rejects_an_invalid_observed_snapshot() {
        let expected = deployment_binding();
        let invalid = MusubiRegistrySnapshotV1 {
            finalized_height: expected.minimum_snapshot.finalized_height + 1,
            finalized_block_hash: [0; 32],
            index_revision: expected.minimum_snapshot.index_revision,
        };
        let error = validate_deployment_binding(
            &expected,
            &expected.chain_id,
            expected.genesis_hash,
            invalid,
        )
        .expect_err("an invalid observed snapshot must fail even when its counters do not regress");
        assert_eq!(error.code(), "MUSUBI_ARCHIVE_LOCATION_EVIDENCE_INVALID");
    }

    #[test]
    fn locked_fetch_binding_accepts_the_anchor_and_a_later_snapshot() {
        let expected = deployment_binding();
        validate_deployment_binding(
            &expected,
            &expected.chain_id,
            expected.genesis_hash,
            expected.minimum_snapshot,
        )
        .expect("the exact lock anchor is valid");
        validate_deployment_binding(
            &expected,
            &expected.chain_id,
            expected.genesis_hash,
            MusubiRegistrySnapshotV1 {
                finalized_height: expected.minimum_snapshot.finalized_height + 1,
                finalized_block_hash: [0x57; 32],
                index_revision: expected.minimum_snapshot.index_revision + 1,
            },
        )
        .expect("a non-regressing later finalized snapshot is valid");
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn fetch_adapter_records_the_expected_locked_deployment() {
        let temporary = tempfile::tempdir().expect("temporary cache root");
        let cache = MusubiCache::open(temporary.path()).expect("secure cache root");
        let registry = RegistryReadClientV1::new(
            "https://registry.example/"
                .parse()
                .expect("public registry URL"),
            Duration::from_secs(1),
            753,
        )
        .expect("signer-free registry client");
        let expected = deployment_binding();
        let adapter = MusubiArchiveFetchAdapterV1::new(&registry, &cache).with_expected_deployment(
            &expected.chain_id,
            expected.genesis_hash,
            expected.minimum_snapshot,
        );

        assert_eq!(adapter.expected_deployment.as_ref(), Some(&expected));
    }

    #[test]
    fn compact_location_evidence_binds_archive_and_finalized_height() {
        let archive_id = ArchiveId::new([0x41; 32]);
        let location = compact_location(archive_id);
        validate_location_evidence(archive_id, location.finalized_height, &location)
            .expect("matching compact finalized location");

        let error = validate_location_evidence(
            ArchiveId::new([0x42; 32]),
            location.finalized_height,
            &location,
        )
        .expect_err("a location from another archive must fail");
        assert_eq!(error.code(), "MUSUBI_ARCHIVE_LOCATION_EVIDENCE_INVALID");

        let error =
            validate_location_evidence(archive_id, location.finalized_height - 1, &location)
                .expect_err("a location newer than the query snapshot must fail");
        assert_eq!(error.code(), "MUSUBI_ARCHIVE_LOCATION_EVIDENCE_INVALID");
    }

    #[test]
    fn unavailable_transport_fails_before_opening_a_stream() {
        let mut transport = UnavailableSorafsArchiveTransportV1;
        let manifest = ManifestDigest::new([9; 32]);
        let error = transport
            .storage_plan(&manifest, ProviderId::new([8; 32]), &commitment())
            .expect_err("unconfigured plan retrieval fails closed");
        assert_eq!(error.code(), "SORAFS_ARCHIVE_PLAN_SERVICE_NOT_CONFIGURED");
        assert_eq!(error.class(), ArchiveFetchFailureClassV1::Permanent);
    }

    #[cfg(unix)]
    #[test]
    fn exact_fetch_rejects_a_zero_archive_before_network_access() {
        let temporary = tempfile::tempdir().expect("temporary cache root");
        let cache = MusubiCache::open(temporary.path()).expect("secure cache root");
        let registry = RegistryReadClientV1::new(
            "https://registry.example/"
                .parse()
                .expect("public registry URL"),
            Duration::from_secs(1),
            753,
        )
        .expect("signer-free registry client");
        let observer = CountingIntegrityObserver::default();
        let adapter =
            MusubiArchiveFetchAdapterV1::new(&registry, &cache).with_integrity_observer(&observer);
        let mut transport = UnavailableSorafsArchiveTransportV1;

        let error = adapter
            .fetch_exact(ArchiveId::new([0; 32]), &mut transport)
            .expect_err("zero archive identity must fail locally");
        assert_eq!(error.code(), "MUSUBI_ARCHIVE_LOCATION_EVIDENCE_INVALID");
        assert_eq!(error.class(), ArchiveFetchFailureClassV1::Permanent);
        assert_eq!(observer.archive_commitment.load(Ordering::SeqCst), 0);
        assert_eq!(observer.other.load(Ordering::SeqCst), 0);
    }
}
