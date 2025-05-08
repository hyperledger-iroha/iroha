//! Module for client-related configuration and structs

use core::str::FromStr;
use std::{path::Path, time::Duration};

use derive_more::Display;
use error_stack::ResultExt;
use eyre::Result;
use iroha_config_base::{env::ReadEnv, read::ConfigReader, toml::TomlSource};
use iroha_primitives::small::SmallStr;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use url::Url;

use crate::{
    crypto::KeyPair,
    data_model::{prelude::*, ChainId},
};

mod user;

pub use user::Root as UserConfig;

use crate::secrecy::SecretString;

#[allow(missing_docs)]
pub const DEFAULT_TRANSACTION_TIME_TO_LIVE: Duration = Duration::from_secs(100);
#[allow(missing_docs)]
pub const DEFAULT_TRANSACTION_STATUS_TIMEOUT: Duration = Duration::from_secs(15);
#[allow(missing_docs)]
pub const DEFAULT_TRANSACTION_NONCE: bool = false;

/// Valid web auth login string. See [`WebLogin::from_str`]
#[derive(Debug, Display, Clone, PartialEq, Eq, DeserializeFromStr, SerializeDisplay)]
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

/// Basic Authentication credentials
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct BasicAuth {
    /// Login for Basic Authentication
    pub web_login: WebLogin,
    /// Password for Basic Authentication
    pub password: SecretString,
}

/// Complete client configuration
#[derive(Clone, Debug, Serialize)]
#[allow(missing_docs)]
pub struct Config {
    pub chain: ChainId,
    pub account: AccountId,
    pub key_pair: KeyPair,
    pub basic_auth: Option<BasicAuth>,
    pub torii_api_url: Url,
    pub transaction_ttl: Duration,
    pub transaction_status_timeout: Duration,
    pub transaction_add_nonce: bool,
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
    pub fn load(path: LoadPath<impl AsRef<Path>>) -> error_stack::Result<Self, LoadError> {
        Self::load_with_env(path, Box::new(iroha_config_base::env::std_env))
    }

    fn load_with_env(
        path: LoadPath<impl AsRef<Path>>,
        env: impl ReadEnv + 'static,
    ) -> error_stack::Result<Self, LoadError> {
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
    use std::{collections::HashSet, io::Write};

    use assertables::{assert_contains, assert_contains_as_result};
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
        fn with_scheme(scheme: &str) -> error_stack::Result<Config, user::ParseError> {
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
