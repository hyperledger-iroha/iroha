//! Nonmonetary first-device authentication-key creation and read-only recovery.
//!
//! The only fresh grant is private process memory created after this invocation durably
//! publishes a new intent. A restored intent never reconstructs it. Its consumed marker is
//! synced before the generation upcall. Authentication data is not Wallet enrollment,
//! payment signing authority, a hardware journal, or a verified Core verdict.
//!
//! An immutable root membership record is published before its operation directory; loss
//! of that child cannot make the operation fresh again. The released application also
//! durably consumes its one initial Native dispatch before calling reserve, and routes
//! every later call to restore. That independent app history protects complete loss of
//! this Native root. Correct released code and a stock uncompromised OS are required;
//! software records cannot detect erasure of every independent history or prove a
//! hardware counter/nonforking journal.

use std::{
    ffi::CString,
    fs::File,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd, RawFd},
    path::{Component, Path, PathBuf},
};

use norito::NoritoSchema;
use rand::TryRngCore as _;

const DOMAIN: &[u8] = b"BPNG.FIRST_DEVICE.AUTH.CHALLENGE.V1\0";
const ROOT: &str = "iroha-first-device-auth-key-v1";
const RECORD_MAX: usize = 4096;
const TRANSCRIPT_LEN: usize = 276;

/// Historical observed key data, never a verified hardware or login result.
pub(crate) struct KeyData {
    pub(crate) slot: [u8; 32],
    pub(crate) public_key: Vec<u8>,
}

/// Every failure is recover-only. No error authorizes another generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Error {
    Invalid,
    Unavailable,
    Busy,
    Changed,
    ExistingIntent,
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

/// Positive lookup remains usable on API26–30; absence and errors never grant freshness.
pub(crate) enum Probe {
    Present(Vec<u8>),
    Unavailable,
}

/// The typed StrongBox refusal alone permits one different fresh TEE slot in this invocation.
pub(crate) enum Generated {
    Present(Vec<u8>),
    StrongBoxUnavailable,
    Unavailable,
}

/// The retained Kotlin platform has private JNI upcalls, distinct from Wallet's platform.
pub(crate) trait Platform {
    fn require_original(&self) -> Result<()>;
    fn transcript(&self) -> Result<Vec<u8>>;
    fn no_backup_root(&self) -> Result<PathBuf>;
    fn api_level(&self) -> Result<u32>;
    fn probe(&self, slot: &[u8; 32]) -> Probe;
    fn generate(&self, slot: &[u8; 32], digest: &[u8; 32], strong_box: bool) -> Generated;
}

#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::first_device_auth_key_v1::IntentV1")]
struct Intent {
    version: u16,
    transcript: Vec<u8>,
    slot: [u8; 32],
    attempt: u8,
    strong_box: bool,
}

#[derive(PartialEq, Eq, norito::Encode, norito::Decode, NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::first_device_auth_key_v1::ConsumedV1")]
struct Consumed {
    version: u16,
    intent_sha256: [u8; 32],
}

#[derive(PartialEq, Eq, norito::Encode, norito::Decode, NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::first_device_auth_key_v1::SelectedV1")]
struct Selected {
    version: u16,
    intent_sha256: [u8; 32],
    public_key: Vec<u8>,
}

#[derive(PartialEq, Eq, norito::Encode, norito::Decode, NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::first_device_auth_key_v1::StrongBoxRefusedV1")]
struct StrongBoxRefused {
    version: u16,
    intent_sha256: [u8; 32],
}

#[derive(PartialEq, Eq, norito::Encode, norito::Decode, NoritoSchema)]
#[norito_schema(name = "connect_norito_bridge::first_device_auth_key_v1::OperationMembershipV1")]
struct OperationMembership {
    version: u16,
    transcript: Vec<u8>,
}

/// No constructor, byte decoder or restored-flow path can produce this capability.
struct FreshGrant {
    intent: Intent,
    original: Vec<u8>,
    consumed: bool,
}

impl FreshGrant {
    fn consume(&mut self, directory: &Directory) -> Result<()> {
        if self.consumed {
            return Err(Error::ExistingIntent);
        }
        // Consume process memory first. Any publication/sync error leaves this invocation
        // unable to retry generation, even if a caller catches the error.
        self.consumed = true;
        directory.require_bytes(&intent_name(self.intent.attempt), &self.original)?;
        directory.write_new(
            &consumed_name(self.intent.attempt),
            &encode(&Consumed {
                version: 1,
                intent_sha256: iroha_crypto::sha256(&self.original),
            })?,
        )
    }
}

fn encode<T: norito::NoritoSerialize>(value: &T) -> Result<Vec<u8>> {
    let bytes = norito::encode_canonical(value).map_err(|_| Error::Invalid)?;
    if bytes.len() > RECORD_MAX {
        return Err(Error::Invalid);
    }
    Ok(bytes)
}

fn decode<T>(bytes: &[u8]) -> Result<T>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > RECORD_MAX {
        return Err(Error::Invalid);
    }
    norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
        .map_err(|_| Error::Invalid)
}

fn operation(transcript: &[u8]) -> Result<[u8; 32]> {
    if transcript.len() != TRANSCRIPT_LEN || !transcript.starts_with(DOMAIN) {
        return Err(Error::Invalid);
    }
    let offset = DOMAIN.len();
    let operation = transcript[offset + 32..offset + 64]
        .try_into()
        .map_err(|_| Error::Invalid)?;
    let issued = u64::from_le_bytes(
        transcript[260..268]
            .try_into()
            .map_err(|_| Error::Invalid)?,
    );
    let expires = u64::from_le_bytes(
        transcript[268..276]
            .try_into()
            .map_err(|_| Error::Invalid)?,
    );
    if operation == [0; 32]
        || issued == 0
        || expires <= issued
        || expires - issued > 600_000
        || transcript[offset + 64..offset + 96] == transcript[offset + 96..offset + 128]
    {
        return Err(Error::Invalid);
    }
    Ok(operation)
}

