//! Streaming helpers (HPKE fingerprinters, suite listings).

use clap::{Args, Subcommand};
use eyre::{Result, WrapErr, eyre};
use iroha_crypto::streaming::STREAMING_DEFAULT_KEM_SUITE;
use iroha_crypto::streaming::kyber_public_fingerprint_with_suite;
use soranet_pq::{MlKemSuite, SuiteParseError};
use std::fmt::Write as _;

use crate::{Run, RunContext};
use crate::cli_output::print_with_optional_text;
use crate::json_macros::JsonSerialize;

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

#[derive(Debug, JsonSerialize)]
struct FingerprintOutput {
    suite: String,
    fingerprint_hex: String,
}

#[derive(Debug, JsonSerialize)]
struct SuitesOutput {
    default_suite: String,
    suites: Vec<String>,
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::Fingerprint(args) => args.run(context),
            Command::Suites => {
                let mut rows: Vec<String> =
                    MlKemSuite::ALL.iter().map(ToString::to_string).collect();
                rows.sort();
                let output = SuitesOutput {
                    default_suite: STREAMING_DEFAULT_KEM_SUITE.to_string(),
                    suites: rows.clone(),
                };
                let text = render_suites_text(&rows);
                print_with_optional_text(context, Some(text), &output)
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

        let output = FingerprintOutput {
            suite: suite.to_string(),
            fingerprint_hex: format!("0x{}", hex::encode(fingerprint)),
        };
        let text = format!(
            "suite={} fingerprint={}",
            output.suite, output.fingerprint_hex
        );
        print_with_optional_text(context, Some(text), &output)
    }
}

fn parse_suite(raw: &str) -> Result<MlKemSuite> {
    raw.parse::<MlKemSuite>()
        .map_err(|SuiteParseError(value)| eyre!("unsupported ML-KEM suite '{value}'"))
}

fn render_suites_text(rows: &[String]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Supported ML-KEM suites:");
    for label in rows {
        let _ = writeln!(out, "  - {label}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_i18n::{Bundle, Language, Localizer};
    use soranet_pq::generate_mlkem_keypair;
    use std::fmt::Display;

    struct TestContext {
        output_format: crate::CliOutputFormat,
        printed: Vec<String>,
        config: iroha::config::Config,
        i18n: Localizer,
    }

    impl TestContext {
        fn new(output_format: crate::CliOutputFormat) -> Self {
            Self {
                output_format,
                printed: Vec::new(),
                config: crate::fallback_config(),
                i18n: Localizer::new(Bundle::Cli, Language::English),
            }
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &iroha::config::Config {
            &self.config
        }

        fn transaction_metadata(&self) -> Option<&iroha::data_model::metadata::Metadata> {
            None
        }

        fn input_instructions(&self) -> bool {
            false
        }

        fn output_instructions(&self) -> bool {
            false
        }

        fn i18n(&self) -> &Localizer {
            &self.i18n
        }

        fn output_format(&self) -> crate::CliOutputFormat {
            self.output_format
        }

        fn print_data<T>(&mut self, data: &T) -> eyre::Result<()>
        where
            T: norito::json::JsonSerialize + ?Sized,
        {
            let rendered = norito::json::to_json_pretty(data)?;
            self.printed.push(rendered);
            Ok(())
        }

        fn println(&mut self, data: impl Display) -> eyre::Result<()> {
            self.printed.push(data.to_string());
            Ok(())
        }
    }

    #[test]
    fn suites_run_emits_json_in_json_mode() {
        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);
        Command::Suites.run(&mut ctx).expect("run suites");
        assert_eq!(ctx.printed.len(), 1);
        let value: norito::json::Value =
            norito::json::from_str(&ctx.printed[0]).expect("json output");
        let obj = value.as_object().expect("object");
        assert_eq!(
            obj.get("default_suite").and_then(|v| v.as_str()),
            Some(STREAMING_DEFAULT_KEM_SUITE.to_string().as_str())
        );
    }

    #[test]
    fn fingerprint_run_emits_json_with_hex() {
        let mut ctx = TestContext::new(crate::CliOutputFormat::Json);
        let suite = STREAMING_DEFAULT_KEM_SUITE;
        let keypair = generate_mlkem_keypair(suite);
        let args = FingerprintArgs {
            suite: Some(suite.to_string()),
            public_key_hex: hex::encode(keypair.public_key()),
        };
        args.run(&mut ctx).expect("run fingerprint");
        assert_eq!(ctx.printed.len(), 1);
        let value: norito::json::Value =
            norito::json::from_str(&ctx.printed[0]).expect("json output");
        let obj = value.as_object().expect("object");
        let fingerprint = obj
            .get("fingerprint_hex")
            .and_then(|v| v.as_str())
            .expect("fingerprint");
        assert!(fingerprint.starts_with("0x"));
        assert_eq!(fingerprint.len(), 66);
    }

    #[test]
    fn render_suites_text_includes_header() {
        let text = render_suites_text(&vec!["mlkem512".into()]);
        assert!(text.starts_with("Supported ML-KEM suites:"));
        assert!(text.contains("- mlkem512"));
    }
}
