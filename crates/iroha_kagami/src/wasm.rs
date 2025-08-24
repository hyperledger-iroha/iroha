use std::{
    io::{BufWriter, Write},
    path::PathBuf,
    str::FromStr,
};

use clap::{arg, command, Args as ClapArgs, Subcommand};
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
        /// Where to store the output WASM. If the file exists, it will be overwritten.
        #[arg(long)]
        out_file: PathBuf,
    },
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
            } => {
                let mut args = cargo_args.0;
                let profile = match args.iter().position(|arg| arg.contains("--profile")) {
                    Some(idx) => {
                        let profile = args
                            .get(idx + 1)
                            .expect("--profile requires a value in the form: --cargo-args=\"--profile <PROFILE>..\"");
                        let profile = profile.parse::<Profile>().expect(&format!(
                            "unknown profile `{}`, valid options are: deploy, release",
                            profile
                        ));
                        args.drain(idx..idx + 2);
                        profile
                    }
                    None => {
                        eprintln!("warning: \"--cargo-args\" missing \"--profile\"; using `release` by default. Use --cargo-args=\"--profile <PROFILE>..\" to specify one.");
                        Profile::Release
                    }
                };

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
