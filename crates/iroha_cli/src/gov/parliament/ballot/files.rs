//! Owner-only local files backing the timed-OVN ballot commands.
//!
//! A juror keeps three kinds of file between commands:
//!
//! - The **key file** holds one 32-byte root seed as a headered Norito frame of
//!   `TimedOvnKeyFileFrameV1`. Every registration and ballot secret is derived
//!   from this seed deterministically, so a restarted or repeated command
//!   rebuilds byte-identical records. The seed never appears on argv, in the
//!   environment, in output, or in `Debug` text.
//! - A **choice lock** `<key-file>.choice-<participant-hash>` records the one
//!   choice this key may ever cast for one seat (ballot attempt and account).
//!   It is created atomically, never replaced, and written before any ballot
//!   bytes exist, so concurrent or repeated `cast` runs with the same key file
//!   cannot build two ballots with different choices for one seat; two such
//!   ballots would reveal both choices once the release opens. The lock lives
//!   with the key file whatever `--state-file` names, so casting from a copy of
//!   the key file in another directory bypasses it; never copy a key file.
//! - The **state file** is canonical JSON holding the consensus trust anchor: a
//!   finality checkpoint promoted after every authenticated casting-proof page.
//!   Promotion holds an exclusive advisory lock on `<state-file>.lock`,
//!   re-reads the file and only ever advances it, so concurrent commands never
//!   regress the checkpoint.
//!
//! Every file must be a singly linked regular file owned by the current user
//! with mode `0600` or `0400`; the final path component is never followed as a
//! symlink. New files are published atomically: the key file and choice locks
//! without replacing an existing file, the state file by an atomic rename.

use std::{
    fs,
    path::{Path, PathBuf},
};

use eyre::{Result, WrapErr as _, bail, eyre};
use norito::json::{JsonDeserialize, JsonSerialize};
use zeroize::{Zeroize as _, Zeroizing};

use super::BallotChoiceArg;

/// Exact width of the timed-OVN root seed.
pub(super) const TIMED_OVN_SEED_BYTES_V1: usize = 32;
/// Key-file frame layout version.
const KEY_FILE_VERSION_V1: u16 = 1;
/// State-file layout version.
const STATE_FILE_VERSION_V1: u16 = 1;
/// Upper bound for a key-file frame (the frame is a few dozen bytes).
const MAX_KEY_FILE_BYTES: u64 = 1024;
/// Upper bound for a state file.
const MAX_STATE_FILE_BYTES: u64 = 1024 * 1024;
/// Choice-lock file layout version.
const CHOICE_LOCK_VERSION_V1: u16 = 1;
/// Upper bound for a choice-lock file.
const MAX_CHOICE_LOCK_BYTES: u64 = 1024;

/// On-disk key-file frame. The seed is wiped by its owners after use.
#[derive(
    norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_cli::gov::parliament::ballot::TimedOvnKeyFileFrameV1")]
struct TimedOvnKeyFileFrameV1 {
    version: u16,
    seed: [u8; TIMED_OVN_SEED_BYTES_V1],
}

/// One juror's timed-OVN root seed; zeroized on drop and never printed.
pub(super) struct TimedOvnSeedV1(Zeroizing<[u8; TIMED_OVN_SEED_BYTES_V1]>);

impl TimedOvnSeedV1 {
    /// Wrap existing seed bytes, rejecting the all-zero seed.
    pub(super) fn from_bytes(mut bytes: [u8; TIMED_OVN_SEED_BYTES_V1]) -> Result<Self> {
        let seed = Zeroizing::new(bytes);
        bytes.zeroize();
        if seed.iter().all(|byte| *byte == 0) {
            bail!("timed-OVN seed must not be all zero");
        }
        Ok(Self(seed))
    }

    /// Draw a fresh seed from the operating-system CSPRNG.
    pub(super) fn generate() -> Result<Self> {
        use rand::TryRngCore as _;
        let mut bytes = Zeroizing::new([0_u8; TIMED_OVN_SEED_BYTES_V1]);
        rand::rngs::OsRng
            .try_fill_bytes(&mut bytes[..])
            .map_err(|_| eyre!("operating-system randomness is unavailable"))?;
        Self::from_bytes(*bytes)
    }

