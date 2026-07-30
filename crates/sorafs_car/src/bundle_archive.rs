//! Strict, bounded reader for native-bundle gzip/USTAR archives.
//!
//! The reader validates framing as it advances and returns success only after
//! validating the complete archive. Each file payload is streamed through a
//! size-bounded visitor, so callers can materialize entries without retaining
//! the decoded archive in memory. Paths are returned as validated components
//! so callers never need to split or normalize untrusted archive names.

use std::{
    collections::BTreeMap,
    error::Error as StdError,
    fmt,
    io::{self, Read, Write},
};

use flate2::{Compression, GzBuilder, bufread::GzDecoder};
use thiserror::Error;

const TAR_BLOCK_BYTES: usize = 512;
const TAR_BLOCK_BYTES_U64: u64 = TAR_BLOCK_BYTES as u64;
const MAX_PORTABLE_COMPONENT_BYTES: usize = 255;
const MAX_PORTABLE_PATH_BYTES: usize = 255;
const MAX_CANONICAL_USTAR_SIZE: u64 = 0o77_777_777_777;
const CANONICAL_GZIP_HEADER: [u8; 10] =
    [0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff];

/// Hard protocol ceiling for compressed bundle-archive bytes.
pub const BUNDLE_ARCHIVE_PROTOCOL_MAX_COMPRESSED_BYTES: u64 = 512 * 1024 * 1024;
/// Hard protocol ceiling for decoded gzip/USTAR bytes.
pub const BUNDLE_ARCHIVE_PROTOCOL_MAX_DECODED_BYTES: u64 = 3 * 1024 * 1024 * 1024;
/// Hard protocol ceiling for archive entries.
pub const BUNDLE_ARCHIVE_PROTOCOL_MAX_ENTRIES: u32 = 65_536;
/// Hard protocol ceiling for one regular-file payload.
pub const BUNDLE_ARCHIVE_PROTOCOL_MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
/// Hard protocol ceiling for aggregate regular-file payload bytes.
pub const BUNDLE_ARCHIVE_PROTOCOL_MAX_TOTAL_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Resource limits applied while reading a native-bundle archive.
///
/// All limits must be non-zero. `max_file_bytes` may not exceed
/// `max_total_file_bytes`, and `max_total_file_bytes` may not exceed
/// `max_decoded_bytes`. Every field is also bounded by the immutable protocol
/// ceilings exported by this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BundleArchiveLimits {
    /// Maximum number of compressed bytes accepted from the input reader.
    pub max_compressed_bytes: u64,
    /// Maximum number of bytes emitted by the gzip decoder, including USTAR
    /// headers, padding, and terminator blocks.
    pub max_decoded_bytes: u64,
    /// Maximum number of file and directory headers in the archive.
    pub max_entries: u32,
    /// Maximum declared size of any one regular file.
    pub max_file_bytes: u64,
    /// Maximum sum of all declared regular-file sizes.
    pub max_total_file_bytes: u64,
}

impl BundleArchiveLimits {
    fn validate(self) -> Result<Self, BundleArchiveError> {
        if self.max_compressed_bytes == 0 {
            return Err(
                ArchiveError::InvalidLimits("max_compressed_bytes must be non-zero").into(),
            );
        }
        if self.max_compressed_bytes > BUNDLE_ARCHIVE_PROTOCOL_MAX_COMPRESSED_BYTES {
            return Err(ArchiveError::InvalidLimits(
                "max_compressed_bytes exceeds the protocol ceiling",
            )
            .into());
        }
        if self.max_decoded_bytes == 0 {
            return Err(ArchiveError::InvalidLimits("max_decoded_bytes must be non-zero").into());
        }
        if self.max_decoded_bytes > BUNDLE_ARCHIVE_PROTOCOL_MAX_DECODED_BYTES {
            return Err(ArchiveError::InvalidLimits(
                "max_decoded_bytes exceeds the protocol ceiling",
            )
            .into());
        }
        if self.max_entries == 0 {
            return Err(ArchiveError::InvalidLimits("max_entries must be non-zero").into());
        }
        if self.max_entries > BUNDLE_ARCHIVE_PROTOCOL_MAX_ENTRIES {
            return Err(ArchiveError::InvalidLimits(
                "max_entries exceeds the protocol ceiling of 65536",
            )
            .into());
        }
        if self.max_file_bytes == 0 {
            return Err(ArchiveError::InvalidLimits("max_file_bytes must be non-zero").into());
        }
        if self.max_file_bytes > BUNDLE_ARCHIVE_PROTOCOL_MAX_FILE_BYTES {
            return Err(
                ArchiveError::InvalidLimits("max_file_bytes exceeds the protocol ceiling").into(),
            );
        }
        if self.max_total_file_bytes == 0 {
            return Err(
                ArchiveError::InvalidLimits("max_total_file_bytes must be non-zero").into(),
            );
        }
        if self.max_total_file_bytes > BUNDLE_ARCHIVE_PROTOCOL_MAX_TOTAL_FILE_BYTES {
            return Err(ArchiveError::InvalidLimits(
                "max_total_file_bytes exceeds the protocol ceiling",
            )
            .into());
        }
        if self.max_file_bytes > self.max_total_file_bytes {
            return Err(ArchiveError::InvalidLimits(
                "max_file_bytes must not exceed max_total_file_bytes",
            )
            .into());
        }
        if self.max_total_file_bytes > self.max_decoded_bytes {
            return Err(ArchiveError::InvalidLimits(
                "max_total_file_bytes must not exceed max_decoded_bytes",
            )
            .into());
        }
        Ok(self)
    }
}

/// Kind of a validated native-bundle archive entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleArchiveEntryKind {
    /// A regular file with a streamed payload.
    File,
    /// A directory with a zero-length payload.
    Directory,
}

/// Metadata for one validated native-bundle archive entry.
///
/// The entry borrows no decoder state and is valid only for the duration of
/// the visitor call. Its path and path components are already validated and
/// require no caller-side normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleArchiveEntry {
    path: String,
    path_components: Vec<String>,
    kind: BundleArchiveEntryKind,
    mode: u32,
    size: u64,
}

impl BundleArchiveEntry {
    /// Return the canonical slash-separated relative path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Return the validated path components.
    #[must_use]
    pub fn path_components(&self) -> &[String] {
        &self.path_components
    }

    /// Return the entry kind.
    #[must_use]
    pub fn kind(&self) -> BundleArchiveEntryKind {
        self.kind
    }

    /// Return the canonical USTAR permission mode.
    #[must_use]
    pub fn mode(&self) -> u32 {
        self.mode
    }

    /// Return the declared payload size.
    ///
    /// Directory entries always have size zero.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }
}

/// Counts and byte totals for a fully validated archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BundleArchiveSummary {
    compressed_bytes: u64,
    decoded_bytes: u64,
    entry_count: u32,
    file_count: u32,
    total_file_bytes: u64,
}

impl BundleArchiveSummary {
    /// Return the number of compressed input bytes.
    #[must_use]
    pub fn compressed_bytes(&self) -> u64 {
        self.compressed_bytes
    }

    /// Return the number of decoded bytes, including USTAR framing.
    #[must_use]
    pub fn decoded_bytes(&self) -> u64 {
        self.decoded_bytes
    }

    /// Return the number of file and directory entries.
    #[must_use]
    pub fn entry_count(&self) -> u32 {
        self.entry_count
    }

    /// Return the number of regular-file entries.
    #[must_use]
    pub fn file_count(&self) -> u32 {
        self.file_count
    }

    /// Return the sum of all regular-file payload sizes.
    #[must_use]
    pub fn total_file_bytes(&self) -> u64 {
        self.total_file_bytes
    }
}

/// One borrowed regular file for deterministic canonical bundle encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BundleArchiveFile<'a> {
    path: &'a str,
    mode: u32,
    payload: &'a [u8],
}

impl<'a> BundleArchiveFile<'a> {
    /// Describe one regular file.
    ///
    /// Validation is deferred to [`write_gzip_ustar`]. `mode` must be either
    /// `0o644` or `0o755`.
    #[must_use]
    pub const fn new(path: &'a str, mode: u32, payload: &'a [u8]) -> Self {
        Self {
            path,
            mode,
            payload,
        }
    }

