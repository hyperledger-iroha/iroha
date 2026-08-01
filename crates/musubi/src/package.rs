//! Secure, deterministic planning for Musubi V1 source packages.
//!
//! This module deliberately starts from manifest-declared roots and additions. It does not
//! inventory the package directory and then subtract an ignore list. Callers are expected to
//! translate a validated `Musubi.toml` into [`PackageLayout`], pass the original manifest and
//! verification lock documents to [`plan_package`], and use the resulting immutable plan for a
//! clean compiler check and SoraFS CAR construction.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt, fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
    str::FromStr,
};

pub use iroha_data_model::musubi::MusubiArtifactDescriptorV1;
use iroha_data_model::{
    musubi::{MusubiContentDigestV1, MusubiSemanticReleaseManifestV1, MusubiVerificationLockV1},
    name::Name,
};
#[cfg(test)]
use norito::codec::Decode;
use norito::codec::Encode;
use sorafs_car::{CarBuildPlan, CarWriteStats, CarWriter, FileEntry, FilePayload, PayloadSource};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;

/// Maximum total bytes in a Musubi V1 normalized source tree.
pub const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum bytes in a Musubi V1 CARv2 archive.
pub const MAX_CAR_BYTES: u64 = 96 * 1024 * 1024;
/// Maximum number of regular files in a Musubi V1 source tree.
pub const MAX_SOURCE_FILES: usize = 4_096;
/// Maximum number of chunks in a Musubi V1 source CAR plan.
pub const MAX_SOURCE_CHUNKS: usize = 16_384;

const MANIFEST_PATH: &str = "Musubi.toml";
const VERIFICATION_LOCK_PATH: &str = "Musubi.lock";
const BUNDLE_RELEASE_PATH: &str = ".musubi/semantic-release.norito";
const BUNDLE_DESCRIPTOR_PATH: &str = ".musubi/artifact-descriptor.norito";
const BUNDLE_VERIFICATION_LOCK_PATH: &str = ".musubi/verification-lock.norito";
const MAX_PATH_COMPONENTS: usize = 64;
const MAX_PATH_COMPONENT_BYTES: usize = 255;
const MAX_PATH_BYTES: usize = 4 * 1024;

const SOURCE_TREE_DOMAIN: &[u8] = b"musubi-source-tree-v1\0";
const ARTIFACT_DESCRIPTOR_DOMAIN: &[u8] = b"musubi-artifact-descriptor-v1\0";
const BUNDLE_DOMAIN: &[u8] = b"musubi-bundle-v1\0";

/// A manifest-derived positive selection of package files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageLayout {
    root: PathBuf,
    library: Option<PathBuf>,
    contracts: Vec<PathBuf>,
    tests: Vec<PathBuf>,
    readme: Option<PathBuf>,
    license: Option<PathBuf>,
    includes: Vec<PathBuf>,
}

impl PackageLayout {
    /// Start a package layout rooted at `root` with no source selectors.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            library: None,
            contracts: Vec::new(),
            tests: Vec::new(),
            readme: None,
            license: None,
            includes: Vec::new(),
        }
    }

    /// Return the package root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Set the declared library source directory.
    pub fn set_library(&mut self, path: impl Into<PathBuf>) {
        self.library = Some(path.into());
    }

    /// Add one declared local contract file or directory.
    pub fn add_contract(&mut self, path: impl Into<PathBuf>) {
        self.contracts.push(path.into());
    }

    /// Add one declared test file or directory.
    pub fn add_test(&mut self, path: impl Into<PathBuf>) {
        self.tests.push(path.into());
    }

    /// Set the declared readme file.
    pub fn set_readme(&mut self, path: impl Into<PathBuf>) {
        self.readme = Some(path.into());
    }

    /// Set the declared license file.
    pub fn set_license(&mut self, path: impl Into<PathBuf>) {
        self.license = Some(path.into());
    }

    /// Add one explicit positive include file or directory.
    pub fn add_include(&mut self, path: impl Into<PathBuf>) {
        self.includes.push(path.into());
    }
}

/// One immutable file selected for a clean package tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedFile {
    path: String,
    components: Vec<String>,
    bytes: Vec<u8>,
}

impl PlannedFile {
    /// Return the canonical portable archive path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Return the canonical portable path components used by SoraFS.
    #[must_use]
    pub fn components(&self) -> &[String] {
        &self.components
    }

    /// Return the exact file payload.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// A bounded, byte-ordered package file plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackagePlan {
    files: Vec<PlannedFile>,
    source_bytes: u64,
}

impl PackagePlan {
    /// Return files in strict UTF-8 byte order by canonical portable path.
    #[must_use]
    pub fn files(&self) -> &[PlannedFile] {
        &self.files
    }

    /// Return the total regular-file payload size.
    #[must_use]
    pub const fn source_bytes(&self) -> u64 {
        self.source_bytes
    }

    /// Return the generated normalized verification lock bytes.
    #[must_use]
    pub fn verification_lock(&self) -> &[u8] {
        self.required_file(VERIFICATION_LOCK_PATH)
    }

    /// Return the canonicalized package manifest bytes.
    #[must_use]
    pub fn canonical_manifest(&self) -> &[u8] {
        self.required_file(MANIFEST_PATH)
    }

    /// Build deterministic source-tree, descriptor, and bundle commitment materials.
    ///
    /// # Errors
    ///
    /// Returns an error when the semantic manifest or exact verification lock is invalid, or
    /// when their release and lock-digest bindings differ.
    pub fn commitment_materials(
        &self,
        semantic_release_manifest: &MusubiSemanticReleaseManifestV1,
        verification_lock: &MusubiVerificationLockV1,
    ) -> Result<PackageCommitments, PackageError> {
        semantic_release_manifest
            .validate()
            .map_err(|error| PackageError::InvalidBundleBinding(error.to_string()))?;
        verification_lock
            .validate()
            .map_err(|error| PackageError::InvalidBundleBinding(error.to_string()))?;
        if verification_lock.root != semantic_release_manifest.release
            || verification_lock.digest() != semantic_release_manifest.verification_lock_digest
        {
            return Err(PackageError::InvalidBundleBinding(
                "semantic release and exact verification lock do not match".to_owned(),
            ));
        }
        let semantic_release_bytes = semantic_release_manifest.encode();
        let verification_lock_bytes = verification_lock.encode();
        let source_tree_material = source_tree_material(&self.files);
        let source_tree_digest =
            MusubiContentDigestV1::new(domain_digest(SOURCE_TREE_DOMAIN, &source_tree_material));
        let descriptor = MusubiArtifactDescriptorV1::new(
            semantic_release_manifest.semantic_digest(),
            source_tree_digest,
            verification_lock.digest(),
            self.source_bytes,
            u32::try_from(self.files.len()).expect("Musubi V1 file bound fits u32"),
        )
        .map_err(|error| PackageError::InvalidBundleBinding(error.to_string()))?;
        let descriptor_material = descriptor_commitment_material(&descriptor);
        let descriptor_digest = MusubiContentDigestV1::new(domain_digest(
            ARTIFACT_DESCRIPTOR_DOMAIN,
            &descriptor_material,
        ));
        let bundle_material = bundle_commitment_material(
            &semantic_release_bytes,
            &descriptor_material,
            &source_tree_material,
            &verification_lock_bytes,
        );
        let bundle_digest =
            MusubiContentDigestV1::new(domain_digest(BUNDLE_DOMAIN, &bundle_material));
        Ok(PackageCommitments {
            source_tree_material,
            source_tree_digest,
            descriptor,
            descriptor_material,
            descriptor_digest,
            bundle_material,
            bundle_digest,
        })
    }

