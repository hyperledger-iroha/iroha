//! Production-configured services for the resumable Musubi publication state machine.
//!
//! Endpoints and public deployment bindings are loaded only from the selected platform
//! `client.toml`. Account authentication reuses that file's Iroha signer through a fixed,
//! domain-separated request protocol; no token, credential, or provider URL enters a project,
//! command line, lockfile, or publication journal.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    fs::OpenOptions,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use iroha::musubi_runtime::{
    AuthenticatedMusubiPublicationRuntimeClientV1, MusubiProviderReadbackRequestV1,
    MusubiPublicationRuntimeTransportErrorV1, MusubiPublicationRuntimeTransportFailureClassV1,
    MusubiSeedIngressStageRequestV1, MusubiStorageCoordinationRequestV1,
    MusubiStorageLocationDispositionV1, publication_service_origin,
    validate_publication_service_base_url,
};
use iroha_data_model::{
    isi::musubi::{AddMusubiArchiveLocationV1, RegisterMusubiArchiveV1},
    musubi::{
        MUSUBI_MAX_LOCATION_PROVIDERS_V1, MusubiNamespaceDelegationV1,
        MusubiSeedIngressReceiptBindingV1, MusubiSeedIngressReceiptV1,
    },
    sorafs::capacity::ProviderId,
};
use norito::DecodeLimits;
use url::Url;

use crate::{
    publish::{
        PublicationArchiveRegistrationV1, PublicationBackendError, PublicationOperationIdV1,
        PublicationReadbackEvidenceV1, PublicationRequestV1, PublicationValidationEvidenceV1,
    },
    registry::{PublicationRuntimeServicesV1, RegistryFailureClassV1, RegistrySigningClientV1},
};

const DEFAULT_CLIENT_CONFIG: &str = "client.toml";
const MAX_CLIENT_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_DELEGATION_BYTES: u64 = 256 * 1024;
const MAX_DELEGATION_BYTES_USIZE: usize = 256 * 1024;
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 30_000;
const MAX_REQUEST_TIMEOUT_MS: u64 = 60_000;
const DELEGATION_DECODE_LIMITS: DecodeLimits =
    DecodeLimits::new(64, MAX_DELEGATION_BYTES_USIZE, 256, 512 * 1024, 16);

/// Public request bindings selected by the production platform configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionPublicationBindingsV1 {
    /// Authenticated seed-ingress broker whose controller signs staging receipts.
    pub ingress_broker: iroha_data_model::account::AccountId,
    /// Seed provider selected for initial admitted ingress.
    pub seed_provider: ProviderId,
    /// Exact current registry admission-policy revision.
    pub expected_policy_revision: u64,
    /// Optional public generation-bound delegation for a delegated first package claim.
    pub namespace_delegation: Option<MusubiNamespaceDelegationV1>,
}

/// Stable, secret-redacted platform publication configuration failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProductionPublicationConfigurationErrorV1 {
    code: &'static str,
}

impl ProductionPublicationConfigurationErrorV1 {
    const fn new(code: &'static str) -> Self {
        Self { code }
    }

    /// Return the stable payload-free failure code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for ProductionPublicationConfigurationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code)
    }
}

impl std::error::Error for ProductionPublicationConfigurationErrorV1 {}

/// Clean-package compiler validation injected by the packaging command.
pub trait PublicationCleanPackageValidatorV1 {
    /// Validate the exact packaged CAR and return secret-free evidence.
    fn validate_clean_package(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError>;
}

impl<F> PublicationCleanPackageValidatorV1 for F
where
    F: FnMut(
        PublicationOperationIdV1,
        &PublicationRequestV1,
        &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError>,
{
    fn validate_clean_package(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
        self(operation_id, request, car)
    }
}

/// Fail-closed validator for callers that have not wired clean packaged-tree compilation.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnavailablePublicationCleanPackageValidatorV1;

impl PublicationCleanPackageValidatorV1 for UnavailablePublicationCleanPackageValidatorV1 {
    fn validate_clean_package(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
        // Configuration-only callers may select this explicit sentinel. Executable publish and
        // resume commands inject their clean packaged-tree validator instead.
        Err(PublicationBackendError::permanent(
            "PACKAGE_VALIDATOR_NOT_CONFIGURED",
        ))
    }
}

#[derive(Clone)]
struct ParsedProductionPublicationConfigV1 {
    seed_ingress_url: Url,
    storage_coordinator_url: Url,
    provider_gateways: BTreeMap<ProviderId, Url>,
    request_timeout: Duration,
    bindings: ProductionPublicationBindingsV1,
}

impl fmt::Debug for ParsedProductionPublicationConfigV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParsedProductionPublicationConfigV1")
            .field("bindings", &self.bindings)
            .field("provider_gateway_count", &self.provider_gateways.len())
            .field("request_timeout", &self.request_timeout)
            .finish_non_exhaustive()
    }
}

