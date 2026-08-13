use std::{
    collections::{HashSet, TryReserveError},
    fs::{self, File, Metadata as FsMetadata, OpenOptions},
    io::{self, Read as _},
    path::Path,
    time::{Duration, SystemTime},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hex::FromHexError;
use iroha_crypto::soranet::token::{self, AdmissionToken, MintError, compute_issuer_fingerprint};
use norito::{
    DecodeLimits,
    json::{self, Value},
};
use rand::{CryptoRng, RngCore};
use soranet_pq::MlDsaSuite;
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
/// First-release maximum number of revoked token identifiers retained in one list.
///
/// This matches [`TokenConfig`](crate::config::TokenConfig)'s first-release
/// default replay-store capacity: the revocation side cannot admit more live
/// identifiers than the corresponding token replay corridor.
pub const REVOCATION_LIST_MAX_ENTRIES_V1: usize = 8_192;
const REVOCATION_TOKEN_ID_BYTES: usize = 32;
const REVOCATION_TOKEN_ID_HEX_BYTES: usize = REVOCATION_TOKEN_ID_BYTES * 2;
// A decoded ASCII byte can occupy six source bytes as `\uXXXX`; quotes add two.
const REVOCATION_LIST_MAX_ENCODED_STRING_BYTES_V1: usize = REVOCATION_TOKEN_ID_HEX_BYTES * 6 + 2;
const REVOCATION_LIST_MAX_TOTAL_STRING_BYTES_V1: usize =
    REVOCATION_LIST_MAX_ENTRIES_V1 * REVOCATION_TOKEN_ID_HEX_BYTES;
const REVOCATION_LIST_MAX_RETAINED_ID_BYTES_V1: usize =
    REVOCATION_LIST_MAX_ENTRIES_V1 * REVOCATION_TOKEN_ID_BYTES;
// Worst-case escaped strings occupy about 3.1 MiB. The 4 MiB corridor leaves
// bounded room for JSON whitespace while canonical producers emit under 1 MiB.
const REVOCATION_LIST_MAX_FILE_BYTES_V1: usize = 4 * 1024 * 1024;
const REVOCATION_LIST_MAX_NESTING_DEPTH_V1: usize = 2;
const REVOCATION_LIST_MAX_ALLOCATED_BYTES_V1: usize = 2 * 1024 * 1024;
const REVOCATION_LIST_CANONICAL_ENTRY_BYTES: usize = 70;
const REVOCATION_LIST_MAX_CANONICAL_BYTES_V1: usize =
    REVOCATION_LIST_MAX_ENTRIES_V1 * REVOCATION_LIST_CANONICAL_ENTRY_BYTES + 3;
const REVOCATION_LIST_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    REVOCATION_LIST_MAX_ENTRIES_V1,
    REVOCATION_TOKEN_ID_HEX_BYTES,
    REVOCATION_LIST_MAX_ENTRIES_V1,
    REVOCATION_LIST_MAX_ALLOCATED_BYTES_V1,
    REVOCATION_LIST_MAX_NESTING_DEPTH_V1,
);
const fn revocation_list_preflight_limits_v1() -> json::JsonPreflightLimits {
    json::JsonPreflightLimits::new(
        REVOCATION_LIST_MAX_FILE_BYTES_V1,
        REVOCATION_LIST_MAX_ENTRIES_V1 + 1,
        REVOCATION_LIST_MAX_ENCODED_STRING_BYTES_V1,
        REVOCATION_TOKEN_ID_HEX_BYTES,
        REVOCATION_LIST_MAX_TOTAL_STRING_BYTES_V1,
        REVOCATION_LIST_MAX_ENTRIES_V1,
        REVOCATION_LIST_MAX_ENTRIES_V1,
        0,
        REVOCATION_LIST_MAX_ENTRIES_V1,
        REVOCATION_LIST_MAX_NESTING_DEPTH_V1,
    )
}
#[cfg(any(target_os = "macos", target_os = "ios"))]
const REVOCATION_LIST_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
#[cfg(any(target_os = "linux", target_os = "android"))]
const REVOCATION_LIST_O_NOFOLLOW_FLAG: i32 = 0x0002_0000;
#[cfg(any(
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
const REVOCATION_LIST_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
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
compile_error!("SoraNet revocation-list loading requires a defined no-follow flag");
#[cfg(unix)]
type RevocationFileIdentity = (u64, u64);
#[cfg(windows)]
type RevocationFileIdentity = (Option<u32>, Option<u64>);
#[cfg(not(any(unix, windows)))]
type RevocationFileIdentity = ();
#[cfg(unix)]
fn revocation_file_identity(metadata: &FsMetadata) -> RevocationFileIdentity {
    use std::os::unix::fs::MetadataExt as _;
    (metadata.dev(), metadata.ino())
}
#[cfg(windows)]
fn revocation_file_identity(metadata: &FsMetadata) -> RevocationFileIdentity {
    use std::os::windows::fs::MetadataExt as _;
    (metadata.volume_serial_number(), metadata.file_index())
}
#[cfg(not(any(unix, windows)))]
fn revocation_file_identity(_metadata: &FsMetadata) -> RevocationFileIdentity {}
#[cfg(unix)]
const fn revocation_file_identity_available(_identity: RevocationFileIdentity) -> bool {
    true
}
#[cfg(windows)]
const fn revocation_file_identity_available(identity: RevocationFileIdentity) -> bool {
    identity.0.is_some() && identity.1.is_some()
}
#[cfg(not(any(unix, windows)))]
const fn revocation_file_identity_available(_identity: RevocationFileIdentity) -> bool {
    false
}
#[cfg(windows)]
fn revocation_file_is_reparse_point(metadata: &FsMetadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}
#[cfg(not(windows))]
fn revocation_file_is_reparse_point(_metadata: &FsMetadata) -> bool {
    false
}
fn validate_revocation_file_metadata(metadata: &FsMetadata) -> io::Result<()> {
    if metadata.file_type().is_symlink()
        || revocation_file_is_reparse_point(metadata)
        || !metadata.file_type().is_file()
        || !revocation_file_identity_available(revocation_file_identity(metadata))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "revocation list must be a direct regular file with a stable identity",
        ));
    }
    Ok(())
}
#[cfg(unix)]
fn open_revocation_file_direct(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(REVOCATION_LIST_O_NOFOLLOW_FLAG);
    options.open(path)
}
#[cfg(windows)]
fn open_revocation_file_direct(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(path)
}
#[cfg(not(any(unix, windows)))]
fn open_revocation_file_direct(_path: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "stable direct-file opens are unavailable on this platform",
    ))
}
#[cfg(unix)]
fn revocation_file_metadata_unchanged(left: &FsMetadata, right: &FsMetadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    revocation_file_identity(left) == revocation_file_identity(right)
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
        && left.mode() == right.mode()
}
#[cfg(windows)]
fn revocation_file_metadata_unchanged(left: &FsMetadata, right: &FsMetadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    revocation_file_identity_available(revocation_file_identity(left))
        && revocation_file_identity(left) == revocation_file_identity(right)
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
        && left.file_attributes() == right.file_attributes()
}
#[cfg(not(any(unix, windows)))]
fn revocation_file_metadata_unchanged(_left: &FsMetadata, _right: &FsMetadata) -> bool {
    false
}
#[cfg(test)]
static REVOCATION_FILE_READ_REPLACEMENT: std::sync::Mutex<
    Option<(std::path::PathBuf, std::path::PathBuf)>,