    /// Return the canonical relative archive path.
    #[must_use]
    pub const fn path(&self) -> &'a str {
        self.path
    }

    /// Return the canonical permission mode.
    #[must_use]
    pub const fn mode(&self) -> u32 {
        self.mode
    }

    /// Return the borrowed file payload.
    #[must_use]
    pub const fn payload(&self) -> &'a [u8] {
        self.payload
    }
}

/// Encode regular files as one deterministic canonical gzip/USTAR bundle.
///
/// Entries are validated and sorted by their canonical relative path before
/// any output is written. Parent directories are implicit; the archive
/// contains regular-file headers only. The gzip header, compression level,
/// USTAR metadata, numeric fields, padding, and two-block terminator match the
/// profile accepted by [`visit_gzip_ustar`].
///
/// # Errors
///
/// Returns [`io::ErrorKind::InvalidInput`] when there are no files, a path or
/// mode is non-canonical, two paths collide, a file is another file's parent,
/// or a payload is too large for the canonical USTAR size field. I/O and
/// compression failures are returned unchanged.
pub fn write_gzip_ustar<W: Write>(writer: W, files: &[BundleArchiveFile<'_>]) -> io::Result<W> {
    let ordered = validate_bundle_archive_files_for_write(files)?;
    let mut gzip = GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(writer, Compression::new(6));
    let zero_block = [0_u8; TAR_BLOCK_BYTES];
    for file in ordered {
        let header = canonical_file_header(file)?;
        gzip.write_all(&header)?;
        gzip.write_all(file.payload)?;
        let padding = (TAR_BLOCK_BYTES - file.payload.len() % TAR_BLOCK_BYTES) % TAR_BLOCK_BYTES;
        gzip.write_all(&zero_block[..padding])?;
    }
    gzip.write_all(&zero_block)?;
    gzip.write_all(&zero_block)?;
    gzip.finish()
}

fn validate_bundle_archive_files_for_write<'a>(
    files: &'a [BundleArchiveFile<'a>],
) -> io::Result<Vec<&'a BundleArchiveFile<'a>>> {
    if files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "canonical bundle archive must contain at least one regular file",
        ));
    }
    let mut ordered = Vec::<&BundleArchiveFile<'a>>::new();
    ordered.try_reserve_exact(files.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::OutOfMemory,
            "failed to reserve canonical bundle archive file index",
        )
    })?;
    ordered.extend(files);
    ordered.sort_unstable_by(|left, right| left.path.cmp(right.path));

    let mut paths = BTreeMap::<Vec<String>, PathRecord>::new();
    for (index, file) in ordered.iter().enumerate() {
        if !matches!(file.mode, 0o644 | 0o755) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "canonical bundle archive file `{}` mode must be 0644 or 0755",
                    file.path
                ),
            ));
        }
        let entry_number = u32::try_from(index + 1).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "canonical bundle archive exceeds the protocol entry ceiling",
            )
        })?;
        if entry_number > BUNDLE_ARCHIVE_PROTOCOL_MAX_ENTRIES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "canonical bundle archive exceeds the protocol entry ceiling",
            ));
        }
        let (path, path_components) =
            validate_path(file.path.as_bytes(), entry_number).map_err(invalid_write_input)?;
        let size = u64::try_from(file.payload.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("canonical bundle archive file `{}` is too large", file.path),
            )
        })?;
        if size > MAX_CANONICAL_USTAR_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "canonical bundle archive file `{}` exceeds the USTAR size field",
                    file.path
                ),
            ));
        }
        let entry = BundleArchiveEntry {
            path,
            path_components,
            kind: BundleArchiveEntryKind::File,
            mode: file.mode,
            size,
        };
        check_path_conflicts(&paths, &entry).map_err(invalid_write_input)?;
        insert_path(&mut paths, &entry);
    }
    Ok(ordered)
}

fn invalid_write_input(error: BundleArchiveError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error.to_string())
}

fn canonical_file_header(file: &BundleArchiveFile<'_>) -> io::Result<[u8; TAR_BLOCK_BYTES]> {
    let path = file.path.as_bytes();
    let (prefix, name) = canonical_ustar_split(path).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "canonical bundle archive path `{}` cannot be represented in USTAR",
                file.path
            ),
        )
    })?;
    let mut header = [0_u8; TAR_BLOCK_BYTES];
    header[..name.len()].copy_from_slice(name);
    header[345..345 + prefix.len()].copy_from_slice(prefix);
    write_canonical_octal(&mut header[100..108], u64::from(file.mode))?;
    write_canonical_octal(&mut header[108..116], 0)?;
    write_canonical_octal(&mut header[116..124], 0)?;
    write_canonical_octal(
        &mut header[124..136],
        u64::try_from(file.payload.len()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "file size does not fit u64")
        })?,
    )?;
    write_canonical_octal(&mut header[136..148], 0)?;
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    write_canonical_octal(&mut header[329..337], 0)?;
    write_canonical_octal(&mut header[337..345], 0)?;
    header[148..156].fill(b' ');
    let checksum: u64 = header.iter().map(|byte| u64::from(*byte)).sum();
    let rendered = format!("{checksum:06o}\0 ");
    if rendered.len() != 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "canonical USTAR checksum does not fit its field",
        ));
    }
    header[148..156].copy_from_slice(rendered.as_bytes());
    Ok(header)
}

fn write_canonical_octal(field: &mut [u8], value: u64) -> io::Result<()> {
    let digits = field.len().checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "canonical USTAR numeric field is empty",
        )
    })?;
    let rendered = format!("{value:0digits$o}");
    if rendered.len() != digits {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "value does not fit canonical USTAR numeric field",
        ));
    }
    field[..digits].copy_from_slice(rendered.as_bytes());
    field[digits] = 0;
    Ok(())
}

/// Error returned when a native-bundle archive is malformed, unsafe, or over
/// its configured limits.
///
/// The concrete error categories are intentionally private so callers cannot
/// accidentally treat an archive-policy failure as recoverable based on
/// unstable parser internals. The display text includes the rejected entry or
/// limit where it is safe to do so.
#[derive(Debug)]
pub struct BundleArchiveError(ArchiveError);

impl fmt::Display for BundleArchiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl StdError for BundleArchiveError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0.source()
    }
}

impl From<ArchiveError> for BundleArchiveError {
    fn from(error: ArchiveError) -> Self {
        Self(error)
    }
}

