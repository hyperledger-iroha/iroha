//! First-release Musubi registry, signing, and production-publication boundary.
//!
//! Public finalized reads retain only a Torii URL and never construct an account or
//! key pair. Mutations load a required Iroha `client.toml` only when the mutation is
//! dispatched, then sign the concrete V1 instruction locally. Publication delegates
//! clean-package validation, authenticated seed ingress, pin/order coordination, and
//! provider readback to an explicit runtime service; the default service fails closed.

use std::{
    error::Error,
    fmt, fs,
    io::Read,
    path::{Path, PathBuf},
    str::FromStr as _,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iroha::{
    client::{
        Client, PublicMusubiQueryPathV1, PublicMusubiQueryResultV1, post_public_musubi_query_v1,
    },
    config::Config,
};
use iroha_data_model::{
    isi::{InstructionBox, musubi::PublishMusubiReleaseV1},
    musubi::{
        MUSUBI_MAX_PAGE_SIZE_V1, MUSUBI_MIN_HEALTHY_REPLICAS_V1, MusubiAliasHistoryPageV1,
        MusubiAliasQueryV1, MusubiAliasRecordV1, MusubiArchiveLocationPageV1,
        MusubiArchiveLocationQueryV1, MusubiArchiveLocationStateV1, MusubiArchiveLocationV1,
        MusubiExactPackageQueryV1, MusubiExactReleaseQueryV1, MusubiMaintainerPageV1,
        MusubiOrderedPackagePageV1, MusubiOrderedPrefixQueryV1, MusubiOrderedPrefixV1,
        MusubiPackageIdV1, MusubiPackagePageQueryV1, MusubiPackageRecordV1,
        MusubiPackageSelectorV1, MusubiPageRequestV1, MusubiReleaseIdV1, MusubiReleaseRecordV1,
        MusubiResolverIndexPageV1, MusubiResolverIndexQueryV1, MusubiSeedIngressReceiptBindingV1,
        MusubiSeedIngressReceiptV1, MusubiVersionPageV1, MusubiVersionReqV1,
    },
    sorafs::capacity::ProviderId,
    transaction::FeePaymentIntent,
};
use norito::json::{JsonDeserialize, JsonSerialize};
use url::Url;

use crate::publish::{
    PublicationAdvanceV1, PublicationAmxSubmissionV1, PublicationArchiveRegistrationV1,
    PublicationBackend, PublicationBackendError, PublicationBackendFailureClass,
    PublicationCarSource, PublicationEngine, PublicationError, PublicationFinalEvidenceV1,
    PublicationOperationIdV1, PublicationReadbackEvidenceV1, PublicationRequestV1,
    PublicationValidationEvidenceV1,
};

const DEFAULT_CLIENT_CONFIG: &str = "client.toml";
const MAX_PUBLIC_CONFIG_BYTES: u64 = 1024 * 1024;
const DEFAULT_PUBLIC_QUERY_TIMEOUT: Duration = Duration::from_secs(30);

/// Retry classification for a redacted registry failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryFailureClassV1 {
    /// The same bounded request may succeed when retried.
    Retryable,
    /// External state or local configuration must change before retrying.
    Permanent,
    /// The exact requested record does not exist.
    NotFound,
    /// A finalized cursor is stale and must not be silently restarted.
    StaleCursor,
}

/// Stable, secret-redacted registry error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegistryErrorV1 {
    class: RegistryFailureClassV1,
    code: &'static str,
}

impl RegistryErrorV1 {
    const fn new(class: RegistryFailureClassV1, code: &'static str) -> Self {
        Self { class, code }
    }

    /// Return the retry classification without exposing transport or configuration detail.
    #[must_use]
    pub const fn class(&self) -> RegistryFailureClassV1 {
        self.class
    }

    /// Return the stable public error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for RegistryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code)
    }
}

impl Error for RegistryErrorV1 {}

/// Signer-free client for the fixed public Musubi V1 finalized-query inventory.
#[derive(Clone, Debug)]
pub struct RegistryReadClientV1 {
    torii_url: Url,
    timeout: Duration,
}

