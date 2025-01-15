use std::path::PathBuf;

use clap::{Subcommand, Args as ClapArgs};
use color_eyre::eyre::{eyre, Context};
use iroha_wasm_builder::Builder;
use owo_colors::OwoColorize;
use std::io::{BufWriter, Write};

use crate::{Outcome, RunArgs};

#[derive(Debug, Clone, Subcommand)]
pub enum Args {
    /// Apply `cargo check` to the smartcontract
    Check {
        #[command(flatten)]
        common: CommonArgs,
    },
    /// Build the smartcontract
    Build {
        #[command(flatten)]
        common: CommonArgs,
        /// Optimize WASM output.
        #[arg(long)]
        optimize: bool,
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
            } => {
                let builder = Builder::new(&path).show_output();
                builder.check()?;
            }
            Args::Build {
                common: CommonArgs { path },
                optimize,
                out_file,
            } => {
                let builder = Builder::new(&path).show_output();

                let output = {
                    // not showing the spinner here, cargo does a progress bar for us

                    match builder.build() {
                        Ok(output) => output,
                        err => err?,
                    }
                };

                let output = if optimize {
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
