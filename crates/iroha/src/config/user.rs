//! User configuration view.

use std::{path::PathBuf, time::Duration};

use error_stack::{Report, ResultExt};
use iroha_config::parameters::{actual::SorafsRolloutPhase, defaults};
use iroha_config_base::{
    ParameterOrigin, ReadConfig, WithOrigin,
    attach::ConfigValueAndOrigin,
    util::{DurationMs, Emitter, EmitterResultExt},
};
use sorafs_manifest::alias_cache::AliasCachePolicy;
use sorafs_orchestrator::AnonymityPolicy;
use url::Url;

use crate::{
    config::BasicAuth,
    crypto::{KeyPair, PrivateKey, PublicKey},
    data_model::prelude::{AccountId, ChainId, DomainId},
};

/// Minimal allowed transaction time-to-live.
const MIN_TRANSACTION_TTL: Duration = Duration::from_secs(1);
const PUBLIC_TAIRA_CHAIN_ID: &str = "809574f5-fee7-5e69-bfcf-52451e42d50f";
const PUBLIC_NEXUS_CHAIN_ID: &str = "00000000-0000-0000-0000-000000000753";
const TAIRA_CHAIN_DISCRIMINANT: u16 = 369;
const NEXUS_CHAIN_DISCRIMINANT: u16 = 753;

fn known_chain_discriminant_for_chain_id(chain_id: &ChainId) -> Option<u16> {
    match chain_id.as_str() {
        "iroha3-taira" | PUBLIC_TAIRA_CHAIN_ID => Some(TAIRA_CHAIN_DISCRIMINANT),
        "iroha3-nexus" | PUBLIC_NEXUS_CHAIN_ID => Some(NEXUS_CHAIN_DISCRIMINANT),
        _ => None,
    }
}

/// Root of the user-facing configuration loaded from TOML + env.
#[derive(Clone, Debug, ReadConfig)]
pub struct Root {
    /// Unique chain identifier.
    #[config(env = "CHAIN")]
    pub chain: ChainId,
    /// Torii API URL.
    #[config(env = "TORII_URL")]
    pub torii_url: WithOrigin<Url>,
    /// Torii API version label (semantic `major.minor`).
    #[config(
        env = "TORII_API_VERSION",
        default = "defaults::torii::api_default_version()"
    )]
    pub torii_api_version: WithOrigin<String>,
    /// Minimum Torii API version required for proof/staking/fee endpoints.
    #[config(default = "defaults::torii::api_min_proof_version()")]
    pub torii_api_min_proof_version: WithOrigin<String>,
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
    /// Invalid Torii API version label.
    #[error("Invalid Torii API version: `{value}`")]
    InvalidToriiApiVersion {
        /// Raw label supplied by the user or env.
        value: String,
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
    #[error("Account domain must use `domain.dataspace` format: `{value}`")]
    InvalidAccountDomain {
        /// Raw configured value.
        value: String,
    },
}

type ReportResult<T, E> = core::result::Result<T, Report<[E]>>;