impl RegistryReadClientV1 {
    /// Construct a signer-free client from an already validated public Torii URL.
    pub fn new(torii_url: Url, timeout: Duration) -> Result<Self, RegistryErrorV1> {
        if !matches!(torii_url.scheme(), "http" | "https")
            || !torii_url.username().is_empty()
            || torii_url.password().is_some()
            || timeout == Duration::ZERO
            || timeout > Duration::from_secs(60)
        {
            return Err(RegistryErrorV1::new(
                RegistryFailureClassV1::Permanent,
                "MUSUBI_REGISTRY_PUBLIC_CONFIG_INVALID",
            ));
        }
        Ok(Self { torii_url, timeout })
    }

    /// Load only `torii_url` from explicit `--config` or the platform `client.toml` convention.
    ///
    /// Account, private-key, bearer-token, and basic-auth fields are neither parsed into their
    /// typed forms nor retained. The default path is the same required `client.toml` used by the
    /// Iroha CLI; project manifests and command-line credential values are never consulted.
    pub fn load(config: Option<&Path>) -> Result<Self, RegistryErrorV1> {
        let path = config.map_or_else(|| PathBuf::from(DEFAULT_CLIENT_CONFIG), Path::to_path_buf);
        let text = read_bounded_config(&path)?;
        let document = text.parse::<toml::Value>().map_err(|_| {
            RegistryErrorV1::new(
                RegistryFailureClassV1::Permanent,
                "MUSUBI_REGISTRY_PUBLIC_CONFIG_INVALID",
            )
        })?;
        let raw_url = document
            .get("torii_url")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| {
                RegistryErrorV1::new(
                    RegistryFailureClassV1::Permanent,
                    "MUSUBI_REGISTRY_PUBLIC_CONFIG_INVALID",
                )
            })?;
        let torii_url = raw_url.parse::<Url>().map_err(|_| {
            RegistryErrorV1::new(
                RegistryFailureClassV1::Permanent,
                "MUSUBI_REGISTRY_PUBLIC_CONFIG_INVALID",
            )
        })?;
        let timeout = document
            .get("torii_request_timeout_ms")
            .and_then(toml::Value::as_integer)
            .and_then(|value| u64::try_from(value).ok())
            .map(Duration::from_millis)
            .unwrap_or(DEFAULT_PUBLIC_QUERY_TIMEOUT)
            .min(Duration::from_secs(60));
        Self::new(torii_url, timeout)
    }

    /// Return the configured public endpoint. No account or credential material is retained.
    #[must_use]
    pub const fn torii_url(&self) -> &Url {
        &self.torii_url
    }

    /// Resolve canonical `namespace/package` text to its structural package identity.
    pub fn resolve_selector(
        &self,
        selector: &MusubiPackageSelectorV1,
    ) -> Result<MusubiPackageIdV1, RegistryErrorV1> {
        selector.validate().map_err(|_| invalid_response())?;
        let prefix =
            MusubiOrderedPrefixV1::new(&selector.to_string()).map_err(|_| invalid_response())?;
        let page = self.ordered_prefix(&MusubiOrderedPrefixQueryV1 {
            prefix,
            page: first_page(2),
        })?;
        page.items
            .into_iter()
            .find(|entry| &entry.selector == selector)
            .map(|entry| entry.package)
            .ok_or_else(|| {
                RegistryErrorV1::new(RegistryFailureClassV1::NotFound, "MUSUBI_PACKAGE_NOT_FOUND")
            })
    }

    /// Fetch and validate one exact authoritative package record.
    pub fn exact_package(
        &self,
        package: MusubiPackageIdV1,
    ) -> Result<Option<MusubiPackageRecordV1>, RegistryErrorV1> {
        let output = self.query_optional::<_, MusubiPackageRecordV1>(
            PublicMusubiQueryPathV1::ExactPackage,
            &MusubiExactPackageQueryV1 { package },
        )?;
        if let Some(record) = &output {
            record.validate().map_err(|_| invalid_response())?;
        }
        Ok(output)
    }

    /// Fetch and validate one exact immutable release record.
    pub fn exact_release(
        &self,
        release: MusubiReleaseIdV1,
    ) -> Result<Option<MusubiReleaseRecordV1>, RegistryErrorV1> {
        let output = self.query_optional::<_, MusubiReleaseRecordV1>(
            PublicMusubiQueryPathV1::ExactRelease,
            &MusubiExactReleaseQueryV1 { release },
        )?;
        if let Some(record) = &output {
            record.validate().map_err(|_| invalid_response())?;
        }
        Ok(output)
    }

    /// Fetch and validate one finalized resolver-index page.
    pub fn resolver_index(
        &self,
        request: &MusubiResolverIndexQueryV1,
    ) -> Result<MusubiResolverIndexPageV1, RegistryErrorV1> {
        let page = self.query_required::<_, MusubiResolverIndexPageV1>(
            PublicMusubiQueryPathV1::ResolverIndex,
            request,
        )?;
        page.validate().map_err(|_| invalid_response())?;
        for row in &page.items {
            row.validate().map_err(|_| invalid_response())?;
            if row.release.package != request.package {
                return Err(invalid_response());
            }
        }
        Ok(page)
    }

    /// Fetch and validate one finalized package-version page.
    pub fn versions(
        &self,
        request: &MusubiPackagePageQueryV1,
    ) -> Result<MusubiVersionPageV1, RegistryErrorV1> {
        let page = self
            .query_required::<_, MusubiVersionPageV1>(PublicMusubiQueryPathV1::Versions, request)?;
        page.validate().map_err(|_| invalid_response())?;
        for version in &page.items {
            version.validate().map_err(|_| invalid_response())?;
        }
        Ok(page)
    }

    /// Fetch and validate one finalized accepted-member page.
    pub fn maintainers(
        &self,
        request: &MusubiPackagePageQueryV1,
    ) -> Result<MusubiMaintainerPageV1, RegistryErrorV1> {
        let page = self.query_required::<_, MusubiMaintainerPageV1>(
            PublicMusubiQueryPathV1::Maintainers,
            request,
        )?;
        page.validate().map_err(|_| invalid_response())?;
        for member in &page.items {
            member.validate().map_err(|_| invalid_response())?;
            if member.package != request.package {
                return Err(invalid_response());
            }
        }
        Ok(page)
    }

    /// Fetch and validate one finalized archive-location page.
    pub fn archive_locations(
        &self,
        request: &MusubiArchiveLocationQueryV1,
    ) -> Result<Option<MusubiArchiveLocationPageV1>, RegistryErrorV1> {
        let page = self.query_optional::<_, MusubiArchiveLocationPageV1>(
            PublicMusubiQueryPathV1::ArchiveLocations,
            request,
        )?;
        if let Some(page) = &page {
            page.validate().map_err(|_| invalid_response())?;
            for location in &page.items {
                location.validate().map_err(|_| invalid_response())?;
                if location.archive_id != request.archive_id {
                    return Err(invalid_response());
                }
            }
        }
        Ok(page)
    }

    /// Fetch and structurally validate one exact permanent alias record.
    pub fn alias(
        &self,
        request: &MusubiAliasQueryV1,
    ) -> Result<Option<MusubiAliasRecordV1>, RegistryErrorV1> {
        let record =
            self.query_optional::<_, MusubiAliasRecordV1>(PublicMusubiQueryPathV1::Alias, request)?;
        if let Some(record) = &record {
            record.alias.validate().map_err(|_| invalid_response())?;
            record.target.validate().map_err(|_| invalid_response())?;
            if record.alias != request.alias
                || record.pricing_revision == 0
                || record.paid_xor == 0
                || record.registered_at_height == 0
                || record.history_revision == 0
            {
                return Err(invalid_response());
            }
        }
        Ok(record)
    }

    /// Fetch and validate one finalized permanent-alias history page.
    pub fn alias_history(
        &self,
        request: &MusubiAliasQueryV1,
    ) -> Result<MusubiAliasHistoryPageV1, RegistryErrorV1> {
        let page = self.query_required::<_, MusubiAliasHistoryPageV1>(
            PublicMusubiQueryPathV1::AliasHistory,
            request,
        )?;
        page.validate().map_err(|_| invalid_response())?;
        for entry in &page.items {
            entry.validate().map_err(|_| invalid_response())?;
            if entry.alias != request.alias {
                return Err(invalid_response());
            }
        }
        Ok(page)
    }

    /// Fetch and validate one finalized byte-ordered package-prefix page.
    pub fn ordered_prefix(
        &self,
        request: &MusubiOrderedPrefixQueryV1,
    ) -> Result<MusubiOrderedPackagePageV1, RegistryErrorV1> {
        let page = self.query_required::<_, MusubiOrderedPackagePageV1>(
            PublicMusubiQueryPathV1::OrderedPrefix,
            request,
        )?;
        page.validate().map_err(|_| invalid_response())?;
        for entry in &page.items {
            entry.validate().map_err(|_| invalid_response())?;
            if !entry
                .selector
                .to_string()
                .starts_with(request.prefix.as_str())
            {
                return Err(invalid_response());
            }
        }
        Ok(page)
    }

    fn query_required<Q, R>(
        &self,
        path: PublicMusubiQueryPathV1,
        query: &Q,
    ) -> Result<R, RegistryErrorV1>
    where
        Q: JsonSerialize + ?Sized,
        R: JsonDeserialize,
    {
        self.query_optional(path, query)?.ok_or_else(|| {
            RegistryErrorV1::new(
                RegistryFailureClassV1::NotFound,
                "MUSUBI_REGISTRY_RECORD_NOT_FOUND",
            )
        })
    }

    fn query_optional<Q, R>(
        &self,
        path: PublicMusubiQueryPathV1,
        query: &Q,
    ) -> Result<Option<R>, RegistryErrorV1>
    where
        Q: JsonSerialize + ?Sized,
        R: JsonDeserialize,
    {
        let response = post_public_musubi_query_v1(&self.torii_url, path, query, self.timeout)
            .map_err(|_| {
                RegistryErrorV1::new(
                    RegistryFailureClassV1::Retryable,
                    "MUSUBI_REGISTRY_QUERY_FAILED",
                )
            })?;
        match response {
            PublicMusubiQueryResultV1::Found(value) => Ok(Some(value)),
            PublicMusubiQueryResultV1::NotFound => Ok(None),
            PublicMusubiQueryResultV1::StaleCursor => Err(RegistryErrorV1::new(
                RegistryFailureClassV1::StaleCursor,
                "MUSUBI_REGISTRY_STALE_CURSOR",
            )),
        }
    }
}

