//! CLI for generating Iroha sample configuration, genesis,
//! cryptographic key pairs and other. To be used with all compliant Iroha
//! installations.
#![allow(
    clippy::doc_markdown,
    clippy::uninlined_format_args,
    clippy::items_after_test_module,
    clippy::struct_excessive_bools,
    clippy::cast_possible_truncation,
    clippy::match_same_arms,
    clippy::unnecessary_debug_formatting
)]
use std::io::{BufWriter, Write, stdout};

use clap::{Args as ClapArgs, Parser, Subcommand};
use color_eyre::eyre::WrapErr as _;
use iroha_genesis::init_instruction_registry;

mod client_configs;
mod codec;
mod crypto;
mod genesis;
mod kura;
/// Helpers for generating a multi-peer localnet (configs, scripts, genesis).
pub mod localnet;
mod localnet_tui;
mod schema;
mod swarm;
mod tui;
mod verify;
mod wizard;

/// Norito JSON derive macros available to Kagami modules.
pub mod json_macros {
    pub use norito::derive::{FastJson, FastJsonWrite, JsonDeserialize, JsonSerialize};
}

/// Outcome shorthand used throughout this crate
// Note: migrate Kagami CLI to `error_stack` once modules no longer depend on `color_eyre` convenience macros.
pub(crate) type Outcome = color_eyre::Result<()>;

const TOP_LEVEL_HELP: &str = concat!(
    "Common tasks:\n",
    "  kagami localnet-wizard\n",
    "  kagami wizard --profile nexus\n",
    "  kagami localnet --out-dir ./localnet\n",
    "  kagami docker --peers 4 --config-dir ./localnet --image hyperledger/iroha:dev --out-file docker-compose.yml\n",
    "  kagami keys --algorithm bls_normal --pop --json\n",
    "  kagami advanced markdown-help\n",
);

fn main() -> Outcome {
    color_eyre::install()?;
    init_instruction_registry();
    let cli = Cli::parse();
    cli.ui.install();
    let mut writer = BufWriter::new(stdout());
    cli.command.run(&mut writer)
}

/// Trait to encapsulate common attributes of the commands and sub-commands.
trait RunArgs<T: Write> {
    /// Run the given command.
    ///
    /// # Errors
    /// if inner command fails.
    fn run(self, writer: &mut BufWriter<T>) -> Outcome;
}

#[derive(Parser, Debug)]
#[command(
    name = "kagami",
    version,
    author,
    about = "Task-first Iroha operator tooling for guided setup, local devnets, genesis work, and diagnostics.",
    after_help = TOP_LEVEL_HELP
)]
struct Cli {
    #[command(flatten)]
    ui: tui::UiOptions,
    #[command(subcommand)]
    command: Command,
}

/// Kagami is a task-first Iroha operator toolbox with guided flows for node setup and local
/// devnets, plus advanced low-level helpers.
#[derive(Debug, Subcommand)]
enum Command {
    /// Guided node/bootstrap flow for configuring a peer against an existing network profile
    Wizard(wizard::Args),
    /// Guided disposable local devnet flow for generating peers, configs, genesis, and scripts
    LocalnetWizard(localnet_tui::LocalnetWizardArgs),
    /// Generate a bare-metal local network: genesis, per-peer configs, client config, and scripts
    Localnet(localnet::Args),
    /// Generate Docker Compose deployment manifests from an existing config/genesis directory
    Docker(swarm::Args),
    /// Generate cryptographic key pairs and optional validator Proofs-of-Possession
    Keys(Box<crypto::Args>),
    /// Commands related to genesis
    #[clap(subcommand)]
    Genesis(genesis::Args),
    /// Verify a genesis manifest against a preset profile
    Verify(verify::Args),
    /// Advanced low-level helpers for codec conversion, schema generation, block inspection, and docs
    #[clap(subcommand)]
    Advanced(AdvancedCommand),
}

