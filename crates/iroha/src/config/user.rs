//! User configuration view.
use std::{fmt, fs::File, io::Read as _, path::PathBuf, time::Duration};
use error_stack::{Report, ResultExt};
use iroha_config::parameters::{actual::SorafsRolloutPhase, defaults};
use iroha_config_base::{
    ParameterOrigin, ReadConfig, WithOrigin,
    attach::ConfigValueAndOrigin,
    util::{DurationMs, Emitter, EmitterResultExt},
};
use iroha_torii_shared::{network_profile, network_profile_names};
use sorafs_manifest::alias_cache::AliasCachePolicy;
use sorafs_orchestrator::AnonymityPolicy;
use url::Url;
use crate::{
    config::BasicAuth,
    crypto::{KeyPair, PrivateKey, PublicKey},
    data_model::{
        name,
        prelude::{AccountId, ChainId, DomainId, NetworkId},
    },
};
/// Minimal allowed transaction time-to-live.
const MIN_TRANSACTION_TTL: Duration = Duration::from_secs(1);
const MAX_PRIVATE_KEY_FILE_BYTES: u64 = 4 * 1024;
const MAX_PUBLIC_IDENTITY_FILE_BYTES: u64 = 512;
/// Root of the user-facing configuration loaded from TOML + env.
#[derive(Clone, Debug, ReadConfig)]
pub struct Root {
    /// Unique chain identifier.
    #[config(env = "CHAIN")]
    pub chain: ChainId,
    /// Exact genesis-lineage identity used for signed protocol requests.
    pub network_id: Option<NetworkId>,
    /// Public file containing the canonical exact genesis-lineage identity.
    pub network_id_file: Option<WithOrigin<PathBuf>>,
    /// Torii API URL.
    #[config(env = "TORII_URL")]
    pub torii_url: WithOrigin<Url>,
    /// Optional HTTP Basic Auth credentials.
    pub basic_auth: Option<BasicAuth>,
    /// Timeout for Torii HTTP requests.
    #[config(default = "super::DEFAULT_TORII_REQUEST_TIMEOUT.into()")]
    pub torii_request_timeout_ms: WithOrigin<DurationMs>,
    #[config(nested)]
    /// Account configuration.
    pub account: Account,
    #[config(nested)]
    /// Transaction defaults.
    pub transaction: Transaction,
    #[config(nested)]
    /// Connect queue diagnostics and replay helpers.
    pub connect: Connect,
    #[config(nested)]
    /// SoraFS-specific configuration.
    pub sorafs: Sorafs,
    #[config(nested)]
    /// Soracloud-specific client behavior.
    pub soracloud: Soracloud,
    #[config(nested)]
    /// Optional Musubi production-publication platform bindings.
    pub musubi: Musubi,
}
#[derive(thiserror::Error, Debug)]
/// Errors found while validating or parsing user configuration.
pub enum ParseError {
    /// Transaction status timeout should be smaller than its time-to-live.
    #[error("Transaction status timeout should be smaller than its time-to-live")]
    TxTimeoutVsTtl,
    /// Transaction time-to-live is below the minimal allowed value.
    #[error("Transaction time-to-live should be at least {MIN_TRANSACTION_TTL:?}")]
    TxTtlTooSmall,
    /// Failed to construct a key pair from provided public and private keys.
    #[error("Failed to construct a key pair from provided public and private keys")]
    KeyPair,
    /// Unsupported URL scheme in `torii_url`.
    #[error("Unsupported URL scheme: `{scheme}`")]
    UnsupportedUrlScheme {
        /// Scheme part of the provided URL.
        scheme: String,
    },
    /// Invalid `SoraFS` anonymity rollout policy label.
    #[error("Invalid `SoraFS` anonymity policy label: `{value}`")]
    InvalidSorafsAnonymityPolicy {
        /// The supplied label.
        value: String,
    },
    /// Invalid rollout phase label (`sorafs.rollout_phase`).
    #[error("Invalid `SoraFS` rollout phase label: `{value}`")]
    InvalidSorafsRolloutPhase {
        /// The supplied label.
        value: String,
    },
    /// Connect queue root path was empty after parsing.
    #[error("`connect.queue_root` must not be empty")]
    EmptyConnectQueueRoot,
    /// Invalid account domain literal.
    #[error("Account domain must use `dataspace` or `domain.dataspace` format: `{value}`")]
    InvalidAccountDomain {
        /// Raw configured value.
        value: String,
    },
    /// Unknown public account network profile.
    #[error("Invalid account network profile: `{profile}`")]
    InvalidAccountProfile {
        /// Supplied profile label.
        profile: String,
    },
    /// Public account network profile and explicit chain discriminant disagree.
    #[error(
        "Account profile `{profile}` expects chain discriminant `{expected}`, but config supplied `{actual}`"
    )]
    AccountProfileDiscriminantMismatch {
        /// Supplied profile label.
        profile: String,
        /// Expected I105 chain discriminant for the profile.
        expected: u16,
        /// Actual explicit I105 chain discriminant.
        actual: u16,
    },
    /// Account config omitted both profile and explicit discriminant.
    #[error("Account config must set `profile` or an explicit `chain_discriminant`")]
    MissingAccountNetworkContext,
    /// Exact network identity was absent, ambiguous, or invalid.
    #[error("Invalid exact network identity configuration")]
    InvalidNetworkIdentity,
}
type ReportResult<T, E> = core::result::Result<T, Report<[E]>>;
fn valid_account_domain_scope_literal(value: &str) -> bool {
    if value.trim().is_empty() || value.trim() != value {
        return false;
    }
    if value.contains('.') {
        DomainId::parse_fully_qualified(value).is_ok()
    } else {
        name::canonicalize_domain_label(value).is_ok()
    }
}
fn resolve_account_private_key(
    inline: Option<WithOrigin<PrivateKey>>,
    file: Option<WithOrigin<PathBuf>>,
    emitter: &mut Emitter<ParseError>,
) -> Option<(PrivateKey, ParameterOrigin)> {
    let file = match (inline, file) {
        (Some(_), Some(_)) => {
            emitter.emit(
                Report::new(ParseError::KeyPair).attach(
                    "account.private_key and account.private_key_file are mutually exclusive; configure exactly one private-key source",
                ),
            );
            return None;
        }
        (Some(inline), None) => return Some(inline.into_tuple()),
        (None, Some(file)) => file,
        (None, None) => {
            emitter.emit(
                Report::new(ParseError::KeyPair).attach(
                    "missing account private-key source; configure exactly one of account.private_key or account.private_key_file",
                ),
            );
            return None;
        }
    };
    let path = file.resolve_relative_path();
    let (_, origin) = file.into_tuple();
    if path.as_os_str().is_empty() {
        emitter.emit(Report::new(ParseError::KeyPair).attach("account.private_key_file is empty"));
        return None;
    }
    let opened = match File::open(&path) {
        Ok(opened) => opened,
        Err(err) => {
            emitter.emit(Report::new(ParseError::KeyPair).attach(format!(
                "failed to open account.private_key_file `{}`: {err}",
                path.display()
            )));
            return None;
        }
    };
    let metadata = match opened.metadata() {
        Ok(metadata) => metadata,
        Err(err) => {
            emitter.emit(Report::new(ParseError::KeyPair).attach(format!(
                "failed to inspect account.private_key_file `{}`: {err}",
                path.display()
            )));
            return None;
        }
    };
    if !metadata.is_file() {
        emitter.emit(Report::new(ParseError::KeyPair).attach(format!(
            "account.private_key_file `{}` must reference a regular file",
            path.display()
        )));
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = metadata.permissions().mode();
        if mode & 0o077 != 0 {
            emitter.emit(Report::new(ParseError::KeyPair).attach(format!(
                "account.private_key_file `{}` must not be readable or writable by group/other users (mode {:04o})",
                path.display(),
                mode & 0o777
            )));
            return None;
        }
    }
    let mut encoded = String::new();
    if let Err(err) = opened
        .take(MAX_PRIVATE_KEY_FILE_BYTES + 1)
        .read_to_string(&mut encoded)
    {
        emitter.emit(Report::new(ParseError::KeyPair).attach(format!(
            "failed to read account.private_key_file `{}` as UTF-8: {err}",
            path.display()
        )));
        return None;
    }
    if encoded.len() as u64 > MAX_PRIVATE_KEY_FILE_BYTES {
        emitter.emit(Report::new(ParseError::KeyPair).attach(format!(
            "account.private_key_file `{}` exceeds the {MAX_PRIVATE_KEY_FILE_BYTES}-byte limit",
            path.display()
        )));
        return None;
    }
    let encoded = encoded
        .strip_suffix("\r\n")
        .or_else(|| encoded.strip_suffix('\n'))
        .unwrap_or(&encoded);
    if encoded.is_empty() || encoded.bytes().any(|byte| matches!(byte, b'\r' | b'\n')) {
        emitter.emit(Report::new(ParseError::KeyPair).attach(format!(
            "account.private_key_file `{}` must contain exactly one canonical private key",
            path.display()
        )));
        return None;
    }
    match encoded.parse::<PrivateKey>() {
        Ok(private_key) => Some((private_key, origin)),
        Err(err) => {
            emitter.emit(Report::new(ParseError::KeyPair).attach(format!(
                "account.private_key_file `{}` does not contain a canonical private key: {err}",
                path.display()
            )));
            None
        }
    }
}
fn resolve_network_id_source(
    inline: Option<NetworkId>,
    file: Option<WithOrigin<PathBuf>>,
    emitter: &mut Emitter<ParseError>,
) -> Option<NetworkId> {
    let file = match (inline, file) {
        (Some(_), Some(_)) => {
            emitter.emit(Report::new(ParseError::InvalidNetworkIdentity).attach(
                "network_id and network_id_file are mutually exclusive; configure exactly one public identity source",
            ));
            return None;
        }
        (Some(network_id), None) => return Some(network_id),
        (None, Some(file)) => file,
        (None, None) => {
            emitter.emit(Report::new(ParseError::InvalidNetworkIdentity).attach(
                "missing exact network identity; configure exactly one of network_id or network_id_file",
            ));
            return None;
        }
    };
    let path = file.resolve_relative_path();
    if path.as_os_str().is_empty() {
        emitter.emit(
            Report::new(ParseError::InvalidNetworkIdentity).attach("network_id_file is empty"),
        );
        return None;
    }
    let opened = match File::open(&path) {
        Ok(opened) => opened,
        Err(err) => {
            emitter.emit(
                Report::new(ParseError::InvalidNetworkIdentity).attach(format!(
                    "failed to open network_id_file `{}`: {err}",
                    path.display()
                )),
            );
            return None;
        }
    };
    let metadata = match opened.metadata() {
        Ok(metadata) => metadata,
        Err(err) => {
            emitter.emit(
                Report::new(ParseError::InvalidNetworkIdentity).attach(format!(
                    "failed to inspect network_id_file `{}`: {err}",
                    path.display()
                )),
            );
            return None;
        }
    };
    if !metadata.is_file() {
        emitter.emit(
            Report::new(ParseError::InvalidNetworkIdentity).attach(format!(
                "network_id_file `{}` must reference a regular file",
                path.display()
            )),
        );
        return None;
    }
    let mut encoded = String::new();
    if let Err(err) = opened
        .take(MAX_PUBLIC_IDENTITY_FILE_BYTES + 1)
        .read_to_string(&mut encoded)
    {
        emitter.emit(
            Report::new(ParseError::InvalidNetworkIdentity).attach(format!(
                "failed to read network_id_file `{}` as UTF-8: {err}",
                path.display()
            )),
        );
        return None;
    }
    if encoded.len() as u64 > MAX_PUBLIC_IDENTITY_FILE_BYTES {
        emitter.emit(
            Report::new(ParseError::InvalidNetworkIdentity).attach(format!(
                "network_id_file `{}` exceeds the {MAX_PUBLIC_IDENTITY_FILE_BYTES}-byte limit",
                path.display()
            )),
        );
        return None;
    }
    let encoded = encoded
        .strip_suffix("\r\n")
        .or_else(|| encoded.strip_suffix('\n'))
        .unwrap_or(&encoded);
    if encoded.is_empty() || encoded.bytes().any(|byte| matches!(byte, b'\r' | b'\n')) {
        emitter.emit(
            Report::new(ParseError::InvalidNetworkIdentity).attach(format!(
                "network_id_file `{}` must contain exactly one canonical network identity",
                path.display()
            )),
        );
        return None;
    }
    match encoded.parse::<NetworkId>() {
        Ok(network_id) if network_id.to_string() == encoded => Some(network_id),
        Ok(_) => {
            emitter.emit(
                Report::new(ParseError::InvalidNetworkIdentity).attach(format!(
                    "network_id_file `{}` does not contain the canonical network identity spelling",
                    path.display()
                )),
            );
            None
        }
        Err(err) => {
            emitter.emit(
                Report::new(ParseError::InvalidNetworkIdentity).attach(format!(
                    "network_id_file `{}` does not contain a valid network identity: {err}",
                    path.display()
                )),
            );
            None
        }
    }
}
impl Root {
    /// Validates user configuration for semantic errors and constructs a complete
    /// [`super::Config`].
    ///
    /// # Errors
    /// If a set of validity errors occurs.
    pub fn parse(self) -> ReportResult<super::Config, ParseError> {
        self.parse_with_musubi_sections()
            .map(|(configuration, _publication, _fetch)| configuration)
    }
    pub(crate) fn parse_with_musubi(
        self,
    ) -> ReportResult<(super::Config, MusubiPublication), ParseError> {
        self.parse_with_musubi_sections()
            .map(|(configuration, publication, _fetch)| (configuration, publication))
    }
    #[allow(clippy::too_many_lines)]
    fn parse_with_musubi_sections(
        self,
    ) -> ReportResult<(super::Config, MusubiPublication, MusubiFetch), ParseError> {
        let Self {
            chain: chain_id,
            network_id,
            network_id_file,
            torii_url,
            basic_auth,
            torii_request_timeout_ms,
            account:
                Account {
                    domain: domain_literal,
                    profile,
                    public_key,
                    private_key,
                    private_key_file,
                    chain_discriminant,
                },
            transaction:
                Transaction {
                    time_to_live_ms: tx_ttl,
                    status_timeout_ms: tx_timeout,
                    nonce: tx_add_nonce,
                },
            connect: Connect { queue_root },
            sorafs,
            soracloud,
            musubi:
                Musubi {
                    publication: musubi_publication,
                    fetch: musubi_fetch,
                },
        } = self;
        let mut emitter = Emitter::new();
        let network_id = resolve_network_id_source(network_id, network_id_file, &mut emitter);
        if tx_ttl.value().get() < MIN_TRANSACTION_TTL {
            emitter.emit(
                Report::new(ParseError::TxTtlTooSmall)
                    .attach(tx_ttl.clone().into_attachment())
                    .attach(format!(
                        "Note: minimal allowed TTL is {MIN_TRANSACTION_TTL:?}"
                    )),
            )
        }
        if tx_timeout.value() > tx_ttl.value() {
            emitter.emit(
                Report::new(ParseError::TxTimeoutVsTtl)
                    .attach(tx_timeout.clone().into_attachment())
                    .attach(tx_ttl.clone().into_attachment())
                    .attach(format!(
                        "Note: transaction status timeout must not exceed TTL (timeout={:?}, ttl={:?})",
                        tx_timeout.value(),
                        tx_ttl.value()
                    )),
            )
        }
        match torii_url.value().scheme() {
            "http" | "https" => {}
            scheme => emitter.emit(
                Report::new(ParseError::UnsupportedUrlScheme {
                    scheme: scheme.to_string(),
                })
                .attach(torii_url.clone().into_attachment())
                .attach("Note: only `http` and `https` protocols are supported"),
            ),
        }
        let torii_api_url = {
            let mut url = torii_url.into_value();
            let path = url.path();
            // Ensure torii url ends with a trailing slash
            if !path.ends_with('/') {
                let path = path.to_owned() + "/";
                url.set_path(&path)
            }
            url
        };
        let chain_discriminant = match profile.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
            Some(profile_name) => {
                if let Some(profile) = network_profile(profile_name) {
                    match chain_discriminant.origin() {
                        ParameterOrigin::Default { .. } => WithOrigin::new(
                            profile.chain_discriminant,
                            ParameterOrigin::custom(format!(
                                "derived from account profile `{}`",
                                profile.name
                            )),
                        ),
                        _ if *chain_discriminant.value() == profile.chain_discriminant => {
                            chain_discriminant
                        }
                        _ => {
                            emitter.emit(
                                Report::new(ParseError::AccountProfileDiscriminantMismatch {
                                    profile: profile.name.to_owned(),
                                    expected: profile.chain_discriminant,
                                    actual: *chain_discriminant.value(),
                                })
                                .attach(format!(
                                    "expected profile `{}` chain_discriminant={}, actual chain_discriminant={}",
                                    profile.name,
                                    profile.chain_discriminant,
                                    chain_discriminant.value()
                                )),
                            );
                            chain_discriminant
                        }
                    }
                } else {
                    emitter.emit(
                        Report::new(ParseError::InvalidAccountProfile {
                            profile: profile_name.to_owned(),
                        })
                        .attach(format!(
                            "supported account profiles: {}",
                            network_profile_names()
                        )),
                    );
                    chain_discriminant
                }
            }
            None => chain_discriminant,
        };
        let (public_key, public_key_origin) = public_key.into_tuple();
        let private_key = resolve_account_private_key(private_key, private_key_file, &mut emitter);
        if !valid_account_domain_scope_literal(&domain_literal) {
            emitter.emit(Report::new(ParseError::InvalidAccountDomain {
                value: domain_literal.clone(),
            }));
        }
        let key_pair = private_key.and_then(|(private_key, private_key_origin)| {
            KeyPair::new(public_key.clone(), private_key)
                .attach(ConfigValueAndOrigin::new("[REDACTED]", public_key_origin))
                .attach(ConfigValueAndOrigin::new("[REDACTED]", private_key_origin))
                .change_context(ParseError::KeyPair)
                .ok_or_emit(&mut emitter)
        });
        let account_id = AccountId::of(public_key);
        let (queue_root_path, queue_root_origin) = queue_root.into_tuple();
        if queue_root_path.as_os_str().is_empty() {
            emitter.emit(
                Report::new(ParseError::EmptyConnectQueueRoot)
                    .attach(ConfigValueAndOrigin::new(
                        "[EMPTY]",
                        queue_root_origin.clone(),
                    ))
                    .attach("connect.queue_root must point to the Connect queue root directory"),
            );
        }
        let Sorafs {
            alias_cache,
            rollout_phase,
            anonymity_policy,
        } = sorafs;
        let Soracloud { http_witness_file } = soracloud;
        let alias_policy = alias_cache.into_policy();
        let rollout_phase_value =
            SorafsRolloutPhase::parse(rollout_phase.as_str()).unwrap_or_else(|| {
                emitter.emit(
                    Report::new(ParseError::InvalidSorafsRolloutPhase {
                        value: rollout_phase.clone(),
                    })
                    .attach("invalid `sorafs.rollout_phase`; expected exactly canary|ramp|default"),
                );
                SorafsRolloutPhase::default()
            });
        let phase_default_policy = match rollout_phase_value {
            SorafsRolloutPhase::Canary => AnonymityPolicy::GuardPq,
            SorafsRolloutPhase::Ramp => AnonymityPolicy::MajorityPq,
            SorafsRolloutPhase::Default => AnonymityPolicy::StrictPq,
        };
        let default_anonymity_policy = anonymity_policy
            .map_or(phase_default_policy, |label| {
                AnonymityPolicy::parse(&label).unwrap_or_else(|| {
                    emitter.emit(
                        Report::new(ParseError::InvalidSorafsAnonymityPolicy {
                            value: label.clone(),
                        })
                        .attach(format!(
                            "invalid `sorafs.anonymity_policy` value `{label}`; expected exactly anon-guard-pq|anon-majority-pq|anon-strict-pq"
                        )),
                    );
                    phase_default_policy
                })
            });
        emitter.into_result()?;
        let network_id =
            network_id.expect("network identity should be valid when emitter succeeds");
        Ok((
            super::Config {
                chain: chain_id,
                network_id,
                account: account_id,
                account_chain_discriminant: chain_discriminant.into_value(),
                key_pair: key_pair.unwrap(),
                torii_api_url,
                basic_auth,
                torii_request_timeout: torii_request_timeout_ms.into_value().get(),
                transaction_ttl: tx_ttl.into_value().get(),
                transaction_status_timeout: tx_timeout.into_value().get(),
                transaction_add_nonce: tx_add_nonce,
                connect_queue_root: queue_root_path,
                soracloud_http_witness_file: http_witness_file,
                sorafs_alias_cache: alias_policy,
                sorafs_anonymity_policy: default_anonymity_policy,
                sorafs_rollout_phase: rollout_phase_value,
            },
            musubi_publication,
            musubi_fetch,
        ))
    }
}
/// Account parameters for building the default signer identity.
#[derive(Debug, Clone, ReadConfig)]
pub struct Account {
    /// Dataspace or domain.dataspace scope used for account alias operations.
    #[config(env = "ACCOUNT_DOMAIN")]
    pub domain: String,
    /// Public network profile used to derive the I105 chain discriminant.
    #[config(env = "ACCOUNT_PROFILE")]
    pub profile: Option<String>,
    /// Public key of the account.
    #[config(env = "ACCOUNT_PUBLIC_KEY")]
    pub public_key: WithOrigin<PublicKey>,
    /// Private key of the account.
    #[config(env = "ACCOUNT_PRIVATE_KEY")]
    pub private_key: Option<WithOrigin<PrivateKey>>,
    /// Owner-held file containing the account's canonical private key.
    #[config(env = "ACCOUNT_PRIVATE_KEY_FILE")]
    pub private_key_file: Option<WithOrigin<PathBuf>>,
    /// I105 chain discriminant used when parsing and rendering account literals.
    #[config(
        env = "ACCOUNT_CHAIN_DISCRIMINANT",
        default = "defaults::common::chain_discriminant()"
    )]
    pub chain_discriminant: WithOrigin<u16>,
}
/// Transaction defaults used by the client.
#[derive(Debug, Clone, ReadConfig)]
pub struct Transaction {
    /// Transaction time-to-live.
    #[config(default = "super::DEFAULT_TRANSACTION_TIME_TO_LIVE.into()")]
    pub time_to_live_ms: WithOrigin<DurationMs>,
    /// Timeout for waiting on transaction status.
    #[config(default = "super::DEFAULT_TRANSACTION_STATUS_TIMEOUT.into()")]
    pub status_timeout_ms: WithOrigin<DurationMs>,
    /// Whether to add a random nonce to transactions.
    #[config(default = "super::DEFAULT_TRANSACTION_NONCE")]
    pub nonce: bool,
}
/// Connect queue persistence and diagnostics.
#[derive(Debug, Clone, ReadConfig)]
pub struct Connect {
    /// Root directory containing Connect queue state.
    #[config(default = "super::default_connect_queue_root()")]
    pub queue_root: WithOrigin<PathBuf>,
}
impl Default for Connect {
    fn default() -> Self {
        Self {
            queue_root: WithOrigin::inline(super::default_connect_queue_root()),
        }
    }
}
/// Soracloud-specific client settings.
#[derive(Debug, Clone, Default, ReadConfig)]
pub struct Soracloud {
    /// Optional path to a JSON canonical request witness used for multisig Soracloud HTTP.
    pub http_witness_file: Option<PathBuf>,
}
/// Musubi-specific platform client configuration.
#[derive(Clone, Debug, Default, ReadConfig)]
pub struct Musubi {
    #[config(nested)]
    /// Optional production-publication service and public request bindings.
    pub publication: MusubiPublication,
    #[config(nested)]
    /// Optional authenticated `SoraFS` archive-fetch bindings.
    pub fetch: MusubiFetch,
}
/// Raw authenticated archive-fetch values admitted from `[musubi.fetch]`.
///
/// These fields have no environment mappings. Operator credentials are named only by
/// platform-owned files and are never embedded in this configuration value.
#[derive(Clone, Default, ReadConfig)]
pub struct MusubiFetch {
    /// Exact genesis-lineage identity used to sign stream-token issuance requests.
    pub network_id: Option<NetworkId>,
    /// Stable, non-secret client label sent to `SoraFS` token issuers.
    pub client_id: Option<String>,
    /// Optional bounded request timeout in milliseconds.
    pub request_timeout_ms: Option<u64>,
    /// Exact provider-specific HTTPS origins and credential-file bindings.
    #[config(default)]
    pub provider_gateways: Vec<MusubiFetchProviderGateway>,
}
impl fmt::Debug for MusubiFetch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MusubiFetch")
            .field("configured", &!self.provider_gateways.is_empty())
            .field("network_id_configured", &self.network_id.is_some())
            .field("provider_gateway_count", &self.provider_gateways.len())
            .field("request_timeout_ms", &self.request_timeout_ms)
            .finish_non_exhaustive()
    }
}
/// One trusted provider identity, HTTPS origin, and runtime-only operator key file.
#[derive(Clone, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct MusubiFetchProviderGateway {
    /// Lowercase hexadecimal public `SoraFS` provider identifier.
    pub provider_id: String,
    /// Provider-specific `SoraFS` HTTPS origin.
    pub url: String,
    /// Canonical operator public key authorized by this provider.
    pub operator_public_key: String,
    /// Platform-owned file containing the provider-authorized operator private key.
    pub operator_private_key_file: String,
}
impl fmt::Debug for MusubiFetchProviderGateway {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MusubiFetchProviderGateway")
            .field("provider_id", &self.provider_id)
            .field(
                "operator_public_key_configured",
                &!self.operator_public_key.is_empty(),
            )
            .field(
                "operator_private_key_configured",
                &!self.operator_private_key_file.is_empty(),
            )
            .finish_non_exhaustive()
    }
}
/// Raw production-publication values admitted from `[musubi.publication]`.
///
/// These fields deliberately have no environment mappings. The Musubi runtime performs the
/// security-sensitive semantic validation and keeps service URLs out of diagnostics.
#[derive(Clone, Default, ReadConfig)]
pub struct MusubiPublication {
    /// Private authenticated seed-ingress HTTPS base URL.
    pub seed_ingress_url: Option<String>,
    /// Private permanent-pin and replication-coordinator HTTPS base URL.
    pub storage_coordinator_url: Option<String>,
    /// Public I105 account of the admitted seed-ingress broker.
    pub ingress_broker: Option<String>,
    /// Lowercase hexadecimal public `SoraFS` seed-provider identifier.
    pub seed_provider: Option<String>,
    /// Exact registry admission-policy revision expected by the publication.
    pub expected_policy_revision: Option<u64>,
    /// Optional bounded request timeout in milliseconds.
    pub request_timeout_ms: Option<u64>,
    /// Exact provider-specific readback origins.
    #[config(default)]
    pub provider_gateways: Vec<MusubiPublicationProviderGateway>,
    /// Optional path to a canonical public namespace-delegation proof.
    pub namespace_delegation_file: Option<String>,
}
impl fmt::Debug for MusubiPublication {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MusubiPublication")
            .field("configured", &self.seed_ingress_url.is_some())
            .field("provider_gateway_count", &self.provider_gateways.len())
            .field("request_timeout_ms", &self.request_timeout_ms)
            .field(
                "namespace_delegation_configured",
                &self.namespace_delegation_file.is_some(),
            )
            .finish_non_exhaustive()
    }
}
/// One public provider identity and private provider-specific readback HTTPS base.
#[derive(Clone, norito::JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct MusubiPublicationProviderGateway {
    /// Lowercase hexadecimal public `SoraFS` provider identifier.
    pub provider_id: String,
    /// Provider-specific authenticated readback HTTPS base URL.
    pub url: String,
}
impl fmt::Debug for MusubiPublicationProviderGateway {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MusubiPublicationProviderGateway")
            .field("provider_id", &self.provider_id)
            .finish_non_exhaustive()
    }
}
/// SoraFS-specific configuration.
#[derive(Debug, Clone, ReadConfig)]
pub struct Sorafs {
    #[config(nested)]
    /// Alias cache policy applied to gateway responses.
    pub alias_cache: AliasCache,
    /// Rollout phase label controlling default anonymity policy.
    #[config(default = "defaults::sorafs::gateway::rollout_phase()")]
    pub rollout_phase: String,
    /// Default `SoraNet` anonymity policy stage for gateway fetches.
    pub anonymity_policy: Option<String>,
}
impl Default for Sorafs {
    fn default() -> Self {
        Self {
            alias_cache: AliasCache::default(),
            rollout_phase: defaults::sorafs::gateway::rollout_phase(),
            anonymity_policy: defaults::sorafs::gateway::anonymity_policy(),
        }
    }
}
/// Alias cache policy knobs surfaced to clients.
#[derive(Debug, Clone, Copy, ReadConfig)]
pub struct AliasCache {
    /// Positive TTL in seconds applied to cached alias proofs.
    #[config(default = "defaults::torii::SORAFS_ALIAS_POSITIVE_TTL_SECS")]
    pub positive_ttl: u64,
    /// Refresh window in seconds before the positive TTL elapses.
    #[config(default = "defaults::torii::SORAFS_ALIAS_REFRESH_WINDOW_SECS")]
    pub refresh_window: u64,
    /// Hard expiry in seconds after which stale proofs are rejected.
    #[config(default = "defaults::torii::SORAFS_ALIAS_HARD_EXPIRY_SECS")]
    pub hard_expiry: u64,
    /// Negative cache TTL in seconds for missing aliases.
    #[config(default = "defaults::torii::SORAFS_ALIAS_NEGATIVE_TTL_SECS")]
    pub negative_ttl: u64,
    /// TTL in seconds for revoked aliases (`410 Gone` responses).
    #[config(default = "defaults::torii::SORAFS_ALIAS_REVOCATION_TTL_SECS")]
    pub revocation_ttl: u64,
    /// Maximum age in seconds tolerated before alias proof bundles must rotate.
    #[config(default = "defaults::torii::SORAFS_ALIAS_ROTATION_MAX_AGE_SECS")]
    pub rotation_max_age: u64,
    /// Grace period in seconds applied after an approved successor manifest.
    #[config(default = "defaults::torii::SORAFS_ALIAS_SUCCESSOR_GRACE_SECS")]
    pub successor_grace: u64,
    /// Grace period in seconds applied to governance-driven alias rotations.
    #[config(default = "defaults::torii::SORAFS_ALIAS_GOVERNANCE_GRACE_SECS")]
    pub governance_grace: u64,
}
impl Default for AliasCache {
    fn default() -> Self {
        Self {
            positive_ttl: defaults::torii::SORAFS_ALIAS_POSITIVE_TTL_SECS,
            refresh_window: defaults::torii::SORAFS_ALIAS_REFRESH_WINDOW_SECS,
            hard_expiry: defaults::torii::SORAFS_ALIAS_HARD_EXPIRY_SECS,
            negative_ttl: defaults::torii::SORAFS_ALIAS_NEGATIVE_TTL_SECS,
            revocation_ttl: defaults::torii::SORAFS_ALIAS_REVOCATION_TTL_SECS,
            rotation_max_age: defaults::torii::SORAFS_ALIAS_ROTATION_MAX_AGE_SECS,
            successor_grace: defaults::torii::SORAFS_ALIAS_SUCCESSOR_GRACE_SECS,
            governance_grace: defaults::torii::SORAFS_ALIAS_GOVERNANCE_GRACE_SECS,
        }
    }
}
impl AliasCache {
    fn into_policy(self) -> AliasCachePolicy {
        AliasCachePolicy::new(
            Duration::from_secs(self.positive_ttl),
            Duration::from_secs(self.refresh_window),
            Duration::from_secs(self.hard_expiry),
            Duration::from_secs(self.negative_ttl),
            Duration::from_secs(self.revocation_ttl),
            Duration::from_secs(self.rotation_max_age),
            Duration::from_secs(self.successor_grace),
            Duration::from_secs(self.governance_grace),
        )
    }
}
#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, str::FromStr, time::Duration};
    use iroha_crypto::Algorithm;
    use super::*;
    fn root_with_timeouts(ttl: Duration, timeout: Duration) -> Root {
        let key_pair =
            KeyPair::try_from_seed(b"iroha:config:user:tests".to_vec(), Algorithm::Ed25519)
                .expect("derive user-config fixture key");
        Root {
            chain: ChainId::from_str("test-chain").expect("chain id"),
            network_id: Some(NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
                crate::data_model::block::BlockHeader,
            >::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([0xA5; iroha_crypto::Hash::LENGTH]),
            ))),
            network_id_file: None,
            torii_url: WithOrigin::inline(
                Url::parse("http://127.0.0.1:8080/torii").expect("valid torii url"),
            ),
            basic_auth: None,
            torii_request_timeout_ms: WithOrigin::inline(DurationMs::from(
                crate::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            )),
            account: Account {
                domain: "wonderland.universal".to_owned(),
                profile: Some(iroha_torii_shared::NETWORK_PROFILE_MINAMOTO.to_owned()),
                public_key: WithOrigin::inline(key_pair.public_key().clone()),
                private_key: Some(WithOrigin::inline(key_pair.private_key().clone())),
                private_key_file: None,
                chain_discriminant: WithOrigin::new(
                    defaults::common::chain_discriminant(),
                    ParameterOrigin::default(iroha_config_base::ParameterId::from([
                        "account",
                        "chain_discriminant",
                    ])),
                ),
            },
            transaction: Transaction {
                time_to_live_ms: WithOrigin::inline(DurationMs::from(ttl)),
                status_timeout_ms: WithOrigin::inline(DurationMs::from(timeout)),
                nonce: false,
            },
            connect: Connect::default(),
            sorafs: Sorafs::default(),
            soracloud: Soracloud::default(),
            musubi: Musubi::default(),
        }
    }
    #[test]
    fn root_with_timeouts_uses_checked_fixture_seed_derivation() {
        let root = root_with_timeouts(Duration::from_secs(5), Duration::from_secs(3));
        let expected =
            KeyPair::try_from_seed(b"iroha:config:user:tests".to_vec(), Algorithm::Ed25519)
                .expect("derive expected user-config fixture key");
        assert_eq!(root.account.public_key.value(), expected.public_key());
        assert_eq!(
            root.account
                .private_key
                .as_ref()
                .expect("inline private key")
                .value(),
            expected.private_key()
        );
    }
    #[test]
    fn network_id_file_supplies_the_exact_signing_domain() {
        let expected = NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            crate::data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"client file backed network identity"),
        ));
        let identity_file = tempfile::NamedTempFile::new().expect("identity file");
        fs::write(identity_file.path(), format!("{expected}\n")).expect("write identity");
        let mut emitter = Emitter::new();
        let resolved = resolve_network_id_source(
            None,
            Some(WithOrigin::inline(identity_file.path().to_path_buf())),
            &mut emitter,
        );
        assert_eq!(resolved, Some(expected));
        assert!(emitter.into_result().is_ok());
    }
    #[test]
    fn network_id_rejects_ambiguous_inline_and_file_sources() {
        let expected = NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            crate::data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"ambiguous client network identity"),
        ));
        let identity_file = tempfile::NamedTempFile::new().expect("identity file");
        fs::write(identity_file.path(), format!("{expected}\n")).expect("write identity");
        let mut emitter = Emitter::new();
        assert!(
            resolve_network_id_source(
                Some(expected),
                Some(WithOrigin::inline(identity_file.path().to_path_buf())),
                &mut emitter,
            )
            .is_none()
        );
        assert!(emitter.into_result().is_err());
    }
    #[test]
    fn parse_accepts_timeout_not_exceeding_ttl() {
        let ttl = Duration::from_secs(5);
        let timeout = Duration::from_secs(3);
        let config = root_with_timeouts(ttl, timeout)
            .parse()
            .expect("configuration should be valid");
        assert_eq!(config.transaction_ttl, ttl);
        assert_eq!(config.transaction_status_timeout, timeout);
    }
    #[test]
    fn parse_preserves_account_chain_discriminant() {
        let mut root = root_with_timeouts(Duration::from_secs(5), Duration::from_secs(3));
        root.account.profile = None;
        root.account.chain_discriminant = WithOrigin::inline(777);
        let config = root.parse().expect("configuration should be valid");
        assert_eq!(config.account_chain_discriminant, 777);
    }
    #[test]
    fn parse_uses_account_profile_chain_discriminant() {
        let mut root = root_with_timeouts(Duration::from_secs(5), Duration::from_secs(3));
        root.account.profile = Some(iroha_torii_shared::NETWORK_PROFILE_TAIRA.to_owned());
        let config = root.parse().expect("configuration should be valid");
        assert_eq!(
            config.account_chain_discriminant,
            iroha_torii_shared::TAIRA_CHAIN_DISCRIMINANT
        );
    }
    #[test]
    fn parse_rejects_account_profile_discriminant_mismatch() {
        let mut root = root_with_timeouts(Duration::from_secs(5), Duration::from_secs(3));
        root.account.profile = Some(iroha_torii_shared::NETWORK_PROFILE_TAIRA.to_owned());
        root.account.chain_discriminant = WithOrigin::inline(753);
        let err = root
            .parse()
            .expect_err("profile/discriminant mismatch should be rejected");
        let parse_errors: Vec<_> = err
            .frames()
            .filter_map(|frame| frame.downcast_ref::<ParseError>())
            .collect();
        assert!(
            parse_errors.iter().any(|error| {
                matches!(
                    error,
                    ParseError::AccountProfileDiscriminantMismatch {
                        profile,
                        expected,
                        actual
                    } if profile == iroha_torii_shared::NETWORK_PROFILE_TAIRA
                        && *expected == iroha_torii_shared::TAIRA_CHAIN_DISCRIMINANT
                        && *actual == 753
                )
            }),
            "expected profile mismatch error, found {parse_errors:?}"
        );
    }
    #[test]
    fn parse_uses_default_account_chain_discriminant_without_profile() {
        let mut root = root_with_timeouts(Duration::from_secs(5), Duration::from_secs(3));
        root.account.profile = None;
        let config = root.parse().expect("configuration should be valid");
        assert_eq!(
            config.account_chain_discriminant,
            defaults::common::chain_discriminant()
        );
    }
    #[test]
    fn parse_rejects_unknown_account_profile() {
        let mut root = root_with_timeouts(Duration::from_secs(5), Duration::from_secs(3));
        root.account.profile = Some("unknownnet".to_owned());
        let err = root
            .parse()
            .expect_err("unknown profile should be rejected");
        let parse_errors: Vec<_> = err
            .frames()
            .filter_map(|frame| frame.downcast_ref::<ParseError>())
            .collect();
        assert!(
            parse_errors.iter().any(|error| {
                matches!(
                    error,
                    ParseError::InvalidAccountProfile { profile } if profile == "unknownnet"
                )
            }),
            "expected invalid profile error, found {parse_errors:?}"
        );
    }
    #[test]
    fn parse_preserves_torii_request_timeout() {
        let mut root = root_with_timeouts(Duration::from_secs(5), Duration::from_secs(3));
        let timeout = Duration::from_secs(7);
        root.torii_request_timeout_ms = WithOrigin::inline(DurationMs::from(timeout));
        let config = root.parse().expect("configuration should be valid");
        assert_eq!(config.torii_request_timeout, timeout);
    }
    #[test]
    fn parse_preserves_soracloud_http_witness_file() {
        let mut root = root_with_timeouts(Duration::from_secs(5), Duration::from_secs(3));
        let witness_file = PathBuf::from("/tmp/soracloud-witness.json");
        root.soracloud.http_witness_file = Some(witness_file.clone());
        let config = root.parse().expect("configuration should be valid");
        assert_eq!(config.soracloud_http_witness_file, Some(witness_file));
    }
    #[test]
    fn parse_rejects_timeout_exceeding_ttl() {
        let err = root_with_timeouts(Duration::from_secs(2), Duration::from_secs(3))
            .parse()
            .expect_err("timeout longer than TTL should be rejected");
        let parse_errors: Vec<_> = err
            .frames()
            .filter_map(|frame| frame.downcast_ref::<ParseError>())
            .collect();
        assert!(
            parse_errors
                .iter()
                .any(|error| matches!(error, ParseError::TxTimeoutVsTtl)),
            "expected `ParseError::TxTimeoutVsTtl`, found {parse_errors:?}"
        );
        assert!(format!("{err:?}").contains("transaction status timeout must not exceed TTL"));
    }
    #[test]
    fn parse_rejects_empty_connect_queue_root() {
        let mut root = root_with_timeouts(Duration::from_secs(5), Duration::from_secs(3));
        root.connect.queue_root = WithOrigin::inline(PathBuf::new());
        let err = root
            .parse()
            .expect_err("empty connect.queue_root should be rejected");
        let parse_errors: Vec<_> = err
            .frames()
            .filter_map(|frame| frame.downcast_ref::<ParseError>())
            .collect();
        assert!(
            parse_errors
                .iter()
                .any(|error| matches!(error, ParseError::EmptyConnectQueueRoot)),
            "expected `ParseError::EmptyConnectQueueRoot`, found {parse_errors:?}"
        );
    }
    #[test]
    fn parse_accepts_dataspace_account_domain_scope() {
        let mut root = root_with_timeouts(Duration::from_secs(5), Duration::from_secs(3));
        root.account.domain = "wonderland".to_owned();
        let config = root
            .parse()
            .expect("bare dataspace account scope should be accepted");
        assert_eq!(config.chain.as_str(), "test-chain");
    }
    #[test]
    fn parse_rejects_invalid_account_domain_scope_without_panicking() {
        let mut root = root_with_timeouts(Duration::from_secs(5), Duration::from_secs(3));
        root.account.domain = "wonderland universal".to_owned();
        let err = root
            .parse()
            .expect_err("invalid account scope should be rejected");
        let parse_errors: Vec<_> = err
            .frames()
            .filter_map(|frame| frame.downcast_ref::<ParseError>())
            .collect();
        assert!(
            parse_errors
                .iter()
                .any(|error| matches!(error, ParseError::InvalidAccountDomain { value } if value == "wonderland universal")),
            "expected `ParseError::InvalidAccountDomain`, found {parse_errors:?}"
        );
    }
}