#[derive(Debug, Error)]
enum ArchiveError {
    #[error("invalid bundle archive limits: {0}")]
    InvalidLimits(&'static str),
    #[error("failed to read compressed bundle archive: {0}")]
    CompressedRead(#[source] io::Error),
    #[error("compressed bundle archive exceeds {limit} bytes")]
    CompressedLimit { limit: u64 },
    #[error("failed to reserve memory for compressed bundle archive")]
    CompressedAllocation,
    #[error("bundle archive must use the canonical gzip header 1f8b08000000000000ff")]
    InvalidGzipHeader,
    #[error("failed to decode bundle archive while reading {context}: {source}")]
    DecodedRead {
        context: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("bundle archive is truncated while reading {0}")]
    Truncated(&'static str),
    #[error("bundle archive has more than {limit} entries")]
    EntryLimit { limit: u32 },
    #[error("USTAR header {entry} is invalid: {reason}")]
    InvalidHeader { entry: u32, reason: &'static str },
    #[error("USTAR header {entry} checksum mismatch: declared {declared:o}, computed {computed:o}")]
    HeaderChecksum {
        entry: u32,
        declared: u64,
        computed: u64,
    },
    #[error("USTAR header {entry} uses unsupported type flag 0x{type_flag:02x}")]
    UnsupportedType { entry: u32, type_flag: u8 },
    #[error("USTAR header {entry} has an unsafe path: {reason}")]
    UnsafePath { entry: u32, reason: &'static str },
    #[error("duplicate bundle archive path `{0}`")]
    DuplicatePath(String),
    #[error("bundle archive path `{path}` is not strictly after `{previous}`")]
    PathOrder { path: String, previous: String },
    #[error("bundle archive path `{path}` has an ASCII case-fold collision with `{existing}`")]
    CaseFoldCollision { path: String, existing: String },
    #[error("bundle archive path `{path}` conflicts with regular file `{file}`")]
    FileParentConflict { path: String, file: String },
    #[error("bundle archive file `{path}` is {size} bytes; maximum is {limit}")]
    FileLimit { path: String, size: u64, limit: u64 },
    #[error("bundle archive files total {size} bytes; maximum is {limit}")]
    TotalFileLimit { size: u64, limit: u64 },
    #[error("visitor failed for bundle archive path `{path}`: {source}")]
    Visitor {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("visitor left {remaining} unread payload bytes for bundle archive path `{path}`")]
    PayloadNotConsumed { path: String, remaining: u64 },
    #[error("bundle archive path `{0}` has non-zero USTAR payload padding")]
    NonZeroPadding(String),
    #[error("bundle archive has only one zero USTAR terminator block")]
    IncompleteTerminator,
    #[error("bundle archive must contain at least one entry")]
    EmptyArchive,
    #[error("bundle archive must contain at least one regular file")]
    NoRegularFiles,
    #[error("bundle archive has decoded data after exactly two USTAR terminator blocks")]
    TarTrailer,
    #[error("gzip stream is followed by {bytes} trailing compressed bytes")]
    TrailingCompressedData { bytes: usize },
}

/// Validate and stream a gzip-compressed canonical USTAR native bundle.
///
/// The compressed input is buffered only up to `limits.max_compressed_bytes`
/// so that the parser can prove the gzip stream has exactly one member with the
/// fixed ten-byte header `1f8b08000000000000ff`. The accepted USTAR profile
/// permits only regular files (`0644` or `0755`) and directories (`0755`), with
/// zero identity, timestamp, device, link, user-name, group-name, and reserved
/// fields. Paths use one deterministic name/prefix split and strict
/// lexicographic order. Exactly two zero terminator blocks end an archive that
/// contains at least one regular file.
///
/// Decoded data is never buffered as a whole. For each validated entry,
/// `visitor` receives a reader capped at the entry's declared size. The visitor
/// must read that reader to EOF; returning successfully with unread bytes
/// rejects the archive. Directory readers are empty.
///
/// The visitor should create filesystem objects relative to an already-open,
/// trusted root by walking [`BundleArchiveEntry::path_components`]. It should
/// not reconstruct a host path from the raw display path. A filesystem visitor
/// must use no-follow, component-relative operations and exclusive file
/// creation so a pre-existing link cannot redirect extraction. Because the
/// visitor runs while later entries are still being validated, side effects
/// must target a disposable staging area that is published only after this
/// function returns `Ok`.
///
/// # Errors
///
/// Returns an error when a resource limit is invalid or exceeded, the gzip or
/// USTAR representation is non-canonical, an entry path is unsafe or
/// conflicting, the visitor fails or leaves payload bytes unread, or the
/// decoder does not finish at the exact end of the sole gzip member.
pub fn visit_gzip_ustar<R, F>(
    reader: R,
    limits: BundleArchiveLimits,
    mut visitor: F,
) -> Result<BundleArchiveSummary, BundleArchiveError>
where
    R: Read,
    F: FnMut(&BundleArchiveEntry, &mut dyn Read) -> io::Result<()>,
{
    let limits = limits.validate()?;
    let compressed = read_compressed(reader, limits.max_compressed_bytes)?;
    validate_gzip_header(&compressed)?;
    let compressed_bytes =
        u64::try_from(compressed.len()).map_err(|_| ArchiveError::CompressedAllocation)?;

    let gzip = GzDecoder::new(compressed.as_slice());
    let mut decoded = LimitedDecodedReader::new(gzip, limits.max_decoded_bytes);
    let state = visit_ustar_entries(&mut decoded, limits, &mut visitor)?;

    let decoded_bytes = decoded.bytes_read();
    let gzip = decoded.into_inner();
    let trailing = gzip.into_inner();
    if !trailing.is_empty() {
        return Err(ArchiveError::TrailingCompressedData {
            bytes: trailing.len(),
        }
        .into());
    }

    Ok(BundleArchiveSummary {
        compressed_bytes,
        decoded_bytes,
        entry_count: state.entry_count,
        file_count: state.file_count,
        total_file_bytes: state.total_file_bytes,
    })
}

#[derive(Debug, Default)]
struct ArchiveState {
    paths: BTreeMap<Vec<String>, PathRecord>,
    previous_path: Option<String>,
    entry_count: u32,
    file_count: u32,
    total_file_bytes: u64,
}

fn visit_ustar_entries<R, F>(
    decoded: &mut R,
    limits: BundleArchiveLimits,
    visitor: &mut F,
) -> Result<ArchiveState, BundleArchiveError>
where
    R: Read,
    F: FnMut(&BundleArchiveEntry, &mut dyn Read) -> io::Result<()>,
{
    let mut state = ArchiveState::default();
    loop {
        let mut header = [0_u8; TAR_BLOCK_BYTES];
        read_exact_decoded(decoded, &mut header, "USTAR header")?;
        if is_zero_block(&header) {
            validate_terminator(decoded, &state)?;
            return Ok(state);
        }

        if state.entry_count == limits.max_entries {
            return Err(ArchiveError::EntryLimit {
                limit: limits.max_entries,
            }
            .into());
        }
        let entry_number = state
            .entry_count
            .checked_add(1)
            .ok_or(ArchiveError::EntryLimit {
                limit: limits.max_entries,
            })?;
        let entry = parse_header(&header, entry_number)?;
        validate_entry(&state, &entry, limits)?;
        visit_entry_payload(decoded, &entry, visitor)?;
        read_payload_padding(decoded, &entry)?;
        state.commit(entry, entry_number);
    }
}

fn validate_terminator<R: Read>(
    decoded: &mut R,
    state: &ArchiveState,
) -> Result<(), BundleArchiveError> {
    if state.entry_count == 0 {
        return Err(ArchiveError::EmptyArchive.into());
    }
    if state.file_count == 0 {
        return Err(ArchiveError::NoRegularFiles.into());
    }
    let mut second = [0_u8; TAR_BLOCK_BYTES];
    read_exact_decoded(decoded, &mut second, "second USTAR terminator block")?;
    if !is_zero_block(&second) {
        return Err(ArchiveError::IncompleteTerminator.into());
    }
    require_decoded_eof(decoded)
}

fn validate_entry(
    state: &ArchiveState,
    entry: &BundleArchiveEntry,
    limits: BundleArchiveLimits,
) -> Result<(), BundleArchiveError> {
    check_path_conflicts(&state.paths, entry)?;
    if let Some(previous) = &state.previous_path {
        if entry.path.as_str() <= previous.as_str() {
            return Err(ArchiveError::PathOrder {
                path: entry.path.clone(),
                previous: previous.clone(),
            }
            .into());
        }
    }
    if entry.kind != BundleArchiveEntryKind::File {
        return Ok(());
    }
    if entry.size > limits.max_file_bytes {
        return Err(ArchiveError::FileLimit {
            path: entry.path.clone(),
            size: entry.size,
            limit: limits.max_file_bytes,
        }
        .into());
    }
    let total =
        state
            .total_file_bytes
            .checked_add(entry.size)
            .ok_or(ArchiveError::TotalFileLimit {
                size: u64::MAX,
                limit: limits.max_total_file_bytes,
            })?;
    if total > limits.max_total_file_bytes {
        return Err(ArchiveError::TotalFileLimit {
            size: total,
            limit: limits.max_total_file_bytes,
        }
        .into());
    }
    Ok(())
}

fn visit_entry_payload<R, F>(
    decoded: &mut R,
    entry: &BundleArchiveEntry,
    visitor: &mut F,
) -> Result<(), BundleArchiveError>
where
    R: Read,
    F: FnMut(&BundleArchiveEntry, &mut dyn Read) -> io::Result<()>,
{
    let mut payload = decoded.take(entry.size);
    visitor(entry, &mut payload).map_err(|source| ArchiveError::Visitor {
        path: entry.path.clone(),
        source,
    })?;
    let remaining = payload.limit();
    if remaining != 0 {
        return Err(ArchiveError::PayloadNotConsumed {
            path: entry.path.clone(),
            remaining,
        }
        .into());
    }
    Ok(())
}

impl ArchiveState {
    fn commit(&mut self, entry: BundleArchiveEntry, entry_number: u32) {
        if entry.kind == BundleArchiveEntryKind::File {
            self.file_count += 1;
            self.total_file_bytes += entry.size;
        }
        insert_path(&mut self.paths, &entry);
        self.previous_path = Some(entry.path);
        self.entry_count = entry_number;
    }
}

fn validate_gzip_header(compressed: &[u8]) -> Result<(), BundleArchiveError> {
    if !compressed.starts_with(&CANONICAL_GZIP_HEADER) {
        return Err(ArchiveError::InvalidGzipHeader.into());
    }
    Ok(())
}

fn read_compressed<R: Read>(mut reader: R, limit: u64) -> Result<Vec<u8>, BundleArchiveError> {
    let mut compressed = Vec::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let remaining = limit.saturating_sub(total);
        let requested = if remaining == 0 {
            1
        } else {
            usize::try_from(remaining.min(buffer.len() as u64))
                .map_err(|_| ArchiveError::CompressedAllocation)?
        };
        let read = reader
            .read(&mut buffer[..requested])
            .map_err(ArchiveError::CompressedRead)?;
        if read > requested {
            return Err(ArchiveError::CompressedRead(io::Error::new(
                io::ErrorKind::InvalidData,
                "compressed archive reader returned more bytes than requested",
            ))
            .into());
        }
        if read == 0 {
            return Ok(compressed);
        }
        if remaining == 0 {
            return Err(ArchiveError::CompressedLimit { limit }.into());
        }
        total = total
            .checked_add(u64::try_from(read).map_err(|_| ArchiveError::CompressedAllocation)?)
            .ok_or(ArchiveError::CompressedLimit { limit })?;
        if total > limit {
            return Err(ArchiveError::CompressedLimit { limit }.into());
        }
        compressed
            .try_reserve_exact(read)
            .map_err(|_| ArchiveError::CompressedAllocation)?;
        compressed.extend_from_slice(&buffer[..read]);
    }
}

struct LimitedDecodedReader<R> {
    inner: R,
    limit: u64,
    bytes_read: u64,
}

impl<R> LimitedDecodedReader<R> {
    fn new(inner: R, limit: u64) -> Self {
        Self {
            inner,
            limit,
            bytes_read: 0,
        }
    }

    fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: Read> Read for LimitedDecodedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.bytes_read == self.limit {
            let mut probe = [0_u8; 1];
            return match self.inner.read(&mut probe)? {
                0 => Ok(0),
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("decoded bundle archive exceeds {} bytes", self.limit),
                )),
            };
        }

        let remaining = self.limit - self.bytes_read;
        let allowed = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("allowed decoded read length fits usize");
        let read = self.inner.read(&mut buffer[..allowed])?;
        self.bytes_read = self
            .bytes_read
            .checked_add(u64::try_from(read).expect("read length fits u64"))
            .expect("decoded byte count is bounded by a u64 limit");
        Ok(read)
    }
}

fn read_exact_decoded<R: Read>(
    reader: &mut R,
    buffer: &mut [u8],
    context: &'static str,
) -> Result<(), BundleArchiveError> {
    let mut offset = 0;
    while offset < buffer.len() {
        match reader.read(&mut buffer[offset..]) {
            Ok(0) => return Err(ArchiveError::Truncated(context).into()),
            Ok(read) => offset += read,
            Err(source) => {
                return Err(ArchiveError::DecodedRead { context, source }.into());
            }
        }
    }
    Ok(())
}

fn is_zero_block(block: &[u8; TAR_BLOCK_BYTES]) -> bool {
    block.iter().all(|byte| *byte == 0)
}

fn require_decoded_eof<R: Read>(reader: &mut R) -> Result<(), BundleArchiveError> {
    let mut byte = [0_u8; 1];
    let read = reader
        .read(&mut byte)
        .map_err(|source| ArchiveError::DecodedRead {
            context: "end of gzip member",
            source,
        })?;
    if read != 0 {
        return Err(ArchiveError::TarTrailer.into());
    }
    Ok(())
}

fn read_payload_padding<R: Read>(
    reader: &mut R,
    entry: &BundleArchiveEntry,
) -> Result<(), BundleArchiveError> {
    let padding = (TAR_BLOCK_BYTES_U64 - (entry.size % TAR_BLOCK_BYTES_U64)) % TAR_BLOCK_BYTES_U64;
    if padding == 0 {
        return Ok(());
    }
    let padding = usize::try_from(padding).expect("USTAR padding fits usize");
    let mut buffer = [0_u8; TAR_BLOCK_BYTES];
    read_exact_decoded(reader, &mut buffer[..padding], "USTAR payload padding")?;
    if buffer[..padding].iter().any(|byte| *byte != 0) {
        return Err(ArchiveError::NonZeroPadding(entry.path.clone()).into());
    }
    Ok(())
}

fn parse_header(
    header: &[u8; TAR_BLOCK_BYTES],
    entry: u32,
) -> Result<BundleArchiveEntry, BundleArchiveError> {
    validate_ustar_marker(header, entry)?;
    validate_header_checksum(header, entry)?;
    let kind = parse_entry_kind(header[156], entry)?;
    let (mode, size) = parse_header_metadata(header, entry, kind)?;
    let (path, path_components) = parse_header_path(header, entry)?;
    Ok(BundleArchiveEntry {
        path,
        path_components,
        kind,
        mode,
        size,
    })
}

fn validate_ustar_marker(
    header: &[u8; TAR_BLOCK_BYTES],
    entry: u32,
) -> Result<(), BundleArchiveError> {
    if &header[257..263] != b"ustar\0" {
        return Err(ArchiveError::InvalidHeader {
            entry,
            reason: "magic must be `ustar\\0`",
        }
        .into());
    }
    if &header[263..265] != b"00" {
        return Err(ArchiveError::InvalidHeader {
            entry,
            reason: "USTAR version must be `00`",
        }
        .into());
    }
    Ok(())
}

fn validate_header_checksum(
    header: &[u8; TAR_BLOCK_BYTES],
    entry: u32,
) -> Result<(), BundleArchiveError> {
    let declared_checksum = parse_checksum(&header[148..156], entry)?;
    let computed_checksum = header
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            if (148..156).contains(&index) {
                u64::from(b' ')
            } else {
                u64::from(*byte)
            }
        })
        .sum::<u64>();
    if declared_checksum != computed_checksum {
        return Err(ArchiveError::HeaderChecksum {
            entry,
            declared: declared_checksum,
            computed: computed_checksum,
        }
        .into());
    }
    Ok(())
}