fn point(bytes: Vec<u8>) -> Result<Vec<u8>> {
    // This projection is DATA only. The actual Kotlin reader validates the original
    // curve/KeyInfo/certificate chain; Core independently verifies the authentic chain.
    if bytes.len() != 65 || bytes[0] != 4 || bytes[1..].iter().all(|byte| *byte == 0) {
        return Err(Error::Invalid);
    }
    Ok(bytes)
}

fn intent_name(attempt: u8) -> String {
    format!("intent-{attempt}.norito")
}
fn consumed_name(attempt: u8) -> String {
    format!("consumed-{attempt}.norito")
}

fn require_platform(platform: &impl Platform, transcript: &[u8]) -> Result<()> {
    platform.require_original()?;
    if platform.transcript()? != transcript {
        return Err(Error::Changed);
    }
    Ok(())
}

fn new_grant(
    directory: &Directory,
    transcript: &[u8],
    attempt: u8,
    strong_box: bool,
) -> Result<FreshGrant> {
    let mut slot = [0; 32];
    rand::rngs::OsRng
        .try_fill_bytes(&mut slot)
        .map_err(|_| Error::Unavailable)?;
    if slot == [0; 32] {
        return Err(Error::Unavailable);
    }
    let intent = Intent {
        version: 1,
        transcript: transcript.to_vec(),
        slot,
        attempt,
        strong_box,
    };
    let original = encode(&intent)?;
    directory.write_new(&intent_name(attempt), &original)?;
    // Freshness is the result of this exact acknowledged create-new publication,
    // not a filesystem absence probe or a caller-supplied flag.
    Ok(FreshGrant {
        intent,
        original,
        consumed: false,
    })
}

fn generate(
    platform: &impl Platform,
    directory: &Directory,
    transcript: &[u8],
    grant: &mut FreshGrant,
) -> Result<Generated> {
    require_platform(platform, transcript)?;
    directory.recheck()?;
    grant.consume(directory)?;
    require_platform(platform, transcript)?;
    directory.require_bytes(&intent_name(grant.intent.attempt), &grant.original)?;
    directory.require_bytes(
        &consumed_name(grant.intent.attempt),
        &encode(&Consumed {
            version: 1,
            intent_sha256: iroha_crypto::sha256(&grant.original),
        })?,
    )?;
    // There is no callback before the private grant is consumed and the marker's
    // data and directory entries are synced. An exception never recreates it.
    let result = platform.generate(
        &grant.intent.slot,
        &iroha_crypto::sha256(transcript),
        grant.intent.strong_box,
    );
    require_platform(platform, transcript)?;
    directory.recheck()?;
    directory.require_bytes(&intent_name(grant.intent.attempt), &grant.original)?;
    directory.require_bytes(
        &consumed_name(grant.intent.attempt),
        &encode(&Consumed {
            version: 1,
            intent_sha256: iroha_crypto::sha256(&grant.original),
        })?,
    )?;
    Ok(result)
}

fn select(
    platform: &impl Platform,
    directory: &Directory,
    transcript: &[u8],
    grant: &FreshGrant,
    reported: Vec<u8>,
) -> Result<KeyData> {
    let reported = point(reported)?;
    require_platform(platform, transcript)?;
    let observed = match platform.probe(&grant.intent.slot) {
        Probe::Present(bytes) => point(bytes)?,
        Probe::Unavailable => return Err(Error::Unavailable),
    };
    if observed != reported {
        return Err(Error::Changed);
    }
    directory.require_bytes(&intent_name(grant.intent.attempt), &grant.original)?;
    directory.write_new(
        "selected.norito",
        &encode(&Selected {
            version: 1,
            intent_sha256: iroha_crypto::sha256(&grant.original),
            public_key: observed.clone(),
        })?,
    )?;
    require_platform(platform, transcript)?;
    directory.recheck()?;
    Ok(KeyData {
        slot: grant.intent.slot,
        public_key: observed,
    })
}

/// Reserve only a genuinely new operation directory. Existing and restored histories recover only.
pub(crate) fn reserve_original(platform: &impl Platform, transcript: &[u8]) -> Result<KeyData> {
    let operation = operation(transcript)?;
    require_platform(platform, transcript)?;
    let api = platform.api_level()?;
    if api < 26 {
        return Err(Error::Invalid);
    }
    let root = RootLease::open(&platform.no_backup_root()?, true)?;
    let (directory, created) = root.operation(&operation, transcript, true)?;
    if !created {
        return recover_in(platform, &directory, transcript);
    }
    let mut first = new_grant(&directory, transcript, 0, api >= 28)?;
    match generate(platform, &directory, transcript, &mut first)? {
        Generated::Present(bytes) => select(platform, &directory, transcript, &first, bytes),
        Generated::Unavailable => Err(Error::Unavailable),
        Generated::StrongBoxUnavailable if first.intent.strong_box => {
            // This authorization is local to the original typed StrongBox outcome. Its
            // slot and intent are new; no restored record recreates a fallback grant.
            directory.write_new(
                "strongbox-refused.norito",
                &encode(&StrongBoxRefused {
                    version: 1,
                    intent_sha256: iroha_crypto::sha256(&first.original),
                })?,
            )?;
            let mut fallback = new_grant(&directory, transcript, 1, false)?;
            if fallback.intent.slot == first.intent.slot {
                return Err(Error::Invalid);
            }
            match generate(platform, &directory, transcript, &mut fallback)? {
                Generated::Present(bytes) => {
                    select(platform, &directory, transcript, &fallback, bytes)
                }
                Generated::StrongBoxUnavailable | Generated::Unavailable => Err(Error::Unavailable),
            }
        }
        Generated::StrongBoxUnavailable => Err(Error::Invalid),
    }
}

