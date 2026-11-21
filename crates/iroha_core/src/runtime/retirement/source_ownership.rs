//! Offline validation of a reviewed, explicitly scoped native source inventory.
//!
//! Review supplies the ownership assertions; neither filenames nor dependency scans infer them.
//! The independently selected manifest digest pins that review. This module checks its shape,
//! exact source preimages and requested native owners, and refuses every retained dependency.
//! A checked inventory is not retirement evidence or deletion authorization. It authenticates no
//! root, receipt, release build or replay report and never deletes or writes a source file.
//! `source_candidate` joins this owner with authenticated parent/child alias coverage.
//! TODO(N14): qualify an actual same-candidate restart from each root's oldest accepted
//! checkpoint, with the reviewed source removed, before issuing a release verdict.

use std::{io, path::Path};

use iroha_data_model::runtime::definition::ImplementationCodeV1;
use iroha_fs::{FileIdentity, FileSnapshot, ReaderDirectory, RetainedFile};
use norito::{
    Decode, Encode, NoritoSchema,
    json::{JsonDeserialize, JsonSerialize},
};
use sha2::{Digest as _, Sha256};

/// Maximum source units in one reviewed, scoped inventory.
pub const MAX_SOURCE_UNITS: usize = 256;
/// Maximum native or retained owners of one source unit.
pub const MAX_SOURCE_OWNERS: usize = 64;
/// Maximum UTF-8 bytes in a canonical repository-relative source path.
pub const MAX_SOURCE_PATH_BYTES: usize = 1024;
/// Maximum complete canonical inventory frame, independent of source-file read limits.
pub const MAX_MANIFEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_PATH_COMPONENTS: usize = 32;
const MAX_OWNER_LABEL_BYTES: usize = 256;
const MANIFEST_DOMAIN: &[u8] = b"iroha:runtime:reviewed-source-ownership:v1\0";

/// A dependency that must survive removal of its former instruction handler.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    NoritoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito_schema(name = "iroha_core::runtime::source_ownership::RetainedRoleV1")]
#[norito(tag = "role", content = "content")]
pub enum RetainedRoleV1 {
    /// Stored-value or instruction decoding remains available.
    Decoder,
    /// Snapshot or checkpoint restoration still consumes this source.
    CheckpointReader,
    /// An existing proof format still needs verification.
    ProofVerifier,
    /// Certificates from genesis through the startup floor remain verifiable.
    CertificateVerifier,
    /// Authoritative State layout, encoding or storage access remains installed.
    StateStorage,
    /// Immutable descriptors and permanent retirement records remain readable.
    DescriptorOrReceipt,
    /// A kernel, service or host profile still owns this source.
    RuntimeProfile,
    /// A migration still owns this source, irrespective of handler retirement.
    Migration,
    /// Another explicitly reviewed retained dependency; never an exemption.
    Other,
}

/// One reviewed retained dependency. Every role prevents removing its source unit.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    NoritoSchema,
    JsonSerialize,
    JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_core::runtime::source_ownership::RetainedUseV1")]
pub struct RetainedUseV1 {
    /// The retained function of this source.
    pub role: RetainedRoleV1,
    /// Exact reviewed owner label, for example its immutable definition identity.
    pub owner: String,
}

/// One whole source file and all owners asserted by the independently pinned review.
#[derive(
    Debug, Clone, PartialEq, Eq, Encode, Decode, NoritoSchema, JsonSerialize, JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_core::runtime::source_ownership::SourceUnitV1")]
pub struct SourceUnitV1 {
    /// Canonical relative path, using `/`, with no traversal, aliases or platform prefixes.
    pub path: String,
    /// Exact reviewed byte extent; this is checked before any content read.
    pub bytes: u64,
    /// SHA-256 of the complete original file, without normalization.
    pub sha256: [u8; 32],
    /// Sorted, unique exact native module/reference-closure identities owning this source.
    pub native_owners: Vec<ImplementationCodeV1>,
    /// Sorted, unique dependencies which must survive all proposed handler removals.
    pub retained_uses: Vec<RetainedUseV1>,
}

/// Review data for exactly the named source units; it makes no whole-repository ownership claim.
#[derive(
    Debug, Clone, PartialEq, Eq, Encode, Decode, NoritoSchema, JsonSerialize, JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_core::runtime::source_ownership::SourceManifestV1")]
pub struct SourceManifestV1 {
    /// Exact local evidence format; only `1` is accepted.
    pub version: u16,
    /// Sorted, unique source paths in the reviewed scope.
    pub units: Vec<SourceUnitV1>,
}