/// Client that loads a complete signer only at a registry mutation boundary.
#[derive(Clone)]
pub struct RegistrySigningClientV1 {
    client: Client,
}

impl fmt::Debug for RegistrySigningClientV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegistrySigningClientV1")
            .field("authority", &self.client.account)
            .finish_non_exhaustive()
    }
}

impl RegistrySigningClientV1 {
    /// Load a required explicit `--config` or the platform `client.toml`, without env overrides.
    pub fn load(config: Option<&Path>) -> Result<Self, RegistryErrorV1> {
        let path = config.map_or_else(|| PathBuf::from(DEFAULT_CLIENT_CONFIG), Path::to_path_buf);
        let configuration = Config::load_file(path).map_err(|_| {
            RegistryErrorV1::new(
                RegistryFailureClassV1::Permanent,
                "MUSUBI_REGISTRY_SIGNING_CONFIG_INVALID",
            )
        })?;
        let mut client = Client::new(configuration);
        client.torii_request_timeout = client.torii_request_timeout.min(Duration::from_secs(60));
        Ok(Self { client })
    }

    /// Return the configured mutation authority.
    #[must_use]
    pub const fn authority(&self) -> &iroha_data_model::account::AccountId {
        &self.client.account
    }

    /// Sign, submit, and wait for commitment of one concrete V1 instruction.
    pub fn submit_v1<I>(&self, instruction: I) -> Result<[u8; 32], RegistryErrorV1>
    where
        I: Into<InstructionBox>,
    {
        let hash = self
            .client
            .submit_blocking(instruction, FeePaymentIntent::authority(Vec::new(), None))
            .map_err(|_| {
                RegistryErrorV1::new(
                    RegistryFailureClassV1::Retryable,
                    "MUSUBI_REGISTRY_MUTATION_FAILED",
                )
            })?;
        Ok(*hash.as_ref())
    }
}

