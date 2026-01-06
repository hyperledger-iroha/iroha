//! Confidential asset CLI helpers (wallet/offline tooling).

use std::{fs, path::PathBuf};

use base64::Engine as _;
use clap::Subcommand;
use eyre::{Context, Result};
use hex::encode as hex_encode;
use iroha_config::client_api::{
    ConfidentialGas as ConfidentialGasDTO, ConfigUpdateDTO, Logger as LoggerDTO,
};
use iroha_crypto::{ConfidentialKeyset, derive_keyset_from_slice};
use rand::random;
use zeroize::Zeroize;

use crate::{Run, RunContext};

/// Confidential CLI subcommands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Derive confidential key hierarchy (nk/ivk/ovk/fvk) from a spend key.
    CreateKeys(CreateKeysArgs),
    /// Inspect or update the confidential gas schedule.
    #[command(subcommand)]
    Gas(GasCommand),
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            Command::CreateKeys(args) => args.run(context),
            Command::Gas(cmd) => cmd.run(context),
        }
    }
}

/// Arguments for the `create-keys` command.
#[derive(clap::Args, Debug)]
pub struct CreateKeysArgs {
    /// 32-byte spend key in hex (if omitted, a random key is generated).
    #[arg(long, value_name = "HEX32")]
    seed_hex: Option<String>,
    /// Write the derived keyset JSON to a file.
    #[arg(long, value_name = "PATH")]
    output: Option<PathBuf>,
    /// Do not print the generated JSON to stdout.
    #[arg(long)]
    quiet: bool,
}

impl CreateKeysArgs {
    fn parse_seed(&self) -> Result<([u8; 32], bool)> {
        if let Some(seed_hex) = &self.seed_hex {
            let bytes = parse_hex_32(seed_hex)?;
            Ok((bytes, false))
        } else {
            let seed: [u8; 32] = random();
            Ok((seed, true))
        }
    }
}

impl Run for CreateKeysArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let (mut spend_key, random_generated) = self.parse_seed()?;
        let keyset = derive_keyset_from_slice(&spend_key).expect("validated length");

        let json = render_keyset_json(&keyset, random_generated);
        let json_string =
            norito::json::to_json_pretty(&json).map_err(|err| eyre::eyre!(err.to_string()))?;

        if let Some(path) = &self.output {
            fs::write(path, json_string.as_bytes())
                .with_context(|| format!("failed to write keyset JSON to {}", path.display()))?;
            context.println(format!(
                "Wrote confidential keyset JSON to {}",
                path.display()
            ))?;
        }

        if !self.quiet {
            context.print_data(&json)?;
        }

        spend_key.zeroize();

        Ok(())
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum GasCommand {
    /// Fetch the current confidential gas schedule.
    Get,
    /// Update the confidential gas schedule.
    Set(GasSetArgs),
}

impl Run for GasCommand {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        match self {
            GasCommand::Get => {
                let client = context.client_from_config();
                let schedule = client
                    .get_confidential_gas_schedule()
                    .wrap_err("failed to fetch confidential gas schedule")?;
                context.println(format!("proof_base: {}", schedule.proof_base))?;
                context.println(format!("per_public_input: {}", schedule.per_public_input))?;
                context.println(format!("per_proof_byte: {}", schedule.per_proof_byte))?;
                context.println(format!("per_nullifier: {}", schedule.per_nullifier))?;
                context.println(format!("per_commitment: {}", schedule.per_commitment))?;
                Ok(())
            }
            GasCommand::Set(args) => args.run(context),
        }
    }
}

#[derive(clap::Args, Debug)]
pub struct GasSetArgs {
    #[arg(long, value_name = "UNITS")]
    proof_base: u64,
    #[arg(long, value_name = "UNITS")]
    per_public_input: u64,
    #[arg(long, value_name = "UNITS")]
    per_proof_byte: u64,
    #[arg(long, value_name = "UNITS")]
    per_nullifier: u64,
    #[arg(long, value_name = "UNITS")]
    per_commitment: u64,
}

impl GasSetArgs {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = context.client_from_config();
        let config = client
            .get_config()
            .wrap_err("failed to fetch configuration for update")?;
        let update = ConfigUpdateDTO {
            logger: LoggerDTO {
                level: config.logger.level,
                filter: config.logger.filter.clone(),
            },
            network_acl: None,
            network: None,
            confidential_gas: Some(ConfidentialGasDTO {
                proof_base: self.proof_base,
                per_public_input: self.per_public_input,
                per_proof_byte: self.per_proof_byte,
                per_nullifier: self.per_nullifier,
                per_commitment: self.per_commitment,
            }),
            soranet_handshake: None,
            transport: None,
            compute_pricing: None,
        };
        client
            .set_config(&update)
            .wrap_err("failed to submit confidential gas update")?;
        context.println("Confidential gas schedule updated.")?;
        Ok(())
    }
}

