//! One canonical Norito JSON manifest for the four native shipping executables.

use std::{io, path::Path};

use norito::json::{self, JsonDeserialize, JsonSerialize};

/// Sole manifest filename in the flat immutable invocation directory.
pub const MANIFEST_NAME: &str = "release-bundle.json";
/// Largest canonical manifest, including its final newline.
pub const MAX_MANIFEST_BYTES: usize = 32 * 1024;
/// Largest individual executable admitted by the release owner.
pub const MAX_BINARY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Manifest schema for the first native release format.
pub const SCHEMA: &str = "iroha.release-bundle.v1";

const UNIX_DIRECTORY: &str = "effective-owner-single-directory-0500";
const UNIX_EXECUTABLE: &str = "effective-owner-single-link-0500";
const UNIX_MANIFEST: &str = "effective-owner-single-link-0400";
const WINDOWS_DIRECTORY: &str = "effective-sid-protected-directory-rx";
const WINDOWS_EXECUTABLE: &str = "effective-sid-protected-single-link-rx";
const WINDOWS_MANIFEST: &str = "effective-sid-protected-single-link-r";

/// A shipping executable, in the fixed bundle inventory order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(tag = "program", content = "detail")]
pub enum BinaryKind {
    /// Ordinary node daemon.
    #[norito(rename = "irohad")]
    Irohad,
    /// Command-line client.
    #[norito(rename = "iroha")]
    Iroha,
    /// Network and genesis tooling.
    #[norito(rename = "kagami")]
    Kagami,
    /// Public Taira installation profile daemon.
    #[norito(rename = "irohad_taira")]
    IrohadTaira,
}

impl BinaryKind {
    /// Exact mandatory inventory; feature-isolated test programs are absent.
    pub const ALL: [Self; 4] = [Self::Irohad, Self::Iroha, Self::Kagami, Self::IrohadTaira];

    /// Filename on the selected native platform, with no directory components.
    #[must_use]
    pub const fn filename(self, windows: bool) -> &'static str {
        match (self, windows) {
            (Self::Irohad, false) => "iroha3d",
            (Self::Iroha, false) => "iroha",
            (Self::Kagami, false) => "kagami",
            (Self::IrohadTaira, false) => "iroha3d_taira",
            (Self::Irohad, true) => "iroha3d.exe",
            (Self::Iroha, true) => "iroha.exe",
            (Self::Kagami, true) => "kagami.exe",
            (Self::IrohadTaira, true) => "iroha3d_taira.exe",
        }
    }

    /// Index of this executable in the retained flat batch.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Irohad => 0,
            Self::Iroha => 1,
            Self::Kagami => 2,
            Self::IrohadTaira => 3,
        }
    }
}

/// Content and access-policy commitment for one mandatory executable.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct BinaryAttestation {
    /// Closed executable discriminator.
    pub kind: BinaryKind,
    /// Exact direct-child filename for the runner platform.
    pub filename: String,
    /// Lowercase SHA-256 of the original executable bytes.
    pub sha256: String,
    /// Exact original byte count, within the executable ceiling.
    pub size_bytes: u64,
    /// Explicit native executable custody policy.
    pub custody: String,
    /// Original local file identity; a same-byte replacement is a different publication.
    pub identity: NativeIdentity,
}

/// Full machine-local identity anchored by this invocation's authenticated manifest.
///
/// These values are never consensus identities. Native identifiers can be reused once
/// all handles close; admission also checks the unchanged local directory and bytes.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct NativeIdentity {
    /// Unix is 1; Windows is 2.
    pub platform: u8,
    /// Full native volume identifier.
    pub volume: u64,
    /// Full 128-bit object identifier in lowercase hexadecimal native byte order.
    pub object: String,
}

/// Closed manifest payload; parsing does not authenticate filesystem objects.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct Manifest {
    /// Exact first-release schema identifier.
    pub schema: String,
    /// Parent release owner's authenticated workspace source identity.
    pub source_manifest_sha256: String,
    /// Exact retained workspace lockfile digest.
    pub cargo_lock_sha256: String,
    /// Digest of the retained Cargo version transcript.
    pub cargo_version_sha256: String,
    /// Digest of the retained verbose rustc version transcript.
    pub rustc_version_sha256: String,
    /// Native host triple reported by the authenticated rustc transcript.
    pub host_triple: String,
    /// Actual native target of these four executables.
    pub target_triple: String,
    /// Exact Cargo release profile.
    pub profile: String,
    /// Exact absolute invocation directory spelling selected by the parent owner.
    pub bundle_dir: String,
    /// Explicit native immutable-directory custody policy.
    pub directory_custody: String,
    /// Explicit native read-only manifest custody policy.
    pub file_custody: String,
    /// Original immutable invocation directory, independent of its path spelling.
    pub directory_identity: NativeIdentity,
    /// Original manifest file itself, captured before encoding and retained through sealing.
    pub file_identity: NativeIdentity,
    /// Exact ordered attestations for all four shipping programs.
    pub binaries: Vec<BinaryAttestation>,
}

/// Independent bindings supplied by the release invocation, never inferred from its manifest.
#[derive(Debug, Clone, Copy)]
pub struct ExpectedManifest<'a> {
    /// Source census digest authenticated by the parent release owner.
    pub source_sha256: &'a str,
    /// Digest of the independently retained current Cargo.lock.
    pub cargo_lock_sha256: &'a str,
    /// Current invocation directory; replay from another directory is refused.
    pub bundle_dir: &'a Path,
    /// Actual compiled native target of the admitting process.
    pub native_target: &'a str,
    /// Whether the admitting process uses Windows native custody.
    pub windows: bool,
}