    /// Borrow the raw seed bytes.
    pub(super) fn as_bytes(&self) -> &[u8; TIMED_OVN_SEED_BYTES_V1] {
        &self.0
    }

    fn to_frame(&self) -> Result<Zeroizing<Vec<u8>>> {
        let mut frame = TimedOvnKeyFileFrameV1 {
            version: KEY_FILE_VERSION_V1,
            seed: *self.0,
        };
        let encoded = norito::to_bytes(&frame).map(Zeroizing::new);
        frame.seed.zeroize();
        encoded.map_err(|_| eyre!("failed to encode the timed-OVN key file"))
    }

    fn from_frame(bytes: &[u8]) -> Result<Self> {
        let mut frame: TimedOvnKeyFileFrameV1 = norito::decode_from_bytes(bytes)
            .map_err(|_| eyre!("timed-OVN key file is not a valid key-file frame"))?;
        let version = frame.version;
        let seed = Self::from_bytes(frame.seed);
        frame.seed.zeroize();
        if version != KEY_FILE_VERSION_V1 {
            bail!("timed-OVN key file has unsupported version {version}");
        }
        seed
    }
}

impl core::fmt::Debug for TimedOvnSeedV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("TimedOvnSeedV1(<redacted>)")
    }
}

/// Load the seed from an existing owner-only key file.
pub(super) fn load_key_file(path: &Path) -> Result<TimedOvnSeedV1> {
    let bytes = read_owner_only_file(path, MAX_KEY_FILE_BYTES, "timed-OVN key file")?;
    TimedOvnSeedV1::from_frame(&bytes)
}

/// Load the seed from an owner-only key file, or report that no file exists.
///
/// Only a missing final path component counts as absent; an existing file that
/// fails the custody checks is an error.
pub(super) fn load_key_file_if_present(path: &Path) -> Result<Option<TimedOvnSeedV1>> {
    match fs::symlink_metadata(path) {
        Ok(_) => load_key_file(path).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(eyre!(error).wrap_err(format!(
            "failed to inspect timed-OVN key file `{}`",
            path.display()
        ))),
    }
}

/// Load the seed from `path`, or generate it and create the file when absent.
///
/// Returns whether the file was created by this call. An existing file is never
/// replaced, including one created concurrently by another process.
pub(super) fn load_or_create_key_file(path: &Path) -> Result<(TimedOvnSeedV1, bool)> {
    load_or_create_key_file_with(path, TimedOvnSeedV1::generate)
}

/// [`load_or_create_key_file`] with the seed source of a new file supplied by
/// the caller; `generate` runs only when no file exists.
pub(super) fn load_or_create_key_file_with(
    path: &Path,
    generate: impl FnOnce() -> Result<TimedOvnSeedV1>,
) -> Result<(TimedOvnSeedV1, bool)> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok((load_key_file(path)?, false)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let seed = generate()?;
            let frame = seed.to_frame()?;
            match publish_new_owner_only_file(path, &frame, "timed-OVN key file") {
                Ok(()) => Ok((seed, true)),
                Err(PublishError::AlreadyExists) => Ok((load_key_file(path)?, false)),
                Err(PublishError::Other(error)) => Err(error),
            }
        }
        Err(error) => Err(eyre!(error).wrap_err(format!(
            "failed to inspect timed-OVN key file `{}`",
            path.display()
        ))),
    }
}

/// `path` with `suffix` appended to its final component.
fn suffixed_path(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Default state-file path next to the key file: `<key-file>.state.json`.
pub(super) fn default_state_path(key_file: &Path) -> PathBuf {
    suffixed_path(key_file, ".state.json")
}

/// Seat-bound choice-lock path next to the key file:
/// `<key-file>.choice-<participant-hash hex>`.
///
/// The participant hash binds the ballot attempt and the account, so every
/// seat that one key file can cast has its own lock.
pub(super) fn choice_lock_path(key_file: &Path, participant_hash: &[u8; 32]) -> PathBuf {
    suffixed_path(
        key_file,
        &format!(".choice-{}", hex::encode(participant_hash)),
    )
}

/// On-disk choice lock for one seat.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ChoiceLockFileV1 {
    version: u16,
    ballot_attempt_id: String,
    participant_hash: String,
    choice: String,
}