/// An untrusted removal proposal, never a claim that any owner has retired.
#[derive(
    Debug, Clone, PartialEq, Eq, Encode, Decode, NoritoSchema, JsonSerialize, JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_core::runtime::source_ownership::SourceRemovalRequestV1")]
pub struct SourceRemovalRequestV1 {
    /// Sorted, unique paths which must all occur in the reviewed inventory.
    pub paths: Vec<String>,
    /// Exact union of the requested files' native owners; all still need retirement evidence.
    pub native_owners: Vec<ImplementationCodeV1>,
}

/// Explicit local I/O allowances, separate from protocol and manifest identities.
#[derive(Debug, Clone, Copy)]
pub struct SourceReadLimits {
    /// Largest reviewed source-file extent this invocation may inspect and recheck.
    pub max_file_bytes: u64,
    /// Largest sum of reviewed source extents, including originals retained for rechecking.
    pub max_total_bytes: u64,
}

/// Source-review validation failure; no variant grants a partial removal verdict.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A bounded manifest, request or ownership invariant was not satisfied.
    #[error("source ownership: {0}")]
    Invalid(&'static str),
    /// The independently reviewed manifest identity differs.
    #[error("source ownership: reviewed manifest digest differs")]
    ReviewMismatch,
    /// A requested source still has a retained dependency.
    #[error("source ownership: {path} retains {role:?}")]
    Retained {
        /// Reviewed source path that cannot be removed.
        path: String,
        /// Retained dependency category.
        role: RetainedRoleV1,
    },
    /// The exact source length, digest or native metadata changed.
    #[error("source ownership: original preimage changed at {0}")]
    PreimageChanged(String),
    /// Original source acquisition, custody or read failure.
    #[error("source ownership I/O at {path}: {source}")]
    Io {
        /// Relative reviewed path, or `.` for the independently selected root.
        path: String,
        /// Original operating-system failure, including missing files and unsafe custody.
        #[source]
        source: io::Error,
    },
    /// Original canonical encoder refusal; never converted into an authorization result.
    #[error("source ownership: canonical manifest encoding refused")]
    Codec(#[source] norito::core::DecodeAttemptError),
    /// Original local allocation refusal while retaining file owners.
    #[error("source ownership: original owner allocation refused")]
    Allocator(#[source] std::collections::TryReserveError),
}

fn require(condition: bool, reason: &'static str) -> Result<(), Error> {
    if condition {
        Ok(())
    } else {
        Err(Error::Invalid(reason))
    }
}

fn canonical_path(path: &str) -> Result<(), Error> {
    require(
        !path.is_empty() && path.len() <= MAX_SOURCE_PATH_BYTES,
        "source path bound",
    )?;
    require(
        path.bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte)),
        "source path characters",
    )?;
    let mut count = 0;
    for part in path.split('/') {
        require(
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with('.')
                && part.len() <= 255,
            "source path component",
        )?;
        count += 1;
    }
    require(count <= MAX_PATH_COMPONENTS, "source path depth")
}

fn native_owner(owner: &ImplementationCodeV1) -> Result<(), Error> {
    let ImplementationCodeV1::Native { module, .. } = owner else {
        return Err(Error::Invalid(
            "source owner is not an exact native identity",
        ));
    };
    require(
        !module.is_empty()
            && module.len() <= MAX_OWNER_LABEL_BYTES
            && module.bytes().all(|byte| byte.is_ascii_graphic()),
        "native owner label",
    )
}

fn validate_manifest(manifest: &SourceManifestV1) -> Result<(), Error> {
    require(manifest.version == 1, "unknown manifest format")?;
    require(
        !manifest.units.is_empty() && manifest.units.len() <= MAX_SOURCE_UNITS,
        "source unit count",
    )?;
    require(
        manifest
            .units
            .windows(2)
            .all(|pair| pair[0].path < pair[1].path),
        "source paths are not sorted and unique",
    )?;
    for unit in &manifest.units {
        canonical_path(&unit.path)?;
        require(
            unit.native_owners.len() <= MAX_SOURCE_OWNERS
                && unit.retained_uses.len() <= MAX_SOURCE_OWNERS,
            "source owner count",
        )?;
        require(
            !unit.native_owners.is_empty() || !unit.retained_uses.is_empty(),
            "source has no reviewed owner",
        )?;
        require(
            unit.native_owners.windows(2).all(|pair| pair[0] < pair[1])
                && unit.retained_uses.windows(2).all(|pair| pair[0] < pair[1]),
            "source owners are not sorted and unique",
        )?;
        for owner in &unit.native_owners {
            native_owner(owner)?;
        }
        for retained in &unit.retained_uses {
            require(
                !retained.owner.is_empty()
                    && retained.owner.len() <= MAX_OWNER_LABEL_BYTES
                    && retained.owner.bytes().all(|byte| byte.is_ascii_graphic()),
                "retained owner label",
            )?;
        }
    }
    Ok(())
}