/// Production implementation of the runtime-only publication service boundary.
pub struct ProductionPublicationRuntimeV1<V> {
    signing: RegistrySigningClientV1,
    http: AuthenticatedMusubiPublicationRuntimeClientV1,
    validator: V,
    seed_ingress_url: Url,
    storage_coordinator_url: Url,
    provider_gateways: BTreeMap<ProviderId, Url>,
    bindings: ProductionPublicationBindingsV1,
}

impl<V> fmt::Debug for ProductionPublicationRuntimeV1<V> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionPublicationRuntimeV1")
            .field("publisher", self.signing.authority())
            .field("bindings", &self.bindings)
            .field("provider_gateway_count", &self.provider_gateways.len())
            .finish_non_exhaustive()
    }
}

impl<V> ProductionPublicationRuntimeV1<V> {
    /// Return the public request bindings while keeping runtime endpoints encapsulated.
    #[must_use]
    pub const fn bindings(&self) -> &ProductionPublicationBindingsV1 {
        &self.bindings
    }

    fn validate_request(
        &self,
        request: &PublicationRequestV1,
    ) -> Result<(), PublicationBackendError> {
        if &request.chain_id != self.http.chain_id()
            || &request.publisher != self.http.publisher()
            || request.ingress_broker != self.bindings.ingress_broker
            || request.seed_provider != self.bindings.seed_provider
            || request.expected_policy_revision != self.bindings.expected_policy_revision
            || request.namespace_delegation != self.bindings.namespace_delegation
        {
            return Err(PublicationBackendError::permanent(
                "PUBLICATION_PLATFORM_BINDING_MISMATCH",
            ));
        }
        Ok(())
    }
}

