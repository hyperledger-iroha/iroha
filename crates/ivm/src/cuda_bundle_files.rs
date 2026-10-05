//! Build-host and test custody for original authenticated CUDA bundle files.
//!
//! Runtime admission remains the sole borrowed canonical provenance relation;
//! production library builds never compile this filesystem adapter.

#[cfg(feature = "cuda")]
use crate::cuda_build_policy;
use crate::cuda_provenance::{
    BorrowedCudaReader, MAX_MANIFEST_BYTES, MAX_PTX_BYTES, ProvenanceRefusal, is_lower_sha256,
};
#[cfg(feature = "cuda")]
use std::error::Error;
use std::{fs, io::Read as _, path::Path};

pub(super) const MANIFEST_NAME: &str = "provenance.v1";
pub(super) const MANIFEST_SIGNATURE_NAME: &str = "provenance.v1.sig";
pub(super) const MANIFEST_PUBLIC_KEY_NAME: &str = "provenance.v1.pub";

/// Bundle bytes that passed signed provenance and exact source/PTX hashing.
#[derive(Debug)]
pub(super) struct VerifiedCudaBundle {
    /// PTX bytes in the pinned family order, kept live through installation.
    pub artifacts: Vec<Vec<u8>>,
    /// Exact admitted source bytes retained until installation.
    pub sources: Vec<Vec<u8>>,
    /// Exact authenticated inputs retained until installation.
    pub manifest: Vec<u8>,
    pub public_key: [u8; 32],
    pub signature: [u8; 64],
    /// Signed CUDA toolkit-image digest for independent release-runner comparison.
    pub cuda_image_sha256: String,
    /// Exact signed `nvcc --version` output digest.
    #[cfg(test)]
    pub nvcc_version_sha256: String,
    /// Exact signed compiler flags, excluding source and output paths.
    #[cfg(test)]
    pub nvcc_flags: String,
    /// Signed GPU code-generation target.
    #[cfg(test)]
    pub target_profile: String,
}

pub(super) fn same_regular_inode(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    if !before.file_type().is_file()
        || !after.file_type().is_file()
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        before.dev() == after.dev()
            && before.ino() == after.ino()
            && before.ctime() == after.ctime()
            && before.ctime_nsec() == after.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
    target_os = "linux",
    target_os = "android"
))]
fn no_follow_nonblocking_flags() -> i32 {
    // Match the platform file-admission flags in state_overlay_fs. This module
    // also belongs to build.rs, so it cannot depend on the runtime overlay owner.
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))]
    {
        0x100 | 0x4
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let no_follow = if cfg!(any(
            target_arch = "arm",
            target_arch = "aarch64",
            target_arch = "m68k",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        )) {
            0x8000
        } else {
            0x0002_0000
        };
        let nonblocking = if cfg!(any(target_arch = "mips", target_arch = "mips64")) {
            0x80
        } else if cfg!(any(target_arch = "sparc", target_arch = "sparc64")) {
            0x4000
        } else {
            0x800
        };
        no_follow | nonblocking
    }
}

#[cfg(all(
    unix,
    not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_os = "linux",
        target_os = "android"
    ))
))]
pub(super) fn open_regular_file(_path: &Path) -> std::io::Result<fs::File> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "CUDA provenance requires no-follow nonblocking file admission",
    ))
}

#[cfg(any(
    not(unix),
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
    target_os = "linux",
    target_os = "android"
))]
pub(super) fn open_regular_file(path: &Path) -> std::io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(no_follow_nonblocking_flags());
    }
    options.open(path)
}

pub(super) fn read_regular_file(path: &Path, maximum: Option<usize>) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    if !metadata.file_type().is_file() {
        return Err(format!("{} must be a regular file", path.display()));
    }
    if maximum.is_some_and(|maximum| metadata.len() > maximum as u64) {
        return Err(format!("{} exceeds its byte limit", path.display()));
    }
    let mut file = open_regular_file(path)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let opened = file
        .metadata()
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    if !same_regular_inode(&metadata, &opened) {
        return Err(format!("{} changed before read", path.display()));
    }
    let limit = opened
        .len()
        .checked_add(1)
        .ok_or_else(|| format!("{} has an invalid inode size", path.display()))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(limit)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if maximum.is_some_and(|maximum| bytes.len() > maximum) {
        return Err(format!("{} grew beyond its byte limit", path.display()));
    }
    let after = file
        .metadata()
        .map_err(|error| format!("cannot recheck {}: {error}", path.display()))?;
    let after_path = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot recheck {}: {error}", path.display()))?;
    if bytes.len() as u64 != opened.len()
        || !same_regular_inode(&opened, &after)
        || !same_regular_inode(&opened, &after_path)
    {
        return Err(format!("{} changed while read", path.display()));
    }
    Ok(bytes)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BundlePresence {
    Absent,
    Complete,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PresenceRefusal {
    UnreviewedMaterial,
    IncompleteReviewedMaterial,
}
impl std::fmt::Display for PresenceRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::UnreviewedMaterial => {
                "supplied CUDA bundle material has no source-owned approval"
            }
            Self::IncompleteReviewedMaterial => {
                "source-approved CUDA bundle must contain all thirteen original inputs"
            }
        })
    }
}
impl std::error::Error for PresenceRefusal {}