    /// Consume the plan and construct its bounded deterministic SoraFS CARv2 bundle.
    ///
    /// The CAR directory DAG contains the normalized source tree and verification
    /// lock plus the exact semantic release manifest and typed artifact descriptor.
    /// Providers can therefore attest to parsing the bundle rather than merely
    /// storing an opaque byte string.
    ///
    /// # Errors
    ///
    /// Returns an error if SoraFS rejects the logical plan, the V1 chunk ceiling is exceeded,
    /// writing fails, or the final CAR exceeds 96 MiB.
    pub fn into_car(
        self,
        semantic_release_manifest: &MusubiSemanticReleaseManifestV1,
        verification_lock: &MusubiVerificationLockV1,
    ) -> Result<PackageCar, PackageError> {
        let commitments =
            self.commitment_materials(semantic_release_manifest, verification_lock)?;
        let semantic_release_bytes = semantic_release_manifest.encode();
        let verification_lock_bytes = verification_lock.encode();
        let source_file_count = self.files.len();
        let source_bytes = self.source_bytes;
        let mut entries = self
            .files
            .into_iter()
            .map(|file| FileEntry {
                path: file.components,
                data: file.bytes,
            })
            .collect::<Vec<_>>();
        entries.push(FileEntry {
            path: BUNDLE_RELEASE_PATH.split('/').map(str::to_owned).collect(),
            data: semantic_release_bytes,
        });
        entries.push(FileEntry {
            path: BUNDLE_DESCRIPTOR_PATH
                .split('/')
                .map(str::to_owned)
                .collect(),
            data: commitments.descriptor.encode(),
        });
        entries.push(FileEntry {
            path: BUNDLE_VERIFICATION_LOCK_PATH
                .split('/')
                .map(str::to_owned)
                .collect(),
            data: verification_lock_bytes,
        });
        let (plan, payload) = CarBuildPlan::from_files(entries)
            .map_err(|error| PackageError::CarPlan(error.to_string()))?;
        enforce_chunk_limit(plan.chunks.len())?;

        let mut output = BoundedWriter::new(MAX_CAR_BYTES);
        let writer = CarWriter::new(&plan, &payload)
            .map_err(|error| PackageError::CarWrite(error.to_string()))?;
        let stats = match writer.write_to(&mut output) {
            Ok(stats) => stats,
            Err(_) if output.exceeded => {
                return Err(PackageError::CarTooLarge {
                    bytes: output.attempted,
                    maximum: MAX_CAR_BYTES,
                });
            }
            Err(error) => return Err(PackageError::CarWrite(error.to_string())),
        };
        enforce_car_limit(stats.car_size)?;
        Ok(PackageCar {
            plan,
            payload,
            bytes: output.bytes,
            stats,
            commitments,
            source_file_count,
            source_bytes,
        })
    }

    fn required_file(&self, path: &str) -> &[u8] {
        self.files
            .binary_search_by(|file| file.path.as_str().cmp(path))
            .ok()
            .and_then(|index| self.files.get(index))
            .map(PlannedFile::bytes)
            .expect("package planner always injects canonical manifest and lock")
    }
}

/// A deterministic, bounded SoraFS CAR generated from a [`PackagePlan`].
#[derive(Debug)]
pub struct PackageCar {
    plan: CarBuildPlan,
    payload: Vec<u8>,
    bytes: Vec<u8>,
    stats: CarWriteStats,
    commitments: PackageCommitments,
    source_file_count: usize,
    source_bytes: u64,
}

impl PackageCar {
    /// Return the validated SoraFS CAR plan.
    #[must_use]
    pub const fn plan(&self) -> &CarBuildPlan {
        &self.plan
    }

    /// Return the exact concatenated source payload expected by the CAR plan.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Return the complete deterministic CARv2 bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Return SoraFS writer statistics and roots.
    #[must_use]
    pub const fn stats(&self) -> &CarWriteStats {
        &self.stats
    }

    /// Return all normalized source-tree, descriptor, and bundle commitments.
    #[must_use]
    pub const fn commitments(&self) -> &PackageCommitments {
        &self.commitments
    }

    /// Return the source-tree file count, excluding three mandatory bundle metadata entries.
    #[must_use]
    pub const fn source_file_count(&self) -> usize {
        self.source_file_count
    }

    /// Return source-tree bytes, excluding the three mandatory bundle metadata entries.
    #[must_use]
    pub const fn source_bytes(&self) -> u64 {
        self.source_bytes
    }
}

/// All canonical materials and digests needed by the archive-commitment layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageCommitments {
    source_tree_material: Vec<u8>,
    source_tree_digest: MusubiContentDigestV1,
    descriptor: MusubiArtifactDescriptorV1,
    descriptor_material: Vec<u8>,
    descriptor_digest: MusubiContentDigestV1,
    bundle_material: Vec<u8>,
    bundle_digest: MusubiContentDigestV1,
}

impl PackageCommitments {
    /// Return the canonical source-tree transcript.
    #[must_use]
    pub fn source_tree_material(&self) -> &[u8] {
        &self.source_tree_material
    }

    /// Return the domain-separated source-tree digest.
    #[must_use]
    pub const fn source_tree_digest(&self) -> MusubiContentDigestV1 {
        self.source_tree_digest
    }

    /// Return the typed artifact descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &MusubiArtifactDescriptorV1 {
        &self.descriptor
    }

    /// Return the domain-prefixed Norito descriptor material.
    #[must_use]
    pub fn descriptor_material(&self) -> &[u8] {
        &self.descriptor_material
    }

    /// Return the descriptor digest.
    #[must_use]
    pub const fn descriptor_digest(&self) -> MusubiContentDigestV1 {
        self.descriptor_digest
    }

    /// Return the canonical bundle transcript.
    #[must_use]
    pub fn bundle_material(&self) -> &[u8] {
        &self.bundle_material
    }

    /// Return the domain-separated bundle digest.
    #[must_use]
    pub const fn bundle_digest(&self) -> MusubiContentDigestV1 {
        self.bundle_digest
    }
}

/// Error returned while planning or encoding a secure package.
#[derive(Debug)]
pub enum PackageError {
    /// A filesystem operation failed.
    Io {
        /// Operation being performed without secret-bearing input.
        operation: &'static str,
        /// Path involved in the operation.
        path: PathBuf,
        /// Underlying I/O failure.
        source: io::Error,
    },
    /// The package root is not a real directory.
    InvalidRoot(PathBuf),
    /// A selector was absolute, empty, traversing, or otherwise invalid.
    InvalidSelector(PathBuf),
    /// A selector had the wrong file kind.
    WrongFileKind {
        /// Declared selector.
        path: PathBuf,
        /// Required kind.
        expected: &'static str,
    },
    /// A path was not valid portable UTF-8.
    NonPortablePath(String),
    /// Two selected names collide on portable case/Unicode-normalizing filesystems.
    PathCollision {
        /// First selected path.
        first: String,
        /// Conflicting selected path.
        second: String,
    },
    /// A selected entry was a symlink.
    Symlink(PathBuf),
    /// A selected regular file had more than one hard link.
    Hardlink(PathBuf),
    /// A selected entry was neither a regular file nor a directory.
    SpecialFile(PathBuf),
    /// A selected path belongs to a generated, VCS, or local configuration root.
    ExcludedPath(PathBuf),
    /// A selected path is a known credential or private-key location.
    SensitivePath(PathBuf),
    /// Selected bytes contain a known credential/private-key marker.
    SensitiveContent {
        /// Portable path of the rejected file.
        path: String,
        /// Stable marker class; never contains the matched secret.
        marker: &'static str,
    },
    /// The normalized source tree exceeds the file-count ceiling.
    TooManyFiles {
        /// Planned file count.
        count: usize,
        /// V1 maximum.
        maximum: usize,
    },
    /// The normalized source tree exceeds the payload-byte ceiling.
    SourceTooLarge {
        /// Planned source bytes.
        bytes: u64,
        /// V1 maximum.
        maximum: u64,
    },
    /// The SoraFS plan exceeds the chunk-count ceiling.
    TooManyChunks {
        /// Planned chunk count.
        count: usize,
        /// V1 maximum.
        maximum: usize,
    },
    /// The generated CAR exceeds the encoded-byte ceiling.
    CarTooLarge {
        /// Encoded or attempted CAR bytes.
        bytes: u64,
        /// V1 maximum.
        maximum: u64,
    },
    /// A TOML document was invalid or lacked its V1 marker.
    InvalidDocument {
        /// Stable document name.
        document: &'static str,
        /// Parse or schema-marker failure.
        reason: String,
    },
    /// Typed semantic release and verification-lock bindings were inconsistent.
    InvalidBundleBinding(String),
    /// SoraFS rejected the logical source plan.
    CarPlan(String),
    /// SoraFS could not encode the CAR.
    CarWrite(String),
}

