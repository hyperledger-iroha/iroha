//! First-release Musubi registry, signing, and production-publication boundary.
//!
//! Public finalized reads retain only a Torii URL and never construct an account or
//! key pair. Mutations load a required Iroha `client.toml` only when the mutation is
//! dispatched, then sign the concrete V1 instruction locally. Publication delegates
//! clean-package validation, authenticated seed ingress, pin/order coordination, and
//! provider readback to an explicit runtime service; the default service fails closed.
//! Archive registration itself is not delegated: the signer prebuilds, fee-quotes, and
//! signs one exact transaction, the publication journal persists it before submission,
//! and recovery pairs that transaction identity with the authoritative archive embedded
//! in a finalized archive-location page before any storage coordination begins.

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
    config::{Config, resolve_account_chain_discriminant},
};
use iroha_data_model::{
    account::address::ChainDiscriminantGuard,
    isi::{InstructionBox, musubi::PublishMusubiReleaseV1},
    metadata::Metadata,
    musubi::{
        MUSUBI_MAX_PAGE_SIZE_V1, MUSUBI_MIN_HEALTHY_REPLICAS_V1, MusubiAliasHistoryPageV1,
        MusubiAliasQueryV1, MusubiAliasRecordV1, MusubiArchiveLocationIdV1,
        MusubiArchiveLocationPageV1, MusubiArchiveLocationQueryV1, MusubiArchiveLocationStateV1,
        MusubiArchiveLocationV1, MusubiArchiveRetentionDispositionV1, MusubiArchiveRetentionPageV1,
        MusubiArchiveRetentionQueryV1, MusubiExactPackageQueryV1, MusubiExactReleaseQueryV1,
        MusubiMaintainerPageV1, MusubiNamespaceBindingV1, MusubiOrderedPackagePageV1,
        MusubiOrderedPrefixQueryV1, MusubiOrderedPrefixV1, MusubiPackageIdV1,
        MusubiPackagePageQueryV1, MusubiPackageRecordV1, MusubiPackageSelectorV1,
        MusubiPageRequestV1, MusubiReleaseIdV1, MusubiReleaseRecordV1, MusubiResolverIndexPageV1,
        MusubiResolverIndexQueryV1, MusubiSearchPageV1, MusubiSearchQueryV1,
        MusubiSeedIngressReceiptBindingV1, MusubiSeedIngressReceiptV1, MusubiVersionPageV1,
        MusubiVersionReqV1,
    },
    sorafs::capacity::ProviderId,
    transaction::{FeePaymentIntent, SignedTransaction, TransactionPayload},
};
use norito::json::{JsonDeserialize, JsonSerialize};
use url::Url;

use crate::publish::{
    PublicationAdvanceV1, PublicationAmxSubmissionV1, PublicationArchiveAbsenceEvidenceV1,
    PublicationArchiveLocationAdvanceV1, PublicationArchiveLocationIntentV1,
    PublicationArchiveLocationTerminalReasonV1, PublicationArchiveLocationTerminalV1,
    PublicationArchiveRegistrationAdvanceV1, PublicationArchiveRegistrationIntentV1,
    PublicationArchiveRegistrationTerminalV1, PublicationArchiveRegistrationV1, PublicationBackend,
    PublicationBackendError, PublicationBackendFailureClass, PublicationCarSource,
    PublicationEngine, PublicationError, PublicationFinalEvidenceV1, PublicationOperationIdV1,
    PublicationReadbackEvidenceV1, PublicationRegisteredArchiveV1, PublicationReplicationAdvanceV1,
    PublicationRequestV1, PublicationValidationEvidenceV1,
    archive_registration_intent_valid_until_ms,
};

const DEFAULT_CLIENT_CONFIG: &str = "client.toml";
const MAX_PUBLIC_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_PUBLIC_CONFIG_BYTES_USIZE: usize = 1024 * 1024;
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

/// Authoritative terminal state for one exact signed transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RegistryTerminalTransactionStateV1 {
    /// The transaction was finalized with a rejection.
    Rejected,
    /// The transaction expired without application.
    Expired,
}

/// Exact transaction state recovered from the typed Torii status endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RegistryTransactionStateV1 {
    /// No authoritative or pending record is currently visible.
    Absent,
    /// The transaction is pending or only has a non-authoritative terminal hint.
    Pending,
    /// The transaction is durably applied at the reported block height.
    Applied {
        /// Applied block height from authoritative state.
        block_height: u64,
    },
    /// The transaction has an authoritative terminal negative state.
    Terminal {
        /// Terminal kind.
        kind: RegistryTerminalTransactionStateV1,
        /// Finalized rejection height, absent for expiry without block inclusion.
        block_height: Option<u64>,
    },
}

/// Observed result of submitting and status-checking one locally signed mutation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RegistryMutationSubmissionV1 {
    /// The exact transaction is applied.
    Applied {
        /// Locally derived exact signed-transaction hash.
        transaction_hash: [u8; 32],
        /// Applied block height from authoritative state.
        block_height: u64,
    },
    /// The exact transaction has no authoritative terminal result yet.
    Pending {
        /// Locally derived exact signed-transaction hash.
        transaction_hash: [u8; 32],
    },
    /// The exact transaction reached an authoritative negative terminal state.
    Terminal {
        /// Locally derived exact signed-transaction hash.
        transaction_hash: [u8; 32],
        /// Terminal kind.
        kind: RegistryTerminalTransactionStateV1,
        /// Finalized rejection height, absent for expiry without block inclusion.
        block_height: Option<u64>,
    },
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
    account_chain_discriminant: u16,
}

impl RegistryReadClientV1 {
    /// Construct a signer-free client from an already validated public Torii URL.
    pub fn new(
        torii_url: Url,
        timeout: Duration,
        account_chain_discriminant: u16,
    ) -> Result<Self, RegistryErrorV1> {
        if !matches!(torii_url.scheme(), "http" | "https")
            || !torii_url.username().is_empty()
            || torii_url.password().is_some()
            || timeout == Duration::ZERO
            || timeout > Duration::from_secs(60)
            || account_chain_discriminant == 0
        {
            return Err(RegistryErrorV1::new(
                RegistryFailureClassV1::Permanent,
                "MUSUBI_REGISTRY_PUBLIC_CONFIG_INVALID",
            ));
        }
        Ok(Self {
            torii_url,
            timeout,
            account_chain_discriminant,
        })
    }

    /// Load only public endpoint/network context from `--config` or platform `client.toml`.
    ///
    /// Only `torii_url`, timeout, and `[account].profile`/`chain_discriminant` are interpreted.
    /// Account identity, private-key, bearer-token, and basic-auth fields are neither parsed into
    /// typed forms nor retained. The default path is the same required `client.toml` used by the
    /// Iroha CLI; project manifests and command-line credential values are never consulted.
    pub fn load(config: Option<&Path>) -> Result<Self, RegistryErrorV1> {
        let path = config.map_or_else(|| PathBuf::from(DEFAULT_CLIENT_CONFIG), Path::to_path_buf);
        let text = read_bounded_config(&path)?;
        Self::load_from_config_bytes(text.as_bytes())
    }

    /// Parse public endpoint and network context from one already-read `client.toml` image.
    ///
    /// Publication uses this entry point so the signer, private runtime client, and finalized
    /// registry reader all derive from the same bounded file image instead of reopening a path
    /// after authenticated storage coordination has started.
    pub(crate) fn load_from_config_bytes(bytes: &[u8]) -> Result<Self, RegistryErrorV1> {
        if bytes.is_empty() || bytes.len() > MAX_PUBLIC_CONFIG_BYTES_USIZE {
            return Err(invalid_public_config());
        }
        let text = std::str::from_utf8(bytes).map_err(|_| invalid_public_config())?;
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
        let account = match document.get("account") {
            Some(value) => Some(value.as_table().ok_or_else(invalid_public_config)?),
            None => None,
        };
        let profile = match account.and_then(|account| account.get("profile")) {
            Some(value) => Some(value.as_str().ok_or_else(invalid_public_config)?),
            None => None,
        };
        let explicit_discriminant =
            match account.and_then(|account| account.get("chain_discriminant")) {
                Some(value) => {
                    let value = value.as_integer().ok_or_else(invalid_public_config)?;
                    Some(u16::try_from(value).map_err(|_| invalid_public_config())?)
                }
                None => None,
            };
        let account_chain_discriminant =
            resolve_account_chain_discriminant(profile, explicit_discriminant)
                .map_err(|_| invalid_public_config())?;
        Self::new(torii_url, timeout, account_chain_discriminant)
    }

    /// Return the configured public endpoint. No account or credential material is retained.
    #[must_use]
    pub const fn torii_url(&self) -> &Url {
        &self.torii_url
    }