impl<V: PublicationCleanPackageValidatorV1> PublicationRuntimeServicesV1
    for ProductionPublicationRuntimeV1<V>
{
    fn validate_clean_package(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
        self.validate_request(request)?;
        self.validator
            .validate_clean_package(operation_id, request, car)
    }

    fn stage_authenticated_seed_ingress(
        &mut self,
        operation_id: PublicationOperationIdV1,
        expected: &MusubiSeedIngressReceiptBindingV1,
        car: &mut dyn Read,
    ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
        if expected.chain_id != *self.http.chain_id()
            || expected.publisher != *self.http.publisher()
            || expected.ingress_broker != self.bindings.ingress_broker
            || expected.seed_provider != self.bindings.seed_provider
        {
            return Err(PublicationBackendError::permanent(
                "PUBLICATION_PLATFORM_BINDING_MISMATCH",
            ));
        }
        let request = MusubiSeedIngressStageRequestV1 {
            version: 1,
            operation_id: *operation_id.as_bytes(),
            binding: expected.clone(),
        };
        self.http
            .stage_seed_ingress(&self.seed_ingress_url, &request, car, current_time_ms()?)
            .map_err(map_transport_error)
    }

    fn ensure_archive_and_permanent_pin(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<PublicationArchiveRegistrationV1, PublicationBackendError> {
        self.validate_request(request)?;
        self.signing
            .submit_v1(RegisterMusubiArchiveV1::new(
                request.archive_commitment.clone(),
                receipt.clone(),
                request.expected_policy_revision,
            ))
            .map_err(map_registry_error)?;

        let coordination_request = MusubiStorageCoordinationRequestV1 {
            version: 1,
            operation_id: *operation_id.as_bytes(),
            chain_id: request.chain_id.clone(),
            genesis_block_hash: request.genesis_block_hash,
            publisher: request.publisher.clone(),
            commitment: request.archive_commitment.clone(),
            staging_receipt: receipt.clone(),
            expected_policy_revision: request.expected_policy_revision,
        };
        let response = self
            .http
            .coordinate_storage(
                &self.storage_coordinator_url,
                &coordination_request,
                current_time_ms()?,
            )
            .map_err(map_transport_error)?;

        match &response.disposition {
            MusubiStorageLocationDispositionV1::NeedsRegistration {
                provider_attestations,
                expected_location_revision,
            } => {
                self.signing
                    .submit_v1(AddMusubiArchiveLocationV1 {
                        archive_id: request.archive_commitment.archive_id(),
                        location_id: response.location_id,
                        pin_manifest: response.pin_manifest,
                        replication_order: response.replication_order,
                        provider_attestations: provider_attestations.clone(),
                        renew_after_epoch: response.renew_after_epoch,
                        expires_at_epoch: response.expires_at_epoch,
                        expected_location_revision: *expected_location_revision,
                    })
                    .map_err(map_registry_error)?;
            }
            MusubiStorageLocationDispositionV1::Registered(_) => {}
        }

        Ok(PublicationArchiveRegistrationV1 {
            archive: response.archive,
            location_id: response.location_id,
            pin_manifest: response.pin_manifest,
            replication_order: response.replication_order,
            renew_after_epoch: response.renew_after_epoch,
            expires_at_epoch: response.expires_at_epoch,
        })
    }

    fn readback_provider(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        location: &iroha_data_model::musubi::MusubiArchiveLocationV1,
        provider: ProviderId,
    ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
        self.validate_request(request)?;
        let gateway = self.provider_gateways.get(&provider).ok_or_else(|| {
            PublicationBackendError::permanent("PROVIDER_READBACK_ENDPOINT_NOT_CONFIGURED")
        })?;
        let readback_request = MusubiProviderReadbackRequestV1 {
            version: 1,
            operation_id: *operation_id.as_bytes(),
            chain_id: request.chain_id.clone(),
            genesis_block_hash: request.genesis_block_hash,
            publisher: request.publisher.clone(),
            location: location.clone(),
            provider,
            commitment: request.archive_commitment.clone(),
            semantic_release_digest: request.publication.manifest.semantic_digest(),
            verification_lock_digest: request.publication.manifest.verification_lock_digest,
        };
        let response = self
            .http
            .readback_provider(gateway, &readback_request, current_time_ms()?)
            .map_err(map_transport_error)?;
        Ok(PublicationReadbackEvidenceV1 {
            provider: response.provider,
            location_id: response.location_id,
            replication_order: response.replication_order,
            commitment: response.commitment,
            semantic_release_digest: response.semantic_release_digest,
            verification_lock_digest: response.verification_lock_digest,
        })
    }
}

/// Loaded signer, runtime services, and public request bindings for one platform config.
pub struct LoadedProductionPublicationRuntimeV1<V> {
    signing: RegistrySigningClientV1,
    services: ProductionPublicationRuntimeV1<V>,
    bindings: ProductionPublicationBindingsV1,
}

impl<V> fmt::Debug for LoadedProductionPublicationRuntimeV1<V> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoadedProductionPublicationRuntimeV1")
            .field("publisher", self.signing.authority())
            .field("bindings", &self.bindings)
            .finish_non_exhaustive()
    }
}

impl<V> LoadedProductionPublicationRuntimeV1<V> {
    /// Return the public values needed to construct the immutable publication request.
    #[must_use]
    pub const fn bindings(&self) -> &ProductionPublicationBindingsV1 {
        &self.bindings
    }

    /// Split the loaded boundary into signer, runtime services, and public bindings.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        RegistrySigningClientV1,
        ProductionPublicationRuntimeV1<V>,
        ProductionPublicationBindingsV1,
    ) {
        (self.signing, self.services, self.bindings)
    }
}

