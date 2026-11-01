//! Explicit native publisher arguments and bounded publication acknowledgement.

use std::{
    collections::BTreeMap,
    ffi::OsString,
    fmt,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use iroha_fs::{FileSnapshot, OwnerDirectory, PrivateDirectory, ReaderDirectory, RetainedFile};
use iroha_release_bundle::{
    bundle::{Bundle, BundleAnchors},
    publish::{PublishInputs, publish},
};
use sha2::{Digest as _, Sha256};

mod bootstrap;
mod build;
mod environment;
mod policy;
mod provenance;
#[cfg(test)]
mod tests;

/// Refused command with the preserved cooperative or child exit classification.
#[derive(Debug)]
pub struct Failure {
    error: io::Error,
    code: u8,
}

impl Failure {
    pub(super) fn exit_code(&self) -> u8 {
        self.code
    }
    fn cancelled() -> Self {
        Self {
            error: io::Error::new(
                io::ErrorKind::Interrupted,
                "cooperative cancellation requested at command boundary",
            ),
            code: 125,
        }
    }
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        Self { error, code: 2 }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

fn invalid(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, reason)
}

fn exact_path(value: OsString) -> io::Result<PathBuf> {
    let path = PathBuf::from(value);
    let normalized: PathBuf = path.components().collect();
    if !path.is_absolute()
        || normalized.as_os_str() != path.as_os_str()
        || path
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err(invalid("paths must have normalized absolute spelling"));
    }
    let text = path
        .to_str()
        .ok_or_else(|| invalid("release command paths must be UTF-8"))?;
    if text.chars().any(char::is_control) {
        return Err(invalid("release paths contain control characters"));
    }
    Ok(path)
}

fn digest_argument(value: OsString) -> io::Result<String> {
    let text = value
        .into_string()
        .map_err(|_| invalid("digest is not UTF-8"))?;
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(
            "digest must contain exactly 64 lowercase hexadecimal characters",
        ));
    }
    Ok(text)
}

fn invocation_name(value: OsString) -> io::Result<OsString> {
    let text = value
        .to_str()
        .ok_or_else(|| invalid("invocation name is not UTF-8"))?;
    let suffix = text
        .strip_prefix("invocation.")
        .ok_or_else(|| invalid("invalid invocation name"))?;
    if suffix.len() != 6 || !suffix.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(invalid(
            "invocation name requires exactly six alphanumeric characters",
        ));
    }
    Ok(value)
}

#[derive(Clone)]
struct Arguments(BTreeMap<String, OsString>);

impl Arguments {
    fn parse(values: impl Iterator<Item = OsString>) -> io::Result<Self> {
        let mut values = values;
        let mut parsed = BTreeMap::new();
        while let Some(name) = values.next() {
            let name = name
                .into_string()
                .map_err(|_| invalid("option name is not UTF-8"))?;
            if !name.starts_with("--") || name.contains('=') {
                return Err(invalid(
                    "expected one explicit option followed by its value",
                ));
            }
            let value = values
                .next()
                .ok_or_else(|| invalid("option is missing its value"))?;
            if parsed.insert(name, value).is_some() {
                return Err(invalid("duplicate option"));
            }
            if parsed.len() > 16 {
                return Err(invalid("too many options"));
            }
        }
        Ok(Self(parsed))
    }
    fn take(&mut self, name: &'static str) -> io::Result<OsString> {
        self.0.remove(name).ok_or_else(|| invalid(name))
    }
    fn path(&mut self, name: &'static str) -> io::Result<PathBuf> {
        exact_path(self.take(name)?)
    }
    fn digest(&mut self, name: &'static str) -> io::Result<String> {
        digest_argument(self.take(name)?)
    }
    fn finish(self) -> io::Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(invalid("unknown option"))
        }
    }
}

fn private_owner(path: &Path) -> io::Result<(PrivateDirectory, OwnerDirectory)> {
    let private = PrivateDirectory::open_exact(path)?;
    let owner = OwnerDirectory::open(path)?;
    if private.identity()? != owner.identity()? {
        return Err(invalid("private owner changed during admission"));
    }
    private.revalidate()?;
    Ok((private, owner))
}

fn lock_digest(path: &Path) -> io::Result<(RetainedFile, FileSnapshot, String)> {
    let mut original = RetainedFile::open_regular(path)?;
    let before = original.snapshot()?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    let mut total = 0_usize;
    loop {
        let count = original.file_mut().read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count)
            .filter(|total| *total <= 16 * 1024 * 1024)
            .ok_or_else(|| invalid("Cargo.lock exceeds its ceiling"))?;
        hash.update(&buffer[..count]);
    }
    original.revalidate()?;
    if original.snapshot()? != before {
        return Err(invalid("Cargo.lock changed while hashing"));
    }
    Ok((original, before, hex::encode(hash.finalize())))
}