    /// Return the validated public I105 account chain discriminant without loading a signer.
    #[must_use]
    pub const fn account_chain_discriminant(&self) -> u16 {
        self.account_chain_discriminant
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

    /// Bind a canonical selector through its immutable namespace even before first publication.
    ///
    /// Unlike [`Self::resolve_selector`], this queries the namespace directory prefix and does
    /// not require a package row to exist. This is the package/publication boundary for claiming
    /// a previously absent package under an already-registered namespace.
    pub fn bind_selector_namespace(
        &self,
        selector: &MusubiPackageSelectorV1,
    ) -> Result<MusubiPackageIdV1, RegistryErrorV1> {
        selector.validate().map_err(|_| invalid_response())?;
        let prefix = MusubiOrderedPrefixV1::new(&format!("{}/", selector.namespace))
            .map_err(|_| invalid_response())?;
        let page = self.ordered_prefix(&MusubiOrderedPrefixQueryV1 {
            prefix,
            page: first_page(1),
        })?;
        package_id_from_namespace_binding(selector, &page.namespace_binding)
    }

    /// Fetch and validate one exact authoritative package record.
    pub fn exact_package(
        &self,
        package: MusubiPackageIdV1,
    ) -> Result<Option<MusubiPackageRecordV1>, RegistryErrorV1> {
        let requested_package = package.clone();
        let output = self.query_optional::<_, MusubiPackageRecordV1>(
            PublicMusubiQueryPathV1::ExactPackage,
            &MusubiExactPackageQueryV1 { package },
        )?;
        if let Some(record) = &output {
            record.validate().map_err(|_| invalid_response())?;
            if record.package != requested_package {
                return Err(invalid_response());
            }
        }
        Ok(output)
    }

    /// Fetch and validate one exact immutable release record.
    pub fn exact_release(
        &self,
        release: MusubiReleaseIdV1,
    ) -> Result<Option<MusubiReleaseRecordV1>, RegistryErrorV1> {
        let requested_release = release.clone();
        let output = self.query_optional::<_, MusubiReleaseRecordV1>(
            PublicMusubiQueryPathV1::ExactRelease,
            &MusubiExactReleaseQueryV1 { release },
        )?;
        if let Some(record) = &output {
            record.validate().map_err(|_| invalid_response())?;
            if record.manifest.release != requested_release {
                return Err(invalid_response());
            }
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
        page.validate_for(request).map_err(|_| invalid_response())?;
        Ok(page)
    }

    /// Fetch and validate one finalized package-version page.
    pub fn versions(
        &self,
        request: &MusubiPackagePageQueryV1,
    ) -> Result<MusubiVersionPageV1, RegistryErrorV1> {
        let page = self
            .query_required::<_, MusubiVersionPageV1>(PublicMusubiQueryPathV1::Versions, request)?;
        page.validate_for(request).map_err(|_| invalid_response())?;
        Ok(page)
    }

    /// Fetch and validate one finalized accepted-member and pending-invitation page.
    pub fn maintainers(
        &self,
        request: &MusubiPackagePageQueryV1,
    ) -> Result<MusubiMaintainerPageV1, RegistryErrorV1> {
        let page = self.query_required::<_, MusubiMaintainerPageV1>(
            PublicMusubiQueryPathV1::Maintainers,
            request,
        )?;
        validate_maintainer_page(request, &page)?;
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
            if page.archive.archive_id != request.archive_id {
                return Err(invalid_response());
            }
            for location in &page.items {
                location.validate().map_err(|_| invalid_response())?;
                if location.archive_id != request.archive_id {
                    return Err(invalid_response());
                }
            }
        }
        Ok(page)
    }

    /// Fetch and validate exact finalized cache-retention decisions for one bounded batch.
    pub fn archive_retention(
        &self,
        request: &MusubiArchiveRetentionQueryV1,
    ) -> Result<MusubiArchiveRetentionPageV1, RegistryErrorV1> {
        request.validate().map_err(|_| {
            RegistryErrorV1::new(
                RegistryFailureClassV1::Permanent,
                "MUSUBI_REGISTRY_RETENTION_REQUEST_INVALID",
            )
        })?;
        let page = self.query_required::<_, MusubiArchiveRetentionPageV1>(
            PublicMusubiQueryPathV1::ArchiveRetention,
            request,
        )?;
        page.validate().map_err(|_| invalid_response())?;
        if request
            .expected_snapshot
            .is_some_and(|expected| expected != page.snapshot)
            || page
                .items
                .iter()
                .map(|decision| decision.archive_id)
                .ne(request.archive_ids.iter().copied())
        {
            return Err(invalid_response());
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
        page.validate_for(request).map_err(|_| invalid_response())?;
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
        page.validate_for(request).map_err(|_| invalid_response())?;
        Ok(page)
    }

    /// Search the rebuildable finalized-event metadata projection by exact normalized terms.
    ///
    /// This discovery API is intentionally separate from [`Self::resolver_index`]; callers
    /// must resolve a selected structural package through the universal sparse index.
    pub fn search(
        &self,
        request: &MusubiSearchQueryV1,
    ) -> Result<MusubiSearchPageV1, RegistryErrorV1> {
        request.validate().map_err(|_| {
            RegistryErrorV1::new(
                RegistryFailureClassV1::Permanent,
                "MUSUBI_REGISTRY_SEARCH_REQUEST_INVALID",
            )
        })?;
        let page =
            self.query_required::<_, MusubiSearchPageV1>(PublicMusubiQueryPathV1::Search, request)?;
        page.validate_for(request).map_err(|_| invalid_response())?;
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
        let _chain_discriminant = ChainDiscriminantGuard::enter(self.account_chain_discriminant);
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
    account_chain_discriminant: u16,
}

impl fmt::Debug for RegistrySigningClientV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _chain_discriminant = ChainDiscriminantGuard::enter(self.account_chain_discriminant);
        formatter
            .debug_struct("RegistrySigningClientV1")
            .field("authority", &self.client.account)
            .field(
                "account_chain_discriminant",
                &self.account_chain_discriminant,
            )
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
        Ok(Self::from_configuration(configuration))
    }

    pub(crate) fn load_with_publication_config(
        config: Option<&Path>,
    ) -> Result<(Self, iroha::config::MusubiPublicationConfig), RegistryErrorV1> {
        let path = config.map_or_else(|| PathBuf::from(DEFAULT_CLIENT_CONFIG), Path::to_path_buf);
        let (configuration, publication) = Config::load_file_with_musubi_publication(path)
            .map_err(|_| {
                RegistryErrorV1::new(
                    RegistryFailureClassV1::Permanent,
                    "MUSUBI_REGISTRY_SIGNING_CONFIG_INVALID",
                )
            })?;
        Ok((Self::from_configuration(configuration), publication))
    }

    pub(crate) fn load_with_publication_config_bytes(
        path: &Path,
        bytes: &[u8],
    ) -> Result<(Self, iroha::config::MusubiPublicationConfig), RegistryErrorV1> {
        let (configuration, publication) = Config::load_bytes_with_musubi_publication(path, bytes)
            .map_err(|_| {
                RegistryErrorV1::new(
                    RegistryFailureClassV1::Permanent,
                    "MUSUBI_REGISTRY_SIGNING_CONFIG_INVALID",
                )
            })?;
        Ok((Self::from_configuration(configuration), publication))
    }

    fn from_configuration(configuration: Config) -> Self {
        let account_chain_discriminant = configuration.account_chain_discriminant;
        let mut client = {
            let _chain_discriminant = ChainDiscriminantGuard::enter(account_chain_discriminant);
            Client::new(configuration)
        };
        client.torii_request_timeout = client.torii_request_timeout.min(Duration::from_secs(60));
        Self {
            client,
            account_chain_discriminant,
        }
    }

    /// Return the configured mutation authority.
    #[must_use]
    pub const fn authority(&self) -> &iroha_data_model::account::AccountId {
        &self.client.account
    }

    /// Return the validated I105 account chain discriminant used by this signer.
    #[must_use]
    pub const fn account_chain_discriminant(&self) -> u16 {
        self.account_chain_discriminant
    }

    /// Construct the fixed private-publication HTTPS client from this signer.
    ///
    /// Only the chain, account, and key pair are copied. Torii Basic Auth and configured
    /// headers are deliberately excluded from the private publication service boundary.
    pub fn publication_runtime_client(
        &self,
        timeout: Duration,
    ) -> Result<
        iroha::musubi_runtime::AuthenticatedMusubiPublicationRuntimeClientV1,
        iroha::musubi_runtime::MusubiPublicationRuntimeTransportErrorV1,
    > {
        iroha::musubi_runtime::AuthenticatedMusubiPublicationRuntimeClientV1::from_iroha_client(
            &self.client,
            timeout,
        )
    }

    /// Parse one canonical account argument under the signing client's network profile.
    ///
    /// The scoped override is thread-local and is removed before returning. No key material is
    /// accepted, retained, or exposed by this method.
    pub fn parse_account_id(
        &self,
        input: &str,
    ) -> Result<iroha_data_model::account::AccountId, RegistryErrorV1> {
        let _chain_discriminant = ChainDiscriminantGuard::enter(self.account_chain_discriminant);
        iroha_data_model::account::AccountId::parse_encoded(input)
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .map_err(|_| {
                RegistryErrorV1::new(
                    RegistryFailureClassV1::Permanent,
                    "MUSUBI_ACCOUNT_ID_INVALID",
                )
            })
    }

    /// Sign, submit, and wait for commitment of one concrete V1 instruction.
    pub fn submit_v1<I>(&self, instruction: I) -> Result<[u8; 32], RegistryErrorV1>
    where
        I: Into<InstructionBox>,
    {
        match self.submit_observed_v1(instruction)? {
            RegistryMutationSubmissionV1::Applied {
                transaction_hash, ..
            } => Ok(transaction_hash),
            RegistryMutationSubmissionV1::Pending { .. } => Err(RegistryErrorV1::new(
                RegistryFailureClassV1::Retryable,
                "MUSUBI_REGISTRY_TRANSACTION_PENDING",
            )),
            RegistryMutationSubmissionV1::Terminal {
                kind: RegistryTerminalTransactionStateV1::Expired,
                ..
            } => Err(RegistryErrorV1::new(
                RegistryFailureClassV1::Retryable,
                "MUSUBI_REGISTRY_TRANSACTION_EXPIRED",
            )),
            RegistryMutationSubmissionV1::Terminal {
                kind: RegistryTerminalTransactionStateV1::Rejected,
                ..
            } => Err(RegistryErrorV1::new(
                RegistryFailureClassV1::Permanent,
                "MUSUBI_REGISTRY_TRANSACTION_TERMINAL_FAILURE",
            )),
        }
    }

    pub(crate) fn submit_observed_v1<I>(
        &self,
        instruction: I,
    ) -> Result<RegistryMutationSubmissionV1, RegistryErrorV1>
    where
        I: Into<InstructionBox>,
    {
        let payload = self.prebuild_v1(instruction)?;
        let transaction = self.quote_and_sign_v1(payload)?;
        let transaction_hash = *transaction.hash().as_ref();
        let submission = self.submit_signed_v1(&transaction);
        if let Ok(observed_hash) = submission {
            validate_mutation_submission_hash(transaction_hash, observed_hash)?;
        }
        let state = match self.transaction_application_state_v1(&transaction) {
            Ok(state) => state,
            Err(status_error) => {
                return Err(match submission {
                    Err(submission_error)
                        if status_error.class() != RegistryFailureClassV1::Permanent =>
                    {
                        submission_error
                    }
                    _ => status_error,
                });
            }
        };
        match state {
            RegistryTransactionStateV1::Applied { block_height } => {
                Ok(RegistryMutationSubmissionV1::Applied {
                    transaction_hash,
                    block_height,
                })
            }
            RegistryTransactionStateV1::Pending => {
                Ok(RegistryMutationSubmissionV1::Pending { transaction_hash })
            }
            RegistryTransactionStateV1::Absent => match submission {
                Ok(_) => Ok(RegistryMutationSubmissionV1::Pending { transaction_hash }),
                Err(error) => Err(error),
            },
            RegistryTransactionStateV1::Terminal { kind, block_height } => {
                Ok(RegistryMutationSubmissionV1::Terminal {
                    transaction_hash,
                    kind,
                    block_height,
                })
            }
        }
    }

    /// Prebuild one exact unsigned V1 mutation payload without contacting Torii.
    pub fn prebuild_v1<I>(&self, instruction: I) -> Result<TransactionPayload, RegistryErrorV1>
    where
        I: Into<InstructionBox>,
    {
        let _chain_discriminant = ChainDiscriminantGuard::enter(self.account_chain_discriminant);
        let instruction: InstructionBox = instruction.into();
        self.client
            .try_build_transaction_payload_from_items(
                [instruction],
                FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            )
            .map_err(|_| {
                RegistryErrorV1::new(
                    RegistryFailureClassV1::Permanent,
                    "MUSUBI_REGISTRY_TRANSACTION_BUILD_FAILED",
                )
            })
    }

    /// Fee-quote and sign the exact prebuilt payload without submitting it.
    pub fn quote_and_sign_v1(
        &self,
        payload: TransactionPayload,
    ) -> Result<SignedTransaction, RegistryErrorV1> {
        let _chain_discriminant = ChainDiscriminantGuard::enter(self.account_chain_discriminant);
        self.client
            .quote_and_sign_transaction_payload(payload)
            .map_err(|_| {
                RegistryErrorV1::new(
                    RegistryFailureClassV1::Retryable,
                    "MUSUBI_REGISTRY_TRANSACTION_QUOTE_OR_SIGN_FAILED",
                )
            })
    }

    /// Submit and wait for the exact already-signed V1 transaction without rebuilding it.
    pub fn submit_signed_v1(
        &self,
        transaction: &SignedTransaction,
    ) -> Result<[u8; 32], RegistryErrorV1> {
        let _chain_discriminant = ChainDiscriminantGuard::enter(self.account_chain_discriminant);
        let hash = self
            .client
            .submit_transaction_blocking(transaction)
            .map_err(|_| {
                RegistryErrorV1::new(
                    RegistryFailureClassV1::Retryable,
                    "MUSUBI_REGISTRY_MUTATION_FAILED",
                )
            })?;
        Ok(*hash.as_ref())
    }

    pub(crate) fn transaction_application_state_v1(
        &self,
        transaction: &SignedTransaction,
    ) -> Result<RegistryTransactionStateV1, RegistryErrorV1> {
        let _chain_discriminant = ChainDiscriminantGuard::enter(self.account_chain_discriminant);
        let response = self
            .client
            .get_transaction_status_response(transaction.hash())
            .map_err(|_| {
                RegistryErrorV1::new(
                    RegistryFailureClassV1::Retryable,
                    "MUSUBI_REGISTRY_TRANSACTION_STATUS_FAILED",
                )
            })?;
        let Some(response) = response else {
            return Ok(RegistryTransactionStateV1::Absent);
        };
        if response.hash != transaction.hash().to_string() {
            return Err(RegistryErrorV1::new(
                RegistryFailureClassV1::Permanent,
                "MUSUBI_REGISTRY_TRANSACTION_STATUS_HASH_MISMATCH",
            ));
        }
        match response.status.kind.as_str() {
            "Applied" => response
                .status
                .block_height
                .filter(|height| *height > 0)
                .map(|block_height| RegistryTransactionStateV1::Applied { block_height })
                .ok_or_else(|| {
                    RegistryErrorV1::new(
                        RegistryFailureClassV1::Permanent,
                        "MUSUBI_REGISTRY_TRANSACTION_STATUS_INVALID",
                    )
                }),
            "Queued" | "Approved" | "Committed" => Ok(RegistryTransactionStateV1::Pending),
            "Rejected" if response.resolved_from != "state" => {
                Ok(RegistryTransactionStateV1::Pending)
            }
            "Rejected" => response
                .status
                .block_height
                .filter(|height| *height > 0)
                .map(|block_height| RegistryTransactionStateV1::Terminal {
                    kind: RegistryTerminalTransactionStateV1::Rejected,
                    block_height: Some(block_height),
                })
                .ok_or_else(|| {
                    RegistryErrorV1::new(
                        RegistryFailureClassV1::Permanent,
                        "MUSUBI_REGISTRY_TRANSACTION_STATUS_INVALID",
                    )
                }),
            "Expired" if response.resolved_from != "state" => {
                Ok(RegistryTransactionStateV1::Pending)
            }
            "Expired" => {
                if response.status.block_height == Some(0) {
                    return Err(RegistryErrorV1::new(
                        RegistryFailureClassV1::Permanent,
                        "MUSUBI_REGISTRY_TRANSACTION_STATUS_INVALID",
                    ));
                }
                Ok(RegistryTransactionStateV1::Terminal {
                    kind: RegistryTerminalTransactionStateV1::Expired,
                    block_height: response.status.block_height,
                })
            }
            _ => Err(RegistryErrorV1::new(
                RegistryFailureClassV1::Permanent,
                "MUSUBI_REGISTRY_TRANSACTION_STATUS_INVALID",
            )),
        }
    }
}

fn validate_mutation_submission_hash(
    expected_hash: [u8; 32],
    observed_hash: [u8; 32],
) -> Result<(), RegistryErrorV1> {
    if observed_hash != expected_hash {
        return Err(RegistryErrorV1::new(
            RegistryFailureClassV1::Permanent,
            "MUSUBI_REGISTRY_TRANSACTION_HASH_MISMATCH",
        ));
    }
    Ok(())
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

    /// Coordinate and sign an exact location CAS without submitting it.
    ///
    /// The caller journals the returned transaction before invoking
    /// [`Self::submit_or_recover_archive_location`].
    fn prepare_archive_location_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registered: &PublicationRegisteredArchiveV1,
        generation: u8,
        prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError>;

    /// Submit or recover the exact journaled location transaction and finalized state.
    fn submit_or_recover_archive_location(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registered: &PublicationRegisteredArchiveV1,
        intent: &PublicationArchiveLocationIntentV1,
        prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError>;

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
        // This explicit fallback never probes Torii or the implemented private service. A
        // configured production runtime supplies that authenticated service client instead.
        Err(PublicationBackendError::permanent(
            "SEED_INGRESS_SERVICE_NOT_CONFIGURED",
        ))
    }

    fn prepare_archive_location_intent(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        _generation: u8,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError> {
        Err(PublicationBackendError::permanent(
            "STORAGE_COORDINATOR_NOT_CONFIGURED",
        ))
    }

    fn submit_or_recover_archive_location(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        _intent: &PublicationArchiveLocationIntentV1,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError> {
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
        if read.account_chain_discriminant() != signing.account_chain_discriminant() {
            return Err(RegistryErrorV1::new(
                RegistryFailureClassV1::Permanent,
                "MUSUBI_PUBLICATION_REGISTRY_PROFILE_MISMATCH",
            ));
        }
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

    fn recover_registered_archive(
        &self,
        request: &PublicationRequestV1,
        intent: &PublicationArchiveRegistrationIntentV1,
        minimum_finalized_height: Option<u64>,
    ) -> Result<Option<PublicationRegisteredArchiveV1>, PublicationBackendError> {
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
        if minimum_finalized_height.is_some_and(|height| page.snapshot.finalized_height < height) {
            return Ok(None);
        }
        let recovered = PublicationRegisteredArchiveV1 {
            finalized_transaction_hash: intent.transaction_hash,
            chain_id: page.chain_id,
            genesis_block_hash: page.genesis_hash,
            snapshot: page.snapshot,
            archive: page.archive,
        };
        recovered
            .validate_for(request, intent)
            .map_err(|_| PublicationBackendError::permanent("ARCHIVE_REGISTRATION_CONFLICT"))?;
        Ok(Some(recovered))
    }

    fn finalized_archive_absence(
        &self,
        request: &PublicationRequestV1,
        minimum_finalized_height: Option<u64>,
    ) -> Result<Option<PublicationArchiveAbsenceEvidenceV1>, PublicationBackendError> {
        let page = self
            .read
            .archive_retention(&MusubiArchiveRetentionQueryV1 {
                archive_ids: vec![request.archive_commitment.archive_id()],
                expected_snapshot: None,
            })
            .map_err(registry_backend_error)?;
        if page.chain_id != request.chain_id
            || page.genesis_hash != request.genesis_block_hash
            || minimum_finalized_height
                .is_some_and(|height| page.snapshot.finalized_height < height)
        {
            return Ok(None);
        }
        let decision = page.items.into_iter().next().ok_or_else(|| {
            PublicationBackendError::permanent("ARCHIVE_ABSENCE_RESPONSE_INVALID")
        })?;
        if decision.disposition != MusubiArchiveRetentionDispositionV1::RetainUnknown {
            return Ok(None);
        }
        Ok(Some(PublicationArchiveAbsenceEvidenceV1 {
            chain_id: page.chain_id,
            genesis_block_hash: page.genesis_hash,
            snapshot: page.snapshot,
            finalized_time_ms: page.finalized_time_ms,
            decision,
        }))
    }

    fn terminal_registration_state(
        &self,
        request: &PublicationRequestV1,
        intent: &PublicationArchiveRegistrationIntentV1,
        kind: RegistryTerminalTransactionStateV1,
        block_height: Option<u64>,
    ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError> {
        if block_height == Some(0) {
            return Err(PublicationBackendError::permanent(
                "ARCHIVE_REGISTRATION_TERMINAL_STATUS_INVALID",
            ));
        }
        if let Some(registered) = self.recover_registered_archive(request, intent, block_height)? {
            return Ok(PublicationArchiveRegistrationAdvanceV1::Registered(
                registered,
            ));
        }
        let Some(absence) = self.finalized_archive_absence(request, block_height)? else {
            return Ok(PublicationArchiveRegistrationAdvanceV1::Pending);
        };
        match kind {
            RegistryTerminalTransactionStateV1::Rejected => Err(
                PublicationBackendError::permanent("ARCHIVE_REGISTRATION_TRANSACTION_REJECTED"),
            ),
            RegistryTerminalTransactionStateV1::Expired => {
                Ok(PublicationArchiveRegistrationAdvanceV1::TerminalAbsent(
                    PublicationArchiveRegistrationTerminalV1::registry_expired(
                        intent,
                        block_height,
                        absence,
                    ),
                ))
            }
        }
    }

    fn finalized_validity_window_registration_state(
        &self,
        request: &PublicationRequestV1,
        intent: &PublicationArchiveRegistrationIntentV1,
    ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError> {
        let Some(absence) = self.finalized_archive_absence(request, None)? else {
            return Ok(PublicationArchiveRegistrationAdvanceV1::Pending);
        };
        terminal_after_finalized_validity_window(request, intent, absence).map(|terminal| {
            terminal.map_or(
                PublicationArchiveRegistrationAdvanceV1::Pending,
                PublicationArchiveRegistrationAdvanceV1::TerminalAbsent,
            )
        })
    }
}

fn terminal_after_finalized_validity_window(
    request: &PublicationRequestV1,
    intent: &PublicationArchiveRegistrationIntentV1,
    absence: PublicationArchiveAbsenceEvidenceV1,
) -> Result<Option<PublicationArchiveRegistrationTerminalV1>, PublicationBackendError> {
    absence
        .validate_for(request)
        .map_err(|_| PublicationBackendError::permanent("ARCHIVE_ABSENCE_RESPONSE_INVALID"))?;
    let valid_until_ms = archive_registration_intent_valid_until_ms(intent)
        .ok_or_else(|| PublicationBackendError::permanent("ARCHIVE_REGISTRATION_INTENT_INVALID"))?;
    if absence.finalized_time_ms <= valid_until_ms {
        return Ok(None);
    }
    let terminal = PublicationArchiveRegistrationTerminalV1::finalized_validity_window_elapsed(
        intent, absence,
    );
    terminal
        .validate_for(request, intent)
        .map_err(|_| PublicationBackendError::permanent("ARCHIVE_ABSENCE_RESPONSE_INVALID"))?;
    Ok(Some(terminal))
}

fn validate_registration_submission_hash(
    expected: [u8; 32],
    submitted: [u8; 32],
) -> Result<(), PublicationBackendError> {
    if submitted == expected {
        Ok(())
    } else {
        Err(PublicationBackendError::permanent(
            "ARCHIVE_REGISTRATION_TRANSACTION_HASH_MISMATCH",
        ))
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

    fn prepare_archive_registration_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<PublicationArchiveRegistrationIntentV1, PublicationBackendError> {
        self.check_operation(operation_id)?;
        self.check_request(request)?;
        let instruction = request.archive_registration_instruction(receipt);
        let payload = self
            .signing
            .prebuild_v1(instruction)
            .map_err(registry_backend_error)?;
        let signed_transaction = self
            .signing
            .quote_and_sign_v1(payload)
            .map_err(registry_backend_error)?;
        Ok(PublicationArchiveRegistrationIntentV1::new(
            operation_id,
            request,
            receipt.clone(),
            signed_transaction,
        ))
    }

    fn submit_or_recover_archive_registration(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        intent: &PublicationArchiveRegistrationIntentV1,
    ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError> {
        self.check_operation(operation_id)?;
        self.check_request(request)?;
        intent
            .validate_for(operation_id, request, &intent.staging_receipt)
            .map_err(|_| {
                PublicationBackendError::permanent("ARCHIVE_REGISTRATION_INTENT_INVALID")
            })?;
        let initial_state = self
            .signing
            .transaction_application_state_v1(&intent.signed_transaction)
            .map_err(registry_backend_error)?;
        match initial_state {
            RegistryTransactionStateV1::Applied { block_height } => {
                return Ok(self
                    .recover_registered_archive(request, intent, Some(block_height))?
                    .map_or(
                        PublicationArchiveRegistrationAdvanceV1::Pending,
                        PublicationArchiveRegistrationAdvanceV1::Registered,
                    ));
            }
            RegistryTransactionStateV1::Pending => {
                return self.finalized_validity_window_registration_state(request, intent);
            }
            RegistryTransactionStateV1::Terminal { kind, block_height } => {
                return self.terminal_registration_state(request, intent, kind, block_height);
            }
            RegistryTransactionStateV1::Absent => {}
        }

        if let Some(registered) = self.recover_registered_archive(request, intent, None)? {
            return Ok(PublicationArchiveRegistrationAdvanceV1::Registered(
                registered,
            ));
        }
        if let terminal @ PublicationArchiveRegistrationAdvanceV1::TerminalAbsent(_) =
            self.finalized_validity_window_registration_state(request, intent)?
        {
            return Ok(terminal);
        }

        let submission = self.signing.submit_signed_v1(&intent.signed_transaction);
        if let Ok(transaction_hash) = submission {
            validate_registration_submission_hash(intent.transaction_hash, transaction_hash)?;
        }
        let observed_state = self
            .signing
            .transaction_application_state_v1(&intent.signed_transaction)
            .map_err(registry_backend_error)?;
        match observed_state {
            RegistryTransactionStateV1::Applied { block_height } => Ok(self
                .recover_registered_archive(request, intent, Some(block_height))?
                .map_or(
                    PublicationArchiveRegistrationAdvanceV1::Pending,
                    PublicationArchiveRegistrationAdvanceV1::Registered,
                )),
            RegistryTransactionStateV1::Pending => {
                self.finalized_validity_window_registration_state(request, intent)
            }
            RegistryTransactionStateV1::Terminal { kind, block_height } => {
                self.terminal_registration_state(request, intent, kind, block_height)
            }
            RegistryTransactionStateV1::Absent => {
                if let Some(registered) = self.recover_registered_archive(request, intent, None)? {
                    return Ok(PublicationArchiveRegistrationAdvanceV1::Registered(
                        registered,
                    ));
                }
                if let terminal @ PublicationArchiveRegistrationAdvanceV1::TerminalAbsent(_) =
                    self.finalized_validity_window_registration_state(request, intent)?
                {
                    return Ok(terminal);
                }
                match submission {
                    Ok(_) => Ok(PublicationArchiveRegistrationAdvanceV1::Pending),
                    Err(error) => Err(registry_backend_error(error)),
                }
            }
        }
    }

    fn prepare_archive_location_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registered: &PublicationRegisteredArchiveV1,
        generation: u8,
        prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError> {
        self.check_operation(operation_id)?;
        self.check_request(request)?;
        self.services.prepare_archive_location_intent(
            operation_id,
            request,
            registered,
            generation,
            prior_location_ids,
        )
    }

    fn submit_or_recover_archive_location(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registered: &PublicationRegisteredArchiveV1,
        intent: &PublicationArchiveLocationIntentV1,
        prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError> {
        self.check_operation(operation_id)?;
        self.check_request(request)?;
        self.services.submit_or_recover_archive_location(
            operation_id,
            request,
            registered,
            intent,
            prior_location_ids,
        )
    }

    fn finalized_replication(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
    ) -> Result<PublicationReplicationAdvanceV1, PublicationBackendError> {
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
            return Ok(PublicationReplicationAdvanceV1::Pending);
        };
        registration
            .validate_polled_page(request, &page)
            .map_err(|_| {
                PublicationBackendError::permanent("ARCHIVE_LOCATION_FINALIZED_PAGE_INVALID")
            })?;
        let registered = registration
            .intent
            .prepared_page
            .archive
            .registration_projection();
        if page.archive.registration_projection() != registered {
            return Err(PublicationBackendError::permanent(
                "ARCHIVE_LOCATION_FINALIZED_ARCHIVE_CONFLICT",
            ));
        }
        let location = page
            .items
            .iter()
            .find(|candidate| candidate.location_id == registration.location_id())
            .cloned();
        if let Some(location) = location {
            return if location.state == MusubiArchiveLocationStateV1::Healthy
                && location.providers.len() >= usize::from(MUSUBI_MIN_HEALTHY_REPLICAS_V1)
            {
                Ok(PublicationReplicationAdvanceV1::Healthy(location))
            } else {
                Ok(PublicationReplicationAdvanceV1::Pending)
            };
        }
        if page.snapshot.finalized_height <= registration.finalized_page.snapshot.finalized_height
            || page.archive.location_revision
                <= registration.finalized_page.archive.location_revision
        {
            return Ok(PublicationReplicationAdvanceV1::Pending);
        }
        Ok(PublicationReplicationAdvanceV1::Retired(
            PublicationArchiveLocationTerminalV1 {
                transaction_hash: registration.intent.transaction_hash,
                reason: PublicationArchiveLocationTerminalReasonV1::Retired,
                finalized_page: page,
            },
        ))
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
        let submission = self
            .signing
            .submit_observed_v1(instruction.clone())
            .map_err(registry_backend_error)?;
        match submission {
            RegistryMutationSubmissionV1::Applied {
                transaction_hash,
                block_height,
            } => Ok(PublicationAmxSubmissionV1::new(
                operation_id,
                instruction,
                transaction_hash,
                block_height,
            )),
            RegistryMutationSubmissionV1::Pending { .. } => Err(
                PublicationBackendError::retryable("RELEASE_SUBMISSION_TRANSACTION_PENDING"),
            ),
            RegistryMutationSubmissionV1::Terminal {
                kind: RegistryTerminalTransactionStateV1::Expired,
                ..
            } => Err(PublicationBackendError::retryable(
                "RELEASE_SUBMISSION_TRANSACTION_EXPIRED",
            )),
            RegistryMutationSubmissionV1::Terminal {
                kind: RegistryTerminalTransactionStateV1::Rejected,
                ..
            } => Err(PublicationBackendError::permanent(
                "RELEASE_SUBMISSION_TRANSACTION_REJECTED",
            )),
        }
    }

    fn finalized_release_and_index(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        submission: &PublicationAmxSubmissionV1,
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
        if page.snapshot.finalized_height < submission.applied_height {
            return Ok(None);
        }
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
            chain_id: page.chain_id,
            genesis_block_hash: page.genesis_hash,
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

fn package_id_from_namespace_binding(
    selector: &MusubiPackageSelectorV1,
    binding: &MusubiNamespaceBindingV1,
) -> Result<MusubiPackageIdV1, RegistryErrorV1> {
    selector.validate().map_err(|_| invalid_response())?;
    binding.validate().map_err(|_| invalid_response())?;
    if selector.namespace != binding.namespace {
        return Err(invalid_response());
    }
    let package = MusubiPackageIdV1::new(
        binding.home_dataspace,
        binding.scope.clone(),
        selector.name.clone(),
    );
    package.validate().map_err(|_| invalid_response())?;
    Ok(package)
}

fn validate_maintainer_page(
    request: &MusubiPackagePageQueryV1,
    page: &MusubiMaintainerPageV1,
) -> Result<(), RegistryErrorV1> {
    page.validate_for(request).map_err(|_| invalid_response())
}

fn invalid_public_config() -> RegistryErrorV1 {
    RegistryErrorV1::new(
        RegistryFailureClassV1::Permanent,
        "MUSUBI_REGISTRY_PUBLIC_CONFIG_INVALID",
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
    use std::{cell::Cell, io::Write as _, net::TcpListener, time::Duration};

    use iroha::crypto::{Algorithm, ExposedPrivateKey, KeyPair, SignatureOf};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        isi::InstructionBox,
        musubi::{
            ArchiveId, MUSUBI_REGISTRY_VERSION_V1, MusubiAbiBindingV1, MusubiArchiveCommitmentV1,
            MusubiArchiveRecordV1, MusubiArchiveRetentionDecisionV1,
            MusubiArchiveRetentionDispositionV1, MusubiArchiveRetentionPageV1,
            MusubiArchiveRetentionQueryV1, MusubiArtifactGovernanceStateV1, MusubiContentDigestV1,
            MusubiInvitationStateV1, MusubiInviteIdV1, MusubiKotodamaEditionV1,
            MusubiMaintainerDirectoryEntryV1, MusubiMaintainerInvitationV1,
            MusubiNamespaceBindingDigestV1, MusubiNamespaceV1, MusubiPackageMemberV1,
            MusubiPackageRecordV1, MusubiPackageRevisionsV1, MusubiPackageRoleV1,
            MusubiPackageScopeV1, MusubiPublicationV1, MusubiReasonV1, MusubiRegistrySnapshotV1,
            MusubiReleaseIdV1, MusubiReleaseManifestV1, MusubiReleaseMetadataV1,
            MusubiReleaseRecordV1, MusubiReleaseRevisionsV1, MusubiReleaseYankV1,
            MusubiResolutionProofV1, MusubiSearchPageRequestV1, MusubiSeedIngressReceiptApprovalV1,
            MusubiSeedIngressReceiptPayloadV1, MusubiSemanticReleaseDigestV1,
            MusubiVerificationLockV1, MusubiVersionV1,
        },
        nexus::DataSpaceId,
        sorafs::pin_registry::{ChunkerProfileHandle, ManifestRootCid},
        transaction::{Executable, FeePaymentIntent, SignedTransaction, TransactionBuilder},
    };
    use tempfile::tempdir;

    use super::*;

    fn serve_http_once(
        status: &'static str,
        response: Vec<u8>,
    ) -> (Url, thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback listener");
        let address = listener.local_addr().expect("loopback address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("one query connection");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("read timeout");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 2_048];
            let (header_end, content_length) = loop {
                let read = stream.read(&mut buffer).expect("read query request");
                assert_ne!(read, 0, "query request ended before its headers");
                request.extend_from_slice(&buffer[..read]);
                let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n")
                else {
                    continue;
                };
                let headers = std::str::from_utf8(&request[..header_end]).expect("HTTP headers");
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().expect("content length"))
                    })
                    .unwrap_or(0);
                break (header_end + 4, content_length);
            };
            while request.len() < header_end + content_length {
                let read = stream.read(&mut buffer).expect("read query body");
                assert_ne!(read, 0, "query request ended before its body");
                request.extend_from_slice(&buffer[..read]);
            }
            let request_body = request[header_end..header_end + content_length].to_vec();
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response.len()
            )
            .expect("write response headers");
            stream.write_all(&response).expect("write response body");
            request_body
        });
        (
            format!("http://{address}/").parse().expect("loopback URL"),
            server,
        )
    }

    fn serve_json_once(response: Vec<u8>) -> (Url, thread::JoinHandle<Vec<u8>>) {
        serve_http_once("200 OK", response)
    }

    fn retention_page(
        archive_ids: &[ArchiveId],
        snapshot: MusubiRegistrySnapshotV1,
    ) -> MusubiArchiveRetentionPageV1 {
        MusubiArchiveRetentionPageV1 {
            chain_id: ChainId::from("musubi-retention-client-test"),
            genesis_hash: [0x81; 32],
            items: archive_ids
                .iter()
                .map(|archive_id| MusubiArchiveRetentionDecisionV1 {
                    archive_id: *archive_id,
                    disposition: MusubiArchiveRetentionDispositionV1::RetainUnknown,
                    active_releases: 0,
                    yanked_releases: 0,
                    taken_down_releases: 0,
                    storage: None,
                })
                .collect(),
            snapshot,
            finalized_time_ms: 1_700_000_000_000,
        }
    }

    #[test]
    fn archive_retention_client_binds_snapshot_and_exact_item_order() {
        let archive_ids = vec![ArchiveId::new([0x11; 32]), ArchiveId::new([0x22; 32])];
        let snapshot = MusubiRegistrySnapshotV1 {
            finalized_height: 7,
            finalized_block_hash: [0x71; 32],
            index_revision: 9,
        };
        let request = MusubiArchiveRetentionQueryV1 {
            archive_ids: archive_ids.clone(),
            expected_snapshot: Some(snapshot),
        };

        let valid = retention_page(&archive_ids, snapshot);
        let (url, server) =
            serve_json_once(norito::json::to_vec(&valid).expect("retention page JSON"));
        let client = RegistryReadClientV1::new(url, Duration::from_secs(2), 753)
            .expect("signer-free registry client");
        assert_eq!(
            client
                .archive_retention(&request)
                .expect("exact retention response"),
            valid
        );
        server.join().expect("query server");

        let mut stale = retention_page(&archive_ids, snapshot);
        stale.snapshot.finalized_height += 1;
        stale.snapshot.finalized_block_hash = [0x72; 32];
        let (url, server) = serve_json_once(norito::json::to_vec(&stale).expect("stale page JSON"));
        let client = RegistryReadClientV1::new(url, Duration::from_secs(2), 753)
            .expect("signer-free registry client");
        assert_eq!(
            client
                .archive_retention(&request)
                .expect_err("snapshot mismatch must fail closed")
                .code(),
            "MUSUBI_REGISTRY_RESPONSE_INVALID"
        );
        server.join().expect("query server");

        let mismatched_ids = vec![archive_ids[0], ArchiveId::new([0x33; 32])];
        let mismatched = retention_page(&mismatched_ids, snapshot);
        let (url, server) = serve_json_once(
            norito::json::to_vec(&mismatched).expect("mismatched retention page JSON"),
        );
        let client = RegistryReadClientV1::new(url, Duration::from_secs(2), 753)
            .expect("signer-free registry client");
        assert_eq!(
            client
                .archive_retention(&request)
                .expect_err("item mismatch must fail closed")
                .code(),
            "MUSUBI_REGISTRY_RESPONSE_INVALID"
        );
        server.join().expect("query server");

        let reversed = retention_page(&[archive_ids[1], archive_ids[0]], snapshot);
        let (url, server) =
            serve_json_once(norito::json::to_vec(&reversed).expect("reversed page JSON"));
        let client = RegistryReadClientV1::new(url, Duration::from_secs(2), 753)
            .expect("signer-free registry client");
        assert_eq!(
            client
                .archive_retention(&request)
                .expect_err("noncanonical item order must fail closed")
                .code(),
            "MUSUBI_REGISTRY_RESPONSE_INVALID"
        );
        server.join().expect("query server");
    }

    #[test]
    fn archive_retention_rejects_an_invalid_request_before_network_io() {
        let client = RegistryReadClientV1::new(
            "http://127.0.0.1:9/".parse().expect("loopback URL"),
            Duration::from_secs(1),
            753,
        )
        .expect("signer-free reader");
        let error = client
            .archive_retention(&MusubiArchiveRetentionQueryV1 {
                archive_ids: Vec::new(),
                expected_snapshot: None,
            })
            .expect_err("invalid retention request must fail before transport");
        assert_eq!(error.code(), "MUSUBI_REGISTRY_RETENTION_REQUEST_INVALID");
    }

    #[test]
    fn public_config_ignores_signer_fields() {
        let temporary = tempdir().expect("temporary directory");
        let path = temporary.path().join("client.toml");
        fs::write(
            &path,
            r#"
                torii_url = "https://registry.example/iroha/"
                [account]
                profile = "taira"
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
        assert_eq!(client.account_chain_discriminant(), 369);

        let same_bytes = fs::read(&path).expect("read the selected config image");
        let from_bytes = RegistryReadClientV1::load_from_config_bytes(&same_bytes)
            .expect("load public context from the already-read image");
        assert_eq!(from_bytes.torii_url(), client.torii_url());
        assert_eq!(
            from_bytes.account_chain_discriminant(),
            client.account_chain_discriminant()
        );
    }

    #[test]
    fn search_rejects_an_invalid_request_before_network_io() {
        let client = RegistryReadClientV1::new(
            "http://127.0.0.1:9/".parse().expect("loopback URL"),
            Duration::from_secs(1),
            753,
        )
        .expect("signer-free reader");
        let error = client
            .search(&MusubiSearchQueryV1 {
                query: String::new(),
                page: MusubiSearchPageRequestV1 {
                    limit: 50,
                    cursor: None,
                },
            })
            .expect_err("invalid search must fail before transport");
        assert_eq!(error.code(), "MUSUBI_REGISTRY_SEARCH_REQUEST_INVALID");
    }

    #[test]
    fn signing_client_parses_account_arguments_under_its_taira_profile() {
        let temporary = tempdir().expect("temporary directory");
        let path = temporary.path().join("client.toml");
        let signer_keypair = KeyPair::try_from_seed(vec![89; 32], Algorithm::Ed25519)
            .expect("signer fixture key pair");
        let private_key = ExposedPrivateKey(signer_keypair.private_key().clone()).to_string();
        fs::write(
            &path,
            format!(
                r#"
                    chain = "musubi-registry-test"
                    torii_url = "https://registry.example/iroha/"
                    [account]
                    domain = "dex.universal"
                    profile = "taira"
                    public_key = "{}"
                    private_key = "{}"
                "#,
                signer_keypair.public_key(),
                private_key,
            ),
        )
        .expect("write signing config fixture");
        let signing = RegistrySigningClientV1::load(Some(&path)).expect("load Taira signer");
        assert_eq!(signing.account_chain_discriminant(), 369);
        assert!(!format!("{signing:?}").contains(&private_key));

        let expected = account(90);
        let taira_literal = {
            let _chain_discriminant = ChainDiscriminantGuard::enter(369);
            expected.canonical_i105().expect("Taira account literal")
        };
        assert_eq!(
            signing
                .parse_account_id(&taira_literal)
                .expect("Taira account argument"),
            expected
        );

        let default_literal = {
            let _chain_discriminant = ChainDiscriminantGuard::enter(753);
            expected.canonical_i105().expect("default-network literal")
        };
        let error = signing
            .parse_account_id(&default_literal)
            .expect_err("another network's account literal must be rejected");
        assert_eq!(error.code(), "MUSUBI_ACCOUNT_ID_INVALID");
    }

    fn signing_client_at(url: &Url, signer_keypair: &KeyPair) -> RegistrySigningClientV1 {
        let temporary = tempdir().expect("temporary signing configuration");
        let path = temporary.path().join("client.toml");
        let private_key = ExposedPrivateKey(signer_keypair.private_key().clone()).to_string();
        fs::write(
            &path,
            format!(
                r#"
                    chain = "musubi-registry-test"
                    torii_url = "{url}"
                    [account]
                    domain = "dex.universal"
                    profile = "taira"
                    public_key = "{}"
                    private_key = "{}"
                "#,
                signer_keypair.public_key(),
                private_key,
            ),
        )
        .expect("write signing client fixture");
        RegistrySigningClientV1::load(Some(&path)).expect("load signing client fixture")
    }

    fn signed_status_probe(signer: &KeyPair) -> SignedTransaction {
        let authority = AccountId::new(signer.public_key().clone());
        let mut builder = TransactionBuilder::new(
            ChainId::from("musubi-registry-test"),
            authority,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(Vec::<InstructionBox>::new());
        builder.set_creation_time(Duration::from_millis(900));
        builder.sign(signer.private_key())
    }

    fn transaction_status_body_for_hash(hash: String, kind: &str) -> Vec<u8> {
        norito::json::to_vec(&norito::json!({
            "hash": hash,
            "status": { "kind": kind, "block_height": 44 },
            "scope": "local",
            "resolved_from": "state",
        }))
        .expect("transaction status JSON")
    }

    fn transaction_status_body(transaction: &SignedTransaction, kind: &str) -> Vec<u8> {
        transaction_status_body_for_hash(transaction.hash().to_string(), kind)
    }

    #[test]
    fn signing_client_maps_applied_and_pending_transaction_statuses_over_http() {
        let signer = KeyPair::try_from_seed(vec![92; 32], Algorithm::Ed25519)
            .expect("status signer fixture");
        let transaction = signed_status_probe(&signer);

        for (kind, expected) in [
            (
                "Applied",
                RegistryTransactionStateV1::Applied { block_height: 44 },
            ),
            ("Queued", RegistryTransactionStateV1::Pending),
        ] {
            let (url, server) = serve_json_once(transaction_status_body(&transaction, kind));
            let signing = signing_client_at(&url, &signer);
            assert_eq!(
                signing
                    .transaction_application_state_v1(&transaction)
                    .expect("typed transaction status"),
                expected
            );
            server.join().expect("status server");
        }
    }

    #[test]
    fn signing_client_maps_rejected_and_absent_transaction_statuses_over_http() {
        let signer = KeyPair::try_from_seed(vec![93; 32], Algorithm::Ed25519)
            .expect("status signer fixture");
        let transaction = signed_status_probe(&signer);

        let (url, server) = serve_json_once(transaction_status_body(&transaction, "Rejected"));
        let signing = signing_client_at(&url, &signer);
        let state = signing
            .transaction_application_state_v1(&transaction)
            .expect("state-final rejected transaction");
        assert_eq!(
            state,
            RegistryTransactionStateV1::Terminal {
                kind: RegistryTerminalTransactionStateV1::Rejected,
                block_height: Some(44),
            }
        );
        server.join().expect("rejected status server");

        let (url, server) = serve_http_once("404 Not Found", Vec::new());
        let signing = signing_client_at(&url, &signer);
        assert_eq!(
            signing
                .transaction_application_state_v1(&transaction)
                .expect("absent status response"),
            RegistryTransactionStateV1::Absent
        );
        server.join().expect("absent status server");
    }

    #[test]
    fn signing_client_requires_state_finality_and_height_for_terminal_statuses() {
        let signer = KeyPair::try_from_seed(vec![97; 32], Algorithm::Ed25519)
            .expect("status signer fixture");
        let transaction = signed_status_probe(&signer);
        let cached_rejection = norito::json::to_vec(&norito::json!({
            "hash": transaction.hash().to_string(),
            "status": { "kind": "Rejected", "block_height": 44 },
            "scope": "local",
            "resolved_from": "cache",
        }))
        .expect("cached rejection JSON");
        let (url, server) = serve_json_once(cached_rejection);
        let signing = signing_client_at(&url, &signer);
        assert_eq!(
            signing
                .transaction_application_state_v1(&transaction)
                .expect("non-authoritative rejection remains pending"),
            RegistryTransactionStateV1::Pending
        );
        server.join().expect("cached rejection server");

        let heightless_applied = norito::json::to_vec(&norito::json!({
            "hash": transaction.hash().to_string(),
            "status": { "kind": "Applied" },
            "scope": "local",
            "resolved_from": "state",
        }))
        .expect("heightless applied JSON");
        let (url, server) = serve_json_once(heightless_applied);
        let signing = signing_client_at(&url, &signer);
        let error = signing
            .transaction_application_state_v1(&transaction)
            .expect_err("applied status without committed height is not finality evidence");
        assert_eq!(error.class(), RegistryFailureClassV1::Permanent);
        assert_eq!(error.code(), "MUSUBI_REGISTRY_TRANSACTION_STATUS_INVALID");
        server.join().expect("heightless applied server");

        let (url, server) = serve_json_once(transaction_status_body(&transaction, "Expired"));
        let signing = signing_client_at(&url, &signer);
        assert_eq!(
            signing
                .transaction_application_state_v1(&transaction)
                .expect("state-final expiry"),
            RegistryTransactionStateV1::Terminal {
                kind: RegistryTerminalTransactionStateV1::Expired,
                block_height: Some(44),
            }
        );
        server.join().expect("state-final expiry server");

        let zero_height_expiry = norito::json::to_vec(&norito::json!({
            "hash": transaction.hash().to_string(),
            "status": { "kind": "Expired", "block_height": 0 },
            "scope": "local",
            "resolved_from": "state",
        }))
        .expect("zero-height expiry JSON");
        let (url, server) = serve_json_once(zero_height_expiry);
        let signing = signing_client_at(&url, &signer);
        let error = signing
            .transaction_application_state_v1(&transaction)
            .expect_err("zero is not a valid finalized block height");
        assert_eq!(error.code(), "MUSUBI_REGISTRY_TRANSACTION_STATUS_INVALID");
        server.join().expect("zero-height expiry server");
    }

    #[test]
    fn signing_client_rejects_a_status_response_for_another_transaction() {
        let signer = KeyPair::try_from_seed(vec![96; 32], Algorithm::Ed25519)
            .expect("status signer fixture");
        let transaction = signed_status_probe(&signer);
        let (url, server) =
            serve_json_once(transaction_status_body_for_hash("ff".repeat(32), "Applied"));
        let signing = signing_client_at(&url, &signer);
        let error = signing
            .transaction_application_state_v1(&transaction)
            .expect_err("another transaction's applied status must fail closed");
        assert_eq!(error.class(), RegistryFailureClassV1::Permanent);
        assert_eq!(
            error.code(),
            "MUSUBI_REGISTRY_TRANSACTION_STATUS_HASH_MISMATCH"
        );
        server.join().expect("wrong-hash status server");
    }

    #[test]
    fn mutation_submission_hash_must_match_the_locally_signed_transaction() {
        validate_mutation_submission_hash([0x41; 32], [0x41; 32])
            .expect("matching signed transaction hash");
        let error = validate_mutation_submission_hash([0x41; 32], [0x42; 32])
            .expect_err("substituted submission hash");
        assert_eq!(error.class(), RegistryFailureClassV1::Permanent);
        assert_eq!(error.code(), "MUSUBI_REGISTRY_TRANSACTION_HASH_MISMATCH");
    }

    #[test]
    fn public_queries_decode_accounts_with_the_configured_discriminant() {
        let expected = account(91);
        let response = {
            let _chain_discriminant = ChainDiscriminantGuard::enter(369);
            norito::json::to_vec(&expected).expect("account response JSON")
        };
        let (url, server) = serve_json_once(response);
        let client = RegistryReadClientV1::new(url, Duration::from_secs(2), 369)
            .expect("Taira registry reader");

        let actual: AccountId = client
            .query_required(
                PublicMusubiQueryPathV1::Maintainers,
                &norito::json!({"probe": true}),
            )
            .expect("configured discriminant applies during response decoding");
        assert_eq!(actual, expected);
        server.join().expect("query server");
    }

    #[test]
    fn namespace_binding_derives_an_absent_package_identity() {
        let selector: MusubiPackageSelectorV1 =
            "dex.universal/new-package".parse().expect("selector");
        let binding = MusubiNamespaceBindingV1 {
            namespace: selector.namespace.clone(),
            home_dataspace: DataSpaceId::new(7),
            scope: MusubiPackageScopeV1::Domain("dex".parse().expect("domain")),
            generation: 4,
        };

        let package = package_id_from_namespace_binding(&selector, &binding)
            .expect("namespace binding is enough before a package row exists");
        assert_eq!(package.home_dataspace, DataSpaceId::new(7));
        assert_eq!(package.scope, binding.scope);
        assert_eq!(package.name, selector.name);

        let other: MusubiPackageSelectorV1 = "other.universal/new-package"
            .parse()
            .expect("other selector");
        let error = package_id_from_namespace_binding(&other, &binding)
            .expect_err("a response for another namespace must be rejected");
        assert_eq!(error.code(), "MUSUBI_REGISTRY_RESPONSE_INVALID");
    }

    #[test]
    fn exact_queries_reject_valid_records_for_another_identity() {
        let package = MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            "registry-fixture".parse().expect("package name"),
        );
        let owner = account(40);
        let package_record = MusubiPackageRecordV1 {
            package: package.clone(),
            claimed_namespace: MusubiNamespaceV1::new("registry").expect("namespace"),
            claimed_namespace_binding: MusubiNamespaceBindingDigestV1::new([0x41; 32]),
            owners: vec![owner.clone()],
            member_accounts: vec![owner],
            claimed_at_height: 10,
            revisions: MusubiPackageRevisionsV1 {
                governance: 1,
                metadata: 1,
                archive_locations: 1,
            },
        };
        package_record.validate().expect("valid package record");
        let requested_package = MusubiPackageIdV1::new(
            DataSpaceId::new(8),
            MusubiPackageScopeV1::DataspaceRoot,
            "other".parse().expect("other package name"),
        );
        let package_response = {
            let _chain_discriminant = ChainDiscriminantGuard::enter(369);
            norito::json::to_vec(&package_record).expect("package record JSON")
        };
        let (url, server) = serve_json_once(package_response);
        let client =
            RegistryReadClientV1::new(url, Duration::from_secs(2), 369).expect("registry reader");
        let error = client
            .exact_package(requested_package.clone())
            .expect_err("an exact package response cannot cross identities");
        assert_eq!(error.code(), "MUSUBI_REGISTRY_RESPONSE_INVALID");
        let request_body = server.join().expect("package query server");
        let query: MusubiExactPackageQueryV1 =
            norito::json::from_slice(&request_body).expect("exact package query JSON");
        assert_eq!(query.package, requested_package);

        let (request, _, _, _) = publication_fixture();
        let manifest = request.publication.manifest.clone();
        let published_release = manifest.release.clone();
        let release_record = MusubiReleaseRecordV1 {
            release_digest: manifest.release_digest(),
            manifest,
            published_by: request.publisher.clone(),
            published_at_height: 20,
            yank: MusubiReleaseYankV1 {
                release: published_release.clone(),
                yanked: false,
                reason: MusubiReasonV1::new("initial publication").expect("yank reason"),
                changed_by: request.publisher,
                changed_at_height: 20,
                revision: 1,
            },
            artifact_governance: MusubiArtifactGovernanceStateV1::Available,
            revisions: MusubiReleaseRevisionsV1 {
                yank: 1,
                artifact_governance: 1,
            },
        };
        release_record.validate().expect("valid release record");
        let requested_release = MusubiReleaseIdV1::new(
            published_release.package,
            "2.0.0".parse::<MusubiVersionV1>().expect("other version"),
        );
        let release_response = {
            let _chain_discriminant = ChainDiscriminantGuard::enter(369);
            norito::json::to_vec(&release_record).expect("release record JSON")
        };
        let (url, server) = serve_json_once(release_response);
        let client =
            RegistryReadClientV1::new(url, Duration::from_secs(2), 369).expect("registry reader");
        let error = client
            .exact_release(requested_release.clone())
            .expect_err("an exact release response cannot cross identities");
        assert_eq!(error.code(), "MUSUBI_REGISTRY_RESPONSE_INVALID");
        let request_body = server.join().expect("release query server");
        let query: MusubiExactReleaseQueryV1 =
            norito::json::from_slice(&request_body).expect("exact release query JSON");
        assert_eq!(query.release, requested_release);
    }

    #[test]
    fn maintainer_page_accepts_members_and_pending_invites_for_the_requested_package() {
        let package = MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            "demo".parse().expect("package name"),
        );
        let owner = account(41);
        let invited = account(42);
        let mut items = vec![
            MusubiMaintainerDirectoryEntryV1::Accepted(MusubiPackageMemberV1 {
                package: package.clone(),
                account: owner.clone(),
                role: MusubiPackageRoleV1::Owner,
                accepted_at_height: 2,
                governance_revision: 3,
            }),
            MusubiMaintainerDirectoryEntryV1::PendingInvitation(MusubiMaintainerInvitationV1 {
                invite_id: MusubiInviteIdV1::new([4; 32]),
                package: package.clone(),
                invited_by: owner,
                invited_account: invited,
                role: MusubiPackageRoleV1::Owner,
                expected_governance_revision: 3,
                expires_at_height: 20,
                state: MusubiInvitationStateV1::Pending,
            }),
        ];
        items.sort_by_key(MusubiMaintainerDirectoryEntryV1::key);
        let snapshot = MusubiRegistrySnapshotV1 {
            finalized_height: 5,
            finalized_block_hash: [6; 32],
            index_revision: 7,
        };
        let request = MusubiPackagePageQueryV1 {
            package: package.clone(),
            page: first_page(10),
        };
        let page = MusubiMaintainerPageV1 {
            query: request.clone(),
            items,
            next_cursor: None,
            snapshot,
        };
        validate_maintainer_page(&request, &page)
            .expect("accepted and pending entries share the requested package");

        let foreign_package = MusubiPackageIdV1::new(
            DataSpaceId::new(8),
            MusubiPackageScopeV1::DataspaceRoot,
            "foreign".parse().expect("foreign package name"),
        );
        let mismatched = MusubiMaintainerPageV1 {
            query: request.clone(),
            items: vec![MusubiMaintainerDirectoryEntryV1::Accepted(
                MusubiPackageMemberV1 {
                    package: foreign_package,
                    account: account(43),
                    role: MusubiPackageRoleV1::Owner,
                    accepted_at_height: 2,
                    governance_revision: 3,
                },
            )],
            next_cursor: None,
            snapshot,
        };
        let error = validate_maintainer_page(&request, &mismatched)
            .expect_err("a package-crossing directory response must be rejected");
        assert_eq!(error.code(), "MUSUBI_REGISTRY_RESPONSE_INVALID");
    }

    #[test]
    fn bind_selector_namespace_uses_a_namespace_prefix_without_requiring_a_row() {
        let selector: MusubiPackageSelectorV1 =
            "dex.universal/new-package".parse().expect("selector");
        let query = MusubiOrderedPrefixQueryV1 {
            prefix: MusubiOrderedPrefixV1::new("dex.universal/").expect("namespace prefix"),
            page: first_page(1),
        };
        let page = MusubiOrderedPackagePageV1 {
            query,
            chain_id: ChainId::from("musubi-registry-test"),
            genesis_hash: [1; 32],
            namespace_binding: MusubiNamespaceBindingV1 {
                namespace: selector.namespace.clone(),
                home_dataspace: DataSpaceId::new(7),
                scope: MusubiPackageScopeV1::Domain("dex".parse().expect("domain")),
                generation: 4,
            },
            items: Vec::new(),
            next_cursor: None,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 2,
                finalized_block_hash: [3; 32],
                index_revision: 5,
            },
        };
        let response = norito::json::to_vec(&page).expect("directory response JSON");
        let (url, server) = serve_json_once(response);
        let client = RegistryReadClientV1::new(url, Duration::from_secs(2), 753)
            .expect("signer-free registry client");

        let package = client
            .bind_selector_namespace(&selector)
            .expect("empty namespace directory still binds a package");
        assert_eq!(package.home_dataspace, DataSpaceId::new(7));
        assert_eq!(package.name, selector.name);

        let request_body = server.join().expect("query server");
        let query: MusubiOrderedPrefixQueryV1 =
            norito::json::from_slice(&request_body).expect("ordered-prefix request JSON");
        assert_eq!(query.prefix.as_str(), "dex.universal/");
        assert_eq!(query.page.limit, 1);
        assert!(query.page.cursor.is_none());
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

    fn publication_commitment() -> MusubiArchiveCommitmentV1 {
        MusubiArchiveCommitmentV1 {
            root_cid: ManifestRootCid::from_blake3_digest([0x31; 32]).expect("root CID"),
            chunker: ChunkerProfileHandle {
                profile_id: 1,
                namespace: "sorafs".to_owned(),
                name: "sf1".to_owned(),
                semver: "1.0.0".to_owned(),
                multihash_code: 0x1f,
            },
            chunk_plan_digest: MusubiContentDigestV1::new([0x32; 32]),
            por_root: MusubiContentDigestV1::new([0x33; 32]),
            content_length: 1_024,
            car_digest: MusubiContentDigestV1::new([0x34; 32]),
            car_size: 2_048,
            bundle_digest: MusubiContentDigestV1::new([0x35; 32]),
            source_tree_digest: MusubiContentDigestV1::new([0x36; 32]),
            descriptor_digest: MusubiContentDigestV1::new([0x37; 32]),
            file_count: 2,
            chunk_count: 4,
        }
    }

    fn publication_fixture() -> (
        PublicationRequestV1,
        KeyPair,
        KeyPair,
        PublicationArchiveRegistrationIntentV1,
    ) {
        let commitment = publication_commitment();
        let publisher_key =
            KeyPair::try_from_seed(vec![94; 32], Algorithm::Ed25519).expect("publisher key");
        let broker_key =
            KeyPair::try_from_seed(vec![95; 32], Algorithm::Ed25519).expect("broker key");
        let publisher = AccountId::new(publisher_key.public_key().clone());
        let broker = AccountId::new(broker_key.public_key().clone());
        let package = MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            "registry-fixture".parse().expect("package name"),
        );
        let release = MusubiReleaseIdV1::new(
            package,
            "1.0.0".parse::<MusubiVersionV1>().expect("version"),
        );
        let lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: release.clone(),
            root_dependencies: Vec::new(),
            nodes: Vec::new(),
        };
        let manifest = MusubiReleaseManifestV1 {
            release,
            edition: MusubiKotodamaEditionV1::V1,
            abi: MusubiAbiBindingV1::new([0x38; 32]).expect("ABI"),
            dependencies: Vec::new(),
            exports: Vec::new(),
            interface_digest: MusubiContentDigestV1::new([0x39; 32]),
            metadata: MusubiReleaseMetadataV1::default(),
            archive_id: commitment.archive_id(),
            verification_lock_digest: lock.digest(),
        };
        let snapshot = MusubiRegistrySnapshotV1 {
            finalized_height: 42,
            finalized_block_hash: [0x3a; 32],
            index_revision: 3,
        };
        let request = PublicationRequestV1 {
            chain_id: ChainId::from("musubi-registry-test"),
            genesis_block_hash: [0x3b; 32],
            publisher: publisher.clone(),
            ingress_broker: broker.clone(),
            seed_provider: ProviderId::new([0x3c; 32]),
            namespace: MusubiNamespaceV1::new("registry").expect("namespace"),
            publication: MusubiPublicationV1 {
                manifest,
                resolution: MusubiResolutionProofV1 { snapshot, lock },
            },
            archive_commitment: commitment,
            namespace_delegation: None,
            expected_policy_revision: 1,
            expected_governance_revision: None,
            nonce: [0x3d; 32],
        };
        request.validate().expect("publication request fixture");
        let payload = MusubiSeedIngressReceiptPayloadV1 {
            version: MUSUBI_REGISTRY_VERSION_V1,
            binding: MusubiSeedIngressReceiptBindingV1 {
                chain_id: request.chain_id.clone(),
                genesis_block_hash: request.genesis_block_hash,
                publisher,
                ingress_broker: broker,
                seed_provider: request.seed_provider,
                semantic_release_manifest_digest: request.publication.manifest.semantic_digest(),
                archive_id: request.archive_commitment.archive_id(),
                car_body_digest: request.archive_commitment.car_digest,
                car_body_length: request.archive_commitment.car_size,
                nonce: request.nonce,
            },
            issued_at_ms: 1_000,
            expires_at_ms: 2_000,
        };
        let receipt = MusubiSeedIngressReceiptV1 {
            approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
                public_key: broker_key.public_key().clone(),
                signature: SignatureOf::try_from_hash(
                    broker_key.private_key(),
                    payload.signing_hash(),
                )
                .expect("receipt signature"),
            }],
            payload,
        };
        let mut builder = TransactionBuilder::new(
            request.chain_id.clone(),
            request.publisher.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([request.archive_registration_instruction(&receipt)]);
        builder.set_creation_time(Duration::from_millis(receipt.payload.issued_at_ms));
        let transaction = builder.sign(publisher_key.private_key());
        let intent = PublicationArchiveRegistrationIntentV1::new(
            request.operation_id(),
            &request,
            receipt,
            transaction,
        );
        intent
            .validate_for(request.operation_id(), &request, &intent.staging_receipt)
            .expect("registration intent fixture");
        (request, publisher_key, broker_key, intent)
    }

    fn archive_page(
        request: &PublicationRequestV1,
        receipt: MusubiSeedIngressReceiptV1,
    ) -> MusubiArchiveLocationPageV1 {
        MusubiArchiveLocationPageV1 {
            chain_id: request.chain_id.clone(),
            genesis_hash: request.genesis_block_hash,
            archive: MusubiArchiveRecordV1 {
                archive_id: request.archive_commitment.archive_id(),
                commitment: request.archive_commitment.clone(),
                staging_receipt: receipt,
                registered_by: request.publisher.clone(),
                registered_at_height: 50,
                location_revision: 1,
                location_ids: Vec::new(),
            },
            items: Vec::new(),
            next_cursor: None,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 60,
                finalized_block_hash: [0x3e; 32],
                index_revision: 4,
            },
        }
    }

    fn publication_absence(
        request: &PublicationRequestV1,
        finalized_time_ms: u64,
    ) -> PublicationArchiveAbsenceEvidenceV1 {
        PublicationArchiveAbsenceEvidenceV1 {
            chain_id: request.chain_id.clone(),
            genesis_block_hash: request.genesis_block_hash,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 60,
                finalized_block_hash: [0x3e; 32],
                index_revision: 4,
            },
            finalized_time_ms,
            decision: MusubiArchiveRetentionDecisionV1 {
                archive_id: request.archive_commitment.archive_id(),
                disposition: MusubiArchiveRetentionDispositionV1::RetainUnknown,
                active_releases: 0,
                yanked_releases: 0,
                taken_down_releases: 0,
                storage: None,
            },
        }
    }

