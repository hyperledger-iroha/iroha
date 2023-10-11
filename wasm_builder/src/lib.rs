//! Crate with helper tool to build smartcontracts (e.g. triggers and executors) for Iroha 2.
//!
//! See [`Builder`] for more details.

use std::{
    borrow::Cow,
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
};

use eyre::{bail, eyre, Context as _, Result};
use path_absolutize::Absolutize;

/// Current toolchain used to build smartcontracts
const TOOLCHAIN: &str = "+nightly-2023-06-25";

/// WASM Builder for smartcontracts (e.g. triggers and executors).
///
/// # Example
///
/// ```no_run
/// use iroha_wasm_builder::Builder;
/// use eyre::Result;
///
/// fn main() -> Result<()> {
///     let bytes = Builder::new("relative/path/to/smartcontract/")
///         .out_dir("path/to/out/dir") // Optional: Set output directory
///         .format() // Optional: Enable smartcontract formatting
///         .build()? // Run build
///         .optimize()? // Optimize WASM output
///         .into_bytes()?; // Get resulting WASM bytes
///
///     // ...
///
///     Ok(())
/// }
/// ```
#[derive(Debug)]
#[must_use]
pub struct Builder<'path, 'out_dir> {
    /// Path to the smartcontract or to the directory of smartcontracts
    path: &'path Path,
    /// Build output directory
    out_dir: Option<&'out_dir Path>,
    /// Flag to enable smartcontract formatting
    format: bool,
}

impl<'path, 'out_dir> Builder<'path, 'out_dir> {
    /// Initialize [`Builder`] with path to smartcontract.
    ///
    /// `relative_path` should be relative to `CARGO_MANIFEST_DIR`.
    pub fn new<P>(relative_path: &'path P) -> Self
    where
        P: AsRef<Path> + ?Sized,
    {
        Self {
            path: relative_path.as_ref(),
            out_dir: None,
            format: false,
        }
    }

    /// Set smartcontract build output directory.
    ///
    /// By default the output directory will be assigned either from `IROHA_WASM_BUILDER_OUT_DIR` or
    /// `OUT_DIR` environment variables, in this order.
    /// If neither is set, then the `target` directory will be used.
    pub fn out_dir<O>(mut self, out_dir: &'out_dir O) -> Self
    where
        O: AsRef<Path> + ?Sized,
    {
        self.out_dir = Some(out_dir.as_ref());
        self
    }

    /// Enable smartcontract formatting using `cargo fmt`.
    ///
    /// Disabled by default.
    pub fn format(mut self) -> Self {
        self.format = true;
        self
    }

    /// Apply `cargo check` to the smartcontract.
    ///
    /// # Errors
    ///
    /// Can fail due to multiple reasons like invalid path, failed formatting, failed build, etc.
    pub fn check(self) -> Result<()> {
        self.into_internal()?.check()
    }

    /// Build smartcontract and get resulting wasm.
    ///
    /// # Errors
    ///
    /// Can fail due to multiple reasons like invalid path, failed formatting,
    /// failed build, etc.
    ///
    /// Will also return error if ran on workspace and not on the concrete package.
    pub fn build(self) -> Result<Output> {
        self.into_internal()?.build()
    }

    fn into_internal(self) -> Result<internal::Builder<'out_dir>> {
        let abs_path = Self::absolute_path(self.path)?;
        Ok(internal::Builder {
            absolute_path: abs_path
                .canonicalize()
                .wrap_err_with(|| format!("Failed to canonicalize path: {}", abs_path.display()))?,
            out_dir: self.out_dir.map_or_else(
                || -> Result<_> { Ok(Cow::Owned(Self::default_out_dir()?)) },
                |out_dir| Ok(Cow::Borrowed(out_dir)),
            )?,
            format: self.format,
        })
    }

    fn default_out_dir() -> Result<PathBuf> {
        env::var_os("IROHA_WASM_BUILDER_OUT_DIR")
            .or_else(|| env::var_os("OUT_DIR"))
            .map(PathBuf::from)
            .map_or_else(
                || {
                    const PATH: &str = "target";
                    let abs_path = Self::absolute_path(PATH)?;
                    std::fs::DirBuilder::new()
                        .recursive(true)
                        .create(&abs_path)
                        .wrap_err_with(|| {
                            format!("Failed to create directory at path: {}", abs_path.display())
                        })?;
                    abs_path.canonicalize().wrap_err_with(|| {
                        format!("Failed to canonicalize path: {}", abs_path.display())
                    })
                },
                Ok,
            )
    }

    fn absolute_path(relative_path: impl AsRef<Path>) -> Result<PathBuf> {
        // TODO: replace with [std::path::absolute](https://doc.rust-lang.org/stable/std/path/fn.absolute.html)
        // when it's stabilized
        relative_path
            .as_ref()
            .absolutize()
            .map(|abs_path| abs_path.to_path_buf())
            .wrap_err_with(|| {
                format!(
                    "Failed to construct absolute path for: {}",
                    relative_path.as_ref().display(),
                )
            })
    }
}

mod internal {
    //! Internal implementation of [`Builder`](super::Builder).