/// Read only; never creates a root, operation, intent, grant, selection, or key.
pub(crate) fn restore_original(platform: &impl Platform, transcript: &[u8]) -> Result<KeyData> {
    let operation = operation(transcript)?;
    require_platform(platform, transcript)?;
    let root = RootLease::open(&platform.no_backup_root()?, false)?;
    let (directory, _) = root.operation(&operation, transcript, false)?;
    recover_in(platform, &directory, transcript)
}

fn recover_in(
    platform: &impl Platform,
    directory: &Directory,
    transcript: &[u8],
) -> Result<KeyData> {
    require_platform(platform, transcript)?;
    directory.recheck()?;
    let selected_original = directory.read("selected.norito")?;
    let selected = selected_original
        .as_ref()
        .map(|bytes| decode::<Selected>(bytes))
        .transpose()?;
    if selected.as_ref().is_some_and(|value| value.version != 1) {
        return Err(Error::Invalid);
    }
    let mut present: Option<KeyData> = None;
    for attempt in 0..=1 {
        let Some(original) = directory.read(&intent_name(attempt))? else {
            if attempt == 0 {
                return Err(Error::Unavailable);
            }
            continue;
        };
        let intent: Intent = decode(&original)?;
        if intent.version != 1
            || intent.attempt != attempt
            || intent.slot == [0; 32]
            || intent.transcript != transcript
            || (attempt == 1 && intent.strong_box)
        {
            return Err(Error::Changed);
        }
        if attempt == 1 {
            let first_original = directory.read(&intent_name(0))?.ok_or(Error::Changed)?;
            let first: Intent = decode(&first_original)?;
            let refused_original = directory
                .read("strongbox-refused.norito")?
                .ok_or(Error::Changed)?;
            let refused: StrongBoxRefused = decode(&refused_original)?;
            let first_consumed_original =
                directory.read(&consumed_name(0))?.ok_or(Error::Changed)?;
            let first_consumed: Consumed = decode(&first_consumed_original)?;
            if !first.strong_box
                || first.slot == intent.slot
                || refused.version != 1
                || refused.intent_sha256 != iroha_crypto::sha256(&first_original)
                || first_consumed.version != 1
                || first_consumed.intent_sha256 != iroha_crypto::sha256(&first_original)
            {
                return Err(Error::Changed);
            }
            directory.require_bytes("strongbox-refused.norito", &refused_original)?;
            directory.require_bytes(&consumed_name(0), &first_consumed_original)?;
        }
        let Some(consumed_original) = directory.read(&consumed_name(attempt))? else {
            // A crash between durable intent and consumption is recover-only too.
            continue;
        };
        let consumed: Consumed = decode(&consumed_original)?;
        if consumed.version != 1 || consumed.intent_sha256 != iroha_crypto::sha256(&original) {
            return Err(Error::Changed);
        }
        let data = match platform.probe(&intent.slot) {
            Probe::Present(bytes) => KeyData {
                slot: intent.slot,
                public_key: point(bytes)?,
            },
            Probe::Unavailable => continue,
        };
        if let Some(selected) = &selected {
            if selected.intent_sha256 != iroha_crypto::sha256(&original) {
                continue;
            }
            if selected.public_key != data.public_key {
                return Err(Error::Changed);
            }
        }
        directory.require_bytes(&intent_name(attempt), &original)?;
        directory.require_bytes(&consumed_name(attempt), &consumed_original)?;
        if present.replace(data).is_some() {
            return Err(Error::Changed);
        }
        require_platform(platform, transcript)?;
    }
    require_platform(platform, transcript)?;
    directory.recheck()?;
    if let Some(selected_original) = selected_original {
        directory.require_bytes("selected.norito", &selected_original)?;
    } else if directory.read("selected.norito")?.is_some() {
        return Err(Error::Changed);
    }
    present.ok_or(Error::Unavailable)
}

fn io(_: std::io::Error) -> Error {
    Error::Unavailable
}
fn c_name(name: &str) -> Result<CString> {
    CString::new(name).map_err(|_| Error::Invalid)
}
fn stat(file: &File) -> Result<libc::stat> {
    let mut value = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: fstat initializes the supplied stat only on success; file owns a live fd.
    if unsafe { libc::fstat(file.as_raw_fd(), value.as_mut_ptr()) } != 0 {
        return Err(Error::Unavailable);
    }
    // SAFETY: successful fstat initialized the complete stat.
    Ok(unsafe { value.assume_init() })
}
fn same(left: &libc::stat, right: &libc::stat) -> bool {
    left.st_dev == right.st_dev && left.st_ino == right.st_ino
}
fn owned(file: &File, directory: bool) -> Result<()> {
    let value = stat(file)?;
    // SAFETY: geteuid has no preconditions.
    let owner = unsafe { libc::geteuid() };
    let expected_kind = if directory {
        libc::S_IFDIR
    } else {
        libc::S_IFREG
    };
    let expected_mode = if directory { 0o700 } else { 0o600 };
    // Android ARM32 stat.st_mode and libc's mode constants have different widths.
    // Widen both without truncating any type or permission bits on any target.
    let mode = u64::from(value.st_mode);
    if owner == 0
        || value.st_uid != owner
        || mode & u64::from(libc::S_IFMT) != u64::from(expected_kind)
        || mode & 0o777 != expected_mode
        || (!directory && value.st_nlink != 1)
    {
        return Err(Error::Invalid);
    }
    Ok(())
}
fn open_at(parent: RawFd, name: &str, flags: i32, mode: libc::mode_t) -> Result<File> {
    let name = c_name(name)?;
    // SAFETY: name is NUL-terminated, parent is live/AT_FDCWD, and mode matches O_CREAT.
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            mode as libc::c_uint,
        )
    };
    if fd < 0 {
        return Err(Error::Unavailable);
    }
    // SAFETY: this fresh openat fd has no other Rust owner.
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn linked(parent: &File, name: &str, file: &File) -> Result<()> {
    let name = c_name(name)?;
    let mut value = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: parent is live and fstatat writes initialized output on success.
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            name.as_ptr(),
            value.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(Error::Changed);
    }
    // SAFETY: successful fstatat initialized output.
    if !same(&unsafe { value.assume_init() }, &stat(file)?) {
        return Err(Error::Changed);
    }
    Ok(())
}