impl fmt::Display for PackageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "failed to {operation} `{}`: {source}",
                path.display()
            ),
            Self::InvalidRoot(path) => write!(
                formatter,
                "package root `{}` must be a real directory",
                path.display()
            ),
            Self::InvalidSelector(path) => write!(
                formatter,
                "package selector `{}` must be a confined relative path",
                path.display()
            ),
            Self::WrongFileKind { path, expected } => write!(
                formatter,
                "package selector `{}` must be {expected}",
                path.display()
            ),
            Self::NonPortablePath(path) => {
                write!(formatter, "package path `{path}` is not portable UTF-8")
            }
            Self::PathCollision { first, second } => write!(
                formatter,
                "package paths `{first}` and `{second}` have a portable case/Unicode collision"
            ),
            Self::Symlink(path) => write!(
                formatter,
                "symlink `{}` is forbidden in a package",
                path.display()
            ),
            Self::Hardlink(path) => write!(
                formatter,
                "hard-linked file `{}` is forbidden in a package",
                path.display()
            ),
            Self::SpecialFile(path) => write!(
                formatter,
                "special file `{}` is forbidden in a package",
                path.display()
            ),
            Self::ExcludedPath(path) => write!(
                formatter,
                "generated, VCS, or local configuration path `{}` cannot be packaged",
                path.display()
            ),
            Self::SensitivePath(path) => write!(
                formatter,
                "credential or private-key path `{}` cannot be packaged",
                path.display()
            ),
            Self::SensitiveContent { path, marker } => write!(
                formatter,
                "package file `{path}` contains forbidden {marker} material"
            ),
            Self::TooManyFiles { count, maximum } => write!(
                formatter,
                "package has {count} files; Musubi V1 permits at most {maximum}"
            ),
            Self::SourceTooLarge { bytes, maximum } => write!(
                formatter,
                "package has {bytes} source bytes; Musubi V1 permits at most {maximum}"
            ),
            Self::TooManyChunks { count, maximum } => write!(
                formatter,
                "package has {count} chunks; Musubi V1 permits at most {maximum}"
            ),
            Self::CarTooLarge { bytes, maximum } => write!(
                formatter,
                "package CAR has {bytes} bytes; Musubi V1 permits at most {maximum}"
            ),
            Self::InvalidDocument { document, reason } => {
                write!(formatter, "invalid {document}: {reason}")
            }
            Self::InvalidBundleBinding(reason) => {
                write!(formatter, "invalid semantic release bundle: {reason}")
            }
            Self::CarPlan(reason) => write!(formatter, "invalid SoraFS package plan: {reason}"),
            Self::CarWrite(reason) => write!(formatter, "failed to encode package CAR: {reason}"),
        }
    }
}

impl Error for PackageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Canonicalize and validate a Musubi V1 package manifest.
///
/// The TOML parser rejects duplicate keys. Semantic unknown-field validation remains the
/// responsibility of the manifest module before it constructs [`PackageLayout`].
///
/// # Errors
///
/// Returns an error for malformed TOML or a missing/non-V1 `manifest-version` marker.
pub fn canonicalize_manifest_toml(input: &str) -> Result<Vec<u8>, PackageError> {
    let (table, bytes) = canonicalize_toml(MANIFEST_PATH, input)?;
    if table
        .get("manifest-version")
        .and_then(toml::Value::as_integer)
        != Some(1)
    {
        return Err(PackageError::InvalidDocument {
            document: MANIFEST_PATH,
            reason: "`manifest-version = 1` is required".to_owned(),
        });
    }
    Ok(bytes)
}

/// Canonicalize and validate a generated Musubi V1 verification lock.
///
/// Exact graph normalization (including deterministic node and edge ordering) belongs to the
/// lockfile module. This helper makes its TOML representation canonical and enforces the reset
/// markers so an old consumer lock cannot enter a release bundle.
///
/// # Errors
///
/// Returns an error for malformed TOML or missing `schema = "musubi-lock"`/`version = 1`.
pub fn normalize_verification_lock_toml(input: &str) -> Result<Vec<u8>, PackageError> {
    let (table, bytes) = canonicalize_toml(VERIFICATION_LOCK_PATH, input)?;
    let schema_matches = table.get("schema").and_then(toml::Value::as_str) == Some("musubi-lock");
    let version_matches = table.get("version").and_then(toml::Value::as_integer) == Some(1);
    if !schema_matches || !version_matches {
        return Err(PackageError::InvalidDocument {
            document: VERIFICATION_LOCK_PATH,
            reason:
                "`schema = \"musubi-lock\"` and `version = 1` are required; regenerate the lockfile"
                    .to_owned(),
        });
    }
    reject_consumer_only_lock_fields(&toml::Value::Table(table))?;
    Ok(bytes)
}

/// Plan a clean, positive-set Musubi V1 source tree.
///
/// `manifest_toml` is parsed and rendered canonically. `verification_lock_toml` must be the
/// exact verification graph generated for publication and is normalized before inclusion.
/// Only the library/contracts/tests/readme/license/include selectors in `layout` are visited.
///
/// # Errors
///
/// Returns an error for invalid documents, unsafe paths or filesystem objects, credential
/// material, inconsistent filesystem identity, or a V1 resource-limit violation.
pub fn plan_package(
    layout: &PackageLayout,
    manifest_toml: &str,
    verification_lock_toml: &str,
) -> Result<PackagePlan, PackageError> {
    let manifest = canonicalize_manifest_toml(manifest_toml)?;
    let lock = normalize_verification_lock_toml(verification_lock_toml)?;
    let root = validate_root(layout.root())?;
    let mut collector = Collector::new(root);
    collector.insert_virtual(MANIFEST_PATH, manifest)?;
    collector.insert_virtual(VERIFICATION_LOCK_PATH, lock)?;

    if let Some(path) = &layout.library {
        collector.collect_selector(path, SelectionShape::Directory)?;
    }
    for path in &layout.contracts {
        collector.collect_selector(path, SelectionShape::Either)?;
    }
    for path in &layout.tests {
        collector.collect_selector(path, SelectionShape::Either)?;
    }
    if let Some(path) = &layout.readme {
        collector.collect_selector(path, SelectionShape::File)?;
    }
    if let Some(path) = &layout.license {
        collector.collect_selector(path, SelectionShape::File)?;
    }
    for path in &layout.includes {
        collector.collect_selector(path, SelectionShape::Either)?;
    }

    collector.finish()
}

fn canonicalize_toml(
    document: &'static str,
    input: &str,
) -> Result<(toml::Table, Vec<u8>), PackageError> {
    let table = input
        .parse::<toml::Table>()
        .map_err(|error| PackageError::InvalidDocument {
            document,
            reason: error.to_string(),
        })?;
    let mut output = toml::to_string(&table).map_err(|error| PackageError::InvalidDocument {
        document,
        reason: error.to_string(),
    })?;
    if !output.ends_with('\n') {
        output.push('\n');
    }
    Ok((table, output.into_bytes()))
}