fn parse_entry_kind(
    type_flag: u8,
    entry: u32,
) -> Result<BundleArchiveEntryKind, BundleArchiveError> {
    match type_flag {
        b'0' => Ok(BundleArchiveEntryKind::File),
        b'5' => Ok(BundleArchiveEntryKind::Directory),
        type_flag => Err(ArchiveError::UnsupportedType { entry, type_flag }.into()),
    }
}

fn parse_header_metadata(
    header: &[u8; TAR_BLOCK_BYTES],
    entry: u32,
    kind: BundleArchiveEntryKind,
) -> Result<(u32, u64), BundleArchiveError> {
    let mode = parse_octal(&header[100..108], 7, entry, "mode")?;
    let canonical_mode = match kind {
        BundleArchiveEntryKind::File => mode == 0o644 || mode == 0o755,
        BundleArchiveEntryKind::Directory => mode == 0o755,
    };
    if !canonical_mode {
        return Err(ArchiveError::InvalidHeader {
            entry,
            reason: "mode is not canonical for the entry type",
        }
        .into());
    }
    let uid = parse_octal(&header[108..116], 7, entry, "uid")?;
    let gid = parse_octal(&header[116..124], 7, entry, "gid")?;
    let size = parse_octal(&header[124..136], 11, entry, "size")?;
    let mtime = parse_octal(&header[136..148], 11, entry, "mtime")?;
    let device_major = parse_octal(&header[329..337], 7, entry, "device major")?;
    let device_minor = parse_octal(&header[337..345], 7, entry, "device minor")?;
    if uid != 0 || gid != 0 || mtime != 0 || device_major != 0 || device_minor != 0 {
        return Err(ArchiveError::InvalidHeader {
            entry,
            reason: "uid, gid, mtime, and device numbers must be zero",
        }
        .into());
    }
    if kind == BundleArchiveEntryKind::Directory && size != 0 {
        return Err(ArchiveError::InvalidHeader {
            entry,
            reason: "directory size must be zero",
        }
        .into());
    }
    if header[157..257].iter().any(|byte| *byte != 0)
        || header[265..329].iter().any(|byte| *byte != 0)
    {
        return Err(ArchiveError::InvalidHeader {
            entry,
            reason: "link, user, and group names must be empty",
        }
        .into());
    }
    if header[500..512].iter().any(|byte| *byte != 0) {
        return Err(ArchiveError::InvalidHeader {
            entry,
            reason: "reserved header bytes must be zero",
        }
        .into());
    }
    Ok((
        u32::try_from(mode).expect("validated USTAR mode fits u32"),
        size,
    ))
}

