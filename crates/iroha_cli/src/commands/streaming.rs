//! Streaming helpers (HPKE fingerprinters, suite listings).

use clap::{Args, Subcommand};
use eyre::{Result, WrapErr, eyre};
use iroha_crypto::streaming::STREAMING_DEFAULT_KEM_SUITE;
use iroha_crypto::streaming::kyber_public_fingerprint_with_suite;
use soranet_pq::{MlKemSuite, SuiteParseError};

use crate::Run;
use crate::RunContext;

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Compute the ML-KEM fingerprint advertised in `EncryptionSuite::Kyber*`.
    Fingerprint(FingerprintArgs),
    /// List supported ML-KEM suite identifiers.
    Suites,
}

#[derive(Args, Debug)]
pub struct FingerprintArgs {
    /// ML-KEM suite to use (e.g., `mlkem512`, `mlkem768`, `mlkem1024`).
    #[arg(long = "suite", value_name = "NAME")]
    pub suite: Option<String>,
    /// Hex-encoded ML-KEM public key.
    #[arg(long = "public-key", value_name = "HEX")]
    pub public_key_hex: String,
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Fingerprint(args) => args.run(context),
            Command::Suites => {
                let mut rows: Vec<String> =
                    MlKemSuite::ALL.iter().map(ToString::to_string).collect();
                rows.sort();
                context.println("Supported ML-KEM suites:")?;
                for label in rows {
                    context.println(format!("  - {label}"))?;
                }
                Ok(())
            }
        }
    }
}

impl FingerprintArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let suite = match self.suite {
            Some(label) => parse_suite(&label)?,
            None => STREAMING_DEFAULT_KEM_SUITE,
        };

        let public_key =
            hex::decode(&self.public_key_hex).wrap_err("failed to decode Kyber public key hex")?;

        let fingerprint =
            kyber_public_fingerprint_with_suite(&public_key, suite).map_err(|err| {
                eyre!(
                    "fingerprint derivation failed for suite {}: {err}",
                    suite.to_string()
                )
            })?;

        context.println(format!(
            "suite={} fingerprint=0x{}",
            suite,
            hex::encode(fingerprint)
        ))?;
        Ok(())
    }
}

fn parse_suite(raw: &str) -> Result<MlKemSuite> {
    raw.parse::<MlKemSuite>()
        .map_err(|SuiteParseError(value)| eyre!("unsupported ML-KEM suite '{value}'"))
}