/// Runtime-only production services whose authenticated server contracts are outside Torii reads.
pub trait PublicationRuntimeServicesV1 {
    /// Parse, verify, resolve, and compiler-check the exact clean package CAR.
    fn validate_clean_package(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError>;

    /// Stage bytes through an admitted authenticated seed-ingress broker and return its receipt.
    fn stage_authenticated_seed_ingress(
        &mut self,
        operation_id: PublicationOperationIdV1,
        expected: &MusubiSeedIngressReceiptBindingV1,
        car: &mut dyn Read,
    ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError>;

    /// Register/reuse the archive and permanent registry-grade pin/order atomically and idempotently.
    fn ensure_archive_and_permanent_pin(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<PublicationArchiveRegistrationV1, PublicationBackendError>;

    /// Read, parse, and verify the complete archive through one finalized provider.
    fn readback_provider(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        location: &MusubiArchiveLocationV1,
        provider: ProviderId,
    ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError>;
}

/// Fail-closed runtime used until an operator supplies all authenticated production services.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnavailablePublicationRuntimeV1;

impl PublicationRuntimeServicesV1 for UnavailablePublicationRuntimeV1 {
    fn validate_clean_package(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
        Err(PublicationBackendError::permanent(
            "PACKAGE_VALIDATOR_NOT_CONFIGURED",
        ))
    }

    fn stage_authenticated_seed_ingress(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _expected: &MusubiSeedIngressReceiptBindingV1,
        _car: &mut dyn Read,
    ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
        // TODO: Implement the server-side authenticated admitted-broker Norito request/response
        // contract. This must stay fail-closed and must never fall back to `/v1/sorafs/upload`.
        Err(PublicationBackendError::permanent(
            "SEED_INGRESS_SERVICE_NOT_CONFIGURED",
        ))
    }

    fn ensure_archive_and_permanent_pin(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<PublicationArchiveRegistrationV1, PublicationBackendError> {
        Err(PublicationBackendError::permanent(
            "STORAGE_COORDINATOR_NOT_CONFIGURED",
        ))
    }

    fn readback_provider(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _location: &MusubiArchiveLocationV1,
        _provider: ProviderId,
    ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
        Err(PublicationBackendError::permanent(
            "PROVIDER_READBACK_NOT_CONFIGURED",
        ))
    }
}

/// Concrete publication backend combining finalized reads, local signing, and runtime services.
#[derive(Debug)]
pub struct RegistryPublicationBackendV1<S> {
    read: RegistryReadClientV1,
    signing: RegistrySigningClientV1,
    services: S,
    operation_id: PublicationOperationIdV1,
    publisher: iroha_data_model::account::AccountId,
}

impl<S> RegistryPublicationBackendV1<S> {
    /// Bind the backend to exactly one public request and operation id.
    pub fn new(
        read: RegistryReadClientV1,
        signing: RegistrySigningClientV1,
        services: S,
        request: &PublicationRequestV1,
    ) -> Result<Self, RegistryErrorV1> {
        if signing.authority() != &request.publisher {
            return Err(RegistryErrorV1::new(
                RegistryFailureClassV1::Permanent,
                "MUSUBI_PUBLICATION_AUTHORITY_MISMATCH",
            ));
        }
        Ok(Self {
            read,
            signing,
            services,
            operation_id: request.operation_id(),
            publisher: request.publisher.clone(),
        })
    }

    fn check_operation(
        &self,
        operation_id: PublicationOperationIdV1,
    ) -> Result<(), PublicationBackendError> {
        if operation_id == self.operation_id {
            Ok(())
        } else {
            Err(PublicationBackendError::permanent(
                "PUBLICATION_OPERATION_MISMATCH",
            ))
        }
    }

    fn check_request(&self, request: &PublicationRequestV1) -> Result<(), PublicationBackendError> {
        if request.operation_id() == self.operation_id && request.publisher == self.publisher {
            Ok(())
        } else {
            Err(PublicationBackendError::permanent(
                "PUBLICATION_REQUEST_MISMATCH",
            ))
        }
    }
}

impl<S: PublicationRuntimeServicesV1> PublicationBackend for RegistryPublicationBackendV1<S> {
    fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| PublicationBackendError::permanent("SYSTEM_TIME_INVALID"))?;
        u64::try_from(elapsed.as_millis())
            .map_err(|_| PublicationBackendError::permanent("SYSTEM_TIME_INVALID"))
    }

    fn validate_clean_package(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
        self.check_operation(operation_id)?;
        self.check_request(request)?;
        self.services
            .validate_clean_package(operation_id, request, car)
    }

    fn stage_authenticated_seed_ingress(
        &mut self,
        operation_id: PublicationOperationIdV1,
        expected: &MusubiSeedIngressReceiptBindingV1,
        car: &mut dyn Read,
    ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
        self.check_operation(operation_id)?;
        self.services
            .stage_authenticated_seed_ingress(operation_id, expected, car)
    }

    fn ensure_archive_and_permanent_pin(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<PublicationArchiveRegistrationV1, PublicationBackendError> {
        self.check_operation(operation_id)?;
        self.check_request(request)?;
        self.services
            .ensure_archive_and_permanent_pin(operation_id, request, receipt)
    }

    fn finalized_replication(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
    ) -> Result<Option<MusubiArchiveLocationV1>, PublicationBackendError> {
        self.check_operation(operation_id)?;
        self.check_request(request)?;
        let query = MusubiArchiveLocationQueryV1 {
            archive_id: request.archive_commitment.archive_id(),
            page: first_page(MUSUBI_MAX_PAGE_SIZE_V1 as u32),
        };
        let Some(page) = self
            .read
            .archive_locations(&query)
            .map_err(registry_backend_error)?
        else {
            return Ok(None);
        };
        let location = page
            .items
            .into_iter()
            .find(|candidate| candidate.location_id == registration.location_id);
        Ok(location.filter(|candidate| {
            candidate.state == MusubiArchiveLocationStateV1::Healthy
                && candidate.providers.len() >= usize::from(MUSUBI_MIN_HEALTHY_REPLICAS_V1)
        }))
    }

    fn readback_provider(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        location: &MusubiArchiveLocationV1,
        provider: ProviderId,
    ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
        self.check_operation(operation_id)?;
        self.check_request(request)?;
        self.services
            .readback_provider(operation_id, request, location, provider)
    }

    fn submit_release_native_amx(
        &mut self,
        operation_id: PublicationOperationIdV1,
        instruction: &PublishMusubiReleaseV1,
    ) -> Result<PublicationAmxSubmissionV1, PublicationBackendError> {
        self.check_operation(operation_id)?;
        let transaction_hash = self
            .signing
            .submit_v1(instruction.clone())
            .map_err(registry_backend_error)?;
        Ok(PublicationAmxSubmissionV1::new(
            operation_id,
            instruction,
            transaction_hash,
        ))
    }

    fn finalized_release_and_index(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _submission: &PublicationAmxSubmissionV1,
    ) -> Result<Option<PublicationFinalEvidenceV1>, PublicationBackendError> {
        self.check_operation(operation_id)?;
        self.check_request(request)?;
        let release_id = request.publication.manifest.release.clone();
        let Some(home_release) = self
            .read
            .exact_release(release_id.clone())
            .map_err(registry_backend_error)?
        else {
            return Ok(None);
        };
        let requirement = MusubiVersionReqV1::from_str(&format!("={}", release_id.version))
            .map_err(|_| PublicationBackendError::permanent("FINAL_QUERY_INVALID"))?;
        let page = self
            .read
            .resolver_index(&MusubiResolverIndexQueryV1 {
                package: release_id.package.clone(),
                requirement: Some(requirement),
                page: first_page(MUSUBI_MAX_PAGE_SIZE_V1 as u32),
            })
            .map_err(registry_backend_error)?;
        let mut matching = page
            .items
            .into_iter()
            .filter(|row| row.release == release_id);
        let Some(universal_release) = matching.next() else {
            return Ok(None);
        };
        if matching.next().is_some() {
            return Err(PublicationBackendError::permanent(
                "FINAL_QUERY_DUPLICATE_RELEASE",
            ));
        }
        Ok(Some(PublicationFinalEvidenceV1 {
            snapshot: page.snapshot,
            home_release,
            universal_release,
        }))
    }
}

/// Bounded polling policy for finalized replication and release verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicationPollPolicyV1 {
    /// Total calls, including the initial attempt.
    pub max_attempts: u8,
    /// Delay after the first pending or retryable result.
    pub initial_delay: Duration,
    /// Maximum exponential-backoff delay.
    pub max_delay: Duration,
}

impl Default for PublicationPollPolicyV1 {
    fn default() -> Self {
        Self {
            max_attempts: 12,
            initial_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(5),
        }
    }
}

impl PublicationPollPolicyV1 {
    /// Validate non-zero bounded attempts and sub-minute delays.
    pub fn validate(self) -> Result<(), PublicationError> {
        if self.max_attempts == 0
            || self.initial_delay == Duration::ZERO
            || self.max_delay < self.initial_delay
            || self.max_delay > Duration::from_secs(30)
        {
            return Err(PublicationError::InvalidJournal(
                "publication polling policy is invalid".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Resume with bounded exponential backoff, preserving pending state when the budget expires.
pub fn resume_with_bounded_polling(
    engine: &PublicationEngine<'_>,
    operation_id: PublicationOperationIdV1,
    source: &dyn PublicationCarSource,
    backend: &mut dyn PublicationBackend,
    policy: PublicationPollPolicyV1,
) -> Result<PublicationAdvanceV1, PublicationError> {
    policy.validate()?;
    let mut delay = policy.initial_delay;
    let mut last_pending = None;
    let mut last_retryable = None;
    for attempt in 0..policy.max_attempts {
        match engine.resume(operation_id, source, backend) {
            Ok(PublicationAdvanceV1::Pending(phase)) => {
                last_pending = Some(PublicationAdvanceV1::Pending(phase));
                last_retryable = None;
            }
            Ok(result) => return Ok(result),
            Err(PublicationError::Backend(error))
                if error.class() == PublicationBackendFailureClass::Retryable =>
            {
                last_pending = None;
                last_retryable = Some(error);
            }
            Err(error) => return Err(error),
        }
        if attempt + 1 < policy.max_attempts {
            thread::sleep(delay);
            delay = delay.saturating_mul(2).min(policy.max_delay);
        }
    }
    if let Some(error) = last_retryable {
        Err(PublicationError::Backend(error))
    } else if let Some(pending) = last_pending {
        Ok(pending)
    } else {
        Err(PublicationError::InvalidJournal(
            "publication polling completed without an observable result".to_owned(),
        ))
    }
}

fn first_page(limit: u32) -> MusubiPageRequestV1 {
    MusubiPageRequestV1 {
        limit,
        cursor: None,
    }
}

fn invalid_response() -> RegistryErrorV1 {
    RegistryErrorV1::new(
        RegistryFailureClassV1::Permanent,
        "MUSUBI_REGISTRY_RESPONSE_INVALID",
    )
}

fn registry_backend_error(error: RegistryErrorV1) -> PublicationBackendError {
    match error.class() {
        RegistryFailureClassV1::Retryable => PublicationBackendError::retryable(error.code()),
        RegistryFailureClassV1::Permanent
        | RegistryFailureClassV1::NotFound
        | RegistryFailureClassV1::StaleCursor => PublicationBackendError::permanent(error.code()),
    }
}

fn read_bounded_config(path: &Path) -> Result<String, RegistryErrorV1> {
    let metadata = fs::symlink_metadata(path).map_err(|_| {
        RegistryErrorV1::new(
            RegistryFailureClassV1::Permanent,
            "MUSUBI_REGISTRY_CONFIG_NOT_FOUND",
        )
    })?;
    if !metadata.is_file() || metadata.len() > MAX_PUBLIC_CONFIG_BYTES {
        return Err(RegistryErrorV1::new(
            RegistryFailureClassV1::Permanent,
            "MUSUBI_REGISTRY_PUBLIC_CONFIG_INVALID",
        ));
    }
    let mut file = fs::File::open(path).map_err(|_| {
        RegistryErrorV1::new(
            RegistryFailureClassV1::Permanent,
            "MUSUBI_REGISTRY_PUBLIC_CONFIG_INVALID",
        )
    })?;
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    file.by_ref()
        .take(MAX_PUBLIC_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| {
            RegistryErrorV1::new(
                RegistryFailureClassV1::Permanent,
                "MUSUBI_REGISTRY_PUBLIC_CONFIG_INVALID",
            )
        })?;
    if bytes.len() as u64 > MAX_PUBLIC_CONFIG_BYTES {
        return Err(RegistryErrorV1::new(
            RegistryFailureClassV1::Permanent,
            "MUSUBI_REGISTRY_PUBLIC_CONFIG_INVALID",
        ));
    }
    String::from_utf8(bytes).map_err(|_| {
        RegistryErrorV1::new(
            RegistryFailureClassV1::Permanent,
            "MUSUBI_REGISTRY_PUBLIC_CONFIG_INVALID",
        )
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use iroha::crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        musubi::{ArchiveId, MusubiContentDigestV1, MusubiSemanticReleaseDigestV1},
    };
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn public_config_ignores_signer_fields() {
        let temporary = tempdir().expect("temporary directory");
        let path = temporary.path().join("client.toml");
        fs::write(
            &path,
            r#"
                torii_url = "https://registry.example/iroha/"
                [account]
                public_key = "deliberately-not-a-key"
                private_key = "must-not-be-parsed"
            "#,
        )
        .expect("write public config fixture");

        let client = RegistryReadClientV1::load(Some(&path)).expect("load URL only");
        assert_eq!(
            client.torii_url().as_str(),
            "https://registry.example/iroha/"
        );
    }

    struct CountingReader<'a> {
        reads: &'a Cell<usize>,
    }

    impl Read for CountingReader<'_> {
        fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
            self.reads.set(self.reads.get() + 1);
            Ok(0)
        }
    }

    #[test]
    fn missing_seed_ingress_fails_before_reading_car() {
        let mut runtime = UnavailablePublicationRuntimeV1;
        let reads = Cell::new(0);
        let mut reader = CountingReader { reads: &reads };
        let operation = "0101010101010101010101010101010101010101010101010101010101010101"
            .parse()
            .expect("operation id");

        // The service does not inspect either the binding or the reader before refusing use.
        let error = runtime
            .stage_authenticated_seed_ingress(operation, &binding(), &mut reader)
            .expect_err("unconfigured ingress must fail closed");
        assert_eq!(error.code(), "SEED_INGRESS_SERVICE_NOT_CONFIGURED");
        assert_eq!(reads.get(), 0);
    }

    fn account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive fixture key pair");
        AccountId::new(keypair.public_key().clone())
    }

    fn binding() -> MusubiSeedIngressReceiptBindingV1 {
        MusubiSeedIngressReceiptBindingV1 {
            chain_id: ChainId::from("musubi-registry-test"),
            genesis_block_hash: [1; 32],
            publisher: account(2),
            ingress_broker: account(3),
            seed_provider: ProviderId::new([4; 32]),
            semantic_release_manifest_digest: MusubiSemanticReleaseDigestV1::new([5; 32]),
            archive_id: ArchiveId::new([6; 32]),
            car_body_digest: MusubiContentDigestV1::new([7; 32]),
            car_body_length: 8,
            nonce: [9; 32],
        }
    }

    #[test]
    fn polling_policy_rejects_unbounded_delay() {
        let error = PublicationPollPolicyV1 {
            max_attempts: 1,
            initial_delay: Duration::from_secs(31),
            max_delay: Duration::from_secs(31),
        }
        .validate()
        .expect_err("sub-minute bound is mandatory");
        assert!(matches!(error, PublicationError::InvalidJournal(_)));
    }
}