/// Read the choice lock of one seat next to `key_file`, if any.
///
/// A lock that fails the custody checks, or that names another seat, is an
/// error rather than an absent lock.
pub(super) fn read_choice_lock(
    key_file: &Path,
    ballot_attempt_id: &[u8; 32],
    participant_hash: &[u8; 32],
) -> Result<Option<BallotChoiceArg>> {
    let path = choice_lock_path(key_file, participant_hash);
    match fs::symlink_metadata(&path) {
        Ok(_) => decode_choice_lock(&path, ballot_attempt_id, participant_hash).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(eyre!(error).wrap_err(format!(
            "failed to inspect ballot choice lock `{}`",
            path.display()
        ))),
    }
}

fn decode_choice_lock(
    path: &Path,
    ballot_attempt_id: &[u8; 32],
    participant_hash: &[u8; 32],
) -> Result<BallotChoiceArg> {
    let bytes = read_owner_only_file(path, MAX_CHOICE_LOCK_BYTES, "ballot choice lock")?;
    let lock: ChoiceLockFileV1 = norito::json::from_slice(&bytes)
        .wrap_err_with(|| format!("ballot choice lock `{}` is invalid", path.display()))?;
    if lock.version != CHOICE_LOCK_VERSION_V1 {
        bail!(
            "ballot choice lock `{}` has unsupported version {}",
            path.display(),
            lock.version
        );
    }
    if decode_lower_hex32(&lock.ballot_attempt_id, "ballot_attempt_id")? != *ballot_attempt_id
        || decode_lower_hex32(&lock.participant_hash, "participant_hash")? != *participant_hash
    {
        bail!(
            "ballot choice lock `{}` belongs to another seat",
            path.display()
        );
    }
    BallotChoiceArg::from_label(&lock.choice).ok_or_else(|| {
        eyre!(
            "ballot choice lock `{}` has an unknown choice",
            path.display()
        )
    })
}

fn refuse_other_choice(locked: BallotChoiceArg, choice: BallotChoiceArg) -> Result<()> {
    if locked == choice {
        return Ok(());
    }
    bail!(
        "a `{}` ballot was already built for this seat with this key file; building a `{}` \
         ballot for the same seat would reveal both choices, so it is refused",
        locked.label(),
        choice.label()
    )
}

/// Refuse `choice` early when the seat already holds a different lock.
///
/// Only [`lock_choice`] is authoritative; this check spares the network
/// round trips of a command that would be refused anyway.
pub(super) fn check_choice_lock(
    key_file: &Path,
    ballot_attempt_id: &[u8; 32],
    participant_hash: &[u8; 32],
    choice: BallotChoiceArg,
) -> Result<()> {
    match read_choice_lock(key_file, ballot_attempt_id, participant_hash)? {
        Some(locked) => refuse_other_choice(locked, choice),
        None => Ok(()),
    }
}

/// Atomically lock `choice` for one seat, refusing a different choice.
///
/// The lock is published without replacing an existing file, so of several
/// concurrent callers exactly one creates it and every other caller either
/// confirms the same choice or is refused. A caller that races the publication
/// itself may fail closed and simply reruns.
pub(super) fn lock_choice(
    key_file: &Path,
    ballot_attempt_id: &[u8; 32],
    participant_hash: &[u8; 32],
    choice: BallotChoiceArg,
) -> Result<()> {
    if let Some(locked) = read_choice_lock(key_file, ballot_attempt_id, participant_hash)? {
        return refuse_other_choice(locked, choice);
    }
    let path = choice_lock_path(key_file, participant_hash);
    let document = ChoiceLockFileV1 {
        version: CHOICE_LOCK_VERSION_V1,
        ballot_attempt_id: hex::encode(ballot_attempt_id),
        participant_hash: hex::encode(participant_hash),
        choice: choice.label().to_owned(),
    };
    let mut bytes =
        norito::json::to_vec(&document).wrap_err("failed to encode the ballot choice lock")?;
    bytes.push(b'\n');
    match publish_new_owner_only_file(&path, &bytes, "ballot choice lock") {
        Ok(()) => Ok(()),
        Err(PublishError::AlreadyExists) => refuse_other_choice(
            decode_choice_lock(&path, ballot_attempt_id, participant_hash)?,
            choice,
        ),
        Err(PublishError::Other(error)) => Err(error),
    }
}