#[derive(Debug, Subcommand)]
enum AdvancedCommand {
    /// Generate per-client CLI configs from a base client.toml
    #[command(name = "client-configs")]
    ClientConfigs(client_configs::Args),
    /// Commands related to Norito codec conversions
    Codec(codec::Args),
    /// Commands related to block inspection
    Kura(kura::Args),
    /// Output CLI documentation in Markdown format
    MarkdownHelp(MarkdownHelp),
    /// Generate the schema used for code generation in Iroha SDKs
    Schema(schema::Args),
}

impl<T: Write> RunArgs<T> for AdvancedCommand {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        match self {
            Self::ClientConfigs(args) => args.run(writer),
            Self::Codec(args) => args.run(writer),
            Self::Kura(args) => args.run(writer),
            Self::MarkdownHelp(args) => args.run(writer),
            Self::Schema(args) => args.run(writer),
        }
    }
}

impl<T: Write> RunArgs<T> for Command {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        use Command::*;

        match self {
            Wizard(args) => args.run(writer),
            LocalnetWizard(args) => args.run(writer),
            Localnet(args) => args.run(writer),
            Docker(args) => args.run(writer),
            Keys(args) => args.run(writer),
            Genesis(args) => args.run(writer),
            Verify(args) => args.run(writer),
            Advanced(args) => args.run(writer),
        }
    }
}

#[derive(Debug, ClapArgs, Clone)]
struct MarkdownHelp;

impl<T: Write> RunArgs<T> for MarkdownHelp {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let command_info = clap_markdown::help_markdown::<Cli>();
        writer.write_all(command_info.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Error};

    use super::{Cli, Parser};

    fn parse(args: &str) -> Result<Cli, Error> {
        Cli::try_parse_from(args.split(' '))
    }

    #[test]
    fn verify_args() {
        Cli::command().debug_assert();
    }

    #[test]
    fn docker_accepts_valid_flags() {
        assert!(
            parse(
                "kagami docker \
            -p 20 \
            -c ./config \
            -i hyperledger/iroha \
            -o sample.yml\
            -HF",
            )
            .is_ok()
        )
    }

    #[test]
    fn docker_cannot_mix_print_and_force() {
        assert!(
            parse(
                "kagami docker \
            -p 20 \
            -c ./config \
            -i hyperledger/iroha \
            -o sample.yml \
            -PF",
            )
            .is_err()
        )
    }

    #[test]
    fn docker_accepts_pull_image_mode() {
        assert!(
            parse(
                "kagami docker \
            -p 20 \
            -c ./config \
            -i hyperledger/iroha \
            -o sample.yml",
            )
            .is_ok()
        )
    }

    #[test]
    fn docker_accepts_build_mode() {
        assert!(
            parse(
                "kagami docker \
            -p 20 \
            -i hyperledger/iroha \
            -b . \
            -c ./config \
            -o sample.yml",
            )
            .is_ok()
        )
    }

    #[test]
    fn docker_requires_image() {
        assert!(
            parse(
                "kagami docker \
            -p 1 \
            -c ./ \
            -o test.yml",
            )
            .is_err()
        )
    }

    #[test]
    fn advanced_subcommands_parse() {
        assert!(parse("kagami advanced codec list-types").is_ok());
        assert!(parse("kagami advanced schema").is_ok());
        assert!(parse("kagami advanced kura ./store print").is_ok());
        assert!(parse("kagami advanced markdown-help").is_ok());
        assert!(
            parse("kagami advanced client-configs --base-config ./client.toml --names alice")
                .is_ok()
        );
    }

    #[test]
    fn removed_top_level_commands_fail() {
        assert!(parse("kagami crypto --algorithm ed25519").is_err());
        assert!(parse("kagami swarm -p 1 -c ./cfg -i hyperledger/iroha -o docker.yml").is_err());
        assert!(parse("kagami codec list-types").is_err());
        assert!(parse("kagami schema").is_err());
        assert!(parse("kagami kura ./store print").is_err());
        assert!(parse("kagami client-configs --base-config ./client.toml --names alice").is_err());
        assert!(parse("kagami markdown-help").is_err());
    }

    #[test]
    fn checked_in_markdown_help_matches_generated_help() {
        let generated = clap_markdown::help_markdown::<Cli>();
        let checked_in = include_str!("../CommandLineHelp.md");
        assert_eq!(generated, checked_in);
    }
}
