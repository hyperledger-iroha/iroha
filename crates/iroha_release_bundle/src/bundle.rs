//! Content authentication with continuous custody of the complete sealed inventory.

use std::{
    ffi::OsStr,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::Arc,
};

use iroha_fs::{FileIdentity, FlatBatchEntry, FlatBatchKind, ReaderDirectory, SealedFlatBatch};
use sha2::{Digest, Sha256};

use crate::manifest::{
    BinaryKind, ExpectedManifest, MANIFEST_NAME, MAX_BINARY_BYTES, MAX_MANIFEST_BYTES, Manifest,
    NativeIdentity, is_digest,
};

/// Native target selected by Cargo when this admitting process was compiled.
pub const NATIVE_TARGET: &str = env!("IROHA_RELEASE_NATIVE_TARGET");
pub(crate) const MANIFEST_INDEX: usize = 4;

pub(crate) fn invalid(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason)
}

pub(crate) fn inventory() -> io::Result<[FlatBatchEntry; 5]> {
    let executable = |kind: BinaryKind| {
        FlatBatchEntry::new(
            kind.filename(cfg!(windows)),
            FlatBatchKind::Executable,
            usize::try_from(MAX_BINARY_BYTES)
                .map_err(|_| invalid("release executable ceiling exceeds native usize"))?,
        )
    };
    Ok([
        executable(BinaryKind::Irohad)?,
        executable(BinaryKind::Iroha)?,
        executable(BinaryKind::Kagami)?,
        executable(BinaryKind::IrohadTaira)?,
        FlatBatchEntry::new(MANIFEST_NAME, FlatBatchKind::ReadOnly, MAX_MANIFEST_BYTES)?,
    ])
}

pub(crate) fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub(crate) fn native_identity(identity: FileIdentity) -> NativeIdentity {
    let (platform, volume, object) = identity.local_parts();
    NativeIdentity {
        platform,
        volume,
        object: hex::encode(object),
    }
}

pub(crate) fn stream_digest(mut reader: impl Read, maximum: u64) -> io::Result<(String, u64)> {
    let mut hash = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        length = length
            .checked_add(count as u64)
            .filter(|length| *length <= maximum)
            .ok_or_else(|| invalid("release input exceeds its byte ceiling"))?;
        hash.update(&buffer[..count]);
    }
    Ok((hex::encode(hash.finalize()), length))
}

/// Independent anchors inherited from the source-owning release invocation.
///
/// None of these values may be learned from the bundle being admitted. The release
/// owner authenticates the source aggregate and current lockfile before selecting a
/// bundle, and exports the manifest digest only after successful publication.
#[derive(Debug, Clone, Copy)]
pub struct BundleAnchors<'a> {
    /// Canonical full-workspace source aggregate.
    pub source: &'a str,
    /// Independently authenticated current workspace lockfile.
    pub cargo_lock: &'a str,
    /// Exact manifest digest exported by the completed publication transaction.
    pub manifest: &'a str,
}

/// All five immutable originals, their original parent, and authenticated content bindings.
///
/// A clone of an [`Arc`] containing this owner retains the same originals. This type
/// exposes neither a writable capability nor an independently owned executable path.
#[derive(Debug)]
pub struct Bundle {
    pub(crate) batch: Arc<SealedFlatBatch<5>>,
    manifest: Manifest,
    manifest_sha256: String,
}

impl Bundle {
    /// Admit the exact flat native inventory below a retained parent directory.
    ///
    /// # Errors
    /// Refuses changed custody, missing/extra files, noncanonical manifests, stale
    /// external anchors, a foreign host/target, wrong policies or changed executable bytes.
    pub fn open(
        parent: &ReaderDirectory,
        name: impl AsRef<OsStr>,
        anchors: BundleAnchors<'_>,
    ) -> io::Result<Arc<Self>> {
        Self::authenticate(parent.open_sealed_flat_batch(name, inventory()?)?, anchors)
    }