fn open_absolute_directory(path: &Path) -> Result<File> {
    if !path.is_absolute() {
        return Err(Error::Invalid);
    }
    let mut file = open_at(libc::AT_FDCWD, "/", libc::O_RDONLY | libc::O_DIRECTORY, 0)?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                file = open_at(
                    file.as_raw_fd(),
                    name.to_str().ok_or(Error::Invalid)?,
                    libc::O_RDONLY | libc::O_DIRECTORY,
                    0,
                )?;
            }
            _ => return Err(Error::Invalid),
        }
    }
    Ok(file)
}

/// All traversal is descriptor-relative and no-follow; retained descriptors are rechecked.
struct RootLease {
    parent_path: PathBuf,
    parent: File,
    root: File,
    lock: File,
}
impl RootLease {
    fn open(parent_path: &Path, create: bool) -> Result<Self> {
        let parent = open_absolute_directory(parent_path)?;
        owned(&parent, true)?;
        let mut created = false;
        if create {
            let name = c_name(ROOT)?;
            // SAFETY: parent owns a live directory fd and name is NUL-terminated.
            if unsafe { libc::mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } == 0 {
                created = true;
            } else if std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists {
                return Err(Error::Unavailable);
            }
            parent.sync_all().map_err(io)?;
        }
        let root = open_at(
            parent.as_raw_fd(),
            ROOT,
            libc::O_RDONLY | libc::O_DIRECTORY,
            0,
        )?;
        owned(&root, true)?;
        let lock = open_at(
            root.as_raw_fd(),
            "owner.lock",
            libc::O_RDWR
                | if created {
                    libc::O_CREAT | libc::O_EXCL
                } else {
                    0
                },
            0o600,
        )?;
        owned(&lock, false)?;
        lock.sync_all().map_err(io)?;
        root.sync_all().map_err(io)?;
        // SAFETY: lock owns a live regular-file fd; flock remains held until its Drop.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(Error::Busy);
        }
        let lease = Self {
            parent_path: parent_path.to_owned(),
            parent,
            root,
            lock,
        };
        lease.recheck()?;
        Ok(lease)
    }
    fn recheck(&self) -> Result<()> {
        owned(&self.parent, true)?;
        owned(&self.root, true)?;
        owned(&self.lock, false)?;
        let current = open_absolute_directory(&self.parent_path)?;
        if !same(&stat(&current)?, &stat(&self.parent)?) {
            return Err(Error::Changed);
        }
        linked(&self.parent, ROOT, &self.root)?;
        linked(&self.root, "owner.lock", &self.lock)
    }
    fn read_record(&self, name: &str) -> Result<Option<Vec<u8>>> {
        self.recheck()?;
        let value = read_private_file(&self.root, name)?;
        self.recheck()?;
        Ok(value)
    }
    fn require_record(&self, name: &str, bytes: &[u8]) -> Result<()> {
        match self.read_record(name)? {
            Some(original) if original == bytes => Ok(()),
            _ => Err(Error::Changed),
        }
    }
    fn write_record(&self, name: &str, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() || bytes.len() > RECORD_MAX {
            return Err(Error::Invalid);
        }
        self.recheck()?;
        let mut file = open_at(
            self.root.as_raw_fd(),
            name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            0o600,
        )?;
        owned(&file, false)?;
        file.write_all(bytes).map_err(io)?;
        file.sync_all().map_err(io)?;
        self.root.sync_all().map_err(io)?;
        linked(&self.root, name, &file)?;
        self.require_record(name, bytes)?;
        self.recheck()
    }
    fn operation(
        &self,
        operation_id: &[u8; 32],
        transcript: &[u8],
        create: bool,
    ) -> Result<(Directory<'_>, bool)> {
        self.recheck()?;
        if operation(transcript)? != *operation_id {
            return Err(Error::Invalid);
        }
        let name = hex::encode(operation_id);
        let membership_name = format!("operation-{name}.norito");
        let expected = encode(&OperationMembership {
            version: 1,
            transcript: transcript.to_vec(),
        })?;
        let membership = self.read_record(&membership_name)?;
        let created = match membership {
            Some(original) => {
                let value: OperationMembership = decode(&original)?;
                if value.version != 1 || value.transcript != transcript || original != expected {
                    return Err(Error::Changed);
                }
                false
            }
            None if !create => return Err(Error::Unavailable),
            None => {
                // A directory without its mandatory original membership is damaged or
                // unsupported history. Never repair it into a new generation authority.
                let name_c = c_name(&name)?;
                let mut existing = std::mem::MaybeUninit::<libc::stat>::uninit();
                // SAFETY: retained root fd, valid name and initialized output only on success.
                if unsafe {
                    libc::fstatat(
                        self.root.as_raw_fd(),
                        name_c.as_ptr(),
                        existing.as_mut_ptr(),
                        libc::AT_SYMLINK_NOFOLLOW,
                    )
                } == 0
                {
                    return Err(Error::Changed);
                }
                if std::io::Error::last_os_error().kind() != std::io::ErrorKind::NotFound {
                    return Err(Error::Unavailable);
                }
                // Permanent membership is durable BEFORE child creation or any fresh grant.
                // A crash or failed mkdir after this step can only restore/refuse later.
                self.write_record(&membership_name, &expected)?;
                true
            }
        };
        if created {
            let name_c = c_name(&name)?;
            // SAFETY: retained root directory fd and valid fixed hexadecimal child name.
            if unsafe { libc::mkdirat(self.root.as_raw_fd(), name_c.as_ptr(), 0o700) } != 0 {
                return Err(Error::Unavailable);
            }
            self.root.sync_all().map_err(io)?;
        }
        let file = open_at(
            self.root.as_raw_fd(),
            &name,
            libc::O_RDONLY | libc::O_DIRECTORY,
            0,
        )?;
        owned(&file, true)?;
        let directory = Directory {
            root: self,
            name,
            file,
            membership_name,
            membership: expected,
        };
        directory.recheck()?;
        Ok((directory, created))
    }
}