    use std::borrow::Cow;

    use super::*;

    #[derive(Debug)]
    pub struct Builder<'out_dir> {
        pub absolute_path: PathBuf,
        pub out_dir: Cow<'out_dir, Path>,
        pub format: bool,
    }

    impl Builder<'_> {
        pub fn check(self) -> Result<()> {
            self.maybe_format()?;

            self.check_smartcontract().wrap_err_with(|| {
                format!(
                    "Failed to check the smartcontract at path: {}",
                    self.absolute_path.display()
                )
            })
        }

        pub fn build(self) -> Result<Output> {
            self.maybe_format()?;

            let absolute_path = self.absolute_path.clone();
            self.build_smartcontract().wrap_err_with(|| {
                format!(
                    "Failed to build the smartcontract at path: {}",
                    absolute_path.display()
                )
            })
        }

        fn maybe_format(&self) -> Result<()> {
            if self.format {
                self.format_smartcontract().wrap_err_with(|| {
                    format!(
                        "Failed to format the smartcontract at path: {}",
                        self.absolute_path.display()
                    )
                })?;
            }
            Ok(())
        }

        fn build_options() -> impl Iterator<Item = &'static str> {
            [
                "--release",
                "-Z",
                "build-std",
                "-Z",
                "build-std-features=panic_immediate_abort",
                "-Z",
                "unstable-options",
                "--target",
                "wasm32-unknown-unknown",
            ]
            .into_iter()
        }

        fn format_smartcontract(&self) -> Result<()> {
            let command_output = cargo_command()
                .current_dir(&self.absolute_path)
                .arg("fmt")
                .output()
                .wrap_err("Failed to run `cargo fmt`")?;

            check_command_output(&command_output, "cargo fmt")
        }

        fn get_base_command(&self, cmd: &'static str) -> std::process::Command {
            let mut command = cargo_command();
            command
                .current_dir(&self.absolute_path)
                .arg(cmd)
                .args(Self::build_options());

            command
        }

        fn check_smartcontract(&self) -> Result<()> {
            let command_output = self
                .get_base_command("check")
                .output()
                .wrap_err("Failed to run `cargo check`")?;

            check_command_output(&command_output, "cargo check")
        }

        fn build_smartcontract(self) -> Result<Output> {
            let package_name = self
                .retrieve_package_name()
                .wrap_err("Failed to retrieve package name")?;

            let full_out_dir = self.out_dir.join("wasm32-unknown-unknown/release/");
            let wasm_file = full_out_dir.join(package_name).with_extension("wasm");

            let previous_hash = if wasm_file.exists() {
                let hash = sha256::try_digest(wasm_file.as_path()).wrap_err_with(|| {
                    format!(
                        "Failed to compute sha256 digest of wasm file: {}",
                        wasm_file.display()
                    )
                })?;
                Some(hash)
            } else {
                None
            };

            let command_output = self
                .get_base_command("build")
                .env("CARGO_TARGET_DIR", self.out_dir.as_ref())
                .output()
                .wrap_err("Failed to run `cargo build`")?;

            check_command_output(&command_output, "cargo build")?;

            Ok(Output {
                wasm_file,
                previous_hash,
            })
        }

        fn retrieve_package_name(&self) -> Result<String> {
            use std::str::FromStr as _;

            let manifest_output = cargo_command()
                .current_dir(&self.absolute_path)
                .arg("read-manifest")
                .output()
                .wrap_err("Failed to run `cargo read-manifest`")?;

            check_command_output(&manifest_output, "cargo read-manifest")?;

            let manifest = String::from_utf8(manifest_output.stdout)
                .wrap_err("Failed to convert `cargo read-manifest` output to string")?;

            serde_json::Value::from_str(&manifest)
                .wrap_err("Failed to parse `cargo read-manifest` output")?
                .get("name")
                .map(serde_json::Value::to_string)
                .map(|name| name.trim_matches('"').to_owned())
                .ok_or_else(|| {
                    eyre!("Failed to retrieve package name from `cargo read-manifest` output")
                })
        }
    }
}

