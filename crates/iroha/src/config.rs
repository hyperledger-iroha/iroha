//! Module for client-related configuration and structs
use crate::{
    crypto::KeyPair,
    data_model::{ChainId, NetworkId, prelude::*},
};
use core::str::FromStr;
use derive_more::Display;
use error_stack::{Report, ResultExt};
use eyre::Result;
use iroha_config::parameters::actual::SorafsRolloutPhase;
use iroha_config_base::{env::ReadEnv, read::ConfigReader, toml::TomlSource};
use iroha_primitives::small::SmallStr;
use norito::json::{self, JsonDeserialize, JsonSerialize};
/// Re-exported `SoraNet` anonymity policy for client configuration.
pub use sorafs_orchestrator::AnonymityPolicy;
use std::{
    env,
    path::{Path, PathBuf},
    time::Duration,
};
use url::Url;
mod user;
use crate::secrecy::SecretString;
pub use user::{
    MusubiFetch as MusubiFetchConfig,
    MusubiFetchProviderGateway as MusubiFetchProviderGatewayConfig,
    MusubiPublication as MusubiPublicationConfig,
    MusubiPublicationProviderGateway as MusubiPublicationProviderGatewayConfig, ParseError,
    Root as UserConfig,
};
type ReportResult<T, E> = core::result::Result<T, Report<[E]>>;
/// Default time-to-live for transactions submitted via the client API.
pub const DEFAULT_TRANSACTION_TIME_TO_LIVE: Duration = Duration::from_secs(100);
/// Mandatory lifetime of one signed query request.
///
/// Query requests are one-shot and are never automatically re-signed on retry. The node rejects
/// requests whose lifetime exceeds its configured replay-retention window.
pub const DEFAULT_QUERY_TIME_TO_LIVE: Duration = Duration::from_secs(100);
/// Default timeout for waiting on transaction status updates.
pub const DEFAULT_TRANSACTION_STATUS_TIMEOUT: Duration = Duration::from_secs(15);
/// Default timeout for Torii HTTP requests issued by the client.
///
/// This must remain above the Nexus routed/fanout HTTP budget so clients do
/// not abandon a request while Torii is still within its allowed route window.
pub const DEFAULT_TORII_REQUEST_TIMEOUT: Duration = Duration::from_secs(70);
/// Whether to add a random transaction nonce by default.
pub const DEFAULT_TRANSACTION_NONCE: bool = false;
/// Default Connect queue root (`~/.iroha/connect` on Unix, `%USERPROFILE%\.iroha\connect` on Windows).
#[must_use]
pub fn default_connect_queue_root() -> PathBuf {
    let mut base = if cfg!(windows) {
        env::var_os("USERPROFILE").map(PathBuf::from)
    } else {
        env::var_os("HOME").map(PathBuf::from)
    }
    .unwrap_or_else(|| PathBuf::from("."));
    base.push(".iroha");
    base.push("connect");
    base
}
/// Valid web auth login string. See [`WebLogin::from_str`]
#[derive(Debug, Display, Clone, PartialEq, Eq)]
pub struct WebLogin(SmallStr);
impl FromStr for WebLogin {
    type Err = eyre::ErrReport;
    /// Validates that the string is a valid web login
    ///
    /// # Errors
    /// Fails if `login` contains `:` character, which is the binary representation of the '\0'.
    fn from_str(login: &str) -> Result<Self> {
        if login.contains(':') {
            eyre::bail!("The `:` character, in `{login}` is not allowed");
        }
        Ok(Self(SmallStr::from_str(login)))
    }
}
impl WebLogin {
    /// Return the underlying login as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}
impl JsonSerialize for WebLogin {
    fn json_serialize(&self, out: &mut String) {
        self.as_str().json_serialize(out);
    }
}
impl JsonDeserialize for WebLogin {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let raw = parser.parse_string()?;
        Self::from_str(&raw).map_err(|err| json::Error::Message(err.to_string()))
    }
}
/// Basic Authentication credentials
#[derive(Clone, Debug)]
pub struct BasicAuth {
    /// Login for Basic Authentication
    pub web_login: WebLogin,
    /// Password for Basic Authentication
    pub password: SecretString,
}
impl JsonSerialize for BasicAuth {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"web_login\":");
        self.web_login.json_serialize(out);
        out.push(',');
        out.push_str("\"password\":");
        self.password.json_serialize(out);
        out.push('}');
    }
}
impl JsonDeserialize for BasicAuth {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let mut map = json::MapVisitor::new(parser)?;
        let mut web_login: Option<WebLogin> = None;
        let mut password: Option<SecretString> = None;
        while let Some(key) = map.next_key()? {
            match key.as_str() {
                "web_login" => {
                    if web_login.is_some() {
                        return Err(json::Error::duplicate_field("web_login"));
                    }
                    web_login = Some(map.parse_value::<WebLogin>()?);
                }
                "password" => {
                    if password.is_some() {
                        return Err(json::Error::duplicate_field("password"));
                    }
                    password = Some(map.parse_value::<SecretString>()?);
                }
                _ => map.skip_value()?,
            }
        }
        map.finish()?;
        Ok(Self {
            web_login: web_login.ok_or_else(|| json::Error::missing_field("web_login"))?,
            password: password.ok_or_else(|| json::Error::missing_field("password"))?,
        })
    }
}
/// Complete client configuration.
#[derive(Clone, Debug)]
pub struct Config {
    /// Unique chain identifier the client connects to.
    pub chain: ChainId,
    /// Exact genesis-lineage identity the client signs into query requests.
    pub network_id: NetworkId,
    /// Account ID used for signing and submitting transactions.
    pub account: AccountId,
    /// I105 chain discriminant used when parsing and rendering account literals.
    pub account_chain_discriminant: u16,
    /// Key pair corresponding to the account.
    pub key_pair: KeyPair,
    /// Optional Basic Auth credentials for HTTP.
    pub basic_auth: Option<BasicAuth>,
    /// Torii API base URL.
    pub torii_api_url: Url,
    /// Timeout for Torii HTTP requests.
    pub torii_request_timeout: Duration,
    /// Transaction time-to-live.
    pub transaction_ttl: Duration,
    /// Timeout for waiting on transaction status.
    pub transaction_status_timeout: Duration,
    /// Whether to add a random nonce to transactions.
    pub transaction_add_nonce: bool,
    /// Root directory containing Connect queue state for diagnostics and offline replay helpers.
    pub connect_queue_root: PathBuf,
    /// Optional JSON witness file used for multisig-signed Soracloud HTTP requests.
    pub soracloud_http_witness_file: Option<PathBuf>,
    /// Alias cache policy applied when validating `SoraFS` proofs.
    pub sorafs_alias_cache: sorafs_manifest::alias_cache::AliasCachePolicy,
    /// Default `SoraNet` anonymity policy stage for gateway fetches.
    pub sorafs_anonymity_policy: AnonymityPolicy,
    /// Configured rollout phase for staged PQ activation.
    pub sorafs_rollout_phase: SorafsRolloutPhase,
}
/// An error type for [`Config::load`]
#[derive(thiserror::Error, Debug, Copy, Clone)]
#[error("Failed to load configuration")]
pub struct LoadError;
/// Invalid signer-free account network context from a client configuration.
#[derive(thiserror::Error, Debug, Copy, Clone, PartialEq, Eq)]
pub enum AccountChainDiscriminantError {
    /// The configured public network profile is unknown.
    #[error("unknown account network profile")]
    UnknownProfile,
    /// An explicit discriminant disagrees with the selected public profile.
    #[error("account network profile and chain discriminant disagree")]
    ProfileMismatch,
    /// Zero is not a valid public account chain discriminant.
    #[error("account chain discriminant must be nonzero")]
    Zero,
}
/// Resolve a public account profile and optional explicit I105 chain discriminant.
///
/// This helper does not parse or construct an account or key pair, so signer-free
/// clients can validate the same public network context as [`Config::load`].
///
/// # Errors
/// Returns an error for an unknown profile, a profile/discriminant mismatch, or zero.
pub fn resolve_account_chain_discriminant(
    profile: Option<&str>,
    explicit: Option<u16>,
) -> Result<u16, AccountChainDiscriminantError> {
    let profile = profile.map(str::trim).filter(|profile| !profile.is_empty());
    let discriminant = if let Some(profile) = profile {
        let profile = iroha_torii_shared::network_profile(profile)
            .ok_or(AccountChainDiscriminantError::UnknownProfile)?;
        if explicit.is_some_and(|value| value != profile.chain_discriminant) {
            return Err(AccountChainDiscriminantError::ProfileMismatch);
        }
        profile.chain_discriminant
    } else {
        explicit.unwrap_or_else(iroha_config::parameters::defaults::common::chain_discriminant)
    };
    if discriminant == 0 {
        return Err(AccountChainDiscriminantError::Zero);
    }
    Ok(discriminant)
}
/// Where to load configuration from
pub enum LoadPath<P> {
    /// Path specified explicitly, therefore, loading will fail if the file is not found
    Explicit(P),
    /// Using the default path, therefore, loading will not fail if the file is not found
    Default(P),
}
impl Config {
    /// Load one required client configuration file without consulting process environment.
    ///
    /// This is intended for security-sensitive tools whose credential provenance must be the
    /// explicitly selected configuration file. Unlike [`Self::load`], no environment fallback
    /// or override is applied.
    ///
    /// # Errors
    /// Returns an error when the file cannot be read, its TOML is invalid, or the completed
    /// client configuration fails validation.
    pub fn load_file(path: impl AsRef<Path>) -> ReportResult<Self, LoadError> {
        let toml_source = TomlSource::from_file(path).change_context(LoadError)?;
        let config = ConfigReader::new()
            .with_toml_source(toml_source)
            .with_env(|_: &str| None::<std::borrow::Cow<'static, str>>)
            .read_and_complete::<user::Root>()
            .change_context(LoadError)?
            .parse()
            .change_context(LoadError)?;
        Ok(config)
    }
    /// Load a required platform client file and return its typed Musubi publication subtree.
    ///
    /// This path does not consult environment variables. Service URLs remain encapsulated in the
    /// returned redacting configuration and are never copied into the generic [`Client`](crate::client::Client).
    ///
    /// # Errors
    /// Returns an error when the selected file cannot be read, contains unknown or invalid
    /// parameters, or fails client configuration validation.
    pub fn load_file_with_musubi_publication(
        path: impl AsRef<Path>,
    ) -> ReportResult<(Self, MusubiPublicationConfig), LoadError> {
        let toml_source = TomlSource::from_file(path).change_context(LoadError)?;
        Self::load_source_with_musubi_publication(toml_source)
    }
    /// Parse an already-read client TOML source and return its Musubi publication subtree.
    ///
    /// Security-sensitive callers use this entry point after opening a configuration file with
    /// no-follow semantics and reading it from one stable descriptor. The supplied `path` is
    /// retained as configuration provenance and as the base for relative public-proof paths;
    /// this function never reopens it.
    ///
    /// # Errors
    /// Returns an error when `bytes` are not UTF-8 TOML, contain unknown or invalid parameters,
    /// or fail client configuration validation.
    pub fn load_bytes_with_musubi_publication(
        path: impl AsRef<Path>,
        bytes: &[u8],
    ) -> ReportResult<(Self, MusubiPublicationConfig), LoadError> {
        let source = core::str::from_utf8(bytes).change_context(LoadError)?;
        let table = source.parse::<toml::Table>().change_context(LoadError)?;
        Self::load_source_with_musubi_publication(TomlSource::new(
            path.as_ref().to_path_buf(),
            table,
        ))
    }
    fn load_source_with_musubi_publication(
        toml_source: TomlSource,
    ) -> ReportResult<(Self, MusubiPublicationConfig), LoadError> {
        Ok(ConfigReader::new()
            .with_toml_source(toml_source)
            .with_env(|_: &str| None::<std::borrow::Cow<'static, str>>)
            .read_and_complete::<user::Root>()
            .change_context(LoadError)?
            .parse_with_musubi()
            .change_context(LoadError)?)
    }
    /// Loads configuration from a file
    ///
    /// # Errors
    /// - unable to load config from a TOML file
    /// - the config is invalid
    pub fn load(path: LoadPath<impl AsRef<Path>>) -> ReportResult<Self, LoadError> {
        Self::load_with_env(path, Box::new(iroha_config_base::env::std_env))
    }
    fn load_with_env(
        path: LoadPath<impl AsRef<Path>>,
        env: impl ReadEnv + 'static,
    ) -> ReportResult<Self, LoadError> {
        let toml_source = match path {
            LoadPath::Explicit(path) => {
                Some(TomlSource::from_file(path).change_context(LoadError)?)
            }
            LoadPath::Default(path) => match TomlSource::from_file(path) {
                Ok(x) => Some(x),
                Err(err)
                    if matches!(
                        err.current_context(),
                        iroha_config_base::toml::FromFileError::Read
                    ) =>
                {
                    None
                }
                Err(err) => Err(err).change_context(LoadError)?,
            },
        };
        let config = toml_source
            .map_or_else(ConfigReader::new, |x| {
                ConfigReader::new().with_toml_source(x)
            })
            .with_env(env)
            .read_and_complete::<user::Root>()
            .change_context(LoadError)?
            .parse()
            .change_context(LoadError)?;
        Ok(config)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use assertables::assert_contains;
    use iroha_config_base::env::MockEnv;
    use iroha_crypto::ExposedPrivateKey;
    use std::{collections::HashSet, io::Write};
    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked config fixture keypair")
    }
    #[test]
    fn web_login_ok() {
        let _ok: WebLogin = "alice".parse().expect("input is valid");
    }
    #[test]
    fn web_login_bad() {
        let _err = "alice:wonderland"
            .parse::<WebLogin>()
            .expect_err("input has `:`");
    }
    fn config_sample() -> toml::Table {
        toml::toml! {
            chain = "00000000-0000-0000-0000-000000000000"
            network_id = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
            torii_url = "http://127.0.0.1:8080/"
            [basic_auth]
            web_login = "mad_hatter"
            password = "ilovetea"
            [account]
            domain = "wonderland.universal"
            public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            private_key = "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
            [transaction]
            time_to_live_ms = 100_000
            status_timeout_ms = 100_000
            nonce = false
        }
    }
    #[test]
    fn parse_full_toml_config() {
        ConfigReader::new()
            .with_toml_source(TomlSource::inline(config_sample()))
            .read_and_complete::<user::Root>()
            .unwrap();
    }
    #[test]
    fn account_private_key_file_populates_signer() {
        let mut table = config_sample();
        let account = table
            .get_mut("account")
            .and_then(toml::Value::as_table_mut)
            .expect("client account table");
        let private_key = account
            .remove("private_key")
            .and_then(|value| value.as_str().map(str::to_owned))
            .expect("inline client private key");
        let mut key_file = tempfile::NamedTempFile::new().expect("client private-key file");
        writeln!(key_file, "{private_key}").expect("write client private-key file");
        key_file.flush().expect("flush client private-key file");
        account.insert(
            "private_key_file".into(),
            toml::Value::String(key_file.path().to_string_lossy().into_owned()),
        );
        let config = ConfigReader::new()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<user::Root>()
            .expect("file-backed client config should complete")
            .parse()
            .expect("file-backed client config should parse");
        assert_eq!(
            ExposedPrivateKey(config.key_pair.private_key().clone()).to_string(),
            private_key
        );
    }
    #[test]
    fn account_private_key_sources_are_mutually_exclusive() {
        let mut table = config_sample();
        let account = table
            .get_mut("account")
            .and_then(toml::Value::as_table_mut)
            .expect("client account table");
        let key_file = tempfile::NamedTempFile::new().expect("client private-key file");
        account.insert(
            "private_key_file".into(),
            toml::Value::String(key_file.path().to_string_lossy().into_owned()),
        );
        let error = ConfigReader::new()
            .with_toml_source(TomlSource::inline(table))
            .read_and_complete::<user::Root>()
            .expect("duplicate private sources remain structurally readable")
            .parse()
            .expect_err("duplicate client private sources must fail");
        assert_contains!(
            format!("{error:#?}"),
            "account.private_key and account.private_key_file are mutually exclusive"
        );
    }
    #[test]
    fn torii_url_scheme_support() {
        fn with_scheme(scheme: &str) -> ReportResult<Config, user::ParseError> {
            ConfigReader::new()
                .with_toml_source(TomlSource::inline(config_sample()))
                .with_env(MockEnv::from([(
                    "TORII_URL",
                    format!("{scheme}://127.0.0.1:8080"),
                )]))
                .read_and_complete::<user::Root>()
                .unwrap()
                .parse()
        }
        let _ = with_scheme("http").expect("should be fine");
        let _ = with_scheme("https").expect("should be fine");
        let _ = with_scheme("ws").expect_err("not supported");
    }
    #[test]
    fn torii_url_ensure_trailing_slash() {
        let config = ConfigReader::new()
            .with_toml_source(TomlSource::inline(config_sample()))
            .with_env(MockEnv::from([("TORII_URL", "http://127.0.0.1/peer-1")]))
            .read_and_complete::<user::Root>()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(config.torii_api_url.as_str(), "http://127.0.0.1/peer-1/");
    }
    #[test]
    fn invalid_toml_file_is_handled_properly() {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"not a valid toml").unwrap();
        let err =
            Config::load(LoadPath::Explicit(file.path())).expect_err("should fail on toml parsing");
        assert_contains!(
            format!("{err:#?}"),
            "Error while deserializing file contents as TOML"
        );
    }
    #[test]
    fn reads_default_path() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(toml::to_string(&config_sample()).unwrap().as_bytes())
            .unwrap();
        let config = Config::load(LoadPath::Default(file.path())).unwrap();
        assert_eq!(
            config.account.expect_single_signatory().to_string(),
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
        );
    }
    #[test]
    fn load_file_requires_and_parses_the_selected_file() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(toml::to_string(&config_sample()).unwrap().as_bytes())
            .unwrap();
        let config = Config::load_file(file.path()).expect("load explicit file without fallback");
        assert_eq!(config.torii_api_url.as_str(), "http://127.0.0.1:8080/");
        assert!(
            Config::load_file(file.path().with_extension("missing")).is_err(),
            "the selected file is mandatory"
        );
    }
    #[test]
    fn load_bytes_with_musubi_publication_never_reopens_path() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let path = temporary.path().join("already-read-client.toml");
        let bytes = toml::to_string(&config_sample()).expect("serialize client fixture");
        let (config, publication) =
            Config::load_bytes_with_musubi_publication(&path, bytes.as_bytes())
                .expect("parse supplied client bytes");
        assert_eq!(config.torii_api_url.as_str(), "http://127.0.0.1:8080/");
        assert!(publication.seed_ingress_url.is_none());
        assert!(
            !path.exists(),
            "parsing an already-read source must not create or reopen its provenance path"
        );
    }
    #[test]
    fn signer_free_chain_discriminant_resolution_matches_profiles() {
        assert_eq!(
            resolve_account_chain_discriminant(Some("taira"), None).expect("known profile"),
            iroha_torii_shared::TAIRA_CHAIN_DISCRIMINANT
        );
        assert_eq!(
            resolve_account_chain_discriminant(None, Some(777)).expect("explicit discriminant"),
            777
        );
        assert_eq!(
            resolve_account_chain_discriminant(Some("taira"), Some(753)),
            Err(AccountChainDiscriminantError::ProfileMismatch)
        );
        assert_eq!(
            resolve_account_chain_discriminant(None, Some(0)),
            Err(AccountChainDiscriminantError::Zero)
        );
    }
    #[test]
    fn full_env_fallback() {
        let key = checked_random_keypair();
        let env = MockEnv::new()
            .set("CHAIN", "wonder")
            .set(
                "NETWORK_ID",
                "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
            )
            .set("TORII_URL", "http://localhost:8080")
            .set("ACCOUNT_PROFILE", iroha_torii_shared::NETWORK_PROFILE_TAIRA)
            .set(
                "ACCOUNT_CHAIN_DISCRIMINANT",
                iroha_torii_shared::TAIRA_CHAIN_DISCRIMINANT.to_string(),
            )
            .set("ACCOUNT_DOMAIN", "land.universal")
            .set(
                "ACCOUNT_PRIVATE_KEY",
                ExposedPrivateKey(key.private_key().clone()).to_string(),
            )
            .set("ACCOUNT_PUBLIC_KEY", key.public_key().to_string());
        let _config =
            Config::load_with_env(LoadPath::Default("non_existing_path"), env.clone()).unwrap();
        assert_eq!(env.unvisited(), HashSet::new());
        assert_eq!(
            env.unknown(),
            HashSet::from(["ACCOUNT_PRIVATE_KEY_FILE".to_owned()]),
            "the mutually exclusive file-backed private-key source must still be inspected"
        );
    }
}