struct Directory<'a> {
    root: &'a RootLease,
    name: String,
    file: File,
    membership_name: String,
    membership: Vec<u8>,
}
impl Directory<'_> {
    fn recheck(&self) -> Result<()> {
        self.root.recheck()?;
        owned(&self.file, true)?;
        linked(&self.root.root, &self.name, &self.file)?;
        self.root
            .require_record(&self.membership_name, &self.membership)
    }
    fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        self.recheck()?;
        let value = read_private_file(&self.file, name)?;
        self.recheck()?;
        Ok(value)
    }
    fn require_bytes(&self, name: &str, expected: &[u8]) -> Result<()> {
        match self.read(name)? {
            Some(bytes) if bytes == expected => Ok(()),
            _ => Err(Error::Changed),
        }
    }
    fn write_new(&self, name: &str, bytes: &[u8]) -> Result<()> {
        if bytes.is_empty() || bytes.len() > RECORD_MAX {
            return Err(Error::Invalid);
        }
        self.recheck()?;
        let mut file = open_at(
            self.file.as_raw_fd(),
            name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            0o600,
        )?;
        owned(&file, false)?;
        file.write_all(bytes).map_err(io)?;
        file.sync_all().map_err(io)?;
        self.file.sync_all().map_err(io)?;
        linked(&self.file, name, &file)?;
        self.require_bytes(name, bytes)?;
        self.recheck()
    }
}