/// Load the production runtime exclusively from an explicit or platform `client.toml`.
pub fn load_production_publication_runtime_v1<V>(
    config: Option<&Path>,
    validator: V,
) -> Result<LoadedProductionPublicationRuntimeV1<V>, ProductionPublicationConfigurationErrorV1>
where
    V: PublicationCleanPackageValidatorV1,
{
    let config_path =
        config.map_or_else(|| PathBuf::from(DEFAULT_CLIENT_CONFIG), Path::to_path_buf);
    let config_bytes =
        read_bounded_regular(&config_path, MAX_CLIENT_CONFIG_BYTES).map_err(|_| {
            ProductionPublicationConfigurationErrorV1::new("MUSUBI_PUBLICATION_CONFIG_INVALID")
        })?;
    let (signing, publication) =
        RegistrySigningClientV1::load_with_publication_config_bytes(&config_path, &config_bytes)
            .map_err(|_| {
                ProductionPublicationConfigurationErrorV1::new(
                    "MUSUBI_PUBLICATION_SIGNER_CONFIG_INVALID",
                )
            })?;
    let parsed = parse_publication_config(&config_path, &signing, &publication)?;
    let http = signing
        .publication_runtime_client(parsed.request_timeout)
        .map_err(|_| {
            ProductionPublicationConfigurationErrorV1::new(
                "MUSUBI_PUBLICATION_RUNTIME_AUTH_INVALID",
            )
        })?;
    let bindings = parsed.bindings.clone();
    let services = ProductionPublicationRuntimeV1 {
        signing: signing.clone(),
        http,
        validator,
        seed_ingress_url: parsed.seed_ingress_url,
        storage_coordinator_url: parsed.storage_coordinator_url,
        provider_gateways: parsed.provider_gateways,
        bindings: bindings.clone(),
    };
    Ok(LoadedProductionPublicationRuntimeV1 {
        signing,
        services,
        bindings,
    })
}

fn parse_publication_config(
    path: &Path,
    signing: &RegistrySigningClientV1,
    publication: &iroha::config::MusubiPublicationConfig,
) -> Result<ParsedProductionPublicationConfigV1, ProductionPublicationConfigurationErrorV1> {
    if publication.seed_ingress_url.is_none()
        && publication.storage_coordinator_url.is_none()
        && publication.ingress_broker.is_none()
        && publication.seed_provider.is_none()
        && publication.expected_policy_revision.is_none()
        && publication.request_timeout_ms.is_none()
        && publication.provider_gateways.is_empty()
        && publication.namespace_delegation_file.is_none()
    {
        return Err(ProductionPublicationConfigurationErrorV1::new(
            "MUSUBI_PUBLICATION_CONFIG_MISSING",
        ));
    }

    let seed_ingress_url = parse_service_url(required_config_string(
        publication.seed_ingress_url.as_deref(),
    )?)?;
    let storage_coordinator_url = parse_service_url(required_config_string(
        publication.storage_coordinator_url.as_deref(),
    )?)?;
    let ingress_broker = signing
        .parse_account_id(required_config_string(
            publication.ingress_broker.as_deref(),
        )?)
        .map_err(|_| {
            ProductionPublicationConfigurationErrorV1::new(
                "MUSUBI_PUBLICATION_INGRESS_BROKER_INVALID",
            )
        })?;
    let seed_provider = parse_provider_id(required_config_string(
        publication.seed_provider.as_deref(),
    )?)?;
    let expected_policy_revision = publication
        .expected_policy_revision
        .filter(|revision| *revision > 0)
        .ok_or_else(invalid_publication_config)?;
    let request_timeout_ms = publication
        .request_timeout_ms
        .map(|timeout| {
            if timeout == 0 {
                Err(invalid_publication_config())
            } else {
                Ok(timeout)
            }
        })
        .transpose()?
        .unwrap_or(DEFAULT_REQUEST_TIMEOUT_MS);
    if request_timeout_ms > MAX_REQUEST_TIMEOUT_MS {
        return Err(ProductionPublicationConfigurationErrorV1::new(
            "MUSUBI_PUBLICATION_TIMEOUT_INVALID",
        ));
    }
    let provider_gateways = parse_provider_gateways(&publication.provider_gateways)?;
    let namespace_delegation = publication
        .namespace_delegation_file
        .as_deref()
        .map(|raw| load_namespace_delegation(path, raw, signing))
        .transpose()?;
    Ok(ParsedProductionPublicationConfigV1 {
        seed_ingress_url,
        storage_coordinator_url,
        provider_gateways,
        request_timeout: Duration::from_millis(request_timeout_ms),
        bindings: ProductionPublicationBindingsV1 {
            ingress_broker,
            seed_provider,
            expected_policy_revision,
            namespace_delegation,
        },
    })
}

