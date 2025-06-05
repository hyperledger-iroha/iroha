//! User configuration view.

use error_stack::{Report, ResultExt};
use iroha_config_base::{
    attach::ConfigValueAndOrigin,
    util::{DurationMs, Emitter, EmitterResultExt},
    ReadConfig, WithOrigin,
};
use url::Url;

use crate::{
    config::BasicAuth,
    crypto::{KeyPair, PrivateKey, PublicKey},
    data_model::prelude::{AccountId, ChainId, DomainId},
};

/// Root of the user configuration
#[derive(Clone, Debug, ReadConfig)]
#[allow(missing_docs)]
pub struct Root {
    #[config(env = "CHAIN")]
    pub chain: ChainId,
    #[config(env = "TORII_URL")]
    pub torii_url: WithOrigin<Url>,
    #[config(env = "BASIC_AUTH")]
    pub basic_auth: Option<BasicAuth>,
    #[config(nested)]
    pub account: Account,
    #[config(nested)]
    pub transaction: Transaction,
}

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("Transaction status timeout should be smaller than its time-to-live")]
    TxTimeoutVsTtl,
    #[error("Failed to construct a key pair from provided public and private keys")]
    KeyPair,
    #[error("Unsupported URL scheme: `{scheme}`")]
    UnsupportedUrlScheme { scheme: String },
}

impl Root {
    /// Validates user configuration for semantic errors and constructs a complete
    /// [`super::Config`].
    ///
    /// # Errors
    /// If a set of validity errors occurs.
    pub fn parse(self) -> error_stack::Result<super::Config, ParseError> {
        let Self {
            chain: chain_id,
            torii_url,
            basic_auth,
            account:
                Account {
                    domain: domain_id,
                    public_key,
                    private_key,
                },
            transaction:
                Transaction {
                    time_to_live_ms: tx_ttl,
                    status_timeout_ms: tx_timeout,
                    nonce: tx_add_nonce,
                },
        } = self;

        let mut emitter = Emitter::new();

        // TODO: validate if TTL is too small?

        if tx_timeout.value() > tx_ttl.value() {
            emitter.emit(
                Report::new(ParseError::TxTimeoutVsTtl)
                    .attach_printable(tx_timeout.clone().into_attachment())
                    .attach_printable(tx_ttl.clone().into_attachment())
                    // FIXME: is this correct?
                    .attach_printable("Note: it doesn't make sense to set the timeout longer than the possible transaction lifetime"),
            )
        }

        match torii_url.value().scheme() {
            "http" | "https" => {}
            scheme => emitter.emit(
                Report::new(ParseError::UnsupportedUrlScheme {
                    scheme: scheme.to_string(),
                })
                .attach_printable(torii_url.clone().into_attachment())
                .attach_printable("Note: only `http` and `https` protocols are supported"),
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

        let (public_key, public_key_origin) = public_key.into_tuple();
        let (private_key, private_key_origin) = private_key.into_tuple();
        let account_id = AccountId::new(domain_id, public_key.clone());
        let key_pair = KeyPair::new(public_key, private_key)
            .attach_printable(ConfigValueAndOrigin::new("[REDACTED]", public_key_origin))
            .attach_printable(ConfigValueAndOrigin::new("[REDACTED]", private_key_origin))
            .change_context(ParseError::KeyPair)
            .ok_or_emit(&mut emitter);

        emitter.into_result()?;

        Ok(super::Config {
            chain: chain_id,
            account: account_id,
            key_pair: key_pair.unwrap(),
            torii_api_url,
            basic_auth,
            transaction_ttl: tx_ttl.into_value().get(),
            transaction_status_timeout: tx_timeout.into_value().get(),
            transaction_add_nonce: tx_add_nonce,
        })
    }
}

#[derive(Debug, Clone, ReadConfig)]
#[allow(missing_docs)]
pub struct Account {
    #[config(env = "ACCOUNT_DOMAIN")]
    pub domain: DomainId,
    #[config(env = "ACCOUNT_PUBLIC_KEY")]
    pub public_key: WithOrigin<PublicKey>,
    #[config(env = "ACCOUNT_PRIVATE_KEY")]
    pub private_key: WithOrigin<PrivateKey>,
}

#[derive(Debug, Clone, ReadConfig)]
#[allow(missing_docs)]
pub struct Transaction {
    #[config(
        env = "TRANSACTION_TIME_TO_LIVE_MS",
        default = "super::DEFAULT_TRANSACTION_TIME_TO_LIVE.into()"
    )]
    pub time_to_live_ms: WithOrigin<DurationMs>,
    #[config(
        env = "TRANSACTION_STATUS_TIMEOUT_MS",
        default = "super::DEFAULT_TRANSACTION_STATUS_TIMEOUT.into()"
    )]
    pub status_timeout_ms: WithOrigin<DurationMs>,
    #[config(
        env = "TRANSACTION_NONCE",
        default = "super::DEFAULT_TRANSACTION_NONCE"
    )]
    pub nonce: bool,
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use iroha_config_base::{env::MockEnv, read::ConfigReader};

    use super::*;

    #[test]
    fn parses_all_envs() {
        let env = MockEnv::from([
            ("CHAIN", "00000000-0000-0000-0000-000000000000"),
            ("TORII_URL", "http://localhost:8080"),
            ("BASIC_AUTH", "mad_hatter:ilovetea"),
            ("ACCOUNT_DOMAIN", "wonderland"),
            (
                "ACCOUNT_PUBLIC_KEY",
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
            ),
            (
                "ACCOUNT_PRIVATE_KEY",
                "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53",
            ),
            ("TRANSACTION_TIME_TO_LIVE_MS", "100_000"),
            ("TRANSACTION_STATUS_TIMEOUT_MS", "15_000"),
            ("TRANSACTION_NONCE", "false"),
        ]);

        let _root = ConfigReader::new()
            .with_env(env.clone())
            .read_and_complete::<Root>()
            .expect("should be able to be configured only from env vars");

        assert_eq!(env.unvisited(), HashSet::new());
        assert_eq!(env.unknown(), HashSet::new());
    }
}