    pub(crate) fn authenticate(
        batch: SealedFlatBatch<5>,
        anchors: BundleAnchors<'_>,
    ) -> io::Result<Arc<Self>> {
        if !is_digest(anchors.manifest) {
            return Err(invalid("invalid independent manifest anchor"));
        }
        let mut bytes = Vec::new();
        batch.reader(MANIFEST_INDEX)?.read_to_end(&mut bytes)?;
        if digest(&bytes) != anchors.manifest {
            return Err(invalid(
                "release manifest differs from its independent anchor",
            ));
        }
        let manifest = Manifest::decode(
            &bytes,
            ExpectedManifest {
                source_sha256: anchors.source,
                cargo_lock_sha256: anchors.cargo_lock,
                bundle_dir: batch.path(),
                native_target: NATIVE_TARGET,
                windows: cfg!(windows),
            },
        )?;
        if manifest.directory_identity != native_identity(batch.identity()?)
            || manifest.file_identity != native_identity(batch.entry_identity(MANIFEST_INDEX)?)
        {
            return Err(invalid(
                "release directory or manifest original was substituted",
            ));
        }
        for (index, expected) in manifest.binaries.iter().enumerate() {
            if expected.identity != native_identity(batch.entry_identity(index)?) {
                return Err(invalid("release executable original was substituted"));
            }
            let (hash, length) = stream_digest(batch.reader(index)?, MAX_BINARY_BYTES)?;
            if hash != expected.sha256 || length != expected.size_bytes {
                return Err(invalid("release executable differs from its manifest"));
            }
        }
        batch.revalidate()?;
        Ok(Arc::new(Self {
            batch: Arc::new(batch),
            manifest,
            manifest_sha256: anchors.manifest.to_owned(),
        }))
    }

    /// Recheck every retained original, exact inventory and complete native custody.
    ///
    /// # Errors
    /// Refuses any original metadata, identity, inventory or custody mutation.
    pub fn revalidate(&self) -> io::Result<()> {
        self.batch.revalidate()
    }

    /// Authenticated manifest; its paths describe the originals owned by this bundle.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Manifest anchor established before any executable was admitted.
    pub fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    /// Observational directory spelling for diagnostics and publication acknowledgement.
    pub fn path(&self) -> &Path {
        self.batch.path()
    }

    /// Select a program while retaining the complete authenticated bundle.
    pub fn program(self: &Arc<Self>, kind: BinaryKind) -> Program {
        Program {
            bundle: Arc::clone(self),
            kind,
            admission: None,
        }
    }
}

/// Additional original admission owners which must remain live through process exit.
///
/// This extends the mandatory native bundle checks; it cannot replace or bypass them.
/// Implementations retain their complete original inputs and should not own a guarded
/// `Program`, which would introduce an ownership cycle. Owning its raw [`Bundle`] is safe.
pub trait AdmissionCustody: std::fmt::Debug + Send + Sync {
    /// Check all original inputs and retained policies of this admission owner.
    ///
    /// # Errors
    /// Refuses any loss or change of the independently admitted original custody.
    fn revalidate(&self) -> io::Result<()>;
}

/// Cached selection retaining the complete bundle and optional original admission owner.
#[derive(Debug, Clone)]
pub struct Program {
    pub(crate) bundle: Arc<Bundle>,
    pub(crate) kind: BinaryKind,
    admission: Option<Arc<dyn AdmissionCustody>>,
}

impl Program {
    /// Attach one validated admission owner without replacing the bundle's own checks.
    ///
    /// The guard follows every clone, configured command, child and terminal reaper.
    /// A second attachment is refused rather than discarding an earlier authority.
    ///
    /// # Errors
    /// Refuses a second guard, changed bundle custody or failed admission revalidation.
    pub fn with_admission_custody(
        mut self,
        admission: Arc<dyn AdmissionCustody>,
    ) -> io::Result<Self> {
        if self.admission.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "release program already retains its admission custody",
            ));
        }
        self.bundle.revalidate()?;
        admission.revalidate()?;
        self.bundle.revalidate()?;
        self.admission = Some(admission);
        Ok(self)
    }

    /// Selected shipping program discriminator.
    pub const fn kind(&self) -> BinaryKind {
        self.kind
    }

    /// Recheck the complete bundle and its admission owner before using this selection.
    ///
    /// # Errors
    /// Refuses any original or custody change in the bundle or its admission owner.
    pub fn revalidate(&self) -> io::Result<()> {
        self.bundle.revalidate()?;
        if let Some(admission) = &self.admission {
            admission.revalidate()?;
        }
        self.bundle.revalidate()
    }

    /// Observe the original's name for diagnostics, never as standalone launch authority.
    ///
    /// # Errors
    /// Refuses any changed bundle or admission custody.
    pub fn diagnostic_path(&self) -> io::Result<PathBuf> {
        self.revalidate()?;
        self.bundle.batch.entry_path(self.kind.index())
    }
}