fn invalid(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason)
}

fn require(condition: bool, reason: &'static str) -> io::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(invalid(reason))
    }
}

/// Check the canonical spelling of a SHA-256 anchor.
#[must_use]
pub fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_target(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

/// Exact custody labels for directory, executable and manifest on one native platform.
#[must_use]
pub const fn custody_policies(windows: bool) -> (&'static str, &'static str, &'static str) {
    if windows {
        (WINDOWS_DIRECTORY, WINDOWS_EXECUTABLE, WINDOWS_MANIFEST)
    } else {
        (UNIX_DIRECTORY, UNIX_EXECUTABLE, UNIX_MANIFEST)
    }
}

impl Manifest {
    /// Check all semantic bindings before admitting canonical bytes or publishing a manifest.
    ///
    /// # Errors
    /// Refuses stale anchors, target mismatch, a different invocation, nonrelease profiles,
    /// unexpected custody labels, missing/reordered programs and invalid content bounds.
    pub fn validate(&self, expected: ExpectedManifest<'_>) -> io::Result<()> {
        require(self.schema == SCHEMA, "unsupported release bundle schema")?;
        for value in [
            self.source_manifest_sha256.as_str(),
            self.cargo_lock_sha256.as_str(),
            self.cargo_version_sha256.as_str(),
            self.rustc_version_sha256.as_str(),
            expected.source_sha256,
            expected.cargo_lock_sha256,
        ] {
            require(is_digest(value), "invalid release digest spelling")?;
        }
        require(
            self.source_manifest_sha256 == expected.source_sha256,
            "release source anchor differs",
        )?;
        require(
            self.cargo_lock_sha256 == expected.cargo_lock_sha256,
            "release lockfile anchor differs",
        )?;
        require(
            is_target(expected.native_target)
                && self.host_triple == expected.native_target
                && self.target_triple == expected.native_target,
            "release executables require the actual native host and target",
        )?;
        require(self.profile == "release", "release profile differs")?;
        require(
            expected.bundle_dir.is_absolute()
                && expected.bundle_dir.to_str() == Some(self.bundle_dir.as_str()),
            "release invocation directory differs",
        )?;
        let (directory, executable, manifest) = custody_policies(expected.windows);
        require(
            self.directory_custody == directory && self.file_custody == manifest,
            "release directory or manifest custody differs",
        )?;
        require(
            self.binaries.len() == BinaryKind::ALL.len(),
            "release executable inventory must contain exactly four programs",
        )?;
        let identities: Vec<_> = [&self.directory_identity, &self.file_identity]
            .into_iter()
            .chain(self.binaries.iter().map(|binary| &binary.identity))
            .collect();
        for (index, identity) in identities.iter().enumerate() {
            require(
                identity.platform == if expected.windows { 2 } else { 1 }
                    && identity.volume == self.directory_identity.volume
                    && identity.object.len() == 32
                    && identity
                        .object
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
                "release native identity has the wrong platform, volume or spelling",
            )?;
            require(
                !identities[..index].contains(identity),
                "release originals must have distinct native identities",
            )?;
        }
        for (kind, attestation) in BinaryKind::ALL.into_iter().zip(&self.binaries) {
            require(
                attestation.kind == kind
                    && attestation.filename == kind.filename(expected.windows)
                    && attestation.custody == executable,
                "release executable inventory or custody differs",
            )?;
            require(is_digest(&attestation.sha256), "invalid executable digest")?;
            require(
                (1..=MAX_BINARY_BYTES).contains(&attestation.size_bytes),
                "release executable size exceeds bounds",
            )?;
        }
        Ok(())
    }

    /// Encode one canonical bounded Norito JSON payload with a final LF.
    ///
    /// # Errors
    /// Refuses invalid semantic bindings, encoding errors and an oversized manifest.
    pub fn encode(&self, expected: ExpectedManifest<'_>) -> io::Result<Vec<u8>> {
        self.validate(expected)?;
        let mut bytes =
            json::to_vec(self).map_err(|_| invalid("release manifest encoding failed"))?;
        bytes.push(b'\n');
        require(
            bytes.len() <= MAX_MANIFEST_BYTES,
            "release manifest exceeds bound",
        )?;
        Ok(bytes)
    }

    /// Decode only the canonical current format, with all independent bindings required.
    ///
    /// # Errors
    /// Refuses unknown fields, alternate encodings, trailing data, excessive input or a
    /// semantic mismatch. There is no retired TSV decoder or compatibility alias.
    pub fn decode(bytes: &[u8], expected: ExpectedManifest<'_>) -> io::Result<Self> {
        require(
            !bytes.is_empty() && bytes.len() <= MAX_MANIFEST_BYTES && bytes.last() == Some(&b'\n'),
            "release manifest is empty, oversized or unterminated",
        )?;
        let manifest: Self = json::from_slice(bytes)
            .map_err(|_| invalid("release manifest is not its closed Norito JSON type"))?;
        require(
            manifest.encode(expected)? == bytes,
            "release manifest encoding is noncanonical",
        )?;
        Ok(manifest)
    }
}

#[cfg(test)]
#[path = "manifest/tests.rs"]
mod tests;