/// Metadata presence is not cryptographic admission; every supplied row needs
/// the sole canonical verifier before a private owner can be emitted.
pub(crate) fn bundle_presence(
    reviewed: bool,
    present: &[bool; 13],
) -> Result<BundlePresence, PresenceRefusal> {
    if !reviewed {
        return if present.iter().any(|entry| *entry) {
            Err(PresenceRefusal::UnreviewedMaterial)
        } else {
            Ok(BundlePresence::Absent)
        };
    }
    if present.iter().all(|entry| *entry) {
        Ok(BundlePresence::Complete)
    } else {
        Err(PresenceRefusal::IncompleteReviewedMaterial)
    }
}

/// File custody supplies retained originals to the same borrowed canonical walk.
/// No artifact/public-key supplied value selects either source approval pin.
pub(super) fn verify_bundle(
    cuda_dir: &Path,
    stems: &[&'static str],
    trusted_key_sha256: &str,
    manifest_sha256: &str,
) -> Result<VerifiedCudaBundle, String> {
    if !is_lower_sha256(trusted_key_sha256) {
        return Err(ProvenanceRefusal::TrustedFingerprint.to_string());
    }
    let manifest = read_regular_file(&cuda_dir.join(MANIFEST_NAME), Some(MAX_MANIFEST_BYTES))?;
    let public_key = read_regular_file(&cuda_dir.join(MANIFEST_PUBLIC_KEY_NAME), Some(32))?;
    let signature = read_regular_file(&cuda_dir.join(MANIFEST_SIGNATURE_NAME), Some(64))?;
    let mut reader = BorrowedCudaReader::new(
        &manifest,
        &public_key,
        &signature,
        stems,
        trusted_key_sha256,
    )
    .map_err(|error| error.to_string())?;
    let mut artifacts = Vec::with_capacity(stems.len());
    let mut sources = Vec::with_capacity(stems.len());
    for _ in stems {
        let stem = reader.next_row().map_err(|error| error.to_string())?;
        let source = read_regular_file(&cuda_dir.join(format!("{stem}.cu")), None)?;
        let ptx = read_regular_file(&cuda_dir.join(format!("{stem}.ptx")), Some(MAX_PTX_BYTES))?;
        reader
            .accept_row(&source, &ptx)
            .map_err(|error| error.to_string())?;
        sources.push(source);
        artifacts.push(ptx);
    }
    let metadata = reader
        .finish(manifest_sha256)
        .map_err(|error| error.to_string())?;
    let cuda_image_sha256 = metadata.cuda_image_sha256.to_owned();
    #[cfg(test)]
    let nvcc_version_sha256 = metadata.nvcc_version_sha256.to_owned();
    #[cfg(test)]
    let nvcc_flags = metadata.nvcc_flags.to_owned();
    #[cfg(test)]
    let target_profile = metadata.target_profile.to_owned();
    let public_key = public_key
        .try_into()
        .map_err(|_| ProvenanceRefusal::PublicKeyLength.to_string())?;
    let signature = signature
        .try_into()
        .map_err(|_| ProvenanceRefusal::SignatureLength.to_string())?;
    Ok(VerifiedCudaBundle {
        artifacts,
        sources,
        manifest,
        public_key,
        signature,
        cuda_image_sha256,
        #[cfg(test)]
        nvcc_version_sha256,
        #[cfg(test)]
        nvcc_flags,
        #[cfg(test)]
        target_profile,
    })
}

#[cfg(feature = "cuda")]
pub(super) fn validate_ptx_bytes(path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    cuda_build_policy::validate_ptx(bytes).map_err(|error| match error {
        cuda_build_policy::PtxRefusal::Utf8(error) => {
            format!("PTX {} is not UTF-8 text: {error}", path.display())
        }
        cuda_build_policy::PtxRefusal::Directive(directive) => format!(
            "PTX {} is missing required {directive} directive",
            path.display()
        ),
        cuda_build_policy::PtxRefusal::InteriorNul => format!(
            "PTX {} must have exactly one appended terminal NUL",
            path.display()
        ),
    })?;
    Ok(())
}
#[cfg(all(test, feature = "cuda"))]
mod tests {
    use super::*;
    #[test]
    fn ptx_validator_rejects_comment_only_placeholders() {
        let path = Path::new("placeholder.ptx");
        assert!(validate_ptx_bytes(path, b"// Placeholder PTX; CUDA stays disabled.\n").is_err());
    }
    #[test]
    fn ptx_validator_accepts_required_directives_and_entry() {
        let path = Path::new("kernel.ptx");
        let ptx = b".version 7.8\n\
                    .target sm_86\n\
                    .address_size 64\n\
                    .visible .entry kernel() { ret; }\n";
        assert!(validate_ptx_bytes(path, ptx).is_ok());
    }
}
