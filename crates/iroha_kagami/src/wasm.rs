use std::{
    io::{BufWriter, Write},
    path::PathBuf,
    str::FromStr,
};

use clap::{command, Args as ClapArgs, CommandFactory, FromArgMatches, Parser, Subcommand};
use color_eyre::eyre::{eyre, Context};
use iroha_wasm_builder::{Builder, Profile};
use owo_colors::OwoColorize;

use crate::{Outcome, RunArgs};

#[derive(Debug, Clone, Subcommand)]
pub enum Args {
    /// Apply `cargo check` to the smartcontract
    Check {
        #[command(flatten)]
        common: CommonArgs,
        #[arg(long, default_value = "release")]
        profile: Profile,
    },
    /// Build the smartcontract
    Build {
        #[command(flatten)]
        common: CommonArgs,
        /// Build profile
        #[arg(long)]
        profile: Option<Profile>,
        /// Where to store the output WASM. If the file exists, it will be overwritten.
        #[arg(long)]
        out_file: PathBuf,
    },
}

#[derive(Parser, Debug, Default)]
#[command(no_binary_name = true, disable_help_flag = true)]
struct ProfileArg {
    #[arg(long, value_enum)]
    profile: Profile,
}

#[derive(ClapArgs, Debug, Clone)]
pub struct CommonArgs {
    /// Path to the smartcontract
    path: PathBuf,
    /// Extra arguments to pass to `cargo`, e.g. `--locked`
    #[arg(long, require_equals(true), default_value = "")]
    pub(crate) cargo_args: CargoArgs,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CargoArgs(pub Vec<String>);

impl FromStr for CargoArgs {
    type Err = shell_words::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        shell_words::split(s).map(Self)
    }
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        match self {
            Args::Check {
                common: CommonArgs { path, cargo_args },
                profile,
            } => {
                let builder = Builder::new(&path, profile)
                    .cargo_args(cargo_args.0)
                    .show_output();
                builder.check()?;
            }
            Args::Build {
                common: CommonArgs { path, cargo_args },
                out_file,
                profile,
            } => {
                let mut args = cargo_args.0.clone();
                let mut profile_arg = Vec::<String>::with_capacity(2);

                if let Some(arg_idx) = args.iter().position(|arg| arg.contains("--profile")) {
                    if profile.is_some() {
                        eprintln!("warning: value from \"--profile\" is ignored in favor of profile in \"--cargo-args\"");
                    }
                    args.get(arg_idx + 1)
                        .expect("expected format --cargo-args='--profile <PROFILE>..'")
                        .parse::<Profile>()
                        .expect(&format!("Unknown Profile ({})", args[arg_idx + 1]));
                    profile_arg.extend(args.drain(arg_idx..arg_idx + 2));
                } else if profile.is_some() {
                    eprintln!("warning: \"--profile\" arg is deprecated; please use \"--cargo-args='--profile ..'\" instead");
                } else {
                    eprintln!("warning: \"--cargo-args\" missing \"--profile\"; using `release` by default. Use \"--cargo-args='--profile ..'\" to specify one.");
                }

                let matches = ProfileArg::command()
                    .no_binary_name(true)
                    .ignore_errors(true)
                    .try_get_matches_from(&profile_arg)
                    .unwrap_or_default();
                let profile_arg = ProfileArg::from_arg_matches(&matches).unwrap_or_default();

                let profile = profile_arg.profile;

                let builder = Builder::new(&path, profile).cargo_args(args).show_output();

                let output = {
                    // not showing the spinner here, cargo does a progress bar for us
                    match builder.build_unoptimized() {
                        Ok(output) => output,
                        err => err?,
                    }
                };

                let output = if profile.is_optimized() {
                    let sp = if std::env::var("CI").is_err() {
                        Some(spinoff::Spinner::new_with_stream(
                            spinoff::spinners::Binary,
                            "Optimizing the output",
                            None,
                            spinoff::Streams::Stderr,
                        ))
                    } else {
                        None
                    };

                    match output.optimize() {
                        Ok(optimized) => {
                            if let Some(mut sp) = sp {
                                sp.success("Output is optimized");
                            }
                            optimized
                        }
                        err => {
                            if let Some(mut sp) = sp {
                                sp.fail("Optimization failed");
                            }
                            err?
                        }
                    }
                } else {
                    output
                };

                std::fs::copy(output.wasm_file_path(), &out_file).wrap_err_with(|| {
                    eyre!(
                        "Failed to write the resulting file into {}",
                        out_file.display()
                    )
                })?;

                writeln!(
                    writer,
                    "✓ File is written into {}",
                    out_file.display().green().bold()
                )?;
            }
        }

        Ok(())
    }
}
