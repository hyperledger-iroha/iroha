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
use std::{
    fmt,
    io::{BufWriter, Write, stdout},
    process::ExitCode,
};
use clap::{Args as ClapArgs, Parser, Subcommand};
use color_eyre::eyre::WrapErr as _;
use iroha_genesis::init_instruction_registry;
mod client_configs;
mod codec;
mod crypto;
mod genesis;
mod kagemusha;
mod kura;
/// Helpers for generating a multi-peer localnet (configs, scripts, genesis).
pub mod localnet;
mod localnet_tui;
mod privacy_bootstrap;
mod schema;
mod secure_fs;
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
/// Build-time source identity embedded for release artifact validation.
const BUILD_SOURCE_ID: Option<&str> = option_env!("IROHA_GIT_COMMIT_HASH");
const TOP_LEVEL_HELP: &str = concat!(
    "Common tasks:\n",
    "  kagami localnet-wizard\n",
    "  kagami wizard --profile nexus\n",
    "  kagami localnet --out-dir ./localnet\n",
    "  kagami docker --peers 4 --config-dir ./localnet --image hyperledger/iroha:dev --out-file docker-compose.yml\n",
    "  kagami keys --out-dir ./key-custody\n",
    "  kagami keys --algorithm bls_normal --pop --json\n",
    "  kagami advanced markdown-help\n",
);
/// Error requesting one deliberate non-default CLI exit status.
///
/// Security-sensitive publication commands use this after the publication
/// point when durability cannot be established. Keeping the status typed lets
/// the top-level command preserve the ordinary error path without collapsing a
/// commit-uncertain result into a retryable exit code.
#[derive(Debug)]
pub(crate) struct ExplicitExitError {
    code: u8,
    message: String,
}
impl ExplicitExitError {
    /// Construct one explicit CLI exit request with its operator-facing error.
    pub(crate) fn new(code: u8, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    const fn code(&self) -> u8 {
        self.code
    }
}
impl fmt::Display for ExplicitExitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for ExplicitExitError {}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if let Some(explicit) = error.downcast_ref::<ExplicitExitError>() {
                // Explicit security outcomes are machine records. Do not wrap
                // them in color-eyre's Debug report, which would make the
                // documented status line unstable for operators and tooling.
                eprintln!("{explicit}");
                ExitCode::from(explicit.code())
            } else {
                eprintln!("{error:?}");
                ExitCode::FAILURE
            }
        }
    }
}
fn run() -> Outcome {
    let _ = std::hint::black_box(BUILD_SOURCE_ID);
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
    /// Generate validator-only Docker Compose from a prepared bundle or explicit dev seed
    Docker(swarm::Args),
    /// Generate cryptographic key pairs and optional validator Proofs-of-Possession
    Keys(Box<crypto::Args>),
    /// Commands related to genesis
    #[clap(subcommand)]
    Genesis(genesis::Args),
    /// Verify and promote authenticated Kagemusha ABI-21/V4 artifact releases
    Kagemusha(kagemusha::Args),
    /// Emit and validate fail-closed Taira exact-12 privacy bootstrap artifacts
    PrivacyBootstrap(privacy_bootstrap::Args),
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
            Kagemusha(args) => args.run(writer),
            PrivacyBootstrap(args) => args.run(writer),
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
    fn privacy_bootstrap_commands_require_complete_exact_v1_artifact_paths() {
        assert!(
            parse(
                "kagami privacy-bootstrap emit-taira-v1 \
                 --instructions-output ./privacy-instructions.json \
                 --report-output ./privacy-report.json"
            )
            .is_ok()
        );
        assert!(
            parse(
                "kagami privacy-bootstrap validate-taira-v1 \
                 --instructions ./privacy-instructions.json \
                 --report ./privacy-report.json"
            )
            .is_ok()
        );
        assert!(
            parse(
                "kagami privacy-bootstrap render-taira-release-v1 \
                 --activation-instructions ./privacy-instructions.json \
                 --activation-report ./privacy-report.json \
                 --broker-public-export ./broker-public.json \
                 --plan-template ./privacy-plan.staging.json \
                 --config-template ./config.staging.toml \
                 --genesis-template ./genesis.staging.json \
                 --plan-output ./privacy-plan.release.json \
                 --config-output ./config.release.toml \
                 --genesis-output ./genesis.release.json \
                 --broker-public-output ./broker-public.verified.json"
            )
            .is_ok()
        );
        assert!(
            parse(
                "kagami privacy-bootstrap render-taira-release-v1 \
                 --activation-instructions ./privacy-instructions.json \
                 --activation-report ./privacy-report.json \
                 --broker-public-export ./broker-public.json"
            )
            .is_err(),
            "release composition without every staging input and fresh output must fail"
        );
        assert!(
            parse(
                "kagami privacy-bootstrap emit-taira-v1 \
                 --instructions-output ./privacy-instructions.json"
            )
            .is_err(),
            "emission without its digest inventory must remain unavailable"
        );
        assert!(
            parse("kagami privacy-bootstrap emit --output ./legacy.json").is_err(),
            "the first release has no compatibility command aliases"
        );
    }
    #[test]
    fn retired_kagemusha_verify_release_command_is_rejected() {
        assert!(
            parse("kagami kagemusha verify-release --bundle-dir ./release").is_err(),
            "the pre-V4 release verifier must remain unavailable"
        );
        assert!(
            parse(
                "kagami kagemusha verify-release \
                 --bundle-dir ./release \
                 --benchmark-evidence ./benchmark.evidence \
                 --cryptographic-review ./review.evidence"
            )
            .is_err(),
            "complete evidence must not revive the retired verifier"
        );
        assert!(
            parse(
                "kagami kagemusha verify-release \
                 --bundle-dir ./release \
                 --benchmark-evidence ./benchmark.evidence"
            )
            .is_err()
        );
        assert!(
            parse(
                "kagami kagemusha verify-release \
                 --bundle-dir ./release \
                 --cryptographic-review ./review.evidence"
            )
            .is_err()
        );
    }
    #[test]
    fn kagemusha_v4_commands_are_distinct_and_require_complete_evidence() {
        assert!(
            parse("kagami kagemusha verify-release-v4 --bundle-dir ./release-v4").is_err(),
            "ABI-21 verification must hash-check both independent evidence files"
        );
        assert!(
            parse(
                "kagami kagemusha verify-release-v4 \
                 --bundle-dir ./release-v4 \
                 --release-policy ./release-policy.norito \
                 --benchmark-evidence ./benchmark.evidence \
                 --cryptographic-review ./review.evidence"
            )
            .is_ok()
        );
        assert!(
            parse(
                "kagami kagemusha promote-release-v4 \
                 --bundle-dir ./release-v4 \
                 --release-policy ./release-policy.norito \
                 --promotion-record ./promoted-v4.norito \
                 --benchmark-evidence ./benchmark.evidence \
                 --cryptographic-review ./review.evidence"
            )
            .is_ok()
        );
        assert!(
            parse(
                "kagami kagemusha promote-release-v4 \
                 --bundle-dir ./release-v4 \
                 --release-policy ./release-policy.norito \
                 --promotion-record ./promoted-v4.norito \
                 --benchmark-evidence ./benchmark.evidence"
            )
            .is_err()
        );
        assert!(
            parse(
                "kagami kagemusha verify-release-v4 \
                 --bundle-dir ./release-v4 \
                 --benchmark-evidence ./benchmark.evidence \
                 --cryptographic-review ./review.evidence"
            )
            .is_err(),
            "ABI-21 verification must receive the canonical policy explicitly"
        );
        assert!(
            parse(
                "kagami kagemusha promote-release \
                 --bundle-dir ./release-v4 \
                 --promotion-record ./promoted-v4.norito \
                 --benchmark-evidence ./benchmark.evidence \
                 --cryptographic-review ./review.evidence \
                 --abi-version 20"
            )
            .is_err(),
            "the historical V3 command must not accept a V4 alias flag"
        );
    }
    #[test]
    fn kagemusha_taira_base_genesis_rejects_pre_genesis_release_inputs() {
        let base = "kagami kagemusha prepare-taira-testnet-base-genesis-v4 \
             --genesis ./genesis.json \
             --genesis-authority genesis-authority \
             --command-authority command-authority \
             --output ./base-genesis.json";
        assert!(parse(base).is_ok());
        assert!(
            parse(&format!("{base} --release-bundle ./release")).is_err(),
            "a NetworkId-bound release must not be accepted before genesis is signed"
        );
        assert!(
            parse(
                "kagami kagemusha prepare-taira-testnet-bootstrap-v4 \
                 --genesis ./genesis.json \
                 --release-bundle ./release \
                 --genesis-authority genesis-authority \
                 --command-authority command-authority \
                 --output ./genesis.json"
            )
            .is_err(),
            "the self-referential pre-genesis release command must remain retired"
        );
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
    fn keys_owner_only_output_is_a_distinct_format() {
        assert!(parse("kagami keys --out-dir ./custody").is_ok());
        assert!(parse("kagami keys --out-dir ./custody --compact").is_err());
        assert!(parse("kagami keys --out-dir ./custody --json").is_err());
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
        assert!(
            parse(
                "kagami advanced client-configs --base-config ./client.toml --names alice --seed-hex 1111111111111111111111111111111111111111111111111111111111111111"
            )
            .is_ok()
        );
        assert!(
            parse(
                "kagami advanced client-configs --base-config ./client.toml --names alice --seed-prefix demo"
            )
            .is_err()
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