impl Root {
    /// Validates user configuration for semantic errors and constructs a complete
    /// [`super::Config`].
    ///
    /// # Errors
    /// If a set of validity errors occurs.
    #[allow(clippy::too_many_lines)]
    pub fn parse(self) -> ReportResult<super::Config, ParseError> {
        let Self {
            chain: chain_id,
            torii_url,
            torii_api_version,
            torii_api_min_proof_version,
            basic_auth,
            torii_request_timeout_ms,
            account:
                Account {
                    domain: domain_literal,
                    public_key,
                    private_key,
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
        } = self;

        let mut emitter = Emitter::new();

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
        let (torii_api_version, torii_api_version_origin) = torii_api_version.into_tuple();
        let torii_api_version = match super::parse_api_version_label(&torii_api_version) {
            Ok(label) => label,
            Err(reason) => {
                emitter.emit(
                    Report::new(ParseError::InvalidToriiApiVersion {
                        value: torii_api_version.clone(),
                    })
                    .attach(ConfigValueAndOrigin::new(
                        torii_api_version.clone(),
                        torii_api_version_origin,
                    ))
                    .attach(format!(
                        "Torii API versions must use `major.minor` labels: {reason}"
                    )),
                );
                super::DEFAULT_TORII_API_VERSION.to_string()
            }
        };
        let (torii_api_min_proof_version, torii_api_min_proof_version_origin) =
            torii_api_min_proof_version.into_tuple();
        let torii_api_min_proof_version =
            match super::parse_api_version_label(&torii_api_min_proof_version) {
                Ok(label) => label,
                Err(reason) => {
                    emitter.emit(
                        Report::new(ParseError::InvalidToriiApiVersion {
                            value: torii_api_min_proof_version.clone(),
                        })
                        .attach(ConfigValueAndOrigin::new(
                            torii_api_min_proof_version.clone(),
                            torii_api_min_proof_version_origin.clone(),
                        ))
                        .attach(format!(
                            "Torii API versions must use `major.minor` labels: {reason}"
                        )),
                    );
                    torii_api_version.clone()
                }
            };
        let version_key = |label: &str| -> Option<(u16, u16)> {
            let mut parts = label.split('.');
            let major = parts.next()?.parse::<u16>().ok()?;
            let minor = parts.next()?.parse::<u16>().ok()?;
            Some((major, minor))
        };
        if let (Some(min_tuple), Some(default_tuple)) = (
            version_key(&torii_api_min_proof_version),
            version_key(&torii_api_version),
        ) && min_tuple > default_tuple
        {
            emitter.emit(
                Report::new(ParseError::InvalidToriiApiVersion {
                    value: torii_api_min_proof_version.clone(),
                })
                .attach(ConfigValueAndOrigin::new(
                    torii_api_min_proof_version.clone(),
                    torii_api_min_proof_version_origin,
                ))
                .attach(format!(
                    "torii_api_min_proof_version `{torii_api_min_proof_version}` must not exceed torii_api_version `{torii_api_version}`",
                )),
            );
        }

        let chain_discriminant = match chain_discriminant.origin() {
            ParameterOrigin::Default { .. } => known_chain_discriminant_for_chain_id(&chain_id)
                .map(|value| {
                    WithOrigin::new(
                        value,
                        ParameterOrigin::custom(format!(
                            "derived from chain `{}`",
                            chain_id.as_str()
                        )),
                    )
                })
                .unwrap_or(chain_discriminant),
            _ => chain_discriminant,
        };

        let (public_key, public_key_origin) = public_key.into_tuple();
        let (private_key, private_key_origin) = private_key.into_tuple();
        if DomainId::parse_fully_qualified(&domain_literal).is_err() {
            emitter.emit(Report::new(ParseError::InvalidAccountDomain {
                value: domain_literal.clone(),
            }));
        }
        let key_pair = KeyPair::new(public_key.clone(), private_key)
            .attach(ConfigValueAndOrigin::new("[REDACTED]", public_key_origin))
            .attach(ConfigValueAndOrigin::new("[REDACTED]", private_key_origin))
            .change_context(ParseError::KeyPair)
            .ok_or_emit(&mut emitter);
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
                    .attach("invalid `sorafs.rollout_phase`; expected canary|ramp|default or stage_a|stage_b|stage_c aliases"),
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
                            "invalid `sorafs.anonymity_policy` value `{label}`; expected anon-guard-pq|anon-majority-pq|anon-strict-pq or stage_a/stage_b/stage_c aliases"
                        )),
                    );
                    phase_default_policy
                })
            });

        emitter.into_result()?;

        Ok(super::Config {
            chain: chain_id,
            account: account_id,
            account_chain_discriminant: chain_discriminant.into_value(),
            key_pair: key_pair.unwrap(),
            torii_api_url,
            torii_api_version,
            torii_api_min_proof_version,
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
        })
    }
}

/// Account parameters for building the default signer identity.
#[derive(Debug, Clone, ReadConfig)]
pub struct Account {
    /// Domain of the account.
    #[config(env = "ACCOUNT_DOMAIN")]
    pub domain: String,
    /// Public key of the account.
    #[config(env = "ACCOUNT_PUBLIC_KEY")]
    pub public_key: WithOrigin<PublicKey>,
    /// Private key of the account.
    #[config(env = "ACCOUNT_PRIVATE_KEY")]
    pub private_key: WithOrigin<PrivateKey>,
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
    use std::{path::PathBuf, str::FromStr, time::Duration};

    use iroha_crypto::Algorithm;

    use super::*;

    fn root_with_timeouts(ttl: Duration, timeout: Duration) -> Root {
        let key_pair = KeyPair::from_seed(vec![0; 32], Algorithm::Ed25519);
        Root {
            chain: ChainId::from_str("test-chain").expect("chain id"),
            torii_url: WithOrigin::inline(
                Url::parse("http://127.0.0.1:8080/torii").expect("valid torii url"),
            ),
            torii_api_version: WithOrigin::inline(defaults::torii::api_default_version()),
            torii_api_min_proof_version: WithOrigin::inline(
                defaults::torii::api_min_proof_version(),
            ),
            basic_auth: None,
            torii_request_timeout_ms: WithOrigin::inline(DurationMs::from(
                crate::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            )),
            account: Account {
                domain: "wonderland.universal".to_owned(),
                public_key: WithOrigin::inline(key_pair.public_key().clone()),
                private_key: WithOrigin::inline(key_pair.private_key().clone()),
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
        }
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
        root.account.chain_discriminant = WithOrigin::inline(777);

        let config = root.parse().expect("configuration should be valid");

        assert_eq!(config.account_chain_discriminant, 777);
    }

    #[test]
    fn parse_infers_taira_account_chain_discriminant_from_chain_id() {
        let mut root = root_with_timeouts(Duration::from_secs(5), Duration::from_secs(3));
        root.chain = ChainId::from(PUBLIC_TAIRA_CHAIN_ID);

        let config = root.parse().expect("configuration should be valid");

        assert_eq!(config.account_chain_discriminant, TAIRA_CHAIN_DISCRIMINANT);
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
    fn parse_rejects_bare_account_domain_without_panicking() {
        let mut root = root_with_timeouts(Duration::from_secs(5), Duration::from_secs(3));
        root.account.domain = "wonderland".to_owned();

        let err = root
            .parse()
            .expect_err("bare account domain should be rejected");
        let parse_errors: Vec<_> = err
            .frames()
            .filter_map(|frame| frame.downcast_ref::<ParseError>())
            .collect();
        assert!(
            parse_errors
                .iter()
                .any(|error| matches!(error, ParseError::InvalidAccountDomain { value } if value == "wonderland")),
            "expected `ParseError::InvalidAccountDomain`, found {parse_errors:?}"
        );
    }
}