    #[test]
    fn finalized_chain_time_rotates_only_after_deadline_and_exact_absence() {
        let (request, _, _, intent) = publication_fixture();
        let valid_until_ms = archive_registration_intent_valid_until_ms(&intent)
            .expect("registration validity deadline");
        assert!(
            terminal_after_finalized_validity_window(
                &request,
                &intent,
                publication_absence(&request, valid_until_ms),
            )
            .expect("valid nonterminal evidence")
            .is_none()
        );

        let terminal = terminal_after_finalized_validity_window(
            &request,
            &intent,
            publication_absence(&request, valid_until_ms + 1),
        )
        .expect("valid terminal evidence")
        .expect("deadline has elapsed");
        assert_eq!(terminal.transaction_hash, intent.transaction_hash);

        let mut wrong_archive = publication_absence(&request, valid_until_ms + 1);
        wrong_archive.decision.archive_id = ArchiveId::new([0xff; 32]);
        assert_eq!(
            terminal_after_finalized_validity_window(&request, &intent, wrong_archive)
                .expect_err("another archive cannot prove terminal absence")
                .code(),
            "ARCHIVE_ABSENCE_RESPONSE_INVALID"
        );
    }

    #[test]
    fn production_prebuild_and_submission_hash_check_preserve_the_exact_transaction() {
        let (request, publisher_key, _, intent) = publication_fixture();
        let signing = signing_client_at(
            &"http://127.0.0.1:9/".parse().expect("loopback URL"),
            &publisher_key,
        );
        let instruction = request.archive_registration_instruction(&intent.staging_receipt);
        let exact_instruction: InstructionBox = instruction.clone().into();
        let payload = signing
            .prebuild_v1(instruction)
            .expect("offline exact transaction prebuild");
        assert_eq!(payload.chain(), &request.chain_id);
        assert_eq!(payload.authority(), &request.publisher);
        assert!(matches!(
            payload.instructions(),
            Executable::Instructions(instructions)
                if instructions.len() == 1
                    && instructions.iter().next() == Some(&exact_instruction)
        ));
        validate_registration_submission_hash(intent.transaction_hash, intent.transaction_hash)
            .expect("exact returned transaction hash");
        let error = validate_registration_submission_hash(intent.transaction_hash, [0xff; 32])
            .expect_err("substituted transaction hash");
        assert_eq!(
            error.code(),
            "ARCHIVE_REGISTRATION_TRANSACTION_HASH_MISMATCH"
        );

        let read = RegistryReadClientV1::new(
            "http://127.0.0.1:9/".parse().expect("loopback URL"),
            Duration::from_secs(1),
            753,
        )
        .expect("different-profile reader");
        let error = RegistryPublicationBackendV1::new(
            read,
            signing,
            UnavailablePublicationRuntimeV1,
            &request,
        )
        .expect_err("read and signing clients must share one address profile");
        assert_eq!(error.code(), "MUSUBI_PUBLICATION_REGISTRY_PROFILE_MISMATCH");
    }