fn parse_provider_gateways(
    gateways: &[iroha::config::MusubiPublicationProviderGatewayConfig],
) -> Result<BTreeMap<ProviderId, Url>, ProductionPublicationConfigurationErrorV1> {
    if !(2..=MUSUBI_MAX_LOCATION_PROVIDERS_V1).contains(&gateways.len()) {
        return Err(ProductionPublicationConfigurationErrorV1::new(
            "MUSUBI_PUBLICATION_PROVIDER_GATEWAYS_INVALID",
        ));
    }
    let mut result = BTreeMap::new();
    let mut origins = BTreeSet::new();
    for value in gateways {
        let provider = parse_provider_id(required_config_string(Some(&value.provider_id))?)?;
        let url = parse_service_url(required_config_string(Some(&value.url))?)?;
        let origin = publication_service_origin(&url).ok_or_else(|| {
            ProductionPublicationConfigurationErrorV1::new(
                "MUSUBI_PUBLICATION_PROVIDER_GATEWAYS_INVALID",
            )
        })?;
        if result.insert(provider, url).is_some() || !origins.insert(origin) {
            return Err(ProductionPublicationConfigurationErrorV1::new(
                "MUSUBI_PUBLICATION_PROVIDER_GATEWAYS_INVALID",
            ));
        }
    }
    Ok(result)
}

fn load_namespace_delegation(
    config_path: &Path,
    configured_path: &str,
    signing: &RegistrySigningClientV1,
) -> Result<MusubiNamespaceDelegationV1, ProductionPublicationConfigurationErrorV1> {
    if configured_path.is_empty() || configured_path.trim() != configured_path {
        return Err(ProductionPublicationConfigurationErrorV1::new(
            "MUSUBI_PUBLICATION_DELEGATION_INVALID",
        ));
    }
    let path = Path::new(configured_path);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    };
    let bytes = read_bounded_regular(&resolved, MAX_DELEGATION_BYTES).map_err(|_| {
        ProductionPublicationConfigurationErrorV1::new("MUSUBI_PUBLICATION_DELEGATION_INVALID")
    })?;
    let delegation: MusubiNamespaceDelegationV1 =
        norito::decode_canonical_with_limits(&bytes, DELEGATION_DECODE_LIMITS).map_err(|_| {
            ProductionPublicationConfigurationErrorV1::new("MUSUBI_PUBLICATION_DELEGATION_INVALID")
        })?;
    delegation.validate().map_err(|_| {
        ProductionPublicationConfigurationErrorV1::new("MUSUBI_PUBLICATION_DELEGATION_INVALID")
    })?;
    if &delegation.payload.delegate != signing.authority() {
        return Err(ProductionPublicationConfigurationErrorV1::new(
            "MUSUBI_PUBLICATION_DELEGATION_DELEGATE_MISMATCH",
        ));
    }
    Ok(delegation)
}

fn parse_service_url(raw: &str) -> Result<Url, ProductionPublicationConfigurationErrorV1> {
    let url = raw.parse::<Url>().map_err(|_| {
        ProductionPublicationConfigurationErrorV1::new("MUSUBI_PUBLICATION_SERVICE_URL_INVALID")
    })?;
    validate_publication_service_base_url(&url).map_err(|_| {
        ProductionPublicationConfigurationErrorV1::new("MUSUBI_PUBLICATION_SERVICE_URL_INVALID")
    })?;
    Ok(url)
}

fn parse_provider_id(raw: &str) -> Result<ProviderId, ProductionPublicationConfigurationErrorV1> {
    if raw.len() != 64
        || raw
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(ProductionPublicationConfigurationErrorV1::new(
            "MUSUBI_PUBLICATION_PROVIDER_ID_INVALID",
        ));
    }
    let decoded = hex::decode(raw).map_err(|_| {
        ProductionPublicationConfigurationErrorV1::new("MUSUBI_PUBLICATION_PROVIDER_ID_INVALID")
    })?;
    let bytes = <[u8; 32]>::try_from(decoded).map_err(|_| {
        ProductionPublicationConfigurationErrorV1::new("MUSUBI_PUBLICATION_PROVIDER_ID_INVALID")
    })?;
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(ProductionPublicationConfigurationErrorV1::new(
            "MUSUBI_PUBLICATION_PROVIDER_ID_INVALID",
        ));
    }
    Ok(ProviderId::new(bytes))
}

fn required_config_string(
    value: Option<&str>,
) -> Result<&str, ProductionPublicationConfigurationErrorV1> {
    value
        .filter(|value| !value.is_empty() && value.trim() == *value)
        .ok_or_else(invalid_publication_config)
}

const fn invalid_publication_config() -> ProductionPublicationConfigurationErrorV1 {
    ProductionPublicationConfigurationErrorV1::new("MUSUBI_PUBLICATION_CONFIG_INVALID")
}

