//! Guard directory snapshot helpers.
#![allow(unexpected_cfgs)]
use crate::{
    signature::ed25519::{Ed25519Sha512, PublicKey as Ed25519PublicKey},
    soranet::certificate::{
        CertificateValidationPhase, RelayCertificateBundleV2, RelayCertificateV2,
        SRC_V2_MAX_BUNDLE_BYTES,
    },
};
use blake3::Hasher as Blake3Hasher;
use norito::{
    DecodeLimits, NoritoDeserialize, NoritoSerialize, decode_from_bytes_with_limits, to_bytes,
};
use soranet_pq::MlDsaSuite;
use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
    fs,
    io::{self, Read as _},
    path::{Component, Path, PathBuf},
};
const SRC_V2_ISSUER_FINGERPRINT_DOMAIN: &[u8] = b"soranet.src.v2.issuer";
const GUARD_DIRECTORY_SNAPSHOT_DIGEST_DOMAIN: &[u8] = b"soranet.guard-directory.snapshot.v2";
type IssuersByFingerprint<'a> = HashMap<[u8; 32], (Ed25519PublicKey, &'a [u8])>;
/// Schema version used by `GuardDirectorySnapshotV2`.
pub const GUARD_DIRECTORY_VERSION_V2: u8 = 2;
/// Maximum encoded size of one first-release guard-directory snapshot.
pub const GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1: usize = 5 * 1024 * 1024;
/// Maximum number of governance issuers in one first-release snapshot.
pub const GUARD_DIRECTORY_MAX_ISSUERS_V1: usize = 16;
/// Maximum number of relay entries in one first-release snapshot.
pub const GUARD_DIRECTORY_MAX_RELAYS_V1: usize = 64;
/// Maximum byte length of an issuer's ML-DSA-65 public key.
pub const GUARD_DIRECTORY_ISSUER_MLDSA65_MAX_BYTES_V1: usize = 1_952;
/// Maximum byte length of one embedded relay certificate bundle.
pub const GUARD_DIRECTORY_RELAY_CERTIFICATE_MAX_BYTES_V1: usize = SRC_V2_MAX_BUNDLE_BYTES;
const GUARD_DIRECTORY_DECODE_MAX_ALLOCATED_BYTES_V1: usize =
    2 * GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1;
const GUARD_DIRECTORY_DECODE_MAX_NESTING_DEPTH_V1: usize = 16;
#[cfg(any(target_os = "macos", target_os = "ios"))]
const GUARD_DIRECTORY_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
#[cfg(any(target_os = "linux", target_os = "android"))]
const GUARD_DIRECTORY_O_NOFOLLOW_FLAG: i32 = 0x0002_0000;
#[cfg(any(
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
const GUARD_DIRECTORY_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))
))]
compile_error!(
    "guard-directory file loading requires a defined no-follow open flag on this Unix target"
);
const fn guard_directory_decode_limits_v1() -> DecodeLimits {
    DecodeLimits::new(
        GUARD_DIRECTORY_RELAY_CERTIFICATE_MAX_BYTES_V1,
        GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1,
        GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1,
        GUARD_DIRECTORY_DECODE_MAX_ALLOCATED_BYTES_V1,
        GUARD_DIRECTORY_DECODE_MAX_NESTING_DEPTH_V1,
    )
}
/// Read one guard-directory snapshot from a stable, direct regular file.
///
/// Parent aliases are resolved once to a custodied canonical directory and pinned by an open
/// handle. The final path component is opened without following symbolic links or Windows reparse
/// points. File identity, type, and length must remain stable across a read capped at one byte
/// beyond [`GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1`].
///
/// # Errors
/// Returns an I/O error if the path cannot be opened safely, is not a direct
/// regular file, changes while being read, or exceeds the first-release limit.
pub fn read_guard_directory_snapshot_file(path: &Path) -> io::Result<Vec<u8>> {
    read_guard_directory_snapshot_file_with_hook(path, || {})
}
fn read_guard_directory_snapshot_file_with_hook(
    path: &Path,
    after_pin: impl FnOnce(),
) -> io::Result<Vec<u8>> {
    let pinned = PinnedGuardDirectoryPath::new(path)?;
    after_pin();
    pinned.verify_parent()?;
    let path = pinned.path();
    let max_bytes = u64::try_from(GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1)
        .expect("fixed guard-directory snapshot limit fits u64");
    let named_before = fs::symlink_metadata(path)?;
    pinned.validate_file(&named_before)?;
    if named_before.len() > max_bytes {
        return Err(guard_directory_snapshot_too_large());
    }
    let mut file = open_guard_directory_snapshot_file(path)?;
    let opened_before = file.metadata()?;
    pinned.validate_file(&opened_before)?;
    if !guard_directory_path_identifies_open_file(path, &named_before, &file)?
        || opened_before.len() > max_bytes
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guard directory snapshot changed identity or type while opening",
        ));
    }
    let capacity = usize::try_from(opened_before.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "guard directory snapshot length cannot be addressed on this platform",
        )
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.by_ref()
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1 {
        return Err(guard_directory_snapshot_too_large());
    }
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    pinned.validate_file(&opened_after)?;
    pinned.validate_file(&named_after)?;
    let observed_bytes = u64::try_from(bytes.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "guard directory snapshot byte count cannot be represented as u64",
        )
    })?;
    if !guard_directory_open_metadata_identifies_same_file(&opened_before, &opened_after)
        || !guard_directory_path_identifies_open_file(path, &named_after, &file)?
        || opened_before.len() != opened_after.len()
        || opened_after.len() != named_after.len()
        || opened_after.len() != observed_bytes
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "guard directory snapshot changed while it was being read",
        ));
    }
    pinned.verify_parent()?;
    Ok(bytes)
}
struct PinnedGuardDirectoryPath {
    path: PathBuf,
    parent_path: PathBuf,
    parent: fs::File,
    #[cfg(unix)]
    owner_uid: u32,
}
impl PinnedGuardDirectoryPath {
    fn new(path: &Path) -> io::Result<Self> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        if absolute
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "guard directory snapshot path must not contain dot components",
            ));
        }
        let file_name = absolute.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "guard directory snapshot path must name a file",
            )
        })?;
        let parent_path = fs::canonicalize(absolute.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "guard directory snapshot path must have a parent",
            )
        })?)?;
        #[cfg(unix)]
        let owner_uid = guard_directory_current_uid()?;
        #[cfg(unix)]
        validate_guard_directory_ancestor_chain(&parent_path, owner_uid)?;
        let named = fs::symlink_metadata(&parent_path)?;
        if guard_directory_metadata_is_link(&named) || !named.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "guard directory snapshot parent must be a direct directory",
            ));
        }
        let parent = open_guard_directory_parent(&parent_path)?;
        let opened = parent.metadata()?;
        if !opened.is_dir()
            || !guard_directory_parent_path_identifies_open_file(&parent_path, &named, &parent)?
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "guard directory snapshot parent changed while opening",
            ));
        }
        let pinned = Self {
            path: parent_path.join(file_name),
            parent_path,
            parent,
            #[cfg(unix)]
            owner_uid,
        };
        pinned.verify_parent()?;
        Ok(pinned)
    }
    fn path(&self) -> &Path {
        &self.path
    }
    fn verify_parent(&self) -> io::Result<()> {
        let named = fs::symlink_metadata(&self.parent_path)?;
        let opened = self.parent.metadata()?;
        if guard_directory_metadata_is_link(&named)
            || !named.is_dir()
            || !opened.is_dir()
            || !guard_directory_parent_path_identifies_open_file(
                &self.parent_path,
                &named,
                &self.parent,
            )?
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "guard directory snapshot parent changed while in use",
            ));
        }
        #[cfg(unix)]
        validate_guard_directory_ancestor_chain(&self.parent_path, self.owner_uid)?;
        Ok(())
    }
    fn validate_file(&self, metadata: &fs::Metadata) -> io::Result<()> {
        if guard_directory_metadata_is_link(metadata) || !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "guard directory snapshot must be a direct regular file",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            if (metadata.uid() != 0 && metadata.uid() != self.owner_uid)
                || metadata.mode() & 0o022 != 0
                || metadata.nlink() != 1
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "guard directory snapshot must be owner-or-root held, non-writable by other principals, and have one link",
                ));
            }
        }
        Ok(())
    }
}
#[cfg(unix)]
fn guard_directory_current_uid() -> io::Result<u32> {
    use std::os::unix::fs::MetadataExt as _;
    Ok(tempfile::tempfile()?.metadata()?.uid())
}
#[cfg(unix)]
fn validate_guard_directory_ancestor_chain(parent: &Path, owner_uid: u32) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt as _;
    let mut ancestors = Vec::new();
    let mut cursor = parent;
    loop {
        ancestors.push(cursor.to_path_buf());
        let Some(next) = cursor.parent() else {
            break;
        };
        if next == cursor {
            break;
        }
        cursor = next;
    }
    ancestors.reverse();
    let mut metadata = Vec::with_capacity(ancestors.len());
    for ancestor in &ancestors {
        let observed = fs::symlink_metadata(ancestor)?;
        if guard_directory_metadata_is_link(&observed) || !observed.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "guard directory snapshot ancestor must be a direct directory",
            ));
        }
        if observed.uid() != 0 && observed.uid() != owner_uid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "guard directory snapshot ancestor {} is not owner-or-root held",
                    ancestor.display()
                ),
            ));
        }
        metadata.push(observed);
    }
    for (index, observed) in metadata.iter().enumerate() {
        if observed.mode() & 0o022 == 0 {
            continue;
        }
        let protected_sticky_boundary = observed.uid() == 0
            && observed.mode() & 0o1000 != 0
            && metadata
                .get(index + 1)
                .is_some_and(|child| child.uid() == owner_uid && child.mode() & 0o022 == 0);
        if !protected_sticky_boundary {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "guard directory snapshot ancestor {} is replaceable",
                    ancestors[index].display()
                ),
            ));
        }
    }
    Ok(())
}
fn guard_directory_snapshot_too_large() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "guard directory snapshot exceeds the {GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1}-byte first-release limit"
        ),
    )
}
fn guard_directory_metadata_is_link(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}
#[cfg(unix)]
fn guard_directory_open_metadata_identifies_same_file(
    left: &fs::Metadata,
    right: &fs::Metadata,
) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino()
}
#[cfg(windows)]
fn guard_directory_open_metadata_identifies_same_file(
    _left: &fs::Metadata,
    _right: &fs::Metadata,
) -> bool {
    true
}
#[cfg(not(any(unix, windows)))]
fn guard_directory_open_metadata_identifies_same_file(
    _left: &fs::Metadata,
    _right: &fs::Metadata,
) -> bool {
    false
}