/// Externally trusted finality checkpoint that begins every casting proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TrustedCheckpoint {
    /// Finalized height of the checkpoint.
    pub height: u64,
    /// Sumeragi v2 height-context id at that height.
    pub context_id: [u8; 32],
}

impl TrustedCheckpoint {
    /// Validate the structural shape of a checkpoint.
    pub(super) fn validate(self) -> Result<Self> {
        if self.height == 0 {
            bail!("trusted checkpoint height must be non-zero");
        }
        if !is_canonical_hash(&self.context_id) {
            bail!("trusted checkpoint context id is not a canonical Iroha hash");
        }
        Ok(self)
    }
}

/// On-disk state-file document.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct BallotStateFileV1 {
    version: u16,
    network_id: String,
    checkpoint_height: u64,
    checkpoint_context_id: String,
}

/// Loaded, validated ballot state bound to one network and one file.
#[derive(Debug)]
pub(super) struct BallotState {
    path: PathBuf,
    network_id: [u8; 32],
    checkpoint: TrustedCheckpoint,
}

impl BallotState {
    /// Open the state file, or create it from `init` when it does not exist.
    ///
    /// `init` is accepted only for a new file: an existing file keeps its own,
    /// possibly promoted, checkpoint and must belong to `network_id`.
    pub(super) fn open(
        path: &Path,
        network_id: [u8; 32],
        init: Option<TrustedCheckpoint>,
    ) -> Result<Self> {
        match fs::symlink_metadata(path) {
            Ok(_) => {
                if init.is_some() {
                    bail!(
                        "ballot state file `{}` already pins a trusted checkpoint; the \
                         --trusted-checkpoint-* flags only initialize a new state file",
                        path.display()
                    );
                }
                Self::load_for_network(path, network_id)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let checkpoint = init
                    .ok_or_else(|| {
                        eyre!(
                            "ballot state file `{}` does not exist; initialize it with \
                             --trusted-checkpoint-height and --trusted-checkpoint-context-id \
                             taken from an independent source (see `iroha gov parliament \
                             ballot anchor`)",
                            path.display()
                        )
                    })?
                    .validate()?;
                let bytes = encode_state(network_id, checkpoint)?;
                match publish_new_owner_only_file(path, &bytes, "ballot state file") {
                    Ok(()) => Ok(Self {
                        path: path.to_path_buf(),
                        network_id,
                        checkpoint,
                    }),
                    Err(PublishError::AlreadyExists) => bail!(
                        "ballot state file `{}` was created concurrently; rerun without the \
                         --trusted-checkpoint-* flags",
                        path.display()
                    ),
                    Err(PublishError::Other(error)) => Err(error),
                }
            }
            Err(error) => Err(eyre!(error).wrap_err(format!(
                "failed to inspect ballot state file `{}`",
                path.display()
            ))),
        }
    }

    /// Load an existing state file of `network_id` read-only, or report that
    /// none exists.
    pub(super) fn load_if_present(path: &Path, network_id: [u8; 32]) -> Result<Option<Self>> {
        match fs::symlink_metadata(path) {
            Ok(_) => Self::load_for_network(path, network_id).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(eyre!(error).wrap_err(format!(
                "failed to inspect ballot state file `{}`",
                path.display()
            ))),
        }
    }

    /// Load an existing state file and require it to belong to `network_id`.
    fn load_for_network(path: &Path, network_id: [u8; 32]) -> Result<Self> {
        let state = Self::load(path)?;
        if state.network_id != network_id {
            bail!(
                "ballot state file `{}` belongs to a different network",
                path.display()
            );
        }
        Ok(state)
    }