fn read_bounded_regular(path: &Path, maximum: u64) -> std::io::Result<Vec<u8>> {
    let path_before = fs::symlink_metadata(path)?;
    if metadata_is_link_or_reparse(&path_before) || !path_before.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "publication configuration input is not a real regular file",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;

        // Retain read sharing only. Denying write/delete sharing prevents a cooperating process
        // from replacing the selected configuration while this descriptor is authoritative.
        options.share_mode(0x0000_0001);
    }
    set_no_follow(&mut options);
    let mut file = options.open(path)?;
    let before = file.metadata()?;
    if metadata_is_link_or_reparse(&before)
        || !before.is_file()
        || before.len() == 0
        || before.len() > maximum
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "publication configuration input is not a bounded regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        if before.nlink() != 1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "publication configuration input must not be hard-linked",
            ));
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        if before.number_of_links() != Some(1)
            || before.volume_serial_number().is_none()
            || before.file_index().is_none()
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "publication configuration input must have one stable Windows file identity",
            ));
        }
    }
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    file.by_ref()
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let bytes_length = u64::try_from(bytes.len()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "publication configuration input exceeds the supported address width",
        )
    })?;
    let after = file.metadata()?;
    let path_after = fs::symlink_metadata(path)?;
    if bytes.is_empty()
        || bytes_length != before.len()
        || bytes_length > maximum
        || metadata_is_link_or_reparse(&path_after)
        || !same_file_snapshot(&path_before, &before)
        || !same_file_snapshot(&before, &after)
        || !same_file_snapshot(&after, &path_after)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "publication configuration input changed while it was read",
        ));
    }
    Ok(bytes)
}

fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink() || metadata_is_windows_reparse_point(metadata)
}

#[cfg(windows)]
fn metadata_is_windows_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    metadata.file_attributes() & 0x0000_0400 != 0
}

#[cfg(not(windows))]
const fn metadata_is_windows_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
        && left.nlink() == right.nlink()
}

#[cfg(windows)]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    left.volume_serial_number().is_some()
        && left.file_index().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
        && left.file_type() == right.file_type()
        && left.file_attributes() == right.file_attributes()
        && left.file_size() == right.file_size()
        && left.creation_time() == right.creation_time()
        && left.last_write_time() == right.last_write_time()
        && left.number_of_links() == Some(1)
        && right.number_of_links() == Some(1)
}

#[cfg(not(any(unix, windows)))]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

fn set_no_follow(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(platform_no_follow_flag());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;
        // FILE_FLAG_OPEN_REPARSE_POINT makes the final component itself the open target. A
        // symlink/reparse point consequently fails the regular-file check above.
        options.custom_flags(0x0020_0000);
    }
    #[cfg(not(any(unix, windows)))]
    let _ = options;
}

#[cfg(any(target_os = "linux", target_os = "android"))]
const fn platform_no_follow_flag() -> i32 {
    0o400000
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
const fn platform_no_follow_flag() -> i32 {
    0x100
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
const fn platform_no_follow_flag() -> i32 {
    // TODO: Add an atomic no-follow flag when the remaining Unix targets expose one through the
    // existing platform API. These targets still reject symlink metadata and require the opened
    // descriptor to match the pre-open path identity, but are outside the qualified production set.
    0
}

fn current_time_ms() -> Result<u64, PublicationBackendError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| PublicationBackendError::permanent("SYSTEM_TIME_INVALID"))?;
    u64::try_from(elapsed.as_millis())
        .map_err(|_| PublicationBackendError::permanent("SYSTEM_TIME_INVALID"))
}

fn map_transport_error(error: MusubiPublicationRuntimeTransportErrorV1) -> PublicationBackendError {
    match error.class() {
        MusubiPublicationRuntimeTransportFailureClassV1::Retryable => {
            PublicationBackendError::retryable(error.code())
        }
        MusubiPublicationRuntimeTransportFailureClassV1::Permanent => {
            PublicationBackendError::permanent(error.code())
        }
    }
}

fn map_registry_error(error: crate::registry::RegistryErrorV1) -> PublicationBackendError {
    match error.class() {
        RegistryFailureClassV1::Retryable | RegistryFailureClassV1::StaleCursor => {
            PublicationBackendError::retryable("MUSUBI_PUBLICATION_REGISTRY_RETRYABLE")
        }
        RegistryFailureClassV1::Permanent | RegistryFailureClassV1::NotFound => {
            PublicationBackendError::permanent("MUSUBI_PUBLICATION_REGISTRY_REJECTED")
        }
    }
}