/// Build output representing wasm binary.
#[derive(Debug)]
pub struct Output {
    /// Path to the non-optimized `.wasm` file.
    wasm_file: PathBuf,
    /// Hash of the `self.wasm_file` on previous iteration if there is some.
    previous_hash: Option<String>,
}

impl Output {
    /// Optimize wasm output.
    ///
    /// # Errors
    ///
    /// Fails if internal tool fails to optimize wasm binary.
    pub fn optimize(self) -> Result<Self> {
        let optimized_file = PathBuf::from(format!(
            "{parent}{separator}{file}_optimized.{ext}",
            parent = self
                .wasm_file
                .parent()
                .map_or_else(String::default, |p| p.display().to_string()),
            separator = std::path::MAIN_SEPARATOR,
            file = self
                .wasm_file
                .file_stem()
                .map_or_else(|| "output".into(), OsStr::to_string_lossy),
            ext = self
                .wasm_file
                .extension()
                .map_or_else(|| "wasm".into(), OsStr::to_string_lossy),
        ));

        let current_hash = sha256::try_digest(self.wasm_file.as_path()).wrap_err_with(|| {
            format!(
                "Failed to compute sha256 digest of wasm file: {}",
                self.wasm_file.display()
            )
        })?;

        match self.previous_hash {
            Some(previous_hash) if optimized_file.exists() && current_hash == previous_hash => {
                // Do nothing because original `.wasm` file wasn't changed
                // so `_optimized.wasm` should stay the same
            }
            _ => {
                let optimizer = wasm_opt::OptimizationOptions::new_optimize_for_size();
                optimizer.run(self.wasm_file, optimized_file.as_path())?;
            }
        }

        Ok(Self {
            wasm_file: optimized_file,
            previous_hash: Some(current_hash),
        })
    }

    /// Consume [`Output`] and get the underling bytes.
    ///
    /// # Errors
    ///
    /// Fails if the output file cannot be read.
    pub fn into_bytes(self) -> Result<Vec<u8>> {
        use std::{fs::File, io::Read as _};

        let mut wasm_file = File::open(self.wasm_file)?;
        let mut wasm_data = Vec::new();
        wasm_file
            .read_to_end(&mut wasm_data)
            .wrap_err("Failed to read data from the output wasm file")?;

        Ok(wasm_data)
    }

    /// Get the file path of the underlying WASM
    pub fn wasm_file_path(&self) -> &PathBuf {
        &self.wasm_file
    }
}

// TODO: Remove cargo invocation (#2152)
fn cargo_command() -> Command {
    let mut cargo = Command::new("cargo");
    cargo
        // Removing environment variable to avoid
        // `error: infinite recursion detected` when running `cargo lints`
        .env_remove("RUST_RECURSION_COUNT")
        // Removing environment variable to avoid
        // `error: `profiler_builtins` crate (required by compiler options) is not compatible with crate attribute `#![no_core]``
        // when running with `-C instrument-coverage`
        // TODO: Check if there are no problems with that
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .args([
            TOOLCHAIN,
        ]);
    cargo
}

fn check_command_output(command_output: &std::process::Output, command_name: &str) -> Result<()> {
    if !command_output.status.success() {
        bail!(
            "`{}` returned non zero exit code ({}). Stderr:\n{}",
            command_name,
            command_output.status,
            String::from_utf8_lossy(&command_output.stderr)
        );
    }

    Ok(())
}
