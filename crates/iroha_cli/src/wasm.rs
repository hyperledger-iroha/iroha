#![allow(missing_docs)]

use std::path::PathBuf;

use eyre::{eyre, Result, WrapErr};
use iroha_wasm_builder::Builder;
use owo_colors::OwoColorize;

use crate::options;

/// Arguments for wasm subcommand
#[derive(clap::Args, Debug)]
pub struct Args {
    /// Subcommands related to smartcontracts
    #[clap(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Check smartcontract souce files (apply cargo check)
    Check(check::Args),
    /// Build smartcontract from source files
    Build(build::Args),
    /// Run smartcontract tests
    Test(test::Args),
}

impl Command {
    fn run(self) -> Result<()> {
        use Command::*;
        match self {
            Check(args) => args.run(),
            Build(args) => args.run(),
            Test(args) => args.run(),
        }
    }
}

mod test {
    use wasmtime::{Engine, Instance, Module, Store};

    use super::*;

    struct TestMeta<'a> {
        name: &'a str,
        ignore: bool,
    }

    #[derive(clap::Args, Debug)]
    pub struct Args {
        #[command(flatten)]
        common: CommonArgs,
    }

    impl Args {
        pub fn run(self) -> Result<()> {
            // Modules can be compiled through either the text or binary format
            let engine = Engine::default();
            let module =
                Module::from_file(&engine, self.common.path).map_err(|e| eyre::eyre!(e))?;
            let mut tests = Vec::new();
            for export in module.exports() {
                if let Some(name) = export.name().strip_prefix("$webassembly-test$") {
                    let mut ignore = true;
                    let name = name.strip_prefix("ignore$").unwrap_or_else(|| {
                        ignore = false;
                        name
                    });
                    tests.push((export, TestMeta { name, ignore }));
                }
            }
            let total = tests.len();

            eprintln!("\nrunning {total} tests");
            let mut store = Store::new(&engine, ());
            let mut instance =
                Instance::new(&mut store, &module, &[]).map_err(|e| eyre::eyre!(e))?;
            let mut passed = 0;
            let mut failed = 0;
            let mut ignored = 0;
            for (export, meta) in tests {
                eprint!("test {} ...", meta.name);
                if meta.ignore {
                    ignored += 1;
                    eprintln!(" ignored");
                } else {
                    let f = instance
                        .get_typed_func::<(), ()>(&mut store, export.name())
                        .map_err(|e| eyre::eyre!(e))?;

                    let pass = f.call(&mut store, ()).is_ok();
                    if pass {
                        passed += 1;
                        eprintln!(" ok");
                    } else {
                        // Reset instance on test failure. WASM uses `panic=abort`, so
                        // `Drop`s are not called after test failures, and a failed test
                        // might leave an instance in an inconsistent state.
                        store = Store::new(&engine, ());
                        instance =
                            Instance::new(&mut store, &module, &[]).map_err(|e| eyre::eyre!(e))?;

                        failed += 1;
                        eprintln!(" FAILED");
                    }
                }
            }
            eprintln!(
                "\ntest result: {}. {} passed; {} failed; {} ignored;",
                if failed > 0 { "FAILED" } else { "ok" },
                passed,
                failed,
                ignored,
            );

            if failed > 0 {
                Err(eyre!("Some tests failed!"))
            } else {
                Ok(())
            }
        }
    }
}

mod check {
    use super::*;

    #[derive(clap::Args, Debug)]
    pub struct Args {
        #[command(flatten)]
        common: CommonArgs,
    }

    impl Args {
        pub fn run(self) -> Result<()> {
            let builder = Builder::new(&self.common.path).show_output();
            builder.check()
        }
    }
}

mod build {
    use super::*;

    #[derive(clap::Args, Debug)]
    pub struct Args {
        #[command(flatten)]
        common: super::CommonArgs,
        /// Optimize WASM output.
        #[arg(long)]
        optimize: bool,
        /// Where to store the output WASM. If the file exists, it will be overwritten.
        #[arg(long, value_name("PATH"), value_hint(clap::ValueHint::FilePath))]
        out_file: PathBuf,
    }

    impl Args {
        pub fn run(self) -> Result<()> {
            let builder = Builder::new(&self.common.path).show_output();

            let output = {
                // not showing the spinner here, cargo does a progress bar for us

                match builder.build() {
                    Ok(output) => output,
                    err => err?,
                }
            };

            let output = if self.optimize {
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

            std::fs::copy(output.wasm_file_path(), &self.out_file).wrap_err_with(|| {
                eyre!(
                    "Failed to write the resulting file into {}",
                    self.out_file.display()
                )
            })?;

            println!(
                "✓ File is written into {}",
                self.out_file.display().green().bold()
            );

            Ok(())
        }
    }
}

#[derive(clap::Args, Debug)]
struct CommonArgs {
    /// Path to the smartcontract
    path: PathBuf,
}

impl options::RunArgs for Args {
    fn run(self) -> Result<()> {
        self.command.run()
    }
}
