//! Module for client-related configuration and structs

use core::str::FromStr;
use std::{
    env,
    path::{Path, PathBuf},
    time::Duration,
};

use derive_more::Display;
use error_stack::{Report, ResultExt};
use eyre::Result;
use iroha_config::parameters::{actual::SorafsRolloutPhase, defaults};
use iroha_config_base::{env::ReadEnv, read::ConfigReader, toml::TomlSource};
use iroha_primitives::small::SmallStr;
use norito::json::{self, JsonDeserialize, JsonSerialize};
/// Re-exported `SoraNet` anonymity policy for client configuration.
pub use sorafs_orchestrator::AnonymityPolicy;
use url::Url;

use crate::{
    crypto::KeyPair,
    data_model::{ChainId, prelude::*},
};

mod user;

pub use user::{ParseError, Root as UserConfig};

use crate::secrecy::SecretString;

type ReportResult<T, E> = core::result::Result<T, Report<[E]>>;

/// Default time-to-live for transactions submitted via the client API.
pub const DEFAULT_TRANSACTION_TIME_TO_LIVE: Duration = Duration::from_secs(100);
/// Default timeout for waiting on transaction status updates.
pub const DEFAULT_TRANSACTION_STATUS_TIMEOUT: Duration = Duration::from_secs(15);
/// Default timeout for Torii HTTP requests issued by the client.
pub const DEFAULT_TORII_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Whether to add a random transaction nonce by default.
pub const DEFAULT_TRANSACTION_NONCE: bool = false;
/// Default Torii API version header sent by the client.
pub const DEFAULT_TORII_API_VERSION: &str = defaults::torii::API_DEFAULT_VERSION;
/// Default minimum Torii API version used for proof/staking/fee endpoints.
pub const DEFAULT_TORII_API_MIN_PROOF_VERSION: &str = defaults::torii::API_MIN_PROOF_VERSION;

/// Default Torii API version as an owned string.
#[must_use]
pub fn default_torii_api_version() -> String {
    DEFAULT_TORII_API_VERSION.to_string()
}

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
    /// Account ID used for signing and submitting transactions.
    pub account: AccountId,
    /// Key pair corresponding to the account.
    pub key_pair: KeyPair,
    /// Optional Basic Auth credentials for HTTP.
    pub basic_auth: Option<BasicAuth>,
    /// Torii API base URL.
    pub torii_api_url: Url,
    /// Torii API version label (semantic `major.minor`) sent on every request.
    pub torii_api_version: String,
    /// Minimum Torii API version used for proof/staking/fee endpoints.
    pub torii_api_min_proof_version: String,
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

/// Where to load configuration from
pub enum LoadPath<P> {
    /// Path specified explicitly, therefore, loading will fail if the file is not found
    Explicit(P),
    /// Using the default path, therefore, loading will not fail if the file is not found
    Default(P),
}

impl Config {
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

fn parse_api_version_label(raw: &str) -> Result<String, &'static str> {
    let trimmed = raw.trim();
    let trimmed = trimmed.strip_prefix('v').unwrap_or(trimmed);
    let mut parts = trimmed.split('.');
    let major = parts
        .next()
        .ok_or("missing major")?
        .parse::<u16>()
        .map_err(|_| "invalid major")?;
    let minor = parts
        .next()
        .unwrap_or("0")
        .parse::<u16>()
        .map_err(|_| "invalid minor")?;
    Ok(format!("{major}.{minor}"))
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, io::Write};

    use assertables::assert_contains;
    use iroha_config_base::env::MockEnv;
    use iroha_crypto::ExposedPrivateKey;

    use super::*;

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
            torii_url = "http://127.0.0.1:8080/"

            [basic_auth]
            web_login = "mad_hatter"
            password = "ilovetea"

            [account]
            domain = "wonderland"
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
            config.account.signatory().to_string(),
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
        );
    }

    #[test]
    fn full_env_fallback() {
        let key = KeyPair::random();
        let env = MockEnv::new()
            .set("CHAIN", "wonder")
            .set("TORII_URL", "http://localhost:8080")
            .set("TORII_API_VERSION", DEFAULT_TORII_API_VERSION)
            .set("ACCOUNT_DOMAIN", "land")
            .set(
                "ACCOUNT_PRIVATE_KEY",
                ExposedPrivateKey(key.private_key().clone()).to_string(),
            )
            .set("ACCOUNT_PUBLIC_KEY", key.public_key().to_string());

        let _config =
            Config::load_with_env(LoadPath::Default("non_existing_path"), env.clone()).unwrap();

        assert_eq!(env.unvisited(), HashSet::new());
        assert_eq!(env.unknown(), HashSet::new());
    }
}