fn render_keyset_json(keyset: &ConfidentialKeyset, random_generated: bool) -> norito::json::Value {
    let encoder = base64::engine::general_purpose::STANDARD;

    let sk_hex = hex_encode(keyset.spend_key());
    let nk_hex = hex_encode(keyset.nullifier_key());
    let ivk_hex = hex_encode(keyset.incoming_view_key());
    let ovk_hex = hex_encode(keyset.outgoing_view_key());
    let fvk_hex = hex_encode(keyset.full_view_key());

    let sk_b64 = encoder.encode(keyset.spend_key());
    let nk_b64 = encoder.encode(keyset.nullifier_key());
    let ivk_b64 = encoder.encode(keyset.incoming_view_key());
    let ovk_b64 = encoder.encode(keyset.outgoing_view_key());
    let fvk_b64 = encoder.encode(keyset.full_view_key());

    norito::json::object([
        (
            "generated_random",
            norito::json::Value::from(random_generated),
        ),
        ("spend_key_hex", norito::json::Value::from(sk_hex)),
        ("spend_key_b64", norito::json::Value::from(sk_b64)),
        ("nullifier_key_hex", norito::json::Value::from(nk_hex)),
        ("nullifier_key_b64", norito::json::Value::from(nk_b64)),
        ("incoming_view_key_hex", norito::json::Value::from(ivk_hex)),
        ("incoming_view_key_b64", norito::json::Value::from(ivk_b64)),
        ("outgoing_view_key_hex", norito::json::Value::from(ovk_hex)),
        ("outgoing_view_key_b64", norito::json::Value::from(ovk_b64)),
        ("full_view_key_hex", norito::json::Value::from(fvk_hex)),
        ("full_view_key_b64", norito::json::Value::from(fvk_b64)),
    ])
    .expect("static entries")
}

fn parse_hex_32(hex_str: &str) -> Result<[u8; 32]> {
    let trimmed = hex_str.trim();
    let without_prefix = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    let bytes =
        hex::decode(without_prefix).with_context(|| format!("invalid hex string: {hex_str}"))?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| eyre::eyre!("expected 32 bytes, got {}", without_prefix.len() / 2))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Display;

    use iroha::{
        config::Config,
        crypto::{Algorithm, KeyPair},
        data_model::{metadata::Metadata, prelude::*},
    };
    use iroha_i18n::{Bundle, Language, Localizer};
    use norito::json::{self, JsonSerialize};
    use url::Url;

    #[test]
    fn parse_hex_32_rejects_invalid_length() {
        let err = parse_hex_32("deadbeef").expect_err("should fail");
        assert!(format!("{err}").contains("expected 32 bytes"));
    }

    #[test]
    fn render_keyset_json_contains_expected_fields() {
        let keyset = derive_keyset_from_slice(&[0xAA; 32]).expect("length ok");
        let json = render_keyset_json(&keyset, false);
        let value = json::to_value(&json).expect("serialize");
        let object = value.as_object().expect("object");
        assert!(object.contains_key("spend_key_hex"));
        assert!(object.contains_key("nullifier_key_b64"));
    }

    struct TestContext {
        cfg: Config,
        printed: Vec<norito::json::Value>,
        lines: Vec<String>,
        i18n: Localizer,
    }

    impl TestContext {
        fn new() -> Self {
            let key_pair = KeyPair::from_seed(vec![0u8; 32], Algorithm::Ed25519);
            let account_id =
                AccountId::new("wonderland".parse().unwrap(), key_pair.public_key().clone());
            let cfg = Config {
                chain: ChainId::from("00000000-0000-0000-0000-000000000000"),
                account: account_id,
                key_pair,
                basic_auth: None,
                torii_api_url: Url::parse("http://127.0.0.1/").unwrap(),
                torii_api_version: iroha::config::default_torii_api_version(),
                torii_api_min_proof_version: iroha::config::DEFAULT_TORII_API_MIN_PROOF_VERSION
                    .to_string(),
                torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
                transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
                transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
                transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
                connect_queue_root: iroha::config::default_connect_queue_root(),
                sorafs_alias_cache: crate::config_utils::default_alias_cache_policy(),
                sorafs_anonymity_policy: crate::config_utils::default_anonymity_policy(),
                sorafs_rollout_phase: crate::config_utils::default_rollout_phase(),
            };
            Self {
                cfg,
                printed: Vec::new(),
                lines: Vec::new(),
                i18n: Localizer::new(Bundle::Cli, Language::English),
            }
        }
    }

    impl RunContext for TestContext {
        fn config(&self) -> &Config {
            &self.cfg
        }

        fn transaction_metadata(&self) -> Option<&Metadata> {
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

        fn print_data<T>(&mut self, data: &T) -> Result<()>
        where
            T: JsonSerialize + ?Sized,
        {
            self.printed.push(norito::json::to_value(data)?);
            Ok(())
        }

        fn println(&mut self, data: impl Display) -> Result<()> {
            self.lines.push(data.to_string());
            Ok(())
        }
    }

    #[test]
    fn create_keys_with_seed_outputs_expected_hex() {
        let seed_hex = "11".repeat(32);
        let args = CreateKeysArgs {
            seed_hex: Some(seed_hex.clone()),
            output: None,
            quiet: false,
        };
        let mut ctx = TestContext::new();
        args.run(&mut ctx).expect("run command");
        assert_eq!(ctx.printed.len(), 1);
        let value = ctx.printed.pop().unwrap();
        let object = value.as_object().expect("object");
        assert_eq!(
            object
                .get("spend_key_hex")
                .and_then(|v| v.as_str())
                .expect("hex"),
            seed_hex
        );
        assert_eq!(
            object
                .get("spend_key_b64")
                .and_then(|v| v.as_str())
                .map(|s| base64::engine::general_purpose::STANDARD.decode(s).unwrap())
                .unwrap(),
            hex::decode(seed_hex).unwrap()
        );
    }
}