    /// Load and validate an existing state file.
    pub(super) fn load(path: &Path) -> Result<Self> {
        let bytes = read_owner_only_file(path, MAX_STATE_FILE_BYTES, "ballot state file")?;
        let document: BallotStateFileV1 = norito::json::from_slice(&bytes)
            .wrap_err_with(|| format!("ballot state file `{}` is invalid", path.display()))?;
        if document.version != STATE_FILE_VERSION_V1 {
            bail!(
                "ballot state file `{}` has unsupported version {}",
                path.display(),
                document.version
            );
        }
        let network_id = decode_lower_hex32(&document.network_id, "network_id")?;
        let checkpoint = TrustedCheckpoint {
            height: document.checkpoint_height,
            context_id: decode_lower_hex32(
                &document.checkpoint_context_id,
                "checkpoint_context_id",
            )?,
        }
        .validate()?;
        Ok(Self {
            path: path.to_path_buf(),
            network_id,
            checkpoint,
        })
    }

    /// Current trusted checkpoint of this command.
    pub(super) fn checkpoint(&self) -> TrustedCheckpoint {
        self.checkpoint
    }

    /// Durably promote the trusted checkpoint to an authenticated later height.
    ///
    /// Under the exclusive state-file lock the file is re-read, so a checkpoint
    /// promoted meanwhile by another command is kept when it is at least as
    /// high: the file only ever advances. A different checkpoint at the same
    /// height is a fork (or a tampered file) and is refused.
    pub(super) fn promote(&mut self, checkpoint: TrustedCheckpoint) -> Result<()> {
        let checkpoint = checkpoint.validate()?;
        if checkpoint == self.checkpoint {
            return Ok(());
        }
        if checkpoint.height <= self.checkpoint.height {
            bail!("trusted checkpoint promotion must advance the finalized height");
        }
        let _lock = StateFileLock::acquire(&self.path)?;
        let stored = Self::load(&self.path)?;
        if stored.network_id != self.network_id {
            bail!(
                "ballot state file `{}` changed network while in use",
                self.path.display()
            );
        }
        let stored = stored.checkpoint;
        if stored.height == checkpoint.height && stored.context_id != checkpoint.context_id {
            bail!(
                "ballot state file `{}` holds a different checkpoint at height {}; the \
                 authenticated chains disagree",
                self.path.display(),
                checkpoint.height
            );
        }
        if stored.height < checkpoint.height {
            let bytes = encode_state(self.network_id, checkpoint)?;
            replace_owner_only_file(&self.path, &bytes, "ballot state file")?;
        }
        self.checkpoint = checkpoint;
        Ok(())
    }
}

/// Canonical JSON bytes of one state-file document.
fn encode_state(network_id: [u8; 32], checkpoint: TrustedCheckpoint) -> Result<Vec<u8>> {
    let document = BallotStateFileV1 {
        version: STATE_FILE_VERSION_V1,
        network_id: hex::encode(network_id),
        checkpoint_height: checkpoint.height,
        checkpoint_context_id: hex::encode(checkpoint.context_id),
    };
    let mut bytes =
        norito::json::to_vec(&document).wrap_err("failed to encode the ballot state file")?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Exclusive advisory lock on `<state-file>.lock`, held while one command
/// re-reads and rewrites the state file; dropping it releases the lock.
#[cfg(unix)]
struct StateFileLock {
    _file: fs::File,
}

#[cfg(unix)]
impl StateFileLock {
    fn acquire(state_path: &Path) -> Result<Self> {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
        let path = suffixed_path(state_path, ".lock");
        let descriptor = rustix::fs::open(
            &path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from_raw_mode(0o600),
        )
        .wrap_err_with(|| {
            format!(
                "failed to securely open ballot state lock `{}`",
                path.display()
            )
        })?;
        let file = fs::File::from(descriptor);
        let metadata = file
            .metadata()
            .wrap_err("failed to inspect the ballot state lock")?;
        if !metadata.is_file()
            || metadata.nlink() != 1
            || metadata.permissions().mode() & 0o077 != 0
            || metadata.uid() != rustix::process::geteuid().as_raw()
        {
            bail!(
                "ballot state lock `{}` must be an owner-only, singly linked regular file owned \
                 by the current user",
                path.display()
            );
        }
        rustix::fs::flock(&file, rustix::fs::FlockOperation::LockExclusive)
            .wrap_err("failed to lock the ballot state file")?;
        Ok(Self { _file: file })
    }
}

#[cfg(not(unix))]
struct StateFileLock;

#[cfg(not(unix))]
impl StateFileLock {
    fn acquire(_: &Path) -> Result<Self> {
        bail!("ballot state file custody requires a Unix host")
    }
}

/// Whether 32 bytes form a canonical Iroha hash (non-zero, low bit set).
pub(super) fn is_canonical_hash(bytes: &[u8; 32]) -> bool {
    bytes.iter().any(|byte| *byte != 0) && bytes[31] & 1 == 1
}

/// Decode exactly 64 lowercase hexadecimal characters.
pub(super) fn decode_lower_hex32(value: &str, label: &str) -> Result<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        bail!("{label} must be exactly 64 lowercase hexadecimal characters");
    }
    let mut out = [0_u8; 32];
    hex::decode_to_slice(value, &mut out).map_err(|_| eyre!("{label} is invalid hexadecimal"))?;
    Ok(out)
}