fn read_private_file(parent: &File, name: &str) -> Result<Option<Vec<u8>>> {
    let name_c = c_name(name)?;
    // SAFETY: retained directory fd, NUL-terminated static record name and no-follow flags.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name_c.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return if std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
            Ok(None)
        } else {
            Err(Error::Unavailable)
        };
    }
    // SAFETY: freshly opened fd has exactly this owner.
    let mut file = unsafe { File::from_raw_fd(fd) };
    owned(&file, false)?;
    if stat(&file)?.st_size < 0 || stat(&file)?.st_size as usize > RECORD_MAX {
        return Err(Error::Invalid);
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take((RECORD_MAX + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes.len() > RECORD_MAX {
        return Err(Error::Invalid);
    }
    linked(parent, name, &file)?;
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        collections::BTreeMap,
        os::unix::fs::{PermissionsExt, symlink},
    };

    struct Fake {
        root: tempfile::TempDir,
        transcript: Vec<u8>,
        api: u32,
        keys: RefCell<BTreeMap<[u8; 32], Vec<u8>>>,
        generated: RefCell<Vec<([u8; 32], bool)>>,
        refuse_sb: bool,
        fail: Cell<bool>,
        retire: Cell<bool>,
    }
    fn transcript() -> Vec<u8> {
        let mut bytes = DOMAIN.to_vec();
        for value in 1..=7 {
            bytes.extend([value; 32]);
        }
        bytes.extend(100_u64.to_le_bytes());
        bytes.extend(1000_u64.to_le_bytes());
        assert_eq!(bytes.len(), 276);
        bytes
    }
    fn fake(api: u32) -> Fake {
        let root = tempfile::tempdir_in(
            std::env::temp_dir()
                .canonicalize()
                .expect("canonical temp parent"),
        )
        .expect("root");
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
            .expect("mode");
        Fake {
            root,
            transcript: transcript(),
            api,
            keys: RefCell::new(BTreeMap::new()),
            generated: RefCell::new(vec![]),
            refuse_sb: false,
            fail: Cell::new(false),
            retire: Cell::new(false),
        }
    }
    fn public(slot: &[u8; 32]) -> Vec<u8> {
        let mut value = vec![4];
        value.extend(slot);
        value.extend([9; 32]);
        value
    }
    impl Platform for Fake {
        fn require_original(&self) -> Result<()> {
            if self.retire.get() {
                Err(Error::Changed)
            } else {
                Ok(())
            }
        }
        fn transcript(&self) -> Result<Vec<u8>> {
            Ok(self.transcript.clone())
        }
        fn no_backup_root(&self) -> Result<PathBuf> {
            Ok(self.root.path().to_owned())
        }
        fn api_level(&self) -> Result<u32> {
            Ok(self.api)
        }
        fn probe(&self, slot: &[u8; 32]) -> Probe {
            self.keys
                .borrow()
                .get(slot)
                .cloned()
                .map_or(Probe::Unavailable, Probe::Present)
        }
        fn generate(&self, slot: &[u8; 32], digest: &[u8; 32], strong_box: bool) -> Generated {
            assert_eq!(*digest, iroha_crypto::sha256(&self.transcript));
            let operation = operation(&self.transcript).expect("operation");
            let path = self.root.path().join(ROOT).join(hex::encode(operation));
            let attempt = self.generated.borrow().len();
            let consumed: Consumed = decode(
                &std::fs::read(path.join(consumed_name(attempt as u8)))
                    .expect("consumed before callback"),
            )
            .expect("decode");
            assert_eq!(consumed.version, 1);
            self.generated.borrow_mut().push((*slot, strong_box));
            if strong_box && self.refuse_sb {
                return Generated::StrongBoxUnavailable;
            }
            if self.fail.get() {
                return Generated::Unavailable;
            }
            let bytes = public(slot);
            self.keys.borrow_mut().insert(*slot, bytes.clone());
            Generated::Present(bytes)
        }
    }
    #[test]
    fn new_original_consumes_durably_before_upcall() {
        let p = fake(26);
        let data = reserve_original(&p, &p.transcript).unwrap();
        assert_eq!(p.generated.borrow().len(), 1);
        assert_eq!(data.public_key, public(&data.slot));
    }
    #[test]
    fn repeated_reserve_is_recovery_only() {
        let p = fake(31);
        let first = reserve_original(&p, &p.transcript).unwrap();
        let second = reserve_original(&p, &p.transcript).unwrap();
        assert_eq!(first.slot, second.slot);
        assert_eq!(p.generated.borrow().len(), 1);
    }
    #[test]
    fn explicit_restore_does_not_generate() {
        let p = fake(26);
        let first = reserve_original(&p, &p.transcript).unwrap();
        assert_eq!(
            restore_original(&p, &p.transcript).unwrap().slot,
            first.slot
        );
        assert_eq!(p.generated.borrow().len(), 1);
    }
    #[test]
    fn restore_absent_creates_nothing() {
        let p = fake(26);
        assert!(restore_original(&p, &p.transcript).is_err());
        assert!(!p.root.path().join(ROOT).exists());
        assert!(p.generated.borrow().is_empty());
    }
    #[test]
    fn failed_generation_never_retries_same_alias() {
        let p = fake(26);
        p.fail.set(true);
        assert!(reserve_original(&p, &p.transcript).is_err());
        p.fail.set(false);
        assert!(reserve_original(&p, &p.transcript).is_err());
        assert!(restore_original(&p, &p.transcript).is_err());
        assert_eq!(p.generated.borrow().len(), 1);
    }
    #[test]
    fn typed_strongbox_refusal_uses_different_fresh_slot() {
        let mut p = fake(28);
        p.refuse_sb = true;
        let data = reserve_original(&p, &p.transcript).unwrap();
        let generated = p.generated.borrow();
        assert_eq!(generated.len(), 2);
        assert!(generated[0].1);
        assert!(!generated[1].1);
        assert_ne!(generated[0].0, generated[1].0);
        assert_eq!(data.slot, generated[1].0);
    }
    #[test]
    fn generic_failure_does_not_fall_back() {
        let p = fake(28);
        p.fail.set(true);
        assert!(reserve_original(&p, &p.transcript).is_err());
        assert_eq!(p.generated.borrow().len(), 1);
    }
    #[test]
    fn original_retirement_refuses_before_generation() {
        let p = fake(28);
        p.retire.set(true);
        assert!(reserve_original(&p, &p.transcript).is_err());
        assert!(p.generated.borrow().is_empty());
    }
    #[test]
    fn full_transcript_change_refuses_existing_operation() {
        let mut p = fake(26);
        reserve_original(&p, &p.transcript).unwrap();
        p.transcript[DOMAIN.len()] ^= 1;
        assert!(reserve_original(&p, &p.transcript).is_err());
        assert_eq!(p.generated.borrow().len(), 1);
    }
    #[test]
    fn held_exclusive_lease_blocks_other_opener() {
        let p = fake(26);
        let _lease = RootLease::open(p.root.path(), true).unwrap();
        assert_eq!(
            RootLease::open(p.root.path(), true).err(),
            Some(Error::Busy)
        );
    }
    #[test]
    fn missing_consumption_after_crash_is_not_fresh() {
        let p = fake(26);
        {
            let root = RootLease::open(p.root.path(), true).unwrap();
            let (dir, created) = root
                .operation(&operation(&p.transcript).unwrap(), &p.transcript, true)
                .unwrap();
            assert!(created);
            let _unconsumed = new_grant(&dir, &p.transcript, 0, false).unwrap();
        }
        assert!(reserve_original(&p, &p.transcript).is_err());
        assert!(p.generated.borrow().is_empty());
    }
    #[test]
    fn malformed_retained_intent_refuses_without_repair() {
        let p = fake(26);
        reserve_original(&p, &p.transcript).unwrap();
        let path = p
            .root
            .path()
            .join(ROOT)
            .join(hex::encode(operation(&p.transcript).unwrap()))
            .join(intent_name(0));
        std::fs::write(path, b"invalid").unwrap();
        assert!(restore_original(&p, &p.transcript).is_err());
        assert_eq!(p.generated.borrow().len(), 1);
    }
    #[test]
    fn owned_files_keep_exact_kind_permissions_and_single_link_checks() {
        let root = tempfile::tempdir().unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let directory = File::open(root.path()).unwrap();
        assert_eq!(owned(&directory, true), Ok(()));
        assert_eq!(owned(&directory, false), Err(Error::Invalid));

        let path = root.path().join("private.norito");
        std::fs::write(&path, b"retained").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let file = File::open(&path).unwrap();
        assert_eq!(owned(&file, false), Ok(()));
        assert_eq!(owned(&file, true), Err(Error::Invalid));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        assert_eq!(owned(&file, false), Err(Error::Invalid));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

        let alias = root.path().join("alias.norito");
        std::fs::hard_link(&path, &alias).unwrap();
        assert_eq!(owned(&file, false), Err(Error::Invalid));
        std::fs::remove_file(&alias).unwrap();
        assert_eq!(owned(&file, false), Ok(()));
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o770)).unwrap();
        assert_eq!(owned(&directory, true), Err(Error::Invalid));
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[test]
    fn symlink_root_refuses() {
        let p = fake(26);
        symlink(p.root.path(), p.root.path().join(ROOT)).unwrap();
        assert!(reserve_original(&p, &p.transcript).is_err());
        assert!(p.generated.borrow().is_empty());
    }
    #[test]
    fn replaced_lock_is_detected() {
        {
            let p = fake(26);
            let lease = RootLease::open(p.root.path(), true).unwrap();
            let lock_path = p.root.path().join(ROOT).join("owner.lock");
            // Retain the held inode's single link so recheck reaches path identity.
            std::fs::rename(&lock_path, lock_path.with_file_name("retained-owner.lock")).unwrap();
            std::fs::write(&lock_path, b"").unwrap();
            std::fs::set_permissions(&lock_path, std::fs::Permissions::from_mode(0o600)).unwrap();
            assert_eq!(owned(&lease.lock, false), Ok(()));
            assert_eq!(lease.recheck().err(), Some(Error::Changed));
        }
        {
            let p = fake(26);
            let lease = RootLease::open(p.root.path(), true).unwrap();
            let lock_path = p.root.path().join(ROOT).join("owner.lock");
            // Unlinking instead leaves the held inode with zero links; refuse it first.
            std::fs::remove_file(&lock_path).unwrap();
            std::fs::write(&lock_path, b"").unwrap();
            std::fs::set_permissions(&lock_path, std::fs::Permissions::from_mode(0o600)).unwrap();
            assert_eq!(owned(&lease.lock, false), Err(Error::Invalid));
            assert_eq!(lease.recheck().err(), Some(Error::Invalid));
        }
    }
    #[test]
    fn consumed_grant_cannot_be_consumed_twice() {
        let p = fake(26);
        let root = RootLease::open(p.root.path(), true).unwrap();
        let (dir, _) = root
            .operation(&operation(&p.transcript).unwrap(), &p.transcript, true)
            .unwrap();
        let mut grant = new_grant(&dir, &p.transcript, 0, false).unwrap();
        grant.consume(&dir).unwrap();
        assert_eq!(grant.consume(&dir), Err(Error::ExistingIntent));
    }
    #[test]
    fn corrupted_consumption_refuses_without_generation() {
        let p = fake(26);
        reserve_original(&p, &p.transcript).unwrap();
        let path = p
            .root
            .path()
            .join(ROOT)
            .join(hex::encode(operation(&p.transcript).unwrap()))
            .join(consumed_name(0));
        std::fs::write(path, b"invalid").unwrap();
        assert!(restore_original(&p, &p.transcript).is_err());
        assert_eq!(p.generated.borrow().len(), 1);
    }
    #[test]
    fn record_norito_roundtrip_and_trailing_rejection() {
        let intent = Intent {
            version: 1,
            transcript: transcript(),
            slot: [8; 32],
            attempt: 0,
            strong_box: false,
        };
        let bytes = encode(&intent).unwrap();
        assert!(decode::<Intent>(&bytes).unwrap() == intent);
        let mut bad = bytes;
        bad.push(0);
        assert!(decode::<Intent>(&bad).is_err());
    }
    #[test]
    fn malformed_transcript_lifetime_and_domain_refuse() {
        let mut bytes = transcript();
        bytes[0] ^= 1;
        assert!(operation(&bytes).is_err());
        let mut bytes = transcript();
        bytes[268..276].copy_from_slice(&700_001_u64.to_le_bytes());
        assert!(operation(&bytes).is_err());
    }

    #[test]
    fn consumed_before_upcall_crash_does_not_recreate_grant() {
        let p = fake(26);
        {
            let root = RootLease::open(p.root.path(), true).unwrap();
            let (directory, _) = root
                .operation(&operation(&p.transcript).unwrap(), &p.transcript, true)
                .unwrap();
            let mut grant = new_grant(&directory, &p.transcript, 0, false).unwrap();
            grant.consume(&directory).unwrap();
        }
        assert!(restore_original(&p, &p.transcript).is_err());
        assert!(reserve_original(&p, &p.transcript).is_err());
        assert!(p.generated.borrow().is_empty());
    }

    #[test]
    fn lost_generation_reply_recovers_actual_key_without_selection_write() {
        let p = fake(26);
        let slot = {
            let root = RootLease::open(p.root.path(), true).unwrap();
            let (directory, _) = root
                .operation(&operation(&p.transcript).unwrap(), &p.transcript, true)
                .unwrap();
            let mut grant = new_grant(&directory, &p.transcript, 0, false).unwrap();
            grant.consume(&directory).unwrap();
            p.keys
                .borrow_mut()
                .insert(grant.intent.slot, public(&grant.intent.slot));
            grant.intent.slot
        };
        let path = p
            .root
            .path()
            .join(ROOT)
            .join(hex::encode(operation(&p.transcript).unwrap()));
        let before = std::fs::read_dir(&path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(restore_original(&p, &p.transcript).unwrap().slot, slot);
        let after = std::fs::read_dir(&path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(before, after);
        assert!(!path.join("selected.norito").exists());
        assert!(p.generated.borrow().is_empty());
    }

    #[test]
    fn deleted_owner_lock_is_not_repaired_by_reserve() {
        let p = fake(26);
        reserve_original(&p, &p.transcript).unwrap();
        let lock = p.root.path().join(ROOT).join("owner.lock");
        std::fs::remove_file(&lock).unwrap();
        assert!(reserve_original(&p, &p.transcript).is_err());
        assert!(!lock.exists());
        assert_eq!(p.generated.borrow().len(), 1);
    }
    #[test]
    fn erased_operation_child_is_not_recreated_from_membership() {
        let p = fake(26);
        reserve_original(&p, &p.transcript).unwrap();
        let operation = operation(&p.transcript).unwrap();
        let name = hex::encode(operation);
        let root = p.root.path().join(ROOT);
        let membership = root.join(format!("operation-{name}.norito"));
        let original = std::fs::read(&membership).unwrap();
        let child = root.join(&name);
        std::fs::remove_dir_all(&child).unwrap();
        assert!(reserve_original(&p, &p.transcript).is_err());
        assert!(restore_original(&p, &p.transcript).is_err());
        assert!(!child.exists());
        assert_eq!(std::fs::read(membership).unwrap(), original);
        assert_eq!(p.generated.borrow().len(), 1);
    }

    #[test]
    fn crash_after_membership_before_child_is_recover_only() {
        let p = fake(26);
        let operation = operation(&p.transcript).unwrap();
        let name = hex::encode(operation);
        let original = encode(&OperationMembership {
            version: 1,
            transcript: p.transcript.clone(),
        })
        .unwrap();
        {
            let root = RootLease::open(p.root.path(), true).unwrap();
            root.write_record(&format!("operation-{name}.norito"), &original)
                .unwrap();
        }
        assert!(reserve_original(&p, &p.transcript).is_err());
        assert!(restore_original(&p, &p.transcript).is_err());
        assert!(!p.root.path().join(ROOT).join(&name).exists());
        assert_eq!(
            std::fs::read(
                p.root
                    .path()
                    .join(ROOT)
                    .join(format!("operation-{name}.norito"))
            )
            .unwrap(),
            original
        );
        assert!(p.generated.borrow().is_empty());
    }

    #[test]
    fn erased_membership_with_retained_child_is_not_repaired() {
        let p = fake(26);
        reserve_original(&p, &p.transcript).unwrap();
        let name = hex::encode(operation(&p.transcript).unwrap());
        let root = p.root.path().join(ROOT);
        let membership = root.join(format!("operation-{name}.norito"));
        std::fs::remove_file(&membership).unwrap();
        assert!(reserve_original(&p, &p.transcript).is_err());
        assert!(restore_original(&p, &p.transcript).is_err());
        assert!(!membership.exists());
        assert!(root.join(&name).exists());
        assert_eq!(p.generated.borrow().len(), 1);
    }

    #[test]
    fn complete_native_root_loss_restore_creates_nothing() {
        let p = fake(26);
        reserve_original(&p, &p.transcript).unwrap();
        let root = p.root.path().join(ROOT);
        std::fs::remove_dir_all(&root).unwrap();
        // The app's acknowledged one-shot dispatch routes every later call here.
        // Native cannot detect erasure of all independent software histories.
        assert!(restore_original(&p, &p.transcript).is_err());
        assert!(!root.exists());
        assert_eq!(p.generated.borrow().len(), 1);
    }

    #[test]
    fn held_operation_detects_membership_loss_before_grant() {
        let p = fake(26);
        let root = RootLease::open(p.root.path(), true).unwrap();
        let (directory, _) = root
            .operation(&operation(&p.transcript).unwrap(), &p.transcript, true)
            .unwrap();
        std::fs::remove_file(p.root.path().join(ROOT).join(&directory.membership_name)).unwrap();
        assert_eq!(directory.recheck(), Err(Error::Changed));
        assert!(new_grant(&directory, &p.transcript, 0, false).is_err());
        assert!(
            !p.root
                .path()
                .join(ROOT)
                .join(&directory.name)
                .join(intent_name(0))
                .exists()
        );
        assert!(p.generated.borrow().is_empty());
    }

    #[test]
    fn altered_membership_transcript_is_not_repaired() {
        let p = fake(26);
        reserve_original(&p, &p.transcript).unwrap();
        let membership = p.root.path().join(ROOT).join(format!(
            "operation-{}.norito",
            hex::encode(operation(&p.transcript).unwrap())
        ));
        let mut altered = p.transcript.clone();
        altered[DOMAIN.len()] ^= 1;
        let altered_record = encode(&OperationMembership {
            version: 1,
            transcript: altered,
        })
        .unwrap();
        std::fs::write(&membership, &altered_record).unwrap();
        assert!(reserve_original(&p, &p.transcript).is_err());
        assert!(restore_original(&p, &p.transcript).is_err());
        assert_eq!(std::fs::read(membership).unwrap(), altered_record);
        assert_eq!(p.generated.borrow().len(), 1);
    }

    #[test]
    fn partial_membership_record_cannot_authorize_child_creation() {
        let p = fake(26);
        let name = hex::encode(operation(&p.transcript).unwrap());
        {
            let root = RootLease::open(p.root.path(), true).unwrap();
            root.write_record(&format!("operation-{name}.norito"), b"partial")
                .unwrap();
        }
        assert!(reserve_original(&p, &p.transcript).is_err());
        assert!(!p.root.path().join(ROOT).join(&name).exists());
        assert!(p.generated.borrow().is_empty());
    }
}
