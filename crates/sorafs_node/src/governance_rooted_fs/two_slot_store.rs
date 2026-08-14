// Bounded two-slot retained-state store implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotFileIdentityV1 {
    platform: u8,
    first: u64,
    second: u64,
}
impl From<FileIdentity> for TwoSlotFileIdentityV1 {
    fn from(identity: FileIdentity) -> Self {
        #[cfg(unix)]
        {
            Self {
                platform: 1,
                first: identity.device,
                second: identity.inode,
            }
        }
        #[cfg(windows)]
        {
            Self {
                platform: 2,
                first: u64::from(identity.volume_serial_number),
                second: identity.file_index,
            }
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = identity;
            Self {
                platform: 0,
                first: 0,
                second: 0,
            }
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotBindingMaterialV1 {
    format_version: u8,
    store_name_digest: [u8; 32],
    domain: [u8; 32],
    store_nonce: [u8; 32],
    max_payload_bytes: u64,
    header_region_bytes: u64,
    record_header_region_bytes: u64,
    commit_trailer_region_bytes: u64,
    init_lock_identity: TwoSlotFileIdentityV1,
    slot_identities: [TwoSlotFileIdentityV1; 2],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotHeaderV1 {
    binding: TwoSlotBindingMaterialV1,
    binding_digest: [u8; 32],
    slot_id: u8,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotHeaderRegionV1 {
    header: TwoSlotHeaderV1,
    reserved: [u8; TWO_SLOT_HEADER_RESERVED_BYTES_V1],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotRecordHeaderV1 {
    format_version: u8,
    binding_digest: [u8; 32],
    slot_id: u8,
    generation: u64,
    predecessor_digest: [u8; 32],
    payload_len: u64,
    payload_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotRecordHeaderRegionV1 {
    header: TwoSlotRecordHeaderV1,
    reserved: [u8; TWO_SLOT_RECORD_HEADER_RESERVED_BYTES_V1],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotCommitTrailerV1 {
    format_version: u8,
    binding_digest: [u8; 32],
    slot_id: u8,
    generation: u64,
    record_digest: [u8; 32],
    commit_marker: [u8; 16],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct TwoSlotCommitTrailerRegionV1 {
    trailer: TwoSlotCommitTrailerV1,
    reserved: [u8; TWO_SLOT_COMMIT_TRAILER_RESERVED_BYTES_V1],
}
#[derive(Debug, Clone, Copy)]
struct TwoSlotLayoutV1 {
    header_region_bytes: usize,
    record_header_region_bytes: usize,
    payload_offset: u64,
    trailer_offset: u64,
    commit_trailer_region_bytes: usize,
    slot_file_bytes: u64,
}
/// Immutable identity and byte bounds for one local two-slot V1 store.
///
/// `store_nonce` is a caller-owned stable identifier. Callers must persist or
/// derive the same non-zero value on every restart; this layer never invents a
/// replacement nonce while opening an existing store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TwoSlotStoreConfigV1 {
    store_name: String,
    domain: [u8; 32],
    store_nonce: [u8; 32],
    max_payload_bytes: usize,
}
/// Validated deadline and polling cadence for one two-slot initialization lock.
///
/// This bound applies only while opening or creating the fixed store. Normal
/// loads and compare-and-swap operations retain their separate blocking or
/// typed nonblocking contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TwoSlotInitializationWaitV1 {
    timeout: Duration,
    retry_interval: Duration,
}
impl TwoSlotInitializationWaitV1 {
    /// Construct a non-zero initialization wait bounded by the V1 hard limit.
    pub(super) fn try_new(timeout: Duration, retry_interval: Duration) -> io::Result<Self> {
        if timeout.is_zero()
            || timeout > TWO_SLOT_INITIALIZATION_WAIT_MAX_V1
            || retry_interval.is_zero()
            || retry_interval > timeout
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance two-slot initialization wait is outside the V1 bound",
            ));
        }
        Ok(Self {
            timeout,
            retry_interval,
        })
    }
}
impl TwoSlotStoreConfigV1 {
    /// Validate and construct one stable two-slot store identity.
    pub(super) fn try_new(
        store_name: impl Into<OsString>,
        domain: [u8; 32],
        store_nonce: [u8; 32],
        max_payload_bytes: usize,
    ) -> io::Result<Self> {
        let store_name = store_name.into();
        validate_component(&store_name)?;
        let store_name = store_name.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance two-slot store name must be canonical UTF-8",
            )
        })?;
        let name_bytes = store_name.as_bytes().len();
        if name_bytes == 0
            || name_bytes > TWO_SLOT_STORE_NAME_MAX_BYTES_V1
            || store_name.starts_with('.')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance two-slot store name is hidden or exceeds its V1 byte bound",
            ));
        }
        if domain == TWO_SLOT_ZERO_DIGEST || store_nonce == TWO_SLOT_ZERO_DIGEST {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance two-slot domain and stable store nonce must be non-zero",
            ));
        }
        if max_payload_bytes == 0 || max_payload_bytes > TWO_SLOT_MAX_PAYLOAD_BYTES_V1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance two-slot payload bound is outside the V1 limit",
            ));
        }
        Ok(Self {
            store_name: store_name.to_owned(),
            domain,
            store_nonce,
            max_payload_bytes,
        })
    }
}
#[derive(Debug, Clone)]
struct TwoSlotFileV1 {
    handle: Arc<File>,
    identity: FileIdentity,
    name: OsString,
}
/// One exact selected record returned by a two-slot V1 store.
///
/// The snapshot binds the complete store identity as well as its selected
/// generation and record digest, so it can be used as a compare-and-swap
/// predecessor without accepting a snapshot from another store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TwoSlotSnapshotV1 {
    domain: [u8; 32],
    store_nonce: [u8; 32],
    max_payload_bytes: usize,
    binding_digest: [u8; 32],
    generation: u64,
    record_digest: [u8; 32],
    payload: Vec<u8>,
}
impl TwoSlotSnapshotV1 {
    /// Return the committed monotonic generation.
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }
    /// Borrow the exact committed payload.
    pub(super) fn payload(&self) -> &[u8] {
        &self.payload
    }
    /// Return the domain-separated digest of this complete record.
    pub(super) fn record_digest(&self) -> [u8; 32] {
        self.record_digest
    }
}
/// Typed failure returned while attempting a nonblocking two-slot operation.
#[derive(Debug)]
pub(super) enum TwoSlotTryErrorV1 {
    /// Either the process-local gate or the retained operating-system lock is busy.
    Busy,
    /// The operation reached the store and failed validation or I/O.
    Io(io::Error),
}
impl From<io::Error> for TwoSlotTryErrorV1 {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
/// Typed result of one nonblocking two-slot compare-and-swap attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TwoSlotCasOutcomeV1 {
    /// The exact successor is durably stored, including an exact replay/no-op.
    Stored(TwoSlotSnapshotV1),
    /// The selected record is not the requested predecessor.
    Conflict(TwoSlotSnapshotV1),
}
/// A bounded local store backed by two fixed, retained private files.
#[derive(Debug, Clone)]
pub(super) struct TwoSlotStoreV1 {
    directory: RootedDirectory,
    config: TwoSlotStoreConfigV1,
    layout: TwoSlotLayoutV1,
    init_lock_identity: FileIdentity,
    binding_digest: [u8; 32],
    slots: [TwoSlotFileV1; 2],
    process_lock: Arc<Mutex<()>>,
}
/// Exact init-lock lease retained across a caller-owned composite operation.
///
/// The init-lock identity is part of the immutable two-slot binding material,
/// so unlink/recreate substitution cannot split cooperating writers across two
/// independently lockable filesystem objects.
pub(super) struct TwoSlotBoundOperationLeaseV1 {
    store: TwoSlotStoreV1,
    init_lock: Option<TwoSlotInitFileLockV1>,
}
impl fmt::Debug for TwoSlotBoundOperationLeaseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TwoSlotBoundOperationLeaseV1")
            .field("store", &self.store)
            .field("init_lock_retained", &self.init_lock.is_some())
            .finish()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct TwoSlotCommittedRecordV1 {
    slot_id: usize,
    generation: u64,
    predecessor_digest: [u8; 32],
    record_digest: [u8; 32],
    payload: Vec<u8>,
}
#[derive(Debug)]
struct TwoSlotStageV1 {
    name: OsString,
    directory: RootedDirectory,
    byte_count: u64,
    complete: bool,
}
#[derive(Debug)]
struct TwoSlotStageInventoryV1 {
    byte_count: u64,
    has_full_pair: bool,
    canonical_header_count: usize,
}
fn two_slot_codec_error(label: &str, error: impl fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{label} is not canonical Norito: {error}"),
    )
}
fn encode_two_slot_value<T: norito::NoritoSerialize>(
    value: &T,
    label: &str,
) -> io::Result<Vec<u8>> {
    norito::to_bytes(value).map_err(|error| two_slot_codec_error(label, error))
}
fn decode_two_slot_value<T>(bytes: &[u8], label: &str) -> io::Result<T>
where
    for<'decode> T: norito::NoritoDeserialize<'decode> + norito::NoritoSerialize,
{
    let value =
        norito::decode_from_bytes(bytes).map_err(|error| two_slot_codec_error(label, error))?;
    let canonical = encode_two_slot_value(&value, label)?;
    if canonical != bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} uses a noncanonical Norito encoding"),
        ));
    }
    Ok(value)
}
fn zero_two_slot_binding_material() -> TwoSlotBindingMaterialV1 {
    TwoSlotBindingMaterialV1 {
        format_version: TWO_SLOT_FORMAT_VERSION_V1,
        store_name_digest: TWO_SLOT_ZERO_DIGEST,
        domain: TWO_SLOT_ZERO_DIGEST,
        store_nonce: TWO_SLOT_ZERO_DIGEST,
        max_payload_bytes: 0,
        header_region_bytes: 0,
        record_header_region_bytes: 0,
        commit_trailer_region_bytes: 0,
        init_lock_identity: TwoSlotFileIdentityV1 {
            platform: 0,
            first: 0,
            second: 0,
        },
        slot_identities: [
            TwoSlotFileIdentityV1 {
                platform: 0,
                first: 0,
                second: 0,
            },
            TwoSlotFileIdentityV1 {
                platform: 0,
                first: 0,
                second: 0,
            },
        ],
    }
}
fn two_slot_layout(max_payload_bytes: usize) -> io::Result<TwoSlotLayoutV1> {
    let header_region_bytes = encode_two_slot_value(
        &TwoSlotHeaderRegionV1 {
            header: TwoSlotHeaderV1 {
                binding: zero_two_slot_binding_material(),
                binding_digest: TWO_SLOT_ZERO_DIGEST,
                slot_id: 0,
            },
            reserved: [0; TWO_SLOT_HEADER_RESERVED_BYTES_V1],
        },
        "governance two-slot header region",
    )?
    .len();
    let record_header_region_bytes = encode_two_slot_value(
        &TwoSlotRecordHeaderRegionV1 {
            header: TwoSlotRecordHeaderV1 {
                format_version: TWO_SLOT_FORMAT_VERSION_V1,
                binding_digest: TWO_SLOT_ZERO_DIGEST,
                slot_id: 0,
                generation: 0,
                predecessor_digest: TWO_SLOT_ZERO_DIGEST,
                payload_len: 0,
                payload_digest: TWO_SLOT_ZERO_DIGEST,
            },
            reserved: [0; TWO_SLOT_RECORD_HEADER_RESERVED_BYTES_V1],
        },
        "governance two-slot record-header region",
    )?
    .len();
    let commit_trailer_region_bytes = encode_two_slot_value(
        &TwoSlotCommitTrailerRegionV1 {
            trailer: TwoSlotCommitTrailerV1 {
                format_version: TWO_SLOT_FORMAT_VERSION_V1,
                binding_digest: TWO_SLOT_ZERO_DIGEST,
                slot_id: 0,
                generation: 0,
                record_digest: TWO_SLOT_ZERO_DIGEST,
                commit_marker: TWO_SLOT_COMMIT_MARKER_V1,
            },
            reserved: [0; TWO_SLOT_COMMIT_TRAILER_RESERVED_BYTES_V1],
        },
        "governance two-slot commit-trailer region",
    )?
    .len();
    let payload_offset = header_region_bytes
        .checked_add(record_header_region_bytes)
        .ok_or_else(|| io::Error::other("governance two-slot payload offset overflowed"))?;
    let trailer_offset = payload_offset
        .checked_add(max_payload_bytes)
        .ok_or_else(|| io::Error::other("governance two-slot trailer offset overflowed"))?;
    let slot_file_bytes = trailer_offset
        .checked_add(commit_trailer_region_bytes)
        .ok_or_else(|| io::Error::other("governance two-slot file length overflowed"))?;
    Ok(TwoSlotLayoutV1 {
        header_region_bytes,
        record_header_region_bytes,
        payload_offset: u64::try_from(payload_offset)
            .map_err(|_| io::Error::other("governance two-slot payload offset exceeds u64"))?,
        trailer_offset: u64::try_from(trailer_offset)
            .map_err(|_| io::Error::other("governance two-slot trailer offset exceeds u64"))?,
        commit_trailer_region_bytes,
        slot_file_bytes: u64::try_from(slot_file_bytes)
            .map_err(|_| io::Error::other("governance two-slot file length exceeds u64"))?,
    })
}
fn two_slot_store_name_digest(config: &TwoSlotStoreConfigV1) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.governance.two-slot.store-name.v1\0");
    hasher.update(config.store_name.as_bytes());
    *hasher.finalize().as_bytes()
}
fn two_slot_store_namespace(config: &TwoSlotStoreConfigV1) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = two_slot_store_name_digest(config);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}
fn two_slot_binding_material(
    config: &TwoSlotStoreConfigV1,
    layout: TwoSlotLayoutV1,
    init_lock_identity: FileIdentity,
    identities: [FileIdentity; 2],
) -> io::Result<TwoSlotBindingMaterialV1> {
    Ok(TwoSlotBindingMaterialV1 {
        format_version: TWO_SLOT_FORMAT_VERSION_V1,
        store_name_digest: two_slot_store_name_digest(config),
        domain: config.domain,
        store_nonce: config.store_nonce,
        max_payload_bytes: u64::try_from(config.max_payload_bytes)
            .map_err(|_| io::Error::other("governance two-slot payload bound exceeds u64"))?,
        header_region_bytes: u64::try_from(layout.header_region_bytes)
            .map_err(|_| io::Error::other("governance two-slot header region exceeds u64"))?,
        record_header_region_bytes: u64::try_from(layout.record_header_region_bytes).map_err(
            |_| io::Error::other("governance two-slot record-header region exceeds u64"),
        )?,
        commit_trailer_region_bytes: u64::try_from(layout.commit_trailer_region_bytes).map_err(
            |_| io::Error::other("governance two-slot commit-trailer region exceeds u64"),
        )?,
        init_lock_identity: init_lock_identity.into(),
        slot_identities: identities.map(Into::into),
    })
}
fn two_slot_binding_digest(material: &TwoSlotBindingMaterialV1) -> io::Result<[u8; 32]> {
    let encoded = encode_two_slot_value(material, "governance two-slot binding material")?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.governance.two-slot.binding.v1\0");
    hasher.update(&encoded);
    Ok(*hasher.finalize().as_bytes())
}
fn two_slot_record_digest(header: &TwoSlotRecordHeaderV1, payload: &[u8]) -> io::Result<[u8; 32]> {
    let encoded = encode_two_slot_value(header, "governance two-slot record header")?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.governance.two-slot.record.v1\0");
    hasher.update(&encoded);
    hasher.update(payload);
    Ok(*hasher.finalize().as_bytes())
}
fn read_exact_file_region(file: &File, offset: u64, bytes: usize) -> io::Result<Vec<u8>> {
    let mut region = vec![0; bytes];
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt as _;
        file.read_exact_at(&mut region, offset)?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt as _;
        let mut read = 0_usize;
        while read < region.len() {
            let position = offset
                .checked_add(
                    u64::try_from(read).map_err(|_| {
                        io::Error::other("governance two-slot read offset exceeds u64")
                    })?,
                )
                .ok_or_else(|| io::Error::other("governance two-slot read offset overflowed"))?;
            let count = file.seek_read(&mut region[read..], position)?;
            if count == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "governance two-slot region is truncated",
                ));
            }
            read = read
                .checked_add(count)
                .ok_or_else(|| io::Error::other("governance two-slot read length overflowed"))?;
        }
    }
    #[cfg(any(unix, windows))]
    {
        Ok(region)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (file, offset, region);
        Err(platform::unsupported())
    }
}
fn write_exact_file_region(file: &File, offset: u64, bytes: &[u8]) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt as _;
        return file.write_all_at(bytes, offset);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt as _;
        let mut written = 0_usize;
        while written < bytes.len() {
            let position = offset
                .checked_add(u64::try_from(written).map_err(|_| {
                    io::Error::other("governance two-slot write offset exceeds u64")
                })?)
                .ok_or_else(|| io::Error::other("governance two-slot write offset overflowed"))?;
            let count = file.seek_write(&bytes[written..], position)?;
            if count == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "governance two-slot region write made no progress",
                ));
            }
            written = written
                .checked_add(count)
                .ok_or_else(|| io::Error::other("governance two-slot write length overflowed"))?;
        }
        return Ok(());
    }
    #[cfg(not(any(unix, windows)))]
    Err(platform::unsupported())
}
/// One exact opened regular-file binding retained across later verification.
#[derive(Debug, Clone)]
pub(super) struct FileBinding {
    handle: Arc<File>,
    identity: FileIdentity,
    parent: RootedDirectory,
    name: OsString,
    max_bytes: usize,
    private: bool,
}
impl FileBinding {
    /// Return the stable identity of the retained object.
    pub(super) fn identity(&self) -> FileIdentity {
        self.identity
    }
    /// Revalidate the retained object and its parent-relative name.
    pub(super) fn verify(&self) -> io::Result<()> {
        self.parent.verify_file_binding(
            &self.name,
            &self.handle,
            self.identity,
            self.max_bytes,
            self.private,
        )
    }
}
/// Bytes and an exact retained binding read through one opened file.
#[derive(Debug, Clone)]
pub(super) struct FileSnapshot {
    bytes: Vec<u8>,
    binding: FileBinding,
}
impl FileSnapshot {
    /// Borrow the authenticated bytes.
    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Consume the snapshot and return its bytes.
    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    /// Clone the exact opened binding for later snapshot-wide verification.
    pub(super) fn binding(&self) -> FileBinding {
        self.binding.clone()
    }
}
/// Required destination state for one atomic replacement.
#[derive(Debug, Clone)]
pub(super) enum ExpectedFile {
    /// The destination must not exist at promotion time.
    Missing,
    /// The destination must still be this exact object at promotion time.
    Identity(FileBinding),
}
/// One exact regular file retained below a rooted directory.
#[derive(Debug)]
pub(super) struct RetainedFile {
    binding: FileBinding,
}
impl RetainedFile {
    /// Borrow the exact opened file handle.
    pub(super) fn handle(&self) -> &File {
        &self.binding.handle
    }
    /// Revalidate the handle and its current parent-relative binding.
    pub(super) fn verify(&self) -> io::Result<()> {
        self.binding.verify()
    }
}
#[derive(Debug, Clone)]
struct DirectoryBinding {
    parent: Arc<File>,
    parent_identity: FileIdentity,
    name: OsString,
}
/// One retained, stable directory capability.
#[derive(Debug, Clone)]
pub(super) struct RootedDirectory {
    handle: Arc<File>,
    identity: FileIdentity,
    /// Exact initial Windows owner SID; ownership changes are authority changes
    /// and therefore invalidate this retained capability.
    #[cfg(windows)]
    owner_sid: Vec<u8>,
    display_path: PathBuf,
    binding: Option<DirectoryBinding>,
    writable: bool,
}
impl RootedDirectory {
    /// Wrap a directory handle already retained by the Governance root guard.
    pub(super) fn from_retained(
        display_path: PathBuf,
        handle: Arc<File>,
        writable: bool,
    ) -> io::Result<Self> {
        platform::ensure_supported()?;
        let metadata = handle.metadata()?;
        validate_directory_metadata(&display_path, &metadata)?;
        #[cfg(windows)]
        let owner_sid = platform::directory_owner_sid(&handle, &display_path)?;
        let directory = Self {
            identity: file_identity(&metadata)?,
            handle,
            #[cfg(windows)]
            owner_sid,
            display_path,
            binding: None,
            writable,
        };
        directory.verify_handle()?;
        Ok(directory)
    }
    /// Open and retain a release-qualified root directory.
    #[cfg(windows)]
    pub(super) fn open_root(path: &Path, writable: bool) -> io::Result<Self> {
        platform::ensure_supported()?;
        let handle = Arc::new(platform::open_root(path, writable)?);
        Self::from_retained(path.to_path_buf(), handle, writable)
    }
    /// Revalidate the retained object and its current pathname binding.
    pub(super) fn verify_path_binding(&self, path: &Path) -> io::Result<()> {
        self.verify_handle()?;
        let linked = fs::symlink_metadata(path)?;
        validate_directory_metadata(path, &linked)?;
        if file_identity(&linked)? != self.identity {
            return Err(io::Error::other(format!(
                "governance directory path `{}` no longer names its retained object",
                path.display()
            )));
        }
        Ok(())
    }
    /// Revalidate this directory's retained handle and parent-relative binding.
    pub(super) fn verify(&self) -> io::Result<()> {
        self.verify_handle()?;
        if let Some(binding) = &self.binding {
            let parent_metadata = binding.parent.metadata()?;
            validate_directory_metadata(&self.display_path, &parent_metadata)?;
            if file_identity(&parent_metadata)? != binding.parent_identity {
                return Err(io::Error::other(format!(
                    "retained parent for governance directory `{}` changed identity",
                    self.display_path.display()
                )));
            }
            let linked = platform::open_directory(&binding.parent, &binding.name, self.writable)?;
            let linked_metadata = linked.metadata()?;
            validate_directory_metadata(&self.display_path, &linked_metadata)?;
            if file_identity(&linked_metadata)? != self.identity {
                return Err(io::Error::other(format!(
                    "governance directory binding `{}` was substituted",
                    self.display_path.display()
                )));
            }
        }
        self.verify_handle()
    }
    /// Return a path-free digest of this exact retained directory identity.
    pub(super) fn identity_digest(&self) -> io::Result<[u8; 32]> {
        self.verify()?;
        let identity = TwoSlotFileIdentityV1::from(self.identity);
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"sorafs.governance-rooted-fs.directory-identity.v1\0");
        hasher.update(&[identity.platform]);
        hasher.update(&identity.first.to_le_bytes());
        hasher.update(&identity.second.to_le_bytes());
        let digest = *hasher.finalize().as_bytes();
        self.verify()?;
        Ok(digest)
    }
    fn verify_handle(&self) -> io::Result<()> {
        let before = self.handle.metadata()?;
        validate_directory_metadata(&self.display_path, &before)?;
        if file_identity(&before)? != self.identity {
            return Err(io::Error::other(format!(
                "retained governance directory `{}` changed identity",
                self.display_path.display()
            )));
        }
        #[cfg(windows)]
        if platform::directory_owner_sid(&self.handle, &self.display_path)? != self.owner_sid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "retained governance directory `{}` changed owner SID",
                    self.display_path.display()
                ),
            ));
        }
        #[cfg(not(windows))]
        platform::validate_directory_acl(&self.handle, &self.display_path)?;
        let after = self.handle.metadata()?;
        validate_directory_metadata(&self.display_path, &after)?;
        if file_identity(&after)? != self.identity {
            return Err(io::Error::other(format!(
                "retained governance directory `{}` changed identity during ACL inspection",
                self.display_path.display()
            )));
        }
        Ok(())
    }
    /// Validate descriptor-bound ACL policy for this exact directory.
    #[cfg(windows)]
    pub(super) fn validate_acl(&self) -> io::Result<()> {
        self.verify_handle()?;
        validate_retained_directory_acl(&self.handle, &self.display_path)?;
        self.verify_handle()
    }
    /// Flush this exact directory handle.
    pub(super) fn sync_all(&self) -> io::Result<()> {
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot be flushed as a writer",
            ));
        }
        self.verify()?;
        self.handle.sync_all()?;
        self.verify()
    }
    /// Open one direct child directory without following links/reparse points.
    pub(super) fn open_directory(&self, name: &OsStr) -> io::Result<Self> {
        validate_component(name)?;
        self.verify()?;
        let handle = Arc::new(platform::open_directory(&self.handle, name, self.writable)?);
        let metadata = handle.metadata()?;
        let display_path = self.display_path.join(name);
        validate_directory_metadata(&display_path, &metadata)?;
        #[cfg(windows)]
        let owner_sid = platform::directory_owner_sid(&handle, &display_path)?;
        let child = Self {
            handle,
            identity: file_identity(&metadata)?,
            #[cfg(windows)]
            owner_sid,
            display_path,
            binding: Some(DirectoryBinding {
                parent: Arc::clone(&self.handle),
                parent_identity: self.identity,
                name: name.to_os_string(),
            }),
            writable: self.writable,
        };
        self.verify()?;
        child.verify()?;
        Ok(child)
    }
    /// Open or durably create one direct child directory.
    pub(super) fn open_or_create_directory(&self, name: &OsStr) -> io::Result<Self> {
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot create children",
            ));
        }
        match self.open_directory(name) {
            Ok(directory) => Ok(directory),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.verify()?;
                match platform::create_directory(&self.handle, name) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error),
                }
                let directory = self.open_directory(name)?;
                self.handle.sync_all()?;
                self.verify()?;
                directory.verify()?;
                Ok(directory)
            }
            Err(error) => Err(error),
        }
    }
    /// Create one direct child directory without adopting a pre-existing name.
    fn create_child_directory_exclusive(&self, name: &OsStr) -> io::Result<Self> {
        validate_component(name)?;
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot create a two-slot staging directory",
            ));
        }
        self.verify()?;
        platform::create_directory(&self.handle, name)?;
        let child = self.open_directory(name)?;
        self.verify()?;
        child.verify()?;
        Ok(child)
    }
    /// Move one exact retained child directory to a create-only rooted name.
    ///
    /// This operation never replaces or removes a pathname. The returned
    /// capability is reopened below the destination parent and checked against
    /// the exact source-directory identity retained before the rename.
    fn move_child_directory_exclusive(
        &self,
        child: Self,
        destination_parent: &Self,
        destination_name: &OsStr,
    ) -> io::Result<Self> {
        validate_component(destination_name)?;
        if !self.writable || !destination_parent.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot move two-slot recovery state",
            ));
        }
        let binding = child.binding.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance root cannot be moved as a two-slot child",
            )
        })?;
        if binding.parent_identity != self.identity || !Arc::ptr_eq(&binding.parent, &self.handle) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "two-slot staging directory belongs to another retained parent",
            ));
        }
        self.verify()?;
        destination_parent.verify()?;
        child.verify()?;
        let source_name = binding.name.clone();
        let identity = child.identity;
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            platform::rename_exclusive(
                &self.handle,
                &source_name,
                &destination_parent.handle,
                destination_name,
            )?;
        }
        #[cfg(windows)]
        {
            platform::rename_open_file(
                &destination_parent.handle,
                &child.handle,
                &source_name,
                destination_name,
            )?;
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            return Err(platform::unsupported());
        }
        let installed = destination_parent.open_directory(destination_name)?;
        if installed.identity != identity || file_identity(&child.handle.metadata()?)? != identity {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "two-slot directory rename installed a substituted object",
            ));
        }
        match self.open_directory(&source_name) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(replacement) => {
                drop(replacement);
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "two-slot source name was substituted during directory rename",
                ));
            }
            Err(error) => return Err(error),
        }
        installed.verify()?;
        Ok(installed)
    }
    /// Open or atomically initialize a bounded two-fixed-slot V1 store.
    pub(super) fn open_or_create_two_slot_store_v1(
        &self,
        config: TwoSlotStoreConfigV1,
        initial_payload: &[u8],
    ) -> io::Result<TwoSlotStoreV1> {
        open_or_create_two_slot_store_v1_with(self, config, initial_payload, |_| Ok(()))
    }
    /// Open or initialize a two-slot store under a bounded cross-process wait.
    pub(super) fn open_or_create_two_slot_store_v1_bounded(
        &self,
        config: TwoSlotStoreConfigV1,
        initial_payload: &[u8],
        wait: TwoSlotInitializationWaitV1,
    ) -> io::Result<TwoSlotStoreV1> {
        open_or_create_two_slot_store_v1_with_mode(
            self,
            config,
            initial_payload,
            TwoSlotInitializationLockModeV1::Bounded(wait),
            |_| Ok(()),
        )
    }
    /// Load an already initialized two-slot store without mutation.
    pub(super) fn load_existing_two_slot_store_v1(
        &self,
        config: TwoSlotStoreConfigV1,
    ) -> io::Result<TwoSlotSnapshotV1> {
        load_existing_two_slot_store_v1(self, config)
    }
    #[cfg(test)]
    fn open_or_create_two_slot_store_v1_with_init_hook<Hook>(
        &self,
        config: TwoSlotStoreConfigV1,
        initial_payload: &[u8],
        after_step: Hook,
    ) -> io::Result<TwoSlotStoreV1>
    where
        Hook: FnMut(&'static str) -> io::Result<()>,
    {
        open_or_create_two_slot_store_v1_with(self, config, initial_payload, after_step)
    }
    /// Resolve a relative target below this retained directory.
    pub(super) fn resolve_parent(
        &self,
        relative: &Path,
        create_directories: bool,
    ) -> io::Result<(Self, OsString)> {
        if relative.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "rooted governance path must be relative",
            ));
        }
        let mut components = relative.components().peekable();
        let mut directory = self.clone();
        while let Some(component) = components.next() {
            let Component::Normal(name) = component else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "rooted governance path contains a non-canonical component",
                ));
            };
            validate_component(name)?;
            if components.peek().is_none() {
                return Ok((directory, name.to_os_string()));
            }
            directory = if create_directories {
                directory.open_or_create_directory(name)?
            } else {
                directory.open_directory(name)?
            };
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "rooted governance target is empty",
        ))
    }
    /// Read one direct child through a no-follow handle.
    pub(super) fn read_file(&self, name: &OsStr, max_bytes: usize) -> io::Result<FileSnapshot> {
        self.read_file_with_policy(name, max_bytes, false)
    }
    /// Read one private direct child through a no-follow handle.
    pub(super) fn read_private_file(
        &self,
        name: &OsStr,
        max_bytes: usize,
    ) -> io::Result<FileSnapshot> {
        self.read_file_with_policy(name, max_bytes, true)
    }
    fn read_file_with_policy(
        &self,
        name: &OsStr,
        max_bytes: usize,
        private: bool,
    ) -> io::Result<FileSnapshot> {
        validate_component(name)?;
        self.verify()?;
        let mut file = platform::open_file(&self.handle, name, false)?;
        let before = file.metadata()?;
        let path = self.display_path.join(name);
        validate_file_metadata(&path, &before, max_bytes, private)?;
        let identity = file_identity(&before)?;
        let max_bytes_u64 = u64::try_from(max_bytes)
            .map_err(|_| io::Error::other("governance file byte limit exceeds u64"))?;
        let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(max_bytes));
        (&mut file)
            .take(max_bytes_u64.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() > max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "governance state `{}` exceeds {max_bytes} bytes",
                    path.display()
                ),
            ));
        }
        let after = file.metadata()?;
        validate_file_metadata(&path, &after, max_bytes, private)?;
        if !metadata_stable_during_read(&before, &after) {
            return Err(io::Error::other(format!(
                "governance state `{}` changed while reading",
                path.display()
            )));
        }
        let linked = platform::open_file(&self.handle, name, false)?;
        let linked_metadata = linked.metadata()?;
        validate_file_metadata(&path, &linked_metadata, max_bytes, private)?;
        if file_identity(&linked_metadata)? != identity {
            return Err(io::Error::other(format!(
                "governance state `{}` changed while reading",
                path.display()
            )));
        }
        self.verify()?;
        Ok(FileSnapshot {
            bytes,
            binding: FileBinding {
                handle: Arc::new(file),
                identity,
                parent: self.clone(),
                name: name.to_os_string(),
                max_bytes,
                private,
            },
        })
    }
    /// Open or create one private direct child and retain its exact binding.
    pub(super) fn open_or_create_private_file(
        &self,
        name: &OsStr,
        max_bytes: usize,
    ) -> io::Result<RetainedFile> {
        validate_component(name)?;
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot create private files",
            ));
        }
        self.verify()?;
        let handle = match platform::open_read_write_file(&self.handle, name) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match platform::create_file(&self.handle, name) {
                    Ok(file) => file,
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                        platform::open_read_write_file(&self.handle, name)?
                    }
                    Err(error) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        };
        let metadata = handle.metadata()?;
        let path = self.display_path.join(name);
        validate_file_metadata(&path, &metadata, max_bytes, true)?;
        let identity = file_identity(&metadata)?;
        self.verify_file_binding(name, &handle, identity, max_bytes, true)?;
        Ok(RetainedFile {
            binding: FileBinding {
                handle: Arc::new(handle),
                identity,
                parent: self.clone(),
                name: name.to_os_string(),
                max_bytes,
                private: true,
            },
        })
    }
    fn verify_file_binding(
        &self,
        name: &OsStr,
        handle: &File,
        expected: FileIdentity,
        max_bytes: usize,
        private: bool,
    ) -> io::Result<()> {
        validate_component(name)?;
        self.verify()?;
        let path = self.display_path.join(name);
        let retained_metadata = handle.metadata()?;
        validate_file_metadata(&path, &retained_metadata, max_bytes, private)?;
        if file_identity(&retained_metadata)? != expected {
            return Err(io::Error::other(format!(
                "retained governance file `{}` changed identity",
                path.display()
            )));
        }
        let linked = platform::open_file(&self.handle, name, false)?;
        let linked_metadata = linked.metadata()?;
        validate_file_metadata(&path, &linked_metadata, max_bytes, private)?;
        if file_identity(&linked_metadata)? != expected {
            return Err(io::Error::other(format!(
                "governance file binding `{}` was substituted",
                path.display()
            )));
        }
        self.verify()
    }
    /// Return the stable identity of a direct regular child, if present.
    pub(super) fn file_identity(&self, name: &OsStr) -> io::Result<Option<FileIdentity>> {
        self.file_identity_with_policy(name, false)
    }
    /// Retain one direct regular child and its exact name binding, if present.
    pub(super) fn file_binding(
        &self,
        name: &OsStr,
        max_bytes: usize,
    ) -> io::Result<Option<FileBinding>> {
        self.file_binding_with_policy_and_access(name, max_bytes, false, false)
    }
    /// Retain one direct regular child with deletion access to its exact handle.
    pub(super) fn removal_file_binding(
        &self,
        name: &OsStr,
        max_bytes: usize,
    ) -> io::Result<Option<FileBinding>> {
        self.file_binding_with_policy_and_access(name, max_bytes, false, true)
    }
    /// Retain one private direct regular child with deletion access to its exact handle.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(super) fn private_removal_file_binding(
        &self,
        name: &OsStr,
        max_bytes: usize,
    ) -> io::Result<Option<FileBinding>> {
        self.file_binding_with_policy_and_access(name, max_bytes, true, true)
    }
    fn file_binding_with_policy_and_access(
        &self,
        name: &OsStr,
        max_bytes: usize,
        private: bool,
        delete_access: bool,
    ) -> io::Result<Option<FileBinding>> {
        validate_component(name)?;
        self.verify()?;
        let handle = match platform::open_file(&self.handle, name, delete_access) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.verify()?;
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        let metadata = handle.metadata()?;
        let path = self.display_path.join(name);
        validate_file_metadata(&path, &metadata, max_bytes, private)?;
        let identity = file_identity(&metadata)?;
        self.verify_file_binding(name, &handle, identity, max_bytes, private)?;
        Ok(Some(FileBinding {
            handle: Arc::new(handle),
            identity,
            parent: self.clone(),
            name: name.to_os_string(),
            max_bytes,
            private,
        }))
    }
    fn file_identity_with_policy(
        &self,
        name: &OsStr,
        private: bool,
    ) -> io::Result<Option<FileIdentity>> {
        validate_component(name)?;
        self.verify()?;
        match platform::open_file(&self.handle, name, false) {
            Ok(file) => {
                let metadata = file.metadata()?;
                let path = self.display_path.join(name);
                if private {
                    validate_private_regular_file_metadata(&path, &metadata)?;
                } else {
                    validate_regular_file_metadata(&path, &metadata)?;
                }
                let identity = file_identity(&metadata)?;
                self.verify()?;
                Ok(Some(identity))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.verify()?;
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
    /// Atomically replace the target only if its stable identity is unchanged.
    ///
    /// Linux/macOS exchange both bindings before retaining the predecessor.
    /// Windows supports create-only installation and exact-byte no-ops, but
    /// fails changed existing-target replacement closed because it has no
    /// rooted atomic exchange that preserves every raced object. Retained
    /// generations are immutable online; saturation requires offline archival
    /// or cleanup while the writer is stopped.
    pub(super) fn atomic_write(
        &self,
        name: &OsStr,
        temporary_name: &OsStr,
        data: &[u8],
        expected: ExpectedFile,
    ) -> io::Result<()> {
        self.atomic_write_with_sync(
            name,
            temporary_name,
            data,
            expected,
            || Ok(()),
            |file| file.sync_all(),
            |directory| directory.sync_all(),
        )
    }
    fn atomic_write_with_sync<BeforePromote, FileSync, DirectorySync>(
        &self,
        name: &OsStr,
        temporary_name: &OsStr,
        data: &[u8],
        expected: ExpectedFile,
        before_promote: BeforePromote,
        mut sync_file: FileSync,
        mut sync_directory: DirectorySync,
    ) -> io::Result<()>
    where
        BeforePromote: FnOnce() -> io::Result<()>,
        FileSync: FnMut(&File) -> io::Result<()>,
        DirectorySync: FnMut(&File) -> io::Result<()>,
    {
        validate_component(name)?;
        validate_component(temporary_name)?;
        if name == temporary_name {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance atomic target and temporary name must differ",
            ));
        }
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot write",
            ));
        }
        self.verify().map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "verify governance atomic directory `{}`: {error}",
                    self.display_path.display()
                ),
            )
        })?;
        verify_expected_file(self, name, &expected).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "verify governance atomic predecessor `{}`: {error}",
                    self.display_path.join(name).display()
                ),
            )
        })?;
        if let ExpectedFile::Identity(expected_binding) = &expected {
            let current = if expected_binding.private {
                self.read_private_file(name, expected_binding.max_bytes)?
            } else {
                self.read_file(name, expected_binding.max_bytes)?
            };
            if current.binding.identity != expected_binding.identity {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "governance atomic predecessor `{}` changed before exact-byte comparison",
                        self.display_path.join(name).display()
                    ),
                ));
            }
            if current.bytes == data {
                expected_binding.verify()?;
                current.binding.verify()?;
                self.verify()?;
                return Ok(());
            }
            #[cfg(windows)]
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Windows governance existing-target replacement is disabled because the platform has no rooted atomic exchange that preserves every raced object",
            ));
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let retained_name = match &expected {
            ExpectedFile::Identity(binding) => {
                let metadata = binding.handle.metadata()?;
                validate_regular_file_metadata(&self.display_path.join(name), &metadata)?;
                Some(self.available_atomic_retained_name(name, metadata.len(), binding.private)?)
            }
            ExpectedFile::Missing => None,
        };
        let mut temporary =
            platform::create_file(&self.handle, temporary_name).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "create governance atomic temporary `{}`: {error}",
                        self.display_path.join(temporary_name).display()
                    ),
                )
            })?;
        let temporary_path = self.display_path.join(temporary_name);
        #[cfg(windows)]
        let mut renamed = false;
        let result = (|| {
            temporary.write_all(data).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "write governance atomic temporary `{}`: {error}",
                        temporary_path.display()
                    ),
                )
            })?;
            sync_file(&temporary).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "sync governance atomic temporary `{}`: {error}",
                        temporary_path.display()
                    ),
                )
            })?;
            let temporary_metadata = temporary.metadata()?;
            validate_private_regular_file_metadata(&temporary_path, &temporary_metadata)?;
            let temporary_identity = file_identity(&temporary_metadata)?;
            verify_expected_file(self, name, &expected).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "reverify governance atomic predecessor `{}`: {error}",
                        self.display_path.join(name).display()
                    ),
                )
            })?;
            self.verify().map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "reverify governance atomic directory `{}`: {error}",
                        self.display_path.display()
                    ),
                )
            })?;
            before_promote()?;
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            if retained_name.is_some() {
                platform::exchange_open_file(&self.handle, &temporary, temporary_name, name)
            } else {
                platform::rename_open_file(&self.handle, &temporary, temporary_name, name)
            }
            .map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "promote governance atomic temporary `{}` to `{}`: {error}",
                        temporary_path.display(),
                        self.display_path.join(name).display()
                    ),
                )
            })?;
            #[cfg(windows)]
            {
                platform::rename_open_file(
                    &self.handle,
                    &temporary,
                    temporary_name,
                    name,
                )
                .map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "promote governance atomic temporary `{}` to `{}` without replacement: {error}",
                            temporary_path.display(),
                            self.display_path.join(name).display()
                        ),
                    )
                })?;
                renamed = true;
            }
            #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
            platform::rename_open_file(&self.handle, &temporary, temporary_name, name).map_err(
                |error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "promote governance atomic temporary `{}` to `{}`: {error}",
                            temporary_path.display(),
                            self.display_path.join(name).display()
                        ),
                    )
                },
            )?;
            sync_directory(&self.handle).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "sync governance atomic directory `{}`: {error}",
                        self.display_path.display()
                    ),
                )
            })?;
            let promoted = platform::open_file(&self.handle, name, false).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "open promoted governance atomic target `{}`: {error}",
                        self.display_path.join(name).display()
                    ),
                )
            })?;
            let promoted_metadata = promoted.metadata()?;
            validate_private_regular_file_metadata(
                &self.display_path.join(name),
                &promoted_metadata,
            )?;
            if file_identity(&promoted_metadata)? != temporary_identity {
                return Err(io::Error::other(format!(
                    "governance atomic target `{}` is not the promoted temporary object",
                    self.display_path.join(name).display()
                )));
            }
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            if let Some(retained_name) = retained_name.as_deref() {
                let (expected_identity, expected_max_bytes, expected_private) = match &expected {
                    ExpectedFile::Identity(binding) => {
                        (binding.identity, binding.max_bytes, binding.private)
                    }
                    ExpectedFile::Missing => unreachable!("retained replacement has an identity"),
                };
                let predecessor = platform::open_file(&self.handle, temporary_name, false)
                    .map_err(|error| {
                        io::Error::new(
                            error.kind(),
                            format!(
                                "open exchanged governance atomic predecessor `{}`: {error}",
                                temporary_path.display()
                            ),
                        )
                    })?;
                let predecessor_metadata = predecessor.metadata()?;
                validate_file_metadata(
                    &temporary_path,
                    &predecessor_metadata,
                    expected_max_bytes,
                    expected_private,
                )?;
                if file_identity(&predecessor_metadata)? != expected_identity {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        format!(
                            "governance atomic predecessor `{}` was substituted during exchange; both objects were preserved",
                            self.display_path.join(name).display()
                        ),
                    ));
                }
                platform::rename_exclusive(
                    &self.handle,
                    temporary_name,
                    &self.handle,
                    retained_name,
                )
                .map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "retain governance atomic predecessor `{}` as `{}`: {error}; the predecessor remains preserved for offline recovery",
                            temporary_path.display(),
                            self.display_path.join(retained_name).display()
                        ),
                    )
                })?;
                sync_directory(&self.handle).map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "sync governance atomic predecessor retention directory `{}`: {error}",
                            self.display_path.display()
                        ),
                    )
                })?;
                let retained_path = self.display_path.join(retained_name);
                let retained = platform::open_file(&self.handle, retained_name, false)?;
                let retained_metadata = retained.metadata()?;
                validate_file_metadata(
                    &retained_path,
                    &retained_metadata,
                    expected_max_bytes,
                    expected_private,
                )?;
                if file_identity(&retained_metadata)? != expected_identity
                    || file_identity(&predecessor.metadata()?)? != expected_identity
                    || retained_metadata.len() != predecessor_metadata.len()
                {
                    return Err(io::Error::new(
                        io::ErrorKind::WouldBlock,
                        "retained governance atomic predecessor was substituted; every observed object remains preserved for offline inspection",
                    ));
                }
                self.require_file_name_absent(temporary_name)?;
            }
            self.verify().map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "verify durable governance atomic directory `{}`: {error}",
                        self.display_path.display()
                    ),
                )
            })?;
            let durable = platform::open_file(&self.handle, name, false).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "open durable governance atomic target `{}`: {error}",
                        self.display_path.join(name).display()
                    ),
                )
            })?;
            let durable_metadata = durable.metadata()?;
            validate_private_regular_file_metadata(
                &self.display_path.join(name),
                &durable_metadata,
            )?;
            if file_identity(&durable_metadata)? != temporary_identity {
                return Err(io::Error::other(format!(
                    "governance atomic target `{}` changed before durable readback",
                    self.display_path.join(name).display()
                )));
            }
            Ok(())
        })();
        #[cfg(windows)]
        if result.is_err() && !renamed {
            let _ = platform::remove_open_file(
                &self.handle,
                &temporary,
                temporary_name,
                file_identity(&temporary.metadata()?).ok(),
            );
        }
        // POSIX has no conditional unlink-by-descriptor, so a failed
        // transaction keeps every ambiguous object available for recovery.
        // Successful replacement retains the exact predecessor in a bounded
        // V1 slot. Windows never enters existing-target replacement.
        result
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn available_atomic_retained_name(
        &self,
        target_name: &OsStr,
        predecessor_bytes: u64,
        private: bool,
    ) -> io::Result<OsString> {
        let target_name = target_name.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance atomic retention target is not canonical UTF-8",
            )
        })?;
        let mut occupied = [false; ATOMIC_RETAINED_SLOT_COUNT_V1];
        let mut retained_bytes = 0_u64;
        for name in self.child_names_bounded(DEFAULT_CHILD_ENTRY_LIMIT)? {
            let Some(name_utf8) = name.to_str() else {
                continue;
            };
            let Some((retained_target, slot)) = atomic_retained_target_and_slot(name_utf8) else {
                if is_atomic_retained_candidate_for(name_utf8, target_name) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "governance atomic retention name `{}` is not canonical; offline inspection is required",
                            self.display_path.join(&name).display()
                        ),
                    ));
                }
                continue;
            };
            let retained = platform::open_file(&self.handle, &name, false)?;
            let metadata = retained.metadata()?;
            validate_file_metadata(
                &self.display_path.join(&name),
                &metadata,
                usize::MAX,
                private && retained_target == target_name,
            )?;
            let identity = file_identity(&metadata)?;
            let linked = platform::open_file(&self.handle, &name, false)?;
            let linked_metadata = linked.metadata()?;
            validate_file_metadata(
                &self.display_path.join(&name),
                &linked_metadata,
                usize::MAX,
                private && retained_target == target_name,
            )?;
            if file_identity(&linked_metadata)? != identity {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "governance atomic retained generation changed during bounded inventory",
                ));
            }
            retained_bytes = retained_bytes.checked_add(metadata.len()).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance atomic retained-generation byte total overflowed",
                )
            })?;
            if retained_target == target_name {
                occupied[slot] = true;
            }
        }
        let total = retained_bytes
            .checked_add(predecessor_bytes)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance atomic retained-generation byte total overflowed",
                )
            })?;
        if total > ATOMIC_RETAINED_TOTAL_MAX_BYTES_V1 {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                format!(
                    "governance atomic retained generations would exceed the {}-byte V1 aggregate bound; stop the writer and archive or clear retained generations offline",
                    ATOMIC_RETAINED_TOTAL_MAX_BYTES_V1
                ),
            ));
        }
        let slot = occupied
            .iter()
            .position(|occupied| !occupied)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "all {ATOMIC_RETAINED_SLOT_COUNT_V1} V1 predecessor slots for `{target_name}` are occupied; stop the writer and archive or clear them offline"
                    ),
                )
            })?;
        atomic_retained_name(OsStr::new(target_name), slot)
    }
    /// Atomically write, binding replacement to the currently opened target.
    pub(super) fn atomic_replace_current(
        &self,
        name: &OsStr,
        temporary_name: &OsStr,
        data: &[u8],
    ) -> io::Result<()> {
        let expected = match self.file_binding(name, usize::MAX)? {
            Some(binding) => ExpectedFile::Identity(binding),
            None => ExpectedFile::Missing,
        };
        self.atomic_write(name, temporary_name, data, expected)
    }
    /// Enumerate direct child names while retaining this exact directory.
    #[cfg(test)]
    pub(super) fn child_names(&self) -> io::Result<Vec<OsString>> {
        self.child_names_bounded(DEFAULT_CHILD_ENTRY_LIMIT)
    }
    /// Enumerate at most `max_entries` direct child names.
    pub(super) fn child_names_bounded(&self, max_entries: usize) -> io::Result<Vec<OsString>> {
        if max_entries == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance directory enumeration bound must be positive",
            ));
        }
        self.verify()?;
        let mut names = platform::child_names(self, max_entries)?;
        names.sort();
        self.verify()?;
        Ok(names)
    }
    /// Remove matching atomic crash temporaries below this exact directory.
    #[cfg(any(windows, test))]
    pub(super) fn remove_atomic_temps_for(&self, target_name: &str) -> io::Result<usize> {
        validate_component(OsStr::new(target_name))?;
        self.remove_atomic_temps_matching(DEFAULT_CHILD_ENTRY_LIMIT, |candidate| {
            candidate == target_name
        })
    }
    /// Remove bounded atomic crash temporaries whose decoded target is allowed.
    #[cfg(any(windows, test))]
    pub(super) fn remove_atomic_temps_matching<Allowed>(
        &self,
        max_entries: usize,
        mut allowed: Allowed,
    ) -> io::Result<usize>
    where
        Allowed: FnMut(&str) -> bool,
    {
        let mut removed = 0usize;
        for name in self.child_names_bounded(max_entries)? {
            let Some(name_utf8) = name.to_str() else {
                continue;
            };
            let Some(target_name) = atomic_temp_target_name(name_utf8) else {
                continue;
            };
            if !allowed(target_name) {
                continue;
            }
            self.verify()?;
            let file = platform::open_file(&self.handle, &name, true)?;
            let metadata = file.metadata()?;
            validate_regular_file_metadata(&self.display_path.join(&name), &metadata)?;
            let identity = file_identity(&metadata)?;
            let linked = platform::open_file(&self.handle, &name, false)?;
            let linked_metadata = linked.metadata()?;
            validate_regular_file_metadata(&self.display_path.join(&name), &linked_metadata)?;
            if file_identity(&linked_metadata)? != identity {
                return Err(io::Error::other(format!(
                    "governance atomic temporary `{}` changed before recovery",
                    self.display_path.join(&name).display()
                )));
            }
            platform::remove_open_file(&self.handle, &file, &name, Some(identity))?;
            drop(linked);
            drop(file);
            self.require_file_name_absent(&name)?;
            removed = removed.saturating_add(1);
        }
        if removed != 0 {
            self.handle.sync_all()?;
        }
        self.verify()?;
        Ok(removed)
    }
    /// Atomically isolate one exact regular-file binding without unlinking it.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(super) fn isolate_file_binding(
        &self,
        binding: FileBinding,
        quarantine: &Self,
        quarantine_name: &OsStr,
    ) -> io::Result<FileSnapshot> {
        self.isolate_file_binding_with(binding, quarantine, quarantine_name, || Ok(()))
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn isolate_file_binding_with<BeforeRename>(
        &self,
        binding: FileBinding,
        quarantine: &Self,
        quarantine_name: &OsStr,
        before_rename: BeforeRename,
    ) -> io::Result<FileSnapshot>
    where
        BeforeRename: FnOnce() -> io::Result<()>,
    {
        self.isolate_file_binding_with_sync(
            binding,
            quarantine,
            quarantine_name,
            before_rename,
            |directory| directory.sync_all(),
            |directory| directory.sync_all(),
        )
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn isolate_file_binding_with_sync<BeforeRename, SyncSource, SyncQuarantine>(
        &self,
        binding: FileBinding,
        quarantine: &Self,
        quarantine_name: &OsStr,
        before_rename: BeforeRename,
        sync_source: SyncSource,
        sync_quarantine: SyncQuarantine,
    ) -> io::Result<FileSnapshot>
    where
        BeforeRename: FnOnce() -> io::Result<()>,
        SyncSource: FnOnce(&File) -> io::Result<()>,
        SyncQuarantine: FnOnce(&File) -> io::Result<()>,
    {
        validate_component(quarantine_name)?;
        if !self.writable || !quarantine.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot isolate recovery children",
            ));
        }
        self.verify()?;
        quarantine.verify()?;
        binding.verify()?;
        if binding.parent.identity != self.identity
            || !Arc::ptr_eq(&binding.parent.handle, &self.handle)
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "planned governance file binding belongs to a different parent",
            ));
        }
        if self.identity == quarantine.identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance recovery quarantine must be a distinct directory",
            ));
        }
        let FileBinding {
            handle,
            identity,
            name,
            max_bytes,
            private,
            ..
        } = binding;
        before_rename()?;
        platform::rename_exclusive(&self.handle, &name, &quarantine.handle, quarantine_name)?;
        let source_sync = sync_source(&self.handle);
        let quarantine_sync = sync_quarantine(&quarantine.handle);
        source_sync?;
        quarantine_sync?;
        let snapshot = quarantine.read_file_with_policy(quarantine_name, max_bytes, private)?;
        if snapshot.binding.identity != identity || file_identity(&handle.metadata()?)? != identity
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance recovery quarantine captured a substituted file; the preserved entry requires offline inspection",
            ));
        }
        self.require_file_name_absent(&name)?;
        snapshot.binding.verify()?;
        self.verify()?;
        quarantine.verify()?;
        Ok(snapshot)
    }
    /// Remove one direct regular child by exact opened identity.
    #[cfg(any(windows, test))]
    pub(super) fn remove_file_binding(&self, binding: FileBinding) -> io::Result<()> {
        self.verify()?;
        binding.verify()?;
        if binding.parent.identity != self.identity
            || !Arc::ptr_eq(&binding.parent.handle, &self.handle)
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "planned governance file binding belongs to a different parent",
            ));
        }
        let FileBinding {
            handle,
            identity,
            name,
            ..
        } = binding;
        platform::remove_open_file(&self.handle, &handle, &name, Some(identity))?;
        drop(handle);
        self.require_file_name_absent(&name)?;
        self.handle.sync_all()?;
        self.verify()
    }
    fn require_file_name_absent(&self, name: &OsStr) -> io::Result<()> {
        match platform::open_file(&self.handle, name, false) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Ok(replacement) => {
                drop(replacement);
                Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "governance file `{}` was replaced during removal",
                        self.display_path.join(name).display()
                    ),
                ))
            }
            Err(error) => Err(error),
        }
    }
    /// Atomically isolate one exact empty directory without unlinking it.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(super) fn isolate_empty_directory_binding(
        &self,
        child: Self,
        quarantine: &Self,
        quarantine_name: &OsStr,
    ) -> io::Result<()> {
        self.isolate_empty_directory_binding_with(child, quarantine, quarantine_name, || Ok(()))
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn isolate_empty_directory_binding_with<BeforeRename>(
        &self,
        child: Self,
        quarantine: &Self,
        quarantine_name: &OsStr,
        before_rename: BeforeRename,
    ) -> io::Result<()>
    where
        BeforeRename: FnOnce() -> io::Result<()>,
    {
        self.isolate_empty_directory_binding_with_sync(
            child,
            quarantine,
            quarantine_name,
            before_rename,
            |directory| directory.sync_all(),
            |directory| directory.sync_all(),
        )
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn isolate_empty_directory_binding_with_sync<BeforeRename, SyncSource, SyncQuarantine>(
        &self,
        child: Self,
        quarantine: &Self,
        quarantine_name: &OsStr,
        before_rename: BeforeRename,
        sync_source: SyncSource,
        sync_quarantine: SyncQuarantine,
    ) -> io::Result<()>
    where
        BeforeRename: FnOnce() -> io::Result<()>,
        SyncSource: FnOnce(&File) -> io::Result<()>,
        SyncQuarantine: FnOnce(&File) -> io::Result<()>,
    {
        validate_component(quarantine_name)?;
        let binding = child.binding.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance root cannot be isolated as a retained child",
            )
        })?;
        validate_component(&binding.name)?;
        if !self.writable || !quarantine.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot isolate recovery children",
            ));
        }
        self.verify()?;
        child.verify()?;
        quarantine.verify()?;
        if binding.parent_identity != self.identity || !Arc::ptr_eq(&binding.parent, &self.handle) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "planned governance directory binding belongs to a different parent",
            ));
        }
        if self.identity == quarantine.identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance recovery quarantine must be a distinct directory",
            ));
        }
        if !child.child_names_bounded(1)?.is_empty() {
            return Err(io::Error::other(format!(
                "governance directory `{}` is not empty",
                child.display_path.display()
            )));
        }
        let name = binding.name.clone();
        let identity = child.identity;
        before_rename()?;
        platform::rename_exclusive(&self.handle, &name, &quarantine.handle, quarantine_name)?;
        let source_sync = sync_source(&self.handle);
        let quarantine_sync = sync_quarantine(&quarantine.handle);
        source_sync?;
        quarantine_sync?;
        let isolated = quarantine.open_directory(quarantine_name)?;
        if isolated.identity != identity || file_identity(&child.handle.metadata()?)? != identity {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance recovery quarantine captured a substituted directory; the preserved entry requires offline inspection",
            ));
        }
        if !isolated.child_names_bounded(1)?.is_empty() {
            return Err(io::Error::other(
                "isolated governance recovery directory changed after quarantine",
            ));
        }
        match self.open_directory(&name) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(replacement) => {
                drop(replacement);
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "governance directory was replaced during recovery isolation",
                ));
            }
            Err(error) => return Err(error),
        }
        isolated.verify()?;
        self.verify()?;
        quarantine.verify()
    }
    /// Remove one direct empty child directory by its exact retained identity.
    pub(super) fn remove_empty_directory_binding(&self, child: Self) -> io::Result<()> {
        self.remove_empty_directory_binding_with_hook(child, || Ok(()))
    }
    #[cfg(test)]
    pub(super) fn remove_empty_directory_binding_with<BeforeRemove>(
        &self,
        child: Self,
        before_remove: BeforeRemove,
    ) -> io::Result<()>
    where
        BeforeRemove: FnOnce() -> io::Result<()>,
    {
        self.remove_empty_directory_binding_with_hook(child, before_remove)
    }
    fn remove_empty_directory_binding_with_hook<BeforeRemove>(
        &self,
        child: Self,
        before_remove: BeforeRemove,
    ) -> io::Result<()>
    where
        BeforeRemove: FnOnce() -> io::Result<()>,
    {
        let binding = child.binding.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance root cannot be removed as a retained child",
            )
        })?;
        validate_component(&binding.name)?;
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "read-only governance directory cannot remove children",
            ));
        }
        self.verify()?;
        child.verify()?;
        if binding.parent_identity != self.identity || !Arc::ptr_eq(&binding.parent, &self.handle) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "planned governance directory binding belongs to a different parent",
            ));
        }
        if !child.child_names_bounded(1)?.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::DirectoryNotEmpty,
                format!(
                    "governance directory `{}` is not empty",
                    child.display_path.display()
                ),
            ));
        }
        let name = binding.name.clone();
        let identity = child.identity;
        before_remove()?;
        platform::remove_open_directory(&self.handle, &child.handle, &name, Some(identity))?;
        drop(child);
        match self.open_directory(&name) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(replacement) => {
                drop(replacement);
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "governance directory `{}` was replaced during removal",
                        self.display_path.join(&name).display()
                    ),
                ));
            }
            Err(error) => return Err(error),
        }
        self.handle.sync_all()?;
        self.verify()
    }
    #[cfg(test)]
    fn atomic_write_with_test_sync<FileSync, DirectorySync>(
        &self,
        name: &OsStr,
        temporary_name: &OsStr,
        data: &[u8],
        expected: ExpectedFile,
        sync_file: FileSync,
        sync_directory: DirectorySync,
    ) -> io::Result<()>
    where
        FileSync: FnMut(&File) -> io::Result<()>,
        DirectorySync: FnMut(&File) -> io::Result<()>,
    {
        self.atomic_write_with_test_hooks(
            name,
            temporary_name,
            data,
            expected,
            || Ok(()),
            sync_file,
            sync_directory,
        )
    }
    #[cfg(test)]
    fn atomic_write_with_test_hooks<BeforePromote, FileSync, DirectorySync>(
        &self,
        name: &OsStr,
        temporary_name: &OsStr,
        data: &[u8],
        expected: ExpectedFile,
        before_promote: BeforePromote,
        sync_file: FileSync,
        sync_directory: DirectorySync,
    ) -> io::Result<()>
    where
        BeforePromote: FnOnce() -> io::Result<()>,
        FileSync: FnMut(&File) -> io::Result<()>,
        DirectorySync: FnMut(&File) -> io::Result<()>,
    {
        self.atomic_write_with_sync(
            name,
            temporary_name,
            data,
            expected,
            before_promote,
            sync_file,
            sync_directory,
        )
    }
}
fn two_slot_file_byte_limit(layout: TwoSlotLayoutV1) -> io::Result<usize> {
    usize::try_from(layout.slot_file_bytes)
        .map_err(|_| io::Error::other("governance two-slot file length exceeds host limits"))
}
fn expected_two_slot_header_region(
    material: &TwoSlotBindingMaterialV1,
    binding_digest: [u8; 32],
    slot_id: usize,
) -> io::Result<Vec<u8>> {
    let slot_id = u8::try_from(slot_id)
        .map_err(|_| io::Error::other("governance two-slot slot id exceeds u8"))?;
    encode_two_slot_value(
        &TwoSlotHeaderRegionV1 {
            header: TwoSlotHeaderV1 {
                binding: material.clone(),
                binding_digest,
                slot_id,
            },
            reserved: [0; TWO_SLOT_HEADER_RESERVED_BYTES_V1],
        },
        "governance two-slot header region",
    )
}
fn open_existing_two_slot_file(
    directory: &RootedDirectory,
    name: &OsStr,
    layout: TwoSlotLayoutV1,
) -> io::Result<TwoSlotFileV1> {
    validate_component(name)?;
    directory.verify()?;
    let handle = if directory.writable {
        platform::open_read_write_file(&directory.handle, name)?
    } else {
        platform::open_file(&directory.handle, name, false)?
    };
    let metadata = handle.metadata()?;
    let path = directory.display_path.join(name);
    let max_bytes = two_slot_file_byte_limit(layout)?;
    validate_file_metadata(&path, &metadata, max_bytes, true)?;
    if metadata.len() != layout.slot_file_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance two-slot file `{}` has length {}, expected {}",
                path.display(),
                metadata.len(),
                layout.slot_file_bytes
            ),
        ));
    }
    let identity = file_identity(&metadata)?;
    directory.verify_file_binding(name, &handle, identity, max_bytes, true)?;
    Ok(TwoSlotFileV1 {
        handle: Arc::new(handle),
        identity,
        name: name.to_os_string(),
    })
}
fn verify_two_slot_file(store: &TwoSlotStoreV1, slot: &TwoSlotFileV1) -> io::Result<()> {
    let max_bytes = two_slot_file_byte_limit(store.layout)?;
    let path = store.directory.display_path.join(&slot.name);
    let metadata = slot.handle.metadata()?;
    validate_file_metadata(&path, &metadata, max_bytes, true)?;
    if metadata.len() != store.layout.slot_file_bytes || file_identity(&metadata)? != slot.identity
    {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "governance two-slot file `{}` changed identity or length",
                path.display()
            ),
        ));
    }
    store
        .directory
        .verify_file_binding(&slot.name, &slot.handle, slot.identity, max_bytes, true)
}
fn verify_two_slot_headers(store: &TwoSlotStoreV1) -> io::Result<()> {
    store.directory.verify()?;
    let mut children = store
        .directory
        .child_names_bounded(TWO_SLOT_NAMES_V1.len())?;
    children.sort();
    let mut expected_children = TWO_SLOT_NAMES_V1.map(OsString::from).to_vec();
    expected_children.sort();
    if children != expected_children {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot store inventory diverged from its exact V1 pair",
        ));
    }
    if store.slots[0].identity == store.slots[1].identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot files alias the same filesystem object",
        ));
    }
    let material = two_slot_binding_material(
        &store.config,
        store.layout,
        store.init_lock_identity,
        [store.slots[0].identity, store.slots[1].identity],
    )?;
    if two_slot_binding_digest(&material)? != store.binding_digest {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot in-memory binding digest diverged",
        ));
    }
    for (slot_id, slot) in store.slots.iter().enumerate() {
        verify_two_slot_file(store, slot)?;
        let actual = read_exact_file_region(&slot.handle, 0, store.layout.header_region_bytes)?;
        let decoded: TwoSlotHeaderRegionV1 =
            decode_two_slot_value(&actual, "governance two-slot header region")?;
        if decoded.reserved != [0; TWO_SLOT_HEADER_RESERVED_BYTES_V1]
            || actual != expected_two_slot_header_region(&material, store.binding_digest, slot_id)?
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "governance two-slot immutable header `{}` diverged",
                    store.directory.display_path.join(&slot.name).display()
                ),
            ));
        }
    }
    store.directory.verify()
}
fn open_existing_two_slot_store(
    directory: RootedDirectory,
    config: TwoSlotStoreConfigV1,
    init_lock_identity: FileIdentity,
) -> io::Result<TwoSlotStoreV1> {
    let mut children = directory.child_names_bounded(TWO_SLOT_NAMES_V1.len())?;
    children.sort();
    let mut expected = TWO_SLOT_NAMES_V1.map(OsString::from).to_vec();
    expected.sort();
    if children != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot store inventory is not the exact V1 pair",
        ));
    }
    let layout = two_slot_layout(config.max_payload_bytes)?;
    let slots = [
        open_existing_two_slot_file(&directory, OsStr::new(TWO_SLOT_NAMES_V1[0]), layout)?,
        open_existing_two_slot_file(&directory, OsStr::new(TWO_SLOT_NAMES_V1[1]), layout)?,
    ];
    if slots[0].identity == slots[1].identity {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot files alias the same identity",
        ));
    }
    let material = two_slot_binding_material(
        &config,
        layout,
        init_lock_identity,
        [slots[0].identity, slots[1].identity],
    )?;
    let binding_digest = two_slot_binding_digest(&material)?;
    let store = TwoSlotStoreV1 {
        directory,
        config,
        layout,
        init_lock_identity,
        binding_digest,
        slots,
        process_lock: Arc::new(Mutex::new(())),
    };
    verify_two_slot_headers(&store)?;
    Ok(store)
}
fn read_two_slot_record_once(
    store: &TwoSlotStoreV1,
    slot_id: usize,
) -> io::Result<Option<TwoSlotCommittedRecordV1>> {
    let slot = store.slots.get(slot_id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance two-slot id is invalid",
        )
    })?;
    let trailer_before = read_exact_file_region(
        &slot.handle,
        store.layout.trailer_offset,
        store.layout.commit_trailer_region_bytes,
    )?;
    let record_region = read_exact_file_region(
        &slot.handle,
        u64::try_from(store.layout.header_region_bytes).map_err(|_| {
            io::Error::other("governance two-slot record-header offset exceeds u64")
        })?,
        store.layout.record_header_region_bytes,
    )?;
    let absent_if_zero_trailer_stable = || {
        let trailer_after = read_exact_file_region(
            &slot.handle,
            store.layout.trailer_offset,
            store.layout.commit_trailer_region_bytes,
        )?;
        if trailer_before == trailer_after {
            Ok(None)
        } else {
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance two-slot trailer changed during invalid-record read",
            ))
        }
    };
    let corrupt_if_trailer_stable = || {
        let trailer_after = read_exact_file_region(
            &slot.handle,
            store.layout.trailer_offset,
            store.layout.commit_trailer_region_bytes,
        )?;
        if trailer_before == trailer_after {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot record has a stable nonzero malformed commit trailer or committed body",
            ))
        } else {
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance two-slot trailer changed during malformed-record read",
            ))
        }
    };
    if trailer_before.iter().all(|byte| *byte == 0) {
        return absent_if_zero_trailer_stable();
    }
    let trailer_region: TwoSlotCommitTrailerRegionV1 =
        match decode_two_slot_value(&trailer_before, "governance two-slot commit trailer") {
            Ok(trailer) => trailer,
            Err(_) => return corrupt_if_trailer_stable(),
        };
    let record_region: TwoSlotRecordHeaderRegionV1 =
        match decode_two_slot_value(&record_region, "governance two-slot record-header region") {
            Ok(record) => record,
            Err(_) => return corrupt_if_trailer_stable(),
        };
    let trailer = trailer_region.trailer;
    let header = record_region.header;
    let expected_slot =
        u8::try_from(slot_id).map_err(|_| io::Error::other("governance two-slot id exceeds u8"))?;
    if trailer_region.reserved != [0; TWO_SLOT_COMMIT_TRAILER_RESERVED_BYTES_V1]
        || record_region.reserved != [0; TWO_SLOT_RECORD_HEADER_RESERVED_BYTES_V1]
        || trailer.format_version != TWO_SLOT_FORMAT_VERSION_V1
        || header.format_version != TWO_SLOT_FORMAT_VERSION_V1
        || trailer.binding_digest != store.binding_digest
        || header.binding_digest != store.binding_digest
        || trailer.slot_id != expected_slot
        || header.slot_id != expected_slot
        || trailer.generation == 0
        || trailer.generation != header.generation
        || (header.generation == 1 && header.predecessor_digest != TWO_SLOT_ZERO_DIGEST)
        || (header.generation > 1 && header.predecessor_digest == TWO_SLOT_ZERO_DIGEST)
        || trailer.commit_marker != TWO_SLOT_COMMIT_MARKER_V1
    {
        return corrupt_if_trailer_stable();
    }
    let payload_len = match usize::try_from(header.payload_len) {
        Ok(payload_len) if payload_len <= store.config.max_payload_bytes => payload_len,
        _ => return corrupt_if_trailer_stable(),
    };
    let payload = read_exact_file_region(&slot.handle, store.layout.payload_offset, payload_len)?;
    let trailer_after = read_exact_file_region(
        &slot.handle,
        store.layout.trailer_offset,
        store.layout.commit_trailer_region_bytes,
    )?;
    if trailer_before != trailer_after {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "governance two-slot trailer changed while reading its record",
        ));
    }
    let record_digest = two_slot_record_digest(&header, &payload)?;
    if trailer.record_digest != record_digest
        || header.payload_digest != *blake3::hash(&payload).as_bytes()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot committed record or payload digest is invalid",
        ));
    }
    Ok(Some(TwoSlotCommittedRecordV1 {
        slot_id,
        generation: header.generation,
        predecessor_digest: header.predecessor_digest,
        record_digest,
        payload,
    }))
}
fn read_two_slot_record_stable(
    store: &TwoSlotStoreV1,
    slot_id: usize,
) -> io::Result<Option<TwoSlotCommittedRecordV1>> {
    const MAX_RETRIES: usize = 3;
    for _ in 0..MAX_RETRIES {
        match read_two_slot_record_once(store, slot_id) {
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
            result => return result,
        }
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        "governance two-slot record did not stabilize during bounded read",
    ))
}
fn select_two_slot_record_unlocked(store: &TwoSlotStoreV1) -> io::Result<TwoSlotCommittedRecordV1> {
    verify_two_slot_headers(store)?;
    let left = read_two_slot_record_stable(store, 0)?;
    let right = read_two_slot_record_stable(store, 1)?;
    verify_two_slot_headers(store)?;
    match (left, right) {
        (None, None) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot store has no committed record",
        )),
        (Some(record), None) | (None, Some(record)) => Ok(record),
        (Some(left), Some(right)) => {
            if left.generation == right.generation {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance two-slot records have an ambiguous equal generation",
                ));
            }
            let (older, newer) = if left.generation < right.generation {
                (left, right)
            } else {
                (right, left)
            };
            if older.generation.checked_add(1) != Some(newer.generation)
                || newer.predecessor_digest != older.record_digest
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance two-slot records are nonconsecutive or have divergent lineage",
                ));
            }
            Ok(newer)
        }
    }
}
fn two_slot_snapshot(
    store: &TwoSlotStoreV1,
    record: TwoSlotCommittedRecordV1,
) -> TwoSlotSnapshotV1 {
    TwoSlotSnapshotV1 {
        domain: store.config.domain,
        store_nonce: store.config.store_nonce,
        max_payload_bytes: store.config.max_payload_bytes,
        binding_digest: store.binding_digest,
        generation: record.generation,
        record_digest: record.record_digest,
        payload: record.payload,
    }
}
struct TwoSlotOsLock<'file> {
    file: &'file File,
    locked: bool,
}
impl<'file> TwoSlotOsLock<'file> {
    fn acquire(file: &'file File) -> io::Result<Self> {
        File::lock(file)?;
        Ok(Self { file, locked: true })
    }
    fn try_acquire(file: &'file File) -> Result<Self, TwoSlotTryErrorV1> {
        match File::try_lock(file) {
            Ok(()) => Ok(Self { file, locked: true }),
            Err(fs::TryLockError::WouldBlock) => Err(TwoSlotTryErrorV1::Busy),
            Err(fs::TryLockError::Error(error)) => Err(TwoSlotTryErrorV1::Io(error)),
        }
    }
    fn release(mut self) -> io::Result<()> {
        let result = File::unlock(self.file);
        if result.is_ok() {
            self.locked = false;
        }
        result
    }
}
impl Drop for TwoSlotOsLock<'_> {
    fn drop(&mut self) {
        if self.locked {
            let _ = File::unlock(self.file);
        }
    }
}
struct TwoSlotInitFileLockV1 {
    root: RootedDirectory,
    name: OsString,
    handle: File,
    identity: FileIdentity,
    locked: bool,
}
impl TwoSlotInitFileLockV1 {
    fn open(root: &RootedDirectory, config: &TwoSlotStoreConfigV1) -> io::Result<Self> {
        let name = two_slot_init_lock_name(config);
        root.verify()?;
        let handle = match platform::open_read_write_file(&root.handle, &name) {
            Ok(handle) => handle,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match platform::create_file(&root.handle, &name) {
                    Ok(handle) => handle,
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                        platform::open_read_write_file(&root.handle, &name)?
                    }
                    Err(error) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        };
        Self::from_opened(root, name, handle, true)
    }
    fn open_existing(root: &RootedDirectory, config: &TwoSlotStoreConfigV1) -> io::Result<Self> {
        let name = two_slot_init_lock_name(config);
        root.verify()?;
        let handle = platform::open_read_write_file(&root.handle, &name)?;
        Self::from_opened(root, name, handle, false)
    }
    fn from_opened(
        root: &RootedDirectory,
        name: OsString,
        handle: File,
        durably_establish: bool,
    ) -> io::Result<Self> {
        let metadata = handle.metadata()?;
        let path = root.display_path.join(&name);
        validate_file_metadata(&path, &metadata, 0, true)?;
        if metadata.len() != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot init lock must remain an empty fixed file",
            ));
        }
        let identity = file_identity(&metadata)?;
        root.verify_file_binding(&name, &handle, identity, 0, true)?;
        if durably_establish {
            // Every initializer establishes the empty lock file and its parent
            // binding durably before it can serialize initialization. This
            // covers the race where a non-creator opens the name before the
            // creator has reached its parent-directory fsync.
            handle.sync_all()?;
            root.sync_all()?;
        }
        Ok(Self {
            root: root.clone(),
            name,
            handle,
            identity,
            locked: false,
        })
    }
    fn acquire(root: &RootedDirectory, config: &TwoSlotStoreConfigV1) -> io::Result<Self> {
        let mut lock = Self::open(root, config)?;
        File::lock(&lock.handle)?;
        lock.locked = true;
        lock.verify()?;
        Ok(lock)
    }
    fn acquire_bounded(
        root: &RootedDirectory,
        config: &TwoSlotStoreConfigV1,
        wait: TwoSlotInitializationWaitV1,
    ) -> io::Result<Self> {
        let mut lock = Self::open(root, config)?;
        let deadline = Instant::now().checked_add(wait.timeout).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance two-slot initialization deadline overflowed",
            )
        })?;
        loop {
            lock.verify()?;
            match File::try_lock(&lock.handle) {
                Ok(()) => {
                    lock.locked = true;
                    lock.verify()?;
                    return Ok(lock);
                }
                Err(fs::TryLockError::WouldBlock) => {
                    let now = Instant::now();
                    if now >= deadline {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "governance two-slot initialization lock wait expired",
                        ));
                    }
                    std::thread::sleep(wait.retry_interval.min(deadline.duration_since(now)));
                }
                Err(fs::TryLockError::Error(error)) => return Err(error),
            }
        }
    }
    fn try_acquire_bound(
        root: &RootedDirectory,
        config: &TwoSlotStoreConfigV1,
        expected_identity: FileIdentity,
    ) -> Result<Self, TwoSlotTryErrorV1> {
        let mut lock = Self::open_existing(root, config)?;
        if lock.identity != expected_identity {
            return Err(TwoSlotTryErrorV1::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot init lock identity was substituted",
            )));
        }
        match File::try_lock(&lock.handle) {
            Ok(()) => {
                lock.locked = true;
                lock.verify()?;
                if lock.identity != expected_identity {
                    return Err(TwoSlotTryErrorV1::Io(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "governance two-slot init lock identity changed during acquisition",
                    )));
                }
                Ok(lock)
            }
            Err(fs::TryLockError::WouldBlock) => Err(TwoSlotTryErrorV1::Busy),
            Err(fs::TryLockError::Error(error)) => Err(TwoSlotTryErrorV1::Io(error)),
        }
    }
    fn verify(&self) -> io::Result<()> {
        let metadata = self.handle.metadata()?;
        let path = self.root.display_path.join(&self.name);
        validate_file_metadata(&path, &metadata, 0, true)?;
        if metadata.len() != 0 || file_identity(&metadata)? != self.identity {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance two-slot init lock changed identity or length",
            ));
        }
        self.root
            .verify_file_binding(&self.name, &self.handle, self.identity, 0, true)
    }
    fn release(mut self) -> io::Result<()> {
        let verification = self.verify();
        let unlock = File::unlock(&self.handle);
        if unlock.is_ok() {
            self.locked = false;
        }
        match (verification, unlock) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), _) | (Ok(()), Err(error)) => Err(error),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TwoSlotInitializationLockModeV1 {
    Blocking,
    Bounded(TwoSlotInitializationWaitV1),
}
impl Drop for TwoSlotInitFileLockV1 {
    fn drop(&mut self) {
        if self.locked {
            let _ = File::unlock(&self.handle);
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TwoSlotCasModeV1 {
    LegacyBlocking,
    NonblockingTyped,
}
impl TwoSlotBoundOperationLeaseV1 {
    /// Revalidate the exact bound init lock and immutable two-slot headers.
    pub(super) fn verify(&self) -> io::Result<()> {
        let init_lock = self.init_lock.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot bound operation lease was already released",
            )
        })?;
        if init_lock.identity != self.store.init_lock_identity {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance two-slot bound operation lease identity diverged",
            ));
        }
        init_lock.verify()?;
        self.store.verify_exact_parent(&init_lock.root)?;
        verify_two_slot_headers(&self.store)?;
        init_lock.verify()
    }
    /// Revalidate and release this exact bound init-lock lease.
    pub(super) fn release(mut self) -> io::Result<()> {
        let verification = self.verify();
        let release = self
            .init_lock
            .take()
            .expect("validated bound operation lease retains its init lock")
            .release();
        match (verification, release) {
            (Ok(()), Ok(())) => Ok(()),
            (_, Err(error)) | (Err(error), Ok(())) => Err(error),
        }
    }
}
impl TwoSlotStoreV1 {
    /// Try to retain the exact init lock committed into this store's headers.
    ///
    /// This nonblocking lease is intended for higher-level composite
    /// operations that span external effects and a later two-slot CAS.
    pub(super) fn try_acquire_bound_operation_lease(
        &self,
        parent: &RootedDirectory,
    ) -> Result<TwoSlotBoundOperationLeaseV1, TwoSlotTryErrorV1> {
        self.verify_exact_parent(parent)?;
        verify_two_slot_headers(self)?;
        let init_lock = TwoSlotInitFileLockV1::try_acquire_bound(
            parent,
            &self.config,
            self.init_lock_identity,
        )?;
        let lease = TwoSlotBoundOperationLeaseV1 {
            store: self.clone(),
            init_lock: Some(init_lock),
        };
        match lease.verify() {
            Ok(()) => Ok(lease),
            Err(error) => {
                let release = lease.release();
                Err(TwoSlotTryErrorV1::Io(release.err().unwrap_or(error)))
            }
        }
    }
    fn verify_exact_parent(&self, parent: &RootedDirectory) -> io::Result<()> {
        parent.verify()?;
        self.directory.verify()?;
        let binding = self.directory.binding.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot directory has no retained parent binding",
            )
        })?;
        if binding.parent_identity != parent.identity
            || !Arc::ptr_eq(&binding.parent, &parent.handle)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot operation lease parent diverged",
            ));
        }
        parent.verify()
    }
    #[cfg(test)]
    pub(super) fn init_lock_name_for_test(&self) -> OsString {
        two_slot_init_lock_name(&self.config)
    }
    fn with_exclusive_lock<ResultValue>(
        &self,
        operation: impl FnOnce(&Self) -> io::Result<ResultValue>,
    ) -> io::Result<ResultValue> {
        let process_guard = self
            .process_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let os_lock = TwoSlotOsLock::acquire(&self.slots[0].handle)?;
        verify_two_slot_headers(self)?;
        let result = operation(self);
        let unlock = os_lock.release();
        drop(process_guard);
        match (result, unlock) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }
    fn with_try_exclusive_lock<ResultValue>(
        &self,
        operation: impl FnOnce(&Self) -> io::Result<ResultValue>,
    ) -> Result<ResultValue, TwoSlotTryErrorV1> {
        let process_guard = match self.process_lock.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err(TwoSlotTryErrorV1::Busy);
            }
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err(TwoSlotTryErrorV1::Io(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance two-slot process lock is poisoned",
                )));
            }
        };
        let os_lock = TwoSlotOsLock::try_acquire(&self.slots[0].handle)?;
        verify_two_slot_headers(self)?;
        let result = operation(self);
        let unlock = os_lock.release();
        drop(process_guard);
        match (result, unlock) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), _) => Err(TwoSlotTryErrorV1::Io(error)),
            (Ok(_), Err(error)) => Err(TwoSlotTryErrorV1::Io(error)),
        }
    }
    /// Load the highest complete record after strict pair and lineage checks.
    pub(super) fn load(&self) -> io::Result<TwoSlotSnapshotV1> {
        self.with_exclusive_lock(|store| {
            select_two_slot_record_unlocked(store).map(|record| two_slot_snapshot(store, record))
        })
    }
    /// Attempt to load the highest complete record without waiting for either lock.
    pub(super) fn try_load(&self) -> Result<TwoSlotSnapshotV1, TwoSlotTryErrorV1> {
        self.with_try_exclusive_lock(|store| {
            select_two_slot_record_unlocked(store).map(|record| two_slot_snapshot(store, record))
        })
    }
    /// Commit one direct successor of `expected`, or return an exact-byte no-op.
    pub(super) fn compare_and_swap(
        &self,
        expected: &TwoSlotSnapshotV1,
        payload: &[u8],
    ) -> io::Result<TwoSlotSnapshotV1> {
        self.compare_and_swap_with(expected, payload, |_| Ok(()))
    }
    /// Attempt one typed compare-and-swap without waiting for either lock.
    pub(super) fn try_compare_and_swap(
        &self,
        expected: &TwoSlotSnapshotV1,
        payload: &[u8],
    ) -> Result<TwoSlotCasOutcomeV1, TwoSlotTryErrorV1> {
        self.compare_and_swap_attempt_with(
            expected,
            payload,
            TwoSlotCasModeV1::NonblockingTyped,
            |_| Ok(()),
        )
    }
    fn compare_and_swap_with<Hook>(
        &self,
        expected: &TwoSlotSnapshotV1,
        payload: &[u8],
        after_step: Hook,
    ) -> io::Result<TwoSlotSnapshotV1>
    where
        Hook: FnMut(&'static str) -> io::Result<()>,
    {
        match self.compare_and_swap_attempt_with(
            expected,
            payload,
            TwoSlotCasModeV1::LegacyBlocking,
            after_step,
        ) {
            Ok(TwoSlotCasOutcomeV1::Stored(snapshot)) => Ok(snapshot),
            Ok(TwoSlotCasOutcomeV1::Conflict(_)) => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance two-slot compare-and-swap predecessor changed",
            )),
            Err(TwoSlotTryErrorV1::Io(error)) => Err(error),
            Err(TwoSlotTryErrorV1::Busy) => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance two-slot lock unexpectedly busy in blocking operation",
            )),
        }
    }
    fn compare_and_swap_attempt_with<Hook>(
        &self,
        expected: &TwoSlotSnapshotV1,
        payload: &[u8],
        mode: TwoSlotCasModeV1,
        mut after_step: Hook,
    ) -> Result<TwoSlotCasOutcomeV1, TwoSlotTryErrorV1>
    where
        Hook: FnMut(&'static str) -> io::Result<()>,
    {
        if payload.len() > self.config.max_payload_bytes {
            return Err(TwoSlotTryErrorV1::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance two-slot successor payload is outside its configured bound",
            )));
        }
        if expected.domain != self.config.domain
            || expected.store_nonce != self.config.store_nonce
            || expected.max_payload_bytes != self.config.max_payload_bytes
            || expected.binding_digest != self.binding_digest
        {
            return Err(TwoSlotTryErrorV1::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "governance two-slot predecessor belongs to another store or layout",
            )));
        }
        let operation = |store: &Self| {
            let current = select_two_slot_record_unlocked(store)?;
            if current.generation != expected.generation
                || current.record_digest != expected.record_digest
                || current.payload != expected.payload
            {
                let exact_replay = mode == TwoSlotCasModeV1::NonblockingTyped
                    && expected.generation.checked_add(1) == Some(current.generation)
                    && current.predecessor_digest == expected.record_digest
                    && current.payload == payload;
                let current = two_slot_snapshot(store, current);
                return Ok(if exact_replay {
                    TwoSlotCasOutcomeV1::Stored(current)
                } else {
                    TwoSlotCasOutcomeV1::Conflict(current)
                });
            }
            if current.payload == payload {
                return Ok(TwoSlotCasOutcomeV1::Stored(two_slot_snapshot(
                    store, current,
                )));
            }
            let generation = current.generation.checked_add(1).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance two-slot generation exhausted",
                )
            })?;
            let inactive_id = 1_usize.checked_sub(current.slot_id).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance active slot id is invalid",
                )
            })?;
            verify_two_slot_headers(store)?;
            let active_before = read_two_slot_record_stable(store, current.slot_id)?
                .ok_or_else(|| io::Error::other("governance active slot disappeared"))?;
            if active_before != current {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "governance active two-slot record changed before commit",
                ));
            }
            let inactive = &store.slots[inactive_id];
            write_exact_file_region(
                &inactive.handle,
                store.layout.trailer_offset,
                &vec![0; store.layout.commit_trailer_region_bytes],
            )?;
            after_step("inactive-zero-trailer-written")?;
            inactive.handle.sync_all()?;
            after_step("inactive-trailer-invalidated")?;
            verify_two_slot_headers(store)?;
            let slot_id = u8::try_from(inactive_id)
                .map_err(|_| io::Error::other("governance inactive slot id exceeds u8"))?;
            let header = TwoSlotRecordHeaderV1 {
                format_version: TWO_SLOT_FORMAT_VERSION_V1,
                binding_digest: store.binding_digest,
                slot_id,
                generation,
                predecessor_digest: current.record_digest,
                payload_len: u64::try_from(payload.len()).map_err(|_| {
                    io::Error::other("governance two-slot payload length exceeds u64")
                })?,
                payload_digest: *blake3::hash(payload).as_bytes(),
            };
            let header_region = encode_two_slot_value(
                &TwoSlotRecordHeaderRegionV1 {
                    header: header.clone(),
                    reserved: [0; TWO_SLOT_RECORD_HEADER_RESERVED_BYTES_V1],
                },
                "governance two-slot record-header region",
            )?;
            if header_region.len() != store.layout.record_header_region_bytes {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance two-slot record-header layout changed",
                ));
            }
            write_exact_file_region(
                &inactive.handle,
                u64::try_from(store.layout.header_region_bytes).map_err(|_| {
                    io::Error::other("governance two-slot record offset exceeds u64")
                })?,
                &header_region,
            )?;
            // The authenticated length is the sole semantic payload boundary.
            // Bytes beyond it are private fixed-slot residue and are never
            // decoded, hashed, or returned; avoiding a full 192 MiB wipe keeps
            // short governance commits bounded by their actual payload size.
            write_exact_file_region(&inactive.handle, store.layout.payload_offset, payload)?;
            after_step("inactive-record-written")?;
            inactive.handle.sync_all()?;
            after_step("inactive-record-synced")?;
            verify_two_slot_headers(store)?;
            let record_digest = two_slot_record_digest(&header, payload)?;
            let trailer_region = encode_two_slot_value(
                &TwoSlotCommitTrailerRegionV1 {
                    trailer: TwoSlotCommitTrailerV1 {
                        format_version: TWO_SLOT_FORMAT_VERSION_V1,
                        binding_digest: store.binding_digest,
                        slot_id,
                        generation,
                        record_digest,
                        commit_marker: TWO_SLOT_COMMIT_MARKER_V1,
                    },
                    reserved: [0; TWO_SLOT_COMMIT_TRAILER_RESERVED_BYTES_V1],
                },
                "governance two-slot commit-trailer region",
            )?;
            if trailer_region.len() != store.layout.commit_trailer_region_bytes {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance two-slot commit-trailer layout changed",
                ));
            }
            write_exact_file_region(
                &inactive.handle,
                store.layout.trailer_offset,
                &trailer_region,
            )?;
            after_step("inactive-commit-trailer-written")?;
            inactive.handle.sync_all()?;
            after_step("inactive-commit-trailer-synced")?;
            verify_two_slot_headers(store)?;
            let active_after = read_two_slot_record_stable(store, current.slot_id)?
                .ok_or_else(|| io::Error::other("governance active slot became invalid"))?;
            if active_after != current {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "governance active slot changed during successor commit",
                ));
            }
            let selected = select_two_slot_record_unlocked(store)?;
            if selected.slot_id != inactive_id
                || selected.generation != generation
                || selected.record_digest != record_digest
                || selected.payload != payload
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance two-slot durable successor readback diverged",
                ));
            }
            after_step("successor-readback-verified")?;
            Ok(TwoSlotCasOutcomeV1::Stored(two_slot_snapshot(
                store, selected,
            )))
        };
        match mode {
            TwoSlotCasModeV1::LegacyBlocking => self
                .with_exclusive_lock(operation)
                .map_err(TwoSlotTryErrorV1::Io),
            TwoSlotCasModeV1::NonblockingTyped => self.with_try_exclusive_lock(operation),
        }
    }
    #[cfg(test)]
    fn compare_and_swap_with_test_hook<Hook>(
        &self,
        expected: &TwoSlotSnapshotV1,
        payload: &[u8],
        after_step: Hook,
    ) -> io::Result<TwoSlotSnapshotV1>
    where
        Hook: FnMut(&'static str) -> io::Result<()>,
    {
        self.compare_and_swap_with(expected, payload, after_step)
    }
}
fn two_slot_stage_prefix(config: &TwoSlotStoreConfigV1) -> String {
    format!(
        ".iroha-two-slot-{}-stage-v1-",
        two_slot_store_namespace(config)
    )
}
fn two_slot_lost_found_name(config: &TwoSlotStoreConfigV1) -> OsString {
    format!(
        ".iroha-two-slot-{}-lost-found-v1",
        two_slot_store_namespace(config)
    )
    .into()
}
fn two_slot_init_lock_name(config: &TwoSlotStoreConfigV1) -> OsString {
    format!(
        ".iroha-two-slot-{}-init-lock-v1",
        two_slot_store_namespace(config)
    )
    .into()
}
fn is_canonical_two_slot_stage_name(name: &OsStr, prefix: &str) -> bool {
    fn is_lower_hex(byte: u8) -> bool {
        byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
    }
    let Some(name) = name.to_str() else {
        return false;
    };
    let Some(suffix) = name.strip_prefix(prefix) else {
        return false;
    };
    let Some((process, sequence)) = suffix.split_once('-') else {
        return false;
    };
    process.len() == 16
        && sequence.len() == 16
        && process.bytes().all(is_lower_hex)
        && sequence.bytes().all(is_lower_hex)
}
fn is_canonical_two_slot_lost_found_entry(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let Some(index) = name.strip_prefix("entry-v1-") else {
        return false;
    };
    index.len() == 4 && index.bytes().all(|byte| byte.is_ascii_digit())
}
fn two_slot_stage_inventory(
    directory: &RootedDirectory,
    layout: TwoSlotLayoutV1,
) -> io::Result<TwoSlotStageInventoryV1> {
    let names = directory.child_names_bounded(TWO_SLOT_NAMES_V1.len() + 1)?;
    let max_bytes = two_slot_file_byte_limit(layout)?;
    let mut seen = [false; TWO_SLOT_NAMES_V1.len()];
    let mut byte_count = 0_u64;
    let mut canonical_header_count = 0_usize;
    for name in names {
        let Some(slot_id) = TWO_SLOT_NAMES_V1
            .iter()
            .position(|expected| name == OsStr::new(expected))
        else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "governance two-slot recovery directory `{}` contains a non-slot entry",
                    directory.display_path.display()
                ),
            ));
        };
        if seen[slot_id] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot recovery inventory contains a duplicate slot name",
            ));
        }
        seen[slot_id] = true;
        let handle = platform::open_read_write_file(&directory.handle, &name)?;
        let metadata = handle.metadata()?;
        let path = directory.display_path.join(&name);
        validate_file_metadata(&path, &metadata, max_bytes, true)?;
        let identity = file_identity(&metadata)?;
        directory.verify_file_binding(&name, &handle, identity, max_bytes, true)?;
        byte_count = byte_count.checked_add(metadata.len()).ok_or_else(|| {
            io::Error::other("governance two-slot recovery byte count overflowed")
        })?;
        if metadata.len() == layout.slot_file_bytes {
            let encoded = read_exact_file_region(&handle, 0, layout.header_region_bytes)?;
            if let Ok(region) = decode_two_slot_value::<TwoSlotHeaderRegionV1>(
                &encoded,
                "governance two-slot recovery header",
            ) && region.reserved == [0; TWO_SLOT_HEADER_RESERVED_BYTES_V1]
                && region.header.binding.format_version == TWO_SLOT_FORMAT_VERSION_V1
                && region.header.slot_id == u8::try_from(slot_id).unwrap_or(u8::MAX)
            {
                canonical_header_count =
                    canonical_header_count.checked_add(1).ok_or_else(|| {
                        io::Error::other("governance two-slot header count overflowed")
                    })?;
            }
        }
        directory.verify_file_binding(&name, &handle, identity, max_bytes, true)?;
    }
    directory.verify()?;
    Ok(TwoSlotStageInventoryV1 {
        byte_count,
        has_full_pair: seen.into_iter().all(|present| present),
        canonical_header_count,
    })
}
fn two_slot_initial_stage_is_complete(
    store: &TwoSlotStoreV1,
    initial_payload: &[u8],
) -> io::Result<bool> {
    store.with_exclusive_lock(|store| {
        verify_two_slot_headers(store)?;
        let left = read_two_slot_record_stable(store, 0)?;
        let right = read_two_slot_record_stable(store, 1)?;
        match (left, right) {
            (None, None) => Ok(false),
            (Some(record), None)
                if record.slot_id == 0
                    && record.generation == 1
                    && record.predecessor_digest == TWO_SLOT_ZERO_DIGEST
                    && record.payload == initial_payload =>
            {
                Ok(true)
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot stage contains a divergent committed history",
            )),
        }
    })
}
fn classify_two_slot_stage(
    name: OsString,
    directory: RootedDirectory,
    config: &TwoSlotStoreConfigV1,
    init_lock_identity: FileIdentity,
    initial_payload: &[u8],
) -> io::Result<TwoSlotStageV1> {
    let layout = two_slot_layout(config.max_payload_bytes)?;
    let inventory = two_slot_stage_inventory(&directory, layout)?;
    let complete = match open_existing_two_slot_store(
        directory.clone(),
        config.clone(),
        init_lock_identity,
    ) {
        Ok(store) => two_slot_initial_stage_is_complete(&store, initial_payload)?,
        Err(error) => {
            if inventory.has_full_pair && inventory.canonical_header_count == 2 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "governance two-slot stage `{}` has complete typed headers but a divergent binding: {error}",
                        directory.display_path.display()
                    ),
                ));
            }
            false
        }
    };
    Ok(TwoSlotStageV1 {
        name,
        directory,
        byte_count: inventory.byte_count,
        complete,
    })
}
fn collect_two_slot_stages(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
    init_lock_identity: FileIdentity,
    initial_payload: &[u8],
) -> io::Result<Vec<TwoSlotStageV1>> {
    let prefix = two_slot_stage_prefix(config);
    let mut stage_names = Vec::new();
    for name in root.child_names_bounded(DEFAULT_CHILD_ENTRY_LIMIT)? {
        if !name.as_encoded_bytes().starts_with(prefix.as_bytes()) {
            continue;
        }
        if !is_canonical_two_slot_stage_name(&name, &prefix) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "governance two-slot stage name `{}` is noncanonical",
                    name.to_string_lossy()
                ),
            ));
        }
        stage_names.push(name);
        if stage_names.len() > TWO_SLOT_STAGE_ENTRY_HARD_CAP_V1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot staging entry cap is exceeded",
            ));
        }
    }
    stage_names.sort();
    stage_names
        .into_iter()
        .map(|name| {
            let directory = root.open_directory(&name)?;
            classify_two_slot_stage(name, directory, config, init_lock_identity, initial_payload)
        })
        .collect()
}
fn open_or_create_two_slot_lost_found(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
) -> io::Result<RootedDirectory> {
    let name = two_slot_lost_found_name(config);
    match root.open_directory(&name) {
        Ok(directory) => Ok(directory),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match root.create_child_directory_exclusive(&name) {
                Ok(directory) => {
                    root.sync_all()?;
                    Ok(directory)
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    root.open_directory(&name)
                }
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}
fn two_slot_lost_found_state(
    directory: &RootedDirectory,
    layout: TwoSlotLayoutV1,
) -> io::Result<([bool; TWO_SLOT_LOST_FOUND_ENTRY_HARD_CAP_V1], usize, u64)> {
    let names = directory.child_names_bounded(TWO_SLOT_LOST_FOUND_ENTRY_HARD_CAP_V1)?;
    let mut occupied = [false; TWO_SLOT_LOST_FOUND_ENTRY_HARD_CAP_V1];
    let mut total_bytes = 0_u64;
    for name in names {
        if !is_canonical_two_slot_lost_found_entry(&name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "governance two-slot lost+found entry `{}` is noncanonical",
                    name.to_string_lossy()
                ),
            ));
        }
        let index = name
            .to_str()
            .and_then(|name| name.strip_prefix("entry-v1-"))
            .and_then(|index| index.parse::<usize>().ok())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance two-slot lost+found index is invalid",
                )
            })?;
        if index >= occupied.len() || occupied[index] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot lost+found index is duplicated or out of bounds",
            ));
        }
        occupied[index] = true;
        let child = directory.open_directory(&name)?;
        let inventory = two_slot_stage_inventory(&child, layout)?;
        total_bytes = total_bytes
            .checked_add(inventory.byte_count)
            .ok_or_else(|| io::Error::other("governance lost+found byte count overflowed"))?;
        if total_bytes > TWO_SLOT_LOST_FOUND_TOTAL_MAX_BYTES_V1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot lost+found byte cap is exceeded",
            ));
        }
    }
    let entry_count = occupied.iter().filter(|occupied| **occupied).count();
    directory.verify()?;
    Ok((occupied, entry_count, total_bytes))
}
fn quarantine_two_slot_stages(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
    stages: Vec<TwoSlotStageV1>,
) -> io::Result<bool> {
    if stages.is_empty() {
        return Ok(true);
    }
    let layout = two_slot_layout(config.max_payload_bytes)?;
    let lost_found = open_or_create_two_slot_lost_found(root, config)?;
    let (mut occupied, entry_count, existing_bytes) =
        two_slot_lost_found_state(&lost_found, layout)?;
    let required_bytes = stages.iter().try_fold(0_u64, |total, stage| {
        total
            .checked_add(stage.byte_count)
            .ok_or_else(|| io::Error::other("governance two-slot quarantine byte count overflowed"))
    })?;
    if entry_count
        .checked_add(stages.len())
        .is_none_or(|count| count > TWO_SLOT_LOST_FOUND_ENTRY_HARD_CAP_V1)
        || existing_bytes
            .checked_add(required_bytes)
            .is_none_or(|bytes| bytes > TWO_SLOT_LOST_FOUND_TOTAL_MAX_BYTES_V1)
    {
        return Ok(false);
    }
    for stage in stages {
        let inventory = two_slot_stage_inventory(&stage.directory, layout)?;
        if inventory.byte_count != stage.byte_count {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance two-slot stage changed before quarantine",
            ));
        }
        let index = occupied
            .iter()
            .position(|occupied| !*occupied)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "governance two-slot lost+found has no free bounded entry",
                )
            })?;
        let destination_name = OsString::from(format!("entry-v1-{index:04}"));
        root.move_child_directory_exclusive(
            stage.directory.clone(),
            &lost_found,
            &destination_name,
        )?;
        root.sync_all()?;
        lost_found.sync_all()?;
        occupied[index] = true;
    }
    Ok(true)
}
fn create_unique_two_slot_stage(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
) -> io::Result<(OsString, RootedDirectory)> {
    let prefix = two_slot_stage_prefix(config);
    let existing = root
        .child_names_bounded(DEFAULT_CHILD_ENTRY_LIMIT)?
        .into_iter()
        .filter(|name| name.as_encoded_bytes().starts_with(prefix.as_bytes()))
        .count();
    if existing >= TWO_SLOT_STAGE_ENTRY_HARD_CAP_V1 {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "governance two-slot staging entry cap is exhausted",
        ));
    }
    for _ in 0..TWO_SLOT_STAGE_ENTRY_HARD_CAP_V1 {
        let sequence = TWO_SLOT_STAGE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = OsString::from(format!(
            "{prefix}{:016x}-{sequence:016x}",
            u64::from(std::process::id())
        ));
        match root.create_child_directory_exclusive(&name) {
            Ok(directory) => return Ok((name, directory)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "governance two-slot could not allocate a unique bounded staging name",
    ))
}
fn create_two_slot_file<Hook>(
    directory: &RootedDirectory,
    name: &OsStr,
    layout: TwoSlotLayoutV1,
    labels: [&'static str; 2],
    after_step: &mut Hook,
) -> io::Result<TwoSlotFileV1>
where
    Hook: FnMut(&'static str) -> io::Result<()>,
{
    let handle = platform::create_file(&directory.handle, name)?;
    let path = directory.display_path.join(name);
    let max_bytes = two_slot_file_byte_limit(layout)?;
    let before = handle.metadata()?;
    validate_file_metadata(&path, &before, max_bytes, true)?;
    if before.len() != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "new governance two-slot file was not empty",
        ));
    }
    let identity = file_identity(&before)?;
    directory.verify_file_binding(name, &handle, identity, max_bytes, true)?;
    after_step(labels[0])?;
    handle.set_len(layout.slot_file_bytes)?;
    let after = handle.metadata()?;
    validate_file_metadata(&path, &after, max_bytes, true)?;
    if after.len() != layout.slot_file_bytes || file_identity(&after)? != identity {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "new governance two-slot file changed while being sized",
        ));
    }
    directory.verify_file_binding(name, &handle, identity, max_bytes, true)?;
    after_step(labels[1])?;
    Ok(TwoSlotFileV1 {
        handle: Arc::new(handle),
        identity,
        name: name.to_os_string(),
    })
}
fn write_two_slot_record_unlocked<Hook>(
    store: &TwoSlotStoreV1,
    slot_id: usize,
    generation: u64,
    predecessor_digest: [u8; 32],
    payload: &[u8],
    labels: [&'static str; 6],
    after_step: &mut Hook,
) -> io::Result<TwoSlotCommittedRecordV1>
where
    Hook: FnMut(&'static str) -> io::Result<()>,
{
    if generation == 0
        || (generation == 1 && predecessor_digest != TWO_SLOT_ZERO_DIGEST)
        || (generation > 1 && predecessor_digest == TWO_SLOT_ZERO_DIGEST)
        || payload.len() > store.config.max_payload_bytes
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance two-slot record generation, lineage, or payload is invalid",
        ));
    }
    let slot = store.slots.get(slot_id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance two-slot id is invalid",
        )
    })?;
    verify_two_slot_headers(store)?;
    write_exact_file_region(
        &slot.handle,
        store.layout.trailer_offset,
        &vec![0; store.layout.commit_trailer_region_bytes],
    )?;
    after_step(labels[0])?;
    slot.handle.sync_all()?;
    after_step(labels[1])?;
    verify_two_slot_headers(store)?;
    let encoded_slot_id =
        u8::try_from(slot_id).map_err(|_| io::Error::other("governance two-slot id exceeds u8"))?;
    let header = TwoSlotRecordHeaderV1 {
        format_version: TWO_SLOT_FORMAT_VERSION_V1,
        binding_digest: store.binding_digest,
        slot_id: encoded_slot_id,
        generation,
        predecessor_digest,
        payload_len: u64::try_from(payload.len())
            .map_err(|_| io::Error::other("governance two-slot payload exceeds u64"))?,
        payload_digest: *blake3::hash(payload).as_bytes(),
    };
    let header_region = encode_two_slot_value(
        &TwoSlotRecordHeaderRegionV1 {
            header: header.clone(),
            reserved: [0; TWO_SLOT_RECORD_HEADER_RESERVED_BYTES_V1],
        },
        "governance two-slot record-header region",
    )?;
    if header_region.len() != store.layout.record_header_region_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot record-header layout changed",
        ));
    }
    let record_offset = u64::try_from(store.layout.header_region_bytes)
        .map_err(|_| io::Error::other("governance two-slot record offset exceeds u64"))?;
    write_exact_file_region(&slot.handle, record_offset, &header_region)?;
    write_exact_file_region(&slot.handle, store.layout.payload_offset, payload)?;
    after_step(labels[2])?;
    slot.handle.sync_all()?;
    after_step(labels[3])?;
    verify_two_slot_headers(store)?;
    let record_digest = two_slot_record_digest(&header, payload)?;
    let trailer_region = encode_two_slot_value(
        &TwoSlotCommitTrailerRegionV1 {
            trailer: TwoSlotCommitTrailerV1 {
                format_version: TWO_SLOT_FORMAT_VERSION_V1,
                binding_digest: store.binding_digest,
                slot_id: encoded_slot_id,
                generation,
                record_digest,
                commit_marker: TWO_SLOT_COMMIT_MARKER_V1,
            },
            reserved: [0; TWO_SLOT_COMMIT_TRAILER_RESERVED_BYTES_V1],
        },
        "governance two-slot commit-trailer region",
    )?;
    if trailer_region.len() != store.layout.commit_trailer_region_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot commit-trailer layout changed",
        ));
    }
    write_exact_file_region(&slot.handle, store.layout.trailer_offset, &trailer_region)?;
    after_step(labels[4])?;
    slot.handle.sync_all()?;
    after_step(labels[5])?;
    verify_two_slot_headers(store)?;
    let committed = read_two_slot_record_stable(store, slot_id)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot committed record failed exact readback",
        )
    })?;
    if committed.generation != generation
        || committed.predecessor_digest != predecessor_digest
        || committed.record_digest != record_digest
        || committed.payload != payload
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot committed record readback diverged",
        ));
    }
    Ok(committed)
}
fn initialize_two_slot_stage<Hook>(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
    init_lock_identity: FileIdentity,
    initial_payload: &[u8],
    after_step: &mut Hook,
) -> io::Result<TwoSlotStageV1>
where
    Hook: FnMut(&'static str) -> io::Result<()>,
{
    let layout = two_slot_layout(config.max_payload_bytes)?;
    let (name, directory) = create_unique_two_slot_stage(root, config)?;
    after_step("stage-directory-created")?;
    root.sync_all()?;
    after_step("stage-parent-synced")?;
    let slot_0 = create_two_slot_file(
        &directory,
        OsStr::new(TWO_SLOT_NAMES_V1[0]),
        layout,
        ["slot-0-created", "slot-0-sized"],
        after_step,
    )?;
    slot_0.handle.sync_all()?;
    after_step("slot-0-sized-and-synced")?;
    let slot_1 = create_two_slot_file(
        &directory,
        OsStr::new(TWO_SLOT_NAMES_V1[1]),
        layout,
        ["slot-1-created", "slot-1-sized"],
        after_step,
    )?;
    slot_1.handle.sync_all()?;
    after_step("slot-1-sized-and-synced")?;
    let material = two_slot_binding_material(
        config,
        layout,
        init_lock_identity,
        [slot_0.identity, slot_1.identity],
    )?;
    let binding_digest = two_slot_binding_digest(&material)?;
    for (slot_id, slot) in [&slot_0, &slot_1].into_iter().enumerate() {
        let header = expected_two_slot_header_region(&material, binding_digest, slot_id)?;
        if header.len() != layout.header_region_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance two-slot immutable header layout changed",
            ));
        }
        write_exact_file_region(&slot.handle, 0, &header)?;
        after_step(if slot_id == 0 {
            "slot-0-header-written"
        } else {
            "slot-1-header-written"
        })?;
        slot.handle.sync_all()?;
        after_step(if slot_id == 0 {
            "slot-0-header-synced"
        } else {
            "slot-1-header-synced"
        })?;
    }
    let store = TwoSlotStoreV1 {
        directory: directory.clone(),
        config: config.clone(),
        layout,
        init_lock_identity,
        binding_digest,
        slots: [slot_0, slot_1],
        process_lock: Arc::new(Mutex::new(())),
    };
    write_two_slot_record_unlocked(
        &store,
        0,
        1,
        TWO_SLOT_ZERO_DIGEST,
        initial_payload,
        [
            "initial-trailer-invalidated",
            "initial-trailer-invalidation-synced",
            "initial-record-written",
            "initial-record-synced",
            "initial-commit-trailer-written",
            "initial-commit-trailer-synced",
        ],
        after_step,
    )?;
    after_step("initial-record-readback-verified")?;
    directory.sync_all()?;
    after_step("stage-directory-synced")?;
    let stage =
        classify_two_slot_stage(name, directory, config, init_lock_identity, initial_payload)?;
    if !stage.complete {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "new governance two-slot stage is not complete after durable initialization",
        ));
    }
    Ok(stage)
}
fn promote_two_slot_stage<Hook>(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
    init_lock_identity: FileIdentity,
    initial_payload: &[u8],
    stage: TwoSlotStageV1,
    after_step: &mut Hook,
) -> io::Result<TwoSlotStoreV1>
where
    Hook: FnMut(&'static str) -> io::Result<()>,
{
    let stage = classify_two_slot_stage(
        stage.name,
        stage.directory,
        config,
        init_lock_identity,
        initial_payload,
    )?;
    if !stage.complete {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "governance two-slot stage became incomplete before promotion",
        ));
    }
    after_step("before-directory-rename")?;
    let stage = classify_two_slot_stage(
        stage.name,
        stage.directory,
        config,
        init_lock_identity,
        initial_payload,
    )?;
    if !stage.complete {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "governance two-slot stage changed at the promotion boundary",
        ));
    }
    let installed =
        root.move_child_directory_exclusive(stage.directory, root, OsStr::new(&config.store_name))?;
    after_step("directory-renamed")?;
    root.sync_all()?;
    after_step("parent-synced")?;
    let store = open_existing_two_slot_store(installed, config.clone(), init_lock_identity)?;
    if !two_slot_initial_stage_is_complete(&store, initial_payload)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "promoted governance two-slot store lost its initial record",
        ));
    }
    after_step("initialization-postcheck")?;
    Ok(store)
}
fn open_valid_two_slot_canonical(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
    init_lock_identity: FileIdentity,
) -> io::Result<Option<TwoSlotStoreV1>> {
    match root.open_directory(OsStr::new(&config.store_name)) {
        Ok(directory) => {
            let store =
                open_existing_two_slot_store(directory, config.clone(), init_lock_identity)?;
            let _ = store.load()?;
            Ok(Some(store))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}
fn load_existing_two_slot_store_v1(
    root: &RootedDirectory,
    config: TwoSlotStoreConfigV1,
) -> io::Result<TwoSlotSnapshotV1> {
    let store = open_existing_read_only_two_slot_store_v1(root, config)?;
    let snapshot = store.load()?;
    root.verify()?;
    Ok(snapshot)
}
fn open_existing_read_only_two_slot_store_v1(
    root: &RootedDirectory,
    config: TwoSlotStoreConfigV1,
) -> io::Result<TwoSlotStoreV1> {
    if root.writable {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "existing governance two-slot reads require a read-only rooted capability",
        ));
    }
    root.verify()?;
    let init_lock_name = two_slot_init_lock_name(&config);
    let init_lock = platform::open_file(&root.handle, &init_lock_name, false)?;
    let init_lock_metadata = init_lock.metadata()?;
    let init_lock_path = root.display_path.join(&init_lock_name);
    validate_file_metadata(&init_lock_path, &init_lock_metadata, 0, true)?;
    if init_lock_metadata.len() != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance two-slot init lock must remain an empty fixed file",
        ));
    }
    let init_lock_identity = file_identity(&init_lock_metadata)?;
    root.verify_file_binding(&init_lock_name, &init_lock, init_lock_identity, 0, true)?;
    let directory = root.open_directory(OsStr::new(&config.store_name))?;
    let store = open_existing_two_slot_store(directory, config, init_lock_identity)?;
    let _ = store.load()?;
    root.verify_file_binding(&init_lock_name, &init_lock, init_lock_identity, 0, true)?;
    root.verify()?;
    Ok(store)
}
fn open_or_create_two_slot_store_v1_once<Hook>(
    root: &RootedDirectory,
    config: &TwoSlotStoreConfigV1,
    init_lock_identity: FileIdentity,
    initial_payload: &[u8],
    after_step: &mut Hook,
) -> io::Result<TwoSlotStoreV1>
where
    Hook: FnMut(&'static str) -> io::Result<()>,
{
    root.verify()?;
    if let Some(store) = open_valid_two_slot_canonical(root, config, init_lock_identity)? {
        let stages = collect_two_slot_stages(root, config, init_lock_identity, initial_payload)?;
        // A valid canonical store remains available even when bounded
        // preservation space is full. Every stage remains untouched for
        // offline archival, while divergent stages still fail during the
        // classification above.
        let _all_preserved = quarantine_two_slot_stages(root, config, stages)?;
        return Ok(store);
    }
    let mut stages = collect_two_slot_stages(root, config, init_lock_identity, initial_payload)?;
    if let Some(index) = stages.iter().position(|stage| stage.complete) {
        let selected = stages.remove(index);
        if !quarantine_two_slot_stages(root, config, stages)? {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "governance two-slot lost+found capacity is exhausted; archive it offline",
            ));
        }
        return promote_two_slot_stage(
            root,
            config,
            init_lock_identity,
            initial_payload,
            selected,
            after_step,
        );
    }
    if !quarantine_two_slot_stages(root, config, stages)? {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "governance two-slot lost+found capacity is exhausted; archive it offline",
        ));
    }
    let stage = initialize_two_slot_stage(
        root,
        config,
        init_lock_identity,
        initial_payload,
        after_step,
    )?;
    promote_two_slot_stage(
        root,
        config,
        init_lock_identity,
        initial_payload,
        stage,
        after_step,
    )
}
fn open_or_create_two_slot_store_v1_with<Hook>(
    root: &RootedDirectory,
    config: TwoSlotStoreConfigV1,
    initial_payload: &[u8],
    after_step: Hook,
) -> io::Result<TwoSlotStoreV1>
where
    Hook: FnMut(&'static str) -> io::Result<()>,
{
    open_or_create_two_slot_store_v1_with_mode(
        root,
        config,
        initial_payload,
        TwoSlotInitializationLockModeV1::Blocking,
        after_step,
    )
}
fn open_or_create_two_slot_store_v1_with_mode<Hook>(
    root: &RootedDirectory,
    config: TwoSlotStoreConfigV1,
    initial_payload: &[u8],
    lock_mode: TwoSlotInitializationLockModeV1,
    mut after_step: Hook,
) -> io::Result<TwoSlotStoreV1>
where
    Hook: FnMut(&'static str) -> io::Result<()>,
{
    if !root.writable {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "read-only governance directory cannot open a mutable two-slot store",
        ));
    }
    if initial_payload.len() > config.max_payload_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance two-slot initial payload exceeds its configured bound",
        ));
    }
    let init_file_lock = match lock_mode {
        TwoSlotInitializationLockModeV1::Blocking => TwoSlotInitFileLockV1::acquire(root, &config)?,
        TwoSlotInitializationLockModeV1::Bounded(wait) => {
            TwoSlotInitFileLockV1::acquire_bounded(root, &config, wait)?
        }
    };
    const RACE_RETRIES: usize = 4;
    let result = (|| {
        for attempt in 0..RACE_RETRIES {
            init_file_lock.verify()?;
            match open_or_create_two_slot_store_v1_once(
                root,
                &config,
                init_file_lock.identity,
                initial_payload,
                &mut after_step,
            ) {
                Err(error)
                    if attempt + 1 < RACE_RETRIES
                        && matches!(
                            error.kind(),
                            io::ErrorKind::AlreadyExists
                                | io::ErrorKind::NotFound
                                | io::ErrorKind::WouldBlock
                        ) =>
                {
                    continue;
                }
                result => return result,
            }
        }
        Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "governance two-slot initialization race did not converge",
        ))
    })();
    let unlock = init_file_lock.release();
    match (result, unlock) {
        (Ok(store), Ok(())) => Ok(store),
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
    }
}