/// Compute the canonical review identity without reading or endorsing any source.
/// The caller must obtain the expected digest from its independent review, not from the same
/// untrusted manifest it is about to validate. This helper does not establish review authority.
/// # Errors
/// Malformed or oversized inventory, or the original canonical encoder refusal.
pub fn manifest_digest(manifest: &SourceManifestV1) -> Result<[u8; 32], Error> {
    validate_manifest(manifest)?;
    let bytes = norito::core::classify_decode_attempt(|| {
        if norito::canonical_frame_len(manifest)? > MAX_MANIFEST_BYTES {
            return Err(norito::Error::Message(
                "source manifest exceeds its bound".into(),
            ));
        }
        norito::encode_canonical(manifest)
    })
    .map_err(Error::Codec)?;
    let mut digest = Sha256::new();
    digest.update(MANIFEST_DOMAIN);
    digest.update(bytes);
    Ok(digest.finalize().into())
}

fn validate_request(
    manifest: &SourceManifestV1,
    request: &SourceRemovalRequestV1,
) -> Result<(), Error> {
    require(
        !request.paths.is_empty()
            && request.paths.len() <= MAX_SOURCE_UNITS
            && !request.native_owners.is_empty()
            && request.native_owners.len() <= MAX_SOURCE_UNITS * MAX_SOURCE_OWNERS,
        "removal request count",
    )?;
    require(
        request.paths.windows(2).all(|pair| pair[0] < pair[1])
            && request
                .native_owners
                .windows(2)
                .all(|pair| pair[0] < pair[1]),
        "removal request is not sorted and unique",
    )?;
    for owner in &request.native_owners {
        native_owner(owner)?;
    }
    for path in &request.paths {
        canonical_path(path)?;
        let index = manifest
            .units
            .binary_search_by(|unit| unit.path.cmp(path))
            .map_err(|_| Error::Invalid("requested source is outside the reviewed inventory"))?;
        let unit = &manifest.units[index];
        if let Some(retained) = unit.retained_uses.first() {
            return Err(Error::Retained {
                path: path.clone(),
                role: retained.role,
            });
        }
        require(
            !unit.native_owners.is_empty(),
            "requested source has no native owner",
        )?;
        require(
            unit.native_owners
                .iter()
                .all(|owner| request.native_owners.binary_search(owner).is_ok()),
            "requested source has another native owner",
        )?;
    }
    for owner in &request.native_owners {
        require(
            manifest.units.iter().any(|unit| {
                request.paths.binary_search(&unit.path).is_ok()
                    && unit.native_owners.binary_search(owner).is_ok()
            }),
            "requested native owner owns no requested source",
        )?;
    }
    Ok(())
}

fn io_error(path: &str, source: io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}

fn open_source(root: &ReaderDirectory, path: &str) -> Result<RetainedFile, Error> {
    let mut parts = path.split('/').peekable();
    let mut directory = None;
    while let Some(part) = parts.next() {
        let parent = directory.as_ref().unwrap_or(root);
        if parts.peek().is_none() {
            return parent
                .open_retained_regular(part)
                .map_err(|error| io_error(path, error));
        }
        directory = Some(
            parent
                .open_child(part)
                .map_err(|error| io_error(path, error))?,
        );
    }
    Err(Error::Invalid("source path has no leaf"))
}

struct CheckedSource {
    file: RetainedFile,
    identity: FileIdentity,
    snapshot: FileSnapshot,
}

fn check_preimage(source: &CheckedSource, unit: &SourceUnitV1) -> Result<(), Error> {
    let before = source
        .file
        .snapshot()
        .map_err(|error| io_error(&unit.path, error))?;
    if before != source.snapshot
        || source
            .file
            .file()
            .metadata()
            .map_err(|error| io_error(&unit.path, error))?
            .len()
            != unit.bytes
    {
        return Err(Error::PreimageChanged(unit.path.clone()));
    }
    let mut digest = Sha256::new();
    let mut offset = 0_u64;
    let mut buffer = [0_u8; 8192];
    loop {
        let remaining = unit.bytes.saturating_sub(offset);
        let extent = remaining.min((buffer.len() - 1) as u64) as usize + 1;
        let read = iroha_fs::read_at(source.file.file(), &mut buffer[..extent], offset)
            .map_err(|error| io_error(&unit.path, error))?;
        if read == 0 {
            break;
        }
        offset = offset
            .checked_add(read as u64)
            .ok_or(Error::Invalid("source extent overflow"))?;
        if offset > unit.bytes {
            return Err(Error::PreimageChanged(unit.path.clone()));
        }
        digest.update(&buffer[..read]);
    }
    let after = source
        .file
        .snapshot()
        .map_err(|error| io_error(&unit.path, error))?;
    let actual: [u8; 32] = digest.finalize().into();
    if offset != unit.bytes || actual != unit.sha256 || after != before {
        return Err(Error::PreimageChanged(unit.path.clone()));
    }
    Ok(())
}