    #[test]
    fn production_recovery_binds_the_finalized_authoritative_archive_page() {
        let (request, publisher_key, broker_key, intent) = publication_fixture();
        let exact_page = archive_page(&request, intent.staging_receipt.clone());
        let exact_page_json = {
            let _chain_discriminant = ChainDiscriminantGuard::enter(369);
            norito::json::to_vec(&exact_page).expect("archive page JSON")
        };
        let (url, server) = serve_json_once(exact_page_json);
        let read = RegistryReadClientV1::new(url.clone(), Duration::from_secs(2), 369)
            .expect("registry reader");
        let signing = signing_client_at(&url, &publisher_key);
        let backend = RegistryPublicationBackendV1::new(
            read,
            signing,
            UnavailablePublicationRuntimeV1,
            &request,
        )
        .expect("publication backend");
        let recovered = backend
            .recover_registered_archive(&request, &intent, None)
            .expect("recover exact archive")
            .expect("authoritative archive exists");
        assert_eq!(
            recovered.finalized_transaction_hash,
            intent.transaction_hash
        );
        assert_eq!(recovered.archive, exact_page.archive);
        let request_body = server.join().expect("archive query server");
        let query: MusubiArchiveLocationQueryV1 =
            norito::json::from_slice(&request_body).expect("archive query JSON");
        assert_eq!(query.archive_id, intent.archive_id);

        let mut replacement_payload = intent.staging_receipt.payload.clone();
        replacement_payload.issued_at_ms = 1_100;
        replacement_payload.expires_at_ms = 2_100;
        let replacement_receipt = MusubiSeedIngressReceiptV1 {
            approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
                public_key: broker_key.public_key().clone(),
                signature: SignatureOf::try_from_hash(
                    broker_key.private_key(),
                    replacement_payload.signing_hash(),
                )
                .expect("replacement receipt signature"),
            }],
            payload: replacement_payload,
        };
        let conflicting_page = archive_page(&request, replacement_receipt);
        conflicting_page
            .validate()
            .expect("structurally valid conflict");
        let conflicting_page_json = {
            let _chain_discriminant = ChainDiscriminantGuard::enter(369);
            norito::json::to_vec(&conflicting_page).expect("conflicting archive page JSON")
        };
        let (url, server) = serve_json_once(conflicting_page_json);
        let read = RegistryReadClientV1::new(url.clone(), Duration::from_secs(2), 369)
            .expect("registry reader");
        let signing = signing_client_at(&url, &publisher_key);
        let backend = RegistryPublicationBackendV1::new(
            read,
            signing,
            UnavailablePublicationRuntimeV1,
            &request,
        )
        .expect("publication backend");
        let error = backend
            .recover_registered_archive(&request, &intent, None)
            .expect_err("different receipt must not recover the authoritative archive");
        assert_eq!(error.code(), "ARCHIVE_REGISTRATION_CONFLICT");
        server.join().expect("conflicting archive query server");
    }

    #[test]
    fn terminal_transaction_status_still_recovers_an_exact_finalized_archive() {
        let (request, publisher_key, _, intent) = publication_fixture();
        let exact_page = archive_page(&request, intent.staging_receipt.clone());
        let response = {
            let _chain_discriminant = ChainDiscriminantGuard::enter(369);
            norito::json::to_vec(&exact_page).expect("archive page JSON")
        };
        let (url, server) = serve_json_once(response);
        let read = RegistryReadClientV1::new(url.clone(), Duration::from_secs(2), 369)
            .expect("registry reader");
        let signing = signing_client_at(&url, &publisher_key);
        let backend = RegistryPublicationBackendV1::new(
            read,
            signing,
            UnavailablePublicationRuntimeV1,
            &request,
        )
        .expect("publication backend");

        let outcome = backend
            .terminal_registration_state(
                &request,
                &intent,
                RegistryTerminalTransactionStateV1::Rejected,
                Some(55),
            )
            .expect("exact finalized archive wins over terminal status");
        let PublicationArchiveRegistrationAdvanceV1::Registered(registered) = outcome else {
            panic!("terminal status with an exact archive must recover registration");
        };
        assert_eq!(registered.archive, exact_page.archive);
        assert_eq!(registered.snapshot, exact_page.snapshot);
        server.join().expect("archive query server");
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
