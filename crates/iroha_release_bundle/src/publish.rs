//! Native publication while every selected build and binding input remains retained.

use std::{
    ffi::OsStr,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::Arc,
};

use iroha_fs::{FileSnapshot, FlatBatchSealStep, OwnerDirectory, RetainedBuildInput, RetainedFile};
use sha2::{Digest, Sha256};

use crate::{
    bundle::{
        Bundle, BundleAnchors, MANIFEST_INDEX, NATIVE_TARGET, digest, invalid, inventory,
        native_identity,
    },
    manifest::{
        BinaryAttestation, BinaryKind, ExpectedManifest, MAX_BINARY_BYTES, Manifest, SCHEMA,
        custody_policies, is_digest,
    },
};

const MAX_TOOL_TRANSCRIPT: u64 = 64 * 1024;
const MAX_LOCKFILE: u64 = 16 * 1024 * 1024;

/// Inputs supplied by the continuous full-workspace source owner.
///
/// The caller must retain its authenticated source census throughout this call and
/// accept the returned acknowledgement only after its own final custody check. This
/// publisher owns the build, tool and lockfile originals; a supplied source digest
/// alone does not establish full-workspace custody.
#[derive(Debug, Clone, Copy)]
pub struct PublishInputs<'a> {
    /// Canonical workspace aggregate calculated by the live source owner.
    pub source_sha256: &'a str,
    /// Native Cargo release directory containing all four mandatory programs.
    pub build_directory: &'a Path,
    /// Current workspace lockfile, retained independently of the source aggregate.
    pub cargo_lock: &'a Path,
    /// Successful Cargo version command's exact owner-private transcript.
    pub cargo_version: &'a Path,
    /// Successful verbose rustc version command's exact owner-private transcript.
    pub rustc_version: &'a Path,
}

/// Observable publication boundaries; failures preserve the claimed namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishStep {
    /// All seven originals are retained and the tool/lock bindings have been read.
    InputsPinned,
    /// The indexed executable has been copied from its retained original.
    ExecutableCopied(usize),
    /// Canonical manifest bytes have been written to their original creator.
    ManifestWritten,
    /// Native original-handle custody transfer within the flat batch.
    Sealing(FlatBatchSealStep),
    /// Complete sealed inventory and copied bytes have been authenticated.
    Authenticated,
}

struct RegularPin {
    original: RetainedFile,
    snapshot: FileSnapshot,
}

impl RegularPin {
    fn open(path: &Path, private: bool) -> io::Result<Self> {
        let original = if private {
            RetainedFile::open_private(path)?
        } else {
            RetainedFile::open_regular(path)?
        };
        let snapshot = original.snapshot()?;
        Ok(Self { original, snapshot })
    }

    fn revalidate(&self) -> io::Result<()> {
        self.original.revalidate()?;
        if self.original.snapshot()? != self.snapshot {
            return Err(invalid(
                "release binding original changed during publication",
            ));
        }
        Ok(())
    }

    fn read(&mut self, maximum: u64) -> io::Result<Vec<u8>> {
        self.revalidate()?;
        self.original.file_mut().seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        self.original
            .file_mut()
            .take(maximum + 1)
            .read_to_end(&mut bytes)?;
        self.revalidate()?;
        if bytes.len() as u64 > maximum {
            return Err(invalid("release binding input exceeds its ceiling"));
        }
        Ok(bytes)
    }
}

struct BuildPin {
    original: RetainedBuildInput,
    snapshot: FileSnapshot,
}

impl BuildPin {
    fn open(path: &Path) -> io::Result<Self> {
        let original = RetainedBuildInput::open(path)?;
        let snapshot = original.snapshot()?;
        if !(1..=MAX_BINARY_BYTES).contains(&original.len()?) {
            return Err(invalid(
                "release executable is empty or exceeds its ceiling",
            ));
        }
        Ok(Self { original, snapshot })
    }

    fn revalidate(&self) -> io::Result<()> {
        self.original.revalidate()?;
        if self.original.snapshot()? != self.snapshot {
            return Err(invalid("release build original changed during publication"));
        }
        Ok(())
    }

    fn copy(&mut self, mut output: impl Write) -> io::Result<(String, u64)> {
        self.revalidate()?;
        self.original.seek(SeekFrom::Start(0))?;
        let mut hash = Sha256::new();
        let mut length = 0_u64;
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            let count = self.original.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            length = length
                .checked_add(count as u64)
                .filter(|length| *length <= MAX_BINARY_BYTES)
                .ok_or_else(|| invalid("release build input grew beyond its ceiling"))?;
            output.write_all(&buffer[..count])?;
            hash.update(&buffer[..count]);
        }
        output.flush()?;
        self.revalidate()?;
        if length != self.original.len()? {
            return Err(invalid("release build input extent changed during copy"));
        }
        Ok((hex::encode(hash.finalize()), length))
    }
}

fn validate_tools(cargo: &[u8], rustc: &[u8]) -> io::Result<()> {
    for transcript in [cargo, rustc] {
        if transcript.contains(&0) || transcript.contains(&b'\r') || !transcript.ends_with(b"\n") {
            return Err(invalid("release tool transcript is not canonical stdout"));
        }
    }
    let cargo =
        std::str::from_utf8(cargo).map_err(|_| invalid("Cargo version transcript is not UTF-8"))?;
    let rustc =
        std::str::from_utf8(rustc).map_err(|_| invalid("rustc version transcript is not UTF-8"))?;
    if !cargo.starts_with("cargo ") || !rustc.starts_with("rustc ") {
        return Err(invalid(
            "release tool version transcript has the wrong program",
        ));
    }
    let hosts: Vec<_> = rustc
        .lines()
        .filter_map(|line| line.strip_prefix("host: "))
        .collect();
    if hosts != [NATIVE_TARGET] {
        return Err(invalid(
            "release rustc transcript is not the actual native host",
        ));
    }
    Ok(())
}

