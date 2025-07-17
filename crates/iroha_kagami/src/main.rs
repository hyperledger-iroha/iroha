//! CLI for generating Iroha sample configuration, genesis,
//! cryptographic key pairs and other. To be used with all compliant Iroha
//! installations.
use std::io::{stdout, BufWriter, Write};

use clap::{Args as ClapArgs, Parser};
use color_eyre::eyre::WrapErr as _;
use iroha_data_model::prelude::*;

mod codec;
mod crypto;
mod genesis;
mod kura;
mod schema;
mod swarm;
mod wasm;

/// Outcome shorthand used throughout this crate
pub(crate) type Outcome = color_eyre::Result<()>;

fn main() -> Outcome {
    color_eyre::install()?;
    let args = Args::parse();
    let mut writer = BufWriter::new(stdout());
    args.run(&mut writer)
}

/// Trait to encapsulate common attributes of the commands and sub-commands.
trait RunArgs<T: Write> {
    /// Run the given command.
    ///
    /// # Errors
    /// if inner command fails.
    fn run(self, writer: &mut BufWriter<T>) -> Outcome;
}

/// Kagami is a tool used to generate and validate automatically generated data files that are
/// shipped with Iroha.
#[derive(Parser, Debug)]
#[command(name = "kagami", version, author)]
enum Args {
    /// Generate cryptographic key pairs using the given algorithm and either private key or seed
    Crypto(Box<crypto::Args>),
    /// Generate the schema used for code generation in Iroha SDKs
    Schema(schema::Args),
    /// Commands related to genesis
    Genesis(genesis::Args),
    /// Commands related to codec
    Codec(codec::Args),
    /// Commands related to block inspection
    Kura(kura::Args),
    /// Commands related to Docker Compose configuration generation
    Swarm(swarm::Args),
    /// Commands related to building wasm smartcontracts
    #[clap(subcommand)]
    Wasm(wasm::Args),
    /// Output CLI documentation in Markdown format
    MarkdownHelp(MarkdownHelp),
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        use Args::*;

        match self {
            Crypto(args) => args.run(writer),
            Schema(args) => args.run(writer),
            Genesis(args) => args.run(writer),
            Codec(args) => args.run(writer),
            Kura(args) => args.run(writer),
            Swarm(args) => args.run(writer),
            Wasm(args) => args.run(writer),
            MarkdownHelp(args) => args.run(writer),
        }
    }
}

#[derive(Debug, ClapArgs, Clone)]
struct MarkdownHelp;

impl<T: Write> RunArgs<T> for MarkdownHelp {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let command_info = clap_markdown::help_markdown::<Args>();
        writer.write_all(command_info.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Error};

    use super::{Args, Parser};
    use crate::wasm;

    fn parse(args: &str) -> Result<Args, Error> {
        Args::try_parse_from(args.split(' '))
    }

    #[test]
    fn verify_args() {
        Args::command().debug_assert();
    }

    #[test]
    fn ok_with_flags() {
        assert!(parse(
            "kagami swarm \
            -p 20 \
            -c ./config \
            -i hyperledger/iroha \
            -o sample.yml\
            -HF",
        )
        .is_ok())
    }

    #[test]
    fn cannot_mix_print_and_force() {
        assert!(parse(
            "kagami swarm \
            -p 20 \
            -c ./config \
            -i hyperledger/iroha \
            -o sample.yml \
            -PF",
        )
        .is_err())
    }

    #[test]
    fn ok_when_pull_image() {
        assert!(parse(
            "kagami swarm \
            -p 20 \
            -c ./config \
            -i hyperledger/iroha \
            -o sample.yml",
        )
        .is_ok())
    }

    #[test]
    fn ok_when_build_image() {
        assert!(parse(
            "kagami swarm \
            -p 20 \
            -i hyperledger/iroha \
            -b . \
            -c ./config \
            -o sample.yml",
        )
        .is_ok())
    }

    #[test]
    fn fails_when_image_is_omitted() {
        assert!(parse(
            "kagami swarm \
            -p 1 \
            -c ./ \
            -o test.yml",
        )
        .is_err())
    }

    #[test]
    fn wasm_command() {
        parse("kagami wasm build ./test --out-file 1").unwrap();
        parse("kagami wasm check ./test").unwrap();

        let Args::Wasm(wasm::Args::Check { common, .. }) = Args::try_parse_from([
            "kagami",
            "wasm",
            "check",
            "./path",
            "--cargo-args=--locked --frozen",
        ])
        .unwrap() else {
            unreachable!()
        };
        assert_eq!(common.cargo_args.0, vec!["--locked", "--frozen"]);

        Args::try_parse_from([
            "kagami",
            "wasm",
            "build",
            "./path",
            "--cargo-args=--locked --frozen",
            "--out-file=test.wasm",
        ])
        .unwrap();
    }
}