#[cfg(unix)]
fn open_guard_directory_snapshot_file(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .custom_flags(GUARD_DIRECTORY_O_NOFOLLOW_FLAG);
    options.open(path)
}

#[cfg(windows)]
fn open_guard_directory_snapshot_file(path: &Path) -> io::Result<fs::File> {
    super::windows_file_identity::open_direct_file(path)
}

#[cfg(not(any(unix, windows)))]
fn open_guard_directory_snapshot_file(path: &Path) -> io::Result<fs::File> {
    fs::File::open(path)
}

#[cfg(windows)]
fn open_guard_directory_parent(path: &Path) -> io::Result<fs::File> {
    super::windows_file_identity::open_direct_directory(path)
}

#[cfg(not(windows))]
fn open_guard_directory_parent(path: &Path) -> io::Result<fs::File> {
    fs::File::open(path)
}

#[cfg(unix)]
fn guard_directory_path_identifies_open_file(
    _path: &Path,
    named: &fs::Metadata,
    opened: &fs::File,
) -> io::Result<bool> {
    Ok(guard_directory_open_metadata_identifies_same_file(
        named,
        &opened.metadata()?,
    ))
}

#[cfg(windows)]
fn guard_directory_path_identifies_open_file(
    path: &Path,
    _named: &fs::Metadata,
    opened: &fs::File,
) -> io::Result<bool> {
    super::windows_file_identity::path_identifies_file(path, opened)
}

#[cfg(not(any(unix, windows)))]
fn guard_directory_path_identifies_open_file(
    _path: &Path,
    _named: &fs::Metadata,
    _opened: &fs::File,
) -> io::Result<bool> {
    Ok(false)
}

#[cfg(unix)]
fn guard_directory_parent_path_identifies_open_file(
    _path: &Path,
    named: &fs::Metadata,
    opened: &fs::File,
) -> io::Result<bool> {
    Ok(guard_directory_open_metadata_identifies_same_file(
        named,
        &opened.metadata()?,
    ))
}

#[cfg(windows)]
fn guard_directory_parent_path_identifies_open_file(
    path: &Path,
    _named: &fs::Metadata,
    opened: &fs::File,
) -> io::Result<bool> {
    super::windows_file_identity::path_identifies_directory(path, opened)
}