fn parse_header_path(
    header: &[u8; TAR_BLOCK_BYTES],
    entry: u32,
) -> Result<(String, Vec<String>), BundleArchiveError> {
    let name = parse_string_field(&header[0..100], entry, "name")?;
    let prefix = parse_string_field(&header[345..500], entry, "prefix")?;
    if name.is_empty() {
        return Err(ArchiveError::UnsafePath {
            entry,
            reason: "name is empty",
        }
        .into());
    }
    let mut raw_path =
        Vec::with_capacity(prefix.len() + usize::from(!prefix.is_empty()) + name.len());
    if !prefix.is_empty() {
        raw_path.extend_from_slice(prefix);
        raw_path.push(b'/');
    }
    raw_path.extend_from_slice(name);
    let (path, path_components) = validate_path(&raw_path, entry)?;
    let Some((canonical_prefix, canonical_name)) = canonical_ustar_split(&raw_path) else {
        return Err(ArchiveError::InvalidHeader {
            entry,
            reason: "path cannot be represented by canonical USTAR name and prefix fields",
        }
        .into());
    };
    if prefix != canonical_prefix || name != canonical_name {
        return Err(ArchiveError::InvalidHeader {
            entry,
            reason: "name and prefix do not use the canonical USTAR split",
        }
        .into());
    }
    Ok((path, path_components))
}

fn parse_checksum(field: &[u8], entry: u32) -> Result<u64, BundleArchiveError> {
    if field.len() != 8
        || field[6] != 0
        || field[7] != b' '
        || field[..6].iter().any(|byte| !(b'0'..=b'7').contains(byte))
    {
        return Err(ArchiveError::InvalidHeader {
            entry,
            reason: "checksum must be six octal digits followed by NUL and space",
        }
        .into());
    }
    fold_octal(&field[..6], entry, "checksum")
}

fn parse_octal(
    field: &[u8],
    digits: usize,
    entry: u32,
    field_name: &'static str,
) -> Result<u64, BundleArchiveError> {
    if field.len() != digits + 1
        || field[digits] != 0
        || field[..digits]
            .iter()
            .any(|byte| !(b'0'..=b'7').contains(byte))
    {
        return Err(ArchiveError::InvalidHeader {
            entry,
            reason: match field_name {
                "mode" => "mode must use canonical NUL-terminated octal",
                "uid" => "uid must use canonical NUL-terminated octal",
                "gid" => "gid must use canonical NUL-terminated octal",
                "size" => "size must use canonical NUL-terminated octal",
                "mtime" => "mtime must use canonical NUL-terminated octal",
                "device major" => "device major must use canonical NUL-terminated octal",
                "device minor" => "device minor must use canonical NUL-terminated octal",
                _ => "numeric field must use canonical NUL-terminated octal",
            },
        }
        .into());
    }
    fold_octal(&field[..digits], entry, field_name)
}

fn fold_octal(
    digits: &[u8],
    entry: u32,
    field_name: &'static str,
) -> Result<u64, BundleArchiveError> {
    digits.iter().try_fold(0_u64, |value, byte| {
        value
            .checked_mul(8)
            .and_then(|value| value.checked_add(u64::from(*byte - b'0')))
            .ok_or_else(|| {
                ArchiveError::InvalidHeader {
                    entry,
                    reason: match field_name {
                        "size" => "size does not fit u64",
                        _ => "octal field does not fit u64",
                    },
                }
                .into()
            })
    })
}

fn parse_string_field<'a>(
    field: &'a [u8],
    entry: u32,
    field_name: &'static str,
) -> Result<&'a [u8], BundleArchiveError> {
    let Some(nul) = field.iter().position(|byte| *byte == 0) else {
        return Ok(field);
    };
    if field[nul + 1..].iter().any(|byte| *byte != 0) {
        return Err(ArchiveError::InvalidHeader {
            entry,
            reason: match field_name {
                "name" => "name has non-zero bytes after its NUL terminator",
                "prefix" => "prefix has non-zero bytes after its NUL terminator",
                _ => "text field has non-zero bytes after its NUL terminator",
            },
        }
        .into());
    }
    Ok(&field[..nul])
}

fn canonical_ustar_split(path: &[u8]) -> Option<(&[u8], &[u8])> {
    if path.len() <= 100 {
        return Some((&[], path));
    }
    path.iter()
        .enumerate()
        .rev()
        .find(|(index, byte)| {
            **byte == b'/' && *index <= 155 && path.len().saturating_sub(*index + 1) <= 100
        })
        .map(|(index, _)| (&path[..index], &path[index + 1..]))
}

fn validate_path(raw_path: &[u8], entry: u32) -> Result<(String, Vec<String>), BundleArchiveError> {
    if raw_path.is_empty() {
        return Err(ArchiveError::UnsafePath {
            entry,
            reason: "path is empty",
        }
        .into());
    }
    if raw_path.len() > MAX_PORTABLE_PATH_BYTES {
        return Err(ArchiveError::UnsafePath {
            entry,
            reason: "path exceeds the portable 255-byte limit",
        }
        .into());
    }
    if !raw_path.is_ascii() {
        return Err(ArchiveError::UnsafePath {
            entry,
            reason: "path is not ASCII",
        }
        .into());
    }
    if raw_path.first() == Some(&b'/') {
        return Err(ArchiveError::UnsafePath {
            entry,
            reason: "absolute paths are forbidden",
        }
        .into());
    }

    let mut components = Vec::new();
    for component in raw_path.split(|byte| *byte == b'/') {
        if component.is_empty() {
            return Err(ArchiveError::UnsafePath {
                entry,
                reason: "empty path components are forbidden",
            }
            .into());
        }
        if component == b"." || component == b".." {
            return Err(ArchiveError::UnsafePath {
                entry,
                reason: "dot and parent path components are forbidden",
            }
            .into());
        }
        if component.len() > MAX_PORTABLE_COMPONENT_BYTES {
            return Err(ArchiveError::UnsafePath {
                entry,
                reason: "path component exceeds the portable 255-byte limit",
            }
            .into());
        }
        if component
            .iter()
            .any(|byte| !byte.is_ascii_alphanumeric() && !matches!(*byte, b'.' | b'_' | b'-'))
        {
            return Err(ArchiveError::UnsafePath {
                entry,
                reason: "path contains non-portable characters",
            }
            .into());
        }
        if component.last() == Some(&b'.') || component.last() == Some(&b' ') {
            return Err(ArchiveError::UnsafePath {
                entry,
                reason: "path components may not end in a dot or space",
            }
            .into());
        }
        if is_reserved_component(component) {
            return Err(ArchiveError::UnsafePath {
                entry,
                reason: "path contains a reserved platform name",
            }
            .into());
        }
        components.push(
            String::from_utf8(component.to_vec())
                .expect("portable ASCII path component is valid UTF-8"),
        );
    }

    let path =
        String::from_utf8(raw_path.to_vec()).expect("portable ASCII archive path is valid UTF-8");
    Ok((path, components))
}