/// Checked source preimages and ownership proposal, retaining only read-only native file custody.
///
/// This type is deliberately neither serializable nor constructible from a report. Its success
/// says nothing about retirement, complete dependency reachability, build provenance or replay.
/// All required native owners still need authenticated coverage. Recheck immediately before any
/// future consumer binds this observation into a candidate-specific verdict.
pub struct CheckedSourceManifest<'a> {
    root: ReaderDirectory,
    manifest: &'a SourceManifestV1,
    request: &'a SourceRemovalRequestV1,
    digest: [u8; 32],
    sources: Vec<CheckedSource>,
}

impl CheckedSourceManifest<'_> {
    /// Independently pinned review identity, unchanged by local read allowances.
    pub const fn manifest_digest(&self) -> [u8; 32] {
        self.digest
    }
    /// Requested source paths; these have not been removed or authorized for removal.
    pub fn requested_paths(&self) -> &[String] {
        &self.request.paths
    }
    /// Exact native owners whose authenticated every-root coverage remains required.
    pub fn required_native_owners(&self) -> &[ImplementationCodeV1] {
        &self.request.native_owners
    }
    /// Re-read every reviewed original, preserving its initial path, metadata and digest.
    /// # Errors
    /// Missing/replaced originals, changed bytes or custody, or original I/O failure.
    pub fn recheck_preimages(&self) -> Result<(), Error> {
        self.root
            .revalidate()
            .map_err(|error| io_error(".", error))?;
        for (source, unit) in self.sources.iter().zip(&self.manifest.units) {
            check_preimage(source, unit)?;
        }
        self.root.revalidate().map_err(|error| io_error(".", error))
    }
}

/// Validate an independently reviewed source scope and retain every exact original file.
///
/// All ownership and byte-count preflight completes before opening source files. No filesystem
/// discovery, manifest self-approval, root-history inference, source deletion or report-based
/// authorization occurs. Unknown and shared retained uses fail closed.
/// # Errors
/// Any malformed proposal, review mismatch, retained dependency, undeclared native owner,
/// local extent limit, nonregular/link/changed file, digest mismatch, or original resource error.
pub fn check_sources<'a>(
    root: &Path,
    manifest: &'a SourceManifestV1,
    reviewed_digest: [u8; 32],
    request: &'a SourceRemovalRequestV1,
    limits: SourceReadLimits,
) -> Result<CheckedSourceManifest<'a>, Error> {
    let digest = manifest_digest(manifest)?;
    if digest != reviewed_digest {
        return Err(Error::ReviewMismatch);
    }
    validate_request(manifest, request)?;
    require(
        limits.max_file_bytes > 0 && limits.max_total_bytes > 0,
        "source read limits must be nonzero",
    )?;
    let mut total = 0_u64;
    for unit in &manifest.units {
        require(
            unit.bytes <= limits.max_file_bytes,
            "source exceeds individual read limit",
        )?;
        total = total
            .checked_add(unit.bytes)
            .ok_or(Error::Invalid("total source extent overflow"))?;
        require(
            total <= limits.max_total_bytes,
            "source exceeds total read limit",
        )?;
    }
    let root = ReaderDirectory::open(root).map_err(|error| io_error(".", error))?;
    let mut sources: Vec<CheckedSource> = Vec::new();
    sources
        .try_reserve_exact(manifest.units.len())
        .map_err(Error::Allocator)?;
    for unit in &manifest.units {
        let file = open_source(&root, &unit.path)?;
        let identity = file
            .identity()
            .map_err(|error| io_error(&unit.path, error))?;
        require(
            !sources.iter().any(|source| source.identity == identity),
            "different source paths alias one physical file",
        )?;
        let snapshot = file
            .snapshot()
            .map_err(|error| io_error(&unit.path, error))?;
        let source = CheckedSource {
            file,
            identity,
            snapshot,
        };
        check_preimage(&source, unit)?;
        sources.push(source);
    }
    let checked = CheckedSourceManifest {
        root,
        manifest,
        request,
        digest,
        sources,
    };
    checked.recheck_preimages()?;
    Ok(checked)
}

#[cfg(test)]
#[path = "source_ownership/tests.rs"]
mod tests;