#[cfg(test)]
mod tests {
    use iroha::crypto::{Algorithm, ExposedPrivateKey, KeyPair, SignatureOf};
    use iroha_data_model::musubi::{
        MusubiNamespaceBindingDigestV1, MusubiNamespaceDelegationApprovalV1,
        MusubiNamespaceDelegationPayloadV1,
    };
    use tempfile::tempdir;

    use super::*;

    fn write_client_config(
        path: &Path,
        extra: &str,
    ) -> (
        RegistrySigningClientV1,
        iroha::config::MusubiPublicationConfig,
    ) {
        let key_pair =
            KeyPair::try_from_seed(vec![0x51; 32], Algorithm::Ed25519).expect("derive fixture key");
        let private_key = ExposedPrivateKey(key_pair.private_key().clone()).to_string();
        fs::write(
            path,
            format!(
                r#"
                    chain = "musubi-publication-runtime-test"
                    torii_url = "https://torii.example/"
                    [account]
                    domain = "packages.universal"
                    profile = "taira"
                    public_key = "{}"
                    private_key = "{}"

                    [musubi.publication]
                    seed_ingress_url = "https://seed.example/private/"
                    storage_coordinator_url = "https://storage.example/private/"
                    ingress_broker = "{}"
                    seed_provider = "{}"
                    expected_policy_revision = 7
                    request_timeout_ms = 5000
                    provider_gateways = [
                      {{ provider_id = "{}", url = "https://provider-a.example/" }},
                      {{ provider_id = "{}", url = "https://provider-b.example/" }},
                    ]
                    {extra}
                "#,
                key_pair.public_key(),
                private_key,
                {
                    let account =
                        iroha_data_model::account::AccountId::new(key_pair.public_key().clone());
                    let _guard =
                        iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
                    account.canonical_i105().expect("Taira account")
                },
                hex::encode([0x11; 32]),
                hex::encode([0x21; 32]),
                hex::encode([0x22; 32]),
            ),
        )
        .expect("write config");
        RegistrySigningClientV1::load_with_publication_config(Some(path))
            .expect("load signer with typed publication config")
    }

    #[test]
    fn platform_config_returns_only_public_request_bindings() {
        let temporary = tempdir().expect("temporary directory");
        let path = temporary.path().join("client.toml");
        let (signing, publication) = write_client_config(&path, "");
        let parsed = parse_publication_config(&path, &signing, &publication)
            .expect("parse publication config");
        assert_eq!(parsed.bindings.seed_provider, ProviderId::new([0x11; 32]));
        assert_eq!(parsed.bindings.expected_policy_revision, 7);
        assert_eq!(parsed.provider_gateways.len(), 2);
        assert!(parsed.bindings.namespace_delegation.is_none());
        let debug = format!("{parsed:?}");
        assert!(!debug.contains("seed.example"));
        assert!(!debug.contains("storage.example"));
        let platform_debug = format!("{signing:?} {publication:?}");
        assert!(!platform_debug.contains("seed.example"));
        assert!(!platform_debug.contains("storage.example"));
        assert!(!platform_debug.contains("provider-a.example"));
        let fixture_key =
            KeyPair::try_from_seed(vec![0x51; 32], Algorithm::Ed25519).expect("fixture key");
        let fixture_private = ExposedPrivateKey(fixture_key.private_key().clone()).to_string();
        assert!(!platform_debug.contains(&fixture_private));
    }

    #[cfg(unix)]
    #[test]
    fn production_loader_rejects_linked_configuration_inputs() {
        use std::os::unix::fs::symlink;

        let temporary = tempdir().expect("temporary directory");
        let target = temporary.path().join("target.toml");
        let (_signing, _publication) = write_client_config(&target, "");

        let symbolic = temporary.path().join("symbolic.toml");
        symlink(&target, &symbolic).expect("create symbolic link");
        let symbolic_error = load_production_publication_runtime_v1(
            Some(&symbolic),
            UnavailablePublicationCleanPackageValidatorV1,
        )
        .expect_err("symbolic configuration must fail closed");
        assert_eq!(symbolic_error.code(), "MUSUBI_PUBLICATION_CONFIG_INVALID");

        let hard = temporary.path().join("hard.toml");
        fs::hard_link(&target, &hard).expect("create hard link");
        let hard_error = load_production_publication_runtime_v1(
            Some(&hard),
            UnavailablePublicationCleanPackageValidatorV1,
        )
        .expect_err("hard-linked configuration must fail closed");
        assert_eq!(hard_error.code(), "MUSUBI_PUBLICATION_CONFIG_INVALID");
    }