#[cfg(not(any(unix, windows)))]
fn guard_directory_parent_path_identifies_open_file(
    _path: &Path,
    _named: &fs::Metadata,
    _opened: &fs::File,
) -> io::Result<bool> {
    Ok(false)
}
/// Norito-encoded guard directory snapshot.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[norito(decode_from_slice)]
pub struct GuardDirectorySnapshotV2 {
    /// Snapshot schema version (`2`).
    pub version: u8,
    /// Consensus directory hash bound by certificates.
    pub directory_hash: [u8; 32],
    /// Publication timestamp (Unix seconds).
    pub published_at_unix: i64,
    /// Valid-after timestamp (Unix seconds).
    pub valid_after_unix: i64,
    /// Valid-until timestamp (Unix seconds).
    pub valid_until_unix: i64,
    /// Validation phase gate encoded as `u8`.
    pub validation_phase: u8,
    /// Governance issuer records.
    #[norito(default)]
    pub issuers: Vec<GuardDirectoryIssuerV1>,
    /// Relay certificate bundles.
    pub relays: Vec<GuardDirectoryRelayEntryV2>,
}
impl GuardDirectorySnapshotV2 {
    /// Encode the snapshot to Norito bytes.
    ///
    /// # Errors
    /// Returns an error if a first-release resource bound is exceeded or serialization fails.
    pub fn to_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        self.validate_resource_bounds()?;
        let bytes = to_bytes(self)?;
        validate_snapshot_encoded_len(bytes.len())?;
        Ok(bytes)
    }
    /// Decode and inspect a snapshot without establishing external trust or freshness.
    ///
    /// This verifies schema invariants and the self-consistency of signatures against issuer keys
    /// embedded in the same snapshot. It must not be used as an authentication decision. Use
    /// [`Self::authenticate_bytes_at`] at a runtime trust boundary.
    ///
    /// # Errors
    /// Returns an error if decoding or intrinsic validation fails.
    pub fn inspect_bytes(bytes: &[u8]) -> Result<Self, norito::Error> {
        let snapshot = Self::decode_bounded(bytes)?;
        snapshot.validate(None, false)?;
        Ok(snapshot)
    }
    /// Authenticate an exact snapshot artifact and validate it at a supplied time.
    ///
    /// `expected_snapshot_digest` must arrive over a trust path independent of `bytes`. It is a
    /// domain-separated BLAKE3 digest of the exact Norito snapshot bytes, so it commits to the
    /// embedded issuer set as well as all relay certificates. Validity uses a half-open interval:
    /// `valid_after_unix <= at_unix < valid_until_unix`.
    ///
    /// # Errors
    /// Returns an error when the exact artifact digest differs, decoding or
    /// signature validation fails, or the snapshot is not active at `at_unix`.
    pub fn authenticate_bytes_at(
        bytes: &[u8],
        expected_snapshot_digest: [u8; 32],
        at_unix: i64,
    ) -> Result<Self, norito::Error> {
        validate_snapshot_digest(bytes, expected_snapshot_digest)?;
        let snapshot = Self::decode_bounded(bytes)?;
        snapshot.validate(Some(at_unix), true)?;
        Ok(snapshot)
    }
    /// Authenticate a snapshot and retain one validated relay bundle while
    /// streaming validation across the bounded relay set.
    ///
    /// This avoids decoding every certificate a second time when a caller needs
    /// one exact relay after authenticating the complete directory.
    ///
    /// # Errors
    /// Returns an error when the artifact digest differs, a first-release
    /// resource bound or validation rule is violated, or `relay_id` is absent.
    pub fn authenticate_relay_bytes_at(
        bytes: &[u8],
        expected_snapshot_digest: [u8; 32],
        relay_id: [u8; 32],
        at_unix: i64,
    ) -> Result<AuthenticatedGuardDirectoryRelayV2, norito::Error> {
        validate_snapshot_digest(bytes, expected_snapshot_digest)?;
        let snapshot = Self::decode_bounded(bytes)?;
        let relay = snapshot
            .validate_and_select_relay(Some(at_unix), true, Some(relay_id))?
            .ok_or_else(|| {
                norito::Error::Message(format!(
                    "relay {} is absent from the authenticated guard directory",
                    hex::encode(relay_id)
                ))
            })?;
        Ok(AuthenticatedGuardDirectoryRelayV2 {
            snapshot_valid_until_unix: snapshot.valid_until_unix,
            relay,
        })
    }
    fn decode_bounded(bytes: &[u8]) -> Result<Self, norito::Error> {
        validate_snapshot_encoded_len(bytes.len())?;
        decode_from_bytes_with_limits(bytes, guard_directory_decode_limits_v1())
    }
    fn validate_resource_bounds(&self) -> Result<(), norito::Error> {
        if self.issuers.len() > GUARD_DIRECTORY_MAX_ISSUERS_V1 {
            return Err(norito::Error::Message(format!(
                "guard directory snapshot issuer count {} exceeds first-release maximum {GUARD_DIRECTORY_MAX_ISSUERS_V1}",
                self.issuers.len()
            )));
        }
        if self.relays.len() > GUARD_DIRECTORY_MAX_RELAYS_V1 {
            return Err(norito::Error::Message(format!(
                "guard directory snapshot relay count {} exceeds first-release maximum {GUARD_DIRECTORY_MAX_RELAYS_V1}",
                self.relays.len()
            )));
        }
        for issuer in &self.issuers {
            if issuer.mldsa65_public.len() > GUARD_DIRECTORY_ISSUER_MLDSA65_MAX_BYTES_V1 {
                return Err(norito::Error::Message(format!(
                    "guard directory issuer ML-DSA-65 public key length {} exceeds first-release maximum {GUARD_DIRECTORY_ISSUER_MLDSA65_MAX_BYTES_V1}",
                    issuer.mldsa65_public.len()
                )));
            }
        }
        for relay in &self.relays {
            if relay.certificate.len() > GUARD_DIRECTORY_RELAY_CERTIFICATE_MAX_BYTES_V1 {
                return Err(norito::Error::Message(format!(
                    "guard directory relay certificate bundle length {} exceeds first-release maximum {GUARD_DIRECTORY_RELAY_CERTIFICATE_MAX_BYTES_V1}",
                    relay.certificate.len()
                )));
            }
        }
        Ok(())
    }
    fn validate(
        &self,
        at_unix: Option<i64>,
        require_first_release_policy: bool,
    ) -> Result<(), norito::Error> {
        self.validate_and_select_relay(at_unix, require_first_release_policy, None)
            .map(drop)
    }
    fn validate_and_select_relay(
        &self,
        at_unix: Option<i64>,
        require_first_release_policy: bool,
        target_relay_id: Option<[u8; 32]>,
    ) -> Result<Option<RelayCertificateBundleV2>, norito::Error> {
        self.validate_resource_bounds()?;
        let validation_phase = self.validate_header()?;
        if require_first_release_policy
            && validation_phase != CertificateValidationPhase::Phase3RequireDual
        {
            return Err(norito::Error::Message(format!(
                "authenticated guard directory requires phase 3 dual signatures in the first release (got validation_phase {})",
                self.validation_phase
            )));
        }
        if let Some(at_unix) = at_unix {
            self.validate_at(at_unix)?;
        }
        let issuers_by_fingerprint = self.validate_issuers(validation_phase)?;
        self.validate_relays(
            validation_phase,
            &issuers_by_fingerprint,
            at_unix,
            target_relay_id,
        )
    }
    fn validate_header(&self) -> Result<CertificateValidationPhase, norito::Error> {
        if self.version != GUARD_DIRECTORY_VERSION_V2 {
            return Err(norito::Error::Message(format!(
                "guard directory snapshot version mismatch (expected {GUARD_DIRECTORY_VERSION_V2}, got {})",
                self.version
            )));
        }
        let validation_phase = decode_validation_phase(self.validation_phase).ok_or_else(|| {
            norito::Error::Message(format!(
                "guard directory snapshot validation_phase {} is not recognised",
                self.validation_phase
            ))
        })?;
        if self.published_at_unix < 0 || self.valid_after_unix < 0 || self.valid_until_unix < 0 {
            return Err(norito::Error::Message(
                "guard directory snapshot timestamps must be non-negative".to_string(),
            ));
        }
        if self.valid_after_unix >= self.valid_until_unix {
            return Err(norito::Error::Message(
                "guard directory snapshot valid_until_unix must be greater than valid_after_unix"
                    .to_string(),
            ));
        }
        if self.published_at_unix > self.valid_after_unix {
            return Err(norito::Error::Message(
                "guard directory snapshot published_at_unix exceeds valid_after_unix".to_string(),
            ));
        }
        Ok(validation_phase)
    }
    fn validate_at(&self, at_unix: i64) -> Result<(), norito::Error> {
        if at_unix < 0 {
            return Err(norito::Error::Message(
                "guard directory validation time must be non-negative Unix seconds".to_string(),
            ));
        }
        if at_unix < self.valid_after_unix {
            return Err(norito::Error::Message(format!(
                "guard directory snapshot is not yet valid at {at_unix} (valid_after_unix {})",
                self.valid_after_unix
            )));
        }
        if at_unix >= self.valid_until_unix {
            return Err(norito::Error::Message(format!(
                "guard directory snapshot is expired at {at_unix} (valid_until_unix {})",
                self.valid_until_unix
            )));
        }
        Ok(())
    }
    fn validate_issuers(
        &self,
        validation_phase: CertificateValidationPhase,
    ) -> Result<IssuersByFingerprint<'_>, norito::Error> {
        if self.issuers.is_empty() {
            return Err(norito::Error::Message(
                "guard directory snapshot must contain at least one issuer".to_string(),
            ));
        }
        let mut issuer_fingerprints = HashSet::with_capacity(self.issuers.len());
        let mut issuers_by_fingerprint = HashMap::with_capacity(self.issuers.len());
        for issuer in &self.issuers {
            if !issuer_fingerprints.insert(issuer.fingerprint) {
                return Err(norito::Error::Message(
                    "guard directory snapshot contains duplicate issuer fingerprint".to_string(),
                ));
            }
            let ed25519_public =
                Ed25519Sha512::parse_public_key(&issuer.ed25519_public).map_err(|err| {
                    norito::Error::Message(format!(
                        "guard directory issuer Ed25519 public key is invalid: {err}"
                    ))
                })?;
            Self::validate_issuer_mldsa65_public_key_len(validation_phase, &issuer.mldsa65_public)?;
            let computed =
                try_compute_issuer_fingerprint(&issuer.ed25519_public, &issuer.mldsa65_public)?;
            if computed != issuer.fingerprint {
                return Err(norito::Error::Message(
                    "guard directory issuer fingerprint does not match advertised keys".to_string(),
                ));
            }
            issuers_by_fingerprint.insert(
                issuer.fingerprint,
                (ed25519_public, issuer.mldsa65_public.as_slice()),
            );
        }
        Ok(issuers_by_fingerprint)
    }
    fn validate_issuer_mldsa65_public_key_len(
        validation_phase: CertificateValidationPhase,
        mldsa65_public: &[u8],
    ) -> Result<(), norito::Error> {
        if mldsa65_public.is_empty() {
            if validation_phase != CertificateValidationPhase::Phase1AllowSingle {
                return Err(norito::Error::Message(
                    "guard directory issuer ML-DSA-65 public key is required for validation phase"
                        .to_string(),
                ));
            }
            return Ok(());
        }
        validate_issuer_mldsa65_public_key_shape(mldsa65_public)
    }
    fn validate_relays(
        &self,
        validation_phase: CertificateValidationPhase,
        issuers_by_fingerprint: &IssuersByFingerprint<'_>,
        at_unix: Option<i64>,
        target_relay_id: Option<[u8; 32]>,
    ) -> Result<Option<RelayCertificateBundleV2>, norito::Error> {
        if self.relays.is_empty() {
            return Err(norito::Error::Message(
                "guard directory snapshot must contain at least one relay".to_string(),
            ));
        }
        let mut relay_ids = HashSet::with_capacity(self.relays.len());
        let mut selected = None;
        for relay in &self.relays {
            let bundle =
                RelayCertificateBundleV2::from_cbor(&relay.certificate).map_err(|err| {
                    norito::Error::Message(format!(
                        "guard directory relay certificate bundle is invalid: {err}"
                    ))
                })?;
            let issuer = issuers_by_fingerprint
                .get(&bundle.certificate.issuer_fingerprint)
                .ok_or_else(|| {
                    norito::Error::Message(
                        "guard directory relay certificate references unknown issuer fingerprint"
                            .to_string(),
                    )
                })?;
            if bundle.certificate.directory_hash != self.directory_hash {
                return Err(norito::Error::Message(
                    "guard directory relay certificate directory_hash does not match snapshot"
                        .to_string(),
                ));
            }
            if !relay_ids.insert(bundle.certificate.relay_id) {
                return Err(norito::Error::Message(
                    "guard directory snapshot contains duplicate relay id".to_string(),
                ));
            }
            self.validate_relay_certificate_window(&bundle.certificate)?;
            let verified = at_unix.map_or_else(
                || bundle.verify_signatures(&issuer.0, issuer.1, validation_phase),
                |at_unix| bundle.verify_at(&issuer.0, issuer.1, validation_phase, at_unix),
            );
            verified.map_err(|err| {
                norito::Error::Message(format!(
                    "guard directory relay certificate signature verification failed: {err}"
                ))
            })?;
            if Some(bundle.certificate.relay_id) == target_relay_id {
                selected = Some(bundle);
            }
        }
        Ok(selected)
    }
    fn validate_relay_certificate_window(
        &self,
        certificate: &RelayCertificateV2,
    ) -> Result<(), norito::Error> {
        if certificate.published_at > self.published_at_unix {
            return Err(norito::Error::Message(
                "guard directory relay certificate published_at is after snapshot publication"
                    .to_string(),
            ));
        }
        if certificate.valid_after > self.valid_after_unix {
            return Err(norito::Error::Message(
                "guard directory relay certificate is not valid at snapshot valid_after"
                    .to_string(),
            ));
        }
        if certificate.valid_until < self.valid_until_unix {
            return Err(norito::Error::Message(
                "guard directory relay certificate expires before snapshot valid_until".to_string(),
            ));
        }
        Ok(())
    }
}
/// One relay retained from a fully authenticated guard-directory snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedGuardDirectoryRelayV2 {
    /// Exclusive upper bound of the authenticated snapshot validity window.
    pub snapshot_valid_until_unix: i64,
    /// Exact validated relay bundle selected by relay identity.
    pub relay: RelayCertificateBundleV2,
}
fn validate_snapshot_encoded_len(len: usize) -> Result<(), norito::Error> {
    if len > GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1 {
        return Err(norito::Error::Message(format!(
            "guard directory snapshot length {len} exceeds first-release maximum {GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1}"
        )));
    }
    Ok(())
}
fn validate_snapshot_digest(
    bytes: &[u8],
    expected_snapshot_digest: [u8; 32],
) -> Result<(), norito::Error> {
    validate_snapshot_encoded_len(bytes.len())?;
    let actual = compute_snapshot_digest(bytes);
    if actual != expected_snapshot_digest {
        return Err(norito::Error::Message(format!(
            "guard directory snapshot digest mismatch (expected {}, got {})",
            hex::encode(expected_snapshot_digest),
            hex::encode(actual)
        )));
    }
    Ok(())
}
/// Compute the domain-separated digest that authenticates exact snapshot bytes.
#[must_use]
pub fn compute_snapshot_digest(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Blake3Hasher::new();
    hasher.update(GUARD_DIRECTORY_SNAPSHOT_DIGEST_DOMAIN);
    hasher.update(bytes);
    hasher.finalize().into()
}
/// Governance issuer record embedded in guard directory snapshots.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct GuardDirectoryIssuerV1 {
    /// Stable issuer fingerprint.
    pub fingerprint: [u8; 32],
    /// Ed25519 public key.
    pub ed25519_public: [u8; 32],
    /// Optional ML-DSA-65 public key (required for Phase 2+).
    #[norito(default)]
    pub mldsa65_public: Vec<u8>,
}
/// Relay entry embedded in guard directory snapshots.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct GuardDirectoryRelayEntryV2 {
    /// Serialized `RelayCertificateBundleV2` payload.
    pub certificate: Vec<u8>,
}
/// Compute the canonical issuer fingerprint used by SRC v2.
///
/// # Errors
/// Returns an error if the Ed25519 public key is malformed, the non-empty
/// ML-DSA material is not an ML-DSA-65 public key, or the ML-DSA public-key
/// length cannot be represented in the fingerprint's fixed `u32` length field.
pub fn compute_issuer_fingerprint(
    ed25519: &[u8; 32],
    mldsa_public: &[u8],
) -> Result<[u8; 32], norito::Error> {
    compute_issuer_fingerprint_inner(ed25519, mldsa_public)
}
/// Compute the canonical issuer fingerprint used by SRC v2.
///
/// # Errors
/// Returns an error if the Ed25519 public key is malformed, the non-empty
/// ML-DSA material is not an ML-DSA-65 public key, or the ML-DSA public-key
/// length cannot be represented in the fingerprint's fixed `u32` length field.
pub fn try_compute_issuer_fingerprint(
    ed25519: &[u8; 32],
    mldsa_public: &[u8],
) -> Result<[u8; 32], norito::Error> {
    compute_issuer_fingerprint_inner(ed25519, mldsa_public)
}
fn compute_issuer_fingerprint_inner(
    ed25519: &[u8; 32],
    mldsa_public: &[u8],
) -> Result<[u8; 32], norito::Error> {
    validate_issuer_ed25519_public_key(ed25519)?;
    validate_issuer_mldsa65_public_key_shape(mldsa_public)?;
    let mut hasher = Blake3Hasher::new();
    hasher.update(SRC_V2_ISSUER_FINGERPRINT_DOMAIN);
    hasher.update(ed25519);
    hasher.update(&issuer_fingerprint_len_bytes(mldsa_public.len())?);
    hasher.update(mldsa_public);
    Ok(hasher.finalize().into())
}
fn issuer_fingerprint_len_bytes(len: usize) -> Result<[u8; 4], norito::Error> {
    let len = u32::try_from(len).map_err(|_| {
        norito::Error::Message(format!(
            "guard directory issuer ML-DSA public key length {len} exceeds u32::MAX"
        ))
    })?;
    Ok(len.to_be_bytes())
}
fn validate_issuer_ed25519_public_key(ed25519: &[u8; 32]) -> Result<(), norito::Error> {
    Ed25519Sha512::parse_public_key(ed25519)
        .map(drop)
        .map_err(|err| {
            norito::Error::Message(format!(
                "guard directory issuer Ed25519 public key is invalid: {err}"
            ))
        })
}
fn validate_issuer_mldsa65_public_key_shape(mldsa_public: &[u8]) -> Result<(), norito::Error> {
    if mldsa_public.is_empty() {
        return Ok(());
    }
    let expected = MlDsaSuite::MlDsa65.public_key_len();
    if mldsa_public.len() != expected {
        return Err(norito::Error::Message(format!(
            "guard directory issuer ML-DSA-65 public key must be {expected} bytes, got {}",
            mldsa_public.len()
        )));
    }
    if mldsa_public.iter().all(|&byte| byte == 0) {
        return Err(norito::Error::Message(
            "guard directory issuer ML-DSA public key must not be all zero".to_string(),
        ));
    }
    Ok(())
}
/// Encode the validation phase to its wire representation.
#[must_use]
pub const fn encode_validation_phase(phase: CertificateValidationPhase) -> u8 {
    match phase {
        CertificateValidationPhase::Phase1AllowSingle => 1,
        CertificateValidationPhase::Phase2PreferDual => 2,
        CertificateValidationPhase::Phase3RequireDual => 3,
    }
}
/// Decode a validation phase from its wire representation.
#[must_use]
pub const fn decode_validation_phase(raw: u8) -> Option<CertificateValidationPhase> {
    match raw {
        1 => Some(CertificateValidationPhase::Phase1AllowSingle),
        2 => Some(CertificateValidationPhase::Phase2PreferDual),
        3 => Some(CertificateValidationPhase::Phase3RequireDual),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::soranet::{
        certificate::{
            CapabilityToggle, CertificateValidationPhase, KemRotationModeV1, KemRotationPolicyV1,
            RelayCapabilityFlagsV1, RelayCertificateV2, RelayEndpointV2, RelayRolesV2,
        },
        handshake::HandshakeSuite,
    };
    use ed25519_dalek::{SECRET_KEY_LENGTH, SigningKey};
    use soranet_pq::{
        HedgedRngSeed, MlKemSuite, generate_mldsa_keypair_from_seed as generate_mldsa_keypair,
    };
    fn sample_issuer_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[0x11; SECRET_KEY_LENGTH])
    }
    fn sample_mldsa_keypair(personalization: &'static [u8]) -> soranet_pq::MlDsaKeyPair {
        generate_mldsa_keypair(
            MlDsaSuite::MlDsa65,
            HedgedRngSeed::from_entropy([0x44; 32]),
            personalization,
        )
        .expect("sample ML-DSA keypair")
    }
    fn sample_relay_certificate(
        directory_hash: [u8; 32],
        issuer_fingerprint: [u8; 32],
        relay_identity_seed: [u8; 32],
    ) -> RelayCertificateV2 {
        let identity_ed25519 = SigningKey::from_bytes(&relay_identity_seed)
            .verifying_key()
            .to_bytes();
        RelayCertificateV2 {
            relay_id: identity_ed25519,
            identity_ed25519,
            identity_mldsa65: vec![0x55; MlDsaSuite::MlDsa65.public_key_len()],
            descriptor_commit: [0x33; 32],
            roles: RelayRolesV2 {
                entry: true,
                middle: true,
                exit: false,
            },
            guard_weight: 100,
            bandwidth_bytes_per_sec: 1_000_000,
            reputation_weight: 50,
            endpoints: vec![RelayEndpointV2 {
                quic_multiaddr: "/dns/relay.example.test/udp/443/quic".to_string(),
                tls_server_name: "relay.example.test".to_string(),
                tls_spki_sha256: [0xA5; 32],
                priority: 0,
                tags: vec!["nk3".to_string()],
            }],
            capability_flags: RelayCapabilityFlagsV1::new(
                CapabilityToggle::Enabled,
                CapabilityToggle::Disabled,
                CapabilityToggle::Enabled,
                CapabilityToggle::Disabled,
            ),
            kem_policy: KemRotationPolicyV1 {
                mode: KemRotationModeV1::Static,
                preferred_suite: MlKemSuite::MlKem768.kem_id(),
                fallback_suite: None,
                rotation_interval_hours: 0,
                grace_period_hours: 0,
            },
            handshake_suites: vec![
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            published_at: 1_734_000_000,
            valid_after: 1_734_000_000,
            valid_until: 1_734_086_400,
            directory_hash,
            issuer_fingerprint,
            pq_kem_public: vec![0x66; MlKemSuite::MlKem768.public_key_len()],
        }
    }
    fn sample_relay_bundle(
        directory_hash: [u8; 32],
        issuer_fingerprint: [u8; 32],
        relay_id: [u8; 32],
        issuer_signing_key: &SigningKey,
        issuer_mldsa_secret_key: &[u8],
        include_mldsa_signature: bool,
    ) -> RelayCertificateBundleV2 {
        let mut bundle = sample_relay_certificate(directory_hash, issuer_fingerprint, relay_id)
            .issue(issuer_signing_key, issuer_mldsa_secret_key)
            .expect("sample relay certificate issue");
        if !include_mldsa_signature {
            bundle.signatures.mldsa65 = None;
        }
        bundle
    }
    fn replace_first_relay_bundle(
        snapshot: &mut GuardDirectorySnapshotV2,
        bundle: &RelayCertificateBundleV2,
    ) {
        snapshot.relays[0].certificate = bundle.to_cbor();
    }
    fn mutate_first_relay_bundle(
        snapshot: &mut GuardDirectorySnapshotV2,
        mutate: impl FnOnce(&mut RelayCertificateBundleV2),
    ) {
        let mut bundle = RelayCertificateBundleV2::from_cbor(&snapshot.relays[0].certificate)
            .expect("sample relay bundle decodes");
        mutate(&mut bundle);
        replace_first_relay_bundle(snapshot, &bundle);
    }
    fn sample_snapshot() -> GuardDirectorySnapshotV2 {
        let issuer_signing_key = sample_issuer_signing_key();
        let issuer_mldsa = sample_mldsa_keypair(b"directory-snapshot-issuer");
        let ed25519_public = issuer_signing_key.verifying_key().to_bytes();
        let mldsa65_public = issuer_mldsa.public_key().to_vec();
        let fingerprint = compute_issuer_fingerprint(&ed25519_public, &mldsa65_public)
            .expect("sample issuer fingerprint should compute");
        let directory_hash = [0xAB; 32];
        GuardDirectorySnapshotV2 {
            version: GUARD_DIRECTORY_VERSION_V2,
            directory_hash,
            published_at_unix: 1_734_000_000,
            valid_after_unix: 1_734_000_000,
            valid_until_unix: 1_734_086_400,
            validation_phase: encode_validation_phase(
                CertificateValidationPhase::Phase3RequireDual,
            ),
            issuers: vec![GuardDirectoryIssuerV1 {
                fingerprint,
                ed25519_public,
                mldsa65_public,
            }],
            relays: vec![GuardDirectoryRelayEntryV2 {
                certificate: sample_relay_bundle(
                    directory_hash,
                    fingerprint,
                    [0x99; 32],
                    &issuer_signing_key,
                    issuer_mldsa.secret_key(),
                    true,
                )
                .to_cbor(),
            }],
        }
    }
    fn sample_phase1_single_signature_snapshot() -> GuardDirectorySnapshotV2 {
        let issuer_signing_key = sample_issuer_signing_key();
        let issuer_mldsa = sample_mldsa_keypair(b"directory-snapshot-phase1-signer");
        let ed25519_public = issuer_signing_key.verifying_key().to_bytes();
        let mldsa65_public = Vec::new();
        let fingerprint = compute_issuer_fingerprint(&ed25519_public, &mldsa65_public)
            .expect("sample phase-1 issuer fingerprint should compute");
        let directory_hash = [0xAB; 32];
        GuardDirectorySnapshotV2 {
            version: GUARD_DIRECTORY_VERSION_V2,
            directory_hash,
            published_at_unix: 1_734_000_000,
            valid_after_unix: 1_734_000_000,
            valid_until_unix: 1_734_086_400,
            validation_phase: encode_validation_phase(
                CertificateValidationPhase::Phase1AllowSingle,
            ),
            issuers: vec![GuardDirectoryIssuerV1 {
                fingerprint,
                ed25519_public,
                mldsa65_public,
            }],
            relays: vec![GuardDirectoryRelayEntryV2 {
                certificate: sample_relay_bundle(
                    directory_hash,
                    fingerprint,
                    [0x99; 32],
                    &issuer_signing_key,
                    issuer_mldsa.secret_key(),
                    false,
                )
                .to_cbor(),
            }],
        }
    }
    #[test]
    fn encode_decode_validation_phase_roundtrip() {
        for phase in [
            CertificateValidationPhase::Phase1AllowSingle,
            CertificateValidationPhase::Phase2PreferDual,
            CertificateValidationPhase::Phase3RequireDual,
        ] {
            let raw = encode_validation_phase(phase);
            assert_eq!(decode_validation_phase(raw), Some(phase));
        }
        assert_eq!(decode_validation_phase(0), None);
        assert_eq!(decode_validation_phase(4), None);
    }
    #[test]
    fn compute_fingerprint_changes_with_keys() {
        let ed_a = sample_issuer_signing_key().verifying_key().to_bytes();
        let ed_b = SigningKey::from_bytes(&[0x12; SECRET_KEY_LENGTH])
            .verifying_key()
            .to_bytes();
        let ml_a = vec![0xAA; MlDsaSuite::MlDsa65.public_key_len()];
        let ml_b = vec![0xBB; MlDsaSuite::MlDsa65.public_key_len()];
        let fingerprint_a =
            compute_issuer_fingerprint(&ed_a, &ml_a).expect("fingerprint A should compute");
        let fingerprint_b =
            compute_issuer_fingerprint(&ed_b, &ml_a).expect("fingerprint B should compute");
        let fingerprint_c =
            compute_issuer_fingerprint(&ed_a, &ml_b).expect("fingerprint C should compute");
        assert_ne!(fingerprint_a, fingerprint_b);
        assert_ne!(fingerprint_a, fingerprint_c);
        assert_ne!(fingerprint_b, fingerprint_c);
    }
    #[test]
    fn compute_fingerprint_matches_try_helper() {
        let ed25519 = sample_issuer_signing_key().verifying_key().to_bytes();
        let mldsa_public = vec![0xAA; MlDsaSuite::MlDsa65.public_key_len()];
        let via_try = try_compute_issuer_fingerprint(&ed25519, &mldsa_public)
            .expect("canonical issuer fingerprint should compute");
        let direct = compute_issuer_fingerprint(&ed25519, &mldsa_public)
            .expect("canonical issuer fingerprint should compute");
        assert_eq!(via_try, direct);
    }
    #[test]
    fn issuer_fingerprint_rejects_invalid_ed25519_public_key() {
        let ed25519 = [0u8; 32];
        let mldsa_public = vec![0xAA; MlDsaSuite::MlDsa65.public_key_len()];
        let err = try_compute_issuer_fingerprint(&ed25519, &mldsa_public)
            .expect_err("weak issuer Ed25519 public key must fail closed");
        assert!(
            err.to_string().contains("Ed25519 public key"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn issuer_fingerprint_rejects_invalid_mldsa_public_key_length() {
        let ed25519 = sample_issuer_signing_key().verifying_key().to_bytes();
        let mldsa_public = vec![0xAA; MlDsaSuite::MlDsa65.public_key_len() - 1];
        let err = try_compute_issuer_fingerprint(&ed25519, &mldsa_public)
            .expect_err("invalid issuer ML-DSA public-key length must fail closed");
        assert!(
            err.to_string().contains("ML-DSA-65 public key"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn issuer_fingerprint_rejects_all_zero_mldsa_public_key() {
        let ed25519 = sample_issuer_signing_key().verifying_key().to_bytes();
        let mldsa_public = vec![0u8; MlDsaSuite::MlDsa65.public_key_len()];
        let err = try_compute_issuer_fingerprint(&ed25519, &mldsa_public)
            .expect_err("all-zero issuer ML-DSA public key must fail closed");
        assert!(
            err.to_string().contains("all zero"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn issuer_fingerprint_length_overflow_fails_closed() {
        let Some(too_long) = (u64::from(u32::MAX) + 1).try_into().ok() else {
            return;
        };
        let err = issuer_fingerprint_len_bytes(too_long)
            .expect_err("oversized issuer public-key length must fail closed");
        assert!(
            err.to_string().contains("exceeds u32::MAX"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn snapshot_roundtrip() {
        let snapshot = sample_snapshot();
        let bytes = snapshot.to_bytes().expect("serialize");
        let decoded = GuardDirectorySnapshotV2::inspect_bytes(&bytes).expect("deserialize");
        assert_eq!(snapshot, decoded);
    }
    #[test]
    fn snapshot_file_reader_accepts_regular_file_and_rejects_oversize() {
        let temporary = tempfile::tempdir().expect("create guard-directory test root");
        let path = temporary.path().join("guard-directory.norito");
        let expected = b"bounded guard directory";
        fs::write(&path, expected).expect("write bounded regular file");
        assert_eq!(
            read_guard_directory_snapshot_file(&path).expect("read bounded regular file"),
            expected
        );
        fs::File::create(&path)
            .expect("recreate oversized sparse file")
            .set_len(
                u64::try_from(GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1)
                    .expect("fixed snapshot limit fits u64")
                    + 1,
            )
            .expect("extend oversized sparse file");
        let error = read_guard_directory_snapshot_file(&path)
            .expect_err("oversized guard directory must fail before reading");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("first-release limit"));
    }
    #[test]
    fn snapshot_file_reader_rejects_non_regular_path() {
        let temporary = tempfile::tempdir().expect("create guard-directory test root");
        let error = read_guard_directory_snapshot_file(temporary.path())
            .expect_err("directory path must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("direct regular file"));
    }
    #[cfg(unix)]
    #[test]
    fn snapshot_file_reader_pins_symlinked_parent_path() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().expect("temporary directory");
        let target_directory = directory.path().join("target");
        let alternate_directory = directory.path().join("alternate");
        let linked_directory = directory.path().join("linked");
        fs::create_dir(&target_directory).expect("create target directory");
        fs::create_dir(&alternate_directory).expect("create alternate directory");
        symlink(&target_directory, &linked_directory).expect("link parent directory");
        fs::write(target_directory.join("directory.norito"), b"snapshot")
            .expect("write target snapshot");
        fs::write(alternate_directory.join("directory.norito"), b"rollback")
            .expect("write alternate snapshot");
        assert_eq!(
            read_guard_directory_snapshot_file_with_hook(
                &linked_directory.join("directory.norito"),
                || {
                    fs::remove_file(&linked_directory).expect("remove parent alias");
                    symlink(&alternate_directory, &linked_directory)
                        .expect("redirect parent alias");
                },
            )
            .expect("read pinned parent"),
            b"snapshot"
        );
    }
    #[cfg(unix)]
    #[test]
    fn snapshot_file_reader_rejects_symlink() {
        use std::os::unix::fs::symlink;
        let temporary = tempfile::tempdir().expect("create guard-directory test root");
        let target = temporary.path().join("target.norito");
        let link = temporary.path().join("guard-directory.norito");
        fs::write(&target, b"guard directory").expect("write target");
        symlink(&target, &link).expect("create symlink fixture");
        let error = read_guard_directory_snapshot_file(&link)
            .expect_err("symlinked guard directory must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("direct regular file"));
    }
    #[cfg(unix)]
    #[test]
    fn snapshot_file_reader_rejects_replaceable_ancestor_or_file() {
        use std::os::unix::fs::PermissionsExt as _;
        let temporary = tempfile::tempdir().expect("temporary directory");
        let replaceable = temporary.path().join("replaceable");
        fs::create_dir(&replaceable).expect("create replaceable parent");
        let snapshot = replaceable.join("directory.norito");
        fs::write(&snapshot, b"snapshot").expect("write snapshot");
        fs::set_permissions(&replaceable, fs::Permissions::from_mode(0o777))
            .expect("make parent replaceable");
        let error = read_guard_directory_snapshot_file(&snapshot)
            .expect_err("replaceable parent must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        fs::set_permissions(&replaceable, fs::Permissions::from_mode(0o700))
            .expect("restore parent custody");

        fs::set_permissions(&snapshot, fs::Permissions::from_mode(0o666))
            .expect("make snapshot replaceable");
        let error = read_guard_directory_snapshot_file(&snapshot)
            .expect_err("replaceable snapshot must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        fs::set_permissions(&snapshot, fs::Permissions::from_mode(0o600))
            .expect("restore snapshot custody");
        fs::hard_link(&snapshot, replaceable.join("directory-alias.norito"))
            .expect("create second snapshot link");
        let error = read_guard_directory_snapshot_file(&snapshot)
            .expect_err("multiply-linked snapshot must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
    #[test]
    fn snapshot_wire_length_fails_before_decode() {
        let oversized = vec![0_u8; GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1 + 1];
        let err = GuardDirectorySnapshotV2::inspect_bytes(&oversized)
            .expect_err("oversized snapshot bytes must fail closed");
        assert!(
            err.to_string().contains("snapshot length"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn snapshot_producer_enforces_issuer_and_relay_counts() {
        let mut issuers = sample_snapshot();
        issuers.issuers = vec![issuers.issuers[0].clone(); GUARD_DIRECTORY_MAX_ISSUERS_V1 + 1];
        let issuer_err = issuers
            .to_bytes()
            .expect_err("producer must reject too many issuers");
        assert!(issuer_err.to_string().contains("issuer count"));
        let mut relays = sample_snapshot();
        relays.relays = vec![relays.relays[0].clone(); GUARD_DIRECTORY_MAX_RELAYS_V1 + 1];
        let relay_err = relays
            .to_bytes()
            .expect_err("producer must reject too many relays");
        assert!(relay_err.to_string().contains("relay count"));
    }
    #[test]
    fn snapshot_producer_accepts_first_release_resource_boundaries() {
        let mut snapshot = sample_snapshot();
        snapshot.issuers = vec![snapshot.issuers[0].clone(); GUARD_DIRECTORY_MAX_ISSUERS_V1];
        snapshot.relays[0].certificate = vec![0_u8; GUARD_DIRECTORY_RELAY_CERTIFICATE_MAX_BYTES_V1];
        snapshot.relays = vec![snapshot.relays[0].clone(); GUARD_DIRECTORY_MAX_RELAYS_V1];
        let bytes = snapshot
            .to_bytes()
            .expect("all exact first-release resource boundaries must encode");
        assert!(bytes.len() <= GUARD_DIRECTORY_SNAPSHOT_MAX_BYTES_V1);
    }
    #[test]
    fn snapshot_consumer_enforces_entry_counts_after_bounded_decode() {
        let mut issuers = sample_snapshot();
        issuers.issuers = vec![issuers.issuers[0].clone(); GUARD_DIRECTORY_MAX_ISSUERS_V1 + 1];
        let bytes = norito::to_bytes(&issuers).expect("encode out-of-policy issuer fixture");
        let issuer_err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("consumer must reject too many issuers");
        assert!(issuer_err.to_string().contains("issuer count"));
        let mut snapshot = sample_snapshot();
        snapshot.relays = vec![snapshot.relays[0].clone(); GUARD_DIRECTORY_MAX_RELAYS_V1 + 1];
        let bytes = norito::to_bytes(&snapshot).expect("encode out-of-policy wire fixture");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("consumer must reject too many relays");
        assert!(err.to_string().contains("relay count"));
    }
    #[test]
    fn snapshot_producer_enforces_per_entry_byte_bounds() {
        let mut issuer = sample_snapshot();
        issuer.issuers[0].mldsa65_public =
            vec![0_u8; GUARD_DIRECTORY_ISSUER_MLDSA65_MAX_BYTES_V1 + 1];
        let issuer_err = issuer
            .to_bytes()
            .expect_err("producer must reject oversized issuer material");
        assert!(issuer_err.to_string().contains("public key length"));
        let mut relay = sample_snapshot();
        relay.relays[0].certificate =
            vec![0_u8; GUARD_DIRECTORY_RELAY_CERTIFICATE_MAX_BYTES_V1 + 1];
        let relay_err = relay
            .to_bytes()
            .expect_err("producer must reject oversized relay bundle");
        assert!(relay_err.to_string().contains("certificate bundle length"));
    }
    #[test]
    fn snapshot_consumer_enforces_per_entry_byte_bounds() {
        let mut issuer = sample_snapshot();
        issuer.issuers[0].mldsa65_public =
            vec![0_u8; GUARD_DIRECTORY_ISSUER_MLDSA65_MAX_BYTES_V1 + 1];
        let issuer_bytes =
            norito::to_bytes(&issuer).expect("encode out-of-policy issuer byte fixture");
        let issuer_err = GuardDirectorySnapshotV2::inspect_bytes(&issuer_bytes)
            .expect_err("consumer must reject oversized issuer material");
        assert!(issuer_err.to_string().contains("public key length"));
        let mut relay = sample_snapshot();
        relay.relays[0].certificate =
            vec![0_u8; GUARD_DIRECTORY_RELAY_CERTIFICATE_MAX_BYTES_V1 + 1];
        let relay_bytes =
            norito::to_bytes(&relay).expect("encode out-of-policy relay byte fixture");
        GuardDirectorySnapshotV2::inspect_bytes(&relay_bytes)
            .expect_err("consumer decode budget must reject oversized relay bundle");
    }
    #[test]
    fn authenticated_relay_selection_returns_exact_validated_bundle() {
        let snapshot = sample_snapshot();
        let expected = RelayCertificateBundleV2::from_cbor(&snapshot.relays[0].certificate)
            .expect("sample relay bundle decodes");
        let relay_id = expected.certificate.relay_id;
        let bytes = snapshot.to_bytes().expect("serialize snapshot");
        let digest = compute_snapshot_digest(&bytes);
        let selected = GuardDirectorySnapshotV2::authenticate_relay_bytes_at(
            &bytes,
            digest,
            relay_id,
            snapshot.valid_after_unix,
        )
        .expect("exact relay is selected during authentication");
        assert_eq!(
            selected.snapshot_valid_until_unix,
            snapshot.valid_until_unix
        );
        assert_eq!(selected.relay, expected);
        let err = GuardDirectorySnapshotV2::authenticate_relay_bytes_at(
            &bytes,
            digest,
            [0xFF; 32],
            snapshot.valid_after_unix,
        )
        .expect_err("unknown relay identity must fail closed");
        assert!(err.to_string().contains("absent"));
    }
    #[test]
    fn exact_snapshot_digest_authenticates_embedded_issuer_set() {
        let trusted = sample_snapshot();
        let trusted_bytes = trusted.to_bytes().expect("serialize trusted snapshot");
        let trusted_digest = compute_snapshot_digest(&trusted_bytes);
        let attacker_signing_key = SigningKey::from_bytes(&[0x13; SECRET_KEY_LENGTH]);
        let attacker_mldsa = sample_mldsa_keypair(b"directory-snapshot-attacker");
        let attacker_ed25519 = attacker_signing_key.verifying_key().to_bytes();
        let attacker_mldsa_public = attacker_mldsa.public_key().to_vec();
        let attacker_fingerprint =
            compute_issuer_fingerprint(&attacker_ed25519, &attacker_mldsa_public)
                .expect("attacker fingerprint");
        let mut forged = sample_snapshot();
        forged.issuers = vec![GuardDirectoryIssuerV1 {
            fingerprint: attacker_fingerprint,
            ed25519_public: attacker_ed25519,
            mldsa65_public: attacker_mldsa_public,
        }];
        forged.relays[0].certificate = sample_relay_bundle(
            forged.directory_hash,
            attacker_fingerprint,
            [0x99; 32],
            &attacker_signing_key,
            attacker_mldsa.secret_key(),
            true,
        )
        .to_cbor();
        let forged_bytes = forged.to_bytes().expect("serialize forged snapshot");
        GuardDirectorySnapshotV2::inspect_bytes(&forged_bytes)
            .expect("self-signed snapshot is structurally self-consistent");
        let err = GuardDirectorySnapshotV2::authenticate_bytes_at(
            &forged_bytes,
            trusted_digest,
            forged.valid_after_unix,
        )
        .expect_err("a digest for another artifact must reject substituted issuers");
        assert!(err.to_string().contains("snapshot digest mismatch"));
    }
    #[test]
    fn authenticated_snapshot_enforces_half_open_validity_window() {
        let snapshot = sample_snapshot();
        let bytes = snapshot.to_bytes().expect("serialize");
        let digest = compute_snapshot_digest(&bytes);
        GuardDirectorySnapshotV2::authenticate_bytes_at(&bytes, digest, snapshot.valid_after_unix)
            .expect("valid_after is inclusive");
        GuardDirectorySnapshotV2::authenticate_bytes_at(
            &bytes,
            digest,
            snapshot.valid_until_unix - 1,
        )
        .expect("last second before valid_until is valid");
        let early = GuardDirectorySnapshotV2::authenticate_bytes_at(
            &bytes,
            digest,
            snapshot.valid_after_unix - 1,
        )
        .expect_err("not-yet-valid snapshot must fail");
        assert!(early.to_string().contains("not yet valid"));
        let expired = GuardDirectorySnapshotV2::authenticate_bytes_at(
            &bytes,
            digest,
            snapshot.valid_until_unix,
        )
        .expect_err("valid_until is exclusive");
        assert!(expired.to_string().contains("expired"));
    }
    #[test]
    fn authenticated_snapshot_rejects_pre_release_validation_phase() {
        let mut snapshot = sample_snapshot();
        snapshot.validation_phase =
            encode_validation_phase(CertificateValidationPhase::Phase2PreferDual);
        let bytes = snapshot.to_bytes().expect("serialize");
        let digest = compute_snapshot_digest(&bytes);
        GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect("phase 2 remains available for structural diagnostics");
        let err = GuardDirectorySnapshotV2::authenticate_bytes_at(
            &bytes,
            digest,
            snapshot.valid_after_unix,
        )
        .expect_err("authenticated first-release snapshots must require dual signatures");
        assert!(
            err.to_string().contains("phase 3 dual signatures"),
            "unexpected authentication error: {err}"
        );
    }
    #[test]
    fn snapshot_rejects_unknown_validation_phase() {
        let mut snapshot = sample_snapshot();
        snapshot.validation_phase = 0;
        let bytes = snapshot.to_bytes().expect("serialize");
        assert!(GuardDirectorySnapshotV2::inspect_bytes(&bytes).is_err());
    }
    #[test]
    fn snapshot_rejects_version_mismatch() {
        let mut snapshot = sample_snapshot();
        snapshot.version = 1;
        let bytes = snapshot.to_bytes().expect("serialize");
        assert!(GuardDirectorySnapshotV2::inspect_bytes(&bytes).is_err());
    }
    #[test]
    fn snapshot_rejects_empty_issuer_set() {
        let mut snapshot = sample_snapshot();
        snapshot.issuers.clear();
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("empty issuer set must fail");
        assert!(err.to_string().contains("at least one issuer"));
    }
    #[test]
    fn snapshot_rejects_empty_relay_set() {
        let mut snapshot = sample_snapshot();
        snapshot.relays.clear();
        let bytes = snapshot.to_bytes().expect("serialize");
        let err =
            GuardDirectorySnapshotV2::inspect_bytes(&bytes).expect_err("empty relay set must fail");
        assert!(err.to_string().contains("at least one relay"));
    }
    #[test]
    fn snapshot_rejects_invalid_time_window() {
        let mut snapshot = sample_snapshot();
        snapshot.valid_until_unix = snapshot.valid_after_unix;
        let bytes = snapshot.to_bytes().expect("serialize");
        assert!(GuardDirectorySnapshotV2::inspect_bytes(&bytes).is_err());
        let mut snapshot = sample_snapshot();
        snapshot.valid_after_unix = snapshot.valid_until_unix + 1;
        let bytes = snapshot.to_bytes().expect("serialize");
        assert!(GuardDirectorySnapshotV2::inspect_bytes(&bytes).is_err());
        let mut snapshot = sample_snapshot();
        snapshot.published_at_unix = snapshot.valid_after_unix + 1;
        let bytes = snapshot.to_bytes().expect("serialize");
        let error = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("a snapshot cannot be valid before it was published");
        assert!(error.to_string().contains("valid_after_unix"));
    }
    #[test]
    fn snapshot_rejects_negative_header_timestamps() {
        let mut published = sample_snapshot();
        published.published_at_unix = -1;
        let error = GuardDirectorySnapshotV2::inspect_bytes(
            &published
                .to_bytes()
                .expect("serialize negative publication"),
        )
        .expect_err("negative publication time must fail");
        assert!(error.to_string().contains("non-negative"));

        let mut valid_after = sample_snapshot();
        valid_after.valid_after_unix = -1;
        let error = GuardDirectorySnapshotV2::inspect_bytes(
            &valid_after
                .to_bytes()
                .expect("serialize negative valid-after"),
        )
        .expect_err("negative valid-after time must fail");
        assert!(error.to_string().contains("non-negative"));

        let mut valid_until = sample_snapshot();
        valid_until.valid_until_unix = -1;
        let error = GuardDirectorySnapshotV2::inspect_bytes(
            &valid_until
                .to_bytes()
                .expect("serialize negative valid-until"),
        )
        .expect_err("negative valid-until time must fail");
        assert!(error.to_string().contains("non-negative"));
    }
    #[test]
    fn snapshot_rejects_issuer_fingerprint_mismatch() {
        let mut snapshot = sample_snapshot();
        snapshot.issuers[0].fingerprint[0] ^= 0xFF;
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("fingerprint mismatch should fail");
        assert!(err.to_string().contains("fingerprint"));
    }
    #[test]
    fn snapshot_rejects_duplicate_issuer_fingerprints() {
        let mut snapshot = sample_snapshot();
        let duplicate = snapshot.issuers[0].clone();
        snapshot.issuers.push(duplicate);
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("duplicate issuer should fail");
        assert!(err.to_string().contains("duplicate"));
    }
    #[test]
    fn snapshot_rejects_invalid_mldsa65_public_key_length() {
        let mut snapshot = sample_snapshot();
        snapshot.issuers[0].mldsa65_public.pop();
        snapshot.issuers[0].fingerprint = [0xEE; 32];
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("invalid ML-DSA-65 public key length should fail");
        assert!(err.to_string().contains("ML-DSA-65 public key"));
    }
    #[test]
    fn snapshot_rejects_invalid_mldsa65_public_key_length_before_fingerprint() {
        let mut snapshot = sample_snapshot();
        snapshot.issuers[0].mldsa65_public.truncate(1);
        snapshot.issuers[0].fingerprint = [0xEE; 32];
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("issuer ML-DSA-65 key shape must fail before fingerprint");
        let message = err.to_string();
        assert!(
            message.contains("ML-DSA-65 public key"),
            "unexpected message: {message}"
        );
        assert!(
            !message.contains("fingerprint"),
            "shape preflight should run before fingerprint comparison: {message}"
        );
    }
    #[test]
    fn snapshot_rejects_all_zero_mldsa65_public_key_before_fingerprint() {
        let mut snapshot = sample_snapshot();
        snapshot.issuers[0].mldsa65_public = vec![0u8; MlDsaSuite::MlDsa65.public_key_len()];
        snapshot.issuers[0].fingerprint = [0xEE; 32];
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("all-zero issuer ML-DSA-65 key must fail before fingerprint");
        let message = err.to_string();
        assert!(
            message.contains("all zero"),
            "unexpected message: {message}"
        );
        assert!(
            !message.contains("fingerprint"),
            "inert key preflight should run before fingerprint comparison: {message}"
        );
    }
    #[test]
    fn snapshot_rejects_missing_mldsa65_public_key_after_phase1() {
        let mut snapshot = sample_snapshot();
        snapshot.issuers[0].mldsa65_public.clear();
        snapshot.issuers[0].fingerprint = compute_issuer_fingerprint(
            &snapshot.issuers[0].ed25519_public,
            &snapshot.issuers[0].mldsa65_public,
        )
        .expect("sample issuer fingerprint should compute");
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("phase 3 requires ML-DSA-65 issuer key");
        assert!(err.to_string().contains("required"));
    }
    #[test]
    fn snapshot_allows_phase1_empty_mldsa65_public_key() {
        let snapshot = sample_phase1_single_signature_snapshot();
        let bytes = snapshot.to_bytes().expect("serialize");
        GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect("phase 1 may carry an issuer without ML-DSA-65 key");
    }
    #[test]
    fn snapshot_phase2_accepts_single_signature_relay() {
        let mut snapshot = sample_snapshot();
        snapshot.validation_phase =
            encode_validation_phase(CertificateValidationPhase::Phase2PreferDual);
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            bundle.signatures.mldsa65 = None;
        });
        let bytes = snapshot.to_bytes().expect("serialize");
        GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect("phase 2 accepts Ed25519-only relay certificates during rollout");
    }
    #[test]
    fn snapshot_rejects_invalid_issuer_ed25519_public_key() {
        let mut snapshot = sample_snapshot();
        snapshot.issuers[0].ed25519_public = [0xFF; 32];
        snapshot.issuers[0].fingerprint = [0xEE; 32];
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("invalid issuer Ed25519 public key should fail");
        assert!(err.to_string().contains("Ed25519 public key"));
    }
    #[test]
    fn snapshot_rejects_malformed_relay_certificate_bundle() {
        let mut snapshot = sample_snapshot();
        snapshot.relays[0].certificate = vec![0x99, 0x00, 0x01];
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("malformed relay certificate bundle should fail");
        assert!(err.to_string().contains("relay certificate bundle"));
    }
    #[test]
    fn snapshot_rejects_relay_certificate_unknown_issuer() {
        let mut snapshot = sample_snapshot();
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            bundle.certificate.issuer_fingerprint = [0xEE; 32];
        });
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("relay certificate with unknown issuer should fail");
        assert!(err.to_string().contains("unknown issuer"));
    }
    #[test]
    fn snapshot_rejects_relay_certificate_directory_hash_mismatch() {
        let mut snapshot = sample_snapshot();
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            bundle.certificate.directory_hash = [0xDD; 32];
        });
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("relay certificate with mismatched directory hash should fail");
        assert!(err.to_string().contains("directory_hash"));
    }
    #[test]
    fn snapshot_rejects_relay_certificate_outside_snapshot_window() {
        let mut snapshot = sample_snapshot();
        let snapshot_valid_after = snapshot.valid_after_unix;
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            bundle.certificate.valid_after = snapshot_valid_after + 1;
        });
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("relay certificate not valid at snapshot start should fail");
        assert!(err.to_string().contains("valid_after"));
        let mut snapshot = sample_snapshot();
        let snapshot_valid_until = snapshot.valid_until_unix;
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            bundle.certificate.valid_until = snapshot_valid_until - 1;
        });
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("relay certificate expiring inside snapshot window should fail");
        assert!(err.to_string().contains("valid_until"));
        let mut snapshot = sample_snapshot();
        let snapshot_published_at = snapshot.published_at_unix;
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            bundle.certificate.published_at = snapshot_published_at + 1;
        });
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("relay certificate published after snapshot should fail");
        assert!(err.to_string().contains("published_at"));
    }
    #[test]
    fn snapshot_rejects_duplicate_relay_ids() {
        let mut snapshot = sample_snapshot();
        snapshot.relays.push(snapshot.relays[0].clone());
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("duplicate relay id should fail");
        assert!(err.to_string().contains("duplicate relay id"));
    }
    #[test]
    fn snapshot_rejects_bad_relay_certificate_ed25519_signature() {
        let mut snapshot = sample_snapshot();
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            bundle.signatures.ed25519[0] ^= 0xFF;
        });
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("bad relay Ed25519 signature should fail");
        assert!(err.to_string().contains("signature verification"));
    }
    #[test]
    fn snapshot_rejects_bad_relay_certificate_mldsa65_signature() {
        let mut snapshot = sample_snapshot();
        mutate_first_relay_bundle(&mut snapshot, |bundle| {
            let signature = bundle
                .signatures
                .mldsa65
                .as_mut()
                .expect("ML-DSA signature");
            signature[0] ^= 0xFF;
        });
        let bytes = snapshot.to_bytes().expect("serialize");
        let err = GuardDirectorySnapshotV2::inspect_bytes(&bytes)
            .expect_err("bad relay ML-DSA signature should fail");
        assert!(err.to_string().contains("signature verification"));
    }
}