fn is_reserved_component(component: &[u8]) -> bool {
    let basename = component
        .split(|byte| *byte == b'.')
        .next()
        .unwrap_or(component);
    let uppercase = basename
        .iter()
        .map(u8::to_ascii_uppercase)
        .collect::<Vec<_>>();
    matches!(
        uppercase.as_slice(),
        b"CON" | b"PRN" | b"AUX" | b"NUL" | b"CONIN$" | b"CONOUT$" | b"CLOCK$"
    ) || (uppercase.len() == 4
        && matches!(&uppercase[..3], b"COM" | b"LPT")
        && matches!(uppercase[3], b'1'..=b'9'))
}

#[derive(Debug)]
struct PathRecord {
    original: Vec<String>,
    kind: BundleArchiveEntryKind,
}

fn folded_components(components: &[String]) -> Vec<String> {
    components
        .iter()
        .map(|component| component.to_ascii_lowercase())
        .collect()
}

fn display_components(components: &[String]) -> String {
    components.join("/")
}

fn check_path_conflicts(
    paths: &BTreeMap<Vec<String>, PathRecord>,
    entry: &BundleArchiveEntry,
) -> Result<(), BundleArchiveError> {
    let folded = folded_components(&entry.path_components);
    if let Some(existing) = paths.get(&folded) {
        if existing.original == entry.path_components {
            return Err(ArchiveError::DuplicatePath(entry.path.clone()).into());
        }
        return Err(ArchiveError::CaseFoldCollision {
            path: entry.path.clone(),
            existing: display_components(&existing.original),
        }
        .into());
    }

    for depth in 1..folded.len() {
        if let Some(parent) = paths.get(&folded[..depth]) {
            if parent.kind == BundleArchiveEntryKind::File {
                return Err(ArchiveError::FileParentConflict {
                    path: entry.path.clone(),
                    file: display_components(&parent.original),
                }
                .into());
            }
        }
    }

    if entry.kind == BundleArchiveEntryKind::File {
        if let Some((_, descendant)) = paths
            .range(folded.clone()..)
            .take_while(|(candidate, _)| candidate.starts_with(&folded))
            .find(|(candidate, _)| candidate.len() > folded.len())
        {
            return Err(ArchiveError::FileParentConflict {
                path: display_components(&descendant.original),
                file: entry.path.clone(),
            }
            .into());
        }
    }
    Ok(())
}