#[derive(Clone, Copy)]
enum SelectionShape {
    File,
    Directory,
    Either,
}

struct Collector {
    root: PathBuf,
    files: BTreeMap<String, PlannedFile>,
    collision_origins: BTreeMap<String, String>,
    source_bytes: u64,
}

impl Collector {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            files: BTreeMap::new(),
            collision_origins: BTreeMap::new(),
            source_bytes: 0,
        }
    }

    fn insert_virtual(&mut self, path: &str, bytes: Vec<u8>) -> Result<(), PackageError> {
        if let Some(marker) = sensitive_content_marker(&bytes) {
            return Err(PackageError::SensitiveContent {
                path: path.to_owned(),
                marker,
            });
        }
        let components = canonical_portable_components(path)?;
        self.insert(path.to_owned(), path.to_owned(), components, bytes)
    }

    fn collect_selector(
        &mut self,
        selector: &Path,
        shape: SelectionShape,
    ) -> Result<(), PackageError> {
        let relative = validate_selector(selector)?;
        if relative.as_os_str().is_empty() {
            if matches!(shape, SelectionShape::File) {
                return Err(PackageError::WrongFileKind {
                    path: selector.to_path_buf(),
                    expected: "a regular file",
                });
            }
            return self.collect_directory(Path::new(""));
        }
        reject_explicit_excluded_or_sensitive(&relative)?;
        validate_no_symlink_chain(&self.root, &relative)?;
        let physical = self.root.join(&relative);
        let metadata = fs::symlink_metadata(&physical)
            .map_err(|source| io_error("inspect package selector", &relative, source))?;
        if metadata.file_type().is_symlink() {
            return Err(PackageError::Symlink(relative));
        }
        if metadata.is_file() {
            if matches!(shape, SelectionShape::Directory) {
                return Err(PackageError::WrongFileKind {
                    path: relative,
                    expected: "a directory",
                });
            }
            self.collect_file(&relative)
        } else if metadata.is_dir() {
            if matches!(shape, SelectionShape::File) {
                return Err(PackageError::WrongFileKind {
                    path: relative,
                    expected: "a regular file",
                });
            }
            self.collect_directory(&relative)
        } else {
            Err(PackageError::SpecialFile(relative))
        }
    }

    fn collect_directory(&mut self, relative: &Path) -> Result<(), PackageError> {
        let physical = self.root.join(relative);
        validate_confined_directory(&self.root, relative)?;
        let entries = fs::read_dir(&physical)
            .map_err(|source| io_error("read package directory", relative, source))?;
        let mut ordered = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| io_error("read package entry", relative, source))?;
            let name = entry.file_name().into_string().map_err(|name| {
                PackageError::NonPortablePath(relative.join(name).to_string_lossy().into_owned())
            })?;
            ordered.push((name, entry));
        }
        ordered.sort_unstable_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));

        for (name, entry) in ordered {
            let child = relative.join(&name);
            let file_type = entry
                .file_type()
                .map_err(|source| io_error("inspect package entry", &child, source))?;
            if file_type.is_symlink() {
                return Err(PackageError::Symlink(child));
            }
            if is_sensitive_component(&name) {
                return Err(PackageError::SensitivePath(child));
            }
            if file_type.is_dir() && is_excluded_directory(&name) {
                continue;
            }
            if file_type.is_file() && is_fixed_generated_file(&child) {
                continue;
            }
            if file_type.is_dir() {
                self.collect_directory(&child)?;
            } else if file_type.is_file() {
                self.collect_file(&child)?;
            } else {
                return Err(PackageError::SpecialFile(child));
            }
        }
        Ok(())
    }

    fn collect_file(&mut self, relative: &Path) -> Result<(), PackageError> {
        if is_fixed_generated_file(relative) {
            return Ok(());
        }
        let original = relative_to_utf8(relative)?;
        let components = canonical_portable_components(&original)?;
        let canonical = components.join("/");

        if self
            .collision_origins
            .get(&portable_collision_key(&components))
            .is_some_and(|existing| existing == &original)
        {
            return Ok(());
        }
        enforce_file_limit(self.files.len().saturating_add(1))?;
        let physical = self.root.join(relative);
        validate_no_symlink_chain(&self.root, relative)?;
        validate_confined_parent(&self.root, relative)?;
        let linked = fs::symlink_metadata(&physical)
            .map_err(|source| io_error("inspect package file", relative, source))?;
        if linked.file_type().is_symlink() {
            return Err(PackageError::Symlink(relative.to_path_buf()));
        }
        if !linked.is_file() {
            return Err(PackageError::SpecialFile(relative.to_path_buf()));
        }
        #[cfg(unix)]
        if linked.nlink() != 1 {
            return Err(PackageError::Hardlink(relative.to_path_buf()));
        }
        let next_size =
            self.source_bytes
                .checked_add(linked.len())
                .ok_or(PackageError::SourceTooLarge {
                    bytes: u64::MAX,
                    maximum: MAX_SOURCE_BYTES,
                })?;
        enforce_source_limit(next_size)?;

        let mut source = FilePayload::open(&physical)
            .map_err(|source| io_error("securely open package file", relative, source))?;
        let size = usize::try_from(linked.len()).map_err(|_| PackageError::SourceTooLarge {
            bytes: linked.len(),
            maximum: MAX_SOURCE_BYTES,
        })?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(size).map_err(|source| {
            io_error(
                "allocate package file buffer",
                relative,
                io::Error::other(source),
            )
        })?;
        bytes.resize(size, 0);
        source.read_exact(0, &mut bytes).map_err(|source| {
            io_error(
                "read stable package file",
                relative,
                io::Error::other(source),
            )
        })?;
        source.ensure_exhausted(linked.len()).map_err(|source| {
            io_error(
                "verify stable package file",
                relative,
                io::Error::other(source),
            )
        })?;
        if let Some(marker) = sensitive_content_marker(&bytes) {
            return Err(PackageError::SensitiveContent {
                path: canonical,
                marker,
            });
        }
        self.insert(original, canonical, components, bytes)
    }

    fn insert(
        &mut self,
        original: String,
        canonical: String,
        components: Vec<String>,
        bytes: Vec<u8>,
    ) -> Result<(), PackageError> {
        let collision_key = portable_collision_key(&components);
        if let Some(first) = self.collision_origins.get(&collision_key) {
            if first != &original {
                return Err(PackageError::PathCollision {
                    first: first.clone(),
                    second: original,
                });
            }
            return Ok(());
        }
        enforce_file_limit(self.files.len().saturating_add(1))?;
        let byte_len = u64::try_from(bytes.len()).map_err(|_| PackageError::SourceTooLarge {
            bytes: u64::MAX,
            maximum: MAX_SOURCE_BYTES,
        })?;
        let next_size =
            self.source_bytes
                .checked_add(byte_len)
                .ok_or(PackageError::SourceTooLarge {
                    bytes: u64::MAX,
                    maximum: MAX_SOURCE_BYTES,
                })?;
        enforce_source_limit(next_size)?;
        self.collision_origins.insert(collision_key, original);
        self.files.insert(
            canonical.clone(),
            PlannedFile {
                path: canonical,
                components,
                bytes,
            },
        );
        self.source_bytes = next_size;
        Ok(())
    }

    fn finish(self) -> Result<PackagePlan, PackageError> {
        enforce_file_limit(self.files.len())?;
        enforce_source_limit(self.source_bytes)?;
        Ok(PackagePlan {
            files: self.files.into_values().collect(),
            source_bytes: self.source_bytes,
        })
    }
}