/// Failure to publish a new file without replacing an existing one.
enum PublishError {
    AlreadyExists,
    Other(eyre::Report),
}

fn parent_directory(path: &Path) -> PathBuf {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

fn staging_path(path: &Path) -> Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or_else(|| eyre!("`{}` does not name a file", path.display()))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let mut staged = std::ffi::OsString::from(".");
    staged.push(name);
    staged.push(format!(".tmp-{}-{nanos}", std::process::id()));
    Ok(parent_directory(path).join(staged))
}

#[cfg(unix)]
fn validate_owner_only_metadata(metadata: &fs::Metadata, max_len: u64, label: &str) -> Result<()> {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.nlink() != 1
        || metadata.len() == 0
        || metadata.len() > max_len
    {
        bail!("{label} must be a non-empty, bounded, singly linked regular file");
    }
    if !matches!(metadata.permissions().mode() & 0o7777, 0o600 | 0o400) {
        bail!("{label} must be owner-only (mode 0600 or 0400)");
    }
    if metadata.uid() != rustix::process::geteuid().as_raw() {
        bail!("{label} must be owned by the current user");
    }
    Ok(())
}

#[cfg(unix)]
fn same_file_unchanged(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.uid() == after.uid()
        && before.mode() == after.mode()
        && before.nlink() == after.nlink()
        && before.len() == after.len()
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
}

/// Read one bounded owner-only file without following a final symlink.
#[cfg(unix)]
fn read_owner_only_file(path: &Path, max_len: u64, label: &str) -> Result<Zeroizing<Vec<u8>>> {
    use std::io::Read as _;
    let path_metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to inspect {label} `{}`", path.display()))?;
    validate_owner_only_metadata(&path_metadata, max_len, label)?;
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .wrap_err_with(|| format!("failed to securely open {label} `{}`", path.display()))?;
    let mut file = fs::File::from(descriptor);
    let before = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect opened {label}"))?;
    validate_owner_only_metadata(&before, max_len, label)?;
    if !same_file_unchanged(&path_metadata, &before) {
        bail!("{label} changed while it was being opened");
    }
    let capacity = usize::try_from(before.len())
        .map_err(|_| eyre!("{label} length exceeds the host width"))?;
    let mut bytes = Zeroizing::new(Vec::with_capacity(capacity));
    (&mut file)
        .take(max_len.saturating_add(1))
        .read_to_end(&mut bytes)
        .wrap_err_with(|| format!("failed to read {label}"))?;
    let after = file
        .metadata()
        .wrap_err_with(|| format!("failed to re-inspect {label}"))?;
    if !same_file_unchanged(&before, &after) || bytes.len() != capacity {
        bail!("{label} changed while it was being read");
    }
    Ok(bytes)
}

#[cfg(not(unix))]
fn read_owner_only_file(_: &Path, _: u64, label: &str) -> Result<Zeroizing<Vec<u8>>> {
    bail!("{label} custody requires a Unix host with owner-only file modes")
}