fn admit(
    parent: &Path,
    name: &std::ffi::OsStr,
    source: &str,
    lock: &str,
    manifest: &str,
) -> io::Result<(Arc<Bundle>, PrivateDirectory)> {
    let private = PrivateDirectory::open_exact(parent)?;
    let reader = ReaderDirectory::open(parent)?;
    if private.identity()? != reader.identity()? {
        return Err(invalid("bundle parent changed during admission"));
    }
    let bundle = Bundle::open(
        &reader,
        name,
        BundleAnchors {
            source,
            cargo_lock: lock,
            manifest,
        },
    )?;
    private.revalidate()?;
    Ok((bundle, private))
}

fn acknowledge(bundle: &Bundle, output: &mut impl Write) -> io::Result<()> {
    bundle.revalidate()?;
    let path = bundle
        .path()
        .to_str()
        .ok_or_else(|| invalid("bundle path is not UTF-8"))?;
    if path.chars().any(char::is_control) {
        return Err(invalid("bundle path contains control characters"));
    }
    write!(
        output,
        "bundle_dir\t{path}\nmanifest_sha256\t{}\n",
        bundle.manifest_sha256()
    )?;
    output.flush()?;
    bundle.revalidate()
}

/// Execute one explicit command, accepting output only after retained publication.
///
/// # Errors
/// Refuses malformed arguments, invalid custody, failed children and publication errors.
pub fn run(values: impl Iterator<Item = OsString>, output: &mut impl Write) -> Result<(), Failure> {
    run_with_environment(values, output, environment::BuildEnvironment::capture)
}

// Capture is deferred until a new build session, after the original argument and
// inherited-bundle admission branches. Fixtures supply explicit owned OS inputs.
fn run_with_environment(
    values: impl Iterator<Item = OsString>,
    output: &mut impl Write,
    environment: fn() -> io::Result<environment::BuildEnvironment>,
) -> Result<(), Failure> {
    let mut values = values;
    let command = values
        .next()
        .ok_or_else(|| invalid("expected publish, admit, build-publish, bootstrap-build-publish, or build-bridge-publish"))?;
    if command == "--help" {
        if values.next().is_some() {
            return Err(invalid("--help accepts no arguments").into());
        }
        writeln!(
            output,
            "iroha_release_bundle publish|admit|build-publish|bootstrap-build-publish|build-bridge-publish: see src/bin/iroha_release_bundle/README.md for explicit native arguments"
        )?;
        return Ok(());
    }
    let mut arguments = Arguments::parse(values)?;
    match command.to_str() {
        Some("publish") => {
            let parent = arguments.path("--parent")?;
            let name = invocation_name(arguments.take("--name")?)?;
            let source = arguments.digest("--source-sha256")?;
            let build_directory = arguments.path("--build-dir")?;
            let cargo_lock = arguments.path("--cargo-lock")?;
            let cargo_version = arguments.path("--cargo-version")?;
            let rustc_version = arguments.path("--rustc-version")?;
            arguments.finish()?;
            let (private, owner) = private_owner(&parent)?;
            let bundle = publish(
                &owner,
                name,
                PublishInputs {
                    source_sha256: &source,
                    build_directory: &build_directory,
                    cargo_lock: &cargo_lock,
                    cargo_version: &cargo_version,
                    rustc_version: &rustc_version,
                },
            )?;
            private.revalidate()?;
            acknowledge(&bundle, output)?;
            private.revalidate()?;
        }
        Some("admit") => {
            let parent = arguments.path("--parent")?;
            let name = invocation_name(arguments.take("--name")?)?;
            let source = arguments.digest("--source-sha256")?;
            let lock = arguments.digest("--cargo-lock-sha256")?;
            let manifest = arguments.digest("--manifest-sha256")?;
            arguments.finish()?;
            let (bundle, private) = admit(&parent, &name, &source, &lock, &manifest)?;
            acknowledge(&bundle, output)?;
            private.revalidate()?;
        }
        Some("build-publish") => build::run(arguments, output, environment)?,
        Some("bootstrap-build-publish") => bootstrap::run(arguments, output, environment)?,
        Some("build-bridge-publish") => bootstrap::run_bridge(arguments, output, environment)?,
        _ => return Err(invalid("unknown release bundle command").into()),
    }
    Ok(())
}