/// Publish the four shipping originals and their canonical manifest in one sealed directory.
///
/// The parent source owner must keep its source originals alive until it has checked
/// the returned acknowledgement. Existing names are never adopted, repaired or deleted.
///
/// # Errors
/// Refuses unsafe originals, changed pins, foreign tool host, incorrect bounds,
/// existing destination, native sealing errors and failed content authentication.
pub fn publish(
    parent: &OwnerDirectory,
    name: impl AsRef<OsStr>,
    inputs: PublishInputs<'_>,
) -> io::Result<Arc<Bundle>> {
    publish_observed(parent, name, inputs, |_| Ok(()))
}

/// Publish with fallible observations at every copy and native custody transfer.
///
/// # Errors
/// Returns all errors of [`publish`] and observer errors. A failure preserves the
/// claimed namespace, including partially sealed originals, for explicit inspection.
pub fn publish_observed(
    parent: &OwnerDirectory,
    name: impl AsRef<OsStr>,
    inputs: PublishInputs<'_>,
    mut observe: impl FnMut(PublishStep) -> io::Result<()>,
) -> io::Result<Arc<Bundle>> {
    if !is_digest(inputs.source_sha256) {
        return Err(invalid("invalid source-owner aggregate anchor"));
    }
    let mut lock = RegularPin::open(inputs.cargo_lock, false)?;
    let mut cargo = RegularPin::open(inputs.cargo_version, true)?;
    let mut rustc = RegularPin::open(inputs.rustc_version, true)?;
    let mut builds = BinaryKind::ALL
        .into_iter()
        .map(|kind| BuildPin::open(&inputs.build_directory.join(kind.filename(cfg!(windows)))))
        .collect::<io::Result<Vec<_>>>()?;
    let lock_bytes = lock.read(MAX_LOCKFILE)?;
    let cargo_bytes = cargo.read(MAX_TOOL_TRANSCRIPT)?;
    let rustc_bytes = rustc.read(MAX_TOOL_TRANSCRIPT)?;
    validate_tools(&cargo_bytes, &rustc_bytes)?;
    let check = |builds: &[BuildPin]| -> io::Result<()> {
        lock.revalidate()?;
        cargo.revalidate()?;
        rustc.revalidate()?;
        for build in builds {
            build.revalidate()?;
        }
        Ok(())
    };
    observe(PublishStep::InputsPinned)?;
    check(&builds)?;
    let destination = parent.path().join(name.as_ref());
    let (_, executable_custody, file_custody) = custody_policies(cfg!(windows));
    let mut pending = parent.create_flat_batch(name, inventory()?)?;
    let mut binaries = Vec::with_capacity(4);
    for index in 0..builds.len() {
        let (sha256, size_bytes) = builds[index].copy(pending.writer(index)?)?;
        binaries.push(BinaryAttestation {
            kind: BinaryKind::ALL[index],
            filename: BinaryKind::ALL[index].filename(cfg!(windows)).to_owned(),
            sha256,
            size_bytes,
            custody: executable_custody.to_owned(),
            identity: native_identity(pending.entry_identity(index)?),
        });
        observe(PublishStep::ExecutableCopied(index))?;
        check(&builds)?;
    }
    let lock_sha256 = digest(&lock_bytes);
    let manifest = Manifest {
        schema: SCHEMA.to_owned(),
        source_manifest_sha256: inputs.source_sha256.to_owned(),
        cargo_lock_sha256: lock_sha256.clone(),
        cargo_version_sha256: digest(&cargo_bytes),
        rustc_version_sha256: digest(&rustc_bytes),
        host_triple: NATIVE_TARGET.to_owned(),
        target_triple: NATIVE_TARGET.to_owned(),
        profile: "release".to_owned(),
        bundle_dir: destination
            .to_str()
            .ok_or_else(|| invalid("release directory is not UTF-8"))?
            .to_owned(),
        directory_custody: custody_policies(cfg!(windows)).0.to_owned(),
        file_custody: file_custody.to_owned(),
        directory_identity: native_identity(pending.identity()?),
        file_identity: native_identity(pending.entry_identity(MANIFEST_INDEX)?),
        binaries,
    };
    let bytes = manifest.encode(ExpectedManifest {
        source_sha256: inputs.source_sha256,
        cargo_lock_sha256: &lock_sha256,
        bundle_dir: &destination,
        native_target: NATIVE_TARGET,
        windows: cfg!(windows),
    })?;
    pending.writer(MANIFEST_INDEX)?.write_all(&bytes)?;
    observe(PublishStep::ManifestWritten)?;
    check(&builds)?;
    let sealed = pending.seal_observed(|step| {
        observe(PublishStep::Sealing(step))?;
        check(&builds)
    })?;
    let manifest_sha256 = digest(&bytes);
    let bundle = Bundle::authenticate(
        sealed,
        BundleAnchors {
            source: inputs.source_sha256,
            cargo_lock: &lock_sha256,
            manifest: &manifest_sha256,
        },
    )?;
    observe(PublishStep::Authenticated)?;
    check(&builds)?;
    bundle.revalidate()?;
    Ok(bundle)
}