/// Write `bytes` to a fresh owner-only staging file next to `path` and sync it.
#[cfg(unix)]
fn write_staging_file(path: &Path, bytes: &[u8], label: &str) -> Result<PathBuf> {
    use std::io::Write as _;
    let staged = staging_path(path)?;
    let descriptor = rustix::fs::open(
        &staged,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .wrap_err_with(|| format!("failed to create a staging file for the {label}"))?;
    let result = (|| -> Result<()> {
        rustix::fs::fchmod(&descriptor, rustix::fs::Mode::from_raw_mode(0o600))
            .wrap_err_with(|| format!("failed to set owner-only mode on the {label}"))?;
        let mut file = fs::File::from(descriptor);
        file.write_all(bytes)
            .wrap_err_with(|| format!("failed to write the {label}"))?;
        file.sync_all()
            .wrap_err_with(|| format!("failed to sync the {label}"))?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&staged);
        return Err(error);
    }
    Ok(staged)
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path, label: &str) -> Result<()> {
    fs::File::open(parent_directory(path))
        .and_then(|directory| directory.sync_all())
        .wrap_err_with(|| format!("failed to sync the directory of the {label}"))
}

/// Publish a new owner-only file at `path` without replacing an existing one.
#[cfg(unix)]
fn publish_new_owner_only_file(path: &Path, bytes: &[u8], label: &str) -> Result<(), PublishError> {
    let staged = write_staging_file(path, bytes, label).map_err(PublishError::Other)?;
    let linked = fs::hard_link(&staged, path);
    let _ = fs::remove_file(&staged);
    match linked {
        Ok(()) => sync_parent_directory(path, label).map_err(PublishError::Other),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(PublishError::AlreadyExists)
        }
        Err(error) => Err(PublishError::Other(eyre!(error).wrap_err(format!(
            "failed to publish the {label} at `{}`",
            path.display()
        )))),
    }
}

#[cfg(not(unix))]
fn publish_new_owner_only_file(_: &Path, _: &[u8], label: &str) -> Result<(), PublishError> {
    Err(PublishError::Other(eyre!(
        "{label} custody requires a Unix host with owner-only file modes"
    )))
}

/// Atomically replace the owner-only file at `path`.
#[cfg(unix)]
fn replace_owner_only_file(path: &Path, bytes: &[u8], label: &str) -> Result<()> {
    let staged = write_staging_file(path, bytes, label)?;
    if let Err(error) = fs::rename(&staged, path) {
        let _ = fs::remove_file(&staged);
        return Err(eyre!(error).wrap_err(format!(
            "failed to replace the {label} at `{}`",
            path.display()
        )));
    }
    sync_parent_directory(path, label)
}

#[cfg(not(unix))]
fn replace_owner_only_file(_: &Path, _: &[u8], label: &str) -> Result<()> {
    bail!("{label} custody requires a Unix host with owner-only file modes")
}

/// Write one public masked-ballot record as lowercase hex, never replacing a
/// different existing record.
pub(super) fn write_public_record(path: &Path, record: &[u8]) -> Result<()> {
    use std::io::Write as _;
    let mut text = hex::encode(record);
    text.push('\n');
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => file
            .write_all(text.as_bytes())
            .and_then(|()| file.sync_all())
            .wrap_err_with(|| format!("failed to write ballot record `{}`", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = read_public_record(path, record.len())?;
            if existing != record {
                bail!(
                    "`{}` already holds a different ballot record",
                    path.display()
                );
            }
            Ok(())
        }
        Err(error) => Err(eyre!(error).wrap_err(format!(
            "failed to create ballot record `{}`",
            path.display()
        ))),
    }
}

/// Read one public masked-ballot record written by [`write_public_record`].
pub(super) fn read_public_record(path: &Path, expected_len: usize) -> Result<Vec<u8>> {
    use std::io::Read as _;
    let max_text = u64::try_from(expected_len.saturating_mul(2).saturating_add(2))
        .map_err(|_| eyre!("ballot record bound exceeds the host width"))?;
    let mut text = String::new();
    fs::File::open(path)
        .wrap_err_with(|| format!("failed to open ballot record `{}`", path.display()))?
        .take(max_text.saturating_add(1))
        .read_to_string(&mut text)
        .wrap_err_with(|| format!("failed to read ballot record `{}`", path.display()))?;
    let trimmed = text.strip_suffix('\n').unwrap_or(&text);
    if trimmed.len() != expected_len.saturating_mul(2)
        || !trimmed
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        bail!(
            "`{}` is not one lowercase-hex {expected_len}-byte ballot record",
            path.display()
        );
    }
    hex::decode(trimmed).map_err(|_| eyre!("`{}` is invalid hexadecimal", path.display()))
}
