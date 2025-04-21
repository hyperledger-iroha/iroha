use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

use clap::{Args as ClapArgs, Subcommand};
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
        #[arg(long, default_value = "release")]
        profile: Profile,
        /// Where to store the output WASM. If the file exists, it will be overwritten.
        #[arg(long)]
        out_file: PathBuf,
    },
}

#[derive(ClapArgs, Debug, Clone)]
pub struct CommonArgs {
    /// Path to the smartcontract
    path: PathBuf,
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        match self {
            Args::Check {
                common: CommonArgs { path },
                profile,
            } => {
                let builder = Builder::new(&path, profile).show_output();
                builder.check()?;
            }
            Args::Build {
                common: CommonArgs { path },
                out_file,
                profile,
            } => {
                let builder = Builder::new(&path, profile).show_output();

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
