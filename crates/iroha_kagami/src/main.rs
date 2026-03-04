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
#[command(name = "kagami", version, author)]
struct Cli {
    #[command(flatten)]
    ui: tui::UiOptions,
    #[command(subcommand)]
    command: Command,
}

/// Kagami is a tool used to generate and validate automatically generated data files that are
/// shipped with Iroha.
#[derive(Debug, Subcommand)]
enum Command {
    /// Generate cryptographic key pairs using the given algorithm and either private key or seed
    Crypto(Box<crypto::Args>),
    /// Generate per-client CLI configs from a base client.toml
    ClientConfigs(client_configs::Args),
    /// Generate the schema used for code generation in Iroha SDKs
    Schema(schema::Args),
    /// Commands related to genesis
    #[clap(subcommand)]
    Genesis(genesis::Args),
    /// Commands related to Norito codec conversions
    Codec(codec::Args),
    /// Commands related to block inspection
    Kura(kura::Args),
    /// Generate a local (non-Docker) 4+ node setup: genesis, per-peer configs, start/stop scripts
    Localnet(localnet::Args),
    /// Interactive local network generator (prompts for peers/accounts/assets)
    LocalnetWizard(localnet_tui::LocalnetWizardArgs),
    /// Commands related to Docker Compose configuration generation
    Swarm(swarm::Args),
    /// Interactive setup wizard for node configs (keys + config + genesis)
    Wizard(wizard::Args),
    /// Verify a genesis manifest against a preset profile
    Verify(verify::Args),
    /// Output CLI documentation in Markdown format
    MarkdownHelp(MarkdownHelp),
}

impl<T: Write> RunArgs<T> for Command {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        use Command::*;

        match self {
            Crypto(args) => args.run(writer),
            ClientConfigs(args) => args.run(writer),
            Schema(args) => args.run(writer),
            Genesis(args) => args.run(writer),
            Codec(args) => args.run(writer),
            Kura(args) => args.run(writer),
            Localnet(args) => args.run(writer),
            LocalnetWizard(args) => args.run(writer),
            Swarm(args) => args.run(writer),
            Wizard(args) => args.run(writer),
            Verify(args) => args.run(writer),
            MarkdownHelp(args) => args.run(writer),
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
    fn ok_with_flags() {
        assert!(
            parse(
                "kagami swarm \
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
    fn cannot_mix_print_and_force() {
        assert!(
            parse(
                "kagami swarm \
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
    fn ok_when_pull_image() {
        assert!(
            parse(
                "kagami swarm \
            -p 20 \
            -c ./config \
            -i hyperledger/iroha \
            -o sample.yml",
            )
            .is_ok()
        )
    }

    #[test]
    fn ok_when_build_image() {
        assert!(
            parse(
                "kagami swarm \
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
    fn fails_when_image_is_omitted() {
        assert!(
            parse(
                "kagami swarm \
            -p 1 \
            -c ./ \
            -o test.yml",
            )
            .is_err()
        )
    }
}