fn insert_path(paths: &mut BTreeMap<Vec<String>, PathRecord>, entry: &BundleArchiveEntry) {
    paths.insert(
        folded_components(&entry.path_components),
        PathRecord {
            original: entry.path_components.clone(),
            kind: entry.kind,
        },
    );
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use flate2::{Compression, GzBuilder};

    use super::*;

    #[derive(Clone, Copy)]
    enum TestKind {
        File,
        Directory,
    }

    struct TestEntry<'a> {
        path: &'a str,
        kind: TestKind,
        data: &'a [u8],
    }

    fn limits() -> BundleArchiveLimits {
        BundleArchiveLimits {
            max_compressed_bytes: 1 << 20,
            max_decoded_bytes: 1 << 20,
            max_entries: 32,
            max_file_bytes: 1 << 16,
            max_total_file_bytes: 1 << 18,
        }
    }

    fn write_octal(field: &mut [u8], value: u64) {
        let digits = field.len() - 1;
        let rendered = format!("{value:0digits$o}");
        assert_eq!(rendered.len(), digits);
        field[..digits].copy_from_slice(rendered.as_bytes());
        field[digits] = 0;
    }

    fn refresh_checksum(header: &mut [u8; TAR_BLOCK_BYTES]) {
        header[148..156].fill(b' ');
        let checksum: u64 = header.iter().map(|byte| u64::from(*byte)).sum();
        let rendered = format!("{checksum:06o}\0 ");
        header[148..156].copy_from_slice(rendered.as_bytes());
    }

    fn header(entry: &TestEntry<'_>) -> [u8; TAR_BLOCK_BYTES] {
        let mut header = [0_u8; TAR_BLOCK_BYTES];
        let (prefix, name) = canonical_ustar_split(entry.path.as_bytes()).unwrap();
        header[..name.len()].copy_from_slice(name);
        header[345..345 + prefix.len()].copy_from_slice(prefix);
        write_octal(
            &mut header[100..108],
            match entry.kind {
                TestKind::File => 0o644,
                TestKind::Directory => 0o755,
            },
        );
        write_octal(&mut header[108..116], 0);
        write_octal(&mut header[116..124], 0);
        write_octal(
            &mut header[124..136],
            match entry.kind {
                TestKind::File => u64::try_from(entry.data.len()).unwrap(),
                TestKind::Directory => 0,
            },
        );
        write_octal(&mut header[136..148], 0);
        header[156] = match entry.kind {
            TestKind::File => b'0',
            TestKind::Directory => b'5',
        };
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");
        write_octal(&mut header[329..337], 0);
        write_octal(&mut header[337..345], 0);
        refresh_checksum(&mut header);
        header
    }

    fn tar(entries: &[TestEntry<'_>]) -> Vec<u8> {
        let mut tar = Vec::new();
        for entry in entries {
            tar.extend_from_slice(&header(entry));
            if matches!(entry.kind, TestKind::File) {
                tar.extend_from_slice(entry.data);
                let padding =
                    (TAR_BLOCK_BYTES - entry.data.len() % TAR_BLOCK_BYTES) % TAR_BLOCK_BYTES;
                tar.resize(tar.len() + padding, 0);
            }
        }
        tar.resize(tar.len() + TAR_BLOCK_BYTES * 2, 0);
        tar
    }

    fn gzip(decoded: &[u8]) -> Vec<u8> {
        let mut encoder = GzBuilder::new()
            .mtime(0)
            .operating_system(255)
            .write(Vec::new(), Compression::new(6));
        encoder.write_all(decoded).unwrap();
        let archive = encoder.finish().unwrap();
        assert_eq!(
            &archive[..CANONICAL_GZIP_HEADER.len()],
            &CANONICAL_GZIP_HEADER
        );
        archive
    }

    fn visit_all(
        archive: &[u8],
        archive_limits: BundleArchiveLimits,
    ) -> Result<(BundleArchiveSummary, Vec<(String, Vec<u8>)>), BundleArchiveError> {
        let mut visited = Vec::new();
        let summary = visit_gzip_ustar(Cursor::new(archive), archive_limits, |entry, payload| {
            let mut bytes = Vec::new();
            payload.read_to_end(&mut bytes)?;
            visited.push((entry.path().to_owned(), bytes));
            Ok(())
        })?;
        Ok((summary, visited))
    }

    fn one_file_tar(path: &str, data: &[u8]) -> Vec<u8> {
        tar(&[TestEntry {
            path,
            kind: TestKind::File,
            data,
        }])
    }

    #[test]
    fn canonical_writer_is_deterministic_and_round_trips() {
        let files = [
            BundleArchiveFile::new("manifest.txt", 0o644, b"signed"),
            BundleArchiveFile::new("app/server.mjs", 0o755, b"#!/usr/bin/env node\n"),
        ];
        let first = write_gzip_ustar(Vec::new(), &files).unwrap();
        let second = write_gzip_ustar(Vec::new(), &files).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            &first[..CANONICAL_GZIP_HEADER.len()],
            &CANONICAL_GZIP_HEADER
        );

        let (summary, visited) = visit_all(&first, limits()).unwrap();
        assert_eq!(
            visited,
            vec![
                (
                    "app/server.mjs".to_owned(),
                    b"#!/usr/bin/env node\n".to_vec()
                ),
                ("manifest.txt".to_owned(), b"signed".to_vec()),
            ]
        );
        assert_eq!(summary.entry_count(), 2);
        assert_eq!(summary.file_count(), 2);
    }

    #[test]
    fn canonical_writer_rejects_invalid_or_colliding_files() {
        for files in [
            vec![],
            vec![BundleArchiveFile::new("../escape", 0o644, b"x")],
            vec![BundleArchiveFile::new("bad mode", 0o600, b"x")],
            vec![
                BundleArchiveFile::new("Name", 0o644, b"x"),
                BundleArchiveFile::new("name", 0o644, b"y"),
            ],
            vec![
                BundleArchiveFile::new("parent", 0o644, b"x"),
                BundleArchiveFile::new("parent/child", 0o644, b"y"),
            ],
        ] {
            let error = write_gzip_ustar(Vec::new(), &files)
                .expect_err("noncanonical writer input must fail");
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        }
    }

    #[test]
    fn accepts_canonical_streamed_files_and_directories() {
        let decoded = tar(&[
            TestEntry {
                path: "lib",
                kind: TestKind::Directory,
                data: &[],
            },
            TestEntry {
                path: "lib/inrou.dylib",
                kind: TestKind::File,
                data: b"native",
            },
            TestEntry {
                path: "manifest.txt",
                kind: TestKind::File,
                data: b"signed",
            },
        ]);
        let archive = gzip(&decoded);
        let (summary, visited) = visit_all(&archive, limits()).unwrap();

        assert_eq!(
            visited,
            vec![
                ("lib".to_owned(), Vec::new()),
                ("lib/inrou.dylib".to_owned(), b"native".to_vec()),
                ("manifest.txt".to_owned(), b"signed".to_vec()),
            ]
        );
        assert_eq!(summary.compressed_bytes(), archive.len() as u64);
        assert_eq!(summary.decoded_bytes(), decoded.len() as u64);
        assert_eq!(summary.entry_count(), 3);
        assert_eq!(summary.file_count(), 2);
        assert_eq!(summary.total_file_bytes(), 12);
    }

    #[test]
    fn accepts_the_canonical_long_path_split() {
        let path = format!("{}/{}", "a".repeat(90), "b".repeat(80));
        let decoded = one_file_tar(&path, b"x");
        let (_, visited) = visit_all(&gzip(&decoded), limits()).unwrap();
        assert_eq!(visited, vec![(path, b"x".to_vec())]);
    }

    #[test]
    fn rejects_noncanonical_gzip_headers_and_empty_archives() {
        let decoded = one_file_tar("file", b"x");
        let canonical = gzip(&decoded);
        for index in 0..CANONICAL_GZIP_HEADER.len() {
            let mut mutated = canonical.clone();
            mutated[index] ^= 1;
            assert!(
                visit_all(&mutated, limits())
                    .unwrap_err()
                    .to_string()
                    .contains("canonical gzip header"),
                "accepted gzip header mutation at byte {index}"
            );
        }
        assert!(
            visit_all(&[], limits())
                .unwrap_err()
                .to_string()
                .contains("canonical gzip header")
        );

        let empty_tar = vec![0_u8; TAR_BLOCK_BYTES * 2];
        assert!(
            visit_all(&gzip(&empty_tar), limits())
                .unwrap_err()
                .to_string()
                .contains("at least one entry")
        );
        let directories_only = tar(&[TestEntry {
            path: "dir",
            kind: TestKind::Directory,
            data: &[],
        }]);
        assert!(
            visit_all(&gzip(&directories_only), limits())
                .unwrap_err()
                .to_string()
                .contains("regular file")
        );
    }

    #[test]
    fn enforces_all_resource_limits() {
        let decoded = tar(&[
            TestEntry {
                path: "a",
                kind: TestKind::File,
                data: b"abc",
            },
            TestEntry {
                path: "b",
                kind: TestKind::File,
                data: b"def",
            },
        ]);
        let archive = gzip(&decoded);

        let mut constrained = limits();
        constrained.max_compressed_bytes = archive.len() as u64 - 1;
        assert!(visit_all(&archive, constrained).is_err());

        constrained = limits();
        constrained.max_decoded_bytes = decoded.len() as u64 - 1;
        constrained.max_file_bytes = 3;
        constrained.max_total_file_bytes = 6;
        assert!(visit_all(&archive, constrained).is_err());

        constrained = limits();
        constrained.max_entries = 1;
        assert!(visit_all(&archive, constrained).is_err());

        constrained = limits();
        constrained.max_file_bytes = 2;
        constrained.max_total_file_bytes = 6;
        assert!(visit_all(&archive, constrained).is_err());

        constrained = limits();
        constrained.max_file_bytes = 3;
        constrained.max_total_file_bytes = 5;
        assert!(visit_all(&archive, constrained).is_err());

        constrained = limits();
        constrained.max_entries = BUNDLE_ARCHIVE_PROTOCOL_MAX_ENTRIES + 1;
        assert!(visit_all(&archive, constrained).is_err());
    }

    #[test]
    fn validates_immutable_protocol_resource_ceilings() {
        let exact = BundleArchiveLimits {
            max_compressed_bytes: BUNDLE_ARCHIVE_PROTOCOL_MAX_COMPRESSED_BYTES,
            max_decoded_bytes: BUNDLE_ARCHIVE_PROTOCOL_MAX_DECODED_BYTES,
            max_entries: BUNDLE_ARCHIVE_PROTOCOL_MAX_ENTRIES,
            max_file_bytes: BUNDLE_ARCHIVE_PROTOCOL_MAX_FILE_BYTES,
            max_total_file_bytes: BUNDLE_ARCHIVE_PROTOCOL_MAX_TOTAL_FILE_BYTES,
        };
        assert_eq!(exact.validate().unwrap(), exact);

        let mut over = exact;
        over.max_compressed_bytes = BUNDLE_ARCHIVE_PROTOCOL_MAX_COMPRESSED_BYTES + 1;
        assert!(
            over.validate()
                .unwrap_err()
                .to_string()
                .contains("max_compressed_bytes exceeds")
        );

        over = exact;
        over.max_decoded_bytes = BUNDLE_ARCHIVE_PROTOCOL_MAX_DECODED_BYTES + 1;
        assert!(
            over.validate()
                .unwrap_err()
                .to_string()
                .contains("max_decoded_bytes exceeds")
        );

        over = exact;
        over.max_entries = BUNDLE_ARCHIVE_PROTOCOL_MAX_ENTRIES + 1;
        assert!(
            over.validate()
                .unwrap_err()
                .to_string()
                .contains("max_entries exceeds")
        );

        over = exact;
        over.max_file_bytes = BUNDLE_ARCHIVE_PROTOCOL_MAX_FILE_BYTES + 1;
        assert!(
            over.validate()
                .unwrap_err()
                .to_string()
                .contains("max_file_bytes exceeds")
        );

        over = exact;
        over.max_total_file_bytes = BUNDLE_ARCHIVE_PROTOCOL_MAX_TOTAL_FILE_BYTES + 1;
        assert!(
            over.validate()
                .unwrap_err()
                .to_string()
                .contains("max_total_file_bytes exceeds")
        );
    }

    #[test]
    fn rejects_unsupported_entry_types() {
        for type_flag in [
            0, b'1', b'2', b'3', b'4', b'6', b'7', b'x', b'g', b'L', b'K', b'S',
        ] {
            let mut decoded = one_file_tar("file", b"x");
            decoded[156] = type_flag;
            let mut first_header =
                <[u8; TAR_BLOCK_BYTES]>::try_from(&decoded[..TAR_BLOCK_BYTES]).unwrap();
            refresh_checksum(&mut first_header);
            decoded[..TAR_BLOCK_BYTES].copy_from_slice(&first_header);
            let error = visit_all(&gzip(&decoded), limits()).unwrap_err();
            assert!(
                error.to_string().contains("unsupported type flag"),
                "unexpected error for {type_flag:#x}: {error}"
            );
        }
    }

    #[test]
    fn rejects_unsafe_and_colliding_paths() {
        for path in [
            "",
            "/absolute",
            "../parent",
            "a/../parent",
            "./dot",
            "a//empty",
            "a\\backslash",
            "C:drive",
            "has space",
            "control\u{1}",
            "question?",
            "trailing.",
            "CON",
            "aux.txt",
            "COM1",
            "lpt9.dll",
        ] {
            let error = visit_all(&gzip(&one_file_tar(path, b"x")), limits()).unwrap_err();
            assert!(
                error.to_string().contains("unsafe path"),
                "unexpected error for {path}: {error}"
            );
        }

        let non_ascii = one_file_tar("é", b"x");
        assert!(
            visit_all(&gzip(&non_ascii), limits())
                .unwrap_err()
                .to_string()
                .contains("unsafe path")
        );

        for entries in [
            vec![("same", TestKind::File), ("same", TestKind::File)],
            vec![("Name", TestKind::File), ("name", TestKind::File)],
            vec![("parent", TestKind::File), ("parent/child", TestKind::File)],
            vec![("parent/child", TestKind::File), ("parent", TestKind::File)],
            vec![("Parent", TestKind::File), ("parent/child", TestKind::File)],
        ] {
            let entries = entries
                .iter()
                .map(|(path, kind)| TestEntry {
                    path,
                    kind: *kind,
                    data: b"x",
                })
                .collect::<Vec<_>>();
            assert!(visit_all(&gzip(&tar(&entries)), limits()).is_err());
        }
    }

    #[test]
    fn rejects_noncanonical_or_corrupt_headers() {
        let cases: &[(usize, u8)] = &[
            (257, b'U'),
            (263, b' '),
            (124, b' '),
            (125, b'8'),
            (124, 0x80),
            (500, 1),
            (157, b'x'),
        ];
        for &(offset, value) in cases {
            let mut decoded = one_file_tar("file", b"x");
            decoded[offset] = value;
            let mut first_header =
                <[u8; TAR_BLOCK_BYTES]>::try_from(&decoded[..TAR_BLOCK_BYTES]).unwrap();
            refresh_checksum(&mut first_header);
            decoded[..TAR_BLOCK_BYTES].copy_from_slice(&first_header);
            assert!(
                visit_all(&gzip(&decoded), limits()).is_err(),
                "accepted mutation at header offset {offset}"
            );
        }

        let mut bad_checksum = one_file_tar("file", b"x");
        bad_checksum[148] = b'7';
        assert!(
            visit_all(&gzip(&bad_checksum), limits())
                .unwrap_err()
                .to_string()
                .contains("checksum")
        );
    }

    #[test]
    fn rejects_noncanonical_metadata_modes_order_and_path_split() {
        for &(offset, value) in &[
            (114, b'1'),
            (122, b'1'),
            (146, b'1'),
            (335, b'1'),
            (343, b'1'),
            (265, b'u'),
            (297, b'g'),
            (157, b'l'),
            (500, b'r'),
        ] {
            let mut decoded = one_file_tar("file", b"x");
            decoded[offset] = value;
            let mut first_header =
                <[u8; TAR_BLOCK_BYTES]>::try_from(&decoded[..TAR_BLOCK_BYTES]).unwrap();
            refresh_checksum(&mut first_header);
            decoded[..TAR_BLOCK_BYTES].copy_from_slice(&first_header);
            assert!(
                visit_all(&gzip(&decoded), limits()).is_err(),
                "accepted metadata mutation at header offset {offset}"
            );
        }

        let mut file_mode = one_file_tar("file", b"x");
        write_octal(&mut file_mode[100..108], 0o600);
        let mut first_header =
            <[u8; TAR_BLOCK_BYTES]>::try_from(&file_mode[..TAR_BLOCK_BYTES]).unwrap();
        refresh_checksum(&mut first_header);
        file_mode[..TAR_BLOCK_BYTES].copy_from_slice(&first_header);
        assert!(visit_all(&gzip(&file_mode), limits()).is_err());

        let mut directory_mode = tar(&[
            TestEntry {
                path: "dir",
                kind: TestKind::Directory,
                data: &[],
            },
            TestEntry {
                path: "file",
                kind: TestKind::File,
                data: b"x",
            },
        ]);
        write_octal(&mut directory_mode[100..108], 0o644);
        let mut first_header =
            <[u8; TAR_BLOCK_BYTES]>::try_from(&directory_mode[..TAR_BLOCK_BYTES]).unwrap();
        refresh_checksum(&mut first_header);
        directory_mode[..TAR_BLOCK_BYTES].copy_from_slice(&first_header);
        assert!(visit_all(&gzip(&directory_mode), limits()).is_err());

        let out_of_order = tar(&[
            TestEntry {
                path: "b",
                kind: TestKind::File,
                data: b"x",
            },
            TestEntry {
                path: "a",
                kind: TestKind::File,
                data: b"x",
            },
        ]);
        assert!(
            visit_all(&gzip(&out_of_order), limits())
                .unwrap_err()
                .to_string()
                .contains("strictly after")
        );

        let mut alternate_split = one_file_tar("dir/file", b"x");
        alternate_split[..100].fill(0);
        alternate_split[..4].copy_from_slice(b"file");
        alternate_split[345..500].fill(0);
        alternate_split[345..348].copy_from_slice(b"dir");
        let mut first_header =
            <[u8; TAR_BLOCK_BYTES]>::try_from(&alternate_split[..TAR_BLOCK_BYTES]).unwrap();
        refresh_checksum(&mut first_header);
        alternate_split[..TAR_BLOCK_BYTES].copy_from_slice(&first_header);
        assert!(
            visit_all(&gzip(&alternate_split), limits())
                .unwrap_err()
                .to_string()
                .contains("canonical USTAR split")
        );
    }

    #[test]
    fn rejects_truncation_padding_and_trailing_data() {
        let canonical = one_file_tar("file", b"x");

        let mut truncated_tar = canonical.clone();
        truncated_tar.truncate(truncated_tar.len() - TAR_BLOCK_BYTES);
        assert!(visit_all(&gzip(&truncated_tar), limits()).is_err());

        let mut nonzero_padding = canonical.clone();
        nonzero_padding[TAR_BLOCK_BYTES + 1] = 1;
        assert!(
            visit_all(&gzip(&nonzero_padding), limits())
                .unwrap_err()
                .to_string()
                .contains("padding")
        );

        let mut nonzero_tar_trailer = canonical.clone();
        nonzero_tar_trailer.extend_from_slice(&[0_u8; TAR_BLOCK_BYTES]);
        nonzero_tar_trailer[TAR_BLOCK_BYTES * 4] = 1;
        assert!(
            visit_all(&gzip(&nonzero_tar_trailer), limits())
                .unwrap_err()
                .to_string()
                .contains("after exactly two USTAR terminator")
        );

        let mut partial_tar_trailer = canonical.clone();
        partial_tar_trailer.push(0);
        assert!(visit_all(&gzip(&partial_tar_trailer), limits()).is_err());

        let mut extra_zero_block = canonical.clone();
        extra_zero_block.extend_from_slice(&[0_u8; TAR_BLOCK_BYTES]);
        assert!(visit_all(&gzip(&extra_zero_block), limits()).is_err());

        let archive = gzip(&canonical);
        let mut compressed_suffix = archive.clone();
        compressed_suffix.push(0);
        assert!(
            visit_all(&compressed_suffix, limits())
                .unwrap_err()
                .to_string()
                .contains("trailing compressed")
        );

        let mut second_member = archive.clone();
        second_member.extend_from_slice(&gzip(&[]));
        assert!(
            visit_all(&second_member, limits())
                .unwrap_err()
                .to_string()
                .contains("trailing compressed")
        );

        let mut corrupt_crc = archive;
        let crc_offset = corrupt_crc.len() - 8;
        corrupt_crc[crc_offset] ^= 1;
        assert!(visit_all(&corrupt_crc, limits()).is_err());

        let archive = gzip(&canonical);
        let mut corrupt_size = archive.clone();
        let size_offset = corrupt_size.len() - 4;
        corrupt_size[size_offset] ^= 1;
        assert!(visit_all(&corrupt_size, limits()).is_err());

        for removed in 1..=7 {
            assert!(visit_all(&archive[..archive.len() - removed], limits()).is_err());
        }
    }

    #[test]
    fn requires_the_visitor_to_consume_each_payload() {
        let archive = gzip(&one_file_tar("file", b"payload"));
        let error = visit_gzip_ustar(Cursor::new(archive), limits(), |_entry, _payload| Ok(()))
            .unwrap_err();
        assert!(error.to_string().contains("unread payload bytes"));
    }
}