fn validate_root(root: &Path) -> Result<PathBuf, PackageError> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|source| io_error("inspect package root", root, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PackageError::InvalidRoot(root.to_path_buf()));
    }
    fs::canonicalize(root).map_err(|source| io_error("canonicalize package root", root, source))
}

fn validate_selector(selector: &Path) -> Result<PathBuf, PackageError> {
    if selector.is_absolute() {
        return Err(PackageError::InvalidSelector(selector.to_path_buf()));
    }
    let mut output = PathBuf::new();
    for component in selector.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(component) => {
                let value = component.to_str().ok_or_else(|| {
                    PackageError::NonPortablePath(selector.to_string_lossy().into_owned())
                })?;
                canonical_portable_component(value)?;
                output.push(value);
            }
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(PackageError::InvalidSelector(selector.to_path_buf()));
            }
        }
    }
    Ok(output)
}

fn validate_no_symlink_chain(root: &Path, relative: &Path) -> Result<(), PackageError> {
    let mut current = root.to_path_buf();
    let mut shown = PathBuf::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            continue;
        };
        current.push(component);
        shown.push(component);
        let metadata = fs::symlink_metadata(&current)
            .map_err(|source| io_error("inspect selected path", &shown, source))?;
        if metadata.file_type().is_symlink() {
            return Err(PackageError::Symlink(shown));
        }
    }
    Ok(())
}

fn validate_confined_directory(root: &Path, relative: &Path) -> Result<(), PackageError> {
    validate_no_symlink_chain(root, relative)?;
    let physical = root.join(relative);
    let metadata = fs::symlink_metadata(&physical)
        .map_err(|source| io_error("inspect package directory", relative, source))?;
    if metadata.file_type().is_symlink() {
        return Err(PackageError::Symlink(relative.to_path_buf()));
    }
    if !metadata.is_dir() {
        return Err(PackageError::SpecialFile(relative.to_path_buf()));
    }
    let canonical = fs::canonicalize(&physical)
        .map_err(|source| io_error("canonicalize package directory", relative, source))?;
    if !canonical.starts_with(root) {
        return Err(PackageError::InvalidSelector(relative.to_path_buf()));
    }
    Ok(())
}

fn validate_confined_parent(root: &Path, relative: &Path) -> Result<(), PackageError> {
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    let canonical = fs::canonicalize(root.join(parent))
        .map_err(|source| io_error("canonicalize package file parent", parent, source))?;
    if !canonical.starts_with(root) {
        return Err(PackageError::InvalidSelector(relative.to_path_buf()));
    }
    Ok(())
}

fn reject_explicit_excluded_or_sensitive(relative: &Path) -> Result<(), PackageError> {
    for component in relative.components() {
        let Component::Normal(component) = component else {
            continue;
        };
        let value = component.to_str().ok_or_else(|| {
            PackageError::NonPortablePath(relative.to_string_lossy().into_owned())
        })?;
        if is_sensitive_component(value) {
            return Err(PackageError::SensitivePath(relative.to_path_buf()));
        }
        if is_excluded_directory(value) {
            return Err(PackageError::ExcludedPath(relative.to_path_buf()));
        }
    }
    Ok(())
}

fn relative_to_utf8(path: &Path) -> Result<String, PackageError> {
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err(PackageError::NonPortablePath(
                path.to_string_lossy().into_owned(),
            ));
        };
        components.push(
            component
                .to_str()
                .ok_or_else(|| PackageError::NonPortablePath(path.to_string_lossy().into_owned()))?
                .to_owned(),
        );
    }
    if components.is_empty() {
        return Err(PackageError::NonPortablePath(String::new()));
    }
    Ok(components.join("/"))
}

fn canonical_portable_components(path: &str) -> Result<Vec<String>, PackageError> {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') {
        return Err(PackageError::NonPortablePath(path.to_owned()));
    }
    let raw = path.split('/').collect::<Vec<_>>();
    if raw.len() > MAX_PATH_COMPONENTS {
        return Err(PackageError::NonPortablePath(path.to_owned()));
    }
    let mut canonical = Vec::with_capacity(raw.len());
    for component in raw {
        canonical.push(canonical_portable_component(component)?);
    }
    let bytes = canonical
        .iter()
        .map(String::len)
        .sum::<usize>()
        .saturating_add(canonical.len().saturating_sub(1));
    if bytes > MAX_PATH_BYTES {
        return Err(PackageError::NonPortablePath(path.to_owned()));
    }
    Ok(canonical)
}

fn canonical_portable_component(component: &str) -> Result<String, PackageError> {
    if component.is_empty()
        || component == "."
        || component == ".."
        || component.len() > MAX_PATH_COMPONENT_BYTES
        || component.contains(['/', '\\', ':'])
        || component.chars().any(|character| {
            character.is_control()
                || is_bidi_control(character)
                || matches!(character, '<' | '>' | '"' | '|' | '?' | '*')
        })
        || component.ends_with(['.', ' '])
        || is_reserved_component(component)
    {
        return Err(PackageError::NonPortablePath(component.to_owned()));
    }
    normalize_nfc(component).map_err(|()| PackageError::NonPortablePath(component.to_owned()))
}

fn normalize_nfc(component: &str) -> Result<String, ()> {
    let mut output = String::with_capacity(component.len());
    let mut segment = String::new();
    let flush = |segment: &mut String, output: &mut String| -> Result<(), ()> {
        if segment.is_empty() {
            return Ok(());
        }
        let normalized = Name::from_str(segment).map_err(|_| ())?;
        output.push_str(normalized.as_ref());
        segment.clear();
        Ok(())
    };
    for character in component.chars() {
        if matches!(character, '@' | '#' | '$') || character.is_whitespace() {
            flush(&mut segment, &mut output)?;
            output.push(character);
        } else {
            segment.push(character);
        }
    }
    flush(&mut segment, &mut output)?;
    Ok(output)
}

fn portable_collision_key(components: &[String]) -> String {
    components
        .join("/")
        .chars()
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_reserved_component(component: &str) -> bool {
    let basename = component.split('.').next().unwrap_or(component);
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "CLOCK$"]
        .iter()
        .any(|reserved| basename.eq_ignore_ascii_case(reserved))
    {
        return true;
    }
    if let (Some(prefix), Some(suffix)) = (basename.get(..3), basename.get(3..)) {
        let numbered = prefix.eq_ignore_ascii_case("COM") || prefix.eq_ignore_ascii_case("LPT");
        let reserved_digit = suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9');
        return numbered && (reserved_digit || matches!(suffix, "¹" | "²" | "³"));
    }
    false
}