    #[cfg(windows)]
    #[test]
    fn production_loader_rejects_hard_linked_windows_configuration() {
        let temporary = tempdir().expect("temporary directory");
        let target = temporary.path().join("target.toml");
        let (_signing, _publication) = write_client_config(&target, "");
        let hard = temporary.path().join("hard.toml");
        fs::hard_link(&target, &hard).expect("create hard link");

        let error = load_production_publication_runtime_v1(
            Some(&hard),
            UnavailablePublicationCleanPackageValidatorV1,
        )
        .expect_err("hard-linked Windows configuration must fail closed");
        assert_eq!(error.code(), "MUSUBI_PUBLICATION_CONFIG_INVALID");
    }

    #[test]
    fn provider_gateways_must_have_distinct_provider_ids_and_origins() {
        let gateways = vec![
            iroha::config::MusubiPublicationProviderGatewayConfig {
                provider_id: "1111111111111111111111111111111111111111111111111111111111111111"
                    .to_owned(),
                url: "https://same.example/".to_owned(),
            },
            iroha::config::MusubiPublicationProviderGatewayConfig {
                provider_id: "2222222222222222222222222222222222222222222222222222222222222222"
                    .to_owned(),
                url: "https://same.example/other/".to_owned(),
            },
        ];
        assert!(parse_provider_gateways(&gateways).is_err());
    }

    #[test]
    fn unknown_fields_and_retired_upload_routes_fail_closed() {
        let temporary = tempdir().expect("temporary directory");
        let path = temporary.path().join("client.toml");
        let (_signing, _publication) = write_client_config(&path, "");
        let valid = fs::read_to_string(&path).expect("read valid client config");
        fs::write(&path, format!("{valid}\nunexpected = true\n"))
            .expect("write unknown publication field");
        assert!(
            RegistrySigningClientV1::load(Some(&path)).is_err(),
            "platform client config must reject unknown Musubi publication fields"
        );
        assert!(parse_service_url("https://seed.example/v1/sorafs/upload/").is_err());
    }

    #[test]
    fn public_namespace_delegation_file_is_bounded_and_delegate_bound() {
        let temporary = tempdir().expect("temporary directory");
        let config_path = temporary.path().join("client.toml");
        let delegation_path = temporary.path().join("delegation.to");
        let (signing, publication) = write_client_config(
            &config_path,
            r#"namespace_delegation_file = "delegation.to""#,
        );
        let owner_key_pair =
            KeyPair::try_from_seed(vec![0x61; 32], Algorithm::Ed25519).expect("owner key");
        let payload = MusubiNamespaceDelegationPayloadV1 {
            version: 1,
            namespace_binding: MusubiNamespaceBindingDigestV1::new([0x62; 32]),
            owner_generation: 4,
            owner: iroha_data_model::account::AccountId::new(owner_key_pair.public_key().clone()),
            delegate: signing.authority().clone(),
            expires_at_height: 10_000,
        };
        let delegation = MusubiNamespaceDelegationV1 {
            approvals: vec![MusubiNamespaceDelegationApprovalV1 {
                public_key: owner_key_pair.public_key().clone(),
                signature: SignatureOf::try_from_hash(
                    owner_key_pair.private_key(),
                    payload.signing_hash(),
                )
                .expect("sign delegation"),
            }],
            payload,
        };
        fs::write(
            &delegation_path,
            norito::encode_canonical(&delegation).expect("encode delegation"),
        )
        .expect("write delegation");
        let parsed = parse_publication_config(&config_path, &signing, &publication)
            .expect("load public namespace delegation");
        assert_eq!(
            parsed.bindings.namespace_delegation.as_ref(),
            Some(&delegation)
        );

        let mut mismatched = delegation;
        mismatched.payload.delegate = mismatched.payload.owner.clone();
        fs::write(
            &delegation_path,
            norito::encode_canonical(&mismatched).expect("encode mismatched delegation"),
        )
        .expect("rewrite delegation");
        let error = parse_publication_config(&config_path, &signing, &publication)
            .expect_err("delegation for a different actor must fail");
        assert_eq!(
            error.code(),
            "MUSUBI_PUBLICATION_DELEGATION_DELEGATE_MISMATCH"
        );
    }
}