> = std::sync::Mutex::new(None);
#[cfg(test)]
fn replace_revocation_file_for_test(path: &Path) -> io::Result<()> {
    let replacement = {
        let mut hook = REVOCATION_FILE_READ_REPLACEMENT
            .lock()
            .expect("revocation file race hook lock");
        if hook.as_ref().is_some_and(|(expected, _)| expected == path) {
            hook.take().map(|(_, replacement)| replacement)
        } else {
            None
        }
    };
    if let Some(replacement) = replacement {
        fs::rename(replacement, path)?;
    }
    Ok(())
}
fn read_revocation_file_bounded(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    validate_revocation_file_metadata(&before)?;
    let maximum_u64 = u64::try_from(REVOCATION_LIST_MAX_FILE_BYTES_V1)
        .expect("fixed revocation-list limit fits u64");
    if before.len() > maximum_u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "revocation list is {} bytes; first-release limit is {REVOCATION_LIST_MAX_FILE_BYTES_V1} bytes",
                before.len()
            ),
        ));
    }
    #[cfg(test)]
    replace_revocation_file_for_test(path)?;
    let mut file = open_revocation_file_direct(path)?;
    let opened = file.metadata()?;
    validate_revocation_file_metadata(&opened)?;
    if !revocation_file_metadata_unchanged(&before, &opened) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "revocation list changed between inspection and open",
        ));
    }
    let expected_len = usize::try_from(opened.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "revocation-list length is not representable on this host",
        )
    })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected_len)
        .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
    bytes.resize(expected_len, 0);
    file.read_exact(&mut bytes).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "revocation list changed length while being read",
            )
        } else {
            error
        }
    })?;
    let mut growth_probe = [0_u8; 1];
    if file.read(&mut growth_probe)? != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "revocation list grew while being read or exceeds its first-release limit",
        ));
    }
    let after_file = file.metadata()?;
    let after_path = fs::symlink_metadata(path)?;
    validate_revocation_file_metadata(&after_file)?;
    validate_revocation_file_metadata(&after_path)?;
    if !revocation_file_metadata_unchanged(&opened, &after_file)
        || !revocation_file_metadata_unchanged(&opened, &after_path)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "revocation list changed while being read",
        ));
    }
    Ok(Some(bytes))
}
/// Request parameters for minting an admission token.
#[derive(Debug, Clone)]
pub struct MintRequest<'a> {
    /// ML-DSA suite used for signing.
    pub suite: MlDsaSuite,
    /// Issuer public key bytes.
    pub issuer_public_key: &'a [u8],
    /// Issuer secret key bytes.
    pub issuer_secret_key: &'a [u8],
    /// Relay identifier bound to the token.
    pub relay_id: [u8; 32],
    /// Resume transcript hash bound to the token.
    pub transcript_hash: [u8; 32],
    /// Token activation time.
    pub issued_at: SystemTime,
    /// Token expiry time.
    pub expires_at: SystemTime,
    /// Token flags (reserved for future use).
    pub flags: u8,
}
/// Metadata derived from a decoded admission token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenMetadata {
    pub token_id: [u8; 32],
    pub issuer_fingerprint: [u8; 32],
    pub relay_id: [u8; 32],
    pub transcript_hash: [u8; 32],
    pub issued_at: SystemTime,
    pub expires_at: SystemTime,
    pub flags: u8,
    pub signature_len: usize,
}
impl TokenMetadata {
    /// Compute the configured TTL for this token.
    #[must_use]
    pub fn ttl(&self) -> Duration {
        match self.expires_at.duration_since(self.issued_at) {
            Ok(ttl) => ttl,
            Err(_) => Duration::ZERO,
        }
    }
    fn issued_at_iso(&self) -> String {
        OffsetDateTime::from(self.issued_at)
            .format(&Rfc3339)
            .expect("RFC3339 format")
    }
    fn expires_at_iso(&self) -> String {
        OffsetDateTime::from(self.expires_at)
            .format(&Rfc3339)
            .expect("RFC3339 format")
    }
}
/// Minted token bundle containing the raw token and derived metadata.
/// Minted token bundle containing the raw token and derived metadata.
#[derive(Debug, Clone)]
pub struct TokenBundle {
    pub token: AdmissionToken,
    pub metadata: TokenMetadata,
}
impl TokenBundle {
    /// Create a bundle from a freshly minted or decoded token.
    fn new(token: AdmissionToken) -> Result<Self, TokenToolError> {
        let issued_at = token
            .checked_issued_at()
            .ok_or(TokenToolError::TimestampOutOfRange {
                field: "issued_at",
                value: token.issued_at(),
            })?;
        let expires_at = token
            .checked_expires_at()
            .ok_or(TokenToolError::TimestampOutOfRange {
                field: "expires_at",
                value: token.expires_at(),
            })?;
        let metadata = TokenMetadata {
            token_id: token.token_id(),
            issuer_fingerprint: *token.issuer_fingerprint(),
            relay_id: *token.relay_id(),
            transcript_hash: *token.transcript_hash(),
            issued_at,
            expires_at,
            flags: token.flags(),
            signature_len: token.signature().len(),
        };
        Ok(Self { token, metadata })
    }
    /// Serialise bundle details into a JSON value using Norito helpers.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let encoded = self.token.encode();
        let base64 = BASE64.encode(&encoded);
        let hex = hex::encode(&encoded);
        let ttl = self.metadata.ttl().as_secs();
        let mut object = json::Map::new();
        object.insert("token_base64".into(), Value::from(base64));
        object.insert("token_hex".into(), Value::from(hex));
        object.insert(
            "token_id_hex".into(),
            Value::from(hex::encode(self.metadata.token_id)),
        );
        object.insert(
            "issuer_fingerprint_hex".into(),
            Value::from(hex::encode(self.metadata.issuer_fingerprint)),
        );
        object.insert(
            "relay_id_hex".into(),
            Value::from(hex::encode(self.metadata.relay_id)),
        );
        object.insert(
            "transcript_hash_hex".into(),
            Value::from(hex::encode(self.metadata.transcript_hash)),
        );
        object.insert(
            "issued_at".into(),
            Value::from(self.metadata.issued_at_iso()),
        );
        object.insert(
            "expires_at".into(),
            Value::from(self.metadata.expires_at_iso()),
        );
        object.insert("ttl_secs".into(), Value::from(ttl));
        object.insert("flags".into(), Value::from(self.metadata.flags));
        object.insert(
            "signature_len".into(),
            Value::from(self.metadata.signature_len as u64),
        );
        Value::Object(object)
    }
}
/// Persistent revocation list backed by a JSON document.
#[derive(Debug, Clone, Default)]
pub struct RevocationList {
    // Sorted storage preserves the BTreeSet iteration contract without one
    // allocator call per hostile input entry.
    entries: Vec<[u8; REVOCATION_TOKEN_ID_BYTES]>,
}
impl RevocationList {
    /// Load a revocation list from disk. Missing files return an empty set.
    pub fn load_or_default(path: &Path) -> Result<Self, TokenToolError> {
        let Some(bytes) = read_revocation_file_bounded(path)? else {
            return Ok(Self::default());
        };
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        let profile = json::preflight_slice(&bytes, revocation_list_preflight_limits_v1())
            .map_err(TokenToolError::RevocationPreflight)?;
        validate_revocation_profile(profile)?;
        norito::with_decode_limits_scope(REVOCATION_LIST_DECODE_LIMITS_V1, || {
            decode_revocation_list(&bytes, profile.root_container_entries())
        })
    }
    /// Persist the revocation list to disk, creating parent directories if needed.
    pub fn write(&self, path: &Path) -> Result<(), TokenToolError> {
        let bytes = self.to_canonical_json_bytes()?;
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, bytes)?;
        Ok(())
    }
    /// Insert a token identifier into the revocation list.
    ///
    /// This compatibility helper returns `false` for duplicates or a bounded
    /// resource refusal. Call [`Self::try_insert`] when the distinction matters.
    pub fn insert(&mut self, token_id: [u8; 32]) -> bool {
        self.try_insert(token_id).unwrap_or(false)
    }
    /// Fallibly insert a token identifier under the first-release entry cap.
    pub fn try_insert(&mut self, token_id: [u8; 32]) -> Result<bool, TokenToolError> {
        let position = match self.entries.binary_search(&token_id) {
            Ok(_) => return Ok(false),
            Err(position) => position,
        };
        if self.entries.len() >= REVOCATION_LIST_MAX_ENTRIES_V1 {
            return Err(TokenToolError::RevocationCapacity {
                actual: self.entries.len().saturating_add(1),
                maximum: REVOCATION_LIST_MAX_ENTRIES_V1,
            });
        }
        self.entries
            .try_reserve(1)
            .map_err(|source| TokenToolError::RevocationAllocation {
                context: "token identifier insertion",
                source,
            })?;
        self.entries.insert(position, token_id);
        Ok(true)
    }
    /// Return the current entries sorted lexicographically.
    pub fn entries(&self) -> impl Iterator<Item = &[u8; 32]> {
        self.entries.iter()
    }
    fn to_canonical_json_bytes(&self) -> Result<Vec<u8>, TokenToolError> {
        if self.entries.len() > REVOCATION_LIST_MAX_ENTRIES_V1 {
            return Err(TokenToolError::RevocationCapacity {
                actual: self.entries.len(),
                maximum: REVOCATION_LIST_MAX_ENTRIES_V1,
            });
        }
        let expected_len = if self.entries.is_empty() {
            3
        } else {
            self.entries
                .len()
                .checked_mul(REVOCATION_LIST_CANONICAL_ENTRY_BYTES)
                .and_then(|length| length.checked_add(3))
                .ok_or(TokenToolError::RevocationCapacity {
                    actual: self.entries.len(),
                    maximum: REVOCATION_LIST_MAX_ENTRIES_V1,
                })?
        };
        if expected_len > REVOCATION_LIST_MAX_CANONICAL_BYTES_V1 {
            return Err(TokenToolError::RevocationCapacity {
                actual: self.entries.len(),
                maximum: REVOCATION_LIST_MAX_ENTRIES_V1,
            });
        }
        let mut output = Vec::new();
        output.try_reserve_exact(expected_len).map_err(|source| {
            TokenToolError::RevocationAllocation {
                context: "canonical JSON output",
                source,
            }
        })?;
        if self.entries.is_empty() {
            output.extend_from_slice(b"[]\n");
            return Ok(output);
        }
        // Token IDs contain only lowercase hexadecimal bytes, so this bounded
        // renderer exactly matches Norito's pretty JSON array layout without
        // materialising a second `Vec<String>` or `Value` tree.
        output.push(b'[');
        for (index, id) in self.entries.iter().enumerate() {
            if index != 0 {
                output.push(b',');
            }
            output.extend_from_slice(b"\n  \"");
            append_token_id_hex(&mut output, id);
            output.push(b'"');
        }
        output.extend_from_slice(b"\n]\n");
        debug_assert_eq!(output.len(), expected_len);
        Ok(output)
    }
}
fn validate_revocation_profile(profile: json::JsonPreflightProfile) -> Result<(), TokenToolError> {
    if profile.arrays() != 1
        || profile.objects() != 0
        || profile.array_entries() != profile.root_container_entries()
        || profile.values() != profile.root_container_entries().saturating_add(1)
    {
        return Err(TokenToolError::RevocationAdmission(
            "revocation list must be one flat JSON array of token-id strings",
        ));
    }
    let retained_bytes = profile
        .root_container_entries()
        .checked_mul(REVOCATION_TOKEN_ID_BYTES)
        .ok_or(TokenToolError::RevocationCapacity {
            actual: profile.root_container_entries(),
            maximum: REVOCATION_LIST_MAX_ENTRIES_V1,
        })?;
    if retained_bytes > REVOCATION_LIST_MAX_RETAINED_ID_BYTES_V1 {
        return Err(TokenToolError::RevocationCapacity {
            actual: profile.root_container_entries(),
            maximum: REVOCATION_LIST_MAX_ENTRIES_V1,
        });
    }
    Ok(())
}
fn decode_revocation_list(
    bytes: &[u8],
    entry_count: usize,
) -> Result<RevocationList, TokenToolError> {
    let input =
        std::str::from_utf8(bytes).map_err(|_| TokenToolError::Json(json::Error::InvalidUtf8))?;
    let mut parser = json::Parser::new(input);
    parser.expect(b'[')?;
    let mut entries = Vec::new();
    entries.try_reserve_exact(entry_count).map_err(|source| {
        TokenToolError::RevocationAllocation {
            context: "decoded token identifiers",
            source,
        }
    })?;
    let mut seen = HashSet::new();
    seen.try_reserve(entry_count)
        .map_err(|source| TokenToolError::RevocationAllocation {
            context: "duplicate detection",
            source,
        })?;
    for index in 0..entry_count {
        if index != 0 {
            parser.expect(b',')?;
        }
        let value = parser.parse_string()?;
        let id = parse_revocation_token_id(&value)?;
        if !seen.insert(id) {
            return Err(TokenToolError::DuplicateRevocation {
                index,
                token_id_hex: value,
            });
        }
        entries.push(id);
    }
    parser.expect(b']')?;
    parser.skip_ws();
    if !parser.eof() {
        return Err(TokenToolError::RevocationAdmission(
            "revocation list contains trailing JSON data",
        ));
    }
    entries.sort_unstable();
    Ok(RevocationList { entries })
}
fn parse_revocation_token_id(
    value: &str,
) -> Result<[u8; REVOCATION_TOKEN_ID_BYTES], TokenToolError> {
    if !value.len().is_multiple_of(2) {
        return Err(TokenToolError::Hex {
            field: "revocation_list",
            error: FromHexError::OddLength,
        });
    }
    for (index, byte) in value.bytes().enumerate() {
        if !byte.is_ascii_hexdigit() {
            return Err(TokenToolError::Hex {
                field: "revocation_list",
                error: FromHexError::InvalidHexCharacter {
                    c: char::from(byte),
                    index,
                },
            });
        }
    }
    let actual = value.len() / 2;
    if actual != REVOCATION_TOKEN_ID_BYTES {
        return Err(TokenToolError::InvalidLength {
            field: "revocation_list",
            expected: REVOCATION_TOKEN_ID_BYTES,
            actual,
        });
    }
    let mut id = [0_u8; REVOCATION_TOKEN_ID_BYTES];
    hex::decode_to_slice(value, &mut id).map_err(|error| TokenToolError::Hex {
        field: "revocation_list",
        error,
    })?;
    Ok(id)
}
fn append_token_id_hex(output: &mut Vec<u8>, id: &[u8; REVOCATION_TOKEN_ID_BYTES]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in id {
        let byte = *byte;
        output.push(HEX[usize::from(byte >> 4)]);
        output.push(HEX[usize::from(byte & 0x0f)]);
    }
}
/// Errors surfaced by token tooling helpers.
#[derive(Debug, Error)]
pub enum TokenToolError {
    #[error("invalid hex for {field}: {error}")]
    Hex {
        field: &'static str,
        #[source]
        error: FromHexError,
    },
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] json::Error),
    #[error("revocation-list JSON preflight failed: {0}")]
    RevocationPreflight(#[source] json::JsonPreflightError),
    #[error("revocation-list JSON admission failed: {0}")]
    RevocationAdmission(&'static str),
    #[error("revocation list contains {actual} entries; first-release limit is {maximum}")]
    RevocationCapacity { actual: usize, maximum: usize },
    #[error("failed to allocate bounded revocation-list storage for {context}: {source}")]
    RevocationAllocation {
        context: &'static str,
        #[source]
        source: TryReserveError,
    },
    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("RFC3339 parse error for {field}: {error}")]
    TimeParse {
        field: &'static str,
        #[source]
        error: time::error::Parse,
    },
    #[error("token mint error: {0}")]
    Mint(#[from] MintError),
    #[error("token decode error: {0}")]
    Decode(#[from] token::DecodeError),
    #[error("issued_at must be earlier than expires_at")]
    InvalidTemporalBounds,
    #[error("{field} timestamp {value} is out of range for system time")]
    TimestampOutOfRange { field: &'static str, value: u64 },
    #[error("expected {expected} bytes for {field}, got {actual}")]
    InvalidLength {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("duplicate token id in revocation list at index {index}: {token_id_hex}")]
    DuplicateRevocation { index: usize, token_id_hex: String },
}
/// Mint a token bundle using the provided RNG.
pub fn mint_token<R: RngCore + CryptoRng>(
    request: &MintRequest<'_>,
    rng: &mut R,
) -> Result<TokenBundle, TokenToolError> {
    ensure_temporal_bounds(request.issued_at, request.expires_at)?;
    let fingerprint = compute_issuer_fingerprint(request.issuer_public_key);
    let token = AdmissionToken::mint(
        request.suite,
        request.issuer_secret_key,
        fingerprint,
        request.relay_id,
        request.transcript_hash,
        request.issued_at,
        request.expires_at,
        request.flags,
        rng,
    )?;
    TokenBundle::new(token)
}
/// Decode a token frame and collect metadata.
pub fn inspect_token(bytes: &[u8]) -> Result<TokenBundle, TokenToolError> {
    let token = AdmissionToken::decode(bytes)?;
    TokenBundle::new(token)
}
/// Decode a base64 or hexadecimal token string.
pub fn decode_token_string(input: &str) -> Result<Vec<u8>, TokenToolError> {
    let trimmed = input.trim();
    let is_hex_candidate = trimmed.len().is_multiple_of(2)
        && !trimmed.is_empty()
        && trimmed.chars().all(|c| c.is_ascii_hexdigit());
    if is_hex_candidate {
        let bytes = parse_hex_bytes(trimmed, "token_hex")?;
        return Ok(bytes);
    }
    let decoded = BASE64.decode(trimmed).map_err(TokenToolError::Base64)?;
    Ok(decoded)
}
/// Parse an RFC3339 timestamp into `SystemTime`.
pub fn parse_rfc3339(value: &str, field: &'static str) -> Result<SystemTime, TokenToolError> {
    let dt = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|error| TokenToolError::TimeParse { field, error })?;
    Ok(SystemTime::from(dt))
}
/// Encode a token frame as base64.
#[must_use]
pub fn encode_token_base64(token: &AdmissionToken) -> String {
    BASE64.encode(token.encode())
}
/// Encode a token frame as hexadecimal.
#[must_use]
pub fn encode_token_hex(token: &AdmissionToken) -> String {
    hex::encode(token.encode())
}
/// Helper used by configuration parsing to load revocation IDs from disk.
pub fn read_revocation_file(path: &Path) -> Result<Vec<[u8; 32]>, TokenToolError> {
    let list = RevocationList::load_or_default(path)?;
    Ok(list.entries)
}
fn ensure_temporal_bounds(start: SystemTime, end: SystemTime) -> Result<(), TokenToolError> {
    if end <= start {
        return Err(TokenToolError::InvalidTemporalBounds);
    }
    Ok(())
}
pub fn parse_hex_array<const N: usize>(
    value: &str,
    field: &'static str,
) -> Result<[u8; N], TokenToolError> {
    let bytes = hex::decode(value).map_err(|error| TokenToolError::Hex { field, error })?;
    if bytes.len() != N {
        return Err(TokenToolError::InvalidLength {
            field,
            expected: N,
            actual: bytes.len(),
        });
    }
    let mut array = [0u8; N];
    array.copy_from_slice(&bytes);
    Ok(array)
}
pub fn parse_hex_bytes(value: &str, field: &'static str) -> Result<Vec<u8>, TokenToolError> {
    hex::decode(value).map_err(|error| TokenToolError::Hex { field, error })
}
#[cfg(test)]
mod tests {
    use std::time::UNIX_EPOCH;
    use rand::{SeedableRng, rngs::StdRng};
    use soranet_pq::generate_mldsa_keypair_from_os as generate_mldsa_keypair;
    use tempfile::tempdir;
    use super::*;
    const RELAY_ID: [u8; 32] = [0x45; 32];
    const TRANSCRIPT: [u8; 32] = [0xAB; 32];
    fn revocation_id(index: usize) -> [u8; REVOCATION_TOKEN_ID_BYTES] {
        let mut id = [0_u8; REVOCATION_TOKEN_ID_BYTES];
        id[REVOCATION_TOKEN_ID_BYTES - 8..].copy_from_slice(
            &u64::try_from(index)
                .expect("test index fits u64")
                .to_be_bytes(),
        );
        id
    }
    fn encoded_token_with_times(issued_at: u64, expires_at: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"SNTK");
        bytes.push(AdmissionToken::VERSION);
        bytes.push(0);
        bytes.extend_from_slice(&issued_at.to_be_bytes());
        bytes.extend_from_slice(&expires_at.to_be_bytes());
        bytes.extend_from_slice(&RELAY_ID);
        bytes.extend_from_slice(&TRANSCRIPT);
        bytes.extend_from_slice(&[0xAA; 16]);
        bytes.extend_from_slice(&[0xBB; 32]);
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.push(0xCC);
        bytes
    }
    #[test]
    fn mint_and_inspect_round_trip() {
        let keypair = generate_mldsa_keypair(MlDsaSuite::MlDsa44).expect("keypair");
        let issued_at = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        let expires_at = issued_at + Duration::from_secs(600);
        let mut rng = StdRng::seed_from_u64(0xDEADBEEF);
        let request = MintRequest {
            suite: MlDsaSuite::MlDsa44,
            issuer_public_key: keypair.public_key(),
            issuer_secret_key: keypair.secret_key(),
            relay_id: RELAY_ID,
            transcript_hash: TRANSCRIPT,
            issued_at,
            expires_at,
            flags: 0,
        };
        let bundle = mint_token(&request, &mut rng).expect("mint");
        assert_eq!(bundle.metadata.relay_id, RELAY_ID);
        assert_eq!(bundle.metadata.transcript_hash, TRANSCRIPT);
        assert_eq!(bundle.metadata.flags, 0);
        assert_eq!(bundle.metadata.ttl(), Duration::from_secs(600));
        let encoded = bundle.token.encode();
        let decoded = inspect_token(&encoded).expect("inspect");
        assert_eq!(bundle.metadata, decoded.metadata);
    }
    #[test]
    fn inspect_rejects_unrepresentable_token_timestamps_without_panic() {
        let err = inspect_token(&encoded_token_with_times(10, u64::MAX))
            .expect_err("unrepresentable expires_at should fail closed");
        match err {
            TokenToolError::Decode(token::DecodeError::TimestampOutOfRange { field, value }) => {
                assert_eq!(field, "expires_at");
                assert_eq!(value, u64::MAX);
            }
            other => panic!("expected timestamp decode error, got {other:?}"),
        }
    }
    #[test]
    fn revocation_list_round_trip() {
        let dir = tempdir().expect("tmp");
        let path = dir.path().join("revocations.json");
        let mut list = RevocationList::default();
        list.insert([0x11; 32]);
        list.insert([0x22; 32]);
        list.write(&path).expect("write");
        let loaded = RevocationList::load_or_default(&path).expect("load");
        assert_eq!(
            loaded.entries().map(hex::encode).collect::<Vec<_>>(),
            vec![hex::encode([0x11; 32]), hex::encode([0x22; 32])]
        );
        assert_eq!(
            list.to_canonical_json_bytes().expect("render list"),
            format!(
                "[\n  \"{}\",\n  \"{}\"\n]\n",
                hex::encode([0x11; 32]),
                hex::encode([0x22; 32])
            )
            .into_bytes()
        );
    }
    #[test]
    fn revocation_file_limit_accepts_exact_and_rejects_plus_one() {
        let dir = tempdir().expect("tmp");
        let exact = dir.path().join("exact.json");
        let mut bytes = b"[]".to_vec();
        bytes.resize(REVOCATION_LIST_MAX_FILE_BYTES_V1, b' ');
        std::fs::write(&exact, bytes).expect("write exact file");
        assert_eq!(
            RevocationList::load_or_default(&exact)
                .expect("exact file limit must load")
                .entries()
                .count(),
            0
        );
        let plus_one = dir.path().join("plus-one.json");
        let file = File::create(&plus_one).expect("create oversized file");
        file.set_len(
            u64::try_from(REVOCATION_LIST_MAX_FILE_BYTES_V1 + 1)
                .expect("fixed file limit fits u64"),
        )
        .expect("size oversized file");
        let error =
            RevocationList::load_or_default(&plus_one).expect_err("file limit + 1 must fail");
        assert!(
            matches!(error, TokenToolError::Io(ref source) if source.kind() == io::ErrorKind::InvalidData),
            "unexpected error: {error:?}"
        );
    }
    #[cfg(unix)]
    #[test]
    fn revocation_list_rejects_symlink_input() {
        use std::os::unix::fs::symlink;
        let dir = tempdir().expect("tmp");
        let target = dir.path().join("target.json");
        let link = dir.path().join("link.json");
        std::fs::write(&target, b"[]").expect("write target");
        symlink(&target, &link).expect("create symlink");
        let error = RevocationList::load_or_default(&link).expect_err("symlink must fail");
        assert!(
            matches!(error, TokenToolError::Io(ref source) if source.kind() == io::ErrorKind::InvalidData),
            "unexpected error: {error:?}"
        );
    }
    #[cfg(unix)]
    #[test]
    fn revocation_list_rejects_path_replacement_race() {
        let dir = tempdir().expect("tmp");
        let configured = dir.path().join("revocations.json");
        let replacement = dir.path().join("replacement.json");
        std::fs::write(&configured, b"[]").expect("write configured file");
        std::fs::write(&replacement, b"[]").expect("write replacement file");
        *REVOCATION_FILE_READ_REPLACEMENT
            .lock()
            .expect("race hook lock") = Some((configured.clone(), replacement));
        let error =
            RevocationList::load_or_default(&configured).expect_err("path replacement must fail");
        assert!(
            matches!(error, TokenToolError::Io(ref source) if source.kind() == io::ErrorKind::InvalidData),
            "unexpected error: {error:?}"
        );
    }
    #[test]
    fn revocation_preflight_enforces_depth_count_and_string_limits() {
        let dir = tempdir().expect("tmp");
        let deep = dir.path().join("deep.json");
        std::fs::write(&deep, b"[[\"00\"]]").expect("write deep input");
        assert!(matches!(
            RevocationList::load_or_default(&deep),
            Err(TokenToolError::RevocationPreflight(_))
        ));
        let long_string = dir.path().join("long-string.json");
        std::fs::write(
            &long_string,
            format!("[\"{}\"]", "a".repeat(REVOCATION_TOKEN_ID_HEX_BYTES + 1)),
        )
        .expect("write long string input");
        assert!(matches!(
            RevocationList::load_or_default(&long_string),
            Err(TokenToolError::RevocationPreflight(_))
        ));
        let too_many = dir.path().join("too-many.json");
        let token = format!("\"{}\"", "00".repeat(REVOCATION_TOKEN_ID_BYTES));
        let body = format!(
            "[{}]",
            std::iter::repeat_n(token, REVOCATION_LIST_MAX_ENTRIES_V1 + 1)
                .collect::<Vec<_>>()
                .join(",")
        );
        assert!(body.len() < REVOCATION_LIST_MAX_FILE_BYTES_V1);
        std::fs::write(&too_many, body).expect("write excessive count input");
        assert!(matches!(
            RevocationList::load_or_default(&too_many),
            Err(TokenToolError::RevocationPreflight(_))
        ));
    }
    #[test]
    fn revocation_preflight_accepts_maximally_escaped_exact_string() {
        let dir = tempdir().expect("tmp");
        let path = dir.path().join("escaped.json");
        let encoded = "\\u0061".repeat(REVOCATION_TOKEN_ID_HEX_BYTES);
        assert_eq!(
            encoded.len() + 2,
            REVOCATION_LIST_MAX_ENCODED_STRING_BYTES_V1
        );
        std::fs::write(&path, format!("[\"{encoded}\"]")).expect("write escaped id");
        let loaded = RevocationList::load_or_default(&path).expect("escaped exact string loads");
        assert_eq!(
            loaded.entries().copied().collect::<Vec<_>>(),
            vec![[0xaa; 32]]
        );
    }
    #[test]
    fn revocation_duplicate_error_preserves_input_index_and_spelling() {
        let dir = tempdir().expect("tmp");
        let path = dir.path().join("duplicate.json");
        let duplicate = "AA".repeat(REVOCATION_TOKEN_ID_BYTES);
        std::fs::write(
            &path,
            format!(
                "[\"{duplicate}\",\"{}\",\"{duplicate}\"]",
                "bb".repeat(REVOCATION_TOKEN_ID_BYTES)
            ),
        )
        .expect("write duplicate list");
        let error = RevocationList::load_or_default(&path).expect_err("duplicate must fail");
        assert!(
            matches!(error, TokenToolError::DuplicateRevocation { index: 2, ref token_id_hex } if token_id_hex == &duplicate),
            "unexpected error: {error:?}"
        );
    }
    #[test]
    fn revocation_producer_accepts_exact_count_and_rejects_plus_one() {
        let mut list = RevocationList::default();
        for index in 0..REVOCATION_LIST_MAX_ENTRIES_V1 {
            assert!(
                list.try_insert(revocation_id(index))
                    .expect("bounded insertion")
            );
        }
        let bytes = list
            .to_canonical_json_bytes()
            .expect("render exact-count list");
        assert_eq!(bytes.len(), REVOCATION_LIST_MAX_CANONICAL_BYTES_V1);
        let dir = tempdir().expect("tmp");
        let path = dir.path().join("exact-count.json");
        std::fs::write(&path, bytes).expect("write exact-count list");
        assert_eq!(
            RevocationList::load_or_default(&path)
                .expect("exact-count list must load")
                .entries()
                .count(),
            REVOCATION_LIST_MAX_ENTRIES_V1
        );
        let error = list
            .try_insert(revocation_id(REVOCATION_LIST_MAX_ENTRIES_V1))
            .expect_err("entry count + 1 must fail");
        assert!(matches!(
            error,
            TokenToolError::RevocationCapacity {
                actual,
                maximum: REVOCATION_LIST_MAX_ENTRIES_V1,
            } if actual == REVOCATION_LIST_MAX_ENTRIES_V1 + 1
        ));
    }
    #[test]
    fn decode_token_string_accepts_hex() {
        let bytes = vec![0xAA, 0xBB, 0xCC];
        let hex = hex::encode(&bytes);
        let decoded = decode_token_string(&hex).expect("decode");
        assert_eq!(decoded, bytes);
    }
    #[test]
    fn decode_token_string_accepts_base64() {
        let bytes = vec![1u8, 2, 3, 4];
        let b64 = BASE64.encode(&bytes);
        let decoded = decode_token_string(&b64).expect("decode");
        assert_eq!(decoded, bytes);
    }
}