fn is_bidi_control(character: char) -> bool {
    matches!(
        character,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

fn is_fixed_generated_file(path: &Path) -> bool {
    matches!(path.to_str(), Some(MANIFEST_PATH | VERIFICATION_LOCK_PATH))
}

fn is_excluded_directory(component: &str) -> bool {
    matches!(
        component.to_ascii_lowercase().as_str(),
        ".git"
            | ".hg"
            | ".svn"
            | ".cargo"
            | ".config"
            | ".iroha"
            | ".musubi"
            | ".gradle"
            | ".idea"
            | ".vscode"
            | "target"
            | "dist"
            | "build"
            | "out"
            | "coverage"
            | "node_modules"
    )
}

fn is_sensitive_component(component: &str) -> bool {
    let lower = component.to_ascii_lowercase();
    lower == ".env"
        || lower.starts_with(".env.")
        || matches!(
            lower.as_str(),
            ".ssh"
                | ".gnupg"
                | ".aws"
                | ".azure"
                | ".kube"
                | ".docker"
                | ".netrc"
                | ".npmrc"
                | ".pypirc"
                | "credentials"
                | "credentials.json"
                | "secrets"
                | "secrets.toml"
                | "id_rsa"
                | "id_dsa"
                | "id_ecdsa"
                | "id_ed25519"
        )
        || lower.contains("validator_secrets")
        || [".pem", ".key", ".p12", ".pfx", ".jks", ".keystore", ".kdbx"]
            .iter()
            .any(|suffix| lower.ends_with(suffix))
}

fn sensitive_content_marker(bytes: &[u8]) -> Option<&'static str> {
    const FIXED: &[(&[u8], &str)] = &[
        (b"-----BEGIN PRIVATE KEY-----", "PKCS#8 private key"),
        (b"-----BEGIN RSA PRIVATE KEY-----", "RSA private key"),
        (b"-----BEGIN EC PRIVATE KEY-----", "EC private key"),
        (
            b"-----BEGIN OPENSSH PRIVATE KEY-----",
            "OpenSSH private key",
        ),
        (
            b"-----BEGIN PGP PRIVATE KEY BLOCK-----",
            "OpenPGP private key",
        ),
        (b"AGE-SECRET-KEY-", "age private key"),
    ];
    for (needle, label) in FIXED {
        if contains_ascii_case_insensitive(bytes, needle) {
            return Some(label);
        }
    }
    if contains_aws_access_key(bytes) {
        return Some("AWS access key");
    }
    for line in bytes.split(|byte| *byte == b'\n') {
        if sensitive_assignment(line) {
            return Some("credential assignment");
        }
    }
    None
}

fn reject_consumer_only_lock_fields(value: &toml::Value) -> Result<(), PackageError> {
    const FORBIDDEN: &[&str] = &[
        "cache-path",
        "cache_path",
        "source-plan",
        "source_plan",
        "timestamp",
        "created-at",
        "created_at",
        "updated-at",
        "updated_at",
        "credential",
        "credentials",
        "provider-url",
        "provider_url",
        "bearer-token",
        "bearer_token",
        "stream-token",
        "stream_token",
        "private-key",
        "private_key",
    ];
    match value {
        toml::Value::Table(table) => {
            for (key, child) in table {
                if FORBIDDEN.iter().any(|forbidden| key == forbidden) {
                    return Err(PackageError::InvalidDocument {
                        document: VERIFICATION_LOCK_PATH,
                        reason: format!(
                            "consumer-only or secret-bearing field `{key}` is forbidden in a verification lock"
                        ),
                    });
                }
                reject_consumer_only_lock_fields(child)?;
            }
        }
        toml::Value::Array(values) => {
            for child in values {
                reject_consumer_only_lock_fields(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn sensitive_assignment(line: &[u8]) -> bool {
    const KEYS: &[&[u8]] = &[
        b"private_key",
        b"private-key",
        b"secret_key",
        b"secret-key",
        b"identity_private_key",
        b"bearer_token",
        b"bearer-token",
        b"stream_token",
        b"stream-token",
        b"aws_secret_access_key",
        b"mnemonic",
    ];
    let mut input = trim_ascii(line);
    if input.first() == Some(&b'"') {
        input = &input[1..];
    }
    let Some(key) = KEYS
        .iter()
        .find(|key| input.len() >= key.len() && input[..key.len()].eq_ignore_ascii_case(key))
    else {
        return false;
    };
    input = &input[key.len()..];
    input = trim_ascii_start(input);
    if input.first() == Some(&b'"') {
        input = trim_ascii_start(&input[1..]);
    }
    if !matches!(input.first(), Some(b'=' | b':')) {
        return false;
    }
    input = trim_ascii_start(&input[1..]);
    if matches!(input.first(), Some(b'"' | b'\'')) {
        input = &input[1..];
    }
    let token_len = input
        .iter()
        .take_while(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'_' | b'-' | b'=' | b'.')
        })
        .count();
    token_len >= 24
}

fn contains_aws_access_key(bytes: &[u8]) -> bool {
    bytes.windows(20).any(|window| {
        matches!(&window[..4], b"AKIA" | b"ASIA")
            && window[4..]
                .iter()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    })
}

fn contains_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(start, |index| index + 1);
    &bytes[start..end]
}

fn trim_ascii_start(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    &bytes[start..]
}

fn source_tree_material(files: &[PlannedFile]) -> Vec<u8> {
    let mut output = Vec::new();
    append_frame(&mut output, SOURCE_TREE_DOMAIN);
    output.extend_from_slice(
        &u32::try_from(files.len())
            .expect("Musubi V1 file bound fits u32")
            .to_be_bytes(),
    );
    for file in files {
        append_frame(&mut output, file.path.as_bytes());
        output.extend_from_slice(
            &u64::try_from(file.bytes.len())
                .expect("Musubi V1 source bound fits u64")
                .to_be_bytes(),
        );
        output.extend_from_slice(blake3::hash(&file.bytes).as_bytes());
    }
    output
}

fn descriptor_commitment_material(descriptor: &MusubiArtifactDescriptorV1) -> Vec<u8> {
    let mut output = Vec::new();
    append_frame(&mut output, ARTIFACT_DESCRIPTOR_DOMAIN);
    append_frame(&mut output, &descriptor.encode());
    output
}

fn bundle_commitment_material(
    semantic_release_manifest: &[u8],
    descriptor_material: &[u8],
    source_tree_material: &[u8],
    verification_lock: &[u8],
) -> Vec<u8> {
    let mut output = Vec::new();
    append_frame(&mut output, BUNDLE_DOMAIN);
    append_frame(&mut output, semantic_release_manifest);
    append_frame(&mut output, descriptor_material);
    append_frame(&mut output, source_tree_material);
    append_frame(&mut output, verification_lock);
    output
}

fn append_frame(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(
        &u64::try_from(bytes.len())
            .expect("bounded commitment material length fits u64")
            .to_be_bytes(),
    );
    output.extend_from_slice(bytes);
}

fn domain_digest(domain: &[u8], material: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(
        &u64::try_from(material.len())
            .expect("bounded commitment material length fits u64")
            .to_be_bytes(),
    );
    hasher.update(material);
    *hasher.finalize().as_bytes()
}

fn enforce_file_limit(count: usize) -> Result<(), PackageError> {
    if count > MAX_SOURCE_FILES {
        return Err(PackageError::TooManyFiles {
            count,
            maximum: MAX_SOURCE_FILES,
        });
    }
    Ok(())
}

fn enforce_source_limit(bytes: u64) -> Result<(), PackageError> {
    if bytes > MAX_SOURCE_BYTES {
        return Err(PackageError::SourceTooLarge {
            bytes,
            maximum: MAX_SOURCE_BYTES,
        });
    }
    Ok(())
}

fn enforce_chunk_limit(count: usize) -> Result<(), PackageError> {
    if count > MAX_SOURCE_CHUNKS {
        return Err(PackageError::TooManyChunks {
            count,
            maximum: MAX_SOURCE_CHUNKS,
        });
    }
    Ok(())
}

fn enforce_car_limit(bytes: u64) -> Result<(), PackageError> {
    if bytes > MAX_CAR_BYTES {
        return Err(PackageError::CarTooLarge {
            bytes,
            maximum: MAX_CAR_BYTES,
        });
    }
    Ok(())
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> PackageError {
    PackageError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

struct BoundedWriter {
    bytes: Vec<u8>,
    maximum: u64,
    attempted: u64,
    exceeded: bool,
}

impl BoundedWriter {
    fn new(maximum: u64) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
            attempted: 0,
            exceeded: false,
        }
    }
}

impl Write for BoundedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let additional = u64::try_from(buffer.len())
            .map_err(|_| io::Error::other("CAR write length does not fit u64"))?;
        self.attempted = u64::try_from(self.bytes.len())
            .ok()
            .and_then(|written| written.checked_add(additional))
            .unwrap_or(u64::MAX);
        if self.attempted > self.maximum {
            self.exceeded = true;
            return Err(io::Error::other("Musubi V1 CAR byte ceiling exceeded"));
        }
        self.bytes
            .try_reserve_exact(buffer.len())
            .map_err(io::Error::other)?;
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use iroha_data_model::{
        musubi::{
            MUSUBI_REGISTRY_VERSION_V1, MusubiAbiBindingV1, MusubiContentDigestV1,
            MusubiKotodamaEditionV1, MusubiPackageIdV1, MusubiPackageScopeV1, MusubiReleaseIdV1,
            MusubiReleaseMetadataV1, MusubiSemanticReleaseManifestV1, MusubiVerificationLockV1,
        },
        nexus::DataSpaceId,
    };
    use tempfile::tempdir;

    use super::*;

    const MANIFEST: &str = r#"
manifest-version = 1

[package]
name = "demo"
version = "1.0.0"
"#;
    const LOCK: &str = r#"
version = 1
schema = "musubi-lock"
packages = []
"#;

    fn base_layout(root: &Path) -> PackageLayout {
        let mut layout = PackageLayout::new(root);
        layout.set_library("src");
        layout
    }

    fn semantic_release() -> (MusubiSemanticReleaseManifestV1, MusubiVerificationLockV1) {
        let package = MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            "demo".parse().expect("package name"),
        );
        let release = MusubiReleaseIdV1::new(package, "1.0.0".parse().expect("version"));
        let lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: release.clone(),
            root_dependencies: Vec::new(),
            nodes: Vec::new(),
        };
        let semantic = MusubiSemanticReleaseManifestV1 {
            release,
            edition: MusubiKotodamaEditionV1::V1,
            abi: MusubiAbiBindingV1::new([8; 32]).expect("ABI"),
            dependencies: Vec::new(),
            exports: Vec::new(),
            interface_digest: MusubiContentDigestV1::new([9; 32]),
            metadata: MusubiReleaseMetadataV1::default(),
            verification_lock_digest: lock.digest(),
        };
        (semantic, lock)
    }

    #[test]
    fn plans_only_positive_files_in_byte_order() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir(temp.path().join("src")).expect("src");
        fs::write(temp.path().join("src/z.ko"), b"z").expect("z");
        fs::write(temp.path().join("src/a.ko"), b"a").expect("a");
        fs::write(temp.path().join("undeclared-secret.txt"), b"do not scan").expect("extra");

        let plan = plan_package(&base_layout(temp.path()), MANIFEST, LOCK).expect("plan");
        let paths = plan
            .files()
            .iter()
            .map(PlannedFile::path)
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            ["Musubi.lock", "Musubi.toml", "src/a.ko", "src/z.ko"]
        );
        assert!(!paths.contains(&"undeclared-secret.txt"));
    }

    #[test]
    fn injects_canonical_documents_instead_of_disk_copies() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir(temp.path().join("src")).expect("src");
        fs::write(temp.path().join("src/lib.ko"), b"fn main() {}").expect("source");
        fs::write(temp.path().join(MANIFEST_PATH), b"not the manifest").expect("disk manifest");
        fs::write(temp.path().join(VERIFICATION_LOCK_PATH), b"not the lock").expect("disk lock");
        let mut layout = base_layout(temp.path());
        layout.add_include(".");

        let plan = plan_package(&layout, MANIFEST, LOCK).expect("plan");
        assert_eq!(
            plan.canonical_manifest(),
            canonicalize_manifest_toml(MANIFEST).unwrap()
        );
        assert_eq!(
            plan.verification_lock(),
            normalize_verification_lock_toml(LOCK).unwrap()
        );
    }

    #[test]
    fn canonical_toml_is_order_independent_and_requires_v1_markers() {
        let left = canonicalize_manifest_toml("manifest-version=1\nname='x'\n").expect("left");
        let right =
            canonicalize_manifest_toml("name = 'x'\nmanifest-version = 1\n").expect("right");
        assert_eq!(left, right);
        assert!(left.ends_with(b"\n"));
        assert!(matches!(
            canonicalize_manifest_toml("manifest-version=2"),
            Err(PackageError::InvalidDocument { .. })
        ));
        assert!(matches!(
            normalize_verification_lock_toml("version=3"),
            Err(PackageError::InvalidDocument { .. })
        ));
        assert!(matches!(
            normalize_verification_lock_toml(
                "schema='musubi-lock'\nversion=1\n[[package]]\ncache_path='/tmp/source'\n"
            ),
            Err(PackageError::InvalidDocument { .. })
        ));
    }

    #[test]
    fn excludes_generated_roots_but_rejects_explicit_selection() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("src/target")).expect("target");
        fs::write(temp.path().join("src/target/generated.ko"), b"generated").expect("generated");
        fs::write(temp.path().join("src/lib.ko"), b"source").expect("source");
        let plan = plan_package(&base_layout(temp.path()), MANIFEST, LOCK).expect("plan");
        assert!(
            !plan
                .files()
                .iter()
                .any(|file| file.path().contains("target"))
        );

        let mut explicit = PackageLayout::new(temp.path());
        explicit.add_include("src/target");
        assert!(matches!(
            plan_package(&explicit, MANIFEST, LOCK),
            Err(PackageError::ExcludedPath(_))
        ));
    }

    #[test]
    fn rejects_traversal_absolute_and_portable_reserved_paths() {
        let temp = tempdir().expect("tempdir");
        let mut traversal = PackageLayout::new(temp.path());
        traversal.add_include("../outside");
        assert!(matches!(
            plan_package(&traversal, MANIFEST, LOCK),
            Err(PackageError::InvalidSelector(_))
        ));

        let mut absolute = PackageLayout::new(temp.path());
        absolute.add_include(temp.path());
        assert!(matches!(
            plan_package(&absolute, MANIFEST, LOCK),
            Err(PackageError::InvalidSelector(_))
        ));

        fs::create_dir(temp.path().join("src")).expect("src");
        fs::write(temp.path().join("src/CON.ko"), b"reserved").expect("reserved");
        assert!(matches!(
            plan_package(&base_layout(temp.path()), MANIFEST, LOCK),
            Err(PackageError::NonPortablePath(_))
        ));
    }

    #[test]
    fn rejects_case_and_unicode_equivalent_collisions() {
        // Exercise the logical-path index directly: common macOS filesystems collapse these
        // names before `read_dir`, while the portable package check must behave identically on
        // case-sensitive and case-insensitive hosts.
        let case = tempdir().expect("tempdir");
        let mut collector = Collector::new(case.path().to_path_buf());
        collector
            .insert_virtual("src/Foo.ko", b"one".to_vec())
            .expect("first case spelling");
        assert!(matches!(
            collector.insert_virtual("src/foo.ko", b"two".to_vec()),
            Err(PackageError::PathCollision { .. })
        ));

        let unicode = tempdir().expect("tempdir");
        let mut collector = Collector::new(unicode.path().to_path_buf());
        collector
            .insert_virtual("src/caf\u{e9}.ko", b"one".to_vec())
            .expect("first Unicode spelling");
        assert!(matches!(
            collector.insert_virtual("src/cafe\u{301}.ko", b"two".to_vec()),
            Err(PackageError::PathCollision { .. })
        ));
    }

    #[test]
    fn normalizes_a_single_decomposed_unicode_path() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir(temp.path().join("src")).expect("src");
        fs::write(temp.path().join("src/cafe\u{301}.ko"), b"source").expect("source");
        let plan = plan_package(&base_layout(temp.path()), MANIFEST, LOCK).expect("plan");
        assert!(
            plan.files()
                .iter()
                .any(|file| file.path() == "src/caf\u{e9}.ko")
        );
    }

    #[test]
    fn rejects_sensitive_paths_and_contents_without_echoing_secrets() {
        let path_case = tempdir().expect("tempdir");
        fs::create_dir(path_case.path().join("src")).expect("src");
        fs::write(path_case.path().join("src/id_ed25519"), b"secret").expect("key");
        assert!(matches!(
            plan_package(&base_layout(path_case.path()), MANIFEST, LOCK),
            Err(PackageError::SensitivePath(_))
        ));

        let content_case = tempdir().expect("tempdir");
        fs::create_dir(content_case.path().join("src")).expect("src");
        fs::write(
            content_case.path().join("src/lib.ko"),
            b"-----BEGIN PRIVATE KEY-----\nvery-secret-value",
        )
        .expect("source");
        let error = plan_package(&base_layout(content_case.path()), MANIFEST, LOCK)
            .expect_err("private key must fail");
        assert!(matches!(error, PackageError::SensitiveContent { .. }));
        assert!(!error.to_string().contains("very-secret-value"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_hardlinks_special_files_and_non_utf8_paths() {
        use std::{
            ffi::OsString,
            os::unix::{ffi::OsStringExt as _, fs::symlink, net::UnixListener},
        };

        let symlinks = tempdir().expect("tempdir");
        fs::create_dir(symlinks.path().join("src")).expect("src");
        fs::write(symlinks.path().join("outside.ko"), b"outside").expect("outside");
        symlink("../outside.ko", symlinks.path().join("src/link.ko")).expect("symlink");
        assert!(matches!(
            plan_package(&base_layout(symlinks.path()), MANIFEST, LOCK),
            Err(PackageError::Symlink(_))
        ));

        let hardlinks = tempdir().expect("tempdir");
        fs::create_dir(hardlinks.path().join("src")).expect("src");
        fs::write(hardlinks.path().join("src/a.ko"), b"source").expect("source");
        fs::hard_link(
            hardlinks.path().join("src/a.ko"),
            hardlinks.path().join("src/b.ko"),
        )
        .expect("hardlink");
        assert!(matches!(
            plan_package(&base_layout(hardlinks.path()), MANIFEST, LOCK),
            Err(PackageError::Hardlink(_))
        ));

        let special = tempdir().expect("tempdir");
        fs::create_dir(special.path().join("src")).expect("src");
        let _socket = UnixListener::bind(special.path().join("src/socket")).expect("socket");
        assert!(matches!(
            plan_package(&base_layout(special.path()), MANIFEST, LOCK),
            Err(PackageError::SpecialFile(_))
        ));

        let non_utf8 = tempdir().expect("tempdir");
        fs::create_dir(non_utf8.path().join("src")).expect("src");
        if fs::write(
            non_utf8
                .path()
                .join("src")
                .join(OsString::from_vec(vec![0xff, b'.', b'k', b'o'])),
            b"source",
        )
        .is_err()
        {
            // Some filesystems (including the default macOS filesystem) reject a raw
            // non-UTF-8 directory entry before Musubi can observe it.
            return;
        }
        assert!(matches!(
            plan_package(&base_layout(non_utf8.path()), MANIFEST, LOCK),
            Err(PackageError::NonPortablePath(_))
        ));
    }

    #[test]
    fn commitments_and_car_are_deterministic() {
        let left = tempdir().expect("tempdir");
        let right = tempdir().expect("tempdir");
        for root in [left.path(), right.path()] {
            fs::create_dir(root.join("src")).expect("src");
        }
        fs::write(left.path().join("src/b.ko"), b"b").expect("b");
        fs::write(left.path().join("src/a.ko"), b"a").expect("a");
        fs::write(right.path().join("src/a.ko"), b"a").expect("a");
        fs::write(right.path().join("src/b.ko"), b"b").expect("b");

        let left_plan = plan_package(&base_layout(left.path()), MANIFEST, LOCK).expect("left plan");
        let right_plan =
            plan_package(&base_layout(right.path()), MANIFEST, LOCK).expect("right plan");
        let (semantic, lock) = semantic_release();
        let left_commitments = left_plan
            .commitment_materials(&semantic, &lock)
            .expect("left commitments");
        let right_commitments = right_plan
            .commitment_materials(&semantic, &lock)
            .expect("right commitments");
        assert_eq!(left_commitments, right_commitments);
        assert_eq!(left_commitments.descriptor().version, 1);
        assert_eq!(
            left_commitments
                .descriptor()
                .semantic_release_manifest_digest,
            semantic.semantic_digest()
        );
        assert_eq!(
            left_commitments.descriptor().verification_lock_digest,
            lock.digest()
        );
        let encoded_descriptor = left_commitments.descriptor().encode();
        let decoded_descriptor =
            MusubiArtifactDescriptorV1::decode(&mut encoded_descriptor.as_slice())
                .expect("decode descriptor");
        assert_eq!(&decoded_descriptor, left_commitments.descriptor());

        let left_car = left_plan.into_car(&semantic, &lock).expect("left CAR");
        let right_car = right_plan.into_car(&semantic, &lock).expect("right CAR");
        assert_eq!(left_car.bytes(), right_car.bytes());
        assert_eq!(left_car.commitments(), &left_commitments);
        assert_eq!(left_car.source_file_count(), 4);
        assert_eq!(left_car.plan().files.len(), 7);
        assert!(
            left_car
                .plan()
                .files
                .iter()
                .any(|file| file.path.join("/") == BUNDLE_RELEASE_PATH)
        );
        assert!(
            left_car
                .plan()
                .files
                .iter()
                .any(|file| file.path.join("/") == BUNDLE_DESCRIPTOR_PATH)
        );
        assert!(
            left_car
                .plan()
                .files
                .iter()
                .any(|file| file.path.join("/") == BUNDLE_VERIFICATION_LOCK_PATH)
        );
        assert!(left_car.bytes().len() as u64 <= MAX_CAR_BYTES);
        assert!(left_car.plan().chunks.len() <= MAX_SOURCE_CHUNKS);
    }

    #[test]
    fn semantic_bundle_rejects_a_different_exact_lock() {
        let temp = tempdir().expect("tempdir");
        fs::create_dir(temp.path().join("src")).expect("src");
        fs::write(temp.path().join("src/lib.ko"), b"fn demo() {}").expect("source");
        let plan = plan_package(&base_layout(temp.path()), MANIFEST, LOCK).expect("plan");
        let (semantic, mut lock) = semantic_release();
        lock.version = 2;
        assert!(matches!(
            plan.commitment_materials(&semantic, &lock),
            Err(PackageError::InvalidBundleBinding(_))
        ));
    }

    #[test]
    fn limit_guards_report_exact_v1_ceilings() {
        assert!(enforce_file_limit(MAX_SOURCE_FILES).is_ok());
        assert!(matches!(
            enforce_file_limit(MAX_SOURCE_FILES + 1),
            Err(PackageError::TooManyFiles {
                maximum: MAX_SOURCE_FILES,
                ..
            })
        ));
        assert!(enforce_source_limit(MAX_SOURCE_BYTES).is_ok());
        assert!(matches!(
            enforce_source_limit(MAX_SOURCE_BYTES + 1),
            Err(PackageError::SourceTooLarge {
                maximum: MAX_SOURCE_BYTES,
                ..
            })
        ));
        assert!(enforce_chunk_limit(MAX_SOURCE_CHUNKS).is_ok());
        assert!(matches!(
            enforce_chunk_limit(MAX_SOURCE_CHUNKS + 1),
            Err(PackageError::TooManyChunks {
                maximum: MAX_SOURCE_CHUNKS,
                ..
            })
        ));
        assert!(enforce_car_limit(MAX_CAR_BYTES).is_ok());
        assert!(matches!(
            enforce_car_limit(MAX_CAR_BYTES + 1),
            Err(PackageError::CarTooLarge {
                maximum: MAX_CAR_BYTES,
                ..
            })
        ));
    }
}
